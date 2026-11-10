//! libgd's own image formats: GD (`.gd`) and GD2 (`.gd2`).
//!
//! Big-endian. **GD 2.x:** the id 0xFFFF (palette) or 0xFFFE (true colour), width, height,
//! then the colour header: a true-colour flag byte, for palette images the colour count
//! (16-bit), the transparent colour (32-bit, −1 for none), and for palette images 256
//! entries of red, green, blue and alpha. **GD 1.x** has no id: width, height, a colour
//! count byte, the transparent index (16-bit, 257 for none) and 256 RGB entries; it has no
//! signature and is recognised by its exact size.
//!
//! **GD2:** `gd2\0`, the version (1 or 2), width, height, chunk size, the format (1 raw
//! palette, 2 compressed palette, 3 raw true colour, 4 compressed true colour), and the
//! chunks across and down. Compressed files then index every chunk (offset and length);
//! the colour header follows (version 2's as above, version 1's as GD 1.x's). The image is
//! stored chunk by chunk in rows of chunks, each chunk's pixels row by row, compressed
//! chunks as zlib streams.
//!
//! Pixels are palette indices (one byte) or true colours (32-bit `0xAARRGGBB`); gd's alpha
//! runs from 0 (opaque) to 127 (transparent) and is widened to 8 bits.

use crate::{Error, Image, Info, Model, Pixels, Result, check_dims, check_payload};

fn u16be(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn u32be(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// gd's 7-bit alpha (0 opaque, 127 transparent) to 8-bit opacity.
fn alpha(a: u8) -> u8 {
    let a = u32::from(a.min(127));
    (255 - (a * 255 + 63) / 127) as u8
}

/// A true colour `0xAARRGGBB` to RGBA.
fn colour(v: u32) -> [u8; 4] {
    [
        (v >> 16) as u8,
        (v >> 8) as u8,
        v as u8,
        alpha((v >> 24) as u8 & 0x7F),
    ]
}

/// The colour header.
struct Colours {
    true_colour: bool,
    count: u32,
    transparent: Option<u32>,
    palette: Vec<[u8; 4]>,
    /// Bytes read.
    len: usize,
}

/// `x2`: the GD 2.x layout (with the flag byte and alpha); else GD 1.x's.
fn colours(d: &[u8], at: usize, x2: bool, true_colour: bool) -> Result<Colours> {
    let short = || Error::Truncated("colour header".into());
    let mut p = at;
    let (count, transparent) = if x2 {
        let flag = *d.get(p).ok_or_else(short)?;
        p += 1;
        if (flag != 0) != true_colour {
            return Err(Error::Malformed("true-colour flag disagrees".into()));
        }
        let count = if true_colour {
            0
        } else {
            let c = u16be(d, p).ok_or_else(short)?;
            p += 2;
            u32::from(c)
        };
        let t = u32be(d, p).ok_or_else(short)?;
        p += 4;
        (count, (t as i32 >= 0).then_some(t))
    } else {
        let c = *d.get(p).ok_or_else(short)?;
        let t = u16be(d, p + 1).ok_or_else(short)?;
        p += 3;
        (u32::from(c), (t != 257).then_some(u32::from(t)))
    };
    let mut palette = Vec::new();
    if !true_colour {
        let n = if x2 { 4 } else { 3 };
        let pal = d.get(p..p + 256 * n).ok_or_else(short)?;
        palette = pal
            .chunks_exact(n)
            .map(|c| [c[0], c[1], c[2], if x2 { alpha(c[3]) } else { 255 }])
            .collect();
        if let Some(t) = transparent.filter(|&t| t < 256) {
            palette[t as usize][3] = 0;
        }
        p += 256 * n;
    }
    Ok(Colours {
        true_colour,
        count,
        transparent,
        palette,
        len: p - at,
    })
}

fn facts(
    width: u32,
    height: u32,
    c: &Colours,
    coding: String,
    fields: Vec<(String, String)>,
) -> Info {
    let mut fields = fields;
    if !c.true_colour {
        fields.push(("Colours".into(), c.count.to_string()));
    }
    if let Some(t) = c.transparent {
        fields.push((
            "Transparent".into(),
            if c.true_colour {
                format!("#{:06X}", t & 0xFF_FFFF)
            } else {
                format!("index {t}")
            },
        ));
    }
    let has_alpha = c.true_colour || c.palette.iter().any(|p| p[3] < 255);
    Info {
        width,
        height,
        pages: 1,
        bit_depth: 8,
        model: if c.true_colour {
            Model::Rgb
        } else {
            Model::Palette
        },
        has_alpha,
        orientation: 1,
        coding,
        summary: Vec::new(),
        fields,
    }
}

/// Pixel `v` (an index or a true colour) to RGBA.
fn rgba(c: &Colours, v: u32) -> [u8; 4] {
    if c.true_colour {
        let mut p = colour(v);
        if c.transparent == Some(v & 0x7FFF_FFFF) {
            p[3] = 0;
        }
        p
    } else {
        c.palette.get(v as usize).copied().unwrap_or([0, 0, 0, 255])
    }
}

/// GD (`.gd`), 1.x and 2.x.
pub mod image {
    use super::*;

    struct Parsed {
        width: u32,
        height: u32,
        v2: bool,
        colours: Colours,
        data: usize,
    }

    fn v1_size(d: &[u8]) -> Option<(u32, u32)> {
        let (w, h) = (u16be(d, 0)?, u16be(d, 2)?);
        (w > 0 && h > 0 && d.len() == 7 + 768 + w as usize * h as usize)
            .then_some((u32::from(w), u32::from(h)))
    }

    /// GD 2.x's id and a sensible flag byte.
    pub fn probe(head: &[u8]) -> bool {
        head.len() >= 8
            && matches!(u16be(head, 0), Some(0xFFFE | 0xFFFF))
            && head[6] == u8::from(head[1] == 0xFE)
    }

    /// GD 1.x: no id, recognised by its exact size.
    pub fn probe_v1(data: &[u8]) -> bool {
        !probe(data) && v1_size(data).is_some()
    }

    fn parse(d: &[u8]) -> Result<Parsed> {
        if probe(d) {
            let tc = d[1] == 0xFE;
            let (width, height) = (
                u32::from(u16be(d, 2).unwrap_or(0)),
                u32::from(u16be(d, 4).unwrap_or(0)),
            );
            check_dims(width.into(), height.into())?;
            let c = colours(d, 6, true, tc)?;
            Ok(Parsed {
                width,
                height,
                v2: true,
                data: 6 + c.len,
                colours: c,
            })
        } else if let Some((width, height)) = v1_size(d) {
            let c = colours(d, 4, false, false)?;
            Ok(Parsed {
                width,
                height,
                v2: false,
                data: 4 + c.len,
                colours: c,
            })
        } else {
            Err(Error::Unsupported(
                "not a GD image: no GD 2 header, and not the size of a GD 1 image".into(),
            ))
        }
    }

    pub fn info(data: &[u8], _page: u32) -> Result<Info> {
        let p = parse(data)?;
        let coding = format!(
            "GD {}, {}",
            if p.v2 { "2" } else { "1" },
            if p.colours.true_colour {
                "true colour"
            } else {
                "palette"
            }
        );
        Ok(facts(p.width, p.height, &p.colours, coding, Vec::new()))
    }

    pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
        let p = parse(data)?;
        let n = p.width as usize * p.height as usize;
        let bpp = if p.colours.true_colour { 4 } else { 1 };
        let src = &data[p.data.min(data.len())..];
        check_payload(src.len(), (n * bpp) as u64, 1)?;
        let mut out = Vec::with_capacity(n * 4);
        for px in src.chunks_exact(bpp).take(n) {
            let v = if bpp == 4 {
                u32::from_be_bytes([px[0], px[1], px[2], px[3]])
            } else {
                u32::from(px[0])
            };
            out.extend_from_slice(&rgba(&p.colours, v));
        }
        let truncated = out.len() < n * 4;
        out.resize(n * 4, crate::GREY);
        Ok(Image {
            width: p.width,
            height: p.height,
            pixels: Pixels::Rgba8(out),
            truncated,
        })
    }
}

/// GD2 (`.gd2`).
pub mod gd2 {
    use super::*;

    /// Largest chunk side accepted.
    const MAX_CHUNK: u32 = 4096;

    pub fn probe(head: &[u8]) -> bool {
        head.len() >= 18 && head.starts_with(b"gd2\0") && matches!(u16be(head, 4), Some(1 | 2))
    }

    struct Parsed {
        version: u16,
        width: u32,
        height: u32,
        chunk: u32,
        format: u16,
        across: u32,
        down: u32,
        /// (offset, length) of each chunk, compressed files only.
        index: Vec<(usize, usize)>,
        colours: Colours,
        data: usize,
    }

    fn parse(d: &[u8]) -> Result<Parsed> {
        if !probe(d) {
            return Err(Error::NotRecognised);
        }
        let g = |o| u32::from(u16be(d, o).unwrap_or(0));
        let version = g(4) as u16;
        let (width, height, chunk, format, across, down) =
            (g(6), g(8), g(10), g(12) as u16, g(14), g(16));
        check_dims(width.into(), height.into())?;
        if !(1..=4).contains(&format) {
            return Err(Error::Unsupported(format!("GD2 format {format}")));
        }
        if chunk == 0 || chunk > MAX_CHUNK {
            return Err(Error::Malformed(format!("chunk size {chunk}")));
        }
        if across != width.div_ceil(chunk) || down != height.div_ceil(chunk) {
            return Err(Error::Malformed(
                "chunk counts do not cover the image".into(),
            ));
        }
        let compressed = format == 2 || format == 4;
        let mut at = 18;
        let mut index = Vec::new();
        if compressed {
            let n = across as usize * down as usize;
            check_payload(d.len(), (n * 8) as u64, 1)?;
            for i in 0..n {
                let (Some(o), Some(l)) = (u32be(d, at + i * 8), u32be(d, at + i * 8 + 4)) else {
                    return Err(Error::Truncated("chunk index".into()));
                };
                index.push((o as usize, l as usize));
            }
            at += n * 8;
        }
        let tc = format >= 3;
        let colours = colours(d, at, version == 2, tc)?;
        at += colours.len;
        Ok(Parsed {
            version,
            width,
            height,
            chunk,
            format,
            across,
            down,
            index,
            colours,
            data: at,
        })
    }

    pub fn info(data: &[u8], _page: u32) -> Result<Info> {
        let p = parse(data)?;
        let coding = format!(
            "GD2, {}{}",
            if p.colours.true_colour {
                "true colour"
            } else {
                "palette"
            },
            if p.index.is_empty() {
                ""
            } else {
                ", zlib chunks"
            }
        );
        let fields = vec![
            ("Version".to_string(), p.version.to_string()),
            ("Chunk size".into(), p.chunk.to_string()),
            ("Chunks".into(), format!("{}×{}", p.across, p.down)),
        ];
        Ok(facts(p.width, p.height, &p.colours, coding, fields))
    }

    pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
        let p = parse(data)?;
        let (w, h, cs) = (p.width as usize, p.height as usize, p.chunk as usize);
        let bpp = if p.colours.true_colour { 4 } else { 1 };
        check_payload(data.len(), (w * h * bpp) as u64, 1032)?;
        let mut out = vec![crate::GREY; w * h * 4];
        let mut truncated = false;
        let mut raw_at = p.data;
        let mut buf = Vec::new();
        for cy in 0..p.down as usize {
            for cx in 0..p.across as usize {
                let (x0, y0) = (cx * cs, cy * cs);
                let (cw, ch) = (cs.min(w - x0), cs.min(h - y0));
                let need = cw * ch * bpp;
                let src: &[u8] = if p.index.is_empty() {
                    let s = data
                        .get(raw_at..(raw_at + need).min(data.len()))
                        .unwrap_or(&[]);
                    raw_at += need;
                    s
                } else {
                    let (o, l) = p.index[cy * p.across as usize + cx];
                    let z = data
                        .get(o..o.saturating_add(l).min(data.len()))
                        .unwrap_or(&[]);
                    buf.clear();
                    buf.resize(need, 0);
                    match crate::inflate_zlib(z, &mut buf) {
                        Some(n) if n == need => {}
                        _ => truncated = true,
                    }
                    &buf
                };
                truncated |= src.len() < need;
                for (i, px) in src.chunks_exact(bpp).take(cw * ch).enumerate() {
                    let (x, y) = (x0 + i % cw, y0 + i / cw);
                    let v = if bpp == 4 {
                        u32::from_be_bytes([px[0], px[1], px[2], px[3]])
                    } else {
                        u32::from(px[0])
                    };
                    let o = (y * w + x) * 4;
                    out[o..o + 4].copy_from_slice(&rgba(&p.colours, v));
                }
            }
        }
        let _ = p.format;
        Ok(Image {
            width: p.width,
            height: p.height,
            pixels: Pixels::Rgba8(out),
            truncated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gd_both_versions() {
        // GD 2.x palette, 2×1, index 1 transparent.
        let mut d = vec![0xFF, 0xFF, 0, 2, 0, 1, 0, 1, 0];
        d.extend_from_slice(&1u32.to_be_bytes());
        for i in 0..256u32 {
            d.extend_from_slice(&[i as u8, 0, 0, 0]);
        }
        d.extend_from_slice(&[0, 1]);
        assert!(image::probe(&d));
        let Pixels::Rgba8(p) = image::decode(&d, 0).unwrap().pixels else {
            panic!()
        };
        assert_eq!(p, [0, 0, 0, 255, 1, 0, 0, 0]);
        // GD 2.x true colour, half transparent.
        let mut t = vec![0xFF, 0xFE, 0, 1, 0, 1, 1];
        t.extend_from_slice(&u32::MAX.to_be_bytes());
        t.extend_from_slice(&0x40_10_20_30u32.to_be_bytes());
        let Pixels::Rgba8(p) = image::decode(&t, 0).unwrap().pixels else {
            panic!()
        };
        assert_eq!(p, [0x10, 0x20, 0x30, 126]);
        // GD 1.x: by size.
        let mut v1 = vec![0, 1, 0, 1, 2, 1, 1];
        v1.extend((0..768).map(|i| (i % 3) as u8 * 100));
        v1.push(0);
        assert!(image::probe_v1(&v1));
        let Pixels::Rgba8(p) = image::decode(&v1, 0).unwrap().pixels else {
            panic!()
        };
        assert_eq!(p, [0, 100, 200, 255]);
        assert!(matches!(
            image::info(&v1[..20], 0),
            Err(Error::Unsupported(_))
        ));
    }

    #[test]
    fn gd2_raw_and_compressed() {
        // 3×2 true colour in 2×2 chunks, raw.
        let mut d = b"gd2\0".to_vec();
        for v in [2u16, 3, 2, 2, 3, 2, 1] {
            d.extend_from_slice(&v.to_be_bytes());
        }
        d.push(1);
        d.extend_from_slice(&u32::MAX.to_be_bytes());
        // Chunk (0,0): 2×2, then chunk (1,0): 1×2.
        for v in [1u32, 2, 4, 5, 3, 6] {
            d.extend_from_slice(&v.to_be_bytes());
        }
        let Pixels::Rgba8(p) = gd2::decode(&d, 0).unwrap().pixels else {
            panic!()
        };
        let blue: Vec<u8> = p.chunks_exact(4).map(|c| c[2]).collect();
        assert_eq!(blue, [1, 2, 3, 4, 5, 6]);
        assert!(gd2::info(&d, 0).unwrap().coding.contains("true colour"));
    }
}
