//! Cost a voice of each kind, as a share of one core in real time at 44.1 kHz.
use ca72::light::Light;
use ca72::voice::{Panel, Quality, Waveform};
use std::time::Instant;

const RATE: f64 = 44_100.0;

fn fat(mut p: Panel) -> Panel {
    for o in p.osc.iter_mut() {
        o.on = true;
        o.waveform = Waveform::Sawtooth;
    }
    p.emphasis = 0.7;
    p.cutoff = 0.6;
    p.contour_amount = 0.7;
    p.noise_on = true;
    p.filter_mod = true;
    p.osc_mod = true;
    p.mod_wheel = 0.3;
    p
}

fn run(name: &str, p: Panel, circuit: bool) {
    let t0 = Instant::now();
    let mut l = Light::new(RATE);
    l.set_panel(&p);
    if circuit {
        l.make_circuit_filter();
        l.use_circuit_filter(true);
    }
    let made = t0.elapsed().as_secs_f64();
    let n = (RATE * 5.0) as usize;
    let mut sum = 0.0;
    let t = Instant::now();
    for i in 0..n {
        if i % 22050 == 0 {
            l.note(48 + ((i / 22050) % 12) as i32, true);
        }
        if i % 22050 == 15000 {
            l.note(48 + ((i / 22050) % 12) as i32, false);
        }
        sum += l.tick(0.0).abs();
    }
    let us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!(
        "{name:24} {us:6.2} us/sample {:6.1}% of a core  (made in {:.1} ms, chk {sum:.2})",
        us * RATE / 1e4,
        made * 1e3
    );
}

fn main() {
    let mut p = Panel::default();
    p.quality = Quality::Potato;
    run("LO, 1 osc", p, false);
    run("LO, 3 osc fat", fat(p), false);
    run("HI filter, 1 osc", p, true);
    run("HI filter, 3 osc fat", fat(p), true);
}
