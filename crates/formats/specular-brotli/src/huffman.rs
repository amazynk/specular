//! Prefix codes (RFC 7932 §3): reading their description and decoding symbols.
//!
//! A code decodes through a table indexed by the next [`ROOT`] input bits; codes longer than
//! that continue in a second-level table chosen by the root entry. Codes arrive least
//! significant bit first, so table indices are the canonical codes bit-reversed.

use crate::bits::BitReader;
use crate::{Error, Result};

const ROOT: u32 = 8;
const MAX_LEN: usize = 15;

/// A table entry: the symbol (or second-level table offset) in the high 16 bits, the bits it
/// uses in the low 8. A root entry using more than [`ROOT`] bits links to a second-level
/// table of `1 << (bits - ROOT)` entries.
#[derive(Clone, Debug, Default)]
pub(crate) struct Huffman {
    table: Vec<u32>,
}

impl Huffman {
    /// A code of one symbol, which takes no bits.
    pub(crate) fn single(sym: u16) -> Self {
        Self {
            table: vec![u32::from(sym) << 16; 1 << ROOT],
        }
    }

    /// The canonical code of `lens` (0: unused, 1..=15), which must be complete.
    pub(crate) fn from_lengths(lens: &[u8]) -> Result<Self> {
        let mut count = [0u32; MAX_LEN + 1];
        for &l in lens {
            count[usize::from(l)] += 1;
        }
        count[0] = 0;
        let kraft: u32 = (1..=MAX_LEN).map(|l| count[l] << (MAX_LEN - l)).sum();
        if kraft != 1 << MAX_LEN {
            return Err(Error::Invalid("incomplete prefix code"));
        }
        let mut next = [0u32; MAX_LEN + 1];
        let mut code = 0;
        for l in 1..=MAX_LEN {
            code = (code + count[l - 1]) << 1;
            next[l] = code;
        }
        // Second-level table sizes, per root index.
        let mut codes = Vec::with_capacity(lens.len());
        let mut sub = [0u8; 1 << ROOT];
        for (sym, &l) in lens.iter().enumerate() {
            if l == 0 {
                continue;
            }
            let l32 = u32::from(l);
            let rev = next[usize::from(l)].reverse_bits() >> (32 - l32);
            next[usize::from(l)] += 1;
            codes.push((rev, l32, sym as u32));
            if l32 > ROOT {
                let s = &mut sub[(rev & ((1 << ROOT) - 1)) as usize];
                *s = (*s).max((l32 - ROOT) as u8);
            }
        }
        let mut table = vec![0u32; 1 << ROOT];
        for (i, &s) in sub.iter().enumerate() {
            if s > 0 {
                table[i] = (table.len() as u32) << 16 | (ROOT + u32::from(s));
                table.resize(table.len() + (1 << s), 0);
            }
        }
        for (rev, l, sym) in codes {
            if l <= ROOT {
                let mut i = rev;
                while i < 1 << ROOT {
                    table[i as usize] = sym << 16 | l;
                    i += 1 << l;
                }
            } else {
                let link = table[(rev & ((1 << ROOT) - 1)) as usize];
                let base = (link >> 16) as usize;
                let size = 1u32 << ((link & 0xFF) - ROOT);
                let extra = l - ROOT;
                let mut i = rev >> ROOT;
                while i < size {
                    table[base + i as usize] = sym << 16 | extra;
                    i += 1 << extra;
                }
            }
        }
        Ok(Self { table })
    }

    /// Decodes one symbol.
    #[inline(always)]
    pub(crate) fn read(&self, br: &mut BitReader<'_>) -> Result<u16> {
        br.refill();
        let bits = br.peek();
        let mut e = self.table[(bits & ((1 << ROOT) - 1)) as usize];
        let mut n = e & 0xFF;
        if n > ROOT {
            let sub = n - ROOT;
            let i = (e >> 16) as usize + ((bits >> ROOT) & ((1 << sub) - 1)) as usize;
            e = *self.table.get(i).ok_or(Error::Invalid("prefix code"))?;
            n = ROOT + (e & 0xFF);
        }
        if n > br.available() {
            return Err(Error::Truncated);
        }
        br.consume(n);
        Ok((e >> 16) as u16)
    }

    /// Reads a code description (§3.4, §3.5) over an alphabet of `size` symbols.
    pub(crate) fn read_code(br: &mut BitReader<'_>, size: usize) -> Result<Self> {
        let hskip = br.read(2)?;
        if hskip == 1 {
            simple(br, size)
        } else {
            complex(br, size, hskip as usize)
        }
    }
}

fn simple(br: &mut BitReader<'_>, size: usize) -> Result<Huffman> {
    let nsym = br.read(2)? as usize + 1;
    let width = usize::BITS - (size - 1).leading_zeros();
    let mut syms = [0u16; 4];
    for i in 0..nsym {
        let s = br.read(width)? as usize;
        if s >= size || syms[..i].contains(&(s as u16)) {
            return Err(Error::Invalid("simple prefix code symbol"));
        }
        syms[i] = s as u16;
    }
    let lens: &[u8] = match nsym {
        1 => return Ok(Huffman::single(syms[0])),
        2 => &[1, 1],
        3 => &[1, 2, 2],
        _ if br.bit()? => &[1, 2, 3, 3],
        _ => &[2, 2, 2, 2],
    };
    // Lengths in symbol order: same-length codes go to symbols in increasing order, which the
    // canonical construction does by itself.
    let max = usize::from(*syms[..nsym].iter().max().unwrap_or(&0));
    let mut all = vec![0u8; max + 1];
    for (&s, &l) in syms.iter().zip(lens) {
        all[usize::from(s)] = l;
    }
    Huffman::from_lengths(&all)
}

/// The order code length code lengths are stored in.
const CL_ORDER: [usize; 18] = [1, 2, 3, 4, 0, 5, 17, 6, 16, 7, 8, 9, 10, 11, 12, 13, 14, 15];

fn complex(br: &mut BitReader<'_>, size: usize, hskip: usize) -> Result<Huffman> {
    // The code length code lengths, in a fixed variable-length code: by the next four bits,
    // the length (0..=5) and the bits it takes.
    const VALUE: [u8; 16] = [0, 4, 3, 2, 0, 4, 3, 1, 0, 4, 3, 2, 0, 4, 3, 5];
    const BITS: [u8; 16] = [2, 2, 2, 3, 2, 2, 2, 4, 2, 2, 2, 3, 2, 2, 2, 4];
    let mut cl = [0u8; 18];
    let mut space = 32i32;
    let mut used = 0;
    let mut last = 0;
    for &sym in &CL_ORDER[hskip..] {
        br.refill();
        let i = (br.peek() & 15) as usize;
        let n = u32::from(BITS[i]);
        if n > br.available() {
            return Err(Error::Truncated);
        }
        br.consume(n);
        let v = VALUE[i];
        cl[sym] = v;
        if v != 0 {
            space -= 32 >> v;
            used += 1;
            last = sym;
            if space <= 0 {
                break;
            }
        }
    }
    let cl_code = match (used, space) {
        (1, _) => Huffman::single(last as u16),
        (_, 0) => Huffman::from_lengths(&cl)?,
        _ => return Err(Error::Invalid("code length code")),
    };

    let mut lens = vec![0u8; size];
    let mut sym = 0;
    let mut prev = 8u8;
    let mut repeat = 0usize;
    let mut repeat_len = 0u8;
    let mut space = 32768i32;
    while sym < size && space > 0 {
        let c = cl_code.read(br)? as u8;
        if c < 16 {
            repeat = 0;
            lens[sym] = c;
            sym += 1;
            if c != 0 {
                prev = c;
                space -= 32768 >> c;
            }
        } else {
            let (extra, len) = if c == 16 { (2, prev) } else { (3, 0) };
            if repeat_len != len {
                repeat = 0;
                repeat_len = len;
            }
            let old = repeat;
            if repeat > 0 {
                repeat = (repeat - 2) << extra;
            }
            repeat += br.read(extra)? as usize + 3;
            let delta = repeat - old;
            if sym + delta > size {
                return Err(Error::Invalid("code length repeat past the alphabet"));
            }
            lens[sym..sym + delta].fill(len);
            sym += delta;
            if len != 0 {
                space -= (delta as i32) * (32768 >> len);
            }
        }
    }
    if space != 0 {
        return Err(Error::Invalid("incomplete prefix code"));
    }
    Huffman::from_lengths(&lens)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `(value, bits)` pairs least significant bit first.
    fn pack(fields: &[(u32, u32)]) -> Vec<u8> {
        let mut out = Vec::new();
        let (mut acc, mut n) = (0u64, 0);
        for &(v, k) in fields {
            acc |= u64::from(v) << n;
            n += k;
            while n >= 8 {
                out.push(acc as u8);
                acc >>= 8;
                n -= 8;
            }
        }
        if n > 0 {
            out.push(acc as u8);
        }
        out.extend_from_slice(&[0; 8]);
        out
    }

    #[test]
    fn canonical_codes() {
        // RFC 7932 §3.2's example: lengths (3, 3, 3, 3, 3, 2, 4, 4) give F=00, A=010, ...,
        // G=1110, H=1111, written most significant bit first.
        let h = Huffman::from_lengths(&[3, 3, 3, 3, 3, 2, 4, 4]).unwrap();
        let codes = [(0b010, 3), (0b011, 3), (0b100, 3), (0b101, 3)];
        let codes = codes
            .into_iter()
            .chain([(0b110, 3), (0b00, 2), (0b1110, 4), (0b1111, 4)]);
        for (sym, (code, len)) in codes.enumerate() {
            let rev = (code as u32).reverse_bits() >> (32 - len);
            let data = pack(&[(rev, len), (0x5A, 8)]);
            let mut br = BitReader::new(&data);
            assert_eq!(h.read(&mut br).unwrap(), sym as u16);
            assert_eq!(br.read(8).unwrap(), 0x5A);
        }
        assert!(Huffman::from_lengths(&[1, 2]).is_err());
        assert!(Huffman::from_lengths(&[1, 1, 1]).is_err());
    }

    #[test]
    fn long_codes_use_second_level() {
        // Lengths 1, 2, ..., 14, 15, 15: complete, with codes up to 15 bits.
        let mut lens: Vec<u8> = (1..=15).collect();
        lens.push(15);
        let h = Huffman::from_lengths(&lens).unwrap();
        for (sym, &l) in lens.iter().enumerate() {
            // Symbol k < 15 is k ones then a zero; the last two are all ones, then 0 / 1.
            let l = u32::from(l);
            let code = if sym < 15 {
                (1u32 << l) - 2
            } else {
                (1u32 << l) - 2 + (sym as u32 - 14)
            };
            let rev = code.reverse_bits() >> (32 - l);
            let data = pack(&[(rev, l), (0x3C, 8)]);
            let mut br = BitReader::new(&data);
            assert_eq!(h.read(&mut br).unwrap(), sym as u16, "{sym}");
            assert_eq!(br.read(8).unwrap(), 0x3C);
        }
    }
}
