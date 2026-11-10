//! Rigaku R-AXIS image plates (R-AXIS II/IV/IV++/VII, RAPID): a fixed big-endian header
//! starting `R-AXIS`, then 16-bit big-endian pixels filling the file's end. A pixel with its
//! top bit set stores `value / ratio` (the photomultiplier ratio in the header) in the low
//! 15 bits. RAPID plates are cylindrical (`IP shape`), so the flat-detector geometry is only
//! approximate there.

use crate::bytes::{f32_at, i32_at, str_at};
use crate::geometry::Geometry;
use crate::{Coding, Error, Frame, Image, Kind, Result, Sample, bad, finite, positive};

const HEADER: usize = 1024;

/// Whether the file starts with an R-AXIS header.
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(b"R-AXIS") || head.starts_with(b"RAXIS")
}

pub(crate) fn parse(data: &[u8]) -> Result<Image> {
    if data.len() < HEADER {
        return Err(Error::Truncated);
    }
    let int = |at| i32_at(data, at, true).unwrap_or(0);
    let real = |at| f64::from(f32_at(data, at, true).unwrap_or(f32::NAN));
    let w = u32::try_from(int(768)).map_err(|_| bad("X pixels"))?;
    let h = u32::try_from(int(772)).map_err(|_| bad("Y pixels"))?;
    let n = u64::from(w) * u64::from(h) * 2;
    let n = usize::try_from(n).map_err(|_| bad("image size"))?;
    if n > data.len() - HEADER.min(data.len()) {
        return Err(Error::Truncated);
    }
    let offset = data.len() - n;
    let ratio = real(800);
    let ratio = if ratio.is_finite() && ratio > 0.0 {
        ratio as f32
    } else {
        1.0
    };
    let mut g = Geometry {
        wavelength: positive(Some(real(292))),
        distance: positive(Some(real(344))),
        ..Geometry::default()
    };
    let (bx, by) = (real(540), real(544));
    if bx.is_finite() && by.is_finite() && (bx != 0.0 || by != 0.0) {
        g.beam = Some((bx, by));
    }
    let (px, py) = (real(776), real(780));
    if px > 0.0 && py > 0.0 {
        g.pixel = Some((px, py));
    }
    g.curved = int(448) == 1;
    g.phi = finite(Some(real(520)));
    let (start, stop) = (real(524), real(528));
    if start.is_finite() && stop.is_finite() && stop != start {
        g.osc_start = Some(start);
        g.osc_width = Some(stop - start);
        g.osc_axis = Some("φ".to_owned());
    }
    g.exposure = positive(Some(real(536) * 60.0));
    g.omega = finite(Some(real(548)));
    g.chi = finite(Some(real(552)));
    g.two_theta = finite(Some(real(556)));
    let mut header: Vec<(String, String)> = Vec::new();
    for (k, at, len) in [
        ("Device", 0, 10),
        ("Version", 10, 10),
        ("Crystal name", 20, 20),
        ("Crystal system", 40, 12),
        ("Space group", 84, 12),
        ("Memo", 100, 72),
        ("Date", 172, 12),
        ("Measured by", 184, 20),
        ("User", 268, 20),
        ("X-ray target", 288, 4),
        ("Host", 812, 10),
        ("IP type", 822, 10),
    ] {
        if let Some(v) = str_at(data, at, len) {
            header.push((k.to_owned(), v));
        }
    }
    for (k, at) in [
        ("Photomultiplier ratio", 800),
        ("Generator kV", 316),
        ("Generator mA", 320),
    ] {
        let v = real(at);
        if v.is_finite() && v != 0.0 {
            header.push((k.to_owned(), format!("{v}")));
        }
    }
    let detector = [str_at(data, 0, 10), str_at(data, 822, 10)]
        .into_iter()
        .flatten()
        .reduce(|a, b| format!("{a} ({b})"));
    Ok(Image {
        kind: Kind::Raxis,
        width: w,
        height: h,
        sample: Sample::U16,
        frames: vec![Frame {
            coding: Coding::Raxis { offset, ratio },
            header: Vec::new(),
        }],
        header,
        geometry: g,
        detector,
        date: str_at(data, 172, 12),
        mask_negative: false,
        dummy: None,
        title: str_at(data, 20, 20),
    })
}

/// `n` pixels at `offset`, the top bit scaled by `ratio`.
pub(crate) fn read(data: &[u8], offset: usize, n: usize, ratio: f32) -> Result<Vec<f32>> {
    let src = data
        .get(offset..offset.checked_add(n * 2).ok_or(Error::Truncated)?)
        .ok_or(Error::Truncated)?;
    Ok(src
        .chunks_exact(2)
        .map(|c| {
            let v = u16::from_be_bytes([c[0], c[1]]);
            if v & 0x8000 != 0 {
                // Whole counts, as the plate reader reports them.
                (f32::from(v & 0x7fff) * ratio).trunc()
            } else {
                f32::from(v)
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plate() {
        let mut d = vec![0u8; HEADER];
        d[..6].copy_from_slice(b"R-AXIS");
        let mut put = |at: usize, b: [u8; 4]| d[at..at + 4].copy_from_slice(&b);
        put(292, 0.70926f32.to_be_bytes());
        put(344, 127.26f32.to_be_bytes());
        put(540, 1.5f32.to_be_bytes());
        put(544, 0.5f32.to_be_bytes());
        put(768, 2i32.to_be_bytes());
        put(772, 2i32.to_be_bytes());
        put(776, 0.2f32.to_be_bytes());
        put(780, 0.2f32.to_be_bytes());
        put(800, 32f32.to_be_bytes());
        put(448, 1i32.to_be_bytes());
        for v in [1u16, 0x8000 | 100, 3, 0x7fff] {
            d.extend(v.to_be_bytes());
        }
        assert!(sniff(&d));
        let im = Image::parse(&d).unwrap();
        assert_eq!(im.kind, Kind::Raxis);
        assert_eq!(im.frame(&d, 0).unwrap(), [1.0, 3200.0, 3.0, 32767.0]);
        let g = &im.geometry;
        assert!(g.curved);
        assert_eq!(g.beam, Some((1.5, 0.5)));
        assert!((g.wavelength.unwrap() - 0.70926).abs() < 1e-6);
    }
}
