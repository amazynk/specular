//! HDF5's bit-packing filters: N-bit (5) and scale-offset (6), for atomic integer and float
//! elements. Both pack each element's significant bits back to back, most significant bit
//! first, across the whole chunk.

use crate::{Error, Result};

fn bad(what: &str) -> Error {
    Error::Malformed(format!("{what} stream"))
}

/// Reads bit strings most significant bit first.
struct Bits<'a> {
    b: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn read(&mut self, n: u32) -> Result<u64> {
        let mut v = 0u64;
        let mut left = n;
        while left > 0 {
            let byte = *self.b.get(self.pos / 8).ok_or_else(|| bad("packed"))?;
            let used = (self.pos % 8) as u32;
            let avail = 8 - used;
            let take = avail.min(left);
            let bits = (u64::from(byte) >> (avail - take)) & ((1u64 << take) - 1);
            v = (v << take) | bits;
            left -= take;
            self.pos += take as usize;
        }
        Ok(v)
    }
}

fn put(out: &mut [u8], v: u64, be: bool) {
    let n = out.len();
    for (k, o) in out.iter_mut().enumerate() {
        let shift = if be { (n - 1 - k) * 8 } else { k * 8 };
        *o = (v >> shift) as u8;
    }
}

fn get(b: &[u8], be: bool) -> u64 {
    let n = b.len();
    b.iter().enumerate().fold(0u64, |a, (k, &x)| {
        let shift = if be { (n - 1 - k) * 8 } else { k * 8 };
        a | (u64::from(x) << shift)
    })
}

/// Undoes the N-bit filter on one chunk of `expected` bytes.
pub(crate) fn nbit(src: &[u8], params: &[u32], expected: usize) -> Result<Vec<u8>> {
    let p = |i: usize| {
        params
            .get(i)
            .copied()
            .ok_or_else(|| Error::Malformed("N-bit parameters".into()))
    };
    if p(1)? != 0 {
        // Nothing was packed (full-precision type).
        return src
            .get(..expected)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| bad("N-bit"));
    }
    let n = p(2)? as usize;
    match p(3)? {
        1 => {}
        c => return Err(Error::Unsupported(format!("N-bit datatype class {c}"))),
    }
    let size = p(4)? as usize;
    let be = p(5)? == 1;
    let prec = p(6)?;
    let off = p(7)?;
    if size == 0 || size > 8 || prec == 0 || u64::from(prec) + u64::from(off) > size as u64 * 8 {
        return Err(Error::Malformed("N-bit parameters".into()));
    }
    let total = n.checked_mul(size).filter(|&t| t <= expected.max(n * size));
    let total = total.ok_or_else(|| bad("N-bit"))?;
    if total != expected {
        return Err(bad("N-bit"));
    }
    // The reader layer appends the sign (see `layout::dataset`). The file type keeps the value
    // at bit `off`, but the reader treats every integer as full width, so the value is
    // returned already shifted down and sign-extended, as HDF5's conversion to a native type
    // would.
    let signed = params.get(8) == Some(&1);
    let mut out = vec![0u8; total];
    let mut r = Bits { b: src, pos: 0 };
    let s = 64 - prec;
    for e in out.chunks_exact_mut(size) {
        let v = r.read(prec)?;
        let v = if signed {
            (((v << s) as i64) >> s) as u64
        } else {
            v
        };
        put(e, v, be);
    }
    Ok(out)
}

/// Undoes the scale-offset filter on one chunk of `expected` bytes.
pub(crate) fn scale_offset(src: &[u8], params: &[u32], expected: usize) -> Result<Vec<u8>> {
    let p = |i: usize| {
        params
            .get(i)
            .copied()
            .ok_or_else(|| Error::Malformed("scale-offset parameters".into()))
    };
    let scale_type = p(0)?;
    let scale = p(1)? as i32;
    let n = p(2)? as usize;
    let float = match p(3)? {
        0 => false,
        1 => true,
        c => return Err(Error::Unsupported(format!("scale-offset class {c}"))),
    };
    let size = p(4)? as usize;
    let signed = p(5)? == 1;
    let be = p(6)? == 1;
    let fill_defined = p(7)? == 1;
    if !matches!(size, 1 | 2 | 4 | 8) || (float && !matches!(size, 4 | 8)) {
        return Err(Error::Unsupported(format!(
            "scale-offset of {size}-byte data"
        )));
    }
    if float && scale_type != 0 {
        return Err(Error::Unsupported("scale-offset E-scaling".into()));
    }
    if n.checked_mul(size) != Some(expected) {
        return Err(bad("scale-offset"));
    }
    // The fill value, kept in the parameters as native (little-endian) bytes.
    let fill: Vec<u8> = params
        .get(8..)
        .unwrap_or_default()
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .take(size)
        .collect();
    let fill = (fill.len() == size).then(|| get(&fill, false));
    if src.len() < 21 {
        return Err(bad("scale-offset"));
    }
    let minbits = u32::from_le_bytes([src[0], src[1], src[2], src[3]]);
    let mlen = usize::from(src[4]).min(8);
    let minval = get(&src[5..5 + mlen], false);
    let body = &src[21..];
    let bits = size as u32 * 8;
    if minbits > bits {
        return Err(bad("scale-offset"));
    }
    let mut out = vec![0u8; expected];
    if minbits == bits {
        let raw = body.get(..expected).ok_or_else(|| bad("scale-offset"))?;
        for (o, e) in out.chunks_exact_mut(size).zip(raw.chunks_exact(size)) {
            put(o, get(e, false), be);
        }
        return Ok(out);
    }
    let mask = if bits == 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    };
    let fill_code = if minbits >= 64 {
        u64::MAX
    } else {
        (1u64 << minbits) - 1
    };
    let mut r = Bits { b: body, pos: 0 };
    let sext = |v: u64| -> i64 {
        let s = 64 - bits;
        ((v << s) as i64) >> s
    };
    let fill = if fill_defined { fill } else { None };
    if float {
        let pow = 10f64.powi(scale);
        let min_f32 = f32::from_bits(minval as u32);
        let min_f64 = f64::from_bits(minval);
        for o in out.chunks_exact_mut(size) {
            let v = if minbits == 0 { 0 } else { r.read(minbits)? };
            let x = match fill {
                Some(f) if v == fill_code => f,
                _ if size == 4 => {
                    let i = v as u32 as i32;
                    let f = i as f32 / pow as f32 + min_f32;
                    u64::from(f.to_bits())
                }
                _ => {
                    let f = v as i64 as f64 / pow + min_f64;
                    f.to_bits()
                }
            };
            put(o, x, be);
        }
    } else {
        for o in out.chunks_exact_mut(size) {
            let v = if minbits == 0 { 0 } else { r.read(minbits)? };
            let x = match fill {
                Some(f) if v == fill_code => f,
                _ if signed => (sext(v).wrapping_add(sext(minval & mask))) as u64 & mask,
                _ => v.wrapping_add(minval) & mask,
            };
            put(o, x, be);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_reader() {
        let mut r = Bits {
            b: &[0b1011_0011, 0b1100_0000],
            pos: 0,
        };
        assert_eq!(r.read(3).unwrap(), 0b101);
        assert_eq!(r.read(7).unwrap(), 0b100_1111);
        assert!(r.read(7).is_err());
    }

    #[test]
    fn nbit_atomic() {
        // Two u16 values of 4 bits at offset 2: 0b1010 and 0b0111.
        let params = [8, 0, 2, 1, 2, 0, 4, 2];
        let out = nbit(&[0b1010_0111], &params, 4).unwrap();
        assert_eq!(out, [0b1010, 0, 0b0111, 0]);
        // Signed: 0b1010 is -6 in four bits.
        let params = [8, 0, 2, 1, 2, 0, 4, 2, 1];
        let out = nbit(&[0b1010_0111], &params, 4).unwrap();
        assert_eq!(out, [0xfa, 0xff, 0b0111, 0]);
    }

    #[test]
    fn scale_offset_ints() {
        // Three i16 values 100, 103 and 107: minimum 100, 3 bits each.
        let mut src = vec![3, 0, 0, 0, 8];
        src.extend_from_slice(&100u64.to_le_bytes());
        src.resize(21, 0);
        src.extend_from_slice(&[0b0000_1111, 0b1000_0000]);
        let params = [2, 0, 3, 0, 2, 1, 0, 0];
        let out = scale_offset(&src, &params, 6).unwrap();
        let v: Vec<i16> = out
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(v, [100, 103, 107]);
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    #[test]
    fn nbit_offset_overflow() {
        // One 1-byte element of 1 bit at bit offset 2^32 - 1.
        let params = [8, 0, 1, 1, 1, 0, 1, u32::MAX];
        assert!(nbit(&[0], &params, 1).is_err());
    }
}
