//! Inverse transforms (8.7.4): DCT-II, DST-VII and DCT-VIII up to 64 points with zero-out,
//! and the low-frequency non-separable transform. Blocks are row-major `[y * w + x]`.

use crate::tables::{
    DCT2_64, DCT8_4, DCT8_8, DCT8_16, DCT8_32, DST7_4, DST7_8, DST7_16, DST7_32, LFNST16, LFNST48,
};

pub const COEFF_MIN: i32 = -(1 << 15);
pub const COEFF_MAX: i32 = (1 << 15) - 1;

/// Coefficient `j` of basis... `transMatrix[ sample ][ coef ]` of kernel `tr` for size `n`.
#[inline]
fn kernel(tr: u8, n: usize, coef: usize, sample: usize) -> i32 {
    match tr {
        0 => i32::from(DCT2_64[coef * (64 / n)][sample]),
        1 => i32::from(match n {
            4 => DST7_4[coef][sample],
            8 => DST7_8[coef][sample],
            16 => DST7_16[coef][sample],
            _ => DST7_32[coef][sample],
        }),
        _ => i32::from(match n {
            4 => DCT8_4[coef][sample],
            8 => DCT8_8[coef][sample],
            16 => DCT8_16[coef][sample],
            _ => DCT8_32[coef][sample],
        }),
    }
}

/// A transform kernel as a dense `[coef][sample]` matrix for size `n` (cached; the first
/// `nz` rows are the ones used).
fn matrix(tr: u8, n: usize, nz: usize) -> &'static [i32] {
    use std::sync::OnceLock;
    static T: OnceLock<Vec<Vec<i32>>> = OnceLock::new();
    let t = T.get_or_init(|| {
        let mut v = Vec::new();
        for tr in 0..3u8 {
            for l in 0..7 {
                let n = 1usize << l;
                let ok = if tr == 0 {
                    n >= 2
                } else {
                    (4..=32).contains(&n)
                };
                let rows = if tr == 0 { n } else { n.min(16) };
                let mut m = vec![0i32; rows * n];
                if ok {
                    for c in 0..rows {
                        for s in 0..n {
                            m[c * n + s] = kernel(tr, n, c, s);
                        }
                    }
                }
                v.push(m);
            }
        }
        v
    });
    let m = &t[usize::from(tr) * 7 + n.trailing_zeros() as usize];
    &m[..nz * n]
}

/// Transformation process (8.7.4.1 steps 1-6) of an `w x h` block of scaled coefficients `d`
/// (row-major) with kernels `tr_hor` / `tr_ver` and non-zero region `nz_w x nz_h`, into
/// residuals `res`.
#[allow(clippy::too_many_arguments)]
pub fn inverse(
    d: &[i32],
    w: usize,
    h: usize,
    tr_hor: u8,
    tr_ver: u8,
    nz_w: usize,
    nz_h: usize,
    bit_depth: u32,
    res: &mut [i32],
) {
    let log2_range = 15;
    // g[x][y] for x < nz_w, y < h, stored [y * nz_w + x].
    let mut g = vec![0i32; nz_w * h];
    if h > 1 {
        let mv = matrix(tr_ver, h, nz_h);
        for x in 0..nz_w {
            for y in 0..h {
                let mut s = 0i32;
                for j in 0..nz_h {
                    let c = d[j * w + x];
                    if c != 0 {
                        s += mv[j * h + y] * c;
                    }
                }
                g[y * nz_w + x] = if w > 1 {
                    ((s + 64) >> 7).clamp(COEFF_MIN, COEFF_MAX)
                } else {
                    s
                };
            }
        }
    } else {
        g[..nz_w].copy_from_slice(&d[..nz_w]);
    }
    let shift = if h > 1 && w > 1 {
        5 + log2_range - bit_depth
    } else {
        6 + log2_range - bit_depth
    };
    let rnd = 1i32 << (shift - 1);
    if w > 1 {
        let mh = matrix(tr_hor, w, nz_w);
        for y in 0..h {
            let row = &g[y * nz_w..(y + 1) * nz_w];
            let out = &mut res[y * w..(y + 1) * w];
            for (x, o) in out.iter_mut().enumerate() {
                let mut s = 0i32;
                for (j, &c) in row.iter().enumerate() {
                    if c != 0 {
                        s += mh[j * w + x] * c;
                    }
                }
                *o = (s + rnd) >> shift;
            }
        }
    } else {
        for y in 0..h {
            res[y * w] = (g[y * nz_w] + rnd) >> shift;
        }
    }
}

/// lfnstTrSetIdx from the (wide-angle mapped) intra mode (Table 41).
pub fn lfnst_set(mode: i32) -> usize {
    match mode {
        m if m < 0 => 1,
        0..=1 => 0,
        2..=12 => 1,
        13..=23 => 2,
        24..=44 => 3,
        45..=55 => 2,
        _ => 1,
    }
}

/// Applies the LFNST (8.7.4.1, 8.7.4.2) in place to the top-left coefficients of `d`
/// (row-major, width `w`), for a `w x h` block, `idx` = lfnst_idx, `mode` the mapped mode.
pub fn lfnst(d: &mut [i32], w: usize, h: usize, idx: u32, mode: i32) {
    let big = w >= 8 && h >= 8;
    let n_out = if big { 48 } else { 16 };
    let log2 = if big { 3 } else { 2 };
    let nz = if (w == 4 && h == 4) || (w == 8 && h == 8) {
        8
    } else {
        16
    };
    let set = lfnst_set(mode);
    let k = (idx - 1) as usize;
    // u[ x ] from the 4x4 diagonal scan.
    let mut u = [0i32; 16];
    const SCAN4: [(usize, usize); 16] = [
        (0, 0),
        (0, 1),
        (1, 0),
        (0, 2),
        (1, 1),
        (2, 0),
        (0, 3),
        (1, 2),
        (2, 1),
        (3, 0),
        (1, 3),
        (2, 2),
        (3, 1),
        (2, 3),
        (3, 2),
        (3, 3),
    ];
    for (i, &(x, y)) in SCAN4.iter().enumerate().take(nz) {
        u[i] = d[y * w + x];
    }
    let mut v = [0i32; 48];
    for (i, vi) in v.iter_mut().enumerate().take(n_out) {
        let mut s = 0i32;
        for (j, &uj) in u.iter().enumerate().take(nz) {
            if uj != 0 {
                let m = if big {
                    i32::from(LFNST48[set][k][j][i])
                } else {
                    i32::from(LFNST16[set][k][j][i])
                };
                s += m * uj;
            }
        }
        *vi = ((s + 64) >> 7).clamp(COEFF_MIN, COEFF_MAX);
    }
    let n = 1usize << log2;
    for y in 0..n {
        for x in 0..n {
            let val = if mode <= 34 {
                if y < 4 {
                    Some(v[x + (y << log2)])
                } else if x < 4 {
                    Some(v[32 + x + ((y - 4) << 2)])
                } else {
                    None
                }
            } else if x < 4 {
                Some(v[y + (x << log2)])
            } else if y < 4 {
                Some(v[32 + y + ((x - 4) << 2)])
            } else {
                None
            };
            if let Some(val) = val {
                d[y * w + x] = val;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dc_only_dct_is_flat() {
        // A lone DC coefficient: 64 * 64 * c >> 7 then >> (20 - bitDepth) for 4x4 at 10 bits.
        let mut d = vec![0i32; 16];
        d[0] = 256;
        let mut r = vec![0i32; 16];
        inverse(&d, 4, 4, 0, 0, 4, 4, 10, &mut r);
        let g = (64 * 256 + 64) >> 7;
        let want = (64 * g + (1 << 9)) >> 10;
        assert!(r.iter().all(|&v| v == want), "{r:?}");
    }

    #[test]
    fn tables_have_expected_corners() {
        assert_eq!(DCT2_64[0][0], 64);
        assert_eq!(DCT2_64[1][0], 91);
        assert_eq!(DCT2_64[1][63], -91);
        assert_eq!(DST7_4[0], [29, 55, 74, 84]);
        assert_eq!(LFNST16[0][0][0][0], 108);
        assert_eq!(lfnst_set(-3), 1);
        assert_eq!(lfnst_set(30), 3);
    }
}
