//! Download activity: every request the app sends to a server reports here, so the window
//! can show what is downloading, how fast and how long it has left (the "Downloads"
//! indicator and panel).
//!
//! Requests are grouped by what they fetch and from where ([`Label`]): the hundred tile
//! requests of one map pan are one [`Activity`] with counts; a model download is one with
//! bytes and a total. An activity is shown while a request of it runs, then lingers
//! [`LINGER`] (finished) or [`FAILED_LINGER`] (failed, cancelled) and goes away. The next
//! request after that starts a new one, counted from zero.
//!
//! Cheap on the hot path: a chunk is one atomic add ([`Request::add`]); a request's start
//! and end take a short lock. Speed is an exponentially weighted average over [`SPEED_TAU`],
//! sampled when the app takes a [`Registry::snapshot`] (a few times a second while
//! something runs, never while nothing does).
//!
//! Nothing here is localised and nothing leaves the machine: the label carries the host and
//! a short description, never a URL path, a query string or credentials.

use std::sync::atomic::{AtomicU64, Ordering};
pub mod off;

use std::sync::{Arc, OnceLock, Weak};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// How long a finished activity stays on screen.
pub const LINGER: Duration = Duration::from_secs(2);
/// How long a failed or cancelled one stays (long enough to read why).
pub const FAILED_LINGER: Duration = Duration::from_secs(5);
/// The speed's averaging time constant.
pub const SPEED_TAU: Duration = Duration::from_secs(3);
/// Samples closer than this are skipped (the rate of a few milliseconds is noise).
pub const SPEED_SAMPLE: Duration = Duration::from_millis(200);
/// Requests of a cancelled activity that start this soon after the cancel are cancelled
/// too (a downloader's fallback to a mirror, a tile loader asking again at once).
pub const CANCEL_WINDOW: Duration = Duration::from_secs(1);
/// Progress wakes the app at most this often (starts and ends wake it at once).
pub const WAKE_EVERY: Duration = Duration::from_millis(250);

/// What is being fetched (the app words it).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum What {
    /// Anything not described more closely ("Data from <host>").
    Data,
    /// Tiles of a remote image: a cloud-optimised GeoTIFF, an OMERO image.
    ImageTiles,
    /// A remote point cloud (COPC).
    PointCloud,
    /// Base-map tiles.
    MapTiles,
    /// A map service: capabilities, features, a map image.
    MapService,
    /// DICOMweb searches and metadata.
    Dicom,
    /// A DICOM series streamed in, by its description.
    DicomSeries(String),
    /// Studies sent to a PACS (C-STORE or STOW-RS): bytes count what was sent.
    DicomSend,
    /// OMERO browsing, thumbnails and metadata.
    Omero,
    /// Chunks of a remote Zarr array (an OME-Zarr image on a server or in a bucket).
    ZarrChunks,
    /// The metadata of a remote Zarr store (groups, arrays, listings).
    ZarrMetadata,
    /// A Local AI model, by its name.
    Model(String),
    /// The update check.
    UpdateCheck,
    /// Tiles of a sky survey (HiPS) shown around an astronomical image.
    SkySurvey,
    /// A network stream the user opened: a file, HLS or DASH segments, internet radio.
    Stream,
}

/// What and from where: requests with the same label are one activity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Label {
    pub what: What,
    /// The server's host (and port, when not the default), never more of the URL.
    pub host: String,
}

impl Label {
    pub fn new(what: What, url: &str) -> Label {
        Label {
            what,
            host: host_of(url),
        }
    }
}

/// Why a request failed (the app words it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// No connection: the server could not be reached.
    Offline,
    Timeout,
    /// The server answered with this HTTP status.
    Status(u16),
    /// The server refused the credentials.
    Auth,
    Cancelled,
    /// Anything else (a broken response, a disk error).
    Other,
}

/// Counted progress a server reports (a DICOM retrieval's sub-operations): items ended
/// well, with a failure or with a warning, and still to come.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Items {
    pub completed: u32,
    pub failed: u32,
    pub warning: u32,
    pub remaining: u32,
}

impl Items {
    pub fn ended(&self) -> u32 {
        self.completed + self.failed + self.warning
    }
    pub fn total(&self) -> u32 {
        self.ended() + self.remaining
    }
}

/// Where an activity is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    /// Every request ended; some may have failed ([`Activity::failed`]).
    Done,
    /// Every request failed; the last one's reason.
    Failed(Failure),
    /// Stopped from the panel (or by whoever started it).
    Cancelled,
}

/// An activity as the app shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Activity {
    pub id: u64,
    pub label: Label,
    pub state: State,
    /// Bytes received, including a resumed download's part from before.
    pub bytes: u64,
    /// Bytes expected in all, once every request's length is known.
    pub total: Option<u64>,
    /// Requests started, ended (either way), and failed.
    pub requests: u32,
    pub finished: u32,
    pub failed: u32,
    /// Bytes per second, once measured (only while running).
    pub speed: Option<f64>,
    /// Time left, for a single request with a known length at a measured speed.
    pub eta: Option<Duration>,
    /// How far: bytes of the total for one request, requests ended of started for many;
    /// `None`: unknown (an indeterminate bar).
    pub fraction: Option<f32>,
    pub elapsed: Duration,
    /// The panel offers Stop.
    pub cancellable: bool,
    /// Counted progress, when the server reports it ([`Request::items`]).
    pub items: Option<Items>,
}

impl Activity {
    /// Many requests (shown with counts) rather than one download (shown with bytes).
    pub fn grouped(&self) -> bool {
        self.requests > 1
    }
}

/// The host of `url` (with its port when given), without user info, path or query; empty
/// for something that is not a URL.
pub fn host_of(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else {
        return String::new();
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    host.to_ascii_lowercase()
}

/// An exponentially weighted speed.
#[derive(Clone, Copy, Debug, Default)]
struct Speed {
    last: Option<(Instant, u64)>,
    per_s: f64,
    samples: u32,
}

impl Speed {
    fn sample(&mut self, bytes: u64, now: Instant) {
        let Some((t0, b0)) = self.last else {
            self.last = Some((now, bytes));
            return;
        };
        let dt = now.saturating_duration_since(t0);
        if dt < SPEED_SAMPLE {
            return;
        }
        if bytes < b0 {
            // A retried attempt took its bytes back: measure again from here.
            *self = Speed {
                last: Some((now, bytes)),
                ..Speed::default()
            };
            return;
        }
        let r = (bytes - b0) as f64 / dt.as_secs_f64();
        let a = 1.0 - (-dt.as_secs_f64() / SPEED_TAU.as_secs_f64()).exp();
        self.per_s = if self.samples == 0 {
            r
        } else {
            self.per_s + a * (r - self.per_s)
        };
        self.samples += 1;
        self.last = Some((now, bytes));
    }

    fn get(&self) -> Option<f64> {
        (self.samples > 0 && self.per_s > 0.0).then_some(self.per_s)
    }
}

#[derive(Debug)]
struct EntryState {
    started: Instant,
    requests: u32,
    finished: u32,
    failed: u32,
    in_flight: u32,
    /// Known lengths (and the actual sizes of ended requests).
    total: u64,
    /// Requests whose length is not known yet (or never will be while they run).
    unknown: u32,
    /// Bytes resumed from an earlier session (in the progress, not in the speed).
    prefix: u64,
    idle_since: Option<Instant>,
    last_failure: Option<Failure>,
    /// Cancelled from the panel: requests starting before this are cancelled too.
    cancel_until: Option<Instant>,
    cancelled: bool,
    speed: Speed,
    items: Option<Items>,
}

#[derive(Debug)]
struct Entry {
    id: u64,
    label: Label,
    /// Bytes received over the network (a chunk is one add).
    bytes: AtomicU64,
    /// Bumped by a cancel: a request that saw another value at its start is cancelled.
    cancel_epoch: AtomicU64,
    st: Mutex<EntryState>,
}

impl Entry {
    fn linger(st: &EntryState) -> Duration {
        if st.cancelled || (st.failed > 0 && st.failed == st.requests) {
            FAILED_LINGER
        } else {
            LINGER
        }
    }

    /// Ended and lingered long enough: gone from the panel.
    fn expired(st: &EntryState, now: Instant) -> bool {
        st.in_flight == 0
            && st
                .idle_since
                .is_some_and(|t| now.saturating_duration_since(t) >= Entry::linger(st))
    }
}

#[derive(Default)]
struct Inner {
    entries: Mutex<Vec<Arc<Entry>>>,
    generation: AtomicU64,
    next_id: AtomicU64,
    waker: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    last_wake: Mutex<Option<Instant>>,
}

impl Inner {
    /// Bumps the generation; calls the waker now (`force`) or at most every [`WAKE_EVERY`].
    fn wake(&self, force: bool) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        let now = Instant::now();
        {
            let mut l = self.last_wake.lock();
            if !force && l.is_some_and(|t| now.saturating_duration_since(t) < WAKE_EVERY) {
                return;
            }
            *l = Some(now);
        }
        let w = self.waker.lock().clone();
        if let Some(w) = w {
            w();
        }
    }
}

/// The registry; [`global`] is the app's. Cheap to clone.
#[derive(Clone, Default)]
pub struct Registry(Arc<Inner>);

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("entries", &self.0.entries.lock().len())
            .finish_non_exhaustive()
    }
}

/// The app's registry: every request reports here.
pub fn global() -> &'static Registry {
    static G: OnceLock<Registry> = OnceLock::new();
    G.get_or_init(Registry::default)
}

impl Registry {
    pub fn new() -> Registry {
        Registry::default()
    }

    /// Called when an activity starts or ends (at once) and as progress moves (at most
    /// every [`WAKE_EVERY`]).
    pub fn set_waker(&self, f: Arc<dyn Fn() + Send + Sync>) {
        *self.0.waker.lock() = Some(f);
    }

    /// Bumped by every start, end and cancel: progress alone does not move it.
    pub fn generation(&self) -> u64 {
        self.0.generation.load(Ordering::Acquire)
    }

    /// Starts a request of the activity `label` (a new one when none is shown).
    pub fn begin(&self, label: Label) -> Request {
        self.begin_at(label, Instant::now())
    }

    #[doc(hidden)]
    pub fn begin_at(&self, label: Label, now: Instant) -> Request {
        let mut entries = self.0.entries.lock();
        let found = entries.iter().position(|e| e.label == label);
        let reuse = found.filter(|&i| !Entry::expired(&entries[i].st.lock(), now));
        if let (Some(i), None) = (found, reuse) {
            entries.remove(i);
        }
        let (entry, fresh) = match reuse {
            Some(i) => (entries[i].clone(), false),
            None => {
                let e = Arc::new(Entry {
                    id: self.0.next_id.fetch_add(1, Ordering::Relaxed) + 1,
                    label,
                    bytes: AtomicU64::new(0),
                    cancel_epoch: AtomicU64::new(0),
                    st: Mutex::new(EntryState {
                        started: now,
                        requests: 0,
                        finished: 0,
                        failed: 0,
                        in_flight: 0,
                        total: 0,
                        unknown: 0,
                        prefix: 0,
                        idle_since: None,
                        last_failure: None,
                        cancel_until: None,
                        cancelled: false,
                        speed: Speed::default(),
                        items: None,
                    }),
                });
                entries.push(e.clone());
                (e, true)
            }
        };
        drop(entries);
        let (epoch, cancelled, was_idle) = {
            let mut st = entry.st.lock();
            let was_idle = st.in_flight == 0;
            st.requests += 1;
            st.in_flight += 1;
            st.unknown += 1;
            st.idle_since = None;
            // Cancelled a moment ago: this one goes too.
            let cancelled = st.cancel_until.is_some_and(|t| now < t);
            if was_idle && !cancelled {
                st.cancelled = false;
            }
            (
                entry.cancel_epoch.load(Ordering::Acquire),
                cancelled,
                was_idle,
            )
        };
        // Shown, or running again: at once.
        self.0.wake(fresh || was_idle);
        Request {
            entry,
            reg: Arc::downgrade(&self.0),
            epoch,
            cancelled,
            attempt: 0,
            len: None,
            has_head: false,
            prefix: 0,
            ended: false,
        }
    }

    /// Whether anything is shown (a cheap look before a [`snapshot`](Self::snapshot)).
    pub fn any(&self) -> bool {
        !self.0.entries.lock().is_empty()
    }

    /// The activities shown now, oldest first; samples the speeds and forgets the expired.
    pub fn snapshot(&self) -> Vec<Activity> {
        self.snapshot_at(Instant::now())
    }

    #[doc(hidden)]
    pub fn snapshot_at(&self, now: Instant) -> Vec<Activity> {
        let mut entries = self.0.entries.lock();
        entries.retain(|e| !Entry::expired(&e.st.lock(), now));
        entries
            .iter()
            .map(|e| {
                let net = e.bytes.load(Ordering::Acquire);
                let mut st = e.st.lock();
                let running = st.in_flight > 0;
                if running {
                    st.speed.sample(net, now);
                }
                let bytes = net + st.prefix;
                let total = (st.unknown == 0 && st.total > 0).then_some(st.total.max(bytes));
                let state = if running {
                    State::Running
                } else if st.cancelled {
                    State::Cancelled
                } else if st.failed > 0 && st.failed == st.requests {
                    State::Failed(st.last_failure.unwrap_or(Failure::Other))
                } else {
                    State::Done
                };
                let speed = if running { st.speed.get() } else { None };
                let single = st.requests <= 1;
                let items = st.items.filter(|i| i.total() > 0);
                let fraction = match (state, items) {
                    (State::Done, _) => Some(1.0),
                    (_, Some(i)) => Some(i.ended() as f32 / i.total() as f32),
                    _ if single => total.map(|t| (bytes as f64 / t as f64) as f32),
                    _ => Some(st.finished as f32 / st.requests.max(1) as f32),
                };
                let counted = items.filter(|i| running && i.ended() > 0);
                let eta = match (single && running, total, speed) {
                    // At the pace so far.
                    _ if counted.is_some() => counted.map(|i| {
                        let per = now.saturating_duration_since(st.started).as_secs_f64()
                            / f64::from(i.ended());
                        Duration::from_secs_f64((per * f64::from(i.remaining)).min(1e7))
                    }),
                    (true, Some(t), Some(s)) => Some(Duration::from_secs_f64(
                        (t.saturating_sub(bytes) as f64 / s).min(1e7),
                    )),
                    _ => None,
                };
                let end = if running {
                    now
                } else {
                    st.idle_since.unwrap_or(now)
                };
                Activity {
                    id: e.id,
                    label: e.label.clone(),
                    state,
                    bytes,
                    total,
                    requests: st.requests,
                    finished: st.finished,
                    failed: st.failed,
                    speed,
                    eta,
                    fraction: fraction.map(|f| f.clamp(0.0, 1.0)),
                    elapsed: end.saturating_duration_since(st.started),
                    cancellable: running,
                    items,
                }
            })
            .collect()
    }

    /// When the next shown activity goes away (the app looks again then).
    pub fn next_expiry(&self, now: Instant) -> Option<Instant> {
        self.0
            .entries
            .lock()
            .iter()
            .filter_map(|e| {
                let st = e.st.lock();
                (st.in_flight == 0)
                    .then(|| st.idle_since.map(|t| t + Entry::linger(&st)))
                    .flatten()
            })
            .filter(|t| *t > now)
            .min()
    }

    /// Stops the running requests of activity `id` (and any it starts in the next
    /// [`CANCEL_WINDOW`]). Whether it was running.
    pub fn cancel(&self, id: u64) -> bool {
        self.cancel_at(id, Instant::now())
    }

    #[doc(hidden)]
    pub fn cancel_at(&self, id: u64, now: Instant) -> bool {
        let e = self.0.entries.lock().iter().find(|e| e.id == id).cloned();
        let Some(e) = e else { return false };
        {
            let mut st = e.st.lock();
            if st.in_flight == 0 {
                return false;
            }
            st.cancel_until = Some(now + CANCEL_WINDOW);
            st.cancelled = true;
        }
        e.cancel_epoch.fetch_add(1, Ordering::AcqRel);
        self.0.wake(true);
        true
    }
}

/// One request of an activity, from its start to its end; dropping it ends it (as
/// finished, unless [`finish`](Self::finish) said otherwise).
#[derive(Debug)]
#[must_use = "the request ends when this drops"]
pub struct Request {
    entry: Arc<Entry>,
    reg: Weak<Inner>,
    epoch: u64,
    cancelled: bool,
    /// Bytes of this attempt (a retry takes them back).
    attempt: u64,
    len: Option<u64>,
    has_head: bool,
    prefix: u64,
    ended: bool,
}

impl Request {
    /// The activity's id ([`Registry::cancel`]).
    pub fn id(&self) -> u64 {
        self.entry.id
    }

    /// The response's head arrived: its length, if the server gave one (the whole file's,
    /// for a resumed download), and how much of that is already here (`prefix`). Called
    /// again for a retried attempt, it replaces the earlier one's.
    pub fn head(&mut self, len: Option<u64>, prefix: u64) {
        let prefix = len.map_or(prefix, |l| prefix.min(l));
        if self.attempt > 0 {
            self.entry.bytes.fetch_sub(self.attempt, Ordering::AcqRel);
            self.attempt = 0;
        }
        let mut st = self.entry.st.lock();
        if self.has_head {
            if let Some(l) = self.len {
                st.total = st.total.saturating_sub(l);
            } else {
                st.unknown = st.unknown.saturating_sub(1);
            }
            st.prefix = st.prefix.saturating_sub(self.prefix);
        } else {
            st.unknown = st.unknown.saturating_sub(1);
        }
        match len {
            Some(l) => st.total += l,
            None => st.unknown += 1,
        }
        st.prefix += prefix;
        self.has_head = true;
        self.len = len;
        self.prefix = prefix;
    }

    /// `n` more bytes arrived.
    #[inline]
    pub fn add(&mut self, n: u64) {
        self.attempt += n;
        self.entry.bytes.fetch_add(n, Ordering::AcqRel);
    }

    /// Stopped from the panel: the fetch should end with a cancel.
    #[inline]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled || self.entry.cancel_epoch.load(Ordering::Acquire) != self.epoch
    }

    /// The server's counts so far (shown instead of a byte fraction).
    pub fn items(&mut self, items: Items) {
        self.entry.st.lock().items = Some(items);
        if let Some(reg) = self.reg.upgrade() {
            reg.wake(false);
        }
    }

    /// A handle that tells whether this request was stopped from the panel, for another
    /// thread.
    pub fn watch(&self) -> CancelWatch {
        CancelWatch {
            entry: self.entry.clone(),
            epoch: self.epoch,
            cancelled: self.cancelled,
        }
    }

    /// Ends the request: fine, or why not.
    pub fn finish(mut self, r: Result<(), Failure>) {
        self.end(r, Instant::now());
    }

    #[doc(hidden)]
    pub fn finish_at(mut self, r: Result<(), Failure>, now: Instant) {
        self.end(r, now);
    }

    fn end(&mut self, r: Result<(), Failure>, now: Instant) {
        if std::mem::replace(&mut self.ended, true) {
            return;
        }
        let r = match r {
            Err(_) if self.is_cancelled() => Err(Failure::Cancelled),
            r => r,
        };
        let idle = {
            let mut st = self.entry.st.lock();
            // What this request counts for from now on: what it brought.
            let actual = self.attempt + self.prefix;
            match (self.has_head, self.len) {
                (true, Some(l)) => st.total = st.total.saturating_sub(l) + actual,
                _ => {
                    st.unknown = st.unknown.saturating_sub(1);
                    st.total += actual;
                }
            }
            st.finished += 1;
            st.in_flight = st.in_flight.saturating_sub(1);
            match r {
                Ok(()) => {}
                Err(Failure::Cancelled) => {
                    st.failed += 1;
                    st.cancelled = true;
                    st.last_failure = Some(Failure::Cancelled);
                }
                Err(f) => {
                    st.failed += 1;
                    st.last_failure = Some(f);
                }
            }
            let idle = st.in_flight == 0;
            if idle {
                st.idle_since = Some(now);
            }
            idle
        };
        if let Some(reg) = self.reg.upgrade() {
            reg.wake(idle);
        }
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        self.end(Ok(()), Instant::now());
    }
}

/// See [`Request::watch`].
#[derive(Clone, Debug)]
pub struct CancelWatch {
    entry: Arc<Entry>,
    epoch: u64,
    cancelled: bool,
}

impl CancelWatch {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled || self.entry.cancel_epoch.load(Ordering::Acquire) != self.epoch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(what: What) -> Label {
        Label::new(
            what,
            "https://user:pw@Tiles.Example.org:8443/a/b?sig=SECRET",
        )
    }

    #[test]
    fn host_never_carries_credentials_paths_or_queries() {
        assert_eq!(
            host_of("https://user:pw@Tiles.Example.org:8443/a/b?sig=SECRET"),
            "tiles.example.org:8443"
        );
        assert_eq!(host_of("http://h?x=1"), "h");
        assert_eq!(host_of("not a url"), "");
    }

    #[test]
    fn speed_and_eta_follow_the_bytes() {
        let reg = Registry::new();
        let t0 = Instant::now();
        let mut r = reg.begin_at(label(What::Model("clip".into())), t0);
        r.head(Some(10_000_000), 0);
        let a = &reg.snapshot_at(t0)[0];
        assert_eq!(a.state, State::Running);
        assert_eq!(a.total, Some(10_000_000));
        assert_eq!(a.speed, None, "no speed before a second sample");
        assert_eq!(a.eta, None);
        // 1 MB/s for two seconds.
        for s in 1..=2u64 {
            r.add(1_000_000);
            reg.snapshot_at(t0 + Duration::from_secs(s));
        }
        let a = &reg.snapshot_at(t0 + Duration::from_secs(2))[0];
        let v = a.speed.unwrap();
        assert!((v - 1_000_000.0).abs() < 1.0, "{v}");
        assert_eq!(a.bytes, 2_000_000);
        assert!((a.fraction.unwrap() - 0.2).abs() < 1e-6);
        assert_eq!(a.eta.unwrap().as_secs(), 8);
        // The speed then doubles: the average moves towards it, not at once.
        r.add(2_000_000);
        let a = &reg.snapshot_at(t0 + Duration::from_secs(3))[0];
        let v = a.speed.unwrap();
        let alpha = 1.0 - (-1.0f64 / SPEED_TAU.as_secs_f64()).exp();
        assert!((v - (1e6 + alpha * 1e6)).abs() < 1.0, "{v}");
        assert!(v > 1_000_000.0 && v < 2_000_000.0);
        r.finish_at(Ok(()), t0 + Duration::from_secs(3));
        let a = &reg.snapshot_at(t0 + Duration::from_secs(3))[0];
        assert_eq!(a.state, State::Done);
        assert_eq!(a.speed, None);
        assert_eq!(a.eta, None);
        assert!(!a.cancellable);
    }

    #[test]
    fn a_resumed_part_counts_in_progress_not_in_speed() {
        let reg = Registry::new();
        let t0 = Instant::now();
        let mut r = reg.begin_at(label(What::Model("m".into())), t0);
        r.head(Some(1000), 600);
        reg.snapshot_at(t0);
        r.add(100);
        let a = &reg.snapshot_at(t0 + Duration::from_secs(1))[0];
        assert_eq!(a.bytes, 700);
        assert!((a.speed.unwrap() - 100.0).abs() < 1e-9);
        assert_eq!(a.eta.unwrap().as_secs(), 3);
        drop(r);
    }

    #[test]
    fn a_retried_attempt_takes_its_bytes_back() {
        let reg = Registry::new();
        let t0 = Instant::now();
        let mut r = reg.begin_at(label(What::Data), t0);
        r.head(Some(500), 0);
        r.add(300);
        r.head(Some(500), 0);
        r.add(500);
        let a = &reg.snapshot_at(t0)[0];
        assert_eq!((a.bytes, a.total), (500, Some(500)));
        r.finish_at(Ok(()), t0);
    }

    #[test]
    fn many_requests_are_one_line_with_counts() {
        let reg = Registry::new();
        let t0 = Instant::now();
        let mut rs: Vec<Request> = (0..4)
            .map(|_| reg.begin_at(label(What::MapTiles), t0))
            .collect();
        // Another host is another line; another kind from the same host too.
        let other = reg.begin_at(Label::new(What::MapTiles, "https://b.example/x"), t0);
        let model = reg.begin_at(label(What::ImageTiles), t0);
        let snap = reg.snapshot_at(t0);
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].requests, 4);
        assert!(snap[0].grouped());
        assert_eq!(snap[0].total, None, "lengths not known yet");
        for r in &mut rs {
            r.head(Some(1000), 0);
            r.add(1000);
        }
        let last = rs.pop().unwrap();
        for r in rs.drain(..) {
            r.finish_at(Ok(()), t0);
        }
        let a = &reg.snapshot_at(t0)[0];
        assert_eq!((a.finished, a.requests, a.bytes), (3, 4, 4000));
        assert_eq!(a.total, Some(4000));
        assert!((a.fraction.unwrap() - 0.75).abs() < 1e-6);
        assert_eq!(a.eta, None, "no time left for a group");
        last.finish_at(Err(Failure::Status(404)), t0);
        let a = &reg.snapshot_at(t0)[0];
        assert_eq!(a.state, State::Done, "some failed, not all");
        assert_eq!(a.failed, 1);
        drop((other, model));
    }

    #[test]
    fn chunked_bodies_have_no_total_until_they_end() {
        let reg = Registry::new();
        let t0 = Instant::now();
        let mut r = reg.begin_at(label(What::Data), t0);
        r.head(None, 0);
        r.add(42);
        let a = &reg.snapshot_at(t0)[0];
        assert_eq!((a.total, a.fraction), (None, None));
        r.finish_at(Ok(()), t0);
        let a = &reg.snapshot_at(t0)[0];
        assert_eq!((a.total, a.fraction), (Some(42), Some(1.0)));
    }

    #[test]
    fn finished_lingers_then_goes_and_a_new_burst_starts_from_zero() {
        let reg = Registry::new();
        let t0 = Instant::now();
        let r = reg.begin_at(label(What::MapTiles), t0);
        r.finish_at(Ok(()), t0);
        assert_eq!(reg.next_expiry(t0), Some(t0 + LINGER));
        assert_eq!(reg.snapshot_at(t0 + LINGER / 2).len(), 1);
        // A request while it lingers joins it.
        let r = reg.begin_at(label(What::MapTiles), t0 + LINGER / 2);
        let a = &reg.snapshot_at(t0 + LINGER / 2)[0];
        assert_eq!((a.requests, a.state), (2, State::Running));
        let id = a.id;
        r.finish_at(Ok(()), t0 + LINGER / 2);
        assert!(
            reg.snapshot_at(t0 + LINGER).len() == 1,
            "lingers from its last end"
        );
        assert!(reg.snapshot_at(t0 + LINGER * 2).is_empty());
        assert!(!reg.any());
        assert_eq!(reg.next_expiry(t0), None);
        let r = reg.begin_at(label(What::MapTiles), t0 + LINGER * 3);
        let a = &reg.snapshot_at(t0 + LINGER * 3)[0];
        assert_eq!(a.requests, 1);
        assert_ne!(a.id, id);
        drop(r);
    }

    #[test]
    fn failures_linger_longer_with_their_reason() {
        let reg = Registry::new();
        let t0 = Instant::now();
        let r = reg.begin_at(label(What::UpdateCheck), t0);
        r.finish_at(Err(Failure::Offline), t0);
        let a = &reg.snapshot_at(t0 + LINGER)[0];
        assert_eq!(a.state, State::Failed(Failure::Offline));
        assert!(reg.snapshot_at(t0 + FAILED_LINGER).is_empty());
    }

    #[test]
    fn cancel_stops_running_requests_and_ones_started_right_after() {
        let reg = Registry::new();
        let t0 = Instant::now();
        let r = reg.begin_at(label(What::Model("m".into())), t0);
        let id = r.id();
        assert!(!r.is_cancelled());
        assert!(reg.cancel_at(id, t0));
        assert!(r.is_cancelled());
        // A fallback to another mirror a moment later is cancelled too.
        let again = reg.begin_at(label(What::Model("m".into())), t0 + CANCEL_WINDOW / 2);
        assert!(again.is_cancelled());
        // The fetch ends with an error; it reads as cancelled.
        r.finish_at(Err(Failure::Other), t0);
        again.finish_at(Err(Failure::Other), t0 + CANCEL_WINDOW / 2);
        let a = &reg.snapshot_at(t0 + CANCEL_WINDOW)[0];
        assert_eq!(a.state, State::Cancelled);
        assert!(
            !reg.cancel_at(id, t0 + CANCEL_WINDOW),
            "nothing left to stop"
        );
        // Later requests run normally.
        let later = reg.begin_at(label(What::Model("m".into())), t0 + CANCEL_WINDOW * 2);
        assert!(!later.is_cancelled());
        assert_eq!(
            reg.snapshot_at(t0 + CANCEL_WINDOW * 2)[0].state,
            State::Running
        );
        drop(later);
        assert!(!reg.cancel(999), "no such activity");
    }

    #[test]
    fn progress_does_not_move_the_generation_but_starts_and_ends_do() {
        let reg = Registry::new();
        let woke = Arc::new(AtomicU64::new(0));
        let w = woke.clone();
        reg.set_waker(Arc::new(move || {
            w.fetch_add(1, Ordering::Relaxed);
        }));
        let g0 = reg.generation();
        let mut r = reg.begin(label(What::Data));
        assert!(reg.generation() > g0);
        assert_eq!(
            woke.load(Ordering::Relaxed),
            1,
            "a new activity wakes at once"
        );
        let g1 = reg.generation();
        r.head(Some(10), 0);
        r.add(5);
        assert_eq!(reg.generation(), g1);
        drop(r);
        assert!(reg.generation() > g1);
        assert_eq!(woke.load(Ordering::Relaxed), 2, "the end wakes at once");
    }

    #[test]
    fn dropping_a_request_ends_it() {
        let reg = Registry::new();
        drop(reg.begin(label(What::Data)));
        let a = &reg.snapshot()[0];
        assert_eq!((a.state, a.finished), (State::Done, 1));
    }

    #[test]
    fn counted_progress_and_a_watch() {
        let reg = Registry::new();
        let t0 = Instant::now();
        let mut r = reg.begin_at(label(What::DicomSeries("CT".into())), t0);
        let w = r.watch();
        r.items(Items {
            completed: 3,
            failed: 1,
            warning: 0,
            remaining: 4,
        });
        let a = &reg.snapshot_at(t0 + Duration::from_secs(8))[0];
        assert_eq!(a.fraction, Some(0.5));
        // 8 s for 4: 8 s more for the other 4.
        assert_eq!(a.eta.map(|d| d.as_secs()), Some(8));
        assert_eq!(a.items.map(|i| i.total()), Some(8));
        assert!(!w.is_cancelled());
        reg.cancel(a.id);
        assert!(w.is_cancelled() && r.is_cancelled());
        r.finish(Err(Failure::Cancelled));
    }
}
