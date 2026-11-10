//! `--show gallery-scroll-bench` (with `--screenshot`): scripted scrolling through the folder
//! with frames paced at a display refresh, reporting the UI thread's frame times and the frames
//! that missed their refresh.
//!
//! The script is timed by the wall clock, like a user: input arrives on schedule however slow
//! the frames are. It runs, in order (each can be left out):
//!
//! * `wheel`: mouse-wheel bursts (a notch every 15 ms for 0.4 s, then 0.25 s apart), down;
//! * `fling`: touchpad flings (pixel deltas every 8 ms decaying from 14 000 px/s), down then up;
//! * `drag`: the scrollbar thumb dragged to the bottom over 3 s, then to the middle over 1 s;
//! * `pgdn`: Page Down held (33 Hz key repeat) for 4 s;
//! * `settle`: 1.5 s without input (the tiles refine).
//!
//! Environment (benchmarks only):
//! `SPECULAR_SCROLL_BENCH_HZ` (refresh, default 120), `SPECULAR_SCROLL_BENCH_PHASES`
//! (comma list, default all), `SPECULAR_SCROLL_BENCH_DUPES=1` (start the duplicates scan as the
//! script starts), `SPECULAR_SCROLL_BENCH_CSV=PATH` (one line per frame).
//!
//! A frame's *work* is the UI thread's time for it: the gallery's event-loop upkeep since the
//! previous frame plus the frame itself, without waiting for the swapchain image. A frame is
//! *late* when it started more than half a refresh after its slot; `missed` counts the refreshes
//! that showed no new frame.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use specular_log::info;
use specular_thumbs::layout::Nav;
use specular_widgets::Button;

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Wait,
    Wheel,
    Fling,
    Drag,
    PgDn,
    Settle,
    Done,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Phase::Wait => "wait",
            Phase::Wheel => "wheel",
            Phase::Fling => "fling",
            Phase::Drag => "drag",
            Phase::PgDn => "pgdn",
            Phase::Settle => "settle",
            Phase::Done => "done",
        }
    }

    fn secs(self) -> f64 {
        match self {
            Phase::Wheel => 4.0,
            Phase::Fling => 5.0,
            Phase::Drag => 5.0,
            Phase::PgDn => 4.0,
            Phase::Settle => 1.5,
            Phase::Wait | Phase::Done => 0.0,
        }
    }
}

const ORDER: [Phase; 5] = [
    Phase::Wheel,
    Phase::Fling,
    Phase::Drag,
    Phase::PgDn,
    Phase::Settle,
];

/// Per-frame UI-thread time of the gallery's parts (ms), filled in as the frame runs.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Prof {
    pub tick: f32,
    pub feed: f32,
    /// Parts of `feed`: the wish list built (visible, ahead, behind, meta), handed to the
    /// engine's feeder thread, and the rest (folder mosaics).
    pub f_list: f32,
    pub f_post: f32,
    pub f_rest: f32,
    pub upload: f32,
    pub upload_bytes: u64,
    pub uploads: u32,
    pub gallery: f32,
    /// Atlas pages added, and the time of the uploads that added them.
    pub pages: u32,
    pub page: f32,
    /// Sub-phases of `tick` and `gallery` (ms), indexed as [`SUBS`].
    pub sub: [f32; SUBS.len()],
    /// Captions shaped this frame.
    pub caps: u32,
}

/// Sub-phases of the tick (`t_`), the uploads (`u_`), `draw` (`d_`) and the rest of the
/// frame (`r_`), CSV names.
pub(super) const SUBS: [&str; 27] = [
    "t_drive",
    "t_msg",
    "t_ready",
    "t_watch",
    "t_similar",
    "t_layout",
    "t_rest",
    "d_scroll",
    "d_pre",
    "d_layout",
    "d_tiles",
    "d_flush",
    "d_over",
    "d_shape",
    "d_bars",
    "d_facets",
    "d_rest",
    "u_begin",
    "u_scan",
    "u_insert",
    "u_queue",
    "u_end",
    "r_pre",
    "r_image",
    "r_ui",
    "r_submit",
    "r_present",
];
pub(super) const T_DRIVE: usize = 0;
pub(super) const T_MSG: usize = 1;
pub(super) const T_READY: usize = 2;
pub(super) const T_WATCH: usize = 3;
pub(super) const T_SIMILAR: usize = 4;
pub(super) const T_LAYOUT: usize = 5;
pub(super) const T_REST: usize = 6;
pub(super) const D_SCROLL: usize = 7;
pub(super) const D_PRE: usize = 8;
pub(super) const D_LAYOUT: usize = 9;
pub(super) const D_TILES: usize = 10;
pub(super) const D_FLUSH: usize = 11;
pub(super) const D_OVER: usize = 12;
pub(super) const D_SHAPE: usize = 13;
pub(super) const D_BARS: usize = 14;
pub(super) const D_FACETS: usize = 15;
pub(super) const D_REST: usize = 16;
pub(super) const U_BEGIN: usize = 17;
pub(super) const U_SCAN: usize = 18;
pub(super) const U_INSERT: usize = 19;
pub(super) const U_QUEUE: usize = 20;
pub(super) const U_END: usize = 21;
/// The rest of the frame (`app.rs`): before the swapchain image (the gallery phases are part
/// of it), the image's scene, the UI layer, submit and present.
const R_PRE: usize = 22;

/// The UI thread's scheduler and fault counters (Linux): CPU time and run-queue wait (ns),
/// major faults. Bench only: it reads two files under `/proc`.
#[derive(Clone, Copy, Debug, Default)]
struct Sched {
    cpu_ns: u64,
    wait_ns: u64,
    minflt: u64,
    majflt: u64,
}

fn sched_now() -> Sched {
    let mut s = Sched::default();
    if let Ok(t) = std::fs::read_to_string("/proc/thread-self/schedstat") {
        let mut it = t.split_whitespace().map(|v| v.parse::<u64>().unwrap_or(0));
        s.cpu_ns = it.next().unwrap_or(0);
        s.wait_ns = it.next().unwrap_or(0);
    }
    if let Ok(t) = std::fs::read_to_string("/proc/thread-self/stat")
        && let Some(close) = t.rfind(')')
    {
        // After the command: the state (field 3) .. minflt (10), majflt (12).
        let f: Vec<u64> = t[close + 1..]
            .split_whitespace()
            .take(10)
            .map(|v| v.parse().unwrap_or(0))
            .collect();
        s.minflt = f.get(7).copied().unwrap_or(0);
        s.majflt = f.get(9).copied().unwrap_or(0);
    }
    s
}

#[derive(Clone, Copy, Debug)]
struct Rec {
    phase: Phase,
    t: f32,
    work: f32,
    interval: f32,
    missed: u32,
    prof: Prof,
    visible: u32,
    /// Visible image tiles drawn with some thumbnail (any class).
    shown: u32,
    /// … at the class the tiles want.
    exact: u32,
    /// Since the previous frame: the UI thread's CPU time and run-queue wait (ms), major faults.
    cpu: f32,
    runq: f32,
    minflt: u32,
    majflt: u32,
}

pub(super) struct ScrollBench {
    hz: f64,
    phases: Vec<Phase>,
    dupes: bool,
    csv: Option<PathBuf>,
    pub phase: Phase,
    /// Index in `phases`.
    at: usize,
    phase_start: Option<Instant>,
    started: Option<Instant>,
    /// Next scripted input of the phase (s since the phase started).
    next_input: f64,
    fling_v: f32,
    drag: Option<(f32, f32)>,
    last_frame: Option<Instant>,
    last_sched: Option<Sched>,
    recs: Vec<Rec>,
    pub prof: Prof,
    pub started_dupes: bool,
    /// The most pixel bytes queued for upload at once, and the most the engine kept.
    pub queue_peak: u64,
    pub engine_peak: u64,
}

impl ScrollBench {
    pub fn from_env() -> ScrollBench {
        let hz = std::env::var("SPECULAR_SCROLL_BENCH_HZ")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|h| *h >= 10.0)
            .unwrap_or(120.0);
        let phases = match std::env::var("SPECULAR_SCROLL_BENCH_PHASES") {
            Ok(s) if !s.is_empty() => ORDER
                .iter()
                .copied()
                .filter(|p| s.split(',').any(|n| n == p.name()))
                .collect(),
            _ => ORDER.to_vec(),
        };
        ScrollBench {
            hz,
            phases,
            dupes: std::env::var("SPECULAR_SCROLL_BENCH_DUPES").is_ok_and(|v| v == "1"),
            csv: std::env::var_os("SPECULAR_SCROLL_BENCH_CSV").map(PathBuf::from),
            phase: Phase::Wait,
            at: 0,
            phase_start: None,
            started: None,
            next_input: 0.0,
            fling_v: 0.0,
            drag: None,
            last_frame: None,
            last_sched: None,
            recs: Vec::with_capacity(4096),
            prof: Prof::default(),
            started_dupes: false,
            queue_peak: 0,
            engine_peak: 0,
        }
    }

    /// Time between frames.
    pub fn period(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.hz)
    }

    pub fn running(&self) -> bool {
        self.phase != Phase::Done
    }
}

impl App {
    /// Scripted input for the scroll bench, from the event-loop turn.
    pub(super) fn scroll_bench_drive(&mut self) {
        let Some(mut b) = self.gallery.sbench.take() else {
            return;
        };
        self.scroll_bench_input(&mut b);
        self.gallery.sbench = Some(b);
    }

    fn scroll_bench_input(&mut self, b: &mut ScrollBench) {
        let now = Instant::now();
        if b.phase == Phase::Wait {
            // Start once the listing is in and the grid was drawn once.
            let ga = &self.gallery;
            if ga.listing || ga.tiles.is_empty() || ga.atlas.is_none() || ga.dirty {
                return;
            }
            b.started = Some(now);
            b.at = 0;
            b.phase = b.phases.first().copied().unwrap_or(Phase::Done);
            b.phase_start = Some(now);
            b.next_input = 0.0;
            info!(target: UI, "scroll bench: {} tiles, {} Hz, phases {:?}, dupes {}",
                ga.tiles.len(), b.hz, b.phases, b.dupes);
            if b.dupes {
                self.similar_start();
                b.started_dupes = true;
            }
        }
        if b.phase == Phase::Done {
            return;
        }
        let t = b.phase_start.map_or(0.0, |s| (now - s).as_secs_f64());
        if t >= b.phase.secs() {
            if b.phase == Phase::Drag
                && let Some((x, y)) = b.drag.take()
            {
                self.gallery_button(false, Button::Primary, (x, y));
            }
            b.at += 1;
            b.phase = b.phases.get(b.at).copied().unwrap_or(Phase::Done);
            b.phase_start = Some(now);
            b.next_input = 0.0;
            b.fling_v = 0.0;
            if b.phase == Phase::Done {
                self.scroll_bench_report(b);
            }
            self.redraw = true;
            return;
        }
        let (gx, gy) = {
            let ga = &self.gallery;
            (ga.vw * 0.5, ga.grid_top() + ga.grid_h() * 0.5)
        };
        // Bounded: a long stall delivers the missed input at once, as a real queue would.
        for _ in 0..4096 {
            if b.next_input > t {
                break;
            }
            let ti = b.next_input;
            match b.phase {
                Phase::Wheel => {
                    // 0.4 s of notches 15 ms apart, then a 0.25 s pause.
                    let k = ti % 0.65;
                    if k < 0.4 {
                        self.gallery_wheel((0.0, -1.0), false, (gx, gy));
                        b.next_input += 0.015;
                    } else {
                        b.next_input = ((ti / 0.65).floor() + 1.0) * 0.65;
                    }
                }
                Phase::Fling => {
                    // Flings 1.25 s apart: down, down, up, down.
                    let k = ti % 1.25;
                    if k < 0.008 {
                        b.fling_v = 14_000.0;
                    }
                    let dir = if (ti / 1.25) as u32 % 4 == 2 {
                        1.0
                    } else {
                        -1.0
                    };
                    let dy = b.fling_v * 0.008;
                    if dy > 1.0 {
                        // Pixel deltas arrive divided by 40 (see `gallery_wheel`).
                        self.gallery_wheel((0.0, dir * dy / 40.0), true, (gx, gy));
                    }
                    b.fling_v *= (-0.008f32 / 0.45).exp();
                    b.next_input += 0.008;
                }
                Phase::Drag => {
                    let Some(track) = self
                        .gallery
                        .hits
                        .iter()
                        .find(|h| h.1 == Hit::Scrollbar)
                        .map(|h| h.0)
                    else {
                        b.next_input += 0.008;
                        continue;
                    };
                    let x = track.x + track.w * 0.5;
                    let (top, bottom) = (track.y + 4.0, track.y + track.h - 4.0);
                    let y = if ti < 3.0 {
                        top + (bottom - top) * (ti / 3.0) as f32
                    } else if ti < 3.5 {
                        bottom
                    } else if ti < 4.5 {
                        bottom + (top + (bottom - top) * 0.5 - bottom) * ((ti - 3.5) as f32)
                    } else {
                        top + (bottom - top) * 0.5
                    };
                    if b.drag.is_none() {
                        let (ty, th) = self.gallery.thumb();
                        let p = (x, ty + th * 0.5);
                        self.gallery_button(true, Button::Primary, p);
                        b.drag = Some(p);
                    }
                    self.gallery_pointer_move((x, y));
                    b.drag = Some((x, y));
                    b.next_input += 0.008;
                }
                Phase::PgDn => {
                    self.gallery_nav(Nav::PageDown, false, false);
                    b.next_input += 1.0 / 33.0;
                }
                Phase::Settle | Phase::Wait | Phase::Done => b.next_input = f64::MAX,
            }
        }
        self.redraw = true;
    }

    /// Prints the summary (stderr, one JSON line) and the per-frame CSV.
    fn scroll_bench_report(&mut self, b: &ScrollBench) {
        let budget = 1000.0 / b.hz as f32;
        let mut line = format!(
            "{{\"items\":{},\"hz\":{},\"dupes\":{},\"phases\":{{",
            self.gallery.tiles.len(),
            b.hz,
            b.started_dupes
        );
        let mut groups: Vec<(&str, Vec<&Rec>)> = b
            .phases
            .iter()
            .map(|p| (p.name(), b.recs.iter().filter(|r| r.phase == *p).collect()))
            .collect();
        groups.push((
            "scroll",
            b.recs.iter().filter(|r| r.phase != Phase::Settle).collect(),
        ));
        for (k, (name, recs)) in groups.iter().enumerate() {
            let work: Vec<f32> = recs.iter().map(|r| r.work).collect();
            let get = |f: fn(&Prof) -> f32| -> f32 {
                recs.iter().map(|r| f(&r.prof)).fold(0.0, f32::max)
            };
            let missed: u32 = recs.iter().map(|r| r.missed).sum();
            let over = work.iter().filter(|w| **w > budget).count();
            let over60 = work.iter().filter(|w| **w > 1000.0 / 60.0).count();
            let (vis, shown, exact) = recs.iter().fold((0u64, 0u64, 0u64), |a, r| {
                (
                    a.0 + u64::from(r.visible),
                    a.1 + u64::from(r.shown),
                    a.2 + u64::from(r.exact),
                )
            });
            let frac = |n: u64| if vis == 0 { 0.0 } else { n as f64 / vis as f64 };
            let max_up = recs.iter().map(|r| r.prof.upload_bytes).max().unwrap_or(0);
            let _ = write!(
                line,
                "{}\"{name}\":{{\"frames\":{},\"p50\":{:.2},\"p99\":{:.2},\"max\":{:.2},\"over\":{over},\
                 \"over60\":{over60},\"missed\":{missed},\"max_tick\":{:.2},\"max_feed\":{:.2},\
                 \"max_f_list\":{:.2},\"max_f_post\":{:.2},\
                 \"max_upload\":{:.2},\"max_gallery\":{:.2},\"max_upload_kb\":{},\"shown\":{:.3},\"exact\":{:.3}}}",
                if k == 0 { "" } else { "," },
                recs.len(),
                pct(&work, 0.5),
                pct(&work, 0.99),
                pct(&work, 1.0),
                get(|p| p.tick),
                get(|p| p.feed),
                get(|p| p.f_list),
                get(|p| p.f_post),
                get(|p| p.upload),
                get(|p| p.gallery),
                max_up >> 10,
                frac(shown),
                frac(exact),
            );
        }
        let e = self
            .gallery
            .engine
            .as_ref()
            .map(|e| e.stats())
            .unwrap_or_default();
        let _ = write!(
            line,
            "}},\"engine\":{{\"from_cache\":{},\"generated\":{},\"cancelled\":{},\"failed\":{}}},\"rss_mb\":{},\"hwm_mb\":{},\"queue_peak_mb\":{},\"engine_peak_mb\":{},\"items\":{}}}",
            e.from_cache,
            e.generated,
            e.cancelled,
            e.failed,
            rss_mb(),
            status_mb("VmHWM:"),
            b.queue_peak >> 20,
            b.engine_peak >> 20,
            self.gallery.entries.len(),
        );
        info!(target: UI, "scroll bench: {line}");
        eprintln!("scroll-bench {line}");
        if let Some(p) = &b.csv {
            let mut s = String::from(
                "phase,t,work,interval,missed,tick,feed,f_list,f_post,f_rest,\
                 upload,gallery,upload_kb,uploads,pages,page,visible,shown,exact,cpu,runq,minflt,majflt,caps",
            );
            for n in SUBS {
                let _ = write!(s, ",{n}");
            }
            s.push('\n');
            for r in &b.recs {
                let _ = writeln!(
                    s,
                    "{},{:.4},{:.3},{:.3},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{:.3},{},{},{}",
                    r.phase.name(),
                    r.t,
                    r.work,
                    r.interval,
                    r.missed,
                    r.prof.tick,
                    r.prof.feed,
                    r.prof.f_list,
                    r.prof.f_post,
                    r.prof.f_rest,
                    r.prof.upload,
                    r.prof.gallery,
                    r.prof.upload_bytes >> 10,
                    r.prof.uploads,
                    r.prof.pages,
                    r.prof.page,
                    r.visible,
                    r.shown,
                    r.exact
                );
                s.pop();
                let _ = write!(
                    s,
                    ",{:.3},{:.3},{},{},{}",
                    r.cpu, r.runq, r.minflt, r.majflt, r.prof.caps
                );
                for v in r.prof.sub {
                    let _ = write!(s, ",{v:.3}");
                }
                s.push('\n');
            }
            if let Err(e) = std::fs::write(p, s) {
                warn!(target: UI, "scroll bench: {}: {e}", p.display());
            }
        }
    }
}

impl Gallery {
    /// The scroll bench's frame period while it runs (the headless loop paces frames by it).
    pub(in crate::app) fn bench_period(&self) -> Option<Duration> {
        self.sbench
            .as_ref()
            .filter(|b| b.running())
            .map(ScrollBench::period)
    }

    /// The scroll bench still runs (screenshots wait for it).
    pub(in crate::app) fn bench_busy(&self) -> bool {
        self.sbench.as_ref().is_some_and(ScrollBench::running) || self.cmp.bench_running()
    }

    /// One presented frame: `total` from its start, `acquire_ms` of it waiting for the
    /// swapchain.
    pub(in crate::app) fn scroll_bench_frame(
        &mut self,
        start: Instant,
        total: Duration,
        acquire_ms: f64,
    ) {
        let Some(mut b) = self.sbench.take() else {
            return;
        };
        let prof = std::mem::take(&mut b.prof);
        let period = b.period().as_secs_f32() * 1000.0;
        let interval = b
            .last_frame
            .map_or(0.0, |l| (start - l).as_secs_f32() * 1000.0);
        b.last_frame = Some(start);
        let sched = sched_now();
        let last_sched = b.last_sched.replace(sched).unwrap_or(sched);
        if matches!(b.phase, Phase::Wait | Phase::Done) {
            self.sbench = Some(b);
            return;
        }
        let work = (total.as_secs_f64() * 1000.0 - acquire_ms).max(0.0) as f32 + prof.tick;
        // Refreshes without a new frame: the frame came more than half a period late.
        let missed = ((interval / period) - 0.5).floor().max(0.0) as u32;
        let (visible, shown, exact) = self.drawn_quality();
        b.recs.push(Rec {
            phase: b.phase,
            t: b.started.map_or(0.0, |s| s.elapsed().as_secs_f32()),
            work,
            interval,
            missed,
            prof,
            visible,
            shown,
            exact,
            cpu: sched.cpu_ns.saturating_sub(last_sched.cpu_ns) as f32 / 1e6,
            runq: sched.wait_ns.saturating_sub(last_sched.wait_ns) as f32 / 1e6,
            minflt: sched.minflt.saturating_sub(last_sched.minflt) as u32,
            majflt: sched.majflt.saturating_sub(last_sched.majflt) as u32,
        });
        self.sbench = Some(b);
    }

    /// The frame's parts outside the gallery (ms), from `R_PRE` on.
    pub(in crate::app) fn bench_render(&mut self, parts: [f32; 5]) {
        if let Some(b) = self.sbench.as_mut() {
            for (k, v) in parts.into_iter().enumerate() {
                b.prof.sub[R_PRE + k] += v;
            }
        }
    }

    /// Bench only: adds the time since `t` to sub-phase `k` and restarts `t`.
    pub(in crate::app) fn lap(&mut self, k: usize, t: &mut Instant) {
        if let Some(b) = self.sbench.as_mut() {
            let n = Instant::now();
            b.prof.sub[k] += (n - *t).as_secs_f32() * 1000.0;
            *t = n;
        }
    }

    /// Visible image tiles, those with a thumbnail of any class, and those at the wanted class.
    fn drawn_quality(&self) -> (u32, u32, u32) {
        let Some(atlas) = &self.atlas else {
            return (0, 0, 0);
        };
        let class = self.class();
        let range = self
            .layout
            .items_in(self.scroll, self.scroll + self.grid_h());
        let (mut v, mut s, mut x) = (0, 0, 0);
        for i in range {
            let Some(e) = self.entry_of(i) else { continue };
            if e.kind != EntryKind::Image {
                continue;
            }
            v += 1;
            if atlas.contains(&e.path, class) {
                x += 1;
                s += 1;
            } else if specular_thumbs::CLASSES
                .iter()
                .any(|&c| atlas.contains(&e.path, c))
            {
                s += 1;
            }
        }
        (v, s, x)
    }
}

fn pct(v: &[f32], p: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    s[((s.len() - 1) as f32 * p).round() as usize]
}

fn rss_mb() -> u64 {
    status_mb("VmRSS:")
}

/// A `/proc/self/status` line in MiB (`VmRSS:` now, `VmHWM:` its peak).
fn status_mb(key: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
        })
        .map_or(0, |kb| kb >> 10)
}
