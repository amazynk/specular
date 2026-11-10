//! `f16` <-> `f32` conversion (nightly `f16`; F16C on the x86-64-v3 baseline).

#[inline(always)]
pub(crate) fn h2f(bits: u16) -> f32 {
    f16::from_bits(bits) as f32
}

#[inline(always)]
pub(crate) fn f2h(v: f32) -> u16 {
    (v as f16).to_bits()
}

/// Output sample type: `f32`, or `u16` holding `f16` bits.
pub(crate) trait Sample: Copy + Send + Sync + 'static {
    const ZERO: Self;
    const ONE: Self;
    fn from_half(bits: u16) -> Self;
    fn from_f32(v: f32) -> Self;
    fn to_f32(self) -> f32;
}

impl Sample for f32 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    #[inline(always)]
    fn from_half(bits: u16) -> Self {
        h2f(bits)
    }
    #[inline(always)]
    fn from_f32(v: f32) -> Self {
        v
    }
    #[inline(always)]
    fn to_f32(self) -> f32 {
        self
    }
}

impl Sample for u16 {
    const ZERO: Self = 0;
    const ONE: Self = 0x3c00;
    #[inline(always)]
    fn from_half(bits: u16) -> Self {
        bits
    }
    #[inline(always)]
    fn from_f32(v: f32) -> Self {
        f2h(v)
    }
    #[inline(always)]
    fn to_f32(self) -> f32 {
        h2f(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_halves() {
        for b in 0..=u16::MAX {
            let f = h2f(b);
            if f.is_nan() {
                assert!(h2f(f2h(f)).is_nan());
            } else {
                assert_eq!(f2h(f), b, "{b:#x}");
            }
        }
        assert_eq!(h2f(0x3c00), 1.0);
        assert_eq!(f2h(65520.0), 0x7c00); // rounds to infinity
    }
}
