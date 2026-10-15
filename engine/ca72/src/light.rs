//! Potato mode's voice (decisions.md R31): the instrument's panel played by a model far
//! cheaper than the circuit's, for computers the circuit is too heavy for. Not derived from
//! the circuit but fitted to the circuit model's measured behaviour ([`laws`]; the plug-in's
//! `tests/potato_reference.rs` measures both): band-limited oscillators (polyBLEP) at the
//! circuit's levels and shapes, the mixer's channels through their loaded pots, the input
//! pair's compression, one ladder of four one-pole stages solved without delay around the
//! loop (its gain the emphasis, its own limiter setting how loud it rings), FILTER MODE HI
//! (the bus less the filter), the plug-in's DRIVE, contours of RC segments with the circuit's
//! time laws and trigger, and the VCA's measured law. The audio
//! path runs in single precision at the voice's rate; what the panel and the contours set
//! (the oscillators' increments, the ladder's coefficient) is worked out every
//! [`CONTROL_EVERY`] samples and followed between.
//!
//! The keys are the instrument's: lowest-note priority (or NOTE PRIORITY's HIGH or LAST, as the
//! circuit's voice has them: [`crate::voice::Priority`]), single triggering (a key pressed while
//! another is held does not start the contours again), the contours re-armed once the trigger
//! contact has been open 11.5 ms; GLIDE moves the pitch as the circuit's hold does. Nothing here
//! allocates once made.

use crate::tuning::FREQ_CENTRE;
#[cfg(test)]
use crate::voice::OscPanel;
use crate::voice::{
    ContourKnobs, FILTER_ATTACK, FILTER_DECAY, LOUDNESS_ATTACK, LOUDNESS_DECAY, Panel, Priority,
    Waveform, emphasis_r14, ext_taper, glide_r, sustain_track, time_pot, volume_track,
};

/// Samples between the control laws' updates.
pub const CONTROL_EVERY: usize = 16;

/// The laws, fitted to the circuit model's measurements at 48 kHz (`tests/potato_reference.rs`;
/// decisions.md R31), as calibrated against the hardware reference (R42 on): where the circuit
/// reads a knob through the reference's law (voice.rs's `volume_track`, `time_pot` and the
/// rest), the light voice reads it through the same law.
pub mod laws {
    /// Key 69 on 8' at TUNE's centre, Hz (the circuit's: 439.78, within 1.4 cents of equal
    /// temperament from key 24 to 108).
    pub const A4: f64 = 440.0;
    /// FREQUENCY (oscillators 2 and 3) from its centre, cents: a cubic in the pot's track
    /// (the panel's FREQUENCY is the pot's wiper: `freq_dial`), within 1.2 cents of the
    /// measured.
    pub const OSC2_CENTS: [f64; 4] = [-855.506, 1753.088, -94.505, 19.578];
    pub const OSC3_CENTS: [f64; 4] = [-862.370, 1806.991, -206.463, 83.185];
    /// TUNE at either end, cents.
    pub const TUNE_CENTS: f64 = 267.44;
    /// Oscillator 3 with OSC. 3 CONTROL off: as this key held on 8' (201.5 Hz) at FREQUENCY's
    /// centre; FREQUENCY then, from its centre, cents, a quintic in the pot's track (its
    /// summing node without the keys' input: 69 semitones end to end); TUNE moves it not at
    /// all.
    pub const OSC3_FREE_KEY: f64 = 55.479;
    pub const OSC3_FREE_CENTS: [f64; 6] = [-3797.431, 9228.8, -4699.331, 3541.6, -1559.71, 402.07];

    /// Each waveform's level against the sawtooth's (the bus's unit: a sawtooth at VOLUME 10
    /// is 1 at its peak), its channel's source resistance (ohm, for the pot's loading) and the
    /// rectangles' duty cycles.
    pub const TRIANGLE: f32 = 0.857;
    pub const SAWTOOTH: f32 = 1.0;
    pub const REVERSE_SAWTOOTH: f32 = 1.0;
    pub const SQUARE: f32 = 1.032;
    pub const WIDE: f32 = 1.046;
    pub const NARROW: f32 = 1.059;
    pub const DUTY: [f32; 3] = [0.484, 0.30, 0.179];
    /// The triangle's lowest point, as a share of its period from its top (the sawtooth's
    /// reset): a little before half way, as the circuit's, whose even harmonics stand 37 dB
    /// (the second), 43 and 46 dB under its fundamental.
    pub const TRIANGLE_BOTTOM: f32 = 0.4915;
    /// The shark tooth: the triangle inverted (its peak at the sawtooth's reset, and this
    /// share of a period ahead of it) and the falling sawtooth, as R030 and R031 mix them (its
    /// odd harmonics so within 0.5 dB of the circuit's to the ninth).
    pub const SHARK_TRIANGLE: f32 = 0.704;
    pub const SHARK_SAWTOOTH: f32 = 0.189;
    pub const SHARK_AHEAD: f32 = 0.0175;
    pub const SOURCE_OHMS: [f64; 4] = [687.0, 2246.0, 3500.0, 8790.0];
    /// Oscillator 3's channel against the others' (its sawtooth through the reverse
    /// sawtooth's stage).
    pub const OSC3_LEVEL: f32 = 0.914;
    /// The high-passes the output shows (the waveforms' droop), Hz: the filter's output
    /// coupling (C5/C1, which FILTER MODE HI's direct branch does not pass through) and the
    /// rest after the VCA; together within 0.1 dB of the circuit's at 7 to 28 Hz.
    pub const COUPLING_HZ: f64 = 13.0;
    pub const DROOP_HZ: f64 = 4.4;
    /// FILTER MODE HI (decisions.md R46): the bus less the filter's output, the bus at the
    /// filter's own passband at EMPHASIS 0 (the open ladder's, times this) and through its
    /// own coupling, Hz (vcf.rs's `MODE_HZ`).
    pub const MODE_GAIN: f32 = 1.0;
    pub const MODE_HZ: f64 = 3.0;
    /// The VCA's control feeding through to the output, its thump as the loudness contour
    /// moves: the VCA's gain through a high-pass and a low-pass (time constants, s), times
    /// this (fitted to the circuit's output with nothing sounding, four contours at once,
    /// within 14 dB of the thump; most of it under 5 Hz).
    pub const THUMP: f32 = 0.0385;
    pub const THUMP_TIMES: (f64, f64) = (0.11, 0.035);

    /// The noise: white's and pink's level (on their generators' units), their channels'
    /// source resistances (pink's through R50), and each one's band (a high-pass and a
    /// low-pass, Hz).
    pub const WHITE: f32 = 0.600;
    pub const PINK: f32 = 0.440;
    pub const NOISE_OHMS: (f64, f64) = (700.0, 26.2e3);
    pub const WHITE_BAND: (f64, f64) = (60.0, 9_500.0);
    pub const PINK_BAND: (f64, f64) = (40.0, 6_500.0);
    /// The white's low-pass at each of `NOISE_TOP_AT`'s rates, Hz: the fit at 48 kHz takes in
    /// the circuit's own top near Nyquist; at 88.2 kHz and over its noise reaches higher.
    pub const NOISE_TOP_AT: [f64; 2] = [48_000.0, 88_200.0];
    pub const WHITE_TOP: [f64; 2] = [9_500.0, 19_000.0];
    pub const PINK_TOP: [f64; 2] = [6_500.0, 9_000.0];

    /// The ladder's input pair, a tanh on the input less the loop's feedback: the bus in its
    /// units (two and three sawtooths at full VOLUME compress 0.6 and 1.1 dB, the circuit's
    /// 0.7 and 1.2), off its centre by `BIAS` (with the triangle's own, its second harmonic
    /// from -37 dB at VOLUME 1 to -32 dB at 10, as the circuit's).
    pub const DRIVE: f32 = 0.69;
    pub const BIAS: f32 = 0.10;
    /// The loop's gain in `LOOP` (fitted with the pair off its centre by 0.28) times this:
    /// the same gain through the pair's slope at `BIAS`.
    pub const LOOP_SCALE: f64 = 0.941_52;
    /// The plug-in's DRIVE at gain G: the pair's limit (its tanh's) lowered to G to this
    /// power, its slope kept: the circuit's ladder, its every stage a pair, holds a driven
    /// input lower than one pair would (0.88 of it at 24 dB; the level so within 0.75 dB of
    /// the circuit's from 0 to 24 dB on six patches).
    pub const DRIVE_CEILING: f64 = -0.045;
    /// The output from the ladder's (at the VCA's gain 1: the loudness contour at 4.455 V).
    pub const OUT: f32 = 0.613;

    /// CUTOFF: log2 of the self-oscillating ladder's pitch, Hz, at the knob's 0, 0.05, ...,
    /// 1 (measured from 0.15 to 0.9; below, continued at the slope there in the pot's track,
    /// voice.rs's `cutoff_track`; above, past Nyquist, continued).
    pub const CUTOFF_LOG2: [f32; 21] = [
        5.2, 5.5, 5.81, 6.345, 6.787, 7.257, 7.745, 8.306, 8.878, 9.459, 10.045, 10.626, 11.208,
        11.789, 12.366, 12.874, 13.375, 13.866, 14.229, 14.42, 14.55,
    ];
    /// KEYBOARD CONTROL 1, 2 and both: octaves of the cutoff an octave of the keys, from C
    /// (36, where the keyboard's voltage is 0).
    pub const KEYBOARD_CONTROL: [f64; 3] = [0.3276, 0.6486, 0.9555];
    pub const TRACKING_FROM: f64 = 36.0;
    /// AMOUNT OF CONTOUR: octaves a volt of the filter contour above its rest at 10, and the
    /// share at 0, 2.5, 5, 7.5 and 10.
    pub const CONTOUR_OCTAVES_PER_VOLT: f64 = 2.10;
    pub const AMOUNT: [f64; 5] = [0.0, 0.192, 0.487, 0.778, 1.0];
    /// EMPHASIS: the loop's gain (4 rings; times `LOOP_SCALE`) at these positions of the knob
    /// drawn with the generic taper (below), for cutoffs of 60, 87, 228, 912 and 3731 Hz (log2
    /// below; between them interpolated, beyond them held): fitted to the drawn circuit's
    /// responses to noise at EMPHASIS up to 7 (decisions.md R31), within 0.4 dB of the
    /// calibrated circuit's passbands at 5 and 7 and 1 dB of its peaks at 5, 6 and 7 below its
    /// threshold. Past the threshold the gain follows R14's law, 11.84 / (R14 + 1.58K) (R14
    /// in K), to 7.5 at 10, scaled by `RING_GAIN`; at 10 it rings down to about 81 Hz and not
    /// below, as the circuit's.
    pub const EMPHASIS_AT: [f64; 11] = [
        0.0, 0.25, 0.5, 0.7, 0.72, 0.735, 0.744, 0.79, 0.85, 0.9, 1.0,
    ];
    pub const LOOP_AT_LOG2: [f64; 10] = [
        5.9, 6.44, 6.864, 7.783, 8.75, 9.745, 10.749, 11.752, 12.747, 13.711,
    ];
    pub const LOOP: [[f64; 11]; 10] = [
        [
            0.146, 0.502, 1.08, 1.95, 2.06, 2.14, 2.2, 2.45, 2.8, 3.2, 3.8,
        ],
        [
            0.154, 0.528, 1.2, 2.3, 2.45, 2.56, 2.65, 3.05, 3.55, 3.9, 4.212,
        ],
        [
            0.158, 0.54, 1.502, 2.79, 2.955, 3.079, 3.445, 3.805, 4.1, 4.229, 4.429,
        ],
        [
            0.166, 0.567, 1.575, 3.306, 3.509, 3.678, 3.693, 4.13, 4.407, 4.652, 5.229,
        ],
        [
            0.201, 0.616, 1.675, 3.595, 3.815, 3.962, 3.964, 4.313, 5.043, 5.669, 6.713,
        ],
        [
            0.238, 0.668, 1.779, 3.749, 3.986, 4.079, 4.127, 4.651, 5.436, 6.112, 7.695,
        ],
        [
            0.244, 0.676, 1.798, 3.82, 4.033, 4.121, 4.127, 4.707, 5.502, 6.186, 8.055,
        ],
        [
            0.248, 0.68, 1.81, 3.849, 4.06, 4.123, 4.127, 4.737, 5.536, 6.225, 8.29,
        ],
        [
            0.248, 0.68, 1.811, 3.845, 4.061, 4.125, 4.127, 4.74, 5.54, 6.23, 8.267,
        ],
        [
            0.248, 0.68, 1.811, 4.05, 4.123, 4.125, 4.127, 4.74, 5.54, 6.23, 8.267,
        ],
    ];
    pub const RING_GAIN: f64 = 2.5;
    /// EMPHASIS read as the knob drawn with its generic taper would set R14 (voice.rs's
    /// `emphasis_r14` against `emphasis_r14_drawn`); and over the last `EMPHASIS_NEAR` of
    /// the drawn knob below the calibrated circuit's ringing threshold (`THRESHOLD`, the drawn
    /// knob, at these cutoffs, log2 Hz; between them interpolated, beyond them held: it rings
    /// from 8.3 near 210 Hz, 7.25 near 1 kHz and 7.0 near 5 kHz), moved on by `SHIFT` to the
    /// table's own threshold there.
    pub const THRESHOLD_AT_LOG2: [f64; 3] = [7.74, 10.04, 12.37];
    pub const THRESHOLD: [f64; 3] = [0.7923, 0.7274, 0.7164];
    pub const SHIFT: [f64; 3] = [-0.0032, 0.0195, 0.0176];
    pub const EMPHASIS_NEAR: f64 = 0.012;
    /// The ladder's corner above the ring's pitch (the table above), octaves, below the ringing
    /// threshold: fitted to the drawn circuit's responses at its CUTOFF 0.3, 0.5 and 0.7 (220,
    /// 858 and 3454 Hz rings), and below them to the calibrated circuit's resonant peaks at
    /// CUTOFF 0.05 to 0.2 (within 4 Hz); it closes to nothing at the threshold, as the ring's
    /// own saturation pulls the circuit's corner down to its pitch.
    pub const CORNER: [f64; 4] = [-0.1, 0.048, 0.088, 0.111];
    pub const CORNER_AT_LOG2: [f64; 4] = [6.2, 7.78, 9.74, 11.75];
    /// The filter's passband against the open filter's, as the circuit's (whose passband
    /// falls as its cutoff goes below 100 Hz, 4.2 dB at CUTOFF 0): the ladder's output times
    /// this at these cutoffs (log2 Hz), dB.
    pub const PASS_AT_LOG2: [f64; 13] = [
        5.2, 5.5, 5.81, 6.345, 6.787, 7.257, 7.745, 8.306, 8.878, 10.045, 10.626, 14.229, 14.55,
    ];
    pub const PASS_DB: [f64; 13] = [
        -4.1, -3.4, -2.8, -2.0, -1.5, -1.1, -0.8, -0.45, -0.2, 0.1, 0.3, 0.1, 0.0,
    ];
    /// Past the threshold, where `CORNER` has closed, the ring's own pitch raised this much,
    /// octaves (the input pair's knee in the loop pulls the ring flat; the circuit's rings
    /// measured, within 3 cents from 81 Hz to 3.5 kHz).
    pub const RING_RAISE: [f64; 5] = [0.001, -0.0015, 0.0085, 0.0065, -0.0025];
    pub const RING_RAISE_AT_LOG2: [f64; 5] = [6.44, 7.78, 8.75, 11.75, 12.75];
    /// Past the threshold (as `CORNER` closes), the pair's limit raised to this at these
    /// cutoffs (log2 Hz): the circuit's rings at EMPHASIS 10 louder against the input's limit
    /// as they rise (its every stage a pair), within 0.3 dB of them to 15 kHz.
    pub const RING_LEVEL_AT_LOG2: [f64; 15] = [
        6.34, 6.79, 7.26, 7.74, 8.31, 8.88, 9.46, 10.04, 10.63, 11.21, 11.79, 12.37, 12.87, 13.38,
        13.87,
    ];
    pub const RING_LEVEL: [f64; 15] = [
        1.738, 1.352, 1.239, 1.236, 1.205, 1.212, 1.228, 1.235, 1.226, 1.244, 1.264, 1.300, 1.350,
        1.376, 1.451,
    ];

    /// The contours: their rest from power-on and the level an attack stops at (its
    /// plateau), V (filter, loudness); where its RC curve aims, as a share of the plateau's
    /// rise from rest; and how much earlier a fast attack stops (the timing capacitor's
    /// series resistance against the charging current): the plateau less the aim's distance
    /// above it times this over the attack's time constant, s.
    pub const REST: [f64; 2] = [0.0743, -0.516];
    pub const PEAK: [f64; 2] = [4.7917, 5.6014];
    pub const ATTACK_AIM: [f64; 2] = [1.82, 1.40];
    pub const ATTACK_ESR: [f64; 2] = [1.93e-5, 1.66e-5];
    /// The time constants, s, against the pots' resistance R (voice.rs's `time_pot` through
    /// the reference's laws, ohm): A (R / 100K)^B + T0 as (A, B, T0) for ATTACK, DECAY from the
    /// peak, and the release with the DECAY switch on (DECAY's pot), each (filter, loudness):
    /// within 0.6 % of the circuit's at ATTACK 2 to 10 (DECAY with `DECAY_FALL`).
    pub const ATTACK_TIME: [(f64, f64, f64); 2] =
        [(1.114_91, 1.007_46, 0.0007), (1.115_32, 1.003_32, 0.0008)];
    pub const DECAY_TIME: [(f64, f64, f64); 2] =
        [(1.168, 1.000_67, 0.0003), (1.167, 1.000_55, 0.0003)];
    /// DECAY from the peak faster the further it has to fall (the circuit's through its
    /// diodes): its rate times 1 + this times the volts to go (filter, loudness); its 50, 90
    /// and 99 % times so within 2 % of the circuit's (one exponential's 4 to 6 % apart).
    pub const DECAY_FALL: [f64; 2] = [0.05, 0.035];
    pub const RELEASE_TIME: [(f64, f64, f64); 2] =
        [(1.119_76, 1.002_92, 0.0003), (1.123_86, 1.004_29, 0.0003)];
    /// SUSTAIN's level, V: a cubic in the pot's track (voice.rs's `sustain_track`; filter,
    /// loudness), within 6 mV of the circuit's.
    pub const SUSTAIN: [[f64; 4]; 2] = [
        [0.0431, 3.7727, -2.2079, 2.2245],
        [-0.4729, 4.9704, -2.4266, 2.3829],
    ];
    /// Where a release falls to, V (filter, loudness), with the DECAY switch on (at DECAY's
    /// time).
    pub const RELEASED: [f64; 2] = [0.1426, -0.4964];
    /// With the switch off, by DECAY at each of `RELEASE_OFF_AT`: where it falls to, V, and
    /// its time constant, s (the capacitor through DECAY's pot and the switch's resistor at
    /// once: at once and to `RELEASED`'s level at 0, lower and slower as the pot turns up;
    /// the circuit's falls so far and then creeps on, its last tenth over some hundred
    /// milliseconds).
    pub const RELEASE_OFF_AT: [f64; 11] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
    pub const RELEASED_OFF: [[f64; 11]; 2] = [
        [
            0.1392, -0.0179, -0.0688, -0.0872, -0.1049, -0.1178, -0.1263, -0.1936, -0.2195,
            -0.2363, -0.2443,
        ],
        [
            -0.4998, -0.6582, -0.6988, -0.7182, -0.7368, -0.7513, -0.7601, -0.8294, -0.8585,
            -0.8773, -0.886,
        ],
    ];
    pub const RELEASE_OFF: [[f64; 11]; 2] = [
        [
            0.0005, 0.0178, 0.0209, 0.0217, 0.0224, 0.0228, 0.0231, 0.0249, 0.0256, 0.026, 0.0262,
        ],
        [
            0.0005, 0.0174, 0.0200, 0.0208, 0.0215, 0.0220, 0.0222, 0.0238, 0.0243, 0.0246, 0.0247,
        ],
    ];
    /// The trigger: the contours start this long after a key goes down and fall this long
    /// after the last is let go, s; a key pressed `REARM` or more after the last was let go
    /// starts them again (an attack), even before they have fallen.
    pub const TRIGGER_ON: f64 = 0.0078;
    pub const TRIGGER_OFF: f64 = 0.0141;
    pub const REARM: f64 = 0.0115;

    /// The VCA: its gain (1 at the loudness contour's 4.455 V) every 0.1 V from 0 V; past
    /// the last, as there.
    pub const VCA_STEP: f64 = 0.1;
    pub const VCA_GAIN: [f32; 54] = [
        0.0, 0.001, 0.0096, 0.0281, 0.0503, 0.0737, 0.0976, 0.1215, 0.1454, 0.1691, 0.1927, 0.219,
        0.245, 0.2706, 0.296, 0.3211, 0.3458, 0.3702, 0.3969, 0.4232, 0.4491, 0.4746, 0.4997,
        0.5245, 0.5512, 0.5775, 0.601, 0.6264, 0.6536, 0.6803, 0.7065, 0.7321, 0.7573, 0.782,
        0.8082, 0.8338, 0.8589, 0.8854, 0.9112, 0.9364, 0.961, 0.9839, 0.9959, 0.9992, 1.0001, 1.0,
        0.9994, 0.9985, 0.9974, 0.9961, 0.9947, 0.9933, 0.9917, 0.99,
    ];

    /// GLIDE: the pitch slides at a constant rate, semitones a second times its pot's ohms
    /// (voice.rs's `glide_r`, the hold's 330 ohm added), rising; falling, this share of it.
    /// It holds while no key is down (the keyboard's track-and-hold).
    pub const GLIDE_RATE: f64 = 9.76e7;
    pub const GLIDE_FALLING: f64 = 0.535;
    /// The keys the keyboard circuit plays (F to C, MIDI); beyond them the plug-in adds the
    /// rest of the scale after it, moving through GLIDE's resistance and the hold's 1 uF alone
    /// (an RC, not the slide), while a key is down (voice.rs's `beyond`).
    pub const KEYBOARD_KEYS: (f64, f64) = (41.0, 84.0);
    pub const GLIDE_FARADS: f64 = 1e-6;
    /// Where a fresh voice's pitch rests, as a key: the keyboard's output before any key.
    pub const PITCH_AT_REST: f64 = 53.75;
    /// How long a released key's pitch contact stays closed after its trigger contact opens,
    /// s (voice.rs's `RELEASE_LEAD`): a key let go while a higher one is held moves the pitch
    /// this much later.
    pub const RELEASE_LEAD: f64 = 0.002;

    /// The modulation at the MODULATION wheel fully forward, from oscillator 3: semitones of
    /// the oscillators at its waveform's swing (each from -1 to 1) and its centre against the
    /// triangle's, DC-coupled as the circuit's line is: the triangle, reverse sawtooth and
    /// sawtooth about their centres, the rectangles one-sided and inverted against their
    /// sound (up while the rectangle is low: 0.28 to 18.9 semitones with the line's offset);
    /// the cutoff's octaves `FILTER_PER_SEMITONE` of the oscillators' semitones; the line's
    /// offset; from the noise (MODULATION MIX at 10): their RMS and offset.
    pub const MOD_SWING: [f64; 4] = [-8.93, -8.85, -8.81, -9.32];
    pub const MOD_CENTRE: [f64; 4] = [0.0, -1.47, -0.69, 8.54];
    pub const FILTER_PER_SEMITONE: f64 = 0.2632;
    pub const MOD_OFFSET: (f64, f64) = (1.06, 0.29);
    pub const NOISE_SEMITONES_RMS: f64 = 2.80;
    pub const NOISE_OCTAVES_RMS: f64 = 0.679;
    pub const NOISE_OFFSET: (f64, f64) = (0.25, 0.062);
    /// The swing's share against the wheel's position as the drawing's wheel would set its
    /// resistance (a cubic), and the offsets' against the wheel's resistance R (voice.rs's
    /// `mod_wheel_r`): `OFFSET.0` R / (R + `OFFSET.1`) (the line divided by its rheostat;
    /// within 12 % of the circuit's from a quarter forward).
    pub const WHEEL: [f64; 3] = [0.828, 0.584, -0.412];
    pub const OFFSET: (f64, f64) = (0.9915, 218.3);

    /// EXTERNAL INPUT: its gain to the output, `EXT_GAIN t / (1 + EXT_LOAD t (1 - t))` for
    /// VOLUME's law t (voice.rs's `ext_taper`: the 1M pot loaded by the preamplifier; within
    /// 2 %), the level its preamplifier clips at there, and the OVERLOAD lamp's onset and
    /// full brightness there.
    pub const EXT_GAIN: f64 = 173.5;
    pub const EXT_LOAD: f64 = 10.16;
    pub const EXT_CLIP: f64 = 0.765;
    pub const OVERLOAD: (f64, f64) = (0.24, 0.36);
    /// EXTERNAL INPUT's couplings, high-passes, Hz: at the jack, and after its preamplifier
    /// (C20) on the way to the mixer (the circuit's input from the jack to the output within
    /// 0.6 dB of it from 3 to 30 Hz).
    pub const EXT_COUPLING_HZ: f64 = 9.0;
    pub const PREAMP_COUPLING_HZ: f64 = 8.0;

    /// A-440: its level at the output (its table's units).
    pub const A440_LEVEL: f32 = 0.2385;

    /// FEEDBACK's cable: how late the output reaches EXTERNAL INPUT, samples, at each of
    /// `FEEDBACK_AT`'s rates (the circuit's loop from EXTERNAL INPUT back is a count of
    /// samples, not a time: 88 at 44.1 and 48 kHz, 95 at 88.2 and 96, less Potato's own path's;
    /// the loop's time sets how it oscillates).
    pub const FEEDBACK_AT: [f64; 4] = [44_100.0, 48_000.0, 88_200.0, 96_000.0];
    pub const FEEDBACK_DELAY: [f64; 4] = [87.9, 87.0, 94.0, 94.2];
}

/// The noise generators' coefficients at a rate: the white's scale (its density a hertz the
/// same at every rate), the pink filter's poles and gains (Paul Kellet's, at 48 kHz, their
/// corners kept in hertz), the red's pole and gain, and the bands' (white's high-pass and
/// low-pass, pink's).
#[derive(Debug, Clone, Copy)]
struct NoiseLaw {
    scale: f32,
    poles: [f32; 3],
    gains: [f32; 3],
    red: (f32, f32),
    band: [f32; 4],
}

impl NoiseLaw {
    fn new(rate: f64) -> NoiseLaw {
        // (At 48 kHz each factor is 1 exactly: the coefficients as they were.)
        let r = (48_000.0 / rate) as f32;
        let pole = |p: f32| p.powf(r);
        let gain = |g: f32, p: f32| g * ((1.0 - pole(p)) / (1.0 - p));
        let one_pole = |hz: f64| 1.0 - (-std::f64::consts::TAU * hz / rate).exp();
        NoiseLaw {
            scale: (rate / 48_000.0).sqrt() as f32,
            poles: [pole(0.99765), pole(0.963), pole(0.57)],
            gains: [
                gain(0.099_046, 0.99765),
                gain(0.296_516_4, 0.963),
                gain(1.052_691_3, 0.57),
            ],
            red: (pole(0.995), gain(0.05, 0.995)),
            band: [
                (-std::f64::consts::TAU * laws::WHITE_BAND.0 / rate).exp() as f32,
                one_pole(interpolate(&laws::NOISE_TOP_AT, &laws::WHITE_TOP, rate).min(0.45 * rate))
                    as f32,
                (-std::f64::consts::TAU * laws::PINK_BAND.0 / rate).exp() as f32,
                one_pole(interpolate(&laws::NOISE_TOP_AT, &laws::PINK_TOP, rate).min(0.45 * rate))
                    as f32,
            ],
        }
    }
}

/// A-440's waveform, one period (the circuit model's, measured).
const A440_SHAPE: [f32; 64] = [
    -0.014, 0.045, 0.106, 0.170, 0.238, 0.304, 0.362, 0.415, 0.459, 0.491, 0.504, 0.496, 0.480,
    0.464, 0.448, 0.433, 0.418, 0.403, 0.389, 0.375, 0.361, 0.346, 0.329, 0.311, 0.289, 0.265,
    0.239, 0.210, 0.179, 0.146, 0.112, 0.076, 0.038, -0.001, -0.042, -0.085, -0.130, -0.176,
    -0.221, -0.264, -0.304, -0.340, -0.374, -0.404, -0.430, -0.452, -0.470, -0.483, -0.492, -0.496,
    -0.495, -0.490, -0.479, -0.464, -0.443, -0.418, -0.388, -0.354, -0.316, -0.273, -0.227, -0.178,
    -0.126, -0.071,
];

/// tanh, rational: within 2.4 % of it, exactly odd, ±1 beyond ±3.
#[inline(always)]
fn tanh(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// 2^x for |x| < 64, within 2e-7 of it relatively: the nearest integer in the exponent's
/// bits (found by adding and taking away 1.5 * 2^23, not by a library call), and 2^f for the
/// rest (|f| <= 1/2) by its series to the sixth power.
#[inline(always)]
fn exp2(x: f32) -> f32 {
    const SHIFT: f32 = 12_582_912.0;
    let x = x.clamp(-60.0, 60.0);
    let i = (x + SHIFT) - SHIFT;
    let f = x - i;
    let p = 1.0
        + f * (std::f32::consts::LN_2
            + f * (0.240_226_5
                + f * (0.055_504_11
                    + f * (0.009_618_129 + f * (0.001_333_355 + f * 0.000_154_035)))));
    f32::from_bits((p.to_bits() as i32 + ((i as i32) << 23)) as u32)
}

/// The two-sample polyBLEP at phase `t` (0..1) for an increment `dt` and its reciprocal.
#[inline(always)]
fn blep(t: f32, dt: f32, inv: f32) -> f32 {
    if t < dt {
        let x = t * inv;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) * inv;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

/// A table's value at `x`, its points at `xs` (rising); continued flat beyond its ends.
fn interpolate(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    if x <= xs[0] {
        return ys[0];
    }
    for i in 1..xs.len() {
        if x <= xs[i] {
            let a = (x - xs[i - 1]) / (xs[i] - xs[i - 1]);
            return ys[i - 1] + (ys[i] - ys[i - 1]) * a;
        }
    }
    ys[ys.len() - 1]
}

/// A mixer channel's gain for its VOLUME (25K linear, its wiper through `series` ohm to the
/// bus, the source `source` ohm behind it), against the same at 10 for the sawtooth's.
fn channel(volume: f64, source: f64, series: f64) -> f32 {
    let law = |p: f64, source: f64| {
        let p = p.clamp(0.0, 1.0);
        let (up, down) = (source + 25e3 * (1.0 - p), (25e3 * p).max(1e-3));
        (down / (up + down)) / (up * down / (up + down) + series)
    };
    (law(volume, source) / law(1.0, laws::SOURCE_OHMS[1])) as f32
}

/// The keys as the engine plays them: which are held (a bit a MIDI note), lowest first.
#[derive(Debug, Clone, Copy, Default)]
struct Held(u128);

impl Held {
    fn set(&mut self, key: i32, on: bool) {
        if (0..128).contains(&key) {
            let b = 1u128 << key;
            if on {
                self.0 |= b;
            } else {
                self.0 &= !b;
            }
        }
    }

    fn lowest(self) -> Option<i32> {
        (self.0 != 0).then(|| self.0.trailing_zeros() as i32)
    }

    fn highest(self) -> Option<i32> {
        (self.0 != 0).then(|| 127 - self.0.leading_zeros() as i32)
    }

    /// The key held pressed last, by `pressed` (when each was pressed, counted in presses).
    fn newest(self, pressed: &[u32; 128]) -> Option<i32> {
        let mut bits = self.0;
        let mut newest: Option<usize> = None;
        while bits != 0 {
            let k = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            if newest.is_none_or(|n| pressed[k] > pressed[n]) {
                newest = Some(k);
            }
        }
        newest.map(|k| k as i32)
    }
}

/// An oscillator: its phase (0..1) and waveform.
#[derive(Debug, Clone, Copy)]
struct Osc {
    phase: f32,
    wave: Waveform,
}

impl Osc {
    /// One sample at increment `dt` (cycles a sample, under a half) and its reciprocal.
    #[inline(always)]
    fn tick(&mut self, dt: f32, inv: f32) -> f32 {
        let mut p = self.phase + dt;
        if p >= 1.0 {
            p -= 1.0;
        }
        self.phase = p;
        // The sawtooth falls and jumps up at its reset; the reverse sawtooth rises.
        let fall = || 1.0 - 2.0 * p + blep(p, dt, inv);
        let rect = |d: f32| {
            let q = if p >= d { p - d } else { p - d + 1.0 };
            let naive = if p < d { 1.0 } else { -1.0 };
            // (Centred: the rectangle's mean taken out, as the coupling does.)
            naive + blep(p, dt, inv) - blep(q, dt, inv) - (2.0 * d - 1.0)
        };
        match self.wave {
            // (The triangle at its top as the sawtooth resets, as the circuit's is.)
            Waveform::Triangle => {
                const B: f32 = laws::TRIANGLE_BOTTOM;
                let t = if p < B {
                    1.0 - p * (2.0 / B)
                } else {
                    (p - B) * (2.0 / (1.0 - B)) - 1.0
                };
                laws::TRIANGLE * t
            }
            Waveform::SharkTooth => {
                let q = p + laws::SHARK_AHEAD;
                let q = if q >= 1.0 { q - 1.0 } else { q };
                laws::SHARK_TRIANGLE * (4.0 * (q - 0.5).abs() - 1.0) + laws::SHARK_SAWTOOTH * fall()
            }
            Waveform::Sawtooth => laws::SAWTOOTH * fall(),
            Waveform::ReverseSawtooth => -laws::REVERSE_SAWTOOTH * fall(),
            Waveform::Square => laws::SQUARE * rect(laws::DUTY[0]),
            Waveform::WideRectangle => laws::WIDE * rect(laws::DUTY[1]),
            Waveform::NarrowRectangle => laws::NARROW * rect(laws::DUTY[2]),
        }
    }

    /// Its waveform's level at its peak (oscillator 3's swing as a modulation source).
    fn peak(wave: Waveform) -> f32 {
        match wave {
            Waveform::Triangle => laws::TRIANGLE,
            Waveform::SharkTooth => laws::SHARK_TRIANGLE + laws::SHARK_SAWTOOTH,
            Waveform::Sawtooth => laws::SAWTOOTH,
            Waveform::ReverseSawtooth => laws::REVERSE_SAWTOOTH,
            Waveform::Square => laws::SQUARE,
            Waveform::WideRectangle => laws::WIDE,
            Waveform::NarrowRectangle => laws::NARROW,
        }
    }
}

/// White noise (xorshift), and pink and red from it (filtered), with the white's band.
#[derive(Debug, Clone, Copy)]
struct Noise {
    state: u32,
    pink: [f32; 3],
    red: f32,
    /// The white's high-pass (its last input and output) and low-pass, the pink's.
    hp: (f32, f32),
    lp: f32,
    pink_hp: (f32, f32),
    pink_lp: f32,
}

impl Noise {
    #[inline(always)]
    fn white(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        (x as i32 as f32) * (1.0 / 2_147_483_648.0)
    }

    /// White (its band), pink (its) and red, each in its generator's units; `c` the
    /// generators' coefficients at the rate.
    #[inline(always)]
    fn tick(&mut self, c: &NoiseLaw) -> (f32, f32, f32) {
        let w = self.white() * c.scale;
        let b = &mut self.pink;
        b[0] = c.poles[0] * b[0] + w * c.gains[0];
        b[1] = c.poles[1] * b[1] + w * c.gains[1];
        b[2] = c.poles[2] * b[2] + w * c.gains[2];
        let pink = 0.25 * (b[0] + b[1] + b[2] + w * 0.1848);
        self.red = c.red.0 * self.red + c.red.1 * pink;
        let (x0, y0) = self.hp;
        let hp = c.band[0] * (y0 + w - x0);
        self.hp = (w, hp);
        self.lp += (hp - self.lp) * c.band[1];
        let (x0, y0) = self.pink_hp;
        let hp = c.band[2] * (y0 + pink - x0);
        self.pink_hp = (pink, hp);
        self.pink_lp += (hp - self.pink_lp) * c.band[3];
        (self.lp, self.pink_lp, self.red)
    }
}

/// The ladder: four one-pole stages, trapezoidal, the loop solved without delay for its
/// linear part; the loop's feedback through its own limit (how loud it rings).
#[derive(Debug, Clone, Copy, Default)]
struct Ladder {
    s: [f32; 4],
}

impl Ladder {
    /// One sample of input `x` (the pair's units) at stage gain `g` (G / (1 + G) of the
    /// prewarped coefficient G) and loop gain `k`: the fourth stage's output. The input pair
    /// takes the input less the loop's feedback through its knee ([`pair`], off its centre by
    /// `bias`; `limit` its limit, the limit's reciprocal and its output at the bias alone), as
    /// the circuit's does: driven hard, it squashes the resonance and loses less bass.
    #[inline(always)]
    fn tick(&mut self, x: f32, g: f32, k: f32, bias: f32, limit: (f32, f32, f32)) -> f32 {
        let s = &mut self.s;
        let h = 1.0 - g;
        let g2 = g * g;
        let g4 = g2 * g2;
        let rest = h * (g * (g * (g * s[0] + s[1]) + s[2]) + s[3]);
        let y4 = (g4 * x + rest) / (1.0 + k * g4);
        let (top, inv, rest_bias) = limit;
        let u = x - k * y4 + bias;
        let mut y = if top == 1.0 {
            tanh(u)
        } else {
            top * tanh(u * inv)
        } - rest_bias;
        for st in s.iter_mut() {
            let v = (y - *st) * g;
            let out = v + *st;
            *st = out + v;
            y = out;
        }
        y
    }
}

/// A contour generator: its level (V) and whether it is in its attack.
#[derive(Debug, Clone, Copy)]
struct Contour {
    volts: f64,
    attacking: bool,
    /// Never yet started (it rests where power-on left it).
    fresh: bool,
}

/// A contour's coefficients a sample (the attack's, the decay's, the release's), and the
/// levels it aims at, stops at, holds and rests at (V).
#[derive(Debug, Clone, Copy, Default)]
struct ContourLaw {
    attack: f64,
    decay: f64,
    release: f64,
    aim: f64,
    stop: f64,
    sustain: f64,
    released: f64,
    rest: f64,
    fall: f64,
}

impl ContourLaw {
    fn new(k: ContourKnobs, decay_switch: bool, which: usize, rate: f64) -> ContourLaw {
        type Law = [(f64, f64)];
        let (attack_law, decay_law): (&Law, &Law) = if which == 0 {
            (&FILTER_ATTACK, &FILTER_DECAY)
        } else {
            (&LOUDNESS_ATTACK, &LOUDNESS_DECAY)
        };
        let time = |(a, b, t0): (f64, f64, f64), ohms: f64| a * (ohms / 1e5).powf(b) + t0;
        let r_decay = time_pot(k.decay, decay_law);
        let tau_a = time(laws::ATTACK_TIME[which], time_pot(k.attack, attack_law));
        let (rest, peak) = (laws::REST[which], laws::PEAK[which]);
        let aim = rest + laws::ATTACK_AIM[which] * (peak - rest);
        let [c0, c1, c2, c3] = laws::SUSTAIN[which];
        let s = sustain_track(k.sustain);
        let (tau_r, released) = if decay_switch {
            (
                time(laws::RELEASE_TIME[which], r_decay),
                laws::RELEASED[which],
            )
        } else {
            let d = k.decay.clamp(0.0, 1.0);
            (
                interpolate(&laws::RELEASE_OFF_AT, &laws::RELEASE_OFF[which], d),
                interpolate(&laws::RELEASE_OFF_AT, &laws::RELEASED_OFF[which], d),
            )
        };
        let per = |tau: f64| 1.0 - (-1.0 / (tau * rate)).exp();
        ContourLaw {
            attack: per(tau_a),
            decay: per(time(laws::DECAY_TIME[which], r_decay)),
            release: per(tau_r),
            aim,
            stop: peak - (aim - peak) * laws::ATTACK_ESR[which] / tau_a,
            sustain: c0 + s * (c1 + s * (c2 + s * c3)),
            released,
            rest,
            fall: laws::DECAY_FALL[which],
        }
    }
}

impl Contour {
    /// At rest: as the circuit's from power-on (a release leaves it a little higher).
    fn at_rest(law: &ContourLaw) -> Contour {
        Contour {
            volts: law.rest,
            attacking: false,
            fresh: true,
        }
    }

    /// A sample on, its gate high or low.
    #[inline(always)]
    fn tick(&mut self, gate: bool, law: &ContourLaw) -> f64 {
        if gate {
            self.fresh = false;
            if self.attacking {
                let next = self.volts + (law.aim - self.volts) * law.attack;
                if next >= law.stop {
                    // (The comparator flips at its threshold: the attack ends there.)
                    self.volts = law.stop.max(self.volts);
                    self.attacking = false;
                } else {
                    self.volts = next;
                }
            } else {
                let to_go = self.volts - law.sustain;
                self.volts -= to_go * law.decay * (1.0 + law.fall * to_go);
            }
        } else if !self.fresh {
            self.attacking = false;
            self.volts += (law.released - self.volts) * law.release;
        }
        self.volts
    }
}

/// What the panel sets, worked out when it changes.
#[derive(Debug, Clone, Copy, Default)]
struct Setting {
    /// Each oscillator's pitch offset from its key, semitones, and whether it follows the
    /// keys (oscillator 3: OSC. 3 CONTROL).
    offset: [f64; 3],
    follows: [bool; 3],
    /// Each channel's gain into the bus: the oscillators, the noise, the external input.
    gain: [f32; 3],
    noise: f32,
    ext: f32,
    /// The cutoff's log2 Hz from CUTOFF, the keys' share, AMOUNT OF CONTOUR's octaves a
    /// volt, EMPHASIS's loop gain at each of [`laws::LOOP_AT_LOG2`]'s cutoffs.
    cutoff: f64,
    tracking: f64,
    contour: f64,
    emphasis: [f64; 10],
    /// How much of the corner's rise above the ring's pitch EMPHASIS leaves (1 below 7, 0 from
    /// the threshold).
    corner: f64,
    /// The modulation: its depth (the wheel), its mix, where it goes; oscillator 3 as its
    /// source: its sample to its waveform's -1..1 (times `swing`, plus `swing_bias`: the
    /// rectangles' levels before their coupling), and that to semitones.
    depth: f64,
    offset_depth: f64,
    mix: f32,
    osc_mod: bool,
    filter_mod: bool,
    swing: f32,
    swing_bias: f32,
    mod_swing: f32,
    mod_centre: f32,
    /// GLIDE's slide a control step, semitones (rising, falling), and its RC's share left a
    /// control step (beyond the keyboard's keys); the external input's gain to the output.
    glide: (f64, f64),
    beyond: f64,
    ext_gain: f64,
    contours: [ContourLaw; 2],
}

/// The circuit's filter spliced into the light voice (mpc-vst-ca72's HI; not upstream's):
/// the light oscillators, mixer and contours drive board 4's real-time ladder and its control
/// node, in place of the light ladder. The constants are fitted against the circuit voice at
/// 44.1 kHz (`ca72-mpc/examples/fit.rs`).
pub mod splice {
    /// The mixer bus's Norton current per light bus unit, A.
    pub const AMPS: f64 = 5.5453e-5;
    /// Each waveform's DC on the bus per unit of its channel's light gain (the circuit's
    /// rectangles swing from 0 to -4.3 V; the light ones are centred): triangle, shark tooth,
    /// sawtooth, square, wide, narrow.
    pub const DC: [f32; 6] = [-0.1008, -0.0899, -0.0394, -1.0964, -1.4864, -1.7440];
    /// The keyboard's voltage from the light voice's pitch (MIDI semitones), V: slope, offset
    /// (within 0.6 mV from key 29 to 96).
    pub const KBD: (f64, f64) = (0.084378, -3.040893);
    /// The filter contour's voltage from the light contour's, V (within 0.07 V).
    pub const ENV: (f64, f64) = (1.01292, -0.0024);
    /// The light voice's units per volt of the filter's output.
    pub const OUT: f64 = 0.4367;
}

#[derive(Debug, Clone)]
struct CircuitFilter {
    vcf: crate::vcf::Vcf,
    expo: crate::vcf::FilterExpo,
    table: std::sync::Arc<crate::vcf::ExpoTable>,
    /// What the panel sets: the bus's conductance, its DC (light units), CUTOFF's input,
    /// AMOUNT OF CONTOUR and the KEYBOARD CONTROL switches.
    g_bus: f64,
    dc: f32,
    cutoff: crate::expo::Input,
    amount: f64,
    kb: (bool, bool),
    /// The mixer's own noise (voice.rs's `MIXER_HISS`), from which the filter's
    /// self-oscillation starts, and its RMS a sample.
    hiss: crate::noise::Gauss,
    hiss_rms: f64,
}

impl CircuitFilter {
    fn new(rate: f64) -> CircuitFilter {
        use crate::filter_cal::CALIBRATED;
        let mut vcf = crate::vcf::Vcf::new(rate, 4);
        vcf.circuit = CALIBRATED.circuit();
        vcf.prepare(2);
        vcf.set_oversample(2);
        vcf.set_quality(crate::voice::Quality::Potato);
        CircuitFilter {
            vcf,
            expo: CALIBRATED.expo(),
            table: crate::vcf::ExpoTable::shared(CALIBRATED.expo(), 25.0),
            g_bus: crate::filter_cal::G_BUS_OFF,
            dc: 0.0,
            cutoff: crate::expo::Input { r: 200e3, v: 0.0 },
            amount: 0.0,
            kb: (true, false),
            hiss: crate::noise::Gauss::new(0x6869_7373),
            hiss_rms: crate::voice::MIXER_HISS * libm::sqrt(rate / 48e3),
        }
    }

    fn set_panel(&mut self, p: &Panel, gain: &[f32; 3]) {
        // A channel's conductance onto the bus through its VOLUME pot (voice.rs's `channel`).
        let channel = |r_src: f64, volume: f64, on: bool, r_series: f64| {
            if !on {
                return 1.0 / r_series;
            }
            let t = volume_track(volume);
            let (up, down) = (r_src + 25e3 * (1.0 - t), (25e3 * t).max(1e-3));
            1.0 / (up * down / (up + down) + r_series)
        };
        let mut g = 1.0 / 33e3;
        let mut dc = 0.0;
        for (o, op) in p.osc.iter().enumerate() {
            let (r, k) = match op.waveform {
                Waveform::Triangle => (laws::SOURCE_OHMS[0], splice::DC[0]),
                Waveform::SharkTooth | Waveform::ReverseSawtooth => {
                    (laws::SOURCE_OHMS[3], splice::DC[1])
                }
                Waveform::Sawtooth => (laws::SOURCE_OHMS[1], splice::DC[2]),
                Waveform::Square => (laws::SOURCE_OHMS[2], splice::DC[3]),
                Waveform::WideRectangle => (laws::SOURCE_OHMS[2], splice::DC[4]),
                Waveform::NarrowRectangle => (laws::SOURCE_OHMS[2], splice::DC[5]),
            };
            g += channel(r, op.volume, op.on, 33e3);
            dc += k * gain[o];
        }
        g += channel(
            if p.noise_pink { 24e3 } else { 0.0 },
            p.noise_volume,
            p.noise_on,
            11e3,
        );
        self.g_bus = g;
        self.dc = dc;
        let pos = crate::voice::cutoff_track(p.cutoff);
        self.cutoff = crate::expo::Input {
            r: 200e3 + 5e3 * pos * (1.0 - pos),
            v: -10.0 + 20.0 * pos,
        };
        self.amount = p.contour_amount.clamp(0.0, 1.0);
        self.kb = (p.keyboard_control_1, p.keyboard_control_2);
        self.vcf.circuit.r14 = emphasis_r14(p.emphasis);
        self.vcf.high_pass = p.filter_hi;
    }

    /// The ladder's current from the control node: the light contour's and pitch's voltages
    /// and `octaves` more (the modulation and ENTROPY's offset) at R51, as volts.
    #[inline]
    fn i0(&self, env: f64, pitch: f64, octaves: f64) -> f64 {
        use crate::expo::Input;
        let v_kbd = splice::KBD.0 * pitch + splice::KBD.1;
        let env = splice::ENV.0 * env + splice::ENV.1;
        let open = Input {
            r: f64::INFINITY,
            v: 0.0,
        };
        let ins = [
            self.cutoff,
            crate::voice::contour_input(self.amount, env),
            if self.kb.0 {
                Input {
                    r: crate::vcf::R53,
                    v: v_kbd,
                }
            } else {
                open
            },
            if self.kb.1 {
                Input {
                    r: crate::vcf::R54,
                    v: v_kbd,
                }
            } else {
                open
            },
            Input { r: 33e3, v: 0.0 },
            Input {
                r: 100e3,
                v: octaves * crate::voice::FILTER_VOLTS_PER_OCTAVE,
            },
        ];
        let (i, g) = self.expo.node(&ins);
        self.table.current_and_node(i, g).0
    }
}

/// Potato mode's voice.
#[derive(Debug, Clone)]
pub struct Light {
    rate: f64,
    /// The panel, and whether `set` is worked out from it.
    panel: Panel,
    set_for_panel: bool,
    set: Setting,
    held: Held,
    /// When each key was last pressed, counted in presses (for LAST), and the count.
    pressed: [u32; 128],
    presses: u32,
    /// RETRIGGER's keys pressed, waiting for the keys' gate to have been low `gap` samples
    /// (voice.rs's `RETRIGGER_GAP`).
    waiting: Held,
    gap: u32,
    /// A released key whose pitch contact is still closed, the samples it stays so, and that
    /// time in samples.
    lead: (i32, u32),
    lead_samples: u32,
    /// The pitch the keys ask for and where GLIDE has it, semitones (MIDI): the keyboard's
    /// own (F to C), and the keys beyond it (asked for, and where they are).
    target: f64,
    pitch: f64,
    foot: f64,
    beyond: f64,
    /// The keys' gate; the contours' (the trigger's), the samples to its next change, the
    /// samples the keys' gate has been low, and the trigger's times in samples.
    gate: bool,
    trigger: bool,
    pending: u32,
    /// Whether the trigger, when it follows the keys up, starts an attack (re-armed).
    attack: bool,
    open: u32,
    rearm: u32,
    trigger_on: u32,
    trigger_off: u32,
    osc: [Osc; 3],
    /// Each oscillator's increment, its step a sample, and the reciprocal of where it goes.
    dt: [f32; 3],
    dt_step: [f32; 3],
    inv: [f32; 3],
    noise: Noise,
    noise_c: NoiseLaw,
    /// FEEDBACK's cable at the rate, samples.
    feedback_delay: f32,
    /// The output's high-passes (its droop): the filter's coupling and the one after the VCA,
    /// each its coefficient, last input and output; FILTER MODE HI's direct branch's
    /// coupling likewise, and that branch's gain from the bus (control rate); the input
    /// pair's offset at rest.
    coupling: (f32, f32, f32),
    droop: (f32, f32, f32),
    mode: (f32, f32, f32),
    direct: f32,
    /// The filter's passband at its cutoff (control rate).
    pass: f32,
    /// The thump's band-pass: its coefficients (high-pass, low-pass) and its state (the
    /// VCA's last gain, the high-pass's output, the low-pass's).
    thump_c: (f32, f32),
    thump: (f32, f32, f32),
    bias: f32,
    ladder: Ladder,
    /// The ladder's stage gain (and its step a sample) and loop gain.
    g: f32,
    g_step: f32,
    k: f32,
    contours: [Contour; 2],
    count: usize,
    /// ENTROPY's offsets: each oscillator's cents and the cutoff's octaves.
    detune: [f64; 3],
    cutoff_offset: f64,
    /// Oscillator 3's last sample (the modulation's source).
    osc3: f32,
    /// A-440's phase and its step a sample.
    a440: (f32, f32),
    /// FEEDBACK's share; the output's last samples (FEEDBACK's delay) and where the next
    /// goes; the OVERLOAD lamp's brightest.
    pub feedback: f64,
    /// BASS COMP (mpc-vst-ca72's mod): the input raised by `bass_amount` of the loop gain.
    pub bass_comp: bool,
    pub bass_amount: f32,
    fed_back: [f32; 128],
    fed_at: usize,
    lamp: f64,
    /// EXTERNAL INPUT's couplings (the jack's, the preamplifier's): each its coefficient, last
    /// input and output.
    ext_hp: (f64, f64, f64),
    pre_hp: (f32, f32, f32),
    overload_peak: f64,
    /// The plug-in's DRIVE: the gain on the bus into the ladder's input pair (1 the circuit),
    /// and the pair's limit with it (as [`Ladder::tick`] takes it).
    drive: f32,
    drive_top: f32,
    limit: (f32, f32, f32),
    /// Probes for fitting the circuit filter's splice (mpc-vst-ca72): the last sample's bus,
    /// the ladder's output through its passband, and the last control step's nominal cutoff
    /// (log2 Hz, before the corner's corrections).
    pub probe_bus: f32,
    pub probe_y: f32,
    pub probe_l: f64,
    pub probe_pitch: f64,
    pub probe_env: f64,
    /// The circuit's filter (mpc-vst-ca72's HI), made once and played while `circuit_on`.
    circuit: Option<Box<CircuitFilter>>,
    circuit_on: bool,
}

impl Light {
    /// A voice at `rate` Hz, at rest, with the default panel.
    pub fn new(rate: f64) -> Light {
        let at = |s: f64| (s * rate).round() as u32;
        let law = ContourLaw::new(Panel::default().filter_contour, true, 0, rate);
        let mut l = Light {
            rate,
            panel: Panel::default(),
            set_for_panel: false,
            set: Setting::default(),
            held: Held::default(),
            pressed: [0; 128],
            presses: 0,
            waiting: Held::default(),
            gap: at(crate::voice::RETRIGGER_GAP),
            lead: (0, 0),
            lead_samples: at(laws::RELEASE_LEAD),
            target: laws::PITCH_AT_REST,
            pitch: laws::PITCH_AT_REST,
            foot: 0.0,
            beyond: 0.0,
            gate: false,
            trigger: false,
            pending: 0,
            attack: false,
            open: u32::MAX,
            rearm: at(laws::REARM),
            trigger_on: at(laws::TRIGGER_ON),
            trigger_off: at(laws::TRIGGER_OFF),
            osc: [Osc {
                phase: 0.0,
                wave: Waveform::Sawtooth,
            }; 3],
            dt: [0.0; 3],
            dt_step: [0.0; 3],
            inv: [0.0; 3],
            noise: Noise {
                state: 0x4D6F_6F67,
                pink: [0.0; 3],
                red: 0.0,
                hp: (0.0, 0.0),
                lp: 0.0,
                pink_hp: (0.0, 0.0),
                pink_lp: 0.0,
            },
            noise_c: NoiseLaw::new(rate),
            feedback_delay: interpolate(&laws::FEEDBACK_AT, &laws::FEEDBACK_DELAY, rate) as f32,
            coupling: (
                (-std::f64::consts::TAU * laws::COUPLING_HZ / rate).exp() as f32,
                0.0,
                0.0,
            ),
            droop: (
                (-std::f64::consts::TAU * laws::DROOP_HZ / rate).exp() as f32,
                0.0,
                0.0,
            ),
            mode: (
                (-std::f64::consts::TAU * laws::MODE_HZ / rate).exp() as f32,
                0.0,
                0.0,
            ),
            direct: 0.0,
            pass: 1.0,
            thump_c: (
                (-1.0 / (laws::THUMP_TIMES.0 * rate)).exp() as f32,
                (1.0 - (-1.0 / (laws::THUMP_TIMES.1 * rate)).exp()) as f32,
            ),
            thump: (0.0, 0.0, 0.0),
            bias: tanh(laws::BIAS),
            ladder: Ladder::default(),
            g: 0.0,
            g_step: 0.0,
            k: 0.0,
            contours: [Contour::at_rest(&law); 2],
            count: 0,
            detune: [0.0; 3],
            cutoff_offset: 0.0,
            osc3: 0.0,
            a440: (0.0, (440.0 / rate) as f32),
            feedback: 0.0,
            bass_comp: false,
            bass_amount: 1.0,
            fed_back: [0.0; 128],
            fed_at: 0,
            lamp: 0.0,
            ext_hp: (
                (-std::f64::consts::TAU * laws::EXT_COUPLING_HZ / rate).exp(),
                0.0,
                0.0,
            ),
            pre_hp: (
                (-std::f64::consts::TAU * laws::PREAMP_COUPLING_HZ / rate).exp() as f32,
                0.0,
                0.0,
            ),
            overload_peak: 0.0,
            drive: 1.0,
            drive_top: 1.0,
            limit: (1.0, 1.0, 0.0),
            probe_bus: 0.0,
            probe_y: 0.0,
            probe_l: 0.0,
            probe_pitch: 0.0,
            probe_env: 0.0,
            circuit: None,
            circuit_on: false,
        };
        l.set_drive(1.0);
        // (The oscillators start in phase, as the circuit's do when its voice is made; ENTROPY's
        // floor of mismatch then draws them apart: decisions.md R9, R31.)
        l.set_panel(&Panel::default());
        l.contours = [
            Contour::at_rest(&l.set.contours[0]),
            Contour::at_rest(&l.set.contours[1]),
        ];
        l
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The plug-in's DRIVE, as [`crate::voice::Voice::set_drive`]: the mixer's signal into the
    /// filter's input pair raised by `gain` (the ladder's feedback not), driving the pair
    /// harder than the panel's mixer can; 1 is the circuit.
    pub fn set_drive(&mut self, gain: f64) {
        let gain = gain.max(1e-3);
        self.drive = gain as f32;
        self.drive_top = gain.powf(laws::DRIVE_CEILING).min(1.0) as f32;
        let top = self.drive_top;
        self.limit = (top, 1.0 / top, top * tanh(laws::BIAS / top));
        if let Some(c) = &mut self.circuit {
            c.vcf.set_drive(gain);
        }
    }

    /// The three oscillators started together `at` one share of the way through their cycles
    /// (from the sawtooth's top), as [`crate::voice::Voice::start_oscillators_at`]: for a voice
    /// made or put back to rest that is not the first, so that voices playing one note
    /// together do not start in step. Never at a note.
    pub fn start_oscillators_at(&mut self, at: f64) {
        let at = at.clamp(0.0, 0.98) as f32;
        for o in &mut self.osc {
            o.phase = at;
        }
    }

    /// Seeds the noise (each voice its own stream).
    pub fn set_seed(&mut self, seed: u64) {
        let s = (seed ^ (seed >> 32)) as u32;
        self.noise.state = if s == 0 { 0x4D6F_6F67 } else { s };
    }

    /// The panel: what it sets is worked out again if it changed (some microseconds: not for
    /// every sample).
    pub fn set_panel(&mut self, p: &Panel) {
        if self.set_for_panel && *p == self.panel {
            return;
        }
        self.set_for_panel = true;
        self.panel = *p;
        let cubic = |c: [f64; 4], x: f64| c[0] + x * (c[1] + x * (c[2] + x * c[3]));
        for o in 0..3 {
            let range = (2.0 - p.osc[o].range.octaves_below_2()) * 12.0;
            let x = p.osc[o].freq.clamp(0.0, 1.0);
            let free = o == 2 && !p.osc3_control;
            let freq = match o {
                0 => 0.0,
                1 => {
                    let at = |x: f64| cubic(laws::OSC2_CENTS, x);
                    (at(x) - at(FREQ_CENTRE)) / 100.0
                }
                _ if free => {
                    let quintic = |x: f64| {
                        laws::OSC3_FREE_CENTS
                            .iter()
                            .rev()
                            .fold(0.0, |a, c| a * x + c)
                    };
                    (quintic(x) - quintic(FREQ_CENTRE)) / 100.0
                }
                _ => {
                    let at = |x: f64| cubic(laws::OSC3_CENTS, x);
                    (at(x) - at(FREQ_CENTRE)) / 100.0
                }
            };
            self.osc[o].wave = p.osc[o].waveform;
            let tune = if free {
                0.0
            } else {
                p.tune * laws::TUNE_CENTS / 100.0
            };
            self.set.offset[o] = range + freq + tune;
            let source = match p.osc[o].waveform {
                Waveform::Triangle => laws::SOURCE_OHMS[0],
                Waveform::Sawtooth | Waveform::ReverseSawtooth => laws::SOURCE_OHMS[1],
                Waveform::SharkTooth => laws::SOURCE_OHMS[3],
                _ => laws::SOURCE_OHMS[2],
            };
            let level = if o == 2 { laws::OSC3_LEVEL } else { 1.0 };
            self.set.gain[o] = if p.osc[o].on {
                channel(volume_track(p.osc[o].volume), source, 33e3) * level
            } else {
                0.0
            };
        }
        self.set.follows = [true, true, p.osc3_control];
        self.set.noise = if p.noise_on {
            let (ohms, level) = if p.noise_pink {
                (laws::NOISE_OHMS.1, laws::PINK)
            } else {
                (laws::NOISE_OHMS.0, laws::WHITE)
            };
            channel(volume_track(p.noise_volume), ohms, 11e3) / channel(1.0, ohms, 11e3) * level
        } else {
            0.0
        };
        // (The external input works at the output's level; the bus's is that over OUT and
        // DRIVE through the ladder's passband at EMPHASIS 0.)
        // (The pair's slope at its offset.)
        let slope = 1.0 - self.bias * self.bias;
        let passband = f64::from(laws::OUT * laws::DRIVE * slope)
            / (1.0 + laws::LOOP[3][0] * laws::LOOP_SCALE);
        // FILTER MODE HI's direct branch: the open ladder's passband at EMPHASIS 0, as the
        // circuit's is the same at every cutoff (vcf.rs's `MODE_RT`).
        self.direct = laws::MODE_GAIN * laws::DRIVE * slope
            / (1.0 + (laws::LOOP[9][0] * laws::LOOP_SCALE) as f32);
        self.set.ext = if p.ext_on {
            (1.0 / passband) as f32
        } else {
            0.0
        };
        let t = ext_taper(p.ext_volume);
        self.set.ext_gain = laws::EXT_GAIN * t / (1.0 + laws::EXT_LOAD * t * (1.0 - t));
        let knob = p.cutoff.clamp(0.0, 1.0) * 20.0;
        let i = (knob.floor() as usize).min(19);
        let a = knob - i as f64;
        let c = &laws::CUTOFF_LOG2;
        self.set.cutoff = f64::from(c[i]) + (f64::from(c[i + 1]) - f64::from(c[i])) * a;
        self.set.tracking = match (p.keyboard_control_1, p.keyboard_control_2) {
            (false, false) => 0.0,
            (true, false) => laws::KEYBOARD_CONTROL[0],
            (false, true) => laws::KEYBOARD_CONTROL[1],
            (true, true) => laws::KEYBOARD_CONTROL[2],
        };
        self.set.contour = laws::CONTOUR_OCTAVES_PER_VOLT
            * interpolate(
                &[0.0, 0.25, 0.5, 0.75, 1.0],
                &laws::AMOUNT,
                p.contour_amount,
            );
        // (EMPHASIS as the knob drawn with its generic taper would set R14.)
        let emphasis = 1.0 - (1.0 + 80.0 * emphasis_r14(p.emphasis) / 50e3).ln() / 81f64.ln();
        self.set.corner = ((0.744 - emphasis) / 0.044).clamp(0.0, 1.0);
        for ((k, row), &log2) in self
            .set
            .emphasis
            .iter_mut()
            .zip(&laws::LOOP)
            .zip(&laws::LOOP_AT_LOG2)
        {
            let threshold = interpolate(&laws::THRESHOLD_AT_LOG2, &laws::THRESHOLD, log2);
            let shift = interpolate(&laws::THRESHOLD_AT_LOG2, &laws::SHIFT, log2);
            let near = 1.0 - ((threshold - emphasis) / laws::EMPHASIS_NEAR).clamp(0.0, 1.0);
            let at = interpolate(&laws::EMPHASIS_AT, row, emphasis + near * shift);
            // (Past the threshold, `RING_GAIN` of the rise beyond it.)
            *k = laws::LOOP_SCALE
                * if at > 4.0 {
                    4.0 + (at - 4.0) * laws::RING_GAIN
                } else {
                    at
                };
        }
        // (The wheel as the drawing's would set its resistance.)
        let r = crate::modulation::mod_wheel_r(p.mod_wheel);
        let w = 3.0 * (1.0 + r / 1.2e3 * (81f64.powf(1.0 / 3.0) - 1.0)).ln() / 81f64.ln();
        let w = w.clamp(0.0, 1.0);
        let [a, b, c] = laws::WHEEL;
        self.set.depth = w * (a + w * (b + w * c));
        self.set.offset_depth = laws::OFFSET.0 * r / (r + laws::OFFSET.1);
        self.set.mix = p.mod_mix.clamp(0.0, 1.0) as f32;
        self.set.osc_mod = p.osc_mod;
        self.set.filter_mod = p.filter_mod;
        let w3 = p.osc[2].waveform;
        self.set.swing = 1.0 / Osc::peak(w3);
        let (kind, duty) = match w3 {
            Waveform::Triangle => (0, None),
            Waveform::ReverseSawtooth | Waveform::SharkTooth => (1, None),
            Waveform::Sawtooth => (2, None),
            Waveform::Square => (3, Some(laws::DUTY[0])),
            Waveform::WideRectangle => (3, Some(laws::DUTY[1])),
            Waveform::NarrowRectangle => (3, Some(laws::DUTY[2])),
        };
        self.set.swing_bias = duty.map_or(0.0, |d| 2.0 * d - 1.0);
        self.set.mod_swing = laws::MOD_SWING[kind] as f32;
        self.set.mod_centre = laws::MOD_CENTRE[kind] as f32;
        // (GLIDE off, the hold's own 330 ohm: at once.)
        let ohms = glide_r(p.glide, p.glide_on) + 330.0;
        let step = laws::GLIDE_RATE / ohms * CONTROL_EVERY as f64 / self.rate;
        self.set.glide = (step, step * laws::GLIDE_FALLING);
        self.set.beyond = (-(CONTROL_EVERY as f64 / self.rate) / (ohms * laws::GLIDE_FARADS)).exp();
        self.set.contours = [
            ContourLaw::new(p.filter_contour, p.decay, 0, self.rate),
            ContourLaw::new(p.loudness_contour, p.decay, 1, self.rate),
        ];
        if let Some(c) = &mut self.circuit {
            c.set_panel(p, &self.set.gain);
        }
    }

    /// Makes the circuit's filter (some milliseconds and allocation: not on the audio thread)
    /// so that [`Light::use_circuit_filter`] can switch to it at once.
    pub fn make_circuit_filter(&mut self) {
        if self.circuit.is_none() {
            let mut c = Box::new(CircuitFilter::new(self.rate));
            c.set_panel(&self.panel, &self.set.gain);
            c.vcf.set_drive(f64::from(self.drive));
            self.circuit = Some(c);
        }
    }

    /// Takes the circuit's filter `from` another voice (one made off the audio thread), set
    /// for this voice's panel and DRIVE: no allocation.
    pub fn take_circuit_filter(&mut self, from: &mut Light) {
        if let Some(mut c) = from.circuit.take() {
            c.set_panel(&self.panel, &self.set.gain);
            c.vcf.set_drive(f64::from(self.drive));
            self.circuit = Some(c);
        }
    }

    /// Plays the circuit's filter in place of the light ladder (if made), or the light one.
    pub fn use_circuit_filter(&mut self, on: bool) {
        self.circuit_on = on && self.circuit.is_some();
    }

    /// Whether the circuit's filter is playing.
    pub fn circuit_filter(&self) -> bool {
        self.circuit_on
    }

    /// Each oscillator's gain onto the bus (light units; for fitting the splice).
    pub fn bus_gains(&self) -> [f32; 3] {
        self.set.gain
    }

    /// ENTROPY's offsets for the samples that follow: each oscillator's cents, the cutoff's
    /// octaves.
    pub fn set_offsets(&mut self, detune: [f64; 3], cutoff: f64) {
        self.detune = detune;
        self.cutoff_offset = cutoff;
    }

    /// A MIDI note pressed or released.
    pub fn note(&mut self, midi: i32, on: bool) {
        if !(0..128).contains(&midi) {
            return;
        }
        let bit = 1u128 << midi;
        if on && (self.held.0 | self.waiting.0) & bit == 0 {
            self.presses = self.presses.wrapping_add(1);
            self.pressed[midi as usize] = self.presses;
            if self.panel.retrigger {
                // (Down once the keys' gate has been low long enough: `tick`.)
                self.waiting.set(midi, true);
                return;
            }
        }
        if !on && self.waiting.0 & bit != 0 {
            // (Let go before it came down: it never sounded.)
            self.waiting.set(midi, false);
            return;
        }
        if !on && self.sounding() == Some(midi) {
            self.lead = (midi, self.lead_samples);
        }
        self.held.set(midi, on);
    }

    /// The key held that sounds, as the panel's priority chooses it.
    fn sounding(&self) -> Option<i32> {
        match self.panel.priority {
            Priority::Low => self.held.lowest(),
            Priority::High => self.held.highest(),
            Priority::Last => self.held.newest(&self.pressed),
        }
    }

    /// Whether a key is held (or pressed and waiting to come down).
    pub fn held(&self) -> bool {
        self.held.0 != 0 || self.waiting.0 != 0
    }

    /// As [`crate::voice::Voice::trigger_closed`]: the keys' gate at the last sample.
    pub fn trigger_closed(&self) -> bool {
        self.gate
    }

    /// As [`crate::voice::Voice::slow_state`]: the pitch (as volts, an octave a volt) and
    /// the contours, V, once no key is down and the trigger has followed.
    pub fn slow_state(&self) -> Option<[f64; 3]> {
        if self.held() || self.trigger || self.pending > 0 {
            return None;
        }
        Some([
            (self.pitch + self.beyond) / 12.0,
            self.contours[0].volts,
            self.contours[1].volts,
        ])
    }

    /// The contours (filter, loudness), V.
    pub fn contours(&self) -> (f64, f64) {
        (self.contours[0].volts, self.contours[1].volts)
    }

    /// The OVERLOAD lamp now (0 off, 1 fully lit).
    pub fn overload(&self) -> f64 {
        self.lamp
    }

    /// The OVERLOAD lamp's brightest since the last call.
    pub fn take_overload_peak(&mut self) -> f64 {
        std::mem::take(&mut self.overload_peak)
    }

    /// Back to rest: every key up, the contours and the ladder at rest (All Sound Off).
    pub fn rest(&mut self) {
        self.held = Held::default();
        self.waiting = Held::default();
        self.lead = (0, 0);
        (self.gate, self.trigger, self.pending, self.open) = (false, false, 0, u32::MAX);
        self.contours = [
            Contour::at_rest(&self.set.contours[0]),
            Contour::at_rest(&self.set.contours[1]),
        ];
        self.ladder = Ladder::default();
        self.coupling = (self.coupling.0, 0.0, 0.0);
        self.droop = (self.droop.0, 0.0, 0.0);
        self.mode = (self.mode.0, 0.0, 0.0);
        self.thump = (0.0, 0.0, 0.0);
        self.ext_hp = (self.ext_hp.0, 0.0, 0.0);
        self.pre_hp = (self.pre_hp.0, 0.0, 0.0);
        self.fed_back = [0.0; 128];
        // (In phase again, as the circuit's clean voice is.)
        for o in &mut self.osc {
            o.phase = 0.0;
        }
        self.pitch = self.target;
        self.beyond = self.foot;
    }

    /// The control laws, every [`CONTROL_EVERY`] samples: the pitch (GLIDE), the
    /// oscillators' increments and the ladder's coefficients for the next samples.
    fn control(&mut self) {
        let sounding = self.sounding();
        let sounding = if self.lead.1 > 0 {
            self.lead.1 = self.lead.1.saturating_sub(CONTROL_EVERY as u32);
            let lead = self.lead.0;
            sounding.map(|k| match self.panel.priority {
                Priority::Low => k.min(lead),
                Priority::High => k.max(lead),
                Priority::Last if self.pressed[lead as usize] > self.pressed[k as usize] => lead,
                Priority::Last => k,
            })
        } else {
            sounding
        };
        if let Some(key) = sounding {
            let (lo, hi) = laws::KEYBOARD_KEYS;
            let key = f64::from(key);
            self.target = key.clamp(lo, hi);
            self.foot = key - self.target;
            // (The track-and-hold follows while a key is down.)
            let (up, down) = self.set.glide;
            self.pitch += (self.target - self.pitch).clamp(-down, up);
            self.beyond = self.foot + (self.beyond - self.foot) * self.set.beyond;
        }
        let pitch = self.pitch + self.beyond;
        let wheel = self.panel.pitch_wheel * crate::modulation::PITCH_WHEEL_SEMITONES;
        for o in 0..3 {
            let key = if self.set.follows[o] {
                pitch + wheel
            } else {
                laws::OSC3_FREE_KEY
            };
            let semis = key - 69.0 + self.set.offset[o] + self.detune[o] / 100.0;
            let hz = laws::A4 * f64::from(exp2((semis / 12.0) as f32));
            let to = ((hz / self.rate) as f32).min(0.45);
            self.dt_step[o] = (to - self.dt[o]) / CONTROL_EVERY as f32;
            self.inv[o] = 1.0 / to.max(1e-9);
        }
        let (g, k) = self.coefficients(0.0);
        self.g_step = (g - self.g) / CONTROL_EVERY as f32;
        self.k = k;
        let log2 = self.set.cutoff
            + self.set.tracking * (self.pitch + self.beyond - laws::TRACKING_FROM) / 12.0
            + self.set.contour * (self.contours[0].volts - self.set.contours[0].rest)
            + self.cutoff_offset;
        self.probe_l = log2;
        // The passband at this cutoff.
        let pass = interpolate(&laws::PASS_AT_LOG2, &laws::PASS_DB, log2);
        self.pass = f64::from(exp2((pass * (std::f64::consts::LOG2_10 / 20.0)) as f32)) as f32;
        // Past the threshold, the pair's limit for the ring's level at this cutoff.
        let ringing = 1.0 - self.set.corner;
        let top = if ringing > 0.0 {
            let lift = interpolate(&laws::RING_LEVEL_AT_LOG2, &laws::RING_LEVEL, log2);
            self.drive_top * (1.0 + ringing * (lift - 1.0)) as f32
        } else {
            self.drive_top
        };
        if top != self.limit.0 {
            self.limit = (top, 1.0 / top, top * tanh(laws::BIAS / top));
        }
    }

    /// The ladder's stage gain and loop gain for the cutoff the panel, the keys and the
    /// filter contour set, `extra` octaves added.
    #[inline(always)]
    fn coefficients(&self, extra: f64) -> (f32, f32) {
        let s = &self.set;
        let rise = self.contours[0].volts - s.contours[0].rest;
        let log2 = s.cutoff
            + s.tracking * (self.pitch + self.beyond - laws::TRACKING_FROM) / 12.0
            + s.contour * rise
            + self.cutoff_offset
            + extra;
        let corner = interpolate(&laws::CORNER_AT_LOG2, &laws::CORNER, log2) * s.corner
            + interpolate(&laws::RING_RAISE_AT_LOG2, &laws::RING_RAISE, log2) * (1.0 - s.corner);
        let hz = f64::from(exp2((log2 + corner) as f32));
        let w = (std::f64::consts::PI * hz / self.rate).min(1.4);
        // tan by its Padé form, close enough below 1.4 for the ladder's tuning.
        let w2 = w * w;
        let t = w * (15.0 - w2) / (15.0 - 6.0 * w2);
        let t = if t.is_finite() && t > 0.0 { t } else { 1e3 };
        // The loop's gain at this cutoff, between the fitted ones.
        let k = interpolate(&laws::LOOP_AT_LOG2, &s.emphasis, log2);
        ((t / (1.0 + t)) as f32, k as f32)
    }

    /// One output sample (1.0 = 5 V), `ext` at EXTERNAL INPUT.
    #[inline]
    pub fn tick(&mut self, ext: f64) -> f64 {
        if self.count == 0 {
            self.count = CONTROL_EVERY;
            self.control();
        }
        self.count -= 1;
        // RETRIGGER's keys come down once the keys' gate has been low long enough for the
        // contours to start again; until then it is low, the keys held as they were.
        if self.waiting.0 != 0 && !self.gate && self.open >= self.gap {
            self.held.0 |= self.waiting.0;
            self.waiting = Held::default();
        }
        // The trigger: the contours follow the keys' gate late, and a key pressed again
        // within the re-arming time does not start them again.
        let gate = self.held.0 != 0 && self.waiting.0 == 0;
        if gate != self.gate {
            self.gate = gate;
            if gate {
                self.attack = self.open >= self.rearm;
                self.pending = if self.attack {
                    // (Re-armed: an attack, even if the contours have not yet fallen.)
                    self.trigger_on.max(1)
                } else if self.trigger {
                    // (Let go and pressed again before the contours fell: they hold.)
                    0
                } else {
                    1
                };
            } else {
                self.pending = self.trigger_off.max(1);
            }
            self.open = 0;
        }
        if !gate {
            self.open = self.open.saturating_add(1);
        }
        if self.pending > 0 {
            self.pending -= 1;
            if self.pending == 0 {
                self.trigger = gate;
                if gate {
                    for c in &mut self.contours {
                        c.attacking = self.attack;
                    }
                }
            }
        }
        let trigger = self.trigger;
        self.contours[0].tick(trigger, &self.set.contours[0]);
        let loud = self.contours[1].tick(trigger, &self.set.contours[1]);

        // (What the panel sets, read as needed: not the whole of it copied each sample.)
        let s = &self.set;
        let (gain, noise_gain, ext_bus, ext_gain) = (s.gain, s.noise, s.ext, s.ext_gain);
        let (depth, offset_depth, mix_k) = (s.depth, s.offset_depth, s.mix);
        let (swing, swing_bias, mod_swing, mod_centre) =
            (s.swing, s.swing_bias, s.mod_swing, s.mod_centre);
        let (osc_mod, filter_mod, follows3) = (s.osc_mod, s.filter_mod, s.follows[2]);
        for o in 0..3 {
            self.dt[o] += self.dt_step[o];
        }
        let mut dt = self.dt;
        let mut inv = self.inv;
        let mut g = self.g + self.g_step;
        self.g = g;
        let k = self.k;
        let modulating = depth > 0.0 && (osc_mod || filter_mod);
        let mut fm_octaves = 0.0f32;
        let needs_noise = noise_gain != 0.0 || (modulating && mix_k > 0.0);
        let (white, pink, red) = if needs_noise {
            self.noise.tick(&self.noise_c)
        } else {
            (0.0, 0.0, 0.0)
        };
        if modulating {
            // Oscillator 3 (its swing to ±1) and the noise (its RMS to 1) as MODULATION MIX
            // blends them, and the line's offset, through the wheel.
            let (n, rms) = if self.panel.noise_pink {
                (red, 2.163)
            } else {
                (pink, 0.425)
            };
            // (Oscillator 3's semitones at the wheel fully forward.)
            let o = (self.osc3 * swing + swing_bias) * mod_swing + mod_centre;
            let mix = f64::from(mix_k);
            let depth = depth as f32;
            let offset = offset_depth as f32;
            if osc_mod {
                let semis = depth
                    * ((1.0 - mix_k) * o + mix_k * n * (laws::NOISE_SEMITONES_RMS / rms) as f32)
                    + offset
                        * ((1.0 - mix) * laws::MOD_OFFSET.0 + mix * laws::NOISE_OFFSET.0) as f32;
                let r = exp2(semis * (1.0 / 12.0));
                let ri = 1.0 / r;
                for o in 0..if follows3 { 3 } else { 2 } {
                    dt[o] = (dt[o] * r).min(0.45);
                    inv[o] *= ri;
                }
            }
            if filter_mod {
                let octaves = depth
                    * ((1.0 - mix_k) * o * laws::FILTER_PER_SEMITONE as f32
                        + mix_k * n * (laws::NOISE_OCTAVES_RMS / rms) as f32)
                    + offset
                        * ((1.0 - mix) * laws::MOD_OFFSET.1 + mix * laws::NOISE_OFFSET.1) as f32;
                // (On the step's prewarped coefficient, tan scaling as the cutoff does below
                // Nyquist's neighbourhood; the loop's gain as the step has it.)
                fm_octaves = octaves;
                let t = (g / (1.0 - g)) * exp2(octaves);
                g = t.min(5.8) / (1.0 + t.min(5.8));
            }
        }
        // The oscillators.
        // (Each oscillator's increment's reciprocal the control step's: close enough for its
        // polyBLEP's two samples.)
        let w0 = if gain[0] != 0.0 {
            self.osc[0].tick(dt[0], inv[0])
        } else {
            0.0
        };
        let w1 = if gain[1] != 0.0 {
            self.osc[1].tick(dt[1], inv[1])
        } else {
            0.0
        };
        // (Oscillator 3 also as the modulation's source; neither heard nor modulating, its
        // phase rests, as an unheard free-running oscillator's may.)
        let w2 = if gain[2] != 0.0 || modulating {
            self.osc[2].tick(dt[2], inv[2])
        } else {
            0.0
        };
        self.osc3 = w2;
        let noise = if self.panel.noise_pink { pink } else { white };
        // EXTERNAL INPUT through its preamplifier (FEEDBACK's cable added), at the output's
        // level.
        let ext_in = if self.feedback > 0.0 {
            let d = self.feedback_delay;
            let back = |n: usize| self.fed_back[(self.fed_at + 128 - n) & 127];
            let (n, f) = (d as usize, d.fract());
            ext + self.feedback * f64::from(back(n) + (back(n + 1) - back(n)) * f)
        } else {
            ext
        };
        let mut pre = 0.0f32;
        // (Through its coupling, while anything is in it.)
        let ext_in = if ext_in != 0.0 || self.ext_hp.2 != 0.0 {
            let (a, x0, y0) = self.ext_hp;
            let y = a * (y0 + ext_in - x0);
            let y = if y.abs() < 1e-15 && ext_in == 0.0 {
                0.0
            } else {
                y
            };
            self.ext_hp = (a, ext_in, y);
            y
        } else {
            0.0
        };
        if ext_in != 0.0 {
            let x = ext_in * ext_gain;
            let (on, full) = laws::OVERLOAD;
            let lamp = ((x.abs() - on) / (full - on)).clamp(0.0, 1.0);
            self.lamp = lamp;
            self.overload_peak = self.overload_peak.max(lamp);
            pre = tanh((x / laws::EXT_CLIP) as f32) * laws::EXT_CLIP as f32;
        }
        // (The preamplifier's output coupling, while anything is in it.)
        if pre != 0.0 || self.pre_hp.2 != 0.0 {
            let (a, x0, y0) = self.pre_hp;
            let y = a * (y0 + pre - x0);
            let y = if y.abs() < 1e-12 && pre == 0.0 {
                0.0
            } else {
                y
            };
            self.pre_hp = (a, pre, y);
            pre = y;
        }
        let bus = gain[0] * w0 + gain[1] * w1 + gain[2] * w2 + noise_gain * noise + ext_bus * pre;
        // BASS COMP (mpc-vst-ca72's mod, not the instrument's): the input raised with the loop's
        // gain, as the passband falls by it (by 1 + k), so the bass holds up as EMPHASIS rises;
        // the input pair is driven harder with it, as a circuit mod that does so would drive it.
        let bus = if self.bass_comp {
            bus * (1.0 + self.bass_amount * k)
        } else {
            bus
        };
        self.probe_bus = bus;
        self.probe_pitch = self.pitch + self.beyond;
        self.probe_env = self.contours[0].volts;
        let y = if let (true, Some(c)) = (self.circuit_on, self.circuit.as_deref_mut()) {
            // The circuit's filter: its own output coupling and FILTER MODE are in it.
            let octaves = f64::from(fm_octaves) + self.cutoff_offset;
            let i0 = c.i0(self.contours[0].volts, self.pitch + self.beyond, octaves);
            let i_bus = splice::AMPS * f64::from(bus + c.dc) + c.hiss_rms * c.hiss.next_uniform();
            let y = c.vcf.tick(i_bus, c.g_bus, i0);
            (splice::OUT * y) as f32
        } else {
            let y = self.ladder.tick(
                laws::DRIVE * self.drive * bus,
                g.clamp(0.0, 0.999),
                k,
                laws::BIAS,
                self.limit,
            );
            // The filter's output coupling; FILTER MODE HI, the bus (through its own coupling)
            // less it.
            let y = y * self.pass;
            self.probe_y = y;
            let (a, x0, y0) = self.coupling;
            let lo = a * (y0 + y - x0);
            self.coupling = (a, y, lo);
            if self.panel.filter_hi {
                let (a, x0, y0) = self.mode;
                let d = self.direct * bus;
                let direct = a * (y0 + d - x0);
                self.mode = (a, d, direct);
                direct - lo
            } else {
                // (The direct branch's coupling at rest: in HI it starts from the bus's sample.)
                self.mode.1 = 0.0;
                self.mode.2 = 0.0;
                lo
            }
        };
        // The VCA on the loudness contour.
        let g = &laws::VCA_GAIN;
        let v = (loud.max(0.0) * (1.0 / laws::VCA_STEP)) as f32;
        let i = (v as usize).min(g.len() - 2);
        let f = (v - i as f32).min(1.0);
        let gain = g[i] + (g[i + 1] - g[i]) * f;
        // The output's coupling (the rest of the waveforms' droop).
        let y = y * laws::OUT * gain;
        let (a, x0, y0) = self.droop;
        let coupled = a * (y0 + y - x0);
        self.droop = (a, y, coupled);
        // The VCA's thump: its gain through the band-pass.
        let ((ha, lb), (g0, h0, l0)) = (self.thump_c, self.thump);
        let h = ha * (h0 + gain - g0);
        let l = l0 + lb * (h - l0);
        self.thump = (gain, h, l);
        let mut out = f64::from(coupled + laws::THUMP * l);
        if self.panel.a440 {
            let (phase, step) = self.a440;
            let p = phase + step;
            let p = if p >= 1.0 { p - 1.0 } else { p };
            self.a440.0 = p;
            let at = p * 64.0;
            let j = (at as usize).min(63);
            let f = at - j as f32;
            let (a, b) = (A440_SHAPE[j], A440_SHAPE[(j + 1) % 64]);
            out += f64::from((a + (b - a) * f) * laws::A440_LEVEL);
        }
        self.fed_back[self.fed_at] = out as f32;
        self.fed_at = (self.fed_at + 1) & 127;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fast functions follow the true ones closely enough for the model.
    #[test]
    fn the_fast_functions_follow() {
        for i in -2000..2000 {
            let x = i as f32 * 0.01;
            let t = tanh(x);
            assert!((t - x.tanh()).abs() < 0.025, "tanh({x}) = {t}");
            assert_eq!(tanh(-x), -t);
            let e = exp2(x);
            assert!((e / x.exp2() - 1.0).abs() < 1e-6, "exp2({x}) = {e}");
        }
    }

    /// A key sounds at its pitch, and silence follows its release.
    #[test]
    fn a_note_sounds_and_ends() {
        let mut l = Light::new(48_000.0);
        l.set_panel(&Panel {
            cutoff: 1.0,
            decay: false,
            ..Panel::default()
        });
        for _ in 0..4800 {
            assert!(l.tick(0.0).abs() < 1e-9);
        }
        l.note(57, true);
        // (The contours start 7.8 ms after the key, as the circuit's do.)
        for _ in 0..4800 {
            l.tick(0.0);
        }
        let mut peak = 0.0f64;
        let mut crossings = 0;
        let mut last = 0.0;
        for _ in 0..48_000 {
            let y = l.tick(0.0);
            assert!(y.is_finite());
            peak = peak.max(y.abs());
            if last <= 0.0 && y > 0.0 {
                crossings += 1;
            }
            last = y;
        }
        assert!(peak > 0.05, "silent: {peak}");
        assert!(
            (219..=221).contains(&crossings),
            "{crossings} cycles in a second at 220 Hz"
        );
        l.note(57, false);
        // (Two seconds: the VCA's thump has gone too.)
        for _ in 0..96_000 {
            l.tick(0.0);
        }
        assert!(l.tick(0.0).abs() < 1e-6);
        assert!(l.slow_state().is_some());
    }

    /// The panel the laws are checked on: oscillator 1's sawtooth alone at VOLUME 3, the
    /// filter open, the loudness contour held at its top (decisions.md R31's measurement
    /// panel).
    fn measuring() -> Panel {
        let mut p = Panel {
            cutoff: 1.0,
            contour_amount: 0.0,
            keyboard_control_1: false,
            ..Panel::default()
        };
        p.osc[0].volume = 0.3;
        p.loudness_contour = ContourKnobs {
            attack: 0.0,
            decay: 0.5,
            sustain: 1.0,
        };
        p
    }

    /// `n` samples' output.
    fn run(l: &mut Light, n: usize) -> Vec<f64> {
        (0..n).map(|_| l.tick(0.0)).collect()
    }

    /// The frequency of `x` from its rising zero crossings, Hz at 48 kHz.
    fn hz(x: &[f64]) -> f64 {
        let ups: Vec<f64> = (1..x.len())
            .filter(|&i| x[i - 1] <= 0.0 && x[i] > 0.0)
            .map(|i| i as f64 - x[i] / (x[i] - x[i - 1]))
            .collect();
        (ups.len() - 1) as f64 * 48_000.0 / (ups[ups.len() - 1] - ups[0])
    }

    /// Key 69 on 8' is 440 Hz, a RANGE an octave, oscillator 2's FREQUENCY pot at its
    /// counter-clockwise end the circuit's -856.4 cents (`freq_dial`).
    #[test]
    fn the_pitch_laws() {
        let mut l = Light::new(48_000.0);
        l.set_panel(&measuring());
        l.note(69, true);
        run(&mut l, 4800);
        assert!((hz(&run(&mut l, 48_000)) / 440.0 - 1.0).abs() < 2e-4);
        let mut p = measuring();
        p.osc[0].range = crate::tuning::Range::R4;
        l.set_panel(&p);
        run(&mut l, 4800);
        assert!((hz(&run(&mut l, 48_000)) / 880.0 - 1.0).abs() < 2e-4);
        p.osc[0] = OscPanel {
            on: false,
            ..p.osc[0]
        };
        p.osc[1] = OscPanel {
            on: true,
            volume: 0.3,
            freq: 0.0,
            ..p.osc[1]
        };
        l.set_panel(&p);
        run(&mut l, 4800);
        let cents = 1200.0 * (hz(&run(&mut l, 48_000)) / 440.0).log2();
        assert!((cents + 856.4).abs() < 1.5, "{cents} cents");
    }

    /// The filter at EMPHASIS 10 rings at the circuit's pitch and level once something
    /// reaches it (CUTOFF 0.5: 1056 Hz, 0.170 RMS), not at all at CUTOFF 0.1; at CUTOFF 0.5
    /// it rings from EMPHASIS
    /// 7.35, not at 7.15 (the circuit's threshold 7.25); in FILTER MODE HI as in LO.
    #[test]
    fn the_filter_rings_where_the_circuits_does() {
        let ring = |cutoff: f64, emphasis: f64| {
            let mut l = Light::new(48_000.0);
            let mut p = measuring();
            p.cutoff = cutoff;
            p.emphasis = emphasis;
            p.filter_hi = emphasis == 0.99;
            l.set_panel(&p);
            l.note(60, true);
            run(&mut l, 960);
            // (A kick from oscillator 1, then nothing reaching the filter.)
            p.osc[0].on = false;
            l.set_panel(&p);
            run(&mut l, 48_000);
            run(&mut l, 24_000)
        };
        let x = ring(0.5, 1.0);
        let rms = (x.iter().map(|y| y * y).sum::<f64>() / x.len() as f64).sqrt();
        // (The circuit's 0.170: its ring louder than one pair's limit would have it.)
        assert!((rms / 0.170 - 1.0).abs() < 0.04, "rms {rms}");
        assert!((hz(&x) / 1056.0 - 1.0).abs() < 0.01, "{} Hz", hz(&x));
        let hi = ring(0.5, 0.99);
        assert!((hz(&hi) / 1056.0 - 1.0).abs() < 0.01, "HI: {} Hz", hz(&hi));
        let quiet = |x: &[f64]| x.iter().all(|y| y.abs() < 1e-4);
        assert!(quiet(&ring(0.1, 1.0)), "rang at CUTOFF 0.1");
        assert!(!quiet(&ring(0.5, 0.735)), "silent at EMPHASIS 7.35");
        assert!(quiet(&ring(0.5, 0.715)), "rang at EMPHASIS 7.15");
    }

    /// EXTERNAL INPUT coupled at its jack and after its preamplifier as the circuit's: 5 Hz
    /// through the open filter 21 to 24.5 dB under 100 Hz (the circuit's 22.8; the output's
    /// droop alone 11.4, with the jack's coupling alone 18).
    #[test]
    fn the_external_input_is_coupled() {
        let level = |hz: f64| {
            let mut l = Light::new(48_000.0);
            let mut p = measuring();
            p.osc[0].on = false;
            p.ext_on = true;
            p.ext_volume = 0.5;
            l.set_panel(&p);
            l.note(60, true);
            let w = std::f64::consts::TAU * hz / 48_000.0;
            let mut peak = 0.0f64;
            for i in 0..96_000 {
                let y = l.tick(0.001 * (w * f64::from(i)).sin());
                if i >= 48_000 {
                    peak = peak.max(y.abs());
                }
            }
            20.0 * peak.log10()
        };
        let d = level(5.0) - level(100.0);
        assert!(
            (-24.5..-21.0).contains(&d),
            "5 Hz {d:+.1} dB against 100 Hz"
        );
    }

    /// The noise as loud at 44.1 and 96 kHz as at 48 (within 1 dB, white and pink at
    /// VOLUME 5 through the open filter; the circuit's white 0.7 dB louder at 96 kHz, its
    /// open filter reaching higher): its density a hertz, not a sample, as the circuit's.
    #[test]
    fn the_noise_is_as_loud_at_every_rate() {
        let rms = |rate: f64, pink: bool| {
            let mut l = Light::new(rate);
            let mut p = measuring();
            p.osc[0].on = false;
            p.noise_on = true;
            p.noise_pink = pink;
            p.noise_volume = 0.5;
            l.set_panel(&p);
            l.note(60, true);
            let n = (rate * 0.5) as usize;
            run(&mut l, n);
            let x = run(&mut l, 4 * n);
            10.0 * (x.iter().map(|y| y * y).sum::<f64>() / x.len() as f64).log10()
        };
        for pink in [false, true] {
            let at48 = rms(48_000.0, pink);
            for rate in [44_100.0, 96_000.0] {
                let d = rms(rate, pink) - at48;
                assert!(
                    d.abs() < 1.0,
                    "{} at {rate} Hz: {d:+.2} dB",
                    if pink { "pink" } else { "white" }
                );
            }
        }
    }

    /// Beyond the keyboard's keys (F to C) the rest of a glide is GLIDE's RC alone, as the
    /// circuit's voice has it: at GLIDE 6, from key 36 held a second to 43, still 2.14
    /// semitones short after 0.4 s (the circuit's 2.14); and the slide holds once the key is
    /// let go (the keyboard's track-and-hold).
    #[test]
    fn glide_beyond_the_keys_and_held_once_let_go() {
        let mut l = Light::new(48_000.0);
        let mut p = measuring();
        p.glide = 0.6;
        p.glide_on = true;
        l.set_panel(&p);
        l.note(36, true);
        run(&mut l, 48_000);
        l.note(43, true);
        l.note(36, false);
        run(&mut l, 19_200);
        let short = 43.0 - (l.pitch + l.beyond);
        assert!((short - 2.14).abs() < 0.1, "{short} semitones short");
        // Let go mid-slide in the keyboard's own range: the pitch holds there.
        l.note(72, true);
        l.note(43, false);
        run(&mut l, 1440);
        l.note(72, false);
        run(&mut l, 32);
        let at = l.pitch + l.beyond;
        assert!(at < 70.0, "the slide over by then: {at}");
        run(&mut l, 24_000);
        assert!((l.pitch + l.beyond - at).abs() < 1e-9, "slid on from {at}");
    }

    /// The contours start 7.8 ms after the key; SUSTAIN at 10 holds the loudness contour at
    /// 4.45 V; with the DECAY switch off the release falls with a time constant near 20 ms
    /// after its 14 ms, towards -0.70 V (DECAY 2); GLIDE at 4 slides the 11 semitones from
    /// 48 to 59 in about 27 ms from the lower key's release (the circuit's 27.3 ms: its pitch
    /// contact 2 ms behind, then 456 semitones a second).
    #[test]
    fn the_contour_and_glide_laws() {
        let mut l = Light::new(48_000.0);
        let mut p = measuring();
        p.decay = false;
        p.loudness_contour.decay = 0.2;
        l.set_panel(&p);
        let rest = l.contours().1;
        l.note(60, true);
        let mut started = None;
        for i in 0..4800 {
            l.tick(0.0);
            if started.is_none() && l.contours().1 > rest + 0.01 {
                started = Some(i as f64 / 48.0);
            }
        }
        let ms = started.expect("no attack");
        assert!((ms - 7.8).abs() < 0.2, "the attack at {ms} ms");
        run(&mut l, 48_000);
        assert!((l.contours().1 - 4.4549).abs() < 0.01, "{}", l.contours().1);
        l.note(60, false);
        // 14 ms of the trigger, then a time constant (20 ms at DECAY 2): from 4.45 V towards
        // -0.70 V.
        run(&mut l, 677 + 1440);
        let fallen = (l.contours().1 + 0.6988) / (4.4549 + 0.6988);
        let tau = 0.030 / -fallen.ln();
        assert!((tau - 0.0200).abs() < 0.002, "tau {tau}");
        // GLIDE.
        p.glide = 0.4;
        p.glide_on = true;
        l.set_panel(&p);
        l.note(48, true);
        run(&mut l, 48_000);
        l.note(60, true);
        l.note(48, false);
        let mut at = None;
        for i in 0..9600 {
            l.tick(0.0);
            if at.is_none() && l.pitch > 59.0 {
                at = Some(i as f64 / 48.0);
            }
        }
        let ms = at.expect("no glide");
        assert!((ms - 27.3).abs() < 2.0, "11 semitones in {ms} ms");
    }

    /// The contours' times through the reference's laws (voice.rs's `time_pot`), as the
    /// circuit's: ATTACK 6 takes the loudness contour to its plateau in 0.665 s (the circuit's
    /// 0.6645), DECAY 6 the filter contour half way down from it in 0.36 to 0.38 s (0.363)
    /// and nine tenths in 1.27 to 1.33 s (1.304; one exponential through both, 1.25),
    /// SUSTAIN 5 holds the loudness contour at 1.675 V; and a fast attack stops short of the
    /// plateau, as the circuit's does (ATTACK 0: 4.685 V, against 4.792).
    #[test]
    fn the_contours_follow_the_references_laws() {
        let mut p = measuring();
        p.loudness_contour = ContourKnobs {
            attack: 0.6,
            decay: 0.5,
            sustain: 1.0,
        };
        p.filter_contour = ContourKnobs {
            attack: 0.0,
            decay: 0.6,
            sustain: 0.0,
        };
        let mut l = Light::new(48_000.0);
        l.set_panel(&p);
        l.note(60, true);
        let (mut peak, mut top, mut half, mut at) = (0.0f64, 0.0f64, None, 0.0);
        let mut ninety = None;
        for i in 0..96_000 {
            l.tick(0.0);
            let (f, v) = l.contours();
            if f > peak {
                peak = f;
            }
            if half.is_none() && peak > 4.0 && f < 0.5 * (peak + 0.0431) {
                half = Some(i as f64 / 48_000.0);
            }
            if ninety.is_none() && peak > 4.0 && f < 0.1 * peak + 0.9 * 0.0431 {
                ninety = Some(i as f64 / 48_000.0);
            }
            if v > top + 1e-9 {
                top = v;
                at = i as f64 / 48_000.0;
            }
        }
        assert!((peak - 4.685).abs() < 0.01, "ATTACK 0's peak {peak} V");
        assert!(
            (at - 0.6645).abs() < 0.01,
            "ATTACK 6 to its plateau in {at} s"
        );
        let half = half.expect("no decay") - 0.0086;
        assert!((0.36..0.38).contains(&half), "DECAY 6 half way in {half} s");
        let ninety = ninety.expect("no decay") - 0.0086;
        assert!(
            (1.27..1.33).contains(&ninety),
            "DECAY 6 nine tenths in {ninety} s"
        );
        p.loudness_contour.sustain = 0.5;
        l.set_panel(&p);
        run(&mut l, 240_000);
        let held = l.contours().1;
        assert!((held - 1.675).abs() < 0.01, "SUSTAIN 5 at {held} V");
    }

    /// The shark tooth's odd harmonics as the circuit's: the fifth 27.6 dB under the
    /// fundamental (its triangle ahead of the sawtooth's reset; in step, 24.7).
    #[test]
    fn the_shark_tooth_as_the_circuits() {
        let mut l = Light::new(48_000.0);
        let mut p = measuring();
        p.osc[0].waveform = Waveform::SharkTooth;
        p.osc[0].volume = 0.2;
        l.set_panel(&p);
        l.note(33, true);
        run(&mut l, 24_000);
        let x = run(&mut l, 48_000);
        let f0 = hz(&x);
        let amp = |h: f64| {
            let w = std::f64::consts::TAU * h * f0 / 48_000.0;
            let (mut re, mut im) = (0.0, 0.0);
            for (i, y) in x.iter().enumerate() {
                let window = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / x.len() as f64).cos();
                re += window * y * (w * i as f64).cos();
                im += window * y * (w * i as f64).sin();
            }
            re.hypot(im)
        };
        let h5 = 20.0 * (amp(5.0) / amp(1.0)).log10();
        assert!((h5 + 27.6).abs() < 1.0, "the fifth harmonic at {h5:.1} dB");
    }

    /// The passband falls as the circuit's at the lowest cutoffs: a sawtooth at 55 Hz
    /// (VOLUME 3) through CUTOFF 0.1 15.9 dB under CUTOFF 1's (the circuit's 17.3; with the
    /// ladder's own passband alone, 13).
    #[test]
    fn the_passband_falls_at_the_lowest_cutoffs() {
        let level = |cutoff: f64| {
            let mut l = Light::new(48_000.0);
            let mut p = measuring();
            p.cutoff = cutoff;
            l.set_panel(&p);
            l.note(33, true);
            run(&mut l, 24_000);
            let x = run(&mut l, 48_000);
            10.0 * (x.iter().map(|y| y * y).sum::<f64>() / x.len() as f64).log10()
        };
        let d = level(0.1) - level(1.0);
        assert!((d + 15.9).abs() < 1.0, "CUTOFF 0.1 {d:+.1} dB against 1");
    }

    /// DRIVE as the circuit's: oscillator 1's sawtooth at VOLUME 5, the filter open, 6, 12
    /// and 24 dB of it raise the level 5.7, 10.75 and 15.0 dB (the circuit's 5.7, 10.5 and
    /// 15.0: the pair saturates, and its limit falls with DRIVE as the circuit's ladder holds
    /// a driven input).
    #[test]
    fn drive_raises_the_level_less_and_less() {
        let level = |db: f64| {
            let mut l = Light::new(48_000.0);
            let mut p = measuring();
            p.osc[0].volume = 0.5;
            l.set_panel(&p);
            l.set_drive(10f64.powf(db / 20.0));
            l.note(45, true);
            run(&mut l, 24_000);
            let x = run(&mut l, 48_000);
            10.0 * (x.iter().map(|y| y * y).sum::<f64>() / x.len() as f64).log10()
        };
        let off = level(0.0);
        for (db, want) in [(6.0, 5.75), (12.0, 10.75), (24.0, 15.0)] {
            let up = level(db) - off;
            assert!((up - want).abs() < 0.3, "DRIVE {db} dB: {up:+.2} dB");
        }
    }

    /// FILTER MODE HI as the circuit's: the bus less the filter's output, so that a sawtooth
    /// at 110 Hz (VOLUME 3) under CUTOFF 0.7 (5.3 kHz) is mostly gone, 10.6 dB under LO's
    /// (the circuit's 10.8), and over CUTOFF 0.3 (214 Hz) 6.3 dB louder than LO's (6.5); at
    /// EMPHASIS 5 the bass stands 3 dB under the bus (a shelf: 5.7 dB over LO's, the
    /// circuit's 5.7).
    #[test]
    fn filter_mode_hi_is_the_bus_less_the_filter() {
        let level = |cutoff: f64, emphasis: f64, hi: bool| {
            let mut l = Light::new(48_000.0);
            let mut p = measuring();
            p.cutoff = cutoff;
            p.emphasis = emphasis;
            p.filter_hi = hi;
            l.set_panel(&p);
            l.note(45, true);
            run(&mut l, 24_000);
            let x = run(&mut l, 48_000);
            10.0 * (x.iter().map(|y| y * y).sum::<f64>() / x.len() as f64).log10()
        };
        for (cutoff, emphasis, want) in [(0.7, 0.0, -10.8), (0.3, 0.0, 6.5), (0.5, 0.5, 5.7)] {
            let d = level(cutoff, emphasis, true) - level(cutoff, emphasis, false);
            assert!(
                (d - want).abs() < 0.8,
                "CUTOFF {cutoff}, EMPHASIS {emphasis}: HI {d:+.2} dB against LO"
            );
        }
    }

    /// NOTE PRIORITY (decisions.md R50) as the circuit's voice has it: LOW the lowest
    /// key held, HIGH the highest, LAST the key pressed last, then the last still held; a key
    /// let go keeps sounding for the release lead (2 ms) unless a newer key takes over; beyond
    /// the keyboard's keys, the chosen key continues the scale.
    #[test]
    fn the_priority_chooses_the_key_held_that_sounds() {
        let plays = |priority: Priority, steps: &[(i32, i32)]| {
            let mut l = Light::new(48_000.0);
            let mut p = measuring();
            p.priority = priority;
            l.set_panel(&p);
            for &(key, sounding) in steps {
                l.note(key.abs(), key > 0);
                run(&mut l, 480);
                let at = l.target + l.foot;
                assert!(
                    (at - f64::from(sounding)).abs() < 1e-9,
                    "{priority:?}: after {key:+}, key {at} for {sounding}"
                );
            }
        };
        plays(
            Priority::Low,
            &[(57, 57), (64, 57), (50, 50), (-50, 57), (-57, 64)],
        );
        plays(
            Priority::High,
            &[
                (57, 57),
                (64, 64),
                (50, 64),
                (-64, 57),
                (-57, 50),
                (100, 100),
            ],
        );
        plays(
            Priority::Last,
            &[
                (57, 57),
                (64, 64),
                (50, 50),
                (-50, 64),
                (60, 60),
                (-64, 60),
                (-60, 57),
                (30, 30),
            ],
        );
        // The release lead: LAST's key let go sounds 2 ms on, then the last still held.
        let mut l = Light::new(48_000.0);
        let mut p = measuring();
        p.priority = Priority::Last;
        l.set_panel(&p);
        l.note(57, true);
        l.note(64, true);
        run(&mut l, 480);
        l.note(64, false);
        run(&mut l, 48);
        assert_eq!(l.target, 64.0, "within the lead");
        run(&mut l, 96);
        assert_eq!(l.target, 57.0, "after it");
    }

    /// RETRIGGER as the circuit's voice has it: a key pressed while another is held lowers the
    /// keys' gate for the gap (13 ms) and then comes down, the contours starting again (an
    /// attack) and the pitch moving to it only then; off, single triggering.
    #[test]
    fn retrigger_starts_the_contours_again_with_keys_held() {
        let run = |retrigger: bool| {
            let mut l = Light::new(48_000.0);
            let mut p = measuring();
            p.priority = Priority::Last;
            p.retrigger = retrigger;
            p.loudness_contour.decay = 0.3;
            p.loudness_contour.sustain = 0.2;
            l.set_panel(&p);
            l.note(57, true);
            run(&mut l, 24_000);
            let before = l.contours().1;
            l.note(64, true);
            run(&mut l, 384);
            let early = l.target;
            let most = (0..3840)
                .map(|_| {
                    l.tick(0.0);
                    l.contours().1
                })
                .fold(0.0, f64::max);
            (before, most, early, l.target)
        };
        let (before, most, early, late) = run(false);
        assert!((most - before).abs() < 0.05, "{before} V, then {most} V");
        assert_eq!((early, late), (64.0, 64.0));
        let (before, most, early, late) = run(true);
        assert!(most > before + 2.0, "{before} V, then {most} V");
        assert_eq!((early, late), (57.0, 64.0));
    }
}
