//! Alignment of bracket frames by enhanced correlation coefficient maximisation (ECC,
//! Evangelidis & Psarakis, "Parametric image alignment using enhanced correlation coefficient
//! maximization", IEEE TPAMI 2008), coarse to fine on a luma proxy.
//!
//! - **Proxy**: each CFA block (2×2 Bayer, 3×3 X-Trans, one pixel for linear data) becomes
//!   one value, the log2 of its mean. In log units a frame's exposure is an offset, which the
//!   correlation coefficient ignores. Blocks with a saturated or nearly black sample are left
//!   out (NaN), so the clipped highlights of the long frames and the noise of the short ones
//!   do not pull.
//! - **ECC**: the forward-additive iteration of the paper, with every sum taken over the
//!   pixels valid in both images in one parallel pass (zero-mean forms from the plain sums),
//!   for translation, rotation plus translation, similarity, affine or homography (the
//!   paper's warps; each model's Jacobian in [`Motion`]), parametrised in coordinates
//!   normalised to the level's centre and half long side, so the parameters carry from one
//!   pyramid level to the next unchanged.
//! - **Pyramid**: 2× block means down to about 64 pixels on the short side; each level starts
//!   from the coarser result.

// Small k×k systems read clearest with indices.
#![allow(clippy::needless_range_loop)]

use rayon::prelude::*;

use super::homography::{Homography, cholesky_solve};

/// What alignment may move.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    /// The frames are already aligned (a tripod).
    None,
    Translation,
    /// Rotation and translation.
    #[default]
    Euclidean,
    /// Scale, rotation and translation (focus breathing).
    Similarity,
    Affine,
    /// A plane seen from another place (hand-held brackets).
    Homography,
}

/// Maps reference positions to a frame's: `p' = R(angle)(p − c) + c + t`, in sample units
/// of the merged area (proxy units inside the pyramid).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Radians, counter-clockwise in a y-down image.
    pub angle: f64,
    pub tx: f64,
    pub ty: f64,
    pub cx: f64,
    pub cy: f64,
}

impl Transform {
    /// As a homography.
    pub fn homography(&self) -> Homography {
        Homography::similarity(1.0, self.angle, self.tx, self.ty, self.cx, self.cy)
    }

    pub fn identity(cx: f64, cy: f64) -> Transform {
        Transform {
            angle: 0.0,
            tx: 0.0,
            ty: 0.0,
            cx,
            cy,
        }
    }

    #[inline]
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let (s, c) = self.angle.sin_cos();
        let (dx, dy) = (x - self.cx, y - self.cy);
        (
            c * dx - s * dy + self.cx + self.tx,
            s * dx + c * dy + self.cy + self.ty,
        )
    }

    /// Moves no sample by more than 0.001 of a pixel over a `w × h` area.
    pub fn is_identity(&self, w: u32, h: u32) -> bool {
        let r = (w.max(h) as f64) * 0.75;
        self.tx.abs() < 1e-3 && self.ty.abs() < 1e-3 && self.angle.abs() * r < 1e-3
    }

    /// The largest distance a corner of a `w × h` area moves.
    pub fn max_shift(&self, w: u32, h: u32) -> f64 {
        let (w, h) = (w as f64 - 1.0, h as f64 - 1.0);
        [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
            .iter()
            .map(|&(x, y)| {
                let (a, b) = self.apply(x, y);
                ((a - x).powi(2) + (b - y).powi(2)).sqrt()
            })
            .fold(0.0, f64::max)
    }
}

/// A luma proxy: `w × h` values, NaN where not usable.
#[derive(Clone, Debug)]
pub struct Proxy {
    pub w: usize,
    pub h: usize,
    pub v: Vec<f32>,
}

impl Proxy {
    #[inline]
    fn at(&self, x: usize, y: usize) -> f32 {
        self.v[y * self.w + x]
    }

    /// Bilinear sample at (x, y); NaN outside or next to an unusable value.
    #[inline]
    pub(crate) fn sample(&self, x: f64, y: f64) -> f32 {
        if !(x >= 0.0 && y >= 0.0) {
            return f32::NAN;
        }
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        if x0 + 1 >= self.w || y0 + 1 >= self.h {
            return f32::NAN;
        }
        let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
        let a = self.at(x0, y0);
        let b = self.at(x0 + 1, y0);
        let c = self.at(x0, y0 + 1);
        let d = self.at(x0 + 1, y0 + 1);
        let top = a + (b - a) * fx;
        let bot = c + (d - c) * fx;
        top + (bot - top) * fy
    }

    /// Half size: the mean of each 2×2 block's usable values (NaN when fewer than two).
    pub(crate) fn half(&self) -> Proxy {
        let (w, h) = (self.w / 2, self.h / 2);
        let mut v = vec![f32::NAN; w * h];
        v.par_chunks_mut(w.max(1)).enumerate().for_each(|(y, row)| {
            for (x, o) in row.iter_mut().enumerate() {
                let mut s = 0.0;
                let mut n = 0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let q = self.at(2 * x + dx, 2 * y + dy);
                    if q.is_finite() {
                        s += q;
                        n += 1;
                    }
                }
                if n >= 2 {
                    *o = s / n as f32;
                }
            }
        });
        Proxy { w, h, v }
    }

    /// Central-difference gradients (NaN where a neighbour is unusable).
    fn gradients(&self) -> (Proxy, Proxy) {
        let (w, h) = (self.w, self.h);
        let mut gx = vec![f32::NAN; w * h];
        let mut gy = vec![f32::NAN; w * h];
        gx.par_chunks_mut(w)
            .zip(gy.par_chunks_mut(w))
            .enumerate()
            .for_each(|(y, (rx, ry))| {
                if y == 0 || y + 1 >= h {
                    return;
                }
                for x in 1..w.saturating_sub(1) {
                    rx[x] = 0.5 * (self.at(x + 1, y) - self.at(x - 1, y));
                    ry[x] = 0.5 * (self.at(x, y + 1) - self.at(x, y - 1));
                }
            });
        (Proxy { w, h, v: gx }, Proxy { w, h, v: gy })
    }
}

/// The proxy of a normalised plane (see the module documentation). `block` is the CFA block
/// side (1 for linear data); `dark` the normalised level below which a block is noise.
pub(crate) fn proxy(plane: &super::Plane, block: u32, dark: f32) -> Proxy {
    let b = block.max(1) as usize;
    let (pw, ph, cpp) = (plane.w as usize, plane.h as usize, plane.cpp as usize);
    let (w, h) = (pw / b, ph / b);
    let mut v = vec![f32::NAN; w * h];
    v.par_chunks_mut(w.max(1)).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let mut s = 0.0f32;
            let mut ok = true;
            for dy in 0..b {
                let line = &plane.v[((y * b + dy) * pw + x * b) * cpp..][..b * cpp];
                for &q in line {
                    if !q.is_finite() {
                        ok = false;
                    }
                    s += q;
                }
            }
            let m = s / (b * b * cpp) as f32;
            if ok && m > dark {
                *o = m.log2();
            }
        }
    });
    Proxy { w, h, v }
}

/// The most parameters a motion model has (a homography).
const K: usize = 8;

/// Sums over the pixels valid in both images: n, Σt, Σi, Σt², Σi², Σti, Σg, Σgt, Σgi, Σggᵀ.
#[derive(Clone, Copy)]
struct Sums {
    n: f64,
    t: f64,
    i: f64,
    tt: f64,
    ii: f64,
    ti: f64,
    g: [f64; K],
    gt: [f64; K],
    gi: [f64; K],
    gg: [[f64; K]; K],
}

impl Default for Sums {
    fn default() -> Self {
        Sums {
            n: 0.0,
            t: 0.0,
            i: 0.0,
            tt: 0.0,
            ii: 0.0,
            ti: 0.0,
            g: [0.0; K],
            gt: [0.0; K],
            gi: [0.0; K],
            gg: [[0.0; K]; K],
        }
    }
}

impl Sums {
    fn add(mut self, o: Sums) -> Sums {
        self.n += o.n;
        self.t += o.t;
        self.i += o.i;
        self.tt += o.tt;
        self.ii += o.ii;
        self.ti += o.ti;
        for a in 0..K {
            self.g[a] += o.g[a];
            self.gt[a] += o.gt[a];
            self.gi[a] += o.gi[a];
            for b in 0..K {
                self.gg[a][b] += o.gg[a][b];
            }
        }
        self
    }
}

/// A level's parameters, in normalised coordinates `u = (x − c)/r` (`r` half the long side),
/// which do not change from one pyramid level to the next.
type Params = [f64; K];

impl Motion {
    /// Parameters of the model.
    pub fn params(self) -> usize {
        match self {
            Motion::None => 0,
            Motion::Translation => 2,
            Motion::Euclidean => 3,
            Motion::Similarity => 4,
            Motion::Affine => 6,
            Motion::Homography => 8,
        }
    }

    /// The model's map in normalised coordinates.
    fn matrix(self, p: &Params) -> Homography {
        Homography(match self {
            Motion::None => [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            Motion::Translation => [1.0, 0.0, p[0], 0.0, 1.0, p[1], 0.0, 0.0, 1.0],
            Motion::Euclidean => {
                let (s, c) = p[0].sin_cos();
                [c, -s, p[1], s, c, p[2], 0.0, 0.0, 1.0]
            }
            Motion::Similarity => [
                1.0 + p[0],
                -p[1],
                p[2],
                p[1],
                1.0 + p[0],
                p[3],
                0.0,
                0.0,
                1.0,
            ],
            Motion::Affine => [
                1.0 + p[0],
                p[1],
                p[2],
                p[3],
                1.0 + p[4],
                p[5],
                0.0,
                0.0,
                1.0,
            ],
            Motion::Homography => [
                1.0 + p[0],
                p[1],
                p[2],
                p[3],
                1.0 + p[4],
                p[5],
                p[6],
                p[7],
                1.0,
            ],
        })
    }

    /// The parameters nearest to normalised map `m`.
    fn params_of(self, m: &Homography) -> Params {
        let m = m.normalised().0;
        let mut p = [0.0; K];
        match self {
            Motion::None => {}
            Motion::Translation => p[..2].copy_from_slice(&[m[2], m[5]]),
            Motion::Euclidean => p[..3].copy_from_slice(&[m[3].atan2(m[0]), m[2], m[5]]),
            Motion::Similarity => p[..4].copy_from_slice(&[
                (m[0] + m[4]) / 2.0 - 1.0,
                (m[3] - m[1]) / 2.0,
                m[2],
                m[5],
            ]),
            Motion::Affine => {
                p[..6].copy_from_slice(&[m[0] - 1.0, m[1], m[2], m[3], m[4] - 1.0, m[5]])
            }
            Motion::Homography => {
                p.copy_from_slice(&[m[0] - 1.0, m[1], m[2], m[3], m[4] - 1.0, m[5], m[6], m[7]])
            }
        }
        p
    }

    /// ∂q/∂p at normalised position (u, v), where `q = m(u, v)` is already known: rows x, y.
    #[inline]
    fn jacobian(self, p: &Params, u: f64, v: f64, q: (f64, f64)) -> [[f64; K]; 2] {
        let mut j = [[0.0; K]; 2];
        match self {
            Motion::None => {}
            Motion::Translation => {
                j[0][0] = 1.0;
                j[1][1] = 1.0;
            }
            Motion::Euclidean => {
                let (s, c) = p[0].sin_cos();
                j[0][0] = -s * u - c * v;
                j[1][0] = c * u - s * v;
                j[0][1] = 1.0;
                j[1][2] = 1.0;
            }
            Motion::Similarity => {
                j[0][0] = u;
                j[1][0] = v;
                j[0][1] = -v;
                j[1][1] = u;
                j[0][2] = 1.0;
                j[1][3] = 1.0;
            }
            Motion::Affine => {
                j[0][..3].copy_from_slice(&[u, v, 1.0]);
                j[1][3..6].copy_from_slice(&[u, v, 1.0]);
            }
            Motion::Homography => {
                let w = p[6] * u + p[7] * v + 1.0;
                let k = 1.0 / w;
                j[0][..3].copy_from_slice(&[u * k, v * k, k]);
                j[1][3..6].copy_from_slice(&[u * k, v * k, k]);
                j[0][6] = -u * q.0 * k;
                j[0][7] = -v * q.0 * k;
                j[1][6] = -u * q.1 * k;
                j[1][7] = -v * q.1 * k;
            }
        }
        j
    }
}

/// The normalisation of a `w × h` level: pixel = c + r·u.
#[derive(Clone, Copy, Debug)]
struct Frame {
    cx: f64,
    cy: f64,
    r: f64,
}

impl Frame {
    fn to_pixels(self) -> Homography {
        Homography([self.r, 0.0, self.cx, 0.0, self.r, self.cy, 0.0, 0.0, 1.0])
    }
    fn pixels_to_units(self) -> Homography {
        let k = 1.0 / self.r;
        Homography([k, 0.0, -self.cx * k, 0.0, k, -self.cy * k, 0.0, 0.0, 1.0])
    }
    /// A normalised map in pixels of this level.
    fn pixel_map(self, m: &Homography) -> Homography {
        self.to_pixels()
            .then_after(m)
            .then_after(&self.pixels_to_units())
    }
    fn normalised_map(self, h: &Homography) -> Homography {
        self.pixels_to_units()
            .then_after(h)
            .then_after(&self.to_pixels())
    }
}

fn gather(
    t: &Proxy,
    i: &Proxy,
    gx: &Proxy,
    gy: &Proxy,
    motion: Motion,
    p: &Params,
    f: Frame,
) -> Sums {
    let k = motion.params();
    let m = motion.matrix(p);
    let px = f.pixel_map(&m);
    (0..t.h)
        .into_par_iter()
        .map(|y| {
            let mut acc = Sums::default();
            for x in 0..t.w {
                let tv = t.at(x, y);
                if !tv.is_finite() {
                    continue;
                }
                let (qx, qy) = px.apply(x as f64, y as f64);
                let iv = i.sample(qx, qy);
                let ix = gx.sample(qx, qy);
                let iy = gy.sample(qx, qy);
                if !(iv.is_finite() && ix.is_finite() && iy.is_finite()) {
                    continue;
                }
                let (u, v) = ((x as f64 - f.cx) / f.r, (y as f64 - f.cy) / f.r);
                let q = ((qx - f.cx) / f.r, (qy - f.cy) / f.r);
                let j = motion.jacobian(p, u, v, q);
                // The image gradient per normalised unit.
                let (ix, iy) = (f64::from(ix) * f.r, f64::from(iy) * f.r);
                let mut g = [0.0; K];
                for a in 0..k {
                    g[a] = ix * j[0][a] + iy * j[1][a];
                }
                let (tv, iv) = (f64::from(tv), f64::from(iv));
                acc.n += 1.0;
                acc.t += tv;
                acc.i += iv;
                acc.tt += tv * tv;
                acc.ii += iv * iv;
                acc.ti += tv * iv;
                for a in 0..k {
                    acc.g[a] += g[a];
                    acc.gt[a] += g[a] * tv;
                    acc.gi[a] += g[a] * iv;
                    for b in 0..=a {
                        acc.gg[a][b] += g[a] * g[b];
                    }
                }
            }
            acc
        })
        .reduce(Sums::default, Sums::add)
}

/// Solves `h x = b` for a symmetric positive definite `k × k` system (k ≤ 8).
fn solve(h: &[[f64; K]; K], b: &[f64; K], k: usize) -> Option<[f64; K]> {
    let a: Vec<f64> = (0..k * k).map(|i| h[i / k][i % k]).collect();
    let x = cholesky_solve(&a, &b[..k], k)?;
    let mut out = [0.0; K];
    out[..k].copy_from_slice(&x);
    Some(out)
}

fn dot(a: &[f64; K], b: &[f64; K], k: usize) -> f64 {
    (0..k).map(|i| a[i] * b[i]).sum()
}

/// One level's ECC iterations from `p`; the parameters and the correlation coefficient.
fn ecc_level(
    t: &Proxy,
    i: &Proxy,
    f: Frame,
    motion: Motion,
    mut p: Params,
    iterations: usize,
) -> (Params, f64) {
    let k = motion.params();
    let (gx, gy) = i.gradients();
    let mut rho = 0.0;
    for _ in 0..iterations {
        let s = gather(t, i, &gx, &gy, motion, &p, f);
        // Too few pixels to fix the model.
        if s.n < (8 * k).max(16) as f64 {
            break;
        }
        let n = s.n;
        // Zero-mean forms.
        let tt = s.tt - s.t * s.t / n;
        let ii = s.ii - s.i * s.i / n;
        let ti = s.ti - s.t * s.i / n;
        let mut h = [[0.0; K]; K];
        let mut gt = [0.0; K];
        let mut gi = [0.0; K];
        for a in 0..k {
            gt[a] = s.gt[a] - s.g[a] * s.t / n;
            gi[a] = s.gi[a] - s.g[a] * s.i / n;
            for b in 0..=a {
                h[a][b] = s.gg[a][b] - s.g[a] * s.g[b] / n;
                h[b][a] = h[a][b];
            }
        }
        if tt <= 0.0 || ii <= 0.0 {
            break;
        }
        rho = ti / (tt * ii).sqrt();
        let (Some(hgi), Some(hgt)) = (solve(&h, &gi, k), solve(&h, &gt, k)) else {
            break;
        };
        let gi_h_gi = dot(&gi, &hgi, k);
        let gt_h_gi = dot(&gt, &hgi, k);
        let gt_h_gt = dot(&gt, &hgt, k);
        let num = ii - gi_h_gi;
        let den = ti - gt_h_gi;
        let lambda = if den > 0.0 {
            num / den
        } else {
            // The paper's case for a non-positive denominator.
            let l1 = (gi_h_gi / gt_h_gt.max(1e-30)).sqrt();
            let l2 = (gt_h_gi - ti) / gt_h_gt.max(1e-30);
            l1.max(l2)
        };
        // Δp = H⁻¹ Gᵀ(λ t − i).
        let rhs: [f64; K] = std::array::from_fn(|a| lambda * gt[a] - gi[a]);
        let Some(d) = solve(&h, &rhs, k) else { break };
        let before = motion.matrix(&p);
        for a in 0..k {
            p[a] += d[a];
        }
        let after = motion.matrix(&p);
        if !after.is_finite() {
            break;
        }
        // How far the step moves the level's corners, in pixels.
        let step = [
            (-1.0, -1.0),
            (1.0, -1.0),
            (-1.0, 1.0),
            (1.0, 1.0),
            (0.0, 0.0),
        ]
        .iter()
        .map(|&(u, v)| {
            let (a, b) = (before.apply(u, v), after.apply(u, v));
            (a.0 - b.0).hypot(a.1 - b.1)
        })
        .fold(0.0, f64::max)
            * f.r;
        if !step.is_finite() || step < 1e-4 {
            break;
        }
    }
    (p, rho)
}

/// What [`ecc`] found.
#[derive(Clone, Copy, Debug)]
pub struct Found {
    /// Rotation and translation (the nearest for richer models), in the proxies' units.
    pub transform: Transform,
    /// The whole map, in the proxies' units (reference → frame).
    pub h: Homography,
    /// The correlation coefficient at the finest level.
    pub rho: f64,
}

/// Aligns `image` to `template` (same size), coarse to fine. `None` for [`Motion::None`] or
/// when nothing overlaps.
pub fn ecc(template: &Proxy, image: &Proxy, motion: Motion) -> Option<Found> {
    ecc_from(template, image, motion, &Homography::IDENTITY)
}

/// [`ecc`] starting from `init` (reference → frame, proxy units), e.g. a bitmap or feature
/// alignment's.
pub fn ecc_from(
    template: &Proxy,
    image: &Proxy,
    motion: Motion,
    init: &Homography,
) -> Option<Found> {
    if motion == Motion::None || template.w != image.w || template.h != image.h {
        return None;
    }
    let mut ts = vec![template.clone()];
    let mut is = vec![image.clone()];
    while ts.len() < 7 && ts.last().is_some_and(|t| t.w.min(t.h) / 2 >= 48) {
        let t = ts.last().map(Proxy::half)?;
        let i = is.last().map(Proxy::half)?;
        ts.push(t);
        is.push(i);
    }
    // Each level's normalisation: the base's centre and half long side, then each coarser
    // level's from the finer one (block means put the centre at (c − 0.5)/2).
    let mut frames = vec![Frame {
        cx: (template.w as f64 - 1.0) / 2.0,
        cy: (template.h as f64 - 1.0) / 2.0,
        r: template.w.max(template.h) as f64 / 2.0,
    }];
    for l in 1..ts.len() {
        let f = frames[l - 1];
        frames.push(Frame {
            cx: (f.cx - 0.5) / 2.0,
            cy: (f.cy - 0.5) / 2.0,
            r: f.r / 2.0,
        });
    }
    let mut p = motion.params_of(&frames[0].normalised_map(init));
    let mut rho = 0.0;
    for l in (0..ts.len()).rev() {
        let iterations = match (l, motion.params()) {
            (0, k) if k > 4 => 40,
            (0, _) => 25,
            _ => 60,
        };
        let (q, r) = ecc_level(&ts[l], &is[l], frames[l], motion, p, iterations);
        p = q;
        rho = r;
    }
    let f = frames[0];
    let h = f.pixel_map(&motion.matrix(&p));
    if !h.is_finite() {
        return None;
    }
    // The nearest rotation and translation about the centre.
    let m = h.0;
    let angle = m[3].atan2(m[0]);
    let (cx, cy) = (f.cx, f.cy);
    let (qx, qy) = h.apply(cx, cy);
    let transform = Transform {
        angle,
        tx: qx - cx,
        ty: qy - cy,
        cx,
        cy,
    };
    Some(Found { transform, h, rho })
}

/// A binomial 5×5 smoothing (1 4 6 4 1, separable); NaN where any tap is.
fn smooth(p: &Proxy) -> Proxy {
    const K: [f32; 5] = [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0];
    let (w, h) = (p.w, p.h);
    let mut t = vec![f32::NAN; w * h];
    t.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for x in 2..w.saturating_sub(2) {
            row[x] = (0..5).map(|k| K[k] * p.v[y * w + x + k - 2]).sum();
        }
    });
    let mut v = vec![f32::NAN; w * h];
    v.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        if y < 2 || y + 2 >= h {
            return;
        }
        for (x, o) in row.iter_mut().enumerate() {
            *o = (0..5).map(|k| K[k] * t[(y + k - 2) * w + x]).sum();
        }
    });
    Proxy { w, h, v }
}

/// Refines `init` (reference → frame, proxy units) with `motion` at full proxy resolution
/// only: richer models started from a rotation and translation found coarse to fine, so
/// their extra parameters are never fitted on the small, partly clipped coarse levels.
/// Also the correlation `init` itself has on the refinement's support.
pub fn ecc_refine(
    template: &Proxy,
    image: &Proxy,
    motion: Motion,
    init: &Homography,
) -> Option<(Found, f64)> {
    if motion == Motion::None || template.w != image.w || template.h != image.h {
        return None;
    }
    let f = Frame {
        cx: (template.w as f64 - 1.0) / 2.0,
        cy: (template.h as f64 - 1.0) / 2.0,
        r: template.w.max(template.h) as f64 / 2.0,
    };
    let p0 = motion.params_of(&f.normalised_map(init));
    // Noise in the image gradients biases scale-like parameters (toward shrinking); a
    // binomial 5×5 smoothing of both proxies takes most of it out.
    let template = &smooth(template);
    let image = &smooth(image);
    // The support fixed at the start: only pixels valid in both under `init` take part,
    // so the extra parameters cannot gain by moving clipped areas in or out of it.
    let (gx, gy) = image.gradients();
    let mut t = template.clone();
    t.v.par_chunks_mut(t.w).enumerate().for_each(|(y, row)| {
        for (x, v) in row.iter_mut().enumerate() {
            let (qx, qy) = init.apply(x as f64, y as f64);
            // A margin of a pixel for the refinement to move in.
            let ok = [(0.0, 0.0), (-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)]
                .iter()
                .all(|&(dx, dy)| {
                    let (a, b) = (qx + dx, qy + dy);
                    image.sample(a, b).is_finite()
                        && gx.sample(a, b).is_finite()
                        && gy.sample(a, b).is_finite()
                });
            if !ok {
                *v = f32::NAN;
            }
        }
    });
    // The correlation at the start on the same support (one gather before the first step).
    let (_, start) = ecc_level(&t, image, f, motion, p0, 1);
    let (p, rho) = ecc_level(&t, image, f, motion, p0, 40);
    let h = f.pixel_map(&motion.matrix(&p));
    if !h.is_finite() {
        return None;
    }
    let m = h.0;
    let (qx, qy) = h.apply(f.cx, f.cy);
    Some((
        Found {
            transform: Transform {
                angle: m[3].atan2(m[0]),
                tx: qx - f.cx,
                ty: qy - f.cy,
                cx: f.cx,
                cy: f.cy,
            },
            h,
            rho,
        },
        start,
    ))
}

/// A proxy transform in sample units: blocks of `block` samples, proxy centre at the
/// centre of the whole blocks of the merged area.
pub fn to_samples(t: &Transform, block: u32, w: u32, h: u32) -> Transform {
    let s = f64::from(block.max(1));
    Transform {
        angle: t.angle,
        tx: t.tx * s,
        ty: t.ty * s,
        // The proxy covers whole blocks only: its centre in samples.
        cx: (f64::from(w / block.max(1) * block.max(1)) - 1.0) / 2.0,
        cy: (f64::from(h / block.max(1) * block.max(1)) - 1.0) / 2.0,
    }
}

/// Proxy pixel → sample position: block `i` covers samples `b·i … b·i + b − 1`.
pub fn proxy_to_samples(block: u32) -> Homography {
    let b = f64::from(block.max(1));
    let o = (b - 1.0) / 2.0;
    Homography([b, 0.0, o, 0.0, b, o, 0.0, 0.0, 1.0])
}

/// A proxy-unit map in sample units.
pub fn h_to_samples(h: &Homography, block: u32) -> Homography {
    let s = proxy_to_samples(block);
    h.conjugate(&s).unwrap_or(*h)
}

/// A sample-unit map in proxy units.
pub fn h_to_proxy(h: &Homography, block: u32) -> Homography {
    let s = proxy_to_samples(block);
    s.inverse().and_then(|si| h.conjugate(&si)).unwrap_or(*h)
}
