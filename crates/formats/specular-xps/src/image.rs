//! Images of image brushes: decoded through the caller's hook, kept premultiplied with
//! box-filtered smaller levels for minification, and their resolution read from the headers
//! (an image brush's viewbox is in 1/96 inch of the image at its own resolution).

use std::sync::{Arc, Mutex};

use vello_cpu::PixelMetadata;
use vello_cpu::Pixmap;
use vello_cpu::peniko::ImageAlphaType;

/// Most pixels of one image.
pub const MAX_IMAGE_PIXELS: u64 = 1 << 28;
/// Smallest side a level is reduced to.
const MIN_LEVEL_SIDE: u32 = 4;

pub struct Img {
    pub width: u32,
    pub height: u32,
    /// Pixels per inch, horizontally and vertically.
    pub dpi: (f64, f64),
    /// Level 0 is full size; level k is 1/2^k, made on demand.
    levels: Mutex<Vec<Arc<Pixmap>>>,
}

impl std::fmt::Debug for Img {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Img")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

/// Halves an RGBA8 buffer (premultiplied or not: a box filter is linear), odd edges kept.
fn halve(data: &[u8], w: u32, h: u32) -> (Vec<u8>, u32, u32) {
    let (w2, h2) = (w.div_ceil(2), h.div_ceil(2));
    let mut out = vec![0u8; w2 as usize * h2 as usize * 4];
    let (w, h) = (w as usize, h as usize);
    for y in 0..h2 as usize {
        let y0 = y * 2;
        let y1 = (y0 + 1).min(h - 1);
        for x in 0..w2 as usize {
            let x0 = x * 2;
            let x1 = (x0 + 1).min(w - 1);
            for c in 0..4 {
                let s = u32::from(data[(y0 * w + x0) * 4 + c])
                    + u32::from(data[(y0 * w + x1) * 4 + c])
                    + u32::from(data[(y1 * w + x0) * 4 + c])
                    + u32::from(data[(y1 * w + x1) * 4 + c]);
                out[(y * w2 as usize + x) * 4 + c] = ((s + 2) / 4) as u8;
            }
        }
    }
    (out, w2, h2)
}

impl Img {
    /// From straight RGBA8 pixels; reduced until each side fits a pixmap.
    pub fn new(mut data: Vec<u8>, mut w: u32, mut h: u32, dpi: (f64, f64)) -> Option<Img> {
        if w == 0 || h == 0 || data.len() != w as usize * h as usize * 4 {
            return None;
        }
        let (full_w, full_h) = (w, h);
        while w > u32::from(u16::MAX) || h > u32::from(u16::MAX) {
            (data, w, h) = halve(&data, w, h);
        }
        let pix = Pixmap::from_parts(
            data,
            w as u16,
            h as u16,
            PixelMetadata::new(ImageAlphaType::Alpha, true),
        );
        Some(Img {
            width: full_w,
            height: full_h,
            dpi,
            levels: Mutex::new(vec![Arc::new(pix)]),
        })
    }

    /// The level for drawing at `scale` device pixels per image pixel (largest level that is
    /// at least half that), with its own scale relative to full size.
    pub fn level(&self, scale: f64) -> (Arc<Pixmap>, f64) {
        let mut want = 0usize;
        let mut s = scale;
        while s < 0.5 && want < 16 {
            s *= 2.0;
            want += 1;
        }
        let mut levels = self
            .levels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while levels.len() <= want {
            let Some(last) = levels.last().cloned() else {
                break;
            };
            if u32::from(last.width()) <= MIN_LEVEL_SIDE
                || u32::from(last.height()) <= MIN_LEVEL_SIDE
            {
                break;
            }
            let (d, w2, h2) = halve(
                last.data_as_u8_slice(),
                u32::from(last.width()),
                u32::from(last.height()),
            );
            levels.push(Arc::new(Pixmap::from_parts(
                d,
                w2 as u16,
                h2 as u16,
                PixelMetadata::new(ImageAlphaType::AlphaPremultiplied, true),
            )));
        }
        let k = want.min(levels.len() - 1);
        let p = levels[k].clone();
        let rel = f64::from(p.width()) / f64::from(self.width);
        (p, rel)
    }

    pub fn memory(&self) -> usize {
        self.levels
            .lock()
            .map(|l| l.iter().map(|p| p.data_as_u8_slice().len()).sum())
            .unwrap_or(0)
    }
}

fn be16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// The image's resolution from its headers (JFIF, PNG pHYs, TIFF/JPEG XR IFD); 96 dpi when it
/// does not say.
pub fn dpi(d: &[u8]) -> (f64, f64) {
    let found = if d.starts_with(&[0xFF, 0xD8]) {
        jpeg_dpi(d)
    } else if d.starts_with(b"\x89PNG\r\n\x1a\n") {
        png_dpi(d)
    } else if d.starts_with(b"II*\0") || d.starts_with(b"MM\0*") || d.starts_with(b"II\xBC") {
        tiff_dpi(d)
    } else {
        None
    };
    match found {
        Some((x, y)) if x.is_finite() && y.is_finite() && x >= 1.0 && y >= 1.0 => (x, y),
        _ => (96.0, 96.0),
    }
}

fn jpeg_dpi(d: &[u8]) -> Option<(f64, f64)> {
    let mut p = 2;
    while p + 4 <= d.len() {
        if d[p] != 0xFF {
            return None;
        }
        let m = d[p + 1];
        if m == 0xD8 || (0xD0..=0xD7).contains(&m) || m == 0x01 {
            p += 2;
            continue;
        }
        if m == 0xDA || m == 0xD9 {
            return None;
        }
        let len = usize::from(be16(d, p + 2)?);
        if m == 0xE0 && d.get(p + 4..p + 9) == Some(&b"JFIF\0"[..]) {
            let units = *d.get(p + 11)?;
            let x = f64::from(be16(d, p + 12)?);
            let y = f64::from(be16(d, p + 14)?);
            return match units {
                1 => Some((x, y)),
                2 => Some((x * 2.54, y * 2.54)),
                _ => None,
            };
        }
        p += 2 + len;
    }
    None
}

fn png_dpi(d: &[u8]) -> Option<(f64, f64)> {
    let mut p = 8;
    while p + 8 <= d.len() {
        let len = be32(d, p)? as usize;
        let kind = d.get(p + 4..p + 8)?;
        if kind == b"pHYs" {
            let x = f64::from(be32(d, p + 8)?);
            let y = f64::from(be32(d, p + 12)?);
            return (*d.get(p + 16)? == 1).then_some((x * 0.0254, y * 0.0254));
        }
        if kind == b"IDAT" || kind == b"IEND" {
            return None;
        }
        p = p.checked_add(12 + len)?;
    }
    None
}

fn tiff_dpi(d: &[u8]) -> Option<(f64, f64)> {
    let le = d[0] == b'I';
    let r16 = |at: usize| -> Option<u16> {
        let b: [u8; 2] = d.get(at..at + 2)?.try_into().ok()?;
        Some(if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    };
    let r32 = |at: usize| -> Option<u32> {
        let b: [u8; 4] = d.get(at..at + 4)?.try_into().ok()?;
        Some(if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    };
    let ifd = r32(4)? as usize;
    let n = usize::from(r16(ifd)?).min(1000);
    let (mut x, mut y, mut unit) = (None, None, 2u16);
    let rational = |e: usize| -> Option<f64> {
        let off = r32(e + 8)? as usize;
        let num = f64::from(r32(off)?);
        let den = f64::from(r32(off + 4)?);
        (den > 0.0).then_some(num / den)
    };
    for i in 0..n {
        let e = ifd + 2 + i * 12;
        match r16(e)? {
            282 => x = rational(e),
            283 => y = rational(e),
            296 => unit = r16(e + 8)?,
            // JPEG XR: WIDTH_RESOLUTION / HEIGHT_RESOLUTION as floats (dpi).
            0xBC82 => x = Some(f64::from(f32::from_bits(r32(e + 8)?))),
            0xBC83 => y = Some(f64::from(f32::from_bits(r32(e + 8)?))),
            _ => {}
        }
    }
    let k = match unit {
        3 => 2.54,
        1 => return None,
        _ => 1.0,
    };
    let x = x? * k;
    Some((x, y.map_or(x, |y| y * k)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolutions() {
        // JFIF at 300 dpi.
        let mut j = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 16];
        j.extend_from_slice(b"JFIF\0\x01\x02\x01");
        j.extend_from_slice(&[1, 0x2C, 1, 0x2C, 0, 0]);
        j.extend_from_slice(&[0xFF, 0xDA]);
        assert_eq!(dpi(&j), (300.0, 300.0));
        // PNG with pHYs 11811 px/m (300 dpi).
        let mut p = b"\x89PNG\r\n\x1a\n".to_vec();
        p.extend_from_slice(&9u32.to_be_bytes());
        p.extend_from_slice(b"pHYs");
        p.extend_from_slice(&11811u32.to_be_bytes());
        p.extend_from_slice(&11811u32.to_be_bytes());
        p.push(1);
        let (x, _) = dpi(&p);
        assert!((x - 300.0).abs() < 0.1);
        assert_eq!(dpi(b"nothing"), (96.0, 96.0));
    }

    #[test]
    fn levels() {
        let img = Img::new(vec![255; 8 * 6 * 4], 8, 6, (96.0, 96.0)).unwrap();
        let (p, rel) = img.level(1.0);
        assert_eq!((p.width(), rel), (8, 1.0));
        let (p, rel) = img.level(0.2);
        assert_eq!(p.width(), 4);
        assert_eq!(rel, 0.5);
    }
}
