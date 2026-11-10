//! Shapes from IFC's geometric items: swept solids (extruded, tapered, revolved, swept disks,
//! fixed-reference sweeps), faceted B-reps and surface models, triangulated and polygonal
//! face sets, and the CSG primitives.

use std::collections::HashMap;
use std::f64::consts::TAU;

use super::Ctx;
use super::geom::{
    Mat, P2, P3, Shape, add, cross, dot, len, newell, plane_basis, ring_area, scale, sub, unit,
};
use super::profile::Region;
use super::step::V;
use super::tri2d::triangulate;

/// Appends a plane region at height `z` (in its plane's frame `m`) facing +z, or −z when
/// `down`.
fn cap(s: &mut Shape, r: &Region, m: &Mat, z: P3, down: bool) {
    let tris = triangulate(&r.outer, &r.holes);
    let base = s.pos.len() as u32;
    for p in r.outer.iter().chain(r.holes.iter().flatten()) {
        s.pos.push(m.point(add([p[0], p[1], 0.0], z)));
    }
    for t in tris {
        let t = [t[0] + base, t[1] + base, t[2] + base];
        s.tri.push(if down { [t[0], t[2], t[1]] } else { t });
    }
}

/// The rings of a region turned: outer counter-clockwise, holes clockwise.
fn oriented(r: &Region) -> Vec<Vec<P2>> {
    let mut out = Vec::with_capacity(1 + r.holes.len());
    let mut o = r.outer.clone();
    if ring_area(&o) < 0.0 {
        o.reverse();
    }
    out.push(o);
    for h in &r.holes {
        let mut h = h.clone();
        if ring_area(&h) > 0.0 {
            h.reverse();
        }
        out.push(h);
    }
    out
}

/// A quad `a b c d` (counter-clockwise seen from outside).
fn quad(s: &mut Shape, a: P3, b: P3, c: P3, d: P3) {
    let i = s.pos.len() as u32;
    s.pos.extend([a, b, c, d]);
    s.tri.push([i, i + 1, i + 2]);
    s.tri.push([i, i + 2, i + 3]);
}

/// Regions extruded from z = 0 along `v`, in the frame `m`.
pub(crate) fn extrude(regions: &[Region], m: &Mat, v: P3) -> Shape {
    let mut s = Shape {
        closed: true,
        ..Shape::default()
    };
    for r in regions {
        let r = Region {
            outer: oriented(r).swap_remove(0),
            holes: oriented(r).into_iter().skip(1).collect(),
        };
        cap(&mut s, &r, m, [0.0; 3], true);
        cap(&mut s, &r, m, v, false);
        for ring in std::iter::once(&r.outer).chain(&r.holes) {
            let n = ring.len();
            for k in 0..n {
                let (p, q) = (ring[k], ring[(k + 1) % n]);
                let (a, b) = ([p[0], p[1], 0.0], [q[0], q[1], 0.0]);
                quad(
                    &mut s,
                    m.point(a),
                    m.point(b),
                    m.point(add(b, v)),
                    m.point(add(a, v)),
                );
            }
        }
    }
    s.orient_outward();
    s
}

/// A loft between two regions of matching ring sizes (tapered extrusions); `None` when they
/// do not match.
pub(crate) fn loft(a: &[Region], b: &[Region], m: &Mat, v: P3) -> Option<Shape> {
    if a.len() != b.len() {
        return None;
    }
    let mut s = Shape {
        closed: true,
        ..Shape::default()
    };
    for (ra, rb) in a.iter().zip(b) {
        let (oa, ob) = (oriented(ra), oriented(rb));
        if oa.len() != ob.len() || oa.iter().zip(&ob).any(|(x, y)| x.len() != y.len()) {
            return None;
        }
        let ra = Region {
            outer: oa[0].clone(),
            holes: oa[1..].to_vec(),
        };
        let rb = Region {
            outer: ob[0].clone(),
            holes: ob[1..].to_vec(),
        };
        cap(&mut s, &ra, m, [0.0; 3], true);
        cap(&mut s, &rb, m, v, false);
        for (x, y) in oa.iter().zip(&ob) {
            let n = x.len();
            for k in 0..n {
                let (p, q) = (x[k], x[(k + 1) % n]);
                let (p2, q2) = (y[k], y[(k + 1) % n]);
                quad(
                    &mut s,
                    m.point([p[0], p[1], 0.0]),
                    m.point([q[0], q[1], 0.0]),
                    m.point(add([q2[0], q2[1], 0.0], v)),
                    m.point(add([p2[0], p2[1], 0.0], v)),
                );
            }
        }
    }
    s.orient_outward();
    Some(s)
}

/// Rotation of `p` about the line through `o` along unit `axis` by `t` radians.
fn rotate_about(p: P3, o: P3, axis: P3, t: f64) -> P3 {
    let v = sub(p, o);
    let (c, s) = (t.cos(), t.sin());
    let k = axis;
    let r = add(
        add(scale(v, c), scale(cross(k, v), s)),
        scale(k, dot(k, v) * (1.0 - c)),
    );
    add(o, r)
}

/// Regions turned `angle` radians about the axis through `o` along `axis` (both in the
/// regions' plane frame), placed by `m`.
pub(crate) fn revolve(
    regions: &[Region],
    m: &Mat,
    o: P3,
    axis: P3,
    angle: f64,
    steps: usize,
) -> Shape {
    let mut s = Shape {
        closed: true,
        ..Shape::default()
    };
    let Some(axis) = unit(axis) else {
        return s;
    };
    let angle = angle.clamp(-TAU, TAU);
    let full = (angle.abs() - TAU).abs() < 1e-6;
    let steps = steps.max(3);
    for r in regions {
        let rings = oriented(r);
        if !full {
            let rr = Region {
                outer: rings[0].clone(),
                holes: rings[1..].to_vec(),
            };
            // Start cap, and the end cap rotated.
            cap(&mut s, &rr, m, [0.0; 3], true);
            let base = s.pos.len();
            cap(&mut s, &rr, &Mat::IDENTITY, [0.0; 3], false);
            for p in &mut s.pos[base..] {
                *p = m.point(rotate_about(*p, o, axis, angle));
            }
        }
        for ring in &rings {
            let n = ring.len();
            for k in 0..n {
                let (p, q) = (ring[k], ring[(k + 1) % n]);
                let (p, q) = ([p[0], p[1], 0.0], [q[0], q[1], 0.0]);
                for j in 0..steps {
                    let t0 = angle * j as f64 / steps as f64;
                    let t1 = angle * (j + 1) as f64 / steps as f64;
                    quad(
                        &mut s,
                        m.point(rotate_about(p, o, axis, t0)),
                        m.point(rotate_about(q, o, axis, t0)),
                        m.point(rotate_about(q, o, axis, t1)),
                        m.point(rotate_about(p, o, axis, t1)),
                    );
                }
            }
        }
    }
    s.orient_outward();
    s
}

/// Frames along a path by parallel transport (or kept towards `up`): point, tangent (the
/// mean of the segments' at a corner), normal, and the mitre factor that keeps a section's
/// width across a corner. A `closed` path's ends join.
fn frames(path: &[P3], up: Option<P3>, closed: bool) -> Vec<(P3, P3, P3, f64, P3)> {
    let n = path.len();
    let seg_dir = |k: usize| unit(sub(path[(k + 1) % n], path[k % n]));
    let mut out: Vec<(P3, P3, P3, f64, P3)> = Vec::with_capacity(n);
    let mut normal: Option<P3> = None;
    for (k, &here) in path.iter().enumerate() {
        let a = if k > 0 {
            seg_dir(k - 1)
        } else if closed {
            seg_dir(n - 1)
        } else {
            None
        };
        let b = if k + 1 < n || closed {
            seg_dir(k)
        } else {
            None
        };
        let (t, mitre, bend) = match (a, b) {
            (Some(a), Some(b)) => {
                let t = unit(add(a, b)).unwrap_or(b);
                let bend = unit(sub(b, a)).unwrap_or([0.0; 3]);
                (t, 1.0 / dot(t, b).max(0.25), bend)
            }
            (Some(a), None) => (a, 1.0, [0.0; 3]),
            (None, Some(b)) => (b, 1.0, [0.0; 3]),
            (None, None) => ([0.0, 0.0, 1.0], 1.0, [0.0; 3]),
        };
        let nn = match (up, normal) {
            (Some(u), _) => unit(sub(u, scale(t, dot(u, t)))),
            (None, Some(prev)) => unit(sub(prev, scale(t, dot(prev, t)))),
            (None, None) => None,
        }
        .unwrap_or_else(|| plane_basis(t).0);
        normal = Some(nn);
        out.push((here, t, nn, mitre, bend));
    }
    out
}

/// The path without repeated points; whether it closes on itself (its last point dropped).
fn clean_path(path: &[P3]) -> (Vec<P3>, bool) {
    let mut p = path.to_vec();
    p.dedup_by(|a, b| len(sub(*a, *b)) < 1e-12);
    let size = p.iter().fold(0.0f64, |m, q| m.max(len(sub(*q, p[0]))));
    let closed = p.len() > 3 && len(sub(p[0], p[p.len() - 1])) <= size * 1e-9;
    if closed {
        p.pop();
    }
    (p, closed)
}

/// A tube of `radius` (hollow inside `inner`) along `path`.
pub(crate) fn swept_disk(path: &[P3], radius: f64, inner: f64, seg: usize) -> Shape {
    let mut s = Shape {
        closed: true,
        ..Shape::default()
    };
    let (path, closed) = clean_path(path);
    if path.len() < 2 || radius.is_nan() || radius <= 0.0 {
        return s;
    }
    let f = frames(&path, None, closed);
    let ring = |k: usize, r: f64| -> Vec<P3> {
        let (c, t, n, mitre, bend) = f[k];
        let b = cross(t, n);
        (0..seg)
            .map(|j| {
                let a = TAU * j as f64 / seg as f64;
                let v = add(scale(n, r * a.cos()), scale(b, r * a.sin()));
                // Widened along the bend at a corner (a mitre joint).
                add(c, add(v, scale(bend, (mitre - 1.0) * dot(v, bend))))
            })
            .collect()
    };
    let hollow = inner > 0.0 && inner < radius;
    let segs = if closed { path.len() } else { path.len() - 1 };
    for k in 0..segs {
        let k1 = (k + 1) % path.len();
        let (r0, r1) = (ring(k, radius), ring(k1, radius));
        for j in 0..seg {
            let jn = (j + 1) % seg;
            quad(&mut s, r0[j], r0[jn], r1[jn], r1[j]);
        }
        if hollow {
            let (i0, i1) = (ring(k, inner), ring(k1, inner));
            for j in 0..seg {
                let jn = (j + 1) % seg;
                quad(&mut s, i0[jn], i0[j], i1[j], i1[jn]);
            }
        }
    }
    // End caps: disks or rings.
    let ends: &[usize] = if closed { &[] } else { &[0, path.len() - 1] };
    for &k in ends {
        let o = ring(k, radius);
        if hollow {
            let i = ring(k, inner);
            for j in 0..seg {
                let jn = (j + 1) % seg;
                quad(&mut s, o[j], o[jn], i[jn], i[j]);
            }
        } else {
            let c = s.vertex(f[k].0);
            let base = s.pos.len() as u32;
            s.pos.extend_from_slice(&o);
            for j in 0..seg as u32 {
                s.tri.push([c, base + j, base + (j + 1) % seg as u32]);
            }
        }
    }
    // Caps were added facing whichever way; each end must face out along the path. The
    // whole shell is made consistent by orienting the caps against the tube.
    fix_caps(&mut s);
    s.orient_outward();
    s
}

/// Makes every triangle agree in orientation with its neighbours across shared edges
/// (positions compared exactly), starting from the first; used where caps are added
/// without regard to winding.
pub(crate) fn fix_caps(s: &mut Shape) {
    let key = |p: P3| p.map(f64::to_bits);
    // Weld positions so neighbouring triangles share indices.
    let mut ids: HashMap<[u64; 3], u32> = HashMap::new();
    let tri: Vec<[u32; 3]> = s
        .tri
        .iter()
        .map(|t| {
            t.map(|i| {
                let n = ids.len() as u32;
                *ids.entry(key(s.pos[i as usize])).or_insert(n)
            })
        })
        .collect();
    let mut edges: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for (k, t) in tri.iter().enumerate() {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            edges
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push(k as u32);
        }
    }
    let mut flip = vec![false; tri.len()];
    let mut seen = vec![false; tri.len()];
    for start in 0..tri.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut stack = vec![start];
        while let Some(k) = stack.pop() {
            let t = tri[k];
            let t = if flip[k] { [t[0], t[2], t[1]] } else { t };
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                let Some(list) = edges.get(&(a.min(b), a.max(b))) else {
                    continue;
                };
                if list.len() != 2 {
                    continue;
                }
                for &o in list {
                    let o = o as usize;
                    if o == k || seen[o] {
                        continue;
                    }
                    // A consistent neighbour runs the edge the other way (b → a).
                    let u = tri[o];
                    let same_dir = (0..3).any(|f| u[f] == a && u[(f + 1) % 3] == b);
                    flip[o] = same_dir;
                    seen[o] = true;
                    stack.push(o);
                }
            }
        }
    }
    for (t, f) in s.tri.iter_mut().zip(flip) {
        if f {
            t.swap(1, 2);
        }
    }
}

/// A profile swept along a path keeping its y axis towards `up` (fixed-reference and
/// surface-curve sweeps): the profile's x across the path, y towards `up`.
pub(crate) fn sweep_profile(regions: &[Region], path: &[P3], up: Option<P3>) -> Shape {
    let mut s = Shape {
        closed: true,
        ..Shape::default()
    };
    let (path, closed) = clean_path(path);
    if path.len() < 2 {
        return s;
    }
    let f = frames(&path, up, closed);
    let place = |k: usize, p: P2| -> P3 {
        let (c, t, n, mitre, _) = f[k];
        // Profile x across (widened at corners), y along the normal.
        let x = cross(n, t);
        add(c, add(scale(x, p[0] * mitre), scale(n, p[1])))
    };
    let segs = if closed { path.len() } else { path.len() - 1 };
    for r in regions {
        let rings = oriented(r);
        for ring in &rings {
            let n = ring.len();
            for k in 0..segs {
                let k1 = (k + 1) % path.len();
                for j in 0..n {
                    let (p, q) = (ring[j], ring[(j + 1) % n]);
                    quad(&mut s, place(k, p), place(k, q), place(k1, q), place(k1, p));
                }
            }
        }
        let rr = Region {
            outer: rings[0].clone(),
            holes: rings[1..].to_vec(),
        };
        let ends: &[(usize, bool)] = if closed {
            &[]
        } else {
            &[(0, true), (path.len() - 1, false)]
        };
        for &(k, down) in ends {
            let tris = triangulate(&rr.outer, &rr.holes);
            let base = s.pos.len() as u32;
            for p in rr.outer.iter().chain(rr.holes.iter().flatten()) {
                s.pos.push(place(k, *p));
            }
            for t in tris {
                let t = [t[0] + base, t[1] + base, t[2] + base];
                s.tri.push(if down { [t[0], t[2], t[1]] } else { t });
            }
        }
    }
    fix_caps(&mut s);
    s.orient_outward();
    s
}

/// Triangles of a plane polygon with holes given by 3D rings (faces of B-reps); appended to
/// `s` in their orientation (the outer ring's turning).
pub(crate) fn face(s: &mut Shape, outer: &[P3], holes: &[Vec<P3>]) {
    if outer.len() < 3 {
        return;
    }
    if outer.len() == 3 && holes.is_empty() {
        let i = s.pos.len() as u32;
        s.pos.extend_from_slice(outer);
        s.tri.push([i, i + 1, i + 2]);
        return;
    }
    let n = newell(outer);
    if len(n) < 1e-300 {
        return;
    }
    let (u, v) = plane_basis(n);
    let o = outer[0];
    let flat = |p: &P3| -> P2 {
        let d = sub(*p, o);
        [dot(d, u), dot(d, v)]
    };
    let o2: Vec<P2> = outer.iter().map(flat).collect();
    let h2: Vec<Vec<P2>> = holes.iter().map(|h| h.iter().map(flat).collect()).collect();
    let tris = triangulate(&o2, &h2);
    let base = s.pos.len() as u32;
    s.pos.extend_from_slice(outer);
    for h in holes {
        s.pos.extend_from_slice(h);
    }
    // The triangulation is counter-clockwise in (u, v), whose normal is the outer ring's.
    s.tri
        .extend(tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
}

impl Ctx<'_> {
    /// The faces of an `IfcConnectedFaceSet` (closed or open shell).
    pub fn shell(&self, id: u32, s: &mut Shape) {
        let Some(a) = self.s.args(id) else { return };
        for f in a.first().map(V::refs).unwrap_or_default() {
            self.shell_face(f, s);
        }
    }

    pub(crate) fn shell_face(&self, f: u32, s: &mut Shape) {
        let Some(fa) = self.s.args(f) else { return };
        let mut outer: Option<Vec<P3>> = None;
        let mut rings: Vec<Vec<P3>> = Vec::new();
        for b in fa.first().map(V::refs).unwrap_or_default() {
            let Some(ba) = self.s.args(b) else { continue };
            let is_outer = self.s.ty(b) == Some("IFCFACEOUTERBOUND");
            let orient = ba.get(1).and_then(V::as_bool).unwrap_or(true);
            let Some(lp) = ba.first().and_then(V::as_ref) else {
                continue;
            };
            let mut pts: Vec<P3> = match self.s.ty(lp) {
                Some("IFCPOLYLOOP") => self
                    .s
                    .args(lp)
                    .and_then(|l| l.first().map(V::refs))
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|p| self.point(p))
                    .collect(),
                _ => continue,
            };
            pts.dedup_by(|a, b| len(sub(*a, *b)) <= 1e-12);
            while pts.len() > 1 && len(sub(pts[0], pts[pts.len() - 1])) <= 1e-12 {
                pts.pop();
            }
            if pts.len() < 3 {
                continue;
            }
            if !orient {
                pts.reverse();
            }
            if is_outer && outer.is_none() {
                outer = Some(pts);
            } else {
                rings.push(pts);
            }
        }
        let outer = match outer {
            Some(o) => o,
            None => {
                if rings.is_empty() {
                    return;
                }
                // No outer bound marked: the largest ring.
                let k = (0..rings.len())
                    .max_by(|&a, &b| len(newell(&rings[a])).total_cmp(&len(newell(&rings[b]))))
                    .unwrap_or(0);
                rings.swap_remove(k)
            }
        };
        face(s, &outer, &rings);
    }

    /// `IfcTriangulatedFaceSet` and `IfcPolygonalFaceSet`, with per-face colours from an
    /// `IfcIndexedColourMap` when one maps to it.
    pub fn face_set(&self, id: u32, ty: &str) -> Option<Shape> {
        let a = self.s.args(id)?;
        let coords = self.point_list(a.first()?.as_ref()?);
        if coords.is_empty() {
            return None;
        }
        let triangulated =
            ty == "IFCTRIANGULATEDFACESET" || ty == "IFCTRIANGULATEDIRREGULARNETWORK";
        let closed_attr = if triangulated { 2 } else { 1 };
        let closed = a.get(closed_attr).and_then(V::as_bool);
        // PnIndex: the position of each index in the coordinate list, when given.
        let pn_attr = if triangulated { 4 } else { 3 };
        let pn: Vec<i64> = a
            .get(pn_attr)
            .and_then(V::as_list)
            .filter(|l| l.iter().all(|v| matches!(v, V::Int(_))))
            .map(|l| l.iter().filter_map(V::as_i64).collect())
            .unwrap_or_default();
        let at = |i: i64| -> Option<u32> {
            let i = if pn.is_empty() {
                i
            } else {
                *pn.get((i - 1).max(0) as usize)?
            };
            let k = (i - 1) as usize;
            (i >= 1 && k < coords.len()).then_some(k as u32)
        };
        let mut s = Shape {
            pos: coords.clone(),
            closed: closed.unwrap_or(false),
            ..Shape::default()
        };
        // Which source face each triangle comes from (for colours).
        let mut face_of: Vec<u32> = Vec::new();
        if triangulated {
            for (fk, t) in a.get(3)?.as_list()?.iter().enumerate() {
                let ix: Vec<i64> = t
                    .as_list()
                    .map(|l| l.iter().filter_map(V::as_i64).collect())
                    .unwrap_or_default();
                if ix.len() != 3 {
                    continue;
                }
                if let (Some(p), Some(q), Some(r)) = (at(ix[0]), at(ix[1]), at(ix[2])) {
                    s.tri.push([p, q, r]);
                    face_of.push(fk as u32);
                }
            }
        } else {
            for (fk, f) in a
                .get(2)
                .map(V::refs)
                .unwrap_or_default()
                .into_iter()
                .enumerate()
            {
                let Some(fa) = self.s.args(f) else { continue };
                let ring = |v: &V| -> Vec<P3> {
                    v.as_list()
                        .map(|l| {
                            l.iter()
                                .filter_map(V::as_i64)
                                .filter_map(at)
                                .map(|k| coords[k as usize])
                                .collect()
                        })
                        .unwrap_or_default()
                };
                let outer = ring(fa.first().unwrap_or(&V::Null));
                let holes: Vec<Vec<P3>> = fa
                    .get(1)
                    .and_then(V::as_list)
                    .map(|l| l.iter().map(ring).filter(|r| r.len() >= 3).collect())
                    .unwrap_or_default();
                let before = s.tri.len();
                let mut t = Shape::default();
                face(&mut t, &outer, &holes);
                let base = s.pos.len() as u32;
                s.pos.extend_from_slice(&t.pos);
                s.tri.extend(
                    t.tri
                        .iter()
                        .map(|x| [x[0] + base, x[1] + base, x[2] + base]),
                );
                face_of.extend(std::iter::repeat_n(fk as u32, s.tri.len() - before));
            }
        }
        if let Some(map) = self.colour_maps.get(&id) {
            s.colors = self.face_colours(*map, &face_of);
        }
        if closed.is_none() {
            s.closed = is_closed(&s);
        }
        Some(s)
    }

    /// Colours per triangle from an `IfcIndexedColourMap`.
    fn face_colours(&self, map: u32, face_of: &[u32]) -> Option<Vec<[u8; 4]>> {
        let a = self.s.args(map)?;
        // (MappedTo, Opacity, Colours, ColourIndex)
        let opacity = a.get(1).and_then(V::as_f64).unwrap_or(1.0).clamp(0.0, 1.0);
        let list = self.s.args(a.get(2)?.as_ref()?)?;
        let colours: Vec<[u8; 4]> = list
            .first()?
            .as_list()?
            .iter()
            .map(|c| {
                let v = c.reals();
                let g = |k: usize| super::style::to_srgb8(v.get(k).copied().unwrap_or(1.0));
                [g(0), g(1), g(2), (opacity * 255.0).round() as u8]
            })
            .collect();
        let index: Vec<i64> = a.get(3)?.as_list()?.iter().filter_map(V::as_i64).collect();
        Some(
            face_of
                .iter()
                .map(|&f| {
                    index
                        .get(f as usize)
                        .and_then(|&i| colours.get((i - 1).max(0) as usize))
                        .copied()
                        .unwrap_or([255; 4])
                })
                .collect(),
        )
    }

    /// The CSG primitives, in their own frame.
    pub fn primitive(&self, id: u32, ty: &str) -> Option<Shape> {
        let a = self.s.args(id)?;
        let m = self.placement(a.first()?);
        let f = |k: usize| a.get(k).and_then(V::as_f64);
        let seg = self.segments;
        let s = match ty {
            "IFCBLOCK" => {
                let (x, y, z) = (f(1)?, f(2)?, f(3)?);
                let r = Region {
                    outer: vec![[0.0, 0.0], [x, 0.0], [x, y], [0.0, y]],
                    holes: Vec::new(),
                };
                extrude(&[r], &m, [0.0, 0.0, z])
            }
            "IFCRIGHTCIRCULARCYLINDER" => {
                let (h, r) = (f(1)?, f(2)?);
                let reg = Region {
                    outer: super::profile::ellipse(r, r, seg),
                    holes: Vec::new(),
                };
                extrude(&[reg], &m, [0.0, 0.0, h])
            }
            "IFCRIGHTCIRCULARCONE" => {
                let (h, r) = (f(1)?, f(2)?);
                let ring = super::profile::ellipse(r, r, seg);
                pyramid(&ring, &m, h)
            }
            "IFCRECTANGULARPYRAMID" => {
                // The base [0, x] × [0, y], the apex above its centre.
                let (x, y, h) = (f(1)?, f(2)?, f(3)?);
                let (hx, hy) = (x / 2.0, y / 2.0);
                let ring = [[-hx, -hy], [hx, -hy], [hx, hy], [-hx, hy]];
                pyramid(&ring, &m.mul(&Mat::translation([hx, hy, 0.0])), h)
            }
            "IFCSPHERE" => {
                let r = f(1)?;
                sphere(&m, r, seg)
            }
            _ => return None,
        };
        Some(s)
    }
}

/// A cone over a ring (apex above its origin at `h`).
fn pyramid(ring: &[P2], m: &Mat, h: f64) -> Shape {
    let mut s = Shape {
        closed: true,
        ..Shape::default()
    };
    let r = Region {
        outer: ring.to_vec(),
        holes: Vec::new(),
    };
    cap(&mut s, &r, m, [0.0; 3], true);
    let apex = m.point([0.0, 0.0, h]);
    let n = ring.len();
    for k in 0..n {
        let (p, q) = (ring[k], ring[(k + 1) % n]);
        let i = s.pos.len() as u32;
        s.pos
            .extend([m.point([p[0], p[1], 0.0]), m.point([q[0], q[1], 0.0]), apex]);
        s.tri.push([i, i + 1, i + 2]);
    }
    s.orient_outward();
    s
}

fn sphere(m: &Mat, r: f64, seg: usize) -> Shape {
    let mut s = Shape {
        closed: true,
        ..Shape::default()
    };
    let rings = (seg / 2).max(4);
    let pt = |i: usize, j: usize| -> P3 {
        let th = std::f64::consts::PI * i as f64 / rings as f64;
        let ph = TAU * j as f64 / seg as f64;
        m.point([
            r * th.sin() * ph.cos(),
            r * th.sin() * ph.sin(),
            r * th.cos(),
        ])
    };
    for i in 0..rings {
        for j in 0..seg {
            quad(
                &mut s,
                pt(i, j),
                pt(i + 1, j),
                pt(i + 1, j + 1),
                pt(i, j + 1),
            );
        }
    }
    s.orient_outward();
    s
}

/// Whether every edge of `s` is shared by exactly two triangles running it in opposite
/// directions (positions compared exactly).
pub(crate) fn is_closed(s: &Shape) -> bool {
    if s.tri.is_empty() {
        return false;
    }
    let key = |i: u32| s.pos[i as usize].map(f64::to_bits);
    let mut edges: HashMap<([u64; 3], [u64; 3]), i32> = HashMap::with_capacity(s.tri.len() * 3);
    for t in &s.tri {
        for e in 0..3 {
            let (a, b) = (key(t[e]), key(t[(e + 1) % 3]));
            if a == b {
                continue;
            }
            if a < b {
                *edges.entry((a, b)).or_default() += 1;
            } else {
                *edges.entry((b, a)).or_default() -= 1;
            }
        }
    }
    edges.values().all(|&c| c == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(w: f64) -> Region {
        Region {
            outer: vec![[0.0, 0.0], [w, 0.0], [w, w], [0.0, w]],
            holes: Vec::new(),
        }
    }

    #[test]
    fn extrusion_volume_and_closure() {
        let s = extrude(&[square(2.0)], &Mat::IDENTITY, [0.0, 0.0, 3.0]);
        assert!((s.volume() - 12.0).abs() < 1e-9);
        assert!(is_closed(&s));
        // Downwards and slanted: still outward.
        let s = extrude(&[square(2.0)], &Mat::IDENTITY, [1.0, 0.0, -3.0]);
        assert!((s.volume() - 12.0).abs() < 1e-9);
        // With a hole, clockwise outer.
        let r = Region {
            outer: vec![[0.0, 0.0], [0.0, 4.0], [4.0, 4.0], [4.0, 0.0]],
            holes: vec![vec![[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0]]],
        };
        let s = extrude(&[r], &Mat::IDENTITY, [0.0, 0.0, 1.0]);
        assert!((s.volume() - 12.0).abs() < 1e-9, "{}", s.volume());
        assert!(is_closed(&s));
    }

    #[test]
    fn revolution_and_tube() {
        // A unit square 1..2 from the axis, turned fully: a ring of volume π (2² − 1²) · 1.
        let r = Region {
            outer: vec![[1.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0]],
            holes: Vec::new(),
        };
        let s = revolve(
            std::slice::from_ref(&r),
            &Mat::IDENTITY,
            [0.0; 3],
            [0.0, 1.0, 0.0],
            TAU,
            256,
        );
        let want = std::f64::consts::PI * 3.0;
        assert!((s.volume() - want).abs() / want < 1e-3, "{}", s.volume());
        let half = revolve(
            &[r],
            &Mat::IDENTITY,
            [0.0; 3],
            [0.0, 1.0, 0.0],
            TAU / 2.0,
            128,
        );
        assert!(
            (half.volume() - want / 2.0).abs() / want < 1e-3,
            "{}",
            half.volume()
        );
        assert!(is_closed(&half));
        let t = swept_disk(&[[0.0; 3], [0.0, 0.0, 10.0]], 1.0, 0.0, 64);
        let want = std::f64::consts::PI * 10.0;
        assert!((t.volume() - want).abs() / want < 0.01, "{}", t.volume());
        let h = swept_disk(
            &[[0.0; 3], [0.0, 0.0, 10.0], [10.0, 0.0, 10.0]],
            1.0,
            0.5,
            64,
        );
        assert!(h.volume() > 0.0);
    }

    #[test]
    fn notched_face_in_3d() {
        let y = 24.99999999995;
        let outer = [
            [24.99998093, 24.99998092649491, 980.0],
            [1775.0, 24.99998092649136, 980.0],
            [1775.0, 24.99998092649136, 3380.0],
            [24.99998093, 24.99998092649491, 3380.0],
            [25.0, y, 3550.0],
            [10109.824, y, 3550.0],
            [10109.824, y, 250.0],
            [25.0, y, 250.0],
        ];
        let holes: Vec<Vec<P3>> = [2243.0, 3743.0, 5243.0, 6743.0, 8243.0]
            .iter()
            .map(|&x| {
                vec![
                    [x, y, 3380.0],
                    [x, y, 980.0],
                    [x + 1032.0, y, 980.0],
                    [x + 1032.0, y, 3380.0],
                ]
            })
            .collect();
        let mut s = Shape::default();
        face(&mut s, &outer, &holes);
        let want = 10084.824 * 3300.0 - 1750.0 * 2400.0 - 5.0 * 1032.0 * 2400.0;
        assert!((s.area() - want).abs() < 10.0, "{} {want}", s.area());
    }

    #[test]
    fn faces_with_holes() {
        let mut s = Shape::default();
        let outer = [
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [4.0, 0.0, 4.0],
            [0.0, 0.0, 4.0],
        ];
        let hole = vec![
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 3.0],
            [3.0, 0.0, 3.0],
            [3.0, 0.0, 1.0],
        ];
        face(&mut s, &outer, &[hole]);
        assert!((s.area() - 12.0).abs() < 1e-9);
        // The face keeps the outer ring's orientation (normal −y here).
        let t = s.tri[0];
        let [a, b, c] = t.map(|i| s.pos[i as usize]);
        assert!(cross(sub(b, a), sub(c, a))[1] < 0.0);
    }
}
