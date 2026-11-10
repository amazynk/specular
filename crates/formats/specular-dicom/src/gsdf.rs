//! The Grayscale Standard Display Function (PS3.14 Annex B): luminance per just-noticeable-
//! difference index and back, and P-values spread over a display's luminance range in equal
//! JND steps. Maths only; the viewer does not calibrate displays with it yet.

/// The lowest and highest JND index (0.05 to about 3993 cd/m²).
pub const J_MIN: f64 = 1.0;
pub const J_MAX: f64 = 1023.0;

/// PS3.14 B.1, equation (1): log10 L as a rational polynomial in ln j.
const A: f64 = -1.301_187_7;
const B: f64 = -2.584_019_1e-2;
const C: f64 = 8.024_263_6e-2;
const D: f64 = -1.032_022_9e-1;
const E: f64 = 1.364_669_9e-1;
const F: f64 = 2.874_562e-2;
const G: f64 = -2.546_840_4e-2;
const H: f64 = -3.197_897_7e-3;
const K: f64 = 1.299_263_4e-4;
const M: f64 = 1.363_533_4e-3;

/// PS3.14 B.1, equation (2): j as a polynomial in log10 L.
const INV: [f64; 9] = [
    71.498_068,
    94.593_053,
    41.912_053,
    9.824_700_4,
    0.281_754_07,
    -1.187_845_5,
    -0.180_143_49,
    0.147_108_99,
    -0.017_046_845,
];

/// Luminance in cd/m² of JND index `j` (clamped to 1..=1023).
pub fn luminance(j: f64) -> f64 {
    let x = j.clamp(J_MIN, J_MAX).ln();
    let num = A + x * (C + x * (E + x * (G + x * M)));
    let den = 1.0 + x * (B + x * (D + x * (F + x * (H + x * K))));
    10f64.powf(num / den)
}

/// The JND index of luminance `l` in cd/m² (clamped to the function's range). Equation (2)
/// is a fit: it inverts [`luminance`] to within about 0.1 of an index.
pub fn jnd_index(l: f64) -> f64 {
    let y = l.clamp(luminance(J_MIN), luminance(J_MAX)).log10();
    INV.iter().rev().fold(0.0, |acc, c| acc * y + c)
}

/// The luminance a standardised display shows for P-value `p` (0..=1): equal JND steps from
/// `l_min` to `l_max` (the display's measured black and white, ambient included).
pub fn p_luminance(p: f64, l_min: f64, l_max: f64) -> f64 {
    let (j0, j1) = (jnd_index(l_min), jnd_index(l_max));
    luminance(j0 + p.clamp(0.0, 1.0) * (j1 - j0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_and_round_trip() {
        // PS3.14 Table B-1 starts at 0.0500 cd/m² for j = 1.
        assert!((luminance(1.0) - 0.05).abs() < 1e-4);
        let mut prev = 0.0;
        for j in 1..=1023 {
            let l = luminance(j as f64);
            assert!(l > prev);
            prev = l;
            assert!((jnd_index(l) - j as f64).abs() < 0.1, "j {j}");
        }
        assert!((p_luminance(0.0, 0.5, 300.0) - 0.5).abs() < 0.5 * 1e-3);
        assert!((p_luminance(1.0, 0.5, 300.0) - 300.0).abs() < 300.0 * 1e-3);
    }
}
