//! Dr. Halo CUT (`.cut`) with its palette file (`.pal`).
//!
//! CUT: width, height and a reserved word (little-endian), then per scan line a byte count
//! and run-length data: a byte with bit 7 set repeats the next byte (low 7 bits) times, a
//! byte below 0x80 copies that many bytes, 0 ends the line. Pixels are 8-bit palette indices.
//!
//! PAL: a 40-byte header ("AH", version, size, file type 0x0A, subtype, board, mode, the
//! largest index, the largest red, green and blue values), then red, green and blue words per
//! index in 512-byte blocks that entries never straddle. Without one the indices show as
//! grey levels.

use crate::*;

fn header(data: &[u8]) -> Result<(u32, u32)> {
    let w = u32::from(u16le(data, 0).ok_or_else(|| truncated("header"))?);
    let h = u32::from(u16le(data, 2).ok_or_else(|| truncated("header"))?);
    check_dims(w, h)?;
    Ok((w, h))
}

/// Whether the file's first line decodes to the header's width (no signature otherwise).
pub fn probe_file(data: &[u8]) -> bool {
    let Ok((w, _)) = header(data) else {
        return false;
    };
    let mut row = vec![0u8; w as usize];
    let mut i = 6;
    line(data, &mut i, &mut row) == Some(w as usize)
}

/// Decodes one line from `*i` into `out`; the pixels written, `None` when data ends.
fn line(d: &[u8], i: &mut usize, out: &mut [u8]) -> Option<usize> {
    let count = u16le(d, *i)? as usize;
    let end = *i + 2 + count;
    let mut j = *i + 2;
    let mut o = 0;
    while j < end.min(d.len()) {
        let b = d[j];
        j += 1;
        if b == 0 {
            break;
        }
        let n = (b & 0x7f) as usize;
        if b & 0x80 != 0 {
            let v = *d.get(j)?;
            j += 1;
            let k = n.min(out.len() - o);
            out[o..o + k].fill(v);
            o += k;
        } else {
            let lit = d.get(j..j + n)?;
            j += n;
            let k = n.min(out.len() - o);
            out[o..o + k].copy_from_slice(&lit[..k]);
            o += k;
        }
    }
    if end > d.len() {
        return None;
    }
    *i = end;
    Some(o)
}

/// The colours of a Dr. Halo palette file.
pub fn palette(pal: &[u8]) -> Option<Vec<[u8; 4]>> {
    if pal.get(..2)? != b"AH" || *pal.get(6)? != 0x0a {
        return None;
    }
    let max_index = u16le(pal, 12)? as usize;
    let maxes = [u16le(pal, 14)?, u16le(pal, 16)?, u16le(pal, 18)?].map(u32::from);
    let mut out = Vec::new();
    let mut at: usize = 40;
    for _ in 0..=max_index.min(255) {
        // Entries do not straddle 512-byte blocks.
        if at % 512 + 6 > 512 {
            at = at.div_ceil(512) * 512;
        }
        let c = |k: usize| u16le(pal, at + 2 * k).map(|v| scale_to_8(u32::from(v), maxes[k]));
        out.push([c(0)?, c(1)?, c(2)?, 255]);
        at += 6;
    }
    Some(out)
}

pub fn info(data: &[u8], pal: Option<&[u8]>) -> Result<Info> {
    let (w, h) = header(data)?;
    let has = pal.and_then(palette).is_some();
    Ok(if has {
        Info::new(
            w,
            h,
            PixelFormat::Rgba8,
            Model::Palette,
            8,
            "8-bit RLE, .pal palette",
        )
    } else {
        Info::new(
            w,
            h,
            PixelFormat::Gray8,
            Model::Grey,
            8,
            "8-bit RLE, no .pal: grey",
        )
    })
}

pub fn decode(data: &[u8], pal: Option<&[u8]>) -> Result<Image> {
    let (w, h) = header(data)?;
    check_payload(data.len(), u64::from(w) * u64::from(h), 127)?;
    let mut idx = vec![0u8; w as usize * h as usize];
    let mut i = 6;
    let mut rows = 0;
    for row in idx.chunks_exact_mut(w as usize) {
        if line(data, &mut i, row).is_none() {
            break;
        }
        rows += 1;
    }
    let mut img = match pal.and_then(palette) {
        Some(p) => {
            let mut img = Image::from_indices(w, h, &idx, &p);
            for px in img.data[rows * w as usize * 4..].chunks_exact_mut(4) {
                px.copy_from_slice(&[GREY, GREY, GREY, 255]);
            }
            img
        }
        None => {
            // Stretch the indices used to the full grey range.
            let max = idx.iter().copied().max().unwrap_or(0).max(1);
            let mut img = Image::filled(w, h, PixelFormat::Gray8, GREY);
            for (o, &v) in img.data.iter_mut().zip(&idx[..rows * w as usize]) {
                *o = scale_to_8(u32::from(v), u32::from(max));
            }
            img
        }
    };
    img.truncated = rows < h as usize;
    Ok(img)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn cut(w: u16, rows: &[Vec<u8>]) -> Vec<u8> {
        let mut f = Vec::new();
        for v in [w, rows.len() as u16, 0] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        for r in rows {
            let mut enc = Vec::new();
            for chunk in r.chunks(127) {
                if chunk.iter().all(|&v| v == chunk[0]) {
                    enc.extend_from_slice(&[0x80 | chunk.len() as u8, chunk[0]]);
                } else {
                    enc.push(chunk.len() as u8);
                    enc.extend_from_slice(chunk);
                }
            }
            enc.push(0);
            f.extend_from_slice(&(enc.len() as u16).to_le_bytes());
            f.extend_from_slice(&enc);
        }
        f
    }

    pub fn pal(colours: &[[u16; 3]], max: u16) -> Vec<u8> {
        let mut p = b"AH".to_vec();
        p.extend_from_slice(&[0, 0, 0, 0, 0x0a, 0, 0, 0, 0, 0]);
        p.extend_from_slice(&(colours.len() as u16 - 1).to_le_bytes());
        for _ in 0..3 {
            p.extend_from_slice(&max.to_le_bytes());
        }
        p.extend_from_slice(b"Dr. Halo\0\0\0\0\0\0\0\0\0\0\0\0");
        for c in colours {
            if p.len() % 512 + 6 > 512 {
                p.resize(p.len().div_ceil(512) * 512, 0);
            }
            for v in c {
                p.extend_from_slice(&v.to_le_bytes());
            }
        }
        p
    }

    #[test]
    fn with_and_without_palette() {
        let rows = vec![vec![0, 1, 2, 2, 2, 2], vec![3; 6]];
        let f = cut(6, &rows);
        assert!(probe_file(&f));
        let grey = decode(&f, None).unwrap();
        assert_eq!(&grey.data[..6], &[0, 85, 170, 170, 170, 170]);
        assert_eq!(&grey.data[6..], &[255; 6]);
        let colours: Vec<[u16; 3]> = (0..100).map(|i| [i, 0, 99 - i]).collect();
        let p = pal(&colours, 99);
        let img = decode(&f, Some(&p)).unwrap();
        assert_eq!(&img.data[4..8], &[3, 0, 252, 255]);
        assert_eq!(palette(&p).unwrap()[90], [232, 0, 23, 255]);
        assert!(decode(&f[..12], None).unwrap().truncated);
    }
}
