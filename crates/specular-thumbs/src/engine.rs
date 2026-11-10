//! The thumbnail scheduler.
//!
//! The caller (the gallery, once per frame when the view moved) says which thumbnails it
//! wants, most important first: the visible tiles in reading order, then the rows in the
//! scroll direction, then the rows behind. [`Engine::want`] replaces the whole wish list;
//! work for items that dropped out is cancelled (the decoders poll the token per MCU row).
//! A frame posts its list with [`Engine::try_want`] instead, which never waits: the engine's
//! feeder thread applies the newest posted list, taking the locks the workers share.
//!
//! Work runs on the shared [`JobPool`] as small "drainer" jobs. A drainer does not carry an
//! item: it takes the best pending item *when it starts* (late binding), so a fast scroll
//! never leaves a queue of stale requests ahead of what is on screen. Drainers for visible
//! items run at [`Priority::Visible`]; prefetch drainers at [`Priority::Prefetch`], which the
//! pool holds back while the viewer decodes the current image.
//!
//! Each item: key (size, mtime, partial hash) → disk cache → on a miss, open through the
//! registry (sandboxed formats go through the decode host there), generate, store. Results
//! wait in an outbox ([`Engine::take_ready`]); the waker tells the UI to come and get them.
//! Finished pixels also stay in a bounded memory LRU so a tile scrolled back into view after
//! its GPU slot was reused is uploaded again without touching the disk.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};
use specular_core::{CancelToken, JobPool, Priority, Source};
use specular_decode::{DecodeError, Registry};

use crate::cache::{Digest, DiskCache, Meta};
use crate::generate::{self, GenOptions};
use crate::key::ThumbKey;
use crate::pixels::Pixels;

/// Resolves a path to bytes and a cache key. The default reads files; the app's opener also
/// resolves archive members (`book.zip/page.jpg`).
pub trait Opener: Send + Sync {
    fn key(&self, path: &Path) -> std::io::Result<ThumbKey>;
    fn open(&self, path: &Path) -> std::io::Result<Opened>;

    /// Where `path`'s bytes lie in a stream that is only read front to back (a member of a
    /// solid archive), or `None` when it opens on its own (the default). Items of one stream
    /// are made by one worker, in stream order, through [`Opener::open_stream`].
    ///
    /// With `open == false` it must answer at once from what is already known (the engine
    /// asks under its lock, for every waiting item): `None` when the stream has not been
    /// opened yet. With `open == true` it may open the stream's container.
    fn stream(&self, path: &Path, open: bool) -> Option<StreamPos> {
        let _ = (path, open);
        None
    }

    /// Opens `paths` (all of one stream, in stream order), handing each to `each` with its
    /// index as soon as it is read; stops when `each` returns false. An item never handed
    /// over counts as failed. The default opens them one by one. `each` takes the engine's
    /// lock: no lock that [`Opener::stream`] needs may be held while calling it.
    fn open_stream(
        &self,
        paths: &[&Path],
        each: &mut dyn FnMut(usize, std::io::Result<Opened>) -> bool,
    ) {
        for (i, p) in paths.iter().enumerate() {
            if !each(i, self.open(p)) {
                break;
            }
        }
    }
}

/// A place in a stream (see [`Opener::stream`]): which stream, and the order in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StreamPos {
    pub stream: u64,
    pub at: u64,
}

/// Most items of one stream taken by its worker at once (it takes more when done, so a
/// stream is still read once; the cap lets a new wish list reorder the rest).
const STREAM_BATCH: usize = 48;

pub struct Opened {
    pub source: Arc<Source>,
    /// Pixels that must never reach the disk cache (a page of an encrypted archive): the
    /// thumbnail is made and kept in memory only.
    pub private: bool,
}

impl Opened {
    /// `source`, which may be cached on disk.
    pub fn new(source: Arc<Source>) -> Opened {
        Opened {
            source,
            private: false,
        }
    }
}

/// Plain files.
pub struct FileOpener;

impl Opener for FileOpener {
    fn key(&self, path: &Path) -> std::io::Result<ThumbKey> {
        crate::slowio::open(3 * crate::key::SAMPLE as u64);
        ThumbKey::of_file(path).or_else(|e| {
            // A folder that opens as one image: keyed by its entries' stamps.
            if path.is_dir() && specular_core::is_image_folder(path) {
                ThumbKey::of_folder(path)
            } else {
                Err(e)
            }
        })
    }
    fn open(&self, path: &Path) -> std::io::Result<Opened> {
        crate::slowio::open(std::fs::metadata(path).map_or(0, |m| m.len()));
        Ok(Opened::new(Arc::new(Source::open(path)?)))
    }
}

/// Plain files ([`FileOpener`]) whose keys are remembered, for a caller that needs a file's
/// key itself and also has the engine open the file (the similar-shots scan: its cache
/// look-up, the engine's, and the file's content id): the samples are read once. A
/// remembered key is used only while the file's size and time stamp still match it (one
/// `stat`); else it is read again. Clones share the memo.
#[derive(Clone, Default)]
pub struct KeyMemo(Arc<Mutex<HashMap<PathBuf, ThumbKey>>>);

impl KeyMemo {
    /// The key of `path`, remembered.
    pub fn key(&self, path: &Path) -> std::io::Result<ThumbKey> {
        if let Some(k) = self.still(path, false) {
            return Ok(k);
        }
        let k = FileOpener.key(path)?;
        self.0.lock().insert(path.to_path_buf(), k);
        Ok(k)
    }

    /// The key of `path`, forgotten: the caller's last use of it.
    pub fn take(&self, path: &Path) -> std::io::Result<ThumbKey> {
        match self.still(path, true) {
            Some(k) => Ok(k),
            None => FileOpener.key(path),
        }
    }

    /// Forgets `path`'s key.
    pub fn forget(&self, path: &Path) {
        self.0.lock().remove(path);
    }

    /// Forgets every key.
    pub fn clear(&self) {
        self.0.lock().clear();
    }

    pub fn len(&self) -> usize {
        self.0.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The remembered key while the file still has its size and time stamp.
    fn still(&self, path: &Path, remove: bool) -> Option<ThumbKey> {
        let k = if remove {
            self.0.lock().remove(path)?
        } else {
            *self.0.lock().get(path)?
        };
        crate::slowio::stat();
        let now = crate::key::Stamp::of_path(path).ok()?;
        (now == k.stamp).then_some(k)
    }
}

impl Opener for KeyMemo {
    fn key(&self, path: &Path) -> std::io::Result<ThumbKey> {
        KeyMemo::key(self, path)
    }
    fn open(&self, path: &Path) -> std::io::Result<Opened> {
        FileOpener.open(path)
    }
}

#[derive(Clone, Debug)]
pub struct EngineConfig {
    /// Most thumbnails generated at once.
    pub workers: usize,
    /// Bytes of finished pixels kept in memory.
    pub memory_budget: usize,
    /// Mixed into the cache key (quality options that change the pixels).
    pub flags: u32,
    pub generate: GenOptions,
    /// How many of the wanted items count as visible (higher priority).
    pub write_cache: bool,
    /// Computed from each thumbnail of its class when it is generated, stored in its cache
    /// entry and handed back with it ([`Ready::digest`]).
    pub digester: Option<Arc<dyn Digester>>,
}

impl Default for EngineConfig {
    fn default() -> EngineConfig {
        EngineConfig {
            workers: std::thread::available_parallelism()
                .map_or(4, |n| n.get())
                .clamp(2, 8),
            memory_budget: 160 << 20,
            flags: 0,
            generate: GenOptions::default(),
            write_cache: true,
            digester: None,
        }
    }
}

/// Computes a small record from a freshly generated thumbnail (the duplicate finder's
/// signature), on the worker that made it, from the very pixels it made (not the lossy cache
/// copy). Kept in the cache entry, it spares a later reader both the decode of the file and
/// that of the thumbnail.
pub trait Digester: Send + Sync + std::fmt::Debug {
    /// What the digest is, and its version: an entry holding another id holds none. Change
    /// it whenever the bytes for the same pixels would change.
    fn id(&self) -> u32;
    /// The size class it is computed for (other classes carry none).
    fn class(&self) -> u32;
    fn digest(&self, px: &Pixels) -> Vec<u8>;
}

/// One wanted thumbnail. `class` 0 asks for the [`Meta`] only (sorting by date taken).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Request {
    pub path: PathBuf,
    pub class: u32,
}

/// A finished (or early, `sharp == false`) thumbnail.
#[derive(Clone, Debug)]
pub struct Ready {
    pub path: PathBuf,
    pub class: u32,
    /// `None` for meta-only requests.
    pub pixels: Option<Arc<Pixels>>,
    pub average: [u8; 4],
    pub meta: Arc<Meta>,
    /// The final thumbnail for this class (else an embedded preview shown meanwhile).
    pub sharp: bool,
    pub from_cache: bool,
    /// The [`EngineConfig::digester`]'s record of the pixels this thumbnail was made from:
    /// computed now, or read from the cache entry (`None` for an entry made without one).
    pub digest: Option<Arc<[u8]>>,
}

/// Where a generated thumbnail spent its time, for the hardware advisor (DESIGN.md §12.7):
/// the stages of one item on its worker, without the time it waited in the queue. Kept for
/// sharp thumbnails made from the file (not read from the cache) that were still wanted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timing {
    pub path: PathBuf,
    /// [`Meta::format`].
    pub format: String,
    /// The file's size in bytes.
    pub bytes: u64,
    /// Reading the cache key and opening the file (ms).
    pub open_ms: f64,
    /// The container and codec headers (the registry's open).
    pub parse_ms: f64,
    /// Making the pixels: an embedded preview or a reduced decode.
    pub decode_ms: f64,
    /// Of `parse_ms + decode_ms`, the time estimated to have waited for the pages of a
    /// memory-mapped file: wall minus CPU time on this thread, when it had major faults.
    pub io_ms: f64,
    /// Major page faults (pages read from storage) meanwhile.
    pub faults: u64,
    /// The path the selector took for the decode, when it was asked.
    pub note: Option<specular_core::paths::Note>,
    /// Kept in the disk cache for next time (false: no cache, or writing to it failed).
    pub cached: bool,
}

/// Timings kept for [`Engine::try_take_timings`] (the oldest go first).
const TIMINGS_KEPT: usize = 256;
/// Longest wait one major page fault is charged with (a seek or a network round trip with
/// readahead), so a fault or two never turns waiting for helper threads into reading.
const MAX_FAULT_MS: f64 = 25.0;

/// Images from this many pixels up may decode with the decoder's own threads in the gallery
/// ([`decode_in_parallel`]): a gigapixel PNG takes seconds on one core.
pub const PARALLEL_MIN_PIXELS: u64 = 100_000_000;

/// Whether the gallery decodes an image of `pixels` with the decoder's threads
/// ([`GenOptions::parallel`]): only a very large one ([`PARALLEL_MIN_PIXELS`]), a few at a
/// time (`running`: such decodes under way; a quarter of the `workers`, at least one), and
/// only while at most half the workers are busy with other items (`others`). Otherwise the
/// other workers keep the cores busy already, and the decode stays on its worker as every
/// other thumbnail does.
pub fn decode_in_parallel(pixels: u64, others: usize, running: usize, workers: usize) -> bool {
    let workers = workers.max(1);
    pixels >= PARALLEL_MIN_PIXELS && running < (workers / 4).max(1) && others < workers.div_ceil(2)
}

#[derive(Clone, Debug)]
pub enum Outcome {
    Ready(Ready),
    /// `memory` when it needs more free RAM than there is now
    /// ([`DecodeError::NeedsMemory`]): the gallery says so on the tile.
    Failed {
        path: PathBuf,
        /// Why, as the decoder said (the app words it in the user's language).
        error: Arc<DecodeError>,
        memory: Option<specular_core::NeedsMemory>,
    },
}

/// Where applying a wish list spent its time (logged by the feeder when slow).
#[derive(Clone, Copy, Debug, Default)]
struct WantTimes {
    /// Under the engine's lock: the new list checked against running and remembered items.
    pub locked: Duration,
    /// Dropping cancelled jobs from the pool's queue.
    pub purge: Duration,
    /// Queueing new drainers.
    pub submit: Duration,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub pending: usize,
    pub in_flight: usize,
    pub from_cache: u64,
    pub generated: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub memory_bytes: usize,
    pub memory_items: usize,
}

struct MemItem {
    pixels: Arc<Pixels>,
    average: [u8; 4],
    used: u64,
    /// [`Pixels::bytes`] when remembered (the premultiplied copy included).
    bytes: usize,
}

/// The remembered classes of one file, by index in [`crate::CLASSES`]: keyed by path alone, so
/// lookups borrow the caller's path instead of building a `(PathBuf, u32)` key.
#[derive(Default)]
struct MemSlots([Option<MemItem>; crate::CLASSES.len()]);

fn class_index(class: u32) -> Option<usize> {
    crate::CLASSES.iter().position(|&c| c == class)
}

impl MemSlots {
    fn get(&self, class: u32) -> Option<&MemItem> {
        self.0[class_index(class)?].as_ref()
    }

    fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }
}

#[derive(Default)]
struct State {
    /// Wanted and not started, most important first.
    pending: VecDeque<Request>,
    /// How many of `pending` (from the front) are visible.
    visible: usize,
    in_flight: HashMap<Request, CancelToken>,
    outbox: Vec<Outcome>,
    /// Timings of generated thumbnails not taken yet.
    timings: VecDeque<Timing>,
    mem: HashMap<PathBuf, MemSlots>,
    /// Classes in `mem`.
    mem_items: usize,
    mem_bytes: usize,
    meta: HashMap<PathBuf, Arc<Meta>>,
    failed: HashSet<PathBuf>,
    drainers: usize,
    /// Decodes running with the decoder's threads ([`decode_in_parallel`]).
    parallel: usize,
    tick: u64,
    stats: Stats,
    /// Streams a worker is reading ([`Opener::stream`]), with the items other workers handed
    /// to it (already in `in_flight`, their keys read).
    streams: HashMap<u64, Vec<Item>>,
    /// Items workers took from `pending` and are looking up, opening their container to
    /// learn whether they lie in a stream. A stream's worker takes over those of its stream
    /// ([`Inner::stream`]): handed over only after its batch, they would be read out of
    /// order (the first pages of a solid archive after the next 48, restarting the stream).
    resolving: Vec<(Request, CancelToken)>,
    /// Items a stream's worker has read, for any worker to make (at most one per worker
    /// waits: beyond that the stream's worker makes the oldest itself).
    read: VecDeque<ReadItem>,
}

/// An item read from its stream, waiting to be made.
struct ReadItem {
    it: Item,
    key: ThumbKey,
    opened: Opened,
    open_ms: f64,
    generation: u64,
}

/// An item taken from `pending`: what it is, its token, and its key once read.
struct Item {
    req: Request,
    tok: CancelToken,
    key: Option<ThumbKey>,
}

/// A threaded decode under way ([`decode_in_parallel`]); its count drops with it.
struct ParallelSlot<'a>(&'a Inner);

impl Drop for ParallelSlot<'_> {
    fn drop(&mut self) {
        self.0.state.lock().parallel -= 1;
    }
}

/// A cache look-up: the thumbnail, or the key to make it under.
enum Lookup {
    Hit(Ready),
    Miss(ThumbKey),
}

struct Inner {
    cfg: Mutex<EngineConfig>,
    state: Mutex<State>,
    registry: Arc<Registry>,
    pool: Arc<JobPool>,
    opener: Arc<dyn Opener>,
    cache: Option<Arc<DiskCache>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    shutdown: AtomicBool,
    generation: AtomicU64,
    /// The newest wish list posted by [`Engine::try_want`], for the feeder thread.
    wish: Mutex<Wish>,
    wish_cv: Condvar,
}

#[derive(Default)]
struct Wish {
    /// Posted and not taken yet: the list and how many of it are visible.
    list: Option<(Vec<Request>, usize)>,
    /// The feeder is applying one.
    applying: bool,
}

/// See the module docs.
pub struct Engine {
    inner: Arc<Inner>,
    /// The feeder thread runs (else [`Engine::try_want`] applies lists itself).
    feeder: bool,
}

impl Engine {
    /// The decoder registry the engine opens files with.
    pub fn registry(&self) -> &Arc<Registry> {
        &self.inner.registry
    }

    /// The job pool the engine's work runs on (others may queue idle work there too).
    pub fn pool(&self) -> &Arc<JobPool> {
        &self.inner.pool
    }

    pub fn new(
        cfg: EngineConfig,
        registry: Arc<Registry>,
        pool: Arc<JobPool>,
        opener: Arc<dyn Opener>,
        cache: Option<Arc<DiskCache>>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Engine {
        let inner = Arc::new(Inner {
            cfg: Mutex::new(cfg),
            state: Mutex::new(State::default()),
            registry,
            pool,
            opener,
            cache,
            wake,
            shutdown: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            wish: Mutex::new(Wish::default()),
            wish_cv: Condvar::new(),
        });
        // Normal priority (unlike the pool's workers): a posted list is applied at once.
        let this = inner.clone();
        let feeder = match specular_thread::builder("thumb-feed")
            .spawn(move || Engine::feeder(&this))
        {
            Ok(_) => true,
            Err(e) => {
                specular_log::warn!(target: specular_log::CACHE, "thumbnail feeder thread: {e}");
                false
            }
        };
        Engine { inner, feeder }
    }

    pub fn cache(&self) -> Option<&Arc<DiskCache>> {
        self.inner.cache.as_ref()
    }

    pub fn set_config(&self, cfg: EngineConfig) {
        let flags_changed = self.inner.cfg.lock().flags != cfg.flags;
        *self.inner.cfg.lock() = cfg;
        if flags_changed {
            self.forget_all();
        }
    }

    pub fn config(&self) -> EngineConfig {
        self.inner.cfg.lock().clone()
    }

    /// Replaces the wish list now (waiting for the lock). `visible` = how many from the front
    /// are on screen. A list posted by [`Engine::try_want`] and not applied yet is dropped.
    pub fn want(&self, wanted: Vec<Request>, visible: usize) {
        {
            // A list the feeder is applying is older: let it finish first.
            let mut w = self.inner.wish.lock();
            w.list = None;
            while w.applying {
                self.inner.wish_cv.wait(&mut w);
            }
        }
        Inner::want_locked(&self.inner, wanted, visible);
    }

    /// [`Engine::want`] for a frame: hands the list to the engine's feeder thread and returns
    /// at once. The feeder applies the newest list it has (a newer one replaces one not yet
    /// applied): checking it against the running and remembered items, cancelling what
    /// dropped out and queueing drainers take the engine's lock and the pool's queue, which
    /// the (low-priority) workers hold too, so a frame never does that itself. False (ask
    /// again next frame) only if the feeder is taking the previous list this instant.
    pub fn try_want(&self, wanted: Vec<Request>, visible: usize) -> bool {
        if !self.feeder {
            self.want(wanted, visible);
            return true;
        }
        let Some(mut w) = self.inner.wish.try_lock() else {
            return false;
        };
        w.list = Some((wanted, visible));
        drop(w);
        // All: [`Engine::want`] may be waiting on it too.
        self.inner.wish_cv.notify_all();
        true
    }

    /// The feeder thread: applies posted wish lists until the engine is dropped.
    fn feeder(this: &Arc<Inner>) {
        loop {
            let (wanted, visible) = {
                let mut w = this.wish.lock();
                loop {
                    // Set under this lock by the engine's drop, so the wake-up is not missed.
                    if this.shutdown.load(Ordering::Acquire) {
                        return;
                    }
                    if let Some(l) = w.list.take() {
                        w.applying = true;
                        break l;
                    }
                    this.wish_cv.wait(&mut w);
                }
            };
            let n = wanted.len();
            // A panic drops this list only: `applying` must be cleared, or `want` waits
            // forever, and the next list is applied as usual.
            let t = specular_core::isolate::run("thumbnail wish list", || {
                Inner::want_locked(this, wanted, visible)
            });
            this.wish.lock().applying = false;
            this.wish_cv.notify_all();
            let Ok(t) = t else { continue };
            if t.locked + t.purge + t.submit > Duration::from_millis(20) {
                specular_log::debug!(target: specular_log::CACHE,
                    "thumb wish of {n}: {:.1} ms locked, {:.1} ms purge, {:.1} ms submit",
                    t.locked.as_secs_f64() * 1e3, t.purge.as_secs_f64() * 1e3,
                    t.submit.as_secs_f64() * 1e3);
            }
        }
    }

    /// Results since the last call.
    pub fn take_ready(&self) -> Vec<Outcome> {
        std::mem::take(&mut self.inner.state.lock().outbox)
    }

    /// [`Engine::take_ready`] for a frame: `None` if a worker holds the lock (ask again next
    /// frame instead of waiting).
    pub fn try_take_ready(&self) -> Option<Vec<Outcome>> {
        self.inner
            .state
            .try_lock()
            .map(|mut st| std::mem::take(&mut st.outbox))
    }

    /// Timings of the thumbnails generated since the last call, oldest first; `None` if a
    /// worker holds the lock (ask again next frame instead of waiting).
    pub fn try_take_timings(&self) -> Option<Vec<Timing>> {
        self.inner
            .state
            .try_lock()
            .map(|mut st| st.timings.drain(..).collect())
    }

    /// Finished pixels of `path` at `class` from memory, or of the closest other class.
    pub fn mem_get(&self, path: &Path, class: u32) -> Option<(u32, Arc<Pixels>, [u8; 4])> {
        Self::mem_lookup(&mut self.inner.state.lock(), path, class)
    }

    /// [`Engine::mem_get`] for a frame: `Err(())` if a worker holds the lock (ask again next
    /// frame instead of waiting).
    #[allow(clippy::result_unit_err, clippy::type_complexity)]
    pub fn try_mem_get(
        &self,
        path: &Path,
        class: u32,
    ) -> Result<Option<(u32, Arc<Pixels>, [u8; 4])>, ()> {
        let mut st = self.inner.state.try_lock().ok_or(())?;
        Ok(Self::mem_lookup(&mut st, path, class))
    }

    fn mem_lookup(st: &mut State, path: &Path, class: u32) -> Option<(u32, Arc<Pixels>, [u8; 4])> {
        st.tick += 1;
        let tick = st.tick;
        let slots = st.mem.get_mut(path)?;
        if let Some(m) = class_index(class).and_then(|i| slots.0[i].as_mut()) {
            m.used = tick;
            return Some((class, m.pixels.clone(), m.average));
        }
        crate::CLASSES
            .iter()
            .rev()
            .find_map(|&c| slots.get(c).map(|m| (c, m.pixels.clone(), m.average)))
    }

    pub fn meta(&self, path: &Path) -> Option<Arc<Meta>> {
        self.inner.state.lock().meta.get(path).cloned()
    }

    /// The file changed or went away: drop what is remembered about it.
    pub fn invalidate(&self, path: &Path) {
        let mut st = self.inner.state.lock();
        st.failed.remove(path);
        st.meta.remove(path);
        if let Some(slots) = st.mem.remove(path) {
            for m in slots.0.into_iter().flatten() {
                st.mem_bytes -= m.bytes;
                st.mem_items -= 1;
            }
        }
    }

    /// Forgets every in-memory result (the quality options changed).
    pub fn forget_all(&self) {
        let mut st = self.inner.state.lock();
        st.mem.clear();
        st.mem_bytes = 0;
        st.mem_items = 0;
        st.failed.clear();
        for t in st.in_flight.values() {
            t.cancel();
        }
        st.in_flight.clear();
        self.inner.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn stats(&self) -> Stats {
        let st = self.inner.state.lock();
        let mut s = st.stats;
        s.pending = st.pending.len();
        s.in_flight = st.in_flight.len();
        s.memory_bytes = st.mem_bytes;
        s.memory_items = st.mem_items;
        s
    }

    /// Nothing pending or running (a posted wish list counts as pending).
    pub fn idle(&self) -> bool {
        {
            let w = self.inner.wish.lock();
            if w.list.is_some() || w.applying {
                return false;
            }
        }
        let st = self.inner.state.lock();
        st.pending.is_empty() && st.in_flight.is_empty() && st.drainers == 0
    }

    /// [`Engine::idle`] without waiting: `None` while a worker or the feeder holds a lock.
    /// For the UI thread, which must not wait behind lower-priority threads.
    pub fn try_idle(&self) -> Option<bool> {
        {
            let w = self.inner.wish.try_lock()?;
            if w.list.is_some() || w.applying {
                return Some(false);
            }
        }
        let st = self.inner.state.try_lock()?;
        Some(st.pending.is_empty() && st.in_flight.is_empty() && st.drainers == 0)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        {
            let mut w = self.inner.wish.lock();
            w.list = None;
            self.inner.shutdown.store(true, Ordering::Release);
        }
        self.inner.wish_cv.notify_all();
        let mut st = self.inner.state.lock();
        st.pending.clear();
        for t in st.in_flight.values() {
            t.cancel();
        }
    }
}

/// The boundary around one item's work: a panic (a decoder bug on this file) becomes this
/// item's error, so it is delivered as failed and forgotten as running like any other error,
/// and the worker goes on. Without it the item would stay "in flight" and its drainer would
/// never be counted out: thumbnails would stop.
fn guard<T>(path: &Path, f: impl FnOnce() -> Result<T, DecodeError>) -> Result<T, DecodeError> {
    // Thumbnails take a smaller share of the memory budget, so a gallery of large files
    // leaves room for the image being viewed.
    let _share = specular_core::budget::enter_share(specular_core::budget::Share::Thumbnails);
    specular_core::isolate::run(format_args!("thumbnail of {}", path.display()), f)
        .unwrap_or_else(|p| Err(p.into()))
}

fn ms_since(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

impl State {
    fn next_priority(&self) -> Priority {
        if self.visible > 0 {
            Priority::Visible
        } else {
            Priority::Prefetch
        }
    }
}

impl Inner {
    /// Applies a wish list (see [`Engine::want`]), waiting for the engine's lock.
    fn want_locked(this: &Arc<Inner>, wanted: Vec<Request>, visible: usize) -> WantTimes {
        let t0 = Instant::now();
        let mut st = this.state.lock();
        let keep: HashSet<&Request> = wanted.iter().collect();
        let mut dropped = 0;
        st.in_flight.retain(|r, tok| {
            let k = keep.contains(r);
            if !k {
                tok.cancel();
                dropped += 1;
            }
            k
        });
        drop(keep);
        st.stats.cancelled += dropped;
        let mut pending = VecDeque::with_capacity(wanted.len());
        let mut vis = 0;
        for (i, r) in wanted.into_iter().enumerate() {
            let done = if r.class == 0 {
                st.meta.contains_key(&r.path)
            } else {
                st.mem
                    .get(&r.path)
                    .is_some_and(|m| m.get(r.class).is_some())
            };
            if done || st.failed.contains(&r.path) || st.in_flight.contains_key(&r) {
                continue;
            }
            if i < visible {
                vis += 1;
            }
            pending.push_back(r);
        }
        let old = std::mem::replace(&mut st.pending, pending);
        st.visible = vis;
        st.stats.pending = st.pending.len();
        let spawn = this.spawn_count(&st);
        st.drainers += spawn;
        let prio = st.next_priority();
        drop(st);
        drop(old);
        let t1 = Instant::now();
        if dropped > 0 {
            this.pool.purge_cancelled();
        }
        let t2 = Instant::now();
        for _ in 0..spawn {
            Inner::submit(this, prio);
        }
        WantTimes {
            locked: t1 - t0,
            purge: t2 - t1,
            submit: t2.elapsed(),
        }
    }

    fn spawn_count(&self, st: &State) -> usize {
        let workers = self.cfg.lock().workers.max(1);
        let want = (st.pending.len() + st.read.len()).min(workers);
        want.saturating_sub(st.drainers)
    }

    fn submit(this: &Arc<Inner>, prio: Priority) {
        let inner = this.clone();
        this.pool
            .submit_labeled(prio, CancelToken::new(), "thumb", move |_| {
                Inner::drain(&inner);
            });
    }

    /// Takes `pending[i]` out of the queue (keeping the count of visible ones right).
    fn take_pending(st: &mut State, i: usize) -> Option<Request> {
        let req = st.pending.remove(i)?;
        if i < st.visible {
            st.visible -= 1;
        }
        Some(req)
    }

    fn drain(this: &Arc<Inner>) {
        let picked = {
            let mut st = this.state.lock();
            if !this.shutdown.load(Ordering::Acquire)
                && let Some(r) = st.read.pop_front()
            {
                drop(st);
                Inner::make_read(this, r);
                (this.wake)();
                Inner::next(this, this.state.lock());
                return;
            }
            if this.shutdown.load(Ordering::Acquire) || st.pending.is_empty() {
                st.drainers -= 1;
                return;
            }
            // The best item whose stream no other worker is reading (that worker takes it).
            let at = if st.streams.is_empty() {
                Some(0)
            } else {
                st.pending.iter().position(|r| {
                    this.opener
                        .stream(&r.path, false)
                        .is_none_or(|p| !st.streams.contains_key(&p.stream))
                })
            };
            let Some(req) = at.and_then(|i| Inner::take_pending(&mut st, i)) else {
                st.drainers -= 1;
                return;
            };
            let tok = CancelToken::new();
            st.in_flight.insert(req.clone(), tok.clone());
            st.resolving.push((req.clone(), tok.clone()));
            (req, tok)
        };
        let (req, tok) = picked;
        let generation = this.generation.load(Ordering::Relaxed);
        let t0 = Instant::now();
        let cfg = this.cfg.lock().clone();
        let mut timing = None;
        let found = guard(&req.path, || Inner::lookup(this, &cfg, &req));
        let pos = match &found {
            Ok(Lookup::Miss(_)) if !tok.is_cancelled() => this.opener.stream(&req.path, true),
            _ => None,
        };
        {
            let mut st = this.state.lock();
            match st.resolving.iter().position(|(_, t)| t.same(&tok)) {
                Some(i) => {
                    st.resolving.swap_remove(i);
                }
                // A stream's worker took it over meanwhile.
                None => {
                    drop(st);
                    Inner::next(this, this.state.lock());
                    return;
                }
            }
        }
        let result = match found {
            Ok(Lookup::Hit(r)) => Ok(Some(r)),
            Ok(Lookup::Miss(_)) if tok.is_cancelled() => Err(DecodeError::Cancelled),
            Ok(Lookup::Miss(key)) => match pos {
                Some(pos) => {
                    let item = Item {
                        req,
                        tok,
                        key: Some(key),
                    };
                    Inner::stream(this, pos.stream, item, generation);
                    Inner::next(this, this.state.lock());
                    return;
                }
                None => guard(&req.path, || {
                    this.opener
                        .open(&req.path)
                        .map_err(DecodeError::from)
                        .and_then(|o| {
                            Inner::make(this, &cfg, &req, &tok, &key, o, ms_since(t0), &mut timing)
                        })
                }),
            },
            Err(e) => Err(e),
        };
        Inner::finish(this, &req, &tok, generation, result, timing, t0);
        Inner::next(this, this.state.lock());
    }

    /// Makes the items of one stream ([`Opener::stream`]) in stream order: `first`, those
    /// other workers hand over meanwhile and the waiting ones of the same stream, until none
    /// is left. When another worker is reading the stream already, `first` is handed to it.
    fn stream(this: &Arc<Inner>, stream: u64, first: Item, generation: u64) {
        {
            let mut st = this.state.lock();
            if let Some(q) = st.streams.get_mut(&stream) {
                q.push(first);
                return;
            }
            st.streams.insert(stream, Vec::new());
        }
        let mut items = vec![first];
        // Where the last batch ended in the stream.
        let mut cursor = None;
        loop {
            {
                let mut st = this.state.lock();
                let handed = st.streams.get_mut(&stream).map(std::mem::take);
                items.extend(handed.unwrap_or_default());
                if !this.shutdown.load(Ordering::Acquire) {
                    Inner::take_batch(this, &mut st, stream, &mut items, &mut cursor);
                }
                if items.is_empty() {
                    // Released under the lock, so nothing is handed over after the last look.
                    st.streams.remove(&stream);
                    // Workers that found only this stream's items left may have stopped.
                    let spawn = this.spawn_count(&st);
                    st.drainers += spawn;
                    let prio = st.next_priority();
                    drop(st);
                    for _ in 0..spawn {
                        Inner::submit(this, prio);
                    }
                    return;
                }
            }
            Inner::stream_batch(this, std::mem::take(&mut items), generation);
        }
    }

    /// Fills `items` (those in hand) to a batch of `stream`: takes over the items of the
    /// stream other workers are still resolving, then the waiting ones in stream order from
    /// where the items in hand start, or from `cursor` (where the last batch ended), or failing
    /// both, from the stream's front. When some of the stream's waiting items are visible,
    /// only those are taken. Moves `cursor` to the end of the batch.
    fn take_batch(
        this: &Arc<Inner>,
        st: &mut State,
        stream: u64,
        items: &mut Vec<Item>,
        cursor: &mut Option<u64>,
    ) {
        let at = |p: &Path| {
            this.opener
                .stream(p, false)
                .filter(|p| p.stream == stream)
                .map(|p| p.at)
        };
        let mut i = 0;
        while i < st.resolving.len() {
            if at(&st.resolving[i].0.path).is_some() {
                let (req, tok) = st.resolving.swap_remove(i);
                items.push(Item {
                    req,
                    tok,
                    key: None,
                });
            } else {
                i += 1;
            }
        }
        let room = STREAM_BATCH.saturating_sub(items.len());
        if room > 0 {
            let ours: Vec<(usize, u64)> = st
                .pending
                .iter()
                .enumerate()
                .filter_map(|(i, r)| at(&r.path).map(|a| (i, a)))
                .collect();
            let visible = st.visible;
            let any_visible = ours.iter().any(|&(i, _)| i < visible);
            let mut cand: Vec<(usize, u64)> = ours
                .into_iter()
                .filter(|&(i, _)| !any_visible || i < visible)
                .collect();
            cand.sort_unstable_by_key(|&(_, a)| a);
            let start = items
                .iter()
                .filter_map(|it| at(&it.req.path))
                .min()
                .or(*cursor);
            let from = start.map_or(0, |s| cand.partition_point(|&(_, a)| a < s));
            let from = if from < cand.len() { from } else { 0 };
            let mut take: Vec<usize> = cand[from..].iter().take(room).map(|&(i, _)| i).collect();
            take.sort_unstable_by(|a, b| b.cmp(a));
            for i in take {
                if let Some(req) = Inner::take_pending(st, i) {
                    let tok = CancelToken::new();
                    st.in_flight.insert(req.clone(), tok.clone());
                    items.push(Item {
                        req,
                        tok,
                        key: None,
                    });
                }
            }
        }
        if let Some(end) = items.iter().filter_map(|it| at(&it.req.path)).max() {
            *cursor = Some(end);
        }
    }

    /// One batch of [`Inner::stream`]: cache hits first, then the misses read in one pass.
    fn stream_batch(this: &Arc<Inner>, mut items: Vec<Item>, generation: u64) {
        let cfg = this.cfg.lock().clone();
        // An item whose place is not known (its stream was closed meanwhile) goes last.
        items.sort_by_cached_key(|it| {
            this.opener
                .stream(&it.req.path, false)
                .map_or(u64::MAX, |p| p.at)
        });
        let mut misses = Vec::with_capacity(items.len());
        let mut hits = 0;
        for it in items {
            let t0 = Instant::now();
            let found = match it.key {
                Some(k) => Ok(Lookup::Miss(k)),
                None => guard(&it.req.path, || Inner::lookup(this, &cfg, &it.req)),
            };
            let result = match found {
                Ok(Lookup::Miss(key)) if !it.tok.is_cancelled() => {
                    misses.push((it, key));
                    continue;
                }
                Ok(Lookup::Miss(_)) => Err(DecodeError::Cancelled),
                Ok(Lookup::Hit(r)) => Ok(Some(r)),
                Err(e) => Err(e),
            };
            Inner::finish(this, &it.req, &it.tok, generation, result, None, t0);
            hits += 1;
        }
        if hits > 0 {
            (this.wake)();
        }
        if misses.is_empty() {
            return;
        }
        let paths: Vec<&Path> = misses.iter().map(|(it, _)| it.req.path.as_path()).collect();
        let mut done = vec![false; misses.len()];
        let mut last = Instant::now();
        // A panic in the archive reader ends the batch; its unread items fail below.
        let what = format!("reading {} thumbnails from their archive", paths.len());
        let broke = specular_core::isolate::run(what, || {
            this.opener.open_stream(&paths, &mut |i, opened| {
                let Some(((it, key), d)) = misses.get(i).zip(done.get_mut(i)) else {
                    return true;
                };
                if std::mem::replace(d, true) {
                    return true;
                }
                // Reading it includes skipping what lies before it in the stream.
                let open_ms = ms_since(last);
                match opened {
                    Ok(opened) if !it.tok.is_cancelled() => {
                        // Made by another worker while this one reads on; when one is waiting
                        // for every worker already, this one makes the oldest.
                        let r = ReadItem {
                            it: Item {
                                req: it.req.clone(),
                                tok: it.tok.clone(),
                                key: Some(*key),
                            },
                            key: *key,
                            opened,
                            open_ms,
                            generation,
                        };
                        let mut st = this.state.lock();
                        let workers = cfg.workers.max(1);
                        st.read.push_back(r);
                        let own = (st.read.len() > workers)
                            .then(|| st.read.pop_front())
                            .flatten();
                        let spawn = this.spawn_count(&st);
                        st.drainers += spawn;
                        let prio = st.next_priority();
                        drop(st);
                        for _ in 0..spawn {
                            Inner::submit(this, prio);
                        }
                        if let Some(r) = own {
                            Inner::make_read(this, r);
                            (this.wake)();
                        }
                    }
                    opened => {
                        let result = match opened {
                            Err(e) if !it.tok.is_cancelled() => Err(e.into()),
                            _ => Err(DecodeError::Cancelled),
                        };
                        Inner::finish(
                            this,
                            &it.req,
                            &it.tok,
                            generation,
                            result,
                            None,
                            Instant::now(),
                        );
                    }
                }
                last = Instant::now();
                // Stop reading when nothing further on is still wanted.
                !this.shutdown.load(Ordering::Acquire)
                    && misses
                        .get(i + 1..)
                        .is_some_and(|rest| rest.iter().any(|(m, _)| !m.tok.is_cancelled()))
            })
        })
        .err();
        for ((it, _), d) in misses.iter().zip(&done) {
            if !*d {
                let e = if it.tok.is_cancelled() {
                    DecodeError::Cancelled
                } else if let Some(p) = &broke {
                    p.clone().into()
                } else {
                    std::io::Error::other("not read from its archive").into()
                };
                Inner::finish(
                    this,
                    &it.req,
                    &it.tok,
                    generation,
                    Err(e),
                    None,
                    Instant::now(),
                );
            }
        }
    }

    /// Makes an item its stream's worker read ([`State::read`]).
    fn make_read(this: &Arc<Inner>, r: ReadItem) {
        let cfg = this.cfg.lock().clone();
        let t0 = Instant::now();
        let mut timing = None;
        let ReadItem {
            it,
            key,
            opened,
            open_ms,
            generation,
        } = r;
        let result = if it.tok.is_cancelled() {
            Err(DecodeError::Cancelled)
        } else {
            guard(&it.req.path, || {
                Inner::make(
                    this,
                    &cfg,
                    &it.req,
                    &it.tok,
                    &key,
                    opened,
                    open_ms,
                    &mut timing,
                )
            })
        };
        Inner::finish(this, &it.req, &it.tok, generation, result, timing, t0);
    }

    /// Keeps draining: one item per job so the pool can interleave other work and apply its
    /// foreground gate between items.
    fn next(this: &Arc<Inner>, mut st: parking_lot::MutexGuard<'_, State>) {
        let more = !(st.pending.is_empty() && st.read.is_empty())
            && !this.shutdown.load(Ordering::Acquire);
        let prio = st.next_priority();
        if !more {
            st.drainers -= 1;
        }
        drop(st);
        (this.wake)();
        if more {
            Inner::submit(this, prio);
        }
    }

    /// Delivers an item's result (if it is still wanted) and forgets it as running.
    #[allow(clippy::too_many_arguments)]
    fn finish(
        this: &Arc<Inner>,
        req: &Request,
        tok: &CancelToken,
        generation: u64,
        result: Result<Option<Ready>, DecodeError>,
        timing: Option<Timing>,
        t0: Instant,
    ) {
        let mut st = this.state.lock();
        let still_wanted = st.in_flight.remove(req).is_some() && !tok.is_cancelled();
        let current = generation == this.generation.load(Ordering::Relaxed);
        match result {
            Ok(_) if !still_wanted || !current => {}
            Ok(Some(ready)) => {
                if ready.from_cache {
                    st.stats.from_cache += 1;
                } else {
                    st.stats.generated += 1;
                }
                specular_log::trace!(target: specular_log::CACHE, "thumb {} class {} in {:.1} ms (cache {})",
                    req.path.display(), req.class, t0.elapsed().as_secs_f64() * 1e3, ready.from_cache);
                Inner::remember(this, &mut st, &ready);
                st.outbox.push(Outcome::Ready(ready));
                if let Some(t) = timing {
                    if st.timings.len() == TIMINGS_KEPT {
                        st.timings.pop_front();
                    }
                    st.timings.push_back(t);
                }
            }
            Ok(None) => {}
            Err(DecodeError::Cancelled) => {}
            Err(e) if still_wanted => {
                st.stats.failed += 1;
                st.failed.insert(req.path.clone());
                st.outbox.push(Outcome::Failed {
                    path: req.path.clone(),
                    memory: match e {
                        DecodeError::NeedsMemory(n) => Some(n),
                        _ => None,
                    },
                    error: Arc::new(e),
                });
            }
            Err(_) => {}
        }
    }

    fn remember(this: &Arc<Inner>, st: &mut State, r: &Ready) {
        st.meta.insert(r.path.clone(), r.meta.clone());
        let Some(px) = r.pixels.clone() else {
            return;
        };
        st.tick += 1;
        let used = st.tick;
        let bytes = px.bytes();
        let Some(ci) = class_index(r.class) else {
            return;
        };
        let item = MemItem {
            pixels: px,
            average: r.average,
            used,
            bytes,
        };
        let old = match st.mem.get_mut(&r.path) {
            Some(slots) => slots.0[ci].replace(item),
            None => {
                let mut slots = MemSlots::default();
                slots.0[ci] = Some(item);
                st.mem.insert(r.path.clone(), slots);
                None
            }
        };
        match old {
            Some(old) => st.mem_bytes -= old.bytes,
            None => st.mem_items += 1,
        }
        st.mem_bytes += bytes;
        let budget = this.cfg.lock().memory_budget;
        if st.mem_bytes > budget {
            // Down to 90 % in one go, so the sort is paid once per tenth of the budget.
            let mut by_age: Vec<(u64, &Path, usize)> = Vec::with_capacity(st.mem_items);
            for (p, slots) in &st.mem {
                for (i, m) in slots.0.iter().enumerate() {
                    if let Some(m) = m {
                        by_age.push((m.used, p.as_path(), i));
                    }
                }
            }
            by_age.sort_unstable_by_key(|e| e.0);
            let mut freed = 0;
            let mut evict: Vec<(PathBuf, usize)> = Vec::new();
            for (_, p, i) in by_age {
                if st.mem_bytes - freed <= budget / 10 * 9 {
                    break;
                }
                if let Some(m) = st.mem.get(p).and_then(|s| s.0[i].as_ref()) {
                    freed += m.bytes;
                    evict.push((p.to_path_buf(), i));
                }
            }
            for (p, i) in evict {
                if let Some(slots) = st.mem.get_mut(&p) {
                    if slots.0[i].take().is_some() {
                        st.mem_items -= 1;
                    }
                    if slots.is_empty() {
                        st.mem.remove(&p);
                    }
                }
            }
            st.mem_bytes -= freed;
        }
    }

    /// The image's meta, read from its header and metadata without decoding pixels; `None`
    /// when it no longer opens.
    fn reread_meta(this: &Arc<Inner>, path: &Path) -> Option<Meta> {
        let opened = this.opener.open(path).ok()?;
        let session = this.registry.open_for_thumbnail(opened.source).ok()?;
        Some(generate::meta_of(session.as_ref()).without_exif_tags())
    }

    /// A cached subframe's star metrics into `meta`: from their blob, else measured now
    /// (an entry made before they were kept) and stored.
    fn astro_from_cache(
        this: &Arc<Inner>,
        cfg: &EngineConfig,
        key: &ThumbKey,
        path: &Path,
        meta: &mut Meta,
    ) {
        let Some(cache) = &this.cache else { return };
        let Some(a) = meta.astro.as_mut() else { return };
        if let Some(m) = cache
            .get_blob(key, crate::cache::ASTRO_BLOB)
            .and_then(|b| specular_decode::astro::FrameMetrics::from_bytes(&b))
        {
            a.metrics = Some(m);
            return;
        }
        let measured = this.opener.open(path).ok().and_then(|o| {
            let s = this.registry.open_for_thumbnail(o.source).ok()?;
            specular_decode::astro::measure(s.as_ref(), &CancelToken::new())?.ok()
        });
        if let Some(m) = measured {
            a.metrics = Some(m);
            if cfg.write_cache
                && let Err(e) = cache.put_blob(key, crate::cache::ASTRO_BLOB, &m.to_bytes())
            {
                specular_log::debug!(target: specular_log::CACHE, "subframe metrics cache write failed: {e}");
            }
        }
    }

    /// Reads the thumbnail from the disk cache, or says what key to make it under.
    fn lookup(this: &Arc<Inner>, cfg: &EngineConfig, req: &Request) -> Result<Lookup, DecodeError> {
        let key = this.opener.key(&req.path)?;
        let digester = cfg
            .digester
            .as_ref()
            .filter(|d| req.class != 0 && d.class() == req.class);
        if let Some(cache) = &this.cache {
            let classes: &[u32] = if req.class == 0 {
                &crate::CLASSES
            } else {
                std::slice::from_ref(&req.class)
            };
            for &c in classes {
                if let Some(mut e) = cache.get(&key, c, cfg.flags) {
                    if e.meta_stale
                        && let Some(meta) = Self::reread_meta(this, &req.path)
                    {
                        // Written before this metadata was read: the pixels stay, the meta
                        // is read again from the header (no decode) and stored with them.
                        if cfg.write_cache
                            && let Err(err) = cache.refresh_meta(&key, c, cfg.flags, &meta)
                        {
                            specular_log::debug!(target: specular_log::CACHE, "thumbnail cache meta refresh failed: {err}");
                        }
                        e.meta = meta;
                    }
                    if e.meta.astro.is_some() {
                        Self::astro_from_cache(this, cfg, &key, &req.path, &mut e.meta);
                    }
                    let pixels = (req.class != 0).then(|| {
                        e.pixels.prepare();
                        Arc::new(e.pixels)
                    });
                    let digest = e
                        .digest
                        .filter(|d| digester.is_some_and(|g| g.id() == d.id))
                        .map(|d| Arc::from(d.bytes));
                    return Ok(Lookup::Hit(Ready {
                        path: req.path.clone(),
                        class: req.class,
                        pixels,
                        average: e.average,
                        meta: Arc::new(e.meta),
                        sharp: true,
                        from_cache: true,
                        digest,
                    }));
                }
            }
        }
        Ok(Lookup::Miss(key))
    }

    /// Makes the thumbnail from the opened file and stores it under `key`; `timing` gets the
    /// stages (`open_ms`: what reading the key and opening took).
    #[allow(clippy::too_many_arguments)]
    fn make(
        this: &Arc<Inner>,
        cfg: &EngineConfig,
        req: &Request,
        tok: &CancelToken,
        key: &ThumbKey,
        opened: Opened,
        open_ms: f64,
        timing: &mut Option<Timing>,
    ) -> Result<Option<Ready>, DecodeError> {
        let digester = cfg
            .digester
            .as_ref()
            .filter(|d| req.class != 0 && d.class() == req.class);
        let bytes = opened.source.len() as u64;
        // An encrypted archive's page is kept in memory only.
        let write_cache = cfg.write_cache && !opened.private;
        // What the work cost besides its wall time: the path the selector took (a note the
        // decoder leaves on this thread) and the file pages read meanwhile (major faults).
        let _ = specular_core::paths::take_note();
        let usage = specular_core::paths::thread_usage();
        let t_parse = Instant::now();
        let session = this.registry.open_for_thumbnail(opened.source)?;
        let mut meta = generate::meta_of(session.as_ref()).without_exif_tags();
        if let (Some(a), Some(cache)) = (meta.astro.as_mut(), &this.cache) {
            a.metrics = cache
                .get_blob(key, crate::cache::ASTRO_BLOB)
                .and_then(|b| specular_decode::astro::FrameMetrics::from_bytes(&b));
        }
        let meta = Arc::new(meta);
        let parse_ms = ms_since(t_parse);
        if req.class == 0 {
            return Ok(Some(Ready {
                path: req.path.clone(),
                class: 0,
                pixels: None,
                average: [128, 128, 128, 255],
                meta,
                sharp: true,
                from_cache: false,
                digest: None,
            }));
        }
        let mut rough = |px: Pixels| {
            px.prepare();
            let average = px.average();
            let mut st = this.state.lock();
            if st.in_flight.contains_key(req) {
                st.outbox.push(Outcome::Ready(Ready {
                    path: req.path.clone(),
                    class: req.class,
                    pixels: Some(Arc::new(px)),
                    average,
                    meta: meta.clone(),
                    sharp: false,
                    from_cache: false,
                    digest: None,
                }));
                drop(st);
                (this.wake)();
            }
        };
        let t_decode = Instant::now();
        // A larger class than the digest's (HiDPI tiles) makes the digest's class too, from
        // the same decode, so the duplicate finder has its signature without another one.
        let sig_also = cfg.digester.as_ref().filter(|g| {
            g.class() < req.class
                && write_cache
                && this
                    .cache
                    .as_ref()
                    .is_some_and(|c| c.get_digest(key, g.class(), cfg.flags, g.id()).is_none())
        });
        // A very large image while the pool has room: the decoder's threads.
        let mut opts = cfg.generate;
        let slot = if opts.parallel {
            None
        } else {
            let info = session.info();
            let pixels = u64::from(info.width) * u64::from(info.height);
            let mut st = this.state.lock();
            let others = st.in_flight.len().saturating_sub(1);
            decode_in_parallel(pixels, others, st.parallel, cfg.workers).then(|| {
                st.parallel += 1;
                opts.parallel = true;
                specular_log::debug!(target: specular_log::CACHE,
                    "thumbnail of {} ({:.0} MP): threaded decode", req.path.display(), pixels as f64 / 1e6);
                ParallelSlot(this)
            })
        };
        let (px, also) = generate::generate_also(
            session.as_ref(),
            req.class,
            sig_also.map(|g| g.class()),
            opts,
            tok,
            &mut rough,
        )?;
        drop(slot);
        // A subframe's star metrics, from the plane the thumbnail was just made from (the
        // session keeps it), so they are ready before cull mode asks.
        let meta = match meta.astro.as_ref() {
            Some(a) if a.metrics.is_none() => {
                let mut m = (*meta).clone();
                if let Some(Ok(fm)) = specular_decode::astro::measure(session.as_ref(), tok)
                    && let Some(a) = m.astro.as_mut()
                {
                    a.metrics = Some(fm);
                    if let Some(cache) = this.cache.as_ref().filter(|_| write_cache)
                        && let Err(e) =
                            cache.put_blob(key, crate::cache::ASTRO_BLOB, &fm.to_bytes())
                    {
                        specular_log::debug!(target: specular_log::CACHE, "subframe metrics cache write failed: {e}");
                    }
                }
                Arc::new(m)
            }
            _ => meta,
        };
        let decode_ms = ms_since(t_decode);
        let (faults, io_ms) = match (usage, specular_core::paths::thread_usage()) {
            (Some(a), Some(b)) => {
                let faults = b.major_faults.saturating_sub(a.major_faults);
                let idle = (parse_ms + decode_ms - (b.cpu_us - a.cpu_us) / 1e3).max(0.0);
                (faults, idle.min(faults as f64 * MAX_FAULT_MS))
            }
            _ => (0, 0.0),
        };
        *timing = Some(Timing {
            path: req.path.clone(),
            format: meta.format.clone(),
            bytes,
            open_ms,
            parse_ms,
            decode_ms,
            io_ms,
            faults,
            note: specular_core::paths::take_note(),
            cached: false,
        });
        // The alpha scan and premultiplied copy happen here, on the worker, not at upload.
        px.prepare();
        // From the pixels as made, before the cache's lossy coding.
        let digest = digester.map(|g| Digest {
            id: g.id(),
            bytes: g.digest(&px),
        });
        if let Some(cache) = &this.cache
            && write_cache
        {
            match cache.put_with_digest(key, req.class, cfg.flags, &px, &meta, digest.as_ref()) {
                Ok(()) => {
                    if let Some(t) = timing.as_mut() {
                        t.cached = true;
                    }
                }
                Err(e) => {
                    specular_log::debug!(target: specular_log::CACHE, "thumbnail cache write failed: {e}");
                }
            }
        }
        if let (Some(g), Some(also), Some(cache)) = (sig_also, also, &this.cache) {
            also.pixels.prepare();
            let digest = Digest {
                id: g.id(),
                bytes: g.digest(&also.pixels),
            };
            if let Err(e) = cache.put_with_digest(
                key,
                g.class(),
                cfg.flags,
                &also.pixels,
                &meta,
                Some(&digest),
            ) {
                specular_log::debug!(target: specular_log::CACHE, "thumbnail cache write failed: {e}");
            }
        }
        Ok(Some(Ready {
            path: req.path.clone(),
            class: req.class,
            average: px.average(),
            pixels: Some(Arc::new(px)),
            meta,
            sharp: true,
            from_cache: false,
            digest: digest.map(|d| Arc::from(d.bytes)),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// An opener whose `open` blocks until released, counting calls, to test ordering and
    /// cancellation without real decodes.
    struct Gate {
        opened: Mutex<Vec<PathBuf>>,
        release: AtomicBool,
    }

    impl Opener for Gate {
        fn key(&self, path: &Path) -> std::io::Result<ThumbKey> {
            Ok(ThumbKey::of_bytes(
                path.as_os_str().as_encoded_bytes(),
                None,
            ))
        }
        fn open(&self, path: &Path) -> std::io::Result<Opened> {
            self.opened.lock().push(path.to_path_buf());
            while !self.release.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(std::io::Error::other("test opener"))
        }
    }

    fn reqs(names: &[&str]) -> Vec<Request> {
        names
            .iter()
            .map(|n| Request {
                path: PathBuf::from(n),
                class: 128,
            })
            .collect()
    }

    fn wait(f: impl Fn() -> bool) {
        let t = Instant::now();
        while !f() {
            assert!(t.elapsed() < Duration::from_secs(10), "timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Only a very large image decodes threaded, a few at a time, while at most half the
    /// workers are busy with other items; normal images never do.
    #[test]
    fn threaded_decodes_for_very_large_images_only() {
        let gp = 38_400 * 38_400;
        // 24 MP photo: never, however idle the pool.
        assert!(!decode_in_parallel(6000 * 4000, 0, 0, 8));
        assert!(!decode_in_parallel(PARALLEL_MIN_PIXELS - 1, 0, 0, 8));
        assert!(decode_in_parallel(PARALLEL_MIN_PIXELS, 0, 0, 8));
        assert!(decode_in_parallel(gp, 3, 0, 8));
        // Half the workers or more busy with other items: on its worker.
        assert!(!decode_in_parallel(gp, 4, 0, 8));
        assert!(!decode_in_parallel(gp, 7, 0, 8));
        // Two at a time with eight workers, one with fewer than eight.
        assert!(decode_in_parallel(gp, 1, 1, 8));
        assert!(!decode_in_parallel(gp, 1, 2, 8));
        assert!(!decode_in_parallel(gp, 1, 1, 4));
        // Small pools: alone on two workers, or on one.
        assert!(decode_in_parallel(gp, 0, 0, 2));
        assert!(!decode_in_parallel(gp, 1, 0, 2));
        assert!(decode_in_parallel(gp, 0, 0, 1));
        assert!(decode_in_parallel(gp, 0, 0, 0));
    }

    #[test]
    fn late_binding_order_and_cancellation() {
        let pool = Arc::new(JobPool::new(1, "thumb-test"));
        let gate = Arc::new(Gate {
            opened: Mutex::new(vec![]),
            release: AtomicBool::new(false),
        });
        let cfg = EngineConfig {
            workers: 1,
            ..EngineConfig::default()
        };
        let e = Engine::new(
            cfg,
            Arc::new(Registry::new()),
            pool,
            gate.clone(),
            None,
            Arc::new(|| {}),
        );
        e.want(reqs(&["a", "b", "c"]), 3);
        wait(|| gate.opened.lock().len() == 1);
        assert_eq!(gate.opened.lock()[0], PathBuf::from("a"));
        // The user scrolled: "a" is no longer wanted (cancelled), "z" comes first now.
        e.want(reqs(&["z", "y", "c"]), 2);
        assert_eq!(e.stats().cancelled, 1);
        gate.release.store(true, Ordering::Release);
        wait(|| e.idle());
        let order = gate.opened.lock().clone();
        assert_eq!(order, ["a", "z", "y", "c"].map(PathBuf::from));
        // Failures are reported for wanted items only, and not retried.
        let out = e.take_ready();
        let failed: Vec<_> = out
            .iter()
            .filter_map(|o| match o {
                Outcome::Failed { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(failed, ["z", "y", "c"].map(PathBuf::from));
        e.want(reqs(&["z"]), 1);
        assert!(e.idle());
        e.invalidate(Path::new("z"));
        e.want(reqs(&["z"]), 1);
        wait(|| e.idle());
        assert_eq!(gate.opened.lock().len(), 5);
    }

    #[test]
    fn posted_lists_are_applied_by_the_feeder() {
        let pool = Arc::new(JobPool::new(1, "thumb-test"));
        let gate = Arc::new(Gate {
            opened: Mutex::new(vec![]),
            release: AtomicBool::new(false),
        });
        let cfg = EngineConfig {
            workers: 1,
            ..EngineConfig::default()
        };
        let e = Engine::new(
            cfg,
            Arc::new(Registry::new()),
            pool,
            gate.clone(),
            None,
            Arc::new(|| {}),
        );
        let post = |l: Vec<Request>, v: usize| wait(|| e.try_want(l.clone(), v));
        post(reqs(&["a", "b"]), 2);
        assert!(!e.idle());
        wait(|| gate.opened.lock().len() == 1);
        assert_eq!(gate.opened.lock()[0], PathBuf::from("a"));
        // A newer list replaces the posted one; "a" (running) is cancelled once it is applied.
        post(reqs(&["x"]), 1);
        post(reqs(&["z", "y"]), 2);
        wait(|| e.stats().cancelled == 1);
        // `want` after a post: the newer (synchronous) list wins.
        post(reqs(&["q"]), 1);
        e.want(reqs(&["z", "y"]), 2);
        gate.release.store(true, Ordering::Release);
        wait(|| e.idle());
        let order = gate.opened.lock().clone();
        assert_eq!(order[0], PathBuf::from("a"));
        assert!(!order.contains(&PathBuf::from("q")), "{order:?}");
        assert!(order.ends_with(&["z", "y"].map(PathBuf::from)), "{order:?}");
    }

    /// A solid archive: paths `s/NN` lie at NN in stream 7 (known once the archive was opened
    /// through [`Opener::stream`]); other paths are plain files. Records each pass over the
    /// stream; reads only as many members as `steps` allows.
    struct Solid {
        known: AtomicBool,
        passes: Mutex<Vec<Vec<PathBuf>>>,
        plain: Mutex<Vec<PathBuf>>,
        steps: AtomicU64,
        read: AtomicU64,
        /// Opening the archive takes this long times the how-manieth opener a worker is
        /// (one at a time, as the app's opener holds one open per archive); 0 opens at once.
        open_ms: AtomicU64,
        opens: AtomicU64,
    }

    impl Solid {
        fn new(steps: u64) -> Arc<Solid> {
            Arc::new(Solid {
                known: AtomicBool::new(false),
                passes: Mutex::new(vec![]),
                plain: Mutex::new(vec![]),
                steps: AtomicU64::new(steps),
                read: AtomicU64::new(0),
                open_ms: AtomicU64::new(0),
                opens: AtomicU64::new(0),
            })
        }

        fn at(path: &Path) -> Option<u64> {
            path.strip_prefix("s").ok()?.to_str()?.parse().ok()
        }
    }

    impl Opener for Solid {
        fn key(&self, path: &Path) -> std::io::Result<ThumbKey> {
            Ok(ThumbKey::of_bytes(
                path.as_os_str().as_encoded_bytes(),
                None,
            ))
        }
        fn open(&self, path: &Path) -> std::io::Result<Opened> {
            self.plain.lock().push(path.to_path_buf());
            Err(std::io::Error::other("test opener"))
        }
        fn stream(&self, path: &Path, open: bool) -> Option<StreamPos> {
            let at = Solid::at(path)?;
            if open {
                let ms = self.open_ms.load(Ordering::Acquire);
                if ms > 0 && !self.known.load(Ordering::Acquire) {
                    let n = self.opens.fetch_add(1, Ordering::AcqRel) + 1;
                    std::thread::sleep(Duration::from_millis(ms * n));
                }
                self.known.store(true, Ordering::Release);
            }
            self.known
                .load(Ordering::Acquire)
                .then_some(StreamPos { stream: 7, at })
        }
        fn open_stream(
            &self,
            paths: &[&Path],
            each: &mut dyn FnMut(usize, std::io::Result<Opened>) -> bool,
        ) {
            self.passes.lock().push(vec![]);
            for (i, p) in paths.iter().enumerate() {
                let n = self.read.fetch_add(1, Ordering::AcqRel);
                while self.steps.load(Ordering::Acquire) <= n {
                    std::thread::sleep(Duration::from_millis(1));
                }
                if let Some(pass) = self.passes.lock().last_mut() {
                    pass.push(p.to_path_buf());
                }
                if !each(i, Err(std::io::Error::other("test stream"))) {
                    break;
                }
            }
        }
    }

    fn solid_engine(solid: &Arc<Solid>, workers: usize) -> Engine {
        Engine::new(
            EngineConfig {
                workers,
                ..EngineConfig::default()
            },
            Arc::new(Registry::new()),
            Arc::new(JobPool::new(workers, "thumb-test")),
            solid.clone(),
            None,
            Arc::new(|| {}),
        )
    }

    #[test]
    fn solid_members_are_read_in_stream_order_in_few_passes() {
        let solid = Solid::new(u64::MAX);
        let e = solid_engine(&solid, 4);
        // Wanted in screen order, which is not stream order, with plain files between.
        let mut names: Vec<String> = (0..60).rev().map(|i| format!("s/{i:02}")).collect();
        names.insert(3, "plain-a".into());
        names.insert(30, "plain-b".into());
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        e.want(reqs(&names), 20);
        wait(|| e.idle());
        let passes = solid.passes.lock().clone();
        let mut all: Vec<PathBuf> = passes.iter().flatten().cloned().collect();
        for p in &passes {
            let at: Vec<u64> = p.iter().filter_map(|p| Solid::at(p)).collect();
            assert!(at.is_sorted(), "{at:?}");
        }
        // Each member read once; a handful of passes (the first worker's batch, what other
        // workers handed over while it opened the archive, what the batch cap left).
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 60);
        assert_eq!(passes.iter().map(Vec::len).sum::<usize>(), 60);
        assert!(passes.len() <= 4, "{} passes", passes.len());
        // Plain files open on their own; everything failed (the test opener has no bytes).
        assert_eq!(solid.plain.lock().len(), 2);
        let failed = e
            .take_ready()
            .iter()
            .filter(|o| matches!(o, Outcome::Failed { .. }))
            .count();
        assert_eq!(failed, 62);
    }

    #[test]
    fn the_first_members_are_read_first_while_workers_race_to_open() {
        let solid = Solid::new(u64::MAX);
        solid.open_ms.store(30, Ordering::Release);
        let e = solid_engine(&solid, 4);
        // Every worker takes one of the first members and opens the archive; the first to
        // have it open reads for all, the members the others hold included.
        let names: Vec<String> = (0..100).map(|i| format!("s/{i:02}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        e.want(reqs(&names), 0);
        wait(|| e.idle());
        assert!(solid.opens.load(Ordering::Acquire) > 1);
        let passes = solid.passes.lock().clone();
        let all: Vec<u64> = passes
            .iter()
            .flatten()
            .filter_map(|p| Solid::at(p))
            .collect();
        // One run through the stream, in batches.
        assert_eq!(all, (0..100).collect::<Vec<u64>>(), "{passes:?}");
    }

    #[test]
    fn a_solid_pass_stops_when_nothing_further_is_wanted() {
        let solid = Solid::new(0);
        let e = solid_engine(&solid, 1);
        let names: Vec<String> = (0..10).map(|i| format!("s/{i:02}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        e.want(reqs(&names), 10);
        // One worker: the first item opened the archive, the other nine joined its batch.
        wait(|| solid.read.load(Ordering::Acquire) == 1);
        assert_eq!(e.stats().in_flight, 10);
        // The user moved on before the first member was read.
        e.want(reqs(&["plain"]), 1);
        assert_eq!(e.stats().cancelled, 10);
        solid.steps.store(u64::MAX, Ordering::Release);
        wait(|| e.idle());
        // The pass ended after the member it was reading: the other nine were not read.
        assert_eq!(*solid.passes.lock(), vec![vec![PathBuf::from("s/00")]]);
        assert_eq!(*solid.plain.lock(), vec![PathBuf::from("plain")]);
        // Nothing reported for the cancelled members.
        let out = e.take_ready();
        assert_eq!(out.len(), 1, "{out:?}");
    }

    fn write_png(path: &Path, w: u32, h: u32) {
        let rgba: Vec<u8> = (0..w * h)
            .flat_map(|i| [(i % w) as u8, (i / w) as u8, 200, 255])
            .collect();
        std::fs::write(path, specular_png_encode(w, h, &rgba)).unwrap();
    }

    /// A minimal stored-deflate PNG writer, so the test needs no encoder crate.
    fn specular_png_encode(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        fn crc(data: &[u8]) -> u32 {
            let mut c = 0xFFFF_FFFFu32;
            for &b in data {
                c ^= b as u32;
                for _ in 0..8 {
                    c = if c & 1 != 0 {
                        0xEDB8_8320 ^ (c >> 1)
                    } else {
                        c >> 1
                    };
                }
            }
            !c
        }
        fn chunk(o: &mut Vec<u8>, t: &[u8], d: &[u8]) {
            o.extend_from_slice(&(d.len() as u32).to_be_bytes());
            let mut td = t.to_vec();
            td.extend_from_slice(d);
            o.extend_from_slice(&td);
            o.extend_from_slice(&crc(&td).to_be_bytes());
        }
        let mut raw = Vec::new();
        for y in 0..h as usize {
            raw.push(0);
            raw.extend_from_slice(&rgba[y * w as usize * 4..(y + 1) * w as usize * 4]);
        }
        let mut z = vec![0x78, 0x01];
        for (i, blk) in raw.chunks(65535).enumerate() {
            let last = (i + 1) * 65535 >= raw.len();
            z.push(last as u8);
            z.extend_from_slice(&(blk.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!(blk.len() as u16)).to_le_bytes());
            z.extend_from_slice(blk);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &x in &raw {
            a = (a + x as u32) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&(b << 16 | a).to_be_bytes());
        let mut o = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(&mut o, b"IHDR", &ihdr);
        chunk(&mut o, b"IDAT", &z);
        chunk(&mut o, b"IEND", &[]);
        o
    }

    /// A decoder that panics on one file (the test hook trips on its name) fails that
    /// thumbnail only: the others are made, the engine goes idle (no item stays in flight, no
    /// drainer is lost), the workers are all there, and later requests still work.
    #[test]
    fn a_panicking_decoder_fails_one_thumbnail_only() {
        let dir = crate::cache::test_dir("engine-panic");
        let names = ["a.png", "b-specular-test-panic.png", "c.png", "d.png"];
        for n in names {
            write_png(&dir.join(n), 64, 48);
        }
        let pool = Arc::new(JobPool::new(2, "thumb-test"));
        let e = Engine::new(
            EngineConfig {
                workers: 2,
                ..EngineConfig::default()
            },
            Arc::new(Registry::new()),
            pool.clone(),
            Arc::new(FileOpener),
            None,
            Arc::new(|| {}),
        );
        let want: Vec<Request> = names
            .iter()
            .map(|n| Request {
                path: dir.join(n),
                class: 128,
            })
            .collect();
        e.want(want, 4);
        wait(|| e.idle());
        let out = e.take_ready();
        let mut ready: Vec<_> = out
            .iter()
            .filter_map(|o| match o {
                Outcome::Ready(r) => Some(r.path.file_name().unwrap().to_owned()),
                _ => None,
            })
            .collect();
        ready.sort();
        assert_eq!(ready, ["a.png", "c.png", "d.png"]);
        let failed: Vec<_> = out
            .iter()
            .filter_map(|o| match o {
                Outcome::Failed { path, error, .. } => Some((path.clone(), error.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(failed.len(), 1, "{out:?}");
        assert_eq!(failed[0].0, dir.join(names[1]));
        assert!(
            matches!(*failed[0].1, DecodeError::Internal(_)),
            "{}",
            failed[0].1
        );
        assert!(failed[0].1.to_string().starts_with("internal error"));
        assert_eq!(e.stats().in_flight, 0);
        assert_eq!(pool.alive(), 2);
        // Later work still runs.
        let again = dir.join("e.png");
        write_png(&again, 64, 48);
        e.want(
            vec![Request {
                path: again.clone(),
                class: 128,
            }],
            1,
        );
        wait(|| e.idle());
        assert!(matches!(e.take_ready().last(), Some(Outcome::Ready(r)) if r.path == again));
    }

    #[test]
    fn subframes_get_star_metrics_once_and_from_the_cache() {
        use specular_astro::synth::{Field, Profile, Psf, fits_u16};
        let dir = crate::cache::test_dir("engine-astro");
        let img = dir.join("light_0001.fits");
        let f = Field::grid(
            600,
            400,
            Psf::round(Profile::Gaussian, 4.0),
            30,
            (1000.0, 6000.0),
            900.0,
            9.0,
            3,
        );
        let cards = [
            ("OBJECT", "'M 42'"),
            ("FILTER", "'L'"),
            ("IMAGETYP", "'LIGHT'"),
            ("EXPTIME", "120."),
        ];
        std::fs::write(&img, fits_u16(&f.render(), 600, 400, &cards)).unwrap();
        let cache = Arc::new(DiskCache::new(dir.join("cache"), 64 << 20));
        let pool = Arc::new(JobPool::new(2, "thumb-test"));
        let reg = Arc::new(Registry::new());
        let mk = || {
            Engine::new(
                EngineConfig::default(),
                reg.clone(),
                pool.clone(),
                Arc::new(FileOpener),
                Some(cache.clone()),
                Arc::new(|| {}),
            )
        };
        let want = vec![Request {
            path: img.clone(),
            class: 128,
        }];
        let ready = |e: &Engine| {
            e.want(want.clone(), 1);
            wait(|| e.idle());
            let out = e.take_ready();
            match out.into_iter().last() {
                Some(Outcome::Ready(r)) => r,
                o => panic!("{o:?}"),
            }
        };
        let r = ready(&mk());
        assert!(!r.from_cache);
        assert_eq!(r.meta.exposure, Some(120.0));
        let a = r.meta.astro.as_ref().unwrap();
        assert_eq!(a.object.as_deref(), Some("M 42"));
        assert_eq!(a.frame_type.as_deref(), Some("light"));
        let m = a.metrics.unwrap();
        assert_eq!(m.stars, 30);
        assert!((m.fwhm.unwrap() - 4.0).abs() < 0.12, "{m:?}");
        // A new engine: the entry's header facts and the metrics' blob.
        let r = ready(&mk());
        assert!(r.from_cache);
        assert_eq!(r.meta.astro.as_ref().unwrap().metrics, Some(m));
        // An entry without its blob (made before metrics were kept) gets them measured.
        let key = FileOpener.key(&img).unwrap();
        cache.remove_blob(&key, crate::cache::ASTRO_BLOB);
        let r = ready(&mk());
        assert!(r.from_cache);
        assert_eq!(r.meta.astro.as_ref().unwrap().metrics, Some(m));
    }

    #[test]
    fn generates_caches_and_hits() {
        let dir = crate::cache::test_dir("engine");
        let img = dir.join("pic.png");
        write_png(&img, 300, 200);
        let cache = Arc::new(DiskCache::new(dir.join("cache"), 64 << 20));
        let pool = Arc::new(JobPool::new(2, "thumb-test"));
        let reg = Arc::new(Registry::new());
        let mk = || {
            Engine::new(
                EngineConfig::default(),
                reg.clone(),
                pool.clone(),
                Arc::new(FileOpener),
                Some(cache.clone()),
                Arc::new(|| {}),
            )
        };
        let e = mk();
        let want = vec![Request {
            path: img.clone(),
            class: 128,
        }];
        e.want(want.clone(), 1);
        wait(|| e.idle());
        let out = e.take_ready();
        let Some(Outcome::Ready(r)) = out.last() else {
            panic!("{out:?}")
        };
        assert!(r.sharp && !r.from_cache);
        let px = r.pixels.as_ref().unwrap();
        assert_eq!((px.width, px.height), (128, 85));
        assert_eq!(r.meta.format, "PNG");
        assert_eq!((r.meta.width, r.meta.height), (300, 200));
        // Generated from the file: one timing, with the file's size and stages.
        let t = e.try_take_timings().unwrap();
        assert_eq!(t.len(), 1, "{t:?}");
        assert_eq!(
            (t[0].path.as_path(), t[0].format.as_str()),
            (img.as_path(), "PNG")
        );
        assert_eq!(t[0].bytes, std::fs::metadata(&img).unwrap().len());
        assert!(t[0].open_ms >= 0.0 && t[0].parse_ms >= 0.0 && t[0].decode_ms > 0.0);
        assert!(t[0].io_ms <= t[0].faults as f64 * MAX_FAULT_MS);
        assert!(t[0].cached);
        assert!(e.try_take_timings().unwrap().is_empty());
        // In memory now: wanting it again does nothing.
        assert!(e.mem_get(&img, 128).is_some());
        e.want(want.clone(), 1);
        assert!(e.idle());
        // A fresh engine (new process) hits the disk cache.
        let e2 = mk();
        e2.want(want.clone(), 1);
        wait(|| e2.idle());
        let out = e2.take_ready();
        let Some(Outcome::Ready(r)) = out.last() else {
            panic!("{out:?}")
        };
        assert!(r.from_cache);
        assert_eq!(e2.stats().from_cache, 1);
        // Read from the cache: nothing for the advisor.
        assert!(e2.try_take_timings().unwrap().is_empty());
        // Meta-only requests come from any cached class.
        let e3 = mk();
        e3.want(
            vec![Request {
                path: img.clone(),
                class: 0,
            }],
            0,
        );
        wait(|| e3.idle());
        assert!(e3.meta(&img).is_some());
        // The file changes: invalidate → regenerated, not served stale.
        write_png(&img, 100, 400);
        e2.invalidate(&img);
        e2.want(want, 1);
        wait(|| e2.idle());
        let out = e2.take_ready();
        let Some(Outcome::Ready(r)) = out.last() else {
            panic!("{out:?}")
        };
        assert!(!r.from_cache);
        assert_eq!((r.meta.width, r.meta.height), (100, 400));
        drop((e, e2, e3));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Sums the pixels: enough to tell fresh pixels from the cache's lossy copy.
    #[derive(Debug)]
    struct Sum(u32);

    impl Digester for Sum {
        fn id(&self) -> u32 {
            self.0
        }
        fn class(&self) -> u32 {
            128
        }
        fn digest(&self, px: &Pixels) -> Vec<u8> {
            let s: u64 = px.rgba.iter().map(|&b| b as u64).sum();
            s.to_le_bytes().to_vec()
        }
    }

    #[test]
    fn digests_come_from_fresh_pixels_and_from_the_cache() {
        let dir = crate::cache::test_dir("engine-digest");
        let img = dir.join("pic.png");
        write_png(&img, 300, 200);
        let cache = Arc::new(DiskCache::new(dir.join("cache"), 64 << 20));
        let pool = Arc::new(JobPool::new(2, "thumb-test"));
        let reg = Arc::new(Registry::new());
        let mk = |digester: Option<Arc<dyn Digester>>| {
            Engine::new(
                EngineConfig {
                    digester,
                    ..EngineConfig::default()
                },
                reg.clone(),
                pool.clone(),
                Arc::new(FileOpener),
                Some(cache.clone()),
                Arc::new(|| {}),
            )
        };
        let get = |e: &Engine, class: u32| {
            e.want(
                vec![Request {
                    path: img.clone(),
                    class,
                }],
                1,
            );
            wait(|| e.idle());
            match e.take_ready().pop() {
                Some(Outcome::Ready(r)) => r,
                o => panic!("{o:?}"),
            }
        };
        let e = mk(Some(Arc::new(Sum(7))));
        let r = get(&e, 128);
        assert!(!r.from_cache);
        let fresh = Sum(7).digest(r.pixels.as_ref().unwrap());
        assert_eq!(r.digest.as_deref(), Some(&fresh[..]));
        // Other classes carry none.
        assert!(get(&e, 256).digest.is_none());
        // A new process reads it back with the entry, not from the decoded pixels.
        let r = get(&mk(Some(Arc::new(Sum(7)))), 128);
        assert!(r.from_cache);
        assert_eq!(r.digest.as_deref(), Some(&fresh[..]));
        // Another digest version, or none asked for, sees none; the entry still serves.
        let r = get(&mk(Some(Arc::new(Sum(8)))), 128);
        assert!(r.from_cache && r.digest.is_none());
        let r = get(&mk(None), 128);
        assert!(r.from_cache && r.digest.is_none());
        let key = FileOpener.key(&img).unwrap();
        let flags = EngineConfig::default().flags;
        let (meta, d) = cache.get_digest(&key, 128, flags, 7).unwrap();
        assert_eq!((meta.width, d.bytes), (300, fresh));
        assert!(cache.get_digest(&key, 128, flags, 8).is_none());
        drop(e);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A larger class than the digest's (HiDPI tiles) stores the digest's class with its
    /// digest from the same decode: the duplicate finder then reads it without a decode.
    #[test]
    fn a_larger_class_stores_the_digest_class_too() {
        let dir = crate::cache::test_dir("engine-digest-along");
        let img = dir.join("pic.png");
        write_png(&img, 300, 200);
        let cache = Arc::new(DiskCache::new(dir.join("cache"), 64 << 20));
        let pool = Arc::new(JobPool::new(2, "thumb-test"));
        let reg = Arc::new(Registry::new());
        let opens = Arc::new(CountOpens(AtomicU64::new(0)));
        let e = Engine::new(
            EngineConfig {
                digester: Some(Arc::new(Sum(7))),
                ..EngineConfig::default()
            },
            reg.clone(),
            pool.clone(),
            opens.clone(),
            Some(cache.clone()),
            Arc::new(|| {}),
        );
        let get = |class: u32| {
            e.want(
                vec![Request {
                    path: img.clone(),
                    class,
                }],
                1,
            );
            wait(|| e.idle());
            match e.take_ready().pop() {
                Some(Outcome::Ready(r)) => r,
                o => panic!("{o:?}"),
            }
        };
        let r = get(256);
        assert!(!r.from_cache && r.digest.is_none());
        assert_eq!(opens.0.load(Ordering::Relaxed), 1);
        let key = FileOpener.key(&img).unwrap();
        let flags = EngineConfig::default().flags;
        let (meta, d) = cache.get_digest(&key, 128, flags, 7).unwrap();
        assert_eq!(meta.width, 300);
        // The digest is that of the pixels as made, which the entry then serves (no open).
        let r = get(128);
        assert!(r.from_cache);
        assert_eq!(r.digest.as_deref(), Some(&d.bytes[..]));
        assert_eq!(opens.0.load(Ordering::Relaxed), 1);
        let (px, also) = crate::generate::generate_also(
            &*reg.open_path(&img).unwrap(),
            256,
            Some(128),
            EngineConfig::default().generate,
            &CancelToken::new(),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(px.width, 256);
        assert_eq!(Sum(7).digest(&also.unwrap().pixels), d.bytes);
        drop(e);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Files, counting opens (an open is where a decode or a meta read starts).
    struct CountOpens(AtomicU64);

    impl Opener for CountOpens {
        fn key(&self, path: &Path) -> std::io::Result<ThumbKey> {
            FileOpener.key(path)
        }
        fn open(&self, path: &Path) -> std::io::Result<Opened> {
            self.0.fetch_add(1, Ordering::Relaxed);
            FileOpener.open(path)
        }
    }

    #[test]
    fn older_entries_keep_their_pixels_and_get_fresh_meta_once() {
        let dir = crate::cache::test_dir("engine-stale-meta");
        let img = dir.join("pic.png");
        write_png(&img, 300, 200);
        let cache = Arc::new(DiskCache::new(dir.join("cache"), 64 << 20));
        let pool = Arc::new(JobPool::new(2, "thumb-test"));
        let reg = Arc::new(Registry::new());
        let opens = Arc::new(CountOpens(AtomicU64::new(0)));
        let get = || {
            let e = Engine::new(
                EngineConfig::default(),
                reg.clone(),
                pool.clone(),
                opens.clone(),
                Some(cache.clone()),
                Arc::new(|| {}),
            );
            e.want(
                vec![Request {
                    path: img.clone(),
                    class: 128,
                }],
                1,
            );
            wait(|| e.idle());
            match e.take_ready().pop() {
                Some(Outcome::Ready(r)) => r,
                o => panic!("{o:?}"),
            }
        };
        let first = get();
        assert!(!first.from_cache);
        let made = opens.0.load(Ordering::Relaxed);
        // As an older version left it: the same pixels, meta missing what it reads now.
        let key = FileOpener.key(&img).unwrap();
        let flags = EngineConfig::default().flags;
        let path = cache.path(&key, 128, flags);
        let wrong = Meta {
            format: "PNG".into(),
            width: 1,
            height: 1,
            ..Meta::default()
        };
        cache
            .put(&key, 128, flags, first.pixels.as_ref().unwrap(), &wrong)
            .unwrap();
        let mut b = std::fs::read(&path).unwrap();
        let version = b[4];
        b[4] = version - 1;
        std::fs::write(&path, &b).unwrap();
        // Served from the cache, with the meta read again: one open, no decode.
        let r = get();
        assert!(r.from_cache);
        assert_eq!((r.meta.width, r.meta.height), (300, 200));
        assert_eq!(*r.meta, *first.meta);
        assert_eq!(opens.0.load(Ordering::Relaxed), made + 1);
        // Stored back at the current version, the pixels untouched.
        let after = std::fs::read(&path).unwrap();
        assert_eq!(after[4], version);
        let e = cache.get(&key, 128, flags).unwrap();
        assert!(!e.meta_stale);
        assert_eq!(e.meta, *first.meta);
        // From now on a plain hit: no open at all.
        let r = get();
        assert!(r.from_cache);
        assert_eq!(*r.meta, *first.meta);
        assert_eq!(opens.0.load(Ordering::Relaxed), made + 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn key_memo_reads_once_while_the_file_is_unchanged() {
        let dir = crate::cache::test_dir("engine-keymemo");
        let img = dir.join("pic.bin");
        std::fs::write(&img, vec![7u8; 100_000]).unwrap();
        let memo = KeyMemo::default();
        let k = memo.key(&img).unwrap();
        assert_eq!(k, ThumbKey::of_file(&img).unwrap());
        assert_eq!(memo.len(), 1);
        // Through the engine's side too, then the last use forgets it.
        assert_eq!(Opener::key(&memo, &img).unwrap(), k);
        assert_eq!(memo.take(&img).unwrap(), k);
        assert!(memo.is_empty());
        // Changed since: read again, not the remembered key.
        memo.key(&img).unwrap();
        std::fs::write(&img, vec![8u8; 100_001]).unwrap();
        let k2 = memo.key(&img).unwrap();
        assert_ne!(k2, k);
        assert_eq!(k2, ThumbKey::of_file(&img).unwrap());
        std::fs::write(&img, vec![9u8; 100_002]).unwrap();
        assert_eq!(memo.take(&img).unwrap(), ThumbKey::of_file(&img).unwrap());
        memo.key(&img).unwrap();
        memo.forget(&img);
        assert!(memo.is_empty());
        // Gone.
        memo.key(&img).unwrap();
        std::fs::remove_file(&img).unwrap();
        assert!(memo.key(&img).is_err() && memo.take(&img).is_err());
        memo.clear();
        assert!(memo.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn memory_lru_is_bounded() {
        let pool = Arc::new(JobPool::new(1, "thumb-test"));
        let cfg = EngineConfig {
            memory_budget: 10_000,
            ..EngineConfig::default()
        };
        let e = Engine::new(
            cfg,
            Arc::new(Registry::new()),
            pool,
            Arc::new(FileOpener),
            None,
            Arc::new(|| {}),
        );
        for i in 0..10 {
            let r = Ready {
                path: PathBuf::from(format!("{i}")),
                class: 128,
                pixels: Some(Arc::new(Pixels::new(25, 25, vec![0; 2500]))),
                average: [0; 4],
                meta: Arc::new(Meta::default()),
                sharp: true,
                from_cache: false,
                digest: None,
            };
            let mut st = e.inner.state.lock();
            Inner::remember(&e.inner, &mut st, &r);
        }
        let s = e.stats();
        assert!(s.memory_bytes <= 10_000, "{}", s.memory_bytes);
        assert!(e.mem_get(Path::new("9"), 128).is_some());
        assert!(e.mem_get(Path::new("0"), 128).is_none());
    }
}
