//! Sample types and their conversion to `f32` plane values, shared by the EM and SPM readers.
//!
//! Integers convert to their value (rounded to the nearest `f32` past 24 bits), floats as
//! stored (`f64` rounded to nearest), booleans to 0 / 1 and complex samples to their
//! magnitude (`hypot`, in the sample's precision).

use rayon::prelude::*;

/// Byte order of stored samples.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Order {
    #[default]
    Little,
    Big,
}

/// A stored sample type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sample {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F16,
    F32,
    F64,
    /// One byte, zero or not.
    Bool,
    /// Real and imaginary `f32`.
    ComplexF32,
    /// Real and imaginary `f64`.
    ComplexF64,
    /// Real and imaginary `i16`.
    ComplexI16,
}

impl Sample {
    /// Bytes per sample.
    pub fn bytes(self) -> usize {
        match self {
            Sample::I8 | Sample::U8 | Sample::Bool => 1,
            Sample::I16 | Sample::U16 | Sample::F16 => 2,
            Sample::I32 | Sample::U32 | Sample::F32 | Sample::ComplexI16 => 4,
            Sample::I64 | Sample::U64 | Sample::F64 | Sample::ComplexF32 => 8,
            Sample::ComplexF64 => 16,
        }
    }

    pub fn is_complex(self) -> bool {
        matches!(
            self,
            Sample::ComplexF32 | Sample::ComplexF64 | Sample::ComplexI16
        )
    }

    /// Bits of one component (8 for `I8`, 32 for `ComplexF32`).
    pub fn bits(self) -> u32 {
        match self {
            Sample::ComplexF32 => 32,
            Sample::ComplexF64 => 64,
            Sample::ComplexI16 => 16,
            Sample::Bool => 1,
            s => s.bytes() as u32 * 8,
        }
    }

    /// A short description, e.g. "16-bit unsigned", "32-bit float", "64-bit complex".
    pub fn describe(self) -> &'static str {
        match self {
            Sample::I8 => "8-bit signed",
            Sample::U8 => "8-bit unsigned",
            Sample::I16 => "16-bit signed",
            Sample::U16 => "16-bit unsigned",
            Sample::I32 => "32-bit signed",
            Sample::U32 => "32-bit unsigned",
            Sample::I64 => "64-bit signed",
            Sample::U64 => "64-bit unsigned",
            Sample::F16 => "16-bit float",
            Sample::F32 => "32-bit float",
            Sample::F64 => "64-bit float",
            Sample::Bool => "binary",
            Sample::ComplexF32 => "64-bit complex",
            Sample::ComplexF64 => "128-bit complex",
            Sample::ComplexI16 => "32-bit integer complex",
        }
    }
}

/// Samples converted per parallel task.
const CHUNK: usize = 1 << 16;

/// Converts `out.len()` samples of type `s` to `f32`: sample `i` starts at byte `i * stride`
/// of `bytes` (`stride` at least the sample size; RGBA pixels pass their pixel size).
/// Samples past the end of `bytes` become NaN. Returns whether any were missing.
pub fn convert(bytes: &[u8], s: Sample, order: Order, stride: usize, out: &mut [f32]) -> bool {
    let size = s.bytes();
    let stride = stride.max(size);
    let complete = if bytes.len() < size {
        0
    } else {
        ((bytes.len() - size) / stride + 1).min(out.len())
    };
    let (have, missing) = out.split_at_mut(complete);
    have.par_chunks_mut(CHUNK)
        .enumerate()
        .for_each(|(c, chunk)| {
            let base = c * CHUNK * stride;
            convert_run(&bytes[base..], s, order, stride, chunk);
        });
    missing.fill(f32::NAN);
    !missing.is_empty()
}

fn convert_run(bytes: &[u8], s: Sample, order: Order, stride: usize, out: &mut [f32]) {
    let le = order == Order::Little;
    macro_rules! each {
        ($n:literal, $f:expr) => {
            for (i, o) in out.iter_mut().enumerate() {
                let p = i * stride;
                let mut b = [0u8; $n];
                b.copy_from_slice(&bytes[p..p + $n]);
                *o = $f(b);
            }
        };
    }
    match s {
        Sample::I8 => each!(1, |b: [u8; 1]| f32::from(b[0] as i8)),
        Sample::U8 => each!(1, |b: [u8; 1]| f32::from(b[0])),
        Sample::Bool => each!(1, |b: [u8; 1]| f32::from(u8::from(b[0] != 0))),
        Sample::I16 => each!(2, |b| f32::from(if le {
            i16::from_le_bytes(b)
        } else {
            i16::from_be_bytes(b)
        })),
        Sample::U16 => each!(2, |b| f32::from(if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })),
        Sample::F16 => each!(2, |b| f16_to_f32(if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })),
        Sample::I32 => each!(4, |b| (if le {
            i32::from_le_bytes(b)
        } else {
            i32::from_be_bytes(b)
        }) as f32),
        Sample::U32 => each!(4, |b| (if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        }) as f32),
        Sample::F32 => each!(4, |b| if le {
            f32::from_le_bytes(b)
        } else {
            f32::from_be_bytes(b)
        }),
        Sample::I64 => each!(8, |b| (if le {
            i64::from_le_bytes(b)
        } else {
            i64::from_be_bytes(b)
        }) as f32),
        Sample::U64 => each!(8, |b| (if le {
            u64::from_le_bytes(b)
        } else {
            u64::from_be_bytes(b)
        }) as f32),
        Sample::F64 => each!(8, |b| (if le {
            f64::from_le_bytes(b)
        } else {
            f64::from_be_bytes(b)
        }) as f32),
        Sample::ComplexI16 => each!(4, |b: [u8; 4]| {
            let r = [b[0], b[1]];
            let i = [b[2], b[3]];
            let (r, i) = if le {
                (i16::from_le_bytes(r), i16::from_le_bytes(i))
            } else {
                (i16::from_be_bytes(r), i16::from_be_bytes(i))
            };
            f32::from(r).hypot(f32::from(i))
        }),
        Sample::ComplexF32 => each!(8, |b: [u8; 8]| {
            let (r, i) = (f32_at(&b[..4], le), f32_at(&b[4..], le));
            r.hypot(i)
        }),
        Sample::ComplexF64 => each!(16, |b: [u8; 16]| {
            let (r, i) = (f64_at(&b[..8], le), f64_at(&b[8..], le));
            r.hypot(i) as f32
        }),
    }
}

/// The `f32` at the start of `b`.
pub fn f32_at(b: &[u8], le: bool) -> f32 {
    let a = [b[0], b[1], b[2], b[3]];
    if le {
        f32::from_le_bytes(a)
    } else {
        f32::from_be_bytes(a)
    }
}

/// The `f64` at the start of `b`.
pub fn f64_at(b: &[u8], le: bool) -> f64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[..8]);
    if le {
        f64::from_le_bytes(a)
    } else {
        f64::from_be_bytes(a)
    }
}

/// An IEEE half-precision value as `f32` (exact).
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = u32::from(h >> 15) << 31;
    let exp = u32::from((h >> 10) & 0x1f);
    let man = u32::from(h & 0x3ff);
    let bits = match (exp, man) {
        (0, 0) => sign,
        (0, m) => {
            // Subnormal: m × 2^-24, exact in f32.
            let v = m as f32 * (1.0 / 16_777_216.0);
            return if sign != 0 { -v } else { v };
        }
        (0x1f, 0) => sign | 0x7f80_0000,
        (0x1f, m) => sign | 0x7fc0_0000 | (m << 13),
        (e, m) => sign | ((e + 127 - 15) << 23) | (m << 13),
    };
    f32::from_bits(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x7bff), 65504.0);
        assert_eq!(f16_to_f32(0x0001), 2f32.powi(-24));
        assert_eq!(f16_to_f32(0x8000).to_bits(), 0x8000_0000);
        assert!(f16_to_f32(0x7c00).is_infinite());
        assert!(f16_to_f32(0x7e00).is_nan());
    }

    #[test]
    fn conversions() {
        let mut out = [0f32; 3];
        let missing = convert(
            &[1, 0, 0xff, 0xff, 2],
            Sample::I16,
            Order::Little,
            2,
            &mut out,
        );
        assert!(missing);
        assert_eq!(out[..2], [1.0, -1.0]);
        assert!(out[2].is_nan());
        let mut out = [0f32; 2];
        convert(&[0, 1, 0, 2], Sample::U16, Order::Big, 2, &mut out);
        assert_eq!(out, [1.0, 2.0]);
        // Every other byte (an RGBA channel).
        let mut out = [0f32; 2];
        assert!(!convert(
            &[5, 9, 6, 9],
            Sample::U8,
            Order::Little,
            2,
            &mut out
        ));
        assert_eq!(out, [5.0, 6.0]);
        let mut c = Vec::new();
        c.extend(3f32.to_le_bytes());
        c.extend(4f32.to_le_bytes());
        let mut out = [0f32; 1];
        convert(&c, Sample::ComplexF32, Order::Little, 8, &mut out);
        assert_eq!(out, [5.0]);
    }
}
