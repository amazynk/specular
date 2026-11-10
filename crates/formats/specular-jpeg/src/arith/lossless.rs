//! Arithmetic-coded lossless frames (SOF11), written from ITU-T T.81 Annex H: the predictors of
//! H.1.2.1 (selections 1–7, point transform, 2–16 bit samples, differences modulo 2^16) and the
//! two-dimensional statistical model of H.1.2.3: the DC difference coder of F.1.4.4.1 with its
//! decision tree, conditioned on the difference categories (DAC bounds L and U) of the samples to
//! the left and above (25 states of four bins, Figure H.2), and two sets of magnitude bins chosen
//! by whether the difference above is large (158 bins per table).
//!
//! Every sample depends on its left and upper neighbours and on statistics adapted by all the
//! samples before it, so a scan decodes serially (restart intervals would allow bands, but no
//! encoder writes such files in practice). Samples go into per-component planes padded to whole
//! MCUs, and the output stage is the Huffman lossless fallback's ([`crate::fallback::finish`]):
//! full-size RGBA from the raw samples (no colour transform), reduced to 8 bits like
//! `jpeg-decoder`'s output, then box-downscaled and cropped. A region decode stops each scan
//! below the region's last row. Corrupt data stops the scan and flags the decode truncated.

use std::sync::atomic::{AtomicBool, Ordering};

use super::{Broken, CUT_FILL, Coder};
use crate::error::{JpegError, Result};
use crate::fallback::{finish, out_rect, to_8bit};
use crate::geom::{alloc_i16, alloc_zeroed};
use crate::marker::{Parsed, Scan};
use crate::{DecodedPlanes, Region, Scale};

/// Statistics bins per conditioning table: 25 x 4 + 2 x 29 = 158 (H.1.2.3.2), rounded up to a
/// power of two so bin indices are masked, not checked.
const BINS: usize = 256;
/// First magnitude bin when the difference above is zero or small / large.
const X1_SMALL: usize = 100;
const X1_LARGE: usize = 129;
/// Zero bytes past a scan's data that its final decisions may read (FLUSH, D.1.8, leaves out
/// the trailing zero bytes of the code register).
const END_FILL: u32 = 8;

/// One component's samples (at the reduced precision `P - Pt`) and the difference category
/// (0 zero, 1 +small, 2 -small, 3 +large, 4 -large) coded for each, padded to whole MCUs.
struct CompPlane {
    stride: usize,
    /// Samples (and categories) actually covering the component.
    cw: usize,
    ch: usize,
    samples: Vec<i16>,
    cat: Vec<u8>,
    /// Point transform of the scan that coded the component.
    pt: u8,
}

/// Decode one difference (F.1.4.4.1 with Table H.3: S0 = `s0`, magnitude bins from `x1`), and
/// its category under the bounds `l`, `u`.
#[inline(always)]
fn diff(
    d: &mut Coder,
    st: &mut [u8; BINS],
    s0: usize,
    x1: usize,
    l: u8,
    u: u8,
) -> std::result::Result<(i32, u8), Broken> {
    if !d.decode(&mut st[s0 & (BINS - 1)]) {
        return Ok((0, 0));
    }
    let neg = d.decode(&mut st[(s0 + 1) & (BINS - 1)]);
    let mut i = s0 + 2 + neg as usize;
    let mut mag: i32 = 0;
    if d.decode(&mut st[i & (BINS - 1)]) {
        mag = 1;
        i = x1;
        while d.decode(&mut st[i & (BINS - 1)]) {
            mag <<= 1;
            i += 1;
            if mag >= 0x8000 || i >= x1 + 15 {
                return Err(Broken);
            }
        }
    }
    // Categories as for DC differences (F.1.4.4.1.2), on the magnitude category.
    let cat = if mag < (1i32 << l) >> 1 {
        0
    } else if mag > (1i32 << u) >> 1 {
        3 + neg as u8
    } else {
        1 + neg as u8
    };
    let mut v = mag;
    i += 14;
    let mut bit = mag >> 1;
    while bit != 0 {
        v |= bit & (d.decode(&mut st[i & (BINS - 1)]) as i32).wrapping_neg();
        bit >>= 1;
    }
    v += 1;
    Ok((if neg { -v } else { v }, cat))
}

/// Every scan of a SOF11 frame down to the MCU row holding image row `rows - 1`: the
/// component planes and whether the data ended early or broke off.
fn decode_planes(
    p: &Parsed,
    data: &[u8],
    rows: usize,
    cancel: Option<&AtomicBool>,
) -> Result<(Vec<CompPlane>, bool)> {
    let f = &p.frame;
    let (w, h) = (f.width as usize, f.height as usize);
    let (hmax, vmax) = (f.max_h as usize, f.max_v as usize);
    let (mx, my) = (w.div_ceil(hmax), h.div_ceil(vmax));
    // Refuse absurd allocations for files that cannot hold the declared samples (an
    // arithmetic-coded sample can cost far less than a bit, so only against tiny files).
    let samples: u64 = f
        .comps
        .iter()
        .map(|c| (mx * c.h as usize) as u64 * (my * c.v as usize) as u64)
        .sum();
    let avail_bits = (data.len().saturating_sub(p.info.header_len) as u64) * 8;
    if samples * 3 > (64 << 20) && samples > avail_bits.saturating_mul(1 << 20) {
        return Err(JpegError::Truncated(
            "file far too short for the declared image size",
        ));
    }
    let mut planes = Vec::with_capacity(f.comps.len());
    for c in &f.comps {
        let (pw, ph) = (mx * c.h as usize, my * c.v as usize);
        planes.push(CompPlane {
            stride: pw,
            cw: (w * c.h as usize).div_ceil(hmax),
            ch: (h * c.v as usize).div_ceil(vmax),
            samples: alloc_i16(pw * ph)?,
            cat: alloc_zeroed(pw * ph)?,
            pt: 0,
        });
    }
    let mut truncated = false;
    for s in &p.scans {
        truncated |= decode_scan(p, data, s, &mut planes, rows, cancel)?;
    }
    tracing::debug!(target: "jpeg", scans = p.scans.len(), truncated, "arithmetic lossless");

    Ok((planes, truncated))
}

/// One component's samples: width, height, row-major values.
pub(crate) type Plane16 = (usize, usize, Vec<u16>);

/// A SOF11 frame at full precision: per component its samples (`cw` x `ch`, row-major, the
/// point transform undone) and whether the data ended early.
pub(crate) fn samples(
    p: &Parsed,
    data: &[u8],
    cancel: Option<&AtomicBool>,
) -> Result<(Vec<Plane16>, bool)> {
    let (planes, truncated) = decode_planes(p, data, p.frame.height as usize, cancel)?;
    let mut out = Vec::with_capacity(planes.len());
    for pl in planes {
        let mut v = Vec::new();
        v.try_reserve_exact(pl.cw * pl.ch)
            .map_err(|_| JpegError::TooLarge((pl.cw * pl.ch * 2) as u64))?;
        for row in pl.samples.chunks_exact(pl.stride).take(pl.ch) {
            v.extend(row[..pl.cw].iter().map(|&s| (s as u16) << pl.pt));
        }
        out.push((pl.cw, pl.ch, v));
    }
    Ok((out, truncated))
}

/// Decode a SOF11 frame (`p` parsed with all scans) at `scale`, cropped to `region`.
pub(crate) fn decode(
    p: &Parsed,
    data: &[u8],
    scale: Scale,
    region: Option<Region>,
    cancel: Option<&AtomicBool>,
) -> Result<DecodedPlanes> {
    let f = &p.frame;
    let (w, h) = (f.width as usize, f.height as usize);
    let (_, _, _, y1) = out_rect(w, h, scale, region)?;
    // Image rows the output needs.
    let rows = (y1 * scale as usize).min(h);
    let (hmax, vmax) = (f.max_h as usize, f.max_v as usize);
    let (planes, truncated) = decode_planes(p, data, rows, cancel)?;

    let nc = f.comps.len();
    let precision = f.precision as u32;
    // Per component: plane column of every image column, plane row offset of every image row.
    let cols: Vec<Vec<u32>> = f
        .comps
        .iter()
        .map(|c| (0..w).map(|x| (x * c.h as usize / hmax) as u32).collect())
        .collect();
    let row_off: Vec<Vec<usize>> = f
        .comps
        .iter()
        .zip(&planes)
        .map(|(c, pl)| {
            (0..h)
                .map(|y| y * c.v as usize / vmax * pl.stride)
                .collect()
        })
        .collect();
    let sample = |i: usize| -> u8 {
        let (px, c) = (i / nc, i % nc);
        let (x, y) = (px % w, px / w);
        let pl = &planes[c];
        let v = (pl.samples[row_off[c][y] + cols[c][x] as usize] as u16) << pl.pt;
        if precision == 8 {
            v as u8
        } else {
            to_8bit(v, precision)
        }
    };
    finish(w, h, nc, sample, scale, region, truncated)
}

/// Decode one scan into `planes`, down to the MCU row holding image row `rows - 1`. Returns
/// whether the data ended early or broke off.
fn decode_scan(
    p: &Parsed,
    data: &[u8],
    s: &Scan,
    planes: &mut [CompPlane],
    rows: usize,
    cancel: Option<&AtomicBool>,
) -> Result<bool> {
    let f = &p.frame;
    let predictor = s.ss;
    if !(1..=7).contains(&predictor) {
        return Err(JpegError::Malformed("lossless predictor"));
    }
    let pt = s.al;
    if pt >= f.precision {
        return Err(JpegError::Malformed("lossless point transform"));
    }
    let interleaved = s.comps.len() > 1;
    let (hmax, vmax) = (f.max_h as usize, f.max_v as usize);
    // Units per row, unit rows (MCUs; samples of a single-component scan), and the unit rows
    // the output needs.
    let (ux, total, need) = if interleaved {
        let (mx, my) = (
            (f.width as usize).div_ceil(hmax),
            (f.height as usize).div_ceil(vmax),
        );
        (mx, my, rows.div_ceil(vmax).min(my))
    } else {
        let pl = &planes[s.comps[0].ci];
        let v = f.comps[s.comps[0].ci].v as usize;
        (pl.cw, pl.ch, (rows * v).div_ceil(vmax).min(pl.ch))
    };
    let ri = s.restart_interval as usize;
    if ri > 0 && !ri.is_multiple_of(ux) {
        // H.1.1: a whole number of MCU rows.
        return Err(JpegError::Malformed("lossless restart interval"));
    }
    let init = 1i32 << (f.precision - pt - 1);
    let end = s.data_end.min(data.len());
    let open_end = end >= data.len();
    let mut d = Coder::new(data, s.data_start, end);
    let mut st = [[0u8; BINS]; 4];
    let mut left = ri;
    // Per scan component: (h, v) of its unit, conditioning bounds, the first sample row of the
    // current restart interval.
    let geo: Vec<(usize, usize, u8, u8)> = s
        .comps
        .iter()
        .map(|sc| {
            let fc = &f.comps[sc.ci];
            let (nh, nv) = if interleaved {
                (fc.h as usize, fc.v as usize)
            } else {
                (1, 1)
            };
            (nh, nv, s.arith.dc_l[sc.td], s.arith.dc_u[sc.td])
        })
        .collect();
    for sc in &s.comps {
        planes[sc.ci].pt = pt;
    }
    let mut first_row = vec![0usize; s.comps.len()];
    for row in 0..need {
        if let Some(c) = cancel
            && c.load(Ordering::Relaxed)
        {
            return Err(JpegError::Cancelled);
        }
        if ri > 0 {
            if left == 0 {
                d = d.restarted();
                st = [[0u8; BINS]; 4];
                left = ri;
                for (k, g) in geo.iter().enumerate() {
                    first_row[k] = row * g.1;
                }
            }
            left -= ux;
        }
        for u in 0..ux {
            for (k, sc) in s.comps.iter().enumerate() {
                let (nh, nv, l, up) = geo[k];
                let pl = &mut planes[sc.ci];
                let stats = &mut st[sc.td];
                for by in 0..nv {
                    let y = row * nv + by;
                    let first = y == first_row[k];
                    for bx in 0..nh {
                        let x = u * nh + bx;
                        let i = y * pl.stride + x;
                        let ca = if x == 0 { 0 } else { pl.cat[i - 1] as usize };
                        let cb = if first {
                            0
                        } else {
                            pl.cat[i - pl.stride] as usize
                        };
                        let x1 = if cb >= 3 { X1_LARGE } else { X1_SMALL };
                        let Ok((v, cat)) = diff(&mut d, stats, 20 * ca + 4 * cb, x1, l, up) else {
                            tracing::debug!(target: "jpeg", row, "arithmetic lossless: corrupt data, scan stopped");
                            return Ok(true);
                        };
                        let px = if first {
                            if x == 0 {
                                init
                            } else {
                                pl.samples[i - 1] as u16 as i32
                            }
                        } else if x == 0 {
                            pl.samples[i - pl.stride] as u16 as i32
                        } else {
                            let ra = pl.samples[i - 1] as u16 as i32;
                            let rb = pl.samples[i - pl.stride] as u16 as i32;
                            let rc = pl.samples[i - pl.stride - 1] as u16 as i32;
                            match predictor {
                                1 => ra,
                                2 => rb,
                                3 => rc,
                                4 => ra + rb - rc,
                                5 => ra + ((rb - rc) >> 1),
                                6 => rb + ((ra - rc) >> 1),
                                _ => (ra + rb) >> 1,
                            }
                        };
                        // Modulo 2^16 (H.2.2).
                        pl.samples[i] = px.wrapping_add(v) as u16 as i16;
                        pl.cat[i] = cat;
                    }
                }
            }
        }
        if open_end && d.filled() > CUT_FILL {
            tracing::debug!(target: "jpeg", row, "arithmetic lossless: data ends early");
            return Ok(true);
        }
    }
    // A difference can cost far less than a bit, so a cut file may decode to the end on the
    // zeros fed past the data: at the end of a scan, more than an encoder's flush leaves out
    // (the trailing zeros of the code register) means the data was cut.
    if open_end && need == total && d.filled() > END_FILL {
        tracing::debug!(target: "jpeg", "arithmetic lossless: data ends before the scan does");
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
