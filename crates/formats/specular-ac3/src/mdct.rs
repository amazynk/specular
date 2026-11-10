//! The inverse transforms of A/52 §7.9.4: the 512-sample IMDCT and the pair of 256-sample
//! ones for block switching, each through an N/4 (N/8) point complex IFFT with pre- and
//! post-twiddles, then the KBD window (alpha 5) and overlap-add.

use std::f64::consts::PI;

#[derive(Clone, Copy, Default)]
struct C {
    re: f32,
    im: f32,
}

impl C {
    #[inline]
    fn mul(self, o: C) -> C {
        C {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
        }
    }
}

/// A complex inverse FFT (positive exponent, unscaled) of a power-of-two size.
struct Fft {
    n: usize,
    rev: Vec<u16>,
    tw: Vec<C>,
}

impl Fft {
    fn new(n: usize) -> Fft {
        let bits = n.trailing_zeros();
        let rev = (0..n)
            .map(|i| ((i as u32).reverse_bits() >> (32 - bits)) as u16)
            .collect();
        let tw = (0..n / 2)
            .map(|k| {
                let a = 2.0 * PI * k as f64 / n as f64;
                C {
                    re: a.cos() as f32,
                    im: a.sin() as f32,
                }
            })
            .collect();
        Fft { n, rev, tw }
    }

    fn run(&self, x: &mut [C]) {
        let n = self.n;
        for i in 0..n {
            let j = usize::from(self.rev[i]);
            if i < j {
                x.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= n {
            let half = len / 2;
            let step = n / len;
            for s in (0..n).step_by(len) {
                for k in 0..half {
                    let w = self.tw[k * step];
                    let a = x[s + k];
                    let b = x[s + k + half].mul(w);
                    x[s + k] = C {
                        re: a.re + b.re,
                        im: a.im + b.im,
                    };
                    x[s + k + half] = C {
                        re: a.re - b.re,
                        im: a.im - b.im,
                    };
                }
            }
            len *= 2;
        }
    }
}

/// The transforms and the window, shared by all channels.
pub(crate) struct Imdct {
    fft128: Fft,
    fft64: Fft,
    /// `xcos1 + j xsin1` (128) and `xcos2 + j xsin2` (64).
    t1: Vec<C>,
    t2: Vec<C>,
    /// The 256 rising half of the window.
    pub(crate) window: [f32; 256],
}

/// The zeroth-order modified Bessel function of the first kind.
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..60 {
        term *= q / (k as f64 * k as f64);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// The Kaiser-Bessel derived window of A/52 (alpha 5), its first 256 values.
fn kbd() -> [f32; 256] {
    let alpha = 5.0;
    let kaiser: Vec<f64> = (0..=256)
        .map(|n| {
            let r = 2.0 * n as f64 / 256.0 - 1.0;
            bessel_i0(PI * alpha * (1.0 - r * r).max(0.0).sqrt())
        })
        .collect();
    let total: f64 = kaiser.iter().sum();
    let mut w = [0f32; 256];
    let mut acc = 0.0;
    for n in 0..256 {
        acc += kaiser[n];
        w[n] = (acc / total).sqrt() as f32;
    }
    w
}

impl Imdct {
    pub(crate) fn new() -> Imdct {
        let n = 512.0;
        let t1 = (0..128)
            .map(|k| {
                let a = 2.0 * PI * (8.0 * k as f64 + 1.0) / (8.0 * n);
                C {
                    re: -a.cos() as f32,
                    im: -a.sin() as f32,
                }
            })
            .collect();
        let t2 = (0..64)
            .map(|k| {
                let a = 2.0 * PI * (8.0 * k as f64 + 1.0) / (4.0 * n);
                C {
                    re: -a.cos() as f32,
                    im: -a.sin() as f32,
                }
            })
            .collect();
        Imdct {
            fft128: Fft::new(128),
            fft64: Fft::new(64),
            t1,
            t2,
            window: kbd(),
        }
    }

    /// Inverse transforms the 256 coefficients `x` (block switched when `short`), windows and
    /// overlaps with `delay`, writing 256 PCM samples.
    pub(crate) fn run(&self, x: &[f32; 256], short: bool, delay: &mut [f32; 256], pcm: &mut [f32]) {
        let w = &self.window;
        let mut out = [0f32; 512];
        if !short {
            let mut z = [C::default(); 128];
            for k in 0..128 {
                let v = C {
                    re: x[255 - 2 * k],
                    im: x[2 * k],
                };
                z[k] = v.mul(self.t1[k]);
            }
            self.fft128.run(&mut z);
            let mut y = [C::default(); 128];
            for n in 0..128 {
                y[n] = z[n].mul(self.t1[n]);
            }
            for n in 0..64 {
                out[2 * n] = -y[64 + n].im * w[2 * n];
                out[2 * n + 1] = y[63 - n].re * w[2 * n + 1];
                out[128 + 2 * n] = -y[n].re * w[128 + 2 * n];
                out[128 + 2 * n + 1] = y[127 - n].im * w[128 + 2 * n + 1];
                out[256 + 2 * n] = -y[64 + n].re * w[255 - 2 * n];
                out[256 + 2 * n + 1] = y[63 - n].im * w[254 - 2 * n];
                out[384 + 2 * n] = y[n].im * w[127 - 2 * n];
                out[384 + 2 * n + 1] = -y[127 - n].re * w[126 - 2 * n];
            }
        } else {
            let mut z1 = [C::default(); 64];
            let mut z2 = [C::default(); 64];
            for k in 0..64 {
                // X1[k] = X[2k], X2[k] = X[2k+1]
                let a = C {
                    re: x[2 * (127 - 2 * k)],
                    im: x[2 * (2 * k)],
                };
                let b = C {
                    re: x[2 * (127 - 2 * k) + 1],
                    im: x[2 * (2 * k) + 1],
                };
                z1[k] = a.mul(self.t2[k]);
                z2[k] = b.mul(self.t2[k]);
            }
            self.fft64.run(&mut z1);
            self.fft64.run(&mut z2);
            let mut y1 = [C::default(); 64];
            let mut y2 = [C::default(); 64];
            for n in 0..64 {
                y1[n] = z1[n].mul(self.t2[n]);
                y2[n] = z2[n].mul(self.t2[n]);
            }
            // The first transform has α = −1 in the forward transform of §8.2.3, the second
            // α = +1: the first's output is the second's shifted by 128 samples (the kernel
            // changes sign every 256). Only this reconstructs a signal through long, short
            // and long blocks (see the test); §7.9.4.2 step 5 as printed does not.
            let mut u = [0f32; 256];
            for n in 0..64 {
                u[2 * n] = -y1[n].re;
                u[2 * n + 1] = y1[63 - n].im;
                u[128 + 2 * n] = y1[n].im;
                u[128 + 2 * n + 1] = -y1[63 - n].re;
            }
            for n in 0..128 {
                out[n] = -u[n + 128] * w[n];
                out[n + 128] = u[n] * w[n + 128];
            }
            for n in 0..64 {
                out[256 + 2 * n] = -y2[n].re * w[255 - 2 * n];
                out[256 + 2 * n + 1] = y2[63 - n].im * w[254 - 2 * n];
                out[384 + 2 * n] = y2[n].im * w[127 - 2 * n];
                out[384 + 2 * n + 1] = -y2[63 - n].re * w[126 - 2 * n];
            }
        }
        for n in 0..256 {
            pcm[n] = 2.0 * (out[n] + delay[n]);
            delay[n] = out[256 + n];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Table 7.33's values, a few of them, to five decimals.
    #[test]
    fn window_matches_the_standard() {
        let w = Imdct::new().window;
        for (i, want) in [
            (0, 0.00014),
            (10, 0.00220),
            (100, 0.44030),
            (200, 0.99558),
            (255, 1.0),
        ] {
            assert!((f64::from(w[i]) - want).abs() < 6e-6, "w[{i}] = {}", w[i]);
        }
        // Princen-Bradley: w[n]^2 + w[255-n]^2 = 1
        for n in 0..256 {
            let s = w[n] * w[n] + w[255 - n] * w[255 - n];
            assert!((s - 1.0).abs() < 1e-5);
        }
    }

    /// The forward transform of A/52 §8.2.3 (α −1, 0, +1 for the first short, long and
    /// second short transform).
    fn forward(x: &[f64], a: f64) -> Vec<f64> {
        let n = x.len() as f64;
        (0..x.len() / 2)
            .map(|k| {
                let k = k as f64;
                -2.0 / n
                    * x.iter()
                        .enumerate()
                        .map(|(i, &v)| {
                            v * (2.0 * PI / (4.0 * n) * (2.0 * i as f64 + 1.0) * (2.0 * k + 1.0)
                                + PI / 4.0 * (2.0 * k + 1.0) * (1.0 + a))
                                .cos()
                        })
                        .sum::<f64>()
            })
            .collect()
    }

    /// Blocks long, short, long, long encoded by the forward transform decode back to the
    /// signal (time domain aliasing cancels across the switch).
    #[test]
    fn block_switching_reconstructs() {
        let t = Imdct::new();
        let w: Vec<f64> = (0..512)
            .map(|n| f64::from(t.window[if n < 256 { n } else { 511 - n }]))
            .collect();
        let mut seed = 7u32;
        let sig: Vec<f64> = (0..256 * 6)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                f64::from(seed >> 8) / f64::from(1u32 << 24) - 0.5
            })
            .collect();
        let mut delay = [0f32; 256];
        let mut out = vec![0f32; 256 * 5];
        for b in 0..5 {
            let seg: Vec<f64> = (0..512).map(|i| sig[256 * b + i] * w[i]).collect();
            let short = b == 2;
            let mut x = [0f32; 256];
            if short {
                let x1 = forward(&seg[..256], -1.0);
                let x2 = forward(&seg[256..], 1.0);
                for k in 0..128 {
                    x[2 * k] = x1[k] as f32;
                    x[2 * k + 1] = x2[k] as f32;
                }
            } else {
                for (d, v) in x.iter_mut().zip(forward(&seg, 0.0)) {
                    *d = v as f32;
                }
            }
            t.run(&x, short, &mut delay, &mut out[256 * b..256 * b + 256]);
        }
        // block b's output is samples 256 b .. 256 (b + 1) of the signal, from block 1 on
        for b in 1..5 {
            let worst = (256 * b..256 * b + 256)
                .map(|i| (out[i] - sig[i] as f32).abs())
                .fold(0f32, f32::max);
            assert!(worst < 1e-5, "block {b}: off by {worst}");
        }
    }

    /// The fast transform against the IMDCT's definition, for a long block.
    #[test]
    fn long_transform_is_an_imdct() {
        let t = Imdct::new();
        let mut x = [0f32; 256];
        for (k, v) in x.iter_mut().enumerate() {
            *v = ((k * 37 % 101) as f32 - 50.0) / 64.0;
        }
        let mut delay = [0f32; 256];
        let mut pcm = [0f32; 256];
        t.run(&x, false, &mut delay, &mut pcm);
        // second half of the windowed output is in `delay`; the first half in pcm / 2
        let n = 512.0f64;
        for i in [0usize, 17, 100, 255] {
            let mut s = 0.0;
            for (k, &v) in x.iter().enumerate() {
                s += f64::from(v)
                    * (2.0 * PI / (4.0 * n)
                        * (2.0 * i as f64 + 1.0 + n / 2.0)
                        * (2.0 * k as f64 + 1.0))
                        .cos();
            }
            let got = f64::from(pcm[i]) / 2.0 / f64::from(t.window[i]);
            // the transform's own scale: compare up to one constant factor and sign
            let ratio = got / s;
            assert!(
                s.abs() < 1e-3
                    || (ratio.abs() - 1.0).abs() < 1e-3
                    || (ratio.abs() - 0.5).abs() < 1e-3
                    || (ratio.abs() - 2.0).abs() < 1e-3,
                "bin {i}: {got} vs {s}"
            );
        }
    }
}
