//! CABAC arithmetic decoding engine (9.3.1.2, 9.3.3.2).
//!
//! The 9-bit codIOffset is kept scaled in a 64-bit word together with up to ~40 bits of
//! look-ahead: `value = codIOffset << cnt | next cnt bits`. Renormalisation only lowers
//! `cnt`; comparisons scale the range instead of shifting the offset, and the stream is
//! refilled 32 bits at a time.

use crate::spec_tables::{CABAC_INIT, RANGE_LPS, TRANS_IDX};
use crate::spec_tables_444::CABAC_INIT_444;

/// Context variables: 0..460, and 460..1024 for 4:4:4 Cb/Cr residuals and 8x8
/// coded_block_flag.
pub const NUM_CTX: usize = 1024;

/// Context state transitions on the packed (pStateIdx << 1 | valMPS) byte.
struct Trans {
    mps: [u8; 128],
    lps: [u8; 128],
    /// rangeTabLPS by packed state and qCodIRangeIdx.
    range: [[u8; 4]; 128],
}

static TRANS: Trans = build_trans();

const fn build_trans() -> Trans {
    let mut t = Trans {
        mps: [0; 128],
        lps: [0; 128],
        range: [[0; 4]; 128],
    };
    let mut s = 0;
    while s < 128 {
        let p = s >> 1;
        let m = s & 1;
        t.mps[s] = ((TRANS_IDX[p].1 as usize) << 1 | m) as u8;
        let nm = if p == 0 { 1 - m } else { m };
        t.lps[s] = ((TRANS_IDX[p].0 as usize) << 1 | nm) as u8;
        t.range[s] = RANGE_LPS[p];
        s += 1;
    }
    t
}

pub struct Cabac<'a> {
    data: &'a [u8],
    /// Next byte to load.
    pos: usize,
    value: u64,
    cnt: u32,
    range: u32,
    pub ctx: [u8; NUM_CTX],
}

impl<'a> Cabac<'a> {
    /// Starts decoding at byte `pos` of `data` (which carries zero padding at its end).
    pub fn new(data: &'a [u8], pos: usize) -> Cabac<'a> {
        let mut c = Cabac {
            data,
            pos,
            value: 0,
            cnt: 0,
            range: 510,
            ctx: [0; NUM_CTX],
        };
        c.start(pos);
        c
    }

    /// (Re)initialises the engine at byte `pos` (slice start, after PCM samples).
    pub fn start(&mut self, pos: usize) {
        self.pos = pos;
        self.value = 0;
        self.cnt = 0;
        self.refill();
        // codIOffset = 9 bits.
        self.cnt -= 9;
        self.range = 510;
    }

    /// Initialises the context variables (9.3.1.1). `set`: 0 for I slices, 1 + cabac_init_idc
    /// otherwise. `all`: also ctxIdx 460..1024 (only 4:4:4 uses them).
    pub fn init_contexts(&mut self, set: usize, qp: i32, all: bool) {
        let qp = qp.clamp(0, 51);
        let n = if all { NUM_CTX } else { 460 };
        let init = CABAC_INIT[set].iter().chain(CABAC_INIT_444[set].iter());
        for (c, &(m, n)) in self.ctx[..n].iter_mut().zip(init) {
            let pre = (((m as i32 * qp) >> 4) + n as i32).clamp(1, 126);
            *c = if pre <= 63 {
                ((63 - pre) << 1) as u8
            } else {
                (((pre - 64) << 1) | 1) as u8
            };
        }
    }

    #[inline(always)]
    fn refill(&mut self) {
        let b = match self.data.get(self.pos..self.pos + 4) {
            Some(b) => u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
            None => {
                let mut b = [0u8; 4];
                if self.pos < self.data.len() {
                    let r = &self.data[self.pos..];
                    b[..r.len()].copy_from_slice(r);
                }
                b.into_iter().fold(0u32, |a, x| a << 8 | x as u32)
            }
        };
        self.pos += 4;
        self.value = (self.value << 32) | b as u64;
        self.cnt += 32;
    }

    /// Whether the decoder has consumed more than the payload plus its padding (corrupt data).
    pub fn overrun(&self) -> bool {
        self.pos > self.data.len() + 8
    }

    /// DecodeDecision for context `i`.
    #[inline(always)]
    pub fn decision(&mut self, i: usize) -> u32 {
        if self.cnt < 8 {
            self.refill();
        }
        let s = self.ctx[i] as usize;
        let lps = TRANS.range[s][((self.range >> 6) & 3) as usize] as u32;
        self.range -= lps;
        let scaled = (self.range as u64) << self.cnt;
        let bin;
        if self.value >= scaled {
            self.value -= scaled;
            self.range = lps;
            bin = (s & 1) as u32 ^ 1;
            self.ctx[i] = TRANS.lps[s];
            let shift = self.range.leading_zeros() - 23;
            self.range <<= shift;
            self.cnt -= shift;
        } else {
            bin = (s & 1) as u32;
            self.ctx[i] = TRANS.mps[s];
            if self.range < 256 {
                self.range <<= 1;
                self.cnt -= 1;
            }
        }
        bin
    }

    /// DecodeBypass.
    #[inline(always)]
    pub fn bypass(&mut self) -> u32 {
        if self.cnt < 8 {
            self.refill();
        }
        self.cnt -= 1;
        let scaled = (self.range as u64) << self.cnt;
        if self.value >= scaled {
            self.value -= scaled;
            1
        } else {
            0
        }
    }

    /// DecodeTerminate.
    #[inline(always)]
    pub fn terminate(&mut self) -> bool {
        if self.cnt < 8 {
            self.refill();
        }
        self.range -= 2;
        let scaled = (self.range as u64) << self.cnt;
        if self.value >= scaled {
            true
        } else {
            if self.range < 256 {
                self.range <<= 1;
                self.cnt -= 1;
            }
            false
        }
    }

    /// Byte position of the first byte after the arithmetic code, valid right after a
    /// terminate bin of 1 (for I_PCM samples).
    pub fn pcm_pos(&self) -> usize {
        // Bits consumed by the 9-bit register: everything loaded minus the look-ahead.
        let bits = self.pos * 8 - self.cnt as usize;
        bits.div_ceil(8)
    }

    /// The buffer being decoded.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// Unary bins with ctx `first` for bin 0 and `rest` for the others, up to `max`.
    pub fn unary(&mut self, first: usize, rest: usize, max: u32) -> u32 {
        if self.decision(first) == 0 {
            return 0;
        }
        let mut v = 1;
        while v < max && self.decision(rest) == 1 {
            v += 1;
        }
        v
    }

    /// k-th order Exp-Golomb bypass suffix (9.3.2.3).
    pub fn exp_golomb_bypass(&mut self, mut k: u32) -> u32 {
        let mut abs = 0u32;
        while self.bypass() == 1 {
            abs = abs.wrapping_add(1 << k);
            k += 1;
            if k >= 31 {
                return abs;
            }
        }
        while k > 0 {
            k -= 1;
            abs = abs.wrapping_add(self.bypass() << k);
        }
        abs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal CABAC encoder (9.3.4) to round-trip the decoder.
    struct Enc {
        low: u32,
        range: u32,
        outstanding: u32,
        first: bool,
        bits: Vec<bool>,
        ctx: [u8; NUM_CTX],
    }

    impl Enc {
        fn put(&mut self, b: bool) {
            if self.first {
                self.first = false;
            } else {
                self.bits.push(b);
            }
            while self.outstanding > 0 {
                self.bits.push(!b);
                self.outstanding -= 1;
            }
        }
        fn renorm(&mut self) {
            while self.range < 256 {
                if self.low < 256 {
                    self.put(false);
                } else if self.low >= 512 {
                    self.low -= 512;
                    self.put(true);
                } else {
                    self.low -= 256;
                    self.outstanding += 1;
                }
                self.range <<= 1;
                self.low <<= 1;
            }
        }
        fn decision(&mut self, i: usize, bin: u32) {
            let s = self.ctx[i] as usize;
            let lps = TRANS.range[s][((self.range >> 6) & 3) as usize] as u32;
            self.range -= lps;
            if bin != (s & 1) as u32 {
                self.low += self.range;
                self.range = lps;
                self.ctx[i] = TRANS.lps[s];
            } else {
                self.ctx[i] = TRANS.mps[s];
            }
            self.renorm();
        }
        fn bypass(&mut self, bin: u32) {
            self.low <<= 1;
            if bin == 1 {
                self.low += self.range;
            }
            if self.low >= 1024 {
                self.put(true);
                self.low -= 1024;
            } else if self.low < 512 {
                self.put(false);
            } else {
                self.low -= 512;
                self.outstanding += 1;
            }
        }
        fn terminate(&mut self, bin: bool) {
            self.range -= 2;
            if bin {
                self.low += self.range;
                self.range = 2;
                self.renorm();
                self.put((self.low >> 9) & 1 == 1);
                let v = ((self.low >> 7) & 3) | 1;
                self.bits.push(v & 2 != 0);
                self.bits.push(v & 1 != 0);
            } else {
                self.renorm();
            }
        }
    }

    #[test]
    fn round_trip() {
        let mut d = Cabac::new(&[0; 16], 0);
        d.init_contexts(1, 30, true);
        let mut e = Enc {
            low: 0,
            range: 510,
            outstanding: 0,
            first: true,
            bits: Vec::new(),
            ctx: d.ctx,
        };
        let mut seq = Vec::new();
        let mut x = 12345u32;
        for i in 0..5000 {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            let kind = (x >> 16) % 4;
            let ctx = ((x >> 8) % 40) as usize;
            let bin = if kind < 2 {
                ((x >> 20).is_multiple_of(7)) as u32
            } else {
                (x >> 21) & 1
            };
            seq.push((kind, ctx, bin));
            match kind {
                0 | 1 => e.decision(ctx, bin),
                2 => e.bypass(bin),
                _ => e.terminate(false),
            }
            if i == 4999 {
                e.terminate(true);
            }
        }
        let nbits = e.bits.len();
        // Stop bit already written by the flush; pad to bytes.
        let mut bytes = vec![0u8; nbits.div_ceil(8) + 16];
        for (i, &b) in e.bits.iter().enumerate() {
            if b {
                bytes[i / 8] |= 0x80 >> (i % 8);
            }
        }
        let mut d = Cabac::new(&bytes, 0);
        d.init_contexts(1, 30, true);
        for &(kind, ctx, bin) in &seq {
            let got = match kind {
                0 | 1 => d.decision(ctx),
                2 => d.bypass(),
                _ => d.terminate() as u32,
            };
            let want = if kind == 3 { 0 } else { bin };
            assert_eq!(got, want);
        }
        assert!(d.terminate());
        assert_eq!(d.pcm_pos(), nbits.div_ceil(8));
        assert_eq!(d.pos * 8 - d.cnt as usize, nbits);
    }
}
