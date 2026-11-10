//! Point clouds: own readers for LAS 1.0–1.4, LAZ (LASzip), COPC, PCD, XYZ/PTS/TXT columns,
//! point-only PLY and E57, into one [`PointCloud`]: `f32` positions relative to an `f64`
//! origin in a z-up frame, with packed colour, classification, intensity and returns, and
//! an octree of leaves whose points are shuffled, so any prefix of a leaf is an even
//! subsample of it ([`Leaf`]): the GPU view draws as much of each leaf as its size on
//! screen needs, under a per-frame point budget.
//!
//! [`open`] reads the header and returns a [`Loading`] at once with what the header says
//! (counts, box, CRS); [`Job::run`] then decodes the points in parallel chunks, publishing
//! each finished chunk as a [`Batch`] for a view that fills in while loading, and ends with
//! the whole cloud (or an error). Formats without a box in their header (PCD, XYZ, PLY,
//! E57) are read whole by [`open`].
//!
//! Everything is bounded: point counts by [`Limits::max_points`] (a larger cloud is evenly
//! subsampled and says so), header-declared sizes against the file's length, and malformed
//! input is an error, never a panic.

mod build;
mod classes;
mod copc;
mod copc_fetch;
mod crs;
mod e57;
pub mod fixtures;
mod las;
mod laz;
mod lod;
mod pcd;
mod ply;
pub mod preview;
mod ptx;
mod text;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::{Condvar, Mutex};

pub use classes::{CLASS_COLOURS, class_colour, is_standard_class};
pub use copc_fetch::{Fetched, RangeRead, fetch_levels};
pub use lod::{LEAF_POINTS, View, select};
pub use specular_mesh::Bounds;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("malformed point cloud: {0}")]
    Malformed(String),
    #[error("unsupported point cloud: {0}")]
    Unsupported(String),
    #[error("point cloud too large: {0}")]
    TooLarge(String),
    #[error("no points")]
    Empty,
    #[error("cancelled")]
    Cancelled,
}

pub(crate) fn malformed<T>(m: impl Into<String>) -> Result<T> {
    Err(Error::Malformed(m.into()))
}

/// A point cloud file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointFormat {
    /// ASPRS LAS 1.0–1.4.
    Las,
    /// LASzip-compressed LAS (`.laz`); a COPC file is one with its hierarchy.
    Laz,
    /// Point Cloud Library's PCD.
    Pcd,
    /// Whitespace- or comma-separated columns (`.xyz`, `.txt`).
    Xyz,
    /// Leica PTS (a point count line, then x y z intensity r g b columns).
    Pts,
    /// PLY with vertices and no faces.
    Ply,
    /// ASTM E57 (one or more scans).
    E57,
    /// Leica PTX: one or more scans, each a grid of x y z intensity [r g b] rows with its
    /// pose.
    Ptx,
}

impl PointFormat {
    pub const ALL: [PointFormat; 8] = [
        PointFormat::Las,
        PointFormat::Laz,
        PointFormat::Pcd,
        PointFormat::Xyz,
        PointFormat::Pts,
        PointFormat::Ply,
        PointFormat::E57,
        PointFormat::Ptx,
    ];

    /// Extensions (lower case) chosen by name; PLY is the mesh reader's, which hands a
    /// face-less file over.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            PointFormat::Las => &["las"],
            PointFormat::Laz => &["laz"],
            PointFormat::Pcd => &["pcd"],
            PointFormat::Xyz => &["xyz"],
            PointFormat::Pts => &["pts"],
            PointFormat::Ply => &[],
            PointFormat::E57 => &["e57"],
            PointFormat::Ptx => &["ptx"],
        }
    }

    pub fn from_extension(ext: &str) -> Option<PointFormat> {
        let ext = ext.to_ascii_lowercase();
        PointFormat::ALL
            .into_iter()
            .find(|f| f.extensions().contains(&ext.as_str()))
    }

    /// The format of a file from its first bytes, where it has a signature (LAS and LAZ
    /// share "LASF"; the point format byte says which).
    pub fn sniff(head: &[u8]) -> Option<PointFormat> {
        if head.starts_with(b"LASF") {
            let compressed = head.get(104).is_some_and(|b| b & 0xC0 != 0);
            return Some(if compressed {
                PointFormat::Laz
            } else {
                PointFormat::Las
            });
        }
        if head.starts_with(b"ASTM-E57") {
            return Some(PointFormat::E57);
        }
        if pcd::sniff(head) {
            return Some(PointFormat::Pcd);
        }
        None
    }

    /// A short name for the info panel.
    pub fn name(self) -> &'static str {
        match self {
            PointFormat::Las => "LAS",
            PointFormat::Laz => "LAZ",
            PointFormat::Pcd => "PCD",
            PointFormat::Xyz => "XYZ",
            PointFormat::Pts => "PTS",
            PointFormat::Ply => "PLY",
            PointFormat::E57 => "E57",
            PointFormat::Ptx => "PTX",
        }
    }
}

/// Caps on what is read.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Points kept at most; a larger cloud is evenly subsampled to about this many.
    pub max_points: u64,
    /// Points a file may declare at most (beyond it the file is refused).
    pub max_declared: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_points: 32_000_000,
            max_declared: 1 << 40,
        }
    }
}

/// What the points carry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Channels {
    pub rgb: bool,
    pub intensity: bool,
    pub classification: bool,
    pub returns: bool,
}

/// One scan of an E57 file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    pub name: Option<String>,
    pub points: u64,
    /// Spherical coordinates (converted on reading).
    pub spherical: bool,
    /// Its pose put it in place (a rotation or translation that is not the identity).
    pub posed: bool,
}

/// What the file says about its cloud.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CloudInfo {
    pub format: Option<PointFormat>,
    /// Format version ("1.4"; "0.7" for PCD; "1.0" for E57).
    pub version: Option<String>,
    /// LAS point data record format (0–10).
    pub point_format: Option<u8>,
    /// LASzip's compressor ("LASzip 3.4 r3, layered"), or PCD's data encoding.
    pub coding: Option<String>,
    /// The coordinate reference system's name (from WKT or GeoTIFF keys).
    pub crs: Option<String>,
    /// A COPC file (an octree of LAZ chunks).
    pub copc: bool,
    /// Points in the file.
    pub total: u64,
    /// Points read (fewer than `total` when subsampled).
    pub loaded: u64,
    pub channels: Channels,
    /// Classes present with their point counts (of the points read), ascending.
    pub classes: Vec<(u8, u64)>,
    /// Intensity's 1st and 99th percentile (of the points read).
    pub intensity_range: [u16; 2],
    /// E57 scans.
    pub scans: Vec<Scan>,
    /// The box in the file's coordinates (min xyz, max xyz).
    pub extent: Option<[f64; 6]>,
}

/// Bits of [`PointCloud::attrs`]' second word above the intensity.
pub mod attr {
    /// Shift of the return number (4 bits) and the number of returns (4 bits above it).
    pub const RETURNS_SHIFT: u32 = 16;
}

/// A point cloud: positions relative to `origin` in a z-up frame, and per point two words:
/// `r | g << 8 | b << 16 | class << 24` (sRGB) and `intensity | return << 16 | returns << 20`.
#[derive(Clone, Debug, Default)]
pub struct PointCloud {
    pub origin: [f64; 3],
    pub positions: Vec<[f32; 3]>,
    pub attrs: Vec<[u32; 2]>,
    /// The box of `positions`.
    pub bounds: Bounds,
    /// Leaves in order; their ranges cover `positions` without overlap.
    pub leaves: Vec<Leaf>,
    pub info: CloudInfo,
}

/// A cell of the octree: `count` points from `first`, in random order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leaf {
    pub first: u32,
    pub count: u32,
    pub bounds: Bounds,
}

impl PointCloud {
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Bytes held.
    pub fn bytes(&self) -> usize {
        self.positions.len() * 12 + self.attrs.len() * 8 + self.leaves.len() * 32
    }
}

/// Points decoded so far, published while loading.
#[derive(Debug, Default)]
pub struct Batch {
    pub positions: Vec<[f32; 3]>,
    pub attrs: Vec<[u32; 2]>,
}

#[derive(Default)]
struct State {
    batches: Vec<Arc<Batch>>,
    loaded: u64,
    intensity_range: Option<[u16; 2]>,
    done: Option<Result<Arc<PointCloud>>>,
}

/// A cloud being read: what its header says at once, its points as they arrive and the
/// finished cloud.
pub struct Loading {
    /// What the header says (counts and channels; `loaded` 0).
    pub info: CloudInfo,
    /// The origin positions are relative to.
    pub origin: [f64; 3],
    /// The box from the header, relative to `origin` (empty when it has none).
    pub bounds: Bounds,
    /// Points expected (after subsampling).
    pub expected: u64,
    state: Mutex<State>,
    cv: Condvar,
    cancel: AtomicBool,
}

impl std::fmt::Debug for Loading {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loading")
            .field("info", &self.info)
            .field("expected", &self.expected)
            .finish_non_exhaustive()
    }
}

impl Loading {
    pub(crate) fn new(info: CloudInfo, origin: [f64; 3], bounds: Bounds, expected: u64) -> Self {
        Loading {
            info,
            origin,
            bounds,
            expected,
            state: Mutex::new(State::default()),
            cv: Condvar::new(),
            cancel: AtomicBool::new(false),
        }
    }

    /// A finished loading of `cloud`.
    pub(crate) fn finished(cloud: PointCloud) -> Self {
        let l = Loading::new(
            cloud.info.clone(),
            cloud.origin,
            cloud.bounds,
            cloud.len() as u64,
        );
        l.finish(Ok(cloud));
        l
    }

    pub(crate) fn push(&self, b: Arc<Batch>) {
        let mut s = self.state.lock();
        s.loaded += b.positions.len() as u64;
        s.batches.push(b);
        drop(s);
        self.cv.notify_all();
    }

    pub(crate) fn finish(&self, r: Result<PointCloud>) {
        let mut s = self.state.lock();
        s.batches.clear();
        s.done = Some(r.map(Arc::new));
        drop(s);
        self.cv.notify_all();
    }

    pub(crate) fn set_intensity_range(&self, r: [u16; 2]) {
        self.state.lock().intensity_range = Some(r);
    }

    /// Intensity's 1st and 99th percentile: of the finished cloud, or estimated from the
    /// first points while loading.
    pub fn intensity_range(&self) -> [u16; 2] {
        let s = self.state.lock();
        match &s.done {
            Some(Ok(c)) => c.info.intensity_range,
            _ => s.intensity_range.unwrap_or(self.info.intensity_range),
        }
    }

    /// Batches from the `from`-th on (empty once finished).
    pub fn batches(&self, from: usize) -> Vec<Arc<Batch>> {
        let s = self.state.lock();
        s.batches.get(from..).map(<[_]>::to_vec).unwrap_or_default()
    }

    /// Points published so far.
    pub fn loaded(&self) -> u64 {
        let s = self.state.lock();
        match &s.done {
            Some(Ok(c)) => c.len() as u64,
            _ => s.loaded,
        }
    }

    /// The finished cloud, or the error that ended loading; `None` while loading.
    pub fn result(&self) -> Option<Result<Arc<PointCloud>>> {
        self.state.lock().done.clone()
    }

    /// Waits for the end of loading.
    pub fn wait(&self) -> Result<Arc<PointCloud>> {
        let mut s = self.state.lock();
        loop {
            if let Some(r) = &s.done {
                return r.clone();
            }
            self.cv.wait(&mut s);
        }
    }

    /// Stops loading (the cloud ends as [`Error::Cancelled`]).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// The work left after [`open`]: decoding the points.
pub struct Job {
    plan: Box<las::Plan>,
}

impl Job {
    /// Decodes the points of `bytes` (the same bytes [`open`] saw) into `loading`, in
    /// parallel, publishing batches as chunks finish.
    ///
    /// A panic while decoding (a bug) still ends the loading, as malformed, before it
    /// unwinds on: whoever waits on `loading` gets an error instead of waiting forever.
    pub fn run(self, bytes: &[u8], loading: &Loading) {
        struct EndOnPanic<'a>(&'a Loading);
        impl Drop for EndOnPanic<'_> {
            fn drop(&mut self) {
                if std::thread::panicking() {
                    self.0
                        .finish(malformed("internal error while reading the points"));
                }
            }
        }
        let guard = EndOnPanic(loading);
        #[cfg(test)]
        tests::trip(bytes);
        let r = (*self.plan).run(bytes, loading);
        drop(guard);
        loading.finish(r);
    }
}

/// Starts reading `bytes` as `format`: the header now, the points by the returned job (none
/// when they were read here).
pub fn open(
    format: PointFormat,
    bytes: &[u8],
    limits: &Limits,
) -> Result<(Arc<Loading>, Option<Job>)> {
    let whole = |c: Result<PointCloud>| -> Result<(Arc<Loading>, Option<Job>)> {
        let c = c?;
        if c.is_empty() {
            return Err(Error::Empty);
        }
        Ok((Arc::new(Loading::finished(c)), None))
    };
    match format {
        PointFormat::Las | PointFormat::Laz => {
            let plan = las::Plan::new(bytes, limits)?;
            let loading = Arc::new(plan.loading());
            Ok((
                loading,
                Some(Job {
                    plan: Box::new(plan),
                }),
            ))
        }
        PointFormat::Pcd => whole(pcd::read(bytes, limits)),
        PointFormat::Xyz | PointFormat::Pts => whole(text::read(format, bytes, limits)),
        PointFormat::Ply => whole(ply::read(bytes, limits)),
        PointFormat::E57 => whole(e57::read(bytes, limits)),
        PointFormat::Ptx => whole(ptx::read(bytes, limits)),
    }
}

/// Reads the whole cloud on this thread.
pub fn read(format: PointFormat, bytes: &[u8], limits: &Limits) -> Result<Arc<PointCloud>> {
    let (loading, job) = open(format, bytes, limits)?;
    if let Some(job) = job {
        job.run(bytes, &loading);
    }
    loading.wait()
}

/// A finished cloud of points another reader decoded (3D Tiles point contents):
/// `positions` relative to `origin` in a z-up frame, with sRGB colours when given.
pub fn from_points(
    positions: Vec<[f32; 3]>,
    rgb: Option<Vec<[u8; 3]>>,
    origin: [f64; 3],
    coding: &str,
) -> Result<Arc<Loading>> {
    let attrs: Vec<[u32; 2]> = match &rgb {
        Some(c) if c.len() == positions.len() => c
            .iter()
            .map(|c| {
                [
                    u32::from(c[0]) | (u32::from(c[1]) << 8) | (u32::from(c[2]) << 16),
                    0,
                ]
            })
            .collect(),
        _ => vec![[0, 0]; positions.len()],
    };
    let info = CloudInfo {
        coding: Some(coding.to_owned()),
        total: positions.len() as u64,
        channels: Channels {
            rgb: rgb.is_some(),
            ..Channels::default()
        },
        ..CloudInfo::default()
    };
    let cloud = build::finish(positions, attrs, info, origin)?;
    Ok(Arc::new(Loading::finished(cloud)))
}

/// Whether a PLY file is a point cloud (vertices, no faces), from its header.
pub fn ply_is_points(bytes: &[u8]) -> bool {
    ply::is_points(bytes)
}
