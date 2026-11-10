//! Utah Raster Toolkit RLE (`.rle`).
//!
//! A little-endian header: magic 0xCC52, x and y position, width and height, flags (1 clear
//! first, 2 no background, 4 alpha, 8 comments), colour channels, bits per pixel (8), colour
//! map channels and log2 of the map length. Then the background colour (one byte per
//! channel, padded to an even length; a single filler byte with "no background"), the colour
//! maps (16-bit entries, the value in the high byte), the comments, and opcodes: skip lines,
//! set channel (255 = alpha), skip pixels, byte data, run data and end. Lines count up from
//! the bottom.

use crate::*;

const MAGIC: u16 = 0xcc52;
const MAX_CHANNELS: usize = 3;

struct Header {
    xpos: i32,
    ypos: i32,
    width: u32,
    height: u32,
    flags: u8,
    ncolors: usize,
    background: Vec<u8>,
    maps: usize,
    map_len: usize,
    cmap: Vec<u16>,
    data: usize,
}

pub fn probe(head: &[u8]) -> bool {
    u16le(head, 0) == Some(MAGIC) && head.get(12) == Some(&8) && head.len() > 14
}

fn header(d: &[u8]) -> Result<Header> {
    if !probe(d) {
        return Err(Error::NotRecognised);
    }
    let s = |o: usize| i32::from(u16le(d, o).unwrap_or(0) as i16);
    let width = u32::from(u16le(d, 6).unwrap_or(0));
    let height = u32::from(u16le(d, 8).unwrap_or(0));
    check_dims(width, height)?;
    let flags = d[10];
    let ncolors = d[11] as usize;
    let maps = *d.get(13).ok_or_else(|| truncated("header"))? as usize;
    let maplog = *d.get(14).ok_or_else(|| truncated("header"))? as usize;
    if ncolors == 0 || ncolors > 254 || maplog > 8 {
        return Err(Error::Unsupported(format!(
            "{ncolors} channels, colour map length 2^{maplog}"
        )));
    }
    let mut at = 15;
    let mut background = Vec::new();
    if flags & 2 == 0 {
        background = d
            .get(at..at + ncolors)
            .ok_or_else(|| truncated("background"))?
            .to_vec();
        at += ncolors;
        if ncolors.is_multiple_of(2) {
            at += 1;
        }
    } else {
        at += 1;
    }
    let map_len = if maps > 0 { 1 << maplog } else { 0 };
    let mut cmap = Vec::with_capacity(maps * map_len);
    for k in 0..maps * map_len {
        cmap.push(u16le(d, at + 2 * k).ok_or_else(|| truncated("colour map"))?);
    }
    at += 2 * maps * map_len;
    if flags & 8 != 0 {
        let n = u16le(d, at).ok_or_else(|| truncated("comments"))? as usize;
        at += 2 + n + (n & 1);
    }
    Ok(Header {
        xpos: s(2),
        ypos: s(4),
        width,
        height,
        flags,
        ncolors,
        background,
        maps,
        map_len,
        cmap,
        data: at,
    })
}

pub fn info(data: &[u8]) -> Result<Info> {
    let h = header(data)?;
    let alpha = h.flags & 4 != 0;
    let mut coding = format!(
        "8-bit, {} channel{}",
        h.ncolors,
        if h.ncolors == 1 { "" } else { "s" }
    );
    if alpha {
        coding.push_str(" + alpha");
    }
    if h.maps > 0 {
        coding.push_str(&format!(", {}-entry colour map", h.map_len));
    }
    let model = if h.ncolors == 1 && h.maps == 3 {
        Model::Palette
    } else if h.ncolors == 1 && h.maps < 3 {
        Model::Grey
    } else {
        Model::Rgb
    };
    Ok(Info::new(h.width, h.height, PixelFormat::Rgba8, model, 8, coding).alpha(alpha))
}

pub fn decode(d: &[u8]) -> Result<Image> {
    let h = header(d)?;
    let (w, ht) = (h.width as usize, h.height as usize);
    check_payload(d.len(), (w * ht) as u64, 2000)?;
    let alpha = h.flags & 4 != 0;
    let nch = h.ncolors.min(MAX_CHANNELS);
    // Planes: R, G, B (fewer when fewer channels), alpha; bottom line first.
    let mut planes = vec![vec![0u8; w * ht]; 4];
    if h.flags & 2 == 0 {
        for (c, p) in planes.iter_mut().take(nch).enumerate() {
            p.fill(h.background.get(c).copied().unwrap_or(0));
        }
    }
    planes[3].fill(if alpha { 0 } else { 255 });
    let mut i = h.data;
    let (mut x, mut y) = (0usize, 0usize);
    let mut ch: Option<usize> = Some(0);
    let mut ended = false;
    let xoff = 0usize;
    let _ = (h.xpos, h.ypos);
    while i + 1 < d.len() {
        let op = d[i];
        let long = op & 0x40 != 0;
        let (operand, next) = if long {
            match u16le(d, i + 2) {
                Some(v) => (v as usize, i + 4),
                None => break,
            }
        } else {
            (d[i + 1] as usize, i + 2)
        };
        i = next;
        match op & !0x40 {
            1 => {
                y += operand;
                x = xoff;
            }
            2 => {
                ch = match operand {
                    255 => Some(3),
                    c if c < nch => Some(c),
                    _ => None,
                };
                x = xoff;
            }
            3 => x += operand,
            5 => {
                let n = operand + 1;
                let Some(src) = d.get(i..i + n) else {
                    break;
                };
                i += n + (n & 1);
                if let Some(c) = ch
                    && y < ht
                {
                    for (k, &v) in src.iter().enumerate() {
                        if x + k < w {
                            planes[c][y * w + x + k] = v;
                        }
                    }
                }
                x += n;
            }
            6 => {
                let n = operand + 1;
                let Some(v) = u16le(d, i) else {
                    break;
                };
                i += 2;
                if let Some(c) = ch
                    && y < ht
                {
                    let end = (x + n).min(w);
                    if x < end {
                        planes[c][y * w + x..y * w + end].fill(v as u8);
                    }
                }
                x += n;
            }
            7 => {
                ended = true;
                break;
            }
            _ => break,
        }
        if y >= ht {
            ended = true;
            break;
        }
    }
    let map = |c: usize, v: u8| -> u8 {
        if h.maps == 0 {
            return v;
        }
        let m = c.min(h.maps - 1);
        (h.cmap.get(m * h.map_len + v as usize).copied().unwrap_or(0) >> 8) as u8
    };
    let mut img = Image::filled(h.width, h.height, PixelFormat::Rgba8, 0);
    for yy in 0..ht {
        let src_y = ht - 1 - yy;
        for xx in 0..w {
            let k = src_y * w + xx;
            let px = if nch == 1 {
                let v = planes[0][k];
                if h.maps >= 3 {
                    [map(0, v), map(1, v), map(2, v)]
                } else {
                    let g = map(0, v);
                    [g, g, g]
                }
            } else {
                [
                    map(0, planes[0][k]),
                    map(1, planes[1][k]),
                    map(2, planes[2.min(nch - 1)][k]),
                ]
            };
            let o = (yy * w + xx) * 4;
            img.data[o..o + 4].copy_from_slice(&[px[0], px[1], px[2], planes[3][k]]);
        }
    }
    img.truncated = !ended;
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(w: u16, h: u16, flags: u8, ncolors: u8) -> Vec<u8> {
        let mut f = MAGIC.to_le_bytes().to_vec();
        for v in [0u16, 0, w, h] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        f.extend_from_slice(&[flags, ncolors, 8, 0, 0]);
        f
    }

    #[test]
    fn rgb_with_background() {
        let mut f = head(3, 2, 0, 3);
        f.extend_from_slice(&[10, 20, 30]); // background (odd count: no filler)
        // Bottom line: R byte data 3, G run 3 of 7, B skip 1 then 1 byte.
        f.extend_from_slice(&[2, 0, 5, 2, 1, 2, 3, 0]);
        f.extend_from_slice(&[2, 1, 6, 2, 7, 0]);
        f.extend_from_slice(&[2, 2, 3, 1, 5, 0, 99, 0]);
        f.extend_from_slice(&[7, 0]);
        let img = decode(&f).unwrap();
        assert!(!img.truncated);
        // Top line is all background.
        assert_eq!(&img.data[..4], &[10, 20, 30, 255]);
        // Bottom line: (1,7,30), (2,7,99), (3,7,30).
        assert_eq!(
            &img.data[12..24],
            &[1, 7, 30, 255, 2, 7, 99, 255, 3, 7, 30, 255]
        );
    }

    #[test]
    fn grey_alpha_long_ops() {
        let mut f = head(2, 1, 2 | 4, 1);
        f.push(0); // filler
        f.extend_from_slice(&[0x46, 0, 1, 0, 50, 0]); // long run data: 2 × 50
        f.extend_from_slice(&[2, 255, 6, 1, 200, 0, 7, 0]);
        let i = info(&f).unwrap();
        assert!(i.has_alpha);
        let img = decode(&f).unwrap();
        assert_eq!(img.data, [50, 50, 50, 200, 50, 50, 50, 200]);
    }
}
