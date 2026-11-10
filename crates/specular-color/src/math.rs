//! Small colour-science helpers: 3×3 matrices, CIE XYZ ↔ L*a*b*, chromatic adaptation and
//! RGB primaries.

/// A tristimulus or device triple.
pub type Vec3 = [f32; 3];

/// The ICC profile connection space illuminant (D50), exactly as encoded in s15Fixed16.
pub const D50: Vec3 = [0.964_202_9, 1.0, 0.824_905_4];

/// CIE D65 as used by sRGB / BT.709 (x 0.3127, y 0.3290).
pub const D65_XY: [f64; 2] = [0.3127, 0.3290];

/// A row-major 3×3 matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3(pub [[f64; 3]; 3]);

impl Mat3 {
    /// The identity.
    pub const IDENTITY: Mat3 = Mat3([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    /// A matrix whose columns are `a`, `b`, `c`.
    pub fn from_cols(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Mat3 {
        Mat3([[a[0], b[0], c[0]], [a[1], b[1], c[1]], [a[2], b[2], c[2]]])
    }

    /// A diagonal matrix.
    pub fn diag(d: [f64; 3]) -> Mat3 {
        Mat3([[d[0], 0.0, 0.0], [0.0, d[1], 0.0], [0.0, 0.0, d[2]]])
    }

    /// `self · o`.
    pub fn mul(&self, o: &Mat3) -> Mat3 {
        let mut r = [[0.0; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| self.0[i][k] * o.0[k][j]).sum();
            }
        }
        Mat3(r)
    }

    /// `self · v` in double precision.
    pub fn apply64(&self, v: [f64; 3]) -> [f64; 3] {
        let m = &self.0;
        [
            m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
            m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
            m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
        ]
    }

    /// The inverse, or `None` when (nearly) singular.
    pub fn inverse(&self) -> Option<Mat3> {
        let m = &self.0;
        let c00 = m[1][1] * m[2][2] - m[1][2] * m[2][1];
        let c01 = m[1][2] * m[2][0] - m[1][0] * m[2][2];
        let c02 = m[1][0] * m[2][1] - m[1][1] * m[2][0];
        let det = m[0][0] * c00 + m[0][1] * c01 + m[0][2] * c02;
        if !det.is_finite() || det.abs() < 1e-12 {
            return None;
        }
        let d = 1.0 / det;
        Some(Mat3([
            [
                c00 * d,
                (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * d,
                (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * d,
            ],
            [
                c01 * d,
                (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * d,
                (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * d,
            ],
            [
                c02 * d,
                (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * d,
                (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * d,
            ],
        ]))
    }

    /// Single-precision copy, row-major.
    pub fn to_f32(&self) -> [f32; 9] {
        let m = &self.0;
        [
            m[0][0] as f32,
            m[0][1] as f32,
            m[0][2] as f32,
            m[1][0] as f32,
            m[1][1] as f32,
            m[1][2] as f32,
            m[2][0] as f32,
            m[2][1] as f32,
            m[2][2] as f32,
        ]
    }

    /// Largest absolute element difference.
    pub fn max_diff(&self, o: &Mat3) -> f64 {
        let mut d: f64 = 0.0;
        for i in 0..3 {
            for j in 0..3 {
                d = d.max((self.0[i][j] - o.0[i][j]).abs());
            }
        }
        d
    }
}

/// `m · v` for a row-major single-precision matrix.
#[inline]
pub fn mul3(m: &[f32; 9], v: Vec3) -> Vec3 {
    [
        m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
        m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
        m[6] * v[0] + m[7] * v[1] + m[8] * v[2],
    ]
}

/// XYZ of a chromaticity with Y = 1.
pub fn xy_to_xyz(xy: [f64; 2]) -> [f64; 3] {
    [xy[0] / xy[1], 1.0, (1.0 - xy[0] - xy[1]) / xy[1]]
}

const BRADFORD: Mat3 = Mat3([
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
]);

/// Bradford chromatic adaptation from white `src` to white `dst` (both XYZ).
pub fn bradford(src: [f64; 3], dst: [f64; 3]) -> Mat3 {
    let Some(inv) = BRADFORD.inverse() else {
        return Mat3::IDENTITY;
    };
    let s = BRADFORD.apply64(src);
    let d = BRADFORD.apply64(dst);
    if s.iter().any(|v| v.abs() < 1e-9) {
        return Mat3::IDENTITY;
    }
    inv.mul(&Mat3::diag([d[0] / s[0], d[1] / s[1], d[2] / s[2]]))
        .mul(&BRADFORD)
}

/// The D50 PCS white in double precision.
pub fn d50() -> [f64; 3] {
    [D50[0] as f64, D50[1] as f64, D50[2] as f64]
}

/// RGB → XYZ(D50) for the given primaries and white point (chromaticities), Bradford-adapted
/// to D50 the way ICC matrix/TRC profiles store their colorants.
pub fn rgb_to_pcs(r: [f64; 2], g: [f64; 2], b: [f64; 2], white: [f64; 2]) -> Mat3 {
    let p = Mat3::from_cols(xy_to_xyz(r), xy_to_xyz(g), xy_to_xyz(b));
    let w = xy_to_xyz(white);
    let s = p.inverse().map(|i| i.apply64(w)).unwrap_or([1.0; 3]);
    let m = p.mul(&Mat3::diag(s));
    bradford(w, d50()).mul(&m)
}

/// Linear BT.709 / sRGB (D65) → XYZ(D50): the working space Specular renders in.
pub fn bt709_to_pcs() -> Mat3 {
    rgb_to_pcs([0.64, 0.33], [0.30, 0.60], [0.15, 0.06], D65_XY)
}

/// L*a*b* (D50) → XYZ (D50, Y of white = 1).
#[inline]
pub fn lab_to_xyz(lab: Vec3) -> Vec3 {
    let fy = (lab[0] + 16.0) / 116.0;
    let fx = fy + lab[1] / 500.0;
    let fz = fy - lab[2] / 200.0;
    #[inline]
    fn finv(t: f32) -> f32 {
        const D: f32 = 6.0 / 29.0;
        if t > D {
            t * t * t
        } else {
            3.0 * D * D * (t - 4.0 / 29.0)
        }
    }
    [D50[0] * finv(fx), D50[1] * finv(fy), D50[2] * finv(fz)]
}

/// XYZ (D50) → L*a*b* (D50).
#[inline]
pub fn xyz_to_lab(xyz: Vec3) -> Vec3 {
    #[inline]
    fn f(t: f32) -> f32 {
        const E: f32 = 216.0 / 24389.0;
        if t > E {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    }
    let fx = f(xyz[0] / D50[0]);
    let fy = f(xyz[1] / D50[1]);
    let fz = f(xyz[2] / D50[2]);
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 colour difference between two L*a*b* colours.
pub fn delta_e2000(a: Vec3, b: Vec3) -> f32 {
    let (l1, a1, b1) = (a[0] as f64, a[1] as f64, a[2] as f64);
    let (l2, a2, b2) = (b[0] as f64, b[1] as f64, b[2] as f64);
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let cm = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cm.powi(7) / (cm.powi(7) + 25f64.powi(7))).sqrt());
    let a1p = a1 * (1.0 + g);
    let a2p = a2 * (1.0 + g);
    let c1p = (a1p * a1p + b1 * b1).sqrt();
    let c2p = (a2p * a2p + b2 * b2).sqrt();
    let hp = |b: f64, a: f64| {
        if b == 0.0 && a == 0.0 {
            0.0
        } else {
            let h = b.atan2(a).to_degrees();
            if h < 0.0 { h + 360.0 } else { h }
        }
    };
    let h1p = hp(b1, a1p);
    let h2p = hp(b2, a2p);
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let dhp = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else {
        h2p - h1p + 360.0
    };
    let dh = 2.0 * (c1p * c2p).sqrt() * (dhp.to_radians() / 2.0).sin();
    let lm = (l1 + l2) / 2.0;
    let cmp = (c1p + c2p) / 2.0;
    let hm = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (hm - 30.0).to_radians().cos()
        + 0.24 * (2.0 * hm).to_radians().cos()
        + 0.32 * (3.0 * hm + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hm - 63.0).to_radians().cos();
    let dtheta = 30.0 * (-((hm - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cmp.powi(7) / (cmp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lm - 50.0).powi(2) / (20.0 + (lm - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cmp;
    let sh = 1.0 + 0.015 * cmp * t;
    let rt = -(2.0 * dtheta).to_radians().sin() * rc;
    let v = (dl / sl).powi(2) + (dc / sc).powi(2) + (dh / sh).powi(2) + rt * (dc / sc) * (dh / sh);
    v.max(0.0).sqrt() as f32
}

/// The sRGB transfer function (encoded → linear), extended by symmetry below zero.
#[inline]
pub fn srgb_to_linear(v: f32) -> f32 {
    let a = v.abs();
    let l = if a <= 0.04045 {
        a / 12.92
    } else {
        ((a + 0.055) / 1.055).powf(2.4)
    };
    l.copysign(v)
}

/// The inverse sRGB transfer function (linear → encoded), extended by symmetry below zero.
#[inline]
pub fn linear_to_srgb(v: f32) -> f32 {
    let a = v.abs();
    let e = if a <= 0.003_130_8 {
        a * 12.92
    } else {
        1.055 * a.powf(1.0 / 2.4) - 0.055
    };
    e.copysign(v)
}

/// f32 → IEEE half bits (round to nearest even), for Rgba16Float uploads.
pub fn f16_bits(v: f32) -> u16 {
    let x = v.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let mut e = ((x >> 23) & 0xff) as i32;
    let mut m = x & 0x7f_ffff;
    if e == 0xff {
        return sign | 0x7c00 | if m != 0 { 0x200 } else { 0 };
    }
    e = e - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        m |= 0x80_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        let mut r = m >> shift;
        let rem = m & ((1 << shift) - 1);
        if rem > half || (rem == half && (r & 1) == 1) {
            r += 1;
        }
        return sign | r as u16;
    }
    let mut r = ((e as u32) << 10) | (m >> 13);
    let rem = m & 0x1fff;
    if rem > 0x1000 || (rem == 0x1000 && (r & 1) == 1) {
        r += 1;
    }
    sign | r as u16
}

/// IEEE half bits → f32.
pub fn f16_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    let v = match e {
        0 => m * 2f32.powi(-24),
        31 => {
            if m == 0.0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    };
    s * v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_round_trip() {
        for lab in [
            [50.0, 20.0, -30.0],
            [0.0, 0.0, 0.0],
            [100.0, 0.0, 0.0],
            [5.0, -3.0, 2.0],
        ] {
            let back = xyz_to_lab(lab_to_xyz(lab));
            for i in 0..3 {
                assert!((back[i] - lab[i]).abs() < 1e-3, "{lab:?} → {back:?}");
            }
        }
    }

    #[test]
    fn srgb_pcs_matrix_matches_icc_srgb() {
        // Colorants of the canonical sRGB ICC profile.
        let m = bt709_to_pcs();
        let want = Mat3([
            [0.4361, 0.3851, 0.1431],
            [0.2225, 0.7169, 0.0606],
            [0.0139, 0.0971, 0.7141],
        ]);
        assert!(m.max_diff(&want) < 6e-4, "{m:?}");
    }

    #[test]
    fn de2000_reference() {
        // Sharma et al. test data, pair 1.
        let d = delta_e2000([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485]);
        assert!((d - 2.0425).abs() < 1e-3, "{d}");
    }

    #[test]
    fn half_round_trip() {
        for v in [0.0, 1.0, -2.5, 0.000_1, 65504.0] {
            assert!((f16_to_f32(f16_bits(v)) - v).abs() <= v.abs() * 1e-3);
        }
    }
}
