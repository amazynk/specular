//! `specular-png`: a fast PNG / APNG decoder.
//!
//! * **Own inflate.** [`inflate`](crate) is a safe-Rust DEFLATE decoder after libdeflate's
//!   design (64-bit branchless bit-buffer refills, combined length/extra-bit decode tables,
//!   multi-literal fast loop, word-wise match copies), made *resumable* so that it can work in
//!   a small sliding window.
//! * **Fused pipeline.** Rows are inflated into a window of a few hundred KiB, unfiltered while
//!   they are still in cache and converted straight into the output layout; the filtered image
//!   is never materialised. 8-bit RGBA and gray rows are unfiltered directly into the output.
//! * **SIMD unfiltering.** Up is a vector add; Sub/Average/Paeth work a pixel per `std::simd`
//!   vector (16-bit lanes) for 3..=8 bytes per pixel.
//! * **Parallelism.** For images of at least [`AUTO_PARALLEL_MIN_PIXELS`], inflate runs on its
//!   own thread and unfiltering + conversion + downscaling on the calling one (a two-stage
//!   pipeline). Files with Apple's `iDOT` chunk (macOS/iOS screenshots) have independently
//!   compressed horizontal parts; those are inflated in parallel.
//! * **Scaled decode.** [`DecodeOptions::scale`] box-filters while rows stream out (in linear
//!   light, with premultiplied alpha), so a 1/8 decode needs only 1/64 of the output memory. For
//!   Adam7-interlaced files, [`decode_adam7_preview`] decodes just the first pass(es): a
//!   nearest-sampled 1/8 (1/4, 1/2) image from 1/64 (1/16, 1/4) of the compressed data.
//! * **Output layouts.** [`Output::Auto8`] picks the smallest 8-bit layout that loses nothing:
//!   one gray plane, three RGB planes (opaque colour) or interleaved RGBA. [`Output::Auto16`]
//!   keeps 16-bit samples in the smallest 16-bit layout: grey, grey + alpha or RGBA (APNG
//!   frames: RGBA). [`Output::Rgba8`] and [`Output::Rgba16`] force a layout; 16-bit samples are
//!   kept by the 16-bit outputs only (otherwise rounded, `round(v * 255 / 65535)`).
//! * **APNG.** [`PngInfo::frames`] lists the frames; [`decode_frame`] decodes one frame's
//!   sub-image and [`Animator`] composites frames (dispose none / background / previous,
//!   blend source / over) one at a time, holding only the canvas (plus one saved copy for
//!   "previous" disposal). [`decode`] returns the default image.
//! * **Metadata.** PLTE/tRNS, gAMA, cHRM, sRGB, iCCP (decompressed ICC profile), eXIf, pHYs and
//!   the PNG third-edition HDR chunks cICP, mDCv and cLLi are exposed in [`PngInfo`]; no colour
//!   management is applied.
//! * **Robustness.** Malformed input never panics. Truncated or corrupt image data yields the
//!   rows decoded so far with [`Image::truncated`] set; the rest is mid-grey (opaque). An
//!   unknown filter type ends the image the same way.
//!
//! # Checksums
//! CRCs and the zlib Adler-32 are **not verified by default** (like browsers, which also ignore
//! them for image data): the inflater detects structural damage on its own, a viewer wants to
//! show whatever is decodable, and corrupt-but-structurally-valid data is rare. Checking costs
//! one extra pass over the compressed data (CRC) and the decompressed data (Adler-32), a few
//! percent of decode time. [`DecodeOptions::verify_checksums`] turns both on; failures are then
//! errors.
//!
//! # Memory
//! The output (`width * height * bytes per pixel / scale^2`) plus a window of
//! `32 KiB + max(2 rows, 256 KiB)`, two scratch rows, and (pipeline) up to four in-flight
//! batches of 256 KiB. Files with many IDAT chunks are first concatenated (compressed size).
//! Adam7 files at reduced scale are decoded at full resolution first. `iDOT` parts other than
//! the first are inflated into whole buffers (their filtered size).

#![feature(portable_simd)]

mod apng;
mod chunk;
mod convert;
mod crc;
mod decode;
mod encode;
mod error;
mod filter;
mod inflate;
mod scale;

use std::ops::Range;
use std::sync::atomic::AtomicBool;

pub use apng::{Animator, CompositedFrame, decode_frame};
#[doc(hidden)]
pub use decode::bench_inflate;
pub use decode::{decode, decode_adam7_preview, decode_scales, downscale, set_wavefront_min};
pub use encode::{EncodeError, EncodeOptions, encode, encode_with, encode_with_text};
pub use error::{PngError, Result};
pub use inflate::InflateError;

/// PNG colour type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorType {
    Gray,
    Rgb,
    Indexed,
    GrayAlpha,
    Rgba,
}

impl ColorType {
    fn from_code(c: u8) -> Option<ColorType> {
        Some(match c {
            0 => ColorType::Gray,
            2 => ColorType::Rgb,
            3 => ColorType::Indexed,
            4 => ColorType::GrayAlpha,
            6 => ColorType::Rgba,
            _ => return None,
        })
    }
    fn allowed_depths(self) -> &'static [u8] {
        match self {
            ColorType::Gray => &[1, 2, 4, 8, 16],
            ColorType::Indexed => &[1, 2, 4, 8],
            _ => &[8, 16],
        }
    }
    /// Samples per pixel in the file.
    pub fn channels(self) -> usize {
        match self {
            ColorType::Gray | ColorType::Indexed => 1,
            ColorType::GrayAlpha => 2,
            ColorType::Rgb => 3,
            ColorType::Rgba => 4,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            ColorType::Gray => "gray",
            ColorType::Rgb => "RGB",
            ColorType::Indexed => "indexed",
            ColorType::GrayAlpha => "gray+alpha",
            ColorType::Rgba => "RGBA",
        }
    }
}

/// tRNS contents. Colour keys are in the image's bit depth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Transparency {
    Gray(u16),
    Rgb(u16, u16, u16),
    /// Alpha of the first palette entries (the rest are opaque).
    Palette(Vec<u8>),
}

/// cHRM chromaticities, times 100 000.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chromaticities {
    pub white: (u32, u32),
    pub red: (u32, u32),
    pub green: (u32, u32),
    pub blue: (u32, u32),
}

/// cICP: coding-independent code points (ITU-T H.273), e.g. primaries 9 / transfer 16 for
/// BT.2100 PQ. When present it takes precedence over iCCP / sRGB / gAMA / cHRM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cicp {
    pub color_primaries: u8,
    pub transfer_function: u8,
    pub matrix_coefficients: u8,
    pub full_range: bool,
}

/// mDCv: mastering display colour volume (chromaticities × 50 000, luminance × 10 000 cd/m²).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MasteringDisplay {
    pub primaries: [(u16, u16); 3],
    pub white: (u16, u16),
    pub max_luminance: u32,
    pub min_luminance: u32,
}

/// cLLi: content light level (× 10 000 cd/m²).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentLightLevel {
    pub max_cll: u32,
    pub max_fall: u32,
}

/// acTL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimationInfo {
    /// Frames as declared (the usable count is `PngInfo::frames.len()`).
    pub num_frames: u32,
    /// 0 = loop forever.
    pub num_plays: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisposeOp {
    None,
    Background,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendOp {
    Source,
    Over,
}

/// One APNG frame (fcTL plus where its data is).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
    pub delay_num: u16,
    /// 0 means 100 (APNG spec).
    pub delay_den: u16,
    pub dispose: DisposeOp,
    pub blend: BlendOp,
    /// The frame is the default image (its data is the IDAT chunks).
    pub from_idat: bool,
    /// Byte ranges of the frame's zlib data in the file (fdAT payloads without the sequence
    /// number, or IDAT payloads).
    pub data: Vec<Range<usize>>,
}

impl FrameInfo {
    /// Delay in seconds.
    pub fn delay_secs(&self) -> f64 {
        let den = if self.delay_den == 0 {
            100
        } else {
            self.delay_den
        };
        f64::from(self.delay_num) / f64::from(den)
    }
}

/// A part of an `iDOT`-split image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdotPart {
    pub first_row: u32,
    pub rows: u32,
    /// File offset of the part's first IDAT chunk (its length field).
    pub file_offset: usize,
}

/// Header and metadata. Ranges index into the buffer passed to [`read_info`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PngInfo {
    pub width: u32,
    pub height: u32,
    /// Bits per sample (per palette index for indexed images): 1, 2, 4, 8 or 16.
    pub bit_depth: u8,
    pub color_type: ColorType,
    /// Adam7.
    pub interlaced: bool,
    pub palette: Option<Vec<[u8; 3]>>,
    pub transparency: Option<Transparency>,
    /// gAMA, times 100 000 (45455 = 1/2.2).
    pub gamma: Option<u32>,
    pub chromaticities: Option<Chromaticities>,
    /// sRGB rendering intent.
    pub srgb_intent: Option<u8>,
    /// Decompressed iCCP profile.
    pub icc_profile: Option<Vec<u8>>,
    pub icc_name: Option<String>,
    pub cicp: Option<Cicp>,
    pub mastering_display: Option<MasteringDisplay>,
    pub content_light_level: Option<ContentLightLevel>,
    /// eXIf payload (a TIFF structure).
    pub exif: Option<Range<usize>>,
    /// IDAT payloads, in order.
    pub idat: Vec<Range<usize>>,
    /// A chunk after the IDAT chunks (or IEND) was seen: the image data is complete.
    pub idat_complete: bool,
    /// acTL, if the file is an APNG with at least one usable frame.
    pub animation: Option<AnimationInfo>,
    /// APNG frames in display order (empty for still images).
    pub frames: Vec<FrameInfo>,
    /// The default image (IDAT) is the first animation frame.
    pub default_image_is_frame: bool,
    /// Apple `iDOT` parts.
    pub idot: Option<Vec<IdotPart>>,
    /// pHYs in pixels per metre (x, y), when the unit is metres.
    pub pixels_per_unit: Option<(u32, u32)>,
}

impl PngInfo {
    /// Whether decoded pixels can be non-opaque.
    pub fn has_alpha(&self) -> bool {
        match self.color_type {
            ColorType::GrayAlpha | ColorType::Rgba => true,
            ColorType::Indexed => matches!(&self.transparency,
                Some(Transparency::Palette(a)) if a.iter().any(|&a| a != 255)),
            _ => self.transparency.is_some(),
        }
    }
    /// The eXIf payload.
    pub fn exif<'a>(&self, data: &'a [u8]) -> Option<&'a [u8]> {
        data.get(self.exif.clone()?)
    }
    /// The layout [`Output::Auto8`] decodes to.
    pub fn auto_layout(&self) -> Layout {
        if self.has_alpha() {
            return Layout::Rgba8;
        }
        match self.color_type {
            ColorType::Gray => Layout::Gray8,
            ColorType::Indexed
                if self
                    .palette
                    .as_ref()
                    .is_some_and(|p| p.iter().all(|c| c[0] == c[1] && c[1] == c[2])) =>
            {
                Layout::Gray8
            }
            _ => Layout::Rgb8Planar,
        }
    }
    /// The layout [`Output::Auto16`] decodes to: 16-bit grey (with alpha for a grey + alpha
    /// image or a grey tRNS key) stays grey, everything else is [`Layout::Rgba16`].
    pub fn auto_layout16(&self) -> Layout {
        match self.color_type {
            ColorType::Gray if self.transparency.is_some() => Layout::GrayAlpha16,
            ColorType::Gray => Layout::Gray16,
            ColorType::GrayAlpha => Layout::GrayAlpha16,
            _ => Layout::Rgba16,
        }
    }
    /// Short description, e.g. "RGB 8-bit, interlaced".
    pub fn coding(&self) -> String {
        format!(
            "{} {}-bit{}{}",
            self.color_type.name(),
            self.bit_depth,
            if self.interlaced { ", interlaced" } else { "" },
            if self.animation.is_some() {
                format!(", {} frames", self.frames.len())
            } else {
                String::new()
            }
        )
    }
}

/// Decompresses a whole zlib stream (a zTXt or iCCP payload), giving up beyond `max_out`
/// bytes.
pub fn inflate_zlib(input: &[u8], max_out: usize) -> std::result::Result<Vec<u8>, InflateError> {
    inflate::zlib_to_vec(input, max_out)
}

/// Decompresses the first member of a gzip stream (RFC 1952: a header, raw DEFLATE, then a
/// CRC-32 and length trailer, both checked), giving up beyond `max_out` bytes.
pub fn inflate_gzip(input: &[u8], max_out: usize) -> std::result::Result<Vec<u8>, InflateError> {
    inflate::gzip_to_vec(input, max_out)
}

/// Decompresses every member of a gzip stream (each member's CRC-32 and length checked,
/// NUL padding after the last allowed), giving up beyond `max_out` bytes; `hint` is the
/// expected output length, when known, to allocate once.
pub fn inflate_gzip_members(
    input: &[u8],
    max_out: usize,
    hint: Option<usize>,
) -> std::result::Result<Vec<u8>, InflateError> {
    inflate::gzip_members_to_vec(input, max_out, hint)
}

/// Decompresses only the first `n` bytes of a gzip stream (fewer if it is shorter), for
/// reading a header inside a `.gz` file without inflating the rest.
pub fn inflate_gzip_prefix(input: &[u8], n: usize) -> std::result::Result<Vec<u8>, InflateError> {
    inflate::gzip_prefix(input, n)
}

/// Decompresses a whole raw DEFLATE stream (RFC 1951, no zlib header), giving up beyond
/// `max_out` bytes.
pub fn inflate_raw(input: &[u8], max_out: usize) -> std::result::Result<Vec<u8>, InflateError> {
    inflate::raw_to_vec(input, max_out)
}

/// Decompresses a whole raw DEFLATE stream compressed against a preset dictionary (zlib's
/// `deflateSetDictionary` without the zlib wrapper; Flash Screen Video 2's primed blocks),
/// giving up beyond `max_out` bytes.
pub fn inflate_raw_with_dictionary(
    input: &[u8],
    dict: &[u8],
    max_out: usize,
) -> std::result::Result<Vec<u8>, InflateError> {
    inflate::raw_with_dictionary_to_vec(input, dict, max_out)
}

/// Decompresses only the first `n` bytes of a raw DEFLATE stream (fewer if the stream is
/// shorter), for reading a header out of a compressed container member without inflating
/// the rest.
pub fn inflate_raw_prefix(input: &[u8], n: usize) -> std::result::Result<Vec<u8>, InflateError> {
    let limit = n.saturating_add(inflate::MAX_MATCH);
    let mut out = vec![0u8; limit.saturating_add(inflate::OUT_SLACK)];
    let mut pos = 0;
    let mut inf = inflate::Inflater::new(input);
    // One call either fills past `n` (OutputFull leaves < MAX_MATCH of room before `limit`)
    // or finishes the stream.
    inf.inflate(&mut out, &mut pos, limit)?;
    out.truncate(pos.min(n));
    Ok(out)
}

/// Read header information and metadata (walks all chunks; no image data is decompressed).
pub fn read_info(data: &[u8]) -> Result<PngInfo> {
    chunk::parse(data, false)
}

/// What to decode into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Output {
    /// [`PngInfo::auto_layout`]: gray, planar RGB or RGBA, 8 bits.
    #[default]
    Auto8,
    Rgba8,
    /// Interleaved RGBA with native-endian `u16` samples.
    Rgba16,
    /// [`PngInfo::auto_layout16`]: native-endian `u16` grey, grey + alpha or RGBA.
    Auto16,
}

/// Layout of a decoded [`Image`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// One 8-bit plane.
    Gray8,
    /// Three 8-bit planes R, G, B.
    Rgb8Planar,
    /// One interleaved RGBA8 plane (straight alpha).
    Rgba8,
    /// One interleaved RGBA plane of native-endian u16 samples (8 bytes per pixel).
    Rgba16,
    /// One plane of native-endian u16 grey (2 bytes per pixel).
    Gray16,
    /// One interleaved plane of native-endian u16 grey and alpha (4 bytes per pixel, straight
    /// alpha).
    GrayAlpha16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plane {
    pub width: u32,
    pub height: u32,
    /// Row stride in bytes.
    pub stride: usize,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Size in output (scaled) pixels: `ceil(W / scale) x ceil(H / scale)`.
    pub width: u32,
    pub height: u32,
    pub layout: Layout,
    pub planes: Vec<Plane>,
    /// The image data ended early or was corrupt; missing rows are mid-grey.
    pub truncated: bool,
}

impl Image {
    /// Interleaved RGBA8 copy (for tests and simple consumers).
    pub fn to_rgba8(&self) -> Vec<u8> {
        let n = self.width as usize * self.height as usize;
        let mut out = Vec::with_capacity(n * 4);
        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                let at =
                    |p: usize, b: usize| self.planes[p].data[y * self.planes[p].stride + x * b];
                match self.layout {
                    Layout::Gray8 => {
                        let g = at(0, 1);
                        out.extend_from_slice(&[g, g, g, 255]);
                    }
                    Layout::Rgb8Planar => {
                        out.extend_from_slice(&[at(0, 1), at(1, 1), at(2, 1), 255])
                    }
                    Layout::Rgba8 => {
                        let o = y * self.planes[0].stride + x * 4;
                        out.extend_from_slice(&self.planes[0].data[o..o + 4]);
                    }
                    Layout::Rgba16 | Layout::Gray16 | Layout::GrayAlpha16 => {
                        let (n, ch): (usize, [usize; 4]) = match self.layout {
                            Layout::Gray16 => (2, [0, 0, 0, 4]),
                            Layout::GrayAlpha16 => (4, [0, 0, 0, 1]),
                            _ => (8, [0, 1, 2, 3]),
                        };
                        let o = y * self.planes[0].stride + x * n;
                        let d = &self.planes[0].data[o..o + n];
                        for c in ch {
                            out.push(if c * 2 >= n {
                                255
                            } else {
                                let v = u16::from_ne_bytes([d[c * 2], d[c * 2 + 1]]);
                                convert::r16((v >> 8) as u8, v as u8)
                            });
                        }
                    }
                }
            }
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Parallelism {
    /// A two-stage pipeline (and parallel iDOT parts) for images of at least
    /// [`AUTO_PARALLEL_MIN_PIXELS`]; a single thread otherwise.
    #[default]
    Auto,
    Single,
    /// Always use helper threads (where the file allows it).
    Threads,
}

/// Whether a threaded decode unfilters rows in parallel column segments (a wavefront) or row
/// by row. The output is identical; the wavefront costs about 50 % more CPU time for about
/// half the wall time where unfiltering, not inflating, limits the decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Wavefront {
    /// A fixed gate: at least 32 MiB unfiltered and compressed at least 8:1. Applications
    /// that measure the machine (Specular's path selector) choose `On` or `Off` instead.
    #[default]
    Auto,
    /// Whenever the decode is threaded and has at least two threads.
    On,
    /// Row by row.
    Off,
}

/// Below this many pixels `Parallelism::Auto` decodes on the calling thread.
pub const AUTO_PARALLEL_MIN_PIXELS: u64 = 1_000_000;

#[derive(Clone, Copy, Debug)]
pub struct DecodeOptions<'a> {
    pub output: Output,
    /// Box-filter downscale factor (1 = full size; any value >= 1).
    pub scale: u32,
    pub parallelism: Parallelism,
    /// Wavefront or row-by-row unfiltering in threaded decodes.
    pub wavefront: Wavefront,
    /// Verify chunk CRCs and the Adler-32 of the image data (see the crate docs).
    pub verify_checksums: bool,
    /// Checked every few rows; when set the decode returns [`PngError::Cancelled`].
    pub cancel: Option<&'a AtomicBool>,
}

impl Default for DecodeOptions<'_> {
    fn default() -> Self {
        DecodeOptions {
            output: Output::Auto8,
            scale: 1,
            parallelism: Parallelism::Auto,
            wavefront: Wavefront::Auto,
            verify_checksums: false,
            cancel: None,
        }
    }
}
