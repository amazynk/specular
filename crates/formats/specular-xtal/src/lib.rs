//! Diffraction detector images: the frame formats crystallography and scattering beamlines
//! write, read into f32 planes with one geometry header.
//!
//! * **CBF / miniCBF** ([`cbf`]): imgCIF text with a MIME binary section; byte-offset and
//!   uncompressed data; the Pilatus/Eiger header in `_array_data.header_contents`.
//! * **ESRF EDF** ([`edf`]): `{ key = value ; }` blocks before each frame's data, several
//!   frames per file, each slice of a `Dim_3` volume a frame; every data type and byte
//!   order; gzip, zlib, bzip2 and byte-offset blocks; binning (`BSize_1/2`).
//! * **SMV** ([`smv`]): ADSC and Rigaku d*TREK (Saturn, HyPix, Pilatus at Rigaku) `.img`.
//! * **Bruker SFRM** ([`bruker`]): 80-column header records, version 86 (ASCII overflow
//!   records) and 100 (underflow and two overflow tables, baseline).
//! * **MarCCD** ([`marccd`]): a TIFF file whose frame header sits at byte 1024.
//! * **Rigaku R-AXIS** ([`raxis`]): a fixed big-endian header, 16-bit data with a
//!   photomultiplier-ratio high bit.
//!
//! Masked pixels (negative counts in Pilatus/Eiger files, EDF `Dummy` values) come out as
//! NaN. Several formats share `.img`: [`sniff`] goes by content only. Whole files may be
//! compressed by gzip, bzip2, xz or Unix compress (`.Z`, [`lzw`]).

pub mod bruker;
mod bytes;
pub mod cbf;
pub mod edf;
mod geometry;
pub mod lzw;
pub mod marccd;
pub mod raxis;
pub mod smv;

pub use geometry::{Geometry, HC_KEV_ANGSTROM, Ring, Scatter};

/// Largest image read, in pixels per frame.
pub const MAX_PIXELS: u64 = 1 << 28;
/// Largest decompressed frame block, bytes.
pub const MAX_BLOCK: usize = 1 << 31;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("file is truncated")]
    Truncated,
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, Error>;

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// The detector file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Cbf,
    Edf,
    Smv,
    Bruker,
    MarCcd,
    Raxis,
}

impl Kind {
    /// Short name for the info panel.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Cbf => "CBF",
            Kind::Edf => "ESRF EDF",
            Kind::Smv => "SMV",
            Kind::Bruker => "Bruker SFRM",
            Kind::MarCcd => "MarCCD",
            Kind::Raxis => "R-AXIS",
        }
    }
}

/// Bytes of the file start [`sniff`] wants to see every signature (MarCCD's is at 1028; CBF
/// files may have a few KiB of text before their binary section).
pub const SNIFF_HEAD: usize = 16 << 10;

/// The format of a file from its first bytes (up to [`SNIFF_HEAD`]; fewer bytes recognise
/// fewer formats: MarCCD needs 1044, a miniCBF without `###CBF` its binary section marker).
pub fn sniff(head: &[u8]) -> Option<Kind> {
    if smv::sniff(head) {
        Some(Kind::Smv)
    } else if edf::sniff(head) {
        Some(Kind::Edf)
    } else if cbf::sniff(head) {
        Some(Kind::Cbf)
    } else if bruker::sniff(head) {
        Some(Kind::Bruker)
    } else if marccd::sniff(head) {
        Some(Kind::MarCcd)
    } else if raxis::sniff(head) {
        Some(Kind::Raxis)
    } else {
        None
    }
}

/// A whole file compressed by gzip, bzip2, xz or Unix compress (`.cbf.gz`, `.edf.bz2`,
/// `.img.Z`, …), unpacked, with its memory reserved from the budget while the caller keeps
/// it; `None` when the file is not compressed.
pub fn uncompress(data: &[u8]) -> Result<Option<(Vec<u8>, specular_core::Reservation)>> {
    if lzw::sniff(data) {
        return lzw::decompress_reserved(data, MAX_BLOCK).map(Some);
    }
    // Reserved as the output grows (before each allocation), and handed over with it.
    specular_xcf::decompress_reserved(data).map_err(|e| match e {
        specular_xcf::Error::NeedsMemory(n) => Error::from(n),
        e => bad(format!("compressed file: {e}")),
    })
}

/// The detector format inside a whole-file gzip or Unix compress (`.Z`) stream, from its
/// first bytes.
pub fn sniff_gzip(data: &[u8]) -> Option<Kind> {
    if lzw::sniff(data) {
        return sniff(&lzw::decompress(data, SNIFF_HEAD).ok()?);
    }
    if !data.starts_with(&[0x1f, 0x8b]) {
        return None;
    }
    let head = specular_png::inflate_gzip_prefix(data, SNIFF_HEAD).ok()?;
    sniff(&head)
}

/// How samples are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sample {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
}

impl Sample {
    pub fn bytes(self) -> usize {
        match self {
            Sample::U8 | Sample::I8 => 1,
            Sample::U16 | Sample::I16 => 2,
            Sample::U32 | Sample::I32 | Sample::F32 => 4,
            Sample::U64 | Sample::I64 | Sample::F64 => 8,
        }
    }

    /// For the info panel, e.g. "32-bit signed integer".
    pub fn name(self) -> &'static str {
        match self {
            Sample::U8 => "8-bit unsigned integer",
            Sample::I8 => "8-bit signed integer",
            Sample::U16 => "16-bit unsigned integer",
            Sample::I16 => "16-bit signed integer",
            Sample::U32 => "32-bit unsigned integer",
            Sample::I32 => "32-bit signed integer",
            Sample::U64 => "64-bit unsigned integer",
            Sample::I64 => "64-bit signed integer",
            Sample::F32 => "32-bit float",
            Sample::F64 => "64-bit float",
        }
    }
}

/// Where one frame's samples are and how they are coded.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Coding {
    /// Plain samples at `offset`.
    Raw {
        offset: usize,
        sample: Sample,
        big: bool,
    },
    /// CBF byte-offset deltas at `offset`, `len` bytes, values of element type `sample`;
    /// the frame starts `skip` values in (a slice of an EDF volume).
    ByteOffset {
        offset: usize,
        len: usize,
        sample: Sample,
        skip: usize,
    },
    /// An EDF block compressed as a whole (gzip, zlib or bzip2), samples inside; the frame
    /// starts `skip` bytes into the unpacked block (a slice of an EDF volume).
    Packed {
        offset: usize,
        len: usize,
        method: edf::Packing,
        sample: Sample,
        big: bool,
        skip: usize,
    },
    /// Bruker frame with its overflow tables.
    Bruker(bruker::Layout),
    /// R-AXIS 16-bit big-endian samples with the photomultiplier-ratio bit.
    Raxis { offset: usize, ratio: f32 },
}

/// One frame of a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub(crate) coding: Coding,
    /// The frame's own header lines (EDF frames after the first), for the info panel.
    pub header: Vec<(String, String)>,
}

/// A parsed detector file: everything but the pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub kind: Kind,
    pub width: u32,
    pub height: u32,
    /// How the first frame's samples are stored.
    pub sample: Sample,
    /// Frames of the same size, file order.
    pub frames: Vec<Frame>,
    /// Header lines (key, value) of the first frame, file order.
    pub header: Vec<(String, String)>,
    pub geometry: Geometry,
    /// Detector name or model, as the header gives it.
    pub detector: Option<String>,
    /// When the frame was taken, as written.
    pub date: Option<String>,
    /// Negative values mean masked pixels (gaps, bad pixels): Pilatus and Eiger.
    pub mask_negative: bool,
    /// The value marking pixels without data (EDF `Dummy`), with its tolerance.
    pub dummy: Option<(f64, f64)>,
    /// A title or sample name.
    pub title: Option<String>,
}

impl Image {
    /// Parses the header(s) of a detector file of any [`Kind`].
    pub fn parse(data: &[u8]) -> Result<Image> {
        let kind = sniff(&data[..data.len().min(SNIFF_HEAD)])
            .ok_or_else(|| bad("not a known detector image"))?;
        let im = match kind {
            Kind::Cbf => cbf::parse(data)?,
            Kind::Edf => edf::parse(data)?,
            Kind::Smv => smv::parse(data)?,
            Kind::Bruker => bruker::parse(data)?,
            Kind::MarCcd => marccd::parse(data)?,
            Kind::Raxis => raxis::parse(data)?,
        };
        if im.width == 0
            || im.height == 0
            || u64::from(im.width) * u64::from(im.height) > MAX_PIXELS
        {
            return Err(bad(format!("image size {} × {}", im.width, im.height)));
        }
        if im.frames.is_empty() {
            return Err(bad("no frames"));
        }
        Ok(im)
    }

    /// How the first frame is coded, for the info panel: "byte offset", "gzip", "zlib",
    /// "bzip2", "overflow tables", "PMT ratio" or "uncompressed".
    pub fn compression(&self) -> &'static str {
        match self.frames.first().map(|f| &f.coding) {
            Some(Coding::ByteOffset { .. }) => "byte offset",
            Some(Coding::Packed { method, .. }) => match method {
                edf::Packing::Gzip => "gzip",
                edf::Packing::Zlib => "zlib",
                edf::Packing::Bzip2 => "bzip2",
            },
            Some(Coding::Bruker(_)) => "overflow tables",
            Some(Coding::Raxis { .. }) => "PMT ratio",
            _ => "uncompressed",
        }
    }

    /// Pixels per frame.
    pub fn pixels(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// Frame `k`'s values, row by row from the top; masked pixels NaN.
    pub fn frame(&self, data: &[u8], k: usize) -> Result<Vec<f32>> {
        let f = self
            .frames
            .get(k)
            .ok_or_else(|| bad(format!("no frame {k}")))?;
        let n = self.pixels();
        let mut v = match &f.coding {
            &Coding::Raw {
                offset,
                sample,
                big,
            } => {
                let len = n * sample.bytes();
                let src = data
                    .get(offset..offset.checked_add(len).ok_or(Error::Truncated)?)
                    .ok_or(Error::Truncated)?;
                bytes::samples(src, sample, big)
            }
            &Coding::ByteOffset {
                offset,
                len,
                sample,
                skip,
            } => {
                let src = data
                    .get(offset..offset.saturating_add(len).min(data.len()))
                    .ok_or(Error::Truncated)?;
                let mut v = cbf::byte_offset(src, skip.saturating_add(n), sample)?;
                if skip > 0 {
                    v.drain(..skip.min(v.len()));
                }
                v
            }
            &Coding::Packed {
                offset,
                len,
                method,
                sample,
                big,
                skip,
            } => {
                let src = data
                    .get(offset..offset.saturating_add(len).min(data.len()))
                    .ok_or(Error::Truncated)?;
                let want = n * sample.bytes();
                let raw = edf::unpack(src, method, skip.saturating_add(want))?;
                let block = raw
                    .get(skip..skip.saturating_add(want))
                    .ok_or(Error::Truncated)?;
                bytes::samples(block, sample, big)
            }
            Coding::Bruker(l) => bruker::read(data, l, n)?,
            &Coding::Raxis { offset, ratio } => raxis::read(data, offset, n, ratio)?,
        };
        if self.mask_negative {
            for x in &mut v {
                if *x < 0.0 {
                    *x = f32::NAN;
                }
            }
        }
        if let Some((dummy, tol)) = self.dummy {
            let tol = tol.max(f64::from(f32::EPSILON) * dummy.abs());
            for x in &mut v {
                if (f64::from(*x) - dummy).abs() <= tol {
                    *x = f32::NAN;
                }
            }
        }
        Ok(v)
    }
}

/// The first number in `s` (skipping any text before it), e.g. 0.3 in "0.30000 m".
pub(crate) fn first_number(s: &str) -> Option<f64> {
    numbers(s).next()
}

/// The numbers in `s`, in order (separated by anything that is not part of a number).
pub(crate) fn numbers(s: &str) -> impl Iterator<Item = f64> + '_ {
    let b = s.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < b.len() {
            let start = i;
            let c = b[i];
            let starts = c.is_ascii_digit()
                || ((c == b'-' || c == b'+' || c == b'.')
                    && b.get(i + 1)
                        .is_some_and(|n| n.is_ascii_digit() || *n == b'.'));
            if !starts {
                i += 1;
                continue;
            }
            i += 1;
            while i < b.len() {
                let c = b[i];
                let exp_sign = (c == b'-' || c == b'+') && matches!(b[i - 1], b'e' | b'E');
                if c.is_ascii_digit() || c == b'.' || c == b'e' || c == b'E' || exp_sign {
                    i += 1;
                } else {
                    break;
                }
            }
            let mut t = &s[start..i];
            // "1.5e" or "deg." style tails.
            while let Some(x) = t.strip_suffix(['e', 'E', '.', '-', '+']) {
                if x.parse::<f64>().is_ok() || t.parse::<f64>().is_err() {
                    t = x;
                } else {
                    break;
                }
            }
            if let Ok(v) = t.parse::<f64>() {
                return Some(v);
            }
        }
        None
    })
}

/// A header value that is a positive number (files write 0 for unknown).
pub(crate) fn positive(v: Option<f64>) -> Option<f64> {
    v.filter(|v| v.is_finite() && *v > 0.0)
}

/// A header value that is a finite number.
pub(crate) fn finite(v: Option<f64>) -> Option<f64> {
    v.filter(|v| v.is_finite())
}

/// `s` trimmed, or `None` when empty.
pub(crate) fn text(s: &str) -> Option<String> {
    let t = s.trim().trim_matches('\0').trim();
    (!t.is_empty()).then(|| t.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_scanning() {
        assert_eq!(first_number("0.30000 m"), Some(0.3));
        assert_eq!(first_number("172e-6 m x 172e-6 m"), Some(172e-6));
        let v: Vec<f64> = numbers("(1230.10, 1310.50) pixels").collect();
        assert_eq!(v, [1230.1, 1310.5]);
        assert_eq!(first_number("74.3999 deg."), Some(74.3999));
        assert_eq!(first_number("-0.0004 deg."), Some(-0.0004));
        assert_eq!(first_number("PILATUS"), None);
        let v: Vec<f64> = numbers("1.0 1.541870").collect();
        assert_eq!(v, [1.0, 1.54187]);
        assert_eq!(first_number("5.4e+003"), Some(5400.0));
    }

    /// Whole-file gzip, bzip2 and xz (48 MiB of zeros each, from a few bytes to 48 KiB):
    /// the output is reserved before it is allocated, so a small budget stops the stream
    /// and a large one hands over exactly one reservation covering every byte held.
    #[test]
    fn whole_file_streams_reserve_as_they_grow() {
        use specular_core::budget::{MemoryBudget, with_budget};
        use std::sync::Arc;
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
        for ext in ["gz", "bz2", "xz"] {
            let z = std::fs::read(format!("{dir}zeros48m.{ext}")).unwrap();
            let small = Arc::new(MemoryBudget::fixed(16 << 20));
            let r = with_budget(small.clone(), || uncompress(&z));
            assert!(matches!(r, Err(Error::NeedsMemory(_))), "{ext}: {r:?}");
            assert_eq!(small.reserved(), 0, "{ext}");
            let b = Arc::new(MemoryBudget::fixed(256 << 20));
            let (out, held) = with_budget(b.clone(), || uncompress(&z)).unwrap().unwrap();
            assert_eq!(out.len(), 48 << 20, "{ext}");
            assert!(out.iter().all(|&v| v == 0), "{ext}");
            // Every byte allocated is reserved, and only by the handed-over reservation
            // (the block vector of bzip2 is given back).
            assert!(held.bytes() >= out.capacity() as u64, "{ext}: {held:?}");
            assert!(held.bytes() <= 2 * out.capacity() as u64, "{ext}: {held:?}");
            assert_eq!(b.reserved(), held.bytes(), "{ext}");
            drop((out, held));
            assert_eq!(b.reserved(), 0, "{ext}");
        }
    }
}
