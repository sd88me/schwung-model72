//! Board 2's dual contour generator (circuit No. 8) in real time, derived from its circuit
//! (docs/circuit/board2.md, `circuits/boards/board2-contour.lib`). Figure 9-7's
//! reference designators; the loudness section's in brackets where they differ.
//!
//! - **Trigger**: a key joins the trigger bus to +10 V; the bus charges C7 through the
//!   germanium CR10 and R60, and C7 drives Q20's base through R55. At rest CR10's leakage
//!   holds C7 near -10 V, so C7 must charge past Q20's threshold first (a few
//!   milliseconds); after the key is released Q20 stays on until C7 has drained into its
//!   base (about 12 ms). Q20 pulls down the reset line (R37); Q12 (through R19) inverts it
//!   into V-trig, whose rise sets both flip-flops through C1 [C4].
//! - **Flip-flop** Q1/Q4 [Q25/Q15]: set, Q5 [Q16] charges the timing capacitor C5 [C2]
//!   through R7 [R42] and the ATTACK pot toward +9.3 V; the flip-flop resets when the
//!   output, divided by R33/R29 [R24/R27] toward -10 V, drives enough current through CR3
//!   [CR6] into Q4's [Q15's] base to take Q1 [Q25] out of saturation, or while the reset
//!   line drives it through CR1 [CR8].
//! - **Decay**: reset, Q7 [Q18] (saturated) lets the capacitor discharge through the DECAY
//!   pot into the sustain node, held by the PNP follower Q8 [Q19] at the SUSTAIN divider's
//!   voltage; on release V-trig pulls the node down through CR2 [CR9] (DECAY on: the final
//!   decay at the DECAY time) and, with DECAY off, dumps the capacitor through CR7 [CR4]
//!   and R1401 1.5K: each capacitor through one of its own, as the hardware reference
//!   does (`ContourCircuit::dump_each`), or both through the drawing's one.
//! - **Output**: Q22 and Q21, a complementary follower [Q3 and Q2, a Darlington], whose
//!   drop follows from its transistors at the load's current.
//!
//! The flip-flops are latches (their transitions take microseconds); everything else is
//! the circuit's equations, solved every sample. The +9.3 V rail is ideal at its resting
//! value (its decoupling is not modelled; docs/circuit/assumptions.md).

use crate::devices::{Bjt, BjtAt, Diode, DiodeAt, series_junction_from};
use crate::prof::Part;
use crate::vcf::TIS93;

/// 2N3392 (`Q2N3392`) as in `mm-devices.lib`.
pub const Q2N3392: Bjt = Bjt {
    is: 5e-15,
    bf: 212.0,
    ise: 1e-14,
    ne: 1.5,
    vaf: 100.0,
    ikf: 0.1,
    br: 4.0,
    rb: 10.0,
    re: 0.5,
    rc: 1.0,
    cje: 4.5e-12,
    vje: 0.75,
    mje: 0.33,
    cjc: 3.5e-12,
    vjc: 0.75,
    mjc: 0.33,
    tf: 0.4e-9,
    xti: 3.0,
    xtb: 1.5,
    eg: 1.11,
    tnom: 25.0,
};

/// Q12, V-trig's transistor, as the hardware reference has it: the 2N3392's model with the
/// current gain at which both contours, released with DECAY at its end, fall as the
/// reference's (`Q12HG` in `mm-devices.lib`; docs/calibration, session N; board2.md B2-14):
/// a modern high-gain part's, where the drawing's 2N3392 bin is 150 to 300.
pub const Q12HG: Bjt = Bjt {
    bf: 650.0,
    ..Q2N3392
};

/// 1N34A (`D1N34A`) and 1N4004 (`D1N4004`) as in `mm-devices.lib`.
pub const D1N34A: Diode = Diode {
    is: 1e-6,
    n: 1.5,
    rs: 20.0,
    eg: 0.67,
    xti: 3.0,
};
/// The peak detectors' CR3 and CR6 as the hardware reference has them: silicon small-signal
/// diodes (`DCR36` in `mm-devices.lib`), not the drawing's 1N34A germanium. Their saturation
/// current is the one at which both flip-flops reset where the reference's contours peak
/// (docs/calibration, session L; board2.md B2-11); the rest is the library's generic
/// small-signal silicon diode's (`DSG3246`).
pub const DCR36: Diode = Diode {
    is: 9.45e-9,
    n: 1.75,
    rs: 0.6,
    eg: 1.11,
    xti: 3.0,
};
pub const D1N4004: Diode = Diode {
    is: 7e-9,
    n: 1.9,
    rs: 0.04,
    eg: 1.11,
    xti: 3.0,
};

/// A section's output follower.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Follower {
    /// Q22 (NPN) and Q21 (PNP, emitter through R50 to +9.3 V): one base-emitter drop.
    Complementary,
    /// Q3 and Q2 (NPN Darlington): two drops.
    Darlington,
}

/// One contour section's constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Section {
    /// R7 [R42] in series with the ATTACK pot; the timing capacitor C5 [C2], and its
    /// dielectric absorption as branches (capacitance F, resistance ohm) across it
    /// ([`C_ABSORPTION`]; a capacitance of 0 leaves a branch out: the drawing's ideal part).
    pub r7: f64,
    pub c: f64,
    pub absorption: [(f64, f64); 3],
    /// The timing capacitor's series resistance (ohm): its terminal, which the follower
    /// and the peak detector read, sits the current through it times this off its charge.
    pub esr: f64,
    /// R48 [R16] into the follower; the follower.
    pub r_in: f64,
    pub follower: Follower,
    /// The sustain node: R10 [R45] from +9.3 V; the divider at Q8's [Q19's] base: R13
    /// [R57] from the SUSTAIN wiper, R17 to -10 V (infinite in the loudness section), R1
    /// [4.7K] to ground.
    pub r10: f64,
    pub r_sus: f64,
    pub r_neg: f64,
    pub r_gnd: f64,
    /// The peak divider: R33 [R24] from the output, R29 [R27] to -10 V.
    pub r_top: f64,
    pub r_bot: f64,
    /// The flip-flop: R3 [R38] (Q4's collector to Q1's base), R4 [R39] (Q4's collector
    /// load), R14 [R56] (Q1's collector to Q4's base), R5 [R40] and R6 [R41] (Q1's
    /// collector load, through Q5's base node); R35 [R36] from the reset line.
    pub r3: f64,
    pub r4: f64,
    pub r14: f64,
    pub r5: f64,
    pub r6: f64,
    pub r_reset: f64,
    /// Q6 [Q17]: R8 [R43] to -10 V, R9 [R44] into Q7's [Q18's] base.
    pub r8: f64,
    pub r9: f64,
}

/// The timing capacitors' dielectric absorption (C5 and C2, 10 uF electrolytics): three
/// branches of a capacitance through a resistance across each, fitted to the hardware
/// reference's contours after a fast and a slower attack (docs/calibration, session M;
/// board2.md B2-12): its output falls 0.21 V within a millisecond of a fast attack's peak and
/// 0.33 V within 10 ms, as the absorbed charge catches up, and hardly at all after a slow one.
/// Over a long time the capacitor holds 13.26 uF.
pub const C_ABSORPTION: [(f64, f64); 3] =
    [(0.769e-6, 779.0), (0.202e-6, 28.26e3), (0.223e-6, 237.6e3)];

/// The timing capacitors' series resistance, ohm (with [`C_ABSORPTION`] and [`R7`]).
pub const C_ESR: f64 = 1.53;

/// R7 [R42], the attack's resistor: 100 on Figure 9-7, 86.6 (E96) where the hardware
/// reference's fastest attacks rise with its capacitors as they are (docs/calibration,
/// session M; board2.md B2-12).
pub const R7: f64 = 86.6;

pub const FILTER_SECTION: Section = Section {
    r7: R7,
    c: 10e-6,
    absorption: C_ABSORPTION,
    esr: C_ESR,
    r_in: 10e3,
    follower: Follower::Complementary,
    r10: 27e3,
    r_sus: 4.7e3,
    r_neg: 150e3,
    r_gnd: 3e3,
    r_top: 5.6e3,
    r_bot: 16.9e3,
    r3: 10e3,
    r4: 10e3,
    r14: 33e3,
    r5: 3.3e3,
    r6: 560.0,
    r_reset: 10e3,
    r8: 10e3,
    r9: 4.7e3,
};

pub const LOUDNESS_SECTION: Section = Section {
    r7: R7,
    c: 10e-6,
    absorption: C_ABSORPTION,
    esr: C_ESR,
    r_in: 10e3,
    follower: Follower::Darlington,
    r10: 27e3,
    r_sus: 4.7e3,
    r_neg: f64::INFINITY,
    r_gnd: 4.7e3,
    r_top: 6.8e3,
    r_bot: 16.9e3,
    r3: 10e3,
    r4: 10e3,
    r14: 33e3,
    r5: 3.3e3,
    r6: 560.0,
    r_reset: 10e3,
    r8: 10e3,
    r9: 4.7e3,
};

/// The circuit's constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourCircuit {
    /// The decoupled rail (Q26's emitter), V.
    pub p93: f64,
    pub vp: f64,
    pub vn: f64,
    /// Trigger section: R60, C7, R55, R37, R19, R23, R32, R49.
    pub r60: f64,
    pub c7: f64,
    pub r55: f64,
    pub r37: f64,
    pub r19: f64,
    pub r23: f64,
    pub r32: f64,
    pub r49: f64,
    /// The trigger bus's load on the keyboard circuit: R65, R34 and C13 to -10 V.
    pub r65: f64,
    pub r34: f64,
    pub c13: f64,
    /// The left hand controller's R1401 (DECAY off).
    pub r1401: f64,
    /// With DECAY off, each capacitor dumped through an R1401 of its own (the hardware
    /// reference's, whose two DECAY switches release one contour as fast whatever the
    /// other holds: docs/calibration), rather than both through the left hand
    /// controller's one (Figure 9-12, `false`), where a contour held higher slows the
    /// other's release.
    pub dump_each: bool,
    pub filter: Section,
    pub loudness: Section,
    pub npn: Bjt,
    pub pnp: Bjt,
    /// Q12, V-trig's transistor: [`Q12HG`] as the hardware reference has it, or
    /// [`Q2N3392`] as drawn.
    pub q12: Bjt,
    pub ge: Diode,
    pub si: Diode,
    /// CR3 and CR6, the peak detectors' diodes: [`DCR36`] as the hardware reference has
    /// them, or [`D1N34A`] as drawn.
    pub peak_diode: Diode,
}

impl Default for ContourCircuit {
    fn default() -> Self {
        ContourCircuit {
            p93: 9.307,
            vp: 10.0,
            vn: -10.0,
            r60: 100e3,
            c7: 0.1e-6,
            r55: 47e3,
            r37: 10e3,
            r19: 10e3,
            r23: 3.3e3,
            r32: 10e3,
            r49: 220.0,
            r65: 5.1e3,
            r34: 100e3,
            c13: 0.01e-6,
            r1401: 1.5e3,
            dump_each: true,
            filter: FILTER_SECTION,
            loudness: LOUDNESS_SECTION,
            npn: Q2N3392,
            pnp: TIS93,
            q12: Q12HG,
            ge: D1N34A,
            si: D1N4004,
            peak_diode: DCR36,
        }
    }
}

/// One section's front panel controls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Controls {
    /// The ATTACK and DECAY pots' resistance in circuit, ohm (1M audio rheostats).
    pub attack: f64,
    pub decay: f64,
    /// SUSTAIN (5K linear): the wiper's position from its grounded end, 0..1.
    pub sustain: f64,
}

/// The front panel and left hand controller settings the contours depend on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub filter: Controls,
    pub loudness: Controls,
    /// The left hand controller's DECAY switch.
    pub decay_on: bool,
    /// EXT. S-TRIG shorted to ground.
    pub s_trig: bool,
}

/// What loads each output (to ground, and the peak divider to -10 V is the circuit's):
/// conductance, S.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loads {
    pub filter: f64,
    pub loudness: f64,
}

impl Default for Loads {
    fn default() -> Self {
        // AMOUNT OF CONTOUR (5K) on the filter contour; the VCA's R59 and Q18 (about 2M
        // while Q18 conducts) on the loudness contour.
        Loads {
            filter: 1.0 / 5e3,
            loudness: 1.0 / 2e6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct SectionState {
    /// The timing capacitor's voltage.
    v: f64,
    /// The flip-flop: set (attack) or reset.
    set: bool,
    /// The output and the sustain node.
    out: f64,
    e7: f64,
    /// Warm starts: Q7's and Q8's junctions (through R9, and the follower Q8).
    j7: f64,
    j8: f64,
    /// The last sample charged the capacitor (Q7 off): the decay's path was open.
    attacked: bool,
    /// The absorption branches' capacitors' voltages.
    da: [f64; 3],
}

impl SectionState {
    /// The absorption branches over a step of `h` (trapezoidal) from the capacitor at `v`:
    /// their current at `v`, and the companion (conductance, current) by which their current
    /// at the step's end is `g v' - b`.
    fn absorption(&self, s: &Section, h: f64) -> (f64, f64, f64) {
        let (mut i, mut g, mut b) = (0.0, 0.0, 0.0);
        for (&(ck, rk), &vk) in s.absorption.iter().zip(&self.da) {
            if ck <= 0.0 {
                continue;
            }
            let a = 0.5 * h / (rk * ck);
            i += (self.v - vk) / rk;
            g += 1.0 / ((1.0 + a) * rk);
            b += (vk * (1.0 - a) + a * self.v) / ((1.0 + a) * rk);
        }
        (i, g, b)
    }

    /// The branches moved on a step from the capacitor at `v_old` to its present voltage.
    fn absorb(&mut self, s: &Section, h: f64, v_old: f64) {
        for (&(ck, rk), vk) in s.absorption.iter().zip(self.da.iter_mut()) {
            if ck <= 0.0 {
                *vk = self.v;
                continue;
            }
            let a = 0.5 * h / (rk * ck);
            *vk = (*vk * (1.0 - a) + a * (v_old + self.v)) / (1.0 + a);
        }
    }

    /// The branches as charged as the capacitor (within [`SETTLED`]).
    fn absorbed(&self, s: &Section) -> bool {
        s.absorption
            .iter()
            .zip(&self.da)
            .all(|(&(ck, _), &vk)| ck <= 0.0 || (self.v - vk).abs() < SETTLED)
    }
}

/// What a section's behaviour needs from the settings, computed when they change.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct SectionSetup {
    /// Q8's [Q19's] base divider's Thevenin voltage and its resistance seen through Q8's
    /// base (with RE): the follower's series resistance.
    v_th8: f64,
    r_e8: f64,
    /// Q6's [Q17's] emitter with the flip-flop reset.
    v_e6: f64,
    /// The output at which the flip-flop resets.
    peak: f64,
}

/// The dual contour generator in real time.
#[derive(Debug, Clone)]
pub struct Contours {
    pub circuit: ContourCircuit,
    pub celsius: f64,
    pub loads: Loads,
    h: f64,
    /// The solvers' tolerances ([`Contours::set_quality`]): the root finders' and the
    /// transistors' last step, and the decay's Newton step, V.
    tol: f64,
    tol_decay: f64,
    /// States: C13's node (the trigger bus through R65), C7, the two sections.
    t2: f64,
    v7: f64,
    sections: [SectionState; 2],
    /// Algebraic nodes kept for the next sample: the reset line, V-trig; warm starts for
    /// CR10's junction.
    rst: f64,
    vtrig: f64,
    j10: f64,
    /// Warm starts for the reset feeds' junctions (CR1, CR8 into their bases) and Q12's
    /// base as the reset line sees it (performance: solved cold, each took tens of
    /// bisections).
    j_feed: [f64; 2],
    j12b: f64,
    /// Q20's and Q12's junction voltages (base-emitter, base-collector).
    x20: [f64; 2],
    x12: [f64; 2],
    setup: Option<(Panel, Loads, f64, [SectionSetup; 2])>,
    /// Parts at rest are not solved again (performance): the last tick's inputs; the
    /// trigger section's C7 and reset line, and each section in decay or sustain, once they
    /// move less than [`SETTLED`] a tick with the inputs unchanged. Any change of the key,
    /// the panel, the loads, the temperature or V-trig wakes them.
    inputs: Option<(bool, Panel, Loads, u64)>,
    trig_settled: bool,
    sec_settled: [bool; 2],
    /// In High Fidelity and Potato (`lean`): the trigger section woken only by what it
    /// reads (the key, EXT. S-TRIG, the temperature), not by every panel change; Q20 and
    /// Q12 solved from their last solve within the tick (in every mode their steps are
    /// limited as the nodal solver's: until 2026-09-30 No Compromises held them to 0.2 V
    /// down and 2 Vt up, and they stopped short hundreds of times a few seconds); the dump
    /// node from its last solution with its slope analytic.
    lean: bool,
    trig_inputs: Option<(bool, bool, u64)>,
    /// The lean modes' warm starts for the dump nodes, filter and loudness (the same node
    /// when they share R1401; NaN: none).
    dump: [f64; 2],
    /// The transistors' parameters at the temperature, and what they were worked out for
    /// ([`Contours::devices`]).
    devices: std::cell::Cell<Option<DevicesAt>>,
}

/// [`Contours::devices`]' cache: the temperature's bits, the models (NPN, PNP, germanium
/// and silicon diodes), and their parameters at it.
type DevicesAt = (u64, [Bjt; 3], [Diode; 2], [BjtAt; 3], [DiodeAt; 2]);

/// A part moving less than this a tick (V) is settled: what it still would move is at most
/// this times its time constant in ticks (under 25 nV for a 10 s decay at 24 kHz).
pub const SETTLED: f64 = 1e-13;

/// A V-trig that falls less than this in a tick is solved once with the sections, and in
/// High Fidelity and Potato the passes' tolerance on it, V (140 dB under a contour's swing).
const COUPLED: f64 = 1e-6;

/// A capacitor's current through DECAY below which V-trig and the sections are solved once
/// (it holds V-trig up only with DECAY near its end: through 10K a capacitor draws under half
/// a milliampere, and the one pass was within ngspice's budgets there; CR2 [CR9] carries
/// Q7's base current and R10's, 1.6 mA, in every release), A.
const COUPLING_CURRENT: f64 = 0.5e-3;

/// The most passes of V-trig and the sections solved together in a tick (the factory
/// presets' chords need at most 9).
const COUPLING_PASSES: usize = 40;

/// The two outputs, V.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ContourOut {
    pub filter: f64,
    pub loudness: f64,
    /// V-trig (Q12's collector): high while triggered.
    pub vtrig: f64,
}

/// A monotone scalar root between `lo` and `hi` (f(lo) and f(hi) of opposite signs):
/// Newton steps from `x` kept inside the bracket, bisection otherwise.
fn root(f: impl Fn(f64) -> f64, lo: f64, hi: f64, x: f64, tol: f64) -> f64 {
    let rising = f(lo) < 0.0;
    root_dir(f, lo, hi, x, rising, tol)
}

/// [`root`] for a function whose direction is known (`rising`: negative at `lo`): the same
/// steps without evaluating it at `lo`, which for the trigger's nodes means a transistor
/// solved far from its warm start (performance).
/// [`root_dir`] for a function that gives its slope with its value: the same bracket and
/// steps, one evaluation a step instead of two (performance).
fn root_slope(
    f: impl Fn(f64) -> (f64, f64),
    mut lo: f64,
    mut hi: f64,
    x: f64,
    rising: bool,
    tol: f64,
) -> f64 {
    let mut x = x.clamp(lo.min(hi), lo.max(hi));
    for _ in 0..80 {
        let (fx, slope) = f(x);
        if (fx < 0.0) == rising {
            lo = x;
        } else {
            hi = x;
        }
        let next = if slope != 0.0 {
            x - fx / slope
        } else {
            f64::NAN
        };
        if (next - x).abs() < tol && next >= lo.min(hi) - 1e-9 && next <= lo.max(hi) + 1e-9 {
            return next;
        }
        let nx = if next > lo.min(hi) && next < lo.max(hi) {
            next
        } else {
            0.5 * (lo + hi)
        };
        if (hi - lo).abs() < tol {
            return nx;
        }
        x = nx;
    }
    crate::unconverged::note(crate::unconverged::Solver::ContourRoot);
    x
}

fn root_dir(
    f: impl Fn(f64) -> f64,
    mut lo: f64,
    mut hi: f64,
    x: f64,
    rising: bool,
    tol: f64,
) -> f64 {
    let mut x = x.clamp(lo.min(hi), lo.max(hi));
    for _ in 0..80 {
        let fx = f(x);
        if (fx < 0.0) == rising {
            lo = x;
        } else {
            hi = x;
        }
        let d = 1e-7 * (1.0 + x.abs());
        let slope = (f(x + d) - fx) / d;
        let next = if slope != 0.0 {
            x - fx / slope
        } else {
            f64::NAN
        };
        if (next - x).abs() < tol && next >= lo.min(hi) - 1e-9 && next <= lo.max(hi) + 1e-9 {
            return next;
        }
        let nx = if next > lo.min(hi) && next < lo.max(hi) {
            next
        } else {
            0.5 * (lo + hi)
        };
        if (hi - lo).abs() < tol {
            return nx;
        }
        x = nx;
    }
    crate::unconverged::note(crate::unconverged::Solver::ContourRoot);
    x
}

/// An NPN with its emitter grounded, its base through `r` from `vb` and its collector at
/// `vc` (Q20, Q12): [`solve_junctions_limited`]'s Newton steps and limits with the
/// Jacobian analytic ([`BjtAt::currents_d`]) instead of by differences (performance).
/// Returns the junction voltages and the collector and base currents.
fn grounded_npn(
    m: &BjtAt,
    q: &Bjt,
    x: [f64; 2],
    vb: f64,
    r: f64,
    vc: f64,
    tol: f64,
) -> ([f64; 2], f64, f64) {
    grounded_npn_with(m, q, x, (vb, r, vc), tol, true)
}

/// [`grounded_npn`]; with `pnjlim` (High Fidelity, Potato) a junction's step limited only
/// as it rises past its critical voltage (SPICE's limiting, as the nodal solver's), not
/// held to 0.2 V down and 2 Vt up: a collector junction swinging volts took some fifty
/// steps.
fn grounded_npn_with(
    m: &BjtAt,
    q: &Bjt,
    mut x: [f64; 2],
    (vb, r, vc): (f64, f64, f64),
    tol: f64,
    pnjlim: bool,
) -> ([f64; 2], f64, f64) {
    let crit = if pnjlim {
        crate::mna::vcrit(m.vt, m.is)
    } else {
        0.0
    };
    let rb = r + q.rb;
    let mut converged = false;
    for _ in 0..60 {
        let d = m.currents_d(x[0], x[1], q.vaf);
        let vbi = vb - rb * d.ib;
        let res = [
            vbi - q.re * (d.ic + d.ib) - x[0],
            vbi - (vc - q.rc * d.ic) - x[1],
        ];
        let j = [
            [
                -rb * d.dib[0] - q.re * (d.dic[0] + d.dib[0]) - 1.0,
                -rb * d.dib[1] - q.re * (d.dic[1] + d.dib[1]),
            ],
            [
                -rb * d.dib[0] + q.rc * d.dic[0],
                -rb * d.dib[1] + q.rc * d.dic[1] - 1.0,
            ],
        ];
        let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
        let dx = [
            (res[0] * j[1][1] - res[1] * j[0][1]) / det,
            (j[0][0] * res[1] - j[1][0] * res[0]) / det,
        ];
        let step = if pnjlim {
            let to = |k: usize| crate::mna::pnjlim(x[k] - dx[k], x[k], m.vt, crit) - x[k];
            [to(0), to(1)]
        } else {
            let lim = |d: f64| d.clamp(-0.2, 2.0 * m.vt);
            [lim(-dx[0]), lim(-dx[1])]
        };
        x = [x[0] + step[0], x[1] + step[1]];
        if step[0].abs() < tol && step[1].abs() < tol {
            converged = true;
            break;
        }
    }
    if !converged {
        crate::unconverged::note(crate::unconverged::Solver::ContourTransistor);
    }
    let (ic, ib) = m.currents(x[0], x[1], q.vaf);
    (x, ic, ib)
}

/// The currents' slopes against the base's source voltage and the collector voltage at a
/// [`grounded_npn`] solution `x`: the junctions move by -J^-1 times the residual's slope
/// against each (performance: for the trigger's roots). Returns
/// ([dIc/dVb, dIc/dVc], [dIb/dVb, dIb/dVc]).
fn grounded_npn_slopes(m: &BjtAt, q: &Bjt, x: [f64; 2], r: f64) -> ([f64; 2], [f64; 2]) {
    let rb = r + q.rb;
    let d = m.currents_d(x[0], x[1], q.vaf);
    let j00 = -rb * d.dib[0] - q.re * (d.dic[0] + d.dib[0]) - 1.0;
    let j01 = -rb * d.dib[1] - q.re * (d.dic[1] + d.dib[1]);
    let j10 = -rb * d.dib[0] + q.rc * d.dic[0];
    let j11 = -rb * d.dib[1] + q.rc * d.dic[1] - 1.0;
    let det = j00 * j11 - j01 * j10;
    // The residual moves by [1, 1] a volt of Vb and by [0, -1] a volt of Vc.
    let x_vb = [-(j11 - j01) / det, -(j00 - j10) / det];
    let x_vc = [-j01 / det, j00 / det];
    let along = |g: [f64; 2], dx: [f64; 2]| g[0] * dx[0] + g[1] * dx[1];
    (
        [along(d.dic, x_vb), along(d.dic, x_vc)],
        [along(d.dib, x_vb), along(d.dib, x_vc)],
    )
}

impl Contours {
    /// The quality mode, from the next sample (switchable while it plays): the solvers'
    /// tolerances, in High Fidelity and Potato 1e-9 V (a Newton step of that leaves an error
    /// of the order of its square; `ca72-lab hifi`).
    pub fn set_quality(&mut self, q: crate::voice::Quality) {
        self.lean = q != crate::voice::Quality::NoCompromises;
        (self.tol, self.tol_decay) = match q {
            crate::voice::Quality::NoCompromises => (1e-12, 1e-11),
            crate::voice::Quality::HighFidelity | crate::voice::Quality::Potato => (1e-9, 1e-9),
        };
    }

    /// Steps of `h` s from the next tick (its sub-rate changed; Potato).
    pub fn set_step(&mut self, h: f64) {
        self.h = h;
    }

    pub fn new(circuit: ContourCircuit, rate: f64) -> Contours {
        Contours {
            circuit,
            celsius: 25.0,
            loads: Loads::default(),
            h: 1.0 / rate,
            tol: 1e-12,
            tol_decay: 1e-11,
            t2: circuit.vn,
            v7: circuit.vn,
            sections: [SectionState {
                j7: 0.7,
                j8: 0.6,
                ..SectionState::default()
            }; 2],
            rst: 3.0,
            vtrig: 0.05,
            j10: 0.0,
            j_feed: [0.0; 2],
            j12b: 0.0,
            x20: [0.0, -3.0],
            x12: [0.7, 0.6],
            setup: None,
            inputs: None,
            trig_settled: false,
            sec_settled: [false; 2],
            lean: false,
            trig_inputs: None,
            dump: [f64::NAN; 2],
            devices: std::cell::Cell::new(None),
        }
    }

    /// The NPN's and the PNP's parameters at the temperature, and the germanium and silicon
    /// diodes' laws, worked out again only when it or the models change (performance:
    /// the followers took them at each of their root finder's evaluations). The same values
    /// either way.
    fn devices_and_diodes(&self) -> ([BjtAt; 3], [DiodeAt; 2]) {
        let c = &self.circuit;
        let (q, d) = ([c.npn, c.pnp, c.q12], [c.ge, c.si]);
        let key = self.celsius.to_bits();
        if let Some((k, q0, d0, qa, da)) = self.devices.get()
            && k == key
            && q0 == q
            && d0 == d
        {
            return (qa, da);
        }
        let qa = q.map(|x| x.at(self.celsius));
        let da = d.map(|x| x.at(self.celsius));
        self.devices.set(Some((key, q, d, qa, da)));
        (qa, da)
    }

    /// Settles every state at rest (no key) for these settings.
    pub fn settle(&mut self, panel: &Panel) {
        for _ in 0..(2.0 / self.h) as usize {
            self.tick(false, panel);
        }
    }

    fn setup(&mut self, panel: &Panel) -> [SectionSetup; 2] {
        if let Some((p, l, t, s)) = self.setup
            && p == *panel
            && l == self.loads
            && t == self.celsius
        {
            return s;
        }
        let c = self.circuit;
        let mk = |s: &Section, ctl: &Controls| {
            let p = c.pnp.at(self.celsius);
            // The divider at Q8's base: the SUSTAIN wiper (5K linear across +10 V) through
            // R13, R17 to -10 V, R1 to ground.
            let pos = ctl.sustain.clamp(0.0, 1.0);
            let v_w = c.vp * pos;
            let r_w = 5e3 * pos * (1.0 - pos);
            let g13 = 1.0 / (s.r_sus + r_w);
            let g_neg = if s.r_neg.is_finite() {
                1.0 / s.r_neg
            } else {
                0.0
            };
            let g_sum = g13 + g_neg + 1.0 / s.r_gnd;
            SectionSetup {
                v_th8: (v_w * g13 + c.vn * g_neg) / g_sum,
                r_e8: c.pnp.re + (1.0 / g_sum + c.pnp.rb) / (p.bf + 1.0),
                v_e6: self.q6_emitter(s),
                peak: self.peak(s),
            }
        };
        let s = [
            mk(&c.filter, &panel.filter),
            mk(&c.loudness, &panel.loudness),
        ];
        self.setup = Some((*panel, self.loads, self.celsius, s));
        s
    }

    /// The output at which a section's flip-flop resets: Q1 [Q25] leaves saturation when
    /// its base current from Q4's collector through R3 falls below its collector current
    /// over beta; Q4 then sinks what R4 brings less R3's current, which its base current
    /// through CR3 must support, together with what R14 takes back toward Q1's saturated
    /// collector. The divider gives the output.
    fn peak(&self, s: &Section) -> f64 {
        let c = &self.circuit;
        let n = c.npn.at(self.celsius);
        let p = c.pnp.at(self.celsius);
        // Q1's collector current in the set state: R5 from Q5's base node (Q5's
        // emitter-base drop at its base current).
        let v_sat = 0.05;
        let vbe1 = n.vt * crate::ulp::log(2e-3 / n.is);
        let veb5 = p.vt * crate::ulp::log(5e-3 / p.is);
        let i_c1 = (c.p93 - veb5 - v_sat) / s.r5;
        let ib1_min = i_c1 / n.bf;
        let v_c4 = vbe1 + s.r3 * ib1_min;
        let i_c4 = (c.p93 - v_c4) / s.r4 - (v_c4 - vbe1) / s.r3;
        let vbe4 = n.vt * crate::ulp::log(i_c4 / n.is);
        let i_cr3 = n.base_current(i_c4, -1.0, c.npn.vaf) + (vbe4 - v_sat) / s.r14;
        let law = c.peak_diode.law(self.celsius);
        let v_cr3 = root(|v| law(v).0 - i_cr3, -0.5, 2.0, 0.2, self.tol) + c.peak_diode.rs * i_cr3;
        let v_pk = vbe4 + v_cr3;
        v_pk + s.r_top * ((v_pk - c.vn) / s.r_bot + i_cr3)
    }

    /// Q6's [Q17's] emitter with the flip-flop reset: Q6 follows Q1's [Q25's] collector,
    /// which R6 and R5 pull up against R14's current into Q4's [Q15's] base and Q6's base
    /// current; Q6 feeds R8 to -10 V and Q7's base through R9 (Q7 saturated, its base a
    /// junction above the sustain node, about 2 V).
    fn q6_emitter(&self, s: &Section) -> f64 {
        let c = &self.circuit;
        let n = c.npn.at(self.celsius);
        let vb4 = n.vt * crate::ulp::log(0.5e-3 / n.is);
        let vb7 = 2.0 + n.vt * crate::ulp::log(1.3e-3 * n.bf / n.is);
        let mut v_e6 = 7.5;
        for _ in 0..6 {
            let i_e6 = (v_e6 - c.vn) / s.r8 + ((v_e6 - vb7) / s.r9).max(0.0);
            let i_b6 = n.base_current(i_e6, -1.0, c.npn.vaf);
            let r = s.r6 + s.r5;
            let f_c1 = (c.p93 - r * i_b6 + r * vb4 / s.r14) / (1.0 + r / s.r14);
            v_e6 = f_c1 - n.vt * crate::ulp::log1p(i_e6 / n.is);
        }
        v_e6
    }

    /// The follower's output for the capacitor at `v`, with the output loaded by `g` to
    /// ground and the peak divider to -10 V; from the last output.
    fn follow(&self, s: &Section, g: f64, v: f64, guess: f64, (n, p): (BjtAt, BjtAt)) -> f64 {
        let c = &self.circuit;
        let load = |out: f64| out * g + (out - c.vn) / (s.r_top + s.r_bot);
        let f = |out: f64| match s.follower {
            Follower::Complementary => {
                // Q22 carries Q21's base current: the load's current over (1 + beta21); its
                // base current comes through R48.
                let ie22 = load(out).max(0.0) / (1.0 + p.bf);
                let ib22 = ie22 / (1.0 + n.bf);
                v - s.r_in * ib22 - n.vt * crate::ulp::log1p(ie22 / n.is) - out
            }
            Follower::Darlington => {
                let ie2 = load(out).max(0.0);
                let ib2 = ie2 / (1.0 + n.bf);
                let ib3 = ib2 / (1.0 + n.bf);
                v - s.r_in * ib3
                    - n.vt * crate::ulp::log1p(ib2 / n.is)
                    - n.vt * crate::ulp::log1p(ie2 / n.is)
                    - out
            }
        };
        // Falling in the output.
        debug_assert!(f(c.vn) >= 0.0);
        root_dir(
            f,
            c.vn,
            v + 1.0,
            guess.clamp(c.vn, v + 1.0),
            false,
            self.tol,
        )
    }

    /// One sample: the key (held or not) and the settings in; the contours out.
    pub fn tick(&mut self, key: bool, panel: &Panel) -> ContourOut {
        let mut laps = crate::prof::Laps::start();
        let setup = self.setup(panel);
        let c = self.circuit;
        let h = self.h;
        let (tol, tol_decay) = (self.tol, self.tol_decay);
        let ([npn, pnp, q12d], [ge_at, si_at]) = self.devices_and_diodes();
        let ge = move |v: f64| ge_at.current(v);
        let si = move |v: f64| si_at.current(v);
        let base = npn.base_law();
        // The trigger bus: held at +10 V by the key; otherwise it follows C13's node.
        let (t2_inf, tau) = if key {
            let r = c.r65 * c.r34 / (c.r65 + c.r34);
            ((c.vp * c.r34 + c.vn * c.r65) / (c.r65 + c.r34), r * c.c13)
        } else {
            (c.vn, c.r34 * c.c13)
        };
        let t2_old = self.t2;
        self.t2 = t2_inf + (self.t2 - t2_inf) * crate::ulp::exp(-h / tau);
        let kb = if key { c.vp } else { self.t2 };
        let inputs = Some((key, *panel, self.loads, self.celsius.to_bits()));
        let same = inputs == self.inputs;
        let trig_inputs = Some((key, panel.s_trig, self.celsius.to_bits()));
        let same_trig = if self.lean {
            trig_inputs == self.trig_inputs
        } else {
            same
        };
        self.trig_inputs = trig_inputs;
        if !same {
            self.inputs = inputs;
            self.sec_settled = [false; 2];
        }
        if !same_trig {
            self.trig_settled = false;
        }
        // C7: charged through CR10 and R60, drained through R55 into Q20's base; the
        // reset line: R37 against Q20's collector, Q12's base (R19), the flip-flops' reset
        // feeds (R35, R36 through CR1, CR8 into a conducting base) and EXT. S-TRIG. Q20 in
        // every region (in saturation its collector junction takes base current too, which
        // drains C7 faster), so C7 and the line are solved together: two passes.
        let vb4 = npn.vt * crate::ulp::log(0.5e-3 / npn.is);
        let j_feed = self.j_feed;
        let feed_at =
            |rst: f64, r: f64, j: f64| series_junction_from(rst - vb4, r + c.ge.rs, ge, j);
        let j12b = self.j12b;
        let base12 = q12d.base_law();
        let q12_base = |r: f64| series_junction_from(r, c.r19 + c.q12.rb, &base12, j12b);
        let j10 = self.j10;
        // (In High Fidelity and Potato each solve of Q20 and Q12 starts from the last
        // one's, not the last tick's, and limits its steps as the nodal solver does.)
        let lean = self.lean;
        let x20 = std::cell::Cell::new(self.x20);
        let q20 = |v7: f64, rst: f64| {
            if lean {
                let s = grounded_npn_with(&npn, &c.npn, x20.get(), (v7, c.r55, rst), tol, true);
                x20.set(s.0);
                s
            } else {
                grounded_npn(&npn, &c.npn, x20.get(), v7, c.r55, rst, tol)
            }
        };
        let i60 = |v7: f64| series_junction_from(kb - v7, c.r60 + c.ge.rs, ge, j10).1;
        let (v0, rst0) = (self.v7, self.rst);
        let (mut v7, mut rst) = (v0, rst0);
        let passes = if self.trig_settled { 0 } else { 2 };
        let f_old = if passes > 0 {
            (i60(v0) - q20(v0, rst0).2) / c.c7
        } else {
            0.0
        };
        // A junction behind a resistance moves its current by g / (1 + R g) a volt.
        let through = |g: f64, r: f64| g / (1.0 + r * g);
        // The root finders' brackets are checked (debug builds) with the transistor solved
        // from anywhere to convergence: the warm-started solves they use are held to 0.2 V
        // down and 2 Vt up a step, which does not reach a bracket's far end in its steps
        // (the finders never evaluate there).
        let solved = |vb: f64, r: f64, vc: f64| {
            grounded_npn_with(&npn, &c.npn, [0.7, 0.6], (vb, r, vc), 1e-12, true)
        };
        for _ in 0..passes {
            let r_now = rst;
            // Rising: C7's current falls as it charges.
            let g7_by = |v: f64, (x, _, ib): ([f64; 2], f64, f64)| {
                let (vj, i) = series_junction_from(kb - v, c.r60 + c.ge.rs, ge, j10);
                let di = -through(ge(vj).1, c.r60 + c.ge.rs);
                let (_, dib) = grounded_npn_slopes(&npn, &c.npn, x, c.r55);
                let f7 = (i - ib) / c.c7;
                let df7 = (di - dib[0]) / c.c7;
                (v - v0 - 0.5 * h * (f_old + f7), 1.0 - 0.5 * h * df7)
            };
            let g7 = |v: f64| g7_by(v, q20(v, r_now));
            debug_assert!(g7_by(v0 - 20.0, solved(v0 - 20.0, c.r55, r_now)).0 < 0.0);
            v7 = root_slope(g7, v0 - 20.0, v0 + 20.0, v0 + h * f_old, true, tol);
            let v_now = v7;
            let f_rst_by = |r: f64, (x, ic20, _): ([f64; 2], f64, f64)| {
                let (dic20, _) = grounded_npn_slopes(&npn, &c.npn, x, c.r55);
                let (j12, ib12) = q12_base(r);
                let d12 = through(base12(j12).1, c.r19 + c.q12.rb);
                let (s_trig, d_trig) = if panel.s_trig {
                    (r / c.r49, 1.0 / c.r49)
                } else {
                    (0.0, 0.0)
                };
                let mut f = (c.p93 - r) / c.r37 - ic20 - ib12 - s_trig;
                let mut df = -1.0 / c.r37 - dic20[1] - d12 - d_trig;
                for (k, r_reset) in [c.filter.r_reset, c.loudness.r_reset]
                    .into_iter()
                    .enumerate()
                {
                    let (jf, i) = feed_at(r, r_reset, j_feed[k]);
                    f -= i;
                    df -= through(ge(jf).1, r_reset + c.ge.rs);
                }
                (f, df)
            };
            let f_rst = |r: f64| f_rst_by(r, q20(v_now, r));
            // Falling: every current the line feeds grows with it.
            debug_assert!(f_rst_by(0.0, solved(v_now, c.r55, 0.0)).0 >= 0.0);
            rst = root_slope(f_rst, 0.0, c.p93, rst, false, tol);
        }
        if passes > 0 {
            self.trig_settled = same_trig
                && (v7 - v0).abs() < SETTLED
                && (rst - rst0).abs() < SETTLED
                && (self.t2 - t2_old).abs() < SETTLED;
            self.v7 = v7;
            self.rst = rst;
            self.j10 = series_junction_from(kb - v7, c.r60 + c.ge.rs, ge, j10).0;
            self.j12b = q12_base(rst).0;
            self.x20 = q20(v7, rst).0;
        }
        laps.lap(Part::Trigger);
        // Q12 likewise: its base through R19 from the reset line, its collector V-trig:
        // R32 and R23 from +9.3 V against it; the sections' sustain nodes feed it through
        // CR2 [CR9] and, with DECAY off, their capacitors through CR7 [CR4] and R1401.
        let x12 = std::cell::Cell::new(self.x12);
        let q12 = |vt: f64| {
            if lean {
                let s = grounded_npn_with(&q12d, &c.q12, x12.get(), (rst, c.r19, vt), tol, true);
                x12.set(s.0);
                s
            } else {
                grounded_npn(&q12d, &c.q12, x12.get(), rst, c.r19, vt, tol)
            }
        };
        // The dump node (R1401's far end) for V-trig at `vt`, the diodes of the capacitors
        // `caps` into one R1401: both into the left hand controller's one, or each into its
        // own ([`ContourCircuit::dump_each`]). (In High Fidelity and Potato from its last
        // solution with its slope analytic, not from V-trig's voltage by differences: its
        // bisections from there made the contours' slowest ticks.)
        let (lean, dump_warm) = (self.lean, self.dump);
        let dump_node = |vt: f64, caps: &[f64], warm: f64| {
            let fd = |d: f64| caps.iter().map(|&v| si(v - d).0).sum::<f64>() - (d - vt) / c.r1401;
            let lo = caps.iter().fold(vt, |a, &v| a.min(v)) - 1.0;
            let hi = caps.iter().fold(vt, |a, &v| a.max(v)) + 1.0;
            // Falling: the diodes' currents fall and R1401's grows as the node rises.
            debug_assert!(fd(lo) >= 0.0);
            if lean {
                // (Beyond 80 thermal voltages the diode law holds its exponent, so its slope
                // there is not its current's: no slope, and the root finder bisects. Nothing
                // here carries an ampere.)
                let fd_slope = |d: f64| {
                    let (i, g) = caps.iter().fold((0.0, 0.0), |(i, g), &v| {
                        let (iv, gv) = si(v - d);
                        (i + iv, g + gv)
                    });
                    let f = i - (d - vt) / c.r1401;
                    let slope = if f.abs() < 1.0 {
                        -g - 1.0 / c.r1401
                    } else {
                        0.0
                    };
                    (f, slope)
                };
                let guess = if warm.is_finite() { warm } else { vt };
                root_slope(fd_slope, lo, hi, guess, false, tol)
            } else {
                root_dir(fd, lo, hi, vt, false, tol)
            }
        };
        // Each section's dump node for V-trig at `vt`, the sections' capacitors at `ss`.
        let dump_nodes = |vt: f64, ss: &[SectionState; 2]| {
            if c.dump_each {
                [
                    dump_node(vt, &[ss[0].v], dump_warm[0]),
                    dump_node(vt, &[ss[1].v], dump_warm[1]),
                ]
            } else {
                let d = dump_node(vt, &[ss[0].v, ss[1].v], dump_warm[0]);
                [d, d]
            }
        };
        // The sections as the tick found them.
        let first = self.sections;
        let f_vt_by = |vt: f64, (x, ic12, _): ([f64; 2], f64, f64), ss: &[SectionState; 2]| {
            let (dic12, _) = grounded_npn_slopes(&q12d, &c.q12, x, c.r19);
            let (dump, d_dump) = if panel.decay_on {
                (0.0, 0.0)
            } else {
                // A node moves with V-trig by R1401's share against its diodes'. (Its
                // capacitors as the tick found them: their dump is not solved with the
                // passes, B2-13.)
                let d = dump_nodes(vt, &first);
                let at = |d: f64, g_diodes: f64| {
                    let dd = 1.0 / (c.r1401 * (g_diodes + 1.0 / c.r1401));
                    ((d - vt) / c.r1401, (dd - 1.0) / c.r1401)
                };
                if c.dump_each {
                    let (i0, g0) = at(d[0], si(first[0].v - d[0]).1);
                    let (i1, g1) = at(d[1], si(first[1].v - d[1]).1);
                    (i0 + i1, g0 + g1)
                } else {
                    at(d[0], si(first[0].v - d[0]).1 + si(first[1].v - d[0]).1)
                }
            };
            let (i0, g0) = si(ss[0].e7 - vt);
            let (i1, g1) = si(ss[1].e7 - vt);
            (
                (c.p93 - vt) / (c.r32 + c.r23) + i0 + i1 + dump - ic12,
                -1.0 / (c.r32 + c.r23) - g0 - g1 + d_dump - dic12[1],
            )
        };
        // V-trig for the sections at `ss`, from `guess`.
        let solve_vt = |ss: &[SectionState; 2], guess: f64| {
            let f_vt = |vt: f64| f_vt_by(vt, q12(vt), ss);
            root_slope(f_vt, 0.0, c.p93, guess, false, tol)
        };
        let vtrig_old = self.vtrig;
        // Falling: every current into V-trig falls as it rises.
        debug_assert!(
            f_vt_by(
                0.0,
                grounded_npn_with(&q12d, &c.q12, [0.7, 0.6], (rst, c.r19, 0.0), 1e-12, true),
                &self.sections
            )
            .0 >= 0.0
        );
        // V-trig and the sections are one system: through CR2 [CR9] (and with DECAY off CR7
        // [CR4]) a falling V-trig draws the capacitors' currents, which hold it up. Solved
        // from the sections' last states and they then stepped from it, a release through
        // DECAY's end ran slower the longer the step (the error halved as the rate doubled:
        // at Potato's 6 kHz 2.5 times ngspice's fall). While V-trig falls, V-trig is solved
        // again from the sections as stepped and they again from it, by Newton's method on
        // V-trig through the sections' own response to it, in a bracket, to the solvers'
        // tolerance (board2.md, B2-13). Still or rising, it is one pass as before.
        let start = (self.sections, self.j_feed, self.sec_settled);
        let mut vtrig = solve_vt(&first, vtrig_old);
        // Coupled only while V-trig falls and a capacitor's current through DECAY is large
        // (with DECAY near its end): otherwise one pass, as before.
        let couple = vtrig_old - vtrig > COUPLED
            && [(c.filter, panel.filter), (c.loudness, panel.loudness)]
                .iter()
                .zip(first)
                .any(|((s, ctl), st)| {
                    (st.v - st.e7).abs() >= COUPLING_CURRENT * (ctl.decay.max(0.0) + 5.0 + s.esr)
                });
        let mut bracket = (0.0, c.p93);
        // Each decaying section's start for the next pass: this pass's solution moved along
        // its response to V-trig's step (from the start of the sample, its node's steps held
        // to 0.2 V, a pass took many iterations where the capacitors fall volts a sample).
        let mut guess: [Option<(f64, f64)>; 2] = [None; 2];
        let mut pass = 0;
        let mut out;
        loop {
            (self.sections, self.j_feed, self.sec_settled) = start;
            // Each section's capacitor and sustain node per volt of V-trig, this pass.
            let mut sens = [[0.0f64; 2]; 2];
            let mut stepped: [Option<(f64, f64)>; 2] = [None; 2];
            self.vtrig = vtrig;
            self.x12 = q12(vtrig).0;
            let d_nodes = if panel.decay_on {
                [vtrig; 2]
            } else {
                dump_nodes(vtrig, &first)
            };
            self.dump = if lean && !panel.decay_on {
                d_nodes
            } else {
                [f64::NAN; 2]
            };
            laps.lap(Part::VTrig);
            // The flip-flops: held reset while the reset line drives CR1 [CR8] (the current a
            // reset needs is the one the peak detector must supply); set as V-trig rises.
            let rising = vtrig_old < 0.5 * c.p93 && vtrig >= 0.5 * c.p93;
            out = ContourOut {
                vtrig,
                ..ContourOut::default()
            };
            for (k, &d_node) in d_nodes.iter().enumerate() {
                let (s, ctl, g, su) = if k == 0 {
                    (c.filter, panel.filter, self.loads.filter, setup[0])
                } else {
                    (c.loudness, panel.loudness, self.loads.loudness, setup[1])
                };
                let mut st = self.sections[k];
                let was_set = st.set;
                let (jf, i_feed) = feed_at(rst, s.r_reset, self.j_feed[k]);
                self.j_feed[k] = jf;
                if i_feed > 20e-6 {
                    st.set = false;
                } else if rising {
                    st.set = true;
                }
                if st.set != was_set || (vtrig - vtrig_old).abs() >= SETTLED {
                    self.sec_settled[k] = false;
                }
                if self.sec_settled[k] {
                    if k == 0 {
                        out.filter = st.out;
                    } else {
                        out.loudness = st.out;
                    }
                    continue;
                }
                let before = (st.v, st.e7, st.out);
                // With DECAY off, the capacitor feeds the dump node through CR7 [CR4].
                let dump_at = |v: f64| {
                    if panel.decay_on {
                        (0.0, 0.0)
                    } else {
                        si(v - d_node)
                    }
                };
                let i_dump = |v: f64| dump_at(v).0;
                let v_old = st.v;
                // The capacitor's absorption: its branches' current now and their companion.
                let (i_da_old, g_da, b_da) = st.absorption(&s, h);
                // Q5 saturated (driven by about 2.6 mA): 25 mV; the attack's current through R7,
                // ATTACK and the capacitor's series resistance, and the terminal it lifts.
                let v5 = c.p93 - 0.025;
                let ga = 1.0 / (s.r7 + ctl.attack.max(0.0) + s.esr);
                let terminal_attacking = |v: f64| v + s.esr * ga * (v5 - v);
                let terminal;
                if st.set {
                    // The capacitor charges (trapezoidal, exact for this linear path without
                    // the absorption).
                    st.v = (s.c / h * v_old
                        + 0.5 * (ga * (v5 - v_old) - i_da_old + ga * v5 + b_da))
                        / (s.c / h + 0.5 * (ga + g_da));
                    st.attacked = true;
                    terminal = terminal_attacking(st.v);
                } else {
                    // Decay: the capacitor and the sustain node together (Newton on both): the
                    // capacitor discharges through DECAY and saturated Q7 into the node, which
                    // R10 and Q7's base current (from Q6 through R9) also feed, Q8 holds at its
                    // base divider and CR2 pulls toward V-trig.
                    let r_dec = ctl.decay.max(0.0) + 5.0 + s.esr;
                    let emitter = |x: f64| {
                        let (i, d) = pnp.base_law()(x);
                        (i * (pnp.bf + 1.0), d * (pnp.bf + 1.0))
                    };
                    let (j7, j8) = (st.j7, st.j8);
                    let r7 = s.r9 + c.npn.rb;
                    // Trapezoidal, but the first sample after the attack backward Euler (as
                    // ngspice steps after a switch): the path through DECAY was open and the
                    // sustain node's last voltage is from before the attack, and through DECAY's
                    // end (5 ohm) their difference made an ampere of current that never flowed,
                    // the capacitor stepped volts below the node.
                    let (w_old, w_new) = if st.attacked { (0.0, 1.0) } else { (0.5, 0.5) };
                    st.attacked = false;
                    let i_old = -(v_old - st.e7) / r_dec - i_dump(v_old);
                    // The residuals and their Jacobian, analytic: each junction in series with
                    // its resistance changes its current by g / (1 + R g) a volt, g its slope at
                    // the solution (performance: finite differences took three residuals).
                    let res = |v: f64, e7: f64| {
                        let (x7, ib7) = series_junction_from(su.v_e6 - e7, r7, &base, j7);
                        let g7 = base(x7).1;
                        let (i_b7, d_b7) = if ib7 > 0.0 {
                            (ib7, -g7 / (1.0 + r7 * g7))
                        } else {
                            (0.0, 0.0)
                        };
                        let (x8, i8) = series_junction_from(e7 - su.v_th8, su.r_e8, emitter, j8);
                        let g8 = emitter(x8).1;
                        let d8 = g8 / (1.0 + su.r_e8.max(0.0) * g8);
                        let (i_si, g_si) = si(e7 - vtrig);
                        let (i_d, g_d) = dump_at(v);
                        let i = -(v - e7) / r_dec - i_d;
                        // (The absorption's branches were never open: trapezoidal throughout.)
                        let i_da = g_da * v - b_da;
                        let r = [
                            v - v_old - h * (w_old * i_old + w_new * i) / s.c
                                + 0.5 * h * (i_da_old + i_da) / s.c,
                            (c.p93 - e7) / s.r10 + (v - e7) / r_dec + i_b7 - i8 - i_si,
                        ];
                        let j = [
                            [
                                1.0 + (w_new * (1.0 / r_dec + g_d) + 0.5 * g_da) * h / s.c,
                                -w_new * h / (r_dec * s.c),
                            ],
                            [1.0 / r_dec, -1.0 / s.r10 - 1.0 / r_dec + d_b7 - d8 - g_si],
                        ];
                        (r, j)
                    };
                    let (mut v, mut e7) = guess[k]
                        .unwrap_or((v_old + (2.0 * w_old * i_old - i_da_old) * h / s.c, st.e7));
                    let mut converged = false;
                    // (Up to 100 iterations: its node's steps held to 0.2 V, a jump of volts takes
                    // tens; 30 stopped short twice in perf-ext, which `unconverged` counts.)
                    for _ in 0..100 {
                        let (r0, j) = res(v, e7);
                        let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
                        let sv = (r0[0] * j[1][1] - r0[1] * j[0][1]) / det;
                        let se = (j[0][0] * r0[1] - j[1][0] * r0[0]) / det;
                        // Limit the node's steps (its diodes are exponential).
                        let se = se.clamp(-0.2, 0.2);
                        v -= sv;
                        e7 -= se;
                        if sv.abs() < tol_decay && se.abs() < tol_decay {
                            converged = true;
                            break;
                        }
                    }
                    if !converged {
                        crate::unconverged::note(crate::unconverged::Solver::ContourDecay);
                    }
                    // Off where the circuit goes (a POLY voice resumed after resting, its
                    // capacitor where it stopped, the knobs moved meanwhile to DECAY's end) the
                    // steps can run off below the -10 V rail. Then the step again from where the
                    // capacitor was, both nodes' steps held to 0.2 V, and failing that the
                    // capacitor held where it was for this sample.
                    if !(v.is_finite() && e7.is_finite()) || v < c.vn {
                        let (mut v2, mut e72) = (v_old, st.e7);
                        let mut settled = false;
                        for _ in 0..400 {
                            let (r0, j) = res(v2, e72);
                            let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
                            let sv = ((r0[0] * j[1][1] - r0[1] * j[0][1]) / det).clamp(-0.2, 0.2);
                            let se = ((j[0][0] * r0[1] - j[1][0] * r0[0]) / det).clamp(-0.2, 0.2);
                            v2 -= sv;
                            e72 -= se;
                            if sv.abs() < tol_decay && se.abs() < tol_decay {
                                settled = true;
                                break;
                            }
                        }
                        (v, e7) = if settled && v2.is_finite() && e72.is_finite() && v2 >= c.vn {
                            (v2, e72)
                        } else {
                            (v_old, st.e7)
                        };
                    }
                    if couple {
                        // How they move with V-trig (through CR2 [CR9]; the dump node is held
                        // for the sample): J (dv, de7) = -d(residuals)/d(V-trig).
                        let (_, jj) = res(v, e7);
                        let cv = [0.0, si(e7 - vtrig).1];
                        let det = jj[0][0] * jj[1][1] - jj[0][1] * jj[1][0];
                        sens[k] = [
                            -(cv[0] * jj[1][1] - cv[1] * jj[0][1]) / det,
                            -(jj[0][0] * cv[1] - jj[1][0] * cv[0]) / det,
                        ];
                        stepped[k] = Some((v, e7));
                    }
                    st.v = v;
                    st.e7 = e7;
                    st.j7 = series_junction_from(su.v_e6 - e7, s.r9 + c.npn.rb, &base, j7).0;
                    st.j8 = series_junction_from(e7 - su.v_th8, su.r_e8, emitter, j8).0;
                    terminal = v - s.esr * ((v - e7) / r_dec + i_dump(v));
                }
                laps.lap(Part::Decay);
                st.out = self.follow(&s, g, terminal, st.out, (npn, pnp));
                laps.lap(Part::Follow);
                if st.set && st.out >= su.peak {
                    // The flip-flop resets as the output reaches the peak, within the sample:
                    // the capacitor stops there (its voltage where the output meets the peak,
                    // by the secant through the sample's two ends and once again), not a
                    // sample's rise above it (at Potato's 6 kHz half a volt with ATTACK at 0).
                    let (v0, o0) = (before.0, before.2);
                    let mut absorbed = false;
                    if st.out > o0 && o0 < su.peak {
                        let v_end = st.v;
                        let mut v = v0 + (su.peak - o0) * (st.v - v0) / (st.out - o0);
                        let mut out =
                            self.follow(&s, g, terminal_attacking(v), su.peak, (npn, pnp));
                        if out > o0 && (out - su.peak).abs() > 1e-6 {
                            v = v0 + (su.peak - o0) * (v - v0) / (out - o0);
                            out = self.follow(&s, g, terminal_attacking(v), out, (npn, pnp));
                        }
                        // The absorption's branches charge until then; for the rest of the
                        // sample the capacitor shares its charge with them alone (the decay's
                        // path opens on the next), as it starts to in the circuit.
                        let theta = ((v - v0) / (v_end - v0)).clamp(0.0, 1.0);
                        st.v = v;
                        st.absorb(&s, theta * h, v0);
                        let hr = (1.0 - theta) * h;
                        if hr > 0.0 {
                            let (i0, g_da, b_da) = st.absorption(&s, hr);
                            let v_at = st.v;
                            st.v =
                                (s.c / hr * v_at - 0.5 * i0 + 0.5 * b_da) / (s.c / hr + 0.5 * g_da);
                            st.absorb(&s, hr, v_at);
                            out = self.follow(&s, g, st.v, out, (npn, pnp));
                        }
                        // The attack's current stops with the reset: the terminal falls to the
                        // capacitor's charge (its series resistance's step).
                        if hr <= 0.0 {
                            out = self.follow(&s, g, st.v, out, (npn, pnp));
                        }
                        st.out = out;
                        absorbed = true;
                    }
                    st.set = false;
                    if !absorbed {
                        st.absorb(&s, h, v_old);
                    }
                } else {
                    st.absorb(&s, h, v_old);
                }
                self.sec_settled[k] = same
                    && !st.set
                    && st.absorbed(&s)
                    && (st.v - before.0).abs() < SETTLED
                    && (st.e7 - before.1).abs() < SETTLED
                    && (st.out - before.2).abs() < SETTLED;
                self.sections[k] = st;
                if k == 0 {
                    out.filter = st.out;
                } else {
                    out.loudness = st.out;
                }
            }
            pass += 1;
            // Only a falling V-trig draws the capacitors' currents (rising, CR2 [CR9] and CR7
            // [CR4] turn off; and there the flip-flops' set at half the rail is a step that
            // the passes would straddle).
            if !couple {
                break;
            }
            let g = solve_vt(&self.sections, vtrig);
            let f = g - vtrig;
            if f.abs() <= if lean { COUPLED } else { tol } {
                break;
            }
            if pass >= COUPLING_PASSES {
                crate::unconverged::note(crate::unconverged::Solver::ContourCoupling);
                break;
            }
            // F(V-trig) = g - V-trig falls through zero as V-trig rises: keep the bracket.
            if f > 0.0 {
                bracket.0 = vtrig;
            } else {
                bracket.1 = vtrig;
            }
            // Newton's step on F: g's slope from V-trig's KCL at g, the sections' sustain
            // nodes (through CR2 [CR9]) moving with V-trig as this pass found.
            let slope = f_vt_by(g, q12(g), &self.sections).1;
            let moved: f64 = self
                .sections
                .iter()
                .zip(sens)
                .map(|(st, sk)| si(st.e7 - g).1 * sk[1])
                .sum();
            let next = vtrig - f / (-moved / slope - 1.0);
            let next = if next > bracket.0 && next < bracket.1 {
                next
            } else {
                0.5 * (bracket.0 + bracket.1)
            };
            for k in 0..2 {
                guess[k] = stepped[k].map(|(v, e7)| {
                    (
                        v + sens[k][0] * (next - vtrig),
                        e7 + sens[k][1] * (next - vtrig),
                    )
                });
            }
            vtrig = next;
        }
        out
    }

    /// The timing capacitors' voltages (C5, C2), for tests.
    pub fn capacitors(&self) -> [f64; 2] {
        [self.sections[0].v, self.sections[1].v]
    }

    /// The outputs at which the flip-flops reset, V.
    pub fn peaks(&mut self, panel: &Panel) -> [f64; 2] {
        let s = self.setup(panel);
        [s[0].peak, s[1].peak]
    }

    /// The trigger section's nodes: C7, the reset line, V-trig.
    pub fn trigger_nodes(&self) -> [f64; 3] {
        [self.v7, self.rst, self.vtrig]
    }
}
