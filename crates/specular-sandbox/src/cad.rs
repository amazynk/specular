//! The tessellated CAD model a host sends back for [`crate::protocol::Request::CadRead`].
//!
//! The host's OpenCASCADE bridge (`specular-decode-host/occt/specular_occt.cpp`) writes it;
//! [`CadModel::decode`] reads it in the viewer, checking every count, index and float, since
//! the bytes come from a process that parsed a hostile file.
//!
//! ```text
//! magic "SPCADM01", then records: u8 tag · u32 length · body[length], ending with tag 0.
//! 1 INFO  f64 unit (metres per coordinate unit) · str file unit · u32 solids · u32 faces
//!         · u32 faces without a mesh · u32 instances · f64 deflection · f32 box[6]
//! 2 NODE  u32 parent (0xFFFFFFFF: none) · u8 kind (0 assembly, 1 part) · u8 has colour
//!         · f32 rgba[4] (linear) · str name
//! 3 MESH  u32 node · u8 has colour · f32 rgba[4] · u32 vertices · u32 triangles
//!         · f32 positions[3v] · f32 normals[3v] · u32 indices[3t]
//! 4 NOTE  str
//! 5 PMI   u8 kind · u8 type · str name · u8 flags · f64 value · f64 upper · f64 lower
//!         · u8 qualifier · u8 fit hole · u8 fit variance · u8 fit grade · u8 decimals (int)
//!         · u8 decimals (fraction) · u8 value type · u8 material · u8 zone · f64 zone value
//!         · f64 max value · u16 m · u8 modifiers[m]
//!         · u16 d · d × (str letter · u8 position · u16 m · u8 modifiers[m]
//!                        · u8 with-value kind · f64 with value)
//!         · str datum letter · u8 target type · u32 target number · f64 target length
//!         · f64 target width · f32 text[3] · f32 plane[9] (origin, x, normal)
//!         · f32 point[3] · f32 point2[3] · u32 p · p × (u32 n · f32 points[3n])
//!         · u32 f · f × (u32 MESH record · u32 first triangle · u32 triangles)
//! 6 VIEW  str name · u8 projection · f32 eye[3] · f32 direction[3] · f32 up[3] · f64 zoom
//!         · f64 window width · f64 window height · u32 n · u32 PMI records[n]
//! ```
//!
//! PMI codes are OpenCASCADE's XDE enumerations (`XCAFDimTolObjects_*`), read by
//! `specular_decode::pmi`. PMI and views come after the meshes; MESH records are counted from
//! 0 in the order they come, PMI records likewise.
//!
//! Little endian; strings are a `u32` length and UTF-8. Positions carry every assembly
//! transformation; triangles wind counter-clockwise seen from outside. Unknown tags are
//! skipped. Nodes come in depth-first order, a parent before its children.

/// First bytes of a model.
pub const MAGIC: &[u8; 8] = b"SPCADM01";

const NO_PARENT: u32 = u32::MAX;
const MAX_NODES: usize = 4_000_000;
const MAX_NAME: usize = 64 << 10;

/// One node of the assembly tree.
#[derive(Debug, Clone, PartialEq)]
pub struct CadNode {
    /// Index of the parent node; `None` at the top.
    pub parent: Option<u32>,
    /// An assembly (it has children) rather than a part (it has geometry).
    pub assembly: bool,
    /// Linear RGBA, when the file gives the node a colour.
    pub colour: Option<[f32; 4]>,
    pub name: String,
}

/// The triangles of one part instance in one colour.
#[derive(Debug, Clone, PartialEq)]
pub struct CadMesh {
    /// Index of its [`CadNode`].
    pub node: u32,
    /// Linear RGBA, when the file gives these faces (or their part) a colour.
    pub colour: Option<[f32; 4]>,
    pub positions: Vec<[f32; 3]>,
    /// Unit normals, one per position.
    pub normals: Vec<[f32; 3]>,
    /// Three per triangle.
    pub indices: Vec<u32>,
}

/// What a [`CadPmi`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PmiKind {
    #[default]
    Dimension,
    GeometricTolerance,
    Datum,
    DatumTarget,
    /// Drawn only, with no semantic data.
    Graphic,
}

/// [`CadPmi::flags`]: a nominal value.
pub const PMI_VALUE: u8 = 1;
/// A plus/minus tolerance (`upper`, and `lower` as a magnitude below).
pub const PMI_PLUS_MINUS: u8 = 2;
/// Limits (`lower`, `upper`).
pub const PMI_RANGE: u8 = 4;
/// A class of tolerance (an ISO 286 fit).
pub const PMI_FIT: u8 = 8;
pub const PMI_TEXT: u8 = 16;
pub const PMI_PLANE: u8 = 32;
pub const PMI_POINT: u8 = 64;
pub const PMI_POINT2: u8 = 128;

/// A datum a geometric tolerance refers to.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CadDatumRef {
    pub letter: String,
    /// 1 primary, 2 secondary, 3 tertiary.
    pub position: u8,
    /// `XCAFDimTolObjects_DatumSingleModif` codes.
    pub modifiers: Vec<u8>,
    /// `XCAFDimTolObjects_DatumModifWithValue` code and its value.
    pub with_value_kind: u8,
    pub with_value: f64,
}

/// One annotation of the model's PMI (product manufacturing information), as XDE read it.
/// Values and points are in the model's coordinate unit, angles in degrees.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CadPmi {
    pub kind: PmiKind,
    /// `XCAFDimTolObjects_DimensionType`, `_GeomToleranceType` or `_DatumTargetType`.
    pub code: u8,
    pub name: String,
    pub flags: u8,
    pub value: f64,
    pub upper: f64,
    pub lower: f64,
    pub qualifier: u8,
    pub fit_hole: bool,
    pub fit_variance: u8,
    pub fit_grade: u8,
    pub decimals: (u8, u8),
    /// Tolerances: `GeomToleranceTypeValue` (⌀), `MatReqModif` and `ZoneModif` codes.
    pub value_type: u8,
    pub material: u8,
    pub zone: u8,
    pub zone_value: f64,
    pub max_value: f64,
    /// `DimensionModif` or `GeomToleranceModif` codes.
    pub modifiers: Vec<u8>,
    pub datums: Vec<CadDatumRef>,
    /// A datum's or datum target's letter.
    pub letter: String,
    pub target_type: u8,
    pub target_number: u32,
    pub target_length: f64,
    pub target_width: f64,
    pub text: [f32; 3],
    /// Origin, x direction and normal of the annotation plane.
    pub plane: [f32; 9],
    pub point: [f32; 3],
    pub point2: [f32; 3],
    /// The annotation as drawn in the file (polylines).
    pub polylines: Vec<Vec<[f32; 3]>>,
    /// The triangles of the faces it applies to: MESH record, first triangle, count.
    pub faces: Vec<[u32; 3]>,
}

/// A saved view of the model (an AP242 camera with the PMI it shows).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CadView {
    pub name: String,
    /// 0 none, 1 central (perspective), 2 parallel.
    pub projection: u8,
    pub eye: [f32; 3],
    pub direction: [f32; 3],
    pub up: [f32; 3],
    pub zoom: f64,
    pub width: f64,
    pub height: f64,
    /// Indices into [`CadModel::pmi`].
    pub pmi: Vec<u32>,
}

/// XDE's geometric tolerance types by code, as STEP names them (`_TOLERANCE` left off).
pub const TOLERANCE_TYPES: [&str; 16] = [
    "",
    "ANGULARITY",
    "CIRCULAR_RUNOUT",
    "CIRCULARITY",
    "COAXIALITY",
    "CONCENTRICITY",
    "CYLINDRICITY",
    "FLATNESS",
    "PARALLELISM",
    "PERPENDICULARITY",
    "POSITION",
    "LINE_PROFILE",
    "SURFACE_PROFILE",
    "STRAIGHTNESS",
    "SYMMETRY",
    "TOTAL_RUNOUT",
];

/// XDE's geometric tolerance modifiers by code, as STEP names them.
pub const TOLERANCE_MODIFIERS: [&str; 17] = [
    "ANY_CROSS_SECTION",
    "COMMON_ZONE",
    "EACH_RADIAL_ELEMENT",
    "FREE_STATE",
    "LEAST_MATERIAL_REQUIREMENT",
    "LINE_ELEMENT",
    "MAJOR_DIAMETER",
    "MAXIMUM_MATERIAL_REQUIREMENT",
    "MINOR_DIAMETER",
    "NOT_CONVEX",
    "PITCH_DIAMETER",
    "RECIPROCITY_REQUIREMENT",
    "SEPARATE_REQUIREMENT",
    "STATISTICAL_TOLERANCE",
    "TANGENT_PLANE",
    "ALL_AROUND",
    "ALL_OVER",
];

/// ISO 286 fundamental deviations (XDE's form variance codes; 0 none).
pub const FORM_VARIANCES: [&str; 29] = [
    "", "A", "B", "C", "CD", "D", "E", "EF", "F", "FG", "G", "H", "JS", "J", "K", "M", "N", "P",
    "R", "S", "T", "U", "V", "X", "Y", "Z", "ZA", "ZB", "ZC",
];

/// ISO 286 grades by XDE's code (IT01 to IT18), as the number written after the letter.
pub const GRADES: [&str; 20] = [
    "01", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
    "16", "17", "18",
];

/// Most PMI records and saved views read, and most polyline points in all.
const MAX_PMI: usize = 100_000;
const MAX_VIEWS: usize = 10_000;
const MAX_PMI_POINTS: usize = 4_000_000;

/// A STEP or IGES model as OpenCASCADE read and tessellated it.
#[derive(Debug, Clone, PartialEq)]
pub struct CadModel {
    /// Metres per coordinate unit (0.001: millimetres).
    pub unit_metres: f64,
    /// The length unit the file states ("MM", "INCH"), when it says.
    pub file_unit: String,
    pub solids: u32,
    pub faces: u32,
    /// Faces OpenCASCADE could not mesh (left out).
    pub faces_unmeshed: u32,
    /// Part instances placed in the assembly.
    pub instances: u32,
    /// The linear deflection used, in coordinate units.
    pub deflection: f64,
    /// min x, y, z, max x, y, z of the exact geometry, in coordinate units.
    pub bounds: [f32; 6],
    pub nodes: Vec<CadNode>,
    pub meshes: Vec<CadMesh>,
    /// What was approximated or left out.
    pub notes: Vec<String>,
    pub pmi: Vec<CadPmi>,
    pub views: Vec<CadView>,
}

impl Default for CadModel {
    fn default() -> Self {
        CadModel {
            unit_metres: 0.001,
            file_unit: String::new(),
            solids: 0,
            faces: 0,
            faces_unmeshed: 0,
            instances: 0,
            deflection: 0.0,
            bounds: [0.0; 6],
            nodes: Vec::new(),
            meshes: Vec::new(),
            notes: Vec::new(),
            pmi: Vec::new(),
            views: Vec::new(),
        }
    }
}

impl CadModel {
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }

    pub fn vertex_count(&self) -> usize {
        self.meshes.iter().map(|m| m.positions.len()).sum()
    }

    /// Reads a model, refusing more than `max_triangles` triangles or `max_vertices`
    /// vertices in all.
    pub fn decode(b: &[u8], max_triangles: u64, max_vertices: u64) -> Result<CadModel, String> {
        let mut r = Rd(b);
        if r.take(8)? != MAGIC {
            return Err("not a CAD model".into());
        }
        let mut m = CadModel::default();
        let (mut tris, mut verts) = (0u64, 0u64);
        let mut points = 0usize;
        loop {
            let tag = r.u8()?;
            if tag == 0 {
                r.u32()?;
                break;
            }
            let len = r.u32()? as usize;
            let mut body = Rd(r.take(len)?);
            match tag {
                1 => {
                    m.unit_metres = body.f64()?;
                    if !(m.unit_metres > 0.0 && m.unit_metres.is_finite()) {
                        return Err("bad unit".into());
                    }
                    m.file_unit = body.str()?;
                    m.solids = body.u32()?;
                    m.faces = body.u32()?;
                    m.faces_unmeshed = body.u32()?;
                    m.instances = body.u32()?;
                    m.deflection = body.f64()?;
                    for v in &mut m.bounds {
                        *v = body.f32()?;
                    }
                }
                2 => {
                    if m.nodes.len() >= MAX_NODES {
                        return Err("too many assembly nodes".into());
                    }
                    let parent = body.u32()?;
                    let parent = if parent == NO_PARENT {
                        None
                    } else if (parent as usize) < m.nodes.len() {
                        Some(parent)
                    } else {
                        return Err("assembly node before its parent".into());
                    };
                    let assembly = body.u8()? == 0;
                    let colour = body.colour()?;
                    let name = body.str()?;
                    m.nodes.push(CadNode {
                        parent,
                        assembly,
                        colour,
                        name,
                    });
                }
                3 => {
                    let node = body.u32()?;
                    if node as usize >= m.nodes.len() {
                        return Err("mesh of an unknown node".into());
                    }
                    let colour = body.colour()?;
                    let nv = body.u32()? as u64;
                    let nt = body.u32()? as u64;
                    verts += nv;
                    tris += nt;
                    if verts > max_vertices || tris > max_triangles {
                        return Err(format!(
                            "more than {max_triangles} triangles or {max_vertices} vertices"
                        ));
                    }
                    // Exactly the bytes the counts promise, before anything is allocated.
                    if body.0.len() as u64 != nv * 24 + nt * 12 {
                        return Err("mesh record size mismatch".into());
                    }
                    let (nv, nt) = (nv as usize, nt as usize);
                    let mut positions = Vec::with_capacity(nv);
                    for _ in 0..nv {
                        let p = body.vec3()?;
                        if !p.iter().all(|v| v.is_finite()) {
                            return Err("vertex is not finite".into());
                        }
                        positions.push(p);
                    }
                    let mut normals = Vec::with_capacity(nv);
                    for _ in 0..nv {
                        let n = body.vec3()?;
                        normals.push(if n.iter().all(|v| v.is_finite()) {
                            n
                        } else {
                            [0.0, 0.0, 1.0]
                        });
                    }
                    let mut indices = Vec::with_capacity(nt * 3);
                    for _ in 0..nt * 3 {
                        let i = body.u32()?;
                        if i as usize >= nv {
                            return Err("triangle index out of range".into());
                        }
                        indices.push(i);
                    }
                    m.meshes.push(CadMesh {
                        node,
                        colour,
                        positions,
                        normals,
                        indices,
                    });
                }
                4 => {
                    if m.notes.len() < 64 {
                        m.notes.push(body.str()?);
                    }
                    continue;
                }
                5 => {
                    if m.pmi.len() >= MAX_PMI {
                        return Err("too many PMI annotations".into());
                    }
                    let p = body.pmi(&mut points)?;
                    m.pmi.push(p);
                }
                6 => {
                    if m.views.len() >= MAX_VIEWS {
                        return Err("too many saved views".into());
                    }
                    let v = body.view()?;
                    m.views.push(v);
                }
                _ => continue,
            }
            if !body.0.is_empty() {
                return Err(format!("record {tag} has trailing bytes"));
            }
        }
        if !r.0.is_empty() {
            return Err("bytes after the end record".into());
        }
        // Face triangles inside their mesh, views' PMI inside the list.
        for p in &m.pmi {
            for f in &p.faces {
                let n = m
                    .meshes
                    .get(f[0] as usize)
                    .map(|x| x.indices.len() / 3)
                    .ok_or("PMI of an unknown mesh")?;
                if u64::from(f[1]) + u64::from(f[2]) > n as u64 {
                    return Err("PMI triangles out of range".into());
                }
            }
        }
        let n = m.pmi.len();
        for v in &m.views {
            if v.pmi.iter().any(|&i| i as usize >= n) {
                return Err("view of an unknown PMI".into());
            }
        }
        Ok(m)
    }

    /// The bytes [`CadModel::decode`] reads (the host bridge writes the same; tests use this).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        let mut rec = |tag: u8, body: Vec<u8>| {
            out.push(tag);
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&body);
        };
        let mut b = Wr::default();
        b.f64(self.unit_metres);
        b.str(&self.file_unit);
        for v in [self.solids, self.faces, self.faces_unmeshed, self.instances] {
            b.u32(v);
        }
        b.f64(self.deflection);
        for v in self.bounds {
            b.f32(v);
        }
        rec(1, b.0);
        for n in &self.nodes {
            let mut b = Wr::default();
            b.u32(n.parent.unwrap_or(NO_PARENT));
            b.0.push(if n.assembly { 0 } else { 1 });
            b.colour(n.colour);
            b.str(&n.name);
            rec(2, b.0);
        }
        for m in &self.meshes {
            let mut b = Wr::default();
            b.u32(m.node);
            b.colour(m.colour);
            b.u32(m.positions.len() as u32);
            b.u32((m.indices.len() / 3) as u32);
            for p in m.positions.iter().chain(&m.normals) {
                for v in p {
                    b.f32(*v);
                }
            }
            for i in &m.indices {
                b.u32(*i);
            }
            rec(3, b.0);
        }
        for s in &self.notes {
            let mut b = Wr::default();
            b.str(s);
            rec(4, b.0);
        }
        for p in &self.pmi {
            rec(5, Wr::pmi(p));
        }
        for v in &self.views {
            rec(6, Wr::view(v));
        }
        rec(0, Vec::new());
        out
    }
}

struct Rd<'a>(&'a [u8]);

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.0.len() < n {
            return Err("model truncated".into());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn f64(&mut self) -> Result<f64, String> {
        let mut a = [0u8; 8];
        a.copy_from_slice(self.take(8)?);
        Ok(f64::from_le_bytes(a))
    }
    fn vec3(&mut self) -> Result<[f32; 3], String> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    fn str(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        if n > MAX_NAME {
            return Err("name too long".into());
        }
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    /// A float; not finite: 0.
    fn fin64(&mut self) -> Result<f64, String> {
        let v = self.f64()?;
        Ok(if v.is_finite() { v } else { 0.0 })
    }
    /// Floats; not finite: 0.
    fn fin32s<const N: usize>(&mut self) -> Result<[f32; N], String> {
        let mut a = [0f32; N];
        for v in &mut a {
            let x = self.f32()?;
            *v = if x.is_finite() { x } else { 0.0 };
        }
        Ok(a)
    }
    fn bytes(&mut self, n: usize) -> Result<Vec<u8>, String> {
        Ok(self.take(n)?.to_vec())
    }
    /// A PMI record's body; `points` counts polyline points over the model.
    fn pmi(&mut self, points: &mut usize) -> Result<CadPmi, String> {
        let kind = match self.u8()? {
            0 => PmiKind::Dimension,
            1 => PmiKind::GeometricTolerance,
            2 => PmiKind::Datum,
            3 => PmiKind::DatumTarget,
            4 => PmiKind::Graphic,
            k => return Err(format!("unknown PMI kind {k}")),
        };
        let code = self.u8()?;
        let name = self.str()?;
        let flags = self.u8()?;
        let (value, upper, lower) = (self.fin64()?, self.fin64()?, self.fin64()?);
        let b = self.take(9)?;
        let (qualifier, fit_hole, fit_variance, fit_grade) = (b[0], b[1] != 0, b[2], b[3]);
        let decimals = (b[4], b[5]);
        let (value_type, material, zone) = (b[6], b[7], b[8]);
        let (zone_value, max_value) = (self.fin64()?, self.fin64()?);
        let n = self.u16()? as usize;
        let modifiers = self.bytes(n)?;
        let n = self.u16()? as usize;
        let mut datums = Vec::with_capacity(n.min(16));
        for _ in 0..n {
            let letter = self.str()?;
            let position = self.u8()?;
            let m = self.u16()? as usize;
            let modifiers = self.bytes(m)?;
            let with_value_kind = self.u8()?;
            let with_value = self.fin64()?;
            datums.push(CadDatumRef {
                letter,
                position,
                modifiers,
                with_value_kind,
                with_value,
            });
        }
        let letter = self.str()?;
        let target_type = self.u8()?;
        let target_number = self.u32()?;
        let (target_length, target_width) = (self.fin64()?, self.fin64()?);
        let text = self.fin32s::<3>()?;
        let plane = self.fin32s::<9>()?;
        let point = self.fin32s::<3>()?;
        let point2 = self.fin32s::<3>()?;
        let n = self.u32()? as usize;
        // Each polyline takes at least its count's 4 bytes.
        if n > self.0.len() / 4 {
            return Err("PMI polylines truncated".into());
        }
        let mut polylines = Vec::with_capacity(n);
        for _ in 0..n {
            let k = self.u32()? as usize;
            *points = points.saturating_add(k);
            if *points > MAX_PMI_POINTS || k > self.0.len() / 12 {
                return Err("PMI drawing too large or truncated".into());
            }
            let mut line = Vec::with_capacity(k);
            for _ in 0..k {
                line.push(self.fin32s::<3>()?);
            }
            polylines.push(line);
        }
        let n = self.u32()? as usize;
        if n > self.0.len() / 12 {
            return Err("PMI faces truncated".into());
        }
        let mut faces = Vec::with_capacity(n);
        for _ in 0..n {
            faces.push([self.u32()?, self.u32()?, self.u32()?]);
        }
        Ok(CadPmi {
            kind,
            code,
            name,
            flags,
            value,
            upper,
            lower,
            qualifier,
            fit_hole,
            fit_variance,
            fit_grade,
            decimals,
            value_type,
            material,
            zone,
            zone_value,
            max_value,
            modifiers,
            datums,
            letter,
            target_type,
            target_number,
            target_length,
            target_width,
            text,
            plane,
            point,
            point2,
            polylines,
            faces,
        })
    }
    fn view(&mut self) -> Result<CadView, String> {
        let name = self.str()?;
        let projection = self.u8()?;
        let eye = self.fin32s::<3>()?;
        let direction = self.fin32s::<3>()?;
        let up = self.fin32s::<3>()?;
        let (zoom, width, height) = (self.fin64()?, self.fin64()?, self.fin64()?);
        let n = self.u32()? as usize;
        if n > self.0.len() / 4 {
            return Err("view truncated".into());
        }
        let mut pmi = Vec::with_capacity(n);
        for _ in 0..n {
            pmi.push(self.u32()?);
        }
        Ok(CadView {
            name,
            projection,
            eye,
            direction,
            up,
            zoom,
            width,
            height,
            pmi,
        })
    }
    /// A has-colour byte and four floats (clamped to 0–1; not finite: no colour).
    fn colour(&mut self) -> Result<Option<[f32; 4]>, String> {
        let has = self.u8()? != 0;
        let c = [self.f32()?, self.f32()?, self.f32()?, self.f32()?];
        Ok((has && c.iter().all(|v| v.is_finite())).then(|| c.map(|v| v.clamp(0.0, 1.0))))
    }
}

#[derive(Default)]
struct Wr(Vec<u8>);

impl Wr {
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: usize) {
        self.0
            .extend_from_slice(&(v.min(u16::MAX as usize) as u16).to_le_bytes());
    }
    fn f32s(&mut self, a: &[f32]) {
        for v in a {
            self.f32(*v);
        }
    }
    fn pmi(p: &CadPmi) -> Vec<u8> {
        let mut b = Wr::default();
        b.u8(match p.kind {
            PmiKind::Dimension => 0,
            PmiKind::GeometricTolerance => 1,
            PmiKind::Datum => 2,
            PmiKind::DatumTarget => 3,
            PmiKind::Graphic => 4,
        });
        b.u8(p.code);
        b.str(&p.name);
        b.u8(p.flags);
        for v in [p.value, p.upper, p.lower] {
            b.f64(v);
        }
        for v in [
            p.qualifier,
            p.fit_hole as u8,
            p.fit_variance,
            p.fit_grade,
            p.decimals.0,
            p.decimals.1,
            p.value_type,
            p.material,
            p.zone,
        ] {
            b.u8(v);
        }
        b.f64(p.zone_value);
        b.f64(p.max_value);
        b.u16(p.modifiers.len());
        b.0.extend_from_slice(&p.modifiers);
        b.u16(p.datums.len());
        for d in &p.datums {
            b.str(&d.letter);
            b.u8(d.position);
            b.u16(d.modifiers.len());
            b.0.extend_from_slice(&d.modifiers);
            b.u8(d.with_value_kind);
            b.f64(d.with_value);
        }
        b.str(&p.letter);
        b.u8(p.target_type);
        b.u32(p.target_number);
        b.f64(p.target_length);
        b.f64(p.target_width);
        b.f32s(&p.text);
        b.f32s(&p.plane);
        b.f32s(&p.point);
        b.f32s(&p.point2);
        b.u32(p.polylines.len() as u32);
        for l in &p.polylines {
            b.u32(l.len() as u32);
            for q in l {
                b.f32s(q);
            }
        }
        b.u32(p.faces.len() as u32);
        for f in &p.faces {
            for v in f {
                b.u32(*v);
            }
        }
        b.0
    }
    fn view(v: &CadView) -> Vec<u8> {
        let mut b = Wr::default();
        b.str(&v.name);
        b.u8(v.projection);
        b.f32s(&v.eye);
        b.f32s(&v.direction);
        b.f32s(&v.up);
        b.f64(v.zoom);
        b.f64(v.width);
        b.f64(v.height);
        b.u32(v.pmi.len() as u32);
        for i in &v.pmi {
            b.u32(*i);
        }
        b.0
    }
    fn colour(&mut self, c: Option<[f32; 4]>) {
        self.0.push(c.is_some() as u8);
        for v in c.unwrap_or([1.0; 4]) {
            self.f32(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CadModel {
        CadModel {
            unit_metres: 0.001,
            file_unit: "INCH".into(),
            solids: 2,
            faces: 12,
            faces_unmeshed: 1,
            instances: 2,
            deflection: 0.05,
            bounds: [0.0, 0.0, 0.0, 10.0, 20.0, 30.0],
            nodes: vec![
                CadNode {
                    parent: None,
                    assembly: true,
                    colour: None,
                    name: "Gearbox".into(),
                },
                CadNode {
                    parent: Some(0),
                    assembly: false,
                    colour: Some([1.0, 0.0, 0.0, 1.0]),
                    name: "Wheel ü".into(),
                },
            ],
            meshes: vec![CadMesh {
                node: 1,
                colour: Some([0.5, 0.5, 0.5, 0.25]),
                positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                indices: vec![0, 1, 2],
            }],
            notes: vec!["1 faces could not be meshed".into()],
            pmi: vec![
                CadPmi {
                    kind: PmiKind::GeometricTolerance,
                    code: 10,
                    name: "Position.1".into(),
                    flags: PMI_VALUE | PMI_TEXT,
                    value: 0.25,
                    value_type: 1,
                    material: 1,
                    modifiers: vec![3],
                    datums: vec![CadDatumRef {
                        letter: "A".into(),
                        position: 1,
                        modifiers: vec![15],
                        ..CadDatumRef::default()
                    }],
                    text: [1.0, 2.0, 3.0],
                    polylines: vec![vec![[0.0; 3], [1.0, 1.0, 0.0]]],
                    faces: vec![[0, 0, 1]],
                    ..CadPmi::default()
                },
                CadPmi {
                    kind: PmiKind::Datum,
                    letter: "A".into(),
                    ..CadPmi::default()
                },
            ],
            views: vec![CadView {
                name: "MBD_0".into(),
                projection: 2,
                direction: [0.0, 0.0, -1.0],
                up: [0.0, 1.0, 0.0],
                pmi: vec![0, 1],
                ..CadView::default()
            }],
        }
    }

    #[test]
    fn round_trip() {
        let m = sample();
        let b = m.encode();
        assert_eq!(CadModel::decode(&b, 10, 10).unwrap(), m);
        assert_eq!(m.triangle_count(), 1);
        assert_eq!(m.vertex_count(), 3);
        // Every strict prefix fails, never panics.
        for n in 0..b.len() {
            assert!(CadModel::decode(&b[..n], 10, 10).is_err(), "prefix {n}");
        }
    }

    #[test]
    fn limits_and_bad_indices() {
        let b = sample().encode();
        assert!(CadModel::decode(&b, 0, 10).is_err());
        assert!(CadModel::decode(&b, 10, 2).is_err());
        let mut m = sample();
        m.meshes[0].indices[2] = 3;
        assert!(CadModel::decode(&m.encode(), 10, 10).is_err());
        let mut m = sample();
        m.nodes[1].parent = Some(1);
        assert!(CadModel::decode(&m.encode(), 10, 10).is_err());
        let mut m = sample();
        m.meshes[0].node = 2;
        assert!(CadModel::decode(&m.encode(), 10, 10).is_err());
        let mut m = sample();
        m.meshes[0].positions[0][0] = f32::NAN;
        assert!(CadModel::decode(&m.encode(), 10, 10).is_err());
        let mut m = sample();
        m.pmi[0].faces = vec![[0, 0, 2]];
        assert!(CadModel::decode(&m.encode(), 10, 10).is_err());
        let mut m = sample();
        m.pmi[0].faces = vec![[1, 0, 1]];
        assert!(CadModel::decode(&m.encode(), 10, 10).is_err());
        let mut m = sample();
        m.views[0].pmi = vec![2];
        assert!(CadModel::decode(&m.encode(), 10, 10).is_err());
        // Not finite PMI floats read as 0.
        let mut m = sample();
        m.pmi[0].value = f64::NAN;
        m.pmi[0].text[0] = f32::INFINITY;
        let d = CadModel::decode(&m.encode(), 10, 10).unwrap();
        assert_eq!((d.pmi[0].value, d.pmi[0].text[0]), (0.0, 0.0));
    }

    #[test]
    fn hostile_bytes_never_panic() {
        let b = sample().encode();
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        for i in 8..b.len() {
            for v in [0u8, 1, 0x7F, 0xFF] {
                let mut m = b.clone();
                m[i] = v;
                let _ = CadModel::decode(&m, 1 << 20, 1 << 20);
            }
        }
        for _ in 0..200 {
            let mut m = MAGIC.to_vec();
            for _ in 0..64 {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                m.push(s as u8);
            }
            let _ = CadModel::decode(&m, 1 << 20, 1 << 20);
        }
    }
}
