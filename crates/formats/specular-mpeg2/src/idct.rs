//! The 8×8 inverse DCT: the integer Loeffler–Ligtenberg–Moschytz factorisation with 13-bit
//! constants (IEEE 1180 accurate), rows first and then columns, two extra bits kept between
//! the passes, 16-bit intermediates.
//!
//! The IJG `jrevdct` libavcodec uses for `-idct int` takes shortcuts for sparse inputs whose
//! combined constants are rounded on their own, so for some zero patterns its result is not
//! the general transform's. Those patterns are modelled as corrections to the general
//! coefficients ([`ODD_FIX`], and the even part's x2 = 0 case), so output matches it exactly.

const CONST_BITS: i32 = 13;
const PASS1_BITS: i32 = 2;

const C0_298: i32 = 2446;
const C0_390: i32 = 3196;
const C0_541: i32 = 4433;
const C0_765: i32 = 6270;
const C0_899: i32 = 7373;
const C1_175: i32 = 9633;
const C1_501: i32 = 12299;
const C1_847: i32 = 15137;
const C1_961: i32 = 16069;
const C2_053: i32 = 16819;
const C2_562: i32 = 20995;
const C3_072: i32 = 25172;

#[inline(always)]
fn descale(x: i32, n: i32) -> i32 {
    (x + (1 << (n - 1))) >> n
}

/// Corrections to the odd part's coefficients for sparse inputs: indexed by which of
/// x1, x3, x5, x7 are non-zero (bits 0..4), the change to each output b0..b3 per input
/// (x1, x3, x5, x7). With three or four non-zero inputs there is none.
const ODD_FIX: [[[i32; 4]; 4]; 16] = {
    let mut t = [[[0; 4]; 4]; 16];
    // x1 only, x3 only, x5 only.
    t[1][1][0] = -1;
    t[2][1][1] = -1;
    t[2][2][1] = -1;
    t[4][1][2] = -1;
    t[4][2][2] = -1;
    t[4][3][2] = -1;
    // x1 and x3.
    t[3][0][0] = 1;
    t[3][1][0] = -1;
    t[3][1][1] = -1;
    t[3][2][1] = -1;
    // x1 and x5.
    t[5][1][2] = -1;
    t[5][1][0] = -1;
    t[5][3][0] = -1;
    t[5][3][2] = -1;
    // x3 and x5.
    t[6][1][2] = -1;
    t[6][1][1] = -1;
    t[6][2][1] = -2;
    t[6][2][2] = -1;
    // x1 and x7.
    t[9][3][0] = -1;
    // x3 and x7.
    t[10][0][3] = 1;
    t[10][2][1] = -1;
    // x5 and x7.
    t[12][0][3] = 1;
    t
};

/// One 8-point pass over `x[0], x[s], .., x[7s]`, scaled by 2^CONST_BITS.
#[inline(always)]
fn pass(x: [i32; 8]) -> [i32; 8] {
    // Even part.
    let z1 = (x[2] + x[6]) * C0_541;
    // With x2 = 0 the rotation's constant is rounded on its own (one more).
    let t2 = z1 - x[6] * C1_847 + if x[2] == 0 { x[6] } else { 0 };
    let t3 = z1 + x[2] * C0_765;
    let t0 = (x[0] + x[4]) << CONST_BITS;
    let t1 = (x[0] - x[4]) << CONST_BITS;
    let (t10, t13, t11, t12) = (t0 + t3, t0 - t3, t1 + t2, t1 - t2);
    // Odd part.
    let (a0, a1, a2, a3) = (x[7], x[5], x[3], x[1]);
    let z1 = a0 + a3;
    let z2 = a1 + a2;
    let z3 = a0 + a2;
    let z4 = a1 + a3;
    let z5 = (z3 + z4) * C1_175;
    let z1 = -z1 * C0_899;
    let z2 = -z2 * C2_562;
    let z3 = -z3 * C1_961 + z5;
    let z4 = -z4 * C0_390 + z5;
    let b0 = a0 * C0_298 + z1 + z3;
    let b1 = a1 * C2_053 + z2 + z4;
    let b2 = a2 * C3_072 + z2 + z3;
    let b3 = a3 * C1_501 + z1 + z4;
    let pattern = usize::from(a3 != 0)
        | usize::from(a2 != 0) << 1
        | usize::from(a1 != 0) << 2
        | usize::from(a0 != 0) << 3;
    let (b0, b1, b2, b3) = if pattern.count_ones() >= 3 || pattern == 8 || pattern == 0 {
        (b0, b1, b2, b3)
    } else {
        let f = &ODD_FIX[pattern];
        let x = [a3, a2, a1, a0];
        let d = |i: usize| f[i][0] * x[0] + f[i][1] * x[1] + f[i][2] * x[2] + f[i][3] * x[3];
        (b0 + d(0), b1 + d(1), b2 + d(2), b3 + d(3))
    };
    [
        t10 + b3,
        t11 + b2,
        t12 + b1,
        t13 + b0,
        t13 - b0,
        t12 - b1,
        t11 - b2,
        t10 - b3,
    ]
}

/// Inverse transform of `blk` (raster order, row = vertical frequency) in place: the residual
/// (or intra samples before clamping).
pub fn idct(blk: &mut [i32; 64]) {
    for r in 0..8 {
        let row = &mut blk[r * 8..r * 8 + 8];
        if row[1..].iter().all(|&c| c == 0) {
            let dc = i32::from((row[0] << PASS1_BITS) as i16);
            row.fill(dc);
            continue;
        }
        let o = pass([
            row[0], row[1], row[2], row[3], row[4], row[5], row[6], row[7],
        ]);
        for (d, v) in row.iter_mut().zip(o) {
            *d = i32::from(descale(v, CONST_BITS - PASS1_BITS) as i16);
        }
    }
    for c in 0..8 {
        let col = [
            blk[c],
            blk[8 + c],
            blk[16 + c],
            blk[24 + c],
            blk[32 + c],
            blk[40 + c],
            blk[48 + c],
            blk[56 + c],
        ];
        let o = pass(col);
        for (r, v) in o.into_iter().enumerate() {
            blk[r * 8 + c] = i32::from(descale(v, CONST_BITS + PASS1_BITS + 3) as i16);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against the double-precision definition: within IEEE 1180's peak error of 1.
    #[test]
    fn close_to_the_exact_transform() {
        let mut seed = 12345u32;
        let mut rnd = || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (seed >> 16) as i32
        };
        let mut worst = 0;
        for _ in 0..2000 {
            let mut b = [0i32; 64];
            for v in b.iter_mut() {
                *v = rnd() % 512 - 256;
            }
            let mut want = [0f64; 64];
            for (y, w) in want.chunks_mut(8).enumerate() {
                for (x, o) in w.iter_mut().enumerate() {
                    let mut s = 0.0;
                    for v in 0..8 {
                        for u in 0..8 {
                            let cu = if u == 0 { 0.5f64.sqrt() } else { 1.0 };
                            let cv = if v == 0 { 0.5f64.sqrt() } else { 1.0 };
                            s += cu
                                * cv
                                * f64::from(b[v * 8 + u])
                                * ((2 * x + 1) as f64 * u as f64 * std::f64::consts::PI / 16.0)
                                    .cos()
                                * ((2 * y + 1) as f64 * v as f64 * std::f64::consts::PI / 16.0)
                                    .cos();
                        }
                    }
                    *o = s / 4.0;
                }
            }
            idct(&mut b);
            for i in 0..64 {
                worst = worst.max((f64::from(b[i]) - want[i].round()).abs() as i32);
            }
        }
        assert!(worst <= 1, "peak error {worst}");
    }

    /// Blocks through IJG's `jrevdct` (a harness's output: 64 inputs, 64 outputs per line),
    /// when `SPECULAR_JREVDCT_BLOCKS` names such a file.
    #[test]
    fn matches_jrevdct() {
        let Some(p) = std::env::var_os("SPECULAR_JREVDCT_BLOCKS") else {
            return;
        };
        let text = std::fs::read_to_string(p).unwrap();
        let mut bad = 0;
        for (n, l) in text.lines().enumerate() {
            let v: Vec<i32> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            let mut b = [0i32; 64];
            b.copy_from_slice(&v[..64]);
            let input = b;
            idct(&mut b);
            if b[..] != v[64..128] {
                bad += 1;
                if bad < 4 {
                    let nz: Vec<(usize, i32)> = input
                        .iter()
                        .copied()
                        .enumerate()
                        .filter(|x| x.1 != 0)
                        .collect();
                    eprintln!("block {n}: input {nz:?}");
                    eprintln!("  want {:?}\n  got  {:?}", &v[64..128], b);
                }
            }
        }
        assert_eq!(bad, 0, "{bad} blocks differ");
    }
}
