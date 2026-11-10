//! The BPG file header and its extensions (BPG specification 0.9.5, section 3).

use crate::{AlphaKind, Animation, Chroma, ColourSpace, Error, Info};
use std::ops::Range;

/// The file magic, `BPG\xfb`.
pub const MAGIC: &[u8; 4] = b"BPG\xfb";

/// Extension tags.
const TAG_EXIF: u32 = 1;
const TAG_ICC: u32 = 2;
const TAG_XMP: u32 = 3;
const TAG_THUMBNAIL: u32 = 4;
const TAG_ANIMATION: u32 = 5;

/// Everything the header says, with where the parts are in the file.
#[derive(Clone, Debug)]
pub(crate) struct Header {
    pub info: Info,
    /// The thumbnail extension's bytes (a whole BPG file).
    pub thumbnail: Option<Range<usize>>,
    /// `hevc_header_and_data()`: from the first `hevc_header()` to the end of the picture
    /// data (clamped to the file).
    pub hevc: Range<usize>,
    /// Whether `picture_data_length` reached past the end of the file.
    pub cut_short: bool,
}

/// Reads a `ue7(32)`: big-endian groups of 7 bits, the high bit set on all bytes but the
/// last, shortest form only. Returns the value and the bytes it took.
pub(crate) fn ue7(data: &[u8], at: usize) -> Result<(u32, usize), Error> {
    let mut v: u64 = 0;
    for i in 0..5 {
        let Some(&b) = data.get(at + i) else {
            return Err(Error::Truncated("BPG header".into()));
        };
        if i == 0 && b == 0x80 {
            return Err(Error::Malformed(
                "BPG length not in its shortest form".into(),
            ));
        }
        v = (v << 7) | u64::from(b & 0x7f);
        if b & 0x80 == 0 {
            return u32::try_from(v)
                .map(|v| (v, i + 1))
                .map_err(|_| Error::Malformed("BPG number over 32 bits".into()));
        }
    }
    Err(Error::Malformed("BPG number over 32 bits".into()))
}

/// Parses the header and the extensions.
pub(crate) fn parse(data: &[u8]) -> Result<Header, Error> {
    if !data.starts_with(MAGIC) {
        return Err(Error::NotBpg);
    }
    if data.len() < 6 {
        return Err(Error::Truncated("BPG header".into()));
    }
    let f1 = data[4];
    let f2 = data[5];
    let pixel_format = f1 >> 5;
    let alpha1 = f1 & 0x10 != 0;
    let bit_depth = (f1 & 0x0f) + 8;
    let cs = f2 >> 4;
    let has_ext = f2 & 0x08 != 0;
    let alpha2 = f2 & 0x04 != 0;
    let limited_range = f2 & 0x02 != 0;
    let animated = f2 & 0x01 != 0;
    let (chroma, video_siting) = match pixel_format {
        0 => (Chroma::Gray, false),
        1 => (Chroma::C420, false),
        2 => (Chroma::C422, false),
        3 => (Chroma::C444, false),
        4 => (Chroma::C420, true),
        5 => (Chroma::C422, true),
        _ => {
            return Err(Error::Unsupported(
                "BPG pixel format reserved for later versions".into(),
            ));
        }
    };
    if bit_depth > 14 {
        return Err(Error::Unsupported(format!(
            "BPG at {bit_depth} bits per sample (the BPG specification stops at 14)"
        )));
    }
    let colour_space = match cs {
        0 => ColourSpace::YCbCr601,
        1 => ColourSpace::Rgb,
        2 => ColourSpace::YCgCo,
        3 => ColourSpace::YCbCr709,
        4 => ColourSpace::YCbCr2020,
        5 => {
            return Err(Error::Unsupported(
                "BPG BT.2020 constant-luminance colour (reserved by the BPG specification)".into(),
            ));
        }
        _ => {
            return Err(Error::Unsupported(
                "BPG colour space reserved for later versions".into(),
            ));
        }
    };
    let (alpha, cmyk) = match (alpha1, alpha2) {
        (false, false) => (AlphaKind::None, false),
        (true, false) => (AlphaKind::Straight, false),
        (true, true) => (AlphaKind::Premultiplied, false),
        (false, true) => (AlphaKind::None, true),
    };
    if chroma == Chroma::Gray && (cs != 0 || cmyk) {
        return Err(Error::Malformed("grey BPG with a colour space".into()));
    }
    let mut at = 6;
    let (width, n) = ue7(data, at)?;
    at += n;
    let (height, n) = ue7(data, at)?;
    at += n;
    if width == 0 || height == 0 {
        return Err(Error::Malformed("BPG picture without pixels".into()));
    }
    let (data_len, n) = ue7(data, at)?;
    at += n;
    let mut info = Info {
        width,
        height,
        chroma,
        video_siting,
        bit_depth,
        colour_space,
        limited_range,
        alpha,
        cmyk,
        animation: None,
        icc: None,
        exif: None,
        xmp: None,
        has_thumbnail: false,
    };
    let mut thumbnail = None;
    if has_ext {
        let (ext_len, n) = ue7(data, at)?;
        at += n;
        let end = at
            .checked_add(ext_len as usize)
            .filter(|&e| e <= data.len())
            .ok_or_else(|| Error::Truncated("BPG extensions".into()))?;
        while at < end {
            let (tag, n) = ue7(&data[..end], at)?;
            at += n;
            let (len, n) = ue7(&data[..end], at)?;
            at += n;
            let body = at
                .checked_add(len as usize)
                .filter(|&e| e <= end)
                .map(|e| at..e)
                .ok_or_else(|| Error::Truncated("BPG extension".into()))?;
            at = body.end;
            let bytes = &data[body.clone()];
            match tag {
                TAG_EXIF => info.exif = Some(bytes.to_vec()),
                TAG_ICC => info.icc = Some(bytes.to_vec()),
                TAG_XMP => info.xmp = Some(bytes.to_vec()),
                TAG_THUMBNAIL => {
                    info.has_thumbnail = bytes.starts_with(MAGIC);
                    thumbnail = info.has_thumbnail.then_some(body);
                }
                TAG_ANIMATION if animated => {
                    let (loop_count, n) = ue7(bytes, 0)?;
                    let (num, m) = ue7(bytes, n)?;
                    let (den, _) = ue7(bytes, n + m)?;
                    if num == 0 || den == 0 || loop_count > 0xffff || num > 0xffff || den > 0xffff {
                        return Err(Error::Malformed("BPG animation frame period".into()));
                    }
                    info.animation = Some(Animation {
                        loop_count,
                        frame_period_num: num,
                        frame_period_den: den,
                    });
                }
                _ => {}
            }
        }
        at = end;
    }
    if animated && info.animation.is_none() {
        return Err(Error::Malformed(
            "BPG animation without its animation control extension".into(),
        ));
    }
    let (hevc_end, cut_short) = if data_len == 0 {
        (data.len(), false)
    } else {
        match at.checked_add(data_len as usize) {
            Some(e) if e <= data.len() => (e, false),
            _ => (data.len(), true),
        }
    };
    if at >= hevc_end {
        return Err(Error::Truncated("BPG picture data".into()));
    }
    Ok(Header {
        info,
        thumbnail,
        hevc: at..hevc_end,
        cut_short,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ue7_examples() {
        assert_eq!(ue7(&[0x08], 0).unwrap(), (8, 1));
        assert_eq!(ue7(&[0x84, 0x1e], 0).unwrap(), (542, 2));
        assert_eq!(ue7(&[0xac, 0xbe, 0x17], 0).unwrap(), (728_855, 3));
        assert!(ue7(&[0x80, 0x01], 0).is_err());
        assert!(ue7(&[0x84], 0).is_err());
        assert!(ue7(&[0xff, 0xff, 0xff, 0xff, 0xff, 0x01], 0).is_err());
    }
}
