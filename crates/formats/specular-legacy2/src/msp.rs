//! Microsoft Paint pictures (`.msp`, Windows 1 and 2).
//!
//! A 32-byte little-endian header: the key (`DanM` for version 1, `LinS` for version 2),
//! width, height, the bitmap's and printer's aspect ratios, the printer size, and a checksum
//! word that makes the XOR of all sixteen header words zero. Version 1 rows follow as packed
//! 1-bit pixels (most significant bit first, 1 = white). Version 2 has a table of each row's
//! compressed length, then the rows run-length coded: a byte 0 is followed by a count and a
//! byte to repeat; any other byte is the count of literal bytes after it.

use crate::{Error, Image, Info, Model, Pixels, Result, check_dims, u16le};

pub fn probe(head: &[u8]) -> bool {
    if head.len() < 32 || !(head.starts_with(b"DanM") || head.starts_with(b"LinS")) {
        return false;
    }
    (0..16).fold(0u16, |x, i| x ^ u16le(head, i * 2).unwrap_or(0)) == 0
}

struct Header {
    v2: bool,
    width: u32,
    height: u32,
    aspect: (u16, u16),
    printer: (u16, u16),
}

fn header(d: &[u8]) -> Result<Header> {
    if !probe(d) {
        return Err(if d.starts_with(b"DanM") || d.starts_with(b"LinS") {
            Error::Malformed("bad header checksum".into())
        } else {
            Error::NotRecognised
        });
    }
    let w = |o| u32::from(u16le(d, o).unwrap_or(0));
    let (width, height) = (w(4), w(6));
    check_dims(width.into(), height.into())?;
    Ok(Header {
        v2: d.starts_with(b"LinS"),
        width,
        height,
        aspect: (w(8) as u16, w(10) as u16),
        printer: (w(16) as u16, w(18) as u16),
    })
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    let h = header(data)?;
    let mut fields = vec![(
        "Version".to_string(),
        if h.v2 { "2" } else { "1" }.to_string(),
    )];
    if h.aspect.0 > 0 && h.aspect.1 > 0 {
        fields.push((
            "Aspect ratio".into(),
            format!("{}:{}", h.aspect.0, h.aspect.1),
        ));
    }
    if h.printer.0 > 0 && h.printer.1 > 0 {
        fields.push((
            "Printer size".into(),
            format!("{}×{}", h.printer.0, h.printer.1),
        ));
    }
    Ok(Info {
        width: h.width,
        height: h.height,
        pages: 1,
        bit_depth: 1,
        model: Model::Bilevel,
        has_alpha: false,
        orientation: 1,
        coding: if h.v2 {
            "1-bit, run-length coded".into()
        } else {
            "1-bit, uncompressed".into()
        },
        summary: Vec::new(),
        fields,
    })
}

fn unpack(rows: &[u8], w: usize, h: usize, out: &mut [u8]) {
    let rb = w.div_ceil(8);
    for (y, row) in rows.chunks(rb).take(h).enumerate() {
        for x in 0..w {
            let bit = row.get(x / 8).map_or(1, |b| (b >> (7 - x % 8)) & 1);
            out[y * w + x] = if bit == 1 { 255 } else { 0 };
        }
    }
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    let h = header(data)?;
    let (w, hh) = (h.width as usize, h.height as usize);
    let rb = w.div_ceil(8);
    let mut out = vec![crate::GREY; w * hh];
    let truncated;
    if !h.v2 {
        let rows = &data[32..];
        crate::check_payload(rows.len(), (rb * hh) as u64, 1)?;
        truncated = rows.len() < rb * hh;
        unpack(rows, w, hh, &mut out);
    } else {
        let mut packed = Vec::with_capacity(rb * hh);
        let mut at = 32 + hh * 2;
        let mut short = false;
        for y in 0..hh {
            let len = u16le(data, 32 + y * 2).unwrap_or(0) as usize;
            let src = data.get(at..(at + len).min(data.len())).unwrap_or(&[]);
            short |= src.len() < len;
            at += len;
            let start = packed.len();
            let mut i = 0;
            while i < src.len() && packed.len() - start < rb {
                let t = src[i] as usize;
                i += 1;
                if t == 0 {
                    let (Some(&n), Some(&v)) = (src.get(i), src.get(i + 1)) else {
                        break;
                    };
                    i += 2;
                    let n = (n as usize).min(rb - (packed.len() - start));
                    packed.extend(std::iter::repeat_n(v, n));
                } else {
                    let lit = &src[i..(i + t).min(src.len())];
                    i += t;
                    let n = lit.len().min(rb - (packed.len() - start));
                    packed.extend_from_slice(&lit[..n]);
                }
            }
            // A row coded short is white past its end (an empty row is all white).
            packed.resize(start + rb, 0xFF);
        }
        truncated = short;
        unpack(&packed, w, hh, &mut out);
    }
    Ok(Image {
        width: h.width,
        height: h.height,
        pixels: Pixels::Gray8(out),
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(key: &[u8; 4], w: u16, h: u16) -> Vec<u8> {
        let mut d = key.to_vec();
        for v in [w, h, 1, 1, 1, 1, w, h, 0, 0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.resize(32, 0);
        let x = (0..16).fold(0u16, |x, i| x ^ u16le(&d, i * 2).unwrap());
        d[24..26].copy_from_slice(&x.to_le_bytes());
        d
    }

    #[test]
    fn both_versions() {
        let mut v1 = head(b"DanM", 9, 2);
        v1.extend_from_slice(&[0x80, 0x00, 0xFF, 0x80]);
        assert!(probe(&v1));
        let img = decode(&v1, 0).unwrap();
        let Pixels::Gray8(p) = img.pixels else {
            panic!()
        };
        assert_eq!(&p[..9], &[255, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&p[9..], &[255; 9]);
        let mut v2 = head(b"LinS", 16, 2);
        v2.extend_from_slice(&[3, 0, 3, 0]);
        v2.extend_from_slice(&[0, 2, 0xF0]);
        v2.extend_from_slice(&[2, 0x0F, 0xAA]);
        let Pixels::Gray8(p) = decode(&v2, 0).unwrap().pixels else {
            panic!()
        };
        assert_eq!(&p[..8], &[255, 255, 255, 255, 0, 0, 0, 0]);
        assert_eq!(&p[16..24], &[0, 0, 0, 0, 255, 255, 255, 255]);
        let mut bad = v1.clone();
        bad[24] ^= 1;
        assert!(matches!(info(&bad, 0), Err(Error::Malformed(_))));
    }
}
