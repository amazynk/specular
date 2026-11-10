//! Decoding a coded video sequence: picture order counts (8.3.1), the
//! reference picture set (8.3.2), missing reference generation (8.3.3) and
//! the output order of the decoded picture buffer (C.5.2).
//!
//! # Frame threading
//!
//! With more than one thread, each picture is decoded whole on a worker of
//! a small pool while the caller goes on placing the next ones. A picture
//! publishes its CTB rows as they are loop-filtered for good
//! ([`RefPic`]); a later picture's motion compensation waits only for the
//! rows its vectors (and filter taps) reach, so consecutive pictures decode
//! overlapped, a few CTB rows apart. Workers take pictures in decoding
//! order and a picture only ever waits on earlier ones, and every picture
//! is finished whatever becomes of its decode (damage, cancellation, a
//! panic), so the pipeline cannot deadlock. Output waits for a picture to
//! finish; the pictures and the order are those of a single-threaded
//! decode.
//!
//! A picture with wavefront rows or several tiles is also decoded in
//! parallel inside: its job opens it to the same pool's idle workers, which
//! join it as helpers; see [`super::pipe`]. Helpers only take threads no
//! picture is waiting for, and leave between units as soon as one is:
//! pictures in flight are the cheaper parallelism (each filters its own
//! rows), so helpers fill what frame threading leaves idle (intra-only
//! stretches, a reference chain one picture deep, the first pictures after
//! a flush, a worker done with its picture while the next waits on it).
//!
//! # Cancelled pictures
//!
//! A cancelled picture is finished with what it had published, the rest
//! grey, and still joins the buffer, never output, so that the pictures
//! after it see the buffer a full decode would have. A picture is tainted
//! when one of its references, or the picture its lost CTBs were copied
//! from, was cancelled or is tainted itself: its samples are not those of
//! the stream. Tainted pictures are dropped like cancelled ones (their
//! tags come back from [`Seq::next_dropped`]), never output concealed, so
//! a caller sees the same thing whatever the thread count and wherever
//! the cancel fell: the pictures after a cancel are dropped until one that
//! depends on nothing cancelled, at the latest the next IRAP picture.

use super::inter::refpic::Status;
use super::inter::{InterCtx, RefEntry, RefPic};
use super::pipe::{self, Spawn};
use super::{OutGeom, ParameterSets, Plan, crop, decode_piped, plan};
use crate::Decoded;
use crate::error::{Error, Result};
use crate::nal::{NalHeader, Rbsp};
use crate::ps::Sps;
use crate::slice::{SliceHeader, parse as parse_slice_header};
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Condvar, Mutex, Weak};
use std::thread::JoinHandle;

/// How often a caller waiting for a worker looks at its cancel flag.
const CANCEL_POLL: Duration = Duration::from_millis(10);

/// NAL unit types the sequence logic distinguishes.
mod nut {
    pub const RADL_N: u8 = 6;
    pub const RASL_N: u8 = 8;
    pub const RASL_R: u8 = 9;
    pub const BLA_N_LP: u8 = 18;
    pub const IDR_W_RADL: u8 = 19;
    pub const IDR_N_LP: u8 = 20;
    pub const CRA: u8 = 21;
    pub const EOS: u8 = 36;
    pub const EOB: u8 = 37;
}

/// More pictures than any conforming stream keeps: a bound on memory for
/// the ones that do not conform.
const MAX_DPB: usize = 17;

/// How a picture in the buffer is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    Unused,
    Short,
    Long,
}

/// One picture of the decoded picture buffer.
#[derive(Clone)]
struct DpbPic {
    pic: Arc<RefPic>,
    mark: Mark,
    /// Needed for output.
    output: bool,
    /// `PicLatencyCount`.
    latency: u32,
    geom: OutGeom,
    /// The caller's tag of the access unit.
    tag: i64,
    /// The memory budget's share of the picture, held while it is in the buffer or a seek
    /// snapshot (shared, like `pic`).
    res: Arc<specular_core::Reservation>,
}

/// A picture bumped for output, handed back once it is decoded.
#[derive(Clone)]
struct Pending {
    pic: Arc<RefPic>,
    geom: OutGeom,
    tag: i64,
    /// The picture's reservation (its [`DpbPic::res`]): it may leave the buffer before
    /// it is handed back.
    _res: Arc<specular_core::Reservation>,
}

/// Everything decoding one picture needs, owned so a worker can run it.
struct Job {
    sets: Arc<ParameterSets>,
    parsed: Vec<(NalHeader, Rbsp)>,
    ctx: InterCtx,
    /// What lost CTBs are copied from: the previous picture.
    src: Option<Arc<RefPic>>,
    out: Arc<RefPic>,
    /// CTUs in the picture.
    total: usize,
    /// The plan of the picture, shared by every thread decoding it; none
    /// when nothing of the picture decodes: it is the previous picture
    /// (when of the same size) or grey.
    plan: Option<Plan>,
}

/// Decodes `job` into `job.out`, and finishes it whatever happens. `par`
/// is the thread budget and the way to start helpers on the pool.
fn run(job: &Job, cancel: Option<&AtomicBool>, par: Option<(usize, &Spawn<'_>)>) {
    let res = catch_unwind(AssertUnwindSafe(|| -> Result<usize> {
        if let Some(p) = &job.plan {
            let (sets, parsed) = (&job.sets, &job.parsed);
            let (src, out) = (job.src.as_deref(), &job.out);
            match decode_piped(p, sets, parsed, &job.ctx, cancel, src, out, par) {
                Err(e @ Error::Cancelled) => return Err(e),
                Err(_) if job.out.progress() == 0 => {}
                r => return r,
            }
        }
        stand_in(&job.out, job.src.as_deref());
        Ok(job.total)
    }));
    let mut status = match res {
        Ok(Ok(damaged)) => Status {
            damaged,
            total: job.total,
            ..Status::default()
        },
        Ok(Err(Error::Cancelled)) => cancelled(job.total),
        // A bug: what was published stays, the rest is grey.
        _ => Status {
            damaged: job.total,
            total: job.total,
            ..Status::default()
        },
    };
    if !status.cancelled {
        // Lost CTBs (a stand-in being all lost) come from `src`.
        let src = job.src.as_deref().filter(|_| status.damaged > 0);
        status.tainted = job.ctx.refs.iter().map(|e| &*e.pic).chain(src).any(spoilt);
    }
    job.out.finish(status);
}

/// The status of a picture whose decode was cancelled.
fn cancelled(total: usize) -> Status {
    Status {
        damaged: total,
        total,
        cancelled: true,
        tainted: false,
    }
}

/// True when `p`, once finished, was cancelled or is tainted.
fn spoilt(p: &RefPic) -> bool {
    p.wait_done();
    p.status().is_some_and(|s| s.cancelled || s.tainted)
}

/// Publishes `out` as a copy of `src` when they are the same size; leaves
/// it grey otherwise.
fn stand_in(out: &RefPic, src: Option<&RefPic>) {
    let Some(src) = src.filter(|s| s.all_dims() == out.all_dims()) else {
        return;
    };
    for r in out.progress()..out.n_rows() {
        if out.same_layout(src) {
            src.wait_rows(r + 1);
            out.share_row(r, src);
            continue;
        }
        let plane = |c: usize| -> Box<[u16]> {
            let (a, b) = out.row_lines(c, r);
            let mut v = Vec::with_capacity((b - a) * out.dims(c).0);
            for y in a..b {
                src.wait_line(c, y);
                v.extend_from_slice(src.line(c, y));
            }
            v.into_boxed_slice()
        };
        let row = super::inter::refpic::Row {
            planes: [plane(0), plane(1), plane(2)],
        };
        out.publish(r, Some(row), Vec::new());
    }
}

/// Work for the pool.
type Task = Box<dyn FnOnce() + Send>;

/// Helps decode an open picture until `leave` says the thread is wanted
/// elsewhere.
type Help = Arc<dyn Fn(&dyn Fn() -> bool) + Send + Sync>;

/// A picture being decoded in parallel that idle workers may join.
struct Open {
    par: Arc<pipe::Par>,
    help: Help,
}

/// The pool's queues: pictures in decoding order go first, then open
/// pictures that have units left, oldest first.
#[derive(Default)]
struct Queue {
    pictures: VecDeque<Task>,
    open: Vec<Open>,
    /// Worker threads.
    workers: usize,
    /// Workers running a task.
    busy: usize,
    closed: bool,
}

/// What the pool's workers and the pictures they decode share.
#[derive(Default)]
struct Tasks {
    q: Mutex<Queue>,
    cv: Condvar,
}

impl Tasks {
    fn lock(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.q.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Queues a picture.
    fn push(&self, t: Task) {
        self.lock().pictures.push_back(t);
        self.cv.notify_one();
    }

    /// Opens a picture to idle workers.
    fn open(&self, o: Open) {
        self.lock().open.push(o);
        self.cv.notify_all();
    }

    /// Closes the picture of `par`.
    fn close(&self, par: &Arc<pipe::Par>) {
        self.lock().open.retain(|o| !Arc::ptr_eq(&o.par, par));
    }

    /// Whether a picture is waiting for a worker.
    fn picture_waiting(&self) -> bool {
        !self.lock().pictures.is_empty()
    }

    /// The next task, or none once the pool is closed and empty.
    fn pop(self: &Arc<Self>) -> Option<Task> {
        let mut q = self.lock();
        loop {
            if let Some(t) = q.pictures.pop_front() {
                q.busy += 1;
                return Some(t);
            }
            q.open.retain(|o| o.par.wants());
            if let Some(o) = q.open.first() {
                let help = Arc::clone(&o.help);
                let tasks = Arc::clone(self);
                q.busy += 1;
                return Some(Box::new(move || help(&|| tasks.picture_waiting())));
            }
            if q.closed {
                return None;
            }
            q = self.cv.wait(q).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// A task taken by [`Tasks::pop`] has returned.
    fn done(&self) {
        let mut q = self.lock();
        q.busy = q.busy.saturating_sub(1);
    }

    /// Workers with nothing to do, not counting those the queued pictures
    /// will take.
    fn spare(&self) -> usize {
        let q = self.lock();
        q.workers.saturating_sub(q.busy + q.pictures.len())
    }
}

/// Worker threads taking pictures in decoding order, and helpers for them.
struct Pool {
    tasks: Arc<Tasks>,
    handles: Vec<JoinHandle<()>>,
}

impl Pool {
    /// `n` workers, or none when the system will not give threads.
    fn new(n: usize) -> Pool {
        let tasks = Arc::new(Tasks::default());
        let handles = (0..n)
            .map(|i| {
                let tasks = Arc::clone(&tasks);
                specular_thread::builder(format!("dec-hevc-{i}"))
                    .spawn(move || {
                        while let Some(t) = tasks.pop() {
                            // Every task finishes its picture itself; a
                            // panic must not take the worker with it.
                            let _ = catch_unwind(AssertUnwindSafe(t));
                            tasks.done();
                        }
                    })
                    .ok()
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        tasks.q.lock().unwrap_or_else(|e| e.into_inner()).workers = handles.len();
        Pool { tasks, handles }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.tasks
            .q
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .closed = true;
        self.tasks.cv.notify_all();
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

/// The pool task decoding `job`, which it opens to idle workers of the
/// same pool when it splits into units; the pool is held weakly (a queued
/// task must not keep the pool it is queued on alive). A picture is split
/// only when a worker is free as it starts: otherwise it is decoded on its
/// own thread, as splitting costs merging the units, and helpers would
/// only come once a worker is done with its own picture.
fn picture_task(
    job: Box<Job>,
    cancel: Arc<AtomicBool>,
    tasks: Weak<Tasks>,
    threads: usize,
) -> Task {
    Box::new(move || {
        let job: Arc<Job> = Arc::from(job);
        let opened: core::cell::RefCell<Option<Arc<pipe::Par>>> = core::cell::RefCell::new(None);
        let spawn = |par: Arc<pipe::Par>| {
            let Some(tasks) = tasks.upgrade() else {
                return;
            };
            let (job, helped, cancel) = (Arc::clone(&job), Arc::clone(&par), Arc::clone(&cancel));
            let help: Help = Arc::new(move |leave: &dyn Fn() -> bool| {
                let Some(plan) = &job.plan else {
                    return;
                };
                pipe::help(
                    &helped,
                    leave,
                    plan,
                    &job.sets,
                    &job.parsed,
                    &job.ctx,
                    Some(&cancel),
                    job.src.as_deref(),
                    &job.out,
                );
            });
            tasks.open(Open {
                par: Arc::clone(&par),
                help,
            });
            *opened.borrow_mut() = Some(par);
        };
        let spare = tasks.upgrade().map_or(0, |t| t.spare());
        let budget = if spare > 0 { threads } else { 1 };
        run(&job, Some(&cancel), Some((budget, &spawn)));
        if let (Some(par), Some(tasks)) = (opened.take(), tasks.upgrade()) {
            tasks.close(&par);
        }
    })
}

/// The decoding state of a [`Seq`] between two access units: parameter
/// sets, the decoded picture buffer, the pictures due for output and the
/// POC state. The pictures are shared, not copied: a snapshot costs a few
/// reference counts, and keeps its pictures alive while it is held.
#[derive(Clone)]
pub struct Snapshot {
    sets: Arc<ParameterSets>,
    dpb: Vec<DpbPic>,
    out: VecDeque<Pending>,
    prev_tid0_poc: i32,
    first: bool,
    no_irap_yet: bool,
    no_rasl: bool,
    last: Option<Arc<RefPic>>,
    /// The colour planes' decoders (separate colour planes; see `crate::VideoDecoder`).
    pub(crate) planes: Option<Arc<[Snapshot; 3]>>,
}

impl Snapshot {
    fn pics(&self) -> impl Iterator<Item = &Arc<RefPic>> {
        self.dpb
            .iter()
            .map(|p| &p.pic)
            .chain(self.out.iter().map(|p| &p.pic))
            .chain(self.last.iter())
    }

    /// Every picture finished decoding, none cancelled or tainted.
    fn usable(&self) -> bool {
        self.pics()
            .all(|p| p.status().is_some_and(|s| !s.cancelled && !s.tainted))
    }

    /// The pictures held, each as (identity, bytes of samples): pictures
    /// shared between snapshots count once by identity. Each picture holds its
    /// own reservation from the memory budget (shared by every snapshot and
    /// the decoder), so these bytes are reserved already.
    pub fn pictures(&self) -> Vec<(usize, u64)> {
        let mut v: Vec<(usize, u64)> = self
            .pics()
            .map(|p| {
                let bytes: u64 = p
                    .all_dims()
                    .iter()
                    .map(|&(w, h)| w as u64 * h as u64 * 2)
                    .sum();
                (Arc::as_ptr(p) as usize, bytes)
            })
            .collect();
        for s in self.planes.iter().flat_map(|p| p.iter()) {
            v.extend(s.pictures());
        }
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// The state of a sequence decode; see [`crate::VideoDecoder`].
pub struct Seq {
    sets: Arc<ParameterSets>,
    /// Pictures decoded at once; one decodes on the caller's thread.
    threads: usize,
    pool: Option<Pool>,
    /// Pictures handed to the pool and perhaps not finished, oldest first.
    inflight: VecDeque<Arc<RefPic>>,
    /// Stops the pictures in flight; replaced once raised.
    cancel: Arc<AtomicBool>,
    dpb: Vec<DpbPic>,
    /// Bumped pictures, in output order.
    out: VecDeque<Pending>,
    /// `PicOrderCntVal` of `prevTid0Pic`.
    prev_tid0_poc: i32,
    /// The next IRAP picture starts a new coded video sequence (start of
    /// stream, or after an end of sequence NAL unit).
    first: bool,
    /// No IRAP picture decoded yet.
    no_irap_yet: bool,
    /// `NoRaslOutputFlag` of the last IRAP picture.
    no_rasl: bool,
    /// The most recently decoded picture, concealment's source.
    last: Option<Arc<RefPic>>,
    /// Tags of access units that will never be output (skipped, not for
    /// output, or discarded unoutput), oldest first.
    dropped: VecDeque<i64>,
}

impl Seq {
    /// A decoder primed with out-of-band parameter sets.
    /// `threads` is the number of pictures decoded at once: 0 for one per
    /// core (up to 16), 1 for the calling thread only.
    pub fn new(sets: ParameterSets, threads: usize) -> Seq {
        let threads = match threads {
            0 => std::thread::available_parallelism().map_or(1, |n| n.get().min(16)),
            n => n.min(64),
        };
        Seq {
            sets: Arc::new(sets),
            threads,
            pool: None,
            inflight: VecDeque::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            dpb: Vec::new(),
            out: VecDeque::new(),
            prev_tid0_poc: 0,
            first: true,
            no_irap_yet: true,
            no_rasl: true,
            last: None,
            dropped: VecDeque::new(),
        }
    }

    /// The next picture in output order, once it is known to be next and
    /// is decoded. A picture whose decode was cancelled, or that is tainted
    /// by a cancelled one, is dropped instead.
    pub fn next_output(&mut self) -> Option<Decoded> {
        loop {
            let st = self.out.front()?.pic.status()?;
            let p = self.out.pop_front()?;
            if st.cancelled || st.tainted {
                self.dropped.push_back(p.tag);
                continue;
            }
            let pic = &p.pic;
            return Some(Decoded {
                frame: crop(|c, y| pic.line(c, y), &p.geom),
                damaged: st.damaged,
                total: st.total,
                signal: p.geom.signal,
                tag: p.tag,
            });
        }
    }

    /// The tag of the next access unit known never to be output.
    pub fn next_dropped(&mut self) -> Option<i64> {
        self.dropped.pop_front()
    }

    /// The tag of the picture [`Seq::next_output`] would return now.
    pub fn ready_tag(&self) -> Option<i64> {
        let p = self.out.front()?;
        p.pic.status()?;
        Some(p.tag)
    }

    /// Takes the next picture in output order without making its frame
    /// (the caller would throw it away); returns its tag.
    pub fn skip_output(&mut self) -> Option<i64> {
        self.ready_tag()?;
        self.out.pop_front().map(|p| p.tag)
    }

    /// The decoding state between two access units (see [`Snapshot`]).
    /// The previous picture (the source of concealment for a damaged next
    /// picture) is kept only when the buffer holds it anyway, so a snapshot
    /// keeps no picture alive that nothing else needs.
    pub fn snapshot(&self) -> Snapshot {
        let held = |l: &Arc<RefPic>| {
            self.dpb.iter().any(|p| Arc::ptr_eq(&p.pic, l))
                || self.out.iter().any(|p| Arc::ptr_eq(&p.pic, l))
        };
        Snapshot {
            sets: Arc::clone(&self.sets),
            dpb: self.dpb.clone(),
            out: self.out.clone(),
            prev_tid0_poc: self.prev_tid0_poc,
            first: self.first,
            no_irap_yet: self.no_irap_yet,
            no_rasl: self.no_rasl,
            last: self.last.clone().filter(held),
            planes: None,
        }
    }

    /// Stops the pictures in flight and continues from `s` instead. False
    /// (and the decoder left empty, as after an end of sequence) when a
    /// picture of `s` was cancelled or is tainted by one.
    pub fn restore(&mut self, s: &Snapshot) -> bool {
        self.abort();
        for p in self.inflight.drain(..) {
            p.wait_done();
        }
        self.dropped.clear();
        if !s.usable() {
            self.dpb.clear();
            self.out.clear();
            self.last = None;
            self.first = true;
            self.no_irap_yet = true;
            return false;
        }
        self.sets = Arc::clone(&s.sets);
        self.dpb = s.dpb.clone();
        self.out = s.out.clone();
        self.prev_tid0_poc = s.prev_tid0_poc;
        self.first = s.first;
        self.no_irap_yet = s.no_irap_yet;
        self.no_rasl = s.no_rasl;
        self.last = s.last.clone();
        true
    }

    /// Outputs every picture still waiting, in order, empties the buffer
    /// and waits for every picture to be decoded.
    pub fn flush(&mut self) {
        self.flush_dpb();
        for p in self.inflight.drain(..) {
            p.wait_done();
        }
    }

    /// Bumps every picture still waiting, in order, and empties the buffer.
    fn flush_dpb(&mut self) {
        while self.bump() {}
        self.dpb.clear();
    }

    /// Stops the pictures in flight; they will be dropped, not output.
    fn abort(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
    }

    /// Decodes `job` on the calling thread, or hands it to the pool.
    fn submit(&mut self, job: Box<Job>, cancel: Option<&AtomicBool>) -> Result<()> {
        let cancelled = || cancel.is_some_and(|c| c.load(Ordering::Relaxed));
        if self.threads <= 1 {
            run(&job, cancel, None);
            return match job.out.status() {
                Some(s) if s.cancelled => Err(Error::Cancelled),
                _ => Ok(()),
            };
        }
        while self.inflight.front().is_some_and(|p| p.is_done()) {
            self.inflight.pop_front();
        }
        while self.inflight.len() > self.threads {
            let Some(p) = self.inflight.front() else {
                break;
            };
            if p.wait_done_for(CANCEL_POLL) {
                self.inflight.pop_front();
            } else if cancelled() {
                self.abort();
                return Err(Error::Cancelled);
            }
        }
        if cancelled() {
            self.abort();
            return Err(Error::Cancelled);
        }
        let threads = self.threads;
        let pool = self.pool.get_or_insert_with(|| Pool::new(threads));
        if pool.handles.is_empty() {
            // No worker threads (spawning failed): decode here from now on.
            self.threads = 1;
            return self.submit(job, cancel);
        }
        let out = Arc::clone(&job.out);
        let task = picture_task(
            job,
            Arc::clone(&self.cancel),
            Arc::downgrade(&pool.tasks),
            threads,
        );
        pool.tasks.push(task);
        self.inflight.push_back(out);
        Ok(())
    }

    /// Decodes one access unit: its NAL units without start codes or length
    /// prefixes. Parameter sets in it replace earlier ones with their ids.
    ///
    /// A picture that cannot be placed in the sequence (no parameter sets, a
    /// leading picture with no usable anchor) is skipped; one whose slices
    /// are damaged is concealed. Errors report a picture that was skipped.
    pub fn decode(&mut self, nals: &[&[u8]], tag: i64, cancel: Option<&AtomicBool>) -> Result<()> {
        let mut parsed: Vec<(NalHeader, Rbsp)> = Vec::new();
        let mut first_err: Option<Error> = None;
        let mut eos = false;
        for nal in nals {
            let h = match NalHeader::parse(nal) {
                Ok(h) => h,
                Err(e) => {
                    first_err.get_or_insert(e);
                    continue;
                }
            };
            if h.layer_id != 0 {
                continue;
            }
            match h.nal_unit_type {
                32..=34 => {
                    if let Err(e) = Arc::make_mut(&mut self.sets).add(nal) {
                        first_err.get_or_insert(e);
                    }
                }
                nut::EOS | nut::EOB => eos = true,
                // Reserved VCL types are ignored (7.4.2.2).
                0..=9 | 16..=21 => match Rbsp::from_nal(nal) {
                    Ok(r) => parsed.push((h, r)),
                    Err(e) => {
                        first_err.get_or_insert(e);
                    }
                },
                _ => {}
            }
        }
        let res = if parsed.is_empty() {
            self.dropped.push_back(tag);
            Ok(())
        } else {
            let r = self.picture(parsed, tag, cancel, first_err);
            if r.is_err() {
                self.dropped.push_back(tag);
            }
            r
        };
        if eos {
            // The next picture is an IRAP picture with NoRaslOutputFlag 1, which
            // outputs (or, a CRA picture, discards) what is still waiting (C.5.2.2).
            self.first = true;
        }
        res
    }

    /// A decoder of its own for one colour plane of a stream with separate colour planes,
    /// starting from this one's parameter sets.
    pub fn plane_decoder(&self) -> Seq {
        let mut s = Seq::new(ParameterSets::default(), self.threads);
        s.sets = Arc::clone(&self.sets);
        s
    }

    /// For an access unit whose pictures have separate colour planes, its NAL units for each
    /// colour plane's decoder: the slice segments of that colour_plane_id (a dependent
    /// segment goes with the segment before it) and every non-VCL unit. `None` for any other
    /// access unit. Parameter sets in the access unit are taken first.
    pub fn split_planes<'a>(&mut self, nals: &[&'a [u8]]) -> Option<[Vec<&'a [u8]>; 3]> {
        for nal in nals {
            if let Ok(h) = NalHeader::parse(nal)
                && h.layer_id == 0
                && matches!(h.nal_unit_type, 32..=34)
            {
                let _ = Arc::make_mut(&mut self.sets).add(nal);
            }
        }
        split_planes(&self.sets, nals)
    }
}

/// [`Seq::split_planes`] with the parameter sets `sets` (taken from them only).
pub fn split_planes<'a>(sets: &ParameterSets, nals: &[&'a [u8]]) -> Option<[Vec<&'a [u8]>; 3]> {
    // The first slice's SPS, from the few bytes up to slice_pic_parameter_set_id: most
    // streams stop here.
    let separate = nals.iter().find_map(|nal| {
        let h = NalHeader::parse(nal).ok()?;
        if h.layer_id != 0 || !matches!(h.nal_unit_type, 0..=9 | 16..=21) {
            return None;
        }
        let rbsp = Rbsp::from_nal(&nal[..nal.len().min(16)]).ok()?;
        let r = &mut crate::bits::BitReader::new(&rbsp.data);
        r.u1().ok()?;
        if h.is_irap() {
            r.u1().ok()?;
        }
        let pps = sets.pps(r.ue().ok()?).ok()?;
        Some(sets.sps(pps.sps_id).ok()?.separate_colour_plane)
    });
    if separate != Some(true) {
        return None;
    }
    let mut out: [Vec<&'a [u8]>; 3] = Default::default();
    let mut plane = None;
    let mut split = false;
    for nal in nals {
        let Ok(h) = NalHeader::parse(nal) else {
            continue;
        };
        let vcl = h.layer_id == 0 && matches!(h.nal_unit_type, 0..=9 | 16..=21);
        if !vcl {
            out.iter_mut().for_each(|o| o.push(nal));
            continue;
        }
        let Ok(rbsp) = Rbsp::from_nal(nal) else {
            continue;
        };
        match first_header(sets, &[(h, rbsp)]) {
            Ok((sh, sps)) => {
                if !sps.separate_colour_plane {
                    return None;
                }
                split = true;
                plane = Some(sh.colour_plane as usize);
            }
            // A dependent segment: the plane of the segment before it.
            Err(_) if plane.is_some() => {}
            Err(_) => continue,
        }
        if let Some(p) = plane {
            out[p].push(nal);
        }
    }
    split.then_some(out)
}

impl Seq {
    /// Places, decodes and stores one picture.
    fn picture(
        &mut self,
        parsed: Vec<(NalHeader, Rbsp)>,
        tag: i64,
        cancel: Option<&AtomicBool>,
        first_err: Option<Error>,
    ) -> Result<()> {
        let nh = parsed[0].0;
        let t = nh.nal_unit_type;
        if !(0..=9).contains(&t) && !(16..=21).contains(&t) {
            return Err(Error::Unsupported("reserved NAL unit type"));
        }
        let irap = nh.is_irap();
        let (sh, geom, n_ctbs, log2_ctb, max_lsb, max_dec, max_reorder, max_latency) = {
            let (h, sps) = first_header(&self.sets, &parsed)?;
            (
                h,
                OutGeom::of(sps),
                sps.size_in_ctbs(),
                sps.log2_ctb,
                1i32 << sps.log2_max_poc_lsb,
                sps.max_dec_pic_buffering as usize,
                sps.max_num_reorder as usize,
                sps.max_latency,
            )
        };
        if geom.width.saturating_mul(geom.height) > 1 << 28 {
            return Err(Error::TooLarge);
        }
        if irap {
            self.no_rasl = self.first || (nut::BLA_N_LP - 2..=nut::IDR_N_LP).contains(&t);
        } else if self.no_irap_yet {
            return Err(Error::InvalidData("picture before the first IRAP picture"));
        }
        if matches!(t, nut::RASL_N | nut::RASL_R) && self.no_rasl {
            // Its references precede the random access point (8.1.3: not
            // output, and not decoded either).
            self.dropped.push_back(tag);
            return Ok(());
        }
        // 8.3.1: PicOrderCntVal.
        let lsb = sh.poc_lsb as i32;
        let msb = if irap && self.no_rasl {
            0
        } else {
            let prev_lsb = self.prev_tid0_poc & (max_lsb - 1);
            let prev_msb = self.prev_tid0_poc - prev_lsb;
            if lsb < prev_lsb && prev_lsb - lsb >= max_lsb / 2 {
                prev_msb + max_lsb
            } else if lsb > prev_lsb && lsb - prev_lsb > max_lsb / 2 {
                prev_msb - max_lsb
            } else {
                prev_msb
            }
        };
        let poc = msb.wrapping_add(lsb);
        let sub_layer_non_ref = t <= 14 && t.is_multiple_of(2);
        if nh.temporal_id == 0 && !(nut::RADL_N..=nut::RASL_R).contains(&t) && !sub_layer_non_ref {
            self.prev_tid0_poc = poc;
        }

        // 8.3.2: the reference picture set and the marking it implies.
        let ctx = self.apply_rps(&sh, t, poc, max_lsb, &geom)?;

        // C.5.2.2: output and removal before the picture is decoded.
        if irap && self.no_rasl && !self.no_irap_yet {
            // A CRA picture here follows an end of sequence: NoOutputOfPriorPicsFlag
            // is 1 whatever its slice header says.
            let no_output = t == nut::CRA || sh.no_output_of_prior_pics;
            if no_output {
                let gone = self.dpb.iter().filter(|p| p.output).map(|p| p.tag);
                self.dropped.extend(gone);
                self.dpb.clear();
            } else {
                self.flush_dpb();
            }
        } else {
            self.dpb.retain(|p| p.output || p.mark != Mark::Unused);
            loop {
                let waiting = self.dpb.iter().filter(|p| p.output).count();
                let late = max_latency
                    .is_some_and(|l| self.dpb.iter().any(|p| p.output && p.latency >= l));
                let full = self.dpb.len() >= max_dec.max(1);
                if !(waiting > max_reorder || late || full || self.dpb.len() >= MAX_DPB) {
                    break;
                }
                if !self.bump() {
                    // Nothing left to output: the buffer is full of
                    // references, which only a broken stream does.
                    if self.dpb.len() >= MAX_DPB {
                        self.dpb.remove(0);
                    } else {
                        break;
                    }
                }
            }
        }
        self.no_irap_yet = false;
        self.first = false;

        // Decode: what cannot be decoded at all is known before the
        // decode starts, everything else is concealed.
        let plan = match plan(&self.sets, &parsed, true, first_err) {
            Ok(p) => p
                .sets(&self.sets)
                .is_ok_and(|(sps, _)| plane_dims(&OutGeom::of(sps)) == plane_dims(&geom))
                .then_some(p),
            Err(e @ (Error::Cancelled | Error::Unsupported(_) | Error::TooLarge)) => return Err(e),
            Err(_) => None,
        };
        let stand_in = plan.is_none();
        // 16-bit samples of every plane, reserved before the rows are allocated: up to
        // MAX_DPB of them stay in the buffer.
        let samples: usize = plane_dims(&geom).iter().map(|(w, h)| w * h).sum();
        let res =
            specular_core::budget::reserve_n(samples as u64, 2).map_err(Error::NeedsMemory)?;
        let out = Arc::new(RefPic::new(
            poc,
            plane_dims(&geom),
            (geom.bit_depth_y, geom.bit_depth_c),
            log2_ctb,
            geom.sub_h,
            sh.is_inter() && !stand_in,
        ));
        let job = Box::new(Job {
            sets: Arc::clone(&self.sets),
            parsed,
            ctx,
            src: self.last.clone(),
            out: Arc::clone(&out),
            total: n_ctbs,
            plan,
        });
        // A picture cancelled before or while it decodes still joins the
        // buffer (see the module notes), finished, never output.
        let cancelled_now = match self.submit(job, cancel) {
            Ok(()) => false,
            Err(Error::Cancelled) => true,
            Err(e) => return Err(e),
        };
        if cancelled_now {
            out.finish(cancelled(n_ctbs));
        }
        let pic = out;
        self.last = Some(Arc::clone(&pic));

        // C.5.2.3: the picture joins the buffer; bump what is due.
        for p in &mut self.dpb {
            if p.output {
                p.latency = p.latency.saturating_add(1);
            }
        }
        self.dpb.push(DpbPic {
            pic,
            mark: Mark::Short,
            output: sh.pic_output && !cancelled_now,
            latency: 0,
            geom,
            tag,
            res: Arc::new(res),
        });
        // A cancelled picture's tag is dropped by the caller, with the error.
        if !sh.pic_output && !cancelled_now {
            self.dropped.push_back(tag);
        }
        loop {
            let waiting = self.dpb.iter().filter(|p| p.output).count();
            let late =
                max_latency.is_some_and(|l| self.dpb.iter().any(|p| p.output && p.latency >= l));
            if !(waiting > max_reorder || late) || !self.bump() {
                break;
            }
        }
        if cancelled_now {
            return Err(Error::Cancelled);
        }
        Ok(())
    }

    /// Derives the reference picture set of the current picture, marks the
    /// buffer accordingly and builds the reference table (8.3.2, 8.3.3).
    fn apply_rps(
        &mut self,
        sh: &SliceHeader,
        t: u8,
        poc: i32,
        max_lsb: i32,
        geom: &OutGeom,
    ) -> Result<InterCtx> {
        let mut ctx = InterCtx {
            poc,
            ..InterCtx::default()
        };
        if matches!(t, nut::IDR_W_RADL | nut::IDR_N_LP) {
            for p in &mut self.dpb {
                p.mark = Mark::Unused;
            }
            return Ok(ctx);
        }
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut foll = Vec::new();
        for (d, used) in sh.st_rps.negative() {
            (if used { &mut before } else { &mut foll }).push(poc.wrapping_add(d));
        }
        for (d, used) in sh.st_rps.positive() {
            (if used { &mut after } else { &mut foll }).push(poc.wrapping_add(d));
        }
        // (POC, full POC known, used by the current picture).
        let lt: Vec<(i32, bool, bool)> = sh
            .lt
            .iter()
            .map(|e| {
                let mut p = e.poc_lsb as i32;
                if e.msb_present {
                    let cycle = (e.msb_cycle as i32).wrapping_mul(max_lsb);
                    p = poc.wrapping_sub(cycle).wrapping_sub(sh.poc_lsb as i32 - p);
                }
                (p, e.msb_present, e.used)
            })
            .collect();

        let mut new_mark = vec![Mark::Unused; self.dpb.len()];
        let mut lt_slots: Vec<Option<usize>> = Vec::with_capacity(lt.len());
        for &(p, full, _) in &lt {
            let found = self.dpb.iter().position(|d| {
                d.mark != Mark::Unused
                    && if full {
                        d.pic.poc == p
                    } else {
                        d.pic.poc & (max_lsb - 1) == p
                    }
            });
            if let Some(i) = found {
                new_mark[i] = Mark::Long;
            }
            lt_slots.push(found);
        }
        let dpb = &self.dpb;
        let find_st = |p: i32, new_mark: &mut Vec<Mark>| {
            let i = dpb.iter().enumerate().position(|(i, d)| {
                d.mark == Mark::Short && new_mark[i] == Mark::Unused && d.pic.poc == p
            })?;
            new_mark[i] = Mark::Short;
            Some(i)
        };
        let before_slots: Vec<Option<usize>> =
            before.iter().map(|&p| find_st(p, &mut new_mark)).collect();
        let after_slots: Vec<Option<usize>> =
            after.iter().map(|&p| find_st(p, &mut new_mark)).collect();
        for &p in &foll {
            find_st(p, &mut new_mark);
        }
        for (d, m) in self.dpb.iter_mut().zip(&new_mark) {
            d.mark = *m;
        }

        // The reference table, generating what is missing.
        let geom = *geom;
        let entry = |slot: Option<usize>, p: i32, lt: bool, dpb: &mut Vec<DpbPic>| {
            let usable = slot.filter(|&i| dpb[i].pic.all_dims() == plane_dims(&geom));
            let pic = match usable {
                Some(i) => Arc::clone(&dpb[i].pic),
                None => {
                    let g = Arc::new(RefPic::generated(
                        p,
                        plane_dims(&geom),
                        (geom.bit_depth_y, geom.bit_depth_c),
                    ));
                    if dpb.len() < MAX_DPB {
                        dpb.push(DpbPic {
                            pic: Arc::clone(&g),
                            mark: if lt { Mark::Long } else { Mark::Short },
                            output: false,
                            latency: 0,
                            geom,
                            tag: 0,
                            // A generated picture shares one grey row: nothing to hold.
                            res: Arc::new(specular_core::Reservation::none()),
                        });
                    }
                    g
                }
            };
            RefEntry {
                poc: pic.poc,
                pic,
                lt,
            }
        };
        for (&p, &s) in before.iter().zip(&before_slots) {
            ctx.refs.push(entry(s, p, false, &mut self.dpb));
        }
        ctx.n_before = ctx.refs.len();
        for (&p, &s) in after.iter().zip(&after_slots) {
            ctx.refs.push(entry(s, p, false, &mut self.dpb));
        }
        ctx.n_after = ctx.refs.len() - ctx.n_before;
        for (&(p, _, used), &s) in lt.iter().zip(&lt_slots) {
            if used {
                ctx.refs.push(entry(s, p, true, &mut self.dpb));
            }
        }
        if ctx.refs.len() > 255 {
            return Err(Error::InvalidData("too many reference pictures"));
        }
        Ok(ctx)
    }

    /// Outputs the waiting picture with the smallest POC (C.5.2.4); false
    /// when none is waiting.
    fn bump(&mut self) -> bool {
        let Some(i) = self
            .dpb
            .iter()
            .enumerate()
            .filter(|(_, p)| p.output)
            .min_by_key(|(_, p)| p.pic.poc)
            .map(|(i, _)| i)
        else {
            return false;
        };
        let p = &mut self.dpb[i];
        p.output = false;
        self.out.push_back(Pending {
            pic: Arc::clone(&p.pic),
            geom: p.geom,
            tag: p.tag,
            _res: Arc::clone(&p.res),
        });
        if p.mark == Mark::Unused {
            self.dpb.remove(i);
        }
        true
    }
}

/// The SPS and first slice segment header of a picture.
fn first_header<'s>(
    sets: &'s ParameterSets,
    parsed: &[(NalHeader, Rbsp)],
) -> Result<(SliceHeader, &'s Sps)> {
    let mut err = Error::NoSlices;
    for (nh, rbsp) in parsed {
        let r = &mut crate::bits::BitReader::new(&rbsp.data);
        let id = r.u1().and_then(|_| {
            if nh.is_irap() {
                r.u1()?;
            }
            r.ue()
        });
        let res = id.and_then(|id| {
            let pps = sets.pps(id)?;
            let sps = sets.sps(pps.sps_id)?;
            Ok((parse_slice_header(nh, &rbsp.data, sps, pps, None)?, sps))
        });
        match res {
            Ok((h, sps)) if !h.dependent => return Ok((h, sps)),
            Ok(_) => err = Error::InvalidData("picture starts with a dependent slice segment"),
            Err(e) => err = e,
        }
    }
    Err(err)
}

/// The plane sizes of pictures of geometry `g`, as [`crate::picture::Picture`]
/// allocates them.
fn plane_dims(g: &OutGeom) -> [(usize, usize); 3] {
    let c = match (g.width.checked_div(g.sub_w), g.height.checked_div(g.sub_h)) {
        (Some(a), Some(b)) => (a, b),
        _ => (0, 0),
    };
    [(g.width, g.height), c, c]
}

impl Drop for Seq {
    fn drop(&mut self) {
        // Nobody will see the pictures in flight.
        self.cancel.store(true, Ordering::Relaxed);
        for p in self.inflight.drain(..) {
            p.wait_done();
        }
    }
}
