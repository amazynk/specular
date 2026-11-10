//! The two entropy coders of RFC 9043 §3.8: the binary range coder with its adaptive states
//! (and the integers built from it), and the Golomb-Rice bit reader with its per-context
//! adaptive parameters.

/// RFC 9043 Figure 24: the default state after a 1.
const DEFAULT_ONE: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37,
    37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 56, 57, 58, 59,
    60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 75, 76, 77, 78, 79, 80, 81, 82,
    83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103,
    104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 114, 115, 116, 117, 118, 119, 120, 121,
    122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 133, 134, 135, 136, 137, 138, 139,
    140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 152, 153, 154, 155, 156, 157,
    158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 171, 172, 173, 174, 175,
    176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 190, 191, 192, 194,
    194, 195, 196, 197, 198, 199, 200, 201, 202, 202, 204, 205, 206, 207, 208, 209, 209, 210, 211,
    212, 213, 215, 215, 216, 217, 218, 219, 220, 220, 222, 223, 224, 225, 226, 227, 227, 229, 229,
    230, 231, 232, 234, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248,
    248, 0, 0, 0, 0, 0, 0, 0,
];

/// The state transitions of the range coder (RFC 9043 §3.8.1.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transitions {
    pub one: [u8; 256],
    pub zero: [u8; 256],
}

impl Default for Transitions {
    fn default() -> Self {
        Transitions::from_one(DEFAULT_ONE)
    }
}

impl Transitions {
    /// The table after a 0 mirrors the one after a 1: `zero[i] = 256 - one[256 - i]`.
    pub fn from_one(one: [u8; 256]) -> Transitions {
        let mut zero = [0u8; 256];
        for i in 1..256 {
            zero[256 - i] = 0u8.wrapping_sub(one[i]);
        }
        Transitions { one, zero }
    }

    /// The custom table of coder type 2: the default plus the coded deltas.
    pub fn custom(delta: &[i32; 256]) -> Transitions {
        let mut one = DEFAULT_ONE;
        for i in 1..256 {
            one[i] = (i32::from(DEFAULT_ONE[i]) + delta[i]) as u8;
        }
        Transitions::from_one(one)
    }
}

/// The state array of one integer context (RFC 9043 `CONTEXT_SIZE`).
pub type States = [u8; 32];

/// The binary range decoder (RFC 9043 §3.8.1.1), closed mode: bytes past the end read as 0.
pub struct RangeDecoder<'a> {
    buf: &'a [u8],
    /// Bytes taken into `low` so far (the RFC's `j`).
    pos: usize,
    low: u32,
    range: u32,
    ended: bool,
    /// A coded integer longer than 32 bits was met (damaged data).
    pub overflow: bool,
}

impl<'a> RangeDecoder<'a> {
    pub fn new(buf: &'a [u8]) -> RangeDecoder<'a> {
        let b = |i: usize| u32::from(buf.get(i).copied().unwrap_or(0));
        let mut c = RangeDecoder {
            buf,
            pos: 2,
            low: b(0) << 8 | b(1),
            range: 0xFF00,
            ended: buf.len() <= 2,
            overflow: false,
        };
        if c.low >= c.range {
            c.low = c.range;
            c.ended = true;
        }
        c
    }

    #[inline(always)]
    fn refill(&mut self) {
        if self.range < 0x100 {
            self.range <<= 8;
            self.low <<= 8;
            if !self.ended {
                if let Some(&b) = self.buf.get(self.pos) {
                    self.low |= u32::from(b);
                }
                self.pos += 1;
                if self.pos >= self.buf.len() {
                    self.ended = true;
                }
            }
        }
    }

    /// One binary decision under `state`, which adapts.
    #[inline(always)]
    pub fn bit(&mut self, state: &mut u8, t: &Transitions) -> bool {
        let split = (self.range * u32::from(*state)) >> 8;
        self.range -= split;
        if self.low < self.range {
            *state = t.zero[usize::from(*state)];
            self.refill();
            false
        } else {
            self.low -= self.range;
            self.range = split;
            *state = t.one[usize::from(*state)];
            self.refill();
            true
        }
    }

    /// An integer (RFC 9043 Figure 21): zero, else exponent, mantissa and sign.
    #[inline(always)]
    pub fn symbol(&mut self, s: &mut States, t: &Transitions, signed: bool) -> i32 {
        if self.bit(&mut s[0], t) {
            return 0;
        }
        let mut e = 0usize;
        while self.bit(&mut s[1 + e.min(9)], t) {
            e += 1;
            if e > 31 {
                self.overflow = true;
                return 0;
            }
        }
        let mut a: u32 = 1;
        for i in (0..e).rev() {
            a = a << 1 | u32::from(self.bit(&mut s[22 + i.min(9)], t));
        }
        let a = a as i32;
        if signed && self.bit(&mut s[11 + e.min(10)], t) {
            a.wrapping_neg()
        } else {
            a
        }
    }

    /// An unsigned header value.
    pub fn unsigned(&mut self, s: &mut States, t: &Transitions) -> u32 {
        self.symbol(s, t, false) as u32
    }

    /// Ends the range-coded part with the sentinel symbol (state 129) and gives the offset of
    /// the first byte after it (RFC 9043 §3.8.1.1.1): one byte before the decoder's position.
    pub fn sentinel(&mut self, t: &Transitions) -> usize {
        let mut s = 129u8;
        self.bit(&mut s, t);
        self.pos - 1
    }
}

/// Big-endian bit reader for the Golomb-Rice mode; reads past the end as zeros.
pub struct BitReader<'a> {
    buf: &'a [u8],
    /// Next byte to load into `acc`.
    at: usize,
    acc: u64,
    /// Valid bits in `acc` (from the top).
    have: u32,
}

impl<'a> BitReader<'a> {
    pub fn new(buf: &'a [u8]) -> BitReader<'a> {
        BitReader {
            buf,
            at: 0,
            acc: 0,
            have: 0,
        }
    }

    #[inline(always)]
    fn fill(&mut self) {
        if self.at + 8 <= self.buf.len() {
            let mut b = [0u8; 8];
            b.copy_from_slice(&self.buf[self.at..self.at + 8]);
            // The next eight bytes go below the valid bits; the whole bytes among them count
            // (bits past them are the stream's own and are loaded again, identically, later).
            self.acc |= u64::from_be_bytes(b) >> self.have;
            let take = (64 - self.have) / 8;
            self.at += take as usize;
            self.have += take * 8;
        } else {
            while self.have <= 56 {
                let b = self.buf.get(self.at).copied().unwrap_or(0);
                self.acc |= u64::from(b) << (56 - self.have);
                self.at += 1;
                self.have += 8;
            }
        }
    }

    /// `n` bits (0 to 32), most significant first.
    #[inline(always)]
    pub fn bits(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        if self.have < n {
            self.fill();
        }
        let v = (self.acc >> (64 - n)) as u32;
        self.acc <<= n;
        self.have -= n;
        v
    }

    #[inline(always)]
    pub fn bit(&mut self) -> bool {
        self.bits(1) != 0
    }

    /// RFC 9043 Figure 26: an unsigned Golomb-Rice code with parameter `k`; twelve zero
    /// bits escape to the value minus 11 in `esc` bits.
    #[inline(always)]
    fn unsigned_golomb(&mut self, k: u32, esc: u32) -> u32 {
        if self.have < 32 {
            self.fill();
        }
        let lead = self.acc.leading_zeros().min(12);
        if lead < 12 {
            self.acc <<= lead + 1;
            self.have -= lead + 1;
            (lead << k).wrapping_add(self.bits(k))
        } else {
            self.acc <<= 12;
            self.have -= 12;
            self.bits(esc).wrapping_add(11)
        }
    }

    /// RFC 9043 Figure 27: the signed form.
    #[inline(always)]
    fn signed_golomb(&mut self, k: u32, esc: u32) -> i32 {
        let v = self.unsigned_golomb(k, esc);
        if v & 1 != 0 {
            -((v >> 1) as i32) - 1
        } else {
            (v >> 1) as i32
        }
    }

    /// RFC 9043 §3.8.2.4: a sample difference under its context's adaptive state, `bits`
    /// wide.
    #[inline(always)]
    pub fn vlc(&mut self, s: &mut Vlc, bits: u32) -> i32 {
        let mut i = s.count;
        let mut k = 0u32;
        while i < s.error_sum && k < 24 {
            k += 1;
            i += i;
        }
        let mut v = self.signed_golomb(k, bits);
        if 2 * s.drift < -s.count {
            v = -1 - v;
        }
        let sh = 32 - bits;
        let ret = (v.wrapping_add(s.bias) << sh) >> sh;
        s.error_sum = s
            .error_sum
            .saturating_add(v.unsigned_abs().min(1 << 30) as i32);
        s.drift = s.drift.wrapping_add(v);
        if s.count == 128 {
            s.count >>= 1;
            s.drift >>= 1;
            s.error_sum >>= 1;
        }
        s.count += 1;
        if s.drift <= -s.count {
            s.bias = (s.bias - 1).max(-128);
            s.drift = (s.drift + s.count).max(-s.count + 1);
        } else if s.drift > 0 {
            s.bias = (s.bias + 1).min(127);
            s.drift = (s.drift - s.count).min(0);
        }
        ret
    }
}

/// The adaptive Golomb-Rice state of one context (RFC 9043 §3.8.2.5).
#[derive(Clone, Copy, Debug)]
pub struct Vlc {
    pub drift: i32,
    pub error_sum: i32,
    pub bias: i32,
    pub count: i32,
}

impl Default for Vlc {
    fn default() -> Self {
        Vlc {
            drift: 0,
            error_sum: 4,
            bias: 0,
            count: 1,
        }
    }
}

/// CRC-32 with the IEEE polynomial, most significant bit first, no inversions (RFC 9043
/// §4.9.3): a block whose parity is right leaves 0.
pub fn crc(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut c = (i as u32) << 24;
            for _ in 0..8 {
                c = if c & 0x8000_0000 != 0 {
                    c << 1 ^ 0x04C1_1DB7
                } else {
                    c << 1
                };
            }
            *e = c;
        }
        t
    });
    data.iter()
        .fold(0u32, |c, &b| c << 8 ^ t[usize::from((c >> 24) as u8 ^ b)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golomb_examples_of_the_rfc() {
        // RFC 9043 Table 3.
        let mut r = BitReader::new(&[0b1000_0000]);
        assert_eq!(r.unsigned_golomb(0, 8), 0);
        let mut r = BitReader::new(&[0b0010_0000]);
        assert_eq!(r.unsigned_golomb(0, 8), 2);
        let mut r = BitReader::new(&[0b1100_0000]);
        assert_eq!(r.unsigned_golomb(2, 8), 2);
        let mut r = BitReader::new(&[0b0101_0000]);
        assert_eq!(r.unsigned_golomb(2, 8), 5);
        let mut r = BitReader::new(&[0, 0b0000_1000, 0]);
        assert_eq!(r.unsigned_golomb(3, 8), 139);
    }

    #[test]
    fn bit_reader_crosses_words() {
        let data: Vec<u8> = (0..40u8).collect();
        let mut r = BitReader::new(&data);
        for (i, &b) in data.iter().enumerate() {
            assert_eq!(r.bits(3) << 5 | r.bits(5), u32::from(b), "byte {i}");
        }
        assert_eq!(r.bits(8), 0);
    }

    #[test]
    fn crc_of_a_block_with_its_parity_is_zero() {
        let mut d = b"FFV1 slice".to_vec();
        let c = crc(&d);
        d.extend(c.to_be_bytes());
        assert_eq!(crc(&d), 0);
    }

    #[test]
    fn transitions_mirror() {
        let t = Transitions::default();
        assert_eq!(t.zero[256 - 8], 0u8.wrapping_sub(t.one[8]));
        assert_eq!(t.one[128], 134);
    }
}
