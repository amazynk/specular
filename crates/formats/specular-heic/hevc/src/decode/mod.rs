//! Picture level decoding: parameter set selection, slice dispatch,
//! concealment of what could not be decoded, filters.

mod conceal;
mod ctu;
mod geom;
pub(crate) mod inter;
mod mode;
mod pipe;
mod recon;
mod refs;
mod residual;
mod residual_ctx;
mod residual_levels;
mod sao_syn;
pub(crate) mod seq;
mod sets;
mod slice_data;
mod state;
mod tu;
mod wpp;

pub use geom::Geometry;
pub use sets::ParameterSets;

use crate::error::{Error, Result};
use crate::filter::{CtbFilter, FilterInfo, RowFilter, loop_filters};
use crate::nal::{NalHeader, Rbsp};
use crate::picture::Picture;
use crate::ps::scaling;
use crate::slice::{SliceHeader, parse as parse_slice_header};
use crate::{ChromaFormat, Frame};
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use conceal::LostCtb;
use core::sync::atomic::{AtomicBool, Ordering};
use slice_data::{CtxState, Env, Halt, Outcome, Runs, Segment, Shared};

/// Why a segment that ended before the next one starts is lost from there.
pub(crate) const ENDED_EARLY: Error = Error::InvalidData("slice segment ended early");

/// What failed in a picture: per run ([`Runs`]), the first tile-scan
/// address lost (`usize::MAX` for none), and the failure at the smallest
/// address, with why.
pub(crate) struct Fails {
    pub runs: Vec<usize>,
    pub worst: Option<(usize, Error)>,
}

impl Fails {
    /// Records that run `run` is lost from tile-scan address `ts` on.
    fn fail(&mut self, run: usize, ts: usize, e: Error) {
        if let Some(f) = self.runs.get_mut(run) {
            *f = (*f).min(ts);
        }
        if self.worst.is_none_or(|(t, _)| ts < t) {
            self.worst = Some((ts, e));
        }
    }

    /// Whether the CTB at tile-scan address `ts`, covered by a segment, is
    /// lost.
    fn lost(&self, runs: &Runs, ts: usize) -> bool {
        self.runs.get(runs.of(ts)).is_some_and(|&f| f <= ts)
    }
}

/// A decoded picture plus what concealment had to do.
pub struct Output {
    /// The cropped picture.
    pub frame: Frame,
    /// CTUs that could not be decoded and were concealed.
    pub damaged: usize,
    /// CTUs in the picture.
    pub total: usize,
    /// The VUI video signal type of the SPS, when signalled.
    pub signal: Option<crate::ps::vui::Signal>,
}

/// The serial path's [`Env`]: everything is already in the picture. With
/// a row pipeline, the CTB rows that are final by the time a CTB starts are
/// concealed, filtered and published first.
struct SerialEnv<'c, 'p> {
    cancel: Option<&'c AtomicBool>,
    w: usize,
    wpp: BTreeMap<usize, CtxState>,
    post: Option<(Post, PostCx<'p>)>,
}

impl Env for SerialEnv<'_, '_> {
    fn before_ctb(
        &mut self,
        pic: &mut Picture,
        fi: &mut FilterInfo,
        rs: usize,
    ) -> core::result::Result<(), Halt> {
        if rs.is_multiple_of(self.w) && self.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(Halt::Cancelled);
        }
        if let Some((p, cx)) = &mut self.post {
            // Every CTB before this one in tile scan is final: the segments
            // before were settled when they ended, and this one has not
            // failed before this CTB.
            let ts = cx.geo.rs_to_ts[rs] as usize;
            p.settle(cx, pic, fi, ts);
        }
        Ok(())
    }
    fn wpp_load(&mut self, rs: usize) -> Option<CtxState> {
        self.wpp.get(&rs).copied()
    }
    fn wpp_store(&mut self, rs: usize, st: CtxState) {
        self.wpp.insert(rs, st);
    }
    fn after_ctb(&mut self, _pic: &Picture, _fi: &FilterInfo, _rs: usize) {}
}

/// Decodes one still picture from its parameter sets and slice NAL units.
///
/// `threads` above one decodes wavefront rows or tiles in parallel when the stream
/// allows it; the result is the same either way, damaged streams included.
pub fn decode(
    sets: &ParameterSets,
    slices: &[&[u8]],
    threads: usize,
    cancel: Option<&AtomicBool>,
) -> Result<Output> {
    if slices.is_empty() {
        return Err(Error::NoSlices);
    }
    // The first error seen anywhere, reported if nothing at all decodes.
    let mut first_err: Option<Error> = None;
    let mut parsed: Vec<(NalHeader, Rbsp)> = Vec::with_capacity(slices.len());
    let mut non_irap = false;
    for nal in slices {
        let h = match NalHeader::parse(nal) {
            Ok(h) => h,
            Err(e) => {
                first_err.get_or_insert(e);
                continue;
            }
        };
        if !h.is_vcl() || h.layer_id != 0 {
            continue;
        }
        if !h.is_irap() {
            non_irap = true;
            continue;
        }
        match Rbsp::from_nal(nal) {
            Ok(r) => parsed.push((h, r)),
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    if parsed.is_empty() {
        return Err(if non_irap {
            Error::Unsupported("non-IRAP picture (inter prediction)")
        } else {
            first_err.unwrap_or(Error::NoSlices)
        });
    }
    let out = decode_parsed(sets, &parsed, threads, cancel, first_err)?;
    let planes = [&out.pic.y, &out.pic.cb, &out.pic.cr];
    let line = |c: usize, y: usize| {
        let p = planes[c];
        let o = p.offset(0, y);
        &p.data[o..o + p.width]
    };
    Ok(Output {
        frame: crop(line, &OutGeom::of(out.sps)),
        damaged: out.damaged,
        total: out.total,
        signal: out.sps.signal,
    })
}

/// A decoded, loop-filtered picture before cropping.
pub(crate) struct PicOut<'s> {
    /// The picture, whole coded size.
    pub pic: Picture,
    /// The SPS it was decoded with.
    pub sps: &'s crate::ps::Sps,
    /// CTUs concealed.
    pub damaged: usize,
    /// CTUs in the picture.
    pub total: usize,
}

/// What decoding a picture needs once its slice segment headers are
/// parsed. Owns everything, so that every thread decoding the picture can
/// share one plan; [`Plan::sets`] and [`Plan::segs`] give back what it
/// refers to in the parameter sets and slice NAL units it was made from.
pub(crate) struct Plan {
    pps_id: u32,
    geo: Geometry,
    factors: Option<alloc::boxed::Box<scaling::ScalingFactors>>,
    /// The filter parameters before any CTB is decoded.
    fi: FilterInfo,
    heads: Vec<SegHead>,
    /// Some slice is an inter slice, and the picture has references.
    has_inter: bool,
    first_err: Option<Error>,
}

/// A slice segment of a [`Plan`]: [`Segment`] without its RBSP.
struct SegHead {
    h: SliceHeader,
    /// The slice NAL unit it was parsed from.
    nal: usize,
    starts: Vec<usize>,
    ts0: usize,
    ts_end: usize,
}

impl Plan {
    /// The parameter sets the plan was made with, from the same `sets`.
    pub fn sets<'s>(
        &self,
        sets: &'s ParameterSets,
    ) -> Result<(&'s crate::ps::Sps, &'s crate::ps::Pps)> {
        let pps = sets.pps(self.pps_id)?;
        Ok((sets.sps(pps.sps_id)?, pps))
    }

    /// The slice segments, from the same `parsed` the plan was made from.
    pub fn segs<'p>(&'p self, parsed: &'p [(NalHeader, Rbsp)]) -> Vec<Segment<'p>> {
        self.heads
            .iter()
            .map(|s| Segment {
                h: &s.h,
                data: &parsed[s.nal].1.data,
                starts: &s.starts,
                ts0: s.ts0,
                ts_end: s.ts_end,
            })
            .collect()
    }
}

/// Chooses the parameter sets of a picture and parses its slice segment
/// headers. `inter` when the picture belongs to a sequence (inter slices
/// can be decoded). An error means nothing of the picture decodes.
pub(crate) fn plan(
    sets: &ParameterSets,
    parsed: &[(NalHeader, Rbsp)],
    inter: bool,
    mut first_err: Option<Error>,
) -> Result<Plan> {
    if parsed.is_empty() {
        return Err(first_err.unwrap_or(Error::NoSlices));
    }
    // The PPS of the first slice whose PPS is known decides the picture.
    let mut pps = None;
    for (nh, rbsp) in parsed {
        let r = &mut crate::bits::BitReader::new(&rbsp.data);
        let id = r.u1().and_then(|_| {
            if nh.is_irap() {
                r.u1()?;
            }
            r.ue()
        });
        match id.and_then(|id| sets.pps(id)) {
            Ok(p) => {
                pps = Some(p);
                break;
            }
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    let pps = pps.ok_or(first_err.unwrap_or(Error::NoSlices))?;
    let sps = sets.sps(pps.sps_id)?;
    // Cross-component prediction is restricted to 4:4:4 (7.4.3.3.2).
    if pps.cross_component_prediction && sps.chroma_array_type != 3 {
        return Err(Error::InvalidData(
            "cross_component_prediction_enabled_flag without 4:4:4",
        ));
    }
    // log2_sao_offset_scale_luma/chroma range (7.4.3.3.2).
    if pps.sao_offset_scale_luma > u32::from(sps.bit_depth_y.saturating_sub(10))
        || pps.sao_offset_scale_chroma > u32::from(sps.bit_depth_c.saturating_sub(10))
    {
        return Err(Error::InvalidData("log2_sao_offset_scale out of range"));
    }
    let geo = Geometry::new(sps, pps)?;
    let factors = scaling_factors(sps, pps);
    let mut fi = FilterInfo {
        bit_depth_y: sps.bit_depth_y,
        bit_depth_c: sps.bit_depth_c,
        chroma_array_type: sps.chroma_array_type,
        sub_w: sps.sub_w,
        sub_h: sps.sub_h,
        ctb_log2: sps.log2_ctb,
        pic_w_ctbs: geo.w_ctbs,
        pic_h_ctbs: geo.h_ctbs,
        pcm_loop_filter_disabled: sps.pcm_loop_filter_disabled,
        across_tiles: pps.across_tiles,
        sao_enabled: false,
        ctb: vec![CtbFilter::default(); geo.size_ctbs],
        tile_id: geo.tile_id.clone(),
        slice_addr: vec![u32::MAX; geo.size_ctbs],
    };

    // Every slice segment whose header parses, in increasing address
    // order. A dependent segment needs the header before it, so one that
    // follows a dropped segment is dropped too.
    let mut segs: Vec<SegHead> = Vec::with_capacity(parsed.len());
    let mut prev: Option<SliceHeader> = None;
    for (nal, (nh, rbsp)) in parsed.iter().enumerate() {
        let h = match parse_slice_header(nh, &rbsp.data, sps, pps, prev.as_ref()) {
            Ok(h) if h.pps_id == pps.id => h,
            Ok(_) => {
                first_err.get_or_insert(Error::Unsupported("a picture using more than one PPS"));
                prev = None;
                continue;
            }
            Err(e) => {
                first_err.get_or_insert(e);
                prev = None;
                continue;
            }
        };
        let ts0 = geo.rs_to_ts[h.segment_address as usize] as usize;
        if segs.last().is_some_and(|l| l.ts0 >= ts0) {
            first_err.get_or_insert(Error::InvalidData("slice segments out of order"));
            prev = None;
            continue;
        }
        let starts = slice_data::substream_starts(&h, rbsp);
        prev = Some(h.clone());
        segs.push(SegHead {
            h,
            nal,
            starts,
            ts0,
            ts_end: geo.size_ctbs,
        });
    }
    if segs.is_empty() {
        return Err(first_err.unwrap_or(Error::NoSlices));
    }
    for i in 1..segs.len() {
        segs[i - 1].ts_end = segs[i].ts0;
    }
    for s in &segs {
        fi.sao_enabled |= s.h.sao_luma || s.h.sao_chroma;
    }
    let has_inter = inter && segs.iter().any(|s| s.h.is_inter());
    Ok(Plan {
        pps_id: pps.id,
        geo,
        factors,
        fi,
        heads: segs,
        has_inter,
        first_err,
    })
}

/// Decodes the VCL NAL units of one still picture: wavefront rows or tiles
/// in parallel when `threads` allows and the stream has them, then the loop
/// filters over the whole picture. `first_err` is reported when nothing
/// decodes.
fn decode_parsed<'s>(
    sets: &'s ParameterSets,
    parsed: &[(NalHeader, Rbsp)],
    threads: usize,
    cancel: Option<&AtomicBool>,
    first_err: Option<Error>,
) -> Result<PicOut<'s>> {
    let plan = plan(sets, parsed, false, first_err)?;
    let (sps, pps) = plan.sets(sets)?;
    let segs = plan.segs(parsed);
    let Plan {
        ref geo,
        ref factors,
        ref first_err,
        ..
    } = plan;
    let mut fi = plan.fi.clone();
    let first_err = *first_err;
    let mut pic = Picture::new(sps.width, sps.height, sps.sub_w, sps.sub_h)?;
    let shared = Shared {
        sps,
        pps,
        geo,
        sf: factors.as_deref(),
        inter: None,
    };
    let runs = Runs::new(geo, pps, &segs);
    let units = (threads > 1)
        .then(|| wpp::units(&shared, &segs, &runs, threads))
        .flatten()
        .filter(|u| u.list.len() >= 2);
    let fails = if let Some(units) = units {
        wpp::decode(
            &shared, &segs, &units, &runs, &mut pic, &mut fi, threads, cancel,
        )?
    } else {
        let mut env = SerialEnv {
            cancel,
            w: geo.w_ctbs,
            wpp: BTreeMap::new(),
            post: None,
        };
        decode_serial(&shared, &segs, &runs, &mut pic, &mut fi, &mut env)?
    };

    // Lost CTBs: anything before the first segment, and in each run
    // everything from its first failure to its end.
    let mut lost: Vec<Option<LostCtb>> = vec![None; geo.size_ctbs];
    let uncovered = segs.first().map_or(geo.size_ctbs, |s| s.ts0);
    for ts in 0..uncovered {
        let rs = geo.ts_to_rs[ts] as usize;
        lost[rs] = Some(LostCtb {
            slice_addr: rs as u32,
            qp: pps.init_qp,
        });
    }
    for s in &segs {
        for ts in (s.ts0..s.ts_end).filter(|&ts| fails.lost(&runs, ts)) {
            let rs = geo.ts_to_rs[ts] as usize;
            lost[rs] = Some(LostCtb {
                slice_addr: s.h.slice_address,
                qp: s.h.qp,
            });
        }
    }
    let damaged = lost.iter().filter(|l| l.is_some()).count();
    if damaged == geo.size_ctbs {
        return Err(fails
            .worst
            .map(|w| w.1)
            .or(first_err)
            .unwrap_or(Error::NoSlices));
    }
    if damaged > 0 {
        conceal::conceal(
            &mut pic,
            &mut fi,
            geo,
            &lost,
            (sps.bit_depth_y, sps.bit_depth_c),
            (sps.sub_w, sps.sub_h),
            None,
        );
    }
    loop_filters(&mut pic, &fi, threads);
    Ok(PicOut {
        pic,
        sps,
        damaged,
        total: geo.size_ctbs,
    })
}

/// Decodes one picture of a sequence, concealing, loop-filtering and
/// publishing each CTB row into `out` as soon as it is final, so that
/// pictures predicted from it can start on it while it is still being
/// decoded. `src` is the picture lost CTBs are copied from.
///
/// With `par` (a thread budget above one, and a way to start helpers on
/// the sequence's pool) a picture with wavefront rows or several tiles is
/// decoded in parallel ([`pipe`]); otherwise on the calling thread. The
/// result is the same either way.
///
/// Returns the CTUs concealed: a picture of which nothing decodes is
/// concealed whole (from `src`, else grey), which is what a picture
/// standing in for a lost one is. Errors (from planning, or cancellation)
/// leave `out` with the rows published so far, not finished.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_piped(
    plan: &Plan,
    sets: &ParameterSets,
    parsed: &[(NalHeader, Rbsp)],
    ictx: &inter::InterCtx,
    cancel: Option<&AtomicBool>,
    src: Option<&inter::RefPic>,
    out: &inter::RefPic,
    par: Option<(usize, &pipe::Spawn<'_>)>,
) -> Result<usize> {
    let (sps, pps) = plan.sets(sets)?;
    let segs = plan.segs(parsed);
    let (geo, has_inter) = (&plan.geo, plan.has_inter);
    let mut fi = plan.fi.clone();
    let mut pic = Picture::new(sps.width, sps.height, sps.sub_w, sps.sub_h)?;
    let dims = [
        (pic.y.width, pic.y.height),
        (pic.cb.width, pic.cb.height),
        (pic.cr.width, pic.cr.height),
    ];
    if !out.layout_is(dims, geo.ctb_log2, sps.sub_h) {
        return Err(Error::InvalidData("reference picture layout mismatch"));
    }
    pic.mvf = vec![crate::picture::MvField::default(); pic.cu_flags.len()];
    if has_inter {
        pic.bs = vec![0; pic.cu_flags.len()];
    }
    let shared = Shared {
        sps,
        pps,
        geo,
        sf: plan.factors.as_deref(),
        inter: Some(ictx),
    };
    let post = Post::new(geo, sps, pps, has_inter);
    let cx = PostCx {
        geo,
        src,
        out,
        ictx,
    };
    let runs = Runs::new(geo, pps, &segs);
    if let Some((threads, spawn)) = par
        && threads > 1
        && let Some(units) = wpp::units(&shared, &segs, &runs, threads)
        && units.list.len() >= 2
    {
        return pipe::lead(pipe::Lead {
            units,
            runs,
            threads,
            sh: &shared,
            segs: &segs,
            pic,
            fi,
            post,
            cx: &cx,
            cancel,
            spawn,
        });
    }
    let mut env = SerialEnv {
        cancel,
        w: geo.w_ctbs,
        wpp: BTreeMap::new(),
        post: Some((post, cx)),
    };
    decode_serial(&shared, &segs, &runs, &mut pic, &mut fi, &mut env)?;
    let Some((mut post, cx)) = env.post.take() else {
        return Err(Error::InvalidData("row pipeline lost"));
    };
    post.settle(&cx, &mut pic, &mut fi, geo.size_ctbs);
    Ok(post.damaged)
}

/// What the row pipeline reads and writes besides the picture: the same
/// for every thread working on it.
struct PostCx<'p> {
    geo: &'p Geometry,
    src: Option<&'p inter::RefPic>,
    out: &'p inter::RefPic,
    ictx: &'p inter::InterCtx,
}

/// The row pipeline of [`decode_piped`]: which CTBs are lost, and how far
/// concealment, the loop filters and publication have got.
struct Post {
    /// Per CTB raster address: lost, and what to.
    lost: Vec<Option<LostCtb>>,
    /// Per CTB row: one past the largest tile-scan address in it.
    row_end: Vec<usize>,
    /// CTB rows concealed, with their boundary strengths.
    rows_done: usize,
    filt: RowFilter,
    init_qp: i32,
    bit_depth: (u8, u8),
    sub: (usize, usize),
    has_inter: bool,
    damaged: usize,
}

impl Post {
    /// Nothing settled yet.
    fn new(geo: &Geometry, sps: &crate::ps::Sps, pps: &crate::ps::Pps, has_inter: bool) -> Post {
        Post {
            lost: vec![None; geo.size_ctbs],
            row_end: (0..geo.h_ctbs)
                .map(|r| {
                    (r * geo.w_ctbs..(r + 1) * geo.w_ctbs)
                        .map(|rs| geo.rs_to_ts[rs] as usize + 1)
                        .max()
                        .unwrap_or(0)
                })
                .collect(),
            rows_done: 0,
            filt: RowFilter::new(),
            init_qp: pps.init_qp,
            bit_depth: (sps.bit_depth_y, sps.bit_depth_c),
            sub: (sps.sub_w, sps.sub_h),
            has_inter,
            damaged: 0,
        }
    }

    /// Marks the CTBs at tile-scan addresses `ts` lost, attributed to the
    /// slice at `attr.0` with QP `attr.1`, or else each to itself with the
    /// picture's initial QP.
    fn lose(&mut self, geo: &Geometry, ts: core::ops::Range<usize>, attr: Option<(u32, i32)>) {
        for t in ts {
            let rs = geo.ts_to_rs[t] as usize;
            let (slice_addr, qp) = attr.unwrap_or((rs as u32, self.init_qp));
            if self.lost[rs].replace(LostCtb { slice_addr, qp }).is_none() {
                self.damaged += 1;
            }
        }
    }

    /// Every CTB before tile-scan address `upto` is final (decoded, or
    /// marked lost): conceals, filters and publishes the rows that allows.
    fn settle(&mut self, cx: &PostCx<'_>, pic: &mut Picture, fi: &mut FilterInfo, upto: usize) {
        let mut rows = self.rows_done;
        while rows < cx.geo.h_ctbs && self.row_end[rows] <= upto {
            rows += 1;
        }
        self.settle_rows(cx, pic, fi, rows);
    }

    /// Every CTB of the first `rows` CTB rows is final: conceals, filters
    /// and publishes the rows that allows.
    fn settle_rows(
        &mut self,
        cx: &PostCx<'_>,
        pic: &mut Picture,
        fi: &mut FilterInfo,
        rows: usize,
    ) {
        let geo = cx.geo;
        let ctb_log2 = geo.ctb_log2;
        let first = self.rows_done;
        while self.rows_done < rows.min(geo.h_ctbs) {
            let r = self.rows_done;
            for rs in r * geo.w_ctbs..(r + 1) * geo.w_ctbs {
                if let Some(l) = self.lost[rs] {
                    conceal::conceal_ctb(pic, fi, geo, rs, l, self.bit_depth, self.sub, cx.src);
                }
            }
            if self.has_inter {
                inter::bs::derive_rows(pic, (r << ctb_log2) / 8, ((r + 1) << ctb_log2) / 8);
            }
            self.rows_done += 1;
        }
        if self.rows_done == first {
            return;
        }
        self.filt.advance(pic, fi, self.rows_done, |pic, r, lines| {
            publish(cx, pic, r, lines)
        });
    }
}

/// Hands the final row `r`, whose lines per component are `lines`, to the
/// reference picture, with the motion of its 16x16 blocks.
fn publish(cx: &PostCx<'_>, pic: &Picture, r: usize, lines: [Vec<u16>; 3]) {
    let out = cx.out;
    let [y, cb, cr] = lines.map(Vec::into_boxed_slice);
    let row = inter::refpic::Row {
        planes: [y, cb, cr],
    };
    let cols = if out.has_col() {
        let w16 = pic.y.width.div_ceil(16);
        out.col_rows(r)
            .map(|k| inter::compress_row(&pic.mvf, pic.min4_w, w16, k, cx.ictx))
            .collect()
    } else {
        Vec::new()
    };
    out.publish(r, Some(row), cols);
}

/// Decodes the segments one after another. A run ([`Runs`]: a slice
/// within a tile) that fails is not decoded further, the rest of it is lost
/// anyway; decoding goes on at the next tile. With a row pipeline in `env`,
/// the CTBs are settled as they become final.
fn decode_serial(
    sh: &Shared<'_>,
    segs: &[Segment<'_>],
    runs: &Runs,
    pic: &mut Picture,
    fi: &mut FilterInfo,
    env: &mut SerialEnv<'_, '_>,
) -> Result<Fails> {
    let mut fails = Fails {
        runs: vec![usize::MAX; runs.len()],
        worst: None,
    };
    let mut ds = None;
    if let Some((p, cx)) = &mut env.post {
        let uncovered = segs.first().map_or(sh.geo.size_ctbs, |s| s.ts0);
        p.lose(cx.geo, 0..uncovered, None);
        p.settle(cx, pic, fi, uncovered);
    }
    for seg in segs {
        let mut carry = ds.take().filter(|_| seg.h.dependent);
        let attr = Some((seg.h.slice_address, seg.h.qp));
        let mut a = seg.ts0;
        while a < seg.ts_end {
            let r = runs.of(a);
            let stop = runs.end(r).min(seg.ts_end);
            if fails.runs[r] <= a {
                if let Some((p, cx)) = &mut env.post {
                    p.lose(cx.geo, a..stop, attr);
                    p.settle(cx, pic, fi, stop);
                }
                a = stop;
                carry = None;
                continue;
            }
            let fail = match slice_data::run(
                sh,
                pic,
                fi,
                env,
                seg,
                a,
                runs.sub(seg, a),
                usize::MAX,
                carry.take().as_ref(),
            ) {
                Outcome::Ended { ts, ds: d } => {
                    if ts + 1 < seg.ts_end {
                        Some((ts, ts + 1, ENDED_EARLY))
                    } else {
                        ds = Some(d);
                        None
                    }
                }
                Outcome::Paused => Some((a, a, Error::InvalidData("slice segment paused"))),
                Outcome::Failed(ts, e) => Some((ts, ts, e)),
                Outcome::Cancelled => return Err(Error::Cancelled),
            };
            let Some((at, from, e)) = fail else {
                break;
            };
            // `at` is the CTB the run failed in (or after): the rest of
            // its run is lost from `from`.
            let rr = runs.of(at);
            let end = runs.end(rr).min(seg.ts_end);
            fails.fail(rr, from, e);
            if let Some((p, cx)) = &mut env.post {
                p.lose(cx.geo, from.min(end)..end, attr);
                p.settle(cx, pic, fi, end);
            }
            a = end;
        }
        if let Some((p, cx)) = &mut env.post {
            p.settle(cx, pic, fi, seg.ts_end);
        }
    }
    Ok(fails)
}

/// Derives the scaling factors in force for the picture, if any.
fn scaling_factors(
    sps: &crate::ps::Sps,
    pps: &crate::ps::Pps,
) -> Option<alloc::boxed::Box<scaling::ScalingFactors>> {
    if !sps.scaling_list_enabled {
        return None;
    }
    let data = match (&pps.scaling, &sps.scaling) {
        (Some(d), _) => d.clone(),
        (None, Some(d)) => d.clone(),
        (None, None) => scaling::ScalingListData::default(),
    };
    Some(scaling::derive(&data, sps.chroma_array_type))
}

/// What output needs to know of the SPS a picture was decoded with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OutGeom {
    /// Conformance window offsets, luma samples: left, right, top, bottom.
    pub crop: [usize; 4],
    /// Coded luma width.
    pub width: usize,
    /// Coded luma height.
    pub height: usize,
    /// `SubWidthC`, zero for monochrome.
    pub sub_w: usize,
    /// `SubHeightC`, zero for monochrome.
    pub sub_h: usize,
    /// `BitDepthY`.
    pub bit_depth_y: u8,
    /// `BitDepthC`.
    pub bit_depth_c: u8,
    /// `ChromaArrayType`.
    pub chroma_array_type: u8,
    /// The VUI video signal type.
    pub signal: Option<crate::ps::vui::Signal>,
}

impl OutGeom {
    /// The output geometry of `sps`.
    pub fn of(sps: &crate::ps::Sps) -> OutGeom {
        OutGeom {
            crop: sps.crop,
            width: sps.width,
            height: sps.height,
            sub_w: sps.sub_w,
            sub_h: sps.sub_h,
            bit_depth_y: sps.bit_depth_y,
            bit_depth_c: sps.bit_depth_c,
            chroma_array_type: sps.chroma_array_type,
            signal: sps.signal,
        }
    }
}

/// Applies the conformance window and packs the planes into a [`Frame`].
/// `line(c, y)` is line `y` of plane `c`, whole coded width.
pub(crate) fn crop<'a>(line: impl Fn(usize, usize) -> &'a [u16], g: &OutGeom) -> Frame {
    let [l, r, t, b] = g.crop;
    let w = g.width.saturating_sub(l + r).max(1);
    let h = g.height.saturating_sub(t + b).max(1);
    let (sw, sh) = if g.sub_w == 0 {
        (1, 1)
    } else {
        (g.sub_w, g.sub_h)
    };
    let (cw, ch) = if g.sub_w == 0 {
        (0, 0)
    } else {
        (w / sw, h / sh)
    };
    let mut y = Vec::with_capacity(w * h);
    for j in 0..h {
        y.extend_from_slice(&line(0, t + j)[l..l + w]);
    }
    let mut cb = Vec::with_capacity(cw * ch);
    let mut cr = Vec::with_capacity(cw * ch);
    for j in 0..ch {
        let x = l / sw;
        cb.extend_from_slice(&line(1, t / sh + j)[x..x + cw]);
        cr.extend_from_slice(&line(2, t / sh + j)[x..x + cw]);
    }
    Frame {
        width: w as u32,
        height: h as u32,
        bit_depth: g.bit_depth_y,
        bit_depth_chroma: g.bit_depth_c,
        chroma: ChromaFormat::from_idc(g.chroma_array_type).unwrap_or(ChromaFormat::Monochrome),
        y,
        cb,
        cr,
        y_stride: w as u32,
        c_stride: cw as u32,
    }
}
