//! Pixel helpers: box downscaling, sample conversion and the colour composite of channels.

use crate::{Channel, Rgba};
use rayon::prelude::*;
use specular_tiff as tf;

/// One channel's values.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    pub width: u32,
    pub height: u32,
    /// Row-major, NaN where undefined.
    pub values: Vec<f32>,
    pub truncated: bool,
}

/// Average `f`×`f` blocks of RGBA8 (partial blocks at the right and bottom edges average what
/// they hold).
pub fn box_reduce_rgba(src: &Rgba, f: u32) -> Rgba {
    if f <= 1 {
        return src.clone();
    }
    let (w, h) = (src.width.div_ceil(f), src.height.div_ceil(f));
    let sw = src.width as usize;
    let mut out = vec![0u8; w as usize * h as usize * 4];
    out.par_chunks_mut((w as usize * 4).max(1))
        .enumerate()
        .for_each(|(y, row)| {
            let y0 = y * f as usize;
            let y1 = (y0 + f as usize).min(src.height as usize);
            for x in 0..w as usize {
                let x0 = x * f as usize;
                let x1 = (x0 + f as usize).min(sw);
                let mut acc = [0u32; 4];
                for yy in y0..y1 {
                    let r = &src.data[(yy * sw + x0) * 4..(yy * sw + x1) * 4];
                    for p in r.chunks_exact(4) {
                        for c in 0..4 {
                            acc[c] += u32::from(p[c]);
                        }
                    }
                }
                let n = ((y1 - y0) * (x1 - x0)).max(1) as u32;
                for c in 0..4 {
                    row[x * 4 + c] = ((acc[c] + n / 2) / n) as u8;
                }
            }
        });
    Rgba {
        width: w,
        height: h,
        data: out,
        truncated: src.truncated,
    }
}

/// Average `f`×`f` blocks of values (NaN ignored).
pub(crate) fn box_reduce_plane(src: &Plane, f: u32) -> Plane {
    if f <= 1 {
        return src.clone();
    }
    let (w, h) = (src.width.div_ceil(f), src.height.div_ceil(f));
    let sw = src.width as usize;
    let mut out = vec![0f32; w as usize * h as usize];
    out.par_chunks_mut((w as usize).max(1))
        .enumerate()
        .for_each(|(y, row)| {
            let y0 = y * f as usize;
            let y1 = (y0 + f as usize).min(src.height as usize);
            for (x, o) in row.iter_mut().enumerate() {
                let x0 = x * f as usize;
                let x1 = (x0 + f as usize).min(sw);
                let (mut acc, mut n) = (0f64, 0u32);
                for yy in y0..y1 {
                    for &v in &src.values[yy * sw + x0..yy * sw + x1] {
                        if !v.is_nan() {
                            acc += f64::from(v);
                            n += 1;
                        }
                    }
                }
                *o = if n == 0 {
                    f32::NAN
                } else {
                    (acc / f64::from(n)) as f32
                };
            }
        });
    Plane {
        width: w,
        height: h,
        values: out,
        truncated: src.truncated,
    }
}

/// A TIFF decode as RGBA8 (grey expanded; wide formats rounded).
pub(crate) fn tiff_rgba(d: tf::Decoded) -> Rgba {
    let n = d.width as usize * d.height as usize;
    let data = match d.format {
        tf::PixelFormat::Rgba8 => d.data,
        tf::PixelFormat::Gray8 => {
            let mut v = Vec::with_capacity(n * 4);
            for &g in &d.data[..n.min(d.data.len())] {
                v.extend_from_slice(&[g, g, g, 255]);
            }
            v
        }
        _ => {
            let p = tiff_plane_raw(&d, 0);
            let mut v = Vec::with_capacity(n * 4);
            for &x in &p {
                let g = (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                v.extend_from_slice(&[g, g, g, 255]);
            }
            v
        }
    };
    Rgba {
        width: d.width,
        height: d.height,
        data,
        truncated: d.truncated,
    }
}

/// Sample `c` of a TIFF decode as values: 8 and 16-bit integers as stored, half floats as
/// values. For [`tiff_rgba`]'s fallback the 16-bit integers are normalised to 0..1.
fn tiff_plane_raw(d: &tf::Decoded, norm: u8) -> Vec<f32> {
    let n = d.width as usize * d.height as usize;
    let scale16 = if norm == 0 { 1.0 / 65535.0 } else { 1.0 };
    match d.format {
        tf::PixelFormat::Gray8 => d.data.iter().take(n).map(|&v| f32::from(v)).collect(),
        tf::PixelFormat::GrayF32 => d
            .data
            .chunks_exact(4)
            .take(n)
            .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
            .collect(),
        tf::PixelFormat::Rgba8 => d
            .data
            .chunks_exact(4)
            .take(n)
            .map(|p| f32::from(p[1]))
            .collect(),
        tf::PixelFormat::Gray16 => d
            .data
            .chunks_exact(2)
            .take(n)
            .map(|b| f32::from(u16::from_ne_bytes([b[0], b[1]])) * scale16)
            .collect(),
        tf::PixelFormat::GrayAlpha16 => d
            .data
            .chunks_exact(4)
            .take(n)
            .map(|b| f32::from(u16::from_ne_bytes([b[0], b[1]])) * scale16)
            .collect(),
        tf::PixelFormat::Rgba16 => d
            .data
            .chunks_exact(8)
            .take(n)
            .map(|b| f32::from(u16::from_ne_bytes([b[2], b[3]])) * scale16)
            .collect(),
        tf::PixelFormat::Rgba16F => d
            .data
            .chunks_exact(8)
            .take(n)
            .map(|b| half_to_f32(u16::from_ne_bytes([b[0], b[1]])))
            .collect(),
    }
}

/// The first sample of a TIFF decode as values in the file's units. `signed_bits`: the
/// stored sample is a signed integer of that many bits, which the decoder offsets to unsigned.
pub(crate) fn tiff_plane(d: tf::Decoded, signed_bits: Option<u16>) -> Plane {
    let mut values = tiff_plane_raw(&d, 1);
    if let Some(b @ 1..=16) = signed_bits {
        let off = f32::from(1u16 << (b - 1));
        for v in &mut values {
            *v -= off;
        }
    }
    Plane {
        width: d.width,
        height: d.height,
        values,
        truncated: d.truncated,
    }
}

pub(crate) fn half_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = (h >> 10) & 0x1f;
    let m = f32::from(h & 0x3ff);
    s * match e {
        0 => m * 2f32.powi(-24),
        31 => {
            if m == 0.0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => (1.0 + m / 1024.0) * 2f32.powi(i32::from(e) - 15),
    }
}

/// A display window from values: the 0.1 and 99.9 percentiles (min and max when the
/// histogram is degenerate).
pub(crate) fn auto_window(values: &[f32]) -> (f64, f64) {
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for &v in values {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    if !lo.is_finite() {
        return (0.0, 1.0);
    }
    if hi <= lo {
        return (f64::from(lo), f64::from(lo) + 1.0);
    }
    const BINS: usize = 4096;
    let mut hist = vec![0u32; BINS];
    let k = (BINS - 1) as f32 / (hi - lo);
    let mut n = 0u64;
    for &v in values {
        if v.is_finite() {
            // A subnormal span makes `k` infinite: the top value lands past the last bin.
            hist[(((v - lo) * k) as usize).min(BINS - 1)] += 1;
            n += 1;
        }
    }
    let at = |q: f64| {
        let target = (n as f64 * q) as u64;
        let mut acc = 0u64;
        for (i, &c) in hist.iter().enumerate() {
            acc += u64::from(c);
            if acc > target {
                return f64::from(lo) + i as f64 / f64::from(k);
            }
        }
        f64::from(hi)
    };
    let (a, b) = (at(0.001), at(0.999) + 1.0 / f64::from(k));
    if b > a {
        (a, b)
    } else {
        (f64::from(lo), f64::from(hi))
    }
}

/// Default colours of `n` channels (fluorescence conventions): one grey; two magenta and
/// green; then red, green, blue, and further ones in cyan, magenta, yellow, white.
pub(crate) fn default_colour(n: usize, k: usize) -> [u8; 3] {
    if n == 1 {
        return [255; 3];
    }
    if n == 2 {
        return [[255, 0, 255], [0, 255, 0]][k.min(1)];
    }
    const C: [[u8; 3]; 7] = [
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [0, 255, 255],
        [255, 0, 255],
        [255, 255, 0],
        [255, 255, 255],
    ];
    C[k % C.len()]
}

/// Composite channel planes (same size) additively in their colours through their windows.
pub(crate) fn composite(planes: &[Plane], channels: &[Channel], windows: &[(f64, f64)]) -> Rgba {
    let (w, h) = planes.first().map_or((0, 0), |p| (p.width, p.height));
    let n = w as usize * h as usize;
    let k = planes.len();
    let looks: Vec<([f32; 3], f32, f32)> = (0..k)
        .map(|i| {
            let c = channels
                .get(i)
                .and_then(|c| c.colour)
                .unwrap_or_else(|| default_colour(k, i));
            let (lo, hi) = windows.get(i).copied().unwrap_or((0.0, 1.0));
            let span = (hi - lo).max(1e-12);
            (
                [
                    f32::from(c[0]) / 255.0,
                    f32::from(c[1]) / 255.0,
                    f32::from(c[2]) / 255.0,
                ],
                lo as f32,
                (1.0 / span) as f32,
            )
        })
        .collect();
    let mut out = vec![0u8; n * 4];
    out.par_chunks_mut(4 * (w as usize).max(1))
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w as usize {
                let i = y * w as usize + x;
                let mut acc = [0f32; 3];
                for (p, (col, lo, inv)) in planes.iter().zip(&looks) {
                    let v = p.values.get(i).copied().unwrap_or(f32::NAN);
                    if v.is_nan() {
                        continue;
                    }
                    let t = ((v - lo) * inv).clamp(0.0, 1.0);
                    for c in 0..3 {
                        acc[c] += t * col[c];
                    }
                }
                for c in 0..3 {
                    row[x * 4 + c] = (srgb_encode(acc[c].min(1.0)) * 255.0 + 0.5) as u8;
                }
                row[x * 4 + 3] = 255;
            }
        });
    Rgba {
        width: w,
        height: h,
        data: out,
        truncated: planes.iter().any(|p| p.truncated),
    }
}

/// Channel intensities are display values already (a window maps them to 0..1 brightness);
/// they are kept as such, like a fluorescence viewer's LUT.
fn srgb_encode(v: f32) -> f32 {
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_reduce_averages_partial_blocks() {
        let src = Rgba {
            width: 3,
            height: 1,
            data: vec![0, 0, 0, 255, 100, 100, 100, 255, 50, 50, 50, 255],
            truncated: false,
        };
        let r = box_reduce_rgba(&src, 2);
        assert_eq!((r.width, r.height), (2, 1));
        assert_eq!(&r.data[..4], &[50, 50, 50, 255]);
        assert_eq!(&r.data[4..], &[50, 50, 50, 255]);
    }

    #[test]
    fn window_and_composite() {
        let values: Vec<f32> = (0..1000).map(|v| v as f32).collect();
        let (lo, hi) = auto_window(&values);
        assert!(lo < 3.0 && hi > 996.0, "{lo} {hi}");
        let p = Plane {
            width: 2,
            height: 1,
            values: vec![0.0, 10.0],
            truncated: false,
        };
        let ch = Channel {
            colour: Some([255, 0, 0]),
            ..Default::default()
        };
        let r = composite(&[p], &[ch], &[(0.0, 10.0)]);
        assert_eq!(r.data, vec![0, 0, 0, 255, 255, 0, 0, 255]);
        assert_eq!(half_to_f32(0x3c00), 1.0);
    }
}
