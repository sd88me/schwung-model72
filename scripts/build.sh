#!/usr/bin/env bash
# Cross-compile the Schwung module for Ableton Move (aarch64 Linux) and package it as build/model-72-module.tar.gz.
# Needs Docker; nothing on a device is touched. Run scripts/test.sh first.
set -euo pipefail
cd "$(dirname "$0")/.."
python3 tools/gen_schwung.py
scripts/rust.sh
OUT=build/modules/sound_generators/model-72
rm -rf build/modules build/model-72-module.tar.gz
mkdir -p "$OUT"
cp module.json help.json LICENSE THIRD-PARTY-NOTICES.txt "$OUT/"
docker run --rm -u "$(id -u):$(id -g)" -v "$PWD":/w -w /w model72-schwung-build bash -euc "
    aarch64-linux-gnu-gcc -O2 -shared -fPIC -Wall -Ivendor/schwung -Isrc src/schwung_plugin.c -o $OUT/dsp.so \
        -Lbuild/rust/aarch64 -lca72_mpc -Wl,--exclude-libs,ALL -Wl,--gc-sections -lpthread -ldl -lm
    aarch64-linux-gnu-strip --strip-unneeded $OUT/dsp.so
    aarch64-linux-gnu-objdump -T $OUT/dsp.so | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1
    aarch64-linux-gnu-objdump -T $OUT/dsp.so | grep -c move_plugin_init_v2
    tar --owner=0 --group=0 -czf build/model-72-module.tar.gz -C build/modules/sound_generators model-72
    tar -tzvf build/model-72-module.tar.gz
"
echo "Built: build/model-72-module.tar.gz"
