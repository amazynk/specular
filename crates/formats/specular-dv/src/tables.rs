//! Scan orders, area numbers, quantisation steps and weights of SMPTE 314M clause 5
//! (figures 27 and 28, table 23, 5.2.2), and the inverse DCTs of 5.2.1.

use std::sync::OnceLock;

/// Figure 27, 8-8 DCT: the 1-based output order of each coefficient, row v (vertical) by
/// column h. (It is the JPEG zigzag.)
const ORDER_88: [[u8; 8]; 8] = [
    [1, 2, 6, 7, 15, 16, 28, 29],
    [3, 5, 8, 14, 17, 27, 30, 43],
    [4, 9, 13, 18, 26, 31, 42, 44],
    [10, 12, 19, 25, 32, 41, 45, 54],
    [11, 20, 24, 33, 40, 46, 53, 55],
    [21, 23, 34, 39, 47, 52, 56, 61],
    [22, 35, 38, 48, 51, 57, 60, 62],
    [36, 37, 49, 50, 58, 59, 63, 64],
];

/// Figure 27, 2-4-8 DCT: rows 0..3 are the field sum, rows 4..7 the difference.
const ORDER_248: [[u8; 8]; 8] = [
    [1, 3, 7, 19, 21, 35, 37, 51],
    [5, 9, 17, 23, 33, 39, 49, 53],
    [11, 15, 25, 31, 41, 47, 55, 61],
    [13, 27, 29, 43, 45, 57, 59, 63],
    [2, 4, 8, 20, 22, 36, 38, 52],
    [6, 10, 18, 24, 34, 40, 50, 54],
    [12, 16, 26, 32, 42, 48, 56, 62],
    [14, 28, 30, 44, 46, 58, 60, 64],
];

/// Figure 28: area numbers, same layout (the DC entry is unused).
const AREA_88: [[u8; 8]; 8] = [
    [0, 0, 0, 1, 1, 1, 2, 2],
    [0, 0, 1, 1, 1, 2, 2, 2],
    [0, 1, 1, 1, 2, 2, 2, 3],
    [1, 1, 1, 2, 2, 2, 3, 3],
    [1, 1, 2, 2, 2, 3, 3, 3],
    [1, 2, 2, 2, 3, 3, 3, 3],
    [2, 2, 2, 3, 3, 3, 3, 3],
    [2, 2, 3, 3, 3, 3, 3, 3],
];
const AREA_248: [[u8; 8]; 8] = [
    [0, 0, 1, 1, 1, 2, 2, 3],
    [0, 1, 1, 2, 2, 2, 3, 3],
    [1, 1, 2, 2, 2, 3, 3, 3],
    [1, 2, 2, 2, 3, 3, 3, 3],
    [0, 0, 1, 1, 2, 2, 2, 3],
    [0, 1, 1, 2, 2, 2, 3, 3],
    [1, 1, 2, 2, 2, 3, 3, 3],
    [1, 2, 2, 3, 3, 3, 3, 3],
];

/// Table 23: the quantisation step of each area (columns) per row; a class's row for QNO q
/// is `ROW_OFFSET[class] - q`.
const STEPS: [[u8; 4]; 22] = [
    [1, 1, 1, 1],
    [1, 1, 1, 1],
    [1, 1, 1, 1],
    [1, 1, 1, 1],
    [1, 1, 1, 1],
    [1, 1, 1, 1],
    [1, 1, 1, 1],
    [1, 1, 1, 2],
    [1, 1, 2, 2],
    [1, 1, 2, 2],
    [1, 2, 2, 4],
    [1, 2, 2, 4],
    [2, 2, 4, 4],
    [2, 2, 4, 4],
    [2, 4, 4, 8],
    [2, 4, 4, 8],
    [4, 4, 8, 8],
    [4, 4, 8, 8],
    [4, 8, 8, 16],
    [4, 8, 8, 16],
    [8, 8, 16, 16],
    [8, 8, 16, 16],
];
const ROW_OFFSET: [u8; 4] = [15, 18, 21, 20];

/// Per DCT mode: for scan index 0..64, the raster index (v * 8 + h) and the area number.
pub(crate) struct Scan {
    pub raster: [u8; 64],
    pub area: [u8; 64],
}

pub(crate) fn scans() -> &'static [Scan; 2] {
    static S: OnceLock<[Scan; 2]> = OnceLock::new();
    S.get_or_init(|| {
        let build = |order: &[[u8; 8]; 8], area: &[[u8; 8]; 8]| {
            let mut s = Scan {
                raster: [0; 64],
                area: [0; 64],
            };
            for v in 0..8 {
                for h in 0..8 {
                    let i = usize::from(order[v][h] - 1);
                    s.raster[i] = (v * 8 + h) as u8;
                    s.area[i] = area[v][h];
                }
            }
            s
        };
        [build(&ORDER_88, &AREA_88), build(&ORDER_248, &AREA_248)]
    })
}

/// The quantisation step of `area` for a block of `class` in a macro block with `qno`,
/// times two for class 3 (whose initial scaling dropped the lowest bit).
pub(crate) fn step(class: u8, area: u8, qno: u8) -> i32 {
    let row = usize::from(ROW_OFFSET[usize::from(class & 3)] - (qno & 15));
    let s = i32::from(STEPS[row][usize::from(area & 3)]);
    if class == 3 { s * 2 } else { s }
}

/// 5.2.2: the inverse of the weight W(h, v) per mode, raster order.
pub(crate) fn inverse_weights() -> &'static [[f32; 64]; 2] {
    static W: OnceLock<[[f32; 64]; 2]> = OnceLock::new();
    W.get_or_init(|| {
        let cs = |m: f64| (m * std::f64::consts::PI / 16.0).cos();
        let w = [
            1.0,
            cs(4.0) / (4.0 * cs(7.0) * cs(2.0)),
            cs(4.0) / (2.0 * cs(6.0)),
            1.0 / (2.0 * cs(5.0)),
            7.0 / 8.0,
            cs(4.0) / cs(3.0),
            cs(4.0) / cs(2.0),
            cs(4.0) / cs(1.0),
        ];
        let mut out = [[0f32; 64]; 2];
        for v in 0..8 {
            for h in 0..8 {
                let w88 = if h == 0 && v == 0 {
                    0.25
                } else {
                    w[h] * w[v] / 2.0
                };
                let w248 = if h == 0 && v == 0 {
                    0.25
                } else if v < 4 {
                    w[h] * w[2 * v] / 2.0
                } else {
                    w[h] * w[2 * (v - 4)] / 2.0
                };
                out[0][v * 8 + h] = (1.0 / w88) as f32;
                out[1][v * 8 + h] = (1.0 / w248) as f32;
            }
        }
        out
    })
}

/// Cosine bases: `c8[k][n] = C(k) cos(πk(2n+1)/16)` and `c4[u][z] = C(u) cos(πu(2z+1)/8)`
/// with C(0) = 0.5/√2 and C(k) = 0.5 otherwise (5.2.1).
struct Basis {
    c8: [[f32; 8]; 8],
    c4: [[f32; 4]; 4],
}

fn basis() -> &'static Basis {
    static B: OnceLock<Basis> = OnceLock::new();
    B.get_or_init(|| {
        let c = |k: usize| if k == 0 { 0.5 / 2f64.sqrt() } else { 0.5 };
        let pi = std::f64::consts::PI;
        let mut b = Basis {
            c8: [[0.0; 8]; 8],
            c4: [[0.0; 4]; 4],
        };
        for k in 0..8 {
            for n in 0..8 {
                b.c8[k][n] = (c(k) * (pi * k as f64 * (2 * n + 1) as f64 / 16.0).cos()) as f32;
            }
        }
        for u in 0..4 {
            for z in 0..4 {
                b.c4[u][z] = (c(u) * (pi * u as f64 * (2 * z + 1) as f64 / 8.0).cos()) as f32;
            }
        }
        b
    })
}

/// Inverse DCT of one block's coefficients (raster order, v * 8 + h) into 8×8 samples with
/// the 128 offset added and clipped to 0..=255. `mode248` selects the 2-4-8 transform.
pub(crate) fn idct(coef: &[f32; 64], mode248: bool, out: &mut [u8; 64]) {
    let b = basis();
    // Horizontal pass: r[v][x] = Σh coef[v][h] c8[h][x].
    let mut r = [[0f32; 8]; 8];
    for v in 0..8 {
        let row = &coef[v * 8..v * 8 + 8];
        if row.iter().all(|&c| c == 0.0) {
            continue;
        }
        for (h, &c) in row.iter().enumerate() {
            if c != 0.0 {
                for x in 0..8 {
                    r[v][x] += c * b.c8[h][x];
                }
            }
        }
    }
    let mut p = [[0f32; 8]; 8];
    if mode248 {
        for z in 0..4 {
            for x in 0..8 {
                let mut s = 0.0;
                let mut d = 0.0;
                for u in 0..4 {
                    s += r[u][x] * b.c4[u][z];
                    d += r[u + 4][x] * b.c4[u][z];
                }
                p[2 * z][x] = s + d;
                p[2 * z + 1][x] = s - d;
            }
        }
    } else {
        for y in 0..8 {
            for x in 0..8 {
                let mut s = 0.0;
                for v in 0..8 {
                    s += r[v][x] * b.c8[v][y];
                }
                p[y][x] = s;
            }
        }
    }
    for y in 0..8 {
        for x in 0..8 {
            out[y * 8 + x] = (p[y][x] + 128.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_cover_every_coefficient() {
        for s in scans() {
            let mut seen = [false; 64];
            for &r in &s.raster {
                assert!(!seen[usize::from(r)]);
                seen[usize::from(r)] = true;
            }
            assert_eq!(s.raster[0], 0);
        }
    }

    #[test]
    fn steps_follow_table_23() {
        assert_eq!(step(0, 0, 15), 1);
        assert_eq!(step(0, 3, 0), 8);
        assert_eq!(step(1, 3, 0), 16);
        assert_eq!(step(2, 0, 0), 8);
        assert_eq!(step(3, 3, 0), 32);
        assert_eq!(step(3, 0, 15), 2);
        assert_eq!(step(2, 3, 9), 4);
    }

    #[test]
    fn dc_only_blocks_are_flat() {
        // A weighted DC of d gives samples of d / 2 around 128 in both modes.
        for mode in [false, true] {
            let mut c = [0f32; 64];
            c[0] = 40.0 * inverse_weights()[usize::from(mode)][0];
            let mut out = [0u8; 64];
            idct(&c, mode, &mut out);
            assert!(out.iter().all(|&v| v == 148), "{out:?}");
        }
    }
}
