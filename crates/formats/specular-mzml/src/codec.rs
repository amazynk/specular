//! Binary arrays: base64, zlib, MS-Numpress (linear prediction, positive integer and short
//! logged float, as the published MS-Numpress specification describes them) and the plain
//! number types.

use specular_spectra::Error;

/// Bytes one array may expand to at most.
pub const MAX_ARRAY_BYTES: usize = 1 << 30;

/// Decodes base64 (standard alphabet), skipping whitespace; `None` for anything else.
pub fn base64(text: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3 + 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut pad = 0usize;
    for &c in text {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                pad += 1;
                continue;
            }
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        if pad > 0 {
            // Data after padding.
            return None;
        }
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    if pad > 2 {
        return None;
    }
    Some(out)
}

/// How an array's numbers are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Number {
    F16,
    F32,
    F64,
    I32,
    I64,
}

/// How an array is compressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    None,
    Zlib,
    Linear,
    Pic,
    Slof,
    LinearZlib,
    PicZlib,
    SlofZlib,
}

impl Compression {
    fn zlib_first(self) -> bool {
        matches!(
            self,
            Compression::Zlib
                | Compression::LinearZlib
                | Compression::PicZlib
                | Compression::SlofZlib
        )
    }
}

fn inflate(b: &[u8]) -> Result<Vec<u8>, Error> {
    specular_png::inflate_zlib(b, MAX_ARRAY_BYTES).map_err(|e| match e {
        specular_png::InflateError::NeedsMemory(_) => Error::TooLarge,
        e => Error::Malformed(format!("a binary array does not inflate: {e}")),
    })
}

/// Decodes one array's base64 text into values.
pub fn decode(
    text: &[u8],
    number: Number,
    comp: Compression,
    big_endian: bool,
) -> Result<Vec<f64>, Error> {
    let raw =
        base64(text).ok_or_else(|| Error::Malformed("a binary array is not base64".into()))?;
    let bytes = if comp.zlib_first() && !raw.is_empty() {
        inflate(&raw)?
    } else {
        raw
    };
    match comp {
        Compression::Linear | Compression::LinearZlib => numpress_linear(&bytes),
        Compression::Pic | Compression::PicZlib => numpress_pic(&bytes),
        Compression::Slof | Compression::SlofZlib => numpress_slof(&bytes),
        Compression::None | Compression::Zlib => Ok(numbers(&bytes, number, big_endian)),
    }
}

fn half(h: u16) -> f64 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = i32::from((h >> 10) & 0x1f);
    let m = f64::from(h & 0x3ff);
    s * match e {
        0 => m * 2f64.powi(-24),
        31 if m == 0.0 => f64::INFINITY,
        31 => f64::NAN,
        _ => (1.0 + m / 1024.0) * 2f64.powi(e - 15),
    }
}

/// Plain numbers, little- or big-endian; a partial trailing number is dropped.
pub fn numbers(b: &[u8], number: Number, be: bool) -> Vec<f64> {
    macro_rules! conv {
        ($n:expr, $f:expr) => {
            b.chunks_exact($n)
                .map(|c| {
                    let a: [u8; $n] = c.try_into().expect("chunk size");
                    $f(a)
                })
                .collect()
        };
    }
    match (number, be) {
        (Number::F16, false) => conv!(2, |a| half(u16::from_le_bytes(a))),
        (Number::F16, true) => conv!(2, |a| half(u16::from_be_bytes(a))),
        (Number::F32, false) => conv!(4, |a| f64::from(f32::from_le_bytes(a))),
        (Number::F32, true) => conv!(4, |a| f64::from(f32::from_be_bytes(a))),
        (Number::F64, false) => conv!(8, f64::from_le_bytes),
        (Number::F64, true) => conv!(8, f64::from_be_bytes),
        (Number::I32, false) => conv!(4, |a| f64::from(i32::from_le_bytes(a))),
        (Number::I32, true) => conv!(4, |a| f64::from(i32::from_be_bytes(a))),
        (Number::I64, false) => conv!(8, |a| i64::from_le_bytes(a) as f64),
        (Number::I64, true) => conv!(8, |a| i64::from_be_bytes(a) as f64),
    }
}

fn corrupt() -> Error {
    Error::Malformed("an MS-Numpress array is corrupt".into())
}

/// The fixed point all Numpress arrays but `pic` start with: a big-endian double.
fn fixed_point(b: &[u8]) -> Result<f64, Error> {
    let a: [u8; 8] = b
        .get(..8)
        .ok_or_else(corrupt)?
        .try_into()
        .map_err(|_| corrupt())?;
    Ok(f64::from_be_bytes(a))
}

/// Reads half-bytes, high half first.
struct Nibbles<'a> {
    b: &'a [u8],
    pos: usize,
    high: bool,
}

impl Nibbles<'_> {
    fn at_end(&self) -> bool {
        self.pos >= self.b.len()
    }

    /// The input ends here: whole bytes used, or a last low half of 0 (padding).
    fn padding_only(&self) -> bool {
        self.pos + 1 == self.b.len() && !self.high && self.b[self.pos] & 0xf == 0
    }

    fn next(&mut self) -> Option<u32> {
        let byte = *self.b.get(self.pos)?;
        Some(if self.high {
            self.high = false;
            u32::from(byte >> 4)
        } else {
            self.high = true;
            self.pos += 1;
            u32::from(byte & 0xf)
        })
    }

    /// One integer: a head half-byte giving how many leading half-bytes are 0 (head ≤ 8)
    /// or 0xf (head − 8), then the rest, least significant first.
    fn int(&mut self) -> Result<i32, Error> {
        let head = self.next().ok_or_else(corrupt)?;
        let (n, mut v) = if head <= 8 {
            (head, 0u32)
        } else {
            let n = head - 8;
            let ones = if n >= 8 {
                u32::MAX
            } else {
                !(u32::MAX >> (4 * n))
            };
            (n, ones)
        };
        for i in n..8 {
            let h = self.next().ok_or_else(corrupt)?;
            v |= h << ((i - n) * 4);
        }
        Ok(v as i32)
    }
}

fn nibbles(b: &[u8]) -> Nibbles<'_> {
    Nibbles {
        b,
        pos: 0,
        high: true,
    }
}

fn le_u32(b: &[u8], at: usize) -> Result<i64, Error> {
    let a: [u8; 4] = b
        .get(at..at + 4)
        .ok_or_else(corrupt)?
        .try_into()
        .map_err(|_| corrupt())?;
    Ok(i64::from(u32::from_le_bytes(a)))
}

/// MS-Numpress linear prediction: a fixed point, the first two values as 4-byte integers,
/// then each value's difference from the straight line through the two before it.
pub fn numpress_linear(b: &[u8]) -> Result<Vec<f64>, Error> {
    if b.len() == 8 || b.is_empty() {
        return Ok(Vec::new());
    }
    if b.len() < 12 {
        return Err(corrupt());
    }
    let fp = fixed_point(b)?;
    let mut out = Vec::with_capacity(b.len() * 2);
    let mut a = le_u32(b, 8)?;
    out.push(a as f64 / fp);
    if b.len() == 12 {
        return Ok(out);
    }
    if b.len() < 16 {
        return Err(corrupt());
    }
    let mut c = le_u32(b, 12)?;
    out.push(c as f64 / fp);
    let mut nb = nibbles(&b[16..]);
    while !nb.at_end() {
        if nb.padding_only() {
            break;
        }
        let r = i64::from(nb.int()?);
        let y = c + (c - a) + r;
        out.push(y as f64 / fp);
        a = c;
        c = y;
        if out.len() * 8 > MAX_ARRAY_BYTES {
            return Err(Error::TooLarge);
        }
    }
    Ok(out)
}

/// MS-Numpress positive integer: every value rounded and stored as a variable-length
/// integer.
pub fn numpress_pic(b: &[u8]) -> Result<Vec<f64>, Error> {
    let mut out = Vec::with_capacity(b.len());
    let mut nb = nibbles(b);
    while !nb.at_end() {
        if nb.padding_only() {
            break;
        }
        out.push(f64::from(nb.int()?));
        if out.len() * 8 > MAX_ARRAY_BYTES {
            return Err(Error::TooLarge);
        }
    }
    Ok(out)
}

/// MS-Numpress short logged float: a fixed point, then `ln(v + 1) × fixed point` as 2-byte
/// little-endian integers.
pub fn numpress_slof(b: &[u8]) -> Result<Vec<f64>, Error> {
    if b.is_empty() {
        return Ok(Vec::new());
    }
    let fp = fixed_point(b)?;
    Ok(b[8..]
        .chunks_exact(2)
        .map(|c| (f64::from(u16::from_le_bytes([c[0], c[1]])) / fp).exp() - 1.0)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_and_rejects_junk() {
        assert_eq!(base64(b"AAAAAAAA8D8=").unwrap(), 1.0f64.to_le_bytes());
        assert_eq!(base64(b"TWFu").unwrap(), b"Man");
        assert_eq!(base64(b"TWE=").unwrap(), b"Ma");
        assert_eq!(base64(b"T W\nE=").unwrap(), b"Ma");
        assert_eq!(base64(b"").unwrap(), b"");
        assert!(base64(b"TW*=").is_none());
        assert!(base64(b"TW==A").is_none());
    }

    #[test]
    fn numbers_and_halves() {
        let b: Vec<u8> = [1.5f32, -2.0]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        assert_eq!(numbers(&b, Number::F32, true), vec![1.5, -2.0]);
        assert_eq!(numbers(&[0x00, 0x3c], Number::F16, false), vec![1.0]);
        assert_eq!(numbers(&[0x00, 0xc0], Number::F16, false), vec![-2.0]);
        assert_eq!(
            numbers(&(-5i64).to_le_bytes(), Number::I64, false),
            vec![-5.0]
        );
    }

    #[test]
    fn numpress_ints_with_leading_ones() {
        // Head 0xf (7 leading 0xf half-bytes, one more: 0xe) is -2; head 0x8 is 0.
        let mut nb = nibbles(&[0xfe, 0x80]);
        assert_eq!(nb.int().unwrap(), -2);
        assert_eq!(nb.int().unwrap(), 0);
        // Head 7 then one half-byte 5: 5.
        let mut nb = nibbles(&[0x75]);
        assert_eq!(nb.int().unwrap(), 5);
    }

    #[test]
    fn corrupt_numpress_is_an_error() {
        assert!(numpress_linear(&[0; 10]).is_err());
        assert!(numpress_pic(&[0x05]).is_err());
        assert!(numpress_slof(&[1, 2]).is_err());
    }
}
