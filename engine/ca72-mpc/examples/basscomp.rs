//! BASS COMP: the level below the cutoff (a sawtooth's lowest harmonics) as EMPHASIS rises.
use ca72::light::Light;
use ca72::voice::{Panel, Quality};
fn low_db(p: Panel, circuit: bool, comp: bool) -> f64 {
    let mut l = Light::new(44100.0);
    l.set_panel(&p);
    if circuit {
        l.make_circuit_filter();
        l.use_circuit_filter(true);
    }
    l.bass_comp = comp;
    l.note(36, true);
    let (mut s, mut y1) = (0.0, 0.0);
    let n = 44100;
    for i in 0..2 * n {
        let y = l.tick(0.0);
        // (A one-pole low-pass at about 150 Hz: the bass.)
        y1 += (y - y1) * 0.021;
        if i >= n {
            s += y1 * y1;
        }
    }
    10.0 * (s / n as f64).log10()
}
fn main() {
    for circuit in [false, true] {
        for e in [0.0, 0.5, 0.8, 0.95] {
            let mut p = Panel::default();
            p.quality = Quality::Potato;
            p.cutoff = 0.45;
            p.emphasis = e;
            p.contour_amount = 0.0;
            p.loudness_contour.sustain = 1.0;
            println!(
                "{} emphasis {e}: bass {:6.1} dB, with BASS COMP {:6.1} dB",
                if circuit { "HI" } else { "LO" },
                low_db(p, circuit, false),
                low_db(p, circuit, true)
            );
        }
    }
}
