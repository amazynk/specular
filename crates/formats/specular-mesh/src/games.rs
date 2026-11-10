//! Game and hobby model formats, from their public descriptions:
//!
//! - Quake II models (`.md2`, "IDP2" version 8): the first frame's vertices (bytes scaled and
//!   moved per frame), triangles and skin coordinates; skins are not loaded.
//! - MilkShape 3D (`.ms3d`, "MS3D000000" version 3 or 4): vertices, triangles with their
//!   normals and groups with their materials' diffuse colours.
//! - MagicaVoxel (`.vox`): each model's voxels as cubes (faces between voxels left out),
//!   coloured from the file's palette (or MagicaVoxel's default), placed by the scene's
//!   translations.

use crate::builder::Builder;
use crate::{Limits, Material, Mesh, Result, Up, bad};

fn u16le(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
fn i32le(d: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}
fn f32le(d: &[u8], at: usize) -> Option<f32> {
    Some(f32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

pub(crate) fn is_md2(d: &[u8]) -> bool {
    d.starts_with(b"IDP2") && i32le(d, 4) == Some(8)
}

pub(crate) fn is_ms3d(d: &[u8]) -> bool {
    d.starts_with(b"MS3D000000") && matches!(i32le(d, 10), Some(3 | 4))
}

pub(crate) fn is_vox(d: &[u8]) -> bool {
    d.starts_with(b"VOX ") && d.get(8..12) == Some(b"MAIN")
}

pub(crate) fn read_md2(d: &[u8], limits: &Limits) -> Result<Mesh> {
    let h = |k: usize| i32le(d, 4 * k).unwrap_or(-1);
    let (sw, sh) = (h(2).max(1) as f32, h(3).max(1) as f32);
    let (nv, nst, ntri, nframes) = (h(6), h(7), h(8), h(10));
    let (ofs_st, ofs_tris, ofs_frames) = (h(12), h(13), h(14));
    if nv <= 0 || ntri <= 0 || nframes <= 0 || nv > 1 << 16 || ntri > 1 << 20 {
        return bad("MD2 counts");
    }
    let (nv, nst, ntri) = (nv as usize, nst.max(0) as usize, ntri as usize);
    let fr = ofs_frames.max(0) as usize;
    let scale = [
        f32le(d, fr).unwrap_or(1.0),
        f32le(d, fr + 4).unwrap_or(1.0),
        f32le(d, fr + 8).unwrap_or(1.0),
    ];
    let mv = [
        f32le(d, fr + 12).unwrap_or(0.0),
        f32le(d, fr + 16).unwrap_or(0.0),
        f32le(d, fr + 20).unwrap_or(0.0),
    ];
    let verts = d
        .get(fr + 40..fr + 40 + 4 * nv)
        .ok_or(crate::Error::Malformed("MD2 frame".into()))?;
    let pos: Vec<[f32; 3]> = verts
        .chunks_exact(4)
        .map(|v| {
            [
                f32::from(v[0]) * scale[0] + mv[0],
                f32::from(v[1]) * scale[1] + mv[1],
                f32::from(v[2]) * scale[2] + mv[2],
            ]
        })
        .collect();
    let st = |i: usize| -> Option<[f32; 2]> {
        let o = ofs_st.max(0) as usize + 4 * i;
        Some([
            f32::from(u16le(d, o)? as i16) / sw,
            f32::from(u16le(d, o + 2)? as i16) / sh,
        ])
    };
    let mut b = Builder::new(limits);
    b.reserve_vertices(3 * ntri)?;
    b.reserve_triangles(ntri)?;
    for t in 0..ntri {
        let o = ofs_tris.max(0) as usize + 12 * t;
        let mut idx = [0u32; 3];
        for (k, slot) in idx.iter_mut().enumerate() {
            let vi = usize::from(
                u16le(d, o + 2 * k).ok_or(crate::Error::Malformed("MD2 triangles".into()))?,
            );
            let si = usize::from(u16le(d, o + 6 + 2 * k).unwrap_or(0));
            let p = *pos
                .get(vi)
                .ok_or(crate::Error::Malformed("MD2 vertex index".into()))?;
            let uv = (si < nst).then(|| st(si)).flatten();
            *slot = b.vertex(p, None, None, uv)?;
        }
        // Quake's triangles wind clockwise.
        b.triangle(idx[0], idx[2], idx[1])?;
    }
    b.note("the skins (PCX pictures beside the model) are not loaded");
    if nframes > 1 {
        b.note(format!("first of {nframes} animation frames"));
    }
    b.up = Up::Z;
    let mut m = b.finish()?;
    m.title = Some(
        String::from_utf8_lossy(
            d.get(fr + 24..fr + 40)
                .unwrap_or_default()
                .split(|&c| c == 0)
                .next()
                .unwrap_or_default(),
        )
        .into_owned(),
    )
    .filter(|s| !s.is_empty());
    Ok(m)
}

pub(crate) fn read_ms3d(d: &[u8], limits: &Limits) -> Result<Mesh> {
    let mut at = 14;
    let nv = usize::from(u16le(d, at).ok_or(crate::Error::Malformed("MS3D vertices".into()))?);
    at += 2;
    let mut pos = Vec::with_capacity(nv);
    for _ in 0..nv {
        pos.push([
            f32le(d, at + 1).unwrap_or(0.0),
            f32le(d, at + 5).unwrap_or(0.0),
            f32le(d, at + 9).unwrap_or(0.0),
        ]);
        at += 15;
    }
    let nt = usize::from(u16le(d, at).ok_or(crate::Error::Malformed("MS3D triangles".into()))?);
    at += 2;
    let tris_at = at;
    at += 70 * nt;
    // Groups: their triangles and material.
    let ng = usize::from(u16le(d, at).unwrap_or(0));
    at += 2;
    let mut group_of = vec![None; nt];
    for _ in 0..ng {
        let n = usize::from(u16le(d, at + 33).unwrap_or(0));
        let tris: Vec<usize> = (0..n)
            .filter_map(|k| u16le(d, at + 35 + 2 * k).map(usize::from))
            .collect();
        let mat = d.get(at + 35 + 2 * n).map(|&m| m as i8);
        for t in tris {
            if t < nt {
                group_of[t] = mat.filter(|&m| m >= 0).map(|m| m as u32);
            }
        }
        at += 36 + 2 * n;
    }
    let nm = usize::from(u16le(d, at).unwrap_or(0));
    at += 2;
    let mut b = Builder::new(limits);
    for k in 0..nm {
        let o = at + 361 * k;
        let name = String::from_utf8_lossy(
            d.get(o..o + 32)
                .unwrap_or_default()
                .split(|&c| c == 0)
                .next()
                .unwrap_or_default(),
        )
        .into_owned();
        let diffuse = [
            f32le(d, o + 48).unwrap_or(0.8),
            f32le(d, o + 52).unwrap_or(0.8),
            f32le(d, o + 56).unwrap_or(0.8),
            f32le(d, o + 340).unwrap_or(1.0).clamp(0.0, 1.0),
        ];
        b.add_material(Material {
            name,
            base_color: diffuse.map(|v| v.clamp(0.0, 1.0)),
            texture: None,
        });
    }
    b.reserve_vertices(3 * nt)?;
    b.reserve_triangles(nt)?;
    for (t, g) in group_of.iter().enumerate() {
        let o = tris_at + 70 * t;
        let mut idx = [0u32; 3];
        for (k, slot) in idx.iter_mut().enumerate() {
            let vi = usize::from(
                u16le(d, o + 2 + 2 * k).ok_or(crate::Error::Malformed("MS3D triangle".into()))?,
            );
            let p = *pos
                .get(vi)
                .ok_or(crate::Error::Malformed("MS3D vertex index".into()))?;
            let n = [
                f32le(d, o + 8 + 12 * k).unwrap_or(0.0),
                f32le(d, o + 12 + 12 * k).unwrap_or(0.0),
                f32le(d, o + 16 + 12 * k).unwrap_or(0.0),
            ];
            let uv = [
                f32le(d, o + 44 + 4 * k).unwrap_or(0.0),
                f32le(d, o + 56 + 4 * k).unwrap_or(0.0),
            ];
            *slot = b.vertex(p, Some(n), None, Some(uv))?;
        }
        b.material(g.filter(|&m| (m as usize) < nm));
        b.triangle(idx[0], idx[1], idx[2])?;
    }
    if nm > 0 {
        b.note("textures are not loaded; materials show their diffuse colour");
    }
    b.up = Up::Y;
    b.finish()
}

/// MagicaVoxel's palette when a file has none (its documented default, ABGR words).
const DEFAULT_PALETTE: [u32; 256] = [
    0x00000000, 0xffffffff, 0xffccffff, 0xff99ffff, 0xff66ffff, 0xff33ffff, 0xff00ffff, 0xffffccff,
    0xffccccff, 0xff99ccff, 0xff66ccff, 0xff33ccff, 0xff00ccff, 0xffff99ff, 0xffcc99ff, 0xff9999ff,
    0xff6699ff, 0xff3399ff, 0xff0099ff, 0xffff66ff, 0xffcc66ff, 0xff9966ff, 0xff6666ff, 0xff3366ff,
    0xff0066ff, 0xffff33ff, 0xffcc33ff, 0xff9933ff, 0xff6633ff, 0xff3333ff, 0xff0033ff, 0xffff00ff,
    0xffcc00ff, 0xff9900ff, 0xff6600ff, 0xff3300ff, 0xff0000ff, 0xffffffcc, 0xffccffcc, 0xff99ffcc,
    0xff66ffcc, 0xff33ffcc, 0xff00ffcc, 0xffffcccc, 0xffcccccc, 0xff99cccc, 0xff66cccc, 0xff33cccc,
    0xff00cccc, 0xffff99cc, 0xffcc99cc, 0xff9999cc, 0xff6699cc, 0xff3399cc, 0xff0099cc, 0xffff66cc,
    0xffcc66cc, 0xff9966cc, 0xff6666cc, 0xff3366cc, 0xff0066cc, 0xffff33cc, 0xffcc33cc, 0xff9933cc,
    0xff6633cc, 0xff3333cc, 0xff0033cc, 0xffff00cc, 0xffcc00cc, 0xff9900cc, 0xff6600cc, 0xff3300cc,
    0xff0000cc, 0xffffff99, 0xffccff99, 0xff99ff99, 0xff66ff99, 0xff33ff99, 0xff00ff99, 0xffffcc99,
    0xffcccc99, 0xff99cc99, 0xff66cc99, 0xff33cc99, 0xff00cc99, 0xffff9999, 0xffcc9999, 0xff999999,
    0xff669999, 0xff339999, 0xff009999, 0xffff6699, 0xffcc6699, 0xff996699, 0xff666699, 0xff336699,
    0xff006699, 0xffff3399, 0xffcc3399, 0xff993399, 0xff663399, 0xff333399, 0xff003399, 0xffff0099,
    0xffcc0099, 0xff990099, 0xff660099, 0xff330099, 0xff000099, 0xffffff66, 0xffccff66, 0xff99ff66,
    0xff66ff66, 0xff33ff66, 0xff00ff66, 0xffffcc66, 0xffcccc66, 0xff99cc66, 0xff66cc66, 0xff33cc66,
    0xff00cc66, 0xffff9966, 0xffcc9966, 0xff999966, 0xff669966, 0xff339966, 0xff009966, 0xffff6666,
    0xffcc6666, 0xff996666, 0xff666666, 0xff336666, 0xff006666, 0xffff3366, 0xffcc3366, 0xff993366,
    0xff663366, 0xff333366, 0xff003366, 0xffff0066, 0xffcc0066, 0xff990066, 0xff660066, 0xff330066,
    0xff000066, 0xffffff33, 0xffccff33, 0xff99ff33, 0xff66ff33, 0xff33ff33, 0xff00ff33, 0xffffcc33,
    0xffcccc33, 0xff99cc33, 0xff66cc33, 0xff33cc33, 0xff00cc33, 0xffff9933, 0xffcc9933, 0xff999933,
    0xff669933, 0xff339933, 0xff009933, 0xffff6633, 0xffcc6633, 0xff996633, 0xff666633, 0xff336633,
    0xff006633, 0xffff3333, 0xffcc3333, 0xff993333, 0xff663333, 0xff333333, 0xff003333, 0xffff0033,
    0xffcc0033, 0xff990033, 0xff660033, 0xff330033, 0xff000033, 0xffffff00, 0xffccff00, 0xff99ff00,
    0xff66ff00, 0xff33ff00, 0xff00ff00, 0xffffcc00, 0xffcccc00, 0xff99cc00, 0xff66cc00, 0xff33cc00,
    0xff00cc00, 0xffff9900, 0xffcc9900, 0xff999900, 0xff669900, 0xff339900, 0xff009900, 0xffff6600,
    0xffcc6600, 0xff996600, 0xff666600, 0xff336600, 0xff006600, 0xffff3300, 0xffcc3300, 0xff993300,
    0xff663300, 0xff333300, 0xff003300, 0xffff0000, 0xffcc0000, 0xff990000, 0xff660000, 0xff330000,
    0xff0000ee, 0xff0000dd, 0xff0000bb, 0xff0000aa, 0xff000088, 0xff000077, 0xff000055, 0xff000044,
    0xff000022, 0xff000011, 0xff00ee00, 0xff00dd00, 0xff00bb00, 0xff00aa00, 0xff008800, 0xff007700,
    0xff005500, 0xff004400, 0xff002200, 0xff001100, 0xffee0000, 0xffdd0000, 0xffbb0000, 0xffaa0000,
    0xff880000, 0xff770000, 0xff550000, 0xff440000, 0xff220000, 0xff110000, 0xffeeeeee, 0xffdddddd,
    0xffbbbbbb, 0xffaaaaaa, 0xff888888, 0xff777777, 0xff555555, 0xff444444, 0xff222222, 0xff111111,
];

/// One voxel model: its size and voxels (x, y, z, colour index).
struct VoxModel {
    size: [i32; 3],
    voxels: Vec<[u8; 4]>,
}

/// Reads a scene node's translation ("_t" in its first frame's dictionary).
fn vox_dict(d: &[u8], mut at: usize) -> (Vec<(String, String)>, usize) {
    let n = i32le(d, at).unwrap_or(0).clamp(0, 256) as usize;
    at += 4;
    let mut out = Vec::new();
    let s = |at: &mut usize| -> String {
        let len = i32le(d, *at).unwrap_or(0).clamp(0, 4096) as usize;
        let v =
            String::from_utf8_lossy(d.get(*at + 4..*at + 4 + len).unwrap_or_default()).into_owned();
        *at += 4 + len;
        v
    };
    for _ in 0..n {
        let k = s(&mut at);
        let v = s(&mut at);
        out.push((k, v));
    }
    (out, at)
}

pub(crate) fn read_vox(d: &[u8], limits: &Limits) -> Result<Mesh> {
    let mut models: Vec<VoxModel> = Vec::new();
    let mut palette: Option<Vec<[u8; 4]>> = None;
    // Scene graph: transforms (child, translation), groups (children), shapes (models).
    let mut trn: std::collections::HashMap<i32, (i32, [i32; 3])> = Default::default();
    let mut grp: std::collections::HashMap<i32, Vec<i32>> = Default::default();
    let mut shp: std::collections::HashMap<i32, Vec<i32>> = Default::default();
    let mut at = 20;
    let mut size = [0i32; 3];
    while at + 12 <= d.len() {
        let id = &d[at..at + 4];
        let n = i32le(d, at + 4).unwrap_or(0).max(0) as usize;
        let body = at + 12;
        match id {
            b"SIZE" => {
                size = [
                    i32le(d, body).unwrap_or(0),
                    i32le(d, body + 4).unwrap_or(0),
                    i32le(d, body + 8).unwrap_or(0),
                ];
            }
            b"XYZI" => {
                let k = i32le(d, body).unwrap_or(0).max(0) as usize;
                let v = d.get(body + 4..body + 4 + 4 * k).unwrap_or_default();
                models.push(VoxModel {
                    size,
                    voxels: v
                        .chunks_exact(4)
                        .map(|c| [c[0], c[1], c[2], c[3]])
                        .collect(),
                });
            }
            b"RGBA" => {
                let p = d.get(body..body + 1024).unwrap_or_default();
                palette = Some(
                    p.chunks_exact(4)
                        .map(|c| [c[0], c[1], c[2], c[3]])
                        .collect(),
                );
            }
            b"nTRN" => {
                let node = i32le(d, body).unwrap_or(-1);
                let (_, mut p) = vox_dict(d, body + 4);
                let child = i32le(d, p).unwrap_or(-1);
                p += 4 + 4 + 4; // child, reserved, layer
                let frames = i32le(d, p).unwrap_or(0);
                p += 4;
                let mut t = [0; 3];
                if frames > 0 {
                    let (f, _) = vox_dict(d, p);
                    if let Some((_, v)) = f.iter().find(|(k, _)| k == "_t") {
                        let n: Vec<i32> = v
                            .split_whitespace()
                            .filter_map(|x| x.parse().ok())
                            .collect();
                        if n.len() == 3 {
                            t = [n[0], n[1], n[2]];
                        }
                    }
                }
                trn.insert(node, (child, t));
            }
            b"nGRP" => {
                let node = i32le(d, body).unwrap_or(-1);
                let (_, p) = vox_dict(d, body + 4);
                let k = i32le(d, p).unwrap_or(0).clamp(0, 1 << 16) as usize;
                grp.insert(
                    node,
                    (0..k).filter_map(|i| i32le(d, p + 4 + 4 * i)).collect(),
                );
            }
            b"nSHP" => {
                let node = i32le(d, body).unwrap_or(-1);
                let (_, p) = vox_dict(d, body + 4);
                let k = i32le(d, p).unwrap_or(0).clamp(0, 1 << 16) as usize;
                let mut ms = Vec::new();
                let mut q = p + 4;
                for _ in 0..k {
                    ms.push(i32le(d, q).unwrap_or(-1));
                    let (_, nq) = vox_dict(d, q + 4);
                    q = nq;
                }
                shp.insert(node, ms);
            }
            _ => {}
        }
        // MAIN holds the rest as its children.
        at = if id == b"MAIN" {
            body + n
        } else {
            body + n + i32le(d, at + 8).unwrap_or(0).max(0) as usize
        };
    }
    if models.is_empty() {
        return bad("a VOX file without voxels");
    }
    let colour = |i: u8| -> [u8; 4] {
        match &palette {
            Some(p) => p
                .get(usize::from(i).wrapping_sub(1))
                .copied()
                .unwrap_or([255; 4]),
            None => {
                let v = DEFAULT_PALETTE[usize::from(i)];
                [
                    (v & 255) as u8,
                    (v >> 8 & 255) as u8,
                    (v >> 16 & 255) as u8,
                    255,
                ]
            }
        }
    };
    // Each model's offset: the scene's translations down to it (centred as MagicaVoxel
    // places models), else side by side.
    let mut offsets: Vec<Option<[i32; 3]>> = vec![None; models.len()];
    let mut stack = vec![(0i32, [0i32; 3], 0usize)];
    while let Some((node, t, depth)) = stack.pop() {
        if depth > 64 {
            continue;
        }
        if let Some(&(child, tt)) = trn.get(&node) {
            stack.push((child, [t[0] + tt[0], t[1] + tt[1], t[2] + tt[2]], depth + 1));
        } else if let Some(kids) = grp.get(&node) {
            for &k in kids {
                stack.push((k, t, depth + 1));
            }
        } else if let Some(ms) = shp.get(&node) {
            for &m in ms {
                if let Some(o) = offsets.get_mut(m.max(0) as usize)
                    && let Some(md) = models.get(m.max(0) as usize)
                {
                    *o = Some([
                        t[0] - md.size[0] / 2,
                        t[1] - md.size[1] / 2,
                        t[2] - md.size[2] / 2,
                    ]);
                }
            }
        }
    }
    let mut b = Builder::new(limits);
    let mut next_x = 0;
    for (mi, m) in models.iter().enumerate() {
        let off = offsets[mi].unwrap_or_else(|| {
            let o = [next_x, 0, 0];
            next_x += m.size[0] + 2;
            o
        });
        let mut filled = std::collections::HashSet::with_capacity(m.voxels.len());
        for v in &m.voxels {
            filled.insert((v[0], v[1], v[2]));
        }
        // The six faces: normal, then four corners (counter-clockwise from outside).
        const FACES: [([i32; 3], [[f32; 3]; 4]); 6] = [
            (
                [1, 0, 0],
                [
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [1.0, 1.0, 1.0],
                    [1.0, 0.0, 1.0],
                ],
            ),
            (
                [-1, 0, 0],
                [
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0],
                    [0.0, 1.0, 1.0],
                    [0.0, 1.0, 0.0],
                ],
            ),
            (
                [0, 1, 0],
                [
                    [0.0, 1.0, 0.0],
                    [0.0, 1.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [1.0, 1.0, 0.0],
                ],
            ),
            (
                [0, -1, 0],
                [
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 0.0, 1.0],
                    [0.0, 0.0, 1.0],
                ],
            ),
            (
                [0, 0, 1],
                [
                    [0.0, 0.0, 1.0],
                    [1.0, 0.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [0.0, 1.0, 1.0],
                ],
            ),
            (
                [0, 0, -1],
                [
                    [0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [1.0, 0.0, 0.0],
                ],
            ),
        ];
        for v in &m.voxels {
            let c = colour(v[3]);
            for (n, corners) in FACES {
                let nb = (
                    i32::from(v[0]) + n[0],
                    i32::from(v[1]) + n[1],
                    i32::from(v[2]) + n[2],
                );
                if (0..256).contains(&nb.0)
                    && (0..256).contains(&nb.1)
                    && (0..256).contains(&nb.2)
                    && filled.contains(&(nb.0 as u8, nb.1 as u8, nb.2 as u8))
                {
                    continue;
                }
                let nf = [n[0] as f32, n[1] as f32, n[2] as f32];
                let mut idx = [0u32; 4];
                for (k, q) in corners.iter().enumerate() {
                    let p = [
                        (i32::from(v[0]) + off[0]) as f32 + q[0],
                        (i32::from(v[1]) + off[1]) as f32 + q[1],
                        (i32::from(v[2]) + off[2]) as f32 + q[2],
                    ];
                    idx[k] = b.vertex(p, Some(nf), Some(c), None)?;
                }
                b.triangle(idx[0], idx[1], idx[2])?;
                b.triangle(idx[0], idx[2], idx[3])?;
            }
        }
    }
    b.up = Up::Z;
    let mut mesh = b.finish()?;
    mesh.flat = true;
    let voxels: usize = models.iter().map(|m| m.voxels.len()).sum();
    mesh.notes
        .push(format!("{} models, {voxels} voxels", models.len()));
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_voxels_share_a_face() {
        let mut d = b"VOX \x96\x00\x00\x00MAIN\x00\x00\x00\x00".to_vec();
        let mut kids = Vec::new();
        kids.extend_from_slice(b"SIZE");
        kids.extend_from_slice(&12i32.to_le_bytes());
        kids.extend_from_slice(&0i32.to_le_bytes());
        for v in [2i32, 1, 1] {
            kids.extend_from_slice(&v.to_le_bytes());
        }
        kids.extend_from_slice(b"XYZI");
        kids.extend_from_slice(&12i32.to_le_bytes());
        kids.extend_from_slice(&0i32.to_le_bytes());
        kids.extend_from_slice(&2i32.to_le_bytes());
        kids.extend_from_slice(&[0, 0, 0, 1, 1, 0, 0, 2]);
        d.extend_from_slice(&(kids.len() as i32).to_le_bytes());
        d.extend_from_slice(&kids);
        d[12..16].copy_from_slice(&0i32.to_le_bytes());
        d[16..20].copy_from_slice(&(kids.len() as i32).to_le_bytes());
        assert!(is_vox(&d));
        let m = read_vox(&d[..], &Limits::default()).unwrap();
        // Ten outer faces of two triangles.
        assert_eq!(m.indices.len() / 3, 20);
    }
}
