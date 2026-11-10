//! The Library (DESIGN §10): a catalogue of the images in watched locations, kept current in
//! the background, searched with the gallery's query language across every location at once.
//!
//! * **Catalogue:** SQLite in WAL mode ([`schema`]), in the platform data folder. Readers
//!   (search) never wait for the indexer, and the indexer's writes are short transactions.
//! * **Locations, not imports:** folders are indexed where they are ([`Library::add_location`]);
//!   nothing is copied or moved, and nothing is written next to the images.
//! * **Indexer:** one thread at low priority. A walk records each image's identity (path,
//!   size, time stamp, device and inode) folder by folder; a second pass reads the facts
//!   (dimensions, camera, capture date, XMP keywords and marks) through the thumbnail crate's
//!   header-only reader, a batch at a time, and backs off while the user browses
//!   ([`Library::hold`]). File watching keeps it current, with a walk as the fallback; a
//!   moved or renamed file keeps its row (marks, tags) and a location whose volume is gone is
//!   shown offline rather than emptied.
//! * **Search:** [`Library::search`] pushes what SQL can answer into the query (formats,
//!   ratings, labels, sizes, dates, numbers, flags; text through the FTS5 trigram index) and
//!   checks the rest with [`specular_query::Matcher`], so results are exactly the gallery's.

// File identity on Windows (volume serial number and file index, scan.rs).
#![cfg_attr(windows, feature(windows_by_handle))]

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension};

pub use specular_query::Fold;
use specular_thumbs::{DiskCache, Meta};

pub mod ai;
pub mod albums;
pub mod edits;
mod facts;
mod hashes;
mod indexer;
pub mod local;
mod matches;
pub mod places;
mod scan;
pub mod schema;
pub mod search;
pub mod skip;
mod snapshot;
mod store;

/// Work counted process-wide, for tests that bound the work done per file: counts hold on
/// a loaded machine, where times do not (`tests/scale.rs`).
#[doc(hidden)]
pub mod work_counts {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Full-text rows deleted and written again.
    pub static FTS_REWRITES: AtomicU64 = AtomicU64::new(0);
    /// SQLite steps spent finding the next files for the facts and signature passes.
    pub static TODO_STEPS: AtomicU64 = AtomicU64::new(0);
    /// Duplicate and similar-shot matching: signatures taken in, candidates looked at, and
    /// members of groups formed again.
    pub static MATCH_OPS: AtomicU64 = AtomicU64::new(0);

    /// Adds the steps `st` took since the last call (and starts it counting again).
    pub(crate) fn todo_steps(st: &rusqlite::Statement<'_>) {
        let n = st.reset_status(rusqlite::StatementStatus::VmStep);
        add_todo_steps(n.max(0) as u64);
    }

    pub(crate) fn add_todo_steps(n: u64) {
        TODO_STEPS.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn match_ops(n: u64) {
        MATCH_OPS.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn fts_rewrite() {
        FTS_REWRITES.fetch_add(1, Ordering::Relaxed);
    }
}

pub use facts::{PLACE_MAX_KM, place_of};
pub use hashes::{DecodeSigner, Signer};
pub use search::{Found, Row, Search, Sort};
pub use skip::Skip;
pub use snapshot::Snapshot;

/// How often a location that cannot be watched (or not all of it) is walked again, by
/// default ([`Config::rescan_every`]).
pub const RESCAN_EVERY: Duration = Duration::from_secs(15 * 60);

/// The catalogue's file name in the data folder.
pub const FILE_NAME: &str = "library.sqlite3";

/// The catalogue for this process: `$SPECULAR_LIBRARY_DB` when set (tests, screenshots,
/// benchmarks), else [`FILE_NAME`] in `data_dir` (`specular_settings::Locations::data_dir`).
pub fn default_path(data_dir: &Path) -> PathBuf {
    match std::env::var_os("SPECULAR_LIBRARY_DB").filter(|v| !v.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => data_dir.join(FILE_NAME),
    }
}

/// Culling marks, as the gallery keeps them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mark {
    /// 0–5 stars.
    pub rating: u8,
    /// 0 none, 1–4 red, yellow, green, blue.
    pub label: u8,
    /// 1 pick, -1 reject, 0 neither.
    pub flag: i8,
}

/// What the facts pass learns about one image.
#[derive(Clone, Debug, Default)]
pub struct Probed {
    pub meta: Meta,
    /// Keywords in the file's own XMP (`dc:subject`).
    pub keywords: Vec<String>,
    /// Words the file holds for text search (a song's lyrics), when it has any.
    pub text: Option<String>,
}

/// Reads an image's facts without decoding it.
pub trait Prober: Send + Sync {
    /// None when the file is not an image the decoders read.
    fn probe(&self, path: &Path) -> Option<Probed>;
    /// An astronomy subframe's star metrics (this decodes its pixels); None for anything
    /// else.
    fn measure(&self, _path: &Path) -> Option<specular_decode::astro::FrameMetrics> {
        None
    }
}

/// The decoders' header reader ([`specular_thumbs::generate::meta_of`]) and the keywords of
/// the file's embedded XMP packet.
pub struct DecodeProber {
    pub registry: Arc<specular_decode::Registry>,
}

/// How far into a file an embedded XMP packet is looked for (JPEG APP1, PNG iTXt, TIFF and
/// WebP put it near the start).
const XMP_WINDOW: usize = 256 << 10;

impl Prober for DecodeProber {
    fn probe(&self, path: &Path) -> Option<Probed> {
        let s = self.registry.open_path(path).ok()?;
        let meta = specular_thumbs::generate::meta_of(s.as_ref());
        let bytes = s.source().bytes();
        let keywords = embedded_keywords(&bytes[..bytes.len().min(XMP_WINDOW)]);
        let text = s.search_text();
        Some(Probed {
            meta,
            keywords,
            text,
        })
    }

    fn measure(&self, path: &Path) -> Option<specular_decode::astro::FrameMetrics> {
        let s = self.registry.open_path(path).ok()?;
        specular_decode::astro::measure(s.as_ref(), &specular_core::CancelToken::new())?.ok()
    }
}

/// The keywords embedded in a file starting with `head`: its XMP's (hierarchy kept), else a
/// JPEG's IPTC Keywords.
pub fn embedded_keywords(head: &[u8]) -> Vec<String> {
    specular_metadata::keywords::from_file_head(head)
}

/// What the library is set up with.
#[derive(Clone)]
pub struct Config {
    /// The catalogue file ([`default_path`]).
    pub path: PathBuf,
    /// Case and accent folding of the full-text index and of queries (the app's locale-aware
    /// one); the index is rebuilt when `fold_id` changes (in the background: until it is
    /// through, searches check every file's text without it).
    pub fold: Fold,
    pub fold_id: &'static str,
    /// Whether a file is an image to index (by its name; the decoders' extensions).
    pub is_image: Arc<dyn Fn(&Path) -> bool + Send + Sync>,
    pub prober: Arc<dyn Prober>,
    /// The thumbnail cache, to note which sizes it already holds for each file.
    pub cache: Option<Arc<DiskCache>>,
    /// The thumbnail engine's cache-key flags ([`specular_thumbs::engine::EngineConfig::flags`]).
    pub cache_flags: u32,
    /// Threads reading facts at once.
    pub threads: usize,
    /// Files per facts transaction.
    pub batch: usize,
    /// Called first on every thread the library starts (the app lowers its priority).
    pub on_thread_start: Option<fn()>,
    /// Called (from the indexer's thread) when progress or the catalogue changed.
    pub wake: Arc<dyn Fn() + Send + Sync>,
    /// Watch locations for changes (else rely on `rescan_every`).
    pub watch: bool,
    /// Walk each location again this often when it cannot be watched.
    pub rescan_every: Duration,
    /// Look for offline locations coming back this often.
    pub offline_every: Duration,
    /// After the user's last input, wait this long before reading facts again (see
    /// [`Library::hold`]).
    pub hold: Duration,
    /// Makes the signatures for `dupes:` and `similar:` (None: no duplicate groups).
    pub signer: Option<Arc<dyn Signer>>,
    /// No heartbeat for this long while there is work: [`Halt::Stalled`] ([`STALL_AFTER`]).
    pub stall_after: Duration,
    /// Folders the walk passes over ([`Library::set_skip`] changes it).
    pub skip: Skip,
    /// Reads Develop sidecars into the catalogue on a walk (None: not read).
    pub edit_import: Option<EditImport>,
    /// Folders (and those below) where other apps' sidecars have the last word on ratings,
    /// labels, flags and keywords ([`Library::set_prefer_foreign`]); elsewhere they only fill
    /// in what the user has not set.
    pub prefer_foreign: Vec<PathBuf>,
}

/// How the walk reads Develop sidecars the catalogue has no edit for: their names end in
/// `suffix` (`IMG.CR2.specular.xmp`, `IMG.CR2.v2.specular.xmp`), and `read` gives a photo's
/// version as the catalogue stores it (None: unreadable).
#[derive(Clone)]
pub struct EditImport {
    pub suffix: &'static str,
    pub read: ReadEdit,
}

/// Reads version `u32` of the photo at the path from its Develop sidecar.
pub type ReadEdit = Arc<dyn Fn(&Path, u32) -> Option<edits::StoredEdit> + Send + Sync>;

impl Config {
    pub fn new(path: PathBuf) -> Config {
        let registry = Arc::new(specular_decode::Registry::new());
        let r = registry.clone();
        Config {
            path,
            fold: specular_query::ascii_fold,
            fold_id: "ascii",
            is_image: Arc::new(move |p: &Path| r.supports_extension(p)),
            prober: Arc::new(DecodeProber {
                registry: registry.clone(),
            }),
            cache: None,
            cache_flags: 0,
            threads: 2,
            batch: 256,
            on_thread_start: None,
            wake: Arc::new(|| {}),
            watch: true,
            rescan_every: RESCAN_EVERY,
            offline_every: Duration::from_secs(30),
            hold: Duration::from_millis(400),
            stall_after: STALL_AFTER,
            signer: Some(Arc::new(DecodeSigner { registry })),
            skip: Skip::default(),
            edit_import: None,
            prefer_foreign: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Sql(rusqlite::Error),
    Io(std::io::Error),
    /// Written by a newer Specular: searched, never written.
    Newer {
        found: u32,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Sql(e) => write!(f, "catalogue: {e}"),
            Error::Io(e) => write!(f, "catalogue: {e}"),
            Error::Newer { found } => write!(
                f,
                "catalogue schema {found} is newer than this build's {}",
                schema::VERSION
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Sql(e) => Some(e),
            Error::Io(e) => Some(e),
            Error::Newer { .. } => None,
        }
    }
}

/// What kind of failure an [`Error`] is, for wording it to the user without the SQL text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorClass {
    /// Another program (or another Specular) holds the catalogue.
    Busy,
    /// The catalogue file is damaged or is not a catalogue.
    Corrupt,
    /// The disk is full.
    Full,
    /// No permission to read or write the catalogue.
    Permission,
    /// The catalogue's disk is read-only.
    ReadOnly,
    /// The catalogue could not be opened (its folder is missing, say).
    CannotOpen,
    /// Out of memory.
    Memory,
    /// Written by a newer Specular.
    Newer,
    /// An I/O error: see [`Error::io`].
    Io,
    /// Anything else (a bug, most likely).
    Other,
}

impl Error {
    /// The kind of failure.
    pub fn class(&self) -> ErrorClass {
        use rusqlite::ErrorCode as C;
        match self {
            Error::Io(_) => ErrorClass::Io,
            Error::Newer { .. } => ErrorClass::Newer,
            Error::Sql(e) => match e.sqlite_error_code() {
                Some(C::DatabaseBusy | C::DatabaseLocked) => ErrorClass::Busy,
                Some(C::DatabaseCorrupt | C::NotADatabase) => ErrorClass::Corrupt,
                Some(C::DiskFull) => ErrorClass::Full,
                Some(C::PermissionDenied | C::AuthorizationForStatementDenied) => {
                    ErrorClass::Permission
                }
                Some(C::ReadOnly) => ErrorClass::ReadOnly,
                Some(C::CannotOpen | C::SystemIoFailure) => ErrorClass::CannotOpen,
                Some(C::OutOfMemory) => ErrorClass::Memory,
                _ => ErrorClass::Other,
            },
        }
    }

    /// The I/O error, when it is one.
    pub fn io(&self) -> Option<&std::io::Error> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Error {
        Error::Sql(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e)
    }
}

/// A watched location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub id: i64,
    pub path: PathBuf,
    /// The root is missing or another volume is mounted there: its files stay catalogued.
    pub offline: bool,
    /// Images catalogued in it.
    pub files: u64,
    /// Of those, how many still wait for their facts.
    pub pending: u64,
    /// When the last walk of it finished (seconds since the epoch).
    pub last_scan: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    /// Walking folders, recording files.
    Walking,
    /// Reading facts of recorded files.
    Reading,
}

/// What the indexer is doing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub phase: Phase,
    /// The location being walked.
    pub location: Option<PathBuf>,
    /// Images seen by the current walk.
    pub walked: u64,
    /// Images waiting for their facts (when the reading began, plus any added since).
    pub to_read: u64,
    /// Facts read since the reading began.
    pub read: u64,
    /// Backing off while the user browses.
    pub held: bool,
    /// The indexing was stopped ([`Library::cancel`]): what was catalogued stays, files not
    /// read yet stay waiting ([`Location::pending`]) until [`Library::rescan`] or
    /// [`Library::add_location`] (or the next start) takes it up again.
    pub stopped: bool,
    /// Bumped by every committed change to what a search would find.
    pub changes: u64,
    /// The indexer stopped by itself, or stopped moving ([`Halt`]); [`Library::retry`]
    /// starts it again. What was catalogued stays searchable meanwhile.
    pub halt: Option<Halt>,
    /// The folder watches ran out: some folders are not watched, and changes in them are
    /// found by a walk every [`Config::rescan_every`] instead.
    pub out_of_watches: bool,
}

/// Why the indexer is not going on by itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Halt {
    /// A database error or a bug (`class` [`ErrorClass::Other`]) stopped it; `detail` is for
    /// the log and the diagnostics bundle, not for the user.
    Failed {
        class: ErrorClass,
        io: Option<std::io::ErrorKind>,
        detail: String,
    },
    /// It has work but showed no sign of life for `secs` (a file or drive that does not
    /// answer). It clears by itself when the indexer moves again.
    Stalled { secs: u64 },
}

/// No heartbeat for this long while there is work: [`Halt::Stalled`].
pub const STALL_AFTER: Duration = Duration::from_secs(120);

impl Progress {
    pub fn busy(&self) -> bool {
        self.phase != Phase::Idle && self.halt.is_none()
    }
}

/// Requests to the indexer thread.
pub(crate) enum Cmd {
    Add(PathBuf),
    Remove(i64),
    Rescan(Option<i64>),
    Marks(Vec<(PathBuf, Mark)>),
    /// Keywords written to files' sidecars by the app.
    Keywords(Vec<(PathBuf, Vec<String>)>),
    /// IPTC fields written by the app (`specular_metadata::edit::Record::to_lines`).
    Iptc(Vec<(PathBuf, String)>),
    /// A Develop edit to store (files outside the catalogue are skipped).
    Edit(PathBuf, u32, Box<edits::StoredEdit>),
    /// A Develop version to forget.
    DeleteEdit(PathBuf, u32),
    /// [`Shared::skip`] changed: every location is walked again (unless stopped).
    Skip,
    /// [`Shared::prefer_foreign`] changed for these folders: their files with other apps'
    /// sidecars are read again.
    PreferForeign(Vec<PathBuf>),
    /// Stop walking and reading until the next rescan or added location.
    Cancel,
    /// A watcher has events (only wakes the loop).
    Poke,
    /// Start again after a [`Halt::Failed`].
    Retry,
    Stop,
}

/// Another app's sidecar of a catalogued file, as last read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignSidecar {
    /// Relative to the file's folder.
    pub name: String,
    pub app: Option<specular_metadata::sidecars::App>,
    /// The darktable duplicate it describes; 0 for the photo itself.
    pub duplicate: u32,
    pub values: specular_metadata::sidecars::Values,
    pub version_name: Option<String>,
}

/// What the indexer shares with the handle.
pub(crate) struct Shared {
    pub progress: Mutex<Progress>,
    pub hold: Mutex<Hold>,
    /// Locations whose removal was asked for and is not done yet: they are left out of
    /// [`Shared::locations`] at once, and long work returns at its next folder or file so
    /// the indexer can delete them.
    pub removing: Mutex<HashSet<i64>>,
    pub stop: AtomicBool,
    /// [`Library::cancel`] was called and the indexer has not taken it in yet: walks and
    /// reads return at their next folder or file.
    pub cancel: AtomicBool,
    /// Commands sent and not yet taken: long work returns to the loop when one is waiting.
    pub queued: AtomicU64,
    /// The indexer is waiting with nothing to do.
    pub idle: AtomicBool,
    pub locations: Mutex<Vec<Location>>,
    /// Folders the walk passes over.
    pub skip: Mutex<Arc<Skip>>,
    /// Folders (and those below) where other apps' sidecars have the last word.
    pub prefer_foreign: Mutex<Arc<Vec<PathBuf>>>,
    /// The heartbeat: ms since `born` when the indexer (or one of its readers) last showed
    /// it is moving ([`Shared::beat`]).
    pub born: Instant,
    pub beat: AtomicU64,
}

/// The user's browsing ([`Library::hold`]): until when, and since when without a break.
#[derive(Default)]
pub(crate) struct Hold {
    until: Option<Instant>,
    began: Option<Instant>,
}

/// While the user browses, background work waits this long…
const HOLD_WAIT: Duration = Duration::from_millis(300);
/// …then works this long, and so on: browsing slows the indexing to a quarter, it never
/// stops it (a hold renewed for minutes once kept a walk from moving at all).
const HOLD_WORK: Duration = Duration::from_millis(100);

impl Shared {
    /// The indexer is moving (a turn of its loop, a file read, a folder walked).
    pub fn beat(&self) {
        let ms = self.born.elapsed().as_millis() as u64;
        self.beat.fetch_max(ms, Ordering::Relaxed);
    }

    /// Time since the last [`Shared::beat`].
    pub fn since_beat(&self) -> Duration {
        let ms = self.born.elapsed().as_millis() as u64;
        Duration::from_millis(ms.saturating_sub(self.beat.load(Ordering::Relaxed)))
    }

    /// Whether background work should wait now: the user browses, and this is a waiting
    /// part of the cycle ([`HOLD_WAIT`], [`HOLD_WORK`]).
    pub fn held(&self) -> bool {
        let h = self.hold.lock();
        let now = Instant::now();
        match (h.until, h.began) {
            (Some(until), Some(began)) if now < until => {
                let cycle = (HOLD_WAIT + HOLD_WORK).as_nanos();
                now.duration_since(began).as_nanos() % cycle < HOLD_WAIT.as_nanos()
            }
            _ => false,
        }
    }

    /// Whether the user browsed within [`Config::hold`].
    pub fn browsing(&self) -> bool {
        self.hold.lock().until.is_some_and(|t| Instant::now() < t)
    }

    fn hold(&self, d: Duration) {
        let mut h = self.hold.lock();
        let now = Instant::now();
        if h.until.is_none_or(|t| now >= t) {
            h.began = Some(now);
        }
        h.until = Some(now + d);
    }

    /// A location's removal waits: walks, facts and signatures return at their next folder
    /// or file.
    pub fn removal_waits(&self) -> bool {
        !self.removing.lock().is_empty()
    }

    /// The locations as the indexer read them, without those being removed.
    pub fn set_locations(&self, mut v: Vec<Location>) {
        let r = self.removing.lock();
        v.retain(|l| !r.contains(&l.id));
        *self.locations.lock() = v;
    }

    /// Long work returns at its next folder or file: the library closes or the indexing
    /// was stopped.
    pub fn halting(&self) -> bool {
        self.stop.load(Ordering::Relaxed) || self.cancel.load(Ordering::Relaxed)
    }
}

/// The open catalogue and its indexer.
pub struct Library {
    shared: Arc<Shared>,
    tx: mpsc::Sender<Cmd>,
    thread: Option<std::thread::JoinHandle<()>>,
    reader: Mutex<Connection>,
    /// The reader's search tables, kept between searches.
    side: search::SideCache,
    fold: Fold,
    fold_id: &'static str,
    hold: Duration,
    stall_after: Duration,
    read_only: bool,
    path: PathBuf,
}

impl Library {
    /// Opens (creating or upgrading) the catalogue and starts the indexer, which walks every
    /// location once to catch up on what changed while Specular was not running.
    ///
    /// Never on the UI thread: migrations of a big catalogue take seconds, and any step may
    /// wait out another writer's transaction (debug builds panic there).
    pub fn open(cfg: Config) -> Result<Library, Error> {
        specular_platform::assert_not_ui_thread("Library::open");
        if let Some(dir) = cfg.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut writer = Connection::open(&cfg.path)?;
        schema::configure(&writer)?;
        let found = schema::migrate_checked(&mut writer)?;
        let read_only = found > schema::VERSION;
        let reader = Connection::open(&cfg.path)?;
        schema::configure(&reader)?;
        schema::configure_reader(&reader)?;
        let shared = Arc::new(Shared {
            progress: Mutex::new(Progress::default()),
            hold: Mutex::new(Hold::default()),
            removing: Mutex::new(HashSet::new()),
            stop: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            queued: AtomicU64::new(0),
            idle: AtomicBool::new(false),
            locations: Mutex::new(store::locations(&reader)?),
            skip: Mutex::new(Arc::new(cfg.skip.clone())),
            prefer_foreign: Mutex::new(Arc::new(cfg.prefer_foreign.clone())),
            born: Instant::now(),
            beat: AtomicU64::new(0),
        });
        let (tx, rx) = mpsc::channel();
        let (fold, fold_id, hold, path) = (cfg.fold, cfg.fold_id, cfg.hold, cfg.path.clone());
        let stall_after = cfg.stall_after;
        let thread = if read_only {
            specular_log::warn!(target: specular_log::FOLDER, "library: {}", Error::Newer { found });
            None
        } else {
            let sh = shared.clone();
            let poke = tx.clone();
            Some(
                specular_thread::builder("lib-index")
                    .spawn(move || indexer::run(writer, cfg, sh, rx, poke))?,
            )
        };
        Ok(Library {
            shared,
            tx,
            thread,
            reader: Mutex::new(reader),
            side: search::SideCache::default(),
            fold,
            fold_id,
            hold,
            stall_after,
            read_only,
            path,
        })
    }

    /// Written by a newer Specular: search works, nothing is indexed or written.
    pub fn read_only(&self) -> bool {
        self.read_only
    }

    fn send(&self, c: Cmd) {
        self.shared.queued.fetch_add(1, Ordering::AcqRel);
        let _ = self.tx.send(c);
    }

    /// Starts watching `path` (indexed in the background). A folder inside a location is
    /// already watched; a folder around locations takes them over.
    pub fn add_location(&self, path: &Path) {
        self.send(Cmd::Add(path.to_path_buf()));
    }

    /// Forgets a location and everything catalogued in it (the files are not touched). It
    /// leaves [`Library::locations`] at once; a walk or reading under way returns at its next
    /// folder or file, and the indexer deletes its rows.
    pub fn remove_location(&self, id: i64) {
        self.shared.removing.lock().insert(id);
        self.shared.locations.lock().retain(|l| l.id != id);
        self.send(Cmd::Remove(id));
    }

    /// Stops the indexing under way: a walk returns at its next folder and the reading of
    /// facts and signatures at its next file. What was catalogued stays (searchable, files
    /// without their facts yet counted in [`Location::pending`]); nothing more is walked
    /// or read until [`Library::rescan`] or [`Library::add_location`], or the next start.
    /// Folder watching goes on, so the catalogue still follows changes.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
        self.send(Cmd::Cancel);
    }

    /// Changes the folders the walk passes over: every location is walked again (when the
    /// indexing was stopped, at the next [`Library::rescan`]), so folders now skipped leave
    /// the catalogue and folders no longer skipped come in.
    pub fn set_skip(&self, skip: Skip) {
        {
            let mut cur = self.shared.skip.lock();
            if **cur == skip {
                return;
            }
            *cur = Arc::new(skip);
        }
        self.send(Cmd::Skip);
    }

    /// The folders the walk passes over.
    pub fn skip(&self) -> Arc<Skip> {
        self.shared.skip.lock().clone()
    }

    /// Sets the folders (and those below them) where other apps' sidecars have the last word
    /// ("prefer the other app's values"). Their files with such sidecars are read again.
    pub fn set_prefer_foreign(&self, mut folders: Vec<PathBuf>) {
        folders.sort();
        folders.dedup();
        let changed: Vec<PathBuf> = {
            let mut cur = self.shared.prefer_foreign.lock();
            if **cur == folders {
                return;
            }
            let changed = cur
                .iter()
                .filter(|f| !folders.contains(f))
                .chain(folders.iter().filter(|f| !cur.contains(f)))
                .cloned()
                .collect();
            *cur = Arc::new(folders);
            changed
        };
        if !self.read_only {
            self.send(Cmd::PreferForeign(changed));
        }
    }

    /// The folders where other apps' sidecars have the last word.
    pub fn prefer_foreign(&self) -> Arc<Vec<PathBuf>> {
        self.shared.prefer_foreign.lock().clone()
    }

    /// Walks a location again (all of them with `None`); after [`Library::cancel`], also
    /// takes the reading up again.
    pub fn rescan(&self, id: Option<i64>) {
        self.send(Cmd::Rescan(id));
    }

    /// Mirrors marks set in the gallery into the catalogue (files it does not hold are
    /// skipped).
    pub fn set_marks(&self, marks: Vec<(PathBuf, Mark)>) {
        if !marks.is_empty() {
            self.send(Cmd::Marks(marks));
        }
    }

    /// Mirrors keywords written to sidecars into the catalogue (files it does not hold are
    /// skipped).
    pub fn set_keywords(&self, keywords: Vec<(PathBuf, Vec<String>)>) {
        if !keywords.is_empty() && !self.read_only {
            self.send(Cmd::Keywords(keywords));
        }
    }

    /// Mirrors IPTC fields written by the app (each file's fields after the write, as
    /// `specular_metadata::edit::Record::to_lines` gives them) into the catalogue.
    pub fn set_iptc(&self, fields: Vec<(PathBuf, String)>) {
        if !fields.is_empty() && !self.read_only {
            self.send(Cmd::Iptc(fields));
        }
    }

    /// Every keyword the catalogue knows, for suggestions. Off the UI thread.
    pub fn tag_names(&self) -> Result<Vec<String>, Error> {
        specular_platform::assert_not_ui_thread("Library::tag_names");
        let conn = self.reader.lock();
        Ok(store::tag_names(&conn)?)
    }

    /// The catalogued files with keyword `path` or one below it (for a keyword rename).
    pub fn files_with_tag(&self, path: &str) -> Result<Vec<PathBuf>, Error> {
        specular_platform::assert_not_ui_thread("Library::files_with_tag");
        let conn = self.reader.lock();
        Ok(store::files_with_tag(&conn, path)?)
    }

    /// Stores version `version` of `path`'s Develop edit (on the indexer; skipped when the
    /// file is not catalogued).
    pub fn save_edit(&self, path: PathBuf, version: u32, edit: edits::StoredEdit) {
        if !self.read_only {
            self.send(Cmd::Edit(path, version, Box::new(edit)));
        }
    }

    /// Forgets version `version` of `path`'s Develop edit (on the indexer). The image and its
    /// other versions stay.
    pub fn delete_edit(&self, path: PathBuf, version: u32) {
        if !self.read_only {
            self.send(Cmd::DeleteEdit(path, version));
        }
    }

    /// The Develop versions of `path` the catalogue holds, with their names (empty: none).
    /// Off the UI thread.
    pub fn edit_versions(&self, path: &Path) -> Result<Vec<(u32, String)>, Error> {
        specular_platform::assert_not_ui_thread("Library::edit_versions");
        let conn = self.reader.lock();
        match edits::file_id(&conn, path)? {
            Some(id) => Ok(edits::named_versions(&conn, id)?),
            None => Ok(Vec::new()),
        }
    }

    /// The photos in folder `dir` the catalogue holds an edit for (any version). Off the UI
    /// thread.
    pub fn edited_in(&self, dir: &Path) -> Result<Vec<PathBuf>, Error> {
        specular_platform::assert_not_ui_thread("Library::edited_in");
        let Some(d) = dir.to_str() else {
            return Ok(Vec::new());
        };
        let conn = self.reader.lock();
        Ok(edits::edited_in(&conn, d)?
            .into_iter()
            .map(|n| dir.join(n))
            .collect())
    }

    /// How many Develop versions each of `paths` has, for those with more than one (the
    /// gallery's badge). Off the UI thread.
    pub fn version_counts(&self, paths: &[PathBuf]) -> Result<Vec<(PathBuf, u32)>, Error> {
        specular_platform::assert_not_ui_thread("Library::version_counts");
        let conn = self.reader.lock();
        let mut ids = Vec::new();
        for p in paths {
            if let Some(id) = edits::file_id(&conn, p)? {
                ids.push((id, p));
            }
        }
        let only: Vec<i64> = ids.iter().map(|x| x.0).collect();
        let mut counts: std::collections::BTreeMap<i64, u32> =
            edits::version_counts(&conn, &only)?.into_iter().collect();
        // darktable's duplicates count as versions of the photo.
        for (id, d) in edits::duplicate_counts(&conn, &only)? {
            let n = counts.entry(id).or_insert(1);
            *n = (*n).max(1) + d;
        }
        Ok(counts
            .into_iter()
            .filter(|&(_, n)| n > 1)
            .filter_map(|(id, n)| ids.iter().find(|x| x.0 == id).map(|x| (x.1.clone(), n)))
            .collect())
    }

    /// What other apps' sidecars of `path` said when last read (its darktable duplicates
    /// too), weightiest first. Off the UI thread.
    pub fn foreign_sidecars(&self, path: &Path) -> Result<Vec<ForeignSidecar>, Error> {
        specular_platform::assert_not_ui_thread("Library::foreign_sidecars");
        let conn = self.reader.lock();
        match edits::file_id(&conn, path)? {
            Some(id) => Ok(store::foreign_sidecars(&conn, id)?),
            None => Ok(Vec::new()),
        }
    }

    /// Version `version` of `path`'s Develop edit, when the catalogue has one. Off the UI
    /// thread.
    pub fn load_edit(&self, path: &Path, version: u32) -> Result<Option<edits::StoredEdit>, Error> {
        specular_platform::assert_not_ui_thread("Library::load_edit");
        let conn = self.reader.lock();
        match edits::file_id(&conn, path)? {
            Some(id) => Ok(edits::load(&conn, id, version)?),
            None => Ok(None),
        }
    }

    /// Every album ([`albums`]), by name. Off the UI thread.
    pub fn albums(&self) -> Result<Vec<albums::Album>, Error> {
        specular_platform::assert_not_ui_thread("Library::albums");
        Ok(albums::list(&self.reader.lock())?)
    }

    /// Album changes are small and asked for by the user: they run on the search connection,
    /// so their result (a new album's id) is known at once, not after the indexer's queue.
    fn album_write<T>(
        &self,
        what: &'static str,
        f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>,
    ) -> Result<T, Error> {
        specular_platform::assert_not_ui_thread(what);
        if self.read_only {
            return Err(Error::Newer {
                found: schema::VERSION + 1,
            });
        }
        let mut conn = self.reader.lock();
        Ok(f(&mut conn)?)
    }

    /// Makes an album (see [`albums::create`]); its id.
    pub fn album_create(
        &self,
        name: &str,
        kind: albums::Kind,
        rules: &specular_query::rules::Rules,
    ) -> Result<i64, Error> {
        self.album_write("Library::album_create", |c| {
            albums::create(c, name, kind, rules)
        })
    }

    pub fn album_rename(&self, id: i64, name: &str) -> Result<(), Error> {
        self.album_write("Library::album_rename", |c| albums::rename(c, id, name))
    }

    /// Renames a group of albums; how many moved.
    pub fn album_rename_group(&self, group: &str, to: &str) -> Result<usize, Error> {
        self.album_write("Library::album_rename_group", |c| {
            albums::rename_group(c, group, to)
        })
    }

    pub fn album_set_rules(
        &self,
        id: i64,
        rules: &specular_query::rules::Rules,
    ) -> Result<(), Error> {
        self.album_write("Library::album_set_rules", |c| {
            albums::set_rules(c, id, rules)
        })
    }

    /// Deletes albums; the files stay.
    pub fn album_delete(&self, ids: &[i64]) -> Result<(), Error> {
        self.album_write("Library::album_delete", |c| albums::delete(c, ids))
    }

    /// Puts files in a manual album: (how many went in, the paths the catalogue does not
    /// hold).
    pub fn album_add(&self, id: i64, paths: &[PathBuf]) -> Result<(usize, Vec<PathBuf>), Error> {
        self.album_write("Library::album_add", |c| {
            let (ids, missing) = albums::ids_of(c, paths)?;
            Ok((albums::add(c, id, &ids)?, missing))
        })
    }

    /// Takes files out of a manual album; how many were in it.
    pub fn album_remove(&self, id: i64, paths: &[PathBuf]) -> Result<usize, Error> {
        self.album_write("Library::album_remove", |c| {
            let (ids, _) = albums::ids_of(c, paths)?;
            albums::remove(c, id, &ids)
        })
    }

    /// The user is browsing: walking, reading facts and signatures back off until
    /// [`Config::hold`] after the last call, so the grid keeps the disk and the cores. They
    /// slow down, never stop: a quarter of the time is theirs however long the browsing.
    /// Call it on the user's input (scrolls, flings, keys), not on redraws or hovers.
    pub fn hold(&self) {
        self.shared.hold(self.hold);
    }

    /// Whether the user browsed within [`Config::hold`] (background work such as the AI
    /// pack's indexing backs off too).
    pub fn held(&self) -> bool {
        self.shared.browsing()
    }

    /// The catalogue file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn progress(&self) -> Progress {
        let mut p = self.shared.progress.lock().clone();
        p.held = p.phase == Phase::Reading && self.shared.browsing();
        if p.halt.is_none() && !self.shared.stop.load(Ordering::Relaxed) {
            if self.thread.as_ref().is_some_and(|t| t.is_finished()) {
                // Ended without saying why (it always says, so this is a bug).
                p.halt = Some(Halt::Failed {
                    class: ErrorClass::Other,
                    io: None,
                    detail: "the indexer thread ended".into(),
                });
            } else if self.thread.is_some() && !self.shared.idle.load(Ordering::Acquire) {
                let since = self.shared.since_beat();
                if since >= self.stall_after {
                    p.halt = Some(Halt::Stalled {
                        secs: since.as_secs(),
                    });
                }
            }
        }
        p
    }

    /// Starts the indexer again after it stopped by itself ([`Progress::halt`]): a failed
    /// one is started afresh (commands sent meanwhile were kept and follow); a stalled one
    /// gets another [`STALL_AFTER`] and walks every location again once it moves.
    pub fn retry(&self) {
        self.shared.beat();
        self.send(Cmd::Retry);
    }

    /// The locations as last known to the indexer (cheap: no query).
    pub fn locations(&self) -> Vec<Location> {
        self.shared.locations.lock().clone()
    }

    /// Runs a search on the calling thread (never the UI thread: a search over 100 000 files
    /// takes tens of milliseconds, and debug builds panic there).
    pub fn search(&self, req: &Search) -> Result<Found, Error> {
        let mut rows = Vec::new();
        let mut found = self.search_each(req, &mut |r| rows.push(r))?;
        found.rows = rows;
        Ok(found)
    }

    /// [`Self::search`], handing each row to `emit` in order instead of collecting them
    /// (`rows` stays empty; see [`search::run_each`]). On the calling thread, never the UI
    /// thread.
    pub fn search_each(&self, req: &Search, emit: &mut dyn FnMut(Row)) -> Result<Found, Error> {
        specular_platform::assert_not_ui_thread("Library::search");
        let conn = self.reader.lock();
        // A location being removed is gone for the user already (its rows go a moment later).
        let ids: Vec<i64> = self.shared.removing.lock().iter().copied().collect();
        let mut gone = Vec::new();
        for id in ids {
            let p: Option<String> = conn
                .query_row("SELECT path FROM folders WHERE id = ?1", [id], |r| r.get(0))
                .optional()?;
            gone.extend(p.map(PathBuf::from));
        }
        let mut dropped = 0;
        let side = Some(&self.side);
        let mut found =
            search::run_each_cached(&conn, req, self.fold, self.fold_id, side, &mut |r| {
                if gone.iter().any(|g| r.path.starts_with(g)) {
                    dropped += 1;
                } else {
                    emit(r);
                }
            })?;
        found.total = found.total.saturating_sub(dropped);
        Ok(found)
    }

    /// The AI pack's Search embeddings of `model` stored after rowid `after` (see
    /// [`ai::embeddings_since`]); `None` while Search has no tables. On the reader, off the
    /// UI thread.
    #[allow(clippy::type_complexity)]
    pub fn embeddings_since(
        &self,
        model: &str,
        dims: usize,
        after: i64,
    ) -> Result<Option<(Vec<(i64, Vec<u16>)>, i64)>, Error> {
        specular_platform::assert_not_ui_thread("Library::embeddings_since");
        let conn = self.reader.lock();
        if !ai::has(&conn, "search")? {
            return Ok(None);
        }
        Ok(Some(ai::embeddings_since(&conn, model, dims, after)?))
    }

    /// Files whose text found by the AI pack's Text feature holds every word (see
    /// [`ai::find_text`]); `None` while Text has no tables. On the reader, off the UI thread.
    pub fn find_text(
        &self,
        words: &[String],
        fold: &dyn Fn(&str) -> String,
        limit: usize,
    ) -> Result<Option<Vec<(i64, f32)>>, Error> {
        specular_platform::assert_not_ui_thread("Library::find_text");
        let conn = self.reader.lock();
        if !ai::has(&conn, "text")? {
            return Ok(None);
        }
        Ok(Some(ai::find_text(&conn, words, fold, limit)?))
    }

    /// Images catalogued, and how many of them have their facts (off the UI thread).
    pub fn counts(&self) -> Result<(u64, u64), Error> {
        specular_platform::assert_not_ui_thread("Library::counts");
        let conn = self.reader.lock();
        Ok(conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(state <> 0), 0) FROM files",
            [],
            |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64)),
        )?)
    }

    /// Waits until the indexer has nothing left to do (tests, benchmarks). False on timeout.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let t = Instant::now();
        loop {
            let idle = self.read_only || self.shared.idle.load(Ordering::Acquire);
            if idle && self.shared.queued.load(Ordering::Acquire) == 0 {
                return true;
            }
            if t.elapsed() > timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Cmd::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Seconds since the epoch.
pub(crate) fn now_s() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Pending work of the indexer, oldest first.
pub(crate) type Queue = VecDeque<indexer::Job>;

#[cfg(test)]
mod tests;
