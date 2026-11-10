//! Canon CRX sensor codec (CR3): lossless ("RAW") and lossy wavelet ("C-RAW") variants.
//!
//! Independent implementation written from the format description (Laurent Clévy's CR3 notes)
//! and by studying the behaviour of existing decoders; values are verified bit-exact against
//! LibRaw on real files (see `tests/samples.rs` and the bench notes).
//!
//! Layout: the `CMP1` box in the CRAW sample entry describes the image (plane size, tiles,
//! bit depth, CFA layout, wavelet levels). A sample in `mdat` starts with `mdat_hdr_size` bytes
//! of tile / plane / subband headers, followed by the entropy-coded subband data. Each colour
//! plane (R, G1, G2, B at half resolution) of each tile is coded independently, so tiles x
//! planes decode in parallel.
//!
//! Entropy coding: adaptive Golomb-Rice codes (unary zero count + `k` low bits) with run-length
//! mode for flat areas and median-edge prediction for the lowest band. Lossy files add a 5/3
//! integer wavelet (up to three levels) with per-line or per-block quantisation steps.

use rayon::prelude::*;

use crate::bits::BitReader;

/// Parsed `CMP1` header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Header {
    pub version: u16,
    /// Full image size (all planes interleaved).
    pub width: u32,
    pub height: u32,
    pub tile_w: u32,
    pub tile_h: u32,
    pub n_bits: u8,
    pub n_planes: u8,
    /// 0 RGGB, 1 GRBG, 2 GBRG, 3 BGGR.
    pub cfa_layout: u8,
    pub enc_type: u8,
    pub levels: u8,
    pub mdat_hdr_size: u32,
    pub median_bits: u8,
}

impl Header {
    /// Effective bits per output sample.
    pub fn bits(&self) -> u32 {
        if self.enc_type == 3 {
            self.median_bits as u32
        } else {
            self.n_bits as u32
        }
    }

    pub fn cfa_name(&self) -> &'static str {
        match self.cfa_layout {
            1 => "GRBG",
            2 => "GBRG",
            3 => "BGGR",
            _ => "RGGB",
        }
    }
}

fn be16(b: &[u8], o: usize) -> Option<u32> {
    Some(u16::from_be_bytes(b.get(o..o + 2)?.try_into().ok()?) as u32)
}

fn be32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// Size and reason for a `CMP1` header that [`parse_cmp1`] refuses or that is not a Bayer
/// (four-plane) image.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Refused {
    pub width: u32,
    pub height: u32,
    pub bits: u32,
    pub why: &'static str,
}

/// Why a `CMP1` header cannot be decoded as a Bayer image, or `None` when it is unreadable.
pub(crate) fn refused(b: &[u8]) -> Option<Refused> {
    if b.len() < 40 {
        return None;
    }
    let why = match b[25] >> 4 {
        4 => "Canon CR3: unsupported CRX header variant",
        // No CR3 body writes sRAW or mRAW (C-RAW replaced them); roll-burst frames (encoding 3,
        // colour-difference planes) are four-plane and take the CRX path. Others are unknown.
        _ => "Canon CR3: unknown CRX plane layout (no CR3 camera writes sRAW or mRAW)",
    };
    Some(Refused {
        width: be32(b, 8)?,
        height: be32(b, 12)?,
        bits: u32::from(b[24]),
        why,
    })
}

/// Parse the payload of a `CMP1` box. Returns `None` for headers this decoder does not handle.
pub(crate) fn parse_cmp1(b: &[u8]) -> Option<Header> {
    if b.len() < 40 {
        return None;
    }
    let version = be16(b, 4)? as u16;
    let n_bits = b[24];
    let n_planes = b[25] >> 4;
    let mut h = Header {
        version,
        width: be32(b, 8)?,
        height: be32(b, 12)?,
        tile_w: be32(b, 16)?,
        tile_h: be32(b, 20)?,
        n_bits,
        n_planes,
        cfa_layout: b[25] & 15,
        enc_type: b[26] >> 4,
        levels: b[26] & 15,
        mdat_hdr_size: be32(b, 28)?,
        median_bits: n_bits,
    };
    let has_tile_cols = b[27] >> 7;
    let has_tile_rows = (b[27] >> 6) & 1;
    let ext = b[32] >> 7;
    if ext != 0 && n_planes == 4 && b.len() > 84 && (b[56] >> 6) & 1 == 1 {
        h.median_bits = b[84];
    }
    if (version != 0x100 && version != 0x200) || h.mdat_hdr_size == 0 {
        return None;
    }
    match h.enc_type {
        1 if h.n_bits <= 15 => {}
        0 | 3 if h.n_bits <= 14 => {}
        _ => return None,
    }
    if h.n_planes == 1 {
        if h.cfa_layout != 0 || h.enc_type != 0 || h.n_bits != 8 {
            return None;
        }
    } else if h.n_planes != 4
        || h.width & 1 != 0
        || h.height & 1 != 0
        || h.tile_w & 1 != 0
        || h.tile_h & 1 != 0
        || h.cfa_layout > 3
        || h.n_bits == 8
    {
        return None;
    }
    if h.enc_type == 3 && (h.n_planes != 4 || h.n_bits <= 8 || !(1..=16).contains(&h.median_bits)) {
        return None;
    }
    if h.n_bits == 0 || h.tile_w > h.width || h.tile_h > h.height || h.levels > 3 {
        return None;
    }
    if has_tile_cols > 1 || has_tile_rows > 1 {
        return None;
    }
    Some(h)
}

// ---------------------------------------------------------------------------------------------
// Tables

/// Run-length step per adaptive state.
const JS: [u32; 32] = [
    1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 8, 8, 8, 8, 16, 16, 32, 32, 64, 64, 128, 128, 256, 512,
    1024, 2048, 4096, 8192, 16384, 32768,
];
/// Extra run-length bits per adaptive state (log2 of `JS`).
const J: [u32; 32] = [
    0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 9, 10, 11, 12, 13,
    14, 15,
];
/// Quantisation step mantissas (one octave in six steps, x64).
const Q_STEP: [i32; 6] = [0x28, 0x2D, 0x33, 0x39, 0x40, 0x48];
/// Extra wavelet coefficients a tile borrows from its right / bottom neighbour, indexed by
/// `[levels - 1][size & 7][2 * level + {0: high band, 1: low band}]`. Follows from the 5/3
/// synthesis: each level needs the high / low coefficients that reconstruct its samples up to
/// the extension the level above asks for, and a low band is as long as the deeper level can
/// reconstruct (`ex_coef_table_follows_from_the_5_3_synthesis` re-derives it).
const EX_COEF: [[[u8; 6]; 8]; 3] = [
    [
        [1, 1, 0, 0, 0, 0],
        [1, 0, 0, 0, 0, 0],
        [1, 1, 0, 0, 0, 0],
        [1, 0, 0, 0, 0, 0],
        [1, 1, 0, 0, 0, 0],
        [1, 0, 0, 0, 0, 0],
        [1, 1, 0, 0, 0, 0],
        [1, 0, 0, 0, 0, 0],
    ],
    [
        [1, 1, 1, 1, 0, 0],
        [1, 0, 1, 0, 0, 0],
        [1, 2, 2, 1, 0, 0],
        [1, 1, 1, 1, 0, 0],
        [1, 1, 1, 1, 0, 0],
        [1, 0, 1, 0, 0, 0],
        [1, 2, 2, 1, 0, 0],
        [1, 1, 1, 1, 0, 0],
    ],
    [
        [1, 1, 1, 1, 1, 1],
        [1, 0, 1, 0, 1, 0],
        [1, 2, 2, 2, 2, 1],
        [1, 1, 1, 2, 2, 1],
        [1, 1, 1, 2, 2, 1],
        [1, 0, 1, 1, 1, 1],
        [1, 2, 2, 1, 1, 1],
        [1, 1, 1, 1, 1, 1],
    ],
];

const RIGHT: u8 = 1;
const LEFT: u8 = 2;
const BOTTOM: u8 = 4;
const TOP: u8 = 8;

// ---------------------------------------------------------------------------------------------
// Bitstream helpers

/// Count (and consume) zero bits up to and including the next one bit.
#[inline(always)]
fn zeros(br: &mut BitReader) -> u32 {
    let mut n = 0u32;
    loop {
        br.refill();
        let v = br.peek(32);
        if v != 0 {
            let z = v.leading_zeros();
            br.consume(z + 1);
            return n + z;
        }
        br.consume(32);
        n += 32;
        if br.overrun() {
            return n;
        }
    }
}

/// Golomb-Rice code with parameter `k` (escape to a 21-bit literal after 41 zeros).
#[inline(always)]
fn golomb(br: &mut BitReader, k: i32) -> u32 {
    // Fast path: prefix and suffix both within the next 32 bits.
    br.refill();
    let v = br.peek(32);
    let z = v.leading_zeros();
    let ku = k as u32;
    if z + 1 + ku <= 32 && (1..=30).contains(&k) {
        let low = (v << z << 1) >> (32 - ku);
        br.consume(z + 1 + ku);
        return low | (z << ku);
    }
    let z = zeros(br);
    if z >= 41 {
        br.bits(21)
    } else if k > 0 {
        br.bits(k as u32) | z.wrapping_shl(k as u32)
    } else {
        z
    }
}

/// Zig-zag to signed.
#[inline(always)]
fn signed(c: u32) -> i32 {
    -((c & 1) as i32) ^ (c >> 1) as i32
}

#[inline(always)]
fn iabs(v: i32) -> i32 {
    v.wrapping_abs()
}

/// Adapt the Golomb parameter to the last code; `max` 0 means unbounded.
#[inline(always)]
fn predict_k(k: i32, code: u32, max: i32) -> i32 {
    let c = code as i32;
    let k = k.clamp(0, 30);
    let half = (1i32 << k) >> 1;
    let hi = c >> k;
    let n = k - (c < half) as i32 + (hi > 2) as i32 + (hi > 5) as i32;
    let n = if max == 0 || n < max { n } else { max };
    n.clamp(0, 30)
}

/// Run length in a flat area (the leading one bit has been read); `left` symbols remain.
#[inline]
fn run_length(br: &mut BitReader, s: &mut usize, left: usize) -> Option<usize> {
    let mut n = 1usize;
    while br.bits(1) == 1 {
        n += JS[*s] as usize;
        if n > left {
            n = left;
            break;
        }
        if *s < 31 {
            *s += 1;
        }
        if n == left {
            break;
        }
    }
    if n < left {
        if J[*s] != 0 {
            n += br.bits(J[*s]) as usize;
        }
        *s = s.saturating_sub(1);
        if n > left {
            return None;
        }
    }
    Some(n)
}

/// Median-edge predictor.
#[inline(always)]
fn med(left: i32, top: i32, top_left: i32) -> i32 {
    let delta = top.wrapping_sub(top_left);
    let neg = delta < 0;
    let idx = ((((top_left < left) ^ neg) as usize) << 1) + ((left < top) ^ neg) as usize;
    [
        left.wrapping_add(delta),
        left.wrapping_add(delta),
        left,
        top,
    ][idx]
}

// ---------------------------------------------------------------------------------------------
// Subband line decoder

#[derive(Clone, Copy)]
enum Mode {
    /// Median prediction from the previous line (lowest band of a "partial" plane).
    Median,
    /// As `Median` on values rounded to `mask * 2` steps.
    Rounded { mask: i32, bits: u32 },
    /// Plain coefficients (wavelet high bands, non-progressive planes).
    NoRef,
}

struct Band<'a> {
    br: BitReader<'a>,
    w: usize,
    mode: Mode,
    s: usize,
    k: i32,
    prev: Vec<i32>,
    cur: Vec<i32>,
    /// Per-column `k` history for `NoRef`.
    kh: Vec<i32>,
    line: usize,
}

type R<T> = std::result::Result<T, &'static str>;

impl<'a> Band<'a> {
    fn new(data: &'a [u8], w: usize, mode: Mode) -> Band<'a> {
        Band {
            br: BitReader::new(data, false),
            w,
            mode,
            s: 0,
            k: 0,
            prev: vec![0; w + 2],
            cur: vec![0; w + 2],
            kh: if matches!(mode, Mode::NoRef) {
                vec![0; w]
            } else {
                Vec::new()
            },
            line: 0,
        }
    }

    /// Decode the next line; the values are `self.prev[1..=w]` afterwards.
    fn next_line(&mut self) -> R<()> {
        if self.w == 0 {
            return Ok(());
        }
        let mut prev = std::mem::take(&mut self.prev);
        let mut cur = std::mem::take(&mut self.cur);
        let r = match (self.line, self.mode) {
            (0, Mode::Median) => self.top_line(&mut cur),
            (0, Mode::Rounded { mask, .. }) => self.top_line_rounded(&mut cur, mask),
            (0, Mode::NoRef) => self.top_line_noref(&mut prev, &mut cur),
            (_, Mode::Median) => self.line_median(&prev, &mut cur),
            (_, Mode::Rounded { mask, bits }) => self.line_rounded(&mut prev, &mut cur, mask, bits),
            (_, Mode::NoRef) => self.line_noref(&prev, &mut cur),
        };
        self.line += 1;
        // The line just decoded becomes the reference for the next one.
        self.prev = cur;
        self.cur = prev;
        r
    }

    fn values(&self) -> &[i32] {
        &self.prev[1..=self.w]
    }

    #[inline(always)]
    fn code(&mut self) -> u32 {
        golomb(&mut self.br, self.k)
    }

    fn top_line(&mut self, cur: &mut [i32]) -> R<()> {
        cur[0] = 0;
        let mut p = 0usize;
        let mut length = self.w as isize;
        while length > 1 {
            if cur[p] != 0 {
                cur[p + 1] = cur[p];
            } else {
                if self.br.bits(1) == 1 {
                    let n = run_length(&mut self.br, &mut self.s, length as usize)
                        .ok_or("CRX: bad run length")?;
                    length -= n as isize;
                    for _ in 0..n {
                        cur[p + 1] = cur[p];
                        p += 1;
                    }
                    if length <= 0 {
                        break;
                    }
                }
                cur[p + 1] = 0;
            }
            let c = self.code();
            cur[p + 1] = cur[p + 1].wrapping_add(signed(c));
            self.k = predict_k(self.k, c, 15);
            p += 1;
            length -= 1;
        }
        if length == 1 {
            cur[p + 1] = cur[p];
            let c = self.code();
            cur[p + 1] = cur[p + 1].wrapping_add(signed(c));
            self.k = predict_k(self.k, c, 15);
            p += 1;
        }
        cur[p + 1] = cur[p].wrapping_add(1);
        Ok(())
    }

    fn top_line_rounded(&mut self, cur: &mut [i32], mask: i32) -> R<()> {
        cur[0] = 0;
        let mut p = 0usize;
        let mut length = self.w as isize;
        let step = |s: i32| mask.wrapping_mul(2).wrapping_mul(s).wrapping_add(s >> 31);
        while length > 1 {
            if iabs(cur[p]) > mask {
                cur[p + 1] = cur[p];
            } else {
                if self.br.bits(1) == 1 {
                    let n = run_length(&mut self.br, &mut self.s, length as usize)
                        .ok_or("CRX: bad run length")?;
                    length -= n as isize;
                    for _ in 0..n {
                        cur[p + 1] = cur[p];
                        p += 1;
                    }
                }
                if length <= 0 {
                    break;
                }
                cur[p + 1] = 0;
            }
            let c = self.code();
            cur[p + 1] = cur[p + 1].wrapping_add(step(signed(c)));
            self.k = predict_k(self.k, c, 15);
            p += 1;
            length -= 1;
        }
        if length == 1 {
            // The slot is not reset first here: it still holds zero from initialisation.
            let c = self.code();
            cur[p + 1] = cur[p + 1].wrapping_add(step(signed(c)));
            self.k = predict_k(self.k, c, 15);
            p += 1;
        }
        cur[p + 1] = cur[p].wrapping_add(1);
        Ok(())
    }

    fn top_line_noref(&mut self, prev: &mut [i32], cur: &mut [i32]) -> R<()> {
        prev[0] = 0;
        cur[0] = 0;
        let (mut p, mut q) = (0usize, 0usize);
        let mut length = self.w as isize;
        while length > 1 {
            if cur[p] != 0 {
                let c = self.code();
                cur[p + 1] = signed(c);
                self.k = predict_k(self.k, c, 15);
            } else {
                let mut n = 0usize;
                if self.br.bits(1) == 1 {
                    n = run_length(&mut self.br, &mut self.s, length as usize)
                        .ok_or("CRX: bad run length")?;
                }
                length -= n as isize;
                for _ in 0..n {
                    self.kh[q] = 0;
                    cur[p + 1] = 0;
                    p += 1;
                    q += 1;
                }
                if length <= 0 {
                    break;
                }
                let c = self.code();
                cur[p + 1] = signed(c.wrapping_add(1));
                self.k = predict_k(self.k, c, 15);
            }
            self.kh[q] = self.k;
            q += 1;
            p += 1;
            length -= 1;
        }
        if length == 1 {
            let c = self.code();
            cur[p + 1] = signed(c);
            self.k = predict_k(self.k, c, 15);
            self.kh[q] = self.k;
            p += 1;
        }
        cur[p + 1] = 0;
        Ok(())
    }

    /// Hot path (lossless planes): bit reader and Golomb state live in locals so they stay in
    /// registers.
    fn line_median(&mut self, prev: &[i32], cur: &mut [i32]) -> R<()> {
        let mut br = std::mem::replace(&mut self.br, BitReader::new(&[], false));
        let (mut k, mut s) = (self.k, self.s);
        let w = self.w;
        let prev = &prev[..w + 2];
        let cur = &mut cur[..w + 2];
        let (mut p0, mut p1) = (0usize, 0usize);
        cur[0] = prev[1];
        let mut length = w as isize;
        let mut bad = false;
        // One symbol: `median` selects the edge predictor, `not_eol` folds in the next gradient.
        macro_rules! sym {
            ($median:expr, $not_eol:expr) => {{
                let pred = if $median {
                    med(cur[p1], prev[p0 + 1], prev[p0])
                } else {
                    prev[p0 + 1]
                };
                let mut c = golomb(&mut br, k);
                cur[p1 + 1] = pred.wrapping_add(signed(c));
                if $not_eol {
                    let nd = prev[p0 + 2].wrapping_sub(prev[p0 + 1]).wrapping_shl(1);
                    c = c.wrapping_add(iabs(nd) as u32) >> 1;
                    #[allow(unused_assignments)]
                    {
                        p0 += 1;
                    }
                }
                k = predict_k(k, c, 15);
                p1 += 1;
            }};
        }
        while length > 1 {
            if cur[p1] != prev[p0 + 1] || cur[p1] != prev[p0 + 2] {
                sym!(true, true);
            } else {
                if br.bits(1) == 1 {
                    let Some(n) = run_length(&mut br, &mut s, length as usize) else {
                        bad = true;
                        break;
                    };
                    length -= n as isize;
                    p0 += n;
                    let v = cur[p1];
                    cur[p1 + 1..p1 + 1 + n].fill(v);
                    p1 += n;
                }
                if length > 0 {
                    sym!(false, length > 1);
                }
            }
            length -= 1;
        }
        if !bad {
            if length == 1 {
                sym!(true, false);
            }
            cur[p1 + 1] = cur[p1].wrapping_add(1);
        }
        self.br = br;
        self.k = k;
        self.s = s;
        if bad {
            Err("CRX: bad run length")
        } else {
            Ok(())
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    fn sym_rounded(
        &mut self,
        prev: &[i32],
        cur: &mut [i32],
        p0: usize,
        p1: &mut usize,
        mask: i32,
        bits: u32,
        do_sym: bool,
        do_code: bool,
    ) {
        let sym = if do_sym {
            med(cur[*p1], prev[p0 + 1], prev[p0])
        } else {
            prev[p0 + 1]
        };
        let c = self.code();
        let s = signed(c);
        cur[*p1 + 1] = mask
            .wrapping_mul(2)
            .wrapping_mul(s)
            .wrapping_add(s >> 31)
            .wrapping_add(sym);
        if do_code {
            let (a, b) = (prev[p0 + 2], prev[p0 + 1]);
            let code = if a > b {
                a.wrapping_sub(b).wrapping_add(mask - 1) >> bits
            } else {
                -(b.wrapping_sub(a).wrapping_add(mask) >> bits)
            };
            let v = c.wrapping_add(2u32.wrapping_mul(iabs(code) as u32)) >> 1;
            self.k = predict_k(self.k, v, 15);
        } else {
            self.k = predict_k(self.k, c, 15);
        }
        *p1 += 1;
    }

    fn line_rounded(&mut self, prev: &mut [i32], cur: &mut [i32], mask: i32, bits: u32) -> R<()> {
        let mut reached = false;
        prev[0] = prev[1];
        cur[0] = prev[1];
        let (mut p0, mut p1) = (0usize, 0usize);
        let mut length = self.w as isize;
        while length > 1 {
            if iabs(prev[p0 + 2].wrapping_sub(prev[p0 + 1])) > mask {
                self.sym_rounded(prev, cur, p0, &mut p1, mask, bits, true, true);
                p0 += 1;
                reached = true;
            } else if reached || iabs(prev[p0].wrapping_sub(cur[p1])) > mask {
                self.sym_rounded(prev, cur, p0, &mut p1, mask, bits, true, true);
                p0 += 1;
                reached = false;
            } else {
                if self.br.bits(1) == 1 {
                    let n = run_length(&mut self.br, &mut self.s, length as usize)
                        .ok_or("CRX: bad run length")?;
                    length -= n as isize;
                    p0 += n;
                    let v = cur[p1];
                    cur[p1 + 1..p1 + 1 + n].fill(v);
                    p1 += n;
                }
                if length > 1 {
                    self.sym_rounded(prev, cur, p0, &mut p1, mask, bits, false, true);
                    p0 += 1;
                    reached = iabs(prev[p0 + 1].wrapping_sub(prev[p0])) > mask;
                } else if length == 1 {
                    self.sym_rounded(prev, cur, p0, &mut p1, mask, bits, false, false);
                }
            }
            length -= 1;
        }
        if length == 1 {
            self.sym_rounded(prev, cur, p0, &mut p1, mask, bits, true, false);
        }
        cur[p1 + 1] = cur[p1].wrapping_add(1);
        Ok(())
    }

    /// Hot path (wavelet bands): state in locals, see [`Band::line_median`].
    fn line_noref(&mut self, prev: &[i32], cur: &mut [i32]) -> R<()> {
        let mut br = std::mem::replace(&mut self.br, BitReader::new(&[], false));
        let (mut k, mut s) = (self.k, self.s);
        let w = self.w;
        let prev = &prev[..w + 2];
        let cur = &mut cur[..w + 2];
        let kh = &mut self.kh[..w];
        let mut bad = false;
        let mut i = 0usize;
        // Adapt `k` to the new code and the `k` used one line up.
        macro_rules! adapt {
            ($c:expr) => {{
                k = predict_k(k, $c, 0);
                if kh[i + 1] - k <= 1 {
                    if k >= 15 {
                        k = 15;
                    }
                } else {
                    k += 1;
                }
            }};
        }
        // `i + 1 < w` instead of `i < w - 1`: w >= 1 here.
        while i + 1 < w {
            if prev[i + 2] | prev[i + 1] | cur[i] != 0 {
                let c = golomb(&mut br, k);
                cur[i + 1] = signed(c);
                adapt!(c);
            } else {
                let mut n = 0usize;
                if br.bits(1) == 1 {
                    let Some(r) = run_length(&mut br, &mut s, w - i) else {
                        bad = true;
                        break;
                    };
                    n = r;
                }
                if n > 0 {
                    cur[i + 1..i + 1 + n].fill(0);
                    kh[i..i + n].fill(0);
                    i += n;
                }
                if i + 1 >= w {
                    if i + 1 == w {
                        let c = golomb(&mut br, k);
                        cur[i + 1] = signed(c.wrapping_add(1));
                        k = predict_k(k, c, 15);
                        kh[i] = k;
                    }
                    i += 1;
                    continue;
                }
                let c = golomb(&mut br, k);
                cur[i + 1] = signed(c.wrapping_add(1));
                adapt!(c);
            }
            kh[i] = k;
            i += 1;
        }
        if !bad && i + 1 == w {
            let c = golomb(&mut br, k);
            cur[i + 1] = signed(c);
            k = predict_k(k, c, 15);
            kh[i] = k;
        }
        self.br = br;
        self.k = k;
        self.s = s;
        if bad {
            Err("CRX: bad run length")
        } else {
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Image / tile / subband headers

#[derive(Clone, Debug, Default)]
struct SubbandInfo {
    width: usize,
    height: usize,
    /// Byte offset of this band's data within its plane's data.
    data_offset: usize,
    data_size: usize,
    /// Old-style header: per-line quantisation updates.
    supports_partial: bool,
    q_param: i32,
    q_step_base: i32,
    q_step_mult: i32,
    row_start: usize,
    row_end: usize,
    col_start: usize,
    col_end: usize,
    level_shift: u32,
}

#[derive(Clone, Debug, Default)]
struct PlaneInfo {
    data_offset: usize,
    supports_partial: bool,
    rounded_mask: i32,
    bands: Vec<SubbandInfo>,
}

#[derive(Clone, Debug, Default)]
struct QStep {
    width: usize,
    height: usize,
    table: Vec<i32>,
}

#[derive(Clone, Debug, Default)]
struct Tile {
    width: usize,
    height: usize,
    flags: u8,
    data_offset: usize,
    qp_size: usize,
    extra_size: usize,
    planes: Vec<PlaneInfo>,
    /// Per wavelet level (deepest first), when the tile carries block quantisation data.
    qsteps: Option<Vec<QStep>>,
}

struct Image {
    plane_w: usize,
    plane_h: usize,
    tile_cols: usize,
    tile_rows: usize,
    levels: usize,
    tiles: Vec<Tile>,
}

fn setup_image(h: &Header, hdr: &[u8], data: &[u8]) -> R<Image> {
    let (mut pw, mut ph, mut tw, mut th) = (
        h.width as usize,
        h.height as usize,
        h.tile_w as usize,
        h.tile_h as usize,
    );
    if h.n_planes == 4 {
        pw >>= 1;
        ph >>= 1;
        tw >>= 1;
        th >>= 1;
    }
    if tw < 0x16 || th < 0x16 || pw > 0x7fff || ph > 0x7fff {
        return Err("CRX: bad tile geometry");
    }
    let tile_cols = pw.div_ceil(tw);
    let tile_rows = ph.div_ceil(th);
    if tile_cols > 0xff
        || tile_rows > 0xff
        || pw - tw * (tile_cols - 1) < 0x16
        || ph - th * (tile_rows - 1) < 0x16
    {
        return Err("CRX: bad tile geometry");
    }
    let levels = h.levels as usize;
    let n_bands = 3 * levels + 1;
    let n_planes = h.n_planes as usize;
    let n_tiles = tile_cols * tile_rows;
    let mut tiles = Vec::with_capacity(n_tiles);
    for t in 0..n_tiles {
        let (col, row) = (t % tile_cols, t / tile_cols);
        let mut flags = 0u8;
        let width = if col + 1 < tile_cols {
            tw
        } else {
            pw - tw * (tile_cols - 1)
        };
        let height = if row + 1 < tile_rows {
            th
        } else {
            ph - th * (tile_rows - 1)
        };
        if col + 1 < tile_cols {
            flags |= RIGHT;
        }
        if col > 0 {
            flags |= LEFT;
        }
        if row + 1 < tile_rows {
            flags |= BOTTOM;
        }
        if row > 0 {
            flags |= TOP;
        }
        tiles.push(Tile {
            width,
            height,
            flags,
            ..Default::default()
        });
    }

    let mut p = 0usize;
    let get = |o: usize, n: usize| -> R<u32> {
        let b = hdr.get(o..o + n).ok_or("CRX: truncated header")?;
        Ok(b.iter().fold(0u32, |a, &v| (a << 8) | v as u32))
    };
    let mut tile_off = 0usize;
    for (ti, tile) in tiles.iter_mut().enumerate() {
        let sign = get(p, 2)?;
        let size = get(p + 2, 2)? as usize;
        if !((sign == 0xff01 && size == 8) || (sign == 0xff11 && (size == 8 || size == 16))) {
            return Err("CRX: bad tile header");
        }
        let tail = get(p + 10, 2)?;
        if (size == 8 && tail != 0) || (size == 16 && tail != 0x4000) {
            return Err("CRX: bad tile header");
        }
        if get(p + 8, 2)? as usize != ti {
            return Err("CRX: tile number mismatch");
        }
        let tile_size = get(p + 4, 4)? as usize;
        tile.data_offset = tile_off;
        if size == 16 {
            if get(p + 18, 2)? != 0 {
                return Err("CRX: bad tile header");
            }
            tile.qp_size = get(p + 12, 4)? as usize;
            tile.extra_size = get(p + 16, 2)? as usize;
            if h.version == 0x200 {
                tile.qsteps = Some(Vec::new());
            }
        }
        p += size + 4;
        tile_off = tile_off.checked_add(tile_size).ok_or("CRX: tile size")?;

        let mut comp_off = 0usize;
        for ci in 0..n_planes {
            let sign = get(p, 2)?;
            let size = get(p + 2, 2)?;
            if (sign != 0xff02 && sign != 0xff12) || size != 8 {
                return Err("CRX: bad plane header");
            }
            let flags8 = *hdr.get(p + 8).ok_or("CRX: truncated header")?;
            if (flags8 >> 4) as usize != ci || get(p + 9, 3)? != 0 {
                return Err("CRX: bad plane header");
            }
            let comp_size = get(p + 4, 4)? as usize;
            let rounded_bits = (flags8 >> 1) & 3;
            let mut plane = PlaneInfo {
                data_offset: comp_off,
                supports_partial: flags8 & 8 != 0,
                rounded_mask: 0,
                bands: Vec::with_capacity(n_bands),
            };
            comp_off = comp_off.checked_add(comp_size).ok_or("CRX: plane size")?;
            p += 12;
            if rounded_bits != 0 {
                if levels != 0 || !plane.supports_partial {
                    return Err("CRX: rounded bits with wavelet");
                }
                plane.rounded_mask = 1 << (rounded_bits - 1);
            }
            // Subband headers.
            let mut band_off = 0usize;
            for bi in 0..n_bands {
                let sign = get(p, 2)?;
                let size = get(p + 2, 2)? as usize;
                if !((sign == 0xff03 && size == 8) || (sign == 0xff13 && size == 16)) {
                    return Err("CRX: bad subband header");
                }
                let band_size = get(p + 4, 4)? as usize;
                let idx = *hdr.get(p + 8).ok_or("CRX: truncated header")? >> 4;
                if idx as usize != bi {
                    return Err("CRX: subband number mismatch");
                }
                let mut b = SubbandInfo {
                    data_offset: band_off,
                    ..Default::default()
                };
                if sign == 0xff03 {
                    let bits = get(p + 8, 4)?;
                    b.data_size = band_size.saturating_sub((bits & 0x7ffff) as usize);
                    b.supports_partial = bits & 0x800_0000 != 0;
                    b.q_param = ((bits >> 19) & 0xff) as i32;
                } else {
                    if get(p + 8, 2)? & 0xfff != 0 || get(p + 18, 2)? != 0 {
                        return Err("CRX: bad subband header");
                    }
                    b.data_size = band_size.saturating_sub(get(p + 16, 2)? as usize);
                    b.q_step_base = get(p + 12, 4)? as i32;
                    b.q_step_mult = get(p + 10, 2)? as i32;
                }
                band_off = band_off.checked_add(band_size).ok_or("CRX: band size")?;
                plane.bands.push(b);
                p += size + 4;
            }
            setup_subbands(h, levels, tile, &mut plane.bands);
            tile.planes.push(plane);
        }
    }

    // Block quantisation tables (version 0x200 with QP data).
    for tile in &mut tiles {
        if tile.qsteps.is_none() {
            continue;
        }
        let src = data
            .get(tile.data_offset..tile.data_offset.saturating_add(tile.qp_size))
            .ok_or("CRX: truncated QP data")?;
        tile.qsteps = Some(make_qsteps(src, tile.width, tile.height, levels)?);
    }
    Ok(Image {
        plane_w: pw,
        plane_h: ph,
        tile_cols,
        tile_rows,
        levels,
        tiles,
    })
}

/// Subband sizes for a tile: each level halves the size; tiles with neighbours carry a few
/// extra coefficients across the shared edge so the wavelet is seamless.
fn setup_subbands(h: &Header, levels: usize, tile: &Tile, bands: &mut [SubbandInfo]) {
    let (mut bw, mut bh) = (tile.width, tile.height);
    let v2 = h.version == 0x200;
    let set_idx =
        |b: &mut SubbandInfo, level: usize, cs: usize, ce: usize, rs: usize, re: usize| {
            if v2 {
                b.col_start = cs;
                b.col_end = ce;
                b.row_start = rs;
                b.row_end = re;
                b.level_shift = 3u32.saturating_sub(level as u32);
            }
        };
    let n = bands.len();
    if levels == 0 {
        bands[0].width = bw;
        bands[0].height = bh;
        return;
    }
    let row_ex = &EX_COEF[levels - 1][tile.width & 7];
    let col_ex = &EX_COEF[levels - 1][tile.height & 7];
    for level in 0..levels {
        let (w_odd, h_odd) = (bw & 1, bh & 1);
        bw = (bw + w_odd) >> 1;
        bh = (bh + h_odd) >> 1;
        let (mut wex0, mut wex1, mut hex0, mut hex1) = (0usize, 0usize, 0usize, 0usize);
        let (mut cs, mut rs) = (0usize, 0usize);
        if tile.flags & RIGHT != 0 {
            wex0 = row_ex[2 * level] as usize;
            wex1 = row_ex[2 * level + 1] as usize;
        }
        if tile.flags & LEFT != 0 {
            wex0 += 1;
            cs = 1;
        }
        if tile.flags & BOTTOM != 0 {
            hex0 = col_ex[2 * level] as usize;
            hex1 = col_ex[2 * level + 1] as usize;
        }
        if tile.flags & TOP != 0 {
            hex0 += 1;
            rs = 1;
        }
        let hh = n - 1 - 3 * level;
        bands[hh].width = bw + wex0 - w_odd;
        bands[hh].height = bh + hex0 - h_odd;
        set_idx(&mut bands[hh], level + 1, cs, wex0 - cs, rs, hex0 - rs);
        let lh = hh - 1;
        bands[lh].width = bw + wex1;
        bands[lh].height = bh + hex0 - h_odd;
        set_idx(&mut bands[lh], level + 1, 0, wex1, rs, hex0 - rs);
        let hl = hh - 2;
        bands[hl].width = bw + wex0 - w_odd;
        bands[hl].height = bh + hex1;
        set_idx(&mut bands[hl], level + 1, cs, wex0 - cs, 0, hex1);
    }
    let wex = if tile.flags & RIGHT != 0 {
        row_ex[2 * levels - 1] as usize
    } else {
        0
    };
    let hex = if tile.flags & BOTTOM != 0 {
        col_ex[2 * levels - 1] as usize
    } else {
        0
    };
    bands[0].width = bw + wex;
    bands[0].height = bh + hex;
    set_idx(&mut bands[0], levels, 0, wex, 0, hex);
}

fn q_step(q: i32) -> R<i32> {
    if q < 0 {
        return Err("CRX: negative quantiser");
    }
    let (m, e) = (Q_STEP[(q % 6) as usize], q / 6);
    Ok(if e >= 6 {
        m.wrapping_shl(((e - 6) & 31) as u32)
    } else {
        m >> (6 - e)
    })
}

/// Decode the per-block quantiser map (8x2 plane pixels per entry) and derive per-level tables.
fn make_qsteps(src: &[u8], tw: usize, th: usize, levels: usize) -> R<Vec<QStep>> {
    if !(1..=3).contains(&levels) {
        return Err("CRX: QP data without wavelet");
    }
    let qw = tw.div_ceil(8);
    let qh = th.div_ceil(2);
    let mut br = BitReader::new(src, false);
    let mut table = Vec::with_capacity(qw * qh);
    let mut l0 = vec![0i32; qw + 2];
    let mut l1 = vec![0i32; qw + 2];
    let mut k = 0i32;
    let read_qp = |br: &mut BitReader, k: i32| -> u32 {
        let z = zeros(br);
        if z >= 23 {
            br.bits(8)
        } else if k > 0 {
            br.bits(k as u32) | z.wrapping_shl(k as u32)
        } else {
            z
        }
    };
    for row in 0..qh {
        // `l0` is the previous line, `l1` the current one.
        if row == 0 {
            l1[0] = 0;
            for p in 0..qw {
                let qp = read_qp(&mut br, k);
                l1[p + 1] = l1[p].wrapping_add(signed(qp));
                k = predict_k(k, qp, 7);
            }
        } else {
            l1[0] = l0[1];
            let mut dh = l0[1].wrapping_sub(l0[0]);
            for p in 0..qw {
                let (left, top) = (l1[p], l0[p + 1]);
                let dv = l0[p].wrapping_sub(left);
                let idx =
                    ((((dv < 0) ^ (dh < 0)) as usize) << 1) + ((left < top) ^ (dh < 0)) as usize;
                let pred = [left.wrapping_add(dh), left.wrapping_add(dh), left, top][idx];
                let qp = read_qp(&mut br, k);
                l1[p + 1] = pred.wrapping_add(signed(qp));
                if p + 1 < qw {
                    dh = l0[p + 2].wrapping_sub(l0[p + 1]);
                    k = predict_k(
                        k,
                        qp.wrapping_add(2u32.wrapping_mul(iabs(dh) as u32)) >> 1,
                        7,
                    );
                } else {
                    k = predict_k(k, qp, 7);
                }
            }
        }
        l1[qw + 1] = l1[qw].wrapping_add(1);
        table.extend(l1[1..=qw].iter().map(|&v| v.wrapping_add(4)));
        std::mem::swap(&mut l0, &mut l1);
    }
    let qh4 = th.div_ceil(4);
    let qh8 = th.div_ceil(8);
    let mut out = Vec::with_capacity(levels);
    let row = |r: usize| r.min(qh - 1) * qw;
    if levels == 3 {
        let mut t = Vec::with_capacity(qw * qh8);
        for r in 0..qh8 {
            let rows = [row(4 * r), row(4 * r + 1), row(4 * r + 2), row(4 * r + 3)];
            for c in 0..qw {
                let s = rows.iter().fold(0i32, |a, &o| a.wrapping_add(table[o + c]));
                let q = (((s < 0) as i32) * 3).wrapping_add(s) >> 2;
                t.push(q_step(q)?);
            }
        }
        out.push(QStep {
            width: qw,
            height: qh8,
            table: t,
        });
    }
    if levels >= 2 {
        let mut t = Vec::with_capacity(qw * qh4);
        for r in 0..qh4 {
            let (a, b) = (row(2 * r), row(2 * r + 1));
            for c in 0..qw {
                t.push(q_step(table[a + c].wrapping_add(table[b + c]) / 2)?);
            }
        }
        out.push(QStep {
            width: qw,
            height: qh4,
            table: t,
        });
    }
    let t = table.iter().map(|&q| q_step(q)).collect::<R<Vec<_>>>()?;
    out.push(QStep {
        width: qw,
        height: qh,
        table: t,
    });
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Plane decode

/// Decode all lines of one subband (with dequantisation) into a `width x height` buffer.
fn decode_band(b: &SubbandInfo, src: &[u8], mode: Mode, q: Option<&QStep>) -> R<Vec<i32>> {
    let (w, h) = (b.width, b.height);
    let mut out = vec![0i32; w * h];
    if b.data_size == 0 || w == 0 {
        return Ok(out);
    }
    let src = src
        .get(b.data_offset..b.data_offset.saturating_add(b.data_size))
        .ok_or("CRX: subband data out of range")?;
    let mut band = Band::new(src, w, mode);
    let mut kq = 0i32;
    let mut q_param = b.q_param;
    for (r, dst) in out.chunks_exact_mut(w).enumerate() {
        if b.supports_partial && q.is_none() {
            // Per-line quantiser update.
            let br = &mut band.br;
            let z = zeros(br);
            let c = if z >= 23 {
                br.bits(8)
            } else if kq > 0 {
                br.bits(kq as u32) | z.wrapping_shl(kq as u32)
            } else {
                z
            };
            q_param = q_param.wrapping_add(signed(c));
            kq = predict_k(kq, c, 0);
            if kq > 7 {
                return Err("CRX: bad quantiser update");
            }
        }
        band.next_line()?;
        dst.copy_from_slice(band.values());
        match q {
            Some(q) => {
                let qr = if r < b.row_start {
                    0
                } else if r + b.row_end < h {
                    r - b.row_end
                } else {
                    h.saturating_sub(b.row_end + b.row_start + 1)
                };
                let line = q
                    .table
                    .get(qr.min(q.height.saturating_sub(1)) * q.width..)
                    .ok_or("CRX: QP table")?;
                let at = |i: usize| -> i32 { line.get(i.min(q.width - 1)).copied().unwrap_or(0) };
                let scale = |qs: i32| {
                    b.q_step_base
                        .wrapping_add(qs.wrapping_mul(b.q_step_mult) >> 3)
                        .clamp(1, 0x168000)
                };
                let last = w.saturating_sub(b.col_end + b.col_start + 1) >> b.level_shift;
                for (i, v) in dst.iter_mut().enumerate() {
                    let qi = if i < b.col_start {
                        0
                    } else if i + b.col_end < w {
                        (i - b.col_start) >> b.level_shift
                    } else {
                        last
                    };
                    *v = v.wrapping_mul(scale(at(qi)));
                }
            }
            None => {
                let s = q_step(q_param)?;
                if s != 1 {
                    for v in dst.iter_mut() {
                        *v = v.wrapping_mul(s);
                    }
                }
            }
        }
    }
    Ok(out)
}

/// One level of the inverse 5/3 integer lifting along a line: `low` / `high` coefficients to
/// `out` (`out.len()` samples). `left` / `right`: neighbouring tiles contribute an extra high
/// coefficient before / after.
#[inline]
fn idwt_1d(low: &[i32], high: &[i32], out: &mut [i32], left: bool, right: bool) -> R<()> {
    let n = out.len();
    if n == 0 {
        return Ok(());
    }
    if n == 1 {
        out[0] = *low.first().ok_or("CRX: wavelet geometry")?;
        return Ok(());
    }
    let a = left as usize;
    let m = if n.is_multiple_of(2) {
        n / 2 - 1
    } else {
        (n - 3) / 2
    };
    let need_l = if n.is_multiple_of(2) && !right {
        m + 1
    } else {
        m + 2
    };
    let need_h = a + if right { m + 2 } else { m + 1 };
    if low.len() < need_l || high.len() < need_h {
        return Err("CRX: wavelet geometry");
    }
    let hp = &high[a..];
    out[0] = if left {
        low[0].wrapping_sub(high[0].wrapping_add(hp[0]).wrapping_add(2) >> 2)
    } else {
        low[0].wrapping_sub(hp[0].wrapping_add(1) >> 1)
    };
    for k in 0..m {
        let even = low[k + 1].wrapping_sub(hp[k].wrapping_add(hp[k + 1]).wrapping_add(2) >> 2);
        out[2 * k + 1] = hp[k].wrapping_add(out[2 * k].wrapping_add(even) >> 1);
        out[2 * k + 2] = even;
    }
    let x = out[2 * m];
    if right {
        let d = low[m + 1].wrapping_sub(hp[m].wrapping_add(hp[m + 1]).wrapping_add(2) >> 2);
        out[2 * m + 1] = hp[m].wrapping_add(x.wrapping_add(d) >> 1);
        if n % 2 == 1 {
            out[2 * m + 2] = d;
        }
    } else if n % 2 == 1 {
        let d = low[m + 1].wrapping_sub(hp[m].wrapping_add(1) >> 1);
        out[2 * m + 1] = hp[m].wrapping_add(x.wrapping_add(d) >> 1);
        out[2 * m + 2] = d;
    } else {
        out[2 * m + 1] = hp[m].wrapping_add(x);
    }
    Ok(())
}

/// Inverse 2-D transform of one level: rows first, then columns.
#[allow(clippy::too_many_arguments)]
fn idwt_2d(
    ll: &[i32],
    hl: &[i32],
    lh: &[i32],
    hh: &[i32],
    b: [&SubbandInfo; 3],
    ll_w: usize,
    ll_h: usize,
    out_w: usize,
    out_h: usize,
    flags: u8,
) -> R<Vec<i32>> {
    let [bhl, blh, bhh] = b;
    let (left, right) = (flags & LEFT != 0, flags & RIGHT != 0);
    let (top, bottom) = (flags & TOP != 0, flags & BOTTOM != 0);
    if ll_h != bhl.height || blh.height != bhh.height || ll_w != blh.width || bhl.width != bhh.width
    {
        return Err("CRX: wavelet geometry");
    }
    let n_lv = ll_h;
    let n_hv = blh.height;
    // Horizontal pass: low-vertical rows from (LL, HL), high-vertical rows from (LH, HH).
    let mut lv = vec![0i32; n_lv * out_w];
    for (r, o) in lv.chunks_exact_mut(out_w).enumerate() {
        idwt_1d(
            &ll[r * ll_w..(r + 1) * ll_w],
            &hl[r * bhl.width..(r + 1) * bhl.width],
            o,
            left,
            right,
        )?;
    }
    let mut hv = vec![0i32; n_hv * out_w];
    for (r, o) in hv.chunks_exact_mut(out_w).enumerate() {
        idwt_1d(
            &lh[r * blh.width..(r + 1) * blh.width],
            &hh[r * bhh.width..(r + 1) * bhh.width],
            o,
            left,
            right,
        )?;
    }
    // Vertical pass, done on whole rows at once (same lifting as `idwt_1d`).
    let n = out_h;
    let mut out = vec![0i32; n * out_w];
    if n == 0 {
        return Ok(out);
    }
    if n_lv == 0 {
        return Err("CRX: wavelet geometry");
    }
    let lrow = |k: usize| &lv[k * out_w..(k + 1) * out_w];
    let hrow = |k: usize| &hv[k * out_w..(k + 1) * out_w];
    if n == 1 {
        out.copy_from_slice(lrow(0));
        return Ok(out);
    }
    let a = top as usize;
    let m = if n.is_multiple_of(2) {
        n / 2 - 1
    } else {
        (n - 3) / 2
    };
    let need_l = if n.is_multiple_of(2) && !bottom {
        m + 1
    } else {
        m + 2
    };
    let need_h = a + if bottom { m + 2 } else { m + 1 };
    if n_lv < need_l || n_hv < need_h {
        return Err("CRX: wavelet geometry");
    }
    let hp = |k: usize| hrow(k + a);
    {
        let (o0, _) = out.split_at_mut(out_w);
        let l0 = lrow(0);
        if top {
            let (h0, h1) = (hrow(0), hp(0));
            for i in 0..out_w {
                o0[i] = l0[i].wrapping_sub(h0[i].wrapping_add(h1[i]).wrapping_add(2) >> 2);
            }
        } else {
            let h1 = hp(0);
            for i in 0..out_w {
                o0[i] = l0[i].wrapping_sub(h1[i].wrapping_add(1) >> 1);
            }
        }
    }
    for k in 0..m {
        let (head, tail) = out.split_at_mut((2 * k + 1) * out_w);
        let x0 = &head[2 * k * out_w..];
        let (o1, rest) = tail.split_at_mut(out_w);
        let o2 = &mut rest[..out_w];
        let (l1, h0, h1) = (lrow(k + 1), hp(k), hp(k + 1));
        for i in 0..out_w {
            let even = l1[i].wrapping_sub(h0[i].wrapping_add(h1[i]).wrapping_add(2) >> 2);
            o1[i] = h0[i].wrapping_add(x0[i].wrapping_add(even) >> 1);
            o2[i] = even;
        }
    }
    {
        let (head, tail) = out.split_at_mut((2 * m + 1) * out_w);
        let x0 = &head[2 * m * out_w..];
        let (o1, rest) = tail.split_at_mut(out_w);
        let h0 = hp(m);
        if bottom || n % 2 == 1 {
            let l1 = lrow(m + 1);
            let h1 = if bottom { Some(hp(m + 1)) } else { None };
            for i in 0..out_w {
                let d = match h1 {
                    Some(h1) => l1[i].wrapping_sub(h0[i].wrapping_add(h1[i]).wrapping_add(2) >> 2),
                    None => l1[i].wrapping_sub(h0[i].wrapping_add(1) >> 1),
                };
                o1[i] = h0[i].wrapping_add(x0[i].wrapping_add(d) >> 1);
                if n % 2 == 1 {
                    rest[i] = d;
                }
            }
        } else {
            for i in 0..out_w {
                o1[i] = h0[i].wrapping_add(x0[i]);
            }
        }
    }
    Ok(out)
}

/// Decode one plane of one tile to `tile.width x tile.height` values.
/// `parallel`: the subbands of a wavelet (C-RAW) plane decode in parallel (each has its own
/// offset): a tile row of a CR3 is a handful of tile planes, fewer than the cores.
fn decode_tile_plane(
    img: &Image,
    tile: &Tile,
    plane: usize,
    data: &[u8],
    parallel: bool,
) -> R<Vec<i32>> {
    let pi = &tile.planes[plane];
    let base = tile
        .data_offset
        .checked_add(tile.qp_size)
        .and_then(|v| v.checked_add(tile.extra_size))
        .and_then(|v| v.checked_add(pi.data_offset))
        .ok_or("CRX: data offset")?;
    let src = data.get(base..).ok_or("CRX: plane data out of range")?;
    let first_mode = if pi.supports_partial {
        if pi.rounded_mask > 0 {
            let mask = pi.rounded_mask;
            let mut bits = 1u32;
            if mask & !1 != 0 {
                while mask >> bits != 0 {
                    bits += 1;
                }
            }
            Mode::Rounded { mask, bits }
        } else {
            Mode::Median
        }
    } else {
        Mode::NoRef
    };
    if img.levels == 0 {
        let b = &pi.bands[0];
        if b.width != tile.width || b.height != tile.height {
            return Err("CRX: band geometry");
        }
        if b.data_size == 0 {
            return Ok(vec![0; tile.width * tile.height]);
        }
        let s = src
            .get(b.data_offset..b.data_offset.saturating_add(b.data_size))
            .ok_or("CRX: subband data out of range")?;
        let mut band = Band::new(s, b.width, first_mode);
        let mut out = Vec::with_capacity(tile.width * tile.height);
        for _ in 0..tile.height {
            band.next_line()?;
            out.extend_from_slice(band.values());
        }
        return Ok(out);
    }
    let qs = tile.qsteps.as_deref();
    let q = |level: usize| qs.and_then(|v| v.get(level));
    let nbands = 1 + 3 * img.levels;
    if pi.bands.len() < nbands {
        return Err("CRX: band count");
    }
    let band = |i: usize| {
        if i == 0 {
            decode_band(&pi.bands[0], src, first_mode, q(0))
        } else {
            decode_band(&pi.bands[i], src, Mode::NoRef, q((i - 1) / 3))
        }
    };
    let decoded: Vec<R<Vec<i32>>> = if parallel {
        (0..nbands).into_par_iter().map(band).collect()
    } else {
        (0..nbands).map(band).collect()
    };
    let mut decoded = decoded.into_iter().collect::<R<Vec<_>>>()?;
    let mut ll = std::mem::take(&mut decoded[0]);
    let (mut ll_w, mut ll_h) = (pi.bands[0].width, pi.bands[0].height);
    for level in 0..img.levels {
        let i = 3 * level + 1;
        let hl = std::mem::take(&mut decoded[i]);
        let lh = std::mem::take(&mut decoded[i + 1]);
        let hh = std::mem::take(&mut decoded[i + 2]);
        let (ow, oh) = if level + 1 == img.levels {
            (tile.width, tile.height)
        } else {
            (pi.bands[i + 4].width, pi.bands[i + 3].height)
        };
        ll = idwt_2d(
            &ll,
            &hl,
            &lh,
            &hh,
            [&pi.bands[i], &pi.bands[i + 1], &pi.bands[i + 2]],
            ll_w,
            ll_h,
            ow,
            oh,
            tile.flags,
        )?;
        ll_w = ow;
        ll_h = oh;
    }
    Ok(ll)
}

/// Decode a CRX sample (`sample` = the whole track sample, headers included) into a
/// `width x height` CFA buffer.
pub(crate) fn decode(h: &Header, sample: &[u8], out: &mut [u16], parallel: bool) -> R<()> {
    let hs = h.mdat_hdr_size as usize;
    let hdr = sample.get(..hs).ok_or("CRX: truncated header")?;
    let data = &sample[hs..];
    let img = setup_image(h, hdr, data)?;
    let (w, ht) = (h.width as usize, h.height as usize);
    if out.len() < w * ht {
        return Err("CRX: output buffer");
    }
    let np = h.n_planes as usize;
    let nb = h.n_bits as u32;
    // Plane -> (dy, dx) position within the 2x2 cell.
    let pos: [(usize, usize); 4] = match h.cfa_layout {
        0 => [(0, 0), (0, 1), (1, 0), (1, 1)],
        1 => [(0, 1), (0, 0), (1, 1), (1, 0)],
        2 => [(1, 0), (1, 1), (0, 0), (0, 1)],
        _ => [(1, 1), (1, 0), (0, 1), (0, 0)],
    };
    let mut row0 = 0usize;
    for tr in 0..img.tile_rows {
        let row_tiles = &img.tiles[tr * img.tile_cols..(tr + 1) * img.tile_cols];
        let th = row_tiles[0].height;
        let jobs: Vec<(usize, usize)> = (0..row_tiles.len())
            .flat_map(|t| (0..np).map(move |p| (t, p)))
            .collect();
        let run =
            |&(t, p): &(usize, usize)| decode_tile_plane(&img, &row_tiles[t], p, data, parallel);
        let res: Vec<R<Vec<i32>>> = if parallel {
            jobs.par_iter().map(run).collect()
        } else {
            jobs.iter().map(run).collect()
        };
        let planes = res.into_iter().collect::<R<Vec<_>>>()?;
        // Convert: tile-row of planes -> output rows.
        let rows_per = if np == 4 { 2 } else { 1 };
        let start = row0 * rows_per * w;
        let end = ((row0 + th) * rows_per * w).min(out.len());
        let dst = &mut out[start..end];
        let conv = |(y, chunk): (usize, &mut [u16])| {
            let mut x0 = 0usize;
            for (t, tile) in row_tiles.iter().enumerate() {
                let tw = tile.width;
                if np == 1 {
                    let src = &planes[t][y * tw..(y + 1) * tw];
                    let (med, maxv) = (1i32 << (nb - 1), (1i32 << nb) - 1);
                    for (o, &v) in chunk[x0..x0 + tw].iter_mut().zip(src) {
                        *o = med.wrapping_add(v).clamp(0, maxv) as u16;
                    }
                } else {
                    let line = |p: usize| &planes[t * np + p][y * tw..(y + 1) * tw];
                    if h.enc_type == 3 {
                        let mb = h.median_bits as u32;
                        let med = (1i32 << (mb - 1)) << 10;
                        let maxv = (1i32 << mb) - 1;
                        let (p0, p1, p2, p3) = (line(0), line(1), line(2), line(3));
                        for i in 0..tw {
                            let (a, b, c, d) = (
                                p0[i] as i16 as i32,
                                p1[i] as i16 as i32,
                                p2[i] as i16 as i32,
                                p3[i] as i16 as i32,
                            );
                            let base = med + (a << 10);
                            let g = base - 168 * b - 585 * d;
                            let g = if g < 0 {
                                -(((g.wrapping_abs() + 512) >> 9) & !1)
                            } else {
                                ((g + 512) >> 9) & !1
                            };
                            let vals = [
                                (base + 1510 * d + 512) >> 10,
                                (c + g + 1) >> 1,
                                (g - c + 1) >> 1,
                                (base + 1927 * b + 512) >> 10,
                            ];
                            for (pl, v) in vals.into_iter().enumerate() {
                                let (dy, dx) = pos[pl];
                                chunk[dy * w + 2 * (x0 + i) + dx] = v.clamp(0, maxv) as u16;
                            }
                        }
                    } else {
                        for (pl, &(dy, dx)) in pos.iter().enumerate() {
                            let src = line(pl);
                            let row = &mut chunk[dy * w..(dy + 1) * w];
                            if h.enc_type == 1 {
                                let maxv = (1i32 << (nb - 1)) - 1;
                                let minv = -(1i32 << (nb - 1));
                                for (i, &v) in src.iter().enumerate() {
                                    row[2 * (x0 + i) + dx] = v.clamp(minv, maxv) as i16 as u16;
                                }
                            } else {
                                let (med, maxv) = (1i32 << (nb - 1), (1i32 << nb) - 1);
                                for (i, &v) in src.iter().enumerate() {
                                    row[2 * (x0 + i) + dx] =
                                        med.wrapping_add(v).clamp(0, maxv) as u16;
                                }
                            }
                        }
                    }
                }
                x0 += tw;
            }
        };
        let chunk_len = rows_per * w;
        if parallel {
            dst.par_chunks_mut(chunk_len).enumerate().for_each(conv);
        } else {
            dst.chunks_mut(chunk_len).enumerate().for_each(conv);
        }
        row0 += th;
    }
    if row0 != img.plane_h
        || img
            .tiles
            .iter()
            .take(img.tile_cols)
            .map(|t| t.width)
            .sum::<usize>()
            != img.plane_w
    {
        return Err("CRX: tile geometry");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::EX_COEF;

    /// Re-derive the neighbour extension table from the inverse 5/3 lifting
    /// (`x[2k] = L[k] - (H[k-1] + H[k] + 2) >> 2`, `x[2k+1] = H[k] + (x[2k] + x[2k+2]) >> 1`).
    /// Top-down: the samples a level must output fix the last high / low coefficient it reads,
    /// and that low count is what the next level must output. Bottom-up: a level's low band is
    /// as long as the contiguous run the deeper level can reconstruct from its coefficients.
    fn derive(levels: usize, size: usize) -> [u8; 6] {
        let mut n = vec![size];
        for _ in 0..levels {
            n.push(n.last().unwrap().div_ceil(2));
        }
        let (mut hcount, mut lcount) = (Vec::new(), Vec::new());
        let mut need = size;
        for _ in 0..levels {
            let (m, k) = (need - 1, (need - 1) / 2);
            let (h, l) = if m % 2 == 1 {
                (k + 2, k + 2)
            } else {
                (k + 1, k + 1)
            };
            hcount.push(h);
            lcount.push(l);
            need = l;
        }
        let mut out = [0u8; 6];
        let mut low = lcount[levels - 1];
        for level in (0..levels).rev() {
            let high = hcount[level];
            out[2 * level] = (high - n[level] / 2) as u8;
            out[2 * level + 1] = (low - n[level + 1]) as u8;
            let mut x = 0;
            loop {
                let k = x / 2;
                let ok = if x % 2 == 0 {
                    k < low && k < high
                } else {
                    k < high && k + 1 < low && k + 1 < high
                };
                if !ok {
                    break;
                }
                x += 1;
            }
            low = x;
        }
        out
    }

    #[test]
    fn ex_coef_table_follows_from_the_5_3_synthesis() {
        for (levels, table) in (1..=3).zip(&EX_COEF) {
            for (r, row) in table.iter().enumerate() {
                let used = 2 * levels;
                assert_eq!(
                    row[..used],
                    derive(levels, 64 + r)[..used],
                    "levels {levels}, size & 7 = {r}"
                );
            }
        }
    }

    #[test]
    fn refused_headers_report_size_and_reason() {
        let mut b = [0u8; 40];
        b[8..12].copy_from_slice(&3000u32.to_be_bytes());
        b[12..16].copy_from_slice(&2000u32.to_be_bytes());
        b[24] = 14;
        b[25] = 3 << 4;
        let r = super::refused(&b).unwrap();
        assert_eq!((r.width, r.height, r.bits), (3000, 2000, 14));
        assert!(r.why.contains("sRAW"));
        b[25] = 4 << 4;
        assert!(super::refused(&b).unwrap().why.contains("header"));
        assert!(super::refused(&b[..39]).is_none());
    }
}
