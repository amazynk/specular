//! Coefficient-buffer decoding of multi-scan sequential JPEGs and 12-bit data, plus the scan
//! planning shared with the progressive pipeline in [`crate::prog`] (which `decode` hands
//! progressive frames to). Coefficients live in per-component DC and AC arrays; a final
//! parallel pass runs the IDCT over the region. At 1/8 scale only DC arrays are allocated.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::error::{JpegError, Result};
use crate::geom::{OutGeom, alloc_i16};
use crate::huffman::HuffTable;
use crate::idct::{Dst, Simd, dc_only_value, dc_only_value12, idct_12bit, idct_block};
use crate::marker::{Parsed, Scan};
use crate::sequential::{Checkpoint, DecState, decode_block, skip_block};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    DcFirst,
    DcRefine,
    AcFirst,
    AcRefine,
    /// Sequential scan: full DC + AC per block.
    Seq,
}

/// Stored block window of one component (block coordinates).
#[derive(Clone, Debug)]
pub(crate) struct Window {
    pub bx: Range<u32>,
    pub by: Range<u32>,
}

impl Window {
    #[inline(always)]
    pub fn idx(&self, bx: u32, by: u32) -> Option<usize> {
        if bx >= self.bx.start && bx < self.bx.end && by >= self.by.start && by < self.by.end {
            Some(
                ((by - self.by.start) * (self.bx.end - self.bx.start) + (bx - self.bx.start))
                    as usize,
            )
        } else {
            None
        }
    }
    pub fn len(&self) -> usize {
        (self.bx.end - self.bx.start) as usize * (self.by.end - self.by.start) as usize
    }
}

/// Per-scan row checkpoints (index for region decoding of coefficient-path files).
pub(crate) type ScanIndex = Vec<Vec<Checkpoint>>;

pub(crate) struct CoefParams<'c> {
    pub simd: Simd,
    pub threads: usize,
    pub cancel: Option<&'c AtomicBool>,
    pub index: Option<&'c ScanIndex>,
    pub want_index: bool,
    /// Column checkpoints of `index` (progressive region decodes, see [`crate::cols`]).
    pub cols: Option<&'c crate::cols::ColCache>,
    /// Skip the final IDCT (index building only).
    pub no_output: bool,
    /// Coefficient buffers to reuse and return (consecutive stripes of one decode, see
    /// `crate::coef_striped`): freeing and reallocating them per stripe costs page faults
    /// that serialize the stripe's threads.
    pub reuse: Option<&'c std::sync::Mutex<Vec<Vec<i16>>>>,
}

pub(crate) struct CoefOutcome {
    pub truncated: bool,
    pub index: Option<ScanIndex>,
}

/// One component of a scan: (frame comp index, h, v, dc table, ac table).
pub(crate) type PlanComp<'a> = (
    usize,
    u32,
    u32,
    Option<&'a HuffTable>,
    Option<&'a HuffTable>,
);

pub(crate) struct ScanPlan<'a> {
    pub scan: &'a Scan,
    pub si: usize,
    pub kind: Kind,
    pub comps: Vec<PlanComp<'a>>,
    pub interleaved: bool,
    pub units_x: u32,
    pub rows: u32,
    /// Rows to decode.
    pub need: Range<u32>,
}

pub(crate) fn plan_scans<'a>(
    p: &'a Parsed,
    g: &OutGeom,
    dc_only: bool,
    full_rows: bool,
) -> Vec<ScanPlan<'a>> {
    let f = &p.frame;
    let mut out = Vec::new();
    for (si, s) in p.scans.iter().enumerate() {
        let kind = if !f.progressive {
            Kind::Seq
        } else if s.ss == 0 {
            if s.ah == 0 {
                Kind::DcFirst
            } else {
                Kind::DcRefine
            }
        } else if s.ah == 0 {
            Kind::AcFirst
        } else {
            Kind::AcRefine
        };
        if dc_only && matches!(kind, Kind::AcFirst | Kind::AcRefine) {
            continue;
        }
        let interleaved = s.comps.len() > 1;
        let comps = s
            .comps
            .iter()
            .map(|sc| {
                let fc = &f.comps[sc.ci];
                (
                    sc.ci,
                    fc.h,
                    fc.v,
                    s.dc[sc.td].as_deref(),
                    s.ac[sc.ta].as_deref(),
                )
            })
            .collect::<Vec<_>>();
        let (units_x, rows, need) = if interleaved {
            (
                f.mcus_x,
                f.mcus_y,
                if full_rows { 0..g.my1 } else { g.my0..g.my1 },
            )
        } else {
            let fc = &f.comps[s.comps[0].ci];
            let r0 = if full_rows {
                0
            } else {
                (g.my0 * fc.v).min(fc.bh)
            };
            let r1 = (g.my1 * fc.v).min(fc.bh);
            (fc.bw, fc.bh, r0..r1)
        };
        out.push(ScanPlan {
            scan: s,
            si,
            kind,
            comps,
            interleaved,
            units_x,
            rows,
            need,
        });
    }
    out
}

/// Mutable coefficient targets for one decoding chain.
struct Targets<'s> {
    /// DC arrays per frame component (None if this chain doesn't own DC).
    dc: [Option<&'s mut [i16]>; 4],
    /// AC arrays (64 per block) per frame component.
    ac: [Option<&'s mut [i16]>; 4],
}

struct Ctx<'a, 'c> {
    data: &'a [u8],
    win: &'c [Window],
    cancel: Option<&'c AtomicBool>,
    dc_only: bool,
}

/// Decode `plan.need` rows of one scan into `tg`.
fn decode_scan(
    cx: &Ctx,
    plan: &ScanPlan,
    tg: &mut Targets,
    start: Checkpoint,
    first_row: u32,
    mut record: Option<&mut Vec<Checkpoint>>,
) -> Result<bool> {
    let s = plan.scan;
    let ri = s.restart_interval;
    let mut st = DecState::from_cp(cx.data, &start);
    let mut scratch = [0i16; 64];
    let row_range = if record.is_some() {
        0..plan.rows
    } else {
        first_row..plan.need.end
    };
    for row in row_range {
        if let Some(r) = record.as_deref_mut() {
            r.push(st.checkpoint());
        }
        if row >= plan.need.end {
            // Index building past the region: keep parsing (coefficients still needed for
            // refinement history only within stored windows).
        }
        if let Some(c) = cx.cancel
            && c.load(Ordering::Relaxed)
        {
            return Err(JpegError::Cancelled);
        }
        for ux in 0..plan.units_x {
            st.before_mcu(ri);
            for (slot, &(ci, h, v, dct, act)) in plan.comps.iter().enumerate() {
                let (nh, nv) = if plan.interleaved { (h, v) } else { (1, 1) };
                for bv in 0..nv {
                    for bh in 0..nh {
                        let (bx, by) = if plan.interleaved {
                            (ux * h + bh, row * v + bv)
                        } else {
                            (ux, row)
                        };
                        let idx = cx.win[ci].idx(bx, by);
                        let pred = &mut st.dc[slot];
                        match plan.kind {
                            Kind::Seq => {
                                let (dct, act) = (dct.unwrap(), act.unwrap());
                                if cx.dc_only {
                                    skip_block(&mut st.br, dct, act, pred);
                                    if let (Some(i), Some(d)) = (idx, tg.dc[ci].as_deref_mut()) {
                                        d[i] = *pred as i16;
                                    }
                                } else {
                                    let has_ac =
                                        decode_block(&mut st.br, dct, act, pred, &mut scratch);
                                    if let Some(i) = idx {
                                        if let Some(d) = tg.dc[ci].as_deref_mut() {
                                            d[i] = scratch[0];
                                        }
                                        if let Some(a) = tg.ac[ci].as_deref_mut() {
                                            let b = &mut a[i * 64..i * 64 + 64];
                                            b.copy_from_slice(&scratch);
                                            b[0] = 0;
                                        }
                                    }
                                    if has_ac {
                                        scratch = [0; 64];
                                    }
                                    scratch[0] = 0;
                                }
                            }
                            // Progressive scans are decoded by `crate::prog`.
                            Kind::DcFirst | Kind::DcRefine | Kind::AcFirst | Kind::AcRefine => {}
                        }
                    }
                }
            }
        }
        if st.br.overrun() {
            return Ok(true);
        }
        if record.is_none() && row + 1 >= plan.need.end {
            break;
        }
    }
    Ok(false)
}

/// Decode a coefficient-path image into `planes` (one per frame component, geometry `g`).
pub(crate) fn decode(
    p: &Parsed,
    data: &[u8],
    g: &OutGeom,
    planes: &mut [&mut [u8]],
    cp: &CoefParams,
) -> Result<CoefOutcome> {
    let f = &p.frame;
    if f.progressive {
        return crate::prog::decode(p, data, g, planes, cp);
    }
    let dc_only = g.n == 1 && f.precision == 8 && !cp.want_index;
    let full_rows = cp.index.is_none() && !dc_only;
    let full_cols = !dc_only;
    let plans = plan_scans(p, g, dc_only, full_rows || cp.want_index);
    tracing::debug!(
        target: "jpeg",
        scans = plans.len(),
        dc_only,
        with_index = cp.index.is_some(),
        want_index = cp.want_index,
        "multi-scan sequential: coefficient buffers"
    );
    if let Some(ix) = cp.index {
        if ix.len() != p.scans.len() {
            return Err(JpegError::IndexMismatch);
        }
        for pl in &plans {
            if ix[pl.si].len() != pl.rows as usize {
                return Err(JpegError::IndexMismatch);
            }
        }
    }
    // Stored windows.
    let win: Vec<Window> = f
        .comps
        .iter()
        .map(|c| {
            let bx = if full_cols || cp.want_index {
                0..c.bw_pad
            } else {
                g.mx0 * c.h..g.mx1 * c.h
            };
            let by0 = if full_rows || cp.want_index {
                0
            } else {
                g.my0 * c.v
            };
            Window {
                bx,
                by: by0..g.my1 * c.v,
            }
        })
        .collect();
    let win = if cp.want_index {
        // The index build needs every row's coefficients for refinement history.
        f.comps
            .iter()
            .map(|c| Window {
                bx: 0..c.bw_pad,
                by: 0..c.bh_pad,
            })
            .collect::<Vec<_>>()
    } else {
        win
    };
    let mut dc: Vec<Vec<i16>> = Vec::new();
    let mut ac: Vec<Vec<i16>> = Vec::new();
    for w in &win {
        dc.push(alloc_i16(w.len())?);
        ac.push(if dc_only {
            Vec::new()
        } else {
            alloc_i16(w.len() * 64)?
        });
    }

    // Chains: progressive => DC chain + one AC chain per component; sequential => one chain.
    let cx = Ctx {
        data,
        win: &win,
        cancel: cp.cancel,
        dc_only,
    };
    let nscans = p.scans.len();
    let mut index: Vec<Vec<Checkpoint>> = vec![Vec::new(); nscans];
    let start_cp = |pl: &ScanPlan| -> Checkpoint {
        match cp.index {
            Some(ix) if !cp.want_index => {
                ix[pl.si][pl.need.start.min(pl.rows.saturating_sub(1)) as usize]
            }
            _ => Checkpoint::at(pl.scan.data_start, pl.scan.restart_interval),
        }
    };
    let run_chain = |plans: &[&ScanPlan],
                     tg: &mut Targets,
                     idx_out: &mut Vec<(usize, Vec<Checkpoint>)>|
     -> Result<bool> {
        let mut trunc = false;
        for pl in plans {
            if pl.need.is_empty() && !cp.want_index {
                continue;
            }
            let mut rec = if cp.want_index {
                Some(Vec::with_capacity(pl.rows as usize))
            } else {
                None
            };
            let first = if cp.index.is_some() && !cp.want_index {
                pl.need.start
            } else {
                0
            };
            trunc |= decode_scan(&cx, pl, tg, start_cp(pl), first, rec.as_mut())?;
            if let Some(r) = rec {
                idx_out.push((pl.si, r));
            }
        }
        Ok(trunc)
    };

    let mut idx_parts: Vec<(usize, Vec<Checkpoint>)> = Vec::new();
    let mut tg = Targets {
        dc: Default::default(),
        ac: Default::default(),
    };
    for (i, (d, a)) in dc.iter_mut().zip(ac.iter_mut()).enumerate() {
        tg.dc[i] = Some(d.as_mut_slice());
        if !dc_only {
            tg.ac[i] = Some(a.as_mut_slice());
        }
    }
    let all: Vec<&ScanPlan> = plans.iter().collect();
    let truncated = run_chain(&all, &mut tg, &mut idx_parts)?;
    for (si, v) in idx_parts {
        index[si] = v;
    }

    if !cp.no_output {
        let coefs = Coefs {
            win: &win,
            dc: &dc,
            ac: &ac,
            dc_only,
        };
        output(p, g, &coefs, planes, cp.simd, cp.threads)?;
    }
    let index = if cp.want_index && !truncated {
        Some(index)
    } else {
        None
    };
    Ok(CoefOutcome { truncated, index })
}

/// Decoded coefficients of a multi-scan image: per component its window, DC values and (unless
/// `dc_only`) 64 AC coefficients per block.
pub(crate) struct Coefs<'a> {
    pub win: &'a [Window],
    pub dc: &'a [Vec<i16>],
    pub ac: &'a [Vec<i16>],
    pub dc_only: bool,
}

/// Final IDCT of the region from the coefficient arrays.
pub(crate) fn output(
    p: &Parsed,
    g: &OutGeom,
    coefs: &Coefs,
    planes: &mut [&mut [u8]],
    simd: Simd,
    threads: usize,
) -> Result<()> {
    let Coefs {
        win,
        dc,
        ac,
        dc_only,
    } = *coefs;
    let f = &p.frame;
    let twelve = f.precision == 12;
    for (ci, plane) in planes.iter_mut().enumerate() {
        let c = &f.comps[ci];
        let q = &p.quant[ci];
        let w = &win[ci];
        let stride = g.comps[ci].stride;
        let n = g.comps[ci].n;
        let bx0 = g.mx0 * c.h;
        let nbx = (g.mx1 - g.mx0) * c.h;
        let by0 = g.my0 * c.v;
        let row_bytes = stride * n;
        let wdc = &dc[ci];
        let wac = &ac[ci];
        let job = |(r, dst): (usize, &mut [u8])| {
            let by = by0 + r as u32;
            let mut blk = [0i16; 64];
            for i in 0..nbx {
                let bx = bx0 + i;
                let off = i as usize * n;
                let Some(idx) = w.idx(bx, by) else { continue };
                if dc_only {
                    let v = if twelve {
                        dc_only_value12(wdc[idx], q[0])
                    } else {
                        dc_only_value(wdc[idx], q[0])
                    };
                    dst[off] = v;
                    continue;
                }
                let a = &wac[idx * 64..idx * 64 + 64];
                blk.copy_from_slice(a);
                blk[0] = wdc[idx];
                if twelve {
                    idct_12bit(&blk, q, n, dst, off, stride);
                } else {
                    let has_ac = a[1..].iter().any(|&x| x != 0);
                    idct_block(
                        simd,
                        n,
                        &blk,
                        q,
                        has_ac,
                        Dst {
                            buf: dst,
                            off,
                            stride,
                        },
                    );
                }
            }
        };
        if threads > 1 {
            plane.par_chunks_mut(row_bytes).enumerate().for_each(job);
        } else {
            plane.chunks_mut(row_bytes).enumerate().for_each(job);
        }
    }
    Ok(())
}
