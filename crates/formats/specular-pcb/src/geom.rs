//! Outlines of flashes and draws, in millimetres, y up.
//!
//! Every closed outline is oriented the same way (positive area) and every hole the other
//! way, so a non-zero fill of several outlines in one path is their union, holes included.

use std::f64::consts::{PI, TAU};

use specular_vector::kurbo::{Affine, Arc, BezPath, Point, Shape, Vec2};

/// Curve flattening tolerance (mm) where curves are sampled.
pub(crate) const FLAT: f64 = 1e-3;
/// Points closer than this are one point (mm).
pub(crate) const EPS: f64 = 1e-9;

/// `p` with its subpaths turned to positive area (`hole` false) or negative area.
pub(crate) fn orient(p: BezPath, hole: bool) -> BezPath {
    let a = p.area();
    if (a < 0.0) != hole && a != 0.0 {
        p.reverse_subpaths()
    } else {
        p
    }
}

/// Appends `sub` (one closed subpath) to `out`, oriented as an outline or a hole.
pub(crate) fn push(out: &mut BezPath, sub: BezPath, hole: bool) {
    for el in orient(sub, hole).elements() {
        out.push(*el);
    }
}

/// `p` under `a`; a mirroring transform keeps outlines positive.
pub(crate) fn transformed(p: &BezPath, a: Affine) -> BezPath {
    let mut q = a * p.clone();
    if a.determinant() < 0.0 {
        q = q.reverse_subpaths();
    }
    q
}

pub(crate) fn unit(angle: f64) -> Vec2 {
    Vec2::new(angle.cos(), angle.sin())
}

fn arc_to(p: &mut BezPath, center: Point, r: f64, start: f64, sweep: f64) {
    let arc = Arc::new(center, Vec2::new(r, r), start, sweep, 0.0);
    for el in arc.append_iter(FLAT * 0.1) {
        p.push(el);
    }
}

pub(crate) fn circle(c: Point, r: f64) -> BezPath {
    let mut p = BezPath::new();
    p.move_to(c + unit(0.0) * r);
    arc_to(&mut p, c, r, 0.0, TAU);
    p.close_path();
    orient(p, false)
}

/// The closed polygon through `pts`.
pub(crate) fn polygon(pts: &[Point]) -> BezPath {
    let mut p = BezPath::new();
    for (i, q) in pts.iter().enumerate() {
        if i == 0 {
            p.move_to(*q);
        } else {
            p.line_to(*q);
        }
    }
    p.close_path();
    orient(p, false)
}

/// An axis-aligned rectangle centred on `c`.
pub(crate) fn rect(c: Point, w: f64, h: f64) -> BezPath {
    let (x, y) = (w / 2.0, h / 2.0);
    polygon(&[
        Point::new(c.x - x, c.y - y),
        Point::new(c.x + x, c.y - y),
        Point::new(c.x + x, c.y + y),
        Point::new(c.x - x, c.y + y),
    ])
}

/// A rectangle with semicircular ends along its long side, centred on `c`.
pub(crate) fn obround(c: Point, w: f64, h: f64) -> BezPath {
    if (w - h).abs() < EPS {
        return circle(c, w / 2.0);
    }
    if w > h {
        let d = Vec2::new((w - h) / 2.0, 0.0);
        capsule(c - d, c + d, h / 2.0)
    } else {
        let d = Vec2::new(0.0, (h - w) / 2.0);
        capsule(c - d, c + d, w / 2.0)
    }
}

/// A regular polygon of `n` vertices on a circle of diameter `d`, the first at `rot`
/// degrees.
pub(crate) fn regular(c: Point, d: f64, n: usize, rot_deg: f64) -> BezPath {
    let n = n.clamp(3, 1000);
    let pts: Vec<Point> = (0..n)
        .map(|i| c + unit((rot_deg + 360.0 * i as f64 / n as f64).to_radians()) * (d / 2.0))
        .collect();
    polygon(&pts)
}

/// The line from `a` to `b` drawn `2r` wide with round ends.
pub(crate) fn capsule(a: Point, b: Point, r: f64) -> BezPath {
    let d = b - a;
    if d.hypot() < EPS {
        return circle(a, r);
    }
    let t = d.y.atan2(d.x);
    let left = unit(t + PI / 2.0) * r;
    let mut p = BezPath::new();
    p.move_to(a + left);
    p.line_to(b + left);
    arc_to(&mut p, b, r, t + PI / 2.0, -PI);
    p.line_to(a - left);
    arc_to(&mut p, a, r, t - PI / 2.0, -PI);
    p.close_path();
    orient(p, false)
}

/// The arc around `c` of radius `r` from angle `start` through `sweep` (radians, positive
/// counter-clockwise), drawn `2hw` wide with round ends.
pub(crate) fn arc_band(c: Point, r: f64, start: f64, sweep: f64, hw: f64) -> BezPath {
    if sweep.abs() < 1e-12 {
        return circle(c + unit(start) * r, hw);
    }
    let ro = r + hw;
    let ri = r - hw;
    if sweep.abs() >= TAU - 1e-9 {
        let mut p = BezPath::new();
        push(&mut p, circle(c, ro), false);
        if ri > EPS {
            push(&mut p, circle(c, ri), true);
        }
        return p;
    }
    if ri <= EPS {
        // Thicker than its radius: capsules along chords.
        let n = ((sweep.abs() / (PI / 64.0)).ceil() as usize).max(8);
        let mut p = BezPath::new();
        let pt = |i: usize| c + unit(start + sweep * i as f64 / n as f64) * r;
        for i in 0..n {
            push(&mut p, capsule(pt(i), pt(i + 1), hw), false);
        }
        return p;
    }
    let s = sweep.signum();
    let end = start + sweep;
    let mut p = BezPath::new();
    p.move_to(c + unit(start) * ro);
    arc_to(&mut p, c, ro, start, sweep);
    arc_to(&mut p, c + unit(end) * r, hw, end, s * PI);
    arc_to(&mut p, c, ri, end, -sweep);
    arc_to(&mut p, c + unit(start) * r, hw, start + PI, s * PI);
    p.close_path();
    orient(p, false)
}

/// The convex hull of `pts` as a closed outline (`None` for fewer than three distinct
/// points in general position).
pub(crate) fn hull(pts: &[Point]) -> Option<BezPath> {
    let mut v: Vec<Point> = pts.to_vec();
    v.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    v.dedup_by(|a, b| (a.x - b.x).abs() < EPS && (a.y - b.y).abs() < EPS);
    if v.len() < 3 {
        return None;
    }
    let cross = |o: Point, a: Point, b: Point| (a - o).cross(b - o);
    let mut lower: Vec<Point> = Vec::new();
    for &p in &v {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<Point> = Vec::new();
    for &p in v.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    (lower.len() >= 3).then(|| polygon(&lower))
}

/// Points along every subpath of `p` (curves flattened), for hulls and bounds.
pub(crate) fn sample(p: &BezPath) -> Vec<Point> {
    let mut out = Vec::new();
    specular_vector::kurbo::flatten(p.elements().iter().copied(), FLAT * 10.0, |el| {
        use specular_vector::kurbo::PathEl;
        match el {
            PathEl::MoveTo(q) | PathEl::LineTo(q) => out.push(q),
            _ => {}
        }
    });
    out
}

/// The signed sweep from `a0` to `a1` (radians) turning counter-clockwise (`ccw`) or
/// clockwise; `full` makes equal angles a whole turn.
pub(crate) fn sweep(a0: f64, a1: f64, ccw: bool, full: bool) -> f64 {
    let turn = |d: f64| if d < 1e-9 && full { TAU } else { d };
    if ccw {
        turn((a1 - a0).rem_euclid(TAU))
    } else {
        -turn((a0 - a1).rem_euclid(TAU))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn outlines_have_their_areas_and_orientation() {
        let c = circle(Point::ORIGIN, 1.0);
        assert!(close(c.area(), PI, 1e-3), "{}", c.area());
        let r = rect(Point::new(5.0, 5.0), 2.0, 3.0);
        assert!(close(r.area(), 6.0, 1e-9));
        let k = capsule(Point::ORIGIN, Point::new(10.0, 0.0), 0.5);
        assert!(close(k.area(), 10.0 + PI * 0.25, 1e-4), "{}", k.area());
        let o = obround(Point::ORIGIN, 1.0, 3.0);
        assert!(close(o.area(), 2.0 + PI * 0.25, 1e-4));
        let h = regular(Point::ORIGIN, 2.0, 4, 0.0);
        assert!(close(h.area(), 2.0, 1e-9));
        // A quarter arc of radius 5, 1 wide: band plus two half caps.
        let b = arc_band(Point::ORIGIN, 5.0, 0.0, PI / 2.0, 0.5);
        let want = PI / 4.0 * (5.5f64.powi(2) - 4.5f64.powi(2)) + PI * 0.25;
        assert!(close(b.area(), want, 1e-3), "{} {want}", b.area());
        let cw = arc_band(Point::ORIGIN, 5.0, PI / 2.0, -PI / 2.0, 0.5);
        assert!(close(cw.area(), want, 1e-3));
        let full = arc_band(Point::ORIGIN, 5.0, 0.0, TAU, 0.5);
        assert!(close(
            full.area(),
            PI * (5.5f64.powi(2) - 4.5f64.powi(2)),
            1e-3
        ));
    }

    #[test]
    fn holes_and_mirrors_keep_the_union() {
        let mut p = BezPath::new();
        push(&mut p, circle(Point::ORIGIN, 2.0), false);
        push(&mut p, circle(Point::ORIGIN, 1.0), true);
        assert!(close(p.area(), PI * 3.0, 1e-3));
        let m = transformed(&p, Affine::FLIP_X);
        assert!(close(m.area(), PI * 3.0, 1e-3));
    }

    #[test]
    fn hull_and_sweeps() {
        let h = hull(&[
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
            Point::new(0.5, 0.5),
        ])
        .unwrap();
        assert!(close(h.area(), 1.0, 1e-12));
        assert!(close(sweep(0.0, PI / 2.0, true, true), PI / 2.0, 1e-12));
        assert!(close(sweep(0.0, PI / 2.0, false, true), -1.5 * PI, 1e-12));
        assert!(close(sweep(1.0, 1.0, true, true), TAU, 1e-12));
        assert!(close(sweep(1.0, 1.0, false, false), 0.0, 1e-12));
    }
}
