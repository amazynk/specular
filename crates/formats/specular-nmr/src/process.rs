//! Processing a FID: exponential window, zero fill, Fourier transform, the digital filter's
//! group delay and an automatic phase correction.
//!
//! Samples are `f64` (an `f32` transform loses the dynamic range of 32-bit integer FIDs:
//! small lines beside a solvent peak sink into its rounding noise). Spectra come out in
//! Bruker's order, the highest frequency (ppm) first like `1r`: `fftshift(fft(fid))` read
//! from bin 0 downwards.

use std::f64::consts::PI;

/// A complex sample.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct C64 {
    pub re: f64,
    pub im: f64,
}

impl C64 {
    pub const fn new(re: f64, im: f64) -> C64 {
        C64 { re, im }
    }

    fn mul(self, o: C64) -> C64 {
        C64::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }

    fn cis(t: f64) -> C64 {
        let (s, c) = t.sin_cos();
        C64::new(c, s)
    }

    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
}

/// In-place forward transform `X_k = Σ x_j e^{-2πi jk/n}`; `n` a power of two.
pub fn fft(data: &mut [C64]) {
    let n = data.len();
    if n < 2 {
        return;
    }
    debug_assert!(n.is_power_of_two());
    // Bit reversal.
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if j > i {
            data.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        // Twiddles computed directly per stage (no drift).
        let tw: Vec<C64> = (0..half)
            .map(|k| C64::cis(-2.0 * PI * k as f64 / len as f64))
            .collect();
        for start in (0..n).step_by(len) {
            for k in 0..half {
                let a = data[start + k];
                let b = data[start + k + half].mul(tw[k]);
                data[start + k] = C64::new(a.re + b.re, a.im + b.im);
                data[start + k + half] = C64::new(a.re - b.re, a.im - b.im);
            }
        }
        len *= 2;
    }
}

/// How a FID is processed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Spectrum size in complex points (a power of two, at least the FID's).
    pub size: usize,
    /// Exponential line broadening, Hz (0 for none).
    pub lb_hz: f64,
    /// Spectral width, Hz.
    pub sw_hz: f64,
    /// The digital filter's group delay, in dwell times (0 for none).
    pub group_delay: f64,
}

/// The FID windowed, zero filled, transformed into NMR order and corrected for the group
/// delay (not phased).
pub fn spectrum(fid: &[C64], s: &Settings) -> Vec<C64> {
    let n = s.size.max(fid.len()).next_power_of_two().max(2);
    let mut d = vec![C64::default(); n];
    let k = if s.sw_hz > 0.0 {
        -PI * s.lb_hz / s.sw_hz
    } else {
        0.0
    };
    for (j, (o, v)) in d.iter_mut().zip(fid).enumerate() {
        let w = (k * j as f64).exp();
        *o = C64::new(v.re * w, v.im * w);
    }
    fft(&mut d);
    d.rotate_left(n / 2);
    if s.group_delay != 0.0 {
        // A time shift of the FID is a linear phase across the spectrum (nmrglue's
        // `rm_dig_filter(post_proc=True)`).
        let step = 2.0 * PI * s.group_delay / n as f64;
        for (i, v) in d.iter_mut().enumerate() {
            *v = v.mul(C64::cis(step * i as f64));
        }
    }
    // Bruker's quadrature runs the other way: point i of the spectrum is bin −i of the
    // transform, so the highest ppm comes first (as in `1r`).
    let flipped: Vec<C64> = (0..n).map(|i| d[(n - i) % n]).collect();
    flipped
}

/// `spectrum[i] · e^{i(p0 + p1·i/n)}`, phases in degrees (nmrglue's `ps`).
pub fn phase(spectrum: &[C64], p0: f64, p1: f64) -> Vec<C64> {
    let n = spectrum.len().max(1) as f64;
    let (a, b) = (p0.to_radians(), p1.to_radians() / n);
    spectrum
        .iter()
        .enumerate()
        .map(|(i, v)| v.mul(C64::cis(a + b * i as f64)))
        .collect()
}

/// The real part after phasing, without allocating the complex result.
fn real_phased(spectrum: &[C64], p0: f64, p1: f64, out: &mut Vec<f64>) {
    out.clear();
    let n = spectrum.len().max(1) as f64;
    let rot = C64::cis(p1.to_radians() / n);
    let mut w = C64::cis(p0.to_radians());
    for (i, v) in spectrum.iter().enumerate() {
        if i % 1024 == 0 {
            // Renew the rotation now and then: the recurrence drifts slowly.
            w = C64::cis(p0.to_radians() + p1.to_radians() * i as f64 / n);
        }
        out.push(v.re * w.re - v.im * w.im);
        w = w.mul(rot);
    }
}

/// What a phase cost looks at: every point, or only those clearly above the noise.
struct Target {
    /// Points counted (`None`: all).
    mask: Option<Vec<usize>>,
    /// Their total power.
    power: f64,
}

impl Target {
    fn all(s: &[C64]) -> Target {
        Target {
            mask: None,
            power: s.iter().map(|v| v.re * v.re + v.im * v.im).sum(),
        }
    }

    /// Points whose magnitude is over 8 times the median magnitude (at least the 4
    /// largest): the lines, without the noise between them.
    fn lines(s: &[C64]) -> Target {
        let mut m: Vec<f64> = s.iter().map(|v| v.abs()).collect();
        let mid = m.len() / 2;
        let (_, &mut med, _) = m.select_nth_unstable_by(mid, f64::total_cmp);
        let mut mask: Vec<usize> = (0..s.len()).filter(|&i| s[i].abs() > 8.0 * med).collect();
        if mask.len() < 4 {
            let mut order: Vec<usize> = (0..s.len()).collect();
            order.sort_by(|&a, &b| s[b].abs().total_cmp(&s[a].abs()));
            order.truncate(4);
            order.sort_unstable();
            mask = order;
        }
        let power = mask
            .iter()
            .map(|&i| s[i].re * s[i].re + s[i].im * s[i].im)
            .sum();
        Target {
            mask: Some(mask),
            power,
        }
    }
}

/// The phase cost: the negative intensity left once the median (the baseline) is taken
/// off, over the power of the points looked at. An absorptive spectrum has none beyond its
/// noise; any dispersive part dips below the baseline. (ACME's derivative entropy, tried
/// first, settled on mixed phases with few lines.)
fn negative_cost(real: &[f64], target: &Target, sorted: &mut Vec<f64>) -> f64 {
    if real.is_empty() || target.power <= 0.0 {
        return 0.0;
    }
    sorted.clear();
    sorted.extend_from_slice(real);
    let mid = sorted.len() / 2;
    let (_, &mut med, _) = sorted.select_nth_unstable_by(mid, f64::total_cmp);
    let neg = |r: f64| {
        let d = r - med;
        if d < 0.0 { d * d } else { 0.0 }
    };
    let sum: f64 = match &target.mask {
        None => real.iter().map(|&r| neg(r)).sum(),
        Some(m) => m.iter().map(|&i| neg(real[i])).sum(),
    };
    sum / target.power
}

/// A spectrum reduced to at most `max` points by summing runs (phase is near constant
/// across a run, so the cost keeps its shape).
fn reduced(spectrum: &[C64], max: usize) -> Vec<C64> {
    let k = spectrum.len().div_ceil(max.max(1)).max(1);
    spectrum
        .chunks(k)
        .map(|c| {
            let (re, im) = c.iter().fold((0.0, 0.0), |a, v| (a.0 + v.re, a.1 + v.im));
            C64::new(re, im)
        })
        .collect()
}

/// Cost of a phase pair on `s`.
fn cost(s: &[C64], target: &Target, p: (f64, f64), buf: &mut (Vec<f64>, Vec<f64>)) -> f64 {
    real_phased(s, p.0, p.1, &mut buf.0);
    negative_cost(&buf.0, target, &mut buf.1)
}

/// The phases minimising the cost over `target`: a coarse grid on the reduced spectrum,
/// then Nelder–Mead on it and on the whole spectrum.
fn search(
    spectrum: &[C64],
    small: &[C64],
    (t, ts): (&Target, &Target),
    buf: &mut (Vec<f64>, Vec<f64>),
) -> (f64, f64) {
    let mut best = (f64::INFINITY, (0.0, 0.0));
    let mut p1 = -360.0;
    while p1 <= 360.0 {
        let mut p0 = -180.0;
        while p0 < 180.0 {
            let c = cost(small, ts, (p0, p1), buf);
            if c < best.0 {
                best = (c, (p0, p1));
            }
            p0 += 10.0;
        }
        p1 += 20.0;
    }
    let coarse = nelder_mead(|p| cost(small, ts, p, buf), best.1, 5.0);
    nelder_mead(|p| cost(spectrum, t, p, buf), coarse, 2.0)
}

/// Minimises `f` from `start` by Nelder–Mead (fixed steps and iteration count: the result
/// is the same on every machine).
fn nelder_mead(mut f: impl FnMut((f64, f64)) -> f64, start: (f64, f64), step: f64) -> (f64, f64) {
    let mut pts = [start, (start.0 + step, start.1), (start.0, start.1 + step)];
    let mut val = pts.map(&mut f);
    for _ in 0..200 {
        // Order best to worst.
        let mut idx = [0usize, 1, 2];
        idx.sort_by(|&a, &b| val[a].total_cmp(&val[b]));
        pts = idx.map(|i| pts[i]);
        val = idx.map(|i| val[i]);
        let size = (pts[1].0 - pts[0].0).abs()
            + (pts[1].1 - pts[0].1).abs()
            + (pts[2].0 - pts[0].0).abs()
            + (pts[2].1 - pts[0].1).abs();
        if size < 1e-3 {
            break;
        }
        let c = ((pts[0].0 + pts[1].0) / 2.0, (pts[0].1 + pts[1].1) / 2.0);
        let at = |t: f64| (c.0 + t * (pts[2].0 - c.0), c.1 + t * (pts[2].1 - c.1));
        let r = at(-1.0);
        let fr = f(r);
        if fr < val[0] {
            let e = at(-2.0);
            let fe = f(e);
            if fe < fr {
                pts[2] = e;
                val[2] = fe;
            } else {
                pts[2] = r;
                val[2] = fr;
            }
        } else if fr < val[1] {
            pts[2] = r;
            val[2] = fr;
        } else {
            let k = at(0.5);
            let fk = f(k);
            if fk < val[2] {
                pts[2] = k;
                val[2] = fk;
            } else {
                for i in 1..3 {
                    pts[i] = ((pts[i].0 + pts[0].0) / 2.0, (pts[i].1 + pts[0].1) / 2.0);
                    val[i] = f(pts[i]);
                }
            }
        }
    }
    let best = (0..3)
        .min_by(|&a, &b| val[a].total_cmp(&val[b]))
        .unwrap_or(0);
    pts[best]
}

/// Zero- and first-order phases (degrees, nmrglue's convention) that make `spectrum`
/// absorptive, by the least negative intensity below the baseline: once over every point,
/// once over the lines only (where noise would drown a weak spectrum's cost); the result
/// with the lower cost over the lines wins. The zero order comes back in (−180, 180].
pub fn autophase(spectrum: &[C64]) -> (f64, f64) {
    if spectrum.len() < 8 {
        return (0.0, 0.0);
    }
    let small = reduced(spectrum, 8192);
    let mut buf = (
        Vec::with_capacity(spectrum.len()),
        Vec::with_capacity(spectrum.len()),
    );
    let (all, all_small) = (Target::all(spectrum), Target::all(&small));
    let (lines, lines_small) = (Target::lines(spectrum), Target::lines(&small));
    let a = search(spectrum, &small, (&all, &all_small), &mut buf);
    let b = search(spectrum, &small, (&lines, &lines_small), &mut buf);
    let pick = if cost(spectrum, &lines, a, &mut buf) <= cost(spectrum, &lines, b, &mut buf) {
        a
    } else {
        b
    };
    (wrap(pick.0), pick.1)
}

/// An angle in (−180, 180].
pub fn wrap(a: f64) -> f64 {
    let r = a.rem_euclid(360.0);
    if r > 180.0 { r - 360.0 } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_matches_the_definition() {
        let x: Vec<C64> = (0..16)
            .map(|j| C64::new((j as f64 * 0.7).sin(), (j as f64 * 0.3).cos()))
            .collect();
        let mut y = x.clone();
        fft(&mut y);
        for (k, yk) in y.iter().enumerate() {
            let mut s = C64::default();
            for (j, xj) in x.iter().enumerate() {
                let t = xj.mul(C64::cis(-2.0 * PI * (j * k) as f64 / 16.0));
                s = C64::new(s.re + t.re, s.im + t.im);
            }
            assert!((s.re - yk.re).abs() < 1e-12 && (s.im - yk.im).abs() < 1e-12);
        }
    }

    /// A FID of Lorentzian lines at `freqs` (cycles per point), dephased.
    fn fid(n: usize, freqs: &[f64], p0: f64) -> Vec<C64> {
        (0..n)
            .map(|j| {
                let mut v = C64::default();
                for &f in freqs {
                    let t = C64::cis(2.0 * PI * f * j as f64 + p0.to_radians());
                    let d = (-(j as f64) / 300.0).exp();
                    v = C64::new(v.re + t.re * d, v.im + t.im * d);
                }
                v
            })
            .collect()
    }

    #[test]
    fn autophase_makes_lines_absorptive() {
        let f = fid(2048, &[0.11, -0.2, 0.31], 63.0);
        let s = spectrum(
            &f,
            &Settings {
                size: 4096,
                lb_hz: 0.0,
                sw_hz: 1.0,
                group_delay: 0.0,
            },
        );
        let (p0, p1) = autophase(&s);
        let r: Vec<f64> = phase(&s, p0, p1).iter().map(|v| v.re).collect();
        let neg: f64 = r.iter().filter(|v| **v < 0.0).map(|v| v * v).sum();
        let all: f64 = r.iter().map(|v| v * v).sum();
        assert!(neg / all < 1e-3, "{p0} {p1} {}", neg / all);
        assert!((wrap(p0 + 63.0)).abs() < 2.0, "{p0}");
        assert!(p1.abs() < 5.0, "{p1}");
    }
}
