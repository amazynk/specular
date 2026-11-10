//! Sensofar `.plu` (PLu confocal / interferometric, little-endian, no magic number): a
//! 500-byte header (date text, comment, rows and columns, pixel pitch in micrometres,
//! measurement configuration), the rows and columns again, the heights as float32
//! micrometres (1000001 where nothing was measured), two more such markers and a 24-bit
//! RGB texture of the same size.
//!
//! Rows are top-down as SensoSCAN shows them. Pages: the height map, then the texture as
//! colour (red, green and blue planes in 0..=1).

use specular_em::dataset::{Dataset, Plane};
use specular_em::samples::Sample;

use crate::{Error, Result, alloc, area, f32le, height, meta, page, slice, text, u32le};

const ROWS: usize = 388;
const COLS: usize = 392;
const PITCH_X: usize = 404;
const PITCH_Y: usize = 408;
const DATA: usize = 508;
const INVALID: f32 = 1_000_001.0;

/// The header's shape: a printable date at the start, the size twice and a positive pitch.
pub fn probe(head: &[u8]) -> bool {
    let Some(date) = head.get(..24) else {
        return false;
    };
    let printable = date[..3].iter().all(u8::is_ascii_alphabetic)
        && date.iter().all(|&c| c == 0 || (0x20..0x7f).contains(&c));
    let size = |o| u32le(head, o).unwrap_or(0);
    let pitch = f32le(head, PITCH_X).unwrap_or(0.0);
    printable
        && size(ROWS) > 0
        && size(COLS) > 0
        && size(ROWS) == size(500)
        && size(COLS) == size(504)
        && pitch.is_finite()
        && pitch > 0.0
}

pub fn read(file: &[u8]) -> Result<Dataset> {
    if !probe(file) {
        return Err(Error::NotFormat("Sensofar PLu"));
    }
    let rows = u32le(file, ROWS).unwrap_or(0) as usize;
    let cols = u32le(file, COLS).unwrap_or(0) as usize;
    let n = area(cols, rows)?;
    let data = slice(file, DATA, n * 4, "heights")?;
    let mut v = alloc(n)?;
    v.extend(data.chunks_exact(4).map(|c| {
        let z = f32le(c, 0).unwrap_or(INVALID);
        if z == INVALID {
            f32::NAN
        } else {
            (f64::from(z) * 1e-6) as f32
        }
    }));
    let um = |o| f64::from(f32le(file, o).unwrap_or(0.0)) * 1e-6;
    let (dx, dy) = (um(PITCH_X), um(PITCH_Y));
    let date = text(file, 0, 128);
    let comment = text(file, 132, 256);
    let mut pg = page(cols, rows, dx, dy);
    pg.planes.push(height(v));
    pg.sample = Some(Sample::F32);
    pg.levelling = true;
    pg.instrument = Some("Sensofar".into());
    pg.date = (!date.is_empty()).then(|| date.clone());
    meta(&mut pg, "Comment", comment);
    let mut pages = vec![pg];

    // The texture follows two more invalid markers.
    let tex = DATA + n * 4 + 8;
    if let Ok(rgb) = slice(file, tex, n * 3, "texture") {
        let mut planes = [alloc(n)?, alloc(n)?, alloc(n)?];
        for px in rgb.chunks_exact(3) {
            for (c, p) in planes.iter_mut().enumerate() {
                p.push(f32::from(px[c]) / 255.0);
            }
        }
        let mut t = page(cols, rows, dx, dy);
        let [r, g, b] = planes;
        t.planes = vec![
            Plane::values("Red", "", r),
            Plane::values("Green", "", g),
            Plane::values("Blue", "", b),
        ];
        t.rgb = true;
        t.sample = Some(Sample::U8);
        t.title = Some("Texture".into());
        t.date = pages[0].date.clone();
        pages.push(t);
    }
    Ok(Dataset { pages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_and_texture() {
        let mut f = vec![0u8; DATA];
        f[..24].copy_from_slice(b"Mon Apr 19 11:40:13 2021");
        for o in [ROWS, 500] {
            f[o..o + 4].copy_from_slice(&1u32.to_le_bytes());
        }
        for o in [COLS, 504] {
            f[o..o + 4].copy_from_slice(&2u32.to_le_bytes());
        }
        f[PITCH_X..PITCH_X + 4].copy_from_slice(&0.5f32.to_le_bytes());
        f[PITCH_Y..PITCH_Y + 4].copy_from_slice(&0.5f32.to_le_bytes());
        for z in [2.0f32, INVALID, INVALID, INVALID] {
            f.extend_from_slice(&z.to_le_bytes());
        }
        f.extend_from_slice(&[255, 0, 0, 0, 255, 0]);
        assert!(probe(&f));
        let d = read(&f).unwrap();
        let (z, _) = d.pages[0].read_plane(&f, 0).unwrap();
        assert!((z[0] - 2e-6).abs() < 1e-12 && z[1].is_nan());
        assert!((d.pages[0].x.as_ref().unwrap().step - 5e-7).abs() < 1e-15);
        let (g, _) = d.pages[1].read_plane(&f, 1).unwrap();
        assert_eq!(g, [0.0, 1.0]);
        assert!(!probe(&f[..400]));
    }
}
