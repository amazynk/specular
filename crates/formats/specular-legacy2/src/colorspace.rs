//! The colour spaces ImageMagick writes MIFF pixels in, back to sRGB.
//!
//! Each stored channel is normalised 0..1 (with ImageMagick's offsets: chroma and a/b around
//! 0.5, hue as a fraction of a turn). Conversions follow the published definitions (BT.601 and
//! BT.709 YCbCr, YUV, YIQ, YDbDr, Ohta's I1I2I3, HSL/HSV/HSI/HWB/HCL, CIE XYZ/xyY/Lab/Luv/LCH
//! with a D65 white, CAT02 LMS, Kodak printing-density log, Display P3, Oklab) and are
//! checked against ImageMagick's own reading of every one.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Cs {
    Cmy,
    /// Y and two colour differences, a linear transform of sRGB-coded values (forward
    /// matrix rows).
    Matrix(&'static [[f64; 3]; 3], bool),
    Hsl,
    Hsv,
    Hsi,
    Hwb,
    Hcl,
    Hclp,
    Xyz,
    XyY,
    Lms,
    Lab,
    Lch,
    Luv,
    LchUv,
    Log,
    ScRgb,
    DisplayP3,
    Oklab,
}

// The matrices are constants: equality is never NaN.
impl Eq for Cs {}

const Y601: [f64; 3] = [0.298839, 0.586811, 0.114350];
static YCBCR601: [[f64; 3]; 3] = [
    Y601,
    [-0.1687367, -0.331264, 0.5],
    [0.5, -0.418688, -0.081312],
];
static YCBCR709: [[f64; 3]; 3] = [
    [0.212656, 0.715158, 0.072186],
    [-0.114572, -0.385428, 0.5],
    [0.5, -0.454153, -0.045847],
];
static YUV: [[f64; 3]; 3] = [
    Y601,
    [-0.14740, -0.28950, 0.43690],
    [0.61500, -0.51500, -0.10000],
];
static YIQ: [[f64; 3]; 3] = [
    Y601,
    [0.595716, -0.274453, -0.321263],
    [0.211456, -0.522591, 0.311135],
];
static YDBDR: [[f64; 3]; 3] = [Y601, [-0.450, -0.883, 1.333], [-1.333, 1.116, 0.217]];
static OHTA: [[f64; 3]; 3] = [
    [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
    [0.5, 0.0, -0.5],
    [-0.25, 0.5, -0.25],
];
/// Linear sRGB → XYZ (D65).
static SRGB_XYZ: [[f64; 3]; 3] = [
    [0.4124564, 0.3575761, 0.1804375],
    [0.2126729, 0.7151522, 0.0721750],
    [0.0193339, 0.1191920, 0.9503041],
];
/// Linear Display P3 → XYZ (D65).
static P3_XYZ: [[f64; 3]; 3] = [
    [0.4865709, 0.2656677, 0.1982173],
    [0.2289746, 0.6917385, 0.0792869],
    [0.0, 0.0451134, 1.0439444],
];
/// XYZ → CAT02 LMS.
static CAT02: [[f64; 3]; 3] = [
    [0.7328, 0.4296, -0.1624],
    [-0.7036, 1.6975, 0.0061],
    [0.0030, 0.0136, 0.9834],
];
const WHITE: [f64; 3] = [0.950456, 1.0, 1.088754];

impl Cs {
    /// ImageMagick's colour space name, or `None` for one not handled here.
    pub(crate) fn parse(name: &str) -> Option<Cs> {
        Some(match name.to_ascii_lowercase().as_str() {
            "cmy" => Cs::Cmy,
            "ycbcr" | "rec601ycbcr" | "ypbpr" => Cs::Matrix(&YCBCR601, true),
            "rec709ycbcr" => Cs::Matrix(&YCBCR709, true),
            "yuv" => Cs::Matrix(&YUV, true),
            "yiq" => Cs::Matrix(&YIQ, true),
            "ydbdr" => Cs::Matrix(&YDBDR, true),
            "ohta" => Cs::Matrix(&OHTA, true),
            "hsl" => Cs::Hsl,
            "hsv" | "hsb" => Cs::Hsv,
            "hsi" => Cs::Hsi,
            "hwb" => Cs::Hwb,
            "hcl" => Cs::Hcl,
            "hclp" => Cs::Hclp,
            // ImageMagick stores CAT02LMS as XYZ.
            "xyz" | "cat02lms" => Cs::Xyz,
            "xyy" => Cs::XyY,
            "lms" => Cs::Lms,
            "lab" | "cielab" => Cs::Lab,
            "lch" | "lchab" => Cs::Lch,
            "luv" => Cs::Luv,
            "lchuv" => Cs::LchUv,
            "log" => Cs::Log,
            "scrgb" => Cs::ScRgb,
            "displayp3" => Cs::DisplayP3,
            "oklab" => Cs::Oklab,
            _ => return None,
        })
    }

    /// The name shown in the info panel.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Cs::Cmy => "CMY",
            Cs::Matrix(m, _) if std::ptr::eq(m, &YCBCR601) => "YCbCr (BT.601)",
            Cs::Matrix(m, _) if std::ptr::eq(m, &YCBCR709) => "YCbCr (BT.709)",
            Cs::Matrix(m, _) if std::ptr::eq(m, &YUV) => "YUV",
            Cs::Matrix(m, _) if std::ptr::eq(m, &YIQ) => "YIQ",
            Cs::Matrix(m, _) if std::ptr::eq(m, &YDBDR) => "YDbDr",
            Cs::Matrix(..) => "Ohta I1I2I3",
            Cs::Hsl => "HSL",
            Cs::Hsv => "HSV",
            Cs::Hsi => "HSI",
            Cs::Hwb => "HWB",
            Cs::Hcl => "HCL",
            Cs::Hclp => "HCLp",
            Cs::Xyz => "CIE XYZ",
            Cs::XyY => "CIE xyY",
            Cs::Lms => "LMS (CAT02)",
            Cs::Lab => "CIE L*a*b*",
            Cs::Lch => "CIE LCHab",
            Cs::Luv => "CIE L*u*v*",
            Cs::LchUv => "CIE LCHuv",
            Cs::Log => "printing-density log",
            Cs::ScRgb => "scRGB (linear)",
            Cs::DisplayP3 => "Display P3",
            Cs::Oklab => "Oklab",
        }
    }

    /// Stored channels (0..1) → sRGB-coded red, green, blue (0..1).
    pub(crate) fn to_srgb(self, s: [f32; 3]) -> [f32; 3] {
        let c = s.map(|v| v as f64);
        let rgb = match self {
            Cs::Cmy => c.map(|v| 1.0 - v),
            // YUV back through the published (rounded) inverse, as written files expect.
            Cs::Matrix(m, _) if std::ptr::eq(m, &YUV) => {
                let (y, u, v) = (c[0], c[1] - 0.5, c[2] - 0.5);
                [
                    y + 1.13983 * v,
                    y - 0.39465 * u - 0.58060 * v,
                    y + 2.03211 * u,
                ]
            }
            Cs::Matrix(m, offset) => {
                let o = if offset { 0.5 } else { 0.0 };
                mul(&inv(m), [c[0], c[1] - o, c[2] - o])
            }
            Cs::Hsl => {
                let (h, s, l) = (c[0], c[1], c[2]);
                let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
                hue_rgb(h, chroma, l - chroma / 2.0)
            }
            Cs::Hsv => {
                let chroma = c[2] * c[1];
                hue_rgb(c[0], chroma, c[2] - chroma)
            }
            Cs::Hsi => hsi(c[0], c[1], c[2]),
            Cs::Hwb => hwb(c[0], c[1], c[2]),
            Cs::Hcl | Cs::Hclp => hcl(c[0], c[1], c[2], self == Cs::Hclp),
            Cs::Xyz => from_xyz(c),
            Cs::XyY => {
                let (x, y, yy) = (c[0], c[1], c[2]);
                if y <= 0.0 {
                    [0.0; 3]
                } else {
                    from_xyz([x * yy / y, yy, (1.0 - x - y) * yy / y])
                }
            }
            Cs::Lms => from_xyz(mul(&inv(&CAT02), c)),
            Cs::Lab => from_xyz(lab_xyz(
                c[0] * 100.0,
                (c[1] - 0.5) * 255.0,
                (c[2] - 0.5) * 255.0,
            )),
            Cs::Lch => {
                let (a, b) = polar((c[1] - 0.5) * 255.0, c[2]);
                from_xyz(lab_xyz(c[0] * 100.0, a, b))
            }
            Cs::Luv => from_xyz(luv_xyz(
                c[0] * 100.0,
                c[1] * 354.0 - 134.0,
                c[2] * 262.0 - 140.0,
            )),
            Cs::LchUv => {
                let (u, v) = polar((c[1] - 0.5) * 255.0, c[2]);
                from_xyz(luv_xyz(c[0] * 100.0, u, v))
            }
            Cs::Log => c.map(|v| {
                // Kodak printing density: codes 95 (black) and 685 (white) of 1024, 0.002
                // density per code, negative gamma 0.6.
                let k = 0.002 / 0.6;
                let black = 10f64.powf((95.0 - 685.0) * k);
                let lin = (10f64.powf((v * 1024.0 - 685.0) * k) - black) / (1.0 - black);
                encode(lin)
            }),
            Cs::ScRgb => c.map(encode),
            Cs::DisplayP3 => from_xyz(mul(&P3_XYZ, c.map(decode))),
            Cs::Oklab => oklab(c[0], c[1] - 0.5, c[2] - 0.5),
        };
        rgb.map(|v| v.clamp(0.0, 1.0) as f32)
    }
}

fn mul(m: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn inv(m: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    [
        [
            (e * i - f * h) / det,
            (c * h - b * i) / det,
            (b * f - c * e) / det,
        ],
        [
            (f * g - d * i) / det,
            (a * i - c * g) / det,
            (c * d - a * f) / det,
        ],
        [
            (d * h - e * g) / det,
            (b * g - a * h) / det,
            (a * e - b * d) / det,
        ],
    ]
}

/// sRGB-coded → linear.
fn decode(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear → sRGB-coded.
fn encode(v: f64) -> f64 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn from_xyz(xyz: [f64; 3]) -> [f64; 3] {
    mul(&inv(&SRGB_XYZ), xyz).map(encode)
}

/// Chroma and hue (fraction of a turn) → the two Cartesian components.
fn polar(chroma: f64, hue: f64) -> (f64, f64) {
    let h = hue * std::f64::consts::TAU;
    (chroma * h.cos(), chroma * h.sin())
}

fn lab_xyz(l: f64, a: f64, b: f64) -> [f64; 3] {
    let (eps, kappa) = (216.0 / 24389.0, 24389.0 / 27.0);
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let f = |t: f64| {
        if t * t * t > eps {
            t * t * t
        } else {
            (116.0 * t - 16.0) / kappa
        }
    };
    let y = if l > kappa * eps {
        fy * fy * fy
    } else {
        l / kappa
    };
    [f(fx) * WHITE[0], y * WHITE[1], f(fz) * WHITE[2]]
}

fn luv_xyz(l: f64, u: f64, v: f64) -> [f64; 3] {
    let (eps, kappa) = (216.0 / 24389.0, 24389.0 / 27.0);
    if l <= 0.0 {
        return [0.0; 3];
    }
    let den = WHITE[0] + 15.0 * WHITE[1] + 3.0 * WHITE[2];
    let (u0, v0) = (4.0 * WHITE[0] / den, 9.0 * WHITE[1] / den);
    let y = if l > kappa * eps {
        ((l + 16.0) / 116.0).powi(3)
    } else {
        l / kappa
    };
    let a = (52.0 * l / (u + 13.0 * l * u0) - 1.0) / 3.0;
    let b = -5.0 * y;
    let d = y * (39.0 * l / (v + 13.0 * l * v0) - 5.0);
    let x = (d - b) / (a + 1.0 / 3.0);
    [x, y, x * a + b]
}

/// RGB of hue `h` (fraction of a turn) and chroma `c`, plus `m` on every channel.
fn hue_rgb(h: f64, c: f64, m: f64) -> [f64; 3] {
    let h6 = (h.rem_euclid(1.0)) * 6.0;
    let x = c * (1.0 - (h6 % 2.0 - 1.0).abs());
    let (r, g, b) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m]
}

fn hsi(h: f64, s: f64, i: f64) -> [f64; 3] {
    let deg = h.rem_euclid(1.0) * 360.0;
    let rad = |d: f64| d.to_radians();
    let lo = i * (1.0 - s);
    let hi = |d: f64| i * (1.0 + s * rad(d).cos() / rad(60.0 - d).cos());
    if deg < 120.0 {
        let r = hi(deg);
        [r, 3.0 * i - r - lo, lo]
    } else if deg < 240.0 {
        let g = hi(deg - 120.0);
        [lo, g, 3.0 * i - g - lo]
    } else {
        let b = hi(deg - 240.0);
        [3.0 * i - b - lo, lo, b]
    }
}

fn hwb(h: f64, w: f64, b: f64) -> [f64; 3] {
    let v = 1.0 - b;
    if v <= w {
        return [v; 3];
    }
    let h6 = h.rem_euclid(1.0) * 6.0;
    let i = h6.floor();
    let mut f = h6 - i;
    if i as u32 & 1 == 1 {
        f = 1.0 - f;
    }
    let n = w + f * (v - w);
    match i as u32 {
        0 => [v, n, w],
        1 => [n, v, w],
        2 => [w, v, n],
        3 => [w, n, v],
        4 => [n, w, v],
        _ => [v, w, n],
    }
}

fn hcl(h: f64, c: f64, l: f64, keep_hue: bool) -> [f64; 3] {
    let rgb = hue_rgb(h, c, 0.0);
    let luma = |v: [f64; 3]| Y601[0] * v[0] + Y601[1] * v[1] + Y601[2] * v[2];
    let m = l - luma(rgb);
    let out = rgb.map(|v| v + m);
    if !keep_hue {
        return out;
    }
    // HCLp: scale the chroma down instead of clipping, keeping the hue.
    let z = if m < 0.0 {
        let y = luma(rgb);
        if y > 0.0 { l / y } else { 1.0 }
    } else {
        let max = rgb.iter().copied().fold(0.0, f64::max);
        if m + max > 1.0 && max > luma(rgb) {
            (1.0 - l) / (max - luma(rgb))
        } else {
            1.0
        }
    };
    if z == 1.0 {
        return out;
    }
    let y = luma(rgb);
    rgb.map(|v| l + (v - y) * z)
}

fn oklab(l: f64, a: f64, b: f64) -> [f64; 3] {
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    [
        4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3,
        -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3,
        -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3,
    ]
    .map(encode)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ImageMagick's stored values of sRGB (0.2, 0.5, 0.8) in each space.
    #[test]
    fn known_values() {
        let want = [0.2f32, 0.5, 0.8];
        for (name, s) in [
            ("CMY", [0.8, 0.49999, 0.2]),
            ("YCbCr", [0.44466, 0.70062, 0.32560]),
            ("Rec709YCbCr", [0.45786, 0.68438, 0.33625]),
            ("YUV", [0.44466, 0.67489, 0.28550]),
            ("YIQ", [0.44466, 0.22490, 0.52990]),
            ("OHTA", [0.50001, 0.20002, 0.50001]),
            ("HSL", [0.58334, 0.6, 0.50001]),
            ("HSV", [0.58334, 0.75, 0.8]),
            ("HSI", [0.58334, 0.6, 0.50001]),
            ("HWB", [0.58334, 0.2, 0.2]),
            ("HCL", [0.58334, 0.6, 0.44466]),
            ("XYZ", [0.19918, 0.20371, 0.60009]),
            ("xyY", [0.19858, 0.20311, 0.20371]),
            ("LMS", [0.13602, 0.20932, 0.59350]),
            ("CIELab", [0.52255, 0.51093, 0.31844]),
            ("LCHab", [0.52255, 0.68188, 0.75956]),
            ("Luv", [0.52255, 0.30132, 0.26043]),
            ("LCHuv", [0.52255, 0.80117, 0.69209]),
            ("Log", [0.27022, 0.47776, 0.60566]),
            ("scRGB", [0.03311, 0.21405, 0.60383]),
            ("DisplayP3", [0.28333, 0.49346, 0.77716]),
            ("Oklab", [0.58834, 0.45571, 0.36809]),
        ] {
            let got = Cs::parse(name).unwrap().to_srgb(s);
            for k in 0..3 {
                assert!((got[k] - want[k]).abs() < 2e-3, "{name}: {got:?}");
            }
        }
    }
}
