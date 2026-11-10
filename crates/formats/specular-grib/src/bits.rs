//! Big-endian octet fields and an MSB-first bit reader.

use crate::{Error, Result};

/// Unsigned big-endian integer of `n` octets (1..=8) at `at`.
pub fn uint(s: &[u8], at: usize, n: usize) -> Result<u64> {
    let b = s.get(at..at + n).ok_or(Error::Truncated)?;
    Ok(b.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
}

pub fn u8_(s: &[u8], at: usize) -> Result<u8> {
    s.get(at).copied().ok_or(Error::Truncated)
}

pub fn u16_(s: &[u8], at: usize) -> Result<u16> {
    Ok(uint(s, at, 2)? as u16)
}

pub fn u32_(s: &[u8], at: usize) -> Result<u32> {
    Ok(uint(s, at, 4)? as u32)
}

/// Sign-and-magnitude integer of `n` octets (GRIB's signed fields).
pub fn sint(s: &[u8], at: usize, n: usize) -> Result<i64> {
    let v = uint(s, at, n)?;
    let sign = 1u64 << (8 * n - 1);
    Ok(if v & sign != 0 {
        -((v & !sign) as i64)
    } else {
        v as i64
    })
}

/// Whether the `n` octets at `at` are all ones (GRIB's "missing").
pub fn missing(s: &[u8], at: usize, n: usize) -> bool {
    s.get(at..at + n)
        .is_some_and(|b| b.iter().all(|&x| x == 0xFF))
}

pub fn f32_(s: &[u8], at: usize) -> Result<f32> {
    Ok(f32::from_bits(u32_(s, at)?))
}

/// IBM System/360 single precision (GRIB1's reference value).
pub fn ibm(s: &[u8], at: usize) -> Result<f64> {
    let v = u32_(s, at)?;
    let sign = if v >> 31 == 1 { -1.0 } else { 1.0 };
    let exp = ((v >> 24) & 0x7F) as i32 - 64;
    let mant = f64::from(v & 0x00FF_FFFF) / f64::from(1u32 << 24);
    Ok(sign * mant * 16f64.powi(exp))
}

/// MSB-first reader of fixed-width values.
pub struct BitReader<'a> {
    data: &'a [u8],
    /// Bit position.
    pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BitReader { data, pos: 0 }
    }

    /// `n` bits (0..=32), or zero bits past the end (GRIB writers may trim trailing zeros).
    #[inline]
    pub fn get(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let byte = self.pos >> 3;
        let shift = (self.pos & 7) as u32;
        let mut w = [0u8; 8];
        let avail = self.data.len().saturating_sub(byte).min(8);
        if avail > 0 {
            w[..avail].copy_from_slice(&self.data[byte..byte + avail]);
        }
        let v = u64::from_be_bytes(w);
        self.pos += n as usize;
        ((v << shift) >> (64 - n)) as u32
    }

    pub fn align(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }
}

/// `count` packed unsigned values of `bits` (0..=32) bits from the start of `data`.
pub fn unpack(data: &[u8], bits: u32, count: usize) -> Result<Vec<u32>> {
    if bits > 32 {
        return Err(Error::Unsupported(format!("{bits} bits per value")));
    }
    if (count * bits as usize).div_ceil(8) > data.len() {
        return Err(Error::Truncated);
    }
    let mut out = Vec::with_capacity(count);
    match bits {
        0 => out.resize(count, 0),
        8 => out.extend(data[..count].iter().map(|&b| u32::from(b))),
        16 => out.extend(
            data[..count * 2]
                .chunks_exact(2)
                .map(|b| u32::from(u16::from_be_bytes([b[0], b[1]]))),
        ),
        _ => {
            let mut r = BitReader::new(data);
            out.extend((0..count).map(|_| r.get(bits)));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields() {
        let d = [0x80, 0x05, 0xFF, 0xFF, 0x41, 0x10, 0x00, 0x00];
        assert_eq!(sint(&d, 0, 2).unwrap(), -5);
        assert_eq!(sint(&d, 1, 1).unwrap(), 5);
        assert!(missing(&d, 2, 2));
        // 0x41100000: 16^1 × 0x100000/2^24 = 1.0.
        assert_eq!(ibm(&d, 4).unwrap(), 1.0);
        let mut r = BitReader::new(&[0b1011_0011, 0b0101_0000]);
        assert_eq!(r.get(3), 0b101);
        assert_eq!(r.get(7), 0b100_1101);
        assert_eq!(r.get(6), 0b01_0000);
        assert_eq!(r.get(4), 0);
        assert_eq!(unpack(&[0xAB, 0xCD, 0xEF], 12, 2).unwrap(), [0xABC, 0xDEF]);
        assert!(unpack(&[0xAB], 12, 2).is_err());
    }
}
