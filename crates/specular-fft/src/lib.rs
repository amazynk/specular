//! Fast Fourier transforms for the science views.
//!
//! [`Fft`] plans a forward transform of any length `n`: a mixed-radix Stockham autosort FFT
//! when every prime factor of `n` is small (radix-4, 2, 3 and 5 butterflies, a generic
//! butterfly for odd primes up to [`MAX_RADIX`]), else Bluestein's chirp-z algorithm over a
//! power-of-two transform. Samples are `f32` complex; twiddles are computed in `f64`.
//!
//! [`power_spectrum`] is the 2-D view: the centred (`fftshift`-ed) `log₁₀ |F|²` of an image
//! plane, real rows transformed two at a time as one complex row, rows and columns in
//! parallel on rayon's pool. [`frequency`] maps a column or row of that view to its spatial
//! frequency.

use std::f64::consts::PI;
use std::ops::{Add, Mul, Sub};

use rayon::prelude::*;

/// The largest prime factor handled by a direct butterfly; larger ones go through Bluestein.
pub const MAX_RADIX: usize = 61;

/// A complex sample.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    pub re: f32,
    pub im: f32,
}

impl Complex {
    pub const ZERO: Complex = Complex { re: 0.0, im: 0.0 };

    pub const fn new(re: f32, im: f32) -> Complex {
        Complex { re, im }
    }

    /// `e^{iθ}`, computed in `f64`.
    fn cis(theta: f64) -> Complex {
        let (s, c) = theta.sin_cos();
        Complex::new(c as f32, s as f32)
    }

    pub fn conj(self) -> Complex {
        Complex::new(self.re, -self.im)
    }

    /// `|z|²`.
    pub fn norm_sqr(self) -> f32 {
        self.re * self.re + self.im * self.im
    }

    fn scale(self, k: f32) -> Complex {
        Complex::new(self.re * k, self.im * k)
    }

    /// `-i·z`.
    fn mul_neg_i(self) -> Complex {
        Complex::new(self.im, -self.re)
    }
}

impl Add for Complex {
    type Output = Complex;
    fn add(self, o: Complex) -> Complex {
        Complex::new(self.re + o.re, self.im + o.im)
    }
}

impl Sub for Complex {
    type Output = Complex;
    fn sub(self, o: Complex) -> Complex {
        Complex::new(self.re - o.re, self.im - o.im)
    }
}

impl Mul for Complex {
    type Output = Complex;
    fn mul(self, o: Complex) -> Complex {
        Complex::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}

/// One Stockham pass: radix `p` over sub-transforms of length `l` already done.
#[derive(Debug)]
struct Stage {
    p: usize,
    l: usize,
    /// `W_{lp}^{q·f}` at `(q - 1)·l + f`, for q in 1..p and f in 0..l.
    twiddles: Vec<Complex>,
    /// `W_p^k` for the generic butterfly.
    roots: Vec<Complex>,
}

#[derive(Debug)]
enum Kind {
    Stockham(Vec<Stage>),
    Bluestein {
        inner: Box<Fft>,
        /// `e^{-iπk²/n}`, k in 0..n.
        chirp: Vec<Complex>,
        /// The transformed conjugate chirp, already divided by the inner length.
        kernel: Vec<Complex>,
    },
}

/// A planned forward FFT of one length.
#[derive(Debug)]
pub struct Fft {
    n: usize,
    kind: Kind,
}

impl Fft {
    /// Plans a transform of length `n` (0 and 1 are identities).
    pub fn new(n: usize) -> Fft {
        let factors = factor(n);
        if factors.iter().all(|&p| p <= MAX_RADIX) {
            return Fft {
                n,
                kind: Kind::Stockham(stages(n, &factors)),
            };
        }
        let m = (2 * n - 1).next_power_of_two();
        let inner = Fft::new(m);
        let two_n = 2 * n as u64;
        let chirp: Vec<Complex> = (0..n as u64)
            .map(|k| {
                let k2 = (k * k) % two_n;
                Complex::cis(-PI * k2 as f64 / n as f64)
            })
            .collect();
        let mut kernel = vec![Complex::ZERO; m];
        kernel[0] = chirp[0].conj();
        for k in 1..n {
            kernel[k] = chirp[k].conj();
            kernel[m - k] = chirp[k].conj();
        }
        let mut scratch = vec![Complex::ZERO; inner.scratch_len()];
        inner.forward(&mut kernel, &mut scratch);
        let k = 1.0 / m as f32;
        for v in &mut kernel {
            *v = v.scale(k);
        }
        Fft {
            n,
            kind: Kind::Bluestein {
                inner: Box::new(inner),
                chirp,
                kernel,
            },
        }
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Scratch the transforms need, in samples.
    pub fn scratch_len(&self) -> usize {
        match &self.kind {
            Kind::Stockham(_) => self.n,
            Kind::Bluestein { inner, .. } => inner.n + inner.scratch_len(),
        }
    }

    /// `X_k = Σ x_j e^{-2πi jk/n}` in place. `scratch` holds at least [`Fft::scratch_len`]
    /// samples.
    pub fn forward(&self, data: &mut [Complex], scratch: &mut [Complex]) {
        let n = self.n;
        let data = &mut data[..n];
        match &self.kind {
            Kind::Stockham(stages) => {
                let scratch = &mut scratch[..n];
                let mut in_data = true;
                for s in stages {
                    if in_data {
                        pass(s, n, data, scratch);
                    } else {
                        pass(s, n, scratch, data);
                    }
                    in_data = !in_data;
                }
                if !in_data {
                    data.copy_from_slice(scratch);
                }
            }
            Kind::Bluestein {
                inner,
                chirp,
                kernel,
            } => {
                let m = inner.n;
                let (a, inner_scratch) = scratch.split_at_mut(m);
                for k in 0..n {
                    a[k] = data[k] * chirp[k];
                }
                a[n..].fill(Complex::ZERO);
                inner.forward(a, inner_scratch);
                // Inverse by conjugation: ifft(A·B) = conj(fft(conj(A·B))) (the 1/m is in B).
                for (v, k) in a.iter_mut().zip(kernel) {
                    *v = (*v * *k).conj();
                }
                inner.forward(a, inner_scratch);
                for k in 0..n {
                    data[k] = a[k].conj() * chirp[k];
                }
            }
        }
    }

    /// The unnormalised inverse, `x_j = Σ X_k e^{2πi jk/n}`, in place.
    pub fn inverse(&self, data: &mut [Complex], scratch: &mut [Complex]) {
        for v in data[..self.n].iter_mut() {
            *v = v.conj();
        }
        self.forward(data, scratch);
        for v in data[..self.n].iter_mut() {
            *v = v.conj();
        }
    }
}

/// Prime factors of `n`, fours first (each a radix-4 pass), then 2, 3, 5 and larger primes.
fn factor(mut n: usize) -> Vec<usize> {
    let mut out = Vec::new();
    if n < 2 {
        return out;
    }
    while n.is_multiple_of(4) {
        out.push(4);
        n /= 4;
    }
    let mut p = 2;
    while p * p <= n {
        while n.is_multiple_of(p) {
            out.push(p);
            n /= p;
        }
        p += if p == 2 { 1 } else { 2 };
    }
    if n > 1 {
        out.push(n);
    }
    out
}

fn stages(n: usize, factors: &[usize]) -> Vec<Stage> {
    let mut l = 1;
    let mut out = Vec::with_capacity(factors.len());
    for &p in factors {
        let lp = l * p;
        let mut twiddles = Vec::with_capacity((p - 1) * l);
        for q in 1..p {
            for f in 0..l {
                let e = ((q * f) % lp) as f64;
                twiddles.push(Complex::cis(-2.0 * PI * e / lp as f64));
            }
        }
        let roots = if matches!(p, 2..=5) {
            Vec::new()
        } else {
            (0..p)
                .map(|k| Complex::cis(-2.0 * PI * k as f64 / p as f64))
                .collect()
        };
        out.push(Stage {
            p,
            l,
            twiddles,
            roots,
        });
        l = lp;
    }
    debug_assert_eq!(l, n.max(1));
    out
}

/// One radix-`p` pass from `x` to `y`. After it, `y[r·lp + f + l·s]` holds output `f + l·s`
/// of the length-`lp` transform of subsequence `r` (the samples `r + m·t`, `m = n / lp`).
fn pass(s: &Stage, n: usize, x: &[Complex], y: &mut [Complex]) {
    let (p, l) = (s.p, s.l);
    let lp = l * p;
    let m = n / lp;
    let stride = m * l;
    let tw = &s.twiddles;
    for r in 0..m {
        let src = r * l;
        let dst = r * lp;
        match p {
            2 => {
                for f in 0..l {
                    let a0 = x[src + f];
                    let a1 = x[src + stride + f] * tw[f];
                    y[dst + f] = a0 + a1;
                    y[dst + l + f] = a0 - a1;
                }
            }
            3 => {
                const H: f32 = 0.866_025_4; // √3 / 2
                for f in 0..l {
                    let a0 = x[src + f];
                    let a1 = x[src + stride + f] * tw[f];
                    let a2 = x[src + 2 * stride + f] * tw[l + f];
                    let sum = a1 + a2;
                    let d = (a1 - a2).mul_neg_i().scale(H);
                    let c = a0 - sum.scale(0.5);
                    y[dst + f] = a0 + sum;
                    y[dst + l + f] = c + d;
                    y[dst + 2 * l + f] = c - d;
                }
            }
            4 => {
                for f in 0..l {
                    let a0 = x[src + f];
                    let a1 = x[src + stride + f] * tw[f];
                    let a2 = x[src + 2 * stride + f] * tw[l + f];
                    let a3 = x[src + 3 * stride + f] * tw[2 * l + f];
                    let t0 = a0 + a2;
                    let t1 = a0 - a2;
                    let t2 = a1 + a3;
                    let t3 = (a1 - a3).mul_neg_i();
                    y[dst + f] = t0 + t2;
                    y[dst + l + f] = t1 + t3;
                    y[dst + 2 * l + f] = t0 - t2;
                    y[dst + 3 * l + f] = t1 - t3;
                }
            }
            5 => {
                let c1 = (2.0 * PI / 5.0).cos() as f32;
                let c2 = (4.0 * PI / 5.0).cos() as f32;
                let s1 = (2.0 * PI / 5.0).sin() as f32;
                let s2 = (4.0 * PI / 5.0).sin() as f32;
                for f in 0..l {
                    let a0 = x[src + f];
                    let a1 = x[src + stride + f] * tw[f];
                    let a2 = x[src + 2 * stride + f] * tw[l + f];
                    let a3 = x[src + 3 * stride + f] * tw[2 * l + f];
                    let a4 = x[src + 4 * stride + f] * tw[3 * l + f];
                    let b1 = a1 + a4;
                    let b2 = a2 + a3;
                    let d1 = a1 - a4;
                    let d2 = a2 - a3;
                    let t1 = a0 + b1.scale(c1) + b2.scale(c2);
                    let t2 = a0 + b1.scale(c2) + b2.scale(c1);
                    let u1 = (d1.scale(s1) + d2.scale(s2)).mul_neg_i();
                    let u2 = (d1.scale(s2) - d2.scale(s1)).mul_neg_i();
                    y[dst + f] = a0 + b1 + b2;
                    y[dst + l + f] = t1 + u1;
                    y[dst + 4 * l + f] = t1 - u1;
                    y[dst + 2 * l + f] = t2 + u2;
                    y[dst + 3 * l + f] = t2 - u2;
                }
            }
            _ => {
                let mut a = [Complex::ZERO; MAX_RADIX];
                for f in 0..l {
                    a[0] = x[src + f];
                    for q in 1..p {
                        a[q] = x[src + q * stride + f] * tw[(q - 1) * l + f];
                    }
                    for k in 0..p {
                        let mut acc = Complex::ZERO;
                        let mut e = 0;
                        for &v in &a[..p] {
                            acc = acc + v * s.roots[e];
                            e += k;
                            if e >= p {
                                e -= p;
                            }
                        }
                        y[dst + k * l + f] = acc;
                    }
                }
            }
        }
    }
}

/// Spatial frequency of index `i` of a centred spectrum of `n` samples spaced `step` apart
/// (cycles per unit of `step`): `(i − ⌊n/2⌋) / (n·step)`, numpy's `fftshift(fftfreq)`.
pub fn frequency(i: f64, n: usize, step: f64) -> f64 {
    (i - (n / 2) as f64) / (n as f64 * step)
}

/// Columns transformed together per task (contiguous gathers from the row-transformed
/// image).
const COLUMN_BLOCK: usize = 16;

/// The centred 2-D power spectrum `log₁₀ |F(u, v)|²` of a `width × height` plane (rows
/// top-down), the zero frequency at (`⌊width/2⌋`, `⌊height/2⌋`) like numpy's
/// `fftshift(fft2(·))`. NaN samples count as the mean of the others; exact zeros of the power
/// come out NaN. `None` when the buffer would not fit in memory.
pub fn power_spectrum(plane: &[f32], width: usize, height: usize) -> Option<Vec<f32>> {
    let n = width.checked_mul(height)?;
    if n == 0 || plane.len() < n {
        return None;
    }
    let plane = &plane[..n];
    let (sum, count) = plane
        .iter()
        .filter(|v| v.is_finite())
        .fold((0f64, 0usize), |(s, c), &v| (s + f64::from(v), c + 1));
    let fill = if count > 0 {
        (sum / count as f64) as f32
    } else {
        0.0
    };
    let value = |v: f32| if v.is_finite() { v } else { fill };

    let mut rows = Vec::new();
    rows.try_reserve_exact(n).ok()?;
    rows.resize(n, Complex::ZERO);
    let row_fft = Fft::new(width);
    // Two real rows as one complex row, then split by Hermitian symmetry.
    rows.par_chunks_mut(2 * width)
        .zip(plane.par_chunks(2 * width))
        .for_each_init(
            || vec![Complex::ZERO; row_fft.scratch_len() + width],
            |buf, (out, src)| {
                let (z, scratch) = buf.split_at_mut(width);
                let two = src.len() > width;
                for (x, v) in z.iter_mut().enumerate() {
                    let b = if two { value(src[width + x]) } else { 0.0 };
                    *v = Complex::new(value(src[x]), b);
                }
                row_fft.forward(z, scratch);
                for k in 0..width {
                    let zk = z[k];
                    let zn = z[(width - k) % width].conj();
                    out[k] = (zk + zn).scale(0.5);
                    if two {
                        out[width + k] = (zk - zn).mul_neg_i().scale(0.5);
                    }
                }
            },
        );

    let col_fft = Fft::new(height);
    let blocks: Vec<Vec<f32>> = (0..width.div_ceil(COLUMN_BLOCK))
        .into_par_iter()
        .map(|b| {
            let x0 = b * COLUMN_BLOCK;
            let bw = COLUMN_BLOCK.min(width - x0);
            let mut col = vec![Complex::ZERO; height];
            let mut scratch = vec![Complex::ZERO; col_fft.scratch_len()];
            let mut cols = vec![Complex::ZERO; bw * height];
            for y in 0..height {
                let row = &rows[y * width + x0..y * width + x0 + bw];
                for (c, &v) in row.iter().enumerate() {
                    cols[c * height + y] = v;
                }
            }
            // Power, already shifted vertically: row v of the block holds frequency row
            // (v + h - ⌊h/2⌋) mod h.
            let mut power = vec![0f32; bw * height];
            for c in 0..bw {
                col.copy_from_slice(&cols[c * height..(c + 1) * height]);
                col_fft.forward(&mut col, &mut scratch);
                for (k, v) in col.iter().enumerate() {
                    let y = (k + height / 2) % height;
                    let p = v.norm_sqr();
                    power[y * bw + c] = if p > 0.0 { p.log10() } else { f32::NAN };
                }
            }
            power
        })
        .collect();
    drop(rows);

    let mut out = Vec::new();
    out.try_reserve_exact(n).ok()?;
    out.resize(n, 0f32);
    for (b, power) in blocks.iter().enumerate() {
        let x0 = b * COLUMN_BLOCK;
        let bw = COLUMN_BLOCK.min(width - x0);
        for y in 0..height {
            for c in 0..bw {
                let x = (x0 + c + width / 2) % width;
                out[y * width + x] = power[y * bw + c];
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests;
