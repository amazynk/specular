//! Codebooks (spec section 3): the Huffman trees and their VQ value vectors.

use crate::Error;
use crate::bits::{BitReader, ilog};

/// Codewords up to this long decode with one table lookup.
const FAST_BITS: u32 = 10;

/// A decoded codebook.
pub(crate) struct Codebook {
    pub(crate) dims: usize,
    /// Indexed by the next `fast_bits` bits: `entry << 6 | length`, 0 for none (longer code).
    fast: Vec<u32>,
    fast_bits: u32,
    /// Codes longer than `fast_bits`: (codeword MSB-first left aligned, entry, length), sorted.
    long: Vec<(u32, u32, u8)>,
    /// The entry of a single-entry book (it sinks one bit, whatever its value).
    single: Option<u32>,
    /// `entries * dims` values when the book has a value mapping.
    pub(crate) values: Option<Vec<f32>>,
}

fn malformed(s: &str) -> Error {
    Error::Malformed(s.to_string())
}

/// `float32_unpack` (spec 9.2.2).
fn float32_unpack(x: u32) -> f32 {
    let mantissa = f64::from(x & 0x1f_ffff);
    let exponent = ((x & 0x7fe0_0000) >> 21) as i32;
    let v = mantissa * 2f64.powi(exponent - 788);
    (if x & 0x8000_0000 != 0 { -v } else { v }) as f32
}

/// `lookup1_values` (spec 9.2.3): the greatest r with r^dims <= entries.
fn lookup1_values(entries: u32, dims: usize) -> u32 {
    let pow_le = |r: u64| {
        let mut p = 1u64;
        for _ in 0..dims {
            p = p.saturating_mul(r);
            if p > u64::from(entries) {
                return false;
            }
        }
        true
    };
    let mut r = (f64::from(entries).powf(1.0 / dims as f64)).floor() as u64;
    while r > 0 && !pow_le(r) {
        r -= 1;
    }
    while pow_le(r + 1) {
        r += 1;
    }
    r as u32
}

impl Codebook {
    /// Reads one codebook from the setup header. `budget` is the number of codebook entries
    /// and value scalars the remaining books may still use.
    pub(crate) fn parse(br: &mut BitReader, budget: &mut u64) -> Result<Codebook, Error> {
        let eop = || malformed("the setup header ends early");
        if br.read(24).ok_or_else(eop)? != 0x56_4342 {
            return Err(malformed("bad codebook sync pattern"));
        }
        let dims = br.read(16).ok_or_else(eop)? as usize;
        let entries = br.read(24).ok_or_else(eop)?;
        let over = || Error::Unsupported("Vorbis with oversized codebooks".to_string());
        *budget = budget.checked_sub(u64::from(entries)).ok_or_else(over)?;
        let ordered = br.bit().ok_or_else(eop)?;
        // Codeword lengths; 0 marks an unused entry.
        let mut lengths = vec![0u8; entries as usize];
        if !ordered {
            let sparse = br.bit().ok_or_else(eop)?;
            for l in lengths.iter_mut() {
                if !sparse || br.bit().ok_or_else(eop)? {
                    *l = br.read(5).ok_or_else(eop)? as u8 + 1;
                }
            }
        } else {
            let mut cur = 0u32;
            let mut len = br.read(5).ok_or_else(eop)? + 1;
            while cur < entries {
                if len > 32 {
                    return Err(malformed("a codeword is longer than 32 bits"));
                }
                let number = br.read(ilog(entries - cur)).ok_or_else(eop)?;
                if number > entries - cur {
                    return Err(malformed("too many codewords"));
                }
                lengths[cur as usize..(cur + number) as usize].fill(len as u8);
                cur += number;
                len += 1;
            }
        }
        let lookup_type = br.read(4).ok_or_else(eop)?;
        let values = match lookup_type {
            0 => None,
            1 | 2 => {
                let min = float32_unpack(br.read(32).ok_or_else(eop)?);
                let delta = float32_unpack(br.read(32).ok_or_else(eop)?);
                let value_bits = br.read(4).ok_or_else(eop)? + 1;
                let sequence_p = br.bit().ok_or_else(eop)?;
                let total = entries as u64 * dims as u64;
                *budget = budget.checked_sub(total).ok_or_else(over)?;
                let lookup_values = if lookup_type == 1 {
                    if dims == 0 {
                        0
                    } else {
                        u64::from(lookup1_values(entries, dims))
                    }
                } else {
                    total
                };
                // Each multiplicand costs value_bits of the packet: bound before allocating.
                if lookup_values.saturating_mul(u64::from(value_bits)) > 8 << 24 {
                    return Err(malformed("the codebook value table is too large"));
                }
                let mut mult = Vec::with_capacity(lookup_values as usize);
                for _ in 0..lookup_values {
                    mult.push(br.read(value_bits).ok_or_else(eop)? as f32);
                }
                let mut v = Vec::with_capacity(total as usize);
                for e in 0..entries as usize {
                    let mut last = 0f32;
                    if lookup_type == 1 {
                        let mut divisor = 1usize;
                        for _ in 0..dims {
                            let off = (e / divisor) % mult.len().max(1);
                            let x = mult.get(off).copied().unwrap_or(0.0) * delta + min + last;
                            v.push(x);
                            if sequence_p {
                                last = x;
                            }
                            divisor = divisor.saturating_mul(mult.len());
                        }
                    } else {
                        for j in 0..dims {
                            let x = mult[e * dims + j] * delta + min + last;
                            v.push(x);
                            if sequence_p {
                                last = x;
                            }
                        }
                    }
                }
                Some(v)
            }
            _ => {
                return Err(Error::Unsupported(
                    "Vorbis with a reserved codebook lookup type".to_string(),
                ));
            }
        };
        let mut book = Codebook {
            dims,
            fast: Vec::new(),
            fast_bits: 0,
            long: Vec::new(),
            single: None,
            values,
        };
        book.build_tree(&lengths)?;
        Ok(book)
    }

    /// Assigns codewords in entry order, each the lowest free one of its length (spec 3.2.1).
    fn build_tree(&mut self, lengths: &[u8]) -> Result<(), Error> {
        let used = lengths.iter().filter(|&&l| l != 0).count();
        if used == 0 {
            return Ok(());
        }
        if used == 1 {
            let (e, &l) = lengths
                .iter()
                .enumerate()
                .find(|(_, l)| **l != 0)
                .unwrap_or((0, &1));
            if l != 1 {
                return Err(malformed("a single-entry codebook with a long codeword"));
            }
            self.single = Some(e as u32);
            return Ok(());
        }
        let max_len = u32::from(lengths.iter().copied().max().unwrap_or(0));
        self.fast_bits = max_len.min(FAST_BITS);
        self.fast = vec![0; 1 << self.fast_bits];
        // available[l]: the free codeword of length l (left aligned), 0 for none.
        let mut available = [0u32; 33];
        let mut first = true;
        for (e, &l) in lengths.iter().enumerate() {
            if l == 0 {
                continue;
            }
            let len = u32::from(l);
            let code = if first {
                first = false;
                for (i, a) in available
                    .iter_mut()
                    .enumerate()
                    .take(len as usize + 1)
                    .skip(1)
                {
                    *a = 1u32 << (32 - i);
                }
                0
            } else {
                let mut z = len as usize;
                while z > 0 && available[z] == 0 {
                    z -= 1;
                }
                if z == 0 {
                    return Err(malformed("an overspecified codebook"));
                }
                let res = available[z];
                available[z] = 0;
                for (y, a) in available
                    .iter_mut()
                    .enumerate()
                    .take(len as usize + 1)
                    .skip(z + 1)
                {
                    *a = res + (1u32 << (32 - y));
                }
                res
            };
            if len <= self.fast_bits {
                let rev = code.reverse_bits();
                let packed = (e as u32) << 6 | len;
                for f in 0..1u32 << (self.fast_bits - len) {
                    self.fast[(rev | f << len) as usize] = packed;
                }
            } else {
                self.long.push((code, e as u32, l));
            }
        }
        self.long.sort_unstable_by_key(|&(c, _, _)| c);
        Ok(())
    }

    /// Reads one entry number (scalar context); None at the end of the packet or for a bit
    /// pattern that is no codeword.
    #[inline]
    pub(crate) fn decode(&self, br: &mut BitReader) -> Option<u32> {
        if let Some(e) = self.single {
            br.read(1)?;
            return Some(e);
        }
        let (bits, avail) = br.peek32();
        if let Some(&p) = self.fast.get((bits & ((1 << self.fast_bits) - 1)) as usize)
            && p != 0
        {
            let len = p & 63;
            if len > avail {
                br.exhaust();
                return None;
            }
            br.consume(len);
            return Some(p >> 6);
        }
        let x = bits.reverse_bits();
        let i = self.long.partition_point(|&(c, _, _)| c <= x);
        let &(code, entry, len) = self.long.get(i.checked_sub(1)?)?;
        let len = u32::from(len);
        if len > avail || (x ^ code) >> (32 - len) != 0 {
            br.exhaust();
            return None;
        }
        br.consume(len);
        Some(entry)
    }

    /// Reads one entry and returns its value vector (VQ context).
    #[inline]
    pub(crate) fn decode_vq<'s>(&'s self, br: &mut BitReader) -> Option<&'s [f32]> {
        let e = self.decode(br)? as usize;
        let v = self.values.as_deref()?;
        v.get(e * self.dims..(e + 1) * self.dims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(lookup1_values(81, 4), 3);
        assert_eq!(lookup1_values(80, 4), 2);
        assert_eq!(lookup1_values(1, 7), 1);
        assert_eq!(lookup1_values(16_777_215, 1), 16_777_215);
        assert_eq!(float32_unpack(0), 0.0);
        // 1 * 2^(788 - 788) = 1, with sign.
        assert_eq!(float32_unpack(788 << 21 | 1), 1.0);
        assert_eq!(float32_unpack(0x8000_0000 | 789 << 21 | 3), -6.0);
    }

    #[test]
    fn spec_tree() {
        // Section 3.2.1's example: lengths 2 4 4 4 4 2 3 3.
        let mut b = Codebook {
            dims: 1,
            fast: Vec::new(),
            fast_bits: 0,
            long: Vec::new(),
            single: None,
            values: None,
        };
        b.build_tree(&[2, 4, 4, 4, 4, 2, 3, 3]).unwrap();
        // Codewords MSB first: 00, 0100, 0101, 0110, 0111, 10, 110, 111. Write entries
        // 6, 1, 5, 7, 0 as a bit string read first-bit-lowest.
        let s = "110".to_string() + "0100" + "10" + "111" + "00";
        let mut bytes = vec![0u8; 4];
        for (i, c) in s.chars().enumerate() {
            if c == '1' {
                bytes[i / 8] |= 1 << (i % 8);
            }
        }
        let mut r = BitReader::new(&bytes);
        let got: Vec<u32> = (0..5).map(|_| b.decode(&mut r).unwrap()).collect();
        assert_eq!(got, [6, 1, 5, 7, 0]);
        let mut over = Codebook { ..b };
        assert!(over.build_tree(&[1, 1, 1]).is_err());
    }
}
