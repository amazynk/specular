//! farbfeld: "farbfeld", u32 BE width and height, then 16-bit big-endian RGBA.

use rayon::prelude::*;

use crate::*;

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"farbfeld")
}

fn header(data: &[u8]) -> Result<(u32, u32)> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let w = u32be(data, 8).ok_or_else(|| Error::Truncated("header".into()))?;
    let h = u32be(data, 12).ok_or_else(|| Error::Truncated("header".into()))?;
    check_dims(w, h)?;
    Ok((w, h))
}

pub fn info(data: &[u8]) -> Result<Info> {
    let (width, height) = header(data)?;
    Ok(Info {
        format: Format::Farbfeld,
        width,
        height,
        pixel_format: PixelFormat::Rgba8,
        has_alpha: any_transparent(data, width, height),
        model: crate::Model::Rgb,
        bit_depth: 16,
        coding: "16-bit RGBA".into(),
    })
}

/// Whether some alpha sample is below 0xffff, or pixels are missing (they decode grey at half
/// alpha). Files over [`ALPHA_SCAN_MAX`] are not read: true.
fn any_transparent(data: &[u8], w: u32, h: u32) -> bool {
    let n = u64::from(w) * u64::from(h) * 8;
    if data.len() > ALPHA_SCAN_MAX || ((data.len() - 16) as u64) < n {
        return true;
    }
    data[16..16 + n as usize]
        .chunks_exact(8)
        .any(|px| px[6] != 0xff || px[7] != 0xff)
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let (w, h) = header(data)?;
    let src = &data[16..];
    check_payload(src.len(), w as u64 * h as u64 * 8, 64)?;
    let mut img = Image::new(w, h, PixelFormat::Rgba8);
    let row = w as usize * 4;
    img.truncated = src.len() < row * 2 * h as usize;
    img.data
        .par_chunks_mut(row)
        .enumerate()
        .with_min_len(16)
        .for_each(|(y, out)| {
            let s = src.get(y * row * 2..).unwrap_or(&[]);
            let n = (s.len() / 2).min(row);
            for (o, b) in out[..n].iter_mut().zip(s.chunks_exact(2)) {
                *o = u16_to_u8(u16::from_be_bytes([b[0], b[1]]));
            }
            out[n..].fill(GREY);
        });
    Ok(img)
}
