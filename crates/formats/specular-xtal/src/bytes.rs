//! Sample conversion and fixed-layout header reads.

use crate::Sample;

/// `src` (whole samples) as f32 values.
pub(crate) fn samples(src: &[u8], sample: Sample, big: bool) -> Vec<f32> {
    macro_rules! conv {
        ($t:ty, $n:expr) => {
            src.chunks_exact($n)
                .map(|c| {
                    let a: [u8; $n] = c.try_into().unwrap_or([0; $n]);
                    (if big {
                        <$t>::from_be_bytes(a)
                    } else {
                        <$t>::from_le_bytes(a)
                    }) as f32
                })
                .collect()
        };
    }
    match sample {
        Sample::U8 => src.iter().map(|&b| f32::from(b)).collect(),
        Sample::I8 => src.iter().map(|&b| f32::from(b as i8)).collect(),
        Sample::U16 => conv!(u16, 2),
        Sample::I16 => conv!(i16, 2),
        Sample::U32 => conv!(u32, 4),
        Sample::I32 => conv!(i32, 4),
        Sample::U64 => conv!(u64, 8),
        Sample::I64 => conv!(i64, 8),
        Sample::F32 => conv!(f32, 4),
        Sample::F64 => conv!(f64, 8),
    }
}

/// A 32-bit integer at `at`, `big`-endian or not.
pub(crate) fn i32_at(d: &[u8], at: usize, big: bool) -> Option<i32> {
    let a: [u8; 4] = d.get(at..at + 4)?.try_into().ok()?;
    Some(if big {
        i32::from_be_bytes(a)
    } else {
        i32::from_le_bytes(a)
    })
}

pub(crate) fn u32_at(d: &[u8], at: usize, big: bool) -> Option<u32> {
    i32_at(d, at, big).map(|v| v as u32)
}

pub(crate) fn f32_at(d: &[u8], at: usize, big: bool) -> Option<f32> {
    u32_at(d, at, big).map(f32::from_bits)
}

/// A NUL- or space-padded ASCII field.
pub(crate) fn str_at(d: &[u8], at: usize, len: usize) -> Option<String> {
    let b = d.get(at..at + len)?;
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    crate::text(&String::from_utf8_lossy(&b[..end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts() {
        assert_eq!(samples(&[1, 0, 0, 2], Sample::U16, false), [1.0, 512.0]);
        assert_eq!(samples(&[1, 0, 0, 2], Sample::U16, true), [256.0, 2.0]);
        assert_eq!(samples(&[0xff, 0xff], Sample::I16, false), [-1.0]);
        assert_eq!(samples(&1.5f32.to_be_bytes(), Sample::F32, true), [1.5]);
        assert_eq!(samples(&[200], Sample::I8, false), [-56.0]);
        assert_eq!(str_at(b"MMX\0\0junk", 0, 9).as_deref(), Some("MMX"));
        assert_eq!(f32_at(&2.0f32.to_le_bytes(), 0, false), Some(2.0));
    }
}
