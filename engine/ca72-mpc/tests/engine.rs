use ca72_mpc::{Engine, params as p};

fn rms(e: &mut Engine, blocks: usize) -> f64 {
    let mut out = [0i16; 256];
    let mut sum = 0.0;
    for _ in 0..blocks {
        e.render(&mut out);
        sum += out
            .iter()
            .map(|&x| f64::from(x) * f64::from(x))
            .sum::<f64>();
    }
    (sum / (blocks * 256) as f64).sqrt() / 32768.0
}

#[test]
fn a_note_sounds_at_the_defaults() {
    let mut e = Engine::new();
    rms(&mut e, 4);
    e.midi(&[0x90, 60, 100]);
    let r = rms(&mut e, 100);
    assert!(r > 0.01, "rms {r}");
    e.midi(&[0x80, 60, 0]);
    rms(&mut e, 800);
    let r = rms(&mut e, 10);
    assert!(r < 1e-4, "tail {r}");
}

#[test]
fn poly_plays_a_chord_and_hi_plays_the_circuit_filter() {
    let mut e = Engine::new();
    e.set("mode", "1");
    rms(&mut e, 2);
    for k in [48, 52, 55] {
        e.midi(&[0x90, k, 100]);
    }
    let chord = rms(&mut e, 100);
    assert!(chord > 0.02, "chord {chord}");
    e.set("quality", "1");
    // (The circuit filter's thread: up to some seconds on a slow machine.)
    let t = std::time::Instant::now();
    loop {
        rms(&mut e, 1);
        if e.get("hi_ready").as_deref() == Some("1") || t.elapsed().as_secs() > 60 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(e.get("hi_ready").as_deref(), Some("1"));
    e.midi(&[0x90, 48, 100]);
    let hi = rms(&mut e, 100);
    assert!(hi > 0.01, "hi {hi}");
}

#[test]
fn state_round_trips() {
    let mut a = Engine::new();
    a.set("cutoff", "-2.5");
    a.set("osc2_waveform", "4");
    a.set("voices", "7");
    let s = a.get("state").unwrap();
    let mut b = Engine::new();
    b.set("state", &s);
    for k in p::KEYS {
        assert_eq!(a.get(k), b.get(k), "{k}");
    }
}

#[test]
fn idle_voices_cost_nothing_and_wake() {
    let mut e = Engine::new();
    rms(&mut e, 200);
    assert!(e.get("awake").as_deref() == Some("0"));
    e.midi(&[0x90, 60, 100]);
    assert!(rms(&mut e, 50) > 0.01);
}

#[test]
fn unison_and_entropy_play() {
    let mut e = Engine::new();
    e.set("mode", "2");
    e.set("voices", "6");
    e.set("entropy", "60");
    rms(&mut e, 2);
    e.midi(&[0x90, 45, 100]);
    let r = rms(&mut e, 100);
    assert!(r > 0.01, "unison {r}");
    assert_eq!(e.get("awake").as_deref(), Some("6"));
}

#[test]
fn every_factory_preset_plays() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vst/presets/factory.json"
    ))
    .unwrap();
    // [[name, {key: value}], ...]: the .xpl files' states, as `key=value;...`.
    let mut n = 0;
    for chunk in text.split("\n ],\n").chain(std::iter::once("")) {
        let Some(start) = chunk.find('"') else {
            continue;
        };
        let name = chunk[start + 1..].split('"').next().unwrap().to_string();
        let state: Vec<String> = chunk
            .lines()
            .filter_map(|l| l.trim().trim_end_matches(',').split_once("\": "))
            .map(|(k, v)| format!("{}={}", k.trim().trim_start_matches('"'), v))
            .collect();
        if state.is_empty() {
            continue;
        }
        let mut e = Engine::new();
        e.set("state", &state.join(";"));
        rms(&mut e, 2);
        e.midi(&[0x90, 48, 100]);
        let r = rms(&mut e, 150);
        println!("{name}: {r:.4}");
        assert!(r > 0.002, "{name} silent ({r})");
        n += 1;
    }
    assert_eq!(n, 19);
}

#[test]
fn stereo_spreads_and_doubles() {
    let side = |e: &mut Engine| {
        let mut out = [0i16; 256];
        let (mut l, mut r) = (0.0, 0.0);
        for _ in 0..100 {
            e.render(&mut out);
            for f in out.chunks(2) {
                l += f64::from(f[0]).powi(2);
                r += f64::from(f[1]).powi(2);
            }
        }
        (l.sqrt(), r.sqrt())
    };
    // POLY at full WIDTH: the first voice (EVEN) at the left edge.
    let mut e = Engine::new();
    e.set("mode", "1");
    e.set("spread", "100");
    rms(&mut e, 2);
    e.midi(&[0x90, 48, 100]);
    let (l, r) = side(&mut e);
    assert!(l > 10.0 * r, "poly voice 1 left: {l} {r}");
    // DOUBLE: both sides, two voices awake.
    let mut e = Engine::new();
    e.set("played_as", "1");
    e.set("spread", "100");
    rms(&mut e, 2);
    e.midi(&[0x90, 48, 100]);
    let (l, r) = side(&mut e);
    assert!(l > 0.3 * r && r > 0.3 * l, "double both sides: {l} {r}");
}

#[test]
fn the_field_shows_sounding_voices() {
    let mut e = Engine::new();
    e.set("mode", "1");
    e.set("spread", "100");
    rms(&mut e, 2);
    assert_eq!(e.get("dot_1").as_deref(), Some("0.0000"));
    let rev = e.get("display_rev").unwrap();
    e.midi(&[0x90, 48, 100]);
    e.midi(&[0x90, 55, 100]);
    rms(&mut e, 20);
    assert_ne!(e.get("display_rev").unwrap(), rev);
    // EVEN: the first two voices at the edges.
    let a: f64 = e.get("dot_1").unwrap().parse().unwrap();
    let b: f64 = e.get("dot_2").unwrap().parse().unwrap();
    assert!(a < 0.05 && b > 0.95, "{a} {b}");
}
