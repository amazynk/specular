//! Least-significant-bit-first bit reader over a byte slice (RFC 7932 §1.5.1).
//!
//! A 64-bit buffer is topped up eight bytes at a time; bytes loaded past the bits counted in
//! `n` are the same bytes the next refill would load, so they are harmless. Reading past the
//! end of the input is an error, not zero padding.

use crate::{Error, Result};

pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    /// The next byte to load into `buf`.
    pos: usize,
    buf: u64,
    /// Valid bits in `buf`.
    n: u32,
}

impl<'a> BitReader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            buf: 0,
            n: 0,
        }
    }

    /// Makes at least 56 bits available, or all that is left of the input.
    #[inline(always)]
    pub(crate) fn refill(&mut self) {
        if self.n >= 56 {
            return;
        }
        if let Some(chunk) = self.data.get(self.pos..self.pos + 8) {
            let mut w = [0u8; 8];
            w.copy_from_slice(chunk);
            self.buf |= u64::from_le_bytes(w) << self.n;
            self.pos += ((63 - self.n) >> 3) as usize;
            self.n |= 56;
        } else {
            while self.n <= 56 {
                let Some(&b) = self.data.get(self.pos) else {
                    break;
                };
                self.buf |= u64::from(b) << self.n;
                self.pos += 1;
                self.n += 8;
            }
        }
    }

    /// The buffered bits (only the low `available()` are input; above that, zeros or input
    /// not yet counted).
    #[inline(always)]
    pub(crate) fn peek(&self) -> u64 {
        self.buf
    }

    #[inline(always)]
    pub(crate) fn available(&self) -> u32 {
        self.n
    }

    /// Drops `k` bits; the caller has checked `k <= available()`.
    #[inline(always)]
    pub(crate) fn consume(&mut self, k: u32) {
        debug_assert!(k <= self.n);
        self.buf >>= k;
        self.n -= k;
    }

    /// The next `k` bits (`k <= 24`) as an integer.
    #[inline(always)]
    pub(crate) fn read(&mut self, k: u32) -> Result<u32> {
        debug_assert!(k <= 24);
        self.refill();
        if k > self.n {
            return Err(Error::Truncated);
        }
        let v = (self.buf & ((1u64 << k) - 1)) as u32;
        self.consume(k);
        Ok(v)
    }

    #[inline(always)]
    pub(crate) fn bit(&mut self) -> Result<bool> {
        self.read(1).map(|b| b == 1)
    }

    /// Skips to the next byte boundary; the skipped bits must be zero.
    pub(crate) fn align(&mut self) -> Result<()> {
        let k = self.n & 7;
        if self.read(k)? != 0 {
            return Err(Error::Invalid("non-zero padding bits"));
        }
        Ok(())
    }

    /// The next `len` whole bytes; the reader must be byte aligned.
    pub(crate) fn bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        debug_assert_eq!(self.n & 7, 0);
        // Return the buffered whole bytes to the input.
        let at = self.pos - (self.n / 8) as usize;
        let end = at.checked_add(len).ok_or(Error::Truncated)?;
        let out = self.data.get(at..end).ok_or(Error::Truncated)?;
        self.pos = end;
        self.buf = 0;
        self.n = 0;
        Ok(out)
    }
}
