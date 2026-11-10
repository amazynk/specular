//! Geometry: instance transforms, paths expanded to polygons, convex hulls.

use specular_vector::kurbo::{Affine, Point, Vec2};

use crate::Strans;

/// sin and cos of `deg` degrees, exact for multiples of 90.
pub fn sin_cos_deg(deg: f64) -> (f64, f64) {
    let r = deg.rem_euclid(360.0);
    if r == 0.0 {
        (0.0, 1.0)
    } else if r == 90.0 {
        (1.0, 0.0)
    } else if r == 180.0 {
        (0.0, -1.0)
    } else if r == 270.0 {
        (-1.0, 0.0)
    } else {
        r.to_radians().sin_cos()
    }
}

/// The placement of an instance (or a text): reflect about x, magnify, rotate, move to
/// `origin`; all in database units.
pub fn placement(origin: [i32; 2], s: &Strans) -> Affine {
    let (sin, cos) = sin_cos_deg(s.angle);
    let m = if s.mag.is_finite() && s.mag != 0.0 {
        s.mag
    } else {
        1.0
    };
    let f = if s.reflect { -1.0 } else { 1.0 };
    // [cos -sin; sin cos] * m * [1 0; 0 f]
    Affine::new([
        m * cos,
        m * sin,
        -m * sin * f,
        m * cos * f,
        f64::from(origin[0]),
        f64::from(origin[1]),
    ])
}

/// The local placement of an instance with absolute magnification or rotation, under a
/// parent transform `parent` (whose linear part is a similarity): the magnification and
/// angle are taken relative to the top cell instead of the parent.
pub fn absolute(parent: Affine, origin: [i32; 2], s: &Strans) -> Affine {
    if !(s.abs_mag || s.abs_angle) {
        return placement(origin, s);
    }
    let c = parent.as_coeffs();
    let det = c[0] * c[3] - c[1] * c[2];
    let pscale = det.abs().sqrt();
    let preflect = det < 0.0;
    // Rotation of the parent: the angle of its x axis.
    let pangle = c[1].atan2(c[0]).to_degrees();
    let mut t = s.clone();
    if s.abs_mag && pscale > 0.0 && pscale.is_finite() {
        t.mag = s.mag / pscale;
    }
    if s.abs_angle {
        // parent R(p) F^r · R(a') = R(p ± a') F^r; solve for the total angle `a`.
        t.angle = if preflect {
            pangle - s.angle
        } else {
            s.angle - pangle
        };
    }
    placement(origin, &t)
}

/// The scale of a similarity transform.
pub fn scale_of(a: Affine) -> f64 {
    a.determinant().abs().sqrt()
}

/// How a path ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ends {
    /// Extended by these lengths at the start and the end (0 for flush, half the width
    /// for pathtype 2).
    Extended(f64, f64),
    /// Round (pathtype 1).
    Round,
}

/// Points with consecutive duplicates removed.
fn dedup(points: &[Point]) -> Vec<Point> {
    let mut v: Vec<Point> = Vec::with_capacity(points.len());
    for &p in points {
        if v.last() != Some(&p) {
            v.push(p);
        }
    }
    v
}

/// Where two lines meet: `a + u·da` and `b + v·db`, as (u, v); `None` when parallel.
fn intersect(a: Point, da: Vec2, b: Point, db: Vec2) -> Option<(f64, f64)> {
    let den = da.cross(db);
    if den.abs() < 1e-12 {
        return None;
    }
    let d = b - a;
    Some((d.cross(db) / den, d.cross(da) / den))
}

/// Segments of a round end: half circles get this many steps.
const ROUND_STEPS: usize = 16;

/// The outline of a path `width` wide along `spine`: the offset sides joined the way
/// GDSII viewers do (mitred, cut at half the width past the corner when sharper), with
/// the ends as given. Empty when it has no length or no width.
pub fn path_outline(spine: &[Point], width: f64, ends: Ends) -> Vec<Point> {
    let pts = dedup(spine);
    let h = width.abs() / 2.0;
    if pts.len() < 2 || !(h > 0.0 && h.is_finite()) {
        return Vec::new();
    }
    let n = pts.len();
    let dirs: Vec<Vec2> = pts
        .windows(2)
        .map(|w| {
            let d = w[1] - w[0];
            d / d.hypot()
        })
        .collect();
    let normal = |d: Vec2| Vec2::new(-d.y, d.x);
    let mut first = pts[0];
    let mut last = pts[n - 1];
    if let Ends::Extended(b, e) = ends {
        first -= dirs[0] * b;
        last += dirs[n - 2] * e;
    }
    // One side: +1 left, -1 right.
    let side = |s: f64| -> Vec<Point> {
        let mut out = Vec::with_capacity(n + 2);
        out.push(first + normal(dirs[0]) * (s * h));
        for k in 1..n - 1 {
            let (d0, d1) = (dirs[k - 1], dirs[k]);
            let a = pts[k] + normal(d0) * (s * h);
            let b = pts[k] + normal(d1) * (s * h);
            match intersect(a, d0, b, d1) {
                None => {
                    // Straight on, or turning back.
                    if d0.dot(d1) > 0.0 {
                        out.push(b);
                    } else {
                        out.push(a + d0 * h);
                        out.push(b - d1 * h);
                    }
                }
                Some((u, v)) => {
                    let outer = (d0.cross(d1) * s) < 0.0;
                    if !outer || (u <= h && -v <= h) {
                        out.push(a + d0 * u);
                    } else {
                        out.push(a + d0 * u.min(h));
                        out.push(b + d1 * v.max(-h));
                    }
                }
            }
        }
        out.push(last + normal(dirs[n - 2]) * (s * h));
        out
    };
    let left = side(1.0);
    let right = side(-1.0);
    let mut poly = Vec::with_capacity(left.len() + right.len() + 2 * ROUND_STEPS);
    if ends == Ends::Round {
        // Start cap: from the right side's start round the back to the left side's start.
        arc(&mut poly, pts[0], h, normal(dirs[0]) * -1.0, -1.0);
    }
    poly.extend(left);
    if ends == Ends::Round {
        arc(&mut poly, pts[n - 1], h, normal(dirs[n - 2]), -1.0);
    }
    poly.extend(right.into_iter().rev());
    dedup(&poly)
}

/// A half circle of radius `r` about `c` from `c + r·from` turning by π (`turn` -1:
/// clockwise), without its end points.
fn arc(out: &mut Vec<Point>, c: Point, r: f64, from: Vec2, turn: f64) {
    let a0 = from.y.atan2(from.x);
    for i in 1..ROUND_STEPS {
        let a = a0 + turn * std::f64::consts::PI * (i as f64) / (ROUND_STEPS as f64);
        out.push(c + Vec2::new(a.cos(), a.sin()) * r);
    }
}

/// The convex hull of `pts` (counter-clockwise, no collinear points).
pub fn hull(mut pts: Vec<Point>) -> Vec<Point> {
    pts.retain(|p| p.x.is_finite() && p.y.is_finite());
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: Point, a: Point, b: Point| (a - o).cross(b - o);
    let mut h: Vec<Point> = Vec::with_capacity(2 * pts.len());
    for &p in &pts {
        while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
            h.pop();
        }
        h.push(p);
    }
    let lower = h.len() + 1;
    for &p in pts.iter().rev().skip(1) {
        while h.len() >= lower && cross(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
            h.pop();
        }
        h.push(p);
    }
    h.pop();
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn straight_paths() {
        let s = [p(0.0, 0.0), p(10.0, 0.0)];
        assert_eq!(
            path_outline(&s, 2.0, Ends::Extended(0.0, 0.0)),
            vec![p(0.0, 1.0), p(10.0, 1.0), p(10.0, -1.0), p(0.0, -1.0)]
        );
        assert_eq!(
            path_outline(&s, 2.0, Ends::Extended(1.0, 3.0)),
            vec![p(-1.0, 1.0), p(13.0, 1.0), p(13.0, -1.0), p(-1.0, -1.0)]
        );
        let r = path_outline(&s, 2.0, Ends::Round);
        assert_eq!(r.len(), 4 + 2 * (ROUND_STEPS - 1));
        let xs: Vec<f64> = r.iter().map(|q| q.x).collect();
        assert!((xs.iter().cloned().fold(f64::MAX, f64::min) + 1.0).abs() < 1e-9);
        assert!((xs.iter().cloned().fold(f64::MIN, f64::max) - 11.0).abs() < 1e-9);
        assert!(path_outline(&[p(1.0, 1.0), p(1.0, 1.0)], 2.0, Ends::Round).is_empty());
    }

    #[test]
    fn mitred_corner() {
        let s = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)];
        let o = path_outline(&s, 2.0, Ends::Extended(0.0, 0.0));
        assert_eq!(
            o,
            vec![
                p(0.0, 1.0),
                p(9.0, 1.0),
                p(9.0, 10.0),
                p(11.0, 10.0),
                p(11.0, -1.0),
                p(0.0, -1.0)
            ]
        );
    }

    #[test]
    fn hulls_and_placements() {
        let h = hull(vec![
            p(0.0, 0.0),
            p(2.0, 0.0),
            p(1.0, 1.0),
            p(2.0, 2.0),
            p(0.0, 2.0),
            p(1.0, 0.0),
        ]);
        assert_eq!(h.len(), 4);
        let s = Strans {
            reflect: true,
            angle: 90.0,
            mag: 2.0,
            ..Strans::default()
        };
        let a = placement([5, 0], &s);
        // (1, 1) reflected (1, -1), doubled (2, -2), turned 90° (2, 2), moved (7, 2).
        assert_eq!(a * p(1.0, 1.0), p(7.0, 2.0));
    }
}
