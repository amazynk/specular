//! Aligning two revisions of a sheet: a similarity transform (scale, rotation, shift) taking
//! the other sheet's pixels onto this one's.
//!
//! - From two point pairs the user picks ([`Similarity::from_pairs`]): exact.
//! - Automatically ([`auto`]): first a coarse estimate on 512 px copies (Fourier–Mellin: the
//!   rotation and scale from phase correlation of the log-polar magnitude spectra, then the
//!   shift from phase correlation of the sheets), then a refinement at full size: patches
//!   with structure in both directions are matched by phase correlation (sub-pixel) and the
//!   transform is fitted to the matches by least squares, three times. The coarse step is
//!   skipped when a transform is already known (the user's two pairs).
//!
//! Pixel (i, j) covers i..i+1 × j..j+1; values are grey, 255 paper.

use specular_fft::{Complex, Fft};

use crate::P;

/// A grey raster, rows top down, 255 white.
#[derive(Clone, Debug, PartialEq)]
pub struct Grey {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

impl Grey {
    pub fn new(w: usize, h: usize, px: Vec<u8>) -> Grey {
        assert_eq!(px.len(), w * h, "grey raster size");
        Grey { w, h, px }
    }

    /// Luma of RGBA rows (`stride` bytes apart), composited on white.
    pub fn from_rgba(w: usize, h: usize, rgba: &[u8], stride: usize) -> Grey {
        let mut px = Vec::with_capacity(w * h);
        for y in 0..h {
            let row = &rgba[y * stride..];
            for x in 0..w {
                let p = &row[x * 4..x * 4 + 4];
                let l = (u32::from(p[0]) * 54 + u32::from(p[1]) * 183 + u32::from(p[2]) * 19) >> 8;
                let a = u32::from(p[3]);
                px.push(((l * a + 255 * (255 - a)) / 255) as u8);
            }
        }
        Grey { w, h, px }
    }

    pub fn get(&self, x: usize, y: usize) -> u8 {
        self.px[y * self.w + x]
    }

    /// Bilinear value at a continuous point; paper (255) outside.
    pub fn sample(&self, x: f64, y: f64) -> f32 {
        let (u, v) = (x - 0.5, y - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = ((u - x0) as f32, (v - y0) as f32);
        let at = |i: f64, j: f64| -> f32 {
            if i < 0.0 || j < 0.0 || i >= self.w as f64 || j >= self.h as f64 {
                255.0
            } else {
                f32::from(self.px[j as usize * self.w + i as usize])
            }
        };
        let a = at(x0, y0) * (1.0 - fx) + at(x0 + 1.0, y0) * fx;
        let b = at(x0, y0 + 1.0) * (1.0 - fx) + at(x0 + 1.0, y0 + 1.0) * fx;
        a * (1.0 - fy) + b * fy
    }

    /// A copy whose long side is at most `max` (box filter), and its scale (copy px per px).
    pub fn reduce(&self, max: usize) -> (Grey, f64) {
        let long = self.w.max(self.h).max(1);
        if long <= max {
            return (self.clone(), 1.0);
        }
        let k = max as f64 / long as f64;
        let (w, h) = (
            ((self.w as f64 * k).round() as usize).max(1),
            ((self.h as f64 * k).round() as usize).max(1),
        );
        let (kx, ky) = (self.w as f64 / w as f64, self.h as f64 / h as f64);
        let mut px = Vec::with_capacity(w * h);
        for y in 0..h {
            let (sy0, sy1) = (
                (y as f64 * ky) as usize,
                (((y + 1) as f64 * ky) as usize).min(self.h),
            );
            for x in 0..w {
                let (sx0, sx1) = (
                    (x as f64 * kx) as usize,
                    (((x + 1) as f64 * kx) as usize).min(self.w),
                );
                let (mut s, mut n) = (0u32, 0u32);
                for j in sy0..sy1.max(sy0 + 1).min(self.h) {
                    for i in sx0..sx1.max(sx0 + 1).min(self.w) {
                        s += u32::from(self.px[j * self.w + i]);
                        n += 1;
                    }
                }
                px.push((s / n.max(1)) as u8);
            }
        }
        (Grey { w, h, px }, w as f64 / self.w as f64)
    }
}

/// `x' = a·x − b·y + tx`, `y' = b·x + a·y + ty` (a = s·cos θ, b = s·sin θ): the other sheet's
/// pixels onto this one's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Similarity {
    pub a: f64,
    pub b: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Similarity {
    pub const IDENTITY: Similarity = Similarity {
        a: 1.0,
        b: 0.0,
        tx: 0.0,
        ty: 0.0,
    };

    /// Scale `s`, rotation `theta` (radians; clockwise on screen, y being down), then shift.
    pub fn new(s: f64, theta: f64, tx: f64, ty: f64) -> Similarity {
        Similarity {
            a: s * theta.cos(),
            b: s * theta.sin(),
            tx,
            ty,
        }
    }

    pub fn apply(&self, p: P) -> P {
        P::new(
            self.a * p.x - self.b * p.y + self.tx,
            self.b * p.x + self.a * p.y + self.ty,
        )
    }

    pub fn scale(&self) -> f64 {
        self.a.hypot(self.b)
    }

    /// Radians.
    pub fn angle(&self) -> f64 {
        self.b.atan2(self.a)
    }

    pub fn inverse(&self) -> Similarity {
        let d = (self.a * self.a + self.b * self.b).max(1e-300);
        let (a, b) = (self.a / d, -self.b / d);
        Similarity {
            a,
            b,
            tx: -(a * self.tx - b * self.ty),
            ty: -(b * self.tx + a * self.ty),
        }
    }

    /// `self` after `o`: p ↦ self(o(p)).
    pub fn after(&self, o: &Similarity) -> Similarity {
        Similarity {
            a: self.a * o.a - self.b * o.b,
            b: self.b * o.a + self.a * o.b,
            tx: self.a * o.tx - self.b * o.ty + self.tx,
            ty: self.b * o.tx + self.a * o.ty + self.ty,
        }
    }

    /// The same map between copies of the sheets scaled by `ka` (this one) and `kb` (the
    /// other one).
    pub fn rescaled(&self, ka: f64, kb: f64) -> Similarity {
        let s = Similarity::new(ka, 0.0, 0.0, 0.0);
        let u = Similarity::new(1.0 / kb, 0.0, 0.0, 0.0);
        s.after(self).after(&u)
    }

    /// The map taking `b1` to `a1` and `b2` to `a2`.
    pub fn from_pairs(a1: P, a2: P, b1: P, b2: P) -> Option<Similarity> {
        Similarity::fit(&[(a1, b1, 1.0), (a2, b2, 1.0)])
    }

    /// The least-squares map taking each `b` to its `a`, weighted.
    pub fn fit(pairs: &[(P, P, f64)]) -> Option<Similarity> {
        let wsum: f64 = pairs.iter().map(|p| p.2).sum();
        if pairs.len() < 2 || wsum.is_nan() || wsum <= 0.0 {
            return None;
        }
        let mean =
            |f: &dyn Fn(&(P, P, f64)) -> f64| pairs.iter().map(|p| f(p) * p.2).sum::<f64>() / wsum;
        let (ax, ay) = (mean(&|p| p.0.x), mean(&|p| p.0.y));
        let (bx, by) = (mean(&|p| p.1.x), mean(&|p| p.1.y));
        let (mut num_re, mut num_im, mut den) = (0.0, 0.0, 0.0);
        for (pa, pb, w) in pairs {
            let (ux, uy) = (pa.x - ax, pa.y - ay);
            let (vx, vy) = (pb.x - bx, pb.y - by);
            // (u)·conj(v) as complex numbers.
            num_re += w * (ux * vx + uy * vy);
            num_im += w * (uy * vx - ux * vy);
            den += w * (vx * vx + vy * vy);
        }
        if den.is_nan() || den <= 1e-12 {
            return None;
        }
        let (a, b) = (num_re / den, num_im / den);
        let t = Similarity {
            a,
            b,
            tx: ax - (a * bx - b * by),
            ty: ay - (b * bx + a * by),
        };
        (t.a.is_finite() && t.b.is_finite() && t.scale() > 1e-6).then_some(t)
    }
}

/// A raster's ink (0 paper, 1 black) in an `n × n` canvas, top left, with its size.
fn ink_canvas(g: &Grey, n: usize) -> Vec<f32> {
    let mut out = vec![0f32; n * n];
    for y in 0..g.h.min(n) {
        for x in 0..g.w.min(n) {
            out[y * n + x] = 1.0 - f32::from(g.px[y * g.w + x]) / 255.0;
        }
    }
    out
}

/// In-place 2-D FFT of an `n × n` array.
fn fft2(data: &mut [Complex], n: usize, inverse: bool) {
    let f = Fft::new(n);
    let mut scratch = vec![Complex::ZERO; f.scratch_len()];
    let run = |row: &mut [Complex], s: &mut [Complex]| {
        if inverse {
            f.inverse(row, s);
        } else {
            f.forward(row, s);
        }
    };
    for row in data.chunks_mut(n) {
        run(row, &mut scratch);
    }
    let mut col = vec![Complex::ZERO; n];
    for x in 0..n {
        for y in 0..n {
            col[y] = data[y * n + x];
        }
        run(&mut col, &mut scratch);
        for y in 0..n {
            data[y * n + x] = col[y];
        }
    }
}

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = (i as f64 + 0.5) / n as f64;
            (0.5 - 0.5 * (std::f64::consts::TAU * t).cos()) as f32
        })
        .collect()
}

/// Phase correlation of two `n × n` arrays: the shift `d` with `b(x) ≈ a(x − d)` and the
/// peak's height (1 for identical content).
pub fn phase_correlate(a: &[f32], b: &[f32], n: usize, window: bool) -> (f64, f64, f64) {
    let win = window.then(|| hann(n));
    let load = |v: &[f32]| -> Vec<Complex> {
        let mean = v.iter().sum::<f32>() / v.len().max(1) as f32;
        v.iter()
            .enumerate()
            .map(|(i, &s)| {
                let w = win.as_ref().map_or(1.0, |w| w[i % n] * w[i / n]);
                Complex::new((s - if window { mean } else { 0.0 }) * w, 0.0)
            })
            .collect()
    };
    let (mut fa, mut fb) = (load(a), load(b));
    fft2(&mut fa, n, false);
    fft2(&mut fb, n, false);
    let mut r: Vec<Complex> = fb
        .iter()
        .zip(&fa)
        .map(|(&x, &y)| {
            let c = x * y.conj();
            let m = c.norm_sqr().sqrt();
            if m > 1e-12 {
                Complex::new(c.re / m, c.im / m)
            } else {
                Complex::ZERO
            }
        })
        .collect();
    fft2(&mut r, n, true);
    let nn = (n * n) as f32;
    let v: Vec<f32> = r.iter().map(|c| c.re / nn).collect();
    let (mut best, mut bi) = (f32::NEG_INFINITY, 0);
    for (i, &x) in v.iter().enumerate() {
        if x > best {
            best = x;
            bi = i;
        }
    }
    let (px, py) = (bi % n, bi / n);
    let at = |x: isize, y: isize| {
        v[(y.rem_euclid(n as isize) as usize) * n + x.rem_euclid(n as isize) as usize]
    };
    let sub = |m: f32, c: f32, p: f32| -> f64 {
        let d = m - 2.0 * c + p;
        if d.abs() < 1e-12 {
            0.0
        } else {
            (0.5 * (m - p) / d).clamp(-0.5, 0.5) as f64
        }
    };
    let (xi, yi) = (px as isize, py as isize);
    let dx = px as f64 + sub(at(xi - 1, yi), best, at(xi + 1, yi));
    let dy = py as f64 + sub(at(xi, yi - 1), best, at(xi, yi + 1));
    let wrap = |d: f64| if d > n as f64 / 2.0 { d - n as f64 } else { d };
    (wrap(dx), wrap(dy), f64::from(best))
}

/// Coarse alignment size.
const COARSE: usize = 512;
/// Log-polar resolution (angles over half a turn, and log radii).
const LOG_POLAR: usize = 256;

/// The centred, high-pass filtered magnitude spectrum of a canvas.
fn magnitude(canvas: &[f32], n: usize) -> Vec<f32> {
    let w = hann(n);
    let mut c: Vec<Complex> = canvas
        .iter()
        .enumerate()
        .map(|(i, &v)| Complex::new(v * w[i % n] * w[i / n], 0.0))
        .collect();
    fft2(&mut c, n, false);
    let h = n / 2;
    let mut out = vec![0f32; n * n];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = ((x + h) % n, (y + h) % n);
            let (u, v) = (
                (x as f64 - h as f64) / n as f64,
                (y as f64 - h as f64) / n as f64,
            );
            let k = (std::f64::consts::PI * u).cos() * (std::f64::consts::PI * v).cos();
            let hp = (1.0 - k) * (2.0 - k);
            out[y * n + x] = (c[fy * n + fx].norm_sqr().sqrt() as f64 * hp) as f32;
        }
    }
    out
}

fn log_polar(m: &[f32], n: usize) -> (Vec<f32>, f64) {
    let c = (n / 2) as f64;
    let r_max = c * 0.9;
    let r_min = 2.0f64;
    let base = (r_max / r_min).ln() / LOG_POLAR as f64;
    let mut out = vec![0f32; LOG_POLAR * LOG_POLAR];
    for j in 0..LOG_POLAR {
        let r = r_min * (base * j as f64).exp();
        for i in 0..LOG_POLAR {
            let t = std::f64::consts::PI * i as f64 / LOG_POLAR as f64;
            let (x, y) = (c + r * t.cos(), c + r * t.sin());
            let (x0, y0) = (x.floor(), y.floor());
            let (fx, fy) = ((x - x0) as f32, (y - y0) as f32);
            let at = |i: f64, j: f64| m[(j as usize).min(n - 1) * n + (i as usize).min(n - 1)];
            let v = (at(x0, y0) * (1.0 - fx) + at(x0 + 1.0, y0) * fx) * (1.0 - fy)
                + (at(x0, y0 + 1.0) * (1.0 - fx) + at(x0 + 1.0, y0 + 1.0) * fx) * fy;
            out[j * LOG_POLAR + i] = v.max(0.0).ln_1p();
        }
    }
    (out, base)
}

/// `b` warped into an `n × n` canvas through `t` (b's pixels onto the canvas), as ink.
fn warp_ink(b: &Grey, t: &Similarity, n: usize) -> Vec<f32> {
    let inv = t.inverse();
    let mut out = vec![0f32; n * n];
    for y in 0..n {
        for x in 0..n {
            let q = inv.apply(P::new(x as f64 + 0.5, y as f64 + 0.5));
            out[y * n + x] = 1.0 - b.sample(q.x, q.y) / 255.0;
        }
    }
    out
}

/// The coarse transform of `b` onto `a` (both at most [`COARSE`] px), and its peak height.
fn coarse(a: &Grey, b: &Grey) -> Option<(Similarity, f64)> {
    let n = COARSE;
    let (ca, cb) = (ink_canvas(a, n), ink_canvas(b, n));
    let (la, base) = log_polar(&magnitude(&ca, n), n);
    let (lb, _) = log_polar(&magnitude(&cb, n), n);
    let (dt, dr, _) = phase_correlate(&la, &lb, LOG_POLAR, true);
    let theta = std::f64::consts::PI * dt / LOG_POLAR as f64;
    let s = (dr * base).exp();
    let centre = |g: &Grey| P::new(g.w as f64 / 2.0, g.h as f64 / 2.0);
    let (pa, pb) = (centre(a), centre(b));
    let mut best: Option<(Similarity, f64)> = None;
    // The spectrum cannot tell a half turn, and the signs depend on conventions: try each
    // reading and keep the one whose shift correlates best.
    for th in [
        theta,
        -theta,
        theta + std::f64::consts::PI,
        -theta + std::f64::consts::PI,
    ] {
        for sc in [s, 1.0 / s] {
            if !(0.2..5.0).contains(&sc) {
                continue;
            }
            let l = Similarity::new(sc, th, 0.0, 0.0);
            // About the centres.
            let q = l.apply(pb);
            let t0 = Similarity {
                tx: pa.x - q.x,
                ty: pa.y - q.y,
                ..l
            };
            let w = warp_ink(b, &t0, n);
            let (dx, dy, peak) = phase_correlate(&ca, &w, n, false);
            // w(x) ≈ a(x − d): shift the warped sheet back by d.
            let t = Similarity {
                tx: t0.tx - dx,
                ty: t0.ty - dy,
                ..t0
            };
            if best.is_none_or(|(_, p)| peak > p) {
                best = Some((t, peak));
            }
        }
    }
    best
}

/// A transform of the other sheet onto this one, how well it fits and from how many matches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aligned {
    pub t: Similarity,
    /// Root-mean-square distance (px) of the matched patches from the transform; NaN when the
    /// refinement found too few.
    pub rms: f64,
    pub matches: usize,
}

/// Aligns `b` (the other sheet) onto `a`: from `start` when given, else from the coarse
/// estimate. `None` when nothing correlates.
pub fn auto(a: &Grey, b: &Grey, start: Option<Similarity>) -> Option<Aligned> {
    let t = match start {
        Some(t) => t,
        None => {
            let (sa, ka) = a.reduce(COARSE);
            let (sb, kb) = b.reduce(COARSE);
            let (t, _) = coarse(&sa, &sb)?;
            t.rescaled(1.0 / ka, 1.0 / kb)
        }
    };
    Some(refine(a, b, t))
}

/// Patch side for the refinement.
fn patch_side(a: &Grey) -> usize {
    if a.w.min(a.h) >= 1200 { 128 } else { 64 }
}

/// Smoothed ink of the `p × p` patch centred at `c`, sampled through `map` (patch pixel →
/// raster point).
fn patch(g: &Grey, p: usize, c: P, map: &dyn Fn(P) -> P) -> Vec<f32> {
    let mut v = vec![0f32; p * p];
    for y in 0..p {
        for x in 0..p {
            let q = map(P::new(
                c.x + x as f64 + 0.5 - p as f64 / 2.0,
                c.y + y as f64 + 0.5 - p as f64 / 2.0,
            ));
            v[y * p + x] = 1.0 - g.sample(q.x, q.y) / 255.0;
        }
    }
    // A binomial blur: sub-pixel peaks are steadier on thin lines.
    let mut t = v.clone();
    for y in 0..p {
        for x in 1..p - 1 {
            t[y * p + x] = 0.25 * v[y * p + x - 1] + 0.5 * v[y * p + x] + 0.25 * v[y * p + x + 1];
        }
    }
    for y in 1..p - 1 {
        for x in 0..p {
            v[y * p + x] =
                0.25 * t[(y - 1) * p + x] + 0.5 * t[y * p + x] + 0.25 * t[(y + 1) * p + x];
        }
    }
    v
}

/// Patch centres with structure in both directions (corners, crossings, text), best first
/// and at least a patch apart: the smaller of the summed squared x and y ink gradients over
/// each window, found on a reduced copy with integral images.
fn patch_centres(a: &Grey, p: usize) -> Vec<P> {
    const MAX: usize = 16;
    let (r, k) = a.reduce(COARSE);
    let (w, h) = (r.w, r.h);
    if w < 3 || h < 3 {
        return Vec::new();
    }
    // Integral images of the squared gradients.
    let mut ix = vec![0f64; (w + 1) * (h + 1)];
    let mut iy = vec![0f64; (w + 1) * (h + 1)];
    for y in 0..h {
        let (mut sx, mut sy) = (0f64, 0f64);
        for x in 0..w {
            let v = |x: usize, y: usize| f64::from(r.px[y * w + x]) / 255.0;
            let gx = if x > 0 { v(x, y) - v(x - 1, y) } else { 0.0 };
            let gy = if y > 0 { v(x, y) - v(x, y - 1) } else { 0.0 };
            sx += gx * gx;
            sy += gy * gy;
            ix[(y + 1) * (w + 1) + x + 1] = ix[y * (w + 1) + x + 1] + sx;
            iy[(y + 1) * (w + 1) + x + 1] = iy[y * (w + 1) + x + 1] + sy;
        }
    }
    let sum = |t: &[f64], x0: usize, y0: usize, x1: usize, y1: usize| {
        t[y1 * (w + 1) + x1] - t[y0 * (w + 1) + x1] - t[y1 * (w + 1) + x0] + t[y0 * (w + 1) + x0]
    };
    // Window and margin in reduced pixels.
    let win = ((p as f64 * k).round() as usize).clamp(4, w.min(h));
    let half = win / 2;
    let step = (win / 4).max(1);
    let mut cands: Vec<(f64, P)> = Vec::new();
    let mut y = half;
    while y + half <= h {
        let mut x = half;
        while x + half <= w {
            let (x0, y0, x1, y1) = (x - half, y - half, x - half + win, y - half + win);
            let sc = sum(&ix, x0, y0, x1, y1).min(sum(&iy, x0, y0, x1, y1));
            if sc > 0.0 {
                cands.push((sc, P::new(x as f64 / k, y as f64 / k)));
            }
            x += step;
        }
        y += step;
    }
    cands.sort_by(|x, y| y.0.total_cmp(&x.0));
    let top = cands.first().map_or(0.0, |c| c.0);
    let mut out: Vec<P> = Vec::new();
    for (sc, c) in cands {
        if out.len() >= MAX || sc < top * 0.02 {
            break;
        }
        if out.iter().all(|o| o.dist(c) >= p as f64) {
            out.push(c);
        }
    }
    out
}

fn refine(a: &Grey, b: &Grey, mut t: Similarity) -> Aligned {
    let p = patch_side(a);
    let centres = patch_centres(a, p);
    let id = |q: P| q;
    let fixed: Vec<(P, Vec<f32>)> = centres.iter().map(|&c| (c, patch(a, p, c, &id))).collect();
    let mut rms = f64::NAN;
    let mut matches = 0;
    for _ in 0..4 {
        let inv = t.inverse();
        let mut pairs: Vec<(P, P, f64)> = Vec::new();
        for (c, pa) in &fixed {
            let pw = patch(b, p, *c, &|q| inv.apply(q));
            let (dx, dy, peak) = phase_correlate(pa, &pw, p, true);
            if peak < 0.08 || dx.abs() > p as f64 / 4.0 || dy.abs() > p as f64 / 4.0 {
                continue;
            }
            // pw(x) ≈ pa(x − d): this sheet's point c is the other's inv(c + d).
            pairs.push((*c, inv.apply(P::new(c.x + dx, c.y + dy)), peak));
        }
        if pairs.len() < 3 {
            break;
        }
        let Some(mut fit) = Similarity::fit(&pairs) else {
            break;
        };
        // Drop matches far from the rest, then fit again.
        let res = |f: &Similarity, q: &(P, P, f64)| f.apply(q.1).dist(q.0);
        let mut r: Vec<f64> = pairs.iter().map(|q| res(&fit, q)).collect();
        r.sort_by(f64::total_cmp);
        let limit = (r[r.len() / 2] * 3.0).max(1.0);
        let kept: Vec<_> = pairs
            .iter()
            .copied()
            .filter(|q| res(&fit, q) <= limit)
            .collect();
        if kept.len() >= 3
            && let Some(f) = Similarity::fit(&kept)
        {
            fit = f;
        }
        rms = (kept.iter().map(|q| res(&fit, q).powi(2)).sum::<f64>() / kept.len().max(1) as f64)
            .sqrt();
        matches = kept.len();
        let moved = [P::new(0.0, 0.0), P::new(a.w as f64, a.h as f64)]
            .iter()
            .map(|&q| fit.apply(t.inverse().apply(q)).dist(q))
            .fold(0.0, f64::max);
        t = fit;
        if moved < 0.02 {
            break;
        }
    }
    Aligned { t, rms, matches }
}
