//! Apple ProRes RAW (QuickTime `aprn` "ProRes RAW" and `aprh` "ProRes RAW HQ"): Bayer
//! sensor frames.
//!
//! Apple has not published the coding. This is an independent implementation from the
//! behaviour of the reverse-engineered decoder in FFmpeg 8 (studied for technique only, no
//! code or tables taken), checked sample for sample against `ffmpeg -c:v prores_raw` output
//! (see `tests/raw_oracle.rs`).
//!
//! A frame is `be32 size`, `"prrf"`, `be16 header length`, the header (size, Bayer layout,
//! sensel range, white-balance factors, camera → XYZ matrix, gain, flags, an optional
//! weight matrix and linearisation curve), a table of `be16` tile sizes, then the tiles. The
//! picture is cut into rows of 16 lines, each into tiles of 2^k × 16 columns (k from the
//! header's alignment down to 0, the widest that fit first). A tile holds the four Bayer
//! components (the second-row green, the first-row green, blue, red, in that order), each
//! a row of 8×8 blocks of the half-resolution plane, coded like ProRes slices: the DC terms
//! of all blocks first (the first absolute, the rest as differences with adaptive Rice /
//! exponential Golomb codes), then the AC terms interleaved across the blocks in the
//! interlaced scan order as (level run, zero run) pairs with adaptive codebooks. Blocks are
//! dequantised by the weight matrix times the tile's scale and inverse transformed with the
//! 12-bit ProRes transform ([`crate::idct`]).
//!
//! The transform's 12-bit output is a compressed code: the linearisation curve (eight
//! control points over the code range, a default of powers of two when the header has none)
//! turns it into linear 16-bit sensor values, with black at 256 and white at the header's
//! sensel range above it. [`Output::Coded`] gives the code itself, as FFmpeg 8.0 outputs it
//! (12 bits shifted to 16).

use rayon::prelude::*;

use crate::bits::Bits;
use crate::idct::{self, Arith};
use crate::{Result, bad};

/// Black of the linear output (the header gives the sensel range above it).
pub const BLACK: u16 = 0x100;

/// What [`decode`] writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// Linear 16-bit sensor values (the linearisation curve applied).
    Linear,
    /// The transform's 12-bit code shifted to 16 bits, before linearisation.
    Coded,
    /// [`Output::Coded`] with FFmpeg 8.0's reading of a component's last bits: it goes on
    /// decoding past the end of the component's data into the bytes after it, so a few
    /// blocks gain a stray coefficient. Only for comparing against that version (Apple's
    /// decoder, and FFmpeg after 8.0, stop at the end).
    #[doc(hidden)]
    CodedFfmpeg80,
}

/// The header of one ProRes RAW frame.
#[derive(Clone, Debug, PartialEq)]
pub struct RawHeader {
    pub version: u8,
    /// The encoder's four-character identifier (`appl`, `atm0` for Atomos, `peac` for
    /// Panasonic…).
    pub vendor: [u8; 4],
    pub width: u32,
    pub height: u32,
    /// Margins to drop after demosaicing: left, right, top, bottom.
    pub crop: [u8; 4],
    /// 0 RGGB, 1 GRBG, 2 GBRG, 3 BGGR (only RGGB is known in files).
    pub bayer: u8,
    /// White of the linear output: the sensel range above [`BLACK`].
    pub white: u32,
    /// White-balance multipliers of red and blue (green 1).
    pub wb: [f32; 2],
    /// White-balanced camera RGB → CIE XYZ (D65), row-major.
    pub matrix: [[f32; 3]; 3],
    /// Gain after the matrix (exposure, informational).
    pub gain: f32,
    /// The white balance's colour temperature in kelvin (0: not given).
    pub cct: u16,
    /// Tiles are 2^`align` columns of 16 wide, the right edge in smaller powers of two.
    align: u8,
    qmat: [u8; 64],
    curve: [u16; 8],
    /// The curve came with the frame (else the default).
    pub has_curve: bool,
    /// Offset of the tile size table.
    tiles_at: usize,
}

impl RawHeader {
    /// The Bayer layout's name.
    pub fn cfa_name(&self) -> &'static str {
        match self.bayer {
            1 => "GRBG",
            2 => "GBRG",
            3 => "BGGR",
            _ => "RGGB",
        }
    }
}

/// The default linearisation curve: each eighth of the code range spans twice the linear
/// range of the one below (the first two the same), from 0 to 2^15 and on to 2^16.
const DEFAULT_CURVE: [u16; 8] = {
    let mut c = [0u16; 8];
    let mut i = 1;
    while i < 8 {
        c[i] = 256 << i;
        i += 1;
    }
    c
};

fn be16(b: &[u8], o: usize) -> u16 {
    match b.get(o..o + 2) {
        Some(s) => u16::from_be_bytes([s[0], s[1]]),
        None => 0,
    }
}

fn be32(b: &[u8], o: usize) -> u32 {
    match b.get(o..o + 4) {
        Some(s) => u32::from_be_bytes([s[0], s[1], s[2], s[3]]),
        None => 0,
    }
}

fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_bits(be32(b, o))
}

/// Reads a frame's header (the whole movie sample).
pub fn raw_header(frame: &[u8]) -> Result<RawHeader> {
    /// The largest side read (8K and a margin).
    const MAX_SIDE: u32 = 16384;
    if frame.len() < 10 || frame.get(4..8) != Some(b"prrf") {
        return Err(bad("not a ProRes RAW frame"));
    }
    if be32(frame, 0) as usize > frame.len() {
        return Err(bad("ProRes RAW frame is cut short"));
    }
    let len = usize::from(be16(frame, 8));
    if len < 62 || frame.len() < 8 + len {
        return Err(bad("ProRes RAW frame header is cut short"));
    }
    // The header without its length field; fields past its end read as zero.
    let h = &frame[10..8 + len];
    let version = h[1];
    if version > 1 {
        return Err(crate::Error::Unsupported(format!(
            "ProRes RAW frame version {version}"
        )));
    }
    let width = u32::from(be16(h, 6));
    let height = u32::from(be16(h, 8));
    if width == 0 || height == 0 || width & 1 != 0 || height & 1 != 0 {
        return Err(bad("ProRes RAW frame size"));
    }
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(crate::Error::Unsupported(format!(
            "ProRes RAW frame of {width}×{height}"
        )));
    }
    let flags = be16(h, 68);
    let align = ((flags >> 1) & 7) as u8;
    if align > 4 {
        return Err(bad("ProRes RAW tile alignment"));
    }
    let mut at = 70;
    let mut qmat = [1u8; 64];
    if flags & 1 != 0 {
        for (i, q) in qmat.iter_mut().enumerate() {
            *q = h.get(at + i).copied().unwrap_or(0);
        }
        at += 64;
    }
    let has_curve = flags & 0x10 != 0;
    let curve = if has_curve {
        std::array::from_fn(|i| be16(h, at + 2 * i))
    } else {
        DEFAULT_CURVE
    };
    let mut matrix = [[0f32; 3]; 3];
    for (r, row) in matrix.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            *v = f32_at(h, 26 + 4 * (3 * r + c));
        }
    }
    Ok(RawHeader {
        version,
        vendor: [h[2], h[3], h[4], h[5]],
        width,
        height,
        crop: [h[10], h[11], h[12], h[13]],
        bayer: (be16(h, 14) & 3) as u8,
        white: u32::from(be16(h, 16)) + u32::from(BLACK),
        wb: [f32_at(h, 18), f32_at(h, 22)],
        matrix,
        gain: f32_at(h, 62),
        cct: be16(h, 66),
        align,
        qmat,
        curve,
        has_curve,
        tiles_at: 8 + len,
    })
}

/// One tile: its place, its width in blocks (log2) and its bytes.
struct Tile {
    x: u32,
    log2_blocks: u32,
    data: std::ops::Range<usize>,
}

/// The tiles of each 16-line row, in coded order.
fn tiles(frame: &[u8], h: &RawHeader) -> Result<Vec<Vec<Tile>>> {
    let cols = h.width.div_ceil(16);
    let rows = h.height.div_ceil(16);
    // Per row: the aligned tiles, then one of each smaller power of two the rest holds.
    let per_row = (cols >> h.align) + (cols & ((1 << h.align) - 1)).count_ones();
    let count = per_row as usize * rows as usize;
    let mut at = h.tiles_at;
    let mut data_at = at + 2 * count;
    if data_at > frame.len() {
        return Err(bad("ProRes RAW tile table is cut short"));
    }
    let mut out = Vec::with_capacity(rows as usize);
    for _ in 0..rows {
        let mut row = Vec::with_capacity(per_row as usize);
        let mut x = 0;
        let mut rest = cols;
        for e in (0..=u32::from(h.align)).rev() {
            let unit = 1 << e;
            while unit <= rest {
                let size = usize::from(be16(frame, at));
                at += 2;
                let end = data_at + size;
                if end > frame.len() {
                    return Err(bad("ProRes RAW tile past the frame's end"));
                }
                row.push(Tile {
                    x: x * 16,
                    log2_blocks: e,
                    data: data_at..end,
                });
                data_at = end;
                x += unit;
                rest -= unit;
            }
        }
        out.push(row);
    }
    Ok(out)
}

/// Rice / exponential Golomb parameters: the prefix length up to which the code is Rice,
/// the exponential Golomb order past it, the Rice order.
#[derive(Clone, Copy)]
struct Code {
    switch: u32,
    exp: u32,
    rice: u32,
}

const fn code(switch: u32, exp: u32, rice: u32) -> Code {
    Code { switch, exp, rice }
}

/// The first DC term's code.
const FIRST_DC: Code = code(2, 11, 12);
/// The code of the DC difference that starts each run of 16 blocks.
const DC_RESTART: Code = code(0, 6, 4);

/// The code of a DC difference, from the previous difference's code number `v`.
fn dc_code(v: u32) -> Code {
    match v.div_ceil(2) {
        0 => code(0, 1, 0),
        1 => code(0, 2, 1),
        2 => code(0, 3, 2),
        3..=5 => code(0, 3, 3),
        6..=11 => code(0, 4, 4),
        _ => code(0, 7, 6),
    }
}

/// The code of an AC level, from the previous level's code number.
fn level_code(v: u32) -> Code {
    match v {
        0 => code(0, 0, 0),
        1 => code(2, 1, 1),
        2..=3 => code(1, 1, 1),
        4..=6 => code(2, 2, 2),
        7..=9 => code(1, 2, 2),
        10..=17 => code(2, 3, 3),
        18..=19 => code(1, 3, 3),
        20..=41 => code(2, 4, 4),
        42..=93 => code(3, 5, 5),
        _ => code(1, 6, 6),
    }
}

/// The code of a zero run, from the previous run.
fn run_code(v: u32) -> Code {
    match v {
        0 => code(2, 0, 0),
        1 => code(1, 0, 0),
        2..=3 => code(0, 0, 0),
        4..=5 => code(2, 1, 1),
        6..=7 => code(1, 1, 1),
        8..=9 => code(0, 1, 1),
        10..=11 => code(0, 2, 1),
        12 => code(2, 2, 2),
        13..=24 => code(0, 2, 2),
        25..=26 => code(0, 3, 2),
        _ => code(0, 4, 4),
    }
}

/// The code of the length of a run of non-zero levels, from the level before the run.
fn length_code(v: u32) -> Code {
    match v {
        0 => code(1, 0, 0),
        1 => code(1, 1, 1),
        2..=3 => code(2, 2, 2),
        4..=5 => code(1, 2, 2),
        6..=7 => code(4, 3, 3),
        8..=13 => code(2, 3, 3),
        _ => code(0, 3, 3),
    }
}

/// The first level code, zero-run code and level-run code of a component.
const LEVEL_START: Code = code(0, 3, 1);
const RUN_START: Code = code(0, 0, 0);
const LENGTH_START: Code = code(0, 4, 2);

/// One code number, or `None` where the component's data ends: the next 32 bits (zeros
/// past the end) are all zero, or a code would be longer than 32 bits.
#[inline(always)]
fn value(b: &mut Bits, c: Code) -> Option<u32> {
    if b.left() == 0 {
        return None;
    }
    let w = b.peek32();
    if w == 0 {
        return None;
    }
    let q = w.leading_zeros();
    if q <= c.switch {
        let low = if c.rice == 0 {
            0
        } else {
            (w << (q + 1)) >> (32 - c.rice)
        };
        b.read(q + 1 + c.rice);
        return Some((q << c.rice) + low);
    }
    let bits = c.exp + 2 * q - c.switch;
    if bits > 32 {
        return None;
    }
    b.read(bits);
    Some(
        (w >> (32 - bits))
            .wrapping_add((c.switch + 1) << c.rice)
            .wrapping_sub(1 << c.exp),
    )
}

/// The coefficients of one component of a tile: `1 << log2` blocks, each transposed
/// (`[column * 8 + row]`, the transform's order), as 16-bit values.
fn coefficients(data: &[u8], log2: u32, blocks: &mut [[i32; 64]]) {
    for b in blocks.iter_mut() {
        b.fill(0);
    }
    let n_blocks = 1usize << log2;
    let mask = n_blocks - 1;
    let n_codes = 64 * n_blocks;
    let mut b = Bits::new(data);
    // The stored DC is one above the running sum (the transform's rounding expects it).
    let store = |v: i32| i32::from((v + 1) as i16);
    let Some(mut dc) = value(&mut b, FIRST_DC) else {
        return;
    };
    let mut prev = ((dc >> 1) as i32) ^ -((dc & 1) as i32);
    blocks[0][0] = store(prev);
    let mut negative = false;
    for (n, block) in blocks.iter_mut().enumerate().take(n_blocks).skip(1) {
        let c = if n & 15 == 1 { DC_RESTART } else { dc_code(dc) };
        let Some(v) = value(&mut b, c) else {
            return;
        };
        dc = v;
        // The low bit flips the sign of the step from the previous one's.
        negative ^= v & 1 != 0;
        let step = v.div_ceil(2) as i32;
        let add = if negative { -step } else { step };
        negative = add < 0;
        prev += add;
        block[0] = store(prev);
    }
    let mut level_c = LEVEL_START;
    let mut run_c = RUN_START;
    let mut length_c = LENGTH_START;
    let mut put = |n: usize, level: u32, negative: bool| {
        let pos = usize::from(crate::slice::INTERLACED_T[n >> log2]);
        let v = level as i32 + 1;
        blocks[n & mask][pos] = i32::from((if negative { -v } else { v }) as i16);
    };
    let mut n = n_blocks;
    while n < n_codes {
        let Some(len) = value(&mut b, length_c) else {
            return;
        };
        for _ in 0..len {
            let Some(level) = value(&mut b, level_c) else {
                return;
            };
            level_c = level_code(level);
            let neg = b.bit();
            put(n, level, neg);
            n += 1;
            if n == n_codes {
                return;
            }
        }
        let Some(run) = value(&mut b, run_c) else {
            return;
        };
        run_c = run_code(run);
        n += run as usize + 1;
        if n >= n_codes {
            return;
        }
        let Some(level) = value(&mut b, level_c) else {
            return;
        };
        level_c = level_code(level);
        length_c = length_code(level);
        let neg = b.bit();
        put(n, level, neg);
        n += 1;
    }
}

/// FFmpeg 8.0's end-of-component behaviour, for the oracle comparison only.
///
/// FFmpeg 8.0 decodes a component until its bit count runs out, not where the coded data
/// ends: its cached bit reader loads whole 64- and 32-bit words, so the bits it sees past
/// the end are the bytes after the component up to the last word it loaded, then zeros;
/// 32 zero bits read as the number 0 (reading nothing). Code numbers are 16-bit.
mod ffmpeg80 {
    use super::*;

    /// The cached big-endian reader's state.
    struct Reader<'a> {
        data: &'a [u8],
        /// The component's size in bytes (refills stop at it).
        end: usize,
        ptr: usize,
        bits: u64,
        /// Valid bits, wrapping below zero like the reader's unsigned count.
        valid: u32,
    }

    impl<'a> Reader<'a> {
        fn new(data: &'a [u8], size: usize) -> Reader<'a> {
            let mut r = Reader {
                data,
                end: size,
                ptr: 0,
                bits: 0,
                valid: 0,
            };
            r.refill64();
            r
        }
        fn byte(&self, i: usize) -> u64 {
            u64::from(self.data.get(i).copied().unwrap_or(0))
        }
        fn refill64(&mut self) -> bool {
            if self.ptr >= self.end {
                return false;
            }
            self.bits = (0..8).fold(0, |a, k| a << 8 | self.byte(self.ptr + k));
            self.ptr += 8;
            self.valid = 64;
            true
        }
        fn refill32(&mut self) {
            if self.ptr >= self.end {
                return;
            }
            let w = (0..4).fold(0, |a, k| a << 8 | self.byte(self.ptr + k));
            self.bits |= w << (32 - self.valid);
            self.ptr += 4;
            self.valid += 32;
        }
        fn peek32(&mut self) -> u32 {
            if 32 > self.valid {
                self.refill32();
            }
            (self.bits >> 32) as u32
        }
        fn skip_remaining(&mut self, n: u32) {
            self.bits = self.bits.checked_shl(n).unwrap_or(0);
            self.valid = self.valid.wrapping_sub(n);
        }
        fn skip(&mut self, n: u32) {
            if n < self.valid {
                self.skip_remaining(n);
            } else {
                let n = n - self.valid;
                self.bits = 0;
                self.valid = 0;
                self.refill64();
                if n > 0 {
                    self.skip_remaining(n);
                }
            }
        }
        fn bit(&mut self) -> bool {
            if self.valid == 0 && !self.refill64() {
                return false;
            }
            let v = self.bits >> 63;
            self.skip_remaining(1);
            v == 1
        }
        /// Bits left, as the reader's (32-bit) count.
        fn left(&self) -> i32 {
            (self.end as i64 * 8 - self.ptr as i64 * 8 + i64::from(self.valid)) as i32
        }
    }

    fn value(b: &mut Reader, c: Code) -> i32 {
        let w = b.peek32();
        if w == 0 {
            return 0;
        }
        let q = w.leading_zeros();
        let v = if q <= c.switch {
            let low = if c.rice == 0 {
                0
            } else {
                (w << (q + 1)) >> (32 - c.rice)
            };
            b.skip(q + 1 + c.rice);
            (q << c.rice) + low
        } else {
            let bits = (c.exp + 2 * q - c.switch).min(32);
            b.skip(bits);
            (w >> (32 - bits))
                .wrapping_add((c.switch + 1) << c.rice)
                .wrapping_sub(1 << c.exp)
        };
        i32::from(v as i16)
    }

    /// A table index from a 16-bit code number (negative ones read the first entry).
    fn idx(v: i32) -> u32 {
        v.max(0) as u32
    }

    /// [`super::coefficients`] the FFmpeg 8.0 way: `data` runs on past the component's
    /// `size` bytes.
    pub(super) fn coefficients(data: &[u8], size: usize, log2: u32, blocks: &mut [[i32; 64]]) {
        for b in blocks.iter_mut() {
            b.fill(0);
        }
        let n_blocks = 1usize << log2;
        let mask = n_blocks - 1;
        let n_codes = 64 * n_blocks;
        let mut b = Reader::new(data, size);
        let store = |v: i32| i32::from((v + 1) as i16);
        let mut dc = value(&mut b, FIRST_DC);
        let mut prev = (dc >> 1) ^ -(dc & 1);
        blocks[0][0] = store(prev);
        let mut negative = 0i32;
        for (n, block) in blocks.iter_mut().enumerate().take(n_blocks).skip(1) {
            if b.left() <= 0 {
                break;
            }
            let c = if n & 15 == 1 {
                DC_RESTART
            } else {
                dc_code(idx(dc))
            };
            dc = value(&mut b, c);
            negative ^= dc & 1;
            let step = (dc + 1) >> 1;
            let add = i32::from(((-negative ^ step) + negative) as i16);
            negative = i32::from(add < 0);
            prev += add;
            block[0] = store(prev);
        }
        let (mut level_c, mut run_c, mut length_c) = (LEVEL_START, RUN_START, LENGTH_START);
        let mut put = |n: usize, level: i32, negative: bool| {
            let pos = usize::from(crate::slice::INTERLACED_T[n >> log2]);
            let v = level + 1;
            blocks[n & mask][pos] = i32::from((if negative { -v } else { v }) as i16);
        };
        let mut n = n_blocks as i64;
        let codes = n_codes as i64;
        while n <= codes {
            if b.left() <= 0 {
                break;
            }
            let len = value(&mut b, length_c);
            for i in 0..i64::from(len.max(0)) {
                if b.left() <= 0 || n + i >= codes {
                    break;
                }
                let level = value(&mut b, level_c);
                level_c = level_code(idx(level));
                let neg = b.bit();
                put((n + i) as usize, level, neg);
            }
            n += i64::from(len);
            if n >= codes {
                break;
            }
            let run = value(&mut b, run_c);
            run_c = run_code(idx(run));
            n += i64::from(run) + 1;
            if n >= codes || n < 0 || b.left() <= 0 {
                break;
            }
            let level = value(&mut b, level_c);
            let neg = b.bit();
            put(n as usize, level, neg);
            level_c = level_code(idx(level));
            length_c = length_code(idx(level));
            n += 1;
        }
    }
}

/// Linear value of a 16-bit code through the curve's eight segments of 8192 codes.
#[inline(always)]
fn linear(u: u32, curve: &[u16; 8]) -> u16 {
    let seg = (u >> 13) as usize;
    let frac = u & 0x1fff;
    let lo = u32::from(curve[seg]);
    // Past the last point the curve runs on to 2^16 (the step is taken modulo 2^16).
    let hi = curve.get(seg + 1).map_or(0, |&v| u32::from(v));
    let o = (lo * 8192 + (hi.wrapping_sub(lo) & 0xffff) * frac + 4096) >> 13;
    o.min(0xffff) as u16
}

/// Decodes one 16-line row of tiles into `out` (`lines` rows of `width` samples).
fn decode_row(
    frame: &[u8],
    h: &RawHeader,
    row: &[Tile],
    out: &mut [u16],
    output: Output,
) -> Result<()> {
    let width = h.width as usize;
    let lines = out.len() / width;
    let mut blocks = vec![[0i32; 64]; 16];
    // The weight matrix transposed, the transform's order.
    let weights: [i32; 64] = std::array::from_fn(|i| i32::from(h.qmat[(i % 8) * 8 + i / 8]));
    let mut px = [0u16; 64];
    for t in row {
        let d = &frame[t.data.clone()];
        if d.len() < 8 {
            return Err(bad("ProRes RAW tile header"));
        }
        let head = usize::from(d[0] >> 3);
        let scale = i32::from(d[1]);
        let s0 = usize::from(be16(d, 2));
        let s1 = usize::from(be16(d, 4));
        let s2 = usize::from(be16(d, 6));
        let Some(s3) = d.len().checked_sub(head + s0 + s1 + s2) else {
            return Err(bad("ProRes RAW tile sizes"));
        };
        let q: [i32; 64] = std::array::from_fn(|i| (weights[i] * scale) >> 1);
        let mut at = head;
        let n_blocks = 1usize << t.log2_blocks;
        // Coded order: second-row green, first-row green, blue, red.
        for (comp, size) in [(2usize, s0), (1, s1), (3, s2), (0, s3)] {
            let data = &d[at..at + size];
            if output == Output::CodedFfmpeg80 {
                let start = t.data.start + at;
                ffmpeg80::coefficients(
                    &frame[start..],
                    size,
                    t.log2_blocks,
                    &mut blocks[..n_blocks],
                );
            } else {
                coefficients(data, t.log2_blocks, &mut blocks[..n_blocks]);
            }
            at += size;
            let (dy, dx) = (comp >> 1, comp & 1);
            for (k, block) in blocks[..n_blocks].iter().enumerate() {
                idct::put(block, &q, &Arith::B12, &mut px);
                let x0 = t.x as usize + 16 * k + dx;
                for r in 0..8 {
                    let y = 2 * r + dy;
                    if y >= lines {
                        break;
                    }
                    let line = &mut out[y * width..(y + 1) * width];
                    for c in 0..8 {
                        let x = x0 + 2 * c;
                        if x >= width {
                            break;
                        }
                        let u = u32::from(px[r * 8 + c]) << 4;
                        line[x] = match output {
                            Output::Coded | Output::CodedFfmpeg80 => u as u16,
                            Output::Linear => linear(u, &h.curve),
                        };
                    }
                }
            }
        }
    }
    Ok(())
}

/// Decodes a frame's sensor samples into `out` (`width × height`, row-major), rows of tiles
/// in parallel when `parallel`.
pub fn decode(
    frame: &[u8],
    h: &RawHeader,
    out: &mut [u16],
    output: Output,
    parallel: bool,
) -> Result<()> {
    let width = h.width as usize;
    if out.len() != width * h.height as usize {
        return Err(bad("ProRes RAW output size"));
    }
    let rows = tiles(frame, h)?;
    if parallel {
        out.par_chunks_mut(width * 16)
            .zip(rows.par_iter())
            .try_for_each(|(o, r)| decode_row(frame, h, r, o, output))
    } else {
        out.chunks_mut(width * 16)
            .zip(&rows)
            .try_for_each(|(o, r)| decode_row(frame, h, r, o, output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_curve_doubles_each_segment() {
        assert_eq!(
            DEFAULT_CURVE,
            [0, 512, 1024, 2048, 4096, 8192, 16384, 32768]
        );
        // The first segment is a sixteenth: codes below 8192 are their linear value × 16.
        assert_eq!(linear(4096, &DEFAULT_CURVE), 256);
        assert_eq!(linear(8192, &DEFAULT_CURVE), 512);
        // The top segment runs from 32768 on to 65536.
        assert_eq!(linear(57344, &DEFAULT_CURVE), 32768);
        assert_eq!(linear(65535, &DEFAULT_CURVE), 65532);
        // Monotonic.
        let mut last = 0;
        for u in 0..65536 {
            let v = linear(u, &DEFAULT_CURVE);
            assert!(v >= last);
            last = v;
        }
    }

    #[test]
    fn codes() {
        // Rice order 1, switch 0: "1 1" = 1.
        let mut b = Bits::new(&[0b1100_0000, 0, 0, 0, 0xff]);
        assert_eq!(value(&mut b, code(0, 2, 1)), Some(1));
        // Exponential Golomb past the switch: q = 2 > 0, bits = 2 + 4 - 0 = 6: "001xxx".
        let mut b = Bits::new(&[0b0010_1000, 0, 0, 0, 0xff]);
        // 0b001010 = 10, + (1 << 1) - (1 << 2) = 8.
        assert_eq!(value(&mut b, code(0, 2, 1)), Some(8));
        // All zero: the component ends.
        let mut b = Bits::new(&[0, 0, 0, 0, 0xff]);
        assert_eq!(value(&mut b, code(0, 2, 1)), None);
        let mut b = Bits::new(&[]);
        assert_eq!(value(&mut b, code(0, 2, 1)), None);
    }

    /// A frame header: `w × h`, alignment `align`, optional weights and curve.
    fn frame(w: u16, h: u16, align: u16) -> Vec<u8> {
        let mut hd = vec![0u8, 0];
        hd.extend_from_slice(b"test");
        hd.extend_from_slice(&w.to_be_bytes());
        hd.extend_from_slice(&h.to_be_bytes());
        hd.extend_from_slice(&[2, 2, 2, 2, 0, 0]);
        hd.extend_from_slice(&3840u16.to_be_bytes());
        for v in [
            2.0f32, 1.5, 0.5, 0.3, 0.2, 0.2, 0.7, 0.1, 0.0, 0.1, 1.0, 1.0,
        ] {
            hd.extend_from_slice(&v.to_bits().to_be_bytes());
        }
        hd.extend_from_slice(&5600u16.to_be_bytes());
        hd.extend_from_slice(&(align << 1).to_be_bytes());
        let mut f = vec![0u8; 8];
        f[4..8].copy_from_slice(b"prrf");
        f.extend_from_slice(&((hd.len() + 2) as u16).to_be_bytes());
        f.extend_from_slice(&hd);
        let cols = u32::from(w).div_ceil(16);
        let per_row = (cols >> align) + (cols & ((1 << align) - 1)).count_ones();
        let n = per_row * u32::from(h).div_ceil(16);
        // Each tile: a header of 8 bytes and four empty components.
        for _ in 0..n {
            f.extend_from_slice(&8u16.to_be_bytes());
        }
        for _ in 0..n {
            f.extend_from_slice(&[8 << 3, 2, 0, 0, 0, 0, 0, 0]);
        }
        let len = f.len() as u32;
        f[..4].copy_from_slice(&len.to_be_bytes());
        f
    }

    #[test]
    fn header_fields() {
        let f = frame(64, 32, 2);
        let h = raw_header(&f).unwrap();
        assert_eq!(
            (h.width, h.height, h.align, h.white),
            (64, 32, 2, 3840 + 256)
        );
        assert_eq!(h.wb, [2.0, 1.5]);
        assert_eq!(h.matrix[0], [0.5, 0.3, 0.2]);
        assert_eq!((h.gain, h.cct, h.cfa_name()), (1.0, 5600, "RGGB"));
        assert!(!h.has_curve);
    }

    #[test]
    fn right_edge_tiles_halve() {
        // 7 columns of 16 at alignment 2: tiles of 4, 2 and 1 columns.
        let f = frame(112, 16, 2);
        let h = raw_header(&f).unwrap();
        let t = tiles(&f, &h).unwrap();
        let got: Vec<_> = t[0].iter().map(|t| (t.x, t.log2_blocks)).collect();
        assert_eq!(got, [(0, 2), (64, 1), (96, 0)]);
    }

    #[test]
    fn empty_components_are_mid_code() {
        // No coefficients: every block is the transform of zero, mid-code 2048 (12 bits).
        let f = frame(48, 20, 1);
        let h = raw_header(&f).unwrap();
        let mut out = vec![0u16; 48 * 20];
        decode(&f, &h, &mut out, Output::Coded, true).unwrap();
        assert!(out.iter().all(|&v| v == 2048 << 4), "{:?}", &out[..4]);
        decode(&f, &h, &mut out, Output::Linear, false).unwrap();
        assert!(out.iter().all(|&v| v == linear(2048 << 4, &DEFAULT_CURVE)));
    }

    #[test]
    fn damaged_frames_fail_cleanly() {
        let f = frame(64, 32, 2);
        for n in 0..f.len() {
            if let Ok(h) = raw_header(&f[..n]) {
                let mut out = vec![0u16; (h.width * h.height) as usize];
                let _ = decode(&f[..n], &h, &mut out, Output::Linear, false);
            }
        }
        let mut bad = f.clone();
        bad[4] = b'x';
        assert!(raw_header(&bad).is_err());
        // Random tile bytes never panic.
        let mut seed = 7u32;
        let mut g = f.clone();
        for _ in 0..200 {
            for b in g.iter_mut().skip(100) {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *b = (seed >> 24) as u8;
            }
            if let Ok(h) = raw_header(&g) {
                let mut out = vec![0u16; (h.width * h.height) as usize];
                let _ = decode(&g, &h, &mut out, Output::Linear, false);
            }
        }
    }
}
