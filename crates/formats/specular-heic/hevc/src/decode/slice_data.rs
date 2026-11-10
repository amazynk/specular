//! Slice segment data: the CTU loop, tiles, wavefront substreams and the
//! entropy-state carry-over between them (clauses 7.3.8.1 and 9.3.1).
//!
//! [`run`] decodes a run of CTUs of one slice segment. The serial driver runs
//! a whole segment in one call; the wavefront driver runs one CTU row of it
//! per call, on whichever worker owns that row. Everything that differs
//! between the two — where the row above comes from, where the wavefront
//! context state is kept, cancellation — goes through [`Env`], so the parse
//! itself, and therefore what a damaged stream decodes to, is the same.

use super::ctu::coding_tree_unit;
use super::geom::Geometry;
use super::inter::mc::McScratch;
use super::inter::{InterCtx, SliceRefs};
use super::state::Dec;
use crate::cabac::{Cabac, NUM_CTX, initial_contexts_for};
use crate::error::Error;
use crate::filter::{CtbFilter, FilterInfo};
use crate::nal::Rbsp;
use crate::picture::Picture;
use crate::ps::scaling::ScalingFactors;
use crate::ps::{Pps, Sps};
use crate::slice::SliceHeader;
use alloc::vec;
use alloc::vec::Vec;

/// The picture-wide, read-only inputs of slice data decoding.
pub struct Shared<'a> {
    /// Active SPS.
    pub sps: &'a Sps,
    /// Active PPS.
    pub pps: &'a Pps,
    /// Scan and tile tables.
    pub geo: &'a Geometry,
    /// Scaling factors in force, if any.
    pub sf: Option<&'a ScalingFactors>,
    /// The reference pictures, for a picture with inter slices.
    pub inter: Option<&'a InterCtx>,
}

/// One slice segment, ready to decode.
pub struct Segment<'a> {
    /// Its header (inherited fields filled in for a dependent segment).
    pub h: &'a SliceHeader,
    /// The RBSP the header and data were parsed from.
    pub data: &'a [u8],
    /// RBSP byte offset of each substream (entry points), the first being
    /// the start of the slice segment data.
    pub starts: &'a [usize],
    /// First CTB, in tile scan.
    pub ts0: usize,
    /// One past the last CTB the segment may cover: where the next segment
    /// starts, or the end of the picture.
    pub ts_end: usize,
}

impl Segment<'_> {
    /// The RBSP bytes of substream `sub`, bounded by the next entry point.
    fn substream(&self, sub: usize) -> Option<(&[u8], usize)> {
        let start = *self.starts.get(sub)?;
        let end = self
            .starts
            .get(sub + 1)
            .copied()
            .unwrap_or(self.data.len())
            .min(self.data.len());
        if start > end {
            return None;
        }
        Some((&self.data[..end], start))
    }
}

/// Context variables plus the Rice statistics stored with them
/// (`TableStateIdx*`, `TableMpsVal*`, `TableStatCoeff*`).
#[derive(Clone, Copy)]
pub struct CtxState {
    /// Packed context variables.
    pub ctx: [u8; NUM_CTX],
    /// `StatCoeff[]`.
    pub stat: [i32; 4],
}

/// What a dependent slice segment inherits from the end of the segment
/// before it: the entropy state (`TableStateIdxDs` and friends) and
/// `qPY_PREV`, which only resets at the start of a slice, tile or
/// wavefront row.
#[derive(Clone, Copy)]
pub struct DsState {
    /// Entropy state at the end of the segment.
    pub st: CtxState,
    /// `QpY` of the last coding unit of the segment.
    pub qp_last: i32,
}

/// Why [`Env::before_ctb`] stopped the decode.
pub enum Halt {
    /// The caller's cancellation flag was raised.
    Cancelled,
}

/// The decoder-specific services [`run`] needs.
pub trait Env {
    /// Called before CTB `rs` is decoded: wait for and import the row above,
    /// poll cancellation.
    fn before_ctb(&mut self, pic: &mut Picture, fi: &mut FilterInfo, rs: usize)
    -> Result<(), Halt>;
    /// The wavefront state stored after CTB `rs`, if it was stored.
    fn wpp_load(&mut self, rs: usize) -> Option<CtxState>;
    /// Stores the wavefront state after CTB `rs`.
    fn wpp_store(&mut self, rs: usize, st: CtxState);
    /// Called after CTB `rs` has been decoded.
    fn after_ctb(&mut self, pic: &Picture, fi: &FilterInfo, rs: usize);
}

/// How a call to [`run`] ended.
pub enum Outcome {
    /// `end_of_slice_segment_flag` was set after the CTB at tile-scan
    /// address `ts`; the state a dependent segment would continue from.
    Ended {
        /// The last CTB of the segment.
        ts: usize,
        /// The carry-over for a following dependent segment.
        ds: DsState,
    },
    /// The requested stop, a substream boundary, was reached cleanly.
    Paused,
    /// The CTB at `ts` could not be decoded (or the segment did not end
    /// where the next one starts).
    Failed(usize, Error),
    /// Cancelled before the CTB at the given address.
    Cancelled,
}

/// The column of the tile containing CTB `rs` that starts its CTB rows.
pub(super) fn tile_col_start(geo: &Geometry, rs: usize) -> usize {
    let x = rs % geo.w_ctbs;
    let mut c = x;
    while c > 0 && geo.tile_id[rs - (x - c) - 1] == geo.tile_id[rs] {
        c -= 1;
    }
    c
}

/// True when CTB `ts` (tile scan) is the first CTB of its tile.
pub(super) fn first_in_tile(geo: &Geometry, ts: usize) -> bool {
    ts == 0 || geo.tile_id[geo.ts_to_rs[ts - 1] as usize] != geo.tile_id[geo.ts_to_rs[ts] as usize]
}

/// The stretches of a picture over which a failure loses what follows it:
/// one slice within one tile. A failure loses the rest of its slice in its
/// own tile only: the next tile's substream has its own entry point and
/// reads nothing of the tile before, so it decodes whatever happened there
/// (without entry points there is nothing to restart at, and it is lost).
pub struct Runs {
    /// The first tile-scan address of the first segment.
    first: usize,
    /// Per tile-scan address from `first` on: its run.
    of: Vec<u32>,
    /// Per run: one past its last tile-scan address.
    end: Vec<usize>,
    /// Per tile-scan address `t`: substreams starting at addresses `1..=t`.
    subs: Vec<u32>,
}

impl Runs {
    /// The runs of the picture of `segs`, which are in order and contiguous.
    pub fn new(geo: &Geometry, pps: &Pps, segs: &[Segment<'_>]) -> Runs {
        let n = geo.size_ctbs;
        let w = geo.w_ctbs;
        let mut subs = vec![0u32; n];
        for t in 1..n {
            let r = geo.ts_to_rs[t] as usize;
            let start =
                first_in_tile(geo, t) || (pps.wpp_enabled && r % w == tile_col_start(geo, r));
            subs[t] = subs[t - 1] + u32::from(start);
        }
        let first = segs.first().map_or(n, |s| s.ts0).min(n);
        let mut of = Vec::with_capacity(n - first);
        let mut end = Vec::new();
        let mut seg = 0;
        for t in first..n {
            while segs.get(seg + 1).is_some_and(|s| s.ts0 <= t) {
                seg += 1;
            }
            let new_slice = segs.get(seg).is_some_and(|s| s.ts0 == t && !s.h.dependent);
            if t == first || new_slice || first_in_tile(geo, t) {
                if let Some(e) = end.last_mut() {
                    *e = t;
                }
                end.push(n);
            }
            of.push((end.len() - 1) as u32);
        }
        Runs {
            first,
            of,
            end,
            subs,
        }
    }

    /// The number of runs.
    pub fn len(&self) -> usize {
        self.end.len()
    }

    /// The run of tile-scan address `ts` (at or after the first segment).
    pub fn of(&self, ts: usize) -> usize {
        self.of
            .get(ts.wrapping_sub(self.first))
            .map_or(usize::MAX, |&r| r as usize)
    }

    /// One past the last tile-scan address of run `r`.
    pub fn end(&self, r: usize) -> usize {
        self.end.get(r).copied().unwrap_or(usize::MAX)
    }

    /// The index within `seg` of the substream starting at, or containing,
    /// tile-scan address `ts`.
    pub fn sub(&self, seg: &Segment<'_>, ts: usize) -> usize {
        (self.subs[ts] - self.subs[seg.ts0]) as usize
    }

    /// Substream starts at tile-scan addresses `a+1..=b`.
    pub fn subs_between(&self, a: usize, b: usize) -> usize {
        (self.subs[b] - self.subs[a]) as usize
    }
}

/// Builds the decoder state for `seg` over the given substream.
fn new_dec<'a>(
    sh: &Shared<'a>,
    pic: &'a mut Picture,
    fi: &'a mut FilterInfo,
    seg: &'a Segment<'a>,
    cab: Cabac<'a>,
) -> Dec<'a> {
    let (sps, pps) = (sh.sps, sh.pps);
    let h = SliceHeader::clone(seg.h);
    Dec {
        sps,
        pps,
        geo: sh.geo,
        sf: sh.sf,
        pic,
        fi,
        cat: sps.chroma_array_type,
        qp_bd_y: 6 * (sps.bit_depth_y as i32 - 8),
        qp_bd_c: 6 * (sps.bit_depth_c as i32 - 8),
        log2_qg: sps
            .log2_ctb
            .saturating_sub(pps.diff_cu_qp_delta_depth as usize),
        log2_cqg: sps
            .log2_ctb
            .saturating_sub(pps.diff_cu_chroma_qp_offset_depth as usize),
        ctb_rs: 0,
        qp_pred: h.qp,
        qp_last: h.qp,
        qp_y: h.qp,
        qp_delta_coded: false,
        cqo_coded: false,
        cqo_cb: 0,
        cqo_cr: 0,
        tq_bypass: false,
        intra_split: false,
        mode_y: [1; 4],
        mode_c: [1; 4],
        cbf_cb: [[false; 2]; 6],
        cbf_cr: [[false; 2]; 6],
        coeffs: [0; 1024],
        pred: [0; 1024],
        csbf: [0; 64],
        first_qg: true,
        stat_coeff: [0; 4],
        chroma_dm: [false; 4],
        cu_x: 0,
        cu_y: 0,
        cu_half: 0,
        res_y: [0; 1024],
        res_scale: [0; 2],
        ictx: sh.inter,
        sl: sh
            .inter
            .filter(|_| h.is_inter())
            .and_then(|c| SliceRefs::new(c, &h))
            .unwrap_or_default(),
        cu_inter: false,
        rdpcm: 0,
        inter_split: false,
        cip: pps.constrained_intra_pred && sh.inter.is_some(),
        mc: sh.inter.map(|_| McScratch::new()),
        sh: h,
        cab,
    }
}

/// The loop-filter parameters every CTB of `h` carries.
fn ctb_template(pps: &Pps, h: &SliceHeader) -> CtbFilter {
    CtbFilter {
        deblock_disabled: h.deblock_disabled,
        beta_offset_div2: h.beta_offset_div2.clamp(-6, 6) as i8,
        tc_offset_div2: h.tc_offset_div2.clamp(-6, 6) as i8,
        across_slices: h.across_slices,
        // cQpPicOffset of clause 8.7.2.5.5 is the PPS offset alone: the
        // slice-level and CU-level chroma QP offsets do not reach the filter.
        cb_qp_offset: pps.cb_qp_offset as i16,
        cr_qp_offset: pps.cr_qp_offset as i16,
        sao: Default::default(),
    }
}

/// Initialises the entropy state at the start of a substream or segment
/// (clause 9.3.1): fresh at a tile start, from the CTB above and to the right
/// at a wavefront row start, from the previous segment at the start of a
/// dependent segment, fresh otherwise. Also resets `qPY_PREV` where 8.6.1 says
/// it starts over.
fn init_entropy<E: Env>(
    d: &mut Dec<'_>,
    env: &mut E,
    ts: usize,
    rs: usize,
    seg_start: bool,
    ds: Option<&DsState>,
) {
    let geo = d.geo;
    let w = geo.w_ctbs;
    let fresh = |d: &mut Dec<'_>| {
        d.cab.ctx = initial_contexts_for(d.sh.init_type(), d.sh.qp);
        d.stat_coeff = [0; 4];
    };
    let reset_qp = |d: &mut Dec<'_>| {
        d.first_qg = true;
        d.qp_last = d.sh.qp;
    };
    if first_in_tile(geo, ts) {
        fresh(d);
        reset_qp(d);
    } else if d.pps.wpp_enabled && rs % w == tile_col_start(geo, rs) {
        let (x, y) = (rs % w, rs / w);
        let t = (y > 0 && x + 1 < w).then(|| rs - w + 1);
        let avail = t.filter(|&t| {
            geo.tile_id[t] == geo.tile_id[rs] && d.fi.slice_addr[t] == d.sh.slice_address
        });
        match avail.and_then(|t| env.wpp_load(t)) {
            Some(st) => {
                d.cab.ctx = st.ctx;
                d.stat_coeff = if d.sps.persistent_rice {
                    st.stat
                } else {
                    [0; 4]
                };
            }
            None => fresh(d),
        }
        reset_qp(d);
    } else if seg_start && d.sh.dependent {
        match ds {
            Some(ds) => {
                d.cab.ctx = ds.st.ctx;
                d.stat_coeff = if d.sps.persistent_rice {
                    ds.st.stat
                } else {
                    [0; 4]
                };
                d.first_qg = false;
                d.qp_last = ds.qp_last;
            }
            None => {
                fresh(d);
                reset_qp(d);
            }
        }
    } else {
        fresh(d);
        reset_qp(d);
    }
}

/// Decodes CTUs of `seg` starting at tile-scan address `ts_from`, which is
/// the first CTB of substream `sub` of the segment.
///
/// Decoding continues across substreams until the segment ends, fails, or
/// reaches a substream that starts at or after `stop`. `ds` is the
/// carry-over from the previous segment, for a dependent segment.
#[allow(clippy::too_many_arguments)]
pub fn run<E: Env>(
    sh: &Shared<'_>,
    pic: &mut Picture,
    fi: &mut FilterInfo,
    env: &mut E,
    seg: &Segment<'_>,
    ts_from: usize,
    sub: usize,
    stop: usize,
    ds: Option<&DsState>,
) -> Outcome {
    let geo = sh.geo;
    let Some((bytes, off)) = seg.substream(sub) else {
        return Outcome::Failed(ts_from, Error::InvalidData("entry point out of range"));
    };
    let cab = match Cabac::new(bytes, off, seg.h.qp) {
        Ok(c) => c,
        Err(e) => return Outcome::Failed(ts_from, e),
    };
    if seg.h.is_inter() {
        let Some(ctx) = sh.inter else {
            return Outcome::Failed(
                ts_from,
                Error::InvalidData("inter slice in a still picture"),
            );
        };
        if ctx.refs.is_empty() {
            return Outcome::Failed(
                ts_from,
                Error::InvalidData("inter slice without references"),
            );
        }
    }
    let template = ctb_template(sh.pps, seg.h);
    let mut d = new_dec(sh, pic, fi, seg, cab);
    let mut sub = sub;
    let mut ts = ts_from;
    // The substream start still to be initialised, once the row above is in.
    let mut pending_init = true;
    loop {
        if ts >= geo.size_ctbs {
            return Outcome::Failed(
                ts.saturating_sub(1),
                Error::InvalidData("slice runs past the end of the picture"),
            );
        }
        let rs = geo.ts_to_rs[ts] as usize;
        if let Err(Halt::Cancelled) = env.before_ctb(d.pic, d.fi, rs) {
            return Outcome::Cancelled;
        }
        if pending_init {
            init_entropy(&mut d, env, ts, rs, ts == seg.ts0, ds);
            pending_init = false;
        }
        d.fi.slice_addr[rs] = d.sh.slice_address;
        d.fi.ctb[rs] = template;
        if let Err(e) = coding_tree_unit(&mut d, rs) {
            return Outcome::Failed(ts, e);
        }
        if d.pps.wpp_enabled && rs % geo.w_ctbs == tile_col_start(geo, rs) + 1 {
            env.wpp_store(
                rs,
                CtxState {
                    ctx: d.cab.ctx,
                    stat: d.stat_coeff,
                },
            );
        }
        env.after_ctb(d.pic, d.fi, rs);
        match d.cab.terminate() {
            Ok(0) => {}
            Ok(_) => {
                return Outcome::Ended {
                    ts,
                    ds: DsState {
                        st: CtxState {
                            ctx: d.cab.ctx,
                            stat: d.stat_coeff,
                        },
                        qp_last: d.qp_last,
                    },
                };
            }
            Err(e) => return Outcome::Failed(ts, e),
        }
        let next = ts + 1;
        if next >= seg.ts_end {
            return Outcome::Failed(
                ts,
                Error::InvalidData("slice segment runs into the next one"),
            );
        }
        let nrs = geo.ts_to_rs[next] as usize;
        let new_tile = geo.tile_id[nrs] != geo.tile_id[rs];
        let new_row = d.pps.wpp_enabled && nrs % geo.w_ctbs == tile_col_start(geo, nrs);
        if new_tile || new_row {
            // end_of_subset_one_bit, then byte_alignment().
            match d.cab.terminate() {
                Ok(1) => {}
                Ok(_) => {
                    return Outcome::Failed(ts, Error::InvalidData("end_of_subset_one_bit is 0"));
                }
                Err(e) => return Outcome::Failed(ts, e),
            }
            sub += 1;
            if next >= stop {
                return Outcome::Paused;
            }
            // Without an entry point the next substream starts at the byte
            // boundary after end_of_subset_one_bit.
            let restarted = match seg.substream(sub) {
                Some((bytes, off)) => Cabac::new(bytes, off, d.sh.qp),
                None if seg.starts.len() <= 1 => {
                    let off = d.cab.aligned_pos();
                    Cabac::new(seg.data, off, d.sh.qp)
                }
                None => Err(Error::InvalidData("entry point out of range")),
            };
            match restarted {
                Ok(c) => d.cab = c,
                Err(e) => return Outcome::Failed(next, e),
            }
            pending_init = true;
        }
        ts = next;
    }
}

/// Converts the entry point offsets of `h` into RBSP byte offsets.
pub fn substream_starts(h: &SliceHeader, rbsp: &Rbsp) -> Vec<usize> {
    let mut starts = Vec::with_capacity(h.entry_points.len() + 1);
    starts.push(h.data_offset);
    let mut nal_off = rbsp.rbsp_to_nal(h.data_offset);
    for &e in &h.entry_points {
        nal_off = nal_off.saturating_add(e as usize);
        starts.push(rbsp.nal_to_rbsp(nal_off));
    }
    starts
}
