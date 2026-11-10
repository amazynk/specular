//! In-place complex FFT for the sizes the filterbanks need: radix-2 for powers of two, mixed
//! radix 2/3/5 for the 960-sample frame's 480 and 60 points. The IMDCT and the forward MDCT
//! (for long-term prediction) are built on it (DCT-IV through an N/4-point complex FFT with
//! pre- and post-twiddles).

use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct C {
    pub re: f32,
    pub im: f32,
}

impl C {
    #[inline]
    pub fn new(re: f32, im: f32) -> Self {
        C { re, im }
    }
    #[inline]
    pub fn mul(self, o: C) -> C {
        C::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
    #[inline]
    pub fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
    #[inline]
    pub fn sub(self, o: C) -> C {
        C::new(self.re - o.re, self.im - o.im)
    }
    /// e^{iθ}
    pub fn expi(theta: f64) -> C {
        C::new(theta.cos() as f32, theta.sin() as f32)
    }
}

/// A forward FFT (`X_k = Σ x_n e^{-2πikn/N}`) of one size.
pub(crate) struct Fft {
    n: usize,
    /// The mixed-radix transform, for sizes that are not a power of two.
    mixed: Option<Mixed>,
    /// Index pairs the bit-reversal permutation swaps.
    swaps: Vec<(u16, u16)>,
    /// Twiddles of each stage after the first, contiguous: stage of length `len` uses
    /// e^{-2πik/len} for k < len/2.
    tw: Vec<C>,
}

impl Fft {
    pub fn new(n: usize) -> Self {
        if !n.is_power_of_two() {
            return Fft {
                n,
                mixed: Some(Mixed::new(n)),
                swaps: Vec::new(),
                tw: Vec::new(),
            };
        }
        assert!((2..=1 << 15).contains(&n));
        let bits = n.trailing_zeros();
        let swaps = (0..n as u32)
            .filter_map(|i| {
                let j = i.reverse_bits() >> (32 - bits);
                (i < j).then_some((i as u16, j as u16))
            })
            .collect();
        let mut tw = Vec::new();
        let mut len = 4;
        while len <= n {
            for k in 0..len / 2 {
                tw.push(C::expi(-2.0 * PI * k as f64 / len as f64));
            }
            len *= 2;
        }
        Fft {
            n,
            mixed: None,
            swaps,
            tw,
        }
    }

    pub fn run(&self, x: &mut [C]) {
        let n = self.n;
        if let Some(m) = &self.mixed {
            m.run(&mut x[..n]);
            return;
        }
        let x = &mut x[..n];
        for &(i, j) in &self.swaps {
            x.swap(usize::from(i), usize::from(j));
        }
        // first stage: twiddle 1
        for pair in x.chunks_exact_mut(2) {
            let (a, b) = (pair[0], pair[1]);
            pair[0] = a.add(b);
            pair[1] = a.sub(b);
        }
        let mut len = 4;
        let mut tw = &self.tw[..];
        while len <= n {
            let half = len / 2;
            let (stage, rest) = tw.split_at(half);
            tw = rest;
            for block in x.chunks_exact_mut(len) {
                let (lo, hi) = block.split_at_mut(half);
                for ((a, b), w) in lo.iter_mut().zip(hi.iter_mut()).zip(stage) {
                    let t = b.mul(*w);
                    *b = a.sub(t);
                    *a = a.add(t);
                }
            }
            len *= 2;
        }
    }
}

/// A mixed-radix (2, 3, 5) decimation-in-time FFT, recursive, for the few sizes that are not
/// a power of two.
struct Mixed {
    n: usize,
    factors: Vec<usize>,
    /// e^{-2πij/n} for j < n.
    tw: Vec<C>,
}

impl Mixed {
    fn new(n: usize) -> Self {
        let mut factors = Vec::new();
        let mut rest = n;
        for p in [2, 3, 5] {
            while rest.is_multiple_of(p) {
                factors.push(p);
                rest /= p;
            }
        }
        assert!(rest == 1 && n > 1, "FFT size {n} is not 2^a 3^b 5^c");
        let tw = (0..n)
            .map(|j| C::expi(-2.0 * PI * j as f64 / n as f64))
            .collect();
        Mixed { n, factors, tw }
    }

    fn run(&self, x: &mut [C]) {
        let input = x.to_vec();
        self.rec(&input, 0, 1, x, 0);
    }

    /// The DFT of the `out.len()` inputs `input[start + i·stride]`, into `out`.
    fn rec(&self, input: &[C], start: usize, stride: usize, out: &mut [C], f: usize) {
        let len = out.len();
        if len == 1 {
            out[0] = input[start];
            return;
        }
        let p = self.factors[f];
        let m = len / p;
        for r in 0..p {
            self.rec(
                input,
                start + r * stride,
                stride * p,
                &mut out[r * m..(r + 1) * m],
                f + 1,
            );
        }
        // w_len^j is tw[j·step]
        let step = self.n / len;
        let mut t = [C::default(); 5];
        for k in 0..m {
            for (r, tr) in t[..p].iter_mut().enumerate() {
                *tr = out[r * m + k].mul(self.tw[(r * k * step) % self.n]);
            }
            for q in 0..p {
                let mut s = t[0];
                for (r, tr) in t[..p].iter().enumerate().skip(1) {
                    s = s.add(tr.mul(self.tw[(r * q * m * step) % self.n]));
                }
                out[q * m + k] = s;
            }
        }
    }
}

/// DCT-IV of one size `m`: `y[k] = Σ_j x[j] cos(π/m (j + ½)(k + ½))`, through an `m/2`-point
/// complex FFT with pre- and post-twiddles.
pub(crate) struct Dct4 {
    m: usize,
    fft: Fft,
    tw: Vec<C>,
    buf: Vec<C>,
}

impl Dct4 {
    pub fn new(m: usize) -> Self {
        // twiddle exp(-iπ(8p+1)/(8m)), used before and after the FFT
        let tw = (0..m / 2)
            .map(|p| C::expi(-PI * (8 * p + 1) as f64 / (8 * m) as f64))
            .collect();
        Dct4 {
            m,
            fft: Fft::new(m / 2),
            tw,
            buf: vec![C::default(); m / 2],
        }
    }

    /// `x` and `out` have `m` values.
    pub fn run(&mut self, x: &[f32], out: &mut [f32]) {
        let m = self.m;
        for p in 0..m / 2 {
            self.buf[p] = C::new(x[2 * p], x[m - 1 - 2 * p]).mul(self.tw[p]);
        }
        self.fft.run(&mut self.buf);
        for q in 0..m / 2 {
            let v = self.buf[q].mul(self.tw[q]);
            out[2 * q] = v.re;
            out[m - 1 - 2 * q] = -v.im;
        }
    }
}

/// Inverse MDCT of one size: `n/2` coefficients to `n` windowed-domain samples, scaled by `2/n`
/// times a caller scale so `y[t] = s·(2/N)·Σ_k X_k cos(2π/N (t + n0)(k + ½))`, n0 = (N/2+1)/2.
pub(crate) struct Imdct {
    n: usize,
    fft: Fft,
    pre: Vec<C>,
    buf: Vec<C>,
    z: Vec<f32>,
}

impl Imdct {
    pub fn new(n: usize) -> Self {
        let m = n / 2;
        let fft = Fft::new(m / 2);
        // DCT-IV twiddle exp(-iπ(8p+1)/(8M)), used before and after the FFT
        let pre = (0..m / 2)
            .map(|p| C::expi(-PI * (8 * p + 1) as f64 / (8 * m) as f64))
            .collect();
        Imdct {
            n,
            fft,
            pre,
            buf: vec![C::default(); m / 2],
            z: vec![0.0; m],
        }
    }

    /// `x` has `n/2` coefficients; writes `n` samples to `out`.
    pub fn run(&mut self, x: &[f32], out: &mut [f32], scale: f32) {
        let m = self.n / 2;
        // DCT-IV of length M: z[k] = Σ x[j] cos(π/M (j+½)(k+½))
        for p in 0..m / 2 {
            let v = C::new(x[2 * p], x[m - 1 - 2 * p]);
            self.buf[p] = v.mul(self.pre[p]);
        }
        self.fft.run(&mut self.buf);
        let s = scale * 2.0 / self.n as f32;
        for q in 0..m / 2 {
            let v = self.buf[q].mul(self.pre[q]);
            self.z[2 * q] = v.re * s;
            self.z[m - 1 - 2 * q] = -v.im * s;
        }
        // unfold: y[t] with m = t + M/2
        let half = m / 2;
        for (t, o) in out[..self.n].iter_mut().enumerate() {
            let k = t + half;
            *o = if k < m {
                self.z[k]
            } else if k < 2 * m {
                -self.z[2 * m - 1 - k]
            } else {
                -self.z[k - 2 * m]
            };
        }
    }
}

/// Forward MDCT of one size: `n` samples to `n/2` coefficients,
/// `X_k = 2·Σ_t x[t] cos(2π/N (t + n0)(k + ½))`, n0 = (N/2+1)/2: the encoder's transform,
/// which long-term prediction runs on its time-domain estimate.
pub(crate) struct Mdct {
    n: usize,
    dct: Dct4,
    fold: Vec<f32>,
}

impl Mdct {
    pub fn new(n: usize) -> Self {
        Mdct {
            n,
            dct: Dct4::new(n / 2),
            fold: vec![0.0; n / 2],
        }
    }

    /// `x` has `n` samples; writes `n/2` coefficients to `out`.
    pub fn run(&mut self, x: &[f32], out: &mut [f32]) {
        let m = self.n / 2;
        let h = m / 2;
        // fold the quarters a b c d into (-c_r - d, a - b_r), then a DCT-IV
        for i in 0..h {
            self.fold[i] = -x[3 * h - 1 - i] - x[3 * h + i];
            self.fold[h + i] = x[i] - x[m - 1 - i];
        }
        self.dct.run(&self.fold, &mut out[..m]);
        for v in &mut out[..m] {
            *v *= 2.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_matches_dft() {
        let n = 64;
        let f = Fft::new(n);
        let x: Vec<C> = (0..n)
            .map(|i| C::new((i as f32 * 0.37).sin(), (i as f32).cos() * 0.5))
            .collect();
        let mut y = x.clone();
        f.run(&mut y);
        for (k, yk) in y.iter().enumerate() {
            let mut s = C::default();
            for (j, xj) in x.iter().enumerate() {
                s = s.add(xj.mul(C::expi(-2.0 * PI * (k * j) as f64 / n as f64)));
            }
            assert!((s.re - yk.re).abs() < 1e-4 && (s.im - yk.im).abs() < 1e-4);
        }
    }

    #[test]
    fn mixed_fft_matches_dft() {
        for n in [60usize, 480] {
            let f = Fft::new(n);
            let x: Vec<C> = (0..n)
                .map(|i| C::new((i as f32 * 0.37).sin(), (i as f32).cos() * 0.5))
                .collect();
            let mut y = x.clone();
            f.run(&mut y);
            for (k, yk) in y.iter().enumerate() {
                let mut s = C::default();
                for (j, xj) in x.iter().enumerate() {
                    s = s.add(xj.mul(C::expi(-2.0 * PI * ((k * j) % n) as f64 / n as f64)));
                }
                assert!(
                    (s.re - yk.re).abs() < 2e-3 && (s.im - yk.im).abs() < 2e-3,
                    "n {n} k {k}"
                );
            }
        }
    }

    #[test]
    fn mdct_matches_formula() {
        for n in [240usize, 256, 1920, 2048] {
            let x: Vec<f32> = (0..n).map(|i| ((i * 7 % 13) as f32 - 6.0) * 0.1).collect();
            let mut out = vec![0.0; n / 2];
            Mdct::new(n).run(&x, &mut out);
            let n0 = (n as f64 / 2.0 + 1.0) / 2.0;
            for (k, &o) in out.iter().enumerate() {
                let mut s = 0.0f64;
                for (t, &v) in x.iter().enumerate() {
                    s += f64::from(v)
                        * (2.0 * PI / n as f64 * (t as f64 + n0) * (k as f64 + 0.5)).cos();
                }
                s *= 2.0;
                assert!((s - f64::from(o)).abs() < 2e-2, "n {n} k {k}: {s} vs {o}");
            }
        }
    }

    #[test]
    fn imdct_matches_formula() {
        for n in [240usize, 256, 1920, 2048] {
            let m = n / 2;
            let x: Vec<f32> = (0..m).map(|i| ((i * 7 % 13) as f32 - 6.0) * 0.1).collect();
            let mut out = vec![0.0; n];
            Imdct::new(n).run(&x, &mut out, 1.0);
            let n0 = (m as f64 + 1.0) / 2.0;
            for (t, &o) in out.iter().enumerate() {
                let mut s = 0.0f64;
                for (k, &xk) in x.iter().enumerate() {
                    s += f64::from(xk)
                        * (2.0 * PI / n as f64 * (t as f64 + n0) * (k as f64 + 0.5)).cos();
                }
                s *= 2.0 / n as f64;
                assert!((s - f64::from(o)).abs() < 1e-4, "n {n} t {t}: {s} vs {o}");
            }
        }
    }
}
