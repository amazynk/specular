//! Alias PIX (`.pix`, Alias/Wavefront renderers): a 10-byte big-endian header (width,
//! height, x and y offsets, bits per pixel 24 or 8), then run-length packets: a count (1‥255)
//! and one pixel, blue, green, red for 24 bits or a grey (matte) byte for 8. No signature:
//! chosen by extension and checked by decoding the first line.

use crate::*;

fn header(data: &[u8]) -> Result<(u32, u32, u16)> {
    let w = u32::from(u16be(data, 0).ok_or_else(|| truncated("header"))?);
    let h = u32::from(u16be(data, 2).ok_or_else(|| truncated("header"))?);
    let bpp = u16be(data, 8).ok_or_else(|| truncated("header"))?;
    if bpp != 24 && bpp != 8 {
        return Err(Error::NotRecognised);
    }
    check_dims(w, h)?;
    Ok((w, h, bpp))
}

pub fn probe_file(data: &[u8]) -> bool {
    let Ok((w, h, bpp)) = header(data) else {
        return false;
    };
    // The packets of the first line add up to its width exactly (runs end at line ends).
    let n = if bpp == 24 { 4 } else { 2 };
    let mut x = 0u32;
    let mut i = 10;
    while x < w {
        match data.get(i) {
            Some(&0) | None => return false,
            Some(&c) => x += u32::from(c),
        }
        i += n;
    }
    x == w && (h == 1 || i < data.len())
}

pub fn info(data: &[u8]) -> Result<Info> {
    let (w, h, bpp) = header(data)?;
    Ok(if bpp == 24 {
        Info::new(w, h, PixelFormat::Rgba8, Model::Rgb, 8, "24-bit RLE")
    } else {
        Info::new(w, h, PixelFormat::Gray8, Model::Grey, 8, "8-bit matte, RLE")
    })
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let (w, h, bpp) = header(data)?;
    let n = if bpp == 24 { 4 } else { 2 };
    let total = w as usize * h as usize;
    check_payload(data.len(), total as u64, 64)?;
    let fmt = if bpp == 24 {
        PixelFormat::Rgba8
    } else {
        PixelFormat::Gray8
    };
    let mut img = Image::filled(w, h, fmt, GREY);
    if bpp == 24 {
        for px in img.data.chunks_exact_mut(4) {
            px[3] = 255;
        }
    }
    let mut o = 0;
    for p in data[10..].chunks_exact(n) {
        if o >= total {
            break;
        }
        let k = (p[0] as usize).min(total - o);
        if bpp == 24 {
            for px in img.data[o * 4..(o + k) * 4].chunks_exact_mut(4) {
                px.copy_from_slice(&[p[3], p[2], p[1], 255]);
            }
        } else {
            img.data[o..o + k].fill(p[1]);
        }
        o += k;
    }
    img.truncated = o < total;
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_and_matte() {
        let mut f = vec![0, 3, 0, 2, 0, 0, 0, 0, 0, 24];
        f.extend_from_slice(&[2, 1, 2, 3, 1, 4, 5, 6, 3, 7, 8, 9]);
        assert!(probe_file(&f));
        let img = decode(&f).unwrap();
        assert!(!img.truncated);
        assert_eq!(&img.data[..12], &[3, 2, 1, 255, 3, 2, 1, 255, 6, 5, 4, 255]);
        assert_eq!(&img.data[20..24], &[9, 8, 7, 255]);
        let mut m = vec![0, 2, 0, 1, 0, 0, 0, 0, 0, 8];
        m.extend_from_slice(&[2, 77]);
        assert_eq!(decode(&m).unwrap().data, [77, 77]);
        assert!(!probe_file(&[0, 2, 0, 1, 0, 0, 0, 0, 0, 16]));
    }
}
