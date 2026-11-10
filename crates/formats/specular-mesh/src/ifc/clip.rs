//! Booleans the cheap way: a closed shape clipped by a plane, the cut capped (half-space
//! clipping), and a convex region subtracted as the union of the pieces outside each of its
//! planes in turn (openings, polygonal bounded half-spaces). Shapes stay closed, so pieces
//! can be cut again.

use std::collections::HashMap;

use super::geom::in_ring;
use super::geom::{P2, P3, Shape, dot, plane_basis, ring_area, scale, sub};
use super::tri2d::triangulate;

/// The half-space `n · x ≤ d` (`n` unit).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Plane {
    pub n: P3,
    pub d: f64,
}

impl Plane {
    pub fn flipped(&self) -> Plane {
        Plane {
            n: scale(self.n, -1.0),
            d: -self.d,
        }
    }
}

/// `s` cut by `pl`: the part with `n · x ≤ d`, the cut closed by a cap. Distances within
/// `tol` of the plane count as on it.
pub(crate) fn clip(s: &Shape, pl: &Plane, tol: f64) -> Shape {
    clip_cap(s, pl, tol).0
}

/// [`clip`], and the index of the first cap triangle (all after it are the cap's).
fn clip_cap(s: &Shape, pl: &Plane, tol: f64) -> (Shape, usize) {
    let dist: Vec<f64> = s
        .pos
        .iter()
        .map(|&p| {
            let v = dot(pl.n, p) - pl.d;
            if v.abs() <= tol { 0.0 } else { v }
        })
        .collect();
    // Only the vertices triangles use count.
    let (mut any_in, mut any_out) = (false, false);
    for t in &s.tri {
        for &i in t {
            let d = dist[i as usize];
            any_in |= d < 0.0;
            any_out |= d > 0.0;
        }
    }
    if !any_out {
        return (s.clone(), s.tri.len());
    }
    if !any_in {
        return (
            Shape {
                closed: s.closed,
                ..Shape::default()
            },
            0,
        );
    }
    let mut out = Shape {
        pos: s.pos.clone(),
        closed: s.closed,
        ..Shape::default()
    };
    let mut colours: Option<Vec<[u8; 4]>> = s.colors.as_ref().map(|_| Vec::new());
    // Points where edges cross, by the edge's end positions (both triangles at an edge
    // compute the same point).
    let key = |p: P3| p.map(f64::to_bits);
    let mut cuts: HashMap<([u64; 3], [u64; 3]), u32> = HashMap::new();
    let mut segs: Vec<(u32, u32)> = Vec::new();
    for (t, tri) in s.tri.iter().enumerate() {
        let d = tri.map(|i| dist[i as usize]);
        let colour = s.colors.as_ref().and_then(|c| c.get(t).copied());
        if d.iter().all(|&v| v == 0.0) {
            // On the plane: a face of the kept part when it faces out of it.
            let [a, b, c] = tri.map(|i| s.pos[i as usize]);
            let n = super::geom::cross(sub(b, a), sub(c, a));
            if dot(n, pl.n) > 0.0 {
                out.tri.push(*tri);
                if let (Some(cs), Some(c)) = (&mut colours, colour) {
                    cs.push(c);
                }
            }
            continue;
        }
        if d.iter().all(|&v| v <= 0.0) {
            out.tri.push(*tri);
            if let (Some(cs), Some(c)) = (&mut colours, colour) {
                cs.push(c);
            }
            // An edge lying on the plane borders the cut.
            for e in 0..3 {
                let (a, b) = (tri[e], tri[(e + 1) % 3]);
                if d[e] == 0.0 && d[(e + 1) % 3] == 0.0 {
                    segs.push((a, b));
                }
            }
            continue;
        }
        if d.iter().all(|&v| v >= 0.0) {
            continue;
        }
        // Sutherland–Hodgman over the triangle; `on` marks corners on the plane.
        let mut poly: Vec<(u32, bool)> = Vec::with_capacity(4);
        for e in 0..3 {
            let (a, b) = (tri[e], tri[(e + 1) % 3]);
            let (da, db) = (d[e], d[(e + 1) % 3]);
            if da <= 0.0 {
                poly.push((a, da == 0.0));
            }
            if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
                let (pa, pb) = (s.pos[a as usize], s.pos[b as usize]);
                // Computed from the end with the smaller key, so neighbours agree.
                let (p, q, dp, dq) = if key(pa) <= key(pb) {
                    (pa, pb, da, db)
                } else {
                    (pb, pa, db, da)
                };
                let k = (key(p), key(q));
                let idx = *cuts.entry(k).or_insert_with(|| {
                    let t = dp / (dp - dq);
                    let x = [0, 1, 2].map(|c| p[c] + (q[c] - p[c]) * t);
                    out.pos.push(x);
                    (out.pos.len() - 1) as u32
                });
                poly.push((idx, true));
            }
        }
        for k in 1..poly.len().saturating_sub(1) {
            out.tri.push([poly[0].0, poly[k].0, poly[k + 1].0]);
            if let (Some(cs), Some(c)) = (&mut colours, colour) {
                cs.push(c);
            }
        }
        let m = poly.len();
        for k in 0..m {
            let (a, b) = (poly[k], poly[(k + 1) % m]);
            if a.1 && b.1 {
                segs.push((a.0, b.0));
            }
        }
    }
    out.colors = colours;
    let cap_start = out.tri.len();
    if s.closed {
        cap(&mut out, &segs, pl, tol.max(1e-12) * 10.0);
    }
    compact(&mut out);
    (out, cap_start)
}

/// The part of a convex polygon with `n · x ≤ d`.
fn clip_poly(poly: &[P3], pl: &Plane, tol: f64) -> Vec<P3> {
    let n = poly.len();
    let d: Vec<f64> = poly
        .iter()
        .map(|&p| {
            let v = dot(pl.n, p) - pl.d;
            if v.abs() <= tol { 0.0 } else { v }
        })
        .collect();
    if d.iter().all(|&v| v <= 0.0) {
        return poly.to_vec();
    }
    if d.iter().all(|&v| v >= 0.0) {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n + 2);
    for k in 0..n {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        let (da, db) = (d[k], d[(k + 1) % n]);
        if da <= 0.0 {
            out.push(a);
        }
        if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
            // From the end with the smaller key, as `clip` does, so cuts of shared edges
            // agree to the bit.
            let key = |p: P3| p.map(f64::to_bits);
            let (p, q, dp, dq) = if key(a) <= key(b) {
                (a, b, da, db)
            } else {
                (b, a, db, da)
            };
            let t = dp / (dp - dq);
            out.push([0, 1, 2].map(|c| p[c] + (q[c] - p[c]) * t));
        }
    }
    out
}

/// Positions within `grid` of each other made one (the cuts of different edges at a
/// corner), and triangles that collapse dropped.
fn weld(s: &mut Shape, grid: f64) {
    let mut seen: HashMap<[i64; 3], P3> = HashMap::new();
    for p in &mut s.pos {
        let k = p.map(|v| (v / grid).round() as i64);
        *p = *seen.entry(k).or_insert(*p);
    }
    let mut colours = s.colors.take();
    let mut keep_c = Vec::new();
    let pos = &s.pos;
    let mut kept = Vec::with_capacity(s.tri.len());
    for (t, tri) in s.tri.iter().enumerate() {
        let [a, b, c] = tri.map(|i| pos[i as usize]);
        if a != b && b != c && a != c {
            kept.push(*tri);
            if let Some(cs) = &colours {
                keep_c.push(cs.get(t).copied().unwrap_or([255; 4]));
            }
        }
    }
    s.tri = kept;
    if colours.is_some() {
        colours = Some(keep_c);
    }
    s.colors = colours;
}

fn push_fan(out: &mut Shape, poly: &[P3], colour: Option<[u8; 4]>) {
    if poly.len() < 3 {
        return;
    }
    let base = out.pos.len() as u32;
    out.pos.extend_from_slice(poly);
    for k in 1..poly.len() as u32 - 1 {
        out.tri.push([base, base + k, base + k + 1]);
        if let (Some(c), Some(col)) = (&mut out.colors, colour) {
            c.push(col);
        }
    }
}

/// Closed shape `a` minus the convex region inside every plane of `planes` (outward
/// normals), as one closed surface: `a`'s faces outside the region, and the region's faces
/// inside `a` (its cross-sections of `a`, cut to each face) facing into it.
pub(crate) fn subtract_surface(a: &Shape, planes: &[Plane], tol: f64) -> Shape {
    // Wholly outside some plane: untouched.
    for pl in planes {
        let lo = a
            .tri
            .iter()
            .flatten()
            .map(|&i| dot(pl.n, a.pos[i as usize]) - pl.d)
            .fold(f64::INFINITY, f64::min);
        if lo >= -tol {
            return a.clone();
        }
    }
    let mut out = Shape {
        closed: a.closed,
        colors: a.colors.as_ref().map(|_| Vec::new()),
        ..Shape::default()
    };
    // a's faces outside the region.
    for (t, tri) in a.tri.iter().enumerate() {
        let colour = a.colors.as_ref().and_then(|c| c.get(t).copied());
        let mut rem: Vec<P3> = tri.iter().map(|&i| a.pos[i as usize]).collect();
        let normal = {
            let [p, q, r] = [rem[0], rem[1], rem[2]];
            super::geom::cross(sub(q, p), sub(r, p))
        };
        for (i, pl) in planes.iter().enumerate() {
            // A face lying in the plane: facing out of the region it is the region's side
            // (cut by the other planes); facing into it, it borders material outside. Where
            // it lies within the region's other planes the region's wall below (the section
            // of that material) already covers it: only the rest is kept, else the surface
            // has that patch twice (a second opening flush with the first one's walls).
            if rem.iter().all(|&p| (dot(pl.n, p) - pl.d).abs() <= tol) {
                if dot(normal, pl.n) > 0.0 {
                    continue;
                }
                for other in &planes[i + 1..] {
                    push_fan(&mut out, &clip_poly(&rem, &other.flipped(), tol), colour);
                    rem = clip_poly(&rem, other, tol);
                    if rem.len() < 3 {
                        break;
                    }
                }
                rem.clear();
                break;
            }
            let outside = clip_poly(&rem, &pl.flipped(), tol);
            if outside.len() == rem.len() && outside == rem {
                // Wholly outside this plane: kept as it is.
                push_fan(&mut out, &rem, colour);
                rem.clear();
                break;
            }
            push_fan(&mut out, &outside, colour);
            rem = clip_poly(&rem, pl, tol);
            if rem.len() < 3 {
                break;
            }
        }
    }
    // The region's walls inside a.
    for (i, pl) in planes.iter().enumerate() {
        let (cut, start) = clip_cap(a, &pl.flipped(), tol);
        let colour = cut.colors.as_ref().and_then(|c| c.last().copied());
        for tri in &cut.tri[start.min(cut.tri.len())..] {
            let mut poly: Vec<P3> = tri.iter().map(|&k| cut.pos[k as usize]).collect();
            for (j, other) in planes.iter().enumerate() {
                if j != i {
                    poly = clip_poly(&poly, other, tol);
                    if poly.len() < 3 {
                        break;
                    }
                }
            }
            push_fan(&mut out, &poly, colour);
        }
    }
    weld(&mut out, tol.max(1e-12) * 10.0);
    out
}

/// Drops the positions no triangle uses.
fn compact(s: &mut Shape) {
    let mut map = vec![u32::MAX; s.pos.len()];
    let mut pos = Vec::with_capacity(s.tri.len());
    for t in &mut s.tri {
        for i in t.iter_mut() {
            let m = &mut map[*i as usize];
            if *m == u32::MAX {
                *m = pos.len() as u32;
                pos.push(s.pos[*i as usize]);
            }
            *i = *m;
        }
    }
    s.pos = pos;
}

/// Closes the cut: the segments (each running along the kept faces) chained into rings by
/// position, triangulated in the plane facing `+n`.
fn cap(out: &mut Shape, segs: &[(u32, u32)], pl: &Plane, grid: f64) {
    // Ends within `grid` of each other join (cuts of a T-junction's two sides).
    let key = |p: P3| p.map(|v| (v / grid).round() as i64);
    // The cap runs each segment backwards; opposite duplicates cancel.
    let mut next: HashMap<[i64; 3], Vec<(u32, u32)>> = HashMap::new();
    let mut count: HashMap<([i64; 3], [i64; 3]), i32> = HashMap::new();
    for &(a, b) in segs {
        let (ka, kb) = (key(out.pos[a as usize]), key(out.pos[b as usize]));
        if ka == kb {
            continue;
        }
        *count.entry((kb, ka)).or_default() += 1;
    }
    let mut cancelled: HashMap<([i64; 3], [i64; 3]), i32> = HashMap::new();
    for (&(x, y), &c) in &count {
        let opp = count.get(&(y, x)).copied().unwrap_or(0);
        let keep = c - opp.min(c);
        if keep > 0 {
            cancelled.insert((x, y), keep);
        }
    }
    for &(a, b) in segs {
        let (ka, kb) = (key(out.pos[a as usize]), key(out.pos[b as usize]));
        if let Some(c) = cancelled.get_mut(&(kb, ka))
            && *c > 0
        {
            *c -= 1;
            next.entry(kb).or_default().push((b, a));
        }
    }
    // Chain.
    let mut rings: Vec<Vec<u32>> = Vec::new();
    let starts: Vec<[i64; 3]> = next.keys().copied().collect();
    for s in starts {
        while let Some(first) = next.get_mut(&s).and_then(Vec::pop) {
            let mut ring = vec![first.0];
            let mut cur = first.1;
            let mut closed = false;
            for _ in 0..1_000_000 {
                let k = key(out.pos[cur as usize]);
                if k == s {
                    closed = true;
                    break;
                }
                ring.push(cur);
                match next.get_mut(&k).and_then(Vec::pop) {
                    Some(e) => cur = e.1,
                    None => break,
                }
            }
            if closed && ring.len() >= 3 {
                rings.push(ring);
            }
        }
    }
    if rings.is_empty() {
        return;
    }
    let (u, v) = plane_basis(pl.n);
    let flat = |p: P3| -> P2 { [dot(p, u), dot(p, v)] };
    let flat_rings: Vec<Vec<P2>> = rings
        .iter()
        .map(|r| r.iter().map(|&i| flat(out.pos[i as usize])).collect())
        .collect();
    // Nesting depth by containment: even rings are outer, odd ones holes of the ring just
    // around them.
    let n = rings.len();
    let areas: Vec<f64> = flat_rings.iter().map(|r| ring_area(r).abs()).collect();
    let inside = |a: usize, b: usize| -> bool {
        // Ring a inside ring b: a's first point in b, and a smaller.
        areas[a] < areas[b] && in_ring(flat_rings[a][0], &flat_rings[b])
    };
    let depth: Vec<usize> = (0..n)
        .map(|a| (0..n).filter(|&b| b != a && inside(a, b)).count())
        .collect();
    for o in (0..n).filter(|&k| depth[k].is_multiple_of(2)) {
        let holes: Vec<usize> = (0..n)
            .filter(|&h| depth[h] == depth[o] + 1 && inside(h, o))
            .collect();
        let hole_rings: Vec<Vec<P2>> = holes.iter().map(|&h| flat_rings[h].clone()).collect();
        let tris = triangulate(&flat_rings[o], &hole_rings);
        let mut idx: Vec<u32> = rings[o].clone();
        for &h in &holes {
            idx.extend_from_slice(&rings[h]);
        }
        for t in tris {
            out.tri
                .push([idx[t[0] as usize], idx[t[1] as usize], idx[t[2] as usize]]);
            if let Some(c) = &mut out.colors {
                c.push(c.last().copied().unwrap_or([255; 4]));
            }
        }
    }
}

/// A convex region to subtract: inside every plane; its bounds when finite.
#[derive(Clone, Debug)]
pub(crate) struct Region {
    pub planes: Vec<Plane>,
    pub bounds: Option<(P3, P3)>,
}

impl Region {
    pub fn new(planes: Vec<Plane>, bounds: Option<(P3, P3)>) -> Region {
        Region { planes, bounds }
    }
}

/// `pieces` minus `r`: closed pieces whose bounds meet it lose it; the rest stay.
pub(crate) fn subtract_region(pieces: Vec<Shape>, r: &Region, tol: f64) -> Vec<Shape> {
    pieces
        .into_iter()
        .map(|p| {
            if !p.closed {
                return p;
            }
            if let (Some((lo, hi)), Some((plo, phi))) = (r.bounds, p.bounds())
                && (0..3).any(|c| phi[c] < lo[c] - tol || plo[c] > hi[c] + tol)
            {
                return p;
            }
            subtract_surface(&p, &r.planes, tol)
        })
        .filter(|p| !p.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::geom::Mat;
    use super::super::profile::Region;
    use super::super::solid::{extrude, is_closed};
    use super::*;

    fn cube(x0: f64, x1: f64) -> Shape {
        let r = Region {
            outer: vec![[x0, x0], [x1, x0], [x1, x1], [x0, x1]],
            holes: Vec::new(),
        };
        extrude(&[r], &Mat::translation([0.0, 0.0, x0]), [0.0, 0.0, x1 - x0])
    }

    #[test]
    fn half_space_clip_caps() {
        let c = cube(0.0, 2.0);
        let pl = Plane {
            n: super::super::geom::unit([1.0, 1.0, 1.0]).unwrap(),
            d: 3f64.sqrt(),
        };
        let k = clip(&c, &pl, 1e-12);
        // The corner tetrahedron x+y+z ≥ 3 has volume (3-0)^3/6 … minus outside parts; the
        // plane through (1,1,1) splits the cube in half by symmetry.
        assert!((k.volume() - 4.0).abs() < 1e-9, "{}", k.volume());
        assert!(is_closed(&k));
        // Flush planes keep or drop the whole.
        let all = clip(
            &c,
            &Plane {
                n: [0.0, 0.0, 1.0],
                d: 2.0,
            },
            1e-9,
        );
        assert!((all.volume() - 8.0).abs() < 1e-9);
        let none = clip(
            &c,
            &Plane {
                n: [0.0, 0.0, 1.0],
                d: 0.0,
            },
            1e-9,
        );
        assert!(none.is_empty());
    }

    #[test]
    fn convex_hole_through_a_slab() {
        // A 4×4×1 slab with a 2×2 hole through it (the hole taller than the slab).
        let r = Region {
            outer: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
            holes: Vec::new(),
        };
        let slab = extrude(&[r], &Mat::IDENTITY, [0.0, 0.0, 1.0]);
        let planes = [
            Plane {
                n: [1.0, 0.0, 0.0],
                d: 3.0,
            },
            Plane {
                n: [-1.0, 0.0, 0.0],
                d: -1.0,
            },
            Plane {
                n: [0.0, 1.0, 0.0],
                d: 3.0,
            },
            Plane {
                n: [0.0, -1.0, 0.0],
                d: -1.0,
            },
            Plane {
                n: [0.0, 0.0, 1.0],
                d: 2.0,
            },
            Plane {
                n: [0.0, 0.0, -1.0],
                d: 1.0,
            },
        ];
        let cut = subtract_surface(&slab, &planes, 1e-9);
        assert!((cut.volume() - 12.0).abs() < 1e-9, "{}", cut.volume());
        // Outer faces 2 × 12 + 4 × 4, the hole's walls 4 × 2 × 1.
        assert!((cut.area() - 48.0).abs() < 1e-9, "{}", cut.area());
        // A region away from the slab leaves it as it was.
        let mut far = planes;
        far[0].d = 13.0;
        far[1].d = -11.0;
        let same = subtract_surface(&slab, &far, 1e-9);
        assert_eq!(same.tri.len(), slab.tri.len());
        // A second hole cut from the first result, through its T-junctions.
        let mut second = planes;
        second[0].d = 3.6;
        second[1].d = -3.2;
        second[2].d = 3.9;
        second[3].d = -0.1;
        let two = subtract_surface(&cut, &second, 1e-9);
        let want = 12.0 - 0.4 * 3.8;
        assert!((two.volume() - want).abs() < 1e-9, "{}", two.volume());
        // The same hole again (a second opening flush with the first one's walls) takes
        // nothing more and leaves one wall where there was one.
        let again = subtract_surface(&cut, &planes, 1e-9);
        assert!((again.volume() - 12.0).abs() < 1e-9, "{}", again.volume());
        assert!((again.area() - 48.0).abs() < 1e-9, "{}", again.area());
    }
}
