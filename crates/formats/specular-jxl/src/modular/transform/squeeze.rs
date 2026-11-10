use std::num::Wrapping;

use crate::grid::{MutableSubgrid, SharedSubgrid};

use crate::modular::Sample;

use crate::modular::sample::Sealed as _;

pub(crate) use crate::modular::Sample as SqueezeSample;

#[inline(always)]
fn unsqueeze<S: SqueezeSample>(residu: S, left: S, avg: S, next_avg: S) -> (S, S) {
    let diff = residu.sq_add(S::sq_tendency(left, avg, next_avg));
    let first = avg.sq_add(diff.sq_half());
    let second = first.sq_sub(diff);
    (first, second)
}

/// Inverse horizontal squeeze. `out` has the full output width; its first `ceil(width / 2)`
/// columns hold the averages on entry. `residu` holds the `floor(width / 2)` residual columns.
pub fn inverse_h<S: SqueezeSample>(out: &mut MutableSubgrid<'_, S>, residu: SharedSubgrid<'_, S>) {
    let height = out.height();
    let width = out.width();
    let avg_width = width.div_ceil(2);
    debug_assert_eq!(residu.width(), width / 2);
    let mut scratch = vec![S::default(); avg_width];
    for y in 0..height {
        let row_out = out.get_row_mut(y);
        scratch.copy_from_slice(&row_out[..avg_width]);
        let residu_row = residu.get_row(y);
        let avg_row = &scratch[..];

        let mut avg = avg_row[0];
        let mut left = avg;
        let mut row_out_it = row_out.chunks_exact_mut(2);
        for (x, pair) in (&mut row_out_it).enumerate() {
            let next_avg = avg_row.get(x + 1).copied().unwrap_or(avg);
            let (first, second) = unsqueeze(residu_row[x], left, avg, next_avg);
            pair[0] = first;
            pair[1] = second;
            avg = next_avg;
            left = second;
        }
        if let [v] = row_out_it.into_remainder() {
            *v = avg_row[avg_width - 1];
        }
    }
}

/// Inverse vertical squeeze. `out` has the full output height; its first `ceil(height / 2)` rows
/// hold the averages on entry. `residu` holds the `floor(height / 2)` residual rows.
pub fn inverse_v<S: SqueezeSample>(out: &mut MutableSubgrid<'_, S>, residu: SharedSubgrid<'_, S>) {
    let width = out.width();
    let height = out.height();
    if width == 0 || height == 0 {
        return;
    }
    let avg_height = height.div_ceil(2);
    debug_assert_eq!(residu.height(), height / 2);
    let mut scratch = Vec::with_capacity(avg_height * width);
    for y in 0..avg_height {
        scratch.extend_from_slice(out.get_row(y));
    }
    let mut top = scratch[..width].to_vec();
    for y in 0..height / 2 {
        let avg_row = &scratch[y * width..][..width];
        let next_row = if y + 1 < avg_height {
            &scratch[(y + 1) * width..][..width]
        } else {
            avg_row
        };
        let residu_row = residu.get_row(y);
        let (first_row, second_row) = out.two_rows_mut(2 * y, 2 * y + 1);
        for x in 0..width {
            let (first, second) = unsqueeze(residu_row[x], top[x], avg_row[x], next_row[x]);
            first_row[x] = first;
            second_row[x] = second;
        }
        top.copy_from_slice(second_row);
    }
    if height % 2 == 1 {
        out.get_row_mut(height - 1)
            .copy_from_slice(&scratch[(avg_height - 1) * width..][..width]);
    }
}

#[inline(always)]
pub(crate) fn tendency_i32(a: i32, b: i32, c: i32) -> i32 {
    let a = Wrapping(a);
    let b = Wrapping(b);
    let c = Wrapping(c);

    let n1 = Wrapping(1);
    let n2 = Wrapping(2);
    let n3 = Wrapping(3);
    let n4 = Wrapping(4);
    let n6 = Wrapping(6);
    let n12 = Wrapping(12);

    if a >= b && b >= c {
        let mut x = (n4 * a - n3 * c - b + n6) / n12;
        if x - (x & n1) > n2 * (a - b) {
            x = n2 * (a - b) + n1;
        }
        if x + (x & n1) > n2 * (b - c) {
            x = n2 * (b - c);
        }
        x.0
    } else if a <= b && b <= c {
        let mut x = (n4 * a - n3 * c - b - n6) / n12;
        if x + (x & n1) < n2 * (a - b) {
            x = n2 * (a - b) - n1;
        }
        if x - (x & n1) < n2 * (b - c) {
            x = n2 * (b - c);
        }
        x.0
    } else {
        0
    }
}

#[inline(always)]
pub(crate) fn tendency_i16(a: i16, b: i16, c: i16) -> i16 {
    let a = Wrapping(a);
    let b = Wrapping(b);
    let c = Wrapping(c);

    let n1 = Wrapping(1);
    let n2 = Wrapping(2);
    let n3 = Wrapping(3);
    let n4 = Wrapping(4);
    let n6 = Wrapping(6);
    let n12 = Wrapping(12);

    if a >= b && b >= c {
        let mut x = (n4 * a - n3 * c - b + n6) / n12;
        if x - (x & n1) > n2 * (a - b) {
            x = n2 * (a - b) + n1;
        }
        if x + (x & n1) > n2 * (b - c) {
            x = n2 * (b - c);
        }
        x.0
    } else if a <= b && b <= c {
        let mut x = (n4 * a - n3 * c - b - n6) / n12;
        if x + (x & n1) < n2 * (a - b) {
            x = n2 * (a - b) - n1;
        }
        if x - (x & n1) < n2 * (b - c) {
            x = n2 * (b - c);
        }
        x.0
    } else {
        0
    }
}
