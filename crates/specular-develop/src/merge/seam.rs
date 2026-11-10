//! Where each frame of a panorama shows: a label per pixel of the output proxy.
//!
//! - **Start**: each pixel goes to the frame it lies deepest inside (the largest distance
//!   to that frame's border, a chamfer distance transform), a Voronoi split of the overlaps.
//! - **Graph cut** (Kwatra et al., "Graphcut textures", SIGGRAPH 2003): frames join the
//!   composite one at a time; over the overlap of the composite and the new frame a minimum
//!   cut picks which shows, with the cost of a seam between neighbours `p, q` the
//!   difference of the two pictures at both (log luma, plus their gradients), plus a large
//!   cost where the difference is beyond noise (the merge's z-test: something moved), so
//!   seams go around moving people. Pixels next to where only one side exists are tied to
//!   that side; a small pull toward the Voronoi start keeps the seam there where the
//!   pictures agree everywhere.
//! - **Max-flow**: Boykov & Kolmogorov, "An experimental comparison of min-cut/max-flow
//!   algorithms for energy minimization in vision", TPAMI 2004: two search trees grown from
//!   the terminals, augmented along the paths where they meet, orphans adopted.

// Grid loops read clearest with indices.
#![allow(clippy::needless_range_loop)]

use std::collections::VecDeque;

/// No frame shows here.
pub const NONE: u16 = u16::MAX;

/// A frame's picture on the output proxy: log luma (NaN where the frame does not reach)
/// and the noise of that log luma (standard deviation).
#[derive(Clone, Debug)]
pub struct Layer {
    pub v: Vec<f32>,
    pub sigma: Vec<f32>,
}

/// The chamfer (3-4) distance of each `true` pixel to the nearest `false` one or the
/// raster edge.
pub fn distance(mask: &[bool], w: usize, h: usize) -> Vec<f32> {
    let big = f32::MAX / 4.0;
    let mut d: Vec<f32> = mask.iter().map(|&m| if m { big } else { 0.0 }).collect();
    let at = |x: i64, y: i64, d: &[f32]| -> f32 {
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
            0.0
        } else {
            d[y as usize * w + x as usize]
        }
    };
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let i = y as usize * w + x as usize;
            if d[i] == 0.0 {
                continue;
            }
            let v = d[i]
                .min(at(x - 1, y, &d) + 3.0)
                .min(at(x, y - 1, &d) + 3.0)
                .min(at(x - 1, y - 1, &d) + 4.0)
                .min(at(x + 1, y - 1, &d) + 4.0);
            d[i] = v;
        }
    }
    for y in (0..h as i64).rev() {
        for x in (0..w as i64).rev() {
            let i = y as usize * w + x as usize;
            if d[i] == 0.0 {
                continue;
            }
            let v = d[i]
                .min(at(x + 1, y, &d) + 3.0)
                .min(at(x, y + 1, &d) + 3.0)
                .min(at(x + 1, y + 1, &d) + 4.0)
                .min(at(x - 1, y + 1, &d) + 4.0);
            d[i] = v;
        }
    }
    d
}

/// The Voronoi start: each pixel to the frame it is deepest inside.
pub fn voronoi(layers: &[Layer], w: usize, h: usize) -> Vec<u16> {
    let dists: Vec<Vec<f32>> = layers
        .iter()
        .map(|l| {
            let m: Vec<bool> = l.v.iter().map(|v| v.is_finite()).collect();
            distance(&m, w, h)
        })
        .collect();
    (0..w * h)
        .map(|i| {
            let mut best = (NONE, 0.0f32);
            for (k, d) in dists.iter().enumerate() {
                if d[i] > best.1 {
                    best = (k as u16, d[i]);
                }
            }
            best.0
        })
        .collect()
}

/// A flow network for [`max_flow`]: nodes with terminal capacities and directed edge pairs.
pub struct Graph {
    /// Source minus sink capacity of each node.
    tr: Vec<f64>,
    /// Flow straight from the source to the sink through a node's two terminals.
    through: f64,
    first: Vec<u32>,
    next: Vec<u32>,
    to: Vec<u32>,
    cap: Vec<f64>,
}

const NIL: u32 = u32::MAX;

impl Graph {
    pub fn new(nodes: usize) -> Graph {
        Graph {
            tr: vec![0.0; nodes],
            through: 0.0,
            first: vec![NIL; nodes],
            next: Vec::new(),
            to: Vec::new(),
            cap: Vec::new(),
        }
    }

    /// Capacities from the source and to the sink of node `p` (added to what it has).
    pub fn terminal(&mut self, p: usize, source: f64, sink: f64) {
        // The smaller of the two always flows; only the difference is left to route.
        let before = self.tr[p];
        self.tr[p] += source - sink;
        self.through += (before.abs() + source + sink - self.tr[p].abs()) / 2.0;
    }

    /// An edge `p → q` of capacity `pq` and its reverse of `qp`.
    pub fn edge(&mut self, p: usize, q: usize, pq: f64, qp: f64) {
        for (a, b, c) in [(p, q, pq), (q, p, qp)] {
            let e = self.to.len() as u32;
            self.to.push(b as u32);
            self.cap.push(c);
            self.next.push(self.first[a]);
            self.first[a] = e;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tree {
    Free,
    S,
    T,
}

/// Parent links: an edge index, the terminal, or none (orphan or free).
const TERMINAL: u32 = u32::MAX - 1;

/// The minimum cut of `g`: true for nodes on the source side; and the flow.
pub fn max_flow(mut g: Graph) -> (Vec<bool>, f64) {
    let n = g.tr.len();
    let mut tree = vec![Tree::Free; n];
    let mut parent = vec![NIL; n];
    let mut active: VecDeque<u32> = VecDeque::new();
    let mut is_active = vec![false; n];
    let mut flow = g.through;
    for p in 0..n {
        if g.tr[p] > 0.0 {
            tree[p] = Tree::S;
        } else if g.tr[p] < 0.0 {
            tree[p] = Tree::T;
        } else {
            continue;
        }
        parent[p] = TERMINAL;
        active.push_back(p as u32);
        is_active[p] = true;
    }
    let mut orphans: Vec<u32> = Vec::new();
    // Nodes known to reach a terminal since the last augmentation (BK's time stamps).
    let mut stamp = vec![0u32; n];
    let mut time = 1u32;
    let mut path: Vec<usize> = Vec::new();
    // Whether node `p` reaches a terminal through valid parents.
    let mut rooted = |p: usize, parent: &[u32], g: &Graph, time: u32| -> bool {
        path.clear();
        let mut k = p;
        let ok = loop {
            if stamp[k] == time {
                break true;
            }
            path.push(k);
            match parent[k] {
                TERMINAL => break true,
                NIL => break false,
                e => k = g.to[e as usize ^ 1] as usize,
            }
            if path.len() > n {
                break false;
            }
        };
        if ok {
            for &q in &path {
                stamp[q] = time;
            }
        }
        ok
    };
    while let Some(&p) = active.front() {
        let p = p as usize;
        if tree[p] == Tree::Free {
            active.pop_front();
            is_active[p] = false;
            continue;
        }
        // Grow from p until a path is found.
        let mut meet: Option<u32> = None;
        let mut e = g.first[p];
        while e != NIL {
            let q = g.to[e as usize] as usize;
            // Residual along the tree's direction.
            let res = if tree[p] == Tree::S {
                g.cap[e as usize]
            } else {
                g.cap[e as usize ^ 1]
            };
            if res > 0.0 {
                if tree[q] == Tree::Free {
                    tree[q] = tree[p];
                    // The edge into q from p (for S) or out of q to p (for T): kept as the
                    // edge from parent to child, e (p → q).
                    parent[q] = e;
                    if !is_active[q] {
                        active.push_back(q as u32);
                        is_active[q] = true;
                    }
                } else if tree[q] != tree[p] {
                    meet = Some(if tree[p] == Tree::S { e } else { e ^ 1 });
                    break;
                }
            }
            e = g.next[e as usize];
        }
        let Some(mid) = meet else {
            active.pop_front();
            is_active[p] = false;
            continue;
        };
        // The path: source … s ─mid→ t … sink. Parent edges point parent → child in both
        // trees, so in T the flow runs child → parent along the reverse edge.
        let s = g.to[mid as usize ^ 1] as usize;
        let t = g.to[mid as usize] as usize;
        let mut bottleneck = g.cap[mid as usize];
        let mut k = s;
        while parent[k] != TERMINAL {
            let pe = parent[k] as usize;
            bottleneck = bottleneck.min(g.cap[pe]);
            k = g.to[pe ^ 1] as usize;
        }
        bottleneck = bottleneck.min(g.tr[k]);
        let mut k = t;
        while parent[k] != TERMINAL {
            let pe = parent[k] as usize;
            bottleneck = bottleneck.min(g.cap[pe ^ 1]);
            k = g.to[pe ^ 1] as usize;
        }
        bottleneck = bottleneck.min(-g.tr[k]);
        // Augment.
        g.cap[mid as usize] -= bottleneck;
        g.cap[mid as usize ^ 1] += bottleneck;
        let mut k = s;
        while parent[k] != TERMINAL {
            let pe = parent[k] as usize;
            g.cap[pe] -= bottleneck;
            g.cap[pe ^ 1] += bottleneck;
            if g.cap[pe] <= 0.0 {
                parent[k] = NIL;
                orphans.push(k as u32);
            }
            k = g.to[pe ^ 1] as usize;
        }
        g.tr[k] -= bottleneck;
        if g.tr[k] <= 0.0 {
            parent[k] = NIL;
            orphans.push(k as u32);
        }
        let mut k = t;
        while parent[k] != TERMINAL {
            let pe = parent[k] as usize;
            g.cap[pe ^ 1] -= bottleneck;
            g.cap[pe] += bottleneck;
            if g.cap[pe ^ 1] <= 0.0 {
                parent[k] = NIL;
                orphans.push(k as u32);
            }
            k = g.to[pe ^ 1] as usize;
        }
        g.tr[k] += bottleneck;
        if g.tr[k] >= 0.0 {
            parent[k] = NIL;
            orphans.push(k as u32);
        }
        flow += bottleneck;
        time = time.wrapping_add(1).max(1);
        // Adoption.
        while let Some(o) = orphans.pop() {
            let o = o as usize;
            let side = tree[o];
            if side == Tree::Free {
                continue;
            }
            if g.tr[o] != 0.0 && (g.tr[o] > 0.0) == (side == Tree::S) {
                parent[o] = TERMINAL;
                continue;
            }
            // A new parent q in the same tree with residual q → o (S) or o → q (T),
            // itself rooted.
            let mut found = NIL;
            let mut e = g.first[o];
            while e != NIL {
                let q = g.to[e as usize] as usize;
                if tree[q] == side {
                    // e is o → q; e^1 is q → o.
                    let res = if side == Tree::S {
                        g.cap[e as usize ^ 1]
                    } else {
                        g.cap[e as usize]
                    };
                    if res > 0.0 && rooted(q, &parent, &g, time) {
                        found = e ^ 1;
                        break;
                    }
                }
                e = g.next[e as usize];
            }
            if found != NIL {
                parent[o] = found;
                continue;
            }
            // Free o; its children become orphans, neighbours that could feed it active.
            let mut e = g.first[o];
            while e != NIL {
                let q = g.to[e as usize] as usize;
                if tree[q] == side {
                    let res = if side == Tree::S {
                        g.cap[e as usize ^ 1]
                    } else {
                        g.cap[e as usize]
                    };
                    if res > 0.0 && !is_active[q] {
                        active.push_back(q as u32);
                        is_active[q] = true;
                    }
                    let pq = parent[q];
                    if pq != NIL && pq != TERMINAL && g.to[pq as usize ^ 1] as usize == o {
                        parent[q] = NIL;
                        orphans.push(q as u32);
                    }
                }
                e = g.next[e as usize];
            }
            tree[o] = Tree::Free;
            // Rooted marks may run through o.
            time = time.wrapping_add(1).max(1);
            parent[o] = NIL;
        }
    }
    ((0..n).map(|p| tree[p] == Tree::S).collect(), flow)
}

/// The z beyond which a difference counts as motion.
const MOTION_Z: f32 = 4.0;
/// The seam cost added where something moved.
const MOTION_COST: f64 = 50.0;
/// The pull toward the Voronoi start, per pixel.
const PULL: f64 = 1e-3;

/// The difference cost of showing one picture or the other at pixel `i`.
fn cost(a: &Layer, b: &Layer, i: usize, j: usize) -> f64 {
    let d = f64::from((a.v[i] - b.v[i]).abs());
    // The gradient difference toward the neighbour.
    let ga = a.v[j] - a.v[i];
    let gb = b.v[j] - b.v[i];
    let g = if ga.is_finite() && gb.is_finite() {
        f64::from((ga - gb).abs())
    } else {
        0.0
    };
    let s = (a.sigma[i].powi(2) + b.sigma[i].powi(2)).sqrt().max(1e-6);
    let moved = (a.v[i] - b.v[i]).abs() / s > MOTION_Z;
    d + g + if moved { MOTION_COST } else { 0.0 }
}

/// The labels of `layers` on a `w × h` proxy (see the module documentation).
pub fn seams(layers: &[Layer], w: usize, h: usize) -> Vec<u16> {
    let start = voronoi(layers, w, h);
    let mut label = vec![NONE; w * h];
    for (k, l) in layers.iter().enumerate() {
        let k = k as u16;
        let valid = |i: usize| l.v[i].is_finite();
        // Pixels only this frame reaches take it.
        let mut overlap: Vec<u32> = Vec::new();
        for i in 0..w * h {
            if !valid(i) {
                continue;
            }
            if label[i] == NONE {
                label[i] = k;
            } else {
                overlap.push(i as u32);
            }
        }
        if overlap.is_empty() {
            continue;
        }
        // Only the overlap goes into the graph.
        let mut node = vec![u32::MAX; w * h];
        for (n, &i) in overlap.iter().enumerate() {
            node[i as usize] = n as u32;
        }
        let mut g = Graph::new(overlap.len());
        for (n, &i) in overlap.iter().enumerate() {
            let i = i as usize;
            let (x, y) = (i % w, i / w);
            let a = &layers[label[i] as usize];
            // Ties to the side that exists alone next door.
            for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                if node[j] != u32::MAX {
                    continue;
                }
                let composite = label[j] != NONE && label[j] != k;
                if composite && !valid(j) {
                    g.terminal(n, 1e9, 0.0);
                } else if valid(j) && label[j] == k {
                    g.terminal(n, 0.0, 1e9);
                }
            }
            if start[i] == k {
                g.terminal(n, 0.0, PULL);
            } else {
                g.terminal(n, PULL, 0.0);
            }
            // Seam costs to the right and below.
            for (dx, dy) in [(1usize, 0usize), (0, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= w || ny >= h {
                    continue;
                }
                let j = ny * w + nx;
                if node[j] == u32::MAX {
                    continue;
                }
                let aj = &layers[label[j] as usize];
                let c = cost(a, l, i, j) + cost(aj, l, j, i);
                g.edge(n, node[j] as usize, c, c);
            }
        }
        let (source, _) = max_flow(g);
        for (n, &i) in overlap.iter().enumerate() {
            if !source[n] {
                label[i as usize] = k;
            }
        }
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_flow_of_a_small_graph() {
        // s→0 (3), s→1 (2), 0→1 (1), 0→2 (3), 1→3 (2), 2→3 (1)... with node 2 → t (2)
        // and 3 → t (3): max flow 5.
        let mut g = Graph::new(4);
        g.terminal(0, 3.0, 0.0);
        g.terminal(1, 2.0, 0.0);
        g.terminal(2, 0.0, 2.0);
        g.terminal(3, 0.0, 3.0);
        g.edge(0, 1, 1.0, 0.0);
        g.edge(0, 2, 3.0, 0.0);
        g.edge(1, 3, 2.0, 0.0);
        g.edge(2, 3, 1.0, 0.0);
        let (side, f) = max_flow(g);
        assert!((f - 5.0).abs() < 1e-12, "{f}");
        assert!(side.len() == 4);
    }

    #[test]
    fn max_flow_matches_a_brute_force_min_cut() {
        let mut seed = 12345u64;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f64 / (1u64 << 24) as f64
        };
        for _ in 0..30 {
            let n = 7;
            let mut tr = vec![(0.0, 0.0); n];
            let mut edges = Vec::new();
            let mut g = Graph::new(n);
            for (p, t) in tr.iter_mut().enumerate() {
                *t = (
                    rnd() * 3.0 * f64::from(rnd() > 0.5),
                    rnd() * 3.0 * f64::from(rnd() > 0.5),
                );
                g.terminal(p, t.0, t.1);
            }
            for p in 0..n {
                for q in p + 1..n {
                    if rnd() > 0.5 {
                        let (a, b) = (rnd() * 2.0, rnd() * 2.0);
                        edges.push((p, q, a, b));
                        g.edge(p, q, a, b);
                    }
                }
            }
            let (side, f) = max_flow(g);
            let cut = |s: &dyn Fn(usize) -> bool| {
                let mut c = 0.0;
                for (p, t) in tr.iter().enumerate() {
                    c += if s(p) { t.1 } else { t.0 };
                }
                for &(p, q, a, b) in &edges {
                    if s(p) && !s(q) {
                        c += a;
                    }
                    if s(q) && !s(p) {
                        c += b;
                    }
                }
                c
            };
            let best = (0..1u32 << n)
                .map(|m| cut(&|p| m >> p & 1 == 1))
                .fold(f64::INFINITY, f64::min);
            assert!((f - best).abs() < 1e-9, "flow {f} vs cut {best}");
            assert!((cut(&|p| side[p]) - best).abs() < 1e-9);
        }
    }

    #[test]
    fn seam_goes_around_a_moving_object() {
        let (w, h) = (120usize, 60usize);
        let scene = |x: usize, y: usize| ((x as f32 * 0.21).sin() * (y as f32 * 0.17).cos()) * 0.3;
        // Frame a covers x < 80, b covers x ≥ 40; b has a person at x 52..64, y 20..40.
        let a = Layer {
            v: (0..w * h)
                .map(|i| {
                    if i % w < 80 {
                        scene(i % w, i / w)
                    } else {
                        f32::NAN
                    }
                })
                .collect(),
            sigma: vec![0.01; w * h],
        };
        let b = Layer {
            v: (0..w * h)
                .map(|i| {
                    let (x, y) = (i % w, i / w);
                    if x < 40 {
                        f32::NAN
                    } else if (52..64).contains(&x) && (20..40).contains(&y) {
                        scene(x, y) + 2.0
                    } else {
                        scene(x, y)
                    }
                })
                .collect(),
            sigma: vec![0.01; w * h],
        };
        let v = voronoi(&[a.clone(), b.clone()], w, h);
        // The plain Voronoi split runs through the person (x = 60).
        assert!((20..40).any(|y| v[y * w + 54] == 0 && v[y * w + 62] == 1));
        let l = seams(&[a, b], w, h);
        // No seam cuts the person: one frame shows all of it.
        let first = l[20 * w + 52];
        for y in 20..40 {
            for x in 52..64 {
                assert_eq!(
                    l[y * w + x],
                    first,
                    "the seam cuts the person at ({x}, {y})"
                );
            }
        }
        assert!(l.iter().all(|&k| k != NONE));
        assert_eq!(l[30 * w + 100], 1);
        assert_eq!(l[30 * w + 10], 0);
    }
}
