#!/usr/bin/env bash
# The module's contract checks and a host simulation of the plugin on x86 under ASan/UBSan. No Move needed.
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/check_engine.sh
python3 tools/gen_schwung.py
python3 - <<'PY'
import json, os
d = json.load(open("build/contract.json"))
assert os.path.getsize("module.json") < 8192, "module.json over the loader's 8 KB cap"
cp = d["chain_params"]; keys = [m["key"] for m in cp]
assert len(keys) == len(set(keys)) <= 256, "duplicate or too many params"
levels = d["ui_hierarchy"]["levels"]
for name, lv in levels.items():
    for p in lv.get("params", []):
        if "key" in p: assert p["key"] in keys, (name, p["key"])
    assert len(lv.get("knobs", [])) <= 8, name
    for k in lv.get("knobs", []): assert k in keys and k != "preset", (name, k)
txt = json.dumps(d)
assert all(ord(c) < 128 for c in txt), "non-ASCII text (the device's 5x7 font cannot draw it)"
for m in cp:
    if m["type"] == "enum": assert m["options"] and all(len(o) <= 31 for o in m["options"]), m["key"]
    assert len(m["name"]) <= 31 and len(m.get("short_name", "")) <= 15, m["key"]
    if "short_options" in m: assert len(m["short_options"]) == len(m["options"]), m["key"]
print("contract: %d params, %d levels, %d presets" % (len(keys), len(levels), len(d["presets"])))
PY
[ -f build/rust/x86_64/libca72_mpc.a ] && [ -z "$(find engine -newer build/rust/x86_64/libca72_mpc.a -type f ! -path '*/target/*' | head -1)" ] || scripts/rust.sh
gcc -O1 -g -Wall -Wextra -fsanitize=address,undefined -Ivendor/schwung -Isrc src/schwung_plugin.c test/host_sim.c -o build/host_sim \
    -Lbuild/rust/x86_64 -lca72_mpc -lpthread -ldl -lm
echo "== host_sim"; ASAN_OPTIONS=detect_leaks=0 ./build/host_sim
python3 -c 'import sys; sys.path.insert(0, "tools"); import gen_schwung; gen_schwung.preset_pairs("build/preset_pairs.txt")'
gcc -O2 -Wall test/test_presets.c -o build/test_presets -Lbuild/rust/x86_64 -lca72_mpc -lpthread -ldl -lm
echo "== test_presets"; ./build/test_presets
