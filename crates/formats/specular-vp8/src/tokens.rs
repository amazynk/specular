//! DCT/WHT coefficient tokens (RFC 6386 section 13), dequantised as they are read.

use crate::boolcoder::BoolDecoder;
use crate::header::Dequant;
use crate::modes::MbInfo;
use crate::tables::{COEFF_BANDS, PCAT3, PCAT4, PCAT5, PCAT6, ZIGZAG};

type PlaneProbs = [[[u8; 11]; 3]; 8];

/// Block coefficient kinds: nothing to add, DC only, or a full transform.
pub(crate) const NONE: u8 = 0;
pub(crate) const DC_ONLY: u8 = 1;
pub(crate) const FULL: u8 = 2;

/// One macroblock's dequantised coefficients in raster order: 16 Y, 4 U, 4 V, then Y2.
#[derive(Clone)]
pub(crate) struct MbCoeffs {
    pub c: [i16; 400],
    pub kinds: [u8; 25],
}

impl Default for MbCoeffs {
    fn default() -> Self {
        MbCoeffs {
            c: [0; 400],
            kinds: [NONE; 25],
        }
    }
}

impl MbCoeffs {
    /// Zeroes the blocks that hold coefficients.
    pub(crate) fn clear(&mut self) {
        for b in 0..25 {
            if self.kinds[b] != NONE {
                self.c[b * 16..b * 16 + 16].fill(0);
                self.kinds[b] = NONE;
            }
        }
    }
}

#[inline(always)]
fn extra(d: &mut BoolDecoder<'_>, probs: &[u8]) -> i32 {
    let mut v = 0;
    for &p in probs {
        if p == 0 {
            break;
        }
        v = (v << 1) | d.read(p) as i32;
    }
    v
}

/// Reads one block's tokens from position `first`. Returns 0 when the block ends at once,
/// else the position after the last token read.
#[inline(always)]
fn block(
    d: &mut BoolDecoder<'_>,
    probs: &PlaneProbs,
    ctx: usize,
    first: usize,
    dc: i32,
    ac: i32,
    out: &mut [i16],
) -> usize {
    let mut i = first;
    let mut p = &probs[COEFF_BANDS[i] as usize][ctx];
    if !d.read(p[0]) {
        return 0;
    }
    loop {
        if !d.read(p[1]) {
            // DCT_0: the next token can't be the end of the block.
            i += 1;
            if i == 16 {
                return 16;
            }
            p = &probs[COEFF_BANDS[i] as usize][0];
            continue;
        }
        let (v, next_ctx) = if !d.read(p[2]) {
            (1, 1)
        } else {
            let v = if !d.read(p[3]) {
                if !d.read(p[4]) {
                    2
                } else {
                    3 + d.read(p[5]) as i32
                }
            } else if !d.read(p[6]) {
                if !d.read(p[7]) {
                    5 + d.read(159) as i32
                } else {
                    7 + ((d.read(165) as i32) << 1) + d.read(145) as i32
                }
            } else if !d.read(p[8]) {
                if !d.read(p[9]) {
                    11 + extra(d, &PCAT3)
                } else {
                    19 + extra(d, &PCAT4)
                }
            } else if !d.read(p[10]) {
                35 + extra(d, &PCAT5)
            } else {
                67 + extra(d, &PCAT6)
            };
            (v, 2)
        };
        let v = if d.flag() { -v } else { v };
        let q = if i == 0 { dc } else { ac };
        // The reference decoder keeps dequantised values in 16 bits.
        out[ZIGZAG[i] as usize] = v.wrapping_mul(q) as i16;
        i += 1;
        if i == 16 {
            return 16;
        }
        p = &probs[COEFF_BANDS[i] as usize][next_ctx];
        if !d.read(p[0]) {
            return i;
        }
    }
}

/// Nonzero-ness of the blocks along a macroblock edge: 4 Y, 2 U, 2 V, Y2.
pub(crate) type Ctx = [u8; 9];

/// Reads a macroblock's coefficients. Returns whether any block had tokens.
pub(crate) fn read_mb(
    d: &mut BoolDecoder<'_>,
    probs: &[PlaneProbs; 4],
    mb: &MbInfo,
    dq: &Dequant,
    above: &mut Ctx,
    left: &mut Ctx,
    out: &mut MbCoeffs,
) -> bool {
    let mut any = false;
    let (y_plane, first) = if mb.has_y2() {
        let ctx = (above[8] + left[8]) as usize;
        let n = block(
            d,
            &probs[1],
            ctx,
            0,
            dq[1][0],
            dq[1][1],
            &mut out.c[384..400],
        );
        let nz = (n > 0) as u8;
        above[8] = nz;
        left[8] = nz;
        out.kinds[24] = match n {
            0 => NONE,
            1 => DC_ONLY,
            _ => FULL,
        };
        any |= n > 0;
        (0, 1)
    } else {
        (3, 0)
    };
    for b in 0..16 {
        let (x, y) = (b & 3, b >> 2);
        let ctx = (above[x] + left[y]) as usize;
        let n = block(
            d,
            &probs[y_plane],
            ctx,
            first,
            dq[0][0],
            dq[0][1],
            &mut out.c[b * 16..b * 16 + 16],
        );
        let nz = (n > 0) as u8;
        above[x] = nz;
        left[y] = nz;
        out.kinds[b] = if n == 0 {
            NONE
        } else if n == 1 {
            DC_ONLY
        } else {
            FULL
        };
        any |= n > 0;
    }
    for b in 16..24 {
        // U: contexts 4, 5; V: 6, 7.
        let base = if b < 20 { 4 } else { 6 };
        let (x, y) = (base + (b & 1), base + ((b >> 1) & 1));
        let ctx = (above[x] + left[y]) as usize;
        let n = block(
            d,
            &probs[2],
            ctx,
            0,
            dq[2][0],
            dq[2][1],
            &mut out.c[b * 16..b * 16 + 16],
        );
        let nz = (n > 0) as u8;
        above[x] = nz;
        left[y] = nz;
        out.kinds[b] = match n {
            0 => NONE,
            1 => DC_ONLY,
            _ => FULL,
        };
        any |= n > 0;
    }
    any
}

/// Clears the contexts of a macroblock without coefficients (the Y2 ones only when it has
/// a Y2 block).
pub(crate) fn skip_mb(mb: &MbInfo, above: &mut Ctx, left: &mut Ctx) {
    above[..8].fill(0);
    left[..8].fill(0);
    if mb.has_y2() {
        above[8] = 0;
        left[8] = 0;
    }
}
