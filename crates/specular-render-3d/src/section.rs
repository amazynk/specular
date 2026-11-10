//! Section planes and the section box: what they cut away (the shaders discard it) and the
//! caps that close the cut, one hatched fill per material.
//!
//! A cap is found on the CPU: each triangle the plane crosses gives a segment, the segments
//! chain into loops end to end (the cut points are computed from each edge's corners in a
//! fixed order, so two triangles sharing an edge give the same point to the bit), and the
//! loops of one material are filled even-odd (a loop inside another is a hole) by the ear
//! clipper the IFC reader uses. Only the triangles under [`Bvh`] boxes the plane crosses are
//! visited. A chain that does not close (an open surface) gives no fill; its segments are
//! still the cut's outline.

use std::collections::HashMap;

use specular_mesh::{Bounds, Material, Mesh, Part, Unit, Up};

use crate::bvh::{Aabb, NavScene};
use crate::camera::{V3, cross, dot, len, scale, sub, unit};

/// Section planes at most.
pub const MAX_PLANES: usize = 6;

/// A cutting plane: everything on the side `normal` points to (`normal·p > offset`) is cut
/// away.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionPlane {
    pub normal: V3,
    pub offset: f64,
}

impl SectionPlane {
    /// Through `p`, cutting away the side `n` points to.
    pub fn through(p: V3, n: V3) -> SectionPlane {
        let n = unit(n);
        SectionPlane {
            normal: n,
            offset: dot(n, p),
        }
    }

    /// The other side cut away.
    pub fn flipped(self) -> SectionPlane {
        SectionPlane {
            normal: scale(self.normal, -1.0),
            offset: -self.offset,
        }
    }

    /// How far `p` is on the cut-away side (negative: kept).
    pub fn distance(&self, p: V3) -> f64 {
        dot(self.normal, p) - self.offset
    }
}

/// The section planes and box of a view.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sections {
    pub planes: Vec<SectionPlane>,
    /// Only what is inside is kept.
    pub section_box: Option<Bounds>,
}

impl Sections {
    pub fn is_empty(&self) -> bool {
        self.planes.is_empty() && self.section_box.is_none()
    }

    /// Every plane that cuts: the planes, then the box's six faces (outward normals).
    pub fn all_planes(&self) -> Vec<SectionPlane> {
        let mut v = self.planes.clone();
        if let Some(b) = &self.section_box {
            for k in 0..3 {
                let mut n = [0.0; 3];
                n[k] = 1.0;
                v.push(SectionPlane {
                    normal: n,
                    offset: f64::from(b.max[k]),
                });
                n[k] = -1.0;
                v.push(SectionPlane {
                    normal: n,
                    offset: -f64::from(b.min[k]),
                });
            }
        }
        v
    }

    /// `p` is kept (on no plane's cut-away side, within `eps`).
    pub fn keeps(&self, p: V3, eps: f64) -> bool {
        self.all_planes().iter().all(|pl| pl.distance(p) <= eps)
    }

    /// For the shaders: up to [`MAX_PLANES`] + 6 planes as (normal, offset).
    pub fn uniform(&self) -> ([[f32; 4]; 12], u32) {
        let mut out = [[0.0; 4]; 12];
        let all = self.all_planes();
        let n = all.len().min(12);
        for (o, p) in out.iter_mut().zip(&all) {
            *o = [
                p.normal[0] as f32,
                p.normal[1] as f32,
                p.normal[2] as f32,
                p.offset as f32,
            ];
        }
        (out, n as u32)
    }
}

/// One closed outline of a cut, in the plane.
#[derive(Clone, Debug, PartialEq)]
pub struct CapLoop {
    /// Index into the mesh's materials (`None`: the default look).
    pub material: Option<u32>,
    pub points: Vec<V3>,
    pub closed: bool,
}

/// The caps of every plane: their outlines and the fill as a mesh (canonical frame).
#[derive(Clone, Debug, Default)]
pub struct Caps {
    pub loops: Vec<CapLoop>,
    pub mesh: Option<Mesh>,
    /// Filled area per plane (model units²).
    pub area: Vec<f64>,
}

/// The material of each triangle of `mesh`.
fn materials_of(mesh: &Mesh) -> Vec<Option<u32>> {
    let mut m = vec![None; mesh.indices.len() / 3];
    for p in &mesh.parts {
        let end = ((p.first + p.count) as usize).min(m.len());
        for v in m.iter_mut().take(end).skip(p.first as usize) {
            *v = p.material;
        }
    }
    m
}

/// Bits of a point, for joining segment ends exactly.
type Key = [u64; 3];

fn key(p: V3) -> Key {
    // +0.0 and -0.0 are the same point.
    p.map(|v| (v + 0.0).to_bits())
}

/// Where edge `a`–`b` crosses the plane, the same for either order of the corners.
fn cut_point(pl: &SectionPlane, a: V3, b: V3) -> V3 {
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    let (da, db) = (pl.distance(a), pl.distance(b));
    let t = da / (da - db);
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t)
}

/// The segment where triangle `t` (canonical, its winding) crosses the plane, oriented so
/// the kept side's solid is on its left seen from the cut-away side, or `None`.
fn segment(pl: &SectionPlane, t: [V3; 3]) -> Option<(V3, V3)> {
    // A corner on the plane counts as cut away, so every crossing is one of two edges.
    let s = t.map(|p| pl.distance(p) >= 0.0);
    let n_out = s.iter().filter(|v| **v).count();
    if n_out == 0 || n_out == 3 {
        return None;
    }
    // The lone corner: on the cut-away side if one is, else kept.
    let lone_out = n_out == 1;
    let i = (0..3).find(|&i| s[i] == lone_out)?;
    let (a, b, c) = (t[i], t[(i + 1) % 3], t[(i + 2) % 3]);
    let p = cut_point(pl, a, b);
    let q = cut_point(pl, c, a);
    // Walking a → b → c, the boundary goes from the a–b edge to the c–a edge; which way
    // depends on which side a is on.
    Some(if lone_out { (q, p) } else { (p, q) })
}

/// The caps of `planes` through `mesh` (its triangles under `scene`'s hierarchy when given;
/// every triangle otherwise), clipped by `sections` (the other planes and the box).
pub fn caps(mesh: &Mesh, scene: Option<&NavScene>, sections: &Sections) -> Caps {
    let mats = materials_of(mesh);
    let all = sections.all_planes();
    let eps = 1e-6 * f64::from(mesh.bounds.radius()).max(1e-9);
    let mut out = Caps::default();
    let mut tris: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let mut parts: Vec<(Option<u32>, u32, u32)> = Vec::new();
    let tri_of = |i: usize| -> [V3; 3] {
        std::array::from_fn(|k| {
            let v = mesh.indices[3 * i + k] as usize;
            let p = mesh.positions.get(v).copied().unwrap_or_default();
            specular_mesh::preview::canonical(p, mesh.up).map(f64::from)
        })
    };
    for (pi, pl) in all.iter().enumerate() {
        // Triangles the plane crosses.
        let mut cand: Vec<u32> = Vec::new();
        match scene.filter(|s| s.len() == mats.len()) {
            Some(s) => s.bvh.visit(
                |b: &Aabb| b.side(pl.normal, pl.offset) == 0,
                |i| cand.push(i),
            ),
            None => cand.extend(0..mats.len() as u32),
        }
        cand.sort_unstable();
        // Segments by material.
        let mut by_mat: HashMap<Option<u32>, Vec<(V3, V3)>> = HashMap::new();
        for &i in &cand {
            if let Some(sg) = segment(pl, tri_of(i as usize)) {
                by_mat.entry(mats[i as usize]).or_default().push(sg);
            }
        }
        let mut keys: Vec<Option<u32>> = by_mat.keys().copied().collect();
        keys.sort();
        let (u, v) = basis(pl.normal);
        let mut area = 0.0;
        for m in keys {
            let segs = &by_mat[&m];
            let loops = chain(segs);
            let mut rings: Vec<Vec<[f64; 2]>> = Vec::new();
            let mut ring3: Vec<Vec<V3>> = Vec::new();
            for (pts, closed) in loops {
                if closed && pts.len() >= 3 {
                    rings.push(pts.iter().map(|p| [dot(*p, u), dot(*p, v)]).collect());
                    ring3.push(pts.clone());
                }
                out.loops.push(CapLoop {
                    material: m,
                    points: pts,
                    closed,
                });
            }
            // Even-odd: a ring inside an odd number of others is a hole of the innermost.
            let depth: Vec<usize> = (0..rings.len())
                .map(|i| {
                    (0..rings.len())
                        .filter(|&j| j != i && inside(&rings[j], rings[i][0]))
                        .count()
                })
                .collect();
            for i in (0..rings.len()).filter(|&i| depth[i].is_multiple_of(2)) {
                let holes: Vec<usize> = (0..rings.len())
                    .filter(|&j| depth[j] == depth[i] + 1 && inside(&rings[i], rings[j][0]))
                    .collect();
                let hole_rings: Vec<Vec<[f64; 2]>> =
                    holes.iter().map(|&j| rings[j].clone()).collect();
                let tri = specular_mesh::triangulate_polygon(&rings[i], &hole_rings);
                let mut corners: Vec<V3> = ring3[i].clone();
                for &j in &holes {
                    corners.extend(ring3[j].iter().copied());
                }
                let first = (idx.len() / 3) as u32;
                let mut kept = 0u32;
                for t in tri {
                    // The fill clipped exactly by the other planes (and the box).
                    let mut poly: Vec<V3> = t.map(|k| corners[k as usize]).to_vec();
                    for (j, o) in all.iter().enumerate() {
                        if j != pi {
                            poly = clip_polygon(&poly, o);
                        }
                    }
                    for k in 1..poly.len().saturating_sub(1) {
                        let c = [poly[0], poly[k], poly[k + 1]];
                        let e1 = sub(c[1], c[0]);
                        let e2 = sub(c[2], c[0]);
                        let a2 = len(cross(e1, e2));
                        if a2 <= eps * eps {
                            continue;
                        }
                        area += a2 / 2.0;
                        let base = tris.len() as u32;
                        for p in c {
                            tris.push(p.map(|x| x as f32));
                        }
                        idx.extend([base, base + 1, base + 2]);
                        kept += 1;
                    }
                }
                if kept > 0 {
                    parts.push((m, first, kept));
                }
            }
        }
        out.area.push(area);
    }
    if !idx.is_empty() {
        out.mesh = Some(cap_mesh(mesh, tris, idx, parts, &all));
    }
    out
}

/// The fill as a mesh: one hatched material per source material (a lighter tint of it).
fn cap_mesh(
    src: &Mesh,
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
    parts: Vec<(Option<u32>, u32, u32)>,
    planes: &[SectionPlane],
) -> Mesh {
    // Normals: each triangle faces the cut-away side of its plane (towards the viewer
    // looking into the cut).
    let mut normals = vec![[0.0f32; 3]; positions.len()];
    for t in indices.chunks_exact(3) {
        let c: V3 = std::array::from_fn(|k| {
            f64::from(
                positions[t[0] as usize][k]
                    + positions[t[1] as usize][k]
                    + positions[t[2] as usize][k],
            ) / 3.0
        });
        let n = planes
            .iter()
            .min_by(|a, b| a.distance(c).abs().total_cmp(&b.distance(c).abs()))
            .map_or([0.0, 0.0, 1.0], |p| p.normal);
        for &v in t {
            normals[v as usize] = n.map(|x| x as f32);
        }
    }
    let mut materials: Vec<Material> = Vec::new();
    let mut map: HashMap<Option<u32>, u32> = HashMap::new();
    let mut out_parts: Vec<Part> = Vec::new();
    // Parts grouped by material, in order of first appearance.
    type Runs = (Option<u32>, Vec<(u32, u32)>);
    let mut grouped: Vec<Runs> = Vec::new();
    for (m, first, count) in parts {
        match grouped.iter_mut().find(|g| g.0 == m) {
            Some(g) => g.1.push((first, count)),
            None => grouped.push((m, vec![(first, count)])),
        }
    }
    let mut new_idx = Vec::with_capacity(indices.len());
    for (m, runs) in grouped {
        let id = *map.entry(m).or_insert_with(|| {
            let base = m
                .and_then(|i| src.materials.get(i as usize))
                .map_or([0.50, 0.53, 0.58, 1.0], |mm| mm.base_color);
            materials.push(Material {
                name: m
                    .and_then(|i| src.materials.get(i as usize))
                    .map_or_else(String::new, |mm| mm.name.clone()),
                base_color: [base[0], base[1], base[2], 1.0],
                texture: None,
            });
            (materials.len() - 1) as u32
        });
        let first = (new_idx.len() / 3) as u32;
        let mut count = 0;
        for (f, c) in runs {
            new_idx.extend_from_slice(&indices[3 * f as usize..3 * (f + c) as usize]);
            count += c;
        }
        out_parts.push(Part {
            first,
            count,
            material: Some(id),
        });
    }
    let mut bounds = Bounds::EMPTY;
    for p in &positions {
        bounds.add(*p);
    }
    Mesh {
        format: None,
        positions,
        normals,
        colors: None,
        uvs: None,
        indices: new_idx,
        parts: out_parts,
        materials,
        images: Vec::new(),
        unit: Unit::Unknown,
        up: Up::Z,
        bounds,
        flat: false,
        file_normals: true,
        notes: Vec::new(),
        title: None,
        generator: None,
    }
}

/// The part of convex polygon `poly` the plane keeps (Sutherland–Hodgman).
fn clip_polygon(poly: &[V3], pl: &SectionPlane) -> Vec<V3> {
    let n = poly.len();
    let mut out = Vec::with_capacity(n + 2);
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (da, db) = (pl.distance(a), pl.distance(b));
        if da <= 0.0 {
            out.push(a);
        }
        if (da <= 0.0) != (db <= 0.0) {
            let t = da / (da - db);
            out.push(std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t));
        }
    }
    out
}

/// Two unit vectors spanning the plane with normal `n`.
fn basis(n: V3) -> (V3, V3) {
    let a = if n[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let u = unit(cross(n, a));
    let v = cross(n, u);
    (u, v)
}

/// Even-odd point in polygon.
fn inside(ring: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut c = false;
    let n = ring.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            c = !c;
        }
        j = i;
    }
    c
}

/// Segments joined end to start into loops (closed) and chains (open).
pub(crate) fn chain(segs: &[(V3, V3)]) -> Vec<(Vec<V3>, bool)> {
    let mut from: HashMap<Key, Vec<usize>> = HashMap::with_capacity(segs.len());
    for (i, s) in segs.iter().enumerate() {
        if key(s.0) != key(s.1) {
            from.entry(key(s.0)).or_default().push(i);
        }
    }
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    for start in 0..segs.len() {
        if used[start] || key(segs[start].0) == key(segs[start].1) {
            continue;
        }
        used[start] = true;
        let mut pts = vec![segs[start].0, segs[start].1];
        let k0 = key(segs[start].0);
        let mut closed = false;
        loop {
            let end = key(*pts.last().unwrap_or(&segs[start].1));
            if end == k0 {
                pts.pop();
                closed = true;
                break;
            }
            let next = from
                .get(&end)
                .and_then(|v| v.iter().copied().find(|&i| !used[i]));
            match next {
                Some(i) => {
                    used[i] = true;
                    pts.push(segs[i].1);
                }
                None => break,
            }
        }
        out.push((pts, closed));
    }
    out
}

/// The point of the caps' outlines nearest to `p` (for measuring on the cut), within
/// `tolerance`.
pub fn snap_to_cut(caps: &Caps, p: V3, tolerance: f64) -> Option<V3> {
    let mut best: Option<(f64, V3)> = None;
    for l in &caps.loops {
        let n = l.points.len();
        let segs = if l.closed { n } else { n.saturating_sub(1) };
        for i in 0..segs {
            let (a, b) = (l.points[i], l.points[(i + 1) % n]);
            let ab = sub(b, a);
            let ll = dot(ab, ab);
            let t = if ll > 0.0 {
                (dot(sub(p, a), ab) / ll).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let q: V3 = std::array::from_fn(|k| a[k] + ab[k] * t);
            let d = len(sub(q, p));
            if d <= tolerance && best.is_none_or(|b| d < b.0) {
                best = Some((d, q));
            }
        }
    }
    best.map(|b| b.1)
}
