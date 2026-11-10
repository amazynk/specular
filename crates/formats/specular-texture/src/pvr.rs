//! PowerVR PVR version 3 container: a 52-byte header (pixel format, colour space, channel
//! type, size, surfaces, faces, mips), metadata (orientation), then per mip level its
//! surfaces, faces and depth slices.

use crate::{Container, Error, Format, Layout, Level, Result, Supercompression, Texture, ktx2};

const HEADER: usize = 52;

/// Channel types with a sign (signed byte / short / integer, normalised or not, and float).
fn signed_type(t: u32) -> bool {
    matches!(t, 1 | 3 | 5 | 7 | 9 | 11 | 12)
}

/// Compressed format id → format.
fn compressed(id: u32, channel_type: u32) -> Option<Format> {
    use Format::*;
    let signed = signed_type(channel_type);
    let pvrtc = |two_bpp, alpha| Pvrtc { two_bpp, alpha };
    Some(match id {
        0 => pvrtc(true, false),
        1 => pvrtc(true, true),
        2 => pvrtc(false, false),
        3 => pvrtc(false, true),
        6 => Etc1,
        7 => Bc1 { alpha: true },
        8 | 9 => Bc2,
        10 | 11 => Bc3,
        12 => Bc4 { signed },
        13 => Bc5 { signed },
        14 => Bc6h { signed },
        15 => Bc7,
        22 => Etc2Rgb,
        23 => Etc2Rgba,
        24 => Etc2Rgba1,
        25 => EacR11 { signed },
        26 => EacRg11 { signed },
        27..=40 => ktx2::astc((id - 27) as usize, matches!(channel_type, 12 | 13)),
        _ => return None,
    })
}

fn compressed_name(id: u32) -> String {
    match id {
        4 | 5 => "PVRTC2".into(),
        41..=50 => "ASTC 3D".into(),
        _ => format!("PVR pixel format {id}"),
    }
}

/// Uncompressed format: channel names (low word, one byte each) and bits (high word).
fn uncompressed(pf: u64, channel_type: u32) -> Option<Format> {
    let names: Vec<u8> = pf.to_le_bytes()[..4]
        .iter()
        .copied()
        .take_while(|&c| c != 0)
        .collect();
    let bits: Vec<u8> = pf.to_le_bytes()[4..8].to_vec();
    let n = names.len();
    if n == 0 || bits[..n].iter().any(|&b| b != bits[0]) || bits[n..].iter().any(|&b| b != 0) {
        return None;
    }
    let float = matches!(channel_type, 12 | 13);
    match (bits[0], float, names.as_slice()) {
        (8, false, _) if matches!(channel_type, 0 | 2) => {
            let mut masks = [0u32; 4];
            let mut grey = false;
            for (i, &c) in names.iter().enumerate() {
                let slot = match c {
                    b'r' => 0,
                    b'g' => 1,
                    b'b' => 2,
                    b'a' => 3,
                    b'l' | b'i' => {
                        grey = true;
                        0
                    }
                    _ => return None,
                };
                masks[slot] = 0xff << (8 * i);
            }
            Some(Format::Masked {
                bytes: n as u8,
                masks,
                grey,
            })
        }
        (16, false, b"r" | b"rg" | b"rgb" | b"rgba") if matches!(channel_type, 4 | 6) => {
            Some(Format::Unorm16 { channels: n as u8 })
        }
        (16, true, b"r" | b"rg" | b"rgb" | b"rgba") => Some(Format::Half { channels: n as u8 }),
        (32, true, b"r" | b"rg" | b"rgb" | b"rgba") => Some(Format::Float { channels: n as u8 }),
        _ => None,
    }
}

pub(crate) fn parse(d: &[u8]) -> Result<Texture> {
    if d.len() < HEADER {
        return Err(Error::Truncated("PVR header"));
    }
    let swap = d.starts_with(b"\x03RVP");
    let word = move |p: usize| -> u32 {
        d.get(p..p + 4).map_or(0, |b| {
            let b = [b[0], b[1], b[2], b[3]];
            if swap {
                u32::from_be_bytes(b)
            } else {
                u32::from_le_bytes(b)
            }
        })
    };
    let flags = word(4);
    let pf = u64::from(word(8)) | u64::from(word(12)) << 32;
    let colour_space = word(16);
    let channel_type = word(20);
    let height = word(24).max(1);
    let width = word(28);
    let depth = word(32).max(1);
    let layers = word(36).max(1);
    let faces = word(40).max(1);
    let mips = word(44).max(1);
    let meta_len = word(48) as usize;
    let format = if pf >> 32 == 0 {
        compressed(pf as u32, channel_type)
            .ok_or_else(|| Error::Unsupported(compressed_name(pf as u32)))?
    } else {
        if swap && pf >> 32 & 0xff > 8 {
            return Err(Error::Unsupported("big-endian PVR pixels".into()));
        }
        uncompressed(pf, channel_type).ok_or_else(|| {
            let names = String::from_utf8_lossy(&pf.to_le_bytes()[..4]).replace('\0', "");
            Error::Unsupported(format!("PVR pixel format {names}"))
        })?
    };
    if mips > 32 {
        return Err(Error::Malformed("PVR level count"));
    }
    // Metadata: (FourCC, key, size, data) entries; key 3 of 'PVR\x03' is the orientation, its
    // second byte non-zero for rows going up.
    let mut flip = false;
    let meta_end = HEADER.saturating_add(meta_len).min(d.len());
    let mut p = HEADER;
    while p + 12 <= meta_end {
        let (fourcc, key, size) = (word(p), word(p + 4), word(p + 8) as usize);
        if fourcc == 0x0352_5650 && key == 3 && size >= 2 {
            flip = d.get(p + 13).is_some_and(|&y| y != 0);
        }
        p = p.saturating_add(12).saturating_add(size);
    }
    // DXT2 and DXT4 are premultiplied; so is anything flagged.
    let premultiplied =
        (flags & 2 != 0 || matches!(pf, 8 | 10)) && format.has_alpha() && !format.is_float();
    let images = u64::from(layers) * u64::from(faces);
    let mut levels = Vec::new();
    let mut off = (HEADER + meta_len) as u64;
    for k in 0..mips {
        let (w, h) = ((width >> k).max(1), (height >> k).max(1));
        let dk = u64::from((depth >> k).max(1));
        if off >= d.len() as u64 {
            break;
        }
        let total = crate::image_bytes(format, w, h)
            .saturating_mul(images)
            .saturating_mul(dk);
        let end = off.saturating_add(total);
        levels.push(Level {
            width: w,
            height: h,
            range: off as usize..end.min(d.len() as u64) as usize,
            uncompressed_len: usize::try_from(total).unwrap_or(usize::MAX),
        });
        off = end;
    }
    Ok(Texture {
        container: Container::Pvr,
        format,
        width,
        height,
        depth,
        layers,
        faces,
        srgb: colour_space == 1,
        flip_y: flip,
        premultiplied,
        levels,
        supercompression: Supercompression::None,
        layout: Layout::PerLevel,
        image: 0,
        row_align: 1,
        basis: None,
        seek: None,
    })
}
