//! Clash detection between two sets of a building model's elements.
//!
//! - Hard clashes: the elements' triangles cross and one reaches into the other deeper than
//!   a tolerance (so faces that merely touch, as a wall on a slab, are not clashes), or one
//!   element lies wholly inside the other.
//! - Clearance clashes: elements that do not clash hard but come closer than a distance.
//!
//! The broad phase sorts the elements' boxes (grown by the clearance) along one axis and
//! sweeps; the narrow phase walks two bounding volume hierarchies of the elements'
//! triangles together: exact segment–triangle crossings, then the penetration depth as
//! the deepest vertex of either element inside the other (by ray parity, three rays
//! voting) measured to the other's surface; for clearance, the exact smallest distance
//! between triangles. Pairs run in parallel on the CPU.
//!
//! Each clash is one element pair. Results are kept with a status (new, active, resolved)
//! in `<model>.specular-clashes.json` next to the model: a clash found again is active, one
//! no longer found is resolved.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use specular_mesh::ifc::Element;

type P = [f64; 3];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul(a: P, s: f64) -> P {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn len2(a: P) -> f64 {
    dot(a, a)
}

type Tri = [P; 3];

/// An axis-aligned box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: P,
    pub max: P,
}

impl Aabb {
    fn empty() -> Aabb {
        Aabb {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        }
    }
    fn grow(&mut self, p: P) {
        for ((lo, hi), x) in self.min.iter_mut().zip(self.max.iter_mut()).zip(p) {
            *lo = lo.min(x);
            *hi = hi.max(x);
        }
    }
    fn join(&mut self, o: &Aabb) {
        self.grow(o.min);
        self.grow(o.max);
    }
    fn is_empty(&self) -> bool {
        self.min[0] > self.max[0]
    }
    /// The gap between two boxes (0 when they overlap).
    fn gap(&self, o: &Aabb) -> f64 {
        let mut d = 0.0;
        for k in 0..3 {
            let g = (o.min[k] - self.max[k])
                .max(self.min[k] - o.max[k])
                .max(0.0);
            d += g * g;
        }
        d.sqrt()
    }
    pub fn centre(&self) -> P {
        mul(add(self.min, self.max), 0.5)
    }
}

/// A bounding volume hierarchy over one element's triangles.
struct Bvh {
    tris: Vec<Tri>,
    /// (box, first child or first triangle, count: 0 for an inner node).
    nodes: Vec<(Aabb, u32, u32)>,
}

const LEAF: usize = 4;

impl Bvh {
    fn new(mut tris: Vec<Tri>) -> Bvh {
        let mut nodes = Vec::with_capacity(tris.len() / 2 + 1);
        nodes.push((Aabb::empty(), 0, 0));
        let n = tris.len();
        Self::build(&mut tris, &mut nodes, 0, 0, n, 0);
        Bvh { tris, nodes }
    }

    fn tri_box(t: &Tri) -> Aabb {
        let mut b = Aabb::empty();
        for p in t {
            b.grow(*p);
        }
        b
    }

    fn build(
        tris: &mut [Tri],
        nodes: &mut Vec<(Aabb, u32, u32)>,
        at: usize,
        lo: usize,
        hi: usize,
        depth: usize,
    ) {
        let mut b = Aabb::empty();
        for t in &tris[lo..hi] {
            b.join(&Self::tri_box(t));
        }
        if hi - lo <= LEAF || depth > 48 {
            nodes[at] = (b, lo as u32, (hi - lo) as u32);
            return;
        }
        let ext = sub(b.max, b.min);
        let k = if ext[0] >= ext[1] && ext[0] >= ext[2] {
            0
        } else if ext[1] >= ext[2] {
            1
        } else {
            2
        };
        let mid = (lo + hi) / 2;
        let c = |t: &Tri| t[0][k] + t[1][k] + t[2][k];
        tris[lo..hi].select_nth_unstable_by(mid - lo, |a, b| c(a).total_cmp(&c(b)));
        let first = nodes.len();
        nodes.push((Aabb::empty(), 0, 0));
        nodes.push((Aabb::empty(), 0, 0));
        nodes[at] = (b, first as u32, 0);
        Self::build(tris, nodes, first, lo, mid, depth + 1);
        Self::build(tris, nodes, first + 1, mid, hi, depth + 1);
    }

    fn bounds(&self) -> Aabb {
        self.nodes[0].0
    }

    /// Calls `f` on every pair of leaves (triangle ranges) of `self` and `o` whose boxes
    /// are within `gap`; `f` returns false to stop.
    fn pairs(&self, o: &Bvh, gap: f64, f: &mut dyn FnMut(&[Tri], &[Tri]) -> bool) {
        let mut stack = vec![(0usize, 0usize)];
        while let Some((a, b)) = stack.pop() {
            let (ba, fa, na) = self.nodes[a];
            let (bb, fb, nb) = o.nodes[b];
            if ba.gap(&bb) > gap {
                continue;
            }
            match (na > 0, nb > 0) {
                (true, true) => {
                    let ta = &self.tris[fa as usize..(fa + na) as usize];
                    let tb = &o.tris[fb as usize..(fb + nb) as usize];
                    if !f(ta, tb) {
                        return;
                    }
                }
                (true, false) => {
                    stack.push((a, fb as usize));
                    stack.push((a, fb as usize + 1));
                }
                (false, true) => {
                    stack.push((fa as usize, b));
                    stack.push((fa as usize + 1, b));
                }
                (false, false) => {
                    // Split the bigger box.
                    let va = len2(sub(ba.max, ba.min));
                    let vb = len2(sub(bb.max, bb.min));
                    if va >= vb {
                        stack.push((fa as usize, b));
                        stack.push((fa as usize + 1, b));
                    } else {
                        stack.push((a, fb as usize));
                        stack.push((a, fb as usize + 1));
                    }
                }
            }
        }
    }

    /// How many triangles a ray from `o` along `d` crosses.
    fn crossings(&self, o: P, d: P) -> usize {
        let mut n = 0;
        let mut stack = vec![0usize];
        let inv = d.map(|x| if x == 0.0 { f64::INFINITY } else { 1.0 / x });
        while let Some(i) = stack.pop() {
            let (b, f, c) = self.nodes[i];
            if !ray_box(o, inv, &b) {
                continue;
            }
            if c > 0 {
                for t in &self.tris[f as usize..(f + c) as usize] {
                    if ray_tri(o, d, t).is_some_and(|s| s > 0.0) {
                        n += 1;
                    }
                }
            } else {
                stack.push(f as usize);
                stack.push(f as usize + 1);
            }
        }
        n
    }

    /// Whether `p` is inside the closed surface (two of three rays agree).
    fn inside(&self, p: P) -> bool {
        let b = self.bounds();
        if (0..3).any(|k| p[k] < b.min[k] || p[k] > b.max[k]) {
            return false;
        }
        const DIRS: [P; 3] = [
            [0.577_2, 0.301_3, 0.759_3],
            [-0.412_310_56, 0.866_025_40, -0.282_842_71],
            [0.123_1, -0.707_3, -0.696_3],
        ];
        DIRS.iter()
            .filter(|d| self.crossings(p, **d) % 2 == 1)
            .count()
            >= 2
    }

    /// The distance from `p` to the nearest triangle.
    fn distance(&self, p: P) -> f64 {
        let mut best = f64::INFINITY;
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let (b, f, c) = self.nodes[i];
            let g = point_box(p, &b);
            if g * g >= best {
                continue;
            }
            if c > 0 {
                for t in &self.tris[f as usize..(f + c) as usize] {
                    best = best.min(len2(sub(p, closest_on_tri(p, t))));
                }
            } else {
                stack.push(f as usize);
                stack.push(f as usize + 1);
            }
        }
        best.sqrt()
    }
}

fn point_box(p: P, b: &Aabb) -> f64 {
    let d: f64 = (0..3)
        .map(|k| (b.min[k] - p[k]).max(p[k] - b.max[k]).max(0.0).powi(2))
        .sum();
    d.sqrt()
}

fn ray_box(o: P, inv: P, b: &Aabb) -> bool {
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for k in 0..3 {
        let mut a = (b.min[k] - o[k]) * inv[k];
        let mut c = (b.max[k] - o[k]) * inv[k];
        if a.is_nan() || c.is_nan() {
            // On the slab's edge with a zero direction: inside when between.
            if o[k] < b.min[k] || o[k] > b.max[k] {
                return false;
            }
            continue;
        }
        if a > c {
            std::mem::swap(&mut a, &mut c);
        }
        t0 = t0.max(a);
        t1 = t1.min(c);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// Möller–Trumbore: the ray parameter where `o + s d` crosses `t`.
fn ray_tri(o: P, d: P, t: &Tri) -> Option<f64> {
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-18 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(o, t[0]);
    let u = dot(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(d, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(dot(e2, q) * inv)
}

/// Where the segment `a`–`b` crosses `t` (strictly inside the segment), if it does.
fn segment_tri(a: P, b: P, t: &Tri) -> Option<P> {
    let d = sub(b, a);
    let s = ray_tri(a, d, t)?;
    (s > 1e-9 && s < 1.0 - 1e-9).then(|| add(a, mul(d, s)))
}

/// A point where two triangles cross, if they do (coplanar contact is not a crossing).
fn tri_tri(a: &Tri, b: &Tri) -> Option<P> {
    for (x, y) in [(0, 1), (1, 2), (2, 0)] {
        if let Some(p) = segment_tri(a[x], a[y], b) {
            return Some(p);
        }
        if let Some(p) = segment_tri(b[x], b[y], a) {
            return Some(p);
        }
    }
    None
}

/// The point of `t` nearest `p` (Ericson, Real-Time Collision Detection 5.1.5).
fn closest_on_tri(p: P, t: &Tri) -> P {
    let (a, b, c) = (t[0], t[1], t[2]);
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return add(a, mul(ab, d1 / (d1 - d3)));
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return add(a, mul(ac, d2 / (d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return add(b, mul(sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6))));
    }
    let denom = va + vb + vc;
    if denom.abs() < 1e-300 {
        return a;
    }
    let v = vb / denom;
    let w = vc / denom;
    add(a, add(mul(ab, v), mul(ac, w)))
}

/// The nearest points of segments `p1 q1` and `p2 q2` (Ericson 5.1.9).
fn closest_segments(p1: P, q1: P, p2: P, q2: P) -> (P, P) {
    let d1 = sub(q1, p1);
    let d2 = sub(q2, p2);
    let r = sub(p1, p2);
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);
    let (s, t);
    if a <= 1e-300 && e <= 1e-300 {
        return (p1, p2);
    }
    if a <= 1e-300 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = dot(d1, r);
        if e <= 1e-300 {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = dot(d1, d2);
            let denom = a * e - b * b;
            let mut s0 = if denom > 1e-300 {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = (-c / a).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = s0;
            t = t0;
        }
    }
    (add(p1, mul(d1, s)), add(p2, mul(d2, t)))
}

/// The nearest points of two triangles that do not cross.
fn tri_distance(a: &Tri, b: &Tri) -> (f64, P, P) {
    let mut best = (f64::INFINITY, a[0], b[0]);
    let mut try_pair = |p: P, q: P| {
        let d = len2(sub(p, q));
        if d < best.0 {
            best = (d, p, q);
        }
    };
    for p in a {
        try_pair(*p, closest_on_tri(*p, b));
    }
    for q in b {
        try_pair(closest_on_tri(*q, a), *q);
    }
    for (i, j) in [(0, 1), (1, 2), (2, 0)] {
        for (k, l) in [(0, 1), (1, 2), (2, 0)] {
            let (p, q) = closest_segments(a[i], a[j], b[k], b[l]);
            try_pair(p, q);
        }
    }
    (best.0.sqrt(), best.1, best.2)
}

/// What a test looks for.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    /// Hard clashes deeper than `tolerance` (mesh units).
    Hard { tolerance: f64 },
    /// Elements closer than `distance` (mesh units) that do not clash hard.
    Clearance { distance: f64, tolerance: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Hard,
    Clearance,
}

/// One clashing pair of elements.
#[derive(Clone, Debug, PartialEq)]
pub struct Clash {
    /// Indices into the elements.
    pub a: usize,
    pub b: usize,
    pub kind: Kind,
    /// Hard: the penetration depth; clearance: the distance (mesh units).
    pub depth: f64,
    /// Where: a crossing or the deepest point (hard), the middle of the nearest points
    /// (clearance), in mesh units.
    pub point: P,
    /// The region of the clash: the two elements' boxes' overlap (grown by the clearance).
    pub region: Aabb,
}

/// The model's triangles by element (`elements` sorted by first vertex).
pub struct Model<'a> {
    pub positions: &'a [[f32; 3]],
    pub indices: &'a [u32],
    pub elements: &'a [Element],
}

impl Model<'_> {
    /// Each element's triangles.
    fn triangles(&self) -> Vec<Vec<Tri>> {
        let mut out: Vec<Vec<Tri>> = vec![Vec::new(); self.elements.len()];
        let p = |i: u32| {
            let v = self.positions[i as usize];
            [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]
        };
        for t in self.indices.chunks_exact(3) {
            if t.iter().any(|&i| i as usize >= self.positions.len()) {
                continue;
            }
            if let Some(e) = crate::apply::element_of(self.elements, t[0]) {
                out[e].push([p(t[0]), p(t[1]), p(t[2])]);
            }
        }
        out
    }
}

/// Runs a clash test between elements `a` and `b` (indices; an element in both is tested
/// against the others, never itself). `skip` names pairs never to report (parts of one
/// assembly, an element and the opening filler it holds).
pub fn run(
    m: &Model,
    a: &[usize],
    b: &[usize],
    mode: Mode,
    skip: &(dyn Fn(usize, usize) -> bool + Sync),
) -> Vec<Clash> {
    let tris = m.triangles();
    let reach = match mode {
        Mode::Hard { .. } => 0.0,
        Mode::Clearance { distance, .. } => distance,
    };
    let boxes: Vec<Aabb> = tris
        .iter()
        .map(|ts| {
            let mut bx = Aabb::empty();
            for t in ts {
                for p in t {
                    bx.grow(*p);
                }
            }
            bx
        })
        .collect();
    // Broad phase: sweep along x.
    let in_b: Vec<bool> = {
        let mut v = vec![false; tris.len()];
        for &i in b {
            if i < v.len() {
                v[i] = true;
            }
        }
        v
    };
    let in_a: Vec<bool> = {
        let mut v = vec![false; tris.len()];
        for &i in a {
            if i < v.len() {
                v[i] = true;
            }
        }
        v
    };
    let mut order: Vec<usize> = (0..tris.len())
        .filter(|&i| (in_a[i] || in_b[i]) && !boxes[i].is_empty())
        .collect();
    order.sort_by(|&x, &y| boxes[x].min[0].total_cmp(&boxes[y].min[0]));
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for (k, &i) in order.iter().enumerate() {
        let bi = boxes[i];
        for &j in &order[k + 1..] {
            if boxes[j].min[0] > bi.max[0] + reach {
                break;
            }
            let ab = in_a[i] && in_b[j];
            let ba = in_b[i] && in_a[j];
            if !(ab || ba) || bi.gap(&boxes[j]) > reach {
                continue;
            }
            let (x, y) = (i.min(j), i.max(j));
            if !skip(x, y) {
                pairs.push((x, y));
            }
        }
    }
    // The hierarchies of the elements in some pair.
    let mut used = vec![false; tris.len()];
    for &(x, y) in &pairs {
        used[x] = true;
        used[y] = true;
    }
    let bvhs: Vec<Option<Bvh>> = tris
        .into_par_iter()
        .enumerate()
        .map(|(i, t)| used[i].then(|| Bvh::new(t)))
        .collect();
    let mut out: Vec<Clash> = pairs
        .par_iter()
        .filter_map(|&(x, y)| {
            let (Some(p), Some(q)) = (bvhs[x].as_ref(), bvhs[y].as_ref()) else {
                return None;
            };
            narrow(p, q, mode).map(|(kind, depth, point)| {
                let mut region = Aabb {
                    min: std::array::from_fn(|k| boxes[x].min[k].max(boxes[y].min[k]) - reach),
                    max: std::array::from_fn(|k| boxes[x].max[k].min(boxes[y].max[k]) + reach),
                };
                for k in 0..3 {
                    if region.min[k] > region.max[k] {
                        let c = (region.min[k] + region.max[k]) / 2.0;
                        region.min[k] = c;
                        region.max[k] = c;
                    }
                }
                Clash {
                    a: x,
                    b: y,
                    kind,
                    depth,
                    point,
                    region,
                }
            })
        })
        .collect();
    out.sort_by(|p, q| {
        q.depth
            .total_cmp(&p.depth)
            .then(p.a.cmp(&q.a))
            .then(p.b.cmp(&q.b))
    });
    if matches!(mode, Mode::Clearance { .. }) {
        out.sort_by(|p, q| p.depth.total_cmp(&q.depth).then(p.a.cmp(&q.a)));
    }
    out
}

/// The deepest vertex of `p` inside `q`: its depth and position.
fn deepest_inside(p: &Bvh, q: &Bvh) -> Option<(f64, P)> {
    let qb = q.bounds();
    let mut best: Option<(f64, P)> = None;
    let mut seen = std::collections::HashSet::new();
    for t in &p.tris {
        for v in t {
            if point_box(*v, &qb) > 0.0 {
                continue;
            }
            let key = v.map(f64::to_bits);
            if !seen.insert(key) {
                continue;
            }
            if q.inside(*v) {
                let d = q.distance(*v);
                if best.is_none_or(|(b, _)| d > b) {
                    best = Some((d, *v));
                }
            }
        }
    }
    best
}

/// Twice the radius of the largest ball found inside both (on a grid over the region
/// around the crossings), and where.
fn overlap_thickness(p: &Bvh, q: &Bvh, crossings: &Aabb) -> Option<(f64, P)> {
    let ext = sub(crossings.max, crossings.min);
    let grow = ext[0].max(ext[1]).max(ext[2]);
    let (pb, qb) = (p.bounds(), q.bounds());
    let region = Aabb {
        min: std::array::from_fn(|k| (crossings.min[k] - grow).max(pb.min[k]).max(qb.min[k])),
        max: std::array::from_fn(|k| (crossings.max[k] + grow).min(pb.max[k]).min(qb.max[k])),
    };
    if region.is_empty() {
        return None;
    }
    const N: usize = 6;
    let mut best: Option<(f64, P)> = None;
    for i in 0..N {
        for j in 0..N {
            for k in 0..N {
                let f = |n: usize, a: usize| {
                    (n as f64 + 0.5) / N as f64 * (region.max[a] - region.min[a]) + region.min[a]
                };
                let x = [f(i, 0), f(j, 1), f(k, 2)];
                if !(p.inside(x) && q.inside(x)) {
                    continue;
                }
                let r = p.distance(x).min(q.distance(x));
                if best.is_none_or(|(b, _)| 2.0 * r > b) {
                    best = Some((2.0 * r, x));
                }
            }
        }
    }
    best
}

/// Two copies of one element (the same element in two models of a federation, or modelled
/// twice): the boxes agree and every vertex of each lies on the other's surface, within
/// `tolerance`. Their depth is the box's smallest side.
fn coincident(p: &Bvh, q: &Bvh, tolerance: f64) -> Option<(f64, P)> {
    let (pb, qb) = (p.bounds(), q.bounds());
    if pb.is_empty() || qb.is_empty() {
        return None;
    }
    if (0..3).any(|k| {
        (pb.min[k] - qb.min[k]).abs() > tolerance || (pb.max[k] - qb.max[k]).abs() > tolerance
    }) {
        return None;
    }
    let on = |a: &Bvh, b: &Bvh| a.tris.iter().flatten().all(|v| b.distance(*v) <= tolerance);
    if !(on(p, q) && on(q, p)) {
        return None;
    }
    let ext = sub(pb.max, pb.min);
    Some((ext[0].min(ext[1]).min(ext[2]), pb.centre()))
}

fn narrow(p: &Bvh, q: &Bvh, mode: Mode) -> Option<(Kind, f64, P)> {
    let tolerance = match mode {
        Mode::Hard { tolerance } | Mode::Clearance { tolerance, .. } => tolerance,
    };
    // Where the surfaces cross (up to a few hundred points).
    let mut cross_box = Aabb::empty();
    let mut crossing: Option<P> = None;
    let mut crossings = 0;
    p.pairs(q, 0.0, &mut |ta, tb| {
        for a in ta {
            for b in tb {
                if let Some(x) = tri_tri(a, b) {
                    crossing.get_or_insert(x);
                    cross_box.grow(x);
                    crossings += 1;
                    if crossings >= 512 {
                        return false;
                    }
                }
            }
        }
        true
    });
    // How deep either reaches into the other (also when one holds the other whole).
    let mut deep = match (deepest_inside(p, q), deepest_inside(q, p)) {
        (Some(x), Some(y)) => Some(if x.0 >= y.0 { x } else { y }),
        (x, y) => x.or(y),
    };
    // A part passing through (a duct through a wall) has no vertex inside the other: the
    // thickness of the overlap, sampled around the crossings, is its depth.
    if crossing.is_some()
        && deep.is_none_or(|(d, _)| d <= tolerance)
        && let Some(t) = overlap_thickness(p, q, &cross_box)
        && deep.is_none_or(|(d, _)| t.0 > d)
    {
        deep = Some(t);
    }
    if deep.is_none_or(|(d, _)| d <= tolerance)
        && let Some(t) = coincident(p, q, tolerance.max(1e-9))
    {
        deep = Some(t);
    }
    if let Some((d, at)) = deep
        && d > tolerance
    {
        return Some((Kind::Hard, d, crossing.unwrap_or(at)));
    }
    let Mode::Clearance { distance, .. } = mode else {
        return None;
    };
    if crossing.is_some() || deep.is_some() {
        // Touching or barely in: closer than any clearance.
        let at = crossing.or(deep.map(|d| d.1))?;
        return Some((Kind::Clearance, 0.0, at));
    }
    let mut best = (f64::INFINITY, [0.0; 3]);
    p.pairs(q, distance, &mut |ta, tb| {
        for a in ta {
            for b in tb {
                let (d, x, y) = tri_distance(a, b);
                if d < best.0 {
                    best = (d, mul(add(x, y), 0.5));
                }
            }
        }
        true
    });
    (best.0 < distance).then_some((Kind::Clearance, best.0, best.1))
}

// --- Selection sets ------------------------------------------------------------------

/// Which elements a side of a test takes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Selector {
    #[default]
    All,
    /// By IFC class, upper case (`IFCWALL`); subtypes as their own classes.
    Classes(Vec<String>),
    /// By storey name.
    Storeys(Vec<String>),
}

impl Selector {
    /// The elements chosen; `storey` gives an element's storey name.
    pub fn pick(
        &self,
        elements: &[Element],
        storey: &dyn Fn(usize) -> Option<String>,
    ) -> Vec<usize> {
        (0..elements.len())
            .filter(|&i| match self {
                Selector::All => true,
                Selector::Classes(c) => c.iter().any(|c| *c == elements[i].kind),
                Selector::Storeys(s) => storey(i).is_some_and(|n| s.contains(&n)),
            })
            .collect()
    }
}

// --- Status sidecar ------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    New,
    Active,
    Resolved,
}

/// A clash as kept between runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    pub a: String,
    pub b: String,
    pub kind: Kind,
    pub status: Status,
    /// The depth or distance last found, metres.
    pub depth_m: f64,
    /// Set by hand (kept when the clash is found again).
    #[serde(default)]
    pub note: Option<String>,
}

/// `<model>.specular-clashes.json`: statuses by element pair.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sidecar {
    pub version: u32,
    pub model: String,
    pub clashes: Vec<Kept>,
}

/// Where the sidecar of `model` goes.
pub fn sidecar_path(model: &Path) -> PathBuf {
    let mut s = model.as_os_str().to_owned();
    s.push(".specular-clashes.json");
    PathBuf::from(s)
}

fn key(a: &str, b: &str, k: Kind) -> (String, String, bool) {
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    (x.to_owned(), y.to_owned(), k == Kind::Hard)
}

impl Sidecar {
    pub fn load(path: &Path) -> Sidecar {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }

    /// Takes in a run's clashes as (GUID a, GUID b, kind, depth in metres): found again is
    /// active, new is new, gone is resolved. Returns each found clash's status, in order.
    pub fn update(&mut self, found: &[(String, String, Kind, f64)]) -> Vec<Status> {
        let mut prev: BTreeMap<(String, String, bool), Kept> = self
            .clashes
            .drain(..)
            .map(|k| (key(&k.a, &k.b, k.kind), k))
            .collect();
        let mut out = Vec::with_capacity(found.len());
        let mut now: Vec<Kept> = Vec::with_capacity(found.len());
        let mut seen = HashMap::new();
        for (a, b, kind, d) in found {
            let k = key(a, b, *kind);
            if seen.insert(k.clone(), ()).is_some() {
                out.push(Status::Active);
                continue;
            }
            let (status, note) = match prev.remove(&k) {
                Some(p) => (Status::Active, p.note),
                None => (Status::New, None),
            };
            out.push(status);
            now.push(Kept {
                a: k.0.clone(),
                b: k.1.clone(),
                kind: *kind,
                status,
                depth_m: *d,
                note,
            });
        }
        for (_, mut p) in prev {
            p.status = Status::Resolved;
            now.push(p);
        }
        self.version = 1;
        self.clashes = now;
        out
    }

    /// The status of one pair, if known.
    pub fn status(&self, a: &str, b: &str, kind: Kind) -> Option<Status> {
        let k = key(a, b, kind);
        self.clashes
            .iter()
            .find(|c| key(&c.a, &c.b, c.kind) == k)
            .map(|c| c.status)
    }

    /// Sets a pair's status by hand.
    pub fn set(&mut self, a: &str, b: &str, kind: Kind, s: Status) {
        let k = key(a, b, kind);
        if let Some(c) = self
            .clashes
            .iter_mut()
            .find(|c| key(&c.a, &c.b, c.kind) == k)
        {
            c.status = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_mesh::ifc::Element;

    /// A box's 8 corners and 12 triangles, outward.
    fn cube(pos: &mut Vec<[f32; 3]>, idx: &mut Vec<u32>, lo: [f32; 3], hi: [f32; 3]) -> (u32, u32) {
        let first = pos.len() as u32;
        for k in 0..8 {
            pos.push([
                if k & 1 == 0 { lo[0] } else { hi[0] },
                if k & 2 == 0 { lo[1] } else { hi[1] },
                if k & 4 == 0 { lo[2] } else { hi[2] },
            ]);
        }
        let faces = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ];
        for f in faces {
            idx.extend([f[0], f[1], f[2], f[0], f[2], f[3]].map(|v| v + first));
        }
        (first, 8)
    }

    fn model(boxes: &[([f32; 3], [f32; 3])]) -> (Vec<[f32; 3]>, Vec<u32>, Vec<Element>) {
        let (mut pos, mut idx, mut els) = (Vec::new(), Vec::new(), Vec::new());
        for (i, (lo, hi)) in boxes.iter().enumerate() {
            let (first, n) = cube(&mut pos, &mut idx, *lo, *hi);
            els.push(Element {
                id: i as u32 + 1,
                global_id: format!("G{i}"),
                kind: "IFCWALL".into(),
                first_vertex: first,
                vertices: n,
                ..Element::default()
            });
        }
        (pos, idx, els)
    }

    fn clashes(boxes: &[([f32; 3], [f32; 3])], mode: Mode) -> Vec<Clash> {
        let (pos, idx, els) = model(boxes);
        let m = Model {
            positions: &pos,
            indices: &idx,
            elements: &els,
        };
        let all: Vec<usize> = (0..els.len()).collect();
        run(&m, &all, &all, mode, &|_, _| false)
    }

    #[test]
    fn hard_clashes_need_depth() {
        let hard = Mode::Hard { tolerance: 0.01 };
        // Overlapping by 0.2 along x.
        let c = clashes(
            &[([0.0; 3], [1.0; 3]), ([0.8, 0.2, 0.2], [2.0, 0.8, 0.8])],
            hard,
        );
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, Kind::Hard);
        assert!((c[0].depth - 0.2).abs() < 1e-6, "{}", c[0].depth);
        // Touching faces: no clash.
        let c = clashes(
            &[([0.0; 3], [1.0; 3]), ([1.0, 0.0, 0.0], [2.0, 1.0, 1.0])],
            hard,
        );
        assert!(c.is_empty(), "{c:?}");
        // Barely in (5 mm, under the 10 mm tolerance): no clash.
        let c = clashes(
            &[([0.0; 3], [1.0; 3]), ([0.995, 0.2, 0.2], [2.0, 0.8, 0.8])],
            hard,
        );
        assert!(c.is_empty(), "{c:?}");
        // One wholly inside the other.
        let c = clashes(&[([0.0; 3], [1.0; 3]), ([0.4; 3], [0.6; 3])], hard);
        assert_eq!(c.len(), 1);
        assert!((c[0].depth - 0.4).abs() < 1e-6, "{}", c[0].depth);
        // Apart.
        let c = clashes(&[([0.0; 3], [1.0; 3]), ([3.0; 3], [4.0; 3])], hard);
        assert!(c.is_empty());
    }

    #[test]
    fn copies_of_one_element_clash() {
        let hard = Mode::Hard { tolerance: 0.01 };
        // The same 0.3 m slab twice (one model's copy of another's).
        let c = clashes(
            &[([0.0; 3], [4.0, 3.0, 0.3]), ([0.0; 3], [4.0, 3.0, 0.3])],
            hard,
        );
        assert_eq!(c.len(), 1, "{c:?}");
        assert!((c[0].depth - 0.3).abs() < 1e-6, "{}", c[0].depth);
        // A copy 5 mm thin is under the tolerance: no clash.
        let c = clashes(
            &[([0.0; 3], [4.0, 3.0, 0.005]), ([0.0; 3], [4.0, 3.0, 0.005])],
            hard,
        );
        assert!(c.is_empty(), "{c:?}");
    }

    #[test]
    fn a_duct_through_a_wall_clashes() {
        let hard = Mode::Hard { tolerance: 0.01 };
        // A 0.2 m duct right through a 0.3 m wall: no vertex of either inside the other.
        let c = clashes(
            &[
                ([0.0, 0.0, 0.0], [4.0, 0.3, 3.0]),
                ([1.0, -1.0, 1.0], [1.2, 1.3, 1.2]),
            ],
            hard,
        );
        assert_eq!(c.len(), 1, "{c:?}");
        assert!(
            c[0].depth > 0.1 && c[0].depth <= 0.2 + 1e-9,
            "{}",
            c[0].depth
        );
    }

    #[test]
    fn clearance_finds_near_misses() {
        let clear = Mode::Clearance {
            distance: 0.1,
            tolerance: 0.01,
        };
        let c = clashes(
            &[([0.0; 3], [1.0; 3]), ([1.05, 0.0, 0.0], [2.0, 1.0, 1.0])],
            clear,
        );
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, Kind::Clearance);
        assert!((c[0].depth - 0.05).abs() < 1e-6, "{}", c[0].depth);
        // A diagonal gap: corner to corner.
        let c = clashes(
            &[([0.0; 3], [1.0; 3]), ([1.03, 1.04, 0.0], [2.0, 2.0, 1.0])],
            clear,
        );
        assert!((c[0].depth - 0.05).abs() < 1e-6, "{}", c[0].depth);
        let c = clashes(
            &[([0.0; 3], [1.0; 3]), ([1.2, 0.0, 0.0], [2.0, 1.0, 1.0])],
            clear,
        );
        assert!(c.is_empty());
        // A hard clash is reported as hard under a clearance test too.
        let c = clashes(
            &[([0.0; 3], [1.0; 3]), ([0.5, 0.0, 0.0], [2.0, 1.0, 1.0])],
            clear,
        );
        assert_eq!(c[0].kind, Kind::Hard);
    }

    #[test]
    fn sets_and_skips() {
        let (pos, idx, els) = model(&[
            ([0.0; 3], [1.0; 3]),
            ([0.5, 0.0, 0.0], [1.5, 1.0, 1.0]),
            ([1.2, 0.0, 0.0], [2.0, 1.0, 1.0]),
        ]);
        let m = Model {
            positions: &pos,
            indices: &idx,
            elements: &els,
        };
        let hard = Mode::Hard { tolerance: 0.01 };
        // Only 0 against the rest.
        let c = run(&m, &[0], &[1, 2], hard, &|_, _| false);
        assert_eq!(
            c.iter().map(|c| (c.a, c.b)).collect::<Vec<_>>(),
            vec![(0, 1)]
        );
        let all = [0, 1, 2];
        let c = run(&m, &all, &all, hard, &|_, _| false);
        assert_eq!(c.len(), 2);
        let c = run(&m, &all, &all, hard, &|a, b| (a, b) == (0, 1));
        assert_eq!(
            c.iter().map(|c| (c.a, c.b)).collect::<Vec<_>>(),
            vec![(1, 2)]
        );
    }

    #[test]
    fn statuses_follow_runs() {
        let mut s = Sidecar::default();
        let r1 = s.update(&[
            ("A".into(), "B".into(), Kind::Hard, 0.1),
            ("C".into(), "D".into(), Kind::Hard, 0.2),
        ]);
        assert_eq!(r1, vec![Status::New, Status::New]);
        let r2 = s.update(&[("B".into(), "A".into(), Kind::Hard, 0.1)]);
        assert_eq!(r2, vec![Status::Active]);
        assert_eq!(s.status("C", "D", Kind::Hard), Some(Status::Resolved));
        let back: Sidecar = serde_json::from_slice(&s.to_json()).unwrap();
        assert_eq!(back, s);
        assert_eq!(
            sidecar_path(Path::new("/m/a.ifc")),
            PathBuf::from("/m/a.ifc.specular-clashes.json")
        );
    }
}
