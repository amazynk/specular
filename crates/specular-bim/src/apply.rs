//! A BCF viewpoint on a building model: its components hidden, shown, coloured and selected
//! element by element (by IFC GUID), and its cameras and clipping planes between BCF's frame
//! (metres in the project's coordinates) and the mesh's.

use std::collections::HashMap;

use specular_mesh::ifc::Element;
use specular_mesh::{Material, Mesh, Part};

use crate::bcf::{BcfCamera, CameraKind, ClippingPlane, Components};

/// The colour selected elements are drawn in (linear).
pub const SELECTED: [f32; 4] = [0.95, 0.42, 0.05, 1.0];

/// Where the mesh sits in BCF's frame: a mesh point `p` is at `p * metres + origin`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub metres: f64,
    pub origin: [f64; 3],
}

impl Default for Frame {
    fn default() -> Frame {
        Frame {
            metres: 1.0,
            origin: [0.0; 3],
        }
    }
}

impl Frame {
    pub fn to_bcf(&self, p: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|k| p[k] * self.metres + self.origin[k])
    }

    pub fn from_bcf(&self, p: [f64; 3]) -> [f64; 3] {
        let m = if self.metres > 0.0 { self.metres } else { 1.0 };
        std::array::from_fn(|k| (p[k] - self.origin[k]) / m)
    }
}

/// A camera in the mesh's frame: eye, unit direction, up, and the field of view (radians,
/// perspective) or the height shown (mesh units, orthographic).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshCamera {
    pub eye: [f64; 3],
    pub direction: [f64; 3],
    pub up: [f64; 3],
    pub perspective: bool,
    pub fov: f64,
    pub height: f64,
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0.0 { v.map(|c| c / l) } else { v }
}

/// A BCF camera in the mesh's frame.
pub fn camera_from_bcf(c: &BcfCamera, f: &Frame) -> MeshCamera {
    let (perspective, fov, height) = match c.kind {
        CameraKind::Perspective { fov } => (true, fov.to_radians(), 1.0),
        CameraKind::Orthogonal { scale } => (false, 1.0, scale / f.metres.max(1e-12)),
    };
    MeshCamera {
        eye: f.from_bcf(c.viewpoint),
        direction: unit(c.direction),
        up: unit(c.up),
        perspective,
        fov,
        height,
    }
}

/// A mesh camera as BCF's, with the view's `aspect` (width over height).
pub fn camera_to_bcf(c: &MeshCamera, f: &Frame, aspect: f64) -> BcfCamera {
    BcfCamera {
        kind: if c.perspective {
            CameraKind::Perspective {
                fov: c.fov.to_degrees(),
            }
        } else {
            CameraKind::Orthogonal {
                scale: c.height * f.metres,
            }
        },
        viewpoint: f.to_bcf(c.eye),
        direction: unit(c.direction),
        up: unit(c.up),
        aspect: Some(aspect),
    }
}

/// A BCF clipping plane as the mesh's (normal, offset: `normal·p > offset` is cut away).
pub fn plane_from_bcf(p: &ClippingPlane, f: &Frame) -> ([f64; 3], f64) {
    let n = unit(p.direction);
    let at = f.from_bcf(p.location);
    (n, n[0] * at[0] + n[1] * at[1] + n[2] * at[2])
}

pub fn plane_to_bcf(n: [f64; 3], offset: f64, f: &Frame) -> ClippingPlane {
    let n = unit(n);
    let at = n.map(|c| c * offset);
    ClippingPlane {
        location: f.to_bcf(at),
        direction: n,
    }
}

/// The element owning vertex `v` (`elements` by first vertex).
pub fn element_of(elements: &[Element], v: u32) -> Option<usize> {
    let i = elements.partition_point(|e| e.first_vertex <= v);
    let i = i.checked_sub(1)?;
    (v < elements[i].first_vertex + elements[i].vertices).then_some(i)
}

/// How each element is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ElementLook {
    pub hidden: bool,
    /// Straight sRGB RGBA.
    pub colour: Option<[u8; 4]>,
    pub selected: bool,
}

/// What applying a viewpoint did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    pub hidden: usize,
    pub coloured: usize,
    pub selected: usize,
    /// GUIDs the viewpoint names that the model does not have.
    pub missing: usize,
}

/// Each element's look under `c`.
pub fn looks(elements: &[Element], c: &Components) -> (Vec<ElementLook>, Applied) {
    let by_guid: HashMap<&str, usize> = elements
        .iter()
        .enumerate()
        .map(|(i, e)| (e.global_id.as_str(), i))
        .collect();
    let mut out = vec![
        ElementLook {
            hidden: !c.default_visibility,
            ..ElementLook::default()
        };
        elements.len()
    ];
    let mut stats = Applied::default();
    let find = |g: &Option<String>, stats: &mut Applied| -> Option<usize> {
        let r = g.as_deref().and_then(|g| by_guid.get(g).copied());
        if r.is_none() {
            stats.missing += 1;
        }
        r
    };
    for e in &c.exceptions {
        if let Some(i) = find(&e.ifc_guid, &mut stats) {
            out[i].hidden = c.default_visibility;
        }
    }
    for k in &c.coloring {
        let Some(rgba) = k.rgba() else { continue };
        for e in &k.components {
            if let Some(i) = find(&e.ifc_guid, &mut stats) {
                out[i].colour = Some(rgba);
            }
        }
    }
    for e in &c.selection {
        if let Some(i) = find(&e.ifc_guid, &mut stats) {
            out[i].selected = true;
        }
    }
    stats.hidden = out.iter().filter(|l| l.hidden).count();
    stats.coloured = out.iter().filter(|l| l.colour.is_some()).count();
    stats.selected = out.iter().filter(|l| l.selected).count();
    (out, stats)
}

fn linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// `mesh` with each element drawn as `looks` says: hidden elements' triangles left out,
/// coloured and selected ones in materials of their own (selection wins). Positions and
/// normals are shared with nothing: this is a new mesh to upload.
pub fn restyle(mesh: &Mesh, elements: &[Element], looks: &[ElementLook]) -> Mesh {
    let tris = mesh.indices.len() / 3;
    let mut mat_of = vec![None; tris];
    for p in &mesh.parts {
        let end = ((p.first + p.count) as usize).min(tris);
        for m in mat_of.iter_mut().take(end).skip(p.first as usize) {
            *m = p.material;
        }
    }
    let mut materials: Vec<Material> = mesh.materials.clone();
    let mut extra: HashMap<[u8; 4], u32> = HashMap::new();
    let mut selected: Option<u32> = None;
    let mut colors = mesh.colors.clone();
    let mut chosen: Vec<(Option<u32>, u32)> = Vec::with_capacity(tris);
    for (t, &base) in mat_of.iter().enumerate() {
        let v = mesh.indices[3 * t];
        let look = element_of(elements, v).and_then(|i| looks.get(i).map(|l| (i, *l)));
        let mut m = base;
        if let Some((i, l)) = look {
            if l.hidden {
                continue;
            }
            let over = if l.selected {
                Some(*selected.get_or_insert_with(|| {
                    materials.push(Material {
                        name: "selected".into(),
                        base_color: SELECTED,
                        texture: None,
                    });
                    (materials.len() - 1) as u32
                }))
            } else {
                l.colour.map(|c| {
                    *extra.entry(c).or_insert_with(|| {
                        materials.push(Material {
                            name: format!("bcf {:02X}{:02X}{:02X}", c[0], c[1], c[2]),
                            base_color: [
                                linear(c[0]),
                                linear(c[1]),
                                linear(c[2]),
                                f32::from(c[3]) / 255.0,
                            ],
                            texture: None,
                        });
                        (materials.len() - 1) as u32
                    })
                })
            };
            if let Some(o) = over {
                m = Some(o);
                // The override's colour alone (vertex colours would tint it).
                if let Some(cols) = colors.as_mut() {
                    let e = &elements[i];
                    let r = e.first_vertex as usize..(e.first_vertex + e.vertices) as usize;
                    for c in cols.get_mut(r).into_iter().flatten() {
                        *c = [255; 4];
                    }
                }
            }
        }
        chosen.push((m, t as u32));
    }
    // Grouped by material, in order of first use.
    let mut order: Vec<Option<u32>> = Vec::new();
    let mut groups: HashMap<Option<u32>, Vec<u32>> = HashMap::new();
    for (m, t) in chosen {
        groups
            .entry(m)
            .or_insert_with(|| {
                order.push(m);
                Vec::new()
            })
            .push(t);
    }
    let mut indices = Vec::with_capacity(mesh.indices.len());
    let mut parts = Vec::with_capacity(order.len());
    for m in order {
        let first = (indices.len() / 3) as u32;
        let ts = &groups[&m];
        for &t in ts {
            indices.extend_from_slice(&mesh.indices[3 * t as usize..3 * t as usize + 3]);
        }
        parts.push(Part {
            first,
            count: ts.len() as u32,
            material: m,
        });
    }
    Mesh {
        indices,
        parts,
        materials,
        colors,
        ..mesh.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bcf::{Coloring, Component};

    fn el(guid: &str, first: u32, n: u32) -> Element {
        Element {
            global_id: guid.into(),
            first_vertex: first,
            vertices: n,
            ..Element::default()
        }
    }

    /// Three elements of one triangle each.
    fn model() -> (Mesh, Vec<Element>) {
        let positions: Vec<[f32; 3]> = (0..9).map(|i| [i as f32, 0.0, (i % 3) as f32]).collect();
        let mesh = Mesh {
            positions,
            normals: vec![[0.0, 1.0, 0.0]; 9],
            indices: (0..9).collect(),
            parts: vec![Part {
                first: 0,
                count: 3,
                material: None,
            }],
            ..Mesh::default()
        };
        (mesh, vec![el("A", 0, 3), el("B", 3, 3), el("C", 6, 3)])
    }

    #[test]
    fn components_hide_colour_and_select_by_guid() {
        let (mesh, els) = model();
        assert_eq!(element_of(&els, 4), Some(1));
        assert_eq!(element_of(&els, 9), None);
        let c = Components {
            selection: vec![Component::of("C")],
            default_visibility: true,
            exceptions: vec![Component::of("A"), Component::of("nope")],
            hints: None,
            coloring: vec![Coloring {
                color: "FF00FF00".into(),
                components: vec![Component::of("B")],
            }],
        };
        let (l, st) = looks(&els, &c);
        assert!(l[0].hidden && !l[1].hidden && !l[2].hidden);
        assert_eq!(l[1].colour, Some([0, 255, 0, 255]));
        assert!(l[2].selected);
        assert_eq!(
            (st.hidden, st.coloured, st.selected, st.missing),
            (1, 1, 1, 1)
        );
        let m = restyle(&mesh, &els, &l);
        assert_eq!(m.indices.len(), 6, "A's triangle is gone");
        assert_eq!(m.parts.len(), 2);
        let names: Vec<&str> = m
            .parts
            .iter()
            .map(|p| m.materials[p.material.unwrap() as usize].name.as_str())
            .collect();
        assert_eq!(names, ["bcf 00FF00", "selected"]);
        // Only C shown.
        let only = Components {
            default_visibility: false,
            exceptions: vec![Component::of("C")],
            ..Components::default()
        };
        let (l, _) = looks(&els, &only);
        assert_eq!(restyle(&mesh, &els, &l).indices, vec![6, 7, 8]);
    }

    #[test]
    fn cameras_and_planes_cross_frames() {
        let f = Frame {
            metres: 0.001,
            origin: [1000.0, 2000.0, 0.0],
        };
        let mc = MeshCamera {
            eye: [5000.0, -3000.0, 1600.0],
            direction: [0.0, 1.0, 0.0],
            up: [0.0, 0.0, 1.0],
            perspective: true,
            fov: 55f64.to_radians(),
            height: 1.0,
        };
        let b = camera_to_bcf(&mc, &f, 1.5);
        assert_eq!(b.viewpoint, [1005.0, 1997.0, 1.6]);
        let back = camera_from_bcf(&b, &f);
        for k in 0..3 {
            assert!((back.eye[k] - mc.eye[k]).abs() < 1e-6);
        }
        assert!((back.fov - mc.fov).abs() < 1e-12);
        let p = plane_to_bcf([0.0, 0.0, 1.0], 2500.0, &f);
        assert_eq!(p.location, [1000.0, 2000.0, 2.5]);
        let (n, d) = plane_from_bcf(&p, &f);
        assert_eq!(n, [0.0, 0.0, 1.0]);
        assert!((d - 2500.0).abs() < 1e-9);
    }
}
