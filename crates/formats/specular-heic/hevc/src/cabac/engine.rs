//! The CABAC arithmetic decoding engine (clause 9.3.4.3).
//!
//! `ivlOffset` is kept scaled by `1 << 7` in `value`, with up to seven not yet
//! consumed stream bits buffered beneath it, so that the engine fetches a
//! byte at a time and renormalises with a single shift instead of a bit loop.
//! Every comparison the specification makes on `ivlOffset` is made here on
//! `value` against `ivlCurrRange << 7`, which is the same comparison because
//! the buffered bits are always below the scaled range's resolution.

use super::ctx::{INIT_VALUES, INIT_VALUES_1, INIT_VALUES_2, NUM_CTX};
use super::tables::{RANGE_TAB_LPS, TRANS_IDX_LPS, TRANS_IDX_MPS};
use crate::error::{Error, Result};

/// The packed context after a bin, indexed by `is_lps << 7 | pStateIdx << 1 | valMps`.
static NEXT_STATE: [u8; 256] = next_states();

const fn next_states() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut s = 0;
    while s < 128 {
        let state = s >> 1;
        let mps = (s & 1) as u8;
        t[s] = (TRANS_IDX_MPS[state] << 1) | mps;
        let flip = if state == 0 { 1 } else { 0 };
        t[128 + s] = (TRANS_IDX_LPS[state] << 1) | (mps ^ flip);
        s += 1;
    }
    t
}

/// Number of zero bytes the engine may synthesise past the end of a substream.
///
/// The nine-bit prefetch of `initialization` plus renormalisation legitimately
/// reads a little past the last meaningful byte of a slice segment.
const OVERREAD_SLACK: usize = 16;

/// Arithmetic decoder plus the flat array of context variables.
///
/// Context variables are packed as `pStateIdx << 1 | valMps`.
#[derive(Clone)]
pub struct Cabac<'a> {
    data: &'a [u8],
    /// Index of the next byte the arithmetic decoder will fetch.
    next: usize,
    /// Bit cursor for raw (PCM) reads, counted from the first bit of `data`.
    pos: usize,
    range: u32,
    /// `ivlOffset << 7`, plus the buffered bits below bit 7.
    value: u32,
    /// Minus one minus the number of buffered bits; a refill is due at zero.
    bits_needed: i32,
    /// `pStateIdx << 1 | valMps` for every context variable.
    pub ctx: [u8; NUM_CTX],
}

/// Derives every context variable from `SliceQpY` (clause 9.3.2.2).
///
/// Each entry is packed as `pStateIdx << 1 | valMps`.
pub fn initial_contexts(qp: i32) -> [u8; NUM_CTX] {
    init_from(&INIT_VALUES, qp)
}

/// [`initial_contexts`] for `initType` 0, 1 or 2 (clause 9.3.2.2).
pub fn initial_contexts_for(init_type: usize, qp: i32) -> [u8; NUM_CTX] {
    match init_type {
        0 => init_from(&INIT_VALUES, qp),
        1 => init_from(&INIT_VALUES_1, qp),
        _ => init_from(&INIT_VALUES_2, qp),
    }
}

fn init_from(table: &[u8; NUM_CTX], qp: i32) -> [u8; NUM_CTX] {
    let q = qp.clamp(0, 51);
    let mut out = [0u8; NUM_CTX];
    for (o, &init) in out.iter_mut().zip(table.iter()) {
        let slope = (init >> 4) as i32 * 5 - 45;
        let offset = ((init & 15) as i32) * 8 - 16;
        let pre = (((slope * q) >> 4) + offset).clamp(1, 126);
        let mps = u8::from(pre > 63);
        let state = if mps == 1 { pre - 64 } else { 63 - pre };
        *o = ((state as u8) << 1) | mps;
    }
    out
}

impl<'a> Cabac<'a> {
    /// Creates an engine over `data` starting at byte `byte_off`.
    ///
    /// `qp` is `SliceQpY`, used to initialise the context variables.
    pub fn new(data: &'a [u8], byte_off: usize, qp: i32) -> Result<Cabac<'a>> {
        let mut c = Cabac {
            data,
            next: byte_off,
            pos: byte_off * 8,
            range: 510,
            value: 0,
            bits_needed: -8,
            ctx: [0; NUM_CTX],
        };
        c.init_contexts(qp);
        c.init_engine()?;
        Ok(c)
    }

    /// Resets the context variables from the initialisation table (9.3.2.2).
    pub fn init_contexts(&mut self, qp: i32) {
        self.ctx = initial_contexts(qp);
    }

    /// Restarts the arithmetic decoder at the current byte position (9.3.2.5).
    pub fn init_engine(&mut self) -> Result<()> {
        self.pos = (self.pos + 7) & !7;
        self.next = self.pos >> 3;
        self.range = 510;
        let hi = self.fetch();
        let lo = self.fetch();
        self.value = (hi << 8) | lo;
        self.bits_needed = -8;
        if self.value >> 7 >= 510 {
            return Err(Error::InvalidData("CABAC ivlOffset is 510 or 511"));
        }
        self.check_overread()
    }

    /// Next byte boundary at or after the current position.
    ///
    /// `DecodeTerminate` consumes no bit when it returns 1 and the last bit it
    /// shifted in is the stop bit, so this is where the next substream starts.
    pub fn aligned_pos(&self) -> usize {
        (self.bit_pos() + 7) >> 3
    }

    /// The bit position the specification's nine-bit `ivlOffset` has reached.
    #[inline]
    fn bit_pos(&self) -> usize {
        // `-bits_needed - 1` bits are fetched but not yet part of ivlOffset.
        self.next * 8 - (-self.bits_needed - 1) as usize
    }

    /// Fails once the engine has run more than the permitted slack past the end.
    #[inline]
    fn check_overread(&self) -> Result<()> {
        if self.next > self.data.len() + OVERREAD_SLACK {
            return Err(Error::Truncated);
        }
        Ok(())
    }

    /// Next stream byte, or zero past the end; `check_overread` bounds the latter.
    #[inline(always)]
    fn fetch(&mut self) -> u32 {
        let b = self.data.get(self.next).copied().unwrap_or(0);
        self.next += 1;
        u32::from(b)
    }

    /// Shifts one bit into `value`, refilling when the buffer runs dry.
    #[inline(always)]
    fn shift_one(&mut self) {
        self.value <<= 1;
        self.bits_needed += 1;
        if self.bits_needed == 0 {
            self.bits_needed = -8;
            self.value |= self.fetch();
        }
    }

    /// `DecodeDecision`: decodes one bin with context variable `ctx_idx`.
    ///
    /// Branch-free apart from the byte refill: whether a bin is the MPS is
    /// close to a coin toss for the residual flags, so the MPS/LPS choice is
    /// made with masks, the next state comes from one table indexed by the
    /// packed state and the outcome, and both paths share one renormalisation.
    #[inline(always)]
    pub fn decision(&mut self, ctx_idx: usize) -> u32 {
        let s = usize::from(self.ctx[ctx_idx] & 127);
        let lps = u32::from(RANGE_TAB_LPS[s >> 1][((self.range >> 6) & 3) as usize]);
        let range = self.range - lps;
        let scaled = range << 7;
        let is_lps = u32::from(self.value >= scaled);
        let mask = is_lps.wrapping_neg();
        self.value -= scaled & mask;
        let range = (range & !mask) | (lps & mask);
        self.ctx[ctx_idx] = NEXT_STATE[((is_lps as usize) << 7) | s];
        // 2 <= range < 512 here, so this is zero to seven shifts.
        let shift = range.leading_zeros() - 23;
        self.range = range << shift;
        self.value <<= shift;
        self.bits_needed += shift as i32;
        if self.bits_needed >= 0 {
            let b = self.fetch();
            self.value |= b << self.bits_needed;
            self.bits_needed -= 8;
        }
        (s as u32 & 1) ^ is_lps
    }

    /// `DecodeBypass`: decodes one bin with the equiprobable model.
    #[inline(always)]
    pub fn bypass(&mut self) -> u32 {
        self.shift_one();
        let scaled = self.range << 7;
        // Bypass bins are equiprobable, so a branch here mispredicts half the
        // time; a mask does the conditional subtraction instead.
        let bit = u32::from(self.value >= scaled);
        self.value -= scaled & bit.wrapping_neg();
        bit
    }

    /// Alignment prior to aligned bypass decoding (clause 9.3.4.3.6).
    ///
    /// Sets `ivlCurrRange` to 256. A conforming stream then has `ivlOffset`
    /// below 256; anything else is corrupt and would leave the bypass
    /// decoder outside its interval.
    #[inline]
    pub fn align_bypass(&mut self) -> Result<()> {
        self.range = 256;
        if self.value >= 256 << 7 {
            return Err(Error::InvalidData("CABAC offset beyond aligned range"));
        }
        Ok(())
    }

    /// Decodes `n` bypass bins as an unsigned integer, most significant first.
    #[inline]
    pub fn bypass_bits(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.bypass();
        }
        v
    }

    /// `DecodeTerminate`: decodes `end_of_slice_segment_flag` and `pcm_flag`.
    ///
    /// This is also where a substream that ran past its end is reported: the
    /// bin decoders never fail, and every syntax structure between two
    /// terminate bins is bounded, so the error surfaces here at the latest.
    #[inline]
    pub fn terminate(&mut self) -> Result<u32> {
        self.check_overread()?;
        self.range -= 2;
        let scaled = self.range << 7;
        if self.value >= scaled {
            return Ok(1);
        }
        if self.range < 256 {
            self.range <<= 1;
            self.shift_one();
        }
        Ok(0)
    }

    /// Aligns to the next byte boundary after `DecodeTerminate` returned 1.
    ///
    /// Used before reading PCM samples, which are not arithmetically coded.
    pub fn align_after_terminate(&mut self) {
        // `DecodeTerminate` consumes no bit when it returns 1, and the last bit
        // shifted into ivlOffset is the alignment or stop bit, so the next
        // unread bit is already the first bit after it.
        self.pos = self.aligned_pos() << 3;
    }

    /// Reads `n` raw bits directly from the substream (PCM sample data).
    pub fn raw_bits(&mut self, n: u32) -> Result<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let byte_idx = self.pos >> 3;
            if byte_idx > self.data.len() + OVERREAD_SLACK {
                return Err(Error::Truncated);
            }
            let b = self.data.get(byte_idx).copied().unwrap_or(0);
            v = (v << 1) | u32::from((b >> (7 - (self.pos & 7))) & 1);
            self.pos += 1;
        }
        Ok(v)
    }
}
