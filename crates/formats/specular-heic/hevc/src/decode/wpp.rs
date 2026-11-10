//! Parallel decoding inside one picture, in units that the bitstream lets
//! decode independently of each other: wavefront rows (one CTB row per
//! unit, each starting once the row above is two CTBs ahead — clause
//! 9.3.1's dependency, which is also the intra-prediction dependency), or
//! tiles (one tile per unit, nothing shared but the loop filters, which
//! run afterwards; wavefront rows inside a tile run in order).
//!
//! Each worker decodes a unit one CTB row at a time, each row into a band
//! holding it plus eight sample rows of halo above. What a CTB row needs
//! from the row above — its bottom sample row, its coding-tree depths, and
//! in inter pictures the coding-unit flags and motion of its bottom 4x4 row
//! (skip contexts, merge and AMVP neighbours, constrained intra
//! prediction) — is copied into the band below: inside a tile from the
//! band before, and between wavefront rows CTB by CTB through the row's
//! [`Halo`] (with the slice addresses, SAO parameters and wavefront context
//! state), just before it is needed. A tile reads nothing outside itself.
//! Each finished CTB row of a unit goes to the caller's sink, which
//! [`merge`]s it into the picture, so the rows of a picture with several
//! tile columns can be filtered while its tiles are still being decoded.
//! Motion compensation and temporal motion vector prediction read only
//! reference pictures, which are complete or waited on row by row.
//!
//! Parsing is done by [`run`] exactly as on the serial path, so a damaged
//! stream fails at the same CTB; the caller turns per-run failures
//! ([`Runs`]) into lost CTBs the same way for both.
//!
//! Still pictures run the workers on scoped threads ([`decode`]); pictures
//! of a sequence run them on the sequence's pool ([`super::pipe`]).

use super::Fails;
use super::slice_data::{
    CtxState, DsState, Env, Halt, Outcome, Runs, Segment, Shared, first_in_tile, run,
    tile_col_start,
};
use crate::error::{Error, Result};
use crate::filter::{CtbFilter, FilterInfo};
use crate::picture::{MvField, Picture};
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};

/// One stretch of a unit decoded by one call to [`run`].
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    seg: usize,
    /// First and one-past-last tile-scan address.
    a: usize,
    b: usize,
    /// Substream index of `a` within the segment.
    sub: usize,
}

/// A part of the picture decoded by one worker.
#[derive(Debug)]
pub struct Unit {
    pieces: Vec<Piece>,
    /// CTB rows `r0..r1` and columns `c0..c1` covered.
    pub r0: usize,
    pub r1: usize,
    pub c0: usize,
    pub c1: usize,
    /// For a wavefront row, the unit of the row above it in its tile (or
    /// picture), if there is one.
    above: Option<usize>,
    /// For a wavefront row, whether a unit waits on it.
    below: bool,
}

/// How a picture splits into units.
#[derive(Debug)]
pub struct Units {
    /// Each unit after any unit it waits on.
    pub list: Vec<Unit>,
    /// The units are wavefront rows, each waiting on the one above it.
    pub rows: bool,
}

/// How the picture splits into units decoded in parallel, if it does:
/// wavefront rows when wavefronts are on without tiles; tiles when there
/// are several, or with wavefronts too and fewer tiles than `threads`, the
/// wavefront rows of the tiles; in every case only when every segment
/// carries an entry point for each substream it starts.
pub fn units(sh: &Shared<'_>, segs: &[Segment<'_>], runs: &Runs, threads: usize) -> Option<Units> {
    let geo = sh.geo;
    let (w, n) = (geo.w_ctbs, geo.size_ctbs);
    let tiles = geo.col_width.len() * geo.row_height.len();
    let rows = sh.pps.wpp_enabled
        && geo.h_ctbs >= 2
        && (!sh.pps.tiles_enabled || (tiles < threads && geo.row_height.iter().any(|&h| h >= 2)));
    let by_tile = sh.pps.tiles_enabled && tiles >= 2;
    if segs.is_empty() || !(rows || by_tile) || w == 0 {
        return None;
    }
    let rs = |t: usize| geo.ts_to_rs[t] as usize;
    for s in segs {
        if s.ts0 >= s.ts_end
            || s.ts_end > n
            || runs.subs_between(s.ts0, s.ts_end - 1) + 1 != s.starts.len()
        {
            return None;
        }
    }
    let bounds: Vec<usize> = (0..n)
        .filter(|&t| {
            t == 0 || first_in_tile(geo, t) || (rows && rs(t) % w == tile_col_start(geo, rs(t)))
        })
        .collect();
    let mut list: Vec<Unit> = bounds
        .iter()
        .enumerate()
        .map(|(i, &t0)| {
            let t1 = bounds.get(i + 1).copied().unwrap_or(n);
            let (f, l) = (rs(t0), rs(t1 - 1));
            // Segments are in order and contiguous (checked by the caller's
            // plan: each ends where the next starts).
            let first = segs.partition_point(|s| s.ts_end <= t0);
            let pieces = segs[first..]
                .iter()
                .enumerate()
                .take_while(|(_, s)| s.ts0 < t1)
                .map(|(k, s)| {
                    let a = s.ts0.max(t0);
                    Piece {
                        seg: first + k,
                        a,
                        b: s.ts_end.min(t1),
                        sub: runs.sub(s, a),
                    }
                })
                .collect();
            Unit {
                pieces,
                r0: f / w,
                r1: l / w + 1,
                c0: f % w,
                c1: l % w + 1,
                above: None,
                below: false,
            }
        })
        .collect();
    if rows {
        // Row by row, so that the row above a unit is claimed before it,
        // and the rows of all tiles across the picture finish together.
        list.sort_by_key(|u| (u.r0, u.c0));
        for i in 0..list.len() {
            let (r0, c0) = (list[i].r0, list[i].c0);
            if r0 > 0 && geo.tile_id[(r0 - 1) * w + c0] == geo.tile_id[r0 * w + c0] {
                let j = list[..i].partition_point(|u| (u.r0, u.c0) < (r0 - 1, c0));
                list[i].above = Some(j);
                list[j].below = true;
            }
        }
    }
    Some(Units { list, rows })
}

/// What one row needs to hand to the row below, for the row's columns
/// from luma column `x0` (chroma column `cx0`, CTB column `c0`) on.
struct Halo {
    x0: usize,
    cx0: usize,
    c0: usize,
    luma: Vec<u16>,
    cb: Vec<u16>,
    cr: Vec<u16>,
    depth: Vec<u8>,
    /// Coding-unit flags and motion of the bottom 4x4 row (inter pictures).
    flags: Vec<u8>,
    mvf: Vec<MvField>,
    slice_addr: Vec<u32>,
    ctb: Vec<CtbFilter>,
    wpp: Option<CtxState>,
}

/// State shared by all workers of one picture.
pub struct Sync {
    /// Per wavefront row, the number of leading CTBs that are done (decoded
    /// or given up on).
    progress: Mutex<Vec<usize>>,
    cv: Condvar,
    halos: Vec<Mutex<Halo>>,
    aborted: AtomicBool,
    next: AtomicUsize,
    /// The failure at the smallest tile-scan address, and why.
    worst: Mutex<Option<(usize, Error)>>,
    /// Per run, the first tile-scan address known to be lost.
    run_fail: Vec<AtomicUsize>,
    /// Bands merged and free for another unit row.
    pool: Mutex<Vec<Picture>>,
}

/// At most this many free bands are kept: enough for every worker to have
/// one in the queue and one being merged.
const POOL: usize = 32;

pub(super) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Sync {
    /// Fresh state for `units` of a picture of `runs`.
    pub fn new(units: &Units, dims: Dims, runs: &Runs) -> Sync {
        let halos = if units.rows { &units.list[..] } else { &[] };
        Sync {
            progress: Mutex::new(vec![0; units.list.len()]),
            cv: Condvar::new(),
            halos: halos
                .iter()
                .map(|u| {
                    let (x0, x1) = if u.below {
                        dims.cols(u.c0, u.c1)
                    } else {
                        (0, 0)
                    };
                    let (cx0, cx1) = dims.ccols(u.c0, u.c1).filter(|_| u.below).unwrap_or((0, 0));
                    let n4 = x1.div_ceil(4) - x0 / 4;
                    let m4 = if dims.inter { n4 } else { 0 };
                    let nc = if u.below { u.c1 - u.c0 } else { 0 };
                    Mutex::new(Halo {
                        x0,
                        cx0,
                        c0: u.c0,
                        luma: vec![0; x1 - x0],
                        cb: vec![0; cx1 - cx0],
                        cr: vec![0; cx1 - cx0],
                        depth: vec![0; n4],
                        flags: vec![0; m4],
                        mvf: vec![MvField::default(); m4],
                        slice_addr: vec![u32::MAX; nc],
                        ctb: vec![CtbFilter::default(); nc],
                        wpp: None,
                    })
                })
                .collect(),
            aborted: AtomicBool::new(false),
            next: AtomicUsize::new(0),
            worst: Mutex::new(None),
            run_fail: (0..runs.len())
                .map(|_| AtomicUsize::new(usize::MAX))
                .collect(),
            pool: Mutex::new(Vec::new()),
        }
    }

    /// Fills the pool with `n` bands big enough for any row of any unit,
    /// allocated by the calling thread: workers then only ever reuse them,
    /// rather than leaving freed bands behind in their own allocator arenas.
    pub fn prefill(&self, n: usize, units: &Units, d: Dims) -> Result<()> {
        let wide = units
            .list
            .iter()
            .map(|u| d.cols(u.c0, u.c1))
            .max_by_key(|&(a, b)| b - a)
            .unwrap_or((0, 0));
        let rows = (d.ctb + 8).min(d.height);
        let mut pool = lock(&self.pool);
        for _ in 0..n.min(POOL) {
            let mut b = Picture::empty();
            b.reshape(
                d.width,
                d.height,
                (d.sub_w, d.sub_h),
                (0, wide.1 - wide.0),
                (0, rows),
            )?;
            if d.inter {
                b.mvf.resize(b.cu_flags.len(), MvField::default());
            }
            pool.push(b);
        }
        Ok(())
    }

    /// Returns the band of a merged row to the pool.
    pub fn recycle(&self, done: Done) {
        if let Some(b) = done.band {
            let mut p = lock(&self.pool);
            if p.len() < POOL {
                p.push(b);
            }
        }
    }

    fn set_progress(&self, r: usize, done: usize) {
        let mut p = lock(&self.progress);
        if p[r] < done {
            p[r] = done;
        }
        drop(p);
        self.cv.notify_all();
    }

    /// Stops every worker at its next CTB and releases every waiting one.
    pub fn abort(&self) {
        self.aborted.store(true, Ordering::Relaxed);
        let _p = lock(&self.progress);
        self.cv.notify_all();
    }

    /// Whether some of the `n` units are not claimed yet.
    pub fn unclaimed(&self, n: usize) -> bool {
        self.next.load(Ordering::Relaxed) < n
    }

    /// True once a worker has given up (cancelled, or failed outright).
    pub fn aborted(&self) -> bool {
        self.aborted.load(Ordering::Relaxed)
    }

    /// Waits until row `r` has `need` CTBs done.
    fn wait(&self, r: usize, need: usize) -> core::result::Result<(), Halt> {
        let mut p = lock(&self.progress);
        loop {
            if self.aborted.load(Ordering::Relaxed) {
                return Err(Halt::Cancelled);
            }
            if p[r] >= need {
                return Ok(());
            }
            p = self.cv.wait(p).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Records that run `run` is lost from tile-scan address `ts` on.
    fn fail(&self, run: usize, ts: usize, e: Error) {
        let mut w = lock(&self.worst);
        if w.is_none_or(|(t, _)| ts < t) {
            *w = Some((ts, e));
        }
        drop(w);
        if let Some(f) = self.run_fail.get(run) {
            f.fetch_min(ts, Ordering::Relaxed);
        }
    }

    /// The first tile-scan address of run `run` known to be lost
    /// (`usize::MAX` for none). Final for the addresses of the run before
    /// any CTB row of it still being decoded: a failure is recorded before
    /// the row it is in is handed on.
    pub fn run_fail(&self, run: usize) -> usize {
        self.run_fail
            .get(run)
            .map_or(usize::MAX, |f| f.load(Ordering::Relaxed))
    }

    /// What failed.
    pub fn fails(&self) -> Fails {
        Fails {
            runs: (0..self.run_fail.len()).map(|r| self.run_fail(r)).collect(),
            worst: *lock(&self.worst),
        }
    }
}

/// Picture geometry the bands and their copies need.
#[derive(Clone, Copy, Debug)]
pub struct Dims {
    w_ctbs: usize,
    ctb: usize,
    width: usize,
    height: usize,
    /// Chroma plane width (zero for monochrome).
    cw: usize,
    sub_w: usize,
    sub_h: usize,
    /// The picture has motion: bands carry it.
    inter: bool,
}

impl Dims {
    /// The geometry of `pic`, whose CTBs are `1 << ctb_log2` samples wide.
    pub fn of(pic: &Picture, w_ctbs: usize, ctb_log2: usize, sub: (usize, usize)) -> Dims {
        Dims {
            w_ctbs,
            ctb: 1 << ctb_log2,
            width: pic.y.width,
            height: pic.y.height,
            cw: pic.cb.width,
            sub_w: sub.0,
            sub_h: sub.1,
            inter: !pic.mvf.is_empty(),
        }
    }

    /// Luma columns of CTB columns `a..b`.
    fn cols(&self, a: usize, b: usize) -> (usize, usize) {
        (
            (a * self.ctb).min(self.width),
            (b * self.ctb).min(self.width),
        )
    }

    /// Chroma columns of CTB columns `a..b`, if there is chroma.
    fn ccols(&self, a: usize, b: usize) -> Option<(usize, usize)> {
        if self.sub_w == 0 || self.cw == 0 {
            return None;
        }
        let (a, b) = self.cols(a, b);
        Some((
            a / self.sub_w,
            if b == self.width {
                self.cw
            } else {
                b / self.sub_w
            },
        ))
    }
}

/// The worker's view of one unit.
struct UnitEnv<'a> {
    sync: &'a Sync,
    cancel: Option<&'a AtomicBool>,
    dims: Dims,
    /// The unit, and whether it is a wavefront row.
    u: usize,
    rows: bool,
    /// The unit of the wavefront row above, and whether one waits on this.
    above: Option<usize>,
    below: bool,
    /// CTB columns `c0..c1` of the unit.
    c0: usize,
    c1: usize,
    /// The CTB row whose band is in the picture, once one is.
    row: Option<usize>,
    /// The next CTB row of the unit to hand on.
    shipped: usize,
    /// Why a band could not be allocated.
    err: Option<Error>,
    sink: &'a mut dyn FnMut(usize, Done),
    /// CTB columns of the row above already copied into the band.
    copied: usize,
    /// Wavefront states inside a tile.
    wpp: BTreeMap<usize, CtxState>,
}

impl UnitEnv<'_> {
    /// Puts a fresh band for CTB row `r` into `pic`, carrying over what it
    /// needs from the row before when that was the unit's last, and hands
    /// the band it replaces on.
    fn enter_row(&mut self, pic: &mut Picture, fi: &FilterInfo, r: usize) -> Result<()> {
        let d = self.dims;
        let oy = (r * d.ctb).saturating_sub(8);
        let mut band = lock(&self.sync.pool).pop().unwrap_or_else(Picture::empty);
        band.reshape(
            d.width,
            d.height,
            (d.sub_w, d.sub_h),
            d.cols(self.c0, self.c1),
            (oy, (r + 1) * d.ctb - oy),
        )?;
        if d.inter {
            band.mvf.resize(band.cu_flags.len(), MvField::default());
        }
        if !self.rows && self.row.is_some_and(|p| p + 1 == r) {
            carry(pic, &mut band, d, (self.c0, self.c1), r);
        }
        core::mem::swap(pic, &mut band);
        if let Some(p) = self.row.replace(r) {
            self.ship(p, band, fi);
        }
        Ok(())
    }

    /// Hands on the unit's rows before `row` that were never entered (all
    /// lost), then `row` in `band`.
    fn ship(&mut self, row: usize, band: Picture, fi: &FilterInfo) {
        self.skip_to(row);
        let w = self.dims.w_ctbs;
        let (a, b) = (row * w + self.c0, row * w + self.c1);
        (self.sink)(
            self.u,
            Done {
                row,
                band: Some(band),
                ctb: fi.ctb[a..b].to_vec(),
                slice_addr: fi.slice_addr[a..b].to_vec(),
            },
        );
        self.shipped = row + 1;
    }

    /// Hands on the unit's rows before `r` not handed on yet as never
    /// entered.
    fn skip_to(&mut self, r: usize) {
        while self.shipped < r {
            (self.sink)(
                self.u,
                Done {
                    row: self.shipped,
                    band: None,
                    ctb: Vec::new(),
                    slice_addr: Vec::new(),
                },
            );
            self.shipped += 1;
        }
    }
}

/// Copies what CTB row `r` needs from the row above, columns `cols`, from
/// `old` (whose band holds that row) into `new`.
fn carry(old: &Picture, new: &mut Picture, d: Dims, cols: (usize, usize), r: usize) {
    let y = r * d.ctb - 1;
    let (a, b) = d.cols(cols.0, cols.1);
    let (s, t) = (old.y.offset(a, y), new.y.offset(a, y));
    new.y.data[t..t + b - a].copy_from_slice(&old.y.data[s..s + b - a]);
    if let Some((ca, cb_)) = d.ccols(cols.0, cols.1) {
        let cy = r * d.ctb / d.sub_h - 1;
        let (s, t) = (old.cb.offset(ca, cy), new.cb.offset(ca, cy));
        let n = cb_ - ca;
        new.cb.data[t..t + n].copy_from_slice(&old.cb.data[s..s + n]);
        new.cr.data[t..t + n].copy_from_slice(&old.cr.data[s..s + n]);
    }
    let (s, t) = (old.range4(a, b, y), new.range4(a, b, y));
    new.ct_depth[t.clone()].copy_from_slice(&old.ct_depth[s.clone()]);
    if d.inter {
        new.cu_flags[t.clone()].copy_from_slice(&old.cu_flags[s.clone()]);
        new.mvf[t].copy_from_slice(&old.mvf[s]);
    }
}

impl Env for UnitEnv<'_> {
    fn before_ctb(
        &mut self,
        pic: &mut Picture,
        fi: &mut FilterInfo,
        rs: usize,
    ) -> core::result::Result<(), Halt> {
        if self.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            self.sync.abort();
            return Err(Halt::Cancelled);
        }
        if self.sync.aborted() {
            return Err(Halt::Cancelled);
        }
        let r = rs / self.dims.w_ctbs;
        if self.row != Some(r)
            && let Err(e) = self.enter_row(pic, fi, r)
        {
            self.err = Some(e);
            self.sync.abort();
            return Err(Halt::Cancelled);
        }
        let Some(above) = self.above else {
            return Ok(());
        };
        let need = (rs % self.dims.w_ctbs + 2).min(self.c1);
        if self.copied >= need {
            return Ok(());
        }
        self.sync.wait(above, need)?;
        let halo = lock(&self.sync.halos[above]);
        let d = self.dims;
        let y = r * d.ctb - 1;
        let (a, b) = d.cols(self.copied, need);
        let o = pic.y.offset(a, y);
        pic.y.data[o..o + b - a].copy_from_slice(&halo.luma[a - halo.x0..b - halo.x0]);
        if let Some((ca, cb_)) = d.ccols(self.copied, need) {
            let cy = r * d.ctb / d.sub_h - 1;
            let o = pic.cb.offset(ca, cy);
            let h = ca - halo.cx0..cb_ - halo.cx0;
            pic.cb.data[o..o + cb_ - ca].copy_from_slice(&halo.cb[h.clone()]);
            pic.cr.data[o..o + cb_ - ca].copy_from_slice(&halo.cr[h]);
        }
        let t = pic.range4(a, b, y);
        let h4 = a / 4 - halo.x0 / 4..a / 4 - halo.x0 / 4 + t.len();
        pic.ct_depth[t.clone()].copy_from_slice(&halo.depth[h4.clone()]);
        if d.inter {
            pic.cu_flags[t.clone()].copy_from_slice(&halo.flags[h4.clone()]);
            pic.mvf[t].copy_from_slice(&halo.mvf[h4]);
        }
        let base = (r - 1) * d.w_ctbs;
        for x in self.copied..need {
            fi.slice_addr[base + x] = halo.slice_addr[x - halo.c0];
            fi.ctb[base + x] = halo.ctb[x - halo.c0];
        }
        self.copied = need;
        Ok(())
    }

    fn wpp_load(&mut self, rs: usize) -> Option<CtxState> {
        if !self.rows {
            return self.wpp.get(&rs).copied();
        }
        // Only ever asked for CTB 1 of the row above in the tile, which
        // `before_ctb` has already waited for.
        self.above.and_then(|a| lock(&self.sync.halos[a]).wpp)
    }

    fn wpp_store(&mut self, rs: usize, st: CtxState) {
        if self.rows {
            lock(&self.sync.halos[self.u]).wpp = Some(st);
        } else {
            self.wpp.insert(rs, st);
        }
    }

    fn after_ctb(&mut self, pic: &Picture, fi: &FilterInfo, rs: usize) {
        if !self.rows {
            return;
        }
        let d = self.dims;
        let x = rs % d.w_ctbs;
        let r = rs / d.w_ctbs;
        if self.below {
            let mut halo = lock(&self.sync.halos[self.u]);
            let y = (r + 1) * d.ctb - 1;
            let (a, b) = d.cols(x, x + 1);
            let o = pic.y.offset(a, y);
            let x0 = halo.x0;
            halo.luma[a - x0..b - x0].copy_from_slice(&pic.y.data[o..o + b - a]);
            if let Some((ca, cb_)) = d.ccols(x, x + 1) {
                let cy = (r + 1) * d.ctb / d.sub_h - 1;
                let o = pic.cb.offset(ca, cy);
                let h = ca - halo.cx0..cb_ - halo.cx0;
                halo.cb[h.clone()].copy_from_slice(&pic.cb.data[o..o + cb_ - ca]);
                halo.cr[h].copy_from_slice(&pic.cr.data[o..o + cb_ - ca]);
            }
            let s = pic.range4(a, b, y);
            let h4 = a / 4 - x0 / 4..a / 4 - x0 / 4 + s.len();
            halo.depth[h4.clone()].copy_from_slice(&pic.ct_depth[s.clone()]);
            if d.inter {
                halo.flags[h4.clone()].copy_from_slice(&pic.cu_flags[s.clone()]);
                halo.mvf[h4].copy_from_slice(&pic.mvf[s]);
            }
            let c0 = halo.c0;
            halo.slice_addr[x - c0] = fi.slice_addr[rs];
            halo.ctb[x - c0] = fi.ctb[rs];
        }
        self.sync.set_progress(self.u, x + 1);
    }
}

/// One CTB row of a unit, decoded: its band (none if no CTB of it was
/// decoded: all of them are lost) and the filter parameters and slice
/// addresses of the unit's CTBs in it.
pub struct Done {
    pub row: usize,
    band: Option<Picture>,
    ctb: Vec<CtbFilter>,
    slice_addr: Vec<u32>,
}

/// Copies the finished row of unit `u` into `pic` and `fi`.
pub fn merge(pic: &mut Picture, fi: &mut FilterInfo, done: &Done, u: &Unit, d: Dims) {
    let Some(band) = &done.band else {
        return;
    };
    let r = done.row;
    let y0 = (r * d.ctb).min(d.height);
    let y1 = ((r + 1) * d.ctb).min(d.height);
    let (x0, x1) = d.cols(u.c0, u.c1);
    for y in y0..y1 {
        let (s, t) = (band.y.offset(x0, y), pic.y.offset(x0, y));
        pic.y.data[t..t + x1 - x0].copy_from_slice(&band.y.data[s..s + x1 - x0]);
    }
    if let (Some((cx0, cx1)), Some(c0), Some(c1)) = (
        d.ccols(u.c0, u.c1),
        y0.checked_div(d.sub_h),
        y1.checked_div(d.sub_h),
    ) {
        let c1 = if y1 == d.height { pic.cb.height } else { c1 };
        for y in c0..c1 {
            let (s, t) = (band.cb.offset(cx0, y), pic.cb.offset(cx0, y));
            let n = cx1 - cx0;
            pic.cb.data[t..t + n].copy_from_slice(&band.cb.data[s..s + n]);
            pic.cr.data[t..t + n].copy_from_slice(&band.cr.data[s..s + n]);
        }
    }
    for y4 in y0 / 4..y1.div_ceil(4) {
        let (s, t) = (band.range4(x0, x1, y4 * 4), pic.range4(x0, x1, y4 * 4));
        pic.intra_mode[t.clone()].copy_from_slice(&band.intra_mode[s.clone()]);
        pic.qp_y[t.clone()].copy_from_slice(&band.qp_y[s.clone()]);
        pic.cu_flags[t.clone()].copy_from_slice(&band.cu_flags[s.clone()]);
        pic.ct_depth[t.clone()].copy_from_slice(&band.ct_depth[s.clone()]);
        if d.inter {
            pic.mvf[t].copy_from_slice(&band.mvf[s]);
        }
    }
    let (a8, b8) = (x0 / 8, x1.div_ceil(8));
    for y8 in y0 / 8..y1.div_ceil(8) {
        let s = (y8 - band.oy8) * band.min8_w + a8 - band.ox8;
        let t = (y8 - pic.oy8) * pic.min8_w + a8 - pic.ox8;
        pic.edges[t..t + b8 - a8].copy_from_slice(&band.edges[s..s + b8 - a8]);
    }
    let base = r * d.w_ctbs + u.c0;
    let cw = u.c1 - u.c0;
    fi.ctb[base..base + cw].copy_from_slice(&done.ctb);
    fi.slice_addr[base..base + cw].copy_from_slice(&done.slice_addr);
}

/// The worker loop: claims units until none are left (or, before claiming
/// one, `leave` says to stop), handing each CTB row of each to `sink` as it
/// is finished, every row of the unit exactly once and in order. `fi` is
/// the worker's own copy of the picture's initial filter parameters;
/// `cancel` is polled before every CTB.
#[allow(clippy::too_many_arguments)]
pub fn worker(
    sh: &Shared<'_>,
    segs: &[Segment<'_>],
    units: &Units,
    runs: &Runs,
    sync: &Sync,
    mut fi: FilterInfo,
    dims: Dims,
    cancel: Option<&AtomicBool>,
    leave: &dyn Fn() -> bool,
    sink: &mut dyn FnMut(usize, Done),
) -> Result<()> {
    let w = dims.w_ctbs;
    let mut guard = AbortOnDrop(sync, true);
    let empty = || {
        let mut p = Picture::empty();
        p.reshape(
            dims.width,
            dims.height,
            (dims.sub_w, dims.sub_h),
            (0, dims.width),
            (0, 0),
        )
        .map(|()| p)
    };
    loop {
        if sync.aborted() {
            return Err(Error::Cancelled);
        }
        if leave() {
            guard.1 = false;
            return Ok(());
        }
        let u = sync.next.fetch_add(1, Ordering::Relaxed);
        let Some(unit) = units.list.get(u) else {
            guard.1 = false;
            return Ok(());
        };
        // Stands in until the first CTB of the unit swaps in its row's band.
        let mut band = empty()?;
        let mut env = UnitEnv {
            sync,
            cancel,
            dims,
            u,
            rows: units.rows,
            above: unit.above,
            below: unit.below,
            c0: unit.c0,
            c1: unit.c1,
            row: None,
            shipped: unit.r0,
            err: None,
            sink: &mut *sink,
            copied: unit.c0,
            wpp: BTreeMap::new(),
        };
        let mut ds: Option<(usize, DsState)> = None;
        for p in &unit.pieces {
            let seg = &segs[p.seg];
            let carry = ds
                .take()
                .filter(|&(i, _)| seg.h.dependent && i + 1 == p.seg)
                .map(|(_, d)| d);
            if sync.run_fail(runs.of(p.a)) > p.a {
                match run(
                    sh,
                    &mut band,
                    &mut fi,
                    &mut env,
                    seg,
                    p.a,
                    p.sub,
                    p.b,
                    carry.as_ref(),
                ) {
                    Outcome::Ended { ts, ds: d } => {
                        if ts + 1 < seg.ts_end {
                            sync.fail(runs.of(ts), ts + 1, super::ENDED_EARLY);
                        } else {
                            ds = Some((p.seg, d));
                        }
                    }
                    Outcome::Paused => {}
                    Outcome::Failed(ts, e) => sync.fail(runs.of(ts), ts, e),
                    Outcome::Cancelled => return Err(env.err.take().unwrap_or(Error::Cancelled)),
                }
            }
            if units.rows {
                // Releases the row below past CTBs given up on.
                sync.set_progress(u, sh.geo.ts_to_rs[p.b - 1] as usize % w + 1);
            }
        }
        if units.rows {
            // Rows with no piece at all still have to release the row below.
            sync.set_progress(u, w);
        }
        if let Some(r) = env.row.take() {
            let last = core::mem::replace(&mut band, empty()?);
            env.ship(r, last, &fi);
        }
        env.skip_to(unit.r1);
    }
}

/// Releases every waiting worker if a worker leaves early, whatever the
/// reason, so that no row waits forever on a row nobody will finish.
struct AbortOnDrop<'a>(&'a Sync, bool);

impl Drop for AbortOnDrop<'_> {
    fn drop(&mut self) {
        if self.1 {
            self.0.abort();
        }
    }
}

/// Decodes all segments of a still picture in `units` on `threads` scoped
/// workers. Returns what failed.
#[allow(clippy::too_many_arguments)]
pub fn decode(
    sh: &Shared<'_>,
    segs: &[Segment<'_>],
    units: &Units,
    runs: &Runs,
    pic: &mut Picture,
    fi: &mut FilterInfo,
    threads: usize,
    cancel: Option<&AtomicBool>,
) -> Result<Fails> {
    let dims = Dims::of(
        pic,
        sh.geo.w_ctbs,
        sh.geo.ctb_log2,
        (sh.sps.sub_w, sh.sps.sub_h),
    );
    let sync = Sync::new(units, dims, runs);
    let threads = threads.clamp(1, units.list.len().max(1));
    sync.prefill(2 * threads + 2, units, dims)?;
    let fi0 = fi.clone();
    let out = Mutex::new((pic, fi));
    let results: Vec<Result<()>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|wi| {
                specular_thread::spawn_scoped(s, format!("dec-hevc-wf-{wi}"), || {
                    let mut sink = |u: usize, done: Done| {
                        let mut g = lock(&out);
                        let (pic, fi) = &mut *g;
                        merge(pic, fi, &done, &units.list[u], dims);
                        drop(g);
                        sync.recycle(done);
                    };
                    worker(
                        sh,
                        segs,
                        units,
                        runs,
                        &sync,
                        fi0.clone(),
                        dims,
                        cancel,
                        &|| false,
                        &mut sink,
                    )
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or(Err(Error::InvalidData("decoder worker panicked")))
            })
            .collect()
    });
    // A real failure (allocation, a panic) outranks the cancellation the
    // other workers saw as a result of it.
    if let Some(e) = results
        .iter()
        .find_map(|r| r.err().filter(|&e| e != Error::Cancelled))
    {
        return Err(e);
    }
    if sync.aborted() {
        return Err(Error::Cancelled);
    }
    Ok(sync.fails())
}
