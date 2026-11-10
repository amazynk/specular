//! Matrix-based intra prediction (8.4.5.2.2 to 8.4.5.2.5).

use crate::tables::{MIP0, MIP1, MIP2};

/// Downsamples `refs` to `n` boundary samples (8.4.5.2.3).
fn reduce(refs: &[i32], n: usize, out: &mut [i32]) {
    let s = refs.len();
    if n < s {
        let d = s / n;
        let l = d.trailing_zeros();
        for (x, o) in out.iter_mut().enumerate().take(n) {
            let sum: i32 = refs[x * d..(x + 1) * d].iter().sum();
            *o = (sum + (1 << (l - 1))) >> l;
        }
    } else {
        out[..n].copy_from_slice(&refs[..n]);
    }
}

/// MIP prediction of a `w x h` block from the unfiltered top row `ref_t` (w samples) and
/// left column `ref_l` (h samples).
#[allow(clippy::too_many_arguments)]
pub fn predict(
    ref_t: &[i32],
    ref_l: &[i32],
    w: usize,
    h: usize,
    mode: usize,
    transposed: bool,
    bit_depth: u32,
    out: &mut [i32],
) {
    let size_id = if w == 4 && h == 4 {
        0
    } else if w == 4 || h == 4 || (w == 8 && h == 8) {
        1
    } else {
        2
    };
    let (bsize, psize) = match size_id {
        0 => (2, 4),
        1 => (4, 4),
        _ => (4, 8),
    };
    let in_size = 2 * bsize - usize::from(size_id == 2);
    let mut red_t = [0i32; 4];
    let mut red_l = [0i32; 4];
    reduce(ref_t, bsize, &mut red_t);
    reduce(ref_l, bsize, &mut red_l);
    let mut ptemp = [0i32; 8];
    if transposed {
        ptemp[..bsize].copy_from_slice(&red_l[..bsize]);
        ptemp[bsize..2 * bsize].copy_from_slice(&red_t[..bsize]);
    } else {
        ptemp[..bsize].copy_from_slice(&red_t[..bsize]);
        ptemp[bsize..2 * bsize].copy_from_slice(&red_l[..bsize]);
    }
    let mut p = [0i32; 8];
    if size_id == 2 {
        for x in 0..in_size {
            p[x] = ptemp[x + 1] - ptemp[0];
        }
    } else {
        p[0] = (1 << (bit_depth - 1)) - ptemp[0];
        for x in 1..in_size {
            p[x] = ptemp[x] - ptemp[0];
        }
    }
    let sum: i32 = p[..in_size].iter().sum();
    let ow = 32 - 32 * sum;
    let max = (1 << bit_depth) - 1;
    let mut pm = [0i32; 64];
    for pos in 0..psize * psize {
        let mut s = 0i32;
        for i in 0..in_size {
            let wgt = i32::from(match size_id {
                0 => MIP0[mode][pos][i],
                1 => MIP1[mode][pos][i],
                _ => MIP2[mode][pos][i],
            });
            s += wgt * p[i];
        }
        pm[pos] = (((s + ow) >> 6) + ptemp[0]).clamp(0, max);
    }
    if transposed {
        let mut t = [0i32; 64];
        for y in 0..psize {
            for x in 0..psize {
                t[x * psize + y] = pm[y * psize + x];
            }
        }
        pm = t;
    }
    // pm[ y * psize + x ] = predMip[ x ][ y ].
    if w == psize && h == psize {
        out[..w * h].copy_from_slice(&pm[..w * h]);
        return;
    }
    upsample(&pm, psize, w, h, ref_t, ref_l, out);
}

/// MIP prediction upsampling (8.4.5.2.5).
fn upsample(
    pm: &[i32],
    psize: usize,
    w: usize,
    h: usize,
    ref_t: &[i32],
    ref_l: &[i32],
    out: &mut [i32],
) {
    let up_h = w / psize;
    let up_v = h / psize;
    // Work on a (w + 1) x (h + 1) grid with row/column 0 holding the references.
    let gw = w + 1;
    let mut g = vec![0i32; gw * (h + 1)];
    let at = |x: isize, y: isize| ((y + 1) as usize) * gw + (x + 1) as usize;
    for y in 0..psize {
        for x in 0..psize {
            g[at(((x + 1) * up_h - 1) as isize, ((y + 1) * up_v - 1) as isize)] = pm[y * psize + x];
        }
    }
    for x in 0..w {
        g[at(x as isize, -1)] = ref_t[x];
    }
    for y in 0..h {
        g[at(-1, y as isize)] = ref_l[y];
    }
    if up_h > 1 {
        for n in 1..=psize {
            let yh = (n * up_v - 1) as isize;
            for m in 0..psize {
                let xh = (m * up_h) as isize - 1;
                let a = g[at(xh, yh)];
                let b = g[at(xh + up_h as isize, yh)];
                for dx in 1..up_h {
                    let s = (up_h - dx) as i32 * a + dx as i32 * b;
                    g[at(xh + dx as isize, yh)] = (s + up_h as i32 / 2) / up_h as i32;
                }
            }
        }
    }
    if up_v > 1 {
        for n in 0..psize {
            let yv = (n * up_v) as isize - 1;
            for m in 0..w {
                let a = g[at(m as isize, yv)];
                let b = g[at(m as isize, yv + up_v as isize)];
                for dy in 1..up_v {
                    let s = (up_v - dy) as i32 * a + dy as i32 * b;
                    g[at(m as isize, yv + dy as isize)] = (s + up_v as i32 / 2) / up_v as i32;
                }
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            out[y * w + x] = g[at(x as isize, y as isize)];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_references_give_a_near_flat_block() {
        let t = [512i32; 16];
        let l = [512i32; 16];
        let mut out = [0i32; 256];
        for mode in 0..6 {
            predict(&t, &l, 16, 16, mode, mode % 2 == 1, 10, &mut out);
            assert!(out.iter().all(|&v| (v - 512).abs() <= 1), "mode {mode}");
        }
        predict(&t[..4], &l[..4], 4, 4, 3, false, 10, &mut out);
        assert!(out[..16].iter().all(|&v| (v - 512).abs() <= 1));
    }
}
