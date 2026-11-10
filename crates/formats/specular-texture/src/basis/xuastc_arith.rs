//! The range coder of XUASTC LDR's arithmetic profiles (Said-style, 32-bit), written from
//! Binomial's published "XUASTC LDR - Arithmetic/Range Coding": adaptive binary and
//! multi-symbol models, plain bits, truncated binary and gamma codes. Past the end of the data
//! the decoder reads zeros (as the reference does); callers bound every loop and check what
//! they decode.

use crate::{Error, Result};

const DM_LEN_SHIFT: u32 = 15;
const DM_MAX_COUNT: u32 = 1 << DM_LEN_SHIFT;
const BM_LEN_SHIFT: u32 = 13;
const BM_MAX_COUNT: u32 = 1 << BM_LEN_SHIFT;
const MIN_LEN: u32 = 1 << 24;
const GAMMA_PREFIX: usize = 3;
const GAMMA_TAIL: usize = 4;

/// Adaptive bit model.
#[derive(Clone, Copy)]
pub(super) struct BitModel {
    bit0: u32,
    count: u32,
    prob: u32,
    until: i32,
    interval: u32,
}

impl Default for BitModel {
    fn default() -> Self {
        BitModel {
            bit0: 1,
            count: 2,
            prob: 1 << (BM_LEN_SHIFT - 1),
            until: 4,
            interval: 4,
        }
    }
}

impl BitModel {
    fn update(&mut self) {
        if self.count >= BM_MAX_COUNT {
            self.count = (self.count + 1) >> 1;
            self.bit0 = (self.bit0 + 1) >> 1;
            if self.bit0 == self.count {
                self.count += 1;
            }
        }
        let scale = 0x8000_0000u32 / self.count;
        self.prob = self.bit0.wrapping_mul(scale) >> (31 - BM_LEN_SHIFT);
        self.interval = ((5 * self.interval) >> 2).clamp(4, 128);
        self.until = self.interval as i32;
    }
}

/// Adaptive multi-symbol model.
#[derive(Clone, Default)]
pub(super) struct DataModel {
    n: u32,
    total: u32,
    freqs: Vec<u32>,
    cum: Vec<u32>,
    interval: u32,
    until: i32,
}

impl DataModel {
    pub(super) fn new(n: u32, faster: bool) -> Self {
        let mut m = DataModel {
            n,
            total: n,
            freqs: vec![1; n as usize],
            cum: vec![0; n as usize + 1],
            interval: n,
            until: 0,
        };
        if n > 0 {
            m.update();
            if faster {
                m.interval = n.div_ceil(8).max(4).min((n + 6) << 3);
            }
            m.until = m.interval as i32;
        }
        m
    }

    pub(super) fn symbols(&self) -> u32 {
        self.n
    }

    fn update(&mut self) {
        while self.total >= DM_MAX_COUNT {
            self.total = 0;
            for f in &mut self.freqs {
                *f = (*f + 1) >> 1;
                self.total += *f;
            }
        }
        let scale = 0x8000_0000u32 / self.total;
        let mut sum = 0u32;
        for (c, &f) in self.cum.iter_mut().zip(&self.freqs) {
            *c = scale.wrapping_mul(sum) >> (31 - DM_LEN_SHIFT);
            sum += f;
        }
        self.cum[self.n as usize] = DM_MAX_COUNT;
        self.interval = ((5 * self.interval) >> 2).clamp(4, (self.n + 6) << 3);
        self.until = self.interval as i32;
    }
}

/// Gamma code contexts.
#[derive(Clone, Copy, Default)]
pub(super) struct Gamma {
    prefix: [BitModel; GAMMA_PREFIX],
    tail: [BitModel; GAMMA_TAIL],
}

pub(super) struct Arith<'a> {
    d: &'a [u8],
    pos: usize,
    value: u32,
    length: u32,
}

impl<'a> Arith<'a> {
    pub(super) fn new(d: &'a [u8]) -> Result<Self> {
        if d.len() < 5 {
            return Err(Error::Truncated("XUASTC LDR arithmetic data"));
        }
        Ok(Arith {
            d,
            pos: 4,
            value: u32::from_be_bytes([d[0], d[1], d[2], d[3]]),
            length: u32::MAX,
        })
    }

    /// Bytes read past the end (zeros supplied); a stream that needs many is damaged.
    pub(super) fn overrun(&self) -> usize {
        self.pos.saturating_sub(self.d.len())
    }

    fn renorm(&mut self) {
        while self.length < MIN_LEN {
            let b = self.d.get(self.pos).copied().unwrap_or(0);
            self.pos += 1;
            self.value = (self.value << 8) | u32::from(b);
            self.length <<= 8;
        }
    }

    pub(super) fn get_bit(&mut self) -> u32 {
        self.length >>= 1;
        let bit = u32::from(self.value >= self.length);
        if bit == 1 {
            self.value -= self.length;
        }
        if self.length < MIN_LEN {
            self.renorm();
        }
        bit
    }

    /// 1…20 plain bits.
    pub(super) fn get_bits(&mut self, n: u32) -> u32 {
        debug_assert!((1..=20).contains(&n));
        self.length >>= n;
        let v = self.value / self.length;
        self.value -= self.length * v;
        if self.length < MIN_LEN {
            self.renorm();
        }
        v.min((1 << n) - 1)
    }

    pub(super) fn truncated_binary(&mut self, n: u32) -> u32 {
        let k = 31 - n.leading_zeros();
        let u = (1 << (k + 1)) - n;
        let mut r = if k > 0 { self.get_bits(k) } else { 0 };
        if r >= u {
            r = ((r << 1) | self.get_bits(1)) - u;
        }
        r
    }

    pub(super) fn bit(&mut self, m: &mut BitModel) -> bool {
        let x = m.prob * (self.length >> BM_LEN_SHIFT);
        let bit = self.value >= x;
        if !bit {
            self.length = x;
            m.bit0 += 1;
        } else {
            self.value -= x;
            self.length -= x;
        }
        m.count += 1;
        if self.length < MIN_LEN {
            self.renorm();
        }
        m.until -= 1;
        if m.until <= 0 {
            m.update();
        }
        bit
    }

    /// Gamma code (≥ 1), or 0 for a damaged prefix.
    pub(super) fn gamma(&mut self, g: &mut Gamma) -> u32 {
        let mut k = 0usize;
        while self.bit(&mut g.prefix[k.min(GAMMA_PREFIX - 1)]) {
            k += 1;
            if k > 16 {
                return 0;
            }
        }
        let mut n = 1u32 << k;
        for i in (0..k).rev() {
            if self.bit(&mut g.tail[i.min(GAMMA_TAIL - 1)]) {
                n |= 1 << i;
            }
        }
        n
    }

    pub(super) fn sym(&mut self, m: &mut DataModel) -> u32 {
        let (mut x, mut y) = (0u32, self.length);
        self.length >>= DM_LEN_SHIFT;
        let (mut lo, mut hi) = (0u32, m.n);
        let mut mid = hi >> 1;
        loop {
            let z = self.length.wrapping_mul(m.cum[mid as usize]);
            if z > self.value {
                hi = mid;
                y = z;
            } else {
                lo = mid;
                x = z;
            }
            mid = (lo + hi) >> 1;
            if mid == lo {
                break;
            }
        }
        self.value -= x;
        self.length = y - x;
        if self.length < MIN_LEN {
            self.renorm();
        }
        m.freqs[lo as usize] += 1;
        m.total += 1;
        m.until -= 1;
        if m.until <= 0 {
            m.update();
        }
        lo
    }
}
