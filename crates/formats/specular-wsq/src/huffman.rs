//! Canonical Huffman tables (JPEG-style: code counts per length 1–16, then the values) and the
//! entropy-coded bit stream: 0xFF bytes are followed by a stuffed 0x00, and a 0xFF followed by
//! anything else is a marker, which ends the stream (a new block, a restart or the end).
//!
//! Codes of up to [`FAST_BITS`] bits are resolved by one table lookup; longer ones walk the
//! per-length code ranges. As in the reference decoder, a marker is only recognised where a
//! new byte is needed, and a code cut short by a marker is dropped.

use crate::{Error, Result};

const FAST_BITS: u32 = 10;

#[derive(Clone, Debug)]
pub(crate) struct HuffTable {
    /// `(length, value)` by the next `FAST_BITS` bits; length 0: longer code (or none).
    fast: Vec<(u8, u8)>,
    /// Largest code of each length (index 1..=16), -1 when there is none.
    maxcode: [i32; 18],
    /// Code minus this gives the index into `values` for each length.
    offset: [i32; 17],
    values: Vec<u8>,
}

impl HuffTable {
    pub fn new(counts: &[u8; 16], values: &[u8]) -> Result<HuffTable> {
        let mut maxcode = [-1i32; 18];
        let mut offset = [0i32; 17];
        let mut fast = vec![(0u8, 0u8); 1 << FAST_BITS];
        let mut code: i32 = 0;
        let mut k: usize = 0;
        for len in 1..=16usize {
            let n = counts[len - 1] as usize;
            if n > 0 {
                offset[len] = k as i32 - code;
                for _ in 0..n {
                    if code >= 1 << len {
                        return Err(Error::Malformed("over-subscribed Huffman table".into()));
                    }
                    if len as u32 <= FAST_BITS {
                        let shift = FAST_BITS - len as u32;
                        let base = (code as usize) << shift;
                        for e in &mut fast[base..base + (1 << shift)] {
                            *e = (len as u8, values[k]);
                        }
                    }
                    code += 1;
                    k += 1;
                }
                maxcode[len] = code - 1;
            }
            code <<= 1;
        }
        Ok(HuffTable {
            fast,
            maxcode,
            offset,
            values: values.to_vec(),
        })
    }
}

/// What the stream gave instead of a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stop {
    /// A marker at this byte position.
    Marker(usize),
    /// The data ended.
    End,
    /// Bits that are no code of the table.
    Bad,
}

pub(crate) struct Bits<'a> {
    d: &'a [u8],
    /// Next byte to load.
    pos: usize,
    buf: u64,
    n: u32,
    /// A marker was met at `pos` (nothing past it is loaded).
    stop: Option<Stop>,
}

impl<'a> Bits<'a> {
    pub fn new(d: &'a [u8], pos: usize) -> Bits<'a> {
        Bits {
            d,
            pos,
            buf: 0,
            n: 0,
            stop: None,
        }
    }

    #[inline]
    fn refill(&mut self) {
        while self.n <= 56 && self.stop.is_none() {
            let Some(&b) = self.d.get(self.pos) else {
                self.stop = Some(Stop::End);
                break;
            };
            if b == 0xFF {
                match self.d.get(self.pos + 1) {
                    Some(0) => {
                        self.pos += 2;
                    }
                    Some(_) => {
                        self.stop = Some(Stop::Marker(self.pos));
                        break;
                    }
                    None => {
                        self.stop = Some(Stop::End);
                        break;
                    }
                }
            } else {
                self.pos += 1;
            }
            self.buf |= u64::from(b) << (56 - self.n);
            self.n += 8;
        }
    }

    /// One bit, or why there is none.
    #[inline]
    fn bit(&mut self) -> std::result::Result<u32, Stop> {
        if self.n == 0 {
            self.refill();
            if self.n == 0 {
                return Err(self.stop.unwrap_or(Stop::End));
            }
        }
        let b = (self.buf >> 63) as u32;
        self.buf <<= 1;
        self.n -= 1;
        Ok(b)
    }

    /// A value through `t`.
    #[inline]
    pub fn decode(&mut self, t: &HuffTable) -> std::result::Result<u8, Stop> {
        if self.n < 16 {
            self.refill();
        }
        if self.n >= 16 {
            let peek = (self.buf >> (64 - FAST_BITS)) as usize;
            let (len, v) = t.fast[peek];
            if len > 0 {
                self.buf <<= len;
                self.n -= u32::from(len);
                return Ok(v);
            }
            let mut code = (self.buf >> (64 - FAST_BITS)) as i32;
            let mut len = FAST_BITS as usize;
            while code > t.maxcode[len] {
                len += 1;
                if len > 16 {
                    return Err(Stop::Bad);
                }
                code = (self.buf >> (64 - len)) as i32;
            }
            self.buf <<= len;
            self.n -= len as u32;
            return t
                .values
                .get((code + t.offset[len]) as usize)
                .copied()
                .ok_or(Stop::Bad);
        }
        // Near a marker or the end: bit by bit.
        let mut code = self.bit()? as i32;
        let mut len = 1usize;
        while code > t.maxcode[len] {
            len += 1;
            if len > 16 {
                return Err(Stop::Bad);
            }
            code = (code << 1) | self.bit()? as i32;
        }
        let i = code + t.offset[len];
        t.values.get(i as usize).copied().ok_or(Stop::Bad)
    }

    /// `k` (8 or 16) raw bits.
    pub fn raw(&mut self, k: u32) -> std::result::Result<u32, Stop> {
        if self.n < k {
            self.refill();
            if self.n < k {
                return Err(self.stop.unwrap_or(Stop::End));
            }
        }
        let v = (self.buf >> (64 - k)) as u32;
        self.buf <<= k;
        self.n -= k;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_short_and_long() {
        // Lengths: two of 2 bits, one of 3, one of 12.
        let mut counts = [0u8; 16];
        counts[1] = 2;
        counts[2] = 1;
        counts[11] = 1;
        let t = HuffTable::new(&counts, &[10, 20, 30, 40]).unwrap();
        // 00 -> 10, 01 -> 20, 100 -> 30, 1010_0000_0000 -> 40
        let bits = "00".to_owned() + "01" + "100" + "101000000000" + "01";
        let mut bytes = Vec::new();
        let mut padded = bits.clone();
        while !padded.len().is_multiple_of(8) {
            padded.push('1');
        }
        for ch in padded.as_bytes().chunks(8) {
            bytes.push(u8::from_str_radix(std::str::from_utf8(ch).unwrap(), 2).unwrap());
        }
        bytes.extend([0xFF, 0xA1]);
        let mut b = Bits::new(&bytes, 0);
        let got: Vec<u8> = (0..5).map(|_| b.decode(&t).unwrap()).collect();
        assert_eq!(got, [10, 20, 30, 40, 20]);
        // The padding ones run into the marker.
        assert_eq!(b.decode(&t), Err(Stop::Marker(bytes.len() - 2)));
    }

    #[test]
    fn stuffed_ff() {
        let mut counts = [0u8; 16];
        counts[7] = 1; // one 8-bit code: 00000000
        let t = HuffTable::new(&counts, &[7]).unwrap();
        let d = [0x00, 0xFF, 0x00, 0x12, 0xFF, 0xA3];
        let mut b = Bits::new(&d, 0);
        assert_eq!(b.decode(&t), Ok(7));
        assert_eq!(b.raw(8), Ok(0xFF));
        assert_eq!(b.raw(8), Ok(0x12));
        assert_eq!(b.raw(8), Err(Stop::Marker(4)));
    }
}
