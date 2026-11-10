//! Sun Raster: 32-byte big-endian header, optional colour map, rows padded to 16 bits;
//! types old/standard (BGR), byte-encoded RLE, and RGB.

use crate::*;

const MAGIC: [u8; 4] = [0x59, 0xA6, 0x6A, 0x95];

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(&MAGIC)
}

struct Header {
    w: u32,
    h: u32,
    depth: u32,
    kind: u32,
    map: Vec<[u8; 3]>,
    data_at: usize,
}

fn header(data: &[u8]) -> Result<Header> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let g = |o| u32be(data, o).ok_or_else(|| Error::Truncated("header".into()));
    let (w, h, depth, _len, kind, maptype, maplen) =
        (g(4)?, g(8)?, g(12)?, g(16)?, g(20)?, g(24)?, g(28)?);
    check_dims(w, h)?;
    if !matches!(depth, 1 | 8 | 24 | 32) {
        return Err(Error::Unsupported(format!("{depth}-bit")));
    }
    if kind > 3 {
        return Err(Error::Unsupported(format!("raster type {kind}")));
    }
    let maplen = maplen as usize;
    let map_bytes = data
        .get(32..32usize.saturating_add(maplen))
        .ok_or_else(|| Error::Truncated("colour map".into()))?;
    let map = match maptype {
        0 => Vec::new(),
        1 => {
            let n = maplen / 3;
            (0..n)
                .map(|i| [map_bytes[i], map_bytes[n + i], map_bytes[2 * n + i]])
                .collect()
        }
        2 => Vec::new(), // raw map: meaning undefined, ignored
        t => return Err(Error::Unsupported(format!("colour map type {t}"))),
    };
    Ok(Header {
        w,
        h,
        depth,
        kind,
        map,
        data_at: 32 + maplen,
    })
}

fn gray(h: &Header) -> bool {
    h.depth <= 8 && h.map.is_empty()
}

pub fn info(data: &[u8]) -> Result<Info> {
    let h = header(data)?;
    Ok(Info {
        format: Format::SunRaster,
        width: h.w,
        height: h.h,
        pixel_format: if gray(&h) {
            PixelFormat::Gray8
        } else {
            PixelFormat::Rgba8
        },
        has_alpha: false,
        model: if gray(&h) {
            crate::Model::Grey
        } else if h.depth <= 8 {
            crate::Model::Palette
        } else {
            crate::Model::Rgb
        },
        // Per sample: 24/32-bit pixels hold 8-bit samples; a colour map index as stored.
        bit_depth: h.depth.min(8) as u8,
        coding: format!(
            "{}-bit{}{}",
            h.depth,
            if h.map.is_empty() {
                ""
            } else {
                " colour-mapped"
            },
            if h.kind == 2 { ", RLE" } else { "" }
        ),
    })
}

/// Sun byte-encoded RLE: 0x80 0x00 = literal 0x80, 0x80 n v = n+1 copies of v.
fn unrle(src: &[u8], out: &mut [u8]) -> usize {
    let (mut i, mut o) = (0, 0);
    while o < out.len() && i < src.len() {
        let b = src[i];
        i += 1;
        if b != 0x80 {
            out[o] = b;
            o += 1;
            continue;
        }
        let Some(&n) = src.get(i) else { break };
        i += 1;
        if n == 0 {
            out[o] = 0x80;
            o += 1;
            continue;
        }
        let Some(&v) = src.get(i) else { break };
        i += 1;
        let take = (n as usize + 1).min(out.len() - o);
        out[o..o + take].fill(v);
        o += take;
    }
    o
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let hd = header(data)?;
    let fmt = if gray(&hd) {
        PixelFormat::Gray8
    } else {
        PixelFormat::Rgba8
    };
    let (w, h) = (hd.w as usize, hd.h as usize);
    let stride = (w * hd.depth as usize).div_ceil(16) * 2;
    let src = data.get(hd.data_at..).unwrap_or(&[]);
    let total = stride * h;
    check_payload(src.len(), total as u64, if hd.kind == 2 { 128 } else { 1 })?;
    let mut img = Image::new(hd.w, hd.h, fmt);
    let raw: std::borrow::Cow<[u8]> = if hd.kind == 2 {
        let mut buf = vec![GREY; total];
        let n = unrle(src, &mut buf);
        img.truncated = n < total;
        buf.into()
    } else {
        img.truncated = src.len() < total;
        if img.truncated {
            let mut buf = src.to_vec();
            buf.resize(total, GREY);
            buf.into()
        } else {
            src[..total].into()
        }
    };
    let rgb_order = hd.kind == 3;
    let px = fmt.channels();
    for (y, out) in img.data.chunks_exact_mut(w * px).enumerate() {
        let r = &raw[y * stride..(y + 1) * stride];
        match hd.depth {
            1 => {
                for x in 0..w {
                    let bit = r[x >> 3] & (0x80 >> (x & 7)) != 0;
                    if hd.map.is_empty() {
                        out[x] = if bit { 0 } else { 255 };
                    } else {
                        let [a, b, c] = hd.map.get(bit as usize).copied().unwrap_or([0; 3]);
                        out[x * 4..x * 4 + 4].copy_from_slice(&[a, b, c, 255]);
                    }
                }
            }
            8 => {
                if hd.map.is_empty() {
                    out.copy_from_slice(&r[..w]);
                } else {
                    expand_palette(&r[..w], &hd.map, out);
                }
            }
            d => {
                let n = d as usize / 8;
                for (o, p) in out.chunks_exact_mut(4).zip(r.chunks_exact(n)) {
                    // 32-bit pixels carry a leading pad byte.
                    let p = &p[n - 3..];
                    let (a, b, c) = if rgb_order {
                        (p[0], p[1], p[2])
                    } else {
                        (p[2], p[1], p[0])
                    };
                    o.copy_from_slice(&[a, b, c, 255]);
                }
            }
        }
    }
    Ok(img)
}
