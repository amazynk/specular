//! `specular-fits`: a FITS reader for Specular's science viewing.
//!
//! * **Structure.** Every HDU of a file is walked at [`parse`]: the primary HDU, `IMAGE`
//!   extensions (multi-extension files), and binary tables, of which those with `ZIMAGE = T`
//!   are tile-compressed images. Headers keep every card ([`header`]), with `CONTINUE` long
//!   strings folded in.
//! * **Images.** `BITPIX` 8, 16, 32, 64, −32 and −64, `NAXIS` 1 to 999 (cubes and
//!   hypercubes: axes past the second are planes), big-endian throughout. Values come out
//!   as physical `f32` (`BZERO + BSCALE × stored`), with `BLANK` integers and IEEE NaNs as NaN.
//!   Planes are read on request ([`read_plane`]), converted in parallel, so a large cube never
//!   has to be in memory at once.
//! * **Tile compression** ([`tiled`]): `RICE_1`, `GZIP_1`, `GZIP_2`, `HCOMPRESS_1` (with
//!   smoothing), `PLIO_1` and `NOCOMPRESS`, integer and quantised floating-point images
//!   (`NO_DITHER`, `SUBTRACTIVE_DITHER_1` and `_2`), tiles stored losslessly in
//!   `GZIP_COMPRESSED_DATA` or `UNCOMPRESSED_DATA`, per-tile or header `ZSCALE` / `ZZERO` /
//!   `ZBLANK`.
//! * **WCS** ([`wcs`]): celestial coordinates of the image plane in every projection of
//!   FITS WCS Paper II plus HEALPix (`HPX`, `XPH`), through SIP, TPV and the HST `D2IM` /
//!   `CPDIS` lookup tables, and the linear or logarithmic world coordinates of the other
//!   axes. What it cannot follow is reported ([`wcs::Wcs::unsupported`]), never guessed.
//! * **Orientation.** FITS stores the first row at the bottom. [`read_plane`] returns rows
//!   top-down as displayed (the last FITS row first), which is how astronomers view images
//!   (north up for most data); [`wcs::Wcs`] takes coordinates in that top-down frame.
//! * **Robustness.** Malformed input never panics; sizes are checked before allocating, and a
//!   truncated last HDU reads as NaN past the end of the file ([`Plane::truncated`]).

pub mod catalogue;
pub mod compress;
pub mod events;
pub mod header;
pub mod healpix;
pub mod table;
pub mod tiled;
pub mod wcs;

use rayon::prelude::*;

pub use header::{Card, Header, Value, sniff};

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not a FITS file")]
    NotFits,
    #[error("malformed FITS: {0}")]
    Malformed(String),
    #[error("unsupported FITS: {0}")]
    Unsupported(String),
    #[error("image too large")]
    TooLarge,
    /// Under the caps, but more than the memory budget has free now.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
    #[error("cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Largest plane (width × height) and largest image (all axes) accepted.
pub const MAX_PLANE: u64 = 1 << 30;
pub const MAX_IMAGE: u64 = 1 << 40;

/// What an HDU holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Image,
    /// A binary table holding a tile-compressed image.
    CompressedImage,
    BinTable,
    Table,
    /// Another extension type (its `XTENSION` value).
    Other(String),
}

/// A HDU's image: its stored type, axes and scaling.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageDesc {
    /// `BITPIX` of the image (`ZBITPIX` of a compressed one).
    pub bitpix: i32,
    /// `NAXISn`, fastest first (width, height, planes…).
    pub axes: Vec<usize>,
    pub bzero: f64,
    pub bscale: f64,
    /// `BLANK` of an integer image.
    pub blank: Option<i64>,
    /// The compression of a tile-compressed image.
    pub tiles: Option<tiled::Tiles>,
}

impl ImageDesc {
    pub fn width(&self) -> usize {
        self.axes.first().copied().unwrap_or(1)
    }
    pub fn height(&self) -> usize {
        self.axes.get(1).copied().unwrap_or(1)
    }
    /// Number of 2-D planes (the product of the axes past the second).
    pub fn planes(&self) -> usize {
        self.axes.iter().skip(2).product()
    }
    /// Bytes per stored sample.
    pub fn sample_bytes(&self) -> usize {
        (self.bitpix.unsigned_abs() / 8) as usize
    }
    /// The plane's linear index for a position on the axes past the second.
    pub fn plane_index(&self, pos: &[usize]) -> usize {
        let mut idx = 0;
        let mut stride = 1;
        for (i, &n) in self.axes.iter().skip(2).enumerate() {
            idx += pos.get(i).copied().unwrap_or(0).min(n.saturating_sub(1)) * stride;
            stride *= n;
        }
        idx
    }
    /// Short description, e.g. "32-bit float" or "16-bit integer, Rice".
    pub fn coding(&self) -> String {
        let t = match self.bitpix {
            8 => "8-bit unsigned",
            16 if self.bzero == 32768.0 && self.bscale == 1.0 => "16-bit unsigned",
            16 => "16-bit integer",
            32 if self.bzero == 2147483648.0 && self.bscale == 1.0 => "32-bit unsigned",
            32 => "32-bit integer",
            64 => "64-bit integer",
            -32 => "32-bit float",
            -64 => "64-bit float",
            _ => "?",
        };
        match &self.tiles {
            Some(t2) => format!("{t}, {}", t2.algorithm.name()),
            None => t.to_owned(),
        }
    }
}

/// One header-data unit.
#[derive(Clone, Debug, PartialEq)]
pub struct Hdu {
    pub index: usize,
    pub kind: Kind,
    pub header: Header,
    /// Byte offset of the data, and its length as the header states it.
    pub data_offset: usize,
    pub data_len: usize,
    /// The image, for image HDUs with data (and compressed images).
    pub image: Option<ImageDesc>,
}

impl Hdu {
    /// `EXTNAME` (with `EXTVER` when above 1), when the HDU has one.
    pub fn name(&self) -> Option<String> {
        let n = self.header.str("EXTNAME")?.trim();
        if n.is_empty() {
            return None;
        }
        Some(match self.header.i64("EXTVER") {
            Some(v) if v > 1 => format!("{n},{v}"),
            _ => n.to_owned(),
        })
    }
}

/// A parsed file: its HDUs (data not read).
#[derive(Clone, Debug, PartialEq)]
pub struct Fits {
    pub hdus: Vec<Hdu>,
}

impl Fits {
    /// The HDUs holding an image with at least one pixel.
    pub fn images(&self) -> impl Iterator<Item = &Hdu> {
        self.hdus.iter().filter(|h| h.image.is_some())
    }
}

/// The number of bytes the data of a HDU with `header` occupies (before block padding).
fn data_size(h: &Header) -> Option<usize> {
    let bitpix = h.i64("BITPIX")?;
    let naxis = h.i64("NAXIS")?;
    if !(0..=999).contains(&naxis) {
        return None;
    }
    if naxis == 0 {
        return Some(0);
    }
    let mut n: u64 = 1;
    let mut groups = false;
    for i in 1..=naxis {
        let a = h.i64(&format!("NAXIS{i}"))?;
        if a < 0 {
            return None;
        }
        if i == 1 && a == 0 && h.bool("GROUPS") == Some(true) {
            groups = true;
            continue;
        }
        n = n.checked_mul(a as u64)?;
    }
    let pcount = h.i64("PCOUNT").unwrap_or(0).max(0) as u64;
    let gcount = h.i64("GCOUNT").unwrap_or(1).max(if groups { 0 } else { 1 }) as u64;
    let bits = n.checked_add(pcount)?.checked_mul(gcount)?;
    let bytes = bits.checked_mul(bitpix.unsigned_abs())? / 8;
    usize::try_from(bytes).ok()
}

pub(crate) fn axes_of(h: &Header, prefix: &str, count_key: &str) -> Result<Vec<usize>> {
    let n = h
        .i64(count_key)
        .ok_or_else(|| Error::Malformed(format!("no {count_key}")))?;
    if !(0..=999).contains(&n) {
        return Err(Error::Malformed(format!("{count_key} = {n}")));
    }
    let mut axes = Vec::with_capacity(n as usize);
    for i in 1..=n {
        let a = h
            .i64(&format!("{prefix}{i}"))
            .ok_or_else(|| Error::Malformed(format!("no {prefix}{i}")))?;
        if a < 0 {
            return Err(Error::Malformed(format!("{prefix}{i} = {a}")));
        }
        axes.push(a as usize);
    }
    Ok(axes)
}

pub(crate) fn check_size(axes: &[usize]) -> Result<()> {
    let plane = axes
        .iter()
        .take(2)
        .try_fold(1u64, |a, &b| a.checked_mul(b as u64));
    let all = axes.iter().try_fold(1u64, |a, &b| a.checked_mul(b as u64));
    match (plane, all) {
        (Some(p), Some(a)) if p <= MAX_PLANE && a <= MAX_IMAGE => Ok(()),
        _ => Err(Error::TooLarge),
    }
}

/// Reads the image description of a primary or IMAGE HDU.
fn image_desc(h: &Header) -> Result<Option<ImageDesc>> {
    let bitpix = h
        .i64("BITPIX")
        .ok_or(Error::Malformed("no BITPIX".into()))?;
    if !matches!(bitpix, 8 | 16 | 32 | 64 | -32 | -64) {
        return Err(Error::Malformed(format!("BITPIX = {bitpix}")));
    }
    let axes = axes_of(h, "NAXIS", "NAXIS")?;
    if axes.is_empty() || axes.contains(&0) || h.bool("GROUPS") == Some(true) {
        return Ok(None);
    }
    check_size(&axes)?;
    Ok(Some(ImageDesc {
        bitpix: bitpix as i32,
        axes,
        bzero: h.f64("BZERO").unwrap_or(0.0),
        bscale: h.f64("BSCALE").filter(|s| *s != 0.0).unwrap_or(1.0),
        blank: if bitpix > 0 { h.i64("BLANK") } else { None },
        tiles: None,
    }))
}

/// Walks every HDU of `data`.
pub fn parse(data: &[u8]) -> Result<Fits> {
    if !sniff(data) {
        return Err(Error::NotFits);
    }
    let mut hdus = Vec::new();
    let mut pos = 0usize;
    while pos < data.len() {
        let Some((header, hlen)) = header::parse(&data[pos..]) else {
            if hdus.is_empty() {
                return Err(Error::Malformed("primary header has no END".into()));
            }
            break; // Trailing garbage or a damaged extension: keep what we have.
        };
        let index = hdus.len();
        let kind = if index == 0 {
            Kind::Primary
        } else {
            match header.str("XTENSION").map(str::trim) {
                Some("IMAGE") | Some("IUEIMAGE") => Kind::Image,
                Some("BINTABLE") if header.bool("ZIMAGE") == Some(true) => Kind::CompressedImage,
                Some("BINTABLE") => Kind::BinTable,
                Some("TABLE") => Kind::Table,
                Some(x) => Kind::Other(x.to_owned()),
                None => break, // Not an extension: data after the last HDU.
            }
        };
        let Some(dlen) = data_size(&header) else {
            if hdus.is_empty() {
                return Err(Error::Malformed("bad data size".into()));
            }
            break;
        };
        let data_offset = pos + hlen;
        let image = match kind {
            Kind::Primary | Kind::Image => image_desc(&header).ok().flatten(),
            Kind::CompressedImage => tiled::image_desc(&header).ok(),
            _ => None,
        };
        hdus.push(Hdu {
            index,
            kind,
            header,
            data_offset,
            data_len: dlen,
            image,
        });
        let next = dlen.div_ceil(header::BLOCK).checked_mul(header::BLOCK);
        match next.and_then(|n| data_offset.checked_add(n)) {
            Some(n) => pos = n,
            None => break,
        }
    }
    Ok(Fits { hdus })
}

/// One decoded plane: physical values, rows top-down as displayed.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    pub width: usize,
    pub height: usize,
    /// `width × height` values; NaN for blank pixels.
    pub values: Vec<f32>,
    /// Part of the plane lay past the end of the file (read as NaN).
    pub truncated: bool,
}

/// Reads plane `plane` (the linear index over the axes past the second, see
/// [`ImageDesc::plane_index`]) of `hdu`'s image as physical values, top-down.
pub fn read_plane(data: &[u8], hdu: &Hdu, plane: usize) -> Result<Plane> {
    let img = hdu
        .image
        .as_ref()
        .ok_or_else(|| Error::Unsupported("HDU has no image".into()))?;
    if plane >= img.planes().max(1) {
        return Err(Error::Malformed("no such plane".into()));
    }
    let (w, h) = (img.width(), img.height());
    if let Some(t) = &img.tiles {
        return tiled::read_plane(data, hdu, img, t, plane);
    }
    let bytes = img.sample_bytes();
    let plane_bytes = w * h * bytes;
    let start = hdu
        .data_offset
        .saturating_add(plane.saturating_mul(plane_bytes));
    let avail = data.get(start..).unwrap_or(&[]);
    let n = w * h;
    let _res = specular_core::budget::reserve_n(n as u64, 4)?;
    let mut values = Vec::new();
    values.try_reserve_exact(n).map_err(|_| Error::TooLarge)?;
    values.resize(n, f32::NAN);
    let truncated = avail.len() < plane_bytes;
    let conv = Convert::new(img);
    // FITS row r (0-based from the bottom) goes to display row h - 1 - r.
    values
        .par_chunks_mut(w.max(1))
        .enumerate()
        .for_each(|(drow, out)| {
            let r = h - 1 - drow;
            let off = r * w * bytes;
            let src = avail.get(off..).unwrap_or(&[]);
            let src = &src[..src.len().min(w * bytes)];
            conv.row(src, out);
        });
    Ok(Plane {
        width: w,
        height: h,
        values,
        truncated,
    })
}

/// The values along the third axis at displayed pixel (`x`, `y`) (rows top-down as
/// [`read_plane`] gives them), further axes at their first position: a data cube's spectrum
/// at a pixel. Values past the end of the file are NaN. Uncompressed images only.
pub fn read_spectrum(data: &[u8], hdu: &Hdu, x: usize, y: usize) -> Result<Vec<f32>> {
    let img = hdu
        .image
        .as_ref()
        .ok_or_else(|| Error::Unsupported("HDU has no image".into()))?;
    if img.tiles.is_some() {
        return Err(Error::Unsupported(
            "spectra of tile-compressed cubes".into(),
        ));
    }
    let (w, h) = (img.width(), img.height());
    if x >= w || y >= h {
        return Err(Error::Malformed("no such pixel".into()));
    }
    let n = img.axes.get(2).copied().unwrap_or(1);
    // Every plane holds at least a byte of the file.
    if n > data.len().max(1) {
        return Err(Error::TooLarge);
    }
    let bytes = img.sample_bytes();
    let plane_bytes = (w * h * bytes) as u64;
    let at = ((h - 1 - y) * w + x) * bytes;
    let conv = Convert::new(img);
    let mut out = vec![f32::NAN; n];
    for (k, o) in out.iter_mut().enumerate() {
        let start = hdu.data_offset as u64 + k as u64 * plane_bytes + at as u64;
        let Ok(start) = usize::try_from(start) else {
            break;
        };
        let Some(src) = data.get(start..start.saturating_add(bytes)) else {
            break;
        };
        conv.row(src, std::slice::from_mut(o));
    }
    Ok(out)
}

/// Stored big-endian samples to physical f32.
#[derive(Clone, Copy)]
pub(crate) struct Convert {
    pub bitpix: i32,
    pub bzero: f64,
    pub bscale: f64,
    pub blank: Option<i64>,
}

impl Convert {
    pub(crate) fn new(img: &ImageDesc) -> Convert {
        Convert {
            bitpix: img.bitpix,
            bzero: img.bzero,
            bscale: img.bscale,
            blank: img.blank,
        }
    }

    /// Converts the samples in `src` into `out` (a short `src` leaves the rest untouched).
    pub(crate) fn row(&self, src: &[u8], out: &mut [f32]) {
        let (z, s) = (self.bzero, self.bscale);
        let ident = z == 0.0 && s == 1.0;
        macro_rules! ints {
            ($t:ty, $n:expr) => {{
                let blank = self.blank;
                for (o, c) in out.iter_mut().zip(src.chunks_exact($n)) {
                    let v = <$t>::from_be_bytes(c.try_into().unwrap());
                    *o = if blank == Some(v as i64) {
                        f32::NAN
                    } else if ident {
                        v as f32
                    } else {
                        (z + s * v as f64) as f32
                    };
                }
            }};
        }
        match self.bitpix {
            8 => {
                let blank = self.blank;
                for (o, &v) in out.iter_mut().zip(src) {
                    *o = if blank == Some(i64::from(v)) {
                        f32::NAN
                    } else {
                        (z + s * f64::from(v)) as f32
                    };
                }
            }
            16 => {
                // The hot integer case: f32 arithmetic is exact for 16-bit values with
                // integer scaling (unsigned via BZERO = 32768).
                let blank = self.blank;
                let zf = z as f32;
                let exact = z.fract() == 0.0 && s == 1.0 && z.abs() <= 65536.0;
                for (o, c) in out.iter_mut().zip(src.chunks_exact(2)) {
                    let v = i16::from_be_bytes([c[0], c[1]]);
                    *o = if blank == Some(i64::from(v)) {
                        f32::NAN
                    } else if exact {
                        f32::from(v) + zf
                    } else {
                        (z + s * f64::from(v)) as f32
                    };
                }
            }
            32 => ints!(i32, 4),
            64 => ints!(i64, 8),
            -32 => {
                for (o, c) in out.iter_mut().zip(src.chunks_exact(4)) {
                    let v = f32::from_be_bytes([c[0], c[1], c[2], c[3]]);
                    *o = if ident {
                        v
                    } else {
                        (z + s * f64::from(v)) as f32
                    };
                }
            }
            -64 => {
                for (o, c) in out.iter_mut().zip(src.chunks_exact(8)) {
                    let v = f64::from_be_bytes(c.try_into().unwrap());
                    *o = (z + s * v) as f32;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
