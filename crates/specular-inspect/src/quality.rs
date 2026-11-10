//! Compare's perceptual metrics (DESIGN.md §6.3): SSIM and CIEDE2000, the definitions the GPU
//! passes follow (`compare_ssim.wgsl`, `compare_de.wgsl`) and their CPU references.
//!
//! - **SSIM** (Wang, Bovik, Sheikh and Simoncelli 2004) on luma: an 11 × 11 Gaussian window
//!   of σ 1.5 ([`SSIM_RADIUS`], [`SSIM_SIGMA`]), K₁ 0.01, K₂ 0.03, a dynamic range of 1, the
//!   population (not sample) covariance, and the mean over every pixel whose window lies
//!   inside the region (no padding). That is scikit-image's `structural_similarity` with
//!   `gaussian_weights=True, sigma=1.5, use_sample_covariance=False, data_range=1`. Luma is
//!   the BT.709 weighting of the pixel in the [`MetricSpace`] (relative luminance in linear
//!   light, Y′ of the encoded values in sRGB), premultiplied (over black).
//! - **ΔE2000** (CIE 142-2001, Sharma's implementation notes): each pixel clipped to 0‥1
//!   (HDR highlights above SDR white count as white), premultiplied (over black), BT.709 to
//!   XYZ to CIELAB with the D65 white and 1.0 as Y = 100, whatever the metric space. The mean,
//!   the largest and the 95th percentile, the last from a histogram of [`DE_BINS`] bins
//!   spaced by the square root (fine near the visible threshold of about 1, coarse past 20).

use crate::diff::MetricSpace;

/// The SSIM window's radius (11 × 11) and its Gaussian's σ.
pub const SSIM_RADIUS: usize = 5;
pub const SSIM_SIGMA: f64 = 1.5;
/// (K₁ L)² and (K₂ L)² with L = 1.
pub const SSIM_C1: f64 = 0.01 * 0.01;
pub const SSIM_C2: f64 = 0.03 * 0.03;

/// The normalised 1D Gaussian weights of the window (the 2D window is their outer product).
pub fn ssim_weights() -> [f32; 2 * SSIM_RADIUS + 1] {
    let mut w = [0f64; 2 * SSIM_RADIUS + 1];
    for (i, v) in w.iter_mut().enumerate() {
        let x = i as f64 - SSIM_RADIUS as f64;
        *v = (-x * x / (2.0 * SSIM_SIGMA * SSIM_SIGMA)).exp();
    }
    let sum: f64 = w.iter().sum();
    w.map(|v| (v / sum) as f32)
}

/// The luma SSIM compares, of a premultiplied linear pixel in `space`.
pub fn ssim_luma(space: MetricSpace, p: [f32; 4]) -> f32 {
    let q = space.convert(p);
    crate::luma([q[0], q[1], q[2]])
}

/// SSIM of one window from its weighted moments: means, variances and covariance.
pub fn ssim_of(mx: f64, my: f64, vx: f64, vy: f64, cxy: f64) -> f64 {
    ((2.0 * mx * my + SSIM_C1) * (2.0 * cxy + SSIM_C2))
        / ((mx * mx + my * my + SSIM_C1) * (vx + vy + SSIM_C2))
}

/// The mean SSIM of luma planes `a` and `b` (`w` × `h`, rows top down), or None when the
/// region is smaller than the window.
pub fn ssim_reference(a: &[f32], b: &[f32], w: usize, h: usize) -> Option<f64> {
    let d = 2 * SSIM_RADIUS + 1;
    if w < d || h < d || a.len() < w * h || b.len() < w * h {
        return None;
    }
    let k = ssim_weights().map(f64::from);
    // Horizontal pass of the five moments, then vertical, over the valid centres only.
    let ow = w - d + 1;
    let mut hx = vec![[0f64; 5]; ow * h];
    for y in 0..h {
        for x in 0..ow {
            let mut m = [0f64; 5];
            for (i, &kw) in k.iter().enumerate() {
                let (p, q) = (f64::from(a[y * w + x + i]), f64::from(b[y * w + x + i]));
                m[0] += kw * p;
                m[1] += kw * q;
                m[2] += kw * p * p;
                m[3] += kw * q * q;
                m[4] += kw * p * q;
            }
            hx[y * ow + x] = m;
        }
    }
    let oh = h - d + 1;
    let mut sum = 0f64;
    for y in 0..oh {
        for x in 0..ow {
            let mut m = [0f64; 5];
            for (i, &kw) in k.iter().enumerate() {
                let r = hx[(y + i) * ow + x];
                for c in 0..5 {
                    m[c] += kw * r[c];
                }
            }
            let (mx, my) = (m[0], m[1]);
            sum += ssim_of(mx, my, m[2] - mx * mx, m[3] - my * my, m[4] - mx * my);
        }
    }
    Some(sum / (ow * oh) as f64)
}

/// The ΔE histogram's bins and the ΔE its last bin starts to hold everything above.
pub const DE_BINS: usize = 256;
pub const DE_RANGE: f32 = 128.0;

/// The bin of ΔE `de`: `⌊√(de / DE_RANGE) · DE_BINS⌋`, the last holding the rest (the
/// shader's `de_bin`).
pub fn de_bin(de: f32) -> usize {
    let t = (de.max(0.0) / DE_RANGE).sqrt() * DE_BINS as f32;
    (t as usize).min(DE_BINS - 1)
}

/// The ΔE at the lower edge of bin `i` (`DE_BINS` gives the range's end).
pub fn de_edge(i: usize) -> f32 {
    let t = i as f32 / DE_BINS as f32;
    t * t * DE_RANGE
}

/// CIELAB (D65 white, 1.0 = Y 100) of a linear BT.709 colour clipped to 0‥1.
pub fn lab_d65(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb.map(|c| f64::from(c.clamp(0.0, 1.0)));
    let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let z = 0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b;
    let f = |t: f64| {
        const E: f64 = 216.0 / 24389.0;
        if t > E {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.950_47), f(y), f(z / 1.088_83));
    [
        (116.0 * fy - 16.0) as f32,
        (500.0 * (fx - fy)) as f32,
        (200.0 * (fy - fz)) as f32,
    ]
}

/// ΔE2000 of two premultiplied linear pixels (over black).
pub fn pixel_delta_e(a: [f32; 4], b: [f32; 4]) -> f32 {
    specular_color::math::delta_e2000(lab_d65([a[0], a[1], a[2]]), lab_d65([b[0], b[1], b[2]]))
}

/// ΔE2000 statistics of a region.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeltaEStats {
    pub pixels: u64,
    pub mean: f64,
    /// The 95th percentile, read from the histogram (interpolated within its bin).
    pub p95: f64,
    pub max: f32,
}

impl DeltaEStats {
    /// From the sum, the largest value and the histogram over `pixels` pixels (what the GPU
    /// reads back).
    pub fn from_sums(sum: f64, max: f32, hist: &[u32], pixels: u64) -> DeltaEStats {
        DeltaEStats {
            pixels,
            mean: sum / pixels.max(1) as f64,
            p95: percentile(hist, 0.95).min(f64::from(max)),
            max,
        }
    }
}

/// The `q` quantile (0‥1) of the values binned in `hist` ([`de_bin`]): linear within the bin
/// it falls in.
pub fn percentile(hist: &[u32], q: f64) -> f64 {
    let n: u64 = hist.iter().map(|&c| u64::from(c)).sum();
    if n == 0 {
        return 0.0;
    }
    let want = q * n as f64;
    let mut below = 0u64;
    for (i, &c) in hist.iter().enumerate() {
        let c = u64::from(c);
        if c > 0 && (below + c) as f64 >= want {
            let f = ((want - below as f64) / c as f64).clamp(0.0, 1.0);
            let (lo, hi) = (f64::from(de_edge(i)), f64::from(de_edge(i + 1)));
            return lo + (hi - lo) * f;
        }
        below += c;
    }
    f64::from(de_edge(hist.len()))
}

/// The CPU reference of the ΔE statistics of premultiplied linear pixels `a` against `b`.
pub fn delta_e_reference(a: &[[f32; 4]], b: &[[f32; 4]]) -> DeltaEStats {
    let mut hist = vec![0u32; DE_BINS];
    let (mut sum, mut max, mut n) = (0f64, 0f32, 0u64);
    for (pa, pb) in a.iter().zip(b) {
        let de = pixel_delta_e(*pa, *pb);
        hist[de_bin(de)] += 1;
        sum += f64::from(de);
        max = max.max(de);
        n += 1;
    }
    DeltaEStats::from_sums(sum, max, &hist, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic test plane: smooth structure plus a fine pattern.
    fn plane(w: usize, h: usize) -> Vec<f32> {
        (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f32, (i / w) as f32);
                0.5 + 0.3 * (x * 0.37).sin() * (y * 0.23).cos() + 0.1 * ((x + 2.0 * y) % 7.0) / 7.0
            })
            .collect()
    }

    /// A 3 × 3 box blur with edges clamped.
    fn blur(p: &[f32], w: usize, h: usize) -> Vec<f32> {
        let mut out = vec![0f32; p.len()];
        for y in 0..h {
            for x in 0..w {
                let mut s = 0.0;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let xx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                        let yy = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                        s += p[yy * w + xx];
                    }
                }
                out[y * w + x] = s / 9.0;
            }
        }
        out
    }

    #[test]
    fn ssim_window_is_the_published_one() {
        let k = ssim_weights();
        assert_eq!(k.len(), 11);
        assert!((k.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        assert!((k[5] - 0.266_011).abs() < 1e-5, "{}", k[5]);
        assert_eq!(k[0], k[10]);
    }

    #[test]
    fn ssim_of_identical_images_is_one() {
        let (w, h) = (40, 30);
        let a = plane(w, h);
        let s = ssim_reference(&a, &a, w, h).expect("big enough");
        assert!((s - 1.0).abs() < 1e-12, "{s}");
        // Smaller than the window: no value.
        assert_eq!(ssim_reference(&a, &a, 10, 30), None);
    }

    /// Against scikit-image 0.25 (`structural_similarity(a, b, gaussian_weights=True,
    /// sigma=1.5, use_sample_covariance=False, data_range=1.0)`) on the same planes, as
    /// float64 (tools/quality/ssim_oracle.py prints them).
    #[test]
    fn ssim_matches_scikit_image() {
        let (w, h) = (40, 30);
        let a = plane(w, h);
        let b = blur(&a, w, h);
        let s = ssim_reference(&a, &b, w, h).expect("big enough");
        assert!((s - SKIMAGE_BLUR).abs() < 1e-6, "{s}");
        // A brightness shift lowers the luminance term only.
        let c: Vec<f32> = a.iter().map(|v| v * 0.8 + 0.05).collect();
        let s = ssim_reference(&a, &c, w, h).expect("big enough");
        assert!((s - SKIMAGE_SCALED).abs() < 1e-6, "{s}");
    }

    /// The oracle's values (see `ssim_matches_scikit_image`).
    const SKIMAGE_BLUR: f64 = 0.942_037_975_474_460_6;
    const SKIMAGE_SCALED: f64 = 0.971_097_016_381_353_7;

    #[test]
    fn delta_e_bins_cover_the_range() {
        assert_eq!(de_bin(0.0), 0);
        assert_eq!(de_bin(-1.0), 0);
        assert_eq!(de_bin(1e9), DE_BINS - 1);
        assert_eq!(de_edge(DE_BINS), DE_RANGE);
        for i in 0..DE_BINS {
            // A value in the middle of a bin lands in it.
            let mid = (de_edge(i) + de_edge(i + 1)) * 0.5;
            assert_eq!(de_bin(mid), i, "{i}");
        }
        // Fine near 1 (the visible threshold), under a tenth of a unit wide.
        let i = de_bin(1.0);
        assert!(de_edge(i + 1) - de_edge(i) < 0.1);
    }

    #[test]
    fn delta_e_follows_cielab() {
        // White is L* 100, black L* 0, neither has chroma.
        let w = lab_d65([1.0; 3]);
        assert!((w[0] - 100.0).abs() < 1e-3 && w[1].abs() < 1e-3 && w[2].abs() < 1e-3);
        assert_eq!(lab_d65([0.0; 3]), [0.0; 3]);
        // HDR above white is clipped to white.
        assert_eq!(lab_d65([4.0; 3]), w);
        // Mid grey (linear 0.18) is L* 49.5 or so.
        assert!((lab_d65([0.18; 3])[0] - 49.496).abs() < 0.01);
        // Black against white: ΔE2000 100.
        let de = pixel_delta_e([0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
        assert!((de - 100.0).abs() < 1e-3, "{de}");
        assert_eq!(
            pixel_delta_e([0.3, 0.2, 0.1, 1.0], [0.3, 0.2, 0.1, 1.0]),
            0.0
        );
    }

    #[test]
    fn delta_e_statistics() {
        // 100 pixels: 95 identical, 5 black against white.
        let a: Vec<[f32; 4]> = (0..100).map(|_| [0.0, 0.0, 0.0, 1.0]).collect();
        let mut b = a.clone();
        for p in b.iter_mut().take(5) {
            *p = [1.0, 1.0, 1.0, 1.0];
        }
        let s = delta_e_reference(&a, &b);
        assert_eq!(s.pixels, 100);
        assert!((s.mean - 5.0).abs() < 1e-3, "{}", s.mean);
        assert!((s.max - 100.0).abs() < 1e-3);
        // The 95th percentile is the last of the zeros: in the first bin.
        assert!(s.p95 <= f64::from(de_edge(1)), "{}", s.p95);
        // Uniform values: the percentile within a bin's width of the exact one.
        let a: Vec<[f32; 4]> = (0..1000).map(|_| [0.2, 0.2, 0.2, 1.0]).collect();
        let b: Vec<[f32; 4]> = (0..1000)
            .map(|i| {
                let v = 0.2 + 0.1 * i as f32 / 1000.0;
                [v, 0.2, 0.2, 1.0]
            })
            .collect();
        let s = delta_e_reference(&a, &b);
        let mut des: Vec<f32> = a
            .iter()
            .zip(&b)
            .map(|(p, q)| pixel_delta_e(*p, *q))
            .collect();
        des.sort_by(f32::total_cmp);
        let exact = f64::from(des[949]);
        let i = de_bin(des[949]);
        let width = f64::from(de_edge(i + 1) - de_edge(i));
        assert!((s.p95 - exact).abs() <= width, "{} vs {exact}", s.p95);
        assert_eq!(percentile(&[0; DE_BINS], 0.95), 0.0);
    }
}
