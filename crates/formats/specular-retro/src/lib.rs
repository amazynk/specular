//! Decoders for retro, game and terminal raster formats.
//!
//! | Module | Format | Coverage |
//! |---|---|---|
//! | [`sixel`] | DEC Sixel | 7- and 8-bit introducers, raster attributes, RGB and HLS colour registers, repeats, transparent background |
//! | [`xcursor`] | X cursor (`Xcur`) | every image; one page per nominal size, the first frame of an animation |
//! | [`avs`] | Stardent AVS X image | 8-bit ARGB |
//! | [`macpaint`] | MacPaint | 576×720 1-bit PackBits, with or without a MacBinary header |
//! | [`gem`] | GEM IMG / XIMG | 1–8 planes, pattern, solid and literal runs, vertical repeats, XIMG RGB palettes |
//! | [`atari`] | Atari ST DEGAS (`.pi1`–`.pi3`, `.pc1`–`.pc3`) and NEOchrome (`.neo`) | low, medium and high resolution, STE palettes |
//! | [`koala`] | Commodore 64 Koala Painter | 160×200 multicolour, shown 320×200 |
//! | [`zxscr`] | ZX Spectrum screen | 256×192 bitmap with attributes (BRIGHT), bitmap-only dumps |
//! | [`halo`] | Dr. Halo CUT with its `.pal` | 8-bit RLE; grey without a palette |
//! | [`softimage`] | Softimage PIC | uncompressed, pure and mixed RLE channel packets, alpha |
//! | [`aliaspix`] | Alias PIX | 24-bit and 8-bit run-length |
//! | [`rla`] | Wavefront RLA and 3ds Max RPF | 8-bit RGB(A) channels, scanline RLE |
//! | [`utahrle`] | Utah Raster Toolkit RLE | 1–4 channels, colour maps, background fill |
//! | [`wal`] | Quake 2 WAL texture | 8-bit indexed, with the game's palette when given |
//! | [`blp`] | Blizzard BLP1 / BLP2 | palette (with 0/1/4/8-bit alpha) and BGRA here; DXT and JPEG handed back |
//!
//! Each module has `info` and `decode` (some take a page or a side file's bytes). Pixels come
//! out as 8-bit grey or straight RGBA, top row first.
//!
//! Robustness: no input panics. Sizes are capped by [`MAX_DIMENSION`] and [`MAX_PIXELS`],
//! and allocation follows only checked header sizes; pixel data that ends early yields an
//! image with [`Image::truncated`] set and the missing part grey (or transparent).

pub mod aliaspix;
pub mod atari;
pub mod avs;
pub mod blp;
pub mod gem;
pub mod halo;
pub mod koala;
pub mod macpaint;
pub mod rla;
pub mod sixel;
pub mod softimage;
pub mod utahrle;
pub mod wal;
pub mod xcursor;
pub mod zxscr;

/// Largest width or height accepted.
pub const MAX_DIMENSION: u32 = 1 << 16;
/// Largest pixel count accepted (256 MP: 1 GiB as RGBA).
pub const MAX_PIXELS: u64 = 1 << 28;
/// The value missing pixels get.
pub const GREY: u8 = 128;

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
    /// An image filled with `fill` (per channel).
    pub fn filled(width: u32, height: u32, format: PixelFormat, fill: u8) -> Image {
        Image {
            width,
            height,
            format,
            data: vec![fill; width as usize * height as usize * format.channels()],
            truncated: false,
        }
    }

    /// RGBA from 8-bit palette indices.
    pub fn from_indices(width: u32, height: u32, idx: &[u8], pal: &[[u8; 4]]) -> Image {
        let mut img = Image::filled(width, height, PixelFormat::Rgba8, 0);
        for (o, &i) in img.data.chunks_exact_mut(4).zip(idx) {
            o.copy_from_slice(&pal.get(i as usize).copied().unwrap_or([0, 0, 0, 255]));
        }
        img
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

/// Header facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    /// The decoded [`Image`] layout.
    pub pixel_format: PixelFormat,
    /// Some pixel may be less than opaque.
    pub has_alpha: bool,
    pub model: Model,
    /// Bits per sample (or per pixel for palette images) as stored.
    pub bit_depth: u8,
    /// Short technical description, e.g. "8-bit palette, RLE".
    pub coding: String,
    /// Images in the file shown as pages (X cursor sizes); 1 for the rest.
    pub pages: u32,
}

impl Info {
    pub(crate) fn new(
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        model: Model,
        bit_depth: u8,
        coding: impl Into<String>,
    ) -> Info {
        Info {
            width,
            height,
            pixel_format,
            has_alpha: false,
            model,
            bit_depth,
            coding: coding.into(),
            pages: 1,
        }
    }
    pub(crate) fn alpha(mut self, a: bool) -> Info {
        self.has_alpha = a;
        self
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

/// Refuse headers that promise far more pixels than the file could hold (corrupt
/// dimensions), so they cannot make us allocate for them. `ratio`: the format's best
/// compression ratio.
pub(crate) fn check_payload(have: usize, need: u64, ratio: u64) -> Result<()> {
    if (have as u64).saturating_mul(ratio).saturating_add(4096) < need {
        return Err(Error::Truncated(format!(
            "{have} bytes of pixel data for {need} bytes of image"
        )));
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

pub(crate) fn truncated(what: &str) -> Error {
    Error::Truncated(what.into())
}

/// Apple PackBits: unpacks into `out` from `src`, returning the bytes read. Stops when `out`
/// is full or `src` ends (`out` then partly filled: the second value is false).
pub(crate) fn unpackbits(src: &[u8], out: &mut [u8]) -> (usize, bool) {
    let (mut i, mut o) = (0, 0);
    while o < out.len() {
        let Some(&n) = src.get(i) else {
            return (i, false);
        };
        i += 1;
        let n = n as i8;
        if n >= 0 {
            let len = n as usize + 1;
            let Some(lit) = src.get(i..i + len) else {
                let avail = &src[i..];
                let k = avail.len().min(out.len() - o);
                out[o..o + k].copy_from_slice(&avail[..k]);
                return (src.len(), false);
            };
            let k = len.min(out.len() - o);
            out[o..o + k].copy_from_slice(&lit[..k]);
            o += k;
            i += len;
        } else if n != -128 {
            let Some(&v) = src.get(i) else {
                return (i, false);
            };
            i += 1;
            let len = (1 - n as isize) as usize;
            let k = len.min(out.len() - o);
            out[o..o + k].fill(v);
            o += k;
        }
    }
    (i, true)
}

/// Bits per channel `bits` (1‥8) to 8 bits.
#[inline]
pub(crate) fn scale_to_8(v: u32, max: u32) -> u8 {
    if max == 0 {
        return 0;
    }
    ((v.min(max) * 255 + max / 2) / max) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packbits() {
        let mut out = [0u8; 8];
        let (n, ok) = unpackbits(&[2, 1, 2, 3, 0xfe, 9, 0x80, 0], &mut out[..6]);
        assert!(ok);
        assert_eq!(n, 6);
        assert_eq!(&out[..6], &[1, 2, 3, 9, 9, 9]);
        let (_, ok) = unpackbits(&[5, 1], &mut out);
        assert!(!ok);
    }
}
