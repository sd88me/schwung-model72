# Vendored from idlefoundry/ca-72

Upstream: https://github.com/idlefoundry/ca-72 at commit `3f990769036481210c8a0d8e53d480e9661827e7`
(0.2.0, 2026-10-14). Licence: GPL-3.0-or-later (`../LICENSE`, the same as this repository's).

| Here | Upstream | Changes |
|---|---|---|
| `ca72/` | `crates/ca72/` | `Cargo.toml` stands alone (no workspace inheritance); `tests/` left out. `src/light.rs` only, below. |
| `ca72-mpc/src/character.rs` | `crates/ca72-plugin/src/character.rs` | SPREAD's stereo placement (`pair`, `gains`, plugin-kit-stereo) removed: the port is mono. |
| `ca72-mpc/src/stereo.rs` | plugin-kit `crates/plugin-kit-stereo/src/place.rs` @ `4c0f0f3` (github.com/idlefoundry/plugin-kit, GPL-3.0-or-later) | the glide and tests left out. |
| `../vst/presets/factory.toml` | `crates/ca72-plugin/sounds/presets.toml` | none (converted by `tools/presets.py`). |

The skin's pictures (`../skin/assets/`) are copied from upstream's `crates/ca72-panel/assets/worn/`; the skin is drawn by `../skin/make_skin.py`

## `src/light.rs`

- **The circuit's filter spliced into the light voice** (`splice`, `CircuitFilter`; `make_circuit_filter`,
  `take_circuit_filter`, `use_circuit_filter`): QUALITY HI here. The light oscillators, mixer and contours drive board
  4's real-time ladder (`vcf::Vcf` at Potato's 2x) through the circuit's own control node (`ExpoTable`), in place of
  the light ladder; the mixer's hiss (`voice::MIXER_HISS`) is added to the bus so the filter self-oscillates as the
  circuit's does. The constants in `splice` are fitted against the circuit voice at 44.1 kHz by
  `ca72-mpc/examples/fit.rs`; `ca72-mpc/examples/compare.rs` measures the result (third-octave bands within 0.2 to
  0.6 dB of the circuit voice on most patches, the self-oscillation's pitch within a cent).
- **BASS COMP** (`bass_comp`, `bass_amount`): the ladder's input raised by the loop's gain (1 + k) as EMPHASIS
  rises, so the passband's bass holds (the bass below the cutoff about -17 dB at EMPHASIS 8 instead of -30;
  `ca72-mpc/examples/basscomp.rs`). Not the instrument's: this port's own mod.
- **Probes** (`probe_bus`, `probe_y`, `probe_l`, `probe_pitch`, `probe_env`, `bus_gains`) for that fitting.

Nothing else in the crate is changed (`diff -r` against upstream lists `light.rs` alone).
