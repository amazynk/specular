//! Mesh tools for segment export: a closed-manifold check, connected parts, Taubin smoothing
//! that keeps each part's volume, and decimation to a triangle count.
//!
//! Decimation is edge collapse ordered by the quadric error metric (Garland and Heckbert):
//! each vertex carries the sum of its faces' plane quadrics, an edge collapses to the point
//! minimising their sum, cheapest first. A collapse is skipped when it would change the
//! topology (the link condition: the edge's ends share exactly the two vertices opposite it,
//! and no face would be doubled) or flip or flatten a face, so a closed surface stays closed.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::iso::Mesh;
use crate::{cross, dot};

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// A plane quadric's ten coefficients (a², ab, ac, ad, b², bc, bd, c², cd, d²).
type Quadric = [f64; 10];

fn plane_quadric(n: V3, d: f64, w: f64) -> Quadric {
    let [a, b, c] = n;
    [
        a * a * w,
        a * b * w,
        a * c * w,
        a * d * w,
        b * b * w,
        b * c * w,
        b * d * w,
        c * c * w,
        c * d * w,
        d * d * w,
    ]
}

fn error(q: &Quadric, v: V3) -> f64 {
    let [x, y, z] = v;
    q[0] * x * x
        + 2.0 * q[1] * x * y
        + 2.0 * q[2] * x * z
        + 2.0 * q[3] * x
        + q[4] * y * y
        + 2.0 * q[5] * y * z
        + 2.0 * q[6] * y
        + q[7] * z * z
        + 2.0 * q[8] * z
        + q[9]
}

/// The point minimising `q`, if its 3 × 3 system is well conditioned.
fn optimum(q: &Quadric) -> Option<V3> {
    let m = [[q[0], q[1], q[2]], [q[1], q[4], q[5]], [q[2], q[5], q[7]]];
    let r = [-q[3], -q[6], -q[8]];
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let scale = q[0].abs() + q[4].abs() + q[7].abs();
    let floor = 1e-9 * scale.powi(3).max(1e-300);
    if det.is_nan() || det.abs() <= floor {
        return None;
    }
    let col = |c: usize| {
        let mut a = m;
        for (row, v) in a.iter_mut().zip(r) {
            row[c] = v;
        }
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
    };
    Some([col(0) / det, col(1) / det, col(2) / det])
}

#[derive(PartialEq)]
struct Candidate {
    cost: f64,
    a: u32,
    b: u32,
    stamps: (u32, u32),
    to: V3,
}

impl Eq for Candidate {}

impl Ord for Candidate {
    fn cmp(&self, o: &Self) -> Ordering {
        // Cheapest first from a max-heap; ties by vertex so the order is deterministic.
        o.cost
            .total_cmp(&self.cost)
            .then_with(|| o.a.cmp(&self.a))
            .then_with(|| o.b.cmp(&self.b))
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Mesh {
    /// Every edge joins exactly two triangles, once in each direction (a closed, oriented
    /// 2-manifold surface: watertight), and no triangle repeats a vertex.
    pub fn is_closed(&self) -> bool {
        let mut directed: HashMap<(u32, u32), u32> =
            HashMap::with_capacity(self.triangles.len() * 3);
        for t in &self.triangles {
            if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                return false;
            }
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *directed.entry((a, b)).or_default() += 1;
            }
        }
        directed
            .iter()
            .all(|(&(a, b), &n)| n == 1 && directed.get(&(b, a)) == Some(&1))
    }

    /// Each vertex's connected part (numbered from 0), and the number of parts.
    pub fn parts(&self) -> (Vec<u32>, usize) {
        let n = self.vertices.len();
        let mut parent: Vec<u32> = (0..n as u32).collect();
        fn find(p: &mut [u32], mut x: u32) -> u32 {
            while p[x as usize] != x {
                p[x as usize] = p[p[x as usize] as usize];
                x = p[x as usize];
            }
            x
        }
        for t in &self.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2])] {
                let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                if ra != rb {
                    parent[ra.max(rb) as usize] = ra.min(rb);
                }
            }
        }
        let mut ids: HashMap<u32, u32> = HashMap::new();
        let mut out = Vec::with_capacity(n);
        for v in 0..n as u32 {
            let r = find(&mut parent, v);
            let next = ids.len() as u32;
            out.push(*ids.entry(r).or_insert(next));
        }
        let count = ids.len();
        (out, count)
    }

    /// Each part's enclosed volume (mm³), by [`Mesh::parts`].
    fn part_volumes(&self, part: &[u32], count: usize) -> Vec<f64> {
        let mut out = vec![0.0; count];
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| self.vertices[i as usize]);
            out[part[t[0] as usize] as usize] += dot(a, cross(b, c)) / 6.0;
        }
        out
    }

    /// [`Mesh::smooth`], then each connected part scaled about its centre back to the
    /// volume it had, so smoothing neither shrinks nor grows what is printed.
    pub fn smooth_keeping_volume(&mut self, passes: u32) {
        let (part, count) = self.parts();
        let before = self.part_volumes(&part, count);
        self.smooth(passes);
        let after = self.part_volumes(&part, count);
        let mut centre = vec![[0.0; 3]; count];
        let mut n = vec![0usize; count];
        for (v, &p) in self.vertices.iter().zip(&part) {
            for c in 0..3 {
                centre[p as usize][c] += v[c];
            }
            n[p as usize] += 1;
        }
        for (c, &k) in centre.iter_mut().zip(&n) {
            *c = c.map(|x| x / k.max(1) as f64);
        }
        for (v, &p) in self.vertices.iter_mut().zip(&part) {
            let p = p as usize;
            if before[p].abs() < 1e-12 || after[p].abs() < 1e-12 || before[p] * after[p] < 0.0 {
                continue;
            }
            let k = (before[p] / after[p]).cbrt();
            *v = [0, 1, 2].map(|c| centre[p][c] + (v[c] - centre[p][c]) * k);
        }
    }

    /// Collapses edges until at most `target` triangles are left (or no collapse keeps the
    /// surface valid). Unused vertices are dropped.
    pub fn decimate(&mut self, target: usize) {
        if self.triangles.len() <= target {
            return;
        }
        let n = self.vertices.len();
        let mut q = vec![[0.0; 10]; n];
        let mut vt: Vec<Vec<u32>> = vec![Vec::new(); n];
        for (i, t) in self.triangles.iter().enumerate() {
            let [a, b, c] = t.map(|i| self.vertices[i as usize]);
            let nn = cross(sub(b, a), sub(c, a));
            let l = len(nn);
            if l > 0.0 {
                let u = nn.map(|x| x / l);
                let pq = plane_quadric(u, -dot(u, a), l / 2.0);
                for &v in t {
                    for (k, x) in pq.iter().enumerate() {
                        q[v as usize][k] += x;
                    }
                }
            }
            for &v in t {
                vt[v as usize].push(i as u32);
            }
        }
        let mut alive_t = vec![true; self.triangles.len()];
        let mut alive_v = vec![true; n];
        let mut stamp = vec![0u32; n];
        let mut heap = BinaryHeap::new();
        let candidate = |verts: &[V3], q: &[Quadric], stamp: &[u32], a: u32, b: u32| {
            let mut qs = q[a as usize];
            for (k, x) in q[b as usize].iter().enumerate() {
                qs[k] += x;
            }
            let (pa, pb) = (verts[a as usize], verts[b as usize]);
            let mid = [0, 1, 2].map(|c| (pa[c] + pb[c]) / 2.0);
            let edge = len(sub(pa, pb));
            let mut best = [pa, pb, mid]
                .into_iter()
                .map(|p| (error(&qs, p), p))
                .min_by(|x, y| x.0.total_cmp(&y.0))
                .expect("three choices");
            if let Some(o) = optimum(&qs).filter(|o| len(sub(*o, mid)) <= edge) {
                let e = error(&qs, o);
                if e < best.0 {
                    best = (e, o);
                }
            }
            Candidate {
                cost: best.0.max(0.0),
                a,
                b,
                stamps: (stamp[a as usize], stamp[b as usize]),
                to: best.1,
            }
        };
        {
            let mut edges: Vec<(u32, u32)> = Vec::with_capacity(self.triangles.len() * 3 / 2);
            for t in &self.triangles {
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    if a < b {
                        edges.push((a, b));
                    }
                }
            }
            edges.sort_unstable();
            edges.dedup();
            for (a, b) in edges {
                heap.push(candidate(&self.vertices, &q, &stamp, a, b));
            }
        }
        let mut count = self.triangles.len();
        while count > target {
            let Some(c) = heap.pop() else { break };
            let (a, b) = (c.a as usize, c.b as usize);
            if !alive_v[a] || !alive_v[b] || (stamp[a], stamp[b]) != c.stamps {
                continue;
            }
            if !self.can_collapse(&vt, &alive_t, c.a, c.b, c.to) {
                continue;
            }
            // b joins a at the new point.
            self.vertices[a] = c.to;
            let qb = q[b];
            for (k, x) in qb.iter().enumerate() {
                q[a][k] += x;
            }
            let around_b = std::mem::take(&mut vt[b]);
            for t in around_b {
                if !alive_t[t as usize] {
                    continue;
                }
                let tri = &mut self.triangles[t as usize];
                if tri.contains(&c.a) {
                    alive_t[t as usize] = false;
                    count -= 1;
                } else {
                    for v in tri.iter_mut() {
                        if *v == c.b {
                            *v = c.a;
                        }
                    }
                    vt[a].push(t);
                }
            }
            alive_v[b] = false;
            vt[a].retain(|&t| alive_t[t as usize]);
            vt[a].sort_unstable();
            vt[a].dedup();
            stamp[a] += 1;
            let mut nb: Vec<u32> = vt[a]
                .iter()
                .flat_map(|&t| self.triangles[t as usize])
                .filter(|&v| v != c.a)
                .collect();
            nb.sort_unstable();
            nb.dedup();
            for u in nb {
                vt[u as usize].retain(|&t| alive_t[t as usize]);
                let (x, y) = (c.a.min(u), c.a.max(u));
                heap.push(candidate(&self.vertices, &q, &stamp, x, y));
            }
        }
        // Compact.
        let mut remap = vec![u32::MAX; n];
        let mut vertices = Vec::new();
        let mut triangles = Vec::with_capacity(count);
        for (t, alive) in self.triangles.iter().zip(&alive_t) {
            if !alive {
                continue;
            }
            triangles.push(t.map(|v| {
                let r = &mut remap[v as usize];
                if *r == u32::MAX {
                    *r = vertices.len() as u32;
                    vertices.push(self.vertices[v as usize]);
                }
                *r
            }));
        }
        self.vertices = vertices;
        self.triangles = triangles;
    }

    /// Whether collapsing edge (`a`, `b`) to `to` keeps the surface a valid closed manifold
    /// with no face flipped or flattened.
    fn can_collapse(&self, vt: &[Vec<u32>], alive: &[bool], a: u32, b: u32, to: V3) -> bool {
        let live = |v: u32| {
            vt[v as usize]
                .iter()
                .copied()
                .filter(|&t| alive[t as usize])
        };
        let ring = |v: u32| {
            let mut r: Vec<u32> = live(v)
                .flat_map(|t| self.triangles[t as usize])
                .filter(|&x| x != v)
                .collect();
            r.sort_unstable();
            r.dedup();
            r
        };
        let (ra, rb) = (ring(a), ring(b));
        let shared = ra.iter().filter(|x| rb.binary_search(x).is_ok()).count();
        let both = live(b)
            .filter(|&t| self.triangles[t as usize].contains(&a))
            .count();
        if both != 2 || shared != 2 {
            return false;
        }
        // A face of b moved onto a must not repeat one a already has (a tetrahedron, or a
        // fold that would leave two faces back to back).
        let key = |t: [u32; 3]| {
            let mut k = t;
            k.sort_unstable();
            k
        };
        let faces_a: Vec<[u32; 3]> = live(a).map(|t| key(self.triangles[t as usize])).collect();
        for t in live(b) {
            let tri = self.triangles[t as usize];
            if tri.contains(&a) {
                continue;
            }
            let moved = key(tri.map(|v| if v == b { a } else { v }));
            if faces_a.contains(&moved) {
                return false;
            }
        }
        // No surviving face flips or collapses to a sliver.
        for v in [a, b] {
            for t in live(v) {
                let tri = self.triangles[t as usize];
                if tri.contains(&a) && tri.contains(&b) {
                    continue;
                }
                let p = tri.map(|x| self.vertices[x as usize]);
                let moved = tri.map(|x| {
                    if x == a || x == b {
                        to
                    } else {
                        self.vertices[x as usize]
                    }
                });
                let n0 = cross(sub(p[1], p[0]), sub(p[2], p[0]));
                let n1 = cross(sub(moved[1], moved[0]), sub(moved[2], moved[0]));
                let (l0, l1) = (len(n0), len(n1));
                if l1 <= 1e-12 * l0.max(1e-30) || l1 == 0.0 {
                    return false;
                }
                if dot(n0, n1) < 0.2 * l0 * l1 {
                    return false;
                }
                // Slivers: a face's least height against its longest edge may not drop
                // below 2 % unless it already was that thin.
                let thin = |q: [V3; 3], l: f64| {
                    let longest = len(sub(q[1], q[0]))
                        .max(len(sub(q[2], q[1])))
                        .max(len(sub(q[0], q[2])))
                        .max(1e-30);
                    l / (longest * longest)
                };
                let (r0, r1) = (thin(p, l0), thin(moved, l1));
                if r1 < 0.02 && r1 < r0 {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
#[path = "mesh_ops_tests.rs"]
mod tests;
