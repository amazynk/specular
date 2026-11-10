//! Upright: the camera angles that make lines plumb and level (after H. Lee, E. Shechtman,
//! J. Wang, S. Lee, "Automatic Upright Adjustment of Photographs", CVPR 2012).
//!
//! Lines are in the oriented frame (square pixels, centred, y down), as the lens-corrected
//! picture shows them before any keystone. The forward map is the keystone of
//! [`super::persp::keystone`] followed by the straighten turn the crop module already has,
//! so the found roll works together with it.
//!
//! - **Energy:** each vertical line contributes the sine of its angle from plumb after the
//!   map, each horizontal line its angle from level, weighted (by length for detected
//!   lines). Full and auto add a small distortion penalty: the change of the warped frame's
//!   area and aspect. Auto adds a prior on the yaw that fades out as the horizontal set
//!   grows, so a weak set does not drive it. A tiny pull to 0 keeps undetermined angles
//!   where they are.
//! - **Solver:** Levenberg–Marquardt with a numeric Jacobian, from 0.
//! - **Unknowns by mode:** level: roll; vertical: roll and pitch; full and auto: all three;
//!   guided: roll, pitch when there are vertical guides, yaw when there are horizontal ones.
//! - **Detected lines** ([`auto`]): classified near-vertical and near-horizontal (within 30°
//!   of the current roll), then each set is cut to the lines through one vanishing point by
//!   RANSAC on pairs (seeded: the same picture gives the same answer). The two vanishing
//!   points also give a focal length when the file has none (`f² = −v₁·v₂`).

use super::persp::{keystone, turn};
use crate::colour::M3;

/// A line segment's two ends (centred oriented-frame pixels) and its weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub a: (f64, f64),
    pub b: (f64, f64),
    pub weight: f64,
}

impl Line {
    pub fn new(a: (f64, f64), b: (f64, f64), weight: f64) -> Line {
        Line { a, b, weight }
    }

    fn len(&self) -> f64 {
        (self.b.0 - self.a.0).hypot(self.b.1 - self.a.1)
    }

    /// Angle from the x axis, 0..π.
    fn angle(&self) -> f64 {
        (self.b.1 - self.a.1)
            .atan2(self.b.0 - self.a.0)
            .rem_euclid(std::f64::consts::PI)
    }

    /// The homogeneous line through both ends.
    fn homogeneous(&self) -> [f64; 3] {
        cross([self.a.0, self.a.1, 1.0], [self.b.0, self.b.1, 1.0])
    }
}

/// What a solve looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Level,
    Vertical,
    Full,
    Auto,
    Guided,
}

/// The problem: the frame, its focal length, the straighten turn after the keystone and the
/// lines to make plumb and level.
#[derive(Clone, Debug)]
pub struct Problem {
    pub w: f64,
    pub h: f64,
    /// Pixels.
    pub f: f64,
    /// Radians, clockwise.
    pub straighten: f64,
    pub vertical: Vec<Line>,
    pub horizontal: Vec<Line>,
}

/// Pitch, yaw and roll (radians) and, from vanishing points, a focal length (pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solution {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
    pub focal: Option<f64>,
}

/// Why no solve was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Fewer than two usable lines for an axis the mode needs.
    FewLines,
}

/// Lines a mode needs per axis it uses.
pub const MIN_LINES: usize = 2;

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn map(m: &M3, p: (f64, f64)) -> Option<(f64, f64)> {
    super::persp::apply(m, p)
}

/// The forward map for angles `x = [pitch, yaw, roll]`.
fn forward(p: &Problem, x: [f64; 3]) -> M3 {
    super::persp::mul(&turn(p.straighten), &keystone(p.f, x[0], x[1], x[2]))
}

/// Which angles a goal solves (pitch, yaw, roll).
fn unknowns(goal: Goal, p: &Problem) -> [bool; 3] {
    match goal {
        Goal::Level => [false, false, true],
        Goal::Vertical => [true, false, true],
        Goal::Full | Goal::Auto => [true, true, true],
        Goal::Guided => [!p.vertical.is_empty(), !p.horizontal.is_empty(), true],
    }
}

/// The residuals at angles `x`.
fn residuals(goal: Goal, p: &Problem, x: [f64; 3], out: &mut Vec<f64>) {
    out.clear();
    let m = forward(p, x);
    let mut push = |l: &Line, vertical: bool| {
        let s = l.weight.max(0.0).sqrt();
        match (map(&m, l.a), map(&m, l.b)) {
            (Some(a), Some(b)) => {
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let n = dx.hypot(dy).max(1e-12);
                out.push(s * if vertical { dx / n } else { dy / n });
            }
            // Beyond the horizon: as bad as it gets.
            _ => out.push(s),
        }
    };
    for l in &p.vertical {
        push(l, true);
    }
    for l in &p.horizontal {
        push(l, false);
    }
    // A tiny pull to 0 for what the lines leave open.
    for v in x {
        out.push(1e-3 * v);
    }
    if matches!(goal, Goal::Full | Goal::Auto) {
        // Distortion: the warped frame's area and aspect against the frame's.
        let k = keystone(p.f, x[0], x[1], x[2]);
        let (w, h) = (p.w / 2.0, p.h / 2.0);
        let c: Vec<(f64, f64)> = [(-w, -h), (w, -h), (w, h), (-w, h)]
            .iter()
            .filter_map(|&q| map(&k, q))
            .collect();
        if c.len() == 4 {
            let area = 0.5
                * (0..4)
                    .map(|i| {
                        let (a, b) = (c[i], c[(i + 1) % 4]);
                        a.0 * b.1 - b.0 * a.1
                    })
                    .sum::<f64>()
                    .abs();
            let d = |i: usize, j: usize| (c[i].0 - c[j].0).hypot(c[i].1 - c[j].1);
            let (tw, th) = ((d(0, 1) + d(2, 3)) / 2.0, (d(1, 2) + d(3, 0)) / 2.0);
            let mu = 0.003;
            out.push(mu * (area / (4.0 * w * h)).max(1e-9).ln());
            out.push(mu * ((tw / th.max(1e-9)) / (w / h)).max(1e-9).ln());
        } else {
            out.push(1.0);
            out.push(1.0);
        }
    }
    if goal == Goal::Auto {
        // The yaw prior fades as the horizontal set reaches half a diagonal of length.
        let diag = p.w.hypot(p.h);
        let support: f64 = p.horizontal.iter().map(|l| l.len() / diag).sum();
        let fade = (1.0 - support / 0.5).max(0.0);
        out.push(0.3 * fade * x[1]);
    }
}

/// Solves `p` for `goal` (see the module documentation).
pub fn solve(goal: Goal, p: &Problem) -> Result<Solution, Refusal> {
    let (nv, nh) = (p.vertical.len(), p.horizontal.len());
    let enough = match goal {
        Goal::Level => nv + nh >= MIN_LINES,
        Goal::Vertical | Goal::Auto => nv >= MIN_LINES,
        Goal::Full => nv >= MIN_LINES && nh >= MIN_LINES,
        Goal::Guided => nv + nh >= 1,
    };
    if !enough {
        return Err(Refusal::FewLines);
    }
    let free = unknowns(goal, p);
    let idx: Vec<usize> = (0..3).filter(|&i| free[i]).collect();
    let mut x = [0.0f64; 3];
    let mut r = Vec::new();
    let mut rt = Vec::new();
    let cost = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>();
    residuals(goal, p, x, &mut r);
    let mut c = cost(&r);
    let mut lambda = 1e-3;
    let k = idx.len();
    for _ in 0..200 {
        // Numeric Jacobian (central differences).
        let hstep = 1e-6;
        let mut jac = vec![vec![0.0; r.len()]; k];
        for (col, &i) in idx.iter().enumerate() {
            let mut xp = x;
            let mut xm = x;
            xp[i] += hstep;
            xm[i] -= hstep;
            let mut rp = Vec::new();
            let mut rm = Vec::new();
            residuals(goal, p, xp, &mut rp);
            residuals(goal, p, xm, &mut rm);
            for (j, v) in jac[col].iter_mut().enumerate() {
                *v = (rp[j] - rm[j]) / (2.0 * hstep);
            }
        }
        let mut a = [[0.0f64; 3]; 3];
        let mut g = [0.0f64; 3];
        for i in 0..k {
            for j in 0..k {
                a[i][j] = jac[i].iter().zip(&jac[j]).map(|(u, v)| u * v).sum();
            }
            g[i] = jac[i].iter().zip(&r).map(|(u, v)| u * v).sum();
        }
        let mut improved = false;
        for _ in 0..12 {
            let mut m = a;
            for (i, row) in m.iter_mut().enumerate().take(k) {
                row[i] += lambda * a[i][i].max(1e-12);
            }
            let Some(d) = solve_k(&m, &g, k) else {
                lambda *= 10.0;
                continue;
            };
            let mut xn = x;
            for (col, &i) in idx.iter().enumerate() {
                xn[i] -= d[col];
            }
            residuals(goal, p, xn, &mut rt);
            let cn = cost(&rt);
            if cn < c {
                let step: f64 = d.iter().take(k).map(|v| v.abs()).fold(0.0, f64::max);
                x = xn;
                std::mem::swap(&mut r, &mut rt);
                let done = (c - cn) < 1e-16 * c.max(1e-300) || step < 1e-12;
                c = cn;
                lambda = (lambda * 0.3).max(1e-12);
                improved = true;
                if done {
                    return Ok(solution(x));
                }
                break;
            }
            lambda *= 10.0;
        }
        if !improved {
            break;
        }
    }
    Ok(solution(x))
}

fn solution(x: [f64; 3]) -> Solution {
    Solution {
        pitch: x[0],
        yaw: x[1],
        roll: x[2],
        focal: None,
    }
}

/// Solves the `k × k` system `m · d = g` (Gaussian elimination with pivoting).
fn solve_k(m: &[[f64; 3]; 3], g: &[f64; 3], k: usize) -> Option<[f64; 3]> {
    let mut a = *m;
    let mut b = *g;
    for c in 0..k {
        let p = (c..k).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-300 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        let pivot = a[c];
        for r in c + 1..k {
            let f = a[r][c] / pivot[c];
            for (x, y) in a[r][c..k].iter_mut().zip(&pivot[c..k]) {
                *x -= f * y;
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = [0.0; 3];
    for c in (0..k).rev() {
        let s: f64 = (c + 1..k).map(|j| a[c][j] * x[j]).sum();
        x[c] = (b[c] - s) / a[c][c];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// Splits `lines` into near-vertical and near-horizontal ones (within 30° of plumb and
/// level once turned by `roll`, radians clockwise); the rest are dropped.
pub fn classify(lines: &[Line], roll: f64) -> (Vec<Line>, Vec<Line>) {
    let pi = std::f64::consts::PI;
    let lim = 30f64.to_radians();
    let (mut v, mut h) = (Vec::new(), Vec::new());
    for l in lines {
        // The angle on screen after the turn, 0..π.
        let a = (l.angle() + roll).rem_euclid(pi);
        let from_level = a.min(pi - a);
        if from_level <= lim {
            h.push(*l);
        } else if (a - pi / 2.0).abs() <= lim {
            v.push(*l);
        }
    }
    (v, h)
}

/// The angle (radians) between line `l` and the direction from its middle to the
/// homogeneous point `vp`.
fn vp_error(l: &Line, vp: [f64; 3]) -> f64 {
    let m = ((l.a.0 + l.b.0) / 2.0, (l.a.1 + l.b.1) / 2.0);
    let (dx, dy) = if vp[2].abs() < 1e-12 {
        (vp[0], vp[1])
    } else {
        (vp[0] / vp[2] - m.0, vp[1] / vp[2] - m.1)
    };
    let (lx, ly) = (l.b.0 - l.a.0, l.b.1 - l.a.1);
    let n = dx.hypot(dy) * lx.hypot(ly);
    if n < 1e-12 {
        return 0.0;
    }
    ((dx * ly - dy * lx).abs() / n).min(1.0).asin()
}

/// A tiny deterministic generator for RANSAC.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % n.max(1) as u64) as usize
    }
}

/// The lines through the best-supported vanishing point (RANSAC on pairs, inliers within
/// 1.5°, support by weight), and that point.
pub fn dominant(lines: &[Line]) -> (Vec<Line>, Option<[f64; 3]>) {
    if lines.len() < 2 {
        return (lines.to_vec(), None);
    }
    let tol = 1.5f64.to_radians();
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    let n = lines.len();
    let tries = (n * (n - 1) / 2).min(500);
    let mut best: Option<(f64, [f64; 3])> = None;
    let all_pairs = n * (n - 1) / 2 <= 500;
    let mut pair = (0usize, 1usize);
    for _ in 0..tries {
        let (i, j) = if all_pairs {
            let p = pair;
            pair.1 += 1;
            if pair.1 >= n {
                pair.0 += 1;
                pair.1 = pair.0 + 1;
            }
            p
        } else {
            let i = rng.below(n);
            let mut j = rng.below(n);
            if j == i {
                j = (j + 1) % n;
            }
            (i, j)
        };
        let vp = cross(lines[i].homogeneous(), lines[j].homogeneous());
        if vp.iter().all(|v| v.abs() < 1e-18) {
            continue;
        }
        let s: f64 = lines
            .iter()
            .filter(|l| vp_error(l, vp) <= tol)
            .map(|l| l.weight)
            .sum();
        if best.is_none_or(|(b, _)| s > b) {
            best = Some((s, vp));
        }
    }
    match best {
        Some((_, vp)) => (
            lines
                .iter()
                .filter(|l| vp_error(l, vp) <= tol)
                .copied()
                .collect(),
            Some(vp),
        ),
        None => (lines.to_vec(), None),
    }
}

/// The focal length (pixels) two orthogonal vanishing points imply (principal point at the
/// origin), when both are finite and it is real and plausible.
pub fn focal_from(v1: [f64; 3], v2: [f64; 3], diag: f64) -> Option<f64> {
    if v1[2].abs() < 1e-12 || v2[2].abs() < 1e-12 {
        return None;
    }
    let (a, b) = (
        (v1[0] / v1[2], v1[1] / v1[2]),
        (v2[0] / v2[2], v2[1] / v2[2]),
    );
    let f2 = -(a.0 * b.0 + a.1 * b.1);
    let f = (f2 > 0.0).then(|| f2.sqrt())?;
    // 12–300 mm equivalent.
    let mm = super::persp::focal_mm35(f, diag, 0.0);
    (12.0..=300.0).contains(&mm).then_some(f)
}

/// Upright from detected `lines` (centred oriented-frame pixels) for `goal` on a `w × h`
/// frame; `f` is the focal length when known (else the vanishing points' or 28 mm), `roll`
/// the turn already applied (straighten plus rotate, radians), `straighten` the crop
/// module's own.
pub fn auto(
    goal: Goal,
    lines: &[Line],
    w: f64,
    h: f64,
    f: Option<f64>,
    roll: f64,
    straighten: f64,
) -> Result<Solution, Refusal> {
    let diag = w.hypot(h);
    let weighted: Vec<Line> = lines
        .iter()
        .map(|l| Line {
            weight: l.len() / diag,
            ..*l
        })
        .collect();
    let (v, hz) = classify(&weighted, roll);
    let (v, vv) = dominant(&v);
    let (hz, vh) = dominant(&hz);
    let mut focal = None;
    let f = match f {
        Some(f) => f,
        None => {
            focal = match (vv, vh) {
                (Some(a), Some(b)) if v.len() >= MIN_LINES && hz.len() >= MIN_LINES => {
                    focal_from(a, b, diag)
                }
                _ => None,
            };
            focal.unwrap_or(
                super::persp::DEFAULT_FOCAL_MM35 / super::persp::FULL_FRAME_DIAGONAL_MM * diag,
            )
        }
    };
    let p = Problem {
        w,
        h,
        f,
        straighten,
        vertical: v,
        horizontal: if goal == Goal::Vertical {
            Vec::new()
        } else {
            hz
        },
    };
    let mut s = solve(goal, &p)?;
    s.focal = focal;
    Ok(s)
}

/// The most pixels the line detector looks at (larger pictures are reduced first).
pub const PICTURE_PIXELS: usize = 2_000_000;

/// Segments longer than this fraction of the diagonal are kept.
pub const MIN_LENGTH: f64 = 0.015;

/// LSD's segments on an RGBA scene-linear picture (`w × h`) of the oriented frame
/// (`oriented`, square pixels), as centred oriented-frame lines. The luminance is scaled
/// to its 99th percentile and gamma-encoded first, as the detector expects grey levels.
pub fn lines_of_picture(rgba: &[f32], w: usize, h: usize, oriented: (f64, f64)) -> Vec<Line> {
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return Vec::new();
    }
    // A box reduction to at most PICTURE_PIXELS.
    let k = ((w * h) as f64 / PICTURE_PIXELS as f64)
        .sqrt()
        .ceil()
        .max(1.0) as usize;
    let (rw, rh) = (w / k, h / k);
    if rw < 8 || rh < 8 {
        return Vec::new();
    }
    let mut lum = vec![0f32; rw * rh];
    for y in 0..rh {
        for x in 0..rw {
            let mut s = 0.0;
            for j in 0..k {
                for i in 0..k {
                    let p = &rgba[((y * k + j) * w + x * k + i) * 4..][..3];
                    let v = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
                    s += if v.is_finite() { v.max(0.0) } else { 0.0 };
                }
            }
            lum[y * rw + x] = s / (k * k) as f32;
        }
    }
    let mut sorted = lum.clone();
    let i99 = (sorted.len() * 99 / 100).min(sorted.len() - 1);
    let top = *sorted.select_nth_unstable_by(i99, f32::total_cmp).1;
    let top = if top > 1e-6 { top } else { 1.0 };
    for v in &mut lum {
        *v = (*v / top).clamp(0.0, 1.0).powf(1.0 / 2.2);
    }
    let segs = super::lsd::detect(&lum, rw, rh, 255.0);
    let (sx, sy) = (oriented.0 / rw as f64, oriented.1 / rh as f64);
    let min = MIN_LENGTH * oriented.0.hypot(oriented.1);
    segs.iter()
        .map(|s| {
            Line::new(
                (s.a.0 * sx - oriented.0 / 2.0, s.a.1 * sy - oriented.1 / 2.0),
                (s.b.0 * sx - oriented.0 / 2.0, s.b.1 * sy - oriented.1 / 2.0),
                1.0,
            )
        })
        .filter(|l| l.len() >= min)
        .collect()
}

/// Runs automatic upright `mode` on `found` for params `p`: the new params (the solved
/// angles, the keystone sliders back at 0, and the focal length the vanishing points gave
/// when nothing else did).
pub fn run(
    mode: super::persp::Mode,
    p: &super::persp::PerspParams,
    found: &crate::preview::UprightLines,
) -> Result<super::persp::PerspParams, Refusal> {
    use super::persp::Mode;
    let goal = match mode {
        Mode::Level => Goal::Level,
        Mode::Vertical => Goal::Vertical,
        Mode::Full => Goal::Full,
        _ => Goal::Auto,
    };
    let (w, h) = found.oriented;
    let known = p.focal_mm35 > 0.0 || found.focal35.is_some();
    let f = known.then(|| super::persp::focal_px(p, found.focal35, w, h));
    let straighten = f64::from(found.straighten).to_radians();
    let s = auto(goal, &found.lines, w, h, f, straighten, straighten)?;
    // The sliders start again from the found camera.
    let mut out = p.clone();
    out.mode = mode;
    out.vertical = 0.0;
    out.horizontal = 0.0;
    out.rotate = 0.0;
    out.solved = Some([s.pitch, s.yaw, s.roll].map(|v| v.to_degrees() as f32));
    if let Some(f) = s.focal {
        out.focal_mm35 = super::persp::focal_mm35(f, w, h).round() as f32;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::geometry::persp::{apply, inverse};

    /// Lines that are plumb and level after the keystone `k`: the true scene's lines mapped
    /// back through it.
    fn scene(k: &M3, w: f64, h: f64) -> (Vec<Line>, Vec<Line>) {
        let inv = inverse(k).unwrap();
        let back = |p: (f64, f64)| apply(&inv, p).unwrap();
        let mut v = Vec::new();
        let mut hz = Vec::new();
        for i in 0..6 {
            let x = -w * 0.4 + w * 0.8 * f64::from(i) / 5.0;
            v.push(Line::new(back((x, -h * 0.3)), back((x, h * 0.3)), 1.0));
        }
        for i in 0..5 {
            let y = -h * 0.35 + h * 0.7 * f64::from(i) / 4.0;
            hz.push(Line::new(back((-w * 0.3, y)), back((w * 0.3, y)), 1.0));
        }
        (v, hz)
    }

    #[test]
    fn full_recovers_known_angles() {
        let (w, h) = (3000.0, 2000.0);
        let f = 28.0 / 43.266_615 * f64::hypot(w, h);
        let (p0, y0, r0) = (12f64.to_radians(), -8f64.to_radians(), 2f64.to_radians());
        let k = keystone(f, p0, y0, r0);
        let (v, hz) = scene(&k, w, h);
        let p = Problem {
            w,
            h,
            f,
            straighten: 0.0,
            vertical: v,
            horizontal: hz,
        };
        let s = solve(Goal::Full, &p).unwrap();
        let e = [s.pitch - p0, s.yaw - y0, s.roll - r0].map(|v| v.to_degrees().abs());
        assert!(e.iter().all(|v| *v < 0.05), "{e:?} {s:?}");
        let s = solve(Goal::Vertical, &p).unwrap();
        assert!(s.yaw == 0.0);
    }

    #[test]
    fn too_few_lines_are_refused() {
        let p = Problem {
            w: 100.0,
            h: 100.0,
            f: 100.0,
            straighten: 0.0,
            vertical: vec![Line::new((0.0, -1.0), (0.0, 1.0), 1.0)],
            horizontal: Vec::new(),
        };
        assert_eq!(solve(Goal::Vertical, &p), Err(Refusal::FewLines));
        assert_eq!(solve(Goal::Full, &p), Err(Refusal::FewLines));
        assert!(solve(Goal::Guided, &p).is_ok());
    }

    #[test]
    fn ransac_drops_lines_off_the_vanishing_point() {
        let mut ls: Vec<Line> = (0..8)
            .map(|i| {
                let x = -400.0 + 100.0 * f64::from(i);
                Line::new((x, 0.0), (x * 0.9, -300.0), 1.0)
            })
            .collect();
        ls.push(Line::new((10.0, 50.0), (60.0, -250.0), 1.0));
        let (keep, vp) = dominant(&ls);
        assert_eq!(keep.len(), 8);
        let vp = vp.unwrap();
        assert!((vp[1] / vp[2] + 3000.0).abs() < 1.0, "{vp:?}");
    }
}
