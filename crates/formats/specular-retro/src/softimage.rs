//! Softimage PIC (`.pic`; Radiance HDR files share the extension, told apart by the magic).
//!
//! A 104-byte big-endian header: magic 0x5380F634, version (float), an 80-byte comment,
//! "PICT", width, height, pixel ratio (float), fields and padding. Then channel packets of
//! four bytes (chained, size 8, type, channel mask: 0x80 red, 0x40 green, 0x20 blue, 0x10
//! alpha), chained while the first byte is nonzero. Each scan line holds every packet's
//! channels in turn: uncompressed (one byte per channel per pixel), pure RLE (a count, then
//! a pixel repeated) or mixed RLE (n < 128: n + 1 literal pixels; 128: a 16-bit count and a
//! pixel; above: n − 127 repeats of a pixel).

use crate::*;

pub const MAGIC: [u8; 4] = [0x53, 0x80, 0xf6, 0x34];
const HEADER: usize = 104;

#[derive(Clone, Copy, Debug)]
struct Packet {
    kind: u8,
    mask: u8,
}

struct Header {
    width: u32,
    height: u32,
    packets: Vec<Packet>,
    data: usize,
}

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(&MAGIC) && head.get(88..92) == Some(b"PICT")
}

fn header(data: &[u8]) -> Result<Header> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let w = u32::from(u16be(data, 92).ok_or_else(|| truncated("header"))?);
    let h = u32::from(u16be(data, 94).ok_or_else(|| truncated("header"))?);
    check_dims(w, h)?;
    let mut packets = Vec::new();
    let mut at = HEADER;
    loop {
        let p = data
            .get(at..at + 4)
            .ok_or_else(|| truncated("channel packets"))?;
        if p[2] > 2 {
            return Err(Error::Unsupported(format!("channel packet type {}", p[2])));
        }
        if p[1] != 8 {
            return Err(Error::Unsupported(format!("{}-bit channels", p[1])));
        }
        packets.push(Packet {
            kind: p[2],
            mask: p[3] & 0xf0,
        });
        at += 4;
        if p[0] == 0 || packets.len() >= 8 {
            break;
        }
    }
    Ok(Header {
        width: w,
        height: h,
        packets,
        data: at,
    })
}

fn has_alpha(h: &Header) -> bool {
    h.packets.iter().any(|p| p.mask & 0x10 != 0)
}

pub fn info(data: &[u8]) -> Result<Info> {
    let h = header(data)?;
    let kinds: Vec<&str> = h
        .packets
        .iter()
        .map(|p| match p.kind {
            0 => "uncompressed",
            1 => "pure RLE",
            _ => "mixed RLE",
        })
        .collect();
    let mut kinds2 = kinds.clone();
    kinds2.dedup();
    let alpha = has_alpha(&h);
    Ok(Info::new(
        h.width,
        h.height,
        PixelFormat::Rgba8,
        Model::Rgb,
        8,
        format!(
            "8-bit {}, {}",
            if alpha { "RGBA" } else { "RGB" },
            kinds2.join(" + ")
        ),
    )
    .alpha(alpha))
}

/// The channel offsets (into RGBA) a mask names, in file order.
fn channels(mask: u8) -> Vec<usize> {
    (0..4).filter(|k| mask & (0x80 >> k) != 0).collect()
}

/// Decodes one packet's channels of a line into `row` (RGBA); `None` when the data ends.
fn packet_line(d: &[u8], i: &mut usize, p: Packet, row: &mut [u8]) -> Option<()> {
    let ch = channels(p.mask);
    let n = ch.len();
    let w = row.len() / 4;
    let mut x = 0;
    let put = |row: &mut [u8], x: usize, px: &[u8]| {
        for (k, &c) in ch.iter().enumerate() {
            row[x * 4 + c] = px[k];
        }
    };
    match p.kind {
        0 => {
            while x < w {
                let px = d.get(*i..*i + n)?;
                *i += n;
                put(row, x, px);
                x += 1;
            }
        }
        1 => {
            while x < w {
                let count = *d.get(*i)? as usize;
                let px = d.get(*i + 1..*i + 1 + n)?;
                *i += 1 + n;
                for _ in 0..count {
                    if x >= w {
                        break;
                    }
                    put(row, x, px);
                    x += 1;
                }
            }
        }
        _ => {
            while x < w {
                let c = *d.get(*i)? as usize;
                *i += 1;
                if c < 128 {
                    for _ in 0..=c {
                        let px = d.get(*i..*i + n)?;
                        *i += n;
                        if x < w {
                            put(row, x, px);
                            x += 1;
                        }
                    }
                } else {
                    let count = if c == 128 {
                        let v = u16be(d, *i)? as usize;
                        *i += 2;
                        v
                    } else {
                        c - 127
                    };
                    let px = d.get(*i..*i + n)?;
                    *i += n;
                    for _ in 0..count {
                        if x >= w {
                            break;
                        }
                        put(row, x, px);
                        x += 1;
                    }
                }
            }
        }
    }
    Some(())
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let h = header(data)?;
    check_payload(
        data.len(),
        u64::from(h.width) * u64::from(h.height) * 3,
        // A 16-bit count run: 65535 pixels in 6 bytes.
        30000,
    )?;
    let mut img = Image::filled(h.width, h.height, PixelFormat::Rgba8, 0);
    if !has_alpha(&h) {
        for px in img.data.chunks_exact_mut(4) {
            px[3] = 255;
        }
    }
    let row_len = h.width as usize * 4;
    let mut i = h.data;
    let mut rows = 0;
    'lines: for row in img.data.chunks_exact_mut(row_len) {
        for &p in &h.packets {
            if packet_line(data, &mut i, p, row).is_none() {
                break 'lines;
            }
        }
        rows += 1;
    }
    for px in img.data[rows * row_len..].chunks_exact_mut(4) {
        px.copy_from_slice(&[GREY, GREY, GREY, 255]);
    }
    img.truncated = rows < h.height as usize;
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(w: u16, h: u16, packets: &[[u8; 4]]) -> Vec<u8> {
        let mut f = MAGIC.to_vec();
        f.extend_from_slice(&3.71f32.to_be_bytes());
        f.extend_from_slice(&[0u8; 80]);
        f.extend_from_slice(b"PICT");
        f.extend_from_slice(&w.to_be_bytes());
        f.extend_from_slice(&h.to_be_bytes());
        f.extend_from_slice(&1.0f32.to_be_bytes());
        f.extend_from_slice(&[0, 3, 0, 0]);
        for p in packets {
            f.extend_from_slice(p);
        }
        f
    }

    #[test]
    fn mixed_rgb_and_pure_alpha() {
        let mut f = head(4, 2, &[[1, 8, 2, 0xe0], [0, 8, 1, 0x10]]);
        assert!(probe(&f));
        // Line 0: 2 literal pixels, then 2 repeats; alpha run of 4.
        f.extend_from_slice(&[1, 10, 20, 30, 40, 50, 60, 129, 7, 8, 9, 4, 200]);
        // Line 1: 16-bit count run of 4; alpha 1 + 3.
        f.extend_from_slice(&[128, 0, 4, 1, 2, 3, 1, 0, 3, 255]);
        let i = info(&f).unwrap();
        assert!(i.has_alpha);
        assert_eq!(i.coding, "8-bit RGBA, mixed RLE + pure RLE");
        let img = decode(&f).unwrap();
        assert!(!img.truncated);
        assert_eq!(&img.data[..8], &[10, 20, 30, 200, 40, 50, 60, 200]);
        assert_eq!(&img.data[8..16], &[7, 8, 9, 200, 7, 8, 9, 200]);
        assert_eq!(&img.data[16..24], &[1, 2, 3, 0, 1, 2, 3, 255]);
    }

    #[test]
    fn uncompressed_and_truncated() {
        let mut f = head(2, 2, &[[0, 8, 0, 0xe0]]);
        f.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let img = decode(&f).unwrap();
        assert!(img.truncated);
        assert_eq!(&img.data[..8], &[1, 2, 3, 255, 4, 5, 6, 255]);
        assert_eq!(&img.data[8..12], &[GREY, GREY, GREY, 255]);
    }
}
