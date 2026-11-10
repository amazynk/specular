//! Wavefront RLA (`.rla`) and 3ds Max Rich Pixel Format (`.rpf`, RLA with extra channels).
//!
//! A 740-byte big-endian header: the window and active window (left, right, bottom, top),
//! frame, storage type, colour, matte and auxiliary channel counts, revision (0xFFFE for the
//! later layout), text fields, and the bits per colour and matte channel. Then one 32-bit
//! offset per scan line, bottom line first. A line holds each colour channel, then each
//! matte channel, as a 16-bit length and run-length bytes: a signed count c ≥ 0 repeats the
//! next byte c + 1 times, c < 0 copies −c bytes. Auxiliary channels (depth, normals…) are
//! skipped.

use crate::*;

const HEADER: usize = 740;

struct Header {
    width: u32,
    height: u32,
    colour: usize,
    matte: usize,
    bits: u16,
}

fn header(data: &[u8]) -> Result<Header> {
    if data.len() < HEADER {
        return Err(Error::NotRecognised);
    }
    let s = |i: usize| i32::from(u16be(data, i).unwrap_or(0) as i16);
    let (left, right, bottom, top) = (s(8), s(10), s(12), s(14));
    let colour = s(20);
    let matte = s(22);
    let storage = s(18);
    if right < left || top < bottom || !(0..=4).contains(&colour) || !(0..=4).contains(&matte) {
        return Err(Error::NotRecognised);
    }
    if storage != 0 {
        return Err(Error::Unsupported(format!("RLA storage type {storage}")));
    }
    let bits = match u16be(data, 658).unwrap_or(0) {
        0 => 8,
        b => b,
    };
    let width = (right - left + 1) as u32;
    let height = (top - bottom + 1) as u32;
    check_dims(width, height)?;
    Ok(Header {
        width,
        height,
        colour: colour as usize,
        matte: matte as usize,
        bits,
    })
}

/// Whether the header is plausible and its line offsets point into the file.
pub fn probe_file(data: &[u8]) -> bool {
    header(data).is_ok_and(|h| {
        let first = u32be(data, HEADER).unwrap_or(0) as usize;
        let ok_end = HEADER + 4 * h.height as usize;
        h.colour + h.matte > 0 && first >= ok_end && first < data.len()
    })
}

pub fn info(data: &[u8]) -> Result<Info> {
    let h = header(data)?;
    if h.bits != 8 {
        return Err(Error::Unsupported(format!("{}-bit RLA channels", h.bits)));
    }
    let alpha = h.matte > 0;
    let grey = h.colour == 1 || (h.colour == 0 && h.matte > 0);
    Ok(Info::new(
        h.width,
        h.height,
        PixelFormat::Rgba8,
        if grey { Model::Grey } else { Model::Rgb },
        8,
        format!(
            "8-bit, {} colour + {} matte channels, RLE",
            h.colour, h.matte
        ),
    )
    .alpha(alpha))
}

/// Decodes one run-length channel into `out` (stride 4); false when the data is short.
fn channel(src: &[u8], out: &mut [u8], c: usize) -> bool {
    let w = out.len() / 4;
    let mut x = 0;
    let mut i = 0;
    while x < w {
        let Some(&n) = src.get(i) else {
            return false;
        };
        i += 1;
        let n = n as i8;
        if n >= 0 {
            let Some(&v) = src.get(i) else {
                return false;
            };
            i += 1;
            for _ in 0..=n as usize {
                if x < w {
                    out[x * 4 + c] = v;
                    x += 1;
                }
            }
        } else {
            let k = (-(n as i32)) as usize;
            let Some(lit) = src.get(i..i + k) else {
                return false;
            };
            i += k;
            for &v in lit {
                if x < w {
                    out[x * 4 + c] = v;
                    x += 1;
                }
            }
        }
    }
    true
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let h = header(data)?;
    if h.bits != 8 {
        return Err(Error::Unsupported(format!("{}-bit RLA channels", h.bits)));
    }
    let (w, ht) = (h.width as usize, h.height as usize);
    let mut img = Image::filled(h.width, h.height, PixelFormat::Rgba8, GREY);
    let mut bad = false;
    for line in 0..ht {
        let y = ht - 1 - line;
        let row = &mut img.data[y * w * 4..(y + 1) * w * 4];
        let Some(off) = u32be(data, HEADER + 4 * line) else {
            bad = true;
            break;
        };
        let mut at = off as usize;
        let mut ok = true;
        // Colour channels R, G, B (one channel: grey), then the matte into alpha.
        for k in 0..h.colour + h.matte {
            let Some(len) = u16be(data, at) else {
                ok = false;
                break;
            };
            let src = data.get(at + 2..at + 2 + len as usize).unwrap_or(&[]);
            at += 2 + len as usize;
            let c = if k < h.colour { k.min(2) } else { 3 };
            if k > h.colour {
                // Only the first matte channel is alpha.
                continue;
            }
            ok &= channel(src, row, c);
        }
        if h.colour == 1 {
            for px in row.chunks_exact_mut(4) {
                px[1] = px[0];
                px[2] = px[0];
            }
        }
        if h.matte == 0 {
            for px in row.chunks_exact_mut(4) {
                px[3] = 255;
            }
        }
        bad |= !ok;
    }
    img.truncated = bad;
    Ok(img)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// An RLA with `channels` colour + `matte` channels; `px(x, y, c)` top-down.
    pub fn build(
        w: u16,
        h: u16,
        colour: u16,
        matte: u16,
        px: impl Fn(usize, usize, usize) -> u8,
    ) -> Vec<u8> {
        let mut f = vec![0u8; HEADER];
        let put =
            |f: &mut Vec<u8>, at: usize, v: u16| f[at..at + 2].copy_from_slice(&v.to_be_bytes());
        for (at, v) in [
            (0, 0),
            (2, w - 1),
            (4, 0),
            (6, h - 1),
            (8, 0),
            (10, w - 1),
            (12, 0),
            (14, h - 1),
        ] {
            put(&mut f, at, v);
        }
        put(&mut f, 20, colour);
        put(&mut f, 22, matte);
        put(&mut f, 26, 0xfffe);
        put(&mut f, 658, 8);
        put(&mut f, 662, 8);
        let table = f.len();
        f.resize(table + 4 * h as usize, 0);
        for line in 0..h as usize {
            let y = h as usize - 1 - line;
            let off = f.len() as u32;
            f[table + 4 * line..table + 4 * line + 4].copy_from_slice(&off.to_be_bytes());
            for c in 0..(colour + matte) as usize {
                // Literal runs of up to 128.
                let vals: Vec<u8> = (0..w as usize).map(|x| px(x, y, c)).collect();
                let mut enc = Vec::new();
                for ch in vals.chunks(128) {
                    enc.push((-(ch.len() as i32)) as i8 as u8);
                    enc.extend_from_slice(ch);
                }
                f.extend_from_slice(&(enc.len() as u16).to_be_bytes());
                f.extend_from_slice(&enc);
            }
        }
        f
    }

    #[test]
    fn rgba_bottom_up() {
        let f = build(3, 2, 3, 1, |x, y, c| (x * 10 + y * 100 + c) as u8);
        assert!(probe_file(&f));
        let i = info(&f).unwrap();
        assert!(i.has_alpha);
        let img = decode(&f).unwrap();
        assert!(!img.truncated);
        assert_eq!(&img.data[..4], &[0, 1, 2, 3]);
        assert_eq!(&img.data[(3 + 2) * 4..][..4], &[120, 121, 122, 123]);
    }

    #[test]
    fn runs() {
        let mut row = [0u8; 12];
        assert!(channel(&[2, 9], &mut row, 1));
        assert_eq!(row, [0, 9, 0, 0, 0, 9, 0, 0, 0, 9, 0, 0]);
        assert!(!channel(&[0xfe, 1], &mut row, 0));
    }
}
