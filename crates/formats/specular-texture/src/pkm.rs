//! PKM, the ETC file of Ericsson's etcpack (and of Android's `etc1tool`): a 16-byte
//! big-endian header (`PKM `, the version `10` or `20`, the data format, the width and height
//! padded to whole 4×4 blocks, then the image's own width and height) and the blocks, row by
//! row, with no mip chain.
//!
//! Formats: 0 ETC1 RGB; in version 2 also 1 ETC2 RGB, 2 and 3 ETC2 RGBA (an EAC alpha
//! block), 4 ETC2 RGB with punch-through alpha, 5 and 6 EAC R11 and RG11, 7 and 8 their
//! signed forms, and 9–11 the sRGB forms of 1, 3 and 4.

use crate::{Container, Error, Format, Layout, Level, Result, Supercompression, Texture};

pub(crate) const MAGIC: &[u8; 4] = b"PKM ";
const HEADER: usize = 16;

pub(crate) fn is_pkm(head: &[u8]) -> bool {
    head.len() >= 6 && head.starts_with(MAGIC) && (&head[4..6] == b"10" || &head[4..6] == b"20")
}

fn u16be(d: &[u8], o: usize) -> u32 {
    u32::from(u16::from_be_bytes([d[o], d[o + 1]]))
}

pub(crate) fn parse(d: &[u8]) -> Result<Texture> {
    if d.len() < HEADER {
        return Err(Error::Truncated("PKM header"));
    }
    let v2 = &d[4..6] == b"20";
    let code = u16be(d, 6);
    let (ext_w, ext_h, width, height) = (u16be(d, 8), u16be(d, 10), u16be(d, 12), u16be(d, 14));
    let (format, srgb) = match (v2, code) {
        (_, 0) => (Format::Etc1, false),
        (true, 1) => (Format::Etc2Rgb, false),
        (true, 2 | 3) => (Format::Etc2Rgba, false),
        (true, 4) => (Format::Etc2Rgba1, false),
        (true, 5) => (Format::EacR11 { signed: false }, false),
        (true, 6) => (Format::EacRg11 { signed: false }, false),
        (true, 7) => (Format::EacR11 { signed: true }, false),
        (true, 8) => (Format::EacRg11 { signed: true }, false),
        (true, 9) => (Format::Etc2Rgb, true),
        (true, 10) => (Format::Etc2Rgba, true),
        (true, 11) => (Format::Etc2Rgba1, true),
        (_, c) => {
            return Err(Error::Unsupported(format!(
                "PKM data format {c} is not an ETC1, ETC2 or EAC format"
            )));
        }
    };
    if ext_w < width
        || ext_h < height
        || ext_w != width.div_ceil(4) * 4
        || ext_h != height.div_ceil(4) * 4
    {
        return Err(Error::Malformed("PKM padded size"));
    }
    let (_, _, block) = format.block();
    let size = (ext_w as usize / 4) * (ext_h as usize / 4) * block;
    let end = HEADER.saturating_add(size).min(d.len());
    Ok(Texture {
        container: Container::Pkm,
        format,
        width,
        height,
        depth: 1,
        layers: 1,
        faces: 1,
        srgb,
        flip_y: false,
        premultiplied: false,
        levels: vec![Level {
            width,
            height,
            range: HEADER..end,
            uncompressed_len: size,
        }],
        supercompression: Supercompression::None,
        layout: Layout::PerLevel,
        image: 0,
        row_align: 1,
        basis: None,
        seek: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecodeOptions;

    #[test]
    fn header_and_formats() {
        let mut d = b"PKM 20".to_vec();
        for v in [1u16, 8, 4, 6, 3] {
            d.extend_from_slice(&v.to_be_bytes());
        }
        d.extend_from_slice(&[0u8; 16]);
        assert!(is_pkm(&d));
        let t = Texture::open(&d).unwrap();
        assert_eq!((t.width, t.height, t.format), (6, 3, Format::Etc2Rgb));
        let out = t.decode(&d, &DecodeOptions::default()).unwrap();
        assert_eq!((out.width, out.height), (6, 3));
        d[7] = 12;
        assert!(matches!(Texture::open(&d), Err(Error::Unsupported(_))));
        d[7] = 1;
        d[9] = 12;
        assert!(Texture::open(&d).is_err());
    }
}
