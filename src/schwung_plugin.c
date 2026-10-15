/* Model-72 as a Schwung (Ableton Move) sound generator: plugin API v2 around the CA-72 engine (engine/ca72-mpc, Rust, C ABI).
 *
 * Schwung runs every entry point on the SPI audio callback (vendor/schwung/plugin_api_v1.h, "THREADING CONTRACT"): no file I/O,
 * no allocation, no blocking, no logging. Making the engine allocates its ten voices and starts the thread that builds QUALITY
 * HI's circuit filter, and dropping it joins that thread. So:
 *   - create_instance only allocates the small wrapper and starts a worker (demoted to SCHED_OTHER on cores 0-2, as the
 *     contract demands; the engine's own builder thread inherits that);
 *   - the worker makes the engine, loads the Init sound and publishes it; until then render_block writes silence and MIDI and
 *     parameters are dropped;
 *   - destroy_instance flags the worker, which drops the engine and frees the wrapper; the library's destructor waits for the
 *     workers so dlclose never unmaps code one is still running.
 * Everything the audio thread calls on the engine (midi, render, set/get of a parameter) is the code the MPC plugin runs in its
 * audio callback.
 *
 * This module's panel differs from the engine's (tools/gen_schwung.py): the OSC 1-3, NOISE and PATCH switches follow their
 * VOLUME (on above 0), EXTERNAL INPUT and MAIN OUT are fixed, and AT > MOD turns pad pressure into the modulation wheel. */
#define _GNU_SOURCE
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "plugin_api_v1.h"
#include "schwung_meta.h"

void *ca72_create(void);
void ca72_destroy(void *e);
void ca72_midi(void *e, const uint8_t *m, int len);
void ca72_set_param(void *e, const char *k, const char *v);
int ca72_get_param(void *e, const char *k, char *buf, int len);
void ca72_render(void *e, int16_t *out, int frames);

typedef struct {
    void *eng;                        /* published by the worker */
    atomic_int state;                 /* 0 loading, 1 ready, 2 failed */
    atomic_int closing;               /* set by destroy_instance; the worker then frees everything */
    int preset;
    int at_mod;                       /* AT > MOD */
    uint8_t cc_mod;                   /* the last mod wheel (CC 1) received */
    uint8_t at_chan;                  /* channel pressure */
    uint8_t at_key[128];              /* poly pressure of each held key */
    uint8_t sent_mod;                 /* what the engine last got as CC 1 */
} wrap_t;

static void sleep_ms(int ms) { struct timespec ts = { 0, ms * 1000000L }; nanosleep(&ts, NULL); }

/* Workers still running code from this library: Schwung dlcloses the module right after the last instance is destroyed. */
static atomic_int g_workers;
__attribute__((destructor)) static void wait_for_workers(void) {
    for (int i = 0; i < 15000 && atomic_load(&g_workers) > 0; i++) sleep_ms(2);   /* at most 30 s (HI's filter build) */
}

static void derive_switches(wrap_t *w) {
    char v[32];
    for (int i = 0; i < NDERIVED; i++)
        if (ca72_get_param(w->eng, DERIVED[i][1], v, sizeof v) > 0)
            ca72_set_param(w->eng, DERIVED[i][0], strtod(v, NULL) > 0.0 ? "1" : "0");
}

static void load_preset(wrap_t *w, int i) {
    if (i < 0 || i >= NPRESETS) return;
    w->preset = i;
    ca72_set_param(w->eng, "state", PRESETS[i].state);
    derive_switches(w);
}

static void *worker_main(void *arg) {
    wrap_t *w = arg;
    struct sched_param sp = { .sched_priority = 0 };   /* threads inherit the callback's SCHED_FIFO 70: demote first and keep off core 3 */
    sched_setscheduler(0, SCHED_OTHER, &sp);
    cpu_set_t set; CPU_ZERO(&set); CPU_SET(0, &set); CPU_SET(1, &set); CPU_SET(2, &set);
    sched_setaffinity(0, sizeof set, &set);
    void *e = ca72_create();
    if (e) { w->eng = e; load_preset(w, 0); atomic_store(&w->state, 1); } else atomic_store(&w->state, 2);
    while (!atomic_load(&w->closing)) sleep_ms(5);
    if (e) ca72_destroy(e);
    free(w);
    atomic_fetch_sub(&g_workers, 1);   /* last access to anything in this library */
    return NULL;
}

static void *sw_create(const char *module_dir, const char *json_defaults) {
    (void)module_dir; (void)json_defaults;
    wrap_t *w = calloc(1, sizeof *w);
    if (!w) return NULL;
    pthread_t t;
    atomic_fetch_add(&g_workers, 1);
    if (pthread_create(&t, NULL, worker_main, w)) { atomic_fetch_sub(&g_workers, 1); free(w); return NULL; }
    pthread_detach(t);
    return w;
}

static void sw_destroy(void *p) { if (p) atomic_store(&((wrap_t *)p)->closing, 1); }   /* the worker owns the teardown */

static int ready(wrap_t *w) { return w && atomic_load(&w->state) == 1; }

/* The mod wheel the engine hears: CC 1, or with AT > MOD the pads' pressure where that is higher (the panel's MOD WHEEL knob
 * still adds on top in the engine, the higher of the two winning). */
static void send_mod(wrap_t *w) {
    uint8_t m = w->cc_mod;
    if (w->at_mod) {
        if (w->at_chan > m) m = w->at_chan;
        for (int k = 0; k < 128; k++) if (w->at_key[k] > m) m = w->at_key[k];
    }
    if (m == w->sent_mod) return;
    w->sent_mod = m;
    uint8_t cc[3] = { 0xB0, 1, m };
    ca72_midi(w->eng, cc, 3);
}

static void sw_midi(void *p, const uint8_t *msg, int len, int source) {
    (void)source;
    wrap_t *w = p;
    if (!ready(w) || len < 1) return;
    uint8_t st = msg[0] & 0xF0;
    switch (st) {
    case 0xA0: if (len >= 3) { w->at_key[msg[1] & 0x7F] = msg[2] & 0x7F; send_mod(w); } return;
    case 0xD0: if (len >= 2) { w->at_chan = msg[1] & 0x7F; send_mod(w); } return;
    case 0xB0: if (len >= 3 && msg[1] == 1) { w->cc_mod = msg[2] & 0x7F; send_mod(w); return; } break;
    case 0x80: case 0x90:
        if (len >= 3 && (st == 0x80 || msg[2] == 0) && w->at_key[msg[1] & 0x7F]) { w->at_key[msg[1] & 0x7F] = 0; send_mod(w); }
        break;
    }
    ca72_midi(w->eng, msg, len);
}

/* enum parameters arrive as an option label or an index; the engine takes the index */
static int enum_index(const char *key, const char *val) {
    for (int i = 0; i < SCHWUNG_NENUMS; i++)
        if (!strcmp(SCHWUNG_ENUMS[i].key, key))
            for (int k = 0; k < SCHWUNG_ENUMS[i].nopt; k++)
                if (!strcmp(SCHWUNG_ENUMS[i].opt[k], val)) return k;
    return -1;
}

static void sw_set(void *p, const char *key, const char *val) {
    wrap_t *w = p;
    if (!ready(w) || !key || !val) return;
    char num[16];
    char *endp; (void)strtod(val, &endp);
    if (val[0] && *endp) {   /* not a number: an option label */
        int i = enum_index(key, val);
        if (i < 0) return;
        snprintf(num, sizeof num, "%d", i); val = num;
    }
    if (!strcmp(key, "preset")) { load_preset(w, atoi(val)); return; }
    if (!strcmp(key, "at_mod")) { w->at_mod = atoi(val) != 0; send_mod(w); return; }
    for (int i = 0; i < NDERIVED; i++)   /* the switches are the plugin's own */
        if (!strcmp(key, DERIVED[i][0])) return;
    if (!strcmp(key, "ext_on") || !strcmp(key, "main_output") || !strcmp(key, "feedback") || !strcmp(key, "volume")) return;
    ca72_set_param(w->eng, key, val);
    if (!strcmp(key, "state")) { derive_switches(w); return; }
    for (int i = 0; i < NDERIVED; i++)
        if (!strcmp(key, DERIVED[i][1])) ca72_set_param(w->eng, DERIVED[i][0], strtod(val, NULL) > 0.0 ? "1" : "0");
}

static int copy_str(char *buf, int n, const char *s) {
    int len = (int)strlen(s);
    if (len >= n) return -1;
    memcpy(buf, s, (size_t)len + 1);
    return len;
}

static int sw_get(void *p, const char *key, char *buf, int n) {
    wrap_t *w = p;
    if (!w || !key || !buf || n < 2) return -1;
    if (!strcmp(key, "chain_params")) return copy_str(buf, n, SCHWUNG_CHAIN_PARAMS);
    if (!strcmp(key, "ui_hierarchy")) return copy_str(buf, n, SCHWUNG_UI_HIERARCHY);
    if (!strcmp(key, "module_id")) return copy_str(buf, n, MODULE_ID);
    if (!strcmp(key, "ready")) return snprintf(buf, (size_t)n, "%d", atomic_load(&w->state));
    if (!strcmp(key, "preset_count")) return snprintf(buf, (size_t)n, "%d", NPRESETS);
    if (!ready(w)) {
        if (!strcmp(key, "preset_name")) return copy_str(buf, n, atomic_load(&w->state) == 2 ? "(load failed)" : "(loading)");
        return -1;
    }
    if (!strcmp(key, "preset")) return snprintf(buf, (size_t)n, "%d", w->preset);
    if (!strcmp(key, "preset_name")) return copy_str(buf, n, PRESETS[w->preset].name);
    if (!strcmp(key, "at_mod")) return snprintf(buf, (size_t)n, "%d", w->at_mod);
    int r = ca72_get_param(w->eng, key, buf, n);
    return r > 0 ? r - 1 : -1;
}

static int sw_error(void *p, char *buf, int n) {
    wrap_t *w = p;
    if (w && atomic_load(&w->state) == 2 && n > 8) return copy_str(buf, n, "Model-72 could not start");
    return 0;
}

static void sw_render(void *p, int16_t *out, int frames) {
    wrap_t *w = p;
    if (!ready(w)) { memset(out, 0, (size_t)frames * 4); return; }
    ca72_render(w->eng, out, frames);
}

static plugin_api_v2_t api = {
    .api_version = MOVE_PLUGIN_API_VERSION_2,
    .create_instance = sw_create, .destroy_instance = sw_destroy, .on_midi = sw_midi,
    .set_param = sw_set, .get_param = sw_get, .get_error = sw_error, .render_block = sw_render,
};

plugin_api_v2_t *move_plugin_init_v2(const host_api_v1_t *host) { (void)host; return &api; }
