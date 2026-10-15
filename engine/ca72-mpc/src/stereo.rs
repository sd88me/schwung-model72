#![allow(dead_code)]
//! Vendored from idlefoundry/plugin-kit `crates/plugin-kit-stereo/src/place.rs` at
//! 4c0f0f31041cb28b9dc803996dd01d8f44af5ce8 (GPL-3.0-or-later; see engine/VENDORED.md): the glide and
//! the tests left out.
//!
//! Where the voices sit in the stereo field and how loud: the pan law, SCATTER's placement of
//! POLY's voices at full SPREAD and DOUBLE's pairs (the CA-74's R27, R28, R30 and R41), INNER's
//! band on either side (K7), which leaves EDGES nothing of its own (dropped with it), the glide
//! a voice's place follows its controls with, and the trims of voices summed (R25, R28).

/// The pan law: at `pan` (0 left, 0.5 centre, 1 right) each side's gain, of constant power
/// (a voice as loud wherever it sits, so the centre does not outweigh the sides), both
/// whole at the centre as with SPREAD at 0: √2 cos and √2 sin of a quarter turn's `pan` (the
/// CA-74's R27).
pub fn pan_gains(pan: f64) -> (f32, f32) {
    let a = pan.clamp(0.0, 1.0) * std::f64::consts::FRAC_PI_2;
    (
        (std::f64::consts::SQRT_2 * a.cos()) as f32,
        (std::f64::consts::SQRT_2 * a.sin()) as f32,
    )
}

/// Where SCATTER puts the voices POLY plays at full SPREAD, the player's choice of two (the
/// CA-74's R30; EDGES, a third, dropped with INNER, K7); each voice as loud wherever it sits
/// ([`pan_gains`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    /// Evenly spaced from edge to edge, however many voices play: taken left and right in
    /// turn, each side's from its edge, then halfway in, then the rest from the outside
    /// inwards; with an odd number the last in the centre. The first two voices (a pair, a
    /// chord's first notes) are at the edges, and the field is filled without a hole.
    #[default]
    Even,
    /// From the centre outwards, by the golden ratio: the first voice in the centre and the
    /// next spread evenly about it, however many play: the narrowest.
    Centre,
}

/// The most voices placed apart.
const MOST: usize = 10;

/// How far out (0 to 1) DOUBLE's pair of one voice alone (POLY and UNISON off) sits at full
/// SPREAD, for [`pair_at`] and [`pair_gains`]: all the way, whatever the placement, since it has
/// no place among others (the CA-74's R41).
pub const ALONE: f64 = 1.0;

impl Placement {
    /// Where voice `k` of `n` (VOICES) sits at full SPREAD, -1 (left) to 1 (right). A voice
    /// past `n` (one letting go after VOICES was lowered) takes the place of `k` less `n`.
    pub fn place(self, k: usize, n: usize) -> f64 {
        let n = n.clamp(2, MOST);
        let k = k % n;
        match self {
            Placement::Centre => 2.0 * (0.5 + k as f64 * 0.618_033_988_749_894_8).fract() - 1.0,
            Placement::Even => {
                let side = n / 2;
                if k >= 2 * side {
                    return 0.0;
                }
                // The `i`th place taken on its side, `j` places in from its edge.
                let i = k / 2;
                let middle = side / 2;
                let j = match i {
                    0 => 0,
                    1 => middle,
                    _ if i <= middle => i - 1,
                    _ => i,
                };
                let at = 1.0 - 2.0 * j as f64 / (n - 1) as f64;
                if k.is_multiple_of(2) { -at } else { at }
            }
        }
    }

    /// How far out to either side DOUBLE's pair of voice `k` of `n` sits at full SPREAD, 0 to 1
    /// (the CA-74's R41): as far out as the voice's own place, so each pair stays about the
    /// centre (a voice placed in the centre, its pair there too, until INNER opens it). Every
    /// pair at the edges, as EDGES had them, is INNER at 100 % (K7).
    pub fn pair(self, k: usize, n: usize) -> f64 {
        self.place(k, n).abs()
    }

    /// The side voice `k` of `n` sits on, -1 (left) or 1 (right): its place's; a voice placed
    /// in the centre (EVEN's last of an odd number, CENTER's first), the side its turn falls on,
    /// as EVEN takes them, the left first, so that INNER clears the centre of every voice (K7).
    pub fn side(self, k: usize, n: usize) -> f64 {
        let place = self.place(k, n);
        if place != 0.0 {
            place.signum()
        } else if (k % n.clamp(2, MOST)).is_multiple_of(2) {
            -1.0
        } else {
            1.0
        }
    }

    /// Its index (0..2), as a voices' mix carries it to the worker threads, and back (a
    /// plug-in maps its own parameter's index).
    pub fn index(self) -> usize {
        match self {
            Placement::Even => 0,
            Placement::Centre => 1,
        }
    }

    pub fn from_index(i: usize) -> Placement {
        match i {
            1 => Placement::Centre,
            _ => Placement::Even,
        }
    }
}

/// How far out on its side a voice sits by `inner` (0..1) of INNER, a share of SPREAD's way out,
/// for `out` (0 centre to 1 edge) where its placement puts it: the side's band from INNER's
/// share of the way out to SPREAD's edge, the voice as far across it as `out` (K7). At INNER 0
/// the band reaches the centre, `out` itself, to the bit, as before INNER; at 1 every voice at
/// the edge.
pub fn band(inner: f64, out: f64) -> f64 {
    inner + (1.0 - inner) * out
}

/// Where voice `k` of `n` sits, -1 (left) to 1 (right), as `placement` has it, by `spread`
/// (0..1) of SPREAD, in its side's band by `inner` (0..1) of INNER ([`band`],
/// [`Placement::side`]): where [`voice_gains`] puts it, for a display to draw it there.
pub fn voice_at(spread: f64, inner: f64, placement: Placement, k: usize, n: usize) -> f64 {
    spread * placement.side(k, n) * band(inner, placement.place(k, n).abs())
}

/// How far out to either side DOUBLE's pair `out` (0 to 1, [`Placement::pair`], or [`ALONE`] for
/// a voice alone) sits by `spread` and `inner` (0..1): where [`pair_gains`] puts it, for a
/// display to draw it there.
pub fn pair_at(spread: f64, inner: f64, out: f64) -> f64 {
    spread * band(inner, out)
}

/// The gains (left, right) that put voice `k` of `n` in its place, as `placement` has it, by
/// `spread` (0..1) of SPREAD, in its side's band by `inner` (0..1) of INNER ([`voice_at`],
/// [`pan_gains`]; both whole at SPREAD 0, whatever INNER).
pub fn voice_gains(
    spread: f64,
    inner: f64,
    placement: Placement,
    k: usize,
    n: usize,
) -> (f32, f32) {
    if spread > 0.0 {
        pan_gains(0.5 + 0.5 * voice_at(spread, inner, placement, k, n))
    } else {
        (1.0, 1.0)
    }
}

/// DOUBLE's pair `out` (0 to 1, [`Placement::pair`] or [`ALONE`]) by `spread` (0..1), in each
/// side's band by `inner` (0..1, [`pair_at`]): the gains (left, right) of the voice, half the
/// detune flat, that far to the right, and of its twin, half sharp, as far to the left (the
/// CA-74's R28 and R41; K7).
pub fn pair_gains(spread: f64, inner: f64, out: f64) -> [(f32, f32); 2] {
    let at = pair_at(spread, inner, out);
    [pan_gains(0.5 + 0.5 * at), pan_gains(0.5 - 0.5 * at)]
}

/// The detune between DOUBLE's two voices of a note at its full amount, cents (the CA-74's R28).
pub const DOUBLE_CENTS: f64 = 20.0;

/// DOUBLE's trim on the output: a note's two voices, each its own, add in power, so it is
/// turned down by two's square root and sounds about as loud as one voice a note (R28).
pub const DOUBLE_TRIM: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// UNISON's trim on the output for `voices` voices on every key: not in phase, they add in
/// power, so it is turned down by their number's square root and sounds about as loud as one
/// voice (R25).
pub fn unison_trim(voices: usize) -> f64 {
    1.0 / (voices as f64).sqrt()
}
