//! The modulation path (docs/circuit/board3.md): the front panel's MODULATION MIX network,
//! board 3's modulation mix amplifier (circuit No. 15, `board3-modmix.lib`), R57, and the
//! left hand controller's wheels.
//!
//! The amplifier is linear to 0.05 % over +-5 V and flat to 100 kHz in ngspice, so it is its
//! DC transfer: an offset and a gain from each end of the MODULATION MIX pot, solved from the
//! circuit (the nodal solver) for the pot's positions and interpolated. The line after R57
//! divides against the MODULATION wheel and its loads (the oscillators' MOD bus, the filter's
//! R52) as resistors.

use crate::contour::Q2N3392;
use crate::mna::{Circuit, GND, NoConvergence, Node, Part};
use crate::vca::Q2N4058;

/// MODULATION MIX's positions solved (0: oscillator 3 alone, CCW; 1: the noise alone).
const MIX_POINTS: usize = 65;

/// The pitch wheel: its 25K linear pot from +10 V to GND, the wiper 15.3K from GND in the
/// detent (Folkman; dwg 1449), +-2 V over its travel (Figure 9-2: "5V +- 2V"; assumptions
/// A22).
pub fn pitch_wheel_volts(wheel: f64) -> f64 {
    10.0 * 15.3 / 25.0 + 2.0 * wheel.clamp(-1.0, 1.0)
}

/// The pitch wheel's travel each way from its detent, in semitones, as the calibrated
/// voice plays it: its +-2 V through R12 150K against the keyboard's and the control jack's
/// 51.1K, at the calibrated oscillators' 0.984 octaves a volt (5.35 accepts 13 to 17
/// semitones end to end). `tests/voice.rs` measures it and requires this figure.
pub const PITCH_WHEEL_SEMITONES: f64 = 8.07;

/// The MODULATION wheel as drawn: R1402, 50K audio used as a rheostat from the line to GND,
/// 90 degrees of rotation from its counterclockwise stop, "1.2K WHEN FULLY FORWARD" (Figure
/// 9-12): the audio law's first third scaled to 1.2K (assumptions A22), ohm.
pub fn mod_wheel_r_drawn(wheel: f64) -> f64 {
    MOD_WHEEL_FULL_DRAWN * wheel_shape(wheel)
}

/// The drawing's wheel fully forward, and the hardware reference's: the resistance at which
/// oscillator 1 swings 11.9 semitones peak to peak under oscillator 3's triangle (on LO,
/// MODULATION MIX at oscillator 3), as the reference's does with its MOD DEPTH at 10 and
/// MIDI's modulation wheel at 127 (docs/calibration, session J), ohm.
pub const MOD_WHEEL_FULL_DRAWN: f64 = 1.2e3;
pub const MOD_WHEEL_FULL: f64 = 685.0;

/// The wheel's law as a share of its resistance fully forward: the generic audio taper's
/// first third (A22).
fn wheel_shape(wheel: f64) -> f64 {
    let taper = |p: f64| (crate::ulp::pow(81.0, p) - 1.0) / 80.0;
    taper(wheel.clamp(0.0, 1.0) / 3.0) / taper(1.0 / 3.0)
}

/// The MODULATION wheel's resistance at a quarter, half, three quarters and fully forward:
/// where oscillator 1 swings under oscillator 3's triangle (on LO, MODULATION MIX at
/// oscillator 3) as the hardware reference's does with its MOD DEPTH knob at 2.5, 5, 7.5 and
/// 10 and no MIDI wheel, 0.069, 0.214 and 0.636 of the swing at 10 (docs/calibration,
/// session L), the swing at 10 its MIDI wheel's at 127 ([`MOD_WHEEL_FULL`]). The drawing's
/// law (A22) gives 91, 222 and 412 ohm (of 685).
const MOD_WHEEL_LAW: [(f64, f64); 4] = [
    (0.25, 29.5),
    (0.5, 95.1),
    (0.75, 349.0),
    (1.0, MOD_WHEEL_FULL),
];

/// A law through `points` (position, ohm): below the first the drawing's wheel's shape
/// scaled to meet it, then straight in the resistance's logarithm.
fn through(points: &[(f64, f64)], x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    let (x0, r0) = points[0];
    if x <= x0 {
        return r0 * wheel_shape(x) / wheel_shape(x0);
    }
    for k in points.windows(2) {
        let ((xa, ra), (xb, rb)) = (k[0], k[1]);
        if x <= xb {
            return ra * crate::ulp::pow(rb / ra, (x - xa) / (xb - xa));
        }
    }
    points[points.len() - 1].1
}

/// [`through`] inverted: the position at which the law is `r` ohm.
fn position(points: &[(f64, f64)], r: f64) -> f64 {
    let (x0, r0) = points[0];
    if r <= r0 {
        // The wheel's shape, the audio taper's first third, inverted.
        let shape = (r.max(0.0) / r0) * wheel_shape(x0);
        let x = 3.0 * crate::ulp::log(1.0 + shape * (crate::ulp::pow(81.0, 1.0 / 3.0) - 1.0))
            / crate::ulp::log(81.0);
        return x.clamp(0.0, x0);
    }
    for k in points.windows(2) {
        let ((xa, ra), (xb, rb)) = (k[0], k[1]);
        if r <= rb {
            return xa + (xb - xa) * crate::ulp::log(r / ra) / crate::ulp::log(rb / ra);
        }
    }
    1.0
}

/// The MODULATION wheel as the voice has it: the hardware reference's MOD DEPTH knob's law
/// ([`MOD_WHEEL_LAW`]), ohm.
pub fn mod_wheel_r(wheel: f64) -> f64 {
    through(&MOD_WHEEL_LAW, wheel)
}

/// The resistance MIDI's modulation wheel puts on the line at 32, 64, 96 and 127 on the
/// hardware reference (oscillator 1 swinging 1.29, 2.62, 6.05 and 11.9 semitones;
/// docs/calibration, session J): its MIDI implementation's curve ("soft" by default, its
/// manual), not the knob's law. A MIDI wheel's message takes the depth from the knob until
/// the knob is moved (session L).
const MIDI_WHEEL_R: [(f64, f64); 4] = [
    (32.0 / 127.0, 46.3),
    (64.0 / 127.0, 97.8),
    (96.0 / 127.0, 261.0),
    (1.0, MOD_WHEEL_FULL),
];

/// Where MIDI's modulation wheel (0..1, control change 1's value over 127) puts the
/// MODULATION wheel: the position whose resistance is the reference's for it, straight in
/// the resistance's logarithm between [`MIDI_WHEEL_R`] (below 32 the wheel's shape, scaled
/// to meet it).
pub fn midi_wheel(cc: f64) -> f64 {
    let cc = cc.clamp(0.0, 1.0);
    if cc == 1.0 {
        return 1.0;
    }
    position(&MOD_WHEEL_LAW, through(&MIDI_WHEEL_R, cc))
}

/// The amplifier's DC transfer at one MODULATION MIX position: its output (before R57) is
/// `offset + gain_noise * noise + gain_osc3 * osc3` behind `r_out`, the sources' voltages as
/// they reach R24 and R23.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MixTransfer {
    pub offset: f64,
    pub gain_noise: f64,
    pub gain_osc3: f64,
    /// Its output resistance (the closed loop's), ohm.
    pub r_out: f64,
}

const P10: Node = 1;
const N10: Node = 2;
const NOISE: Node = 3;
const OSC3: Node = 4;
const HELD: usize = 5;
const A: Node = 5;
const B: Node = 6;
const SUM: Node = 7;
const NE: Node = 8;
const N17: Node = 9;
const N38: Node = 10;
const AMP: Node = 11;
const NODES: usize = 12;

/// The amplifier with the MODULATION MIX network at `mix`, sources held.
fn circuit(mix: f64) -> Circuit {
    let m = mix.clamp(0.0, 1.0);
    let mut c = Circuit::new(NODES, HELD);
    c.set(P10, 10.0);
    c.set(N10, -10.0);
    for (n, v) in [(SUM, 0.0), (NE, -0.6), (N17, 9.3), (N38, 9.3), (AMP, 0.0)] {
        c.set(n, v);
    }
    let r = |c: &mut Circuit, a, b, r: f64| {
        c.add(Part::Resistor { a, b, r });
    };
    // R24, R23 24K; R3 25K linear, its wiper at GND (CCW: the wiper at the noise's end).
    r(&mut c, NOISE, A, 24e3);
    r(&mut c, OSC3, B, 24e3);
    r(&mut c, A, GND, (25e3 * m).max(1e-3));
    r(&mut c, B, GND, (25e3 * (1.0 - m)).max(1e-3));
    // The amplifier (circuit No. 15).
    r(&mut c, A, SUM, 43e3);
    r(&mut c, B, SUM, 43e3);
    r(&mut c, AMP, SUM, 91e3);
    c.add_bjt(P10, SUM, NE, &Q2N3392, 25.0, false);
    c.add_bjt(N17, GND, NE, &Q2N3392, 25.0, false);
    r(&mut c, NE, N10, 43e3);
    r(&mut c, P10, N17, 6.2e3);
    c.add_bjt(AMP, N17, P10, &Q2N4058, 25.0, true);
    r(&mut c, AMP, N10, 10e3);
    r(&mut c, N17, N38, 390.0);
    c.add(Part::Capacitor {
        a: N38,
        b: GND,
        c: 0.022e-6,
    });
    c
}

/// The amplifier's DC transfer at a MODULATION MIX position: three operating points.
pub fn transfer(mix: f64) -> Result<MixTransfer, NoConvergence> {
    let mut c = circuit(mix);
    let mut at = |noise: f64, osc3: f64| -> Result<f64, NoConvergence> {
        c.set(NOISE, noise);
        c.set(OSC3, osc3);
        c.dc()?;
        Ok(c.v(AMP))
    };
    let offset = at(0.0, 0.0)?;
    let n = at(1.0, 0.0)? - offset;
    let o = at(0.0, 1.0)? - offset;
    // The output resistance while sourcing: the output near +2 V, then loaded with 2.2K
    // to GND (about 1 mA, as the wheel's line draws).
    let drive = 2.0 / if o.abs() > n.abs() { o } else { n };
    let (dn, doo) = if o.abs() > n.abs() {
        (0.0, drive)
    } else {
        (drive, 0.0)
    };
    let v0 = at(dn, doo)?;
    let mut loaded = c.clone();
    loaded.add(Part::Resistor {
        a: AMP,
        b: GND,
        r: 2.2e3,
    });
    loaded.dc()?;
    let v1 = loaded.v(AMP);
    let r_out = (v0 - v1) / (v1 / 2.2e3);
    Ok(MixTransfer {
        offset,
        gain_noise: n,
        gain_osc3: o,
        r_out,
    })
}

/// The path's loads on the line after R57, as a Thevenin source each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineLoads {
    /// The oscillators' MOD bus with OSC. 3 MODULATION on (three 51.1K inputs to their
    /// summing junctions at -5 V against R163 33K from +10 V), or none.
    pub osc_mod: bool,
    /// R52 33K to the filter's control node (its voltage), with FILTER MODULATION on.
    pub filter_mod: bool,
    pub filter_node: f64,
}

/// The modulation path.
#[derive(Debug, Clone)]
pub struct Modulation {
    table: Vec<MixTransfer>,
}

impl Modulation {
    /// Solves the amplifier over MODULATION MIX's travel.
    pub fn new() -> Result<Modulation, NoConvergence> {
        let table = (0..MIX_POINTS)
            .map(|k| transfer(k as f64 / (MIX_POINTS - 1) as f64))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Modulation { table })
    }

    /// The amplifier's transfer at a MODULATION MIX position (interpolated).
    pub fn mix(&self, mix: f64) -> MixTransfer {
        let x = mix.clamp(0.0, 1.0) * (MIX_POINTS - 1) as f64;
        let i = (x.floor() as usize).min(MIX_POINTS - 2);
        let f = x - i as f64;
        let (a, b) = (self.table[i], self.table[i + 1]);
        let l = |p: f64, q: f64| p + (q - p) * f;
        MixTransfer {
            offset: l(a.offset, b.offset),
            gain_noise: l(a.gain_noise, b.gain_noise),
            gain_osc3: l(a.gain_osc3, b.gain_osc3),
            r_out: l(a.r_out, b.r_out),
        }
    }

    /// The load MODULATION MIX's ends put on the sources through R24 and R23, ohm: each end's
    /// track to GND against R62 or R64 43K into the amplifier's summing junction (at about
    /// GND): (noise's side, oscillator 3's side).
    pub fn source_loads(mix: f64) -> (f64, f64) {
        let m = mix.clamp(0.0, 1.0);
        let par = |a: f64, b: f64| if a <= 0.0 { 0.0 } else { a * b / (a + b) };
        (
            24e3 + par(25e3 * m, 43e3),
            24e3 + par(25e3 * (1.0 - m), 43e3),
        )
    }

    /// The line's voltage (to the MOD bus and R52): the amplifier's output behind its output
    /// resistance and R57 1K, against the MODULATION wheel's resistance and the loads. The
    /// output stage only sources (Q7, a PNP, pulls up; R30 10K to -10 V pulls down): where
    /// the line would need Q7 to sink, Q7 is off and R30 alone drives it.
    pub fn line(amp: f64, r_out: f64, wheel_r: f64, loads: &LineLoads) -> f64 {
        // The line's own loads as a Norton source: conductance and current.
        let mut g_l = 1.0 / wheel_r.max(0.1);
        let mut i_l = 0.0;
        if loads.osc_mod {
            g_l += 3.0 / 51.1e3 + 1.0 / 33e3;
            i_l += -5.0 * 3.0 / 51.1e3 + 10.0 / 33e3;
        }
        if loads.filter_mod {
            g_l += 1.0 / 33e3;
            i_l += loads.filter_node / 33e3;
        }
        let r_src = 1e3 + r_out;
        let line = (amp / r_src + i_l) / (1.0 / r_src + g_l);
        // Q7's current: R30's and the line's, from the amplifier's output.
        let q7 = (amp + 10.0) / 10e3 + (amp - line) / r_src;
        if q7 >= 0.0 {
            return line;
        }
        // Q7 off: the output node between R30 and R57's line.
        let g_57 = 1.0 / 1e3;
        let (g_30, i_30) = (1.0 / 10e3, -10.0 / 10e3);
        // Two nodes: out (R30, R57) and the line (R57, loads).
        let det = (g_30 + g_57) * (g_57 + g_l) - g_57 * g_57;
        (g_57 * i_30 + (g_30 + g_57) * i_l) / det
    }
}
