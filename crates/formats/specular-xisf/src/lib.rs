//! `specular-xisf`: a decoder for XISF 1.0 (Extensible Image Serialization Format, PixInsight).
//!
//! * **Container.** Monolithic files: the `XISF0100` signature, the XML header's length (u32
//!   little-endian), four reserved bytes, the XML header, then attached data blocks at absolute
//!   offsets (`location="attachment:offset:size"`). Inline (`inline:base64`, `inline:hex`) and
//!   embedded (`<Data encoding="…">`) blocks are read too, and `<Reference ref="…"/>` elements
//!   resolve to the element with that `uid`.
//! * **Images.** Every `<Image>`: any geometry (`w:h[:d…]:channels`; dimensions past the
//!   second are multiplied out into [`Image::depth`] slices), UInt8/16/32/64 and Float32/64
//!   samples (complex samples are refused), planar (default) and normal (interleaved) storage,
//!   either byte order, Gray / RGB / CIELab colour spaces, float `bounds`.
//! * **Compression.** zlib, LZ4, LZ4HC and Zstandard, with byte shuffling (`codec+sh:size:item`)
//!   and sub-blocks (`subblocks="c,u:c,u…"`).
//! * **Metadata.** `<FITSKeyword>` elements and `<Property>` elements (scalars, strings, time
//!   points; vectors and matrices by type only), per image and at file level (also inside
//!   `<Metadata>`).
//! * **Values.** Planes come out as `f32` real values: integers as their integer value, floats
//!   as stored; nothing is normalised (the bounds are reported, not applied).
//! * **Robustness.** Malformed input never panics; sizes are checked against [`MAX_BLOCK`] and
//!   [`MAX_SAMPLES`] before anything is allocated.

mod codec;
mod header;
#[cfg(test)]
mod tests;

use std::borrow::Cow;

use rayon::prelude::*;

/// The file signature.
pub const SIGNATURE: &[u8; 8] = b"XISF0100";
/// Largest (uncompressed) data block read, in bytes.
pub const MAX_BLOCK: u64 = 1 << 34;
/// Largest number of samples (width × height × depth × channels) in an image.
pub const MAX_SAMPLES: u64 = 1 << 31;

/// Errors from [`parse`] and the pixel readers.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not an XISF file")]
    NotXisf,
    #[error("truncated: {0}")]
    Truncated(&'static str),
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("image too large")]
    TooLarge,
    #[error("corrupt compressed data: {0}")]
    Codec(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Whether `head` starts with the XISF signature.
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(SIGNATURE)
}

/// A parsed XISF header.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Xisf {
    pub images: Vec<Image>,
    /// File-level properties (direct children of the root and of `<Metadata>`).
    pub properties: Vec<Property>,
}

/// Parses the header of a monolithic XISF file (`data` is the whole file).
pub fn parse(data: &[u8]) -> Result<Xisf> {
    header::parse(data)
}

/// Pixel sample formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SampleFormat {
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Float32,
    Float64,
}

impl SampleFormat {
    /// Bytes per sample.
    pub fn bytes(self) -> usize {
        match self {
            SampleFormat::UInt8 => 1,
            SampleFormat::UInt16 => 2,
            SampleFormat::UInt32 | SampleFormat::Float32 => 4,
            SampleFormat::UInt64 | SampleFormat::Float64 => 8,
        }
    }
    pub fn is_float(self) -> bool {
        matches!(self, SampleFormat::Float32 | SampleFormat::Float64)
    }
    /// The XISF name, e.g. "UInt16".
    pub fn name(self) -> &'static str {
        match self {
            SampleFormat::UInt8 => "UInt8",
            SampleFormat::UInt16 => "UInt16",
            SampleFormat::UInt32 => "UInt32",
            SampleFormat::UInt64 => "UInt64",
            SampleFormat::Float32 => "Float32",
            SampleFormat::Float64 => "Float64",
        }
    }
}

/// How channels are laid out in the pixel block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Storage {
    /// One whole plane (all slices) per channel, channel after channel (XISF's default).
    #[default]
    Planar,
    /// Interleaved: all channels of a pixel together.
    Normal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColorSpace {
    #[default]
    Gray,
    Rgb,
    CieLab,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ByteOrder {
    #[default]
    Little,
    Big,
}

/// Where a data block is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Location {
    /// At an absolute file offset.
    Attachment { offset: u64, len: u64 },
    /// In the element's text (already decoded from base64 / hex).
    Inline(Vec<u8>),
    /// In a `<Data>` child element (already decoded).
    Embedded(Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Codec {
    Zlib,
    Lz4,
    Lz4Hc,
    Zstd,
}

/// A data block's compression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compression {
    pub codec: Codec,
    /// Size of the whole block uncompressed.
    pub uncompressed: u64,
    /// Byte-shuffling item size (`codec+sh`), when shuffled.
    pub shuffle_item: Option<u32>,
    /// Sub-blocks (compressed size, uncompressed size), in order; empty for one block.
    pub subblocks: Vec<(u64, u64)>,
}

/// A `<FITSKeyword>` element as stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FitsKeyword {
    pub name: String,
    pub value: String,
    pub comment: String,
}

impl FitsKeyword {
    /// The value without the quotes of a FITS string (`'M31 '` → `M31`).
    pub fn text(&self) -> &str {
        let v = self.value.trim();
        match v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
            Some(s) => s.trim_end(),
            None => v,
        }
    }
}

/// A `<Property>` element.
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    pub id: String,
    /// The XISF type name ("Float64", "String", "TimePoint", "UI16Vector", …).
    pub kind: String,
    pub value: PropertyValue,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Number(f64),
    String(String),
    Bool(bool),
    TimePoint(String),
    /// A `F32Vector` or `F64Vector`.
    Vector(Vec<f64>),
    /// A `F32Matrix` or `F64Matrix`, row by row.
    Matrix {
        rows: usize,
        columns: usize,
        values: Vec<f64>,
    },
    /// Complex numbers, integer vectors and matrices, and float ones whose block is
    /// compressed or does not hold the stated size: the `value` attribute or a short
    /// description.
    Other(String),
}

/// One `<Image>` element.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// The stated dimensions without the channel count (width, height, …).
    pub geometry: Vec<u32>,
    pub width: u32,
    pub height: u32,
    /// Slices: the product of the dimensions past the second (1 for a 2-D image).
    pub depth: u32,
    pub channels: u32,
    pub sample: SampleFormat,
    pub storage: Storage,
    pub color_space: ColorSpace,
    pub byte_order: ByteOrder,
    /// The `bounds` attribute (float images' nominal range).
    pub bounds: Option<(f64, f64)>,
    pub location: Location,
    pub compression: Option<Compression>,
    pub fits_keywords: Vec<FitsKeyword>,
    pub properties: Vec<Property>,
    pub image_type: Option<String>,
    pub offset: Option<String>,
    pub orientation: Option<String>,
    pub id: Option<String>,
    /// The `<ColorFilterArray>` child: its pattern (letters top row first, from the first
    /// stored pixel) and its width and height.
    pub color_filter: Option<(String, u32, u32)>,
}

impl Image {
    /// Samples in the whole image (every slice and channel).
    pub fn sample_count(&self) -> u64 {
        u64::from(self.width)
            * u64::from(self.height)
            * u64::from(self.depth)
            * u64::from(self.channels)
    }

    /// Bytes of the uncompressed pixel block.
    pub fn byte_len(&self) -> u64 {
        self.sample_count() * self.sample.bytes() as u64
    }

    /// The first FITS keyword called `name`.
    pub fn keyword(&self, name: &str) -> Option<&FitsKeyword> {
        self.fits_keywords.iter().find(|k| k.name == name)
    }

    /// The property with id `id`.
    pub fn property(&self, id: &str) -> Option<&Property> {
        self.properties.iter().find(|p| p.id == id)
    }

    /// The uncompressed, un-shuffled pixel block (borrowed from `data` when stored plainly).
    pub fn pixel_bytes<'a>(&'a self, data: &'a [u8]) -> Result<Cow<'a, [u8]>> {
        let need = self.byte_len();
        if need > MAX_BLOCK {
            return Err(Error::TooLarge);
        }
        let raw: &[u8] = match &self.location {
            Location::Attachment { offset, len } => {
                let end = offset
                    .checked_add(*len)
                    .filter(|&e| e <= data.len() as u64)
                    .ok_or(Error::Truncated("attached block past the end of the file"))?;
                &data[*offset as usize..end as usize]
            }
            Location::Inline(v) | Location::Embedded(v) => v,
        };
        let block: Cow<'a, [u8]> = match &self.compression {
            Some(c) => {
                if c.uncompressed != need {
                    return Err(Error::Malformed(format!(
                        "uncompressed size {} for an image of {need} bytes",
                        c.uncompressed
                    )));
                }
                Cow::Owned(codec::decompress(raw, c, MAX_BLOCK)?)
            }
            None => match &self.location {
                Location::Attachment { .. } => Cow::Borrowed(raw),
                Location::Inline(_) | Location::Embedded(_) => Cow::Owned(raw.to_vec()),
            },
        };
        if (block.len() as u64) < need {
            return Err(Error::Truncated("pixel data"));
        }
        Ok(block)
    }

    /// Plane (`channel`, `slice`) of the uncompressed block from [`Image::pixel_bytes`] as
    /// `width × height` real values, row-major.
    pub fn plane_from_bytes(&self, bytes: &[u8], channel: u32, slice: u32) -> Result<Vec<f32>> {
        if channel >= self.channels || slice >= self.depth {
            return Err(Error::Malformed("no such plane".into()));
        }
        if (bytes.len() as u64) < self.byte_len() {
            return Err(Error::Truncated("pixel data"));
        }
        let bps = self.sample.bytes();
        let n = self.width as usize * self.height as usize;
        let (start, stride) = match self.storage {
            Storage::Planar => (
                ((channel as usize * self.depth as usize) + slice as usize) * n * bps,
                bps,
            ),
            Storage::Normal => (
                (slice as usize * n * self.channels as usize + channel as usize) * bps,
                self.channels as usize * bps,
            ),
        };
        let mut out = Vec::new();
        out.try_reserve_exact(n).map_err(|_| Error::TooLarge)?;
        out.resize(n, 0.0);
        convert(
            bytes,
            start,
            stride,
            self.sample,
            self.byte_order == ByteOrder::Big,
            &mut out,
        );
        Ok(out)
    }

    /// Plane (`channel`, `slice`) as `width × height` real values, row-major.
    pub fn plane_f32(&self, data: &[u8], channel: u32, slice: u32) -> Result<Vec<f32>> {
        let b = self.pixel_bytes(data)?;
        self.plane_from_bytes(&b, channel, slice)
    }

    /// Every plane, channel-major then slice (`(c · depth + z) · w · h + y · w + x`).
    pub fn decode_all_f32(&self, data: &[u8]) -> Result<Vec<f32>> {
        let b = self.pixel_bytes(data)?;
        let mut out = Vec::new();
        out.try_reserve_exact(self.sample_count() as usize)
            .map_err(|_| Error::TooLarge)?;
        for c in 0..self.channels {
            for z in 0..self.depth {
                out.extend(self.plane_from_bytes(&b, c, z)?);
            }
        }
        Ok(out)
    }
}

/// Converts `out.len()` samples starting at byte `start`, `stride` bytes apart. The caller
/// has checked that they lie inside `bytes`.
fn convert(bytes: &[u8], start: usize, stride: usize, s: SampleFormat, big: bool, out: &mut [f32]) {
    const CHUNK: usize = 1 << 16;
    out.par_chunks_mut(CHUNK).enumerate().for_each(|(k, o)| {
        let b = &bytes[start + k * CHUNK * stride..];
        macro_rules! run {
            ($t:ty, $n:literal) => {{
                let f = |a: [u8; $n]| -> f32 {
                    (if big {
                        <$t>::from_be_bytes(a)
                    } else {
                        <$t>::from_le_bytes(a)
                    }) as f32
                };
                if stride == $n {
                    for (d, c) in o.iter_mut().zip(b.chunks_exact($n)) {
                        let mut a = [0u8; $n];
                        a.copy_from_slice(c);
                        *d = f(a);
                    }
                } else {
                    for (d, c) in o.iter_mut().zip(b.chunks(stride)) {
                        let mut a = [0u8; $n];
                        a.copy_from_slice(&c[..$n]);
                        *d = f(a);
                    }
                }
            }};
        }
        match s {
            SampleFormat::UInt8 => {
                for (d, c) in o.iter_mut().zip(b.iter().step_by(stride)) {
                    *d = f32::from(*c);
                }
            }
            SampleFormat::UInt16 => run!(u16, 2),
            SampleFormat::UInt32 => run!(u32, 4),
            SampleFormat::UInt64 => run!(u64, 8),
            SampleFormat::Float32 => run!(f32, 4),
            SampleFormat::Float64 => run!(f64, 8),
        }
    });
}
