//! Rhino 3D models (`.3dm`, openNURBS archives of Rhino 2 to 8): the meshes in the file and
//! the render meshes Rhino saves with B-reps and extrusions, placed by block instances
//! (nested, with their transforms), coloured by object, layer or material (one material per
//! distinct colour; vertex colours where a mesh has them). The archive's preview image
//! ([`preview_bmp`]) and its properties ([`facts`]) are read without the geometry.
//!
//! NURBS surfaces, polysurfaces and SubD objects without saved meshes are not tessellated:
//! they are counted in the notes, and a model with nothing else is refused with the reason.
//! Curves, points, annotations and lights are not drawn. Rhino 1 archives are refused.
//! Z is up; the file's unit is kept (units the mesh model has no name for are scaled to one
//! it has).

mod archive;
mod objects;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use archive::{Cur, Header, Uuid};
use objects::{Attributes, Layer, Mat, Properties, RMesh};

use crate::builder::{Affine, Builder, apply3, srgb_to_linear};
use crate::{Error, Limits, Material, Mesh, Result, Unit, Up};

/// The refusal for a model whose surfaces have no saved meshes.
pub const NO_RENDER_MESHES: &str = "this Rhino model has only NURBS surfaces without saved \
     render meshes; save it in Rhino with render meshes (Save Small off) to view it";

/// The refusal for a model of SubD objects only.
pub const SUBD_ONLY: &str = "this Rhino model has only SubD objects, which are not shown yet; \
     export them from Rhino as meshes to view them";

/// The refusal for a Rhino 1 archive.
pub const RHINO1: &str =
    "Rhino 1 models are not supported; open and save it in a newer Rhino to view it";

/// ON::object_type values (object record types).
mod kind {
    pub const POINT: u32 = 0x1;
    pub const POINTSET: u32 = 0x2;
    pub const CURVE: u32 = 0x4;
    pub const SURFACE: u32 = 0x8;
    pub const BREP: u32 = 0x10;
    pub const MESH: u32 = 0x20;
    pub const LIGHT: u32 = 0x100;
    pub const ANNOTATION: u32 = 0x200;
    pub const INSTANCE_REF: u32 = 0x1000;
    pub const TEXT_DOT: u32 = 0x2000;
    pub const DETAIL: u32 = 0x8000;
    pub const HATCH: u32 = 0x1_0000;
    pub const SUBD: u32 = 0x4_0000;
    pub const CLIPPLANE: u32 = 0x2000_0000;
    pub const EXTRUSION: u32 = 0x4000_0000;
}

/// One object record.
struct Obj<'a> {
    kind: u32,
    class: Option<Uuid>,
    data: Option<Cur<'a>>,
    attrs: Attributes,
}

/// What the archive holds, tables parsed, geometry not yet.
struct File<'a> {
    header: Header,
    props: Properties,
    units: Option<(u32, f64, String)>,
    layers: Vec<Layer>,
    mats: Vec<Mat>,
    idefs: HashMap<Uuid, (String, Vec<Uuid>)>,
    objects: Vec<Obj<'a>>,
    /// Records that could not be read.
    damaged: usize,
}

fn records<'a>(table: Cur<'a>, tcode: u32) -> impl Iterator<Item = Cur<'a>> {
    table
        .chunks()
        .map_while(|c| c.ok())
        .filter(move |c| c.tcode == tcode)
        .map(|c| c.body)
}

/// Reads the archive's tables; `objects` false skips the object table (preview, facts).
fn scan(bytes: &[u8], objects: bool) -> Result<File<'_>> {
    let header =
        archive::header(bytes).ok_or_else(|| Error::Malformed("not a Rhino 3DM file".into()))?;
    if header.raw == 1 {
        return Err(Error::Unsupported(RHINO1.into()));
    }
    let mut f = File {
        header,
        props: Properties::default(),
        units: None,
        layers: Vec::new(),
        mats: Vec::new(),
        idefs: HashMap::new(),
        objects: Vec::new(),
        damaged: 0,
    };
    let top = Cur::new(bytes, header.start, header.big());
    for ch in top.chunks() {
        let Ok(ch) = ch else {
            // A damaged or truncated tail: keep what was read.
            f.damaged += 1;
            break;
        };
        match ch.tcode {
            archive::TCODE_PROPERTIES_TABLE => f.props = objects::read_properties(ch.body),
            archive::TCODE_SETTINGS_TABLE => f.units = objects::read_units(ch.body),
            archive::TCODE_LAYER_TABLE => {
                for mut r in records(ch.body, archive::TCODE_LAYER_RECORD) {
                    let l = archive::read_object(&mut r)
                        .ok()
                        .flatten()
                        .and_then(|mut o| objects::read_layer(&mut o.data).ok());
                    f.layers.push(l.unwrap_or(Layer {
                        visible: true,
                        material: -1,
                        ..Layer::default()
                    }));
                }
            }
            archive::TCODE_MATERIAL_TABLE => {
                for mut r in records(ch.body, archive::TCODE_MATERIAL_RECORD) {
                    let m = archive::read_object(&mut r)
                        .ok()
                        .flatten()
                        .and_then(|mut o| objects::read_material(&mut o.data).ok());
                    f.mats.push(m.unwrap_or_default());
                }
            }
            archive::TCODE_INSTANCE_DEFINITION_TABLE => {
                for mut r in records(ch.body, archive::TCODE_INSTANCE_DEFINITION_RECORD) {
                    match archive::read_object(&mut r)
                        .ok()
                        .flatten()
                        .and_then(|mut o| objects::read_instance_def(&mut o.data).ok())
                    {
                        Some((id, name, members)) => {
                            f.idefs.insert(id, (name, members));
                        }
                        None => f.damaged += 1,
                    }
                }
            }
            archive::TCODE_OBJECT_TABLE if objects => {
                let v5 = header.raw >= 50 || (header.raw >= 5 && f.props.opennurbs >= 200_712_190);
                for r in records(ch.body, archive::TCODE_OBJECT_RECORD) {
                    match object_record(r, v5) {
                        Ok(o) => f.objects.push(o),
                        Err(_) => f.damaged += 1,
                    }
                }
            }
            _ => {}
        }
    }
    Ok(f)
}

fn object_record(body: Cur<'_>, v5: bool) -> Result<Obj<'_>> {
    let mut o = Obj {
        kind: 0,
        class: None,
        data: None,
        attrs: Attributes {
            material: -1,
            visible: true,
            ..Attributes::default()
        },
    };
    for ch in body.chunks() {
        let ch = ch?;
        match ch.tcode {
            archive::TCODE_OBJECT_RECORD_TYPE => o.kind = u32::try_from(ch.value).unwrap_or(0),
            archive::TCODE_OPENNURBS_CLASS => {
                if let Some(obj) = archive::object_in(ch.body)? {
                    o.class = Some(obj.class);
                    o.data = Some(obj.data);
                }
            }
            archive::TCODE_OBJECT_RECORD_ATTRIBUTES => {
                let mut b = ch.body;
                if let Ok(a) = objects::read_attributes(&mut b, v5) {
                    o.attrs = a;
                }
            }
            _ => {}
        }
    }
    Ok(o)
}

/// What an object draws.
enum Geo {
    Meshes(Vec<RMesh>),
    Instance(Uuid, [f64; 16]),
    /// A surface or polysurface without a saved mesh.
    Unmeshed,
    /// A SubD object (Rhino saves no mesh with it).
    Subd,
    /// Curves, points, annotations…
    NotDrawn,
}

fn geometry(o: &Obj, limits: &Limits) -> Result<Geo> {
    let (Some(class), Some(data)) = (o.class, o.data) else {
        return Ok(Geo::NotDrawn);
    };
    let mut d = data;
    if class == objects::MESH {
        return Ok(Geo::Meshes(vec![objects::read_mesh(&mut d, limits)?]));
    }
    if objects::BREPS.contains(&class) {
        let b = objects::read_brep(&mut d, limits, true)?;
        return Ok(if b.is_empty() {
            Geo::Unmeshed
        } else {
            Geo::Meshes(b)
        });
    }
    if class == objects::EXTRUSION {
        return Ok(match objects::read_extrusion(&mut d, limits, true)? {
            Some(m) => Geo::Meshes(vec![m]),
            None => Geo::Unmeshed,
        });
    }
    if class == objects::INSTANCE_REF {
        let (id, x) = objects::read_instance_ref(&mut d)?;
        return Ok(Geo::Instance(id, x));
    }
    if class == objects::SUBD || o.kind == kind::SUBD {
        return Ok(Geo::Subd);
    }
    if o.kind == kind::SURFACE {
        return Ok(Geo::Unmeshed);
    }
    Ok(Geo::NotDrawn)
}

/// The unit, and the factor taking the file's coordinates to it.
fn unit_of(units: &Option<(u32, f64, String)>) -> (Unit, f64) {
    let Some((us, metres, _)) = units else {
        return (Unit::Unknown, 1.0);
    };
    match us {
        1 => (Unit::Micron, 1.0),
        2 => (Unit::Millimetre, 1.0),
        3 => (Unit::Centimetre, 1.0),
        4 => (Unit::Metre, 1.0),
        5 => (Unit::Metre, 1e3),
        6 => (Unit::Inch, 1e-6),
        7 => (Unit::Inch, 1e-3),
        8 => (Unit::Inch, 1.0),
        9 => (Unit::Foot, 1.0),
        10 => (Unit::Foot, 5280.0),
        11 if *metres > 0.0 && metres.is_finite() => (Unit::Metre, *metres),
        12 => (Unit::Micron, 1e-4),
        13 => (Unit::Micron, 1e-3),
        14 => (Unit::Centimetre, 10.0),
        15 => (Unit::Metre, 10.0),
        16 => (Unit::Metre, 100.0),
        17 => (Unit::Metre, 1e6),
        18 => (Unit::Metre, 1e9),
        19 => (Unit::Foot, 3.0),
        20 => (Unit::Inch, 1.0 / 72.0),
        21 => (Unit::Inch, 1.0 / 6.0),
        22 => (Unit::Metre, 1852.0),
        23 => (Unit::Metre, 1.495_978_707e11),
        24 => (Unit::Metre, 9.460_730_472_580_8e15),
        25 => (Unit::Metre, 3.085_677_581_491_367e16),
        _ => (Unit::Unknown, 1.0),
    }
}

/// The unit's name for the info panel.
fn unit_name(units: &Option<(u32, f64, String)>) -> Option<String> {
    let (us, metres, custom) = units.as_ref()?;
    let n = match us {
        0 => "none",
        1 => "microns",
        2 => "millimetres",
        3 => "centimetres",
        4 => "metres",
        5 => "kilometres",
        6 => "microinches",
        7 => "mils",
        8 => "inches",
        9 => "feet",
        10 => "miles",
        11 => {
            let name = if custom.trim().is_empty() {
                "custom"
            } else {
                custom.trim()
            };
            return Some(format!("{name} ({metres} m)"));
        }
        12 => "ångströms",
        13 => "nanometres",
        14 => "decimetres",
        15 => "decametres",
        16 => "hectometres",
        17 => "megametres",
        18 => "gigametres",
        19 => "yards",
        20 => "printer's points",
        21 => "printer's picas",
        22 => "nautical miles",
        23 => "astronomical units",
        24 => "light years",
        25 => "parsecs",
        _ => return None,
    };
    Some(n.to_owned())
}

/// A colour for the triangles: sRGB with alpha, `None` for the default look.
type Look = Option<[u8; 4]>;

/// Deepest nesting of block instances followed.
const MAX_BLOCK_DEPTH: usize = 16;

/// Most block placements followed in all.
const MAX_PLACEMENTS: usize = 4_000_000;

struct Emit<'f, 'a> {
    f: &'f File<'a>,
    by_id: HashMap<Uuid, usize>,
    /// Geometry read once per object (blocks place it many times).
    geo: HashMap<usize, std::rc::Rc<Geo>>,
    limits: Limits,
    b: Builder,
    materials: HashMap<[u8; 4], u32>,
    unmeshed: usize,
    subd: usize,
    not_drawn: usize,
    hidden: usize,
    damaged: usize,
    missing_blocks: usize,
    deep: bool,
    /// Block definitions being placed (outermost first).
    open_blocks: Vec<Uuid>,
    placements: usize,
}

impl Emit<'_, '_> {
    fn layer(&self, a: &Attributes) -> Option<&Layer> {
        usize::try_from(a.layer)
            .ok()
            .and_then(|i| self.f.layers.get(i))
    }

    /// The name of the material the object draws with, when it has a named one.
    fn material_name(&self, a: &Attributes) -> Option<String> {
        let index = match a.material_source {
            1 => a.material,
            0 => self.layer(a)?.material,
            _ => return None,
        };
        let m = usize::try_from(index)
            .ok()
            .and_then(|i| self.f.mats.get(i))?;
        Some(m.name.trim().to_owned()).filter(|n| !n.is_empty())
    }

    fn material_colour(&self, index: i32) -> Option<[u8; 4]> {
        let m = usize::try_from(index)
            .ok()
            .and_then(|i| self.f.mats.get(i))?;
        let t = if m.transparency.is_finite() {
            m.transparency.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let [r, g, b, _] = m.diffuse;
        Some([r, g, b, ((1.0 - t) * 255.0).round() as u8])
    }

    /// The object's colour: its material's when it has one, else its display colour;
    /// black (Rhino's default layer colour) draws with the default look.
    fn look(&self, a: &Attributes, parent: Look) -> Look {
        let layer = self.layer(a);
        let material = match a.material_source {
            1 => self.material_colour(a.material),
            3 => parent,
            _ => layer.and_then(|l| self.material_colour(l.material)),
        };
        if material.is_some() {
            return material;
        }
        let display = match a.color_source {
            1 => Some(a.color),
            2 => None,
            3 => return parent,
            _ => layer.map(|l| l.color),
        };
        display
            .map(|[r, g, b, _]| [r, g, b, 255])
            .filter(|c| c[..3] != [0, 0, 0])
    }

    fn material_for(&mut self, look: Look, name: &str) -> Option<u32> {
        let c = look?;
        if let Some(&m) = self.materials.get(&c) {
            return Some(m);
        }
        let lin = |v: u8| srgb_to_linear(f32::from(v) / 255.0);
        let m = self.b.add_material(Material {
            name: name.to_owned(),
            base_color: [lin(c[0]), lin(c[1]), lin(c[2]), f32::from(c[3]) / 255.0],
            texture: None,
        });
        self.materials.insert(c, m);
        Some(m)
    }

    fn shown(&self, a: &Attributes) -> bool {
        a.visible && a.mode != 1 && self.layer(a).is_none_or(|l| l.visible)
    }

    fn geometry(&mut self, i: usize) -> std::rc::Rc<Geo> {
        if let Some(g) = self.geo.get(&i) {
            return g.clone();
        }
        let g = match geometry(&self.f.objects[i], &self.limits) {
            Ok(g) => g,
            Err(Error::TooLarge(_)) => {
                self.damaged += 1;
                Geo::NotDrawn
            }
            Err(_) => {
                self.damaged += 1;
                Geo::NotDrawn
            }
        };
        let g = std::rc::Rc::new(g);
        self.geo.insert(i, g.clone());
        g
    }

    fn draw(&mut self, i: usize, xf: &Affine, parent: Look, depth: usize) -> Result<()> {
        let a = self.f.objects[i].attrs.clone();
        let look = self.look(&a, parent);
        let geo = self.geometry(i);
        match &*geo {
            Geo::Meshes(ms) => {
                let name = self
                    .material_name(&a)
                    .or_else(|| Some(a.name.trim().to_owned()).filter(|n| !n.is_empty()))
                    .or_else(|| self.layer(&a).map(|l| l.name.clone()))
                    .unwrap_or_default();
                let m = self.material_for(look, &name);
                self.b.material(m);
                for mesh in ms {
                    self.mesh(mesh, xf)?;
                }
            }
            Geo::Instance(idef, x) => {
                // Too deep, a definition that holds itself, or too many placements in all
                // (a hostile file's blocks can multiply without end).
                if depth >= MAX_BLOCK_DEPTH || self.open_blocks.contains(idef) {
                    self.deep = true;
                    return Ok(());
                }
                self.placements += 1;
                if self.placements > MAX_PLACEMENTS {
                    return Err(Error::TooLarge(format!(
                        "more than {MAX_PLACEMENTS} block placements"
                    )));
                }
                let Some((_, members)) = self.f.idefs.get(idef) else {
                    self.missing_blocks += 1;
                    return Ok(());
                };
                let m = Affine([
                    [x[0], x[1], x[2], x[3]],
                    [x[4], x[5], x[6], x[7]],
                    [x[8], x[9], x[10], x[11]],
                ]);
                let inner = xf.then_after(&m);
                let members: Vec<usize> = members
                    .iter()
                    .filter_map(|id| self.by_id.get(id).copied())
                    .collect();
                self.open_blocks.push(*idef);
                for k in members {
                    if self.shown(&self.f.objects[k].attrs) {
                        self.draw(k, &inner, look, depth + 1)?;
                    }
                }
                self.open_blocks.pop();
            }
            Geo::Unmeshed => self.unmeshed += 1,
            Geo::Subd => self.subd += 1,
            Geo::NotDrawn => self.not_drawn += 1,
        }
        Ok(())
    }

    fn mesh(&mut self, m: &RMesh, xf: &Affine) -> Result<()> {
        let nv = m.v.len();
        if nv == 0 || m.f.is_empty() {
            return Ok(());
        }
        self.b.reserve_vertices(nv)?;
        self.b.reserve_triangles(m.triangles())?;
        let nm = xf.normal_matrix();
        let flip = xf.det() < 0.0;
        let base = self.b.vertex_count();
        for (k, &p) in m.v.iter().enumerate() {
            let n = m.n.as_ref().map(|n| apply3(&nm, n[k]));
            let c = m.c.as_ref().map(|c| c[k]);
            self.b.vertex(xf.point(p), n, c, None)?;
        }
        let ok = |i: u32| (i as usize) < nv;
        for f in &m.f {
            if !(ok(f[0]) && ok(f[1]) && ok(f[2]) && ok(f[3])) {
                continue;
            }
            let [a, b, c, d] = f.map(|i| base + i);
            let mut tri = |x: u32, y: u32, z: u32| {
                if flip {
                    self.b.triangle(x, z, y)
                } else {
                    self.b.triangle(x, y, z)
                }
            };
            tri(a, b, c)?;
            if f[2] != f[3] {
                tri(a, c, d)?;
            }
        }
        Ok(())
    }
}

/// The model's meshes and cached render meshes as one mesh.
pub fn read(bytes: &[u8], limits: &Limits) -> Result<Mesh> {
    let f = scan(bytes, true)?;
    let (unit, scale) = unit_of(&f.units);
    let mut b = Builder::new(limits);
    b.unit = unit;
    b.up = Up::Z;
    b.generator = f.props.application.clone();
    let by_id = f
        .objects
        .iter()
        .enumerate()
        .filter(|(_, o)| !o.attrs.id.is_nil())
        .map(|(i, o)| (o.attrs.id, i))
        .collect();
    let mut e = Emit {
        f: &f,
        by_id,
        geo: HashMap::new(),
        limits: *limits,
        b,
        materials: HashMap::new(),
        unmeshed: 0,
        subd: 0,
        not_drawn: 0,
        hidden: 0,
        damaged: f.damaged,
        missing_blocks: 0,
        deep: false,
        open_blocks: Vec::new(),
        placements: 0,
    };
    let root = Affine([
        [scale, 0.0, 0.0, 0.0],
        [0.0, scale, 0.0, 0.0],
        [0.0, 0.0, scale, 0.0],
    ]);
    for i in 0..f.objects.len() {
        let a = &f.objects[i].attrs;
        if a.mode == 3 {
            // Part of a block definition: drawn where instances place it.
            continue;
        }
        if !e.shown(a) {
            e.hidden += 1;
            continue;
        }
        e.draw(i, &root, None, 0)?;
    }
    let Emit {
        mut b,
        unmeshed,
        subd,
        not_drawn,
        hidden,
        damaged,
        missing_blocks,
        deep,
        ..
    } = e;
    if b.triangle_count() == 0 {
        let reason = if unmeshed > 0 {
            NO_RENDER_MESHES.to_owned()
        } else if subd > 0 {
            SUBD_ONLY.to_owned()
        } else if damaged > 0 {
            return Err(Error::Malformed(format!(
                "{damaged} damaged records and nothing to show"
            )));
        } else if not_drawn > 0 {
            "this Rhino model has no surfaces or meshes, only curves, points or annotations"
                .to_owned()
        } else if hidden > 0 {
            "every object in this Rhino model is hidden".to_owned()
        } else {
            "this Rhino model is empty".to_owned()
        };
        return Err(Error::Unsupported(reason));
    }
    if unmeshed > 0 {
        b.note(format!(
            "{unmeshed} surfaces or polysurfaces have no saved render mesh and are not shown"
        ));
    }
    if subd > 0 {
        b.note(format!("{subd} SubD objects are not shown yet"));
    }
    if not_drawn > 0 {
        b.note(format!(
            "{not_drawn} curves, points, annotations or lights are not drawn"
        ));
    }
    if hidden > 0 {
        b.note(format!("{hidden} hidden objects are not shown"));
    }
    if damaged > 0 {
        b.note(format!("{damaged} damaged records were skipped"));
    }
    if missing_blocks > 0 {
        b.note(format!(
            "{missing_blocks} block instances name a definition the file lacks"
        ));
    }
    if deep {
        b.note_once("blocks nested more than 16 deep or inside themselves are not shown");
    }
    b.finish()
}

/// The preview image the file carries, as a complete BMP file.
pub fn preview_bmp(bytes: &[u8]) -> Option<Vec<u8>> {
    let f = scan(bytes, false).ok()?;
    let (compressed, start, end) = f.props.preview?;
    let mut c = Cur::new(&bytes[..end], start, f.header.big());
    let mut h = [0u8; 40];
    let size = c.i32().ok()?;
    let width = c.i32().ok()?;
    let height = c.i32().ok()?;
    let planes = c.i16().ok()?;
    let bits = c.i16().ok()?;
    let compression = c.i32().ok()?;
    let image = c.i32().ok()?;
    let rest: Vec<i32> = (0..4).map(|_| c.i32()).collect::<Result<_>>().ok()?;
    let _ = size;
    if width <= 0 || height == 0 || image <= 0 || width > 16_384 || height.abs() > 16_384 {
        return None;
    }
    let colours = if rest[2] != 0 {
        usize::try_from(rest[2]).ok()?
    } else {
        match bits {
            1 => 2,
            4 => 16,
            8 => 256,
            _ => 0,
        }
    };
    if colours > 256 {
        return None;
    }
    let image = image as usize;
    let palette = colours * 4;
    let mut data = Vec::new();
    if compressed {
        // Palette and pixels in one buffer, or the palette then the pixels.
        let total = c.peek_u32()? as usize;
        if total == palette + image {
            data = c.compressed(total).ok()??;
        } else if total == palette {
            data = c.compressed(palette).ok()?.unwrap_or_default();
            data.extend(c.compressed(image).ok()??);
        } else {
            return None;
        }
    } else {
        data.extend_from_slice(c.bytes(palette + image).ok()?);
    }
    if data.len() != palette + image {
        return None;
    }
    h[0..4].copy_from_slice(&40u32.to_le_bytes());
    h[4..8].copy_from_slice(&width.to_le_bytes());
    h[8..12].copy_from_slice(&height.to_le_bytes());
    h[12..14].copy_from_slice(&planes.to_le_bytes());
    h[14..16].copy_from_slice(&bits.to_le_bytes());
    h[16..20].copy_from_slice(&compression.to_le_bytes());
    h[20..24].copy_from_slice(&(image as u32).to_le_bytes());
    for (k, v) in rest.iter().enumerate() {
        h[24 + 4 * k..28 + 4 * k].copy_from_slice(&v.to_le_bytes());
    }
    let offset = 14 + 40 + palette;
    let total = offset + image;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(offset as u32).to_le_bytes());
    out.extend_from_slice(&h);
    out.extend_from_slice(&data);
    Some(out)
}

/// The preview's size, from its header.
fn preview_size(bytes: &[u8], f: &File) -> Option<(i32, i32)> {
    let (_, start, end) = f.props.preview?;
    let mut c = Cur::new(&bytes[..end], start, f.header.big());
    c.i32().ok()?;
    let w = c.i32().ok()?;
    let h = c.i32().ok()?;
    (w > 0 && h != 0).then_some((w, h.abs()))
}

/// Facts for the info panel (version, application, units, object counts), cheap: no
/// geometry is decoded.
pub fn facts(bytes: &[u8]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let Some(h) = archive::header(bytes) else {
        return out;
    };
    let mut add = |k: &str, v: String| out.push((k.to_owned(), v));
    add("Rhino version", h.rhino().to_string());
    let Ok(f) = scan(bytes, true) else {
        return out;
    };
    let p = &f.props;
    let mut add = |k: &str, v: &Option<String>| {
        if let Some(v) = v {
            out.push((k.to_owned(), v.clone()));
        }
    };
    add("Saved by", &p.application);
    add("Application URL", &p.url);
    add("Application details", &p.details);
    add("Created by", &p.created_by);
    add("Created", &p.created);
    add("Last edited by", &p.edited_by);
    add("Last edited", &p.edited);
    add("Units", &unit_name(&f.units));
    if let Some(r) = p.revisions.filter(|&r| r > 0) {
        out.push(("Revisions".into(), r.to_string()));
    }
    out.push(("Objects".into(), f.objects.len().to_string()));
    let names: [(u32, &str); 15] = [
        (kind::MESH, "Meshes"),
        (kind::BREP, "Surfaces and polysurfaces"),
        (kind::SURFACE, "Untrimmed surfaces"),
        (kind::EXTRUSION, "Extrusions"),
        (kind::SUBD, "SubD objects"),
        (kind::CURVE, "Curves"),
        (kind::POINT, "Points"),
        (kind::POINTSET, "Point clouds"),
        (kind::ANNOTATION, "Annotations"),
        (kind::TEXT_DOT, "Text dots"),
        (kind::HATCH, "Hatches"),
        (kind::INSTANCE_REF, "Block instances"),
        (kind::LIGHT, "Lights"),
        (kind::DETAIL, "Details"),
        (kind::CLIPPLANE, "Clipping planes"),
    ];
    for (k, name) in names {
        let n = f.objects.iter().filter(|o| o.kind == k).count();
        if n > 0 {
            out.push((name.into(), n.to_string()));
        }
    }
    if !f.idefs.is_empty() {
        out.push(("Block definitions".into(), f.idefs.len().to_string()));
    }
    out.push(("Layers".into(), f.layers.len().to_string()));
    if !f.mats.is_empty() {
        out.push(("Materials".into(), f.mats.len().to_string()));
    }
    // Render meshes: whether the surfaces carry them (without decoding them).
    let (mut with, mut without) = (0usize, 0usize);
    let limits = Limits::default();
    for o in &f.objects {
        let (Some(class), Some(mut d)) = (o.class, o.data) else {
            continue;
        };
        let has = if objects::BREPS.contains(&class) {
            objects::read_brep(&mut d, &limits, false).map(|b| !b.is_empty())
        } else if class == objects::EXTRUSION {
            objects::read_extrusion(&mut d, &limits, false).map(|m| m.is_some())
        } else {
            continue;
        };
        match has {
            Ok(true) => with += 1,
            _ => without += 1,
        }
    }
    if with + without > 0 {
        let v = match (with, without) {
            (_, 0) => "saved".to_owned(),
            (0, _) => "not saved".to_owned(),
            (w, n) => format!("saved for {w} of {} objects", w + n),
        };
        out.push(("Render meshes".into(), v));
    }
    if let Some((w, h)) = preview_size(bytes, &f) {
        out.push(("Preview image".into(), format!("{w} × {h}")));
    }
    if let Some(n) = &p.notes {
        let mut s: String = n.chars().take(200).collect();
        if s.len() < n.len() {
            s.push('…');
        }
        out.push(("Notes".into(), s));
    }
    out
}
