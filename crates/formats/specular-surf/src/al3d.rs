//! Alicona `.al3d` (focus variation): `AliconaImaging\0\r\n`, then 52-byte tags (a 20-byte
//! key, a 30-byte value, CR LF) beginning with `Version` and `TagCount`, a 256-byte comment
//! and the images at the offsets the tags give.
//!
//! The depth image is float32 metres, rows padded up to a multiple of eight samples when
//! the file leaves room for it, with `InvalidPixelValue` (or NaN) where nothing was
//! measured. The texture (`NumberOfPlanes` 8-bit planes, one after another) becomes a grey
//! or colour page. Rows are top-down as Alicona's software shows them.

use std::collections::HashMap;

use specular_em::dataset::{Dataset, Plane};
use specular_em::samples::Sample;

use crate::{Error, Result, alloc, area, f32le, height, meta, page, slice, text};

const MAGIC: &[u8] = b"AliconaImaging\0\r\n";
const TAG: usize = 52;

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

fn tags(file: &[u8]) -> Result<(HashMap<String, String>, usize)> {
    let mut map = HashMap::new();
    let mut p = MAGIC.len();
    let mut count = 2;
    let mut k = 0;
    while k < count {
        let t = slice(file, p, TAG, "tags")?;
        let key = text(t, 0, 20);
        let value = text(t, 20, 30);
        if key == "TagCount" {
            // The count is of the tags after Version and TagCount.
            count = 2 + value.parse::<usize>().unwrap_or(0).min(1024);
        }
        map.insert(key, value);
        p += TAG;
        k += 1;
    }
    Ok((map, p))
}

/// The row stride (samples) of an image of `rows × cols` samples of `size` bytes at `at`,
/// ending at most at `end`: the columns rounded up to a multiple of eight when they fit.
fn stride(cols: usize, rows: usize, size: usize, at: usize, end: usize) -> usize {
    let padded = cols.div_ceil(8) * 8;
    let room = end.saturating_sub(at);
    if padded != cols && room >= padded * rows * size {
        padded
    } else {
        cols
    }
}

pub fn read(file: &[u8]) -> Result<Dataset> {
    if !probe(file) {
        return Err(Error::NotFormat("Alicona AL3D"));
    }
    let (t, end) = tags(file)?;
    let num = |k: &str| t.get(k).and_then(|v| v.trim().parse::<f64>().ok());
    let int = |k: &str| t.get(k).and_then(|v| v.trim().parse::<usize>().ok());
    let (cols, rows) = match (int("Cols"), int("Rows")) {
        (Some(c), Some(r)) => (c, r),
        _ => return Err(Error::Malformed("no image size".into())),
    };
    let n = area(cols, rows)?;
    let depth = int("DepthImageOffset").unwrap_or(0);
    let texture = int("TextureImageOffset").unwrap_or(0);
    if depth == 0 {
        return Err(Error::NoImage);
    }
    let limit = if texture > depth { texture } else { file.len() };
    let s = stride(cols, rows, 4, depth, limit);
    let data = slice(file, depth, (rows - 1) * s * 4 + cols * 4, "depth image")?;
    let invalid = num("InvalidPixelValue").map(|v| v as f32);
    let mut v = alloc(n)?;
    for y in 0..rows {
        let row = &data[y * s * 4..(y * s + cols) * 4];
        v.extend(row.chunks_exact(4).map(|c| {
            let z = f32le(c, 0).unwrap_or(f32::NAN);
            if Some(z) == invalid { f32::NAN } else { z }
        }));
    }
    let dx = num("PixelSizeXMeter").unwrap_or(0.0);
    let dy = num("PixelSizeYMeter").unwrap_or(0.0);
    let mut pg = page(cols, rows, dx, dy);
    pg.planes.push(height(v));
    pg.sample = Some(Sample::F32);
    pg.levelling = true;
    pg.instrument = Some("Alicona".into());
    for (k, v) in [
        ("Version", "Version"),
        ("Invalid value", "InvalidPixelValue"),
        ("Texture planes", "NumberOfPlanes"),
    ] {
        meta(&mut pg, k, t.get(v).cloned().unwrap_or_default());
    }
    meta(&mut pg, "Comment", text(file, end, 256));
    let mut pages = vec![pg];

    let planes = int("NumberOfPlanes").unwrap_or(0);
    if texture != 0 && matches!(planes, 1 | 3 | 4) {
        let s = stride(cols, rows, 1, texture, file.len() / planes.max(1));
        let names = ["Red", "Green", "Blue"];
        let mut tex = page(cols, rows, dx, dy);
        for (k, name) in names.iter().enumerate().take(planes) {
            let at = texture + k * rows * s;
            let Ok(b) = slice(file, at, (rows - 1) * s + cols, "texture") else {
                break;
            };
            let mut p = alloc(n)?;
            for y in 0..rows {
                p.extend(b[y * s..y * s + cols].iter().map(|&c| f32::from(c) / 255.0));
            }
            let label = if planes == 1 { "Texture" } else { name };
            tex.planes.push(Plane::values(label, "", p));
        }
        if !tex.planes.is_empty() {
            tex.rgb = tex.planes.len() == 3;
            tex.sample = Some(Sample::U8);
            tex.title = Some("Texture".into());
            pages.push(tex);
        }
    }
    Ok(Dataset { pages })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(f: &mut Vec<u8>, k: &str, v: &str) {
        let mut t = [0u8; TAG];
        t[..k.len()].copy_from_slice(k.as_bytes());
        t[20..20 + v.len()].copy_from_slice(v.as_bytes());
        t[50..].copy_from_slice(b"\r\n");
        f.extend_from_slice(&t);
    }

    #[test]
    fn depth_with_padding_and_invalid() {
        let mut f = MAGIC.to_vec();
        let header = MAGIC.len() + 7 * TAG + 256;
        tag(&mut f, "Version", "1");
        tag(&mut f, "TagCount", "5");
        tag(&mut f, "Cols", "3");
        tag(&mut f, "Rows", "2");
        tag(&mut f, "DepthImageOffset", &header.to_string());
        tag(&mut f, "InvalidPixelValue", "3e15");
        tag(&mut f, "PixelSizeXMeter", "1e-6");
        f.resize(header, 0);
        for z in [1.0f32, 2.0, 3e15, 0.0, 0.0, 0.0, 0.0, 0.0, 4.0, 5.0, 6.0] {
            f.extend_from_slice(&z.to_le_bytes());
        }
        f.extend_from_slice(&[0; 20]);
        let d = read(&f).unwrap();
        let (z, _) = d.pages[0].read_plane(&f, 0).unwrap();
        assert_eq!(z[..2], [1.0, 2.0]);
        assert!(z[2].is_nan());
        assert_eq!(z[3..], [4.0, 5.0, 6.0]);
        assert!(d.pages[0].y.is_none());
        assert!(read(&f[..200]).is_err());
    }
}
