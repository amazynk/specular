//! Scaling of transform coefficients (clause 8.6.3).

use super::extent::extent;

/// `levelScale[]` from clause 8.6.3.
pub const LEVEL_SCALE: [i64; 6] = [40, 45, 51, 57, 64, 72];

/// Smallest representable scaled coefficient, `CoeffMinY` / `CoeffMinC`.
pub const COEFF_MIN: i64 = -32768;
/// Largest representable scaled coefficient, `CoeffMaxY` / `CoeffMaxC`.
pub const COEFF_MAX: i64 = 32767;

/// Scales `levels` in place, turning `TransCoeffLevel` into `d[x][y]`.
///
/// `qp` is `Qp'Y` or `Qp'Cb` / `Qp'Cr`, `n` the transform block size and
/// `factors` the `ScalingFactor` block, or `None` for a flat factor of 16.
///
/// Only the rectangle that holds the non-zero levels is touched: a zero level
/// scales to `(0 + (1 << (bdShift - 1))) >> bdShift`, which is zero for every
/// `bdShift` in range, so the rest of the block is already its own result.
///
/// `levels` is left untouched when it is shorter than `n * n`, when `n` is
/// zero, or when `factors` is present but shorter than `n * n`.
#[cfg(test)]
pub fn scale(levels: &mut [i32], n: usize, qp: i32, bit_depth: u8, factors: Option<&[u8]>) {
    scale_range(levels, n, qp, bit_depth, factors, 15);
}

/// [`scale`] with a given `log2TransformRange` (equations 8-302 to 8-309):
/// 15 normally, `Max(15, bitDepth + 6)` under
/// `extended_precision_processing_flag`. `CoeffMin` and `CoeffMax` are
/// `-(1 << log2_range)` and `(1 << log2_range) - 1`.
///
/// `levels` is also left untouched when `log2_range` is outside 15..=22.
pub fn scale_range(
    levels: &mut [i32],
    n: usize,
    qp: i32,
    bit_depth: u8,
    factors: Option<&[u8]>,
    log2_range: u32,
) {
    // The common range gets its own copy with constant clip bounds, kept
    // out of line like the original `scale`.
    match log2_range {
        15 => scale_impl::<true>(levels, n, qp, bit_depth, factors, 15),
        16..=22 => scale_impl::<false>(levels, n, qp, bit_depth, factors, log2_range),
        _ => {}
    }
}

#[inline(never)]
fn scale_impl<const MAIN: bool>(
    levels: &mut [i32],
    n: usize,
    qp: i32,
    bit_depth: u8,
    factors: Option<&[u8]>,
    log2_range: u32,
) {
    let (coeff_min, coeff_max) = if MAIN {
        (COEFF_MIN, COEFF_MAX)
    } else {
        (-(1i64 << log2_range), (1i64 << log2_range) - 1)
    };
    let log2n = n.trailing_zeros() as i32;
    let bd_shift = bit_depth as i32 + log2n + 10 - log2_range as i32;
    if !(1..=40).contains(&bd_shift) {
        return;
    }
    let count = n * n;
    if n == 0 || levels.len() < count {
        return;
    }
    let (rows, cols) = extent(levels, n);
    let qp = qp.max(0);
    let ls = LEVEL_SCALE[(qp % 6) as usize];
    let shift = (qp / 6) as u32;
    let round = 1i64 << (bd_shift - 1);
    let rows = levels.chunks_exact_mut(n).take(rows);
    match factors {
        None => {
            let mul = ls << 4; // m[x][y] is 16 for every coefficient
            for row in rows {
                for v in &mut row[..cols] {
                    let t = (((*v as i64) * mul) << shift) + round;
                    *v = (t >> bd_shift).clamp(coeff_min, coeff_max) as i32;
                }
            }
        }
        Some(f) if f.len() >= count => {
            for (row, frow) in rows.zip(f.chunks_exact(n)) {
                for (v, &m) in row[..cols].iter_mut().zip(&frow[..cols]) {
                    let t = (((*v as i64) * ls * m as i64) << shift) + round;
                    *v = (t >> bd_shift).clamp(coeff_min, coeff_max) as i32;
                }
            }
        }
        Some(_) => {}
    }
}

#[cfg(test)]
#[path = "dequant_tests.rs"]
mod tests;
