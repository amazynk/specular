//! Atari ST screens: DEGAS (`.pi1`–`.pi3`, compressed `.pc1`–`.pc3`) and NEOchrome (`.neo`).
//!
//! The ST screen is 32000 bytes of interleaved bit planes: each group of 16 pixels is one
//! big-endian word per plane (4 planes at 320×200, 2 at 640×200, 1 at 640×400). Palettes are
//! 16 words `0RGB`; the STE's fourth bit per channel is the low bit stored on top (bit 3), so
//! a nibble `v` is `(v & 7) << 1 | v >> 3`.
//!
//! DEGAS: the resolution word (bit 15 set when compressed), the palette, then the screen; a
//! compressed screen is PackBits per scan line, plane by plane. NEOchrome: a 128-byte header
//! (flag, resolution, palette at byte 4), then the screen.
//!
//! Medium resolution has pixels twice as tall as wide; they are shown as stored.

use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Degas,
    Neochrome,
}

pub const SCREEN: usize = 32000;

struct Header {
    res: u16,
    compressed: bool,
    palette: [u16; 16],
    data: usize,
}

fn dims(res: u16) -> (u32, u32, usize) {
    match res {
        0 => (320, 200, 4),
        1 => (640, 200, 2),
        _ => (640, 400, 1),
    }
}

fn header(kind: Kind, data: &[u8]) -> Result<Header> {
    let (res_at, pal_at, start) = match kind {
        Kind::Degas => (0, 2, 34),
        Kind::Neochrome => (2, 4, 128),
    };
    let r = u16be(data, res_at).ok_or_else(|| truncated("header"))?;
    let compressed = kind == Kind::Degas && r & 0x8000 != 0;
    let res = r & 0x7fff;
    if res > 2 || (kind == Kind::Neochrome && u16be(data, 0) != Some(0)) {
        return Err(Error::NotRecognised);
    }
    if !compressed && data.len() < start + SCREEN - 1024 {
        return Err(Error::NotRecognised);
    }
    let mut palette = [0u16; 16];
    for (i, p) in palette.iter_mut().enumerate() {
        *p = u16be(data, pal_at + 2 * i).ok_or_else(|| truncated("palette"))?;
    }
    Ok(Header {
        res,
        compressed,
        palette,
        data: start,
    })
}

/// Whether the whole file looks like this kind of screen (sizes are fixed when stored
/// uncompressed).
pub fn probe_file(kind: Kind, data: &[u8]) -> bool {
    match header(kind, data) {
        Ok(h) if h.compressed => data.len() < 34 + SCREEN + 2048,
        Ok(_) => match kind {
            Kind::Degas => matches!(data.len(), 32034 | 32066 | 32128),
            Kind::Neochrome => data.len() == 32128,
        },
        Err(_) => false,
    }
}

/// The palette as RGBA (ST three-bit or STE four-bit channels).
fn palette(p: &[u16; 16]) -> Vec<[u8; 4]> {
    let ste = p.iter().any(|v| v & 0x888 != 0);
    p.iter()
        .map(|&v| {
            let ch = |n: u16| {
                let n = (n & 0xf) as u32;
                if ste {
                    scale_to_8((n & 7) << 1 | n >> 3, 15)
                } else {
                    scale_to_8(n & 7, 7)
                }
            };
            [ch(v >> 8), ch(v >> 4), ch(v), 255]
        })
        .collect()
}

pub fn info(kind: Kind, data: &[u8]) -> Result<Info> {
    let h = header(kind, data)?;
    let (w, ht, planes) = dims(h.res);
    let name = match (kind, h.res) {
        (Kind::Degas, _) if h.compressed => "DEGAS Elite, compressed",
        (Kind::Degas, _) => "DEGAS",
        (Kind::Neochrome, _) => "NEOchrome",
    };
    let res = ["low", "medium", "high"][h.res as usize];
    Ok(Info::new(
        w,
        ht,
        PixelFormat::Rgba8,
        Model::Palette,
        planes as u8,
        format!("{name}, {res} resolution, {} colours", 1 << planes),
    ))
}

/// The screen bytes in ST layout: copied, or unpacked from DEGAS's per-plane lines.
fn screen(h: &Header, data: &[u8]) -> (Vec<u8>, bool) {
    let mut s = vec![0u8; SCREEN];
    let src = data.get(h.data..).unwrap_or(&[]);
    if !h.compressed {
        let n = src.len().min(SCREEN);
        s[..n].copy_from_slice(&src[..n]);
        return (s, n == SCREEN);
    }
    let (_, ht, planes) = dims(h.res);
    let line = SCREEN / ht as usize;
    let plane_bytes = line / planes;
    let mut buf = vec![0u8; line];
    let mut i = 0;
    for y in 0..ht as usize {
        let (n, ok) = unpackbits(&src[i.min(src.len())..], &mut buf);
        i += n;
        // buf: plane 0's bytes, then plane 1's…; interleave to words.
        let out = &mut s[y * line..(y + 1) * line];
        for p in 0..planes {
            for k in 0..plane_bytes {
                let word = k / 2;
                out[word * 2 * planes + p * 2 + k % 2] = buf[p * plane_bytes + k];
            }
        }
        if !ok {
            return (s, false);
        }
    }
    (s, true)
}

pub fn decode(kind: Kind, data: &[u8]) -> Result<Image> {
    let h = header(kind, data)?;
    let (w, ht, planes) = dims(h.res);
    let (s, complete) = screen(&h, data);
    let pal = if planes == 1 {
        // Monochrome: colour 0's lowest bit says whether the screen is inverted.
        if h.palette[0] & 1 == 0 {
            vec![[0, 0, 0, 255], [255, 255, 255, 255]]
        } else {
            vec![[255, 255, 255, 255], [0, 0, 0, 255]]
        }
    } else {
        palette(&h.palette)
    };
    let mut idx = vec![0u8; w as usize * ht as usize];
    let words_per_line = w as usize / 16;
    for y in 0..ht as usize {
        for g in 0..words_per_line {
            let base = (y * words_per_line + g) * planes * 2;
            for bit in 0..16 {
                let mut c = 0u8;
                for p in 0..planes {
                    let word = u16::from_be_bytes([s[base + p * 2], s[base + p * 2 + 1]]);
                    c |= ((word >> (15 - bit) & 1) as u8) << p;
                }
                idx[y * w as usize + g * 16 + bit] = c;
            }
        }
    }
    let mut img = Image::from_indices(w, ht, &idx, &pal);
    img.truncated = !complete;
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_res_planes_and_ste_palette() {
        let mut f = vec![0u8, 0];
        let mut pal = [0u16; 16];
        pal[1] = 0x0700; // ST red
        pal[5] = 0x0f0f; // STE white-ish magenta: f → 15
        for p in pal {
            f.extend_from_slice(&p.to_be_bytes());
        }
        let mut s = vec![0u8; SCREEN];
        // First 16 pixels: pixel 0 = colour 1 (plane 0), pixel 1 = colour 5 (planes 0 and 2).
        s[0] = 0xc0; // plane 0 word: bits 15, 14
        s[4] = 0x40; // plane 2 word: bit 14
        f.extend_from_slice(&s);
        assert!(probe_file(Kind::Degas, &f));
        let i = info(Kind::Degas, &f).unwrap();
        assert_eq!((i.width, i.height), (320, 200));
        let img = decode(Kind::Degas, &f).unwrap();
        // STE palette in use (0x0f0f has bit 3 set): 7 → (7<<1)=14 → 238.
        assert_eq!(&img.data[..4], &[238, 0, 0, 255]);
        assert_eq!(&img.data[4..8], &[255, 0, 255, 255]);
        assert_eq!(&img.data[8..12], &[0, 0, 0, 255]);
    }

    #[test]
    fn compressed_matches_plain() {
        let mut plain = vec![0u8, 1];
        plain.extend(std::iter::repeat_n([0x07u8, 0x77], 16).flatten());
        let mut s = vec![0u8; SCREEN];
        for (i, b) in s.iter_mut().enumerate() {
            *b = (i * 31 % 251) as u8;
        }
        plain.extend_from_slice(&s);
        let mut packed = vec![0x80u8, 1];
        packed.extend_from_slice(&plain[2..34]);
        // Medium: 160 bytes a line, 2 planes of 80 bytes; literal runs of 80.
        for y in 0..200 {
            let line = &s[y * 160..(y + 1) * 160];
            for p in 0..2 {
                let bytes: Vec<u8> = (0..80).map(|k| line[(k / 2) * 4 + p * 2 + k % 2]).collect();
                packed.push(79);
                packed.extend_from_slice(&bytes);
            }
        }
        let a = decode(Kind::Degas, &plain).unwrap();
        let b = decode(Kind::Degas, &packed).unwrap();
        assert_eq!((a.width, a.height), (640, 200));
        assert!(!b.truncated);
        assert_eq!(a.data, b.data);
        assert!(decode(Kind::Degas, &packed[..5000]).unwrap().truncated);
    }

    #[test]
    fn neochrome() {
        let mut f = vec![0u8; 128];
        f[3] = 2; // high resolution
        f.extend(vec![0xffu8; SCREEN]);
        assert!(probe_file(Kind::Neochrome, &f));
        let img = decode(Kind::Neochrome, &f).unwrap();
        assert_eq!((img.width, img.height), (640, 400));
        assert_eq!(&img.data[..4], &[255, 255, 255, 255]);
    }
}
