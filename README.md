# Model-72 for Schwung

**A circuit-modelled 1970s monosynth for Ableton Move**, as a [Schwung](https://github.com/charlesvestal/schwung) sound
generator.

Model-72 is a port of [**CA-72**](https://github.com/idlefoundry/ca-72), Idle Foundry's free, open-source software
model of the Minimoog Model D (about 1972-73, the CA3046 "old" oscillator board). CA-72's sound is derived from the
instrument's own circuit: every board was transcribed from its service documents into SPICE netlists and simulated,
the real-time models were derived from those simulations and tested against them, and the result was calibrated
against a hardware instrument. It is an exceptionally accurate and excellent-sounding digital emulation, and it is the
reason this module exists.

Model-72 puts that engine on Move as a native Schwung module, its panel laid out as Move's knob pages, and makes it run
in real time on Move's CPU. It plays in a Signal Chain slot like any Schwung synth, with no computer and no other
software.

> Model-72 is an unofficial, independent port. It is **not affiliated with or endorsed by Idle Foundry Ltd., Moog
> Music Inc. or Ableton AG.** "Minimoog" is a trademark of Moog Music Inc., used here only to say what CA-72 models.
> The module's name carries no company's name or logo.

## Pages

The root page is the **preset browser**: turn the jog wheel to choose a sound, click to open the pages. Its knobs are
the playable eight: **Cutoff, Emphasis, Contour Amount, Filter Decay, Loudness Decay, Glide, Mod Wheel, Volume**.

| Page | Knobs 1-8 |
|---|---|
| **Oscillators** | Osc 1 Range · Osc 1 Wave · Osc 2 Range · Osc 2 Wave · Osc 2 Freq · Osc 3 Range · Osc 3 Wave · Osc 3 Freq |
| **Mixer** | Osc 1 · Osc 2 · Osc 3 · Noise · Patch (Ext In) · Noise Type · Drive · Bass Comp |
| **Filter** | Cutoff · Emphasis · Contour Amount · Attack · Decay · Sustain · Filter Mode · Filter Mod |
| **Loudness** | Attack · Decay · Sustain · Volume · Level · Auto Gain, and Auto Gain's correction in dB (a readout) |
| **Controller** | Tune · Glide · Glide Switch · Decay Switch · A-440 · Kbd Control 1 · Kbd Control 2 |
| **Modulation** | Mod Mix · Osc Mod · Osc 3 Control · Mod Wheel · Bend Range · AT > Mod |
| **Voice** | Quality (LO / HI) · Mode · Voices · Entropy · Note Priority · Retrigger |
| **Stereo** | Width · Inner · Detune · Played As · Placement |

Every switch on the panel is an enum: Off / On, or its named positions (FILTER MODE LO / HI, NOTE PRIORITY Low / Last /
High, PLAYED AS Scatter / Double ...). Everything is saved with the set.

## The Move panel

It is CA-72's instrument with a few changes for a box with pads and eight knobs:

- **No source switches.** OSC 1, 2, 3 and NOISE play while their volume is above 0. At 0 the module switches the source
  off, exactly as the panel's switch does (it costs nothing then); on the instrument a volume of 0 is already about
  -140 dB, so nothing is lost.
- **PATCH is always wired.** The output is cabled into EXTERNAL INPUT, the classic feedback mod, and the **Patch (Ext In)**
  volume is how much: 0 is unpatched. EXTERNAL INPUT takes no other audio.
- **AT > Mod.** Pad pressure (polyphonic or channel aftertouch) turns the modulation wheel. The higher of the pressure and
  CC 1 wins, and the Mod Wheel knob adds as before. Move has no wheels: pitch bend and CC 1 come in over USB MIDI.
- **Presets.** CA-72's 19 factory sounds and an Init, converted to this panel: a switched-off source comes in at volume
  0, and Patch is 0 unless the sound used PATCH (Feedback Funk does).
- **Dropped:** MAIN OUT (always on).

## Performance and accuracy: the hybrid approach

CA-72's full circuit voice is far too heavy for a small ARM board in real time. CA-72 also has a "light" model (its
Potato mode), whose laws are fitted to the circuit model. Model-72 uses both, as a switch:

- **QUALITY LO**: CA-72's light voice. MONO, POLY (2 to 10 voices) or UNISON, with ENTROPY, stereo, DOUBLE, DRIVE and
  AUTO GAIN. Light enough that a full chord or a ten-voice unison fits.
- **QUALITY HI**: one voice, the **hybrid**. The light oscillators, mixer and contours drive the *circuit's own filter*:
  board 4's transistor ladder, solved in real time, through its real control node, in place of the light filter. In
  measurements it stays within **0.2 to 0.6 dB** of CA-72's full circuit model on most patches (third-octave bands), and
  the filter self-oscillates in tune to within a cent. DOUBLE is off in HI.

The circuit filter is built on a background thread when the module loads (it takes some seconds); LO plays straight
away, and HI plays the light filter until it is ready. HI is far heavier than LO; its cost on Move has yet to be
measured.

## Features

- **The whole instrument**: three oscillators (range, waveform, frequency), the mixer with noise (white or pink), the
  filter and both contours, glide, the modulation wheel and pitch bend, FILTER MODE, keyboard control, NOTE PRIORITY,
  RETRIGGER, A-440, as on CA-72's panel.
- **MONO, POLY or UNISON**, 2 to 10 voices, **ENTROPY** (every voice a little different, as every real instrument is).
- **Stereo**: WIDTH, INNER, DETUNE, SCATTER / DOUBLE, EVEN / CENTER.
- **FILTER DRIVE**, **AUTO GAIN** (with a readout of its correction) and **VOLUME**.
- **Two mods that are not the instrument's:**
  - **BASS COMP**: raises the filter's input with resonance so the bass holds as EMPHASIS rises (at EMPHASIS 8 the bass
    below the cutoff measured about -17 dB with it against -30 dB without). It is not DRIVE, which raises the input by a
    fixed amount whatever the resonance.
  - **PATCH**: the output cabled into EXTERNAL INPUT (above).

Not ported from CA-72: LOCK, MIDI Learn, audio into EXTERNAL INPUT, and AUTO GAIN's per-sound measurement (CA-72's
average curve stands in, as it does there until a sound is measured).

## Installing

Needs Schwung on the Move. Download `model-72-module.tar.gz` from the
[releases](https://github.com/sd88me/schwung-model72/releases) and unpack it into Schwung's sound generators:

    scp model-72-module.tar.gz ableton@move.local:/data/UserData/
    ssh ableton@move.local 'tar -xzf /data/UserData/model-72-module.tar.gz -C /data/UserData/schwung/modules/sound_generators/'
    ssh root@move.local reboot

Then add **Model-72** to a Signal Chain slot. (`scripts/deploy.sh [host] --reboot` does the same from a build.)

Status: builds and passes the host simulation; not yet run on a Move. Reports are welcome.

## Building

Needs Docker (the Rust engine and the aarch64 cross-compiler run in `engine/Dockerfile`'s image):

    scripts/test.sh                       # contract checks + host simulation of the plugin under ASan/UBSan (x86)
    scripts/build.sh                      # aarch64 build -> build/model-72-module.tar.gz
    scripts/deploy.sh [host] [--reboot]   # install on a Move (default move.local); a new dsp.so needs a reboot

| | |
|---|---|
| `engine/ca72/` | CA-72's real-time model, vendored; `src/light.rs` is the only file changed (`engine/VENDORED.md`) |
| `engine/ca72-mpc/` | The engine: voices, MODE, ENTROPY, stereo, AUTO GAIN, the hybrid HI voice; fitting and bench examples. `engine/params.json` and `presets.json` are its parameters and factory sounds |
| `tools/gen_schwung.py` | The pages, names, ranges, the converted presets and `module.json` (generates `src/schwung_meta.h`) |
| `src/schwung_plugin.c` | The Schwung plugin: the engine on a worker thread, the derived switches, AT > Mod, presets |
| `test/host_sim.c` | Drives the plugin the way Schwung's chain host does |
| `vendor/schwung/` | Schwung's plugin API header |

## Licence and credits

**Model-72 is free software under the GNU General Public License, version 3 or later** (`LICENSE`), as CA-72 is. The
complete corresponding source of every release is this repository at its tag.

| Part | Author | Licence |
|---|---|---|
| **CA-72**: the circuit and light models, the sound, the factory presets, the stereo and voice-character code (`engine/ca72/`, `engine/ca72-mpc/src/character.rs`, `stereo.rs`) | Copyright 2026 **Idle Foundry Ltd.** ([ca-72](https://github.com/idlefoundry/ca-72), commit `3f99076`, 0.2.0; stereo placement from [plugin-kit](https://github.com/idlefoundry/plugin-kit) `4c0f0f3`) | GPL-3.0-or-later |
| **The port**: `engine/ca72-mpc` (hybrid HI voice, voices and modes, AUTO GAIN, BASS COMP, PATCH), the changes to `light.rs`, the Schwung module | sd88me, with Claude (Anthropic) | GPL-3.0-or-later |
| **Schwung** plugin API header | Charles Vestal and contributors | MIT |
| `libm` (Rust, statically linked) and Rust's standard library | The Rust project | MIT / Apache-2.0 |

`THIRD-PARTY-NOTICES.txt` has the notices; `VENDORED.md` lists the vendored code. Model-72 contains no Moog Music, Idle
Foundry or Ableton trademarks or logos beyond the attribution above.

If you like this, the thanks belong first to Idle Foundry for CA-72 and for releasing it as free software.
