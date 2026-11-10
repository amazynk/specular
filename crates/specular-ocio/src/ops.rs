//! The CPU operations a processor runs on each RGB value, in f64: matrices with offsets,
//! exponents (with OCIO's negative styles), the sRGB-style piecewise curve, logs (plain,
//! affine and camera), ranges, ASC CDL, 1D and 3D tables, and the ACES fixed functions.

/// How negative values pass an exponent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Negative {
    /// Clamped to 0 (OCIO's default for ExponentTransform: `clamp`).
    Clamp,
    /// Mirrored around 0.
    Mirror,
    /// Passed through unchanged.
    PassThru,
    /// Continued as a line (ExponentWithLinear: `linear`).
    Linear,
}

/// 3D table interpolation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interp {
    Trilinear,
    Tetrahedral,
}

/// One operation.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// `out = m · in + offset`, 3 × 3 (the alpha row and column of OCIO's 4 × 4 dropped).
    Matrix { m: [f64; 9], offset: [f64; 3] },
    /// `out = in ^ e` per channel (forward), or `in ^ (1/e)` (inverse).
    Exponent {
        e: [f64; 3],
        neg: Negative,
        inverse: bool,
    },
    /// The sRGB-style curve: linear below a break, `((x + o) / (1 + o)) ^ g` above
    /// (forward: encoded → linear).
    ExpLinear {
        g: [f64; 3],
        o: [f64; 3],
        neg: Negative,
        inverse: bool,
    },
    /// Log of base `base` with affine sides and, for camera logs, a linear segment below
    /// `lin_break` (forward: linear → log).
    Log {
        base: f64,
        log_slope: [f64; 3],
        log_offset: [f64; 3],
        lin_slope: [f64; 3],
        lin_offset: [f64; 3],
        lin_break: Option<[f64; 3]>,
        linear_slope: Option<[f64; 3]>,
        inverse: bool,
    },
    /// `out = in · scale + offset`, then clamped to `[lo, hi]` where given.
    Range {
        scale: f64,
        offset: f64,
        lo: Option<f64>,
        hi: Option<f64>,
    },
    /// ASC CDL: slope, offset, power then saturation (Rec. 709 luma), clamped (v1.2).
    Cdl {
        slope: [f64; 3],
        offset: [f64; 3],
        power: [f64; 3],
        sat: f64,
        clamp: bool,
        inverse: bool,
    },
    /// 1D tables per channel over `[lo, hi]` of the input, linearly interpolated.
    Lut1d {
        size: usize,
        /// `size` RGB entries.
        values: Vec<[f64; 3]>,
        lo: [f64; 3],
        hi: [f64; 3],
        /// Entries indexed by the half-float bit pattern of the input (CTF half domain).
        half_domain: bool,
        inverse: bool,
    },
    /// A 3D table over `[lo, hi]³`, blue fastest (`values[(r·n + g)·n + b]`); readers of
    /// red-fastest files (.cube) reorder.
    Lut3d {
        size: usize,
        values: Vec<[f64; 3]>,
        lo: [f64; 3],
        hi: [f64; 3],
        interp: Interp,
    },
    /// ACES 1.0 RRT glow (forward or inverse).
    AcesGlow10 { inverse: bool },
    /// ACES 1.0 RRT red modifier.
    AcesRedMod10 { inverse: bool },
    /// ACES 1.0 dark to dim surround (in AP1 linear).
    AcesDarkToDim10 { inverse: bool },
    /// The ACES 1.0 RRT tone scale (segmented spline C5) then the 48-nit ODT tone scale
    /// (segmented spline C9), per channel, in AP1; output in cinema Y (0.02–48).
    AcesToneScale10 { inverse: bool },
    /// `Y → linear CV` of the ACES ODTs (`(Y - black) / (white - black)`).
    AcesYToLinCv {
        white: f64,
        black: f64,
        inverse: bool,
    },
}

#[inline]
fn signed_pow(x: f64, e: f64, neg: Negative) -> f64 {
    if x >= 0.0 {
        return x.powf(e);
    }
    match neg {
        Negative::Clamp => 0.0,
        Negative::Mirror => -(-x).powf(e),
        Negative::PassThru | Negative::Linear => x,
    }
}

/// The sRGB-style curve, encoded → linear (gamma `g`, offset `o`).
fn explin_fwd(x: f64, g: f64, o: f64, neg: Negative) -> f64 {
    // Break where the line meets the curve with matching slope.
    let brk = o / (g - 1.0);
    let slope = ((1.0 + o) / g).powf(g) * ((g - 1.0) / o).powf(g - 1.0);
    let x_brk = brk; // in encoded units
    if x < x_brk {
        if x < 0.0 && neg == Negative::Mirror {
            return -explin_fwd(-x, g, o, neg);
        }
        x / slope
    } else {
        ((x + o) / (1.0 + o)).powf(g)
    }
}

fn explin_inv(y: f64, g: f64, o: f64, neg: Negative) -> f64 {
    let brk = o / (g - 1.0);
    let slope = ((1.0 + o) / g).powf(g) * ((g - 1.0) / o).powf(g - 1.0);
    let y_brk = brk / slope;
    if y < y_brk {
        if y < 0.0 && neg == Negative::Mirror {
            return -explin_inv(-y, g, o, neg);
        }
        y * slope
    } else {
        (1.0 + o) * y.powf(1.0 / g) - o
    }
}

// ---- ACES 1.0 RRT/ODT pieces, from the public Academy CTL (aces-dev 1.0.3).

fn rgb_2_saturation(rgb: [f64; 3]) -> f64 {
    let mx = rgb[0].max(rgb[1]).max(rgb[2]);
    let mn = rgb[0].min(rgb[1]).min(rgb[2]);
    (mx.max(1e-10) - mn.max(1e-10)) / mx.max(1e-2)
}

fn rgb_2_yc(rgb: [f64; 3]) -> f64 {
    let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
    let chroma = (b * (b - g) + g * (g - r) + r * (r - b)).max(0.0).sqrt();
    (b + g + r + 1.75 * chroma) / 3.0
}

fn sigmoid_shaper(x: f64) -> f64 {
    let t = (1.0 - (x / 2.0).abs()).max(0.0);
    let y = 1.0 + (1.0 - t * t).copysign(x);
    y / 2.0
}

fn glow_fwd(yc_in: f64, glow_gain_in: f64, glow_mid: f64) -> f64 {
    if yc_in <= 2.0 / 3.0 * glow_mid {
        glow_gain_in
    } else if yc_in >= 2.0 * glow_mid {
        0.0
    } else {
        glow_gain_in * (glow_mid / yc_in - 0.5)
    }
}

fn glow_inv(yc_out: f64, glow_gain_in: f64, glow_mid: f64) -> f64 {
    if yc_out <= (1.0 + glow_gain_in) * 2.0 / 3.0 * glow_mid {
        -glow_gain_in / (1.0 + glow_gain_in)
    } else if yc_out >= 2.0 * glow_mid {
        0.0
    } else {
        glow_gain_in * (glow_mid / yc_out - 0.5) / (glow_gain_in / 2.0 - 1.0)
    }
}

fn rgb_2_hue(rgb: [f64; 3]) -> f64 {
    let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
    if r == g && g == b {
        return 0.0;
    }
    let h = (3f64.sqrt() * (g - b)).atan2(2.0 * r - g - b).to_degrees();
    if h < 0.0 { h + 360.0 } else { h }
}

fn center_hue(hue: f64, center: f64) -> f64 {
    let mut h = hue - center;
    if h < -180.0 {
        h += 360.0;
    } else if h > 180.0 {
        h -= 360.0;
    }
    h
}

fn cubic_basis_shaper(x: f64, w: f64) -> f64 {
    const M: [[f64; 4]; 4] = [
        [-1. / 6., 3. / 6., -3. / 6., 1. / 6.],
        [3. / 6., -6. / 6., 3. / 6., 0.],
        [-3. / 6., 0., 3. / 6., 0.],
        [1. / 6., 4. / 6., 1. / 6., 0.],
    ];
    let knots = [-w / 2.0, -w / 4.0, 0.0, w / 4.0, w / 2.0];
    if x <= knots[0] || x >= knots[4] {
        return 0.0;
    }
    let knot_coord = (x - knots[0]) * 4.0 / w;
    let j = (knot_coord as usize).min(3);
    let t = knot_coord - j as f64;
    let monomials = [t * t * t, t * t, t, 1.0];
    let row = |r: usize| {
        monomials[0] * M[0][r]
            + monomials[1] * M[1][r]
            + monomials[2] * M[2][r]
            + monomials[3] * M[3][r]
    };
    1.5 * match j {
        3 => row(0),
        2 => row(1),
        1 => row(2),
        _ => row(3),
    }
}

const RRT_GLOW_GAIN: f64 = 0.05;
const RRT_GLOW_MID: f64 = 0.08;
const RRT_RED_SCALE: f64 = 0.82;
const RRT_RED_PIVOT: f64 = 0.03;
const RRT_RED_HUE: f64 = 0.0;
const RRT_RED_WIDTH: f64 = 135.0;

fn aces_glow(rgb: [f64; 3], inverse: bool) -> [f64; 3] {
    let sat = rgb_2_saturation(rgb);
    let s = sigmoid_shaper((sat - 0.4) / 0.2);
    let yc = rgb_2_yc(rgb);
    let g = if inverse {
        1.0 + glow_inv(yc, RRT_GLOW_GAIN * s, RRT_GLOW_MID)
    } else {
        1.0 + glow_fwd(yc, RRT_GLOW_GAIN * s, RRT_GLOW_MID)
    };
    [rgb[0] * g, rgb[1] * g, rgb[2] * g]
}

fn aces_red_mod(rgb: [f64; 3], inverse: bool) -> [f64; 3] {
    let hue = rgb_2_hue(rgb);
    let centred = center_hue(hue, RRT_RED_HUE);
    let w = cubic_basis_shaper(centred, RRT_RED_WIDTH);
    let mut o = rgb;
    if !inverse {
        let sat = rgb_2_saturation(rgb);
        o[0] = rgb[0] + w * sat * (RRT_RED_PIVOT - rgb[0]) * (1.0 - RRT_RED_SCALE);
    } else {
        // The CTL inverse: solve the quadratic for the red channel.
        let min_chan = if centred < 0.0 { rgb[1] } else { rgb[2] };
        let a = w * (1.0 - RRT_RED_SCALE) - 1.0;
        let bb = rgb[0] - w * (RRT_RED_PIVOT + min_chan) * (1.0 - RRT_RED_SCALE);
        let c = w * RRT_RED_PIVOT * min_chan * (1.0 - RRT_RED_SCALE);
        o[0] = if a == 0.0 {
            rgb[0]
        } else {
            (-bb - (bb * bb - 4.0 * a * c).max(0.0).sqrt()) / (2.0 * a)
        };
    }
    o
}

// Segmented splines (ACES 1.0 CTL `ACESlib.Tonescales`).
struct SplineC5 {
    coefs_low: [f64; 6],
    coefs_high: [f64; 6],
    min: (f64, f64),
    mid: (f64, f64),
    max: (f64, f64),
}

const RRT_PARAMS: SplineC5 = SplineC5 {
    coefs_low: [
        -4.0,
        -4.0,
        -3.1573765773,
        -0.4852499958,
        1.8477324706,
        1.8477324706,
    ],
    coefs_high: [-0.7185482425, 2.0810307172, 3.6681241237, 4.0, 4.0, 4.0],
    min: (0.18 / 32768.0, 0.0001),
    mid: (0.18, 4.8),
    max: (0.18 * 262_144.0, 10000.0),
};

fn spline_c5_fwd(x: f64) -> f64 {
    let c = &RRT_PARAMS;
    const N_KNOTS_LOW: usize = 4;
    const N_KNOTS_HIGH: usize = 4;
    let log_x = x.max(HALF_MIN).log10();
    let m = [[0.5, -1.0, 0.5], [-1.0, 1.0, 0.5], [0.5, 0.0, 0.0]];
    let log_y = if log_x <= c.min.0.log10() {
        c.min.1.log10()
    } else if log_x < c.mid.0.log10() {
        let knot_coord = (N_KNOTS_LOW - 1) as f64 * (log_x - c.min.0.log10())
            / (c.mid.0.log10() - c.min.0.log10());
        let j = knot_coord as usize;
        let t = knot_coord - j as f64;
        let cf = [c.coefs_low[j], c.coefs_low[j + 1], c.coefs_low[j + 2]];
        let monomials = [t * t, t, 1.0];
        dot3(monomials, mul_m3(m, cf))
    } else if log_x < c.max.0.log10() {
        let knot_coord = (N_KNOTS_HIGH - 1) as f64 * (log_x - c.mid.0.log10())
            / (c.max.0.log10() - c.mid.0.log10());
        let j = knot_coord as usize;
        let t = knot_coord - j as f64;
        let cf = [c.coefs_high[j], c.coefs_high[j + 1], c.coefs_high[j + 2]];
        let monomials = [t * t, t, 1.0];
        dot3(monomials, mul_m3(m, cf))
    } else {
        c.max.1.log10()
    };
    10f64.powf(log_y)
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `M · c` with M stored by rows as the CTL's `mult_f3_f33(cf, M)` (row vector × matrix).
fn mul_m3(m: [[f64; 3]; 3], cf: [f64; 3]) -> [f64; 3] {
    // CTL: monomials · (cf · M): cf row vector times M.
    [
        cf[0] * m[0][0] + cf[1] * m[1][0] + cf[2] * m[2][0],
        cf[0] * m[0][1] + cf[1] * m[1][1] + cf[2] * m[2][1],
        cf[0] * m[0][2] + cf[1] * m[1][2] + cf[2] * m[2][2],
    ]
}

fn spline_c5_inv(y: f64) -> f64 {
    let c = &RRT_PARAMS;
    let knot_inc_low = (c.mid.0.log10() - c.min.0.log10()) / 3.0;
    let knot_inc_high = (c.max.0.log10() - c.mid.0.log10()) / 3.0;
    let mut knot_y_low = [0.0; 4];
    for (i, k) in knot_y_low.iter_mut().enumerate() {
        *k = (c.coefs_low[i] + c.coefs_low[i + 1]) / 2.0;
    }
    let mut knot_y_high = [0.0; 4];
    for (i, k) in knot_y_high.iter_mut().enumerate() {
        *k = (c.coefs_high[i] + c.coefs_high[i + 1]) / 2.0;
    }
    let log_y = y.max(1e-10).log10();
    let m = [[0.5, -1.0, 0.5], [-1.0, 1.0, 0.5], [0.5, 0.0, 0.0]];
    let solve = |ks: &[f64; 4], coefs: &[f64; 6], lo: f64, inc: f64| -> f64 {
        // The segment whose knots enclose the value (the first or last past the ends).
        let j = (0..3).rev().find(|&k| log_y > ks[k]).unwrap_or(0);
        let cf = [coefs[j], coefs[j + 1], coefs[j + 2]];
        let tmp = mul_m3(m, cf);
        let (a, b, cc) = (tmp[0], tmp[1], tmp[2] - log_y);
        let d = (b * b - 4.0 * a * cc).max(0.0).sqrt();
        let t = (2.0 * cc) / (-d - b);
        lo + (t + j as f64) * inc
    };
    let log_x = if log_y <= c.min.1.log10() {
        c.min.0.log10()
    } else if log_y <= c.mid.1.log10() {
        solve(&knot_y_low, &c.coefs_low, c.min.0.log10(), knot_inc_low)
    } else if log_y < c.max.1.log10() {
        solve(&knot_y_high, &c.coefs_high, c.mid.0.log10(), knot_inc_high)
    } else {
        c.max.0.log10()
    };
    10f64.powf(log_x)
}

struct SplineC9 {
    coefs_low: [f64; 10],
    coefs_high: [f64; 10],
    min: (f64, f64),
    mid: (f64, f64),
    max: (f64, f64),
}

/// ODT 48 nits: minPoint (segmented_spline_c5_fwd(0.18·2^-6.5), 0.02), mid (…(0.18), 4.8),
/// max (…(0.18·2^6.5), 48).
fn odt_48() -> SplineC9 {
    SplineC9 {
        coefs_low: [
            -1.6989700043,
            -1.6989700043,
            -1.4779000000,
            -1.2291000000,
            -0.8648000000,
            -0.4480000000,
            0.0051800000,
            0.4511080334,
            0.9113744414,
            0.9113744414,
        ],
        coefs_high: [
            0.5154386965,
            0.8470437783,
            1.1358000000,
            1.3802000000,
            1.5197000000,
            1.5985000000,
            1.6467000000,
            1.6746091357,
            1.6878733390,
            1.6878733390,
        ],
        min: (spline_c5_fwd(0.18 * 2f64.powf(-6.5)), 0.02),
        mid: (spline_c5_fwd(0.18), 4.8),
        max: (spline_c5_fwd(0.18 * 2f64.powf(6.5)), 48.0),
    }
}

/// The smallest positive half float, the CTL's floor before logs.
const HALF_MIN: f64 = 5.960_464_48e-8;
/// The 48-nit ODT spline's slope above its maximum.
const ODT_SLOPE_HIGH: f64 = 0.04;

fn spline_c9_fwd(x: f64, c: &SplineC9) -> f64 {
    let log_x = x.max(HALF_MIN).log10();
    let m = [[0.5, -1.0, 0.5], [-1.0, 1.0, 0.5], [0.5, 0.0, 0.0]];
    let log_y = if log_x <= c.min.0.log10() {
        c.min.1.log10()
    } else if log_x < c.mid.0.log10() {
        let kc = 7.0 * (log_x - c.min.0.log10()) / (c.mid.0.log10() - c.min.0.log10());
        let j = (kc as usize).min(7);
        let t = kc - j as f64;
        let cf = [c.coefs_low[j], c.coefs_low[j + 1], c.coefs_low[j + 2]];
        dot3([t * t, t, 1.0], mul_m3(m, cf))
    } else if log_x < c.max.0.log10() {
        let kc = 7.0 * (log_x - c.mid.0.log10()) / (c.max.0.log10() - c.mid.0.log10());
        let j = (kc as usize).min(7);
        let t = kc - j as f64;
        let cf = [c.coefs_high[j], c.coefs_high[j + 1], c.coefs_high[j + 2]];
        dot3([t * t, t, 1.0], mul_m3(m, cf))
    } else {
        log_x * ODT_SLOPE_HIGH + (c.max.1.log10() - ODT_SLOPE_HIGH * c.max.0.log10())
    };
    10f64.powf(log_y)
}

fn spline_c9_inv(y: f64, c: &SplineC9) -> f64 {
    let knot_inc_low = (c.mid.0.log10() - c.min.0.log10()) / 7.0;
    let knot_inc_high = (c.max.0.log10() - c.mid.0.log10()) / 7.0;
    let mut kyl = [0.0; 8];
    let mut kyh = [0.0; 8];
    for i in 0..8 {
        kyl[i] = (c.coefs_low[i] + c.coefs_low[i + 1]) / 2.0;
        kyh[i] = (c.coefs_high[i] + c.coefs_high[i + 1]) / 2.0;
    }
    let log_y = y.max(1e-10).log10();
    let m = [[0.5, -1.0, 0.5], [-1.0, 1.0, 0.5], [0.5, 0.0, 0.0]];
    let solve = |ks: &[f64; 8], coefs: &[f64; 10], lo: f64, inc: f64| -> f64 {
        let j = (0..7).rev().find(|&k| log_y > ks[k]).unwrap_or(0);
        let cf = [coefs[j], coefs[j + 1], coefs[j + 2]];
        let tmp = mul_m3(m, cf);
        let (a, b, cc) = (tmp[0], tmp[1], tmp[2] - log_y);
        let d = (b * b - 4.0 * a * cc).max(0.0).sqrt();
        let t = (2.0 * cc) / (-d - b);
        lo + (t + j as f64) * inc
    };
    let log_x = if log_y <= c.min.1.log10() {
        c.min.0.log10()
    } else if log_y <= c.mid.1.log10() {
        solve(&kyl, &c.coefs_low, c.min.0.log10(), knot_inc_low)
    } else if log_y < c.max.1.log10() {
        solve(&kyh, &c.coefs_high, c.mid.0.log10(), knot_inc_high)
    } else {
        c.max.0.log10()
    };
    10f64.powf(log_x)
}

const DIM_SURROUND_GAMMA: f64 = 0.9811;

fn xyy(xyz: [f64; 3]) -> [f64; 3] {
    let d = (xyz[0] + xyz[1] + xyz[2]).max(1e-10);
    [xyz[0] / d, xyz[1] / d, xyz[1]]
}

fn xyy_xyz(v: [f64; 3]) -> [f64; 3] {
    let (x, y, yy) = (v[0], v[1].max(1e-10), v[2]);
    [x * yy / y, yy, (1.0 - x - y) * yy / y]
}

/// AP1 → XYZ (D60 white, the CTL's `AP1_2_XYZ_MAT`).
pub(crate) const AP1_TO_XYZ: [f64; 9] = [
    0.6624541811,
    0.1340042065,
    0.1561876870,
    0.2722287168,
    0.6740817658,
    0.0536895174,
    -0.0055746495,
    0.0040607335,
    1.0103391003,
];

fn mat3_mul(m: &[f64; 9], v: [f64; 3]) -> [f64; 3] {
    [
        m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
        m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
        m[6] * v[0] + m[7] * v[1] + m[8] * v[2],
    ]
}

/// 3 × 3 inverse.
pub(crate) fn mat3_inv(m: &[f64; 9]) -> Option<[f64; 9]> {
    let det = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
        + m[2] * (m[3] * m[7] - m[4] * m[6]);
    if det.abs() < 1e-300 {
        return None;
    }
    let i = 1.0 / det;
    Some([
        (m[4] * m[8] - m[5] * m[7]) * i,
        (m[2] * m[7] - m[1] * m[8]) * i,
        (m[1] * m[5] - m[2] * m[4]) * i,
        (m[5] * m[6] - m[3] * m[8]) * i,
        (m[0] * m[8] - m[2] * m[6]) * i,
        (m[2] * m[3] - m[0] * m[5]) * i,
        (m[3] * m[7] - m[4] * m[6]) * i,
        (m[1] * m[6] - m[0] * m[7]) * i,
        (m[0] * m[4] - m[1] * m[3]) * i,
    ])
}

fn dark_to_dim(rgb: [f64; 3], inverse: bool) -> [f64; 3] {
    let xyz = mat3_mul(&AP1_TO_XYZ, rgb);
    let mut v = xyy(xyz);
    let g = if inverse {
        1.0 / DIM_SURROUND_GAMMA
    } else {
        DIM_SURROUND_GAMMA
    };
    v[2] = v[2].clamp(0.0, 65504.0).powf(g);
    let xyz = xyy_xyz(v);
    let inv = mat3_inv(&AP1_TO_XYZ).unwrap_or([1., 0., 0., 0., 1., 0., 0., 0., 1.]);
    mat3_mul(&inv, xyz)
}

/// Linear interpolation into a 1D table at fractional index `f`.
fn lerp_table(values: &[[f64; 3]], c: usize, f: f64) -> f64 {
    let n = values.len();
    if n == 0 {
        return f;
    }
    let f = f.clamp(0.0, (n - 1) as f64);
    let i = (f.floor() as usize).min(n.saturating_sub(2));
    let t = f - i as f64;
    if n == 1 {
        return values[0][c];
    }
    values[i][c] * (1.0 - t) + values[i + 1][c] * t
}

/// The input that a monotonic 1D table maps to `y` (channel `c`), as a fraction of the table.
fn invert_table(values: &[[f64; 3]], c: usize, y: f64) -> f64 {
    let n = values.len();
    if n < 2 {
        return 0.0;
    }
    let rising = values[n - 1][c] >= values[0][c];
    let at = |i: usize| if rising { values[i][c] } else { -values[i][c] };
    let y = if rising { y } else { -y };
    if y <= at(0) {
        return 0.0;
    }
    if y >= at(n - 1) {
        return (n - 1) as f64;
    }
    let (mut lo, mut hi) = (0usize, n - 1);
    while hi - lo > 1 {
        let m = (lo + hi) / 2;
        if at(m) <= y {
            lo = m;
        } else {
            hi = m;
        }
    }
    let (a, b) = (at(lo), at(hi));
    if b == a {
        lo as f64
    } else {
        lo as f64 + (y - a) / (b - a)
    }
}

/// The half-float bit pattern of `x` (round to nearest even), for half-domain tables.
pub(crate) fn half_bits(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32 - 127 + 15;
    let man = b & 0x7f_ffff;
    if exp >= 31 {
        return sign
            | 0x7c00
            | if (b & 0x7fff_ffff) > 0x7f80_0000 {
                0x200
            } else {
                0
            };
    }
    if exp <= 0 {
        if exp < -10 {
            return sign;
        }
        let m = man | 0x80_0000;
        let shift = (14 - exp) as u32;
        let half = 1u32 << (shift - 1);
        let mut v = m >> shift;
        let rest = m & ((1 << shift) - 1);
        if rest > half || (rest == half && v & 1 == 1) {
            v += 1;
        }
        return sign | v as u16;
    }
    let mut v = ((exp as u32) << 10) | (man >> 13);
    let rest = man & 0x1fff;
    if rest > 0x1000 || (rest == 0x1000 && v & 1 == 1) {
        v += 1;
    }
    sign | v as u16
}

impl Op {
    /// Runs the operation on `c`.
    pub fn apply(&self, c: &mut [f64; 3]) {
        match self {
            Op::Matrix { m, offset } => {
                let v = mat3_mul(m, *c);
                *c = [v[0] + offset[0], v[1] + offset[1], v[2] + offset[2]];
            }
            Op::Exponent { e, neg, inverse } => {
                for k in 0..3 {
                    let ex = if *inverse { 1.0 / e[k] } else { e[k] };
                    c[k] = signed_pow(c[k], ex, *neg);
                }
            }
            Op::ExpLinear { g, o, neg, inverse } => {
                for k in 0..3 {
                    c[k] = if *inverse {
                        explin_inv(c[k], g[k], o[k], *neg)
                    } else {
                        explin_fwd(c[k], g[k], o[k], *neg)
                    };
                }
            }
            Op::Log {
                base,
                log_slope,
                log_offset,
                lin_slope,
                lin_offset,
                lin_break,
                linear_slope,
                inverse,
            } => {
                let lb = base.ln();
                for k in 0..3 {
                    // Camera log: linear below the break, with a slope matching the log
                    // segment there unless given.
                    let lin_seg = lin_break.map(|b| {
                        let brk = b[k];
                        let slope = linear_slope.map(|s| s[k]).unwrap_or_else(|| {
                            log_slope[k] * lin_slope[k]
                                / ((lin_slope[k] * brk + lin_offset[k]) * lb)
                        });
                        let y_brk = log_slope[k] * (lin_slope[k] * brk + lin_offset[k]).ln() / lb
                            + log_offset[k];
                        (brk, slope, y_brk - slope * brk)
                    });
                    if !inverse {
                        let x = c[k];
                        c[k] = match lin_seg {
                            Some((brk, s, off)) if x <= brk => s * x + off,
                            _ => {
                                let v = (lin_slope[k] * x + lin_offset[k])
                                    .max(f64::from(f32::MIN_POSITIVE));
                                log_slope[k] * v.ln() / lb + log_offset[k]
                            }
                        };
                    } else {
                        let y = c[k];
                        let y_brk = lin_seg.map(|(brk, s, off)| s * brk + off);
                        c[k] = match (lin_seg, y_brk) {
                            (Some((_, s, off)), Some(yb)) if y <= yb => (y - off) / s,
                            _ => {
                                (base.powf((y - log_offset[k]) / log_slope[k]) - lin_offset[k])
                                    / lin_slope[k]
                            }
                        };
                    }
                }
            }
            Op::Range {
                scale,
                offset,
                lo,
                hi,
            } => {
                for v in c.iter_mut() {
                    let mut x = *v * scale + offset;
                    if let Some(l) = lo {
                        x = x.max(*l);
                    }
                    if let Some(h) = hi {
                        x = x.min(*h);
                    }
                    *v = x;
                }
            }
            Op::Cdl {
                slope,
                offset,
                power,
                sat,
                clamp,
                inverse,
            } => {
                const L: [f64; 3] = [0.2126, 0.7152, 0.0722];
                if !inverse {
                    for k in 0..3 {
                        let mut v = c[k] * slope[k] + offset[k];
                        if *clamp {
                            v = v.clamp(0.0, 1.0);
                        }
                        v = if v > 0.0 || !*clamp {
                            signed_pow(v, power[k], Negative::PassThru)
                        } else {
                            0.0
                        };
                        c[k] = v;
                    }
                    let l = dot3(L, *c);
                    for v in c.iter_mut() {
                        *v = l + sat * (*v - l);
                        if *clamp {
                            *v = v.clamp(0.0, 1.0);
                        }
                    }
                } else {
                    let l = dot3(L, *c);
                    for v in c.iter_mut() {
                        *v = l + (*v - l) / sat;
                        if *clamp {
                            *v = v.clamp(0.0, 1.0);
                        }
                    }
                    for k in 0..3 {
                        let v = if c[k] > 0.0 {
                            c[k].powf(1.0 / power[k])
                        } else if *clamp {
                            0.0
                        } else {
                            c[k]
                        };
                        let v = (v - offset[k]) / slope[k];
                        c[k] = if *clamp { v.clamp(0.0, 1.0) } else { v };
                    }
                }
            }
            Op::Lut1d {
                size,
                values,
                lo,
                hi,
                half_domain,
                inverse,
            } => {
                for k in 0..3 {
                    if *half_domain {
                        if *inverse {
                            // Rare; nearest search over the half table.
                            let f = invert_table(values, k, c[k]);
                            c[k] = f64::from(half_to_f32(f.round() as u16));
                        } else {
                            let h = half_bits(c[k] as f32) as usize;
                            c[k] = values.get(h).map_or(c[k], |v| v[k]);
                        }
                        continue;
                    }
                    let span = (hi[k] - lo[k]).max(1e-30);
                    if *inverse {
                        let f = invert_table(values, k, c[k]);
                        c[k] = lo[k] + f / (*size as f64 - 1.0).max(1.0) * span;
                    } else {
                        let f = (c[k] - lo[k]) / span * (*size as f64 - 1.0);
                        c[k] = lerp_table(values, k, f);
                    }
                }
            }
            Op::Lut3d {
                size,
                values,
                lo,
                hi,
                interp,
            } => {
                *c = lut3d(*size, values, *lo, *hi, *interp, *c);
            }
            Op::AcesGlow10 { inverse } => *c = aces_glow(*c, *inverse),
            Op::AcesRedMod10 { inverse } => *c = aces_red_mod(*c, *inverse),
            Op::AcesDarkToDim10 { inverse } => *c = dark_to_dim(*c, *inverse),
            Op::AcesToneScale10 { inverse } => {
                let odt = odt_48();
                for v in c.iter_mut() {
                    *v = if *inverse {
                        spline_c5_inv(spline_c9_inv(*v, &odt))
                    } else {
                        spline_c9_fwd(spline_c5_fwd(*v), &odt)
                    };
                }
            }
            Op::AcesYToLinCv {
                white,
                black,
                inverse,
            } => {
                for v in c.iter_mut() {
                    *v = if *inverse {
                        *v * (white - black) + black
                    } else {
                        (*v - black) / (white - black)
                    };
                }
            }
        }
    }
}

/// A half float's value.
pub(crate) fn half_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = i32::from((h >> 10) & 0x1f);
    let m = f32::from(h & 0x3ff);
    s * match e {
        0 => m * 2f32.powi(-24),
        31 => {
            if m == 0.0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

/// A 3D table lookup; `values[(r·n + g)·n + b]`.
pub(crate) fn lut3d(
    n: usize,
    values: &[[f64; 3]],
    lo: [f64; 3],
    hi: [f64; 3],
    interp: Interp,
    c: [f64; 3],
) -> [f64; 3] {
    if n < 2 || values.len() < n * n * n {
        return c;
    }
    let mut idx = [0usize; 3];
    let mut f = [0f64; 3];
    for k in 0..3 {
        let x = ((c[k] - lo[k]) / (hi[k] - lo[k]).max(1e-30) * (n - 1) as f64)
            .clamp(0.0, (n - 1) as f64);
        let i = (x.floor() as usize).min(n - 2);
        idx[k] = i;
        f[k] = x - i as f64;
    }
    let at = |dr: usize, dg: usize, db: usize| {
        values[((idx[0] + dr) * n + idx[1] + dg) * n + idx[2] + db]
    };
    let mut o = [0f64; 3];
    match interp {
        Interp::Trilinear => {
            for (k, ok) in o.iter_mut().enumerate() {
                let c00 = at(0, 0, 0)[k] * (1.0 - f[2]) + at(0, 0, 1)[k] * f[2];
                let c01 = at(0, 1, 0)[k] * (1.0 - f[2]) + at(0, 1, 1)[k] * f[2];
                let c10 = at(1, 0, 0)[k] * (1.0 - f[2]) + at(1, 0, 1)[k] * f[2];
                let c11 = at(1, 1, 0)[k] * (1.0 - f[2]) + at(1, 1, 1)[k] * f[2];
                let c0 = c00 * (1.0 - f[1]) + c01 * f[1];
                let c1 = c10 * (1.0 - f[1]) + c11 * f[1];
                *ok = c0 * (1.0 - f[0]) + c1 * f[0];
            }
        }
        Interp::Tetrahedral => {
            let (fr, fg, fb) = (f[0], f[1], f[2]);
            let v000 = at(0, 0, 0);
            let v111 = at(1, 1, 1);
            for k in 0..3 {
                o[k] = if fr > fg {
                    if fg > fb {
                        (1.0 - fr) * v000[k]
                            + (fr - fg) * at(1, 0, 0)[k]
                            + (fg - fb) * at(1, 1, 0)[k]
                            + fb * v111[k]
                    } else if fr > fb {
                        (1.0 - fr) * v000[k]
                            + (fr - fb) * at(1, 0, 0)[k]
                            + (fb - fg) * at(1, 0, 1)[k]
                            + fg * v111[k]
                    } else {
                        (1.0 - fb) * v000[k]
                            + (fb - fr) * at(0, 0, 1)[k]
                            + (fr - fg) * at(1, 0, 1)[k]
                            + fg * v111[k]
                    }
                } else if fb > fg {
                    (1.0 - fb) * v000[k]
                        + (fb - fg) * at(0, 0, 1)[k]
                        + (fg - fr) * at(0, 1, 1)[k]
                        + fr * v111[k]
                } else if fb > fr {
                    (1.0 - fg) * v000[k]
                        + (fg - fb) * at(0, 1, 0)[k]
                        + (fb - fr) * at(0, 1, 1)[k]
                        + fr * v111[k]
                } else {
                    (1.0 - fg) * v000[k]
                        + (fg - fr) * at(0, 1, 0)[k]
                        + (fr - fb) * at(1, 1, 0)[k]
                        + fb * v111[k]
                };
            }
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_round_trip() {
        let srgb = Op::ExpLinear {
            g: [2.4; 3],
            o: [0.055; 3],
            neg: Negative::Linear,
            inverse: false,
        };
        let mut c = [0.5, 0.01, 1.0];
        srgb.apply(&mut c);
        assert!((c[0] - 0.214_041_14).abs() < 1e-6);
        assert!((c[1] - 0.01 / 12.92).abs() < 1e-5);
        let inv = Op::ExpLinear {
            g: [2.4; 3],
            o: [0.055; 3],
            neg: Negative::Linear,
            inverse: true,
        };
        inv.apply(&mut c);
        assert!((c[0] - 0.5).abs() < 1e-9 && (c[1] - 0.01).abs() < 1e-9);
        // The ACES 1.0 tone scales: 18 % grey to 4.8 nits... then the ODT: about 4.8 nits.
        assert!((spline_c5_fwd(0.18) - 4.8).abs() < 1e-6);
        let odt = odt_48();
        let y = spline_c9_fwd(spline_c5_fwd(0.18), &odt);
        assert!((y - 4.8).abs() < 1e-3, "{y}");
        for x in [0.005, 0.05, 0.18, 1.0, 10.0] {
            let y = spline_c9_fwd(spline_c5_fwd(x), &odt);
            let back = spline_c5_inv(spline_c9_inv(y, &odt));
            assert!((back / x - 1.0).abs() < 1e-6, "{x} {back}");
        }
        for v in [0.0f32, 1.0, -2.5, 0.18, 65504.0, 1e-6, 6.1e-5] {
            // Exact half values come back unchanged.
            let h = half_to_f32(half_bits(v));
            assert_eq!(half_bits(h), half_bits(v));
            assert!(
                (half_to_f32(half_bits(v)) - v).abs() <= v.abs() * 1e-3 + 1e-7,
                "{v}"
            );
        }
    }
}
