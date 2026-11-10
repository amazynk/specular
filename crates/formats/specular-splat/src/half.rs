//! IEEE half floats (the harmonics are kept as them, as the GPU reads them).

/// A half float's value.
pub fn f16_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = i32::from((h >> 10) & 0x1f);
    let m = f32::from(h & 0x3ff);
    s * match e {
        0 => m * 2f32.powi(-24),
        31 => {
            if m == 0.0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

/// The nearest half float (ties to even), saturating to infinity, NaN kept.
pub fn f32_to_f16(v: f32) -> u16 {
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let e = ((b >> 23) & 0xff) as i32;
    let m = b & 0x7f_ffff;
    if e == 0xff {
        return sign | 0x7c00 | if m != 0 { 0x200 } else { 0 };
    }
    let e16 = e - 127 + 15;
    if e16 >= 31 {
        return sign | 0x7c00;
    }
    if e16 <= 0 {
        if e16 < -10 {
            return sign;
        }
        // Subnormal: the implicit bit joins the mantissa, shifted down.
        let m = m | 0x80_0000;
        let shift = (14 - e16) as u32;
        let half = 1u32 << (shift - 1);
        let rest = m & ((1 << shift) - 1);
        let mut r = m >> shift;
        if rest > half || (rest == half && r & 1 == 1) {
            r += 1;
        }
        return sign | r as u16;
    }
    let mut r = ((e16 as u32) << 10) | (m >> 13);
    let rest = m & 0x1fff;
    if rest > 0x1000 || (rest == 0x1000 && r & 1 == 1) {
        r += 1;
    }
    sign | r as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for v in [
            0.0f32,
            1.0,
            -2.5,
            0.333_251_95,
            65504.0,
            6.1035156e-5,
            5.96e-8,
            -0.0,
        ] {
            let h = f32_to_f16(v);
            assert!((f16_to_f32(h) - v).abs() <= v.abs() * 1e-3 + 1e-7, "{v}");
        }
        assert_eq!(f32_to_f16(1e6), 0x7c00);
        assert!(f16_to_f32(f32_to_f16(f32::NAN)).is_nan());
        // Every half survives the round trip.
        for h in 0..=0xffffu16 {
            let v = f16_to_f32(h);
            if !v.is_nan() {
                assert_eq!(f32_to_f16(v), h, "{h:#x}");
            }
        }
    }
}
