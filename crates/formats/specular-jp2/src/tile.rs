//! Per-tile decoding: geometry (Annex B), packet parsing (B.10), tier-1 dispatch, windowed
//! inverse DWT, inverse component transform and DC level shift.

use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::dwt::{self, Coef};
use crate::geom::{Rect, ceil_shr, ceil_shr_off, high_half, low_half, try_vec};
use crate::markers::{
    CBS_BYPASS, CBS_HT, CBS_MIXED, CBS_TERMALL, Codestream, CompSiz, Progression, TileParams,
};
use crate::t1::{MAX_BITPLANES, NEG, SegRef, T1};
use crate::t2::{Body, ResGrid, for_each_packet};
use crate::tagtree::{BitReader, TagTree};
use crate::{Jp2Error, Result};

/// Caps on per-tile bookkeeping (hostile headers).
const MAX_PRECINCTS: usize = 1 << 22;
const MAX_BLOCKS: usize = 1 << 23;

struct Chunk<'a> {
    data: &'a [u8],
    passes: u8,
    layer: u16,
}

struct Seg<'a> {
    max: u8,
    passes: u8,
    /// Index of the segment's first coding pass in the code-block (HT code-blocks).
    first: u16,
    chunks: Vec<Chunk<'a>>,
}

/// HT code-block coding mode, decided at its first non-empty contribution (T.814 B.10.7).
const HT_UNDECIDED: u8 = 0;
const HT_YES: u8 = 1;
/// A Part 1 code-block in an HT mixed-mode tile-component.
const HT_CLASSIC: u8 = 2;

struct CodeBlock<'a> {
    /// Band coordinates.
    rect: Rect,
    included: bool,
    numbps: u8,
    lblock: u8,
    /// HT mode ([`HT_UNDECIDED`] until the first non-placeholder pass).
    ht: u8,
    /// Coding passes received so far (placeholders included).
    npasses: u16,
    segs: Vec<Seg<'a>>,
}

struct PrecBand<'a> {
    nx: u32,
    incl: TagTree,
    zbp: TagTree,
    blocks: Vec<CodeBlock<'a>>,
}

struct Precinct<'a> {
    bands: Vec<PrecBand<'a>>,
}

struct BandGeom {
    /// 0 LL, 1 HL, 2 LH, 3 HH.
    orient: u8,
    rect: Rect,
    /// Mb: magnitude bit-planes (guard bits + exponent - 1).
    mb: u8,
    /// Quantisation step (irreversible only).
    step: f32,
    /// Needed coefficients (band coordinates).
    window: Rect,
}

struct ResGeom {
    rect: Rect,
    ppx: u8,
    ppy: u8,
    px0: u32,
    py0: u32,
    grid: ResGrid,
    cbw: u8,
    cbh: u8,
    bands: Vec<BandGeom>,
    /// Synthesis window E_r (resolution coordinates), empty when not needed.
    window: Rect,
    /// Needed output of this level T_r (inside `window`).
    target: Rect,
}

struct CompGeom {
    siz: CompSiz,
    style: u8,
    reversible: bool,
    /// Decoded on the 64-bit path: reversible HT code-blocks with more than
    /// [`MAX_BITPLANES`] bit-planes.
    wide: bool,
    roishift: u8,
    /// Highest resolution decoded (levels - reduce).
    rtop: usize,
    res: Vec<ResGeom>,
}

/// What to decode from a tile.
pub(crate) struct TileRequest<'c> {
    pub reduce: u8,
    /// Output window on the reduced reference grid (absolute coordinates).
    pub window: Rect,
    pub max_layers: u16,
    pub cancel: Option<&'c AtomicBool>,
    /// Keep components deeper than 16 bits at full precision (GRIB's packed integers).
    pub full: bool,
}

/// One decoded component of a tile: samples at the decoded resolution, DC-shifted and clamped
/// to `[0, 2^prec)` (precisions above 16 are reduced to 16).
pub(crate) struct CompOut {
    /// Window in component coordinates at the decoded resolution.
    pub rect: Rect,
    pub data: Samples,
}

/// Component samples. Up to 16 bits the DC shift and clamp are deferred to [`Samples::native`]
/// (fused into output assembly); deeper components are converted up front.
pub(crate) enum Samples {
    U16(Vec<u16>),
    I32 {
        v: Vec<i32>,
        dc: i32,
        maxv: i32,
    },
    F32 {
        v: Vec<f32>,
        dc: f32,
        maxv: f32,
    },
    /// Components decoded on the 64-bit path (more than 30 bit-planes, e.g. 32-bit ones):
    /// `shift` reduces them to 16 bits for [`Samples::native`].
    I64 {
        v: Vec<i64>,
        dc: i64,
        maxv: i64,
        shift: u32,
    },
}

impl Samples {
    pub fn is_empty(&self) -> bool {
        match self {
            Samples::U16(v) => v.is_empty(),
            Samples::I32 { v, .. } => v.is_empty(),
            Samples::F32 { v, .. } => v.is_empty(),
            Samples::I64 { v, .. } => v.is_empty(),
        }
    }

    /// Native values of samples `start..start + out.len()`.
    #[inline]
    pub fn native(&self, start: usize, out: &mut [u16]) {
        let n = out.len();
        match self {
            Samples::U16(v) => out.copy_from_slice(&v[start..start + n]),
            Samples::I32 { v, dc, maxv } => {
                for (o, &x) in out.iter_mut().zip(&v[start..start + n]) {
                    *o = x.saturating_add(*dc).clamp(0, *maxv) as u16;
                }
            }
            Samples::F32 { v, dc, maxv } => {
                for (o, &x) in out.iter_mut().zip(&v[start..start + n]) {
                    // `max` maps NaN to 0. Adding 2^23 to a value in [0, 65535] leaves the
                    // value rounded to the nearest integer in the low mantissa bits.
                    let v = (x + dc).max(0.0).min(*maxv) + 8_388_608.0;
                    *o = v.to_bits() as u16;
                }
            }
            Samples::I64 { v, dc, maxv, shift } => {
                for (o, &x) in out.iter_mut().zip(&v[start..start + n]) {
                    *o = (x.saturating_add(*dc).clamp(0, *maxv) >> shift) as u16;
                }
            }
        }
    }

    /// Native values of samples `start..start + out.len()` at full precision, for components
    /// of up to 32 bits (deeper ones, and irreversible ones over 16 bits, come reduced).
    pub fn native_u32(&self, start: usize, out: &mut [u32]) {
        let n = out.len();
        match self {
            Samples::U16(v) => {
                for (o, &x) in out.iter_mut().zip(&v[start..start + n]) {
                    *o = u32::from(x);
                }
            }
            Samples::I32 { v, dc, maxv } => {
                for (o, &x) in out.iter_mut().zip(&v[start..start + n]) {
                    *o = x.saturating_add(*dc).clamp(0, *maxv) as u32;
                }
            }
            Samples::F32 { .. } => {
                let mut t = vec![0i32; n];
                self.native_i32(start, &mut t);
                for (o, x) in out.iter_mut().zip(t) {
                    *o = x as u32;
                }
            }
            Samples::I64 { v, dc, maxv, .. } => {
                let maxv = (*maxv).min(u32::MAX as i64);
                for (o, &x) in out.iter_mut().zip(&v[start..start + n]) {
                    *o = x.saturating_add(*dc).clamp(0, maxv) as u32;
                }
            }
        }
    }

    /// Native values of samples `start..start + out.len()` at full precision.
    pub fn native_i32(&self, start: usize, out: &mut [i32]) {
        let n = out.len();
        match self {
            Samples::U16(v) => {
                for (o, &x) in out.iter_mut().zip(&v[start..start + n]) {
                    *o = i32::from(x);
                }
            }
            Samples::I32 { v, dc, maxv } => {
                for (o, &x) in out.iter_mut().zip(&v[start..start + n]) {
                    *o = x.saturating_add(*dc).clamp(0, *maxv);
                }
            }
            Samples::F32 { v, dc, maxv } => {
                for (o, &x) in out.iter_mut().zip(&v[start..start + n]) {
                    let y = (x + dc).round();
                    *o = if y.is_nan() {
                        0
                    } else {
                        y.clamp(0.0, *maxv) as i32
                    };
                }
            }
            Samples::I64 { v, dc, maxv, .. } => {
                let maxv = (*maxv).min(i32::MAX as i64);
                for (o, &x) in out.iter_mut().zip(&v[start..start + n]) {
                    *o = x.saturating_add(*dc).clamp(0, maxv) as i32;
                }
            }
        }
    }

    /// Native value of sample `i`.
    #[inline]
    pub fn get(&self, i: usize) -> u16 {
        let mut o = [0u16];
        self.native(i, &mut o);
        o[0]
    }
}

pub(crate) struct TileOut {
    pub comps: Vec<CompOut>,
    pub truncated: bool,
}

pub(crate) fn cancelled(c: Option<&AtomicBool>) -> bool {
    c.is_some_and(|c| c.load(Ordering::Relaxed))
}

struct TileState<'a> {
    params: TileParams,
    comps: Vec<CompGeom>,
    precincts: Vec<Option<Box<Precinct<'a>>>>,
    blocks_created: usize,
}

fn band_rect(tc: &Rect, nb: u8, orient: u8) -> Rect {
    if nb == 0 {
        return *tc;
    }
    let xob = (orient & 1) as u64;
    let yob = (orient >> 1) as u64;
    let s = nb as u32;
    let offx = xob << (s - 1);
    let offy = yob << (s - 1);
    Rect::new(
        ceil_shr_off(tc.x0, offx, s),
        ceil_shr_off(tc.y0, offy, s),
        ceil_shr_off(tc.x1, offx, s),
        ceil_shr_off(tc.y1, offy, s),
    )
}

/// Passes per codeword segment of a Part 1 code-block.
fn seg_max(style: u8, idx: usize) -> u8 {
    if style & CBS_TERMALL != 0 {
        1
    } else if style & CBS_BYPASS != 0 {
        if idx == 0 {
            10
        } else if idx % 2 == 1 {
            2
        } else {
            1
        }
    } else {
        109
    }
}

/// Read the codeword segment lengths of `n` new coding passes of an HT (or not yet decided,
/// in mixed mode) code-block (T.814 B.10.7): placeholder passes before the first HT
/// cleanup pass, the first cleanup segment (which absorbs them), then alternating SigProp +
/// MagRef and cleanup segments. In mixed mode the first non-empty contribution may instead
/// reveal a Part 1 code-block. Returns false on corrupt or unsupported headers.
fn ht_lengths(
    br: &mut BitReader,
    cb: &mut CodeBlock,
    n: u32,
    style: u8,
    key: usize,
    pending: &mut Vec<(usize, usize, u32, u8)>,
) -> bool {
    let mixed = style & CBS_MIXED != 0;
    let lb = cb.lblock as u32;
    let log2 = |v: u32| 31 - v.leading_zeros();
    let full = lb + log2(n);
    if full > 32 {
        return false;
    }
    let mut num = cb.npasses as u32;
    let mut rem = n;
    if num + n > 255 {
        return false;
    }
    if cb.ht == HT_UNDECIDED {
        let href = (num + n - 1) % 3;
        // Length bits as for a Part 1 code-block with all `n` passes in one segment.
        let extend = |br: &mut BitReader, mut bits: u32, mut len: u64| {
            while bits < full {
                len = len << 1 | br.bit() as u64;
                bits += 1;
            }
            len
        };
        let first_seg = n.checked_sub(href).filter(|&s| s >= 1);
        let (bits, len) = match first_seg {
            Some(seg) => {
                let bits = lb + log2(seg);
                (bits, br.bits(bits) as u64)
            }
            None => (0, 0),
        };
        let is_ht = match first_seg {
            Some(_) if len != 0 => {
                if mixed {
                    lb > 3 && len > 1 && len >> (bits - 1) == 0
                } else if len < 2 {
                    return false;
                } else {
                    true
                }
            }
            _ => false,
        };
        if !is_ht {
            let len = extend(br, bits, len);
            if len == 0 {
                // Placeholder passes only.
                cb.npasses = (num + n) as u16;
                return true;
            }
            // A Part 1 code-block (mixed mode only), all passes in one segment; earlier
            // placeholder passes become its leading (empty) passes.
            if !mixed || style & (CBS_BYPASS | CBS_TERMALL) != 0 || len > u32::MAX as u64 {
                return false;
            }
            let total = num + n;
            cb.ht = HT_CLASSIC;
            cb.npasses = total as u16;
            cb.segs.push(Seg {
                max: seg_max(style & !(CBS_HT | CBS_MIXED), 0),
                passes: total as u8,
                first: 0,
                chunks: Vec::new(),
            });
            pending.push((key, cb.segs.len() - 1, len as u32, total as u8));
            return true;
        }
        let seg = first_seg.unwrap_or(1);
        cb.ht = HT_YES;
        cb.segs.push(Seg {
            max: (num + seg) as u8,
            passes: (num + seg) as u8,
            first: 0,
            chunks: Vec::new(),
        });
        pending.push((key, cb.segs.len() - 1, len as u32, seg as u8));
        num += seg;
        rem -= seg;
    }
    while rem > 0 {
        let (take, bits, cont) = match num % 3 {
            0 => (1, lb, false),
            1 => {
                let t = rem.min(2);
                (t, lb + t - 1, false)
            }
            _ => (1, lb, true),
        };
        if bits > 32 || cb.segs.len() >= 1024 {
            return false;
        }
        let len = br.bits(bits);
        let joins = cont
            && cb
                .segs
                .last()
                .is_some_and(|s| s.first as u32 + s.passes as u32 == num && s.passes < s.max);
        if joins {
            if let Some(s) = cb.segs.last_mut() {
                s.passes += 1;
            }
        } else {
            cb.segs.push(Seg {
                max: if num.is_multiple_of(3) { 1 } else { 2 },
                passes: take as u8,
                first: num as u16,
                chunks: Vec::new(),
            });
        }
        pending.push((key, cb.segs.len() - 1, len, take as u8));
        num += take;
        rem -= take;
    }
    cb.npasses = num as u16;
    !br.overrun
}

impl<'a> TileState<'a> {
    fn new(cs: &Codestream<'a>, t: usize, params: TileParams, req: &TileRequest) -> Result<Self> {
        let trect = cs.siz.tile_rect(t);
        let nc = cs.siz.comps.len();
        let mut comps = Vec::with_capacity(nc);
        let mut nprec = 0usize;
        let mct = params.cod.mct;
        let _ = mct;
        for c in 0..nc {
            let siz = cs.siz.comps[c];
            let cc = &params.comps[c];
            let q = &params.quant[c];
            let (dx, dy) = (siz.dx as u32, siz.dy as u32);
            let tc = Rect::new(
                trect.x0.div_ceil(dx),
                trect.y0.div_ceil(dy),
                trect.x1.div_ceil(dx),
                trect.y1.div_ceil(dy),
            );
            let nl = cc.levels;
            if req.reduce > nl {
                return Err(Jp2Error::InvalidRequest(
                    "reduce exceeds the decomposition levels of a tile",
                ));
            }
            let rtop = (nl - req.reduce) as usize;
            let mut res = Vec::with_capacity(nl as usize + 1);
            for r in 0..=nl as usize {
                let sh = nl as u32 - r as u32;
                let rr = Rect::new(
                    ceil_shr(tc.x0, sh),
                    ceil_shr(tc.y0, sh),
                    ceil_shr(tc.x1, sh),
                    ceil_shr(tc.y1, sh),
                );
                let (ppx, ppy) = cc.precincts.get(r).copied().unwrap_or((15, 15));
                let (ppx, ppy) = (ppx.min(31), ppy.min(31));
                let px0 = rr.x0 >> ppx;
                let py0 = rr.y0 >> ppy;
                let (pw, ph) = if rr.is_empty() {
                    (0, 0)
                } else {
                    (
                        ceil_shr(rr.x1, ppx as u32) - px0,
                        ceil_shr(rr.y1, ppy as u32) - py0,
                    )
                };
                let count = pw as usize * ph as usize;
                if nprec + count > MAX_PRECINCTS {
                    return Err(Jp2Error::TooLarge((nprec + count) as u64));
                }
                // Position keys on the reference grid (B.12.1.3).
                let scx = (dx as u64) << sh;
                let scy = (dy as u64) << sh;
                let x_first = if !rr.x0.is_multiple_of(1u32 << ppx) {
                    trect.x0 as u64
                } else {
                    scx * ((px0 as u64) << ppx)
                };
                let y_first = if !rr.y0.is_multiple_of(1u32 << ppy) {
                    trect.y0 as u64
                } else {
                    scy * ((py0 as u64) << ppy)
                };
                let grid = ResGrid {
                    pw,
                    ph,
                    base: nprec,
                    x0_key: scx * ((px0 as u64) << ppx),
                    x_step: scx << ppx,
                    x_first,
                    y0_key: scy * ((py0 as u64) << ppy),
                    y_step: scy << ppy,
                    y_first,
                };
                nprec += count;
                let (cbw, cbh) = if r == 0 {
                    (cc.cbw.min(ppx), cc.cbh.min(ppy))
                } else {
                    (
                        cc.cbw.min(ppx.saturating_sub(1)),
                        cc.cbh.min(ppy.saturating_sub(1)),
                    )
                };
                let mut bands = Vec::new();
                let orients: &[u8] = if r == 0 { &[0] } else { &[1, 2, 3] };
                for &o in orients {
                    let nb = if r == 0 { nl } else { nl - r as u8 + 1 };
                    let rect = band_rect(&tc, nb, o);
                    let bidx = if r == 0 {
                        0
                    } else {
                        1 + 3 * (r - 1) + (o as usize - 1)
                    };
                    let (e, m) = q.step(bidx, nb, nl);
                    let mb = (q.guard as i32 + e as i32 - 1).clamp(0, 255) as u8;
                    let gain = match o {
                        0 => 0,
                        3 => 2,
                        _ => 1,
                    };
                    let rb = siz.prec as i32 + gain;
                    let step = if cc.reversible || q.style == 0 {
                        1.0
                    } else {
                        (2f64.powi(rb - e as i32) * (1.0 + m as f64 / 2048.0)) as f32
                    };
                    bands.push(BandGeom {
                        orient: o,
                        rect,
                        mb,
                        step,
                        window: Rect::default(),
                    });
                }
                res.push(ResGeom {
                    rect: rr,
                    ppx,
                    ppy,
                    px0,
                    py0,
                    grid,
                    cbw,
                    cbh,
                    bands,
                    window: Rect::default(),
                    target: Rect::default(),
                });
            }
            // Needed windows, top-down.
            let need = {
                let w = req.window;
                let rt = &res[rtop].rect;
                if w.is_empty() || rt.is_empty() {
                    Rect::default()
                } else {
                    // Clamped like the sample lookup in `color::Mapping::fill`: a window over
                    // the edge of a subsampled component still needs its nearest sample.
                    let cx = |x: u32| (x / dx).clamp(rt.x0, rt.x1 - 1);
                    let cy = |y: u32| (y / dy).clamp(rt.y0, rt.y1 - 1);
                    Rect::new(cx(w.x0), cy(w.y0), cx(w.x1 - 1) + 1, cy(w.y1 - 1) + 1)
                }
            };
            let margin = if cc.reversible {
                <i32 as Coef>::MARGIN + 1
            } else {
                <f32 as Coef>::MARGIN + 1
            };
            let mut t_r = need;
            for r in (0..=rtop).rev() {
                let rg = &mut res[r];
                rg.target = t_r;
                if t_r.is_empty() {
                    continue;
                }
                if r == 0 {
                    rg.window = t_r;
                    rg.bands[0].window = t_r;
                    break;
                }
                let e = t_r.grow_within(margin, &rg.rect);
                rg.window = e;
                let (lx0, lx1) = low_half(e.x0, e.x1);
                let (ly0, ly1) = low_half(e.y0, e.y1);
                let (hx0, hx1) = high_half(e.x0, e.x1);
                let (hy0, hy1) = high_half(e.y0, e.y1);
                for b in &mut rg.bands {
                    b.window = match b.orient {
                        1 => Rect::new(hx0, ly0, hx1, ly1),
                        2 => Rect::new(lx0, hy0, lx1, hy1),
                        _ => Rect::new(hx0, hy0, hx1, hy1),
                    }
                    .intersect(&b.rect);
                }
                t_r = Rect::new(lx0, ly0, lx1, ly1);
            }
            let wide = cc.reversible
                && cc.style & CBS_HT != 0
                && res
                    .iter()
                    .flat_map(|r| &r.bands)
                    .any(|b| b.mb > MAX_BITPLANES);
            comps.push(CompGeom {
                siz,
                style: cc.style,
                reversible: cc.reversible,
                wide,
                roishift: params.roishift[c],
                rtop,
                res,
            });
        }
        // The colour transform needs its three components on one path.
        if mct == 1
            && comps.len() >= 3
            && comps[..3].iter().any(|c| c.wide)
            && comps[..3].iter().all(|c| c.reversible)
        {
            comps[..3].iter_mut().for_each(|c| c.wide = true);
        }
        let mut precincts = Vec::new();
        precincts
            .try_reserve_exact(nprec)
            .map_err(|_| Jp2Error::TooLarge(nprec as u64 * 8))?;
        precincts.resize_with(nprec, || None);
        Ok(TileState {
            params,
            comps,
            precincts,
            blocks_created: 0,
        })
    }

    fn precinct_res_rect(rg: &ResGeom, p: usize) -> Rect {
        let pw = rg.grid.pw.max(1) as usize;
        let i = (p % pw) as u64;
        let j = (p / pw) as u64;
        let x0 = (rg.px0 as u64 + i) << rg.ppx;
        let y0 = (rg.py0 as u64 + j) << rg.ppy;
        let x1 = x0 + (1u64 << rg.ppx);
        let y1 = y0 + (1u64 << rg.ppy);
        let cl = |v: u64| v.min(u32::MAX as u64) as u32;
        Rect::new(cl(x0), cl(y0), cl(x1), cl(y1)).intersect(&rg.rect)
    }

    /// Is packet (c, r, p, l) needed for the requested output?
    fn needed(&self, c: usize, r: usize, p: usize, l: u16, max_layers: u16) -> bool {
        let cg = &self.comps[c];
        if r > cg.rtop || l >= max_layers {
            return false;
        }
        let rg = &cg.res[r];
        !rg.window.is_empty() && Self::precinct_res_rect(rg, p).intersects(&rg.window)
    }

    fn make_precinct(&mut self, c: usize, r: usize, p: usize) -> Result<Box<Precinct<'a>>> {
        let rg = &self.comps[c].res[r];
        let pw = rg.grid.pw.max(1) as usize;
        let i = (p % pw) as u64;
        let j = (p / pw) as u64;
        let mut bands = Vec::with_capacity(rg.bands.len());
        for b in &rg.bands {
            let (sx, sy) = if r == 0 {
                (rg.ppx as u32, rg.ppy as u32)
            } else {
                (
                    rg.ppx.saturating_sub(1) as u32,
                    rg.ppy.saturating_sub(1) as u32,
                )
            };
            let cl = |v: u64| v.min(u32::MAX as u64) as u32;
            let bx0 = (rg.px0 as u64 + i) << sx;
            let by0 = (rg.py0 as u64 + j) << sy;
            let pr = Rect::new(
                cl(bx0),
                cl(by0),
                cl(bx0 + (1u64 << sx)),
                cl(by0 + (1u64 << sy)),
            )
            .intersect(&b.rect);
            let (cbw, cbh) = (rg.cbw as u32, rg.cbh as u32);
            let (nx, ny, cx0, cy0) = if pr.is_empty() {
                (0, 0, 0, 0)
            } else {
                let cx0 = pr.x0 >> cbw;
                let cy0 = pr.y0 >> cbh;
                (
                    ceil_shr(pr.x1, cbw) - cx0,
                    ceil_shr(pr.y1, cbh) - cy0,
                    cx0,
                    cy0,
                )
            };
            let n = nx as usize * ny as usize;
            self.blocks_created += n;
            if self.blocks_created > MAX_BLOCKS {
                return Err(Jp2Error::TooLarge(self.blocks_created as u64 * 48));
            }
            let mut blocks = Vec::with_capacity(n);
            for y in 0..ny {
                for x in 0..nx {
                    let bx = (cx0 + x) << cbw;
                    let by = (cy0 + y) << cbh;
                    blocks.push(CodeBlock {
                        // The last block may end past u32::MAX (an image at the end of the
                        // reference grid): clamp, the intersection is inside `pr` anyway.
                        rect: Rect::new(
                            bx,
                            by,
                            cl(u64::from(bx) + (1u64 << cbw)),
                            cl(u64::from(by) + (1u64 << cbh)),
                        )
                        .intersect(&pr),
                        included: false,
                        numbps: 0,
                        lblock: 3,
                        ht: HT_UNDECIDED,
                        npasses: 0,
                        segs: Vec::new(),
                    });
                }
            }
            bands.push(PrecBand {
                nx,
                incl: TagTree::new(nx, ny),
                zbp: TagTree::new(nx, ny),
                blocks,
            });
        }
        Ok(Box::new(Precinct { bands }))
    }

    /// Parse one packet. `Ok(false)`: the data ended or is corrupt (stop parsing this tile).
    #[allow(clippy::too_many_arguments)]
    fn packet(
        &mut self,
        c: usize,
        r: usize,
        p: usize,
        layer: u16,
        hdr: &mut Option<(Vec<u8>, usize)>,
        body: &mut Body<'a>,
        pending: &mut Vec<(usize, usize, u32, u8)>,
    ) -> Result<bool> {
        let (sop, eph) = (self.params.cod.sop, self.params.cod.eph);
        let k = self.comps[c].res[r].grid.base + p;
        if self.precincts[k].is_none() {
            let pr = self.make_precinct(c, r, p)?;
            self.precincts[k] = Some(pr);
        }
        if sop {
            body.skip_sop();
        }
        let in_body = hdr.is_none();
        let (hslice, hpos): (&[u8], usize) = match hdr {
            Some((v, pos)) => (v.as_slice(), *pos),
            None => match body.current() {
                Some(s) => (s, body.pos),
                None => return Ok(false),
            },
        };
        if hpos >= hslice.len() {
            return Ok(false);
        }
        let style = self.comps[c].style;
        // Missing MSBs count down from Mb + s with an ROI shift s (T.800 H.1): a block may
        // have fewer planes than the shift alone.
        let roi = self.comps[c].roishift as u32;
        let mbs: Vec<u32> = self.comps[c].res[r]
            .bands
            .iter()
            .map(|b| b.mb as u32 + roi)
            .collect();
        let prec = self.precincts[k].as_mut().expect("created above");
        let mut br = BitReader::new(hslice, hpos);
        pending.clear();
        if br.bit() != 0 {
            for (bi, pb) in prec.bands.iter_mut().enumerate() {
                for (bk, cb) in pb.blocks.iter_mut().enumerate() {
                    let included = if !cb.included {
                        let leaf_x = bk as u32 % pb.nx.max(1);
                        let leaf_y = bk as u32 / pb.nx.max(1);
                        let leaf = (leaf_y * pb.nx + leaf_x) as usize;
                        match pb.incl.decode(&mut br, leaf, layer as u32 + 1) {
                            Some(_) => {
                                let Some(z) = pb.zbp.decode_value(&mut br, leaf, 74) else {
                                    return Ok(false);
                                };
                                cb.numbps = mbs[bi].saturating_sub(z).min(255) as u8;
                                cb.included = true;
                                true
                            }
                            None => false,
                        }
                    } else {
                        br.bit() != 0
                    };
                    if br.overrun {
                        return Ok(false);
                    }
                    if !included {
                        continue;
                    }
                    // Number of coding passes (Table B.4).
                    let n = if br.bit() == 0 {
                        1
                    } else if br.bit() == 0 {
                        2
                    } else {
                        let v = br.bits(2);
                        if v != 3 {
                            3 + v
                        } else {
                            let v = br.bits(5);
                            if v != 31 { 6 + v } else { 37 + br.bits(7) }
                        }
                    };
                    while br.bit() == 1 {
                        cb.lblock = cb.lblock.saturating_add(1);
                        if br.overrun || cb.lblock > 32 {
                            return Ok(false);
                        }
                    }
                    let key = bi << 24 | bk;
                    if style & CBS_HT != 0 && cb.ht != HT_CLASSIC {
                        if !ht_lengths(&mut br, cb, n, style, key, pending) {
                            return Ok(false);
                        }
                        continue;
                    }
                    let cstyle = style & !(CBS_HT | CBS_MIXED);
                    cb.npasses = cb.npasses.saturating_add(n as u16);
                    let mut rem = n;
                    while rem > 0 {
                        let need_new = cb.segs.last().is_none_or(|s| s.passes >= s.max);
                        if need_new {
                            if cb.segs.len() >= 1024 {
                                return Ok(false);
                            }
                            cb.segs.push(Seg {
                                max: seg_max(cstyle, cb.segs.len()),
                                passes: 0,
                                first: 0,
                                chunks: Vec::new(),
                            });
                        }
                        let si = cb.segs.len() - 1;
                        let seg = &mut cb.segs[si];
                        let take = rem.min((seg.max - seg.passes) as u32);
                        seg.passes += take as u8;
                        let bits = cb.lblock as u32 + (31 - take.leading_zeros());
                        if bits > 32 {
                            return Ok(false);
                        }
                        let len = br.bits(bits);
                        pending.push((key, si, len, take as u8));
                        rem -= take;
                    }
                    if br.overrun {
                        return Ok(false);
                    }
                }
            }
        }
        br.align();
        let mut pos = br.pos;
        if br.overrun || pos > hslice.len() {
            return Ok(false);
        }
        if eph && hslice.len() >= pos + 2 && hslice[pos] == 0xFF && hslice[pos + 1] == 0x92 {
            pos += 2;
        }
        if in_body {
            body.pos = pos;
        } else if let Some((_, hp)) = hdr {
            *hp = pos;
        }
        let mut ok = true;
        for &(key, si, len, take) in pending.iter() {
            let (bi, bk) = (key >> 24, key & 0xFF_FFFF);
            let (data, short) = body.take(len as usize);
            prec.bands[bi].blocks[bk].segs[si].chunks.push(Chunk {
                data,
                passes: take,
                layer,
            });
            if short {
                ok = false;
            }
        }
        Ok(ok)
    }
}

/// Values produced by tier-1 for a band coefficient.
trait Sample: Coef {
    /// HT code-blocks of this sample type are decoded into 64-bit words ([`T1::row64`]).
    const WIDE: bool = false;
    fn from_mag(mag2: u32, neg: bool, step: f32) -> Self;
    /// [`Sample::from_mag`] of a 64-bit word (only `WIDE` types get them).
    fn from_mag64(mag2: u64, neg: bool, step: f32) -> Self;
}

impl Sample for i32 {
    #[inline(always)]
    fn from_mag(mag2: u32, neg: bool, _step: f32) -> i32 {
        let v = (mag2 >> 1) as i32;
        if neg { v.wrapping_neg() } else { v }
    }
    fn from_mag64(mag2: u64, neg: bool, step: f32) -> i32 {
        Self::from_mag(mag2 as u32, neg, step)
    }
}

impl Sample for f32 {
    #[inline(always)]
    fn from_mag(mag2: u32, neg: bool, step: f32) -> f32 {
        let v = mag2 as f32 * (step * 0.5);
        if neg { -v } else { v }
    }
    fn from_mag64(mag2: u64, neg: bool, step: f32) -> f32 {
        let v = mag2 as f32 * (step * 0.5);
        if neg { -v } else { v }
    }
}

impl Sample for i64 {
    const WIDE: bool = true;
    #[inline(always)]
    fn from_mag(mag2: u32, neg: bool, _step: f32) -> i64 {
        let v = (mag2 >> 1) as i64;
        if neg { -v } else { v }
    }
    #[inline(always)]
    fn from_mag64(mag2: u64, neg: bool, _step: f32) -> i64 {
        let v = (mag2 >> 1) as i64;
        if neg { -v } else { v }
    }
}

enum Plane {
    Int(Vec<i32>),
    Float(Vec<f32>),
    Wide(Vec<i64>),
}

struct Job<'s, 'a> {
    blk: &'s CodeBlock<'a>,
    orient: u8,
    mb_step: f32,
    win: Rect,
    offx: u32,
    offy: u32,
}

impl<'a> TileState<'a> {
    /// Decode all needed code-blocks of resolution `r` of component `c` into `a` (width `w`),
    /// a quadrant-layout buffer with low halves of size `nlx x nly`.
    #[allow(clippy::too_many_arguments)]
    fn decode_blocks<T: Sample>(
        &self,
        c: usize,
        r: usize,
        a: &mut [T],
        w: usize,
        nlx: u32,
        nly: u32,
        max_layers: u16,
    ) {
        let cg = &self.comps[c];
        let rg = &cg.res[r];
        let mut jobs: Vec<Job> = Vec::new();
        for p in 0..rg.grid.count() {
            let Some(prec) = &self.precincts[rg.grid.base + p] else {
                continue;
            };
            for (bi, pb) in prec.bands.iter().enumerate() {
                let bg = &rg.bands[bi];
                if bg.window.is_empty() {
                    continue;
                }
                let (offx, offy) = (
                    if bg.orient & 1 != 0 { nlx } else { 0 },
                    if bg.orient & 2 != 0 { nly } else { 0 },
                );
                for blk in &pb.blocks {
                    if blk.included && !blk.segs.is_empty() && blk.rect.intersects(&bg.window) {
                        jobs.push(Job {
                            blk,
                            orient: bg.orient,
                            mb_step: bg.step,
                            win: bg.window,
                            offx,
                            offy,
                        });
                    }
                }
            }
        }
        if jobs.is_empty() {
            return;
        }
        let style = cg.style;
        let roishift = cg.roishift as u32;
        // Destination rows of each job; merge overlapping row ranges into groups that own a
        // disjoint slice of `a`, each behind a lock (the writes are short next to tier-1).
        let dst_rows = |j: &Job| {
            let inter = j.blk.rect.intersect(&j.win);
            let y0 = (inter.y0 - j.win.y0 + j.offy) as usize;
            (y0, y0 + inter.h() as usize)
        };
        let mut order: Vec<usize> = (0..jobs.len()).collect();
        order.sort_by_key(|&i| dst_rows(&jobs[i]).0);
        let mut groups: Vec<(usize, usize)> = Vec::new();
        let mut group_of = vec![0usize; jobs.len()];
        for &i in &order {
            let (y0, y1) = dst_rows(&jobs[i]);
            match groups.last_mut() {
                Some(g) if y0 < g.1 => g.1 = g.1.max(y1),
                _ => groups.push((y0, y1)),
            }
            group_of[i] = groups.len() - 1;
        }
        let mut slices = Vec::with_capacity(groups.len());
        let mut rest: &mut [T] = &mut a[..];
        let mut at = 0usize;
        for &(y0, y1) in &groups {
            let tail = std::mem::take(&mut rest);
            let (_, tail) = tail.split_at_mut(((y0 - at) * w).min(tail.len()));
            let (g, tail) = tail.split_at_mut(((y1 - y0) * w).min(tail.len()));
            slices.push(std::sync::Mutex::new(g));
            rest = tail;
            at = y1;
        }
        jobs.par_iter()
            .zip(group_of.par_iter())
            .for_each_init(T1::default, |t1, (job, &gi)| {
                let blk = job.blk;
                let (bw, bh) = (blk.rect.w() as usize, blk.rect.h() as usize);
                let mut store: Vec<&[u8]> = Vec::new();
                let mut spans: Vec<(usize, usize, u8)> = Vec::new();
                let ht = style & CBS_HT != 0 && blk.ht == HT_YES;
                let nbps = if ht && T::WIDE {
                    blk.numbps.min(crate::ht::WIDE_BITPLANES)
                } else {
                    blk.numbps.min(MAX_BITPLANES + 1)
                };
                let mut cup_plane = None;
                if ht {
                    // The latest complete, non-empty HT set within the layers: its cleanup
                    // segment and the SigProp/MagRef passes that follow it.
                    let within = |s: &Seg| s.chunks.iter().all(|c| c.layer < max_layers);
                    let cup = blk.segs.iter().enumerate().rev().find(|(_, s)| {
                        let last = s.first as u32 + s.max as u32 - 1;
                        s.passes == s.max
                            && last.is_multiple_of(3)
                            && within(s)
                            && s.chunks.iter().map(|c| c.data.len()).sum::<usize>() >= 2
                    });
                    if let Some((ci, s)) = cup {
                        let last = s.first as u32 + s.max as u32 - 1;
                        cup_plane = (nbps as u32).checked_sub(1 + last / 3);
                        store.extend(s.chunks.iter().map(|c| c.data));
                        spans.push((0, store.len(), 1));
                        if let Some(r) = blk.segs.get(ci + 1)
                            && r.first as u32 == last + 1
                        {
                            let s0 = store.len();
                            let mut passes = 0u8;
                            for ch in r.chunks.iter().take_while(|c| c.layer < max_layers) {
                                store.push(ch.data);
                                passes += ch.passes;
                            }
                            if passes > 0 {
                                spans.push((s0, store.len(), passes));
                            }
                        }
                    }
                }
                'outer: for seg in blk.segs.iter().filter(|_| !ht) {
                    let s = store.len();
                    let mut passes = 0u8;
                    let mut stop = false;
                    for ch in &seg.chunks {
                        if ch.layer >= max_layers {
                            stop = true;
                            break;
                        }
                        store.push(ch.data);
                        passes = passes.saturating_add(ch.passes);
                    }
                    if passes > 0 {
                        spans.push((s, store.len(), passes));
                    }
                    if stop {
                        break 'outer;
                    }
                }
                let segs: Vec<SegRef> = spans
                    .iter()
                    .map(|&(s, e, p)| SegRef {
                        chunks: &store[s..e],
                        passes: p,
                    })
                    .collect();
                if ht {
                    t1.decode_ht(bw, bh, style, cup_plane, &segs, T::WIDE);
                } else {
                    let cstyle = style & !(CBS_HT | CBS_MIXED);
                    t1.decode(bw, bh, job.orient, cstyle, nbps, &segs);
                }
                let inter = blk.rect.intersect(&job.win);
                let (sx, sy) = (inter.x0 - blk.rect.x0, inter.y0 - blk.rect.y0);
                let dx0 = (inter.x0 - job.win.x0 + job.offx) as usize;
                let n = inter.w() as usize;
                let gy0 = groups[gi].0;
                let (dy0, _) = dst_rows(job);
                let mut g = slices[gi].lock().unwrap_or_else(|e| e.into_inner());
                for (k, y) in (sy..sy + inter.h()).enumerate() {
                    let o = (dy0 - gy0 + k) * w + dx0;
                    let Some(dst) = g.get_mut(o..o + n) else {
                        break;
                    };
                    if t1.wide {
                        const NEG64: u64 = 1 << 63;
                        match t1.row64(y as usize, sx as usize, n) {
                            Some(mags) => {
                                for (d, &v) in dst.iter_mut().zip(mags) {
                                    let m = v & !NEG64;
                                    let m = if roishift == 0 {
                                        m
                                    } else if roishift >= 63 {
                                        0
                                    } else if m >= 1 << roishift {
                                        m >> roishift
                                    } else {
                                        m
                                    };
                                    *d = T::from_mag64(m, v & NEG64 != 0, job.mb_step);
                                }
                            }
                            None => dst.fill(T::default()),
                        }
                        continue;
                    }
                    let Some(mags) = t1.row(y as usize, sx as usize, n) else {
                        dst.fill(T::default());
                        continue;
                    };
                    if roishift == 0 {
                        for (d, &m) in dst.iter_mut().zip(mags) {
                            *d = T::from_mag(m & !NEG, m & NEG != 0, job.mb_step);
                        }
                    } else {
                        for (d, &v) in dst.iter_mut().zip(mags) {
                            let m = v & !NEG;
                            let m = if roishift >= 31 {
                                0
                            } else if m >= 1 << roishift {
                                m >> roishift
                            } else {
                                m
                            };
                            *d = T::from_mag(m, v & NEG != 0, job.mb_step);
                        }
                    }
                }
            });
    }

    fn reconstruct<T: Sample>(&self, c: usize, req: &TileRequest) -> Result<Vec<T>> {
        let cg = &self.comps[c];
        let w0 = cg.res[0].window;
        // Two buffers sized for the largest level, swapped between levels: `cur` holds the
        // current level (natural layout), `a` the next level in quadrant layout.
        let maxn = (0..=cg.rtop)
            .map(|r| cg.res[r].window.area() as usize)
            .max()
            .unwrap_or(0);
        let mut cur: Vec<T> = try_vec(maxn)?;
        let mut a: Vec<T> = if cg.rtop > 0 {
            try_vec(maxn)?
        } else {
            Vec::new()
        };
        let mut cur_n = w0.area() as usize;
        if !w0.is_empty() {
            self.decode_blocks(
                c,
                0,
                &mut cur[..cur_n],
                w0.w() as usize,
                0,
                0,
                req.max_layers,
            );
        }
        for r in 1..=cg.rtop {
            if cancelled(req.cancel) {
                return Err(Jp2Error::Cancelled);
            }
            let rg = &cg.res[r];
            let e = rg.window;
            if e.is_empty() {
                // Empty at this resolution (narrow edge tile); higher ones may not be.
                cur_n = 0;
                continue;
            }
            let (w, h) = (e.w() as usize, e.h() as usize);
            let (lx0, lx1) = low_half(e.x0, e.x1);
            let (ly0, ly1) = low_half(e.y0, e.y1);
            let (nlx, nly) = ((lx1 - lx0) as usize, (ly1 - ly0) as usize);
            let prev = cg.res[r - 1].target;
            debug_assert_eq!(prev, Rect::new(lx0, ly0, lx1, ly1));
            let have_low = nlx > 0 && cur_n >= nlx * nly;
            // Low quadrant from the previous level, zeros elsewhere.
            let src = &cur[..cur_n];
            a[..w * h]
                .par_chunks_mut(w)
                .enumerate()
                .for_each(|(y, row)| {
                    if y < nly && have_low {
                        row[..nlx].copy_from_slice(&src[y * nlx..(y + 1) * nlx]);
                        row[nlx..].fill(T::default());
                    } else {
                        row.fill(T::default());
                    }
                });
            self.decode_blocks(
                c,
                r,
                &mut a[..w * h],
                w,
                nlx as u32,
                nly as u32,
                req.max_layers,
            );
            dwt::idwt_level(&mut a[..w * h], w, h, e.x0, e.y0, &mut cur[..w * h]);
            let t = rg.target;
            if t == e {
                cur_n = w * h;
            } else {
                let tw = t.w() as usize;
                let (sx, sy0) = ((t.x0 - e.x0) as usize, (t.y0 - e.y0) as usize);
                let full = &cur[..w * h];
                a[..t.area() as usize]
                    .par_chunks_mut(tw.max(1))
                    .enumerate()
                    .for_each(|(y, row)| {
                        let s = (y + sy0) * w + sx;
                        row.copy_from_slice(&full[s..s + tw]);
                    });
                std::mem::swap(&mut cur, &mut a);
                cur_n = t.area() as usize;
            }
        }
        drop(a);
        cur.truncate(cur_n);
        Ok(cur)
    }
}

/// Decode tile `t`.
pub(crate) fn decode_tile(cs: &Codestream<'_>, t: usize, req: &TileRequest) -> Result<TileOut> {
    let (params, ppt) = cs.tile_params(t)?;
    let mut st = TileState::new(cs, t, params, req)?;
    let parts: Vec<&[u8]> = cs.tiles[t]
        .iter()
        .map(|&i| &cs.data[cs.parts[i].data.clone()])
        .collect();
    let mut truncated = cs.truncated;
    let mut body = Body::new(parts);
    let mut hdr: Option<(Vec<u8>, usize)> = if let Some(ppm) = &cs.ppm {
        let mut v = Vec::new();
        for &i in &cs.tiles[t] {
            if let Some(h) = ppm.get(cs.parts[i].seq) {
                v.extend_from_slice(h);
            }
        }
        Some((v, 0))
    } else {
        ppt.map(|v| (v, 0))
    };

    // Count needed packets for early termination.
    let max_layers = req.max_layers.min(st.params.cod.layers);
    let mut total_needed = 0u64;
    for (c, cg) in st.comps.iter().enumerate() {
        for r in 0..=cg.rtop {
            let rg = &cg.res[r];
            if rg.window.is_empty() {
                continue;
            }
            for p in 0..rg.grid.count() {
                if st.needed(c, r, p, 0, 1) {
                    total_needed += max_layers as u64;
                }
            }
        }
        let _ = c;
    }
    let grids: Vec<Vec<ResGrid>> = st
        .comps
        .iter()
        .map(|cg| cg.res.iter().map(|r| r.grid).collect())
        .collect();
    let mut next = vec![0u16; st.precincts.len()];
    let mut done = 0u64;
    let mut err = None;
    let mut pending = Vec::new();
    let prog: Progression = st.params.cod.prog;
    let pocs = st.params.poc.clone();
    let layers = st.params.cod.layers;
    let mut count = 0u32;
    if total_needed > 0 {
        let flow = for_each_packet(&grids, layers, prog, &pocs, &mut next, |c, r, p, l| {
            count = count.wrapping_add(1);
            if count.is_multiple_of(1024) && cancelled(req.cancel) {
                err = Some(Jp2Error::Cancelled);
                return ControlFlow::Break(());
            }
            match st.packet(c, r, p, l, &mut hdr, &mut body, &mut pending) {
                Ok(true) => {}
                Ok(false) => {
                    return ControlFlow::Break(());
                }
                Err(e) => {
                    err = Some(e);
                    return ControlFlow::Break(());
                }
            }
            if st.needed(c, r, p, l, max_layers) {
                done += 1;
                if done >= total_needed {
                    return ControlFlow::Break(());
                }
            }
            ControlFlow::Continue(())
        });
        let _ = flow;
        if let Some(e) = err {
            return Err(e);
        }
        if done < total_needed {
            truncated = true;
        }
    }

    // Reconstruct components in parallel. Every component's two level buffers (4-byte
    // samples) are charged to the memory budget first, and stay charged while the planes
    // live: up to 16 384 components of a tile-sized area each.
    let nc = st.comps.len();
    let scratch: u64 = st
        .comps
        .iter()
        .map(|cg| {
            (0..=cg.rtop)
                .map(|r| cg.res[r].window.area())
                .max()
                .unwrap_or(0)
        })
        .sum();
    let _scratch = specular_core::budget::reserve_n(scratch, 8)?;
    let planes: Vec<Result<Plane>> = (0..nc)
        .into_par_iter()
        .map(|c| {
            if st.comps[c].wide {
                st.reconstruct::<i64>(c, req).map(Plane::Wide)
            } else if st.comps[c].reversible {
                st.reconstruct::<i32>(c, req).map(Plane::Int)
            } else {
                st.reconstruct::<f32>(c, req).map(Plane::Float)
            }
        })
        .collect();
    let mut planes: Vec<Plane> = planes.into_iter().collect::<Result<_>>()?;

    // Inverse component transform.
    if st.params.cod.mct == 1 && nc >= 3 {
        let rect = |c: usize| st.comps[c].res[st.comps[c].rtop].target;
        if rect(0) == rect(1) && rect(0) == rect(2) {
            let (p0, rest) = planes.split_at_mut(1);
            let (p1, p2) = rest.split_at_mut(1);
            match (&mut p0[0], &mut p1[0], &mut p2[0]) {
                (Plane::Int(a), Plane::Int(b), Plane::Int(c)) => {
                    a.par_iter_mut()
                        .zip(b.par_iter_mut())
                        .zip(c.par_iter_mut())
                        .for_each(|((y0, y1), y2)| {
                            let g = *y0 - ((*y2 + *y1) >> 2);
                            let r = *y2 + g;
                            let b = *y1 + g;
                            *y0 = r;
                            *y1 = g;
                            *y2 = b;
                        });
                }
                (Plane::Wide(a), Plane::Wide(b), Plane::Wide(c)) => {
                    a.par_iter_mut()
                        .zip(b.par_iter_mut())
                        .zip(c.par_iter_mut())
                        .for_each(|((y0, y1), y2)| {
                            let g = *y0 - ((*y2 + *y1) >> 2);
                            let r = *y2 + g;
                            let b = *y1 + g;
                            *y0 = r;
                            *y1 = g;
                            *y2 = b;
                        });
                }
                (Plane::Float(a), Plane::Float(b), Plane::Float(c)) => {
                    a.par_iter_mut()
                        .zip(b.par_iter_mut())
                        .zip(c.par_iter_mut())
                        .for_each(|((y, cb), cr)| {
                            let r = *y + 1.402 * *cr;
                            let g = *y - 0.344_136 * *cb - 0.714_136 * *cr;
                            let b = *y + 1.772 * *cb;
                            *y = r;
                            *cb = g;
                            *cr = b;
                        });
                }
                _ => {}
            }
        }
    }

    // Binary-complement non-linearity (NLT type 3) on signed components: negative values
    // had every bit but the sign flipped by the encoder (OpenEXR's HTJ2K codes half and float
    // bit patterns this way, so that they order like the numbers).
    for (c, pl) in planes.iter_mut().enumerate() {
        let siz = &st.comps[c].siz;
        if cs.nlt.get(c) != Some(&3) || !siz.signed {
            continue;
        }
        if let Plane::Wide(v) = pl {
            let m = (1i64 << (siz.prec - 1).min(62)) - 1;
            v.par_iter_mut().for_each(|x| {
                if *x < 0 {
                    *x ^= m;
                }
            });
            continue;
        }
        if siz.prec > 31 {
            continue;
        }
        let m = ((1u64 << (siz.prec - 1)) - 1) as i32;
        match pl {
            Plane::Int(v) => v.par_iter_mut().for_each(|x| {
                if *x < 0 {
                    *x ^= m;
                }
            }),
            Plane::Float(v) => v.par_iter_mut().for_each(|x| {
                let r = x.round();
                if r < 0.0 {
                    *x = ((r as i32) ^ m) as f32;
                }
            }),
            Plane::Wide(_) => {}
        }
    }

    // Lookup-table non-linearity (NLT type 4, OpenEXR's lossy LJ2K): the reconstructed sample,
    // as a fraction of the component's range, is looked up (linear interpolation between the
    // table's points over [Dmin, Dmax]), scaled to the NLT bit depth and, for signed
    // components, turned from binary complement back to two's complement. The result is a
    // whole sample (a half or float bit pattern for OpenEXR), so it goes to the 64-bit plane.
    for (c, pl) in planes.iter_mut().enumerate() {
        let Some(lut) = cs.nlt_lut.get(c).and_then(Option::as_ref) else {
            continue;
        };
        if cs.nlt.get(c) != Some(&4) {
            continue;
        }
        let prec = st.comps[c].siz.prec as i32;
        let x: Vec<f32> = match pl {
            Plane::Float(v) => std::mem::take(v),
            Plane::Int(v) => v.iter().map(|&x| x as f32).collect(),
            Plane::Wide(v) => v.iter().map(|&x| x as f32).collect(),
        };
        let scale = (-(prec as f32)).exp2();
        *pl = Plane::Wide(
            x.par_iter()
                .map(|&x| nlt_lut_sample(lut, x * scale))
                .collect(),
        );
    }

    // DC level shift, clamp, reduce precision to 16 bits.
    let comps = planes
        .into_par_iter()
        .enumerate()
        .map(|(c, pl)| {
            let cg = &st.comps[c];
            let prec = cg.siz.prec as u32;
            let shift = prec.saturating_sub(16);
            let maxv = ((1u64 << prec) - 1) as i64;
            let dc = 1i64 << (prec - 1);
            let rect = cg.res[cg.rtop].target;
            let data = match pl {
                Plane::Wide(v) => Samples::I64 { v, dc, maxv, shift },
                Plane::Int(v) if prec <= 16 || req.full && prec <= 31 => Samples::I32 {
                    v,
                    dc: dc as i32,
                    maxv: maxv as i32,
                },
                Plane::Float(v) if prec <= 16 || req.full && prec <= 24 => Samples::F32 {
                    v,
                    dc: dc as f32,
                    maxv: maxv as f32,
                },
                // Deeper irreversible components at full precision (OpenEXR's legacy lossy
                // LJ2K floats): whole samples on the 64-bit path, as exact as f32 allows.
                Plane::Float(v) if req.full => Samples::I64 {
                    v: v.par_iter()
                        .map(|&x| if x.is_nan() { 0 } else { x.round() as i64 })
                        .collect(),
                    dc,
                    maxv,
                    shift,
                },
                Plane::Int(v) => Samples::U16(
                    v.par_iter()
                        .map(|&x| ((x as i64 + dc).clamp(0, maxv) >> shift) as u16)
                        .collect(),
                ),
                Plane::Float(v) => Samples::U16(
                    v.par_iter()
                        .map(|&x| {
                            let y = (x as f64 + dc as f64).round();
                            let y = if y.is_nan() {
                                0
                            } else {
                                (y as i64).clamp(0, maxv)
                            };
                            (y >> shift) as u16
                        })
                        .collect(),
                ),
            };
            CompOut {
                rect: if data.is_empty() {
                    Rect::default()
                } else {
                    rect
                },
                data,
            }
        })
        .collect();
    Ok(TileOut { comps, truncated })
}

/// Decomposition levels of the component with the fewest, from the main header.
pub(crate) fn min_levels(cs: &Codestream<'_>) -> u8 {
    (0..cs.siz.comps.len())
        .map(|c| cs.main_comp(c).levels)
        .min()
        .unwrap_or(0)
}

/// One sample through a type 4 non-linearity: `x` is the reconstructed sample as a fraction
/// of the component's range (signed components: -0.5..0.5). Written from OpenJPH's decoder
/// (BSD-2-Clause) for technique: f32 arithmetic, the table extended by its end points.
fn nlt_lut_sample(lut: &crate::markers::NltLut, x: f32) -> i64 {
    let n = lut.points.len();
    let unit = 1.0 / ((1u64 << 32) - 1) as f64;
    let d_min = (lut.d_min as f64 * unit) as f32;
    let d_max = (lut.d_max as f64 * unit) as f32;
    let delta = (d_max - d_min) / (n - 1) as f32;
    let inv_delta = (n - 1) as f32 / (d_max - d_min);
    let mul = (1u64 << lut.bit_depth) as f32;
    let point = |k: usize| lut.points[k.min(n - 1)] as f32 / mul;
    let t = (x + 0.5).clamp(d_min, d_max);
    let k = (((t - d_min) * inv_delta).floor() as usize).min(n - 1);
    let d_k = d_min + k as f32 * delta;
    let (t_k, t_k1) = (point(k), point(k + 1));
    let z = t_k + (t - d_k) * inv_delta * (t_k1 - t_k);
    if lut.signed {
        let half = (1u64 << (lut.bit_depth - 1)) as f32;
        let v = (z * mul - half).round() as i64;
        // Binary complement to two's complement (an involution).
        let bias = (1i64 << (lut.bit_depth - 1)) + 1;
        if v >= 0 { v } else { -v - bias }
    } else {
        (z * mul).round() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(out: &mut Vec<u8>, marker: u16, body: &[u8]) {
        out.extend_from_slice(&marker.to_be_bytes());
        out.extend_from_slice(&u16::try_from(body.len() + 2).unwrap().to_be_bytes());
        out.extend_from_slice(body);
    }

    /// 64 components of one 4096 × 4096 tile (a few hundred bytes): the image output is
    /// 64 MB of RGBA, but every component's reconstruction buffers make 8 GiB together. They
    /// are charged to the memory budget before they are made.
    #[test]
    fn component_buffers_within_memory_budget() {
        let mut cs = vec![0xFF, 0x4F];
        let mut siz = vec![0, 0];
        for v in [4096u32, 4096, 0, 0, 4096, 4096, 0, 0] {
            siz.extend_from_slice(&v.to_be_bytes());
        }
        siz.extend_from_slice(&64u16.to_be_bytes());
        siz.extend([7u8, 1, 1].repeat(64));
        seg(&mut cs, 0xFF51, &siz);
        seg(&mut cs, 0xFF52, &[0, 0, 0, 1, 0, 0, 4, 4, 0, 1]);
        seg(&mut cs, 0xFF5C, &[0x40, 8 << 3]);
        let mut sot = 0u16.to_be_bytes().to_vec();
        sot.extend_from_slice(&(12u32 + 2 + 64).to_be_bytes());
        sot.extend_from_slice(&[0, 1]);
        seg(&mut cs, 0xFF90, &sot);
        cs.extend_from_slice(&[0xFF, 0x93]);
        cs.extend_from_slice(&[0u8; 64]);
        cs.extend_from_slice(&[0xFF, 0xD9]);
        let parsed = crate::markers::parse(&cs).unwrap();
        let req = TileRequest {
            reduce: 0,
            window: Rect::new(0, 0, 4096, 4096),
            max_layers: u16::MAX,
            cancel: None,
            full: false,
        };
        let r = specular_core::budget::with_limit(1 << 30, || decode_tile(&parsed, 0, &req));
        match r {
            Err(crate::Jp2Error::NeedsMemory(n)) => assert!(n.needed >= 8 << 30, "{n:?}"),
            Err(e) => panic!("{e:?}"),
            Ok(_) => panic!("decoded"),
        }
    }

    /// The tile buffers are reserved on the decode pool's workers, from the budget of the
    /// thread that decodes: 64 components of a 1024 × 1024 tile need 512 MiB of buffers
    /// for 4 MiB of output, refused by a 64 MiB budget whatever the threads.
    #[test]
    fn pool_workers_reserve_from_the_callers_budget() {
        let mut cs = vec![0xFF, 0x4F];
        let mut siz = vec![0, 0];
        for v in [1024u32, 1024, 0, 0, 1024, 1024, 0, 0] {
            siz.extend_from_slice(&v.to_be_bytes());
        }
        siz.extend_from_slice(&64u16.to_be_bytes());
        siz.extend([7u8, 1, 1].repeat(64));
        seg(&mut cs, 0xFF51, &siz);
        seg(&mut cs, 0xFF52, &[0, 0, 0, 1, 0, 0, 4, 4, 0, 1]);
        seg(&mut cs, 0xFF5C, &[0x40, 8 << 3]);
        let mut sot = 0u16.to_be_bytes().to_vec();
        sot.extend_from_slice(&(12u32 + 2 + 64).to_be_bytes());
        sot.extend_from_slice(&[0, 1]);
        seg(&mut cs, 0xFF90, &sot);
        cs.extend_from_slice(&[0xFF, 0x93]);
        cs.extend_from_slice(&[0u8; 64]);
        cs.extend_from_slice(&[0xFF, 0xD9]);
        for parallelism in [
            crate::Parallelism::Threads(2),
            crate::Parallelism::Auto,
            crate::Parallelism::Single,
        ] {
            let opts = crate::DecodeOptions {
                parallelism,
                ..Default::default()
            };
            let r = specular_core::budget::with_limit(64 << 20, || crate::decode(&cs, &opts));
            assert!(
                matches!(r, Err(crate::Jp2Error::NeedsMemory(_))),
                "{parallelism:?}: {:?}",
                r.err()
            );
        }
    }
}
