//! Bundle adjustment of a panorama's cameras (Brown & Lowe, "Automatic panoramic image
//! stitching using invariant features", IJCV 2007, §4–5).
//!
//! - **Model**: every frame is a camera turning about one centre: a rotation (world →
//!   camera, as an angle-axis vector), a focal length in pixels, the principal point at the
//!   frame's centre, and optionally one radial distortion term `k1` shared by all frames
//!   (one lens; used when there is no lens profile to correct the frames first).
//! - **Start**: the focal length from the pairwise homographies (Szeliski & Shum 1997:
//!   a homography of a pure rotation has orthogonal, equal-length columns and rows once the
//!   focal lengths are divided out) unless the metadata gives it; rotations along the
//!   maximum spanning tree of the match graph, each from its neighbour's by the absolute
//!   orientation of their matched rays.
//! - **Adjustment**: Levenberg–Marquardt (Marquardt 1963; Triggs et al., "Bundle
//!   adjustment — a modern synthesis", 2000) on the ray-space error: for each match, the
//!   difference of the two world rays, scaled by √(f_a·f_b) so it is in pixels, with a
//!   Huber loss (iteratively reweighted). Frames join one at a time, the best-connected
//!   first, each started from its best-matching joined neighbour, and the joined set is
//!   adjusted again. The first frame's rotation is held (the gauge). Normal equations are
//!   dense, (4N + 1)², solved by Cholesky.
//! - **Straightening** (§5.1): people rarely twist the camera about its forward axis, so the
//!   cameras' x axes lie in one plane; the world's up is the null vector of their
//!   covariance, and the world is turned so that up is the y axis and the mean view looks
//!   along z.

// Small fixed-size matrices read clearest with indices.
#![allow(clippy::needless_range_loop)]

use rayon::prelude::*;

use super::homography::{
    Homography, angle_axis, cholesky_solve, mul3, mul3v, rotation, symmetric_eigen, transpose3,
};
use super::ransac::{Intrinsics, Pair, absolute_orientation};

/// One frame's camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// World → camera.
    pub r: [f64; 9],
    /// Pixels.
    pub f: f64,
    pub cx: f64,
    pub cy: f64,
    /// Radial distortion: normalised position `u` shows at `u·(1 + k1·|u|²)`.
    pub k1: f64,
}

impl Camera {
    pub fn intrinsics(&self) -> Intrinsics {
        Intrinsics {
            f: self.f,
            cx: self.cx,
            cy: self.cy,
        }
    }

    /// The pixel a world direction shows at; `None` behind the camera.
    pub fn project(&self, d: [f64; 3]) -> Option<(f64, f64)> {
        let c = mul3v(&self.r, d);
        if c[2] <= 1e-9 {
            return None;
        }
        let (u, v) = (c[0] / c[2], c[1] / c[2]);
        let k = 1.0 + self.k1 * (u * u + v * v);
        Some((self.cx + self.f * u * k, self.cy + self.f * v * k))
    }

    /// The unit world direction pixel (x, y) shows.
    pub fn unproject(&self, x: f64, y: f64) -> [f64; 3] {
        let (du, dv) = ((x - self.cx) / self.f, (y - self.cy) / self.f);
        let (mut u, mut v) = (du, dv);
        if self.k1 != 0.0 {
            // Invert the distortion (fixed point on the radius).
            let rd = du.hypot(dv);
            if rd > 0.0 {
                let mut r = rd;
                for _ in 0..20 {
                    r = rd / (1.0 + self.k1 * r * r);
                }
                u = du * r / rd;
                v = dv * r / rd;
            }
        }
        let n = (u * u + v * v + 1.0).sqrt();
        mul3v(&transpose3(&self.r), [u / n, v / n, 1.0 / n])
    }
}

/// The matches of a frame pair that passed RANSAC.
#[derive(Clone, Debug)]
pub struct PairMatches {
    pub a: usize,
    pub b: usize,
    /// `(point in a, point in b)`.
    pub pairs: Vec<Pair>,
    /// The pair's homography (a → b), for the focal estimate.
    pub h: Homography,
}

/// A frame's size and, when the metadata says, its focal length in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameInfo {
    pub w: u32,
    pub h: u32,
    pub focal: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BundleOptions {
    /// Estimate `k1` (no lens profile corrected the frames).
    pub k1: bool,
    /// Huber threshold in pixels.
    pub huber: f64,
    /// Hold the focal lengths (the metadata is trusted).
    pub fixed_focal: bool,
}

impl Default for BundleOptions {
    fn default() -> Self {
        BundleOptions {
            k1: false,
            huber: 2.0,
            fixed_focal: false,
        }
    }
}

/// The adjusted cameras.
#[derive(Clone, Debug)]
pub struct Bundle {
    /// Per frame given; `None` for frames not connected to the largest component.
    pub cameras: Vec<Option<Camera>>,
    /// Root-mean-square reprojection error over all matches, pixels.
    pub rms: f64,
}

/// A focal length from a homography of a pure rotation, both cameras' (√(f_a·f_b));
/// coordinates centred on the principal points.
pub fn focal_of(h: &Homography) -> Option<f64> {
    let h = h.normalised().0;
    // f_b from the columns, f_a from the rows (see the module documentation).
    let fb2 = {
        let (d1, d2) = (h[6] * h[7], h[7] * h[7] - h[6] * h[6]);
        let (n1, n2) = (
            -(h[0] * h[1] + h[3] * h[4]),
            h[0] * h[0] + h[3] * h[3] - h[1] * h[1] - h[4] * h[4],
        );
        let (a, b) = if d1.abs() > d2.abs() {
            (n1, d1)
        } else {
            (n2, d2)
        };
        (b != 0.0).then(|| a / b)
    };
    let fa2 = {
        let (d1, d2) = (
            h[0] * h[3] + h[1] * h[4],
            h[0] * h[0] + h[1] * h[1] - h[3] * h[3] - h[4] * h[4],
        );
        let (n1, n2) = (-h[2] * h[5], h[5] * h[5] - h[2] * h[2]);
        let (a, b) = if d1.abs() > d2.abs() {
            (n1, d1)
        } else {
            (n2, d2)
        };
        (b != 0.0).then(|| a / b)
    };
    match (fa2, fb2) {
        (Some(a), Some(b)) if a > 0.0 && b > 0.0 => Some((a * b).sqrt().sqrt()),
        (Some(a), _) if a > 0.0 => Some(a.sqrt()),
        (_, Some(b)) if b > 0.0 => Some(b.sqrt()),
        _ => None,
    }
}

/// The parameters being adjusted: per joined frame its rotation (but the gauge's) and focal
/// length (unless held), then `k1`.
struct Layout {
    rot: Vec<Option<usize>>,
    foc: Vec<Option<usize>>,
    k1: Option<usize>,
    n: usize,
}

impl Layout {
    fn new(joined: &[usize], frames: usize, gauge: usize, o: &BundleOptions) -> Layout {
        let (mut rot, mut foc) = (vec![None; frames], vec![None; frames]);
        let mut n = 0;
        for &i in joined {
            if i != gauge {
                rot[i] = Some(n);
                n += 3;
            }
            if !o.fixed_focal {
                foc[i] = Some(n);
                n += 1;
            }
        }
        let k1 = o.k1.then(|| {
            n += 1;
            n - 1
        });
        Layout { rot, foc, k1, n }
    }
}

/// Cameras with parameter vector `delta` added.
fn apply(cams: &[Camera], l: &Layout, delta: &[f64]) -> Vec<Camera> {
    let mut out = cams.to_vec();
    for (i, c) in out.iter_mut().enumerate() {
        if let Some(at) = l.rot[i] {
            let w = angle_axis(&c.r);
            c.r = rotation([w[0] + delta[at], w[1] + delta[at + 1], w[2] + delta[at + 2]]);
        }
        if let Some(at) = l.foc[i] {
            c.f += delta[at];
        }
        if let Some(k) = l.k1 {
            c.k1 = cams[i].k1 + delta[k];
        }
    }
    out
}

/// The 3 residuals of one match.
#[inline]
fn residual(a: &Camera, b: &Camera, p: &Pair) -> [f64; 3] {
    let ra = a.unproject(p.0.0, p.0.1);
    let rb = b.unproject(p.1.0, p.1.1);
    let s = (a.f * b.f).sqrt();
    [
        s * (ra[0] - rb[0]),
        s * (ra[1] - rb[1]),
        s * (ra[2] - rb[2]),
    ]
}

/// The parameters of camera `i` touched by its residuals: (index, which), where `which` is
/// 0..3 rotation, 3 focal, 4 k1.
fn params_of(l: &Layout, i: usize) -> Vec<(usize, u8)> {
    let mut v = Vec::new();
    if let Some(at) = l.rot[i] {
        v.extend((0..3).map(|k| (at + k, k as u8)));
    }
    if let Some(at) = l.foc[i] {
        v.push((at, 3));
    }
    v
}

fn perturb(c: &Camera, which: u8, e: f64) -> Camera {
    let mut c = *c;
    match which {
        0..=2 => {
            let mut w = angle_axis(&c.r);
            w[which as usize] += e;
            c.r = rotation(w);
        }
        3 => c.f += e,
        _ => c.k1 += e,
    }
    c
}

/// Normal equations and the robust cost at `cams`.
fn normal_equations(
    cams: &[Camera],
    matches: &[&PairMatches],
    l: &Layout,
    huber: f64,
) -> (Vec<f64>, Vec<f64>, f64) {
    let n = l.n;
    let parts: Vec<(Vec<f64>, Vec<f64>, f64)> = matches
        .par_iter()
        .map(|m| {
            let mut jtj = vec![0.0; n * n];
            let mut jtr = vec![0.0; n];
            let mut cost = 0.0;
            let (a, b) = (&cams[m.a], &cams[m.b]);
            let mut ps: Vec<(usize, u8, bool)> = params_of(l, m.a)
                .into_iter()
                .map(|(i, w)| (i, w, true))
                .chain(params_of(l, m.b).into_iter().map(|(i, w)| (i, w, false)))
                .collect();
            if let Some(k) = l.k1 {
                ps.push((k, 4, true));
            }
            for p in &m.pairs {
                let r = residual(a, b, p);
                let norm = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
                // Huber: quadratic within the threshold, linear beyond.
                let w = if norm <= huber { 1.0 } else { huber / norm };
                cost += if norm <= huber {
                    norm * norm
                } else {
                    2.0 * huber * norm - huber * huber
                };
                let mut j: Vec<[f64; 3]> = Vec::with_capacity(ps.len());
                for &(_, which, on_a) in &ps {
                    let e = match which {
                        0..=2 => 1e-6,
                        3 => 1e-4 * a.f.max(1.0) * 1e-2,
                        _ => 1e-6,
                    };
                    let (rp, rm) = if which == 4 {
                        let (ap, bp) = (perturb(a, 4, e), perturb(b, 4, e));
                        let (am, bm) = (perturb(a, 4, -e), perturb(b, 4, -e));
                        (residual(&ap, &bp, p), residual(&am, &bm, p))
                    } else if on_a {
                        (
                            residual(&perturb(a, which, e), b, p),
                            residual(&perturb(a, which, -e), b, p),
                        )
                    } else {
                        (
                            residual(a, &perturb(b, which, e), p),
                            residual(a, &perturb(b, which, -e), p),
                        )
                    };
                    j.push(std::array::from_fn(|k| (rp[k] - rm[k]) / (2.0 * e)));
                }
                for (x, &(ix, _, _)) in ps.iter().enumerate() {
                    for k in 0..3 {
                        jtr[ix] += w * j[x][k] * r[k];
                    }
                    for (y, &(iy, _, _)) in ps.iter().enumerate() {
                        let mut s = 0.0;
                        for k in 0..3 {
                            s += j[x][k] * j[y][k];
                        }
                        jtj[ix * n + iy] += w * s;
                    }
                }
            }
            (jtj, jtr, cost)
        })
        .collect();
    let mut jtj = vec![0.0; n * n];
    let mut jtr = vec![0.0; n];
    let mut cost = 0.0;
    for (a, b, c) in parts {
        for (x, y) in jtj.iter_mut().zip(a) {
            *x += y;
        }
        for (x, y) in jtr.iter_mut().zip(b) {
            *x += y;
        }
        cost += c;
    }
    (jtj, jtr, cost)
}

fn robust_cost(cams: &[Camera], matches: &[&PairMatches], huber: f64) -> f64 {
    matches
        .par_iter()
        .map(|m| {
            m.pairs
                .iter()
                .map(|p| {
                    let r = residual(&cams[m.a], &cams[m.b], p);
                    let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
                    if n <= huber {
                        n * n
                    } else {
                        2.0 * huber * n - huber * huber
                    }
                })
                .sum::<f64>()
        })
        .sum()
}

/// Levenberg–Marquardt over the joined frames.
fn adjust(
    cams: &mut [Camera],
    matches: &[&PairMatches],
    joined: &[usize],
    gauge: usize,
    o: &BundleOptions,
    iterations: usize,
) {
    let l = Layout::new(joined, cams.len(), gauge, o);
    if l.n == 0 || matches.is_empty() {
        return;
    }
    let mut lambda = 1e-3;
    for _ in 0..iterations {
        let (jtj, jtr, cost) = normal_equations(cams, matches, &l, o.huber);
        let mut improved = false;
        for _ in 0..10 {
            let mut a = jtj.clone();
            for i in 0..l.n {
                a[i * l.n + i] += lambda * jtj[i * l.n + i].max(1e-9);
            }
            let rhs: Vec<f64> = jtr.iter().map(|v| -v).collect();
            let Some(d) = cholesky_solve(&a, &rhs, l.n) else {
                lambda *= 10.0;
                continue;
            };
            let next = apply(cams, &l, &d);
            if next.iter().any(|c| !(c.f > 1.0 && c.f.is_finite())) {
                lambda *= 10.0;
                continue;
            }
            let c2 = robust_cost(&next, matches, o.huber);
            if c2 < cost {
                cams.copy_from_slice(&next);
                lambda = (lambda / 10.0).max(1e-12);
                improved = cost - c2 > 1e-10 * cost;
                break;
            }
            lambda *= 10.0;
        }
        if !improved {
            break;
        }
    }
}

/// Root-mean-square reprojection error of `matches` (each way), pixels.
pub fn reprojection_rms(cams: &[Camera], matches: &[PairMatches]) -> f64 {
    let (mut s, mut n) = (0.0, 0usize);
    for m in matches {
        let (a, b) = (&cams[m.a], &cams[m.b]);
        for p in &m.pairs {
            if let Some(q) = b.project(a.unproject(p.0.0, p.0.1)) {
                s += (q.0 - p.1.0).powi(2) + (q.1 - p.1.1).powi(2);
                n += 1;
            }
            if let Some(q) = a.project(b.unproject(p.1.0, p.1.1)) {
                s += (q.0 - p.0.0).powi(2) + (q.1 - p.0.1).powi(2);
                n += 1;
            }
        }
    }
    if n == 0 { 0.0 } else { (s / n as f64).sqrt() }
}

/// The largest connected component of the match graph over `frames` frames.
pub fn largest_component(frames: usize, matches: &[PairMatches]) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..frames).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut k = i;
        while p[k] != r {
            let next = p[k];
            p[k] = r;
            k = next;
        }
        r
    }
    for m in matches {
        let (a, b) = (find(&mut parent, m.a), find(&mut parent, m.b));
        if a != b {
            parent[a] = b;
        }
    }
    let mut size = vec![0usize; frames];
    for i in 0..frames {
        let r = find(&mut parent, i);
        size[r] += 1;
    }
    let best = (0..frames)
        .max_by_key(|&r| (size[r], usize::MAX - r))
        .unwrap_or(0);
    (0..frames)
        .filter(|&i| find(&mut parent, i) == best)
        .collect()
}

/// The rotation taking frame `a`'s camera to `b`'s (`R_b = R_ab R_a`) from their matches.
fn relative(m: &PairMatches, ca: &Camera, cb: &Camera, a_is_first: bool) -> [f64; 9] {
    let (ia, ib) = (ca.intrinsics(), cb.intrinsics());
    let (ra, rb): (Vec<[f64; 3]>, Vec<[f64; 3]>) = m
        .pairs
        .iter()
        .map(|p| {
            let (pa, pb) = if a_is_first { (p.0, p.1) } else { (p.1, p.0) };
            (ia.ray(pa.0, pa.1), ib.ray(pb.0, pb.1))
        })
        .unzip();
    absolute_orientation(&ra, &rb)
}

/// Adjusts the cameras of `frames` from their `matches` (pairs that passed the overlap
/// test). Frames outside the largest connected component get `None`.
pub fn bundle(frames: &[FrameInfo], matches: &[PairMatches], o: &BundleOptions) -> Bundle {
    let n = frames.len();
    let component = largest_component(n, matches);
    let mut on = vec![false; n];
    for &i in &component {
        on[i] = true;
    }
    let matches: Vec<&PairMatches> = matches.iter().filter(|m| on[m.a] && on[m.b]).collect();
    // Focal lengths: the metadata's, else the median of the pairs' estimates, else the
    // frame's long side (about a 53° lens).
    let mut fs: Vec<f64> = matches
        .iter()
        .filter_map(|m| {
            let (fa, fb) = (&frames[m.a], &frames[m.b]);
            // Centre the coordinates on the principal points.
            let ta = Homography::translation(f64::from(fa.w) / 2.0, f64::from(fa.h) / 2.0);
            let tb = Homography::translation(-f64::from(fb.w) / 2.0, -f64::from(fb.h) / 2.0);
            focal_of(&tb.then_after(&m.h).then_after(&ta))
        })
        .filter(|f| f.is_finite())
        .collect();
    fs.sort_by(f64::total_cmp);
    let fallback = fs
        .get(fs.len() / 2)
        .copied()
        .unwrap_or_else(|| f64::from(frames.first().map_or(1000, |f| f.w.max(f.h))));
    let mut cams: Vec<Camera> = frames
        .iter()
        .map(|f| Camera {
            r: rotation([0.0; 3]),
            f: f.focal.unwrap_or(fallback),
            cx: f64::from(f.w) / 2.0,
            cy: f64::from(f.h) / 2.0,
            k1: 0.0,
        })
        .collect();
    let mut o = *o;
    if frames.iter().any(|f| f.focal.is_none()) {
        o.fixed_focal = false;
    }
    // Match counts between frames.
    let mut weight = vec![0usize; n * n];
    for m in &matches {
        weight[m.a * n + m.b] += m.pairs.len();
        weight[m.b * n + m.a] += m.pairs.len();
    }
    // The root: the frame with the most matches.
    let Some(&root) = component
        .iter()
        .max_by_key(|&&i| (0..n).map(|j| weight[i * n + j]).sum::<usize>())
    else {
        return Bundle {
            cameras: vec![None; n],
            rms: 0.0,
        };
    };
    let mut joined = vec![root];
    let mut is_joined = vec![false; n];
    is_joined[root] = true;
    while joined.len() < component.len() {
        // The unjoined frame with most matches to the joined set, started from its best
        // joined neighbour.
        let Some((next, from)) = component
            .iter()
            .filter(|&&i| !is_joined[i])
            .filter_map(|&i| {
                let (best, w) = joined
                    .iter()
                    .map(|&j| (j, weight[i * n + j]))
                    .max_by_key(|x| x.1)?;
                let total: usize = joined.iter().map(|&j| weight[i * n + j]).sum();
                (w > 0).then_some((i, best, total))
            })
            .max_by_key(|x| x.2)
            .map(|x| (x.0, x.1))
        else {
            break;
        };
        let m = matches
            .iter()
            .filter(|m| (m.a == next && m.b == from) || (m.a == from && m.b == next))
            .max_by_key(|m| m.pairs.len());
        if let Some(m) = m {
            // R_next = R_(from→next) R_from.
            let rel = relative(m, &cams[from], &cams[next], m.a == from);
            cams[next].r = mul3(&rel, &cams[from].r);
            cams[next].f = cams[from].f;
        }
        joined.push(next);
        is_joined[next] = true;
        let sub: Vec<&PairMatches> = matches
            .iter()
            .copied()
            .filter(|m| is_joined[m.a] && is_joined[m.b])
            .collect();
        let it = if joined.len() == component.len() {
            100
        } else {
            15
        };
        adjust(&mut cams, &sub, &joined, root, &o, it);
    }
    straighten(&mut cams, &component);
    let all: Vec<PairMatches> = matches.iter().map(|m| (*m).clone()).collect();
    let rms = reprojection_rms(&cams, &all);
    Bundle {
        cameras: (0..n).map(|i| on[i].then_some(cams[i])).collect(),
        rms,
    }
}

/// Turns the world so that up is −y and the mean view looks along +z (see the module
/// documentation).
pub fn straighten(cams: &mut [Camera], which: &[usize]) {
    if which.is_empty() {
        return;
    }
    let mut cov = [0.0f64; 9];
    let (mut ysum, mut zsum) = ([0.0f64; 3], [0.0f64; 3]);
    for &i in which {
        let r = &cams[i].r;
        let x = [r[0], r[1], r[2]];
        for a in 0..3 {
            for b in 0..3 {
                cov[a * 3 + b] += x[a] * x[b];
            }
            ysum[a] += r[3 + a];
            zsum[a] += r[6 + a];
        }
    }
    let (vals, vecs) = symmetric_eigen(&cov, 3);
    let mut up = if which.len() >= 3 && vals[1] > 1e-6 {
        [vecs[0][0], vecs[0][1], vecs[0][2]]
    } else {
        ysum
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    // The image y axis points down: up agrees with the cameras' mean y.
    if dot(up, ysum) < 0.0 {
        up = up.map(|v| -v);
    }
    let norm = |a: [f64; 3]| {
        let n = dot(a, a).sqrt().max(1e-300);
        a.map(|v| v / n)
    };
    let y = norm(up);
    // Forward: the mean view with its up part removed.
    let d = dot(zsum, y);
    let mut z = norm([zsum[0] - d * y[0], zsum[1] - d * y[1], zsum[2] - d * y[2]]);
    if !z.iter().all(|v| v.is_finite()) || dot(z, z) < 0.5 {
        z = [0.0, 0.0, 1.0];
    }
    let x = [
        y[1] * z[2] - y[2] * z[1],
        y[2] * z[0] - y[0] * z[2],
        y[0] * z[1] - y[1] * z[0],
    ];
    // G: old world → new world, rows the new axes in old coordinates.
    let g = [x[0], x[1], x[2], y[0], y[1], y[2], z[0], z[1], z[2]];
    let gt = transpose3(&g);
    for c in cams.iter_mut() {
        c.r = mul3(&c.r, &gt);
    }
}

#[cfg(test)]
mod tests {
    use super::super::homography::rotation_distance;
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// Cameras turning about one centre: yaw steps with small pitch and roll.
    fn truth(n: usize, f: f64) -> Vec<Camera> {
        (0..n)
            .map(|i| {
                let yaw = 0.42 * i as f64;
                let pitch = 0.03 * (i as f64 * 1.3).sin();
                let roll = 0.01 * (i as f64 * 0.7).cos();
                let r = mul3(
                    &rotation([0.0, 0.0, roll]),
                    &mul3(&rotation([pitch, 0.0, 0.0]), &rotation([0.0, yaw, 0.0])),
                );
                Camera {
                    r,
                    f,
                    cx: 320.0,
                    cy: 240.0,
                    k1: 0.0,
                }
            })
            .collect()
    }

    /// Matches between neighbours from random world points seen by both, with noise.
    fn matches(cams: &[Camera], noise: f64) -> Vec<PairMatches> {
        let mut rng = Rng(77);
        let mut out = Vec::new();
        for a in 0..cams.len() {
            for b in a + 1..cams.len() {
                let mut pairs = Vec::new();
                for _ in 0..4000 {
                    let p = (rng.next() * 640.0, rng.next() * 480.0);
                    let d = cams[a].unproject(p.0, p.1);
                    let Some(q) = cams[b].project(d) else {
                        continue;
                    };
                    if !(0.0..640.0).contains(&q.0) || !(0.0..480.0).contains(&q.1) {
                        continue;
                    }
                    let e = |r: &mut Rng| (r.next() - 0.5) * 2.0 * noise;
                    pairs.push(((p.0 + e(&mut rng), p.1 + e(&mut rng)), q));
                    if pairs.len() >= 400 {
                        break;
                    }
                }
                if pairs.len() >= 30 {
                    let h = super::super::ransac::dlt(&pairs).expect("dlt");
                    out.push(PairMatches { a, b, pairs, h });
                }
            }
        }
        out
    }

    #[test]
    fn focal_from_a_rotation_homography() {
        let i = Intrinsics {
            f: 900.0,
            cx: 0.0,
            cy: 0.0,
        };
        let r = rotation([0.05, 0.3, 0.02]);
        let h = super::super::ransac::rotation_homography(&r, &i, &i);
        let f = focal_of(&h).expect("focal");
        assert!((f - 900.0).abs() < 1e-6, "{f}");
    }

    #[test]
    fn bundle_recovers_rotations_and_focal_length() {
        let f = 820.0;
        let cams = truth(6, f);
        let m = matches(&cams, 0.05);
        let frames = vec![
            FrameInfo {
                w: 640,
                h: 480,
                focal: None,
            };
            6
        ];
        let b = bundle(&frames, &m, &BundleOptions::default());
        let got: Vec<Camera> = b.cameras.iter().map(|c| c.expect("joined")).collect();
        assert!(b.rms < 0.1, "rms {:.4} px", b.rms);
        for c in &got {
            assert!((c.f - f).abs() / f < 0.003, "focal {:.2}", c.f);
        }
        // Relative rotations are free of the world's choice.
        for i in 1..6 {
            let want = mul3(&cams[i].r, &transpose3(&cams[0].r));
            let have = mul3(&got[i].r, &transpose3(&got[0].r));
            let d = rotation_distance(&want, &have).to_degrees();
            assert!(d < 0.01, "frame {i}: {d:.4}°");
        }
        // Straightened: every camera's x axis is level (perpendicular to up, −y).
        for c in &got {
            assert!(c.r[1].abs() < 0.02, "x axis tilt {:.4}", c.r[1]);
        }
    }

    #[test]
    fn frames_without_matches_are_left_out() {
        let cams = truth(3, 800.0);
        let mut m = matches(&cams, 0.0);
        m.retain(|p| p.a == 0 && p.b == 1);
        let frames = vec![
            FrameInfo {
                w: 640,
                h: 480,
                focal: None,
            };
            4
        ];
        let b = bundle(&frames, &m, &BundleOptions::default());
        assert!(b.cameras[0].is_some() && b.cameras[1].is_some());
        assert!(b.cameras[2].is_none() && b.cameras[3].is_none());
    }
}
