//! The synthesis filterbank (§4.6.11): IMDCT of each window, window shapes (sine and
//! Kaiser–Bessel derived), the four window sequences and overlap-add.

use std::f64::consts::PI;

use crate::fft::{Imdct, Mdct};

/// `window_sequence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Sequence {
    #[default]
    OnlyLong,
    LongStart,
    EightShort,
    LongStop,
}

impl Sequence {
    pub fn from_bits(v: u32) -> Sequence {
        match v & 3 {
            0 => Sequence::OnlyLong,
            1 => Sequence::LongStart,
            2 => Sequence::EightShort,
            _ => Sequence::LongStop,
        }
    }
}

/// The rising half of a sine window of length `n`.
fn sine(n: usize) -> Vec<f32> {
    (0..n / 2)
        .map(|i| (PI / n as f64 * (i as f64 + 0.5)).sin() as f32)
        .collect()
}

/// Zeroth-order modified Bessel function of the first kind.
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..60 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// The rising half of a Kaiser–Bessel derived window of length `n` with parameter `alpha`.
fn kbd(n: usize, alpha: f64) -> Vec<f32> {
    let half = n / 2;
    let kaiser: Vec<f64> = (0..=half)
        .map(|j| {
            let r = 2.0 * j as f64 / half as f64 - 1.0;
            bessel_i0(PI * alpha * (1.0 - r * r).max(0.0).sqrt())
        })
        .collect();
    let total: f64 = kaiser.iter().sum();
    let mut acc = 0.0;
    (0..half)
        .map(|i| {
            acc += kaiser[i];
            (acc / total).sqrt() as f32
        })
        .collect()
}

/// Per-channel filterbank state, for one frame length (1024, or 960 for DAB+).
pub(crate) struct Filterbank {
    /// Samples per frame (N/2 of the long transform).
    n: usize,
    long: Imdct,
    short: Imdct,
    /// The forward transform long-term prediction runs on its estimate (made on first use).
    mdct: Option<Mdct>,
    /// Rising window halves by shape (0 sine, 1 KBD), long and short.
    win_long: [Vec<f32>; 2],
    win_short: [Vec<f32>; 2],
    buf: Vec<f32>,
    tmp: Vec<f32>,
}

/// Overlap state of one channel.
#[derive(Clone)]
pub(crate) struct Overlap {
    pub saved: Vec<f32>,
    pub prev_shape: usize,
}

impl Overlap {
    pub fn new(frame_len: usize) -> Self {
        Overlap {
            saved: vec![0.0; frame_len],
            prev_shape: 0,
        }
    }
}

impl Filterbank {
    pub fn new(frame_len: usize) -> Self {
        let n = frame_len;
        let s = n / 8;
        Filterbank {
            n,
            long: Imdct::new(2 * n),
            short: Imdct::new(2 * s),
            mdct: None,
            win_long: [sine(2 * n), kbd(2 * n, 4.0)],
            win_short: [sine(2 * s), kbd(2 * s, 6.0)],
            buf: vec![0.0; 2 * n],
            tmp: vec![0.0; 2 * s],
        }
    }

    /// Applies the window of a long-transform frame to `buf` (2N samples) in place: the
    /// previous frame's shape rising, the current one's falling, as the sequence shapes it.
    fn window_long(&self, buf: &mut [f32], seq: Sequence, shape: usize, prev: usize) {
        let (n, s) = (self.n, self.n / 8);
        let flat = (n - s) / 2;
        let wl_prev = &self.win_long[prev];
        let wl = &self.win_long[shape];
        let ws_prev = &self.win_short[prev];
        let ws = &self.win_short[shape];
        match seq {
            Sequence::LongStop => {
                buf[..flat].fill(0.0);
                for i in 0..s {
                    buf[flat + i] *= ws_prev[i];
                }
            }
            _ => {
                for i in 0..n {
                    buf[i] *= wl_prev[i];
                }
            }
        }
        match seq {
            Sequence::LongStart => {
                for i in 0..s {
                    buf[n + flat + i] *= ws[s - 1 - i];
                }
                buf[n + flat + s..2 * n].fill(0.0);
            }
            _ => {
                for i in 0..n {
                    buf[n + i] *= wl[n - 1 - i];
                }
            }
        }
    }

    /// Turns N spectral coefficients into N output samples (in the 16-bit domain the
    /// coefficients are in), updating the channel's overlap.
    pub fn synth(
        &mut self,
        spec: &[f32],
        seq: Sequence,
        shape: usize,
        ov: &mut Overlap,
        out: &mut [f32],
    ) {
        let (n, s) = (self.n, self.n / 8);
        let flat = (n - s) / 2;
        let shape = shape.min(1);
        let prev = ov.prev_shape.min(1);
        let mut buf = std::mem::take(&mut self.buf);
        buf.fill(0.0);
        if seq == Sequence::EightShort {
            let ws_prev = &self.win_short[prev];
            let ws = &self.win_short[shape];
            for w in 0..8 {
                self.short.run(&spec[w * s..w * s + s], &mut self.tmp, 1.0);
                let rise = if w == 0 { ws_prev } else { ws };
                let off = flat + s * w;
                for i in 0..s {
                    buf[off + i] += self.tmp[i] * rise[i];
                    buf[off + s + i] += self.tmp[s + i] * ws[s - 1 - i];
                }
            }
        } else {
            self.long.run(&spec[..n], &mut buf, 1.0);
            self.window_long(&mut buf, seq, shape, prev);
        }
        for i in 0..n {
            out[i] = buf[i] + ov.saved[i];
        }
        ov.saved.copy_from_slice(&buf[n..2 * n]);
        ov.prev_shape = shape;
        self.buf = buf;
    }

    /// Long-term prediction's spectral estimate (§4.6.6): the 2N-sample time estimate `x`,
    /// windowed as this frame is (`prev_shape` the overlap's shape, before `synth` updates
    /// it), through the forward MDCT into `out` (N coefficients).
    pub fn analyse(
        &mut self,
        x: &mut [f32],
        seq: Sequence,
        shape: usize,
        prev_shape: usize,
        out: &mut [f32],
    ) {
        let n = self.n;
        self.window_long(&mut x[..2 * n], seq, shape.min(1), prev_shape.min(1));
        self.mdct
            .get_or_insert_with(|| Mdct::new(2 * n))
            .run(&x[..2 * n], &mut out[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_are_power_complementary() {
        for w in [sine(2048), kbd(2048, 4.0), sine(256), kbd(256, 6.0)] {
            let n = w.len();
            for i in 0..n {
                let s = w[i] * w[i] + w[n - 1 - i] * w[n - 1 - i];
                assert!((s - 1.0).abs() < 1e-5, "{s}");
            }
        }
    }

    #[test]
    fn analysis_and_synthesis_reconstruct() {
        // the forward path long-term prediction uses, then the synthesis: both frame
        // lengths, the window shape switching between frames
        for n in [1024usize, 960] {
            let mut fb = Filterbank::new(n);
            let mut enc = Filterbank::new(n);
            let mut ov = Overlap::new(n);
            let signal: Vec<f32> = (0..n * 5)
                .map(|i| ((i as f32) * 0.013).sin() * 1000.0)
                .collect();
            let mut out = vec![0.0; n];
            let mut spec = vec![0.0; n];
            let mut prev = 0;
            for f in 0..4 {
                let shape = f % 2;
                let mut seg = signal[f * n..f * n + 2 * n].to_vec();
                enc.analyse(&mut seg, Sequence::OnlyLong, shape, prev, &mut spec);
                fb.synth(&spec, Sequence::OnlyLong, shape, &mut ov, &mut out);
                prev = shape;
                if f > 0 {
                    for i in 0..n {
                        let want = signal[f * n + i];
                        assert!(
                            (out[i] - want).abs() < 0.05,
                            "n {n} frame {f} {i}: {} {want}",
                            out[i]
                        );
                    }
                }
            }
        }
    }

    /// A forward MDCT, for checking time-domain alias cancellation.
    fn mdct(x: &[f32]) -> Vec<f32> {
        let n = x.len();
        let n0 = (n as f64 / 2.0 + 1.0) / 2.0;
        (0..n / 2)
            .map(|k| {
                let mut s = 0.0f64;
                for (t, &v) in x.iter().enumerate() {
                    s += f64::from(v)
                        * (2.0 * PI / n as f64 * (t as f64 + n0) * (k as f64 + 0.5)).cos();
                }
                (2.0 * s) as f32
            })
            .collect()
    }

    #[test]
    fn long_frames_reconstruct() {
        let mut fb = Filterbank::new(1024);
        let mut ov = Overlap::new(1024);
        let signal: Vec<f32> = (0..1024 * 4)
            .map(|i| ((i as f32) * 0.013).sin() * 1000.0)
            .collect();
        let win = &fb.win_long[1].clone();
        let mut out = vec![0.0; 1024];
        for f in 0..3 {
            let seg = &signal[f * 1024..f * 1024 + 2048];
            let w: Vec<f32> = (0..2048)
                .map(|i| seg[i] * if i < 1024 { win[i] } else { win[2047 - i] })
                .collect();
            let spec = mdct(&w);
            fb.synth(&spec, Sequence::OnlyLong, 1, &mut ov, &mut out);
            if f > 0 {
                for i in 0..1024 {
                    let want = signal[f * 1024 + i];
                    assert!(
                        (out[i] - want).abs() < 0.05,
                        "frame {f} {i}: {} {want}",
                        out[i]
                    );
                }
            }
        }
    }
}
