//! Revision clouds: a closed polygon drawn as a chain of half circles bulging outwards, each
//! edge split into arcs of about the same chord.

use crate::P;
use std::f64::consts::{FRAC_PI_2, PI};

/// One circular arc: centre, radius, start angle and sweep (radians; y down, so a positive
/// sweep turns clockwise on screen).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    pub c: P,
    pub r: f64,
    pub start: f64,
    pub sweep: f64,
}

impl Arc {
    pub fn at(&self, a: f64) -> P {
        P::new(self.c.x + self.r * a.cos(), self.c.y + self.r * a.sin())
    }
}

/// The chord of a cloud's arcs for line width `width` and text size `size`.
pub fn arc_length(width: f64, size: f64) -> f64 {
    (size * 1.4).max(width * 8.0).max(1e-6)
}

/// The cloud around `pts` (a closed polygon) with arcs of about `chord`.
pub fn arcs(pts: &[P], chord: f64) -> Vec<Arc> {
    let n = pts.len();
    if n < 2 || chord.is_nan() || chord <= 0.0 {
        return Vec::new();
    }
    // Shoelace: the sign says which side of each edge is outside.
    let area2: f64 = (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    let ccw = area2 >= 0.0;
    let mut out = Vec::new();
    for i in 0..n {
        let (p, q) = (pts[i], pts[(i + 1) % n]);
        let len = p.dist(q);
        if len < 1e-9 {
            continue;
        }
        let k = (len / chord).round().clamp(1.0, 10_000.0) as usize;
        let (dx, dy) = ((q.x - p.x) / k as f64, (q.y - p.y) / k as f64);
        // Outward normal.
        let (nx, ny) = if ccw { (dy, -dx) } else { (-dy, dx) };
        let normal = ny.atan2(nx);
        for j in 0..k {
            let a = P::new(p.x + dx * j as f64, p.y + dy * j as f64);
            let b = P::new(a.x + dx, a.y + dy);
            let c = P::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            let start = (a.y - c.y).atan2(a.x - c.x);
            // The arc passes the outward normal halfway.
            let mut d = normal - start;
            while d > PI {
                d -= 2.0 * PI;
            }
            while d < -PI {
                d += 2.0 * PI;
            }
            let sweep = if d >= 0.0 { PI } else { -PI };
            out.push(Arc {
                c,
                r: a.dist(b) / 2.0,
                start,
                sweep,
            });
        }
    }
    out
}

/// The arcs as a polyline, points at most `tol` from the true curve.
pub fn flatten(arcs: &[Arc], tol: f64) -> Vec<P> {
    let mut out = Vec::new();
    for a in arcs {
        let step = if a.r > tol {
            2.0 * (1.0 - tol / a.r).clamp(-1.0, 1.0).acos()
        } else {
            PI
        };
        let n = (a.sweep.abs() / step.max(1e-3)).ceil().clamp(2.0, 256.0) as usize;
        for i in 0..=n {
            if i == 0 && !out.is_empty() {
                continue;
            }
            out.push(a.at(a.start + a.sweep * i as f64 / n as f64));
        }
    }
    out
}

/// The arcs as cubic Béziers (start, control, control, end), at most a quarter turn each.
pub fn beziers(arcs: &[Arc]) -> Vec<[P; 4]> {
    let mut out = Vec::new();
    for a in arcs {
        let n = (a.sweep.abs() / FRAC_PI_2).ceil().max(1.0) as usize;
        let s = a.sweep / n as f64;
        let k = 4.0 / 3.0 * (s / 4.0).tan();
        for i in 0..n {
            let t0 = a.start + s * i as f64;
            let t1 = t0 + s;
            let p0 = a.at(t0);
            let p3 = a.at(t1);
            let d0 = P::new(-t0.sin() * a.r * k, t0.cos() * a.r * k);
            let d1 = P::new(-t1.sin() * a.r * k, t1.cos() * a.r * k);
            out.push([
                p0,
                P::new(p0.x + d0.x, p0.y + d0.y),
                P::new(p3.x - d1.x, p3.y - d1.y),
                p3,
            ]);
        }
    }
    out
}
