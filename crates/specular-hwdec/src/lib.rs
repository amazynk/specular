//! Optional hardware image decoding (DESIGN.md §12.3), always with a software fallback.
//!
//! * **Linux:** VA-API. libva (`libva.so.2`, `libva-drm.so.2`) is loaded with `dlopen` at run
//!   time, so there is no build-time C dependency; without it (or without a driver) the probe
//!   reports nothing and every decode stays in software. JPEG baseline is decoded; HEVC and AV1
//!   support is probed and reported for the HEIC/AVIF paths.
//! * **Windows:** D3D12 Video Decode ([`d3d12`]): HEVC (HEIC) and the H.264 / HEVC video of
//!   motion clips through the API Windows ships; AV1 is probed and reported. The slideshow
//!   export's H.264 encoder is the GPU driver's Media Foundation MFT (`mf_enc`). Without a GPU
//!   decoder the probe reports nothing.
//! * **macOS:** VideoToolbox / ImageIO: designed and stubbed behind `cfg` (see [`platform`]);
//!   it reports no capabilities until implemented.
//!
//! Whether a decode goes to the hardware is the path selector's choice
//! (`specular_core::paths`, DESIGN.md §12.3.1): [`probe`] registers what this machine has, the
//! first hardware decode is checked against the software decoder ([`check_quality`]), and if it
//! differs by more than lossy re-decoding should (PSNR below [`MIN_PSNR_DB`]) the path is
//! blocklisted for this driver. [`set_enabled`] is the off switch ("CPU only", or setting
//! `performance.hardware_decode` off).
//!
//! **Where it runs.** We parse every container and codec header in safe Rust ([`jpeg`],
//! `hevc`, `h264`); the driver gets validated parameter structs and the entropy-coded bytes,
//! which the fixed-function engine (not driver C code) parses. [`gate`] checks every struct
//! field the driver sizes or indexes with right before the hand-off. So the decode runs in the
//! viewer process, like the Vulkan driver it shares Mesa with, rather than in the sandboxed
//! decode host: a host would need the DRM render node and its ioctls (or D3D12's device) inside
//! its sandbox, every frame would cross the process boundary, and it would not hide the GPU
//! kernel driver, which the viewer's renderer reaches anyway. See UNSAFE.md for the FFI.

#[cfg(windows)]
mod d3d12;
pub mod encode;
pub mod gate;
#[cfg(any(target_os = "linux", windows))]
pub(crate) mod h264;
pub mod heic;
// Only the Linux and Windows decoders use all of it; macOS decodes HEVC elsewhere.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
pub(crate) mod hevc;
#[cfg(any(target_os = "linux", windows))]
pub(crate) mod hevc_video;
pub mod jpeg;
#[cfg(windows)]
mod mf_enc;
// Only the Linux and Windows decoders use all of it; macOS decodes HEVC elsewhere.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
pub(crate) mod params;
pub mod platform;
#[cfg(target_os = "linux")]
mod vaapi;
pub mod video;
#[cfg(any(target_os = "linux", windows))]
pub(crate) mod video_stream;

pub use params::HevcPlanes;

use parking_lot::Mutex;
use specular_core::paths::{Engine, Task};
use specular_core::{PixelLayout, Raster};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Lowest acceptable Y-plane PSNR of a hardware decode against our software decoder. Different
/// IDCTs on the same coefficients differ by at most ±1–2 levels (≥ 45 dB); a wrong table or
/// layout drops far below 30 dB.
pub const MIN_PSNR_DB: f64 = 40.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file is fine but this decoder can't take it: use software.
    Unsupported(String),
    Malformed(String),
    /// Hardware decoding is off, unavailable or failed the quality check.
    Unavailable,
    /// The driver returned an error.
    Hardware(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "not supported by the hardware decoder: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::Unavailable => write!(f, "hardware decoding is off or unavailable"),
            Error::Hardware(s) => write!(f, "hardware decoder: {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// One codec's decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecCaps {
    pub max_width: u32,
    pub max_height: u32,
    /// Chroma formats the driver can decode into, e.g. "4:2:0, 4:4:4".
    pub formats: String,
}

/// What the hardware decoders of this machine offer.
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    /// "VA-API", "Media Foundation", "VideoToolbox"...
    pub api: &'static str,
    pub device: String,
    pub jpeg: Option<CodecCaps>,
    /// The H.264 decoder (motion photo clips).
    pub h264: Option<CodecCaps>,
    pub hevc: Option<CodecCaps>,
    pub av1: Option<CodecCaps>,
    /// The H.264 encoder (slideshow video export).
    pub h264_encode: Option<CodecCaps>,
    /// Other codecs with decoders (for the hardware report).
    pub others: Vec<&'static str>,
    /// Stream formats of the codecs above that decode in software on this machine.
    pub software_only: Vec<SoftwareOnly>,
}

/// A stream format the hardware decoder hands to the software one, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareOnly {
    /// "H.264 High 10", "HEVC 4:2:2"...
    pub format: &'static str,
    /// "no driver profile"...
    pub reason: &'static str,
}

impl Capabilities {
    /// One line per codec for `--hw-report`.
    pub fn report(&self) -> Vec<String> {
        let c = |name: &str, c: &Option<CodecCaps>| match c {
            Some(c) => format!(
                "{name}: up to {}×{} ({})",
                c.max_width, c.max_height, c.formats
            ),
            None => format!("{name}: no"),
        };
        let mut v = vec![
            format!("{} on {}", self.api, self.device),
            c("JPEG", &self.jpeg),
            c("H.264", &self.h264),
            c("HEVC", &self.hevc),
            c("AV1", &self.av1),
            c("H.264 encode", &self.h264_encode),
        ];
        if !self.others.is_empty() {
            v.push(format!("also: {}", self.others.join(", ")));
        }
        if !self.software_only.is_empty() {
            let s: Vec<String> = self
                .software_only
                .iter()
                .map(|s| format!("{} ({})", s.format, s.reason))
                .collect();
            v.push(format!("software only: {}", s.join(", ")));
        }
        v
    }
}

static ENABLED: AtomicBool = AtomicBool::new(false);
/// 0 not checked yet, 1 passed, 2 failed (off for the session).
static QUALITY: AtomicU8 = AtomicU8::new(0);

/// Allows or forbids hardware decoding ("CPU only" or `performance.hardware_decode` off forbid
/// it). Allowing it does not open the device; [`probe`] or the first decode does.
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Whether hardware decodes should be attempted: enabled, and not failed the quality check.
pub fn enabled() -> bool {
    switched_on() && QUALITY.load(Ordering::Relaxed) != 2
}

/// The off switch alone ([`set_enabled`]), without any codec's quality state; false once a
/// hardware decode timed out ([`heic::wedged`]).
pub(crate) fn switched_on() -> bool {
    ENABLED.load(Ordering::Relaxed) && !heic::wedged()
}

/// Whether the next hardware decode should be compared with a software decode.
pub fn needs_quality_check() -> bool {
    QUALITY.load(Ordering::Relaxed) == 0
}

/// Forgets the quality check's result ("Try again", "Measure again" in Settings →
/// Performance): the next hardware decode is compared with a software decode again.
pub fn reset_quality_check() {
    QUALITY.store(0, Ordering::Relaxed);
}

/// A device with an HEVC decoder as the HEIC core drives it ([`heic`]): a pool of `n`
/// pictures of one coded size and bit depth, pictures queued into pool slots and read back in
/// order. VA-API on Linux, D3D12 on Windows.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
pub(crate) trait HevcDevice {
    fn hevc_limits(&self) -> Option<hevc::Limits>;
    fn hevc_prepare(&mut self, w: u32, h: u32, ten_bit: bool, n: usize) -> Result<(), Error>;
    fn hevc_release(&mut self);
    fn hevc_submit(&mut self, pic: &hevc::Picture<'_>, slot: usize) -> Result<(), Error>;
    fn hevc_read(
        &mut self,
        slot: usize,
        timeout_ns: u64,
        region: (u32, u32, u32, u32),
    ) -> Result<HevcPlanes, Error>;
}

struct Backend {
    #[cfg(target_os = "linux")]
    va: Mutex<vaapi::Va>,
    #[cfg(windows)]
    d3d: Mutex<d3d12::Device>,
    #[cfg(not(any(target_os = "linux", windows)))]
    _none: Mutex<()>,
    caps: Capabilities,
}

static BACKEND: OnceLock<Option<Backend>> = OnceLock::new();

fn backend() -> Option<&'static Backend> {
    BACKEND
        .get_or_init(|| {
            let t0 = std::time::Instant::now();
            #[cfg(target_os = "linux")]
            let b = match vaapi::Va::open() {
                Ok((va, caps)) => Some(Backend {
                    va: Mutex::new(va),
                    caps,
                }),
                Err(e) => {
                    specular_log::debug!(target: specular_log::DECODE, "VA-API unavailable: {e}");
                    None
                }
            };
            #[cfg(windows)]
            let b = match d3d12::Device::open() {
                Ok((d3d, caps)) => Some(Backend {
                    d3d: Mutex::new(d3d),
                    caps,
                }),
                Err(e) => {
                    specular_log::debug!(target: specular_log::DECODE, "D3D12 video unavailable: {e}");
                    None
                }
            };
            #[cfg(not(any(target_os = "linux", windows)))]
            let b = platform::probe().map(|caps| Backend {
                _none: Mutex::new(()),
                caps,
            });
            specular_log::debug!(target: specular_log::DECODE,
                "hardware decode probe: {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
            b
        })
        .as_ref()
}

/// Opens the device (if not yet) and registers its decoders with the path selector as
/// unverified, so the selector may try them (blocklisted or trusted state from the cache is
/// kept). Returns the capabilities. Call off the UI thread: opening takes about 5–30 ms.
pub fn probe() -> Option<&'static Capabilities> {
    let caps = capabilities();
    if let Some(caps) = caps {
        if caps.jpeg.is_some() {
            specular_core::paths::selector().set_available(Task::JpegDecode, Engine::HwDecoder);
        }
        if caps.hevc.is_some() && heic::usable() {
            specular_core::paths::selector().set_available(Task::HeicDecode, Engine::HwDecoder);
        }
        if caps.h264_encode.is_some() {
            specular_core::paths::selector().set_available(Task::VideoEncode, Engine::HwEncoder);
        }
    }
    PROBED.store(true, Ordering::Release);
    caps
}

/// [`probe`] has registered what it found with the path selector.
static PROBED: AtomicBool = AtomicBool::new(false);

/// The hardware decoders of this machine (opens the device on first call: about 5–30 ms).
pub fn capabilities() -> Option<&'static Capabilities> {
    backend().map(|b| &b.caps)
}

/// What the probe found, without opening the device: `None` until [`probe`] has registered
/// the decoders with the path selector (a decode may open the device earlier, but until then
/// the selector does not offer it), `Some(None)` when this machine has no hardware decoder (or
/// no driver for it).
pub fn probed() -> Option<Option<&'static Capabilities>> {
    if !PROBED.load(Ordering::Acquire) {
        return None;
    }
    Some(BACKEND.get().and_then(|b| b.as_ref()).map(|b| &b.caps))
}

/// Whether [`decode_jpeg`] could take an image of this size (without opening the device).
pub fn jpeg_fits(width: u32, height: u32) -> bool {
    BACKEND.get().is_none_or(|b| {
        b.as_ref()
            .and_then(|b| b.caps.jpeg.as_ref())
            .is_some_and(|c| width <= c.max_width && height <= c.max_height)
    })
}

/// Decodes a whole baseline JPEG on the hardware decoder into Y/Cb/Cr (or grey) planes at full
/// size. Fails with [`Error::Unsupported`] for files or sizes the hardware can't take and with
/// [`Error::Unavailable`] when hardware decoding is off; callers fall back to software.
pub fn decode_jpeg(data: &[u8]) -> Result<Raster, Error> {
    if !enabled() {
        return Err(Error::Unavailable);
    }
    decode_jpeg_unchecked(data)
}

/// [`decode_jpeg`] without the enabled check (benchmarks and tests).
pub fn decode_jpeg_unchecked(data: &[u8]) -> Result<Raster, Error> {
    let b = backend().ok_or(Error::Unavailable)?;
    let caps = b.caps.jpeg.as_ref().ok_or(Error::Unavailable)?;
    let j = jpeg::parse(data)?;
    #[cfg(target_os = "linux")]
    {
        // Bounded: a wedged HEVC decode may hold the device forever.
        let va =
            b.va.try_lock_for(heic::DEFAULT_TIMEOUT)
                .ok_or_else(|| Error::Unsupported("the device is busy".into()))?;
        va.decode_jpeg(&j, caps)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (j, caps);
        Err(Error::Unavailable)
    }
}

/// PSNR (dB) between two 8-bit planes of the same size; `None` if the sizes differ.
pub fn psnr(a: &[u8], b: &[u8]) -> Option<f64> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let se: u64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| {
            let d = x as i64 - y as i64;
            (d * d) as u64
        })
        .sum();
    if se == 0 {
        return Some(f64::INFINITY);
    }
    let mse = se as f64 / a.len() as f64;
    Some(10.0 * (255.0 * 255.0 / mse).log10())
}

/// Lowest PSNR over the planes of two rasters of the same layout and size (tightly packed rows
/// compared row by row).
pub fn raster_psnr(a: &Raster, b: &Raster) -> Option<f64> {
    if a.layout != b.layout
        || (a.width, a.height) != (b.width, b.height)
        || a.planes.len() != b.planes.len()
        || !matches!(a.layout, PixelLayout::YCbCr | PixelLayout::Gray)
    {
        return None;
    }
    let mut worst = f64::INFINITY;
    for (p, q) in a.planes.iter().zip(&b.planes) {
        if (p.width, p.height) != (q.width, q.height) {
            return None;
        }
        let w = p.width as usize;
        let rows = |r: &specular_core::RasterPlane| -> Vec<u8> {
            r.data
                .chunks(r.stride.max(1))
                .take(r.height as usize)
                .flat_map(|row| row[..w.min(row.len())].iter().copied())
                .collect()
        };
        worst = worst.min(psnr(&rows(p), &rows(q))?);
    }
    Some(worst)
}

/// Records the result of comparing a hardware decode with the software decode of the same
/// file. Returns whether hardware decoding stays on; below [`MIN_PSNR_DB`] (or with mismatched
/// output) it is switched off for the rest of the session.
pub fn check_quality(hw: &Raster, sw: &Raster) -> bool {
    let p = raster_psnr(hw, sw);
    let ok = p.is_some_and(|p| p >= MIN_PSNR_DB);
    QUALITY.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
    specular_core::paths::selector().verified(Task::JpegDecode, Engine::HwDecoder, ok);
    if ok {
        specular_log::info!(target: specular_log::DECODE,
            "hardware JPEG decode verified: {:.1} dB against software", p.unwrap_or(0.0));
    } else {
        specular_log::warn!(target: specular_log::DECODE,
            "hardware JPEG decode differs from software ({p:?} dB); using software for this session");
    }
    ok
}
