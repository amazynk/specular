//! Monkey's Audio's frame bit reader and range decoder. The frame's bytes are read as
//! little-endian 32-bit words, each from its top bit down; the range decoder takes whole
//! bytes from that stream. Residuals are an overflow count from a fixed 64-symbol model and a
//! base uniform below an adaptive pivot (files from 3990), or a base of `k` bits
//! (3891–3989).

use crate::tables::{TOTAL_1, TOTAL_2, WIDTH_1, WIDTH_2};

const BOTTOM: u32 = 1 << 23;
const EXTRA_BITS: u32 = 7;

/// Lower bounds of the running sum for each `k`.
const K_MIN: [u32; 32] = {
    let mut t = [0u32; 32];
    let mut i = 1;
    while i < 28 {
        t[i] = 1 << (i + 4);
        i += 1;
    }
    t
};

pub(crate) struct Bad;

pub(crate) struct Reader<'a> {
    d: &'a [u8],
    bit: u64,
    low: u32,
    range: u32,
    buffer: u32,
    new: bool,
    version: u16,
    /// Bytes read past the end of the frame's data.
    pub overrun: bool,
}

/// One channel's adaptive state.
#[derive(Clone, Copy)]
pub(crate) struct State {
    k: u32,
    ksum: u32,
}

impl Default for State {
    fn default() -> State {
        State {
            k: 10,
            ksum: (1 << 10) * 16,
        }
    }
}

impl<'a> Reader<'a> {
    /// A reader of `d` (32-bit words from its start), the first `skip_bits` bits passed.
    pub fn new(d: &'a [u8], skip_bits: usize, version: u16) -> Reader<'a> {
        Reader {
            d,
            bit: skip_bits as u64,
            low: 0,
            range: 0,
            buffer: 0,
            new: version >= 3990,
            version,
            overrun: false,
        }
    }

    fn bit_at(&mut self, p: u64) -> u32 {
        let word = (p / 32) as usize;
        let within = (p % 32) as usize;
        let byte = word * 4 + 3 - within / 8;
        match self.d.get(byte) {
            Some(&b) => u32::from(b >> (7 - within % 8)) & 1,
            None => {
                self.overrun = true;
                0
            }
        }
    }

    /// `n` (≤ 32) bits, the first the highest.
    pub fn bits(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.bit_at(self.bit);
            self.bit += 1;
        }
        v
    }

    fn byte(&mut self) -> u32 {
        if self.bit.is_multiple_of(8) {
            let word = (self.bit / 32) as usize;
            let byte = word * 4 + 3 - ((self.bit % 32) / 8) as usize;
            self.bit += 8;
            match self.d.get(byte) {
                Some(&b) => u32::from(b),
                None => {
                    self.overrun = true;
                    0
                }
            }
        } else {
            self.bits(8)
        }
    }

    /// The zeros before the next one bit (the unary part of a Rice code), the one taken.
    pub fn unary(&mut self) -> Result<u32, Bad> {
        let mut n = 0u32;
        while self.bit_at(self.bit) == 0 {
            self.bit += 1;
            n += 1;
            if self.overrun || n > 1 << 24 {
                return Err(Bad);
            }
        }
        self.bit += 1;
        Ok(n)
    }

    /// Ends a range-coded run (files 3891–3949 code each channel of a frame on its own):
    /// the bytes the decoder had read ahead are given back, as the encoder wrote them.
    pub fn finish_range(&mut self) {
        while self.range <= BOTTOM {
            self.bit += 8;
            self.range <<= 8;
            if self.range == 0 {
                return;
            }
        }
        if self.version <= 3950 {
            self.bit = self.bit.saturating_sub(16);
        }
    }

    /// Starts the range decoder (after the frame's checksum words).
    pub fn start_range(&mut self) {
        self.bit = self.bit.div_ceil(8) * 8;
        let _ = self.byte();
        self.buffer = self.byte();
        self.low = self.buffer >> (8 - EXTRA_BITS);
        self.range = 1 << EXTRA_BITS;
    }

    fn normalize(&mut self) -> Result<(), Bad> {
        while self.range <= BOTTOM {
            if self.range == 0 {
                return Err(Bad);
            }
            self.buffer = (self.buffer << 8) | self.byte();
            self.low = (self.low << 8) | ((self.buffer >> 1) & 0xff);
            self.range <<= 8;
        }
        Ok(())
    }

    fn peek(&mut self, shift: u32) -> Result<u32, Bad> {
        self.normalize()?;
        self.range >>= shift;
        if self.range == 0 {
            return Err(Bad);
        }
        Ok(self.low / self.range)
    }

    fn take(&mut self, shift: u32) -> Result<u32, Bad> {
        let v = self.peek(shift)?;
        self.low %= self.range;
        Ok(v)
    }

    fn take_div(&mut self, div: u32) -> Result<u32, Bad> {
        self.normalize()?;
        self.range /= div;
        if self.range == 0 {
            return Err(Bad);
        }
        let v = self.low / self.range;
        self.low %= self.range;
        Ok(v)
    }

    fn model(&mut self, width: &[u32; 64], total: &[u32; 65]) -> Result<u32, Bad> {
        let t = self.peek(16)?;
        if t >= 65536 {
            return Err(Bad);
        }
        let ov = total.partition_point(|&x| x <= t) - 1;
        self.low = self.low.wrapping_sub(self.range.wrapping_mul(total[ov]));
        self.range = self.range.wrapping_mul(width[ov]);
        Ok(ov as u32)
    }

    fn overflow(&mut self, pivot: &mut u32) -> Result<u32, Bad> {
        // The escape for large overflows may itself signal a smaller pivot (32-bit audio);
        // each signal costs 32 bits, so the depth stays small on real data.
        for _ in 0..64 {
            let mut ov = self.model(&WIDTH_2, &TOTAL_2)?;
            if ov == 63 {
                ov = self.take(16)? << 16;
                ov |= self.take(16)?;
                if ov == 1 {
                    *pivot = 32768;
                    continue;
                }
            }
            return Ok(ov);
        }
        Err(Bad)
    }

    /// The next residual of a channel.
    pub fn value(&mut self, s: &mut State) -> Result<i64, Bad> {
        let v: i64 = if self.new {
            let mut pivot = (s.ksum / 32).max(1);
            let ov = self.overflow(&mut pivot)?;
            let base = if pivot >= 1 << 16 {
                let nbits = 32 - pivot.leading_zeros();
                let shift = nbits.saturating_sub(16);
                let split = 1u32 << shift;
                let a = self.take_div((pivot / split) + 1)?;
                let b = self.take_div(split)?;
                a.wrapping_mul(split).wrapping_add(b)
            } else {
                self.take_div(pivot)?
            };
            i64::from(base) + i64::from(ov) * i64::from(pivot)
        } else {
            let mut ov = self.model(&WIDTH_1, &TOTAL_1)?;
            let k = if ov == 63 {
                ov = 0;
                self.take(5)?
            } else {
                s.k.saturating_sub(1)
            };
            let mut v = if k <= 16 || self.version < 3910 {
                i64::from(self.take(k)?)
            } else {
                let x1 = self.take(16)?;
                let x2 = self.take(k - 16)?;
                i64::from(x1) | i64::from(x2) << 16
            };
            v += i64::from(ov) << k.min(32);
            v
        };
        s.ksum = s
            .ksum
            .wrapping_add(((v + 1) / 2) as u32)
            .wrapping_sub((s.ksum.wrapping_add(16)) >> 5);
        if s.ksum < K_MIN[s.k as usize] {
            s.k = s.k.saturating_sub(1);
        } else if (s.k as usize + 1) < 32
            && K_MIN[s.k as usize + 1] != 0
            && s.ksum >= K_MIN[s.k as usize + 1]
        {
            s.k += 1;
        }
        Ok(if v & 1 != 0 { (v >> 1) + 1 } else { -(v >> 1) })
    }
}
