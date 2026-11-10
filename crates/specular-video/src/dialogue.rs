//! Dialogue boost (night mode): speech made clearer and loud passages tamed, at one
//! strength (0 to 1, gentle at the default 0.5).
//!
//! - **Compression**: a feed-forward compressor on the channels' common peak (5 ms attack,
//!   200 ms release, a 6 dB soft knee) with partial make-up gain: quiet speech comes up and
//!   explosions come down. Streams that carry their own dynamic range control (AC-3 and
//!   E-AC-3, whose decoder applies it) are not compressed again.
//! - **Speech emphasis**: a broad lift around 2.5 kHz on the middle of a stereo image (what
//!   both channels share, where dialogue is mixed), leaving the sides as they are.
//! - **Centre lift**: when 5.1 or 7.1 is folded to stereo, the centre channel (the
//!   dialogue's) is mixed louder ([`centre_gain`]).

use crate::eq::{Biquad, Section, Shape};

/// The default strength.
pub const DEFAULT_STRENGTH: f32 = 0.5;

/// How much louder the centre is mixed into a stereo fold at `strength` (linear, 1 to 2).
pub fn centre_gain(strength: f32) -> f32 {
    1.0 + strength.clamp(0.0, 1.0)
}

/// The compressor's curve at `strength`: threshold (dBFS), ratio, make-up (dB).
fn curve(strength: f32) -> (f32, f32, f32) {
    let s = strength.clamp(0.0, 1.0);
    let threshold = -18.0 - 14.0 * s;
    let ratio = 1.5 + 2.5 * s;
    let makeup = -threshold * (1.0 - 1.0 / ratio) * 0.5;
    (threshold, ratio, makeup)
}

/// The static gain (dB) for a level (dBFS) with a soft knee.
fn static_gain(level: f32, threshold: f32, ratio: f32) -> f32 {
    const KNEE: f32 = 6.0;
    let over = level - threshold;
    let slope = 1.0 / ratio - 1.0;
    if over <= -KNEE / 2.0 {
        0.0
    } else if over >= KNEE / 2.0 {
        slope * over
    } else {
        slope * (over + KNEE / 2.0).powi(2) / (2.0 * KNEE)
    }
}

/// The dialogue stage for one stream.
#[derive(Clone, Debug)]
pub struct Dialogue {
    ch: usize,
    strength: f32,
    /// Compress (off for streams with their own control).
    compress: bool,
    emphasis: Section,
    /// The detector's level (dBFS) and the per-frame coefficients.
    env: f32,
    attack: f32,
    release: f32,
}

impl Dialogue {
    pub fn new(ch: usize, rate: u32, strength: f32) -> Dialogue {
        let rate_f = rate.max(1) as f32;
        let coef = |ms: f32| (-1.0 / (ms * 0.001 * rate_f)).exp();
        let mut d = Dialogue {
            ch: ch.max(1),
            strength: -1.0,
            compress: true,
            emphasis: Section::new(Biquad::IDENTITY, 1),
            env: -120.0,
            attack: coef(5.0),
            release: coef(200.0),
        };
        d.set(rate, strength);
        d
    }

    /// A new strength (the state is kept).
    pub fn set(&mut self, rate: u32, strength: f32) {
        let s = strength.clamp(0.0, 1.0);
        if s == self.strength {
            return;
        }
        self.strength = s;
        let db = f64::from(2.0 + 4.0 * s);
        self.emphasis.set(Biquad::design(
            Shape::Peak,
            2500.0,
            0.7,
            db,
            f64::from(rate.max(1)),
        ));
    }

    /// Compress or not (a stream with its own dynamic range control: not).
    pub fn set_compress(&mut self, on: bool) {
        self.compress = on;
    }

    pub fn reset(&mut self) {
        self.emphasis.reset();
        self.env = -120.0;
    }

    /// Processes interleaved frames in place.
    pub fn run(&mut self, buf: &mut [f32]) {
        let (threshold, ratio, makeup) = curve(self.strength);
        let ch = self.ch;
        for f in buf.chunks_exact_mut(ch) {
            // Emphasis on the middle (or the only channel).
            if ch >= 2 {
                let (l, r) = (f64::from(f[0]), f64::from(f[1]));
                let (m, s) = ((l + r) * 0.5, (l - r) * 0.5);
                let m = self.emphasis.tick(0, m);
                f[0] = (m + s) as f32;
                f[1] = (m - s) as f32;
            } else {
                f[0] = self.emphasis.tick(0, f64::from(f[0])) as f32;
            }
            if !self.compress {
                continue;
            }
            let peak = f.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            let level = 20.0 * peak.max(1e-6).log10();
            let k = if level > self.env {
                self.attack
            } else {
                self.release
            };
            self.env = level + k * (self.env - level);
            let g = 10f32.powf((static_gain(self.env, threshold, ratio) + makeup) / 20.0);
            for s in f.iter_mut() {
                *s *= g;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(amp: f32, n: usize) -> Vec<f32> {
        (0..n)
            .flat_map(|i| {
                let v = amp * (std::f32::consts::TAU * 220.0 * i as f32 / 48_000.0).sin();
                [v, v]
            })
            .collect()
    }

    fn rms_db(v: &[f32]) -> f32 {
        let tail = &v[v.len() / 2..];
        10.0 * (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).log10()
    }

    #[test]
    fn quiet_comes_up_and_loud_comes_down() {
        let mut d = Dialogue::new(2, 48_000, DEFAULT_STRENGTH);
        let mut quiet = tone(0.01, 48_000);
        let q0 = rms_db(&quiet);
        d.run(&mut quiet);
        let mut d = Dialogue::new(2, 48_000, DEFAULT_STRENGTH);
        let mut loud = tone(0.9, 48_000);
        let l0 = rms_db(&loud);
        d.run(&mut loud);
        let (q1, l1) = (rms_db(&quiet), rms_db(&loud));
        // The range between them shrinks by several dB.
        assert!(q1 > q0 + 5.0, "{q0} → {q1}");
        assert!((l1 - q1) < (l0 - q0) - 8.0, "{l0} {q0} → {l1} {q1}");
    }

    #[test]
    fn curve_is_continuous() {
        let (t, r, _) = curve(0.5);
        let mut last = static_gain(-80.0, t, r);
        for i in 0..800 {
            let g = static_gain(-80.0 + i as f32 * 0.1, t, r);
            assert!(g <= last + 1e-4 && (g - last).abs() < 0.1);
            last = g;
        }
        assert_eq!(centre_gain(0.5), 1.5);
    }

    #[test]
    fn emphasis_leaves_the_sides() {
        // L = −R: all side, nothing in the middle; the emphasis does not touch it.
        let mut d = Dialogue::new(2, 48_000, 1.0);
        d.set_compress(false);
        let mut v: Vec<f32> = (0..4800)
            .flat_map(|i| {
                let x = 0.3 * (std::f32::consts::TAU * 2500.0 * i as f32 / 48_000.0).sin();
                [x, -x]
            })
            .collect();
        let before = v.clone();
        d.run(&mut v);
        assert!(v.iter().zip(&before).all(|(a, b)| (a - b).abs() < 1e-6));
    }
}
