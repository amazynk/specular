//! KTX (version 1) container: a GL-style header (internal format, or format and type for
//! uncompressed data), key/value data (orientation), then per mip level an image size word and
//! its images ordered array element, face, z slice. Either byte order is read; uncompressed
//! data (rows padded to 4 bytes) with multi-byte components is accepted only in the reader's (little-endian) order.

use crate::{Container, Error, Format, Layout, Level, Result, Supercompression, Texture, ktx2};

pub(crate) const IDENTIFIER: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x31, 0x31, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];
const HEADER: usize = 64;

fn masked(bytes: u8, masks: [u32; 4], grey: bool) -> Format {
    Format::Masked { bytes, masks, grey }
}

/// glInternalFormat of compressed data → (format, sRGB).
fn compressed(code: u32) -> Option<(Format, bool)> {
    use Format::*;
    let pvrtc = |two_bpp, alpha| Pvrtc { two_bpp, alpha };
    Some(match code {
        0x8D64 => (Etc1, false),
        0x9270 => (EacR11 { signed: false }, false),
        0x9271 => (EacR11 { signed: true }, false),
        0x9272 => (EacRg11 { signed: false }, false),
        0x9273 => (EacRg11 { signed: true }, false),
        0x9274 => (Etc2Rgb, false),
        0x9275 => (Etc2Rgb, true),
        0x9276 => (Etc2Rgba1, false),
        0x9277 => (Etc2Rgba1, true),
        0x9278 => (Etc2Rgba, false),
        0x9279 => (Etc2Rgba, true),
        0x8C00 => (pvrtc(false, false), false),
        0x8C01 => (pvrtc(true, false), false),
        0x8C02 => (pvrtc(false, true), false),
        0x8C03 => (pvrtc(true, true), false),
        0x8A54 => (pvrtc(true, false), true),
        0x8A55 => (pvrtc(false, false), true),
        0x8A56 => (pvrtc(true, true), true),
        0x8A57 => (pvrtc(false, true), true),
        // GL's ASTC enums serve both profiles; the LDR decode is used (HDR blocks show the
        // error colour).
        0x93B0..=0x93BD => (ktx2::astc((code - 0x93B0) as usize, false), false),
        0x93D0..=0x93DD => (ktx2::astc((code - 0x93D0) as usize, false), true),
        0x83F0 => (Bc1 { alpha: false }, false),
        0x83F1 => (Bc1 { alpha: true }, false),
        0x83F2 => (Bc2, false),
        0x83F3 => (Bc3, false),
        0x8C4C => (Bc1 { alpha: false }, true),
        0x8C4D => (Bc1 { alpha: true }, true),
        0x8C4E => (Bc2, true),
        0x8C4F => (Bc3, true),
        0x8DBB => (Bc4 { signed: false }, false),
        0x8DBC => (Bc4 { signed: true }, false),
        0x8DBD => (Bc5 { signed: false }, false),
        0x8DBE => (Bc5 { signed: true }, false),
        0x8E8C => (Bc7, false),
        0x8E8D => (Bc7, true),
        0x8E8E => (Bc6h { signed: true }, false),
        0x8E8F => (Bc6h { signed: false }, false),
        _ => return None,
    })
}

/// glFormat and glType of uncompressed data → format.
fn uncompressed(format: u32, ty: u32) -> Option<Format> {
    use Format::*;
    let channels = match format {
        0x1903 | 0x1906 | 0x1909 => 1, // RED, ALPHA, LUMINANCE
        0x8227 | 0x190A => 2,          // RG, LUMINANCE_ALPHA
        0x1907 => 3,                   // RGB
        0x1908 | 0x80E1 => 4,          // RGBA, BGRA
        _ => return None,
    };
    Some(match (ty, format) {
        // UNSIGNED_BYTE
        (0x1401, 0x1903 | 0x1906 | 0x1909) => masked(1, [0xff, 0, 0, 0], true),
        (0x1401, 0x190A) => masked(2, [0xff, 0, 0, 0xff00], true),
        (0x1401, 0x8227) => masked(2, [0xff, 0xff00, 0, 0], false),
        (0x1401, 0x1907) => masked(3, [0xff, 0xff00, 0xff_0000, 0], false),
        (0x1401, 0x1908) => masked(4, [0xff, 0xff00, 0xff_0000, 0xff00_0000], false),
        (0x1401, 0x80E1) => masked(4, [0xff_0000, 0xff00, 0xff, 0xff00_0000], false),
        // UNSIGNED_SHORT_5_6_5, _4_4_4_4, _5_5_5_1
        (0x8363, 0x1907) => masked(2, [0xf800, 0x7e0, 0x1f, 0], false),
        (0x8033, 0x1908) => masked(2, [0xf000, 0xf00, 0xf0, 0xf], false),
        (0x8034, 0x1908) => masked(2, [0xf800, 0x7c0, 0x3e, 0x1], false),
        (0x1403, 0x1903 | 0x8227 | 0x1907 | 0x1908) => Unorm16 { channels },
        (0x140B | 0x8D61, 0x1903 | 0x8227 | 0x1907 | 0x1908) => Half { channels },
        (0x1406, 0x1903 | 0x8227 | 0x1907 | 0x1908) => Float { channels },
        _ => return None,
    })
}

/// Whether `KTXorientation` says rows go bottom-up ("S=r,T=u").
fn flip_y(kvd: &[u8], word: impl Fn(&[u8], usize) -> u32) -> bool {
    let mut p = 0;
    while p + 4 <= kvd.len() {
        let len = word(kvd, p) as usize;
        let Some(kv) = kvd.get(p + 4..(p + 4).saturating_add(len)) else {
            break;
        };
        if let Some(z) = kv.iter().position(|&b| b == 0)
            && &kv[..z] == b"KTXorientation"
        {
            return kv[z..].windows(3).any(|w| w == b"T=u");
        }
        p += 4 + len.div_ceil(4) * 4;
    }
    false
}

pub(crate) fn parse(d: &[u8]) -> Result<Texture> {
    if d.len() < HEADER {
        return Err(Error::Truncated("KTX header"));
    }
    let swap = match d[12..16] {
        [1, 2, 3, 4] => false,
        [4, 3, 2, 1] => true,
        _ => return Err(Error::Malformed("KTX endianness")),
    };
    let word = move |d: &[u8], p: usize| -> u32 {
        d.get(p..p + 4).map_or(0, |b| {
            let b = [b[0], b[1], b[2], b[3]];
            if swap {
                u32::from_be_bytes(b)
            } else {
                u32::from_le_bytes(b)
            }
        })
    };
    let gl_type = word(d, 16);
    let type_size = word(d, 20);
    let gl_format = word(d, 24);
    let internal = word(d, 28);
    let width = word(d, 36);
    let height = word(d, 40).max(1);
    let depth = word(d, 44).max(1);
    let array = word(d, 48);
    let faces = word(d, 52).max(1);
    let mips = word(d, 56).max(1);
    let kvd_len = word(d, 60) as usize;
    let (format, srgb) = if gl_type == 0 {
        compressed(internal)
            .ok_or_else(|| Error::Unsupported(format!("KTX internal format {internal:#x}")))?
    } else {
        if swap && type_size > 1 {
            return Err(Error::Unsupported("big-endian KTX pixels".into()));
        }
        let f = uncompressed(gl_format, gl_type).ok_or_else(|| {
            Error::Unsupported(format!("KTX format {gl_format:#x} type {gl_type:#x}"))
        })?;
        // GL_SRGB8, GL_SRGB8_ALPHA8.
        (f, matches!(internal, 0x8C41 | 0x8C43))
    };
    if mips > 32 {
        return Err(Error::Malformed("KTX level count"));
    }
    let flip = d
        .get(HEADER..HEADER.saturating_add(kvd_len))
        .is_some_and(|k| flip_y(k, word));
    let layers = array.max(1);
    // A cube map that is not an array stores the image size of one face.
    let per_face = faces == 6 && array == 0;
    let mut levels = Vec::new();
    let mut off = HEADER as u64 + kvd_len as u64;
    for k in 0..mips {
        let (w, h) = ((width >> k).max(1), (height >> k).max(1));
        if off + 4 > d.len() as u64 {
            break;
        }
        let size = u64::from(word(d, off as usize));
        let total = if per_face { size * 6 } else { size };
        let start = off + 4;
        levels.push(Level {
            width: w,
            height: h,
            range: start as usize..start.saturating_add(total).min(d.len() as u64) as usize,
            uncompressed_len: usize::try_from(total).unwrap_or(usize::MAX),
        });
        // Faces and levels are padded to 4 bytes.
        let face_pad = if per_face {
            size.div_ceil(4) * 4 * 6
        } else {
            total
        };
        off = start.saturating_add(face_pad.div_ceil(4) * 4);
    }
    Ok(Texture {
        container: Container::Ktx1,
        format,
        width,
        height,
        depth,
        layers,
        faces,
        srgb,
        flip_y: flip,
        premultiplied: false,
        levels,
        supercompression: Supercompression::None,
        layout: Layout::PerLevel,
        image: 0,
        row_align: 4,
        basis: None,
        seek: None,
    })
}
