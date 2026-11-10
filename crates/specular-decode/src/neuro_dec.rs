//! HCP-style neuroimaging files via specular-neuro: GIFTI surfaces and the per-vertex data
//! that colours them, CIFTI-2 maps over surfaces and subcortical voxels, and TrackVis /
//! MRtrix tractograms.
//!
//! - A GIFTI surface (`.surf.gii`) is a mesh ([`Session::mesh`]); arrays of values beside
//!   its vertices colour it, one page each.
//! - A GIFTI data file (`.func.gii`, `.shape.gii`, `.label.gii`) has no geometry: the
//!   session finds a `.surf.gii` beside it with the same vertex count and hemisphere (HCP
//!   naming; midthickness before white, pial and inflated) and shows that surface coloured
//!   by each array in turn. Without one, opening fails with [`needs_surface`].
//! - A CIFTI-2 file shows its surface brainordinates on the left and right surfaces found
//!   beside it the same way, one page per map (or time point); without surfaces, its
//!   subcortical voxels as a volume (medical mode and MPR), every map a volume; without
//!   either, [`needs_surface`].
//! - Scalars use the science colour maps over their 2nd–98th percentile (a diverging map
//!   when they straddle zero evenly); labels use the file's label table. Vertices without a
//!   value (the medial wall) are grey.
//! - A tractogram's streamlines (at most [`MAX_SHOWN_POINTS`] points; more are subsampled
//!   by streamline) are [`Session::tracts`], in RAS millimetres.
//!
//! Like meshes, these have no pixel size: the session renders an isometric preview whose
//! long side is [`PREVIEW_LONG_SIDE`](crate::mesh_dec::PREVIEW_LONG_SIDE) px.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_inspect::ColourMap;
use specular_mesh::{Bounds, Mesh, Up};
pub use specular_neuro::Tractogram;

mod dconn;
pub use dconn::Connectivity;
use specular_neuro::cifti::{self, Cifti, IndexMap, Model};
use specular_neuro::gifti::{self, Gifti, Label, Role};
use specular_neuro::{tck, trk};

use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, MeshModel, PageLabel, Result, Session,
    TractsModel,
};

/// Streamline points shown at most; larger tractograms keep every n-th streamline.
pub const MAX_SHOWN_POINTS: usize = 1_000_000;

/// The CIFTI volume shown without surfaces holds at most this many bytes of float voxels
/// (maps past it are left out).
const MAX_VOLUME_BYTES: usize = 512 << 20;

/// `.surf.gii` files looked at beside a data file, and the bytes read of each.
const MAX_CANDIDATES: usize = 256;
const CANDIDATE_HEAD: usize = 64 << 10;

/// Grey for vertices without a value (the medial wall of a CIFTI map).
const NO_DATA: [u8; 4] = [150, 150, 150, 255];

/// The start of the message of an open that needs a surface ([`needs_surface`]).
const NEEDS_SURFACE: &str = "needs a surface with ";

/// The vertex count of the surface a GIFTI data file or a CIFTI file needs and did not find
/// beside it, when `e` is that error.
pub fn needs_surface(e: &DecodeError) -> Option<usize> {
    match e {
        DecodeError::Unsupported(m) => m
            .strip_prefix(NEEDS_SURFACE)?
            .split(' ')
            .next()?
            .parse()
            .ok(),
        _ => None,
    }
}

fn needs(vertices: usize) -> DecodeError {
    DecodeError::Unsupported(format!(
        "{NEEDS_SURFACE}{vertices} vertices (a .surf.gii beside the file)"
    ))
}

/// What the info panel says about a neuroimaging file, and the values a surface is
/// coloured by.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NeuroFacts {
    pub gifti: Option<GiftiFacts>,
    pub cifti: Option<CiftiFacts>,
    pub tracts: Option<TractFacts>,
    /// The shown surface's values per mesh vertex, when they are scalars (not labels).
    pub scalars: Option<SurfaceScalars>,
    /// A dense connectivity file: the rows to show as seeds are picked.
    pub connectivity: Option<ConnectivityModel>,
}

/// A dense connectivity file ([`Connectivity`]) shared by the sessions showing it; equal
/// when it is the same file opened once.
#[derive(Clone, Debug)]
pub struct ConnectivityModel(pub Arc<Connectivity>);

impl PartialEq for ConnectivityModel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// The values a surface page shows, for recolouring it with another colour map.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceScalars {
    /// One per mesh vertex; NaN where there is none.
    pub values: Arc<[f32]>,
    /// The values mapped to the ends of the colour map.
    pub range: (f32, f32),
    /// The values straddle zero: a diverging map suits them.
    pub diverging: bool,
}

/// A GIFTI file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GiftiFacts {
    pub version: String,
    pub arrays: usize,
    /// "surface", "labels" or "data".
    pub role: &'static str,
    pub vertices: Option<usize>,
    pub triangles: Option<usize>,
    /// `AnatomicalStructurePrimary` ("CortexLeft").
    pub structure: Option<String>,
    /// `GeometricType` of a surface ("Anatomical", "Inflated").
    pub geometry: Option<String>,
    /// The array shown: its name, intent and data type.
    pub array: Option<String>,
    pub intent: Option<String>,
    pub data_type: Option<&'static str>,
    /// The first array's encoding ("GZipBase64Binary").
    pub encoding: &'static str,
    /// Entries of the label table.
    pub labels: usize,
    /// The coordinate transform of the vertices: from and to space, and whether it was
    /// applied (spaces differ and the matrix is not the identity).
    pub transform: Option<(String, String, bool)>,
    /// The surface a data file is shown on (its file name).
    pub surface: Option<String>,
}

/// A CIFTI-2 file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CiftiFacts {
    pub version: String,
    /// The NIfTI intent ("ConnDenseScalar").
    pub intent: &'static str,
    /// Rows × columns of the matrix.
    pub shape: [usize; 2],
    /// What indexes the maps: "scalars", "labels", "series", "parcels".
    pub map_kind: &'static str,
    pub maps: usize,
    /// A series' start, step and unit ("SECOND").
    pub series: Option<(f64, f64, String)>,
    /// Brain models (or parcels): how many, and the voxels among them.
    pub models: usize,
    pub parcels: Option<usize>,
    pub voxels: usize,
    /// Every brain structure named, short ("CORTEX_LEFT").
    pub structures: Vec<String>,
    /// The surfaces the file needs: structure, vertices, and the file found for it.
    pub surfaces: Vec<(String, usize, Option<String>)>,
    /// "surfaces" or "voxels": how the file is shown.
    pub shown: &'static str,
    /// Maps left out of the voxel volume (it would be too large).
    pub maps_left_out: usize,
    /// Dense connectivity: the seeds (rows) and the bytes read per row.
    pub dense: Option<(usize, usize)>,
}

/// A tractogram.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TractFacts {
    /// "TrackVis 2", "MRtrix".
    pub format: String,
    pub streamlines: usize,
    pub points: usize,
    /// Points drawn (fewer when subsampled).
    pub shown_points: usize,
    pub shown_streamlines: usize,
    pub scalars: Vec<String>,
    pub properties: Vec<String>,
    /// TrackVis grid: dimensions, voxel size (mm), voxel order.
    pub dims: Option<[i16; 3]>,
    pub voxel_size: Option<[f32; 3]>,
    pub voxel_order: Option<String>,
    /// How points became RAS millimetres: "vox_to_ras", "voxel order" (TrackVis), "scanner"
    /// (MRtrix).
    pub space: &'static str,
    /// MRtrix step size (mm), as written.
    pub step: Option<String>,
    /// Mean streamline length in millimetres.
    pub mean_length: Option<f64>,
}

pub struct NeuroDecoder(pub Format);

impl Decoder for NeuroDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        !head.is_empty() && sniff(self.0, head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        !data.is_empty() && sniff(self.0, data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::Gifti => &["gii"],
            Format::Cifti => &[
                "dscalar.nii",
                "dlabel.nii",
                "dtseries.nii",
                "pscalar.nii",
                "plabel.nii",
                "ptseries.nii",
                "dconn.nii",
            ],
            Format::Trk => &["trk"],
            Format::Tck => &["tck"],
            _ => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        match self.0 {
            Format::Gifti => open_gifti(source),
            Format::Cifti => open_cifti(source),
            _ => open_tracts(source),
        }
    }
}

fn sniff(f: Format, data: &[u8]) -> bool {
    match f {
        Format::Gifti => gifti::sniff(data),
        Format::Cifti => cifti::sniff(data),
        Format::Trk => trk::sniff(data),
        Format::Tck => tck::sniff(data),
        _ => false,
    }
}

fn map_err(e: specular_neuro::Error) -> DecodeError {
    use specular_neuro::Error as E;
    match e {
        E::NotRecognised => DecodeError::UnsupportedFormat,
        E::Truncated(s) => DecodeError::Truncated(s),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
        E::Io(s) => DecodeError::Io(std::io::Error::other(s)),
    }
}

fn mesh_err(e: specular_mesh::Error) -> DecodeError {
    match e {
        specular_mesh::Error::TooLarge(_) => DecodeError::TooLarge,
        specular_mesh::Error::Empty => DecodeError::Malformed("a surface without faces".into()),
        e => DecodeError::Malformed(e.to_string()),
    }
}

// ---------------------------------------------------------------------------------------
// Surfaces

/// A surface's geometry from a GIFTI file: vertices (transformed where the file says),
/// faces, and the facts about them.
struct Geometry {
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
    transform: Option<(String, String, bool)>,
}

fn geometry(g: &Gifti) -> Result<Geometry> {
    let (Some(p), Some(t)) = (g.pointset(), g.triangles()) else {
        return Err(DecodeError::Malformed(
            "a GIFTI surface without vertices or faces".into(),
        ));
    };
    let xyz = p.values.to_f32();
    let mut positions: Vec<[f32; 3]> = xyz.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    let n = positions.len();
    let indices: Vec<u32> = t
        .values
        .to_i32()
        .into_iter()
        .map(|i| {
            if i >= 0 && (i as usize) < n {
                i as u32
            } else {
                u32::MAX
            }
        })
        .collect();
    let mut transform = None;
    if let Some(c) = p.coord.first() {
        let identity =
            (0..4).all(|r| (0..4).all(|k| c.matrix[r][k] == f64::from(u8::from(r == k))));
        let apply = c.data_space.trim() != c.transformed_space.trim() && !identity;
        if apply {
            let m = c.matrix;
            for q in &mut positions {
                let v = q.map(f64::from);
                *q = [0, 1, 2]
                    .map(|r| (m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2] + m[r][3]) as f32);
            }
        }
        transform = Some((
            space_name(&c.data_space),
            space_name(&c.transformed_space),
            apply,
        ));
    }
    Ok(Geometry {
        positions,
        indices,
        transform,
    })
}

/// "NIFTI_XFORM_TALAIRACH" as "Talairach".
fn space_name(s: &str) -> String {
    let s = s.trim();
    match s.strip_prefix("NIFTI_XFORM_").unwrap_or(s) {
        "UNKNOWN" => "unknown".into(),
        "SCANNER_ANAT" => "scanner".into(),
        "ALIGNED_ANAT" => "aligned".into(),
        "TALAIRACH" => "Talairach".into(),
        "MNI_152" => "MNI 152".into(),
        o => o.to_owned(),
    }
}

/// What colours a page of a surface.
enum PageValues {
    Plain,
    Scalars(Vec<f32>),
    Labels(Vec<i32>, Arc<[Label]>),
}

/// Where a surface session's pages come from.
enum Layers {
    /// A bare surface: one page, no colours.
    None,
    /// GIFTI arrays: (array, column) per page.
    Gifti {
        g: Arc<Gifti>,
        pages: Vec<(usize, usize)>,
        labels: Arc<[Label]>,
    },
    /// CIFTI maps: each brainordinate column's mesh vertices.
    Cifti {
        c: Arc<Cifti>,
        data: Data,
        assign: Vec<(u32, u32)>,
    },
    /// A dense connectivity file: one page, a seed's row (the first seed's when opened).
    Dense(Arc<Connectivity>),
}

/// A CIFTI file's (inflated) bytes.
#[derive(Clone)]
enum Data {
    Source(Arc<Source>),
    Owned(Arc<Vec<u8>>),
}

impl Data {
    fn bytes(&self) -> &[u8] {
        match self {
            Data::Source(s) => s.bytes(),
            Data::Owned(v) => v,
        }
    }
}

struct Shared {
    source: Arc<Source>,
    /// The surface without colours.
    mesh: Mesh,
    layers: Layers,
    pages: u32,
    format: Format,
    facts: NeuroFacts,
}

impl Shared {
    fn page_values(&self, page: u32) -> Result<PageValues> {
        let n = self.mesh.positions.len();
        Ok(match &self.layers {
            Layers::None => PageValues::Plain,
            Layers::Gifti { g, pages, labels } => {
                let &(a, col) = pages
                    .get(page as usize)
                    .ok_or_else(|| DecodeError::Malformed("no such page".into()))?;
                let arr = &g.arrays[a];
                let cols = arr.columns();
                let at = |v: usize| v * cols + col;
                if arr.is(gifti::LABEL)
                    || (!labels.is_empty() && !matches!(arr.values, gifti::Values::F32(_)))
                {
                    let keys = (0..n)
                        .map(|v| {
                            if at(v) < arr.values.len() {
                                arr.values.get(at(v)) as i32
                            } else {
                                i32::MIN
                            }
                        })
                        .collect();
                    PageValues::Labels(keys, labels.clone())
                } else {
                    PageValues::Scalars(
                        (0..n)
                            .map(|v| {
                                if at(v) < arr.values.len() {
                                    arr.values.get(at(v)) as f32
                                } else {
                                    f32::NAN
                                }
                            })
                            .collect(),
                    )
                }
            }
            Layers::Cifti { c, data, assign } => {
                let vals = c.map(data.bytes(), page as usize).map_err(map_err)?;
                let get = |col: u32| vals.get(col as usize).copied().unwrap_or(f32::NAN);
                if c.is_labels() {
                    let mut keys = vec![i32::MIN; n];
                    for &(v, col) in assign {
                        keys[v as usize] = get(col).round() as i32;
                    }
                    let table: Arc<[Label]> = c.labels(page as usize).unwrap_or(&[]).into();
                    PageValues::Labels(keys, table)
                } else {
                    let mut out = vec![f32::NAN; n];
                    for &(v, col) in assign {
                        out[v as usize] = get(col);
                    }
                    PageValues::Scalars(out)
                }
            }
            Layers::Dense(conn) => {
                PageValues::Scalars(conn.vertex_values(&conn.row(conn.first_seed())?))
            }
        })
    }

    fn page_name(&self, page: u32) -> String {
        match &self.layers {
            Layers::None => String::new(),
            Layers::Gifti { g, pages, .. } => {
                let Some(&(a, col)) = pages.get(page as usize) else {
                    return String::new();
                };
                let arr = &g.arrays[a];
                let base = arr.name().map(str::to_owned).unwrap_or_else(|| {
                    let i = arr.intent.trim();
                    i.strip_prefix("NIFTI_INTENT_")
                        .unwrap_or(i)
                        .to_ascii_lowercase()
                });
                if arr.columns() > 1 {
                    format!("{base} [{}]", col + 1)
                } else {
                    base
                }
            }
            Layers::Cifti { c, .. } => c.map_name(page as usize),
            Layers::Dense(conn) => conn.seed_name(conn.first_seed()),
        }
    }
}

/// The 2nd and 98th percentile of the finite values; `None` without any.
fn robust_range(values: &[f32]) -> Option<(f32, f32)> {
    let mut v: Vec<f32> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    // A spread sample is enough for percentiles.
    if v.len() > 1 << 18 {
        let step = v.len() / (1 << 18);
        v = v.into_iter().step_by(step).collect();
    }
    v.sort_by(f32::total_cmp);
    let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
    let (mut lo, mut hi) = (at(0.02), at(0.98));
    if hi <= lo {
        (lo, hi) = (v[0], v[v.len() - 1]);
    }
    Some((lo, hi))
}

/// The scalars' colouring range and whether they straddle zero evenly (diverging).
fn scalar_range(values: &[f32]) -> ((f32, f32), bool) {
    let Some((lo, hi)) = robust_range(values) else {
        return ((0.0, 1.0), false);
    };
    if lo < 0.0 && hi > 0.0 {
        let (a, b) = (-lo, hi);
        if a.max(b) <= 4.0 * a.min(b) {
            let m = a.max(b);
            return ((-m, m), true);
        }
    }
    ((lo, hi), false)
}

/// sRGB colours of `values` through `map` over `range` (grey where there is no value).
pub fn colour_scalars(values: &[f32], range: (f32, f32), map: ColourMap) -> Vec<[u8; 4]> {
    let table = map.table();
    let (lo, hi) = range;
    let span = if hi > lo { hi - lo } else { 1.0 };
    values
        .iter()
        .map(|&x| {
            if !x.is_finite() {
                return NO_DATA;
            }
            let t = ((x - lo) / span).clamp(0.0, 1.0);
            let c = table[(t * (table.len() - 1) as f32).round() as usize];
            let b = c.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
            [b[0], b[1], b[2], 255]
        })
        .collect()
}

/// sRGB colours of `values` over their robust range: Vik when they straddle zero evenly,
/// else Viridis (as surfaces are coloured).
pub fn auto_colours(values: &[f32]) -> Vec<[u8; 4]> {
    let (range, diverging) = scalar_range(values);
    let map = if diverging {
        ColourMap::Vik
    } else {
        ColourMap::Viridis
    };
    colour_scalars(values, range, map)
}

/// sRGB colours of label `keys` from their table (translucent labels over grey).
fn colour_labels(keys: &[i32], table: &[Label]) -> Vec<[u8; 4]> {
    let lut: std::collections::HashMap<i32, [u8; 4]> = table
        .iter()
        .map(|l| {
            let a = l.rgba[3].clamp(0.0, 1.0);
            let c = [0, 1, 2].map(|k| {
                let v = l.rgba[k].clamp(0.0, 1.0) * a + f32::from(NO_DATA[k]) / 255.0 * (1.0 - a);
                (v * 255.0).round() as u8
            });
            (l.key, [c[0], c[1], c[2], 255])
        })
        .collect();
    keys.iter()
        .map(|k| lut.get(k).copied().unwrap_or(NO_DATA))
        .collect()
}

fn page_label(name: String) -> Option<PageLabel> {
    Some(PageLabel::Map { name: name.into() })
}

struct SurfaceSession {
    shared: Arc<Shared>,
    info: ImageInfo,
    page: u32,
    model: MeshModel,
    facts: NeuroFacts,
    /// The last whole preview rendered, by scale: regions crop it.
    cache: Mutex<Option<(u32, Arc<Vec<u8>>)>>,
}

fn surface_session(shared: Arc<Shared>, page: u32) -> Result<Arc<dyn Session>> {
    let mut facts = shared.facts.clone();
    let mut mesh = shared.mesh.clone();
    match shared.page_values(page)? {
        PageValues::Plain => {}
        PageValues::Scalars(v) => {
            let (range, diverging) = scalar_range(&v);
            let map = if diverging {
                ColourMap::Vik
            } else {
                ColourMap::Viridis
            };
            mesh.colors = Some(colour_scalars(&v, range, map));
            facts.scalars = Some(SurfaceScalars {
                values: v.into(),
                range,
                diverging,
            });
        }
        PageValues::Labels(keys, table) => {
            mesh.colors = Some(colour_labels(&keys, &table));
        }
    }
    if let (Some(gf), Layers::Gifti { g, pages, .. }) = (&mut facts.gifti, &shared.layers)
        && let Some(&(a, _)) = pages.get(page as usize)
    {
        let arr = &g.arrays[a];
        gf.array = Some(shared.page_name(page)).filter(|s| !s.is_empty());
        gf.intent = Some(arr.intent.trim().to_owned());
        gf.data_type = Some(arr.values.type_name());
    }
    let (w, h) = crate::mesh_dec::preview_size_of(mesh.bounds, Up::Z);
    let coding = format!(
        "surface, {} vertices, {} triangles",
        mesh.vertex_count(),
        mesh.triangle_count()
    );
    let info = ImageInfo {
        format: shared.format,
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: true,
        bit_depth: 8,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(SurfaceSession {
        info,
        page,
        model: MeshModel {
            mesh: Arc::new(mesh),
            textures: Arc::new(Vec::new()),
            ifc: None,
        },
        facts,
        shared,
        cache: Mutex::new(None),
    }))
}

impl SurfaceSession {
    fn rendered(&self, scale: u32) -> Arc<Vec<u8>> {
        if let Some((s, px)) = &*self.cache.lock()
            && *s == scale
        {
            return px.clone();
        }
        let (w, h) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let px = Arc::new(specular_mesh::preview::render(&self.model.mesh, &[], w, h));
        *self.cache.lock() = Some((scale, px.clone()));
        px
    }
}

impl Session for SurfaceSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.shared.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        crop(&self.info, req, cancel, |s| self.rendered(s))
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        render_memory(&self.info, req) + self.model.mesh.memory_bytes() as u64
    }
    fn memory_bytes(&self) -> usize {
        let cached = self.cache.lock().as_ref().map_or(0, |(_, px)| px.len());
        self.model.mesh.memory_bytes() + cached
    }
    fn trim_memory(&self) -> usize {
        self.cache.lock().take().map_or(0, |(_, px)| px.len())
    }
    fn mesh(&self) -> Option<&MeshModel> {
        Some(&self.model)
    }
    fn neuro(&self) -> Option<&NeuroFacts> {
        Some(&self.facts)
    }
    fn page_count(&self) -> u32 {
        self.shared.pages
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        if self.shared.pages < 2 && matches!(self.shared.layers, Layers::None) {
            return None;
        }
        page_label(self.shared.page_name(self.page)).filter(|_| self.shared.pages > 1)
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.shared.pages {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        surface_session(self.shared.clone(), page)
    }
}

/// Crops a whole rendering (by `render(scale)`) to the request.
fn crop(
    info: &ImageInfo,
    req: &DecodeRequest,
    cancel: &CancelToken,
    render: impl FnOnce(u32) -> Arc<Vec<u8>>,
) -> Result<Raster> {
    if cancel.is_cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let scale = req.scale.clamp(1, 8);
    let (sw, sh) = (info.width.div_ceil(scale), info.height.div_ceil(scale));
    let (x, y, w, h) = match req.region {
        Some(r) => {
            let x = r.x.min(sw);
            let y = r.y.min(sh);
            (x, y, r.w.min(sw - x), r.h.min(sh - y))
        }
        None => (0, 0, sw, sh),
    };
    if w == 0 || h == 0 {
        return Err(DecodeError::Unsupported("empty region".into()));
    }
    let full = render(scale);
    if cancel.is_cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let data = if (x, y, w, h) == (0, 0, sw, sh) {
        full.as_ref().clone()
    } else {
        let mut out = Vec::with_capacity(w as usize * h as usize * 4);
        for row in y..y + h {
            let o = (row as usize * sw as usize + x as usize) * 4;
            out.extend_from_slice(&full[o..o + w as usize * 4]);
        }
        out
    };
    Ok(Raster {
        width: w,
        height: h,
        origin: (x, y),
        scale,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride: w as usize * 4,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    })
}

/// The output, the cached preview and the renderer's supersampled colour and depth.
fn render_memory(info: &ImageInfo, req: &DecodeRequest) -> u64 {
    let s = req.scale.clamp(1, 8) as u64;
    let whole = info.width as u64 * info.height as u64 / (s * s);
    whole * 4 * 2 + whole * 4 * 8
}

// ---------------------------------------------------------------------------------------
// Finding surfaces

/// "CIFTI_STRUCTURE_CORTEX_LEFT" and "CortexLeft" both as "cortexleft".
fn structure_key(s: &str) -> String {
    let k: String = s
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    k.strip_prefix("ciftistructure")
        .map(str::to_owned)
        .unwrap_or(k)
}

/// 'L' or 'R' from a structure name.
fn structure_side(s: &str) -> Option<char> {
    let k = structure_key(s);
    if k.ends_with("left") {
        Some('L')
    } else if k.ends_with("right") {
        Some('R')
    } else {
        None
    }
}

/// 'L' or 'R' from a file name's tokens ("100307.L.midthickness…", "lh.white.surf.gii").
fn name_side(name: &str) -> Option<char> {
    name.split(['.', '_', '-', ' '])
        .find_map(|t| match t.to_ascii_lowercase().as_str() {
            "l" | "lh" | "left" => Some('L'),
            "r" | "rh" | "right" => Some('R'),
            _ => None,
        })
}

/// How well a surface's kind suits showing data: midthickness first.
fn kind_rank(name: &str) -> u32 {
    let n = name.to_ascii_lowercase();
    if n.contains("midthickness") {
        0
    } else if n.contains("very_inflated") || n.contains("very-inflated") {
        4
    } else if n.contains("white") {
        1
    } else if n.contains("pial") {
        2
    } else if n.contains("inflated") {
        3
    } else if n.contains("sphere") || n.contains("flat") {
        8
    } else {
        5
    }
}

/// The vertex count and structure a `.surf.gii` states near its start (its pointset
/// array's `Dim0` and `AnatomicalStructurePrimary`).
fn scan_surface_head(head: &[u8]) -> Option<(usize, Option<String>)> {
    let s = String::from_utf8_lossy(head);
    let at = s.find("NIFTI_INTENT_POINTSET")?;
    let open = s[..at].rfind("<DataArray")?;
    let close = open + s[open..].find('>')?;
    let tag = &s[open..close];
    let d = tag.find("Dim0")?;
    let rest = &tag[d + 4..];
    let q = rest.find(['"', '\''])?;
    let rest = &rest[q + 1..];
    let e = rest.find(['"', '\''])?;
    let n = rest[..e].trim().parse().ok()?;
    let structure = s.find("AnatomicalStructurePrimary").and_then(|i| {
        let r = &s[i..];
        let v = r.find("<Value>")?;
        let r = &r[v + 7..];
        let e = r.find("</Value>")?;
        let t = r[..e].trim();
        let t = t
            .strip_prefix("<![CDATA[")
            .and_then(|t| t.strip_suffix("]]>"))
            .unwrap_or(t);
        Some(t.trim().to_owned())
    });
    Some((n, structure))
}

fn read_head(path: &Path, n: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let f = std::fs::File::open(path).ok()?;
    let mut out = Vec::with_capacity(n);
    f.take(n as u64).read_to_end(&mut out).ok()?;
    Some(out)
}

/// The best `.surf.gii` beside `file` with `vertices` vertices on `side` ('L', 'R', or
/// either when `None`) or of `structure`.
fn find_surface(
    file: &Path,
    vertices: usize,
    structure: Option<&str>,
    side: Option<char>,
) -> Option<PathBuf> {
    let dir = file
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let own = file.file_name()?.to_string_lossy().into_owned();
    let mut found: Vec<(u32, usize, String, PathBuf)> = Vec::new();
    for e in std::fs::read_dir(dir).ok()?.flatten().take(4096) {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.to_ascii_lowercase().ends_with(".surf.gii") || name == own {
            continue;
        }
        if found.len() >= MAX_CANDIDATES {
            break;
        }
        let path = e.path();
        let Some((n, st)) = read_head(&path, CANDIDATE_HEAD).and_then(|h| scan_surface_head(&h))
        else {
            continue;
        };
        if n != vertices {
            continue;
        }
        let same = match (structure, st.as_deref()) {
            (Some(a), Some(b)) => structure_key(a) == structure_key(b),
            _ => match (
                side,
                st.as_deref()
                    .and_then(structure_side)
                    .or_else(|| name_side(&name)),
            ) {
                (Some(a), Some(b)) => a == b,
                (Some(_), None) => false,
                (None, _) => true,
            },
        };
        if !same {
            continue;
        }
        let common = own
            .bytes()
            .zip(name.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        found.push((kind_rank(&name), usize::MAX - common, name, path));
    }
    found.sort();
    found.into_iter().next().map(|f| f.3)
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Whether the source names a file on disk whose folder can be looked in.
fn on_disk(source: &Source) -> Option<&Path> {
    let p = source.path();
    p.is_file().then_some(p)
}

// ---------------------------------------------------------------------------------------
// GIFTI

fn gifti_facts(g: &Gifti) -> GiftiFacts {
    GiftiFacts {
        version: g.version.clone(),
        arrays: g.arrays.len(),
        role: match g.role() {
            Role::Surface => "surface",
            Role::Labels => "labels",
            Role::Data => "data",
        },
        vertices: g.vertex_count(),
        triangles: g.triangles().map(|t| t.values.len() / 3),
        structure: g.structure().map(str::to_owned),
        geometry: g
            .pointset()
            .and_then(|p| gifti::meta(&p.meta, "GeometricType"))
            .map(str::to_owned),
        encoding: g.arrays.first().map_or("", |a| a.encoding.name()),
        labels: g.labels.len(),
        ..GiftiFacts::default()
    }
}

/// The data pages of `g` over `n` vertices: every column of every array of values.
fn data_pages(g: &Gifti, n: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (i, a) in g.arrays.iter().enumerate() {
        if a.is(gifti::POINTSET) || a.is(gifti::TRIANGLE) || a.is("NIFTI_INTENT_NODE_INDEX") {
            continue;
        }
        if a.dims.first() != Some(&n) || a.values.len() < n {
            continue;
        }
        for c in 0..a.columns() {
            out.push((i, c));
        }
    }
    out
}

fn surface_mesh(geo: Geometry) -> Result<Mesh> {
    let mut m = specular_mesh::from_triangles(
        geo.positions,
        geo.indices,
        None,
        &specular_mesh::Limits::default(),
    )
    .map_err(mesh_err)?;
    m.unit = specular_mesh::Unit::Millimetre;
    m.up = Up::Z;
    Ok(m)
}

fn open_gifti(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let path = on_disk(&source).map(Path::to_path_buf);
    let g = gifti::parse(source.bytes(), path.as_deref()).map_err(map_err)?;
    let mut facts = gifti_facts(&g);
    let (geo, surface_file) = if g.role() == Role::Surface {
        (geometry(&g)?, None)
    } else {
        let n = g
            .vertex_count()
            .ok_or_else(|| DecodeError::Malformed("a GIFTI file without arrays".into()))?;
        let structure = g.structure().map(str::to_owned);
        let side = structure
            .as_deref()
            .and_then(structure_side)
            .or_else(|| name_side(&file_name(source.path())));
        let found = path
            .as_deref()
            .and_then(|p| find_surface(p, n, structure.as_deref(), side))
            .ok_or_else(|| needs(n))?;
        let s = gifti::open(&found).map_err(map_err)?;
        if s.vertex_count() != Some(n) {
            return Err(needs(n));
        }
        (geometry(&s)?, Some(file_name(&found)))
    };
    facts.transform = geo.transform.clone();
    facts.surface = surface_file;
    let mesh = surface_mesh(geo)?;
    let n = mesh.positions.len();
    let pages = data_pages(&g, n);
    if g.role() != Role::Surface && pages.is_empty() {
        return Err(DecodeError::Malformed(
            "no array with a value per vertex".into(),
        ));
    }
    let count = pages.len().max(1);
    let layers = if pages.is_empty() {
        Layers::None
    } else {
        let labels: Arc<[Label]> = g.labels.clone().into();
        Layers::Gifti {
            g: Arc::new(g),
            pages,
            labels,
        }
    };
    let shared = Arc::new(Shared {
        source,
        mesh,
        layers,
        pages: u32::try_from(count).map_err(|_| DecodeError::TooLarge)?,
        format: Format::Gifti,
        facts: NeuroFacts {
            gifti: Some(facts),
            ..NeuroFacts::default()
        },
    });
    surface_session(shared, 0)
}

// ---------------------------------------------------------------------------------------
// CIFTI

fn short_structure(s: &str) -> String {
    s.trim()
        .strip_prefix("CIFTI_STRUCTURE_")
        .unwrap_or(s.trim())
        .to_owned()
}

/// The surface structures a file needs and their vertex counts, in file order: for dense
/// connectivity, those of the seeds and then any others the rows colour.
fn surface_list(c: &Cifti) -> Vec<(String, usize)> {
    let mut out = c.surfaces();
    if c.is_dense_connectivity() {
        for m in c.models_of(0) {
            if let Model::Surface { vertices_total, .. } = m.model
                && !out
                    .iter()
                    .any(|s| structure_key(&s.0) == structure_key(&m.structure))
            {
                out.push((m.structure.clone(), vertices_total));
            }
        }
    }
    out
}

fn cifti_facts(c: &Cifti) -> CiftiFacts {
    let md = c.map_dim();
    let sd = c.spatial_dim();
    let mut structures: Vec<String> = Vec::new();
    match &c.maps[sd] {
        IndexMap::BrainModels { models, .. } => {
            for m in models {
                let s = short_structure(&m.structure);
                if !structures.contains(&s) {
                    structures.push(s);
                }
            }
        }
        IndexMap::Parcels { surfaces, .. } => {
            structures.extend(surfaces.iter().map(|s| short_structure(&s.0)));
        }
        _ => {}
    }
    CiftiFacts {
        version: c.version.clone(),
        intent: c.nifti.intent(),
        shape: c.shape,
        map_kind: c.maps[md].kind(),
        maps: c.map_count(),
        series: match &c.maps[md] {
            IndexMap::Series {
                start,
                step,
                unit,
                exponent,
                ..
            } => {
                let f = 10f64.powi(*exponent);
                Some((start * f, step * f, unit.clone()))
            }
            _ => None,
        },
        models: c.models().len(),
        parcels: match &c.maps[sd] {
            IndexMap::Parcels { parcels, .. } => Some(parcels.len()),
            _ => None,
        },
        voxels: c.voxel_count(),
        structures,
        surfaces: surface_list(c)
            .into_iter()
            .map(|(s, n)| (short_structure(&s), n, None))
            .collect(),
        shown: "surfaces",
        maps_left_out: 0,
        dense: None,
    }
}

fn open_cifti(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let data = match cifti::inflate(source.bytes()).map_err(map_err)? {
        Cow::Borrowed(_) => Data::Source(source.clone()),
        Cow::Owned(v) => Data::Owned(Arc::new(v)),
    };
    let c = cifti::parse(data.bytes()).map_err(map_err)?;
    let mut facts = cifti_facts(&c);
    if c.is_dense_connectivity() {
        let size = c.nifti.sample_size().unwrap_or(0);
        facts.dense = Some((c.shape[1], c.shape[0].saturating_mul(size)));
    }
    let path = on_disk(&source).map(Path::to_path_buf);
    // A surface beside the file for every surface structure, where there is one.
    let mut parts: Vec<(String, Gifti)> = Vec::new();
    for (i, (structure, n)) in surface_list(&c).into_iter().enumerate() {
        let side = structure_side(&structure);
        let Some(p) = path
            .as_deref()
            .and_then(|f| find_surface(f, n, Some(&structure), side))
        else {
            continue;
        };
        let Ok(g) = gifti::open(&p) else { continue };
        if g.vertex_count() != Some(n) || g.role() != Role::Surface {
            continue;
        }
        facts.surfaces[i].2 = Some(file_name(&p));
        parts.push((structure, g));
    }
    if !parts.is_empty() {
        return cifti_surfaces(source, c, data, parts, facts);
    }
    if c.is_dense_connectivity() {
        if c.volume_of(0).is_some() {
            return dense_volume(source, c, data, facts);
        }
    } else if c.voxel_count() > 0 && c.volume().is_some() {
        return cifti_volume(source, &c, data.bytes(), facts);
    }
    let n = surface_list(&c).iter().map(|s| s.1).max().unwrap_or(0);
    Err(needs(n))
}

fn cifti_surfaces(
    source: Arc<Source>,
    c: Cifti,
    data: Data,
    parts: Vec<(String, Gifti)>,
    facts: CiftiFacts,
) -> Result<Arc<dyn Session>> {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    // Each structure's first mesh vertex.
    let mut bases: Vec<(String, u32)> = Vec::new();
    let mut transform = None;
    for (structure, g) in &parts {
        let geo = geometry(g)?;
        let base = positions.len() as u32;
        let n = geo.positions.len() as u32;
        indices.extend(
            geo.indices
                .iter()
                .map(|&i| if i < n { i + base } else { u32::MAX }),
        );
        positions.extend(geo.positions);
        bases.push((structure_key(structure), base));
        transform = transform.or(geo.transform);
    }
    let total = positions.len() as u32;
    let base_of = |s: &str| bases.iter().find(|b| b.0 == structure_key(s)).map(|b| b.1);
    let mut mesh = surface_mesh(Geometry {
        positions,
        indices,
        transform: None,
    })?;
    mesh.title = None;
    let gifti = transform.map(|t| GiftiFacts {
        transform: Some(t),
        ..GiftiFacts::default()
    });
    if c.is_dense_connectivity() {
        let conn = Arc::new(Connectivity::new(
            Arc::new(c),
            data,
            total as usize,
            &base_of,
        ));
        let shared = Arc::new(Shared {
            source,
            mesh,
            layers: Layers::Dense(conn.clone()),
            pages: 1,
            format: Format::Cifti,
            facts: NeuroFacts {
                cifti: Some(facts),
                gifti,
                connectivity: Some(ConnectivityModel(conn)),
                ..NeuroFacts::default()
            },
        });
        return surface_session(shared, 0);
    }
    let mut assign: Vec<(u32, u32)> = Vec::new();
    let sd = c.spatial_dim();
    match &c.maps[sd] {
        IndexMap::BrainModels { models, .. } => {
            for m in models {
                let (Model::Surface { vertices, .. }, Some(base)) =
                    (&m.model, base_of(&m.structure))
                else {
                    continue;
                };
                for (k, &v) in vertices.iter().enumerate() {
                    let at = base.saturating_add(v);
                    if at < total {
                        assign.push((at, (m.offset + k) as u32));
                    }
                }
            }
        }
        IndexMap::Parcels { parcels, .. } => {
            for (p, parcel) in parcels.iter().enumerate() {
                for (s, verts) in &parcel.vertices {
                    let Some(base) = base_of(s) else { continue };
                    for &v in verts {
                        let at = base.saturating_add(v);
                        if at < total {
                            assign.push((at, p as u32));
                        }
                    }
                }
            }
        }
        _ => {}
    }
    let pages = u32::try_from(c.map_count().max(1)).map_err(|_| DecodeError::TooLarge)?;
    let shared = Arc::new(Shared {
        source,
        mesh,
        layers: Layers::Cifti {
            c: Arc::new(c),
            data,
            assign,
        },
        pages,
        format: Format::Cifti,
        facts: NeuroFacts {
            cifti: Some(facts),
            gifti,
            ..NeuroFacts::default()
        },
    });
    surface_session(shared, 0)
}

/// A CIFTI file's voxels as a float volume in its volume space, every map a volume (as
/// many as fit [`MAX_VOLUME_BYTES`]); zero outside the brain models.
fn cifti_volume(
    source: Arc<Source>,
    c: &Cifti,
    data: &[u8],
    mut facts: CiftiFacts,
) -> Result<Arc<dyn Session>> {
    let vs = c.volume().ok_or(DecodeError::UnsupportedFormat)?;
    let [d0, d1, d2] = vs.dims;
    let per = d0
        .checked_mul(d1)
        .and_then(|n| n.checked_mul(d2))
        .filter(|&n| n > 0 && n.saturating_mul(4) <= MAX_VOLUME_BYTES)
        .ok_or(DecodeError::TooLarge)?;
    let maps = c.map_count().max(1);
    let shown = maps.min(MAX_VOLUME_BYTES / 4 / per).max(1);
    facts.maps_left_out = maps - shown;
    facts.shown = "voxels";
    // (voxel index, column) of every voxel brainordinate.
    let mut voxels: Vec<(usize, usize)> = Vec::new();
    let index = |v: &[u32; 3]| {
        let [i, j, k] = v.map(|x| x as usize);
        (i < d0 && j < d1 && k < d2).then(|| i + d0 * (j + d1 * k))
    };
    match &c.maps[c.spatial_dim()] {
        IndexMap::BrainModels { models, .. } => {
            for m in models {
                if let Model::Voxels(vx) = &m.model {
                    for (k, v) in vx.iter().enumerate() {
                        if let Some(i) = index(v) {
                            voxels.push((i, m.offset + k));
                        }
                    }
                }
            }
        }
        IndexMap::Parcels { parcels, .. } => {
            for (p, parcel) in parcels.iter().enumerate() {
                for v in &parcel.voxels {
                    if let Some(i) = index(v) {
                        voxels.push((i, p));
                    }
                }
            }
        }
        _ => {}
    }
    let mut buf = vec![0f32; per * shown];
    for m in 0..shown {
        let vals = c.map(data, m).map_err(map_err)?;
        let out = &mut buf[m * per..(m + 1) * per];
        for &(i, col) in &voxels {
            if let Some(&x) = vals.get(col) {
                out[i] = if x.is_finite() { x } else { 0.0 };
            }
        }
    }
    let volume = float_volume(vs, buf, shown, &c.nifti.description, c.nifti.intent())?;
    crate::medvol_dec::open_prepared(
        source,
        volume,
        Format::Cifti,
        NeuroFacts {
            cifti: Some(facts),
            ..NeuroFacts::default()
        },
    )
}

/// `buf` (`count` volumes of `vs`'s grid, x fastest, float) as a volume, oriented as the
/// volume reader keeps volumes (LPS).
fn float_volume(
    vs: &cifti::VolumeSpace,
    buf: Vec<f32>,
    count: usize,
    description: &str,
    intent: &str,
) -> Result<crate::medvol_dec::medvol::MedVolume> {
    use crate::medvol_dec::medvol;
    let [d0, d1, d2] = vs.dims;
    let bytes: Vec<u8> = buf.iter().flat_map(|x| x.to_le_bytes()).collect();
    let extra: &[usize] = if count > 1 { &[count] } else { &[] };
    let layout = medvol::Layout::dense([d0, d1, d2], extra, 1).ok_or(DecodeError::TooLarge)?;
    let a = vs.affine;
    // RAS to the LPS the volume reader keeps.
    let affine = [
        [-a[0][0], -a[0][1], -a[0][2], -a[0][3]],
        [-a[1][0], -a[1][1], -a[1][2], -a[1][3]],
        [a[2][0], a[2][1], a[2][2], a[2][3]],
    ];
    let spacing = [0, 1, 2].map(|k| (0..3).map(|r| a[r][k] * a[r][k]).sum::<f64>().sqrt());
    let mut dims = vec![d0, d1, d2];
    if count > 1 {
        dims.push(count);
    }
    let header = medvol::Header {
        kind: medvol::Kind::Nifti2,
        dtype: medvol::DataType::F32,
        layout,
        affine,
        orientation: medvol::OrientationSource::Sform,
        dims,
        spacing,
        scale: None,
        cal: None,
        description: Some(description.to_owned()).filter(|d| !d.is_empty()),
        intent: Some(intent.to_owned()),
        anatomical: None,
        params: Vec::new(),
        data: medvol::DataRef {
            file: medvol::DataFile::Attached,
            skip: medvol::Skip::Start { lines: 0, bytes: 0 },
            encoding: medvol::Encoding::Raw,
            big_endian: false,
        },
    };
    medvol::MedVolume::from_bytes(header, Arc::new(bytes), 0, false)
        .map_err(|e| DecodeError::Malformed(e.to_string()))
}

/// A dense connectivity file without surfaces: the first seed's row as a volume, the
/// rows of other seeds read as they are picked in MPR.
fn dense_volume(
    source: Arc<Source>,
    c: Cifti,
    data: Data,
    mut facts: CiftiFacts,
) -> Result<Arc<dyn Session>> {
    facts.shown = "voxels";
    let conn = Arc::new(Connectivity::new(Arc::new(c), data, 0, &|_| None));
    let volume = conn.volume_of_seed(0)?;
    crate::medvol_dec::open_prepared(
        source,
        volume,
        Format::Cifti,
        NeuroFacts {
            cifti: Some(facts),
            connectivity: Some(ConnectivityModel(conn)),
            ..NeuroFacts::default()
        },
    )
}

// ---------------------------------------------------------------------------------------
// Tractograms

/// Tractograms (`.trk`, `.tck`) in the folder of `path`, by name: the streamlines MPR draws
/// over a volume. At most a few; none when the folder cannot be listed.
pub fn tracts_beside(path: &Path) -> Vec<PathBuf> {
    const MAX: usize = 8;
    let Some(dir) = path.parent() else {
        return Vec::new();
    };
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("trk") || x.eq_ignore_ascii_case("tck"))
                && p.is_file()
        })
        .take(MAX_CANDIDATES)
        .collect();
    out.sort();
    out.truncate(MAX);
    out
}

/// The streamlines of a tractogram file in RAS millimetres (TrackVis through its
/// voxel-to-RAS matrix, MRtrix scanner coordinates as written), subsampled to at most
/// [`MAX_SHOWN_POINTS`] points as the 3D view shows them.
pub fn read_tracts(path: &Path) -> Result<Tractogram> {
    let source = Source::open(path).map_err(DecodeError::Io)?;
    let data = source.bytes();
    let t = if trk::sniff(data) {
        trk::parse(data).map_err(map_err)?.1
    } else {
        tck::parse(data).map_err(map_err)?.1
    };
    Ok(t.subsample(MAX_SHOWN_POINTS))
}

fn open_tracts(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let (format, tracts, mut facts) = if trk::sniff(data) {
        let (h, t) = trk::parse(data).map_err(map_err)?;
        let facts = TractFacts {
            format: format!("TrackVis {}", h.version),
            scalars: h.scalars.clone(),
            properties: h.properties.clone(),
            dims: Some(h.dim),
            voxel_size: Some(h.voxel_size),
            voxel_order: Some(h.voxel_order.trim().to_owned()).filter(|s| !s.is_empty()),
            space: if h.has_vox_to_ras {
                "vox_to_ras"
            } else {
                "voxel order"
            },
            ..TractFacts::default()
        };
        (Format::Trk, t, facts)
    } else {
        let (h, t) = tck::parse(data).map_err(map_err)?;
        let facts = TractFacts {
            format: "MRtrix".into(),
            space: "scanner",
            step: h.field("step_size").map(str::to_owned),
            ..TractFacts::default()
        };
        (Format::Tck, t, facts)
    };
    if tracts.points.is_empty() {
        return Err(DecodeError::Malformed(
            "a tractogram without streamlines".into(),
        ));
    }
    facts.streamlines = tracts.streamlines();
    facts.points = tracts.points.len();
    let mut total = 0f64;
    for s in 0..tracts.streamlines() {
        for w in tracts.streamline(s).windows(2) {
            total += f64::from(dist(w[0], w[1]));
        }
    }
    facts.mean_length = (tracts.streamlines() > 0).then(|| total / tracts.streamlines() as f64);
    let shown = tracts.subsample(MAX_SHOWN_POINTS);
    facts.shown_points = shown.points.len();
    facts.shown_streamlines = shown.streamlines();
    let (lo, hi) = shown
        .bounds()
        .ok_or_else(|| DecodeError::Malformed("no finite points".into()))?;
    let (w, h) = crate::mesh_dec::preview_size_of(Bounds { min: lo, max: hi }, Up::Z);
    let info = ImageInfo {
        format,
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: true,
        bit_depth: 8,
        progressive: false,
        coding: format!(
            "tractogram, {} streamlines, {} points",
            facts.streamlines, facts.points
        ),
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(TractSession {
        source,
        info,
        model: TractsModel {
            tracts: Arc::new(shown),
        },
        facts: NeuroFacts {
            tracts: Some(facts),
            ..NeuroFacts::default()
        },
        cache: Mutex::new(None),
    }))
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The direction colour of a segment: |x| red, |y| green, |z| blue (the DTI convention).
pub fn direction_colour(a: [f32; 3], b: [f32; 3]) -> [u8; 4] {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if l <= 0.0 || !l.is_finite() {
        return [200, 200, 200, 255];
    }
    let c = d.map(|v| (v.abs() / l * 255.0).round().clamp(0.0, 255.0) as u8);
    [c[0], c[1], c[2], 255]
}

/// Renders streamlines, direction-coloured, from the isometric view to straight sRGB RGBA8.
pub fn render_tracts(t: &Tractogram, width: u32, height: u32) -> Vec<u8> {
    use specular_mesh::preview::ISO;
    let (w, h) = (width.max(1) as usize, height.max(1) as usize);
    let ss = 2usize;
    let (sw, sh) = (w * ss, h * ss);
    let mut colour = vec![[0u8; 4]; sw * sh];
    let mut depth = vec![f32::INFINITY; sw * sh];
    let Some((lo, hi)) = t.bounds() else {
        return vec![0; w * h * 4];
    };
    let [right, down, fwd] = ISO;
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let c = [0, 1, 2].map(|k| (lo[k] + hi[k]) / 2.0);
    let proj = |p: [f32; 3]| {
        let q = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
        [dot(q, right), dot(q, down), dot(q, fwd)]
    };
    // The projected extent from the box corners.
    let (mut plo, mut phi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    for i in 0..8 {
        let p = proj([
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ]);
        for k in 0..2 {
            plo[k] = plo[k].min(p[k]);
            phi[k] = phi[k].max(p[k]);
        }
    }
    let ext = [(phi[0] - plo[0]).max(1e-20), (phi[1] - plo[1]).max(1e-20)];
    let scale = 0.92 * (sw as f32 / ext[0]).min(sh as f32 / ext[1]);
    let mid = [(plo[0] + phi[0]) / 2.0, (plo[1] + phi[1]) / 2.0];
    let screen = |p: [f32; 3]| {
        let q = proj(p);
        [
            (q[0] - mid[0]) * scale + sw as f32 / 2.0,
            (q[1] - mid[1]) * scale + sh as f32 / 2.0,
            q[2],
        ]
    };
    // Line width: one output pixel per 512 of the long side (at least one), so the lines
    // survive fitting, export and thumbnails (they nearly vanished at 1 px on 1024).
    let pen = (((w.max(h) as f32 / 512.0).max(1.0) * ss as f32).round() as i64).max(ss as i64);
    let mut plot = |x: i64, y: i64, z: f32, c: [u8; 4]| {
        for (dx, dy) in (0..pen).flat_map(|dy| (0..pen).map(move |dx| (dx, dy))) {
            let (x, y) = (
                x + dx - (pen - ss as i64) / 2,
                y + dy - (pen - ss as i64) / 2,
            );
            if x < 0 || y < 0 || x >= sw as i64 || y >= sh as i64 {
                continue;
            }
            let i = y as usize * sw + x as usize;
            if z < depth[i] {
                depth[i] = z;
                colour[i] = c;
            }
        }
    };
    for s in 0..t.streamlines() {
        for seg in t.streamline(s).windows(2) {
            let (a, b) = (seg[0], seg[1]);
            if !(a.iter().chain(&b).all(|v| v.is_finite())) {
                continue;
            }
            let col = direction_colour(a, b);
            let (pa, pb) = (screen(a), screen(b));
            let steps = (pb[0] - pa[0])
                .abs()
                .max((pb[1] - pa[1]).abs())
                .ceil()
                .clamp(1.0, 4096.0) as usize;
            for k in 0..=steps {
                let f = k as f32 / steps as f32;
                let x = pa[0] + (pb[0] - pa[0]) * f;
                let y = pa[1] + (pb[1] - pa[1]) * f;
                let z = pa[2] + (pb[2] - pa[2]) * f;
                // Farther lines a little darker, for depth.
                plot(x as i64, y as i64, z, col);
            }
        }
    }
    // Box-filter down, with straight alpha.
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            for sy in 0..ss {
                for sx in 0..ss {
                    let p = colour[(y * ss + sy) * sw + x * ss + sx];
                    let a = u32::from(p[3]);
                    for k in 0..3 {
                        acc[k] += u32::from(p[k]) * a;
                    }
                    acc[3] += a;
                }
            }
            let o = (y * w + x) * 4;
            if acc[3] > 0 {
                for k in 0..3 {
                    out[o + k] = acc[k].checked_div(acc[3]).unwrap_or(0) as u8;
                }
                out[o + 3] = (acc[3] / (ss * ss) as u32) as u8;
            }
        }
    }
    out
}

struct TractSession {
    source: Arc<Source>,
    info: ImageInfo,
    model: TractsModel,
    facts: NeuroFacts,
    cache: Mutex<Option<(u32, Arc<Vec<u8>>)>>,
}

impl TractSession {
    fn rendered(&self, scale: u32) -> Arc<Vec<u8>> {
        if let Some((s, px)) = &*self.cache.lock()
            && *s == scale
        {
            return px.clone();
        }
        let (w, h) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let px = Arc::new(render_tracts(&self.model.tracts, w, h));
        *self.cache.lock() = Some((scale, px.clone()));
        px
    }

    fn tract_bytes(&self) -> usize {
        let t = &self.model.tracts;
        t.points.len() * 12
            + t.offsets.len() * 4
            + t.scalars.iter().map(|s| s.1.len() * 4).sum::<usize>()
            + t.properties.iter().map(|s| s.1.len() * 4).sum::<usize>()
    }
}

impl Session for TractSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        crop(&self.info, req, cancel, |s| self.rendered(s))
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        render_memory(&self.info, req)
    }
    fn memory_bytes(&self) -> usize {
        let cached = self.cache.lock().as_ref().map_or(0, |(_, px)| px.len());
        self.tract_bytes() + cached
    }
    fn trim_memory(&self) -> usize {
        self.cache.lock().take().map_or(0, |(_, px)| px.len())
    }
    fn tracts(&self) -> Option<&TractsModel> {
        Some(&self.model)
    }
    fn neuro(&self) -> Option<&NeuroFacts> {
        Some(&self.facts)
    }
}

#[cfg(test)]
mod tests;
