//! Decoders for film and VFX raster formats.
//!
//! | Module | Format | Coverage |
//! |---|---|---|
//! | [`dpx`] | SMPTE 268M DPX, v1.0 and v2.0, either byte order | 1/8/10/12/16-bit integer and 32/64-bit float samples; packed, filled method A and B; RGB, RGBA, ABGR, single channels, luma, CbYCrY 4:2:2, CbYACrYA, CbYCr 4:4:4, CbYCrA, user-defined 2–8 channels, composite video as grey; signed integers; RLE (floats too); separate R/G/B(/A), luma + colour-difference and RGB + alpha elements combined into one page; every other image element as a page |
//! | [`cineon`] | Kodak Cineon 4.5, either byte order | 1–16-bit (mixed between channels), signed or not, every packing, pixel/line/channel interleave, 1–8 channels (RGB shown) |
//!
//! Each module has `probe(head)`, `info(data, page)` and `decode(data, page)`; [`detect`],
//! [`info`] and [`decode`] dispatch on content.
//!
//! # Colour
//! Log data (DPX transfer "printing density" or "logarithmic", every Cineon file) is turned
//! into scene-linear light with the Kodak printing-density conversion: a code value `c` is
//! `0.002` density per code (or what the header's reference quantities say), and with
//! negative film gamma `γ = 0.6`, reference white `W = 685` and black `B = 95`
//!
//! ```text
//! lin(c) = (10^((c − W)·d/γ) − 10^((B − W)·d/γ)) / (1 − 10^((B − W)·d/γ))
//! ```
//!
//! so black is 0, reference white 1.0 and the highlights above it (up to about 13.5 at code
//! 1023) are kept as HDR. A "display gamma" of 1.7 (the film-industry print-look convention)
//! is *not* baked in: the output is linear and the viewer's display transform encodes it,
//! which shows reference white as white and leaves the highlights to tone mapping. DPX files
//! whose television header gives a black and white level inside the code range use those
//! instead of 95 and 685, and both formats take the density per code from their reference
//! (DPX) or minimum/maximum (Cineon) quantities when those are plausible. Cineon's
//! origination "gamma" is the input device's, not the film's, and is only reported.
//!
//! ImageMagick's printing-density curve scales code values by 1024/1023 before the same
//! formula, so its linear values run up to about 0.6 % above these near white.
//!
//! Linear DPX (transfer 2) comes out linear, scaled by the reference codes. Video and
//! unspecified transfers are display-coded and come out as 16-bit samples stretched from the
//! reference codes (video range for YCbCr), with the YCbCr matrix chosen by the colorimetric
//! code (BT.601 for the BT.601 and composite codes, BT.709 otherwise). YCbCr reference codes
//! left at zero (ImageMagick) are taken as full range; unset ones as video range.
//!
//! Robustness: no input panics. Sizes are capped by [`MAX_DIMENSION`] and [`MAX_PIXELS`],
//! allocation follows only checked header sizes, and pixel data that ends early yields an
//! image with [`Image::truncated`] set and the missing part grey.

pub mod cineon;
pub mod dpx;
mod log;
mod unpack;

/// Largest width or height accepted.
pub const MAX_DIMENSION: u32 = 1 << 16;
/// Largest pixel count accepted (256 MP).
pub const MAX_PIXELS: u64 = 1 << 28;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not this format.
    NotRecognised,
    /// A valid file using a feature this decoder does not implement (a plain reason).
    Unsupported(String),
    /// The header (or the structure needed to find the pixels) is cut short.
    Truncated(String),
    Malformed(String),
    /// Dimensions over [`MAX_DIMENSION`] / [`MAX_PIXELS`].
    TooLarge,
    /// The memory budget cannot hold the decode.
    NeedsMemory(specular_core::NeedsMemory),
}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
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

/// [`Info::summary`] and [`Info::fields`].
pub(crate) type Facts = (Vec<(&'static str, String)>, Vec<(String, String)>);

/// Which format a file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dpx,
    Cineon,
}

/// How the stored colours are modelled (for the info panel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Grey,
    Rgb,
    Palette,
    YCbCr,
    Bilevel,
}

/// Header facts of one page (DPX image element).
#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    /// Image elements in the file (DPX), each a page; 1 for Cineon.
    pub pages: u32,
    /// Bits per sample as stored.
    pub bit_depth: u8,
    pub model: Model,
    pub has_alpha: bool,
    /// EXIF orientation code 1..=8 (1 = as stored); [`decode`] does not apply it.
    pub orientation: u8,
    /// Short technical line, e.g. "10-bit RGB, filled (method A), big-endian, printing density".
    pub coding: String,
    /// Key facts by stable key ("timecode", "edge-code", "frame-rate", …).
    pub summary: Vec<(&'static str, String)>,
    /// Every set header field (name, value), in file order.
    pub fields: Vec<(String, String)>,
}

/// Decoded pixels: `width × height`, rows top first as stored (orientation not applied).
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels,
    /// Pixel data ended early or was corrupt; the missing part is grey.
    pub truncated: bool,
}

/// Interleaved samples.
#[derive(Clone, Debug, PartialEq)]
pub enum Pixels {
    /// Display-coded, straight alpha.
    Gray8(Vec<u8>),
    Rgba8(Vec<u8>),
    /// Display-coded, full 0..=65535 range.
    Gray16(Vec<u16>),
    Rgba16(Vec<u16>),
    /// Linear light, 1.0 = reference white (may exceed 1); straight alpha.
    RgbaF32(Vec<f32>),
}

/// Identify a format from the first bytes (4 are enough).
pub fn detect(head: &[u8]) -> Option<Kind> {
    if dpx::probe(head) {
        Some(Kind::Dpx)
    } else if cineon::probe(head) {
        Some(Kind::Cineon)
    } else {
        None
    }
}

/// Header facts of page `page`.
pub fn info(data: &[u8], page: u32) -> Result<Info> {
    match detect(data).ok_or(Error::NotRecognised)? {
        Kind::Dpx => dpx::info(data, page),
        Kind::Cineon => cineon::info(data, page),
    }
}

/// Decode page `page`.
pub fn decode(data: &[u8], page: u32) -> Result<Image> {
    match detect(data).ok_or(Error::NotRecognised)? {
        Kind::Dpx => dpx::decode(data, page),
        Kind::Cineon => cineon::decode(data, page),
    }
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

/// Reads fixed-layout header fields in the file's byte order.
#[derive(Clone, Copy)]
pub(crate) struct Reader<'a> {
    pub d: &'a [u8],
    pub be: bool,
}

impl<'a> Reader<'a> {
    pub fn u8(&self, o: usize) -> Option<u8> {
        self.d.get(o).copied()
    }
    pub fn u16(&self, o: usize) -> Option<u16> {
        let b: [u8; 2] = self.d.get(o..o + 2)?.try_into().ok()?;
        Some(if self.be {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    }
    pub fn u32(&self, o: usize) -> Option<u32> {
        let b: [u8; 4] = self.d.get(o..o + 4)?.try_into().ok()?;
        Some(if self.be {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    }
    /// A set 32-bit field: `None` for the all-ones "undefined" value.
    pub fn u32_set(&self, o: usize) -> Option<u32> {
        self.u32(o).filter(|&v| v != u32::MAX)
    }
    /// A set, finite float field.
    pub fn f32_set(&self, o: usize) -> Option<f32> {
        let bits = self.u32(o)?;
        let v = f32::from_bits(bits);
        (bits != u32::MAX && v.is_finite()).then_some(v)
    }
    /// A NUL-terminated (or full-width) ASCII field, trimmed; `None` when empty or unset.
    pub fn str(&self, o: usize, len: usize) -> Option<String> {
        let b = self.d.get(o..o + len)?;
        let end = b.iter().position(|&c| c == 0).unwrap_or(len);
        let b = &b[..end];
        if b.iter().all(|&c| c == 0xff) {
            return None;
        }
        let s: String = b
            .iter()
            .map(|&c| {
                if (0x20..0x7f).contains(&c) {
                    c as char
                } else {
                    ' '
                }
            })
            .collect();
        let s = s.trim().to_owned();
        (!s.is_empty()).then_some(s)
    }
}

/// Formats a float compactly ("24", "23.976").
pub(crate) fn num(v: f32) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_owned()
}

/// "YYYY:MM:DD:HH:MM:SS[TZ]" (DPX) as "YYYY-MM-DD HH:MM:SS[ TZ]"; anything else as stored.
pub(crate) fn tidy_date(s: &str) -> String {
    let b = s.as_bytes();
    let digits =
        |r: std::ops::Range<usize>| b.get(r).is_some_and(|x| x.iter().all(u8::is_ascii_digit));
    if b.len() >= 19
        && digits(0..4)
        && digits(5..7)
        && digits(8..10)
        && digits(11..13)
        && digits(14..16)
        && digits(17..19)
    {
        let tz = s[19..].trim();
        let mut out = format!(
            "{}-{}-{} {}:{}:{}",
            &s[0..4],
            &s[5..7],
            &s[8..10],
            &s[11..13],
            &s[14..16],
            &s[17..19]
        );
        if !tz.is_empty() {
            out.push(' ');
            out.push_str(tz);
        }
        out
    } else {
        s.to_owned()
    }
}

/// EXIF orientation of a DPX / Cineon orientation code (0..=7).
pub(crate) fn exif_orientation(code: u32) -> u8 {
    match code {
        1 => 2,
        2 => 4,
        3 => 3,
        4 => 5,
        5 => 6,
        6 => 8,
        7 => 7,
        _ => 1,
    }
}

pub(crate) fn orientation_name(code: u32) -> &'static str {
    match code {
        0 => "left to right, top to bottom",
        1 => "right to left, top to bottom",
        2 => "left to right, bottom to top",
        3 => "right to left, bottom to top",
        4 => "top to bottom, left to right",
        5 => "top to bottom, right to left",
        6 => "bottom to top, left to right",
        7 => "bottom to top, right to left",
        _ => "undefined",
    }
}

/// Refuse headers that promise far more pixels than the file could hold (corrupt sizes),
/// so they cannot make us allocate for them; partial files within that bound still decode.
pub(crate) fn check_payload(have: usize, need: u64, ratio: u64) -> Result<()> {
    if (have as u64).saturating_mul(ratio).saturating_add(4096) < need {
        return Err(Error::Truncated(format!(
            "{have} bytes of pixel data for {need} bytes of image"
        )));
    }
    Ok(())
}
