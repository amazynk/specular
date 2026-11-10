//! Stored sample types: integers of either byte order, IEEE 754 and VAX (F, D, G) reals, and
//! complex pairs of either (shown as their magnitude).

/// How a real is coded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Real {
    /// IEEE 754, big-endian.
    IeeeBig,
    /// IEEE 754, little-endian.
    IeeeLittle,
    /// VAX F (4 bytes) or D (8 bytes) floating point.
    Vax,
    /// VAX G floating point (8 bytes).
    VaxG,
}

/// A stored sample type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sample {
    /// An integer of `bytes` bytes (1, 2, 4 or 8).
    Int {
        bytes: u8,
        signed: bool,
        big_endian: bool,
    },
    /// A real of 4 or 8 bytes.
    Float { bytes: u8, real: Real },
    /// A complex pair of reals of `bytes` bytes each (4 or 8).
    Complex { bytes: u8, real: Real },
}

impl Sample {
    pub const U8: Sample = Sample::Int {
        bytes: 1,
        signed: false,
        big_endian: true,
    };

    /// Bytes per sample.
    pub fn bytes(self) -> usize {
        match self {
            Sample::Int { bytes, .. } | Sample::Float { bytes, .. } => bytes as usize,
            Sample::Complex { bytes, .. } => 2 * bytes as usize,
        }
    }

    pub fn bits(self) -> u8 {
        (self.bytes() * 8).min(255) as u8
    }

    /// Whether this is a real or complex type.
    pub fn is_float(self) -> bool {
        !matches!(self, Sample::Int { .. })
    }

    /// "16-bit signed, big-endian", "32-bit float, VAX", "complex 32-bit float".
    pub fn name(self) -> String {
        let real = |r: Real| match r {
            Real::IeeeBig => ", big-endian",
            Real::IeeeLittle => ", little-endian",
            Real::Vax => ", VAX",
            Real::VaxG => ", VAX G",
        };
        match self {
            Sample::Int {
                bytes,
                signed,
                big_endian,
            } => {
                let order = match (bytes, big_endian) {
                    (1, _) => "",
                    (_, true) => ", big-endian",
                    (_, false) => ", little-endian",
                };
                let sign = if signed { "signed" } else { "unsigned" };
                format!("{}-bit {sign}{order}", bytes as u32 * 8)
            }
            Sample::Float { bytes, real: r } => {
                format!("{}-bit float{}", bytes as u32 * 8, real(r))
            }
            Sample::Complex { bytes, real: r } => {
                format!("complex {}-bit float{}", bytes as u32 * 8, real(r))
            }
        }
    }

    /// A constant written as an integer bit pattern (`16#FF7FFFFB#`): for real samples the
    /// real with those bits (VAX reals in their word order as read), else the integer.
    pub fn from_bits(self, bits: u64) -> f64 {
        match self {
            Sample::Float { bytes: 4, real } | Sample::Complex { bytes: 4, real } => match real {
                Real::IeeeBig | Real::IeeeLittle => f64::from(f32::from_bits(bits as u32)),
                _ => {
                    let x = bits as u32;
                    let [a, b] = ((x >> 16) as u16).to_le_bytes();
                    let [c, d] = (x as u16).to_le_bytes();
                    vax_f(&[a, b, c, d])
                }
            },
            Sample::Float { real, .. } | Sample::Complex { real, .. } => match real {
                Real::IeeeBig | Real::IeeeLittle => f64::from_bits(bits),
                Real::Vax => vax_d(&words(bits)),
                Real::VaxG => vax_g(&words(bits)),
            },
            Sample::Int { .. } => bits as f64,
        }
    }

    /// One sample as `f64` (complex: the magnitude). `b` holds at least [`Sample::bytes`].
    #[inline]
    pub fn read(self, b: &[u8]) -> f64 {
        match self {
            Sample::Int {
                bytes,
                signed,
                big_endian,
            } => int(b, bytes, signed, big_endian),
            Sample::Float { bytes, real } => float(b, bytes, real),
            Sample::Complex { bytes, real } => {
                let n = bytes as usize;
                let re = float(b, bytes, real);
                let im = float(&b[n..], bytes, real);
                re.hypot(im)
            }
        }
    }
}

#[inline]
fn int(b: &[u8], bytes: u8, signed: bool, big_endian: bool) -> f64 {
    macro_rules! num {
        ($t:ty, $n:expr) => {{
            let a: [u8; $n] = b[..$n].try_into().unwrap_or([0; $n]);
            if big_endian {
                <$t>::from_be_bytes(a)
            } else {
                <$t>::from_le_bytes(a)
            }
        }};
    }
    match (bytes, signed) {
        (1, false) => f64::from(b[0]),
        (1, true) => f64::from(b[0] as i8),
        (2, false) => f64::from(num!(u16, 2)),
        (2, true) => f64::from(num!(i16, 2)),
        (4, false) => f64::from(num!(u32, 4)),
        (4, true) => f64::from(num!(i32, 4)),
        (8, false) => num!(u64, 8) as f64,
        (8, true) => num!(i64, 8) as f64,
        _ => f64::NAN,
    }
}

#[inline]
fn float(b: &[u8], bytes: u8, real: Real) -> f64 {
    match (bytes, real) {
        (4, Real::IeeeBig) => f64::from(f32::from_be_bytes([b[0], b[1], b[2], b[3]])),
        (4, Real::IeeeLittle) => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        (8, Real::IeeeBig) => f64::from_be_bytes(b[..8].try_into().unwrap_or([0; 8])),
        (8, Real::IeeeLittle) => f64::from_le_bytes(b[..8].try_into().unwrap_or([0; 8])),
        (4, _) => vax_f(b),
        (8, Real::Vax) => vax_d(b),
        (8, Real::VaxG) => vax_g(b),
        _ => f64::NAN,
    }
}

/// The 16-bit words of a VAX real, most significant first (each word is little-endian, the
/// words in order of significance).
#[inline]
fn vax_bits(b: &[u8], words: usize) -> u64 {
    (0..words).fold(0u64, |acc, i| {
        (acc << 16) | u64::from(u16::from_le_bytes([b[2 * i], b[2 * i + 1]]))
    })
}

/// A VAX real from its sign, biased exponent and fraction bits: `0.1f × 2^(e − bias)`, i.e.
/// `1.f × 2^(e − bias − 1)`. Exponent 0 is zero, or with the sign set the reserved operand
/// (NaN here).
#[inline]
fn vax(sign: bool, e: u64, frac: u64, frac_bits: u32, bias: i32) -> f64 {
    if e == 0 {
        return if sign { f64::NAN } else { 0.0 };
    }
    let m = 1.0 + frac as f64 / (1u64 << frac_bits) as f64;
    let v = m * 2f64.powi(e as i32 - bias - 1);
    if sign { -v } else { v }
}

/// VAX F floating point (4 bytes: 1 sign, 8 exponent bits biased by 128, 23 fraction bits).
pub fn vax_f(b: &[u8]) -> f64 {
    let x = vax_bits(b, 2);
    vax(x >> 31 & 1 == 1, x >> 23 & 0xff, x & 0x7f_ffff, 23, 128)
}

/// VAX D floating point (8 bytes: 1 sign, 8 exponent bits biased by 128, 55 fraction bits).
pub fn vax_d(b: &[u8]) -> f64 {
    let x = vax_bits(b, 4);
    vax(
        x >> 63 & 1 == 1,
        x >> 55 & 0xff,
        x & ((1 << 55) - 1),
        55,
        128,
    )
}

/// VAX G floating point (8 bytes: 1 sign, 11 exponent bits biased by 1024, 52 fraction bits).
pub fn vax_g(b: &[u8]) -> f64 {
    let x = vax_bits(b, 4);
    vax(
        x >> 63 & 1 == 1,
        x >> 52 & 0x7ff,
        x & ((1 << 52) - 1),
        52,
        1024,
    )
}

/// The 4 bytes of `v` as VAX F (for tests and fixtures; `v` must be in VAX range).
pub fn to_vax_f(v: f32) -> [u8; 4] {
    if v == 0.0 {
        return [0; 4];
    }
    let bits = v.to_bits();
    let sign = bits >> 31;
    let e = ((bits >> 23) & 0xff) + 2;
    let x = (sign << 31) | (e << 23) | (bits & 0x7f_ffff);
    let hi = (x >> 16) as u16;
    let lo = x as u16;
    let [a, b] = hi.to_le_bytes();
    let [c, d] = lo.to_le_bytes();
    [a, b, c, d]
}

/// The 8 bytes of `v` as VAX D (for tests and fixtures; `v` must be in VAX range).
pub fn to_vax_d(v: f64) -> [u8; 8] {
    if v == 0.0 {
        return [0; 8];
    }
    let bits = v.to_bits();
    let sign = bits >> 63;
    let e = ((bits >> 52) & 0x7ff) as i64 - 1023 + 129;
    let frac = (bits & ((1 << 52) - 1)) << 3;
    let x = (sign << 63) | ((e as u64 & 0xff) << 55) | frac;
    words(x)
}

/// The 8 bytes of `v` as VAX G (for tests and fixtures).
pub fn to_vax_g(v: f64) -> [u8; 8] {
    if v == 0.0 {
        return [0; 8];
    }
    let bits = v.to_bits();
    let sign = bits >> 63;
    let e = ((bits >> 52) & 0x7ff) + 2;
    let x = (sign << 63) | ((e & 0x7ff) << 52) | (bits & ((1 << 52) - 1));
    words(x)
}

fn words(x: u64) -> [u8; 8] {
    let mut out = [0u8; 8];
    for i in 0..4 {
        let w = (x >> (48 - 16 * i)) as u16;
        out[2 * i..2 * i + 2].copy_from_slice(&w.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vax_reals() {
        // 1.0 in VAX F is 0x4080 0x0000: bytes 80 40 00 00.
        assert_eq!(vax_f(&[0x80, 0x40, 0, 0]), 1.0);
        assert_eq!(to_vax_f(1.0), [0x80, 0x40, 0, 0]);
        // -2.5 and a small value round-trip.
        for v in [-2.5f32, 0.15625, 1234.5, 3.0e-30, 1.0e30] {
            assert_eq!(vax_f(&to_vax_f(v)) as f32, v, "{v}");
        }
        for v in [-2.5f64, 0.1, 1_234.567_890_123, 1.0e-30] {
            assert_eq!(vax_d(&to_vax_d(v)), v, "{v}");
            assert_eq!(vax_g(&to_vax_g(v)), v, "{v}");
        }
        assert_eq!(vax_d(&[0x80, 0x40, 0, 0, 0, 0, 0, 0]), 1.0);
        assert_eq!(vax_g(&[0x10, 0x40, 0, 0, 0, 0, 0, 0]), 1.0);
        // The reserved operand.
        assert!(vax_f(&[0x00, 0x80, 0, 0]).is_nan());
        assert_eq!(vax_f(&[0, 0, 0x12, 0x34]), 0.0);
    }

    #[test]
    fn ints_and_names() {
        let s = Sample::Int {
            bytes: 2,
            signed: true,
            big_endian: false,
        };
        assert_eq!(s.read(&[0xfe, 0xff]), -2.0);
        assert_eq!(s.name(), "16-bit signed, little-endian");
        assert_eq!(Sample::U8.name(), "8-bit unsigned");
        let c = Sample::Complex {
            bytes: 4,
            real: Real::IeeeBig,
        };
        let mut b = 3f32.to_be_bytes().to_vec();
        b.extend(4f32.to_be_bytes());
        assert_eq!(c.read(&b), 5.0);
        assert_eq!(c.bytes(), 8);
        let f = Sample::Float {
            bytes: 4,
            real: Real::IeeeBig,
        };
        assert_eq!(f.from_bits(0xFF7F_FFFB) as f32, f32::from_bits(0xFF7F_FFFB));
        assert_eq!(s.from_bits(7), 7.0);
    }
}
