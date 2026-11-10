//! Blur and exposure from pixels alone (no model), for culling.
//!
//! - **Sharpness**: the 4-neighbour Laplacian of the luminance, its mean energy per 16×16
//!   block, and the 90th percentile over blocks (a sharp subject on a soft background still
//!   counts as sharp), mapped to 0..1 by `1 − e^(−e/K)`.
//! - **Exposure**: mean luminance (0..1, as encoded) and the fraction of pixels clipped
//!   (at or above 250) or crushed (at or below 5).
//!
//! Measured on a picture of about 512 px (a thumbnail), or on a face crop for eye focus.

use super::Image;

/// Laplacian energy giving a sharpness of 1 − 1/e.
const K: f32 = 60.0;
/// Block side for the percentile.
const BLOCK: usize = 16;

/// Exposure facts of a picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exposure {
    /// Mean luminance, 0..1.
    pub mean: f32,
    /// Fraction of pixels at or above 250.
    pub clipped: f32,
    /// Fraction of pixels at or below 5.
    pub crushed: f32,
}

fn luma(img: Image) -> Vec<f32> {
    img.rgba[..img.width * img.height * 4]
        .chunks_exact(4)
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
        .collect()
}

/// Sharpness of the luminance plane `y` (`w`×`h`), 0 (flat or blurred) to 1.
pub fn sharpness_of(y: &[f32], w: usize, h: usize) -> f32 {
    if w < 3 || h < 3 {
        return 0.0;
    }
    let bw = (w - 2).div_ceil(BLOCK);
    let bh = (h - 2).div_ceil(BLOCK);
    let mut energy = vec![0f32; bw * bh];
    let mut count = vec![0u32; bw * bh];
    for yy in 1..h - 1 {
        for xx in 1..w - 1 {
            let c = y[yy * w + xx];
            let l = y[yy * w + xx - 1]
                + y[yy * w + xx + 1]
                + y[(yy - 1) * w + xx]
                + y[(yy + 1) * w + xx]
                - 4.0 * c;
            let b = ((yy - 1) / BLOCK) * bw + (xx - 1) / BLOCK;
            energy[b] += l * l;
            count[b] += 1;
        }
    }
    let mut per: Vec<f32> = energy
        .iter()
        .zip(&count)
        .filter(|(_, c)| **c > 0)
        .map(|(&e, &c)| e / c as f32)
        .collect();
    if per.is_empty() {
        return 0.0;
    }
    per.sort_by(f32::total_cmp);
    let p90 = per[((per.len() - 1) as f32 * 0.9).round() as usize];
    1.0 - (-p90.sqrt() / K.sqrt()).exp()
}

/// Sharpness of a picture, 0..1.
pub fn sharpness(img: Image) -> f32 {
    sharpness_of(&luma(img), img.width, img.height)
}

/// Exposure facts of a picture.
pub fn exposure(img: Image) -> Exposure {
    let y = luma(img);
    let n = y.len().max(1) as f32;
    let mean = y.iter().sum::<f32>() / n / 255.0;
    let clipped = y.iter().filter(|&&v| v >= 249.5).count() as f32 / n;
    let crushed = y.iter().filter(|&&v| v <= 5.5).count() as f32 / n;
    Exposure {
        mean,
        clipped,
        crushed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(w: usize, h: usize, cell: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let on = ((x / cell) + (y / cell)).is_multiple_of(2);
                let c = if on { 220 } else { 30 };
                v.extend_from_slice(&[c, c, c, 255]);
            }
        }
        v
    }

    fn blur(px: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
        let mut out = px.to_vec();
        for y in 0..h {
            for x in 0..w {
                let mut a = 0u32;
                let mut n = 0u32;
                for yy in y.saturating_sub(r)..(y + r + 1).min(h) {
                    for xx in x.saturating_sub(r)..(x + r + 1).min(w) {
                        a += px[(yy * w + xx) * 4] as u32;
                        n += 1;
                    }
                }
                let v = (a / n) as u8;
                out[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[v, v, v]);
            }
        }
        out
    }

    #[test]
    fn blur_lowers_sharpness() {
        let (w, h) = (128, 96);
        let sharp = checker(w, h, 4);
        let soft = blur(&sharp, w, h, 3);
        let s1 = sharpness(Image::new(w, h, &sharp).unwrap());
        let s2 = sharpness(Image::new(w, h, &soft).unwrap());
        assert!(s1 > 0.9 && s2 < s1 * 0.6, "{s1} {s2}");
        let flat = vec![128u8; w * h * 4];
        assert_eq!(sharpness(Image::new(w, h, &flat).unwrap()), 0.0);
    }

    #[test]
    fn exposure_counts_clipped_and_crushed() {
        let mut px = vec![255u8; 4 * 4];
        px[..8].copy_from_slice(&[0, 0, 0, 255, 0, 0, 0, 255]);
        let e = exposure(Image::new(2, 2, &px).unwrap());
        assert_eq!(e.clipped, 0.5);
        assert_eq!(e.crushed, 0.5);
        assert!((e.mean - 0.5).abs() < 1e-3);
    }
}
