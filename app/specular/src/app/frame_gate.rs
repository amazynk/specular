//! Why each frame was drawn, and the scripted runs of the app gates
//! (`tools/bench/perf-gate.py --suite app`).
//!
//! Every frame carries the set of causes that asked for it since the previous one: an input
//! event, a loader event, a subsystem's tick, a timer the last frame set, the last frame's own
//! "draw again" (animation, text, a changed view), or the compositor. A frame presented with
//! none is a bug: something asked for it without saying why, which is how idle loops start.
//!
//! `SPECULAR_GATE_REPORT=PATH` writes the record as JSON to PATH on exit: frames per cause,
//! frames without a cause, input-to-frame latency (wall clock, information only) and the
//! process's peak RSS and page faults. With `SPECULAR_GATE_SCRIPT` the run also drives itself
//! and exits:
//!
//! * `idle`: settle, then count frames for `SPECULAR_GATE_SECS` (default 3);
//! * `press`: settle, press the left button in the middle of the window and hold it still;
//! * `video-paused`: once the video is on screen, pause it, then count;
//! * `walk`: in the Library (`--library`), add `SPECULAR_GATE_WALK` as a location, and once the
//!   walk runs show `SPECULAR_GATE_IMAGE` in the viewer, so the walk goes on out of sight;
//! * `index`: the same, then wait (up to 30 minutes) for the walk and indexing to finish, so a
//!   later run finds a full catalogue;
//! * `next`: settle with the next image prefetched, go to it, count the frames until it is
//!   sharp, then count idle frames;
//! * `mpr`: open MPR on the series, wait for its first view and full load, then count;
//! * `orbit`: after `mpr`, orbit the 3D view in `SPECULAR_GATE_STEPS` (default 20) key steps
//!   50 ms apart, count the frames, then count idle frames once the 3D view has refined;
//! * `loupe`: tap the loupe on over the middle of the image, move the pointer
//!   `SPECULAR_GATE_STEPS` times by 24 px, 50 ms apart, wait until its tiles are in, then
//!   count idle frames with the loupe up. Facts: the frames, scene redraws and tile jobs
//!   started and cancelled while it moved (the scene must not be redrawn for the loupe);
//! * `none` (or unset): record only, for the other benchmark modes.
//!
//! Settling means: the idle-time calibration of the paths done, no frame for one second, no
//! timer set (`wake_at`: a toast or fade still to come), no tiles missing, the decode pool
//! empty and the current image (if any) sharp or failed, within 60 s (`settle_timeout` in the
//! report otherwise). After an action, counting starts once nothing drew for 2.5 s and no
//! timer is set (at most 15 s after it).

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use super::{ActiveEventLoop, App, ElementState, MouseButton};
use specular_input::Action;

/// What asked for a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Cause {
    Start,
    Key,
    Pointer,
    Button,
    Wheel,
    Gesture,
    Window,
    Compositor,
    Loader,
    Accessibility,
    Decoders,
    Remote,
    Medical,
    Mpr,
    Science,
    Commands,
    Panels,
    Inspect,
    Compare,
    Work,
    Gallery,
    Memory,
    Watch,
    Slideshow,
    Present,
    Cull,
    Keys,
    Dialogs,
    Bench,
    Settings,
    Timer,
    Animation,
    Text,
    ViewChanged,
    GalleryAnim,
    Chrome,
    StandIn,
    Fonts,
    PointerHide,
    Selection,
    KeySequence,
    Script,
    Again,
}

const NAMES: [&str; 43] = [
    "start",
    "key",
    "pointer",
    "button",
    "wheel",
    "gesture",
    "window",
    "compositor",
    "loader",
    "accessibility",
    "decoders",
    "remote",
    "medical",
    "mpr",
    "science",
    "commands",
    "panels",
    "inspect",
    "compare",
    "work",
    "gallery",
    "memory",
    "watch",
    "slideshow",
    "present",
    "cull",
    "keys",
    "dialogs",
    "bench",
    "settings",
    "timer",
    "animation",
    "text",
    "view-changed",
    "gallery-anim",
    "chrome",
    "stand-in",
    "fonts",
    "pointer-hide",
    "selection",
    "key-sequence",
    "script",
    "again",
];

impl Cause {
    /// Input from the user (for the input-to-frame latency).
    fn is_input(self) -> bool {
        matches!(
            self,
            Cause::Key | Cause::Pointer | Cause::Button | Cause::Wheel | Cause::Gesture
        )
    }
}

/// A set of causes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Causes(u64);

impl Causes {
    pub(crate) fn add(&mut self, c: Cause) {
        self.0 |= 1 << c as u8;
    }

    pub(crate) fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) fn has(self, c: Cause) -> bool {
        self.0 & (1 << c as u8) != 0
    }

    fn iter(self) -> impl Iterator<Item = &'static str> {
        (0..NAMES.len())
            .filter(move |i| self.0 & (1 << i) != 0)
            .map(|i| NAMES[i])
    }

    /// For the render log: "key, loader" (or "none").
    pub(crate) fn describe(self) -> String {
        let v: Vec<&str> = self.iter().collect();
        if v.is_empty() {
            "none".to_owned()
        } else {
            v.join(", ")
        }
    }
}

/// The causes of the frame being asked for, kept by the app at all times (two words), and the
/// report when one is wanted.
#[derive(Default)]
pub(crate) struct FrameCauses {
    pending: Causes,
    /// The cause of `wake_at`, the timer the app sleeps until.
    pub(crate) wake: Option<Cause>,
    /// When the first input of the pending frame arrived.
    input_at: Option<Instant>,
    /// The loader events since the last frame (names; kept only for a report).
    loader_events: Vec<&'static str>,
    gate: Option<Box<Gate>>,
}

impl FrameCauses {
    pub(crate) fn from_env() -> FrameCauses {
        FrameCauses {
            gate: Gate::from_env().map(Box::new),
            ..FrameCauses::default()
        }
    }

    /// A loader event was handled (named in the report when its frame is in a window).
    pub(crate) fn loader_event(&mut self, name: &'static str) {
        if self.gate.is_some()
            && self.loader_events.len() < 16
            && !self.loader_events.contains(&name)
        {
            self.loader_events.push(name);
        }
    }

    pub(crate) fn add(&mut self, c: Cause) {
        if c.is_input() && self.input_at.is_none() {
            self.input_at = Some(Instant::now());
        }
        self.pending.add(c);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Script {
    None,
    Idle,
    Press,
    VideoPaused,
    Walk,
    Index,
    Next,
    Mpr,
    Orbit,
    Loupe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Waiting for the scene to settle (or for the script's start condition).
    Settle,
    /// The script acted; waiting for its condition before counting.
    Acting,
    /// Counting frames until `until`.
    Window,
    Done,
}

struct Gate {
    report: std::path::PathBuf,
    script: Script,
    secs: f64,
    steps: u32,
    started: Instant,
    phase: Phase,
    phase_at: Instant,
    frames: u64,
    last_frame: Option<Instant>,
    first_present_ms: Option<f64>,
    by_cause: [u64; NAMES.len()],
    uncaused: u64,
    uncaused_at_ms: Vec<f64>,
    input_latency_ms: Vec<f64>,
    frame_cpu_ms: Vec<f64>,
    settle_ms: Option<f64>,
    settle_timeout: bool,
    window_frames: u64,
    window_causes: [u64; NAMES.len()],
    window_secs: f64,
    /// The loader events behind frames in the window.
    window_events: Vec<&'static str>,
    /// Script-specific facts, JSON members ("\"name\": value").
    facts: Vec<String>,
    /// `next`: when the key went and the frames since.
    acted_at: Option<Instant>,
    acted_frames: u64,
    steps_done: u32,
    /// `loupe`: the counters when the moves began.
    loupe_base: super::loupe_bench::TileStats,
    settle_max: Duration,
    /// `next`: the frame (counted from the key) that first showed the image sharp, and when.
    sharp_at: Option<(u64, Instant)>,
    written: bool,
}

const SETTLE_QUIET: Duration = Duration::from_secs(1);
const SETTLE_MAX: Duration = Duration::from_secs(60);
const GRACE: Duration = Duration::from_millis(700);
/// After an action: nothing drawn for this long before counting (longer than the controls'
/// fade-out after a press), at most `ACT_MAX` after the action.
const ACT_QUIET: Duration = Duration::from_millis(2500);
const ACT_MAX: Duration = Duration::from_secs(15);
const STEP_EVERY: Duration = Duration::from_millis(50);
/// `index`: how long the Library may take to index its location.
const INDEX_MAX: Duration = Duration::from_secs(1800);

impl Gate {
    fn from_env() -> Option<Gate> {
        let report = std::env::var_os("SPECULAR_GATE_REPORT").filter(|v| !v.is_empty())?;
        let script = match std::env::var("SPECULAR_GATE_SCRIPT").as_deref() {
            Ok("idle") => Script::Idle,
            Ok("press") => Script::Press,
            Ok("video-paused") => Script::VideoPaused,
            Ok("walk") => Script::Walk,
            Ok("index") => Script::Index,
            Ok("next") => Script::Next,
            Ok("mpr") => Script::Mpr,
            Ok("orbit") => Script::Orbit,
            Ok("loupe") => Script::Loupe,
            _ => Script::None,
        };
        let num = |k: &str, d: f64| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(d)
        };
        let now = Instant::now();
        Some(Gate {
            report: report.into(),
            script,
            secs: num("SPECULAR_GATE_SECS", 3.0),
            steps: num("SPECULAR_GATE_STEPS", 20.0) as u32,
            started: now,
            phase: Phase::Settle,
            phase_at: now,
            frames: 0,
            last_frame: None,
            first_present_ms: None,
            by_cause: [0; NAMES.len()],
            uncaused: 0,
            uncaused_at_ms: Vec::new(),
            input_latency_ms: Vec::new(),
            frame_cpu_ms: Vec::new(),
            settle_ms: None,
            settle_timeout: false,
            window_frames: 0,
            window_causes: [0; NAMES.len()],
            window_secs: 0.0,
            window_events: Vec::new(),
            facts: Vec::new(),
            acted_at: None,
            acted_frames: 0,
            steps_done: 0,
            loupe_base: Default::default(),
            settle_max: Duration::from_secs_f64(num("SPECULAR_GATE_SETTLE_MAX", 60.0)),
            sharp_at: None,
            written: false,
        })
    }

    fn enter(&mut self, p: Phase) {
        self.phase = p;
        self.phase_at = Instant::now();
        if p == Phase::Window {
            self.window_frames = 0;
            self.window_causes = [0; NAMES.len()];
        }
    }

    fn quiet(&self) -> bool {
        self.quiet_for(SETTLE_QUIET)
    }

    fn quiet_for(&self, d: Duration) -> bool {
        self.last_frame.is_none_or(|t| t.elapsed() >= d)
    }

    fn fact(&mut self, name: &str, value: impl std::fmt::Display) {
        self.facts.push(format!("\"{name}\": {value}"));
    }

    fn json(&self) -> String {
        let causes = |v: &[u64; NAMES.len()]| {
            let mut s = String::from("{");
            for (i, n) in v.iter().enumerate().filter(|(_, n)| **n > 0) {
                if s.len() > 1 {
                    s.push_str(", ");
                }
                let _ = write!(s, "\"{}\": {n}", NAMES[i]);
            }
            s.push('}');
            s
        };
        let stats = |v: &[f64]| {
            let mut v = v.to_vec();
            v.sort_by(f64::total_cmp);
            let q = |q: f64| {
                v.get(((v.len() as f64 - 1.0) * q).round() as usize)
                    .copied()
                    .unwrap_or(0.0)
            };
            format!(
                "{{\"n\": {}, \"p50\": {:.2}, \"p95\": {:.2}, \"max\": {:.2}}}",
                v.len(),
                q(0.5),
                q(0.95),
                v.last().copied().unwrap_or(0.0)
            )
        };
        let (minflt, majflt) = faults();
        let rchar = read_bytes();
        let opt = |v: Option<f64>| v.map_or("null".to_owned(), |v| format!("{v:.1}"));
        let mut s = format!(
            "{{\"script\": \"{:?}\", \"frames\": {}, \"uncaused\": {}, \"uncaused_at_ms\": {:?}, \
             \"causes\": {}, \"first_present_ms\": {}, \"settle_ms\": {}, \"settle_timeout\": {}, \
             \"window\": {{\"secs\": {:.2}, \"frames\": {}, \"causes\": {}, \"loader_events\": {:?}}}, \
             \"input_latency_ms\": {}, \"frame_cpu_ms\": {}, \"peak_rss_mb\": {:.1}, \
             \"minflt\": {minflt}, \"majflt\": {majflt}, \"read_syscall_bytes\": {rchar}, \
             \"run_ms\": {:.0}",
            self.script,
            self.frames,
            self.uncaused,
            self.uncaused_at_ms
                .iter()
                .map(|v| (v * 10.0).round() / 10.0)
                .collect::<Vec<_>>(),
            causes(&self.by_cause),
            opt(self.first_present_ms),
            opt(self.settle_ms),
            self.settle_timeout,
            self.window_secs,
            self.window_frames,
            causes(&self.window_causes),
            self.window_events,
            stats(&self.input_latency_ms),
            stats(&self.frame_cpu_ms),
            super::peak_rss_bytes() as f64 / 1048576.0,
            self.started.elapsed().as_secs_f64() * 1000.0,
        );
        for f in &self.facts {
            s.push_str(", ");
            s.push_str(f);
        }
        s.push('}');
        s
    }

    fn write(&mut self) {
        if self.written {
            return;
        }
        self.written = true;
        if let Err(e) = std::fs::write(&self.report, self.json() + "\n") {
            eprintln!("specular: gate report {}: {e}", self.report.display());
        }
    }
}

/// Bytes the process read with read() and friends, files and sockets (`/proc/self/io`
/// rchar; zero elsewhere). Mapped files are not in it.
fn read_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/io")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("rchar:"))
                .and_then(|v| v.trim().parse().ok())
        })
        .unwrap_or(0)
}

/// Minor and major page faults of the process so far (`/proc/self/stat`; zero elsewhere).
fn faults() -> (u64, u64) {
    let Ok(s) = std::fs::read_to_string("/proc/self/stat") else {
        return (0, 0);
    };
    let Some(rest) = s.rsplit_once(')').map(|r| r.1) else {
        return (0, 0);
    };
    let f: Vec<&str> = rest.split_whitespace().collect();
    // After the name: state ppid pgrp session tty tpgid flags minflt cminflt majflt.
    let n = |i: usize| f.get(i).and_then(|v| v.parse().ok()).unwrap_or(0);
    (n(7), n(9))
}

impl App {
    /// Runs `f`; when it asked for a frame nothing had asked for yet, `c` is that frame's
    /// cause.
    pub(super) fn because<R>(&mut self, c: Cause, f: impl FnOnce(&mut App) -> R) -> R {
        let asked = self.redraw;
        let r = f(self);
        if !asked && self.redraw {
            self.causes.add(c);
        }
        r
    }

    /// Sets the timer the app sleeps until to `t` if that is sooner, with its cause.
    pub(super) fn wake_by(&mut self, t: Instant, c: Cause) {
        if self.wake_at.is_none_or(|w| t < w) {
            self.wake_at = Some(t);
            self.causes.wake = Some(c);
        }
    }

    /// The timer expired: its cause asks for the frame.
    pub(super) fn wake_due(&mut self) {
        let c = self.causes.wake.take().unwrap_or(Cause::Timer);
        self.causes.add(c);
    }

    /// Just after a present: the frame takes the pending causes. `asked` is whether the app
    /// asked for it (else the compositor did).
    pub(super) fn frame_presented(&mut self, asked: bool, cpu_ms: f64) -> Causes {
        // `next` counts frames up to the one that shows the image sharp (as drawn, not as
        // seen later by the script's tick).
        let next_pending = self.causes.gate.as_deref().is_some_and(|g| {
            g.script == Script::Next && g.acted_at.is_some() && g.sharp_at.is_none()
        });
        let sharp_now = next_pending && self.gate_sharp();
        let fc = &mut self.causes;
        let mut set = std::mem::take(&mut fc.pending);
        if set.is_empty() && !asked {
            set.add(Cause::Compositor);
        }
        let input_at = fc.input_at.take();
        let loader_events = std::mem::take(&mut fc.loader_events);
        let Some(g) = fc.gate.as_deref_mut() else {
            return set;
        };
        let now = Instant::now();
        g.frames += 1;
        g.first_present_ms
            .get_or_insert_with(|| (now - specular_core::process_start()).as_secs_f64() * 1e3);
        g.last_frame = Some(now);
        g.frame_cpu_ms.push(cpu_ms);
        if let Some(t) = input_at {
            g.input_latency_ms.push((now - t).as_secs_f64() * 1e3);
        }
        if set.is_empty() {
            g.uncaused += 1;
            if g.uncaused_at_ms.len() < 20 {
                g.uncaused_at_ms.push((now - g.started).as_secs_f64() * 1e3);
            }
        }
        for i in 0..NAMES.len() {
            if set.0 & (1 << i) != 0 {
                g.by_cause[i] += 1;
                if g.phase == Phase::Window {
                    g.window_causes[i] += 1;
                }
            }
        }
        if g.phase == Phase::Window {
            g.window_frames += 1;
            if set.has(Cause::Loader) {
                for n in loader_events {
                    if !g.window_events.contains(&n) {
                        g.window_events.push(n);
                    }
                }
            }
        }
        if g.acted_at.is_some() {
            g.acted_frames += 1;
        }
        if sharp_now {
            g.sharp_at = Some((g.acted_frames, now));
        }
        set
    }

    /// Whether the current image is sharp, failed, or there is none.
    fn gate_sharp(&self) -> bool {
        let Some(id) = self.current_id() else {
            return true;
        };
        let img = self.images.get(&id);
        if img.is_some_and(|i| i.error.is_some()) {
            return true;
        }
        let rank = img.and_then(|i| i.gpu_rank);
        matches!((self.sharp_scale(id), rank), (Some(s), Some(r)) if r <= s)
    }

    /// Whether the next image in the folder is on the GPU at the scale it will need.
    fn gate_next_prefetched(&mut self) -> bool {
        let Some(cur) = self.current else {
            return false;
        };
        if cur + 1 >= self.entries.len() {
            return false;
        }
        let id = self.id_for(cur + 1);
        let rank = self.images.get(&id).and_then(|i| i.gpu_rank);
        matches!((self.sharp_scale(id), rank), (Some(s), Some(r)) if r <= s)
    }

    /// Nothing on its way: the current image sharp with all its tiles, and the decode pool
    /// empty (the neighbours' prefetch done; a neighbour opened later still draws).
    pub(super) fn gate_settled(&self) -> bool {
        let pool = self.loader.pool();
        self.tiles_missing == 0
            && self.tile_jobs.is_empty()
            && self.gate_sharp()
            && pool.queued() == 0
            && pool.running() == 0
    }

    /// Once per event-loop turn: drives the script. Returns when it wants to run next.
    pub(super) fn gate_tick(&mut self, el: &ActiveEventLoop) -> Option<Instant> {
        let g = self.causes.gate.as_deref()?;
        let (script, phase, since, settle_max) =
            (g.script, g.phase, g.phase_at.elapsed(), g.settle_max);
        // Quiet: nothing drawn for a while and no timer set (a toast or the controls still to
        // fade): a frame after that has no business being drawn.
        let quiet = g.quiet() && self.wake_at.is_none();
        let poll = Instant::now() + Duration::from_millis(50);
        match phase {
            Phase::Done => return None,
            Phase::Settle => {
                if script == Script::None {
                    return None;
                }
                let start = match script {
                    // Playing: the first frames are on screen.
                    // Playing, with its first frames on screen (pausing a clip that already
                    // ended would start it again).
                    Script::VideoPaused => {
                        let playing = self.anim.as_ref().is_some_and(|a| a.is_playing());
                        playing && g.frames >= 3
                    }
                    Script::Walk | Script::Index => {
                        self.library_has_folders().is_some()
                            && quiet
                            && since > Duration::from_secs(1)
                    }
                    Script::Next => self.gate_settled() && quiet && self.gate_next_prefetched(),
                    _ => self.gate_settled() && quiet,
                };
                // The idle-time calibration first: its end may draw (`SPECULAR_GATE_SETTLE_MAX`
                // lets a warm-up run wait for it).
                let start = start && !super::performance::calibrating();
                let timeout = since > settle_max;
                if !(start || timeout) {
                    return Some(poll);
                }
                let g = self.causes.gate.as_deref_mut().expect("gate");
                g.settle_ms = Some(g.started.elapsed().as_secs_f64() * 1e3);
                g.settle_timeout = timeout;
                self.gate_act(el);
            }
            Phase::Acting => {
                if !self.gate_acting(since) {
                    return Some(poll);
                }
                let g = self.causes.gate.as_deref_mut().expect("gate");
                g.enter(Phase::Window);
            }
            Phase::Window => {
                let secs = g.secs;
                if since.as_secs_f64() < secs {
                    return Some(Instant::now() + Duration::from_secs_f64(secs) - since);
                }
                self.gate_finish();
                return None;
            }
        }
        Some(poll)
    }

    /// The script's action once settled.
    fn gate_act(&mut self, el: &ActiveEventLoop) {
        let script = self.causes.gate.as_deref().expect("gate").script;
        match script {
            Script::Press => {
                let (w, h) = self.gfx.as_ref().map_or((0, 0), |g| {
                    (g.window.inner_size().width, g.window.inner_size().height)
                });
                self.pointer = Some((w as f32 / 2.0, h as f32 / 2.0));
                self.because(Cause::Script, |a| {
                    a.on_mouse_button(ElementState::Pressed, MouseButton::Left, el)
                });
            }
            Script::VideoPaused => {
                let playing = self.anim.as_ref().is_some_and(|a| a.is_playing());
                self.because(Cause::Script, |a| a.playback_action(Action::PlayPause));
                let g = self.causes.gate.as_deref_mut().expect("gate");
                g.fact("was_playing", playing);
            }
            Script::Walk | Script::Index => {
                let dir = std::env::var_os("SPECULAR_GATE_WALK").map(std::path::PathBuf::from);
                if let Some(d) = dir {
                    self.because(Cause::Script, |a| a.library_enter_adding(d));
                }
            }
            Script::Next => {
                let g = self.causes.gate.as_deref_mut().expect("gate");
                g.acted_at = Some(Instant::now());
                g.acted_frames = 0;
                self.because(Cause::Script, |a| a.navigate(1));
            }
            Script::Mpr | Script::Orbit => {
                self.because(Cause::Script, |a| a.mpr_show("mpr"));
            }
            Script::Loupe => {
                let vp = self.view.viewport;
                self.because(Cause::Script, |a| {
                    a.loupe_latched = true;
                    a.on_cursor_moved((vp.x + vp.w * 0.5, vp.y + vp.h * 0.5));
                });
            }
            Script::Idle | Script::None => {}
        }
        let g = self.causes.gate.as_deref_mut().expect("gate");
        g.enter(Phase::Acting);
    }

    /// Whether the script's action is complete (and counting can start).
    fn gate_acting(&mut self, since: Duration) -> bool {
        let script = self.causes.gate.as_deref().expect("gate").script;
        match script {
            Script::Walk => {
                let busy = self.library_walking();
                let g = self.causes.gate.as_deref_mut().expect("gate");
                if g.acted_at.is_none() {
                    if !busy && since < SETTLE_MAX {
                        return false;
                    }
                    g.acted_at = Some(Instant::now());
                    g.fact("walk_started", busy);
                    if let Some(p) = std::env::var_os("SPECULAR_GATE_IMAGE") {
                        self.because(Cause::Script, |a| a.open_target(p.into()));
                    }
                    return false;
                }
                let at = g.acted_at.expect("acted");
                if at.elapsed() < ACT_QUIET + GRACE
                    || !(self.gate_settled() || at.elapsed() > SETTLE_MAX)
                {
                    return false;
                }
                // The walk must still run while frames are counted.
                let g = self.causes.gate.as_deref_mut().expect("gate");
                g.acted_at = None;
                true
            }
            Script::Index => {
                let busy = self.library_walking();
                let g = self.causes.gate.as_deref_mut().expect("gate");
                if g.acted_at.is_none() {
                    if busy {
                        g.acted_at = Some(Instant::now());
                        g.fact("walk_started", true);
                    }
                    return since > SETTLE_MAX;
                }
                let at = g.acted_at.expect("acted");
                if busy && at.elapsed() < INDEX_MAX {
                    return false;
                }
                g.fact(
                    "index_ms",
                    format!("{:.0}", at.elapsed().as_secs_f64() * 1e3),
                );
                g.fact("index_finished", !busy);
                g.acted_at = None;
                true
            }
            Script::Next => {
                let g = self.causes.gate.as_deref_mut().expect("gate");
                // Sharp: then idle again, once the controls' fade after the key is done.
                let Some(at) = g.acted_at else {
                    return g.quiet_for(ACT_QUIET) && self.wake_at.is_none() || since > ACT_MAX;
                };
                let Some((frames, t)) = g.sharp_at else {
                    if at.elapsed() >= SETTLE_MAX {
                        g.fact("frames_to_sharp", "null");
                        g.acted_at = None;
                    }
                    return false;
                };
                let ms = (t - at).as_secs_f64() * 1e3;
                g.fact("frames_to_sharp", frames);
                g.fact("next_sharp_ms", format!("{ms:.1}"));
                g.acted_at = None;
                false
            }
            Script::Mpr | Script::Orbit => {
                let settled = self.mpr_settled();
                if !settled && since < SETTLE_MAX {
                    return false;
                }
                let first = self.mpr_first_view_ms();
                let g = self.causes.gate.as_deref_mut().expect("gate");
                if !g
                    .facts
                    .iter()
                    .any(|f| f.starts_with("\"mpr_first_view_ms\""))
                {
                    g.fact(
                        "mpr_first_view_ms",
                        first.map_or("null".into(), |v| format!("{v:.1}")),
                    );
                    g.fact("mpr_frames_to_settled", g.frames);
                }
                if script == Script::Mpr {
                    return g.quiet() && self.wake_at.is_none();
                }
                // Orbit: one key step every STEP_EVERY, then wait for the refinement.
                if g.steps_done < g.steps {
                    let due = g.acted_at.is_none_or(|t| t.elapsed() >= STEP_EVERY);
                    if !due {
                        return false;
                    }
                    if g.steps_done == 0 {
                        g.acted_frames = 0;
                        g.facts.push(String::new());
                    }
                    g.acted_at = Some(Instant::now());
                    g.steps_done += 1;
                    self.because(Cause::Key, |a| a.mpr_action(Action::PanPageRight));
                    return false;
                }
                if g.facts.last().is_some_and(String::is_empty) {
                    let frames = g.acted_frames;
                    g.facts.pop();
                    g.fact("orbit_steps", g.steps);
                    g.fact("orbit_frames", frames);
                    let n = frames as usize;
                    let tail: Vec<f64> = g.frame_cpu_ms.iter().rev().take(n).copied().collect();
                    let worst = tail.iter().copied().fold(0.0, f64::max);
                    g.fact("orbit_frame_cpu_max_ms", format!("{worst:.2}"));
                }
                if !(settled && g.quiet() && self.wake_at.is_none()) {
                    return false;
                }
                let g = self.causes.gate.as_deref_mut().expect("gate");
                g.fact("orbit_frames_to_rest", g.acted_frames);
                g.acted_at = None;
                true
            }
            Script::Loupe => {
                let stats = self.tile_stats;
                let settled = self.gate_settled() && since >= GRACE;
                let shown = self.loupe_view().is_some();
                let g = self.causes.gate.as_deref_mut().expect("gate");
                // The loupe's first tiles in, then one move every STEP_EVERY.
                if g.steps_done == 0 && !(settled || since > SETTLE_MAX) {
                    return false;
                }
                if g.steps_done < g.steps {
                    if g.acted_at.is_some_and(|t| t.elapsed() < STEP_EVERY) {
                        return false;
                    }
                    if g.steps_done == 0 {
                        g.acted_frames = 0;
                        g.loupe_base = stats;
                    }
                    g.acted_at = Some(Instant::now());
                    g.steps_done += 1;
                    let step = g.steps_done as f32;
                    let vp = self.view.viewport;
                    self.because(Cause::Pointer, |a| {
                        a.on_cursor_moved((vp.x + vp.w * 0.5 + 24.0 * step, vp.y + vp.h * 0.5));
                    });
                    return false;
                }
                if g.acted_at.is_some() {
                    let b = g.loupe_base;
                    g.fact("loupe_moves", g.steps);
                    g.fact("loupe_shown", shown);
                    g.fact("loupe_frames", g.acted_frames);
                    g.fact("loupe_scene_redraws", stats.scenes - b.scenes);
                    g.fact("loupe_jobs_started", stats.started - b.started);
                    g.fact("loupe_jobs_cancelled", stats.cancelled - b.cancelled);
                    g.acted_at = None;
                }
                settled && g.quiet() && self.wake_at.is_none() || since > ACT_MAX
            }
            // What the action itself set off (the controls fading after a press, the paused
            // frame) finishes first: counting starts once nothing drew for ACT_QUIET.
            Script::VideoPaused | Script::Press | Script::Idle | Script::None => {
                let g = self.causes.gate.as_deref().expect("gate");
                since >= GRACE
                    && (g.quiet_for(ACT_QUIET) && self.wake_at.is_none() || since > ACT_MAX)
            }
        }
    }

    /// The counting window ended: release what the script holds, write the report, exit.
    fn gate_finish(&mut self) {
        let script = self.causes.gate.as_deref().expect("gate").script;
        if script == Script::Walk {
            let busy = self.library_walking();
            let g = self.causes.gate.as_deref_mut().expect("gate");
            g.fact("walk_busy_at_end", busy);
        }
        let g = self.causes.gate.as_deref_mut().expect("gate");
        g.window_secs = g.phase_at.elapsed().as_secs_f64();
        g.enter(Phase::Done);
        g.write();
        self.exit_requested = true;
    }

    /// On exit (also for runs without a script).
    pub(super) fn gate_write(&mut self) {
        if let Some(g) = self.causes.gate.as_deref_mut() {
            g.write();
        }
    }
}
