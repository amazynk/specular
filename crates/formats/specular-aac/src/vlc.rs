//! Prefix-code decoding through two-level lookup tables built from (code, length, symbol)
//! lists. Codes up to 19 bits (the scalefactor codebook's longest) are supported.

use crate::bits::Bits;

const ROOT: u32 = 9;

#[derive(Clone, Copy, Default)]
struct Entry {
    /// Symbol, or the start of a second-level table when `len` is 0 and `sub` is set.
    value: u32,
    /// Code length in bits for a leaf (relative to the table's level); 0 marks an empty slot.
    len: u8,
    /// Bits indexing the second-level table, for a link entry.
    sub: u8,
}

pub(crate) struct Vlc {
    table: Vec<Entry>,
}

impl Vlc {
    /// Builds a decoder; `None` if the codes are not a valid prefix code or are too long.
    pub fn new(codes: &[(u32, u8, u32)]) -> Option<Vlc> {
        let max = codes.iter().map(|c| c.1).max()?;
        if max > 24 || codes.iter().any(|c| c.1 == 0 || u64::from(c.0) >> c.1 != 0) {
            return None;
        }
        let mut table = vec![Entry::default(); 1 << ROOT];
        // group long codes by their root prefix to size the second-level tables
        let mut sub_bits = vec![0u8; 1 << ROOT];
        for &(code, len, _) in codes {
            if u32::from(len) > ROOT {
                let prefix = (code >> (u32::from(len) - ROOT)) as usize;
                sub_bits[prefix] = sub_bits[prefix].max(len - ROOT as u8);
            }
        }
        for (prefix, &bits) in sub_bits.iter().enumerate() {
            if bits > 0 {
                let start = table.len();
                table.resize(start + (1 << bits), Entry::default());
                table[prefix] = Entry {
                    value: start as u32,
                    len: 0,
                    sub: bits,
                };
            }
        }
        for &(code, len, sym) in codes {
            let len32 = u32::from(len);
            if len32 <= ROOT {
                let shift = ROOT - len32;
                let base = (code << shift) as usize;
                for slot in &mut table[base..base + (1 << shift)] {
                    if slot.len != 0 || slot.sub != 0 {
                        return None;
                    }
                    *slot = Entry {
                        value: sym,
                        len,
                        sub: 0,
                    };
                }
            } else {
                let prefix = (code >> (len32 - ROOT)) as usize;
                let link = table[prefix];
                let bits = u32::from(link.sub);
                let rest = len32 - ROOT;
                let low = code & ((1 << rest) - 1);
                let shift = bits - rest;
                let base = link.value as usize + (low << shift) as usize;
                for slot in &mut table[base..base + (1 << shift)] {
                    if slot.len != 0 {
                        return None;
                    }
                    *slot = Entry {
                        value: sym,
                        len: len - ROOT as u8,
                        sub: 0,
                    };
                }
            }
        }
        Some(Vlc { table })
    }

    /// Decodes one symbol; `None` for a bit pattern that is no code.
    #[inline]
    pub fn read(&self, b: &mut Bits) -> Option<u32> {
        let e = self.table[b.peek(ROOT) as usize];
        if e.len != 0 {
            b.skip(usize::from(e.len));
            return Some(e.value);
        }
        if e.sub == 0 {
            return None;
        }
        b.skip(ROOT as usize);
        let idx = e.value as usize + b.peek(u32::from(e.sub)) as usize;
        let e2 = self.table[idx];
        if e2.len == 0 {
            return None;
        }
        b.skip(usize::from(e2.len));
        Some(e2.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_short_and_long_codes() {
        // 0, 10, 110, 1110, then 12-bit codes under the 1111 prefix
        let mut codes = vec![
            (0b0, 1, 100),
            (0b10, 2, 101),
            (0b110, 3, 102),
            (0b1110, 4, 103),
        ];
        for i in 0..256u32 {
            codes.push(((0b1111 << 8) | i, 12, i));
        }
        let v = Vlc::new(&codes).unwrap();
        // 0 | 10 | 1111 0000 0101 | 110 | 1110 + padding
        let data = [0b0101_1110, 0b0000_1011, 0b1011_1000];
        let mut b = Bits::new(&data);
        assert_eq!(v.read(&mut b), Some(100));
        assert_eq!(v.read(&mut b), Some(101));
        assert_eq!(v.read(&mut b), Some(5));
        assert_eq!(v.read(&mut b), Some(102));
        assert_eq!(v.read(&mut b), Some(103));
        assert_eq!(b.pos(), 22);
    }

    #[test]
    fn rejects_bad_codes() {
        assert!(Vlc::new(&[(0, 1, 0), (0, 2, 1)]).is_none());
        assert!(Vlc::new(&[(4, 2, 0)]).is_none());
        // an incomplete code reads as invalid, not as a panic
        let v = Vlc::new(&[(0, 1, 7)]).unwrap();
        assert_eq!(v.read(&mut Bits::new(&[0x80])), None);
    }
}
