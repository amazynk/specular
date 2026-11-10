//! Whole-slide images and microscopy pyramids.
//!
//! A [`Slide`] is one image stored at several resolutions (levels), with the slide's facts
//! (vendor, objective, microns per pixel, scanner, date) and its associated images (label,
//! macro, thumbnail). It reads any power-of-two downscale of any region as RGBA8, from the
//! coarsest level that is at least as sharp, and nothing else.
//!
//! - Aperio SVS: tiled TIFF levels (JPEG or JPEG 2000 tiles), the `|`-separated
//!   ImageDescription, the thumbnail, label and macro IFDs.
//! - Hamamatsu NDPI: one JPEG per level with restart markers; a region is decoded by
//!   splicing the restart intervals that cover it into a small JPEG of its own
//!   ([`ndpi`]). Files over 4 GiB keep the high halves of offsets after each IFD.
//! - Hamamatsu VMS and VMU ([`vms`]): an INI index over JPEG image files with restart
//!   markers (read like NDPI levels, the optimisation file's row offsets speeding the
//!   search for the intervals) or uncompressed NGR files; the map as the 1/8 level.
//! - Leica SCN: the XML description's collection, images and their resolution IFDs.
//! - Generic pyramidal TIFF: tiled levels in the main IFD chain or SubIFDs.
//! - OME-TIFF: OME-XML pixels, channels, physical sizes, TiffData plane maps (files in the
//!   same folder only) and SubIFD pyramids.
//! - DICOM VL Whole Slide Microscopy: a series of instances (TILED_FULL or TILED_SPARSE,
//!   concatenations, focal planes, optical paths) as one pyramid, with its LABEL, OVERVIEW
//!   and THUMBNAIL images ([`dicomwsi`]).
//! - OME-Zarr v0.1-0.5 (Zarr v2 and v3), from a folder or a `.zip`: multiscales, axes,
//!   scale transformations, omero channels; raw, gzip, zlib, Zstandard and Blosc chunks,
//!   sharding.
//! - Stitched scans ([`mosaic`]): Roche/Ventana BIF (level-0 tiles placed by their recorded
//!   joints, [`bif`]), 3DHISTECH MIRAX (`.mrxs` with `Slidedat.ini`, `Index.dat` and data
//!   files, [`mrxs`]) and Olympus/Evident VSI (the `.vsi` and its `.ets` frame files,
//!   [`vsi`]).
//! - External readers ([`external`]): Zeiss CZI, whose regions libCZI reads in Specular's
//!   sandboxed decode host; this crate gives them levels, compositing and the slide model.
//!
//! Multi-channel images (fluorescence) are composited in their channels' colours with each
//! channel's display window (from the file, else from its coarsest level's histogram).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub mod annot;
pub mod bif;
pub mod dicomwsi;
pub mod external;
pub mod mrxs;
pub mod ndpi;
pub mod ome;
pub mod vms;
pub mod vsi;
pub mod xml;
pub mod zarr;
pub mod zip;

mod aperio;
mod blob;
mod composite;
mod mosaic;
mod scn;
mod stitched;
mod tiffslide;

/// Synthetic slides for tests here and in dependent crates (feature `fixtures`).
#[cfg(any(test, feature = "fixtures"))]
#[doc(hidden)]
#[path = "tests.rs"]
pub mod fixtures;

pub use composite::{Plane, box_reduce_rgba};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The file is a TIFF but none of the slide conventions.
    #[error("not a slide")]
    NotSlide,
    #[error("malformed slide: {0}")]
    Malformed(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("truncated: {0}")]
    Truncated(String),
    #[error("too large")]
    TooLarge,
    #[error("cancelled")]
    Cancelled,
    #[error("i/o: {0}")]
    Io(String),
    /// Under the caps, but more than the memory budget has free now (try again later).
    #[error("{0}")]
    NeedsMemory(specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<specular_tiff::Error> for Error {
    fn from(e: specular_tiff::Error) -> Self {
        use specular_tiff::Error as E;
        match e {
            E::Truncated(s) => Error::Truncated(s.into()),
            E::Malformed(s) => Error::Malformed(s.into()),
            E::Unsupported(s) => Error::Unsupported(s),
            E::TooLarge => Error::TooLarge,
            E::Cancelled => Error::Cancelled,
            E::NoPage | E::BadRegion => Error::Malformed(e.to_string()),
        }
    }
}

impl From<specular_jp2::Jp2Error> for Error {
    fn from(e: specular_jp2::Jp2Error) -> Self {
        use specular_jp2::Jp2Error as E;
        match e {
            E::Cancelled => Error::Cancelled,
            E::TooLarge(_) => Error::TooLarge,
            E::NeedsMemory(n) => Error::NeedsMemory(n),
            e => Error::Malformed(format!("JPEG 2000: {e}")),
        }
    }
}

impl From<specular_jpeg::JpegError> for Error {
    fn from(e: specular_jpeg::JpegError) -> Self {
        match e {
            specular_jpeg::JpegError::Cancelled => Error::Cancelled,
            e => Error::Malformed(format!("JPEG: {e}")),
        }
    }
}

/// Largest output (pixels) one read may produce.
pub const MAX_READ_PIXELS: u64 = 1 << 26;

pub(crate) fn check_cancel(c: Option<&AtomicBool>) -> Result<()> {
    if c.is_some_and(|c| c.load(Ordering::Relaxed)) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

/// Who wrote the slide, as far as its structure tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Vendor {
    Aperio,
    Hamamatsu,
    Leica,
    /// OME-TIFF (Bio-Formats, QuPath, bfconvert…).
    OmeTiff,
    OmeZarr,
    /// A tiled TIFF pyramid of no particular convention.
    Generic,
    /// DICOM VL Whole Slide Microscopy.
    Dicom,
    /// Roche/Ventana BIF (iScan).
    Ventana,
    /// 3DHISTECH MIRAX.
    Mirax,
    /// Olympus/Evident cellSens VSI.
    Olympus,
    /// Zeiss CZI (ZEN, Axioscan), through libCZI.
    Zeiss,
}

impl Vendor {
    pub fn name(self) -> &'static str {
        match self {
            Vendor::Aperio => "Aperio",
            Vendor::Hamamatsu => "Hamamatsu",
            Vendor::Leica => "Leica",
            Vendor::OmeTiff => "OME-TIFF",
            Vendor::OmeZarr => "OME-Zarr",
            Vendor::Generic => "Pyramidal TIFF",
            Vendor::Dicom => "DICOM WSI",
            Vendor::Ventana => "Roche Ventana",
            Vendor::Mirax => "3DHISTECH MIRAX",
            Vendor::Olympus => "Olympus VSI",
            Vendor::Zeiss => "Zeiss CZI",
        }
    }
}

/// What an associated image is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssociatedKind {
    Label,
    Macro,
    Thumbnail,
    /// Another image of the file (an OME-TIFF series, a Leica SCN image) by its name.
    Other(String),
}

/// One stored resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub width: u32,
    pub height: u32,
    /// Full-resolution pixels per level pixel.
    pub downsample: f64,
}

/// One channel of a multi-channel image.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Channel {
    pub name: String,
    /// Display colour, as the file gives it.
    pub colour: Option<[u8; 3]>,
    /// Display window (values mapped to black and full colour), as the file gives it.
    pub window: Option<(f64, f64)>,
}

/// The facts of a slide for the info panel.
#[derive(Clone, Debug, PartialEq)]
pub struct Props {
    pub vendor: Vendor,
    /// Microns per pixel (x, y) at full resolution.
    pub mpp: Option<(f64, f64)>,
    /// Objective (apparent) magnification.
    pub objective: Option<f64>,
    /// Scanner model or identifier.
    pub scanner: Option<String>,
    /// Acquisition date as stored.
    pub date: Option<String>,
    /// The image's name.
    pub name: Option<String>,
    /// Pixel type, e.g. "8-bit RGB, JPEG", "16-bit unsigned".
    pub sample: String,
    /// Z step, microns (OME).
    pub z_step: Option<f64>,
    /// Time step, seconds (OME).
    pub t_step: Option<f64>,
    /// Vendor properties (key, value), in file order.
    pub properties: Vec<(String, String)>,
    /// The image centre's offset from the centre of the slide glass, nanometres (NDPI), which
    /// places NDP.view annotations.
    pub centre_offset_nm: Option<(f64, f64)>,
}

impl Props {
    pub fn new(vendor: Vendor) -> Props {
        Props {
            vendor,
            mpp: None,
            objective: None,
            scanner: None,
            date: None,
            name: None,
            sample: String::new(),
            z_step: None,
            t_step: None,
            properties: Vec::new(),
            centre_offset_nm: None,
        }
    }
}

/// An RGBA8 raster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    /// Some tile was missing or damaged.
    pub truncated: bool,
}

/// A rectangle in the pixels of a (scaled) image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Region {
    /// `r` clipped to a `w`×`h` image; `None` when nothing is left.
    pub fn clip(r: Option<Region>, w: u32, h: u32) -> Option<Region> {
        let r = r.unwrap_or(Region { x: 0, y: 0, w, h });
        let x1 = r.x.saturating_add(r.w).min(w);
        let y1 = r.y.saturating_add(r.h).min(h);
        (r.x < x1 && r.y < y1).then(|| Region {
            x: r.x,
            y: r.y,
            w: x1 - r.x,
            h: y1 - r.y,
        })
    }
}

/// A plane position past x and y (OME's Z and T; channels are composited).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Position {
    pub z: u32,
    pub t: u32,
}

/// The bytes of the files a slide reads: the opened file first, then [`Slide::files`] in
/// order (empty slices for files that could not be read).
#[derive(Clone, Copy, Debug)]
pub struct Files<'a> {
    pub data: &'a [&'a [u8]],
}

impl<'a> Files<'a> {
    pub fn get(&self, i: usize) -> Result<&'a [u8]> {
        self.data
            .get(i)
            .copied()
            .filter(|d| !d.is_empty())
            .ok_or_else(|| Error::Io(format!("file {i} of the set is missing")))
    }
}

/// How to read.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReadOptions<'a> {
    /// Power-of-two downscale of the full-resolution image.
    pub scale: u32,
    /// In the pixels of the downscaled image; `None` for all of it.
    pub region: Option<Region>,
    pub position: Position,
    pub cancel: Option<&'a AtomicBool>,
    /// Decode tiles on the rayon pool.
    pub parallel: bool,
}

pub(crate) enum Backend {
    Tiff(tiffslide::TiffSlide),
    Ndpi(ndpi::Ndpi),
    Zarr(zarr::ZarrSlide),
    Dicom(dicomwsi::DicomSlide),
    Stitched(Box<stitched::Stitched>),
    External(Box<external::ExternalSlide>),
    Vms(Box<vms::Vms>),
}

/// A whole-slide image or microscopy pyramid.
pub struct Slide {
    pub props: Props,
    /// Full-resolution size.
    pub width: u32,
    pub height: u32,
    /// Stored levels, full resolution first.
    pub levels: Vec<Level>,
    /// Channels (one for RGB slides).
    pub channels: Vec<Channel>,
    /// Z and T sizes (1 for slides).
    pub size_z: u32,
    pub size_t: u32,
    /// Associated images with their sizes.
    pub associated: Vec<(AssociatedKind, u32, u32)>,
    /// Further files of the set (OME-TIFF), by name in the opened file's folder.
    pub files: Vec<String>,
    backend: Backend,
}

impl std::fmt::Debug for Slide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slide")
            .field("props", &self.props)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("levels", &self.levels)
            .field("channels", &self.channels)
            .field("associated", &self.associated)
            .finish_non_exhaustive()
    }
}

/// Whether a TIFF file's first directory names a slide convention (cheap: header and first
/// IFD's description and private tags only).
pub fn probe_tiff(data: &[u8]) -> Option<Vendor> {
    tiffslide::probe(data)
}

/// The companion OME-XML file (a plain name in the same folder) that a "BinaryOnly" OME-TIFF
/// keeps its metadata in; open it with [`Slide::open_tiff_with_metadata`].
pub fn ome_metadata_file(data: &[u8]) -> Option<String> {
    tiffslide::metadata_file(data)
}

impl Slide {
    /// Open a TIFF-based slide. [`Error::NotSlide`] for a TIFF that is none.
    pub fn open_tiff(data: &[u8]) -> Result<Slide> {
        tiffslide::open(data)
    }

    /// Open a TIFF-based slide whose file is called `name` (an OME-TIFF set names its files:
    /// planes in a file of that name are this file's).
    pub fn open_tiff_named(data: &[u8], name: &str) -> Result<Slide> {
        tiffslide::open_named(data, Some(name))
    }

    /// Open one file of an OME-TIFF set, called `name`, with the set's OME-XML `xml` (the
    /// companion file [`ome_metadata_file`] names).
    pub fn open_tiff_with_metadata(data: &[u8], name: &str, xml: &str) -> Result<Slide> {
        tiffslide::open_with_metadata(data, Some(name), xml)
    }

    /// Open an OME-Zarr image from its store.
    pub fn open_zarr(store: Arc<dyn zarr::Store>) -> Result<Slide> {
        zarr::open(store)
    }

    /// Open the DICOM whole slide image that `members[0]` (name, bytes) is an instance of,
    /// from the instances among the other files `members[1..]` (its folder's). The files
    /// used besides the first are [`Slide::files`].
    pub fn open_dicom(members: &[(&str, &[u8])]) -> Result<Slide> {
        dicomwsi::open(members)
    }

    /// Open a MIRAX slide from its files: the `.mrxs`, `Slidedat.ini`, the index file and the
    /// data files [`mrxs::data_files`] lists, in that order. [`Slide::files`] names them
    /// (after the `.mrxs`) in the slide's folder.
    pub fn open_mrxs(files: Files) -> Result<Slide> {
        mrxs::open(files)
    }

    /// Open a Hamamatsu VMS or VMU slide from its files: the index, then the files
    /// [`vms::files`] lists, in that order (empty slices for files that could not be read).
    pub fn open_vms(files: Files) -> Result<Slide> {
        vms::open(files)
    }

    /// Open an Olympus VSI slide: `members[0]` is the `.vsi` file, the others its frame
    /// files by path from the `.vsi`'s folder (see [`vsi::open`]).
    pub fn open_vsi(members: &[(&str, &[u8])]) -> Result<Slide> {
        vsi::open(members)
    }

    /// Read a downscaled region as RGBA8.
    pub fn read(&self, files: Files, o: &ReadOptions) -> Result<Rgba> {
        let s = o.scale.max(1);
        if !s.is_power_of_two() {
            return Err(Error::Unsupported(format!("scale 1/{s}")));
        }
        let r = Region::clip(o.region, self.width.div_ceil(s), self.height.div_ceil(s))
            .ok_or_else(|| Error::Malformed("region outside the image".into()))?;
        if u64::from(r.w) * u64::from(r.h) > MAX_READ_PIXELS {
            return Err(Error::TooLarge);
        }
        let o = ReadOptions {
            scale: s,
            region: Some(r),
            ..*o
        };
        match &self.backend {
            Backend::Tiff(t) => t.read(self, files, &o),
            Backend::Ndpi(n) => n.read(files.get(0)?, &o),
            Backend::Zarr(z) => z.read(self, &o),
            Backend::Dicom(d) => d.read(files, &o),
            Backend::Stitched(m) => m.read(self, files, &o),
            Backend::External(e) => e.read(self, &o),
            Backend::Vms(v) => v.read(files, &o),
        }
    }

    /// One channel's values (any sample type as f32) of a downscaled region, for
    /// multi-channel images read as planes.
    pub fn read_channel(&self, files: Files, channel: u32, o: &ReadOptions) -> Result<Plane> {
        let s = o.scale.max(1);
        let r = Region::clip(o.region, self.width.div_ceil(s), self.height.div_ceil(s))
            .ok_or_else(|| Error::Malformed("region outside the image".into()))?;
        if u64::from(r.w) * u64::from(r.h) > MAX_READ_PIXELS {
            return Err(Error::TooLarge);
        }
        let o = ReadOptions {
            scale: s,
            region: Some(r),
            ..*o
        };
        match &self.backend {
            Backend::Tiff(t) => t.read_channel(files, channel, &o),
            Backend::Zarr(z) => z.read_channel(channel, &o),
            Backend::Stitched(m) => m.read_channel(files, channel, &o),
            Backend::External(e) => e.read_channel(channel, &o),
            Backend::Ndpi(_) | Backend::Dicom(_) | Backend::Vms(_) => {
                Err(Error::Unsupported("channels of an RGB slide".into()))
            }
        }
    }

    /// Whether the slide is read as separate channels composited in colour.
    pub fn multichannel(&self) -> bool {
        match &self.backend {
            Backend::Tiff(t) => t.multichannel(),
            Backend::Zarr(_) => true,
            Backend::Stitched(m) => m.multichannel,
            Backend::External(e) => e.multichannel(),
            Backend::Ndpi(_) | Backend::Dicom(_) | Backend::Vms(_) => false,
        }
    }

    /// Focal layers of an RGB slide read whole at any Z ([`Position::z`] in [`Slide::read`],
    /// a Hamamatsu VMS): each is a picture of the slide of its own. 1 for every other
    /// slide (Z of multi-channel images is read as planes).
    pub fn rgb_layers(&self) -> u32 {
        match &self.backend {
            Backend::Vms(_) => self.size_z.max(1),
            _ => 1,
        }
    }

    /// Decode associated image `i` (see [`Slide::associated`]).
    pub fn read_associated(
        &self,
        files: Files,
        i: usize,
        cancel: Option<&AtomicBool>,
    ) -> Result<Rgba> {
        match &self.backend {
            Backend::Tiff(t) => t.read_associated(files, i, cancel),
            Backend::Ndpi(n) => n.read_associated(files.get(0)?, i, cancel),
            Backend::Zarr(_) => Err(Error::Unsupported("associated images".into())),
            Backend::Dicom(d) => d.read_associated(files, i, cancel),
            Backend::Stitched(m) => m.read_associated(files, i, cancel),
            Backend::External(e) => e.read_associated(i, cancel),
            Backend::Vms(v) => v.read_associated(files, i, cancel),
        }
    }

    /// Bytes of the caches the slide keeps (decoded whole NDPI levels without restart
    /// markers, channel windows).
    pub fn memory_bytes(&self) -> usize {
        match &self.backend {
            Backend::Ndpi(n) => n.memory_bytes(),
            Backend::Stitched(m) => m.memory_bytes(),
            Backend::External(e) => e.memory_bytes(),
            Backend::Vms(v) => v.memory_bytes(),
            _ => 0,
        }
    }

    /// Drop the caches; returns the bytes released.
    pub fn trim_memory(&self) -> usize {
        match &self.backend {
            Backend::Ndpi(n) => n.trim_memory(),
            Backend::Stitched(m) => m.trim(),
            Backend::External(e) => e.trim(),
            Backend::Vms(v) => v.trim(),
            _ => 0,
        }
    }

    /// The coarsest power-of-two downscale worth reading (the smallest level, at least 1/8 of
    /// it below).
    pub fn max_scale(&self) -> u32 {
        let d = self.levels.last().map_or(1.0, |l| l.downsample).max(1.0);
        let p = (d.log2().round() as u32).min(20);
        (1u32 << p).saturating_mul(8).min(1 << 16)
    }
}
