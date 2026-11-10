//! Rate-distortion optimised quantisation for CABAC ("trellis"): picks each block's levels
//! by Viterbi search over the states of the level coding contexts, minimising
//! `(x − level)² + λ·bits` with `x` the unquantised (fractional) level. Bits are estimated
//! from the slice's current context states, frozen for the block.

use crate::cabac::{
    BIN_COST, BIN_COST_PAIR, CAT_CHROMA_DC, NUM_CTX, cbf_ctx, residual_ctx, sig_last_inc,
};
use crate::transform::MAX_LEVEL;

use std::simd::cmp::SimdPartialOrd;
use std::simd::num::{SimdFloat, SimdUint};
use std::simd::{Mask, Select, Simd, simd_swizzle};

type F32x8 = Simd<f32, 8>;
type F32x16 = Simd<f32, 16>;
type U32x8 = Simd<u32, 8>;

/// Context states of the coeff_abs_level_minus1 coding (in reverse scan order): nothing
/// coded yet, then (numDecodAbsLevelGt1 = 0, numDecodAbsLevelEq1 = 1, 2, ≥ 3), then
/// numDecodAbsLevelGt1 = 1, 2, 3, ≥ 4.
const STATES: usize = 8;
/// ctxIdxInc of the first bin per state.
const INC0: [usize; STATES] = [1, 2, 3, 4, 0, 0, 0, 0];
/// ctxIdxInc of the further bins per state (5 + min(numDecodAbsLevelGt1, 4)).
const INC_GT1: [usize; STATES] = [5, 5, 5, 5, 6, 7, 8, 9];
// Next state after a level of 1: [1, 2, 3, 3, 4, 5, 6, 7]; after one above 1:
// [4, 4, 4, 4, 5, 6, 7, 7] (the swizzles in `quantise`).

#[inline]
fn cost(ctx: &[u8; NUM_CTX], i: usize, b: bool) -> u32 {
    u32::from(BIN_COST[usize::from(ctx[i])][usize::from(b)])
}

/// Bits (1/256) of exp-Golomb (k = 0) bypass suffix value `v`.
#[inline]
fn eg0_bits(v: u32) -> u32 {
    (2 * (32 - (v + 1).leading_zeros()) - 1) * 256
}

/// The last position of `x` whose magnitude rounds to a level (≥ ½), if any.
#[inline]
fn last_significant(x: &[f32]) -> Option<usize> {
    let half = F32x16::splat(0.5);
    let mut mask = 0u64;
    let mut chunks = x.chunks_exact(16);
    for (k, c) in chunks.by_ref().enumerate() {
        mask |= F32x16::from_slice(c).abs().simd_ge(half).to_bitmask() << (16 * k);
    }
    let rest = chunks.remainder();
    if !rest.is_empty() {
        let mut a = [0f32; 16];
        a[..rest.len()].copy_from_slice(rest);
        let base = x.len() - rest.len();
        mask |= F32x16::from_array(a).abs().simd_ge(half).to_bitmask() << base;
    }
    (mask != 0).then(|| 63 - mask.leading_zeros() as usize)
}

/// The levels (scan order, written into `out`) of a residual block of category `cat` whose
/// unquantised levels are `x` (scan order, `x.len()` = maxNumCoeff), costing `lambda`
/// (squared level units per 1/256 bit). `cbf_inc` is the coded_block_flag increment, `None`
/// where the flag is not coded (8×8 luma). Returns whether any level is non-zero.
pub fn quantise(
    ctx: &[u8; NUM_CTX],
    cat: usize,
    cbf_inc: Option<usize>,
    x: &[f32],
    lambda: f32,
    out: &mut [i16],
) -> bool {
    debug_assert!(cat != CAT_CHROMA_DC);
    let n = x.len();
    out[..n].fill(0);
    // Last coefficient worth considering: past it everything rounds to zero anyway.
    let Some(end) = last_significant(x) else {
        return false;
    };
    let (sig_base, last_base, abs_base) = residual_ctx(cat);
    // Both bin costs (packed, `b` = 1 in the upper half) of the ten abs contexts, then per
    // state (lane) the first bin's costs and the further bins'.
    let pair = Simd::<u32, 16>::from_array(std::array::from_fn(|k| {
        if k < 10 {
            BIN_COST_PAIR[usize::from(ctx[abs_base + k])]
        } else {
            0
        }
    }));
    let first: U32x8 = simd_swizzle!(pair, INC0);
    let more: U32x8 = simd_swizzle!(pair, INC_GT1);
    let lo = U32x8::splat(0xffff);
    let (first0, first1) = (first & lo, first >> 16);
    let (more0, more1) = (more & lo, more >> 16);
    // Bits of level magnitude `l` ≥ 1 (with sign) per state.
    let bits1 = first0 + U32x8::splat(256);
    let bits2 = first1 + more0 + U32x8::splat(256);
    let level_bits = |l: u32| -> U32x8 {
        match l {
            1 => return bits1,
            2 => return bits2,
            _ => {}
        }
        let a = l - 1;
        let prefix = a.min(14);
        let b = first1 + U32x8::splat(prefix - 1) * more1 + U32x8::splat(256);
        if a < 14 {
            b + more0
        } else {
            b + U32x8::splat(eg0_bits(a - 14))
        }
    };

    const INF: f32 = f32::INFINITY;
    let inf = F32x8::splat(INF);
    let mut j = F32x8::from_array([0.0, INF, INF, INF, INF, INF, INF, INF]);
    let lam = F32x8::splat(lambda);
    let state = U32x8::from_array([0, 1, 2, 3, 4, 5, 6, 7]);
    // Back pointers per scan position and state: the previous state, and the level as 0 or
    // its offset from `lo` + 1 (bits 3 and up).
    let mut back = [[0u8; STATES]; 64];
    // Distortion of the zeros after `end` is common to all paths: left out.
    for i in (0..=end).rev() {
        let q = x[i].abs();
        let hi = ((q + 0.5) as u32).min(MAX_LEVEL as u32);
        let coded = i + 1 < n;
        let (si, li) = sig_last_inc(cat, i);
        let sig0 = if coded {
            cost(ctx, sig_base + si, false)
        } else {
            0
        };
        let d0 = F32x8::splat(q * q);
        // Level 0: not started stays so for free; others code a 0 significance flag.
        let zero_bits =
            F32x8::from_array([0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]) * F32x8::splat(sig0 as f32);
        let mut nj = (j + d0) + lam * zero_bits;
        let mut nb = state;
        if hi == 0 {
            j = nj;
            back[i] = nb.cast::<u8>().to_array();
            continue;
        }
        let (s1, l1) = if coded {
            (
                cost(ctx, sig_base + si, true),
                [
                    cost(ctx, last_base + li, false),
                    cost(ctx, last_base + li, true),
                ],
            )
        } else {
            (0, [0, 0])
        };
        // Levels: each state's cost moves to its next state; where several states lead to
        // the same one, the lowest state wins ties (`<` in state order).
        let relax = |nj: &mut F32x8, nb: &mut U32x8, c: F32x8, from: U32x8| {
            let m = c.simd_lt(*nj);
            *nj = m.select(c, *nj);
            *nb = m.select(from, *nb);
        };
        let flags = U32x8::splat(s1)
            + U32x8::from_array([l1[1], l1[0], l1[0], l1[0], l1[0], l1[0], l1[0], l1[0]]);
        let lo = hi.saturating_sub(1).max(1);
        for l in lo..=hi {
            let d = F32x8::splat((q - l as f32) * (q - l as f32));
            let c = (j + d) + lam * (flags + level_bits(l)).cast::<f32>();
            let from = state | U32x8::splat((l - lo + 1) << 3);
            // Lanes past the first take the cost from the state they name.
            let pick = |c: F32x8, k: [bool; 8]| Mask::<i32, 8>::from_array(k).select(c, inf);
            if l == 1 {
                // 0→1, 1→2, 2→3, 3→3, 4..7 stay.
                let a = simd_swizzle!(c, [0, 0, 1, 2, 4, 5, 6, 7]);
                let fa = simd_swizzle!(from, [0, 0, 1, 2, 4, 5, 6, 7]);
                relax(
                    &mut nj,
                    &mut nb,
                    pick(a, [false, true, true, true, true, true, true, true]),
                    fa,
                );
                let b = simd_swizzle!(c, [3, 3, 3, 3, 3, 3, 3, 3]);
                let fb = simd_swizzle!(from, [3, 3, 3, 3, 3, 3, 3, 3]);
                relax(
                    &mut nj,
                    &mut nb,
                    pick(b, [false, false, false, true, false, false, false, false]),
                    fb,
                );
            } else {
                // 0..3→4, 4→5, 5→6, 6→7, 7→7. The earliest of equal costs wins, as when
                // relaxing one source after the other, so the sources are paired up first:
                // state 4 takes the better of (0, 1) against the better of (2, 3), state 7
                // the better of (6, 7). Lane 0 holds the (2, 3) pair on the way.
                let a = pick(
                    simd_swizzle!(c, [2, 0, 0, 0, 0, 4, 5, 6]),
                    [true, false, false, false, true, true, true, true],
                );
                let fa = simd_swizzle!(from, [2, 0, 0, 0, 0, 4, 5, 6]);
                let b = pick(
                    simd_swizzle!(c, [3, 0, 0, 0, 1, 7, 7, 7]),
                    [true, false, false, false, true, false, false, true],
                );
                let fb = simd_swizzle!(from, [3, 0, 0, 0, 1, 7, 7, 7]);
                let m = b.simd_lt(a);
                let (e, fe) = (m.select(b, a), m.select(fb, fa));
                let z = pick(
                    simd_swizzle!(e, [0, 0, 0, 0, 0, 0, 0, 0]),
                    [false, false, false, false, true, false, false, false],
                );
                let fz = simd_swizzle!(fe, [0, 0, 0, 0, 0, 0, 0, 0]);
                let m = z.simd_lt(e);
                let (g, fg) = (m.select(z, e), m.select(fz, fe));
                relax(
                    &mut nj,
                    &mut nb,
                    pick(g, [false, false, false, false, true, true, true, true]),
                    fg,
                );
            }
        }
        j = nj;
        back[i] = nb.cast::<u8>().to_array();
    }
    let j = j.to_array();
    let (c0, c1) = match cbf_inc {
        Some(inc) => {
            let k = cbf_ctx(cat, inc);
            (cost(ctx, k, false), cost(ctx, k, true))
        }
        None => (0, 0),
    };
    let mut best = (j[0] + lambda * c0 as f32, 0usize);
    for (s, &v) in j.iter().enumerate().skip(1) {
        let c = v + lambda * c1 as f32;
        if c < best.0 {
            best = (c, s);
        }
    }
    if best.1 == 0 {
        return false;
    }
    let mut s = best.1;
    for i in 0..=end {
        let b = back[i][s];
        if b >> 3 != 0 {
            let hi = ((x[i].abs() + 0.5) as u32).min(MAX_LEVEL as u32);
            let l = (hi.saturating_sub(1).max(1) + u32::from(b >> 3) - 1) as i16;
            out[i] = if x[i] < 0.0 { -l } else { l };
        }
        s = usize::from(b & 7);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cabac::{CAT_LUMA_4X4, CabacEnc};

    fn ctx() -> [u8; NUM_CTX] {
        *CabacEnc::new(crate::bits::BitWriter::new(), 0, 26).contexts()
    }

    #[test]
    fn zero_lambda_rounds_to_nearest() {
        let x = [
            3.4f32, -2.6, 0.7, 0.2, -1.6, 0.0, 0.0, 0.49, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        let mut out = [0i16; 16];
        assert!(quantise(&ctx(), CAT_LUMA_4X4, Some(0), &x, 0.0, &mut out));
        assert_eq!(&out[..8], &[3, -3, 1, 0, -2, 0, 0, 0]);
    }

    #[test]
    fn large_lambda_zeroes_small_levels() {
        let mut x = [0f32; 16];
        x[9] = 0.9;
        let mut out = [0i16; 16];
        assert!(!quantise(&ctx(), CAT_LUMA_4X4, Some(0), &x, 0.05, &mut out));
        assert_eq!(out, [0; 16]);
    }
}
