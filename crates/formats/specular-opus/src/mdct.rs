//! Inverse MDCT for CELT (RFC 6716 section 4.3.7): an N/4-point complex FFT between a pre- and
//! a post-rotation, then the low-overlap TDAC windowing that also overlap-adds the previous
//! block.

use std::f64::consts::PI;

#[derive(Clone, Copy, Default)]
struct C32 {
    re: f32,
    im: f32,
}

#[inline]
fn cmul(a: C32, b: C32) -> C32 {
    C32 {
        re: a.re * b.re - a.im * b.im,
        im: a.re * b.im + a.im * b.re,
    }
}

/// A mixed-radix (2, 3, 4, 5) forward complex FFT, unscaled.
struct Fft {
    n: usize,
    /// (radix, remaining length) per recursion level.
    factors: Vec<(usize, usize)>,
    /// exp(-2 pi i k / n), k in 0..n.
    tw: Vec<C32>,
}

impl Fft {
    fn new(n: usize) -> Fft {
        let mut factors = Vec::new();
        let mut m = n;
        while m > 1 {
            let p = if m.is_multiple_of(4) {
                4
            } else if m.is_multiple_of(2) {
                2
            } else if m.is_multiple_of(3) {
                3
            } else if m.is_multiple_of(5) {
                5
            } else {
                m
            };
            m /= p;
            factors.push((p, m));
        }
        let tw = (0..n)
            .map(|k| {
                let a = -2.0 * PI * k as f64 / n as f64;
                C32 {
                    re: a.cos() as f32,
                    im: a.sin() as f32,
                }
            })
            .collect();
        Fft { n, factors, tw }
    }

    fn run(&self, inp: &[C32], out: &mut [C32]) {
        self.work(out, inp, 0, 1, 0, 1);
    }

    /// Transform of length `out.len()` of inp[off + k*stride]; twiddles step `tws` in `tw`.
    fn work(
        &self,
        out: &mut [C32],
        inp: &[C32],
        off: usize,
        stride: usize,
        level: usize,
        tws: usize,
    ) {
        let (p, m) = self.factors[level];
        if m == 1 {
            for (q, o) in out.iter_mut().enumerate().take(p) {
                *o = inp[off + q * stride];
            }
        } else {
            for q in 0..p {
                self.work(
                    &mut out[q * m..(q + 1) * m],
                    inp,
                    off + q * stride,
                    stride * p,
                    level + 1,
                    tws * p,
                );
            }
        }
        // Butterflies: out[k + m*s] = sum_q W^(q*k) Y_q[k] W_p^(q*s).
        let tw = &self.tw;
        let n = self.n;
        match p {
            2 => {
                for k in 0..m {
                    let a = out[k];
                    let b = cmul(out[m + k], tw[k * tws]);
                    out[k] = C32 {
                        re: a.re + b.re,
                        im: a.im + b.im,
                    };
                    out[m + k] = C32 {
                        re: a.re - b.re,
                        im: a.im - b.im,
                    };
                }
            }
            4 => {
                for k in 0..m {
                    let a0 = out[k];
                    let a1 = cmul(out[m + k], tw[k * tws]);
                    let a2 = cmul(out[2 * m + k], tw[2 * k * tws]);
                    let a3 = cmul(out[3 * m + k], tw[3 * k * tws]);
                    let s02 = C32 {
                        re: a0.re + a2.re,
                        im: a0.im + a2.im,
                    };
                    let d02 = C32 {
                        re: a0.re - a2.re,
                        im: a0.im - a2.im,
                    };
                    let s13 = C32 {
                        re: a1.re + a3.re,
                        im: a1.im + a3.im,
                    };
                    let d13 = C32 {
                        re: a1.re - a3.re,
                        im: a1.im - a3.im,
                    };
                    out[k] = C32 {
                        re: s02.re + s13.re,
                        im: s02.im + s13.im,
                    };
                    out[2 * m + k] = C32 {
                        re: s02.re - s13.re,
                        im: s02.im - s13.im,
                    };
                    // -i * d13 for s = 1, +i * d13 for s = 3.
                    out[m + k] = C32 {
                        re: d02.re + d13.im,
                        im: d02.im - d13.re,
                    };
                    out[3 * m + k] = C32 {
                        re: d02.re - d13.im,
                        im: d02.im + d13.re,
                    };
                }
            }
            3 => {
                let w1 = tw[n / 3];
                for k in 0..m {
                    let a0 = out[k];
                    let a1 = cmul(out[m + k], tw[k * tws]);
                    let a2 = cmul(out[2 * m + k], tw[2 * k * tws]);
                    let s = C32 {
                        re: a1.re + a2.re,
                        im: a1.im + a2.im,
                    };
                    let d = C32 {
                        re: a1.re - a2.re,
                        im: a1.im - a2.im,
                    };
                    out[k] = C32 {
                        re: a0.re + s.re,
                        im: a0.im + s.im,
                    };
                    let t = C32 {
                        re: a0.re + w1.re * s.re,
                        im: a0.im + w1.re * s.im,
                    };
                    // w1.im * d rotated by i.
                    let r = C32 {
                        re: -w1.im * d.im,
                        im: w1.im * d.re,
                    };
                    out[m + k] = C32 {
                        re: t.re + r.re,
                        im: t.im + r.im,
                    };
                    out[2 * m + k] = C32 {
                        re: t.re - r.re,
                        im: t.im - r.im,
                    };
                }
            }
            5 => {
                let ya = tw[n / 5];
                let yb = tw[2 * n / 5];
                for k in 0..m {
                    let a0 = out[k];
                    let a1 = cmul(out[m + k], tw[k * tws]);
                    let a2 = cmul(out[2 * m + k], tw[2 * k * tws]);
                    let a3 = cmul(out[3 * m + k], tw[3 * k * tws]);
                    let a4 = cmul(out[4 * m + k], tw[4 * k * tws]);
                    let s14 = C32 {
                        re: a1.re + a4.re,
                        im: a1.im + a4.im,
                    };
                    let d14 = C32 {
                        re: a1.re - a4.re,
                        im: a1.im - a4.im,
                    };
                    let s23 = C32 {
                        re: a2.re + a3.re,
                        im: a2.im + a3.im,
                    };
                    let d23 = C32 {
                        re: a2.re - a3.re,
                        im: a2.im - a3.im,
                    };
                    out[k] = C32 {
                        re: a0.re + s14.re + s23.re,
                        im: a0.im + s14.im + s23.im,
                    };
                    let t1 = C32 {
                        re: a0.re + ya.re * s14.re + yb.re * s23.re,
                        im: a0.im + ya.re * s14.im + yb.re * s23.im,
                    };
                    let r1 = C32 {
                        re: -(ya.im * d14.im + yb.im * d23.im),
                        im: ya.im * d14.re + yb.im * d23.re,
                    };
                    let t2 = C32 {
                        re: a0.re + yb.re * s14.re + ya.re * s23.re,
                        im: a0.im + yb.re * s14.im + ya.re * s23.im,
                    };
                    let r2 = C32 {
                        re: -(yb.im * d14.im - ya.im * d23.im),
                        im: yb.im * d14.re - ya.im * d23.re,
                    };
                    out[m + k] = C32 {
                        re: t1.re + r1.re,
                        im: t1.im + r1.im,
                    };
                    out[4 * m + k] = C32 {
                        re: t1.re - r1.re,
                        im: t1.im - r1.im,
                    };
                    out[2 * m + k] = C32 {
                        re: t2.re + r2.re,
                        im: t2.im + r2.im,
                    };
                    out[3 * m + k] = C32 {
                        re: t2.re - r2.re,
                        im: t2.im - r2.im,
                    };
                }
            }
            _ => {
                // Generic DFT (never used by the CELT sizes).
                let mut tmp = vec![C32::default(); p];
                for k in 0..m {
                    for (s, t) in tmp.iter_mut().enumerate() {
                        let mut acc = C32::default();
                        for q in 0..p {
                            let w = tw[(q * (k + m * s) * tws) % n];
                            let v = cmul(out[q * m + k], w);
                            acc.re += v.re;
                            acc.im += v.im;
                        }
                        *t = acc;
                    }
                    for (s, t) in tmp.iter().enumerate() {
                        out[k + m * s] = *t;
                    }
                }
            }
        }
    }
}

/// One inverse-MDCT size.
struct Size {
    /// MDCT length (twice the number of coefficients).
    n: usize,
    fft: Fft,
    /// cos(2 pi (i + 1/8) / n), i in 0..n/2.
    trig: Vec<f32>,
}

/// Inverse MDCTs of 240 to 1920 points (shift 3..0) with the 120-sample CELT window.
pub(crate) struct Mdct {
    sizes: Vec<Size>,
    pub(crate) window: Vec<f32>,
    scratch_in: Vec<C32>,
    scratch_out: Vec<C32>,
}

pub(crate) const OVERLAP: usize = 120;

/// The 120-sample CELT overlap window.
pub(crate) fn window() -> Vec<f32> {
    (0..OVERLAP)
        .map(|i| {
            let s = (0.5 * PI * (i as f64 + 0.5) / OVERLAP as f64).sin();
            (0.5 * PI * s * s).sin() as f32
        })
        .collect()
}

impl Mdct {
    pub(crate) fn new() -> Mdct {
        let sizes = (0..4)
            .map(|shift| {
                let n = 1920 >> shift;
                let trig = (0..n / 2)
                    .map(|i| (2.0 * PI * (i as f64 + 0.125) / n as f64).cos() as f32)
                    .collect();
                Size {
                    n,
                    fft: Fft::new(n / 4),
                    trig,
                }
            })
            .collect();
        let window = window();
        Mdct {
            sizes,
            window,
            scratch_in: vec![C32::default(); 480],
            scratch_out: vec![C32::default(); 480],
        }
    }

    /// Inverse transform of the coefficients `inp[k * stride]` (k < n/2) into `out`: writes
    /// out[OVERLAP/2 .. OVERLAP/2 + n/2] and windows/overlap-adds out[0 .. OVERLAP] with the
    /// tail the previous block left there.
    pub(crate) fn backward(&mut self, inp: &[f32], stride: usize, out: &mut [f32], shift: usize) {
        let sz = &self.sizes[shift];
        let n = sz.n;
        let n2 = n / 2;
        let n4 = n / 4;
        let t = &sz.trig;
        let zin = &mut self.scratch_in[..n4];
        // Pre-rotation (real and imaginary swapped: a forward FFT does the inverse).
        for i in 0..n4 {
            let x1 = inp[2 * i * stride];
            let x2 = inp[(n2 - 1 - 2 * i) * stride];
            let yr = x2 * t[i] + x1 * t[n4 + i];
            let yi = x1 * t[i] - x2 * t[n4 + i];
            zin[i] = C32 { re: yi, im: yr };
        }
        let zout = &mut self.scratch_out[..n4];
        sz.fft.run(zin, zout);
        // Post-rotation into out[OVERLAP/2 ..].
        let base = &mut out[OVERLAP / 2..OVERLAP / 2 + n2];
        for k in 0..n4 {
            let re = zout[k].im;
            let im = zout[k].re;
            let yr = re * t[k] + im * t[n4 + k];
            let yi = re * t[n4 + k] - im * t[k];
            base[2 * k] = yr;
            base[n2 - 1 - 2 * k] = yi;
        }
        // TDAC mirror over the overlap.
        let w = &self.window;
        for i in 0..OVERLAP / 2 {
            let x1 = out[OVERLAP - 1 - i];
            let x2 = out[i];
            out[i] = w[OVERLAP - 1 - i] * x2 - w[i] * x1;
            out[OVERLAP - 1 - i] = w[i] * x2 + w[OVERLAP - 1 - i] * x1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_matches_dft() {
        for n in [60usize, 120, 240, 480] {
            let f = Fft::new(n);
            let inp: Vec<C32> = (0..n)
                .map(|i| C32 {
                    re: ((i * 7) % 13) as f32 - 6.0,
                    im: ((i * 3) % 5) as f32 - 2.0,
                })
                .collect();
            let mut out = vec![C32::default(); n];
            f.run(&inp, &mut out);
            for k in [0usize, 1, 7, n / 2, n - 1] {
                let (mut re, mut im) = (0f64, 0f64);
                for (j, x) in inp.iter().enumerate() {
                    let a = -2.0 * PI * (j * k % n) as f64 / n as f64;
                    re += x.re as f64 * a.cos() - x.im as f64 * a.sin();
                    im += x.re as f64 * a.sin() + x.im as f64 * a.cos();
                }
                assert!(
                    (out[k].re as f64 - re).abs() < 1e-2 && (out[k].im as f64 - im).abs() < 1e-2,
                    "n={n} k={k}"
                );
            }
        }
    }

    #[test]
    fn window_is_power_complementary() {
        let m = Mdct::new();
        for i in 0..OVERLAP {
            let a = m.window[i];
            let b = m.window[OVERLAP - 1 - i];
            assert!((a * a + b * b - 1.0).abs() < 1e-5);
        }
    }
}
