//! The 8×8 inverse DCT and dequantisation of a block (RDD 36 §7.4, §7.5).
//!
//! The transform is the even/odd (Chen) factorisation, rows first: the cosines
//! `√2·cos(k·π/16)` as fixed-point constants, 16-bit intermediates between the passes, the
//! mid-grey offset added to the DC term of each column before the column pass. RDD 36 sets
//! an accuracy bound, not the arithmetic. The constants and shifts here were identified
//! against FFmpeg's decoder output (crafted single-coefficient frames, then random blocks), so
//! frames compare sample for sample: at 10 bits `W3` is 19265 and `W4` exactly 2^14; at 12
//! bits `W4` is 2^15 − 1.

/// The fixed-point arithmetic of one output depth.
#[derive(Clone, Copy)]
pub struct Arith {
    /// W1..W7 (index 0 unused).
    w: [i32; 8],
    row_shift: u32,
    col_shift: u32,
    /// The output range: values clip to `[lo, hi]` (the codes RDD 36 reserves are outside).
    lo: i32,
    hi: i32,
}

impl Arith {
    /// 10-bit output (the 422 profiles).
    pub const B10: Arith = Arith {
        w: [0, 22725, 21407, 19265, 16384, 12873, 8867, 4520],
        row_shift: 15,
        col_shift: 18,
        lo: 4,
        hi: 1019,
    };
    /// 12-bit output (4444 and 4444 XQ).
    pub const B12: Arith = Arith {
        w: [0, 45451, 42813, 38531, 32767, 25746, 17734, 9041],
        row_shift: 16,
        col_shift: 17,
        lo: 4,
        hi: 4091,
    };
}

use std::simd::cmp::{SimdOrd, SimdPartialEq};
use std::simd::num::SimdInt;
use std::simd::{Select, Simd};

/// The mid-grey offset, added to the DC row between the passes.
const BIAS: i32 = 8192;

type V = Simd<i32, 8>;

/// Dequantises `block` by `qmat` (the weight matrix times the quantiser scale), both
/// *transposed* (`[column * 8 + row]`), transforms it and writes the 8×8 clipped samples to
/// `out` (8 per row). The rows are transformed eight at a time, one per lane.
#[inline]
pub fn put(block: &[i32; 64], qmat: &[i32; 64], a: &Arith, out: &mut [u16; 64]) {
    let w = a.w.map(V::splat);
    let i16v = |v: V| (v << 16) >> 16;
    // v[k]: coefficient k of every row.
    let v: [V; 8] = std::array::from_fn(|k| {
        let c = V::from_slice(&block[k * 8..k * 8 + 8]);
        let q = V::from_slice(&qmat[k * 8..k * 8 + 8]);
        i16v(c * q)
    });
    let rs = V::splat(a.row_shift as i32);
    let mut a0 = w[4] * v[0] + V::splat(1 << (a.row_shift - 1));
    let mut a1 = a0;
    let mut a2 = a0;
    let mut a3 = a0;
    a0 += w[2] * v[2];
    a1 += w[6] * v[2];
    a2 -= w[6] * v[2];
    a3 -= w[2] * v[2];
    let b0 = w[1] * v[1] + w[3] * v[3] + w[5] * v[5] + w[7] * v[7];
    let b1 = w[3] * v[1] - w[7] * v[3] - w[1] * v[5] - w[5] * v[7];
    let b2 = w[5] * v[1] - w[1] * v[3] + w[7] * v[5] + w[3] * v[7];
    let b3 = w[7] * v[1] - w[5] * v[3] + w[3] * v[5] - w[1] * v[7];
    a0 += w[4] * v[4] + w[6] * v[6];
    a1 += -(w[4] * v[4]) - w[2] * v[6];
    a2 += -(w[4] * v[4]) + w[2] * v[6];
    a3 += w[4] * v[4] - w[6] * v[6];
    let o = |x: V| i16v(x >> rs);
    let mut m = [
        o(a0 + b0),
        o(a1 + b1),
        o(a2 + b2),
        o(a3 + b3),
        o(a3 - b3),
        o(a2 - b2),
        o(a1 - b1),
        o(a0 - b0),
    ];
    // Rows with only a DC term take the shortcut (it rounds differently at 12 bits).
    let ac = v[1] | v[2] | v[3] | v[4] | v[5] | v[6] | v[7];
    let dc_only = ac.simd_eq(V::splat(0));
    let dc = i16v((v[0] + V::splat(1)) >> V::splat(1));
    for x in &mut m {
        *x = dc_only.select(dc, *x);
    }
    // m[x] holds output column x of every row; the columns pass wants rows.
    let c = transpose(m);
    let c0 = i16v(c[0] + V::splat(BIAS));
    let cs = V::splat(a.col_shift as i32);
    let mut a0 = w[4] * (c0 + V::splat((1 << (a.col_shift - 1)) / a.w[4]));
    let mut a1 = a0;
    let mut a2 = a0;
    let mut a3 = a0;
    a0 += w[2] * c[2];
    a1 += w[6] * c[2];
    a2 -= w[6] * c[2];
    a3 -= w[2] * c[2];
    let b0 = w[1] * c[1] + w[3] * c[3] + w[5] * c[5] + w[7] * c[7];
    let b1 = w[3] * c[1] - w[7] * c[3] - w[1] * c[5] - w[5] * c[7];
    let b2 = w[5] * c[1] - w[1] * c[3] + w[7] * c[5] + w[3] * c[7];
    let b3 = w[7] * c[1] - w[5] * c[3] + w[3] * c[5] - w[1] * c[7];
    a0 += w[4] * c[4] + w[6] * c[6];
    a1 += -(w[4] * c[4]) - w[2] * c[6];
    a2 += -(w[4] * c[4]) + w[2] * c[6];
    a3 += w[4] * c[4] - w[6] * c[6];
    let (lo, hi) = (V::splat(a.lo), V::splat(a.hi));
    let o = |x: V| i16v(x >> cs).simd_clamp(lo, hi).cast::<u16>();
    let rows = [
        o(a0 + b0),
        o(a1 + b1),
        o(a2 + b2),
        o(a3 + b3),
        o(a3 - b3),
        o(a2 - b2),
        o(a1 - b1),
        o(a0 - b0),
    ];
    for (dst, r) in out.chunks_exact_mut(8).zip(rows) {
        r.copy_to_slice(dst);
    }
}

/// Transposes an 8×8 matrix held as eight row vectors (three rounds of perfect shuffles).
#[inline(always)]
fn transpose(mut r: [V; 8]) -> [V; 8] {
    for _ in 0..3 {
        let mut n = r;
        for i in 0..4 {
            let (x, y) = r[i].interleave(r[i + 4]);
            n[2 * i] = x;
            n[2 * i + 1] = y;
        }
        r = n;
    }
    r
}

/// The scalar form of [`put`] on a block in raster order: the reference the vector form is
/// tested against.
#[cfg(test)]
pub fn put_scalar(block: &mut [i32; 64], qmat: &[i32; 64], a: &Arith, out: &mut [u16; 64]) {
    for (c, q) in block.iter_mut().zip(qmat) {
        *c = i32::from((*c * *q) as i16);
    }
    for r in block.chunks_exact_mut(8) {
        row(r, a);
    }
    for c in block.iter_mut().take(8) {
        *c = i32::from((*c + BIAS) as i16);
    }
    for c in 0..8 {
        col(block, c, a, out);
    }
}

#[cfg(test)]
#[inline(always)]
fn row(r: &mut [i32], a: &Arith) {
    let w = &a.w;
    let s = a.row_shift;
    if r[1..].iter().all(|&v| v == 0) {
        // W4 is (about) half the shift's unit at both depths.
        let v = i32::from(((r[0] + 1) >> 1) as i16);
        r.fill(v);
        return;
    }
    let mut a0 = w[4] * r[0] + (1 << (s - 1));
    let mut a1 = a0;
    let mut a2 = a0;
    let mut a3 = a0;
    a0 += w[2] * r[2];
    a1 += w[6] * r[2];
    a2 -= w[6] * r[2];
    a3 -= w[2] * r[2];
    let mut b0 = w[1] * r[1] + w[3] * r[3];
    let mut b1 = w[3] * r[1] - w[7] * r[3];
    let mut b2 = w[5] * r[1] - w[1] * r[3];
    let mut b3 = w[7] * r[1] - w[5] * r[3];
    if r[4] != 0 || r[5] != 0 || r[6] != 0 || r[7] != 0 {
        a0 += w[4] * r[4] + w[6] * r[6];
        a1 += -w[4] * r[4] - w[2] * r[6];
        a2 += -w[4] * r[4] + w[2] * r[6];
        a3 += w[4] * r[4] - w[6] * r[6];
        b0 += w[5] * r[5] + w[7] * r[7];
        b1 += -w[1] * r[5] - w[5] * r[7];
        b2 += w[7] * r[5] + w[3] * r[7];
        b3 += w[3] * r[5] - w[1] * r[7];
    }
    let o = |v: i32| i32::from((v >> s) as i16);
    r[0] = o(a0 + b0);
    r[7] = o(a0 - b0);
    r[1] = o(a1 + b1);
    r[6] = o(a1 - b1);
    r[2] = o(a2 + b2);
    r[5] = o(a2 - b2);
    r[3] = o(a3 + b3);
    r[4] = o(a3 - b3);
}

#[cfg(test)]
#[inline(always)]
fn col(b: &[i32; 64], c: usize, a: &Arith, out: &mut [u16; 64]) {
    let w = &a.w;
    let s = a.col_shift;
    let x = |k: usize| b[k * 8 + c];
    let mut a0 = w[4] * (x(0) + ((1 << (s - 1)) / w[4]));
    let mut a1 = a0;
    let mut a2 = a0;
    let mut a3 = a0;
    a0 += w[2] * x(2);
    a1 += w[6] * x(2);
    a2 -= w[6] * x(2);
    a3 -= w[2] * x(2);
    let mut b0 = w[1] * x(1) + w[3] * x(3);
    let mut b1 = w[3] * x(1) - w[7] * x(3);
    let mut b2 = w[5] * x(1) - w[1] * x(3);
    let mut b3 = w[7] * x(1) - w[5] * x(3);
    let x4 = x(4);
    a0 += w[4] * x4;
    a1 -= w[4] * x4;
    a2 -= w[4] * x4;
    a3 += w[4] * x4;
    let x5 = x(5);
    b0 += w[5] * x5;
    b1 -= w[1] * x5;
    b2 += w[7] * x5;
    b3 += w[3] * x5;
    let x6 = x(6);
    a0 += w[6] * x6;
    a1 -= w[2] * x6;
    a2 += w[2] * x6;
    a3 -= w[6] * x6;
    let x7 = x(7);
    b0 += w[7] * x7;
    b1 -= w[5] * x7;
    b2 += w[3] * x7;
    b3 -= w[1] * x7;
    let o = |v: i32| i32::from((v >> s) as i16).clamp(a.lo, a.hi) as u16;
    out[c] = o(a0 + b0);
    out[8 + c] = o(a1 + b1);
    out[16 + c] = o(a2 + b2);
    out[24 + c] = o(a3 + b3);
    out[32 + c] = o(a3 - b3);
    out[40 + c] = o(a2 - b2);
    out[48 + c] = o(a1 - b1);
    out[56 + c] = o(a0 - b0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dc_only_is_flat_mid_grey_plus_dc() {
        // A DC level of -16 at weight 16: 512 - 256/32 = 504 (10-bit), 2048 - 256/8 at 12 bits.
        for (a, want) in [(Arith::B10, 504), (Arith::B12, 2016)] {
            let mut b = [0i32; 64];
            b[0] = -16;
            let mut out = [0u16; 64];
            put_scalar(&mut b, &[16; 64], &a, &mut out);
            assert!(out.iter().all(|&v| v == want), "{:?}", &out[..8]);
        }
    }

    #[test]
    fn clips_to_the_legal_range() {
        let mut b = [0i32; 64];
        b[0] = 2000;
        let mut out = [0u16; 64];
        put_scalar(&mut b, &[16; 64], &Arith::B10, &mut out);
        assert!(out.iter().all(|&v| v == 1019));
    }

    #[test]
    fn transpose_is_a_transpose() {
        let r: [V; 8] =
            std::array::from_fn(|i| V::from_array(std::array::from_fn(|j| (i * 8 + j) as i32)));
        let t = transpose(r);
        for (i, row) in t.iter().enumerate() {
            for (j, &v) in row.as_array().iter().enumerate() {
                assert_eq!(v, (j * 8 + i) as i32);
            }
        }
    }

    #[test]
    fn vector_form_matches_the_scalar_one() {
        // A small LCG: sparse and dense blocks, DC-only rows, both depths.
        let mut seed = 0x1234_5678u32;
        let mut rnd = |n: u32| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) % n
        };
        for case in 0..4000 {
            let a = if case % 2 == 0 {
                Arith::B10
            } else {
                Arith::B12
            };
            let mut b = [0i32; 64];
            let n = 1 + rnd(if case % 3 == 0 { 64 } else { 6 });
            for _ in 0..n {
                b[rnd(64) as usize] = rnd(400) as i32 - 200;
            }
            let q: [i32; 64] = std::array::from_fn(|_| 1 + rnd(60) as i32);
            let mut want = [0u16; 64];
            put_scalar(&mut b.clone(), &q, &a, &mut want);
            let tr =
                |m: &[i32; 64]| -> [i32; 64] { std::array::from_fn(|i| m[(i % 8) * 8 + i / 8]) };
            let mut got = [0u16; 64];
            put(&tr(&b), &tr(&q), &a, &mut got);
            assert_eq!(got, want, "case {case}");
        }
    }
}
