//! Reading Musepack's bit stream: most significant bit first, prefix codes from the tables in
//! `tables.rs`, and the two counting codes of SV8 (a number below a bound in truncated
//! binary, and the positions of `k` set bits among `n` by their combination rank).

use std::sync::OnceLock;

/// A prefix code: its entries (code left-aligned to 16 bits, length, symbol) from the
/// highest code down, and where to start looking for each value of the next 8 bits.
pub(crate) struct Code {
    entries: &'static [(u16, u8, i8)],
    start: [u8; 256],
}

impl Code {
    pub(crate) fn new(entries: &'static [(u16, u8, i8)]) -> Code {
        let mut start = [0u8; 256];
        for (p, s) in start.iter_mut().enumerate() {
            // the first entry whose top byte is at most p (the last entry's code is 0)
            let i = entries
                .iter()
                .position(|e| usize::from(e.0 >> 8) <= p)
                .unwrap_or(entries.len() - 1);
            *s = i as u8;
        }
        Code { entries, start }
    }
}

/// C(n, k) for n ≤ 32, k ≤ 16.
fn binomials() -> &'static [[u32; 17]; 33] {
    static B: OnceLock<[[u32; 17]; 33]> = OnceLock::new();
    B.get_or_init(|| {
        let mut b = [[0u32; 17]; 33];
        for n in 0..33 {
            b[n][0] = 1;
            for k in 1..17.min(n + 1) {
                b[n][k] = b[n - 1][k - 1] + if k < n { b[n - 1][k] } else { 0 };
            }
        }
        b
    })
}

/// A cursor over bytes, most significant bit first; reading past the end gives zeros and
/// is remembered.
pub(crate) struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(d: &'a [u8], pos: usize) -> Bits<'a> {
        Bits { d, pos }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Whether more bits were read than there are.
    pub(crate) fn overrun(&self) -> bool {
        self.pos > self.d.len() * 8
    }

    fn byte(&self, i: usize) -> u32 {
        u32::from(self.d.get(i).copied().unwrap_or(0))
    }

    /// The next 16 bits, without moving.
    fn peek16(&self) -> u32 {
        let i = self.pos >> 3;
        let w = (self.byte(i) << 16) | (self.byte(i + 1) << 8) | self.byte(i + 2);
        (w >> (8 - (self.pos & 7))) & 0xFFFF
    }

    /// `n` bits (at most 32) as a number.
    pub(crate) fn read(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        let mut left = n;
        while left > 0 {
            let take = left.min(16);
            let w = self.peek16() >> (16 - take);
            v = (v << take) | w;
            self.pos += take as usize;
            left -= take;
        }
        v
    }

    pub(crate) fn bit(&mut self) -> bool {
        self.read(1) != 0
    }

    /// One symbol of a prefix code.
    pub(crate) fn huff(&mut self, c: &Code) -> i32 {
        let p = self.peek16();
        let mut i = usize::from(c.start[(p >> 8) as usize]);
        while u32::from(c.entries[i].0) > p {
            i += 1;
        }
        let (_, len, sym) = c.entries[i];
        self.pos += usize::from(len);
        i32::from(sym)
    }

    /// A number from 0 to `max` in truncated binary.
    pub(crate) fn bounded(&mut self, max: u32) -> u32 {
        if max == 0 {
            return 0;
        }
        let values = max + 1;
        let len = 32 - (values - 1).leading_zeros();
        let lost = (1u32 << len) - values;
        let mut v = self.read(len - 1);
        if v >= lost {
            v = ((v << 1) | self.read(1)) - lost;
        }
        v
    }

    /// `k` set bits among `n` (1 ≤ k ≤ n/2, n ≤ 32), coded by their rank among all such
    /// patterns in truncated binary.
    pub(crate) fn combination(&mut self, k: u32, n: u32) -> u32 {
        let b = binomials();
        let total = b[n as usize][k as usize];
        let len = 32 - (total - 1).leading_zeros();
        // The reference decoder's table leaves this one entry at 0, and encoders follow it.
        let lost = if (k, n) == (16, 32) {
            0
        } else {
            (1u32 << len) - total
        };
        let mut code = self.read(len - 1);
        if code >= lost {
            code = ((code << 1) | self.read(1)).wrapping_sub(lost);
        }
        let (mut bits, mut k, mut n) = (0u32, k as usize, n as usize);
        while k > 0 && n > 0 {
            n -= 1;
            let c = b[n][k];
            if code >= c {
                bits |= 1 << n;
                code -= c;
                k -= 1;
            }
        }
        bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_matches_the_truncated_binary_lengths() {
        // 33 values: 6 bits, the 31 smallest coded in 5
        let d = [0b0000_0000u8, 0xFF];
        assert_eq!(Bits::new(&d, 0).bounded(32), 0);
        let mut r = Bits::new(&[0b1111_1000, 0], 0);
        assert_eq!(r.bounded(32), 31 * 2 - 31);
        assert_eq!(r.pos(), 6);
        // one value more than nothing: a single bit
        let mut r = Bits::new(&[0x80], 0);
        assert_eq!(r.bounded(1), 1);
        assert_eq!(r.pos(), 1);
    }

    #[test]
    fn combinations_rank_patterns() {
        // all 2-of-5 patterns decode to distinct values with two bits set
        let mut seen = std::collections::HashSet::new();
        for code in 0..10u32 {
            // C(5,2) = 10: 4 bits, 6 lost; codes below 6 take 3 bits
            let (val, len) = if code < 6 { (code, 3) } else { (code + 6, 4) };
            let d = [((val << (8 - len)) & 0xFF) as u8, 0];
            let bits = Bits::new(&d, 0).combination(2, 5);
            assert_eq!(bits.count_ones(), 2);
            assert!(bits < 32);
            assert!(seen.insert(bits));
        }
    }

    #[test]
    fn reads_across_bytes() {
        let d = [0xAB, 0xCD, 0xEF, 0x12, 0x34];
        let mut r = Bits::new(&d, 4);
        assert_eq!(r.read(32), 0xBCDE_F123);
        assert_eq!(r.read(4), 4);
        assert!(!r.overrun());
        r.read(1);
        assert!(r.overrun());
    }
}
