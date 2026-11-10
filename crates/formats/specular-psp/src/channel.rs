//! Channel data: decompression (none, RLE, LZ77) and row layout.

use crate::{Channel, Compression, Error};

/// PSP's RLE: a count byte above 128 repeats the next byte (count − 128) times, else copies
/// that many bytes (128 copies 128). Stops at `want` bytes; returns what was produced.
pub(crate) fn unrle(src: &[u8], want: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(want);
    let mut i = 0;
    while i < src.len() && out.len() < want {
        let c = src[i] as usize;
        i += 1;
        if c > 128 {
            let Some(&v) = src.get(i) else { break };
            i += 1;
            let n = (c - 128).min(want - out.len());
            out.resize(out.len() + n, v);
        } else {
            let e = (i + c).min(src.len());
            let n = (e - i).min(want - out.len());
            out.extend_from_slice(&src[i..i + n]);
            i = e;
        }
    }
    out
}

/// Inflates a zlib stream (falling back to the raw deflate data behind its header, so a
/// damaged or cut stream keeps the rows decoded before the damage).
fn inflate(src: &[u8], want: usize) -> Vec<u8> {
    if let Ok(v) = specular_png::inflate_zlib(src, want) {
        return v;
    }
    let raw = src.get(2..).unwrap_or(&[]);
    specular_png::inflate_raw_prefix(raw, want).unwrap_or_default()
}

/// A channel's bytes, decompressed, at most `want` of them (fewer when the data is short),
/// and their reservation from the memory budget, to keep as long as the bytes.
pub(crate) fn bytes(
    data: &[u8],
    ch: &Channel,
    comp: Compression,
    want: usize,
) -> Result<(Vec<u8>, specular_core::Reservation), Error> {
    let r = specular_core::budget::reserve(want as u64)?;
    let src = data.get(ch.data.clone()).unwrap_or(&[]);
    let v = match comp {
        Compression::None => src[..src.len().min(want)].to_vec(),
        Compression::Rle => unrle(src, want),
        Compression::Lz77 => inflate(src, want),
        Compression::Jpeg => Vec::new(),
    };
    Ok((v, r))
}

/// One channel as a plane of samples of `bits` bits (1, 4, 8 or 16) for a `w`×`h` bitmap.
pub(crate) struct Plane {
    pub(crate) data: Vec<u8>,
    pub(crate) stride: usize,
    pub(crate) bits: u32,
    pub(crate) w: usize,
    pub(crate) h: usize,
    /// Rows present (fewer than `h` when the data ran short).
    pub(crate) rows: usize,
    /// The memory budget's share of `data`, held while the plane lives.
    _res: specular_core::Reservation,
}

impl Plane {
    /// Rows of 1- and 4-bit channels are padded to 4 bytes; 8- and 16-bit rows as written by
    /// PaintShop Pro are not, although the specification says all are. The length tells.
    pub(crate) fn read(
        data: &[u8],
        ch: &Channel,
        comp: Compression,
        bits: u32,
        w: usize,
        h: usize,
    ) -> Result<Plane, Error> {
        let row = (w * bits as usize).div_ceil(8);
        let padded = row.div_ceil(4) * 4;
        let want = padded.checked_mul(h).ok_or(Error::TooLarge)?;
        let (v, res) = bytes(data, ch, comp, want)?;
        let stride = if bits < 8 || (v.len() >= want && padded != row && v.len() > row * h) {
            padded
        } else {
            row
        };
        let rows = v.len().checked_div(stride).unwrap_or(0).min(h);
        Ok(Plane {
            data: v,
            stride: stride.max(1),
            bits,
            w,
            h,
            rows,
            _res: res,
        })
    }

    pub(crate) fn complete(&self) -> bool {
        self.rows >= self.h
    }

    /// Sample `x` of row `y` (0 past the data), in the sample's own range.
    #[inline]
    pub(crate) fn get(&self, x: usize, y: usize) -> u16 {
        if y >= self.rows || x >= self.w {
            return 0;
        }
        let r = y * self.stride;
        match self.bits {
            8 => u16::from(self.data[r + x]),
            16 => {
                let i = r + 2 * x;
                self.data
                    .get(i..i + 2)
                    .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
            }
            4 => {
                let b = self.data[r + x / 2];
                u16::from(if x.is_multiple_of(2) { b >> 4 } else { b & 15 })
            }
            _ => {
                let b = self.data[r + x / 8];
                u16::from((b >> (7 - (x % 8))) & 1)
            }
        }
    }

    /// Sample scaled to 16 bits (8-bit and 16-bit planes; others as is).
    #[inline]
    pub(crate) fn get16(&self, x: usize, y: usize) -> u16 {
        match self.bits {
            8 => self.get(x, y) * 257,
            _ => self.get(x, y),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_holds_its_reservation() {
        let b = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(1 << 20));
        let data = vec![7u8; 64 * 64];
        let ch = Channel {
            data: 0..data.len(),
            dib: 0,
            kind: 0,
        };
        let p = specular_core::budget::with_budget(b.clone(), || {
            Plane::read(&data, &ch, Compression::None, 8, 64, 64).unwrap()
        });
        assert_eq!(b.reserved(), 64 * 64, "dropped while the plane lives");
        assert!(p.complete());
        drop(p);
        assert_eq!(b.reserved(), 0);
        // A 16 384 × 16 384 channel from 4 KiB is refused by a 64 MiB budget.
        let r = specular_core::budget::with_limit(64 << 20, || {
            Plane::read(&data, &ch, Compression::Rle, 8, 16_384, 16_384).map(|_| ())
        });
        assert!(matches!(r, Err(Error::NeedsMemory(_))), "{:?}", r.err());
    }

    #[test]
    fn rle_runs_and_literals() {
        let src = [131, 7, 2, 1, 2, 128 + 1, 9];
        assert_eq!(unrle(&src, 100), vec![7, 7, 7, 1, 2, 9]);
        assert_eq!(unrle(&src, 4), vec![7, 7, 7, 1]);
        // Cut short: no panic.
        assert_eq!(unrle(&[5, 1, 2], 10), vec![1, 2]);
        assert_eq!(unrle(&[200], 10), Vec::<u8>::new());
    }
}
