//! One picture of a sequence decoded in parallel on the sequence's pool.
//!
//! The picture's job (the lead) splits it into the units of [`wpp::units`]
//! — wavefront rows, or tiles — and asks the pool for helpers, as many as
//! the units allow: the picture is open to the pool's idle workers, which
//! join it whenever they have nothing better to do, also once it is well
//! under way. Helpers are ordinary pool tasks: each plans the picture
//! again from the job (planning is deterministic and cheap), then claims
//! units exactly as the lead does, leaving between units when the pool has
//! a picture waiting for a thread. Units are claimed in order and only by
//! threads that are running, so a wavefront row only ever waits on a row
//! being worked on; a helper that starts once every unit is claimed (or
//! after the picture is done) leaves at once.
//!
//! Each finished CTB row of a unit is queued; whichever thread then gets
//! the picture lock merges every queued row into the picture and settles
//! the row pipeline of [`super::decode_piped`] over the CTB rows every unit
//! has finished: loss is marked there (a run's failures before a CTB are
//! all known once the CTB's row is handed on, as runs never leave their
//! tile), then rows are concealed, deblocked, SAO'd and published in
//! order, exactly as on the serial path. So with several tile columns a
//! row is filtered as soon as every tile across it has got past it, not
//! once whole tiles are done. A thread that finds the lock taken leaves the
//! work to its holder, which looks at the queue again after letting go.

use super::slice_data::{Runs, Segment, Shared};
use super::wpp::{self, Dims, Done, Units, lock};
use super::{Plan, Post, PostCx};
use crate::error::{Error, Result};
use crate::filter::FilterInfo;
use crate::picture::Picture;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::AtomicBool;
use std::sync::{Condvar, Mutex, TryLockError};

/// Opens a picture to the pool's helpers.
pub(crate) type Spawn<'a> = dyn Fn(Arc<Par>) + 'a;

/// The state of one picture shared by its lead and helpers.
pub(crate) struct Par {
    units: Units,
    runs: Runs,
    sync: wpp::Sync,
    dims: Dims,
    /// Finished unit rows not merged yet.
    queue: Mutex<Vec<(usize, Done)>>,
    /// The picture; taken by the lead once every worker has left.
    state: Mutex<Option<State>>,
    ctl: Mutex<Ctl>,
    cv: Condvar,
}

/// Who is working on the picture.
struct Ctl {
    /// Helpers inside.
    active: usize,
    /// The lead is done with the picture: helpers no longer join.
    closed: bool,
    /// A helper's failure other than cancellation.
    err: Option<Error>,
}

/// The picture being put together.
struct State {
    pic: Picture,
    fi: FilterInfo,
    post: Post,
    /// Per CTB row, the CTBs merged (or handed on as lost).
    row_fin: Vec<usize>,
}

/// What the lead brings.
pub(super) struct Lead<'a, 's, 'p> {
    pub units: Units,
    pub runs: Runs,
    /// The thread budget.
    pub threads: usize,
    pub sh: &'a Shared<'s>,
    pub segs: &'a [Segment<'p>],
    pub pic: Picture,
    pub fi: FilterInfo,
    pub post: Post,
    pub cx: &'a PostCx<'a>,
    pub cancel: Option<&'a AtomicBool>,
    pub spawn: &'a Spawn<'a>,
}

/// Decodes the picture as its lead: starts helpers, works with them, waits
/// for them to leave, then finishes the row pipeline. Returns the CTUs
/// concealed.
pub(super) fn lead(l: Lead<'_, '_, '_>) -> Result<usize> {
    let Lead {
        units,
        runs,
        threads,
        sh,
        segs,
        mut pic,
        mut fi,
        mut post,
        cx,
        cancel,
        spawn,
    } = l;
    let geo = sh.geo;
    let dims = Dims::of(&pic, geo.w_ctbs, geo.ctb_log2, (sh.sps.sub_w, sh.sps.sub_h));
    let first = segs.first().map_or(geo.size_ctbs, |s| s.ts0);
    post.lose(geo, 0..first, None);
    post.settle(cx, &mut pic, &mut fi, first);
    let fi0 = fi.clone();
    let sync = wpp::Sync::new(&units, dims, &runs);
    sync.prefill(2 * threads.min(units.list.len()) + 2, &units, dims)?;
    let par = Arc::new(Par {
        sync,
        units,
        runs,
        dims,
        queue: Mutex::new(Vec::new()),
        state: Mutex::new(Some(State {
            pic,
            fi,
            post,
            row_fin: vec![0; geo.h_ctbs],
        })),
        ctl: Mutex::new(Ctl {
            active: 0,
            closed: false,
            err: None,
        }),
        cv: Condvar::new(),
    });
    spawn(Arc::clone(&par));
    let mine = work(&par, sh, segs, fi0, cancel, cx, &|| false);
    let theirs = {
        let mut c = lock(&par.ctl);
        while c.active > 0 {
            c = par.cv.wait(c).unwrap_or_else(|e| e.into_inner());
        }
        c.closed = true;
        c.err.take()
    };
    let Some(mut st) = lock(&par.state).take() else {
        return Err(Error::InvalidData("row pipeline lost"));
    };
    // A real failure (allocation, a panic) outranks the cancellation the
    // other workers saw as a result of it.
    if let Err(e) = mine
        && e != Error::Cancelled
    {
        return Err(e);
    }
    if let Some(e) = theirs {
        return Err(e);
    }
    if par.sync.aborted() || mine.is_err() {
        return Err(Error::Cancelled);
    }
    merge_queued(&par, &mut st);
    advance(&par, &mut st, segs, cx);
    if st.post.rows_done < geo.h_ctbs {
        return Err(Error::InvalidData("unit not decoded"));
    }
    st.post.settle(cx, &mut st.pic, &mut st.fi, geo.size_ctbs);
    Ok(st.post.damaged)
}

impl Par {
    /// Whether the picture has units nobody has claimed yet.
    pub(crate) fn wants(&self) -> bool {
        !self.sync.aborted()
            && self.sync.unclaimed(self.units.list.len())
            && !lock(&self.ctl).closed
    }
}

/// Works on the picture as a helper, if it is still open, until no unit
/// is left or `leave` says the thread is wanted elsewhere. `plan` is the
/// lead's plan of the picture, made from `sets` and `parsed`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn help(
    par: &Par,
    leave: &dyn Fn() -> bool,
    plan: &Plan,
    sets: &super::ParameterSets,
    parsed: &[(crate::nal::NalHeader, crate::nal::Rbsp)],
    ictx: &super::inter::InterCtx,
    cancel: Option<&AtomicBool>,
    src: Option<&super::inter::RefPic>,
    out: &super::inter::RefPic,
) {
    {
        let mut c = lock(&par.ctl);
        if c.closed || par.sync.aborted() {
            return;
        }
        c.active += 1;
    }
    let mut exit = Leave(par, None);
    let res = plan.sets(sets).and_then(|(sps, pps)| {
        let shared = Shared {
            sps,
            pps,
            geo: &plan.geo,
            sf: plan.factors.as_deref(),
            inter: Some(ictx),
        };
        let cx = PostCx {
            geo: &plan.geo,
            src,
            out,
            ictx,
        };
        let segs = plan.segs(parsed);
        work(par, &shared, &segs, plan.fi.clone(), cancel, &cx, leave)
    });
    if let Err(e) = res
        && e != Error::Cancelled
    {
        par.sync.abort();
        exit.1 = Some(e);
    }
}

/// Leaves the picture, recording a failure (a panic counts as one).
struct Leave<'a>(&'a Par, Option<Error>);

impl Drop for Leave<'_> {
    fn drop(&mut self) {
        let mut err = self.1.take();
        if std::thread::panicking() {
            self.0.sync.abort();
            err = Some(Error::InvalidData("decoder worker panicked"));
        }
        let mut c = lock(&self.0.ctl);
        if c.err.is_none() {
            c.err = err;
        }
        c.active -= 1;
        drop(c);
        self.0.cv.notify_all();
    }
}

/// Claims and decodes units until none are left, or `leave` says so.
fn work(
    par: &Par,
    sh: &Shared<'_>,
    segs: &[Segment<'_>],
    fi: FilterInfo,
    cancel: Option<&AtomicBool>,
    cx: &PostCx<'_>,
    leave: &dyn Fn() -> bool,
) -> Result<()> {
    let mut sink = |u: usize, done: Done| {
        lock(&par.queue).push((u, done));
        drain(par, segs, cx);
    };
    wpp::worker(
        sh, segs, &par.units, &par.runs, &par.sync, fi, par.dims, cancel, leave, &mut sink,
    )
}

/// Merges the queued units and settles what they allow, unless another
/// thread holds the picture (it will see the queue when it lets go).
fn drain(par: &Par, segs: &[Segment<'_>], cx: &PostCx<'_>) {
    loop {
        {
            let mut g = match par.state.try_lock() {
                Ok(g) => g,
                Err(TryLockError::Poisoned(e)) => e.into_inner(),
                Err(TryLockError::WouldBlock) => return,
            };
            let Some(st) = g.as_mut() else {
                return;
            };
            merge_queued(par, st);
            advance(par, st, segs, cx);
        }
        if lock(&par.queue).is_empty() {
            return;
        }
    }
}

fn merge_queued(par: &Par, st: &mut State) {
    loop {
        let batch = core::mem::take(&mut *lock(&par.queue));
        if batch.is_empty() {
            return;
        }
        for (u, done) in batch {
            let unit = &par.units.list[u];
            wpp::merge(&mut st.pic, &mut st.fi, &done, unit, par.dims);
            st.row_fin[done.row] += unit.c1 - unit.c0;
            par.sync.recycle(done);
        }
    }
}

/// Marks the loss in, and settles, the CTB rows every unit has finished.
fn advance(par: &Par, st: &mut State, segs: &[Segment<'_>], cx: &PostCx<'_>) {
    let geo = cx.geo;
    let w = geo.w_ctbs;
    let mut rows = st.post.rows_done;
    while rows < geo.h_ctbs && st.row_fin[rows] >= w {
        for rs in rows * w..(rows + 1) * w {
            let ts = geo.rs_to_ts[rs] as usize;
            let i = segs.partition_point(|s| s.ts_end <= ts);
            if let Some(s) = segs.get(i)
                && s.ts0 <= ts
                && par.sync.run_fail(par.runs.of(ts)) <= ts
            {
                st.post
                    .lose(geo, ts..ts + 1, Some((s.h.slice_address, s.h.qp)));
            }
        }
        rows += 1;
    }
    st.post.settle_rows(cx, &mut st.pic, &mut st.fi, rows);
}
