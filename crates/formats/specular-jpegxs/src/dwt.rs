//! Inverse 5/3 wavelet transform (ISO/IEC 21122-1 Annex E) on an interleaved coefficient
//! plane, in place: the horizontal-only levels first, then the levels transformed both ways.

use rayon::prelude::*;

/// Inverse lifting of one sequence with whole-sample symmetric extension.
#[inline]
fn lift(x: &mut [i32]) {
    let n = x.len();
    if n < 2 {
        return;
    }
    x[0] = x[0].wrapping_sub(x[1].wrapping_add(1) >> 1);
    let mut i = 2;
    while i + 1 < n {
        x[i] = x[i].wrapping_sub(x[i - 1].wrapping_add(x[i + 1]).wrapping_add(2) >> 2);
        i += 2;
    }
    if i < n {
        x[i] = x[i].wrapping_sub(x[i - 1].wrapping_add(1) >> 1);
    }
    let mut i = 1;
    while i + 1 < n {
        x[i] = x[i].wrapping_add(x[i - 1].wrapping_add(x[i + 1]) >> 1);
        i += 2;
    }
    if i < n {
        x[i] = x[i].wrapping_add(x[i - 1]);
    }
}

/// Horizontal inverse at column level `hl` on the rows of level `vl`.
fn horizontal(p: &mut [i32], w: usize, hl: u8, vl: u8) {
    let step = 1usize << hl;
    let chunk = w << vl;
    p.par_chunks_mut(chunk)
        .for_each_init(Vec::new, |tmp, rows| {
            let row = &mut rows[..w];
            if step == 1 {
                lift(row);
            } else {
                tmp.clear();
                tmp.extend(row.iter().step_by(step));
                lift(tmp);
                for (d, s) in row.iter_mut().step_by(step).zip(tmp.iter()) {
                    *d = *s;
                }
            }
        });
}

/// Vertical inverse at level (`hl`, `vl`): rows at multiples of 2^vl, columns at multiples
/// of 2^hl. Each lifting step reads a copy of the rows it does not change, so rows can be
/// updated in parallel.
fn vertical(p: &mut [i32], w: usize, h: usize, hl: u8, vl: u8) {
    let step = 1usize << hl;
    let rs = w << vl;
    // Sequence length (rows of this level).
    let n = h.div_ceil(1 << vl);
    if n < 2 {
        return;
    }
    let cols = w.div_ceil(step);
    let gather = |p: &[i32], k: usize, out: &mut Vec<i32>| {
        let row = &p[k * rs..k * rs + w];
        out.extend(row.iter().step_by(step));
    };
    // Odd rows, then the even rows from them.
    let mut odd = Vec::with_capacity(n / 2 * cols);
    for k in (1..n).step_by(2) {
        gather(p, k, &mut odd);
    }
    let odd_row = |k: usize| &odd[(k / 2) * cols..(k / 2 + 1) * cols];
    p.par_chunks_mut(2 * rs).enumerate().for_each(|(j, c)| {
        let k = 2 * j;
        if k >= n {
            return;
        }
        let row = &mut c[..w];
        let below = (k + 1 < n).then(|| odd_row(k + 1));
        let above = (k > 0).then(|| odd_row(k - 1));
        for (i, v) in row.iter_mut().step_by(step).enumerate() {
            let d = match (above, below) {
                (Some(a), Some(b)) => a[i].wrapping_add(b[i]).wrapping_add(2) >> 2,
                (None, Some(b)) => b[i].wrapping_add(1) >> 1,
                (Some(a), None) => a[i].wrapping_add(1) >> 1,
                (None, None) => 0,
            };
            *v = v.wrapping_sub(d);
        }
    });
    drop(odd);
    let mut even = Vec::with_capacity(n.div_ceil(2) * cols);
    for k in (0..n).step_by(2) {
        gather(p, k, &mut even);
    }
    let even_row = |k: usize| &even[(k / 2) * cols..(k / 2 + 1) * cols];
    p.par_chunks_mut(2 * rs).enumerate().for_each(|(j, c)| {
        let k = 2 * j + 1;
        if k >= n || c.len() < rs + w {
            return;
        }
        let row = &mut c[rs..rs + w];
        let above = even_row(k - 1);
        let below = (k + 1 < n).then(|| even_row(k + 1));
        for (i, v) in row.iter_mut().step_by(step).enumerate() {
            let d = match below {
                Some(b) => above[i].wrapping_add(b[i]) >> 1,
                None => above[i],
            };
            *v = v.wrapping_add(d);
        }
    });
}

/// Inverse transform of one component plane of `w`×`h` coefficients.
pub(crate) fn inverse(p: &mut [i32], w: usize, h: usize, nlx: u8, nly: u8) {
    if w == 0 || h == 0 {
        return;
    }
    for d in (nly..nlx).rev() {
        horizontal(p, w, d, nly);
    }
    for d in (0..nly).rev() {
        horizontal(p, w, d, d);
        vertical(p, w, h, d, d);
    }
}

#[cfg(test)]
mod tests {
    use super::lift;

    /// Forward 5/3 lifting (the encoder's), for round trips.
    fn forward(x: &mut [i32]) {
        let n = x.len();
        let mut i = 1;
        while i + 1 < n {
            x[i] -= (x[i - 1] + x[i + 1]) >> 1;
            i += 2;
        }
        if i < n {
            x[i] -= x[i - 1];
        }
        x[0] += (x[1] + 1) >> 1;
        let mut i = 2;
        while i + 1 < n {
            x[i] += (x[i - 1] + x[i + 1] + 2) >> 2;
            i += 2;
        }
        if i < n {
            x[i] += (x[i - 1] + 1) >> 1;
        }
    }

    #[test]
    fn lifting_round_trips() {
        for n in 2..20usize {
            let orig: Vec<i32> = (0..n).map(|i| (i * 37 % 11) as i32 - 5).collect();
            let mut x = orig.clone();
            forward(&mut x);
            lift(&mut x);
            assert_eq!(x, orig, "length {n}");
        }
    }
}
