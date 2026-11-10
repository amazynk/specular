//! Curved planar reformation (CPR) along a centreline the user places.
//!
//! [`Spline`] is a centripetal Catmull–Rom curve (α = ½, the Barry–Goldman pyramid) through
//! the points, sampled densely with its arc length; centripetal parameterisation has no cusps
//! or self-intersections within a segment, so the curve stays close to what was clicked.
//!
//! [`Cpr`] lays a curve out as an image, one column per target pixel, each column a line
//! through the volume drawn by the ordinary MPR shader as a one-pixel-wide [`Plane`]
//! (so slabs, windows and the slice table behave exactly as in MPR):
//! - **straightened**: the column's position is arc length along the curve and its line runs
//!   along the curve's "down" vector (rotation-minimising along the curve from the vector of
//!   interest `up` projected off the first tangent, turned by the view's angle), so the
//!   centreline is the image's middle row;
//! - **stretched**: the column's position is arc length of the curve projected off `up`, and
//!   every line runs along −`up`, so distances along `up` stay true and the curve shows at its
//!   own height.
//!
//! [`cross_section`] is the plane perpendicular to the curve at an arc length.

use crate::{Plane, Slab, cross, dot};
use specular_render::Window;

pub type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: V3, b: V3, k: f64) -> V3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}

fn unit(a: V3) -> Option<V3> {
    let l = len(a);
    (l > 1e-12).then(|| a.map(|c| c / l))
}

fn lerp(a: V3, b: V3, t: f64) -> V3 {
    [0, 1, 2].map(|c| a[c] + (b[c] - a[c]) * t)
}

/// Samples per mm of chord, and per segment at least and at most.
const SAMPLES_PER_MM: f64 = 4.0;
const MIN_SEGMENT_SAMPLES: usize = 8;
const MAX_SEGMENT_SAMPLES: usize = 4096;
/// Control points kept, at most.
pub const MAX_POINTS: usize = 512;

/// A centripetal Catmull–Rom spline through control points, densely sampled.
#[derive(Clone, Debug, PartialEq)]
pub struct Spline {
    /// The control points (consecutive repeats removed).
    pub points: Vec<V3>,
    samples: Vec<V3>,
    /// Arc length at each sample (mm).
    arc: Vec<f64>,
    /// The sample index of each control point.
    knots: Vec<usize>,
}

/// The point at `t` of the segment from `p1` to `p2` (Barry–Goldman, centripetal knots).
fn segment(p: [V3; 4], u: f64) -> V3 {
    let k = |a: V3, b: V3| len(sub(b, a)).max(1e-9).sqrt();
    let t0 = 0.0;
    let t1 = t0 + k(p[0], p[1]);
    let t2 = t1 + k(p[1], p[2]);
    let t3 = t2 + k(p[2], p[3]);
    let t = t1 + u * (t2 - t1);
    let mix = |a: V3, b: V3, ta: f64, tb: f64| lerp(a, b, (t - ta) / (tb - ta));
    let a1 = mix(p[0], p[1], t0, t1);
    let a2 = mix(p[1], p[2], t1, t2);
    let a3 = mix(p[2], p[3], t2, t3);
    let b1 = mix(a1, a2, t0, t2);
    let b2 = mix(a2, a3, t1, t3);
    mix(b1, b2, t1, t2)
}

impl Spline {
    /// The spline through `points` (at most [`MAX_POINTS`]); `None` with fewer than two
    /// distinct points. The ends continue straight (mirrored neighbours as phantom points).
    pub fn new(points: &[V3]) -> Option<Spline> {
        let mut pts: Vec<V3> = Vec::with_capacity(points.len());
        for &p in points.iter().take(MAX_POINTS) {
            if p.iter().any(|c| !c.is_finite()) {
                continue;
            }
            if pts.last().is_none_or(|q| len(sub(p, *q)) > 1e-6) {
                pts.push(p);
            }
        }
        let n = pts.len();
        if n < 2 {
            return None;
        }
        let at = |i: isize| -> V3 {
            if i < 0 {
                add(pts[0], sub(pts[0], pts[1]), 1.0)
            } else if i as usize >= n {
                add(pts[n - 1], sub(pts[n - 1], pts[n - 2]), 1.0)
            } else {
                pts[i as usize]
            }
        };
        let mut samples = vec![pts[0]];
        let mut knots = vec![0];
        for i in 0..n - 1 {
            let i = i as isize;
            let p = [at(i - 1), at(i), at(i + 1), at(i + 2)];
            let chord = len(sub(p[2], p[1]));
            let m = ((chord * SAMPLES_PER_MM).ceil() as usize)
                .clamp(MIN_SEGMENT_SAMPLES, MAX_SEGMENT_SAMPLES);
            for k in 1..m {
                samples.push(segment(p, k as f64 / m as f64));
            }
            samples.push(p[2]);
            knots.push(samples.len() - 1);
        }
        let mut arc = Vec::with_capacity(samples.len());
        let mut s = 0.0;
        arc.push(0.0);
        for w in samples.windows(2) {
            s += len(sub(w[1], w[0]));
            arc.push(s);
        }
        Some(Spline {
            points: pts,
            samples,
            arc,
            knots,
        })
    }

    /// The curve's length (mm).
    pub fn length(&self) -> f64 {
        *self.arc.last().unwrap_or(&0.0)
    }

    /// The arc length of control point `i`.
    pub fn knot(&self, i: usize) -> f64 {
        self.knots.get(i).map_or(0.0, |&k| self.arc[k])
    }

    /// The dense samples (for drawing the curve).
    pub fn samples(&self) -> &[V3] {
        &self.samples
    }

    /// The sample segment holding arc length `s` (clamped) and the fraction within it.
    fn locate(&self, s: f64) -> (usize, f64) {
        let s = s.clamp(0.0, self.length());
        let i = self
            .arc
            .partition_point(|&a| a <= s)
            .clamp(1, self.arc.len() - 1)
            - 1;
        let d = self.arc[i + 1] - self.arc[i];
        let f = if d > 0.0 { (s - self.arc[i]) / d } else { 0.0 };
        (i, f)
    }

    /// The point at arc length `s` (clamped to the curve).
    pub fn at(&self, s: f64) -> V3 {
        let (i, f) = self.locate(s);
        lerp(self.samples[i], self.samples[i + 1], f)
    }

    /// The unit tangent at arc length `s`.
    pub fn tangent(&self, s: f64) -> V3 {
        let (i, _) = self.locate(s);
        let n = self.samples.len();
        let a = self.samples[i.saturating_sub(1)];
        let b = self.samples[(i + 2).min(n - 1)];
        unit(sub(b, a))
            .or_else(|| unit(sub(self.samples[i + 1], self.samples[i])))
            .unwrap_or([1.0, 0.0, 0.0])
    }

    /// The arc length of the curve's point nearest `p`, and its distance.
    pub fn closest(&self, p: V3) -> (f64, f64) {
        let mut best = (0.0, f64::INFINITY);
        for (i, w) in self.samples.windows(2).enumerate() {
            let d = sub(w[1], w[0]);
            let l2 = dot(d, d);
            let t = if l2 > 0.0 {
                (dot(sub(p, w[0]), d) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let q = add(w[0], d, t);
            let e = len(sub(p, q));
            if e < best.1 {
                best = (self.arc[i] + t * (self.arc[i + 1] - self.arc[i]), e);
            }
        }
        best
    }

    /// The unit tangent at sample `i` (central differences).
    fn sample_tangent(&self, i: usize) -> V3 {
        let n = self.samples.len();
        let a = self.samples[i.saturating_sub(1)];
        let b = self.samples[(i + 1).min(n - 1)];
        unit(sub(b, a)).unwrap_or([1.0, 0.0, 0.0])
    }

    /// The rotation-minimising down vector at each sample (double reflection, Wang et al.
    /// 2008), starting from `up` projected off the first tangent and negated.
    fn rmf(&self, up: V3) -> Vec<V3> {
        let up = unit(up).unwrap_or([0.0, 0.0, 1.0]);
        let t0 = self.sample_tangent(0);
        let first = unit(sub(up, t0.map(|c| c * dot(up, t0))))
            .filter(|_| dot(up, t0).abs() < 0.999)
            .map(|u| u.map(|c| -c))
            .unwrap_or_else(|| any_perpendicular(t0));
        let mut out = Vec::with_capacity(self.samples.len());
        out.push(first);
        let mut t = t0;
        for i in 0..self.samples.len() - 1 {
            let r = out[i];
            let v1 = sub(self.samples[i + 1], self.samples[i]);
            let c1 = dot(v1, v1);
            let t1 = self.sample_tangent(i + 1);
            if c1 <= 1e-18 {
                out.push(r);
                t = t1;
                continue;
            }
            let rl = add(r, v1, -2.0 / c1 * dot(v1, r));
            let tl = add(t, v1, -2.0 / c1 * dot(v1, t));
            let v2 = sub(t1, tl);
            let c2 = dot(v2, v2);
            let next = if c2 <= 1e-18 {
                rl
            } else {
                add(rl, v2, -2.0 / c2 * dot(v2, rl))
            };
            // Kept exactly perpendicular and of unit length against rounding.
            let next = unit(sub(next, t1.map(|c| c * dot(next, t1)))).unwrap_or(r);
            out.push(next);
            t = t1;
        }
        out
    }

    /// The frames at arc lengths `at`: the down vector is rotation-minimising along the
    /// curve (no twist beyond the curve's own), starting from the vector of interest `up`
    /// projected off the first tangent and negated (image rows run down), then turned by
    /// `angle` about the tangent. A curve in a plane perpendicular to `up` keeps down = −up.
    pub fn frames(&self, up: V3, angle: f64, at: &[f64]) -> Vec<Frame> {
        let r = self.rmf(up);
        let (c, sn) = (angle.cos(), angle.sin());
        at.iter()
            .map(|&s| {
                let (i, f) = self.locate(s);
                let t = self.tangent(s);
                let mix = lerp(r[i], r[i + 1], f);
                let base = unit(sub(mix, t.map(|x| x * dot(mix, t))))
                    .unwrap_or_else(|| any_perpendicular(t));
                let side = cross(t, base);
                let down = [0, 1, 2].map(|k| base[k] * c + side[k] * sn);
                Frame {
                    point: self.at(s),
                    tangent: t,
                    down,
                    side: cross(t, down),
                }
            })
            .collect()
    }

    /// The frame at arc length `s` (as [`Spline::frames`]).
    pub fn frame_at(&self, up: V3, angle: f64, s: f64) -> Frame {
        self.frames(up, angle, &[s])[0]
    }
}

fn any_perpendicular(t: V3) -> V3 {
    let a = if t[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    unit(cross(t, a)).unwrap_or([0.0, 0.0, 1.0])
}

/// A point of the curve with its directions: the tangent, the image's down vector there
/// and `side` = tangent × down (a straightened CPR's slab direction).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub point: V3,
    pub tangent: V3,
    pub down: V3,
    pub side: V3,
}

/// How a CPR lays the curve out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CprKind {
    #[default]
    Straightened,
    Stretched,
}

impl CprKind {
    pub fn next(self) -> CprKind {
        match self {
            CprKind::Straightened => CprKind::Stretched,
            CprKind::Stretched => CprKind::Straightened,
        }
    }

    /// The message key's value (`straightened`, `stretched`).
    pub fn id(self) -> &'static str {
        match self {
            CprKind::Straightened => "straightened",
            CprKind::Stretched => "stretched",
        }
    }
}

/// How a CPR is viewed: its layout, the vector of interest and turn about the curve, the
/// scale, and the image's centre (arc length along, mm off the curve).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CprView {
    pub kind: CprKind,
    pub up: V3,
    pub angle: f64,
    /// Patient mm per target pixel.
    pub mm: f64,
    /// The arc length at the image's centre column (stretched: of the projected curve).
    pub along: f64,
    /// Mm the image is moved down the column lines.
    pub offset: f64,
}

/// One column of a CPR image: its line through the volume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    /// Arc length along the curve (stretched: of the curve point the column passes).
    pub s: f64,
    /// The patient point at the image's middle row, and the unit vectors down the column
    /// and across it (to the next column).
    pub origin: V3,
    pub down: V3,
    pub across: V3,
}

/// A CPR image laid out in a rectangle of target pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Cpr {
    pub kind: CprKind,
    pub rect: [u32; 4],
    /// The columns from the rectangle's left; `None` past the curve's ends.
    pub columns: Vec<Option<Column>>,
    pub mm: f64,
    /// The middle row's centre (target px).
    pub cy: f64,
}

/// Columns at most (a CPR wider than this is drawn at this many columns' resolution).
pub const MAX_COLUMNS: u32 = 8192;

impl Cpr {
    /// `spline` as `view` lays it out in `rect`.
    pub fn new(spline: &Spline, view: &CprView, rect: [u32; 4]) -> Cpr {
        let w = rect[2].min(MAX_COLUMNS);
        let xc = f64::from(w) / 2.0;
        let cy = f64::from(rect[1]) + f64::from(rect[3]) / 2.0;
        let mm = view.mm.max(1e-6);
        let columns = match view.kind {
            CprKind::Straightened => {
                let at: Vec<f64> = (0..w)
                    .map(|x| view.along + (f64::from(x) + 0.5 - xc) * mm)
                    .collect();
                let clamped: Vec<f64> = at.iter().map(|s| s.clamp(0.0, spline.length())).collect();
                let frames = spline.frames(view.up, view.angle, &clamped);
                at.iter()
                    .zip(&frames)
                    .map(|(&s, f)| {
                        (s >= 0.0 && s <= spline.length()).then(|| Column {
                            s,
                            origin: add(f.point, f.down, -view.offset),
                            down: f.down,
                            across: f.tangent,
                        })
                    })
                    .collect()
            }
            CprKind::Stretched => stretched(spline, view, w, xc, mm),
        };
        Cpr {
            kind: view.kind,
            rect: [rect[0], rect[1], w, rect[3]],
            columns,
            mm,
            cy,
        }
    }

    /// The planes the GPU draws: one per column, thickness `thickness` (mm, across the
    /// image: `across × down`) combined as `slab`; columns past the curve's ends show the
    /// canvas.
    pub fn planes(
        &self,
        thickness: f64,
        slab: Slab,
        window: Window,
        canvas: [f32; 4],
    ) -> Vec<Plane> {
        let mut out = Vec::with_capacity(self.columns.len());
        let mut gap: Option<(u32, u32)> = None;
        let outside = |x0: u32, n: u32| Plane {
            rect: [self.rect[0] + x0, self.rect[1], n, self.rect[3]],
            origin: [1e7; 3],
            du: [0.0, 0.0, 0.0],
            dv: [0.0, 0.0, 0.0],
            thickness: 0.0,
            slab: Slab::Average,
            window,
            canvas,
        };
        for (x, c) in self.columns.iter().enumerate() {
            let x = x as u32;
            let Some(c) = c else {
                gap = Some(gap.map_or((x, 1), |(a, n)| (a, n + 1)));
                continue;
            };
            if let Some((a, n)) = gap.take() {
                out.push(outside(a, n));
            }
            let px = f64::from(self.rect[0] + x) + 0.5;
            let du = c.across.map(|v| v * self.mm);
            let dv = c.down.map(|v| v * self.mm);
            let origin = [0, 1, 2].map(|k| c.origin[k] - du[k] * px - dv[k] * self.cy);
            out.push(Plane {
                rect: [self.rect[0] + x, self.rect[1], 1, self.rect[3]],
                origin,
                du,
                dv,
                thickness,
                slab: if thickness > 0.0 { slab } else { Slab::Average },
                window,
                canvas,
            });
        }
        if let Some((a, n)) = gap {
            out.push(outside(a, n));
        }
        out
    }

    /// The column under target x (px), if the curve reaches it.
    pub fn column(&self, x: f64) -> Option<&Column> {
        let i = (x - f64::from(self.rect[0])).floor();
        if i < 0.0 {
            return None;
        }
        self.columns.get(i as usize)?.as_ref()
    }

    /// The patient point at target pixel position (`x`, `y`).
    pub fn to_patient(&self, x: f64, y: f64) -> Option<V3> {
        let c = self.column(x)?;
        Some(add(c.origin, c.down, (y - self.cy) * self.mm))
    }

    /// Where patient point `p` shows (target px): at the column of its nearest curve point,
    /// its distance along that column's line. `None` when that column is not shown.
    pub fn to_image(&self, spline: &Spline, p: V3) -> Option<(f64, f64)> {
        let (s, _) = spline.closest(p);
        let x = self.x_of(s)?;
        let c = self.column(x)?;
        Some((x, self.cy + dot(sub(p, c.origin), c.down) / self.mm))
    }

    /// The target x of the column showing arc length `s`, if shown.
    pub fn x_of(&self, s: f64) -> Option<f64> {
        // Columns are in increasing s (straightened) or increasing projected arc, which
        // grows with s as well.
        let mut best: Option<(usize, f64)> = None;
        for (i, c) in self.columns.iter().enumerate() {
            if let Some(c) = c {
                let d = (c.s - s).abs();
                if best.is_none_or(|(_, b)| d < b) {
                    best = Some((i, d));
                }
            }
        }
        let (i, d) = best?;
        (d <= 2.0 * self.mm.max(1e-3) + 1e-9).then(|| f64::from(self.rect[0]) + i as f64 + 0.5)
    }
}

/// The stretched layout's columns: arc length of the curve projected off `up` across, the
/// column lines along −`up`, the curve's mean height (along `up`) at the middle row.
fn stretched(spline: &Spline, view: &CprView, w: u32, xc: f64, mm: f64) -> Vec<Option<Column>> {
    let up = unit(view.up).unwrap_or([0.0, 0.0, 1.0]);
    let pts = spline.samples();
    let flat = |p: V3| add(p, up, -dot(p, up));
    let mut q = Vec::with_capacity(pts.len());
    let mut acc = 0.0;
    q.push(0.0);
    for w in pts.windows(2) {
        acc += len(sub(flat(w[1]), flat(w[0])));
        q.push(acc);
    }
    let href = pts.iter().map(|p| dot(*p, up)).sum::<f64>() / pts.len() as f64;
    let down = up.map(|c| -c);
    (0..w)
        .map(|x| {
            let at = view.along + (f64::from(x) + 0.5 - xc) * mm;
            if at < 0.0 || at > acc || pts.len() < 2 {
                return None;
            }
            let i = q.partition_point(|&a| a <= at).clamp(1, q.len() - 1) - 1;
            let d = q[i + 1] - q[i];
            let f = if d > 0.0 { (at - q[i]) / d } else { 0.0 };
            let p = lerp(pts[i], pts[i + 1], f);
            let s = {
                let (a, b) = (spline.arc[i], spline.arc[i + 1]);
                a + (b - a) * f
            };
            let across =
                unit(sub(flat(pts[i + 1]), flat(pts[i]))).unwrap_or_else(|| any_perpendicular(up));
            Some(Column {
                s,
                origin: add(flat(p), up, href + view.offset),
                down,
                across,
            })
        })
        .collect()
}

impl CprView {
    /// The view that fits the whole curve into `rect` (target px), straightened, with
    /// `up` the vector of interest.
    pub fn fit(spline: &Spline, up: V3, kind: CprKind, rect: [u32; 4]) -> CprView {
        let up = unit(up).unwrap_or([0.0, 0.0, 1.0]);
        let along = match kind {
            CprKind::Straightened => spline.length(),
            CprKind::Stretched => {
                let flat = |p: V3| add(p, up, -dot(p, up));
                spline
                    .samples()
                    .windows(2)
                    .map(|w| len(sub(flat(w[1]), flat(w[0]))))
                    .sum()
            }
        };
        let mm = (along * 1.08 / f64::from(rect[2].max(1))).max(1e-3);
        CprView {
            kind,
            up,
            angle: 0.0,
            mm,
            along: along / 2.0,
            offset: 0.0,
        }
    }
}

/// The plane perpendicular to the curve at arc length `s`, centred on the curve in `rect`
/// (target px), `mm` per pixel: its down vector is the CPR's there, its right vector
/// down × tangent, so it looks along the curve.
pub fn cross_section(
    spline: &Spline,
    up: V3,
    angle: f64,
    s: f64,
    rect: [u32; 4],
    mm: f64,
    window: Window,
) -> Plane {
    let f = spline.frame_at(up, angle, s);
    Plane::centred(rect, f.point, cross(f.down, f.tangent), f.down, mm, window)
}

#[cfg(test)]
#[path = "curve_tests.rs"]
pub(crate) mod tests;
