//! What runs in the background, for the "Background work" indicator and panel.
//!
//! Every long background job (the Library's indexer, Local AI's indexing, the similar-images
//! scan, a model download) holds a [`Work`] while it runs and reports into it: its current
//! [`Step`], the item it is on, how far it is (`done` of `total`, or an unknown total), why it
//! waits ([`Pause`]) and what comes after it. Dropping the [`Work`] ends it. The app reads a
//! [`snapshot`] when [`generation`] changed and words it; nothing here is localised.
//!
//! Reporting is data only and cheap: one short lock per call, no allocation unless the item
//! changed. The registry wakes the app ([`set_waker`]) when a job starts, ends, changes step or
//! pause, and at most every [`WAKE_EVERY`] for progress.
//!
//! The rate behind "About 25 minutes left" is kept here ([`Rate`]): exponentially weighted
//! over [`TAU`], from samples at least [`SAMPLE_EVERY`] apart, restarted by a new step or a
//! pause (the time paused never counts), and only given once [`MIN_SAMPLES`] samples
//! over [`MIN_SPAN`] were seen.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// The app is woken for progress at most this often.
pub const WAKE_EVERY: Duration = Duration::from_millis(500);
/// Rate samples are at least this far apart.
pub const SAMPLE_EVERY: Duration = Duration::from_secs(1);
/// The rate's time constant.
pub const TAU: Duration = Duration::from_secs(30);
/// An estimate needs this many samples ...
pub const MIN_SAMPLES: u32 = 3;
/// ... spanning at least this long.
pub const MIN_SPAN: Duration = Duration::from_secs(5);

/// Which job: its name in the indicator and the settings page it links to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// The Library's indexer: finding and reading photos, signatures, groups.
    Library,
    /// Local AI: search by description.
    AiSearch,
    /// Local AI: faces.
    Faces,
    /// Local AI: text in photos (OCR).
    Text,
    /// Local AI: culling scores.
    Culling,
    /// Local AI: a model being downloaded.
    Download,
    /// The gallery's similar-images scan.
    Similar,
    /// Local AI: subtitles generated from a movie's speech.
    Speech,
}

/// What the job is doing now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// Walking folders for photos.
    Finding,
    /// Reading photos' facts (dimensions, dates, camera).
    Reading,
    /// Making thumbnails and similar-shot signatures.
    Signatures,
    /// Grouping duplicates and similar shots.
    Grouping,
    /// Rebuilding the text index.
    TextIndex,
    /// Learning what is in photos (search embeddings).
    Embedding,
    /// Embedding again with a newly chosen model size.
    Switching,
    /// Finding faces.
    Faces,
    /// Reading text in photos.
    Ocr,
    /// Scoring photos for culling.
    Scoring,
    /// Downloading a model.
    Downloading,
    /// Measuring sharpness for best picks (similar scan).
    Focus,
    /// Turning speech into subtitles.
    Transcribing,
}

/// What `done` and `total` count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Unit {
    #[default]
    Photos,
    Bytes,
    /// Anything else (rows, groups).
    Items,
}

/// Why a job waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pause {
    /// The user is browsing.
    Browsing,
    /// On battery with little charge.
    Battery,
    /// The system's low-power mode.
    LowPower,
    /// The machine is hot.
    Heat,
    /// Memory is short.
    Memory,
    /// The user paused it.
    User,
}

/// A job as the panel shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub id: u64,
    pub kind: Kind,
    pub step: Step,
    /// The file or folder it is on (a name, not a path).
    pub item: Option<String>,
    pub done: u64,
    /// None: unknown.
    pub total: Option<u64>,
    pub unit: Unit,
    pub paused: Option<Pause>,
    /// What it does after this step, in order.
    pub then: Vec<Step>,
    /// The user can pause and resume it ([`set_user_paused`]).
    pub pausable: bool,
    pub started: Instant,
    /// When the time left was worked out.
    pub at: Instant,
    rate: Rate,
}

impl Job {
    /// New progress; false when nothing changed.
    fn apply(&mut self, done: u64, total: Option<u64>, now: Instant) -> bool {
        if self.done == done && self.total == total {
            return false;
        }
        self.done = done;
        self.total = total;
        if self.paused.is_none() {
            self.rate.sample(done, now);
            self.at = now;
        }
        true
    }

    /// Items left (None: unknown total).
    pub fn left(&self) -> Option<u64> {
        self.total.map(|t| t.saturating_sub(self.done))
    }

    /// The time left: None while paused, without a total, or with too few samples yet
    /// ("Estimating…"); see [`Job::estimating`].
    pub fn eta(&self) -> Option<Duration> {
        if self.paused.is_some() {
            return None;
        }
        self.rate.eta(self.left()?)
    }

    /// Whether the time left is still being estimated (running, with a total).
    pub fn estimating(&self) -> bool {
        self.paused.is_none() && self.total.is_some() && self.eta().is_none()
    }

    /// Done as a fraction of the total (None: unknown).
    pub fn fraction(&self) -> Option<f32> {
        let t = self.total?;
        Some(if t == 0 {
            1.0
        } else {
            (self.done.min(t) as f64 / t as f64) as f32
        })
    }
}

/// An exponentially weighted rate of progress (items per second).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rate {
    /// The last sample: when and how many were done.
    last: Option<(Instant, u64)>,
    first: Option<Instant>,
    per_s: f64,
    samples: u32,
}

impl Rate {
    /// Forgets the samples (a new step, a pause).
    pub fn restart(&mut self) {
        *self = Rate::default();
    }

    /// Notes `done` at `now`.
    pub fn sample(&mut self, done: u64, now: Instant) {
        let Some((t0, d0)) = self.last else {
            self.last = Some((now, done));
            self.first = Some(now);
            return;
        };
        let dt = now.saturating_duration_since(t0);
        if dt < SAMPLE_EVERY {
            return;
        }
        if done < d0 {
            // Counted again from the start (a new pass): begin again.
            self.restart();
            self.last = Some((now, done));
            self.first = Some(now);
            return;
        }
        let r = (done - d0) as f64 / dt.as_secs_f64();
        let a = 1.0 - (-dt.as_secs_f64() / TAU.as_secs_f64()).exp();
        self.per_s = if self.samples == 0 {
            r
        } else {
            self.per_s + a * (r - self.per_s)
        };
        self.samples += 1;
        self.last = Some((now, done));
    }

    /// Items per second, once enough samples were seen.
    pub fn per_second(&self) -> Option<f64> {
        let (first, (last, _)) = (self.first?, self.last?);
        (self.samples >= MIN_SAMPLES
            && last.saturating_duration_since(first) >= MIN_SPAN
            && self.per_s > 0.0)
            .then_some(self.per_s)
    }

    /// The time `left` items take at this rate.
    pub fn eta(&self, left: u64) -> Option<Duration> {
        let r = self.per_second()?;
        Some(Duration::from_secs_f64((left as f64 / r).min(1e9)))
    }
}

/// Reads a job's (done, total) where the work keeps its own counters ([`Work::follow`]).
type Source = Arc<dyn Fn() -> (u64, Option<u64>) + Send + Sync>;

struct Reg {
    jobs: Vec<Job>,
    /// Jobs whose progress is read when a snapshot is taken, by job id.
    sources: Vec<(u64, Source)>,
    user_paused: Vec<Kind>,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
    last_wake: Option<Instant>,
}

static REG: Mutex<Reg> = Mutex::new(Reg {
    jobs: Vec::new(),
    sources: Vec::new(),
    user_paused: Vec::new(),
    waker: None,
    last_wake: None,
});
static GEN: AtomicU64 = AtomicU64::new(0);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Called when what the panel shows changed (the app redraws).
pub fn set_waker(f: Arc<dyn Fn() + Send + Sync>) {
    REG.lock().waker = Some(f);
}

/// Bumped by every change: read a [`snapshot`] when it moved.
pub fn generation() -> u64 {
    GEN.load(Ordering::Acquire)
}

/// The jobs running, oldest first.
pub fn snapshot() -> Vec<Job> {
    let mut r = REG.lock();
    if !r.sources.is_empty() {
        let now = Instant::now();
        let read: Vec<(u64, (u64, Option<u64>))> =
            r.sources.iter().map(|(id, f)| (*id, f())).collect();
        for (id, (done, total)) in read {
            if let Some(j) = r.jobs.iter_mut().find(|j| j.id == id) {
                j.apply(done, total, now);
            }
        }
    }
    r.jobs.clone()
}

/// Whether the user paused jobs of `kind` (they stay paused across passes until resumed).
pub fn user_paused(kind: Kind) -> bool {
    REG.lock().user_paused.contains(&kind)
}

/// Pauses or resumes jobs of `kind` (the panel's button).
pub fn set_user_paused(kind: Kind, paused: bool) {
    let mut r = REG.lock();
    let was = r.user_paused.contains(&kind);
    let changed = was != paused;
    if paused && !was {
        r.user_paused.push(kind);
    } else if !paused {
        r.user_paused.retain(|k| *k != kind);
    }
    if changed {
        let now = Instant::now();
        for j in r.jobs.iter_mut().filter(|j| j.kind == kind && j.pausable) {
            // Shown at once; the job confirms when it next looks.
            if paused {
                j.paused = Some(Pause::User);
            } else if j.paused == Some(Pause::User) {
                j.paused = None;
            }
            j.rate.restart();
            j.at = now;
        }
        wake(r, true);
    }
}

/// Bumps the generation and calls the waker (now when `force`, else throttled) after
/// unlocking.
fn wake(mut r: parking_lot::MutexGuard<'_, Reg>, force: bool) {
    GEN.fetch_add(1, Ordering::AcqRel);
    let now = Instant::now();
    if !force
        && r.last_wake
            .is_some_and(|t| now.duration_since(t) < WAKE_EVERY)
    {
        return;
    }
    r.last_wake = Some(now);
    let w = r.waker.clone();
    drop(r);
    if let Some(w) = w {
        w();
    }
}

/// A running job; dropping it ends the job.
#[derive(Debug)]
#[must_use = "the job ends when this drops"]
pub struct Work {
    id: u64,
    kind: Kind,
}

impl Work {
    /// Starts a job of `kind` at `step`.
    pub fn start(kind: Kind, step: Step) -> Work {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let now = Instant::now();
        let mut r = REG.lock();
        r.jobs.push(Job {
            id,
            kind,
            step,
            item: None,
            done: 0,
            total: None,
            unit: Unit::Photos,
            paused: None,
            then: Vec::new(),
            pausable: false,
            started: now,
            at: now,
            rate: Rate::default(),
        });
        wake(r, true);
        Work { id, kind }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Changes the job (the closure gets it locked); `force` wakes the app now.
    fn edit(&self, force: bool, f: impl FnOnce(&mut Job) -> bool) {
        let mut r = REG.lock();
        let Some(j) = r.jobs.iter_mut().find(|j| j.id == self.id) else {
            return;
        };
        if f(j) {
            wake(r, force);
        }
    }

    /// A new step (the rate starts again), with its total if known.
    pub fn set_step(&self, step: Step, total: Option<u64>) {
        self.edit(true, |j| {
            if j.step == step && j.total == total {
                return false;
            }
            if j.step != step {
                j.step = step;
                j.done = 0;
                j.item = None;
                j.rate.restart();
                j.at = Instant::now();
            }
            j.total = total;
            true
        });
    }

    pub fn set_unit(&self, unit: Unit) {
        self.edit(false, |j| std::mem::replace(&mut j.unit, unit) != unit);
    }

    /// The file or folder it is on (its name).
    pub fn set_item(&self, item: Option<&str>) {
        self.edit(false, |j| {
            if j.item.as_deref() == item {
                return false;
            }
            j.item = item.map(str::to_owned);
            true
        });
    }

    /// How far it is.
    pub fn set_progress(&self, done: u64, total: Option<u64>) {
        self.edit(false, |j| j.apply(done, total, Instant::now()));
    }

    /// Progress read from the work's own counters whenever the panel looks (a download's
    /// bytes), instead of [`Work::set_progress`] calls: `f` must be cheap (atomics).
    pub fn follow(&self, f: impl Fn() -> (u64, Option<u64>) + Send + Sync + 'static) {
        let mut r = REG.lock();
        r.sources.retain(|(id, _)| *id != self.id);
        r.sources.push((self.id, Arc::new(f)));
    }

    /// Why it waits (None: it runs). A pause restarts the rate: the time paused never counts.
    pub fn set_paused(&self, why: Option<Pause>) {
        self.edit(true, |j| {
            if j.paused == why {
                return false;
            }
            j.paused = why;
            j.rate.restart();
            j.at = Instant::now();
            true
        });
    }

    /// What it does after this step.
    pub fn set_then(&self, then: &[Step]) {
        self.edit(false, |j| {
            if j.then == then {
                return false;
            }
            j.then = then.to_vec();
            true
        });
    }

    /// The user can pause it: it then looks at [`Work::user_paused`] between items.
    pub fn set_pausable(&self, yes: bool) {
        self.edit(true, |j| std::mem::replace(&mut j.pausable, yes) != yes);
    }

    /// Whether the user paused this kind of job.
    pub fn user_paused(&self) -> bool {
        user_paused(self.kind)
    }
}

impl Drop for Work {
    fn drop(&mut self) {
        let mut r = REG.lock();
        let n = r.jobs.len();
        r.jobs.retain(|j| j.id != self.id);
        r.sources.retain(|(id, _)| *id != self.id);
        if r.jobs.len() != n {
            wake(r, true);
        }
    }
}

/// Puts a made-up job in the registry (screenshots, tests): `rate` items per second, as if
/// measured over a minute.
pub fn fake(kind: Kind, step: Step, done: u64, total: Option<u64>, rate: f64) -> Work {
    let w = Work::start(kind, step);
    let now = Instant::now();
    w.edit(true, |j| {
        j.done = done;
        j.total = total;
        if rate > 0.0 {
            j.rate = Rate {
                last: Some((now, done)),
                first: now.checked_sub(Duration::from_secs(60)),
                per_s: rate,
                samples: MIN_SAMPLES,
            };
        }
        true
    });
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_needs_samples_and_restarts() {
        let t0 = Instant::now();
        let mut r = Rate::default();
        r.sample(0, t0);
        assert_eq!(r.eta(100), None);
        for s in 1..=6u64 {
            r.sample(s * 10, t0 + Duration::from_secs(s));
        }
        let per = r.per_second().unwrap();
        assert!((per - 10.0).abs() < 1e-9, "{per}");
        assert_eq!(r.eta(100), Some(Duration::from_secs(10)));
        // Too close together: ignored.
        r.sample(1000, t0 + Duration::from_millis(6100));
        assert!((r.per_second().unwrap() - 10.0).abs() < 1e-9);
        r.restart();
        assert_eq!(r.per_second(), None);
    }

    #[test]
    fn rate_is_weighted_towards_the_recent() {
        let t0 = Instant::now();
        let mut r = Rate::default();
        r.sample(0, t0);
        let mut done = 0;
        for s in 1..=10u64 {
            done += 10;
            r.sample(done, t0 + Duration::from_secs(s));
        }
        // Then twice as fast for a minute.
        for s in 11..=70u64 {
            done += 20;
            r.sample(done, t0 + Duration::from_secs(s));
        }
        let per = r.per_second().unwrap();
        assert!(per > 17.0 && per < 20.0, "{per}");
    }

    #[test]
    fn jobs_report_and_end() {
        let g0 = generation();
        let w = Work::start(Kind::Similar, Step::Signatures);
        assert!(generation() > g0);
        w.set_progress(5, Some(50));
        w.set_item(Some("IMG_0001.JPG"));
        w.set_then(&[Step::Focus]);
        let j = snapshot().into_iter().find(|j| j.id == w.id()).unwrap();
        assert_eq!((j.done, j.total, j.left()), (5, Some(50), Some(45)));
        assert_eq!(j.item.as_deref(), Some("IMG_0001.JPG"));
        assert_eq!(j.then, vec![Step::Focus]);
        assert!(j.estimating());
        w.set_paused(Some(Pause::Browsing));
        let j = snapshot().into_iter().find(|j| j.id == w.id()).unwrap();
        assert!(!j.estimating() && j.eta().is_none(), "no time while paused");
        let id = w.id();
        drop(w);
        assert!(snapshot().iter().all(|j| j.id != id));
    }

    #[test]
    fn user_pause_is_per_kind_and_shown_at_once() {
        let w = Work::start(Kind::Culling, Step::Scoring);
        w.set_pausable(true);
        set_user_paused(Kind::Culling, true);
        assert!(w.user_paused());
        let j = snapshot().into_iter().find(|j| j.id == w.id()).unwrap();
        assert_eq!(j.paused, Some(Pause::User));
        set_user_paused(Kind::Culling, false);
        assert!(!w.user_paused());
        let j = snapshot().into_iter().find(|j| j.id == w.id()).unwrap();
        assert_eq!(j.paused, None);
    }

    #[test]
    fn a_fake_job_has_an_estimate() {
        let w = fake(Kind::Faces, Step::Faces, 1240, Some(9800), 6.0);
        let j = snapshot().into_iter().find(|j| j.id == w.id()).unwrap();
        let eta = j.eta().unwrap().as_secs();
        assert_eq!(eta, 8560 / 6);
    }
}
