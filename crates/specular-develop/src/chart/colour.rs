//! Colour arithmetic of the chart solver, in f64: L*a*b* (D50), CIEDE2000 (Sharma, Wu and
//! Dalal 2005), HSV as the DNG tables use it, and the matrices of the DNG colour model as
//! `specular-raw` applies them (so the solver predicts what the front end renders).

pub type Lab = [f64; 3];
pub type V3 = [f64; 3];
pub type M3 = [[f64; 3]; 3];

pub const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Linear ProPhoto RGB → XYZ D50, as `specular-raw` uses it.
pub fn prophoto_to_xyz() -> M3 {
    specular_raw::dcp::prophoto_to_xyz_d50()
}

/// The D50 white the solver keeps: ProPhoto's white (so a white-preserving matrix maps
/// camera white to ProPhoto (1, 1, 1) exactly, as the DNG reader's row normalisation
/// expects).
pub fn d50() -> V3 {
    let m = prophoto_to_xyz();
    [0, 1, 2].map(|r| m[r].iter().sum())
}

/// XYZ D50 → linear Rec.709 (D65), Bradford, as `specular-raw` uses it.
pub fn xyz_to_rec709() -> M3 {
    specular_raw::dcp::xyz_d50_to_rec709()
}

/// Linear Rec.2020 (D65) → XYZ D50 (through Rec.709, Bradford).
pub fn rec2020_to_xyz() -> M3 {
    let to709 = inv(&rec709_to_rec2020()).unwrap_or(IDENTITY);
    mul(&inv(&xyz_to_rec709()).unwrap_or(IDENTITY), &to709)
}

/// Linear Rec.709 → linear Rec.2020 (both D65).
pub fn rec709_to_rec2020() -> M3 {
    crate::ops::raw::REC709_TO_REC2020.map(|r| r.map(f64::from))
}

pub fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

pub fn mulv(a: &M3, v: V3) -> V3 {
    std::array::from_fn(|i| a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2])
}

pub fn diag(v: V3) -> M3 {
    [[v[0], 0.0, 0.0], [0.0, v[1], 0.0], [0.0, 0.0, v[2]]]
}

pub fn inv(m: &M3) -> Option<M3> {
    let c = |a: usize, b: usize, c: usize, d: usize| m[a][b] * m[c][d] - m[a][d] * m[c][b];
    let det = m[0][0] * c(1, 1, 2, 2) - m[0][1] * c(1, 0, 2, 2) + m[0][2] * c(1, 0, 2, 1);
    if det.abs() < 1e-15 || !det.is_finite() {
        return None;
    }
    let r = [
        [c(1, 1, 2, 2), -c(0, 1, 2, 2), c(0, 1, 1, 2)],
        [-c(1, 0, 2, 2), c(0, 0, 2, 2), -c(0, 0, 1, 2)],
        [c(1, 0, 2, 1), -c(0, 0, 2, 1), c(0, 0, 1, 1)],
    ];
    Some(r.map(|row| row.map(|v| v / det)))
}

/// Rows scaled to sum to 1 (the DNG reader's normalisation).
pub fn normalize_rows(m: &mut M3) {
    for r in m.iter_mut() {
        let s: f64 = r.iter().sum();
        if s.abs() > 1e-12 {
            for v in r.iter_mut() {
                *v /= s;
            }
        }
    }
}

/// Bradford adaptation from white `src` to white `dst` (XYZ).
pub fn bradford(src: V3, dst: V3) -> M3 {
    const B: M3 = [
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    ];
    let bi = inv(&B).unwrap_or(IDENTITY);
    let s = mulv(&B, src);
    let d = mulv(&B, dst);
    mul(
        &bi,
        &mul(&diag([d[0] / s[0], d[1] / s[1], d[2] / s[2]]), &B),
    )
}

/// XYZ of chromaticity `xy` with Y = 1.
pub fn xy_to_xyz(xy: [f64; 2]) -> V3 {
    [xy[0] / xy[1], 1.0, (1.0 - xy[0] - xy[1]) / xy[1]]
}

pub const D65_XY: [f64; 2] = [0.3127, 0.3290];

fn f_lab(t: f64) -> f64 {
    const E: f64 = 216.0 / 24389.0;
    const K: f64 = 24389.0 / 27.0;
    if t > E {
        t.cbrt()
    } else {
        (K * t + 16.0) / 116.0
    }
}

fn f_inv(f: f64) -> f64 {
    const E: f64 = 216.0 / 24389.0;
    const K: f64 = 24389.0 / 27.0;
    let t = f * f * f;
    if t > E { t } else { (116.0 * f - 16.0) / K }
}

/// XYZ (D50, white Y = 1) → L*a*b* against white `w`.
pub fn xyz_to_lab_white(xyz: V3, w: V3) -> Lab {
    let f = [0, 1, 2].map(|i| f_lab(xyz[i] / w[i]));
    [
        116.0 * f[1] - 16.0,
        500.0 * (f[0] - f[1]),
        200.0 * (f[1] - f[2]),
    ]
}

/// XYZ (D50, white Y = 1) → L*a*b* (D50).
pub fn xyz_to_lab(xyz: V3) -> Lab {
    xyz_to_lab_white(xyz, d50())
}

/// L*a*b* (D50) → XYZ (D50, white Y = 1).
pub fn lab_to_xyz(lab: Lab) -> V3 {
    let w = d50();
    let fy = (lab[0] + 16.0) / 116.0;
    let fx = fy + lab[1] / 500.0;
    let fz = fy - lab[2] / 200.0;
    [w[0] * f_inv(fx), w[1] * f_inv(fy), w[2] * f_inv(fz)]
}

/// L*a*b* relative to D65 → L*a*b* (D50) by Bradford.
pub fn d65_lab_to_d50(lab: Lab) -> Lab {
    let w65 = xy_to_xyz(D65_XY);
    let fy = (lab[0] + 16.0) / 116.0;
    let fx = fy + lab[1] / 500.0;
    let fz = fy - lab[2] / 200.0;
    let xyz65 = [w65[0] * f_inv(fx), w65[1] * f_inv(fy), w65[2] * f_inv(fz)];
    xyz_to_lab(mulv(&bradford(w65, d50()), xyz65))
}

/// CIEDE2000 colour difference (kL = kC = kH = 1).
pub fn delta_e2000(a: Lab, b: Lab) -> f64 {
    let p = delta_e2000_parts(a, b);
    (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
}

/// CIEDE2000 as three signed parts whose squares sum to ΔE²: ΔL'/SL, ΔC'/SC + RT·ΔH'/(2 SH)
/// and √(1 − RT²/4)·ΔH'/SH (the rotation term completed into a square). Smooth through 0,
/// unlike ΔE itself: the residuals a least-squares fit wants.
pub fn delta_e2000_parts(a: Lab, b: Lab) -> [f64; 3] {
    use std::f64::consts::PI;
    let deg = |r: f64| r * 180.0 / PI;
    let rad = |d: f64| d * PI / 180.0;
    let c1 = a[1].hypot(a[2]);
    let c2 = b[1].hypot(b[2]);
    let cm = (c1 + c2) / 2.0;
    let c7 = cm.powi(7);
    let g = 0.5 * (1.0 - (c7 / (c7 + 25f64.powi(7))).sqrt());
    let a1 = (1.0 + g) * a[1];
    let a2 = (1.0 + g) * b[1];
    let c1p = a1.hypot(a[2]);
    let c2p = a2.hypot(b[2]);
    let hp = |b: f64, a: f64| {
        if b == 0.0 && a == 0.0 {
            0.0
        } else {
            let h = deg(b.atan2(a));
            if h < 0.0 { h + 360.0 } else { h }
        }
    };
    let h1 = hp(a[2], a1);
    let h2 = hp(b[2], a2);
    let dl = b[0] - a[0];
    let dc = c2p - c1p;
    let dh = if c1p * c2p == 0.0 {
        0.0
    } else if (h2 - h1).abs() <= 180.0 {
        h2 - h1
    } else if h2 - h1 > 180.0 {
        h2 - h1 - 360.0
    } else {
        h2 - h1 + 360.0
    };
    let dhh = 2.0 * (c1p * c2p).sqrt() * rad(dh / 2.0).sin();
    let lm = (a[0] + b[0]) / 2.0;
    let cmp = (c1p + c2p) / 2.0;
    let hm = if c1p * c2p == 0.0 {
        h1 + h2
    } else if (h1 - h2).abs() <= 180.0 {
        (h1 + h2) / 2.0
    } else if h1 + h2 < 360.0 {
        (h1 + h2 + 360.0) / 2.0
    } else {
        (h1 + h2 - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * rad(hm - 30.0).cos()
        + 0.24 * rad(2.0 * hm).cos()
        + 0.32 * rad(3.0 * hm + 6.0).cos()
        - 0.20 * rad(4.0 * hm - 63.0).cos();
    let dtheta = 30.0 * (-((hm - 275.0) / 25.0).powi(2)).exp();
    let c7 = cmp.powi(7);
    let rc = 2.0 * (c7 / (c7 + 25f64.powi(7))).sqrt();
    let l50 = (lm - 50.0).powi(2);
    let sl = 1.0 + 0.015 * l50 / (20.0 + l50).sqrt();
    let sc = 1.0 + 0.045 * cmp;
    let sh = 1.0 + 0.015 * cmp * t;
    let rt = -(rad(2.0 * dtheta)).sin() * rc;
    let (x, y, z) = (dl / sl, dc / sc, dhh / sh);
    [
        x,
        y + rt * z / 2.0,
        (1.0 - rt * rt / 4.0).max(0.0).sqrt() * z,
    ]
}

/// RGB → (hue in [0, 6), saturation, value), as the DNG tables index it.
pub fn rgb_to_hsv([r, g, b]: V3) -> V3 {
    let v = r.max(g).max(b);
    let gap = v - r.min(g).min(b);
    if gap <= 0.0 || v <= 0.0 {
        return [0.0, 0.0, v];
    }
    let h = if r == v {
        let h = (g - b) / gap;
        if h < 0.0 { h + 6.0 } else { h }
    } else if g == v {
        2.0 + (b - r) / gap
    } else {
        4.0 + (r - g) / gap
    };
    [h, gap / v, v]
}

pub fn hsv_to_rgb([h, s, v]: V3) -> V3 {
    if s <= 0.0 {
        return [v; 3];
    }
    let h = if !(0.0..6.0).contains(&h) { 0.0 } else { h };
    let i = (h.floor() as u32).min(5);
    let f = h - f64::from(i);
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sharma, Wu and Dalal's published test pairs (a selection covering the hue wrap,
    /// the mean-hue branches and the rotation term).
    #[test]
    fn ciede2000_matches_the_published_pairs() {
        let pairs: [(Lab, Lab, f64); 8] = [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, 3.1571, -77.2803], [50.0, 0.0, -82.7485], 2.8615),
            ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
            ([50.0, 2.49, -0.001], [50.0, -2.49, 0.0011], 7.2195),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
            (
                [60.2574, -34.0099, 36.2677],
                [60.4626, -34.1751, 39.4387],
                1.2644,
            ),
            (
                [22.7233, 20.0904, -46.694],
                [23.0331, 14.973, -42.5619],
                2.0373,
            ),
            ([2.0776, 0.0795, -1.135], [0.9033, -0.0636, -0.5514], 0.9082),
        ];
        for (a, b, want) in pairs {
            let got = delta_e2000(a, b);
            assert!((got - want).abs() < 1e-4, "{a:?} {b:?}: {got} vs {want}");
            assert!((delta_e2000(b, a) - want).abs() < 1e-4);
        }
    }

    #[test]
    fn lab_round_trips_and_hsv_inverts() {
        for lab in [[50.0, 20.0, -30.0], [5.0, 1.0, 1.0], [96.0, -0.5, 2.0]] {
            let back = xyz_to_lab(lab_to_xyz(lab));
            for i in 0..3 {
                assert!((back[i] - lab[i]).abs() < 1e-9);
            }
        }
        for rgb in [[0.2, 0.5, 0.1], [0.9, 0.1, 0.4], [0.3, 0.3, 0.3]] {
            let back = hsv_to_rgb(rgb_to_hsv(rgb));
            for i in 0..3 {
                assert!((back[i] - rgb[i]).abs() < 1e-12);
            }
        }
        let w = d50();
        assert!((w[0] - 0.9642).abs() < 1e-3 && (w[2] - 0.8252).abs() < 1e-3);
    }
}
