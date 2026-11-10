//! Polygon meshes as the scene formats give them (USD, COLLADA, 3DS, LightWave, FBX): faces of
//! any size over a position array, with normals, texture coordinates and colours per vertex,
//! per face corner, per face or constant, and a material per face. [`emit`] triangulates them
//! (fans for convex quads and triangles, ear clipping in the face's plane for larger faces),
//! transforms them into the model and adds them to a [`Builder`]. Vertices are shared where
//! every attribute is per vertex, else split per corner; computed normals stay smooth across
//! the split (or follow the reader's smoothing keys).

use crate::Result;
use crate::builder::{Affine, Builder, apply3};

/// How an attribute is given.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Attr<'a, T> {
    None,
    /// One per position.
    Vertex(&'a [T]),
    /// One per face corner, in face order.
    Corner(&'a [T]),
    /// One per face.
    Face(&'a [T]),
    Constant(T),
}

impl<T: Copy> Attr<'_, T> {
    fn get(&self, vertex: usize, corner: usize, face: usize) -> Option<T> {
        match self {
            Attr::None => None,
            Attr::Vertex(v) => v.get(vertex).copied(),
            Attr::Corner(v) => v.get(corner).copied(),
            Attr::Face(v) => v.get(face).copied(),
            Attr::Constant(c) => Some(*c),
        }
    }

    fn per_vertex(&self) -> bool {
        matches!(self, Attr::None | Attr::Vertex(_) | Attr::Constant(_))
    }
}

/// One polygon mesh.
pub(crate) struct Poly<'a> {
    pub positions: &'a [[f32; 3]],
    /// Corners of each face.
    pub counts: &'a [u32],
    /// Position index of each corner, face after face.
    pub indices: &'a [u32],
    pub normals: Attr<'a, [f32; 3]>,
    pub uvs: Attr<'a, [f32; 2]>,
    pub colors: Attr<'a, [u8; 4]>,
    /// Material of each face (`None`: the mesh's own).
    pub face_materials: Option<&'a [Option<u32>]>,
    pub material: Option<u32>,
    /// Faces wind clockwise (USD `leftHanded`, LightWave).
    pub clockwise: bool,
    /// Smoothing key of each corner: corners with equal keys share a computed normal (3DS
    /// smoothing groups). `None`: corners of one position share it.
    pub smooth_keys: Option<&'a [u32]>,
}

impl<'a> Poly<'a> {
    pub fn new(positions: &'a [[f32; 3]], counts: &'a [u32], indices: &'a [u32]) -> Poly<'a> {
        Poly {
            positions,
            counts,
            indices,
            normals: Attr::None,
            uvs: Attr::None,
            colors: Attr::None,
            face_materials: None,
            material: None,
            clockwise: false,
            smooth_keys: None,
        }
    }
}

/// Adds `p`, transformed by `xf`, to `b`. Faces with fewer than three corners or a corner
/// out of range are skipped (noted).
pub(crate) fn emit(b: &mut Builder, p: &Poly, xf: &Affine) -> Result<()> {
    let n = p.positions.len();
    let corners: usize = p.counts.iter().map(|&c| c as usize).sum();
    if corners > p.indices.len() {
        b.note_once("a mesh has fewer face indices than its face sizes need");
    }
    let flip = (xf.det() < 0.0) != p.clockwise;
    let nm = xf.normal_matrix();
    let shared = p.normals.per_vertex()
        && p.uvs.per_vertex()
        && p.colors.per_vertex()
        && p.smooth_keys.is_none();
    // Smoothing keys continue after those already in the builder.
    let key_base = b.normal_key.as_ref().map_or(b.vertex_count(), |k| {
        k.iter().copied().max().map_or(0, |m| m + 1)
    });
    if b.normal_key.is_none() {
        b.normal_key = Some((0..b.vertex_count()).collect());
    }
    let mut smooth_slots: std::collections::HashMap<u32, u32> = Default::default();
    let mut next_slot = key_base + n as u32;
    let mut vertex_of = |b: &mut Builder, v: usize, corner: usize, face: usize| -> Result<u32> {
        let pos = xf.point(p.positions[v]);
        let normal = p.normals.get(v, corner, face).map(|q| apply3(&nm, q));
        let uv = p.uvs.get(v, corner, face);
        let colour = p.colors.get(v, corner, face);
        let i = b.vertex(pos, normal, colour, uv)?;
        let key = match p.smooth_keys.and_then(|k| k.get(corner)) {
            Some(&k) => *smooth_slots.entry(k).or_insert_with(|| {
                next_slot += 1;
                next_slot - 1
            }),
            None => key_base + v as u32,
        };
        b.normal_key.as_mut().expect("set above").push(key);
        Ok(i)
    };
    let mut first_shared: Option<u32> = None;
    if shared {
        b.reserve_vertices(n)?;
        let start = b.vertex_count();
        for v in 0..n {
            vertex_of(b, v, 0, 0)?;
        }
        first_shared = Some(start);
    }
    let mut skipped = 0usize;
    let mut at = 0usize;
    let mut tris = Vec::new();
    let mut corner_vertex: Vec<u32> = Vec::new();
    for (face, &count) in p.counts.iter().enumerate() {
        let count = count as usize;
        let Some(ix) = p.indices.get(at..at + count) else {
            break;
        };
        let start = at;
        at += count;
        if count < 3 || ix.iter().any(|&i| i as usize >= n) {
            skipped += 1;
            continue;
        }
        let m = p
            .face_materials
            .and_then(|f| f.get(face).copied().flatten())
            .or(p.material);
        b.material(m);
        corner_vertex.clear();
        for (k, &v) in ix.iter().enumerate() {
            corner_vertex.push(match first_shared {
                Some(s) => s + v,
                None => vertex_of(b, v as usize, start + k, face)?,
            });
        }
        tris.clear();
        triangulate(p.positions, ix, &mut tris);
        for t in &tris {
            let [a, c, d] = t.map(|k| corner_vertex[k]);
            if flip {
                b.triangle(a, d, c)?;
            } else {
                b.triangle(a, c, d)?;
            }
        }
    }
    if skipped > 0 {
        b.note(format!(
            "{skipped} faces with fewer than three corners or bad indices skipped"
        ));
    }
    Ok(())
}

/// Triangles (as corner numbers) of the face whose corners are `ix`: a fan for triangles
/// and convex quads, else ear clipping in the plane of the face's Newell normal (a fan when
/// that fails).
pub(crate) fn triangulate(pos: &[[f32; 3]], ix: &[u32], out: &mut Vec<[usize; 3]>) {
    let n = ix.len();
    let fan = |out: &mut Vec<[usize; 3]>| {
        for k in 1..n - 1 {
            out.push([0, k, k + 1]);
        }
    };
    if n == 3 {
        out.push([0, 1, 2]);
        return;
    }
    let p: Vec<[f64; 3]> = ix.iter().map(|&i| pos[i as usize].map(f64::from)).collect();
    // Newell normal.
    let mut nrm = [0f64; 3];
    for k in 0..n {
        let (a, b) = (p[k], p[(k + 1) % n]);
        nrm[0] += (a[1] - b[1]) * (a[2] + b[2]);
        nrm[1] += (a[2] - b[2]) * (a[0] + b[0]);
        nrm[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let axis = (0..3)
        .max_by(|&a, &b| nrm[a].abs().total_cmp(&nrm[b].abs()))
        .unwrap_or(2);
    if nrm[axis].abs().partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        fan(out);
        return;
    }
    let (u, v) = match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let sign = nrm[axis].signum();
    let q: Vec<[f64; 2]> = p.iter().map(|a| [a[u], a[v] * sign]).collect();
    let cross = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    if n == 4 {
        // Convex quads: split along the shorter diagonal's side that keeps both halves
        // positive; else the other.
        let ok = |a, b, c| cross(q[a], q[b], q[c]) > 0.0;
        if ok(0, 1, 2) && ok(0, 2, 3) {
            out.extend([[0, 1, 2], [0, 2, 3]]);
        } else if ok(1, 2, 3) && ok(1, 3, 0) {
            out.extend([[1, 2, 3], [1, 3, 0]]);
        } else {
            fan(out);
        }
        return;
    }
    let mut left: Vec<usize> = (0..n).collect();
    let before = out.len();
    let mut guard = 0;
    while left.len() > 3 && guard < n * n {
        guard += 1;
        let m = left.len();
        let mut clipped = false;
        for k in 0..m {
            let (a, b, c) = (left[(k + m - 1) % m], left[k], left[(k + 1) % m]);
            if cross(q[a], q[b], q[c]) <= 0.0 {
                continue;
            }
            let inside = left.iter().any(|&o| {
                o != a
                    && o != b
                    && o != c
                    && cross(q[a], q[b], q[o]) >= 0.0
                    && cross(q[b], q[c], q[o]) >= 0.0
                    && cross(q[c], q[a], q[o]) >= 0.0
            });
            if inside {
                continue;
            }
            out.push([a, b, c]);
            left.remove(k);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if left.len() == 3 {
        out.push([left[0], left[1], left[2]]);
    } else {
        out.truncate(before);
        fan(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concave_faces_clip_into_ears() {
        // An L: a fan from corner 0 would cover the notch.
        let pos = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
        ];
        let ix = [0, 1, 2, 3, 4, 5];
        let mut t = Vec::new();
        triangulate(&pos, &ix, &mut t);
        assert_eq!(t.len(), 4);
        let area: f32 = t
            .iter()
            .map(|&[a, b, c]| {
                let (a, b, c) = (pos[a], pos[b], pos[c]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) / 2.0
            })
            .sum();
        assert!((area - 3.0).abs() < 1e-5, "{area}");
    }

    #[test]
    fn shared_and_split_vertices() {
        let pos = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let counts = [4];
        let ix = [0, 1, 2, 3];
        let mut b = Builder::new(&crate::Limits::default());
        emit(&mut b, &Poly::new(&pos, &counts, &ix), &Affine::IDENTITY).unwrap();
        assert_eq!((b.vertex_count(), b.triangle_count()), (4, 2));
        let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let mut p = Poly::new(&pos, &counts, &ix);
        p.uvs = Attr::Corner(&uv);
        emit(&mut b, &p, &Affine::IDENTITY).unwrap();
        assert_eq!((b.vertex_count(), b.triangle_count()), (8, 4));
        let m = b.finish().unwrap();
        // Computed normals face +z (counter-clockwise faces).
        assert!(m.normals.iter().all(|n| n[2] > 0.99));
    }
}
