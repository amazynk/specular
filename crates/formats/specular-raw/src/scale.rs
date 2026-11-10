//! Non-square pixels (DNG `DefaultScale`): the developed picture resampled to square pixels.
//!
//! DNG gives the horizontal and vertical factors that make the stored pixels square. Only
//! their ratio is used: the axis whose factor is larger is stretched by the ratio and the
//! other kept, so no stored detail is thrown away (a file saying 0.5 × 1 shows the same
//! shape as one saying 1 × 2). The resampling is a separable triangle filter, centre-aligned
//! (linear interpolation when stretching, an area-weighted average when shrinking).

use rayon::prelude::*;

use crate::RgbImage;

/// Largest stretch accepted; anything beyond is taken as a damaged tag.
const MAX_RATIO: f64 = 16.0;

/// The stretch to square pixels from a `DefaultScale` value (at least 1 on each axis), or
/// `None` for square pixels or an unusable value.
pub(crate) fn default_scale(v: &[f64]) -> Option<[f32; 2]> {
    let [h, v] = v[..] else {
        return None;
    };
    if !(h.is_finite() && v.is_finite() && h > 0.0 && v > 0.0) {
        return None;
    }
    let m = h.min(v);
    let (h, v) = (h / m, v / m);
    if (h - v).abs() < 1e-6 || h.max(v) > MAX_RATIO {
        return None;
    }
    Some([h as f32, v as f32])
}

/// A `w × h` crop's size with square pixels.
pub(crate) fn square_size(w: u32, h: u32, k: Option<[f32; 2]>) -> (u32, u32) {
    match k {
        Some([kx, ky]) => (stretch(w, kx), stretch(h, ky)),
        None => (w, h),
    }
}

fn stretch(n: u32, k: f32) -> u32 {
    ((f64::from(n) * f64::from(k)).round() as u32).max(1)
}

/// `img` stretched by `k` (from [`default_scale`]).
pub(crate) fn square_pixels(img: &RgbImage, k: [f32; 2]) -> RgbImage {
    let (w, h) = square_size(img.width, img.height, Some(k));
    resample_rgb(img, w, h)
}

/// For each output index: the first input index and the weights from there.
fn taps(n_in: usize, n_out: usize) -> Vec<(usize, Vec<f32>)> {
    let ratio = n_in as f64 / n_out as f64;
    let r = ratio.max(1.0);
    (0..n_out)
        .map(|o| {
            let c = (o as f64 + 0.5) * ratio - 0.5;
            let lo = ((c - r).floor() + 1.0).max(0.0) as usize;
            let hi = ((c + r).ceil() - 1.0).min(n_in as f64 - 1.0).max(lo as f64) as usize;
            let mut w: Vec<f32> = (lo..=hi)
                .map(|i| (1.0 - (i as f64 - c).abs() / r).max(0.0) as f32)
                .collect();
            let sum: f32 = w.iter().sum();
            if sum > 0.0 {
                w.iter_mut().for_each(|x| *x /= sum);
            } else {
                // c lies outside the image (one-pixel inputs): the nearest pixel.
                w.iter_mut().for_each(|x| *x = 0.0);
                w[0] = 1.0;
            }
            (lo, w)
        })
        .collect()
}

/// The 16-bit RGB image `img` resampled to `w × h` (separable triangle filter); the rest of
/// its fields kept. For a whole develop of a sensor with non-square pixels, to
/// [`crate::SensorInfo::square_size`] (or that size at a reduced scale).
pub fn resample_rgb(img: &RgbImage, w: u32, h: u32) -> RgbImage {
    let (iw, ih) = (img.width as usize, img.height as usize);
    let (ow, oh) = (w.max(1) as usize, h.max(1) as usize);
    let mut out = RgbImage {
        width: ow as u32,
        height: oh as u32,
        data: Vec::new(),
        orientation: img.orientation,
        matrix_source: img.matrix_source,
        wb_source: img.wb_source,
        truncated: img.truncated,
    };
    if iw == 0 || ih == 0 || img.data.len() < iw * ih * 3 {
        out.data = vec![0; ow * oh * 3];
        return out;
    }
    if (iw, ih) == (ow, oh) {
        out.data = img.data[..iw * ih * 3].to_vec();
        return out;
    }
    // Horizontal pass into f32 rows, then vertical into the output.
    let hx = taps(iw, ow);
    let mut mid = vec![0f32; ow * ih * 3];
    mid.par_chunks_mut(ow * 3)
        .zip(img.data.par_chunks(iw * 3))
        .for_each(|(dst, src)| {
            for (o, (lo, ws)) in hx.iter().enumerate() {
                let mut acc = [0f32; 3];
                for (j, &wt) in ws.iter().enumerate() {
                    let p = &src[(lo + j) * 3..(lo + j) * 3 + 3];
                    for c in 0..3 {
                        acc[c] += wt * f32::from(p[c]);
                    }
                }
                dst[o * 3..o * 3 + 3].copy_from_slice(&acc);
            }
        });
    let vy = taps(ih, oh);
    out.data = vec![0u16; ow * oh * 3];
    out.data
        .par_chunks_mut(ow * 3)
        .zip(vy.par_iter())
        .for_each(|(dst, (lo, ws))| {
            for (x, d) in dst.iter_mut().enumerate() {
                let mut acc = 0f32;
                for (j, &wt) in ws.iter().enumerate() {
                    acc += wt * mid[(lo + j) * ow * 3 + x];
                }
                *d = acc.round().clamp(0.0, 65535.0) as u16;
            }
        });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MatrixSource, WbSource};

    fn img(w: u32, h: u32, f: impl Fn(u32, u32) -> u16) -> RgbImage {
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                data.extend([f(x, y); 3]);
            }
        }
        RgbImage {
            width: w,
            height: h,
            data,
            orientation: 6,
            matrix_source: MatrixSource::Generic,
            wb_source: WbSource::AsShot,
            truncated: false,
        }
    }

    #[test]
    fn ratio_only_never_shrinks() {
        assert_eq!(default_scale(&[1.0, 1.0]), None);
        assert_eq!(default_scale(&[2.0, 2.0]), None);
        assert_eq!(default_scale(&[0.5, 1.0]), Some([1.0, 2.0]));
        assert_eq!(default_scale(&[2.0, 1.0]), Some([2.0, 1.0]));
        assert_eq!(default_scale(&[1.0, 0.0]), None);
        assert_eq!(default_scale(&[1.0, 100.0]), None);
        assert_eq!(default_scale(&[1.0]), None);
        assert_eq!(square_size(300, 200, Some([1.0, 2.0])), (300, 400));
        assert_eq!(square_size(300, 200, None), (300, 200));
    }

    #[test]
    fn stretch_keeps_flat_areas_and_ramps() {
        // A flat field stays flat; a horizontal ramp keeps its ends and stays monotonic.
        let flat = square_pixels(&img(10, 6, |_, _| 4000), [2.0, 1.0]);
        assert_eq!((flat.width, flat.height), (20, 6));
        assert!(flat.data.iter().all(|&v| v == 4000));
        assert_eq!(flat.orientation, 6);
        let ramp = square_pixels(&img(8, 4, |x, _| x as u16 * 1000), [2.0, 1.0]);
        let row: Vec<u16> = ramp.data[..16 * 3].iter().step_by(3).copied().collect();
        assert_eq!((row[0], row[15]), (0, 7000));
        assert!(row.windows(2).all(|p| p[0] <= p[1]), "{row:?}");
        // Output 7's centre falls a quarter of the way from stored pixel 3 to 4.
        assert_eq!(row[7], 3250);
        // Shrinking: a triangle twice as wide, symmetric about each output centre.
        let half = resample_rgb(&img(4, 2, |x, _| x as u16 * 100), 2, 2);
        assert_eq!(&half.data[..6], &[71, 71, 71, 229, 229, 229]);
    }
}
