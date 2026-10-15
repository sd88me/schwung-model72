#!/usr/bin/env bash
# Refresh engine/ from a checkout of the MPC plugin repo (sd88me/mpc-vst-model72), where the engine is developed and calibrated.
#   scripts/sync_engine.sh [path to that checkout, default ../mpc-vst-model72]
set -euo pipefail
cd "$(dirname "$0")/.."
UP="${1:-../mpc-vst-model72}"
rm -rf engine
git -C "$UP" archive HEAD engine | tar -x
cp "$UP/vst/params.json" "$UP/vst/presets.json" engine/
git -C "$UP" rev-parse HEAD > engine/UPSTREAM
scripts/check_engine.sh --pin "$UP"
python3 tools/gen_schwung.py
echo "engine/ now matches $(cut -c1-8 engine/UPSTREAM) of $UP; run scripts/test.sh, then commit"
