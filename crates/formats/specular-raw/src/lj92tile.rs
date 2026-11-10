//! Fast lossless JPEG decoding of one raw tile (tiled DNG, Sony lossless ARW), written straight
//! into the tile's rows of the sensor buffer.
//!
//! The common case (predictor 1, no restart interval) runs a dedicated loop: the entropy-coded
//! segment is first copied with `FF 00` unstuffed and cut at the first marker, so the bit reader
//! is a plain 64-bit big-endian reader with a branchless refill; differences come from a
//! 12-bit table whose entries already hold the decoded difference whenever code and extra bits
//! fit. Anything else (restart markers, other predictors) falls back to [`ljpeg::decode_into`]
//! through a reusable scratch tile. Both give identical samples.

use crate::error::{RawError, Result};
use crate::huffman::extend;
use crate::ljpeg::{self, HuffSpec, Lj92Header, Parsed};

const LUT_BITS: u32 = 12;
const LUT_SIZE: usize = 1 << LUT_BITS;
/// Entry flag: the high 16 bits hold the difference and bits 0..6 the total length.
const FULL: u32 = 1 << 6;

/// Difference decoding table for one DHT table.
struct DiffTable {
    /// `FULL | total_len | diff << 16`, or `s << 8 | code_len` when the extra bits do not fit,
    /// or 0 for codes longer than `LUT_BITS`.
    lut: [u32; LUT_SIZE],
    maxcode: [i32; 18],
    valptr: [i32; 17],
    mincode: [i32; 17],
    symbols: [u8; 256],
}

impl DiffTable {
    fn new(spec: &HuffSpec) -> Option<Box<DiffTable>> {
        let total = spec.n;
        if total == 0 || total > 256 {
            return None;
        }
        let mut t = Box::new(DiffTable {
            lut: [0; LUT_SIZE],
            maxcode: [-1; 18],
            valptr: [0; 17],
            mincode: [0; 17],
            symbols: spec.symbols,
        });
        let mut code: u32 = 0;
        let mut k = 0usize;
        for len in 1..=16u32 {
            let n = spec.counts[len as usize - 1] as usize;
            t.valptr[len as usize] = k as i32;
            t.mincode[len as usize] = code as i32;
            for _ in 0..n {
                if code >= (1 << len) {
                    return None; // over-subscribed
                }
                if len <= LUT_BITS {
                    let s = spec.symbols[k] as u32;
                    let shift = LUT_BITS - len;
                    let base = (code << shift) as usize;
                    for (j, e) in t.lut[base..base + (1 << shift)].iter_mut().enumerate() {
                        *e = match s {
                            0 | 17.. => FULL | len,
                            // 32768 wraps to the same sample as -32768 under the sample mask.
                            16 => FULL | len | 0x8000_0000,
                            _ if len + s <= LUT_BITS => {
                                let extra = (j as u32 >> (shift - s)) & ((1 << s) - 1);
                                FULL | (len + s) | ((extend(extra, s) as u32) << 16)
                            }
                            _ => (s << 8) | len,
                        };
                    }
                }
                code += 1;
                k += 1;
            }
            if n > 0 {
                t.maxcode[len as usize] = code as i32 - 1;
            }
            code <<= 1;
        }
        t.maxcode[17] = i32::MAX;
        Some(t)
    }

    /// Decode one difference. Callers refill at the start of each MCU (at most four samples):
    /// a table hit consumes at most `LUT_BITS`, and the other paths refill before and after.
    #[inline(always)]
    fn diff(&self, r: &mut Reader) -> i32 {
        let e = self.lut[(r.buf >> (64 - LUT_BITS)) as usize];
        if e & FULL != 0 {
            r.consume(e & 63);
            return (e as i32) >> 16;
        }
        self.diff_slow(r, e)
    }

    #[inline(always)]
    fn diff_slow(&self, r: &mut Reader, e: u32) -> i32 {
        r.refill();
        let (s, len) = if e != 0 {
            ((e >> 8) & 31, e & 63)
        } else {
            self.long_code(r.buf)
        };
        r.consume(len);
        let d = match s {
            1..=15 => {
                let v = (r.buf >> (64 - s)) as u32;
                r.consume(s);
                extend(v, s)
            }
            16 => 32768,
            _ => 0,
        };
        r.refill();
        d
    }

    /// Category and length of a code longer than `LUT_BITS`; invalid codes take 16 bits and
    /// read as category 0.
    #[cold]
    #[inline(never)]
    fn long_code(&self, buf: u64) -> (u32, u32) {
        let bits = (buf >> 48) as i32;
        for len in LUT_BITS + 1..=16 {
            let code = bits >> (16 - len);
            if code <= self.maxcode[len as usize] {
                let idx = self.valptr[len as usize] + code - self.mincode[len as usize];
                let s = self.symbols.get(idx as usize).copied().unwrap_or(0) as u32;
                return (s, len);
            }
        }
        (0, 16)
    }
}

/// Bit reader over unstuffed bytes followed by zero padding.
struct Reader<'a> {
    d: &'a [u8],
    pos: usize,
    /// Bits left-aligned; at least `nbits` of them are valid.
    buf: u64,
    nbits: u32,
}

impl Reader<'_> {
    /// Leaves at least 56 valid bits buffered.
    #[inline(always)]
    fn refill(&mut self) {
        // The data ends with 16 zero bytes, so past the end the last 8 (zeros) are read.
        let p = self.pos.min(self.d.len().saturating_sub(8));
        let w = match self.d.get(p..p + 8) {
            Some(b) => u64::from_be_bytes(b.try_into().unwrap_or_default()),
            None => 0,
        };
        self.buf |= w >> self.nbits;
        self.pos += ((63 - self.nbits) >> 3) as usize;
        self.nbits |= 56;
    }

    #[inline(always)]
    fn consume(&mut self, n: u32) {
        self.buf <<= n;
        self.nbits -= n;
    }

    fn consumed_bits(&self) -> usize {
        self.pos * 8 - self.nbits as usize
    }
}

/// Per-thread reusable buffers and built tables.
#[derive(Default)]
pub(crate) struct Scratch {
    bytes: Vec<u8>,
    tables: Vec<(HuffSpec, Box<DiffTable>)>,
    row: Vec<u16>,
    row2: Vec<u16>,
    tile: Vec<u16>,
}

const FF_MASK: u64 = 0x8080_8080_8080_8080;
const ONES: u64 = 0x0101_0101_0101_0101;

/// Copy entropy-coded data with `FF 00` turned into `FF`, stopping at the first marker, then
/// pad with zeros. Returns the number of real bytes.
fn unstuff(src: &[u8], dst: &mut Vec<u8>) -> usize {
    dst.clear();
    dst.reserve(src.len() + 16);
    let mut i = 0;
    let mut start = 0;
    loop {
        // Skip eight bytes at a time while none is 0xFF.
        while let Some(b) = src.get(i..i + 8) {
            let x = !u64::from_le_bytes(b.try_into().unwrap_or_default());
            if (x.wrapping_sub(ONES) & !x & FF_MASK) != 0 {
                break;
            }
            i += 8;
        }
        while i < src.len() && src[i] != 0xff {
            i += 1;
        }
        if i >= src.len() {
            dst.extend_from_slice(&src[start..]);
            break;
        }
        // src[i] == 0xFF
        dst.extend_from_slice(&src[start..=i]);
        if src.get(i + 1) == Some(&0) {
            i += 2;
            start = i;
        } else {
            dst.pop();
            break;
        }
    }
    let n = dst.len();
    dst.resize(n + 16, 0);
    n
}

/// How a tile's frame maps onto the tile.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layout {
    /// Samples in the tile's row-major order (DNG): the frame may be shaped differently.
    RowMajor,
    /// Sony lossless ARW: a 4-component frame of half the tile's width and height, one MCU per
    /// 2x2 CFA block (components 0 and 1 on the even row, 2 and 3 on the odd row).
    Quad,
}

/// Decode one tile. `stride` is the tile's full row length in samples, `th` its height, and
/// `rows` the parts of the tile's rows inside the image (each at most `stride` long, missing
/// rows at the bottom edge). Returns whether the data was truncated or corrupt.
pub(crate) fn decode_tile(
    src: &[u8],
    layout: Layout,
    stride: usize,
    th: usize,
    rows: &mut [&mut [u16]],
    s: &mut Scratch,
) -> Result<bool> {
    let Parsed { header: h, tables } = ljpeg::parse(src)?;
    if h.subsampled {
        return Err(RawError::Unsupported(
            "lossless JPEG: subsampled components (sRAW)",
        ));
    }
    let nc = h.components as usize;
    let fw = h.width as usize;
    let fh = h.height as usize;
    let ns = fw * fh * nc;
    match layout {
        Layout::Quad if nc != 4 || fw * 2 != stride || fh * 2 != th => {
            return Err(RawError::Malformed("lossless ARW: tile frame layout"));
        }
        Layout::RowMajor if ns == 0 || ns > stride.saturating_mul(th).saturating_mul(2) => {
            return Err(RawError::Malformed("lossless JPEG: tile frame size"));
        }
        _ => {}
    }
    if h.predictor != 1 || h.restart_interval != 0 {
        return decode_generic(src, &h, layout, stride, rows, s);
    }
    // Build (or reuse) the tables.
    let mut idx = [usize::MAX; 4];
    for c in 0..nc {
        let spec = tables[h.tables[c] as usize]
            .as_ref()
            .ok_or(RawError::Malformed("lossless JPEG: missing Huffman table"))?;
        let found = s.tables.iter().position(|(sp, _)| sp == spec);
        idx[c] = match found {
            Some(i) => i,
            None => {
                let t = DiffTable::new(spec)
                    .ok_or(RawError::Malformed("lossless JPEG: bad Huffman table"))?;
                if s.tables.len() >= 8 {
                    s.tables.clear();
                    // Indices found earlier for this tile are gone: rebuild them all.
                    return decode_tile(src, layout, stride, th, rows, s);
                }
                s.tables.push((*spec, t));
                s.tables.len() - 1
            }
        };
    }
    let Scratch {
        bytes,
        tables,
        row,
        row2,
        ..
    } = s;
    let real = unstuff(src.get(h.data_start..).unwrap_or(&[]), bytes);
    let mut r = Reader {
        d: bytes,
        pos: 0,
        buf: 0,
        nbits: 0,
    };
    let pt = h.point_transform as u32;
    let initial = 1i32 << (h.precision as u32 - pt - 1);
    let mask = (1u32 << (h.precision as u32 - pt).min(16)) as i32 - 1;
    let t = |c: usize| &*tables[idx[c.min(nc - 1)]].1;
    let one = idx[..nc].iter().all(|&i| i == idx[0]);
    let mut short = false;
    match layout {
        Layout::Quad => {
            let ts = [t(0), t(1), t(2), t(3)];
            let mut first = [initial; 4];
            for fr in 0..fh {
                let y = 2 * fr;
                if y >= rows.len() {
                    break;
                }
                let (top, bottom) = rows.split_at_mut(y + 1);
                let even = &mut *top[y];
                match bottom.first_mut() {
                    Some(odd) if even.len() == stride && odd.len() == stride => {
                        if one {
                            quad_row::<true>(&mut r, ts, even, odd, &mut first, mask, pt);
                        } else {
                            quad_row::<false>(&mut r, ts, even, odd, &mut first, mask, pt);
                        }
                    }
                    odd => {
                        row.resize(stride, 0);
                        row2.resize(stride, 0);
                        if one {
                            quad_row::<true>(&mut r, ts, row, row2, &mut first, mask, pt);
                        } else {
                            quad_row::<false>(&mut r, ts, row, row2, &mut first, mask, pt);
                        }
                        let n = even.len().min(stride);
                        even[..n].copy_from_slice(&row[..n]);
                        if let Some(odd) = odd {
                            let n = odd.len().min(stride);
                            odd[..n].copy_from_slice(&row2[..n]);
                        }
                    }
                }
            }
        }
        Layout::RowMajor => {
            let line = fw * nc;
            let need = rows
                .len()
                .saturating_sub(1)
                .saturating_mul(stride)
                .saturating_add(rows.last().map_or(0, |r| r.len()));
            short = ns < need;
            macro_rules! run {
                ($n:literal) => {{
                    let ts: [&DiffTable; $n] = std::array::from_fn(|c| t(c));
                    let mut first = [initial; $n];
                    let lrow = |r: &mut Reader, row: &mut [u16], first: &mut [i32; $n]| {
                        if one {
                            left_row::<$n, true>(r, ts, row, first, mask, pt)
                        } else {
                            left_row::<$n, false>(r, ts, row, first, mask, pt)
                        }
                    };
                    if line == stride {
                        for dst in rows.iter_mut().take(fh) {
                            if dst.len() == stride {
                                lrow(&mut r, dst, &mut first);
                            } else {
                                row.resize(stride, 0);
                                lrow(&mut r, row, &mut first);
                                let n = dst.len().min(stride);
                                dst[..n].copy_from_slice(&row[..n]);
                            }
                        }
                    } else {
                        row.resize(line, 0);
                        let mut k = 0usize; // tile sample index of the frame row's start
                        for _ in 0..fh {
                            if k >= need {
                                break;
                            }
                            lrow(&mut r, row, &mut first);
                            scatter(row, k, stride, rows);
                            k += line;
                        }
                    }
                }};
            }
            match nc {
                1 => run!(1),
                2 => run!(2),
                3 => run!(3),
                _ => run!(4),
            }
        }
    }
    Ok(short || r.consumed_bits() > real * 8)
}

/// Copy one decoded frame row starting at tile sample `k` into the tile rows.
fn scatter(row: &[u16], mut k: usize, stride: usize, rows: &mut [&mut [u16]]) {
    let mut src = row;
    while !src.is_empty() {
        let (y, x) = (k / stride, k % stride);
        let Some(dst) = rows.get_mut(y) else { return };
        let n = src.len().min(stride - x);
        if x < dst.len() {
            let m = n.min(dst.len() - x);
            dst[x..x + m].copy_from_slice(&src[..m]);
        }
        src = &src[n..];
        k += n;
    }
}

#[inline(always)]
fn quad_row<const ONE: bool>(
    r: &mut Reader,
    t: [&DiffTable; 4],
    even: &mut [u16],
    odd: &mut [u16],
    first: &mut [i32; 4],
    mask: i32,
    pt: u32,
) {
    let mut p = *first;
    let mut mcus = even.chunks_exact_mut(2).zip(odd.chunks_exact_mut(2));
    if let Some((e, o)) = mcus.next() {
        quad_mcu::<ONE>(r, t, e, o, &mut p, mask, pt);
        *first = p;
    }
    for (e, o) in mcus {
        quad_mcu::<ONE>(r, t, e, o, &mut p, mask, pt);
    }
}

#[inline(always)]
fn quad_mcu<const ONE: bool>(
    r: &mut Reader,
    t: [&DiffTable; 4],
    e: &mut [u16],
    o: &mut [u16],
    p: &mut [i32; 4],
    mask: i32,
    pt: u32,
) {
    let t = |c: usize| if ONE { t[0] } else { t[c] };
    r.refill();
    p[0] = (p[0] + t(0).diff(r)) & mask;
    p[1] = (p[1] + t(1).diff(r)) & mask;
    p[2] = (p[2] + t(2).diff(r)) & mask;
    p[3] = (p[3] + t(3).diff(r)) & mask;
    e[0] = (p[0] << pt) as u16;
    e[1] = (p[1] << pt) as u16;
    o[0] = (p[2] << pt) as u16;
    o[1] = (p[3] << pt) as u16;
}

#[inline(always)]
fn left_row<const NC: usize, const ONE: bool>(
    r: &mut Reader,
    t: [&DiffTable; NC],
    row: &mut [u16],
    first: &mut [i32; NC],
    mask: i32,
    pt: u32,
) {
    let t = |c: usize| if ONE { t[0] } else { t[c] };
    let mut p = *first;
    let mut chunks = row.chunks_exact_mut(NC);
    if let Some(px) = chunks.next() {
        r.refill();
        for c in 0..NC {
            p[c] = (p[c] + t(c).diff(r)) & mask;
            px[c] = (p[c] << pt) as u16;
        }
        *first = p;
    }
    for px in chunks {
        r.refill();
        for c in 0..NC {
            p[c] = (p[c] + t(c).diff(r)) & mask;
            px[c] = (p[c] << pt) as u16;
        }
    }
}

/// Restart intervals or predictors other than 1: decode the whole frame, then copy.
fn decode_generic(
    src: &[u8],
    h: &Lj92Header,
    layout: Layout,
    stride: usize,
    rows: &mut [&mut [u16]],
    s: &mut Scratch,
) -> Result<bool> {
    let ns = h.width as usize * h.height as usize * h.components as usize;
    // Every sample takes at least one bit of entropy-coded data: a frame header claiming more
    // samples than that would only allocate (up to 34 GB) for zeros.
    if ns > src.len().saturating_mul(8) {
        return Err(RawError::Malformed(
            "lossless JPEG: frame larger than its data",
        ));
    }
    s.tile.clear();
    s.tile.resize(ns, 0);
    let (_, truncated) = ljpeg::decode_into(src, &mut s.tile)?;
    let tmp = &s.tile;
    let mut short = false;
    match layout {
        Layout::Quad => {
            let fw = h.width as usize;
            for (y, dst) in rows.iter_mut().enumerate() {
                let Some(src_row) = tmp.get((y / 2) * fw * 4..(y / 2 + 1) * fw * 4) else {
                    short = true;
                    break;
                };
                let o = (y & 1) * 2;
                for (x, d) in dst.iter_mut().enumerate() {
                    *d = src_row.get((x / 2) * 4 + o + (x & 1)).copied().unwrap_or(0);
                }
            }
        }
        Layout::RowMajor => {
            for (y, dst) in rows.iter_mut().enumerate() {
                match tmp.get(y * stride..y * stride + dst.len()) {
                    Some(s) => dst.copy_from_slice(s),
                    None => short = true,
                }
            }
        }
    }
    Ok(truncated || short)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bits::BitReader;
    use crate::huffman::Huffman;

    fn spec(counts: [u8; 16], syms: &[u8]) -> HuffSpec {
        let mut symbols = [0u8; 256];
        symbols[..syms.len()].copy_from_slice(syms);
        HuffSpec {
            counts,
            symbols,
            n: syms.len(),
        }
    }

    /// The table decoder agrees with the reference Huffman decoder on random bits, including
    /// codes longer than the table, category 16, invalid categories and unused codes.
    #[test]
    fn diff_table_matches_reference() {
        let sony = spec(
            [0, 0, 1, 4, 2, 3, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
            &[7, 8, 6, 5, 9, 4, 10, 3, 11, 2, 0, 1, 12, 13, 14],
        );
        let mut long = [1u8; 16];
        long[15] = 2;
        let long = spec(
            long,
            &[3, 1, 0, 9, 15, 16, 2, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14],
        );
        let mut five = [0u8; 16];
        five[4] = 17;
        let five = spec(five, &(0..=16).collect::<Vec<u8>>());
        // Incomplete code space with an out-of-range category.
        let mut sparse = [0u8; 16];
        sparse[1] = 1;
        sparse[13] = 2;
        let sparse = spec(sparse, &[16, 20, 5]);
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for sp in [sony, long, five, sparse] {
            let reference = Huffman::new(&sp.counts, &sp.symbols[..sp.n], true).unwrap();
            let table = DiffTable::new(&sp).unwrap();
            for bias in [0u64, 1, 3] {
                let mut data: Vec<u8> = (0..4096)
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        // Bias toward 1 bits to reach the long codes.
                        let mut b = seed;
                        for _ in 0..bias {
                            b |= b >> 3;
                        }
                        b as u8
                    })
                    .collect();
                let n = data.len();
                data.resize(n + 16, 0);
                let mut br = BitReader::new(&data[..n], false);
                let mut r = Reader {
                    d: &data,
                    pos: 0,
                    buf: 0,
                    nbits: 0,
                };
                for _ in 0..1500 {
                    r.refill();
                    let a = reference.diff(&mut br);
                    let b = table.diff(&mut r);
                    assert_eq!(a as u16, b as u16);
                }
            }
        }
    }

    #[test]
    fn unstuff_cuts_at_marker() {
        let mut v = Vec::new();
        let src = [
            1, 2, 0xff, 0, 3, 4, 5, 6, 7, 8, 9, 10, 0xff, 0, 0xff, 0xd9, 11,
        ];
        let n = unstuff(&src, &mut v);
        assert_eq!(&v[..n], &[1, 2, 0xff, 3, 4, 5, 6, 7, 8, 9, 10, 0xff]);
        assert_eq!(v.len(), n + 16);
        let n = unstuff(&[0xff], &mut v);
        assert_eq!(n, 0);
        let n = unstuff(&[1, 2, 3], &mut v);
        assert_eq!(&v[..n], &[1, 2, 3]);
    }
}
