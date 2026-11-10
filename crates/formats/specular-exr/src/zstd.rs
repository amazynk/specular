//! ZSTD chunks (OpenEXR 3.5, compression 13): one scanline per chunk (or a tile), flat or deep.
//!
//! Chunk layout (little-endian): the magic `zstd-exr` (u64), the wire format (u32: 1, or 2
//! with flag bit 0 "delta after sort"), the flags (u32), the Zstandard frame's size (u64),
//! then that frame. A chunk that would not shrink is stored raw, without the prefix.
//!
//! The frame decompresses to one or two segments, each a u64 length and that many bytes of
//! byte planes (byte 0 of every element, then byte 1, ...): first every 16-bit row of the
//! chunk (element size 2), then every 32-bit row (element size 4); a chunk with only one kind
//! has one segment. A "row" is one channel's samples on one chunk line, in line order and
//! within a line in channel order (the packed layout's order, split by sample size). With
//! the delta flag every row holds differences of its elements (wrapping, as integers), undone
//! by a running sum. Putting the rows back in packed order gives OpenEXR's uncompressed
//! layout ([`crate::codec::Layout::Lines`]).
//!
//! Deep chunks use the same coding: the sample-count table as one 32-bit row of `width`
//! values per line, the sample data with each line's total sample count as every channel's
//! row length on that line.

use std::io::Read;

use crate::codec::BlockGeom;

const MAGIC: u64 = u64::from_le_bytes(*b"zstd-exr");
const PREFIX: usize = 24;
const FORMAT_V1: u32 = 1;
const FORMAT_V2: u32 = 2;
const FLAG_DELTA: u32 = 1;

/// Decode one ZSTD chunk whose packed rows are `rows` (per chunk line, per channel:
/// `(samples, element bytes)`, element bytes 2 or 4) into `out` (exactly the rows' total).
/// `None` on malformed data.
pub(crate) fn decompress(input: &[u8], rows: &[(usize, usize)], out: &mut [u8]) -> Option<()> {
    let u32_at = |p: usize| Some(u32::from_le_bytes(input.get(p..p + 4)?.try_into().ok()?));
    let u64_at = |p: usize| Some(u64::from_le_bytes(input.get(p..p + 8)?.try_into().ok()?));
    if u64_at(0)? != MAGIC {
        return None;
    }
    let (format, flags) = (u32_at(8)?, u32_at(12)?);
    let delta = match format {
        FORMAT_V1 if flags == 0 => false,
        FORMAT_V2 if flags & !FLAG_DELTA == 0 => flags & FLAG_DELTA != 0,
        _ => return None,
    };
    if u64_at(16)? != (input.len() - PREFIX) as u64 {
        return None;
    }
    let total = out.len();
    let mut split = 0usize;
    let mut sum = 0usize;
    for &(n, b) in rows {
        if b != 2 && b != 4 {
            return None;
        }
        let len = n.checked_mul(b)?;
        sum = sum.checked_add(len)?;
        if b == 2 {
            split += len;
        }
    }
    if sum != total {
        return None;
    }
    // The inner stream: the data plus at most two 8-byte segment lengths.
    let cap = total.checked_add(16)?;
    let mut inner = Vec::new();
    inner.try_reserve_exact(cap).ok()?;
    ruzstd::decoding::StreamingDecoder::new(&input[PREFIX..])
        .ok()?
        .take(cap as u64 + 1)
        .read_to_end(&mut inner)
        .ok()?;
    if inner.len() > cap {
        return None;
    }
    let segments: &[(usize, usize)] = match (split, total - split) {
        (0, rest) => &[(rest, 4)],
        (half, 0) => &[(half, 2)],
        (half, rest) => &[(half, 2), (rest, 4)],
    };
    let mut sorted = Vec::new();
    sorted.try_reserve_exact(total).ok()?;
    sorted.resize(total, 0u8);
    let mut q = 0usize;
    let mut at = 0usize;
    for &(len, el) in segments {
        let l = u64::from_le_bytes(inner.get(q..q + 8)?.try_into().ok()?);
        q += 8;
        if l != len as u64 {
            return None;
        }
        let planes = inner.get(q..q + len)?;
        unshuffle(planes, el, &mut sorted[at..at + len]);
        q += len;
        at += len;
    }
    if q != inner.len() {
        return None;
    }
    // Sorted offsets: 16-bit rows from 0, 32-bit rows from `split`, each in packed order.
    let (mut half, mut full, mut packed) = (0usize, split, 0usize);
    for &(n, b) in rows {
        let len = n * b;
        let src = if b == 2 { &mut half } else { &mut full };
        let row = &mut sorted[*src..*src + len];
        if delta {
            undo_delta(row, b);
        }
        out[packed..packed + len].copy_from_slice(row);
        *src += len;
        packed += len;
    }
    Some(())
}

/// A flat chunk (scanlines or a tile) of geometry `g`.
pub(crate) fn decompress_flat(input: &[u8], g: &BlockGeom, out: &mut [u8]) -> Option<()> {
    let mut rows = Vec::with_capacity(g.h * g.chans.len());
    for line in 0..g.h {
        let y = g.y0.checked_add(i32::try_from(line).ok()?)?;
        for (c, cg) in g.chans.iter().enumerate() {
            rows.push((if g.sampled(c, y) { cg.nx } else { 0 }, cg.bytes));
        }
    }
    decompress(input, &rows, out)
}

/// Byte planes of `el`-byte elements back to elements.
fn unshuffle(planes: &[u8], el: usize, out: &mut [u8]) {
    let n = planes.len() / el;
    for (b, plane) in planes.chunks_exact(n.max(1)).take(el).enumerate() {
        for (i, &v) in plane.iter().enumerate() {
            out[i * el + b] = v;
        }
    }
}

/// Running sum (wrapping) over the little-endian elements of one row.
fn undo_delta(row: &mut [u8], el: usize) {
    if el == 2 {
        let mut acc = 0u16;
        for e in row.chunks_exact_mut(2) {
            acc = acc.wrapping_add(u16::from_le_bytes([e[0], e[1]]));
            e.copy_from_slice(&acc.to_le_bytes());
        }
    } else {
        let mut acc = 0u32;
        for e in row.chunks_exact_mut(4) {
            acc = acc.wrapping_add(u32::from_le_bytes([e[0], e[1], e[2], e[3]]));
            e.copy_from_slice(&acc.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encoder for tests: the inverse of every step above, with a stored (raw) Zstandard frame.
    pub(crate) fn encode(rows: &[(usize, usize)], packed: &[u8], delta: bool) -> Vec<u8> {
        let mut half = Vec::new();
        let mut full = Vec::new();
        let mut p = 0;
        for &(n, b) in rows {
            let mut row = packed[p..p + n * b].to_vec();
            p += n * b;
            if delta {
                let v: Vec<u32> = row
                    .chunks_exact(b)
                    .map(|e| {
                        if b == 2 {
                            u16::from_le_bytes([e[0], e[1]]) as u32
                        } else {
                            u32::from_le_bytes(e.try_into().unwrap())
                        }
                    })
                    .collect();
                for (k, e) in row.chunks_exact_mut(b).enumerate() {
                    let d = if k == 0 {
                        v[0]
                    } else {
                        v[k].wrapping_sub(v[k - 1])
                    };
                    e.copy_from_slice(&d.to_le_bytes()[..b]);
                }
            }
            if b == 2 { &mut half } else { &mut full }.extend(row);
        }
        let mut inner = Vec::new();
        for (seg, el) in [(half, 2), (full, 4)] {
            if seg.is_empty() {
                continue;
            }
            inner.extend((seg.len() as u64).to_le_bytes());
            let n = seg.len() / el;
            for b in 0..el {
                inner.extend((0..n).map(|i| seg[i * el + b]));
            }
        }
        if inner.is_empty() {
            inner.extend(0u64.to_le_bytes());
        }
        // A Zstandard frame of one raw block (RFC 8878 3.1.1): no checksum, content size
        // in 8 bytes.
        let mut frame = vec![0x28, 0xB5, 0x2F, 0xFD, 0xE0];
        frame.extend((inner.len() as u64).to_le_bytes());
        let bh = ((inner.len() as u32) << 3) | 1;
        frame.extend(&bh.to_le_bytes()[..3]);
        frame.extend(&inner);
        let mut out = MAGIC.to_le_bytes().to_vec();
        out.extend((if delta { FORMAT_V2 } else { FORMAT_V1 }).to_le_bytes());
        out.extend((delta as u32).to_le_bytes());
        out.extend((frame.len() as u64).to_le_bytes());
        out.extend(frame);
        out
    }

    fn sample() -> (Vec<(usize, usize)>, Vec<u8>) {
        // Two lines: a half channel of 5, a float of 5, a half of 0 (subsampled away).
        let rows = vec![(5, 2), (5, 4), (0, 2), (5, 2), (5, 4), (3, 2)];
        let n: usize = rows.iter().map(|(n, b)| n * b).sum();
        let packed = (0..n).map(|i| (i * 37 + i / 7) as u8).collect();
        (rows, packed)
    }

    #[test]
    fn round_trips_v1_and_v2() {
        let (rows, packed) = sample();
        for delta in [false, true] {
            let enc = encode(&rows, &packed, delta);
            let mut out = vec![0; packed.len()];
            decompress(&enc, &rows, &mut out).unwrap();
            assert_eq!(out, packed, "delta {delta}");
        }
    }

    #[test]
    fn one_kind_of_sample_is_one_segment() {
        for b in [2, 4] {
            let rows = vec![(7, b), (7, b)];
            let packed: Vec<u8> = (0..14 * b).map(|i| (i * 11) as u8).collect();
            let enc = encode(&rows, &packed, true);
            let mut out = vec![0; packed.len()];
            decompress(&enc, &rows, &mut out).unwrap();
            assert_eq!(out, packed);
        }
    }

    #[test]
    fn malformed_is_refused_not_a_panic() {
        let (rows, packed) = sample();
        let enc = encode(&rows, &packed, true);
        let mut out = vec![0; packed.len()];
        for cut in 0..enc.len() {
            assert!(decompress(&enc[..cut], &rows, &mut out).is_none());
        }
        for i in 0..enc.len() {
            let mut bad = enc.clone();
            bad[i] ^= 0x41;
            let _ = decompress(&bad, &rows, &mut out);
        }
        // Wrong format or flag bits.
        let mut bad = enc.clone();
        bad[8] = 3;
        assert!(decompress(&bad, &rows, &mut out).is_none());
        let mut bad = enc;
        bad[12] = 2;
        assert!(decompress(&bad, &rows, &mut out).is_none());
    }
}
