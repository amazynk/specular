//! MQ arithmetic decoder (Annex C) and the raw (bypass) bit reader (D.6).

/// Combined state entry: index = state * 2 + mps.
#[derive(Clone, Copy)]
struct Entry {
    qe: u32,
    nmps: u8,
    nlps: u8,
}

const QE: [(u16, u8, u8, u8); 47] = [
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

/// Padded to 256 entries so indexing by a `u8` state needs no bounds check (states >= 94
/// are never produced).
const fn build() -> [Entry; 256] {
    let mut t = [Entry {
        qe: 0x5601,
        nmps: 0,
        nlps: 0,
    }; 256];
    let mut s = 0;
    while s < 47 {
        let (qe, nmps, nlps, sw) = QE[s];
        let mut mps = 0;
        while mps < 2 {
            t[s * 2 + mps] = Entry {
                qe: qe as u32,
                nmps: nmps * 2 + mps as u8,
                nlps: nlps * 2 + (mps as u8 ^ sw),
            };
            mps += 1;
        }
        s += 1;
    }
    t
}

static TABLE: [Entry; 256] = build();

#[cfg(test)]
pub(crate) const NUM_CTX: usize = 19;
pub(crate) const CTX_RL: usize = 17;
pub(crate) const CTX_UNI: usize = 18;

/// Initial context states (Table D.7): all 0 except UNIFORM (46), RL (3) and ZC 0 (4).
/// Context array size: a power of two >= `NUM_CTX`, so masked indexing needs no bounds check.
pub(crate) const CTX_CAP: usize = 32;

pub(crate) const INIT_CTX: [u8; CTX_CAP] = {
    let mut c = [0u8; CTX_CAP];
    c[0] = 4 * 2;
    c[CTX_RL] = 3 * 2;
    c[CTX_UNI] = 46 * 2;
    c
};

#[derive(Clone, Copy)]
pub(crate) struct Mq<'a> {
    data: &'a [u8],
    pos: usize,
    a: u32,
    c: u32,
    ct: u32,
    pub cx: [u8; CTX_CAP],
}

impl<'a> Mq<'a> {
    pub fn new() -> Mq<'a> {
        Mq {
            data: &[],
            pos: 0,
            a: 0x8000,
            c: 0,
            ct: 0,
            cx: INIT_CTX,
        }
    }

    #[inline(always)]
    fn byte(&self, i: usize) -> u32 {
        // Past the end the stream reads as 0xFF 0xFF..., i.e. a marker: C fills with 1s.
        *self.data.get(i).unwrap_or(&0xFF) as u32
    }

    /// INITDEC on a new segment; contexts are kept.
    pub fn init(&mut self, data: &'a [u8]) {
        self.data = data;
        self.pos = 0;
        self.c = self.byte(0) << 16;
        self.bytein();
        self.c <<= 7;
        self.ct = self.ct.wrapping_sub(7);
        self.a = 0x8000;
    }

    pub fn reset_contexts(&mut self) {
        self.cx = INIT_CTX;
    }

    #[inline(always)]
    fn bytein(&mut self) {
        if self.byte(self.pos) == 0xFF {
            let b1 = self.byte(self.pos + 1);
            if b1 > 0x8F {
                self.c = self.c.wrapping_add(0xFF00);
                self.ct = 8;
            } else {
                self.pos += 1;
                self.c = self.c.wrapping_add(b1 << 9);
                self.ct = 7;
            }
        } else {
            self.pos += 1;
            self.c = self.c.wrapping_add(self.byte(self.pos) << 8);
            self.ct = 8;
        }
    }

    #[inline(always)]
    fn renorm(&mut self) {
        // `a` is never 0 here; shift in one step when no byte has to be read meanwhile.
        let n = self.a.leading_zeros().wrapping_sub(16);
        if n <= self.ct {
            self.a <<= n;
            self.c <<= n;
            self.ct -= n;
            return;
        }
        loop {
            if self.ct == 0 {
                self.bytein();
            }
            self.a <<= 1;
            self.c <<= 1;
            self.ct -= 1;
            if self.a & 0x8000 != 0 {
                break;
            }
        }
    }

    #[inline(always)]
    pub fn decode(&mut self, cx: usize) -> u32 {
        let cx = cx & (CTX_CAP - 1);
        let s = self.cx[cx];
        let e = TABLE[s as usize];
        let mps = (s & 1) as u32;
        self.a -= e.qe;
        if (self.c >> 16) < e.qe {
            let d;
            if self.a < e.qe {
                d = mps;
                self.cx[cx] = e.nmps;
            } else {
                d = mps ^ 1;
                self.cx[cx] = e.nlps;
            }
            self.a = e.qe;
            self.renorm();
            d
        } else {
            self.c -= e.qe << 16;
            if self.a & 0x8000 == 0 {
                let d;
                if self.a < e.qe {
                    d = mps ^ 1;
                    self.cx[cx] = e.nlps;
                } else {
                    d = mps;
                    self.cx[cx] = e.nmps;
                }
                self.renorm();
                d
            } else {
                mps
            }
        }
    }
}

/// Raw (bypass) segment reader.
#[derive(Clone, Copy)]
pub(crate) struct Raw<'a> {
    data: &'a [u8],
    pos: usize,
    c: u32,
    ct: u32,
}

impl<'a> Raw<'a> {
    pub fn new(data: &'a [u8]) -> Raw<'a> {
        Raw {
            data,
            pos: 0,
            c: 0,
            ct: 0,
        }
    }

    #[inline(always)]
    pub fn bit(&mut self) -> u32 {
        if self.ct == 0 {
            let b = *self.data.get(self.pos).unwrap_or(&0xFF) as u32;
            if self.c == 0xFF {
                if b > 0x8F {
                    self.c = 0xFF;
                    self.ct = 8;
                } else {
                    self.c = b;
                    self.pos += 1;
                    self.ct = 7;
                }
            } else {
                self.c = b;
                self.pos += 1;
                self.ct = 8;
            }
        }
        self.ct -= 1;
        (self.c >> self.ct) & 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MQ encoder (Annex C, software conventions) used to round-trip test the decoder.
    struct Enc {
        a: u32,
        c: u32,
        ct: u32,
        out: Vec<u8>,
        b: i32,
        cx: [u8; CTX_CAP],
    }
    impl Enc {
        fn new() -> Enc {
            Enc {
                a: 0x8000,
                c: 0,
                ct: 12,
                out: Vec::new(),
                b: -1,
                cx: INIT_CTX,
            }
        }
        fn byteout(&mut self) {
            if self.b == 0xFF {
                self.out.push(0xFF);
                self.b = (self.c >> 20) as i32;
                self.c &= 0xFFFFF;
                self.ct = 7;
            } else if self.c < 0x8000000 {
                if self.b >= 0 {
                    self.out.push(self.b as u8);
                }
                self.b = (self.c >> 19) as i32;
                self.c &= 0x7FFFF;
                self.ct = 8;
            } else {
                self.b += 1;
                if self.b == 0xFF {
                    self.c &= 0x7FFFFFF;
                    self.out.push(0xFF);
                    self.b = (self.c >> 20) as i32;
                    self.c &= 0xFFFFF;
                    self.ct = 7;
                } else {
                    if self.b >= 0 {
                        self.out.push(self.b as u8);
                    }
                    self.b = (self.c >> 19) as i32;
                    self.c &= 0x7FFFF;
                    self.ct = 8;
                }
            }
        }
        fn renorm(&mut self) {
            loop {
                self.a <<= 1;
                self.c <<= 1;
                self.ct -= 1;
                if self.ct == 0 {
                    self.byteout();
                }
                if self.a & 0x8000 != 0 {
                    break;
                }
            }
        }
        fn encode(&mut self, cx: usize, d: u32) {
            let s = self.cx[cx];
            let e = TABLE[s as usize];
            let mps = (s & 1) as u32;
            if d == mps {
                self.a -= e.qe;
                if self.a & 0x8000 == 0 {
                    if self.a < e.qe {
                        self.a = e.qe;
                    } else {
                        self.c += e.qe;
                    }
                    self.cx[cx] = e.nmps;
                    self.renorm();
                } else {
                    self.c += e.qe;
                }
            } else {
                self.a -= e.qe;
                if self.a < e.qe {
                    self.c += e.qe;
                } else {
                    self.a = e.qe;
                }
                self.cx[cx] = e.nlps;
                self.renorm();
            }
        }
        fn flush(mut self) -> Vec<u8> {
            let t = self.c + self.a;
            self.c |= 0xFFFF;
            if self.c >= t {
                self.c -= 0x8000;
            }
            self.c <<= self.ct;
            self.byteout();
            self.c <<= self.ct;
            self.byteout();
            if self.b != 0xFF && self.b >= 0 {
                self.out.push(self.b as u8);
            }
            self.out
        }
    }

    #[test]
    fn roundtrip() {
        let mut seed = 12345u32;
        let mut syms = Vec::new();
        for _ in 0..20000 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let cx = (seed >> 16) as usize % NUM_CTX;
            // Skewed distribution per context.
            let bias = (cx as u32 * 13) % 100;
            let d = (((seed >> 8) % 100) < bias) as u32;
            syms.push((cx, d));
        }
        let mut e = Enc::new();
        for &(cx, d) in &syms {
            e.encode(cx, d);
        }
        let bytes = e.flush();
        let mut m = Mq::new();
        m.init(&bytes);
        for (i, &(cx, d)) in syms.iter().enumerate() {
            assert_eq!(m.decode(cx), d, "symbol {i}");
        }
    }
}
