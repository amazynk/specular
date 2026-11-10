//! Wavefront OBJ with MTL materials: `v` (with optional `r g b`), `vt`, `vn`, `f` with
//! `v`, `v/vt`, `v//vn` and `v/vt/vn` corners (negative indices count back), polygons as
//! fans, `usemtl` and `mtllib`; MTL `newmtl`, `Kd`, `d`, `Tr` and `map_Kd`. Lines, points,
//! curves and the rest are skipped. Lines ending in a backslash continue on the next.

use std::collections::HashMap;

use crate::builder::{Builder, srgb_to_linear};
use crate::text::{f32_of, i64_of, lines, rest_after_keyword, string, tokens, trim};
use crate::{ImageData, Limits, Material, Mesh, Resolver, Result, Up, bad};

pub(crate) fn read(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    let joined = join_continuations(bytes);
    let bytes = joined.as_deref().unwrap_or(bytes);
    let mut b = Builder::new(limits);
    b.up = Up::Y;
    let mut v: Vec<[f32; 3]> = Vec::new();
    let mut vc: Vec<Option<[u8; 4]>> = Vec::new();
    let mut any_vc = false;
    let mut vt: Vec<[f32; 2]> = Vec::new();
    let mut vn: Vec<[f32; 3]> = Vec::new();
    // (v, vt, vn) → output vertex; vt and vn are -1 when absent.
    let mut corners: HashMap<(u32, i32, i32), u32> = HashMap::new();
    // Plain `f a b c` faces: v → output vertex, no hashing.
    let mut plain: Vec<u32> = Vec::new();
    let mut key: Vec<u32> = Vec::new();
    let mut libs: HashMap<String, MtlMaterial> = HashMap::new();
    let mut used: HashMap<String, u32> = HashMap::new();
    let mut textures: HashMap<String, Option<u32>> = HashMap::new();
    let mut any_vn = false;
    let mut smooth = false;
    let mut skipped_faces = 0usize;
    let mut other = false;
    let mut face: Vec<u32> = Vec::with_capacity(8);
    for line in lines(bytes) {
        let mut t = tokens(line);
        let Some(kw) = t.next() else { continue };
        match kw {
            b"v" => {
                let mut f = || t.next().and_then(f32_of);
                let p = [f(), f(), f()];
                let c = [f(), f(), f()];
                if v.len() >= limits.max_vertices {
                    return Err(crate::Error::TooLarge("too many vertices".into()));
                }
                v.push(p.map(|x| x.unwrap_or(f32::NAN)));
                let colour = match c {
                    [Some(r), Some(g), Some(bl)] => {
                        any_vc = true;
                        let k = if r > 1.0 || g > 1.0 || bl > 1.0 {
                            1.0 / 255.0
                        } else {
                            1.0
                        };
                        Some([r, g, bl].map(|x| ((x * k).clamp(0.0, 1.0) * 255.0 + 0.5) as u8))
                            .map(|[r, g, bl]| [r, g, bl, 255])
                    }
                    _ => None,
                };
                vc.push(colour);
            }
            b"vt" => {
                let mut f = || t.next().and_then(f32_of);
                let (u, w) = (f().unwrap_or(0.0), f().unwrap_or(0.0));
                if vt.len() >= limits.max_vertices {
                    return Err(crate::Error::TooLarge(
                        "too many texture coordinates".into(),
                    ));
                }
                // OBJ's v points up; the model keeps glTF's (down).
                vt.push([u, 1.0 - w]);
            }
            b"vn" => {
                let mut f = || t.next().and_then(f32_of);
                if vn.len() >= limits.max_vertices {
                    return Err(crate::Error::TooLarge("too many normals".into()));
                }
                vn.push([f().unwrap_or(0.0), f().unwrap_or(0.0), f().unwrap_or(0.0)]);
            }
            b"f" | b"fo" => {
                face.clear();
                let mut ok = true;
                for c in t {
                    let mut parts = c.split(|&x| x == b'/');
                    let idx = |p: Option<&[u8]>, n: usize| -> Option<Option<u32>> {
                        match p {
                            None | Some(b"") => Some(None),
                            Some(s) => resolve_index(i64_of(s)?, n).map(Some),
                        }
                    };
                    // A texture or normal index that points nowhere (exporters that write
                    // `f 1/1/1` without any `vt`) drops that attribute, not the face.
                    let (Some(Some(pi)), ti, ni) = (
                        idx(parts.next(), v.len()),
                        idx(parts.next(), vt.len()).flatten(),
                        idx(parts.next(), vn.len()).flatten(),
                    ) else {
                        ok = false;
                        break;
                    };
                    let out = if ti.is_none() && ni.is_none() {
                        if plain.len() < v.len() {
                            plain.resize(v.len(), u32::MAX);
                        }
                        let slot = &mut plain[pi as usize];
                        if *slot == u32::MAX {
                            *slot = b.vertex(v[pi as usize], None, vc[pi as usize], None)?;
                            key.push(pi);
                        }
                        *slot
                    } else {
                        let k = (pi, ti.map_or(-1, |x| x as i32), ni.map_or(-1, |x| x as i32));
                        match corners.get(&k) {
                            Some(&o) => o,
                            None => {
                                let n = ni.map(|i| vn[i as usize]);
                                any_vn |= n.is_some();
                                let o = b.vertex(
                                    v[pi as usize],
                                    n,
                                    vc[pi as usize],
                                    ti.map(|i| vt[i as usize]),
                                )?;
                                key.push(pi);
                                corners.insert(k, o);
                                o
                            }
                        }
                    };
                    face.push(out);
                }
                if !ok || face.len() < 3 {
                    skipped_faces += 1;
                    continue;
                }
                for i in 2..face.len() {
                    b.triangle(face[0], face[i - 1], face[i])?;
                }
            }
            b"usemtl" => {
                let name = string(rest_after_keyword(line));
                let m = match used.get(&name) {
                    Some(&m) => Some(m),
                    None => {
                        let mat = libs.get(&name);
                        let texture = match mat.and_then(|m| m.map_kd.clone()) {
                            Some(file) => *textures.entry(file.clone()).or_insert_with(|| {
                                let image = match resolver.read(&file, limits.max_side_file) {
                                    Ok(bytes) => ImageData::Bytes { bytes, mime: None },
                                    Err(reason) => ImageData::Missing { name: file, reason },
                                };
                                b.add_image(image)
                            }),
                            None => None,
                        };
                        let m = b.add_material(Material {
                            name: name.clone(),
                            base_color: mat.map_or([1.0; 4], |m| m.color),
                            texture,
                        });
                        used.insert(name, m);
                        Some(m)
                    }
                };
                b.material(m);
            }
            b"mtllib" => {
                let rest = rest_after_keyword(line);
                // Names may hold spaces; try the whole rest first, then each token.
                let mut names = vec![string(rest)];
                names.extend(tokens(rest).map(string));
                names.dedup();
                let mut failed = None;
                for name in &names {
                    match resolver.read(name, limits.max_side_file) {
                        Ok(bytes) => {
                            parse_mtl(&bytes, &mut libs);
                            failed = None;
                            break;
                        }
                        Err(reason) => failed = failed.or(Some(reason)),
                    }
                }
                if let Some(reason) = failed {
                    b.note(format!("material library {}: {reason}", names[0]));
                }
            }
            b"s" => {
                smooth = !matches!(t.next(), Some(b"off" | b"0") | None);
            }
            b"l" | b"p" | b"curv" | b"curv2" | b"surf" => other = true,
            b"o" if b.title.is_none() => {
                let n = rest_after_keyword(line);
                if !n.is_empty() {
                    b.title = Some(string(n));
                }
            }
            _ => {}
        }
    }
    if b.triangle_count() == 0 {
        return bad("no faces");
    }
    if !any_vc {
        b.colors = None;
    }
    if skipped_faces > 0 {
        b.note(format!("{skipped_faces} unreadable faces skipped"));
    }
    if other {
        b.note("points, lines and curves are not shown");
    }
    b.flat = !any_vn && !smooth;
    b.normal_key = Some(key);
    b.finish()
}

/// `bytes` with every line continuation (a backslash ending a line, as Rhino writes long
/// faces) turned into spaces; `None` when there is none.
fn join_continuations(bytes: &[u8]) -> Option<Vec<u8>> {
    let is_cont = |i: usize| match bytes.get(i + 1) {
        Some(b'\n') => Some(2),
        Some(b'\r') if bytes.get(i + 2) == Some(&b'\n') => Some(3),
        _ => None,
    };
    let first = bytes
        .iter()
        .enumerate()
        .position(|(i, &c)| c == b'\\' && is_cont(i).is_some())?;
    let mut out = bytes.to_vec();
    let mut i = first;
    while i < out.len() {
        if out[i] == b'\\'
            && let Some(n) = is_cont(i)
        {
            out[i..i + n].fill(b' ');
            i += n;
        } else {
            i += 1;
        }
    }
    Some(out)
}

/// A 1-based (or negative, counting back) OBJ index to a 0-based one, `None` out of range.
fn resolve_index(i: i64, n: usize) -> Option<u32> {
    let z = if i > 0 {
        i - 1
    } else if i < 0 {
        n as i64 + i
    } else {
        return None;
    };
    (0..n as i64).contains(&z).then_some(z as u32)
}

struct MtlMaterial {
    color: [f32; 4],
    map_kd: Option<String>,
}

fn parse_mtl(bytes: &[u8], out: &mut HashMap<String, MtlMaterial>) {
    let mut cur: Option<String> = None;
    for line in lines(bytes) {
        let mut t = tokens(line);
        let Some(kw) = t.next() else { continue };
        if kw == b"newmtl" {
            let name = string(rest_after_keyword(line));
            out.insert(
                name.clone(),
                MtlMaterial {
                    color: [1.0; 4],
                    map_kd: None,
                },
            );
            cur = Some(name);
            continue;
        }
        let Some(m) = cur.as_ref().and_then(|c| out.get_mut(c)) else {
            continue;
        };
        match kw {
            b"Kd" => {
                let mut f = || t.next().and_then(f32_of);
                if let (Some(r), g, bl) = (f(), f(), f()) {
                    // A single value is grey.
                    let (g, bl) = (g.unwrap_or(r), bl.unwrap_or(r));
                    m.color[0] = srgb_to_linear(r);
                    m.color[1] = srgb_to_linear(g);
                    m.color[2] = srgb_to_linear(bl);
                }
            }
            b"d" => {
                if let Some(a) = t.find(|x| *x != b"-halo").and_then(f32_of) {
                    m.color[3] = a.clamp(0.0, 1.0);
                }
            }
            b"Tr" => {
                if let Some(a) = t.next().and_then(f32_of) {
                    m.color[3] = (1.0 - a).clamp(0.0, 1.0);
                }
            }
            b"map_Kd" => {
                if let Some(file) = map_file(rest_after_keyword(line)) {
                    m.map_kd = Some(file);
                }
            }
            _ => {}
        }
    }
}

/// The file name of a `map_*` statement, after its options.
fn map_file(rest: &[u8]) -> Option<String> {
    let toks: Vec<&[u8]> = tokens(rest).collect();
    let mut i = 0;
    while i < toks.len() && toks[i].starts_with(b"-") && toks[i].len() > 1 {
        let args = match toks[i] {
            b"-o" | b"-s" | b"-t" => {
                // Up to three numbers.
                let mut k = 0;
                while k < 3 && toks.get(i + 1 + k).and_then(|t| f32_of(t)).is_some() {
                    k += 1;
                }
                k
            }
            b"-mm" => 2,
            _ => 1,
        };
        i += 1 + args;
    }
    if i >= toks.len() {
        return None;
    }
    // The rest, spaces kept: find where token i starts in `rest`.
    let start = toks[i].as_ptr() as usize - rest.as_ptr() as usize;
    let name = trim(&rest[start..]);
    (!name.is_empty()).then(|| string(name))
}
