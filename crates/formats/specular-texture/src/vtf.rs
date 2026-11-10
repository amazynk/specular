//! Valve Texture Format (VTF 7.0–7.5): a header with the high-resolution image format, frame,
//! face and mip counts, then (7.3 and later) a resource directory locating the image data, or
//! (earlier) the low-resolution thumbnail followed by it. Mip levels are stored smallest
//! first; each holds its frames, faces and depth slices.

use crate::{
    Container, Error, Format, Layout, Level, Result, Supercompression, Texture, image_bytes,
};

fn masked(bytes: u8, masks: [u32; 4]) -> Format {
    Format::Masked {
        bytes,
        masks,
        grey: false,
    }
}

fn grey(bytes: u8, masks: [u32; 4]) -> Format {
    Format::Masked {
        bytes,
        masks,
        grey: true,
    }
}

/// VTF image format → format.
fn format(code: u32) -> Option<Format> {
    use Format::*;
    Some(match code {
        0 => masked(4, [0xff, 0xff00, 0xff_0000, 0xff00_0000]),
        1 => masked(4, [0xff00_0000, 0xff_0000, 0xff00, 0xff]),
        2 | 9 => masked(3, [0xff, 0xff00, 0xff_0000, 0]),
        3 | 10 => masked(3, [0xff_0000, 0xff00, 0xff, 0]),
        4 => masked(2, [0x1f, 0x7e0, 0xf800, 0]),
        5 | 8 => grey(1, [0xff, 0, 0, 0]),
        6 => grey(2, [0xff, 0, 0, 0xff00]),
        11 => masked(4, [0xff00, 0xff_0000, 0xff00_0000, 0xff]),
        12 => masked(4, [0xff_0000, 0xff00, 0xff, 0xff00_0000]),
        13 => Bc1 { alpha: false },
        14 => Bc2,
        15 => Bc3,
        16 => masked(4, [0xff_0000, 0xff00, 0xff, 0]),
        17 => masked(2, [0xf800, 0x7e0, 0x1f, 0]),
        18 => masked(2, [0x7c00, 0x3e0, 0x1f, 0]),
        19 => masked(2, [0xf00, 0xf0, 0xf, 0xf000]),
        20 => Bc1 { alpha: true },
        21 => masked(2, [0x7c00, 0x3e0, 0x1f, 0x8000]),
        22 => Snorm8 { channels: 2 },
        23 => Snorm8 { channels: 4 },
        24 => Half { channels: 4 },
        25 => Unorm16 { channels: 4 },
        27 => Float { channels: 1 },
        28 => Float { channels: 3 },
        29 => Float { channels: 4 },
        _ => return None,
    })
}

fn u16_at(d: &[u8], p: usize) -> u32 {
    d.get(p..p + 2)
        .map_or(0, |b| u32::from(u16::from_le_bytes([b[0], b[1]])))
}

fn u32_at(d: &[u8], p: usize) -> u32 {
    d.get(p..p + 4)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

pub(crate) fn parse(d: &[u8]) -> Result<Texture> {
    if d.len() < 64 {
        return Err(Error::Truncated("VTF header"));
    }
    let (major, minor) = (u32_at(d, 4), u32_at(d, 8));
    if major != 7 || minor > 5 {
        return Err(Error::Unsupported(format!("VTF version {major}.{minor}")));
    }
    let header_size = u32_at(d, 12) as u64;
    let width = u16_at(d, 16);
    let height = u16_at(d, 18).max(1);
    let flags = u32_at(d, 20);
    let frames = u16_at(d, 24).max(1);
    let first_frame = u16_at(d, 26);
    let code = u32_at(d, 52);
    let mips = u32::from(d[56]).max(1);
    let low_code = u32_at(d, 57);
    let (low_w, low_h) = (u32::from(d[61]), u32::from(d[62]));
    let depth = if minor >= 2 { u16_at(d, 63).max(1) } else { 1 };
    let format = format(code).ok_or_else(|| Error::Unsupported(format!("VTF format {code}")))?;
    if mips > 32 {
        return Err(Error::Malformed("VTF mip count"));
    }
    // Environment maps: 6 faces, plus a sphere map before 7.5 unless the first frame is -1.
    let faces = if flags & 0x4000 == 0 {
        1
    } else if minor < 5 && first_frame != 0xffff {
        7
    } else {
        6
    };
    let start = if minor >= 3 {
        let count = u32_at(d, 68) as usize;
        (0..count.min(32))
            .map(|i| 80 + 8 * i)
            .find(|&p| d.get(p..p + 3) == Some(&[0x30, 0, 0]))
            .map(|p| u64::from(u32_at(d, p + 4)))
            .ok_or(Error::Malformed("VTF without a high-resolution image"))?
    } else {
        let low = match format_low(low_code) {
            Some(f) if low_w > 0 && low_h > 0 => image_bytes(f, low_w, low_h),
            _ => 0,
        };
        header_size + low
    };
    let images = u64::from(frames) * u64::from(faces);
    let level_total = |k: u32| {
        let (w, h) = ((width >> k).max(1), (height >> k).max(1));
        image_bytes(format, w, h)
            .saturating_mul(images)
            .saturating_mul(u64::from((depth >> k).max(1)))
    };
    let mut levels = Vec::new();
    for k in 0..mips {
        // Smaller levels come first.
        let off = (k + 1..mips).fold(start, |o, j| o.saturating_add(level_total(j)));
        if off >= d.len() as u64 {
            break;
        }
        let total = level_total(k);
        levels.push(Level {
            width: (width >> k).max(1),
            height: (height >> k).max(1),
            range: off as usize..off.saturating_add(total).min(d.len() as u64) as usize,
            uncompressed_len: usize::try_from(total).unwrap_or(usize::MAX),
        });
    }
    Ok(Texture {
        container: Container::Vtf,
        format,
        width,
        height,
        depth,
        layers: frames,
        faces,
        srgb: flags & 0x40 != 0,
        flip_y: false,
        premultiplied: false,
        levels,
        supercompression: Supercompression::None,
        layout: Layout::PerLevel,
        image: 0,
        row_align: 1,
        basis: None,
        seek: None,
    })
}

/// The thumbnail format (no thumbnail: -1).
fn format_low(code: u32) -> Option<Format> {
    if code == u32::MAX { None } else { format(code) }
}
