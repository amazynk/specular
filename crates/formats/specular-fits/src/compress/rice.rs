//! `RICE_1`: the Rice coding of the tiled image convention.
//!
//! A tile starts with its first value as stored (`bytepix` big-endian bytes); then, MSB first,
//! each block of `blocksize` pixels (the last may be shorter) has a split code of 3, 4 or 5
//! bits (for 1, 2 or 4 bytes per pixel). The code minus one is the split `fs`: −1 means every
//! difference in the block is zero; the maximum (6, 14 or 25) means the differences follow
//! uncoded at 8, 16 or 32 bits each; otherwise each difference is a unary high part (zeros
//! ended by a one) followed by `fs` low bits. Differences are zig-zag mapped (even ≥ 0, odd
//! < 0) and accumulate in wrapping unsigned arithmetic from the first value.
//!
//! The decoder keeps up to 63 bits left-aligned in a `u64` and refills eight bytes at a time,
//! so a typical pixel is one refill check, one `leading_zeros` and two shifts.

use super::CodecError;

/// Decodes a `RICE_1` tile of `out.len()` pixels with `bytepix` bytes per pixel (1, 2 or 4)
/// and `blocksize` pixels per block. Values come out as the stored integer type would hold
/// them: bytes unsigned (0..=255), 2-byte values as `i16`, 4-byte values as `i32`.
pub fn rice(
    input: &[u8],
    bytepix: usize,
    blocksize: usize,
    out: &mut [i32],
) -> Result<(), CodecError> {
    if blocksize == 0 {
        return Err(CodecError::Corrupt("Rice block size 0"));
    }
    match bytepix {
        1 => decode::<1>(input, blocksize, out),
        2 => decode::<2>(input, blocksize, out),
        4 => decode::<4>(input, blocksize, out),
        _ => Err(CodecError::Corrupt("Rice bytes per pixel not 1, 2 or 4")),
    }
}

/// An MSB-first bit reader: `n` valid bits left-aligned in `buf`.
struct Bits<'a> {
    data: &'a [u8],
    /// Next byte to load.
    pos: usize,
    buf: u64,
    n: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Bits {
            data,
            pos: 0,
            buf: 0,
            n: 0,
        }
    }

    /// 56 to 63 valid bits afterwards (zeros past the end of the data, see `overrun`).
    #[inline(always)]
    fn refill(&mut self) {
        if self.n >= 56 {
            return;
        }
        if let Some(b) = self.data.get(self.pos..self.pos + 8) {
            let w = u64::from_be_bytes(b.try_into().unwrap_or([0; 8]));
            // Bits past `n` may already hold the same upcoming bits: OR is idempotent.
            self.buf |= w >> self.n;
            self.pos += ((63 - self.n) >> 3) as usize;
            self.n |= 56;
        } else {
            while self.n < 56 {
                let b = self.data.get(self.pos).copied().unwrap_or(0);
                self.buf |= u64::from(b) << (56 - self.n);
                self.n += 8;
                self.pos += 1;
            }
        }
    }

    /// Drops `k` (≤ `n` ≤ 63) bits.
    #[inline(always)]
    fn consume(&mut self, k: u32) {
        self.buf <<= k;
        self.n -= k;
    }

    /// The next `k` (≤ 32 ≤ `n`) bits.
    #[inline(always)]
    fn take(&mut self, k: u32) -> u32 {
        if k == 0 {
            return 0;
        }
        let v = (self.buf >> (64 - k)) as u32;
        self.consume(k);
        v
    }

    /// More bits were used than the data has.
    fn overrun(&self) -> bool {
        self.pos * 8 > self.data.len() * 8 + self.n as usize
    }
}

#[inline(always)]
fn unmap(d: u32) -> u32 {
    if d & 1 == 0 { d >> 1 } else { !(d >> 1) }
}

#[inline(always)]
fn conv<const B: usize>(v: u32) -> i32 {
    match B {
        1 => (v & 0xFF) as i32,
        2 => i32::from(v as u16 as i16),
        _ => v as i32,
    }
}

fn decode<const B: usize>(
    input: &[u8],
    blocksize: usize,
    out: &mut [i32],
) -> Result<(), CodecError> {
    let (fsbits, fsmax, bbits): (u32, i32, u32) = match B {
        1 => (3, 6, 8),
        2 => (4, 14, 16),
        _ => (5, 25, 32),
    };
    if out.is_empty() {
        return Ok(());
    }
    let Some(first) = input.get(..B) else {
        return Err(CodecError::Corrupt(
            "Rice tile shorter than its first value",
        ));
    };
    let mut last = first.iter().fold(0u32, |a, &b| a << 8 | u32::from(b));
    let mut r = Bits::new(&input[B..]);
    for block in out.chunks_mut(blocksize) {
        r.refill();
        let fs = r.take(fsbits) as i32 - 1;
        if fs < 0 {
            block.fill(conv::<B>(last));
        } else if fs == fsmax {
            for v in block.iter_mut() {
                r.refill();
                last = last.wrapping_add(unmap(r.take(bbits)));
                *v = conv::<B>(last);
            }
        } else if fs > fsmax {
            return Err(CodecError::Corrupt("Rice split code out of range"));
        } else {
            let fs = fs as u32;
            for v in block.iter_mut() {
                r.refill();
                let lz = r.buf.leading_zeros();
                let d = if lz + 1 + fs <= r.n {
                    // The whole code is in the buffer (fs = 0 shifts everything out).
                    let low = ((r.buf << (lz + 1)) >> 1 >> (63 - fs)) as u32;
                    r.consume(lz + 1 + fs);
                    (lz << fs) | low
                } else {
                    let nz = zeros(&mut r)?;
                    r.refill();
                    nz.wrapping_shl(fs) | r.take(fs)
                };
                last = last.wrapping_add(unmap(d));
                *v = conv::<B>(last);
            }
        }
        if r.overrun() {
            return Err(CodecError::Corrupt("Rice tile truncated"));
        }
    }
    Ok(())
}

/// Counts and drops a unary run of zeros and its closing one (a long run, past the buffer).
#[cold]
fn zeros(r: &mut Bits<'_>) -> Result<u32, CodecError> {
    let mut nz = 0u32;
    loop {
        r.refill();
        let lz = r.buf.leading_zeros();
        if lz < r.n {
            r.consume(lz + 1);
            return Ok(nz.saturating_add(lz));
        }
        nz = nz.saturating_add(r.n);
        let n = r.n;
        r.consume(n);
        if r.overrun() {
            return Err(CodecError::Corrupt("Rice tile truncated"));
        }
    }
}
