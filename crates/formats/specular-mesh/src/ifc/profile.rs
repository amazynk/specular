//! Placements, curves and profiles: IFC's coordinate systems as maps, curves as point runs
//! (lines and polylines as given, circles, ellipses and B-splines sampled), and profile
//! definitions as plane regions (an outer ring and holes) in the profile's own plane.

use std::f64::consts::{PI, TAU};

use super::Ctx;
use super::geom::{Mat, P2, P3, add, cross, dedup_ring, len, ring_area, scale, sub, unit};
use super::step::V;

/// A plane region: an outer ring and the holes in it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Region {
    pub outer: Vec<P2>,
    pub holes: Vec<Vec<P2>>,
}

/// Deepest nesting of curves in curves and profiles in profiles.
const MAX_DEPTH: usize = 32;

impl Ctx<'_> {
    pub fn point(&self, id: u32) -> Option<P3> {
        let (p, n) = self.s.coords(id)?;
        (n >= 2 && p.iter().all(|v| v.is_finite())).then_some(p)
    }

    pub fn direction(&self, id: u32) -> Option<P3> {
        let (p, n) = self.s.coords(id)?;
        if n < 2 {
            return None;
        }
        unit(p)
    }

    /// An `IfcAxis2Placement3D` or `…2D` (or `$`): the map from its frame.
    pub fn placement(&self, v: &V) -> Mat {
        match v.as_ref() {
            Some(id) => self.placement_id(id),
            None => Mat::IDENTITY,
        }
    }

    pub fn placement_id(&self, id: u32) -> Mat {
        let Some(a) = self.s.args(id) else {
            return Mat::IDENTITY;
        };
        let o = a
            .first()
            .and_then(V::as_ref)
            .and_then(|p| self.point(p))
            .unwrap_or([0.0; 3]);
        match self.s.ty(id) {
            Some("IFCAXIS2PLACEMENT2D") => {
                let x = a
                    .get(1)
                    .and_then(V::as_ref)
                    .and_then(|d| self.direction(d))
                    .map(|d| unit([d[0], d[1], 0.0]).unwrap_or([1.0, 0.0, 0.0]))
                    .unwrap_or([1.0, 0.0, 0.0]);
                Mat::from_axes(x, [-x[1], x[0], 0.0], [0.0, 0.0, 1.0], o)
            }
            Some("IFCAXIS2PLACEMENT3D") => {
                let z = a
                    .get(1)
                    .and_then(V::as_ref)
                    .and_then(|d| self.direction(d))
                    .unwrap_or([0.0, 0.0, 1.0]);
                let r = a
                    .get(2)
                    .and_then(V::as_ref)
                    .and_then(|d| self.direction(d))
                    .unwrap_or(if z[0].abs() < 0.9 {
                        [1.0, 0.0, 0.0]
                    } else {
                        [0.0, 1.0, 0.0]
                    });
                let x = unit(sub(r, scale(z, super::geom::dot(r, z))))
                    .or_else(|| super::geom::plane_basis(z).0.into())
                    .unwrap_or([1.0, 0.0, 0.0]);
                let y = cross(z, x);
                Mat::from_axes(x, y, z, o)
            }
            Some("IFCAXIS1PLACEMENT") => {
                let z = a
                    .get(1)
                    .and_then(V::as_ref)
                    .and_then(|d| self.direction(d))
                    .unwrap_or([0.0, 0.0, 1.0]);
                let (x, y) = super::geom::plane_basis(z);
                Mat::from_axes(x, y, z, o)
            }
            _ => Mat::IDENTITY,
        }
    }

    /// An `IfcCartesianTransformationOperator2D/3D` (uniform or not) as a map.
    pub fn operator(&self, id: u32) -> Mat {
        let Some(a) = self.s.args(id) else {
            return Mat::IDENTITY;
        };
        let ty = self.s.ty(id).unwrap_or("");
        let dir = |k: usize| a.get(k).and_then(V::as_ref).and_then(|d| self.direction(d));
        let o = a
            .get(2)
            .and_then(V::as_ref)
            .and_then(|p| self.point(p))
            .unwrap_or([0.0; 3]);
        let s1 = a.get(3).and_then(V::as_f64).unwrap_or(1.0);
        if ty.starts_with("IFCCARTESIANTRANSFORMATIONOPERATOR2D") {
            let x = dir(0).unwrap_or([1.0, 0.0, 0.0]);
            let x = unit([x[0], x[1], 0.0]).unwrap_or([1.0, 0.0, 0.0]);
            let y = dir(1)
                .map(|d| [d[0], d[1], 0.0])
                .and_then(unit)
                .unwrap_or([-x[1], x[0], 0.0]);
            let s2 = if ty.ends_with("NONUNIFORM") {
                a.get(4).and_then(V::as_f64).unwrap_or(s1)
            } else {
                s1
            };
            return Mat::from_axes(scale(x, s1), scale(y, s2), [0.0, 0.0, 1.0], o);
        }
        let z = dir(4).unwrap_or([0.0, 0.0, 1.0]);
        let x0 = dir(0).unwrap_or(if z[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        });
        let x = unit(sub(x0, scale(z, super::geom::dot(x0, z)))).unwrap_or([1.0, 0.0, 0.0]);
        let y = match dir(1) {
            Some(y0) => {
                let y1 = sub(y0, scale(z, super::geom::dot(y0, z)));
                let y1 = sub(y1, scale(x, super::geom::dot(y1, x)));
                unit(y1).unwrap_or_else(|| cross(z, x))
            }
            None => cross(z, x),
        };
        let (s2, s3) = if ty.ends_with("NONUNIFORM") {
            (
                a.get(5).and_then(V::as_f64).unwrap_or(s1),
                a.get(6).and_then(V::as_f64).unwrap_or(s1),
            )
        } else {
            (s1, s1)
        };
        Mat::from_axes(scale(x, s1), scale(y, s2), scale(z, s3), o)
    }

    /// Points along a full turn of `angle` radians.
    pub fn arc_steps(&self, angle: f64) -> usize {
        ((angle.abs() / TAU * self.segments as f64).ceil() as usize).clamp(2, 4096)
    }

    /// Appends the points of curve `id` (2D curves with z = 0); false when the curve is of
    /// a kind not read.
    pub fn curve(&self, id: u32, out: &mut Vec<P3>, depth: usize) -> bool {
        if depth > MAX_DEPTH || !self.work.call() {
            return false;
        }
        let start = out.len();
        let ok = self.curve_inner(id, out, depth);
        // Points copied count against the file's budget (curves share segments).
        ok && self.work.copy(out.len().saturating_sub(start))
    }

    fn curve_inner(&self, id: u32, out: &mut Vec<P3>, depth: usize) -> bool {
        let Some(ty) = self.s.ty(id) else {
            return false;
        };
        let Some(a) = self.s.args(id) else {
            return false;
        };
        match ty {
            "IFCPOLYLINE" => {
                for p in a.first().map(V::refs).unwrap_or_default() {
                    if let Some(q) = self.point(p) {
                        out.push(q);
                    }
                }
                true
            }
            "IFCINDEXEDPOLYCURVE" => self.indexed_poly_curve(&a, out),
            "IFCCOMPOSITECURVE" | "IFCCOMPOSITECURVEONSURFACE" => {
                let mut ok = false;
                for seg in a.first().map(V::refs).unwrap_or_default() {
                    let Some(sa) = self.s.args(seg) else { continue };
                    // IfcCompositeCurveSegment(Transition, SameSense, ParentCurve).
                    let same = sa.get(1).and_then(V::as_bool).unwrap_or(true);
                    let Some(parent) = sa.get(2).and_then(V::as_ref) else {
                        continue;
                    };
                    let mut pts = Vec::new();
                    if !self.curve(parent, &mut pts, depth + 1) {
                        continue;
                    }
                    if !same {
                        pts.reverse();
                    }
                    if let (Some(last), Some(first)) = (out.last(), pts.first())
                        && len(sub(*last, *first)) <= self.eps
                    {
                        pts.remove(0);
                    }
                    out.extend(pts);
                    ok = true;
                }
                ok
            }
            "IFCTRIMMEDCURVE" => self.trimmed_curve(&a, out, depth),
            "IFCCIRCLE" | "IFCELLIPSE" => {
                let m = self.placement(a.first().unwrap_or(&V::Null));
                let r1 = a.get(1).and_then(V::as_f64).unwrap_or(0.0);
                let r2 = if ty == "IFCELLIPSE" {
                    a.get(2).and_then(V::as_f64).unwrap_or(r1)
                } else {
                    r1
                };
                if !(r1 > 0.0 && r2 > 0.0) {
                    return false;
                }
                let n = self.arc_steps(TAU).max(8);
                for k in 0..n {
                    let t = TAU * k as f64 / n as f64;
                    out.push(m.point([r1 * t.cos(), r2 * t.sin(), 0.0]));
                }
                true
            }
            "IFCBSPLINECURVEWITHKNOTS" | "IFCRATIONALBSPLINECURVEWITHKNOTS" => {
                self.bspline_curve(&a, ty.starts_with("IFCRATIONAL"), out)
            }
            "IFCLINE" => false,
            _ => false,
        }
    }

    fn indexed_poly_curve(&self, a: &[V], out: &mut Vec<P3>) -> bool {
        let Some(list) = a.first().and_then(V::as_ref) else {
            return false;
        };
        let pts = self.point_list(list);
        if pts.is_empty() {
            return false;
        }
        let get = |i: i64| pts.get((i - 1).max(0) as usize).copied();
        let segs = a.get(1).and_then(V::as_list);
        let push = |out: &mut Vec<P3>, p: P3| {
            if out.last().is_none_or(|l| len(sub(*l, p)) > 1e-12) {
                out.push(p);
            }
        };
        match segs {
            Some(segs) if !segs.is_empty() => {
                for seg in segs {
                    let V::Typed(name, inner) = seg else { continue };
                    let ix: Vec<i64> = inner
                        .as_list()
                        .map(|l| l.iter().filter_map(V::as_i64).collect())
                        .unwrap_or_default();
                    if name.eq_ignore_ascii_case(b"IFCARCINDEX") && ix.len() == 3 {
                        let (Some(p), Some(q), Some(r)) = (get(ix[0]), get(ix[1]), get(ix[2]))
                        else {
                            continue;
                        };
                        for x in self.arc3(p, q, r) {
                            push(out, x);
                        }
                    } else {
                        for &i in &ix {
                            if let Some(p) = get(i) {
                                push(out, p);
                            }
                        }
                    }
                }
            }
            _ => {
                for p in pts {
                    push(out, p);
                }
            }
        }
        true
    }

    /// `IfcCartesianPointList2D/3D` coordinates.
    pub fn point_list(&self, id: u32) -> Vec<P3> {
        let Some(a) = self.s.args(id) else {
            return Vec::new();
        };
        a.first()
            .and_then(V::as_list)
            .map(|l| {
                l.iter()
                    .filter_map(|p| {
                        let c = p.reals();
                        (c.len() >= 2).then(|| [c[0], c[1], c.get(2).copied().unwrap_or(0.0)])
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The arc through three points, from the first to the third.
    pub fn arc3(&self, p: P3, q: P3, r: P3) -> Vec<P3> {
        let (u, v) = (sub(q, p), sub(r, p));
        let n = cross(u, v);
        let nn = super::geom::dot(n, n);
        if nn < 1e-24 {
            return vec![p, q, r];
        }
        // Circumcentre.
        let uu = super::geom::dot(u, u);
        let vv = super::geom::dot(v, v);
        let c = add(
            p,
            scale(
                add(scale(cross(n, u), vv), scale(cross(v, n), uu)),
                1.0 / (2.0 * nn),
            ),
        );
        let rad = len(sub(p, c));
        let e1 = scale(sub(p, c), 1.0 / rad);
        let nz = unit(n).unwrap_or([0.0, 0.0, 1.0]);
        let e2 = cross(nz, e1);
        let ang = |x: P3| {
            let d = sub(x, c);
            let a = super::geom::dot(d, e2).atan2(super::geom::dot(d, e1));
            if a < 0.0 { a + TAU } else { a }
        };
        let end = ang(r);
        let steps = self.arc_steps(end);
        (0..=steps)
            .map(|k| {
                let t = end * k as f64 / steps as f64;
                add(c, add(scale(e1, rad * t.cos()), scale(e2, rad * t.sin())))
            })
            .collect()
    }

    fn trimmed_curve(&self, a: &[V], out: &mut Vec<P3>, depth: usize) -> bool {
        let Some(basis) = a.first().and_then(V::as_ref) else {
            return false;
        };
        let sense = a.get(3).and_then(V::as_bool).unwrap_or(true);
        let prefer_param = a.get(4).and_then(V::as_enum) == Some(b"PARAMETER");
        let trim = |k: usize| -> (Option<f64>, Option<P3>) {
            let mut param = None;
            let mut point = None;
            for v in a.get(k).and_then(V::as_list).unwrap_or(&[]) {
                match v {
                    V::Ref(r) => point = self.point(*r),
                    V::Typed(_, x) => param = x.as_f64(),
                    V::Real(_) | V::Int(_) => param = v.as_f64(),
                    _ => {}
                }
            }
            (param, point)
        };
        let (t1, t2) = (trim(1), trim(2));
        let bty = self.s.ty(basis).unwrap_or("");
        let Some(b) = self.s.args(basis) else {
            return false;
        };
        match bty {
            "IFCCIRCLE" | "IFCELLIPSE" => {
                let m = self.placement(b.first().unwrap_or(&V::Null));
                let r1 = b.get(1).and_then(V::as_f64).unwrap_or(0.0);
                let r2 = if bty == "IFCELLIPSE" {
                    b.get(2).and_then(V::as_f64).unwrap_or(r1)
                } else {
                    r1
                };
                if !(r1 > 0.0 && r2 > 0.0) {
                    return false;
                }
                let inv = m.inverse().unwrap_or(Mat::IDENTITY);
                let angle_of = |t: &(Option<f64>, Option<P3>)| -> Option<f64> {
                    let from_point = t.1.map(|p| {
                        let q = inv.point(p);
                        (q[1] / r2).atan2(q[0] / r1)
                    });
                    let from_param = t.0.map(|v| v * self.angle);
                    if prefer_param {
                        from_param.or(from_point)
                    } else {
                        from_point.or(from_param)
                    }
                };
                let (Some(mut s), Some(mut e)) = (angle_of(&t1), angle_of(&t2)) else {
                    return false;
                };
                if !sense {
                    std::mem::swap(&mut s, &mut e);
                }
                while e <= s + 1e-12 {
                    e += TAU;
                }
                while e - s > TAU + 1e-9 {
                    e -= TAU;
                }
                let steps = self.arc_steps(e - s);
                let mut pts: Vec<P3> = (0..=steps)
                    .map(|k| {
                        let t = s + (e - s) * k as f64 / steps as f64;
                        m.point([r1 * t.cos(), r2 * t.sin(), 0.0])
                    })
                    .collect();
                if !sense {
                    pts.reverse();
                }
                out.extend(pts);
                true
            }
            "IFCLINE" => {
                let p0 = b.first().and_then(V::as_ref).and_then(|p| self.point(p));
                let dir = b.get(1).and_then(V::as_ref).and_then(|v| {
                    let va = self.s.args(v)?;
                    let d = va
                        .first()
                        .and_then(V::as_ref)
                        .and_then(|d| self.direction(d))?;
                    let m = va.get(1).and_then(V::as_f64).unwrap_or(1.0);
                    Some(scale(d, m))
                });
                let at = |t: &(Option<f64>, Option<P3>)| -> Option<P3> {
                    let by_param = match (p0, dir, t.0) {
                        (Some(p), Some(d), Some(u)) => Some(add(p, scale(d, u))),
                        _ => None,
                    };
                    if prefer_param {
                        by_param.or(t.1)
                    } else {
                        t.1.or(by_param)
                    }
                };
                let (Some(p), Some(q)) = (at(&t1), at(&t2)) else {
                    return false;
                };
                // Along the line's direction when the sense agrees, against it when not.
                let forward = match dir {
                    Some(d) => dot3(sub(q, p), d) >= 0.0,
                    None => true,
                };
                if forward == sense {
                    out.extend([p, q]);
                } else {
                    out.extend([q, p]);
                }
                true
            }
            _ => {
                // Other bases: the whole curve between its trims is not computed; the basis
                // curve stands in.
                let mut pts = Vec::new();
                if !self.curve(basis, &mut pts, depth + 1) {
                    return false;
                }
                if !sense {
                    pts.reverse();
                }
                out.extend(pts);
                true
            }
        }
    }

    fn bspline_curve(&self, a: &[V], rational: bool, out: &mut Vec<P3>) -> bool {
        // (Degree, ControlPointsList, CurveForm, ClosedCurve, SelfIntersect,
        //  KnotMultiplicities, Knots, KnotSpec[, WeightsData])
        let deg = a.first().and_then(V::as_i64).unwrap_or(0).clamp(0, 32) as usize;
        let ctrl: Vec<P3> = a
            .get(1)
            .map(V::refs)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| self.point(p))
            .collect();
        let mults: Vec<usize> = a
            .get(5)
            .and_then(V::as_list)
            .map(|l| {
                l.iter()
                    .filter_map(V::as_i64)
                    .map(|m| m.clamp(0, 64) as usize)
                    .collect()
            })
            .unwrap_or_default();
        let knots = a.get(6).map(V::reals).unwrap_or_default();
        let weights = if rational {
            a.get(8).map(V::reals).unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut kv = Vec::new();
        for (k, m) in knots.iter().zip(&mults) {
            kv.extend(std::iter::repeat_n(*k, *m));
        }
        if deg == 0 || ctrl.len() <= deg || kv.len() != ctrl.len() + deg + 1 {
            out.extend(ctrl);
            return true;
        }
        let w = |i: usize| weights.get(i).copied().unwrap_or(1.0);
        let (t0, t1) = (kv[deg], kv[ctrl.len()]);
        let n = (ctrl.len() * 4).clamp(16, 512);
        for s in 0..=n {
            let t = t0 + (t1 - t0) * s as f64 / n as f64;
            // The knot span.
            let mut k = deg;
            while k < ctrl.len() - 1 && t >= kv[k + 1] {
                k += 1;
            }
            // de Boor in homogeneous coordinates.
            let mut d: Vec<[f64; 4]> = (0..=deg)
                .map(|j| {
                    let i = k - deg + j;
                    let p = ctrl[i];
                    let wi = w(i);
                    [p[0] * wi, p[1] * wi, p[2] * wi, wi]
                })
                .collect();
            for r in 1..=deg {
                for j in (r..=deg).rev() {
                    let i = k - deg + j;
                    let den = kv[i + deg + 1 - r] - kv[i];
                    let alpha = if den.abs() < 1e-300 {
                        0.0
                    } else {
                        (t - kv[i]) / den
                    };
                    let prev = d[j - 1];
                    for (c, v) in d[j].iter_mut().enumerate() {
                        *v = (1.0 - alpha) * prev[c] + alpha * *v;
                    }
                }
            }
            let h = d[deg];
            if h[3].abs() > 1e-300 {
                out.push([h[0] / h[3], h[1] / h[3], h[2] / h[3]]);
            }
        }
        true
    }

    /// A closed curve as a plane ring (its x and y).
    pub fn ring(&self, id: u32) -> Option<Vec<P2>> {
        let mut pts = Vec::new();
        if !self.curve(id, &mut pts, 0) {
            return None;
        }
        let mut r: Vec<P2> = pts.into_iter().map(|p| [p[0], p[1]]).collect();
        dedup_ring(&mut r, self.eps * 1e-3);
        (r.len() >= 3).then_some(r)
    }

    /// The regions of profile `id`, in the plane of whatever places it.
    pub fn profile(&self, id: u32, depth: usize) -> Option<Vec<Region>> {
        if depth > MAX_DEPTH || !self.work.call() {
            return None;
        }
        let ty = self.s.ty(id)?;
        let a = self.s.args(id)?;
        let f = |k: usize| a.get(k).and_then(V::as_f64);
        let pos = || {
            let m = self.placement(a.get(2).unwrap_or(&V::Null));
            move |r: Vec<Region>| place_regions(r, &m)
        };
        let seg = self.segments;
        let regions = match ty {
            "IFCARBITRARYCLOSEDPROFILEDEF" => {
                let outer = self.ring(a.get(2)?.as_ref()?)?;
                vec![Region {
                    outer,
                    holes: Vec::new(),
                }]
            }
            "IFCARBITRARYPROFILEDEFWITHVOIDS" => {
                let outer = self.ring(a.get(2)?.as_ref()?)?;
                let holes = a
                    .get(3)
                    .map(V::refs)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|c| self.ring(c))
                    .collect();
                vec![Region { outer, holes }]
            }
            "IFCCENTERLINEPROFILEDEF" => {
                let mut pts = Vec::new();
                if !self.curve(a.get(2)?.as_ref()?, &mut pts, 0) {
                    return None;
                }
                let t = f(3)?;
                let line: Vec<P2> = pts.into_iter().map(|p| [p[0], p[1]]).collect();
                vec![Region {
                    outer: thicken(&line, t)?,
                    holes: Vec::new(),
                }]
            }
            "IFCRECTANGLEPROFILEDEF" => pos()(vec![Region {
                outer: rounded_rect(f(3)?, f(4)?, 0.0, seg),
                holes: Vec::new(),
            }]),
            "IFCROUNDEDRECTANGLEPROFILEDEF" => pos()(vec![Region {
                outer: rounded_rect(f(3)?, f(4)?, f(5).unwrap_or(0.0), seg),
                holes: Vec::new(),
            }]),
            "IFCRECTANGLEHOLLOWPROFILEDEF" => {
                let (w, h, t) = (f(3)?, f(4)?, f(5)?);
                let inner_r = f(6).unwrap_or(0.0);
                let outer_r = f(7).unwrap_or(0.0);
                let mut holes = Vec::new();
                if w > 2.0 * t && h > 2.0 * t {
                    holes.push(rounded_rect(w - 2.0 * t, h - 2.0 * t, inner_r, seg));
                }
                pos()(vec![Region {
                    outer: rounded_rect(w, h, outer_r, seg),
                    holes,
                }])
            }
            "IFCCIRCLEPROFILEDEF" => pos()(vec![Region {
                outer: ellipse(f(3)?, f(3)?, seg),
                holes: Vec::new(),
            }]),
            "IFCCIRCLEHOLLOWPROFILEDEF" => {
                let (r, t) = (f(3)?, f(4)?);
                let holes = if r > t {
                    vec![ellipse(r - t, r - t, seg)]
                } else {
                    Vec::new()
                };
                pos()(vec![Region {
                    outer: ellipse(r, r, seg),
                    holes,
                }])
            }
            "IFCELLIPSEPROFILEDEF" => pos()(vec![Region {
                outer: ellipse(f(3)?, f(4)?, seg),
                holes: Vec::new(),
            }]),
            "IFCISHAPEPROFILEDEF" => {
                let (b, h, tw, tf) = (f(3)?, f(4)?, f(5)?, f(6)?);
                let r = f(7).unwrap_or(0.0);
                pos()(vec![Region {
                    outer: i_shape(b, b, h, tw, tf, tf, r, r, seg),
                    holes: Vec::new(),
                }])
            }
            "IFCASYMMETRICISHAPEPROFILEDEF" => {
                // (…, Position, BottomFlangeWidth, OverallDepth, WebThickness,
                //  BottomFlangeThickness, BottomFlangeFilletRadius, TopFlangeWidth,
                //  TopFlangeThickness, TopFlangeFilletRadius, …)
                let (bb, h, tw, tfb) = (f(3)?, f(4)?, f(5)?, f(6)?);
                let rb = f(7).unwrap_or(0.0);
                let bt = f(8).unwrap_or(bb);
                let tft = f(9).unwrap_or(tfb);
                let rt = f(10).unwrap_or(rb);
                pos()(vec![Region {
                    outer: i_shape(bb, bt, h, tw, tfb, tft, rb, rt, seg),
                    holes: Vec::new(),
                }])
            }
            "IFCLSHAPEPROFILEDEF" => {
                let h = f(3)?;
                let b = f(4).unwrap_or(h);
                let t = f(5)?;
                let r = f(6).unwrap_or(0.0);
                let e = f(7).unwrap_or(0.0);
                let (x0, y0) = (-b / 2.0, -h / 2.0);
                let pts = vec![
                    [x0, y0],
                    [x0 + b, y0],
                    [x0 + b, y0 + t],
                    [x0 + t, y0 + t],
                    [x0 + t, y0 + h],
                    [x0, y0 + h],
                ];
                let radii = [0.0, 0.0, e, r, e, 0.0];
                pos()(vec![Region {
                    outer: fillet(&pts, &radii, seg),
                    holes: Vec::new(),
                }])
            }
            "IFCTSHAPEPROFILEDEF" => {
                let (h, b, tw, tf) = (f(3)?, f(4)?, f(5)?, f(6)?);
                let r = f(7).unwrap_or(0.0);
                let fe = f(8).unwrap_or(0.0);
                let we = f(9).unwrap_or(0.0);
                let (hh, hb, ht) = (h / 2.0, b / 2.0, tw / 2.0);
                let pts = vec![
                    [-ht, -hh],
                    [ht, -hh],
                    [ht, hh - tf],
                    [hb, hh - tf],
                    [hb, hh],
                    [-hb, hh],
                    [-hb, hh - tf],
                    [-ht, hh - tf],
                ];
                let radii = [we, we, r, fe, 0.0, 0.0, fe, r];
                pos()(vec![Region {
                    outer: fillet(&pts, &radii, seg),
                    holes: Vec::new(),
                }])
            }
            "IFCUSHAPEPROFILEDEF" => {
                let (h, b, tw, tf) = (f(3)?, f(4)?, f(5)?, f(6)?);
                let r = f(7).unwrap_or(0.0);
                let e = f(8).unwrap_or(0.0);
                let (hh, hb) = (h / 2.0, b / 2.0);
                let pts = vec![
                    [-hb, -hh],
                    [hb, -hh],
                    [hb, -hh + tf],
                    [-hb + tw, -hh + tf],
                    [-hb + tw, hh - tf],
                    [hb, hh - tf],
                    [hb, hh],
                    [-hb, hh],
                ];
                let radii = [0.0, 0.0, e, r, r, e, 0.0, 0.0];
                pos()(vec![Region {
                    outer: fillet(&pts, &radii, seg),
                    holes: Vec::new(),
                }])
            }
            "IFCCSHAPEPROFILEDEF" => {
                let (h, b, t, g) = (f(3)?, f(4)?, f(5)?, f(6).unwrap_or(0.0));
                let r = f(7).unwrap_or(0.0);
                let (hh, hb) = (h / 2.0, b / 2.0);
                let mut pts = vec![[-hb, -hh], [hb, -hh]];
                let mut radii = vec![r + t, r + t];
                if g > t {
                    pts.extend([[hb, -hh + g], [hb - t, -hh + g]]);
                    radii.extend([0.0, 0.0]);
                }
                pts.extend([
                    [hb - t, -hh + t],
                    [-hb + t, -hh + t],
                    [-hb + t, hh - t],
                    [hb - t, hh - t],
                ]);
                radii.extend([r, r, r, r]);
                if g > t {
                    pts.extend([[hb - t, hh - g], [hb, hh - g]]);
                    radii.extend([0.0, 0.0]);
                }
                pts.extend([[hb, hh], [-hb, hh]]);
                radii.extend([r + t, r + t]);
                // Outer corners of the web side only round when the profile says so.
                if g <= t {
                    let k = radii.len();
                    radii[2] = 0.0;
                    radii[k - 2] = 0.0;
                }
                pos()(vec![Region {
                    outer: fillet(&pts, &radii, seg),
                    holes: Vec::new(),
                }])
            }
            "IFCZSHAPEPROFILEDEF" => {
                let (h, b, tw, tf) = (f(3)?, f(4)?, f(5)?, f(6)?);
                let (hh, ht) = (h / 2.0, tw / 2.0);
                let w = b - ht;
                let pts = vec![
                    [-ht, -hh],
                    [w, -hh],
                    [w, -hh + tf],
                    [ht, -hh + tf],
                    [ht, hh],
                    [-w, hh],
                    [-w, hh - tf],
                    [-ht, hh - tf],
                ];
                let r = f(7).unwrap_or(0.0);
                let e = f(8).unwrap_or(0.0);
                let radii = [0.0, 0.0, e, r, 0.0, 0.0, e, r];
                pos()(vec![Region {
                    outer: fillet(&pts, &radii, seg),
                    holes: Vec::new(),
                }])
            }
            "IFCTRAPEZIUMPROFILEDEF" => {
                let (bx, tx, y, off) = (f(3)?, f(4)?, f(5)?, f(6).unwrap_or(0.0));
                let (hx, hy) = (bx / 2.0, y / 2.0);
                pos()(vec![Region {
                    outer: vec![[-hx, -hy], [hx, -hy], [-hx + off + tx, hy], [-hx + off, hy]],
                    holes: Vec::new(),
                }])
            }
            "IFCCOMPOSITEPROFILEDEF" => {
                let mut all = Vec::new();
                for p in a.get(2).map(V::refs).unwrap_or_default() {
                    if let Some(r) = self.profile(p, depth + 1) {
                        all.extend(r);
                    }
                }
                all
            }
            "IFCDERIVEDPROFILEDEF" | "IFCMIRROREDPROFILEDEF" => {
                let parent = self.profile(a.get(2)?.as_ref()?, depth + 1)?;
                let m = if ty == "IFCMIRROREDPROFILEDEF" {
                    Mat::from_axes([-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3])
                } else {
                    a.get(3)
                        .and_then(V::as_ref)
                        .map(|o| self.operator(o))
                        .unwrap_or(Mat::IDENTITY)
                };
                place_regions(parent, &m)
            }
            _ => return None,
        };
        let regions: Vec<Region> = regions
            .into_iter()
            .filter(|r| r.outer.len() >= 3 && ring_area(&r.outer).abs() > 0.0)
            .collect();
        (!regions.is_empty()).then_some(regions)
    }
}

/// Regions mapped by a plane map (its x and y).
pub(crate) fn place_regions(rs: Vec<Region>, m: &Mat) -> Vec<Region> {
    let f = |p: P2| {
        let q = m.point([p[0], p[1], 0.0]);
        [q[0], q[1]]
    };
    rs.into_iter()
        .map(|r| Region {
            outer: r.outer.into_iter().map(f).collect(),
            holes: r
                .holes
                .into_iter()
                .map(|h| h.into_iter().map(f).collect())
                .collect(),
        })
        .collect()
}

/// An ellipse centred at the origin, counter-clockwise.
pub(crate) fn ellipse(a: f64, b: f64, seg: usize) -> Vec<P2> {
    let n = seg.max(8);
    (0..n)
        .map(|k| {
            let t = TAU * k as f64 / n as f64;
            [a * t.cos(), b * t.sin()]
        })
        .collect()
}

/// A rectangle centred at the origin with rounded corners.
pub(crate) fn rounded_rect(w: f64, h: f64, r: f64, seg: usize) -> Vec<P2> {
    let (hw, hh) = (w / 2.0, h / 2.0);
    let pts = [[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh]];
    let r = r.clamp(0.0, hw.min(hh));
    fillet(&pts, &[r; 4], seg)
}

#[allow(clippy::too_many_arguments)]
fn i_shape(
    bb: f64,
    bt: f64,
    h: f64,
    tw: f64,
    tfb: f64,
    tft: f64,
    rb: f64,
    rt: f64,
    seg: usize,
) -> Vec<P2> {
    let (hh, hbb, hbt, ht) = (h / 2.0, bb / 2.0, bt / 2.0, tw / 2.0);
    let pts = [
        [-hbb, -hh],
        [hbb, -hh],
        [hbb, -hh + tfb],
        [ht, -hh + tfb],
        [ht, hh - tft],
        [hbt, hh - tft],
        [hbt, hh],
        [-hbt, hh],
        [-hbt, hh - tft],
        [-ht, hh - tft],
        [-ht, -hh + tfb],
        [-hbb, -hh + tfb],
    ];
    let radii = [0.0, 0.0, 0.0, rb, rt, 0.0, 0.0, 0.0, 0.0, rt, rb, 0.0];
    fillet(&pts, &radii, seg)
}

/// A ring with corner `k` rounded by `radii[k]` (0: sharp).
pub(crate) fn fillet(pts: &[P2], radii: &[f64], seg: usize) -> Vec<P2> {
    let n = pts.len();
    let mut out = Vec::with_capacity(n * 2);
    for k in 0..n {
        let r = radii.get(k).copied().unwrap_or(0.0);
        let p = pts[k];
        if r.is_nan() || r <= 0.0 {
            out.push(p);
            continue;
        }
        let a = pts[(k + n - 1) % n];
        let b = pts[(k + 1) % n];
        let d1 = norm2([p[0] - a[0], p[1] - a[1]]);
        let d2 = norm2([b[0] - p[0], b[1] - p[1]]);
        let (Some(d1), Some(d2)) = (d1, d2) else {
            out.push(p);
            continue;
        };
        let turn = (d1[0] * d2[1] - d1[1] * d2[0]).atan2(d1[0] * d2[0] + d1[1] * d2[1]);
        if turn.abs() < 1e-6 || turn.abs() > PI - 1e-6 {
            out.push(p);
            continue;
        }
        let t = r * (turn.abs() / 2.0).tan();
        let la = ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt();
        let lb = ((b[0] - p[0]).powi(2) + (b[1] - p[1]).powi(2)).sqrt();
        if t > la * 0.999 || t > lb * 0.999 {
            out.push(p);
            continue;
        }
        let s = [p[0] - d1[0] * t, p[1] - d1[1] * t];
        // The centre is to the left of d1 for a left turn, to the right for a right one.
        let left = [-d1[1], d1[0]];
        let sign = turn.signum();
        let c = [s[0] + left[0] * r * sign, s[1] + left[1] * r * sign];
        let a0 = (s[1] - c[1]).atan2(s[0] - c[0]);
        let steps = ((turn.abs() / TAU * seg as f64).ceil() as usize).max(1);
        for j in 0..=steps {
            let ang = a0 + turn * j as f64 / steps as f64;
            out.push([c[0] + r * ang.cos(), c[1] + r * ang.sin()]);
        }
    }
    out
}

fn dot3(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm2(v: P2) -> Option<P2> {
    let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
    (l > 1e-15).then(|| [v[0] / l, v[1] / l])
}

/// The outline of an open polyline thickened by `t` (mitred joints).
fn thicken(line: &[P2], t: f64) -> Option<Vec<P2>> {
    if line.len() < 2 || t.is_nan() || t <= 0.0 {
        return None;
    }
    let h = t / 2.0;
    let n = line.len();
    let normal_at = |k: usize| -> P2 {
        let d = |i: usize, j: usize| norm2([line[j][0] - line[i][0], line[j][1] - line[i][1]]);
        let a = if k > 0 { d(k - 1, k) } else { None };
        let b = if k + 1 < n { d(k, k + 1) } else { None };
        let (dir, mitre) = match (a, b) {
            (Some(a), Some(b)) => {
                let m = norm2([a[0] + b[0], a[1] + b[1]]).unwrap_or(a);
                let cos = (m[0] * a[0] + m[1] * a[1]).max(0.2);
                (m, 1.0 / cos)
            }
            (Some(a), None) | (None, Some(a)) => (a, 1.0),
            (None, None) => ([1.0, 0.0], 1.0),
        };
        [-dir[1] * h * mitre, dir[0] * h * mitre]
    };
    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    for (k, p) in line.iter().enumerate() {
        let nk = normal_at(k);
        left.push([p[0] + nk[0], p[1] + nk[1]]);
        right.push([p[0] - nk[0], p[1] - nk[1]]);
    }
    right.reverse();
    right.extend(left);
    Some(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fillets_round_corners() {
        let sq = rounded_rect(2.0, 2.0, 0.5, 64);
        let a = ring_area(&sq);
        let want = 4.0 - (1.0 - PI / 4.0);
        assert!((a - want).abs() < 2e-3, "{a} {want}");
        let i = i_shape(0.2, 0.2, 0.4, 0.01, 0.02, 0.02, 0.0, 0.0, 32);
        let want = 2.0 * 0.2 * 0.02 + 0.01 * 0.36;
        assert!((ring_area(&i) - want).abs() < 1e-9);
        // Concave fillets add area.
        let i2 = i_shape(0.2, 0.2, 0.4, 0.01, 0.02, 0.02, 0.01, 0.01, 64);
        let added = 4.0 * (1e-4 - PI * 1e-4 / 4.0);
        assert!(
            (ring_area(&i2) - want - added).abs() < 2e-6,
            "{}",
            ring_area(&i2) - want
        );
    }

    #[test]
    fn thickened_line() {
        let r = thicken(&[[0.0, 0.0], [2.0, 0.0]], 0.2).unwrap();
        assert!((ring_area(&r).abs() - 0.4).abs() < 1e-12);
    }
}
