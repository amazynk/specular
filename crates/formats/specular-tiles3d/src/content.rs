//! Tile contents: Batched 3D Model (`b3dm`), Instanced 3D Model (`i3dm`), Point Cloud
//! (`pnts`), Composite (`cmpt`) and glTF (`glb`, `gltf`), placed in the drawing frame.
//!
//! Every placement is composed in double precision and applied before positions become
//! `f32`: glTF node transforms, glTF's y-up to z-up turn, `RTC_CENTER`, per-instance
//! transforms, the tile's transform and the tileset-to-drawing frame, in that order.

use serde_json::Value;
use specular_mesh::{Limits, Mesh, Resolver};

use crate::fetch::{self, Fetch};
use crate::math::{self, M4, V3};
use crate::tileset::UpAxis;
use crate::{Error, MAX_CONTENT, Result};

/// Points of one content, in the drawing frame.
#[derive(Clone, Debug, Default)]
pub struct Points {
    pub positions: Vec<[f32; 3]>,
    /// sRGB RGBA, one per position.
    pub colors: Option<Vec<[u8; 4]>>,
}

/// What a content holds, in the drawing frame (z up).
#[derive(Clone, Debug, Default)]
pub struct Content {
    pub mesh: Option<Mesh>,
    pub points: Option<Points>,
    pub notes: Vec<String>,
    /// The kinds read ("b3dm", "pnts", …) for the info panel.
    pub kinds: Vec<&'static str>,
}

/// How a content is placed.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    /// Tile frame → drawing frame (the tileset-to-drawing frame after the tile's transform).
    pub to_local: M4,
    pub up: UpAxis,
}

/// Side files of a glTF (buffers, images) next to its content.
struct Side<'a> {
    fetch: &'a dyn Fetch,
    base: &'a str,
}

impl Resolver for Side<'_> {
    fn read(&self, name: &str, max: usize) -> std::result::Result<Vec<u8>, String> {
        let key = fetch::resolve(self.base, name).ok_or("outside the tileset")?;
        fetch::load(self.fetch, &key, max)
    }
}

/// Reads the content at `key` (its bytes `bytes`).
pub fn read(
    bytes: &[u8],
    key: &str,
    place: &Place,
    fetch: &dyn Fetch,
    limits: &Limits,
) -> Result<Content> {
    let mut out = Content::default();
    read_into(bytes, key, place, fetch, limits, &mut out, 0)?;
    if out.mesh.is_none() && out.points.is_none() {
        return Err(Error::Empty);
    }
    Ok(out)
}

fn read_into(
    bytes: &[u8],
    key: &str,
    place: &Place,
    fetch: &dyn Fetch,
    limits: &Limits,
    out: &mut Content,
    depth: u32,
) -> Result<()> {
    let magic = bytes.get(..4).unwrap_or(&[]);
    match magic {
        b"b3dm" => b3dm(bytes, key, place, fetch, limits, out),
        b"i3dm" => i3dm(bytes, key, place, fetch, limits, out),
        b"pnts" => pnts(bytes, place, out),
        b"cmpt" => cmpt(bytes, key, place, fetch, limits, out, depth),
        b"glTF" => {
            out.kinds.push("glTF");
            gltf(bytes, true, key, place, &math::IDENTITY, fetch, limits, out)
        }
        b"geom" | b"vctr" => Err(Error::Unsupported(
            "vector tile content (geom, vctr) was never part of the 3D Tiles standard".into(),
        )),
        _ if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{')
            && !bytes.windows(7).any(|w| w == br#""asset""#) =>
        {
            Err(Error::Unsupported(
                "GeoJSON tile content (a CesiumJS extension, not part of 3D Tiles)".into(),
            ))
        }
        _ if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') => {
            out.kinds.push("glTF");
            gltf(
                bytes,
                false,
                key,
                place,
                &math::IDENTITY,
                fetch,
                limits,
                out,
            )
        }
        _ => Err(Error::Unsupported("tile content of an unknown kind".into())),
    }
}

/// The header's u32 at `o`.
fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// A feature table (or batch table): its JSON and binary body.
struct Table<'a> {
    json: Value,
    bin: &'a [u8],
}

impl Table<'_> {
    fn parse<'b>(json: &[u8], bin: &'b [u8]) -> Result<Table<'b>> {
        let text = std::str::from_utf8(json)
            .map_err(|_| Error::Malformed("feature table is not UTF-8".into()))?
            .trim_end_matches([' ', '\0']);
        let json = if text.trim().is_empty() {
            Value::Object(Default::default())
        } else {
            serde_json::from_str(text)
                .map_err(|e| Error::Malformed(format!("feature table JSON: {e}")))?
        };
        Ok(Table { json, bin })
    }

    fn uint(&self, k: &str) -> Option<u64> {
        let v = self.json.get(k)?;
        if let Some(n) = v.as_u64() {
            return Some(n);
        }
        if let Some(a) = v.as_array() {
            return a.first()?.as_u64();
        }
        let off = v.get("byteOffset")?.as_u64()? as usize;
        u32_at(self.bin, off).map(u64::from)
    }

    /// A global `n`-vector of floats (JSON array or a binary reference).
    fn vec(&self, k: &str, n: usize) -> Option<Vec<f64>> {
        let v = self.json.get(k)?;
        if let Some(a) = v.as_array() {
            let out: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
            return (out.len() == n).then_some(out);
        }
        let off = v.get("byteOffset")?.as_u64()? as usize;
        let ct = v
            .get("componentType")
            .and_then(Value::as_str)
            .unwrap_or("FLOAT");
        let (size, f): (usize, fn(&[u8]) -> f64) = match ct {
            "DOUBLE" => (8, |s| f64::from_le_bytes(s.try_into().unwrap_or([0; 8]))),
            _ => (4, |s| {
                f64::from(f32::from_le_bytes(s.try_into().unwrap_or([0; 4])))
            }),
        };
        (0..n)
            .map(|i| self.bin.get(off + i * size..off + (i + 1) * size).map(f))
            .collect()
    }

    /// A per-item property: `count` items of `n` components of `default` type, as f64.
    fn items(&self, k: &str, count: usize, n: usize, default: &str) -> Result<Option<Vec<f64>>> {
        let Some(v) = self.json.get(k) else {
            return Ok(None);
        };
        let off = v
            .get("byteOffset")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::Malformed(format!("{k} without a byte offset")))?
            as usize;
        let ct = v
            .get("componentType")
            .and_then(Value::as_str)
            .unwrap_or(default);
        let size = match ct {
            "BYTE" | "UNSIGNED_BYTE" => 1,
            "SHORT" | "UNSIGNED_SHORT" => 2,
            "INT" | "UNSIGNED_INT" | "FLOAT" => 4,
            "DOUBLE" => 8,
            t => return Err(Error::Unsupported(format!("{k} stored as {t}"))),
        };
        let total = count
            .checked_mul(n)
            .and_then(|c| c.checked_mul(size))
            .ok_or_else(|| Error::Malformed(format!("{k} is too long")))?;
        let data = self
            .bin
            .get(off..off.saturating_add(total))
            .ok_or_else(|| Error::Malformed(format!("{k} runs past the feature table")))?;
        let out = data
            .chunks_exact(size)
            .map(|s| match ct {
                "BYTE" => f64::from(s[0] as i8),
                "UNSIGNED_BYTE" => f64::from(s[0]),
                "SHORT" => f64::from(i16::from_le_bytes([s[0], s[1]])),
                "UNSIGNED_SHORT" => f64::from(u16::from_le_bytes([s[0], s[1]])),
                "INT" => f64::from(i32::from_le_bytes([s[0], s[1], s[2], s[3]])),
                "UNSIGNED_INT" => f64::from(u32::from_le_bytes([s[0], s[1], s[2], s[3]])),
                "FLOAT" => f64::from(f32::from_le_bytes([s[0], s[1], s[2], s[3]])),
                _ => f64::from_le_bytes(s.try_into().unwrap_or([0; 8])),
            })
            .collect();
        Ok(Some(out))
    }
}

/// The parts of a b3dm/i3dm/pnts: feature table, batch table and body.
struct Parts<'a> {
    ft: Table<'a>,
    body: &'a [u8],
}

/// Splits a tile with a `header` byte header (28 for b3dm and pnts, 32 for i3dm), reading
/// b3dm's two legacy layouts too.
fn split(bytes: &[u8], header: usize, legacy: bool) -> Result<Parts<'_>> {
    let total = (u32_at(bytes, 8).unwrap_or(0) as usize).min(bytes.len());
    let v = |o: usize| u32_at(bytes, o).unwrap_or(0) as usize;
    let (mut ftj, mut ftb, mut btj, mut btb) = (v(12), v(16), v(20), v(24));
    let mut start = header;
    // Early b3dm wrote [batchLength, batchTableByteLength] or [batchTableJsonByteLength,
    // batchTableBinaryByteLength, batchLength] in place of the four table lengths; the GLB
    // magic then lands where a length would be (a huge number).
    const GLTF_AS_LENGTH: usize = 570_425_344;
    if legacy && btj >= GLTF_AS_LENGTH {
        start = header - 8;
        (btj, btb, ftj, ftb) = (ftb, 0, 0, 0);
    } else if legacy && btb >= GLTF_AS_LENGTH {
        start = header - 4;
        (btj, btb, ftj, ftb) = (ftj, ftb, 0, 0);
    }
    let ft_end = start + ftj;
    let ftb_end = ft_end + ftb;
    let bt_end = ftb_end + btj + btb;
    if bt_end > total {
        return Err(Error::Malformed(
            "tables run past the end of the tile".into(),
        ));
    }
    let ft = Table::parse(&bytes[start..ft_end], &bytes[ft_end..ftb_end])?;
    Ok(Parts {
        ft,
        body: &bytes[bt_end..total],
    })
}

/// `RTC_CENTER` as a translation (identity without one).
fn rtc(ft: &Table) -> M4 {
    match ft.vec("RTC_CENTER", 3) {
        Some(c) => math::translation([c[0], c[1], c[2]]),
        None => math::IDENTITY,
    }
}

fn b3dm(
    bytes: &[u8],
    key: &str,
    place: &Place,
    fetch: &dyn Fetch,
    limits: &Limits,
    out: &mut Content,
) -> Result<()> {
    out.kinds.push("b3dm");
    let p = split(bytes, 28, true)?;
    let pre = rtc(&p.ft);
    gltf(p.body, true, key, place, &pre, fetch, limits, out)
}

/// A glTF under `pre` (in the tile frame, after the up-axis turn), into `out`.
#[allow(clippy::too_many_arguments)]
fn gltf(
    bytes: &[u8],
    glb: bool,
    key: &str,
    place: &Place,
    pre: &M4,
    fetch: &dyn Fetch,
    limits: &Limits,
    out: &mut Content,
) -> Result<()> {
    if glb && u32_at(bytes, 4) == Some(1) {
        return Err(Error::Unsupported("glTF 1.0 content".into()));
    }
    let json_part = &bytes[..bytes.len().min(1 << 16)];
    if json_part
        .windows(22)
        .any(|w| w == b"KHR_gaussian_splatting")
    {
        return Err(Error::Unsupported(
            "Gaussian splats in glTF (KHR_gaussian_splatting)".into(),
        ));
    }
    let mut chain = math::mul(&place.to_local, pre);
    if let Some(c) = cesium_rtc(bytes, glb) {
        chain = math::mul(&chain, &math::translation(c));
    }
    chain = math::mul(&chain, &place.up.to_z_up());
    let side = Side { fetch, base: key };
    let scene = specular_mesh::read_gltf_scene(bytes, glb, math::affine(&chain), &side, limits)
        .map_err(|e| match e {
            specular_mesh::Error::Unsupported(m) => Error::Unsupported(m),
            specular_mesh::Error::TooLarge(m) => Error::TooLarge(m),
            specular_mesh::Error::Empty => Error::Empty,
            specular_mesh::Error::Malformed(m) => Error::Malformed(m),
        })?;
    if let Some(m) = scene.mesh {
        out.notes.extend(m.notes.iter().cloned());
        add_mesh(out, m);
    }
    if !scene.points.positions.is_empty() {
        add_points(
            out,
            Points {
                positions: scene.points.positions,
                colors: scene.points.colors,
            },
        );
    }
    Ok(())
}

/// The `CESIUM_RTC` centre older glTF content carries.
fn cesium_rtc(bytes: &[u8], glb: bool) -> Option<V3> {
    let json = if glb {
        let len = u32_at(bytes, 12)? as usize;
        bytes.get(20..20 + len)?
    } else {
        bytes
    };
    let needle = b"CESIUM_RTC";
    if !json.windows(needle.len()).any(|w| w == needle) {
        return None;
    }
    let doc: Value = serde_json::from_slice(json).ok()?;
    let c = doc.pointer("/extensions/CESIUM_RTC/center")?.as_array()?;
    let v: Vec<f64> = c.iter().filter_map(Value::as_f64).collect();
    (v.len() == 3).then(|| [v[0], v[1], v[2]])
}

pub(crate) fn add_mesh(out: &mut Content, m: Mesh) {
    out.mesh = Some(match out.mesh.take() {
        None => m,
        Some(a) => crate::merge::meshes(vec![a, m]),
    });
}

fn add_points(out: &mut Content, p: Points) {
    out.points = Some(match out.points.take() {
        None => p,
        Some(a) => crate::merge::points(vec![a, p]),
    });
}

fn pnts(bytes: &[u8], place: &Place, out: &mut Content) -> Result<()> {
    out.kinds.push("pnts");
    let p = split(bytes, 28, false)?;
    let ft = &p.ft;
    if ft
        .json
        .pointer("/extensions/3DTILES_draco_point_compression")
        .is_some()
    {
        return Err(Error::Unsupported(
            "Draco-compressed points (3DTILES_draco_point_compression)".into(),
        ));
    }
    let n = ft
        .uint("POINTS_LENGTH")
        .ok_or_else(|| Error::Malformed("a point cloud without POINTS_LENGTH".into()))?
        as usize;
    if n > MAX_CONTENT {
        return Err(Error::TooLarge(format!("{n} points in one tile")));
    }
    let xf = math::mul(&place.to_local, &rtc(ft));
    let pos: Vec<V3> = if let Some(v) = ft.items("POSITION", n, 3, "FLOAT")? {
        v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
    } else if let Some(q) = ft.items("POSITION_QUANTIZED", n, 3, "UNSIGNED_SHORT")? {
        let off = ft
            .vec("QUANTIZED_VOLUME_OFFSET", 3)
            .ok_or_else(|| Error::Malformed("quantized points without an offset".into()))?;
        let sc = ft
            .vec("QUANTIZED_VOLUME_SCALE", 3)
            .ok_or_else(|| Error::Malformed("quantized points without a scale".into()))?;
        q.chunks_exact(3)
            .map(|c| {
                [
                    off[0] + c[0] / 65535.0 * sc[0],
                    off[1] + c[1] / 65535.0 * sc[1],
                    off[2] + c[2] / 65535.0 * sc[2],
                ]
            })
            .collect()
    } else {
        return Err(Error::Malformed("a point cloud without positions".into()));
    };
    let positions: Vec<[f32; 3]> = pos
        .iter()
        .map(|&q| math::point(&xf, q).map(|v| v as f32))
        .collect();
    let colors: Option<Vec<[u8; 4]>> = if let Some(c) = ft.items("RGBA", n, 4, "UNSIGNED_BYTE")? {
        Some(
            c.chunks_exact(4)
                .map(|c| [c[0] as u8, c[1] as u8, c[2] as u8, c[3] as u8])
                .collect(),
        )
    } else if let Some(c) = ft.items("RGB", n, 3, "UNSIGNED_BYTE")? {
        Some(
            c.chunks_exact(3)
                .map(|c| [c[0] as u8, c[1] as u8, c[2] as u8, 255])
                .collect(),
        )
    } else if let Some(c) = ft.items("RGB565", n, 1, "UNSIGNED_SHORT")? {
        Some(
            c.iter()
                .map(|&v| {
                    let v = v as u32;
                    let ch = |x: u32, bits: u32| {
                        ((x * 255 + ((1 << bits) - 1) / 2) / ((1 << bits) - 1)) as u8
                    };
                    [ch(v >> 11, 5), ch((v >> 5) & 63, 6), ch(v & 31, 5), 255]
                })
                .collect(),
        )
    } else {
        ft.vec("CONSTANT_RGBA", 4).map(|c| {
            let px = [c[0] as u8, c[1] as u8, c[2] as u8, c[3] as u8];
            vec![px; n]
        })
    };
    add_points(out, Points { positions, colors });
    Ok(())
}

fn oct_decode(x: f64, y: f64, range: f64) -> V3 {
    let (mut x, mut y) = (x / range * 2.0 - 1.0, y / range * 2.0 - 1.0);
    let z = 1.0 - x.abs() - y.abs();
    if z < 0.0 {
        let (ox, oy) = (x, y);
        x = (1.0 - oy.abs()) * ox.signum();
        y = (1.0 - ox.abs()) * oy.signum();
    }
    math::unit([x, y, z])
}

fn i3dm(
    bytes: &[u8],
    key: &str,
    place: &Place,
    fetch: &dyn Fetch,
    limits: &Limits,
    out: &mut Content,
) -> Result<()> {
    out.kinds.push("i3dm");
    let p = split(bytes, 32, false)?;
    let ft = &p.ft;
    let n = ft
        .uint("INSTANCES_LENGTH")
        .ok_or_else(|| Error::Malformed("instances without INSTANCES_LENGTH".into()))?
        as usize;
    if n > MAX_CONTENT {
        return Err(Error::TooLarge(format!("{n} instances in one tile")));
    }
    // The model, in its own z-up frame.
    let embedded = u32_at(bytes, 28).unwrap_or(1) == 1;
    let model_bytes;
    let (model_src, model_key): (&[u8], String) = if embedded {
        (p.body, key.to_owned())
    } else {
        let uri = std::str::from_utf8(p.body)
            .map_err(|_| Error::Malformed("the model's address is not text".into()))?
            .trim_end_matches(['\0', ' ']);
        let k = fetch::resolve(key, uri)
            .ok_or_else(|| Error::Missing(format!("{uri}: outside the tileset")))?;
        model_bytes = fetch::load(fetch, &k, limits.max_side_file).map_err(Error::Missing)?;
        (&model_bytes, k)
    };
    let glb = model_src.starts_with(b"glTF");
    let side = Side {
        fetch,
        base: &model_key,
    };
    let axis = place.up.to_z_up();
    let scene = specular_mesh::read_gltf_scene(model_src, glb, math::affine(&axis), &side, limits)
        .map_err(|e| Error::Malformed(e.to_string()))?;
    let Some(model) = scene.mesh else {
        return Err(Error::Empty);
    };
    out.notes.extend(model.notes.iter().cloned());
    let pos: Vec<V3> = if let Some(v) = ft.items("POSITION", n, 3, "FLOAT")? {
        v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
    } else if let Some(q) = ft.items("POSITION_QUANTIZED", n, 3, "UNSIGNED_SHORT")? {
        let off = ft
            .vec("QUANTIZED_VOLUME_OFFSET", 3)
            .ok_or_else(|| Error::Malformed("quantized instances without an offset".into()))?;
        let sc = ft
            .vec("QUANTIZED_VOLUME_SCALE", 3)
            .ok_or_else(|| Error::Malformed("quantized instances without a scale".into()))?;
        q.chunks_exact(3)
            .map(|c| {
                [
                    off[0] + c[0] / 65535.0 * sc[0],
                    off[1] + c[1] / 65535.0 * sc[1],
                    off[2] + c[2] / 65535.0 * sc[2],
                ]
            })
            .collect()
    } else {
        return Err(Error::Malformed("instances without positions".into()));
    };
    let ups = ft.items("NORMAL_UP", n, 3, "FLOAT")?;
    let rights = ft.items("NORMAL_RIGHT", n, 3, "FLOAT")?;
    let ups_oct = ft.items("NORMAL_UP_OCT32P", n, 2, "UNSIGNED_SHORT")?;
    let rights_oct = ft.items("NORMAL_RIGHT_OCT32P", n, 2, "UNSIGNED_SHORT")?;
    let scales = ft.items("SCALE", n, 1, "FLOAT")?;
    let scales3 = ft.items("SCALE_NON_UNIFORM", n, 3, "FLOAT")?;
    let enu = ft.json.get("EAST_NORTH_UP").and_then(Value::as_bool) == Some(true);
    let rtc_m = rtc(ft);
    let rtc_c = ft
        .vec("RTC_CENTER", 3)
        .map_or([0.0; 3], |c| [c[0], c[1], c[2]]);
    let tris = model.triangle_count().saturating_mul(n);
    if tris > limits.max_triangles {
        return Err(Error::TooLarge(format!("{tris} triangles of instances")));
    }
    let base = math::mul(&place.to_local, &rtc_m);
    let mut mats = Vec::with_capacity(n);
    for (i, &p) in pos.iter().enumerate() {
        let (right, up) = if let (Some(u), Some(r)) = (&ups, &rights) {
            let s = |a: &Vec<f64>| [a[i * 3], a[i * 3 + 1], a[i * 3 + 2]];
            (s(r), s(u))
        } else if let (Some(u), Some(r)) = (&ups_oct, &rights_oct) {
            (
                oct_decode(r[i * 2], r[i * 2 + 1], 65535.0),
                oct_decode(u[i * 2], u[i * 2 + 1], 65535.0),
            )
        } else if enu {
            let f = math::enu_at(math::add(p, rtc_c));
            ([f[0][0], f[1][0], f[2][0]], [f[0][1], f[1][1], f[2][1]])
        } else {
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
        };
        let fwd = math::cross(right, up);
        let s = if let Some(s3) = &scales3 {
            [s3[i * 3], s3[i * 3 + 1], s3[i * 3 + 2]]
        } else if let Some(s1) = &scales {
            [s1[i]; 3]
        } else {
            [1.0; 3]
        };
        let m = [
            [right[0] * s[0], up[0] * s[1], fwd[0] * s[2], p[0]],
            [right[1] * s[0], up[1] * s[1], fwd[1] * s[2], p[1]],
            [right[2] * s[0], up[2] * s[1], fwd[2] * s[2], p[2]],
            [0.0, 0.0, 0.0, 1.0],
        ];
        mats.push(math::mul(&base, &m));
    }
    add_mesh(out, crate::merge::instances(&model, &mats));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmpt(
    bytes: &[u8],
    key: &str,
    place: &Place,
    fetch: &dyn Fetch,
    limits: &Limits,
    out: &mut Content,
    depth: u32,
) -> Result<()> {
    if depth > 8 {
        return Err(Error::Malformed("composite tiles nested too deeply".into()));
    }
    out.kinds.push("cmpt");
    let total = (u32_at(bytes, 8).unwrap_or(0) as usize).min(bytes.len());
    let count = u32_at(bytes, 12).unwrap_or(0) as usize;
    let mut off = 16;
    for _ in 0..count {
        let len = u32_at(bytes, off + 8).unwrap_or(0) as usize;
        let Some(inner) = off
            .checked_add(len)
            .filter(|&e| len >= 12 && e <= total)
            .map(|e| &bytes[off..e])
        else {
            return Err(Error::Malformed(
                "an inner tile runs past the composite".into(),
            ));
        };
        match read_into(inner, key, place, fetch, limits, out, depth + 1) {
            Ok(()) => {}
            Err(Error::TooLarge(m)) => return Err(Error::TooLarge(m)),
            Err(e) => out.notes.push(format!("an inner tile skipped: {e}")),
        }
        off += len;
    }
    Ok(())
}
