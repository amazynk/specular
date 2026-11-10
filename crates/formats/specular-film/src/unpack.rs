//! Integer sample unpacking shared by DPX and Cineon.
//!
//! Two shapes cover every packing either format defines:
//! - a bit *stream* across 32-bit words (DPX "packed": least significant bit first; Cineon
//!   packing 0: most significant bit first);
//! - *containers*: units of 1, 2 or 4 bytes, each holding as many samples as fit, first
//!   sample nearest the most significant end (or the least, [`Layout::low_first`]), padding
//!   either at the least significant end (DPX method A, Cineon "left justified") or the most
//!   significant (method B, "right").
//!
//! Units and words are read in the file's byte order. Rows start on a 32-bit boundary unless
//! the data is too short for that and fits as one continuous stream (some writers never pad).

use crate::{Error, Result};
use rayon::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Stream {
        lsb_first: bool,
    },
    Container {
        unit: u8,
        left: bool,
    },
    /// IEEE floats of `bits` 32 or 64.
    Float,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Layout {
    pub bits: u8,
    pub shape: Shape,
    pub be: bool,
    /// Containers holding several samples: the first sample nearest the least significant
    /// end (SMPTE 268M's datum order, kept for one-component DPX), not the most significant.
    pub low_first: bool,
}

impl Layout {
    fn per_unit(&self) -> usize {
        match self.shape {
            Shape::Container { unit, .. } => (unit as usize * 8 / self.bits as usize).max(1),
            _ => 1,
        }
    }

    pub fn validate(&self) -> Result<()> {
        let ok = match self.shape {
            Shape::Float => self.bits == 32 || self.bits == 64,
            Shape::Stream { .. } => (1..=16).contains(&self.bits),
            Shape::Container { unit, .. } => {
                matches!(unit, 1 | 2 | 4)
                    && (1..=16).contains(&self.bits)
                    && self.bits as u32 <= unit as u32 * 8
            }
        };
        if ok {
            Ok(())
        } else {
            Err(Error::Unsupported(format!(
                "{}-bit samples in this packing are not supported",
                self.bits
            )))
        }
    }

    /// Bytes `n` samples take (unpadded).
    pub fn bytes_for(&self, n: u64) -> u64 {
        match self.shape {
            Shape::Stream { .. } => (n * self.bits as u64).div_ceil(32) * 4,
            Shape::Container { unit, .. } => n.div_ceil(self.per_unit() as u64) * unit as u64,
            Shape::Float => n * self.bits as u64 / 8,
        }
    }

    /// Bytes of one row of `n` samples, padded to a 32-bit boundary.
    pub fn row_bytes(&self, n: u64) -> u64 {
        self.bytes_for(n).next_multiple_of(4)
    }

    fn word(&self, d: &[u8], o: usize) -> u32 {
        match d.get(o..o + 4) {
            Some(b) => {
                let b: [u8; 4] = [b[0], b[1], b[2], b[3]];
                if self.be {
                    u32::from_be_bytes(b)
                } else {
                    u32::from_le_bytes(b)
                }
            }
            None => 0,
        }
    }

    fn unit(&self, d: &[u8], o: usize, unit: u8) -> u32 {
        match unit {
            1 => d[o] as u32,
            2 => {
                let b = [d[o], d[o + 1]];
                (if self.be {
                    u16::from_be_bytes(b)
                } else {
                    u16::from_le_bytes(b)
                }) as u32
            }
            _ => self.word(d, o),
        }
    }

    /// Unpacks `out.len()` samples starting `first` samples after byte `base`. Returns how
    /// many were present in `d` (the rest are left as they are).
    pub fn unpack(&self, d: &[u8], base: usize, first: usize, out: &mut [u16]) -> usize {
        let bits = self.bits as usize;
        let mask = if bits >= 16 {
            0xffff
        } else {
            (1u32 << bits) - 1
        };
        let Some(d) = d.get(base..) else {
            return 0;
        };
        match self.shape {
            Shape::Stream { lsb_first } => {
                let total_bits = d.len() * 8 / 32 * 32;
                for (j, o) in out.iter_mut().enumerate() {
                    let p = (first + j) * bits;
                    if p + bits > total_bits {
                        return j;
                    }
                    let (w, off) = (p / 32, p % 32);
                    let lo = self.word(d, w * 4) as u64;
                    let hi = if off + bits > 32 {
                        self.word(d, w * 4 + 4) as u64
                    } else {
                        0
                    };
                    let v = if lsb_first {
                        ((hi << 32 | lo) >> off) as u32
                    } else {
                        ((lo << 32 | hi) >> (64 - off - bits)) as u32
                    };
                    *o = (v & mask) as u16;
                }
                out.len()
            }
            Shape::Container { unit, left } => {
                let per = self.per_unit();
                let ub = unit as usize;
                let pad = ub * 8 - per * bits;
                let units = d.len() / ub;
                let mut j = 0;
                let mut u = first / per;
                let mut k = first % per;
                while j < out.len() {
                    if u >= units {
                        return j;
                    }
                    let v = self.unit(d, u * ub, unit);
                    while k < per && j < out.len() {
                        let shift = if self.low_first {
                            bits * k + if left { pad } else { 0 }
                        } else {
                            ub * 8 - bits * (k + 1) - if left { 0 } else { pad }
                        };
                        out[j] = ((v >> shift) & mask) as u16;
                        j += 1;
                        k += 1;
                    }
                    k = 0;
                    u += 1;
                }
                out.len()
            }
            Shape::Float => 0,
        }
    }

    /// Unpacks float samples (as `f32`), like [`Layout::unpack`].
    pub fn unpack_float(&self, d: &[u8], base: usize, first: usize, out: &mut [f32]) -> usize {
        let sz = self.bits as usize / 8;
        let Some(d) = d.get(base..) else {
            return 0;
        };
        for (j, o) in out.iter_mut().enumerate() {
            let p = (first + j) * sz;
            let Some(b) = d.get(p..p + sz) else {
                return j;
            };
            *o = if sz == 4 {
                let b = [b[0], b[1], b[2], b[3]];
                f32::from_bits(if self.be {
                    u32::from_be_bytes(b)
                } else {
                    u32::from_le_bytes(b)
                })
            } else {
                let b = [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]];
                (if self.be {
                    f64::from_be_bytes(b)
                } else {
                    f64::from_le_bytes(b)
                }) as f32
            };
        }
        out.len()
    }
}

/// Where each row's samples are: `rows` rows of `n` samples starting at byte `base`, each
/// `stride` bytes apart (aligned rows) or as one continuous stream (`stride` 0).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rows {
    pub base: usize,
    pub stride: usize,
    pub n: usize,
}

impl Rows {
    /// Chooses aligned rows (plus `eol` padding bytes) when the data holds them, else a
    /// continuous stream when that fits, else aligned rows (the image is then truncated).
    pub fn choose(
        l: &Layout,
        base: usize,
        avail: usize,
        n: usize,
        rows: usize,
        eol: usize,
    ) -> Rows {
        let stride = l.row_bytes(n as u64) as usize + eol;
        let aligned = stride as u64 * rows as u64;
        let continuous = l.bytes_for(n as u64 * rows as u64);
        let stride = if (avail as u64) < aligned && (avail as u64) >= continuous && eol == 0 {
            0
        } else {
            stride
        };
        Rows { base, stride, n }
    }

    /// (byte offset, first sample index) of row `y`.
    pub fn at(&self, y: usize) -> (usize, usize) {
        if self.stride == 0 {
            (self.base, y * self.n)
        } else {
            (self.base + y * self.stride, 0)
        }
    }
}

/// Runs `f(y, out_row, scratch)` over the rows of `out` (each `row_len` long) in parallel,
/// with a per-thread scratch row of `n` samples. Returns whether any row was short.
pub(crate) fn for_rows<T: Send, S: Default + Clone + Send>(
    out: &mut [T],
    row_len: usize,
    n: usize,
    f: impl Fn(usize, &mut [T], &mut [S]) -> bool + Sync,
) -> bool {
    if row_len == 0 {
        return false;
    }
    out.par_chunks_mut(row_len)
        .enumerate()
        .map_init(
            || vec![S::default(); n],
            |scratch, (y, row)| !f(y, row, scratch),
        )
        .reduce(|| false, |a, b| a | b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_a_and_b() {
        let a = Layout {
            bits: 10,
            shape: Shape::Container {
                unit: 4,
                left: true,
            },
            be: true,
            low_first: false,
        };
        let w: u32 = (1023 << 22) | (5 << 12) | (7 << 2);
        let mut out = [0u16; 3];
        assert_eq!(a.unpack(&w.to_be_bytes(), 0, 0, &mut out), 3);
        assert_eq!(out, [1023, 5, 7]);
        let b = Layout {
            shape: Shape::Container {
                unit: 4,
                left: false,
            },
            ..a
        };
        let w: u32 = (1023 << 20) | (5 << 10) | 7;
        assert_eq!(b.unpack(&w.to_be_bytes(), 0, 1, &mut out[..2]), 2);
        assert_eq!(out[..2], [5, 7]);
        // One-component DPX: the first datum in the low bits (after method A's padding).
        let c = Layout {
            low_first: true,
            ..a
        };
        let w: u32 = (200 << 22) | (150 << 12) | (100 << 2);
        assert_eq!(c.unpack(&w.to_be_bytes(), 0, 0, &mut out), 3);
        assert_eq!(out, [100, 150, 200]);
        let d = Layout {
            low_first: true,
            ..b
        };
        let w: u32 = (200 << 20) | (150 << 10) | 100;
        assert_eq!(d.unpack(&w.to_be_bytes(), 0, 1, &mut out[..2]), 2);
        assert_eq!(out[..2], [150, 200]);
    }

    #[test]
    fn streams() {
        let l = Layout {
            bits: 10,
            shape: Shape::Stream { lsb_first: true },
            be: false,
            low_first: false,
        };
        // 0x400003ff, 0xa000aad5: 1023, 0, 0, then 0x355 across the words.
        let d = [0xff, 0x03, 0x00, 0x40, 0xd5, 0xaa, 0x00, 0xa0];
        let mut out = [0u16; 4];
        assert_eq!(l.unpack(&d, 0, 0, &mut out), 4);
        assert_eq!(out, [1023, 0, 0, 0x355]);
        let m = Layout {
            shape: Shape::Stream { lsb_first: false },
            be: true,
            ..l
        };
        // 1023, 1, 2 then 3 straddling: bits MSB first.
        let mut bits = 0u64;
        for v in [1023u64, 1, 2, 3, 4, 5] {
            bits = bits << 10 | v;
        }
        let bits = bits << 4;
        let d = bits.to_be_bytes();
        assert_eq!(m.unpack(&d, 0, 0, &mut out), 4);
        assert_eq!(out, [1023, 1, 2, 3]);
    }
}
