//! The adaptive arithmetic decoder of T.82 6.8.3 (the QM coder), with the probability
//! estimation table (Table 24). Register layout as in Table 25: `c` holds CHIGH in its upper
//! and CLOW in its lower 16 bits; bytes enter the top of CLOW.

/// (LSZ, NLPS, NMPS, SWTCH) per state, T.82 Table 24.
const TABLE: [(u16, u8, u8, bool); 113] = [
    (0x5a1d, 1, 1, true),
    (0x2586, 14, 2, false),
    (0x1114, 16, 3, false),
    (0x080b, 18, 4, false),
    (0x03d8, 20, 5, false),
    (0x01da, 23, 6, false),
    (0x00e5, 25, 7, false),
    (0x006f, 28, 8, false),
    (0x0036, 30, 9, false),
    (0x001a, 33, 10, false),
    (0x000d, 35, 11, false),
    (0x0006, 9, 12, false),
    (0x0003, 10, 13, false),
    (0x0001, 12, 13, false),
    (0x5a7f, 15, 15, true),
    (0x3f25, 36, 16, false),
    (0x2cf2, 38, 17, false),
    (0x207c, 39, 18, false),
    (0x17b9, 40, 19, false),
    (0x1182, 42, 20, false),
    (0x0cef, 43, 21, false),
    (0x09a1, 45, 22, false),
    (0x072f, 46, 23, false),
    (0x055c, 48, 24, false),
    (0x0406, 49, 25, false),
    (0x0303, 51, 26, false),
    (0x0240, 52, 27, false),
    (0x01b1, 54, 28, false),
    (0x0144, 56, 29, false),
    (0x00f5, 57, 30, false),
    (0x00b7, 59, 31, false),
    (0x008a, 60, 32, false),
    (0x0068, 62, 33, false),
    (0x004e, 63, 34, false),
    (0x003b, 32, 35, false),
    (0x002c, 33, 9, false),
    (0x5ae1, 37, 37, true),
    (0x484c, 64, 38, false),
    (0x3a0d, 65, 39, false),
    (0x2ef1, 67, 40, false),
    (0x261f, 68, 41, false),
    (0x1f33, 69, 42, false),
    (0x19a8, 70, 43, false),
    (0x1518, 72, 44, false),
    (0x1177, 73, 45, false),
    (0x0e74, 74, 46, false),
    (0x0bfb, 75, 47, false),
    (0x09f8, 77, 48, false),
    (0x0861, 78, 49, false),
    (0x0706, 79, 50, false),
    (0x05cd, 48, 51, false),
    (0x04de, 50, 52, false),
    (0x040f, 50, 53, false),
    (0x0363, 51, 54, false),
    (0x02d4, 52, 55, false),
    (0x025c, 53, 56, false),
    (0x01f8, 54, 57, false),
    (0x01a4, 55, 58, false),
    (0x0160, 56, 59, false),
    (0x0125, 57, 60, false),
    (0x00f6, 58, 61, false),
    (0x00cb, 59, 62, false),
    (0x00ab, 61, 63, false),
    (0x008f, 61, 32, false),
    (0x5b12, 65, 65, true),
    (0x4d04, 80, 66, false),
    (0x412c, 81, 67, false),
    (0x37d8, 82, 68, false),
    (0x2fe8, 83, 69, false),
    (0x293c, 84, 70, false),
    (0x2379, 86, 71, false),
    (0x1edf, 87, 72, false),
    (0x1aa9, 87, 73, false),
    (0x174e, 72, 74, false),
    (0x1424, 72, 75, false),
    (0x119c, 74, 76, false),
    (0x0f6b, 74, 77, false),
    (0x0d51, 75, 78, false),
    (0x0bb6, 77, 79, false),
    (0x0a40, 77, 48, false),
    (0x5832, 80, 81, true),
    (0x4d1c, 88, 82, false),
    (0x438e, 89, 83, false),
    (0x3bdd, 90, 84, false),
    (0x34ee, 91, 85, false),
    (0x2eae, 92, 86, false),
    (0x299a, 93, 87, false),
    (0x2516, 86, 71, false),
    (0x5570, 88, 89, true),
    (0x4ca9, 95, 90, false),
    (0x44d9, 96, 91, false),
    (0x3e22, 97, 92, false),
    (0x3824, 99, 93, false),
    (0x32b4, 99, 94, false),
    (0x2e17, 93, 86, false),
    (0x56a8, 95, 96, true),
    (0x4f46, 101, 97, false),
    (0x47e5, 102, 98, false),
    (0x41cf, 103, 99, false),
    (0x3c3d, 104, 100, false),
    (0x375e, 99, 93, false),
    (0x5231, 105, 102, false),
    (0x4c0f, 106, 103, false),
    (0x4639, 107, 104, false),
    (0x415e, 103, 99, false),
    (0x5627, 105, 106, true),
    (0x50e7, 108, 107, false),
    (0x4b85, 109, 103, false),
    (0x5597, 110, 109, false),
    (0x504f, 111, 107, false),
    (0x5a10, 110, 111, true),
    (0x5522, 112, 109, false),
    (0x59eb, 112, 111, true),
];

/// A state transition packed for the hot path: LSZ in bits 16.., next state after an MPS in
/// bits 8..15, after an LPS in bits 0..6 with the MPS switch in bit 7.
const fn packed() -> [u32; 113] {
    let mut out = [0u32; 113];
    let mut i = 0;
    while i < 113 {
        let (lsz, nlps, nmps, sw) = TABLE[i];
        out[i] =
            (lsz as u32) << 16 | (nmps as u32) << 8 | (nlps as u32) | if sw { 0x80 } else { 0 };
        i += 1;
    }
    out
}

static STATES: [u32; 113] = packed();

/// Per-context adaptive state: bit 7 is MPS, bits 0..6 the state index (0 at reset).
pub(crate) type Contexts = Vec<u8>;

pub(crate) struct Decoder<'a> {
    data: &'a [u8],
    pos: usize,
    c: u32,
    a: u32,
    ct: u32,
}

impl<'a> Decoder<'a> {
    /// INITDEC over the stripe coded data `scd` (already unstuffed). Past its end, zeros are
    /// read (6.8.3.8).
    pub(crate) fn new(scd: &'a [u8]) -> Decoder<'a> {
        let mut d = Decoder {
            data: scd,
            pos: 0,
            c: 0,
            a: 0x10000,
            ct: 0,
        };
        d.byte_in();
        d.c <<= 8;
        d.byte_in();
        d.c <<= 8;
        d.byte_in();
        d
    }

    #[inline]
    fn byte_in(&mut self) {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        self.c = self.c.wrapping_add(u32::from(b) << 8);
        self.ct = 8;
    }

    #[inline]
    fn renorm(&mut self) {
        loop {
            if self.ct == 0 {
                self.byte_in();
            }
            self.a <<= 1;
            self.c <<= 1;
            self.ct -= 1;
            if self.a >= 0x8000 {
                break;
            }
        }
        if self.ct == 0 {
            self.byte_in();
        }
    }

    /// DECODE (Figure 32) in context `st`.
    #[inline]
    pub(crate) fn decode(&mut self, st: &mut u8) -> u8 {
        let s = *st;
        let e = STATES[usize::from(s & 0x7f)];
        let lsz = e >> 16;
        let mps = s >> 7;
        self.a = self.a.wrapping_sub(lsz);
        if (self.c >> 16) < self.a {
            if self.a >= 0x8000 {
                return mps;
            }
            // MPS_EXCHANGE.
            let pix = if self.a < lsz {
                *st = lps_next(s, e);
                1 - mps
            } else {
                *st = (s & 0x80) | ((e >> 8) as u8 & 0x7f);
                mps
            };
            self.renorm();
            pix
        } else {
            // LPS_EXCHANGE.
            self.c = self.c.wrapping_sub(self.a << 16);
            let pix = if self.a < lsz {
                *st = (s & 0x80) | ((e >> 8) as u8 & 0x7f);
                mps
            } else {
                *st = lps_next(s, e);
                1 - mps
            };
            self.a = lsz;
            self.renorm();
            pix
        }
    }
}

#[inline]
fn lps_next(s: u8, e: u32) -> u8 {
    let mut mps = s & 0x80;
    if e & 0x80 != 0 {
        mps ^= 0x80;
    }
    mps | (e as u8 & 0x7f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(words: &[u16]) -> Vec<u8> {
        words
            .iter()
            .flat_map(|w| (0..16).rev().map(move |i| (w >> i) as u8 & 1))
            .collect()
    }

    /// The test sequence of T.82 7.1: 256 pixels in two contexts, 25 coded bytes.
    #[test]
    fn t82_test_sequence() {
        let pix = bits(&[
            0x05e0, 0x0000, 0x8b00, 0x01c4, 0x1700, 0x0034, 0x7fff, 0x1a3f, 0x951b, 0x05d8, 0x1d17,
            0xe770, 0x0000, 0x0000, 0x0656, 0x0e6a,
        ]);
        let cx = bits(&[
            0x0fe0, 0x0000, 0x0f00, 0x00f0, 0xff00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        let scd = [
            0x69, 0x89, 0x99, 0x5c, 0x32, 0xea, 0xfa, 0xa0, 0xd5, 0xff, 0x52, 0x7f, 0xff, 0xff,
            0xff, 0xc0, 0x00, 0x00, 0x00, 0x3f, 0xff, 0x2d, 0x20, 0x82, 0x91,
        ];
        let mut d = Decoder::new(&scd);
        let mut st = [0u8; 2];
        let out: Vec<u8> = cx.iter().map(|&c| d.decode(&mut st[c as usize])).collect();
        assert_eq!(out, pix);
    }
}
