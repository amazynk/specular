//! Keyence VK-X laser confocal `.vk4`: little-endian, `VK4_` then a table of block offsets
//! (measurement settings, colour images, laser intensity and height images, thumbnails,
//! strings).
//!
//! The settings give the pixel pitch in x and y and the height per digit, in picometres.
//! Height images are 32-bit counts, intensity images 16-bit (each after a 768-byte palette),
//! colour images 24-bit BGR. Rows are top-down as VK Viewer shows them. Pages: the height
//! map with the laser intensity as a second plane, then the colour image and the laser +
//! colour image (red, green and blue planes in 0..=1).

use specular_em::dataset::{Dataset, Page, Plane};
use specular_em::samples::Sample;

use crate::{Error, Result, alloc, area, height, meta, page, slice, u32le};

const MAGIC: &[u8] = b"VK4_";
/// Settings field indices (32-bit words).
const X_PITCH: usize = 42;
const Y_PITCH: usize = 43;
const Z_PER_DIGIT: usize = 44;
/// The offset table: settings, colour peak, colour + laser, laser ×3, height ×3, …,
/// strings (index 16).
const TABLE: usize = 12;

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

fn offset(file: &[u8], k: usize) -> usize {
    u32le(file, TABLE + 4 * k).unwrap_or(0) as usize
}

/// A grey image block (palette then samples) of `bits` per sample: width, height, values.
fn grey(file: &[u8], at: usize, bits: u32, what: &'static str) -> Result<(usize, usize, Vec<f32>)> {
    let h = slice(file, at, 20, what)?;
    let g = |o| u32le(h, o).unwrap_or(0) as usize;
    let (w, ht, depth) = (g(0), g(4), g(8) as u32);
    if depth != bits {
        return Err(Error::Unsupported(format!("{depth}-bit {what}")));
    }
    let n = area(w, ht)?;
    let size = (bits / 8) as usize;
    let data = slice(file, at + 796, n * size, what)?;
    let mut v = alloc(n)?;
    if size == 4 {
        v.extend(
            data.chunks_exact(4)
                .map(|c| u32le(c, 0).unwrap_or(0) as f32),
        );
    } else {
        v.extend(
            data.chunks_exact(2)
                .map(|c| f32::from(crate::u16le(c, 0).unwrap_or(0))),
        );
    }
    Ok((w, ht, v))
}

/// A 24-bit BGR colour image block as red, green and blue planes in 0..=1.
fn colour(file: &[u8], at: usize) -> Result<(usize, usize, [Vec<f32>; 3])> {
    let h = slice(file, at, 20, "colour image")?;
    let g = |o| u32le(h, o).unwrap_or(0) as usize;
    let (w, ht, depth) = (g(0), g(4), g(8));
    if depth != 24 {
        return Err(Error::Unsupported(format!("{depth}-bit colour image")));
    }
    let n = area(w, ht)?;
    let data = slice(file, at + 20, n * 3, "colour image")?;
    let mut rgb = [alloc(n)?, alloc(n)?, alloc(n)?];
    for px in data.chunks_exact(3) {
        for (c, plane) in rgb.iter_mut().enumerate() {
            plane.push(f32::from(px[2 - c]) / 255.0);
        }
    }
    Ok((w, ht, rgb))
}

/// Length-prefixed UTF-16LE strings (title, lens name).
fn strings(file: &[u8], at: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut p = at;
    for _ in 0..2 {
        let Some(n) = u32le(file, p) else { break };
        let n = n as usize;
        let Some(b) = n
            .checked_mul(2)
            .and_then(|len| file.get(p + 4..p + 4 + len))
        else {
            break;
        };
        let units: Vec<u16> = b
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        out.push(String::from_utf16_lossy(&units).trim().to_string());
        p += 4 + 2 * n;
    }
    out
}

pub fn read(file: &[u8]) -> Result<Dataset> {
    if !probe(file) {
        return Err(Error::NotFormat("Keyence VK4"));
    }
    let settings = offset(file, 0);
    let s = slice(file, settings, 4 * 60, "settings")?;
    let word = |k: usize| u32le(s, 4 * k).unwrap_or(0);
    let pm = 1e-12;
    let dx = f64::from(word(X_PITCH)) * pm;
    let dy = f64::from(word(Y_PITCH)) * pm;
    let dz = f64::from(word(Z_PER_DIGIT)) * pm;
    let date = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        word(1),
        word(2),
        word(3),
        word(4),
        word(5),
        word(6)
    );
    let text = match offset(file, 16) {
        0 => Vec::new(),
        at => strings(file, at),
    };
    let finish = |pg: &mut Page| {
        pg.date = (word(1) > 0).then(|| date.clone());
        pg.instrument = Some("Keyence VK-X".into());
        pg.title = text.first().filter(|t| !t.is_empty()).cloned();
        if let Some(lens) = text.get(1).filter(|t| !t.is_empty()) {
            meta(pg, "Lens", lens.clone());
        }
        meta(pg, "Height per digit", format!("{} pm", word(Z_PER_DIGIT)));
    };

    let mut pages = Vec::new();
    let h0 = offset(file, 6);
    if h0 != 0 {
        let (w, ht, mut v) = grey(file, h0, 32, "height image")?;
        for z in v.iter_mut() {
            *z = (f64::from(*z) * dz) as f32;
        }
        let mut pg = page(w, ht, dx, dy);
        pg.planes.push(height(v));
        pg.sample = Some(Sample::U32);
        pg.levelling = true;
        let l0 = offset(file, 3);
        if l0 != 0
            && let Ok((lw, lh, l)) = grey(file, l0, 16, "laser image")
            && (lw, lh) == (w, ht)
        {
            pg.planes
                .push(Plane::values("Laser intensity", "counts", l));
        }
        finish(&mut pg);
        pages.push(pg);
    }
    for (k, title) in [(1, "Colour"), (2, "Laser + colour")] {
        let at = offset(file, k);
        if at == 0 {
            continue;
        }
        let Ok((w, ht, [r, g, b])) = colour(file, at) else {
            continue;
        };
        let mut pg = page(w, ht, dx, dy);
        pg.planes = vec![
            Plane::values("Red", "", r),
            Plane::values("Green", "", g),
            Plane::values("Blue", "", b),
        ];
        pg.rgb = true;
        pg.sample = Some(Sample::U8);
        finish(&mut pg);
        pg.title = Some(title.into());
        pages.push(pg);
    }
    if pages.is_empty() {
        return Err(Error::NoImage);
    }
    Ok(Dataset { pages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_and_colour() {
        let mut f = vec![0u8; 80];
        f[..4].copy_from_slice(MAGIC);
        let settings = f.len();
        f[12..16].copy_from_slice(&(settings as u32).to_le_bytes());
        let mut s = vec![0u32; 60];
        s[1] = 2025;
        s[X_PITCH] = 100_000;
        s[Y_PITCH] = 200_000;
        s[Z_PER_DIGIT] = 1000;
        f.extend(s.iter().flat_map(|v| v.to_le_bytes()));
        let h0 = f.len();
        f[36..40].copy_from_slice(&(h0 as u32).to_le_bytes());
        for v in [2u32, 1, 32, 0, 8] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        f.resize(h0 + 796, 0);
        f.extend_from_slice(&3u32.to_le_bytes());
        f.extend_from_slice(&5u32.to_le_bytes());
        let c = f.len();
        f[16..20].copy_from_slice(&(c as u32).to_le_bytes());
        for v in [1u32, 1, 24, 0, 3] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        f.extend_from_slice(&[0, 0, 255]);
        let d = read(&f).unwrap();
        assert_eq!(d.pages.len(), 2);
        let (z, _) = d.pages[0].read_plane(&f, 0).unwrap();
        assert!((z[0] - 3e-9).abs() < 1e-15 && (z[1] - 5e-9).abs() < 1e-15);
        assert!((d.pages[0].y.as_ref().unwrap().step - 2e-7).abs() < 1e-18);
        assert!(d.pages[1].rgb);
        let (r, _) = d.pages[1].read_plane(&f, 0).unwrap();
        assert_eq!(r, [1.0]);
        assert!(read(&f[..100]).is_err());
    }
}
