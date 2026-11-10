//! Decoders for three more legacy raster formats.
//!
//! | Module | Format | Coverage |
//! |---|---|---|
//! | [`xwd`] | X Window Dump | X10 (version 6) and X11 (version 7), either byte order; XYBitmap, XYPixmap and ZPixmap; 1–32 bits per pixel; every visual class, colour maps and channel masks |
//! | [`tim`] | PlayStation TIM | 4- and 8-bit with colour tables (CLUTs), 16-bit 5:5:5, 24-bit |
//! | [`miff`] | ImageMagick MIFF | DirectClass and PseudoClass; sRGB, grey and CMYK, with or without alpha; 8/16/32-bit integers, 16/32/64-bit floats; no compression, RLE, Zip and BZip2; every image of the file as a page |
//! | [`gpl`], [`aco`], [`act`], [`ase`] | GIMP palettes, Photoshop swatches, Adobe colour tables, Adobe Swatch Exchange | every colour as a square of a swatch sheet ([`swatch`]); names, groups and stored values (CMYK, Lab, HSB) in the info |
//! | [`aseprite`] | Aseprite sprites | RGBA, grey and indexed; every frame (linked cels), layer blend modes and opacity, groups, tilemaps; tags and layers in the info |
//! | [`ggr`] | GIMP gradients | every blending function, RGB and HSV colouring, foreground/background end points |
//! | [`gimp`] | GIMP brushes, animated brushes, patterns | brush versions 1 and 2, grey and RGBA; pipe cells as pages; patterns of 1–4 channels |
//! | [`msp`] | Microsoft Paint | versions 1 and 2 (run-length rows) |
//! | [`gd`] | libgd GD and GD2 | GD 1.x and 2.x, palette and true colour; GD2 raw and zlib chunks |
//! | [`pxr`] | Pixar picture | 8-bit RGB, untiled |
//! | [`labeye`] | LabEye IM | 1-bit, grey, palette, LA, RGB(A/X), CMYK, YCC, 16/32-bit integer and float grey; every image as a page |
//! | [`imt`] | IM Tools | 8-bit grey |
//! | [`xvthumb`] | XV thumbnails (P7 332) | 3:3:2 colour |
//! | [`mcidas`] | McIDAS AREA | 1-, 2- and 4-byte elements, either byte order; each band a page |
//! | [`pgx`] | JPEG 2000 PGX | 1–16-bit and 32-bit, signed or unsigned, either byte order |
//!
//! Each module has `probe(head)`, `info(data, page)` and `decode(data, page)`.
//!
//! Robustness: no input panics. Sizes are capped by [`MAX_DIMENSION`] and [`MAX_PIXELS`] and
//! allocation follows only checked header sizes; pixel data that ends early yields an image
//! with [`Image::truncated`] set and the missing part grey.

pub mod aco;
pub mod act;
pub mod ase;
pub mod aseprite;
mod colorspace;
pub mod gd;
pub mod ggr;
pub mod gimp;
pub mod gpl;
pub mod imt;
pub mod labeye;
pub mod mcidas;
pub mod miff;
pub mod msp;
pub mod pgx;
pub mod pxr;
pub mod swatch;
pub mod tim;
pub mod xvthumb;
pub mod xwd;

/// Largest width or height accepted.
pub const MAX_DIMENSION: u32 = 1 << 16;
/// Largest pixel count accepted (256 MP).
pub const MAX_PIXELS: u64 = 1 << 28;
/// The value missing pixels get (8-bit).
pub const GREY: u8 = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not this format.
    NotRecognised,
    /// A valid file using a feature this decoder does not implement (a plain-language reason).
    Unsupported(String),
    /// The header (or the structure needed to find the pixels) is cut short.
    Truncated(String),
    Malformed(String),
    /// Dimensions over [`MAX_DIMENSION`] / [`MAX_PIXELS`].
    TooLarge,
    /// Under the caps, but more than the memory budget has free now.
    NeedsMemory(specular_core::NeedsMemory),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotRecognised => write!(f, "format not recognised"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::TooLarge => write!(f, "image too large"),
            Error::NeedsMemory(n) => write!(f, "{n}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// The colour model of the stored pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Grey,
    Rgb,
    Palette,
    YCbCr,
    /// One bit per pixel.
    Bilevel,
}

/// Header facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    /// Images in the file shown as pages (MIFF); else 1.
    pub pages: u32,
    /// Bits per sample as stored (1 for bilevel; per pixel for palette images).
    pub bit_depth: u8,
    pub model: Model,
    pub has_alpha: bool,
    /// EXIF orientation code 1..=8 (1 = as stored); not applied by `decode`.
    pub orientation: u8,
    /// Short technical description, e.g. "8-bit PseudoColor, ZPixmap, MSB first".
    pub coding: String,
    /// Key facts by stable key ("comment", "window", "created", ...).
    pub summary: Vec<(&'static str, String)>,
    /// Header fields (name, value) in file order.
    pub fields: Vec<(String, String)>,
}

/// Decoded pixels: `width × height`, top row first.
#[derive(Clone, Debug, PartialEq)]
pub enum Pixels {
    /// Display-coded (sRGB-like).
    Gray8(Vec<u8>),
    /// Display-coded, straight alpha.
    Rgba8(Vec<u8>),
    Gray16(Vec<u16>),
    Rgba16(Vec<u16>),
    /// Linear light, 1.0 = reference white.
    RgbaF32(Vec<f32>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels,
    /// Pixel data ended early or was corrupt; the missing part is grey.
    pub truncated: bool,
}

pub(crate) fn check_dims(w: u64, h: u64) -> Result<()> {
    if w == 0 || h == 0 {
        return Err(Error::Malformed(format!("empty image {w}×{h}")));
    }
    if w > MAX_DIMENSION as u64 || h > MAX_DIMENSION as u64 || w * h > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    Ok(())
}

/// Refuse headers that promise far more pixels than the file could hold (corrupt dimensions),
/// so they cannot make us allocate for them. `ratio`: the format's best compression ratio.
pub(crate) fn check_payload(have: usize, need: u64, ratio: u64) -> Result<()> {
    if (have as u64).saturating_mul(ratio).saturating_add(4096) < need {
        return Err(Error::Truncated(format!(
            "{have} bytes of pixel data for {need} bytes of image"
        )));
    }
    Ok(())
}

/// 16-bit sample to 8-bit, rounded.
#[inline]
pub(crate) fn u16_to_u8(v: u16) -> u8 {
    ((v as u32 * 255 + 32895) >> 16) as u8
}

pub(crate) fn u16le(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}
pub(crate) fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// Inflates one zlib stream into `out`; the bytes written, `None` when the stream is broken
/// before filling it.
pub(crate) fn inflate_zlib(src: &[u8], out: &mut [u8]) -> Option<usize> {
    let mut inf = zlib_rs::Inflate::new(true, 15);
    let r = inf.decompress(src, out, zlib_rs::InflateFlush::Finish);
    let n = inf.total_out() as usize;
    match r {
        Ok(_) => Some(n),
        Err(_) if n == out.len() => Some(n),
        Err(_) => None,
    }
}

/// Samples stretched from their own range (finite values only) to 16 bits, for formats
/// whose values are measurements rather than display levels. NaN shows as 0.
pub(crate) fn stretch16(v: &[f64]) -> Vec<u16> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &x in v.iter().filter(|x| x.is_finite()) {
        lo = lo.min(x);
        hi = hi.max(x);
    }
    let span = hi - lo;
    v.iter()
        .map(|&x| {
            if !x.is_finite() || span.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
                0
            } else {
                (((x - lo) / span) * 65535.0 + 0.5) as u16
            }
        })
        .collect()
}
