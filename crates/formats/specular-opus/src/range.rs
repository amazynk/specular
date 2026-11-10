//! The range decoder (RFC 6716 section 4.1), with the raw bits read from the end of the frame.

/// Range decoder state over one Opus frame.
pub(crate) struct RangeDecoder<'a> {
    buf: &'a [u8],
    /// Bytes of `buf` the decoder may use (shrinks when a redundant CELT frame takes the tail).
    storage: usize,
    /// Next byte read by the range coder.
    offs: usize,
    /// Bytes consumed from the end by raw bits.
    end_offs: usize,
    end_window: u32,
    nend_bits: u32,
    /// Whole bits processed so far (RFC 4.1.6), including buffered ones.
    nbits_total: i32,
    rng: u32,
    val: u32,
    rem: u32,
    /// Set when a decoded integer was out of range (a damaged frame).
    pub(crate) error: bool,
}

const CODE_TOP: u32 = 1 << 31;
const CODE_BOT: u32 = 1 << 23;
const CODE_EXTRA: u32 = 7;
const SYM_MAX: u32 = 255;

/// Number of bits needed to store `x` (0 for 0).
#[inline]
pub(crate) fn ilog(x: u32) -> i32 {
    32 - x.leading_zeros() as i32
}

impl<'a> RangeDecoder<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> RangeDecoder<'a> {
        let mut d = RangeDecoder {
            buf,
            storage: buf.len(),
            offs: 0,
            end_offs: 0,
            end_window: 0,
            nend_bits: 0,
            nbits_total: 9,
            rng: 128,
            val: 0,
            rem: 0,
            error: false,
        };
        d.rem = d.read_byte();
        d.val = d.rng - 1 - (d.rem >> 1);
        d.normalize();
        d
    }

    #[inline]
    fn read_byte(&mut self) -> u32 {
        if self.offs < self.storage {
            let b = self.buf[self.offs];
            self.offs += 1;
            u32::from(b)
        } else {
            0
        }
    }

    #[inline]
    fn read_byte_from_end(&mut self) -> u32 {
        if self.end_offs < self.storage {
            self.end_offs += 1;
            u32::from(self.buf[self.storage - self.end_offs])
        } else {
            0
        }
    }

    #[inline]
    fn normalize(&mut self) {
        while self.rng <= CODE_BOT {
            self.nbits_total += 8;
            self.rng <<= 8;
            let sym = self.rem;
            self.rem = self.read_byte();
            let sym = ((sym << 8) | self.rem) >> (8 - CODE_EXTRA);
            self.val = ((self.val << 8).wrapping_add(SYM_MAX & !sym)) & (CODE_TOP - 1);
        }
    }

    /// The frequency `fs` of the next symbol for a total `ft` (ec_decode).
    #[inline]
    pub(crate) fn decode(&mut self, ft: u32) -> (u32, u32) {
        let ext = self.rng / ft;
        let s = self.val / ext;
        (ft - (s + 1).min(ft), ext)
    }

    /// Like [`decode`](Self::decode) with `ft = 1 << bits`.
    #[inline]
    pub(crate) fn decode_bin(&mut self, bits: u32) -> (u32, u32) {
        let ext = self.rng >> bits;
        let s = self.val / ext;
        ((1u32 << bits) - (s + 1).min(1u32 << bits), ext)
    }

    /// Consumes the symbol `[fl, fh)` of total `ft` (ec_dec_update); `ext` from `decode`.
    #[inline]
    pub(crate) fn update(&mut self, ext: u32, fl: u32, fh: u32, ft: u32) {
        let s = ext.wrapping_mul(ft - fh);
        self.val = self.val.wrapping_sub(s);
        self.rng = if fl > 0 {
            ext.wrapping_mul(fh - fl)
        } else {
            self.rng.wrapping_sub(s)
        };
        self.normalize();
    }

    /// One binary symbol where a 1 has probability 1/2^logp.
    #[inline]
    pub(crate) fn bit_logp(&mut self, logp: u32) -> bool {
        let r = self.rng;
        let d = self.val;
        let s = r >> logp;
        let ret = d < s;
        if !ret {
            self.val = d - s;
        }
        self.rng = if ret { s } else { r - s };
        self.normalize();
        ret
    }

    /// One symbol from an inverse-CDF table with total `1 << ftb`.
    #[inline]
    pub(crate) fn icdf(&mut self, icdf: &[u8], ftb: u32) -> usize {
        let mut s = self.rng;
        let d = self.val;
        let r = s >> ftb;
        let mut ret = 0usize;
        let mut t;
        loop {
            t = s;
            // A table always ends with 0, so `ret` stays in bounds for valid tables.
            s = r.wrapping_mul(u32::from(icdf.get(ret).copied().unwrap_or(0)));
            if d >= s {
                break;
            }
            ret += 1;
        }
        self.val = d - s;
        self.rng = t - s;
        self.normalize();
        ret
    }

    /// `bits` raw bits from the end of the frame (at most 25).
    #[inline]
    pub(crate) fn bits(&mut self, bits: u32) -> u32 {
        if bits == 0 {
            return 0;
        }
        let mut window = self.end_window;
        let mut available = self.nend_bits;
        if available < bits {
            loop {
                window |= self.read_byte_from_end() << available;
                available += 8;
                if available > 24 {
                    break;
                }
            }
        }
        let ret = window & ((1u32 << bits) - 1);
        self.end_window = window >> bits;
        self.nend_bits = available - bits;
        self.nbits_total += bits as i32;
        ret
    }

    /// A uniform integer in `0..ft` (ec_dec_uint); `ft >= 2`.
    pub(crate) fn uint(&mut self, ft: u32) -> u32 {
        let ft1 = ft - 1;
        let ftb = ilog(ft1);
        if ftb > 8 {
            let sh = (ftb - 8) as u32;
            let top = (ft1 >> sh) + 1;
            let (s, ext) = self.decode(top);
            self.update(ext, s, s + 1, top);
            let t = (s << sh) | self.bits(sh);
            if t <= ft1 {
                t
            } else {
                self.error = true;
                ft1
            }
        } else {
            let (s, ext) = self.decode(ft);
            self.update(ext, s, s + 1, ft);
            s
        }
    }

    /// Bits used so far, rounded up (ec_tell).
    #[inline]
    pub(crate) fn tell(&self) -> i32 {
        self.nbits_total - ilog(self.rng)
    }

    /// Bits used so far in 1/8 bit units (ec_tell_frac).
    pub(crate) fn tell_frac(&self) -> i32 {
        let nbits = self.nbits_total << 3;
        let mut l = ilog(self.rng);
        let mut r = self.rng >> (l - 16);
        for _ in 0..3 {
            r = (r * r) >> 15;
            let b = (r >> 16) as i32;
            l = 2 * l + b;
            r >>= b;
        }
        nbits - l
    }

    /// The range after the last symbol (the "final range" of conformance checks).
    #[inline]
    pub(crate) fn rng(&self) -> u32 {
        self.rng
    }

    /// Size of the frame in bytes as the decoder sees it.
    #[inline]
    pub(crate) fn storage(&self) -> usize {
        self.storage
    }

    /// Takes the last `n` bytes away from this decoder (they hold a redundant CELT frame).
    pub(crate) fn shrink(&mut self, n: usize) {
        self.storage = self.storage.saturating_sub(n);
    }

    /// Pretends every remaining bit was read (a silence frame).
    pub(crate) fn skip_to_end(&mut self) {
        let total = (self.storage * 8) as i32;
        self.nbits_total += total - self.tell();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tell_starts_at_one_bit() {
        let d = RangeDecoder::new(&[0x55, 0xaa, 0x01]);
        assert_eq!(d.tell(), 1);
        assert_eq!(d.tell_frac(), 8);
    }

    #[test]
    fn empty_frame_decodes_zeros() {
        let mut d = RangeDecoder::new(&[]);
        for _ in 0..20 {
            let _ = d.bit_logp(1);
        }
        assert_eq!(d.bits(8), 0);
    }
}
