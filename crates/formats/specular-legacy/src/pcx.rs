//! ZSoft PCX: 128-byte header, RLE (or raw) scan lines of `planes × bytes_per_line` bytes,
//! optional 256-colour palette after the pixels.

use crate::*;

pub fn probe(head: &[u8]) -> bool {
    head.len() >= 128
        && head[0] == 0x0A
        && matches!(head[1], 0 | 2 | 3 | 4 | 5)
        && head[2] <= 1
        && matches!(head[3], 1 | 2 | 4 | 8)
        && matches!(head[65], 1..=4)
}

struct Header {
    w: u32,
    h: u32,
    bpp: u8,
    planes: u8,
    bpl: usize,
    rle: bool,
    version: u8,
}

fn header(data: &[u8]) -> Result<Header> {
    if data.len() < 128 {
        return Err(if data.first() == Some(&0x0A) {
            Error::Truncated("header".into())
        } else {
            Error::NotRecognised
        });
    }
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let g = |o| u16le(data, o).unwrap_or(0);
    let (x0, y0, x1, y1) = (g(4), g(6), g(8), g(10));
    if x1 < x0 || y1 < y0 {
        return Err(Error::Malformed("window".into()));
    }
    let (w, h) = (x1 as u32 - x0 as u32 + 1, y1 as u32 - y0 as u32 + 1);
    check_dims(w, h)?;
    let (bpp, planes) = (data[3], data[65]);
    match (bpp, planes) {
        (1, 1..=4) | (2, 1) | (4, 1) | (8, 1) | (8, 3) | (8, 4) => {}
        _ => return Err(Error::Unsupported(format!("{bpp} bits × {planes} planes"))),
    }
    let min_bpl = (w as usize * bpp as usize).div_ceil(8);
    let mut bpl = g(66) as usize;
    if bpl < min_bpl {
        // Some writers store 0 or an odd-rounded short value.
        bpl = min_bpl;
    }
    Ok(Header {
        w,
        h,
        bpp,
        planes,
        bpl,
        rle: data[2] == 1,
        version: data[1],
    })
}

fn has_vga_palette(data: &[u8], h: &Header) -> bool {
    h.bpp == 8 && h.planes == 1 && data.len() >= 128 + 769 && data[data.len() - 769] == 0x0C
}

pub fn info(data: &[u8]) -> Result<Info> {
    let h = header(data)?;
    let gray = (h.bpp == 8 && h.planes == 1 && !has_vga_palette(data, &h))
        || (h.bpp == 1 && h.planes == 1);
    let bits = h.bpp * h.planes;
    Ok(Info {
        format: Format::Pcx,
        width: h.w,
        height: h.h,
        pixel_format: if gray {
            PixelFormat::Gray8
        } else {
            PixelFormat::Rgba8
        },
        has_alpha: h.planes == 4 && h.bpp == 8,
        model: match (h.bpp, h.planes) {
            (8, 3) | (8, 4) => crate::Model::Rgb,
            _ if gray => crate::Model::Grey,
            _ => crate::Model::Palette,
        },
        // Per sample for true colour; a palette index as stored.
        bit_depth: if h.planes >= 3 && h.bpp == 8 { 8 } else { bits },
        coding: format!(
            "{bits}-bit {}{}",
            match (h.bpp, h.planes) {
                (8, 3) | (8, 4) => "true colour",
                _ if gray => "grey",
                _ => "palette",
            },
            if h.rle { ", RLE" } else { "" }
        ),
    })
}

/// Classic 16-colour EGA palette, used when a v3 file carries none.
const EGA: [[u8; 3]; 16] = [
    [0, 0, 0],
    [0, 0, 170],
    [0, 170, 0],
    [0, 170, 170],
    [170, 0, 0],
    [170, 0, 170],
    [170, 85, 0],
    [170, 170, 170],
    [85, 85, 85],
    [85, 85, 255],
    [85, 255, 85],
    [85, 255, 255],
    [255, 85, 85],
    [255, 85, 255],
    [255, 255, 85],
    [255, 255, 255],
];

pub fn decode(data: &[u8]) -> Result<Image> {
    let h = header(data)?;
    let inf = info(data)?;
    let line = h.bpl * h.planes as usize;
    let mut body = &data[128..];
    let vga = has_vga_palette(data, &h);
    if vga {
        body = &data[128..data.len() - 769];
    }
    check_payload(
        body.len(),
        line as u64 * h.h as u64,
        if h.rle { 32 } else { 1 },
    )?;
    let (w, ht) = (h.w as usize, h.h as usize);
    let mut img = Image::new(h.w, h.h, inf.pixel_format);
    let palette: Vec<[u8; 3]> = if vga {
        data[data.len() - 768..]
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect()
    } else if h.version == 3 {
        EGA.to_vec()
    } else {
        data[16..64]
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect()
    };
    let mut buf = vec![0u8; line];
    let mut idx = vec![0u8; w];
    // RLE state persists across lines: some encoders let runs cross scan-line boundaries.
    let (mut pos, mut run, mut run_val) = (0usize, 0usize, 0u8);
    let bpc = img.format.channels();
    for y in 0..ht {
        let mut filled = 0;
        if h.rle {
            while filled < line {
                if run > 0 {
                    let n = run.min(line - filled);
                    buf[filled..filled + n].fill(run_val);
                    filled += n;
                    run -= n;
                    continue;
                }
                let Some(&b) = body.get(pos) else { break };
                pos += 1;
                if b >= 0xC0 {
                    let Some(&v) = body.get(pos) else { break };
                    pos += 1;
                    run = (b & 0x3F) as usize;
                    run_val = v;
                } else {
                    buf[filled] = b;
                    filled += 1;
                }
            }
        } else {
            let n = body.len().saturating_sub(pos).min(line);
            buf[..n].copy_from_slice(&body[pos..pos + n]);
            pos += n;
            filled = n;
        }
        if filled < line {
            img.truncated = true;
            buf[filled..].fill(GREY);
        }
        let out = &mut img.data[y * w * bpc..(y + 1) * w * bpc];
        match (h.bpp, h.planes) {
            (8, 1) => {
                if vga {
                    expand_palette(&buf[..w], &palette, out);
                } else {
                    out.copy_from_slice(&buf[..w]);
                }
            }
            (8, p) => {
                let (r, g, b) = (&buf[..w], &buf[h.bpl..h.bpl + w], &buf[2 * h.bpl..][..w]);
                let a = (p == 4).then(|| &buf[3 * h.bpl..][..w]);
                for x in 0..w {
                    out[x * 4..x * 4 + 4].copy_from_slice(&[
                        r[x],
                        g[x],
                        b[x],
                        a.map_or(255, |a| a[x]),
                    ]);
                }
            }
            (1, 1) => {
                for (x, o) in out.iter_mut().enumerate() {
                    *o = if buf[x >> 3] & (0x80 >> (x & 7)) != 0 {
                        255
                    } else {
                        0
                    };
                }
            }
            (bpp, planes) => {
                // Packed (2/4-bit × 1 plane) or bit-planar (1-bit × 2–4 planes) indices.
                let bpp = bpp as usize;
                let per_byte = 8 / bpp;
                let mask = (1u8 << bpp) - 1;
                for (x, v) in idx.iter_mut().enumerate() {
                    let mut i = 0u8;
                    for p in 0..planes as usize {
                        let byte = buf[p * h.bpl + x / per_byte];
                        let shift = 8 - bpp * (x % per_byte + 1);
                        i |= ((byte >> shift) & mask) << (p * bpp);
                    }
                    *v = i;
                }
                expand_palette(&idx, &palette, out);
            }
        }
    }
    Ok(img)
}
