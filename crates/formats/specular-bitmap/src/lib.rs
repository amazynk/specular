//! Fast decoders for Specular's simple raster formats.
//!
//! | Module | Formats | Access |
//! |---|---|---|
//! | [`gif`] | GIF87a/89a: own LZW decoder; first frame, frame table, compositing [`gif::Animator`] | index buffer, random rows |
//! | [`bmp`] | BMP/DIB: 1/2/4/8-bit palette, RLE4/RLE8, 16/24/32-bit, bit fields, V1–V5 headers | uncompressed: random rows |
//! | [`ico`] | ICO and CUR: largest/deepest entry; BMP entries decoded here, PNG entries returned as a byte range | as BMP |
//! | [`tga`] | Truevision TGA: colour-mapped, true-colour, grey, RLE | uncompressed: random rows |
//! | [`qoi`] | QOI | sequential (streamed into the scaler) |
//! | [`pnm`] | PBM/PGM/PPM (P1–P6), PAM (P7), 1–16-bit samples (deeper than 8 bits also as 16-bit output, [`Decode::decode_wide`]) | binary: random rows |
//! | [`ani`] | Windows animated cursors (RIFF `ACON`): frame and step table; frames are ICO/CUR | — |
//!
//! Every decoder parses its header into a small struct that does not borrow the file and
//! decodes from the file bytes on request: any integer scale 1..=8 (box filter in linear light
//! with premultiplied alpha, fused into the decode so the full-resolution image is never
//! materialised unless the format forces it) and any region of the scaled image. Formats with
//! random row access decode only the rows a region needs, split across threads with `rayon`.
//!
//! Robustness: no input panics. Sizes are capped by [`MAX_DIMENSION`] and [`MAX_PIXELS`];
//! truncated pixel data yields a [`Image::truncated`] image with the missing part grey.

#![feature(portable_simd)]

pub mod ani;
pub mod bmp;
pub mod gif;
pub mod ico;
mod lzw;
pub mod pnm;
mod px;
pub mod qoi;
mod scale;
pub mod tga;

/// Largest width or height accepted.
pub const MAX_DIMENSION: u32 = 1 << 18;
/// Largest pixel count accepted (512 MP: 2 GiB as RGBA).
pub const MAX_PIXELS: u64 = 1 << 29;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A valid file using a feature this decoder does not implement.
    Unsupported(String),
    /// The header (or the structure needed to find the pixels) is cut short.
    Truncated(String),
    Malformed(String),
    /// Dimensions over [`MAX_DIMENSION`] / [`MAX_PIXELS`].
    TooLarge,
    /// The region lies outside the scaled image or the scale is not in 1..=8.
    BadRequest(String),
    Cancelled,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::TooLarge => write!(f, "image too large"),
            Error::BadRequest(s) => write!(f, "bad request: {s}"),
            Error::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Interleaved pixel layout of an [`Image`]. The 16-bit formats hold native-endian `u16`
/// samples and come only from [`Decode::decode_wide`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Gray8,
    /// Straight (not premultiplied) alpha.
    Rgba8,
    Gray16,
    /// Grey and straight alpha.
    GrayAlpha16,
    /// Straight alpha (65535 when the image has none).
    Rgba16,
}

impl PixelFormat {
    /// Samples per pixel.
    pub fn channels(self) -> usize {
        match self {
            PixelFormat::Gray8 | PixelFormat::Gray16 => 1,
            PixelFormat::GrayAlpha16 => 2,
            PixelFormat::Rgba8 | PixelFormat::Rgba16 => 4,
        }
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Gray8 => 1,
            PixelFormat::Gray16 => 2,
            PixelFormat::Rgba8 | PixelFormat::GrayAlpha16 => 4,
            PixelFormat::Rgba16 => 8,
        }
    }
}

/// Decoded pixels: `width × height` tightly packed rows.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
    /// Pixel data ended early; the missing part is grey.
    pub truncated: bool,
}

/// A rectangle in *scaled* image coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// What to decode.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Downscale factor, 1..=8. The scaled size is `ceil(width / scale) × ceil(height / scale)`.
    pub scale: u32,
    /// Part of the scaled image; clipped to it. `None` = everything.
    pub region: Option<Region>,
    /// Allow several threads.
    pub parallel: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            scale: 1,
            region: None,
            parallel: true,
        }
    }
}

impl Options {
    pub fn scaled(scale: u32) -> Options {
        Options {
            scale,
            ..Options::default()
        }
    }
}

/// The colour model of the stored pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    /// Grey levels (with or without alpha), including 1-bit black and white.
    Grey,
    /// Red, green and blue samples (with or without alpha).
    Rgb,
    /// Indices into a colour table.
    Palette,
}

/// How a format stores transparency ([`Info::alpha`]); decoded pixels are straight alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaKind {
    /// An alpha channel (or alpha in the colour table), not premultiplied.
    Straight,
    /// An alpha channel premultiplied into the colour (TGA attribute type 4).
    Premultiplied,
    /// A transparent index, an icon's 1-bit AND mask or canvas a GIF's first frame leaves
    /// uncovered: no alpha channel.
    Key,
}

/// Header facts.
#[derive(Clone, Debug)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    /// Where the transparency of [`Info::has_alpha`] comes from (for the info panel).
    pub alpha: AlphaKind,
    /// Decodes to [`PixelFormat::Gray8`].
    pub gray: bool,
    /// How the colours are stored (for the info panel).
    pub model: Model,
    /// Bits per sample (or per pixel for palette images) as stored.
    pub bit_depth: u8,
    /// Short technical description, e.g. "8-bit palette, RLE".
    pub coding: String,
    /// Rows can be decoded independently, so regions and scales cost only the pixels they
    /// touch. Otherwise a decode reads the pixel data from the start.
    pub random_access: bool,
}

/// Cancellation check, polled between row bands.
pub type Cancel<'a> = &'a (dyn Fn() -> bool + Sync);

/// A parsed image header that can decode its pixels from the file bytes.
pub trait Decode: Send + Sync {
    fn info(&self) -> &Info;
    /// Decode `opts` from `data` (the same bytes the header was parsed from).
    fn decode(&self, data: &[u8], opts: &Options, cancel: Cancel) -> Result<Image>;
    /// The 16-bit format [`Decode::decode_wide`] gives, for images deeper than 8 bits.
    fn wide_format(&self) -> Option<PixelFormat> {
        None
    }
    /// [`Decode::decode`], keeping samples deeper than 8 bits at 16 bits
    /// ([`Decode::wide_format`]); the 8-bit decode for everything else.
    fn decode_wide(&self, data: &[u8], opts: &Options, cancel: Cancel) -> Result<Image> {
        self.decode(data, opts, cancel)
    }
}

/// The value missing pixels get.
pub const GREY: u8 = 128;

/// Refuses an image the file cannot hold: `have` bytes give at most `per_byte` pixels each
/// (the format's best compression), with room for a file cut short to a sixteenth (its
/// missing rows show grey). Up to 16 M pixels always pass. Keeps a header-only file from
/// allocating gigabytes.
pub(crate) fn proven(have: usize, w: u32, h: u32, per_byte: u64) -> Result<()> {
    let pixels = u64::from(w) * u64::from(h);
    if pixels > 1 << 24 && pixels / per_byte.max(1) / 16 > have as u64 {
        return Err(Error::Truncated(format!(
            "{w}×{h} pixels from {have} bytes"
        )));
    }
    Ok(())
}

pub(crate) fn check_dims(w: u32, h: u32) -> Result<()> {
    if w == 0 || h == 0 {
        return Err(Error::Malformed(format!("empty image {w}×{h}")));
    }
    if w > MAX_DIMENSION || h > MAX_DIMENSION || w as u64 * h as u64 > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    Ok(())
}

pub(crate) fn u16le(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}
pub(crate) fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}
pub(crate) fn u32be(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

pub use gif::{Animation as GifAnimation, Gif};
