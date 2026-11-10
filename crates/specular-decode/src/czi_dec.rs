//! Zeiss CZI microscopy files (ZEN, Axioscan, Celldiscoverer, LSM 9x0) through libCZI.
//!
//! libCZI never runs in the viewer: a decode host started with `--czi` (a pool of its own,
//! [`pool`]) loads Specular's libCZI bridge and reads the file through the descriptor or handle
//! the viewer passes with every request ([`specular_sandbox::CziSource`]). The host keeps the
//! document open between requests, so a tile costs one region read: libCZI composes the
//! sub-blocks (and pyramid layers) that cover it, in whatever coding the file has (raw, JPEG,
//! JPEG XR, zstd). libCZI is the user's own install (LGPL-3.0, dynamically loaded); without it,
//! or with the setting off ([`set_czi`]), the file is refused with what is needed.
//!
//! * **Pages:** one per scene (the tissue sections of a slide, the wells of a plate), then the
//!   label, slide preview and thumbnail attachments ([`PageLabel::Slide`]). The thumbnail is
//!   also the embedded preview.
//! * **Slides and mosaics** are `specular-wsi` slides ([`wsi::external`]): levels from the
//!   scene's pyramid layers, every decode reads only its region at its zoom, fluorescence
//!   channels composited in their ZEN colours through their ZEN display windows (black and
//!   white point). Brightfield (BGR24, BGR48) slides read as RGB.
//! * **Stacks:** an image of at most [`wsi_dec::ND_MAX_PIXELS`] with several channels or Z, T,
//!   phase, illumination, rotation or view positions is a science image: each axis has a
//!   slider, planes are real values, channels are named and coloured from the metadata and
//!   open at their display ranges. A colour (BGR) stack's channels are its red, green and blue.
//!   Larger stacks show their first Z, T and other positions.
//! * **Facts:** microns per pixel (scale bar, readout), Z step, objective, microscope,
//!   acquisition date, channels (fluorophore, excitation and emission), codings and the
//!   pyramid, from libCZI and the metadata XML.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use parking_lot::{Mutex, RwLock};
use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_sandbox::czi::{CziChannel, CziRect};
use specular_sandbox::{
    Config, CziInfo, CziPlaneKind, CziRegion, CziSource, Error as HostError, ErrorKind, Host,
};

use crate::science::{
    AxisKind, Linear, NdAxis, NdPlane, PixelCalibration, PixelSource, PlaneCache, ScienceKind,
    ScienceSummary,
};
use crate::wsi_dec::{self, wsi};
use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session, SlideImage,
};

/// How Zeiss CZI files may be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CziConfig {
    /// Open them with libCZI when it is installed (on by default).
    pub enabled: bool,
    /// A folder holding libCZI's library, when it is not where the system looks.
    pub library_dir: Option<PathBuf>,
}

impl Default for CziConfig {
    fn default() -> Self {
        CziConfig {
            enabled: std::env::var_os("SPECULAR_CZI").is_none_or(|v| v != "0"),
            library_dir: None,
        }
    }
}

static CONFIG: RwLock<Option<CziConfig>> = RwLock::new(None);

/// Sets how CZI files may be opened (the app calls this from its settings).
pub fn set_czi(c: Option<CziConfig>) {
    *CONFIG.write() = c;
}

fn config() -> CziConfig {
    CONFIG.read().clone().unwrap_or_default()
}

/// The ZISRAW file segment that starts a CZI file.
pub const MAGIC: &[u8] = b"ZISRAWFILE";

/// Longest one read may take in the host (a low zoom of a mosaic without a pyramid reads
/// every tile).
const TIMEOUT: Duration = Duration::from_secs(180);
/// Longest metadata XML read (ZEN's runs to a few MB with large experiments).
const MAX_XML: u64 = 16 << 20;
/// Longest side an associated image is read at.
const ASSOCIATED_MAX: u32 = 8192;
/// Most channels kept (the composite shows 16).
const MAX_CHANNELS: usize = 1024;

/// The CZI host pool for this library folder (rebuilt when the folder changes).
fn pool(dir: &Option<PathBuf>) -> Arc<Host> {
    static POOL: Mutex<Option<(Option<PathBuf>, Arc<Host>)>> = Mutex::new(None);
    let mut g = POOL.lock();
    if let Some((d, h)) = &*g
        && d == dir
    {
        return h.clone();
    }
    let mut cfg = Config::from_env();
    cfg.czi = true;
    // Two: a tile the user waits for need not queue behind a thumbnail's.
    cfg.pool_size = cfg.pool_size.max(2);
    cfg.timeout = std::env::var("SPECULAR_CZI_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(TIMEOUT, |ms| Duration::from_millis(ms.max(1)));
    // Tiles are small, but libCZI decodes whole sub-blocks: up to the budget's share of the
    // memory available now.
    if let Some(avail) = specular_core::available_memory() {
        cfg.memory_limit = cfg.memory_limit.max(avail / 5 * 4).min(1 << 46);
    }
    if let Some(d) = dir {
        let var = if cfg!(windows) {
            "PATH"
        } else if cfg!(target_os = "macos") {
            "DYLD_FALLBACK_LIBRARY_PATH"
        } else {
            "LD_LIBRARY_PATH"
        };
        let mut paths = vec![d.clone()];
        if let Some(old) = std::env::var_os(var) {
            paths.extend(std::env::split_paths(&old));
        }
        if let Ok(joined) = std::env::join_paths(paths) {
            cfg.env.push((var.into(), joined));
        }
    }
    let h = Arc::new(Host::new(cfg));
    *g = Some((dir.clone(), h.clone()));
    h
}

/// libCZI is not on this computer (or not where the user pointed): the file needs the
/// optional decoder (the app's "needs a free decoder" card).
fn needs_libczi(detail: &str) -> DecodeError {
    specular_log::debug!(target: specular_log::DECODE, "CZI: libCZI missing: {detail}");
    DecodeError::NeedsDecoder(crate::OptionalId::Czi)
}

/// The folder holding libCZI's shared library and its version from the file name
/// (`liblibCZI.so.0.70.0`, `liblibCZI.0.70.0.dylib`; "installed" when the name has none).
/// Only looks on disk: the decode host loads it when a file needs it, and says then if it
/// can't.
pub fn find_czi(dir: Option<&Path>) -> std::result::Result<(PathBuf, String), String> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    match dir {
        Some(d) => dirs.push(d.to_path_buf()),
        None => {
            let var = if cfg!(windows) {
                "PATH"
            } else if cfg!(target_os = "macos") {
                "DYLD_FALLBACK_LIBRARY_PATH"
            } else {
                "LD_LIBRARY_PATH"
            };
            if let Some(v) = std::env::var_os(var) {
                dirs.extend(std::env::split_paths(&v));
            }
            let system: &[&str] = if cfg!(target_os = "macos") {
                &["/opt/homebrew/lib", "/usr/local/lib"]
            } else {
                &[
                    "/usr/lib",
                    "/usr/lib64",
                    "/usr/local/lib",
                    "/usr/local/lib64",
                    "/usr/lib/x86_64-linux-gnu",
                    "/usr/lib/aarch64-linux-gnu",
                ]
            };
            dirs.extend(system.iter().map(PathBuf::from));
        }
    }
    for d in &dirs {
        let Ok(rd) = std::fs::read_dir(d) else {
            continue;
        };
        let mut best: Option<String> = None;
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(v) = libczi_version(&name) else {
                continue;
            };
            // The fullest version wins ("0.70.0" over "1" of the soname link).
            if best.as_ref().is_none_or(|b| v.len() > b.len()) {
                best = Some(v);
            }
        }
        if let Some(v) = best {
            let v = if v.is_empty() {
                "installed".to_owned()
            } else {
                v
            };
            return Ok((d.clone(), v));
        }
    }
    Err(match dir {
        Some(d) => format!("no libCZI library in {}", d.display()),
        None => "libCZI's library was not found where the system keeps libraries".to_owned(),
    })
}

/// The version in a libCZI library's file name ("" when it has none), or `None` for other
/// files.
fn libczi_version(name: &str) -> Option<String> {
    if cfg!(windows) || name.ends_with(".dll") {
        return name.eq_ignore_ascii_case("libCZI.dll").then(String::new);
    }
    let rest = name.strip_prefix("liblibCZI")?;
    if let Some(v) = rest.strip_prefix(".so") {
        return match v {
            "" => Some(String::new()),
            v => v.strip_prefix('.').map(str::to_owned),
        };
    }
    let v = rest.strip_suffix(".dylib")?;
    Some(v.trim_start_matches('.').to_owned())
}

fn map_err(e: HostError) -> DecodeError {
    match e {
        HostError::NotInstalled(m) => DecodeError::Unsupported(format!(
            "Zeiss CZI files need libCZI, which cannot run: {m}"
        )),
        HostError::Unavailable(m) => DecodeError::Unsupported(format!(
            "Zeiss CZI files need the sandboxed decode host, which cannot run: {m}"
        )),
        HostError::Remote { kind, message } => match kind {
            // This build cannot run libCZI at all: installing it would not help.
            ErrorKind::NotInstalled if message.contains("no libCZI bridge") => {
                DecodeError::Unsupported(format!("Zeiss CZI files need libCZI: {message}"))
            }
            ErrorKind::NotInstalled => needs_libczi(&message),
            ErrorKind::Unsupported | ErrorKind::Encrypted => DecodeError::Unsupported(message),
            ErrorKind::TooLarge => DecodeError::Malformed(format!("too large: {message}")),
            ErrorKind::Io => DecodeError::Io(std::io::Error::other(message)),
            _ => DecodeError::Malformed(message),
        },
        HostError::Timeout(t) => DecodeError::Malformed(format!(
            "libCZI took longer than {} s on this file",
            t.as_secs()
        )),
        e @ (HostError::Crashed(_) | HostError::Protocol(_)) => {
            DecodeError::Malformed(format!("libCZI failed on this file ({e})"))
        }
        HostError::Io(m) => DecodeError::Io(std::io::Error::other(m)),
    }
}

pub(crate) struct CziDecoder;

impl Decoder for CziDecoder {
    fn format(&self) -> Format {
        Format::Czi
    }
    fn probe(&self, head: &[u8]) -> bool {
        head.starts_with(MAGIC)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        data.starts_with(MAGIC)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["czi"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        open(source)
    }
}

/// Opens a CZI file: its first scene.
pub(crate) fn open(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let cfg = config();
    if !cfg.enabled {
        return Err(DecodeError::Unsupported(
            "Zeiss CZI files open with libCZI, which is switched off in Settings › Optional \
             decoders"
                .into(),
        ));
    }
    let t0 = std::time::Instant::now();
    let file = czi_source(&source)?;
    let host = pool(&cfg.library_dir);
    let raw = host
        .czi_info(&file, MAX_XML, MAX_XML + (16 << 20))
        .map_err(map_err)?;
    let info = CziInfo::decode(&raw, MAX_XML as usize)
        .map_err(|e| DecodeError::Malformed(format!("libCZI's facts: {e}")))?;
    drop(raw);
    let doc = Arc::new(Doc::new(source, file, host, info)?);
    specular_log::debug!(
        target: specular_log::DECODE,
        "CZI: {} sub-blocks, {} scenes, {} channels read by libCZI in the host in {:.0} ms",
        doc.info.subblocks,
        doc.images.len(),
        doc.channels.len(),
        t0.elapsed().as_secs_f64() * 1e3
    );
    open_page(&doc, 0)
}

/// The file for the host: the opened file itself while it still holds the source's bytes,
/// else a private copy.
fn czi_source(source: &Source) -> Result<CziSource> {
    let bytes = source.bytes();
    if let Ok(s) = CziSource::open(source.path())
        && s.len() == bytes.len() as u64
    {
        let edge = bytes.len().min(64 << 10);
        let tail = bytes.len() - edge;
        if s.read_at(0, edge).is_some_and(|b| b == bytes[..edge])
            && s.read_at(tail as u64, edge)
                .is_some_and(|b| b == bytes[tail..])
        {
            return Ok(s);
        }
    }
    CziSource::from_bytes(bytes).map_err(map_err)
}

/// One displayed channel.
#[derive(Clone, Debug)]
struct Chan {
    /// The C coordinate, when the document has a C dimension.
    c: Option<i32>,
    /// libCZI's pixel type.
    pixel: u8,
    /// Which sample of a BGR pixel (0 blue, 1 green, 2 red), for a colour stack split into
    /// channels.
    component: Option<usize>,
    name: String,
    colour: Option<[u8; 3]>,
    /// Values shown black and full colour (ZEN's black and white point).
    window: Option<(f64, f64)>,
}

/// One image page: a scene, or the whole document when it has none.
#[derive(Clone, Debug)]
struct ImagePage {
    scene: Option<i32>,
    rect: CziRect,
    name: String,
}

/// An associated image page.
#[derive(Clone, Debug)]
struct AssocPage {
    attachment: u32,
    label: SlideImage,
}

/// A further dimension a stack may have, besides C: letter, axis name, start, size.
#[derive(Clone, Copy, Debug)]
struct Extra {
    letter: u8,
    start: i32,
    size: u32,
}

/// What the pages of a file share.
struct Doc {
    source: Arc<Source>,
    file: CziSource,
    host: Arc<Host>,
    info: CziInfo,
    facts: XmlFacts,
    images: Vec<ImagePage>,
    assoc: Vec<AssocPage>,
    /// Decoded associated images, by associated page.
    assoc_rasters: Mutex<Vec<Option<Arc<Raster>>>>,
    /// The channels as stored (C order).
    channels: Vec<CziChannel>,
    /// The dimensions past C that a plane has (Z, T, H, I, R, V, B), with more than one or a
    /// non-zero position.
    extras: Vec<Extra>,
}

/// The facts the metadata XML adds.
#[derive(Default, Debug)]
struct XmlFacts {
    microscope: Option<String>,
    objective: Option<String>,
    magnification: Option<f64>,
    na: Option<f64>,
    acquired: Option<String>,
    application: Option<String>,
    detector: Option<String>,
    /// Per channel (by position): fluorophore, excitation and emission wavelength (nm).
    channels: Vec<(Option<String>, Option<f64>, Option<f64>)>,
    /// Time between T positions, seconds.
    t_step: Option<f64>,
}

fn xml_facts(xml: Option<&str>) -> XmlFacts {
    let mut f = XmlFacts::default();
    let Some(root) = xml.and_then(|x| wsi::xml::parse(x).ok()) else {
        return f;
    };
    let Some(info) = root.child("Metadata").and_then(|m| m.child("Information")) else {
        return f;
    };
    let text = |e: Option<&wsi::xml::Element>| {
        e.map(|e| e.text.trim().to_owned())
            .filter(|t| !t.is_empty())
    };
    let num = |e: Option<&wsi::xml::Element>| {
        e.and_then(|e| e.text.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    if let Some(inst) = info.child("Instrument") {
        f.microscope = inst
            .child("Microscopes")
            .and_then(|m| m.child("Microscope"))
            .and_then(|m| m.attr("Name"))
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        if let Some(o) = inst.child("Objectives").and_then(|o| o.child("Objective")) {
            f.objective = o
                .attr("Name")
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty());
            f.magnification = num(o.child("NominalMagnification")).filter(|m| *m > 0.0);
            f.na = num(o.child("LensNA")).filter(|m| *m > 0.0);
        }
        f.detector = inst
            .child("Detectors")
            .and_then(|d| d.child("Detector"))
            .and_then(|d| {
                d.attr("Name")
                    .map(str::to_owned)
                    .or_else(|| text(d.child("Model")))
            })
            .filter(|s| !s.trim().is_empty());
    }
    if let Some(app) = info.child("Application") {
        let name = text(app.child("Name"));
        let ver = text(app.child("Version"));
        f.application = match (name, ver) {
            (Some(n), Some(v)) => Some(format!("{n} {v}")),
            (n, _) => n,
        };
    }
    if let Some(img) = info.child("Image") {
        f.acquired = text(img.child("AcquisitionDateAndTime"));
        if let Some(dims) = img.child("Dimensions") {
            if let Some(ch) = dims.child("Channels") {
                for c in ch.all("Channel").take(MAX_CHANNELS) {
                    f.channels.push((
                        text(c.child("Fluor")),
                        num(c.child("ExcitationWavelength")),
                        num(c.child("EmissionWavelength")),
                    ));
                }
            }
            f.t_step = dims
                .child("T")
                .and_then(|t| t.child("Positions"))
                .and_then(|p| p.child("Interval"))
                .and_then(|i| num(i.child("Increment")))
                .filter(|s| *s > 0.0);
        }
    }
    f
}

/// libCZI's pixel type names.
fn pixel_text(p: u8) -> &'static str {
    match p {
        0 => "8-bit grey",
        1 => "16-bit grey",
        2 => "32-bit float grey",
        3 => "8-bit BGR",
        4 => "16-bit BGR",
        8 => "32-bit float BGR",
        _ => "unknown pixels",
    }
}

fn is_bgr(p: u8) -> bool {
    matches!(p, 3 | 4 | 8)
}

/// libCZI's compression names (raw identifiers).
fn coding_text(c: i32) -> String {
    match c {
        0 => "uncompressed".into(),
        1 => "JPEG".into(),
        2 => "LZW".into(),
        4 => "JPEG XR".into(),
        5 => "zstd".into(),
        6 => "zstd (header)".into(),
        7 => "chunked".into(),
        c => format!("coding {c}"),
    }
}

/// The largest sample value of an integer pixel type (display settings are fractions of it).
fn sample_max(p: u8) -> Option<f64> {
    match p {
        0 | 3 => Some(255.0),
        1 | 4 => Some(65535.0),
        _ => None,
    }
}

impl Doc {
    fn new(source: Arc<Source>, file: CziSource, host: Arc<Host>, info: CziInfo) -> Result<Doc> {
        if info.subblocks == 0 || info.bbox_layer0.is_empty() {
            return Err(DecodeError::Unsupported("the file has no image".into()));
        }
        let facts = xml_facts(info.xml.as_deref());
        // Scenes, in index order; the whole document when it has none or one.
        let mut scenes: Vec<_> = info
            .scenes
            .iter()
            .filter(|s| !s.bbox_layer0.is_empty())
            .copied()
            .collect();
        scenes.sort_by_key(|s| s.index);
        let images: Vec<ImagePage> = if scenes.len() > 1 {
            scenes
                .iter()
                .map(|s| ImagePage {
                    scene: Some(s.index),
                    rect: s.bbox_layer0,
                    name: format!("Scene {}", s.index + 1),
                })
                .collect()
        } else {
            vec![ImagePage {
                scene: scenes.first().map(|s| s.index),
                rect: scenes.first().map_or(info.bbox_layer0, |s| s.bbox_layer0),
                name: String::new(),
            }]
        };
        let mut assoc = Vec::new();
        for (want, label) in [
            ("Label", SlideImage::Label),
            ("SlidePreview", SlideImage::Macro),
            ("Thumbnail", SlideImage::Thumbnail),
        ] {
            if let Some(a) = info.attachments.iter().find(|a| {
                a.name == want && matches!(a.content_type.as_str(), "CZI" | "JPG" | "PNG" | "BMP")
            }) {
                assoc.push(AssocPage {
                    attachment: a.index,
                    label,
                });
            }
        }
        let mut channels = info.channels.clone();
        channels.truncate(MAX_CHANNELS);
        if channels.iter().all(|c| c.pixel.is_none()) {
            let types: Vec<&str> = info.pixels.iter().map(|(p, _)| pixel_text(*p)).collect();
            return Err(DecodeError::Unsupported(format!(
                "the file's pixel type is not supported ({})",
                if types.is_empty() {
                    "none given".to_owned()
                } else {
                    types.join(", ")
                }
            )));
        }
        let mut extras = Vec::new();
        for letter in *b"ZTHIRVB" {
            if let Some(d) = info.dim(letter)
                && d.size > 0
            {
                extras.push(Extra {
                    letter,
                    start: d.start,
                    size: d.size,
                });
            }
        }
        let n = assoc.len();
        Ok(Doc {
            source,
            file,
            host,
            info,
            facts,
            images,
            assoc,
            assoc_rasters: Mutex::new(vec![None; n]),
            channels,
            extras,
        })
    }

    fn pages(&self) -> u32 {
        (self.images.len() + self.assoc.len()) as u32
    }

    fn has_c(&self) -> bool {
        self.info.dim(b'C').is_some()
    }

    /// The channels shown and whether they are composited (else the first is read as RGB).
    fn display_channels(&self, split_rgb: bool) -> (Vec<Chan>, bool) {
        let has_c = self.has_c();
        let first = &self.channels[0];
        if let Some(p) = first.pixel.filter(|p| is_bgr(*p)) {
            if !split_rgb {
                return (Vec::new(), false);
            }
            let max = sample_max(p);
            let window = max.map(|m| (0.0, m));
            let chans = [
                ("R", 2, [255, 0, 0]),
                ("G", 1, [0, 255, 0]),
                ("B", 0, [0, 0, 255]),
            ]
            .iter()
            .map(|(n, k, col)| Chan {
                c: has_c.then_some(first.index),
                pixel: p,
                component: Some(*k),
                name: (*n).to_owned(),
                colour: Some(*col),
                window,
            })
            .collect();
            return (chans, true);
        }
        let chans = self
            .channels
            .iter()
            .filter_map(|c| {
                let pixel = c.pixel.filter(|p| !is_bgr(*p))?;
                let window = c.display.and_then(|d| {
                    let m = sample_max(pixel)?;
                    let (lo, hi) = (f64::from(d.black) * m, f64::from(d.white) * m);
                    (hi > lo).then_some((lo, hi))
                });
                Some(Chan {
                    c: has_c.then_some(c.index),
                    pixel,
                    component: None,
                    name: c.name.clone(),
                    colour: c.display.and_then(|d| d.tint),
                    window,
                })
            })
            .collect();
        (chans, true)
    }

    /// The plane coordinates of extra dimensions at their positions `pos` (in `extras`
    /// order), with channel `c`.
    fn coords(&self, c: Option<i32>, pos: &[u32]) -> ([(u8, i32); 8], u8) {
        let mut out = [(0u8, 0i32); 8];
        let mut n = 0;
        if let Some(c) = c {
            out[n] = (b'C', c);
            n += 1;
        }
        for (k, e) in self.extras.iter().enumerate() {
            if n < out.len() {
                let p = pos
                    .get(k)
                    .copied()
                    .unwrap_or(0)
                    .min(e.size.saturating_sub(1));
                out[n] = (e.letter, e.start.saturating_add(p as i32));
                n += 1;
            }
        }
        (out, n as u8)
    }

    /// One channel of a raw region at `zoom`: `want` (width, height) values, NaN where libCZI
    /// returned fewer.
    #[allow(clippy::too_many_arguments)]
    fn read_values(
        &self,
        scene: Option<i32>,
        raw: CziRect,
        zoom: f32,
        want: (u32, u32),
        chan: &Chan,
        pos: &[u32],
        cancel: Option<&AtomicBool>,
    ) -> wsi::Result<wsi::Plane> {
        check_cancel(cancel)?;
        let bpp = specular_sandbox::protocol::czi_pixel_bytes(chan.pixel)
            .ok_or_else(|| wsi::Error::Unsupported("pixel type".into()))?;
        let (coords, coord_count) = self.coords(chan.c, pos);
        let region = CziRegion {
            x: raw.x,
            y: raw.y,
            w: raw.w,
            h: raw.h,
            zoom,
            scene: scene.and_then(|s| u32::try_from(s).ok()),
            coords,
            coord_count,
            pixel: chan.pixel,
            max_bytes: (u64::from(want.0) + 4) * (u64::from(want.1) + 4) * bpp as u64,
        };
        let (w, h, data) = self
            .host
            .czi_read(&self.file, region, want)
            .map_err(|e| wsi::Error::Malformed(map_err(e).to_string()))?;
        let mut values = vec![f32::NAN; want.0 as usize * want.1 as usize];
        let comps = if is_bgr(chan.pixel) { 3 } else { 1 };
        let k = chan.component.unwrap_or(0).min(comps - 1);
        let row = w as usize * bpp;
        for y in 0..h.min(want.1) as usize {
            let src = &data[y * row..(y + 1) * row];
            let dst = &mut values[y * want.0 as usize..(y + 1) * want.0 as usize];
            for (x, d) in dst.iter_mut().enumerate().take(w as usize) {
                *d = sample(src, chan.pixel, x * comps + k);
            }
        }
        Ok(wsi::Plane {
            width: want.0,
            height: want.1,
            values,
            truncated: false,
        })
    }

    /// The first channel of a raw region of a colour image as RGBA8, `want` pixels (black
    /// where libCZI returned fewer).
    #[allow(clippy::too_many_arguments)]
    fn read_rgba(
        &self,
        scene: Option<i32>,
        raw: CziRect,
        zoom: f32,
        want: (u32, u32),
        pixel: u8,
        window: Option<(f64, f64)>,
        cancel: Option<&AtomicBool>,
    ) -> wsi::Result<wsi::Rgba> {
        check_cancel(cancel)?;
        let bpp = specular_sandbox::protocol::czi_pixel_bytes(pixel)
            .ok_or_else(|| wsi::Error::Unsupported("pixel type".into()))?;
        let c = self.has_c().then_some(self.channels[0].index);
        let (coords, coord_count) = self.coords(c, &[]);
        let region = CziRegion {
            x: raw.x,
            y: raw.y,
            w: raw.w,
            h: raw.h,
            zoom,
            scene: scene.and_then(|s| u32::try_from(s).ok()),
            coords,
            coord_count,
            pixel,
            max_bytes: (u64::from(want.0) + 4) * (u64::from(want.1) + 4) * bpp as u64,
        };
        let (w, h, data) = self
            .host
            .czi_read(&self.file, region, want)
            .map_err(|e| wsi::Error::Malformed(map_err(e).to_string()))?;
        Ok(bgr_to_rgba(&data, (w, h), want, pixel, window))
    }

    /// Associated page `i`, decoded once.
    fn associated(&self, i: usize, cancel: &CancelToken) -> Result<Arc<Raster>> {
        if let Some(Some(r)) = self.assoc_rasters.lock().get(i) {
            return Ok(r.clone());
        }
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let a = self
            .assoc
            .get(i)
            .ok_or_else(|| DecodeError::Malformed("no such page".into()))?;
        let (plane, data) = self
            .host
            .czi_attachment(&self.file, a.attachment, ASSOCIATED_MAX, 256 << 20)
            .map_err(map_err)?;
        let img = match plane.kind {
            CziPlaneKind::Encoded => {
                let flag = cancel.flag();
                let img =
                    wsi::external::decode_image(&data, Some(flag)).map_err(wsi_dec::map_err)?;
                let mut f = 1;
                while img.width / f > ASSOCIATED_MAX || img.height / f > ASSOCIATED_MAX {
                    f *= 2;
                }
                wsi::box_reduce_rgba(&img, f)
            }
            CziPlaneKind::Pixels => {
                let window = plane.display.and_then(|d| {
                    let m = sample_max(plane.pixel)?;
                    let (lo, hi) = (f64::from(d.black) * m, f64::from(d.white) * m);
                    (hi > lo).then_some((lo, hi))
                });
                let size = (plane.width, plane.height);
                if is_bgr(plane.pixel) {
                    bgr_to_rgba(&data, size, size, plane.pixel, window)
                } else {
                    grey_to_rgba(&data, size, plane.pixel, window)
                }
            }
        };
        let r = Arc::new(wsi_dec::rgba_raster(img, (0, 0), 1));
        if let Some(slot) = self.assoc_rasters.lock().get_mut(i) {
            *slot = Some(r.clone());
        }
        Ok(r)
    }

    fn assoc_bytes(&self) -> usize {
        self.assoc_rasters
            .lock()
            .iter()
            .flatten()
            .map(|r| r.memory_bytes())
            .sum()
    }

    fn clear_assoc(&self) -> usize {
        let n = self.assoc_bytes();
        self.assoc_rasters.lock().iter_mut().for_each(|r| *r = None);
        n
    }
}

fn check_cancel(c: Option<&AtomicBool>) -> wsi::Result<()> {
    if c.is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed)) {
        Err(wsi::Error::Cancelled)
    } else {
        Ok(())
    }
}

/// Sample `i` of a row in libCZI's pixel type `p` (little endian).
fn sample(row: &[u8], p: u8, i: usize) -> f32 {
    match p {
        0 | 3 => f32::from(row[i]),
        1 | 4 => f32::from(u16::from_le_bytes([row[2 * i], row[2 * i + 1]])),
        2 | 8 => f32::from_le_bytes([row[4 * i], row[4 * i + 1], row[4 * i + 2], row[4 * i + 3]]),
        _ => f32::NAN,
    }
}

/// BGR rows (`got` pixels) into a `want`-sized RGBA8 image: 8-bit as stored, 16-bit through
/// `window` (else the top 8 bits), float clamped to 0..1.
fn bgr_to_rgba(
    data: &[u8],
    got: (u32, u32),
    want: (u32, u32),
    pixel: u8,
    window: Option<(f64, f64)>,
) -> wsi::Rgba {
    let (ww, wh) = (want.0 as usize, want.1 as usize);
    let mut out = vec![0u8; ww * wh * 4];
    for px in out.chunks_exact_mut(4) {
        px[3] = 255;
    }
    let bpp = specular_sandbox::protocol::czi_pixel_bytes(pixel).unwrap_or(3);
    let row = got.0 as usize * bpp;
    let level = |v: f32| -> u8 {
        let t = match (pixel, window) {
            (0 | 3, _) => return v as u8,
            (1 | 4, Some((lo, hi))) => ((f64::from(v) - lo) / (hi - lo)) as f32,
            (1 | 4, None) => return (v as u32 >> 8) as u8,
            _ => v,
        };
        (t.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
    };
    for y in 0..(got.1 as usize).min(wh) {
        let Some(src) = data.get(y * row..(y + 1) * row) else {
            break;
        };
        for x in 0..(got.0 as usize).min(ww) {
            let o = (y * ww + x) * 4;
            for (k, ch) in [2usize, 1, 0].iter().enumerate() {
                out[o + k] = level(sample(src, pixel, x * 3 + ch));
            }
        }
    }
    wsi::Rgba {
        width: want.0,
        height: want.1,
        data: out,
        truncated: false,
    }
}

/// A grey image through `window` (else its own range) as RGBA8.
fn grey_to_rgba(data: &[u8], size: (u32, u32), pixel: u8, window: Option<(f64, f64)>) -> wsi::Rgba {
    let n = size.0 as usize * size.1 as usize;
    let values: Vec<f32> = (0..n).map(|i| sample(data, pixel, i)).collect();
    let (lo, hi) = window.unwrap_or_else(|| {
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for &v in &values {
            if v.is_finite() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        if hi > lo {
            (f64::from(lo), f64::from(hi))
        } else {
            (0.0, 1.0)
        }
    });
    let mut out = Vec::with_capacity(n * 4);
    for v in values {
        let t = ((f64::from(v) - lo) / (hi - lo)).clamp(0.0, 1.0);
        let g = if t.is_finite() {
            (t * 255.0 + 0.5) as u8
        } else {
            0
        };
        out.extend_from_slice(&[g, g, g, 255]);
    }
    wsi::Rgba {
        width: size.0,
        height: size.1,
        data: out,
        truncated: false,
    }
}

/// The raw region of `rect` that a scaled region covers (clipped to `rect`).
fn raw_region(rect: CziRect, r: wsi::Region, scale: u32) -> CziRect {
    let s = u64::from(scale);
    let x0 = u64::from(r.x) * s;
    let y0 = u64::from(r.y) * s;
    let w = (u64::from(r.w) * s)
        .min(u64::from(rect.w).saturating_sub(x0))
        .max(1);
    let h = (u64::from(r.h) * s)
        .min(u64::from(rect.h).saturating_sub(y0))
        .max(1);
    CziRect {
        x: rect.x.saturating_add(x0.min(i32::MAX as u64) as i32),
        y: rect.y.saturating_add(y0.min(i32::MAX as u64) as i32),
        w: w.min(u64::from(u32::MAX)) as u32,
        h: h.min(u64::from(u32::MAX)) as u32,
    }
}

/// The `specular-wsi` reader of one image page.
struct Reader {
    doc: Arc<Doc>,
    page: ImagePage,
    chans: Vec<Chan>,
    /// The pixel type of an RGB slide (not composited).
    rgb: Option<(u8, Option<(f64, f64)>)>,
    /// Plane positions on `doc.extras` besides Z and T (their first).
    z_axis: Option<usize>,
    t_axis: Option<usize>,
}

impl Reader {
    fn pos(&self, p: wsi::Position) -> Vec<u32> {
        let mut pos = vec![0; self.doc.extras.len()];
        if let Some(z) = self.z_axis {
            pos[z] = p.z;
        }
        if let Some(t) = self.t_axis {
            pos[t] = p.t;
        }
        pos
    }
}

impl wsi::external::External for Reader {
    fn read_channel(&self, channel: u32, o: &wsi::ReadOptions) -> wsi::Result<wsi::Plane> {
        let chan = self
            .chans
            .get(channel as usize)
            .ok_or_else(|| wsi::Error::Malformed("no such channel".into()))?;
        let r = o
            .region
            .ok_or_else(|| wsi::Error::Malformed("no region".into()))?;
        let s = o.scale.max(1);
        let raw = raw_region(self.page.rect, r, s);
        self.doc.read_values(
            self.page.scene,
            raw,
            1.0 / s as f32,
            (r.w, r.h),
            chan,
            &self.pos(o.position),
            o.cancel,
        )
    }

    fn read_rgb(&self, o: &wsi::ReadOptions) -> wsi::Result<wsi::Rgba> {
        let (pixel, window) = self
            .rgb
            .ok_or_else(|| wsi::Error::Malformed("not an RGB image".into()))?;
        let r = o
            .region
            .ok_or_else(|| wsi::Error::Malformed("no region".into()))?;
        let s = o.scale.max(1);
        let raw = raw_region(self.page.rect, r, s);
        self.doc.read_rgba(
            self.page.scene,
            raw,
            1.0 / s as f32,
            (r.w, r.h),
            pixel,
            window,
            o.cancel,
        )
    }

    fn read_associated(&self, _i: usize, _cancel: Option<&AtomicBool>) -> wsi::Result<wsi::Rgba> {
        Err(wsi::Error::Unsupported(
            "associated images are pages".into(),
        ))
    }
}

/// The pages of a document: images first, then associated images.
fn open_page(doc: &Arc<Doc>, page: u32) -> Result<Arc<dyn Session>> {
    let n = doc.images.len() as u32;
    if page < n {
        return Ok(Arc::new(CziSession::new(doc.clone(), page)?));
    }
    let i = (page - n) as usize;
    if i >= doc.assoc.len() {
        return Err(DecodeError::Unsupported("no such page".into()));
    }
    let r = doc.associated(i, &CancelToken::new())?;
    let info = ImageInfo {
        format: Format::Czi,
        width: r.width,
        height: r.height,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: false,
        bit_depth: 8,
        progressive: false,
        coding: "8-bit RGB".into(),
        embedded_preview: None,
        fast_regions: false,
        scales: &wsi_dec::SCALES[..4],
    };
    Ok(Arc::new(AssocSession {
        doc: doc.clone(),
        index: i,
        page,
        info,
    }))
}

/// One scene (or the whole document).
struct CziSession {
    doc: Arc<Doc>,
    page: u32,
    image: ImagePage,
    slide: wsi::Slide,
    info: ImageInfo,
    summary: ScienceSummary,
    /// Science planes for a small stack: the channels and the extra dimensions they span.
    nd: Option<Vec<Chan>>,
    planes: PlaneCache,
}

impl CziSession {
    fn new(doc: Arc<Doc>, page: u32) -> Result<CziSession> {
        let image = doc.images[page as usize].clone();
        let (w, h) = (image.rect.w, image.rect.h);
        let px = u64::from(w) * u64::from(h);
        let deep_extras = doc.extras.iter().any(|e| e.size > 1);
        let (all_chans, multichannel) = doc.display_channels(false);
        let rgb = (!multichannel).then(|| {
            let c = &doc.channels[0];
            let p = c.pixel.unwrap_or(3);
            let window = c.display.and_then(|d| {
                let m = sample_max(p).filter(|_| p == 4)?;
                let (lo, hi) = (f64::from(d.black) * m, f64::from(d.white) * m);
                (hi > lo).then_some((lo, hi))
            });
            (p, window)
        });
        let deep = deep_extras || all_chans.len() > 1;
        let nd_chans = (deep && px <= wsi_dec::ND_MAX_PIXELS).then(|| doc.display_channels(true).0);
        let nd_chans = nd_chans.filter(|c| !c.is_empty());
        // Levels from the scene's pyramid: each layer `minification` times smaller.
        let pyramid = doc.info.pyramid(image.scene);
        let mut levels = vec![wsi::Level {
            width: w,
            height: h,
            downsample: 1.0,
        }];
        if let Some(p) = pyramid {
            let mut layers: Vec<_> = p
                .layers
                .iter()
                .filter(|l| l.minification >= 2 && l.layer > 0 && l.count > 0)
                .collect();
            layers.sort_by_key(|l| l.layer);
            for l in layers.iter().take(24) {
                let d = f64::from(l.minification).powi(i32::from(l.layer));
                if d > 1.0 && d < 1e7 {
                    levels.push(wsi::Level {
                        width: ((f64::from(w) / d).ceil() as u32).max(1),
                        height: ((f64::from(h) / d).ceil() as u32).max(1),
                        downsample: d,
                    });
                }
            }
        }
        let z = doc.extras.iter().position(|e| e.letter == b'Z');
        let t = doc.extras.iter().position(|e| e.letter == b'T');
        let size = |k: Option<usize>| k.map_or(1, |k| doc.extras[k].size);
        let mut props = wsi::Props::new(wsi::Vendor::Zeiss);
        props.mpp = match doc.info.scaling {
            [Some(x), Some(y), _] => Some((x * 1e6, y * 1e6)),
            _ => None,
        };
        props.objective = doc.facts.magnification;
        props.scanner = doc.facts.microscope.clone();
        props.date = doc.facts.acquired.clone();
        props.name = Some(image.name.clone()).filter(|n| !n.is_empty());
        props.z_step = doc.info.scaling[2].map(|z| z * 1e6);
        props.t_step = doc.facts.t_step;
        props.sample = sample_summary(&doc.info);
        let channels: Vec<wsi::Channel> = all_chans
            .iter()
            .map(|c| wsi::Channel {
                name: c.name.clone(),
                colour: c.colour,
                window: c.window,
            })
            .collect();
        let spec = wsi::external::Spec {
            props,
            width: w,
            height: h,
            levels: levels.clone(),
            channels: if multichannel {
                channels
            } else {
                vec![wsi::Channel::default()]
            },
            size_z: size(z),
            size_t: size(t),
            associated: Vec::new(),
            multichannel,
        };
        let reader = Reader {
            doc: doc.clone(),
            page: image.clone(),
            chans: all_chans,
            rgb,
            z_axis: z,
            t_axis: t,
        };
        let slide = wsi::Slide::open_external(spec, Box::new(reader)).map_err(wsi_dec::map_err)?;
        let level_sizes: Vec<(u32, u32)> = levels.iter().map(|l| (l.width, l.height)).collect();
        let mut coding = slide.props.sample.clone();
        if level_sizes.len() > 1 {
            coding += &format!(", {} levels", level_sizes.len());
        }
        let bits = doc.channels[0].pixel.map_or(8, |p| match p {
            0 | 3 => 8,
            1 | 4 => 16,
            _ => 32,
        });
        let info = ImageInfo {
            format: Format::Czi,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: bits,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: true,
            scales: wsi_dec::scales_upto(slide.max_scale()),
        };
        let summary = summary(&doc, &image, &slide, nd_chans.as_deref(), &level_sizes);
        Ok(CziSession {
            doc,
            page,
            image,
            slide,
            info,
            summary,
            nd: nd_chans,
            planes: PlaneCache::new(),
        })
    }

    /// Plane `index` (channel, then the extra dimensions with more than one position) at full
    /// size.
    fn plane(&self, chans: &[Chan], index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        let s = &self.summary;
        for (k, a) in s.axes.iter().enumerate() {
            if index.get(k).copied().unwrap_or(0) >= a.len {
                return Err(DecodeError::Malformed("no such plane".into()));
            }
        }
        let mut k = 0;
        let chan = if s.channel_axis.is_some() {
            k = 1;
            &chans[index[0] as usize]
        } else {
            &chans[0]
        };
        let mut pos = vec![0u32; self.doc.extras.len()];
        for (e, p) in self.doc.extras.iter().zip(pos.iter_mut()) {
            if e.size > 1 {
                *p = index.get(k).copied().unwrap_or(0);
                k += 1;
            }
        }
        self.planes.get_or(index, cancel, || {
            let r = self.image.rect;
            let p = self
                .doc
                .read_values(
                    self.image.scene,
                    r,
                    1.0,
                    (r.w, r.h),
                    chan,
                    &pos,
                    Some(cancel.flag()),
                )
                .map_err(wsi_dec::map_err)?;
            Ok(NdPlane {
                width: p.width,
                height: p.height,
                values: p.values,
                truncated: p.truncated,
            })
        })
    }
}

/// "16-bit grey (14 significant), JPEG XR": the main pixel type and the codings.
fn sample_summary(info: &CziInfo) -> String {
    let mut pixels: Vec<_> = info.pixels.clone();
    pixels.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let mut s = pixels
        .first()
        .map_or("unknown pixels".to_owned(), |(p, _)| {
            pixel_text(*p).to_owned()
        });
    if let Some(b) = info.channels.first().and_then(|c| c.bits)
        && pixels.first().is_some_and(|(p, _)| matches!(p, 1 | 4))
        && b < 16
    {
        s += &format!(" ({b} significant)");
    }
    let mut codings: Vec<_> = info.codings.clone();
    codings.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let names: Vec<String> = codings.iter().map(|(c, _)| coding_text(*c)).collect();
    if !names.is_empty() {
        s += ", ";
        s += &names.join(" + ");
    }
    s
}

fn kw(k: impl Into<String>, v: impl ToString) -> (String, String, String) {
    (k.into(), v.to_string(), String::new())
}

/// The axis name and unit of an extra dimension.
fn extra_name(letter: u8) -> &'static str {
    match letter {
        b'Z' => "Z",
        b'T' => "T",
        b'H' => "Phase",
        b'I' => "Illumination",
        b'R' => "Rotation",
        b'V' => "View",
        b'B' => "Block",
        _ => "",
    }
}

fn summary(
    doc: &Doc,
    image: &ImagePage,
    slide: &wsi::Slide,
    nd: Option<&[Chan]>,
    levels: &[(u32, u32)],
) -> ScienceSummary {
    let info = &doc.info;
    let f = &doc.facts;
    let mut axes = Vec::new();
    let mut channel_axis = None;
    let mut colours = Vec::new();
    let mut ranges = Vec::new();
    if let Some(chans) = nd {
        if chans.len() > 1 {
            channel_axis = Some(0);
            axes.push(NdAxis {
                name: "C".into(),
                len: chans.len() as u32,
                unit: String::new(),
                labels: chans.iter().map(|c| c.name.clone()).collect(),
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Channel,
            });
            if chans.iter().all(|c| c.colour.is_some()) {
                colours = chans
                    .iter()
                    .filter_map(|c| c.colour)
                    .map(|c| c.map(|v| f32::from(v) / 255.0))
                    .collect();
            }
        }
        if chans.iter().all(|c| c.window.is_some()) {
            ranges = chans.iter().filter_map(|c| c.window).collect();
        }
        for e in doc.extras.iter().filter(|e| e.size > 1) {
            let (unit, step) = match e.letter {
                b'Z' => ("µm", info.scaling[2].map(|z| z * 1e6)),
                b'T' => ("s", f.t_step),
                _ => ("", None),
            };
            axes.push(NdAxis {
                name: extra_name(e.letter).into(),
                len: e.size,
                unit: step.map_or(String::new(), |_| unit.to_owned()),
                labels: Vec::new(),
                wcs_axis: None,
                linear: step.map(|step| Linear { offset: 0.0, step }),
                kind: AxisKind::Cube,
            });
        }
    }
    let pixel = slide
        .props
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
    let mut k = vec![kw(
        "CZI version",
        format!("{}.{}", info.version.0, info.version.1),
    )];
    k.push(kw("Sub-blocks", info.subblocks));
    k.push(kw("Pixels", sample_summary(info)));
    let dims: Vec<String> = info
        .dims
        .iter()
        .map(|d| format!("{} {}", char::from(d.letter), d.size))
        .collect();
    if !dims.is_empty() {
        k.push(kw("Dimensions", dims.join(", ")));
    }
    if let Some((lo, hi)) = info.m_range {
        k.push(kw("Mosaic tiles", i64::from(hi) - i64::from(lo) + 1));
    }
    if doc.images.len() > 1 {
        k.push(kw("Scenes", doc.images.len()));
    }
    if let Some(s) = image.scene {
        k.push(kw("Scene", s + 1));
    }
    k.push(kw(
        "Position",
        format!("{}, {} (sub-block coordinates)", image.rect.x, image.rect.y),
    ));
    if levels.len() > 1 {
        let l: Vec<String> = levels.iter().map(|(w, h)| format!("{w}×{h}")).collect();
        k.push(kw("Pyramid", l.join(", ")));
    }
    if let Some(z) = info.scaling[2] {
        k.push(kw("Z step (µm)", format!("{:.4}", z * 1e6)));
    }
    for (i, c) in doc.channels.iter().enumerate() {
        let mut v = if c.name.is_empty() {
            format!("#{}", c.index)
        } else {
            c.name.clone()
        };
        if let Some((fluor, ex, em)) = f.channels.get(i) {
            if let Some(fl) = fluor.as_ref().filter(|fl| *fl != &c.name) {
                v += &format!(", {fl}");
            }
            if let Some(ex) = ex {
                v += &format!(", excitation {ex} nm");
            }
            if let Some(em) = em {
                v += &format!(", emission {em} nm");
            }
        }
        if let Some(p) = c.pixel {
            v += &format!(", {}", pixel_text(p));
        }
        if let Some(b) = c.bits {
            v += &format!(", {b} bits");
        }
        if let Some(d) = c.display {
            if let Some([r, g, b]) = d.tint {
                v += &format!(", colour #{r:02X}{g:02X}{b:02X}");
            }
            v += &format!(", display {:.4}–{:.4}", d.black, d.white);
            if !d.enabled {
                v += ", hidden in ZEN";
            }
        }
        k.push(kw(format!("Channel {}", i + 1), v));
    }
    if let Some(o) = &f.objective {
        k.push(kw("Objective", o));
    }
    if let Some(na) = f.na {
        k.push(kw("Numerical aperture", na));
    }
    if let Some(d) = &f.detector {
        k.push(kw("Detector", d));
    }
    if let Some(a) = &f.application {
        k.push(kw("Software", a));
    }
    if let Some(d) = &info.document {
        for (key, v) in [
            ("Name", &d.name),
            ("Title", &d.title),
            ("Description", &d.description),
            ("Created", &d.created),
            ("User", &d.user),
            ("Comment", &d.comment),
        ] {
            if !v.trim().is_empty() {
                k.push(kw(key, v.trim()));
            }
        }
    }
    let other: Vec<String> = doc
        .extras
        .iter()
        .filter(|e| e.size > 1 && !matches!(e.letter, b'Z' | b'T'))
        .map(|e| format!("{} {}", extra_name(e.letter), e.size))
        .collect();
    if nd.is_none() && (!other.is_empty() || slide.size_z > 1 || slide.size_t > 1) {
        k.push(kw("Shown", "the first Z, T and other positions"));
    }
    let title = info
        .document
        .as_ref()
        .map(|d| {
            if d.title.trim().is_empty() {
                &d.name
            } else {
                &d.title
            }
        })
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty());
    let pyramid = levels.len() > 1;
    ScienceSummary {
        width: image.rect.w,
        height: image.rect.h,
        axes,
        channel_axis,
        sample: slide.props.sample.clone(),
        instrument: f.microscope.clone(),
        date_obs: f.acquired.clone(),
        title,
        keywords: Arc::new(k),
        kind: if pyramid || nd.is_none() {
            ScienceKind::Slide
        } else {
            ScienceKind::Microscopy
        },
        pixel,
        pixel_source: Some(PixelSource::Vendor),
        magnification: f.magnification,
        detector: f.objective.clone(),
        channel_colours: colours,
        display_ranges: ranges,
        raster_only: nd.is_none(),
        vendor: Some("Zeiss".into()),
        levels: levels.to_vec(),
        ..Default::default()
    }
}

impl Session for CziSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.doc.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if !self.slide.multichannel() {
            "RGB"
        } else if self.slide.channels.len() > 1 {
            "Multichannel"
        } else {
            "Greyscale"
        })
    }
    /// The file's thumbnail attachment, when it has one.
    fn decode_preview(&self) -> Option<Result<Raster>> {
        let i = self
            .doc
            .assoc
            .iter()
            .position(|a| a.label == SlideImage::Thumbnail)?;
        Some(
            self.doc
                .associated(i, &CancelToken::new())
                .map(|r| (*r).clone()),
        )
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let scale = wsi_dec::pow2(req.scale);
        let flag = cancel.flag();
        let o = wsi::ReadOptions {
            scale,
            region: wsi_dec::region(req.region),
            position: wsi::Position::default(),
            cancel: Some(flag),
            parallel: req.parallel,
        };
        let img = self
            .slide
            .read(wsi::Files { data: &[] }, &o)
            .map_err(wsi_dec::map_err)?;
        let origin = req.region.map_or((0, 0), |r| (r.x, r.y));
        Ok(wsi_dec::rgba_raster(img, origin, scale))
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = wsi_dec::pow2(req.scale);
        let (w, h) = match req.region {
            Some(r) => (r.w, r.h),
            None => (self.info.width.div_ceil(s), self.info.height.div_ceil(s)),
        };
        // The output and one plane per composited channel (f32), plus the host's reply.
        let channels = self.slide.channels.len().clamp(1, 16) as u64;
        u64::from(w) * u64::from(h) * (4 + 4 * channels + 8) + (16 << 20)
    }
    fn memory_bytes(&self) -> usize {
        self.planes.memory_bytes() + self.doc.assoc_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.planes.clear() + self.doc.clear_assoc()
    }
    fn page_count(&self) -> u32 {
        self.doc.pages()
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        if self.doc.pages() <= 1 {
            return None;
        }
        Some(PageLabel::Slide(
            if self.page == 0 && self.doc.images.len() == 1 {
                SlideImage::Main
            } else {
                SlideImage::Other(Arc::from(self.image.name.as_str()))
            },
        ))
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        open_page(&self.doc, page)
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        let chans = self.nd.as_ref()?;
        Some(self.plane(chans, index, cancel))
    }
}

/// A label, slide preview or thumbnail: decoded whole once, then reduced and cropped.
struct AssocSession {
    doc: Arc<Doc>,
    index: usize,
    page: u32,
    info: ImageInfo,
}

impl Session for AssocSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.doc.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let full = self.doc.associated(self.index, cancel)?;
        let scale = wsi_dec::pow2(req.scale);
        let level = if scale > 1 {
            let src = wsi::Rgba {
                width: full.width,
                height: full.height,
                data: full.planes[0].data.clone(),
                truncated: full.truncated,
            };
            wsi_dec::rgba_raster(wsi::box_reduce_rgba(&src, scale), (0, 0), scale)
        } else {
            (*full).clone()
        };
        Ok(match req.region {
            Some(r) => wsi_dec::crop(&level, r),
            None => level,
        })
    }
    fn decode_memory(&self, _req: &DecodeRequest) -> u64 {
        u64::from(self.info.width) * u64::from(self.info.height) * 4 * 3
    }
    fn memory_bytes(&self) -> usize {
        self.doc.assoc_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.doc.clear_assoc()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn page_count(&self) -> u32 {
        self.doc.pages()
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        Some(PageLabel::Slide(self.doc.assoc[self.index].label.clone()))
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        open_page(&self.doc, page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libczi_is_found_by_its_file_name() {
        if !cfg!(windows) {
            assert_eq!(
                libczi_version("liblibCZI.so.0.70.0").as_deref(),
                Some("0.70.0")
            );
            assert_eq!(libczi_version("liblibCZI.so").as_deref(), Some(""));
            assert_eq!(
                libczi_version("liblibCZI.0.70.0.dylib").as_deref(),
                Some("0.70.0")
            );
            assert_eq!(libczi_version("liblibCZIAPI.so"), None);
            assert_eq!(libczi_version("libczi.txt"), None);
        }
        let d = std::env::temp_dir().join(format!("czi-find-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        assert!(find_czi(Some(&d)).is_err());
        let name = if cfg!(windows) {
            "libCZI.dll"
        } else {
            "liblibCZI.so.0.70.0"
        };
        std::fs::write(d.join(name), b"").unwrap();
        std::fs::write(d.join("liblibCZI.so.1"), b"").unwrap();
        let (dir, v) = find_czi(Some(&d)).unwrap();
        assert_eq!(dir, d);
        assert_eq!(v, if cfg!(windows) { "installed" } else { "0.70.0" });
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn raw_regions_clip_to_the_image() {
        let rect = CziRect {
            x: -100,
            y: 50,
            w: 1000,
            h: 600,
        };
        let r = |x, y, w, h| wsi::Region { x, y, w, h };
        assert_eq!(
            raw_region(rect, r(0, 0, 250, 150), 4),
            CziRect {
                x: -100,
                y: 50,
                w: 1000,
                h: 600
            }
        );
        // The last column of a 1/4 image covers the remaining 1000 - 249×4 = 4 pixels.
        assert_eq!(raw_region(rect, r(249, 0, 1, 1), 4).w, 4);
        assert_eq!(raw_region(rect, r(10, 10, 5, 5), 1).x, -90);
    }

    #[test]
    fn samples_and_colours() {
        let row = [1u8, 2, 3, 4, 0, 0, 128, 63];
        assert_eq!(sample(&row, 0, 2), 3.0);
        assert_eq!(sample(&row, 1, 1), f32::from(u16::from_le_bytes([3, 4])));
        assert_eq!(sample(&row, 2, 1), 1.0);
        // BGR24 → RGBA, padded to the wanted size.
        let img = bgr_to_rgba(&[10, 20, 30], (1, 1), (2, 1), 3, None);
        assert_eq!(img.data, [30, 20, 10, 255, 0, 0, 0, 255]);
        // BGR48 through a window.
        let px: Vec<u8> = [0u16, 32768, 65535]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let img = bgr_to_rgba(&px, (1, 1), (1, 1), 4, Some((0.0, 32768.0)));
        assert_eq!(&img.data[..3], &[255, 255, 0]);
        let g = grey_to_rgba(&[0, 255], (2, 1), 0, None);
        assert_eq!(g.data, [0, 0, 0, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn xml_facts_are_read() {
        let xml = r#"<ImageDocument><Metadata><Information>
            <Application><Name>ZEN</Name><Version>3.4</Version></Application>
            <Image><AcquisitionDateAndTime>2021-06-15T06:14:13Z</AcquisitionDateAndTime>
              <Dimensions><Channels>
                <Channel Id="Channel:0" Name="DAPI"><Fluor>DAPI</Fluor>
                  <ExcitationWavelength>353</ExcitationWavelength>
                  <EmissionWavelength>465</EmissionWavelength></Channel>
              </Channels>
              <T><Positions><Interval><Increment>2.5</Increment></Interval></Positions></T>
              </Dimensions></Image>
            <Instrument><Microscopes><Microscope Name="Celldiscoverer 7"/></Microscopes>
              <Objectives><Objective Name="Plan-Apochromat 5x/0.35"><LensNA>0.35</LensNA>
                <NominalMagnification>5</NominalMagnification></Objective></Objectives>
            </Instrument></Information></Metadata></ImageDocument>"#;
        let f = xml_facts(Some(xml));
        assert_eq!(f.microscope.as_deref(), Some("Celldiscoverer 7"));
        assert_eq!(f.magnification, Some(5.0));
        assert_eq!(f.na, Some(0.35));
        assert_eq!(f.application.as_deref(), Some("ZEN 3.4"));
        assert_eq!(f.channels[0].2, Some(465.0));
        assert_eq!(f.t_step, Some(2.5));
        // Not XML, or another document: nothing, no panic.
        assert!(xml_facts(Some("<a><b")).microscope.is_none());
        assert!(xml_facts(None).objective.is_none());
    }

    #[test]
    fn refused_when_turned_off() {
        set_czi(Some(CziConfig {
            enabled: false,
            library_dir: None,
        }));
        let mut b = MAGIC.to_vec();
        b.resize(64, 0);
        let r = open(Arc::new(Source::from_vec("a.czi", b)));
        set_czi(None);
        match r {
            Err(DecodeError::Unsupported(m)) => assert!(m.contains("switched off"), "{m}"),
            Err(e) => panic!("{e}"),
            Ok(_) => panic!("opened"),
        }
    }
}
