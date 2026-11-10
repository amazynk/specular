//! Background indexing: one installed feature's model over the catalogue, from cached
//! thumbnails (never a second decode of the image).
//!
//! The job walks the catalogue's pending files ([`specular_library::ai::pending`]) in file
//! order, runs the model on each one's cached thumbnail and stores the result, which marks
//! the file done. Work is therefore resumable for free: a job started again (after a cancel,
//! a quit or a crash) continues with what is still pending. Files whose thumbnail is not
//! cached yet are counted as waiting and picked up by a later run, once the gallery or the
//! library has made it.
//!
//! The job yields to the user and the machine, with the same rules as the prefetcher
//! (specular-platform): it pauses while the user browses (the library's hold), in low-power
//! mode or on a low battery, when the machine is hot (Serious or worse) and under critical
//! memory pressure; on battery, when warm (Fair) or under a memory warning it runs at a low
//! duty cycle.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use rusqlite::Connection;
use specular_core::work::{self, Work};
use specular_platform::{MemoryPressure, PowerState, Thermal};
use specular_thumbs::{CLASSES, DiskCache, Stamp, ThumbKey};

use crate::manifest::Feature;
use crate::runtime::{Dispatch, Outputs, Thumb};

/// Why the job is waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pause {
    /// The user is browsing.
    Browsing,
    /// Low-power mode, or on battery with little charge.
    LowEnergy,
    /// The machine is hot.
    Heat,
    /// Memory is short.
    Memory,
    /// The user paused it in the background-work panel.
    User,
}

/// What the machine allows right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    Run,
    /// Run at a low duty cycle.
    Throttle,
    Pause(Pause),
}

/// The scheduling rule.
pub fn gate(power: PowerState, pressure: MemoryPressure, browsing: bool) -> Gate {
    if browsing {
        Gate::Pause(Pause::Browsing)
    } else if power.thermal >= Thermal::Serious {
        Gate::Pause(Pause::Heat)
    } else if pressure == MemoryPressure::Critical {
        Gate::Pause(Pause::Memory)
    } else if power.low_energy() {
        Gate::Pause(Pause::LowEnergy)
    } else if power.on_battery || power.thermal == Thermal::Fair || pressure == MemoryPressure::Warn
    {
        Gate::Throttle
    } else {
        Gate::Run
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Starting,
    Running,
    /// Running at a low duty cycle (on battery, warm, memory warning).
    Throttled,
    Paused(Pause),
    /// Nothing left that has a cached thumbnail.
    Finished,
    Cancelled,
    Failed(String),
}

/// Progress for the UI.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub state: State,
    /// Readable files in the catalogue.
    pub total: u64,
    /// Done by the current model.
    pub done: u64,
    /// Seen in the last pass without a cached thumbnail.
    pub waiting_thumbs: u64,
    /// The model failed on these in the last pass.
    pub failed: u64,
}

impl Progress {
    pub fn active(&self) -> bool {
        matches!(
            self.state,
            State::Starting | State::Running | State::Throttled | State::Paused(_)
        )
    }
}

/// An item of work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub file_id: i64,
    pub pending: specular_library::ai::Pending,
}

/// An owned thumbnail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Where the job's work comes from and goes to.
pub trait WorkSource: Send {
    /// Up to `limit` pending items after file id `after`, in file order.
    fn pending(&mut self, after: i64, limit: usize) -> Result<Vec<Item>, String>;
    /// (total, done).
    fn counts(&mut self) -> Result<(u64, u64), String>;
    /// The item's cached thumbnail, at least `side` pixels where cached.
    fn thumbnail(&mut self, item: &Item, side: u32) -> Option<Image>;
    fn store(&mut self, item: &Item, out: Outputs) -> Result<(), String>;
}

/// Decodes the photo at `path` with its long side at most `side` pixels (None: unreadable).
/// Features that need more than a thumbnail (faces) read photos through it.
pub type Decode = Arc<dyn Fn(&std::path::Path, u32) -> Option<Image> + Send + Sync>;

/// The catalogue and the thumbnail cache.
pub struct LibrarySource {
    conn: Connection,
    cache: Arc<DiskCache>,
    flags: u32,
    feature: Feature,
    model: String,
    decode: Option<Decode>,
}

impl LibrarySource {
    /// `conn` is the pack's own connection ([`specular_library::ai::open`]); `flags` the
    /// thumbnail engine's cache-key flags.
    pub fn new(
        conn: Connection,
        cache: Arc<DiskCache>,
        flags: u32,
        feature: Feature,
        model: &str,
    ) -> LibrarySource {
        LibrarySource {
            conn,
            cache,
            flags,
            feature,
            model: model.into(),
            decode: None,
        }
    }

    /// Reads photos larger than the cached thumbnails through `decode` (falling back to the
    /// cache when it gives nothing).
    pub fn with_decoder(mut self, decode: Decode) -> LibrarySource {
        self.decode = Some(decode);
        self
    }
}

/// Size classes to try for `side`: the smallest that covers it first, then larger, then
/// smaller (a smaller thumbnail beats none).
fn classes_for(side: u32) -> Vec<u32> {
    let first = CLASSES
        .iter()
        .position(|&c| c >= side)
        .unwrap_or(CLASSES.len() - 1);
    let mut out: Vec<u32> = CLASSES[first..].to_vec();
    out.extend(CLASSES[..first].iter().rev());
    out
}

impl WorkSource for LibrarySource {
    fn pending(&mut self, after: i64, limit: usize) -> Result<Vec<Item>, String> {
        specular_library::ai::pending(&self.conn, self.feature.id(), &self.model, after, limit)
            .map(|v| {
                v.into_iter()
                    .map(|p| Item {
                        file_id: p.file_id,
                        pending: p,
                    })
                    .collect()
            })
            .map_err(|e| e.to_string())
    }

    fn counts(&mut self) -> Result<(u64, u64), String> {
        specular_library::ai::counts(&self.conn, self.feature.id(), &self.model)
            .map_err(|e| e.to_string())
    }

    fn thumbnail(&mut self, item: &Item, side: u32) -> Option<Image> {
        if let Some(d) = &self.decode
            && let Ok(Some(path)) = specular_library::ai::file_path(&self.conn, item.file_id)
            && let Some(img) = d(&path, side)
        {
            return Some(img);
        }
        let p = &item.pending;
        let key = ThumbKey {
            stamp: Stamp {
                len: p.size,
                mtime_ns: p.mtime_ns,
            },
            content: p.content?,
        };
        classes_for(side).into_iter().find_map(|class| {
            let e = self.cache.get(&key, class, self.flags)?;
            Some(Image {
                width: e.pixels.width,
                height: e.pixels.height,
                rgba: e.pixels.rgba,
            })
        })
    }

    fn store(&mut self, item: &Item, out: Outputs) -> Result<(), String> {
        use specular_library::ai;
        let id = item.file_id;
        let m = &self.model;
        match out {
            Outputs::Embedding(v) => ai::put_embedding(&self.conn, id, m, &v),
            Outputs::Text { text, boxes } => ai::put_text(&self.conn, id, m, &text, &boxes, "?"),
            Outputs::Scores(s) => ai::put_scores(&self.conn, id, m, &s),
            Outputs::Faces(f) => {
                let faces: Vec<ai::Face> = f
                    .into_iter()
                    .map(|f| ai::Face {
                        x: f.x,
                        y: f.y,
                        w: f.w,
                        h: f.h,
                        score: f.score,
                        vec: f.vec,
                        roll: f.roll,
                    })
                    .collect();
                ai::put_faces(&mut self.conn, id, m, &faces)
            }
        }
        .map_err(|e| e.to_string())
    }
}

/// What the job asks of its surroundings.
#[derive(Clone)]
pub struct Env {
    /// The user is browsing (`Library::held`).
    pub browsing: Arc<dyn Fn() -> bool + Send + Sync>,
    /// Power and memory (`specular_platform::power_state`, `memory_pressure`).
    pub machine: Arc<dyn Fn() -> (PowerState, MemoryPressure) + Send + Sync>,
    /// Called when progress changed (the app redraws Settings).
    pub wake: Arc<dyn Fn() + Send + Sync>,
    /// How long to wait before looking again while paused.
    pub pause_poll: Duration,
    /// Idle time per unit of work time when throttled (4: a 20 % duty cycle).
    pub throttle_ratio: u32,
    /// Called first on the job's thread (the app lowers its priority).
    pub on_thread_start: Option<fn()>,
    /// The job embeds again for a newly chosen model size (the background-work panel says
    /// so instead of a first indexing).
    pub switching: bool,
}

impl Env {
    /// The platform's power and memory state, never browsing.
    pub fn platform() -> Env {
        Env {
            browsing: Arc::new(|| false),
            machine: Arc::new(|| {
                (
                    specular_platform::power_state(),
                    specular_platform::memory_pressure(),
                )
            }),
            wake: Arc::new(|| {}),
            pause_poll: Duration::from_millis(500),
            throttle_ratio: 4,
            on_thread_start: None,
            switching: false,
        }
    }
}

const BATCH: usize = 64;

struct Shared {
    progress: Mutex<Progress>,
    cancel: AtomicBool,
    /// The job in the background-work panel until it ends ([`mirror`]).
    work: Mutex<Option<Work>>,
    kind: work::Kind,
}

/// A running job (stopped and joined on drop).
pub struct Job {
    feature: Feature,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Job {
    pub fn start(source: Box<dyn WorkSource>, dispatch: Dispatch, env: Env) -> Job {
        let feature = dispatch.runtime().feature();
        let (kind, step) = work_of(feature, env.switching);
        let w = Work::start(kind, step);
        w.set_pausable(true);
        let shared = Arc::new(Shared {
            progress: Mutex::new(Progress::default()),
            cancel: AtomicBool::new(false),
            work: Mutex::new(Some(w)),
            kind,
        });
        let sh = shared.clone();
        let thread = specular_thread::builder(format!("ai-{feature}"))
            .spawn(move || {
                if let Some(f) = env.on_thread_start {
                    f();
                }
                // A panic outside the per-file boundary (the catalogue, the cache) stops the
                // job as failed instead of leaving it shown as running.
                let what = format_args!("{feature} job");
                if let Err(p) =
                    specular_core::isolate::run(what, || run(source, &dispatch, &env, &sh))
                {
                    set(&sh, &env, |pr| pr.state = State::Failed(p.brief()));
                }
                (env.wake)();
            })
            .ok();
        Job {
            feature,
            shared,
            thread,
        }
    }

    pub fn feature(&self) -> Feature {
        self.feature
    }

    pub fn progress(&self) -> Progress {
        self.shared.progress.lock().clone()
    }

    /// Asks the job to stop after the current item.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }

    /// Whether the thread has ended.
    pub fn ended(&self) -> bool {
        self.thread.as_ref().is_none_or(|t| t.is_finished())
    }

    /// Waits for the thread (tests). False on timeout.
    pub fn wait(&self, timeout: Duration) -> bool {
        let t = Instant::now();
        while !self.ended() {
            if t.elapsed() > timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        true
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn set(sh: &Shared, env: &Env, f: impl FnOnce(&mut Progress)) {
    let changed = {
        let mut p = sh.progress.lock();
        let before = p.clone();
        f(&mut p);
        (*p != before).then(|| p.clone())
    };
    if let Some(p) = changed {
        mirror(sh, env, &p);
        (env.wake)();
    }
}

/// The background-work panel's job ([`specular_core::work`]) and its first step, by feature.
fn work_of(feature: Feature, switching: bool) -> (work::Kind, work::Step) {
    match feature {
        Feature::Search if switching => (work::Kind::AiSearch, work::Step::Switching),
        Feature::Search => (work::Kind::AiSearch, work::Step::Embedding),
        Feature::Text => (work::Kind::Text, work::Step::Ocr),
        Feature::Culling => (work::Kind::Culling, work::Step::Scoring),
        Feature::People => (work::Kind::Faces, work::Step::Faces),
        // Never indexed: the app's transcription reports itself.
        Feature::Speech => (work::Kind::Speech, work::Step::Transcribing),
        // Never indexed either: Develop computes a mask when a row asks for one.
        Feature::Masks => (work::Kind::Download, work::Step::Downloading),
        // Nor Denoise: Develop runs it on the photo it shows or exports.
        Feature::Denoise => (work::Kind::Download, work::Step::Downloading),
        Feature::Remove | Feature::Depth => (work::Kind::Download, work::Step::Downloading),
    }
}

/// Mirrors the job's progress into the background-work panel; the panel's job goes when
/// this one ends.
fn mirror(sh: &Shared, env: &Env, p: &Progress) {
    let mut w = sh.work.lock();
    if !p.active() {
        *w = None;
        return;
    }
    let Some(w) = w.as_ref() else { return };
    let total = (p.total > 0).then_some(p.total);
    w.set_progress(p.done, total);
    w.set_paused(match &p.state {
        State::Paused(why) => Some(match why {
            Pause::Browsing => work::Pause::Browsing,
            Pause::Heat => work::Pause::Heat,
            Pause::Memory => work::Pause::Memory,
            Pause::User => work::Pause::User,
            Pause::LowEnergy if (env.machine)().0.low_power => work::Pause::LowPower,
            Pause::LowEnergy => work::Pause::Battery,
        }),
        _ => None,
    });
}

/// The scheduling rule, and the user's pause from the background-work panel before it.
fn gate_now(env: &Env, kind: work::Kind) -> Gate {
    if work::user_paused(kind) {
        return Gate::Pause(Pause::User);
    }
    let (power, pressure) = (env.machine)();
    gate(power, pressure, (env.browsing)())
}

fn run(mut source: Box<dyn WorkSource>, dispatch: &Dispatch, env: &Env, sh: &Shared) {
    let side = dispatch.runtime().input_side();
    let refresh = |source: &mut Box<dyn WorkSource>| match source.counts() {
        Ok((total, done)) => set(sh, env, |p| {
            p.total = total;
            p.done = done;
        }),
        Err(e) => set(sh, env, |p| p.state = State::Failed(e)),
    };
    refresh(&mut source);
    let mut cursor = 0i64;
    // This pass: items done, waiting for thumbnails, failed.
    let (mut did, mut waiting, mut failed) = (0u64, 0u64, 0u64);
    loop {
        if sh.cancel.load(Ordering::Relaxed) {
            set(sh, env, |p| p.state = State::Cancelled);
            return;
        }
        let g = gate_now(env, sh.kind);
        if let Gate::Pause(why) = g {
            set(sh, env, |p| p.state = State::Paused(why));
            std::thread::sleep(env.pause_poll);
            continue;
        }
        let batch = match source.pending(cursor, BATCH) {
            Ok(b) => b,
            Err(e) => {
                set(sh, env, |p| p.state = State::Failed(e));
                return;
            }
        };
        if batch.is_empty() {
            set(sh, env, |p| {
                p.waiting_thumbs = waiting;
                p.failed = failed;
            });
            if did == 0 {
                refresh(&mut source);
                set(sh, env, |p| p.state = State::Finished);
                return;
            }
            // Another pass: thumbnails may have come for files skipped.
            cursor = 0;
            (did, waiting, failed) = (0, 0, 0);
            continue;
        }
        for item in batch {
            cursor = item.file_id;
            if sh.cancel.load(Ordering::Relaxed) {
                break;
            }
            let g = gate_now(env, sh.kind);
            if matches!(g, Gate::Pause(_)) {
                // Look at this item again after the pause.
                cursor = item.file_id - 1;
                break;
            }
            set(sh, env, |p| {
                p.state = if g == Gate::Throttle {
                    State::Throttled
                } else {
                    State::Running
                }
            });
            let t0 = Instant::now();
            let Some(img) = source.thumbnail(&item, side) else {
                waiting += 1;
                continue;
            };
            let thumb = Thumb {
                width: img.width,
                height: img.height,
                rgba: &img.rgba,
            };
            // A model that panics on one thumbnail fails that file, like a model error.
            let what = format_args!("{} on file {}", dispatch.runtime().model(), item.file_id);
            let ran = specular_core::isolate::run(what, || dispatch.run(&thumb))
                .unwrap_or_else(|p| Err(crate::runtime::RunError::Failed(p.brief())));
            match ran {
                Ok(out) => match source.store(&item, out) {
                    Ok(()) => {
                        did += 1;
                        set(sh, env, |p| p.done += 1);
                    }
                    Err(e) => {
                        set(sh, env, |p| p.state = State::Failed(e));
                        return;
                    }
                },
                Err(e) => {
                    failed += 1;
                    specular_log::warn!(target: specular_log::JOBS,
                        "ai: {} on file {}: {e}", dispatch.runtime().model(), item.file_id);
                }
            }
            if g == Gate::Throttle {
                std::thread::sleep(t0.elapsed() * env.throttle_ratio.max(1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn power(f: impl FnOnce(&mut PowerState)) -> PowerState {
        let mut p = PowerState::default();
        f(&mut p);
        p
    }

    #[test]
    fn gate_follows_the_power_rules() {
        let n = MemoryPressure::Normal;
        assert_eq!(gate(PowerState::default(), n, false), Gate::Run);
        assert_eq!(
            gate(PowerState::default(), n, true),
            Gate::Pause(Pause::Browsing)
        );
        assert_eq!(
            gate(power(|p| p.thermal = Thermal::Serious), n, false),
            Gate::Pause(Pause::Heat)
        );
        assert_eq!(
            gate(power(|p| p.thermal = Thermal::Fair), n, false),
            Gate::Throttle
        );
        assert_eq!(
            gate(power(|p| p.low_power = true), n, false),
            Gate::Pause(Pause::LowEnergy)
        );
        assert_eq!(
            gate(power(|p| p.on_battery = true), n, false),
            Gate::Throttle
        );
        assert_eq!(
            gate(
                power(|p| {
                    p.on_battery = true;
                    p.battery_percent = Some(15);
                }),
                n,
                false
            ),
            Gate::Pause(Pause::LowEnergy)
        );
        assert_eq!(
            gate(PowerState::default(), MemoryPressure::Warn, false),
            Gate::Throttle
        );
        assert_eq!(
            gate(PowerState::default(), MemoryPressure::Critical, false),
            Gate::Pause(Pause::Memory)
        );
    }

    #[test]
    fn classes_prefer_the_covering_size() {
        assert_eq!(classes_for(128), vec![128, 256, 512]);
        assert_eq!(classes_for(224), vec![256, 512, 128]);
        assert_eq!(classes_for(1000), vec![512, 256, 128]);
    }
}
