//! What the selector chose on this machine and why, for Settings → Performance ("How Specular
//! runs on this computer") and the hardware report; plus the two actions offered there:
//! [`PathSelector::retry`] a blocked path and [`PathSelector::reset_measurements`]
//! ("Measure again").
//!
//! [`PathSelector::snapshot`] only reads: it copies the atomics into a private probe selector
//! and asks that one, so the exploration counters, the load signal and the learned timings of
//! the real selector are untouched. It costs one ~12 KB copy and a few dozen predictions, so
//! it is meant for a page that is open (polled every second or two), not for every frame.

use std::fmt::Write as _;
use std::sync::atomic::Ordering;

use super::{
    BUCKETS, Engine, Estimate, Key, MAX_PATHS, Mode, Objective, Override, PathSelector, Reason,
    Request, Source, Task, Trust, VARIANTS, bucket, cell_index, engine_index,
};

/// The image size the snapshot asks about: a typical camera photo.
pub const SNAPSHOT_MPX: f32 = 12.0;

/// What one request of a task works on, for reports: the size its times are for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    /// The image itself, at the size the report asks about ([`SNAPSHOT_MPX`] in Settings).
    Image,
    /// A model's input thumbnail, always this size ([`Task::Inference`]).
    Thumbnail { width: u32, height: u32 },
    /// One video frame at a typical size ([`Task::VideoEncode`] is timed per frame).
    Frame { width: u32, height: u32 },
}

impl Unit {
    /// The pixels of one request, when the task has a size of its own.
    pub const fn pixels(self) -> Option<u64> {
        match self {
            Unit::Image => None,
            Unit::Thumbnail { width, height } | Unit::Frame { width, height } => {
                Some(width as u64 * height as u64)
            }
        }
    }

    /// The request size for a report about `mpx`-megapixel images.
    pub fn mpx(self, mpx: f32) -> f32 {
        self.pixels().map_or(mpx, |p| p as f32 / 1e6)
    }
}

/// Why a path was blocked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Failure {
    /// Its output differed from the CPU reference ([`PathSelector::verified`]).
    CheckFailed = 1,
    /// The driver or device reported an error.
    Error = 2,
    /// The device was lost, the driver crashed or timed out.
    DeviceLost = 3,
}

impl Failure {
    /// The failure an error message describes (device loss and timeouts are told apart from
    /// other errors; anything else is an error).
    pub fn classify(why: &str) -> Failure {
        let w = why.to_ascii_lowercase();
        if ["lost", "crash", "timeout", "timed out", "hang", "reset"]
            .iter()
            .any(|k| w.contains(k))
        {
            Failure::DeviceLost
        } else if w.contains("check") || w.contains("psnr") || w.contains("mismatch") {
            Failure::CheckFailed
        } else {
            Failure::Error
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Failure::CheckFailed => "check_failed",
            Failure::Error => "error",
            Failure::DeviceLost => "device_lost",
        }
    }

    fn from_u8(v: u8) -> Option<Failure> {
        match v {
            1 => Some(Failure::CheckFailed),
            2 => Some(Failure::Error),
            3 => Some(Failure::DeviceLost),
            _ => None,
        }
    }

    fn from_key(s: &str) -> Option<Failure> {
        (1..=3).filter_map(Failure::from_u8).find(|f| f.key() == s)
    }
}

/// When and why a path was blocked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blocked {
    /// Seconds since 1970 (UTC); 0 when unknown (a table from before this was recorded).
    pub at_unix: u64,
    pub failure: Failure,
}

/// How much the measurements behind a prediction can be trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Confidence {
    /// No measurement: the usual figure for this kind of computer.
    None,
    /// 1–4 runs.
    Low,
    /// 5–19 runs.
    Medium,
    /// 20 runs or more.
    High,
}

impl Confidence {
    pub const fn of(samples: u32) -> Confidence {
        match samples {
            0 => Confidence::None,
            1..=4 => Confidence::Low,
            5..=19 => Confidence::Medium,
            _ => Confidence::High,
        }
    }
}

/// One way of doing a task, as it stands.
#[derive(Clone, Debug)]
pub struct PathState {
    pub engine: Engine,
    pub trust: Trust,
    /// Timed runs over all cells (wall time).
    pub samples: u32,
    /// Runs whose CPU time was measured cleanly.
    pub cpu_samples: u32,
    /// Predicted cost of a [`SNAPSHOT_MPX`] request on an idle machine.
    pub estimate: Estimate,
    pub confidence: Confidence,
    pub blocked: Option<Blocked>,
}

/// A setting that decided instead of the measurements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// `performance.processing` (Prefer the graphics processor, Processor only).
    Processing(Mode),
    /// The task's own switch (`performance.hardware_decode`, `performance.fast_yuv_upload`).
    Override(Task, Override),
}

/// Two costs side by side: the chosen path's and the best other one's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Comparison {
    pub chosen: Estimate,
    pub other_engine: Engine,
    pub other: Estimate,
}

/// Why a decision came out the way it did, in the terms the Settings page shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Why {
    /// Measured on this machine (both paths compared have timings).
    Measured(Comparison),
    /// Not measured yet: the usual choice for this kind of computer (at least one of the
    /// compared costs is a prior).
    NotMeasured(Comparison),
    /// A setting decided.
    Setting(Setting),
    /// The other path is blocked after an error or a failed check.
    Blocked { engine: Engine, blocked: Blocked },
    /// The other path is still being checked against the CPU reference (background work
    /// tries it; the image on screen does not wait for it).
    Checking { engine: Engine },
    /// Nothing else exists on this machine for this task.
    Unavailable { engine: Engine },
}

/// The chosen path for one objective.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    pub objective: Objective,
    pub engine: Engine,
    pub path: usize,
    /// The selector's own reason code (logs).
    pub reason: Reason,
    pub why: Why,
}

/// One task as it stands.
#[derive(Clone, Debug)]
pub struct TaskSnapshot {
    pub task: Task,
    /// Something exists or was measured for this task on this machine (hooks with nothing
    /// registered are not).
    pub active: bool,
    pub override_: Override,
    /// The request size this task's estimates and decisions are for ([`Task::unit`]).
    pub mpx: f32,
    /// Index = path index; [0] is the CPU reference.
    pub paths: Vec<PathState>,
    /// A background request is trying a path not currently chosen right now.
    pub exploring_now: bool,
    /// The image the user is looking at.
    pub interactive: Decision,
    /// Prefetch, thumbnails, indexing.
    pub background: Decision,
}

impl TaskSnapshot {
    /// The paths that are blocked, with when and why.
    pub fn blocked(&self) -> impl Iterator<Item = (&PathState, Blocked)> {
        self.paths.iter().filter_map(|p| p.blocked.map(|b| (p, b)))
    }
}

/// The machine the measurements belong to (from the fingerprint).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Machine {
    pub gpu: String,
    pub driver: String,
    pub cpu: String,
    pub app_version: String,
}

impl Machine {
    /// Splits a [`fingerprint`](super::fingerprint) back into names (`_` back to spaces).
    pub fn parse(fingerprint: &str) -> Machine {
        let mut it = fingerprint.split('|').map(|s| s.replace('_', " "));
        Machine {
            gpu: it.next().unwrap_or_default(),
            driver: it.next().unwrap_or_default(),
            cpu: it.next().unwrap_or_default(),
            app_version: it.next().unwrap_or_default(),
        }
    }
}

/// Everything the Settings page shows about the selector.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub machine: Machine,
    pub gpu_class: super::GpuClass,
    pub threads: u16,
    pub mode: Mode,
    /// On battery or under thermal pressure: the decisions minimise energy.
    pub power_saving: bool,
    pub calibrated: bool,
    /// The image size the decisions and estimates are for; tasks with a [`Unit`] of their
    /// own are for that instead ([`TaskSnapshot::mpx`]).
    pub mpx: f32,
    /// All tasks in [`Task::ALL`] order (filter on [`TaskSnapshot::active`]).
    pub tasks: Vec<TaskSnapshot>,
}

impl Snapshot {
    pub fn task(&self, task: Task) -> &TaskSnapshot {
        &self.tasks[task as usize]
    }

    /// Any path blocked on this machine.
    pub fn any_blocked(&self) -> bool {
        self.tasks.iter().any(|t| t.blocked().next().is_some())
    }
}

impl Task {
    /// What one request of the task works on (reports time it at this size).
    pub const fn unit(self) -> Unit {
        match self {
            // The models' input: ViT-Ti/16 and its kind at 224², which calibration times.
            Task::Inference => Unit::Thumbnail {
                width: 224,
                height: 224,
            },
            // The priors and real runs are per frame; the slideshow export's usual size.
            Task::VideoEncode => Unit::Frame {
                width: 1920,
                height: 1080,
            },
            _ => Unit::Image,
        }
    }

    /// The message id of the task's user-facing name (`specular-i18n`, app.ftl).
    pub const fn message_id(self) -> &'static str {
        match self {
            Task::JpegDecode => "hw-report-path-jpeg-decode",
            Task::YcbcrToRgb => "hw-report-path-ycbcr-to-rgb",
            Task::PngUnfilter => "hw-report-path-png-unfilter",
            Task::HeicDecode => "hw-report-path-heic-decode",
            Task::Inference => "hw-report-path-inference",
            Task::VideoEncode => "hw-report-path-video-encode",
            // The mode's own name (already translated everywhere).
            Task::Develop => "develop-title",
        }
    }
}

impl Engine {
    /// The message id of the engine's user-facing name.
    pub const fn message_id(self) -> &'static str {
        match self {
            Engine::Cpu => "paths-engine-cpu",
            Engine::CpuParallel => "paths-engine-cpu-parallel",
            Engine::Gpu => "paths-engine-gpu",
            Engine::HwDecoder => "paths-engine-hw-decoder",
            Engine::Npu => "paths-engine-npu",
            Engine::HwEncoder => "paths-engine-hw-encoder",
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl PathSelector {
    // -- block bookkeeping (called from paths.rs) --------------------------------------------

    /// Remembers when and why `engine` was blocked; the first block wins until cleared.
    pub(super) fn note_block(&self, task: Task, engine: Engine, f: Failure, at: Option<u64>) {
        let Some(i) = task.path_of(engine).filter(|&i| i > 0) else {
            return;
        };
        let packed = (at.unwrap_or_else(now_unix) << 8) | f as u64;
        let _ = self.blocks[task as usize * MAX_PATHS + i].compare_exchange(
            0,
            packed,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }

    /// Blocklists a path with an explicit failure class (see [`block`](Self::block), which
    /// guesses it from the message).
    pub fn block_as(&self, task: Task, engine: Engine, failure: Failure, why: &str) {
        self.set_trust(task, engine, Trust::Blocked);
        self.note_block(task, engine, failure, None);
        super::log_block(task, engine, why);
    }

    /// When and why `path` was blocked, if it is.
    pub fn blocked_info(&self, task: Task, path: usize) -> Option<Blocked> {
        if self.trust(task, path) != Trust::Blocked {
            return None;
        }
        let v = self.blocks[task as usize * MAX_PATHS + path].load(Ordering::Relaxed);
        Some(Blocked {
            at_unix: v >> 8,
            // Blocked without a record (set_trust directly): an error.
            failure: Failure::from_u8(v as u8).unwrap_or(Failure::Error),
        })
    }

    pub(super) fn write_blocks(&self, s: &mut String) {
        for task in Task::ALL {
            for (i, e) in task.engines().iter().enumerate().skip(1) {
                if let Some(b) = self.blocked_info(task, i) {
                    let _ = writeln!(
                        s,
                        "blocked {} {} {} {}",
                        task.key(),
                        e.key(),
                        b.failure.key(),
                        b.at_unix
                    );
                }
            }
        }
    }

    pub(super) fn read_block(&self, task: &str, engine: &str, failure: &str, at: &str) {
        let Some(task) = Task::ALL.into_iter().find(|t| t.key() == task) else {
            return;
        };
        let Some(i) = engine_index(task, engine).filter(|&i| i > 0) else {
            return;
        };
        let failure = Failure::from_key(failure).unwrap_or(Failure::Error);
        let at = at.parse::<u64>().unwrap_or(0) & (u64::MAX >> 8);
        self.blocks[task as usize * MAX_PATHS + i]
            .store((at << 8) | failure as u64, Ordering::Relaxed);
    }

    // -- actions -----------------------------------------------------------------------------

    /// Unblocks a blocked path: it goes back to unverified, is checked again by the next
    /// background run that can check it, and calibration runs again. Returns whether it was
    /// blocked.
    pub fn retry(&self, task: Task, engine: Engine) -> bool {
        let Some(i) = task.path_of(engine).filter(|&i| i > 0) else {
            return false;
        };
        let k = task as usize * MAX_PATHS + i;
        let was = self.trust[k]
            .compare_exchange(
                Trust::Blocked as u8,
                Trust::Unverified as u8,
                Ordering::Relaxed,
                Ordering::Relaxed,
            )
            .is_ok();
        if was {
            self.blocks[k].store(0, Ordering::Relaxed);
            self.calibrated.store(false, Ordering::Relaxed);
            self.dirty.fetch_add(1, Ordering::Relaxed);
        }
        was
    }

    /// "Measure again": forgets this machine's timings and blocklist; every path that exists
    /// goes back to unverified (checked again before it is used while the user waits), and
    /// calibration runs again. Keeps the fingerprint, mode, overrides and machine class.
    pub fn reset_measurements(&self) {
        for c in &self.cells {
            c.reset();
        }
        for t in &self.trust {
            let _ = t.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                (Trust::from_u8(v) != Trust::Absent).then_some(Trust::Unverified as u8)
            });
        }
        for b in &self.blocks {
            b.store(0, Ordering::Relaxed);
        }
        for r in &self.rechecked {
            r.store(0, Ordering::Relaxed);
        }
        self.calibrated.store(false, Ordering::Relaxed);
        self.dirty.fetch_add(1, Ordering::Relaxed);
    }

    // -- the snapshot ------------------------------------------------------------------------

    /// What each task does now and why, for a [`SNAPSHOT_MPX`] image (see the module docs:
    /// read-only, cheap, never moves the exploration counters).
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot_at(SNAPSHOT_MPX)
    }

    /// [`snapshot`](Self::snapshot) for a `mpx`-megapixel request.
    pub fn snapshot_at(&self, mpx: f32) -> Snapshot {
        let probe = Box::new(PathSelector::new());
        probe.copy_from(self);
        let pixels = (mpx.max(0.01) * 1e6) as u64;
        let tasks = Task::ALL
            .into_iter()
            .map(|task| self.task_snapshot(&probe, task, pixels))
            .collect();
        Snapshot {
            machine: Machine::parse(&self.fingerprint()),
            gpu_class: self.gpu_class(),
            threads: self.threads(),
            mode: self.mode(),
            power_saving: self.power_saving(),
            calibrated: self.calibrated(),
            mpx,
            tasks,
        }
    }

    fn task_snapshot(&self, probe: &PathSelector, task: Task, pixels: u64) -> TaskSnapshot {
        let pixels = task.unit().pixels().unwrap_or(pixels);
        let mpx = pixels as f32 / 1e6;
        let key = Key {
            variant: 0,
            bucket: bucket(pixels),
            busy: false,
        };
        let paths: Vec<PathState> = task
            .engines()
            .iter()
            .enumerate()
            .map(|(i, &engine)| {
                let (samples, cpu_samples) = self.totals(task, i);
                PathState {
                    engine,
                    trust: self.trust(task, i),
                    samples,
                    cpu_samples,
                    estimate: self.estimate(task, i, key, mpx),
                    confidence: Confidence::of(samples),
                    blocked: self.blocked_info(task, i),
                }
            })
            .collect();
        let active = paths.iter().skip(1).any(|p| p.trust != Trust::Absent)
            || paths.iter().any(|p| p.samples + p.cpu_samples > 0);
        let decide = |objective| {
            let req = Request {
                objective,
                pixels,
                variant: 0,
                can_verify: false,
            };
            let c = probe.choose(task, &req);
            Decision {
                objective,
                engine: c.engine,
                path: c.path,
                reason: c.reason,
                why: self.why(task, objective, c.path, c.reason, &paths),
            }
        };
        TaskSnapshot {
            task,
            active,
            override_: self.override_of(task),
            mpx,
            exploring_now: self.exploring[task as usize].load(Ordering::Relaxed) > 0,
            interactive: decide(Objective::Interactive),
            background: decide(Objective::Background),
            paths,
        }
    }

    /// Samples over all cells of a path: (wall, CPU).
    fn totals(&self, task: Task, path: usize) -> (u32, u32) {
        let mut n = 0u32;
        let mut cpu = 0u32;
        for variant in 0..VARIANTS as u8 {
            for bucket in 0..BUCKETS as u8 {
                for busy in [false, true] {
                    let c = &self.cells[cell_index(
                        task,
                        path,
                        Key {
                            variant,
                            bucket,
                            busy,
                        },
                    )];
                    n = n.saturating_add(c.n.load(Ordering::Relaxed));
                    cpu = cpu.saturating_add(c.cpu_n.load(Ordering::Relaxed));
                }
            }
        }
        (n, cpu)
    }

    /// Maps the selector's reason to what the page says.
    fn why(
        &self,
        task: Task,
        objective: Objective,
        chosen: usize,
        reason: Reason,
        paths: &[PathState],
    ) -> Why {
        match reason {
            Reason::CpuOnly => return Why::Setting(Setting::Processing(Mode::CpuOnly)),
            Reason::PreferGpu => return Why::Setting(Setting::Processing(Mode::PreferGpu)),
            Reason::Forced => return Why::Setting(Setting::Override(task, self.override_of(task))),
            _ => {}
        }
        // The best other path that has a cost to compare with (trusted, not chosen).
        let power = self.power_saving();
        let metric = |e: &Estimate| match (objective, power) {
            (Objective::Interactive, false) => e.wall_us,
            (Objective::Background, false) => e.cpu_us,
            // Energy without the per-path power weight is close enough for a comparison
            // line; the decision itself used the full figure.
            (_, true) => e.cpu_us + e.wall_us,
        };
        let other = paths
            .iter()
            .enumerate()
            .filter(|&(i, p)| i != chosen && (i == 0 || p.trust == Trust::Trusted))
            .min_by(|a, b| metric(&a.1.estimate).total_cmp(&metric(&b.1.estimate)));
        if let Some((_, o)) = other
            && !matches!(reason, Reason::Only)
        {
            let cmp = Comparison {
                chosen: paths[chosen].estimate,
                other_engine: o.engine,
                other: o.estimate,
            };
            return if cmp.chosen.source == Source::Measured && cmp.other.source == Source::Measured
            {
                Why::Measured(cmp)
            } else {
                Why::NotMeasured(cmp)
            };
        }
        // No comparison: say why the others are out, the most actionable first.
        let alts = || {
            paths
                .iter()
                .enumerate()
                .skip(1)
                .filter(|&(i, _)| i != chosen)
        };
        if let Some((_, p)) = alts().find(|(_, p)| p.trust == Trust::Blocked) {
            return Why::Blocked {
                engine: p.engine,
                blocked: p.blocked.unwrap_or(Blocked {
                    at_unix: 0,
                    failure: Failure::Error,
                }),
            };
        }
        if let Some((_, p)) = alts().find(|(_, p)| p.trust == Trust::Unverified) {
            return Why::Checking { engine: p.engine };
        }
        let engine = alts()
            .map(|(_, p)| p.engine)
            .next()
            .unwrap_or(task.engines()[task.engines().len() - 1]);
        Why::Unavailable { engine }
    }
}

#[cfg(test)]
mod tests {
    use super::super::GpuClass;
    use super::*;

    fn sel() -> PathSelector {
        let s = PathSelector::new();
        s.set_machine(GpuClass::Integrated, 16);
        s
    }

    #[test]
    fn failure_classes() {
        assert_eq!(
            Failure::classify("VA-API: device lost"),
            Failure::DeviceLost
        );
        assert_eq!(Failure::classify("GPU timed out"), Failure::DeviceLost);
        assert_eq!(
            Failure::classify("vaEndPicture failed (code 1)"),
            Failure::Error
        );
        assert_eq!(
            Failure::classify("failed its check against the CPU reference"),
            Failure::CheckFailed
        );
    }

    #[test]
    fn nothing_registered_is_unavailable() {
        let s = sel();
        let snap = s.snapshot();
        let t = snap.task(Task::JpegDecode);
        assert!(!t.active);
        assert_eq!(t.interactive.engine, Engine::Cpu);
        assert_eq!(
            t.interactive.why,
            Why::Unavailable {
                engine: Engine::HwDecoder
            }
        );
    }

    #[test]
    fn reasons_map_to_settings_blocks_and_checks() {
        let s = sel();
        s.set_available(Task::JpegDecode, Engine::HwDecoder);
        let t = s.snapshot().tasks[Task::JpegDecode as usize].clone();
        assert!(t.active);
        assert_eq!(
            t.interactive.why,
            Why::Checking {
                engine: Engine::HwDecoder
            }
        );
        s.block_as(
            Task::JpegDecode,
            Engine::HwDecoder,
            Failure::DeviceLost,
            "lost",
        );
        let t = s.snapshot().tasks[Task::JpegDecode as usize].clone();
        let Why::Blocked { engine, blocked } = t.background.why else {
            panic!("{:?}", t.background.why)
        };
        assert_eq!(engine, Engine::HwDecoder);
        assert_eq!(blocked.failure, Failure::DeviceLost);
        assert!(blocked.at_unix > 1_700_000_000);
        assert!(s.snapshot().any_blocked());

        s.set_trust(Task::YcbcrToRgb, Engine::Gpu, Trust::Trusted);
        s.set_mode(Mode::CpuOnly);
        let snap = s.snapshot();
        assert_eq!(
            snap.task(Task::YcbcrToRgb).interactive.why,
            Why::Setting(Setting::Processing(Mode::CpuOnly))
        );
        s.set_mode(Mode::Automatic);
        s.set_override(Task::YcbcrToRgb, Override::Off);
        assert_eq!(
            s.snapshot().task(Task::YcbcrToRgb).interactive.why,
            Why::Setting(Setting::Override(Task::YcbcrToRgb, Override::Off))
        );
        s.set_override(Task::YcbcrToRgb, Override::Auto);
        // Trusted, no timings: the usual choice, compared on priors.
        let w = s.snapshot().task(Task::YcbcrToRgb).interactive.why;
        assert!(matches!(w, Why::NotMeasured(_)), "{w:?}");
    }

    #[test]
    fn measured_needs_both_sides_measured() {
        let s = sel();
        s.set_trust(Task::PngUnfilter, Engine::CpuParallel, Trust::Trusted);
        let key = Key {
            variant: 0,
            bucket: bucket(12_000_000),
            busy: false,
        };
        for _ in 0..6 {
            s.record(Task::PngUnfilter, 0, key, 12.0, 120_000.0, Some(120_000.0));
            s.record(Task::PngUnfilter, 1, key, 12.0, 40_000.0, Some(200_000.0));
        }
        let snap = s.snapshot();
        let t = snap.task(Task::PngUnfilter);
        assert_eq!(t.interactive.engine, Engine::CpuParallel);
        let Why::Measured(c) = t.interactive.why else {
            panic!("{:?}", t.interactive.why)
        };
        assert_eq!(c.other_engine, Engine::Cpu);
        assert!(c.chosen.wall_us < c.other.wall_us);
        // In the background the CPU time decides: one thread.
        assert_eq!(t.background.engine, Engine::Cpu);
        assert!(matches!(t.background.why, Why::Measured(_)));
        assert_eq!(t.paths[1].samples, 6);
        assert_eq!(t.paths[1].confidence, Confidence::Medium);
    }

    #[test]
    fn snapshot_does_not_perturb() {
        let s = sel();
        s.set_trust(Task::JpegDecode, Engine::HwDecoder, Trust::Trusted);
        let before = s.to_table();
        for _ in 0..100 {
            let _ = s.snapshot();
        }
        assert_eq!(s.to_table(), before);
        assert_eq!(
            s.background[Task::JpegDecode as usize].load(Ordering::Relaxed),
            0
        );
        assert_eq!(s.take_dirty(), 0);
        assert_eq!(s.inflight(), 0);
    }

    #[test]
    fn exploring_runs_are_counted_while_in_flight() {
        let s = sel();
        s.set_trust(Task::PngUnfilter, Engine::CpuParallel, Trust::Trusted);
        let req = Request::new(12_000_000, 0).objective(Objective::Background);
        let mut explore = None;
        for _ in 0..super::super::EXPLORE_EVERY {
            let c = s.choose(Task::PngUnfilter, &req);
            if c.reason == Reason::Explore {
                explore = Some(c);
            }
        }
        let c = explore.expect("one exploring choice in sixteen");
        let run = s.start(&c);
        assert!(s.snapshot().task(Task::PngUnfilter).exploring_now);
        drop(run);
        assert!(!s.snapshot().task(Task::PngUnfilter).exploring_now);
    }

    #[test]
    fn retry_and_measure_again() {
        let s = sel();
        s.set_trust(Task::YcbcrToRgb, Engine::Gpu, Trust::Trusted);
        s.set_available(Task::JpegDecode, Engine::HwDecoder);
        s.verified(Task::JpegDecode, Engine::HwDecoder, false);
        assert_eq!(
            s.blocked_info(Task::JpegDecode, 1).unwrap().failure,
            Failure::CheckFailed
        );
        // The blocklist entry survives a save and load.
        let t = s.to_table();
        assert!(
            t.contains("blocked jpeg_decode hw_decoder check_failed "),
            "{t}"
        );
        let s2 = sel();
        assert!(s2.load_table(&t, None));
        assert_eq!(
            s2.blocked_info(Task::JpegDecode, 1).unwrap(),
            s.blocked_info(Task::JpegDecode, 1).unwrap()
        );

        s.set_calibrated();
        assert!(s.retry(Task::JpegDecode, Engine::HwDecoder));
        assert!(!s.retry(Task::JpegDecode, Engine::HwDecoder));
        assert_eq!(s.trust(Task::JpegDecode, 1), Trust::Unverified);
        assert!(s.blocked_info(Task::JpegDecode, 1).is_none());
        assert!(!s.calibrated());

        s.block(Task::JpegDecode, Engine::HwDecoder, "driver error");
        let key = Key {
            variant: 0,
            bucket: 3,
            busy: false,
        };
        s.record(Task::YcbcrToRgb, 1, key, 12.0, 5_000.0, None);
        s.set_calibrated();
        let _ = s.take_dirty();
        s.reset_measurements();
        assert!(s.take_dirty() > 0);
        assert_eq!(s.samples(Task::YcbcrToRgb, 1, key), 0);
        assert_eq!(s.trust(Task::JpegDecode, 1), Trust::Unverified);
        assert_eq!(s.trust(Task::YcbcrToRgb, 1), Trust::Unverified);
        assert_eq!(s.trust(Task::PngUnfilter, 1), Trust::Absent);
        assert!(!s.calibrated());
        assert!(!s.snapshot().any_blocked());
        assert!(!s.to_table().contains("\nblocked "));
        assert!(!s.to_table().contains("\ncell "));
    }

    #[test]
    fn machine_names_come_from_the_fingerprint() {
        let m = Machine::parse(&super::super::fingerprint(
            "AMD Radeon 860M",
            "radv Mesa 25.2",
            "AMD Ryzen AI 7 350",
            "0.1.0",
        ));
        assert_eq!(m.gpu, "AMD Radeon 860M");
        assert_eq!(m.driver, "radv Mesa 25.2");
        assert_eq!(m.cpu, "AMD Ryzen AI 7 350");
        assert_eq!(m.app_version, "0.1.0");
    }
}
