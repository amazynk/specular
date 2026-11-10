//! Chunk decompression: NONE, RLE, ZIPS/ZIP, PXR24, B44/B44A (PIZ is in `piz.rs`, DWAA/DWAB in
//! `dwa.rs`, HTJ2K and LJ2K in `ht.rs`, ZSTD in `zstd.rs`).
//!
//! Every decompressor writes the block's samples in one of two layouts (see [`Layout`]) and
//! fails (returns `None`) instead of panicking on malformed data.

use std::sync::OnceLock;

use crate::deep::DeepScratch;
use crate::dwa::{self, DwaScratch};
use crate::half::h2f;
use crate::header::{Channel, Compression, PixelType};
use crate::piz::{self, PizScratch};

/// Geometry of one channel within a block.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ChanGeom {
    /// Samples per line in the block.
    pub nx: usize,
    /// Sampled lines in the block.
    pub ny: usize,
    pub bytes: usize,
    pub ty: PixelType,
    pub p_linear: bool,
    pub ys: i32,
}

/// A chunk's rectangle (absolute pixel coordinates of the level) and per-channel geometry.
#[derive(Clone, Debug, Default)]
pub(crate) struct BlockGeom {
    pub y0: i32,
    pub h: usize,
    pub chans: Vec<ChanGeom>,
    /// Uncompressed size in bytes.
    pub total: usize,
}

impl BlockGeom {
    /// Whether channel `c` has samples on absolute line `y`.
    #[inline]
    pub fn sampled(&self, c: usize, y: i32) -> bool {
        y.rem_euclid(self.chans[c].ys) == 0
    }
}

/// Number of multiples of `s` in `[a, b]`.
pub(crate) fn num_samples(s: i32, a: i32, b: i32) -> usize {
    let (s, a, b) = (s as i64, a as i64, b as i64);
    (b.div_euclid(s) - (a - 1).div_euclid(s)).max(0) as usize
}

/// Layout of decompressed block data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layout {
    /// OpenEXR's uncompressed layout: for each line, for each channel sampled on that line, the
    /// line's samples (little-endian).
    Lines,
    /// Channel after channel; within a channel, sampled line after sampled line.
    Planar,
}

/// Per-thread scratch buffers.
#[derive(Default)]
pub(crate) struct Scratch {
    pub out: Vec<u8>,
    tmp: Vec<u8>,
    piz: PizScratch,
    dwa: DwaScratch,
    pub deep: DeepScratch,
}

/// Inflate zlib-wrapped `input` into exactly `out.len()` bytes. Kept behind this one function so
/// the backend (zlib-rs today) can be swapped for our own inflater.
pub(crate) fn inflate(input: &[u8], out: &mut [u8]) -> Option<()> {
    let cfg = zlib_rs::InflateConfig { window_bits: 15 };
    let (written, rc) = zlib_rs::decompress_slice(out, input, cfg);
    (rc == zlib_rs::ReturnCode::Ok && written.len() == out.len()).then_some(())
}

/// Decompress one chunk into `s.out`. Returns the layout of `s.out`, or `None` on corrupt data.
/// Data that is not smaller than the uncompressed size is stored raw (OpenEXR's rule); the
/// caller handles that and [`Compression::None`]. `chans` are the part's channels (DWA
/// classifies them by name).
pub(crate) fn decompress(
    c: Compression,
    input: &[u8],
    g: &BlockGeom,
    chans: &[Channel],
    s: &mut Scratch,
) -> Option<Layout> {
    s.out.clear();
    s.out.resize(g.total, 0);
    match c {
        Compression::Rle => {
            s.tmp.clear();
            s.tmp.resize(g.total, 0);
            rle(input, &mut s.tmp)?;
            unpredict_interleave(&mut s.tmp, &mut s.out);
            Some(Layout::Lines)
        }
        Compression::Zips | Compression::Zip => {
            s.tmp.clear();
            s.tmp.resize(g.total, 0);
            inflate(input, &mut s.tmp)?;
            unpredict_interleave(&mut s.tmp, &mut s.out);
            Some(Layout::Lines)
        }
        Compression::Pxr24 => {
            pxr24(input, g, &mut s.tmp, &mut s.out)?;
            Some(Layout::Lines)
        }
        Compression::B44 | Compression::B44a => {
            b44(input, g, &mut s.out)?;
            Some(Layout::Planar)
        }
        Compression::Piz => {
            piz::decompress(input, g, &mut s.piz, &mut s.out)?;
            Some(Layout::Planar)
        }
        Compression::Dwaa | Compression::Dwab => {
            dwa::decompress(input, g, chans, &mut s.dwa, &mut s.piz, &mut s.out)?;
            Some(Layout::Planar)
        }
        Compression::Htj2k256 | Compression::Htj2k32 | Compression::Lj2k => {
            crate::ht::decompress(input, g, chans, &mut s.out, c == Compression::Lj2k)
        }
        Compression::Zstd => {
            crate::zstd::decompress_flat(input, g, &mut s.out)?;
            Some(Layout::Lines)
        }
        _ => None,
    }
}

/// OpenEXR RLE: a signed count byte; negative: `-n` literal bytes follow; else repeat the next
/// byte `n + 1` times. Must fill `out` exactly.
pub(crate) fn rle(input: &[u8], out: &mut [u8]) -> Option<()> {
    let mut i = 0;
    let mut o = 0;
    while i < input.len() {
        let n = input[i] as i8;
        i += 1;
        if n < 0 {
            let n = (-(n as i32)) as usize;
            let src = input.get(i..i + n)?;
            out.get_mut(o..o + n)?.copy_from_slice(src);
            i += n;
            o += n;
        } else {
            let n = n as usize + 1;
            let v = *input.get(i)?;
            out.get_mut(o..o + n)?.fill(v);
            i += 1;
            o += n;
        }
    }
    (o == out.len()).then_some(())
}

/// Undo the ZIP/RLE byte predictor (`t[i] = t[i-1] + t[i] - 128`) in place, then interleave the
/// two halves (`out[2i] = t[i]`, `out[2i+1] = t[(n+1)/2 + i]`).
pub(crate) fn unpredict_interleave(t: &mut [u8], out: &mut [u8]) {
    predictor(t);
    let half = t.len().div_ceil(2);
    let (a, b) = t.split_at(half);
    let mut pairs = out.chunks_exact_mut(2);
    for ((o, &x), &y) in (&mut pairs).zip(a).zip(b) {
        o[0] = x;
        o[1] = y;
    }
    if t.len() % 2 == 1 {
        let n = t.len();
        out[n - 1] = a[half - 1];
    }
}

/// Prefix sum of `d - 128` (mod 256), 16 bytes at a time.
fn predictor(t: &mut [u8]) {
    use std::simd::prelude::*;
    let Some((&first, _)) = t.split_first() else {
        return;
    };
    let mut prev = first;
    let rest = &mut t[1..];
    let mut chunks = rest.chunks_exact_mut(16);
    let bias = u8x16::splat(128);
    for c in &mut chunks {
        let mut v = u8x16::from_slice(c) - bias;
        // Log-step inclusive prefix sum within the vector.
        v += v.shift_elements_right::<1>(0u8);
        v += v.shift_elements_right::<2>(0u8);
        v += v.shift_elements_right::<4>(0u8);
        v += v.shift_elements_right::<8>(0u8);
        v += u8x16::splat(prev);
        v.copy_to_slice(c);
        prev = c[15];
    }
    for b in chunks.into_remainder() {
        prev = prev.wrapping_add(*b).wrapping_sub(128);
        *b = prev;
    }
}

/// PXR24: zlib, then per line and channel, byte planes of horizontal differences (HALF: 2,
/// FLOAT: 3 (24-bit), UINT: 4).
fn pxr24(input: &[u8], g: &BlockGeom, tmp: &mut Vec<u8>, out: &mut [u8]) -> Option<()> {
    let mut packed = 0usize;
    for y in g.y0..g.y0 + g.h as i32 {
        for (c, ch) in g.chans.iter().enumerate() {
            if g.sampled(c, y) {
                packed += ch.nx
                    * match ch.ty {
                        PixelType::Half => 2,
                        PixelType::Float => 3,
                        PixelType::Uint => 4,
                    };
            }
        }
    }
    tmp.clear();
    tmp.resize(packed, 0);
    inflate(input, tmp)?;
    let mut src = 0usize;
    let mut dst = 0usize;
    for y in g.y0..g.y0 + g.h as i32 {
        for (c, ch) in g.chans.iter().enumerate() {
            if !g.sampled(c, y) {
                continue;
            }
            let n = ch.nx;
            match ch.ty {
                PixelType::Half => {
                    let (p0, p1) = tmp[src..src + 2 * n].split_at(n);
                    let mut px = 0u16;
                    for ((o, &a), &b) in out[dst..dst + 2 * n].chunks_exact_mut(2).zip(p0).zip(p1) {
                        px = px.wrapping_add(((a as u16) << 8) | b as u16);
                        o.copy_from_slice(&px.to_le_bytes());
                    }
                    src += 2 * n;
                    dst += 2 * n;
                }
                PixelType::Float => {
                    let p = &tmp[src..src + 3 * n];
                    let (p0, p) = p.split_at(n);
                    let (p1, p2) = p.split_at(n);
                    let mut px = 0u32;
                    for (((o, &a), &b), &c) in out[dst..dst + 4 * n]
                        .chunks_exact_mut(4)
                        .zip(p0)
                        .zip(p1)
                        .zip(p2)
                    {
                        px = px.wrapping_add(
                            ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8),
                        );
                        o.copy_from_slice(&px.to_le_bytes());
                    }
                    src += 3 * n;
                    dst += 4 * n;
                }
                PixelType::Uint => {
                    let p = &tmp[src..src + 4 * n];
                    let (p0, p) = p.split_at(n);
                    let (p1, p) = p.split_at(n);
                    let (p2, p3) = p.split_at(n);
                    let mut px = 0u32;
                    for ((((o, &a), &b), &c), &d) in out[dst..dst + 4 * n]
                        .chunks_exact_mut(4)
                        .zip(p0)
                        .zip(p1)
                        .zip(p2)
                        .zip(p3)
                    {
                        px = px.wrapping_add(u32::from_be_bytes([a, b, c, d]));
                        o.copy_from_slice(&px.to_le_bytes());
                    }
                    src += 4 * n;
                    dst += 4 * n;
                }
            }
        }
    }
    (dst == out.len()).then_some(())
}

/// B44 / B44A: HALF channels in 4x4 blocks of 14 bytes (or 3 bytes for flat B44A blocks),
/// other channels raw. Output is planar.
fn b44(input: &[u8], g: &BlockGeom, out: &mut [u8]) -> Option<()> {
    let mut i = 0usize;
    let mut base = 0usize;
    for ch in &g.chans {
        let plane = ch.nx * ch.ny * ch.bytes;
        let dst = &mut out[base..base + plane];
        base += plane;
        if ch.ty != PixelType::Half {
            dst.copy_from_slice(input.get(i..i + plane)?);
            i += plane;
            continue;
        }
        let (nx, ny) = (ch.nx, ch.ny);
        let lin = if ch.p_linear { Some(log_table()) } else { None };
        for by in (0..ny).step_by(4) {
            for bx in (0..nx).step_by(4) {
                let flag = *input.get(i + 2)?;
                let mut s = if flag >= 13 << 2 {
                    let b = input.get(i..i + 3)?;
                    i += 3;
                    unpack3(b)
                } else {
                    let b = input.get(i..i + 14)?;
                    i += 14;
                    unpack14(b)
                };
                if let Some(t) = lin {
                    for v in &mut s {
                        *v = t[*v as usize];
                    }
                }
                for r in 0..4.min(ny - by) {
                    let row = (by + r) * nx;
                    for c in 0..4.min(nx - bx) {
                        let o = (row + bx + c) * 2;
                        dst[o..o + 2].copy_from_slice(&s[r * 4 + c].to_le_bytes());
                    }
                }
            }
        }
    }
    Some(())
}

fn unpack14(b: &[u8]) -> [u16; 16] {
    let b: [u16; 14] = std::array::from_fn(|i| b[i] as u16);
    let shift = b[2] >> 2;
    let bias = 0x20u16 << shift;
    let d = |v: u16| (v & 0x3f) << shift;
    let mut s = [0u16; 16];
    s[0] = (b[0] << 8) | b[1];
    s[4] = s[0]
        .wrapping_add(d((b[2] << 4) | (b[3] >> 4)))
        .wrapping_sub(bias);
    s[8] = s[4]
        .wrapping_add(d((b[3] << 2) | (b[4] >> 6)))
        .wrapping_sub(bias);
    s[12] = s[8].wrapping_add(d(b[4])).wrapping_sub(bias);
    s[1] = s[0].wrapping_add(d(b[5] >> 2)).wrapping_sub(bias);
    s[5] = s[4]
        .wrapping_add(d((b[5] << 4) | (b[6] >> 4)))
        .wrapping_sub(bias);
    s[9] = s[8]
        .wrapping_add(d((b[6] << 2) | (b[7] >> 6)))
        .wrapping_sub(bias);
    s[13] = s[12].wrapping_add(d(b[7])).wrapping_sub(bias);
    s[2] = s[1].wrapping_add(d(b[8] >> 2)).wrapping_sub(bias);
    s[6] = s[5]
        .wrapping_add(d((b[8] << 4) | (b[9] >> 4)))
        .wrapping_sub(bias);
    s[10] = s[9]
        .wrapping_add(d((b[9] << 2) | (b[10] >> 6)))
        .wrapping_sub(bias);
    s[14] = s[13].wrapping_add(d(b[10])).wrapping_sub(bias);
    s[3] = s[2].wrapping_add(d(b[11] >> 2)).wrapping_sub(bias);
    s[7] = s[6]
        .wrapping_add(d((b[11] << 4) | (b[12] >> 4)))
        .wrapping_sub(bias);
    s[11] = s[10]
        .wrapping_add(d((b[12] << 2) | (b[13] >> 6)))
        .wrapping_sub(bias);
    s[15] = s[14].wrapping_add(d(b[13])).wrapping_sub(bias);
    for v in &mut s {
        *v = if *v & 0x8000 != 0 { *v & 0x7fff } else { !*v };
    }
    s
}

fn unpack3(b: &[u8]) -> [u16; 16] {
    let v = ((b[0] as u16) << 8) | b[1] as u16;
    let v = if v & 0x8000 != 0 { v & 0x7fff } else { !v };
    [v; 16]
}

/// B44 `pLinear` decode table, OpenEXR's `logTable`: `8 ln(x)` for positive finite `x`, `-inf`
/// for zero, else 0. (The encoder applies `expTable` before quantising.)
fn log_table() -> &'static [u16; 65536] {
    static T: OnceLock<Box<[u16; 65536]>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Box::new([0u16; 65536]);
        for (i, v) in t.iter_mut().enumerate() {
            let f = h2f(i as u16);
            *v = if f == 0.0 {
                0xfc00
            } else if f.is_finite() && f > 0.0 {
                ((8.0 * (f as f64).ln()) as f16).to_bits()
            } else {
                0
            };
        }
        // OpenEXR's table was generated with single-precision `log`, which lands on the other
        // side of a rounding midpoint for this one input.
        t[0x1d78] = 0xd13c;
        t
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predictor_matches_scalar() {
        for n in [0usize, 1, 2, 15, 16, 17, 33, 100] {
            let src: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            let mut a = src.clone();
            predictor(&mut a);
            let mut b = src.clone();
            for i in 1..n {
                b[i] = b[i - 1].wrapping_add(b[i]).wrapping_sub(128);
            }
            assert_eq!(a, b, "n={n}");
        }
    }

    #[test]
    fn rle_basic() {
        let mut out = [0u8; 6];
        rle(&[2, 7, (-3i8) as u8, 1, 2, 3], &mut out).unwrap();
        assert_eq!(out, [7, 7, 7, 1, 2, 3]);
        assert!(rle(&[6, 7], &mut out).is_none());
        assert!(rle(&[4, 7], &mut out).is_none());
        assert!(rle(&[(-9i8) as u8, 1], &mut out).is_none());
    }

    #[test]
    fn samples() {
        assert_eq!(num_samples(1, -3, 4), 8);
        assert_eq!(num_samples(2, 0, 7), 4);
        assert_eq!(num_samples(2, 1, 7), 3);
        assert_eq!(num_samples(2, -3, -1), 1);
    }
}
