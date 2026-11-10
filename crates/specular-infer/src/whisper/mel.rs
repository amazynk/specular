//! Whisper's input: the log-mel spectrogram of 16 kHz mono sound.
//!
//! A 400-sample Hann window (periodic) every 160 samples (10 ms), centred (the sound padded
//! by reflection at both ends), power spectrum, Slaney-style mel filters (80 or 128 bands,
//! 0–8 kHz, area-normalised), `log10` clamped to 1e-10 and to 8 below the loudest value,
//! then `(x + 4) / 4`. The sound is padded with silence (or cut) to `frames · 160` samples
//! first, as Whisper reads a 30 s window (3,000 frames).
//!
//! The transform is a direct real DFT with SIMD dot products against a table: 400 points
//! are not a power of two, and 3,000 frames take a few tens of milliseconds this way.

use std::simd::f32x8;
use std::simd::num::SimdFloat;

/// Sample rate Whisper takes.
pub const SAMPLE_RATE: usize = 16_000;
/// Window length.
pub const N_FFT: usize = 400;
/// Samples between frames (10 ms).
pub const HOP: usize = 160;
/// Frames of one 30 s window.
pub const FRAMES: usize = 3_000;
/// Samples of one 30 s window.
pub const WINDOW_SAMPLES: usize = FRAMES * HOP;
/// Frequency bins of the real DFT.
const BINS: usize = N_FFT / 2 + 1;

fn hz_to_mel(f: f64) -> f64 {
    let fsp = 200.0 / 3.0;
    let min_log = 1000.0 / fsp;
    let step = 6.4f64.ln() / 27.0;
    if f >= 1000.0 {
        min_log + (f / 1000.0).ln() / step
    } else {
        f / fsp
    }
}

fn mel_to_hz(m: f64) -> f64 {
    let fsp = 200.0 / 3.0;
    let min_log = 1000.0 / fsp;
    let step = 6.4f64.ln() / 27.0;
    if m >= min_log {
        1000.0 * (step * (m - min_log)).exp()
    } else {
        fsp * m
    }
}

/// The mel filter bank, `[n_mels][201]`, row-major (librosa's `filters.mel` with Slaney
/// scale and normalisation, as Whisper ships it).
pub fn filters(n_mels: usize) -> Vec<f32> {
    let top = SAMPLE_RATE as f64 / 2.0;
    let (lo, hi) = (hz_to_mel(0.0), hz_to_mel(top));
    let mel_f: Vec<f64> = (0..n_mels + 2)
        .map(|i| mel_to_hz(lo + (hi - lo) * i as f64 / (n_mels + 1) as f64))
        .collect();
    let mut w = vec![0f32; n_mels * BINS];
    for i in 0..n_mels {
        let enorm = 2.0 / (mel_f[i + 2] - mel_f[i]);
        let (d0, d1) = (mel_f[i + 1] - mel_f[i], mel_f[i + 2] - mel_f[i + 1]);
        for k in 0..BINS {
            let f = top * k as f64 / (BINS - 1) as f64;
            let lower = (f - mel_f[i]) / d0;
            let upper = (mel_f[i + 2] - f) / d1;
            w[i * BINS + k] = (lower.min(upper).max(0.0) * enorm) as f32;
        }
    }
    w
}

/// The log-mel spectrogram maker for one band count (its tables made once).
pub struct Mel {
    n_mels: usize,
    filters: Vec<f32>,
    /// Hann-windowed DFT basis: per bin, `N_FFT` cosines then `N_FFT` sines.
    basis: Vec<f32>,
}

impl Mel {
    pub fn new(n_mels: usize) -> Mel {
        let win: Vec<f64> = (0..N_FFT)
            .map(|n| 0.5 - 0.5 * (std::f64::consts::TAU * n as f64 / N_FFT as f64).cos())
            .collect();
        let mut basis = vec![0f32; BINS * 2 * N_FFT];
        for k in 0..BINS {
            for n in 0..N_FFT {
                // k·n mod N keeps the angle small and exact.
                let a = std::f64::consts::TAU * ((k * n) % N_FFT) as f64 / N_FFT as f64;
                basis[k * 2 * N_FFT + n] = (a.cos() * win[n]) as f32;
                basis[k * 2 * N_FFT + N_FFT + n] = (a.sin() * win[n]) as f32;
            }
        }
        Mel {
            n_mels,
            filters: filters(n_mels),
            basis,
        }
    }

    pub fn n_mels(&self) -> usize {
        self.n_mels
    }

    /// `[n_mels][frames]`, row-major, of `audio` (16 kHz mono) padded with silence or cut to
    /// `frames · 160` samples.
    pub fn log_mel(&self, audio: &[f32], frames: usize) -> Vec<f32> {
        let len = frames * HOP;
        let pad = N_FFT / 2;
        // The padded sound, reflected at both ends (as torch.stft's centre).
        let at = |i: isize| -> f32 {
            let n = len as isize;
            if n <= 1 {
                return 0.0;
            }
            let mut j = i;
            if j < 0 {
                j = -j;
            }
            if j >= n {
                j = 2 * (n - 1) - j;
            }
            let j = j.clamp(0, n - 1) as usize;
            audio.get(j).copied().unwrap_or(0.0)
        };
        let mut power = vec![0f32; BINS];
        let mut frame = vec![0f32; N_FFT];
        let mut out = vec![0f32; self.n_mels * frames];
        for f in 0..frames {
            let start = (f * HOP) as isize - pad as isize;
            if start >= 0 && start as usize + N_FFT <= audio.len().min(len) {
                frame.copy_from_slice(&audio[start as usize..start as usize + N_FFT]);
            } else {
                for (n, v) in frame.iter_mut().enumerate() {
                    *v = at(start + n as isize);
                }
            }
            for (k, p) in power.iter_mut().enumerate() {
                let b = &self.basis[k * 2 * N_FFT..(k + 1) * 2 * N_FFT];
                let re = dot(&frame, &b[..N_FFT]);
                let im = dot(&frame, &b[N_FFT..]);
                *p = re * re + im * im;
            }
            for m in 0..self.n_mels {
                let w = &self.filters[m * BINS..(m + 1) * BINS];
                out[m * frames + f] = dot(w, &power);
            }
        }
        let mut top = f32::MIN;
        for v in out.iter_mut() {
            *v = v.max(1e-10).log10();
            top = top.max(*v);
        }
        for v in out.iter_mut() {
            *v = (v.max(top - 8.0) + 4.0) / 4.0;
        }
        out
    }
}

/// A dot product, eight lanes at a time.
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let mut acc = f32x8::splat(0.0);
    let chunks = n / 8;
    for i in 0..chunks {
        let x = f32x8::from_slice(&a[i * 8..i * 8 + 8]);
        let y = f32x8::from_slice(&b[i * 8..i * 8 + 8]);
        acc += x * y;
    }
    let mut s = acc.reduce_sum();
    for i in chunks * 8..n {
        s += a[i] * b[i];
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test sound: two tones and a chirp, 0.8 s, rounded to f32 as the reference was.
    fn sound() -> Vec<f32> {
        (0..12_800)
            .map(|n| {
                let t = n as f64 / 16_000.0;
                (0.5 * (std::f64::consts::TAU * 440.0 * t).sin()
                    + 0.25 * (std::f64::consts::TAU * 3000.0 * t).sin()
                    + 0.1 * (std::f64::consts::TAU * (200.0 * t + 2000.0 * t * t)).sin())
                    as f32
            })
            .collect()
    }

    // Reference values from an independent f64 numpy implementation (librosa's Slaney
    // formula, numpy's rFFT, reflect-padded centred frames; target/tmp, not shipped).

    #[test]
    fn filters_match_the_reference() {
        for (n, want) in [
            (
                80,
                [1.999_024_1, 0.024_862_594, 0.0, 0.003_164_712, 0.026_664_86],
            ),
            (
                128,
                [3.190_985_5, 0.012_373_986, 0.0, 0.005_041_602, 0.024_841_2],
            ),
        ] {
            let f = filters(n);
            let row = |i: usize| &f[i * BINS..(i + 1) * BINS];
            let got = [
                f.iter().sum::<f32>(),
                row(0)[1],
                row(10)[12],
                row(n - 1).iter().cloned().fold(0.0, f32::max),
                row(n / 2).iter().sum::<f32>(),
            ];
            for (g, w) in got.iter().zip(want) {
                assert!(
                    (g - w).abs() < 2e-6 * w.abs().max(1.0),
                    "{n}: {got:?} vs {want:?}"
                );
            }
        }
    }

    #[test]
    fn log_mel_matches_the_reference() {
        let x = sound();
        type Case<'a> = (usize, &'a [(usize, usize, f32)], f32);
        let cases: [Case; 2] = [
            (
                80,
                &[
                    (0, 0, 1.074_823),
                    (11, 3, 1.439_231),
                    (10, 10, 1.348_781),
                    (12, 40, 1.293_523),
                    (11, 79, 1.436_947),
                    (7, 81, 0.370_756),
                    (30, 79, -0.233_465),
                    (40, 50, -0.538_118),
                ],
                -0.324_254,
            ),
            (
                128,
                &[
                    (0, 0, 0.999_064),
                    (18, 3, 1.486_432),
                    (16, 10, 1.344_291),
                    (17, 40, 1.337_491),
                    (11, 81, 0.426_523),
                    (30, 79, 0.236_428),
                    (127, 99, -0.491_243),
                ],
                -0.307_995,
            ),
        ];
        for (n, pts, mean) in cases {
            let m = Mel::new(n).log_mel(&x, 100);
            for &(b, f, want) in pts {
                let got = m[b * 100 + f];
                assert!(
                    (got - want).abs() < 1e-4,
                    "{n} bands, bin {b} frame {f}: {got} vs {want}"
                );
            }
            let got_mean = m.iter().sum::<f32>() / m.len() as f32;
            assert!(
                (got_mean - mean).abs() < 1e-4,
                "{n}: mean {got_mean} vs {mean}"
            );
        }
    }

    #[test]
    fn silence_is_flat_and_short_sound_is_padded() {
        let m = Mel::new(80);
        let z = m.log_mel(&[], 10);
        assert!(z.iter().all(|&v| (v - z[0]).abs() < 1e-6));
        // Longer sound than the frames: cut.
        let x = sound();
        assert_eq!(m.log_mel(&x, 20).len(), 80 * 20);
    }
}
