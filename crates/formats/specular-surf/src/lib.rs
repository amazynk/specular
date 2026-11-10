//! `specular-surf`: areal surface topography readers for Specular.
//!
//! * **[`x3p`]: ISO 25178-72 X3P.** The zipped `main.xml` (at the root or in one folder),
//!   binary point data (`I`, `L`, `F`, `D`) or an inline data list, the validity bitmap, axis
//!   increments and offsets, instrument and date.
//! * **[`zygo`]: Zygo.** MetroPro `.dat` (phase data in waves scaled to metres, intensity
//!   buckets as extra planes) and Mx `.datx` (HDF5: `/Data/Surface` and `/Data/Intensity`
//!   with their units and no-data values, lateral resolution from `/Attributes`).
//! * **[`vk4`]: Keyence VK-X `.vk4`.** Height, laser intensity and colour images with the
//!   measurement's pixel pitch and height per digit.
//! * **[`plu`]: Sensofar `.plu`.** The confocal height map in micrometres and its grey or
//!   colour texture.
//! * **[`al3d`]: Alicona `.al3d`.** The depth image in metres and the texture planes.
//! * **[`sur`]: Digital Surf `.sur` / `.pro`.** Surfaces, profiles, intensity and RGB images
//!   (uncompressed or `DSCOMPRESSED`), several objects of one size as the planes of a page.
//!
//! Every reader yields a [`specular_em::dataset::Dataset`] of [`Field::Surface`] pages:
//! lateral axes in metres, heights in metres with NaN where the instrument measured nothing,
//! rows top-down as the vendor software shows them, and `levelling` set on height maps.
//! [`detect`] picks the reader from the first bytes; [`EXTENSIONS`] lists the file names.
//! Malformed input never panics.

use specular_em::dataset::{Axis, AxisKind, Dataset, Field, Lateral, Page, Plane};

pub mod al3d;
pub mod plu;
pub mod sur;
pub mod vk4;
pub mod x3p;
pub mod zygo;

/// Errors from the readers.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not a {0} file")]
    NotFormat(&'static str),
    #[error("truncated: {0}")]
    Truncated(&'static str),
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("image too large")]
    TooLarge,
    #[error("no image in the file")]
    NoImage,
}

pub type Result<T> = std::result::Result<T, Error>;

/// The file formats this crate reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    X3p,
    /// Zygo MetroPro `.dat`.
    Zygo,
    /// Zygo Mx `.datx` (HDF5).
    Datx,
    Vk4,
    Plu,
    Al3d,
    Sur,
}

/// File name extensions (lower case) of the formats: X3P, Zygo `.dat` / `.datx`, Keyence,
/// Sensofar, Alicona and Digital Surf (`.pro` is a Digital Surf profile).
pub const EXTENSIONS: &[&str] = &["x3p", "dat", "datx", "vk4", "plu", "al3d", "sur", "pro"];

/// The format of a file from its first bytes (at least 512 for Sensofar, which has no
/// magic number; X3P needs the whole file, see [`detect_file`]).
pub fn detect(head: &[u8]) -> Option<Format> {
    if zygo::probe(head) {
        Some(Format::Zygo)
    } else if vk4::probe(head) {
        Some(Format::Vk4)
    } else if al3d::probe(head) {
        Some(Format::Al3d)
    } else if sur::probe(head) {
        Some(Format::Sur)
    } else if zygo::probe_datx(head) {
        Some(Format::Datx)
    } else if plu::probe(head) {
        Some(Format::Plu)
    } else {
        None
    }
}

/// The format of a whole file: [`detect`], then X3P (a ZIP holding an ISO 5436-2 `main.xml`)
/// and Zygo `.datx` (an HDF5 file with `/Data/Surface`).
pub fn detect_file(file: &[u8]) -> Option<Format> {
    if x3p::probe_file(file) {
        return Some(Format::X3p);
    }
    match detect(file) {
        Some(Format::Datx) => zygo::probe_datx_file(file).then_some(Format::Datx),
        f => f,
    }
}

/// Reads a file of a known format.
pub fn read(format: Format, file: &[u8]) -> Result<Dataset> {
    match format {
        Format::X3p => x3p::read(file),
        Format::Zygo => zygo::read(file),
        Format::Datx => zygo::read_datx(file),
        Format::Vk4 => vk4::read(file),
        Format::Plu => plu::read(file),
        Format::Al3d => al3d::read(file),
        Format::Sur => sur::read(file),
    }
}

/// The largest image: 2^28 values (1 GiB of `f32`).
pub(crate) const MAX_VALUES: usize = 1 << 28;

/// `w × h` when both are nonzero and the product is within [`MAX_VALUES`].
pub(crate) fn area(w: usize, h: usize) -> Result<usize> {
    if w == 0 || h == 0 {
        return Err(Error::NoImage);
    }
    match w.checked_mul(h) {
        Some(n) if n <= MAX_VALUES => Ok(n),
        _ => Err(Error::TooLarge),
    }
}

/// `n` bytes at `at`, or a truncation error naming `what`.
pub(crate) fn slice<'a>(
    file: &'a [u8],
    at: usize,
    n: usize,
    what: &'static str,
) -> Result<&'a [u8]> {
    at.checked_add(n)
        .and_then(|end| file.get(at..end))
        .ok_or(Error::Truncated(what))
}

pub(crate) fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

pub(crate) fn i16le(b: &[u8], at: usize) -> Option<i16> {
    u16le(b, at).map(|v| v as i16)
}

pub(crate) fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

pub(crate) fn i32le(b: &[u8], at: usize) -> Option<i32> {
    u32le(b, at).map(|v| v as i32)
}

pub(crate) fn f32le(b: &[u8], at: usize) -> Option<f32> {
    u32le(b, at).map(f32::from_bits)
}

pub(crate) fn u16be(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

pub(crate) fn i16be(b: &[u8], at: usize) -> Option<i16> {
    u16be(b, at).map(|v| v as i16)
}

pub(crate) fn u32be(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

pub(crate) fn i32be(b: &[u8], at: usize) -> Option<i32> {
    u32be(b, at).map(|v| v as i32)
}

pub(crate) fn f32be(b: &[u8], at: usize) -> Option<f32> {
    u32be(b, at).map(f32::from_bits)
}

/// A fixed-size text field: up to the first NUL, Latin-1 bytes kept as their code points,
/// trimmed.
pub(crate) fn text(b: &[u8], at: usize, n: usize) -> String {
    let b = b.get(at..(at + n).min(b.len())).unwrap_or(&[]);
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    let s: String = b[..end].iter().map(|&c| char::from(c)).collect();
    s.trim().to_string()
}

/// A surface page skeleton: `w × h` pixels at `dx`, `dy` metres per pixel (unusable steps
/// leave the axis unset).
pub(crate) fn page(w: usize, h: usize, dx: f64, dy: f64) -> Page {
    let lateral = |step: f64| {
        let l = Lateral::new(0.0, step, "m");
        l.is_usable().then_some(l)
    };
    Page {
        field: Field::Surface,
        width: w,
        height: h,
        x: lateral(dx),
        y: lateral(dy),
        axis: Axis {
            kind: AxisKind::Channel,
            name: "Channel".into(),
            linear: None,
        },
        ..Page::default()
    }
}

/// A height plane in metres.
pub(crate) fn height(v: Vec<f32>) -> Plane {
    Plane::values("Height", "m", v)
}

/// Adds `(name, value)` to the page's metadata when the value is not empty.
pub(crate) fn meta(p: &mut Page, name: &str, value: impl Into<String>) {
    let v = value.into();
    if !v.is_empty() {
        p.meta.push((name.to_string(), v));
    }
}

/// A `Vec` of `n` values, failing instead of aborting when memory runs out.
pub(crate) fn alloc(n: usize) -> Result<Vec<f32>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| Error::TooLarge)?;
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_by_magic() {
        assert_eq!(detect(&[0x88, 0x1b, 0x03, 0x6f, 0, 0]), Some(Format::Zygo));
        assert_eq!(detect(b"VK4_\0\0\0\0"), Some(Format::Vk4));
        assert_eq!(detect(b"AliconaImaging\0\r\n"), Some(Format::Al3d));
        assert_eq!(detect(b"DIGITAL SURF\0\0"), Some(Format::Sur));
        assert_eq!(detect(b"nothing at all"), None);
        assert_eq!(detect_file(b""), None);
    }

    #[test]
    fn helpers() {
        assert_eq!(area(0, 3), Err(Error::NoImage));
        assert_eq!(area(1 << 20, 1 << 20), Err(Error::TooLarge));
        assert_eq!(text(b"ab \0cd", 0, 6), "ab");
    }
}
