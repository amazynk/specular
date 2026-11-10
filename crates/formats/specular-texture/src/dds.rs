//! DDS container: the DX9 header (FourCC or channel masks) and the DX10 extension (DXGI
//! formats). Data layout: for each array element (cube face), every mip level in turn, each
//! level holding its depth slices.

use crate::{
    Container, Error, Format, Layout, Level, Result, Supercompression, Texture, image_bytes,
};

const HEADER: usize = 128;
const DX10_HEADER: usize = 20;

fn u32_at(d: &[u8], p: usize) -> u32 {
    d.get(p..p + 4)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

const RGBA8: [u32; 4] = [0xff, 0xff00, 0xff_0000, 0xff00_0000];
const BGRA8: [u32; 4] = [0xff_0000, 0xff00, 0xff, 0xff00_0000];
const BGRX8: [u32; 4] = [0xff_0000, 0xff00, 0xff, 0];

fn masked(bytes: u8, masks: [u32; 4]) -> Format {
    Format::Masked {
        bytes,
        masks,
        grey: false,
    }
}

/// DXGI_FORMAT → (format, sRGB).
pub(crate) fn dxgi(code: u32) -> Option<(Format, bool)> {
    use Format::*;
    Some(match code {
        2 => (Float { channels: 4 }, false),
        6 => (Float { channels: 3 }, false),
        10 => (Half { channels: 4 }, false),
        11 => (Unorm16 { channels: 4 }, false),
        16 => (Float { channels: 2 }, false),
        24 => (masked(4, [0x3ff, 0xffc00, 0x3ff0_0000, 0xc000_0000]), false),
        26 => (R11G11B10F, false),
        27 | 28 => (masked(4, RGBA8), false),
        29 => (masked(4, RGBA8), true),
        31 => (Snorm8 { channels: 4 }, false),
        34 => (Half { channels: 2 }, false),
        35 => (Unorm16 { channels: 2 }, false),
        41 => (Float { channels: 1 }, false),
        49 => (masked(2, [0xff, 0xff00, 0, 0]), false),
        51 => (Snorm8 { channels: 2 }, false),
        54 => (Half { channels: 1 }, false),
        56 => (Unorm16 { channels: 1 }, false),
        61 | 65 => (
            Masked {
                bytes: 1,
                masks: [0xff, 0, 0, 0],
                grey: true,
            },
            false,
        ),
        63 => (Snorm8 { channels: 1 }, false),
        67 => (Rgb9e5, false),
        70 | 71 => (Bc1 { alpha: true }, false),
        72 => (Bc1 { alpha: true }, true),
        73 | 74 => (Bc2, false),
        75 => (Bc2, true),
        76 | 77 => (Bc3, false),
        78 => (Bc3, true),
        79 | 80 => (Bc4 { signed: false }, false),
        81 => (Bc4 { signed: true }, false),
        82 | 83 => (Bc5 { signed: false }, false),
        84 => (Bc5 { signed: true }, false),
        85 => (masked(2, [0xf800, 0x7e0, 0x1f, 0]), false),
        86 => (masked(2, [0x7c00, 0x3e0, 0x1f, 0x8000]), false),
        87 | 90 => (masked(4, BGRA8), false),
        91 => (masked(4, BGRA8), true),
        88 | 92 => (masked(4, BGRX8), false),
        93 => (masked(4, BGRX8), true),
        94 | 95 => (Bc6h { signed: false }, false),
        96 => (Bc6h { signed: true }, false),
        97 | 98 => (Bc7, false),
        99 => (Bc7, true),
        115 => (masked(2, [0xf00, 0xf0, 0xf, 0xf000]), false),
        _ => return None,
    })
}

fn fourcc(cc: u32) -> Option<Format> {
    use Format::*;
    Some(match &cc.to_le_bytes() {
        b"DXT1" => Bc1 { alpha: true },
        b"DXT2" | b"DXT3" => Bc2,
        b"DXT4" | b"DXT5" => Bc3,
        b"ATI1" | b"BC4U" => Bc4 { signed: false },
        b"BC4S" => Bc4 { signed: true },
        b"ATI2" | b"BC5U" => Bc5 { signed: false },
        b"BC5S" => Bc5 { signed: true },
        _ => match cc {
            36 => Unorm16 { channels: 4 },
            111 => Half { channels: 1 },
            112 => Half { channels: 2 },
            113 => Half { channels: 4 },
            114 => Float { channels: 1 },
            115 => Float { channels: 2 },
            116 => Float { channels: 4 },
            _ => return None,
        },
    })
}

pub(crate) fn parse(d: &[u8]) -> Result<Texture> {
    if d.len() < HEADER {
        return Err(Error::Truncated("DDS header"));
    }
    if u32_at(d, 4) != 124 {
        return Err(Error::Malformed("DDS header size"));
    }
    let flags = u32_at(d, 8);
    let height = u32_at(d, 12);
    let width = u32_at(d, 16);
    let mut depth = if flags & 0x80_0000 != 0 {
        u32_at(d, 24).max(1)
    } else {
        1
    };
    let mips = u32_at(d, 28).clamp(1, 32);
    let pf_flags = u32_at(d, 80);
    let cc = u32_at(d, 84);
    let bit_count = u32_at(d, 88);
    let m = [u32_at(d, 92), u32_at(d, 96), u32_at(d, 100), u32_at(d, 104)];
    let caps2 = u32_at(d, 112);
    let mut faces = if caps2 & 0x200 != 0 { 6 } else { 1 };
    if caps2 & 0x20_0000 == 0 {
        depth = depth.min(1);
    }
    let mut layers = 1;
    let mut srgb = false;
    let mut premultiplied = false;
    let mut start = HEADER;
    let format = if pf_flags & 0x4 != 0 && &cc.to_le_bytes() == b"DX10" {
        if d.len() < HEADER + DX10_HEADER {
            return Err(Error::Truncated("DX10 header"));
        }
        start += DX10_HEADER;
        let code = u32_at(d, 128);
        let dim = u32_at(d, 132);
        let misc = u32_at(d, 136);
        layers = u32_at(d, 140).max(1);
        if misc & 0x4 != 0 {
            faces = 6;
        }
        if dim != 4 {
            depth = 1;
        }
        let (f, s) = dxgi(code).ok_or_else(|| Error::Unsupported(format!("DXGI format {code}")))?;
        srgb = s;
        // miscFlags2 alpha mode 2 (DDS_ALPHA_MODE_PREMULTIPLIED). Float formats would need the
        // division in linear light before encoding: left as stored.
        premultiplied = u32_at(d, 144) & 7 == 2 && f.has_alpha() && !f.is_float();
        f
    } else if pf_flags & 0x4 != 0 {
        // DXT2 and DXT4 are DXT3 and DXT5 with colour premultiplied by alpha.
        premultiplied = matches!(&cc.to_le_bytes(), b"DXT2" | b"DXT4");
        fourcc(cc).ok_or_else(|| {
            let b = cc.to_le_bytes();
            if b.iter().all(|c| c.is_ascii_graphic()) {
                Error::Unsupported(format!("DDS FourCC {}", String::from_utf8_lossy(&b)))
            } else {
                Error::Unsupported(format!("D3DFORMAT {cc}"))
            }
        })?
    } else {
        let bytes = match bit_count {
            8 | 16 | 24 | 32 => (bit_count / 8) as u8,
            _ => return Err(Error::Unsupported(format!("{bit_count}-bit DDS pixels"))),
        };
        let alpha = if pf_flags & 0x1 != 0 { m[3] } else { 0 };
        if pf_flags & 0x8_0000 != 0 && bit_count == 16 {
            // Bump map DU/DV (V8U8): signed.
            Format::Snorm8 { channels: 2 }
        } else if pf_flags & 0x2_0000 != 0 {
            Format::Masked {
                bytes,
                masks: [m[0], 0, 0, alpha],
                grey: true,
            }
        } else if pf_flags & 0x2 != 0 && pf_flags & 0x40 == 0 {
            // Alpha only: shown as grey.
            Format::Masked {
                bytes,
                masks: [m[3], 0, 0, 0],
                grey: true,
            }
        } else if pf_flags & 0x40 != 0 {
            if m[0] | m[1] | m[2] == 0 {
                return Err(Error::Malformed("DDS RGB without masks"));
            }
            Format::Masked {
                bytes,
                masks: [m[0], m[1], m[2], alpha],
                grey: false,
            }
        } else {
            return Err(Error::Unsupported(
                "DDS pixel format (YUV or unknown)".into(),
            ));
        }
    };
    if let Format::Masked { bytes, masks, .. } = format {
        let limit = if bytes == 4 {
            u32::MAX
        } else {
            (1u32 << (bytes * 8)) - 1
        };
        if masks.iter().any(|&m| m & !limit != 0) {
            return Err(Error::Malformed("DDS channel mask wider than the pixel"));
        }
    }
    let mut levels = Vec::new();
    let mut off = start as u64;
    let mut complete = true;
    for k in 0..mips {
        let (w, h) = ((width >> k).max(1), (height >> k).max(1));
        let one = image_bytes(format, w, h);
        let dk = (depth >> k).max(1) as u64;
        if off >= d.len() as u64 {
            complete = false;
        }
        // The level's slices of the first layer / face; the others are a chain further on.
        let end = off.saturating_add(one.saturating_mul(dk));
        if complete {
            levels.push(Level {
                width: w,
                height: h,
                range: off as usize..end.min(d.len() as u64) as usize,
                uncompressed_len: one as usize,
            });
        }
        off = end;
        if w == 1 && h == 1 && dk == 1 {
            break;
        }
    }
    let chain = off - start as u64;
    Ok(Texture {
        container: Container::Dds,
        format,
        width,
        height,
        depth,
        layers,
        faces,
        srgb,
        flip_y: false,
        premultiplied,
        levels,
        supercompression: Supercompression::None,
        layout: Layout::PerChain { chain },
        image: 0,
        row_align: 1,
        basis: None,
        seek: None,
    })
}
