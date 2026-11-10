//! The inverse MDCT, through a DCT-IV of n/2 points computed with an n/4-point complex FFT.

/// An inverse MDCT of one block size `n`.
pub(crate) struct Imdct {
    /// n/2: the number of coefficients.
    m: usize,
    /// Pre-twiddles exp(-i·π·(k + 1/4)/m) and post-twiddles exp(-i·π·k/m), k < n/4.
    pre: Vec<(f32, f32)>,
    post: Vec<(f32, f32)>,
    /// Bit reversal of the n/4 FFT indices.
    rev: Vec<u16>,
    /// Twiddles of every radix-2 stage, concatenated: for half-length h, exp(-2πi·j/(2h)).
    tw: Vec<(f32, f32)>,
    re: Vec<f32>,
    im: Vec<f32>,
}

impl Imdct {
    pub(crate) fn new(n: usize) -> Imdct {
        let m = n / 2;
        let q = n / 4;
        let pi = std::f64::consts::PI;
        let cis = |a: f64| (a.cos() as f32, a.sin() as f32);
        let pre = (0..q)
            .map(|k| cis(-pi * (k as f64 + 0.25) / m as f64))
            .collect();
        let post = (0..q).map(|k| cis(-pi * k as f64 / m as f64)).collect();
        let bits = q.trailing_zeros();
        let rev = (0..q)
            .map(|k| {
                if bits == 0 {
                    0
                } else {
                    ((k as u32).reverse_bits() >> (32 - bits)) as u16
                }
            })
            .collect();
        let mut tw = Vec::with_capacity(q);
        let mut h = 1;
        while h < q {
            for j in 0..h {
                tw.push(cis(-pi * j as f64 / h as f64));
            }
            h *= 2;
        }
        Imdct {
            m,
            pre,
            post,
            rev,
            tw,
            re: vec![0.0; q],
            im: vec![0.0; q],
        }
    }

    /// The DCT-IV of `x` (n/2 values) into `u`:
    /// u[j] = Σ x[k]·cos(π/m·(j + 1/2)·(k + 1/2)).
    /// The block's output is then y[i] = u[m/2 + i] (i < m/2), -u[3m/2 - 1 - i] (i < 3m/2),
    /// -u[i - 3m/2] (otherwise): y[i] = Σ x[k]·cos(2π/n·(i + 1/2 + n/4)·(k + 1/2)).
    pub(crate) fn dct4(&mut self, x: &[f32], u: &mut [f32]) {
        let m = self.m;
        let q = m / 2;
        let (x, u) = (&x[..m], &mut u[..m]);
        for k in 0..q {
            let (a, b) = (x[2 * k], x[m - 1 - 2 * k]);
            let (c, s) = self.pre[k];
            let r = usize::from(self.rev[k]);
            self.re[r] = a * c - b * s;
            self.im[r] = a * s + b * c;
        }
        self.fft();
        for k in 0..q {
            let (zr, zi) = (self.re[k], self.im[k]);
            let (c, s) = self.post[k];
            u[2 * k] = zr * c - zi * s;
            u[m - 1 - 2 * k] = -(zr * s + zi * c);
        }
    }

    /// In-place radix-2 decimation-in-time FFT of bit-reversed input.
    fn fft(&mut self) {
        let q = self.re.len();
        let (re, im) = (&mut self.re[..], &mut self.im[..]);
        // The first two stages as one radix-4 butterfly.
        if q >= 4 {
            for (r, i) in re.chunks_exact_mut(4).zip(im.chunks_exact_mut(4)) {
                let (ar, ai) = (r[0] + r[1], i[0] + i[1]);
                let (br, bi) = (r[0] - r[1], i[0] - i[1]);
                let (cr, ci) = (r[2] + r[3], i[2] + i[3]);
                // (r2 - r3) times -i.
                let (dr, di) = (i[2] - i[3], r[3] - r[2]);
                r[0] = ar + cr;
                i[0] = ai + ci;
                r[2] = ar - cr;
                i[2] = ai - ci;
                r[1] = br + dr;
                i[1] = bi + di;
                r[3] = br - dr;
                i[3] = bi - di;
            }
        } else if q == 2 {
            let (a, b) = (re[0], re[1]);
            re[0] = a + b;
            re[1] = a - b;
            let (a, b) = (im[0], im[1]);
            im[0] = a + b;
            im[1] = a - b;
            return;
        }
        let mut h = 4;
        let mut t = 3;
        while h < q {
            let tw = &self.tw[t..t + h];
            for (r, i) in re.chunks_exact_mut(2 * h).zip(im.chunks_exact_mut(2 * h)) {
                let (r0, r1) = r.split_at_mut(h);
                let (i0, i1) = i.split_at_mut(h);
                for ((((ar, ai), br), bi), &(c, s)) in r0
                    .iter_mut()
                    .zip(i0.iter_mut())
                    .zip(r1.iter_mut())
                    .zip(i1.iter_mut())
                    .zip(tw)
                {
                    let tr = *br * c - *bi * s;
                    let ti = *br * s + *bi * c;
                    *br = *ar - tr;
                    *bi = *ai - ti;
                    *ar += tr;
                    *ai += ti;
                }
            }
            t += h;
            h *= 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_direct_imdct() {
        for n in [8usize, 16, 64, 256, 2048] {
            let m = n / 2;
            let x: Vec<f32> = (0..m)
                .map(|k| ((k * 7919 % 113) as f32 / 56.0) - 1.0)
                .collect();
            let mut u = vec![0.0; m];
            let mut t = Imdct::new(n);
            t.dct4(&x, &mut u);
            let mut worst = 0f64;
            for i in 0..n {
                let y = if i < m / 2 {
                    u[m / 2 + i]
                } else if i < 3 * m / 2 {
                    -u[3 * m / 2 - 1 - i]
                } else {
                    -u[i - 3 * m / 2]
                };
                let want: f64 = (0..m)
                    .map(|k| {
                        f64::from(x[k])
                            * (2.0 * std::f64::consts::PI / n as f64
                                * (i as f64 + 0.5 + n as f64 / 4.0)
                                * (k as f64 + 0.5))
                                .cos()
                    })
                    .sum();
                worst = worst.max((f64::from(y) - want).abs());
            }
            assert!(worst < 1e-3 * (m as f64).sqrt(), "n {n}: {worst}");
        }
    }
}
