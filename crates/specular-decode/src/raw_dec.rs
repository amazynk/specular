//! Camera RAW through the in-tree `specular-raw` reader.
//!
//! Two modes, chosen at open:
//!
//! * **Sensor** (the sensor data is decodable: Bayer, X-Trans or linear): the image is the developed
//!   sensor crop. [`Session::decode_preview`] shows the largest embedded JPEG preview first (DCT
//!   scaled to about screen size); level 1/1 is the full demosaic, every coarser level is cut
//!   down from one half-size develop (2×2 binning, several times faster). Developed levels are
//!   cached and regions are cropped from them. While the level a region asks for (1/1, 1/2,
//!   or, for 1/4 and 1/8, the half level they are cut from) is not cached, the region develops
//!   alone, equal to the same pixels of the whole level:
//!   - a tiled sensor (lossless ARW, tiled DNG, striped uncompressed data) whose levels and
//!     white balance are known up front develops just the sensor tiles behind the region (kept
//!     in a bounded [`sr::TileCache`] for panning); these sessions report
//!     [`ImageInfo::fast_regions`], so focus measures and zoom tiles decode regions;
//!   - any other sensor (masked black levels, grey-world white balance, untiled
//!     CR2/NEF/ARW2, X-Trans RAF) decodes the whole sensor once at the first 1/1 region and
//!     holds it with its develop statistics (counted in [`Session::memory_bytes`]); later
//!     regions, and whole levels, develop from it. Sensors up to [`HELD_MAX`] are always
//!     held; larger ones when they fit the held budget ([`held_budget`]). All sessions'
//!     held sensors share that budget, least recently used dropped first, and
//!     [`Session::trim_memory`] drops a session's (the next region decodes it again). A
//!     whole-level decode (the overview) keeps its sensor data too while all held fit
//!     [`unasked_budget`], so the first zoom skips the decode. Regions from held data are
//!     assembled from developed [`DEV_TILE`]-px tiles kept in a bounded [`DevTiles`]
//!     (panning develops only the new tiles, in parallel; also dropped by a trim).
//!
//!   1/4 and 1/8 regions box-filter the half-size develop behind them. Output is linear
//!   Rec.709 encoded to 8-bit sRGB (DNG `BaselineExposure` applied); an HDR float path comes
//!   later.
//! * **Preview** (CR3, RAF, unsupported compressions or CFA layouts): the image *is* the largest
//!   embedded preview, decoded by the JPEG decoder (with its scales, row index and regions);
//!   a smaller preview, if any, is the quick first picture.
//!
//! Camera and exposure facts are exposed through [`Session::camera`].

use crate::raw_tiles::{DEV_TILE, DevTile, DevTiles, TileKey};
use crate::whole::{self, Levels};
use crate::{CameraInfo, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};
use parking_lot::Mutex;
use rayon::prelude::*;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_jpeg as sj;
use specular_raw as sr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, Weak};

pub struct RawDecoder;

pub(crate) fn map_err(e: sr::RawError) -> DecodeError {
    match e {
        sr::RawError::NotRaw => DecodeError::Malformed("not a camera RAW file".into()),
        sr::RawError::Malformed(s) => DecodeError::Malformed(s.into()),
        sr::RawError::Unsupported(s) => DecodeError::Unsupported(format!("camera RAW: {s}")),
        sr::RawError::NoPreview => DecodeError::Unsupported("no embedded preview".into()),
        sr::RawError::TooLarge(_) => DecodeError::TooLarge,
        sr::RawError::Cancelled => DecodeError::Cancelled,
        sr::RawError::Jpeg(e) => crate::jpeg::map_err(e),
        other => DecodeError::Malformed(other.to_string()),
    }
}

/// Whether `data` (the whole file) is a camera RAW this decoder takes. Generic TIFF-based
/// RAWs must carry a colour filter array, so 16-bit greyscale TIFFs (scans, microscopy) stay
/// with the TIFF decoder. Raw video clips (Cinema RAW Light, ProRes RAW) are
/// [`crate::rawclip_dec`]'s.
pub(crate) fn is_raw(data: &[u8]) -> bool {
    match sr::detect(data) {
        None | Some(sr::RawFormat::Crm | sr::RawFormat::ProResRaw) => false,
        Some(sr::RawFormat::TiffRaw) => {
            sr::parse(data).is_ok_and(|f| f.info().sensor.as_ref().is_some_and(|s| s.cfa.is_some()))
        }
        Some(_) => true,
    }
}

impl Decoder for RawDecoder {
    fn format(&self) -> Format {
        Format::Raw
    }
    /// Only the formats with their own signature (RAF, CR3, ORF, RW2, MRW, CRW, X3F, ARI,
    /// IIQ); TIFF-based RAWs need [`Decoder::probe_file`].
    fn probe(&self, head: &[u8]) -> bool {
        matches!(
            sr::detect(head),
            Some(
                sr::RawFormat::Raf
                    | sr::RawFormat::Cr3
                    | sr::RawFormat::Orf
                    | sr::RawFormat::Rw2
                    | sr::RawFormat::Mrw
                    | sr::RawFormat::Crw
                    | sr::RawFormat::X3f
                    | sr::RawFormat::Ari
                    | sr::RawFormat::Iiq
            )
        )
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        is_raw(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        // `.fff` is also FLIR's thermal format: the content decides (`probe_file`).
        &[
            "dng", "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "rwl",
            "pef", "srw", "crw", "mrw", "x3f", "3fr", "fff", "iiq", "mos", "mef", "dcr", "kdc",
            "erf", "ari", "gpr", "r3d", "ori", "sti", "k25", "qtk", "mdc", "j6i", "lfp", "lfr",
            "eip", "rwz",
        ]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let ext = source
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        if let Some(why) = sr::video_raw_refusal(data, ext) {
            return Err(DecodeError::Unsupported(why.into()));
        }
        if ext.eq_ignore_ascii_case("eip") {
            return open_eip(&source);
        }
        let file = sr::parse_named(data, ext).map_err(map_err)?;
        open_parsed(source, file)
    }
}

/// A session of a parsed RAW (a still, or one frame of a raw video clip).
pub(crate) fn open_parsed(source: Arc<Source>, file: sr::RawFile) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let ri = file.info();
    let camera = CameraInfo {
        camera: ri.camera_name(),
        lens: ri.lens.clone(),
        iso: ri.iso,
        exposure_time: ri.exposure_time,
        f_number: ri.f_number,
        focal_length: ri.focal_length,
        exposure_bias: ri.exposure_bias,
        date_time: ri.date_time.clone(),
        software: ri.software.clone(),
    };
    let kind = ri.format.map_or("camera RAW", |f| f.name());
    let mut coding = kind.to_string();
    if !camera.camera.is_empty() {
        coding += &format!(", {}", camera.camera);
    }
    // Bayer and X-Trans develop here; LinearRaw, and the patterns and layouts the decode
    // turns into camera RGB itself (four-colour, staggered), arrive already demosaiced.
    let developable = file.can_decode_sensor()
        && ri.sensor.as_ref().is_some_and(|s| {
            s.samples_per_pixel == 3
                || s.demosaiced
                || s.cfa.as_ref().is_some_and(|c| c.developable())
        });
    let raw_orientation = match ri.orientation {
        1..=8 => ri.orientation,
        _ => 1,
    };
    // A rotation the user saved for this RAW lives in its XMP sidecar (the RAW itself is
    // never written); it replaces the camera's orientation.
    let saved = specular_fileops::sidecar_orientation(source.path()).map(u16::from);
    if developable && let Some(s) = ri.sensor.as_ref() {
        let sensor_bytes =
            u64::from(s.width) * u64::from(s.height) * u64::from(s.samples_per_pixel) * 2;
        // Non-square pixels (DNG DefaultScale): shown stretched to square pixels, whole
        // levels only (regions would be cut from the stored grid).
        let stretched = s.square_size != (s.crop.w, s.crop.h);
        let regions = file.region_decodable() && !stretched;
        coding += &format!(
            ", {}-bit {}{}",
            s.bits,
            compression_name(s.compression),
            s.cfa
                .as_ref()
                .map_or(String::new(), |c| format!(" {}", c.name()))
        );
        if let Some(p) = ri.largest_preview() {
            coding += &format!(", preview {}×{}", p.width, p.height);
        }
        if !s.opcodes.is_empty() {
            coding += &format!(", DNG opcodes {}", s.opcodes.join(", "));
        }
        if !s.opcodes_skipped.is_empty() {
            coding += &format!(
                ", DNG opcodes not applied: {}",
                s.opcodes_skipped.join(", ")
            );
        }
        if stretched {
            coding += &format!(
                ", non-square pixels shown {}×{}",
                s.square_size.0, s.square_size.1
            );
        }
        let info = ImageInfo {
            format: Format::Raw,
            width: s.square_size.0,
            height: s.square_size.1,
            orientation: Orientation::from_exif(saved.unwrap_or(raw_orientation) as u8),
            icc_profile: None,
            has_alpha: false,
            bit_depth: s.bits.min(255) as u8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: regions,
            scales: &[1, 2, 4, 8],
        };
        // The first picture: the largest preview shown the way the raw data is.
        let preview = ri
            .largest_preview()
            .filter(|p| p.orientation == raw_orientation || p.orientation == 0)
            .cloned();
        let level_bytes = u64::from(s.square_size.0) * u64::from(s.square_size.1) * 3;
        let gain = 2f32.powf(ri.baseline_exposure.unwrap_or(0.0).clamp(-4.0, 4.0) as f32);
        return Ok(Arc::new(RawSession {
            source,
            info,
            camera,
            file,
            mode: Mode::Sensor {
                preview,
                levels: Levels::new(),
                gain,
                lut: OnceLock::new(),
                regions,
                tiles: sr::TileCache::new((sensor_bytes / 2).min(TILE_CACHE_MAX)),
                sensor_bytes,
                held: Arc::new(Mutex::new(None)),
                hold_work: Mutex::new(()),
                developed: DevTiles::new(level_bytes.min(DEV_TILES_MAX)),
            },
        }));
    }
    // Preview mode.
    let reason = ri
        .sensor
        .as_ref()
        .and_then(|s| s.unsupported_reason)
        .unwrap_or("sensor data not supported");
    let Some(p) = ri.largest_preview().cloned() else {
        return Err(DecodeError::Unsupported(format!(
            "{kind}: {reason}, and no embedded preview"
        )));
    };
    let orientation = match p.orientation {
        1..=8 => p.orientation,
        _ => raw_orientation,
    };
    coding += &format!(", embedded preview only ({reason})");
    let small = ri
        .previews
        .iter()
        .rev()
        .find(|q| q.range != p.range && q.width.max(q.height) >= 160)
        .cloned();
    let (inner, icc, bit_depth) = match p.kind {
        sr::PreviewKind::Jpeg => {
            let bytes = data
                .get(p.range.clone())
                .ok_or_else(|| DecodeError::Malformed("preview out of bounds".into()))?;
            let sub = Arc::new(Source::from_vec(
                source.path().to_path_buf(),
                bytes.to_vec(),
            ));
            let s = crate::jpeg::JpegDecoder.open(sub)?;
            let icc = s.info().icc_profile.clone();
            let depth = s.info().bit_depth;
            (Some(s), icc, depth)
        }
        sr::PreviewKind::Rgb8 | sr::PreviewKind::X3fCoded | sr::PreviewKind::Grey4 => {
            (None, None, 8)
        }
    };
    let (width, height) = match &inner {
        Some(s) => (s.info().width, s.info().height),
        None => (p.width, p.height),
    };
    let info = ImageInfo {
        format: Format::Raw,
        width,
        height,
        orientation: Orientation::from_exif(saved.unwrap_or(orientation) as u8),
        icc_profile: icc,
        has_alpha: false,
        bit_depth,
        progressive: inner.as_ref().is_some_and(|s| s.info().progressive),
        coding,
        embedded_preview: None,
        fast_regions: inner.as_ref().is_some_and(|s| s.info().fast_regions),
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(RawSession {
        source,
        info,
        camera,
        file,
        mode: Mode::Preview {
            preview: p,
            small,
            inner,
            levels: Levels::new(),
        },
    }))
}

/// Phase One EIP ("Enhanced Image Package"): a ZIP holding the IIQ raw and Capture One's
/// settings. The IIQ inside is opened (read out of the archive when it is compressed).
fn open_eip(source: &Arc<Source>) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let not_eip = || DecodeError::Malformed("not a Phase One EIP package (no IIQ inside)".into());
    let zip = specular_ora::zip::Zip::parse(data).map_err(|_| not_eip())?;
    let entry = zip
        .entries
        .iter()
        .find(|e| {
            let name = e
                .name
                .rsplit('/')
                .next()
                .unwrap_or(&e.name)
                .to_ascii_lowercase();
            name.ends_with(".iiq") && !name.starts_with('.')
        })
        .ok_or_else(not_eip)?;
    let iiq = zip
        .read(data, entry, data.len() as u64 * 4 + (1 << 30))
        .map_err(|_| DecodeError::Malformed("the IIQ inside the EIP package is damaged".into()))?;
    let sub = Arc::new(Source::from_vec(
        source.path().with_extension("iiq"),
        iiq.into_owned(),
    ));
    RawDecoder.open(sub)
}

/// Upper bound of the decoded sensor tiles a session keeps for regions (also at most half the
/// sensor data).
const TILE_CACHE_MAX: u64 = 128 << 20;

/// Upper bound of the developed tiles a session keeps for regions ([`DevTiles`]; also at most
/// one 8-bit full-size level).
const DEV_TILES_MAX: u64 = 64 << 20;

/// Sensor data (bytes) a session always may hold to develop regions from (128 MP of single
/// samples), and the least [`held_budget`].
const HELD_MAX: u64 = 256 << 20;

/// Most sensor data all sessions hold together: [`HELD_MAX`], or a quarter of the memory
/// budget's free bytes now plus `held` (what they hold already), when that is more. A larger sensor
/// develops whole levels for its regions instead (which needs more memory than holding it:
/// the sensor data, a 16-bit and an 8-bit RGB level).
fn held_budget(held: u64) -> u64 {
    specular_core::budget::free().map_or(HELD_MAX, |a| HELD_MAX.max(a.saturating_add(held) / 4))
}

/// The whole decoded sensor data and what developing it needs from all of it.
struct Held {
    raw: sr::RawImage,
    stats: sr::DevelopStats,
    /// A region developed from it (else it was kept from a whole-level decode, unasked, and
    /// may make room for another one kept that way, see [`hold_unasked`]).
    zoomed: AtomicBool,
}

impl Held {
    fn bytes(&self) -> u64 {
        self.raw.data.len() as u64 * 2
    }
}

/// A session's held sensor data, if any.
type HeldSlot = Mutex<Option<Arc<Held>>>;

/// The slots of sessions holding sensor data, least recently used first.
static HOLDERS: Mutex<Vec<Weak<HeldSlot>>> = Mutex::new(Vec::new());

/// Bytes held by `slot`.
fn slot_bytes(slot: &HeldSlot) -> u64 {
    slot.lock().as_ref().map_or(0, |h| h.bytes())
}

/// Sensor data held by all sessions.
fn held_total() -> u64 {
    HOLDERS
        .lock()
        .iter()
        .filter_map(Weak::upgrade)
        .map(|s| slot_bytes(&s))
        .sum()
}

/// Before `me` holds `bytes`: drops other sessions' held sensors that are `droppable`, least
/// recently used first, until all fit `budget(held by the others)`, and forgets `me` (and
/// dropped or empty slots). Whether `bytes` more fit then; `strict`: drops nothing when they
/// would not. `list` is [`HOLDERS`], locked.
fn make_room_in(
    list: &mut Vec<Weak<HeldSlot>>,
    me: &Arc<HeldSlot>,
    bytes: u64,
    budget: impl FnOnce(u64) -> u64,
    droppable: impl Fn(&Held) -> bool,
    strict: bool,
) -> bool {
    list.retain(|w| {
        w.upgrade()
            .is_some_and(|s| !Arc::ptr_eq(&s, me) && s.lock().is_some())
    });
    let mut total: u64 = list
        .iter()
        .filter_map(Weak::upgrade)
        .map(|s| slot_bytes(&s))
        .sum();
    let budget = budget(total);
    if strict {
        let can_drop: u64 = list
            .iter()
            .filter_map(Weak::upgrade)
            .filter_map(|s| {
                s.lock()
                    .as_ref()
                    .filter(|h| droppable(h))
                    .map(|h| h.bytes())
            })
            .sum();
        if (total - can_drop.min(total)).saturating_add(bytes) > budget {
            return false;
        }
    }
    let mut i = 0;
    while total.saturating_add(bytes) > budget && i < list.len() {
        let Some(s) = list[i].upgrade() else {
            list.remove(i);
            continue;
        };
        let mut slot = s.lock();
        if slot.as_ref().is_some_and(|h| droppable(h))
            && let Some(h) = slot.take()
        {
            drop(slot);
            list.remove(i);
            total = total.saturating_sub(h.bytes());
            specular_log::debug!(
                target: specular_log::DECODE,
                "raw: held sensor data of another image ({} MiB) dropped to hold {} MiB",
                h.bytes() >> 20,
                bytes >> 20
            );
        } else {
            i += 1;
        }
    }
    total.saturating_add(bytes) <= budget
}

/// [`make_room_in`] for the sensor data a region needs: any other may be dropped.
fn make_room(me: &Arc<HeldSlot>, bytes: u64, budget: impl FnOnce(u64) -> u64) {
    make_room_in(&mut HOLDERS.lock(), me, bytes, budget, |_| true, false);
}

/// Most sensor data all sessions hold when one more is held unasked ([`hold_unasked`]):
/// [`HELD_MAX`], or a quarter of the memory budget's free bytes now plus `held` (what they hold
/// already) when that is less, so nothing is held unasked when memory is tight.
fn unasked_budget(held: u64) -> u64 {
    specular_core::budget::free().map_or(HELD_MAX, |a| HELD_MAX.min(a.saturating_add(held) / 4))
}

/// Keeps `h`, the sensor data a whole-level decode of `me`'s image has just developed, for
/// its regions (the first 1:1 zoom then skips the sensor decode), when it fits
/// `budget` ([`unasked_budget`]) after dropping other sensors held unasked that no region used
/// yet, least recently used first (never one a region used). Whether it is held.
fn hold_unasked(me: &Arc<HeldSlot>, h: Held, budget: impl FnOnce(u64) -> u64) -> bool {
    let bytes = h.bytes();
    let mut list = HOLDERS.lock();
    if me.lock().is_some()
        || !make_room_in(
            &mut list,
            me,
            bytes,
            budget,
            |o| !o.zoomed.load(Ordering::Relaxed),
            true,
        )
    {
        return false;
    }
    *me.lock() = Some(Arc::new(h));
    list.push(Arc::downgrade(me));
    true
}

/// `me` as the most recently used holder.
fn touch(me: &Arc<HeldSlot>) {
    let mut list = HOLDERS.lock();
    list.retain(|w| w.strong_count() > 0 && !std::ptr::eq(w.as_ptr(), Arc::as_ptr(me)));
    list.push(Arc::downgrade(me));
}

/// Where a region request develops from (see [`RawSession::route`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    /// The sensor tiles behind the region ([`sr::RawFile::region_decodable`]).
    Tiles,
    /// The held whole sensor data.
    Held,
    /// The whole sensor data, decoded now and held.
    Hold,
    /// The developed tiles behind the region, all kept ([`DevTiles`]).
    Developed,
}

/// What a region develops from.
enum Src<'a> {
    Tiles(&'a sr::TileCache),
    Held(&'a Held),
}

/// `r` clipped to a `w`×`h` level.
fn clip(r: crate::Rect, w: u32, h: u32) -> crate::Rect {
    let (x, y) = (r.x.min(w), r.y.min(h));
    crate::Rect {
        x,
        y,
        w: r.x.saturating_add(r.w).min(w) - x,
        h: r.y.saturating_add(r.h).min(h) - y,
    }
}

fn compression_name(c: sr::SensorCompression) -> String {
    match c {
        sr::SensorCompression::Uncompressed => "uncompressed".into(),
        sr::SensorCompression::LosslessJpeg => "lossless JPEG".into(),
        sr::SensorCompression::NikonHuffman => "Nikon compressed".into(),
        sr::SensorCompression::SonyArw2 => "Sony ARW2".into(),
        sr::SensorCompression::Panasonic => "Panasonic compressed".into(),
        sr::SensorCompression::PentaxHuffman => "Pentax compressed".into(),
        sr::SensorCompression::CanonCrx => "Canon CRX".into(),
        sr::SensorCompression::ProResRaw => "ProRes RAW".into(),
        sr::SensorCompression::Fujifilm => "Fujifilm compressed".into(),
        sr::SensorCompression::Olympus => "Olympus compressed".into(),
        sr::SensorCompression::LossyJpeg => "lossy JPEG".into(),
        sr::SensorCompression::NikonHighEfficiency { star: false } => {
            "Nikon High Efficiency (HE, TicoRAW)".into()
        }
        sr::SensorCompression::NikonHighEfficiency { star: true } => {
            "Nikon High Efficiency* (HE*, TicoRAW)".into()
        }
        sr::SensorCompression::Other(n) => format!("compression {n}"),
    }
}

enum Mode {
    Sensor {
        /// Shown first (its orientation matches the raw data's).
        preview: Option<sr::Preview>,
        /// Developed whole levels: 1/1 (full demosaic) and 1/2 (binned) are bases.
        levels: Levels,
        /// Exposure gain applied before sRGB encoding (DNG `BaselineExposure`).
        gain: f32,
        /// Linear 16-bit → sRGB 8-bit, built on first use.
        lut: OnceLock<Vec<u8>>,
        /// Regions develop from the sensor tiles behind them
        /// ([`sr::RawFile::region_decodable`]).
        regions: bool,
        /// Decoded sensor tiles of the regions.
        tiles: sr::TileCache,
        /// Bytes of the decoded sensor data.
        sensor_bytes: u64,
        /// Otherwise the first 1/1 region decodes the whole sensor data into `held` (when it
        /// fits, see [`held_budget`]), and regions develop from it (whole levels too). Listed in
        /// [`HOLDERS`] while it holds data.
        held: Arc<HeldSlot>,
        /// Taken while the sensor data is decoded for `held`.
        hold_work: Mutex<()>,
        /// Regions developed from `held` are assembled from developed tiles kept here.
        developed: DevTiles,
    },
    Preview {
        preview: sr::Preview,
        /// A smaller preview for the quick first picture.
        small: Option<sr::Preview>,
        /// JPEG session on the preview's bytes (`None` for an uncompressed RGB preview).
        inner: Option<Arc<dyn Session>>,
        /// Levels of an RGB preview.
        levels: Levels,
    },
}

struct RawSession {
    source: Arc<Source>,
    info: ImageInfo,
    camera: CameraInfo,
    file: sr::RawFile,
    mode: Mode,
}

/// sRGB transfer function of linear 0..=65535 (times `gain`) to 8 bits.
fn srgb_lut(gain: f32) -> Vec<u8> {
    (0..65536u32)
        .map(|v| {
            let l = (v as f32 / 65535.0 * gain).clamp(0.0, 1.0);
            let e = if l <= 0.003_130_8 {
                12.92 * l
            } else {
                1.055 * l.powf(1.0 / 2.4) - 0.055
            };
            (e * 255.0 + 0.5) as u8
        })
        .collect()
}

/// A developed image as planar 8-bit sRGB.
fn encode(img: &sr::RgbImage, lut: &[u8], scale: u32) -> Raster {
    let n = img.width as usize * img.height as usize;
    let mut planes: [Vec<u8>; 3] = std::array::from_fn(|_| Vec::with_capacity(n));
    for px in img.data[..n * 3].chunks_exact(3) {
        for (p, &v) in planes.iter_mut().zip(px) {
            p.push(lut[v as usize]);
        }
    }
    Raster {
        width: img.width,
        height: img.height,
        origin: (0, 0),
        scale,
        layout: PixelLayout::Rgb,
        planes: planes
            .into_iter()
            .map(|data| RasterPlane {
                width: img.width,
                height: img.height,
                stride: img.width as usize,
                data,
                h_div: 1,
                v_div: 1,
            })
            .collect(),
        truncated: img.truncated,
    }
}

/// An RGBA preview image as a raster at `scale` (0 = a thumbnail).
fn rgba_raster(p: sr::PreviewImage, scale: u32) -> Raster {
    let mut r = whole::interleaved(p.width, p.height, PixelLayout::Rgba, p.rgba, false);
    r.scale = scale;
    r
}

impl RawSession {
    /// A preview decoded to about screen size, as a thumbnail raster.
    fn decode_small(&self, p: &sr::Preview) -> Result<Raster> {
        self.decode_small_at(p, 1920)
    }

    /// A preview decoded with its long side at least `long_side` (DCT scaling of a JPEG
    /// preview; other kinds whole), as a thumbnail raster.
    fn decode_small_at(&self, p: &sr::Preview, long_side: u32) -> Result<Raster> {
        let data = self.source.bytes();
        match p.kind {
            sr::PreviewKind::Jpeg => {
                let bytes = data
                    .get(p.range.clone())
                    .ok_or_else(|| DecodeError::Malformed("preview out of bounds".into()))?;
                let ji = sj::read_info(bytes).map_err(crate::jpeg::map_err)?;
                let opts = sj::DecodeOptions {
                    scale: sj::choose_scale(&ji, long_side, long_side),
                    ..Default::default()
                };
                let d = sj::decode(bytes, &opts).map_err(crate::jpeg::map_err)?;
                Ok(crate::jpeg::to_raster(d, 0))
            }
            sr::PreviewKind::Rgb8 | sr::PreviewKind::X3fCoded | sr::PreviewKind::Grey4 => {
                sr::decode_preview(data, p, 0)
                    .map(|i| rgba_raster(i, 0))
                    .map_err(map_err)
            }
        }
    }

    /// The held sensor data, if any.
    fn held(&self) -> Option<Arc<Held>> {
        match &self.mode {
            Mode::Sensor { held, .. } => held.lock().clone(),
            Mode::Preview { .. } => None,
        }
    }

    /// The whole sensor data, decoded once (concurrent callers wait for that decode) and held.
    fn hold_sensor(
        &self,
        held: &Arc<HeldSlot>,
        work: &Mutex<()>,
        bytes: u64,
        cancel: &CancelToken,
    ) -> Result<Arc<Held>> {
        let kept = || {
            let h = held.lock().clone()?;
            h.zoomed.store(true, Ordering::Relaxed);
            touch(held);
            Some(h)
        };
        if let Some(h) = kept() {
            return Ok(h);
        }
        let _work = work.lock();
        if let Some(h) = kept() {
            return Ok(h);
        }
        make_room(held, bytes, held_budget);
        let _res = specular_core::budget::reserve(bytes)?;
        let opts = sr::DecodeOptions {
            parallel: true,
            cancel: Some(cancel.flag()),
            ..Default::default()
        };
        let t0 = std::time::Instant::now();
        let raw = self
            .file
            .decode_raw(self.source.bytes(), &opts)
            .map_err(map_err)?;
        let stats = sr::DevelopStats::of(&raw).map_err(map_err)?;
        specular_log::debug!(
            target: specular_log::DECODE,
            "raw: sensor {}×{} held for regions ({} MiB), {:.1} ms",
            raw.width,
            raw.height,
            raw.data.len() >> 19,
            t0.elapsed().as_secs_f64() * 1e3
        );
        let h = Arc::new(Held {
            raw,
            stats,
            zoomed: AtomicBool::new(true),
        });
        *held.lock() = Some(h.clone());
        touch(held);
        Ok(h)
    }

    /// The displayed size when the sensor has non-square pixels (DNG `DefaultScale`).
    fn stretched(&self) -> Option<(u32, u32)> {
        let s = self.file.info().sensor.as_ref()?;
        (s.square_size != (s.crop.w, s.crop.h)).then_some(s.square_size)
    }

    /// Where region request `req` develops from; `None`: cropped from a whole level.
    fn route(&self, req: &DecodeRequest) -> Option<Route> {
        if self.stretched().is_some() {
            return None;
        }
        let Mode::Sensor {
            levels,
            regions,
            sensor_bytes,
            held,
            developed,
            ..
        } = &self.mode
        else {
            return None;
        };
        let r = req.region?;
        let s = req.scale.max(1);
        // A cached level (or, below 1/2, the half level it is cut from) is cheaper.
        if !matches!(s, 1 | 2 | 4 | 8) || levels.has(s) || (s > 2 && levels.has(2)) {
            return None;
        }
        if *regions {
            Some(Route::Tiles)
        } else if held.lock().is_some() {
            Some(Route::Held)
        } else if let Some(keys) = self.dev_keys(r, s)
            && developed.has_all(keys)
        {
            Some(Route::Developed)
        } else if s == 1
            && (*sensor_bytes <= HELD_MAX || *sensor_bytes <= held_budget(held_total()))
        {
            Some(Route::Hold)
        } else {
            None
        }
    }

    /// For region `r` of level 1/`scale` (1, 2, 4 or 8): `r` clipped to the level, and the
    /// region of the develop at `size` behind it (the same unless `scale` > 2, whose pixels are
    /// cut down from the half-size develop's, as [`Levels`] cuts the whole level 1/2).
    fn region_plan(
        &self,
        r: crate::Rect,
        scale: u32,
    ) -> (crate::Rect, crate::Rect, sr::OutputSize) {
        let (w, h) = (self.info.width, self.info.height);
        let half = ((w / 2).max(1), (h / 2).max(1));
        match scale {
            1 => {
                let r = clip(r, w, h);
                (r, r, sr::OutputSize::Full)
            }
            2 => {
                let r = clip(r, half.0, half.1);
                (r, r, sr::OutputSize::Half)
            }
            s => {
                let r = clip(r, w.div_ceil(s), h.div_ceil(s));
                let (x0, x1) = whole::source_span(r.x, r.w, s / 2, half.0);
                let (y0, y1) = whole::source_span(r.y, r.h, s / 2, half.1);
                let dev = crate::Rect {
                    x: x0,
                    y: y0,
                    w: x1 - x0,
                    h: y1 - y0,
                };
                (r, dev, sr::OutputSize::Half)
            }
        }
    }

    /// Size of the develop at `size` (level 1/1 or 1/2).
    fn dev_level(&self, size: sr::OutputSize) -> (u32, u32) {
        let (w, h) = (self.info.width, self.info.height);
        match size {
            sr::OutputSize::Full => (w, h),
            sr::OutputSize::Half => ((w / 2).max(1), (h / 2).max(1)),
        }
    }

    /// The developed tiles behind region `r` of level 1/`scale` (see [`Self::region_plan`]);
    /// `None` when the region is empty or its tiles would fill more than half the session's
    /// [`DevTiles`] (it then develops alone, nothing kept).
    fn dev_keys(&self, r: crate::Rect, scale: u32) -> Option<Vec<TileKey>> {
        let Mode::Sensor { developed, .. } = &self.mode else {
            return None;
        };
        let (out, dev, size) = self.region_plan(r, scale);
        if out.w == 0 || out.h == 0 || dev.w == 0 || dev.h == 0 {
            return None;
        }
        let half = size == sr::OutputSize::Half;
        let (tx0, tx1) = (dev.x / DEV_TILE, (dev.x + dev.w - 1) / DEV_TILE);
        let (ty0, ty1) = (dev.y / DEV_TILE, (dev.y + dev.h - 1) / DEV_TILE);
        let n = u64::from(tx1 - tx0 + 1) * u64::from(ty1 - ty0 + 1);
        if n * u64::from(DEV_TILE * DEV_TILE) * 3 > developed.max() / 2 {
            return None;
        }
        Some(
            (ty0..=ty1)
                .flat_map(|ty| (tx0..=tx1).map(move |tx| (half, tx, ty)))
                .collect(),
        )
    }

    /// Region `r` of level 1/`scale` (1, 2, 4 or 8) assembled from the developed tiles behind
    /// it ([`Self::dev_keys`]), equal to the same pixels of the whole level. Tiles not kept yet
    /// develop from `held()` (in parallel with `parallel`) and are kept; a region with too many
    /// tiles develops alone from it ([`Self::develop_region`]).
    #[allow(clippy::too_many_arguments)]
    fn region_from_developed(
        &self,
        r: crate::Rect,
        scale: u32,
        developed: &DevTiles,
        held: &dyn Fn() -> Result<Arc<Held>>,
        gain: f32,
        lut: &OnceLock<Vec<u8>>,
        parallel: bool,
        cancel: &CancelToken,
    ) -> Result<Raster> {
        let Some(keys) = self.dev_keys(r, scale) else {
            let h = held()?;
            return self.develop_region(r, scale, Src::Held(&h), gain, lut, parallel, cancel);
        };
        let (out, dev, size) = self.region_plan(r, scale);
        let (lw, lh) = self.dev_level(size);
        let t0 = std::time::Instant::now();
        let mut tiles: Vec<Option<Arc<DevTile>>> = keys.iter().map(|&k| developed.get(k)).collect();
        let missing: Vec<usize> = (0..keys.len()).filter(|&i| tiles[i].is_none()).collect();
        if !missing.is_empty() {
            let h = held()?;
            let lut = lut.get_or_init(|| srgb_lut(gain));
            let one = |&i: &usize| -> Result<Arc<DevTile>> {
                let (_, tx, ty) = keys[i];
                let t = clip(
                    crate::Rect {
                        x: tx * DEV_TILE,
                        y: ty * DEV_TILE,
                        w: DEV_TILE,
                        h: DEV_TILE,
                    },
                    lw,
                    lh,
                );
                let opts = sr::DecodeOptions {
                    size,
                    region: Some(sr::Rect {
                        x: t.x,
                        y: t.y,
                        w: t.w,
                        h: t.h,
                    }),
                    parallel,
                    cancel: Some(cancel.flag()),
                    ..Default::default()
                };
                let img = sr::develop_with(&h.raw, &h.stats, &opts).map_err(map_err)?;
                let e = encode(&img, lut, 1);
                let mut planes = e.planes.into_iter().map(|p| p.data);
                Ok(Arc::new(DevTile {
                    w: e.width,
                    h: e.height,
                    planes: std::array::from_fn(|_| planes.next().unwrap_or_default()),
                    truncated: e.truncated,
                }))
            };
            let made: Vec<Arc<DevTile>> = if parallel && missing.len() > 1 {
                missing.par_iter().map(one).collect::<Result<_>>()?
            } else {
                missing.iter().map(one).collect::<Result<_>>()?
            };
            for (&i, t) in missing.iter().zip(made) {
                developed.insert(keys[i], t.clone());
                tiles[i] = Some(t);
            }
        }
        // The develop region `dev`, copied from the tiles.
        let (dw, dh) = (dev.w as usize, dev.h as usize);
        let mut planes: [Vec<u8>; 3] = std::array::from_fn(|_| vec![0u8; dw * dh]);
        let mut truncated = false;
        for (&(_, tx, ty), t) in keys.iter().zip(&tiles) {
            let Some(t) = t else { continue };
            truncated |= t.truncated;
            let (ox, oy) = (tx * DEV_TILE, ty * DEV_TILE);
            let (x0, x1) = (dev.x.max(ox), (dev.x + dev.w).min(ox + t.w));
            let (y0, y1) = (dev.y.max(oy), (dev.y + dev.h).min(oy + t.h));
            if x0 >= x1 {
                continue;
            }
            let n = (x1 - x0) as usize;
            for y in y0..y1 {
                let s = (y - oy) as usize * t.w as usize + (x0 - ox) as usize;
                let d = (y - dev.y) as usize * dw + (x0 - dev.x) as usize;
                for (dp, sp) in planes.iter_mut().zip(&t.planes) {
                    dp[d..d + n].copy_from_slice(&sp[s..s + n]);
                }
            }
        }
        let mut part = Raster {
            width: dev.w,
            height: dev.h,
            origin: (dev.x, dev.y),
            scale: scale.min(2),
            layout: PixelLayout::Rgb,
            planes: planes
                .into_iter()
                .map(|data| RasterPlane {
                    width: dev.w,
                    height: dev.h,
                    stride: dw,
                    data,
                    h_div: 1,
                    v_div: 1,
                })
                .collect(),
            truncated,
        };
        if scale > 2 {
            part = whole::downscale_part(&part, (lw, lh), scale, out);
        }
        specular_log::debug!(
            target: specular_log::DECODE,
            "raw: 1/{scale} region {}×{} at {},{} from developed tiles ({} kept, {} developed, {} held): {:.1} ms",
            part.width,
            part.height,
            out.x,
            out.y,
            keys.len() - missing.len(),
            missing.len(),
            developed.len(),
            t0.elapsed().as_secs_f64() * 1e3
        );
        Ok(part)
    }

    fn develop(
        &self,
        base: u32,
        gain: f32,
        lut: &OnceLock<Vec<u8>>,
        cancel: &CancelToken,
    ) -> Result<Raster> {
        let opts = sr::DecodeOptions {
            size: if base == 1 {
                sr::OutputSize::Full
            } else {
                sr::OutputSize::Half
            },
            parallel: true,
            cancel: Some(cancel.flag()),
            ..Default::default()
        };
        let data = self.source.bytes();
        let t0 = std::time::Instant::now();
        let held = self.held();
        let mut kept = false;
        let img = match &held {
            // The held sensor data develops the same as a fresh decode.
            Some(h) => sr::develop_with(&h.raw, &h.stats, &opts).map_err(map_err)?,
            None => {
                // The sensor data and the 16-bit RGB level developed from it.
                let sensor = match &self.mode {
                    Mode::Sensor { sensor_bytes, .. } => *sensor_bytes,
                    _ => 0,
                };
                let level = u64::from(self.info.width) * u64::from(self.info.height) * 6
                    / u64::from(base * base);
                let _res = specular_core::budget::reserve(sensor.saturating_add(level))?;
                let raw = self.file.decode_raw(data, &opts).map_err(map_err)?;
                let stats = sr::DevelopStats::of(&raw).map_err(map_err)?;
                let img = sr::develop_with(&raw, &stats, &opts).map_err(map_err)?;
                // Kept for the regions of a later zoom while memory allows (tiled sensors
                // decode just the tiles behind a region instead).
                if let Mode::Sensor {
                    held: slot,
                    regions: false,
                    ..
                } = &self.mode
                {
                    kept = hold_unasked(
                        slot,
                        Held {
                            raw,
                            stats,
                            zoomed: AtomicBool::new(false),
                        },
                        unasked_budget,
                    );
                }
                img
            }
        };
        // Non-square pixels: the level stretched to square ones, at its exact size.
        let img = match self.stretched() {
            Some((w, h)) => sr::resample_rgb(&img, w.div_ceil(base), h.div_ceil(base)),
            None => img,
        };
        let r = encode(&img, lut.get_or_init(|| srgb_lut(gain)), base);
        specular_log::debug!(
            target: specular_log::DECODE,
            "raw: 1/{base} {}×{}{}: {:.1} ms",
            r.width,
            r.height,
            if held.is_some() {
                " from held sensor data"
            } else if kept {
                ", sensor data held for regions"
            } else {
                ""
            },
            t0.elapsed().as_secs_f64() * 1e3
        );
        Ok(r)
    }

    /// Region `r` of level 1/`scale` (1, 2, 4 or 8) developed from `src` alone, equal to the
    /// same pixels of the whole level: 1/1 is a full-size develop of the region, 1/2 a
    /// half-size one, and coarser levels cut down the half-size pixels behind the region.
    #[allow(clippy::too_many_arguments)]
    fn develop_region(
        &self,
        r: crate::Rect,
        scale: u32,
        src: Src,
        gain: f32,
        lut: &OnceLock<Vec<u8>>,
        parallel: bool,
        cancel: &CancelToken,
    ) -> Result<Raster> {
        let (out, dev, size) = self.region_plan(r, scale);
        if out.w == 0 || out.h == 0 {
            return Ok(Raster {
                width: 0,
                height: 0,
                origin: (out.x, out.y),
                scale,
                layout: PixelLayout::Rgb,
                planes: (0..3)
                    .map(|_| RasterPlane {
                        width: 0,
                        height: 0,
                        stride: 0,
                        data: Vec::new(),
                        h_div: 1,
                        v_div: 1,
                    })
                    .collect(),
                truncated: false,
            });
        }
        let opts = sr::DecodeOptions {
            size,
            region: Some(sr::Rect {
                x: dev.x,
                y: dev.y,
                w: dev.w,
                h: dev.h,
            }),
            tiles: match src {
                Src::Tiles(t) => Some(t),
                Src::Held(_) => None,
            },
            parallel,
            cancel: Some(cancel.flag()),
            ..Default::default()
        };
        let t0 = std::time::Instant::now();
        let (img, window) = match src {
            Src::Tiles(_) => {
                let raw = self
                    .file
                    .decode_raw(self.source.bytes(), &opts)
                    .map_err(map_err)?;
                (sr::develop(&raw, &opts).map_err(map_err)?, raw.window)
            }
            Src::Held(h) => (
                sr::develop_with(&h.raw, &h.stats, &opts).map_err(map_err)?,
                h.raw.window,
            ),
        };
        let mut part = encode(&img, lut.get_or_init(|| srgb_lut(gain)), scale.min(2));
        part.origin = (dev.x, dev.y);
        if scale > 2 {
            let (w, h) = (self.info.width, self.info.height);
            part = whole::downscale_part(&part, ((w / 2).max(1), (h / 2).max(1)), scale, out);
        }
        specular_log::debug!(
            target: specular_log::DECODE,
            "raw: 1/{scale} region {}×{} at {},{} from {} (sensor window {}×{}): {:.1} ms",
            part.width,
            part.height,
            out.x,
            out.y,
            match src {
                Src::Tiles(t) => format!("tiles, {} held", t.len()),
                Src::Held(_) => "held sensor data".into(),
            },
            window.w,
            window.h,
            t0.elapsed().as_secs_f64() * 1e3
        );
        Ok(part)
    }
}

impl Session for RawSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// The sensor data's layout from the header, also when an embedded preview is shown
    /// (the file's samples are still the sensor's; the bit depth is then the preview's, so
    /// the info panel leaves it out).
    fn colour_model(&self) -> Option<&'static str> {
        let s = self.file.info().sensor.as_ref()?;
        Some(match &s.cfa {
            Some(c) if c.bayer2x2().is_some() => "Bayer",
            Some(c) if c.width == 6 && c.height == 6 => "X-Trans",
            Some(_) => "CFA",
            None if s.samples_per_pixel >= 3 => "LinearRaw",
            None => return None,
        })
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }

    fn decode_preview(&self) -> Option<Result<Raster>> {
        let p = match &self.mode {
            Mode::Sensor { preview, .. } => preview.as_ref()?,
            Mode::Preview { small, .. } => small.as_ref()?,
        };
        Some(self.decode_small(p))
    }

    fn decode_preview_for(&self, long_side: u32) -> Option<Result<Raster>> {
        let p = match &self.mode {
            Mode::Sensor { preview, .. } => preview.as_ref()?,
            Mode::Preview { small, .. } => small.as_ref()?,
        };
        Some(self.decode_small_at(p, long_side.clamp(1, 1920)))
    }

    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let size = (self.info.width, self.info.height);
        let route = self.route(req);
        match &self.mode {
            Mode::Sensor {
                gain,
                lut,
                tiles,
                sensor_bytes,
                held,
                hold_work,
                developed,
                ..
            } if let Some(route) = route
                && let Some(r) = req.region =>
            {
                let scale = req.scale.max(1);
                match route {
                    Route::Tiles => self.develop_region(
                        r,
                        scale,
                        Src::Tiles(tiles),
                        *gain,
                        lut,
                        req.parallel,
                        cancel,
                    ),
                    Route::Held | Route::Hold | Route::Developed => self.region_from_developed(
                        r,
                        scale,
                        developed,
                        &|| self.hold_sensor(held, hold_work, *sensor_bytes, cancel),
                        *gain,
                        lut,
                        req.parallel,
                        cancel,
                    ),
                }
            }
            Mode::Sensor {
                levels, gain, lut, ..
            } => levels.decode(
                req,
                size,
                |s| if s <= 1 { 1 } else { 2 },
                |b| self.develop(b, *gain, lut, cancel),
                cancel,
            ),
            Mode::Preview { inner: Some(s), .. } => s.decode(req, cancel),
            Mode::Preview {
                preview,
                inner: None,
                levels,
                ..
            } => levels.decode(
                req,
                size,
                |_| 1,
                |_| {
                    sr::decode_preview(self.source.bytes(), preview, 0)
                        .map(|i| rgba_raster(i, 1))
                        .map_err(map_err)
                },
                cancel,
            ),
        }
    }

    fn prepare_regions(&self, cancel: &CancelToken) -> Result<()> {
        match &self.mode {
            Mode::Preview { inner: Some(s), .. } => s.prepare_regions(cancel),
            _ => Ok(()),
        }
    }

    fn regions_ready(&self) -> bool {
        match &self.mode {
            Mode::Preview { inner: Some(s), .. } => s.regions_ready(),
            _ => true,
        }
    }

    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let sensor = || {
            self.file.info().sensor.as_ref().map_or(0, |x| {
                u64::from(x.width) * u64::from(x.height) * u64::from(x.samples_per_pixel) * 2
            })
        };
        let route = self.route(req);
        match &self.mode {
            Mode::Sensor { .. }
                if let Some(route) = route
                    && let Some(r) = req.region =>
            {
                let (out, dev, size) = self.region_plan(r, s);
                let px = |r: crate::Rect| u64::from(r.w) * u64::from(r.h);
                // The developed 16-bit region and its 8-bit copy (cut down below 1/2); from
                // developed tiles, the tiles to develop (16 and 8-bit) and the 8-bit region.
                let tiles = match route {
                    Route::Tiles => None,
                    _ => self.dev_keys(r, s).map(|k| k.len() as u64),
                };
                let mut m = match tiles {
                    Some(_) if route == Route::Developed => px(dev) * 3,
                    Some(n) => n * u64::from(DEV_TILE * DEV_TILE) * (6 + 3) + px(dev) * 3,
                    None => px(dev) * (6 + 3),
                } + if s > 2 { px(out) * 3 } else { 0 };
                m += match route {
                    // The sensor window and the tiles it touches.
                    Route::Tiles => self
                        .file
                        .region_memory(
                            sr::Rect {
                                x: dev.x,
                                y: dev.y,
                                w: dev.w,
                                h: dev.h,
                            },
                            size,
                        )
                        .unwrap_or(0),
                    Route::Held | Route::Developed => 0,
                    Route::Hold => sensor(),
                };
                m
            }
            Mode::Sensor { levels, .. } => {
                let out = whole::level_bytes(w, h, s, 3);
                let base = if s <= 1 { 1 } else { 2 };
                if levels.has(s) {
                    return out;
                }
                let mut m = out;
                if !levels.has(base) {
                    // Sensor samples (16-bit, CFA or RGB; unless held) + the developed 16-bit
                    // RGB + the 8-bit base.
                    let sensor = if self.held().is_some() { 0 } else { sensor() };
                    m += sensor + whole::level_bytes(w, h, base, 6 + 3);
                }
                m
            }
            Mode::Preview { inner: Some(i), .. } => i.decode_memory(req),
            Mode::Preview { .. } => whole::level_bytes(w, h, 1, 4) * 2,
        }
    }

    fn memory_bytes(&self) -> usize {
        match &self.mode {
            Mode::Sensor {
                levels,
                tiles,
                held,
                developed,
                ..
            } => {
                levels.memory_bytes()
                    + (tiles.memory_bytes() + slot_bytes(held) + developed.memory_bytes()) as usize
            }
            Mode::Preview { inner: Some(i), .. } => i.memory_bytes(),
            Mode::Preview { levels, .. } => levels.memory_bytes(),
        }
    }

    /// Drops the held sensor data, the decoded sensor tiles, the developed tiles and the
    /// developed whole levels: all are made again from the file by the next request that needs
    /// them (a level is developed anew, a region develops alone while its level is missing).
    /// Decodes return copies, so nothing being shown depends on them. A preview session
    /// drops its preview's levels and what its JPEG session keeps.
    fn trim_memory(&self) -> usize {
        match &self.mode {
            Mode::Sensor {
                levels,
                tiles,
                held,
                developed,
                ..
            } => {
                let t = tiles.memory_bytes() + developed.clear();
                tiles.clear();
                let h = held.lock().take().map_or(0, |h| h.bytes());
                let l = levels.clear();
                if t + h > 0 || l > 0 {
                    specular_log::debug!(
                        target: specular_log::DECODE,
                        "raw: {} MiB of held sensor data and tiles, {} MiB of levels released",
                        (t + h) >> 20,
                        l >> 20
                    );
                }
                (t + h) as usize + l
            }
            Mode::Preview { inner, levels, .. } => {
                levels.clear() + inner.as_ref().map_or(0, |i| i.trim_memory())
            }
        }
    }

    fn camera(&self) -> Option<&CameraInfo> {
        Some(&self.camera)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests share [`HOLDERS`].
    static SERIAL: Mutex<()> = Mutex::new(());

    fn slot(n: usize) -> Arc<HeldSlot> {
        let h = held(n, true);
        Arc::new(Mutex::new(Some(Arc::new(h))))
    }

    fn held(n: usize, zoomed: bool) -> Held {
        let raw = sr::RawImage {
            width: n as u32,
            height: 1,
            cpp: 3,
            data: vec![100; n * 3],
            window: sr::Rect {
                x: 0,
                y: 0,
                w: n as u32,
                h: 1,
            },
            cfa: None,
            crop: sr::Rect {
                x: 0,
                y: 0,
                w: n as u32,
                h: 1,
            },
            black: [0.0; 4],
            white: 65535.0,
            linearization: None,
            wb: Some([1.0; 3]),
            rgb_cam: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            matrix_source: sr::MatrixSource::Identity,
            profile: None,
            orientation: 1,
            truncated: false,
            opcodes: None,
        };
        let stats = sr::DevelopStats::of(&raw).unwrap();
        Held {
            raw,
            stats,
            zoomed: AtomicBool::new(zoomed),
        }
    }

    /// Sensor data kept unasked (from a whole-level decode) only makes room by dropping other
    /// such data no region used, least recently used first, and is not kept when that is not
    /// enough.
    #[test]
    fn unasked_hold_drops_only_unzoomed() {
        let _serial = SERIAL.lock();
        let left = |s: &Arc<HeldSlot>| s.lock().is_some();
        let new = || -> Arc<HeldSlot> { Arc::new(Mutex::new(None)) };
        // `a` was used by a region, `b` not.
        let (a, b) = (slot(100), new());
        touch(&a);
        assert!(hold_unasked(&b, held(100, false), |_| u64::MAX));
        assert!(
            !hold_unasked(&b, held(100, false), |_| u64::MAX),
            "held already"
        );
        // No room for 600 more: `b` goes, `a` stays.
        let c = new();
        assert!(hold_unasked(&c, held(100, false), |others| others));
        assert!(left(&a) && !left(&b) && left(&c));
        // Dropping `c` would not be enough (`a`, a region's, stays): nothing dropped, not kept.
        let d = new();
        assert!(!hold_unasked(&d, held(100, false), |others| others - 600));
        assert!(left(&a) && left(&c) && !left(&d));
        // A region's hold may drop any, least recently used first.
        make_room(&d, 600, |others| others - 600);
        assert!(!left(&a) && !left(&c));
    }

    /// Holding more than the budget drops the least recently used other holders first.
    #[test]
    fn held_budget_drops_least_recently_used() {
        let _serial = SERIAL.lock();
        // 600 bytes each.
        let (a, b, c) = (slot(100), slot(100), slot(100));
        for s in [&a, &b, &c] {
            touch(s);
        }
        touch(&a);
        let me: Arc<HeldSlot> = Arc::new(Mutex::new(None));
        // Room for the three and 600 more: none dropped.
        make_room(&me, 600, |held| held + 600);
        assert!(a.lock().is_some() && b.lock().is_some() && c.lock().is_some());
        // No room for 600 more than the others hold: one is dropped, the least recently
        // used (`b`), then `c`.
        let left = |s: &Arc<HeldSlot>| s.lock().is_some();
        make_room(&me, 600, |held| held);
        assert!(left(&a) && !left(&b) && left(&c), "only b dropped");
        make_room(&me, 600, |held| held);
        assert!(left(&a) && !left(&c));
        assert_eq!(slot_bytes(&a), 600);
    }
}
