//! The JPEG XR file container (T.832 Annex A): a little-endian TIFF-like header ("II", 0xBC,
//! version) and IFD that point at the image codestream, an optional planar alpha codestream, and
//! the pixel format, orientation and metadata. A bare codestream ("WMPHOTO") is accepted too.

use crate::Error;

const PIXEL_FORMAT: u16 = 0xBC01;
const TRANSFORMATION: u16 = 0xBC02;
const IMAGE_OFFSET: u16 = 0xBCC0;
const IMAGE_BYTE_COUNT: u16 = 0xBCC1;
const ALPHA_OFFSET: u16 = 0xBCC2;
const ALPHA_BYTE_COUNT: u16 = 0xBCC3;
const ICC_PROFILE: u16 = 0x8773;

/// The first 15 bytes of every JPEG XR pixel format GUID.
const GUID_PREFIX: [u8; 15] = [
    0x24, 0xC3, 0xDD, 0x6F, 0x03, 0x4E, 0xFE, 0x4B, 0xB1, 0x85, 0x3D, 0x77, 0x76, 0x8D, 0xC9,
];

/// What the container says about the image.
#[derive(Clone, Debug, Default)]
pub(crate) struct Container {
    /// Byte range of the image codestream.
    pub image: (usize, usize),
    /// Byte range of the planar alpha codestream.
    pub alpha: Option<(usize, usize)>,
    /// Last byte of the pixel format GUID.
    pub pixel_format: Option<u8>,
    /// Exif-style orientation (1..=8).
    pub orientation: u8,
    pub icc: Option<(usize, usize)>,
}

/// Whether `head` starts a JPEG XR file or codestream.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"II\xBC\x01")
        || head.starts_with(b"II\xBC\x00")
        || head.starts_with(b"WMPHOTO\0")
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// The byte range of an IFD entry's value, clamped to the file.
fn value_range(d: &[u8], entry: usize) -> Option<(usize, usize)> {
    let ty = u16_at(d, entry + 2)?;
    let count = u32_at(d, entry + 4)? as usize;
    let size = match ty {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 => 4,
        5 | 10 | 12 => 8,
        _ => return None,
    };
    let len = count.checked_mul(size)?;
    let start = if len <= 4 {
        entry + 8
    } else {
        u32_at(d, entry + 8)? as usize
    };
    let end = start.checked_add(len)?;
    (end <= d.len()).then_some((start, end))
}

/// An integer entry value (BYTE, SHORT or LONG).
fn int_value(d: &[u8], entry: usize) -> Option<u64> {
    let ty = u16_at(d, entry + 2)?;
    match ty {
        1 => d.get(entry + 8).map(|&b| b as u64),
        3 => u16_at(d, entry + 8).map(u64::from),
        4 => u32_at(d, entry + 8).map(u64::from),
        _ => None,
    }
}

pub(crate) fn parse(d: &[u8]) -> Result<Container, Error> {
    if d.starts_with(b"WMPHOTO\0") {
        return Ok(Container {
            image: (0, d.len()),
            orientation: 1,
            ..Default::default()
        });
    }
    if !probe(d) {
        return Err(Error::NotJxr);
    }
    let ifd = u32_at(d, 4).ok_or_else(|| Error::Truncated("file header".into()))? as usize;
    let n = u16_at(d, ifd).ok_or_else(|| Error::Truncated("IFD".into()))? as usize;
    let mut c = Container {
        orientation: 1,
        ..Default::default()
    };
    let (mut img_off, mut img_len, mut a_off, mut a_len) = (None, None, None, None);
    for k in 0..n {
        let e = ifd + 2 + 12 * k;
        let Some(tag) = u16_at(d, e) else {
            return Err(Error::Truncated("IFD".into()));
        };
        match tag {
            PIXEL_FORMAT => {
                if let Some((s, t)) = value_range(d, e)
                    && t - s == 16
                    && d[s..s + 15] == GUID_PREFIX
                {
                    c.pixel_format = Some(d[s + 15]);
                }
            }
            TRANSFORMATION => {
                c.orientation = match int_value(d, e).unwrap_or(0) {
                    1 => 4,
                    2 => 2,
                    3 => 3,
                    4 => 6,
                    5 => 7,
                    6 => 5,
                    7 => 8,
                    _ => 1,
                };
            }
            IMAGE_OFFSET => img_off = int_value(d, e),
            IMAGE_BYTE_COUNT => img_len = int_value(d, e),
            ALPHA_OFFSET => a_off = int_value(d, e),
            ALPHA_BYTE_COUNT => a_len = int_value(d, e),
            ICC_PROFILE => c.icc = value_range(d, e),
            _ => {}
        }
    }
    let range = |off: Option<u64>, len: Option<u64>| -> Option<(usize, usize)> {
        let off = usize::try_from(off?).ok()?;
        if off >= d.len() {
            return None;
        }
        let end = match len {
            Some(l) if l > 0 => usize::try_from(l).ok()?.saturating_add(off).min(d.len()),
            _ => d.len(),
        };
        Some((off, end))
    };
    c.image =
        range(img_off, img_len).ok_or_else(|| Error::Malformed("no image codestream".into()))?;
    c.alpha = range(a_off, a_len);
    Ok(c)
}

/// Human-readable name of a pixel format (the last GUID byte).
pub(crate) fn pixel_format_name(pf: u8) -> &'static str {
    match pf {
        0x05 => "BlackWhite",
        0x08 => "8bppGray",
        0x09 => "16bppBGR555",
        0x0A => "16bppBGR565",
        0x0B => "16bppGray",
        0x0C => "24bppBGR",
        0x0D => "24bppRGB",
        0x0E => "32bppBGR",
        0x0F => "32bppBGRA",
        0x10 => "32bppPBGRA",
        0x11 => "32bppGrayFloat",
        0x12 => "48bppRGBFixedPoint",
        0x13 => "16bppGrayFixedPoint",
        0x14 => "32bppBGR101010",
        0x15 => "48bppRGB",
        0x16 => "64bppRGBA",
        0x17 => "64bppPRGBA",
        0x18 => "96bppRGBFixedPoint",
        0x19 => "128bppRGBAFloat",
        0x1A => "128bppPRGBAFloat",
        0x1B => "128bppRGBFloat",
        0x1C => "32bppCMYK",
        0x1D => "64bppRGBAFixedPoint",
        0x1E => "128bppRGBAFixedPoint",
        0x1F => "64bppCMYK",
        0x20..=0x2B => "N-channel",
        0x2C => "40bppCMYKAlpha",
        0x2D => "80bppCMYKAlpha",
        0x2E..=0x39 => "N-channel with alpha",
        0x3A => "64bppRGBAHalf",
        0x3B => "48bppRGBHalf",
        0x3D => "32bppRGBE",
        0x3E => "16bppGrayHalf",
        0x3F => "32bppGrayFixedPoint",
        0x40 => "64bppRGBFixedPoint",
        0x41 => "128bppRGBFixedPoint",
        0x42 => "64bppRGBHalf",
        0x43 => "80bppCMYKDIRECTAlpha",
        0x44..=0x53 => "YCC",
        0x54 | 0x55 => "CMYKDIRECT",
        0x56 => "CMYKDIRECTAlpha",
        _ => "unknown",
    }
}

/// Whether the pixel format stores premultiplied alpha.
pub(crate) fn premultiplied(pf: u8) -> bool {
    matches!(pf, 0x10 | 0x17 | 0x1A)
}
