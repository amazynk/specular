//! Context index derivations for residual coding (clauses 9.3.4.2.3 to .2.7).

use super::state::Dec;
use crate::cabac::off;
use crate::error::{Error, Result};
use crate::scan::SUB_RASTER;

/// `ctxIdxMap` for `sig_coeff_flag` in a 4x4 transform block (Table 9-39),
/// by raster position `(yC << 2) | xC`.
const SIG_MAP_4X4: [u8; 16] = [0, 1, 4, 5, 2, 3, 4, 5, 6, 6, 8, 8, 7, 7, 8, 8];

/// `sigCtx` before the size and colour offsets, by `prevCsbf` and raster
/// position `(yP << 2) | xP` inside the sub-block (clause 9.3.4.2.5).
const SIG_PATTERN: [[u8; 16]; 4] = [
    [2, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [2, 2, 2, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 1, 0, 0, 2, 1, 0, 0, 2, 1, 0, 0, 2, 1, 0, 0],
    [2; 16],
];

/// Every position shares one context under `transform_skip_context_enabled_flag`.
const FLAT: [u8; 16] = [0; 16];

/// The `sigCtx` tables by scan position instead of raster position:
/// `SIG_SCAN[scanIdx][t][n]` for `t` the four `prevCsbf` patterns, the 4x4
/// map and the flat table, so that a coefficient costs one lookup.
static SIG_SCAN: [[[u8; 16]; 6]; 3] = sig_scan();

const fn sig_scan() -> [[[u8; 16]; 6]; 3] {
    let tables = [
        SIG_PATTERN[0],
        SIG_PATTERN[1],
        SIG_PATTERN[2],
        SIG_PATTERN[3],
        SIG_MAP_4X4,
        FLAT,
    ];
    let mut out = [[[0u8; 16]; 6]; 3];
    let mut s = 0;
    while s < 3 {
        let mut t = 0;
        while t < 6 {
            let mut n = 0;
            while n < 16 {
                out[s][t][n] = tables[t][SUB_RASTER[s][n] as usize];
                n += 1;
            }
            t += 1;
        }
        s += 1;
    }
    out
}

/// Derives `scanIdx` (clause 7.4.9.11): 0 diagonal, 1 horizontal, 2 vertical.
pub fn scan_index(d: &Dec<'_>, log2_size: usize, c_idx: usize, pred_mode: u8) -> usize {
    let mode_dependent =
        log2_size == 2 || (log2_size == 3 && c_idx == 0) || (log2_size == 3 && d.cat == 3);
    if !mode_dependent {
        return 0;
    }
    if (6..=14).contains(&pred_mode) {
        2
    } else if (22..=30).contains(&pred_mode) {
        1
    } else {
        0
    }
}

/// Decodes `last_sig_coeff_x/y_prefix` and their suffixes (clause 9.3.3.10).
pub fn last_position(d: &mut Dec<'_>, log2_size: usize, c_idx: usize) -> (usize, usize) {
    let (ctx_offset, ctx_shift) = if c_idx == 0 {
        (
            3 * (log2_size - 2) + ((log2_size - 1) >> 2),
            (log2_size + 1) >> 2,
        )
    } else {
        (15, log2_size - 2)
    };
    let c_max = (log2_size << 1) - 1;
    let mut prefix = [0usize; 2];
    for (k, base) in [off::LAST_X, off::LAST_Y].into_iter().enumerate() {
        let mut v = 0usize;
        while v < c_max {
            let ctx = base + ctx_offset + (v >> ctx_shift);
            if d.cab.decision(ctx) == 0 {
                break;
            }
            v += 1;
        }
        prefix[k] = v;
    }
    let mut out = [0usize; 2];
    for k in 0..2 {
        out[k] = if prefix[k] > 3 {
            let bits = (prefix[k] >> 1) - 1;
            let suffix = d.cab.bypass_bits(bits as u32) as usize;
            ((1usize << bits) * (2 + (prefix[k] & 1))) + suffix
        } else {
            prefix[k]
        };
    }
    (out[0], out[1])
}

/// The `sig_coeff_flag` context derivation for one sub-block (clause 9.3.4.2.5).
///
/// Everything that is fixed across the coefficients of a sub-block — the
/// `prevCsbf` pattern, the scan, the block size and colour offsets — is
/// settled once here, leaving one table lookup per coefficient.
pub struct SigCtx {
    /// `sigCtx` before the offsets, by scan position.
    table: &'static [u8; 16],
    /// Context index of a `table` entry of zero.
    base: usize,
    /// Context index of the DC coefficient.
    dc: usize,
}

impl SigCtx {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        d: &Dec<'_>,
        log2_size: usize,
        c_idx: usize,
        xs: usize,
        ys: usize,
        sb_n: usize,
        scan_idx: usize,
        ts_ctx: bool,
    ) -> SigCtx {
        let tables = &SIG_SCAN[scan_idx];
        let chroma = if c_idx > 0 { 27 } else { 0 };
        if ts_ctx {
            let base = off::SIG + if c_idx == 0 { 42 } else { 43 };
            return SigCtx {
                table: &tables[5],
                base,
                dc: base,
            };
        }
        if log2_size == 2 {
            let base = off::SIG + chroma;
            return SigCtx {
                table: &tables[4],
                base,
                dc: base + usize::from(tables[4][0]),
            };
        }
        let mut prev = 0usize;
        if xs + 1 < sb_n {
            prev += d.csbf[ys * sb_n + xs + 1] as usize;
        }
        if ys + 1 < sb_n {
            prev += (d.csbf[(ys + 1) * sb_n + xs] as usize) << 1;
        }
        let add = if c_idx == 0 {
            let size = if log2_size == 3 {
                if scan_idx == 0 { 9 } else { 15 }
            } else {
                21
            };
            usize::from(xs + ys > 0) * 3 + size
        } else {
            chroma + if log2_size == 3 { 9 } else { 12 }
        };
        let table = &tables[prev & 3];
        let base = off::SIG + add;
        SigCtx {
            table,
            base,
            dc: if xs + ys == 0 {
                off::SIG + chroma
            } else {
                base + usize::from(table[0])
            },
        }
    }

    /// Context index of scan position `m`, which is not 0.
    #[inline(always)]
    pub fn at(&self, m: usize) -> usize {
        self.base + usize::from(self.table[m & 15])
    }

    /// Context index of scan position 0.
    #[inline(always)]
    pub fn dc(&self) -> usize {
        self.dc
    }
}

/// Decodes `coeff_abs_level_remaining` (clause 9.3.3.11).
///
/// `log2_range` is `log2TransformRange` of the block's colour component; it
/// selects the limited EGk suffix (clause 9.3.3.4) when it is not 15, which
/// only `extended_precision_processing_flag` makes it.
///
/// The result saturates at `u32::MAX`; the caller clips every level to the
/// coefficient range anyway. Codes too long for any conforming stream fail.
pub fn read_remaining(d: &mut Dec<'_>, rice: u32, log2_range: u32) -> Result<u32> {
    if rice > 30 {
        return Err(Error::InvalidData(
            "coeff_abs_level_remaining Rice parameter",
        ));
    }
    // TR prefix with cMax = 4 << cRiceParam: up to four ones.
    let mut prefix = 0u32;
    while prefix < 4 && d.cab.bypass() == 1 {
        prefix += 1;
    }
    if prefix < 4 {
        let suffix = d.cab.bypass_bits(rice);
        return Ok((prefix << rice) + suffix);
    }
    let k = rice + 1;
    let base = 4u64 << rice;
    let v = if log2_range == 15 {
        // EGk with k = cRiceParam + 1 (clause 9.3.3.3).
        let mut ones = 0u32;
        while d.cab.bypass() == 1 {
            ones += 1;
            if ones + k > 32 {
                return Err(Error::InvalidData(
                    "coeff_abs_level_remaining prefix too long",
                ));
            }
        }
        (((1u64 << ones) - 1) << k) + u64::from(d.cab.bypass_bits(ones + k))
    } else {
        limited_egk(d, k, log2_range)?
    };
    Ok((base + v).min(u64::from(u32::MAX)) as u32)
}

/// The limited k-th order Exp-Golomb suffix of clause 9.3.3.4, used only
/// under `extended_precision_processing_flag`.
#[cold]
#[inline(never)]
fn limited_egk(d: &mut Dec<'_>, k: u32, log2_range: u32) -> Result<u64> {
    let max_ext = 28u32.saturating_sub(log2_range);
    let mut ones = 0u32;
    while ones < max_ext && d.cab.bypass() == 1 {
        ones += 1;
    }
    let escape = if ones == max_ext {
        log2_range
    } else {
        ones + k
    };
    if escape > 32 {
        return Err(Error::InvalidData(
            "coeff_abs_level_remaining escape too long",
        ));
    }
    Ok((((1u64 << ones) - 1) << k) + u64::from(d.cab.bypass_bits(escape)))
}
