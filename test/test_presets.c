/* Each factory preset as converted for this module (VOLUME fixed at 10 and folded into LEVEL, the source switches into their
 * volumes) against the preset with its own VOLUME and LEVEL: the same note through two fresh engines must be as loud.
 * build/preset_pairs.txt (from scripts/test.sh): name, original state, converted state. */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
void *ca72_create(void); void ca72_destroy(void *e); void ca72_midi(void *e, const uint8_t *m, int len);
void ca72_set_param(void *e, const char *k, const char *v); void ca72_render(void *e, int16_t *out, int frames);
static double play(const char *state) {
    void *e = ca72_create();
    ca72_set_param(e, "state", state);
    int16_t o[256]; ca72_render(e, o, 128);   /* the preset is applied at a block's start, before the note (as on the device) */
    uint8_t on[3] = { 0x90, 48, 100 }; ca72_midi(e, on, 3);
    double en = 0;
    for (int b = 0; b < 200; b++) { ca72_render(e, o, 128); for (int i = 0; i < 256; i++) en += (double)o[i] * o[i]; }
    ca72_destroy(e);
    return sqrt(en / (200 * 256.0));
}
int main(int argc, char **argv) {
    static char name[100], a[8000], b[8000];
    FILE *f = fopen(argc > 1 ? argv[1] : "build/preset_pairs.txt", "r");
    if (!f) { puts("FAIL no preset pairs"); return 1; }
    int fails = 0, n = 0;
    while (fgets(name, sizeof name, f) && fgets(a, sizeof a, f) && fgets(b, sizeof b, f)) {
        name[strcspn(name, "\n")] = 0;
        double x = play(a), y = play(b), db = 20 * log10(y / x);
        int ok = x > 100 && fabs(db) < 0.05;
        printf("%s %-16s %+.3f dB\n", ok ? "ok  " : "FAIL", name, db);
        fails += !ok; n++;
    }
    printf(fails || n != 19 ? "FAILED\n" : "PASSED\n");
    return fails || n != 19;
}
