//! The scrub strip: small copies of frames spread over a movie, decoded in the background
//! on a decoder of their own, so dragging the seek bar shows a frame near the pointer at once
//! (scaled up) while the full frame is decoded.
//!
//! * **What it holds** ([`layout`]): a frame every [`StripConfig::spacing`] of the movie (at
//!   most every [`StripConfig::max_spacing`]), each snapped to a sync frame within half that
//!   when there is one (a sync frame costs one decode). For long-GOP video that is every sync
//!   frame plus frames between them. Frames are [`StripConfig::height`] tall (the shown size,
//!   480 px at most), in 8-bit YUV 4:2:0 for video. When they don't fit the memory budget,
//!   the strip first takes every [`StripConfig::max_spacing`], then smaller frames (down to
//!   [`StripConfig::min_height`]), then fewer.
//! * **Order** ([`pick`]): sync frames first, nearest the place of interest (the pointer while
//!   dragging, else the playhead); then the frames between them, a GOP at a time from its
//!   sync frame (the decoder goes on rather than starting over), the GOP nearest the place
//!   of interest first. Frames between sync frames are decoded only while decoding runs at
//!   [`StripConfig::min_speed`] of real time or faster (timed on those frames, so the first
//!   is always tried); otherwise the strip stops at the sync frames.
//! * **When**: only after the player has shown its first frame, at low priority (the
//!   [`StripSource`] lowers its thread's priority before opening the decoder, so the
//!   decoder's own threads inherit it), never while the player waits for an exact frame (a
//!   seek, a step, the drag let go, playback stalling): then the decode in progress stops
//!   between two samples. The strip's thread ends, dropping its decoder, once the strip is
//!   complete, and when the player drops.
//! * **Memory**: the frames are reserved from the memory budget of the thread that started
//!   the strip, within [`StripConfig::budget_bytes`]; a refusal ends the strip where it is.

use crate::{AnimFrame, FrameRef};
use parking_lot::{Condvar, Mutex};
use specular_core::{
    BudgetHandle, CancelToken, PixelLayout, Raster, RasterPlane, Reservation, YuvEncoding,
};
use specular_decode::{DecodeError, Frames};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Opens the strip's own decoder of the movie. Called on the strip's thread, so it can lower
/// that thread's priority first (the decoder's worker threads then inherit it).
pub type StripSource = Box<dyn FnOnce() -> specular_decode::Result<Box<dyn Frames>> + Send>;

/// The tallest strip frame.
pub const STRIP_MAX_HEIGHT: u32 = 480;

/// How the scrub strip is made ([`crate::Player::start_strip`]).
#[derive(Clone, Debug)]
pub struct StripConfig {
    /// Height of a strip frame in pixels: about the movie's shown height, at most
    /// [`STRIP_MAX_HEIGHT`] (never more than the movie's).
    pub height: u32,
    /// The smallest height the strip goes down to to fit its budget.
    pub min_height: u32,
    /// Most bytes the strip's frames take (also bounded by the memory budget).
    pub budget_bytes: usize,
    /// A frame every this much of the movie...
    pub spacing: Duration,
    /// ... or at least every this much while the budget allows.
    pub max_spacing: Duration,
    /// Frames between sync frames are decoded only while decoding keeps up with this
    /// fraction of real time (0: always).
    pub min_speed: f64,
}

impl Default for StripConfig {
    fn default() -> Self {
        StripConfig {
            height: STRIP_MAX_HEIGHT,
            min_height: 144,
            budget_bytes: 128 << 20,
            spacing: Duration::from_millis(250),
            max_spacing: Duration::from_millis(500),
            min_speed: 0.25,
        }
    }
}

/// What the strip has done so far ([`crate::Player::strip_stats`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StripStats {
    /// Frames held, and their bytes.
    pub frames: usize,
    pub bytes: usize,
    /// Frames the strip means to hold ([`layout`]).
    pub planned: usize,
    /// Size of a strip frame.
    pub width: u32,
    pub height: u32,
    /// Frames between the sync frames are being decoded (decoding is fast enough).
    pub between: bool,
    /// Nothing more to do: the strip is complete, or stopped (memory, a decoder that cannot
    /// seek, slow decoding past the sync frames, an error).
    pub done: bool,
    /// Frames decoded into the strip, and decodes stopped to make way for the player.
    pub decoded: u64,
    pub interrupted: u64,
}

#[derive(Default)]
pub(crate) struct StripState {
    pub frames: BTreeMap<u32, FrameRef>,
    pub bytes: usize,
    /// The frame the pointer or playhead is on.
    pub focus: u32,
    /// The player waits for an exact frame: no strip work.
    pub hold: bool,
    /// The player has shown its first frame: the strip may start.
    pub go: bool,
    pub stats: StripStats,
    /// The strip's thread has ended.
    pub exited: bool,
    /// Waiting (held, or before the first frame).
    pub idle: bool,
    /// The frames' memory, taken from the budget (given back when the strip drops).
    pub reservation: Option<Reservation>,
}

pub(crate) struct StripShared {
    pub state: Mutex<StripState>,
    pub work: Condvar,
    pub idle: Condvar,
    /// Stops the strip's decode in progress (set by the player when it holds the strip).
    pub interrupt: Arc<AtomicBool>,
}

impl StripShared {
    pub fn new() -> Arc<StripShared> {
        Arc::new(StripShared {
            state: Mutex::new(StripState::default()),
            work: Condvar::new(),
            idle: Condvar::new(),
            interrupt: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Where the player is (`focus`), whether it has shown a frame (`go`) and whether it
    /// waits for an exact frame (`hold`: the strip's decode stops at once).
    pub fn steer(&self, focus: u32, go: bool, hold: bool) {
        let mut st = self.state.lock();
        if st.exited || (st.focus == focus && st.go == go && st.hold == hold) {
            return;
        }
        if hold && !st.hold {
            self.interrupt.store(true, Ordering::Release);
        }
        st.focus = focus;
        st.go |= go;
        st.hold = hold;
        drop(st);
        self.work.notify_all();
    }

    /// The strip frame nearest frame `i`.
    pub fn nearest(&self, i: u32) -> Option<FrameRef> {
        let st = self.state.lock();
        let below = st.frames.range(..=i).next_back();
        let above = st.frames.range(i..).next();
        [below, above]
            .into_iter()
            .flatten()
            .min_by_key(|(k, _)| k.abs_diff(i))
            .map(|(_, f)| Arc::clone(f))
    }

    pub fn stats(&self) -> StripStats {
        let st = self.state.lock();
        StripStats {
            frames: st.frames.len(),
            bytes: st.bytes,
            ..st.stats.clone()
        }
    }

    /// Blocks until the strip has nothing to do now (done, held, or not started); false on
    /// timeout.
    pub fn wait(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut st = self.state.lock();
        while !(st.exited || st.idle) {
            if self.idle.wait_until(&mut st, deadline).timed_out() {
                return st.exited || st.idle;
            }
        }
        true
    }

    /// Wakes the thread to end it (the player's cancel token is set).
    pub fn stop(&self) {
        let _g = self.state.lock();
        self.interrupt.store(true, Ordering::Release);
        self.work.notify_all();
    }
}

/// What the strip holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Frame indices, ascending.
    pub members: Vec<u32>,
    pub width: u32,
    pub height: u32,
    /// The spacing taken, in nanoseconds of movie.
    pub spacing_ns: u64,
}

/// The kind of pixels a strip frame of a source raster has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripKind {
    /// 8-bit YUV 4:2:0.
    Yuv420,
    /// 8-bit planes as the source's (Gray, RGB, RGBA...): bytes per pixel of each plane and
    /// its chroma divisors.
    Planes([(u8, u8, u8); 4], usize),
}

impl StripKind {
    /// The strip kind for `r`, if the strip can hold it.
    pub fn of(r: &Raster) -> Option<StripKind> {
        match r.layout {
            PixelLayout::Yuv(_) if r.planes.len() == 3 => Some(StripKind::Yuv420),
            PixelLayout::Gray | PixelLayout::YCbCr | PixelLayout::Rgb | PixelLayout::Rgba
                if (1..=4).contains(&r.planes.len()) =>
            {
                let mut p = [(0u8, 1u8, 1u8); 4];
                for (k, pl) in r.planes.iter().enumerate() {
                    p[k] = (pl.bytes_per_pixel() as u8, pl.h_div.max(1), pl.v_div.max(1));
                }
                Some(StripKind::Planes(p, r.planes.len()))
            }
            _ => None,
        }
    }

    /// Bytes of a `w`×`h` strip frame.
    pub fn bytes(self, w: u32, h: u32) -> usize {
        let (w, h) = (w as usize, h as usize);
        match self {
            StripKind::Yuv420 => w * h + 2 * w.div_ceil(2) * h.div_ceil(2),
            StripKind::Planes(p, n) => p[..n]
                .iter()
                .map(|&(b, hd, vd)| {
                    w.div_ceil(usize::from(hd)) * h.div_ceil(usize::from(vd)) * usize::from(b)
                })
                .sum(),
        }
    }
}

/// The width of a strip frame `h` tall of a `sw`×`sh` source (same shape).
fn width_for(sw: u32, sh: u32, h: u32) -> u32 {
    let w = (u64::from(sw) * u64::from(h) + u64::from(sh) / 2) / u64::from(sh.max(1));
    (w as u32).clamp(1, sw.max(1))
}

/// The frame shown at `t` ns (`starts`: each frame's start, ascending from 0).
fn frame_at(starts: &[u64], t: u64) -> u32 {
    starts.partition_point(|&s| s <= t).saturating_sub(1) as u32
}

/// One frame per `spacing` ns of a movie whose frames start at `starts` (`total` ns long),
/// each snapped to the sync frame nearest it when one is within half the spacing.
pub fn members(starts: &[u64], total: u64, sync: &[u32], spacing: u64) -> Vec<u32> {
    let spacing = spacing.max(1);
    let half = spacing / 2;
    let start_of = |k: u32| starts.get(k as usize).copied().unwrap_or(u64::MAX);
    let mut out = Vec::new();
    let mut g = 0u64;
    while g < total.max(1) {
        let i = frame_at(starts, g);
        let p = sync.partition_point(|&k| start_of(k) <= g);
        let snap = [p.checked_sub(1).map(|q| sync[q]), sync.get(p).copied()]
            .into_iter()
            .flatten()
            .filter(|&k| (k as usize) < starts.len() && start_of(k).abs_diff(g) <= half)
            .min_by_key(|&k| start_of(k).abs_diff(g));
        out.push(snap.unwrap_or(i));
        g = g.saturating_add(spacing);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// What the strip of a movie (frame starts `starts`, `total` ns, sync frames `sync`) holds,
/// for frames `src` large of kind `kind`, within `budget` bytes (see the module docs).
pub fn layout(
    starts: &[u64],
    total: u64,
    sync: &[u32],
    src: (u32, u32),
    kind: StripKind,
    budget: usize,
    cfg: &StripConfig,
) -> Layout {
    let (sw, sh) = (src.0.max(1), src.1.max(1));
    let top = cfg.height.clamp(1, STRIP_MAX_HEIGHT).min(sh);
    let floor = cfg.min_height.clamp(1, top);
    let bytes = |h: u32| kind.bytes(width_for(sw, sh, h), h).max(1);
    let ns = |d: Duration| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX).max(1);
    let (near, far) = (ns(cfg.spacing), ns(cfg.max_spacing).max(ns(cfg.spacing)));
    let done = |members: Vec<u32>, h: u32, spacing_ns: u64| Layout {
        members,
        width: width_for(sw, sh, h),
        height: h,
        spacing_ns,
    };
    let dense = members(starts, total, sync, near);
    if dense.len() * bytes(top) <= budget {
        return done(dense, top, near);
    }
    let sparse = members(starts, total, sync, far);
    let mut h = top;
    loop {
        if sparse.len() * bytes(h) <= budget {
            return done(sparse, h, far);
        }
        if h <= floor {
            break;
        }
        h = (h * 3 / 4).max(floor);
    }
    // Fewer frames, at the smallest size.
    let fit = (budget / bytes(floor)).max(1) as u64;
    let mut spacing = total.div_ceil(fit).max(far);
    loop {
        let m = members(starts, total, sync, spacing);
        if m.len() as u64 <= fit || spacing >= total {
            let mut m = m;
            m.truncate(fit as usize);
            return done(m, floor, spacing);
        }
        spacing += spacing / 8 + 1;
    }
}

/// The sync frame at or before `i` (0 when none is).
fn key_of(sync: &[u32], i: u32) -> u32 {
    match sync.partition_point(|&k| k <= i) {
        0 => 0,
        n => sync[n - 1],
    }
}

/// The next strip frame to decode: sync frames first, nearest `focus`; then (`between`) the
/// frames between them, the GOP nearest `focus` first, in order from where the decoder is
/// (`at`, the next frame it produces) when it is in that GOP. `open`: not decoded yet.
pub fn pick(
    members: &[u32],
    sync: &[u32],
    open: impl Fn(u32) -> bool,
    focus: u32,
    at: Option<u32>,
    between: bool,
) -> Option<u32> {
    let is_sync = |m: u32| sync.binary_search(&m).is_ok();
    let cheap = members
        .iter()
        .copied()
        .filter(|&m| open(m) && is_sync(m))
        .min_by_key(|m| m.abs_diff(focus));
    if cheap.is_some() || !between {
        return cheap;
    }
    let m = members
        .iter()
        .copied()
        .filter(|&m| open(m))
        .min_by_key(|m| m.abs_diff(focus))?;
    let key = key_of(sync, m);
    let end = sync
        .get(sync.partition_point(|&k| k <= key))
        .copied()
        .unwrap_or(u32::MAX);
    let lo = members.partition_point(|&k| k < key);
    let hi = members.partition_point(|&k| k < end);
    let gop = &members[lo..hi];
    let from = at.filter(|a| (key..end).contains(a));
    from.and_then(|a| gop.iter().copied().find(|&k| k >= a && open(k)))
        .or_else(|| gop.iter().copied().find(|&k| open(k)))
}

/// A `w`×`h` copy of `r` (box filter) as a strip frame of kind [`StripKind::of`]`(r)`.
pub fn shrink(r: &Raster, w: u32, h: u32) -> Option<Raster> {
    let kind = StripKind::of(r)?;
    let (w, h) = (w.clamp(1, r.width.max(1)), h.clamp(1, r.height.max(1)));
    let plane = |p: &RasterPlane, dw: u32, dh: u32, ch: usize, wide: bool, shift: u32| {
        let data = box_plane(p, dw as usize, dh as usize, ch, wide, shift);
        RasterPlane {
            width: dw,
            height: dh,
            stride: dw as usize * ch,
            data,
            h_div: 1,
            v_div: 1,
        }
    };
    let (layout, planes) = match (kind, r.layout) {
        (StripKind::Yuv420, PixelLayout::Yuv(e)) => {
            let wide = e.sample_bytes() == 2;
            let shift = u32::from(e.bits.clamp(8, 16)) - 8;
            let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
            let mut c = [
                plane(&r.planes[0], w, h, 1, wide, shift),
                plane(&r.planes[1], cw, ch, 1, wide, shift),
                plane(&r.planes[2], cw, ch, 1, wide, shift),
            ];
            for p in &mut c[1..] {
                (p.h_div, p.v_div) = (2, 2);
            }
            let e = YuvEncoding {
                bits: 8,
                cosited_x: false,
                cosited_y: false,
                bottom_y: false,
                right_x: false,
                alpha: false,
                ..e
            };
            (PixelLayout::Yuv(e), c.to_vec())
        }
        (StripKind::Planes(spec, n), layout) => {
            let planes = r.planes[..n]
                .iter()
                .zip(spec)
                .map(|(p, (b, hd, vd))| {
                    let mut q = plane(
                        p,
                        w.div_ceil(u32::from(hd)),
                        h.div_ceil(u32::from(vd)),
                        usize::from(b),
                        false,
                        0,
                    );
                    (q.h_div, q.v_div) = (hd, vd);
                    q
                })
                .collect();
            (layout, planes)
        }
        _ => return None,
    };
    Some(Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout,
        planes,
        truncated: false,
    })
}

/// Box-filters plane `p` (`ch` interleaved channels of `wide` (u16) or 8-bit samples) to
/// `dw`×`dh` 8-bit samples, dropping `shift` low bits.
fn box_plane(p: &RasterPlane, dw: usize, dh: usize, ch: usize, wide: bool, shift: u32) -> Vec<u8> {
    let (sw, sh) = (p.width as usize, p.height as usize);
    let mut out = vec![0u8; dw * dh * ch];
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return out;
    }
    let (dw, dh) = (dw.min(sw), dh.min(sh));
    // Source column x goes to output column x·dw/sw; row y to row y·dh/sh.
    let col: Vec<usize> = (0..sw).map(|x| x * dw / sw).collect();
    let mut nx = vec![0u64; dw];
    for &c in &col {
        nx[c] += 1;
    }
    let bps = if wide { 2 } else { 1 };
    let mut acc = vec![0u64; dw * ch];
    let unit = 1u64 << shift;
    for oy in 0..dh {
        let y0 = (oy * sh).div_ceil(dh);
        let y1 = ((oy + 1) * sh).div_ceil(dh).min(sh);
        acc.fill(0);
        for y in y0..y1 {
            let Some(row) = p.data.get(y * p.stride..y * p.stride + sw * ch * bps) else {
                continue;
            };
            if wide {
                for (x, s) in row.chunks_exact(2 * ch).enumerate() {
                    let o = col[x] * ch;
                    for k in 0..ch {
                        acc[o + k] += u64::from(u16::from_ne_bytes([s[2 * k], s[2 * k + 1]]));
                    }
                }
            } else if ch == 1 {
                for (x, &s) in row.iter().enumerate() {
                    acc[col[x]] += u64::from(s);
                }
            } else {
                for (x, s) in row.chunks_exact(ch).enumerate() {
                    let o = col[x] * ch;
                    for k in 0..ch {
                        acc[o + k] += u64::from(s[k]);
                    }
                }
            }
        }
        let rows = (y1 - y0).max(1) as u64;
        let line = &mut out[oy * dw * ch..(oy + 1) * dw * ch];
        for (ox, n) in nx.iter().enumerate() {
            let d = n * rows * unit;
            for k in 0..ch {
                let v = (acc[ox * ch + k] + d / 2) / d.max(1);
                line[ox * ch + k] = v.min(255) as u8;
            }
        }
    }
    out
}

/// Starts the strip's thread.
pub(crate) fn spawn(
    shared: Arc<StripShared>,
    open: StripSource,
    cfg: StripConfig,
    cancel: CancelToken,
    budget: BudgetHandle,
) -> Option<std::thread::JoinHandle<()>> {
    let s2 = Arc::clone(&shared);
    let r = specular_thread::builder("anim-strip").spawn(move || {
        let b = budget.clone();
        let what = "scrub strip";
        let r =
            specular_core::isolate::run(what, || b.run(|| run(&s2, open, &cfg, &cancel, &budget)));
        if let Err(p) = r {
            specular_log::warn!(target: specular_log::DECODE, "scrub strip: {}", p.brief());
        }
        let mut st = s2.state.lock();
        st.exited = true;
        st.idle = true;
        st.stats.done = true;
        drop(st);
        s2.idle.notify_all();
    });
    match r {
        Ok(h) => Some(h),
        Err(e) => {
            specular_log::warn!(target: specular_log::DECODE, "scrub strip: cannot start: {e}");
            let mut st = shared.state.lock();
            st.exited = true;
            st.idle = true;
            st.stats.done = true;
            None
        }
    }
}

/// How fast the strip's decoder runs: an average of seconds per frame decoded.
#[derive(Default)]
struct Speed(Option<f64>);

impl Speed {
    fn note(&mut self, frames: u32, took: Duration) {
        let spf = took.as_secs_f64() / f64::from(frames.max(1));
        self.0 = Some(match self.0 {
            Some(s) => s * 0.7 + spf * 0.3,
            None => spf,
        });
    }
}

fn run(
    shared: &StripShared,
    open: StripSource,
    cfg: &StripConfig,
    cancel: &CancelToken,
    budget: &BudgetHandle,
) {
    // Nothing before the player's first frame.
    {
        let mut st = shared.state.lock();
        while !st.go {
            if cancel.is_cancelled() {
                return;
            }
            set_idle(shared, &mut st, true);
            shared.work.wait_for(&mut st, Duration::from_millis(250));
        }
    }
    let Ok(mut frames) = open() else {
        return;
    };
    let Some(delays) = frames.delays().filter(|d| !d.is_empty()) else {
        return;
    };
    let count = u32::try_from(delays.len()).unwrap_or(u32::MAX);
    let mut starts = Vec::with_capacity(delays.len());
    let mut total = 0u64;
    for d in &delays {
        starts.push(total);
        total = total.saturating_add(u64::try_from(d.as_nanos()).unwrap_or(u64::MAX));
    }
    let frame_ns = (total / u64::from(count.max(1))).max(1);
    let mut sync = frames.sync_frames().unwrap_or_else(|| (0..count).collect());
    sync.retain(|&k| k < count);
    if sync.first() != Some(&0) {
        sync.insert(0, 0);
    }
    frames.set_interrupt(Arc::clone(&shared.interrupt));
    let mut lay: Option<Layout> = None;
    let mut failed = BTreeSet::new();
    let mut cap = 0usize;
    let mut speed = Speed::default();
    // The next frame the decoder produces.
    let mut at: Option<u32> = None;
    loop {
        let between_ok = speed
            .0
            .is_none_or(|s| cfg.min_speed <= 0.0 || s * cfg.min_speed <= frame_ns as f64 * 1e-9);
        let job = {
            let mut st = shared.state.lock();
            loop {
                if cancel.is_cancelled() {
                    return;
                }
                if st.hold {
                    set_idle(shared, &mut st, true);
                    shared.work.wait(&mut st);
                    continue;
                }
                let job = match &lay {
                    // First the sync frame where the player is: its size sets the layout.
                    None => Some(key_of(&sync, st.focus.min(count - 1))),
                    Some(l) => pick(
                        &l.members,
                        &sync,
                        |m| !st.frames.contains_key(&m) && !failed.contains(&m),
                        st.focus,
                        at,
                        between_ok,
                    ),
                };
                st.stats.between = lay.as_ref().is_some_and(|l| {
                    between_ok || l.members.iter().all(|m| sync.binary_search(m).is_ok())
                });
                match job {
                    Some(j) => {
                        set_idle(shared, &mut st, false);
                        break j;
                    }
                    None => {
                        st.stats.done = true;
                        return;
                    }
                }
            }
        };
        // A stop asked for before this decode was planned is for an older one.
        shared.interrupt.store(false, Ordering::Release);
        let t0 = Instant::now();
        let from = match at {
            Some(a) if a <= job && key_of(&sync, job) <= a => a,
            _ => key_of(&sync, job),
        };
        match frames.seek(job) {
            Ok(true) => {}
            _ => return,
        }
        let r = frames.next_frame(cancel);
        at = None;
        let f = match r {
            Some(Ok(f)) if f.index == job => f,
            Some(Err(DecodeError::Cancelled)) => {
                if cancel.is_cancelled() {
                    return;
                }
                shared.state.lock().stats.interrupted += 1;
                continue;
            }
            Some(Err(DecodeError::NeedsMemory(_))) => return,
            _ => {
                failed.insert(job);
                if lay.is_none() {
                    return;
                }
                continue;
            }
        };
        at = Some(job + 1);
        let l = match &lay {
            Some(l) => l.clone(),
            None => {
                let Some(kind) = StripKind::of(&f.raster) else {
                    return;
                };
                cap = budget.cap(cfg.budget_bytes as u64) as usize;
                let src = (f.raster.width, f.raster.height);
                let l = layout(&starts, total, &sync, src, kind, cap, cfg);
                let mut st = shared.state.lock();
                st.stats.planned = l.members.len();
                st.stats.width = l.width;
                st.stats.height = l.height;
                lay = Some(l.clone());
                l
            }
        };
        let Some(small) = shrink(&f.raster, l.width, l.height) else {
            return;
        };
        // Timed on frames between sync frames only (a sync frame, after a seek, costs more).
        if sync.binary_search(&job).is_err() {
            speed.note(job + 1 - from, t0.elapsed());
        }
        let size = small.memory_bytes();
        let mut st = shared.state.lock();
        if st.bytes + size > cap {
            st.stats.done = true;
            return;
        }
        let ok = match &mut st.reservation {
            Some(r) => r.grow(size as u64).is_ok(),
            None => budget
                .reserve(size as u64)
                .map(|r| st.reservation = Some(r))
                .is_ok(),
        };
        if !ok {
            st.stats.done = true;
            return;
        }
        st.bytes += size;
        st.stats.decoded += 1;
        st.frames.insert(
            job,
            Arc::new(AnimFrame {
                index: job,
                raster: small,
                delay: f.delay,
                strip: true,
            }),
        );
    }
}

fn set_idle(shared: &StripShared, st: &mut StripState, idle: bool) {
    if st.idle != idle {
        st.idle = idle;
        if idle {
            shared.idle.notify_all();
        }
    }
}
