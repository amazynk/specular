//! 3D meshes: own readers for STL (ASCII and binary), OBJ with MTL, PLY (ASCII, binary little-
//! and big-endian), glTF 2.0 (`.gltf` with embedded or side buffers, `.glb`) and 3MF, all into
//! one model: an indexed triangle [`Mesh`] with per-vertex normals (computed when the file has
//! none), optional colours and texture coordinates, materials (a base colour and an optional
//! base-colour texture), the file's units where it says, and bounds.
//!
//! **Safety of input.** Every reader works on bytes that may be malformed or hostile: counts are
//! checked against the bytes present before anything is allocated, every size is capped by
//! [`Limits`], indices are checked, and no input panics. Side files (an OBJ's MTL and textures,
//! a glTF's buffers and images) are read only through a [`Resolver`]: [`DirResolver`] accepts
//! relative paths that stay inside the model's own folder tree (no absolute paths, no `..`
//! out of it, no symbolic links leading out, no URLs), so nothing touches the network.
//!
//! Textures come back as encoded bytes ([`ImageData`]); the caller decodes them with its own
//! decoders. [`preview`] renders a mesh on the CPU for thumbnails.

use std::path::Path;

pub mod alembic;
mod amf;
mod builder;
mod collada;
mod fbx;
#[doc(hidden)]
pub mod fixtures;
mod games;
mod gltf;
pub mod ifc;
mod lwo;
mod meshopt;
mod obj;
mod off;
mod ply;
mod poly;
pub mod preview;
mod resolve;
pub mod rhino;
mod scenes;
mod stl;
mod text;
mod three_ds;
mod threemf;
mod usd;
mod x3d;
mod xml;

#[cfg(test)]
mod tests;

/// Triangles over a plane polygon with holes (the IFC reader's ear clipping): indices into
/// the corners of `outer` followed by those of each hole, counter-clockwise whatever the
/// rings' turning.
pub fn triangulate_polygon(outer: &[[f64; 2]], holes: &[Vec<[f64; 2]>]) -> Vec<[u32; 3]> {
    ifc::triangulate(outer, holes)
}

pub use gltf::{GltfPoints, GltfScene, read_scene as read_gltf_scene};
pub use resolve::{DirResolver, NoResolver, Resolver, relative_climbing, safe_relative};

/// What went wrong reading a mesh.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("not supported: {0}")]
    Unsupported(String),
    #[error("too large: {0}")]
    TooLarge(String),
    #[error("no triangles")]
    Empty,
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::TooLarge(n.to_string())
    }
}

pub(crate) fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Malformed(msg.into()))
}

/// The file formats read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeshFormat {
    Stl,
    Obj,
    Ply,
    Gltf,
    Glb,
    ThreeMf,
    /// USD, text or binary (`.usd`, `.usda`, `.usdc`).
    Usd,
    /// A USDZ package.
    Usdz,
    /// COLLADA (`.dae`).
    Dae,
    /// A COLLADA package (`.zae`).
    Zae,
    /// Autodesk 3D Studio.
    ThreeDs,
    /// LightWave object (LWO2, LWOB).
    Lwo,
    /// Autodesk FBX, binary version 7 (geometry).
    Fbx,
    /// IFC building model (STEP physical file, `.ifczip`): own geometry for what building
    /// models use; see [`ifc`].
    Ifc,
    /// X3D, XML encoding (`.x3d`, gzipped `.x3dz`).
    X3d,
    /// VRML 97 and X3D's classic VRML encoding (`.wrl`, `.x3dv`, gzipped `.wrz`, `.x3dvz`).
    Vrml,
    /// Rhino 3D model (`.3dm`): its meshes and cached render meshes; see [`rhino`].
    Rhino,
    /// Additive Manufacturing File Format (`.amf`, XML, also zipped).
    Amf,
    /// Geomview Object File Format (`.off`, text).
    Off,
    /// Alembic archive (Ogawa container): sample 0 of its polygon meshes; see [`alembic`].
    Alembic,
    /// Quake II model (`.md2`): its first frame.
    Md2,
    /// MilkShape 3D (`.ms3d`).
    Ms3d,
    /// MagicaVoxel (`.vox`): voxels as cubes.
    Vox,
    /// AC3D (`.ac`).
    Ac3d,
    /// DirectX text model (`.x`).
    DirectX,
    /// BioVision motion capture (`.bvh`): the skeleton in its rest pose.
    Bvh,
}

impl MeshFormat {
    pub const ALL: [MeshFormat; 26] = [
        MeshFormat::Stl,
        MeshFormat::Obj,
        MeshFormat::Ply,
        MeshFormat::Gltf,
        MeshFormat::Glb,
        MeshFormat::ThreeMf,
        MeshFormat::Usd,
        MeshFormat::Usdz,
        MeshFormat::Dae,
        MeshFormat::Zae,
        MeshFormat::ThreeDs,
        MeshFormat::Lwo,
        MeshFormat::Fbx,
        MeshFormat::Ifc,
        MeshFormat::X3d,
        MeshFormat::Vrml,
        MeshFormat::Rhino,
        MeshFormat::Amf,
        MeshFormat::Off,
        MeshFormat::Alembic,
        MeshFormat::Md2,
        MeshFormat::Ms3d,
        MeshFormat::Vox,
        MeshFormat::Ac3d,
        MeshFormat::DirectX,
        MeshFormat::Bvh,
    ];

    pub fn name(self) -> &'static str {
        match self {
            MeshFormat::Stl => "STL",
            MeshFormat::Obj => "OBJ",
            MeshFormat::Ply => "PLY",
            MeshFormat::Gltf => "glTF",
            MeshFormat::Glb => "GLB",
            MeshFormat::ThreeMf => "3MF",
            MeshFormat::Usd => "USD",
            MeshFormat::Usdz => "USDZ",
            MeshFormat::Dae => "COLLADA",
            MeshFormat::Zae => "COLLADA package",
            MeshFormat::ThreeDs => "3DS",
            MeshFormat::Lwo => "LightWave",
            MeshFormat::Fbx => "FBX",
            MeshFormat::Ifc => "IFC",
            MeshFormat::X3d => "X3D",
            MeshFormat::Vrml => "VRML",
            MeshFormat::Rhino => "Rhino",
            MeshFormat::Amf => "AMF",
            MeshFormat::Off => "OFF",
            MeshFormat::Alembic => "Alembic",
            MeshFormat::Md2 => "MD2",
            MeshFormat::Ms3d => "MilkShape 3D",
            MeshFormat::Vox => "MagicaVoxel",
            MeshFormat::Ac3d => "AC3D",
            MeshFormat::DirectX => "DirectX",
            MeshFormat::Bvh => "BVH",
        }
    }

    /// File extensions (lower case).
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            MeshFormat::Stl => &["stl"],
            MeshFormat::Obj => &["obj"],
            MeshFormat::Ply => &["ply"],
            MeshFormat::Gltf => &["gltf"],
            MeshFormat::Glb => &["glb"],
            MeshFormat::ThreeMf => &["3mf"],
            MeshFormat::Usd => &["usd", "usda", "usdc"],
            MeshFormat::Usdz => &["usdz"],
            MeshFormat::Dae => &["dae"],
            MeshFormat::Zae => &["zae"],
            MeshFormat::ThreeDs => &["3ds"],
            MeshFormat::Lwo => &["lwo"],
            MeshFormat::Fbx => &["fbx"],
            MeshFormat::Ifc => &["ifc", "ifczip"],
            MeshFormat::X3d => &["x3d", "x3dz"],
            MeshFormat::Vrml => &["wrl", "wrz", "vrml", "x3dv", "x3dvz"],
            MeshFormat::Rhino => &["3dm"],
            MeshFormat::Amf => &["amf"],
            MeshFormat::Off => &["off"],
            MeshFormat::Alembic => &["abc"],
            MeshFormat::Md2 => &["md2"],
            MeshFormat::Ms3d => &["ms3d"],
            MeshFormat::Vox => &["vox"],
            MeshFormat::Ac3d => &["ac", "ac3d"],
            MeshFormat::DirectX => &["x"],
            MeshFormat::Bvh => &["bvh"],
        }
    }

    /// Folders above the model's its side files may come from (scene formats kept in asset
    /// libraries).
    pub fn climb(self) -> usize {
        match self {
            MeshFormat::Usd | MeshFormat::Dae => 4,
            _ => 0,
        }
    }

    /// The format of a file named `ext`, if any.
    pub fn from_extension(ext: &str) -> Option<MeshFormat> {
        let ext = ext.to_ascii_lowercase();
        MeshFormat::ALL
            .into_iter()
            .find(|f| f.extensions().contains(&ext.as_str()))
    }

    /// Whether the first bytes are this format's signature. STL, OBJ and glTF JSON have none
    /// that is specific (an STL header is free text, OBJ and JSON are plain text), so they
    /// are chosen by extension; PLY starts `ply`, GLB `glTF`, and 3MF is a ZIP.
    pub fn sniff(head: &[u8]) -> Option<MeshFormat> {
        if head.starts_with(b"ply\n") || head.starts_with(b"ply\r\n") {
            Some(MeshFormat::Ply)
        } else if head.starts_with(b"glTF") {
            Some(MeshFormat::Glb)
        } else if head.starts_with(b"Kaydara FBX Binary") {
            Some(MeshFormat::Fbx)
        } else if head.starts_with(b"#usda") || head.starts_with(b"PXR-USDC") {
            Some(MeshFormat::Usd)
        } else if head.starts_with(b"#VRML ") || head.starts_with(b"#X3D V") {
            Some(MeshFormat::Vrml)
        } else if head.starts_with(b"3D Geometry File Format ") {
            Some(MeshFormat::Rhino)
        } else if off::sniff(&head[..head.len().min(256)]) {
            Some(MeshFormat::Off)
        } else if alembic::probe(head) {
            Some(MeshFormat::Alembic)
        } else if games::is_md2(head) {
            Some(MeshFormat::Md2)
        } else if games::is_ms3d(head) {
            Some(MeshFormat::Ms3d)
        } else if games::is_vox(head) {
            Some(MeshFormat::Vox)
        } else if scenes::is_ac3d(head) {
            Some(MeshFormat::Ac3d)
        } else if scenes::is_x(head) {
            Some(MeshFormat::DirectX)
        } else if scenes::is_bvh(head) {
            Some(MeshFormat::Bvh)
        } else {
            None
        }
    }
}

/// Length unit of the coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Unit {
    /// The file does not say (STL, OBJ, PLY).
    #[default]
    Unknown,
    Micron,
    Millimetre,
    Centimetre,
    Metre,
    Inch,
    Foot,
}

impl Unit {
    /// The symbol, `None` when unknown.
    pub fn symbol(self) -> Option<&'static str> {
        Some(match self {
            Unit::Unknown => return None,
            Unit::Micron => "µm",
            Unit::Millimetre => "mm",
            Unit::Centimetre => "cm",
            Unit::Metre => "m",
            Unit::Inch => "in",
            Unit::Foot => "ft",
        })
    }

    /// Metres per unit, `None` when unknown.
    pub fn metres(self) -> Option<f64> {
        Some(match self {
            Unit::Unknown => return None,
            Unit::Micron => 1e-6,
            Unit::Millimetre => 1e-3,
            Unit::Centimetre => 1e-2,
            Unit::Metre => 1.0,
            Unit::Inch => 0.0254,
            Unit::Foot => 0.3048,
        })
    }
}

/// Which model axis points up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Up {
    /// CAD and 3D printing (STL, 3MF, PLY scans).
    #[default]
    Z,
    /// glTF (by its specification) and most OBJ exporters.
    Y,
}

/// An axis-aligned box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds::EMPTY
    }
}

impl Bounds {
    pub const EMPTY: Bounds = Bounds {
        min: [f32::INFINITY; 3],
        max: [f32::NEG_INFINITY; 3],
    };

    pub fn is_empty(&self) -> bool {
        (0..3).any(|c| self.min[c].is_nan() || self.max[c].is_nan() || self.min[c] > self.max[c])
    }

    pub fn add(&mut self, p: [f32; 3]) {
        for ((lo, hi), v) in self.min.iter_mut().zip(&mut self.max).zip(p) {
            *lo = lo.min(v);
            *hi = hi.max(v);
        }
    }

    pub fn union(&self, o: &Bounds) -> Bounds {
        Bounds {
            min: [0, 1, 2].map(|c| self.min[c].min(o.min[c])),
            max: [0, 1, 2].map(|c| self.max[c].max(o.max[c])),
        }
    }

    pub fn size(&self) -> [f32; 3] {
        if self.is_empty() {
            return [0.0; 3];
        }
        [0, 1, 2].map(|c| self.max[c] - self.min[c])
    }

    pub fn centre(&self) -> [f32; 3] {
        if self.is_empty() {
            return [0.0; 3];
        }
        [0, 1, 2].map(|c| (self.min[c] + self.max[c]) / 2.0)
    }

    /// Half the diagonal.
    pub fn radius(&self) -> f32 {
        let s = self.size();
        (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt() / 2.0
    }
}

/// An encoded texture image from the file or beside it.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageData {
    /// The image's encoded bytes (PNG, JPEG, …) and the MIME type the file gives, if any.
    Bytes {
        bytes: Vec<u8>,
        mime: Option<String>,
    },
    /// Named by the file but not read: missing, outside the model's folder, a URL, or over
    /// the caps. The reason is for the info panel.
    Missing { name: String, reason: String },
}

/// A surface's look.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    /// Linear RGBA multiplier (glTF `baseColorFactor`, MTL `Kd` and `d`, 3MF `displaycolor`).
    pub base_color: [f32; 4],
    /// Index into [`Mesh::images`] of the base-colour texture (sRGB), sampled with
    /// [`Mesh::uvs`].
    pub texture: Option<u32>,
}

impl Default for Material {
    fn default() -> Self {
        Material {
            name: String::new(),
            base_color: [1.0; 4],
            texture: None,
        }
    }
}

/// A run of triangles drawn with one material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    /// First triangle (index into [`Mesh::indices`] / 3).
    pub first: u32,
    pub count: u32,
    /// Index into [`Mesh::materials`]; `None`: the default look.
    pub material: Option<u32>,
}

/// One indexed triangle mesh: the whole file, with node and build transforms applied.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub format: Option<MeshFormat>,
    pub positions: Vec<[f32; 3]>,
    /// Unit normals, one per position; computed (area-weighted) where the file has none.
    pub normals: Vec<[f32; 3]>,
    /// sRGB RGBA per position, when the file has vertex colours.
    pub colors: Option<Vec<[u8; 4]>>,
    /// Texture coordinates per position (glTF orientation: v down), when the file has them.
    pub uvs: Option<Vec<[f32; 2]>>,
    /// Three per triangle, counter-clockwise front faces.
    pub indices: Vec<u32>,
    /// Triangles grouped by material, in order, covering every triangle.
    pub parts: Vec<Part>,
    pub materials: Vec<Material>,
    pub images: Vec<ImageData>,
    pub unit: Unit,
    pub up: Up,
    /// Of the positions the triangles use.
    pub bounds: Bounds,
    /// The file's normals are facet normals or it has none and is faceted by nature (STL,
    /// 3MF, glTF primitives without normals): shade each triangle flat.
    pub flat: bool,
    /// The file had normals for every vertex.
    pub file_normals: bool,
    /// What was skipped or approximated (unsupported primitives, missing textures, dropped
    /// triangles), for the info panel.
    pub notes: Vec<String>,
    /// The file's own name for the model or its generator, when it gives one.
    pub title: Option<String>,
    pub generator: Option<String>,
}

impl Mesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Bytes held.
    pub fn memory_bytes(&self) -> usize {
        self.positions.len() * 24
            + self.colors.as_ref().map_or(0, |c| c.len() * 4)
            + self.uvs.as_ref().map_or(0, |u| u.len() * 8)
            + self.indices.len() * 4
            + self
                .images
                .iter()
                .map(|i| match i {
                    ImageData::Bytes { bytes, .. } => bytes.len(),
                    ImageData::Missing { .. } => 0,
                })
                .sum::<usize>()
    }
}

/// Caps on what a file may make us allocate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_triangles: usize,
    pub max_vertices: usize,
    /// Side files and embedded images, each.
    pub max_side_file: usize,
    pub max_images: usize,
    /// Uncompressed bytes read from a 3MF package in all.
    pub max_unpacked: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_triangles: 60_000_000,
            max_vertices: 60_000_000,
            max_side_file: 1 << 30,
            max_images: 64,
            max_unpacked: 4 << 30,
        }
    }
}

/// Reads a mesh in `format` from `bytes`, with side files through `resolver`.
pub fn read(
    format: MeshFormat,
    bytes: &[u8],
    resolver: &dyn Resolver,
    limits: &Limits,
) -> Result<Mesh> {
    let mut m = match format {
        MeshFormat::Stl => stl::read(bytes, limits)?,
        MeshFormat::Obj => obj::read(bytes, resolver, limits)?,
        MeshFormat::Ply => ply::read(bytes, resolver, limits)?,
        MeshFormat::Gltf => gltf::read_gltf(bytes, resolver, limits)?,
        MeshFormat::Glb => gltf::read_glb(bytes, resolver, limits)?,
        MeshFormat::ThreeMf => threemf::read(bytes, limits)?,
        MeshFormat::Usd => usd::read(bytes, resolver, limits)?,
        MeshFormat::Usdz => usd::read_usdz(bytes, limits)?,
        MeshFormat::Dae => collada::read(bytes, resolver, limits)?,
        MeshFormat::Zae => collada::read_zae(bytes, limits)?,
        MeshFormat::ThreeDs => three_ds::read(bytes, resolver, limits)?,
        MeshFormat::Lwo => lwo::read(bytes, resolver, limits)?,
        MeshFormat::Fbx => fbx::read(bytes, resolver, limits)?,
        MeshFormat::Ifc => ifc::read(bytes, limits, None)?.mesh,
        MeshFormat::X3d => x3d::read_xml(bytes, resolver, limits)?,
        MeshFormat::Vrml => x3d::read_classic(bytes, resolver, limits)?,
        MeshFormat::Rhino => rhino::read(bytes, limits)?,
        MeshFormat::Amf => amf::read(bytes, limits)?,
        MeshFormat::Off => off::read(bytes, limits)?,
        MeshFormat::Alembic => alembic::read(bytes, limits)?,
        MeshFormat::Md2 => games::read_md2(bytes, limits)?,
        MeshFormat::Ms3d => games::read_ms3d(bytes, limits)?,
        MeshFormat::Vox => games::read_vox(bytes, limits)?,
        MeshFormat::Ac3d => scenes::read_ac3d(bytes, limits)?,
        MeshFormat::DirectX => scenes::read_x(bytes, limits)?,
        MeshFormat::Bvh => scenes::read_bvh(bytes, limits)?,
    };
    m.format = Some(format);
    if m.indices.is_empty() {
        return Err(Error::Empty);
    }
    Ok(m)
}

/// A mesh from bare triangles a reader elsewhere produced (neuroimaging surfaces): three
/// indices per triangle into `positions`, optional sRGB colours per position. Caps apply,
/// invalid triangles are dropped, normals are computed smooth, bounds found; `format` stays
/// `None`.
pub fn from_triangles(
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
    colors: Option<Vec<[u8; 4]>>,
    limits: &Limits,
) -> Result<Mesh> {
    if positions.len() > limits.max_vertices {
        return Err(Error::TooLarge(format!(
            "more than {} vertices",
            limits.max_vertices
        )));
    }
    if indices.len() / 3 > limits.max_triangles {
        return Err(Error::TooLarge(format!(
            "more than {} triangles",
            limits.max_triangles
        )));
    }
    if colors.as_ref().is_some_and(|c| c.len() != positions.len()) {
        return bad("one colour per vertex needed");
    }
    let mut b = builder::Builder::new(limits);
    b.positions = positions;
    b.indices = indices;
    b.colors = colors;
    b.finish()
}

/// Reads the file at `path`: the format from its signature, else its extension; side files
/// from its folder.
pub fn read_path(path: &Path, limits: &Limits) -> Result<Mesh> {
    let bytes = std::fs::read(path).map_err(|e| Error::Malformed(e.to_string()))?;
    let format = MeshFormat::sniff(&bytes)
        .or_else(|| {
            path.extension()
                .and_then(|e| e.to_str())
                .and_then(MeshFormat::from_extension)
        })
        .ok_or_else(|| Error::Unsupported("not a mesh file".into()))?;
    let resolver = DirResolver::for_file(path).climbing(format.climb());
    read(format, &bytes, &resolver, limits)
}
