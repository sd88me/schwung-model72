# Model-72 for Schwung: agent guide

A Schwung (Ableton Move) sound-generator module around the CA-72 engine. Start with README.md. The engine is developed and
calibrated in the MPC plugin repo (sd88me/mpc-vst-model72); `engine/` here is a pinned copy refreshed by
`scripts/sync_engine.sh`, so fix engine bugs there and sync, not here (`scripts/check_engine.sh`, run by `test.sh`, fails on
a hand edit of `engine/`).

Ground rules:
- The module's own panel (derived switches, fixed PATCH/MAIN OUT, AT > Mod, presets, pages) lives in
  `tools/gen_schwung.py` and `src/schwung_plugin.c`; the engine keeps the MPC plugin's full parameter set.
- Schwung's threading contract (vendor/schwung/plugin_api_v1.h): every entry point runs on the audio callback. No file I/O,
  allocation, blocking or logging there; the engine is made and dropped by the worker thread in `src/schwung_plugin.c`.
- `module.json` stays tiny (loader cap 8 KB): `chain_params` / `ui_hierarchy` are served by the plugin.
- Run `scripts/test.sh` before building; `scripts/build.sh` cross-builds for aarch64.
- Don't deploy to `move` in `~/.ssh/config`: that alias points at the Akai Force. `scripts/deploy.sh` refuses 192.168.1.44.
- No "Moog"/"Minimoog" in the product name, module id or art.
