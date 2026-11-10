//! Reversing the PNG row filters (Sub, Up, Average, Paeth).
//!
//! Up is a straight vector add. Sub, Average and Paeth depend on the pixel to the left, so they
//! run one pixel at a time. Sub is plain byte adds; Paeth on 3..=8 bytes per pixel makes a whole
//! pixel one `std::simd` vector of 16-bit lanes, like libpng's SSE2 filters. Average runs on all
//! channels of a pixel at once as 16-bit lanes of a `u64` in general-purpose registers (see
//! [`avg_swar`]): its serial chain is three 1-cycle integer ops per pixel, where vector lanes
//! cost 2-cycle latency per op on recent cores. Rows are unfiltered out of place (`src` is the
//! inflated data, `dst` the destination row), so 8-bit RGBA and gray rows can be reconstructed
//! straight into the output image.

use std::ops::Range;
use std::simd::prelude::*;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU8, Ordering};

/// Reverse filter `ft` for one row. `prev` is the previous reconstructed row (all zeros for the
/// first row of an image or pass); `src`, `dst` and `prev` have the same length. Returns `false`
/// for an unknown filter type (then `dst` is left as is).
pub(crate) fn unfilter(ft: u8, bpp: usize, prev: &[u8], src: &[u8], dst: &mut [u8]) -> bool {
    unfilter_from(ft, bpp, prev, src, dst, [0; 8], [0; 8])
}

/// [`unfilter`] for a segment of a row that starts after other pixels: `left` and `upleft` are
/// the reconstructed pixel before the segment in this row and in the previous row (the first
/// `bpp` bytes are used; zeros at the start of a row).
pub(crate) fn unfilter_from(
    ft: u8,
    bpp: usize,
    prev: &[u8],
    src: &[u8],
    dst: &mut [u8],
    left: [u8; 8],
    upleft: [u8; 8],
) -> bool {
    let n = dst.len();
    let (prev, src) = (&prev[..n], &src[..n]);
    match ft {
        0 => dst.copy_from_slice(src),
        1 => match bpp {
            1 => sub::<1>(src, dst, left),
            2 => sub::<2>(src, dst, left),
            3 => sub::<3>(src, dst, left),
            4 => sub::<4>(src, dst, left),
            6 => sub::<6>(src, dst, left),
            _ => sub::<8>(src, dst, left),
        },
        2 => {
            for ((d, &s), &p) in dst.iter_mut().zip(src).zip(prev) {
                *d = s.wrapping_add(p);
            }
        }
        3 => match bpp {
            1 => avg_swar::<1>(prev, src, dst, left),
            2 => avg_swar::<2>(prev, src, dst, left),
            3 => avg_swar::<3>(prev, src, dst, left),
            4 => avg_swar::<4>(prev, src, dst, left),
            6 => avg_swar::<6>(prev, src, dst, left),
            _ => avg_swar::<8>(prev, src, dst, left),
        },
        4 => match bpp {
            1 => paeth_scalar::<1>(prev, src, dst, left, upleft),
            2 => paeth_scalar::<2>(prev, src, dst, left, upleft),
            3 => paeth_simd::<3, 4>(prev, src, dst, left, upleft),
            4 => paeth_simd::<4, 4>(prev, src, dst, left, upleft),
            6 => paeth_simd::<6, 8>(prev, src, dst, left, upleft),
            _ => paeth_simd::<8, 8>(prev, src, dst, left, upleft),
        },
        _ => return false,
    }
    true
}

fn sub<const N: usize>(src: &[u8], dst: &mut [u8], left: [u8; 8]) {
    let mut a = [0u8; N];
    a.copy_from_slice(&left[..N]);
    for (d, s) in dst.chunks_exact_mut(N).zip(src.chunks_exact(N)) {
        for k in 0..N {
            a[k] = a[k].wrapping_add(s[k]);
        }
        d.copy_from_slice(&a);
    }
}

/// Bytes of a row per Average chunk: a multiple of every pixel size.
const AVG_CHUNK: usize = 24 * 32;

/// Average, all pixel sizes. Each byte is `x + (a + b) / 2 = (a + (b + 2x)) / 2` (mod 256), so
/// `t = b + 2x` is computed for a whole chunk first (16-bit, vectorised, independent of `a`),
/// and the serial part per pixel is one add, shift and mask on up to four 16-bit lanes in a
/// `u64` (SWAR): a 3-op dependency chain in general-purpose registers, which beats vector lanes
/// on CPUs with 2-cycle SIMD integer latency and needs a third of the scalar code's
/// instructions. The bit a shift moves into the next lower lane lands in its bit 15, which the
/// mask clears; lanes past the pixel (padding or the next pixel's `t`) are carried but never
/// stored.
fn avg_swar<const N: usize>(prev: &[u8], src: &[u8], dst: &mut [u8], left: [u8; 8]) {
    const M: u64 = 0x00ff_00ff_00ff_00ff;
    let words = N.div_ceil(4);
    let mut a = [0u64; 2];
    for k in 0..N {
        a[k / 4] |= u64::from(left[k]) << (16 * (k % 4));
    }
    let mut t = [0u8; 2 * AVG_CHUNK + 16];
    for ((d, s), p) in dst
        .chunks_mut(AVG_CHUNK)
        .zip(src.chunks(AVG_CHUNK))
        .zip(prev.chunks(AVG_CHUNK))
    {
        for ((tt, &pp), &ss) in t.chunks_exact_mut(2).zip(p).zip(s) {
            tt.copy_from_slice(&(u16::from(pp) + 2 * u16::from(ss)).to_le_bytes());
        }
        for (j, px) in d.chunks_exact_mut(N).enumerate() {
            let o = 2 * N * j;
            for w in 0..words {
                let tv = u64::from_le_bytes(t[o + 8 * w..o + 8 * w + 8].try_into().unwrap());
                a[w] = ((a[w] + tv) >> 1) & M;
                // Lanes 0..3 to bytes 0..3.
                let y = a[w] | (a[w] >> 8);
                let b = ((y & 0xffff) | ((y >> 16) & 0xffff_0000)) as u32;
                let m = (N - 4 * w).min(4);
                px[4 * w..4 * w + m].copy_from_slice(&b.to_le_bytes()[..m]);
            }
        }
    }
}

#[inline(always)]
fn load<const N: usize, const L: usize>(s: &[u8]) -> Simd<i16, L> {
    let mut t = [0u8; L];
    t[..N].copy_from_slice(&s[..N]);
    Simd::<u8, L>::from_array(t).cast()
}

#[inline(always)]
fn store<const N: usize, const L: usize>(v: Simd<i16, L>, d: &mut [u8]) {
    let b: Simd<u8, L> = v.cast();
    d[..N].copy_from_slice(&b.to_array()[..N]);
}

#[inline(always)]
fn paeth_pred(a: i16, b: i16, c: i16) -> i16 {
    let pa = (b - c).abs();
    let pb = (a - c).abs();
    let pc = (a + b - c - c).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn paeth_scalar<const N: usize>(
    prev: &[u8],
    src: &[u8],
    dst: &mut [u8],
    left: [u8; 8],
    upleft: [u8; 8],
) {
    let mut a = [0i16; N];
    let mut c = [0i16; N];
    for k in 0..N {
        a[k] = i16::from(left[k]);
        c[k] = i16::from(upleft[k]);
    }
    for ((d, s), p) in dst
        .chunks_exact_mut(N)
        .zip(src.chunks_exact(N))
        .zip(prev.chunks_exact(N))
    {
        for k in 0..N {
            let b = i16::from(p[k]);
            let v = (i16::from(s[k]) + paeth_pred(a[k], b, c[k])) as u8;
            d[k] = v;
            a[k] = i16::from(v);
            c[k] = b;
        }
    }
}

/// Paeth on whole pixels as vectors of `L` 16-bit lanes (`L` >= `N`). While a full `L`-byte
/// window fits in the row, pixels are loaded and stored `L` bytes at a time: the lanes past
/// the pixel hold the next pixel's bytes and are carried along (lane-wise, so they never mix
/// with the pixel's own lanes); their stored result is overwritten by the next pixel. Only the
/// last pixel(s) take the exact-width path. Whole-array loads and stores are single moves, where
/// `N`-byte `copy_from_slice` through a stack temporary costs a store-forwarding round trip in
/// optimised builds and std's precondition checks in builds with debug assertions.
fn paeth_simd<const N: usize, const L: usize>(
    prev: &[u8],
    src: &[u8],
    dst: &mut [u8],
    left: [u8; 8],
    upleft: [u8; 8],
) {
    #[inline(always)]
    fn step<const L: usize>(
        a: Simd<i16, L>,
        b: Simd<i16, L>,
        c: Simd<i16, L>,
        x: Simd<i16, L>,
    ) -> Simd<i16, L> {
        let pa = (b - c).abs();
        let pb = (a - c).abs();
        let pc = (a + b - c - c).abs();
        let use_a = pa.simd_le(pb) & pa.simd_le(pc);
        let use_b = pb.simd_le(pc);
        let pred = use_a.select(a, use_b.select(b, c));
        (x + pred) & Simd::splat(0xff)
    }
    #[inline(always)]
    fn wide<const L: usize>(s: &[u8]) -> Simd<i16, L> {
        let t: [u8; L] = s[..L].try_into().unwrap();
        Simd::<u8, L>::from_array(t).cast()
    }
    let n = dst.len().min(src.len()).min(prev.len()) / N * N;
    let mut a = load::<N, L>(&left);
    let mut c = load::<N, L>(&upleft);
    // Pixels whose `L`-byte window lies inside the row.
    let fast = if n >= L { (n - L) / N + 1 } else { 0 };
    for i in 0..fast {
        let o = i * N;
        let b = wide::<L>(&prev[o..]);
        a = step(a, b, c, wide::<L>(&src[o..]));
        let d: &mut [u8; L] = (&mut dst[o..o + L]).try_into().unwrap();
        *d = a.cast::<u8>().to_array();
        c = b;
    }
    for o in (fast * N..n).step_by(N) {
        let b = load::<N, L>(&prev[o..]);
        a = step(a, b, c, load::<N, L>(&src[o..]));
        store::<N, L>(a, &mut dst[o..]);
        c = b;
    }
}

// ---------------------------------------------------------------------------------------------
// Wavefront-parallel unfiltering

/// Rows are cut into column segments of at least this many bytes.
const MIN_SEGMENT: usize = 8 * 1024;
/// At most this many segments per row.
const MAX_SEGMENTS: usize = 12;

/// Byte ranges of the column segments a row of `rb` bytes is cut into (whole pixels each), or
/// a single range when the row is too narrow to be worth splitting.
pub(crate) fn segments(rb: usize, bpp: usize, threads: usize) -> Vec<Range<usize>> {
    let n = (rb / MIN_SEGMENT).clamp(1, MAX_SEGMENTS.min(threads.max(1)));
    let px = rb / bpp;
    let mut v = Vec::with_capacity(n);
    let mut start = 0;
    for i in 1..=n {
        let end = if i == n { rb } else { px * i / n * bpp };
        if end > start {
            v.push(start..end);
            start = end;
        }
    }
    v
}

/// The first `bpp` bytes before `at` in `row`, or zeros at the row start.
fn edge(row: &[u8], at: usize, bpp: usize) -> [u8; 8] {
    let mut e = [0u8; 8];
    if at >= bpp {
        e[..bpp].copy_from_slice(&row[at - bpp..at]);
    }
    e
}

/// Rows per wavefront cell: enough that a cell is ~64 KiB of work, so scheduling stays cheap.
fn cell_rows(seg_len: usize) -> usize {
    ((64 << 10) / seg_len.max(1)).clamp(1, 16)
}

struct Wave<'a> {
    bpp: usize,
    rb: usize,
    /// Rows per cell.
    k: usize,
    rows: usize,
    segs: &'a [Range<usize>],
    prev: &'a [u8],
    src: &'a [u8],
    /// Cell (g, c): segment `c` of rows `g * k..`, one slice per row.
    cells: Vec<RwLock<Vec<&'a mut [u8]>>>,
    pending: Vec<AtomicU8>,
}

impl Wave<'_> {
    fn run<'s>(&'s self, scope: &rayon::Scope<'s>, g: usize, c: usize) {
        let n = self.segs.len();
        let groups = self.pending.len() / n;
        let seg = self.segs[c].clone();
        let i = g * n + c;
        let bpp = self.bpp;
        {
            let read = |j: usize| self.cells[j].read().unwrap_or_else(|e| e.into_inner());
            let left = (c > 0).then(|| read(i - 1));
            let up = (g > 0).then(|| read(i - n));
            let upleft = (g > 0 && c > 0).then(|| read(i - n - 1));
            let mut me = self.cells[i].write().unwrap_or_else(|e| e.into_inner());
            let last = |v: &Vec<&mut [u8]>| -> [u8; 8] {
                let r = v.last().map_or(&[][..], |r| &r[..]);
                edge(r, r.len(), bpp)
            };
            for j in 0..me.len() {
                let r = g * self.k + j;
                let row = &self.src[r * (self.rb + 1)..(r + 1) * (self.rb + 1)];
                let lf = left
                    .as_ref()
                    .map_or([0; 8], |l| edge(l[j], l[j].len(), bpp));
                let ul = match (j, &left, &upleft) {
                    (_, None, _) => [0; 8],
                    (0, _, Some(ul)) => last(ul),
                    (0, _, None) => edge(self.prev, seg.start, bpp),
                    (_, Some(l), _) => edge(l[j - 1], l[j - 1].len(), bpp),
                };
                let (above, rest) = me.split_at_mut(j);
                let dst = &mut *rest[0];
                let src = &row[1 + seg.start..1 + seg.end];
                match (above.last(), &up) {
                    (Some(a), _) => unfilter_from(row[0], bpp, a, src, dst, lf, ul),
                    (None, Some(u)) => {
                        let a = u.last().map_or(&[][..], |r| &r[..]);
                        unfilter_from(row[0], bpp, a, src, dst, lf, ul)
                    }
                    (None, None) => {
                        unfilter_from(row[0], bpp, &self.prev[seg.clone()], src, dst, lf, ul)
                    }
                };
            }
        }
        debug_assert!(g * self.k < self.rows);
        if c + 1 < n && self.pending[i + 1].fetch_sub(1, Ordering::AcqRel) == 1 {
            scope.spawn(move |s| self.run(s, g, c + 1));
        }
        if g + 1 < groups && self.pending[i + n].fetch_sub(1, Ordering::AcqRel) == 1 {
            scope.spawn(move |s| self.run(s, g + 1, c));
        }
    }
}

/// Unfilter consecutive rows `src` (each `rb + 1` bytes, filter byte first) into `dst` (`rb`
/// bytes each) on the rayon pool: rows are cut into the column `segs` and grouped into cells
/// of a few rows; cell (g, c) runs once (g - 1, c) and (g, c - 1) are done (a wavefront).
/// `prev` is the row above the first. Byte-identical to unfiltering row by row. Returns the
/// number of rows done: they stop before the first row with an unknown filter type.
pub(crate) fn unfilter_rows(
    bpp: usize,
    rb: usize,
    segs: &[Range<usize>],
    prev: &[u8],
    src: &[u8],
    dst: &mut [u8],
) -> usize {
    let rows = (src.len() / (rb + 1)).min(dst.len() / rb.max(1)).min(
        src.chunks_exact(rb + 1)
            .position(|r| r[0] > 4)
            .unwrap_or(usize::MAX),
    );
    if rows == 0 || rb == 0 {
        return rows;
    }
    let n = segs.len();
    if n == 1 {
        let mut p = prev;
        for (s, d) in src
            .chunks_exact(rb + 1)
            .zip(dst.chunks_exact_mut(rb))
            .take(rows)
        {
            unfilter(s[0], bpp, p, &s[1..], d);
            p = d;
        }
        return rows;
    }
    let k = cell_rows(rb / n);
    let groups = rows.div_ceil(k);
    let mut cells: Vec<Vec<Vec<&mut [u8]>>> = (0..groups)
        .map(|_| (0..n).map(|_| Vec::with_capacity(k)).collect())
        .collect();
    for (r, mut row) in dst.chunks_exact_mut(rb).take(rows).enumerate() {
        for (c, s) in segs.iter().enumerate() {
            let (a, b) = row.split_at_mut(s.len());
            cells[r / k][c].push(a);
            row = b;
        }
    }
    let cells = cells.into_iter().flatten().map(RwLock::new).collect();
    let pending = (0..groups * n)
        .map(|i| AtomicU8::new(u8::from(i >= n) + u8::from(i % n != 0)))
        .collect();
    let w = Wave {
        bpp,
        rb,
        k,
        rows,
        segs,
        prev,
        src,
        cells,
        pending,
    };
    rayon::scope(|s| w.run(s, 0, 0));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(ft: u8, bpp: usize, prev: &[u8], src: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8; src.len()];
        for i in 0..src.len() {
            let a = if i >= bpp { out[i - bpp] as i16 } else { 0 };
            let b = prev[i] as i16;
            let c = if i >= bpp { prev[i - bpp] as i16 } else { 0 };
            let p = match ft {
                0 => 0,
                1 => a,
                2 => b,
                3 => (a + b) >> 1,
                _ => paeth_pred(a, b, c),
            };
            out[i] = (src[i] as i16 + p) as u8;
        }
        out
    }

    /// The wavefront gives the same bytes as row-by-row unfiltering, for every filter mix,
    /// pixel size and segmentation, and stops before an unknown filter type.
    #[test]
    fn wavefront_matches_serial() {
        let mut s = 99u32;
        let mut rnd = || {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            (s >> 24) as u8
        };
        for bpp in [1usize, 2, 3, 4, 6, 8] {
            for (px, rows, threads) in [(5usize, 3usize, 4usize), (3000, 9, 3), (7001, 17, 12)] {
                let rb = px * bpp;
                let prev: Vec<u8> = (0..rb).map(|_| rnd()).collect();
                let mut src: Vec<u8> = (0..rows * (rb + 1)).map(|_| rnd()).collect();
                for r in 0..rows {
                    src[r * (rb + 1)] = rnd() % 5;
                }
                let mut want = vec![0u8; rows * rb];
                let mut p = prev.clone();
                for (s, d) in src.chunks_exact(rb + 1).zip(want.chunks_exact_mut(rb)) {
                    assert!(unfilter(s[0], bpp, &p, &s[1..], d));
                    p.copy_from_slice(d);
                }
                // Narrow segments too, so that small rows split.
                for segs in [segments(rb, bpp, threads), {
                    let k = 4.min(px);
                    (0..k)
                        .map(|i| px * i / k * bpp..px * (i + 1) / k * bpp)
                        .collect()
                }] {
                    let mut got = vec![0u8; rows * rb];
                    assert_eq!(unfilter_rows(bpp, rb, &segs, &prev, &src, &mut got), rows);
                    assert_eq!(got, want, "bpp {bpp} px {px} segs {segs:?}");
                }
                src[(rows / 2) * (rb + 1)] = 9;
                let mut got = vec![0u8; rows * rb];
                let segs = segments(rb, bpp, threads);
                assert_eq!(
                    unfilter_rows(bpp, rb, &segs, &prev, &src, &mut got),
                    rows / 2
                );
                assert_eq!(got[..rows / 2 * rb], want[..rows / 2 * rb]);
            }
        }
    }

    #[test]
    fn matches_reference() {
        let mut s = 7u32;
        let mut rnd = || {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            (s >> 24) as u8
        };
        for bpp in [1usize, 2, 3, 4, 6, 8] {
            for px in [1usize, 2, 7, 64, 101] {
                let n = px * bpp;
                let prev: Vec<u8> = (0..n).map(|_| rnd()).collect();
                let src: Vec<u8> = (0..n).map(|_| rnd()).collect();
                for ft in 0..5u8 {
                    let mut dst = vec![0u8; n];
                    assert!(unfilter(ft, bpp, &prev, &src, &mut dst));
                    assert_eq!(
                        dst,
                        reference(ft, bpp, &prev, &src),
                        "ft {ft} bpp {bpp} px {px}"
                    );
                }
            }
        }
    }

    /// Every filter and pixel size, on random rows of every length up to 40 pixels, also for
    /// segments starting mid-row (non-zero `left` / `upleft`), against the byte-wise reference.
    #[test]
    fn segments_match_reference() {
        let mut s = 0x1234_5678u32;
        let mut rnd = || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s as u8
        };
        for bpp in [1usize, 2, 3, 4, 6, 8] {
            for px in 1..=40usize {
                for _ in 0..8 {
                    let n = px * bpp;
                    let prev: Vec<u8> = (0..n).map(|_| rnd()).collect();
                    let src: Vec<u8> = (0..n).map(|_| rnd()).collect();
                    let cut = usize::from(rnd()) % px * bpp;
                    for ft in 0..5u8 {
                        let want = reference(ft, bpp, &prev, &src);
                        let mut dst = vec![0u8; n];
                        dst[..cut].copy_from_slice(&want[..cut]);
                        let (lf, ul) = (edge(&want, cut, bpp), edge(&prev, cut, bpp));
                        let (p, x) = (&prev[cut..], &src[cut..]);
                        assert!(unfilter_from(ft, bpp, p, x, &mut dst[cut..], lf, ul));
                        assert_eq!(dst, want, "ft {ft} bpp {bpp} px {px} cut {cut}");
                    }
                }
            }
        }
    }
}
