//! Inverse DCT-II / DST-VII and transform-skip residual derivation,
//! ITU-T H.265 clause 8.6.4.2 ("Transformation process for scaled transform
//! coefficients") and the final bit-depth shift of clause 8.6.2.
//!
//! Written from the specification text alone. No code, table layout or
//! butterfly schedule was taken from any existing decoder. The matrices
//! themselves are built in `matrix`, which is the only place the standard's
//! tabulated cosine appears.

#[path = "dct_matrix.rs"]
mod matrix;

#[cfg(test)]
pub(crate) use matrix::{DCT_MATRIX, DST_MATRIX};
use matrix::{M4, M8, M16, M32, MDST};

use super::extent::extent;

/// Lower bound `coeffMin` on an intermediate transform coefficient (16 bit).
const COEFF_MIN: i32 = -32768;
/// Upper bound `coeffMax` on an intermediate transform coefficient (16 bit).
const COEFF_MAX: i32 = 32767;
/// Right shift applied between the two one-dimensional stages (clause 8.6.4.2).
const STAGE1_SHIFT: u32 = 7;
/// Lowest bit depth this module accepts; below it `bdShift` would be undefined.
const MIN_BIT_DEPTH: u8 = 8;
/// Highest bit depth this module accepts (H.265 range extensions cap).
const MAX_BIT_DEPTH: u8 = 16;

/// `Clip3(coeffMin, coeffMax, v)` of clause 5, written without `clamp` so that
/// the transform keeps no panicking path at all.
#[allow(clippy::manual_clamp)]
fn clip_coeff(v: i32) -> i32 {
    if v < COEFF_MIN {
        COEFF_MIN
    } else if v > COEFF_MAX {
        COEFF_MAX
    } else {
        v
    }
}

/// `bdShift` of clause 8.6.2, or `None` when the bit depth is out of range.
const fn bd_shift_of(bit_depth: u8) -> Option<u32> {
    if bit_depth >= MIN_BIT_DEPTH && bit_depth <= MAX_BIT_DEPTH {
        Some(20 - bit_depth as u32)
    } else {
        None
    }
}

/// One 1-D inverse transform: `out[i] = sum_j m[j][i] * src[j]`.
///
/// The sum index is the *row* of `m`, i.e. this multiplies by the transpose,
/// exactly as clause 8.6.4.2 writes it. Accumulating over `j` on the outside
/// keeps the inner loop a contiguous multiply-accumulate over two slices.
///
/// `terms` is how many of `src`'s entries may be non-zero; the rest add
/// nothing and are not read. Dropping them is exact, not an approximation.
/// With `SKIP` set, a zero term inside that range is skipped too, which pays
/// on the sparse large blocks and costs a mispredicted branch on dense 4x4s.
///
/// The largest attainable magnitude is `32 * 32768 * 90 = 94_371_840`, well
/// inside `i32`, because both stages take 16-bit-clipped inputs.
fn mul_1d<const N: usize, const SKIP: bool>(
    m: &[[i32; N]; N],
    src: &[i32; N],
    terms: usize,
) -> [i32; N] {
    let mut acc = [0i32; N];
    for j in 0..core::cmp::min(terms, N) {
        let c = src[j];
        if SKIP && c == 0 {
            continue;
        }
        let row = &m[j];
        for i in 0..N {
            acc[i] += row[i] * c;
        }
    }
    acc
}

/// The two-dimensional inverse transform for one `N`x`N` block.
///
/// `block` is row-major with `d[x][y]` at `block[y * N + x]`; it is overwritten
/// with the residual `r[x][y]` at the same position.
///
/// With `BOUNDED` set, both stages sum only over the rectangle that holds the
/// non-zero coefficients. A 4x4 block is too small for the scan to pay for
/// itself, so it runs the full, branch-free matrix product instead; the two
/// give identical results because the skipped terms are all zero.
fn two_stage<const N: usize, const BOUNDED: bool>(
    block: &mut [i32],
    m: &[[i32; N]; N],
    bd_shift: u32,
) {
    let b = &mut block[..N * N];
    let (rows, cols) = if BOUNDED { extent(b, N) } else { (N, N) };
    let rnd = 1i32 << (bd_shift - 1);
    if BOUNDED && rows == 1 {
        // Only the first coefficient row is non-zero, so the column stage
        // gives every row of g the same values, m[0][y] * d[x] with m[0][y]
        // equal to m[0][0] for every y of the DCT. The row stage therefore
        // produces one residual row, repeated down the block.
        let mut g0 = [0i32; N];
        for (g, &c) in g0.iter_mut().zip(b.iter()).take(cols) {
            *g = clip_coeff((m[0][0] * c + (1 << (STAGE1_SHIFT - 1))) >> STAGE1_SHIFT);
        }
        let mut row = mul_1d::<N, true>(m, &g0, cols);
        for v in row.iter_mut() {
            *v = (*v + rnd) >> bd_shift;
        }
        for out in b.chunks_exact_mut(N) {
            out.copy_from_slice(&row);
        }
        return;
    }
    // g[y][x], the clipped output of the column stage. Columns at or past
    // `cols` have an all-zero source, so they stay zero and are not computed;
    // the second stage is then told to stop summing there.
    let mut g = [[0i32; N]; N];
    let mut col = [0i32; N];
    for x in 0..cols {
        for j in 0..rows {
            col[j] = b[j * N + x];
        }
        let e = mul_1d::<N, BOUNDED>(m, &col, rows);
        for y in 0..N {
            g[y][x] = clip_coeff((e[y] + (1 << (STAGE1_SHIFT - 1))) >> STAGE1_SHIFT);
        }
    }
    for y in 0..N {
        if BOUNDED && cols == 1 {
            // One column of g means every sample of row y is m[0][i] * g[y][0]
            // with m[0][i] flat across the DCT's first basis row.
            b[y * N..y * N + N].fill((m[0][0] * g[y][0] + rnd) >> bd_shift);
            continue;
        }
        let r = mul_1d::<N, BOUNDED>(m, &g[y], cols);
        for i in 0..N {
            b[y * N + i] = (r[i] + rnd) >> bd_shift;
        }
    }
}

/// One 1-D inverse DCT of `src` (as [`mul_1d`]), using the symmetry of the
/// DCT basis: `m[j][N - 1 - i]` is `m[j][i]` for even `j` and `-m[j][i]`
/// for odd `j`. Summing the even and odd terms separately over the first
/// `H = N / 2` outputs gives both halves, `even + odd` and, mirrored,
/// `even - odd`, for half the multiplies. Integer sums in another order:
/// the result is exactly [`mul_1d`]'s.
///
/// Entries of `src` at or past `terms` must be zero (they are read in pairs).
#[inline(always)]
fn dct_1d<const N: usize, const H: usize>(
    m: &[[i32; N]; N],
    src: &[i32; N],
    terms: usize,
) -> [i32; N] {
    let mut e = [0i32; H];
    let mut o = [0i32; H];
    for j in (0..terms.min(N)).step_by(2) {
        let (ce, co) = (src[j], src[j + 1]);
        let (re, ro) = (&m[j], &m[j + 1]);
        for i in 0..H {
            e[i] += re[i] * ce;
            o[i] += ro[i] * co;
        }
    }
    let mut out = [0i32; N];
    for i in 0..H {
        out[i] = e[i] + o[i];
        out[N - 1 - i] = e[i] - o[i];
    }
    out
}

/// [`two_stage`] for the DCT, built on the basis symmetry of [`dct_1d`].
///
/// The column stage runs across `C` columns at a time, each row of the
/// block being contiguous, rather than gathering one column at a time.
fn dct_two_stage<const N: usize, const H: usize, const C: usize>(
    block: &mut [i32],
    m: &[[i32; N]; N],
    bd_shift: u32,
) {
    let b = &mut block[..N * N];
    let (rows, cols) = extent(b, N);
    if rows == 0 {
        b.fill(0);
        return;
    }
    if rows == 1 || cols == 1 {
        two_stage::<N, true>(b, m, bd_shift);
        return;
    }
    let rnd = 1i32 << (bd_shift - 1);
    let mut g = [[0i32; N]; N];
    // Rows of the source at or past `rows` are zero, and are read in pairs.
    let pairs = rows.div_ceil(2);
    for x0 in (0..cols).step_by(C) {
        for y in 0..H {
            let mut e = [0i32; C];
            let mut o = [0i32; C];
            for p in 0..pairs {
                let j = 2 * p;
                let (me, mo) = (m[j][y], m[j + 1][y]);
                let se: &[i32; C] = b[j * N + x0..][..C].try_into().unwrap();
                let so: &[i32; C] = b[(j + 1) * N + x0..][..C].try_into().unwrap();
                for i in 0..C {
                    e[i] += me * se[i];
                    o[i] += mo * so[i];
                }
            }
            let (top, bot) = g.split_at_mut(N - 1 - y);
            let gt: &mut [i32; C] = (&mut top[y][x0..][..C]).try_into().unwrap();
            let gb: &mut [i32; C] = (&mut bot[0][x0..][..C]).try_into().unwrap();
            for i in 0..C {
                gt[i] = clip_coeff((e[i] + o[i] + (1 << (STAGE1_SHIFT - 1))) >> STAGE1_SHIFT);
                gb[i] = clip_coeff((e[i] - o[i] + (1 << (STAGE1_SHIFT - 1))) >> STAGE1_SHIFT);
            }
        }
    }
    // Columns of g at or past `cols` are zero: their source columns are
    // (a chunk running past `cols` computes them as zero, too).
    for (y, gy) in g.iter().enumerate() {
        let r = dct_1d::<N, H>(m, gy, cols);
        let out: &mut [i32; N] = (&mut b[y * N..][..N]).try_into().unwrap();
        for i in 0..N {
            out[i] = (r[i] + rnd) >> bd_shift;
        }
    }
}

/// Applies the two-dimensional inverse transform in place (clause 8.6.4.2)
/// followed by the final `bdShift` of clause 8.6.2.
///
/// `block` holds the scaled transform coefficients d[x][y] at
/// `block[y * n + x]` and receives the residual samples r[x][y].
/// `tr_type` is 0 for DCT-II and 1 for the 4x4 DST-VII.
///
/// `block` is left untouched when `n` is not 4, 8, 16 or 32, when `tr_type` is
/// 1 and `n` is not 4, when `bit_depth` is outside 8..=16, or when `block` is
/// shorter than `n * n`.
pub fn inverse_transform(block: &mut [i32], n: usize, tr_type: u8, bit_depth: u8) {
    let bd_shift = match bd_shift_of(bit_depth) {
        Some(s) => s,
        None => return,
    };
    match (tr_type, n) {
        (1, 4) if block.len() >= 16 => two_stage::<4, false>(block, &MDST, bd_shift),
        (0, 4) if block.len() >= 16 => two_stage::<4, false>(block, &M4, bd_shift),
        (0, 8) if block.len() >= 64 => dct_two_stage::<8, 4, 8>(block, &M8, bd_shift),
        (0, 16) if block.len() >= 256 => dct_two_stage::<16, 8, 8>(block, &M16, bd_shift),
        (0, 32) if block.len() >= 1024 => dct_two_stage::<32, 16, 8>(block, &M32, bd_shift),
        _ => {}
    }
}

/// Transform-skip residual derivation (clause 8.6.2) for one block.
///
/// `block` holds d[x][y] and receives r[x][y]. `rotate` is `rotateCoeffs`,
/// which the caller derives (`transform_skip_rotation_enabled_flag` and a
/// 4x4 intra block).
///
/// `block` is left untouched when `n` is not 4, 8, 16 or 32, when `bit_depth`
/// is outside 8..=16, or when `block` is shorter than `n * n`.
pub fn transform_skip(block: &mut [i32], n: usize, bit_depth: u8, rotate: bool) {
    if let Some(bd_shift) = bd_shift_of(bit_depth) {
        skip_shift(block, n, bd_shift, rotate);
    }
}

/// [`transform_skip`] under `extended_precision_processing_flag`, where
/// `bdShift` is `Max(20 - bitDepth, 11)` (equation 8-297).
pub fn transform_skip_ext(block: &mut [i32], n: usize, bit_depth: u8, rotate: bool) {
    if let Some(bd_shift) = bd_shift_of(bit_depth) {
        skip_shift(block, n, bd_shift.max(11), rotate);
    }
}

/// Equations 8-300 and 8-301 for one block, with a given `bdShift`.
fn skip_shift(block: &mut [i32], n: usize, bd_shift: u32, rotate: bool) {
    let log2_n: u32 = match n {
        4 => 2,
        8 => 3,
        16 => 4,
        32 => 5,
        _ => return,
    };
    if block.len() < n * n {
        return;
    }
    let b = &mut block[..n * n];
    // Reading d[n-1-x][n-1-y] into r[x][y] is exactly a reversal of the
    // row-major block, since (n-1-y) * n + (n-1-x) == n * n - 1 - (y * n + x).
    if rotate {
        b.reverse();
    }
    let ts_shift = 5 + log2_n;
    let rnd = 1i64 << (bd_shift - 1);
    for v in b.iter_mut() {
        // The coefficients are at most 23 bits wide even with extended
        // precision, so the product stays far inside `i64`.
        *v = ((((*v as i64) << ts_shift) + rnd) >> bd_shift).clamp(i32::MIN as i64, i32::MAX as i64)
            as i32;
    }
}

/// The two-dimensional inverse transform under
/// `extended_precision_processing_flag` (clauses 8.6.2 and 8.6.4).
///
/// The intermediate values are clipped to `log2TransformRange` bits instead
/// of 16, which no longer fits the 32-bit accumulation of
/// [`inverse_transform`], and `bdShift` is `Max(20 - bitDepth, 11)`. This
/// path is only taken by the rare streams that enable the flag, so it is the
/// plain matrix product in 64-bit arithmetic.
///
/// `block` is left untouched on the same conditions as [`inverse_transform`],
/// and also when `log2_range` is outside 15..=22.
pub fn inverse_transform_ext(
    block: &mut [i32],
    n: usize,
    tr_type: u8,
    bit_depth: u8,
    log2_range: u32,
) {
    let bd_shift = match bd_shift_of(bit_depth) {
        Some(s) => s.max(11),
        None => return,
    };
    if !(15..=22).contains(&log2_range) {
        return;
    }
    match (tr_type, n) {
        (1, 4) => wide::<4>(block, &MDST, bd_shift, log2_range),
        (0, 4) => wide::<4>(block, &M4, bd_shift, log2_range),
        (0, 8) => wide::<8>(block, &M8, bd_shift, log2_range),
        (0, 16) => wide::<16>(block, &M16, bd_shift, log2_range),
        (0, 32) => wide::<32>(block, &M32, bd_shift, log2_range),
        _ => {}
    }
}

/// 64-bit two-stage transform with a `log2_range`-bit intermediate clip.
fn wide<const N: usize>(block: &mut [i32], m: &[[i32; N]; N], bd_shift: u32, log2_range: u32) {
    if block.len() < N * N {
        return;
    }
    let b = &mut block[..N * N];
    let (lo, hi) = (-(1i64 << log2_range), (1i64 << log2_range) - 1);
    // g[y][x], the clipped output of the column stage (equation 8-316).
    let mut g = [[0i64; N]; N];
    for x in 0..N {
        for (y, gy) in g.iter_mut().enumerate() {
            let mut acc = 0i64;
            for (j, row) in m.iter().enumerate() {
                acc += row[y] as i64 * b[j * N + x] as i64;
            }
            gy[x] = ((acc + (1 << (STAGE1_SHIFT - 1))) >> STAGE1_SHIFT).clamp(lo, hi);
        }
    }
    let rnd = 1i64 << (bd_shift - 1);
    for (y, gy) in g.iter().enumerate() {
        for i in 0..N {
            let mut acc = 0i64;
            for (row, &gv) in m.iter().zip(gy.iter()) {
                acc += row[i] as i64 * gv;
            }
            b[y * N + i] = ((acc + rnd) >> bd_shift).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        }
    }
}

#[cfg(test)]
#[path = "dct_ref.rs"]
mod reference;

#[cfg(test)]
#[path = "dct_tests.rs"]
mod tests;
