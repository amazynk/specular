//! Whole-slide images and microscopy pyramids (Aperio SVS, Hamamatsu NDPI, Leica SCN,
//! generic tiled pyramidal TIFF, OME-TIFF, OME-Zarr, DICOM VL Whole Slide Microscopy) through
//! `specular-wsi`.
//!
//! A session is the slide's pyramid: [`ImageInfo::scales`] runs from 1 to the coarsest
//! power of two its smallest level serves ([`wsi::Slide::max_scale`]), so the viewer's
//! overview is read from a small level and its detail tiles from the level each zoom needs;
//! every decode reads only the tiles (NDPI: restart intervals, Zarr: chunks) that cover the
//! request. The label, macro and thumbnail images (and further OME series) are pages after
//! the slide ([`PageLabel::Slide`]); the thumbnail also serves as the embedded preview.
//!
//! [`Session::science`] gives the slide's facts and its microns per pixel
//! ([`PixelCalibration`]), for the scale bar, the position readout and the info panel's slide
//! group. Slides are shown from their RGBA raster ([`ScienceSummary::raster_only`]); a small
//! multi-channel or deep OME image (at most [`ND_MAX_PIXELS`]) is a science image whose
//! channels, Z and T are axes of real-valued planes ([`Session::nd_plane`]), shown in the
//! file's channel colours.
//!
//! Annotations a viewer saved beside the slide come with it ([`ScienceSummary::annotations`]):
//! the first of `<file>.ndpa`, `<stem>.ndpa`, `<stem>.xml` (and for a folder or `.zip`,
//! `<name>.xml`) that holds NDP.view, Aperio ImageScope or ASAP annotations.
//!
//! An OME-TIFF set may name further files: they are read from the opened file's folder only
//! (plain file names; anything with a path is ignored and its planes show as missing).
//!
//! Stitched scans: a Ventana BIF is a TIFF (by its iScan XMP); an Olympus VSI is a TIFF
//! with cellSens metadata whose pixels are the `.ets` files of its `_<name>_` folder
//! (`stackN/frame_*.ets`, at most [`VSI_FRAMES_MAX`]); a MIRAX slide is opened from its
//! `.mrxs` (a JPEG preview, so the registry chooses it by name when the `<name>/Slidedat.ini`
//! beside it exists) and reads that folder's `Slidedat.ini`, index and data files.
//!
//! A DICOM whole-slide series is one instance per level (and label, overview, thumbnail):
//! opening any of them reads the other DICOM files of its folder (at most [`DICOM_SCAN_MAX`])
//! and keeps those of the same slide, so the series opens as one pyramid; a lone instance is a
//! pyramid of its own.

#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
pub use specular_wsi as wsi;

use crate::science::{
    AnnotationSet, AxisKind, Linear, NdAxis, NdPlane, PixelCalibration, PlaneCache, ScienceKind,
    ScienceSummary,
};
use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Rect, Result, Session,
    SlideImage,
};

/// Multi-channel or deep images up to this size are science images (value planes on the
/// GPU); larger ones are composited into RGBA tiles.
pub const ND_MAX_PIXELS: u64 = 1 << 24;

/// Every power-of-two scale a slide may offer; [`ImageInfo::scales`] is a prefix.
pub(crate) static SCALES: [u32; 17] = [
    1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
];

/// Which slides a registry entry recognises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// TIFF-based slides, by content.
    Tiff,
    /// OME-Zarr folders and zipped stores.
    Zarr,
    /// MIRAX `.mrxs` with its folder.
    Mrxs,
    /// Hamamatsu VMS / VMU index with its image files.
    Vms,
}

pub struct WsiDecoder(pub Kind);

pub(crate) fn map_err(e: wsi::Error) -> DecodeError {
    match e {
        wsi::Error::NotSlide => DecodeError::UnsupportedFormat,
        wsi::Error::Malformed(s) => DecodeError::Malformed(s),
        wsi::Error::Unsupported(s) => DecodeError::Unsupported(s),
        wsi::Error::Truncated(s) => DecodeError::Truncated(s),
        wsi::Error::TooLarge => DecodeError::TooLarge,
        wsi::Error::Cancelled => DecodeError::Cancelled,
        wsi::Error::Io(s) => DecodeError::Io(std::io::Error::other(s)),
        wsi::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

fn format_of(v: wsi::Vendor) -> Format {
    match v {
        wsi::Vendor::Aperio => Format::Svs,
        wsi::Vendor::Hamamatsu => Format::Ndpi,
        wsi::Vendor::Leica => Format::Scn,
        wsi::Vendor::OmeTiff => Format::OmeTiff,
        wsi::Vendor::OmeZarr => Format::OmeZarr,
        wsi::Vendor::Generic => Format::PyramidTiff,
        wsi::Vendor::Dicom => Format::DicomWsi,
        wsi::Vendor::Ventana => Format::Bif,
        wsi::Vendor::Mirax => Format::Mrxs,
        wsi::Vendor::Olympus => Format::Vsi,
        wsi::Vendor::Zeiss => Format::Czi,
    }
}

impl Decoder for WsiDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Tiff => Format::Svs,
            Kind::Zarr => Format::OmeZarr,
            Kind::Mrxs => Format::Mrxs,
            Kind::Vms => Format::Vms,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        // The first bytes never tell a TIFF slide (it says so further in); a VMS / VMU
        // index starts with its section name.
        self.0 == Kind::Vms && wsi::vms::probe(head).is_some()
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            Kind::Tiff if wsi::vsi::probe(data) => true,
            Kind::Tiff => match wsi::probe_tiff(data) {
                None => false,
                // A tiled pyramid could be a large tiled DNG.
                Some(wsi::Vendor::Generic) => !crate::raw_dec::is_raw(data),
                Some(_) => true,
            },
            // Folders are recognised by `Registry::open`, zipped stores by extension; a
            // `.mrxs` beside its folder by `Registry::open` too.
            Kind::Zarr | Kind::Mrxs => false,
            Kind::Vms => wsi::vms::probe(data).is_some(),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Tiff => &[
                "svs", "ndpi", "scn", "ome.tif", "ome.tiff", "ome.btf", "bif", "vsi",
            ],
            Kind::Zarr => &["ome.zarr", "zarr", "zarr.zip"],
            Kind::Mrxs => &["mrxs"],
            Kind::Vms => &["vms", "vmu"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        match self.0 {
            Kind::Tiff | Kind::Mrxs | Kind::Vms => open(source),
            Kind::Zarr => open_zarr_any(source),
        }
    }
}

/// A Zarr store: an OME-Zarr image as a slide, any other store (or an OME-Zarr one the slide
/// reader refuses, such as complex or Fortran-order arrays) as plain arrays
/// ([`crate::array_dec`]).
pub(crate) fn open_zarr_any(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    match open(source.clone()) {
        Ok(s) => Ok(s),
        Err(e @ (DecodeError::UnsupportedFormat | DecodeError::Unsupported(_))) => {
            crate::array_dec::open_zarr(source).map_err(|plain| match (&e, plain) {
                // The slide reader's reason is the better one when both refuse.
                (DecodeError::Unsupported(_), _) => e,
                (_, p) => p,
            })
        }
        Err(e) => Err(e),
    }
}

/// An OME-Zarr image in `store` (a remote one: `location` is its address, for the info
/// panel) as a slide session.
pub(crate) fn open_zarr_store(
    source: Arc<Source>,
    store: Arc<dyn wsi::zarr::Store>,
    location: &str,
) -> Result<Arc<dyn Session>> {
    let mut slide = wsi::Slide::open_zarr(store).map_err(map_err)?;
    slide
        .props
        .properties
        .push(("Location".to_owned(), location.to_owned()));
    let format = format_of(slide.props.vendor);
    let n = slide.associated.len();
    let shared = Shared {
        source: source.clone(),
        files: vec![source],
        slide,
        format,
        assoc: Mutex::new(vec![None; n]),
    };
    Ok(Arc::new(MainSession::new(Arc::new(shared))))
}

/// The source's bytes as a zipped Zarr store (mapped files are read in place).
struct SourceBytes(Arc<Source>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

/// Open a slide: a TIFF-based one from its bytes, an OME-Zarr store from its folder or zip.
pub fn open(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let shared = Arc::new(Shared::open(source)?);
    Ok(Arc::new(MainSession::new(shared)))
}

/// What the pages of one slide share.
struct Shared {
    source: Arc<Source>,
    /// The files of the set, the opened one first (OME-TIFF companions after it; empty
    /// sources for files that could not be read).
    files: Vec<Arc<Source>>,
    slide: wsi::Slide,
    format: Format,
    /// Decoded associated images, by page.
    assoc: Mutex<Vec<Option<Arc<Raster>>>>,
}

impl Shared {
    fn open(source: Arc<Source>) -> Result<Shared> {
        let path = source.path().to_path_buf();
        if !source.is_folder() && wsi::dicomwsi::probe(source.bytes()) {
            return Shared::open_dicom(source);
        }
        if !source.is_folder() && wsi::vsi::probe(source.bytes()) {
            return Shared::open_vsi(source);
        }
        if !source.is_folder() && is_mrxs(&path) {
            return Shared::open_mrxs(source);
        }
        if !source.is_folder()
            && let Some(kind) = wsi::vms::probe(source.bytes())
        {
            return Shared::open_vms(source, kind);
        }
        let slide = if source.is_folder() {
            let store: Arc<dyn wsi::zarr::Store> = Arc::new(wsi::zarr::DirStore::new(&path));
            wsi::Slide::open_zarr(store).map_err(map_err)?
        } else if source.bytes().starts_with(b"PK") {
            let store = wsi::zarr::ZipStore::new(SourceBytes(source.clone())).map_err(map_err)?;
            wsi::Slide::open_zarr(Arc::new(store)).map_err(map_err)?
        } else {
            let folder = path.parent().unwrap_or(Path::new(""));
            let meta = wsi::ome_metadata_file(source.bytes());
            match (path.file_name().and_then(|n| n.to_str()), meta) {
                // One file of an OME-TIFF set whose metadata is a companion file beside it.
                (Some(name), Some(meta)) => {
                    let xml = companion(folder, &meta);
                    let xml = std::str::from_utf8(xml.bytes()).unwrap_or("");
                    wsi::Slide::open_tiff_with_metadata(source.bytes(), name, xml)
                }
                (Some(name), None) => wsi::Slide::open_tiff_named(source.bytes(), name),
                (None, _) => wsi::Slide::open_tiff(source.bytes()),
            }
            .map_err(map_err)?
        };
        let mut files = vec![source.clone()];
        let folder = path.parent().unwrap_or(Path::new(""));
        for name in &slide.files {
            files.push(Arc::new(companion(folder, name)));
        }
        let format = format_of(slide.props.vendor);
        let n = slide.associated.len();
        Ok(Shared {
            source,
            files,
            slide,
            format,
            assoc: Mutex::new(vec![None; n]),
        })
    }

    /// A DICOM WSI instance: the pyramid is the series (and the slide's label and overview
    /// instances) among the DICOM WSI files of its folder.
    fn open_dicom(source: Arc<Source>) -> Result<Shared> {
        let path = source.path().to_path_buf();
        let own = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let siblings = dicom_siblings(path.parent().unwrap_or(Path::new("")), own);
        let mut members: Vec<(&str, &[u8])> = vec![(own, source.bytes())];
        members.extend(siblings.iter().map(|(n, s)| (n.as_str(), s.bytes())));
        let slide = wsi::Slide::open_dicom(&members).map_err(map_err)?;
        let mut files = vec![source.clone()];
        for name in &slide.files {
            let s = siblings
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, s)| s.clone());
            files.push(s.unwrap_or_else(|| Arc::new(Source::from_vec(path.clone(), Vec::new()))));
        }
        let format = format_of(slide.props.vendor);
        let n = slide.associated.len();
        Ok(Shared {
            source,
            files,
            slide,
            format,
            assoc: Mutex::new(vec![None; n]),
        })
    }

    /// A VSI file and the frame files of its `_<name>_` folder.
    fn open_vsi(source: Arc<Source>) -> Result<Shared> {
        let path = source.path().to_path_buf();
        let own = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_owned();
        let folder = path.parent().unwrap_or(Path::new("")).to_path_buf();
        let names = vsi_frames(&path);
        let sources: Vec<Arc<Source>> = names
            .iter()
            .map(|n| match Source::open(folder.join(n)) {
                Ok(s) if !s.is_folder() => Arc::new(s),
                _ => Arc::new(Source::from_vec(folder.join(n), Vec::new())),
            })
            .collect();
        let mut members: Vec<(&str, &[u8])> = vec![(own.as_str(), source.bytes())];
        members.extend(
            names
                .iter()
                .map(String::as_str)
                .zip(sources.iter().map(|s| s.bytes())),
        );
        let slide = wsi::Slide::open_vsi(&members).map_err(map_err)?;
        let mut files = vec![source.clone()];
        files.extend(sources);
        let format = format_of(slide.props.vendor);
        let n = slide.associated.len();
        Ok(Shared {
            source,
            files,
            slide,
            format,
            assoc: Mutex::new(vec![None; n]),
        })
    }

    /// A MIRAX slide: the `.mrxs` and its folder's `Slidedat.ini`, index and data files.
    fn open_mrxs(source: Arc<Source>) -> Result<Shared> {
        let path = source.path().to_path_buf();
        let dir = mrxs_folder(&path).ok_or_else(|| {
            DecodeError::Unsupported(
                "the MIRAX slide's folder (with Slidedat.ini) is missing beside the .mrxs file"
                    .into(),
            )
        })?;
        let ini = Arc::new(companion(&dir, "Slidedat.ini"));
        if ini.len() > wsi::mrxs::SLIDEDAT_MAX {
            return Err(DecodeError::Malformed("Slidedat.ini is too large".into()));
        }
        let index = wsi::mrxs::index_file(ini.bytes()).unwrap_or_else(|| "Index.dat".into());
        let mut files = vec![
            source.clone(),
            ini.clone(),
            Arc::new(companion(&dir, &index)),
        ];
        for name in wsi::mrxs::data_files(ini.bytes()).map_err(map_err)? {
            files.push(Arc::new(companion(&dir, &name)));
        }
        let data: Vec<&[u8]> = files.iter().map(|s| s.bytes()).collect();
        let slide = wsi::Slide::open_mrxs(wsi::Files { data: &data }).map_err(map_err)?;
        drop(data);
        let format = format_of(slide.props.vendor);
        let n = slide.associated.len();
        Ok(Shared {
            source,
            files,
            slide,
            format,
            assoc: Mutex::new(vec![None; n]),
        })
    }

    /// A Hamamatsu VMS / VMU index and the image, optimisation, map and macro files it
    /// names (plain names in its folder; others, and missing files, read as absent).
    fn open_vms(source: Arc<Source>, kind: wsi::vms::Kind) -> Result<Shared> {
        if source.len() > wsi::vms::INDEX_MAX {
            return Err(DecodeError::Malformed("VMS index too large".into()));
        }
        let path = source.path().to_path_buf();
        let folder = path.parent().unwrap_or(Path::new("")).to_path_buf();
        let mut files = vec![source.clone()];
        for name in wsi::vms::files(source.bytes()).map_err(map_err)? {
            files.push(Arc::new(companion(&folder, &name)));
        }
        let data: Vec<&[u8]> = files.iter().map(|s| s.bytes()).collect();
        let slide = wsi::Slide::open_vms(wsi::Files { data: &data }).map_err(map_err)?;
        drop(data);
        let format = match kind {
            wsi::vms::Kind::Vms => Format::Vms,
            wsi::vms::Kind::Vmu => Format::Vmu,
        };
        let n = slide.associated.len();
        Ok(Shared {
            source,
            files,
            slide,
            format,
            assoc: Mutex::new(vec![None; n]),
        })
    }

    fn read<T>(&self, f: impl FnOnce(wsi::Files) -> wsi::Result<T>) -> Result<T> {
        let data: Vec<&[u8]> = self.files.iter().map(|s| s.bytes()).collect();
        f(wsi::Files { data: &data }).map_err(map_err)
    }

    fn associated(&self, i: usize, cancel: &CancelToken) -> Result<Arc<Raster>> {
        if let Some(Some(r)) = self.assoc.lock().get(i) {
            return Ok(r.clone());
        }
        let flag = cancel.flag();
        let img = self.read(|f| self.slide.read_associated(f, i, Some(flag)))?;
        let r = Arc::new(rgba_raster(img, (0, 0), 1));
        if let Some(slot) = self.assoc.lock().get_mut(i) {
            *slot = Some(r.clone());
        }
        Ok(r)
    }
}

/// Most frame files of a VSI read.
pub const VSI_FRAMES_MAX: usize = 256;

/// Whether `path` is a `.mrxs` file with its slide folder (`<name>/Slidedat.ini`) beside it.
pub fn is_mrxs(path: &Path) -> bool {
    mrxs_folder(path).is_some()
}

fn mrxs_folder(path: &Path) -> Option<std::path::PathBuf> {
    let ext = path.extension()?.to_str()?;
    if !ext.eq_ignore_ascii_case("mrxs") {
        return None;
    }
    let dir = path.parent()?.join(path.file_stem()?);
    dir.join("Slidedat.ini").is_file().then_some(dir)
}

/// The frame files of a VSI file (`_<name>_/stackN/frame_*.ets`), by path from its folder,
/// in name order.
fn vsi_frames(path: &Path) -> Vec<String> {
    let (Some(stem), Some(folder)) = (path.file_stem().and_then(|s| s.to_str()), path.parent())
    else {
        return Vec::new();
    };
    let dir = folder.join(format!("_{stem}_"));
    let list = |d: &Path| -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(d)
            .map(|r| {
                r.flatten()
                    .take(4096)
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    };
    let mut out = Vec::new();
    for stack in list(&dir) {
        if !dir.join(&stack).is_dir() {
            continue;
        }
        for name in list(&dir.join(&stack)) {
            if name.starts_with("frame_")
                && name.to_ascii_lowercase().ends_with(".ets")
                && out.len() < VSI_FRAMES_MAX
            {
                out.push(format!("_{stem}_/{stack}/{name}"));
            }
        }
    }
    out
}

/// A companion file of an OME-TIFF set: a plain name in `folder`, else an empty source (its
/// planes read as missing).
/// Most folder entries looked at for the other instances of a DICOM WSI series.
const DICOM_SCAN_MAX: usize = 4096;

/// The DICOM WSI files beside `own` in `folder` (regular files only, by name).
fn dicom_siblings(folder: &Path, own: &str) -> Vec<(String, Arc<Source>)> {
    let Ok(dir) = std::fs::read_dir(if folder.as_os_str().is_empty() {
        Path::new(".")
    } else {
        folder
    }) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in dir.flatten().take(DICOM_SCAN_MAX) {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name == own || !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        if let Ok(s) = Source::open(entry.path())
            && wsi::dicomwsi::probe(s.bytes())
        {
            out.push((name, Arc::new(s)));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The sidecar names an annotation file of the slide at `path` may have, in order.
fn annotation_sidecars(path: &Path) -> Vec<std::path::PathBuf> {
    let (Some(folder), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str()))
    else {
        return Vec::new();
    };
    let stem = path.file_stem().and_then(|n| n.to_str()).unwrap_or(name);
    // `slide.ome.tif` also looks for `slide.xml`.
    let short = stem.strip_suffix(".ome").unwrap_or(stem);
    let mut out = vec![folder.join(format!("{name}.ndpa"))];
    for s in [stem, short] {
        for ext in ["ndpa", "xml"] {
            let p = folder.join(format!("{s}.{ext}"));
            if p != path && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// The slide's annotations, from the first sidecar beside it that holds some.
fn annotations(path: &Path, slide: &wsi::Slide) -> Option<Arc<AnnotationSet>> {
    let frame = slide
        .props
        .mpp
        .filter(|m| m.0 > 0.0 && m.1 > 0.0)
        .map(|(x, y)| wsi::annot::NdpaFrame {
            nm_per_px: (x * 1000.0, y * 1000.0),
            offset_nm: slide.props.centre_offset_nm.unwrap_or((0.0, 0.0)),
            width: slide.width,
            height: slide.height,
        })
        // Without the slide's offset only a slide centred on the glass is placed right.
        .filter(|_| slide.props.centre_offset_nm.is_some());
    for p in annotation_sidecars(path) {
        let Ok(meta) = std::fs::metadata(&p) else {
            continue;
        };
        if !meta.is_file() || meta.len() > wsi::annot::MAX_BYTES as u64 {
            continue;
        }
        let Ok(src) = Source::open(&p) else {
            continue;
        };
        match wsi::annot::parse(src.bytes(), frame.as_ref()) {
            Ok(set) => {
                return Some(Arc::new(AnnotationSet {
                    file: p.file_name()?.to_string_lossy().into_owned(),
                    set,
                    depth: None,
                }));
            }
            Err(wsi::Error::NotSlide) => {}
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "annotations {}: {e}", p.display());
            }
        }
    }
    None
}

fn companion(folder: &Path, name: &str) -> Source {
    let plain = Path::new(name).file_name().and_then(|n| n.to_str()) == Some(name)
        && name != "."
        && name != "..";
    let p = folder.join(name);
    if plain
        && let Ok(s) = Source::open(&p)
        && !s.is_folder()
    {
        return s;
    }
    specular_log::debug!(target: specular_log::DECODE, "OME-TIFF companion {name:?} not readable from {}", folder.display());
    Source::from_vec(p, Vec::new())
}

pub(crate) fn rgba_raster(img: wsi::Rgba, origin: (u32, u32), scale: u32) -> Raster {
    Raster {
        width: img.width,
        height: img.height,
        origin,
        scale,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: img.width,
            height: img.height,
            stride: img.width as usize * 4,
            data: img.data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: img.truncated,
    }
}

/// The scale list for a slide whose coarsest useful scale is `max`.
pub(crate) fn scales_upto(max: u32) -> &'static [u32] {
    let n = SCALES.iter().take_while(|&&s| s <= max.max(8)).count();
    &SCALES[..n]
}

/// The slide itself (page 0).
struct MainSession {
    shared: Arc<Shared>,
    info: ImageInfo,
    summary: ScienceSummary,
    /// Science planes, for small multi-channel / deep images.
    nd: bool,
    planes: PlaneCache,
    /// The focal layer shown, of [`wsi::Slide::rgb_layers`] (pages before the associated
    /// images).
    z: u32,
}

impl MainSession {
    fn new(shared: Arc<Shared>) -> MainSession {
        MainSession::at_layer(shared, 0)
    }

    fn at_layer(shared: Arc<Shared>, z: u32) -> MainSession {
        let s = &shared.slide;
        let levels: Vec<(u32, u32)> = s.levels.iter().map(|l| (l.width, l.height)).collect();
        let mut coding = s.props.sample.clone();
        if levels.len() > 1 {
            coding += &format!(", {} levels", levels.len());
        }
        let info = ImageInfo {
            format: shared.format,
            width: s.width,
            height: s.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: true,
            scales: scales_upto(s.max_scale()),
        };
        let deep = s.multichannel() || s.size_z > 1 || s.size_t > 1;
        // RGB focal layers are pages, not planes
        let nd = deep
            && s.rgb_layers() == 1
            && u64::from(s.width) * u64::from(s.height) <= ND_MAX_PIXELS;
        let mut summary = summary(&shared, &levels, nd);
        summary.annotations = annotations(shared.source.path(), s);
        let z = z.min(s.rgb_layers() - 1);
        MainSession {
            shared,
            info,
            summary,
            nd,
            planes: PlaneCache::new(),
            z,
        }
    }

    /// Plane `index` (channel, z, t on the summary's axes) at full size.
    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        let mut pos = [0u32; 3];
        for (a, &i) in self.summary.axes.iter().zip(index) {
            let k = match a.name.as_str() {
                "C" => 0,
                "Z" => 1,
                _ => 2,
            };
            pos[k] = i.min(a.len.saturating_sub(1));
        }
        self.planes.get_or(index, cancel, || {
            let flag = cancel.flag();
            let o = wsi::ReadOptions {
                scale: 1,
                region: None,
                position: wsi::Position {
                    z: pos[1],
                    t: pos[2],
                },
                cancel: Some(flag),
                parallel: true,
            };
            let p = self
                .shared
                .read(|f| self.shared.slide.read_channel(f, pos[0], &o))?;
            Ok(NdPlane {
                width: p.width,
                height: p.height,
                values: p.values,
                truncated: p.truncated,
            })
        })
    }
}

/// The science summary of a slide: its facts, microns per pixel, and for science images the
/// C, Z and T axes.
fn summary(shared: &Shared, levels: &[(u32, u32)], nd: bool) -> ScienceSummary {
    let s = &shared.slide;
    let p = &s.props;
    let mut axes = Vec::new();
    let mut channel_axis = None;
    let mut colours = Vec::new();
    if nd {
        let c = s.channels.len() as u32;
        if c > 1 {
            channel_axis = Some(axes.len());
            axes.push(NdAxis {
                name: "C".into(),
                len: c,
                unit: String::new(),
                labels: s.channels.iter().map(|c| c.name.clone()).collect(),
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Channel,
            });
            if s.channels.iter().all(|c| c.colour.is_some()) {
                colours = s
                    .channels
                    .iter()
                    .filter_map(|c| c.colour)
                    .map(|c| c.map(|v| f32::from(v) / 255.0))
                    .collect();
            }
        }
        for (name, len, step, unit) in [
            ("Z", s.size_z, p.z_step, "µm"),
            ("T", s.size_t, p.t_step, "s"),
        ] {
            if len > 1 {
                axes.push(NdAxis {
                    name: name.into(),
                    len,
                    unit: step.map_or(String::new(), |_| unit.to_owned()),
                    labels: Vec::new(),
                    wcs_axis: None,
                    linear: step.map(|step| Linear { offset: 0.0, step }),
                    kind: AxisKind::Cube,
                });
            }
        }
    }
    let pixel = p
        .mpp
        .filter(|m| m.0 > 0.0 && m.1 > 0.0)
        .map(|(x, y)| PixelCalibration {
            x: Linear {
                offset: 0.0,
                step: x,
            },
            y: Linear {
                offset: 0.0,
                step: y,
            },
            unit_x: "µm".into(),
            unit_y: "µm".into(),
        });
    ScienceSummary {
        width: s.width,
        height: s.height,
        axes,
        channel_axis,
        sample: p.sample.clone(),
        instrument: p.scanner.clone(),
        date_obs: p.date.clone(),
        title: p.name.clone(),
        keywords: Arc::new(
            p.properties
                .iter()
                .map(|(k, v)| (k.clone(), v.clone(), String::new()))
                .collect(),
        ),
        kind: ScienceKind::Slide,
        pixel,
        magnification: p.objective,
        channel_colours: colours,
        raster_only: !nd,
        vendor: Some(p.vendor.name().to_owned()),
        levels: levels.to_vec(),
        ..Default::default()
    }
}

pub(crate) fn region(r: Option<Rect>) -> Option<wsi::Region> {
    r.map(|r| wsi::Region {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    })
}

/// The power of two at or above `s`.
pub(crate) fn pow2(s: u32) -> u32 {
    s.max(1).checked_next_power_of_two().unwrap_or(1 << 31)
}

impl Session for MainSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.shared.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(
            if self.shared.slide.multichannel() && self.shared.slide.channels.len() > 1 {
                "Multichannel"
            } else if self.shared.slide.multichannel() {
                "Greyscale"
            } else {
                "RGB"
            },
        )
    }
    /// The scanner's thumbnail, when the slide has one.
    fn decode_preview(&self) -> Option<Result<Raster>> {
        let i = self
            .shared
            .slide
            .associated
            .iter()
            .position(|a| a.0 == wsi::AssociatedKind::Thumbnail)?;
        Some(
            self.shared
                .associated(i, &CancelToken::new())
                .map(|r| (*r).clone()),
        )
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let scale = pow2(req.scale);
        let flag = cancel.flag();
        let o = wsi::ReadOptions {
            scale,
            region: region(req.region),
            position: wsi::Position { z: self.z, t: 0 },
            cancel: Some(flag),
            parallel: req.parallel,
        };
        let img = self.shared.read(|f| self.shared.slide.read(f, &o))?;
        let origin = req.region.map_or((0, 0), |r| (r.x, r.y));
        Ok(rgba_raster(img, origin, scale))
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = pow2(req.scale);
        let (w, h) = match req.region {
            Some(r) => (r.w, r.h),
            None => (self.info.width.div_ceil(s), self.info.height.div_ceil(s)),
        };
        // The output, the level pixels it is reduced from and the tiles decoding meanwhile.
        u64::from(w) * u64::from(h) * 4 * 3 + (16 << 20)
    }
    fn memory_bytes(&self) -> usize {
        self.shared.slide.memory_bytes() + self.planes.memory_bytes() + assoc_bytes(&self.shared)
    }
    fn trim_memory(&self) -> usize {
        let a = assoc_bytes(&self.shared);
        self.shared.assoc.lock().iter_mut().for_each(|r| *r = None);
        self.shared.slide.trim_memory() + self.planes.clear() + a
    }
    fn page_count(&self) -> u32 {
        page_count(&self.shared)
    }
    fn page_index(&self) -> u32 {
        self.z
    }
    fn page_label(&self) -> Option<PageLabel> {
        let layers = self.shared.slide.rgb_layers();
        if layers > 1 {
            return Some(PageLabel::Plane {
                z: Some((self.z, layers)),
                t: None,
            });
        }
        (self.page_count() > 1).then_some(PageLabel::Slide(SlideImage::Main))
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        open_page(&self.shared, page)
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        self.nd.then(|| self.plane(index, cancel))
    }
}

/// The part of an RGBA raster (whole, at its scale) inside `r`, exactly.
pub(crate) fn crop(src: &Raster, r: Rect) -> Raster {
    let x0 = r.x.min(src.width);
    let y0 = r.y.min(src.height);
    let x1 = r.x.saturating_add(r.w).min(src.width);
    let y1 = r.y.saturating_add(r.h).min(src.height);
    let (w, h) = (x1 - x0, y1 - y0);
    let p = &src.planes[0];
    let mut data = Vec::with_capacity(w as usize * h as usize * 4);
    for y in y0..y1 {
        let s = y as usize * p.stride + x0 as usize * 4;
        data.extend_from_slice(&p.data[s..s + w as usize * 4]);
    }
    let img = wsi::Rgba {
        width: w,
        height: h,
        data,
        truncated: src.truncated,
    };
    rgba_raster(img, (x0, y0), src.scale)
}

fn assoc_bytes(shared: &Shared) -> usize {
    shared
        .assoc
        .lock()
        .iter()
        .flatten()
        .map(|r| r.memory_bytes())
        .sum()
}

/// The slide's focal layers (one page each, usually one), then its associated images.
fn page_count(shared: &Shared) -> u32 {
    shared.slide.rgb_layers() + shared.slide.associated.len() as u32
}

fn open_page(shared: &Arc<Shared>, page: u32) -> Result<Arc<dyn Session>> {
    let layers = shared.slide.rgb_layers();
    if page < layers {
        return Ok(Arc::new(MainSession::at_layer(shared.clone(), page)));
    }
    let i = (page - layers) as usize;
    let (kind, w, h) = shared
        .slide
        .associated
        .get(i)
        .cloned()
        .ok_or_else(|| DecodeError::Malformed("no such page".into()))?;
    let label = match kind {
        wsi::AssociatedKind::Label => SlideImage::Label,
        wsi::AssociatedKind::Macro => SlideImage::Macro,
        wsi::AssociatedKind::Thumbnail => SlideImage::Thumbnail,
        wsi::AssociatedKind::Other(n) => SlideImage::Other(n.into()),
    };
    let info = ImageInfo {
        format: shared.format,
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: false,
        bit_depth: 8,
        progressive: false,
        coding: shared.slide.props.sample.clone(),
        embedded_preview: None,
        fast_regions: false,
        scales: &SCALES[..4],
    };
    Ok(Arc::new(AssocSession {
        shared: shared.clone(),
        index: i,
        page,
        label,
        info,
    }))
}

/// An associated image (label, macro, thumbnail, another series): decoded whole once, then
/// reduced and cropped.
struct AssocSession {
    shared: Arc<Shared>,
    index: usize,
    page: u32,
    label: SlideImage,
    info: ImageInfo,
}

impl Session for AssocSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.shared.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let full = self.shared.associated(self.index, cancel)?;
        let scale = pow2(req.scale);
        let level = if scale > 1 {
            let src = wsi::Rgba {
                width: full.width,
                height: full.height,
                data: full.planes[0].data.clone(),
                truncated: full.truncated,
            };
            rgba_raster(wsi::box_reduce_rgba(&src, scale), (0, 0), scale)
        } else {
            (*full).clone()
        };
        Ok(match req.region {
            Some(r) => crop(&level, r),
            None => level,
        })
    }
    fn decode_memory(&self, _req: &DecodeRequest) -> u64 {
        u64::from(self.info.width) * u64::from(self.info.height) * 4 * 3
    }
    fn memory_bytes(&self) -> usize {
        assoc_bytes(&self.shared)
    }
    fn trim_memory(&self) -> usize {
        let a = assoc_bytes(&self.shared);
        self.shared.assoc.lock().iter_mut().for_each(|r| *r = None);
        a
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn page_count(&self) -> u32 {
        page_count(&self.shared)
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        Some(PageLabel::Slide(self.label.clone()))
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        open_page(&self.shared, page)
    }
}
