//! The 16/32-bit saturating fixed-point operators that the 3GPP fixed-point codec
//! descriptions (TS 26.073 for AMR, TS 26.173 for AMR-WB) are written in, with the same
//! rounding and saturation, so that the decoders are bit-exact. The saturation flag the
//! reference keeps globally is only read in one place (the AMR synthesis filter); that
//! filter tracks it itself ([`crate::nb::syn_filt_overflow`]).

#![allow(clippy::many_single_char_names)]

pub const MAX_16: i16 = i16::MAX;
pub const MIN_16: i16 = i16::MIN;
pub const MAX_32: i32 = i32::MAX;
pub const MIN_32: i32 = i32::MIN;

#[inline(always)]
pub fn sat16(x: i32) -> i16 {
    x.clamp(MIN_16 as i32, MAX_16 as i32) as i16
}

#[inline(always)]
pub fn add(a: i16, b: i16) -> i16 {
    sat16(a as i32 + b as i32)
}

#[inline(always)]
pub fn sub(a: i16, b: i16) -> i16 {
    sat16(a as i32 - b as i32)
}

#[inline(always)]
pub fn abs_s(a: i16) -> i16 {
    if a == MIN_16 { MAX_16 } else { a.abs() }
}

#[inline(always)]
pub fn negate(a: i16) -> i16 {
    if a == MIN_16 { MAX_16 } else { -a }
}

#[inline(always)]
pub fn shl(a: i16, n: i16) -> i16 {
    if n < 0 {
        return shr(a, -(n.max(-16)));
    }
    if a == 0 {
        return 0;
    }
    if n > 15 {
        return if a > 0 { MAX_16 } else { MIN_16 };
    }
    let r = (a as i32) << n;
    if r != r as i16 as i32 {
        if a > 0 { MAX_16 } else { MIN_16 }
    } else {
        r as i16
    }
}

#[inline(always)]
pub fn shr(a: i16, n: i16) -> i16 {
    if n < 0 {
        return shl(a, -(n.max(-16)));
    }
    if n >= 15 {
        if a < 0 { -1 } else { 0 }
    } else {
        a >> n
    }
}

#[inline(always)]
pub fn shr_r(a: i16, n: i16) -> i16 {
    if n > 15 {
        return 0;
    }
    let mut r = shr(a, n);
    if n > 0 && (a as i32 & (1 << (n - 1))) != 0 {
        r = r.wrapping_add(1);
    }
    r
}

#[inline(always)]
pub fn mult(a: i16, b: i16) -> i16 {
    sat16((a as i32 * b as i32) >> 15)
}

#[inline(always)]
pub fn mult_r(a: i16, b: i16) -> i16 {
    sat16((a as i32 * b as i32 + 0x4000) >> 15)
}

#[inline(always)]
pub fn l_mult(a: i16, b: i16) -> i32 {
    let p = a as i32 * b as i32;
    if p == 0x4000_0000 { MAX_32 } else { p * 2 }
}

#[inline(always)]
pub fn l_add(a: i32, b: i32) -> i32 {
    a.saturating_add(b)
}

#[inline(always)]
pub fn l_sub(a: i32, b: i32) -> i32 {
    a.saturating_sub(b)
}

#[inline(always)]
pub fn l_mac(acc: i32, a: i16, b: i16) -> i32 {
    l_add(acc, l_mult(a, b))
}

#[inline(always)]
pub fn l_msu(acc: i32, a: i16, b: i16) -> i32 {
    l_sub(acc, l_mult(a, b))
}

#[inline(always)]
pub fn l_abs(a: i32) -> i32 {
    if a == MIN_32 { MAX_32 } else { a.abs() }
}

#[inline(always)]
pub fn extract_h(a: i32) -> i16 {
    (a >> 16) as i16
}

#[inline(always)]
pub fn extract_l(a: i32) -> i16 {
    a as i16
}

#[inline(always)]
pub fn round(a: i32) -> i16 {
    extract_h(l_add(a, 0x8000))
}

#[inline(always)]
pub fn l_deposit_h(a: i16) -> i32 {
    (a as i32) << 16
}

#[inline(always)]
pub fn l_deposit_l(a: i16) -> i32 {
    a as i32
}

#[inline(always)]
pub fn l_shl(a: i32, n: i16) -> i32 {
    if n <= 0 {
        return l_shr(a, -(n.max(-32)));
    }
    let mut x = a;
    for _ in 0..n {
        if x > 0x3fff_ffff {
            return MAX_32;
        }
        if x < -0x4000_0000 {
            return MIN_32;
        }
        x *= 2;
    }
    x
}

#[inline(always)]
pub fn l_shr(a: i32, n: i16) -> i32 {
    if n < 0 {
        return l_shl(a, -(n.max(-32)));
    }
    if n >= 31 {
        if a < 0 { -1 } else { 0 }
    } else {
        a >> n
    }
}

#[inline(always)]
pub fn l_shr_r(a: i32, n: i16) -> i32 {
    if n > 31 {
        return 0;
    }
    let mut r = l_shr(a, n);
    if n > 0 && (a & (1i32 << (n - 1))) != 0 {
        r = r.wrapping_add(1);
    }
    r
}

#[inline(always)]
pub fn norm_s(a: i16) -> i16 {
    if a == 0 {
        return 0;
    }
    if a == -1 {
        return 15;
    }
    let mut v = if a < 0 { !a } else { a };
    let mut n = 0;
    while v < 0x4000 {
        v <<= 1;
        n += 1;
    }
    n
}

#[inline(always)]
pub fn norm_l(a: i32) -> i16 {
    if a == 0 {
        return 0;
    }
    if a == -1 {
        return 31;
    }
    let v = if a < 0 { !a } else { a };
    (v.leading_zeros() as i16) - 1
}

/// Fractional division, `num / den` in Q15, for `0 <= num <= den`, `den > 0`. Values
/// outside that range (damaged input) give 0 or full scale instead of the reference's abort.
pub fn div_s(num: i16, den: i16) -> i16 {
    if num <= 0 || den <= 0 {
        return 0;
    }
    if num >= den {
        return MAX_16;
    }
    let (mut n, d) = (num as i32, den as i32);
    let mut out: i16 = 0;
    for _ in 0..15 {
        out <<= 1;
        n <<= 1;
        if n >= d {
            n -= d;
            out += 1;
        }
    }
    out
}

// --- 32-bit "double precision" helpers (oper_32b) ---

/// Splits a 32-bit value into hi (16 bits) and lo (15 bits) parts.
#[inline(always)]
pub fn l_extract(l: i32) -> (i16, i16) {
    let hi = extract_h(l);
    let lo = extract_l(l_msu(l_shr(l, 1), hi, 16384));
    (hi, lo)
}

#[inline(always)]
pub fn l_comp(hi: i16, lo: i16) -> i32 {
    l_mac(l_deposit_h(hi), lo, 1)
}

#[inline(always)]
pub fn mpy_32(hi1: i16, lo1: i16, hi2: i16, lo2: i16) -> i32 {
    let l = l_mult(hi1, hi2);
    let l = l_mac(l, mult(hi1, lo2), 1);
    l_mac(l, mult(lo1, hi2), 1)
}

#[inline(always)]
pub fn mpy_32_16(hi: i16, lo: i16, n: i16) -> i32 {
    l_mac(l_mult(hi, n), mult(lo, n), 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturation_and_rounding() {
        assert_eq!(add(32000, 1000), MAX_16);
        assert_eq!(mult(MIN_16, MIN_16), MAX_16);
        assert_eq!(l_mult(MIN_16, MIN_16), MAX_32);
        assert_eq!(shl(0x4000, 1), MAX_16);
        assert_eq!(shl(-0x4001, 1), MIN_16);
        assert_eq!(shr(-1, 20), -1);
        assert_eq!(shr_r(3, 1), 2);
        assert_eq!(l_shl(0x4000_0000, 1), MAX_32);
        assert_eq!(l_shr_r(3, 1), 2);
        assert_eq!(norm_s(1), 14);
        assert_eq!(norm_s(-0x4000), 1);
        assert_eq!(norm_l(1), 30);
        assert_eq!(norm_l(-0x4000_0000), 1);
        assert_eq!(div_s(1, 2), 16384);
        assert_eq!(round(0x7fff_8000), MAX_16);
    }
}
