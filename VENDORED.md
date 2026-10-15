# Vendored code

Third-party code committed into this repo (never fetched at build time).

| Component | Upstream | Commit | Licence | Local changes |
|---|---|---|---|---|
| `vendor/schwung/plugin_api_v1.h` | https://github.com/charlesvestal/schwung `src/host/plugin_api_v1.h` | cfeb2b0a (v1.6.3) | MIT | none |
| `engine/` (the synth engine, copied from the MPC plugin repo; CA-72's own vendoring is in `engine/VENDORED.md`) | https://github.com/sd88me/mpc-vst-model72 `engine/`, `vst/params.json`, `vst/presets.json` | see `engine/UPSTREAM` | GPL-3.0-or-later | none; refreshed by `scripts/sync_engine.sh`, pinned by `engine/SHA256SUMS` |
