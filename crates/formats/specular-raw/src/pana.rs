//! Panasonic RW2 raw formats 5, 6 and 7 (tag 0x2D). All pack the frame into independent
//! 16-byte blocks, so rows decode in parallel.
//!
//! * Format 5: the frame row-major without row padding, in pages of 0x4000 bytes stored
//!   rotated by [`V5_SPLIT`] bytes; each block holds 10 (12-bit) or 9 (14-bit) samples, least
//!   significant bits first.
//! * Format 7: the format 5 blocks without the page rotation, each row starting a new block;
//!   the samples after the row's last whole block are not stored.
//! * Format 6: a 128-bit little-endian word per block read from the top: two absolute samples,
//!   then groups of three delta-or-absolute samples sharing a 2-bit scale (11 samples at 14
//!   bits, 14 at 12 bits). Decoded values carry a +15 offset that is removed (clamped at 0).

use rayon::prelude::*;

const BLOCK: usize = 16;
const PAGE: usize = 0x4000;
/// Format 5 pages start with the bytes stored from this offset on.
const V5_SPLIT: usize = 0x1ff8;

/// Samples per block.
fn per_block(version: u32, bits: u32) -> Option<usize> {
    match (version, bits) {
        (5 | 7, 12) => Some(10),
        (5 | 7, 14) => Some(9),
        (6, 12) => Some(14),
        (6, 14) => Some(11),
        _ => None,
    }
}

/// Whether [`decode`] handles this format version and bit depth.
pub(crate) fn supported(version: u32, bits: u32) -> bool {
    per_block(version, bits).is_some()
}

/// Bytes of encoded data for `w x h` samples.
#[cfg(test)]
pub(crate) fn data_len(version: u32, bits: u32, w: usize, h: usize) -> Option<usize> {
    let n = per_block(version, bits)?;
    if version == 7 {
        return (w / n).checked_mul(h)?.checked_mul(BLOCK);
    }
    let len = (w.checked_mul(h)?).div_ceil(n).checked_mul(BLOCK)?;
    // Format 5 pages are rotated as a whole, so only whole pages are usable.
    Some(if version == 5 {
        len.div_ceil(PAGE) * PAGE
    } else {
        len
    })
}

/// The 16 stored bytes of block `q`; `None` if past the end of the data.
#[inline]
fn block(src: &[u8], version: u32, q: usize) -> Option<[u8; BLOCK]> {
    if version != 5 {
        return src.get(q * BLOCK..q * BLOCK + BLOCK)?.try_into().ok();
    }
    let per_page = PAGE / BLOCK;
    let page = q / per_page * PAGE;
    let j = (q % per_page) * BLOCK;
    let o = (j + V5_SPLIT) % PAGE;
    if o + BLOCK <= PAGE {
        return src.get(page + o..page + o + BLOCK)?.try_into().ok();
    }
    // The block straddles the rotation point.
    let p = src.get(page..page + PAGE)?;
    Some(std::array::from_fn(|k| p[(o + k) % PAGE]))
}

/// Decode one block into `out` (`per_block` samples).
#[inline]
fn unpack(version: u32, bits: u32, b: [u8; BLOCK], out: &mut [u16]) {
    let v = u128::from_le_bytes(b);
    if version != 6 {
        let mask = (1u128 << bits) - 1;
        for (k, o) in out.iter_mut().enumerate() {
            *o = ((v >> (k as u32 * bits)) & mask) as u16;
        }
        return;
    }
    // Format 6: fields from the top of the word.
    let (wide, narrow) = if bits == 14 { (14, 10) } else { (12, 8) };
    let mut top = 128u32;
    let mut field = |n: u32| -> u32 {
        top -= n;
        ((v >> top) as u32) & ((1 << n) - 1)
    };
    // Scale >= this means the sample is absolute rather than a delta.
    let abs_base = 1u32 << bits >> 1;
    let mut first = [0u32; 2];
    let mut last = [0u32; 2];
    let (mut base, mut mul) = (0u32, 0u32);
    for (k, o) in out.iter_mut().enumerate() {
        if k % 3 == 2 {
            let s = field(2);
            let s = if s == 3 { 4 } else { s };
            base = (1 << (narrow - 1)) << s;
            mul = 1 << s;
        }
        let mut e = field(if k < 2 { wide } else { narrow });
        let p = k & 1;
        if first[p] != 0 {
            e *= mul;
            if base < abs_base && last[p] > base {
                e += last[p] - base;
            }
            last[p] = e;
        } else {
            first[p] = e;
            if e != 0 {
                last[p] = e;
            } else {
                e = last[p];
            }
        }
        *o = e.saturating_sub(15).min(u16::MAX as u32) as u16;
    }
}

/// Decode `w x h` samples. Returns whether data was missing (those samples stay zero).
pub(crate) fn decode(
    src: &[u8],
    version: u32,
    bits: u32,
    w: usize,
    h: usize,
    out: &mut [u16],
    parallel: bool,
) -> bool {
    let Some(n) = per_block(version, bits) else {
        return true;
    };
    if w == 0 {
        return false;
    }
    let row = |(y, line): (usize, &mut [u16])| -> bool {
        let mut buf = [0u16; 14];
        let p0 = y * w;
        let mut x = 0;
        let mut short = false;
        // Format 7 rows hold whole blocks only.
        let (end, per_row) = if version == 7 {
            (w / n * n, w / n)
        } else {
            (w, 0)
        };
        while x < end {
            let (q, k) = if version == 7 {
                (y * per_row + x / n, 0)
            } else {
                let p = p0 + x;
                (p / n, p % n)
            };
            let take = (n - k).min(end - x);
            match block(src, version, q) {
                Some(b) => {
                    unpack(version, bits, b, &mut buf[..n]);
                    line[x..x + take].copy_from_slice(&buf[k..k + take]);
                }
                None => short = true,
            }
            x += take;
        }
        short
    };
    let out = &mut out[..w * h];
    if parallel {
        out.par_chunks_mut(w)
            .enumerate()
            .map(row)
            .reduce(|| false, |a, b| a | b)
    } else {
        out.chunks_mut(w)
            .enumerate()
            .map(row)
            .fold(false, |a, b| a | b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_and_truncation_do_not_panic() {
        let mut s = 5u32;
        let data: Vec<u8> = (0..70_000)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (s >> 16) as u8
            })
            .collect();
        for (v, b) in [(5, 12), (5, 14), (6, 12), (6, 14), (7, 12), (7, 14)] {
            for len in [0, 15, 16, 0x4000, 0x4001, data.len()] {
                let (w, h) = (126, 40);
                let mut out = vec![0u16; w * h];
                let short = decode(&data[..len], v, b, w, h, &mut out, false);
                assert_eq!(short, len < data_len(v, b, w, h).unwrap(), "v{v} {b} {len}");
            }
        }
    }
}
