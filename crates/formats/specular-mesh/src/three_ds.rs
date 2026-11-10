//! Autodesk 3D Studio (`.3ds`): the chunk tree's objects (vertices, faces with their
//! materials and smoothing groups, texture coordinates, the local frame), materials (diffuse
//! colour and texture map) and the keyframer's node hierarchy (pivot and the first key of
//! position, rotation and scale) for transforms. Without a keyframer the vertices stand as
//! stored (they are kept in world space).

use std::collections::HashMap;

use crate::builder::{Affine, Builder, srgb_to_linear};
use crate::poly::{Attr, Poly, emit};
use crate::{Error, ImageData, Limits, Material, Mesh, Resolver, Result, Up, bad};

struct Chunk<'a> {
    id: u16,
    data: &'a [u8],
}

/// The chunks of `b`, in order (a chunk running past its parent is cut there).
fn chunks(b: &[u8]) -> Vec<Chunk<'_>> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 6 <= b.len() {
        let id = u16::from_le_bytes([b[at], b[at + 1]]);
        let len = u32::from_le_bytes([b[at + 2], b[at + 3], b[at + 4], b[at + 5]]) as usize;
        if len < 6 {
            break;
        }
        let end = (at + len).min(b.len());
        out.push(Chunk {
            id,
            data: &b[at + 6..end],
        });
        at += len;
    }
    out
}

fn cstr(b: &[u8]) -> (String, usize) {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    (
        String::from_utf8_lossy(&b[..n]).into_owned(),
        (n + 1).min(b.len()),
    )
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn f32_at(b: &[u8], at: usize) -> Option<f32> {
    u32_at(b, at).map(f32::from_bits)
}

#[derive(Default)]
struct Object {
    name: String,
    positions: Vec<[f32; 3]>,
    faces: Vec<[u32; 3]>,
    uvs: Vec<[f32; 2]>,
    /// (material name, faces).
    face_mats: Vec<(String, Vec<u16>)>,
    smooth: Vec<u32>,
    /// The object's frame when it was made (rows x, y, z axes then origin).
    frame: Option<[[f32; 3]; 4]>,
}

#[derive(Default)]
struct Node {
    name: String,
    parent: i32,
    id: Option<u16>,
    pivot: [f32; 3],
    pos: Option<[f32; 3]>,
    rot: Option<[f32; 4]>,
    scale: Option<[f32; 3]>,
}

fn colour(c: &[u8]) -> Option<[f32; 3]> {
    // Float (0x10, 0x13) or byte (0x11, 0x12) RGB; the first given.
    for ch in chunks(c) {
        match ch.id {
            0x0010 | 0x0013 => {
                return Some([
                    f32_at(ch.data, 0)?,
                    f32_at(ch.data, 4)?,
                    f32_at(ch.data, 8)?,
                ]);
            }
            0x0011 | 0x0012 if ch.data.len() >= 3 => {
                return Some([0, 1, 2].map(|k| f32::from(ch.data[k]) / 255.0));
            }
            _ => {}
        }
    }
    None
}

/// The first key of a track: its value floats.
fn first_key(d: &[u8], n: usize) -> Option<Vec<f32>> {
    // flags u16, two u32 of unknown use, key count u32; each key: frame u32, spline flags
    // u16 and a float per set flag bit, then the value.
    let count = u32_at(d, 10)?;
    if count == 0 {
        return None;
    }
    let mut at = 14 + 4;
    let flags = u16_at(d, at)?;
    at += 2 + 4 * (flags & 0x1f).count_ones() as usize;
    (0..n).map(|k| f32_at(d, at + 4 * k)).collect()
}

pub(crate) fn read(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    if bytes.len() < 6 || u16_at(bytes, 0) != Some(0x4D4D) {
        return bad("not a 3DS file");
    }
    let top = chunks(bytes);
    let main = top
        .first()
        .ok_or_else(|| Error::Malformed("3DS: empty".into()))?;
    let mut objects: Vec<Object> = Vec::new();
    let mut materials: Vec<(String, Material, Option<String>)> = Vec::new();
    let mut nodes: Vec<Node> = Vec::new();
    let mut total = 0usize;
    for c in chunks(main.data) {
        match c.id {
            0x3D3D => {
                for e in chunks(c.data) {
                    match e.id {
                        0xAFFF => {
                            let mut name = String::new();
                            let mut m = Material::default();
                            let mut tex = None;
                            for mc in chunks(e.data) {
                                match mc.id {
                                    0xA000 => name = cstr(mc.data).0,
                                    0xA020 => {
                                        if let Some(c) = colour(mc.data) {
                                            m.base_color[..3]
                                                .copy_from_slice(&c.map(srgb_to_linear));
                                        }
                                    }
                                    0xA050 => {
                                        // Transparency percentage.
                                        for t in chunks(mc.data) {
                                            if t.id == 0x0030
                                                && let Some(p) = u16_at(t.data, 0)
                                            {
                                                m.base_color[3] =
                                                    1.0 - f32::from(p.min(100)) / 100.0;
                                            }
                                        }
                                    }
                                    0xA200 => {
                                        for t in chunks(mc.data) {
                                            if t.id == 0xA300 {
                                                tex = Some(cstr(t.data).0);
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            m.name = name.clone();
                            materials.push((name, m, tex));
                        }
                        0x4000 => {
                            let (name, n) = cstr(e.data);
                            for o in chunks(&e.data[n..]) {
                                if o.id != 0x4100 {
                                    continue;
                                }
                                let mut obj = Object {
                                    name: name.clone(),
                                    ..Object::default()
                                };
                                for t in chunks(o.data) {
                                    let d = t.data;
                                    match t.id {
                                        0x4110 => {
                                            let n = u16_at(d, 0).unwrap_or(0) as usize;
                                            total += n;
                                            if total > limits.max_vertices {
                                                return Err(Error::TooLarge(
                                                    "3DS with too many vertices".into(),
                                                ));
                                            }
                                            for k in 0..n {
                                                let p = [0, 1, 2]
                                                    .map(|a| f32_at(d, 2 + k * 12 + a * 4));
                                                let [Some(x), Some(y), Some(z)] = p else {
                                                    break;
                                                };
                                                obj.positions.push([x, y, z]);
                                            }
                                        }
                                        0x4120 => {
                                            let n = u16_at(d, 0).unwrap_or(0) as usize;
                                            for k in 0..n {
                                                let f =
                                                    [0, 1, 2].map(|a| u16_at(d, 2 + k * 8 + a * 2));
                                                let [Some(a), Some(b), Some(c)] = f else {
                                                    break;
                                                };
                                                obj.faces.push([a, b, c].map(u32::from));
                                            }
                                            let sub = (2 + n * 8).min(d.len());
                                            for s in chunks(&d[sub..]) {
                                                match s.id {
                                                    0x4130 => {
                                                        let (mname, k) = cstr(s.data);
                                                        let cnt =
                                                            u16_at(s.data, k).unwrap_or(0) as usize;
                                                        let list = (0..cnt)
                                                            .filter_map(|i| {
                                                                u16_at(s.data, k + 2 + i * 2)
                                                            })
                                                            .collect();
                                                        obj.face_mats.push((mname, list));
                                                    }
                                                    0x4150 => {
                                                        obj.smooth = s
                                                            .data
                                                            .chunks_exact(4)
                                                            .map(|c| {
                                                                u32::from_le_bytes([
                                                                    c[0], c[1], c[2], c[3],
                                                                ])
                                                            })
                                                            .collect();
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        0x4140 => {
                                            let n = u16_at(d, 0).unwrap_or(0) as usize;
                                            for k in 0..n {
                                                let (Some(u), Some(v)) =
                                                    (f32_at(d, 2 + k * 8), f32_at(d, 6 + k * 8))
                                                else {
                                                    break;
                                                };
                                                obj.uvs.push([u, 1.0 - v]);
                                            }
                                        }
                                        0x4160 => {
                                            let v: Option<Vec<f32>> =
                                                (0..12).map(|k| f32_at(d, k * 4)).collect();
                                            obj.frame = v.map(|v| {
                                                std::array::from_fn(|r| {
                                                    [v[r * 3], v[r * 3 + 1], v[r * 3 + 2]]
                                                })
                                            });
                                        }
                                        _ => {}
                                    }
                                }
                                objects.push(obj);
                            }
                        }
                        _ => {}
                    }
                }
            }
            0xB000 => {
                // Every node kind (ambient, object, camera and its target, lights) counts in
                // the hierarchy: a mesh may hang under a camera.
                for n in chunks(c.data) {
                    if !(0xB001..=0xB007).contains(&n.id) {
                        continue;
                    }
                    let mut node = Node {
                        parent: -1,
                        ..Node::default()
                    };
                    for k in chunks(n.data) {
                        match k.id {
                            0xB030 => node.id = u16_at(k.data, 0),
                            0xB010 => {
                                let (name, at) = cstr(k.data);
                                node.name = name;
                                node.parent =
                                    u16_at(k.data, at + 4).map_or(-1, |p| i32::from(p as i16));
                            }
                            0xB013 => {
                                if let (Some(x), Some(y), Some(z)) =
                                    (f32_at(k.data, 0), f32_at(k.data, 4), f32_at(k.data, 8))
                                {
                                    node.pivot = [x, y, z];
                                }
                            }
                            0xB020 => node.pos = first_key(k.data, 3).map(|v| [v[0], v[1], v[2]]),
                            0xB021 => {
                                node.rot = first_key(k.data, 4).map(|v| [v[0], v[1], v[2], v[3]])
                            }
                            0xB022 => node.scale = first_key(k.data, 3).map(|v| [v[0], v[1], v[2]]),
                            _ => {}
                        }
                    }
                    nodes.push(node);
                }
            }
            _ => {}
        }
    }
    let mut b = Builder::new(limits);
    b.up = Up::Z;
    // Materials, by name.
    let mut mat_index: HashMap<String, u32> = HashMap::new();
    for (name, mut m, tex) in materials {
        if let Some(t) = tex {
            m.texture = read_texture(&t, resolver, &mut b);
        }
        let i = b.add_material(m);
        mat_index.insert(name, i);
    }
    // World transforms of the keyframer's nodes.
    let worlds = node_worlds(&nodes);
    let mut used_frames = 0;
    for obj in &objects {
        if obj.faces.is_empty() || obj.positions.is_empty() {
            continue;
        }
        let node = nodes.iter().position(|n| n.name == obj.name);
        let xf = match (node, obj.frame) {
            (Some(i), Some(f)) => {
                used_frames += 1;
                // Back from the stored (world) vertices to the object's own frame, then to
                // the node's: world · T(−pivot) · frame⁻¹.
                let n = &nodes[i];
                let inv = invert_frame(&f);
                let piv = translate([-n.pivot[0], -n.pivot[1], -n.pivot[2]].map(f64::from));
                mul(&worlds[i], &mul(&piv, &inv))
            }
            _ => IDENT,
        };
        let counts = vec![3u32; obj.faces.len()];
        let indices: Vec<u32> = obj.faces.iter().flatten().copied().collect();
        let mut face_mat: Vec<Option<u32>> = vec![None; obj.faces.len()];
        for (name, list) in &obj.face_mats {
            let m = mat_index.get(name).copied();
            for &f in list {
                if let Some(slot) = face_mat.get_mut(f as usize) {
                    *slot = m;
                }
            }
        }
        // Smoothing groups: corners of a vertex in faces sharing a group share a normal;
        // group 0 is flat.
        let mut keys = Vec::with_capacity(indices.len());
        let mut slot: HashMap<(u32, u32), u32> = HashMap::new();
        let mut next = 0u32;
        for (f, tri) in obj.faces.iter().enumerate() {
            let g = obj.smooth.get(f).copied().unwrap_or(1);
            for &v in tri {
                if g == 0 {
                    keys.push(next);
                    next += 1;
                } else {
                    keys.push(*slot.entry((v, g)).or_insert_with(|| {
                        next += 1;
                        next - 1
                    }));
                }
            }
        }
        let mut p = Poly::new(&obj.positions, &counts, &indices);
        if obj.uvs.len() == obj.positions.len() {
            p.uvs = Attr::Vertex(&obj.uvs);
        }
        p.face_materials = Some(&face_mat);
        p.smooth_keys = Some(&keys);
        emit(&mut b, &p, &Affine([xf[0], xf[1], xf[2]]))?;
    }
    if !nodes.is_empty() && used_frames == 0 {
        b.note_once("keyframer nodes name no objects; vertices shown as stored");
    }
    b.finish()
}

fn read_texture(name: &str, resolver: &dyn Resolver, b: &mut Builder) -> Option<u32> {
    let max = b.limits.max_side_file;
    // 8.3 names are often stored in another case than the file has.
    let tries = [
        name.to_owned(),
        name.to_ascii_lowercase(),
        name.to_ascii_uppercase(),
    ];
    let mut reason = String::new();
    for t in &tries {
        match resolver.read(t, max) {
            Ok(bytes) => return b.add_image(ImageData::Bytes { bytes, mime: None }),
            Err(e) => reason = e,
        }
    }
    b.add_image(ImageData::Missing {
        name: name.to_owned(),
        reason,
    })
}

type M4 = [[f64; 4]; 4];

const IDENT: M4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn mul(a: &M4, b: &M4) -> M4 {
    let mut m = [[0.0; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            m[r][c] = (0..4).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    m
}

fn translate(t: [f64; 3]) -> M4 {
    let mut m = IDENT;
    m[0][3] = t[0];
    m[1][3] = t[1];
    m[2][3] = t[2];
    m
}

/// The inverse of an object frame (axes as rows of the stored matrix: columns of ours).
fn invert_frame(f: &[[f32; 3]; 4]) -> M4 {
    let mut m = IDENT;
    for c in 0..3 {
        for r in 0..3 {
            m[r][c] = f64::from(f[c][r]);
        }
        m[c][3] = f64::from(f[3][c]);
    }
    let a = Affine([m[0], m[1], m[2]]);
    let det = a.det();
    if det.abs() < 1e-30 || !det.is_finite() {
        return IDENT;
    }
    let n = a.normal_matrix();
    let mut inv = IDENT;
    for r in 0..3 {
        for c in 0..3 {
            inv[r][c] = n[c][r] / det;
        }
    }
    let t = [m[0][3], m[1][3], m[2][3]];
    for row in inv.iter_mut().take(3) {
        row[3] = -(row[0] * t[0] + row[1] * t[1] + row[2] * t[2]);
    }
    inv
}

/// Each node's world transform: its parent's · T(pos) · R(rot) · S(scale).
fn node_worlds(nodes: &[Node]) -> Vec<M4> {
    let local: Vec<M4> = nodes
        .iter()
        .map(|n| {
            let mut m = translate(n.pos.unwrap_or([0.0; 3]).map(f64::from));
            if let Some([angle, x, y, z]) = n.rot {
                let r = crate::collada::axis_angle(
                    [f64::from(x), f64::from(y), f64::from(z)],
                    -f64::from(angle).to_degrees(),
                );
                m = mul(&m, &r);
            }
            if let Some(s) = n.scale {
                let mut sm = IDENT;
                for k in 0..3 {
                    sm[k][k] = f64::from(s[k]);
                }
                m = mul(&m, &sm);
            }
            m
        })
        .collect();
    // Parents by node id where given, else by position.
    let by_id: HashMap<u16, usize> = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| n.id.map(|id| (id, i)))
        .collect();
    let mut world = vec![None; nodes.len()];
    fn resolve(
        i: usize,
        nodes: &[Node],
        local: &[M4],
        by_id: &HashMap<u16, usize>,
        world: &mut Vec<Option<M4>>,
        depth: usize,
    ) -> M4 {
        if let Some(w) = world[i] {
            return w;
        }
        let p = nodes[i].parent;
        let parent = if p < 0 || depth > 64 {
            None
        } else {
            by_id
                .get(&(p as u16))
                .copied()
                .or(Some(p as usize))
                .filter(|&j| j < nodes.len() && j != i)
        };
        let w = match parent {
            Some(j) => mul(
                &resolve(j, nodes, local, by_id, world, depth + 1),
                &local[i],
            ),
            None => local[i],
        };
        world[i] = Some(w);
        w
    }
    (0..nodes.len())
        .map(|i| resolve(i, nodes, &local, &by_id, &mut world, 0))
        .collect()
}
