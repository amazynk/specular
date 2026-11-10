//! Many meshes (or point sets) into one, for drawing a selection as a single model.

use specular_mesh::{Bounds, Mesh, Part, Up};

use crate::content::Points;
use crate::math::{self, M4};

/// All of `parts` as one mesh: vertices appended, indices, materials and images renumbered;
/// vertex colours, texture coordinates filled in (white, 0) where a part has none.
pub fn meshes(parts: Vec<Mesh>) -> Mesh {
    let mut out = Mesh {
        up: Up::Z,
        unit: specular_mesh::Unit::Metre,
        bounds: Bounds::EMPTY,
        flat: !parts.is_empty() && parts.iter().all(|m| m.flat),
        file_normals: !parts.is_empty() && parts.iter().all(|m| m.file_normals),
        ..Default::default()
    };
    let any_colors = parts.iter().any(|m| m.colors.is_some());
    let any_uvs = parts.iter().any(|m| m.uvs.is_some());
    let nv: usize = parts.iter().map(|m| m.positions.len()).sum();
    out.positions.reserve(nv);
    out.normals.reserve(nv);
    out.indices
        .reserve(parts.iter().map(|m| m.indices.len()).sum());
    if any_colors {
        out.colors = Some(Vec::with_capacity(nv));
    }
    if any_uvs {
        out.uvs = Some(Vec::with_capacity(nv));
    }
    for m in parts {
        let v0 = out.positions.len() as u32;
        let t0 = (out.indices.len() / 3) as u32;
        let m0 = out.materials.len() as u32;
        let i0 = out.images.len() as u32;
        let n = m.positions.len();
        out.positions.extend_from_slice(&m.positions);
        if m.normals.len() == n {
            out.normals.extend_from_slice(&m.normals);
        } else {
            out.normals.extend(std::iter::repeat_n([0.0, 0.0, 1.0], n));
        }
        if let Some(c) = &mut out.colors {
            match &m.colors {
                Some(mc) if mc.len() == n => c.extend_from_slice(mc),
                _ => c.extend(std::iter::repeat_n([255; 4], n)),
            }
        }
        if let Some(u) = &mut out.uvs {
            match &m.uvs {
                Some(mu) if mu.len() == n => u.extend_from_slice(mu),
                _ => u.extend(std::iter::repeat_n([0.0; 2], n)),
            }
        }
        out.indices.extend(m.indices.iter().map(|i| i + v0));
        out.parts.extend(m.parts.iter().map(|p| Part {
            first: p.first + t0,
            count: p.count,
            material: p.material.map(|x| x + m0),
        }));
        out.materials.extend(m.materials.into_iter().map(|mut mat| {
            mat.texture = mat.texture.map(|t| t + i0);
            mat
        }));
        out.images.extend(m.images);
        if !m.bounds.is_empty() {
            out.bounds = out.bounds.union(&m.bounds);
        }
        for note in m.notes {
            if !out.notes.contains(&note) {
                out.notes.push(note);
            }
        }
    }
    out
}

/// Point sets as one.
pub fn points(parts: Vec<Points>) -> Points {
    let any_colors = parts.iter().any(|p| p.colors.is_some());
    let mut out = Points::default();
    if any_colors {
        out.colors = Some(Vec::new());
    }
    for p in parts {
        let n = p.positions.len();
        out.positions.extend_from_slice(&p.positions);
        if let Some(c) = &mut out.colors {
            match p.colors {
                Some(pc) if pc.len() == n => c.extend(pc),
                _ => c.extend(std::iter::repeat_n([255; 4], n)),
            }
        }
    }
    out
}

/// `model` once per transform, in double precision.
pub fn instances(model: &Mesh, mats: &[M4]) -> Mesh {
    let n = model.positions.len();
    let mut copies = Vec::with_capacity(mats.len());
    for m in mats {
        let mut c = Mesh {
            positions: Vec::with_capacity(n),
            normals: Vec::with_capacity(n),
            colors: model.colors.clone(),
            uvs: model.uvs.clone(),
            indices: model.indices.clone(),
            parts: model.parts.clone(),
            materials: model.materials.clone(),
            images: Vec::new(),
            up: Up::Z,
            bounds: Bounds::EMPTY,
            flat: model.flat,
            file_normals: model.file_normals,
            ..Default::default()
        };
        // Normals turn with the rotation (the scale's direction effect is ignored); a
        // mirroring transform flips the winding.
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        for (p, nr) in model.positions.iter().zip(&model.normals) {
            let q = math::point(m, p.map(f64::from)).map(|v| v as f32);
            c.bounds.add(q);
            c.positions.push(q);
            let v = math::unit(math::vector(m, nr.map(f64::from)));
            c.normals.push(v.map(|x| x as f32));
        }
        if det < 0.0 {
            for t in c.indices.chunks_exact_mut(3) {
                t.swap(1, 2);
            }
        }
        copies.push(c);
    }
    let mut out = meshes(copies);
    // The images once: every copy's materials point at them.
    out.images = model.images.clone();
    out
}
