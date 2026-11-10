//! CIFTI-2 (`.dscalar.nii`, `.dlabel.nii`, `.dtseries.nii`, `.pscalar.nii`, `.dconn.nii`,
//! …), from the CIFTI-2 specification.
//!
//! A NIfTI-2 file whose matrix (dimensions 5 and 6 of the NIfTI header; the spatial ones
//! are 1) is described by an XML extension (code 32). Each matrix dimension has an index
//! map:
//!
//! - brain models: each column is a vertex of a named surface (`CORTEX_LEFT`…, with the
//!   surface's vertex count) or a voxel of a subcortical structure in one volume space;
//! - parcels: each column is a named set of vertices and voxels;
//! - scalars and labels: named maps (labels with a label table each);
//! - series: time points (start, step and unit).
//!
//! The values are stored with the first CIFTI dimension (NIfTI dimension 5) varying
//! fastest: a row (one index of dimension 1) is contiguous. Dense connectivity files
//! (brain models along both dimensions) are often several gigabytes, so they are read a
//! row at a time ([`Cifti::row`]) from the memory-mapped file.

use std::borrow::Cow;

use crate::gifti::{self, Label};
use crate::xml::{self, Element};
use crate::{Error, Result};

/// The NIfTI extension code of CIFTI-2's XML.
pub const ECODE_CIFTI2: i32 = 32;
/// CIFTI-1's code (not read).
pub const ECODE_CIFTI1: i32 = 30;
/// Largest XML extension accepted.
const MAX_XML: usize = 1 << 28;
/// Largest gzip-compressed CIFTI file accepted, inflated.
pub const MAX_INFLATED: usize = 1 << 31;

/// The NIfTI-2 header fields a CIFTI file uses.
#[derive(Clone, Debug, PartialEq)]
pub struct NiftiInfo {
    pub big_endian: bool,
    /// NIfTI type code (16 float32, 8 int32…).
    pub datatype: i16,
    /// NIfTI `dim[0..8]`.
    pub dim: [i64; 8],
    pub vox_offset: usize,
    /// Value = stored × slope + intercept (identity when unset).
    pub scale: (f64, f64),
    pub intent_code: i32,
    pub intent_name: String,
    pub description: String,
}

impl NiftiInfo {
    /// Bytes per sample, for the data types CIFTI files use.
    pub fn sample_size(&self) -> Option<usize> {
        Some(match self.datatype {
            2 | 256 => 1,
            4 | 512 => 2,
            8 | 16 | 768 => 4,
            64 | 1024 | 1280 => 8,
            _ => return None,
        })
    }

    /// The CIFTI intent's name (dense scalars, labels, series…).
    pub fn intent(&self) -> &'static str {
        match self.intent_code {
            3000 => "ConnUnknown",
            3001 => "ConnDense",
            3002 => "ConnDenseSeries",
            3003 => "ConnParcels",
            3004 => "ConnParcelSries",
            3006 => "ConnDenseScalar",
            3007 => "ConnDenseLabel",
            3008 => "ConnParcelScalr",
            3009 => "ConnParcelDense",
            3010 => "ConnDenseParcel",
            3011 => "ConnPPSr",
            3012 => "ConnPPSc",
            _ => "",
        }
    }
}

/// A voxel grid: dimensions and voxel (i, j, k) to millimetres (RAS), row-major 4 × 4.
#[derive(Clone, Debug, PartialEq)]
pub struct VolumeSpace {
    pub dims: [usize; 3],
    pub affine: [[f64; 4]; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub enum Model {
    /// Vertices of a surface with `vertices_total` vertices.
    Surface {
        vertices_total: usize,
        vertices: Vec<u32>,
    },
    /// Voxels (i, j, k) in the map's volume space.
    Voxels(Vec<[u32; 3]>),
}

/// A brain model: columns `offset..offset + count` of the matrix dimension.
#[derive(Clone, Debug, PartialEq)]
pub struct BrainModel {
    pub offset: usize,
    pub count: usize,
    /// "CIFTI_STRUCTURE_CORTEX_LEFT"…
    pub structure: String,
    pub model: Model,
}

/// A parcel: a named set of vertices (per surface structure) and voxels.
#[derive(Clone, Debug, PartialEq)]
pub struct Parcel {
    pub name: String,
    pub vertices: Vec<(String, Vec<u32>)>,
    pub voxels: Vec<[u32; 3]>,
}

/// One named map (a scalar or label map) of a scalars or labels dimension.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedMap {
    pub name: String,
    pub meta: Vec<(String, String)>,
    /// The label table (label maps only).
    pub labels: Vec<Label>,
}

/// What the entries along one matrix dimension are.
#[derive(Clone, Debug, PartialEq)]
pub enum IndexMap {
    BrainModels {
        volume: Option<VolumeSpace>,
        models: Vec<BrainModel>,
    },
    Parcels {
        volume: Option<VolumeSpace>,
        /// Surface structures and their vertex counts.
        surfaces: Vec<(String, usize)>,
        parcels: Vec<Parcel>,
    },
    Scalars(Vec<NamedMap>),
    Labels(Vec<NamedMap>),
    Series {
        start: f64,
        step: f64,
        /// "SECOND", "HERTZ", "METER", "RADIAN".
        unit: String,
        exponent: i32,
        count: usize,
    },
}

impl IndexMap {
    /// The index type's short name ("brain models", "scalars"…), for the info panel.
    pub fn kind(&self) -> &'static str {
        match self {
            IndexMap::BrainModels { .. } => "brain-models",
            IndexMap::Parcels { .. } => "parcels",
            IndexMap::Scalars(_) => "scalars",
            IndexMap::Labels(_) => "labels",
            IndexMap::Series { .. } => "series",
        }
    }

    fn len(&self) -> Option<usize> {
        match self {
            IndexMap::BrainModels { models, .. } => models
                .iter()
                .map(|m| m.count)
                .try_fold(0usize, |a, c| a.checked_add(c)),
            IndexMap::Parcels { parcels, .. } => Some(parcels.len()),
            IndexMap::Scalars(m) | IndexMap::Labels(m) => Some(m.len()),
            IndexMap::Series { count, .. } => Some(*count),
        }
    }

    fn is_spatial(&self) -> bool {
        matches!(
            self,
            IndexMap::BrainModels { .. } | IndexMap::Parcels { .. }
        )
    }
}

/// A CIFTI-2 file's header and XML.
#[derive(Clone, Debug, PartialEq)]
pub struct Cifti {
    pub nifti: NiftiInfo,
    /// The XML version ("2").
    pub version: String,
    pub meta: Vec<(String, String)>,
    /// Lengths of the two matrix dimensions (NIfTI `dim[5]`, `dim[6]`).
    pub shape: [usize; 2],
    /// The index map of each dimension.
    pub maps: [IndexMap; 2],
}

/// Whether `data` (a file's first bytes, at least 552) is NIfTI-2 with a CIFTI extension.
pub fn sniff(data: &[u8]) -> bool {
    let Some(big) = nifti2_endian(data) else {
        return false;
    };
    data.len() >= 552
        && data[540] != 0
        && i32_at(data, 548, big) == ECODE_CIFTI2
        && i32_at(data, 544, big) >= 16
}

fn nifti2_endian(data: &[u8]) -> Option<bool> {
    if data.len() < 540 || &data[4..8] != b"n+2\0" {
        return None;
    }
    match data[..4] {
        [0x1c, 0x02, 0, 0] => Some(false),
        [0, 0, 0x02, 0x1c] => Some(true),
        _ => None,
    }
}

fn i32_at(b: &[u8], at: usize, big: bool) -> i32 {
    let a = [b[at], b[at + 1], b[at + 2], b[at + 3]];
    if big {
        i32::from_be_bytes(a)
    } else {
        i32::from_le_bytes(a)
    }
}

fn i64_at(b: &[u8], at: usize, big: bool) -> i64 {
    let a: [u8; 8] = b[at..at + 8].try_into().unwrap_or([0; 8]);
    if big {
        i64::from_be_bytes(a)
    } else {
        i64::from_le_bytes(a)
    }
}

fn f64_at(b: &[u8], at: usize, big: bool) -> f64 {
    f64::from_bits(i64_at(b, at, big) as u64)
}

/// The file's bytes, inflated when it is gzip-compressed (`.nii.gz`).
pub fn inflate(data: &[u8]) -> Result<Cow<'_, [u8]>> {
    if data.starts_with(&[0x1f, 0x8b]) {
        specular_png::inflate_gzip(data, MAX_INFLATED)
            .map(Cow::Owned)
            .map_err(|e| Error::Malformed(format!("gzip stream: {e}")))
    } else {
        Ok(Cow::Borrowed(data))
    }
}

/// NIfTI extensions: (code, bytes) each.
pub type Extensions<'a> = Vec<(i32, &'a [u8])>;

/// The NIfTI-2 header and the extensions after it.
pub fn nifti2(data: &[u8]) -> Result<(NiftiInfo, Extensions<'_>)> {
    let big = nifti2_endian(data).ok_or(Error::NotRecognised)?;
    let mut dim = [0i64; 8];
    for (i, d) in dim.iter_mut().enumerate() {
        *d = i64_at(data, 16 + 8 * i, big);
    }
    let vox_offset = usize::try_from(i64_at(data, 168, big))
        .map_err(|_| Error::Malformed("NIfTI vox_offset".into()))?;
    let (slope, inter) = (f64_at(data, 176, big), f64_at(data, 184, big));
    let scale = if slope.is_finite() && slope != 0.0 && inter.is_finite() {
        (slope, inter)
    } else {
        (1.0, 0.0)
    };
    let info = NiftiInfo {
        big_endian: big,
        datatype: i16::from_le_bytes(if big {
            [data[13], data[12]]
        } else {
            [data[12], data[13]]
        }),
        dim,
        vox_offset,
        scale,
        intent_code: i32_at(data, 504, big),
        intent_name: crate::cstr(&data[508..524]),
        description: crate::cstr(&data[240..320]),
    };
    let mut ext = Vec::new();
    if data.len() > 544 && data[540] != 0 {
        let end = vox_offset.min(data.len());
        let mut at = 544usize;
        while at + 8 <= end {
            let size = i32_at(data, at, big);
            let code = i32_at(data, at + 4, big);
            let Ok(size) = usize::try_from(size) else {
                break;
            };
            if size < 8 || at + size > end {
                break;
            }
            ext.push((code, &data[at + 8..at + size]));
            at += size;
        }
    }
    Ok((info, ext))
}

fn ints(text: &[u8]) -> impl Iterator<Item = Option<u32>> + '_ {
    text.split(|c| c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| std::str::from_utf8(t).ok()?.parse().ok())
}

fn volume_space(el: &Element, src: &[u8]) -> Result<VolumeSpace> {
    let bad = |m: &str| Error::Malformed(format!("CIFTI volume: {m}"));
    let dims: Vec<usize> = el
        .attr("VolumeDimensions")
        .unwrap_or("")
        .split(',')
        .filter_map(|d| d.trim().parse().ok())
        .collect();
    let [a, b, c] = dims[..] else {
        return Err(bad("VolumeDimensions"));
    };
    if [a, b, c].iter().any(|&d| d == 0 || d > 1 << 16) {
        return Err(bad("VolumeDimensions"));
    }
    let t = el
        .child("TransformationMatrixVoxelIndicesIJKtoXYZ")
        .ok_or_else(|| bad("no transform"))?;
    let exp: i32 = t
        .attr("MeterExponent")
        .and_then(|e| e.trim().parse().ok())
        .unwrap_or(-3);
    let v: Vec<f64> = String::from_utf8_lossy(&t.bytes(src))
        .split_whitespace()
        .filter_map(|x| x.parse().ok())
        .collect();
    if v.len() != 16 || v.iter().any(|x| !x.is_finite()) || !(-12..=3).contains(&exp) {
        return Err(bad("transform"));
    }
    // To millimetres.
    let k = 10f64.powi(exp + 3);
    let mut affine = [[0.0; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            affine[r][c] = v[r * 4 + c] * if r < 3 { k } else { 1.0 };
        }
    }
    Ok(VolumeSpace {
        dims: [a, b, c],
        affine,
    })
}

fn voxels(
    el: &Element,
    src: &[u8],
    count: usize,
    vol: Option<&VolumeSpace>,
) -> Result<Vec<[u32; 3]>> {
    let bad = || Error::Malformed("CIFTI voxel indices".into());
    let vol = vol.ok_or_else(|| Error::Malformed("CIFTI voxels without a volume".into()))?;
    let text = el.bytes(src);
    if count > text.len() / 6 + 1 {
        return Err(bad());
    }
    let v: Vec<u32> = ints(&text)
        .take(3 * count + 1)
        .collect::<Option<_>>()
        .ok_or_else(bad)?;
    if v.len() != 3 * count {
        return Err(bad());
    }
    let out: Vec<[u32; 3]> = v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    if out
        .iter()
        .any(|p| (0..3).any(|a| p[a] as usize >= vol.dims[a]))
    {
        return Err(Error::Malformed("CIFTI voxel outside its volume".into()));
    }
    Ok(out)
}

fn vertices(text: &[u8], count: usize, total: usize) -> Result<Vec<u32>> {
    let bad = || Error::Malformed("CIFTI vertex indices".into());
    if count > text.len() / 2 + 1 {
        return Err(bad());
    }
    let v: Vec<u32> = ints(text)
        .take(count + 1)
        .collect::<Option<_>>()
        .ok_or_else(bad)?;
    if v.len() != count || v.iter().any(|&i| i as usize >= total) {
        return Err(bad());
    }
    Ok(v)
}

fn usize_attr(el: &Element, name: &str) -> Result<usize> {
    el.attr(name)
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| Error::Malformed(format!("CIFTI {} {name}", el.name)))
}

/// `max`: the most entries a dimension can have (each takes a byte of the file at least).
fn index_map(el: &Element, src: &[u8], max: usize) -> Result<IndexMap> {
    let ty = el.attr("IndicesMapToDataType").unwrap_or("").trim();
    let volume = el
        .child("Volume")
        .map(|v| volume_space(v, src))
        .transpose()?;
    Ok(match ty {
        "CIFTI_INDEX_TYPE_BRAIN_MODELS" => {
            let mut models = Vec::new();
            let mut implied = 0usize;
            for b in el.all("BrainModel") {
                let offset = usize_attr(b, "IndexOffset")?;
                let count = usize_attr(b, "IndexCount")?;
                // Checked before building the model: implicit vertex lists are allocated
                // from the counts alone.
                implied = implied.saturating_add(count);
                if implied > max {
                    return Err(Error::Truncated("CIFTI matrix".into()));
                }
                let structure = b.attr("BrainStructure").unwrap_or("").trim().to_owned();
                let model = match b.attr("ModelType").map(str::trim) {
                    Some("CIFTI_MODEL_TYPE_SURFACE") => {
                        let total = usize_attr(b, "SurfaceNumberOfVertices")?;
                        if total > 1 << 26 || count > total {
                            return Err(Error::Malformed("CIFTI surface size".into()));
                        }
                        let vertices = match b.child("VertexIndices") {
                            Some(v) => vertices(&v.bytes(src), count, total)?,
                            // Optional when every vertex is used, in order.
                            None if count == total => (0..total as u32).collect(),
                            None => return Err(Error::Malformed("CIFTI vertex indices".into())),
                        };
                        Model::Surface {
                            vertices_total: total,
                            vertices,
                        }
                    }
                    Some("CIFTI_MODEL_TYPE_VOXELS") => {
                        let ijk = b
                            .child("VoxelIndicesIJK")
                            .ok_or_else(|| Error::Malformed("CIFTI voxel indices".into()))?;
                        Model::Voxels(voxels(ijk, src, count, volume.as_ref())?)
                    }
                    t => {
                        return Err(Error::Malformed(format!(
                            "CIFTI model type {}",
                            t.unwrap_or("(none)")
                        )));
                    }
                };
                models.push(BrainModel {
                    offset,
                    count,
                    structure,
                    model,
                });
            }
            // The models tile the dimension in order.
            let mut at = 0usize;
            for m in &models {
                if m.offset != at {
                    return Err(Error::Malformed("CIFTI brain model offsets".into()));
                }
                at = at.checked_add(m.count).ok_or(Error::TooLarge)?;
            }
            IndexMap::BrainModels { volume, models }
        }
        "CIFTI_INDEX_TYPE_PARCELS" => {
            let surfaces: Vec<(String, usize)> = el
                .all("Surface")
                .map(|s| {
                    Ok((
                        s.attr("BrainStructure").unwrap_or("").trim().to_owned(),
                        usize_attr(s, "SurfaceNumberOfVertices")?,
                    ))
                })
                .collect::<Result<_>>()?;
            let mut parcels = Vec::new();
            for p in el.all("Parcel") {
                let mut vs = Vec::new();
                for v in p.all("Vertices") {
                    let st = v.attr("BrainStructure").unwrap_or("").trim().to_owned();
                    let total = surfaces
                        .iter()
                        .find(|s| s.0 == st)
                        .map(|s| s.1)
                        .ok_or_else(|| Error::Malformed("CIFTI parcel surface".into()))?;
                    let text = v.bytes(src);
                    let n = ints(&text).count();
                    vs.push((st, vertices(&text, n, total)?));
                }
                let voxels = match p.child("VoxelIndicesIJK") {
                    Some(ijk) => {
                        let n = ints(&ijk.bytes(src)).count() / 3;
                        voxels(ijk, src, n, volume.as_ref())?
                    }
                    None => Vec::new(),
                };
                parcels.push(Parcel {
                    name: p.attr("Name").unwrap_or("").to_owned(),
                    vertices: vs,
                    voxels,
                });
            }
            IndexMap::Parcels {
                volume,
                surfaces,
                parcels,
            }
        }
        "CIFTI_INDEX_TYPE_SCALARS" | "CIFTI_INDEX_TYPE_LABELS" => {
            let maps = el
                .all("NamedMap")
                .map(|m| NamedMap {
                    name: m.child_text("MapName", src).unwrap_or_default(),
                    meta: m
                        .child("MetaData")
                        .map(|md| {
                            md.all("MD")
                                .filter_map(|e| {
                                    Some((
                                        e.child("Name")?.text(src),
                                        e.child("Value").map(|v| v.text(src)).unwrap_or_default(),
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    labels: m
                        .child("LabelTable")
                        .map(|t| gifti::label_table(t, src))
                        .unwrap_or_default(),
                })
                .collect();
            if ty == "CIFTI_INDEX_TYPE_LABELS" {
                IndexMap::Labels(maps)
            } else {
                IndexMap::Scalars(maps)
            }
        }
        "CIFTI_INDEX_TYPE_SERIES" => {
            let f = |n: &str| {
                el.attr(n)
                    .and_then(|v| v.trim().parse::<f64>().ok())
                    .filter(|v| v.is_finite())
            };
            IndexMap::Series {
                start: f("SeriesStart").unwrap_or(0.0),
                step: f("SeriesStep").unwrap_or(1.0),
                unit: el.attr("SeriesUnit").unwrap_or("").trim().to_owned(),
                exponent: el
                    .attr("SeriesExponent")
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0),
                count: usize_attr(el, "NumberOfSeriesPoints")?,
            }
        }
        t => {
            return Err(Error::Unsupported(format!(
                "CIFTI index type {} is not supported",
                if t.is_empty() { "(none)" } else { t }
            )));
        }
    })
}

/// Reads a CIFTI-2 file's header and XML from its bytes (inflated, see [`inflate`]).
pub fn parse(data: &[u8]) -> Result<Cifti> {
    let (nifti, ext) = nifti2(data)?;
    let Some(&(_, xml_bytes)) = ext.iter().find(|e| e.0 == ECODE_CIFTI2) else {
        if ext.iter().any(|e| e.0 == ECODE_CIFTI1) {
            return Err(Error::Unsupported("CIFTI-1 files are not supported".into()));
        }
        return Err(Error::NotRecognised);
    };
    if xml_bytes.len() > MAX_XML {
        return Err(Error::TooLarge);
    }
    // The extension is padded with NULs to a multiple of 16.
    let end = xml_bytes.iter().rposition(|&c| c != 0).map_or(0, |e| e + 1);
    let src = &xml_bytes[..end];
    let root = xml::parse(src)?;
    if root.name != "CIFTI" {
        return Err(Error::Malformed("CIFTI XML root".into()));
    }
    let version = root.attr("Version").unwrap_or("").trim().to_owned();
    if version.starts_with('1') {
        return Err(Error::Unsupported("CIFTI-1 files are not supported".into()));
    }
    let matrix = root
        .child("Matrix")
        .ok_or_else(|| Error::Malformed("CIFTI XML: no Matrix".into()))?;
    let mut maps: [Option<IndexMap>; 2] = [None, None];
    for m in matrix.all("MatrixIndicesMap") {
        let map = index_map(m, src, data.len())?;
        for d in m.attr("AppliesToMatrixDimension").unwrap_or("").split(',') {
            let d: usize = d
                .trim()
                .parse()
                .map_err(|_| Error::Malformed("CIFTI AppliesToMatrixDimension".into()))?;
            if d > 1 {
                return Err(Error::Unsupported(
                    "CIFTI matrices of more than two dimensions are not supported".into(),
                ));
            }
            maps[d] = Some(map.clone());
        }
    }
    let [Some(m0), Some(m1)] = maps else {
        return Err(Error::Malformed(
            "CIFTI XML: a dimension without an index map".into(),
        ));
    };
    let d = &nifti.dim;
    if d[0] < 6 || d[1..5].iter().any(|&x| x != 1) || d[5] < 1 || d[6] < 1 || (d[0] > 6 && d[7] > 1)
    {
        return Err(Error::Malformed("CIFTI NIfTI dimensions".into()));
    }
    let shape = [d[5] as usize, d[6] as usize];
    for (i, m) in [&m0, &m1].into_iter().enumerate() {
        if m.len() != Some(shape[i]) {
            return Err(Error::Malformed(format!(
                "CIFTI dimension {i}: {} entries in the XML, {} in the matrix",
                m.len().unwrap_or(0),
                shape[i]
            )));
        }
    }
    let size = nifti.sample_size().ok_or_else(|| {
        Error::Unsupported(format!(
            "CIFTI values of NIfTI type {} are not supported",
            nifti.datatype
        ))
    })?;
    let need = shape[0]
        .checked_mul(shape[1])
        .and_then(|n| n.checked_mul(size))
        .and_then(|n| n.checked_add(nifti.vox_offset))
        .ok_or(Error::TooLarge)?;
    if need > data.len() {
        return Err(Error::Truncated("CIFTI matrix".into()));
    }
    let meta = matrix
        .child("MetaData")
        .map(|md| {
            md.all("MD")
                .filter_map(|e| {
                    Some((
                        e.child("Name")?.text(src),
                        e.child("Value").map(|v| v.text(src)).unwrap_or_default(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Cifti {
        nifti,
        version,
        meta,
        shape,
        maps: [m0, m1],
    })
}

impl Cifti {
    /// The dimension whose entries are places in the brain (brain models or parcels): the
    /// columns of a map. The other one is the maps (or time points).
    pub fn spatial_dim(&self) -> usize {
        if self.maps[1].is_spatial() || !self.maps[0].is_spatial() {
            1
        } else {
            0
        }
    }

    /// The dimension that indexes maps.
    pub fn map_dim(&self) -> usize {
        1 - self.spatial_dim()
    }

    pub fn map_count(&self) -> usize {
        self.shape[self.map_dim()]
    }

    /// The name of map `i` (named maps), or its time ("t = 1.6 s") for a series, else its
    /// number.
    pub fn map_name(&self, i: usize) -> String {
        match &self.maps[self.map_dim()] {
            IndexMap::Scalars(m) | IndexMap::Labels(m) => m
                .get(i)
                .map(|m| m.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| format!("#{}", i + 1)),
            IndexMap::Series {
                start,
                step,
                unit,
                exponent,
                ..
            } => {
                let t = (start + step * i as f64) * 10f64.powi(*exponent);
                let u = match unit.as_str() {
                    "SECOND" => " s",
                    "HERTZ" => " Hz",
                    "METER" => " m",
                    "RADIAN" => " rad",
                    _ => "",
                };
                format!("{t:.4}{u}")
            }
            IndexMap::Parcels { parcels, .. } => {
                parcels.get(i).map(|p| p.name.clone()).unwrap_or_default()
            }
            IndexMap::BrainModels { .. } => format!("#{}", i + 1),
        }
    }

    /// The label table of map `i`, for label maps.
    pub fn labels(&self, i: usize) -> Option<&[Label]> {
        match &self.maps[self.map_dim()] {
            IndexMap::Labels(m) => m.get(i).map(|m| m.labels.as_slice()),
            _ => None,
        }
    }

    pub fn is_labels(&self) -> bool {
        matches!(self.maps[self.map_dim()], IndexMap::Labels(_))
    }

    /// The brain models (empty for parcels).
    pub fn models(&self) -> &[BrainModel] {
        match &self.maps[self.spatial_dim()] {
            IndexMap::BrainModels { models, .. } => models,
            _ => &[],
        }
    }

    /// The volume space of the voxel models or parcels.
    pub fn volume(&self) -> Option<&VolumeSpace> {
        match &self.maps[self.spatial_dim()] {
            IndexMap::BrainModels { volume, .. } | IndexMap::Parcels { volume, .. } => {
                volume.as_ref()
            }
            _ => None,
        }
    }

    /// Surface structures and their vertex counts, in file order.
    pub fn surfaces(&self) -> Vec<(String, usize)> {
        match &self.maps[self.spatial_dim()] {
            IndexMap::BrainModels { models, .. } => models
                .iter()
                .filter_map(|m| match m.model {
                    Model::Surface { vertices_total, .. } => {
                        Some((m.structure.clone(), vertices_total))
                    }
                    Model::Voxels(_) => None,
                })
                .collect(),
            IndexMap::Parcels { surfaces, .. } => surfaces.clone(),
            _ => Vec::new(),
        }
    }

    /// Voxels in voxel models (or parcels).
    pub fn voxel_count(&self) -> usize {
        match &self.maps[self.spatial_dim()] {
            IndexMap::BrainModels { models, .. } => models
                .iter()
                .map(|m| match &m.model {
                    Model::Voxels(v) => v.len(),
                    Model::Surface { .. } => 0,
                })
                .sum(),
            IndexMap::Parcels { parcels, .. } => parcels.iter().map(|p| p.voxels.len()).sum(),
            _ => 0,
        }
    }

    /// The values of map `i` (scaled), one per brainordinate (or parcel), from the file's
    /// bytes `data` (inflated).
    pub fn map(&self, data: &[u8], i: usize) -> Result<Vec<f32>> {
        let md = self.map_dim();
        if i >= self.shape[md] {
            return Err(Error::Malformed("no such CIFTI map".into()));
        }
        let size = self.nifti.sample_size().ok_or(Error::TooLarge)?;
        let n = self.shape[1 - md];
        let (slope, inter) = self.nifti.scale;
        let big = self.nifti.big_endian;
        let mut out = Vec::with_capacity(n);
        for c in 0..n {
            let (r0, r1) = if md == 0 { (i, c) } else { (c, i) };
            // Dimension 0 fastest.
            let idx = r1 * self.shape[0] + r0;
            let at = self.nifti.vox_offset + idx * size;
            let b = data
                .get(at..at + size)
                .ok_or_else(|| Error::Truncated("CIFTI matrix".into()))?;
            let v = sample(b, self.nifti.datatype, big);
            out.push((v * slope + inter) as f32);
        }
        Ok(out)
    }

    /// Both dimensions are brain models: a dense connectivity matrix (`.dconn.nii`). Each
    /// row (an entry of dimension 1, the seed) holds its connectivity to every entry of
    /// dimension 0; see [`Cifti::row`].
    pub fn is_dense_connectivity(&self) -> bool {
        matches!(
            (&self.maps[0], &self.maps[1]),
            (IndexMap::BrainModels { .. }, IndexMap::BrainModels { .. })
        )
    }

    /// The brain models along dimension `d` (empty unless it is a brain models dimension).
    pub fn models_of(&self, d: usize) -> &[BrainModel] {
        match self.maps.get(d) {
            Some(IndexMap::BrainModels { models, .. }) => models,
            _ => &[],
        }
    }

    /// The volume space of dimension `d`'s voxel models or parcels.
    pub fn volume_of(&self, d: usize) -> Option<&VolumeSpace> {
        match self.maps.get(d)? {
            IndexMap::BrainModels { volume, .. } | IndexMap::Parcels { volume, .. } => {
                volume.as_ref()
            }
            _ => None,
        }
    }

    /// Row `r` of the matrix (scaled): the `shape[0]` values along dimension 0 at index `r`
    /// of dimension 1. Rows are stored whole (dimension 0 varies fastest), so this reads
    /// only that row's bytes: from a memory-mapped file, only its pages. For a dense
    /// connectivity file, the connectivity of seed `r` to every brain model of dimension 0.
    pub fn row(&self, data: &[u8], r: usize) -> Result<Vec<f32>> {
        if r >= self.shape[1] {
            return Err(Error::Malformed("no such CIFTI row".into()));
        }
        let size = self.nifti.sample_size().ok_or(Error::TooLarge)?;
        let n = self.shape[0];
        let at = n
            .checked_mul(size)
            .and_then(|len| Some((r.checked_mul(len)?.checked_add(self.nifti.vox_offset)?, len)))
            .and_then(|(at, len)| data.get(at..at.checked_add(len)?))
            .ok_or_else(|| Error::Truncated("CIFTI matrix".into()))?;
        let (slope, inter) = self.nifti.scale;
        let big = self.nifti.big_endian;
        let ty = self.nifti.datatype;
        if ty == 16 && !big && slope == 1.0 && inter == 0.0 {
            return Ok(at
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect());
        }
        Ok(at
            .chunks_exact(size)
            .map(|b| (sample(b, ty, big) * slope + inter) as f32)
            .collect())
    }
}

fn sample(b: &[u8], ty: i16, big: bool) -> f64 {
    let mut a = [0u8; 8];
    a[..b.len()].copy_from_slice(b);
    if big {
        a[..b.len()].reverse();
    }
    let u = u64::from_le_bytes(a);
    match ty {
        2 => f64::from(a[0]),
        256 => f64::from(a[0] as i8),
        4 => f64::from(u as u16 as i16),
        512 => f64::from(u as u16),
        8 => f64::from(u as u32 as i32),
        768 => f64::from(u as u32),
        16 => f64::from(f32::from_bits(u as u32)),
        64 => f64::from_bits(u),
        1024 => u as i64 as f64,
        1280 => u as f64,
        _ => f64::NAN,
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A little-endian NIfTI-2 header with a CIFTI-2 extension holding `xml`, and a 1 × 1
    /// float matrix.
    fn cifti(xml: &str) -> Vec<u8> {
        let mut x = xml.as_bytes().to_vec();
        x.resize(x.len().next_multiple_of(16), 0);
        let ext = 8 + x.len();
        let mut d = vec![0u8; 544];
        d[..4].copy_from_slice(&540i32.to_le_bytes());
        d[4..8].copy_from_slice(b"n+2\0");
        d[12..14].copy_from_slice(&16i16.to_le_bytes());
        for (i, v) in [6i64, 1, 1, 1, 1, 1, 1, 1].into_iter().enumerate() {
            d[16 + 8 * i..24 + 8 * i].copy_from_slice(&v.to_le_bytes());
        }
        d[168..176].copy_from_slice(&((544 + ext) as i64).to_le_bytes());
        d[540] = 1;
        d.extend_from_slice(&(ext as i32).to_le_bytes());
        d.extend_from_slice(&ECODE_CIFTI2.to_le_bytes());
        d.extend_from_slice(&x);
        d.extend_from_slice(&1f32.to_le_bytes());
        d
    }

    #[test]
    fn implicit_vertices_of_a_huge_surface() {
        let xml = "<CIFTI Version=\"2\"><Matrix>\
            <MatrixIndicesMap AppliesToMatrixDimension=\"0\" \
             IndicesMapToDataType=\"CIFTI_INDEX_TYPE_SCALARS\"><NamedMap><MapName>a</MapName>\
             </NamedMap></MatrixIndicesMap>\
            <MatrixIndicesMap AppliesToMatrixDimension=\"1\" \
             IndicesMapToDataType=\"CIFTI_INDEX_TYPE_BRAIN_MODELS\">\
            <BrainModel IndexOffset=\"0\" IndexCount=\"67108864\" \
             ModelType=\"CIFTI_MODEL_TYPE_SURFACE\" BrainStructure=\"CIFTI_STRUCTURE_CORTEX_LEFT\" \
             SurfaceNumberOfVertices=\"67108864\"/>\
            </MatrixIndicesMap></Matrix></CIFTI>";
        assert!(matches!(parse(&cifti(xml)), Err(Error::Truncated(_))));
        // The same with one vertex parses.
        let ok = xml.replace("67108864", "1");
        assert!(parse(&cifti(&ok)).is_ok());
    }

    const DCONN: &str = "<CIFTI Version=\"2\"><Matrix>\
        <MatrixIndicesMap AppliesToMatrixDimension=\"0,1\" \
         IndicesMapToDataType=\"CIFTI_INDEX_TYPE_BRAIN_MODELS\">\
        <BrainModel IndexOffset=\"0\" IndexCount=\"1\" \
         ModelType=\"CIFTI_MODEL_TYPE_SURFACE\" BrainStructure=\"CIFTI_STRUCTURE_CORTEX_LEFT\" \
         SurfaceNumberOfVertices=\"1\"/>\
        </MatrixIndicesMap></Matrix></CIFTI>";

    #[test]
    fn dense_connectivity_rows() {
        let d = cifti(DCONN);
        let c = parse(&d).unwrap();
        assert!(c.is_dense_connectivity());
        assert_eq!(c.models_of(0).len(), 1);
        assert_eq!(c.row(&d, 0).unwrap(), vec![1.0]);
        // A row past the matrix, and a matrix cut short, are errors (not a panic).
        assert!(matches!(c.row(&d, 1), Err(Error::Malformed(_))));
        assert!(matches!(
            c.row(&d[..d.len() - 1], 0),
            Err(Error::Truncated(_))
        ));
        assert!(matches!(c.row(&d, usize::MAX), Err(Error::Malformed(_))));
        let mut huge = c.clone();
        huge.shape = [usize::MAX / 2, 4];
        assert!(matches!(huge.row(&d, 3), Err(Error::Truncated(_))));
    }
}
