//! The spliced voice (light + the circuit's filter) against the circuit's voice and the light
//! one: level, third-octave spectrum and a self-oscillating filter's pitch, patch by patch.
use ca72::light::Light;
use ca72::voice::{Panel, Quality, Voice, Waveform};

const RATE: f64 = 44_100.0;
const N: usize = 1 << 15;

fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let a = -2.0 * std::f64::consts::PI / len as f64;
        for i in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (c, s) = ((a * k as f64).cos(), (a * k as f64).sin());
                let (xr, xi) = (re[i + k + len / 2], im[i + k + len / 2]);
                let (tr, ti) = (xr * c - xi * s, xr * s + xi * c);
                re[i + k + len / 2] = re[i + k] - tr;
                im[i + k + len / 2] = im[i + k] - ti;
                re[i + k] += tr;
                im[i + k] += ti;
            }
        }
        len <<= 1;
    }
}

/// Power spectrum (Hann) of the last N samples.
fn spectrum(x: &[f64]) -> Vec<f64> {
    let x = &x[x.len() - N..];
    let mut re: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(i, v)| v * (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / N as f64).cos()))
        .collect();
    let mut im = vec![0.0; N];
    fft(&mut re, &mut im);
    (0..N / 2).map(|k| re[k] * re[k] + im[k] * im[k]).collect()
}

fn bands(p: &[f64]) -> Vec<f64> {
    let mut out = Vec::new();
    let mut f = 50.0;
    while f < 16000.0 {
        let (lo, hi) = (f / 2f64.powf(1.0 / 6.0), f * 2f64.powf(1.0 / 6.0));
        let (a, b) = (
            (lo * N as f64 / RATE) as usize,
            (hi * N as f64 / RATE) as usize,
        );
        out.push(10.0 * (p[a..b.max(a + 1)].iter().sum::<f64>() + 1e-30).log10());
        f *= 2f64.powf(1.0 / 3.0);
    }
    out
}

fn rms_db(x: &[f64]) -> f64 {
    let x = &x[x.len() - N..];
    10.0 * (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64 + 1e-30).log10()
}

fn peak_hz(p: &[f64]) -> f64 {
    let k = (2..p.len())
        .max_by(|&a, &b| p[a].partial_cmp(&p[b]).unwrap())
        .unwrap();
    // (Parabolic interpolation on the log magnitude.)
    let (a, b, c) = (p[k - 1].ln(), p[k].ln(), p[k + 1].ln());
    (k as f64 + 0.5 * (a - c) / (a - 2.0 * b + c)) * RATE / N as f64
}

fn render(p: Panel, key: i32, secs: f64) -> [Vec<f64>; 3] {
    let n = (secs * RATE) as usize;
    let mut v = Voice::new(RATE, p);
    let mut h = Light::new(RATE);
    h.set_panel(&p);
    h.make_circuit_filter();
    h.use_circuit_filter(true);
    let mut l = Light::new(RATE);
    l.set_panel(&p);
    v.note(key, true);
    h.note(key, true);
    l.note(key, true);
    let mut out = [
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    ];
    for _ in 0..n {
        out[0].push(v.tick());
        out[1].push(h.tick(0.0));
        out[2].push(l.tick(0.0));
    }
    out
}

fn base() -> Panel {
    let mut p = Panel::default();
    p.quality = Quality::Potato;
    p.loudness_contour.sustain = 1.0;
    p.filter_contour.sustain = 1.0;
    p.contour_amount = 0.0;
    p
}

fn main() {
    let mut cases: Vec<(&str, Panel, i32)> = Vec::new();
    let mut p = base();
    p.cutoff = 0.5;
    cases.push(("saw, cutoff 5", p, 48));
    let mut p = base();
    p.cutoff = 0.4;
    p.emphasis = 0.8;
    cases.push(("saw, cutoff 4, emphasis 8", p, 48));
    let mut p = base();
    p.cutoff = 0.3;
    p.emphasis = 0.6;
    p.osc[0].waveform = Waveform::Square;
    cases.push(("square, cutoff 3, emphasis 6", p, 36));
    let mut p = base();
    p.cutoff = 0.45;
    p.emphasis = 0.7;
    for o in p.osc.iter_mut() {
        o.on = true;
        o.volume = 1.0;
    }
    p.osc[1].freq = 0.52;
    cases.push(("3 osc full volume, emphasis 7", p, 41));
    let mut p = base();
    p.cutoff = 0.6;
    p.filter_hi = true;
    p.emphasis = 0.3;
    cases.push(("FILTER MODE HI", p, 48));
    let mut p = base();
    p.cutoff = 0.35;
    p.osc[0].waveform = Waveform::NarrowRectangle;
    p.contour_amount = 0.6;
    p.filter_contour.sustain = 0.3;
    p.filter_contour.decay = 0.3;
    cases.push(("narrow, contour 6", p, 60));
    let mut p = base();
    p.cutoff = 0.2;
    p.noise_on = true;
    p.osc[0].on = false;
    p.emphasis = 0.5;
    cases.push(("noise, cutoff 2", p, 60));
    println!(
        "{:34} {:>22} {:>26}",
        "", "level dB (C / H / L)", "band error dB (H / L)"
    );
    for (name, p, key) in &cases {
        let r = render(*p, *key, 1.5);
        let s: Vec<Vec<f64>> = r.iter().map(|x| bands(&spectrum(x))).collect();
        let err = |b: &Vec<f64>| {
            let top = s[0].iter().cloned().fold(f64::MIN, f64::max);
            let used: Vec<f64> = s[0]
                .iter()
                .zip(b)
                .filter(|(c, _)| **c > top - 50.0)
                .map(|(c, x)| (c - x).abs())
                .collect();
            used.iter().sum::<f64>() / used.len() as f64
        };
        println!(
            "{name:34} {:7.2} {:7.2} {:7.2}   {:10.2} {:10.2}",
            rms_db(&r[0]),
            rms_db(&r[1]),
            rms_db(&r[2]),
            err(&s[1]),
            err(&s[2])
        );
    }
    println!("== self-oscillation (oscillators off, emphasis 10): pitch Hz C / H / L");
    for (cut, key) in [(0.3, 48), (0.5, 48), (0.5, 72), (0.7, 60)] {
        let mut p = base();
        p.osc[0].on = false;
        p.emphasis = 1.0;
        p.cutoff = cut;
        let r = render(p, key, 2.0);
        let f: Vec<f64> = r.iter().map(|x| peak_hz(&spectrum(x))).collect();
        let cents = |a: f64| 1200.0 * (a / f[0]).log2();
        println!(
            "cutoff {cut} key {key}: {:8.2} {:8.2} {:8.2}  (H {:+.0} c, L {:+.0} c)  level {:.1} {:.1} {:.1}",
            f[0],
            f[1],
            f[2],
            cents(f[1]),
            cents(f[2]),
            rms_db(&r[0]),
            rms_db(&r[1]),
            rms_db(&r[2])
        );
    }
}
