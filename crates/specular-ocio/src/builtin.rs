//! Built-in transforms of the ACES 1.3 configs, from public definitions: colour-space
//! matrices from the primaries (with Bradford adaptation between white points), the
//! ACEScc and ACEScct curves (S-2014-003, S-2016-001), the ACES 1.0 SDR video output
//! transform (the Academy's RRT and 100-nit dim-surround ODT CTL up to CIE XYZ D65), and
//! the display encodings (sRGB, Display P3, Rec.1886, gamma 2.6).

use crate::ops::{Negative, Op, mat3_inv};
use crate::{Error, Result};

type Xy = (f64, f64);

/// Primaries and white point.
pub struct Gamut {
    pub r: Xy,
    pub g: Xy,
    pub b: Xy,
    pub w: Xy,
}

pub const D60: Xy = (0.32168, 0.33767);
pub const D65: Xy = (0.3127, 0.3290);

pub const AP0: Gamut = Gamut {
    r: (0.7347, 0.2653),
    g: (0.0, 1.0),
    b: (0.0001, -0.0770),
    w: D60,
};
pub const AP1: Gamut = Gamut {
    r: (0.713, 0.293),
    g: (0.165, 0.830),
    b: (0.128, 0.044),
    w: D60,
};
pub const REC709: Gamut = Gamut {
    r: (0.64, 0.33),
    g: (0.30, 0.60),
    b: (0.15, 0.06),
    w: D65,
};
pub const P3_D65: Gamut = Gamut {
    r: (0.680, 0.320),
    g: (0.265, 0.690),
    b: (0.150, 0.060),
    w: D65,
};
pub const REC2020: Gamut = Gamut {
    r: (0.708, 0.292),
    g: (0.170, 0.797),
    b: (0.131, 0.046),
    w: D65,
};

fn xyz_of((x, y): Xy) -> [f64; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

fn mul(a: &[f64; 9], b: &[f64; 9]) -> [f64; 9] {
    let mut o = [0.0; 9];
    for i in 0..3 {
        for j in 0..3 {
            o[i * 3 + j] = (0..3).map(|k| a[i * 3 + k] * b[k * 3 + j]).sum();
        }
    }
    o
}

/// RGB → XYZ of a gamut (its own white).
pub fn rgb_to_xyz(g: &Gamut) -> [f64; 9] {
    let (r, gg, b) = (xyz_of(g.r), xyz_of(g.g), xyz_of(g.b));
    let p = [r[0], gg[0], b[0], r[1], gg[1], b[1], r[2], gg[2], b[2]];
    let w = xyz_of(g.w);
    let pi = mat3_inv(&p).unwrap_or([1., 0., 0., 0., 1., 0., 0., 0., 1.]);
    let s = [
        pi[0] * w[0] + pi[1] * w[1] + pi[2] * w[2],
        pi[3] * w[0] + pi[4] * w[1] + pi[5] * w[2],
        pi[6] * w[0] + pi[7] * w[1] + pi[8] * w[2],
    ];
    [
        p[0] * s[0],
        p[1] * s[1],
        p[2] * s[2],
        p[3] * s[0],
        p[4] * s[1],
        p[5] * s[2],
        p[6] * s[0],
        p[7] * s[1],
        p[8] * s[2],
    ]
}

/// Bradford adaptation from white `src` to `dst` (in XYZ).
pub fn bradford(src: Xy, dst: Xy) -> [f64; 9] {
    const B: [f64; 9] = [
        0.8951, 0.2664, -0.1614, -0.7502, 1.7135, 0.0367, 0.0389, -0.0685, 1.0296,
    ];
    let bi = mat3_inv(&B).unwrap_or(B);
    let (s, d) = (xyz_of(src), xyz_of(dst));
    let ls = [
        B[0] * s[0] + B[1] * s[1] + B[2] * s[2],
        B[3] * s[0] + B[4] * s[1] + B[5] * s[2],
        B[6] * s[0] + B[7] * s[1] + B[8] * s[2],
    ];
    let ld = [
        B[0] * d[0] + B[1] * d[1] + B[2] * d[2],
        B[3] * d[0] + B[4] * d[1] + B[5] * d[2],
        B[6] * d[0] + B[7] * d[1] + B[8] * d[2],
    ];
    let k = [
        ld[0] / ls[0],
        0.0,
        0.0,
        0.0,
        ld[1] / ls[1],
        0.0,
        0.0,
        0.0,
        ld[2] / ls[2],
    ];
    mul(&bi, &mul(&k, &B))
}

/// RGB of `a` → RGB of `b`, Bradford-adapted when their whites differ.
pub fn gamut_to_gamut(a: &Gamut, b: &Gamut) -> [f64; 9] {
    let to = rgb_to_xyz(a);
    let from = mat3_inv(&rgb_to_xyz(b)).unwrap_or([1., 0., 0., 0., 1., 0., 0., 0., 1.]);
    if a.w == b.w {
        mul(&from, &to)
    } else {
        mul(&from, &mul(&bradford(a.w, b.w), &to))
    }
}

fn matrix(m: [f64; 9]) -> Op {
    Op::Matrix {
        m,
        offset: [0.0; 3],
    }
}

/// XYZ (D65) → RGB of a D65 gamut.
fn xyz_to(g: &Gamut) -> [f64; 9] {
    mat3_inv(&rgb_to_xyz(g)).unwrap_or([1., 0., 0., 0., 1., 0., 0., 0., 1.])
}

fn sat_matrix(sat: f64) -> [f64; 9] {
    let y = [0.2722287168, 0.6740817658, 0.0536895174];
    let mut m = [0.0; 9];
    for i in 0..3 {
        for j in 0..3 {
            m[i * 3 + j] = (1.0 - sat) * y[j] + if i == j { sat } else { 0.0 };
        }
    }
    m
}

/// ACEScct ↔ linear (forward: ACEScct → linear) as a camera log.
fn acescct(inverse_of_log: bool) -> Op {
    Op::Log {
        base: 2.0,
        log_slope: [1.0 / 17.52; 3],
        log_offset: [9.72 / 17.52; 3],
        lin_slope: [1.0; 3],
        lin_offset: [0.0; 3],
        lin_break: Some([0.0078125; 3]),
        linear_slope: None,
        inverse: inverse_of_log,
    }
}

/// The ACES 1.0 RRT and SDR video ODT (100 nits, dim surround) from ACES2065-1 to CIE XYZ
/// D65, before the display's primaries and encoding.
fn aces_sdr_video_10() -> Vec<Op> {
    let ap0_ap1 = gamut_to_gamut(&AP0, &AP1);
    let ap1_xyz = rgb_to_xyz(&AP1);
    vec![
        Op::AcesGlow10 { inverse: false },
        Op::AcesRedMod10 { inverse: false },
        Op::Range {
            scale: 1.0,
            offset: 0.0,
            lo: Some(0.0),
            hi: None,
        },
        matrix(ap0_ap1),
        Op::Range {
            scale: 1.0,
            offset: 0.0,
            lo: Some(0.0),
            hi: Some(65504.0),
        },
        matrix(sat_matrix(0.96)),
        Op::AcesToneScale10 { inverse: false },
        Op::AcesYToLinCv {
            white: 48.0,
            black: 0.02,
            inverse: false,
        },
        Op::AcesDarkToDim10 { inverse: false },
        matrix(sat_matrix(0.93)),
        matrix(mul(&bradford(D60, D65), &ap1_xyz)),
    ]
}

fn invert(ops: Vec<Op>) -> Result<Vec<Op>> {
    let mut out = Vec::new();
    for op in ops.into_iter().rev() {
        out.push(match op {
            Op::Matrix { m, offset } => {
                let mi = mat3_inv(&m)
                    .ok_or_else(|| Error::Unsupported("a matrix cannot be inverted".into()))?;
                let o = [
                    -(mi[0] * offset[0] + mi[1] * offset[1] + mi[2] * offset[2]),
                    -(mi[3] * offset[0] + mi[4] * offset[1] + mi[5] * offset[2]),
                    -(mi[6] * offset[0] + mi[7] * offset[1] + mi[8] * offset[2]),
                ];
                Op::Matrix { m: mi, offset: o }
            }
            Op::Exponent { e, neg, inverse } => Op::Exponent {
                e,
                neg,
                inverse: !inverse,
            },
            Op::ExpLinear { g, o, neg, inverse } => Op::ExpLinear {
                g,
                o,
                neg,
                inverse: !inverse,
            },
            Op::Log {
                base,
                log_slope,
                log_offset,
                lin_slope,
                lin_offset,
                lin_break,
                linear_slope,
                inverse,
            } => Op::Log {
                base,
                log_slope,
                log_offset,
                lin_slope,
                lin_offset,
                lin_break,
                linear_slope,
                inverse: !inverse,
            },
            Op::Range { lo, hi, .. } => Op::Range {
                scale: 1.0,
                offset: 0.0,
                lo,
                hi,
            },
            Op::Cdl {
                slope,
                offset,
                power,
                sat,
                clamp,
                inverse,
            } => Op::Cdl {
                slope,
                offset,
                power,
                sat,
                clamp,
                inverse: !inverse,
            },
            Op::Lut1d {
                size,
                values,
                lo,
                hi,
                half_domain,
                inverse,
            } => Op::Lut1d {
                size,
                values,
                lo,
                hi,
                half_domain,
                inverse: !inverse,
            },
            Op::AcesGlow10 { inverse } => Op::AcesGlow10 { inverse: !inverse },
            Op::AcesRedMod10 { inverse } => Op::AcesRedMod10 { inverse: !inverse },
            Op::AcesDarkToDim10 { inverse } => Op::AcesDarkToDim10 { inverse: !inverse },
            Op::AcesToneScale10 { inverse } => Op::AcesToneScale10 { inverse: !inverse },
            Op::AcesYToLinCv {
                white,
                black,
                inverse,
            } => Op::AcesYToLinCv {
                white,
                black,
                inverse: !inverse,
            },
            other => {
                return Err(Error::Unsupported(format!(
                    "the inverse of {other:?} is not supported"
                )));
            }
        });
    }
    Ok(out)
}

/// The operations of a built-in style (forward), or why it is refused.
pub fn ops(style: &str, inverse: bool) -> Result<Vec<Op>> {
    let srgb_inv = Op::ExpLinear {
        g: [2.4; 3],
        o: [0.055; 3],
        neg: Negative::Linear,
        inverse: true,
    };
    let gamma_inv = |g: f64| Op::Exponent {
        e: [g; 3],
        neg: Negative::Clamp,
        inverse: true,
    };
    let fwd = match style.trim() {
        "IDENTITY" | "pass_thru" => Vec::new(),
        "ACEScg_to_ACES2065-1" => vec![matrix(gamut_to_gamut(&AP1, &AP0))],
        "ACEScct_to_ACES2065-1" => vec![acescct(true), matrix(gamut_to_gamut(&AP1, &AP0))],
        "ACEScc_to_ACES2065-1" => vec![
            // ACEScc's log2 segments as a log op, the toe below 2^-15 continued linearly
            // (S-2014-003's lowest segment is close to linear there).
            Op::Log {
                base: 2.0,
                log_slope: [1.0 / 17.52; 3],
                log_offset: [9.72 / 17.52; 3],
                lin_slope: [1.0; 3],
                lin_offset: [0.0; 3],
                lin_break: None,
                linear_slope: None,
                inverse: true,
            },
            matrix(gamut_to_gamut(&AP1, &AP0)),
        ],
        "UTILITY - ACES-AP0_to_CIE-XYZ-D65_BFD" => {
            vec![matrix(mul(&bradford(D60, D65), &rgb_to_xyz(&AP0)))]
        }
        "UTILITY - ACES-AP1_to_CIE-XYZ-D65_BFD" => {
            vec![matrix(mul(&bradford(D60, D65), &rgb_to_xyz(&AP1)))]
        }
        "UTILITY - ACES-AP1_to_LINEAR-REC709_BFD" => vec![matrix(gamut_to_gamut(&AP1, &REC709))],
        "UTILITY - ACES-AP0_to_LINEAR-REC709_BFD" => vec![matrix(gamut_to_gamut(&AP0, &REC709))],
        "DISPLAY - CIE-XYZ-D65_to_sRGB" => vec![matrix(xyz_to(&REC709)), srgb_inv],
        // Display P3 mirrors negative values through the curve (sRGB continues the line).
        "DISPLAY - CIE-XYZ-D65_to_DisplayP3" => vec![
            matrix(xyz_to(&P3_D65)),
            Op::ExpLinear {
                g: [2.4; 3],
                o: [0.055; 3],
                neg: Negative::Mirror,
                inverse: true,
            },
        ],
        "DISPLAY - CIE-XYZ-D65_to_REC.1886-REC.709" => {
            vec![matrix(xyz_to(&REC709)), gamma_inv(2.4)]
        }
        "DISPLAY - CIE-XYZ-D65_to_REC.1886-REC.2020" => {
            vec![matrix(xyz_to(&REC2020)), gamma_inv(2.4)]
        }
        "DISPLAY - CIE-XYZ-D65_to_G2.2-REC.709" => vec![matrix(xyz_to(&REC709)), gamma_inv(2.2)],
        "DISPLAY - CIE-XYZ-D65_to_G2.6-P3-D65" => vec![matrix(xyz_to(&P3_D65)), gamma_inv(2.6)],
        "ACES-OUTPUT - ACES2065-1_to_CIE-XYZ-D65 - SDR-VIDEO_1.0" => aces_sdr_video_10(),
        s if s.starts_with("ACES-OUTPUT") && (s.contains("HDR") || s.contains("_2.")) => {
            return Err(Error::Unsupported(format!(
                "the output transform {s} is not supported yet (only ACES 1.0 SDR video)"
            )));
        }
        s => {
            return Err(Error::Unsupported(format!(
                "the built-in transform {s} is not supported"
            )));
        }
    };
    if inverse { invert(fwd) } else { Ok(fwd) }
}

/// Inverts a list of operations (for inverse-direction transforms).
pub fn inverted(ops: Vec<Op>) -> Result<Vec<Op>> {
    invert(ops)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrices() {
        // AP1 → AP0 as published (S-2014-004).
        let m = gamut_to_gamut(&AP1, &AP0);
        let want = [
            0.6954522414,
            0.1406786965,
            0.1638690622,
            0.0447945634,
            0.8596711185,
            0.0955343182,
            -0.0055258826,
            0.0040252103,
            1.0015006723,
        ];
        for (a, b) in m.iter().zip(want) {
            assert!((a - b).abs() < 2e-6, "{m:?}");
        }
        // Rec.709 → XYZ, D65.
        let x = rgb_to_xyz(&REC709);
        assert!(
            (x[0] - 0.412_390_8).abs() < 1e-6 && (x[4] - 0.715_168_7).abs() < 1e-6,
            "{x:?}"
        );
    }
}
