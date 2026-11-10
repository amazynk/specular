//! Colour-difference helpers for the integration tests: CIELAB from linear sRGB, CIEDE2000
//! and its percentiles.

#![allow(dead_code)]

/// sRGB-coded 0..1 → linear.
pub fn srgb_decode(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear sRGB (BT.709, D65) → CIELAB (D65).
pub fn lab_linear_srgb(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb;
    let x = 0.4123908 * r + 0.3575843 * g + 0.1804808 * b;
    let y = 0.2126390 * r + 0.7151687 * g + 0.0721923 * b;
    let z = 0.0193308 * r + 0.1191948 * g + 0.9505322 * b;
    xyz_to_lab([x, y, z])
}

/// CIE XYZ (D65, Y of white 1) → CIELAB (D65).
pub fn xyz_to_lab(xyz: [f64; 3]) -> [f64; 3] {
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(xyz[0] / 0.95047), f(xyz[1]), f(xyz[2] / 1.08883));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 (Sharma, Wu and Dalal 2005).
pub fn de2000(a: [f64; 3], b: [f64; 3]) -> f64 {
    use std::f64::consts::PI;
    let (l1, a1, b1) = (a[0], a[1], a[2]);
    let (l2, a2, b2) = (b[0], b[1], b[2]);
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let cb = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cb.powi(7) / (cb.powi(7) + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let c1p = (a1p * a1p + b1 * b1).sqrt();
    let c2p = (a2p * a2p + b2 * b2).sqrt();
    let hue = |b: f64, a: f64| {
        if b == 0.0 && a == 0.0 {
            0.0
        } else {
            let h = b.atan2(a);
            if h < 0.0 { h + 2.0 * PI } else { h }
        }
    };
    let (h1p, h2p) = (hue(b1, a1p), hue(b2, a2p));
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let mut dh = h2p - h1p;
    if c1p * c2p == 0.0 {
        dh = 0.0;
    } else if dh > PI {
        dh -= 2.0 * PI;
    } else if dh < -PI {
        dh += 2.0 * PI;
    }
    let dhh = 2.0 * (c1p * c2p).sqrt() * (dh / 2.0).sin();
    let lb = (l1 + l2) / 2.0;
    let cbp = (c1p + c2p) / 2.0;
    let hb = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= PI {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 2.0 * PI {
        (h1p + h2p + 2.0 * PI) / 2.0
    } else {
        (h1p + h2p - 2.0 * PI) / 2.0
    };
    let t = 1.0 - 0.17 * (hb - PI / 6.0).cos()
        + 0.24 * (2.0 * hb).cos()
        + 0.32 * (3.0 * hb + PI / 30.0).cos()
        - 0.20 * (4.0 * hb - 63.0 * PI / 180.0).cos();
    let dtheta = 30.0 * PI / 180.0 * (-((hb * 180.0 / PI - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cbp.powi(7) / (cbp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lb - 50.0).powi(2) / (20.0 + (lb - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cbp;
    let sh = 1.0 + 0.015 * cbp * t;
    let rt = -(2.0 * dtheta).sin() * rc;
    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dhh / sh).powi(2) + rt * (dc / sc) * (dhh / sh))
        .sqrt()
}

/// Percentiles of a set of differences.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

impl Stats {
    pub fn of(mut v: Vec<f64>) -> Stats {
        if v.is_empty() {
            return Stats::default();
        }
        v.sort_by(f64::total_cmp);
        let at = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
        Stats {
            p50: at(0.5),
            p95: at(0.95),
            p99: at(0.99),
            max: v[v.len() - 1],
        }
    }

    /// "p50 / p95 / p99 / max".
    pub fn cells(&self) -> String {
        format!(
            "{:.2} / {:.2} / {:.2} / {:.2}",
            self.p50, self.p95, self.p99, self.max
        )
    }

    /// "p95 / max".
    pub fn short(&self) -> String {
        format!("{:.2} / {:.2}", self.p95, self.max)
    }
}

/// The published CIEDE2000 test pairs (Sharma et al., table 1, a sample).
pub fn check_published_pairs() {
    for (a, b, want) in [
        ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
        ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
        (
            [60.2574, -34.0099, 36.2677],
            [60.4626, -34.1751, 39.4387],
            1.2644,
        ),
        ([2.0776, 0.0795, -1.135], [0.9033, -0.0636, -0.5514], 0.9082),
    ] {
        let d = de2000(a, b);
        assert!((d - want).abs() < 1e-4, "{a:?} {b:?}: {d} vs {want}");
    }
}
