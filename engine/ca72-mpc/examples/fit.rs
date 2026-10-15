//! Fits the constants that splice the circuit's filter into the light voice: the bus's
//! current per light bus unit (and each waveform's DC), the ladder current against the light
//! voice's nominal cutoff, and the filter's output volts per light unit.
use ca72::light::Light;
use ca72::voice::{Panel, Quality, Voice, Waveform};

const RATE: f64 = 44_100.0;
const WAVES: [Waveform; 6] = [
    Waveform::Triangle,
    Waveform::SharkTooth,
    Waveform::Sawtooth,
    Waveform::Square,
    Waveform::WideRectangle,
    Waveform::NarrowRectangle,
];

fn stats(x: &[f64]) -> (f64, f64) {
    let m = x.iter().sum::<f64>() / x.len() as f64;
    let v = x.iter().map(|a| (a - m) * (a - m)).sum::<f64>() / x.len() as f64;
    (m, v.sqrt())
}

fn panel(w: Waveform, vol: f64, cutoff: f64) -> Panel {
    let mut p = Panel::default();
    p.quality = Quality::Potato;
    p.osc[0].waveform = w;
    p.osc[0].volume = vol;
    p.cutoff = cutoff;
    p.contour_amount = 0.0;
    p.loudness_contour.sustain = 1.0;
    p.filter_contour.sustain = 1.0;
    p
}

/// (circuit i_bus, i0, y), (light bus, l, y) over a second after a settled half second.
fn run(p: Panel, key: i32) -> (Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let mut v = Voice::new(RATE, p);
    let mut l = Light::new(RATE);
    l.set_panel(&p);
    v.note(key, true);
    l.note(key, true);
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for i in 0..(RATE * 1.5) as usize {
        v.tick();
        l.tick(0.0);
        if i >= (RATE * 0.5) as usize {
            let (_, ib, y) = v.probe();
            a.push([ib, v.filter_probe().0, y]);
            b.push([f64::from(l.probe_bus), l.probe_l, f64::from(l.probe_y)]);
        }
    }
    (a, b)
}

fn col(v: &[[f64; 3]], i: usize) -> Vec<f64> {
    v.iter().map(|x| x[i]).collect()
}

fn line(pts: &[(f64, f64)]) -> (f64, f64, f64) {
    let n = pts.len() as f64;
    let (mx, my) = (
        pts.iter().map(|p| p.0).sum::<f64>() / n,
        pts.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
    let a = sxy / sxx;
    let b = my - a * mx;
    let worst = pts
        .iter()
        .map(|p| (p.1 - (a * p.0 + b)).abs())
        .fold(0.0, f64::max);
    (a, b, worst)
}

fn control_fit() {
    println!("== keyboard volts vs light pitch");
    let mut kp = Vec::new();
    for key in [29, 36, 41, 48, 60, 72, 84, 96] {
        let p = panel(Waveform::Sawtooth, 0.3, 0.5);
        let mut v = Voice::new(RATE, p);
        let mut l = Light::new(RATE);
        l.set_panel(&p);
        v.note(key, true);
        l.note(key, true);
        for _ in 0..(RATE * 0.3) as usize {
            v.tick();
            l.tick(0.0);
        }
        println!(
            "key {key}: v_kbd {:.5} light pitch {:.4}",
            v.probe().0,
            l.probe_pitch
        );
        kp.push((l.probe_pitch, v.probe().0));
    }
    let (a, b, w) = line(&kp);
    println!("fit: v_kbd = {a:.6} * pitch + {b:.6}  worst {w:.2e} V");
    println!("== filter contour volts, circuit vs light, over attack/decay/release");
    let mut p = panel(Waveform::Sawtooth, 0.3, 0.5);
    p.filter_contour.attack = 0.3;
    p.filter_contour.decay = 0.4;
    p.filter_contour.sustain = 0.4;
    let mut v = Voice::new(RATE, p);
    let mut l = Light::new(RATE);
    l.set_panel(&p);
    let mut ep = Vec::new();
    for i in 0..(RATE * 3.0) as usize {
        if i == 100 {
            v.note(60, true);
            l.note(60, true);
        }
        if i == (RATE * 2.0) as usize {
            v.note(60, false);
            l.note(60, false);
        }
        v.tick();
        l.tick(0.0);
        if i % 2205 == 0 {
            println!(
                "t {:.2}: circuit {:.4} light {:.4}",
                i as f64 / RATE,
                v.probe_contours().0,
                l.probe_env
            );
        }
        ep.push((l.probe_env, v.probe_contours().0));
    }
    let (a, b, w) = line(&ep);
    println!("fit: env = {a:.5} * light + {b:.5}  worst {w:.3} V");
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("control") {
        control_fit();
        return;
    }
    println!("== bus: circuit A per light unit and DC (A), filter open");
    for w in WAVES {
        for vol in [1.0, 0.5] {
            let (c, l) = run(panel(w, vol, 1.0), 60);
            let (cm, cs) = stats(&col(&c, 0));
            let (lm, ls) = stats(&col(&l, 0));
            let a = cs / ls;
            let mut l2 = Light::new(RATE);
            l2.set_panel(&panel(w, vol, 1.0));
            let g = f64::from(l2.bus_gains()[0]);
            println!(
                "{w:?} vol {vol}: A {a:.4e}  gain {g:.4}  K (DC per gain, light units) {:.4}",
                (cm - 5.5453e-5 * lm) / (5.5453e-5 * g)
            );
        }
    }
    if std::env::args().nth(1).as_deref() == Some("bus") {
        return;
    }
    println!("== i0 vs nominal cutoff (KEYBOARD CONTROL 1)");
    let mut pts = Vec::new();
    for k in 0..=10 {
        for key in [36, 60, 84] {
            let cut = k as f64 / 10.0;
            let (c, l) = run(panel(Waveform::Sawtooth, 0.3, cut), key);
            let i0 = stats(&col(&c, 1)).0;
            let ll = stats(&col(&l, 1)).0;
            pts.push((ll, i0.log2()));
            println!(
                "cut {cut:.1} key {key}: L {ll:.4}  log2 i0 {:.4}",
                i0.log2()
            );
        }
    }
    let n = pts.len() as f64;
    let (mx, my) = (
        pts.iter().map(|p| p.0).sum::<f64>() / n,
        pts.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
    let alpha = sxy / sxx;
    let beta = my - alpha * mx;
    let worst = pts
        .iter()
        .map(|p| (p.1 - (alpha * p.0 + beta)).abs())
        .fold(0.0, f64::max);
    println!("fit: log2 i0 = {alpha:.5} * L + {beta:.5}   worst residual {worst:.4} oct");
    println!("== filter output volts per light unit (open, emphasis 0)");
    for (w, vol) in [
        (Waveform::Sawtooth, 0.3),
        (Waveform::Triangle, 0.3),
        (Waveform::Sawtooth, 1.0),
    ] {
        let (c, l) = run(panel(w, vol, 1.0), 60);
        let (_, cs) = stats(&col(&c, 2));
        let (_, ls) = stats(&col(&l, 2));
        println!("{w:?} vol {vol}: B (light per volt) {:.5}", ls / cs);
    }
}
