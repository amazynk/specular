//! The SBR QMF banks (§4.6.18.4): 32-band complex analysis of the core output and 64-band
//! (or, for downsampled SBR, 32-band) synthesis. Each slot's modulation is a DCT-IV and a
//! DST-IV (the latter a DCT-IV of the reversed input with alternating signs), each through a
//! complex FFT of a quarter of the transform size.

use std::f64::consts::PI;

use crate::data::sbr::QMF_WINDOW;
use crate::fft::{C, Dct4};

/// e^{iθ}
fn expi(theta: f64) -> C {
    C::new(theta.cos() as f32, theta.sin() as f32)
}

/// The 32-band analysis bank of one channel.
pub(crate) struct Analysis {
    /// x(n): the newest sample at the front of the 320 in use (`x[pos..pos + 320]`).
    x: Vec<f32>,
    pos: usize,
    dct: Dct4,
    /// 2·e^{−i(3π/128)(k+½)}, the phase the ¼-sample offset of the modulation leaves.
    post: [C; 32],
    a: [f32; 32],
    b: [f32; 32],
    c: [f32; 32],
    s: [f32; 32],
}

/// Room for this many slots before the history is moved back.
const ANALYSIS_ROOM: usize = 32 * 32;

impl Analysis {
    pub fn new() -> Self {
        let mut post = [C::default(); 32];
        for (k, p) in post.iter_mut().enumerate() {
            let c = expi(-3.0 * PI * (k as f64 + 0.5) / 128.0);
            *p = C::new(2.0 * c.re, 2.0 * c.im);
        }
        Analysis {
            x: vec![0.0; ANALYSIS_ROOM + 320],
            pos: ANALYSIS_ROOM,
            dct: Dct4::new(32),
            post,
            a: [0.0; 32],
            b: [0.0; 32],
            c: [0.0; 32],
            s: [0.0; 32],
        }
    }

    /// Analyses 32 samples (oldest first) into one slot of 32 subband samples:
    /// X(k) = 2 Σ_n u(n) e^{iπ(k+½)(2n−½)/64}.
    pub fn slot(&mut self, input: &[f32], out: &mut [C]) {
        if self.pos < 32 {
            // keep the newest 288 samples and make room again
            let keep = 288;
            self.x
                .copy_within(self.pos..self.pos + keep, ANALYSIS_ROOM + 32);
            self.pos = ANALYSIS_ROOM + 32;
        }
        self.pos -= 32;
        let x = &mut self.x[self.pos..self.pos + 320];
        for (j, &s) in input[..32].iter().enumerate() {
            x[31 - j] = s;
        }
        // u(n) = Σ_j x(n + 64j)·c(2(n + 64j))
        let x = &self.x[self.pos..self.pos + 320];
        let mut u = [0f32; 64];
        for (n, un) in u.iter_mut().enumerate() {
            let mut acc = 0.0f32;
            for j in 0..5 {
                let i = n + 64 * j;
                acc += x[i] * QMF_WINDOW[2 * i];
            }
            *un = acc;
        }
        // with ψ = (π/32)(k+½)(n+½): Σ_n u(n) e^{iψ} = DCT4(u(n) − u(63−n)) + i·DST4(u(n) +
        // u(63−n)) over n < 32, and the DST-IV is a DCT-IV of the reversed input with
        // alternating signs
        for n in 0..32 {
            self.a[n] = u[n] - u[63 - n];
            self.b[31 - n] = u[n] + u[63 - n];
        }
        self.dct.run(&self.a, &mut self.c);
        self.dct.run(&self.b, &mut self.s);
        for (k, o) in out[..32].iter_mut().enumerate() {
            let s = if k % 2 == 0 { self.s[k] } else { -self.s[k] };
            *o = C::new(self.c[k], s).mul(self.post[k]);
        }
    }
}

/// A synthesis bank of 64 bands, or 32 for downsampled SBR.
pub(crate) struct Synthesis {
    bands: usize,
    /// v(n), the newest block at the front of the `20·bands` in use.
    v: Vec<f32>,
    pos: usize,
    dct: Dct4,
    /// e^{−iδ_k}/64: the phase the modulation offset leaves beyond whole turns (none for
    /// 64 bands).
    pre: Vec<C>,
    a: Vec<f32>,
    b: Vec<f32>,
    c: Vec<f32>,
    s: Vec<f32>,
    /// The window, decimated for 32 bands.
    win: Vec<f32>,
}

const SYNTH_ROOM: usize = 32 * 128;

/// The time offset `o` of the synthesis modulation: 255 for 64 bands (§4.6.18.4.2). For the
/// 32-band bank of downsampled SBR, 127.5 is the offset at which analysis followed by
/// synthesis reconstructs the input (127 would leave aliasing about 30 dB down).
fn offset(bands: usize) -> f64 {
    if bands == 64 { 255.0 } else { 127.5 }
}

impl Synthesis {
    pub fn new(bands: usize) -> Self {
        let b = bands as f64;
        // v(n) = (1/64) Σ_k Re(X_k e^{iθ}), θ = π(k+½)(2n − o)/(2B) = φ − (2k+1)π − δ_k with
        // φ = (π/B)(k+½)(n+½) and δ_k = (π/B)(k+½)((o+1)/2 − 2B); the same 1/64 for both
        // sizes keeps the output level
        let extra = (offset(bands) + 1.0) / 2.0 - 2.0 * b;
        let pre = (0..bands)
            .map(|k| {
                let c = expi(-PI / b * (k as f64 + 0.5) * extra);
                C::new(c.re / 64.0, c.im / 64.0)
            })
            .collect();
        let step = 64 / bands;
        let win = (0..10 * bands).map(|n| QMF_WINDOW[n * step]).collect();
        Synthesis {
            bands,
            v: vec![0.0; SYNTH_ROOM + 20 * bands],
            pos: SYNTH_ROOM,
            dct: Dct4::new(bands),
            pre,
            a: vec![0.0; bands],
            b: vec![0.0; bands],
            c: vec![0.0; bands],
            s: vec![0.0; bands],
            win,
        }
    }

    /// Synthesises one slot (`x` has at least `bands` subband samples) into `bands` samples.
    pub fn slot(&mut self, x: &[C], out: &mut [f32]) {
        let b = self.bands;
        let n2 = 2 * b;
        if self.pos < n2 {
            let keep = 20 * b - n2;
            self.v
                .copy_within(self.pos..self.pos + keep, SYNTH_ROOM + n2);
            self.pos = SYNTH_ROOM + n2;
        }
        self.pos -= n2;
        // Re(X e^{iθ}) = −(A cos φ − B sin φ) for X e^{−iδ} = A + iB: v = (s − c)/64 with c
        // the DCT-IV of A and s the DST-IV of B, each mirrored for n ≥ B
        for (k, (xk, p)) in x[..b].iter().zip(&self.pre).enumerate() {
            let z = xk.mul(*p);
            self.a[k] = z.re;
            self.b[b - 1 - k] = z.im;
        }
        self.dct.run(&self.a, &mut self.c);
        self.dct.run(&self.b, &mut self.s);
        let v = &mut self.v[self.pos..self.pos + 20 * b];
        for n in 0..b {
            let s = if n % 2 == 0 { self.s[n] } else { -self.s[n] };
            v[n] = s - self.c[n];
            v[n2 - 1 - n] = s + self.c[n];
        }
        // out(j) = Σ_i w(B·i + j) with g built from v and w(n) = g(n)·c(n)
        let v = &self.v[self.pos..self.pos + 20 * b];
        for (j, o) in out[..b].iter_mut().enumerate() {
            let mut acc = 0.0f32;
            for i in 0..5 {
                acc += v[4 * b * i + j] * self.win[2 * b * i + j];
                acc += v[4 * b * i + 3 * b + j] * self.win[2 * b * i + b + j];
            }
            *o = acc;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The direct formula of §4.6.18.4.1 for one slot, given x(0..319) newest first.
    fn analysis_direct(x: &[f32]) -> Vec<C> {
        let mut u = [0.0f64; 64];
        for (n, un) in u.iter_mut().enumerate() {
            for j in 0..5 {
                let i = n + 64 * j;
                *un += f64::from(x[i]) * f64::from(QMF_WINDOW[2 * i]);
            }
        }
        (0..32)
            .map(|k| {
                let (mut re, mut im) = (0.0, 0.0);
                for (n, &un) in u.iter().enumerate() {
                    let t = PI / 64.0 * (k as f64 + 0.5) * (2.0 * n as f64 - 0.5);
                    re += 2.0 * un * t.cos();
                    im += 2.0 * un * t.sin();
                }
                C::new(re as f32, im as f32)
            })
            .collect()
    }

    #[test]
    fn analysis_matches_formula() {
        let mut a = Analysis::new();
        let sig: Vec<f32> = (0..32 * 40)
            .map(|i| (i as f32 * 0.071).sin() * 3000.0 + (i as f32 * 1.3).cos() * 500.0)
            .collect();
        let mut out = [C::default(); 32];
        for l in 0..40 {
            a.slot(&sig[32 * l..32 * l + 32], &mut out);
            if l >= 10 {
                // x(n) is the input sample 32·l + 31 − n
                let x: Vec<f32> = (0..320).map(|n| sig[32 * l + 31 - n]).collect();
                let want = analysis_direct(&x);
                for k in 0..32 {
                    let (g, w) = (out[k], want[k]);
                    let tol = 2e-3
                        * want
                            .iter()
                            .map(|c| c.re.abs() + c.im.abs())
                            .fold(1.0, f32::max);
                    assert!(
                        (g.re - w.re).abs() < tol && (g.im - w.im).abs() < tol,
                        "{l} {k}: {g:?} {w:?}"
                    );
                }
            }
        }
    }

    /// The direct synthesis formula (§4.6.18.4.2) against the fast one.
    #[test]
    fn synthesis_matches_formula() {
        for bands in [64usize, 32] {
            let mut s = Synthesis::new(bands);
            let b = bands as f64;
            let mut v = vec![0.0f64; 20 * bands];
            let mut out = vec![0.0f32; bands];
            for l in 0..30 {
                let x: Vec<C> = (0..bands)
                    .map(|k| {
                        C::new(
                            ((k * 7 + l * 3) % 11) as f32 - 5.0,
                            ((k * 5 + l) % 13) as f32 - 6.0,
                        )
                    })
                    .collect();
                s.slot(&x, &mut out);
                v.copy_within(0..18 * bands, 2 * bands);
                for (n, vn) in v[..2 * bands].iter_mut().enumerate() {
                    let mut acc = 0.0;
                    for (k, xk) in x.iter().enumerate() {
                        let t =
                            PI / (2.0 * b) * (k as f64 + 0.5) * (2.0 * n as f64 - offset(bands));
                        acc += f64::from(xk.re) * t.cos() - f64::from(xk.im) * t.sin();
                    }
                    *vn = acc / 64.0;
                }
                let step = 64 / bands;
                for (j, &o) in out.iter().enumerate() {
                    let mut acc = 0.0;
                    for i in 0..5 {
                        acc += v[4 * bands * i + j]
                            * f64::from(QMF_WINDOW[(2 * bands * i + j) * step]);
                        acc += v[4 * bands * i + 3 * bands + j]
                            * f64::from(QMF_WINDOW[(2 * bands * i + bands + j) * step]);
                    }
                    assert!(
                        (acc - f64::from(o)).abs() < 1e-3,
                        "{bands} {l} {j}: {acc} {o}"
                    );
                }
            }
        }
    }

    /// Finds the integer delay `d` minimising Σ (out[i + d] − want(i))² over a stretch, and
    /// returns it with the signal-to-error ratio in dB.
    fn best_fit(
        out: &[f32],
        want: impl Fn(usize) -> f64,
        range: std::ops::Range<usize>,
    ) -> (usize, f64) {
        let (mut best, mut best_d) = (f64::MAX, 0);
        for d in 0..1200 {
            let mut e = 0.0f64;
            for i in range.clone() {
                let diff = f64::from(out[i + d]) - want(i);
                e += diff * diff;
            }
            if e < best {
                best = e;
                best_d = d;
            }
        }
        let sig: f64 = range.map(|i| want(i) * want(i)).sum();
        (best_d, 10.0 * (sig / best.max(1e-30)).log10())
    }

    /// Analysis then 32-band synthesis reconstructs the input with a fixed delay.
    #[test]
    fn analysis_synthesis_reconstructs() {
        let mut a = Analysis::new();
        let mut s = Synthesis::new(32);
        let n = 32 * 200;
        let f = |i: usize| (i as f64 * 0.05).sin() * 8000.0 + (i as f64 * 0.31).sin() * 3000.0;
        let sig: Vec<f32> = (0..n).map(|i| f(i) as f32).collect();
        let mut out = vec![0.0f32; n];
        let mut slot = [C::default(); 32];
        for l in 0..n / 32 {
            a.slot(&sig[32 * l..32 * l + 32], &mut slot);
            s.slot(&slot, &mut out[32 * l..32 * l + 32]);
        }
        let (d, snr) = best_fit(&out, f, 1000..4000);
        // the QMF bank's own reconstruction limit for this signal is 59.4 dB
        assert!(d == 289 && snr > 59.0, "delay {d}: SNR {snr:.1} dB");
    }

    /// Analysis then 64-band synthesis (the upper bands empty) upsamples by two: this uses
    /// every tap of the window, odd ones included.
    #[test]
    fn analysis_synthesis_upsamples() {
        let mut a = Analysis::new();
        let mut s = Synthesis::new(64);
        let n = 32 * 200;
        let f = |t: f64| (t * 0.05).sin() * 8000.0 + (t * 0.31).sin() * 3000.0;
        let sig: Vec<f32> = (0..n).map(|i| f(i as f64) as f32).collect();
        let mut out = vec![0.0f32; 2 * n];
        let mut slot = [C::default(); 64];
        for l in 0..n / 32 {
            a.slot(&sig[32 * l..32 * l + 32], &mut slot[..32]);
            s.slot(&slot, &mut out[64 * l..64 * l + 64]);
        }
        let (d, snr) = best_fit(&out, |i| f(i as f64 / 2.0), 2000..8000);
        assert!(d == 578 && snr > 59.0, "delay {d}: SNR {snr:.1} dB");
    }
}
