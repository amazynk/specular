//! Colour maths of the develop ops, in f64 on the CPU: the
//! working space (linear Rec.2020, D65), chromatic adaptation (CAT16 and Bradford), gamut
//! compression and the output spaces. The ops turn these into a few f32 uniforms, so a
//! kernel never does matrix algebra per pixel beyond one 3×3 product.
//!
//! - **CAT16** (Li, Li, Wang, Zu, Luo, Cui, Melgosa, Brill, Pointer, "Comprehensive colour
//!   appearance model (CAM16)", 2017): von Kries scaling in the CAT16 cone space, complete
//!   adaptation. Checked against colour-science (`tests/cat16_oracle.rs`).
//! - **Bradford** (Lam 1985): the same scaling in the Bradford space (the linear form).
//! - **Gamut compression**: the ACES 1.3 Reference Gamut Compression's distance curve
//!   (the published parametric form: per-channel distance from the achromatic axis,
//!   compressed past a threshold by a power curve with a set limit).

pub mod oklab;

/// Row-major 3×3 matrix.
pub type M3 = [[f64; 3]; 3];

pub const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// CIE 1931 xy of D65 (as ITU-R BT.2020 and BT.709 state it).
pub const D65_XY: [f64; 2] = [0.3127, 0.3290];
/// CIE 1931 xy of D50 (ICC).
pub const D50_XY: [f64; 2] = [0.3457, 0.3585];

/// Rec.2020 primaries (R, G, B), xy.
pub const REC2020_PRIMARIES: [[f64; 2]; 3] = [[0.708, 0.292], [0.170, 0.797], [0.131, 0.046]];
/// Rec.709 / sRGB primaries.
pub const REC709_PRIMARIES: [[f64; 2]; 3] = [[0.640, 0.330], [0.300, 0.600], [0.150, 0.060]];
/// Display P3 primaries (D65).
pub const P3_PRIMARIES: [[f64; 2]; 3] = [[0.680, 0.320], [0.265, 0.690], [0.150, 0.060]];

/// The CAT16 cone-response matrix (XYZ → LMS).
pub const CAT16: M3 = [
    [0.401288, 0.650173, -0.051461],
    [-0.250268, 1.204414, 0.045854],
    [-0.002079, 0.048952, 0.953127],
];

/// The Bradford cone-response matrix (XYZ → RGB "sharpened" responses).
pub const BRADFORD: M3 = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];

pub fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

pub fn mulv(a: &M3, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2])
}

pub fn det(m: &M3) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// The inverse, or `None` when singular.
pub fn inv(m: &M3) -> Option<M3> {
    let d = det(m);
    if !d.is_finite() || d.abs() < 1e-300 {
        return None;
    }
    let c =
        |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    Some([
        [c(1, 2, 1, 2) / d, -c(0, 2, 1, 2) / d, c(0, 1, 1, 2) / d],
        [-c(1, 2, 0, 2) / d, c(0, 2, 0, 2) / d, -c(0, 1, 0, 2) / d],
        [c(1, 2, 0, 1) / d, -c(0, 2, 0, 1) / d, c(0, 1, 0, 1) / d],
    ])
}

/// XYZ (Y = 1) of a chromaticity.
pub fn xy_to_xyz(xy: [f64; 2]) -> [f64; 3] {
    let y = xy[1].max(1e-12);
    [xy[0] / y, 1.0, (1.0 - xy[0] - xy[1]) / y]
}

/// The chromaticity of XYZ (D65 for black).
pub fn xyz_to_xy(v: [f64; 3]) -> [f64; 2] {
    let s = v[0] + v[1] + v[2];
    if s.abs() < 1e-300 || !s.is_finite() {
        return D65_XY;
    }
    [v[0] / s, v[1] / s]
}

/// RGB → XYZ of an RGB space given by its primaries and white (the normalised primary
/// matrix, SMPTE RP 177).
pub fn rgb_to_xyz(primaries: &[[f64; 2]; 3], white: [f64; 2]) -> M3 {
    let p: M3 = std::array::from_fn(|i| {
        let c: [f64; 3] = std::array::from_fn(|k| xy_to_xyz(primaries[k])[i] * primaries[k][1]);
        c
    });
    let s = mulv(&inv(&p).expect("primaries span a space"), xy_to_xyz(white));
    std::array::from_fn(|i| std::array::from_fn(|j| p[i][j] * s[j]))
}

/// Linear Rec.2020 (D65) → XYZ.
pub fn rec2020_to_xyz() -> M3 {
    rgb_to_xyz(&REC2020_PRIMARIES, D65_XY)
}

/// XYZ → linear Rec.2020 (D65).
pub fn xyz_to_rec2020() -> M3 {
    inv(&rec2020_to_xyz()).expect("Rec.2020 is invertible")
}

/// Linear Rec.2020 → linear Rec.709 (both D65).
pub fn rec2020_to_rec709() -> M3 {
    mul(
        &inv(&rgb_to_xyz(&REC709_PRIMARIES, D65_XY)).expect("Rec.709 is invertible"),
        &rec2020_to_xyz(),
    )
}

/// Linear Rec.709 → linear Rec.2020 (both D65).
pub fn rec709_to_rec2020() -> M3 {
    inv(&rec2020_to_rec709()).expect("invertible")
}

/// The adaptation transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cat {
    Cat16,
    Bradford,
}

impl Cat {
    pub fn matrix(self) -> &'static M3 {
        match self {
            Cat::Cat16 => &CAT16,
            Cat::Bradford => &BRADFORD,
        }
    }
}

/// The von Kries adaptation matrix in XYZ: a colour seen under `src_white` (XYZ) → the
/// corresponding colour under `dst_white`, complete adaptation (D = 1).
pub fn adaptation_xyz(cat: Cat, src_white: [f64; 3], dst_white: [f64; 3]) -> M3 {
    let m = cat.matrix();
    let mi = inv(m).expect("cone matrices are invertible");
    let s = mulv(m, src_white);
    let d = mulv(m, dst_white);
    let g: M3 =
        std::array::from_fn(|i| std::array::from_fn(|j| if i == j { d[i] / s[i] } else { 0.0 }));
    mul(&mi, &mul(&g, m))
}

/// [`adaptation_xyz`] between two chromaticities (Y = 1), as a matrix on linear Rec.2020.
pub fn adaptation_rec2020(cat: Cat, src_xy: [f64; 2], dst_xy: [f64; 2]) -> M3 {
    let a = adaptation_xyz(cat, xy_to_xyz(src_xy), xy_to_xyz(dst_xy));
    mul(&xyz_to_rec2020(), &mul(&a, &rec2020_to_xyz()))
}

/// A usable chromaticity: finite and inside the spectral locus's bounding triangle.
pub fn valid_xy(xy: [f64; 2]) -> bool {
    xy[0].is_finite() && xy[1].is_finite() && xy[0] > 0.0 && xy[1] > 0.01 && xy[0] + xy[1] < 1.0
}

/// Gamut compression of one pixel (linear RGB of any space), after the ACES 1.3 Reference
/// Gamut Compression: each channel's distance from the achromatic axis
/// `d = (max − c) / max` is left alone below `threshold` and compressed above it so that
/// distance `limit` lands on 1 (the gamut boundary); `strength` blends from none (0) to
/// full (1). Achromatic and in-gamut colours are unchanged.
pub fn compress_gamut(
    rgb: [f64; 3],
    threshold: f64,
    limit: [f64; 3],
    power: f64,
    strength: f64,
) -> [f64; 3] {
    let ach = rgb[0].max(rgb[1]).max(rgb[2]);
    if ach <= 0.0 || strength <= 0.0 {
        return rgb;
    }
    std::array::from_fn(|i| {
        let d = (ach - rgb[i]) / ach;
        let cd = compress_distance(d, threshold, limit[i], power);
        let d2 = d + (cd - d) * strength.min(1.0);
        ach - d2 * ach
    })
}

/// The distance curve of [`compress_gamut`] (exposed for the WGSL twin's tests).
pub fn compress_distance(d: f64, thr: f64, lim: f64, pwr: f64) -> f64 {
    if d <= thr || lim <= thr {
        return d;
    }
    // Scale so that `lim` maps to 1.
    let s = (lim - thr) / (((1.0 - thr) / (lim - thr)).powf(-pwr) - 1.0).powf(1.0 / pwr);
    let x = (d - thr) / s;
    thr + s * x / (1.0 + x.powf(pwr)).powf(1.0 / pwr)
}

/// Default distance limits per channel (cyan, magenta, yellow directions): how far out of
/// gamut a colour may be and still land on the boundary. Values past the limit stay out
/// and are clipped later.
pub const GAMUT_LIMITS: [f64; 3] = [1.2, 1.3, 1.3];
/// Power of the compression curve (ACES uses 1.2).
pub const GAMUT_POWER: f64 = 1.2;

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &M3, b: &M3, tol: f64) -> bool {
        (0..3).all(|i| (0..3).all(|j| (a[i][j] - b[i][j]).abs() <= tol))
    }

    #[test]
    fn rec2020_matches_the_published_matrix() {
        // ITU-R BT.2087 / BT.2020 normalised primary matrix (4 decimals as published).
        let m = rec2020_to_xyz();
        let want = [
            [0.6370, 0.1446, 0.1689],
            [0.2627, 0.6780, 0.0593],
            [0.0000, 0.0281, 1.0610],
        ];
        assert!(close(&m, &want, 6e-5), "{m:?}");
        // White maps to D65 with Y = 1.
        let w = mulv(&m, [1.0; 3]);
        assert!((w[1] - 1.0).abs() < 1e-12);
        let xy = xyz_to_xy(w);
        assert!((xy[0] - D65_XY[0]).abs() < 1e-12 && (xy[1] - D65_XY[1]).abs() < 1e-12);
    }

    #[test]
    fn adaptation_maps_white_to_white_and_is_identity_for_the_same_white() {
        for cat in [Cat::Cat16, Cat::Bradford] {
            let a = adaptation_xyz(cat, xy_to_xyz([0.4476, 0.4074]), xy_to_xyz(D65_XY));
            let w = mulv(&a, xy_to_xyz([0.4476, 0.4074]));
            let d = xy_to_xyz(D65_XY);
            assert!((0..3).all(|i| (w[i] - d[i]).abs() < 1e-12));
            assert!(close(
                &adaptation_rec2020(cat, D50_XY, D50_XY),
                &IDENTITY,
                1e-12
            ));
            // Rec.2020 white under D65 stays white.
            let m = adaptation_rec2020(cat, D65_XY, D65_XY);
            assert!(close(&m, &IDENTITY, 1e-12));
        }
    }

    #[test]
    fn inverse_is_an_inverse() {
        let m = rec2020_to_xyz();
        assert!(close(&mul(&m, &inv(&m).unwrap()), &IDENTITY, 1e-12));
        assert!(inv(&[[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [0.0, 0.0, 1.0]]).is_none());
    }

    #[test]
    fn gamut_compression_keeps_in_gamut_colours_and_brings_the_limit_to_the_boundary() {
        let inside = [0.5, 0.45, 0.3];
        assert_eq!(
            compress_gamut(inside, 0.8, GAMUT_LIMITS, GAMUT_POWER, 1.0),
            inside
        );
        // A colour at distance `limit` in red lands on distance 1 (red = 0).
        let lim = GAMUT_LIMITS[0];
        let c = [1.0 - lim, 1.0, 0.9];
        let out = compress_gamut(c, 0.8, GAMUT_LIMITS, GAMUT_POWER, 1.0);
        assert!(out[0].abs() < 1e-9, "{out:?}");
        assert_eq!(out[1], 1.0);
        // Monotone in distance.
        let mut last = -1.0;
        for i in 0..200 {
            let d = i as f64 / 100.0;
            let v = compress_distance(d, 0.8, 1.3, 1.2);
            assert!(v >= last);
            last = v;
        }
        // Strength 0 is the identity.
        assert_eq!(compress_gamut(c, 0.8, GAMUT_LIMITS, GAMUT_POWER, 0.0), c);
    }
}
