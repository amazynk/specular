//! HDF5 (`.h5`), NetCDF (`.nc`: classic CDF-1/2/5 and NetCDF-4) and EMD (`.emd`, Berkeley
//! and Velox electron microscopy) through `specular-hdf5` and `specular-netcdf`, shown as
//! scientific N-D images; NeXus, Imaris and BAG are HDF5 conventions read here too.
//!
//! A file holds many arrays; the ones that make images (numeric, two or more dimensions, not
//! tiny) become pages, the most image-like first: the EMD convention's image datasets, the
//! data variables of a NetCDF file (not its coordinates or bounds), or for plain HDF5 the
//! datasets with conventional names (`data`, `image`…) or the `IMAGE` class before the
//! rest, larger before smaller, more dimensions before fewer. The last dimension runs along
//! x, the one before it down y, the others become plane sliders (with coordinate values
//! where the file has them); a trailing dimension of 3 or 4 on 8/16-bit data is a colour
//! axis. NetCDF grids whose y coordinate grows northwards are flipped so north is up.
//!
//! * **NeXus** (`.nxs`): the `@default` chain (root → `NXentry` → `NXdata`) names the
//!   signal, which comes first; the group's `axes` and `AXISNAME_indices` name the slider
//!   axes and give their values (and a pixel calibration when x and y are linear).
//! * **External links** (an Eiger master and its `data_NNNNNN` files) are followed read-only,
//!   relative to the master's folder; numbered data files of one shape are stacked into one
//!   frame axis. A link that leaves the master's folder is refused unless the user opened its
//!   target explicitly ([`allow_linked_file`]); refused and missing links are listed with
//!   the header keywords.
//! * **Imaris** (`.ims`): one image with channel, time and z sliders, the voxel size from the
//!   extents; the block padding past the image is cropped, and the coarser resolution levels
//!   serve reduced decodes.
//! * **BAG** (bathymetry): elevation and uncertainty grids, the 1e6 no-data value as NaN,
//!   rows turned north up, the grid's position and resolution from its ISO 19115 metadata.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_gis::geotiff::Affine2;
use specular_hdf5::{self as hdf5, Num, PlaneSpec, Value};
use specular_netcdf as netcdf;

use crate::geo::{GeoFacts, GeoRef, RasterGeo};
use crate::science::{
    self, AxisKind, Linear, NdAxis, NdPlane, PixelCalibration, PlaneCache, ScienceKind,
    ScienceSummary,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

/// Pages listed at most.
const MAX_PAGES: usize = 1024;
/// Values of one plane at most.
const MAX_PLANE: u64 = 1 << 28;
/// Positions on an axis given labels at most.
const MAX_LABELS: u64 = 1 << 16;
/// Header lines listed at most.
const MAX_KEYWORDS: usize = 4096;
/// External links followed at most.
const MAX_LINKS: usize = 4096;
/// The NeXus signal's ranking bonus, and its `NXdata` group's.
const NEXUS_SIGNAL: i64 = 1000;
const NEXUS_GROUP: i64 = 500;
/// BAG's no-data value.
const BAG_NODATA: f32 = 1.0e6;

/// Files the user opened explicitly (canonical paths): external links may reach them from
/// another folder.
static OPENED: parking_lot::Mutex<Vec<PathBuf>> = parking_lot::const_mutex(Vec::new());

/// Lets HDF5 external links reach `path` although it lies outside the linking file's folder:
/// the viewer calls this for every file the user opens explicitly.
pub fn allow_linked_file(path: &Path) {
    let Ok(c) = path.canonicalize() else {
        return;
    };
    let mut v = OPENED.lock();
    if !v.contains(&c) {
        if v.len() >= 1024 {
            v.remove(0);
        }
        v.push(c);
    }
}

/// Why an external link was not followed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LinkRefusal {
    Missing,
    Outside,
}

/// The file an external link names, relative to the linking file's folder (HDF5 also tries
/// the bare file name there), confined to that folder and its subfolders unless the user
/// opened the target explicitly.
pub(crate) fn resolve_link(master: &Path, name: &str) -> std::result::Result<PathBuf, LinkRefusal> {
    let master = master.canonicalize().map_err(|_| LinkRefusal::Missing)?;
    let dir = master.parent().ok_or(LinkRefusal::Missing)?;
    let rel = Path::new(name);
    let mut tries = vec![if rel.is_absolute() {
        rel.to_path_buf()
    } else {
        dir.join(rel)
    }];
    if let Some(base) = rel.file_name() {
        tries.push(dir.join(base));
    }
    let mut outside = false;
    for t in tries {
        let Ok(c) = t.canonicalize() else {
            continue;
        };
        if !c.is_file() || c == master {
            continue;
        }
        if c.starts_with(dir) || OPENED.lock().contains(&c) {
            return Ok(c);
        }
        outside = true;
    }
    Err(if outside {
        LinkRefusal::Outside
    } else {
        LinkRefusal::Missing
    })
}

pub(crate) fn map_hdf5(e: hdf5::Error) -> DecodeError {
    match e {
        hdf5::Error::NotHdf5 => DecodeError::UnsupportedFormat,
        hdf5::Error::Malformed(s) => DecodeError::Malformed(s),
        hdf5::Error::Unsupported(s) => DecodeError::Unsupported(s),
        hdf5::Error::TooLarge => DecodeError::TooLarge,
        hdf5::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        hdf5::Error::Cancelled => DecodeError::Cancelled,
    }
}

/// Bytes of compressed chunks a sampled thumbnail may decompress (see
/// [`hdf5::File::read_plane_strided`]).
const SAMPLE_BUDGET: u64 = 64 << 20;
/// How long a sampled thumbnail may take before it gives up.
const SAMPLE_TIME: std::time::Duration = std::time::Duration::from_secs(4);

/// A plane of an HDF5 dataset, whole or every `step`-th sample.
/// `resolve` gives the other files a virtual dataset maps from.
fn read_plane_h5<'a>(
    f: &'a hdf5::File,
    data: &'a [u8],
    var: usize,
    spec: &PlaneSpec,
    step: u64,
    stop: &(dyn Fn() -> bool + Sync),
    resolve: &hdf5::Resolve<'a>,
) -> Result<hdf5::Plane<f32>> {
    if step > 1 {
        f.read_plane_strided_in::<f32>(data, var, spec, step, SAMPLE_BUDGET, stop, resolve)
    } else {
        f.read_plane_in::<f32>(data, var, spec, stop, resolve)
    }
    .map_err(map_hdf5)
}

/// No other files for virtual datasets.
fn no_files<'a>(_: &str) -> Option<(&'a hdf5::File, &'a [u8])> {
    None
}

pub(crate) fn map_nc(e: netcdf::Error) -> DecodeError {
    match e {
        netcdf::Error::NotNetcdf => DecodeError::UnsupportedFormat,
        netcdf::Error::Malformed(s) => DecodeError::Malformed(s),
        netcdf::Error::Unsupported(s) => DecodeError::Unsupported(s),
        netcdf::Error::TooLarge => DecodeError::TooLarge,
        netcdf::Error::Cancelled => DecodeError::Cancelled,
        netcdf::Error::Hdf5(e) => map_hdf5(e),
        netcdf::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

/// HDF5 files, NetCDF-4 and EMD included (all start with the HDF5 signature, possibly after
/// a user block).
pub struct Hdf5Decoder;

/// Classic NetCDF files ("CDF\x01", "CDF\x02", "CDF\x05").
pub struct NetcdfDecoder;

impl Decoder for Hdf5Decoder {
    fn format(&self) -> Format {
        Format::Hdf5
    }
    fn probe(&self, head: &[u8]) -> bool {
        hdf5::sniff(head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        hdf5::find_superblock(data).is_some()
    }
    fn extensions(&self) -> &'static [&'static str] {
        // BAG (`.bag`, shared with ROS bags) is claimed by content only (see `crate::sniff`).
        &["h5", "hdf5", "he5", "emd", "nc", "nc4", "nxs", "nx5", "ims"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        // An Alembic archive in its older HDF5 container: refused with the reason.
        if source
            .path()
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("abc"))
        {
            return Err(
                match specular_mesh::alembic::read(source.bytes(), &Default::default()) {
                    Err(specular_mesh::Error::Unsupported(m)) => DecodeError::Unsupported(m),
                    _ => DecodeError::UnsupportedFormat,
                },
            );
        }
        let file = hdf5::File::open(source.bytes()).map_err(map_hdf5)?;
        // NetCDF-4 with an unstructured mesh (UGRID, ICON, MPAS): the mesh's own session.
        let nc_ext = source
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("nc") || e.eq_ignore_ascii_case("nc4"));
        if (netcdf::is_netcdf4(&file) || nc_ext) && hdf5::emd::detect(&file).is_none() {
            let nc = netcdf::Netcdf::from_hdf5(file);
            if let Some(topo) = specular_ugrid::detect(&nc) {
                return crate::ugrid_dec::open(source, nc, topo);
            }
            let model = Model::from_netcdf(nc, source.bytes())?;
            return Ok(Arc::new(H5Session::new(source, Arc::new(model), 0)?));
        }
        let model = Model::from_hdf5(file, &source)?;
        Ok(Arc::new(H5Session::new(source, Arc::new(model), 0)?))
    }
}

impl Decoder for NetcdfDecoder {
    fn format(&self) -> Format {
        Format::NetCdf
    }
    fn probe(&self, head: &[u8]) -> bool {
        netcdf::sniff_classic(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["nc"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let nc = netcdf::Netcdf::open(source.bytes(), &|| false).map_err(map_nc)?;
        if let Some(topo) = specular_ugrid::detect(&nc) {
            return crate::ugrid_dec::open(source, nc, topo);
        }
        let model = Model::from_netcdf(nc, source.bytes())?;
        Ok(Arc::new(H5Session::new(source, Arc::new(model), 0)?))
    }
}

enum Backend {
    Hdf5(hdf5::File),
    Netcdf(netcdf::Netcdf),
}

/// One array shown as an image.
#[derive(Clone, Debug, Default)]
struct Candidate {
    /// HDF5 object index, or NetCDF variable index.
    var: usize,
    /// The linked file ([`Model::linked`]) holding `var`, else the file itself.
    file: Option<usize>,
    /// Datasets stacked along dimension 0 (an Eiger master's data files), when several.
    parts: Vec<Part>,
    /// An Imaris image: channels, time points and resolution levels.
    imaris: Option<Arc<Imaris>>,
    /// A value meaning "no data", read as NaN.
    nodata: Option<f32>,
    /// What the summary says beyond the dataset's own attributes.
    extra: Option<Arc<Extra>>,
    shape: Vec<u64>,
    num: Option<Num>,
    x_dim: usize,
    y_dim: usize,
    /// A colour dimension (red, green, blue).
    colour: Option<usize>,
    /// Rows are stored bottom-up (south first).
    flip_y: bool,
    /// Ranking score, higher first.
    score: i64,
    /// What the EMD convention says about the image.
    emd: Option<hdf5::emd::Image>,
}

/// One dataset of a stack.
#[derive(Clone, Copy, Debug)]
struct Part {
    file: Option<usize>,
    var: usize,
    /// Its length along dimension 0.
    len: u64,
}

/// An Imaris image: `Data` datasets per resolution level, channel and time point.
#[derive(Debug)]
struct Imaris {
    times: usize,
    levels: Vec<ImarisLevel>,
}

#[derive(Debug)]
struct ImarisLevel {
    /// z, y, x (the stored blocks may be padded past them).
    size: [u64; 3],
    /// The `Data` dataset of channel `c`, time point `t` at `c * times + t`.
    vars: Vec<Option<usize>>,
    /// 1/scale of the full image this level is, or 0 when its x and y are not both halved.
    scale: u32,
}

/// Name, unit, labels and calibration of one dimension.
#[derive(Clone, Debug, Default)]
struct AxisMeta {
    name: String,
    unit: String,
    labels: Vec<String>,
    linear: Option<Linear>,
    kind: Option<AxisKind>,
}

/// Facts a convention (NeXus, Imaris, BAG, an external link) adds to a candidate.
#[derive(Debug, Default)]
struct Extra {
    /// The path shown, when not the dataset's own (an external link, a stack).
    path: Option<String>,
    /// Per dataset dimension, overriding what the dataset says.
    axes: Vec<Option<AxisMeta>>,
    pixel: Option<PixelCalibration>,
    kind: Option<ScienceKind>,
    title: Option<String>,
    instrument: Option<String>,
    detector: Option<String>,
    unit: Option<String>,
    keywords: Vec<(String, String, String)>,
}

/// A file reached through an external link.
struct Linked {
    source: Arc<Source>,
    file: hdf5::File,
}

struct Model {
    format: Format,
    backend: Backend,
    linked: Vec<Linked>,
    /// Lines for every page's keywords (external links refused or not found).
    notes: Vec<(String, String, String)>,
    pages: Vec<Candidate>,
    /// The other files virtual datasets map from: (name as the mapping gives it, index into
    /// `linked`).
    virtual_files: Vec<(String, usize)>,
}

impl Model {
    fn plain(format: Format, backend: Backend, pages: Vec<Candidate>) -> Model {
        Model {
            format,
            backend,
            linked: Vec::new(),
            notes: Vec::new(),
            pages,
            virtual_files: Vec::new(),
        }
    }

    /// A file a virtual dataset of the main file maps from, when it was found.
    fn virtual_file(&self, name: &str) -> Option<(&hdf5::File, &[u8])> {
        let &(_, k) = self.virtual_files.iter().find(|(n, _)| n == name)?;
        self.linked.get(k).map(|l| (&l.file, l.source.bytes()))
    }

    /// The HDF5 file (and its bytes) a candidate's dataset lives in.
    fn h5<'a>(&'a self, main: &'a [u8], file: Option<usize>) -> Option<(&'a hdf5::File, &'a [u8])> {
        match (file, &self.backend) {
            (None, Backend::Hdf5(f)) => Some((f, main)),
            (Some(i), _) => self.linked.get(i).map(|l| (&l.file, l.source.bytes())),
            _ => None,
        }
    }
}

/// Dataset names that usually hold the image.
const GOOD_NAMES: &[&str] = &[
    "data",
    "image",
    "images",
    "img",
    "raw",
    "frame",
    "frames",
    "array",
    "intensity",
    "counts",
    "signal",
    "stack",
    "volume",
    "spectrum_image",
    "dataset",
    "values",
];
/// Names that usually hold side information.
const SIDE_NAMES: &[&str] = &[
    "mask",
    "masks",
    "error",
    "errors",
    "err",
    "sigma",
    "uncertainty",
    "variance",
    "var",
    "weights",
    "weight",
    "flag",
    "flags",
    "quality",
    "dq",
    "x",
    "y",
    "z",
    "lat",
    "lon",
    "latitude",
    "longitude",
    "time",
    "coords",
    "coordinates",
    "index",
    "indices",
    "bins",
    "axis",
    "axes",
    "grid",
    "dim",
    "thumbnail",
    "preview",
];

fn name_score(name: &str) -> i64 {
    let n = name.to_ascii_lowercase();
    if GOOD_NAMES.contains(&n.as_str()) {
        50
    } else if SIDE_NAMES.contains(&n.as_str())
        || n.ends_with("_bnds")
        || n.ends_with("_bounds")
        || n.starts_with("dim")
    {
        -50
    } else {
        0
    }
}

/// The plane dimensions of a shape (last two), a colour dimension when the last is 3 or 4
/// on small integers, and whether it makes an image at all.
fn layout(shape: &[u64], num: Num, interlace_plane: bool) -> Option<(usize, usize, Option<usize>)> {
    let r = shape.len();
    if r < 2 || shape.contains(&0) {
        return None;
    }
    let small_int = matches!(num, Num::U8 | Num::U16);
    let (x, y, colour) = if r == 3 && small_int && interlace_plane && matches!(shape[0], 3 | 4) {
        (2, 1, Some(0))
    } else if r >= 3 && small_int && matches!(shape[r - 1], 3 | 4) && shape[r - 2] > 4 {
        (r - 2, r - 3, Some(r - 1))
    } else {
        (r - 1, r - 2, None)
    };
    let (w, h) = (shape[x], shape[y]);
    if w < 2 || h < 2 || w * h < 16 || w > u64::from(u32::MAX) || h > u64::from(u32::MAX) {
        return None;
    }
    if w.saturating_mul(h) > MAX_PLANE {
        return None;
    }
    Some((x, y, colour))
}

fn size_score(shape: &[u64], x: usize, y: usize) -> i64 {
    let px = shape[x].saturating_mul(shape[y]).max(1);
    // log2 of the plane size: bigger first, as a tie-breaker below the name scores.
    i64::from(63 - px.leading_zeros())
}

impl Model {
    fn from_hdf5(file: hdf5::File, source: &Source) -> Result<Model> {
        let data = source.bytes();
        let ext = source
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if hdf5::emd::detect(&file).is_some() {
            let mut pages = Vec::new();
            for im in hdf5::emd::images(&file, data) {
                let Some(ds) = file.objects.get(im.index).and_then(|o| o.dataset()) else {
                    continue;
                };
                let Some((num, _)) = ds.dtype.num() else {
                    continue;
                };
                let shape = ds.shape().to_vec();
                let (w, h) = (shape[im.x_dim], shape[im.y_dim]);
                if w == 0
                    || h == 0
                    || w > u64::from(u32::MAX)
                    || h > u64::from(u32::MAX)
                    || w * h > MAX_PLANE
                {
                    continue;
                }
                pages.push(Candidate {
                    var: im.index,
                    num: Some(num),
                    x_dim: im.x_dim,
                    y_dim: im.y_dim,
                    shape,
                    emd: Some(im),
                    ..Default::default()
                });
            }
            if !pages.is_empty() {
                pages.truncate(MAX_PAGES);
                return Ok(Model::plain(Format::Emd, Backend::Hdf5(file), pages));
            }
        }
        if netcdf::is_netcdf4(&file) || matches!(ext.as_str(), "nc" | "nc4") {
            return Model::from_netcdf(netcdf::Netcdf::from_hdf5(file), data);
        }
        if let Some(pages) = imaris_pages(&file) {
            return Ok(Model::plain(Format::Imaris, Backend::Hdf5(file), pages));
        }
        if let Some(pages) = bag_pages(&file, data) {
            return Ok(Model::plain(Format::Bag, Backend::Hdf5(file), pages));
        }
        let nexus = nexus_default(&file);
        let mut pages = Vec::new();
        // Compound datasets show each numeric field as a dataset of its own.
        for (i, o, ds) in file.datasets().chain(file.fields()) {
            let Some((num, _)) = ds.dtype.num() else {
                continue;
            };
            if o.attr("CLASS").and_then(Value::as_str) == Some("DIMENSION_SCALE") {
                continue;
            }
            let image_class = o.attr("CLASS").and_then(Value::as_str) == Some("IMAGE");
            let plane_interlace =
                o.attr("INTERLACE_MODE").and_then(Value::as_str) == Some("INTERLACE_PLANE");
            let shape = ds.shape().to_vec();
            let Some((x, y, colour)) = layout(&shape, num, plane_interlace) else {
                continue;
            };
            let score = name_score(o.name())
                + if image_class { 60 } else { 0 }
                + size_score(&shape, x, y)
                + nexus_score(nexus.as_ref(), &o.path, o);
            let extra = nexus
                .as_ref()
                .and_then(|n| n.extra(&file, data, &o.path, &shape, x, y));
            pages.push(Candidate {
                var: i,
                num: Some(num),
                x_dim: x,
                y_dim: y,
                colour,
                score,
                shape,
                extra,
                ..Default::default()
            });
        }
        let mut linked = Vec::new();
        let mut notes = Vec::new();
        let virtual_files = virtual_sources(&file, source.path(), &mut linked, &mut notes);
        pages.extend(link_pages(
            &file,
            data,
            source.path(),
            nexus.as_ref(),
            &mut linked,
            &mut notes,
        ));
        if pages.is_empty() {
            return Err(DecodeError::Unsupported(match notes.first() {
                Some((_, why, _)) => format!("no image dataset in the HDF5 file ({why})"),
                None => "no image dataset in the HDF5 file".into(),
            }));
        }
        pages.sort_by_key(|c| std::cmp::Reverse((c.score, c.shape.len())));
        pages.truncate(MAX_PAGES);
        let nexus_ext = matches!(ext.as_str(), "nxs" | "nx5");
        Ok(Model {
            format: if nexus.is_some() || nexus_ext {
                Format::Nexus
            } else {
                Format::Hdf5
            },
            backend: Backend::Hdf5(file),
            linked,
            notes,
            pages,
            virtual_files,
        })
    }

    fn from_netcdf(nc: netcdf::Netcdf, data: &[u8]) -> Result<Model> {
        // Variables named by others' `coordinates` or `bounds` are side information.
        let mut side: Vec<&str> = Vec::new();
        for v in &nc.vars {
            for a in ["coordinates", "bounds", "ancillary_variables"] {
                if let Some(s) = v.text(a) {
                    side.extend(s.split_whitespace());
                }
            }
        }
        let side: Vec<String> = side.into_iter().map(str::to_owned).collect();
        let mut pages = Vec::new();
        for (i, v) in nc.vars.iter().enumerate() {
            let Some(num) = v.num else {
                continue;
            };
            let Some((x, y, colour)) = layout(&v.shape, num, false) else {
                continue;
            };
            let base = v.name.rsplit('/').next().unwrap_or(&v.name);
            let score = if side.iter().any(|s| s == base) {
                -100
            } else {
                0
            } + name_score(base).min(0)
                + size_score(&v.shape, x, y);
            pages.push(Candidate {
                var: i,
                num: Some(num),
                x_dim: x,
                y_dim: y,
                colour,
                flip_y: north_up_flip(&nc, data, &v.dims[y]),
                score,
                shape: v.shape.clone(),
                ..Default::default()
            });
        }
        if pages.is_empty() {
            return Err(DecodeError::Unsupported(
                "no image variable in the NetCDF file".into(),
            ));
        }
        pages.sort_by_key(|c| std::cmp::Reverse((c.score, c.shape.len())));
        pages.truncate(MAX_PAGES);
        Ok(Model::plain(Format::NetCdf, Backend::Netcdf(nc), pages))
    }
}

/// A text attribute, trimmed (Imaris writes text as arrays of one-character strings).
fn attr_text(o: &hdf5::Object, name: &str) -> Option<String> {
    o.attr(name)
        .and_then(Value::text)
        .map(|s| {
            s.trim_matches(|c: char| c == '\0' || c.is_whitespace())
                .to_owned()
        })
        .filter(|s| !s.is_empty())
}

/// `name` relative to the group `group` (absolute names stay).
fn join_path(group: &str, name: &str) -> String {
    if name.starts_with('/') {
        name.to_owned()
    } else if group == "/" {
        format!("/{name}")
    } else {
        format!("{group}/{name}")
    }
}

/// A group's or dataset's parent path.
fn parent_path(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) | None => "/",
        Some(i) => &path[..i],
    }
}

/// Values as a linear function of their index, when they are one.
fn linear_of(v: &[f64]) -> Option<Linear> {
    let n = v.len();
    if n < 2 || v.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let step = (v[n - 1] - v[0]) / (n - 1) as f64;
    let tol = (step.abs() * n as f64).max(v[0].abs()) * 1e-6;
    if step == 0.0
        || v.iter()
            .enumerate()
            .any(|(i, x)| (x - (v[0] + step * i as f64)).abs() > tol)
    {
        return None;
    }
    Some(Linear { offset: v[0], step })
}

/// What the NeXus `@default` chain names: the `NXdata` group, its signal and its axes.
struct Nexus {
    group: String,
    signal: Option<String>,
    /// Axis dataset paths per signal dimension.
    axes: Vec<(usize, String)>,
}

/// Follows root `@default` → `NXentry` (`@default`, else the first) → `NXdata` (`@default`,
/// else the first).
fn nexus_default(file: &hdf5::File) -> Option<Nexus> {
    let class = |o: &hdf5::Object| attr_text(o, "NX_class");
    let exists = |p: &String| file.get(p).is_some();
    let root = file.get("/")?;
    let entry = attr_text(root, "default")
        .map(|d| join_path("/", &d))
        .filter(exists)
        .or_else(|| {
            file.children("/")
                .find(|o| class(o).as_deref() == Some("NXentry"))
                .map(|o| o.path.clone())
        })?;
    let eo = file.get(&entry)?;
    let group = if class(eo).as_deref() == Some("NXdata") {
        entry.clone()
    } else {
        attr_text(eo, "default")
            .map(|d| join_path(&entry, &d))
            .filter(exists)
            .or_else(|| {
                file.children(&entry)
                    .find(|o| class(o).as_deref() == Some("NXdata"))
                    .map(|o| o.path.clone())
            })?
    };
    let g = file.get(&group)?;
    let signal = attr_text(g, "signal").map(|s| join_path(&group, &s));
    let names: Vec<String> = match g.attr("axes") {
        Some(Value::Str(v)) if v.len() == 1 => v[0]
            .split([':', ','])
            .map(|s| s.trim().to_owned())
            .collect(),
        Some(Value::Str(v)) => v.iter().map(|s| s.trim().to_owned()).collect(),
        _ => Vec::new(),
    };
    let mut axes = Vec::new();
    for (pos, n) in names.iter().enumerate() {
        if n.is_empty() || n == "." {
            continue;
        }
        let dim = g
            .attr(&format!("{n}_indices"))
            .and_then(Value::as_f64)
            .filter(|d| *d >= 0.0 && *d < 32.0)
            .map_or(pos, |d| d as usize);
        axes.push((dim, join_path(&group, n)));
    }
    Some(Nexus {
        group,
        signal,
        axes,
    })
}

/// The NeXus ranking bonus of a dataset (or a link) at `path`.
fn nexus_score(n: Option<&Nexus>, path: &str, o: &hdf5::Object) -> i64 {
    let old_signal = o.attr("signal").and_then(Value::as_f64) == Some(1.0);
    let Some(n) = n else {
        return if old_signal { 200 } else { 0 };
    };
    if n.signal.as_deref() == Some(path) {
        NEXUS_SIGNAL
    } else if parent_path(path) == n.group {
        NEXUS_GROUP + if old_signal { 200 } else { 0 }
    } else if old_signal {
        200
    } else {
        0
    }
}

impl Nexus {
    /// The axes (values, units, a pixel calibration) of a dataset in the `NXdata` group.
    fn extra(
        &self,
        file: &hdf5::File,
        data: &[u8],
        path: &str,
        shape: &[u64],
        x: usize,
        y: usize,
    ) -> Option<Arc<Extra>> {
        if parent_path(path) != self.group {
            return None;
        }
        let mut e = Extra {
            axes: vec![None; shape.len()],
            kind: Some(ScienceKind::Gridded),
            ..Default::default()
        };
        let mut lin = [None, None];
        for (dim, ap) in &self.axes {
            let Some(&len) = shape.get(*dim) else {
                continue;
            };
            let Some(i) = file.index_of(ap) else {
                continue;
            };
            let o = &file.objects[i];
            if len > MAX_LABELS || o.dataset().is_none_or(|d| d.shape() != [len]) {
                continue;
            }
            let Ok(v) = file.read_all::<f64>(data, i, MAX_LABELS) else {
                continue;
            };
            let unit = attr_text(o, "units").unwrap_or_default();
            if *dim == x {
                lin[0] = linear_of(&v).map(|l| (l, unit.clone()));
            }
            if *dim == y {
                lin[1] = linear_of(&v).map(|l| (l, unit.clone()));
            }
            e.axes[*dim] = Some(AxisMeta {
                name: attr_text(o, "long_name").unwrap_or_else(|| o.name().to_owned()),
                labels: coord_labels(&v, &unit, None),
                linear: linear_of(&v),
                unit,
                kind: None,
            });
        }
        if let [Some((lx, ux)), Some((ly, uy))] = lin {
            e.pixel = Some(PixelCalibration {
                x: lx,
                y: ly,
                unit_x: ux,
                unit_y: uy,
            });
        }
        Some(Arc::new(e))
    }
}

/// The datasets behind a file's external links: each resolved, opened read-only and kept in
/// `linked`; numbered links of one group and shape (`data_000001`, `data_000002`…) make one
/// stack. Links refused or not found go to `notes`.
fn link_pages(
    file: &hdf5::File,
    data: &[u8],
    master: &Path,
    nexus: Option<&Nexus>,
    linked: &mut Vec<Linked>,
    notes: &mut Vec<(String, String, String)>,
) -> Vec<Candidate> {
    let _ = data;
    // (link path, linked file, dataset index, shape, type)
    let mut found: Vec<(String, usize, usize, Vec<u64>, Num)> = Vec::new();
    let mut opened: Vec<(PathBuf, std::result::Result<usize, String>)> = Vec::new();
    for (lpath, fname, tpath) in file.external_links().take(MAX_LINKS) {
        let note = |why: &str| {
            (
                "external link".to_owned(),
                format!("{lpath} → {fname}:{tpath}: {why}"),
                String::new(),
            )
        };
        let target = match resolve_link(master, fname) {
            Ok(t) => t,
            Err(LinkRefusal::Missing) => {
                notes.push(note("not found"));
                continue;
            }
            Err(LinkRefusal::Outside) => {
                notes.push(note("outside the file's folder, not followed"));
                continue;
            }
        };
        let k = match opened.iter().find(|(p, _)| *p == target) {
            Some((_, r)) => r.clone(),
            None => {
                let r = Source::open(&target)
                    .map_err(|e| e.to_string())
                    .and_then(|src| {
                        let f = hdf5::File::open(src.bytes()).map_err(|e| e.to_string())?;
                        linked.push(Linked {
                            source: Arc::new(src),
                            file: f,
                        });
                        Ok(linked.len() - 1)
                    });
                opened.push((target.clone(), r.clone()));
                r
            }
        };
        let k = match k {
            Ok(k) => k,
            Err(e) => {
                notes.push(note(&e));
                continue;
            }
        };
        let lf = &linked[k].file;
        let Some(i) = lf.index_of(tpath) else {
            notes.push(note("not found"));
            continue;
        };
        let Some((num, _)) = lf.objects[i].dataset().and_then(|d| d.dtype.num()) else {
            continue;
        };
        let shape = lf.objects[i]
            .dataset()
            .map(|d| d.shape().to_vec())
            .unwrap_or_default();
        found.push((lpath.to_owned(), k, i, shape, num));
    }
    // Group numbered links: same group, same name stem, same trailing shape and type.
    let stem = |p: &str| {
        let n = p.rsplit('/').next().unwrap_or(p);
        let s = n.trim_end_matches(|c: char| c.is_ascii_digit());
        let digits = &n[s.len()..];
        (
            format!("{}/{s}", parent_path(p)),
            digits.parse::<u64>().ok(),
        )
    };
    let mut used = vec![false; found.len()];
    let mut pages = Vec::new();
    for i in 0..found.len() {
        if used[i] {
            continue;
        }
        let (st, num_i) = stem(&found[i].0);
        let mut members = vec![i];
        if num_i.is_some() && found[i].3.len() >= 3 {
            for j in i + 1..found.len() {
                let (sj, nj) = stem(&found[j].0);
                if !used[j]
                    && nj.is_some()
                    && sj == st
                    && found[j].3[1..] == found[i].3[1..]
                    && found[j].4 == found[i].4
                {
                    members.push(j);
                }
            }
            members.sort_by_key(|&m| stem(&found[m].0).1);
        }
        for &m in &members {
            used[m] = true;
        }
        let first = &found[members[0]];
        let mut shape = first.3.clone();
        let parts: Vec<Part> = if members.len() > 1 {
            shape[0] = members.iter().map(|&m| found[m].3[0]).sum();
            members
                .iter()
                .map(|&m| Part {
                    file: Some(found[m].1),
                    var: found[m].2,
                    len: found[m].3[0],
                })
                .collect()
        } else {
            Vec::new()
        };
        let Some((x, y, colour)) = layout(&shape, first.4, false) else {
            continue;
        };
        let path = if members.len() > 1 {
            let last = &found[members[members.len() - 1]].0;
            format!("{} … {}", first.0, last.rsplit('/').next().unwrap_or(last))
        } else {
            first.0.clone()
        };
        let name = st
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(['_', '-']);
        let score = name_score(if members.len() > 1 {
            name
        } else {
            file_name_of(&first.0)
        }) + size_score(&shape, x, y)
            + file
                .get(&first.0)
                .map_or(0, |o| nexus_score(nexus, &first.0, o));
        pages.push(Candidate {
            var: first.2,
            file: Some(first.1),
            parts,
            shape,
            num: Some(first.4),
            x_dim: x,
            y_dim: y,
            colour,
            score,
            extra: Some(Arc::new(Extra {
                path: Some(path),
                kind: nexus.map(|_| ScienceKind::Gridded),
                detector: nexus_detector(file, data),
                ..Default::default()
            })),
            ..Default::default()
        });
    }
    pages
}

/// Opens the other files the virtual datasets of `file` map from (in the folder rules of
/// external links); those not reached become notes, and their elements read as the fill
/// value.
fn virtual_sources(
    file: &hdf5::File,
    master: &Path,
    linked: &mut Vec<Linked>,
    notes: &mut Vec<(String, String, String)>,
) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    let mut tried: Vec<String> = Vec::new();
    for (_, o, ds) in file.datasets() {
        for name in ds.virtual_files() {
            if tried.len() >= MAX_LINKS || tried.iter().any(|t| t == name) {
                continue;
            }
            tried.push(name.to_owned());
            let note = |why: &str| {
                (
                    "virtual dataset".to_owned(),
                    format!("{} → {name}: {why}", o.path),
                    String::new(),
                )
            };
            let target = match resolve_link(master, name) {
                Ok(t) => t,
                Err(LinkRefusal::Missing) => {
                    notes.push(note("not found"));
                    continue;
                }
                Err(LinkRefusal::Outside) => {
                    notes.push(note("outside the file's folder, not followed"));
                    continue;
                }
            };
            match Source::open(&target)
                .map_err(|e| e.to_string())
                .and_then(|src| {
                    let f = hdf5::File::open(src.bytes()).map_err(|e| e.to_string())?;
                    Ok((src, f))
                }) {
                Ok((src, f)) => {
                    linked.push(Linked {
                        source: Arc::new(src),
                        file: f,
                    });
                    out.push((name.to_owned(), linked.len() - 1));
                }
                Err(e) => notes.push(note(&e)),
            }
        }
    }
    out
}

fn file_name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A NeXus detector's description (`NXdetector/description`).
fn nexus_detector(file: &hdf5::File, data: &[u8]) -> Option<String> {
    let (i, _, _) = file.datasets().find(|(_, o, _)| {
        o.name() == "description"
            && file
                .get(parent_path(&o.path))
                .and_then(|g| attr_text(g, "NX_class"))
                .as_deref()
                == Some("NXdetector")
    })?;
    string_dataset(file, data, i)
}

/// A string dataset's text (fixed-length, or a 1-D array of characters).
fn string_dataset(file: &hdf5::File, data: &[u8], i: usize) -> Option<String> {
    let ds = file.objects.get(i)?.dataset()?;
    let shape = ds.shape().to_vec();
    if shape.iter().product::<u64>() > 1 << 24 {
        return None;
    }
    let start = vec![0; shape.len()];
    let bytes = file
        .read_box_bytes(data, i, &start, &shape, &|| false)
        .ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let t = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    (!t.is_empty()).then(|| t.replace('\0', ""))
}

/// An Imaris 5.5 file (`/DataSet/ResolutionLevel N/TimePoint T/Channel C/Data`): one page
/// with channel, time and z axes.
fn imaris_pages(file: &hdf5::File) -> Option<Vec<Candidate>> {
    let root = file.get("/")?;
    if root.attr("ImarisDataSet").is_none() && file.get("/DataSet/ResolutionLevel 0").is_none() {
        return None;
    }
    let dir = attr_text(root, "DataSetDirectoryName").unwrap_or_else(|| "DataSet".into());
    let info_dir =
        attr_text(root, "DataSetInfoDirectoryName").unwrap_or_else(|| "DataSetInfo".into());
    let level_path = |r: usize| format!("/{dir}/ResolutionLevel {r}");
    let count = |g: &str, what: &str| {
        (0..4096)
            .take_while(|k| file.get(&format!("{g}/{what} {k}")).is_some())
            .count()
    };
    let l0 = level_path(0);
    let times = count(&l0, "TimePoint");
    let channels = count(&format!("{l0}/TimePoint 0"), "Channel");
    if times == 0 || channels == 0 {
        return None;
    }
    let image = file.get(&format!("/{info_dir}/Image"));
    let num_attr = |o: Option<&hdf5::Object>, k: &str| -> Option<f64> {
        attr_text(o?, k)?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    };
    let mut levels: Vec<ImarisLevel> = Vec::new();
    let mut num = None;
    for r in 0..64 {
        let lp = level_path(r);
        if file.get(&lp).is_none() {
            break;
        }
        let mut vars = Vec::with_capacity(channels * times);
        let mut size = None;
        for c in 0..channels {
            for t in 0..times {
                let g = format!("{lp}/TimePoint {t}/Channel {c}");
                let v = file.index_of(&format!("{g}/Data"));
                if let Some(i) = v
                    && let Some(ds) = file.objects[i].dataset()
                    && ds.shape().len() == 3
                {
                    if num.is_none() {
                        num = ds.dtype.num().map(|n| n.0);
                    }
                    if size.is_none() {
                        let go = file.get(&g);
                        let st = ds.shape();
                        let sz = |k: &str, d: usize| {
                            num_attr(go, k).map_or(st[d], |v| (v as u64).min(st[d]))
                        };
                        size = Some([
                            sz("ImageSizeZ", 0),
                            sz("ImageSizeY", 1),
                            sz("ImageSizeX", 2),
                        ]);
                    }
                    vars.push(Some(i));
                } else {
                    vars.push(None);
                }
            }
        }
        let Some(mut size) = size else {
            break;
        };
        if r == 0
            && let Some(img) = image
        {
            for (d, k) in [(0, "Z"), (1, "Y"), (2, "X")] {
                if let Some(v) = num_attr(Some(img), k)
                    && v >= 1.0
                    && (v as u64) <= size[d]
                {
                    size[d] = v as u64;
                }
            }
        }
        levels.push(ImarisLevel {
            size,
            vars,
            scale: 0,
        });
    }
    let num = num?;
    let [z0, y0, x0] = levels.first()?.size;
    for l in &mut levels {
        l.scale = (0..16u32)
            .map(|j| 1u64 << j)
            .find(|&f| x0.div_ceil(f) == l.size[2] && y0.div_ceil(f) == l.size[1])
            .map_or(0, |f| f as u32);
    }
    let shape = vec![channels as u64, times as u64, z0, y0, x0];
    layout(&shape[3..], num, false)?;
    // Axes: channel names, time stamps, z positions.
    let mut axes = vec![None; 5];
    let chan_names: Vec<String> = (0..channels)
        .map(|c| {
            file.get(&format!("/{info_dir}/Channel {c}"))
                .and_then(|g| attr_text(g, "Name"))
                .unwrap_or_else(|| format!("{}", c + 1))
        })
        .collect();
    axes[0] = Some(AxisMeta {
        name: "Channel".into(),
        labels: chan_names,
        kind: Some(AxisKind::Channel),
        ..Default::default()
    });
    let ti = file.get(&format!("/{info_dir}/TimeInfo"));
    let stamps: Vec<String> = (0..times)
        .map(|t| {
            ti.and_then(|g| attr_text(g, &format!("TimePoint{}", t + 1)))
                .unwrap_or_default()
        })
        .collect();
    axes[1] = Some(AxisMeta {
        name: "Time".into(),
        labels: if stamps.iter().all(|s| !s.is_empty()) {
            stamps
        } else {
            Vec::new()
        },
        ..Default::default()
    });
    let unit = image
        .and_then(|g| attr_text(g, "Unit"))
        .map(|u| match u.as_str() {
            "um" | "micron" | "microns" => "µm".to_owned(),
            _ => u,
        })
        .unwrap_or_else(|| "µm".into());
    let ext = |d: usize, n: u64| -> Option<(f64, f64)> {
        let lo = num_attr(image, &format!("ExtMin{d}"))?;
        let hi = num_attr(image, &format!("ExtMax{d}"))?;
        let step = (hi - lo) / n.max(1) as f64;
        (step > 0.0).then_some((lo, step))
    };
    let mut pixel = None;
    if let (Some((ox, sx)), Some((oy, sy))) = (ext(0, x0), ext(1, y0)) {
        pixel = Some(PixelCalibration {
            x: Linear {
                offset: ox,
                step: sx,
            },
            y: Linear {
                offset: oy,
                step: sy,
            },
            unit_x: unit.clone(),
            unit_y: unit.clone(),
        });
    }
    let z_linear = ext(2, z0).map(|(o, s)| Linear { offset: o, step: s });
    axes[2] = Some(AxisMeta {
        name: "Z".into(),
        unit: if z_linear.is_some() {
            unit.clone()
        } else {
            String::new()
        },
        labels: match z_linear {
            Some(l) if z0 <= MAX_LABELS => (0..z0)
                .map(|k| format!("{} {unit}", num_text(l.at(k as f64))))
                .collect(),
            _ => Vec::new(),
        },
        linear: z_linear,
        kind: None,
    });
    let mut keywords: Vec<(String, String, String)> = Vec::new();
    for g in [
        format!("/{info_dir}/Image"),
        format!("/{info_dir}/Imaris"),
        format!("/{info_dir}/ImarisDataSet"),
    ] {
        if let Some(o) = file.get(&g) {
            for a in &o.attrs {
                if let Some(t) = a.value.text() {
                    keywords.push((a.name.clone(), t, g.clone()));
                }
            }
        }
    }
    let title = image.and_then(|g| attr_text(g, "Name"));
    let first = levels[0].vars.iter().flatten().next().copied()?;
    let levels_n = levels.len();
    Some(vec![Candidate {
        var: first,
        shape,
        num: Some(num),
        x_dim: 4,
        y_dim: 3,
        imaris: Some(Arc::new(Imaris { times, levels })),
        extra: Some(Arc::new(Extra {
            path: Some(format!("/{dir} ({levels_n} resolution levels)")),
            axes,
            pixel,
            kind: Some(ScienceKind::Gridded),
            title,
            keywords,
            ..Default::default()
        })),
        ..Default::default()
    }])
}

/// The text between `open` (up to its `>`) and the next `<`, after `from`.
fn xml_text<'a>(xml: &'a str, open: &str, from: usize) -> Option<(&'a str, &'a str, usize)> {
    let at = from + xml.get(from..)?.find(open)?;
    let gt = at + xml[at..].find('>')?;
    let lt = gt + 1 + xml[gt + 1..].find('<')?;
    Some((&xml[at..gt], xml[gt + 1..lt].trim(), lt))
}

/// A BAG file (`/BAG_root`): its elevation and uncertainty grids.
fn bag_pages(file: &hdf5::File, data: &[u8]) -> Option<Vec<Candidate>> {
    let root = file.get("/BAG_root")?;
    let xml = file
        .index_of("/BAG_root/metadata")
        .and_then(|i| string_dataset(file, data, i))
        .unwrap_or_default();
    // Resolution per dimension, then the corner points (south-west, north-east).
    let (mut res_row, mut res_col, mut uom) = (None, None, String::new());
    let mut pos = 0;
    while let Some(at) = xml[pos..].find("MD_Dimension>").map(|k| pos + k) {
        let end = xml[at + 13..]
            .find("MD_Dimension>")
            .map_or(xml.len(), |k| at + 13 + k);
        let block = &xml[at..end];
        let row = block.contains("\"row\"") || block.contains(">row<");
        let col = block.contains("\"column\"") || block.contains(">column<");
        if let Some((tag, v, _)) = xml_text(block, "Measure", 0)
            && let Ok(v) = v.parse::<f64>()
            && v > 0.0
            && v.is_finite()
        {
            if let Some(u) = tag.split("uom=\"").nth(1).and_then(|r| r.split('"').next()) {
                uom = u.to_owned();
            }
            if row {
                res_row = Some(v);
            } else if col {
                res_col = Some(v);
            }
        }
        pos = end;
    }
    let corners: Vec<(f64, f64)> = xml_text(&xml, "coordinates", 0)
        .map(|(_, v, _)| {
            v.split_whitespace()
                .filter_map(|p| {
                    let (a, b) = p.split_once(',')?;
                    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
                })
                .collect()
        })
        .unwrap_or_default();
    let unit = match uom.to_ascii_lowercase().as_str() {
        "metres" | "meters" | "metre" | "meter" | "m" | "" => "m".to_owned(),
        "degrees" | "degree" | "deg" => "°".to_owned(),
        _ => uom.clone(),
    };
    let epsg = xml_text(&xml, "gmd:code>", 0)
        .and_then(|(_, _, at)| xml_text(&xml, "CharacterString", at))
        .map(|(_, v, _)| v.to_owned());
    let mut keywords = Vec::new();
    for a in &root.attrs {
        keywords.push((a.name.clone(), a.value.display(16), "/BAG_root".to_owned()));
    }
    if let Some(e) = &epsg {
        keywords.push(("Reference system".into(), e.clone(), String::new()));
    }
    let mut pages = Vec::new();
    for (name, score) in [("elevation", 100), ("uncertainty", 50)] {
        let Some(i) = file.index_of(&format!("/BAG_root/{name}")) else {
            continue;
        };
        let Some(ds) = file.objects[i].dataset() else {
            continue;
        };
        let Some((num, _)) = ds.dtype.num() else {
            continue;
        };
        let shape = ds.shape().to_vec();
        if shape.len() != 2 || layout(&shape, num, false).is_none() {
            continue;
        }
        let rows = shape[0];
        let pixel = match (res_col, res_row, corners.first()) {
            (Some(rx), Some(ry), Some(&(x0, y0))) => Some(PixelCalibration {
                x: Linear {
                    offset: x0,
                    step: rx,
                },
                // Row 0 shown is the northernmost.
                y: Linear {
                    offset: y0 + ry * (rows - 1) as f64,
                    step: -ry,
                },
                unit_x: unit.clone(),
                unit_y: unit.clone(),
            }),
            _ => None,
        };
        let o = &file.objects[i];
        pages.push(Candidate {
            var: i,
            shape,
            num: Some(num),
            x_dim: 1,
            y_dim: 0,
            flip_y: true,
            nodata: Some(BAG_NODATA),
            score,
            extra: Some(Arc::new(Extra {
                pixel,
                kind: Some(ScienceKind::Gridded),
                unit: Some(attr_text(o, "units").unwrap_or_else(|| "m".into())),
                title: Some(name.to_owned()),
                keywords: keywords.clone(),
                ..Default::default()
            })),
            ..Default::default()
        });
    }
    (!pages.is_empty()).then_some(pages)
}

/// Whether a NetCDF coordinate variable is a longitude (`east`) or latitude axis by its
/// units, standard name or name.
fn is_lonlat(v: &netcdf::Variable, east: bool) -> bool {
    let units = v.text("units").unwrap_or("").to_ascii_lowercase();
    let std = v.text("standard_name").unwrap_or("").to_ascii_lowercase();
    let name = v.name.to_ascii_lowercase();
    let (unit_end, std_name, names): (&[&str], _, &[&str]) = if east {
        (
            &["_east", "_e", "e"],
            "longitude",
            &["lon", "longitude", "nav_lon"],
        )
    } else {
        (
            &["_north", "_n", "n"],
            "latitude",
            &["lat", "latitude", "nav_lat"],
        )
    };
    units.starts_with("degree") && unit_end.iter().any(|e| units.ends_with(e))
        || std == std_name
        || (names.contains(&name.as_str()) && (units.is_empty() || units.starts_with("degree")))
}

/// The georeference of a NetCDF variable on evenly spaced one-dimensional longitude and
/// latitude coordinates: plain lon/lat on WGS 84, as CF reads coordinates without a grid
/// mapping (a mapping other than `latitude_longitude` gives none).
fn netcdf_lonlat(
    nc: &netcdf::Netcdf,
    data: &[u8],
    v: &netcdf::Variable,
    c: &Candidate,
) -> Option<Arc<GeoRef>> {
    if let Some(gm) = v.text("grid_mapping")
        && nc
            .var(gm.trim())
            .and_then(|m| m.text("grid_mapping_name"))
            .is_some_and(|n| n.trim() != "latitude_longitude")
    {
        return None;
    }
    let (xd, yd) = (v.dims.get(c.x_dim)?, v.dims.get(c.y_dim)?);
    let (xv, yv) = (nc.var(xd)?, nc.var(yd)?);
    if xv.shape.len() != 1 || yv.shape.len() != 1 || !is_lonlat(xv, true) || !is_lonlat(yv, false) {
        return None;
    }
    let x = linear_of(&nc.coordinates(data, xd)?.1)?;
    let ys = nc.coordinates(data, yd)?.1;
    let y = linear_of(&ys)?;
    let (w, h) = (c.shape[c.x_dim] as f64, c.shape[c.y_dim] as f64);
    // Displayed row 0 is the last stored row when the rows are flipped north-up.
    let (top, dy) = if c.flip_y {
        (y.offset + y.step * (h - 1.0), -y.step)
    } else {
        (y.offset, y.step)
    };
    let to_map = Affine2 {
        a: x.step,
        b: 0.0,
        c: x.offset - x.step / 2.0,
        d: 0.0,
        e: dy,
        f: top - dy / 2.0,
    };
    let (x0, y0) = to_map.apply(0.0, 0.0);
    let (x1, y1) = to_map.apply(w, h);
    if y0.abs().max(y1.abs()) > 90.0 + y.step.abs() || (x1 - x0).abs() > 360.0 + x.step.abs() {
        return None;
    }
    let unit = v.text("units").map(str::to_owned);
    Some(Arc::new(GeoRef {
        crs: specular_gis::crs::from_epsg(4326),
        crs_note: Some("lon/lat coordinates, WGS 84 assumed".into()),
        to_map,
        bounds: [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)],
        features: None,
        facts: GeoFacts::Raster(RasterGeo {
            levels: vec![(c.shape[c.x_dim] as u32, c.shape[c.y_dim] as u32)],
            values: true,
            unit,
            band_name: Some(v.name.clone()),
            ..Default::default()
        }),
    }))
}

/// Whether a NetCDF y dimension's coordinate grows northwards (or upwards), so that the
/// first row is the bottom one.
fn north_up_flip(nc: &netcdf::Netcdf, data: &[u8], dim: &str) -> bool {
    let Some(v) = nc.var(dim) else {
        return false;
    };
    let lower = dim.to_ascii_lowercase();
    let units = v.text("units").unwrap_or("").to_ascii_lowercase();
    let std = v.text("standard_name").unwrap_or("").to_ascii_lowercase();
    let axis = v.text("axis").unwrap_or("");
    let northward = units.starts_with("degree") && (units.contains('n') || units == "degrees")
        || matches!(
            std.as_str(),
            "latitude" | "grid_latitude" | "projection_y_coordinate"
        )
        || axis.eq_ignore_ascii_case("y")
        || matches!(lower.as_str(), "lat" | "latitude" | "y" | "nlat" | "rlat");
    if !northward || v.shape.len() != 1 || v.shape[0] < 2 {
        return false;
    }
    let Some((_, c)) = nc.coordinates(data, dim) else {
        return false;
    };
    match (c.first(), c.last()) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}

/// A short number: at most `sig` significant digits, no trailing zeros.
fn num_text(v: f64) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    if v != 0.0 && (v.abs() >= 1e7 || v.abs() < 1e-4) {
        let s = format!("{v:.5e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = if m.contains('.') {
            m.trim_end_matches('0').trim_end_matches('.')
        } else {
            m
        };
        return format!("{m}e{e}");
    }
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_owned() }
}

/// Days from 1970-01-01 to a proleptic Gregorian date; `None` for a date outside
/// [`valid_date`].
fn days_from_civil(y: i64, m: i64, d: i64) -> Option<i64> {
    if !valid_date(y, m, d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// A year within ±9999, month 1–12 and day 1–31: bounded fields keep the calendar arithmetic
/// in range for any digits a file holds.
fn valid_date(y: i64, m: i64, d: i64) -> bool {
    (-9999..=9999).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d)
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// A CF calendar (`calendar` attribute of a time coordinate).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Calendar {
    /// "standard" / "gregorian": Julian before 1582-10-15, Gregorian from then.
    Mixed,
    ProlepticGregorian,
    Julian,
    /// "noleap" / "365_day".
    NoLeap,
    /// "all_leap" / "366_day".
    AllLeap,
    /// "360_day": twelve months of 30 days.
    Day360,
}

/// Day of the year (0-based) of the first of each month, without and with 29 February.
const MONTH_START: [[i64; 12]; 2] = [
    [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334],
    [0, 31, 60, 91, 121, 152, 182, 213, 244, 274, 305, 335],
];

/// Days from 1970-01-01 (Gregorian) to a Julian calendar date.
fn days_from_julian(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    365 * y + y.div_euclid(4) + doy - 719_470
}

fn julian_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_470;
    let era = z.div_euclid(1461);
    let doe = z - era * 1461;
    let yoe = (doe - doe / 1460) / 365;
    let doy = doe - 365 * yoe;
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (era * 4 + yoe + i64::from(m <= 2), m, d)
}

/// The first Gregorian day of the standard calendar, 1582-10-15, from 1970-01-01.
const GREGORIAN_START: i64 = -141_427;

impl Calendar {
    fn parse(name: Option<&str>) -> Option<Calendar> {
        let name = name
            .map(|c| c.trim().to_ascii_lowercase())
            .unwrap_or_default();
        Some(match name.as_str() {
            "" | "standard" | "gregorian" => Calendar::Mixed,
            "proleptic_gregorian" => Calendar::ProlepticGregorian,
            "julian" => Calendar::Julian,
            "noleap" | "365_day" => Calendar::NoLeap,
            "all_leap" | "366_day" => Calendar::AllLeap,
            "360_day" => Calendar::Day360,
            _ => return None,
        })
    }

    /// A day count for a date (any origin; consistent with [`Calendar::date`]); `None` for a
    /// date outside [`valid_date`].
    fn days(self, y: i64, m: i64, d: i64) -> Option<i64> {
        if !valid_date(y, m, d) {
            return None;
        }
        let fixed = |len: i64, leap: usize| len * y + MONTH_START[leap][(m - 1) as usize] + d - 1;
        Some(match self {
            Calendar::ProlepticGregorian => days_from_civil(y, m, d)?,
            Calendar::Julian => days_from_julian(y, m, d),
            Calendar::Mixed => {
                if (y, m, d) >= (1582, 10, 15) {
                    days_from_civil(y, m, d)?
                } else {
                    days_from_julian(y, m, d)
                }
            }
            Calendar::NoLeap => fixed(365, 0),
            Calendar::AllLeap => fixed(366, 1),
            Calendar::Day360 => 360 * y + 30 * (m - 1) + d - 1,
        })
    }

    fn date(self, z: i64) -> (i64, i64, i64) {
        let fixed = |len: i64, leap: usize| {
            let (y, doy) = (z.div_euclid(len), z.rem_euclid(len));
            let m = MONTH_START[leap]
                .iter()
                .rposition(|&s| s <= doy)
                .unwrap_or(0);
            (y, m as i64 + 1, doy - MONTH_START[leap][m] + 1)
        };
        match self {
            Calendar::ProlepticGregorian => civil_from_days(z),
            Calendar::Julian => julian_from_days(z),
            Calendar::Mixed if z >= GREGORIAN_START => civil_from_days(z),
            Calendar::Mixed => julian_from_days(z),
            Calendar::NoLeap => fixed(365, 0),
            Calendar::AllLeap => fixed(366, 1),
            Calendar::Day360 => {
                let (y, doy) = (z.div_euclid(360), z.rem_euclid(360));
                (y, doy / 30 + 1, doy % 30 + 1)
            }
        }
    }
}

/// CF time coordinates ("days since 2000-01-01", "seconds since 1970-01-01T00:00:00Z") as
/// dates in the coordinate's calendar (standard, proleptic Gregorian, Julian, noleap,
/// all_leap, 360_day); `None` for other units or calendars.
fn cf_times(units: &str, calendar: Option<&str>, values: &[f64]) -> Option<Vec<String>> {
    let cal = Calendar::parse(calendar)?;
    let (unit, since) = units.trim().split_once(" since ")?;
    let secs = match unit.trim().to_ascii_lowercase().as_str() {
        "seconds" | "second" | "secs" | "sec" | "s" => 1.0,
        "minutes" | "minute" | "mins" | "min" => 60.0,
        "hours" | "hour" | "hrs" | "hr" | "h" => 3600.0,
        "days" | "day" | "d" => 86400.0,
        _ => return None,
    };
    let since = since
        .trim()
        .trim_end_matches('Z')
        .trim_end_matches("UTC")
        .trim();
    let (date, time) = match since.split_once(['T', ' ']) {
        Some((d, t)) => (d, t.trim()),
        None => (since, ""),
    };
    let mut ymd = date.splitn(3, '-').map(|p| p.trim().parse::<i64>().ok());
    let (y, m, d) = (ymd.next()??, ymd.next()??, ymd.next()??);
    let mut hms = time.split(':').map(|p| p.trim().parse::<f64>().ok());
    let (h, mi, s) = (
        hms.next().flatten().unwrap_or(0.0),
        hms.next().flatten().unwrap_or(0.0),
        hms.next().flatten().unwrap_or(0.0),
    );
    // Hour up to 24, minute under 60, second up to 60 (else not a reference time).
    if !((0.0..=24.0).contains(&h) && (0.0..60.0).contains(&mi) && (0.0..=60.0).contains(&s)) {
        return None;
    }
    let epoch = cal.days(y, m, d)? as f64 * 86400.0 + h * 3600.0 + mi * 60.0 + s;
    let stamps: Vec<f64> = values.iter().map(|v| epoch + v * secs).collect();
    let whole_days = stamps.iter().all(|s| s.rem_euclid(86400.0) == 0.0);
    stamps
        .iter()
        .map(|&s| {
            if !s.is_finite() || s.abs() > 1e14 {
                return None;
            }
            let day = (s / 86400.0).floor();
            let (y, m, d) = cal.date(day as i64);
            // Whole seconds, truncated like a clock (a hair of slack for binary fractions).
            let rest = ((s - day * 86400.0 + 1e-6).floor() as i64).min(86_399);
            Some(if whole_days {
                format!("{y:04}-{m:02}-{d:02}")
            } else {
                format!(
                    "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
                    rest / 3600,
                    rest / 60 % 60,
                    rest % 60
                )
            })
        })
        .collect()
}

/// Labels for coordinate values: CF dates, else numbers with the unit.
pub(crate) fn coord_labels(values: &[f64], units: &str, calendar: Option<&str>) -> Vec<String> {
    if units.contains(" since ")
        && let Some(t) = cf_times(units, calendar, values)
    {
        return t;
    }
    let u = units.trim();
    values
        .iter()
        .map(|&v| {
            if u.is_empty() {
                num_text(v)
            } else {
                format!("{} {u}", num_text(v))
            }
        })
        .collect()
}

struct H5Session {
    source: Arc<Source>,
    model: Arc<Model>,
    page: u32,
    info: ImageInfo,
    summary: ScienceSummary,
    /// The dataset dimension of each summary axis.
    axis_dims: Vec<usize>,
    planes: PlaneCache,
    levels: Levels,
}

impl H5Session {
    fn new(source: Arc<Source>, model: Arc<Model>, page: u32) -> Result<H5Session> {
        let c = model
            .pages
            .get(page as usize)
            .ok_or_else(|| DecodeError::Unsupported("no such page".into()))?
            .clone();
        let data = source.bytes();
        let (w, h) = (c.shape[c.x_dim] as u32, c.shape[c.y_dim] as u32);
        let mut axis_dims: Vec<usize> = (0..c.shape.len())
            .filter(|&d| d != c.x_dim && d != c.y_dim)
            .collect();
        // Imaris: no slider for a single channel, time point or section.
        if c.imaris.is_some() {
            axis_dims.retain(|&d| c.shape[d] > 1);
        }
        // The colour axis first, then the others outermost first.
        if let Some(col) = c.colour {
            axis_dims.retain(|&d| d != col);
            axis_dims.insert(0, col);
        }
        let mut summary = ScienceSummary {
            width: w,
            height: h,
            ..Default::default()
        };
        let mut keywords: Vec<(String, String, String)> = Vec::new();
        let mut sample = c.num.map(Num::describe).unwrap_or_default();
        // Per-axis name, unit and labels.
        let mut axis_meta: Vec<AxisMeta> = vec![AxisMeta::default(); c.shape.len()];
        match &model.backend {
            Backend::Hdf5(main) => {
                let (file, fdata) = model
                    .h5(data, c.file)
                    .ok_or_else(|| DecodeError::Malformed("linked file".into()))?;
                let o = file
                    .object(c.var)
                    .ok_or_else(|| DecodeError::Malformed("not a dataset".into()))?;
                let ds = o
                    .dataset()
                    .ok_or_else(|| DecodeError::Malformed("not a dataset".into()))?;
                let filters = ds.filter_names();
                if !filters.is_empty() {
                    sample += &format!(", {}", filters.join("+"));
                }
                let title = o
                    .attr("long_name")
                    .or_else(|| o.attr("title"))
                    .or_else(|| o.attr("TITLE"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|t| !t.is_empty());
                let shown = c
                    .extra
                    .as_ref()
                    .and_then(|e| e.path.clone())
                    .unwrap_or_else(|| o.path.clone());
                summary.dataset = Some(match (title, &c.emd) {
                    (_, Some(im)) if !im.title.is_empty() && im.title != im.path => {
                        format!("{} — {}", im.path, im.title)
                    }
                    (Some(t), _) => format!("{shown} — {t}"),
                    _ => shown.clone(),
                });
                summary.unit = ["units", "unit", "UNITS"]
                    .iter()
                    .find_map(|k| o.attr(k).and_then(Value::as_str))
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty());
                // Axis names and values from attached dimension scales.
                if let Some(Value::Refs(refs)) = o.attr("DIMENSION_LIST") {
                    for (d, r) in refs.iter().enumerate().take(c.shape.len()) {
                        let Some(scale) = r.first().and_then(|&a| file.at_addr(a)) else {
                            continue;
                        };
                        let name = scale
                            .attr("NAME")
                            .and_then(Value::as_str)
                            .filter(|n| !n.starts_with("This is a netCDF dimension"))
                            .unwrap_or(scale.name())
                            .to_owned();
                        let unit = scale
                            .attr("units")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        let mut labels = Vec::new();
                        if c.shape[d] <= MAX_LABELS
                            && let Some(i) = file.index_of(&scale.path)
                            && let Ok(v) = file.read_all::<f64>(fdata, i, MAX_LABELS)
                            && v.len() as u64 == c.shape[d]
                        {
                            labels = coord_labels(&v, &unit, None);
                        }
                        axis_meta[d] = AxisMeta {
                            name,
                            unit,
                            labels,
                            ..Default::default()
                        };
                    }
                }
                if let Some(im) = &c.emd {
                    for (d, a) in im.axes.iter().enumerate().take(c.shape.len()) {
                        let labels = match a.step {
                            Some(step) if c.shape[d] <= MAX_LABELS => (0..c.shape[d])
                                .map(|k| {
                                    let v = a.start + step * k as f64;
                                    if a.unit.is_empty() {
                                        num_text(v)
                                    } else {
                                        format!("{} {}", num_text(v), a.unit)
                                    }
                                })
                                .collect(),
                            _ => Vec::new(),
                        };
                        axis_meta[d] = AxisMeta {
                            name: a.name.clone(),
                            unit: a.unit.clone(),
                            labels,
                            ..Default::default()
                        };
                    }
                    if let Some((x, y, unit)) = &im.pixel_size
                        && *x > 0.0
                        && *y > 0.0
                        && x.is_finite()
                        && y.is_finite()
                    {
                        let (ox, oy) = im.pixel_offset.unwrap_or((0.0, 0.0));
                        summary.pixel = Some(PixelCalibration {
                            x: Linear {
                                offset: ox,
                                step: *x,
                            },
                            y: Linear {
                                offset: oy,
                                step: *y,
                            },
                            unit_x: unit.clone(),
                            unit_y: unit.clone(),
                        });
                        summary.kind = ScienceKind::ElectronMicroscopy;
                    }
                    summary.instrument = match (&im.instrument, &im.detector) {
                        (Some(i), Some(d)) => Some(format!("{i}, {d}")),
                        (i, d) => i.clone().or_else(|| d.clone()),
                    };
                    summary.date_obs = im.date.clone();
                    keywords.extend(
                        im.metadata
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone(), String::new())),
                    );
                }
                if let Some(e) = &c.extra {
                    for (d, a) in e.axes.iter().enumerate().take(c.shape.len()) {
                        if let Some(a) = a {
                            axis_meta[d] = a.clone();
                        }
                    }
                    if e.pixel.is_some() {
                        summary.pixel = e.pixel.clone();
                    }
                    if let Some(k) = e.kind {
                        summary.kind = k;
                    }
                    if e.unit.is_some() {
                        summary.unit = e.unit.clone();
                    }
                    summary.title = e.title.clone().or(summary.title.take());
                    summary.instrument = e.instrument.clone().or(summary.instrument.take());
                    summary.detector = e.detector.clone().or(summary.detector.take());
                    keywords.extend(e.keywords.iter().cloned());
                }
                keywords.extend(model.notes.iter().cloned());
                // The dataset's attributes, then its groups' up to the root (along the path
                // shown, in this file, for a linked dataset).
                let hidden = |n: &str| netcdf::is_hidden_attr(n);
                if c.imaris.is_none() {
                    keywords.extend(
                        o.attrs
                            .iter()
                            .filter(|a| !hidden(&a.name))
                            .map(|a| (a.name.clone(), a.value.display(16), String::new())),
                    );
                }
                let (walk, mut p) = if c.file.is_some() {
                    (main, shown.split(' ').next().unwrap_or("").to_owned())
                } else {
                    (file, o.path.clone())
                };
                if c.imaris.is_some() {
                    p.clear();
                }
                while let Some(cut) = p.rfind('/') {
                    p.truncate(cut);
                    let gp = if p.is_empty() { "/" } else { p.as_str() };
                    if let Some(g) = walk.get(gp) {
                        keywords.extend(
                            g.attrs
                                .iter()
                                .filter(|a| !hidden(&a.name))
                                .map(|a| (a.name.clone(), a.value.display(16), gp.to_owned())),
                        );
                    }
                    if p.is_empty() {
                        break;
                    }
                }
            }
            Backend::Netcdf(nc) => {
                let v = &nc.vars[c.var];
                sample = v.describe();
                if let Some(file) = nc.hdf5()
                    && let Some((_, _, ds)) = file
                        .datasets()
                        .find(|(_, o, _)| o.path.trim_start_matches('/') == v.name)
                {
                    let filters = ds.filter_names();
                    if !filters.is_empty() {
                        sample += &format!(", {}", filters.join("+"));
                    }
                }
                let title = v.text("long_name").or_else(|| v.text("standard_name"));
                summary.dataset = Some(match title {
                    Some(t) => format!("{} — {t}", v.name),
                    None => v.name.clone(),
                });
                summary.unit = v.text("units").map(str::to_owned);
                summary.object = nc
                    .attr("title")
                    .and_then(Value::as_str)
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty());
                summary.instrument = ["source", "instrument", "platform"].iter().find_map(|k| {
                    nc.attr(k)
                        .and_then(Value::as_str)
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                });
                for (d, dim) in v.dims.iter().enumerate() {
                    let coord = nc.var(dim);
                    let unit = coord.and_then(|c| c.text("units")).unwrap_or("").to_owned();
                    let mut labels = Vec::new();
                    if c.shape[d] <= MAX_LABELS
                        && axis_dims.contains(&d)
                        && let Some((_, vals)) = nc.coordinates(data, dim)
                        && vals.len() as u64 == c.shape[d]
                    {
                        labels = coord_labels(&vals, &unit, coord.and_then(|c| c.text("calendar")));
                    }
                    axis_meta[d] = AxisMeta {
                        name: dim.clone(),
                        unit,
                        labels,
                        ..Default::default()
                    };
                }
                summary.geo = netcdf_lonlat(nc, data, v, &c);
                keywords.extend(
                    v.attrs
                        .iter()
                        .map(|a| (a.name.clone(), a.value.display(16), String::new())),
                );
                keywords.extend(
                    nc.attrs
                        .iter()
                        .map(|a| (format!(":{}", a.name), a.value.display(16), String::new())),
                );
            }
        }
        let mut axes = Vec::with_capacity(axis_dims.len());
        for &d in &axis_dims {
            let AxisMeta {
                name,
                unit,
                labels,
                linear,
                kind,
            } = std::mem::take(&mut axis_meta[d]);
            let colour = Some(d) == c.colour;
            axes.push(NdAxis {
                name,
                len: c.shape[d].min(u64::from(u32::MAX)) as u32,
                unit,
                labels: if colour {
                    ["R", "G", "B", "A"][..c.shape[d].min(4) as usize]
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    labels
                },
                wcs_axis: None,
                linear,
                kind: if colour {
                    AxisKind::Channel
                } else {
                    kind.unwrap_or(AxisKind::Cube)
                },
            });
        }
        summary.channel_axis = c.colour.map(|_| 0);
        summary.axes = axes;
        summary.sample = sample.clone();
        keywords.truncate(MAX_KEYWORDS);
        summary.keywords = Arc::new(keywords);
        let info = ImageInfo {
            format: model.format,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: c.num.map_or(8, |n| (n.size() * 8) as u8),
            progressive: false,
            coding: sample,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(H5Session {
            source,
            model,
            page,
            info,
            summary,
            axis_dims,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn candidate(&self) -> &Candidate {
        &self.model.pages[self.page as usize]
    }

    /// The dataset position of the plane at `index` (a position on each summary axis).
    fn position(&self, index: &[u32]) -> Result<Vec<u64>> {
        let c = self.candidate();
        let mut pos = vec![0u64; c.shape.len()];
        for (k, &d) in self.axis_dims.iter().enumerate() {
            let i = u64::from(index.get(k).copied().unwrap_or(0));
            if i >= c.shape[d] {
                return Err(DecodeError::Malformed("no such plane".into()));
            }
            pos[d] = i;
        }
        Ok(pos)
    }

    /// One plane of an HDF5 candidate at `pos`, from resolution level `level` (Imaris; 0
    /// otherwise), every `step`-th sample along x and y: width, height, values, truncated.
    fn read_h5(
        &self,
        pos: &[u64],
        level: usize,
        step: u64,
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<(usize, usize, Vec<f32>, bool)> {
        let c = self.candidate();
        let main = self.source.bytes();
        let gone = || DecodeError::Malformed("linked file".into());
        if let Some(im) = &c.imaris {
            let lv = im
                .levels
                .get(level)
                .ok_or_else(|| DecodeError::Malformed("no such level".into()))?;
            let k = pos[0] as usize * im.times + pos[1] as usize;
            let var = lv
                .vars
                .get(k)
                .copied()
                .flatten()
                .ok_or_else(|| DecodeError::Malformed("missing Imaris block".into()))?;
            let (f, data) = self.model.h5(main, None).ok_or_else(gone)?;
            let z0 = im.levels[0].size[0].max(1);
            let z = (pos[2] * lv.size[0] / z0).min(lv.size[0].saturating_sub(1));
            let spec = PlaneSpec {
                x_dim: 2,
                y_dim: 1,
                index: vec![z, 0, 0],
            };
            let p = read_plane_h5(f, data, var, &spec, step, stop, &no_files)?;
            let s = step.max(1);
            let (w, h) = (
                lv.size[2].div_ceil(s) as usize,
                lv.size[1].div_ceil(s) as usize,
            );
            if p.width < w || p.height < h {
                return Err(DecodeError::Malformed("Imaris block size".into()));
            }
            let values = if p.width == w {
                let mut v = p.values;
                v.truncate(w * h);
                v
            } else {
                p.values
                    .chunks_exact(p.width)
                    .take(h)
                    .flat_map(|r| &r[..w])
                    .copied()
                    .collect()
            };
            return Ok((w, h, values, p.truncated));
        }
        let (file, var, index) = if c.parts.is_empty() {
            (c.file, c.var, pos.to_vec())
        } else {
            let mut i = pos[0];
            let mut at = None;
            for p in &c.parts {
                if i < p.len {
                    at = Some(p);
                    break;
                }
                i -= p.len;
            }
            let p = at.ok_or_else(|| DecodeError::Malformed("no such plane".into()))?;
            let mut idx = pos.to_vec();
            idx[0] = i;
            (p.file, p.var, idx)
        };
        let (f, data) = self.model.h5(main, file).ok_or_else(gone)?;
        let spec = PlaneSpec {
            x_dim: c.x_dim,
            y_dim: c.y_dim,
            index,
        };
        let p = if file.is_none() {
            let resolve = |name: &str| self.model.virtual_file(name);
            read_plane_h5(f, data, var, &spec, step, stop, &resolve)?
        } else {
            read_plane_h5(f, data, var, &spec, step, stop, &no_files)?
        };
        Ok((p.width, p.height, p.values, p.truncated))
    }

    fn read(&self, index: &[u32], cancel: &CancelToken) -> Result<NdPlane> {
        let stop = || cancel.is_cancelled();
        self.read_level(index, 0, 1, &stop)
    }

    /// One plane at `index` from resolution level `level` (Imaris; 0 otherwise), every
    /// `step`-th sample along x and y.
    fn read_level(
        &self,
        index: &[u32],
        level: usize,
        step: u64,
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<NdPlane> {
        let c = self.candidate();
        let pos = self.position(index)?;
        let data = self.source.bytes();
        let (w, h, mut values, truncated) = match &self.model.backend {
            Backend::Hdf5(_) => self.read_h5(&pos, level, step, stop)?,
            Backend::Netcdf(nc) => {
                let spec = PlaneSpec {
                    x_dim: c.x_dim,
                    y_dim: c.y_dim,
                    index: pos,
                };
                let p = if step > 1 {
                    nc.read_plane_strided(data, c.var, &spec, step, SAMPLE_BUDGET, stop)
                } else {
                    nc.read_plane(data, c.var, &spec, stop)
                }
                .map_err(map_nc)?;
                let v = p.values.iter().map(|&x| x as f32).collect();
                (p.width, p.height, v, p.truncated)
            }
        };
        let s = step.max(1);
        let expect = match &c.imaris {
            Some(im) => im.levels.get(level).map_or((0, 0), |l| {
                (
                    l.size[2].div_ceil(s) as usize,
                    l.size[1].div_ceil(s) as usize,
                )
            }),
            None => (
                u64::from(self.info.width).div_ceil(s) as usize,
                u64::from(self.info.height).div_ceil(s) as usize,
            ),
        };
        if w * h != values.len() || (w, h) != expect {
            return Err(DecodeError::Malformed("plane size".into()));
        }
        if let Some(nd) = c.nodata {
            for v in &mut values {
                if *v == nd {
                    *v = f32::NAN;
                }
            }
        }
        if c.flip_y && w > 0 {
            let (mut top, mut bottom) = (0, h.saturating_sub(1));
            while top < bottom {
                let (a, b) = values.split_at_mut(bottom * w);
                a[top * w..top * w + w].swap_with_slice(&mut b[..w]);
                top += 1;
                bottom -= 1;
            }
        }
        Ok(NdPlane {
            width: w as u32,
            height: h as u32,
            values,
            truncated,
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes
            .get_or(index, cancel, || self.read(index, cancel))
    }

    /// The default rendition from every `step`-th sample of the first plane (or the first
    /// three of a colour axis) at resolution level `level`, within [`SAMPLE_TIME`].
    fn sampled(&self, level: usize, step: u64, cancel: &CancelToken) -> Result<Raster> {
        let start = std::time::Instant::now();
        let late = || start.elapsed() > SAMPLE_TIME;
        let stop = || cancel.is_cancelled() || late();
        let n = self.summary.axes.len();
        let indices: Vec<Vec<u32>> = match self.summary.channel_axis {
            Some(ca) => (0..self.summary.axes[ca].len.min(3))
                .map(|k| {
                    let mut idx = vec![0; n];
                    idx[ca] = k;
                    idx
                })
                .collect(),
            None => vec![vec![0; n]],
        };
        let planes = indices
            .iter()
            .map(|idx| self.read_level(idx, level, step, &stop).map(Arc::new))
            .collect::<Result<Vec<_>>>()
            .map_err(|e| match e {
                DecodeError::Cancelled if !cancel.is_cancelled() => DecodeError::TooLarge,
                e => e,
            })?;
        Ok(science::auto_raster(&planes, None))
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let n = self.summary.axes.len();
        let planes = match self.summary.channel_axis {
            Some(ca) => (0..self.summary.axes[ca].len.min(3))
                .map(|k| {
                    let mut idx = vec![0; n];
                    idx[ca] = k;
                    self.plane(&idx, cancel)
                })
                .collect::<Result<Vec<_>>>()?,
            None => vec![self.plane(&vec![0; n], cancel)?],
        };
        Ok(science::auto_raster(&planes, None))
    }
}

impl Session for H5Session {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode_sampled(&self, long_side: u32, cancel: &CancelToken) -> Option<Result<Raster>> {
        // Sample at twice the thumbnail's size or more, so it is still filtered down.
        let want = u64::from(long_side.max(1)) * 2;
        let (level, long) = match &self.candidate().imaris {
            // Imaris: the coarsest stored level still big enough.
            Some(im) => im
                .levels
                .iter()
                .enumerate()
                .rfind(|(_, l)| l.size[1].max(l.size[2]) >= want)
                .or_else(|| im.levels.iter().enumerate().next())
                .map(|(k, l)| (k, l.size[1].max(l.size[2])))?,
            None => (0, u64::from(self.info.width.max(self.info.height))),
        };
        let step = long / want;
        if step <= 1 && level == 0 {
            return None;
        }
        Some(self.sampled(level, step.max(1), cancel))
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        // Imaris: reduced decodes start from the coarsest stored level they can.
        let stored: Vec<(u32, usize)> = match &self.candidate().imaris {
            Some(im) => im
                .levels
                .iter()
                .enumerate()
                .filter(|(_, l)| l.scale > 0)
                .map(|(k, l)| (l.scale, k))
                .collect(),
            None => Vec::new(),
        };
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |s| {
                stored
                    .iter()
                    .filter(|(f, _)| *f <= s)
                    .map(|(f, _)| *f)
                    .max()
                    .unwrap_or(1)
            },
            |b| match stored.iter().find(|(f, _)| *f == b && b > 1) {
                Some(&(_, k)) => {
                    let idx = vec![0; self.summary.axes.len()];
                    let stop = || cancel.is_cancelled();
                    let p = self.read_level(&idx, k, 1, &stop)?;
                    Ok(science::auto_raster(&[Arc::new(p)], None))
                }
                None => self.auto(cancel),
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        let planes = if self.summary.channel_axis.is_some() {
            3
        } else {
            1
        };
        // f32 planes, plus an f64 plane while a NetCDF plane is unpacked.
        px * 4 * planes
            + if matches!(self.model.backend, Backend::Netcdf(_)) {
                px * 8
            } else {
                0
            }
            + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.planes.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn geo(&self) -> Option<Arc<GeoRef>> {
        self.summary.geo.clone()
    }
    fn page_count(&self) -> u32 {
        self.model.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.model.pages.len() > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: Some(u32::from(self.info.bit_depth)),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(H5Session::new(
            self.source.clone(),
            self.model.clone(),
            page,
        )?))
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if self.summary.channel_axis.is_some() {
            "RGB"
        } else if self.summary.axes.is_empty() {
            "Greyscale"
        } else {
            "Multichannel"
        })
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.plane(index, cancel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test variable: name, dimension ids, text attributes, values, and whether they are
    /// stored as f32.
    type Var<'a> = (&'a str, &'a [u32], &'a [(&'a str, &'a str)], Vec<f64>, bool);

    /// A classic (CDF-1) NetCDF file: dimensions, then variables with text attributes and
    /// big-endian f64 or f32 values.
    fn cdf1(dims: &[(&str, u32)], vars: &[Var]) -> Vec<u8> {
        fn name(b: &mut Vec<u8>, s: &str) {
            b.extend((s.len() as u32).to_be_bytes());
            b.extend(s.as_bytes());
            b.resize(b.len().next_multiple_of(4), 0);
        }
        let header = |begins: &[u32]| {
            let mut b = b"CDF\x01".to_vec();
            b.extend(0u32.to_be_bytes());
            b.extend(10u32.to_be_bytes());
            b.extend((dims.len() as u32).to_be_bytes());
            for (n, l) in dims {
                name(&mut b, n);
                b.extend(l.to_be_bytes());
            }
            b.extend([0; 8]);
            b.extend(11u32.to_be_bytes());
            b.extend((vars.len() as u32).to_be_bytes());
            for (k, (n, ds, atts, vals, double)) in vars.iter().enumerate() {
                name(&mut b, n);
                b.extend((ds.len() as u32).to_be_bytes());
                for d in *ds {
                    b.extend(d.to_be_bytes());
                }
                b.extend(12u32.to_be_bytes());
                b.extend((atts.len() as u32).to_be_bytes());
                for (an, av) in *atts {
                    name(&mut b, an);
                    b.extend(2u32.to_be_bytes());
                    name(&mut b, av);
                }
                b.extend(if *double { 6u32 } else { 5 }.to_be_bytes());
                let size = vals.len() * if *double { 8 } else { 4 };
                b.extend((size.next_multiple_of(4) as u32).to_be_bytes());
                b.extend(begins.get(k).copied().unwrap_or(0).to_be_bytes());
            }
            b
        };
        let mut at = header(&[]).len() as u32;
        let begins: Vec<u32> = vars
            .iter()
            .map(|v| {
                let b = at;
                at += (v.3.len() * if v.4 { 8 } else { 4 }).next_multiple_of(4) as u32;
                b
            })
            .collect();
        let mut b = header(&begins);
        for (_, _, _, vals, double) in vars {
            for v in vals {
                if *double {
                    b.extend(v.to_be_bytes());
                } else {
                    b.extend((*v as f32).to_be_bytes());
                }
            }
        }
        b
    }

    /// CF dates in every calendar against cftime (netCDF4's), when `SPECULAR_PYTHON` names
    /// an interpreter that has it.
    #[test]
    fn cf_dates_against_cftime() {
        let Some(py) = std::env::var_os("SPECULAR_PYTHON") else {
            return;
        };
        const SCRIPT: &str = "import sys, cftime\n\
for a in sys.argv[1:]:\n    \
u, c, v = a.split('|')\n    \
d = cftime.num2date(float(v), u, calendar=c)\n    \
print(f'{d.year:04d}-{d.month:02d}-{d.day:02d} {d.hour:02d}:{d.minute:02d}:{d.second:02d}')\n";
        let mut cases = Vec::new();
        for cal in [
            "standard",
            "proleptic_gregorian",
            "julian",
            "noleap",
            "all_leap",
            "360_day",
        ] {
            for units in [
                "days since 1582-10-01",
                "hours since 1900-03-01 06:00",
                "days since 2000-01-01",
                "seconds since 1970-01-01T00:00:00Z",
                "days since 1-01-01",
            ] {
                for v in [0.0, 1.0, 59.0, 365.25, 10_000.0, 123_456.75, 1e6 + 0.5] {
                    cases.push((units, cal, v));
                }
            }
        }
        let out = std::process::Command::new(py)
            .arg("-I")
            .arg("-c")
            .arg(SCRIPT)
            .args(cases.iter().map(|(u, c, v)| format!("{u}|{c}|{v}")))
            .output()
            .unwrap();
        if !out.status.success() {
            eprintln!("cftime oracle skipped");
            return;
        }
        let want = String::from_utf8(out.stdout).unwrap();
        let mut mismatches = Vec::new();
        for ((u, c, v), w) in cases.iter().zip(want.lines()) {
            let got = cf_times(u, Some(c), &[*v]).unwrap().remove(0);
            let w = if got.len() == 10 {
                w.trim_end_matches(" 00:00:00")
            } else {
                w
            };
            if got != w {
                mismatches.push(format!("{u} {c} {v}: {got} vs {w}"));
            }
        }
        eprintln!(
            "cftime: {} dates, {} mismatches",
            cases.len(),
            mismatches.len()
        );
        assert!(mismatches.is_empty(), "{mismatches:#?}");
    }

    #[test]
    fn netcdf_lonlat_and_calendar() {
        let lat = vec![-45.0, -15.0, 15.0, 45.0];
        let lon = vec![0.0, 90.0, 180.0, 270.0];
        let field: Vec<f64> = (0..32).map(f64::from).collect();
        let data = cdf1(
            &[("time", 2), ("lat", 4), ("lon", 4)],
            &[
                (
                    "time",
                    &[0],
                    &[("units", "days since 2001-02-28"), ("calendar", "360_day")],
                    vec![0.0, 2.0],
                    true,
                ),
                ("lat", &[1], &[("units", "degrees_north")], lat, true),
                ("lon", &[2], &[("units", "degrees_east")], lon, true),
                ("t", &[0, 1, 2], &[("units", "K")], field, false),
            ],
        );
        let s = crate::Registry::new()
            .open(Arc::new(Source::from_vec("t.nc", data)))
            .unwrap();
        let sc = s.science().unwrap();
        assert_eq!((sc.width, sc.height), (4, 4));
        assert_eq!(sc.axes[0].labels, ["2001-02-28", "2001-02-30"]);
        let g = s.geo().unwrap();
        assert_eq!(g.bounds, [-45.0, -60.0, 315.0, 60.0]);
        // Rows are shown north-up: the top row is 45°N.
        assert_eq!(g.to_map.apply(0.5, 0.5), (0.0, 45.0));
        // Row 0 of the plane is the northernmost stored row.
        let p = s.nd_plane(&[1], &CancelToken::new()).unwrap().unwrap();
        assert_eq!(&p.values[..4], &[28.0, 29.0, 30.0, 31.0]);
    }

    #[test]
    fn numbers() {
        assert_eq!(num_text(0.25), "0.25");
        assert_eq!(num_text(-85.0), "-85");
        assert_eq!(num_text(2.5e-10), "2.5e-10");
        assert_eq!(num_text(1.0 / 3.0), "0.333333");
    }

    #[test]
    fn cf_dates() {
        let d = cf_times("days since 2000-01-01", None, &[0.0, 31.0, 59.0, 366.0]).unwrap();
        assert_eq!(d, ["2000-01-01", "2000-02-01", "2000-02-29", "2001-01-01"]);
        let h = cf_times(
            "hours since 1970-01-01 00:00:00",
            Some("gregorian"),
            &[36.5],
        )
        .unwrap();
        assert_eq!(h, ["1970-01-02 12:30:00"]);
        let t = |cal: &str, v: f64| {
            cf_times("days since 2000-01-01", Some(cal), &[v]).unwrap()[0].clone()
        };
        assert_eq!(t("noleap", 59.0), "2000-03-01");
        assert_eq!(t("365_day", 365.0), "2001-01-01");
        assert_eq!(t("all_leap", 366.0 + 59.0), "2001-02-29");
        assert_eq!(t("360_day", 59.0), "2000-02-30");
        assert_eq!(t("360_day", -1.0), "1999-12-30");
        assert_eq!(t("julian", 0.0), "2000-01-01");
        // Julian 2000-01-01 is Gregorian 2000-01-14.
        assert_eq!(
            Some(days_from_julian(2000, 1, 1)),
            days_from_civil(2000, 1, 14)
        );
        assert_eq!(julian_from_days(days_from_julian(-44, 2, 29)), (-44, 2, 29));
        // The standard calendar skips 5-14 October 1582.
        let s = cf_times(
            "days since 1582-10-04",
            Some("standard"),
            &[0.0, 1.0, -365.0],
        )
        .unwrap();
        assert_eq!(s, ["1582-10-04", "1582-10-15", "1581-10-04"]);
        assert_eq!(days_from_civil(1582, 10, 15), Some(GREGORIAN_START));
        assert!(cf_times("days since 2000-01-01", Some("none"), &[1.0]).is_none());
        assert!(cf_times("metres", None, &[1.0]).is_none());
        assert_eq!(
            civil_from_days(days_from_civil(1858, 11, 17).unwrap()),
            (1858, 11, 17)
        );
    }

    #[test]
    fn cf_dates_out_of_range() {
        for (y, m, d) in [
            (i64::MAX, 1, 1),
            (i64::MIN, 3, 1),
            (10_000, 1, 1),
            (2000, 0, 1),
            (2000, 13, 1),
            (2000, 1, 0),
            (2000, 1, 32),
            (2000, 1, i64::MAX),
        ] {
            assert_eq!(days_from_civil(y, m, d), None, "{y} {m} {d}");
        }
        assert_eq!(days_from_civil(-9999, 1, 1), Some(-4_371_587));
        // Out-of-range reference dates and times are not dates: plain numbers instead.
        for units in [
            "days since 9223372036854775807-01-01",
            "days since -9223372036854775808-03-01",
            "days since 2000-13-01",
            "days since 2000-01-99",
            "hours since 2000-01-01 25:00",
            "hours since 2000-01-01 12:60",
            "hours since 2000-01-01 12:00:61",
            "hours since 2000-01-01 -1:00",
        ] {
            for cal in [
                "standard",
                "proleptic_gregorian",
                "julian",
                "noleap",
                "360_day",
            ] {
                assert!(
                    cf_times(units, Some(cal), &[1.0]).is_none(),
                    "{units} {cal}"
                );
            }
            assert_eq!(coord_labels(&[1.0], units, None), [format!("1 {units}")]);
        }
        assert_eq!(
            cf_times("hours since 2000-01-01 24:00:60", None, &[0.0]).unwrap(),
            ["2000-01-02 00:01:00"]
        );
    }

    #[test]
    fn layouts() {
        assert_eq!(layout(&[10, 20], Num::F32, false), Some((1, 0, None)));
        assert_eq!(
            layout(&[480, 640, 3], Num::U8, false),
            Some((1, 0, Some(2)))
        );
        assert_eq!(layout(&[480, 640, 3], Num::F32, false), Some((2, 1, None)));
        assert_eq!(layout(&[3, 480, 640], Num::U8, true), Some((2, 1, Some(0))));
        assert_eq!(layout(&[3, 2], Num::F32, false), None);
        assert_eq!(layout(&[100], Num::F32, false), None);
    }

    fn zlib(raw: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(raw).unwrap();
        e.finish().unwrap()
    }

    /// Row-major chunks of a row-major array of `esize`-byte elements, deflated (edge
    /// chunks padded with zeros).
    fn deflated_chunks(shape: &[u64], chunk: &[u64], esize: usize, raw: &[u8]) -> Vec<Vec<u8>> {
        let rank = shape.len();
        let grid: Vec<u64> = shape
            .iter()
            .zip(chunk)
            .map(|(s, c)| s.div_ceil(*c))
            .collect();
        let n: u64 = grid.iter().product();
        let csize: u64 = chunk.iter().product();
        (0..n)
            .map(|k| {
                let mut g = vec![0u64; rank];
                let mut r = k;
                for d in (0..rank).rev() {
                    g[d] = r % grid[d];
                    r /= grid[d];
                }
                let mut bytes = vec![0u8; csize as usize * esize];
                for e in 0..csize {
                    let mut r = e;
                    let mut at = 0u64;
                    let mut inside = true;
                    let mut l = vec![0u64; rank];
                    for d in (0..rank).rev() {
                        l[d] = r % chunk[d];
                        r /= chunk[d];
                    }
                    for d in 0..rank {
                        let x = g[d] * chunk[d] + l[d];
                        inside &= x < shape[d];
                        at = at * shape[d] + x.min(shape[d] - 1);
                    }
                    if inside {
                        let (s, t) = (at as usize * esize, e as usize * esize);
                        bytes[t..t + esize].copy_from_slice(&raw[s..s + esize]);
                    }
                }
                zlib(&bytes)
            })
            .collect()
    }

    /// A smooth field with some texture, so the zscale limits depend on the samples.
    fn field(w: u64, h: u64, planes: u64) -> Vec<f32> {
        (0..planes * h * w)
            .map(|i| {
                let (x, y, p) = ((i % w) as f32, (i / w % h) as f32, (i / w / h) as f32);
                (x * 0.013).sin() * 100.0 + (y * 0.021).cos() * 80.0 + ((i * 7919) % 97) as f32
                    - p * 10.0
            })
            .collect()
    }

    /// The raster `decode_sampled` should make: the default stretch of every `step`-th
    /// sample of the shown planes, read in full the usual way.
    fn expected(s: &dyn Session, step: u32) -> Raster {
        let sc = s.science().unwrap();
        let n = sc.axes.len();
        let indices: Vec<Vec<u32>> = match sc.channel_axis {
            Some(ca) => (0..sc.axes[ca].len.min(3))
                .map(|k| {
                    let mut idx = vec![0; n];
                    idx[ca] = k;
                    idx
                })
                .collect(),
            None => vec![vec![0; n]],
        };
        let planes: Vec<Arc<NdPlane>> = indices
            .iter()
            .map(|idx| {
                let p = s.nd_plane(idx, &CancelToken::new()).unwrap().unwrap();
                let (w, h) = (p.width.div_ceil(step), p.height.div_ceil(step));
                let values = (0..h)
                    .flat_map(|y| (0..w).map(move |x| (x, y)))
                    .map(|(x, y)| p.values[((y * step) * p.width + x * step) as usize])
                    .collect();
                Arc::new(NdPlane {
                    width: w,
                    height: h,
                    values,
                    truncated: false,
                })
            })
            .collect();
        science::auto_raster(&planes, None)
    }

    /// Pixels of two rasters that differ, plus any difference in size.
    fn raster_mismatches(a: &Raster, b: &Raster) -> usize {
        assert_eq!((a.width, a.height), (b.width, b.height));
        assert_eq!(a.planes.len(), b.planes.len());
        a.planes
            .iter()
            .zip(&b.planes)
            .map(|(p, q)| p.data.iter().zip(&q.data).filter(|(x, y)| x != y).count())
            .sum()
    }

    fn open_vec(name: &str, data: Vec<u8>) -> Arc<dyn Session> {
        crate::Registry::new()
            .open(Arc::new(Source::from_vec(name, data)))
            .unwrap()
    }

    /// Thumbnails of big HDF5 datasets come from every n-th sample of the first plane (or
    /// the first three of a colour axis): the same pixels as the default stretch of those
    /// samples taken from a full read, for contiguous, chunked and compressed storage.
    #[test]
    fn sampled_hdf5_matches_the_full_read() {
        use specular_hdf5::writer::{ROOT, Storage, Ty, Writer};
        let c = CancelToken::new();
        let (w, h) = (1100u64, 700u64);
        let mut bad = 0;
        let mut cases = 0;
        // (file name, shape, element type, chunk, raw bytes)
        let grey: Vec<u8> = field(w, h, 1)
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let stack: Vec<u8> = field(w, h, 3)
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let rgb: Vec<u8> = (0..h * w * 3)
            .map(|i| ((i / 3 % w) * 255 / w + (i % 3) * 40 + (i * 31 % 17)) as u8)
            .collect();
        let shape2 = [h, w];
        let shape3 = [3, h, w];
        let shape_rgb = [h, w, 3];
        type Case<'a> = (&'a str, &'a [u64], Ty, usize, Option<&'a [u64]>, &'a [u8]);
        let datasets: [Case; 5] = [
            ("contiguous.h5", &shape2, Ty::F32, 4, None, &grey),
            ("chunked.h5", &shape2, Ty::F32, 4, Some(&[64, 128]), &grey),
            (
                "stack.h5",
                &shape3,
                Ty::F32,
                4,
                Some(&[1, 100, 100]),
                &stack,
            ),
            ("rgb.h5", &shape_rgb, Ty::U8, 1, Some(&[128, 128, 3]), &rgb),
            ("strip.h5", &shape2, Ty::F32, 4, Some(&[1, 1100]), &grey),
        ];
        for (name, shape, ty, esize, chunk, raw) in datasets {
            let mut wr = Writer::new();
            let storage = match chunk {
                None => Storage::Contiguous,
                Some(ch) => Storage::Chunks {
                    chunk: ch.to_vec(),
                    filters: vec![(1, vec![1])],
                    stored: deflated_chunks(shape, ch, esize, raw),
                },
            };
            wr.dataset(ROOT, "data", ty, shape, raw.to_vec(), storage);
            let s = open_vec(name, wr.finish());
            assert_eq!((s.info().width, s.info().height), (1100, 700), "{name}");
            // Big enough already: no sampling.
            assert!(s.decode_sampled(600, &c).is_none(), "{name}");
            for (long, step) in [(256, 2), (128, 4), (64, 8), (40, 13)] {
                let got = s.decode_sampled(long, &c).unwrap().unwrap();
                let want = expected(&*s, step);
                bad += raster_mismatches(&got, &want);
                cases += 1;
            }
        }
        assert_eq!(cases, 20);
        assert_eq!(bad, 0, "{bad} mismatched pixels");
    }

    /// Classic NetCDF grids sample with byte strides, rows flipped north-up as in the viewer.
    #[test]
    fn sampled_netcdf_matches_the_full_read() {
        let c = CancelToken::new();
        // 601 rows: with a step of 2 the flipped samples land on the same rows.
        let (w, h) = (1100u32, 601u32);
        let lat: Vec<f64> = (0..h).map(|i| -60.0 + f64::from(i) * 0.2).collect();
        let lon: Vec<f64> = (0..w).map(|i| f64::from(i) * 0.3).collect();
        let t: Vec<f64> = field(u64::from(w), u64::from(h), 2)
            .iter()
            .map(|&v| f64::from(v))
            .collect();
        let data = cdf1(
            &[("time", 2), ("lat", h), ("lon", w)],
            &[
                ("lat", &[1], &[("units", "degrees_north")], lat, true),
                ("lon", &[2], &[("units", "degrees_east")], lon, true),
                ("t", &[0, 1, 2], &[("units", "K")], t, false),
            ],
        );
        let s = open_vec("t.nc", data);
        assert_eq!((s.info().width, s.info().height), (w, h));
        let got = s.decode_sampled(256, &c).unwrap().unwrap();
        assert_eq!(raster_mismatches(&got, &expected(&*s, 2)), 0);
        assert!(s.decode_sampled(1024, &c).is_none());
    }

    /// A cancelled or hostile read gives up instead of hanging.
    #[test]
    fn sampled_reads_stop() {
        use specular_hdf5::writer::{ROOT, Storage, Ty, Writer};
        let (w, h) = (1100u64, 700u64);
        let raw: Vec<u8> = field(w, h, 1)
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let mut wr = Writer::new();
        wr.dataset(
            ROOT,
            "data",
            Ty::F32,
            &[h, w],
            raw.clone(),
            Storage::Chunks {
                chunk: vec![64, 64],
                filters: vec![(1, vec![1])],
                stored: deflated_chunks(&[h, w], &[64, 64], 4, &raw),
            },
        );
        let s = open_vec("c.h5", wr.finish());
        let c = CancelToken::new();
        c.cancel();
        assert!(matches!(
            s.decode_sampled(128, &c),
            Some(Err(DecodeError::Cancelled))
        ));
        // A single compressed chunk of 256 MB (in a file of a few hundred bytes) is refused
        // up front. (The writer wants the raw bytes: zeroed pages that are never touched.)
        let mut wr = Writer::new();
        wr.dataset(
            ROOT,
            "data",
            Ty::F32,
            &[8192, 8192],
            vec![0; 256 << 20],
            Storage::Single {
                filters: vec![(1, vec![1])],
                stored: zlib(&[0; 64]),
            },
        );
        let s = open_vec("huge.h5", wr.finish());
        let t = std::time::Instant::now();
        let r = s.decode_sampled(256, &CancelToken::new());
        assert!(matches!(r, Some(Err(_))), "{r:?}");
        assert!(t.elapsed() < std::time::Duration::from_secs(1));
    }
}
