//! JPEG XL decoding for Specular.
//!
//! A deliberately narrow boundary over jxl-oxide (forked in-tree, see `NOTICE`): the rest of Specular sees only the types
//! in this crate, so the decoder behind it can be forked or replaced without touching the app.
//!
//! What this layer adds on top of jxl-oxide:
//!
//! * **8-bit output, fused with scaling.** jxl-oxide renders to `f32` (or integer modular)
//!   planes. Instead of its per-sample `ImageStream` (which also
//!   applies orientation per sample), we read the colour and alpha planes directly, box-average
//!   `scale × scale` blocks in the encoded (gamma) domain and quantise in one parallel pass.
//!   Orientation is *not* applied; it is reported in [`Info::orientation`] (EXIF numbering).
//! * **sRGB by default, HDR tone-mapped.** Images whose transfer function is PQ or HLG are
//!   rendered to sRGB with jxl-oxide's built-in tone mapping; everything else is rendered in its
//!   own colour space and [`Info::output_icc`] describes it (`None` means sRGB).
//! * **Wide output for HDR stills.** [`JxlFile::decode_hdr`] keeps the 16-bit PQ / HLG
//!   signal and [`JxlFile::decode_linear`] linear light as half floats (float modular samples
//!   included), both box-averaged in the same pass and kept in the file's gamut and range
//!   (the fork's `set_unbounded`: no tone mapping to 255 cd/m², no gamut mapping of XYB to
//!   sRGB), as libjxl renders them.
//! * **Truncated files** render whatever has been received (progressive passes / LF groups).
//! * A shared thread pool, so per-decode thread start-up is not paid.
//! * **CMYK** (a black extra channel, with the embedded CMYK profile): [`JxlFile::decode`],
//!   [`JxlFile::decode_dc`] and [`JxlFile::frames`] (stills, layered and animated alike)
//!   convert the inks to sRGB through the process-wide [`set_cmyk_hook`] (the naive formula
//!   without one), then box-average; [`JxlFile::decode_cmyk`] returns the inks themselves.
//!
//! # Not supported
//! * CMYK through [`JxlFile::decode_hdr`] and [`JxlFile::decode_linear`]: CMYK needs an ICC
//!   profile, so it is never PQ, HLG or an enum linear encoding.
//! * CMYK through [`JxlFile::decode_samples`], which returns grey or RGB samples only.
//! * Associated (premultiplied) alpha over CMYK is taken as straight alpha.

#![feature(portable_simd)]
#![deny(unsafe_code)]

// The in-tree fork of jxl-oxide (see NOTICE). Module `X` corresponds to upstream crate `jxl-X`
// (`oxide` is `jxl-oxide` itself, `common` is `jxl-oxide-common`).
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod bitstream;
mod cmyk_hook;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod coding;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod color;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod common;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod frame;
mod gain_map;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod grid;
#[cfg(test)]
mod hostile;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod image;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod modular;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod oxide;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod render;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod threadpool;
mod tracing;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces,
    redundant_semicolons,
    mismatched_lifetime_syntaxes,
    clippy::all
)]
mod vardct;

pub use crate::cmyk_hook::{CmykHook, CmykRowConverter, set_cmyk_hook};
pub use crate::gain_map::{GainMapBundle, Primaries, parse_gain_map_bundle};
use crate::image::BitDepth;
use crate::image::color::{ColourEncoding, TransferFunction};
pub use crate::image::{HeaderLayout, header_layout};
use crate::oxide::{
    AllocTracker, ChannelOrigin, EnumColourEncoding, HdrType, JxlImage, JxlThreadPool,
    RenderingIntent,
};
use crate::render::ImageBuffer;
use rayon::prelude::*;
use std::sync::{Arc, OnceLock};

/// Errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a JPEG XL file")]
    NotJxl,
    #[error("truncated: {0}")]
    Truncated(String),
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("image too large")]
    TooLarge,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Largest image accepted, in pixels (the decoder works in `f32`, ~16 bytes per pixel).
pub const MAX_PIXELS: u64 = 1 << 29;

/// The decoder's tracked allocations (sample planes, MA trees, compressed group data, patch and
/// spline lists) are capped at this many bytes per declared channel sample: the canvas plus
/// every frame (`width × height`), times the colour and extra channels. A file whose headers
/// declare a small image cannot make the decoder allocate gigabytes (a frame cropped far beyond
/// the canvas, millions of splines or patches, a huge palette). The libjxl conformance suite
/// and this crate's tests all decode at 8 (the tightest case needs more than 4); 32 leaves
/// headroom for the narrow and wide render caches coexisting.
const TRACKED_BYTES_PER_SAMPLE: u64 = 32;

/// Tracked-allocation allowance for parsing, on top of the file's own size (its compressed
/// groups are copied once): headers, MA trees, ICC profiles, TOCs.
const TRACKED_PARSE_BYTES: usize = 64 << 20;

/// Whether `head` starts like a JPEG XL file (bare codestream or ISOBMFF container).
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(&[0xff, 0x0a])
        || head.starts_with(&[
            0, 0, 0, 0x0c, b'J', b'X', b'L', b' ', 0x0d, 0x0a, 0x87, 0x0a,
        ])
}

/// HDR transfer function of the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hdr {
    Pq,
    Hlg,
}

/// Header facts.
#[derive(Clone, Debug)]
pub struct Info {
    /// Stored size (before orientation).
    pub width: u32,
    pub height: u32,
    /// EXIF-style orientation, 1..=8.
    pub orientation: u32,
    pub gray: bool,
    pub has_alpha: bool,
    /// The alpha extra channel is associated (premultiplied); decoded pixels are straight.
    pub premultiplied_alpha: bool,
    /// Bits per sample of the colour channels.
    pub bits_per_sample: u32,
    pub float_samples: bool,
    /// VarDCT/XYB ("lossy") rather than modular non-XYB ("lossless" / modular).
    pub xyb: bool,
    pub hdr: Option<Hdr>,
    /// CMYK (black extra channel, described by [`Info::original_icc`]): the 8-bit decodes
    /// convert it to sRGB through [`set_cmyk_hook`]; [`JxlFile::decode_cmyk`] gives its inks.
    pub cmyk: bool,
    pub animated: bool,
    /// Loop count, 0 = forever.
    pub loop_count: u32,
    /// The file carries JPEG reconstruction data (a losslessly recompressed JPEG).
    pub jpeg_reconstruction: bool,
    /// ICC profile embedded in the file (describes the *original*, not necessarily the output).
    pub original_icc: Option<Vec<u8>>,
    /// ICC profile of the decoded 8-bit output; `None` when the output is sRGB.
    pub output_icc: Option<Vec<u8>>,
    pub exif: Option<Vec<u8>>,
    /// Primaries of the file's own colour encoding, when it is an enum encoding with a D65
    /// white point and primaries Specular converts (grey counts as BT.709).
    pub primaries: Option<Primaries>,
    /// The container has a gain map box (`jhgm`), readable or not: see
    /// [`JxlFile::gain_map`].
    pub gain_map: bool,
    /// The colour encoding is an enum one with the linear transfer function (see
    /// [`JxlFile::decode_linear`]).
    pub linear: bool,
    /// Luminance of (1, 1, 1) in cd/m²: the header's intensity target (255 by default).
    pub intensity_target: f32,
}

/// Pixel layout of a [`Decoded`] image; interleaved, 8 bits per sample except
/// [`Layout::Rgba16`] and [`Layout::Rgba16F`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Gray,
    Rgb,
    /// Straight (unassociated) alpha.
    Rgba,
    /// Native-endian `u16` RGBA, straight alpha ([`JxlFile::decode_hdr`]).
    Rgba16,
    /// Native-endian IEEE half-float RGBA, linear light, straight alpha
    /// ([`JxlFile::decode_linear`]).
    Rgba16F,
}

impl Layout {
    pub fn channels(self) -> usize {
        match self {
            Layout::Gray => 1,
            Layout::Rgb => 3,
            Layout::Rgba | Layout::Rgba16 | Layout::Rgba16F => 4,
        }
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Layout::Rgba16 | Layout::Rgba16F => 8,
            l => l.channels(),
        }
    }
}

/// A decoded frame.
#[derive(Clone, Debug)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    pub layout: Layout,
    /// `width * bytes_per_pixel` bytes per row.
    pub data: Vec<u8>,
    /// Frame duration in milliseconds (animations; 0 for stills).
    pub duration_ms: u32,
    /// False when the file was truncated and this is a partial (progressive) render.
    pub complete: bool,
}

/// Integer samples from [`JxlFile::decode_samples`].
#[derive(Clone, Debug)]
pub struct Samples16 {
    pub width: u32,
    pub height: u32,
    /// 1 (grey) or 3 (RGB, interleaved).
    pub channels: u8,
    /// Bits per sample (values are `0..2^bits`).
    pub bits: u8,
    pub data: Vec<u16>,
    /// False when the codestream was cut short and the rest is filled by the decoder.
    pub complete: bool,
}

/// Float samples of a still ([`JxlFile::decode_float_samples`]).
#[derive(Clone, Debug)]
pub struct SamplesF32 {
    pub width: u32,
    pub height: u32,
    /// 1 (grey) or 3 (RGB, interleaved).
    pub channels: u8,
    pub data: Vec<f32>,
    /// False when the codestream was cut short and the rest is filled by the decoder.
    pub complete: bool,
}

/// The inks of a CMYK image ([`JxlFile::decode_cmyk`]).
#[derive(Clone, Debug)]
pub struct Cmyk {
    pub width: u32,
    pub height: u32,
    /// C, M, Y, K per pixel, 0 = no ink, 255 = solid.
    pub ink: Vec<u8>,
    /// Straight alpha, a byte per pixel, when the image has an alpha channel.
    pub alpha: Option<Vec<u8>>,
    /// False when the codestream was cut short and the rest is filled by the decoder.
    pub complete: bool,
}

/// An opened JPEG XL file over a byte container.
pub struct JxlFile<D: AsRef<[u8]>> {
    data: D,
    info: Info,
    /// A Brotli-compressed (`brob`) `jhgm` box's body, decompressed on first use.
    brob_gain_map: OnceLock<std::result::Result<Vec<u8>, String>>,
}

/// Limit of a decompressed `brob` gain map bundle (its codestream is compressed already, so
/// it is about the size of the box).
const MAX_BROB_GAIN_MAP: usize = 64 << 20;

fn pool() -> Arc<rayon::ThreadPool> {
    static POOL: OnceLock<Arc<rayon::ThreadPool>> = OnceLock::new();
    POOL.get_or_init(|| {
        let n = std::thread::available_parallelism().map_or(1, |n| n.get());
        let named = || specular_thread::rayon_pool("dec-jxl");
        Arc::new(
            named()
                .num_threads(n)
                // Renders are spawned detached here; without a handler their panic would abort
                // the process. The panic hook has logged it, and the frame is marked failed
                // (`FrameRenderHandle::render_guarded`).
                .panic_handler(drop)
                .build()
                .unwrap_or_else(|_| {
                    named()
                        .num_threads(1)
                        .panic_handler(drop)
                        .build()
                        .expect("a one-thread rayon pool")
                }),
        )
    })
    .clone()
}

/// Parses a file. `tone_map`: PQ and HLG images render to sRGB (otherwise they render in
/// their own colour encoding).
fn load(data: &[u8], parallel: bool, tone_map: bool) -> Result<JxlImage> {
    if !probe(data) {
        return Err(Error::NotJxl);
    }
    let pool = if parallel {
        JxlThreadPool::with_rayon_thread_pool(pool())
    } else {
        JxlThreadPool::none()
    };
    let tracker = AllocTracker::with_limit(TRACKED_PARSE_BYTES.saturating_add(data.len()));
    // jxl-oxide may panic on some malformed inputs; contain it.
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        JxlImage::builder()
            .pool(pool)
            .alloc_tracker(tracker.clone())
            .read(data)
    }));
    let mut image = match r {
        Ok(Ok(i)) => i,
        Ok(Err(e)) => return Err(classify(&*e, data.len())),
        Err(_) => return Err(Error::Malformed("decoder panicked while parsing".into())),
    };
    let max_px = max_frame_px(&image);
    if max_px > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    let header = image.image_header();
    // Declared samples: the canvas and every frame, all channels (each area at most
    // MAX_PIXELS, at most 256 extra channels; saturating against absurd frame counts).
    let channels = 3 + header.metadata.ec_info.len() as u64;
    let declared = (0..)
        .map_while(|i| image.frame(i))
        .map(|f| f.header().width as u64 * f.header().height as u64)
        .fold(
            header.size.width as u64 * header.size.height as u64,
            u64::saturating_add,
        );
    let budget = declared
        .saturating_mul(channels)
        .saturating_mul(TRACKED_BYTES_PER_SAMPLE)
        .min(1 << 48);
    tracker.expand_limit(budget as usize);
    if tone_map && image.hdr_type().is_some() {
        let enc = if header.metadata.grayscale() {
            EnumColourEncoding::gray_srgb(RenderingIntent::Relative)
        } else {
            EnumColourEncoding::srgb(RenderingIntent::Relative)
        };
        image.request_color_encoding(enc);
    }
    Ok(image)
}

/// The larger of the canvas and the largest frame, in pixels (a frame may be larger than the
/// canvas and placed partly outside it).
fn max_frame_px(image: &JxlImage) -> u64 {
    let s = &image.image_header().size;
    (0..)
        .map_while(|i| image.frame(i))
        .map(|f| f.header().width as u64 * f.header().height as u64)
        .fold(s.width as u64 * s.height as u64, u64::max)
}

fn classify(e: &(dyn std::error::Error + 'static), _len: usize) -> Error {
    let s = e.to_string();
    let l = s.to_ascii_lowercase();
    if l.contains("eof") || l.contains("end of") || l.contains("more data") || l.contains("truncat")
    {
        Error::Truncated(s)
    } else {
        Error::Malformed(s)
    }
}

impl<D: AsRef<[u8]>> JxlFile<D> {
    /// Parse the headers (frames are indexed but not decoded).
    pub fn open(data: D) -> Result<Self> {
        let image = load(data.as_ref(), false, true)?;
        let h = image.image_header();
        let m = &h.metadata;
        let alpha = m.ec_info.iter().any(|e| e.is_alpha());
        let premultiplied_alpha = m
            .ec_info
            .iter()
            .find(|e| e.is_alpha())
            .is_some_and(|e| e.alpha_associated() == Some(true));
        let cmyk = m.ec_info.iter().any(|e| e.is_black());
        let (bits, float) = match m.bit_depth {
            BitDepth::IntegerSample { bits_per_sample } => (bits_per_sample, false),
            BitDepth::FloatSample {
                bits_per_sample, ..
            } => (bits_per_sample, true),
        };
        let hdr = image.hdr_type().map(|t| match t {
            HdrType::Pq => Hdr::Pq,
            HdrType::Hlg => Hdr::Hlg,
        });
        let output_icc = match image.rendered_cicp() {
            // CMYK is converted to sRGB.
            _ if cmyk => None,
            Some([1, 13, _, _]) => None,
            _ => Some(image.rendered_icc()),
        };
        let exif = image.aux_boxes().first_exif().ok().and_then(|d| match d {
            crate::oxide::AuxBoxData::Data(e) => Some(e.payload().to_vec()),
            _ => None,
        });
        let info = Info {
            width: h.size.width,
            height: h.size.height,
            orientation: m.orientation,
            gray: m.grayscale(),
            has_alpha: alpha,
            premultiplied_alpha,
            bits_per_sample: bits,
            float_samples: float,
            xyb: m.xyb_encoded,
            hdr,
            cmyk,
            animated: m.animation.is_some(),
            loop_count: m.animation.as_ref().map_or(0, |a| a.num_loops),
            jpeg_reconstruction: matches!(
                image.jpeg_reconstruction_status(),
                crate::oxide::JpegReconstructionStatus::Available
            ),
            original_icc: image.original_icc().map(<[u8]>::to_vec),
            output_icc,
            exif,
            primaries: gain_map::enum_primaries(&m.colour_encoding),
            gain_map: gain_map::find_gain_map(data.as_ref()) != gain_map::Found::None,
            linear: matches!(
                &m.colour_encoding,
                ColourEncoding::Enum(e) if e.tf == TransferFunction::Linear
            ),
            intensity_target: m.tone_mapping.intensity_target,
        };
        Ok(JxlFile {
            data,
            info,
            brob_gain_map: OnceLock::new(),
        })
    }

    pub fn info(&self) -> &Info {
        &self.info
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_ref()
    }

    /// Estimated peak memory of [`JxlFile::decode`] at `scale`: jxl-oxide's `f32` planes plus
    /// its working set, plus the 8-bit output.
    pub fn decode_memory(&self, scale: u32) -> u64 {
        let ch = if self.info.gray { 1 } else { 3 } + self.info.has_alpha as u64;
        self.memory(scale, ch)
    }

    /// Like [`JxlFile::decode_memory`], for the 8-byte-per-pixel output of
    /// [`JxlFile::decode_hdr`] and [`JxlFile::decode_linear`].
    pub fn decode_wide_memory(&self, scale: u32) -> u64 {
        self.memory(scale, 8)
    }

    fn memory(&self, scale: u32, out_bytes_per_px: u64) -> u64 {
        let px = self.info.width as u64 * self.info.height as u64;
        let ch = if self.info.gray { 1 } else { 3 } + self.info.has_alpha as u64;
        let s = scale.max(1) as u64;
        // f32 planes + roughly one plane-set of intermediates (VarDCT coefficients, groups).
        px * ch * 4 * 2 + px.div_ceil(s * s) * out_bytes_per_px
    }

    /// Decode the first frame at `1/scale` (box filter). CMYK is converted to sRGB first
    /// (see [`set_cmyk_hook`]).
    pub fn decode(&self, scale: u32, parallel: bool) -> Result<Decoded> {
        let mut image = load(self.data.as_ref(), parallel, true)?;
        let pool = parallel.then(pool);
        render(
            &mut image,
            0,
            &self.info,
            scale,
            pool.as_deref(),
            0,
            Out::Bits8,
        )
    }

    /// Decode the first frame of a PQ or HLG image at `1/scale` without tone mapping:
    /// [`Layout::Rgba16`] holding the HDR signal in the file's own primaries
    /// ([`Info::primaries`]). `Unsupported` for other images.
    pub fn decode_hdr(&self, scale: u32, parallel: bool) -> Result<Decoded> {
        if self.info.hdr.is_none() || self.info.primaries.is_none() || self.info.cmyk {
            return Err(Error::Unsupported("not a PQ or HLG image".into()));
        }
        let mut image = load(self.data.as_ref(), parallel, false)?;
        // Wide-gamut XYB (BT.2020) keeps its colours instead of being gamut mapped to sRGB.
        image.set_unbounded(true);
        if !matches!(image.rendered_cicp(), Some([_, 16 | 18, ..])) {
            return Err(Error::Unsupported(
                "HDR image without an enum colour encoding".into(),
            ));
        }
        let pool = parallel.then(pool);
        render(
            &mut image,
            0,
            &self.info,
            scale,
            pool.as_deref(),
            0,
            Out::Signal16,
        )
    }

    /// Decode the first frame of a linear-light image ([`Info::linear`], with
    /// [`Info::primaries`]) at `1/scale` (a box average in linear light) to
    /// [`Layout::Rgba16F`], in the file's own primaries, with 1.0 = SDR reference white:
    /// values above it (float samples, XYB) are kept, NaN becomes 0 and anything beyond the
    /// half range the largest half. `Unsupported` for other images.
    ///
    /// The samples are relative to [`Info::intensity_target`] cd/m². At most 255 (the default)
    /// is SDR, and 1.0 stays reference white (as in OpenEXR); above it 1.0 is that many cd/m²
    /// and reference white is BT.2408's 203 cd/m², so a linear version of a PQ image shows
    /// the same.
    pub fn decode_linear(&self, scale: u32, parallel: bool) -> Result<Decoded> {
        if !self.info.linear || self.info.primaries.is_none() || self.info.cmyk {
            return Err(Error::Unsupported("not a linear image".into()));
        }
        let mut image = load(self.data.as_ref(), parallel, false)?;
        // jxl-oxide would tone map an intensity target above 255 to SDR (and scale 1.0 to
        // 255 cd/m²) and gamut map XYB to sRGB; libjxl keeps linear output relative to the
        // target, in the file's gamut.
        image.set_unbounded(true);
        if !matches!(image.rendered_cicp(), Some([_, 8, ..]) | None) {
            return Err(Error::Unsupported(
                "linear image rendered non-linear".into(),
            ));
        }
        let it = self.info.intensity_target;
        let mul = if it > 255.0 { it / 203.0 } else { 1.0 };
        let pool = parallel.then(pool);
        render(
            &mut image,
            0,
            &self.info,
            scale,
            pool.as_deref(),
            0,
            Out::Half(mul),
        )
    }

    /// Decode the first frame at full size to its integer samples at the file's own bit depth
    /// ([`Info::bits_per_sample`], at most 16): grey (one sample per pixel) or RGB (three,
    /// interleaved), alpha dropped, no colour management. Lossless modular samples come back
    /// exactly; lossy (VarDCT, XYB) ones rounded from the nominal `[0, 1]` range. For
    /// containers (DICOM, TIFF) that need the stored values rather than a display rendering.
    pub fn decode_samples(&self) -> Result<Samples16> {
        let info = &self.info;
        if info.cmyk || info.float_samples || info.bits_per_sample > 16 || info.animated {
            return Err(Error::Unsupported(
                "not an integer still of up to 16 bits".into(),
            ));
        }
        let (planes, complete) = self.first_frame_planes()?;
        let (w, h) = (info.width as usize, info.height as usize);
        let channels = planes.len();
        let max = ((1u32 << info.bits_per_sample) - 1) as f32;
        let mut data = vec![0u16; w * h * channels];
        for (c, plane) in planes.iter().enumerate() {
            for (i, &v) in plane.iter().enumerate() {
                data[i * channels + c] = (v * max + 0.5) as u16;
            }
        }
        Ok(Samples16 {
            width: info.width,
            height: info.height,
            channels: channels as u8,
            bits: info.bits_per_sample as u8,
            data,
            complete,
        })
    }

    /// Decode the first frame of a float image ([`Info::float_samples`]) at full size to its
    /// samples as stored: grey or RGB (interleaved), alpha dropped, no colour management.
    /// Lossless modular samples come back exactly. For containers (TIFF) that need the stored
    /// values.
    pub fn decode_float_samples(&self) -> Result<SamplesF32> {
        let info = &self.info;
        if info.cmyk || !info.float_samples || info.animated {
            return Err(Error::Unsupported("not a float still".into()));
        }
        let (planes, complete) = self.first_frame_planes()?;
        let channels = planes.len();
        let n = info.width as usize * info.height as usize;
        let mut data = vec![0f32; n * channels];
        for (c, plane) in planes.iter().enumerate() {
            for (i, &v) in plane.iter().enumerate() {
                data[i * channels + c] = v;
            }
        }
        Ok(SamplesF32 {
            width: info.width,
            height: info.height,
            channels: channels as u8,
            data,
            complete,
        })
    }

    /// The first frame's colour planes (grey, or R, G, B) at full size, row-major, as the
    /// decoder renders them (nominal range [0, 1]); and whether the frame was complete.
    fn first_frame_planes(&self) -> Result<(Vec<Vec<f32>>, bool)> {
        let info = &self.info;
        let mut image = load(self.data.as_ref(), false, false)?;
        let complete = image.num_loaded_keyframes() > 0;
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if complete {
                image.render_frame(0)
            } else {
                image.render_loading_frame()
            }
        }));
        let render = match r {
            Ok(Ok(r)) => r,
            Ok(Err(e)) if complete => return Err(classify(&*e, 0)),
            Ok(Err(e)) => return Err(Error::Truncated(e.to_string())),
            Err(_) => return Err(Error::Malformed("decoder panicked while rendering".into())),
        };
        let (w, h) = (info.width as usize, info.height as usize);
        let (srcs, _) = sources(&render, &image, (w, h))?;
        let channels = if info.gray { 1 } else { 3 };
        if srcs.len() < channels {
            return Err(Error::Malformed("missing colour channels".into()));
        }
        let mut planes = Vec::with_capacity(channels);
        for src in srcs.iter().take(channels) {
            let mut plane = vec![0.0f32; w * h];
            for y in 0..h {
                src.row(y, w, &mut plane[y * w..(y + 1) * w]);
            }
            planes.push(plane);
        }
        Ok((planes, complete))
    }

    /// Decode the first frame of a CMYK image ([`Info::cmyk`]: a black extra channel) at full
    /// size to its inks, for a CMYK colour conversion with the embedded profile
    /// ([`Info::original_icc`]): no colour management here. For an animation, the first
    /// frame.
    pub fn decode_cmyk(&self, parallel: bool) -> Result<Cmyk> {
        let info = &self.info;
        if !info.cmyk {
            return Err(Error::Unsupported("not a CMYK image".into()));
        }
        let mut image = load(self.data.as_ref(), parallel, false)?;
        let complete = image.num_loaded_keyframes() > 0;
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if complete {
                image.render_frame(0)
            } else {
                image.render_loading_frame()
            }
        }));
        let render = match r {
            Ok(Ok(r)) => r,
            Ok(Err(e)) if complete => return Err(classify(&*e, 0)),
            Ok(Err(e)) => return Err(Error::Truncated(e.to_string())),
            Err(_) => return Err(Error::Malformed("decoder panicked while rendering".into())),
        };
        let size = (info.width as usize, info.height as usize);
        let (inks, alpha) = cmyk_sources(&render, &image, size)?;
        let (w, h) = size;
        let mut ink = vec![0u8; w * h * 4];
        let mut a = alpha.as_ref().map(|_| vec![0u8; w * h]);
        let mut row = vec![0.0f32; w];
        for y in 0..h {
            ink_row(&inks, y, w, &mut row, &mut ink[y * w * 4..(y + 1) * w * 4]);
            if let (Some(src), Some(a)) = (&alpha, a.as_mut()) {
                src.row(y, w, &mut row);
                for (o, &v) in a[y * w..(y + 1) * w].iter_mut().zip(&row) {
                    *o = q8(v.clamp(0.0, 1.0));
                }
            }
        }
        Ok(Cmyk {
            width: info.width,
            height: info.height,
            ink,
            alpha: a,
            complete,
        })
    }

    /// The gain map bundle of the `jhgm` box (plain or Brotli-compressed in a `brob` box):
    /// `None` without one, an error when the box is there but unreadable (cut short, not valid
    /// Brotli, an unknown version, malformed).
    pub fn gain_map(&self) -> Option<Result<GainMapBundle<'_>>> {
        let data = self.data.as_ref();
        match gain_map::find_gain_map(data) {
            gain_map::Found::None => None,
            gain_map::Found::Unreadable(why) => Some(Err(Error::Unsupported(why.into()))),
            gain_map::Found::Box(r) => Some(parse_gain_map_bundle(&data[r])),
            gain_map::Found::Brotli(r) => {
                let body = self.brob_gain_map.get_or_init(|| {
                    specular_brotli::decompress(&data[r], MAX_BROB_GAIN_MAP)
                        .map_err(|e| format!("jhgm box: {e}"))
                });
                Some(match body {
                    Ok(body) => parse_gain_map_bundle(body),
                    Err(e) => Err(Error::Malformed(e.clone())),
                })
            }
        }
    }

    /// Size of the gain map image (from its codestream's headers).
    pub fn gain_map_size(&self) -> Result<(u32, u32)> {
        let g = self
            .gain_map()
            .ok_or_else(|| Error::Unsupported("no gain map".into()))??;
        let image = load(g.codestream, false, false)?;
        let s = &image.image_header().size;
        Ok((s.width, s.height))
    }

    /// Decode the gain map image at full size, 8 bits per sample (grey or RGB), its samples
    /// as coded: no tone mapping or colour conversion.
    pub fn decode_gain_map(&self, parallel: bool) -> Result<Decoded> {
        let g = self
            .gain_map()
            .ok_or_else(|| Error::Unsupported("no gain map".into()))??;
        let mut image = load(g.codestream, parallel, false)?;
        let h = image.image_header();
        let m = &h.metadata;
        if m.ec_info.iter().any(|e| e.is_black()) || m.animation.is_some() {
            return Err(Error::Unsupported("CMYK or animated gain map".into()));
        }
        let info = Info {
            width: h.size.width,
            height: h.size.height,
            orientation: 1,
            gray: m.grayscale(),
            has_alpha: false,
            premultiplied_alpha: false,
            bits_per_sample: 8,
            float_samples: false,
            xyb: m.xyb_encoded,
            hdr: None,
            cmyk: false,
            animated: false,
            loop_count: 0,
            jpeg_reconstruction: false,
            original_icc: None,
            output_icc: None,
            exif: None,
            primaries: None,
            gain_map: false,
            linear: false,
            intensity_target: m.tone_mapping.intensity_target,
        };
        let pool = parallel.then(pool);
        let mut d = render(&mut image, 0, &info, 1, pool.as_deref(), 0, Out::Bits8)?;
        if !d.complete {
            return Err(Error::Truncated("gain map".into()));
        }
        // An alpha channel means nothing in a gain map: drop it.
        if d.layout == Layout::Rgba {
            d.data = d
                .data
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            d.layout = Layout::Rgb;
        }
        Ok(d)
    }

    /// Fast 1/8 preview of the first frame.
    ///
    /// For a VarDCT image this decodes only the LF (DC) image — LfGlobal and the LF groups,
    /// with adaptive LF smoothing — and colour-converts it at 1/8 resolution, skipping all HF
    /// coefficients, IDCTs and restoration filters (Gaborish, EPF), patches, splines and noise.
    /// Otherwise (modular, alpha, CMYK, chroma-subsampled JPEG recompression, animation,
    /// layered frames, truncated LF) it falls back to `decode(8, parallel)`. The result has
    /// `scale == 8` and the size of `decode(8, …)`.
    pub fn decode_dc(&self, parallel: bool) -> Result<Decoded> {
        // CMYK needs its black channel, which the LF preview does not render.
        if !self.info.has_alpha && !self.info.cmyk {
            let image = load(self.data.as_ref(), parallel, true)?;
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                image.render_lf_preview()
            }));
            if let Ok(Ok(Some(render))) = r {
                let pool = parallel.then(pool);
                let size = (
                    self.info.width.div_ceil(8) as usize,
                    self.info.height.div_ceil(8) as usize,
                );
                if let Ok(mut d) = convert(&render, &image, size, 1, pool.as_deref()) {
                    d.scale = 8;
                    return Ok(d);
                }
            }
        }
        self.decode(8, parallel)
    }

    /// Iterate over the animation frames (keyframes), each composited on the canvas, at
    /// `1/scale`.
    /// The iterator owns its state (no borrow of `self`); create a new one to rewind.
    pub fn frames(&self, scale: u32, parallel: bool) -> Result<Frames> {
        let image = load(self.data.as_ref(), parallel, true)?;
        let (num, den) = image
            .image_header()
            .metadata
            .animation
            .as_ref()
            .map_or((0, 1), |a| (a.tps_numerator, a.tps_denominator));
        Ok(Frames {
            info: self.info.clone(),
            image,
            index: 0,
            scale,
            parallel,
            tps: (num, den),
        })
    }
}

/// Animation frames, see [`JxlFile::frames`].
pub struct Frames {
    info: Info,
    image: JxlImage,
    index: usize,
    scale: u32,
    parallel: bool,
    tps: (u32, u32),
}

impl Frames {
    /// Number of complete keyframes in the file.
    pub fn len(&self) -> usize {
        self.image.num_loaded_keyframes()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Iterator for Frames {
    type Item = Result<Decoded>;

    fn next(&mut self) -> Option<Result<Decoded>> {
        if self.index >= self.image.num_loaded_keyframes() {
            return None;
        }
        let ticks = self
            .image
            .frame_header(self.index)
            .map_or(0, |h| h.duration as u64);
        let ms = if self.tps.0 == 0 {
            0
        } else {
            (ticks * 1000 * self.tps.1 as u64 / self.tps.0 as u64).min(u32::MAX as u64) as u32
        };
        let pool = self.parallel.then(pool);
        let r = render(
            &mut self.image,
            self.index,
            &self.info,
            self.scale,
            pool.as_deref(),
            ms,
            Out::Bits8,
        );
        self.index += 1;
        Some(r)
    }
}

/// The output of [`render`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum Out {
    /// 8 bits per sample ([`convert`]).
    Bits8,
    /// The `[0, 1]` signal in 16 bits ([`Layout::Rgba16`]).
    Signal16,
    /// Linear half floats times the factor ([`Layout::Rgba16F`]).
    Half(f32),
}

fn render(
    image: &mut JxlImage,
    keyframe: usize,
    info: &Info,
    scale: u32,
    pool: Option<&rayon::ThreadPool>,
    duration_ms: u32,
    out: Out,
) -> Result<Decoded> {
    // A truncated file renders whatever has been received of the first incomplete frame.
    let complete = image.num_loaded_keyframes() > keyframe;
    if !complete && keyframe > image.num_loaded_keyframes() {
        return Err(Error::Truncated("frame not present".into()));
    }
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if complete {
            image.render_frame(keyframe)
        } else {
            image.render_loading_frame()
        }
    }));
    let render = match r {
        Ok(Ok(r)) => r,
        Ok(Err(e)) if complete => return Err(classify(&*e, 0)),
        Ok(Err(e)) => return Err(Error::Truncated(e.to_string())),
        Err(_) => return Err(Error::Malformed("decoder panicked while rendering".into())),
    };
    let size = (info.width as usize, info.height as usize);
    let mut d = match out {
        Out::Bits8 if info.cmyk => {
            let conv = cmyk_hook::converter(info.original_icc.as_deref());
            convert_cmyk(&render, image, size, scale, pool, conv.as_ref())?
        }
        Out::Bits8 => convert(&render, image, size, scale, pool)?,
        wide => convert_wide(&render, image, size, scale, pool, wide)?,
    };
    d.duration_ms = duration_ms;
    d.complete = complete;
    Ok(d)
}

/// The samples of one buffer.
enum Samples<'a> {
    F32(&'a crate::oxide::AlignedGrid<f32>),
    I32(&'a crate::oxide::AlignedGrid<i32>, f32),
    I16(&'a crate::oxide::AlignedGrid<i16>, f32),
    /// Float modular samples still as their bit patterns: (total bits, exponent bits).
    Float(&'a crate::oxide::AlignedGrid<i32>, (u32, u32)),
    Float16(&'a crate::oxide::AlignedGrid<i16>, (u32, u32)),
}

/// Read one channel of a render, in image coordinates, as nominal `[0, 1]`.
///
/// A rendered buffer need not start at the image origin: a frame may be larger than the image
/// and placed at a negative offset (libjxl conformance `sunset_logo`), so image pixel `(x, y)`
/// is buffer sample `(x + ox, y + oy)`. Samples outside the buffer read as 0, as in
/// [`crate::oxide::Render::image_all_channels`].
struct Src<'a> {
    samples: Samples<'a>,
    ox: i64,
    oy: i64,
    /// Valid buffer size.
    bw: usize,
    bh: usize,
    /// Every image pixel lies inside the buffer (the usual case): rows are plain slices.
    covers: bool,
}

impl<'a> Src<'a> {
    fn new(
        b: &'a ImageBuffer,
        depth: BitDepth,
        origin: Option<&ChannelOrigin>,
        (w, h): (usize, usize),
    ) -> Result<Self> {
        let inv = match depth {
            BitDepth::IntegerSample { bits_per_sample } => {
                1.0 / ((1u64 << bits_per_sample.min(31)) - 1) as f32
            }
            BitDepth::FloatSample { .. } => 0.0,
        };
        let float = match depth {
            BitDepth::FloatSample {
                bits_per_sample,
                exp_bits,
            } if (2..=8).contains(&exp_bits) && (exp_bits + 2..=32).contains(&bits_per_sample) => {
                Some((bits_per_sample, exp_bits))
            }
            _ => None,
        };
        let samples = match (b, float) {
            (ImageBuffer::F32(g), _) => Samples::F32(g),
            (ImageBuffer::I32(g), _) if inv > 0.0 => Samples::I32(g, inv),
            (ImageBuffer::I16(g), _) if inv > 0.0 => Samples::I16(g, inv),
            (ImageBuffer::I32(g), Some(f)) => Samples::Float(g, f),
            (ImageBuffer::I16(g), Some(f)) if f.0 <= 16 => Samples::Float16(g, f),
            _ => return Err(Error::Unsupported("float modular samples".into())),
        };
        let o = origin.copied().unwrap_or(ChannelOrigin {
            x: 0,
            y: 0,
            width: b.width(),
            height: b.height(),
        });
        // The region never exceeds the allocation; clamp in case it ever does.
        let (bw, bh) = (o.width.min(b.width()), o.height.min(b.height()));
        let covers = o.x >= 0
            && o.y >= 0
            && o.x as u64 + w as u64 <= bw as u64
            && o.y as u64 + h as u64 <= bh as u64;
        Ok(Src {
            samples,
            ox: o.x,
            oy: o.y,
            bw,
            bh,
            covers,
        })
    }

    /// The buffer span behind image columns `x0..x0 + n` of image row `y`: (buffer row,
    /// first buffer column, offset into the image span, length). `None` when nothing of it
    /// lies inside the buffer.
    #[inline]
    fn span(&self, y: usize, x0: usize, n: usize) -> Option<(usize, usize, usize, usize)> {
        let by = y as i64 + self.oy;
        if by < 0 || by >= self.bh as i64 {
            return None;
        }
        let bx0 = (x0 as i64 + self.ox).max(0);
        let bx1 = (x0 as i64 + n as i64 + self.ox).min(self.bw as i64);
        (bx1 > bx0).then(|| {
            let skip = (bx0 - (x0 as i64 + self.ox)) as usize;
            (by as usize, bx0 as usize, skip, (bx1 - bx0) as usize)
        })
    }

    /// `out[i]` = sample `(x0 + i, y)` (unclamped, linear when `linear`), 0 outside the buffer.
    fn read(&self, y: usize, x0: usize, out: &mut [f32], linear: bool) {
        let n = out.len();
        let Some((by, bx, skip, len)) = self.span(y, x0, n) else {
            out.fill(0.0);
            return;
        };
        out[..skip].fill(0.0);
        out[skip + len..].fill(0.0);
        let out = &mut out[skip..skip + len];
        match &self.samples {
            Samples::F32(g) if linear => out.copy_from_slice(&g.get_row(by)[bx..bx + len]),
            Samples::F32(g) => {
                for (o, &v) in out.iter_mut().zip(&g.get_row(by)[bx..bx + len]) {
                    *o = v.clamp(0.0, 1.0);
                }
            }
            Samples::I32(g, inv) => {
                for (o, &v) in out.iter_mut().zip(&g.get_row(by)[bx..bx + len]) {
                    *o = v as f32 * inv;
                }
            }
            Samples::I16(g, inv) => {
                for (o, &v) in out.iter_mut().zip(&g.get_row(by)[bx..bx + len]) {
                    *o = v as f32 * inv;
                }
            }
            Samples::Float(g, (bits, exp)) => {
                for (o, &v) in out.iter_mut().zip(&g.get_row(by)[bx..bx + len]) {
                    let f = float_bits(v as u32, *bits, *exp);
                    *o = if linear { f } else { f.clamp(0.0, 1.0) };
                }
            }
            Samples::Float16(g, (bits, exp)) => {
                for (o, &v) in out.iter_mut().zip(&g.get_row(by)[bx..bx + len]) {
                    let f = float_bits(u32::from(v as u16), *bits, *exp);
                    *o = if linear { f } else { f.clamp(0.0, 1.0) };
                }
            }
        }
    }

    /// Row `y` of a covering `F32` buffer, `w` samples from image column 0.
    #[inline]
    fn f32_row(&self, y: usize, w: usize) -> Option<&'a [f32]> {
        match self.samples {
            Samples::F32(g) if self.covers => {
                let (bx, by) = (self.ox as usize, (y as i64 + self.oy) as usize);
                Some(&g.get_row(by)[bx..bx + w])
            }
            _ => None,
        }
    }

    /// The first `w` samples of row `y` as floats, unclamped (linear light can exceed 1).
    fn row_linear(&self, y: usize, w: usize, out: &mut [f32]) {
        self.read(y, 0, &mut out[..w], true);
    }

    /// The first `w` samples of row `y` as floats (nominal-range floats clamped to `[0, 1]`,
    /// as [`Src::accumulate`] reads them).
    fn row(&self, y: usize, w: usize, out: &mut [f32]) {
        self.read(y, 0, &mut out[..w], false);
    }

    /// Add `w` samples of row `y` starting at `x0`, `s` at a time, into `acc`.
    #[inline]
    fn accumulate(&self, y: usize, x0: usize, w: usize, s: usize, acc: &mut [f32]) {
        if self.covers {
            let (bx, by) = (
                (x0 as i64 + self.ox) as usize,
                (y as i64 + self.oy) as usize,
            );
            match &self.samples {
                Samples::F32(g) => {
                    return accumulate_row_clamped(&g.get_row(by)[bx..bx + w], s, acc);
                }
                Samples::I32(g, inv) => {
                    return accumulate_row(&g.get_row(by)[bx..bx + w], s, *inv, acc);
                }
                Samples::I16(g, inv) => {
                    return accumulate_row(&g.get_row(by)[bx..bx + w], s, *inv, acc);
                }
                Samples::Float(..) | Samples::Float16(..) => {}
            }
        }
        let mut row = vec![0.0; w];
        self.read(y, x0, &mut row, false);
        for (a, c) in acc.iter_mut().zip(row.chunks(s)) {
            *a += c.iter().sum::<f32>();
        }
    }
}

/// Sources for the colour channels (at most three) and alpha of `render`, in image
/// coordinates of size `size`: (sources, alpha is premultiplied).
fn sources<'a>(
    render: &'a crate::oxide::Render,
    image: &JxlImage,
    size: (usize, usize),
) -> Result<(Vec<Src<'a>>, Option<bool>)> {
    let meta = &image.image_header().metadata;
    let color = render.color_channels();
    let (ecs, ebufs) = render.extra_channels();
    let origins = render.channel_origins();
    let mut srcs = Vec::with_capacity(4);
    for (k, b) in color.iter().take(3).enumerate() {
        srcs.push(Src::new(b, meta.bit_depth, origins.get(k), size)?);
    }
    let mut alpha = None;
    if let Some(i) = ecs.iter().position(|e| e.is_alpha()) {
        let ec = &meta.ec_info[i];
        alpha = Some(ec.alpha_associated() == Some(true));
        let origin = origins.get(color.len() + i);
        srcs.push(Src::new(&ebufs[i], ec.bit_depth, origin, size)?);
    }
    Ok((srcs, alpha))
}

/// A float sample of `bits` bits with `exp` exponent bits (sign, exponent, mantissa from the
/// top; `2 <= exp <= 8`, `exp + 2 <= bits <= 32`), as stored in modular images. Subnormals
/// are exact; infinities stay infinite; NaN stays NaN.
fn float_bits(v: u32, bits: u32, exp: u32) -> f32 {
    if bits == 32 && exp == 8 {
        return f32::from_bits(v);
    }
    let man_bits = bits - exp - 1;
    let man = v & ((1u32 << man_bits) - 1);
    let e = (v >> man_bits) & ((1u32 << exp) - 1);
    let negative = (v >> (bits - 1)) & 1 == 1;
    let bias = (1i32 << (exp - 1)) - 1;
    let mag = if e == (1u32 << exp) - 1 {
        if man == 0 { f32::INFINITY } else { f32::NAN }
    } else if e == 0 {
        // Subnormal: man · 2^(1 − bias − man_bits).
        man as f32 * (2.0f32).powi(1 - bias - man_bits as i32)
    } else {
        (1.0 + man as f32 / (1u32 << man_bits) as f32) * (2.0f32).powi(e as i32 - bias)
    };
    if negative { -mag } else { mag }
}

/// f32 → IEEE half bits, rounding to nearest even; NaN must be filtered out first.
fn f32_to_f16(f: f32) -> u16 {
    let x = f.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let exp = ((x >> 23) & 0xff) as i32;
    let man = x & 0x7f_ffff;
    let e = exp - 127 + 15;
    if exp == 0xff || e >= 31 {
        return sign | 0x7c00;
    }
    let (v, rest, half) = if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = man | 0x80_0000;
        let shift = (14 - e) as u32;
        (m >> shift, m & ((1 << shift) - 1), 1u32 << (shift - 1))
    } else {
        (((e as u32) << 10) | (man >> 13), man & 0x1fff, 0x1000)
    };
    // A carry out of the mantissa correctly bumps the exponent (up to infinity).
    let v = v + u32::from(rest > half || (rest == half && v & 1 == 1));
    sign | v as u16
}

trait ToF32: Copy {
    fn f(self) -> f32;
}
impl ToF32 for f32 {
    #[inline(always)]
    fn f(self) -> f32 {
        self
    }
}
impl ToF32 for i32 {
    #[inline(always)]
    fn f(self) -> f32 {
        self as f32
    }
}
impl ToF32 for i16 {
    #[inline(always)]
    fn f(self) -> f32 {
        self as f32
    }
}

#[inline]
fn accumulate_row<T: ToF32>(row: &[T], s: usize, mul: f32, acc: &mut [f32]) {
    if s == 1 {
        for (a, &v) in acc.iter_mut().zip(row) {
            *a += v.f() * mul;
        }
    } else {
        for (a, c) in acc.iter_mut().zip(row.chunks(s)) {
            let mut t = 0.0;
            for &v in c {
                t += v.f();
            }
            *a += t * mul;
        }
    }
}

/// Like [`accumulate_row`] for nominal-range floats, clamping each sample to `[0, 1]` first so
/// that out-of-gamut values (common in lossy XYB output) average the way they display.
#[inline]
fn accumulate_row_clamped(row: &[f32], s: usize, acc: &mut [f32]) {
    if s == 1 {
        for (a, &v) in acc.iter_mut().zip(row) {
            *a += v.clamp(0.0, 1.0);
        }
    } else {
        for (a, c) in acc.iter_mut().zip(row.chunks(s)) {
            let mut t = 0.0;
            for &v in c {
                t += v.clamp(0.0, 1.0);
            }
            *a += t;
        }
    }
}

/// `out[3x + c] = q8(rows[c][x].clamp(0, 1))`, eight pixels at a time.
fn quantize_rgb_row(rows: [&[f32]; 3], out: &mut [u8]) {
    use std::simd::f32x8;
    use std::simd::num::SimdFloat;
    let w = out.len() / 3;
    let (zero, one) = (f32x8::splat(0.0), f32x8::splat(1.0));
    let (mul, half, max) = (f32x8::splat(255.0), f32x8::splat(0.5), f32x8::splat(255.0));
    let mut x = 0;
    while x + 8 <= w {
        let q = rows.map(|r| {
            let v = f32x8::from_slice(&r[x..x + 8]).simd_clamp(zero, one);
            (v * mul + half)
                .simd_clamp(zero, max)
                .cast::<u8>()
                .to_array()
        });
        let o = &mut out[x * 3..(x + 8) * 3];
        for i in 0..8 {
            o[3 * i] = q[0][i];
            o[3 * i + 1] = q[1][i];
            o[3 * i + 2] = q[2][i];
        }
        x += 8;
    }
    for x in x..w {
        for c in 0..3 {
            out[3 * x + c] = q8(rows[c][x].clamp(0.0, 1.0));
        }
    }
}

#[inline(always)]
fn q8(v: f32) -> u8 {
    // NaN maps to 0 through the saturating cast.
    (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

fn convert(
    render: &crate::oxide::Render,
    image: &JxlImage,
    (w, h): (usize, usize),
    scale: u32,
    pool: Option<&rayon::ThreadPool>,
) -> Result<Decoded> {
    let gray = render.color_channels().len() == 1;
    let (srcs, alpha) = sources(render, image, (w, h))?;
    let has_alpha = alpha.is_some();
    let premultiplied = alpha == Some(true);
    let layout = match (gray, has_alpha) {
        (true, false) => Layout::Gray,
        (false, false) => Layout::Rgb,
        (_, true) => Layout::Rgba,
    };
    let s = scale.max(1) as usize;
    let (ow, oh) = (w.div_ceil(s), h.div_ceil(s));
    let nch = layout.channels();
    // Reduced straight alpha is averaged weighted by alpha (colour under transparent pixels
    // must not bleed into the edge), which leaves colour × alpha sums: divided out below like
    // associated alpha.
    let weight = has_alpha && !premultiplied && s > 1;
    let divide = premultiplied || weight;
    let mut data = vec![0u8; ow * oh * nch];
    // `scratch`: reused across the rows of one band.
    let row_job = |oy: usize, out: &mut [u8], scratch: &mut Vec<f32>| {
        let y0 = oy * s;
        let y1 = (y0 + s).min(h);
        // Common case (full-size lossy RGB): quantise straight from the planes.
        if s == 1
            && layout == Layout::Rgb
            && let [Some(r), Some(g), Some(b)] = [0, 1, 2].map(|k| srcs[k].f32_row(y0, w))
        {
            quantize_rgb_row([r, g, b], &mut out[..w * 3]);
            return;
        }
        // Full size from integer (modular) planes: each row as floats, then the same
        // quantiser (no per-pixel averaging).
        if s == 1 && layout == Layout::Rgb {
            scratch.resize(3 * w, 0.0);
            let (r, rest) = scratch.split_at_mut(w);
            let (g, b) = rest.split_at_mut(w);
            srcs[0].row(y0, w, r);
            srcs[1].row(y0, w, g);
            srcs[2].row(y0, w, b);
            quantize_rgb_row([r, g, b], &mut out[..w * 3]);
            return;
        }
        // Per-source accumulators over output columns.
        scratch.clear();
        scratch.resize(ow * srcs.len(), 0.0);
        let acc = &mut scratch[..];
        if weight {
            let (mut arow, mut crow) = (vec![0.0f32; w], vec![0.0f32; w]);
            let ai = srcs.len() - 1;
            for y in y0..y1 {
                srcs[ai].row(y, w, &mut arow);
                for (x, &a) in arow.iter().enumerate() {
                    acc[ai * ow + x / s] += a.clamp(0.0, 1.0);
                }
                for (k, src) in srcs[..ai].iter().enumerate() {
                    src.row(y, w, &mut crow);
                    for (x, (&c, &a)) in crow.iter().zip(&arow).enumerate() {
                        acc[k * ow + x / s] += c * a.clamp(0.0, 1.0);
                    }
                }
            }
        } else {
            for y in y0..y1 {
                for (k, src) in srcs.iter().enumerate() {
                    src.accumulate(y, 0, w, s, &mut acc[k * ow..(k + 1) * ow]);
                }
            }
        }
        let ny = (y1 - y0) as f32;
        let last_w = w - (ow - 1) * s;
        for ox in 0..ow {
            let nx = if ox + 1 == ow { last_w } else { s } as f32;
            let inv = 1.0 / (nx * ny);
            let px = &mut out[ox * nch..(ox + 1) * nch];
            if gray {
                let g = acc[ox] * inv;
                match layout {
                    Layout::Gray => px[0] = q8(g),
                    _ => {
                        let mut a = acc[ow + ox] * inv;
                        let gv = if divide && a > 0.0 { g / a } else { g };
                        a = a.clamp(0.0, 1.0);
                        let q = q8(gv);
                        px[0] = q;
                        px[1] = q;
                        px[2] = q;
                        px[3] = q8(a);
                    }
                }
            } else {
                let (mut r, mut g, mut b) =
                    (acc[ox] * inv, acc[ow + ox] * inv, acc[2 * ow + ox] * inv);
                if layout == Layout::Rgba {
                    let a = acc[3 * ow + ox] * inv;
                    if divide && a > 0.0 {
                        r /= a;
                        g /= a;
                        b /= a;
                    }
                    px[3] = q8(a);
                }
                px[0] = q8(r);
                px[1] = q8(g);
                px[2] = q8(b);
            }
        }
    };
    let rows_per_task = (64 / s).max(1);
    match pool {
        Some(p) => p.install(|| {
            data.par_chunks_mut(ow * nch * rows_per_task)
                .enumerate()
                .for_each(|(t, band)| {
                    let mut scratch = Vec::new();
                    for (i, row) in band.chunks_mut(ow * nch).enumerate() {
                        row_job(t * rows_per_task + i, row, &mut scratch);
                    }
                })
        }),
        None => {
            let mut scratch = Vec::new();
            for (oy, row) in data.chunks_mut(ow * nch).enumerate() {
                row_job(oy, row, &mut scratch);
            }
        }
    }
    Ok(Decoded {
        width: ow as u32,
        height: oh as u32,
        scale: s as u32,
        layout,
        data,
        duration_ms: 0,
        complete: true,
    })
}

/// Sources for the inks (the three colour channels, then black) and the alpha of a CMYK
/// `render`, in image coordinates of size `size`.
fn cmyk_sources<'a>(
    render: &'a crate::oxide::Render,
    image: &JxlImage,
    size: (usize, usize),
) -> Result<(Vec<Src<'a>>, Option<Src<'a>>)> {
    let meta = &image.image_header().metadata;
    let color = render.color_channels();
    let (ecs, ebufs) = render.extra_channels();
    let origins = render.channel_origins();
    if color.len() < 3 {
        return Err(Error::Malformed(
            "CMYK without three colour channels".into(),
        ));
    }
    let extra = |i: usize| -> Result<Src<'a>> {
        let origin = origins.get(color.len() + i);
        Src::new(&ebufs[i], meta.ec_info[i].bit_depth, origin, size)
    };
    let black = ecs
        .iter()
        .position(|e| e.is_black())
        .ok_or_else(|| Error::Malformed("no black channel".into()))?;
    let mut inks = Vec::with_capacity(4);
    for (k, b) in color.iter().take(3).enumerate() {
        inks.push(Src::new(b, meta.bit_depth, origins.get(k), size)?);
    }
    inks.push(extra(black)?);
    let alpha = match ecs.iter().position(|e| e.is_alpha()) {
        Some(i) => Some(extra(i)?),
        None => None,
    };
    Ok((inks, alpha))
}

/// Row `y` of the four `inks` as ink amounts, C, M, Y, K per pixel (0 = no ink), into `out`
/// (`4 * w` bytes); `row` is scratch of `w` floats.
///
/// JPEG XL codes every ink as the colour channels are coded, 1.0 being no ink (as libjxl
/// hands them to its CMS, inverted); the converters take 0 as no ink.
fn ink_row(inks: &[Src<'_>], y: usize, w: usize, row: &mut [f32], out: &mut [u8]) {
    for (c, src) in inks.iter().enumerate() {
        src.row(y, w, row);
        for (o, &v) in out[c..].iter_mut().step_by(4).zip(&row[..w]) {
            *o = q8(1.0 - v.clamp(0.0, 1.0));
        }
    }
}

/// The naive CMYK formula (as `specular-jpeg` without a hook): each of R, G, B is its ink's
/// remaining light times black's, `rgba` gets opaque sRGB-encoded bytes.
fn cmyk_naive(ink: &[u8], rgba: &mut [u8]) {
    for (i, o) in ink.chunks_exact(4).zip(rgba.chunks_exact_mut(4)) {
        let k = 255 - u32::from(i[3]);
        let f = |c: u8| (((255 - u32::from(c)) * k + 127) / 255) as u8;
        o.copy_from_slice(&[f(i[0]), f(i[1]), f(i[2]), 255]);
    }
}

/// Like [`convert`] for a CMYK render: each source row's inks to sRGB (`conv`, else
/// [`cmyk_naive`]), then box-averaged to `1/scale` in the sRGB-encoded domain, straight alpha
/// weighting the colour as in [`convert`]. [`Layout::Rgba`] with alpha, else [`Layout::Rgb`].
fn convert_cmyk(
    render: &crate::oxide::Render,
    image: &JxlImage,
    (w, h): (usize, usize),
    scale: u32,
    pool: Option<&rayon::ThreadPool>,
    conv: Option<&CmykRowConverter>,
) -> Result<Decoded> {
    let (inks, alpha) = cmyk_sources(render, image, (w, h))?;
    let layout = if alpha.is_some() {
        Layout::Rgba
    } else {
        Layout::Rgb
    };
    let nch = layout.channels();
    let s = scale.max(1) as usize;
    let (ow, oh) = (w.div_ceil(s), h.div_ceil(s));
    let mut data = vec![0u8; ow * oh * nch];
    let rows_per_task = (64 / s).max(1);
    let band_job = |first: usize, band: &mut [u8]| {
        let mut row = vec![0.0f32; w];
        let mut arow = vec![1.0f32; w];
        let mut ink = vec![0u8; w * 4];
        let mut rgba = vec![0u8; w * 4];
        // Colour (weighted by alpha) and alpha sums per output column.
        let mut acc = vec![0.0f32; ow * 4];
        for (i, out) in band.chunks_mut(ow * nch).enumerate() {
            let y0 = (first + i) * s;
            let y1 = (y0 + s).min(h);
            acc.fill(0.0);
            for y in y0..y1 {
                ink_row(&inks, y, w, &mut row, &mut ink);
                match conv {
                    Some(c) => c(&ink, &mut rgba),
                    None => cmyk_naive(&ink, &mut rgba),
                }
                if let Some(a) = &alpha {
                    a.row(y, w, &mut arow);
                }
                if s == 1 {
                    for ((o, p), &a) in out
                        .chunks_exact_mut(nch)
                        .zip(rgba.chunks_exact(4))
                        .zip(&arow)
                    {
                        o[..3].copy_from_slice(&p[..3]);
                        if nch == 4 {
                            o[3] = q8(a.clamp(0.0, 1.0));
                        }
                    }
                    continue;
                }
                for (x, (p, &a)) in rgba.chunks_exact(4).zip(&arow).enumerate() {
                    let a = a.clamp(0.0, 1.0);
                    let acc = &mut acc[(x / s) * 4..(x / s) * 4 + 4];
                    for c in 0..3 {
                        acc[c] += f32::from(p[c]) * a;
                    }
                    acc[3] += a;
                }
            }
            if s == 1 {
                continue;
            }
            let ny = (y1 - y0) as f32;
            let last_w = w - (ow - 1) * s;
            for (ox, (px, acc)) in out
                .chunks_exact_mut(nch)
                .zip(acc.chunks_exact(4))
                .enumerate()
            {
                let nx = if ox + 1 == ow { last_w } else { s } as f32;
                let inv = if acc[3] > 0.0 { 1.0 / acc[3] } else { 0.0 };
                for c in 0..3 {
                    px[c] = (acc[c] * inv + 0.5).clamp(0.0, 255.0) as u8;
                }
                if nch == 4 {
                    px[3] = q8(acc[3] / (nx * ny));
                }
            }
        }
    };
    match pool {
        Some(p) => p.install(|| {
            data.par_chunks_mut(ow * nch * rows_per_task)
                .enumerate()
                .for_each(|(t, band)| band_job(t * rows_per_task, band))
        }),
        None => {
            if !data.is_empty() {
                band_job(0, &mut data);
            }
        }
    }
    Ok(Decoded {
        width: ow as u32,
        height: oh as u32,
        scale: s as u32,
        layout,
        data,
        duration_ms: 0,
        complete: true,
    })
}

/// Largest finite half.
const F16_MAX: f32 = 65504.0;

#[inline(always)]
fn q16(v: f32) -> u16 {
    (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16
}

/// Like [`convert`], to [`Layout::Rgba16`] ([`Out::Signal16`]: the signal clamped to
/// `[0, 1]`) or [`Layout::Rgba16F`] ([`Out::Half`]: unclamped linear light, averaged as such
/// when reducing): grey is replicated, alpha is opaque when absent. Straight alpha is weighted
/// the same way when reducing.
fn convert_wide(
    render: &crate::oxide::Render,
    image: &JxlImage,
    (w, h): (usize, usize),
    scale: u32,
    pool: Option<&rayon::ThreadPool>,
    out_kind: Out,
) -> Result<Decoded> {
    let half = match out_kind {
        Out::Half(mul) => Some(mul),
        _ => None,
    };
    let (srcs, alpha) = sources(render, image, (w, h))?;
    let has_alpha = alpha.is_some();
    let premultiplied = alpha == Some(true);
    let ncol = srcs.len() - usize::from(alpha.is_some());
    if ncol == 0 {
        return Err(Error::Malformed("no colour channels".into()));
    }
    let s = scale.max(1) as usize;
    let (ow, oh) = (w.div_ceil(s), h.div_ceil(s));
    let weight = has_alpha && !premultiplied && s > 1;
    let divide = premultiplied || weight;
    let mut data = vec![0u8; ow * oh * 8];
    let row_job = |oy: usize, out: &mut [u8]| {
        let y0 = oy * s;
        let y1 = (y0 + s).min(h);
        let mut acc = vec![0.0f32; ow * srcs.len()];
        // Colour as read for this output: the clamped signal, or linear light.
        let colour_row = |src: &Src, y: usize, row: &mut [f32]| match half {
            Some(_) => src.row_linear(y, w, row),
            None => src.row(y, w, row),
        };
        if weight {
            let (mut arow, mut crow) = (vec![0.0f32; w], vec![0.0f32; w]);
            let ai = srcs.len() - 1;
            for y in y0..y1 {
                srcs[ai].row(y, w, &mut arow);
                for (x, &a) in arow.iter().enumerate() {
                    acc[ai * ow + x / s] += a.clamp(0.0, 1.0);
                }
                for (k, src) in srcs[..ai].iter().enumerate() {
                    colour_row(src, y, &mut crow);
                    for (x, (&c, &a)) in crow.iter().zip(&arow).enumerate() {
                        acc[k * ow + x / s] += c * a.clamp(0.0, 1.0);
                    }
                }
            }
        } else if half.is_some() {
            let mut row = vec![0.0f32; w];
            for y in y0..y1 {
                for (k, src) in srcs.iter().enumerate() {
                    if k < ncol {
                        src.row_linear(y, w, &mut row);
                    } else {
                        src.row(y, w, &mut row);
                    }
                    let a = &mut acc[k * ow..(k + 1) * ow];
                    for (a, c) in a.iter_mut().zip(row.chunks(s)) {
                        *a += c.iter().sum::<f32>();
                    }
                }
            }
        } else {
            for y in y0..y1 {
                for (k, src) in srcs.iter().enumerate() {
                    src.accumulate(y, 0, w, s, &mut acc[k * ow..(k + 1) * ow]);
                }
            }
        }
        let ny = (y1 - y0) as f32;
        let last_w = w - (ow - 1) * s;
        for ox in 0..ow {
            let nx = if ox + 1 == ow { last_w } else { s } as f32;
            let inv = 1.0 / (nx * ny);
            let a = if has_alpha {
                (acc[ncol * ow + ox] * inv).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let ch = |k: usize| {
                let v = acc[k.min(ncol - 1) * ow + ox] * inv;
                if divide && a > 0.0 { v / a } else { v }
            };
            let px = match half {
                None => [q16(ch(0)), q16(ch(1)), q16(ch(2)), q16(a)],
                Some(mul) => {
                    let h = |v: f32| {
                        let v = v * mul;
                        f32_to_f16(if v.is_nan() {
                            0.0
                        } else {
                            v.clamp(-F16_MAX, F16_MAX)
                        })
                    };
                    [h(ch(0)), h(ch(1)), h(ch(2)), f32_to_f16(a)]
                }
            };
            for (o, v) in out[ox * 8..ox * 8 + 8].chunks_exact_mut(2).zip(px) {
                o.copy_from_slice(&v.to_ne_bytes());
            }
        }
    };
    let rows_per_task = (64 / s).max(1);
    match pool {
        Some(p) => p.install(|| {
            data.par_chunks_mut(ow * 8 * rows_per_task)
                .enumerate()
                .for_each(|(t, band)| {
                    for (i, row) in band.chunks_mut(ow * 8).enumerate() {
                        row_job(t * rows_per_task + i, row);
                    }
                })
        }),
        None => {
            for (oy, row) in data.chunks_mut(ow * 8).enumerate() {
                row_job(oy, row);
            }
        }
    }
    Ok(Decoded {
        width: ow as u32,
        height: oh as u32,
        scale: s as u32,
        layout: if half.is_some() {
            Layout::Rgba16F
        } else {
            Layout::Rgba16
        },
        data,
        duration_ms: 0,
        complete: true,
    })
}
