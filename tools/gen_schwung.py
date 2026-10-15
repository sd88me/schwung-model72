#!/usr/bin/env python3
"""Generate the Schwung (Ableton Move) module description from engine/params.json and engine/presets.json.

    gen_schwung.py            writes module.json, src/schwung_meta.h and build/contract.json

module.json is small on purpose (the loader caps it at 8 KB; a sound generator's chain_params / ui_hierarchy are read from the
plugin). schwung_meta.h embeds the two JSON strings, the enum option tables (label -> index) and the presets as engine "state"
strings, already converted to this module's panel:

  - no OSC 1/2/3, NOISE or EXTERNAL INPUT switches: a source is on while its VOLUME is above 0 (the plugin derives the switch);
  - PATCH (output cabled into EXTERNAL INPUT) always wired: EXT VOL is the patch amount, 0 is unpatched;
  - MAIN OUT always on; FEEDBACK (retired upstream) 0; the stereo field's dots and AUTO dB are the engine's own.

A preset's switched-off source becomes VOLUME 0, and its EXT VOL becomes 0 unless PATCH was on (EXTERNAL INPUT has no audio on
the MPC either, so nothing is lost).
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..")
VERSION = "1.0.0"
MODULE_ID = "model-72"

# the switches the plugin owns: (switch key, volume key it follows)
DERIVED = [("osc1_on", "osc1_volume"), ("osc2_on", "osc2_volume"), ("osc3_on", "osc3_volume"), ("noise_on", "noise_volume"),
           ("patch", "ext_volume")]
FIXED = {"ext_on": "Off", "main_output": "On", "feedback": 0, "volume": 10}
HIDDEN = {k for k, _ in DERIVED} | set(FIXED) | {"auto_db"} | {"dot_%d" % i for i in range(1, 11)}

# (level key, page name, cell-label prefix for the held-knob header, [param keys]); eight knobs a page at most, in knob order
PAGES = [
    ("osc", "Oscillators", "", ["osc1_range", "osc1_waveform", "osc2_range", "osc2_waveform", "osc2_frequency", "osc3_range",
                                "osc3_waveform", "osc3_frequency"]),
    ("mixer", "Mixer", "", ["osc1_volume", "osc2_volume", "osc3_volume", "noise_volume", "ext_volume", "noise_type", "drive",
                            "bass_comp"]),
    ("filter", "Filter", "", ["cutoff", "emphasis", "contour_amount", "filter_attack", "filter_decay", "filter_sustain",
                              "filter_mode", "filter_mod"]),
    ("loudness", "Loudness", "", ["loudness_attack", "loudness_decay", "loudness_sustain", "level", "auto_gain"]),
    ("controller", "Controller", "", ["tune", "glide", "glide_on", "decay_on", "a440", "keyboard_control_1", "keyboard_control_2"]),
    ("mod", "Modulation", "", ["mod_mix", "osc_mod", "osc3_control", "mod_wheel", "midi_bend_range", "at_mod"]),
    ("voice", "Voice", "", ["quality", "mode", "voices", "entropy", "note_priority", "retrigger"]),
    ("stereo", "Stereo", "", ["spread", "inner", "double", "played_as", "placement"]),
]
# readouts listed on a page (not on a knob)
READOUTS = {"loudness": ["auto_db"]}
# the root page's knobs: the playable eight
ROOT_KNOBS = ["cutoff", "emphasis", "contour_amount", "filter_decay", "loudness_decay", "glide", "mod_wheel", "level"]

# (full name for the held-knob header, short cell label); keys not listed use params.json's name for both
NAMES = {
    "osc1_range": ("Osc 1 Range", "Range 1"), "osc1_waveform": ("Osc 1 Wave", "Wave 1"),
    "osc2_range": ("Osc 2 Range", "Range 2"), "osc2_waveform": ("Osc 2 Wave", "Wave 2"), "osc2_frequency": ("Osc 2 Freq", "Freq 2"),
    "osc3_range": ("Osc 3 Range", "Range 3"), "osc3_waveform": ("Osc 3 Wave", "Wave 3"), "osc3_frequency": ("Osc 3 Freq", "Freq 3"),
    "osc1_volume": ("Osc 1 Volume", "Osc 1"), "osc2_volume": ("Osc 2 Volume", "Osc 2"), "osc3_volume": ("Osc 3 Volume", "Osc 3"),
    "noise_volume": ("Noise Volume", "Noise"), "ext_volume": ("Ext In / Feedback", "Fdbk"), "noise_type": ("Noise Type", "Color"),
    "drive": ("Filter Drive", "Drive"), "bass_comp": ("Bass Comp", "BassC"),
    "contour_amount": ("Contour Amount", "Amount"), "filter_attack": ("Filter Attack", "Attack"),
    "filter_decay": ("Filter Decay", "Decay"), "filter_sustain": ("Filter Sustain", "Sustain"),
    "filter_mode": ("Filter Mode", "Mode"), "filter_mod": ("Filter Mod", "Mod"),
    "loudness_attack": ("Loud Attack", "Attack"), "loudness_decay": ("Loud Decay", "Decay"), "loudness_sustain": ("Loud Sustain", "Sustain"),
    "level": ("Level", "Level"), "auto_gain": ("Auto Gain", "AutoG"), "auto_db": ("Auto Gain dB", "Auto dB"),
    "glide_on": ("Glide Switch", "GlideSw"), "decay_on": ("Decay Switch", "DecaySw"), "a440": ("A-440", "A-440"),
    "keyboard_control_1": ("Kbd Control 1", "Kbd 1"), "keyboard_control_2": ("Kbd Control 2", "Kbd 2"),
    "mod_mix": ("Mod Mix", "Mix"), "osc_mod": ("Osc Mod", "OscMod"), "osc3_control": ("Osc 3 Control", "O3Ctl"),
    "mod_wheel": ("Mod Wheel", "Wheel"), "midi_bend_range": ("Bend Range", "Bend"), "at_mod": ("Aftertouch > Mod", "AT>Mod"),
    "note_priority": ("Note Priority", "Prio"), "retrigger": ("Retrigger", "Retrig"),
    "spread": ("Width", "Width"), "double": ("Detune", "Detune"), "played_as": ("Played As", "Play"),
}
SHORT_OPT = {"Triangle": "Tri", "Shark": "Shrk", "Saw": "Saw", "Square": "Sqr", "Wide": "Wide", "Narrow": "Narw", "Rev Saw": "RSaw",
             "MONO": "Mono", "POLY": "Poly", "UNISON": "Uni", "SCATTER": "Scat", "DOUBLE": "Dbl", "EVEN": "Even", "CENTER": "Cntr",
             "White": "Wht", "Pink": "Pink", "Low": "Low", "Last": "Last", "High": "High"}
# (step, unit) for the continuous controls; the rest step 0.1 on the panel's 0-10 dials
STEPS = {"tune": (0.01, None), "osc2_frequency": (0.05, None), "osc3_frequency": (0.05, None), "cutoff": (0.05, None),
         "drive": (0.5, "dB"), "level": (0.5, "dB"), "auto_db": (0.1, "dB"), "mod_wheel": (0.01, None),
         "midi_bend_range": (0.01, "st"), "entropy": (1, None), "spread": (1, None), "inner": (1, None), "double": (1, None)}
EXTRA = {"at_mod": {"key": "at_mod", "name": "AT > Mod", "options": ["Off", "On"], "default": "Off"}}


def load():
    d = json.load(open(os.path.join(ROOT, "engine", "params.json")))
    params = {p["key"]: p for p in d["params"]}
    params.update(EXTRA)
    return d["params"], params


def num(v):
    return int(v) if float(v).is_integer() else float(v)


def param_meta(key, p):
    full, short = NAMES.get(key, (p["name"], p["name"]))
    m = {"key": key, "name": full[:31]}
    if short != full: m["short_name"] = short[:15]
    if "options" in p:
        d = p.get("default", p["options"][0])
        m["type"] = "enum"; m["options"] = p["options"]; m["default"] = p["options"].index(d) if isinstance(d, str) else d
        if any(o in SHORT_OPT and SHORT_OPT[o] != o for o in p["options"]):
            m["short_options"] = [SHORT_OPT.get(o, o[:4]) for o in p["options"]]
    elif key == "voices" or key in ("entropy", "spread", "inner", "double"):
        m["type"] = "int"; m["min"] = num(p["min"]); m["max"] = num(p["max"]); m["default"] = num(p.get("default", 0))
        if key != "voices": m["unit"] = "%"
    else:
        step, unit = STEPS.get(key, (0.1, None))
        m["type"] = "float"; m["min"] = num(p["min"]); m["max"] = num(p["max"]); m["step"] = step; m["default"] = num(p.get("default", 0))
        if unit: m["unit"] = unit
    if key == "auto_db": m["access"] = "read"
    return m


def convert(values, params):
    """A preset (or the engine's defaults) on this module's panel: {key: index or number}, every engine key."""
    out = {}
    for key, p in params.items():
        if key in EXTRA or key.startswith("dot_") or key == "auto_db": continue
        v = values.get(key, p.get("default", 0))
        out[key] = p["options"].index(v) if "options" in p and isinstance(v, str) else v
    on = lambda k: out[k] == 1
    for sw, vol in DERIVED[:4]:
        if not on(sw): out[vol] = 0
    if not on("patch"): out["ext_volume"] = 0
    for sw, vol in DERIVED: out[sw] = 1 if out[vol] > 0 else 0
    # VOLUME fixed at 10: a preset's lower VOLUME goes into LEVEL (both are the output's gain, after the voices)
    out["level"] = round(min(12.0, out["level"] + 20 * math.log10(output_gain(out["volume"] / 10) / output_gain(1.0))), 2)
    for k, v in FIXED.items(): out[k] = params[k]["options"].index(v) if isinstance(v, str) else v
    return out


def output_gain(v):
    """The engine's VOLUME law (ca72-mpc lib.rs `output_gain`): an audio-taper 5K pot loaded by 10K."""
    t = (81.0 ** min(max(v, 0.0), 1.0) - 1.0) / 80.0
    top, bottom = 5e3 * (1 - t), 5e3 * t
    load = bottom * 10e3 / (bottom + 10e3)
    return load / (top + load) if load > 0 else 1e-9


def state(vals):
    return ";".join("%s=%s" % (k, num(v)) for k, v in vals.items())


def build():
    order, params = load()
    defaults = convert({}, params)
    metas, levels, root_items, seen = [], {}, [], set()
    for lkey, name, _, keys in PAGES:
        for k in keys + READOUTS.get(lkey, []):
            assert k not in seen, k
            seen.add(k)
            m = param_meta(k, params[k])
            if k in defaults: m["default"] = defaults[k]
            metas.append(m)
        items = [{"key": k, "name": NAMES.get(k, (params[k]["name"],) * 2)[1]} for k in keys + READOUTS.get(lkey, [])]
        levels[lkey] = {"name": name, "params": items, "knobs": keys[:8]}
        root_items.append({"level": lkey, "name": name})
    missing = [p["key"] for p in order if p["key"] not in seen and p["key"] not in HIDDEN]
    assert not missing, "params on no page: %s" % missing
    metas.append({"key": "preset", "name": "Preset", "type": "int", "min": 0, "max": 255, "default": 0})
    root = {"name": "Model-72", "list_param": "preset", "count_param": "preset_count", "name_param": "preset_name",
            "params": root_items, "knobs": ROOT_KNOBS}
    hier = {"modes": None, "levels": dict(root=root, **levels)}
    presets = [("Init", defaults)]
    for pr in json.load(open(os.path.join(ROOT, "engine", "presets.json")))["presets"]:
        presets.append((pr["name"], convert(pr["values"], params)))
    return metas, hier, presets


def preset_pairs(path):
    """For test/test_presets.c: each factory preset's state with its own VOLUME and LEVEL, and as converted."""
    _, params = load()
    with open(path, "w") as f:
        for pr in json.load(open(os.path.join(ROOT, "engine", "presets.json")))["presets"]:
            c = convert(pr["values"], params)
            o = dict(c, volume=pr["values"]["volume"], level=pr["values"]["level"])
            f.write("%s\n%s\n%s\n" % (pr["name"], state(o), state(c)))


def cstr(s):
    esc = lambda c: c.replace("\\", "\\\\").replace('"', '\\"')
    return "\n    ".join('"%s"' % esc(s[i:i + 120]) for i in range(0, len(s), 120)) or '""'


def main():
    metas, hier, presets = build()
    cp = json.dumps(metas, separators=(",", ":"))
    uh = json.dumps(hier, separators=(",", ":"))
    print("params %d, chain_params %d bytes, ui_hierarchy %d bytes, presets %d" % (len(metas), len(cp), len(uh), len(presets)),
          file=sys.stderr)
    assert len(cp) < 60000 and len(uh) < 60000
    module = {"id": MODULE_ID, "name": "Model-72", "abbrev": "M72", "version": VERSION,
              "description": "Circuit-modelled 1970s monosynth (CA-72): three oscillators, the ladder filter, mono/poly/unison, stereo",
              "dsp": "dsp.so", "api_version": 2, "component_type": "sound_generator", "author": "sd88me",
              "capabilities": {"audio_out": True, "midi_in": True, "midi_out": False, "aftertouch": True, "chainable": True,
                               "component_type": "sound_generator"}}
    os.makedirs(os.path.join(ROOT, "src"), exist_ok=True)
    os.makedirs(os.path.join(ROOT, "build"), exist_ok=True)
    json.dump(module, open(os.path.join(ROOT, "module.json"), "w"), indent=1)
    json.dump({"chain_params": metas, "ui_hierarchy": hier, "presets": [n for n, _ in presets]},
              open(os.path.join(ROOT, "build", "contract.json"), "w"))
    h = ["/* Generated by tools/gen_schwung.py; do not edit. */", "#pragma once", "#define MODULE_ID \"%s\"" % MODULE_ID,
         "static const char SCHWUNG_CHAIN_PARAMS[] =\n    %s;" % cstr(cp),
         "static const char SCHWUNG_UI_HIERARCHY[] =\n    %s;" % cstr(uh),
         "typedef struct { const char *key; int nopt; const char *const *opt; } schwung_enum_t;"]
    enums = [m for m in metas if m["type"] == "enum"]
    for i, m in enumerate(enums):
        h.append("static const char *const SCHWUNG_OPT_%d[] = { %s };" % (i, ", ".join('"%s"' % o.replace('"', "'") for o in m["options"])))
    h.append("static const schwung_enum_t SCHWUNG_ENUMS[] = {")
    h += ['    { "%s", %d, SCHWUNG_OPT_%d },' % (m["key"], len(m["options"]), i) for i, m in enumerate(enums)]
    h.append("};\n#define SCHWUNG_NENUMS %d" % len(enums))
    h.append("/* the switches the plugin derives from a volume: on while it is above 0 */")
    h.append("static const char *const DERIVED[][2] = { %s };" % ", ".join('{ "%s", "%s" }' % d for d in DERIVED))
    h.append("#define NDERIVED %d" % len(DERIVED))
    h.append("typedef struct { const char *name; const char *state; } preset_t;")
    h.append("static const preset_t PRESETS[] = {")
    h += ['    { "%s", %s },' % (n, cstr(state(v))) for n, v in presets]
    h.append("};\n#define NPRESETS %d" % len(presets))
    open(os.path.join(ROOT, "src", "schwung_meta.h"), "w").write("\n".join(h) + "\n")


if __name__ == "__main__":
    main()
