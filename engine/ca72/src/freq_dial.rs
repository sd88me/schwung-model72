//! Oscillators 2 and 3's FREQUENCY knobs: how a knob's dial sets its pot (decisions.md
//! R54). The pot (R4 or R5, 5K linear: [`crate::tuning::freq_pot`]) puts its oscillator
//! so many semitones from the centre where the factory tuning left it; the dial is printed a
//! semitone a mark, -7 to +7, its travel ending at -7.5 and +7.5 (history.md, "The ticks").
//! The player chooses the knob's law (the editor's settings key, for the computer):
//!
//! - [`Law::Semitones`], the default: every mark where its number says, -7.5 to +7.5
//!   semitones at the stops (the owner, 2026-10-10: "use our perfect ones by default").
//! - [`Law::Hardware`]: the hardware reference's knob, as session N measured it
//!   (docs/calibration, change 29): oscillator 2 -6.48 and +6.61 semitones at the -5 and +5
//!   marks from its 0, the pot's ends at the stops.
//!
//! A sound keeps the pot's position, whatever the law (its plain value, -7.5 to +7.5, is the
//! wiper at (value + 7.5) / 15 of the track, as in 0.1.x): it plays the same through either,
//! only where the knob points changes. A pot beyond a law's stop shows the knob at that stop.

/// How a FREQUENCY knob's dial sets its pot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Law {
    /// Every mark a semitone from the centre, where its number says.
    #[default]
    Semitones,
    /// The hardware reference's knob.
    Hardware,
}

/// Which oscillator's pot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pot {
    /// Oscillator 2's (R4, on IC4's + input).
    Two,
    /// Oscillator 3's (R5, through its control stage IC8), with OSC. 3 CONTROL on. Off, the
    /// pot spans some 69 semitones and its dial gives no interval: the knob sets the pot as it
    /// would with the control on.
    Three,
}

/// The pot's semitones from its centre, measured on the voice (`tests/freq_dial.rs`, each
/// oscillator alone, A3 held, from its resets at 61 positions): a quintic in the wiper's
/// distance from the centre (coefficients of the first to the fifth power), within 0.003 cent
/// of the measured on A3 and 0.08 cent on A2 and A5. The pot spans -8.56 to +8.24 semitones
/// (oscillator 2) and -8.63 to +8.22 (oscillator 3), as the service manual's 14 to 17.
const OSC2_SEMITONES: [f64; 5] = [16.753480, -0.651459, 0.194997, -0.005174, -0.000512];
const OSC3_SEMITONES: [f64; 5] = [16.634609, -0.799108, 0.818439, -0.073963, 0.039155];

/// Oscillator 2's FREQUENCY pot as the hardware reference's knob sets it: the fraction of the
/// track at the dial's stops and its -5 and +5 marks (the knob 0..1 for -7.5..+7.5) at which
/// the oscillator stands where the reference's does from its 0 mark (session N,
/// docs/calibration). The 0 mark stays the centre, where the factory tuning puts it in unison
/// with oscillator 1. A linear track's are 0, 1/6, 5/6 and 1; the clockwise stop reaches 0.06
/// semitone short of the reference's.
const OSC2_HARDWARE: [(f64, f64); 5] = [
    (0.0, 0.0334),
    (1.0 / 6.0, 0.1195),
    (0.5, 0.5),
    (5.0 / 6.0, 0.9000),
    (1.0, 1.0),
];

/// Oscillator 3's likewise (OSC. 3 CONTROL on when measured); its clockwise stop reaches 0.33
/// semitone short of the reference's.
const OSC3_HARDWARE: [(f64, f64); 5] = [
    (0.0, 0.0008),
    (1.0 / 6.0, 0.1060),
    (0.5, 0.5),
    (5.0 / 6.0, 0.9209),
    (1.0, 1.0),
];

/// The dial's ends, its marks' units.
pub const DIAL_END: f64 = 7.5;

impl Pot {
    fn curve(self) -> &'static [f64; 5] {
        match self {
            Pot::Two => &OSC2_SEMITONES,
            Pot::Three => &OSC3_SEMITONES,
        }
    }

    fn hardware(self) -> &'static [(f64, f64); 5] {
        match self {
            Pot::Two => &OSC2_HARDWARE,
            Pot::Three => &OSC3_HARDWARE,
        }
    }

    /// The oscillator's semitones from its centre with the pot's wiper at `wiper` of its
    /// track (0..1).
    pub fn semitones(self, wiper: f64) -> f64 {
        let x = wiper.clamp(0.0, 1.0) - 0.5;
        x * self.curve().iter().rev().fold(0.0, |a, &c| a * x + c)
    }

    /// The pot's wiper (0..1 of its track) that puts the oscillator `semitones` from its
    /// centre; the pot's end beyond its reach.
    pub fn wiper(self, semitones: f64) -> f64 {
        if semitones <= self.semitones(0.0) {
            return 0.0;
        }
        if semitones >= self.semitones(1.0) {
            return 1.0;
        }
        // Newton's steps from the curve's slope at the centre (it rises everywhere, 16.2 to
        // 18.6 semitones a track: a few steps reach the last bit).
        let slope = self.curve()[0];
        let mut w = 0.5 + semitones / slope;
        for _ in 0..8 {
            let x = w.clamp(0.0, 1.0) - 0.5;
            let d = self
                .curve()
                .iter()
                .enumerate()
                .rev()
                .fold(0.0, |a, (k, &c)| a * x + (k + 1) as f64 * c);
            w = (w - (self.semitones(w) - semitones) / d).clamp(0.0, 1.0);
        }
        w
    }

    /// The pot's wiper with the knob at `knob` of its travel (0..1, the dial's -7.5 to +7.5)
    /// read through `law`.
    pub fn wiper_at(self, law: Law, knob: f64) -> f64 {
        let knob = knob.clamp(0.0, 1.0);
        match law {
            Law::Semitones => self.wiper((2.0 * knob - 1.0) * DIAL_END),
            Law::Hardware => along(self.hardware(), knob),
        }
    }

    /// Where the knob points (0..1 of its travel) with the pot at `wiper` through `law`: at its
    /// stop for a pot beyond the law's reach.
    pub fn knob_at(self, law: Law, wiper: f64) -> f64 {
        let wiper = wiper.clamp(0.0, 1.0);
        match law {
            Law::Semitones => (0.5 + self.semitones(wiper) / (2.0 * DIAL_END)).clamp(0.0, 1.0),
            Law::Hardware => back_along(self.hardware(), wiper),
        }
    }
}

/// Straight between a law's points (knob, track; the knob from 0 to 1).
fn along(points: &[(f64, f64)], knob: f64) -> f64 {
    for w in points.windows(2) {
        let ((pa, ta), (pb, tb)) = (w[0], w[1]);
        if knob <= pb {
            return ta + (tb - ta) * (knob - pa) / (pb - pa);
        }
    }
    points[points.len() - 1].1
}

/// [`along`]'s inverse: the knob for a track, at the law's first or last point beyond them.
fn back_along(points: &[(f64, f64)], track: f64) -> f64 {
    if track <= points[0].1 {
        return points[0].0;
    }
    for w in points.windows(2) {
        let ((pa, ta), (pb, tb)) = (w[0], w[1]);
        if track <= tb {
            return pa + (pb - pa) * (track - ta) / (tb - ta);
        }
    }
    points[points.len() - 1].0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Through either law the knob comes back where it was set, the pot at the centre stays
    /// there, and each law's marks are its own: a semitone a mark, or the reference's.
    #[test]
    fn the_laws_and_their_inverses() {
        for pot in [Pot::Two, Pot::Three] {
            assert_eq!(pot.semitones(0.5), 0.0);
            for law in [Law::Semitones, Law::Hardware] {
                assert!(
                    (pot.wiper_at(law, 0.5) - 0.5).abs() < 1e-12,
                    "{pot:?} {law:?}"
                );
                for k in 0..=150 {
                    let knob = f64::from(k) / 150.0;
                    let w = pot.wiper_at(law, knob);
                    assert!(
                        (pot.knob_at(law, w) - knob).abs() < 1e-9,
                        "{pot:?} {law:?} {knob}"
                    );
                }
            }
            for mark in -15..=15 {
                let st = f64::from(mark) / 2.0;
                let w = pot.wiper_at(Law::Semitones, (st + DIAL_END) / (2.0 * DIAL_END));
                assert!((pot.semitones(w) - st).abs() < 1e-9, "{pot:?} {st}");
            }
            // The hardware law's points: the track's (the knob 0..1 for -7.5..+7.5).
            for &(knob, t) in pot.hardware() {
                assert!((pot.wiper_at(Law::Hardware, knob) - t).abs() < 1e-12);
            }
            // Beyond a law's reach, the knob at its stop.
            assert_eq!(pot.knob_at(Law::Semitones, 1.0), 1.0);
            assert_eq!(pot.knob_at(Law::Semitones, 0.0), 0.0);
            assert_eq!(pot.knob_at(Law::Hardware, 0.0), 0.0);
            assert_eq!(pot.wiper(-20.0), 0.0);
            assert_eq!(pot.wiper(20.0), 1.0);
        }
        // The reference's -5 and +5 marks (session N), from its 0 mark (-0.07 and -0.26
        // semitone): oscillator 2 at -6.48 and +6.61, oscillator 3 at -6.73 and +6.92.
        for (pot, marks) in [(Pot::Two, [-6.48, 6.61]), (Pot::Three, [-6.73, 6.92])] {
            for (knob, want) in [1.0 / 6.0, 5.0 / 6.0].into_iter().zip(marks) {
                let got = pot.semitones(pot.wiper_at(Law::Hardware, knob));
                assert!((got - want).abs() < 0.01, "{pot:?} {knob}: {got}");
            }
        }
    }
}
