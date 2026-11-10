//! The 4×4 integer transform, the DC Hadamard transforms, quantisation, and the decoder's
//! scaling and inverse transform (8.5.10–8.5.12) — the reconstruction must equal a decoder's
//! bit for bit.

use std::simd::cmp::{SimdOrd, SimdPartialOrd};
use std::simd::num::{SimdFloat, SimdInt, SimdUint};
use std::simd::{Select, Simd, StdFloat, simd_swizzle};

use crate::tables::{DEQUANT_V, POS_CLASS, QUANT_MF};

type I16x16 = Simd<i16, 16>;
type I32x16 = Simd<i32, 16>;
type I32x4 = Simd<i32, 4>;
type I32x8 = Simd<i32, 8>;
type F64x8 = Simd<f64, 8>;

/// One pass of the forward core transform over the four rows held in lanes 4r..4r+4:
/// `[s03 + s12, 2·d03 + d12, s03 − s12, d03 − 2·d12]` with `s03 = x0 + x3` etc. From the
/// sums and differences of each lane and its mirror (`s = [s03, s12, s12, s03]`,
/// `d = [d03, d12, −d12, −d03]`) that is `e·[1, 2, 1, −1] + f·[1, 1, −1, 2]` with
/// `e = [s0, d0, s3, d3]` and `f = [s1, d1, s2, d2]`.
#[inline(always)]
fn fdct4_rows(v: I16x16) -> I16x16 {
    const ME: I16x16 = Simd::from_array([1, 2, 1, -1, 1, 2, 1, -1, 1, 2, 1, -1, 1, 2, 1, -1]);
    const MF: I16x16 = Simd::from_array([1, 1, -1, 2, 1, 1, -1, 2, 1, 1, -1, 2, 1, 1, -1, 2]);
    let r = simd_swizzle!(v, [3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13, 12]);
    let (s, d) = (v + r, v - r);
    let e = simd_swizzle!(
        s,
        d,
        [0, 16, 3, 19, 4, 20, 7, 23, 8, 24, 11, 27, 12, 28, 15, 31]
    );
    let f = simd_swizzle!(
        s,
        d,
        [1, 17, 2, 18, 5, 21, 6, 22, 9, 25, 10, 26, 13, 29, 14, 30]
    );
    e * ME + f * MF
}

/// The same pass down the columns (whole rows of four lanes).
#[inline(always)]
fn fdct4_cols(v: I16x16) -> I16x16 {
    const ME: I16x16 = Simd::from_array([1, 1, 1, 1, 2, 2, 2, 2, 1, 1, 1, 1, -1, -1, -1, -1]);
    const MF: I16x16 = Simd::from_array([1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, 2, 2, 2, 2]);
    let r = simd_swizzle!(v, [12, 13, 14, 15, 8, 9, 10, 11, 4, 5, 6, 7, 0, 1, 2, 3]);
    let (s, d) = (v + r, v - r);
    let e = simd_swizzle!(
        s,
        d,
        [0, 1, 2, 3, 16, 17, 18, 19, 12, 13, 14, 15, 28, 29, 30, 31]
    );
    let f = simd_swizzle!(
        s,
        d,
        [4, 5, 6, 7, 20, 21, 22, 23, 8, 9, 10, 11, 24, 25, 26, 27]
    );
    e * ME + f * MF
}

/// Forward core transform (Cf · X · Cfᵀ) of a 4×4 residual in lanes (raster order). A
/// residual of 8-bit samples (|x| ≤ 255) stays within ±9180, so `i16` lanes hold it exactly.
#[inline(always)]
fn fdct4v(x: I16x16) -> I32x16 {
    fdct4_cols(fdct4_rows(x)).cast::<i32>()
}

/// Forward core transform of a 4×4 residual (raster order, 8-bit sample differences),
/// Cf · X · Cfᵀ.
#[cfg(test)]
pub fn fdct4(x: &[i32; 16]) -> [i32; 16] {
    debug_assert!(x.iter().all(|v| v.abs() <= 255));
    fdct4v(Simd::from_array(x.map(|v| v as i16))).to_array()
}

/// [`fdct4`] of the difference of 4×4 blocks `a` (stride `astride`) and `b` (stride
/// `bstride`).
#[inline]
pub fn fdct4_diff(a: &[u8], astride: usize, b: &[u8], bstride: usize) -> [i32; 16] {
    let mut x = [0u8; 16];
    let mut y = [0u8; 16];
    for r in 0..4 {
        x[r * 4..r * 4 + 4].copy_from_slice(&a[r * astride..r * astride + 4]);
        y[r * 4..r * 4 + 4].copy_from_slice(&b[r * bstride..r * bstride + 4]);
    }
    let d = Simd::from_array(x).cast::<i16>() - Simd::from_array(y).cast::<i16>();
    fdct4v(d).to_array()
}

/// The 4×4 Hadamard transform (unnormalised).
#[inline]
pub fn hadamard4(x: &[i32; 16]) -> [i32; 16] {
    let mut t = [0i32; 16];
    for r in 0..4 {
        let a = &x[r * 4..r * 4 + 4];
        let s01 = a[0] + a[1];
        let d01 = a[0] - a[1];
        let s23 = a[2] + a[3];
        let d23 = a[2] - a[3];
        t[r * 4] = s01 + s23;
        t[r * 4 + 1] = s01 - s23;
        t[r * 4 + 2] = d01 - d23;
        t[r * 4 + 3] = d01 + d23;
    }
    let mut o = [0i32; 16];
    for c in 0..4 {
        let s01 = t[c] + t[4 + c];
        let d01 = t[c] - t[4 + c];
        let s23 = t[8 + c] + t[12 + c];
        let d23 = t[8 + c] - t[12 + c];
        o[c] = s01 + s23;
        o[4 + c] = s01 - s23;
        o[8 + c] = d01 - d23;
        o[12 + c] = d01 + d23;
    }
    o
}

/// Largest level magnitude written; keeps every level codable with CAVLC's 12-bit escape in
/// Baseline (level_prefix ≤ 15).
pub const MAX_LEVEL: i32 = 2047;

/// Quantises one coefficient: `(|w|·mf + f) >> shift` with the sign of `w`.
#[inline]
fn q(w: i32, mf: i32, f: i32, shift: u32) -> i32 {
    let a = ((w.abs() * mf + f) >> shift).min(MAX_LEVEL);
    if w < 0 { -a } else { a }
}

/// [`QUANT_MF`] by `qp % 6` and raster position.
const MF4: [I32x16; 6] = {
    let mut t = [Simd::from_array([0; 16]); 6];
    let mut m = 0;
    while m < 6 {
        let mut a = [0i32; 16];
        let mut i = 0;
        while i < 16 {
            a[i] = QUANT_MF[m][POS_CLASS[i]];
            i += 1;
        }
        t[m] = Simd::from_array(a);
        m += 1;
    }
    t
};

/// [`DEQUANT_V`] by `qp % 6` and raster position.
const DQ4: [I32x16; 6] = {
    let mut t = [Simd::from_array([0; 16]); 6];
    let mut m = 0;
    while m < 6 {
        let mut a = [0i32; 16];
        let mut i = 0;
        while i < 16 {
            a[i] = DEQUANT_V[m][POS_CLASS[i]];
            i += 1;
        }
        t[m] = Simd::from_array(a);
        m += 1;
    }
    t
};

/// Quantises a transformed 4×4 block (raster order). `intra` picks the rounding offset
/// (1/3 vs 1/6). Position 0 is skipped when `skip_dc` (it is coded through the DC transform).
#[inline]
pub fn quant4(w: &[i32; 16], qp: u8, intra: bool, skip_dc: bool) -> [i16; 16] {
    let m = usize::from(qp % 6);
    let shift = 15 + i32::from(qp / 6);
    let f = if intra {
        (1 << shift) / 3
    } else {
        (1 << shift) / 6
    };
    let w = I32x16::from_array(*w);
    let a = ((w.abs() * MF4[m] + Simd::splat(f)) >> Simd::splat(shift))
        .simd_min(Simd::splat(MAX_LEVEL));
    let mut o = w
        .simd_lt(Simd::splat(0))
        .select(-a, a)
        .cast::<i16>()
        .to_array();
    if skip_dc {
        o[0] = 0;
    }
    o
}

/// Unquantised (fractional) levels of a transformed 4×4 block (raster order).
#[inline]
pub fn frac4(w: &[i32; 16], qp: u8) -> [f32; 16] {
    let m = usize::from(qp % 6);
    let scale = 1.0 / (1u32 << (15 + u32::from(qp / 6))) as f32;
    ((I32x16::from_array(*w) * MF4[m]).cast::<f32>() * Simd::splat(scale)).to_array()
}

/// The squared quantiser step size: the reconstruction error (sum of squared samples) of a
/// level off by one.
pub fn qstep2(qp: u8) -> f32 {
    const Q: [f32; 6] = [0.625, 0.6875, 0.8125, 0.875, 1.0, 1.125];
    let q = Q[usize::from(qp % 6)] * (1u32 << (qp / 6)) as f32;
    q * q
}

/// Quantises the Hadamard-transformed luma DC (already halved) or chroma DC coefficients.
#[inline]
pub fn quant_dc(w: &[i32], out: &mut [i16], qp: u8, intra: bool) {
    let m = usize::from(qp % 6);
    let shift = 16 + u32::from(qp / 6);
    let f = if intra {
        (1 << shift) / 3
    } else {
        (1 << shift) / 6
    };
    for (o, &v) in out.iter_mut().zip(w) {
        *o = q(v, QUANT_MF[m][0], f, shift) as i16;
    }
}

/// Scales levels back (8.5.12.1, flat matrices) for positions 1..16 or 0..16.
#[inline]
pub fn dequant4(c: &[i16; 16], qp: u8, skip_dc: bool) -> [i32; 16] {
    let m = usize::from(qp % 6);
    let s = i32::from(qp / 6);
    // LevelScale = 16·v; (c·16v) << (qP/6 − 4), or rounded right shift below 24 — both
    // equal c·v << (qP/6) exactly.
    let mut d = ((Simd::from_array(*c).cast::<i32>() * DQ4[m]) << Simd::splat(s)).to_array();
    if skip_dc {
        d[0] = 0;
    }
    d
}

/// Inverse transform and scaling of the Intra16x16 luma DC levels (8.5.10), raster order of
/// the 4×4 blocks.
pub fn dequant_luma_dc(c: &[i16; 16], qp: u8) -> [i32; 16] {
    let f = hadamard4(&c.map(i32::from));
    let ls = 16 * DEQUANT_V[usize::from(qp % 6)][0];
    let q6 = i32::from(qp / 6);
    let mut d = [0i32; 16];
    for i in 0..16 {
        d[i] = if qp >= 36 {
            (f[i] * ls) << (q6 - 6)
        } else {
            (f[i] * ls + (1 << (5 - q6))) >> (6 - q6)
        };
    }
    d
}

/// The 2×2 chroma DC transform (its own inverse up to scale).
#[inline]
pub fn hadamard2(c: &[i32; 4]) -> [i32; 4] {
    let s0 = c[0] + c[1];
    let d0 = c[0] - c[1];
    let s1 = c[2] + c[3];
    let d1 = c[2] - c[3];
    [s0 + s1, d0 + d1, s0 - s1, d0 - d1]
}

/// Inverse transform and scaling of chroma DC levels (8.5.11.2).
pub fn dequant_chroma_dc(c: &[i16; 4], qpc: u8) -> [i32; 4] {
    let f = hadamard2(&c.map(i32::from));
    let ls = 16 * DEQUANT_V[usize::from(qpc % 6)][0];
    let q6 = u32::from(qpc / 6);
    f.map(|v| ((v * ls) << q6) >> 5)
}

/// One 1-D pass of the inverse 4×4 transform (8.5.12.2), on four lanes at once.
#[inline(always)]
fn idct4_1d(a: [I32x4; 4]) -> [I32x4; 4] {
    let one = I32x4::splat(1);
    let e = a[0] + a[2];
    let f = a[0] - a[2];
    let g = (a[1] >> one) - a[3];
    let h = a[1] + (a[3] >> one);
    [e + h, f + g, f - g, e - h]
}

/// Transposes a 4×4 block held as four row vectors (two perfect shuffles).
#[inline(always)]
fn transpose4(mut v: [I32x4; 4]) -> [I32x4; 4] {
    for _ in 0..2 {
        let (a, b) = v[0].interleave(v[2]);
        let (c, d) = v[1].interleave(v[3]);
        v = [a, b, c, d];
    }
    v
}

/// Inverse 4×4 transform of scaled coefficients `d` added to `out` (stride `os`).
#[inline]
pub fn idct4_add(d: &[i32; 16], out: &mut [u8], os: usize) {
    let (rows, _) = d.as_chunks::<4>();
    let rows = std::array::from_fn(|r| I32x4::from_array(rows[r]));
    // Rows first, then columns (the shifts make the order matter).
    let r = idct4_1d(transpose4(idct4_1d(transpose4(rows))));
    let (r32, s6) = (I32x4::splat(32), I32x4::splat(6));
    for (k, v) in r.into_iter().enumerate() {
        let p = &mut out[k * os..k * os + 4];
        let x = Simd::<u8, 4>::from_slice(p).cast::<i32>() + ((v + r32) >> s6);
        x.simd_clamp(I32x4::splat(0), I32x4::splat(255))
            .cast::<u8>()
            .copy_to_slice(p);
    }
}

// ---- 8×8 transform (High profile) -------------------------------------------------------

/// The 8×8 core transform matrix, scaled by 8 (its rows are orthogonal).
const T8: [[i32; 8]; 8] = [
    [8, 8, 8, 8, 8, 8, 8, 8],
    [12, 10, 6, 3, -3, -6, -10, -12],
    [8, 4, -4, -8, -8, -4, 4, 8],
    [10, -3, -12, -6, 6, 12, 3, -10],
    [8, -8, -8, 8, 8, -8, -8, 8],
    [6, -12, 3, 10, -10, -3, 12, -6],
    [4, -8, 8, -4, -4, 8, -8, 4],
    [3, -6, 10, -12, 12, -10, 6, -3],
];

/// Squared norms of the rows of `T8`.
const T8_NORM: [i64; 8] = [512, 578, 320, 578, 512, 578, 320, 578];

/// normAdjust8x8 (8-318) by qP % 6 and position class.
const V8: [[i32; 6]; 6] = [
    [20, 18, 32, 19, 25, 24],
    [22, 19, 35, 21, 28, 26],
    [26, 23, 42, 24, 33, 31],
    [28, 25, 45, 26, 35, 33],
    [32, 28, 51, 30, 40, 38],
    [36, 32, 58, 34, 46, 43],
];

/// Position class of raster 8×8 coefficient `i` for `V8`.
const fn class8(i: usize) -> usize {
    let (r, c) = (i / 8, i % 8);
    if r % 4 == 0 && c % 4 == 0 {
        0
    } else if r % 2 == 1 && c % 2 == 1 {
        1
    } else if r % 4 == 2 && c % 4 == 2 {
        2
    } else if (r % 4 == 0 && c % 2 == 1) || (r % 2 == 1 && c % 4 == 0) {
        3
    } else if (r % 4 == 0 && c % 4 == 2) || (r % 4 == 2 && c % 4 == 0) {
        4
    } else {
        5
    }
}

/// Quantiser multipliers by qP % 6 and raster position: levels are `W·MF >> (22 + qP/6)`
/// for the coefficients `W` of `fdct8` — the inverse of the decoder's scaling.
static MF8: [[i64; 64]; 6] = {
    let mut t = [[0i64; 64]; 6];
    let mut m = 0;
    while m < 6 {
        let mut i = 0;
        while i < 64 {
            let d = T8_NORM[i / 8] * T8_NORM[i % 8] * V8[m][class8(i)] as i64;
            t[m][i] = ((1i64 << 36) + d / 2) / d;
            i += 1;
        }
        m += 1;
    }
    t
};

/// The 8×8 frame zig-zag scan: scan position → raster index.
pub const ZIGZAG8: [usize; 64] = {
    let mut t = [0usize; 64];
    let mut k = 0;
    let mut s: usize = 0;
    while s < 15 {
        let lo = s.saturating_sub(7);
        let hi = if s < 7 { s } else { 7 };
        let mut j = 0;
        while j <= hi - lo {
            // Odd diagonals run down (row increasing), even ones up.
            let r = if s % 2 == 1 { lo + j } else { hi - j };
            t[k] = r * 8 + (s - r);
            k += 1;
            j += 1;
        }
        s += 1;
    }
    t
};

/// `MF8` as `f64` (exact: every entry is below 2¹⁶).
static MF8F: [[f64; 64]; 6] = {
    let mut t = [[0f64; 64]; 6];
    let mut m = 0;
    while m < 6 {
        let mut i = 0;
        while i < 64 {
            t[m][i] = MF8[m][i] as f64;
            i += 1;
        }
        m += 1;
    }
    t
};

/// LevelScale8x8 (8-317, flat matrices) by qP % 6 and raster position.
static LS8: [[i32; 64]; 6] = {
    let mut t = [[0i32; 64]; 6];
    let mut m = 0;
    while m < 6 {
        let mut i = 0;
        while i < 64 {
            t[m][i] = 16 * V8[m][class8(i)];
            i += 1;
        }
        m += 1;
    }
    t
};

/// Transposes an 8×8 block held as eight row vectors: three perfect shuffles, each rotating
/// the six bits of (row, column) by one.
#[inline(always)]
fn transpose8(mut v: [I32x8; 8]) -> [I32x8; 8] {
    for _ in 0..3 {
        let mut x = [I32x8::splat(0); 8];
        for i in 0..4 {
            (x[2 * i], x[2 * i + 1]) = v[i].interleave(v[i + 4]);
        }
        v = x;
    }
    v
}

#[inline(always)]
fn rows8(x: &[i32; 64]) -> [I32x8; 8] {
    let (c, _) = x.as_chunks::<8>();
    std::array::from_fn(|r| I32x8::from_array(c[r]))
}

/// `T8 · V` for the eight row vectors of `v` (each lane a separate column): even rows of
/// `T8` are symmetric and odd ones antisymmetric, so they apply to the sums and differences
/// of mirrored rows.
#[inline(always)]
fn t8_apply(v: [I32x8; 8]) -> [I32x8; 8] {
    let e: [I32x8; 4] = std::array::from_fn(|c| v[c] + v[7 - c]);
    let o: [I32x8; 4] = std::array::from_fn(|c| v[c] - v[7 - c]);
    std::array::from_fn(|k| {
        let h = if k % 2 == 0 { &e } else { &o };
        let t = &T8[k];
        h[0] * I32x8::splat(t[0])
            + h[1] * I32x8::splat(t[1])
            + h[2] * I32x8::splat(t[2])
            + h[3] * I32x8::splat(t[3])
    })
}

/// Forward 8×8 transform of a residual (raster order), T8 · X · T8ᵀ.
pub fn fdct8(x: &[i32; 64]) -> [i32; 64] {
    // Exact integer arithmetic, so the passes may go in either order: (T8 · (T8 · X)ᵀ)ᵀ.
    let o = transpose8(t8_apply(transpose8(t8_apply(rows8(x)))));
    let mut out = [0i32; 64];
    for (d, v) in out.as_chunks_mut::<8>().0.iter_mut().zip(o) {
        *d = v.to_array();
    }
    out
}

/// Quantises `fdct8` coefficients (raster order).
pub fn quant8(w: &[i32; 64], qp: u8, intra: bool) -> [i16; 64] {
    let m = usize::from(qp % 6);
    let shift = 22 + u32::from(qp / 6);
    let f = if intra {
        (1i64 << shift) / 3
    } else {
        (1i64 << shift) / 6
    };
    // In f64, exactly: |W|·MF < 2³⁸ and the rounding offset < 2³¹ add without loss, and the
    // shift is a power-of-two scale and a floor.
    let f = F64x8::splat(f as f64);
    let inv = F64x8::splat(1.0 / (1u64 << shift) as f64);
    let max = F64x8::splat(f64::from(MAX_LEVEL));
    let mut o = [0i16; 64];
    let (wc, _) = w.as_chunks::<8>();
    let (mc, _) = MF8F[m].as_chunks::<8>();
    for ((d, w), mf) in o.as_chunks_mut::<8>().0.iter_mut().zip(wc).zip(mc) {
        let w = I32x8::from_array(*w);
        let a: F64x8 = w.abs().cast();
        let l: I32x8 = ((a * F64x8::from_array(*mf) + f) * inv)
            .floor()
            .simd_min(max)
            .cast();
        *d = w
            .simd_lt(I32x8::splat(0))
            .select(-l, l)
            .cast::<i16>()
            .to_array();
    }
    o
}

/// Unquantised (fractional) levels of `fdct8` coefficients (raster order).
pub fn frac8(w: &[i32; 64], qp: u8) -> [f32; 64] {
    let m = usize::from(qp % 6);
    let scale = F64x8::splat(1.0 / (1u64 << (22 + u32::from(qp / 6))) as f64);
    let mut o = [0f32; 64];
    let (wc, _) = w.as_chunks::<8>();
    let (mc, _) = MF8F[m].as_chunks::<8>();
    for ((d, w), mf) in o.as_chunks_mut::<8>().0.iter_mut().zip(wc).zip(mc) {
        let w: F64x8 = I32x8::from_array(*w).cast();
        *d = (w * F64x8::from_array(*mf) * scale)
            .cast::<f32>()
            .to_array();
    }
    o
}

/// Scales 8×8 levels back (8.5.13.1, flat matrices).
pub fn dequant8(c: &[i16; 64], qp: u8) -> [i32; 64] {
    let ls = &LS8[usize::from(qp % 6)];
    let q6 = i32::from(qp / 6);
    let mut d = [0i32; 64];
    let (cc, _) = c.as_chunks::<16>();
    let (lc, _) = ls.as_chunks::<16>();
    for ((d, c), l) in d.as_chunks_mut::<16>().0.iter_mut().zip(cc).zip(lc) {
        let v = I16x16::from_array(*c).cast::<i32>() * I32x16::from_array(*l);
        *d = if q6 >= 6 {
            v << I32x16::splat(q6 - 6)
        } else {
            (v + I32x16::splat(1 << (5 - q6))) >> I32x16::splat(6 - q6)
        }
        .to_array();
    }
    d
}

/// One 1-D pass of the inverse 8×8 transform (8.5.13.2), on eight lanes at once.
#[inline(always)]
fn idct8_1d(d: [I32x8; 8]) -> [I32x8; 8] {
    let one = I32x8::splat(1);
    let two = I32x8::splat(2);
    let a0 = d[0] + d[4];
    let a4 = d[0] - d[4];
    let a2 = (d[2] >> one) - d[6];
    let a6 = d[2] + (d[6] >> one);
    let b0 = a0 + a6;
    let b2 = a4 + a2;
    let b4 = a4 - a2;
    let b6 = a0 - a6;
    let a1 = -d[3] + d[5] - d[7] - (d[7] >> one);
    let a3 = d[1] + d[7] - d[3] - (d[3] >> one);
    let a5 = -d[1] + d[7] + d[5] + (d[5] >> one);
    let a7 = d[3] + d[5] + d[1] + (d[1] >> one);
    let b1 = a1 + (a7 >> two);
    let b7 = a7 - (a1 >> two);
    let b3 = a3 + (a5 >> two);
    let b5 = (a3 >> two) - a5;
    [
        b0 + b7,
        b2 + b5,
        b4 + b3,
        b6 + b1,
        b6 - b1,
        b4 - b3,
        b2 - b5,
        b0 - b7,
    ]
}

/// Inverse 8×8 transform of scaled coefficients added to `out` (stride `os`).
pub fn idct8_add(d: &[i32; 64], out: &mut [u8], os: usize) {
    // Rows first, then columns, as the decoder does (the shifts make the order matter):
    // the row pass runs on the transposed block, and its result is transposed back.
    let r = idct8_1d(transpose8(idct8_1d(transpose8(rows8(d)))));
    let (r32, s6) = (I32x8::splat(32), I32x8::splat(6));
    for (k, v) in r.into_iter().enumerate() {
        let p = &mut out[k * os..k * os + 8];
        let x = Simd::<u8, 8>::from_slice(p).cast::<i32>() + ((v + r32) >> s6);
        x.simd_clamp(I32x8::splat(0), I32x8::splat(255))
            .cast::<u8>()
            .copy_to_slice(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inverse 4×4 transform of scaled coefficients `d` added to `pred` (stride `ps`), written to
    /// `out` (stride `os`).
    fn idct4_add_ref(d: &[i32; 16], out: &mut [u8], os: usize) {
        let mut t = [0i32; 16];
        for r in 0..4 {
            let a = &d[r * 4..r * 4 + 4];
            let e = a[0] + a[2];
            let f = a[0] - a[2];
            let g = (a[1] >> 1) - a[3];
            let h = a[1] + (a[3] >> 1);
            t[r * 4] = e + h;
            t[r * 4 + 1] = f + g;
            t[r * 4 + 2] = f - g;
            t[r * 4 + 3] = e - h;
        }
        for c in 0..4 {
            let e = t[c] + t[8 + c];
            let f = t[c] - t[8 + c];
            let g = (t[4 + c] >> 1) - t[12 + c];
            let h = t[4 + c] + (t[12 + c] >> 1);
            let col = [e + h, f + g, f - g, e - h];
            for (r, v) in col.into_iter().enumerate() {
                let p = &mut out[r * os + c];
                *p = (i32::from(*p) + ((v + 32) >> 6)).clamp(0, 255) as u8;
            }
        }
    }

    #[test]
    fn vector_idct4_matches_scalar() {
        let mut seed = 0x9e37_79b9u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for it in 0..20000 {
            let amp = [1 << 20, 4000, 100][it % 3];
            let d: [i32; 16] = std::array::from_fn(|_| (rnd() % (2 * amp + 1)) as i32 - amp as i32);
            let mut p: [u8; 64] = std::array::from_fn(|_| rnd() as u8);
            let mut q = p;
            idct4_add(&d, &mut p[1..], 13);
            idct4_add_ref(&d, &mut q[1..], 13);
            assert_eq!(p, q);
        }
    }

    /// Forward 8×8 transform of a residual (raster order), T8 · X · T8ᵀ.
    fn fdct8_ref(x: &[i32; 64]) -> [i32; 64] {
        let mut t = [0i32; 64];
        for r in 0..8 {
            for k in 0..8 {
                let mut s = 0;
                for c in 0..8 {
                    s += x[r * 8 + c] * T8[k][c];
                }
                t[r * 8 + k] = s;
            }
        }
        let mut o = [0i32; 64];
        for c in 0..8 {
            for k in 0..8 {
                let mut s = 0;
                for r in 0..8 {
                    s += t[r * 8 + c] * T8[k][r];
                }
                o[k * 8 + c] = s;
            }
        }
        o
    }

    /// Quantises `fdct8` coefficients (raster order).
    fn quant8_ref(w: &[i32; 64], qp: u8, intra: bool) -> [i16; 64] {
        let m = usize::from(qp % 6);
        let shift = 22 + u32::from(qp / 6);
        let f = if intra {
            (1i64 << shift) / 3
        } else {
            (1i64 << shift) / 6
        };
        let mut o = [0i16; 64];
        for i in 0..64 {
            let a = ((i64::from(w[i].abs()) * MF8[m][i] + f) >> shift).min(i64::from(MAX_LEVEL));
            o[i] = if w[i] < 0 { -(a as i16) } else { a as i16 };
        }
        o
    }

    /// Unquantised (fractional) levels of `fdct8` coefficients (raster order).
    fn frac8_ref(w: &[i32; 64], qp: u8) -> [f32; 64] {
        let m = usize::from(qp % 6);
        let scale = 1.0 / (1u64 << (22 + u32::from(qp / 6))) as f64;
        std::array::from_fn(|i| (f64::from(w[i]) * MF8[m][i] as f64 * scale) as f32)
    }

    /// Scales 8×8 levels back (8.5.13.1, flat matrices).
    fn dequant8_ref(c: &[i16; 64], qp: u8) -> [i32; 64] {
        let m = usize::from(qp % 6);
        let q6 = i32::from(qp / 6);
        let mut d = [0i32; 64];
        for i in 0..64 {
            let ls = 16 * V8[m][class8(i)];
            let c = i32::from(c[i]);
            d[i] = if q6 >= 6 {
                (c * ls) << (q6 - 6)
            } else {
                (c * ls + (1 << (5 - q6))) >> (6 - q6)
            };
        }
        d
    }

    /// One 1-D pass of the inverse 8×8 transform (8.5.13.2).
    fn idct8_1d_ref(d: [i32; 8]) -> [i32; 8] {
        let a0 = d[0] + d[4];
        let a4 = d[0] - d[4];
        let a2 = (d[2] >> 1) - d[6];
        let a6 = d[2] + (d[6] >> 1);
        let b0 = a0 + a6;
        let b2 = a4 + a2;
        let b4 = a4 - a2;
        let b6 = a0 - a6;
        let a1 = -d[3] + d[5] - d[7] - (d[7] >> 1);
        let a3 = d[1] + d[7] - d[3] - (d[3] >> 1);
        let a5 = -d[1] + d[7] + d[5] + (d[5] >> 1);
        let a7 = d[3] + d[5] + d[1] + (d[1] >> 1);
        let b1 = a1 + (a7 >> 2);
        let b7 = a7 - (a1 >> 2);
        let b3 = a3 + (a5 >> 2);
        let b5 = (a3 >> 2) - a5;
        [
            b0 + b7,
            b2 + b5,
            b4 + b3,
            b6 + b1,
            b6 - b1,
            b4 - b3,
            b2 - b5,
            b0 - b7,
        ]
    }

    /// Inverse 8×8 transform of scaled coefficients added to `out` (stride `os`).
    fn idct8_add_ref(d: &[i32; 64], out: &mut [u8], os: usize) {
        let mut t = [0i32; 64];
        for r in 0..8 {
            let row = idct8_1d_ref(core::array::from_fn(|k| d[r * 8 + k]));
            t[r * 8..r * 8 + 8].copy_from_slice(&row);
        }
        for c in 0..8 {
            let col = idct8_1d_ref(core::array::from_fn(|k| t[k * 8 + c]));
            for (r, v) in col.into_iter().enumerate() {
                let p = &mut out[r * os + c];
                *p = (i32::from(*p) + ((v + 32) >> 6)).clamp(0, 255) as u8;
            }
        }
    }

    /// The vector 8×8 transforms, quantisation and scaling equal their scalar definitions.
    #[test]
    fn vector_8x8_matches_scalar() {
        let mut seed = 0x1234_5678u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for it in 0..2000 {
            let amp = [255, 40, 3][it % 3];
            let x: [i32; 64] = std::array::from_fn(|_| (rnd() % (2 * amp + 1)) as i32 - amp as i32);
            let w = fdct8(&x);
            assert_eq!(w, fdct8_ref(&x));
            for qp in [0u8, 5, 17, 24, 30, 35, 36, 41, 51] {
                for intra in [false, true] {
                    assert_eq!(quant8(&w, qp, intra), quant8_ref(&w, qp, intra));
                }
                let (f, g) = (frac8(&w, qp), frac8_ref(&w, qp));
                assert!(f.iter().zip(&g).all(|(a, b)| a.to_bits() == b.to_bits()));
                let c: [i16; 64] = std::array::from_fn(|_| (rnd() % 4095) as i16 - 2047);
                let c = if it % 2 == 0 { quant8(&w, qp, true) } else { c };
                let d = dequant8(&c, qp);
                assert_eq!(d, dequant8_ref(&c, qp));
                let mut p: [u8; 128] = std::array::from_fn(|_| rnd() as u8);
                let mut q = p;
                idct8_add(&d, &mut p[3..], 15);
                idct8_add_ref(&d, &mut q[3..], 15);
                assert_eq!(p, q);
            }
        }
    }

    /// The vector 4×4 transform, quantisation and scaling equal their scalar definitions.
    #[test]
    fn vector_4x4_matches_scalar() {
        fn fdct4_ref(x: &[i32; 16]) -> [i32; 16] {
            let mut t = [0i32; 16];
            for r in 0..4 {
                let a = &x[r * 4..r * 4 + 4];
                let (s03, d03, s12, d12) = (a[0] + a[3], a[0] - a[3], a[1] + a[2], a[1] - a[2]);
                t[r * 4..r * 4 + 4].copy_from_slice(&[
                    s03 + s12,
                    2 * d03 + d12,
                    s03 - s12,
                    d03 - 2 * d12,
                ]);
            }
            let mut o = [0i32; 16];
            for c in 0..4 {
                let (s03, d03) = (t[c] + t[12 + c], t[c] - t[12 + c]);
                let (s12, d12) = (t[4 + c] + t[8 + c], t[4 + c] - t[8 + c]);
                o[c] = s03 + s12;
                o[4 + c] = 2 * d03 + d12;
                o[8 + c] = s03 - s12;
                o[12 + c] = d03 - 2 * d12;
            }
            o
        }
        let mut seed = 7u32;
        let mut rnd = move || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            seed >> 8
        };
        for n in 0..4000 {
            let a: [u8; 16] = core::array::from_fn(|_| rnd() as u8);
            // Extremes too: all 0 against all 255 and back.
            let b: [u8; 16] = match n {
                0 => [0; 16],
                1 => [255; 16],
                _ => core::array::from_fn(|_| rnd() as u8),
            };
            let a = if n < 2 { [255 * (1 - n as u8); 16] } else { a };
            let x: [i32; 16] = core::array::from_fn(|i| i32::from(a[i]) - i32::from(b[i]));
            let w = fdct4_ref(&x);
            assert_eq!(fdct4(&x), w);
            assert_eq!(fdct4_diff(&a, 4, &b, 4), w);
            for qp in [0u8, 7, 18, 24, 28, 33, 40, 51] {
                let m = usize::from(qp % 6);
                let shift = 15 + u32::from(qp / 6);
                for (intra, skip_dc) in [(true, false), (false, true), (false, false)] {
                    let f = if intra {
                        (1 << shift) / 3
                    } else {
                        (1 << shift) / 6
                    };
                    let l = quant4(&w, qp, intra, skip_dc);
                    for i in 0..16 {
                        let r = if skip_dc && i == 0 {
                            0
                        } else {
                            q(w[i], QUANT_MF[m][POS_CLASS[i]], f, shift)
                        };
                        assert_eq!(i32::from(l[i]), r);
                    }
                    let d = dequant4(&l, qp, skip_dc);
                    for i in 0..16 {
                        let r = if skip_dc && i == 0 {
                            0
                        } else {
                            (i32::from(l[i]) * DEQUANT_V[m][POS_CLASS[i]]) << (qp / 6)
                        };
                        assert_eq!(d[i], r);
                    }
                }
                let fr = frac4(&w, qp);
                let scale = 1.0 / (1u32 << shift) as f32;
                for i in 0..16 {
                    let r = (w[i] * QUANT_MF[m][POS_CLASS[i]]) as f32 * scale;
                    assert_eq!(fr[i].to_bits(), r.to_bits());
                }
            }
        }
    }

    #[test]
    fn zigzag8_starts_like_the_standard() {
        assert_eq!(&ZIGZAG8[..10], &[0, 1, 8, 16, 9, 2, 3, 10, 17, 24]);
        assert_eq!(ZIGZAG8[63], 63);
        let mut seen = [false; 64];
        for &z in &ZIGZAG8 {
            seen[z] = true;
        }
        assert!(seen.iter().all(|&b| b));
    }

    #[test]
    fn block8_roundtrip() {
        let x: [i32; 64] = core::array::from_fn(|i| ((i * 37) % 23) as i32 - 11);
        for qp in [0u8, 10, 20, 30, 40] {
            let c = quant8(&fdct8(&x), qp, true);
            let mut out = [128u8; 64];
            idct8_add(&dequant8(&c, qp), &mut out, 8);
            let tol = 1 + (1i32 << (qp / 6)) * 2;
            for i in 0..64 {
                let e = (i32::from(out[i]) - 128 - x[i]).abs();
                assert!(e <= tol, "qp {qp} i {i}: {e}");
            }
        }
    }

    #[test]
    fn flat_block_roundtrip() {
        // A constant residual of 10 → only DC; reconstruct close to 10.
        let x = [10i32; 16];
        let w = fdct4(&x);
        assert_eq!(w[0], 160);
        assert!(w[1..].iter().all(|&v| v == 0));
        let c = quant4(&w, 20, true, false);
        let d = dequant4(&c, 20, false);
        let mut out = [100u8; 16];
        idct4_add(&d, &mut out, 4);
        for v in out {
            assert!((109..=111).contains(&v), "{v}");
        }
    }

    /// `qstep2` scales level errors to sample errors for both transforms.
    #[test]
    fn qstep2_matches_reconstruction_error() {
        let mut seed = 1u32;
        let mut rnd = move || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            ((seed >> 16) % 121) as i32 - 60
        };
        // (Small steps add the output rounding's error on top.)
        for qp in [24u8, 30, 37, 42] {
            let (mut e4, mut s4, mut e8, mut s8) = (0f64, 0f64, 0f64, 0f64);
            for _ in 0..400 {
                let x: [i32; 16] = core::array::from_fn(|_| rnd());
                let w = fdct4(&x);
                let f = frac4(&w, qp);
                let c = quant4(&w, qp, true, false);
                let mut out = [128u8; 16];
                idct4_add(&dequant4(&c, qp, false), &mut out, 4);
                for i in 0..16 {
                    e4 += f64::from((f[i] - f32::from(c[i])).powi(2) * qstep2(qp));
                    s4 += f64::from((i32::from(out[i]) - 128 - x[i]).pow(2));
                }
                let x: [i32; 64] = core::array::from_fn(|_| rnd());
                let w = fdct8(&x);
                let f = frac8(&w, qp);
                let c = quant8(&w, qp, true);
                let mut out = [128u8; 64];
                idct8_add(&dequant8(&c, qp), &mut out, 8);
                for i in 0..64 {
                    e8 += f64::from((f[i] - f32::from(c[i])).powi(2) * qstep2(qp));
                    s8 += f64::from((i32::from(out[i]) - 128 - x[i]).pow(2));
                }
            }
            let (r4, r8) = (s4 / e4, s8 / e8);
            assert!(
                (0.9..1.1).contains(&r4) && (0.9..1.1).contains(&r8),
                "qp {qp}: {r4} {r8}"
            );
        }
    }

    #[test]
    fn hadamard_involution() {
        let x: [i32; 16] = core::array::from_fn(|i| (i as i32 * 7) % 11 - 5);
        let y = hadamard4(&hadamard4(&x));
        for i in 0..16 {
            assert_eq!(y[i], x[i] * 16);
        }
    }
}
