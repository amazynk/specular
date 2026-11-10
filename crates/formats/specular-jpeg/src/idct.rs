//! Inverse DCTs, bit-exact ports of libjpeg-turbo's `jidctint.c` (islow 8x8) and `jidctred.c`
//! (4x4, 2x2, 1x1 reduced-size outputs). The AVX2 8x8 kernel performs the identical integer
//! arithmetic on 8 lanes of i32 and produces identical results for valid input.
//!
//! All kernels take coefficients in natural order and the dequantisation table in natural order
//! and write `n x n` u8 samples at `out[off + r * stride ..]`.

const CONST_BITS: i64 = 13;
const PASS1_BITS: i64 = 2;

const FIX_0_298631336: i64 = 2446;
const FIX_0_390180644: i64 = 3196;
const FIX_0_541196100: i64 = 4433;
const FIX_0_765366865: i64 = 6270;
const FIX_0_899976223: i64 = 7373;
const FIX_1_175875602: i64 = 9633;
const FIX_1_501321110: i64 = 12299;
const FIX_1_847759065: i64 = 15137;
const FIX_1_961570560: i64 = 16069;
const FIX_2_053119869: i64 = 16819;
const FIX_2_562915447: i64 = 20995;
const FIX_3_072711026: i64 = 25172;
// jidctred.c
const FIX_0_211164243: i64 = 1730;
const FIX_0_509795579: i64 = 4176;
const FIX_0_601344887: i64 = 4926;
const FIX_0_720959822: i64 = 5906;
const FIX_0_850430095: i64 = 6967;
const FIX_1_061594337: i64 = 8697;
const FIX_1_272758580: i64 = 10426;
const FIX_1_451774981: i64 = 11893;
const FIX_2_172734803: i64 = 17799;
const FIX_3_624509785: i64 = 29692;

#[inline(always)]
fn descale(x: i64, n: i64) -> i64 {
    (x + (1 << (n - 1))) >> n
}

#[inline(always)]
fn clamp_sample(x: i64) -> u8 {
    (x + 128).clamp(0, 255) as u8
}

#[inline(always)]
fn deq(c: &[i16; 64], q: &[u16; 64], i: usize) -> i64 {
    c[i] as i64 * q[i] as i64
}

/// Value of every output sample of a block whose AC coefficients are all zero
/// (identical for all output sizes in libjpeg's islow / reduced IDCTs).
#[inline(always)]
pub(crate) fn dc_only_value(dc: i16, q0: u16) -> u8 {
    let dcq = dc as i64 * q0 as i64;
    clamp_sample(descale(dcq, 3))
}

#[inline(always)]
pub(crate) fn fill_block(n: usize, v: u8, out: &mut [u8], off: usize, stride: usize) {
    for r in 0..n {
        let o = off + r * stride;
        out[o..o + n].fill(v);
    }
}

/// libjpeg `jpeg_idct_islow`.
pub(crate) fn idct8_scalar(
    c: &[i16; 64],
    q: &[u16; 64],
    out: &mut [u8],
    off: usize,
    stride: usize,
) {
    let mut ws = [0i64; 64];
    for col in 0..8 {
        if (1..8).all(|r| c[r * 8 + col] == 0) {
            let dc = deq(c, q, col) << PASS1_BITS;
            for r in 0..8 {
                ws[r * 8 + col] = dc;
            }
            continue;
        }
        let z2 = deq(c, q, 16 + col);
        let z3 = deq(c, q, 48 + col);
        let z1 = (z2 + z3) * FIX_0_541196100;
        let tmp2 = z1 + z3 * -FIX_1_847759065;
        let tmp3 = z1 + z2 * FIX_0_765366865;
        let z2 = deq(c, q, col);
        let z3 = deq(c, q, 32 + col);
        let tmp0 = (z2 + z3) << CONST_BITS;
        let tmp1 = (z2 - z3) << CONST_BITS;
        let (tmp10, tmp13, tmp11, tmp12) = (tmp0 + tmp3, tmp0 - tmp3, tmp1 + tmp2, tmp1 - tmp2);
        let (o0, o1, o2, o3) = odd8(
            deq(c, q, 56 + col),
            deq(c, q, 40 + col),
            deq(c, q, 24 + col),
            deq(c, q, 8 + col),
        );
        let n = CONST_BITS - PASS1_BITS;
        ws[col] = descale(tmp10 + o3, n);
        ws[56 + col] = descale(tmp10 - o3, n);
        ws[8 + col] = descale(tmp11 + o2, n);
        ws[48 + col] = descale(tmp11 - o2, n);
        ws[16 + col] = descale(tmp12 + o1, n);
        ws[40 + col] = descale(tmp12 - o1, n);
        ws[24 + col] = descale(tmp13 + o0, n);
        ws[32 + col] = descale(tmp13 - o0, n);
    }
    for row in 0..8 {
        let w = &ws[row * 8..row * 8 + 8];
        let o = off + row * stride;
        let dst = &mut out[o..o + 8];
        let z2 = w[2];
        let z3 = w[6];
        let z1 = (z2 + z3) * FIX_0_541196100;
        let tmp2 = z1 + z3 * -FIX_1_847759065;
        let tmp3 = z1 + z2 * FIX_0_765366865;
        let tmp0 = (w[0] + w[4]) << CONST_BITS;
        let tmp1 = (w[0] - w[4]) << CONST_BITS;
        let (tmp10, tmp13, tmp11, tmp12) = (tmp0 + tmp3, tmp0 - tmp3, tmp1 + tmp2, tmp1 - tmp2);
        let (o0, o1, o2, o3) = odd8(w[7], w[5], w[3], w[1]);
        let n = CONST_BITS + PASS1_BITS + 3;
        dst[0] = clamp_sample(descale(tmp10 + o3, n));
        dst[7] = clamp_sample(descale(tmp10 - o3, n));
        dst[1] = clamp_sample(descale(tmp11 + o2, n));
        dst[6] = clamp_sample(descale(tmp11 - o2, n));
        dst[2] = clamp_sample(descale(tmp12 + o1, n));
        dst[5] = clamp_sample(descale(tmp12 - o1, n));
        dst[3] = clamp_sample(descale(tmp13 + o0, n));
        dst[4] = clamp_sample(descale(tmp13 - o0, n));
    }
}

#[inline(always)]
fn odd8(t0: i64, t1: i64, t2: i64, t3: i64) -> (i64, i64, i64, i64) {
    let z1 = t0 + t3;
    let z2 = t1 + t2;
    let z3 = t0 + t2;
    let z4 = t1 + t3;
    let z5 = (z3 + z4) * FIX_1_175875602;
    let t0 = t0 * FIX_0_298631336;
    let t1 = t1 * FIX_2_053119869;
    let t2 = t2 * FIX_3_072711026;
    let t3 = t3 * FIX_1_501321110;
    let z1 = z1 * -FIX_0_899976223;
    let z2 = z2 * -FIX_2_562915447;
    let z3 = z3 * -FIX_1_961570560 + z5;
    let z4 = z4 * -FIX_0_390180644 + z5;
    (t0 + z1 + z3, t1 + z2 + z4, t2 + z2 + z3, t3 + z1 + z4)
}

/// libjpeg `jpeg_idct_4x4` (jidctred.c).
pub(crate) fn idct4_scalar(
    c: &[i16; 64],
    q: &[u16; 64],
    out: &mut [u8],
    off: usize,
    stride: usize,
) {
    let mut ws = [0i64; 32]; // 4 rows x 8 cols
    for col in 0..8 {
        if col == 4 {
            continue;
        }
        if [1, 2, 3, 5, 6, 7].iter().all(|&r| c[r * 8 + col] == 0) {
            let dc = deq(c, q, col) << PASS1_BITS;
            for r in 0..4 {
                ws[r * 8 + col] = dc;
            }
            continue;
        }
        let tmp0 = deq(c, q, col) << (CONST_BITS + 1);
        let tmp2 = deq(c, q, 16 + col) * FIX_1_847759065 + deq(c, q, 48 + col) * -FIX_0_765366865;
        let tmp10 = tmp0 + tmp2;
        let tmp12 = tmp0 - tmp2;
        let (z1, z2, z3, z4) = (
            deq(c, q, 56 + col),
            deq(c, q, 40 + col),
            deq(c, q, 24 + col),
            deq(c, q, 8 + col),
        );
        let (t0, t2) = odd4(z1, z2, z3, z4);
        let n = CONST_BITS - PASS1_BITS + 1;
        ws[col] = descale(tmp10 + t2, n);
        ws[24 + col] = descale(tmp10 - t2, n);
        ws[8 + col] = descale(tmp12 + t0, n);
        ws[16 + col] = descale(tmp12 - t0, n);
    }
    for row in 0..4 {
        let w = &ws[row * 8..row * 8 + 8];
        let o = off + row * stride;
        let dst = &mut out[o..o + 4];
        let tmp0 = w[0] << (CONST_BITS + 1);
        let tmp2 = w[2] * FIX_1_847759065 + w[6] * -FIX_0_765366865;
        let tmp10 = tmp0 + tmp2;
        let tmp12 = tmp0 - tmp2;
        let (t0, t2) = odd4(w[7], w[5], w[3], w[1]);
        let n = CONST_BITS + PASS1_BITS + 3 + 1;
        dst[0] = clamp_sample(descale(tmp10 + t2, n));
        dst[3] = clamp_sample(descale(tmp10 - t2, n));
        dst[1] = clamp_sample(descale(tmp12 + t0, n));
        dst[2] = clamp_sample(descale(tmp12 - t0, n));
    }
}

#[inline(always)]
fn odd4(z1: i64, z2: i64, z3: i64, z4: i64) -> (i64, i64) {
    let t0 =
        z1 * -FIX_0_211164243 + z2 * FIX_1_451774981 + z3 * -FIX_2_172734803 + z4 * FIX_1_061594337;
    let t2 =
        z1 * -FIX_0_509795579 + z2 * -FIX_0_601344887 + z3 * FIX_0_899976223 + z4 * FIX_2_562915447;
    (t0, t2)
}

/// libjpeg `jpeg_idct_2x2` (jidctred.c).
pub(crate) fn idct2_scalar(
    c: &[i16; 64],
    q: &[u16; 64],
    out: &mut [u8],
    off: usize,
    stride: usize,
) {
    let mut ws = [0i64; 16]; // 2 rows x 8 cols
    for col in [0usize, 1, 3, 5, 7] {
        if [1, 3, 5, 7].iter().all(|&r| c[r * 8 + col] == 0) {
            let dc = deq(c, q, col) << PASS1_BITS;
            ws[col] = dc;
            ws[8 + col] = dc;
            continue;
        }
        let tmp10 = deq(c, q, col) << (CONST_BITS + 2);
        let tmp0 = deq(c, q, 56 + col) * -FIX_0_720959822
            + deq(c, q, 40 + col) * FIX_0_850430095
            + deq(c, q, 24 + col) * -FIX_1_272758580
            + deq(c, q, 8 + col) * FIX_3_624509785;
        let n = CONST_BITS - PASS1_BITS + 2;
        ws[col] = descale(tmp10 + tmp0, n);
        ws[8 + col] = descale(tmp10 - tmp0, n);
    }
    for row in 0..2 {
        let w = &ws[row * 8..row * 8 + 8];
        let o = off + row * stride;
        let tmp10 = w[0] << (CONST_BITS + 2);
        let tmp0 = w[7] * -FIX_0_720959822
            + w[5] * FIX_0_850430095
            + w[3] * -FIX_1_272758580
            + w[1] * FIX_3_624509785;
        let n = CONST_BITS + PASS1_BITS + 3 + 2;
        out[o] = clamp_sample(descale(tmp10 + tmp0, n));
        out[o + 1] = clamp_sample(descale(tmp10 - tmp0, n));
    }
}

// ---------------------------------------------------------------------------------------------
// 12-bit sample precision variants (libjpeg BITS_IN_JSAMPLE == 12: PASS1_BITS = 1, output
// range 0..4095). The result is reduced to 8 bits by rounding (v * 255 + 2047) / 4095.

#[inline(always)]
fn clamp12_to8(x: i64) -> u8 {
    let v = (x + 2048).clamp(0, 4095);
    ((v * 255 + 2047) / 4095) as u8
}

pub(crate) fn dc_only_value12(dc: i16, q0: u16) -> u8 {
    clamp12_to8(descale(dc as i64 * q0 as i64, 3))
}

/// Generic islow IDCT at 12-bit precision, output n x n with n in {8,4,2,1}. For the reduced
/// sizes we run the full 8x8 transform and average (12-bit files are rare; exactness vs libjpeg
/// is not required for them).
pub(crate) fn idct_12bit(
    c: &[i16; 64],
    q: &[u16; 64],
    n: usize,
    out: &mut [u8],
    off: usize,
    stride: usize,
) {
    let mut px = [0i32; 64];
    idct_islow_raw(c, q, 1, &mut px);
    let f = 8 / n;
    for r in 0..n {
        for x in 0..n {
            let mut sum = 0;
            for dy in 0..f {
                for dx in 0..f {
                    sum += (px[(r * f + dy) * 8 + x * f + dx] as i64).clamp(-2048, 2047);
                }
            }
            let cnt = (f * f) as i64;
            out[off + r * stride + x] = clamp12_to8((sum + cnt / 2).div_euclid(cnt));
        }
    }
}

/// Full 8x8 islow IDCT (libjpeg `jpeg_idct_islow`) with `pass1` extra bits kept between the
/// passes (2 for 8-bit, 1 for 12-bit samples, as libjpeg). The output is the unclamped,
/// not level-shifted sample, row-major.
pub(crate) fn idct_islow_raw(c: &[i16; 64], q: &[u16; 64], pass1: i64, px: &mut [i32; 64]) {
    let mut ws = [0i64; 64];
    for col in 0..8 {
        let z2 = deq(c, q, 16 + col);
        let z3 = deq(c, q, 48 + col);
        let z1 = (z2 + z3) * FIX_0_541196100;
        let tmp2 = z1 + z3 * -FIX_1_847759065;
        let tmp3 = z1 + z2 * FIX_0_765366865;
        let z2 = deq(c, q, col);
        let z3 = deq(c, q, 32 + col);
        let tmp0 = (z2 + z3) << CONST_BITS;
        let tmp1 = (z2 - z3) << CONST_BITS;
        let (tmp10, tmp13, tmp11, tmp12) = (tmp0 + tmp3, tmp0 - tmp3, tmp1 + tmp2, tmp1 - tmp2);
        let (o0, o1, o2, o3) = odd8(
            deq(c, q, 56 + col),
            deq(c, q, 40 + col),
            deq(c, q, 24 + col),
            deq(c, q, 8 + col),
        );
        let s = CONST_BITS - pass1;
        ws[col] = descale(tmp10 + o3, s);
        ws[56 + col] = descale(tmp10 - o3, s);
        ws[8 + col] = descale(tmp11 + o2, s);
        ws[48 + col] = descale(tmp11 - o2, s);
        ws[16 + col] = descale(tmp12 + o1, s);
        ws[40 + col] = descale(tmp12 - o1, s);
        ws[24 + col] = descale(tmp13 + o0, s);
        ws[32 + col] = descale(tmp13 - o0, s);
    }
    for row in 0..8 {
        let w = &ws[row * 8..row * 8 + 8];
        let z2 = w[2];
        let z3 = w[6];
        let z1 = (z2 + z3) * FIX_0_541196100;
        let tmp2 = z1 + z3 * -FIX_1_847759065;
        let tmp3 = z1 + z2 * FIX_0_765366865;
        let tmp0 = (w[0] + w[4]) << CONST_BITS;
        let tmp1 = (w[0] - w[4]) << CONST_BITS;
        let (tmp10, tmp13, tmp11, tmp12) = (tmp0 + tmp3, tmp0 - tmp3, tmp1 + tmp2, tmp1 - tmp2);
        let (o0, o1, o2, o3) = odd8(w[7], w[5], w[3], w[1]);
        let s = CONST_BITS + pass1 + 3;
        let p = &mut px[row * 8..row * 8 + 8];
        let d = |x: i64| descale(x, s).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        p[0] = d(tmp10 + o3);
        p[7] = d(tmp10 - o3);
        p[1] = d(tmp11 + o2);
        p[6] = d(tmp11 - o2);
        p[2] = d(tmp12 + o1);
        p[5] = d(tmp12 - o1);
        p[3] = d(tmp13 + o0);
        p[4] = d(tmp13 - o0);
    }
}

// ---------------------------------------------------------------------------------------------
// Portable SIMD (std::simd: AVX2 under the x86-64-v3 baseline, NEON on aarch64)

pub(crate) mod vector {
    use std::simd::prelude::*;
    use std::simd::simd_swizzle;

    type V = Simd<i32, 8>;
    type V4 = Simd<i32, 4>;

    #[inline(always)]
    fn k(x: i64) -> V {
        V::splat(x as i32)
    }

    #[inline(always)]
    fn k4(x: i64) -> V4 {
        V4::splat(x as i32)
    }

    /// Dequantised row `r` of the block as 8 x i32 (lanes = columns).
    #[inline(always)]
    fn load_row(c: &[i16; 64], q: &[u16; 64], r: usize) -> V {
        let cc = Simd::<i16, 8>::from_slice(&c[8 * r..8 * r + 8]);
        let qq = Simd::<u16, 8>::from_slice(&q[8 * r..8 * r + 8]);
        cc.cast::<i32>() * qq.cast::<i32>()
    }

    /// 8x8 transpose built from in-128-bit-lane unpacks followed by a half swap, which
    /// lower to `vpunpck{l,h}{dq,qdq}` and `vperm2i128` on AVX2.
    #[inline(always)]
    fn transpose(r: &mut [V; 8]) {
        let lo32 = |a: V, b: V| simd_swizzle!(a, b, [0, 8, 1, 9, 4, 12, 5, 13]);
        let hi32 = |a: V, b: V| simd_swizzle!(a, b, [2, 10, 3, 11, 6, 14, 7, 15]);
        let lo64 = |a: V, b: V| simd_swizzle!(a, b, [0, 1, 8, 9, 4, 5, 12, 13]);
        let hi64 = |a: V, b: V| simd_swizzle!(a, b, [2, 3, 10, 11, 6, 7, 14, 15]);
        let lo128 = |a: V, b: V| simd_swizzle!(a, b, [0, 1, 2, 3, 8, 9, 10, 11]);
        let hi128 = |a: V, b: V| simd_swizzle!(a, b, [4, 5, 6, 7, 12, 13, 14, 15]);
        let t0 = lo32(r[0], r[1]);
        let t1 = hi32(r[0], r[1]);
        let t2 = lo32(r[2], r[3]);
        let t3 = hi32(r[2], r[3]);
        let t4 = lo32(r[4], r[5]);
        let t5 = hi32(r[4], r[5]);
        let t6 = lo32(r[6], r[7]);
        let t7 = hi32(r[6], r[7]);
        let u0 = lo64(t0, t2);
        let u1 = hi64(t0, t2);
        let u2 = lo64(t1, t3);
        let u3 = hi64(t1, t3);
        let u4 = lo64(t4, t6);
        let u5 = hi64(t4, t6);
        let u6 = lo64(t5, t7);
        let u7 = hi64(t5, t7);
        *r = [
            lo128(u0, u4),
            lo128(u1, u5),
            lo128(u2, u6),
            lo128(u3, u7),
            hi128(u0, u4),
            hi128(u1, u5),
            hi128(u2, u6),
            hi128(u3, u7),
        ];
    }

    /// One 8-point islow pass over 8 lanes. `bias` is added to the even part before the
    /// final arithmetic shift by `SHIFT`.
    #[inline(always)]
    fn pass<const SHIFT: i32>(v: &mut [V; 8], bias: V) {
        let z2 = v[2];
        let z3 = v[6];
        let z1 = (z2 + z3) * k(super::FIX_0_541196100);
        let tmp2 = z1 + z3 * k(-super::FIX_1_847759065);
        let tmp3 = z1 + z2 * k(super::FIX_0_765366865);
        let tmp0 = ((v[0] + v[4]) << 13) + bias;
        let tmp1 = ((v[0] - v[4]) << 13) + bias;
        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;

        let (t0, t1, t2, t3) = (v[7], v[5], v[3], v[1]);
        let z1 = t0 + t3;
        let z2 = t1 + t2;
        let z3 = t0 + t2;
        let z4 = t1 + t3;
        let z5 = (z3 + z4) * k(super::FIX_1_175875602);
        let t0 = t0 * k(super::FIX_0_298631336);
        let t1 = t1 * k(super::FIX_2_053119869);
        let t2 = t2 * k(super::FIX_3_072711026);
        let t3 = t3 * k(super::FIX_1_501321110);
        let z1 = z1 * k(-super::FIX_0_899976223);
        let z2 = z2 * k(-super::FIX_2_562915447);
        let z3 = z3 * k(-super::FIX_1_961570560) + z5;
        let z4 = z4 * k(-super::FIX_0_390180644) + z5;
        let t0 = t0 + z1 + z3;
        let t1 = t1 + z2 + z4;
        let t2 = t2 + z2 + z3;
        let t3 = t3 + z1 + z4;

        let s = V::splat(SHIFT);
        v[0] = (tmp10 + t3) >> s;
        v[7] = (tmp10 - t3) >> s;
        v[1] = (tmp11 + t2) >> s;
        v[6] = (tmp11 - t2) >> s;
        v[2] = (tmp12 + t1) >> s;
        v[5] = (tmp12 - t1) >> s;
        v[3] = (tmp13 + t0) >> s;
        v[4] = (tmp13 - t0) >> s;
    }

    /// islow 8x8; identical results to `idct8_scalar` for valid input.
    #[inline(always)]
    pub(crate) fn idct8(c: &[i16; 64], q: &[u16; 64], out: &mut [u8], off: usize, stride: usize) {
        let out = &mut out[off..off + 7 * stride + 8];
        let mut v = [V::splat(0); 8];
        for (r, vr) in v.iter_mut().enumerate() {
            *vr = load_row(c, q, r);
        }
        pass::<11>(&mut v, V::splat(1 << 10));
        transpose(&mut v);
        // Rounding for >> 18 plus the +128 level shift folded into the even part.
        pass::<18>(&mut v, V::splat((1 << 17) + (128 << 18)));
        transpose(&mut v);
        // Saturating narrow, two rows at a time (`vpackssdw` + `vpackuswb` shaped).
        for r in (0..8).step_by(2) {
            let both: Simd<i32, 16> = simd_swizzle!(
                v[r],
                v[r + 1],
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
            );
            let px = both
                .simd_clamp(Simd::splat(0), Simd::splat(255))
                .cast::<u8>();
            out[r * stride..r * stride + 8].copy_from_slice(&px.as_array()[..8]);
            out[(r + 1) * stride..(r + 1) * stride + 8].copy_from_slice(&px.as_array()[8..]);
        }
    }

    /// `jpeg_idct_4x4`; identical results to `idct4_scalar` for valid input.
    #[inline(always)]
    pub(crate) fn idct4(c: &[i16; 64], q: &[u16; 64], out: &mut [u8], off: usize, stride: usize) {
        let out = &mut out[off..off + 3 * stride + 4];
        // Pass 1: columns in lanes; row 4 does not contribute to a 4-point output.
        let r0 = load_row(c, q, 0);
        let r1 = load_row(c, q, 1);
        let r2 = load_row(c, q, 2);
        let r3 = load_row(c, q, 3);
        let r5 = load_row(c, q, 5);
        let r6 = load_row(c, q, 6);
        let r7 = load_row(c, q, 7);
        let tmp0 = (r0 << 14) + V::splat(1 << 11);
        let tmp2 = r2 * k(super::FIX_1_847759065) + r6 * k(-super::FIX_0_765366865);
        let tmp10 = tmp0 + tmp2;
        let tmp12 = tmp0 - tmp2;
        let t0 = r7 * k(-super::FIX_0_211164243)
            + r5 * k(super::FIX_1_451774981)
            + (r3 * k(-super::FIX_2_172734803) + r1 * k(super::FIX_1_061594337));
        let t2 = r7 * k(-super::FIX_0_509795579)
            + r5 * k(-super::FIX_0_601344887)
            + (r3 * k(super::FIX_0_899976223) + r1 * k(super::FIX_2_562915447));
        let s12 = V::splat(12);
        let w0 = (tmp10 + t2) >> s12;
        let w3 = (tmp10 - t2) >> s12;
        let w1 = (tmp12 + t0) >> s12;
        let w2 = (tmp12 - t0) >> s12;

        // Pass 2: rows in lanes (4 x i32); c_j = column j of rows 0..4.
        let (a, b) = w0.interleave(w1);
        let (cc, d) = w2.interleave(w3);
        let x0 = simd_swizzle!(a, cc, [0, 1, 8, 9, 2, 3, 10, 11]);
        let x1 = simd_swizzle!(a, cc, [4, 5, 12, 13, 6, 7, 14, 15]);
        let x2 = simd_swizzle!(b, d, [0, 1, 8, 9, 2, 3, 10, 11]);
        let x3 = simd_swizzle!(b, d, [4, 5, 12, 13, 6, 7, 14, 15]);
        let lo = |x: V| -> V4 { simd_swizzle!(x, [0, 1, 2, 3]) };
        let hi = |x: V| -> V4 { simd_swizzle!(x, [4, 5, 6, 7]) };
        let (c0, c1, c2, c3) = (lo(x0), hi(x0), lo(x1), hi(x1));
        let (c5, c6, c7) = (hi(x2), lo(x3), hi(x3));
        // Rounding for >> 19 plus the +128 level shift.
        let tmp0 = (c0 << 14) + V4::splat((1 << 18) + (128 << 19));
        let tmp2 = c2 * k4(super::FIX_1_847759065) + c6 * k4(-super::FIX_0_765366865);
        let tmp10 = tmp0 + tmp2;
        let tmp12 = tmp0 - tmp2;
        let t0 = c7 * k4(-super::FIX_0_211164243)
            + c5 * k4(super::FIX_1_451774981)
            + (c3 * k4(-super::FIX_2_172734803) + c1 * k4(super::FIX_1_061594337));
        let t2 = c7 * k4(-super::FIX_0_509795579)
            + c5 * k4(-super::FIX_0_601344887)
            + (c3 * k4(super::FIX_0_899976223) + c1 * k4(super::FIX_2_562915447));
        let s19 = V4::splat(19);
        let o0 = (tmp10 + t2) >> s19;
        let o3 = (tmp10 - t2) >> s19;
        let o1 = (tmp12 + t0) >> s19;
        let o2 = (tmp12 - t0) >> s19;
        // o_k[r] is output (row r, column k): transpose back to rows.
        let o01 = simd_swizzle!(o0, o1, [0, 4, 1, 5, 2, 6, 3, 7]);
        let o23 = simd_swizzle!(o2, o3, [0, 4, 1, 5, 2, 6, 3, 7]);
        let rows01 = simd_swizzle!(o01, o23, [0, 1, 8, 9, 2, 3, 10, 11]);
        let rows23 = simd_swizzle!(o01, o23, [4, 5, 12, 13, 6, 7, 14, 15]);
        let px = |x: V| {
            x.simd_clamp(V::splat(0), V::splat(255))
                .cast::<u8>()
                .to_array()
        };
        let (p01, p23) = (px(rows01), px(rows23));
        out[..4].copy_from_slice(&p01[..4]);
        out[stride..stride + 4].copy_from_slice(&p01[4..]);
        out[2 * stride..2 * stride + 4].copy_from_slice(&p23[..4]);
        out[3 * stride..3 * stride + 4].copy_from_slice(&p23[4..]);
    }

    /// `jpeg_idct_2x2`; identical results to `idct2_scalar` for valid input.
    #[inline(always)]
    pub(crate) fn idct2(c: &[i16; 64], q: &[u16; 64], out: &mut [u8], off: usize, stride: usize) {
        // Pass 1: columns in lanes; only rows 0, 1, 3, 5, 7 contribute.
        let r0 = load_row(c, q, 0);
        let r1 = load_row(c, q, 1);
        let r3 = load_row(c, q, 3);
        let r5 = load_row(c, q, 5);
        let r7 = load_row(c, q, 7);
        let tmp10 = (r0 << 15) + V::splat(1 << 12);
        let tmp0 = r7 * k(-super::FIX_0_720959822)
            + r5 * k(super::FIX_0_850430095)
            + (r3 * k(-super::FIX_1_272758580) + r1 * k(super::FIX_3_624509785));
        let w0 = (tmp10 + tmp0) >> V::splat(13);
        let w1 = (tmp10 - tmp0) >> V::splat(13);
        // Pass 2: per row, a dot product of columns 1, 3, 5, 7 with the odd constants.
        let kk = V::from_array([
            0,
            super::FIX_3_624509785 as i32,
            0,
            -super::FIX_1_272758580 as i32,
            0,
            super::FIX_0_850430095 as i32,
            0,
            -super::FIX_0_720959822 as i32,
        ]);
        let s0 = (w0 * kk).reduce_sum();
        let s1 = (w1 * kk).reduce_sum();
        let (d0, d1) = (w0[0], w1[0]);
        let px = |d: i32, t: i32| {
            let v = (d << 15).wrapping_add(t).wrapping_add(1 << 19) >> 20;
            (v + 128).clamp(0, 255) as u8
        };
        out[off] = px(d0, s0);
        out[off + 1] = px(d0, s0.wrapping_neg());
        out[off + stride] = px(d1, s1);
        out[off + stride + 1] = px(d1, s1.wrapping_neg());
    }
}

/// Which IDCT implementation to use; chosen once per decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Simd {
    Scalar,
    /// Portable SIMD (`std::simd`).
    Vector,
}

impl Simd {
    pub fn detect() -> Simd {
        if std::env::var_os("SPECULAR_JPEG_NO_SIMD").is_some() {
            Simd::Scalar
        } else {
            Simd::Vector
        }
    }

    #[inline(always)]
    pub fn idct8(self, c: &[i16; 64], q: &[u16; 64], out: &mut [u8], off: usize, stride: usize) {
        match self {
            Simd::Scalar => idct8_scalar(c, q, out, off, stride),
            Simd::Vector => vector::idct8(c, q, out, off, stride),
        }
    }

    #[inline(always)]
    pub fn idct4(self, c: &[i16; 64], q: &[u16; 64], out: &mut [u8], off: usize, stride: usize) {
        match self {
            Simd::Scalar => idct4_scalar(c, q, out, off, stride),
            Simd::Vector => vector::idct4(c, q, out, off, stride),
        }
    }

    #[inline(always)]
    pub fn idct2(self, c: &[i16; 64], q: &[u16; 64], out: &mut [u8], off: usize, stride: usize) {
        match self {
            Simd::Scalar => idct2_scalar(c, q, out, off, stride),
            Simd::Vector => vector::idct2(c, q, out, off, stride),
        }
    }
}

/// Where a block's output samples go: `buf[off..]`, rows `stride` apart.
pub(crate) struct Dst<'a> {
    pub buf: &'a mut [u8],
    pub off: usize,
    pub stride: usize,
}

/// Inverse transform one block at output size `n` (8, 4, 2 or 1) for 8-bit data.
#[inline(always)]
pub(crate) fn idct_block(simd: Simd, n: usize, c: &[i16; 64], q: &[u16; 64], has_ac: bool, d: Dst) {
    let Dst {
        buf: out,
        off,
        stride,
    } = d;
    if !has_ac {
        fill_block(n, dc_only_value(c[0], q[0]), out, off, stride);
        return;
    }
    match n {
        8 => simd.idct8(c, q, out, off, stride),
        4 => simd.idct4(c, q, out, off, stride),
        2 => simd.idct2(c, q, out, off, stride),
        _ => out[off] = dc_only_value(c[0], q[0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_matches_scalar() {
        let mut seed = 12345u32;
        let mut rnd = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed >> 8
        };
        for iter in 0..20000 {
            let mut c = [0i16; 64];
            let mut q = [0u16; 64];
            let density = iter % 64;
            for i in 0..64 {
                q[i] = 1 + (rnd() % 64) as u16;
                if i == 0 || (rnd() % 64) < density as u32 {
                    let mag = if i == 0 {
                        2047 / q[i] as i32
                    } else {
                        1023 / q[i] as i32
                    };
                    c[i] = ((rnd() % (2 * mag as u32 + 1)) as i32 - mag) as i16;
                }
            }
            let mut a = [0u8; 64];
            let mut b = [0u8; 64];
            idct8_scalar(&c, &q, &mut a, 0, 8);
            Simd::Vector.idct8(&c, &q, &mut b, 0, 8);
            assert_eq!(a, b, "idct8 iter {iter}");
            let (mut a, mut b) = ([0u8; 64], [0u8; 64]);
            idct4_scalar(&c, &q, &mut a, 3, 9);
            Simd::Vector.idct4(&c, &q, &mut b, 3, 9);
            assert_eq!(a, b, "idct4 iter {iter}");
            let (mut a, mut b) = ([0u8; 64], [0u8; 64]);
            idct2_scalar(&c, &q, &mut a, 5, 7);
            Simd::Vector.idct2(&c, &q, &mut b, 5, 7);
            assert_eq!(a, b, "idct2 iter {iter}");
        }
    }
}
