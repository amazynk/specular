//! Adaptive Entropy Coding (CCSDS 121.0-B) decoder.
//!
//! The lossless Rice coder that libaec and the SZIP library implement: blocks of residuals
//! coded with the zero-block, second-extension, split-sample (`k` = 0 for the fundamental
//! sequence) or no-compression option, chosen per block by an ID of 1 to 5 bits, and a
//! reference sample at the start of every reference sample interval (RSI) when the unit-delay
//! predictor is on. GRIB2 template 5.42 hands the stream and its flags over directly; HDF5's
//! SZIP filter (id 4) goes through [`szip`].
//!
//! Written from the CCSDS 121.0-B-3 blue book; libaec's flag values are kept so GRIB's
//! `ccsdsFlags` octet can be passed straight through.

use thiserror::Error;

/// Samples are two's complement.
pub const SIGNED: u8 = 1;
/// Samples of 17 to 24 bits take three bytes in [`decode_bytes`] output (four otherwise).
pub const THREE_BYTE: u8 = 2;
/// [`decode_bytes`] writes samples most significant byte first.
pub const MSB: u8 = 4;
/// The unit-delay predictor and its mapping are on (reference samples present).
pub const PREPROCESS: u8 = 8;
/// Restricted IDs for 1 to 4 bit samples (CCSDS 121.0-B-3).
pub const RESTRICTED: u8 = 16;
/// Every RSI starts on a byte boundary.
pub const PAD_RSI: u8 = 32;

/// Why a stream can't be decoded.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    /// The parameters are outside what CCSDS 121.0 allows.
    #[error("unsupported AEC parameters: {0}")]
    Params(&'static str),
    /// The stream ended before all samples were decoded.
    #[error("AEC stream ends early")]
    Truncated,
    /// A code that no encoder writes.
    #[error("corrupt AEC stream")]
    Corrupt,
}

/// Coding parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    /// Bits per sample, 1 to 32.
    pub bits_per_sample: u32,
    /// Samples per block: 8, 16, 32 or 64 (any even size up to 64 is accepted).
    pub block_size: u32,
    /// Blocks per reference sample interval.
    pub rsi: u32,
    /// [`SIGNED`], [`PREPROCESS`], [`RESTRICTED`], [`PAD_RSI`], ... (libaec's values).
    pub flags: u8,
}

impl Params {
    fn check(&self) -> Result<(), Error> {
        if !(1..=32).contains(&self.bits_per_sample) {
            return Err(Error::Params("bits per sample"));
        }
        if self.block_size == 0 || self.block_size > 64 || self.block_size % 2 == 1 {
            return Err(Error::Params("block size"));
        }
        if self.rsi == 0 || self.rsi > 4096 {
            return Err(Error::Params("reference sample interval"));
        }
        Ok(())
    }

    fn id_len(&self) -> u32 {
        let n = self.bits_per_sample;
        if self.flags & RESTRICTED != 0 && n <= 4 {
            if n <= 2 { 1 } else { 2 }
        } else if n <= 8 {
            3
        } else if n <= 16 {
            4
        } else {
            5
        }
    }

    /// Bytes per sample in [`decode_bytes`] output.
    pub fn sample_bytes(&self) -> usize {
        match self.bits_per_sample {
            0..=8 => 1,
            9..=16 => 2,
            17..=24 if self.flags & THREE_BYTE != 0 => 3,
            _ => 4,
        }
    }
}

/// MSB-first bit reader over a byte slice, 64 bits buffered.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    acc: u64,
    /// Valid bits at the top of `acc`.
    n: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            acc: 0,
            n: 0,
        }
    }

    #[inline]
    fn refill(&mut self) {
        if self.pos + 8 <= self.data.len() && self.n <= 32 {
            let mut b = [0u8; 8];
            b.copy_from_slice(&self.data[self.pos..self.pos + 8]);
            let w = u64::from_be_bytes(b);
            let take = (64 - self.n) / 8;
            self.acc |= (w >> self.n) & mask_top(self.n + take * 8);
            self.pos += take as usize;
            self.n += take * 8;
            return;
        }
        while self.n <= 56 && self.pos < self.data.len() {
            self.acc |= u64::from(self.data[self.pos]) << (56 - self.n);
            self.pos += 1;
            self.n += 8;
        }
    }

    /// `k` bits (0 to 32).
    #[inline]
    fn get(&mut self, k: u32) -> Result<u32, Error> {
        if k == 0 {
            return Ok(0);
        }
        if self.n < k {
            self.refill();
            if self.n < k {
                return Err(Error::Truncated);
            }
        }
        let v = (self.acc >> (64 - k)) as u32;
        self.acc <<= k;
        self.n -= k;
        Ok(v)
    }

    /// A fundamental sequence codeword: the count of zeros before the next one.
    #[inline]
    fn fs(&mut self) -> Result<u32, Error> {
        let mut count = 0;
        loop {
            if self.n == 0 {
                self.refill();
                if self.n == 0 {
                    return Err(Error::Truncated);
                }
            }
            let z = self.acc.leading_zeros();
            if z < self.n {
                self.acc = self.acc.checked_shl(z + 1).unwrap_or(0);
                self.n -= z + 1;
                return Ok(count + z);
            }
            count += self.n;
            self.acc = 0;
            self.n = 0;
            if count > 1 << 24 {
                return Err(Error::Corrupt);
            }
        }
    }

    /// Skips to the next byte boundary.
    fn align(&mut self) {
        let r = self.n % 8;
        self.acc <<= r;
        self.n -= r;
    }

    /// Bytes consumed so far (rounded up to whole bytes).
    fn consumed(&self) -> usize {
        self.pos - (self.n / 8) as usize
    }
}

/// The top `bits` bits of a u64 set.
#[inline]
fn mask_top(bits: u32) -> u64 {
    if bits == 0 {
        0
    } else {
        u64::MAX << (64 - bits)
    }
}

/// Second-extension pairs: for codeword `m`, the pair sum and the first codeword of that sum.
const SE: [(u8, u8); 91] = {
    let mut t = [(0u8, 0u8); 91];
    let mut k = 0;
    let mut i = 0;
    while i < 13 {
        let start = k;
        let mut j = 0;
        while j <= i {
            t[k] = (i as u8, start as u8);
            k += 1;
            j += 1;
        }
        i += 1;
    }
    t
};

/// Decodes `count` samples into raw `bits_per_sample`-bit values (two's complement samples
/// keep their bit pattern, masked to the sample width).
pub fn decode(input: &[u8], p: &Params, count: usize) -> Result<Vec<u32>, Error> {
    decode_inner(input, p, count).map(|(v, _)| v)
}

fn decode_inner(input: &[u8], p: &Params, count: usize) -> Result<(Vec<u32>, usize), Error> {
    p.check()?;
    let n = p.bits_per_sample;
    let id_len = p.id_len();
    let id_max = (1u32 << id_len) - 1;
    let bs = p.block_size as usize;
    let pre = p.flags & PREPROCESS != 0;
    let rsi_len = bs * p.rsi as usize;
    // `count` comes from the caller's header: reserve only what the input can plausibly
    // hold (zero blocks may expand further; the vector then grows as samples arrive).
    let mut out =
        Vec::with_capacity(count.min(input.len().saturating_mul(8).saturating_add(rsi_len)));
    let mut bits = Bits::new(input);
    // Residuals of one RSI, reference sample first.
    let mut buf: Vec<u32> = Vec::with_capacity(rsi_len);
    while out.len() < count {
        buf.clear();
        let want = (count - out.len()).min(rsi_len);
        let mut b = 0usize;
        while buf.len() < want {
            let r = usize::from(pre && b == 0);
            let id = bits.get(id_len)?;
            if id == 0 {
                let second = bits.get(1)? == 1;
                if r == 1 {
                    buf.push(bits.get(n)?);
                }
                if second {
                    let mut i = r;
                    while i < bs {
                        let m = bits.fs()? as usize;
                        let &(sum, first) = SE.get(m).ok_or(Error::Corrupt)?;
                        let d1 = (m - usize::from(first)) as u32;
                        if i % 2 == 0 {
                            buf.push(u32::from(sum) - d1);
                            i += 1;
                        }
                        buf.push(d1);
                        i += 1;
                    }
                    b += 1;
                } else {
                    let mut z = bits.fs()? as usize + 1;
                    const ROS: usize = 5;
                    if z == ROS {
                        let left = p.rsi as usize - b;
                        z = left.min(64 - b % 64);
                    } else if z > ROS {
                        z -= 1;
                    }
                    let len = (z * bs - r).min(rsi_len - buf.len());
                    buf.resize(buf.len() + len, 0);
                    b += z;
                }
            } else if id == id_max {
                for _ in 0..bs {
                    buf.push(bits.get(n)?);
                }
                b += 1;
            } else {
                let k = id - 1;
                if k >= n {
                    return Err(Error::Corrupt);
                }
                if r == 1 {
                    buf.push(bits.get(n)?);
                }
                let at = buf.len();
                for _ in r..bs {
                    let q = bits.fs()?;
                    buf.push(q);
                }
                if k > 0 {
                    for v in &mut buf[at..] {
                        *v = (*v << k) | bits.get(k)?;
                    }
                }
                b += 1;
            }
            if b > p.rsi as usize {
                return Err(Error::Corrupt);
            }
        }
        buf.truncate(want);
        if pre {
            unmap(&buf, n, p.flags & SIGNED != 0, &mut out);
        } else {
            out.extend_from_slice(&buf);
        }
        if p.flags & PAD_RSI != 0 {
            bits.align();
        }
    }
    Ok((out, bits.consumed()))
}

/// Undoes the predictor: `res[0]` is the reference sample, the rest mapped residuals.
fn unmap(res: &[u32], n: u32, signed: bool, out: &mut Vec<u32>) {
    let Some((&first, rest)) = res.split_first() else {
        return;
    };
    let mask = if n == 32 { u32::MAX } else { (1u32 << n) - 1 };
    let (xmin, xmax): (i64, i64) = if signed {
        (-(1i64 << (n - 1)), (1i64 << (n - 1)) - 1)
    } else {
        (0, (1i64 << n) - 1)
    };
    let ext = |v: u32| -> i64 {
        if signed && n < 32 && v >> (n - 1) & 1 == 1 {
            i64::from(v) - (1i64 << n)
        } else if signed && n == 32 {
            i64::from(v as i32)
        } else {
            i64::from(v)
        }
    };
    let mut x = ext(first);
    out.push(first & mask);
    for &d in rest {
        let d = i64::from(d);
        let lo = x - xmin;
        let hi = xmax - x;
        let theta = lo.min(hi);
        x = if d <= 2 * theta {
            if d & 1 == 0 {
                x + d / 2
            } else {
                x - (d + 1) / 2
            }
        } else if lo < hi {
            xmin + d
        } else {
            xmax - d
        };
        out.push((x as u32) & mask);
    }
}

/// Decodes `count` samples into libaec's output layout: each sample in
/// [`Params::sample_bytes`] bytes, most significant first with [`MSB`].
pub fn decode_bytes(input: &[u8], p: &Params, count: usize) -> Result<Vec<u8>, Error> {
    let v = decode(input, p, count)?;
    let w = p.sample_bytes();
    let msb = p.flags & MSB != 0;
    let mut out = Vec::with_capacity(v.len() * w);
    for x in v {
        let b = x.to_le_bytes();
        if msb {
            out.extend(b[..w].iter().rev());
        } else {
            out.extend_from_slice(&b[..w]);
        }
    }
    Ok(out)
}

pub mod szip;

#[cfg(test)]
mod tests;
