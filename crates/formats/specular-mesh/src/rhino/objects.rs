//! The records of a 3DM archive this reader uses: meshes (`ON_Mesh`), the render meshes a
//! B-rep (`ON_Brep`) or an extrusion (`ON_Extrusion`) carries, block instances
//! (`ON_InstanceRef`) and their definitions (`ON_InstanceDefinition`), object attributes
//! (layer, colour, material, visibility), layers and materials, and the properties and
//! settings the info panel shows.

use super::archive::{Cur, Object, TCODE_ANONYMOUS_CHUNK, Uuid, read_object};
use crate::{Error, Limits, Result};

pub(crate) const MESH: Uuid = Uuid::parse("4ED7D4E4-E947-11d3-BFE5-0010830122F0");
pub(crate) const BREPS: [Uuid; 4] = [
    Uuid::parse("60B5DBC5-E660-11d3-BFE4-0010830122F0"),
    // TL_Brep, the old B-rep and the old trimmed surface: read as ON_Brep.
    Uuid::parse("F06FC243-A32A-4608-9DD8-A7D2C4CE2A36"),
    Uuid::parse("2D4CFEDB-3E2A-11d4-800E-0010830122F0"),
    Uuid::parse("0705FDEF-3E2A-11d4-800E-0010830122F0"),
];
pub(crate) const EXTRUSION: Uuid = Uuid::parse("36F53175-72B8-4d47-BF1F-B4E6FC24F4B9");
pub(crate) const SUBD: Uuid = Uuid::parse("F09BA4D9-455B-42C3-BA3B-E6CCACEF853B");
pub(crate) const INSTANCE_REF: Uuid = Uuid::parse("F9CFB638-B9D4-4340-87E3-C56E7865D96A");
const RENDER_MESH_ID: Uuid = Uuid::parse("F5E3BAA9-A7A2-49FD-B8A1-66EB274A5F91");
const FINE_MESH_ID: Uuid = Uuid::parse("A2DAF594-F44B-44A6-A44B-E856CD7C05E8");
const ANALYSIS_MESH_ID: Uuid = Uuid::parse("AC12F955-A29F-437B-A4C9-06C85B7FF57C");

/// A mesh as stored: corners per face (a triangle repeats its third corner), positions,
/// and optional per-vertex normals, texture coordinates and colours.
#[derive(Clone, Debug, Default)]
pub(crate) struct RMesh {
    pub v: Vec<[f32; 3]>,
    pub n: Option<Vec<[f32; 3]>>,
    pub t: Option<Vec<[f32; 2]>>,
    pub c: Option<Vec<[u8; 4]>>,
    pub f: Vec<[u32; 4]>,
}

impl RMesh {
    pub fn triangles(&self) -> usize {
        self.f
            .iter()
            .map(|f| if f[2] == f[3] { 1 } else { 2 })
            .sum()
    }
}

fn floats<const N: usize>(raw: &[u8]) -> Vec<[f32; N]> {
    raw.chunks_exact(4 * N)
        .map(|c| {
            std::array::from_fn(|i| {
                f32::from_le_bytes(c[4 * i..4 * i + 4].try_into().unwrap_or([0; 4]))
            })
        })
        .collect()
}

fn colors(raw: &[u8]) -> Vec<[u8; 4]> {
    raw.chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], 255])
        .collect()
}

/// An array written as a count and its elements (meshes of version 1.x).
fn plain_array<'a>(c: &mut Cur<'a>, each: usize) -> Result<(usize, &'a [u8])> {
    let n = c.count(each)?;
    Ok((n, c.bytes(n * each)?))
}

/// An `ON_Mesh` (major version 1: plain arrays; 3: compressed buffers).
pub(crate) fn read_mesh(c: &mut Cur, limits: &Limits) -> Result<RMesh> {
    let (major, _minor) = c.version()?;
    if major != 1 && major != 3 {
        return Err(Error::Unsupported(format!("mesh format version {major}")));
    }
    let vcount = usize::try_from(c.i32()?).map_err(|_| Error::Malformed("vertex count".into()))?;
    let fcount = usize::try_from(c.i32()?).map_err(|_| Error::Malformed("face count".into()))?;
    if vcount > limits.max_vertices {
        return Err(Error::TooLarge(format!(
            "more than {} vertices",
            limits.max_vertices
        )));
    }
    if fcount > limits.max_triangles {
        return Err(Error::TooLarge(format!(
            "more than {} triangles",
            limits.max_triangles
        )));
    }
    // Packed texture and surface domains (4 intervals), surface scale (2 doubles), the
    // vertex, normal and texture boxes (6 + 6 + 4 floats) and the closed flag.
    c.skip(64 + 16 + 24 + 24 + 16 + 4)?;
    // Optional meshing parameters, then four optional curvature statistics, each in a chunk.
    for _ in 0..5 {
        if c.u8()? != 0 {
            c.skip_chunk()?;
        }
    }
    let isize = c.i32()?;
    let each = match isize {
        1 => 1usize,
        2 => 2,
        4 => 4,
        _ if fcount == 0 => 1,
        _ => return Err(Error::Malformed(format!("face index size {isize}"))),
    };
    let raw = c.bytes(
        fcount
            .checked_mul(4 * each)
            .ok_or_else(|| Error::Malformed("faces".into()))?,
    )?;
    let f: Vec<[u32; 4]> = raw
        .chunks_exact(4 * each)
        .map(|q| {
            std::array::from_fn(|k| {
                let s = &q[k * each..(k + 1) * each];
                match each {
                    1 => u32::from(s[0]),
                    2 => u32::from(u16::from_le_bytes([s[0], s[1]])),
                    _ => u32::from_le_bytes([s[0], s[1], s[2], s[3]]),
                }
            })
        })
        .collect();
    let mut m = RMesh {
        f,
        ..RMesh::default()
    };
    if major == 1 {
        let (n, raw) = plain_array(c, 12)?;
        m.v = floats::<3>(raw);
        let _ = n;
        let (n, raw) = plain_array(c, 12)?;
        if n > 0 {
            m.n = Some(floats::<3>(raw));
        }
        let (n, raw) = plain_array(c, 8)?;
        if n > 0 {
            m.t = Some(floats::<2>(raw));
        }
        plain_array(c, 16)?;
        let (n, raw) = plain_array(c, 4)?;
        if n > 0 {
            m.c = Some(colors(raw));
        }
    } else if vcount > 0 {
        m.v = c
            .compressed(vcount * 12)?
            .map(|r| floats::<3>(&r))
            .unwrap_or_default();
        m.n = c.compressed(vcount * 12)?.map(|r| floats::<3>(&r));
        m.t = c.compressed(vcount * 8)?.map(|r| floats::<2>(&r));
        c.compressed(vcount * 16)?;
        m.c = c.compressed(vcount * 4)?.map(|r| colors(&r));
    }
    let nv = m.v.len();
    if m.n.as_ref().is_some_and(|n| n.len() != nv) {
        m.n = None;
    }
    if m.t.as_ref().is_some_and(|t| t.len() != nv) {
        m.t = None;
    }
    if m.c.as_ref().is_some_and(|x| x.len() != nv) {
        m.c = None;
    }
    Ok(m)
}

/// A mesh object read with its class (render meshes inside B-reps and caches).
fn mesh_object(c: &mut Cur, limits: &Limits) -> Result<Option<RMesh>> {
    match read_object(c)? {
        Some(Object { class, mut data }) if class == MESH => read_mesh(&mut data, limits).map(Some),
        _ => Ok(None),
    }
}

/// The render meshes of an `ON_Brep`, one per face that has one (chunk version 3.1 and
/// later keep them after the topology); empty when the file saved none. With `want` false
/// at most one (empty) stands for "there are some".
pub(crate) fn read_brep(c: &mut Cur, limits: &Limits, want: bool) -> Result<Vec<RMesh>> {
    let (major, minor) = c.version()?;
    if major != 3 {
        // Rhino 2's trimmed faces (2.x): no meshes kept.
        return Ok(Vec::new());
    }
    // 2-D curves, 3-D curves, surfaces, vertices, edges, trims, loops: each a chunk.
    for _ in 0..7 {
        c.skip_chunk()?;
    }
    let faces = {
        let ch = c.chunk()?;
        let mut b = ch.body;
        b.version()?;
        usize::try_from(b.i32()?).unwrap_or(0)
    };
    // The bounding box.
    c.skip(48)?;
    let mut meshes = Vec::new();
    if minor >= 1 && !c.at_end() {
        let ch = c.chunk()?;
        if ch.tcode != TCODE_ANONYMOUS_CHUNK {
            return Err(Error::Malformed("B-rep render meshes".into()));
        }
        let mut b = ch.body;
        for _ in 0..faces {
            if b.u8()? == 0 {
                continue;
            }
            if !want {
                // Counting only: one is enough to know they are there.
                meshes.push(RMesh::default());
                break;
            }
            if let Some(m) = mesh_object(&mut b, limits)? {
                meshes.push(m);
            }
        }
    }
    Ok(meshes)
}

/// The render mesh an `ON_Extrusion` caches (chunk version 1.3 and later), if any.
pub(crate) fn read_extrusion(c: &mut Cur, limits: &Limits, want: bool) -> Result<Option<RMesh>> {
    let (mut b, major, minor) = c.versioned_chunk()?;
    if major != 1 || minor < 3 {
        return Ok(None);
    }
    // The profile, path line, path domain, up vector, end normals (two flags, two
    // vectors), path domain again, transposed flag, profile count, caps.
    b.skip_chunk()?;
    b.skip(48 + 16 + 24 + 2 + 48 + 16 + 1 + 4 + 2)?;
    let (mut cache, cmajor, _) = b.versioned_chunk()?;
    if cmajor != 1 {
        return Ok(None);
    }
    let mut best: Option<(u8, RMesh)> = None;
    while cache.u8()? == 1 {
        let (mut item, imajor, _) = cache.versioned_chunk()?;
        if imajor != 1 {
            continue;
        }
        let id = item.uuid()?;
        let rank = if id == RENDER_MESH_ID {
            3
        } else if id == FINE_MESH_ID {
            2
        } else if id == ANALYSIS_MESH_ID {
            0
        } else {
            1
        };
        if !want {
            return Ok(Some(RMesh::default()));
        }
        if best.as_ref().is_some_and(|(r, _)| *r >= rank) {
            continue;
        }
        if let Some(m) = mesh_object(&mut item, limits)? {
            best = Some((rank, m));
        }
    }
    Ok(best.map(|(_, m)| m))
}

/// A block instance: the definition it places and the 4×4 transform (row-major, column
/// vectors).
pub(crate) fn read_instance_ref(c: &mut Cur) -> Result<(Uuid, [f64; 16])> {
    let (major, _) = c.version()?;
    if major != 1 {
        return Err(Error::Unsupported(format!(
            "block instance version {major}"
        )));
    }
    let id = c.uuid()?;
    let x = c.f64s::<16>()?;
    Ok((id, x))
}

/// An `ON_InstanceDefinition`: its id, name and the ids of the objects it holds.
pub(crate) fn read_instance_def(c: &mut Cur) -> Result<(Uuid, String, Vec<Uuid>)> {
    if c.peek_tcode() == Some(TCODE_ANONYMOUS_CHUNK) {
        // Version 6 and later.
        let (mut b, major, _) = c.versioned_chunk()?;
        if major != 1 {
            return Err(Error::Unsupported("block definition version".into()));
        }
        let (id, name) = component_attributes(&mut b)?;
        b.i32()?;
        // The unit system (a chunk), description, URL, URL tag, bounding box.
        b.skip_chunk()?;
        for _ in 0..3 {
            b.string()?;
        }
        b.skip(48)?;
        let mut objects = Vec::new();
        if b.bool()? {
            let n = b.count(16)?;
            for _ in 0..n {
                objects.push(b.uuid()?);
            }
        }
        return Ok((id, name, objects));
    }
    let (major, _) = c.version()?;
    if major != 1 {
        return Err(Error::Unsupported("block definition version".into()));
    }
    let id = c.uuid()?;
    let n = c.count(16)?;
    let mut objects = Vec::with_capacity(n);
    for _ in 0..n {
        objects.push(c.uuid()?);
    }
    let name = c.string()?;
    Ok((id, name, objects))
}

/// The id and name of a model component's attribute chunk (version 6 and later).
fn component_attributes(c: &mut Cur) -> Result<(Uuid, String)> {
    let (mut b, major, _) = c.versioned_chunk()?;
    let (mut id, mut name) = (Uuid::default(), String::new());
    if major != 1 {
        return Ok((id, name));
    }
    // Serial numbers, id, type, index, name: a status byte each (1: the value follows).
    if b.u8()? == 1 {
        b.skip(12)?;
    }
    if b.u8()? == 1 {
        id = b.uuid()?;
    }
    if b.u8()? == 1 {
        b.skip(4)?;
    }
    if b.u8()? == 1 {
        b.skip(4)?;
    }
    if b.u8()? == 1 {
        name = b.string()?;
    }
    Ok((id, name))
}

/// What an object's attributes say about how to draw it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Attributes {
    pub id: Uuid,
    pub name: String,
    pub layer: i32,
    pub material: i32,
    pub color: [u8; 4],
    /// 0 by layer, 1 by object, 2 by material, 3 by parent (block).
    pub color_source: u8,
    /// 0 by layer, 1 by object, 3 by parent.
    pub material_source: u8,
    /// 0 normal, 1 hidden, 2 locked, 3 part of a block definition.
    pub mode: u8,
    pub visible: bool,
}

/// `ON_3dmObjectAttributes`, in the item format of version 5 and later (`v5`) or the
/// fixed one before.
pub(crate) fn read_attributes(c: &mut Cur, v5: bool) -> Result<Attributes> {
    let mut a = Attributes {
        layer: 0,
        material: -1,
        visible: true,
        ..Attributes::default()
    };
    let (major, minor) = c.version()?;
    if v5 && major == 2 {
        a.id = c.uuid()?;
        a.layer = c.i32()?;
        let mut item = c.u8()?;
        // Items in increasing order, each followed by the next item's number (0 ends).
        while item != 0 {
            match item {
                1 => a.name = c.string()?,
                2 => {
                    c.string()?;
                }
                3 => {
                    c.i32()?;
                }
                4 => a.material = c.i32()?,
                5 => c.skip_chunk()?,
                6 => a.color = c.color()?,
                7 => {
                    c.color()?;
                }
                8 => {
                    c.f64()?;
                }
                9 => {
                    c.u8()?;
                }
                10 => {
                    c.i32()?;
                }
                11 => a.visible = c.bool()?,
                12 => a.mode = c.u8()?,
                13 => a.color_source = c.u8()?,
                14 | 15 => {
                    c.u8()?;
                }
                16 => {
                    a.material_source = c.u8()?;
                    // Nothing later matters here.
                    break;
                }
                _ => break,
            }
            item = c.u8()?;
        }
        if a.mode == 1 {
            a.visible = false;
        }
        return Ok(a);
    }
    if major != 1 {
        return Err(Error::Unsupported(format!(
            "object attributes version {major}"
        )));
    }
    a.id = c.uuid()?;
    a.layer = c.i32()?;
    a.material = c.i32()?;
    a.color = c.color()?;
    // Obsolete line style, wire density.
    c.skip(2 + 2 + 8 + 8 + 4)?;
    a.mode = c.u8()?;
    a.color_source = c.u8()?;
    c.u8()?;
    a.material_source = c.u8()?;
    a.name = c.string()?;
    c.string()?;
    a.visible = a.mode != 1;
    if minor >= 1 {
        // Groups.
        let n = c.count(4)?;
        c.skip(n * 4)?;
        if minor >= 2 {
            a.visible = c.bool()?;
        }
    }
    Ok(a)
}

/// A layer: name, colour, render material and visibility.
#[derive(Clone, Debug, Default)]
pub(crate) struct Layer {
    pub name: String,
    pub color: [u8; 4],
    pub material: i32,
    pub visible: bool,
}

pub(crate) fn read_layer(c: &mut Cur) -> Result<Layer> {
    let (major, minor) = c.version()?;
    if major != 1 {
        return Err(Error::Unsupported(format!("layer version {major}")));
    }
    let mode = c.i32()?;
    c.i32()?;
    c.i32()?;
    let material = c.i32()?;
    c.i32()?;
    let color = c.color()?;
    c.skip(2 + 2 + 8 + 8)?;
    let name = c.string()?;
    let mut visible = mode != 1;
    if minor >= 1 {
        visible = c.bool()?;
    }
    Ok(Layer {
        name,
        color,
        material,
        visible,
    })
}

/// A material's name, diffuse colour and transparency (0 opaque … 1 clear).
#[derive(Clone, Debug, Default)]
pub(crate) struct Mat {
    pub name: String,
    pub diffuse: [u8; 4],
    pub transparency: f64,
}

pub(crate) fn read_material(c: &mut Cur) -> Result<Mat> {
    if c.peek_tcode() == Some(TCODE_ANONYMOUS_CHUNK) {
        // Version 6 and later.
        let (mut b, major, _) = c.versioned_chunk()?;
        if major != 1 {
            return Err(Error::Unsupported("material version".into()));
        }
        // Component attributes (id, name…) in a chunk of their own kind.
        let name = {
            let (mut a, amajor, _) = b.versioned_chunk()?;
            let mut name = String::new();
            if amajor == 1 {
                let bits = a.u32()?;
                if bits & 1 != 0 {
                    a.uuid()?;
                }
                if bits & 2 != 0 {
                    a.uuid()?;
                }
                if bits & 4 != 0 {
                    a.i32()?;
                }
                if bits & 8 != 0 {
                    name = a.string()?;
                }
            }
            name
        };
        b.uuid()?;
        b.color()?;
        let diffuse = b.color()?;
        // Emission, specular, reflection and transparent colours; refraction index,
        // reflectivity and shine.
        b.skip(16 + 24)?;
        let transparency = b.f64()?;
        return Ok(Mat {
            name,
            diffuse,
            transparency,
        });
    }
    let (major, _) = c.version()?;
    match major {
        1 => {
            c.color()?;
            let diffuse = c.color()?;
            // Emission and specular colours, shine.
            c.skip(8 + 8)?;
            let transparency = c.f64()?;
            Ok(Mat {
                name: String::new(),
                diffuse,
                transparency,
            })
        }
        2 => {
            let (mut b, _, _) = c.versioned_chunk()?;
            b.uuid()?;
            b.i32()?;
            let name = b.string()?;
            b.uuid()?;
            b.color()?;
            let diffuse = b.color()?;
            b.skip(16 + 24)?;
            let transparency = b.f64()?;
            Ok(Mat {
                name,
                diffuse,
                transparency,
            })
        }
        _ => Err(Error::Unsupported(format!("material version {major}"))),
    }
}

/// The application and revision history from the properties table.
#[derive(Clone, Debug, Default)]
pub(crate) struct Properties {
    pub application: Option<String>,
    pub url: Option<String>,
    pub details: Option<String>,
    pub created_by: Option<String>,
    pub created: Option<String>,
    pub edited_by: Option<String>,
    pub edited: Option<String>,
    pub revisions: Option<i32>,
    pub notes: Option<String>,
    pub opennurbs: u32,
    /// The preview image: (compressed, the record's data).
    pub preview: Option<(bool, usize, usize)>,
}

fn nonempty(s: String) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

/// A `struct tm` as "YYYY-MM-DD HH:MM" (UTC).
fn time(c: &mut Cur) -> Result<Option<String>> {
    let v: Vec<i32> = (0..8).map(|_| c.i32()).collect::<Result<_>>()?;
    let (sec, min, hour, mday, mon, year) = (v[0], v[1], v[2], v[3], v[4], v[5]);
    let _ = sec;
    if year <= 0 || mday <= 0 {
        return Ok(None);
    }
    Ok(Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02} UTC",
        year + 1900,
        mon + 1,
        mday,
        hour,
        min
    )))
}

pub(crate) fn read_properties(body: Cur) -> Properties {
    let mut p = Properties::default();
    for ch in body.chunks() {
        let Ok(ch) = ch else { break };
        let mut b = ch.body;
        let _ = (|| -> Result<()> {
            match ch.tcode {
                super::archive::TCODE_PROPERTIES_OPENNURBS_VERSION => {
                    p.opennurbs = u32::try_from(ch.value).unwrap_or(0);
                }
                super::archive::TCODE_PROPERTIES_APPLICATION => {
                    b.version()?;
                    p.application = nonempty(b.string()?);
                    p.url = nonempty(b.string()?);
                    p.details = nonempty(b.string()?);
                }
                super::archive::TCODE_PROPERTIES_REVISIONHISTORY => {
                    let (major, _) = b.version()?;
                    if major == 1 {
                        p.created_by = nonempty(b.string()?);
                        p.created = time(&mut b)?;
                        p.edited_by = nonempty(b.string()?);
                        p.edited = time(&mut b)?;
                        p.revisions = Some(b.i32()?);
                    }
                }
                super::archive::TCODE_PROPERTIES_NOTES => {
                    b.version()?;
                    b.i32()?;
                    p.notes = nonempty(b.string()?);
                }
                super::archive::TCODE_PROPERTIES_PREVIEWIMAGE => {
                    p.preview = Some((false, b.pos(), b.pos() + b.remaining()));
                }
                super::archive::TCODE_PROPERTIES_COMPRESSED_PREVIEWIMAGE => {
                    p.preview = Some((true, b.pos(), b.pos() + b.remaining()));
                }
                _ => {}
            }
            Ok(())
        })();
    }
    p
}

/// The model's length unit: (ON::LengthUnitSystem number, metres per unit, custom name).
pub(crate) fn read_units(body: Cur) -> Option<(u32, f64, String)> {
    for ch in body.chunks() {
        let ch = ch.ok()?;
        if ch.tcode != super::archive::TCODE_SETTINGS_UNITSANDTOLS {
            continue;
        }
        let mut b = ch.body;
        let r = (|| -> Result<(u32, f64, String)> {
            let version = b.i32()?;
            if !(100..200).contains(&version) {
                return Err(Error::Unsupported("units".into()));
            }
            let us = b.u32()?;
            b.skip(24)?;
            let (mut metres, mut name) = (0.0, String::new());
            if version >= 101 {
                b.skip(8)?;
                if version >= 102 {
                    metres = b.f64()?;
                    name = b.string()?;
                }
            }
            Ok((us, metres, name))
        })();
        return r.ok();
    }
    None
}
