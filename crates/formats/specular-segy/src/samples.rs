//! Sample formats (the binary header's data sample format code, bytes 3225–3226).

/// How trace samples are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    /// 1: 4-byte IBM System/360 hexadecimal floating point.
    Ibm32,
    /// 2: 4-byte two's complement integer.
    I32,
    /// 3: 2-byte two's complement integer.
    I16,
    /// 5: 4-byte IEEE floating point.
    F32,
    /// 6: 8-byte IEEE floating point (rev 2).
    F64,
    /// 7: 3-byte two's complement integer (rev 2).
    I24,
    /// 8: 1-byte two's complement integer.
    I8,
    /// 9: 8-byte two's complement integer (rev 2).
    I64,
    /// 10: 4-byte unsigned integer (rev 2).
    U32,
    /// 11: 2-byte unsigned integer (rev 2).
    U16,
    /// 12: 8-byte unsigned integer (rev 2).
    U64,
    /// 15: 3-byte unsigned integer (rev 2).
    U24,
    /// 16: 1-byte unsigned integer (rev 2).
    U8,
}

impl SampleFormat {
    /// The format of a data sample format code; `None` for unknown codes and for 4 (fixed
    /// point with gain, obsolete).
    pub fn from_code(code: i64) -> Option<SampleFormat> {
        Some(match code {
            1 => SampleFormat::Ibm32,
            2 => SampleFormat::I32,
            3 => SampleFormat::I16,
            5 => SampleFormat::F32,
            6 => SampleFormat::F64,
            7 => SampleFormat::I24,
            8 => SampleFormat::I8,
            9 => SampleFormat::I64,
            10 => SampleFormat::U32,
            11 => SampleFormat::U16,
            12 => SampleFormat::U64,
            15 => SampleFormat::U24,
            16 => SampleFormat::U8,
            _ => return None,
        })
    }

    pub fn code(self) -> i16 {
        match self {
            SampleFormat::Ibm32 => 1,
            SampleFormat::I32 => 2,
            SampleFormat::I16 => 3,
            SampleFormat::F32 => 5,
            SampleFormat::F64 => 6,
            SampleFormat::I24 => 7,
            SampleFormat::I8 => 8,
            SampleFormat::I64 => 9,
            SampleFormat::U32 => 10,
            SampleFormat::U16 => 11,
            SampleFormat::U64 => 12,
            SampleFormat::U24 => 15,
            SampleFormat::U8 => 16,
        }
    }

    /// Bytes per sample.
    pub fn size(self) -> usize {
        match self {
            SampleFormat::I8 | SampleFormat::U8 => 1,
            SampleFormat::I16 | SampleFormat::U16 => 2,
            SampleFormat::I24 | SampleFormat::U24 => 3,
            SampleFormat::Ibm32 | SampleFormat::I32 | SampleFormat::F32 | SampleFormat::U32 => 4,
            SampleFormat::F64 | SampleFormat::I64 | SampleFormat::U64 => 8,
        }
    }

    /// A short English name for the info panel ("IBM float", "int16").
    pub fn name(self) -> &'static str {
        match self {
            SampleFormat::Ibm32 => "IBM float",
            SampleFormat::I32 => "int32",
            SampleFormat::I16 => "int16",
            SampleFormat::F32 => "IEEE float",
            SampleFormat::F64 => "IEEE double",
            SampleFormat::I24 => "int24",
            SampleFormat::I8 => "int8",
            SampleFormat::I64 => "int64",
            SampleFormat::U32 => "uint32",
            SampleFormat::U16 => "uint16",
            SampleFormat::U64 => "uint64",
            SampleFormat::U24 => "uint24",
            SampleFormat::U8 => "uint8",
        }
    }

    /// Bits of the stored sample.
    pub fn bits(self) -> u8 {
        (self.size() * 8) as u8
    }
}

/// An IBM System/360 single-precision float as the nearest `f32`: sign, base-16 exponent
/// in excess 64, and a 24-bit fraction below the radix point. The exact value always fits
/// an `f64`, so the only rounding is the final one.
pub fn ibm_to_f32(u: u32) -> f32 {
    let frac = u & 0x00ff_ffff;
    if frac == 0 {
        return if u >> 31 == 1 { -0.0 } else { 0.0 };
    }
    let exp = ((u >> 24) & 0x7f) as i64;
    // value = frac × 2^-24 × 16^(exp - 64); the power of two is a normal f64 for every
    // exponent (2^-280 ..= 2^228).
    let e2 = 4 * (exp - 64) - 24;
    let scale = f64::from_bits(((1023 + e2) as u64) << 52);
    let v = f64::from(frac) * scale;
    (if u >> 31 == 1 { -v } else { v }) as f32
}

/// Decodes `out.len()` samples from `src`.
pub fn decode(format: SampleFormat, big: bool, src: &[u8], out: &mut [f32]) {
    macro_rules! each {
        ($n:expr, $conv:expr) => {
            for (o, c) in out.iter_mut().zip(src.chunks_exact($n)) {
                let a: [u8; $n] = c.try_into().unwrap_or([0; $n]);
                *o = $conv(a);
            }
        };
    }
    match format {
        SampleFormat::Ibm32 => each!(4, |a| ibm_to_f32(if big {
            u32::from_be_bytes(a)
        } else {
            u32::from_le_bytes(a)
        })),
        SampleFormat::F32 => each!(4, |a| if big {
            f32::from_be_bytes(a)
        } else {
            f32::from_le_bytes(a)
        }),
        SampleFormat::F64 => each!(8, |a| (if big {
            f64::from_be_bytes(a)
        } else {
            f64::from_le_bytes(a)
        }) as f32),
        SampleFormat::I32 => each!(4, |a| (if big {
            i32::from_be_bytes(a)
        } else {
            i32::from_le_bytes(a)
        }) as f32),
        SampleFormat::U32 => each!(4, |a| (if big {
            u32::from_be_bytes(a)
        } else {
            u32::from_le_bytes(a)
        }) as f32),
        SampleFormat::I16 => each!(2, |a| f32::from(if big {
            i16::from_be_bytes(a)
        } else {
            i16::from_le_bytes(a)
        })),
        SampleFormat::U16 => each!(2, |a| f32::from(if big {
            u16::from_be_bytes(a)
        } else {
            u16::from_le_bytes(a)
        })),
        SampleFormat::I64 => each!(8, |a| (if big {
            i64::from_be_bytes(a)
        } else {
            i64::from_le_bytes(a)
        }) as f32),
        SampleFormat::U64 => each!(8, |a| (if big {
            u64::from_be_bytes(a)
        } else {
            u64::from_le_bytes(a)
        }) as f32),
        SampleFormat::I8 => each!(1, |a: [u8; 1]| f32::from(a[0] as i8)),
        SampleFormat::U8 => each!(1, |a: [u8; 1]| f32::from(a[0])),
        SampleFormat::I24 | SampleFormat::U24 => each!(3, |a: [u8; 3]| {
            let u = if big {
                u32::from(a[0]) << 16 | u32::from(a[1]) << 8 | u32::from(a[2])
            } else {
                u32::from(a[2]) << 16 | u32::from(a[1]) << 8 | u32::from(a[0])
            };
            if format == SampleFormat::I24 {
                ((u << 8) as i32 >> 8) as f32
            } else {
                u as f32
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ibm_values() {
        // Examples from the SEG-Y rev 1 standard, appendix (and well-known constants).
        assert_eq!(ibm_to_f32(0x4110_0000), 1.0);
        assert_eq!(ibm_to_f32(0xC110_0000), -1.0);
        assert_eq!(ibm_to_f32(0x4276_A000), 118.625);
        assert_eq!(ibm_to_f32(0xC276_A000), -118.625);
        assert_eq!(ibm_to_f32(0x4080_0000), 0.5);
        assert_eq!(ibm_to_f32(0), 0.0);
        assert!(ibm_to_f32(0x8000_0000).is_sign_negative());
        // Largest and smallest magnitudes overflow and underflow f32.
        assert_eq!(ibm_to_f32(0x7fff_ffff), f32::INFINITY);
        assert_eq!(ibm_to_f32(0x0010_0000), 0.0);
    }

    #[test]
    fn integers() {
        let mut out = [0.0; 2];
        decode(
            SampleFormat::I24,
            true,
            &[0xff, 0xff, 0xfe, 0x00, 0x01, 0x00],
            &mut out,
        );
        assert_eq!(out, [-2.0, 256.0]);
        decode(
            SampleFormat::U24,
            false,
            &[0xfe, 0xff, 0xff, 0x00, 0x01, 0x00],
            &mut out,
        );
        assert_eq!(out, [16_777_214.0, 256.0]);
        decode(
            SampleFormat::I16,
            false,
            &[0xff, 0xff, 0x02, 0x00],
            &mut out,
        );
        assert_eq!(out, [-1.0, 2.0]);
        decode(SampleFormat::I8, true, &[0x80, 0x7f], &mut out);
        assert_eq!(out, [-128.0, 127.0]);
    }
}
