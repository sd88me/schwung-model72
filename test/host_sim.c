/* Host simulation for the Schwung module: drives plugin_api_v2 the way the chain host does (x86, no Move needed). */
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "plugin_api_v1.h"
extern plugin_api_v2_t *move_plugin_init_v2(const host_api_v1_t *host);
static int fails;
#define CHECK(c, m) do { int ok_ = (c); printf("%s %s\n", ok_ ? "ok  " : "FAIL", m); fails += !ok_; } while (0)
static void sleep_ms(int ms) { struct timespec ts = { 0, ms * 1000000L }; nanosleep(&ts, NULL); }
static plugin_api_v2_t *a;
static void *h;
static char buf[70000];
static double block_rms(int blocks) {
    int16_t out[256]; double e = 0;
    for (int b = 0; b < blocks; b++) { a->render_block(h, out, 128); for (int i = 0; i < 256; i++) e += (double)out[i] * out[i]; }
    return sqrt(e / (blocks * 256.0));
}
static double get(const char *k) { return a->get_param(h, k, buf, sizeof buf) > 0 ? atof(buf) : -999; }
static void midi3(uint8_t s, uint8_t d1, uint8_t d2) { uint8_t m[3] = { s, d1, d2 }; a->on_midi(h, m, 3, MOVE_MIDI_SOURCE_INTERNAL); }
static void quiet(void) { midi3(0xB0, 123, 0); for (int i = 0; i < 60; i++) block_rms(10); }

int main(void) {
    a = move_plugin_init_v2(NULL);
    CHECK(a && a->api_version == MOVE_PLUGIN_API_VERSION_2, "init returns the v2 table");
    h = a->create_instance("/nonexistent", NULL);
    CHECK(h != NULL, "create_instance returns at once");
    CHECK(a->get_param(h, "chain_params", buf, sizeof buf) > 1000 && buf[0] == '[', "chain_params is served before the engine is ready");
    CHECK(a->get_param(h, "ui_hierarchy", buf, sizeof buf) > 1000 && buf[0] == '{', "ui_hierarchy is served before the engine is ready");
    for (int t = 0; t < 600 && get("ready") != 1; t++) sleep_ms(10);
    CHECK(get("ready") == 1, "the worker publishes the engine");
    CHECK(get("preset_count") == 20 && get("preset") == 0, "20 presets, Init loaded");
    CHECK(get("osc1_on") == 1 && get("osc2_on") == 0 && get("patch") == 0 && get("main_output") == 1, "Init's switches follow its volumes");

    midi3(0x90, 48, 100);
    double base = block_rms(80);
    CHECK(base > 50.0, "a note makes sound");
    a->set_param(h, "osc1_volume", "0");
    CHECK(get("osc1_on") == 0, "Osc 1 at volume 0 switches it off");
    block_rms(40);
    CHECK(block_rms(40) < base * 0.05, "and silences it (no other source on)");
    a->set_param(h, "osc1_volume", "8");
    CHECK(get("osc1_on") == 1 && block_rms(40) > 50.0, "volume up switches it back on");
    a->set_param(h, "osc1_on", "0");
    CHECK(get("osc1_on") == 1, "the switch itself is not writable");
    a->set_param(h, "ext_volume", "3");
    CHECK(get("patch") == 1 && get("ext_on") == 0, "Ext volume above 0 patches the output in");
    a->set_param(h, "ext_volume", "0");
    CHECK(get("patch") == 0, "Ext volume 0 unpatches");
    quiet();

    a->set_param(h, "filter_mode", "HI");
    CHECK(get("filter_mode") == 1, "an enum accepts its label");
    a->set_param(h, "filter_mode", "0");
    CHECK(get("filter_mode") == 0, "and its index");
    a->set_param(h, "cutoff", "1.250");
    CHECK(fabs(get("cutoff") - 1.25) < 1e-9, "a float is read");

    a->set_param(h, "preset", "3");
    CHECK(get("preset") == 3 && !strcmp((a->get_param(h, "preset_name", buf, sizeof buf), buf), "Feedback Funk"), "preset 3 is Feedback Funk");
    CHECK(get("patch") == 1 && get("ext_volume") > 0, "Feedback Funk keeps its patch");
    a->set_param(h, "preset", "1");
    CHECK(get("osc3_volume") == 0 && get("osc3_on") == 0 && get("ext_volume") == 0 && get("noise_volume") == 0, "Felt Bass: its switched-off sources at volume 0");

    CHECK(get("at_mod") == 0, "AT > Mod is off by default");
    a->set_param(h, "at_mod", "On");
    CHECK(get("at_mod") == 1, "AT > Mod takes its label");
    a->set_param(h, "mod_mix", "10"); a->set_param(h, "osc_mod", "1"); a->set_param(h, "filter_mod", "1");
    midi3(0x90, 60, 100);
    double dry = block_rms(40);
    midi3(0xA0, 60, 127);
    double wet = block_rms(40);
    CHECK(fabs(wet - dry) > 1.0, "pad pressure moves the mod wheel");
    midi3(0x80, 60, 0);
    quiet();

    {   /* VOLUME is fixed (test_presets.c checks the presets' VOLUME went into LEVEL) */
        a->set_param(h, "preset", "7");
        CHECK(get("volume") == 10, "Volume is fixed at 10");
        a->set_param(h, "volume", "5");
        CHECK(get("volume") == 10, "and not writable");
    }
    CHECK(get("auto_db") <= 0 && get("auto_db") > -16, "Auto dB reads");
    CHECK(a->get_param(h, "no_such_key", buf, sizeof buf) < 0, "an unknown key reads as missing");

    a->set_param(h, "quality", "HI");
    for (int t = 0; t < 3000 && get("hi_ready") != 1; t++) { block_rms(1); sleep_ms(10); }
    CHECK(get("hi_ready") == 1, "HI's circuit filter gets built");
    midi3(0x90, 48, 100);
    CHECK(block_rms(80) > 50.0, "HI plays");
    quiet();

    a->destroy_instance(h);
    sleep_ms(100);
    printf(fails ? "FAILED\n" : "PASSED\n");
    return fails;
}
