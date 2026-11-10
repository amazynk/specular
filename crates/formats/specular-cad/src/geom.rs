//! Geometry helpers: the OCS arbitrary axis algorithm, elliptical arcs as cubics, polyline
//! bulges, and B-splines (exact Bézier extraction by knot insertion, sampling otherwise).

use kurbo::{Affine, BezPath, Point, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl V3 {
    pub const Z: V3 = V3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    pub fn new(x: f64, y: f64, z: f64) -> V3 {
        V3 { x, y, z }
    }

    pub fn cross(self, o: V3) -> V3 {
        V3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn len(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn norm(self) -> Option<V3> {
        let l = self.len();
        (l > 1e-12 && l.is_finite()).then(|| V3::new(self.x / l, self.y / l, self.z / l))
    }

    pub fn xy(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}

/// A 3D affine map: rows x', y', z' = [a, b, c, d] · (x, y, z, 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct M3(pub [[f64; 4]; 3]);

impl M3 {
    pub const IDENTITY: M3 = M3([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);

    pub fn translate(v: V3) -> M3 {
        M3([
            [1.0, 0.0, 0.0, v.x],
            [0.0, 1.0, 0.0, v.y],
            [0.0, 0.0, 1.0, v.z],
        ])
    }

    pub fn scale(x: f64, y: f64, z: f64) -> M3 {
        M3([[x, 0.0, 0.0, 0.0], [0.0, y, 0.0, 0.0], [0.0, 0.0, z, 0.0]])
    }

    /// A plan-view affine map lifted to 3D (z kept).
    pub fn from_affine(a: Affine) -> M3 {
        let c = a.as_coeffs();
        M3([
            [c[0], c[2], 0.0, c[4]],
            [c[1], c[3], 0.0, c[5]],
            [0.0, 0.0, 1.0, 0.0],
        ])
    }

    /// The OCS of extrusion `n` (with its origin raised to `elev` along `n`) → WCS.
    pub fn ocs(n: V3, elev: f64) -> M3 {
        let n = n.norm().unwrap_or(V3::Z);
        let (ax, ay) = if n.x.abs() < 1e-12 && n.y.abs() < 1e-12 && n.z > 0.0 {
            (V3::new(1.0, 0.0, 0.0), V3::new(0.0, 1.0, 0.0))
        } else {
            ocs_axes(n)
        };
        M3([
            [ax.x, ay.x, n.x, n.x * elev],
            [ax.y, ay.y, n.y, n.y * elev],
            [ax.z, ay.z, n.z, n.z * elev],
        ])
    }

    /// The plan view of the z = 0 plane under this map.
    pub fn plan(&self) -> Affine {
        let m = &self.0;
        Affine::new([m[0][0], m[1][0], m[0][1], m[1][1], m[0][3], m[1][3]])
    }

    /// Whether the plan view ignores z, so `plan` maps every plane alike.
    pub fn is_flat(&self) -> bool {
        self.0[0][2] == 0.0 && self.0[1][2] == 0.0
    }
}

impl std::ops::Mul for M3 {
    type Output = M3;

    fn mul(self, o: M3) -> M3 {
        let (a, b) = (&self.0, &o.0);
        let mut r = [[0.0; 4]; 3];
        for i in 0..3 {
            for j in 0..4 {
                r[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
            }
            r[i][3] += a[i][3];
        }
        M3(r)
    }
}

/// Object coordinate system → plan view (WCS x/y) for extrusion `n` and elevation `elev`.
pub(crate) fn ocs(n: V3, elev: f64) -> Affine {
    let Some(n) = n.norm() else {
        return Affine::translate((0.0, 0.0));
    };
    if (n.x - 0.0).abs() < 1e-12 && n.y.abs() < 1e-12 && n.z > 0.0 {
        return Affine::IDENTITY;
    }
    let (ax, ay) = ocs_axes(n);
    Affine::new([ax.x, ax.y, ay.x, ay.y, elev * n.x, elev * n.y])
}

/// The OCS x and y axes (in WCS) for unit extrusion `n`.
pub(crate) fn ocs_axes(n: V3) -> (V3, V3) {
    let ax = if n.x.abs() < 1.0 / 64.0 && n.y.abs() < 1.0 / 64.0 {
        V3::new(0.0, 1.0, 0.0).cross(n)
    } else {
        V3::Z.cross(n)
    };
    let ax = ax.norm().unwrap_or(V3::new(1.0, 0.0, 0.0));
    let ay = n.cross(ax).norm().unwrap_or(V3::new(0.0, 1.0, 0.0));
    (ax, ay)
}

/// Appends an elliptical arc `c + u cos t + v sin t` for t from `t0` over `sweep` radians
/// (negative clockwise) as cubics, starting with a move or a line to its first point.
pub(crate) fn ellipse_arc(
    path: &mut BezPath,
    c: Point,
    u: Vec2,
    v: Vec2,
    t0: f64,
    sweep: f64,
    move_to: bool,
) {
    if !(t0.is_finite() && sweep.is_finite()) {
        return;
    }
    let at = |t: f64| c + u * t.cos() + v * t.sin();
    let d = |t: f64| -u * t.sin() + v * t.cos();
    let p0 = at(t0);
    if move_to {
        path.move_to(p0);
    } else {
        path.line_to(p0);
    }
    let n = ((sweep.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).clamp(1, 16);
    let dt = sweep / n as f64;
    let k = 4.0 / 3.0 * (dt / 4.0).tan();
    for i in 0..n {
        let a = t0 + dt * i as f64;
        let b = a + dt;
        let pa = at(a);
        let pb = at(b);
        path.curve_to(pa + d(a) * k, pb - d(b) * k, pb);
    }
}

/// A full ellipse/circle as a closed path.
pub(crate) fn ellipse(c: Point, u: Vec2, v: Vec2) -> BezPath {
    let mut p = BezPath::new();
    ellipse_arc(&mut p, c, u, v, 0.0, std::f64::consts::TAU, true);
    p.close_path();
    p
}

/// Appends the segment from `p0` to `p1` with `bulge` (tan of a quarter of the included
/// angle, positive counter-clockwise); the path is already at `p0`.
pub(crate) fn bulge_to(path: &mut BezPath, p0: Point, p1: Point, bulge: f64) {
    if bulge.abs() < 1e-10 || !bulge.is_finite() || p0.distance(p1) < 1e-12 {
        path.line_to(p1);
        return;
    }
    let (c, r, a0, sweep) = bulge_arc(p0, p1, bulge);
    // Draw from the exact start so rounding never leaves a gap.
    let u = Vec2::new(r, 0.0);
    let v = Vec2::new(0.0, r);
    let n = ((sweep.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).clamp(1, 8);
    let dt = sweep / n as f64;
    let k = 4.0 / 3.0 * (dt / 4.0).tan();
    let at = |t: f64| c + u * t.cos() + v * t.sin();
    let d = |t: f64| -u * t.sin() + v * t.cos();
    for i in 0..n {
        let a = a0 + dt * i as f64;
        let b = a + dt;
        let pa = if i == 0 { p0 } else { at(a) };
        let pb = if i + 1 == n { p1 } else { at(b) };
        path.curve_to(pa + d(a) * k, pb - d(b) * k, pb);
    }
}

/// Centre, radius, start angle and signed sweep of a bulge segment.
pub(crate) fn bulge_arc(p0: Point, p1: Point, bulge: f64) -> (Point, f64, f64, f64) {
    let chord = p1 - p0;
    let len = chord.hypot();
    let sweep = 4.0 * bulge.atan();
    let mid = p0.midpoint(p1);
    let left = Vec2::new(-chord.y, chord.x) / len;
    let off = len / 2.0 * (1.0 - bulge * bulge) / (2.0 * bulge);
    let c = mid + left * off;
    let r = len * (1.0 + bulge * bulge) / (4.0 * bulge.abs());
    let a0 = (p0 - c).atan2();
    (c, r, a0, sweep)
}

/// A point on the polyline built from `pts` with bulges, sampled, with the arc's points for
/// width outlines: returns (point, parameter 0..1 along the segment).
pub(crate) fn bulge_samples(p0: Point, p1: Point, bulge: f64, n: usize) -> Vec<(Point, f64)> {
    if bulge.abs() < 1e-10 || !bulge.is_finite() || p0.distance(p1) < 1e-12 {
        return vec![(p0, 0.0), (p1, 1.0)];
    }
    let (c, r, a0, sweep) = bulge_arc(p0, p1, bulge);
    (0..=n)
        .map(|i| {
            let f = i as f64 / n as f64;
            let t = a0 + sweep * f;
            (c + Vec2::new(r * t.cos(), r * t.sin()), f)
        })
        .collect()
}

/// A B-spline: degree, knots, control points (x, y) and weights (empty when non-rational).
pub(crate) struct Spline {
    pub degree: usize,
    pub knots: Vec<f64>,
    pub ctrl: Vec<Point>,
    pub weights: Vec<f64>,
}

impl Spline {
    fn valid(&self) -> bool {
        let p = self.degree;
        p >= 1
            && self.ctrl.len() > p
            && self.knots.len() == self.ctrl.len() + p + 1
            && self.knots.windows(2).all(|w| w[1] >= w[0] - 1e-12)
            && self.knots[p] < self.knots[self.ctrl.len()]
    }

    fn rational(&self) -> bool {
        self.weights.len() == self.ctrl.len()
            && self
                .weights
                .iter()
                .any(|w| (w - self.weights[0]).abs() > 1e-12)
    }

    /// The curve as a path: exact Bézier segments for non-rational degree ≤ 3, sampled
    /// otherwise (at most `max_points`). `None` when the definition is inconsistent.
    pub fn to_path(&self, max_points: usize) -> Option<BezPath> {
        if !self.valid() {
            return None;
        }
        if self.degree <= 3 && !self.rational() {
            return self.bezier();
        }
        self.sampled(max_points)
    }

    fn bezier(&self) -> Option<BezPath> {
        let p = self.degree;
        let mut knots = self.knots.clone();
        let mut pts = self.ctrl.clone();
        let lo = knots[p];
        let hi = knots[pts.len()];
        // Every distinct knot in the domain gets multiplicity ≥ p.
        let mut distinct: Vec<f64> = Vec::new();
        for &k in &knots {
            if k >= lo && k <= hi && distinct.last().is_none_or(|&l| k > l) {
                distinct.push(k);
            }
        }
        if distinct.len() > 100_000 {
            return None;
        }
        for &u in &distinct {
            let mult = knots.iter().filter(|&&k| k == u).count();
            for _ in mult..p {
                insert_knot(&mut knots, &mut pts, p, u);
            }
        }
        let mut path = BezPath::new();
        let n = pts.len();
        let mut first = true;
        for k in p..n {
            if knots[k] < knots[k + 1] && knots[k] >= lo && knots[k + 1] <= hi {
                let seg = &pts[k - p..=k];
                if first {
                    path.move_to(seg[0]);
                    first = false;
                }
                match p {
                    1 => path.line_to(seg[1]),
                    2 => path.quad_to(seg[1], seg[2]),
                    _ => path.curve_to(seg[1], seg[2], seg[3]),
                }
            }
        }
        (!first).then_some(path)
    }

    fn sampled(&self, max_points: usize) -> Option<BezPath> {
        let p = self.degree;
        let n = self.ctrl.len();
        let lo = self.knots[p];
        let hi = self.knots[n];
        let spans = self.knots[p..=n].windows(2).filter(|w| w[1] > w[0]).count();
        let count = (spans * 16).clamp(16, max_points.max(16));
        let w = |i: usize| {
            if self.weights.len() == n {
                self.weights[i]
            } else {
                1.0
            }
        };
        let mut path = BezPath::new();
        for i in 0..=count {
            let u = lo + (hi - lo) * i as f64 / count as f64;
            let pt = self.eval(u, &w);
            if i == 0 {
                path.move_to(pt);
            } else {
                path.line_to(pt);
            }
        }
        Some(path)
    }

    /// De Boor in homogeneous coordinates.
    fn eval(&self, u: f64, w: &dyn Fn(usize) -> f64) -> Point {
        let p = self.degree;
        let n = self.ctrl.len();
        let mut k = p;
        while k + 1 < n && self.knots[k + 1] <= u {
            k += 1;
        }
        let mut d: Vec<(f64, f64, f64)> = (0..=p)
            .map(|j| {
                let i = k - p + j;
                let wi = w(i);
                (self.ctrl[i].x * wi, self.ctrl[i].y * wi, wi)
            })
            .collect();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = k - p + j;
                let den = self.knots[i + p + 1 - r] - self.knots[i];
                let a = if den.abs() < 1e-300 {
                    0.0
                } else {
                    (u - self.knots[i]) / den
                };
                d[j] = (
                    (1.0 - a) * d[j - 1].0 + a * d[j].0,
                    (1.0 - a) * d[j - 1].1 + a * d[j].1,
                    (1.0 - a) * d[j - 1].2 + a * d[j].2,
                );
            }
        }
        let (x, y, ww) = d[p];
        if ww.abs() < 1e-300 {
            Point::new(x, y)
        } else {
            Point::new(x / ww, y / ww)
        }
    }
}

/// Boehm's knot insertion of `u` once.
fn insert_knot(knots: &mut Vec<f64>, pts: &mut Vec<Point>, p: usize, u: f64) {
    let n = pts.len();
    // Span k with knots[k] <= u < knots[k+1], within p..n-1 (u == the last knot uses n-1).
    let mut k = p;
    while k + 1 < n && knots[k + 1] <= u {
        k += 1;
    }
    let mut q = Vec::with_capacity(n + 1);
    for i in 0..=n {
        if i + p <= k {
            q.push(pts[i]);
        } else if i > k {
            q.push(pts[i - 1]);
        } else {
            let den = knots[i + p] - knots[i];
            let a = if den.abs() < 1e-300 {
                0.0
            } else {
                (u - knots[i]) / den
            };
            q.push(pts[i - 1].lerp(pts[i], a));
        }
    }
    knots.insert(k + 1, u);
    *pts = q;
}

/// A smooth curve through `pts` (Catmull-Rom as cubics), with optional end tangents.
pub(crate) fn through_points(pts: &[Point], t0: Option<Vec2>, t1: Option<Vec2>) -> BezPath {
    let mut path = BezPath::new();
    if pts.is_empty() {
        return path;
    }
    path.move_to(pts[0]);
    let n = pts.len();
    if n == 2 {
        path.line_to(pts[1]);
        return path;
    }
    let tangent = |i: usize| -> Vec2 {
        if i == 0 {
            if let Some(t) = t0.and_then(unit) {
                return t * (pts[1] - pts[0]).hypot();
            }
            pts[1] - pts[0]
        } else if i == n - 1 {
            if let Some(t) = t1.and_then(unit) {
                return t * (pts[n - 1] - pts[n - 2]).hypot();
            }
            pts[n - 1] - pts[n - 2]
        } else {
            (pts[i + 1] - pts[i - 1]) * 0.5
        }
    };
    for i in 0..n - 1 {
        let a = pts[i];
        let b = pts[i + 1];
        path.curve_to(a + tangent(i) / 3.0, b - tangent(i + 1) / 3.0, b);
    }
    path
}

fn unit(v: Vec2) -> Option<Vec2> {
    let l = v.hypot();
    (l > 1e-12 && l.is_finite()).then(|| v / l)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::PathEl;

    #[test]
    fn ocs_negative_z_mirrors_x() {
        let a = ocs(V3::new(0.0, 0.0, -1.0), 0.0);
        let p = a * Point::new(2.0, 3.0);
        assert!((p.x + 2.0).abs() < 1e-12 && (p.y - 3.0).abs() < 1e-12);
    }

    #[test]
    fn clamped_cubic_is_one_bezier() {
        let s = Spline {
            degree: 3,
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            ctrl: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 2.0),
                Point::new(3.0, 2.0),
                Point::new(4.0, 0.0),
            ],
            weights: vec![],
        };
        let p = s.to_path(1000).unwrap();
        let els: Vec<PathEl> = p.elements().to_vec();
        assert_eq!(els.len(), 2);
        assert_eq!(
            els[1],
            PathEl::CurveTo(
                Point::new(1.0, 2.0),
                Point::new(3.0, 2.0),
                Point::new(4.0, 0.0)
            )
        );
    }

    #[test]
    fn knot_insertion_matches_de_boor() {
        let s = Spline {
            degree: 3,
            knots: vec![0.0, 0.0, 0.0, 0.0, 0.3, 0.7, 1.0, 1.0, 1.0, 1.0],
            ctrl: (0..6)
                .map(|i| Point::new(i as f64, (i * i % 5) as f64))
                .collect(),
            weights: vec![],
        };
        let path = s.to_path(1000).unwrap();
        let segs: Vec<_> = path.segments().collect();
        assert_eq!(segs.len(), 3);
        use kurbo::ParamCurve;
        let w = |_| 1.0;
        let pairs = [(0usize, 0.0, 0.3), (1, 0.3, 0.7), (2, 0.7, 1.0)];
        for (i, a, b) in pairs {
            for t in [0.0, 0.25, 0.5, 0.9] {
                let u = a + (b - a) * t;
                let e = s.eval(u, &w);
                let q = segs[i].eval(t);
                assert!(e.distance(q) < 1e-9, "{i} {t}: {e:?} {q:?}");
            }
        }
    }

    #[test]
    fn bulge_half_circle() {
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        bulge_to(&mut p, Point::new(2.0, 0.0), Point::new(2.0, 0.0), 1.0);
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        bulge_to(&mut p, Point::new(0.0, 0.0), Point::new(2.0, 0.0), 1.0);
        let bb = kurbo::Shape::bounding_box(&p);
        // Counter-clockwise from (0,0) to (2,0) goes below the chord.
        assert!((bb.y0 + 1.0).abs() < 1e-3, "{bb:?}");
    }
}
