//! `std::simd` edge-preserving filter for the interior of a row (no mirroring needed).
//!
//! Eight horizontally adjacent pixels are filtered at once; every lane performs exactly the
//! scalar operations of `epf::epf_row` in the same order, so the output is bit-identical.
//! The scalar code handles the left and right edges (`skip_inner`).

use std::simd::cmp::SimdPartialOrd;
use std::simd::num::SimdFloat;
use std::simd::{Select, f32x8};

use crate::render::filter::epf::*;

/// Interior columns handled here; must match the `simd_range` of the scalar `epf_row`.
fn simd_range<const STEP: usize>(width: usize) -> std::ops::Range<usize> {
    let padding = 3 - STEP;
    let right_edge_width = if width < padding * 2 {
        width - width.saturating_sub(padding)
    } else {
        padding
    };
    let start = 4;
    let end = (width - right_edge_width) & !3;
    if start > end {
        start..start
    } else {
        start..end
    }
}

pub(crate) fn epf_row_simd<const STEP: usize>(epf_row: EpfRow<'_, '_>) {
    let EpfRow {
        input_rows,
        output_rows,
        width,
        y,
        sigma_row,
        epf_params,
        ..
    } = epf_row;
    let kernel_offsets = epf_kernel_offsets::<STEP>();
    let dist_offsets = epf_dist_offsets::<STEP>();

    let step_multiplier = if STEP == 0 {
        epf_params.sigma.pass0_sigma_scale
    } else if STEP == 2 {
        epf_params.sigma.pass2_sigma_scale
    } else {
        1.0
    };
    let border_sad_mul = epf_params.sigma.border_sad_mul;
    let channel_scale = epf_params.channel_scale;
    let is_y_border = (y + 1) & 0b110 == 0;
    let sm = if is_y_border {
        [step_multiplier * border_sad_mul; 8]
    } else {
        let mut sm = [step_multiplier; 8];
        sm[0] *= border_sad_mul;
        sm[7] *= border_sad_mul;
        sm
    };

    let range = simd_range::<STEP>(width);
    if range.is_empty() || sigma_row.len() * 8 < range.end {
        return;
    }
    let [out0, out1, out2] = output_rows;
    let mut outs = [out0, out1, out2];

    let neg_inv_sigma_base = f32x8::splat(6.6 * (std::f32::consts::FRAC_1_SQRT_2 - 1.0));
    let one = f32x8::splat(1.0);
    let zero = f32x8::splat(0.0);
    let scales = channel_scale.map(f32x8::splat);

    let mut filter8 = |dx: usize| {
        let load = |c: usize, ry: isize, x: isize| {
            let x = x as usize;
            f32x8::from_slice(&input_rows[c][ry as usize][x..x + 8])
        };
        let sigma = f32x8::from_array(std::array::from_fn(|l| sigma_row[(dx + l) / 8]));
        let skip = sigma.simd_lt(f32x8::splat(0.3));
        if skip.all() {
            // Flat blocks (most of a high-quality image): the filter is the identity.
            for c in 0..3 {
                outs[c][dx..dx + 8].copy_from_slice(&input_rows[c][3][dx..dx + 8]);
            }
            return;
        }
        let smv = f32x8::from_array(std::array::from_fn(|l| sm[(dx + l) & 7]));
        let neg_inv_sigma = neg_inv_sigma_base / sigma * smv;
        let dxi = dx as isize;

        let center: [f32x8; 3] = std::array::from_fn(|c| load(c, 3, dxi));
        let mut sum_weights = one;
        let mut sum_channels = center;
        for &(kx, ky) in kernel_offsets {
            let kernel_dy = 3 + ky;
            let kernel_dx = dxi + kx;
            let mut dist = zero;
            for (c, scale) in scales.iter().enumerate() {
                let mut acc = zero;
                for &(ix, iy) in dist_offsets {
                    acc +=
                        (load(c, kernel_dy + iy, kernel_dx + ix) - load(c, 3 + iy, dxi + ix)).abs();
                }
                dist += *scale * acc;
            }
            let weight = (one + dist * neg_inv_sigma).simd_max(zero);
            sum_weights += weight;
            for (c, sum) in sum_channels.iter_mut().enumerate() {
                *sum += weight * load(c, kernel_dy, kernel_dx);
            }
        }

        for c in 0..3 {
            let v = skip.select(center[c], sum_channels[c] / sum_weights);
            v.copy_to_slice(&mut outs[c][dx..dx + 8]);
        }
    };

    let (start, end) = (range.start, range.end);
    if end - start >= 8 {
        let mut dx = start;
        while dx + 8 <= end {
            filter8(dx);
            dx += 8;
        }
        if dx < end {
            // Overlapping last chunk; the recomputed pixels get identical values.
            filter8(end - 8);
        }
    } else {
        // Fewer than eight interior pixels: run the scalar code on them instead.
        epf_row_scalar_interior::<STEP>(
            &input_rows,
            &mut outs,
            start..end,
            sigma_row,
            &sm,
            &channel_scale,
        );
    }
}

fn epf_row_scalar_interior<const STEP: usize>(
    input_rows: &[[&[f32]; 7]; 3],
    output_rows: &mut [&mut [f32]; 3],
    range: std::ops::Range<usize>,
    sigma_row: &[f32],
    sm: &[f32; 8],
    channel_scale: &[f32; 3],
) {
    let kernel_offsets = epf_kernel_offsets::<STEP>();
    let dist_offsets = epf_dist_offsets::<STEP>();
    for dx in range {
        let sigma_val = sigma_row[dx / 8];
        if sigma_val < 0.3 {
            for c in 0..3 {
                output_rows[c][dx] = input_rows[c][3][dx];
            }
            continue;
        }
        let neg_inv_sigma = 6.6 * (std::f32::consts::FRAC_1_SQRT_2 - 1.0) / sigma_val * sm[dx & 7];
        let mut sum_weights = 1.0f32;
        let mut sum_channels: [f32; 3] = std::array::from_fn(|c| input_rows[c][3][dx]);
        for &(kx, ky) in kernel_offsets {
            let kernel_dy = 3 + ky;
            let kernel_dx = dx as isize + kx;
            let mut dist = 0f32;
            for c in 0..3 {
                let mut acc = 0f32;
                for &(ix, iy) in dist_offsets {
                    let a = input_rows[c][(kernel_dy + iy) as usize][(kernel_dx + ix) as usize];
                    let b = input_rows[c][(3 + iy) as usize][(dx as isize + ix) as usize];
                    acc += (a - b).abs();
                }
                dist += channel_scale[c] * acc;
            }
            let weight = (1.0 + dist * neg_inv_sigma).max(0.0);
            sum_weights += weight;
            for (c, sum) in sum_channels.iter_mut().enumerate() {
                *sum += weight * input_rows[c][kernel_dy as usize][kernel_dx as usize];
            }
        }
        for (c, sum) in sum_channels.into_iter().enumerate() {
            output_rows[c][dx] = sum / sum_weights;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::frame::filter::EpfParams;
    use crate::render::filter::epf::EpfRow;

    fn check<const STEP: usize>(width: usize, y: usize) {
        let mut seed = 0x9e37_79b9u32 ^ width as u32;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 1000) as f32 / 1000.0
        };
        let rows: Vec<Vec<f32>> = (0..21)
            .map(|_| (0..width).map(|_| rnd()).collect())
            .collect();
        let sigma: Vec<f32> = (0..width.div_ceil(8))
            .map(|i| if i % 5 == 3 { 0.1 } else { 0.4 + rnd() * 3.0 })
            .collect();
        let params = EpfParams::default();
        let input_rows: [[&[f32]; 7]; 3] =
            std::array::from_fn(|c| std::array::from_fn(|r| &rows[c * 7 + r][..]));

        let mut a = vec![vec![0f32; width]; 3];
        let mut b = vec![vec![0f32; width]; 3];
        {
            let [a0, a1, a2] = &mut a[..] else {
                unreachable!()
            };
            super::super::epf::epf_row::<STEP>(EpfRow {
                input_rows,
                merged_input_rows: None,
                output_rows: [a0, a1, a2],
                width,
                y,
                sigma_row: &sigma,
                epf_params: &params,
                skip_inner: false,
            });
        }
        {
            let [b0, b1, b2] = &mut b[..] else {
                unreachable!()
            };
            super::epf_row_simd::<STEP>(EpfRow {
                input_rows,
                merged_input_rows: None,
                output_rows: [&mut *b0, &mut *b1, &mut *b2],
                width,
                y,
                sigma_row: &sigma,
                epf_params: &params,
                skip_inner: true,
            });
            super::super::epf::epf_row::<STEP>(EpfRow {
                input_rows,
                merged_input_rows: None,
                output_rows: [b0, b1, b2],
                width,
                y,
                sigma_row: &sigma,
                epf_params: &params,
                skip_inner: true,
            });
        }
        for c in 0..3 {
            for x in 0..width {
                assert_eq!(
                    a[c][x].to_bits(),
                    b[c][x].to_bits(),
                    "step {STEP} w {width} c {c} x {x}"
                );
            }
        }
    }

    #[test]
    fn matches_scalar_bit_exact() {
        for width in [1, 3, 5, 8, 11, 12, 16, 19, 31, 64, 77, 260] {
            for y in [0, 1, 6, 7] {
                check::<0>(width, y);
                check::<1>(width, y);
                check::<2>(width, y);
            }
        }
    }
}
