//! CAVLC residual block coding (9.2, from the writer's side).

use crate::bits::Sink;
use crate::tables::{RUN_BEFORE, TOTAL_ZEROS, TOTAL_ZEROS_CDC, coeff_token};

/// Writes one residual block. `coeffs` are the levels in scan order (length `maxNumCoeff`:
/// 16, 15 for AC blocks, 4 for chroma DC); `nc` is the predicted count (−1 for chroma DC).
/// Returns TotalCoeff.
pub fn write_block<S: Sink>(w: &mut S, coeffs: &[i16], nc: i32) -> u8 {
    let max = coeffs.len();
    // Non-zero levels from the highest frequency down, with the zero run below each.
    let mut levels = [0i32; 16];
    let mut runs = [0u32; 16];
    let mut total = 0usize;
    let mut last = None;
    for i in (0..max).rev() {
        if coeffs[i] != 0 {
            if last.is_none() {
                last = Some(i);
            }
            levels[total] = i32::from(coeffs[i]);
            total += 1;
        } else if total > 0 {
            runs[total - 1] += 1;
        }
    }
    // runs[k] counts zeros below level k, but zeros below the lowest coefficient (before
    // index 0) are not part of any run.
    let mut t1 = 0usize;
    while t1 < total && t1 < 3 && levels[t1].abs() == 1 {
        t1 += 1;
    }
    let (len, code) = coeff_token(nc, total, t1);
    w.put(len, code);
    if total == 0 {
        return 0;
    }
    for &l in &levels[..t1] {
        w.flag(l < 0);
    }
    let mut sl: u32 = if total > 10 && t1 < 3 { 1 } else { 0 };
    for (i, &level) in levels[..total].iter().enumerate().skip(t1) {
        let mut code: u32 = if level > 0 {
            (2 * level - 2) as u32
        } else {
            (-2 * level - 1) as u32
        };
        if i == t1 && t1 < 3 {
            code -= 2;
        }
        write_level(w, code, sl);
        if sl == 0 {
            sl = 1;
        }
        if level.unsigned_abs() > (3 << (sl - 1)) && sl < 6 {
            sl += 1;
        }
    }
    let last = last.unwrap_or(0);
    let total_zeros = (last + 1 - total) as u32;
    if total < max {
        let (l, c) = if max == 4 {
            TOTAL_ZEROS_CDC[total - 1][total_zeros as usize]
        } else {
            TOTAL_ZEROS[total - 1][total_zeros as usize]
        };
        w.put(u32::from(l), u32::from(c));
    }
    let mut zeros_left = total_zeros;
    for &run in &runs[..total - 1] {
        if zeros_left == 0 {
            break;
        }
        let (l, c) = RUN_BEFORE[(zeros_left.min(7) - 1) as usize][run as usize];
        w.put(u32::from(l), u32::from(c));
        zeros_left -= run;
    }
    total as u8
}

/// level_prefix and level_suffix for a levelCode at suffixLength `sl`.
#[inline]
fn write_level<S: Sink>(w: &mut S, code: u32, sl: u32) {
    let (prefix, suffix_len, suffix) = if sl == 0 {
        if code < 14 {
            (code, 0, 0)
        } else if code < 30 {
            (14, 4, code - 14)
        } else {
            (15, 12, code - 30)
        }
    } else if code < (15 << sl) {
        (code >> sl, sl, code & ((1 << sl) - 1))
    } else {
        (15, 12, code - (15 << sl))
    };
    debug_assert!(suffix < (1 << suffix_len) || suffix_len == 0);
    // prefix zeros then a one.
    w.put(prefix + 1, 1);
    w.put(suffix_len, suffix);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bits::BitWriter;

    fn bits(w: BitWriter) -> String {
        let n = w.bit_len();
        let mut w = w;
        while !w.byte_aligned() {
            w.put(1, 0);
        }
        let b = w.into_bytes();
        (0..n)
            .map(|i| {
                if b[i / 8] >> (7 - i % 8) & 1 == 1 {
                    '1'
                } else {
                    '0'
                }
            })
            .collect()
    }

    #[test]
    fn standard_example() {
        // Scan order 0 3 0 1 −1 −1 0 1 0…: TotalCoeff 5, T1s 3, nC 0 → coeff_token
        // 0000100, signs 0 1 1, levels 1 (for +1) and 0010 (for +3), total_zeros 3 → 111,
        // runs 1 → 10, 0 → 1, 0 → 1, 1 → 01.
        let c = [0, 3, 0, 1, -1, -1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut w = BitWriter::new();
        assert_eq!(write_block(&mut w, &c, 0), 5);
        assert_eq!(bits(w), "000010001110010111101101");
    }
}
