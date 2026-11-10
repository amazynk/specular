//! LightWave objects: LWO2 (layers, points, `FACE` and sub-patch polygons, surface tags, UV
//! and colour vertex maps per point and per polygon, surfaces with their colour and image
//! texture through clips) and the older LWOB (points, polygons with surface numbers and
//! detail polygons, surface colours). LightWave is left-handed, y up, with clockwise faces:
//! the model is mirrored in z to a right-handed frame. Modo's LXOB is refused.

use std::collections::HashMap;

use crate::builder::{Affine, Builder};
use crate::poly::{Attr, Poly, emit};
use crate::{Error, ImageData, Limits, Material, Mesh, Resolver, Result, Up, bad};

/// UV maps kept per layer (each is as long as the layer's points); later names are ignored.
const MAX_UV_MAPS: usize = 16;

struct Cur<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Cur<'a> {
    fn left(&self) -> usize {
        self.b.len().saturating_sub(self.at)
    }

    fn u16(&mut self) -> Option<u16> {
        let s = self.b.get(self.at..self.at + 2)?;
        self.at += 2;
        Some(u16::from_be_bytes([s[0], s[1]]))
    }

    fn u32(&mut self) -> Option<u32> {
        let s = self.b.get(self.at..self.at + 4)?;
        self.at += 4;
        Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits)
    }

    fn id(&mut self) -> Option<[u8; 4]> {
        let s = self.b.get(self.at..self.at + 4)?;
        self.at += 4;
        Some([s[0], s[1], s[2], s[3]])
    }

    /// A variable-length index: two bytes, or four when the first is 0xFF.
    fn vx(&mut self) -> Option<u32> {
        if *self.b.get(self.at)? == 0xFF {
            Some(self.u32()? & 0x00FF_FFFF)
        } else {
            self.u16().map(u32::from)
        }
    }

    /// A null-terminated string padded to an even length.
    fn s0(&mut self) -> Option<String> {
        let rest = self.b.get(self.at..)?;
        let n = rest.iter().position(|&c| c == 0)?;
        let s = String::from_utf8_lossy(&rest[..n]).into_owned();
        self.at += (n + 1).next_multiple_of(2);
        Some(s)
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.at..self.at + n)?;
        self.at += n;
        Some(s)
    }
}

/// IFF chunks: (id, data), sizes 4 bytes (`big`) or 2 bytes (sub-chunks), padded to even.
fn chunks(b: &[u8], big: bool) -> Vec<([u8; 4], &[u8])> {
    let mut c = Cur { b, at: 0 };
    let mut out = Vec::new();
    while c.left() >= if big { 8 } else { 6 } {
        let Some(id) = c.id() else { break };
        let len = if big {
            c.u32().map(|v| v as usize)
        } else {
            c.u16().map(usize::from)
        };
        let Some(len) = len else { break };
        let Some(d) = c.take(len.min(c.left())) else {
            break;
        };
        out.push((id, d));
        if len % 2 == 1 {
            c.at += 1;
        }
    }
    out
}

#[derive(Default)]
struct Layer {
    points: Vec<[f32; 3]>,
    number: u16,
    parent: Option<u16>,
    pivot: [f32; 3],
    /// Corners per polygon, flattened.
    counts: Vec<u32>,
    indices: Vec<u32>,
    /// Surface tag of each polygon.
    surf: Vec<u16>,
    /// UV map per point, by map name.
    uv: HashMap<String, Vec<Option<[f32; 2]>>>,
    /// Per-polygon UVs: (map, polygon, point) → uv.
    uv_poly: HashMap<(String, u32, u32), [f32; 2]>,
    rgb: Option<Vec<Option<[f32; 4]>>>,
    subpatch: bool,
}

#[derive(Default, Clone)]
struct Surf {
    colour: [f32; 3],
    transparency: f32,
    image: Option<u32>,
    uv_map: Option<String>,
}

pub(crate) fn read(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    if bytes.len() < 12 || &bytes[..4] != b"FORM" {
        return bad("not a LightWave object");
    }
    let kind = &bytes[8..12];
    let size = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let body = &bytes[12..(8 + size).min(bytes.len()).max(12)];
    match kind {
        b"LWO2" | b"LWO3" => read_lwo2(body, resolver, limits),
        b"LWOB" | b"LWLO" => read_lwob(body, limits),
        b"LXOB" => Err(Error::Unsupported(
            "Modo LXOB objects (only LightWave LWO2 and LWOB are read)".into(),
        )),
        k => bad(format!(
            "not a LightWave object ({})",
            String::from_utf8_lossy(k)
        )),
    }
}

fn read_lwo2(body: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    let mut tags: Vec<String> = Vec::new();
    let mut layers: Vec<Layer> = Vec::new();
    let mut surfs: HashMap<String, Surf> = HashMap::new();
    let mut clips: HashMap<u32, String> = HashMap::new();
    let mut total = 0usize;
    for (id, d) in chunks(body, true) {
        let mut c = Cur { b: d, at: 0 };
        match &id {
            b"TAGS" => {
                while c.left() > 0 {
                    match c.s0() {
                        Some(s) => tags.push(s),
                        None => break,
                    }
                }
            }
            b"LAYR" => {
                let number = c.u16().unwrap_or(0);
                c.u16();
                let pivot = [c.f32(), c.f32(), c.f32()];
                c.s0();
                let parent = c.u16();
                layers.push(Layer {
                    number,
                    parent,
                    pivot: pivot.map(|v| v.unwrap_or(0.0)),
                    ..Layer::default()
                });
            }
            b"PNTS" => {
                if layers.is_empty() {
                    layers.push(Layer::default());
                }
                let l = layers.last_mut().expect("pushed");
                let n = d.len() / 12;
                total += n;
                if total > limits.max_vertices {
                    return Err(Error::TooLarge("LWO with too many points".into()));
                }
                for _ in 0..n {
                    let (Some(x), Some(y), Some(z)) = (c.f32(), c.f32(), c.f32()) else {
                        break;
                    };
                    l.points.push([x, y, z]);
                }
            }
            b"VMAP" | b"VMAD" => {
                let Some(l) = layers.last_mut() else { continue };
                let (Some(t), Some(dim), Some(name)) = (c.id(), c.u16(), c.s0()) else {
                    continue;
                };
                let dim = dim as usize;
                let per_poly = &id == b"VMAD";
                let np = l.points.len();
                while c.left() > 0 {
                    let Some(v) = c.vx() else { break };
                    let poly = if per_poly { c.vx() } else { None };
                    let vals: Option<Vec<f32>> = (0..dim).map(|_| c.f32()).collect();
                    let Some(vals) = vals else { break };
                    match &t {
                        b"TXUV" if dim >= 2 => {
                            let uv = [vals[0], 1.0 - vals[1]];
                            match poly {
                                Some(p) => {
                                    l.uv_poly.insert((name.clone(), p, v), uv);
                                }
                                // Each map is as long as the layer's points: a few bytes
                                // per map name must not cost that many times over.
                                None if l.uv.len() >= MAX_UV_MAPS && !l.uv.contains_key(&name) => {}
                                None => {
                                    let m =
                                        l.uv.entry(name.clone()).or_insert_with(|| vec![None; np]);
                                    if let Some(s) = m.get_mut(v as usize) {
                                        *s = Some(uv);
                                    }
                                }
                            }
                        }
                        b"RGB " | b"RGBA" if dim >= 3 && !per_poly => {
                            let m = l.rgb.get_or_insert_with(|| vec![None; np]);
                            if let Some(s) = m.get_mut(v as usize) {
                                *s = Some([
                                    vals[0],
                                    vals[1],
                                    vals[2],
                                    vals.get(3).copied().unwrap_or(1.0),
                                ]);
                            }
                        }
                        _ => {}
                    }
                }
            }
            b"POLS" => {
                let Some(l) = layers.last_mut() else { continue };
                let Some(t) = c.id() else { continue };
                let face = matches!(&t, b"FACE" | b"PTCH" | b"SUBD");
                if !face {
                    continue;
                }
                if &t != b"FACE" {
                    l.subpatch = true;
                }
                while c.left() > 0 {
                    let Some(n) = c.u16() else { break };
                    let n = (n & 0x03FF) as usize;
                    let idx: Option<Vec<u32>> = (0..n).map(|_| c.vx()).collect();
                    let Some(idx) = idx else { break };
                    l.counts.push(n as u32);
                    l.indices.extend(idx);
                }
                l.surf = vec![0; l.counts.len()];
            }
            b"PTAG" => {
                let Some(l) = layers.last_mut() else { continue };
                if c.id() != Some(*b"SURF") {
                    continue;
                }
                while c.left() > 0 {
                    let (Some(p), Some(tag)) = (c.vx(), c.u16()) else {
                        break;
                    };
                    if let Some(s) = l.surf.get_mut(p as usize) {
                        *s = tag;
                    }
                }
            }
            b"CLIP" => {
                let Some(index) = c.u32() else { continue };
                for (sid, sd) in chunks(d.get(4..).unwrap_or(&[]), false) {
                    if &sid == b"STIL" {
                        let mut sc = Cur { b: sd, at: 0 };
                        if let Some(f) = sc.s0() {
                            clips.insert(index, f);
                        }
                    }
                }
            }
            b"SURF" => {
                let (Some(name), Some(_source)) = (c.s0(), c.s0()) else {
                    continue;
                };
                let mut s = Surf {
                    colour: [0.8; 3],
                    ..Surf::default()
                };
                for (sid, sd) in chunks(d.get(c.at..).unwrap_or(&[]), false) {
                    let mut sc = Cur { b: sd, at: 0 };
                    match &sid {
                        b"COLR" => {
                            if let (Some(r), Some(g), Some(b)) = (sc.f32(), sc.f32(), sc.f32()) {
                                s.colour = [r, g, b];
                            }
                        }
                        b"TRAN" => s.transparency = sc.f32().unwrap_or(0.0),
                        b"BLOK" => {
                            // An image block on the colour channel.
                            let blocks = chunks(sd, false);
                            let Some((hid, hd)) = blocks.first() else {
                                continue;
                            };
                            if hid != b"IMAP" {
                                continue;
                            }
                            let mut hc = Cur { b: hd, at: 0 };
                            hc.s0();
                            let colour = chunks(hd.get(hc.at..).unwrap_or(&[]), false)
                                .iter()
                                .any(|(i, d)| i == b"CHAN" && d.get(..4) == Some(b"COLR"));
                            if !colour {
                                continue;
                            }
                            for (bid, bd) in &blocks[1..] {
                                let mut bc = Cur { b: bd, at: 0 };
                                match bid {
                                    b"IMAG" => s.image = bc.vx(),
                                    b"VMAP" => s.uv_map = bc.s0(),
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                surfs.insert(name, s);
            }
            _ => {}
        }
    }
    let mut b = Builder::new(limits);
    b.up = Up::Y;
    // Materials by tag index.
    let mut by_tag: HashMap<u16, (u32, Surf)> = HashMap::new();
    let mut images: HashMap<u32, Option<u32>> = HashMap::new();
    for (i, t) in tags.iter().enumerate() {
        let Some(s) = surfs.get(t) else { continue };
        let texture = match s.image {
            Some(clip) => *images.entry(clip).or_insert_with(|| {
                clips
                    .get(&clip)
                    .map(|f| read_image(f, resolver, &mut b))
                    .unwrap_or(None)
            }),
            None => None,
        };
        let m = b.add_material(Material {
            name: t.clone(),
            base_color: [
                s.colour[0],
                s.colour[1],
                s.colour[2],
                1.0 - s.transparency.clamp(0.0, 1.0),
            ],
            texture,
        });
        by_tag.insert(i as u16, (m, s.clone()));
    }
    // Left-handed to right-handed: z mirrored.
    for (li, l) in layers.iter().enumerate() {
        if l.counts.is_empty() {
            continue;
        }
        // As Layout places it: the layer hierarchy's root pivot at the origin.
        let mut root = li;
        for _ in 0..64 {
            match layers[root]
                .parent
                .and_then(|p| layers.iter().position(|o| o.number == p))
            {
                Some(p) if p != root => root = p,
                _ => break,
            }
        }
        let [px, py, pz] = layers[root].pivot.map(f64::from);
        let xf = Affine([
            [1.0, 0.0, 0.0, -px],
            [0.0, 1.0, 0.0, -py],
            [0.0, 0.0, -1.0, pz],
        ]);
        if l.subpatch {
            b.note_once("sub-patch surfaces are shown as their control cage");
        }
        let face_mat: Vec<Option<u32>> = l
            .surf
            .iter()
            .map(|t| by_tag.get(t).map(|(m, _)| *m))
            .collect();
        // One UV map: the textured surfaces' (or any).
        let map = by_tag
            .values()
            .find_map(|(_, s)| s.uv_map.clone().filter(|m| l.uv.contains_key(m)))
            .or_else(|| l.uv.keys().min().cloned());
        let mut uvs = None;
        if let Some(m) = &map {
            // Per corner: the polygon's own value where it has one, else the point's.
            let pt = l.uv.get(m);
            let mut out = Vec::with_capacity(l.indices.len());
            let mut at = 0usize;
            for (p, &n) in l.counts.iter().enumerate() {
                for k in 0..n as usize {
                    let v = l.indices.get(at + k).copied().unwrap_or(0);
                    let uv = l
                        .uv_poly
                        .get(&(m.clone(), p as u32, v))
                        .copied()
                        .or_else(|| pt.and_then(|t| t.get(v as usize).copied().flatten()))
                        .unwrap_or([0.0, 0.0]);
                    out.push(uv);
                }
                at += n as usize;
            }
            uvs = Some(out);
        }
        let colors: Option<Vec<[u8; 4]>> = l.rgb.as_ref().map(|c| {
            c.iter()
                .map(|v| {
                    let v = v.unwrap_or([1.0; 4]);
                    [
                        crate::builder::linear_to_srgb8(v[0]),
                        crate::builder::linear_to_srgb8(v[1]),
                        crate::builder::linear_to_srgb8(v[2]),
                        (v[3].clamp(0.0, 1.0) * 255.0).round() as u8,
                    ]
                })
                .collect()
        });
        let mut p = Poly::new(&l.points, &l.counts, &l.indices);
        p.face_materials = Some(&face_mat);
        p.clockwise = true;
        if let Some(u) = &uvs {
            p.uvs = Attr::Corner(u);
        }
        if let Some(c) = &colors {
            p.colors = Attr::Vertex(c);
        }
        emit(&mut b, &p, &xf)?;
    }
    b.finish()
}

fn read_image(name: &str, resolver: &dyn Resolver, b: &mut Builder) -> Option<u32> {
    let max = b.limits.max_side_file;
    // Clips often hold the author's absolute path: try it, then its file name beside the
    // object, then under `images/`.
    let base = name
        .rsplit(['/', '\\', ':'])
        .next()
        .unwrap_or(name)
        .to_owned();
    let mut reason = String::new();
    for t in [name.to_owned(), base.clone(), format!("images/{base}")] {
        match resolver.read(&t, max) {
            Ok(bytes) => return b.add_image(ImageData::Bytes { bytes, mime: None }),
            Err(e) => reason = e,
        }
    }
    b.add_image(ImageData::Missing {
        name: name.to_owned(),
        reason,
    })
}

fn read_lwob(body: &[u8], limits: &Limits) -> Result<Mesh> {
    let mut points: Vec<[f32; 3]> = Vec::new();
    let mut counts: Vec<u32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut surf: Vec<u16> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut colours: HashMap<String, [f32; 3]> = HashMap::new();
    for (id, d) in chunks(body, true) {
        let mut c = Cur { b: d, at: 0 };
        match &id {
            b"PNTS" => {
                let n = d.len() / 12;
                if points.len() + n > limits.max_vertices {
                    return Err(Error::TooLarge("LWO with too many points".into()));
                }
                for _ in 0..n {
                    let (Some(x), Some(y), Some(z)) = (c.f32(), c.f32(), c.f32()) else {
                        break;
                    };
                    points.push([x, y, z]);
                }
            }
            b"SRFS" => {
                while c.left() > 0 {
                    match c.s0() {
                        Some(s) => names.push(s),
                        None => break,
                    }
                }
            }
            b"POLS" => {
                // Polygons; a negative surface number is followed by detail polygons.
                let mut detail = 0usize;
                while c.left() > 0 {
                    let Some(n) = c.u16() else { break };
                    let idx: Option<Vec<u32>> = (0..n).map(|_| c.u16().map(u32::from)).collect();
                    let Some(idx) = idx else { break };
                    let Some(s) = c.u16() else { break };
                    let s = s as i16;
                    counts.push(u32::from(n));
                    indices.extend(idx);
                    surf.push(s.unsigned_abs().saturating_sub(1));
                    if detail > 0 {
                        detail -= 1;
                    } else if s < 0 {
                        detail = c.u16().unwrap_or(0) as usize;
                    }
                }
            }
            b"SURF" => {
                let Some(name) = c.s0() else { continue };
                for (sid, sd) in chunks(d.get(c.at..).unwrap_or(&[]), false) {
                    if &sid == b"COLR" && sd.len() >= 3 {
                        colours.insert(name.clone(), [0, 1, 2].map(|k| f32::from(sd[k]) / 255.0));
                    }
                }
            }
            _ => {}
        }
    }
    let mut b = Builder::new(limits);
    b.up = Up::Y;
    let mats: Vec<u32> = names
        .iter()
        .map(|n| {
            let c = colours.get(n).copied().unwrap_or([0.8; 3]);
            b.add_material(Material {
                name: n.clone(),
                base_color: [c[0], c[1], c[2], 1.0].map(crate::builder::srgb_to_linear),
                texture: None,
            })
        })
        .collect();
    let face_mat: Vec<Option<u32>> = surf
        .iter()
        .map(|&s| mats.get(s as usize).copied())
        .collect();
    let mut p = Poly::new(&points, &counts, &indices);
    p.face_materials = Some(&face_mat);
    p.clockwise = true;
    let mirror = Affine([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0, 0.0],
    ]);
    emit(&mut b, &p, &mirror)?;
    b.finish()
}
