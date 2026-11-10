//! The MQ arithmetic decoder (T.88 Annex E) with the integer (A.2) and symbol ID (A.3)
//! procedures built on it.
//!
//! A context is one byte: probability state index << 1 | MPS.

/// (Qe, NMPS, NLPS, SWITCH) per state (Table E.1).
const QE: [(u32, u8, u8, u8); 47] = [
    (0x5601, 1, 1, 1),
    (0x3401, 2, 6, 0),
    (0x1801, 3, 9, 0),
    (0x0AC1, 4, 12, 0),
    (0x0521, 5, 29, 0),
    (0x0221, 38, 33, 0),
    (0x5601, 7, 6, 1),
    (0x5401, 8, 14, 0),
    (0x4801, 9, 14, 0),
    (0x3801, 10, 14, 0),
    (0x3001, 11, 17, 0),
    (0x2401, 12, 18, 0),
    (0x1C01, 13, 20, 0),
    (0x1601, 29, 21, 0),
    (0x5601, 15, 14, 1),
    (0x5401, 16, 14, 0),
    (0x5101, 17, 15, 0),
    (0x4801, 18, 16, 0),
    (0x3801, 19, 17, 0),
    (0x3401, 20, 18, 0),
    (0x3001, 21, 19, 0),
    (0x2801, 22, 19, 0),
    (0x2401, 23, 20, 0),
    (0x2201, 24, 21, 0),
    (0x1C01, 25, 22, 0),
    (0x1801, 26, 23, 0),
    (0x1601, 27, 24, 0),
    (0x1401, 28, 25, 0),
    (0x1201, 29, 26, 0),
    (0x1101, 30, 27, 0),
    (0x0AC1, 31, 28, 0),
    (0x09C1, 32, 29, 0),
    (0x08A1, 33, 30, 0),
    (0x0521, 34, 31, 0),
    (0x0441, 35, 32, 0),
    (0x02A1, 36, 33, 0),
    (0x0221, 37, 34, 0),
    (0x0141, 38, 35, 0),
    (0x0111, 39, 36, 0),
    (0x0085, 40, 37, 0),
    (0x0049, 41, 38, 0),
    (0x0025, 42, 39, 0),
    (0x0015, 43, 40, 0),
    (0x0009, 44, 41, 0),
    (0x0005, 45, 42, 0),
    (0x0001, 45, 43, 0),
    (0x5601, 46, 46, 0),
];

pub(crate) struct Mq<'a> {
    data: &'a [u8],
    pos: usize,
    c: u32,
    a: u32,
    ct: u32,
    /// Bytes read past the end of `data` (taken as 0xFF).
    overrun: usize,
}

impl<'a> Mq<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Mq<'a> {
        let mut m = Mq {
            data,
            pos: 0,
            c: 0,
            a: 0x8000,
            ct: 0,
            overrun: 0,
        };
        m.c = (m.byte(0) as u32) << 16;
        m.byte_in();
        m.c <<= 7;
        m.ct -= 7;
        m.a = 0x8000;
        m
    }

    #[inline]
    fn byte(&self, i: usize) -> u8 {
        self.data.get(i).copied().unwrap_or(0xFF)
    }

    #[inline]
    fn byte_in(&mut self) {
        if self.byte(self.pos) == 0xFF {
            if self.byte(self.pos + 1) > 0x8F {
                self.c = self.c.wrapping_add(0xFF00);
                self.ct = 8;
                if self.pos + 1 >= self.data.len() {
                    self.overrun += 1;
                }
            } else {
                self.pos += 1;
                self.c = self.c.wrapping_add((self.byte(self.pos) as u32) << 9);
                self.ct = 7;
            }
        } else {
            self.pos += 1;
            if self.pos >= self.data.len() {
                self.overrun += 1;
            }
            self.c = self.c.wrapping_add((self.byte(self.pos) as u32) << 8);
            self.ct = 8;
        }
    }

    /// Whether decoding has run well past the end of the data (a corrupt or cut file).
    pub(crate) fn exhausted(&self) -> bool {
        self.overrun > 256
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
            if self.a & 0x8000 != 0 {
                break;
            }
        }
    }

    /// Decodes one bit in context `cx`.
    #[inline(always)]
    pub(crate) fn decode(&mut self, cx: &mut u8) -> u32 {
        let i = (*cx >> 1) as usize;
        let mps = (*cx & 1) as u32;
        let (qe, nmps, nlps, sw) = QE[i.min(46)];
        let a = self.a.wrapping_sub(qe);
        let d;
        if (self.c >> 16) < qe {
            // LPS sub-interval (at the bottom).
            if a < qe {
                d = mps;
                *cx = (nmps << 1) | mps as u8;
            } else {
                d = 1 - mps;
                *cx = (nlps << 1) | (mps ^ sw as u32) as u8;
            }
            self.a = qe;
        } else {
            self.c -= qe << 16;
            self.a = a;
            if a & 0x8000 != 0 {
                return mps;
            }
            if a < qe {
                d = 1 - mps;
                *cx = (nlps << 1) | (mps ^ sw as u32) as u8;
            } else {
                d = mps;
                *cx = (nmps << 1) | mps as u8;
            }
        }
        self.renorm();
        d
    }
}

/// Contexts of one integer decoding procedure (IADH, IADW, ...).
pub(crate) struct IntCx(Box<[u8; 512]>);

impl IntCx {
    pub(crate) fn new() -> IntCx {
        IntCx(Box::new([0; 512]))
    }

    /// The integer decoding procedure (A.2): `None` is OOB.
    pub(crate) fn decode(&mut self, mq: &mut Mq) -> Option<i64> {
        let cx = &mut self.0;
        let mut prev = 1usize;
        let mut bit = |mq: &mut Mq| {
            let d = mq.decode(&mut cx[prev]) as usize;
            prev = if prev < 256 {
                (prev << 1) | d
            } else {
                (((prev << 1) | d) & 511) | 256
            };
            d as u64
        };
        let s = bit(mq);
        let (n, off) = if bit(mq) == 0 {
            (2, 0)
        } else if bit(mq) == 0 {
            (4, 4)
        } else if bit(mq) == 0 {
            (6, 20)
        } else if bit(mq) == 0 {
            (8, 84)
        } else if bit(mq) == 0 {
            (12, 340)
        } else {
            (32, 4436)
        };
        let mut v: u64 = 0;
        for _ in 0..n {
            v = (v << 1) | bit(mq);
        }
        let v = (v + off) as i64;
        if s == 1 {
            if v == 0 { None } else { Some(-v) }
        } else {
            Some(v)
        }
    }
}

/// Contexts of the symbol ID procedure (A.3) for codes of `len` bits.
pub(crate) struct IdCx {
    cx: Vec<u8>,
    len: u32,
}

impl IdCx {
    pub(crate) fn new(len: u32) -> IdCx {
        IdCx {
            cx: vec![0; 1usize << (len + 1)],
            len,
        }
    }

    pub(crate) fn decode(&mut self, mq: &mut Mq) -> u32 {
        let mut prev = 1usize;
        for _ in 0..self.len {
            let d = mq.decode(&mut self.cx[prev]) as usize;
            prev = (prev << 1) | d;
        }
        (prev - (1 << self.len)) as u32
    }
}

/// ceil(log2(n)), 0 for n <= 1.
pub(crate) fn code_len(n: u64) -> u32 {
    if n <= 1 {
        0
    } else {
        64 - (n - 1).leading_zeros()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test sequence of T.88 Annex H.2: 256 decisions from the given bytes, all in one
    /// context.
    #[test]
    fn annex_h2_sequence() {
        let data = [
            0x84, 0xC7, 0x3B, 0xFC, 0xE1, 0xA1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00, 0x41, 0x0D,
            0xBB, 0x86, 0xF4, 0x31, 0x7F, 0xFF, 0x88, 0xFF, 0x37, 0x47, 0x1A, 0xDB, 0x6A, 0xDF,
            0xFF, 0xAC,
        ];
        let want = [
            0x00, 0x02, 0x00, 0x51, 0x00, 0x00, 0x00, 0xC0, 0x03, 0x52, 0x87, 0x2A, 0xAA, 0xAA,
            0xAA, 0xAA, 0x82, 0xC0, 0x20, 0x00, 0xFC, 0xD7, 0x9E, 0xF6, 0xBF, 0x7F, 0xED, 0x90,
            0x4F, 0x46, 0xA3, 0xBF,
        ];
        let mut mq = Mq::new(&data);
        let mut cx = 0u8;
        let mut got = [0u8; 32];
        for (i, g) in got.iter_mut().enumerate() {
            for b in 0..8 {
                *g |= (mq.decode(&mut cx) as u8) << (7 - b);
            }
            let _ = i;
        }
        assert_eq!(got, want);
    }

    #[test]
    fn code_lengths() {
        assert_eq!(code_len(1), 0);
        assert_eq!(code_len(2), 1);
        assert_eq!(code_len(3), 2);
        assert_eq!(code_len(4), 2);
        assert_eq!(code_len(5), 3);
    }
}
