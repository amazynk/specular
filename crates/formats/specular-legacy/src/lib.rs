//! Decoders for small legacy raster formats.
//!
//! | Module | Format | Coverage |
//! |---|---|---|
//! | [`dcx`] | multi-page PCX | the pages' PCX files |
//! | [`pcx`] | ZSoft PCX | 1/2/4-bit × 1–4 planes (EGA/CGA palettes), 8-bit palette/grey, 24-bit, 32-bit; RLE and raw |
//! | [`sgi`] | SGI / IRIS RGB | verbatim and RLE, 8/16-bit, 1–4 channels (row-parallel) |
//! | [`sun`] | Sun Raster | 1/8/24/32-bit, standard / RLE / RGB types, colour maps |
//! | [`xbm`] | X BitMap | X11 (`char`) and X10 (`short`) arrays |
//! | [`xpm`] | X PixMap 3 | any chars-per-pixel, `#rgb` … `#rrrrggggbbbb`, `None`, X11 colour names |
//! | [`wbmp`] | Wireless BMP | type 0 |
//! | [`farbfeld`] | farbfeld | 16-bit RGBA (row-parallel) |
//! | [`fli`] | Autodesk FLI / FLC animation | COLOR_64/256, BLACK, BRUN, COPY, LC, SS2, PSTAMP skipped; ring frame |
//!
//! Each still-image module has `probe(head)`, `info(data)` and `decode(data)`; [`detect`] and
//! [`decode`] dispatch on content. [`fli`] instead yields composited frames from
//! [`fli::FliFrames`].
//!
//! Robustness: no input panics. Sizes are capped by [`MAX_DIMENSION`] and [`MAX_PIXELS`] and
//! allocation follows only checked header sizes; pixel data that ends early yields an image
//! with [`Image::truncated`] set and the missing part grey.

pub mod dcx;
pub mod farbfeld;
pub mod fli;
pub mod pcx;
pub mod sgi;
pub mod sun;
pub mod wbmp;
pub mod xbm;
pub mod xpm;

/// Largest width or height accepted.
pub const MAX_DIMENSION: u32 = 1 << 16;
/// Largest pixel count accepted (256 MP: 1 GiB as RGBA).
pub const MAX_PIXELS: u64 = 1 << 28;
/// The value missing pixels get.
pub const GREY: u8 = 128;
/// Largest file [`Info::has_alpha`] reads pixels of to see whether any is transparent
/// (farbfeld, XPM); past it, a format that can hold alpha says it has some.
pub const ALPHA_SCAN_MAX: usize = 32 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not this format.
    NotRecognised,
    /// A valid file using a feature this decoder does not implement.
    Unsupported(String),
    /// The header (or the structure needed to find the pixels) is cut short.
    Truncated(String),
    Malformed(String),
    /// Dimensions over [`MAX_DIMENSION`] / [`MAX_PIXELS`].
    TooLarge,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotRecognised => write!(f, "format not recognised"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::TooLarge => write!(f, "image too large"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Pcx,
    /// Multi-page PCX.
    Dcx,
    Sgi,
    SunRaster,
    Xbm,
    Xpm,
    Wbmp,
    Farbfeld,
    Fli,
}

/// Interleaved 8-bit pixel layout of an [`Image`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Gray8,
    /// Straight (not premultiplied) alpha.
    Rgba8,
}

impl PixelFormat {
    pub fn channels(self) -> usize {
        match self {
            PixelFormat::Gray8 => 1,
            PixelFormat::Rgba8 => 4,
        }
    }
}

/// Decoded pixels: `width × height` tightly packed rows, top row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
    /// Pixel data ended early or was corrupt; the missing part is grey.
    pub truncated: bool,
}

impl Image {
    pub(crate) fn new(width: u32, height: u32, format: PixelFormat) -> Image {
        Image {
            width,
            height,
            format,
            data: vec![0; width as usize * height as usize * format.channels()],
            truncated: false,
        }
    }
}

/// Header facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub format: Format,
    pub width: u32,
    pub height: u32,
    /// The decoded [`Image`] layout.
    pub pixel_format: PixelFormat,
    /// Some pixel may be less than opaque. Farbfeld and XPM read their pixels to tell (up to
    /// [`ALPHA_SCAN_MAX`]); the others go by the header.
    pub has_alpha: bool,
    /// How the colours are stored (for the info panel).
    pub model: Model,
    /// Bits per sample (or per pixel for palette images) as stored.
    pub bit_depth: u8,
    /// Short technical description, e.g. "8-bit palette, RLE".
    pub coding: String,
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

/// Identify a format from the first bytes (32 are enough; XBM/XPM look at up to 512).
/// WBMP has no magic: it is only recognised when `head` is the whole file.
pub fn detect(head: &[u8]) -> Option<Format> {
    if farbfeld::probe(head) {
        Some(Format::Farbfeld)
    } else if sun::probe(head) {
        Some(Format::SunRaster)
    } else if sgi::probe(head) {
        Some(Format::Sgi)
    } else if fli::probe(head) {
        Some(Format::Fli)
    } else if dcx::probe(head) {
        Some(Format::Dcx)
    } else if pcx::probe(head) {
        Some(Format::Pcx)
    } else if xpm::probe(head) {
        Some(Format::Xpm)
    } else if xbm::probe(head) {
        Some(Format::Xbm)
    } else if wbmp::probe(head) {
        Some(Format::Wbmp)
    } else {
        None
    }
}

/// Header of any supported still format (FLI: the canvas and first frame's facts).
pub fn info(data: &[u8]) -> Result<Info> {
    match detect(data).ok_or(Error::NotRecognised)? {
        Format::Pcx => pcx::info(data),
        Format::Dcx => dcx::info(data, 0),
        Format::Sgi => sgi::info(data),
        Format::SunRaster => sun::info(data),
        Format::Xbm => xbm::info(data),
        Format::Xpm => xpm::info(data),
        Format::Wbmp => wbmp::info(data),
        Format::Farbfeld => farbfeld::info(data),
        Format::Fli => fli::info(data),
    }
}

/// Decode any supported format (FLI: its first frame).
pub fn decode(data: &[u8]) -> Result<Image> {
    match detect(data).ok_or(Error::NotRecognised)? {
        Format::Pcx => pcx::decode(data),
        Format::Dcx => dcx::decode(data, 0),
        Format::Sgi => sgi::decode(data),
        Format::SunRaster => sun::decode(data),
        Format::Xbm => xbm::decode(data),
        Format::Xpm => xpm::decode(data),
        Format::Wbmp => wbmp::decode(data),
        Format::Farbfeld => farbfeld::decode(data),
        Format::Fli => fli::decode_first(data),
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

pub(crate) fn u16le(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}
pub(crate) fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}
pub(crate) fn u16be(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(o..o + 2)?.try_into().ok()?))
}
pub(crate) fn u32be(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// 16-bit sample to 8-bit, rounded.
#[inline]
pub(crate) fn u16_to_u8(v: u16) -> u8 {
    ((v as u32 * 255 + 32895) >> 16) as u8
}

/// Map palette indices to RGBA (`pal` entries beyond its length are black).
pub(crate) fn expand_palette(idx: &[u8], pal: &[[u8; 3]], out: &mut [u8]) {
    for (o, &i) in out.chunks_exact_mut(4).zip(idx) {
        let [r, g, b] = pal.get(i as usize).copied().unwrap_or([0; 3]);
        o.copy_from_slice(&[r, g, b, 255]);
    }
}

/// Refuse headers that promise far more pixels than the file could hold (corrupt dimensions),
/// so they cannot make us allocate for them. `ratio`: the format's best compression ratio.
/// Partial files within that bound still decode, with [`Image::truncated`] set.
pub(crate) fn check_payload(have: usize, need: u64, ratio: u64) -> Result<()> {
    if (have as u64).saturating_mul(ratio).saturating_add(4096) < need {
        return Err(Error::Truncated(format!(
            "{have} bytes of pixel data for {need} bytes of image"
        )));
    }
    Ok(())
}
