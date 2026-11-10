//! The playback state machine and scheduler.

use crate::FrameRef;
use crate::policy::{DelayPolicy, LoopMode, PlayerConfig, speed_q};
use crate::strip::{self, StripConfig, StripShared, StripSource, StripStats};
use crate::timeline::Timeline;
use crate::worker::{self, Shared, State, Waker, WorkerConfig};
use specular_core::CancelToken;
use specular_decode::{AnimationInfo, Frames, Session};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often to tick while waiting for a frame when there is no [`Waker`].
const POLL_WITHOUT_WAKER: Duration = Duration::from_millis(4);

/// A master clock to follow (a movie's sound): the media time being heard at `now`, from the
/// start of the track (within one loop), or `None` while it does not run (muted, no device,
/// another speed). Called from the UI thread on every tick, so it must be cheap.
pub type MediaClock = Arc<dyn Fn(Instant) -> Option<Duration> + Send + Sync>;

/// Differences from the master clock up to this are left alone (a frame at 120 fps); larger
/// ones move the schedule onto the clock.
const CLOCK_SLACK: Duration = Duration::from_millis(8);
/// Differences beyond this are taken as a clock that has not caught up with a seek yet, and
/// ignored.
const CLOCK_TRUST: Duration = Duration::from_secs(2);

/// What [`Player::tick`] decided.
#[derive(Clone, Debug, Default)]
pub struct TickResult {
    /// A different frame to show now (upload it and redraw). `None`: keep the current one.
    pub show: Option<FrameRef>,
    /// When to call `tick` again. `None`: nothing is scheduled (paused, finished, or waiting
    /// for the decoder, which calls the [`Waker`] when the frame arrives). A deadline in the
    /// past means "as soon as possible".
    pub next_deadline: Option<Instant>,
    /// Frames skipped by this tick because it came too late for them.
    pub dropped: u64,
    /// When the schedule wanted the frame in `show` (while playing): its start on the play
    /// clock, or `now` when this tick (re)started the schedule with it. For measuring how
    /// late frames reach the screen.
    pub due: Option<Instant>,
}

/// Where playback is, for the UI (frame counter, slider, play button).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    /// Frame on screen.
    pub index: Option<u32>,
    /// Frame being fetched for a seek or step.
    pub pending: Option<u32>,
    /// Frame count, once known.
    pub count: Option<u32>,
    /// Completed loops.
    pub lap: u64,
    pub playing: bool,
    /// Played the last loop and stopped on its last frame.
    pub finished: bool,
    pub speed: f64,
    /// Waiting for the decoder while the schedule says another frame is due.
    pub stalled: bool,
    /// Every frame is in memory; the decoder has been dropped.
    pub fully_cached: bool,
    /// Ping-pong is in effect (asked for, and the animation is fully cached).
    pub ping_pong: bool,
    /// The seek bar is being dragged ([`Player::scrub`]): `playing` says whether playback
    /// goes on when it is let go; nothing plays meanwhile.
    pub scrubbing: bool,
}

/// Counters for the HUD and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Frames handed to the UI.
    pub shown: u64,
    /// Frames skipped because their time had passed.
    pub dropped: u64,
    /// Times playback had to wait for the decoder.
    pub stalls: u64,
    /// Times the schedule was re-anchored because decoding could not keep up.
    pub resyncs: u64,
    pub decoded: u64,
    pub evicted: u64,
    /// Frames decoded only to advance the decoder (seeking) and not kept.
    pub discarded: u64,
    pub rewinds: u64,
    /// Times the schedule moved onto the master clock ([`Player::set_clock`]).
    pub clock_syncs: u64,
    pub cached_frames: usize,
    pub cached_bytes: usize,
    /// Frames shown while scrubbing in place of the one under the pointer, until it is
    /// decoded (the nearest decoded so far).
    pub interim: u64,
    /// Decodes stopped because the pointer moved on and their frame was no longer wanted.
    pub interrupted: u64,
    /// Of `interim`, frames from the scrub strip ([`Player::start_strip`]).
    pub strip_shown: u64,
}

/// The scrub strip's shared state and thread.
struct Strip {
    shared: Arc<StripShared>,
    thread: Option<JoinHandle<()>>,
}

/// Plays one animation. Owns a decode-ahead thread, which is cancelled when the player drops.
pub struct Player {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    cancel: CancelToken,
    cfg: PlayerConfig,
    file_loops: u32,
    has_waker: bool,
    tl: Timeline,
    speed_q: u64,
    playing: bool,
    finished: bool,
    /// Sequence number and frame on screen.
    shown: Option<(u64, FrameRef)>,
    /// Sequence number to show as soon as it is decoded (seek, step, first frame).
    pending: Option<u64>,
    /// The schedule: play time `base_t` (ns) was at `base_at`.
    base_t: u64,
    base_at: Option<Instant>,
    /// Play time at which playback was paused, to resume mid-frame.
    paused_t: Option<u64>,
    stalled: bool,
    complete: bool,
    exited: bool,
    error: Option<String>,
    stats: Stats,
    clock: Option<MediaClock>,
    /// The master clock ran at the last tick (the schedule follows it, not decode speed).
    clock_live: bool,
    /// The seek bar is being dragged ([`Player::scrub`]).
    scrubbing: bool,
    /// Playback goes on when the drag ends.
    resume: bool,
    /// The drag's last move was backwards.
    back: bool,
    /// The pending frame is a seek's (or the drag let go): a strip frame near it shows
    /// meanwhile.
    landing: bool,
    strip: Option<Strip>,
}

impl Player {
    /// Start playing `frames`. The worker thread starts decoding immediately; frame 0 shows on
    /// the first [`Player::tick`] after it is decoded.
    ///
    /// `waker` is called from the worker thread when a frame the player waits for arrives; the
    /// app should respond by calling `tick` (e.g. send an event-loop user event).
    pub fn new(
        frames: Box<dyn Frames>,
        info: &AnimationInfo,
        cfg: PlayerConfig,
        waker: Option<Waker>,
    ) -> Player {
        let count = info.frame_count.filter(|&n| n > 0);
        let shared = Shared::new(count);
        let cancel = CancelToken::new();
        let has_waker = waker.is_some();
        let thread = worker::spawn(
            Arc::clone(&shared),
            frames,
            cancel.clone(),
            WorkerConfig {
                budget: cfg.budget_bytes,
                max_ahead: cfg.max_ahead,
            },
            waker,
        );
        let tl = Timeline::new(count, cfg.delay_policy);
        Player {
            shared,
            thread,
            cancel,
            speed_q: speed_q(cfg.speed),
            playing: cfg.autoplay,
            file_loops: info.loop_count,
            has_waker,
            tl,
            cfg,
            finished: false,
            shown: None,
            pending: Some(0),
            base_t: 0,
            base_at: None,
            paused_t: None,
            stalled: false,
            complete: false,
            exited: false,
            error: None,
            stats: Stats::default(),
            clock: None,
            clock_live: false,
            scrubbing: false,
            resume: false,
            back: false,
            landing: false,
            strip: None,
        }
    }

    /// Starts the scrub strip (see `src/strip.rs`): small copies of frames spread over the
    /// movie, decoded in the background on a decoder `open` makes (on the strip's thread)
    /// once the first frame shows. While the seek bar is dragged, and while a seek's frame
    /// decodes, the strip frame nearest the pointer shows until the full frame is there.
    /// Its memory comes from this thread's budget. Only for seekable decoders that know their
    /// frame times (movies); once per player.
    pub fn start_strip(&mut self, open: StripSource, cfg: StripConfig) {
        if self.strip.is_some() {
            return;
        }
        let shared = StripShared::new();
        let thread = strip::spawn(
            Arc::clone(&shared),
            open,
            cfg,
            self.cancel.clone(),
            specular_core::current_budget(),
        );
        self.strip = Some(Strip { shared, thread });
        self.steer_strip();
    }

    /// What the scrub strip holds and has done (`None`: no strip).
    pub fn strip_stats(&self) -> Option<StripStats> {
        self.strip.as_ref().map(|s| s.shared.stats())
    }

    /// The strip frame nearest frame `index`, if it is within `within` frames of it (the
    /// seek bar's hover preview).
    pub fn strip_frame(&self, index: u32, within: u32) -> Option<FrameRef> {
        let f = self.strip.as_ref()?.shared.nearest(index)?;
        (f.index.abs_diff(index) <= within).then_some(f)
    }

    /// Blocks until the strip has nothing to do now (complete, stopped, or held); false on
    /// timeout or without a strip. For tests and benchmarks.
    pub fn wait_strip(&self, timeout: Duration) -> bool {
        self.strip.as_ref().is_some_and(|s| s.shared.wait(timeout))
    }

    /// Follow `clock` while it runs (audio is the master clock of a movie): each tick while
    /// playing, a schedule more than a few milliseconds off the clock moves onto it, so video
    /// never drifts from the sound however their clocks differ. Late frames are then dropped
    /// rather than playback slowing to decode speed. `None` plays by the wall clock.
    pub fn set_clock(&mut self, clock: Option<MediaClock>) {
        self.clock = clock;
        self.clock_live = false;
    }

    /// Length of one loop once every frame's time is known (always, for movies).
    pub fn duration(&self) -> Option<Duration> {
        (self.tl.closed() && !self.tl.ping_pong()).then(|| Duration::from_nanos(self.tl.cycle_ns()))
    }

    /// Where in the loop playback is at `now` (the time readout and seek bar): the frame
    /// being fetched for a seek, else the play clock while playing, else the frame on screen.
    pub fn time(&self, now: Instant) -> Option<Duration> {
        let t = if let Some(p) = self.pending {
            self.tl.start_of(p)?
        } else if self.playing && self.base_at.is_some() {
            self.play_t(now)
        } else if let Some(t) = self.paused_t {
            t
        } else {
            self.tl.start_of(self.shown_seq()?)?
        };
        let cycle = self.tl.cycle_ns();
        let t = if self.tl.closed() && cycle > 0 {
            t % cycle
        } else {
            t
        };
        Some(Duration::from_nanos(t))
    }

    /// The frame shown at time `t` into the loop (for seeking by time; clamped to the last).
    pub fn index_at(&self, t: Duration) -> Option<u32> {
        if !self.tl.closed() {
            return None;
        }
        let cycle = self.tl.cycle_ns();
        let s = self.tl.seq_at(dur_ns(t).min(cycle.saturating_sub(1)));
        Some(self.tl.idx(s.min(self.tl.cycle_len().saturating_sub(1))))
    }

    /// A player for an animated session at 1/`scale`; `Ok(None)` when it is not animated.
    pub fn from_session(
        session: &dyn Session,
        scale: u32,
        cfg: PlayerConfig,
        waker: Option<Waker>,
    ) -> specular_decode::Result<Option<Player>> {
        let Some(info) = session.animation() else {
            return Ok(None);
        };
        let frames = session.frames(scale)?;
        Ok(Some(Player::new(frames, info, cfg, waker)))
    }

    /// Advance playback to `now`. Call when the previous result's `next_deadline` passes,
    /// when the [`Waker`] fires, and after any control call (play, step, seek...). Cheap when
    /// nothing changed.
    pub fn tick(&mut self, now: Instant) -> TickResult {
        let mut res = TickResult::default();
        let shared = Arc::clone(&self.shared);
        let mut st = shared.state.lock();
        self.sync(&st, now);
        let mut waiting = false;
        if let Some(p) = self.pending {
            if let Some(f) = st.ring.frames.get(&self.tl.idx(p)).cloned() {
                self.pending = None;
                self.landing = false;
                self.stalled = false;
                self.show(p, f, &mut res);
                if self.playing {
                    self.anchor(p, now);
                    res.due = Some(now);
                }
            } else {
                if (self.scrubbing || self.landing)
                    && let Some((s, f)) = self.interim(&st, p)
                {
                    self.stats.interim += 1;
                    self.stats.strip_shown += u64::from(f.strip);
                    self.show(s, f, &mut res);
                }
                waiting = true;
            }
        } else if self.playing
            && let Some(s) = self.shown_seq()
        {
            self.follow_clock(now);
            let t = self.play_t(now);
            let (target, last) = self.target_at(t);
            if target > s {
                match self.best_available(&st, s, target) {
                    Some((b, f)) => {
                        res.dropped = b - s - 1;
                        self.stats.dropped += res.dropped;
                        let due = self.tl.start_of(b).unwrap_or(t);
                        if !self.clock_live
                            && b < target
                            && t.saturating_sub(due) > dur_ns(self.cfg.max_lag)
                        {
                            // Decoding cannot keep up: play at its pace rather than skip on.
                            self.anchor(b, now);
                            self.stats.resyncs += 1;
                        }
                        res.due = Some(self.instant_of(due));
                        self.stalled = false;
                        self.show(b, f, &mut res);
                        if last && b == target {
                            self.finish();
                        }
                    }
                    None => {
                        waiting = true;
                        if !self.stalled {
                            self.stalled = true;
                            self.stats.stalls += 1;
                        }
                    }
                }
            } else if last {
                self.finish();
            }
        }
        if waiting && st.exited {
            // Nothing more will arrive (decode error).
            waiting = false;
        }
        if waiting {
            st.waiting = true;
        }
        let notify = self.publish(&mut st);
        drop(st);
        if notify {
            shared.work.notify_one();
        }
        self.steer_strip();

        if waiting {
            if !self.has_waker {
                res.next_deadline = Some(now + POLL_WITHOUT_WAKER);
            }
        } else if self.playing
            && self.pending.is_none()
            && !(self.complete && self.tl.count() == Some(1))
            && let Some(s) = self.shown_seq()
            && let Some(t) = self.tl.start_of(s + 1)
        {
            res.next_deadline = Some(self.instant_of(t));
        }
        res
    }

    pub fn play(&mut self, now: Instant) {
        if self.scrubbing {
            // Plays once the drag ends.
            self.resume = true;
            return;
        }
        if self.finished {
            self.finished = false;
            self.paused_t = None;
            self.pending = Some(0);
        }
        if !self.playing {
            self.playing = true;
            if self.pending.is_none()
                && let Some(s) = self.shown_seq()
            {
                let t = self
                    .paused_t
                    .take()
                    .unwrap_or_else(|| self.tl.start_of(s).unwrap_or(0));
                self.base_t = t;
                self.base_at = Some(now);
            }
        }
        self.publish_now();
    }

    pub fn pause(&mut self, now: Instant) {
        if self.scrubbing {
            self.resume = false;
            return;
        }
        if !self.playing {
            return;
        }
        self.playing = false;
        self.stalled = false;
        if self.pending.is_none()
            && let Some(s) = self.shown_seq()
            && let Some(start) = self.tl.start_of(s)
        {
            let end = self.tl.start_of(s + 1).unwrap_or(start + 1).max(start + 1);
            self.paused_t = Some(self.play_t(now).clamp(start, end - 1));
        }
    }

    pub fn toggle(&mut self, now: Instant) {
        if self.playing || self.scrubbing && self.resume {
            self.pause(now);
        } else {
            self.play(now);
        }
    }

    /// Pause and show the next frame (wrapping to frame 0 after the last).
    pub fn step_forward(&mut self) {
        let Some(s) = self.pending.or(self.shown_seq()) else {
            return;
        };
        self.stop_for_step();
        let mut next = s + 1;
        if self.tl.closed() {
            if self.end_seq().is_some_and(|e| next >= e) {
                next %= self.tl.cycle_len();
            }
        } else if next > self.tl.known() as u64 {
            return;
        }
        self.pending = Some(next);
        self.publish_now();
    }

    /// Pause and show the previous frame (wrapping to the last once the count is known).
    /// Cached frames show at once; otherwise the decoder rewinds and replays up to it.
    pub fn step_back(&mut self) {
        let Some(s) = self.pending.or(self.shown_seq()) else {
            return;
        };
        self.stop_for_step();
        let prev = if s > 0 {
            s - 1
        } else if self.tl.closed() {
            self.tl.cycle_len() - 1
        } else {
            return;
        };
        self.pending = Some(prev);
        self.publish_now();
    }

    /// The seek bar is dragged to frame `index`: it shows as soon as it is decoded, and
    /// meanwhile the decoded frame nearest it (each tick takes the nearest so far). Playback
    /// pauses while dragging; [`Player::seek`] ends the drag on the exact frame and plays on
    /// from it if it was playing. Decodes the pointer has left behind are stopped (when the
    /// decoder can stop between samples), and long-GOP video is decoded toward the pointer
    /// in steps, from the sync frame when it has to start over (the worker's `scrub_goal`).
    pub fn scrub(&mut self, index: u32) {
        if !self.scrubbing {
            self.scrubbing = true;
            self.resume = self.playing && !self.finished;
            self.playing = false;
            self.stalled = false;
        }
        self.landing = false;
        let at = self.pending.or(self.shown_seq()).map(|s| self.tl.idx(s));
        if let Some(at) = at.filter(|&a| a != index) {
            self.back = index < at;
        }
        self.place(index);
    }

    /// Whether the seek bar is being dragged.
    pub fn scrubbing(&self) -> bool {
        self.scrubbing
    }

    /// Show frame `index` (clamped to the known frames), keeping the play state. Playback
    /// continues from it once it is decoded. Ends a drag of the seek bar ([`Player::scrub`]).
    pub fn seek(&mut self, index: u32) {
        if self.scrubbing {
            self.scrubbing = false;
            self.playing = std::mem::take(&mut self.resume);
        }
        self.landing = true;
        self.place(index);
    }

    /// The frame to show next is `index`.
    fn place(&mut self, index: u32) {
        let cur = self.pending.or(self.shown_seq()).unwrap_or(0);
        let s = if self.tl.closed() {
            let index = index.min(self.tl.count().unwrap_or(1) - 1);
            self.tl.lap(cur) * self.tl.cycle_len() + self.tl.pos_of(index)
        } else {
            u64::from(index).min(self.tl.known() as u64)
        };
        self.finished = false;
        self.paused_t = None;
        self.pending = Some(s);
        self.publish_now();
    }

    /// Speed multiplier, clamped to 0.25–4. Takes effect from `now` without a jump.
    pub fn set_speed(&mut self, speed: f64, now: Instant) {
        if self.playing && self.base_at.is_some() {
            self.base_t = self.play_t(now);
            self.base_at = Some(now);
        }
        self.speed_q = speed_q(speed);
        self.cfg.speed = self.speed_q as f64 / 1024.0;
    }

    pub fn speed(&self) -> f64 {
        self.speed_q as f64 / 1024.0
    }

    pub fn set_loop_mode(&mut self, mode: LoopMode) {
        // A finished animation stays finished; `play` restarts it under the new mode.
        self.cfg.loop_mode = mode;
        self.publish_now();
    }

    pub fn set_ping_pong(&mut self, on: bool, now: Instant) {
        self.cfg.ping_pong = on;
        self.reconfigure(now);
        self.publish_now();
    }

    pub fn set_delay_policy(&mut self, policy: DelayPolicy, now: Instant) {
        self.cfg.delay_policy = policy;
        self.reconfigure(now);
    }

    pub fn config(&self) -> &PlayerConfig {
        &self.cfg
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// The frame on screen.
    pub fn current(&self) -> Option<&FrameRef> {
        self.shown.as_ref().map(|(_, f)| f)
    }

    /// A frame if it is in memory (scrubbing previews, filmstrip).
    pub fn cached(&self, index: u32) -> Option<FrameRef> {
        self.shared.state.lock().ring.frames.get(&index).cloned()
    }

    pub fn frame_count(&self) -> Option<u32> {
        self.tl.count().or(self.shared.state.lock().count)
    }

    pub fn position(&self) -> Position {
        Position {
            index: self.shown_seq().map(|s| self.tl.idx(s)),
            pending: self.pending.map(|s| self.tl.idx(s)),
            count: self.frame_count(),
            lap: self.shown_seq().map_or(0, |s| self.tl.lap(s)),
            playing: self.playing || self.resume,
            finished: self.finished,
            speed: self.speed(),
            stalled: self.stalled,
            fully_cached: self.complete,
            ping_pong: self.tl.ping_pong(),
            scrubbing: self.scrubbing,
        }
    }

    pub fn stats(&self) -> Stats {
        let st = self.shared.state.lock();
        Stats {
            decoded: st.decoded,
            evicted: st.evicted,
            discarded: st.discarded,
            rewinds: st.rewinds,
            interrupted: st.interrupted,
            cached_frames: st.ring.frames.len(),
            cached_bytes: st.ring.bytes,
            ..self.stats
        }
    }

    /// Why the animation cannot play (or stopped early), if it failed.
    pub fn error(&self) -> Option<String> {
        self.error
            .clone()
            .or_else(|| self.shared.state.lock().error.clone())
    }

    /// How much free RAM a frame needed when that is why playback stopped.
    pub fn needs_memory(&self) -> Option<specular_core::NeedsMemory> {
        self.shared.state.lock().memory
    }

    /// Block until the decode-ahead worker has nothing to do (window full, fully cached,
    /// or ended). For tests and benchmarks; the UI never needs to wait. False on timeout.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut st = self.shared.state.lock();
        while !(st.idle || st.exited) {
            if self.shared.idle.wait_until(&mut st, deadline).timed_out() {
                return st.idle || st.exited;
            }
        }
        true
    }

    /// Cancel the worker and wait for it to end (normally dropping is enough: the worker
    /// stops after the frame it is decoding, without blocking the UI thread).
    pub fn shutdown(mut self) {
        self.stop_worker();
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
        if let Some(h) = self.strip.as_mut().and_then(|s| s.thread.take()) {
            let _ = h.join();
        }
    }

    // --- internals ---

    fn stop_worker(&self) {
        self.cancel.cancel();
        {
            let _guard = self.shared.state.lock();
            self.shared.work.notify_all();
        }
        if let Some(s) = &self.strip {
            s.shared.stop();
        }
    }

    fn shown_seq(&self) -> Option<u64> {
        self.shown.as_ref().map(|(s, _)| *s)
    }

    fn show(&mut self, s: u64, f: FrameRef, res: &mut TickResult) {
        let same = self.shown.as_ref().is_some_and(|(_, g)| Arc::ptr_eq(g, &f));
        if !same {
            res.show = Some(Arc::clone(&f));
            self.stats.shown += 1;
        }
        self.shown = Some((s, f));
    }

    fn finish(&mut self) {
        self.finished = true;
        self.playing = false;
        self.paused_t = None;
    }

    fn stop_for_step(&mut self) {
        self.scrubbing = false;
        self.landing = false;
        self.resume = false;
        self.playing = false;
        self.finished = false;
        self.stalled = false;
        self.paused_t = None;
    }

    /// Move the schedule onto the master clock when it is off by more than [`CLOCK_SLACK`].
    fn follow_clock(&mut self, now: Instant) {
        self.clock_live = false;
        let Some(clock) = &self.clock else {
            return;
        };
        let cycle = self.tl.cycle_ns();
        if self.base_at.is_none() || !self.tl.closed() || self.tl.ping_pong() || cycle == 0 {
            return;
        }
        let Some(heard) = clock(now) else {
            return;
        };
        self.clock_live = true;
        let t = self.play_t(now);
        let m = dur_ns(heard).min(cycle);
        // The clock knows the time within a loop; the schedule counts loops. Take the
        // reading nearest the schedule (a loop boundary may fall between the two).
        let lap = t / cycle * cycle + m;
        let want = [Some(lap), lap.checked_add(cycle), lap.checked_sub(cycle)]
            .into_iter()
            .flatten()
            .min_by_key(|c| c.abs_diff(t))
            .unwrap_or(lap);
        let off = want.abs_diff(t);
        if off > dur_ns(CLOCK_SLACK) && off <= dur_ns(CLOCK_TRUST) {
            self.base_t = want;
            self.base_at = Some(now);
            self.stats.clock_syncs += 1;
        }
    }

    fn anchor(&mut self, s: u64, now: Instant) {
        self.base_t = self.tl.start_of(s).unwrap_or(0);
        self.base_at = Some(now);
    }

    /// Play time (ns) at `now`.
    fn play_t(&self, now: Instant) -> u64 {
        let Some(at) = self.base_at else {
            return self.base_t;
        };
        let real = now.saturating_duration_since(at).as_nanos();
        let scaled = real * u128::from(self.speed_q) / 1024;
        self.base_t
            .saturating_add(u64::try_from(scaled).unwrap_or(u64::MAX))
    }

    /// When play time `t` (ns) is reached; never early.
    fn instant_of(&self, t: u64) -> Instant {
        let at = self.base_at.unwrap_or_else(Instant::now);
        let dt = u128::from(t.saturating_sub(self.base_t));
        let real = (dt * 1024).div_ceil(u128::from(self.speed_q));
        at + Duration::from_nanos(u64::try_from(real).unwrap_or(u64::MAX))
    }

    /// One past the last sequence number to play, for finite loop counts.
    fn end_seq(&self) -> Option<u64> {
        if !self.tl.closed() {
            return None;
        }
        let plays = self.cfg.loop_mode.plays(self.file_loops)?;
        // Ping-pong ends where it started.
        let extra = u64::from(self.tl.ping_pong());
        Some(plays * self.tl.cycle_len() + extra)
    }

    /// The sequence number due at play time `t`, and whether it is the very last one.
    fn target_at(&self, t: u64) -> (u64, bool) {
        let s = self.tl.seq_at(t);
        match self.end_seq() {
            Some(end) if s + 1 >= end => (end - 1, true),
            _ => (s, false),
        }
    }

    /// The latest sequence number in `after+1..=target` whose frame is in the ring.
    fn best_available(&self, st: &State, after: u64, target: u64) -> Option<(u64, FrameRef)> {
        let span = target - after;
        let span = if self.tl.closed() {
            span.min(self.tl.cycle_len())
        } else {
            span
        };
        (target + 1 - span..=target).rev().find_map(|q| {
            st.ring
                .frames
                .get(&self.tl.idx(q))
                .map(|f| (q, Arc::clone(f)))
        })
    }

    fn sync(&mut self, st: &State, now: Instant) {
        if st.delays.len() != self.tl.known() || st.count != self.tl.count() {
            self.tl.sync(&st.delays, st.count);
        }
        self.complete = st.complete;
        self.exited = st.exited;
        if self.error.is_none() && st.error.is_some() {
            self.error.clone_from(&st.error);
        }
        if (self.cfg.ping_pong && self.complete) != self.tl.ping_pong() {
            self.reconfigure(now);
        }
    }

    /// Rebuild the timeline after a ping-pong or delay-policy change, keeping the frame on
    /// screen, its loop and how far into it playback is.
    fn reconfigure(&mut self, now: Instant) {
        let ping_pong = self.cfg.ping_pong && self.complete;
        let running = self.playing && self.pending.is_none() && self.base_at.is_some();
        let shown = self.shown_seq();
        let offset = match shown {
            Some(s) if running => self
                .play_t(now)
                .saturating_sub(self.tl.start_of(s).unwrap_or(0)),
            _ => 0,
        };
        let old_shown = shown.map(|s| (self.tl.idx(s), self.tl.lap(s)));
        let old_pending = self.pending.map(|s| (self.tl.idx(s), self.tl.lap(s)));
        self.tl.configure(ping_pong, self.cfg.delay_policy);
        if let (Some((s, _)), Some((i, lap))) = (self.shown.as_mut(), old_shown) {
            *s = self.tl.remap(i, lap);
        }
        if let Some((i, lap)) = old_pending {
            self.pending = Some(self.tl.remap(i, lap));
        }
        self.paused_t = None;
        if running && let Some(s) = self.shown_seq() {
            let start = self.tl.start_of(s).unwrap_or(0);
            let end = self.tl.start_of(s + 1).unwrap_or(start + 1).max(start + 1);
            self.base_t = (start + offset).min(end - 1);
            self.base_at = Some(now);
        }
    }

    /// While the frame at sequence number `p` decodes: the frame to show meanwhile, when it
    /// is nearer `p` than the frame on screen. While scrubbing the nearest decoded frame or
    /// strip frame (a full frame wins a tie, so the strip frame on screen hands over to the
    /// full one as soon as it is decoded); after a seek, only a strip frame.
    fn interim(&self, st: &State, p: u64) -> Option<(u64, FrameRef)> {
        let i = self.tl.idx(p);
        let base = p - u64::from(i);
        let full = self.scrubbing.then(|| {
            let below = st.ring.frames.range(..=i).next_back();
            let above = st.ring.frames.range(i..).next();
            [below, above]
                .into_iter()
                .flatten()
                .min_by_key(|(k, _)| k.abs_diff(i))
                .map(|(_, f)| Arc::clone(f))
        });
        let small = self.strip.as_ref().and_then(|s| s.shared.nearest(i));
        // Distance in frames, a strip frame half a frame further.
        let cost = |d: u64, f: &FrameRef| d.saturating_mul(2) + u64::from(f.strip);
        let best = [full.flatten(), small]
            .into_iter()
            .flatten()
            .min_by_key(|f| cost(u64::from(f.index.abs_diff(i)), f))?;
        let s = base + u64::from(best.index);
        let better = self
            .shown
            .as_ref()
            .is_none_or(|(v, f)| cost(s.abs_diff(p), &best) < cost(v.abs_diff(p), f));
        better.then_some((s, best))
    }

    /// Tells the strip where the player is, and to hold while an exact frame is awaited (a
    /// seek, a step, the drag let go, or playback waiting for the decoder).
    fn steer_strip(&self) {
        let Some(strip) = &self.strip else {
            return;
        };
        let focus = self
            .pending
            .or(self.shown_seq())
            .map_or(0, |s| self.tl.idx(s));
        let go = self.shown.is_some();
        let hold =
            go && (self.pending.is_some() && !self.scrubbing || self.playing && self.stalled);
        strip.shared.steer(focus, go, hold);
    }

    /// Tell the worker where the playhead is. True when it should be woken. A decode in
    /// progress that does not lead to a frame now wanted (a seek or the pointer moved
    /// elsewhere) is stopped, when the decoder can stop between samples.
    fn publish(&self, st: &mut State) -> bool {
        if self.tl.ping_pong() {
            return false; // Fully cached: the worker has ended.
        }
        let pos = self.pending.or(self.shown_seq()).unwrap_or(0);
        let limit = self.end_seq();
        st.shown = self.shown_seq();
        if st.pos != pos || st.limit != limit || st.scrub != self.scrubbing {
            st.pos = pos;
            st.limit = limit;
            st.scrub = self.scrubbing;
            st.back = self.scrubbing && self.back;
            st.idle = st.exited;
            if st.interruptible
                && self.pending.is_some()
                && let Some(b) = st.busy
                && !st.ring.frames.contains_key(&self.tl.idx(pos))
                && !st.reaches(b, pos)
            {
                self.shared.interrupt.store(true, Ordering::Release);
            }
            return true;
        }
        false
    }

    fn publish_now(&self) {
        let mut st = self.shared.state.lock();
        let notify = self.publish(&mut st);
        drop(st);
        if notify {
            self.shared.work.notify_one();
        }
        self.steer_strip();
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop_worker();
    }
}

fn dur_ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}
