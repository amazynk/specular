//! Table-driven variable-length code reading: a first-level lookup on the next `BITS` bits,
//! second-level tables for longer codes.

use crate::bits::Bits;

const BITS: u32 = 9;

/// Entry: bit 31 set: a sub-table (offset in bits 0..24, its index width in bits 24..29);
/// otherwise the symbol in bits 0..16 and the code length in bits 16..24 (0: no code).
#[derive(Clone)]
pub struct Vlc {
    table: Vec<u32>,
}

impl Vlc {
    /// A decoder for `codes[symbol] = (code, length)`; entries of length 0 have no code.
    pub fn new(codes: &[(u32, u8)]) -> Vlc {
        let list: Vec<(u32, u8, u16)> = codes
            .iter()
            .enumerate()
            .filter(|(_, c)| c.1 > 0)
            .map(|(i, &(c, l))| (c, l, i as u16))
            .collect();
        Vlc::with_symbols(&list)
    }

    /// A decoder for explicit `(code, length, symbol)` triples.
    pub fn with_symbols(list: &[(u32, u8, u16)]) -> Vlc {
        let mut table = vec![0u32; 1 << BITS];
        build(&mut table, 0, BITS, list, 0);
        Vlc { table }
    }

    /// The next symbol, or None for a bit pattern with no code.
    #[inline]
    pub fn read(&self, r: &mut Bits) -> Option<u16> {
        let w = r.peek32();
        let mut idx = (w >> (32 - BITS)) as usize;
        let mut e = self.table[idx];
        let mut used = 0;
        let mut width = BITS;
        while e & 0x8000_0000 != 0 {
            used += width;
            width = (e >> 24) & 0x1f;
            let off = (e & 0xff_ffff) as usize;
            idx = off + ((w << used) >> (32 - width)) as usize;
            e = self.table[idx];
        }
        let len = (e >> 16) & 0xff;
        if len == 0 {
            return None;
        }
        r.skip(len);
        Some(e as u16)
    }
}

/// Fills `table[base..base + 2^width]` with the codes of `list` whose first `prefix_len` bits
/// (already consumed) are skipped.
fn build(table: &mut Vec<u32>, base: usize, width: u32, list: &[(u32, u8, u16)], prefix_len: u32) {
    // Codes ending within this level.
    let mut longer: Vec<(u32, u8, u16)> = Vec::new();
    for &(code, len, sym) in list {
        let len = len as u32;
        let rest = len - prefix_len;
        // The bits of the code below the prefix.
        let bits = if rest >= 32 {
            code
        } else {
            code & ((1u32 << rest) - 1)
        };
        if rest <= width {
            let start = (bits << (width - rest)) as usize;
            for i in 0..(1usize << (width - rest)) {
                table[base + start + i] = (len << 16) | u32::from(sym);
            }
        } else {
            longer.push((code, len as u8, sym));
        }
    }
    // Group the longer codes by their next `width` bits into sub-tables.
    longer.sort_by_key(|&(c, l, _)| c >> (l as u32 - prefix_len - width));
    let mut i = 0;
    while i < longer.len() {
        let (c, l, _) = longer[i];
        let head = c >> (l as u32 - prefix_len - width) & ((1 << width) - 1);
        let mut j = i;
        let mut max_rest = 0;
        while j < longer.len() {
            let (c2, l2, _) = longer[j];
            if (c2 >> (l2 as u32 - prefix_len - width)) & ((1 << width) - 1) != head {
                break;
            }
            max_rest = max_rest.max(l2 as u32 - prefix_len - width);
            j += 1;
        }
        let sub_width = max_rest.min(BITS);
        let off = table.len();
        table.resize(off + (1 << sub_width), 0);
        table[base + head as usize] = 0x8000_0000 | (sub_width << 24) | off as u32;
        let group: Vec<_> = longer[i..j].to_vec();
        build(table, off, sub_width, &group, prefix_len + width);
        i = j;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_short_and_long_codes() {
        // 1, 01, 001, 0001xxxxxxxxxx (14-bit codes).
        let mut codes = vec![(1, 1), (1, 2), (1, 3)];
        for k in 0..4 {
            codes.push(((1 << 10) | k, 14));
        }
        let v = Vlc::new(&codes);
        // 1 | 01 | 0001 0000000011 | 001
        let data = [0b1010_0010, 0b0000_0001, 0b1001_0000];
        let mut r = Bits::new(&data);
        assert_eq!(v.read(&mut r), Some(0));
        assert_eq!(v.read(&mut r), Some(1));
        assert_eq!(v.read(&mut r), Some(6));
        assert_eq!(v.read(&mut r), Some(2));
        assert_eq!(r.position(), 1 + 2 + 14 + 3);
    }
}
