//! The synthesiser: a song's events played through a [`Bank`] (sampled voices with the
//! zones' envelopes, filters, loops and pans), or through a built-in General MIDI
//! approximation when there is no bank: oscillators by instrument family, plucked strings
//! (Karplus–Strong) and noise-and-sweep drums. Stereo out at any rate.
//!
//! Channel controls followed: program and bank select, volume (7), pan (10), expression
//! (11), sustain pedal (64), all sound / notes off, reset all controllers, pitch bend with
//! its range (RPN 0), and GM / GS / XG resets. Channel 10 plays drums (bank 128).

use std::sync::Arc;

use crate::smf::{Event, Kind, Song};
use crate::{Bank, LoopMode, Sample, Zone};

/// Voices sounding at once (the quietest released one is stolen beyond).
const POLYPHONY: usize = 96;
/// Overall gain, leaving headroom for chords.
const MASTER: f32 = 0.35;
/// Seconds of sound after the last event (releases ring out).
pub const TAIL_SECONDS: f64 = 2.0;

#[derive(Clone, Copy, Debug)]
struct Channel {
    program: u8,
    bank_msb: u8,
    volume: f32,
    expression: f32,
    pan: f32,
    sustain: bool,
    /// Pitch bend in semitones.
    bend: f32,
    bend_range: f32,
    rpn: (u8, u8),
}

impl Channel {
    fn new() -> Channel {
        Channel {
            program: 0,
            bank_msb: 0,
            volume: 100.0 / 127.0,
            expression: 1.0,
            pan: 0.0,
            sustain: false,
            bend: 0.0,
            bend_range: 2.0,
            rpn: (127, 127),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Delay,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
    Done,
}

/// A volume envelope's state: the level is linear in the attack and in decibels after.
#[derive(Clone, Copy, Debug)]
struct Env {
    e: crate::Envelope,
    stage: Stage,
    t: f32,
    /// Attenuation in dB (after the attack).
    db: f32,
    /// The linear level when released during the attack.
    gain: f32,
}

impl Env {
    fn new(e: crate::Envelope) -> Env {
        Env {
            e,
            stage: Stage::Delay,
            t: 0.0,
            db: 0.0,
            gain: 0.0,
        }
    }

    /// The gain for the next `dt` seconds.
    fn step(&mut self, dt: f32) -> f32 {
        loop {
            match self.stage {
                Stage::Delay => {
                    if self.t < self.e.delay {
                        self.t += dt;
                        return 0.0;
                    }
                    self.stage = Stage::Attack;
                    self.t = 0.0;
                }
                Stage::Attack => {
                    if self.t < self.e.attack {
                        self.t += dt;
                        self.gain = (self.t / self.e.attack).min(1.0);
                        return self.gain;
                    }
                    self.stage = Stage::Hold;
                    self.t = 0.0;
                    self.gain = 1.0;
                }
                Stage::Hold => {
                    if self.t < self.e.hold {
                        self.t += dt;
                        return 1.0;
                    }
                    self.stage = Stage::Decay;
                    self.t = 0.0;
                    self.db = 0.0;
                }
                Stage::Decay => {
                    // 100 dB over the decay time, down to the sustain level.
                    self.db += dt * 100.0 / self.e.decay.max(0.001);
                    if self.db >= self.e.sustain_db {
                        self.db = self.e.sustain_db;
                        self.stage = Stage::Sustain;
                    }
                    if self.db > 100.0 {
                        self.stage = Stage::Done;
                        return 0.0;
                    }
                    return db_gain(self.db);
                }
                Stage::Sustain => {
                    if self.db > 100.0 {
                        self.stage = Stage::Done;
                        return 0.0;
                    }
                    return db_gain(self.db);
                }
                Stage::Release => {
                    self.db += dt * 100.0 / self.e.release.max(0.001);
                    if self.db > 100.0 {
                        self.stage = Stage::Done;
                        return 0.0;
                    }
                    return db_gain(self.db) * self.gain;
                }
                Stage::Done => return 0.0,
            }
        }
    }

    fn release(&mut self) {
        if matches!(self.stage, Stage::Release | Stage::Done) {
            return;
        }
        // Released before the decay: continue from the linear level reached.
        if matches!(self.stage, Stage::Delay | Stage::Attack) {
            if self.gain <= 0.0 {
                self.stage = Stage::Done;
                return;
            }
        } else {
            self.gain = 1.0;
        }
        self.stage = Stage::Release;
    }
}

fn db_gain(db: f32) -> f32 {
    10f32.powf(-db / 20.0)
}

/// A two-pole low-pass filter (the RBJ cookbook's), fixed for the note.
#[derive(Clone, Copy, Debug, Default)]
struct Lowpass {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: [f32; 1],
    x2: [f32; 1],
    y1: [f32; 1],
    y2: [f32; 1],
}

impl Lowpass {
    fn new(fc: f32, q_db: f32, rate: f32) -> Option<Lowpass> {
        let fc = fc.min(rate * 0.45);
        if fc <= 10.0 {
            return None;
        }
        let q = 10f32.powf(q_db / 20.0).max(0.707);
        let w = 2.0 * std::f32::consts::PI * fc / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q);
        let a0 = 1.0 + alpha;
        let b1 = (1.0 - c) / a0;
        Some(Lowpass {
            b0: b1 / 2.0,
            b1,
            b2: b1 / 2.0,
            a1: -2.0 * c / a0,
            a2: (1.0 - alpha) / a0,
            ..Lowpass::default()
        })
    }

    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1[0] + self.b2 * self.x2[0]
            - self.a1 * self.y1[0]
            - self.a2 * self.y2[0];
        self.x2[0] = self.x1[0];
        self.x1[0] = x;
        self.y2[0] = self.y1[0];
        self.y1[0] = if y.abs() < 1e-20 { 0.0 } else { y };
        y
    }
}

/// Built-in voices' waves.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Wave {
    Sine,
    /// Sine with harmonics (piano, organ, bell): the second and third's levels.
    Harmonics(f32, f32),
    Saw,
    Square,
    Triangle,
    Noise,
}

#[derive(Clone, Debug)]
enum Source {
    Sampled {
        sample: usize,
        pos: f64,
        /// Frames of the sample per output frame, before the bend.
        step: f64,
        end: f64,
        loop_start: f64,
        loop_end: f64,
        mode: LoopMode,
    },
    Osc {
        wave: Wave,
        phase: f32,
        /// Cycles per output frame.
        step: f32,
        /// A second, detuned oscillator's phase (ensembles and pads).
        detune: Option<f32>,
        noise: u32,
    },
    Pluck {
        buf: Vec<f32>,
        at: usize,
        damping: f32,
    },
    Drum {
        /// Tone: start and end frequency (cycles per frame), sweep time constant.
        f0: f32,
        f1: f32,
        sweep: f32,
        tone: f32,
        noise_level: f32,
        phase: f32,
        t: f32,
        noise: u32,
        /// A high-passed noise (cymbals): the last noise value.
        bright: bool,
        last: f32,
        /// Seconds per frame.
        dt: f32,
    },
}

#[derive(Clone, Debug)]
struct Voice {
    channel: u8,
    key: u8,
    src: Source,
    env: Env,
    gain: f32,
    pan: f32,
    filter: Option<Lowpass>,
    /// Released by its note off (but held while the pedal is down).
    held: bool,
    released: bool,
    exclusive: u16,
    born: u64,
}

fn noise(state: &mut u32) -> f32 {
    // xorshift32
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// PolyBLEP: smooths a saw or square's jump at phase 0 (`t` the phase, `dt` the step).
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        2.0 * t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

fn osc(wave: Wave, ph: f32, dt: f32, noise_state: &mut u32) -> f32 {
    use std::f32::consts::TAU;
    match wave {
        Wave::Sine => (ph * TAU).sin(),
        Wave::Harmonics(h2, h3) => {
            ((ph * TAU).sin() + h2 * (2.0 * ph * TAU).sin() + h3 * (3.0 * ph * TAU).sin())
                / (1.0 + h2 + h3)
        }
        Wave::Saw => 2.0 * ph - 1.0 - blep(ph, dt),
        Wave::Square => {
            let s = if ph < 0.5 { 1.0 } else { -1.0 };
            s + blep(ph, dt) - blep((ph + 0.5) % 1.0, dt)
        }
        Wave::Triangle => 1.0 - 4.0 * (ph - 0.5).abs(),
        Wave::Noise => noise(noise_state),
    }
}

/// The frequency of a key in Hz.
fn key_hz(key: f32) -> f32 {
    440.0 * 2f32.powf((key - 69.0) / 12.0)
}

/// Plays a song.
#[derive(Clone)]
pub struct Synth {
    song: Arc<Song>,
    bank: Option<Arc<Bank>>,
    rate: u32,
    channels: [Channel; 16],
    voices: Vec<Voice>,
    next: usize,
    /// Frames rendered.
    frame: u64,
    seed: u32,
}

impl Synth {
    pub fn new(song: Arc<Song>, bank: Option<Arc<Bank>>, rate: u32) -> Synth {
        Synth {
            song,
            bank,
            rate: rate.max(8000),
            channels: [Channel::new(); 16],
            voices: Vec::with_capacity(POLYPHONY),
            next: 0,
            frame: 0,
            seed: 0x1234_5678,
        }
    }

    /// Frames of the whole song with its tail.
    pub fn length(song: &Song, rate: u32) -> u64 {
        ((song.length_us as f64 / 1e6 + TAIL_SECONDS) * f64::from(rate)).ceil() as u64
    }

    /// Renders `n` stereo frames into `out` (interleaved).
    pub fn render(&mut self, n: usize, out: &mut Vec<f32>) {
        let start = out.len();
        out.resize(start + 2 * n, 0.0);
        let mut done = 0;
        while done < n {
            // Up to the next event.
            let next_frame = self
                .song
                .events
                .get(self.next)
                .map(|e| self.frame_of(e))
                .unwrap_or(u64::MAX);
            if next_frame <= self.frame {
                let e = self.song.events[self.next];
                self.next += 1;
                self.event(&e);
                continue;
            }
            let run = ((next_frame - self.frame).min((n - done) as u64) as usize).min(64);
            let o = &mut out[start + 2 * done..start + 2 * (done + run)];
            self.mix(o);
            self.frame += run as u64;
            done += run;
        }
    }

    /// Plays `n` frames without keeping them (a seek).
    pub fn skip(&mut self, n: u64) {
        let mut scratch = Vec::with_capacity(8192);
        let mut left = n;
        while left > 0 {
            let k = left.min(4096) as usize;
            scratch.clear();
            if self.voices.is_empty() {
                // Nothing sounds: jump to the next event.
                let next = self
                    .song
                    .events
                    .get(self.next)
                    .map(|e| self.frame_of(e))
                    .unwrap_or(u64::MAX);
                let jump = next.saturating_sub(self.frame).min(left);
                if jump > 0 {
                    self.frame += jump;
                    left -= jump;
                    continue;
                }
            }
            self.render(k, &mut scratch);
            left -= k as u64;
        }
    }

    fn frame_of(&self, e: &Event) -> u64 {
        (e.us as u128 * u128::from(self.rate) / 1_000_000) as u64
    }

    fn event(&mut self, e: &Event) {
        let c = usize::from(e.channel & 15);
        match e.kind {
            Kind::NoteOn { key, vel } => self.note_on(c, key, vel),
            Kind::NoteOff { key } => {
                let sustain = self.channels[c].sustain;
                for v in self.voices.iter_mut() {
                    if usize::from(v.channel) == c && v.key == key && !v.released {
                        if sustain {
                            v.held = true;
                        } else {
                            release(v);
                        }
                    }
                }
            }
            Kind::Control { cc, value } => self.control(c, cc, value),
            Kind::Program(p) => self.channels[c].program = p,
            Kind::Bend(b) => {
                let ch = &mut self.channels[c];
                ch.bend = f32::from(b) / 8192.0 * ch.bend_range;
            }
            Kind::Reset => {
                self.channels = [Channel::new(); 16];
                self.voices.clear();
            }
        }
    }

    fn control(&mut self, c: usize, cc: u8, value: u8) {
        let v = f32::from(value) / 127.0;
        let ch = &mut self.channels[c];
        match cc {
            0 => ch.bank_msb = value,
            6 if ch.rpn == (0, 0) => ch.bend_range = f32::from(value),
            7 => ch.volume = v,
            10 => ch.pan = (f32::from(value) - 64.0) / 64.0,
            11 => ch.expression = v,
            64 => {
                ch.sustain = value >= 64;
                if !ch.sustain {
                    for vo in self.voices.iter_mut() {
                        if usize::from(vo.channel) == c && vo.held {
                            vo.held = false;
                            release(vo);
                        }
                    }
                }
            }
            100 => ch.rpn.1 = value,
            101 => ch.rpn.0 = value,
            120 => self.voices.retain(|v| usize::from(v.channel) != c),
            121 => {
                let program = ch.program;
                let bank = ch.bank_msb;
                *ch = Channel::new();
                ch.program = program;
                ch.bank_msb = bank;
            }
            123 => {
                for vo in self.voices.iter_mut() {
                    if usize::from(vo.channel) == c {
                        release(vo);
                    }
                }
            }
            _ => {}
        }
    }

    fn note_on(&mut self, c: usize, key: u8, vel: u8) {
        let ch = self.channels[c];
        let drum = c == 9;
        let rate = self.rate as f32;
        // Velocity: a square law, as the default modulator's concave curve roughly is.
        let velg = (f32::from(vel) / 127.0).powi(2);
        let mut new: Vec<Voice> = Vec::new();
        if let Some(bank) = self.bank.clone() {
            let bank_no = if drum { 128 } else { u16::from(ch.bank_msb) };
            let Some(p) = bank.find(bank_no, ch.program) else {
                return;
            };
            for z in p.zones.iter().filter(|z| {
                (z.keys.0..=z.keys.1).contains(&key) && (z.vels.0..=z.vels.1).contains(&vel)
            }) {
                let Some(s) = bank.samples.get(z.sample) else {
                    continue;
                };
                if let Some(v) = self.sampled(c, key, velg, z, s) {
                    new.push(v);
                }
            }
        } else {
            new.push(self.builtin(c, key, velg, drum, rate));
        }
        for v in new {
            if v.exclusive != 0 {
                for o in self.voices.iter_mut() {
                    if o.channel == v.channel && o.exclusive == v.exclusive {
                        o.env.stage = Stage::Done;
                    }
                }
            }
            if self.voices.len() >= POLYPHONY {
                self.steal();
            }
            self.voices.push(v);
        }
    }

    /// Drops the voice that matters least: a finished one, else the oldest released, else
    /// the oldest.
    fn steal(&mut self) {
        let pick = self
            .voices
            .iter()
            .position(|v| v.env.stage == Stage::Done)
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.released)
                    .min_by_key(|(_, v)| v.born)
                    .map(|(i, _)| i)
            })
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.born)
                    .map(|(i, _)| i)
            });
        if let Some(i) = pick {
            self.voices.swap_remove(i);
        }
    }

    fn sampled(&self, c: usize, key: u8, velg: f32, z: &Zone, s: &Sample) -> Option<Voice> {
        let rate = self.rate as f64;
        let k = f32::from(z.fixed_key.unwrap_or(key));
        let cents = (k - f32::from(z.root)) * z.scale + z.tune;
        let step = f64::from(s.rate) / rate * 2f64.powf(f64::from(cents) / 1200.0);
        let len = s.len as i64;
        let start = z.start_off.clamp(0, len);
        let end = (len + z.end_off).clamp(start, len);
        if end <= start {
            return None;
        }
        let ls = (s.loop_start as i64 + z.loop_start_off).clamp(start, end);
        let le = (s.loop_end as i64 + z.loop_end_off).clamp(ls, end);
        let mode = if le - ls < 2 {
            LoopMode::None
        } else {
            z.loop_mode
        };
        Some(Voice {
            channel: c as u8,
            key,
            src: Source::Sampled {
                sample: z.sample,
                pos: start as f64,
                step,
                end: end as f64,
                loop_start: ls as f64,
                loop_end: le as f64,
                mode,
            },
            env: Env::new(z.env),
            gain: db_gain(z.atten_db) * velg,
            pan: z.pan,
            filter: z
                .cutoff
                .and_then(|fc| Lowpass::new(fc, z.q_db, self.rate as f32)),
            held: false,
            released: false,
            exclusive: z.exclusive,
            born: self.frame,
        })
    }

    /// A voice of the built-in General MIDI approximation.
    fn builtin(&mut self, c: usize, key: u8, velg: f32, drum: bool, rate: f32) -> Voice {
        let ch = self.channels[c];
        let env = |attack: f32, decay: f32, sustain_db: f32, release: f32| crate::Envelope {
            delay: 0.0,
            attack,
            hold: 0.0,
            decay,
            sustain_db,
            release,
        };
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223)
            | 1;
        let seed = self.seed;
        let hz = key_hz(f32::from(key));
        let step = hz / rate;
        let osc_src = |wave: Wave, detune: bool| Source::Osc {
            wave,
            phase: 0.0,
            step,
            detune: detune.then_some(0.25),
            noise: seed,
        };
        let (src, e, gain, exclusive) = if drum {
            let (src, e, gain, excl) = drum_voice(key, rate, seed);
            (src, e, gain, excl)
        } else {
            let family = ch.program / 8;
            match family {
                // Pianos: harmonics fading out.
                0 => (
                    osc_src(Wave::Harmonics(0.5, 0.25), false),
                    env(0.002, 2.5, 100.0, 0.25),
                    0.9,
                    0,
                ),
                // Chromatic percussion: bells and mallets.
                1 => (
                    osc_src(Wave::Harmonics(0.0, 0.3), false),
                    env(0.001, 1.5, 100.0, 0.3),
                    0.8,
                    0,
                ),
                // Organs.
                2 => (
                    osc_src(Wave::Harmonics(0.8, 0.6), false),
                    env(0.01, 0.1, 1.0, 0.08),
                    0.6,
                    0,
                ),
                // Guitars, basses, ethnic plucks: plucked strings.
                3 | 4 | 13 => {
                    let n = ((rate / hz.max(20.0)) as usize).clamp(2, 4096);
                    let mut st = seed;
                    let buf: Vec<f32> = (0..n).map(|_| noise(&mut st)).collect();
                    let damping = if family == 4 { 0.996 } else { 0.994 };
                    (
                        Source::Pluck {
                            buf,
                            at: 0,
                            damping,
                        },
                        env(0.001, 4.0, 100.0, 0.15),
                        if family == 4 { 1.0 } else { 0.8 },
                        0,
                    )
                }
                // Strings and ensembles: slow bowed saws.
                5 | 6 => (osc_src(Wave::Saw, true), env(0.12, 1.0, 3.0, 0.35), 0.35, 0),
                // Brass.
                7 => (osc_src(Wave::Saw, false), env(0.04, 0.5, 4.0, 0.15), 0.4, 0),
                // Reeds.
                8 => (
                    osc_src(Wave::Square, false),
                    env(0.03, 0.5, 3.0, 0.1),
                    0.3,
                    0,
                ),
                // Pipes and flutes.
                9 => (
                    osc_src(Wave::Harmonics(0.15, 0.0), false),
                    env(0.05, 0.5, 2.0, 0.12),
                    0.6,
                    0,
                ),
                // Synth leads.
                10 => (
                    osc_src(Wave::Square, false),
                    env(0.005, 0.5, 4.0, 0.1),
                    0.3,
                    0,
                ),
                // Pads.
                11 => (
                    osc_src(Wave::Triangle, true),
                    env(0.3, 1.0, 2.0, 0.8),
                    0.5,
                    0,
                ),
                // Synth effects.
                12 => (
                    osc_src(Wave::Triangle, true),
                    env(0.05, 1.5, 6.0, 0.6),
                    0.45,
                    0,
                ),
                // Percussive.
                14 => (
                    osc_src(Wave::Sine, false),
                    env(0.001, 0.6, 100.0, 0.2),
                    0.8,
                    0,
                ),
                // Sound effects.
                _ => (
                    osc_src(Wave::Noise, false),
                    env(0.05, 1.0, 10.0, 0.3),
                    0.2,
                    0,
                ),
            }
        };
        Voice {
            channel: c as u8,
            key,
            src,
            env: Env::new(e),
            gain: gain * velg,
            pan: 0.0,
            filter: None,
            held: false,
            released: false,
            exclusive,
            born: self.frame,
        }
    }

    /// Mixes every voice into `out` (stereo, interleaved) and drops the finished ones.
    fn mix(&mut self, out: &mut [f32]) {
        let n = out.len() / 2;
        let dt = 1.0 / self.rate as f32;
        let bank = self.bank.clone();
        for v in self.voices.iter_mut() {
            let ch = self.channels[usize::from(v.channel & 15)];
            let level = v.gain * ch.volume * ch.volume * ch.expression * ch.expression * MASTER;
            let pan = (v.pan + ch.pan).clamp(-1.0, 1.0);
            let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
            let (gl, gr) = (angle.cos() * level, angle.sin() * level);
            let bend = 2f64.powf(f64::from(ch.bend) / 12.0);
            for i in 0..n {
                let g = v.env.step(dt);
                if v.env.stage == Stage::Done {
                    break;
                }
                let Some(x) = next_value(&mut v.src, bank.as_deref(), bend, v.released) else {
                    v.env.stage = Stage::Done;
                    break;
                };
                let x = match v.filter.as_mut() {
                    Some(f) => f.run(x),
                    None => x,
                } * g;
                out[2 * i] += x * gl;
                out[2 * i + 1] += x * gr;
            }
        }
        self.voices.retain(|v| v.env.stage != Stage::Done);
    }
}

fn release(v: &mut Voice) {
    v.released = true;
    v.held = false;
    v.env.release();
}

/// A drum of the General MIDI kit by key: source, envelope, gain, exclusive class.
fn drum_voice(key: u8, rate: f32, seed: u32) -> (Source, crate::Envelope, f32, u16) {
    let env = |decay: f32| crate::Envelope {
        delay: 0.0,
        attack: 0.0005,
        hold: 0.0,
        decay,
        sustain_db: 100.0,
        release: decay.min(0.3),
    };
    let drum =
        |f0: f32, f1: f32, sweep: f32, tone: f32, noise_level: f32, bright: bool| Source::Drum {
            f0: f0 / rate,
            f1: f1 / rate,
            sweep,
            tone,
            noise_level,
            phase: 0.0,
            t: 0.0,
            noise: seed,
            bright,
            last: 0.0,
            dt: 1.0 / rate,
        };
    match key {
        // Kicks.
        35 | 36 => (drum(160.0, 45.0, 0.04, 1.0, 0.05, false), env(0.35), 1.2, 0),
        // Snares and claps.
        38 | 40 => (drum(200.0, 160.0, 0.05, 0.4, 0.8, false), env(0.2), 0.8, 0),
        39 => (drum(1000.0, 1000.0, 1.0, 0.0, 1.0, true), env(0.15), 0.6, 0),
        37 => (drum(800.0, 600.0, 0.01, 0.5, 0.5, true), env(0.05), 0.5, 0),
        // Hi-hats (one class: an open hat is cut by a closed one).
        42 | 44 => (drum(0.0, 0.0, 1.0, 0.0, 1.0, true), env(0.06), 0.35, 1),
        46 => (drum(0.0, 0.0, 1.0, 0.0, 1.0, true), env(0.45), 0.35, 1),
        // Toms.
        41 | 43 | 45 | 47 | 48 | 50 => {
            let f = 70.0 * 2f32.powf(f32::from(key - 41) / 12.0);
            (drum(f * 1.6, f, 0.08, 1.0, 0.15, false), env(0.4), 0.9, 0)
        }
        // Cymbals.
        49 | 52 | 55 | 57 => (drum(0.0, 0.0, 1.0, 0.0, 1.0, true), env(1.5), 0.4, 0),
        51 | 53 | 59 => (drum(3000.0, 3000.0, 1.0, 0.15, 0.6, true), env(1.0), 0.3, 0),
        // Everything else: a pitched click by key.
        _ => {
            let f = key_hz(f32::from(key));
            (drum(f, f * 0.8, 0.05, 0.6, 0.4, true), env(0.12), 0.5, 0)
        }
    }
}

/// The next value of a voice's source (`None`: it has ended).
#[inline]
fn next_value(src: &mut Source, bank: Option<&Bank>, bend: f64, released: bool) -> Option<f32> {
    match src {
        Source::Sampled {
            sample,
            pos,
            step,
            end,
            loop_start,
            loop_end,
            mode,
        } => {
            let bank = bank?;
            let s = bank.samples.get(*sample)?;
            let looping = match mode {
                LoopMode::Continuous => true,
                LoopMode::UntilRelease => !released,
                LoopMode::None => false,
            };
            if looping && *pos >= *loop_end {
                *pos = *loop_start + (*pos - *loop_end) % (*loop_end - *loop_start);
            }
            if *pos >= *end - 1.0 {
                return None;
            }
            let i = *pos as usize;
            let f = (*pos - i as f64) as f32;
            let a = bank.frame(s, i);
            let next = if looping && (i + 1) as f64 >= *loop_end {
                *loop_start as usize
            } else {
                i + 1
            };
            let b = bank.frame(s, next);
            *pos += *step * bend;
            Some(a + (b - a) * f)
        }
        Source::Osc {
            wave,
            phase,
            step,
            detune,
            noise: n,
        } => {
            let dt = *step * bend as f32;
            let mut x = osc(*wave, *phase, dt, n);
            *phase = (*phase + dt) % 1.0;
            if let Some(p2) = detune {
                let dt2 = dt * 1.006;
                x = 0.5 * (x + osc(*wave, *p2, dt2, n));
                *p2 = (*p2 + dt2) % 1.0;
            }
            Some(x)
        }
        Source::Pluck { buf, at, damping } => {
            let n = buf.len();
            let x = buf[*at];
            let next = buf[(*at + 1) % n];
            buf[*at] = 0.5 * (x + next) * *damping;
            *at = (*at + 1) % n;
            Some(x)
        }
        Source::Drum {
            f0,
            f1,
            sweep,
            tone,
            noise_level,
            phase,
            t,
            noise: n,
            bright,
            last,
            dt,
        } => {
            let f = *f1 + (*f0 - *f1) * (-*t / sweep.max(1e-4)).exp();
            *t += *dt;
            let s = (*phase * std::f32::consts::TAU).sin();
            *phase = (*phase + f) % 1.0;
            let mut w = noise(n);
            if *bright {
                let hp = w - *last;
                *last = w;
                w = hp * 0.5;
            }
            Some(s * *tone + w * *noise_level)
        }
    }
}

/// A bank's samples played one after another at their own pitch, with a short gap between,
/// as a preview of what it holds.
#[derive(Clone)]
pub struct SampleList {
    bank: Arc<Bank>,
    rate: u32,
    /// Per sample: its first output frame and output length (gap included).
    spans: Vec<(u64, u64, usize)>,
    total: u64,
}

/// The gap between two samples, in seconds.
const GAP: f64 = 0.25;
/// The longest a sample is played for, in seconds (a looped pad would ring on).
const LONGEST: f64 = 8.0;

impl SampleList {
    pub fn new(bank: Arc<Bank>, rate: u32) -> SampleList {
        let mut spans = Vec::new();
        let mut t = 0u64;
        let gap = (GAP * f64::from(rate)) as u64;
        for (k, s) in bank.samples.iter().enumerate() {
            if s.len < 2 {
                continue;
            }
            let secs = (s.len as f64 / f64::from(s.rate)).min(LONGEST);
            let n = (secs * f64::from(rate)).ceil() as u64;
            spans.push((t, n, k));
            t += n + gap;
        }
        SampleList {
            bank,
            rate,
            spans,
            total: t,
        }
    }

    /// Output frames in all.
    pub fn length(&self) -> u64 {
        self.total
    }

    /// The (sample index, first frame) of each sample played.
    pub fn starts(&self) -> impl Iterator<Item = (usize, u64)> + '_ {
        self.spans.iter().map(|&(t, _, k)| (k, t))
    }

    /// Renders `n` stereo frames from output frame `from`.
    pub fn render(&self, from: u64, n: usize, out: &mut Vec<f32>) {
        let start = out.len();
        out.resize(start + 2 * n, 0.0);
        let end = from + n as u64;
        let first = self.spans.partition_point(|s| s.0 + s.1 <= from);
        for &(t, len, k) in &self.spans[first..] {
            if t >= end {
                break;
            }
            let s = &self.bank.samples[k];
            let step = f64::from(s.rate) / f64::from(self.rate);
            let a = from.max(t);
            let b = end.min(t + len);
            for f in a..b {
                let pos = (f - t) as f64 * step;
                let i = pos as usize;
                let fr = (pos - i as f64) as f32;
                let x0 = self.bank.frame(s, i);
                let x1 = self.bank.frame(s, i + 1);
                let x = (x0 + (x1 - x0) * fr) * 0.8;
                let o = start + 2 * (f - from) as usize;
                out[o] += x;
                out[o + 1] += x;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    #[test]
    fn builtin_plays_and_stops() {
        for program in [0u8, 16, 24, 40, 56, 80, 88, 120] {
            let f = crate::smf::build("t", program, &[(60, 0, 480), (67, 480, 480)]);
            let song = Arc::new(crate::smf::parse(&f).unwrap());
            let mut s = Synth::new(song.clone(), None, 44_100);
            let n = Synth::length(&song, 44_100) as usize;
            let mut out = Vec::new();
            s.render(n, &mut out);
            assert_eq!(out.len(), 2 * n);
            let first = rms(&out[..2 * 22_050]);
            assert!(first > 0.002, "program {program}: {first}");
            assert!(out.iter().all(|v| v.is_finite() && v.abs() < 4.0));
            // Silent at the end of the tail.
            let tail = rms(&out[2 * (n - 2000)..]);
            assert!(tail < 0.01, "program {program}: tail {tail}");
        }
    }

    #[test]
    fn drums_and_skip_agree() {
        let mut f = crate::smf::build("d", 0, &[(36, 0, 240), (42, 240, 240), (38, 480, 240)]);
        // Move the events to channel 10 (status 0x99/0x89).
        for b in f.iter_mut().skip(22) {
            if *b == 0x90 {
                *b = 0x99;
            } else if *b == 0x80 {
                *b = 0x89;
            }
        }
        let song = Arc::new(crate::smf::parse(&f).unwrap());
        assert!(
            song.events.iter().any(|e| e.channel == 9),
            "{:?}",
            song.events
        );
        let mut a = Synth::new(song.clone(), None, 22_050);
        let mut out = Vec::new();
        a.render(30_000, &mut out);
        let r = rms(&out[..4000]);
        assert!(r > 0.01, "{r}");
        let mut b = Synth::new(song, None, 22_050);
        b.skip(10_000);
        let mut tail = Vec::new();
        b.render(20_000, &mut tail);
        let want = &out[20_000..];
        let diff = tail
            .iter()
            .zip(want)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f32::max);
        assert!(diff < 1e-6, "{diff}");
    }

    #[test]
    fn bank_voices_follow_the_keyboard() {
        // One cycle of a 441 Hz sine at 44.1 kHz, looped: key 69 plays it at 441 Hz, key 81
        // an octave up.
        let cycle: Vec<i16> = (0..100)
            .map(|i| ((i as f32 / 100.0 * std::f32::consts::TAU).sin() * 20000.0) as i16)
            .collect();
        let mut sf = crate::sf2::build("b", &[("Sine", 0, 0)], &[("sine", 44_100, 69, &cycle)]);
        // Loop it: sampleModes 1 on the instrument zone (append a generator by hand is
        // awkward; set the loop mode after reading instead).
        let bank = crate::Bank::read(Arc::new(std::mem::take(&mut sf))).unwrap();
        let mut bank = bank;
        for z in bank.presets[0].zones.iter_mut() {
            z.loop_mode = LoopMode::Continuous;
        }
        bank.samples[0].loop_start = 0;
        bank.samples[0].loop_end = 100;
        let bank = Arc::new(bank);
        let f = crate::smf::build("s", 0, &[(81, 0, 960)]);
        let song = Arc::new(crate::smf::parse(&f).unwrap());
        let mut s = Synth::new(song, Some(bank), 44_100);
        let mut out = Vec::new();
        s.render(44_100, &mut out);
        // Count rising zero crossings over the held part: about 882 a second.
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        let part = &left[4410..44_100];
        let ups = part
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        let hz = ups as f32 / (part.len() as f32 / 44_100.0);
        assert!((hz - 882.0).abs() < 10.0, "{hz}");
    }

    #[test]
    fn sample_list_plays_each() {
        let a: Vec<i16> = vec![10000; 4410];
        let b: Vec<i16> = vec![-10000; 2205];
        let f = crate::sf2::build(
            "b",
            &[("P", 0, 0)],
            &[("a", 44_100, 60, &a), ("b", 22_050, 60, &b)],
        );
        let bank = Arc::new(crate::Bank::read(Arc::new(f)).unwrap());
        let l = SampleList::new(bank, 44_100);
        let starts: Vec<(usize, u64)> = l.starts().collect();
        assert_eq!(starts, vec![(0, 0), (1, 4410 + 11_025)]);
        let mut out = Vec::new();
        l.render(0, l.length() as usize, &mut out);
        assert!(out[2 * 100] > 0.2);
        assert!(out[2 * 6000].abs() < 1e-6);
        assert!(out[2 * (15_435 + 100)] < -0.2);
        let mut part = Vec::new();
        l.render(15_435 + 100, 1, &mut part);
        assert_eq!(part[0], out[2 * (15_435 + 100)]);
    }
}
