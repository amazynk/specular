//! SPIDER images, volumes and stacks (`.spi`, `.spider`), the cryo-EM processing system's
//! format (Wadsworth Center), from its published header description.
//!
//! The header is 32-bit floats in the file's byte order (either), numbered from 1: 1 the
//! slices (NZ), 2 the rows (NY), 5 IFORM (1 a 2D image, 3 a volume; negative values Fourier
//! transforms), 6 whether 7–10 hold the maximum, minimum, mean and standard deviation, 12 the
//! columns (NX), 13 the header's records (LABREC), 21 the scale, 22 the header's bytes
//! (LABBYT = LABREC × LENBYT), 23 the record length (LENBYT), 24 ISTACK (> 0 for a stack),
//! 26 MAXIM (the images in a stack), 27 IMGNUM, 38 the pixel size in Å, and the creation
//! date (12 characters at byte 211), time (8 at 223) and title (160 at 231).
//!
//! The values (32-bit floats, rows top first) follow the header. A stack has an overall
//! header, then each image's own header and values. Fourier-transform files are refused
//! by name.

use crate::samples::{self, Order, Sample};
use crate::{Error, MAX_PLANE, Result};

/// A parsed file: the images (or volumes) of a stack, each with its slices.
#[derive(Clone, Debug, PartialEq)]
pub struct Spider {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    /// 1 for a single image or volume.
    pub images: usize,
    pub order: Order,
    pub volume: bool,
    pub stack: bool,
    /// (max, min, mean, standard deviation) when the header says they were computed.
    pub stats: Option<[f32; 4]>,
    pub scale: f32,
    /// Ångström per pixel, when set.
    pub pixel_size: Option<f32>,
    pub date: String,
    pub time: String,
    pub title: String,
    /// Where each image's values start.
    offsets: Vec<usize>,
}

fn word(h: &[u8], n: usize, order: Order) -> f32 {
    let o = (n - 1) * 4;
    let b = [h[o], h[o + 1], h[o + 2], h[o + 3]];
    match order {
        Order::Little => f32::from_le_bytes(b),
        Order::Big => f32::from_be_bytes(b),
    }
}

/// A whole number in a sensible range.
fn int(v: f32, max: f32) -> Option<usize> {
    (v.is_finite() && v.fract() == 0.0 && v >= 0.0 && v <= max).then_some(v as usize)
}

fn sane(h: &[u8], order: Order) -> bool {
    let w = |n| word(h, n, order);
    let iform = w(5);
    let ok_form = [1.0, 3.0, -11.0, -12.0, -21.0, -22.0].contains(&iform);
    let (labrec, labbyt, lenbyt) = (int(w(13), 1e6), int(w(22), 1e9), int(w(23), 1e8));
    ok_form
        && int(w(12), 1e6).is_some_and(|n| n > 0)
        && int(w(2), 1e6).is_some_and(|n| n > 0)
        && matches!((labrec, labbyt, lenbyt), (Some(r), Some(b), Some(l)) if r > 0 && r * l == b)
}

/// Whether `head` (at least 92 bytes) is a SPIDER header in either byte order.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 92 && (sane(head, Order::Big) || sane(head, Order::Little))
}

fn text(h: &[u8], at: usize, len: usize) -> String {
    h.get(at..at + len)
        .map(|b| {
            String::from_utf8_lossy(b)
                .trim_matches(|c: char| c == '\0' || c.is_whitespace())
                .to_owned()
        })
        .unwrap_or_default()
}

pub fn parse(file: &[u8]) -> Result<Spider> {
    if file.len() < 92 {
        return Err(Error::Truncated("SPIDER header"));
    }
    let order = if sane(file, Order::Big) {
        Order::Big
    } else if sane(file, Order::Little) {
        Order::Little
    } else {
        return Err(Error::Malformed("not a SPIDER header".into()));
    };
    let w = |n| word(file, n, order);
    let iform = w(5);
    if iform < 0.0 {
        return Err(Error::Unsupported(
            "SPIDER Fourier-transform files are not supported (images and volumes are)".into(),
        ));
    }
    let nx = int(w(12), 1e6).unwrap_or(1);
    let ny = int(w(2), 1e6).unwrap_or(1);
    let nz = int(w(1).abs(), 1e6).unwrap_or(1).max(1);
    let labbyt = int(w(22), 1e9).unwrap_or(0);
    if labbyt < 92 || labbyt > file.len() {
        return Err(Error::Truncated("SPIDER header"));
    }
    if nx.saturating_mul(ny) as u64 > MAX_PLANE {
        return Err(Error::TooLarge);
    }
    let istack = if file.len() >= 96 { w(24) } else { 0.0 };
    let stack = istack > 0.0;
    let data_bytes = nx * ny * nz * 4;
    let (images, offsets) = if stack {
        let maxim = int(if file.len() >= 104 { w(26) } else { 0.0 }, 1e7).unwrap_or(0);
        let each = labbyt + data_bytes;
        // Images actually present (a stack may be cut short).
        let fit = (file.len() - labbyt) / each.max(1);
        let n = if maxim > 0 {
            maxim.min(fit.max(1))
        } else {
            fit
        };
        if n == 0 {
            return Err(Error::Truncated("SPIDER stack has no images"));
        }
        (n, (0..n).map(|i| labbyt + i * each + labbyt).collect())
    } else {
        (1, vec![labbyt])
    };
    let stats = (w(6) == 1.0).then(|| [w(7), w(8), w(9), w(10)]);
    let px = if file.len() >= 152 { w(38) } else { 0.0 };
    Ok(Spider {
        nx,
        ny,
        nz,
        images,
        order,
        volume: iform == 3.0 && nz > 1,
        stack,
        stats,
        scale: if file.len() >= 84 { w(21) } else { 0.0 },
        pixel_size: (px.is_finite() && px > 0.0).then_some(px),
        date: text(file, 211, 12),
        time: text(file, 223, 8),
        title: text(file, 231, 160),
        offsets,
    })
}

impl Spider {
    /// Planes in the file: every slice of every image.
    pub fn planes(&self) -> usize {
        self.nz * self.images
    }

    /// Reads plane `p` (slice `p % nz` of image `p / nz`), `nx × ny` values top row first.
    /// Returns whether values were missing (they read as NaN).
    pub fn read_plane(&self, file: &[u8], p: usize, out: &mut [f32]) -> Result<bool> {
        if p >= self.planes() || out.len() != self.nx * self.ny {
            return Err(Error::Malformed(format!("plane {p} of {}", self.planes())));
        }
        let (img, z) = (p / self.nz, p % self.nz);
        let at = self.offsets[img].saturating_add(z * self.nx * self.ny * 4);
        let src = file.get(at.min(file.len())..).unwrap_or(&[]);
        Ok(samples::convert(src, Sample::F32, self.order, 4, out))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A header of `labbyt` bytes for an `nx × ny × nz` file.
    pub(crate) fn header(
        nx: usize,
        ny: usize,
        nz: usize,
        iform: f32,
        stack: usize,
        be: bool,
    ) -> Vec<u8> {
        let lenbyt = nx * 4;
        let labrec = 1024usize.div_ceil(lenbyt);
        let labbyt = labrec * lenbyt;
        let mut h = vec![0u8; labbyt];
        let mut put = |n: usize, v: f32| {
            let b = if be { v.to_be_bytes() } else { v.to_le_bytes() };
            h[(n - 1) * 4..n * 4].copy_from_slice(&b);
        };
        put(1, nz as f32);
        put(2, ny as f32);
        put(5, iform);
        put(12, nx as f32);
        put(13, labrec as f32);
        put(22, labbyt as f32);
        put(23, lenbyt as f32);
        put(24, if stack > 0 { 2.0 } else { 0.0 });
        put(26, stack as f32);
        put(38, 1.5);
        h
    }

    fn values(n: usize, base: f32, be: bool) -> Vec<u8> {
        (0..n)
            .flat_map(|i| {
                let v = base + i as f32;
                if be { v.to_be_bytes() } else { v.to_le_bytes() }
            })
            .collect()
    }

    #[test]
    fn images_volumes_and_stacks() {
        let mut f = header(3, 2, 1, 1.0, 0, true);
        f.extend(values(6, 0.0, true));
        assert!(sniff(&f));
        let s = parse(&f).unwrap();
        assert_eq!((s.nx, s.ny, s.planes(), s.order), (3, 2, 1, Order::Big));
        assert_eq!(s.pixel_size, Some(1.5));
        let mut out = vec![0f32; 6];
        assert!(!s.read_plane(&f, 0, &mut out).unwrap());
        assert_eq!(out, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        // A stack of two little-endian images.
        let mut st = header(2, 2, 1, 1.0, 2, false);
        for k in 0..2 {
            st.extend(header(2, 2, 1, 1.0, 0, false));
            st.extend(values(4, 10.0 * k as f32, false));
        }
        let s = parse(&st).unwrap();
        assert_eq!((s.images, s.planes(), s.stack), (2, 2, true));
        let mut out = vec![0f32; 4];
        s.read_plane(&st, 1, &mut out).unwrap();
        assert_eq!(out, [10.0, 11.0, 12.0, 13.0]);
        // A volume.
        let mut v = header(2, 1, 3, 3.0, 0, true);
        v.extend(values(6, 0.0, true));
        let s = parse(&v).unwrap();
        assert!(s.volume);
        let mut out = vec![0f32; 2];
        s.read_plane(&v, 2, &mut out).unwrap();
        assert_eq!(out, [4.0, 5.0]);
        let fourier = header(2, 2, 1, -11.0, 0, true);
        assert!(matches!(parse(&fourier), Err(Error::Unsupported(_))));
    }
}
