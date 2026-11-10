//! CAVLC residual blocks (9.2).

use crate::bits::BitReader;
use crate::tables::cavlc;

/// residual_block_cavlc(): decodes up to `max` coefficients into `coef[0..max]` (scan
/// order, zero-initialised by the caller) and returns TotalCoeff. `nc` is the predicted
/// nC, or -1 for 4:2:0 chroma DC, -2 for 4:2:2 chroma DC.
#[inline]
pub fn residual_block(r: &mut BitReader, nc: i32, max: usize, coef: &mut [i32]) -> Option<u32> {
    let t = cavlc();
    let (total, t1s) = if nc >= 8 {
        let code = r.u(6);
        if code == 3 {
            (0, 0)
        } else {
            let tc = (code >> 2) + 1;
            let t1 = code & 3;
            if t1 > tc {
                return None;
            }
            (tc, t1)
        }
    } else {
        let class = match nc {
            -1 => 3,
            -2 => 4,
            0 | 1 => 0,
            2 | 3 => 1,
            _ => 2,
        };
        let s = t.coeff_token[class].read(r)? as u32;
        (s >> 2, s & 3)
    };
    if total == 0 {
        return Some(0);
    }
    let total = total as usize;
    if total > max {
        return None;
    }
    let mut level = [0i32; 16];
    let mut suffix_len: u32 = if total > 10 && t1s < 3 { 1 } else { 0 };
    for (i, l) in level.iter_mut().enumerate().take(total) {
        if i < t1s as usize {
            *l = if r.u(1) == 1 { -1 } else { 1 };
            continue;
        }
        let w = r.peek32();
        let prefix = w.leading_zeros();
        if prefix > 25 {
            return None;
        }
        r.skip(prefix + 1);
        let mut code: i32 = (prefix.min(15) << suffix_len) as i32;
        let size = if prefix == 14 && suffix_len == 0 {
            4
        } else if prefix >= 15 {
            prefix - 3
        } else {
            suffix_len
        };
        if size > 0 {
            code += r.u(size) as i32;
        }
        if prefix >= 15 && suffix_len == 0 {
            code += 15;
        }
        if prefix >= 16 {
            code += (1 << (prefix - 3)) - 4096;
        }
        if i == t1s as usize && t1s < 3 {
            code += 2;
        }
        let v = if code & 1 == 0 {
            (code + 2) >> 1
        } else {
            (-code - 1) >> 1
        };
        *l = v;
        if suffix_len == 0 {
            suffix_len = 1;
        }
        if v.unsigned_abs() > (3 << (suffix_len - 1)) && suffix_len < 6 {
            suffix_len += 1;
        }
    }
    let mut zeros_left = if total < max {
        let vlc = match max {
            4 => &t.total_zeros_dc[total],
            8 => &t.total_zeros_dc422[total],
            _ => &t.total_zeros[total],
        };
        vlc.read(r)? as usize
    } else {
        0
    };
    if zeros_left + total > max {
        return None;
    }
    // Place from the highest frequency down.
    let mut pos = (total + zeros_left) as isize - 1;
    for (i, &l) in level.iter().enumerate().take(total) {
        coef[pos as usize] = l;
        if i + 1 == total {
            break;
        }
        let run = if zeros_left > 0 {
            let run = t.run_before[zeros_left.min(7)].read(r)? as usize;
            if run > zeros_left {
                return None;
            }
            zeros_left -= run;
            run
        } else {
            0
        };
        pos -= 1 + run as isize;
    }
    Some(total as u32)
}
