//! SGI image (".rgb", ".sgi", ".bw"): 512-byte big-endian header, planar channels, rows stored
//! bottom-up, verbatim or RLE with per-row offset tables (so rows decode in parallel).

use rayon::prelude::*;

use crate::*;

pub fn probe(head: &[u8]) -> bool {
    head.len() >= 12
        && head[0] == 0x01
        && head[1] == 0xDA
        && head[2] <= 1
        && matches!(head[3], 1 | 2)
        && matches!(u16be(head, 4), Some(1..=3))
}

struct Header {
    rle: bool,
    bpc: usize,
    w: u32,
    h: u32,
    z: usize,
}

fn header(data: &[u8]) -> Result<Header> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    if data.len() < 512 {
        return Err(Error::Truncated("header".into()));
    }
    let dim = u16be(data, 4).unwrap_or(0);
    let w = u16be(data, 6).unwrap_or(0) as u32;
    let mut h = u16be(data, 8).unwrap_or(0) as u32;
    let mut z = u16be(data, 10).unwrap_or(0) as usize;
    if dim == 1 {
        h = 1;
    }
    if dim < 3 {
        z = 1;
    }
    if z == 0 {
        return Err(Error::Malformed("no channels".into()));
    }
    if u32be(data, 104) != Some(0) {
        return Err(Error::Unsupported("colour-map SGI (obsolete)".into()));
    }
    check_dims(w, h)?;
    Ok(Header {
        rle: data[2] == 1,
        bpc: data[3] as usize,
        w,
        h,
        z,
    })
}

pub fn info(data: &[u8]) -> Result<Info> {
    let h = header(data)?;
    Ok(Info {
        format: Format::Sgi,
        width: h.w,
        height: h.h,
        pixel_format: if h.z == 1 {
            PixelFormat::Gray8
        } else {
            PixelFormat::Rgba8
        },
        has_alpha: h.z == 2 || h.z >= 4,
        model: if h.z <= 2 {
            crate::Model::Grey
        } else {
            crate::Model::Rgb
        },
        bit_depth: (h.bpc * 8) as u8,
        coding: format!(
            "{}-bit × {}{}",
            h.bpc * 8,
            h.z,
            if h.rle { ", RLE" } else { "" }
        ),
    })
}

/// Decode one RLE channel row into 8-bit samples. Returns samples written.
fn unrle(src: &[u8], bpc: usize, out: &mut [u8]) -> usize {
    let n = out.len();
    let (mut i, mut o) = (0usize, 0usize);
    let get = |i: usize| -> Option<u8> {
        if bpc == 1 {
            src.get(i).copied()
        } else {
            u16be(src, i).map(u16_to_u8)
        }
    };
    while o < n {
        let Some(c) = (if bpc == 1 {
            src.get(i).map(|&b| b as u16)
        } else {
            u16be(src, i)
        }) else {
            break;
        };
        i += bpc;
        let count = (c & 0x7F) as usize;
        if count == 0 {
            break;
        }
        let take = count.min(n - o);
        if c & 0x80 != 0 {
            for k in 0..take {
                let Some(v) = get(i + k * bpc) else {
                    return o + k;
                };
                out[o + k] = v;
            }
            i += count * bpc;
        } else {
            let Some(v) = get(i) else { break };
            i += bpc;
            out[o..o + take].fill(v);
        }
        o += take;
    }
    o
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let hd = header(data)?;
    let inf = info(data)?;
    let (w, h, z, bpc) = (hd.w as usize, hd.h as usize, hd.z, hd.bpc);
    let zc = z.min(4);
    let rows = h * z;
    let tables = if hd.rle {
        let tl = rows * 4;
        let need = 512 + 2 * tl;
        if data.len() < need {
            return Err(Error::Truncated("RLE offset tables".into()));
        }
        check_payload(data.len(), (w * rows * bpc) as u64, 128)?;
        Some((&data[512..512 + tl], &data[512 + tl..need]))
    } else {
        check_payload(data.len() - 512, (w * rows * bpc) as u64, 64)?;
        None
    };
    let mut img = Image::new(hd.w, hd.h, inf.pixel_format);
    let px = inf.pixel_format.channels();
    let bad = std::sync::atomic::AtomicBool::new(false);
    img.data
        .par_chunks_mut(w * px)
        .enumerate()
        .with_min_len(8)
        .for_each_init(
            || vec![vec![0u8; w]; zc],
            |ch, (oy, out)| {
                let y = h - 1 - oy;
                for (c, row) in ch.iter_mut().enumerate() {
                    let r = c * h + y;
                    let got = match tables {
                        Some((starts, lens)) => {
                            let s = u32be(starts, r * 4).unwrap_or(0) as usize;
                            let l = u32be(lens, r * 4).unwrap_or(0) as usize;
                            let src = data
                                .get(s..s.saturating_add(l).min(data.len()))
                                .unwrap_or(&[]);
                            unrle(src, bpc, row)
                        }
                        None => {
                            let start = 512 + r * w * bpc;
                            let src = data.get(start..).unwrap_or(&[]);
                            let n = (src.len() / bpc).min(w);
                            if bpc == 1 {
                                row[..n].copy_from_slice(&src[..n]);
                            } else {
                                for (o, b) in row[..n].iter_mut().zip(src.chunks_exact(2)) {
                                    *o = u16_to_u8(u16::from_be_bytes([b[0], b[1]]));
                                }
                            }
                            n
                        }
                    };
                    if got < w {
                        row[got..].fill(GREY);
                        bad.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                match zc {
                    1 => out.copy_from_slice(&ch[0]),
                    2 => {
                        for (x, o) in out.chunks_exact_mut(4).enumerate() {
                            let g = ch[0][x];
                            o.copy_from_slice(&[g, g, g, ch[1][x]]);
                        }
                    }
                    _ => {
                        for (x, o) in out.chunks_exact_mut(4).enumerate() {
                            let a = if zc == 4 { ch[3][x] } else { 255 };
                            o.copy_from_slice(&[ch[0][x], ch[1][x], ch[2][x], a]);
                        }
                    }
                }
            },
        );
    img.truncated = bad.into_inner();
    Ok(img)
}
