//! The CA-72's voices as an MPC OS plug-in engine: the C functions `vst/engine.c` hands to
//! mpc-vst-plugins' VST2 wrapper (44.1 kHz, interleaved int16 stereo, 128-frame blocks;
//! string parameters in the panel's dial units, `vst/params.json`).
//!
//! QUALITY LO plays the light model (upstream's Potato mode): MODE's MONO one voice, POLY a
//! voice a note and UNISON every note on all VOICES (up to ten), each voice with its own parts
//! under ENTROPY (upstream's `character.rs`).
//! QUALITY HI plays one voice, the light model with the circuit's filter spliced in
//! (`ca72::light::splice`). The circuit filter's tables take seconds to build the first time,
//! so a thread builds it when the engine is made and the audio thread takes it once ready;
//! until then HI plays the light filter.

use ca72::light::Light;
use ca72::modulation::{PITCH_WHEEL_SEMITONES, midi_wheel};
use ca72::tuning::{FREQ_CENTRE, Range};
use ca72::voice::{ContourKnobs, OscPanel, Panel, Priority, Quality, Waveform, audio_taper};
use std::ffi::{CStr, c_char};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

mod character;
pub mod params;
mod stereo;
use character::Character;
use params as p;
use stereo::{ALONE, DOUBLE_TRIM, Placement, pair_at, pair_gains, voice_at, voice_gains};

pub const RATE: f64 = 44_100.0;
const MAX_VOICES: usize = 10;
/// AUTO GAIN's correction (dB) at DRIVE 0, 6, 12, 18 and 24 dB: upstream's presets' average
/// (`drive::AVERAGE`), which its plug-in uses until it has measured the sound in its window.
const AUTO_DB: [f64; 5] = [0.0, -4.74, -8.43, -10.95, -12.42];

/// AUTO GAIN's correction at `drive` dB, straight between [`AUTO_DB`]'s points.
fn auto_db(drive: f64) -> f64 {
    let x = (drive.clamp(0.0, 24.0) / 6.0).min(3.999_999);
    let i = x as usize;
    AUTO_DB[i] + (AUTO_DB[i + 1] - AUTO_DB[i]) * (x - i as f64)
}

/// DOUBLE's detune between a note's two voices at DETUNE 100 %, cents (upstream's
/// `engine::DOUBLE_CENTS`).
const DOUBLE_CENTS: f64 = 100.0;

/// The time constant with which a voice's place follows SPREAD and the rest, s (upstream's
/// `place::GLIDE`): a step glides instead of clicking.
const PLACE_GLIDE: f64 = 0.010;

/// Blocks a released voice stays silent and settled before it stops being computed.
const IDLE_BLOCKS: u32 = 32;

#[derive(Debug)]
struct Slot {
    v: Light,
    key: Option<u8>,
    /// The key it played last (a key struck again takes its voice back).
    key_last: Option<u8>,
    /// When its key went down or up (the engine's note clock), for choosing a voice.
    since: u64,
    idle: bool,
    quiet: u32,
    /// The panel has changed since it last took it.
    stale: bool,
    /// Its own parts and movements (ENTROPY).
    ch: Character,
    /// DOUBLE's second voice of the note, with parts of its own; it takes every key the
    /// voice does, and plays while DOUBLE is on.
    twin: Light,
    ch2: Character,
    /// Where the voice and its twin sit: the gains (left, right) aimed at and as they glide.
    aim: [(f64, f64); 2],
    now: [(f64, f64); 2],
    /// Its loudest sample in the last block (for the stereo field's dots).
    peak: f64,
}

/// What the panel and the plug-in's own controls set in a voice.
#[derive(Debug, Clone, Copy)]
struct Dress {
    panel: Panel,
    entropy: f64,
    feedback: f64,
    drive: f64,
    /// DOUBLE's half detune, cents: the voice that much flat, its twin sharp (0 without).
    half: f64,
    bass_comp: bool,
}

impl Dress {
    fn on(&self, s: &mut Slot) {
        s.v.set_panel(&entropy_panel(&s.ch, self.panel, self.entropy));
        s.twin
            .set_panel(&entropy_panel(&s.ch2, self.panel, self.entropy));
        if self.entropy == 0.0 {
            s.v.set_offsets([-self.half; 3], 0.0);
            s.twin.set_offsets([self.half; 3], 0.0);
        }
        for v in [&mut s.v, &mut s.twin] {
            v.bass_comp = self.bass_comp;
            v.feedback = self.feedback;
            v.set_drive(self.drive);
        }
        s.stale = false;
    }
}

#[derive(Debug)]
pub struct Engine {
    vals: [f64; p::COUNT],
    dirty: bool,
    dress: Dress,
    slots: Vec<Slot>,
    /// QUALITY, MODE (0 MONO, 1 POLY, 2 UNISON) and VOICES as last applied (changing any
    /// stops the notes).
    mode: (bool, usize, usize),
    hi_cell: Arc<Mutex<Option<Box<Light>>>>,
    builder: Option<JoinHandle<()>>,
    hi_ready: bool,
    /// The voice the circuit filter came in (freed when the engine is, not on the audio thread).
    spare: Option<Box<Light>>,
    bend: f64,
    cc_mod: f64,
    clock: u64,
    gain: f64,
    /// DOUBLE on (and playable: not in HI), and MONO's turn among VOICES's places (SCATTER).
    doubled: bool,
    turn: usize,
    glide: f64,
    /// The stereo field's dots (DOT 1 to 10): 0 none, else where a sounding voice sits, and the
    /// count the wrapper polls to report them (HAS_DISPLAY_REV).
    dots: [f64; DOTS],
    rev: u64,
}

const DOTS: usize = 10;

fn range(i: f64) -> Range {
    [
        Range::Lo,
        Range::R32,
        Range::R16,
        Range::R8,
        Range::R4,
        Range::R2,
    ][(i as usize).min(5)]
}

fn wave(i: f64, osc3: bool) -> Waveform {
    let second = if osc3 {
        Waveform::ReverseSawtooth
    } else {
        Waveform::SharkTooth
    };
    [
        Waveform::Triangle,
        second,
        Waveform::Sawtooth,
        Waveform::Square,
        Waveform::WideRectangle,
        Waveform::NarrowRectangle,
    ][(i as usize).min(5)]
}

/// MAIN OUTPUT's VOLUME (5K audio taper into the next stage's 10K) and its switch: upstream's
/// `engine::output_gain`.
fn output_gain(volume: f64, on: bool) -> f64 {
    if !on {
        return 0.0;
    }
    let t = audio_taper(volume);
    let (top, bottom) = (5e3 * (1.0 - t), 5e3 * t);
    let load = bottom * 10e3 / (bottom + 10e3);
    if load <= 0.0 {
        0.0
    } else {
        load / (top + load)
    }
}

impl Engine {
    pub fn new() -> Engine {
        let mut slots = Vec::with_capacity(MAX_VOICES);
        for i in 0..MAX_VOICES {
            let mut v = Light::new(RATE);
            v.set_seed(0x0CA7_2000 + i as u64);
            if i > 0 {
                // (Voices playing together not in step with each other.)
                v.start_oscillators_at(i as f64 / MAX_VOICES as f64);
            }
            let mut twin = Light::new(RATE);
            twin.set_seed(0x0CA7_2100 + i as u64);
            twin.start_oscillators_at((i as f64 + 0.5) / MAX_VOICES as f64);
            slots.push(Slot {
                v,
                key: None,
                key_last: None,
                since: 0,
                idle: true,
                quiet: 0,
                stale: true,
                ch: Character::new(0x0CA7_2000, i, 3, 6),
                twin,
                ch2: Character::new(0x0CA7_2000, MAX_VOICES + i, 3, 6),
                aim: [(1.0, 1.0); 2],
                now: [(1.0, 1.0); 2],
                peak: 0.0,
            });
        }
        let hi_cell = Arc::new(Mutex::new(None));
        let cell = Arc::clone(&hi_cell);
        let builder = std::thread::Builder::new()
            .name("ca72-hi".into())
            .spawn(move || {
                let mut l = Light::new(RATE);
                l.make_circuit_filter();
                if let Ok(mut c) = cell.lock() {
                    *c = Some(Box::new(l));
                }
            })
            .ok();
        let mut e = Engine {
            vals: p::DEFAULTS,
            dirty: true,
            dress: Dress {
                panel: Panel::default(),
                entropy: 0.0,
                feedback: 0.0,
                drive: 1.0,
                half: 0.0,
                bass_comp: false,
            },
            slots,
            mode: (false, 0, 4),
            hi_cell,
            builder,
            hi_ready: false,
            spare: None,
            bend: 0.0,
            cc_mod: 0.0,
            clock: 0,
            gain: 1.0,
            doubled: false,
            turn: 0,
            glide: 1.0 - (-1.0 / (PLACE_GLIDE * RATE)).exp(),
            dots: [0.0; DOTS],
            rev: 0,
        };
        e.mode = e.wanted_mode();
        e
    }

    fn on(&self, i: usize) -> bool {
        self.vals[i] >= 0.5
    }

    fn hi(&self) -> bool {
        self.on(p::QUALITY)
    }

    fn wanted_mode(&self) -> (bool, usize, usize) {
        (
            self.hi(),
            self.vals[p::MODE].round() as usize,
            self.vals[p::VOICES].round() as usize,
        )
    }

    /// How many voices play: one, or POLY's or UNISON's VOICES in LO.
    fn playing(&self) -> usize {
        let (hi, mode, n) = self.mode;
        if hi || mode == 0 {
            1
        } else {
            n.clamp(2, MAX_VOICES)
        }
    }

    fn panel(&self) -> Panel {
        let v = &self.vals;
        let ten = |i: usize| v[i] / 10.0;
        let freq = |i: usize| (v[i] + 7.5) / 15.0;
        let osc =
            |r: usize, w: usize, on: usize, vol: usize, f: Option<usize>, three: bool| OscPanel {
                range: range(v[r]),
                waveform: wave(v[w], three),
                on: v[on] >= 0.5,
                volume: ten(vol),
                freq: f.map_or(FREQ_CENTRE, freq),
            };
        let share = (v[p::MIDI_BEND_RANGE] / PITCH_WHEEL_SEMITONES).clamp(0.0, 1.0);
        Panel {
            osc: [
                osc(
                    p::OSC1_RANGE,
                    p::OSC1_WAVEFORM,
                    p::OSC1_ON,
                    p::OSC1_VOLUME,
                    None,
                    false,
                ),
                osc(
                    p::OSC2_RANGE,
                    p::OSC2_WAVEFORM,
                    p::OSC2_ON,
                    p::OSC2_VOLUME,
                    Some(p::OSC2_FREQUENCY),
                    false,
                ),
                osc(
                    p::OSC3_RANGE,
                    p::OSC3_WAVEFORM,
                    p::OSC3_ON,
                    p::OSC3_VOLUME,
                    Some(p::OSC3_FREQUENCY),
                    true,
                ),
            ],
            osc3_control: self.on(p::OSC3_CONTROL),
            cutoff: (v[p::CUTOFF] + 5.0) / 10.0,
            emphasis: ten(p::EMPHASIS),
            contour_amount: ten(p::CONTOUR_AMOUNT),
            keyboard_control_1: self.on(p::KEYBOARD_CONTROL_1),
            keyboard_control_2: self.on(p::KEYBOARD_CONTROL_2),
            filter_contour: ContourKnobs {
                attack: ten(p::FILTER_ATTACK),
                decay: ten(p::FILTER_DECAY),
                sustain: ten(p::FILTER_SUSTAIN),
            },
            loudness_contour: ContourKnobs {
                attack: ten(p::LOUDNESS_ATTACK),
                decay: ten(p::LOUDNESS_DECAY),
                sustain: ten(p::LOUDNESS_SUSTAIN),
            },
            glide: ten(p::GLIDE),
            glide_on: self.on(p::GLIDE_ON),
            decay: self.on(p::DECAY_ON),
            noise_on: self.on(p::NOISE_ON),
            noise_volume: ten(p::NOISE_VOLUME),
            noise_pink: self.on(p::NOISE_TYPE),
            mod_mix: ten(p::MOD_MIX),
            osc_mod: self.on(p::OSC_MOD),
            filter_mod: self.on(p::FILTER_MOD),
            filter_hi: self.on(p::FILTER_MODE),
            pitch_wheel: (self.bend * share).clamp(-1.0, 1.0),
            mod_wheel: v[p::MOD_WHEEL].max(midi_wheel(self.cc_mod)),
            ext_volume: ten(p::EXT_VOLUME),
            ext_on: self.on(p::EXT_ON) || self.on(p::PATCH),
            a440: self.on(p::A440),
            tune: v[p::TUNE] / 2.5,
            quality: Quality::Potato,
            priority: match v[p::NOTE_PRIORITY].round() as usize {
                1 => Priority::Last,
                2 => Priority::High,
                _ => Priority::Low,
            },
            retrigger: self.on(p::RETRIGGER),
        }
    }

    fn all_rest(&mut self) {
        for s in &mut self.slots {
            s.v.rest();
            s.twin.rest();
            s.key = None;
            s.idle = true;
            s.quiet = 0;
        }
    }

    /// The panel and the plug-in's own controls into every voice (at a block's start).
    fn apply(&mut self) {
        self.dirty = false;
        let mode = self.wanted_mode();
        if mode != self.mode {
            self.mode = mode;
            self.all_rest();
        }
        let panel = self.panel();
        let entropy = (self.vals[p::ENTROPY] / 100.0).clamp(0.0, 1.0);
        // (The output cabled into EXTERNAL INPUT by PATCH: the amount is that input's VOLUME, as on the
        // instrument, so the cable itself carries all of it. FEEDBACK is no longer a control.)
        let feedback = if self.on(p::PATCH) {
            1.0
        } else {
            0.0
        };
        let drive = if self.vals[p::DRIVE] > 0.0 {
            10f64.powf(self.vals[p::DRIVE] / 20.0)
        } else {
            1.0
        };
        let hi = self.mode.0;
        self.doubled = self.on(p::PLAYED_AS) && !hi;
        let half = if self.doubled {
            0.5 * DOUBLE_CENTS * (self.vals[p::DOUBLE] / 100.0).max(1e-6)
        } else {
            0.0
        };
        self.dress = Dress {
            panel,
            entropy,
            feedback,
            drive,
            half,
            bass_comp: self.on(p::BASS_COMP),
        };
        // (A voice's panel costs some tens of microseconds on a Force: only the voices sounding
        // take it now, a Q-Link turning; the rest when their next key comes.)
        let n = self.playing();
        let d = self.dress;
        for (k, s) in self.slots.iter_mut().enumerate() {
            // (With no key down only A-440 sounds: it bypasses the VCA.)
            if panel.a440 && k < n {
                s.idle = false;
                s.quiet = 0;
            }
            if s.idle {
                s.stale = true;
            } else {
                d.on(s);
            }
        }
        self.slots[0].v.use_circuit_filter(hi);
        self.place(false);
        let unison = if !hi && self.mode.1 == 2 {
            stereo::unison_trim(self.playing())
        } else {
            1.0
        };
        self.gain = output_gain(self.vals[p::VOLUME] / 10.0, self.on(p::MAIN_OUTPUT))
            * 10f64.powf(self.vals[p::LEVEL].clamp(-30.0, 12.0) / 20.0)
            * unison
            * if self.doubled { DOUBLE_TRIM } else { 1.0 }
            * 10f64.powf(self.auto_trim() / 20.0);
    }

    /// Where each voice and its twin sit (the gains they glide to), as SPREAD, INNER,
    /// PLACEMENT and DOUBLE have them: POLY's and UNISON's voices each at their place among
    /// VOICES; MONO's at its turn's place (SCATTER), or with DOUBLE its pair all of SPREAD's way
    /// out (upstream's `engine::places`). `at_once`: no glide.
    fn place(&mut self, at_once: bool) {
        let v = &self.vals;
        let (spread, inner) = (v[p::SPREAD] / 100.0, v[p::INNER] / 100.0);
        let placement = Placement::from_index(v[p::PLACEMENT].round() as usize);
        let places = (v[p::VOICES].round() as usize).clamp(2, MAX_VOICES);
        let n = self.playing();
        let wide = |(l, r): (f32, f32)| (f64::from(l), f64::from(r));
        for (k, s) in self.slots.iter_mut().enumerate().take(n) {
            s.aim = if self.doubled {
                let out = if n == 1 {
                    ALONE
                } else {
                    placement.pair(k, places)
                };
                let [a, b] = pair_gains(spread, inner, out);
                [wide(a), wide(b)]
            } else {
                let at = if n == 1 { self.turn } else { k };
                [
                    wide(voice_gains(spread, inner, placement, at, places)),
                    (0.0, 0.0),
                ]
            };
            if at_once {
                s.now = s.aim;
            }
        }
    }

    /// AUTO GAIN's correction now, dB (0 off or without DRIVE).
    fn auto_trim(&self) -> f64 {
        if self.on(p::AUTO_GAIN) {
            auto_db(self.vals[p::DRIVE])
        } else {
            0.0
        }
    }

    fn take_hi(&mut self) {
        if self.hi_ready {
            return;
        }
        let built = match self.hi_cell.try_lock() {
            Ok(mut c) => c.take(),
            Err(_) => None,
        };
        if let Some(mut l) = built {
            self.slots[0].v.take_circuit_filter(&mut l);
            let hi = self.mode.0;
            self.slots[0].v.use_circuit_filter(hi);
            self.spare = Some(l);
            self.hi_ready = true;
        }
    }

    pub fn note(&mut self, key: u8, on: bool) {
        self.clock += 1;
        let n = self.playing();
        if n == 1 || self.mode.1 == 2 {
            // MONO, or UNISON: every voice takes the keys as the one instrument does.
            let d = self.dress;
            for s in &mut self.slots[..n] {
                if s.stale {
                    d.on(s);
                }
                s.v.note(i32::from(key), on);
                s.twin.note(i32::from(key), on);
                s.idle = false;
                s.quiet = 0;
            }
            return;
        }
        let clock = self.clock;
        let d = self.dress;
        let slots = &mut self.slots[..n];
        if !on {
            for s in slots.iter_mut().filter(|s| s.key == Some(key)) {
                s.v.note(i32::from(key), false);
                s.twin.note(i32::from(key), false);
                s.key = None;
                s.since = clock;
            }
            return;
        }
        // The voice that last played this key if free, else the free voice let go longest
        // ago, else the voice held longest.
        let pick = slots
            .iter()
            .position(|s| s.key.is_none() && s.key_last == Some(key))
            .or_else(|| {
                slots
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.key.is_none())
                    .min_by_key(|(_, s)| s.since)
                    .map(|(i, _)| i)
            })
            .unwrap_or_else(|| {
                slots
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, s)| s.since)
                    .map_or(0, |(i, _)| i)
            });
        let s = &mut slots[pick];
        if s.stale {
            d.on(s);
        }
        if let Some(old) = s.key {
            s.v.note(i32::from(old), false);
            s.twin.note(i32::from(old), false);
        }
        s.v.note(i32::from(key), true);
        s.twin.note(i32::from(key), true);
        s.key = Some(key);
        s.key_last = Some(key);
        s.since = clock;
        s.idle = false;
        s.quiet = 0;
    }

    fn all_notes_off(&mut self) {
        for s in &mut self.slots {
            for k in 0..128 {
                s.v.note(k, false);
                s.twin.note(k, false);
            }
            s.key = None;
        }
    }

    pub fn midi(&mut self, m: &[u8]) {
        if m.is_empty() {
            return;
        }
        let (st, d1, d2) = (
            m[0] & 0xF0,
            *m.get(1).unwrap_or(&0),
            *m.get(2).unwrap_or(&0),
        );
        match st {
            0x90 if d2 > 0 => self.note(d1 & 0x7F, true),
            0x80 | 0x90 => self.note(d1 & 0x7F, false),
            0xB0 => match d1 {
                1 => {
                    self.cc_mod = f64::from(d2) / 127.0;
                    self.dirty = true;
                }
                120 | 123 => self.all_notes_off(),
                _ => {}
            },
            0xE0 => {
                let v = (i32::from(d2) << 7 | i32::from(d1)) - 8192;
                self.bend = f64::from(v) / if v < 0 { 8192.0 } else { 8191.0 };
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn index(key: &str) -> Option<usize> {
        p::KEYS.iter().position(|k| *k == key)
    }

    pub fn set(&mut self, key: &str, val: &str) {
        if key == "state" {
            for pair in val.split(';') {
                if let Some((k, v)) = pair.split_once('=') {
                    if k != "state" {
                        self.set(k.trim(), v.trim());
                    }
                }
            }
            return;
        }
        if key == "auto_db" || key.starts_with("dot_") {
            return; // (The engine's own: it shows them.)
        }
        let Some(i) = Engine::index(key) else { return };
        let Ok(x) = val.trim().parse::<f64>() else {
            return;
        };
        if !x.is_finite() {
            return;
        }
        let x = x.clamp(p::MIN[i], p::MAX[i]);
        let x = if p::OPTIONS[i] > 0 || i == p::VOICES {
            x.round()
        } else {
            x
        };
        if self.vals[i] != x {
            self.vals[i] = x;
            self.dirty = true;
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            // (For tests and diagnostics: whether HI's circuit filter is ready, voices awake.)
            "auto_db" => return Some(format!("{:.1}", self.auto_trim())),
            "display_rev" => return Some(format!("{}", self.rev)),
            k if k.starts_with("dot_") => {
                let i: usize = k[4..].parse().ok()?;
                return Some(format!("{:.4}", self.dots.get(i.checked_sub(1)?)?));
            }
            "hi_ready" => return Some(format!("{}", u8::from(self.hi_ready))),
            "awake" => {
                let n = self.playing();
                return Some(format!(
                    "{}",
                    self.slots[..n].iter().filter(|s| !s.idle).count()
                ));
            }
            _ => {}
        }
        if key == "state" {
            let s: Vec<String> = p::KEYS
                .iter()
                .zip(&self.vals)
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            return Some(s.join(";"));
        }
        let i = Engine::index(key)?;
        let v = self.vals[i];
        Some(if p::OPTIONS[i] > 0 || i == p::VOICES {
            format!("{}", v.round() as i64)
        } else {
            format!("{v}")
        })
    }

    /// The stereo field's dots for the voices sounding now: each at its place (DOUBLE: the
    /// pair's two), as many as there are dots. A change bumps the count the wrapper polls.
    fn field(&mut self) {
        let v = &self.vals;
        let (spread, inner) = (v[p::SPREAD] / 100.0, v[p::INNER] / 100.0);
        let placement = Placement::from_index(v[p::PLACEMENT].round() as usize);
        let places = (v[p::VOICES].round() as usize).clamp(2, MAX_VOICES);
        let n = self.playing();
        let mut dots = [0.0; DOTS];
        let mut d = 0;
        let mut put = |at: f64| {
            if d < DOTS {
                // (0.02 to 1 across the field, left to right: 0 is no dot.)
                dots[d] = 0.02 + 0.98 * (0.5 + 0.5 * at.clamp(-1.0, 1.0));
                d += 1;
            }
        };
        for (k, s) in self.slots.iter().enumerate().take(n) {
            if s.idle || s.peak < 1e-3 {
                continue;
            }
            if self.doubled {
                let out = if n == 1 {
                    ALONE
                } else {
                    placement.pair(k, places)
                };
                let at = pair_at(spread, inner, out);
                put(at);
                put(-at);
            } else {
                let at = if n == 1 { self.turn } else { k };
                put(voice_at(spread, inner, placement, at, places));
            }
        }
        // (Moves under a fortieth of the field are not news: no dot steps that finely.)
        if dots
            .iter()
            .zip(&self.dots)
            .any(|(a, b)| (a - b).abs() > 0.025 || (*a == 0.0) != (*b == 0.0))
        {
            self.dots = dots;
            self.rev += 1;
        }
    }

    pub fn render(&mut self, out: &mut [i16]) {
        self.take_hi();
        if self.dirty {
            self.apply();
        }
        let frames = out.len() / 2;
        let n = self.playing();
        let gain = self.gain;
        let entropy = (self.vals[p::ENTROPY] / 100.0).clamp(0.0, 1.0);
        let (doubled, half, glide) = (self.doubled, self.dress.half, self.glide);
        let scatter = n == 1 && !doubled && self.vals[p::SPREAD] > 0.0;
        out.fill(0);
        let mut acc = [(0f64, 0f64); 128];
        for chunk in 0..frames.div_ceil(128) {
            let (a, b) = (chunk * 128, ((chunk + 1) * 128).min(frames));
            let acc = &mut acc[..b - a];
            acc.fill((0.0, 0.0));
            let mut turned = false;
            for s in &mut self.slots[..n] {
                if s.idle {
                    continue;
                }
                let mut peak = 0.0f64;
                let mut closed = s.v.trigger_closed();
                for x in acc.iter_mut() {
                    if entropy > 0.0 {
                        let (cents, octaves) = s.ch.offsets(entropy, RATE);
                        s.v.set_offsets(cents.map(|c| c - half), octaves);
                        if doubled {
                            let (cents, octaves) = s.ch2.offsets(entropy, RATE);
                            s.twin.set_offsets(cents.map(|c| c + half), octaves);
                        }
                    }
                    for (now, aim) in s.now.iter_mut().zip(&s.aim) {
                        now.0 += (aim.0 - now.0) * glide;
                        now.1 += (aim.1 - now.1) * glide;
                    }
                    let y = s.v.tick(0.0);
                    peak = peak.max(y.abs());
                    x.0 += y * s.now[0].0;
                    x.1 += y * s.now[0].1;
                    if doubled {
                        let z = s.twin.tick(0.0);
                        peak = peak.max(z.abs());
                        x.0 += z * s.now[1].0;
                        x.1 += z * s.now[1].1;
                    }
                    // (MONO scattering: each attack at the next of VOICES's places, upstream's
                    // R62; a legato note, its contact staying closed, stays.)
                    let c = s.v.trigger_closed();
                    if scatter && c && !closed {
                        turned = true;
                    }
                    closed = c;
                }
                s.peak = peak;
                if peak < 1e-6 && s.key.is_none() && !s.v.held() && s.v.slow_state().is_some() {
                    s.quiet += 1;
                    if s.quiet >= IDLE_BLOCKS {
                        s.idle = true;
                    }
                } else {
                    s.quiet = 0;
                }
            }
            for (j, (l, r)) in acc.iter().enumerate() {
                out[2 * (a + j)] = (l * gain * 32767.0).clamp(-32768.0, 32767.0) as i16;
                out[2 * (a + j) + 1] = (r * gain * 32767.0).clamp(-32768.0, 32767.0) as i16;
            }
            if turned {
                // (The dots follow below, at the render's end.)
                // (Applied at the block's end: the attack's first samples at the last place,
                // a block's 2.9 ms.)
                let places = (self.vals[p::VOICES].round() as usize).clamp(2, MAX_VOICES);
                self.turn = (self.turn + 1) % places;
                self.place(false);
            }
        }
        self.field();
    }
}

/// A voice's panel with its own parts' tolerances under ENTROPY: upstream's `entropy_panel`.
fn entropy_panel(c: &Character, mut p: Panel, amount: f64) -> Panel {
    if amount == 0.0 {
        return p;
    }
    let k = |i: usize, x: f64| c.knob(i, x, amount);
    p.emphasis = k(0, p.emphasis);
    p.filter_contour.attack = k(1, p.filter_contour.attack);
    p.filter_contour.decay = k(2, p.filter_contour.decay);
    p.loudness_contour.attack = k(3, p.loudness_contour.attack);
    p.loudness_contour.decay = k(4, p.loudness_contour.decay);
    p.glide = k(5, p.glide);
    p
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // (The builder holds no lock for long; the library must not be unloaded under it.)
        if let Some(h) = self.builder.take() {
            let _ = h.join();
        }
    }
}

/// # Safety
/// The wrapper's own contract (`wrapper/engine.h`): pointers it hands over are valid.
#[unsafe(no_mangle)]
pub extern "C" fn ca72_create() -> *mut Engine {
    Box::into_raw(Box::new(Engine::new()))
}

/// # Safety
/// `e` from [`ca72_create`], not used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ca72_destroy(e: *mut Engine) {
    if !e.is_null() {
        // SAFETY: made by ca72_create, handed back once.
        drop(unsafe { Box::from_raw(e) });
    }
}

/// # Safety
/// `e` from [`ca72_create`]; `m` points at `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ca72_midi(e: *mut Engine, m: *const u8, len: i32) {
    if e.is_null() || m.is_null() || len <= 0 {
        return;
    }
    // SAFETY: the wrapper's message, `len` bytes.
    let m = unsafe { std::slice::from_raw_parts(m, len as usize) };
    unsafe { &mut *e }.midi(m);
}

/// # Safety
/// `e` from [`ca72_create`]; `k` and `v` NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ca72_set_param(e: *mut Engine, k: *const c_char, v: *const c_char) {
    if e.is_null() || k.is_null() || v.is_null() {
        return;
    }
    // SAFETY: NUL-terminated strings from the wrapper.
    let (k, v) = unsafe { (CStr::from_ptr(k), CStr::from_ptr(v)) };
    if let (Ok(k), Ok(v)) = (k.to_str(), v.to_str()) {
        unsafe { &mut *e }.set(k, v);
    }
}

/// # Safety
/// `e` from [`ca72_create`]; `k` NUL-terminated; `buf` holds `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ca72_get_param(
    e: *mut Engine,
    k: *const c_char,
    buf: *mut c_char,
    len: i32,
) -> i32 {
    if e.is_null() || k.is_null() || buf.is_null() || len <= 0 {
        return -1;
    }
    // SAFETY: a NUL-terminated key from the wrapper.
    let Ok(k) = unsafe { CStr::from_ptr(k) }.to_str() else {
        return -1;
    };
    let Some(s) = unsafe { &*e }.get(k) else {
        return -1;
    };
    let n = s.len().min(len as usize - 1);
    // SAFETY: `buf` has `len` bytes; n < len.
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf.cast::<u8>(), n);
        *buf.add(n) = 0;
    }
    n as i32 + 1
}

/// # Safety
/// `e` from [`ca72_create`]; `out` holds `frames` interleaved stereo frames.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ca72_render(e: *mut Engine, out: *mut i16, frames: i32) {
    if e.is_null() || out.is_null() || frames <= 0 {
        return;
    }
    // SAFETY: the wrapper's buffer of `frames` stereo frames.
    let out = unsafe { std::slice::from_raw_parts_mut(out, 2 * frames as usize) };
    unsafe { &mut *e }.render(out);
}
