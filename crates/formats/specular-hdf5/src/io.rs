//! Bounds-checked little-endian reading of the file's metadata structures. Every read
//! fails with [`Error::Malformed`] rather than panicking when the structure runs past its
//! end.

use crate::{Error, Result};

/// The sizes the superblock fixes for the whole file.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ctx {
    /// Size of offsets (addresses), bytes.
    pub so: u8,
    /// Size of lengths, bytes.
    pub sl: u8,
    /// The base address: every stored address is relative to it.
    pub base: u64,
}

impl Ctx {
    /// The file position of a stored address, checked to lie inside the file.
    pub fn pos(&self, data: &[u8], addr: u64) -> Result<usize> {
        let p = self
            .base
            .checked_add(addr)
            .filter(|&p| p < data.len() as u64)
            .ok_or_else(|| {
                Error::Malformed(format!("address {addr:#x} past the end of the file"))
            })?;
        Ok(p as usize)
    }

    /// The file position of a stored address and the `len` bytes there, when they lie
    /// inside the file.
    pub fn slice<'a>(&self, data: &'a [u8], addr: u64, len: u64) -> Result<&'a [u8]> {
        let p = self.pos(data, addr)?;
        let end = (p as u64)
            .checked_add(len)
            .filter(|&e| e <= data.len() as u64)
            .ok_or_else(|| Error::Malformed(format!("block at {addr:#x} runs past the end")))?;
        Ok(&data[p..end as usize])
    }

    /// A cursor at a stored address.
    pub fn cur<'a>(&self, data: &'a [u8], addr: u64) -> Result<Cur<'a>> {
        Ok(Cur::new(data, self.pos(data, addr)?))
    }
}

/// A cursor over a byte slice.
#[derive(Clone)]
pub(crate) struct Cur<'a> {
    d: &'a [u8],
    pub pos: usize,
}

fn short(what: &str) -> Error {
    Error::Malformed(format!("{what} runs past its end"))
}

impl<'a> Cur<'a> {
    pub fn new(d: &'a [u8], pos: usize) -> Cur<'a> {
        Cur { d, pos }
    }

    pub fn remaining(&self) -> usize {
        self.d.len().saturating_sub(self.pos)
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.d.len())
            .ok_or_else(|| short("structure"))?;
        let s = &self.d[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u64(&mut self) -> Result<u64> {
        self.uint(8)
    }

    /// An `n`-byte little-endian unsigned integer (`n` ≤ 8).
    pub fn uint(&mut self, n: usize) -> Result<u64> {
        if n > 8 {
            return Err(Error::Malformed(format!("{n}-byte integer")));
        }
        let b = self.bytes(n)?;
        Ok(le(b))
    }

    /// An address; `None` when undefined (all bits set).
    pub fn addr(&mut self, c: &Ctx) -> Result<Option<u64>> {
        let n = c.so as usize;
        let v = self.uint(n)?;
        Ok((!is_undef(v, n)).then_some(v))
    }

    /// A length (size of lengths bytes).
    pub fn len(&mut self, c: &Ctx) -> Result<u64> {
        self.uint(c.sl as usize)
    }

    /// Checks a 4-byte signature.
    pub fn sig(&mut self, s: &[u8; 4], what: &str) -> Result<()> {
        if self.bytes(4)? != s {
            return Err(Error::Malformed(format!("bad {what} signature")));
        }
        Ok(())
    }
}

/// Little-endian unsigned value of up to 8 bytes.
pub(crate) fn le(b: &[u8]) -> u64 {
    b.iter().rev().fold(0u64, |a, &x| (a << 8) | u64::from(x))
}

/// Whether an `n`-byte value is the undefined address (all bits set).
pub(crate) fn is_undef(v: u64, n: usize) -> bool {
    if n >= 8 {
        v == u64::MAX
    } else {
        v == (1u64 << (n * 8)) - 1
    }
}

/// Bytes needed to store values up to `v` (at least 1): HDF5's `H5VM_limit_enc_size`.
pub(crate) fn enc_size(v: u64) -> usize {
    (log2_floor(v) / 8) as usize + 1
}

/// `floor(log2(v))`, 0 for 0.
pub(crate) fn log2_floor(v: u64) -> u32 {
    63 - v.max(1).leading_zeros()
}
