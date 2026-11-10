//! A bounding volume hierarchy over a model's triangles or a cloud's points, in the canonical
//! frame (z up): what walking (floors under the feet, walls against the body) and section
//! caps (only the triangles a plane crosses) ask of a big model, in well under a frame.
//!
//! Built once per model, off the window thread: primitives are split at the median of their
//! centres along the box's longest side down to [`LEAF`] per leaf (a binned SAH would buy a
//! little traversal speed for a slower build; the queries here are few per frame).

use rayon::prelude::*;
use specular_mesh::Mesh;
use specular_mesh::preview::canonical;

use crate::camera::{V3, cross, dot, sub};

/// Primitives per leaf at most.
const LEAF: usize = 4;

/// Points a cloud keeps for collision at most (an even subsample beyond it).
pub const MAX_NAV_POINTS: usize = 4_000_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb {
        min: [f32::INFINITY; 3],
        max: [f32::NEG_INFINITY; 3],
    };

    pub fn add(&mut self, p: [f32; 3]) {
        for (k, v) in p.into_iter().enumerate() {
            self.min[k] = self.min[k].min(v);
            self.max[k] = self.max[k].max(v);
        }
    }

    fn union(&self, o: &Aabb) -> Aabb {
        Aabb {
            min: std::array::from_fn(|k| self.min[k].min(o.min[k])),
            max: std::array::from_fn(|k| self.max[k].max(o.max[k])),
        }
    }

    pub fn overlaps(&self, o: &Aabb) -> bool {
        (0..3).all(|k| self.min[k] <= o.max[k] && o.min[k] <= self.max[k])
    }

    fn centre(&self, k: usize) -> f32 {
        (self.min[k] + self.max[k]) * 0.5
    }

    /// Where a ray from `o` along `inv` (1 / direction) enters, if before `tmax`.
    fn hit(&self, o: V3, inv: V3, tmax: f64) -> Option<f64> {
        let (mut t0, mut t1) = (0f64, tmax);
        for k in 0..3 {
            let a = (f64::from(self.min[k]) - o[k]) * inv[k];
            let b = (f64::from(self.max[k]) - o[k]) * inv[k];
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            // NaN (a zero direction on the slab's plane) keeps the interval.
            if lo > t0 {
                t0 = lo;
            }
            if hi < t1 {
                t1 = hi;
            }
            if t0 > t1 {
                return None;
            }
        }
        Some(t0)
    }

    /// The signed side of a plane `n·p = d` the box is on: -1 below, 1 above, 0 across.
    pub fn side(&self, n: V3, d: f64) -> i32 {
        let c: V3 = std::array::from_fn(|k| f64::from(self.centre(k)));
        let e: V3 = std::array::from_fn(|k| f64::from(self.max[k] - self.min[k]) * 0.5);
        let r = e[0] * n[0].abs() + e[1] * n[1].abs() + e[2] * n[2].abs();
        let s = dot(n, c) - d;
        if s > r {
            1
        } else if s < -r {
            -1
        } else {
            0
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Node {
    b: Aabb,
    /// Leaf: first primitive in `order`; inner: the right child (the left is next).
    index: u32,
    /// Primitives in a leaf, 0 for an inner node.
    count: u32,
}

/// The hierarchy over primitive boxes; primitives are named by their index.
#[derive(Clone, Debug, Default)]
pub struct Bvh {
    nodes: Vec<Node>,
    order: Vec<u32>,
}

impl Bvh {
    /// Over `boxes` (primitive `i` is `boxes[i]`).
    pub fn build(boxes: &[Aabb]) -> Bvh {
        let mut items: Vec<(u32, [f32; 3])> = boxes
            .par_iter()
            .enumerate()
            .map(|(i, b)| (i as u32, std::array::from_fn(|k| b.centre(k))))
            .collect();
        let mut nodes = Vec::with_capacity(2 * boxes.len() / LEAF + 1);
        if !items.is_empty() {
            split(&mut nodes, boxes, &mut items[..], 0);
        }
        Bvh {
            nodes,
            order: items.into_iter().map(|(i, _)| i).collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The nearest primitive a ray from `o` along unit `d` meets before `tmax`, by `hit`
    /// (the distance along the ray where primitive `i` is met), and that distance.
    pub fn ray(
        &self,
        o: V3,
        d: V3,
        tmax: f64,
        mut hit: impl FnMut(u32) -> Option<f64>,
    ) -> Option<(u32, f64)> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = d.map(|c| 1.0 / c);
        let mut best: Option<(u32, f64)> = None;
        let mut far = tmax;
        let mut stack = Vec::with_capacity(64);
        stack.push(0usize);
        while let Some(i) = stack.pop() {
            let n = &self.nodes[i];
            if n.b.hit(o, inv, far).is_none() {
                continue;
            }
            if n.count > 0 {
                let s = n.index as usize;
                for &p in &self.order[s..s + n.count as usize] {
                    if let Some(t) = hit(p).filter(|t| *t >= 0.0 && *t < far) {
                        far = t;
                        best = Some((p, t));
                    }
                }
            } else {
                let (l, r) = (i + 1, n.index as usize);
                // Nearer child last, so it is visited first.
                let tl = self.nodes[l].b.hit(o, inv, far);
                let tr = self.nodes[r].b.hit(o, inv, far);
                match (tl, tr) {
                    (Some(a), Some(b)) if a <= b => stack.extend([r, l]),
                    (Some(_), Some(_)) => stack.extend([l, r]),
                    (Some(_), None) => stack.push(l),
                    (None, Some(_)) => stack.push(r),
                    (None, None) => {}
                }
            }
        }
        best
    }

    /// Every primitive whose box overlaps `q`.
    pub fn overlap(&self, q: &Aabb, mut f: impl FnMut(u32)) {
        self.visit(|b| b.overlaps(q), &mut f);
    }

    /// Every primitive under nodes `keep` accepts (a box test), handed to `f`.
    pub fn visit(&self, keep: impl Fn(&Aabb) -> bool, mut f: impl FnMut(u32)) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let n = &self.nodes[i];
            if !keep(&n.b) {
                continue;
            }
            if n.count > 0 {
                let s = n.index as usize;
                for &p in &self.order[s..s + n.count as usize] {
                    f(p);
                }
            } else {
                stack.push(n.index as usize);
                stack.push(i + 1);
            }
        }
    }
}

fn split(nodes: &mut Vec<Node>, boxes: &[Aabb], items: &mut [(u32, [f32; 3])], first: usize) {
    let me = nodes.len();
    let b = items
        .iter()
        .fold(Aabb::EMPTY, |a, (i, _)| a.union(&boxes[*i as usize]));
    nodes.push(Node {
        b,
        index: first as u32,
        count: items.len() as u32,
    });
    if items.len() <= LEAF {
        return;
    }
    // The longest side of the centres' box.
    let mut cb = Aabb::EMPTY;
    for (_, c) in items.iter() {
        cb.add(*c);
    }
    let ext: [f32; 3] = std::array::from_fn(|k| cb.max[k] - cb.min[k]);
    let axis = (0..3)
        .max_by(|&a, &b| ext[a].total_cmp(&ext[b]))
        .unwrap_or(0);
    let mid = items.len() / 2;
    items.select_nth_unstable_by(mid, |a, b| a.1[axis].total_cmp(&b.1[axis]));
    let (l, r) = items.split_at_mut(mid);
    split(nodes, boxes, l, first);
    let right = nodes.len();
    split(nodes, boxes, r, first + mid);
    nodes[me].index = right as u32;
    nodes[me].count = 0;
}

/// What walking collides with: a model's triangles, a cloud's points (each a small
/// sphere) or both, in the canonical frame, under a [`Bvh`] (triangles first, then points).
#[derive(Clone, Debug, Default)]
pub struct NavScene {
    pub bvh: Bvh,
    tris: Vec<[[f32; 3]; 3]>,
    points: Vec<[f32; 3]>,
    /// A point's radius (model units).
    point_radius: f32,
}

impl NavScene {
    /// The triangles of `mesh` in the canonical frame.
    pub fn from_mesh(mesh: &Mesh) -> NavScene {
        NavScene::from_parts(triangles(mesh), Vec::new(), 0.0)
    }

    /// Points (canonical frame) as spheres of `radius`, at most [`MAX_NAV_POINTS`] of them.
    pub fn from_points(points: &[[f32; 3]], radius: f32) -> NavScene {
        NavScene::from_parts(Vec::new(), thin(points), radius)
    }

    /// A model's triangles and points (canonical frame, spheres of `radius`, at most
    /// [`MAX_NAV_POINTS`]) together: a tileset of models and points.
    pub fn from_mesh_and_points(mesh: &Mesh, points: &[[f32; 3]], radius: f32) -> NavScene {
        NavScene::from_parts(triangles(mesh), thin(points), radius)
    }

    fn from_parts(tris: Vec<[[f32; 3]; 3]>, points: Vec<[f32; 3]>, radius: f32) -> NavScene {
        let r = if points.is_empty() {
            0.0
        } else {
            radius.max(1e-6)
        };
        let mut boxes: Vec<Aabb> = tris
            .par_iter()
            .map(|t| {
                let mut b = Aabb::EMPTY;
                for p in t {
                    b.add(*p);
                }
                b
            })
            .collect();
        boxes.par_extend(points.par_iter().map(|p| Aabb {
            min: p.map(|v| v - r),
            max: p.map(|v| v + r),
        }));
        NavScene {
            bvh: Bvh::build(&boxes),
            tris,
            points,
            point_radius: r,
        }
    }

    pub fn triangle(&self, i: u32) -> Option<[V3; 3]> {
        self.tris
            .get(i as usize)
            .map(|t| t.map(|p| p.map(f64::from)))
    }

    pub fn len(&self) -> usize {
        self.tris.len() + self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How far along unit `d` from `o` the scene is met, within `tmax`.
    pub fn ray(&self, o: V3, d: V3, tmax: f64) -> Option<f64> {
        let r = f64::from(self.point_radius);
        let nt = self.tris.len();
        self.bvh
            .ray(o, d, tmax, |i| match self.tris.get(i as usize) {
                Some(t) => ray_triangle(o, d, *t),
                None => ray_sphere(o, d, self.points[i as usize - nt].map(f64::from), r),
            })
            .map(|h| h.1)
    }

    /// The push out of the scene's deepest contact with a capsule from `a` to `b` of
    /// `radius` (the caller limits which components it applies, and calls again for the
    /// next contact: summing contacts would push twice where two triangles share an edge).
    pub fn push_out(&self, a: V3, b: V3, radius: f64) -> Option<V3> {
        let mut q = Aabb::EMPTY;
        for p in [a, b] {
            q.add(p.map(|v| (v - radius) as f32));
            q.add(p.map(|v| (v + radius) as f32));
        }
        let mut push = [0.0; 3];
        let mut deepest = 0.0;
        let mut any = false;
        let mut contact = |c: V3, s: V3, r: f64| {
            let v = sub(s, c);
            let l = dot(v, v).sqrt();
            if l < r && l > 1e-12 && r - l > deepest {
                let k = (r - l) / l;
                deepest = r - l;
                push = v.map(|x| x * k);
                any = true;
            }
        };
        let pr = f64::from(self.point_radius);
        let nt = self.tris.len();
        self.bvh.overlap(&q, |i| match self.tris.get(i as usize) {
            Some(t) => {
                let (s, c) = segment_triangle(a, b, t.map(|p| p.map(f64::from)));
                contact(c, s, radius);
            }
            None => {
                let p = self.points[i as usize - nt].map(f64::from);
                let s = closest_on_segment(a, b, p);
                contact(p, s, radius + pr);
            }
        });
        any.then_some(push)
    }
}

/// The triangles of `mesh` in the canonical frame.
fn triangles(mesh: &Mesh) -> Vec<[[f32; 3]; 3]> {
    let up = mesh.up;
    mesh.indices
        .par_chunks_exact(3)
        .map(|t| {
            std::array::from_fn(|k| {
                canonical(
                    mesh.positions
                        .get(t[k] as usize)
                        .copied()
                        .unwrap_or_default(),
                    up,
                )
            })
        })
        .collect()
}

/// Every n-th point, at most [`MAX_NAV_POINTS`].
fn thin(points: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let stride = points.len().div_ceil(MAX_NAV_POINTS).max(1);
    points.iter().step_by(stride).copied().collect()
}

/// Möller–Trumbore, both sides.
pub(crate) fn ray_triangle(o: V3, d: V3, t: [[f32; 3]; 3]) -> Option<f64> {
    let [a, b, c] = t.map(|p| p.map(f64::from));
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-14 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(o, a);
    let u = dot(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(d, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = dot(e2, q) * inv;
    (t >= 0.0).then_some(t)
}

fn ray_sphere(o: V3, d: V3, c: V3, r: f64) -> Option<f64> {
    let m = sub(o, c);
    let b = dot(m, d);
    let cc = dot(m, m) - r * r;
    if cc > 0.0 && b > 0.0 {
        return None;
    }
    let disc = b * b - cc;
    if disc < 0.0 {
        return None;
    }
    Some((-b - disc.sqrt()).max(0.0))
}

fn closest_on_segment(a: V3, b: V3, p: V3) -> V3 {
    let ab = sub(b, a);
    let l = dot(ab, ab);
    let t = if l > 0.0 {
        (dot(sub(p, a), ab) / l).clamp(0.0, 1.0)
    } else {
        0.0
    };
    std::array::from_fn(|k| a[k] + ab[k] * t)
}

/// The closest point on triangle `t` to `p` (Ericson's region test).
pub(crate) fn closest_on_triangle(p: V3, t: [V3; 3]) -> V3 {
    let [a, b, c] = t;
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
        let v = d1 / (d1 - d3);
        return std::array::from_fn(|k| a[k] + ab[k] * v);
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return std::array::from_fn(|k| a[k] + ac[k] * w);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return std::array::from_fn(|k| b[k] + (c[k] - b[k]) * w);
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    std::array::from_fn(|k| a[k] + ab[k] * v + ac[k] * w)
}

/// The closest points between segment `a`–`b` and triangle `t`: (on the segment, on the
/// triangle). Iterates the two closest-point maps a few times from both ends and the
/// middle, which settles for the short, upright capsules walking uses.
pub(crate) fn segment_triangle(a: V3, b: V3, t: [V3; 3]) -> (V3, V3) {
    // Where the segment crosses the triangle, they touch.
    let ab = sub(b, a);
    let l = dot(ab, ab).sqrt();
    if l > 0.0 {
        let d = ab.map(|v| v / l);
        if let Some(h) = ray_triangle(a, d, t.map(|p| p.map(|v| v as f32)))
            && h <= l
        {
            let p = std::array::from_fn(|k| a[k] + d[k] * h);
            return (p, p);
        }
    }
    let mut best = (a, closest_on_triangle(a, t), f64::INFINITY);
    for start in [a, b, std::array::from_fn(|k| (a[k] + b[k]) / 2.0)] {
        let mut s = start;
        let mut c = closest_on_triangle(s, t);
        for _ in 0..4 {
            s = closest_on_segment(a, b, c);
            c = closest_on_triangle(s, t);
        }
        let v = sub(s, c);
        let dd = dot(v, v);
        if dd < best.2 {
            best = (s, c, dd);
        }
    }
    // The triangle's edges against the segment (an edge can be nearer than any corner map).
    for (p, q) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
        let (s, c) = segment_segment(a, b, p, q);
        let v = sub(s, c);
        let dd = dot(v, v);
        if dd < best.2 {
            best = (s, c, dd);
        }
    }
    (best.0, best.1)
}

/// The closest points between segments `p1`–`q1` and `p2`–`q2`.
fn segment_segment(p1: V3, q1: V3, p2: V3, q2: V3) -> (V3, V3) {
    let d1 = sub(q1, p1);
    let d2 = sub(q2, p2);
    let r = sub(p1, p2);
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);
    let (s, t);
    if a <= 1e-18 && e <= 1e-18 {
        return (p1, p2);
    }
    if a <= 1e-18 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = dot(d1, r);
        if e <= 1e-18 {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = dot(d1, d2);
            let denom = a * e - b * b;
            let mut s0 = if denom > 1e-18 {
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
    (
        std::array::from_fn(|k| p1[k] + d1[k] * s),
        std::array::from_fn(|k| p2[k] + d2[k] * t),
    )
}
