//! Text scene formats, from their public descriptions:
//!
//! - AC3D (`.ac`, "AC3Db"): materials, then nested objects with a location, a rotation
//!   matrix, vertices and surfaces (polygons fanned; lines skipped) naming a material.
//! - DirectX text (`.x`, "xof 0302txt"/"0303txt"): Frames with FrameTransformMatrix around
//!   Meshes (vertices, faces, MeshNormals, MeshTextureCoords, MeshMaterialList with
//!   Material colours). Binary and compressed `.x` files are refused by name.
//! - BioVision motion capture (`.bvh`): the skeleton in its rest pose, each bone drawn as a
//!   thin box from its joint to the next; the motion is not played.

use crate::builder::{Affine, Builder};
use crate::{Limits, Material, Mesh, Result, Up, bad};

pub(crate) fn is_ac3d(d: &[u8]) -> bool {
    d.starts_with(b"AC3D")
}

pub(crate) fn is_x(d: &[u8]) -> bool {
    d.starts_with(b"xof 03")
}

pub(crate) fn is_bvh(d: &[u8]) -> bool {
    let t = String::from_utf8_lossy(&d[..d.len().min(64)]);
    t.trim_start().starts_with("HIERARCHY")
}

fn num(s: &str) -> f32 {
    s.parse().unwrap_or(0.0)
}

/// A 4×4 row-major matrix as the builder's affine (row vectors: p' = p M).
fn affine_rows(m: &[f32; 16]) -> Affine {
    let mut a = Affine::IDENTITY;
    for r in 0..3 {
        for c in 0..3 {
            a.0[c][r] = f64::from(m[r * 4 + c]);
        }
        a.0[r][3] = f64::from(m[12 + r]);
    }
    a
}

// ---------------------------------------------------------------- AC3D

/// A surface: its flags, material and corners (vertex, texture coordinate).
type AcSurface = (u32, u32, Vec<(usize, [f32; 2])>);

struct AcObject {
    loc: [f32; 3],
    rot: [f32; 9],
    verts: Vec<[f32; 3]>,
    surfs: Vec<AcSurface>,
    kids: usize,
}

pub(crate) fn read_ac3d(d: &[u8], limits: &Limits) -> Result<Mesh> {
    let text = String::from_utf8_lossy(d);
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .peekable();
    let head = lines.next().unwrap_or("");
    if !head.starts_with("AC3D") {
        return bad("not an AC3D file");
    }
    let mut b = Builder::new(limits);
    let mut materials = 0u32;
    // (object, transform of its parent, kids left).
    let mut stack: Vec<(Affine, usize)> = Vec::new();
    let mut objects = 0usize;
    while let Some(l) = lines.next() {
        let toks: Vec<&str> = l.split_whitespace().collect();
        match toks.first().copied() {
            Some("MATERIAL") => {
                let rgb = l.find(" rgb ").map(|i| {
                    let v: Vec<f32> = l[i + 5..].split_whitespace().take(3).map(num).collect();
                    v
                });
                let trans = l.find(" trans ").map_or(0.0, |i| {
                    num(l[i + 7..].split_whitespace().next().unwrap_or("0"))
                });
                let name = l.split('"').nth(1).unwrap_or("").to_owned();
                let c = rgb.unwrap_or_default();
                b.add_material(Material {
                    name,
                    base_color: [
                        c.first().copied().unwrap_or(0.8),
                        c.get(1).copied().unwrap_or(0.8),
                        c.get(2).copied().unwrap_or(0.8),
                        1.0 - trans.clamp(0.0, 1.0),
                    ],
                    texture: None,
                });
                materials += 1;
            }
            Some("OBJECT") => {
                objects += 1;
                if objects > 100_000 {
                    return bad("too many AC3D objects");
                }
                let mut o = AcObject {
                    loc: [0.0; 3],
                    rot: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                    verts: Vec::new(),
                    surfs: Vec::new(),
                    kids: 0,
                };
                while let Some(&l) = lines.peek() {
                    let t: Vec<&str> = l.split_whitespace().collect();
                    match t.first().copied() {
                        Some("loc") if t.len() >= 4 => o.loc = [num(t[1]), num(t[2]), num(t[3])],
                        Some("rot") if t.len() >= 10 => {
                            for k in 0..9 {
                                o.rot[k] = num(t[k + 1]);
                            }
                        }
                        Some("numvert") => {
                            lines.next();
                            let n: usize = t.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                            for _ in 0..n.min(limits.max_vertices) {
                                let v: Vec<f32> = lines
                                    .next()
                                    .unwrap_or("")
                                    .split_whitespace()
                                    .map(num)
                                    .collect();
                                o.verts.push([
                                    v.first().copied().unwrap_or(0.0),
                                    v.get(1).copied().unwrap_or(0.0),
                                    v.get(2).copied().unwrap_or(0.0),
                                ]);
                            }
                            continue;
                        }
                        Some("numsurf") => {
                            lines.next();
                            let n: usize = t.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                            for _ in 0..n.min(limits.max_triangles) {
                                let (mut flags, mut mat, mut refs) = (0u32, 0u32, Vec::new());
                                while let Some(l) = lines.next() {
                                    let s: Vec<&str> = l.split_whitespace().collect();
                                    match s.first().copied() {
                                        Some("SURF") => {
                                            flags = s
                                                .get(1)
                                                .and_then(|v| {
                                                    u32::from_str_radix(
                                                        v.trim_start_matches("0x"),
                                                        16,
                                                    )
                                                    .ok()
                                                })
                                                .unwrap_or(0)
                                        }
                                        Some("mat") => {
                                            mat = s.get(1).and_then(|v| v.parse().ok()).unwrap_or(0)
                                        }
                                        Some("refs") => {
                                            let k: usize =
                                                s.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                                            for _ in 0..k.min(1 << 16) {
                                                let r: Vec<&str> = lines
                                                    .next()
                                                    .unwrap_or("")
                                                    .split_whitespace()
                                                    .collect();
                                                refs.push((
                                                    r.first()
                                                        .and_then(|v| v.parse().ok())
                                                        .unwrap_or(0),
                                                    [
                                                        num(r.get(1).unwrap_or(&"0")),
                                                        num(r.get(2).unwrap_or(&"0")),
                                                    ],
                                                ));
                                            }
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                o.surfs.push((flags, mat, refs));
                            }
                            continue;
                        }
                        Some("kids") => {
                            o.kids = t.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                            lines.next();
                            break;
                        }
                        Some("OBJECT") | Some("MATERIAL") => break,
                        _ => {}
                    }
                    lines.next();
                }
                // Its transform: the parent's, then its own rotation and location.
                let parent = stack.last().map_or(Affine::IDENTITY, |s| s.0);
                let mut own = Affine::IDENTITY;
                for r in 0..3 {
                    for c in 0..3 {
                        own.0[r][c] = f64::from(o.rot[r * 3 + c]);
                    }
                    own.0[r][3] = f64::from(o.loc[r]);
                }
                let m = parent.then_after(&own);
                for (flags, mat, refs) in &o.surfs {
                    if flags & 15 != 0 || refs.len() < 3 {
                        continue;
                    }
                    b.material((*mat < materials).then_some(*mat));
                    let mut idx = Vec::with_capacity(refs.len());
                    for (i, uv) in refs {
                        let Some(p) = o.verts.get(*i) else { continue };
                        idx.push(b.vertex(m.point(*p), None, None, Some([uv[0], 1.0 - uv[1]]))?);
                    }
                    for k in 1..idx.len().saturating_sub(1) {
                        b.triangle(idx[0], idx[k], idx[k + 1])?;
                    }
                }
                // Pop finished parents, then push this one if it has kids.
                if let Some(top) = stack.last_mut() {
                    top.1 = top.1.saturating_sub(1);
                }
                while stack.last().is_some_and(|s| s.1 == 0) {
                    stack.pop();
                }
                if o.kids > 0 {
                    stack.push((m, o.kids));
                }
            }
            _ => {}
        }
    }
    b.up = Up::Y;
    b.finish()
}

// ---------------------------------------------------------------- DirectX .x

/// A node of a `.x` file: its template name, its own name and its data tokens.
struct XNode {
    kind: String,
    data: Vec<String>,
    kids: Vec<XNode>,
}

fn x_tokens(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '#' => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '{' | '}' => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                out.push(c.to_string());
            }
            ';' | ',' => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            '"' => {
                cur.clear();
                for c in chars.by_ref() {
                    if c == '"' {
                        break;
                    }
                    cur.push(c);
                }
                out.push(std::mem::take(&mut cur));
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Parses the top-level `kind [name] { data… children… }` nodes (templates skipped).
fn x_nodes(toks: &[String], at: &mut usize, out: &mut Vec<XNode>) {
    while *at < toks.len() {
        let opens = toks.get(*at + 1).is_some_and(|t| t == "{")
            || (toks.get(*at + 2).is_some_and(|t| t == "{") && toks[*at].parse::<f64>().is_err());
        if !opens {
            *at += 1;
            continue;
        }
        if toks[*at] == "template" {
            while toks.get(*at).is_some_and(|t| t != "{") {
                *at += 1;
            }
            let mut level = 0;
            while *at < toks.len() {
                match toks[*at].as_str() {
                    "{" => level += 1,
                    "}" => level -= 1,
                    _ => {}
                }
                *at += 1;
                if level == 0 {
                    break;
                }
            }
            continue;
        }
        x_nodes_one(toks, at, 0, out);
    }
}

/// Parses one node at `toks[*at]`.
fn x_nodes_one(slice: &[String], at: &mut usize, depth: usize, out: &mut Vec<XNode>) {
    let mut local = *at;
    let brace = if slice.get(local + 1).is_some_and(|t| t == "{") {
        local + 1
    } else {
        local + 2
    };
    let kind = slice[local].clone();
    local = brace + 1;
    let mut node = XNode {
        kind,
        data: Vec::new(),
        kids: Vec::new(),
    };
    while let Some(t) = slice.get(local) {
        if t == "}" {
            local += 1;
            break;
        }
        if slice.get(local + 1).is_some_and(|t| t == "{")
            || (slice.get(local + 2).is_some_and(|t| t == "{") && t.parse::<f64>().is_err())
        {
            if depth < 64 {
                x_nodes_one(slice, &mut local, depth + 1, &mut node.kids);
            } else {
                local += 1;
            }
            continue;
        }
        if t == "{" {
            local += 1;
            while slice.get(local).is_some_and(|t| t != "}") {
                local += 1;
            }
            local += 1;
            continue;
        }
        node.data.push(t.clone());
        local += 1;
    }
    out.push(node);
    *at = local;
}

fn x_mesh(n: &XNode, m: &Affine, b: &mut Builder, mats: &mut Vec<Material>) -> Result<()> {
    let f = |s: &String| s.parse::<f32>().unwrap_or(0.0);
    let d = &n.data;
    let nv: usize = d.first().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut at = 1;
    let mut pos = Vec::with_capacity(nv.min(1 << 20));
    for _ in 0..nv {
        let Some(v) = d.get(at..at + 3) else {
            return bad(".x vertices");
        };
        pos.push([f(&v[0]), f(&v[1]), f(&v[2])]);
        at += 3;
    }
    let nf: usize = d.get(at).and_then(|v| v.parse().ok()).unwrap_or(0);
    at += 1;
    let mut faces = Vec::with_capacity(nf.min(1 << 20));
    for _ in 0..nf {
        let k: usize = d.get(at).and_then(|v| v.parse().ok()).unwrap_or(0);
        let Some(ix) = d.get(at + 1..at + 1 + k) else {
            return bad(".x faces");
        };
        faces.push(
            ix.iter()
                .map(|v| v.parse::<usize>().unwrap_or(0))
                .collect::<Vec<_>>(),
        );
        at += 1 + k;
    }
    let mut uvs: Option<Vec<[f32; 2]>> = None;
    let mut face_mat: Vec<u32> = Vec::new();
    let base = mats.len() as u32;
    for k in &n.kids {
        match k.kind.as_str() {
            "MeshTextureCoords" => {
                let c: usize = k.data.first().and_then(|v| v.parse().ok()).unwrap_or(0);
                uvs = Some(
                    (0..c)
                        .map(|i| {
                            [
                                f(&k.data[1 + 2 * i.min((k.data.len() - 1) / 2)]),
                                k.data.get(2 + 2 * i).map_or(0.0, f),
                            ]
                        })
                        .collect(),
                );
            }
            "MeshMaterialList" => {
                let nfi: usize = k.data.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                face_mat = (0..nfi)
                    .map(|i| k.data.get(2 + i).and_then(|v| v.parse().ok()).unwrap_or(0))
                    .collect();
                for mk in k.kids.iter().filter(|c| c.kind == "Material") {
                    let c: Vec<f32> = mk.data.iter().take(4).map(f).collect();
                    mats.push(Material {
                        name: String::new(),
                        base_color: [
                            c.first().copied().unwrap_or(0.8),
                            c.get(1).copied().unwrap_or(0.8),
                            c.get(2).copied().unwrap_or(0.8),
                            c.get(3).copied().unwrap_or(1.0),
                        ],
                        texture: None,
                    });
                }
            }
            _ => {}
        }
    }
    let nmats = mats.len() as u32;
    let mut index = Vec::with_capacity(pos.len());
    for (i, p) in pos.iter().enumerate() {
        let uv = uvs.as_ref().and_then(|u| u.get(i)).copied();
        index.push(b.vertex(m.point(*p), None, None, uv)?);
    }
    // Direct3D winds clockwise (left-handed): reverse for counter-clockwise.
    for (fi, face) in faces.iter().enumerate() {
        let mat = face_mat
            .get(fi)
            .or(face_mat.last())
            .map(|&k| base + k)
            .filter(|&k| k < nmats);
        b.material(mat);
        let idx: Vec<u32> = face.iter().filter_map(|&k| index.get(k).copied()).collect();
        for k in 1..idx.len().saturating_sub(1) {
            b.triangle(idx[0], idx[k + 1], idx[k])?;
        }
    }
    Ok(())
}

fn x_walk(
    nodes: &[XNode],
    m: &Affine,
    b: &mut Builder,
    mats: &mut Vec<Material>,
    depth: usize,
) -> Result<()> {
    for n in nodes {
        match n.kind.as_str() {
            "Frame" if depth < 64 => {
                let own = n
                    .kids
                    .iter()
                    .find(|k| k.kind == "FrameTransformMatrix")
                    .and_then(|k| {
                        let v: Vec<f32> = k.data.iter().filter_map(|s| s.parse().ok()).collect();
                        (v.len() >= 16).then(|| {
                            let mut a = [0f32; 16];
                            a.copy_from_slice(&v[..16]);
                            affine_rows(&a)
                        })
                    })
                    .unwrap_or(Affine::IDENTITY);
                let mm = m.then_after(&own);
                x_walk(&n.kids, &mm, b, mats, depth + 1)?;
            }
            "Mesh" => x_mesh(n, m, b, mats)?,
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn read_x(d: &[u8], limits: &Limits) -> Result<Mesh> {
    let format = d.get(8..12).unwrap_or_default();
    if format != b"txt " {
        return bad("binary and compressed DirectX .x files are not read (only text ones)");
    }
    let text = String::from_utf8_lossy(&d[16.min(d.len())..]);
    let toks = x_tokens(&text);
    let mut nodes = Vec::new();
    let mut at = 0;
    x_nodes(&toks, &mut at, &mut nodes);
    let mut b = Builder::new(limits);
    let mut mats = Vec::new();
    x_walk(&nodes, &Affine::IDENTITY, &mut b, &mut mats, 0)?;
    for m in mats {
        b.add_material(m);
    }
    b.up = Up::Y;
    b.finish()
}

// ---------------------------------------------------------------- BVH

pub(crate) fn read_bvh(d: &[u8], limits: &Limits) -> Result<Mesh> {
    let text = String::from_utf8_lossy(d);
    let hierarchy = text.split("MOTION").next().unwrap_or("");
    let frames = text
        .split("Frames:")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .and_then(|v| v.parse::<u64>().ok());
    // Joints' absolute rest positions and the bones between them.
    let mut stack: Vec<[f32; 3]> = Vec::new();
    let mut pending: Option<[f32; 3]> = None;
    let mut bones: Vec<([f32; 3], [f32; 3])> = Vec::new();
    let toks: Vec<&str> = hierarchy.split_whitespace().collect();
    let mut i = 0;
    let mut joints = 0;
    while i < toks.len() {
        match toks[i] {
            "{" => {
                let p = pending
                    .take()
                    .unwrap_or_else(|| stack.last().copied().unwrap_or([0.0; 3]));
                stack.push(p);
            }
            "}" => {
                stack.pop();
            }
            "OFFSET" if i + 3 < toks.len() => {
                let o = [num(toks[i + 1]), num(toks[i + 2]), num(toks[i + 3])];
                let base = stack.last().copied().unwrap_or([0.0; 3]);
                let p = [base[0] + o[0], base[1] + o[1], base[2] + o[2]];
                // The OFFSET belongs to the node just opened: move it there.
                if let Some(top) = stack.last_mut() {
                    let parent = *top;
                    *top = p;
                    if stack.len() > 1 && o != [0.0; 3] {
                        bones.push((parent, p));
                    }
                }
                i += 3;
            }
            "ROOT" | "JOINT" => joints += 1,
            _ => {}
        }
        i += 1;
    }
    if bones.is_empty() {
        return bad("a BVH file without bones");
    }
    let size = bones
        .iter()
        .map(|(a, b)| {
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        })
        .fold(0.0f32, f32::max);
    let r = (size * 0.06).max(1e-3);
    let mut b = Builder::new(limits);
    for (p, q) in &bones {
        let dir = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if len <= 0.0 {
            continue;
        }
        let u = [dir[0] / len, dir[1] / len, dir[2] / len];
        let a = if u[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let cross = |x: [f32; 3], y: [f32; 3]| {
            [
                x[1] * y[2] - x[2] * y[1],
                x[2] * y[0] - x[0] * y[2],
                x[0] * y[1] - x[1] * y[0],
            ]
        };
        let s = cross(u, a);
        let sl = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
        let s = [s[0] / sl * r, s[1] / sl * r, s[2] / sl * r];
        let t = cross(u, s);
        // A four-sided prism from p to q, tapering to the child joint.
        let ring = |c: [f32; 3], k: f32| -> [[f32; 3]; 4] {
            [
                [c[0] + (s[0]) * k, c[1] + (s[1]) * k, c[2] + (s[2]) * k],
                [c[0] + (t[0]) * k, c[1] + (t[1]) * k, c[2] + (t[2]) * k],
                [c[0] - (s[0]) * k, c[1] - (s[1]) * k, c[2] - (s[2]) * k],
                [c[0] - (t[0]) * k, c[1] - (t[1]) * k, c[2] - (t[2]) * k],
            ]
        };
        let (r0, r1) = (ring(*p, 1.0), ring(*q, 0.4));
        let mut v0 = [0u32; 4];
        let mut v1 = [0u32; 4];
        for k in 0..4 {
            v0[k] = b.vertex(r0[k], None, None, None)?;
            v1[k] = b.vertex(r1[k], None, None, None)?;
        }
        for k in 0..4 {
            let n = (k + 1) % 4;
            b.triangle(v0[k], v0[n], v1[n])?;
            b.triangle(v0[k], v1[n], v1[k])?;
        }
        b.triangle(v0[0], v0[2], v0[1])?;
        b.triangle(v0[0], v0[3], v0[2])?;
        b.triangle(v1[0], v1[1], v1[2])?;
        b.triangle(v1[0], v1[2], v1[3])?;
    }
    b.note(format!(
        "motion capture skeleton of {joints} joints in its rest pose{}",
        frames
            .map(|f| format!("; {f} frames of motion not played"))
            .unwrap_or_default()
    ));
    b.up = Up::Y;
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_x_file() {
        let x = b"xof 0302txt 0032\nFrame Root {\n FrameTransformMatrix { 1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,5,1;; }\n Mesh tri {\n  3;\n  0;0;0;, 1;0;0;, 0;1;0;;\n  1;\n  3;0,1,2;;\n }\n}\n";
        assert!(is_x(x));
        let m = read_x(x, &Limits::default()).unwrap();
        assert_eq!(m.indices.len(), 3);
        assert!(
            m.positions.iter().all(|p| (p[2] - 5.0).abs() < 1e-6),
            "{:?}",
            m.positions
        );
    }

    #[test]
    fn small_ac3d() {
        let ac = b"AC3Db\nMATERIAL \"red\" rgb 1 0 0  amb 0.2 0.2 0.2  emis 0 0 0  spec 0.5 0.5 0.5  shi 10  trans 0\nOBJECT world\nkids 1\nOBJECT poly\nname \"q\"\nloc 0 0 1\nnumvert 4\n0 0 0\n1 0 0\n1 1 0\n0 1 0\nnumsurf 1\nSURF 0x10\nmat 0\nrefs 4\n0 0 0\n1 1 0\n2 1 1\n3 0 1\nkids 0\n";
        let m = read_ac3d(ac, &Limits::default()).unwrap();
        assert_eq!(m.indices.len(), 6);
        assert!(m.positions.iter().all(|p| (p[2] - 1.0).abs() < 1e-6));
        assert_eq!(m.materials[0].base_color[0], 1.0);
    }

    #[test]
    fn small_bvh() {
        let b = b"HIERARCHY\nROOT Hips\n{\n OFFSET 0 0 0\n CHANNELS 6 Xposition Yposition Zposition Zrotation Xrotation Yrotation\n JOINT Spine\n {\n  OFFSET 0 10 0\n  CHANNELS 3 Zrotation Xrotation Yrotation\n  End Site\n  {\n   OFFSET 0 5 0\n  }\n }\n}\nMOTION\nFrames: 2\nFrame Time: 0.033\n";
        assert!(is_bvh(b));
        let m = read_bvh(b, &Limits::default()).unwrap();
        let top = m.positions.iter().map(|p| p[1]).fold(0.0f32, f32::max);
        assert!((top - 15.0).abs() < 1.5, "{top}");
    }
}
