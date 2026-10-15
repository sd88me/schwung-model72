#!/usr/bin/env bash
# engine/ is a copy of the MPC plugin repo's engine/ (plus vst/params.json and vst/presets.json) at the commit in engine/UPSTREAM.
#   scripts/check_engine.sh             fails when engine/ no longer matches engine/SHA256SUMS (a hand edit: fix it upstream and sync)
#   scripts/check_engine.sh --pin DIR   (sync_engine.sh) writes engine/SHA256SUMS
set -euo pipefail
cd "$(dirname "$0")/.."
files() { (cd engine && find . -type f ! -path './target/*' ! -path './.cargo-home/*' ! -name SHA256SUMS | LC_ALL=C sort); }
if [ "${1:-}" = "--pin" ]; then files | (cd engine && xargs sha256sum > SHA256SUMS); exit 0; fi
(cd engine && sha256sum -c --quiet SHA256SUMS) || { echo "engine/ was edited or is out of step with its pin: fix the engine in mpc-vst-model72 and run scripts/sync_engine.sh" >&2; exit 1; }
[ "$(files | wc -l)" = "$(wc -l < engine/SHA256SUMS)" ] || { echo "engine/ has files its pin does not list" >&2; exit 1; }
echo "engine/ matches its pin $(cut -c1-8 engine/UPSTREAM)"
