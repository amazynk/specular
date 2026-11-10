//! Wireless BMP type 0: multi-byte-integer header, 1-bit rows MSB-first, a set bit is white.
//! There is no magic number, so [`probe`] requires the whole file and an exact size match.

use crate::*;

/// Multi-byte integer: 7 bits per byte, high bit = more follows.
fn mbi(d: &[u8], i: &mut usize) -> Option<u32> {
    let mut v: u32 = 0;
    for _ in 0..5 {
        let b = *d.get(*i)?;
        *i += 1;
        v = v.checked_mul(128)? | (b & 0x7F) as u32;
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

fn header(data: &[u8]) -> Option<(u32, u32, usize)> {
    let mut i = 0;
    if mbi(data, &mut i)? != 0 || *data.get(i)? != 0 {
        return None;
    }
    i += 1;
    let w = mbi(data, &mut i)?;
    let h = mbi(data, &mut i)?;
    Some((w, h, i))
}

/// `data` must be the whole file.
pub fn probe(data: &[u8]) -> bool {
    match header(data) {
        Some((w, h, at)) if w > 0 && h > 0 => {
            (w as u64).div_ceil(8) * h as u64 + at as u64 == data.len() as u64
        }
        _ => false,
    }
}

fn checked(data: &[u8]) -> Result<(u32, u32, usize)> {
    let (w, h, at) = header(data).ok_or(Error::NotRecognised)?;
    check_dims(w, h)?;
    Ok((w, h, at))
}

pub fn info(data: &[u8]) -> Result<Info> {
    let (width, height, _) = checked(data)?;
    Ok(Info {
        format: Format::Wbmp,
        width,
        height,
        pixel_format: PixelFormat::Gray8,
        has_alpha: false,
        model: crate::Model::Grey,
        bit_depth: 1,
        coding: "1-bit".into(),
    })
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let (w, h, at) = checked(data)?;
    let (w, h) = (w as usize, h as usize);
    let src = &data[at..];
    let rb = w.div_ceil(8);
    check_payload(src.len(), (rb * h) as u64, 1)?;
    let mut img = Image::new(w as u32, h as u32, PixelFormat::Gray8);
    for (y, out) in img.data.chunks_exact_mut(w).enumerate() {
        let Some(r) = src.get(y * rb..(y + 1) * rb) else {
            out.fill(GREY);
            img.truncated = true;
            continue;
        };
        for (x, o) in out.iter_mut().enumerate() {
            *o = if r[x >> 3] & (0x80 >> (x & 7)) != 0 {
                255
            } else {
                0
            };
        }
    }
    Ok(img)
}
