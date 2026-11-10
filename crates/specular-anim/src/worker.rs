//! Decode-ahead: a background thread pulls frames from a [`Frames`] decoder into a bounded
//! frame ring shared with the player.
//!
//! The ring is keyed by frame index. The worker decodes in display order, tracking `cursor`,
//! the forward sequence number (loop × count + index) of the next frame it will produce, and
//! the player publishes `pos`, the sequence number it shows or waits for. Frames in
//! `pos..cursor` are the *window* (needed soon) and are never evicted; everything else in the
//! ring was shown already and stays only while the byte budget allows, for stepping back and
//! for caching small animations whole. When the ring holds every frame the animation is
//! *complete*: the decoder is dropped and the thread ends, so later loops cost nothing.
//!
//! The lock is held only to plan the next step and to insert its result, never during a
//! decode.

use crate::FrameRef;
use parking_lot::{Condvar, Mutex};
use specular_core::CancelToken;
use specular_decode::{DecodeError, Frames};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Called from the worker thread when a frame the player is waiting for has arrived (or the
/// animation ended or failed). Typically sends a user event to the event loop so the UI ticks.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkerConfig {
    pub budget: usize,
    pub max_ahead: u32,
}

#[derive(Default)]
pub(crate) struct Ring {
    pub frames: BTreeMap<u32, FrameRef>,
    pub bytes: usize,
}

pub(crate) struct State {
    pub ring: Ring,
    /// Stored delay of every frame decoded so far, by index.
    pub delays: Vec<Duration>,
    pub count: Option<u32>,
    /// Forward sequence number the player shows or waits for.
    pub pos: u64,
    /// Do not decode at or past this forward sequence number (finite loop counts).
    pub limit: Option<u64>,
    /// The player is stalled waiting for a frame: call the waker on the next arrival.
    pub waiting: bool,
    /// Every frame is in the ring; the worker has ended.
    pub complete: bool,
    pub error: Option<String>,
    /// A frame needed more free RAM than there is: playback stops (with `error`) rather than
    /// looping the frames before it.
    pub memory: Option<specular_core::NeedsMemory>,
    /// The worker thread has ended.
    pub exited: bool,
    /// The worker has nothing to do until the player moves (for tests and benchmarks).
    pub idle: bool,
    /// Size of the last decoded frame.
    pub frame_bytes: usize,
    pub decoded: u64,
    pub evicted: u64,
    pub discarded: u64,
    pub rewinds: u64,
    /// The player is scrubbing (the seek bar is being dragged): the worker decodes toward
    /// the pointer in steps and keeps every frame it decodes (see [`Worker::scrub_goal`]).
    pub scrub: bool,
    /// The last move of the drag was backwards.
    pub back: bool,
    /// Forward sequence number of the frame on screen.
    pub shown: Option<u64>,
    /// Forward sequence number of the frame being decoded right now.
    pub busy: Option<u64>,
    /// Indices of the frames a seek starts decoding at ([`Frames::sync_frames`]).
    pub sync: Option<Arc<[u32]>>,
    /// The decoder stops a decode between two samples when [`Shared::interrupt`] is set.
    pub interruptible: bool,
    /// Decodes stopped because the frame was no longer wanted.
    pub interrupted: u64,
}

pub(crate) struct Shared {
    pub state: Mutex<State>,
    /// Wakes the worker (the player moved, or cancellation).
    pub work: Condvar,
    /// Signals `idle`/`exited` changes.
    pub idle: Condvar,
    /// Set (by the player) to stop the decode in progress: it is no longer wanted.
    pub interrupt: Arc<AtomicBool>,
}

impl Shared {
    pub fn new(count: Option<u32>) -> Arc<Shared> {
        Arc::new(Shared {
            state: Mutex::new(State {
                ring: Ring::default(),
                delays: Vec::new(),
                count: count.filter(|&n| n > 0),
                pos: 0,
                limit: None,
                waiting: false,
                complete: false,
                error: None,
                memory: None,
                exited: false,
                idle: false,
                frame_bytes: 0,
                decoded: 0,
                evicted: 0,
                discarded: 0,
                rewinds: 0,
                scrub: false,
                back: false,
                shown: None,
                busy: None,
                sync: None,
                interruptible: false,
                interrupted: 0,
            }),
            work: Condvar::new(),
            idle: Condvar::new(),
            interrupt: Arc::new(AtomicBool::new(false)),
        })
    }
}

/// Frame index of forward sequence number `s`.
pub(crate) fn index_of(s: u64, count: Option<u32>) -> u32 {
    match count {
        Some(n) => (s % u64::from(n)) as u32,
        None => s.min(u64::from(u32::MAX)) as u32,
    }
}

/// The last of the ascending `sync` frames at or before `i` (0 when none is).
pub(crate) fn key_of(sync: &[u32], i: u32) -> u32 {
    match sync.partition_point(|&k| k <= i) {
        0 => 0,
        n => sync[n - 1],
    }
}

impl State {
    /// The forward sequence number of the sync frame a decode of `target` starts at, when
    /// the sync frames are known.
    pub fn key_seq(&self, target: u64) -> Option<u64> {
        let sync = self.sync.as_deref()?;
        let t = index_of(target, self.count);
        Some(target - u64::from(t - key_of(sync, t)))
    }

    /// Whether going on decoding from `at` (forwards, without starting over at a sync frame)
    /// reaches `target`.
    pub fn reaches(&self, at: u64, target: u64) -> bool {
        at <= target && self.key_seq(target).is_none_or(|k| k <= at)
    }

    /// Whether frame `i` is shown at some sequence number in `pos..cursor`.
    fn in_window(&self, i: u32, cursor: u64) -> bool {
        if cursor <= self.pos {
            return false;
        }
        let span = cursor - self.pos;
        match self.count {
            Some(n) => {
                let n = u64::from(n);
                span >= n || (u64::from(i) + n - self.pos % n) % n < span
            }
            None => u64::from(i) >= self.pos && u64::from(i) < cursor,
        }
    }

    /// Frames and bytes in the window.
    fn window(&self, cursor: u64) -> (usize, usize) {
        self.ring
            .frames
            .iter()
            .filter(|(i, _)| self.in_window(**i, cursor))
            .fold((0, 0), |(n, b), (_, f)| {
                (n + 1, b + f.raster.memory_bytes())
            })
    }

    /// Eviction order of a frame outside the window: smallest first. Going forwards from the
    /// cursor, the first frames met are the ones shown longest ago; frames just behind the
    /// playhead go last, so stepping back stays cheap.
    fn evict_key(&self, i: u32, cursor: u64) -> u64 {
        let i = u64::from(i);
        match self.count {
            Some(n) => {
                let n = u64::from(n);
                (i + n - cursor % n) % n
            }
            None if i >= cursor => i - cursor,
            None => (u64::MAX / 2) + i,
        }
    }

    /// Store frame `i` (just decoded; `cursor` already advanced past it), evicting frames
    /// outside the window as needed. While scrubbing every frame decoded is wanted (it is on
    /// the way to the pointer).
    fn insert(&mut self, frame: FrameRef, cursor: u64, budget: usize) {
        let i = frame.index;
        let size = frame.raster.memory_bytes();
        let needed = self.scrub || self.in_window(i, cursor);
        while self.ring.bytes + size > budget {
            let victim = self
                .ring
                .frames
                .keys()
                .copied()
                .filter(|&k| !self.in_window(k, cursor))
                .min_by_key(|&k| self.evict_key(k, cursor));
            // A frame outside the window is only worth keeping if it costs nothing else.
            let Some(victim) = victim.filter(|_| needed) else {
                break;
            };
            if let Some(f) = self.ring.frames.remove(&victim) {
                self.ring.bytes -= f.raster.memory_bytes();
                self.evicted += 1;
            }
        }
        if !needed && self.ring.bytes + size > budget {
            self.discarded += 1;
            return;
        }
        self.ring.bytes += size;
        self.ring.frames.insert(i, frame);
    }

    /// The decoder ended (or failed) at frame `n`: that is the frame count.
    fn end_at(&mut self, n: u32) {
        if self.count != Some(n) {
            self.count = Some(n);
            self.delays.truncate(n as usize);
            let gone: Vec<u32> = self.ring.frames.range(n..).map(|(k, _)| *k).collect();
            for k in gone {
                if let Some(f) = self.ring.frames.remove(&k) {
                    self.ring.bytes -= f.raster.memory_bytes();
                }
            }
        }
        self.check_complete();
    }

    fn check_complete(&mut self) {
        if self
            .count
            .is_some_and(|n| self.ring.frames.len() >= n as usize)
        {
            self.complete = true;
        }
    }
}

/// What stands in for the decoder once the worker has dropped it.
struct Ended;

impl Frames for Ended {
    fn next_frame(
        &mut self,
        _: &CancelToken,
    ) -> Option<specular_decode::Result<specular_decode::Frame>> {
        None
    }
    fn rewind(&mut self) -> specular_decode::Result<()> {
        Ok(())
    }
}

enum Plan {
    Decode { rewind: bool, seek: Option<u32> },
    Exit,
}

struct Worker {
    shared: Arc<Shared>,
    frames: Box<dyn Frames>,
    cancel: CancelToken,
    cfg: WorkerConfig,
    waker: Option<Waker>,
    /// Next index the decoder will produce.
    dec_idx: u32,
    /// Forward sequence number of that frame.
    cursor: u64,
    need_rewind: bool,
    /// Frame to position the decoder on before the next decode ([`Frames::seek`]).
    need_seek: Option<u32>,
    /// The decoder repositions itself ([`Frames::seek`]); cleared when it declines.
    seekable: bool,
    /// The decoder has been dropped (the worker is ending).
    ended: bool,
    /// Where the decoder was before a seek was planned (`cursor`, `dec_idx`, `need_rewind`),
    /// to go on from there when the decoder declines it.
    before_seek: (u64, u32, bool),
}

pub(crate) fn spawn(
    shared: Arc<Shared>,
    frames: Box<dyn Frames>,
    cancel: CancelToken,
    cfg: WorkerConfig,
    waker: Option<Waker>,
) -> Option<std::thread::JoinHandle<()>> {
    let mut w = Worker {
        shared: Arc::clone(&shared),
        frames,
        cancel,
        cfg,
        waker,
        dec_idx: 0,
        cursor: 0,
        need_rewind: false,
        need_seek: None,
        seekable: true,
        before_seek: (0, 0, false),
        ended: false,
    };
    // A decoder that knows every frame's time up front (a movie's sample table) closes the
    // timeline at once, so seeking anywhere and the time readout work before playing through.
    if let Some(delays) = w.frames.delays().filter(|d| !d.is_empty()) {
        let mut st = shared.state.lock();
        st.count = st.count.or(u32::try_from(delays.len()).ok());
        st.delays = delays;
    }
    {
        let sync = w.frames.sync_frames().map(Arc::from);
        let interruptible = w.frames.set_interrupt(Arc::clone(&shared.interrupt));
        let mut st = shared.state.lock();
        st.sync = sync;
        st.interruptible = interruptible;
    }
    match specular_thread::builder("anim-play").spawn(move || w.run_guarded()) {
        Ok(h) => Some(h),
        Err(e) => {
            let mut st = shared.state.lock();
            st.error = Some(format!("cannot start the animation thread: {e}"));
            st.exited = true;
            st.idle = true;
            None
        }
    }
}

impl Worker {
    /// [`Worker::run`] behind a panic boundary: a decoder that panics on a frame fails the
    /// animation like a decode error (the player shows it and stops), never the process.
    fn run_guarded(&mut self) {
        let what = "animation playback";
        if let Err(p) = specular_core::isolate::run(what, || self.run()) {
            self.fail(p.brief());
            // Marks the worker exited and wakes the player; returns Exit as the error is set.
            let _ = self.plan();
        }
    }

    fn run(&mut self) {
        loop {
            let (rewind, seek) = match self.plan() {
                Plan::Exit => return,
                Plan::Decode { rewind, seek } => (rewind, seek),
            };
            if let Some(i) = seek {
                match self.frames.seek(i) {
                    Ok(true) => {}
                    Ok(false) => {
                        // Not seekable after all (an animation's decoder): back to where it
                        // was, and plan again by rewinding or decoding on.
                        self.seekable = false;
                        (self.cursor, self.dec_idx, self.need_rewind) = self.before_seek;
                        continue;
                    }
                    Err(e) => {
                        self.fail(format!("seek failed: {e}"));
                        continue;
                    }
                }
            }
            if rewind {
                if let Err(e) = self.frames.rewind() {
                    self.fail(format!("rewind failed: {e}"));
                    continue;
                }
                self.shared.state.lock().rewinds += 1;
            }
            let result = self.frames.next_frame(&self.cancel);
            self.store(result);
        }
    }

    /// Wait until there is something to decode; position the decoder for the player.
    fn plan(&mut self) -> Plan {
        let shared = Arc::clone(&self.shared);
        let mut st = shared.state.lock();
        loop {
            if self.cancel.is_cancelled() || st.complete || st.error.is_some() {
                // The decoder goes before the worker says it has ended (outside the lock: a
                // decoder may take a while to wind down).
                if !self.ended {
                    self.ended = true;
                    drop(st);
                    drop(std::mem::replace(&mut self.frames, Box::new(Ended)));
                    st = shared.state.lock();
                }
                st.exited = true;
                st.idle = true;
                let wake = std::mem::take(&mut st.waiting);
                drop(st);
                shared.idle.notify_all();
                if wake {
                    self.wake();
                }
                return Plan::Exit;
            }
            // Reposition for a seek: the wanted frame is missing and the decoder is past it,
            // or rewinding reaches it sooner than decoding on.
            let target = st.pos;
            let tidx = index_of(target, st.count);
            if !st.ring.frames.contains_key(&tidx) {
                let forward = target.saturating_sub(self.cursor);
                // Scrubbing: the next frame to decode on the way to the pointer.
                let goal = (st.scrub && self.seekable).then(|| self.scrub_goal(&st, target));
                let jump = match goal {
                    Some(g) => g != self.cursor,
                    None => self.cursor > target || forward > 1,
                };
                if self.seekable && jump {
                    // The decoder finds the frame itself (from the nearest key frame).
                    let goal = goal.unwrap_or(target);
                    if self.need_seek.is_none() {
                        self.before_seek = (self.cursor, self.dec_idx, self.need_rewind);
                    }
                    self.cursor = goal;
                    self.dec_idx = index_of(goal, st.count);
                    self.need_seek = Some(self.dec_idx);
                    self.need_rewind = false;
                } else if self.cursor > target || u64::from(tidx) < forward {
                    let start = target - u64::from(tidx);
                    if self.cursor != start || self.dec_idx != 0 {
                        self.cursor = start;
                        self.dec_idx = 0;
                        self.need_rewind = true;
                        self.need_seek = None;
                    }
                }
            }
            if st.count.is_some_and(|n| self.dec_idx >= n) {
                // End of a pass: continue with the next loop.
                self.dec_idx = 0;
                self.need_rewind = true;
                self.need_seek = None;
            }
            let mut wait = st.limit.is_some_and(|l| self.cursor >= l);
            if !wait && self.cursor >= target {
                let fb = st.frame_bytes;
                let eager = st
                    .count
                    .is_some_and(|n| (n as usize).saturating_mul(fb) <= self.cfg.budget);
                if !eager && self.cursor - target > u64::from(self.cfg.max_ahead.max(1)) {
                    wait = true;
                } else {
                    let (n, bytes) = st.window(self.cursor);
                    wait = n >= 2 && bytes + fb > self.cfg.budget;
                }
                // Already in the ring (an earlier loop): decoding it only advances the
                // decoder, which the window check above has already allowed.
            }
            if !wait {
                st.idle = false;
                st.busy = Some(self.cursor);
                // A stop asked for before this decode was planned is for an older one.
                self.shared.interrupt.store(false, Ordering::Release);
                return Plan::Decode {
                    rewind: std::mem::take(&mut self.need_rewind),
                    seek: self.need_seek.take(),
                };
            }
            if !st.idle {
                st.idle = true;
                shared.idle.notify_all();
            }
            shared.work.wait(&mut st);
        }
    }

    fn store(&mut self, result: Option<specular_decode::Result<specular_decode::Frame>>) {
        let shared = Arc::clone(&self.shared);
        let mut st = shared.state.lock();
        st.busy = None;
        match result {
            None if self.dec_idx == 0 => {
                st.error = Some("animation has no frames".into());
            }
            None => {
                let n = self.dec_idx;
                // The pass ended: `cursor` is the first frame of the next loop.
                if st.count != Some(n) {
                    specular_log::debug!(target: specular_log::DECODE, "anim: {n} frames");
                    self.cursor = self.cursor.div_ceil(u64::from(n)) * u64::from(n);
                }
                st.end_at(n);
                self.dec_idx = 0;
                self.need_rewind = true;
            }
            Some(Err(DecodeError::Cancelled)) if self.cancel.is_cancelled() => {}
            Some(Err(DecodeError::Cancelled))
                if st.interruptible && shared.interrupt.load(Ordering::Acquire) =>
            {
                // No longer wanted: the decoder is where it was before this decode, and the
                // next plan positions it for the frame wanted now.
                st.interrupted += 1;
            }
            Some(Err(DecodeError::NeedsMemory(n))) => {
                // Not a broken frame: the clip is whole, the memory is short. Stop here and
                // say so (the frames before it are not a shorter clip to loop).
                specular_log::warn!(target: specular_log::DECODE, "anim: frame {}: {n}; stopping", self.dec_idx);
                st.error = Some(n.to_string());
                st.memory = Some(n);
            }
            Some(Err(e)) if self.dec_idx == 0 => {
                st.error = Some(e.to_string());
            }
            Some(Err(e)) => {
                // Play what decoded: the animation ends before the broken frame.
                let n = self.dec_idx;
                specular_log::warn!(target: specular_log::DECODE, "anim: frame {}: {e}; playing {n} frames", self.dec_idx);
                if st.count != Some(n) {
                    self.cursor = self.cursor.div_ceil(u64::from(n)) * u64::from(n);
                }
                st.end_at(n);
                self.dec_idx = 0;
                self.need_rewind = true;
            }
            Some(Ok(_)) if st.count.is_some_and(|n| self.dec_idx >= n) => {
                // More frames than the container states: play the stated ones.
                self.dec_idx = 0;
                self.need_rewind = true;
            }
            Some(Ok(frame)) => {
                let i = self.dec_idx;
                self.dec_idx += 1;
                self.cursor += 1;
                st.decoded += 1;
                if i as usize == st.delays.len() {
                    st.delays.push(frame.delay);
                }
                st.frame_bytes = frame.raster.memory_bytes();
                if !st.ring.frames.contains_key(&i) {
                    let f = Arc::new(crate::AnimFrame {
                        index: i,
                        raster: frame.raster,
                        delay: frame.delay,
                        strip: false,
                    });
                    st.insert(f, self.cursor, self.cfg.budget);
                }
                st.check_complete();
            }
        }
        let wake = std::mem::take(&mut st.waiting);
        drop(st);
        if wake {
            self.wake();
        }
    }

    /// While scrubbing, the frame to decode next on the way to `target` (the pointer), so a
    /// frame shows on every step rather than only once the pointer rests:
    ///
    /// * dragging backwards through long-GOP video (every frame there decodes from a sync
    ///   frame before it), the frames just before the target too, as many as fit half the
    ///   ring ([`Worker::behind`]): the drag goes on over them without decoding again;
    /// * when decoding on from where the decoder is reaches the target (dragging forwards
    ///   within a GOP), a quarter of the way there, so the picture eases toward the pointer
    ///   instead of jumping (nearer than 8 frames: the target itself);
    /// * when it must start over at a sync frame, that sync frame first (one frame to
    ///   decode, shown at once) if it is nearer the target than the frame on screen, and
    ///   from there on as above;
    /// * otherwise the target.
    ///
    /// A step forward is taken only when it is nearer the target than the frame on screen.
    fn scrub_goal(&self, st: &State, target: u64) -> u64 {
        let nearer = |s: u64| {
            st.shown
                .is_none_or(|v| s.abs_diff(target) < v.abs_diff(target))
        };
        let cached = |s: u64| st.ring.frames.contains_key(&index_of(s, st.count));
        let key = st.key_seq(target);
        if st.back && key.is_some_and(|k| k < target) {
            let lo = key
                .unwrap_or(target)
                .max(target.saturating_sub(self.behind(st)));
            // Frames before one already kept are left for when the pointer gets there.
            let from = (lo..target)
                .rev()
                .find(|&s| cached(s))
                .map_or(lo, |c| c + 1);
            let mut g = if st.reaches(self.cursor, target) {
                self.cursor.max(from)
            } else {
                from
            };
            while g < target && cached(g) {
                g += 1;
            }
            return g;
        }
        if st.reaches(self.cursor, target) {
            let d = target - self.cursor;
            let step = self.cursor + d / 4;
            return if d > 8 && nearer(step) { step } else { target };
        }
        match key {
            Some(k) if k < target && !cached(k) && nearer(k) => k,
            _ => target,
        }
    }

    /// How many frames before a scrub target to keep when dragging backwards: half the ring
    /// (at least 1, at most 64).
    fn behind(&self, st: &State) -> u64 {
        let per = st.frame_bytes.max(1);
        (self.cfg.budget / 2 / per).clamp(1, 64) as u64
    }

    fn fail(&mut self, msg: String) {
        self.shared.state.lock().error = Some(msg);
    }

    fn wake(&self) {
        if let Some(w) = &self.waker {
            w();
        }
    }
}
