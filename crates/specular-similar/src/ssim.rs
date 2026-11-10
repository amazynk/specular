//! The verify step: structural similarity of two thumbnails, for candidate pairs only.

use crate::sig::Luma;

const C1: f32 = (0.01 * 255.0) * (0.01 * 255.0);
const C2: f32 = (0.03 * 255.0) * (0.03 * 255.0);
/// Window side and step (px at the compared size).
const WIN: usize = 8;
const STEP: usize = 4;

/// Mean SSIM of two lumas (−1…1; 1 = identical), compared at the smaller of the two sizes.
/// Shapes that differ by more than 6 % do not compare (0): they are not the same picture.
pub fn ssim(a: &Luma, b: &Luma) -> f32 {
    if a.w == 0 || a.h == 0 || b.w == 0 || b.h == 0 {
        return 0.0;
    }
    let ra = a.w as f32 / a.h as f32;
    let rb = b.w as f32 / b.h as f32;
    if (ra / rb).max(rb / ra) > 1.06 {
        return 0.0;
    }
    let (w, h) = (a.w.min(b.w), a.h.min(b.h));
    let a = a.resized(w, h);
    let b = b.resized(w, h);
    ssim_same_size(&a.px, &b.px, w as usize, h as usize)
}

fn ssim_same_size(a: &[f32], b: &[f32], w: usize, h: usize) -> f32 {
    if w < WIN || h < WIN {
        return if a == b { 1.0 } else { 0.0 };
    }
    let n = (WIN * WIN) as f32;
    let (mut total, mut count) = (0.0f64, 0u32);
    let mut y = 0;
    while y + WIN <= h {
        let mut x = 0;
        while x + WIN <= w {
            let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0f32, 0.0, 0.0, 0.0, 0.0);
            for yy in y..y + WIN {
                let ra = &a[yy * w + x..yy * w + x + WIN];
                let rb = &b[yy * w + x..yy * w + x + WIN];
                for (&p, &q) in ra.iter().zip(rb) {
                    sa += p;
                    sb += q;
                    saa += p * p;
                    sbb += q * q;
                    sab += p * q;
                }
            }
            let ma = sa / n;
            let mb = sb / n;
            let va = (saa / n - ma * ma).max(0.0);
            let vb = (sbb / n - mb * mb).max(0.0);
            let cov = sab / n - ma * mb;
            let s = ((2.0 * ma * mb + C1) * (2.0 * cov + C2))
                / ((ma * ma + mb * mb + C1) * (va + vb + C2));
            total += f64::from(s);
            count += 1;
            x += STEP;
        }
        y += STEP;
    }
    (total / f64::from(count.max(1))) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(w: u32, h: u32, f: impl Fn(u32, u32) -> f32) -> Luma {
        let mut px = Vec::new();
        for y in 0..h {
            for x in 0..w {
                px.push(f(x, y));
            }
        }
        Luma { w, h, px }
    }

    #[test]
    fn identical_is_one_unrelated_low() {
        let a = plane(64, 48, |x, y| ((x * 7 + y * 3) % 256) as f32);
        assert!((ssim(&a, &a) - 1.0).abs() < 1e-4);
        let b = plane(64, 48, |x, y| (((x * 13) ^ (y * 29)) % 256) as f32);
        assert!(ssim(&a, &b) < 0.5);
        let c = plane(48, 64, |x, y| (x + y) as f32);
        assert_eq!(ssim(&a, &c), 0.0);
    }
}
