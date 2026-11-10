//! The Library (DESIGN.md §10, `L`): every watched location's images in one grid, searched
//! with the filter bar's query language across the whole catalogue (`specular-library`).
//!
//! The mode is the gallery with another source: instead of a folder listing, the rows of a
//! catalogue search (run off the frame, made into entries with their natural sort keys there)
//! replace the entries wholesale; the grid, tiles, thumbnail engine and cache, marks, quick
//! look, compare and opening in the viewer are the gallery's. The query is pushed down to SQL
//! (the gallery's own filter pass is skipped: the rows already match), and the facets panel
//! shows the catalogue's counts.
//!
//! The locations sidebar docks at the trailing edge (sharing the facets panel's dock: the
//! panel, when open, takes its place). It lists all locations, each with its count, offline
//! or pending state, a remove button, and "Add location…"; the indexer's progress shows at
//! its foot and is told to screen readers politely, at the quarters, like the similar-images
//! scan. While the grid scrolls the indexer is held back ([`specular_library::Library::hold`]).
//!
//! The catalogue never blocks a frame: it is opened (migrations included) on a `library-open`
//! worker while the grid says "Opening the library…" (and tells screen readers so); what the
//! user asks for meanwhile waits for it — the gallery's marks are mirrored in full, a folder
//! added is added, and the first search runs — and searches run on `lib-search` workers.
//! Everything else the frame asks of the [`Library`] is a message to its indexer or a copy of
//! shared state.

use std::hash::{Hash, Hasher};

use specular_gfx::{DrawList, Rect, Text, TextStyle, draw_icon};
use specular_input::NamedKey;
use specular_library::{self as lib, Library, Location, Phase, Progress, Search};
use specular_query::facet::Facet;
use specular_thumbs::ThumbKey;

use super::draw::{Frame, text_at, truncate};
use super::*;
use crate::app::ai::semantic;
use crate::loader::Wake;

/// AccessKit ids of the sidebar (beside the grid's, the facets panel's and the bar's).
pub(super) const A11Y_LIBRARY: u64 = A11Y_BASE + (8 << 40);
const ID_GROUP: u64 = A11Y_LIBRARY + 1;
const ID_PROGRESS: u64 = A11Y_LIBRARY + 2;
/// The offline banner (local copies) and its Review button.
const ID_BANNER: u64 = A11Y_LIBRARY + 3;
pub(super) const ID_REVIEW: u64 = A11Y_LIBRARY + 4;
/// Item `k` is `ID_ITEMS + 2k`, its remove button `ID_ITEMS + 2k + 1`.
const ID_ITEMS: u64 = A11Y_LIBRARY + 16;

/// Typing pauses this long before the catalogue is searched again.
const DEBOUNCE: Duration = Duration::from_millis(150);
/// New rows from the indexer refresh the grid at most this often (and not while it scrolls)…
const REFRESH: Duration = Duration::from_secs(3);
/// …but the first rows of an empty grid come at once…
const REFRESH_FIRST: Duration = Duration::from_millis(300);
/// …and a walk's rows every second while the grid is small (a big one sorts for longer).
const REFRESH_WALKING: Duration = Duration::from_secs(1);
/// Below this many rows the grid refreshes at [`REFRESH_WALKING`] during a walk.
const SMALL_GRID: usize = 20_000;
/// Above this many rows [`REFRESH`] grows in proportion: the grid's rebuild on the UI thread
/// grows with the rows (about 0.2 s at a million), so its share of the time stays small.
const LARGE_GRID: usize = 100_000;
/// A change of marks searches again after this long (a rating filter follows).
const MARKS_DELAY: Duration = Duration::from_millis(300);
/// The indexer is held while the grid moved within this long.
const BROWSING: Duration = Duration::from_millis(500);
/// The chrome item sent when the removal of a location is confirmed.
pub(in crate::app) const LIBRARY_REMOVE_ITEM: u64 = crate::app::items::LIBRARY_REMOVE;
/// The chrome items of "Add all of your home folder?": Pictures instead, or home anyway.
pub(in crate::app) const LIBRARY_HOME_PICTURES_ITEM: u64 = crate::app::items::LIBRARY_HOME_PICTURES;
pub(in crate::app) const LIBRARY_HOME_KEEP_ITEM: u64 = crate::app::items::LIBRARY_HOME_KEEP;
/// The help section on folder watches (the sidebar's "Some folders aren't watched").
const WATCHES_TOPIC: specular_help::Topic = specular_help::Topic {
    page: "library",
    anchor: "watches",
};
/// Changes in folders that are not watched show up within this many minutes (the walk
/// that catches them).
const UNWATCHED_MINUTES: i64 = (lib::RESCAN_EVERY.as_secs() / 60) as i64;
/// One turn of the walking bar's moving stripe (s).
const STRIPE_S: f64 = 1.4;

/// A place in the sidebar (and the top bar's toggle): hit areas and keyboard items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Side {
    /// The top bar's button showing or hiding the sidebar.
    Toggle,
    /// The rest of the sidebar (takes clicks from the grid under it).
    Panel,
    All,
    Location(i64),
    Remove(i64),
    Add,
    /// The foot's Cancel while indexing: stops walking and reading (what was catalogued
    /// stays searchable; the rest stays pending).
    Stop,
    /// The foot's Resume once stopped: walks every location again and reads what is pending.
    Resume,
    /// The foot's Retry once the indexer stopped by itself ([`Progress::halt`]).
    Retry,
    /// The foot's "Save diagnostics…" beside Retry.
    Diagnostics,
    /// "Some folders aren't watched" under the locations, once the folder watches ran out:
    /// opens the help on raising the limit.
    Watches,
    /// The albums section ([`super::albums`]): its heading (folds it), a group by its
    /// index in the tree (folds it), an album, their edit and delete buttons, and the
    /// "New album…" and "New smart album…" rows.
    AlbumsTitle,
    Group(usize),
    Album(i64),
    AlbumEdit(i64),
    AlbumDelete(i64),
    GroupEdit(usize),
    GroupDelete(usize),
    NewAlbum,
    NewSmart,
    /// A bar of the date timeline over the grid.
    Bar(usize),
    /// The offline banner's Review (local copies' conflicts).
    LocalReview,
}

/// What a search was asked for (a change searches again).
#[derive(Clone, Debug, PartialEq)]
struct Asked {
    text: String,
    location: Option<i64>,
    /// The album shown: a manual one's id, or a smart one's rules.
    album: Option<i64>,
    rules: Option<specular_query::rules::Rules>,
    facets: bool,
    sort: (SortKey, bool),
}

/// A finished search, made into gallery entries off the frame.
#[derive(Default)]
pub(super) struct Results {
    seq: u64,
    entries: Vec<Entry>,
    keys: Vec<NaturalKey>,
    loc: Option<Arc<Localizer>>,
    /// The catalogue's facts by path, and each entry's index by path: made here, off the
    /// frame (hashing a million paths took the UI thread about half a second). Both key by
    /// one shared copy of each path ([`Gallery::shared_path`]).
    metas: HashMap<Arc<Path>, Arc<Meta>>,
    by_path: HashMap<Arc<Path>, usize>,
    /// Entries the catalogue has no facts for yet.
    no_meta: Vec<usize>,
    /// The grid's order of the entries, for the sort asked for.
    order: Option<((SortKey, bool), Vec<usize>)>,
    marks: Vec<(PathBuf, Mark)>,
    /// Thumbnail keys of offline files (their thumbnails still show from the cache).
    known: Vec<(PathBuf, ThumbKey)>,
    facets: Vec<Facet>,
    total: usize,
    /// Of the rows as shown: the same again leaves the grid alone.
    hash: u64,
    error: Option<String>,
    /// Found by description (Local AI Search): best first, not in the sort order.
    ranked: bool,
    /// The rows are those shown (same hash, count and order as [`Shown`] said): `entries`,
    /// `keys`, `metas`, `marks` and `known` were not built.
    unchanged: bool,
    /// Photos with a Develop edit among the rows.
    edited: Vec<PathBuf>,
    /// Matches per day for the timeline.
    timeline: Vec<(i64, u64)>,
}

/// What the grid shows when a search starts: a search finding the same rows skips building
/// a second copy of them (at 100k files, about 150 MB every refresh while indexing).
#[derive(Clone, Copy)]
struct Shown {
    hash: u64,
    len: usize,
    ranked: bool,
}

#[derive(Default)]
pub(in crate::app) struct LibraryView {
    /// The gallery shows the Library.
    pub on: bool,
    pub(super) lib: Option<Arc<Library>>,
    /// Opening the catalogue failed (why).
    pub(super) error: Option<String>,
    /// The last search failed (why).
    pub(super) search_error: Option<String>,
    /// The sidebar is wanted (it shows unless the facets panel takes the dock).
    pub(super) panel: bool,
    pub(super) locations: Vec<Location>,
    /// The location whose removal is being asked ([`Gallery::library_ask_remove`]).
    pub(super) remove_ask: Option<i64>,
    pub(super) progress: Progress,
    /// Only this location's images (None: all).
    pub(super) location: Option<i64>,
    asked: Option<Asked>,
    /// When to search (debounced typing, refreshes).
    due: Option<Instant>,
    searching: bool,
    /// A search has come back since entering (until then the grid says nothing).
    pub(super) searched: bool,
    seq: u64,
    seen_changes: u64,
    searched_at: Option<Instant>,
    hash: u64,
    pub(super) facets: Vec<Facet>,
    facets_new: bool,
    pub(super) total: usize,
    /// The rows were found by description and show best first (the sort order waits).
    pub(super) relevance: bool,
    /// The folder the gallery showed before (L goes back to it).
    back: Option<PathBuf>,
    /// The indexer was busy this session (its progress node stays to say it is done).
    pub(super) was_busy: bool,
    /// The sidebar item with the keyboard focus (the sidebar has it when Some).
    pub(super) focus: Option<usize>,
    pub(super) scroll: f32,
    /// Where the sidebar was drawn, and its items (for keys and AccessKit).
    pub(super) rect: Option<Rect>,
    pub(super) rows: Vec<(Side, Rect)>,
    /// A folder chosen in the "Add location…" dialog.
    added_rx: Option<Receiver<PathBuf>>,
    /// The catalogue being opened on a worker (its result).
    opening: Option<Receiver<Result<Arc<Library>, String>>>,
    /// Folders added while it opens (added once it is open).
    queued_adds: Vec<PathBuf>,
    /// `--show library-stopped`: the indexing is stopped once the catalogue is open.
    show_stop: bool,
    /// A folder that holds the home folder, chosen to add: the question about it waits for
    /// the app ([`App::library_tick`]), and its answer adds it or Pictures.
    pub(super) home_ask: Option<HomeAsk>,
    /// The question about `home_ask` is to be asked.
    home_due: bool,
    /// The folders the walk passes over (Settings → Library), given to the catalogue when
    /// it opens and whenever they change.
    pub(super) skip: lib::Skip,
    /// Folders where other apps' sidecars have the last word (Settings → Library), likewise.
    pub(super) prefer_sidecars: Vec<PathBuf>,
    /// Albums and the timeline.
    pub(super) albums: super::albums::AlbumsView,
    /// Local copies for offline editing.
    pub(in crate::app) local: super::local::LocalView,
}

/// A location that would take in the whole home folder, and the Pictures folder offered
/// instead (None when there is none of its own).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct HomeAsk {
    pub folder: PathBuf,
    pub pictures: Option<PathBuf>,
}

/// Whether adding `folder` would take in the whole home folder `home` (it is the home folder
/// or holds it), and the Pictures folder to offer instead: `pictures` when it is a folder of
/// its own inside, not the home folder itself.
pub(in crate::app) fn home_ask(folder: &Path, home: &Path, pictures: &Path) -> Option<HomeAsk> {
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let (f, h) = (real(folder), real(home));
    if h.as_os_str().is_empty() || !h.starts_with(&f) {
        return None;
    }
    let pic = real(pictures);
    let pictures =
        (pic.is_dir() && pic != h && pic.starts_with(&f)).then(|| pictures.to_path_buf());
    Some(HomeAsk {
        folder: folder.to_path_buf(),
        pictures,
    })
}

/// The user's home folder (empty when unknown).
fn home_dir() -> PathBuf {
    std::env::home_dir().unwrap_or_default()
}

/// The Library's skip rules from the settings.
pub(in crate::app) fn skip_of(s: &specular_settings::Settings) -> lib::Skip {
    lib::Skip::parse(&s.library.skip_folders, &s.library.skip_marked)
}

impl LibraryView {
    /// The open catalogue (Develop stores edits in it).
    pub(in crate::app) fn handle(&self) -> Option<Arc<Library>> {
        self.lib.clone()
    }
}

impl LibraryView {
    /// The sidebar's keyboard items: all locations, each location, "Add location…" (not when
    /// the catalogue failed to open or is read-only: nothing could be added).
    pub(super) fn items(&self) -> Vec<Side> {
        let mut v = vec![Side::All];
        v.extend(self.locations.iter().map(|l| Side::Location(l.id)));
        if self.error.is_none() && !self.lib.as_ref().is_some_and(|l| l.read_only()) {
            v.push(Side::Add);
        }
        if self.progress.out_of_watches {
            v.push(Side::Watches);
        }
        if let Some(l) = self.lib.as_ref().filter(|_| self.error.is_none()) {
            v.extend(super::albums::items(&self.albums, !l.read_only()));
        }
        v.extend(self.foot_buttons());
        v
    }

    /// The foot's buttons, left to right: [`Self::foot_button`], after "Save diagnostics…"
    /// once the indexer stopped by itself.
    pub(super) fn foot_buttons(&self) -> Vec<Side> {
        match self.foot_button() {
            Some(Side::Retry) => vec![Side::Diagnostics, Side::Retry],
            b => b.into_iter().collect(),
        }
    }

    /// The foot's main button: Cancel while indexing, Resume once stopped, Retry once the
    /// indexer stopped by itself (none while the catalogue opens, failed or is read-only).
    pub(super) fn foot_button(&self) -> Option<Side> {
        if self.error.is_some() || self.opening() || self.lib.as_ref().is_none_or(|l| l.read_only())
        {
            return None;
        }
        if self.progress.halt.is_some() {
            Some(Side::Retry)
        } else if self.progress.busy() {
            Some(Side::Stop)
        } else if self.progress.stopped {
            Some(Side::Resume)
        } else {
            None
        }
    }

    /// The foot's line once the indexing was stopped, by the user or by itself (None
    /// otherwise).
    pub(super) fn stopped_text(&self) -> Option<String> {
        if let Some(h) = &self.progress.halt {
            return Some(t!("library-halted", reason = halt_reason(h)));
        }
        if !self.progress.stopped || self.progress.busy() {
            return None;
        }
        let pending: u64 = self.locations.iter().map(|l| l.pending).sum();
        Some(if pending > 0 {
            t!("library-stopped-pending", count = pending as i64)
        } else {
            t!("library-stopped")
        })
    }

    /// A search is due or running, or the catalogue still opening (the result count is not
    /// known yet).
    pub(super) fn pending(&self) -> bool {
        self.on && (self.searching || self.due.is_some() || self.opening.is_some())
    }

    /// The catalogue is being opened (on its worker).
    pub(super) fn opening(&self) -> bool {
        self.opening.is_some()
    }

    fn location(&self, id: i64) -> Option<&Location> {
        self.locations.iter().find(|l| l.id == id)
    }
}

/// A location's name as listed: its folder name (the path for a root).
fn location_name(l: &Location) -> String {
    l.path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| l.path.display().to_string())
}

/// A folder's name as said (the path for a root).
fn folder_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

/// How often new rows from the indexer refresh a grid of `rows` ([`REFRESH`]).
fn refresh_every(rows: usize, walking: bool) -> Duration {
    if rows == 0 {
        REFRESH_FIRST
    } else if walking && rows < SMALL_GRID {
        REFRESH_WALKING
    } else if rows > LARGE_GRID {
        REFRESH.mul_f64(rows as f64 / LARGE_GRID as f64)
    } else {
        REFRESH
    }
}

/// The walking bar's stripe: its start and width as fractions of the track at `now`
/// (seconds), or the whole track still under reduce-motion.
pub(super) fn walking_stripe(now: f64, reduce_motion: bool) -> (f32, f32) {
    if reduce_motion {
        return (0.0, 1.0);
    }
    const W: f64 = 0.3;
    let t = (now / STRIPE_S).rem_euclid(1.0);
    // From just off the start to just off the end, clipped to the track.
    let x = -W + t * (1.0 + W);
    let (a, b) = (x.max(0.0), (x + W).min(1.0));
    (a as f32, (b - a).max(0.0) as f32)
}

/// The indexer's progress in words (None when idle and nothing happened).
pub(super) fn progress_text(p: &Progress, read: u64) -> Option<String> {
    match p.phase {
        Phase::Idle => None,
        Phase::Walking => Some(t!(
            "library-walking",
            location = p
                .location
                .as_deref()
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            count = p.walked as i64
        )),
        Phase::Reading if p.held => Some(t!("library-held")),
        Phase::Reading => Some(t!(
            "library-reading",
            done = read.min(p.to_read) as i64,
            total = p.to_read as i64
        )),
    }
}

/// Why the indexer stopped by itself, in the user's words.
pub(super) fn halt_reason(h: &lib::Halt) -> String {
    match h {
        lib::Halt::Failed { class, io, .. } => crate::error_text::library_class_reason(*class, *io),
        lib::Halt::Stalled { secs } => {
            t!(
                "library-reason-stalled",
                minutes = (secs / 60).max(1) as i64
            )
        }
    }
}

/// `text` in at most two lines of `w` (the second cut short).
fn two_lines(text: &mut Text, s: &str, style: TextStyle, w: f32) -> Vec<String> {
    if text.measure(s, style) <= w {
        return vec![s.to_owned()];
    }
    let mut first = String::new();
    let mut rest = s;
    for (i, _) in s.match_indices(' ') {
        if text.measure(&s[..i], style) > w {
            break;
        }
        first = s[..i].to_owned();
        rest = s[i + 1..].trim_start();
    }
    if first.is_empty() {
        return vec![truncate(text, s, style, w)];
    }
    vec![first, truncate(text, rest, style, w)]
}

/// Runs a search and makes its rows into entries (on a background thread), with what the
/// grid looks them up by and their order for `view`: work proportional to the rows stays
/// off the frame. The rows come one by one ([`Library::search_each`]): the catalogue's whole
/// row set (about 70 MB at 100 000 files) is never held beside the entries. Rows the grid
/// shows already (`shown`) come back as `unchanged`, before their keys, tables and order
/// are made.
fn search(
    lib: &Library,
    req: &Search,
    seq: u64,
    shown: Option<Shown>,
    view: (SortKey, bool),
) -> Results {
    // `like:` terms, and plain words that name no file, search by description when Local
    // AI's Search is ready (crate::app::ai::semantic).
    let (asked, mut ranked) = semantic::with_like(lib, req);
    // `text:` terms: photos by the words read in them (Local AI's Text), when read.
    let (asked, by_text) = crate::app::ai::text_search::with_text(lib, &asked);
    ranked |= by_text;
    let mut made = Made::new(seq);
    let found = lib.search_each(&asked, &mut |row| made.add(row)).map(|f| {
        if !ranked
            && f.total == 0
            && let Some(d) = semantic::as_description(lib, req)
        {
            match lib.search_each(&d, &mut |row| made.add(row)) {
                Ok(g) => {
                    ranked = true;
                    return g;
                }
                // Rows it gave before failing are not shown.
                Err(_) => made = Made::new(seq),
            }
        }
        f
    });
    let found = match found {
        Ok(f) => f,
        Err(e) => {
            return Results {
                seq,
                error: Some(crate::error_text::library_reason(&e)),
                ..Results::default()
            };
        }
    };
    let hash = made.h.finish();
    let n = made.r.entries.len();
    if shown.is_some_and(|s| s.hash == hash && s.len == n && s.ranked == ranked) {
        // The entries made are dropped here, on the search's thread.
        return Results {
            seq,
            facets: found.facets,
            timeline: found.timeline,
            total: found.total,
            ranked,
            hash,
            unchanged: true,
            ..Results::default()
        };
    }
    made.r.facets = found.facets;
    made.r.timeline = found.timeline;
    made.r.total = found.total;
    made.r.ranked = ranked;
    made.finish(view)
}

/// A search's rows made into entries as they come ([`search`]); [`Made::finish`] adds the
/// rest once the rows are known to be new.
struct Made {
    r: Results,
    h: std::collections::hash_map::DefaultHasher,
    /// Facts by index into the entries.
    metas: Vec<(usize, Arc<Meta>)>,
}

impl Made {
    fn new(seq: u64) -> Made {
        Made {
            r: Results {
                seq,
                ..Results::default()
            },
            h: std::collections::hash_map::DefaultHasher::new(),
            metas: Vec::new(),
        }
    }

    fn add(&mut self, row: lib::Row) {
        let r = &mut self.r;
        (
            row.id,
            row.mtime_ns,
            row.size,
            row.mark,
            row.state,
            row.offline,
        )
            .hash(&mut self.h);
        if row.offline
            && let Some(k) = row.thumb_key()
        {
            r.known.push((row.path.clone(), k));
        }
        if row.mark != lib::Mark::default() {
            let m = row.mark;
            r.marks.push((
                row.path.clone(),
                Mark {
                    rating: m.rating,
                    label: m.label,
                    flag: m.flag,
                },
            ));
        }
        if let Some(d) = &row.develop {
            (d.edited, d.versions).hash(&mut self.h);
            r.edited.push(row.path.clone());
        }
        let modified = Some(row.modified_time());
        let (taken, dims) = match &row.meta {
            Some(m) => (
                m.date_taken,
                (m.width > 0 && m.height > 0).then_some((m.width, m.height)),
            ),
            None => (None, None),
        };
        let i = r.entries.len();
        match row.meta {
            Some(m) => self.metas.push((i, Arc::new(m))),
            None => r.no_meta.push(i),
        }
        r.entries.push(Entry {
            hidden: row.name.starts_with('.'),
            path: row.path,
            name: row.name,
            kind: EntryKind::Image,
            len: row.size,
            modified,
            taken,
            dims,
        });
    }

    /// The entries with their names' sort keys, the tables the grid finds them by (one shared
    /// copy of each path keys both) and their order for `view`.
    fn finish(self, view: (SortKey, bool)) -> Results {
        let Made { mut r, h, metas } = self;
        let loc = I18n::global().localizer();
        let mut keyer = loc.natural_keyer();
        r.keys = r.entries.iter().map(|e| keyer.key(&e.name)).collect();
        drop(keyer);
        r.by_path = HashMap::with_capacity(r.entries.len());
        for (i, e) in r.entries.iter().enumerate() {
            r.by_path.insert(Arc::from(e.path.as_path()), i);
        }
        r.metas = HashMap::with_capacity(metas.len());
        for (i, m) in metas {
            let p = r.entries[i].path.as_path();
            let k = r
                .by_path
                .get_key_value(p)
                .map_or_else(|| Arc::from(p), |(k, _)| k.clone());
            r.metas.insert(k, m);
        }
        if !r.ranked {
            r.order = Some((view, super::sort_order(&r.entries, &r.keys, view, &loc)));
        }
        r.loc = Some(loc);
        r.hash = h.finish();
        r
    }
}

/// `r` with `rows` made into entries, with their facts, marks and order (as [`search`]
/// makes them).
#[cfg(test)]
fn results(r: Results, rows: Vec<lib::Row>, view: (SortKey, bool)) -> Results {
    let mut made = Made::new(r.seq);
    made.r = r;
    for row in rows {
        made.add(row);
    }
    made.finish(view)
}

/// Drops `t` on a thread of its own (here, should no thread start): a million paths freed
/// took the UI thread a tenth of a second.
fn drop_off_frame<T: Send + 'static>(t: T) {
    let _ = specular_thread::builder("lib-drop").spawn(move || drop(t));
}

impl Gallery {
    /// The sidebar shows (the facets panel, when open, takes its place).
    pub(super) fn library_sidebar_shown(&self) -> bool {
        self.library.on
            && self.library.panel
            && !(self.filter_open && self.facets.open)
            && !self.keywords.open
            && !self.meta_edit.open
    }

    /// Leaves the Library's state behind (another folder opens in the gallery).
    pub(super) fn library_off(&mut self) {
        let lv = &mut self.library;
        lv.on = false;
        lv.focus = None;
        lv.rect = None;
        lv.rows.clear();
        lv.asked = None;
        lv.due = None;
        lv.seq += 1;
        lv.hash = 0;
        self.filter.scope = specular_query::Scope::Gallery;
        if let Some(a) = &self.archives {
            a.know_keys(Vec::new());
        }
    }

    /// The message of an empty Library grid (None: say nothing yet).
    pub(super) fn library_empty_text(&self) -> Option<String> {
        let lv = &self.library;
        if let Some(e) = &lv.error {
            return Some(t!("library-unavailable", reason = e.clone()));
        }
        if let Some(e) = &lv.search_error {
            return Some(t!("library-unavailable", reason = e.clone()));
        }
        if lv.opening() {
            return Some(t!("library-opening"));
        }
        if !lv.searched {
            return None;
        }
        if lv.locations.is_empty() {
            return Some(t!("library-empty"));
        }
        if !self.filter.is_empty() {
            return Some(t!("gallery-empty-filter"));
        }
        let shown: Vec<&Location> = lv
            .locations
            .iter()
            .filter(|l| lv.location.is_none_or(|id| id == l.id))
            .collect();
        if !shown.is_empty() && shown.iter().all(|l| l.offline) {
            return Some(t!("library-empty-offline"));
        }
        // Stopped before these were read: not "no images", and not still looking.
        if lv.stopped_text().is_some() {
            return Some(t!("library-empty-stopped"));
        }
        if lv.progress.phase == Phase::Walking
            && let Some(folder) = lv.progress.location.as_deref()
        {
            return Some(t!(
                "library-indexing-in",
                location = folder_name(folder),
                count = lv.progress.walked as i64
            ));
        }
        if lv.progress.busy() || shown.iter().any(|l| l.pending > 0) {
            return Some(t!("library-indexing"));
        }
        Some(t!("gallery-empty"))
    }

    /// When the Library wants the next turn (a debounced search), seconds from now.
    pub(super) fn library_wake(&self) -> Option<f64> {
        let lv = &self.library;
        // Opening: its worker wakes the loop (a timer meanwhile could spin it).
        if !lv.on || lv.searching || lv.opening() {
            return None;
        }
        lv.due
            .map(|t| t.saturating_duration_since(Instant::now()).as_secs_f64())
    }

    /// Searches again at once (an album shown, or changed).
    pub(super) fn library_search_soon(&mut self) {
        self.library.due = Some(Instant::now());
    }

    /// The facets panel shows the catalogue's counts (from the last search).
    pub(super) fn library_facets(&mut self) {
        if std::mem::take(&mut self.library.facets_new) {
            self.facets.facets = self.library.facets.clone();
            self.facets.generation += 1;
        }
    }

    /// The top bar's leading part: the sidebar toggle, "Library" and the image count.
    pub(super) fn draw_library_title(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        f: &Frame,
        avail: f32,
    ) {
        let pal = f.pal;
        let bar = TextStyle::new(13.0);
        let title = t!("library-title");
        let tw = text.measure(&title, bar.weight(600.0)).min(avail * 0.6);
        let tr = self.flip(Rect::new(48.0, 6.0, tw + 4.0, 32.0));
        let title = truncate(text, &title, bar.weight(600.0), tw + 4.0);
        text_at(text, dl, &title, bar.weight(600.0), tr, pal.text, self.rtl);
        // Nothing after the title when the catalogue failed (the grid says why).
        let lv = &self.library;
        let count = if lv.error.is_some() || lv.search_error.is_some() {
            String::new()
        } else if lv.searched {
            t!("gallery-items", count = self.tiles.len() as i64)
        } else {
            t!("library-searching")
        };
        let cw = (avail - tw - 16.0).max(0.0);
        let cr = self.flip(Rect::new(48.0 + tw + 12.0, 6.0, cw, 32.0));
        let count = truncate(text, &count, bar, cw);
        text_at(text, dl, &count, bar, cr, pal.text_muted, self.rtl);
    }

    /// Draws the locations sidebar at the trailing edge.
    /// Draws the sidebar. True while its walking bar moves (draw again soon).
    pub(super) fn draw_library(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) -> bool {
        if !self.library_sidebar_shown() {
            self.library.rect = None;
            self.library.rows.clear();
            return false;
        }
        let pal = f.pal;
        let ts = f.text_scale.clamp(0.75, 3.0);
        let u = |v: f32| (v * ts).round();
        let title_style = TextStyle::new(14.0 * ts).weight(600.0);
        let row_style = TextStyle::new(13.0 * ts);
        let small = TextStyle::new(12.0 * ts).tabular();
        let pw = self.facets_width(ts);
        let top = TOP_BAR
            + if self.filter_open {
                self.filter_bar_h()
            } else {
                0.0
            };
        let h = (self.vh - BOTTOM_BAR - top).max(0.0);
        let panel = Rect::new(self.vw - pw, top, pw, h);
        let r = self.flip(panel);
        dl.shadow(r, 0.0, pal.shadow, 10.0, 0.0);
        dl.rect(r, 0.0, pal.surface_raised);
        dl.rect(
            self.flip(Rect::new(panel.x, top, 1.0, h)),
            0.0,
            pal.separator,
        );
        self.hits.push((r, Hit::Library(Side::Panel)));
        self.library.rect = Some(r);

        let pad = u(12.0);
        let title_h = u(40.0);
        let tr = self.flip(Rect::new(panel.x + pad, top, pw - 2.0 * pad, title_h));
        let s = truncate(text, &t!("library-locations"), title_style, tr.w);
        text_at(text, dl, &s, title_style, tr, pal.text, self.rtl);

        // The foot: the indexer's progress (taller when it stopped by itself: two lines of
        // why, then Save diagnostics… and Retry).
        let halted = self.library.progress.halt.is_some();
        let foot_h = if halted { u(88.0) } else { u(56.0) };
        let foot = Rect::new(panel.x, panel.bottom() - foot_h, pw, foot_h);
        let body = Rect::new(panel.x, top + title_h, pw, (h - title_h - foot_h).max(0.0));

        let row_h = u(44.0);
        let items = self.library.items();
        let foot_buttons = self.library.foot_buttons();
        // The rows of the body (the foot's buttons are the last keyboard items).
        let body_n = items.len() - foot_buttons.len();
        let content = body_n as f32 * row_h + pad;
        if let Some(k) = self.library.focus.filter(|&k| k < body_n) {
            let y = k as f32 * row_h;
            if y < self.library.scroll {
                self.library.scroll = y;
            } else if y + row_h > self.library.scroll + body.h {
                self.library.scroll = y + row_h - body.h;
            }
        }
        self.library.scroll = self.library.scroll.clamp(0.0, (content - body.h).max(0.0));
        let mut placed = Vec::with_capacity(items.len());
        dl.push_clip(self.flip(body));
        let mut y = body.y - self.library.scroll;
        let all_files: u64 = self.library.locations.iter().map(|l| l.files).sum();
        for (k, item) in items.iter().enumerate().take(body_n) {
            let lr = Rect::new(body.x + u(6.0), y, body.w - u(12.0), row_h);
            let rr = self.flip(lr);
            if y + row_h > body.y && y < body.bottom() {
                let hovered = self.pointer.is_some_and(|p| rr.contains(p));
                let focused = self.library.focus == Some(k);
                let in_album = self.library.albums.open.is_some();
                let selected = match item {
                    Side::All => self.library.location.is_none() && !in_album,
                    Side::Location(id) => self.library.location == Some(*id) && !in_album,
                    Side::Album(id) => self.library.albums.open == Some(*id),
                    _ => false,
                };
                let heading = *item == Side::AlbumsTitle;
                if heading {
                    dl.rect(self.flip(Rect::new(lr.x, y, lr.w, 1.0)), 0.0, pal.separator);
                }
                if selected {
                    dl.rect(rr, u(6.0), pal.accent_soft);
                } else if hovered {
                    dl.rect(rr, u(6.0), pal.surface_hover);
                }
                if focused {
                    dl.stroke_rect(rr, u(6.0), pal.focus_ring, 1.5);
                }
                let icon_s = u(18.0);
                let look = self.album_look(*item);
                let indent = look.as_ref().map_or(0.0, |l| l.3 as f32 * u(14.0));
                let ir = self.flip(Rect::new(
                    lr.x + u(8.0) + indent,
                    y + (row_h - icon_s) * 0.5,
                    icon_s,
                    icon_s,
                ));
                let text_x = lr.x + u(8.0) + indent + icon_s + u(10.0);
                let (icon, name, sub, muted) = match item {
                    _ if look.is_some() => {
                        let (icon, name, sub, _) =
                            look.clone().unwrap_or((Icon::Plus, String::new(), None, 0));
                        (icon, name, sub, false)
                    }
                    Side::All => (
                        Icon::Grid,
                        t!("library-all"),
                        Some(t!("gallery-items", count = all_files as i64)),
                        false,
                    ),
                    Side::Location(id) => {
                        let l = self.library.location(*id).cloned().unwrap_or(Location {
                            id: *id,
                            path: PathBuf::new(),
                            offline: false,
                            files: 0,
                            pending: 0,
                            last_scan: None,
                        });
                        let note = self.local_location_note(*id);
                        let sub = if l.offline {
                            match note {
                                Some(n) => specular_i18n::dot_join(&[t!("library-offline"), n]),
                                None => t!("library-offline"),
                            }
                        } else if let Some(n) = note {
                            specular_i18n::dot_join(&[
                                t!("gallery-items", count = l.files as i64),
                                n,
                            ])
                        } else if l.pending > 0 {
                            specular_i18n::dot_join(&[
                                t!("gallery-items", count = l.files as i64),
                                t!("library-location-pending", count = l.pending as i64),
                            ])
                        } else {
                            t!("gallery-items", count = l.files as i64)
                        };
                        (Icon::Folder, location_name(&l), Some(sub), l.offline)
                    }
                    Side::Watches => (
                        Icon::Info,
                        t!("library-unwatched"),
                        Some(t!("library-unwatched-detail", minutes = UNWATCHED_MINUTES)),
                        false,
                    ),
                    _ => (Icon::Plus, t!("library-add"), None, false),
                };
                let c = if muted { pal.text_muted } else { pal.text };
                draw_icon(dl, icon, ir, c, 1.6, self.rtl);
                // The remove button of a location, an album's edit and delete buttons (on
                // hover or focus), from the end edge.
                let mut text_w = (lr.right() - u(8.0) - text_x).max(0.0);
                let mut remove_hit = Vec::new();
                let buttons = match item {
                    Side::Location(id) => vec![(Icon::Close, Side::Remove(*id))],
                    _ => self.album_buttons(*item),
                };
                if hovered || focused {
                    let bs = u(28.0);
                    let mut right = lr.right() - u(4.0);
                    for (icon, side) in buttons {
                        let br = self.flip(Rect::new(right - bs, y + (row_h - bs) * 0.5, bs, bs));
                        right -= bs + u(2.0);
                        let bh = self.pointer.is_some_and(|p| br.contains(p));
                        if bh {
                            dl.rect(br, u(6.0), pal.surface_pressed);
                        }
                        draw_icon(
                            dl,
                            icon,
                            br.centered(u(14.0), u(14.0)),
                            pal.text_muted,
                            1.6,
                            self.rtl,
                        );
                        remove_hit.push((br, Hit::Library(side)));
                        text_w = (text_w - bs - u(2.0)).max(0.0);
                    }
                }
                match sub {
                    Some(sub) => {
                        let nr =
                            self.flip(Rect::new(text_x, y + u(4.0), text_w, row_h * 0.5 - u(2.0)));
                        let s = truncate(text, &name, row_style, text_w);
                        text_at(text, dl, &s, row_style, nr, c, self.rtl);
                        let sr = self.flip(Rect::new(
                            text_x,
                            y + row_h * 0.5,
                            text_w,
                            row_h * 0.5 - u(4.0),
                        ));
                        let s = truncate(text, &sub, small, text_w);
                        text_at(text, dl, &s, small, sr, pal.text_muted, self.rtl);
                    }
                    None if heading => {
                        let nr = self.flip(Rect::new(text_x, y, text_w, row_h));
                        let s = truncate(text, &name, title_style, text_w);
                        text_at(text, dl, &s, title_style, nr, pal.text, self.rtl);
                    }
                    None => {
                        let nr = self.flip(Rect::new(text_x, y, text_w, row_h));
                        let s = truncate(text, &name, row_style, text_w);
                        text_at(text, dl, &s, row_style, nr, pal.accent, self.rtl);
                    }
                }
                self.hits
                    .push((rr.intersect(&self.flip(body)), Hit::Library(*item)));
                // After its row: the last hit pushed wins (`hit_at`), and the row's area
                // holds the button (pushed first, a click on X chose the location instead).
                self.hits.extend(remove_hit);
            }
            placed.push((*item, rr));
            y += row_h;
        }
        dl.pop_clip();

        // Foot: the progress line and bar, and Cancel (or Resume) at its end.
        dl.rect(
            self.flip(Rect::new(foot.x, foot.y, foot.w, 1.0)),
            0.0,
            pal.separator,
        );
        let mut line_w = foot.w - 2.0 * pad;
        // Right to left from the foot's end: the main button last in reading order.
        let mut right = foot.right() - pad;
        let by = if halted { u(56.0) } else { u(6.0) };
        for (j, &b) in foot_buttons.iter().enumerate().rev() {
            let label = match b {
                Side::Stop => t!("app-cancel"),
                Side::Retry => t!("library-retry"),
                Side::Diagnostics => t!("library-save-diagnostics"),
                _ => t!("library-resume"),
            };
            let share = if halted { 0.6 } else { 0.5 };
            let bw = (text.measure(&label, small) + u(20.0)).min(line_w * share);
            let br = self.flip(Rect::new(right - bw, foot.y + by, bw, u(24.0)));
            right -= bw + u(8.0);
            if self.pointer.is_some_and(|p| br.contains(p)) {
                dl.rect(br, u(6.0), pal.surface_hover);
            }
            dl.stroke_rect(br, u(6.0), pal.separator, 1.0);
            if self.library.focus == Some(body_n + j) {
                dl.stroke_rect(br, u(6.0), pal.focus_ring, 1.5);
            }
            let s = truncate(text, &label, small, bw - u(8.0));
            similar::put_centered(text, dl, &s, small, br, pal.text);
            self.hits.push((br, Hit::Library(b)));
            placed.push((b, br));
            if !halted {
                line_w = (line_w - bw - u(8.0)).max(0.0);
            }
        }
        // Keyboard order, left to right.
        let n = placed.len();
        placed[n - foot_buttons.len()..].reverse();
        self.library.rows = placed;
        let p = &self.library.progress;
        let line: Option<String> = if self.library.error.is_some() {
            None
        } else if self.library.opening() {
            Some(t!("library-opening"))
        } else if self.library.lib.as_ref().is_some_and(|l| l.read_only()) {
            Some(t!("library-read-only"))
        } else if p.halt.is_some() {
            self.library.stopped_text()
        } else {
            progress_text(p, p.read)
                .or_else(|| self.library.stopped_text())
                .or_else(|| {
                    (self.library.was_busy || !self.library.locations.is_empty())
                        .then(|| t!("library-up-to-date"))
                })
        };
        if halted && let Some(line) = &line {
            for (k, l) in two_lines(text, line, small, line_w).iter().enumerate() {
                let y = foot.y + u(8.0) + k as f32 * u(20.0);
                let lr = self.flip(Rect::new(foot.x + pad, y, line_w, u(20.0)));
                text_at(text, dl, l, small, lr, pal.text, self.rtl);
            }
        } else if let Some(mut line) = line {
            let lr = self.flip(Rect::new(foot.x + pad, foot.y + u(8.0), line_w, u(20.0)));
            // Walking in a narrow sidebar: the folder's name goes first, then the end is cut;
            // the count comes first in every language, so it always shows.
            if p.phase == Phase::Walking && text.measure(&line, small) > lr.w {
                line = t!("library-walking-count", count = p.walked as i64);
            }
            let s = truncate(text, &line, small, lr.w);
            text_at(text, dl, &s, small, lr, pal.text_muted, self.rtl);
        }
        let mut moving = false;
        // A stopped or stalled indexer shows no bar (never an endless busy one).
        if p.busy() {
            let track = Rect::new(foot.x + pad, foot.y + u(36.0), foot.w - 2.0 * pad, u(4.0));
            dl.rect(self.flip(track), u(2.0), pal.separator);
            match p.phase {
                // How much there is to walk is not known: a stripe moves along the track
                // (under reduce-motion, the track is filled softly and stays).
                Phase::Walking => {
                    let (a, w) = walking_stripe(f.now, f.reduce_motion);
                    let c = if f.reduce_motion {
                        pal.accent_soft
                    } else {
                        pal.accent
                    };
                    if w > 0.0 {
                        let fill = Rect::new(track.x + track.w * a, track.y, track.w * w, track.h);
                        dl.rect(self.flip(fill), u(2.0), c);
                    }
                    moving = !f.reduce_motion;
                }
                _ => {
                    let frac = if p.to_read > 0 {
                        (p.read as f32 / p.to_read as f32).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    if frac > 0.0 {
                        let fill = Rect::new(track.x, track.y, track.w * frac, track.h);
                        dl.rect(self.flip(fill), u(2.0), pal.accent);
                    }
                }
            }
        }
        moving
    }

    /// The wheel over the sidebar. True when it was there.
    pub(super) fn library_wheel(&mut self, dy: f32, p: (f32, f32)) -> bool {
        if !self.library.rect.is_some_and(|r| r.contains(p)) {
            return false;
        }
        self.library.scroll = (self.library.scroll - dy).max(0.0);
        true
    }

    /// AccessKit nodes of the sidebar and the indexer's progress. True when the sidebar has
    /// the keyboard focus (its item then has the tree's).
    pub(super) fn library_a11y(
        &self,
        tree: &mut accesskit::TreeUpdate,
        bounds: &dyn Fn(Rect) -> accesskit::Rect,
    ) -> bool {
        use accesskit::{Live, Node, NodeId, Role, Toggled};
        let lv = &self.library;
        if !lv.on {
            return false;
        }
        let mut top = Vec::new();
        // The indexer's progress: its value as it goes, its name (said politely) only at the
        // quarters of the reading, so a screen reader is not talked over at every file.
        let p = &lv.progress;
        if lv.opening() {
            // Opening the catalogue: said once, politely (it may take a few seconds).
            let mut n = Node::new(Role::ProgressIndicator);
            n.set_live(Live::Polite);
            n.set_label(t!("library-opening"));
            tree.nodes.push((NodeId(ID_PROGRESS), n));
            top.push(NodeId(ID_PROGRESS));
        } else if let Some(line) = lv.stopped_text().filter(|_| p.halt.is_some()) {
            // Stopped by itself: said at once (assertive), with its reason.
            let mut n = Node::new(Role::Status);
            n.set_live(Live::Assertive);
            n.set_label(similar::without_isolates(&line));
            tree.nodes.push((NodeId(ID_PROGRESS), n));
            top.push(NodeId(ID_PROGRESS));
        } else if lv.was_busy || p.busy() {
            let mut n = Node::new(Role::ProgressIndicator);
            n.set_live(Live::Polite);
            match p.phase {
                Phase::Reading => {
                    let said = similar::said_progress(p.read as usize, p.to_read as usize) as u64;
                    n.set_label(progress_text(p, said).unwrap_or_default());
                    n.set_numeric_value(p.read.min(p.to_read) as f64);
                    n.set_min_numeric_value(0.0);
                    n.set_max_numeric_value(p.to_read.max(1) as f64);
                }
                Phase::Walking => n.set_label(t!("library-indexing")),
                Phase::Idle => n.set_label(
                    lv.stopped_text()
                        .unwrap_or_else(|| t!("library-up-to-date")),
                ),
            }
            let full = progress_text(p, p.read)
                .or_else(|| lv.stopped_text())
                .unwrap_or_else(|| t!("library-up-to-date"));
            n.set_value(similar::without_isolates(&full));
            tree.nodes.push((NodeId(ID_PROGRESS), n));
            top.push(NodeId(ID_PROGRESS));
        }
        // The offline banner (local copies), said politely when it changes; its Review.
        if let Some((lines, review)) = self.local_banner() {
            let mut n = Node::new(Role::Status);
            n.set_live(Live::Polite);
            n.set_label(lines.join(" "));
            tree.nodes.push((NodeId(ID_BANNER), n));
            top.push(NodeId(ID_BANNER));
            if review {
                let mut b = Node::new(Role::Button);
                b.set_label(t!("local-review"));
                b.add_action(accesskit::Action::Click);
                tree.nodes.push((NodeId(ID_REVIEW), b));
                top.push(NodeId(ID_REVIEW));
            }
        }
        let mut focused = false;
        if let Some(r) = lv.rect {
            let mut group = Node::new(Role::Group);
            group.set_label(t!("library-locations"));
            group.set_bounds(bounds(r));
            let mut ids = Vec::new();
            for (k, (side, rr)) in lv.rows.iter().enumerate() {
                let nid = NodeId(ID_ITEMS + 2 * k as u64);
                let mut n;
                let mut extra = Vec::new();
                match side {
                    Side::All => {
                        n = Node::new(Role::RadioButton);
                        n.set_label(t!("library-all"));
                        n.set_toggled(if lv.location.is_none() {
                            Toggled::True
                        } else {
                            Toggled::False
                        });
                    }
                    Side::Location(id) => {
                        let Some(l) = lv.location(*id) else { continue };
                        n = Node::new(Role::RadioButton);
                        let name = location_name(l);
                        n.set_label(if l.offline {
                            t!(
                                "library-a11y-location-offline",
                                location = name.as_str(),
                                count = l.files as i64
                            )
                        } else {
                            t!(
                                "library-a11y-location",
                                location = name.as_str(),
                                count = l.files as i64
                            )
                        });
                        if l.pending > 0 {
                            n.set_description(t!(
                                "library-location-pending",
                                count = l.pending as i64
                            ));
                        }
                        n.set_toggled(if lv.location == Some(*id) {
                            Toggled::True
                        } else {
                            Toggled::False
                        });
                        // Its remove button.
                        let bid = NodeId(nid.0 + 1);
                        let mut b = Node::new(Role::Button);
                        b.set_label(t!("library-remove", location = name.as_str()));
                        b.set_bounds(bounds(*rr));
                        b.add_action(accesskit::Action::Click);
                        tree.nodes.push((bid, b));
                        ids.push(nid);
                        ids.push(bid);
                        n.set_bounds(bounds(*rr));
                        n.add_action(accesskit::Action::Click);
                        n.add_action(accesskit::Action::Focus);
                        tree.nodes.push((nid, n));
                        if lv.focus == Some(k) && tree.focus == NodeId(1) {
                            tree.focus = nid;
                            focused = true;
                        }
                        continue;
                    }
                    Side::Stop => {
                        n = Node::new(Role::Button);
                        n.set_label(t!("app-cancel"));
                        n.set_description(t!("library-stop-description"));
                    }
                    Side::Resume => {
                        n = Node::new(Role::Button);
                        n.set_label(t!("library-resume"));
                        n.set_description(t!("library-resume-description"));
                    }
                    Side::Retry => {
                        n = Node::new(Role::Button);
                        n.set_label(t!("library-retry"));
                        n.set_description(t!("library-retry-description"));
                    }
                    Side::Diagnostics => {
                        n = Node::new(Role::Button);
                        n.set_label(t!("library-save-diagnostics"));
                        n.set_description(t!("palette-save-diagnostics-help"));
                    }
                    Side::Watches => {
                        n = Node::new(Role::Button);
                        n.set_label(format!(
                            "{}, {}",
                            t!("library-unwatched"),
                            t!("library-unwatched-detail", minutes = UNWATCHED_MINUTES)
                        ));
                        n.set_description(t!("library-unwatched-description"));
                    }
                    side if self.album_look(*side).is_some() => {
                        n = self.album_node(*side, *rr, bounds, tree, &mut extra, k);
                    }
                    _ => {
                        n = Node::new(Role::Button);
                        n.set_label(t!("library-add"));
                    }
                }
                n.set_bounds(bounds(*rr));
                n.add_action(accesskit::Action::Click);
                n.add_action(accesskit::Action::Focus);
                ids.push(nid);
                ids.append(&mut extra);
                tree.nodes.push((nid, n));
                if lv.focus == Some(k) && tree.focus == NodeId(1) {
                    tree.focus = nid;
                    focused = true;
                }
            }
            group.set_children(ids);
            tree.nodes.push((NodeId(ID_GROUP), group));
            top.push(NodeId(ID_GROUP));
        }
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(i, _)| *i == NodeId(1)) {
            for id in top {
                root.push_child(id);
            }
        }
        focused
    }
}

/// The catalogue's side of the Library, without the [`App`] (so tests drive it as the frame
/// does): nothing here waits for the catalogue.
impl Gallery {
    /// Starts opening the catalogue on a `lib-open` worker, which wakes the loop with
    /// `cfg.wake` when done ([`Gallery::library_poll`] adopts it). Once per run: false (and
    /// nothing started) while it opens, after it opened, and after it failed.
    pub(super) fn library_begin_open(&mut self, cfg: lib::Config) -> bool {
        let lv = &mut self.library;
        if lv.lib.is_some() || lv.error.is_some() || lv.opening.is_some() {
            return false;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let wake = cfg.wake.clone();
        let spawned = specular_thread::builder("lib-open").spawn(move || {
            let r = Library::open(cfg).map(Arc::new).map_err(|e| {
                specular_log::warn!(target: specular_log::FOLDER, "library: catalogue not opened: {e}");
                crate::error_text::library_reason(&e)
            });
            let _ = tx.send(r);
            wake();
        });
        match spawned {
            Ok(_) => lv.opening = Some(rx),
            Err(e) => lv.error = Some(crate::error_text::io_reason(&e)),
        }
        self.dirty = true;
        true
    }

    /// The catalogue opened (or failed to): what was asked meanwhile is applied. True when
    /// it came (redraw).
    fn library_adopt(&mut self, now: Instant) -> bool {
        let lv = &mut self.library;
        let r = match lv.opening.as_ref().map(Receiver::try_recv) {
            None | Some(Err(crossbeam_channel::TryRecvError::Empty)) => return false,
            Some(Ok(r)) => r,
            // Its worker ended without a word (it panicked): a bug.
            Some(Err(crossbeam_channel::TryRecvError::Disconnected)) => {
                Err(t!("error-reason-internal"))
            }
        };
        lv.opening = None;
        match r {
            Ok(l) => {
                info!(target: UI, "library: opened");
                // Changed while it opened.
                l.set_skip(lv.skip.clone());
                // The gallery's marks into the catalogue (sidecars it reads itself): all of
                // them, so marks set while it opened are in.
                let marks: Vec<(PathBuf, lib::Mark)> = self
                    .marks
                    .marks
                    .iter()
                    .map(|(p, m)| (p.clone(), lib_mark(*m)))
                    .collect();
                l.set_marks(marks);
                for p in lv.queued_adds.drain(..) {
                    l.add_location(&p);
                }
                lv.locations = l.locations();
                if std::mem::take(&mut lv.show_stop) {
                    // `--show library-stopped`: stopped as soon as it began.
                    l.cancel();
                }
                lv.lib = Some(l);
                if lv.on {
                    // The first search (it may have been asked already).
                    lv.due = Some(now);
                }
            }
            Err(e) => {
                warn!(target: UI, "library: {e}");
                lv.error = Some(e);
                lv.queued_adds.clear();
            }
        }
        self.dirty = true;
        true
    }

    /// Adds a location (once the catalogue is open, if it is not yet). The announcement
    /// follows at once.
    pub(super) fn library_add_location(&mut self, p: PathBuf, now: Instant) {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.display().to_string());
        let lv = &mut self.library;
        match &lv.lib {
            Some(l) => {
                l.add_location(&p);
                lv.due = Some(now);
            }
            // The search follows the opening.
            None if lv.opening() => lv.queued_adds.push(p),
            None => return,
        }
        self.announce = Some(t!("library-a11y-added", location = name));
    }

    /// Adds a location, unless it would take in the whole home folder: then the app asks
    /// first ([`LibraryView::home_ask`]).
    pub(super) fn library_add_checked(&mut self, p: PathBuf, now: Instant) {
        let pictures = specular_platform::integration::user_pictures_dir();
        match home_ask(&p, &home_dir(), &pictures) {
            Some(ask) => {
                self.library.home_ask = Some(ask);
                self.library.home_due = true;
            }
            None => self.library_add_location(p, now),
        }
    }

    /// The folders where other apps' sidecars have the last word changed (Settings →
    /// Library).
    pub(in crate::app) fn library_set_prefer(&mut self, folders: Vec<PathBuf>) {
        if let Some(l) = &self.library.lib {
            l.set_prefer_foreign(folders.clone());
        }
        self.library.prefer_sidecars = folders;
    }

    /// The Library's skip rules changed (Settings → Library).
    /// Settings › Library › Date timeline.
    pub(in crate::app) fn library_show_timeline(&mut self, on: bool) {
        let av = &mut self.library.albums;
        if av.hide_timeline == !on {
            return;
        }
        av.hide_timeline = !on;
        if !on {
            av.kb = None;
        }
        self.relayout = true;
        self.dirty = true;
    }

    pub(in crate::app) fn library_set_skip(&mut self, skip: lib::Skip) {
        if let Some(l) = &self.library.lib {
            l.set_skip(skip.clone());
        }
        self.library.skip = skip;
    }

    /// Once per event-loop turn while the gallery may show the Library: the catalogue
    /// opening, a folder chosen to add, progress, the indexer's hold, debounced searches
    /// (started on a worker that sends [`Msg::Library`] and calls `wake`). True: redraw.
    pub(super) fn library_poll(&mut self, wake: &Wake) -> bool {
        let now = Instant::now();
        let mut redraw = self.library_adopt(now);
        // A folder chosen to add.
        if let Some(rx) = &self.library.added_rx
            && let Ok(p) = rx.try_recv()
        {
            self.library.added_rx = None;
            self.library_add_checked(p, now);
            redraw = true;
        }
        let Some(l) = self.library.lib.clone() else {
            return redraw;
        };
        let ga = self;
        let on = ga.active && ga.library.on;
        let progress = l.progress();
        let locations = l.locations();
        let lv = &mut ga.library;
        if progress != lv.progress || locations != lv.locations {
            if on {
                redraw = true;
            }
            let finished = lv.progress.busy() && !progress.busy();
            lv.was_busy |= progress.busy();
            if locations.len() != lv.locations.len() {
                lv.due.get_or_insert(now);
            }
            if finished && progress.changes != lv.seen_changes {
                lv.due = Some(now);
            }
            lv.progress = progress;
            if let Some(k) = lv.focus {
                let n = locations.len() + 2;
                lv.focus = Some(k.min(n - 1));
            }
            if lv
                .location
                .is_some_and(|id| !locations.iter().any(|l| l.id == id))
            {
                lv.location = None;
            }
            lv.locations = locations;
        }
        if !ga.library.albums.loaded && !ga.library.albums.busy {
            ga.albums_run(super::albums::Op::Load, wake.clone());
        }
        if !on {
            return redraw;
        }
        ga.filter.scope = specular_query::Scope::Library;
        // Browsing: the indexer backs off (the walk, facts, and signatures after them, which
        // run while the phase shows idle). Only the grid moving says so (a scroll, a fling,
        // keys, the scrollbar): not `wish_fast`, which an empty grid never clears (it kept
        // the hold renewed by every redraw while "Looking for images…" showed), nor hovers.
        let browsing =
            ga.moved_at.is_some_and(|t| t.elapsed() < BROWSING) || ga.scrollbar_drag.is_some();
        if browsing {
            l.hold();
        }
        let (album, rules) = ga.albums_scope();
        let asked = Asked {
            text: ga.filter.text.clone(),
            location: ga.library.location,
            album,
            rules,
            facets: ga.filter_open && (ga.facets.open || ga.picker_counts()),
            sort: (ga.view.sort, ga.view.desc),
        };
        let lv = &mut ga.library;
        if lv.asked.as_ref() != Some(&asked) {
            let first = lv.asked.is_none();
            lv.asked = Some(asked.clone());
            lv.due = Some(if first { now } else { now + DEBOUNCE });
        }
        // New rows from the indexer: refresh now and then, not while the grid scrolls.
        let every = refresh_every(ga.entries.len(), lv.progress.phase == Phase::Walking);
        if lv.progress.changes != lv.seen_changes
            && lv.due.is_none()
            && !browsing
            && lv.searched_at.is_none_or(|t| t.elapsed() >= every)
        {
            lv.due = Some(now);
        }
        if lv.searching || lv.due.is_none_or(|t| t > now) {
            return redraw;
        }
        lv.due = None;
        lv.searching = true;
        lv.seq += 1;
        lv.seen_changes = lv.progress.changes;
        lv.searched_at = Some(now);
        let req = Search {
            query: asked.text,
            // The rows are ordered for the view off the frame (`results`): SQLite's sort
            // would be done twice (most of a million-row search).
            sort: lib::Sort::Unordered,
            descending: asked.sort.1,
            limit: None,
            facets: asked.facets,
            location: asked.location,
            no_pushdown: false,
            language: Some(I18n::global().language()),
            ranked: None,
            album: asked.album,
            rules: asked.rules,
            timeline: true,
        };
        let shown = lv.searched.then_some(Shown {
            hash: lv.hash,
            len: ga.entries.len(),
            ranked: lv.relevance,
        });
        let (seq, tx, wake) = (lv.seq, ga.tx.clone(), wake.clone());
        let view = asked.sort;
        let spawned = specular_thread::builder("lib-search").spawn(move || {
            // A panic is this search's error: "Searching…" must end, and the next
            // search runs as usual.
            let r = specular_core::isolate::run("library search", || {
                search(&l, &req, seq, shown, view)
            })
            .unwrap_or_else(|_| Results {
                seq,
                error: Some(t!("error-reason-internal")),
                ..Results::default()
            });
            let _ = tx.send(Msg::Library(Box::new(r)));
            wake();
        });
        if spawned.is_err() {
            // Tried again next turn.
            lv.searching = false;
            lv.due = Some(now + DEBOUNCE);
        }
        redraw
    }

    /// A search came back: its rows replace the grid's entries. True: redraw.
    pub(super) fn library_results(&mut self, mut r: Results) -> bool {
        let edited = std::mem::take(&mut r.edited);
        let ga = self;
        let lv = &mut ga.library;
        if r.seq != lv.seq {
            return false;
        }
        lv.searching = false;
        if !lv.on {
            return false;
        }
        let first = !lv.searched;
        lv.searched = true;
        lv.search_error = r.error;
        if lv.search_error.is_some() {
            ga.dirty = true;
            return true;
        }
        lv.facets = r.facets;
        lv.facets_new = true;
        let had = lv.albums.days.len() > 1;
        lv.albums.days = std::mem::take(&mut r.timeline);
        if had != (lv.albums.days.len() > 1) {
            // The timeline strip comes or goes: the grid moves.
            ga.relayout = true;
        }
        let lv = &mut ga.library;
        lv.total = r.total;
        let order_changed = lv.relevance != r.ranked;
        lv.relevance = r.ranked;
        let same =
            !order_changed && r.hash == lv.hash && r.entries.len() == ga.entries.len() && !first;
        if r.unchanged && !(!order_changed && r.hash == lv.hash && !first) {
            // The grid changed while it searched (no rows came): search again in full.
            lv.hash = 0;
            lv.due = Some(Instant::now());
            return false;
        }
        lv.hash = r.hash;
        if same || r.unchanged {
            return true;
        }
        // Everything proportional to the rows was made off the frame (`search`): the tables
        // are swapped in, and the old ones freed on another thread.
        let mut metas = r.metas;
        for &i in &r.no_meta {
            // No facts in the catalogue yet: what the gallery read itself stays.
            let p = r.entries[i].path.as_path();
            if let Some(m) = ga.metas.remove(p) {
                let k = r
                    .by_path
                    .get_key_value(p)
                    .map_or_else(|| Arc::from(p), |(k, _)| k.clone());
                metas.insert(k, m);
            }
        }
        // The gallery's table wins (it is mirrored into the catalogue).
        ga.marks.merge_read(r.marks);
        if let Some(a) = &ga.archives {
            a.know_keys(r.known);
        }
        // The rows are new: the tiles follow their files until the rebuild.
        let (entries, by_path) = (&ga.entries, &r.by_path);
        super::remap_tiles(&mut ga.tiles, |i| {
            entries
                .get(i)
                .and_then(|e| by_path.get(e.path.as_path()))
                .copied()
        });
        let old = (
            std::mem::replace(&mut ga.metas, metas),
            std::mem::replace(&mut ga.by_path, r.by_path),
            std::mem::replace(&mut ga.entries, r.entries),
            std::mem::replace(&mut ga.keys, r.keys),
        );
        drop_off_frame(old);
        // Their Develop edits are read again (the versions' entries come back with them).
        ga.edited_pending = Some(edited);
        // Average colours and covers of paths no longer found go (`metas` is the rows').
        ga.trim_to_listing(false);
        ga.keys_loc = r.loc;
        ga.entry_tile.clear();
        ga.list_appended = None;
        ga.selected.retain(|p| ga.by_path.contains_key(p.as_path()));
        if ga
            .focus
            .as_ref()
            .is_some_and(|f| !ga.by_path.contains_key(f.as_path()))
        {
            ga.focus = None;
        }
        ga.wish_key = 0;
        // The order made with the rows, unless the sort or the language changed meanwhile.
        let loc = I18n::global().localizer();
        let fresh = ga.keys_loc.as_ref().is_some_and(|l| Arc::ptr_eq(l, &loc));
        match r.order {
            Some((view, order))
                if fresh && !ga.library.relevance && view == (ga.view.sort, ga.view.desc) =>
            {
                ga.sorted = order;
                ga.dirty = true;
            }
            _ => ga.resort(),
        }
        if first && ga.focus.is_some() {
            ga.reveal = true;
        }
        true
    }

    /// Asks to remove location `id`: the confirmation's question and what follows from it,
    /// or None for a location not (or no longer) listed. Nothing is removed yet.
    pub(super) fn library_ask_remove(&mut self, id: i64) -> Option<(String, String)> {
        let lv = &mut self.library;
        lv.lib.as_ref()?;
        let name = lv.location(id).map(location_name)?;
        lv.remove_ask = Some(id);
        Some((
            t!("library-remove-confirm", location = name),
            t!("library-remove-confirm-detail"),
        ))
    }

    /// The asked removal confirmed ([`Self::library_ask_remove`]): the location's name, or
    /// None when none was asked.
    pub(super) fn library_remove_confirmed(&mut self) -> Option<String> {
        let id = self.library.remove_ask.take()?;
        self.library_forget(id)
    }

    /// Removes a location at once: the catalogue stops what it does for it and drops its
    /// rows in the background, while its row and its images leave the sidebar and the grid
    /// now. Its name as said, or None without a catalogue.
    pub(super) fn library_forget(&mut self, id: i64) -> Option<String> {
        let ga = self;
        let lv = &mut ga.library;
        let l = lv.lib.as_ref()?;
        let gone = lv.location(id).map(|l| l.path.clone());
        let name = lv.location(id).map(location_name).unwrap_or_default();
        l.remove_location(id);
        lv.locations.retain(|l| l.id != id);
        if lv.location == Some(id) {
            lv.location = None;
        }
        if let Some(k) = lv.focus {
            lv.focus = Some(k.min(lv.locations.len() + 1));
        }
        lv.due = Some(Instant::now());
        let Some(root) = gone else { return Some(name) };
        if lv.on && ga.entries.iter().any(|e| e.path.starts_with(&root)) {
            // Entry indices shift: the tiles follow their files until the rebuild.
            let mut next = 0;
            let map: Vec<Option<usize>> = (ga.entries.iter())
                .map(|e| {
                    (!e.path.starts_with(&root)).then(|| {
                        next += 1;
                        next - 1
                    })
                })
                .collect();
            super::remap_tiles(&mut ga.tiles, |i| map.get(i).copied().flatten());
            let (mut entries, mut keys) = (Vec::new(), Vec::new());
            let same = ga.keys.len() == ga.entries.len();
            for (i, e) in std::mem::take(&mut ga.entries).into_iter().enumerate() {
                if !e.path.starts_with(&root) {
                    if same {
                        keys.push(ga.keys[i].clone());
                    }
                    entries.push(e);
                }
            }
            let dropped = ga.by_path.len().saturating_sub(entries.len());
            lv.total = lv.total.saturating_sub(dropped);
            ga.entries = entries;
            ga.reindex();
            ga.trim_to_listing(true);
            ga.keys = if same { keys } else { Vec::new() };
            if !same {
                ga.keys_loc = None;
            }
            ga.entry_tile.clear();
            ga.list_appended = None;
            ga.selected.retain(|p| !p.starts_with(&root));
            if ga.focus.as_ref().is_some_and(|f| f.starts_with(&root)) {
                ga.focus = None;
            }
            ga.wish_key = 0;
            ga.resort();
        }
        Some(name)
    }

    /// Marks changed in the gallery: into the catalogue (while it opens they wait in the
    /// gallery's table, mirrored in full when it is open), and the Library searches again.
    pub(super) fn library_marks_changed(&mut self, changed: &[(PathBuf, Mark)]) {
        let lv = &mut self.library;
        if let Some(l) = &lv.lib {
            l.set_marks(
                changed
                    .iter()
                    .map(|(p, m)| (p.clone(), lib_mark(*m)))
                    .collect(),
            );
        }
        if lv.on && lv.lib.is_some() {
            lv.due = Some(Instant::now() + MARKS_DELAY);
        }
    }
}

impl App {
    /// `L`: into the Library, or out of it (back to the viewer's image, else the folder the
    /// gallery showed before).
    pub(super) fn library_toggle(&mut self) {
        if !(self.gallery.active && self.gallery.library.on) {
            self.library_enter();
            return;
        }
        if self.current.is_some() {
            self.gallery_leave();
        } else if let Some(d) = self.gallery.library.back.clone() {
            self.gallery_enter_at(Some(d), None);
        } else {
            let now = self.now();
            self.gallery_close(now, None);
        }
    }

    /// Shows the Library and leaves it shown if it is (`--library`, the launcher menu).
    pub(in crate::app) fn library_from_desktop(&mut self) {
        if !(self.gallery.active && self.gallery.library.on) {
            self.library_enter();
        }
    }

    /// The Library searching for `query`, as typed in its search bar (the desktop search's
    /// "more results", `specular --search QUERY`).
    pub(in crate::app) fn library_search_from_desktop(&mut self, query: String) {
        self.library_from_desktop();
        let ga = &mut self.gallery;
        ga.filter.text = query.lines().next().unwrap_or_default().to_owned();
        ga.filter_open = true;
        ga.dirty = true;
        ga.relayout = true;
        self.redraw = true;
    }

    /// The folder the gallery shows (not the Library's search), when it is on screen.
    pub(in crate::app) fn gallery_folder(&self) -> Option<&Path> {
        let ga = &self.gallery;
        (ga.active && !ga.library.on)
            .then_some(ga.dir.as_deref())
            .flatten()
    }

    /// Whether the Library has folders, once its catalogue is open and searched; `None`
    /// before (or when it could not be opened).
    pub(in crate::app) fn library_has_folders(&self) -> Option<bool> {
        let lv = &self.gallery.library;
        (lv.lib.is_some() && lv.searched).then_some(!lv.locations.is_empty())
    }

    /// Whether the Library's indexer is walking or matching (the app gates).
    pub(in crate::app) fn library_walking(&self) -> bool {
        let lv = &self.gallery.library;
        lv.lib.as_ref().is_some_and(|l| l.progress().busy())
    }

    /// The Library could not be opened (the start screen stops waiting on it).
    pub(in crate::app) fn library_failed(&self) -> bool {
        self.gallery.library.error.is_some()
    }

    /// The Library's state for the `ui-state` test command (debug builds).
    #[cfg(debug_assertions)]
    pub(in crate::app) fn library_test_state(&self) -> serde_json::Value {
        let lv = &self.gallery.library;
        let p = lv.lib.as_ref().map(|l| l.progress());
        let locations: Vec<serde_json::Value> = lv
            .locations
            .iter()
            .map(|l| {
                serde_json::json!({
                    "path": l.path.to_string_lossy(),
                    "files": l.files,
                    "pending": l.pending,
                    "offline": l.offline,
                    "scanned": l.last_scan.is_some(),
                })
            })
            .collect();
        serde_json::json!({
            "on": lv.on,
            "open": lv.lib.is_some(),
            "error": lv.error,
            "searched": lv.searched,
            "searching": lv.searching || lv.due.is_some(),
            "search_error": lv.search_error,
            "total": lv.total,
            "query": self.gallery.filter.text,
            "locations": locations,
            "busy": p.as_ref().is_some_and(|p| p.busy()),
            "phase": p.as_ref().map(|p| format!("{:?}", p.phase).to_lowercase()),
            "walked": p.as_ref().map_or(0, |p| p.walked),
            "to_read": p.as_ref().map_or(0, |p| p.to_read),
            "read": p.as_ref().map_or(0, |p| p.read),
            "halted": p.as_ref().is_some_and(|p| p.halt.is_some()),
        })
    }

    /// Shows the Library and adds `dir` to it ("Your pictures" on the start screen, once the
    /// user said yes).
    pub(in crate::app) fn library_enter_adding(&mut self, dir: PathBuf) {
        self.library_enter();
        self.gallery.library_add_checked(dir, Instant::now());
        self.library_ask_home();
    }

    /// Asks before adding a folder that holds the whole home folder: Pictures instead (when
    /// there is one), the folder anyway, or Cancel.
    fn library_ask_home(&mut self) {
        let lv = &mut self.gallery.library;
        if !std::mem::take(&mut lv.home_due) {
            return;
        }
        let Some(ask) = lv.home_ask.clone() else {
            return;
        };
        let title = t!("library-home-confirm", folder = folder_name(&ask.folder));
        let mut detail = t!("library-home-confirm-detail");
        let keep = t!("library-home-confirm-keep");
        if ask.pictures.is_some() {
            detail.push('\n');
            detail.push_str(&t!("library-home-confirm-pictures"));
            self.chrome.confirm_choice(
                title,
                detail,
                t!("library-home-confirm-use-pictures"),
                LIBRARY_HOME_PICTURES_ITEM,
                keep,
                LIBRARY_HOME_KEEP_ITEM,
            );
        } else {
            self.chrome
                .confirm_destructive(title, detail, keep, LIBRARY_HOME_KEEP_ITEM);
        }
        self.redraw = true;
    }

    /// The answer to [`Self::library_ask_home`]: Pictures instead (`pictures`), or the
    /// folder asked about.
    pub(in crate::app) fn library_home_answer(&mut self, pictures: bool) {
        let Some(ask) = self.gallery.library.home_ask.take() else {
            return;
        };
        let p = match (pictures, ask.pictures) {
            (true, Some(p)) => p,
            _ => ask.folder,
        };
        self.gallery.library_add_location(p, Instant::now());
        self.redraw = true;
    }

    /// Opens the catalogue (once per run) on a worker: the Library says it is opening until
    /// then. Screenshots open one only when `SPECULAR_LIBRARY_DB` names it, never the user's.
    fn library_open(&mut self) {
        let lv = &self.gallery.library;
        if lv.lib.is_some() || lv.error.is_some() || lv.opening() {
            return;
        }
        let env = std::env::var_os("SPECULAR_LIBRARY_DB").filter(|v| !v.is_empty());
        if self.args.screenshot.is_some() && env.is_none() {
            self.gallery.library.error = Some("SPECULAR_LIBRARY_DB".into());
            return;
        }
        let path = lib::default_path(&specular_settings::Locations::detect().data_dir);
        let mut cfg = lib::Config::new(path);
        let registry = self.loader.registry();
        let r = registry.clone();
        cfg.fold = model::fold;
        cfg.fold_id = "search";
        cfg.is_image = Arc::new(move |p: &Path| r.supports_file(p));
        cfg.prober = Arc::new(lib::DecodeProber {
            registry: registry.clone(),
        });
        cfg.signer = Some(Arc::new(lib::DecodeSigner { registry }));
        cfg.cache = self.thumb_cache();
        cfg.cache_flags = self.gallery.engine.as_ref().map_or(0, |e| e.config().flags);
        cfg.on_thread_start = Some(specular_platform::lower_thread_priority);
        cfg.wake = self.loader.waker();
        cfg.skip = self.gallery.library.skip.clone();
        cfg.prefer_foreign = self.gallery.library.prefer_sidecars.clone();
        cfg.edit_import = Some(crate::app::develop::store::edit_import());
        self.gallery.library_begin_open(cfg);
    }

    /// Shows the Library in the gallery.
    pub(super) fn library_enter(&mut self) {
        self.gallery_prepare();
        self.library_open();
        let focus = self.current.and_then(|i| self.entries.get(i)).cloned();
        let ga = &mut self.gallery;
        if !ga.library.on {
            let back = ga.dir.clone();
            if let Some(old) = ga.dir.take() {
                ga.positions.insert(old, (ga.scroll, ga.focus.clone()));
            }
            if let Some(t) = ga.list_token.take() {
                t.cancel();
            }
            ga.generation += 1;
            ga.edited.clear();
            ga.versions.clear();
            ga.listing = false;
            ga.watch = None;
            ga.refresh_seen = None;
            ga.announce = None;
            ga.entries.clear();
            ga.by_path.clear();
            ga.keys.clear();
            ga.sorted.clear();
            ga.tiles.clear();
            ga.entry_tile.clear();
            ga.layout = Layout::default();
            ga.selected.clear();
            ga.expanded.clear();
            ga.compare = None;
            ga.quick = None;
            ga.info = None;
            ga.hover = None;
            ga.press = None;
            ga.list_error = None;
            ga.scroll = 0.0;
            ga.scroll_target = 0.0;
            // Not a movement (it would hold the indexer as browsing).
            ga.vel_mark = None;
            ga.focus = None;
            ga.anchor = None;
            ga.wish_key = 0;
            ga.filter = Filter {
                scope: specular_query::Scope::Library,
                ..Filter::default()
            };
            ga.facets.close();
            ga.facets.facets.clear();
            ga.facets.generation += 1;
            let lv = &mut ga.library;
            lv.on = true;
            lv.back = back;
            lv.searched = false;
            lv.search_error = None;
            lv.asked = None;
            lv.hash = 0;
            lv.focus = None;
            lv.panel = true;
            ga.filter_open = true;
            ga.filter_focus = false;
            ga.dirty = true;
            ga.relayout = true;
        }
        self.gallery_enter_at(None, focus);
        self.library_tick();
    }

    /// Once per event-loop turn while the gallery may show the Library
    /// ([`Gallery::library_poll`]).
    pub(super) fn library_tick(&mut self) {
        let wake = self.loader.waker();
        self.redraw |= self.gallery.library_poll(&wake);
        self.library_ask_home();
        self.local_tick();
    }

    /// A search came back ([`Gallery::library_results`]).
    pub(super) fn library_results(&mut self, r: Results) {
        self.redraw |= self.gallery.library_results(r);
        if let Some(photos) = self.gallery.edited_pending.take() {
            let g = self.gallery.generation;
            self.gallery_edited_listed(g, photos);
        }
    }

    /// Marks changed in the gallery ([`Gallery::library_marks_changed`]).
    pub(super) fn library_marks_changed(&mut self, changed: &[(PathBuf, Mark)]) {
        self.gallery.library_marks_changed(changed);
    }

    /// The keyword panel wrote keywords: a Library search that may ask about them runs
    /// again (after the catalogue has them).
    pub(in crate::app) fn gallery_keywords_written(&mut self) {
        let lv = &mut self.gallery.library;
        if lv.on && lv.lib.is_some() {
            lv.due = Some(Instant::now() + MARKS_DELAY);
        }
    }

    /// The keyword panel opened: it takes the trailing dock from the facets panel.
    pub(in crate::app) fn gallery_keywords_opened(&mut self) {
        let ga = &mut self.gallery;
        if ga.facets.open {
            ga.facets.close();
        }
        ga.relayout = true;
        ga.dirty = true;
        ga.keywords.catalogue_asked = false;
    }

    /// The keyword panel closed: the grid takes its room back.
    pub(in crate::app) fn gallery_keywords_closed(&mut self) {
        self.gallery.relayout = true;
        self.gallery.dirty = true;
    }

    /// F5 / Reload in the Library: walk every location again (while the catalogue opens,
    /// nothing: opening walks them all).
    pub(super) fn library_rescan(&mut self) {
        let lv = &mut self.gallery.library;
        let Some(l) = &lv.lib else { return };
        l.rescan(lv.location);
        lv.due = Some(Instant::now());
    }

    /// "Add location…": a folder dialog (off the event loop).
    fn library_add(&mut self) {
        let (tx, rx) = crossbeam_channel::bounded::<PathBuf>(1);
        self.loader.background("dlg-library", move |l| {
            if let Some(p) = rfd::FileDialog::new().pick_folder() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
        self.gallery.library.added_rx = Some(rx);
    }

    /// The foot's Cancel: stops the indexing (the catalogue keeps what it has; see
    /// [`Library::cancel`]).
    fn library_stop(&mut self) {
        let lv = &mut self.gallery.library;
        let Some(l) = &lv.lib else { return };
        l.cancel();
        // Shown at once (the indexer confirms on its next turn).
        lv.progress.stopped = true;
        lv.progress.phase = Phase::Idle;
        self.gallery.announce = Some(t!("library-stopped"));
    }

    /// The foot's Resume: walks every location again and reads what is pending.
    fn library_resume(&mut self) {
        let lv = &mut self.gallery.library;
        let Some(l) = &lv.lib else { return };
        l.rescan(None);
        lv.progress.stopped = false;
        lv.due = Some(Instant::now());
    }

    /// The foot's Retry: starts the indexer again after it stopped by itself.
    fn library_retry(&mut self) {
        let lv = &mut self.gallery.library;
        let Some(l) = &lv.lib else { return };
        l.retry();
        // Shown at once (the indexer confirms on its next turn).
        lv.progress.halt = None;
        lv.due = Some(Instant::now());
    }

    /// Removing a location (its X, Delete, an assistive technology's action) asks first:
    /// nothing is removed until the sheet is accepted ([`Self::library_remove_confirmed`]).
    fn library_remove(&mut self, id: i64) {
        let Some((title, detail)) = self.gallery.library_ask_remove(id) else {
            return;
        };
        self.chrome.confirm_destructive(
            title,
            detail,
            t!("library-remove-confirm-ok"),
            LIBRARY_REMOVE_ITEM,
        );
        self.redraw = true;
    }

    /// The removal was confirmed: the location goes at once (row, images, and the walk).
    pub(in crate::app) fn library_remove_confirmed(&mut self) {
        let Some(name) = self.gallery.library_remove_confirmed() else {
            return;
        };
        self.gallery.announce = Some(t!("library-a11y-removed", location = name));
        self.redraw = true;
    }

    /// A click (or Enter, or an assistive technology's) on a sidebar place.
    pub(super) fn library_hit(&mut self, s: Side) {
        let ga = &mut self.gallery;
        match s {
            Side::Toggle => {
                let show = !ga.library_sidebar_shown();
                ga.library.panel = show;
                if show && ga.facets.open {
                    ga.facets.close();
                }
                if !show {
                    ga.library.focus = None;
                }
                ga.relayout = true;
            }
            Side::Panel => {}
            Side::All => {
                ga.library.location = None;
                ga.library.albums.open = None;
            }
            Side::Location(id) => {
                ga.library.location = Some(id);
                ga.library.albums.open = None;
            }
            Side::AlbumsTitle
            | Side::Group(_)
            | Side::Album(_)
            | Side::AlbumEdit(_)
            | Side::AlbumDelete(_)
            | Side::GroupEdit(_)
            | Side::GroupDelete(_)
            | Side::NewAlbum
            | Side::NewSmart
            | Side::Bar(_) => self.albums_hit(s),
            Side::Remove(id) => self.library_remove(id),
            Side::Add => self.library_add(),
            Side::Stop => self.library_stop(),
            Side::Resume => self.library_resume(),
            Side::Retry => self.library_retry(),
            Side::Diagnostics => self.diag_open(),
            Side::Watches => self.chrome.show_help_topic(WATCHES_TOPIC),
            Side::LocalReview => self.local_ask_next(),
        }
        if let Some(k) = self.gallery.library.items().iter().position(|&i| i == s)
            && self.gallery.library.focus.is_some()
        {
            self.gallery.library.focus = Some(k);
        }
        self.redraw = true;
    }

    /// `--show library…` for screenshots and live checks: `SPECULAR_LIBRARY_ADD` (a folder)
    /// is added as a location first (only with `SPECULAR_LIBRARY_DB`, never the user's).
    pub(super) fn library_show(&mut self, rest: &str) -> bool {
        self.library_enter();
        self.gallery.transition = None;
        if std::env::var_os("SPECULAR_LIBRARY_DB").is_some_and(|v| !v.is_empty())
            && let Some(p) = std::env::var_os("SPECULAR_LIBRARY_ADD").filter(|v| !v.is_empty())
        {
            // Added once the catalogue is open (it opens on a worker).
            self.gallery
                .library_add_location(PathBuf::from(p), Instant::now());
        }
        let ga = &mut self.gallery;
        match rest {
            "" => {}
            "-facets" => ga.facets_show(true, false),
            "-sidebar-focus" => ga.library.focus = Some(0),
            "-no-sidebar" => ga.library.panel = false,
            "-stopped" => ga.library.show_stop = true,
            // The gallery's scripted scroll (scrollbench.rs) over the Library's grid.
            "-scroll-bench" => ga.sbench = Some(super::scrollbench::ScrollBench::from_env()),
            other => {
                if let Some(q) = other.strip_prefix("-query=") {
                    ga.filter.text = q.to_owned();
                } else {
                    return false;
                }
            }
        }
        ga.relayout = true;
        self.redraw = true;
        true
    }

    /// Keys while the sidebar has the focus. True when used.
    pub(super) fn library_key(&mut self, n: NamedKey, shift: bool) -> bool {
        let Some(k) = self.gallery.library.focus else {
            return false;
        };
        let items = self.gallery.library.items();
        let last = items.len() - 1;
        let k = k.min(last);
        let lv = &mut self.gallery.library;
        match n {
            NamedKey::ArrowDown => lv.focus = Some((k + 1).min(last)),
            NamedKey::ArrowUp => lv.focus = Some(k.saturating_sub(1)),
            NamedKey::Home => lv.focus = Some(0),
            NamedKey::End => lv.focus = Some(last),
            NamedKey::Enter | NamedKey::Space => self.library_hit(items[k]),
            NamedKey::Delete | NamedKey::Backspace => match items[k] {
                Side::Location(id) => self.library_remove(id),
                Side::Album(id) => self.albums_hit(Side::AlbumDelete(id)),
                Side::Group(g) => self.albums_hit(Side::GroupDelete(g)),
                _ => {}
            },
            // F2: rename an album or group (a smart album's rules too).
            NamedKey::F(2) => match items[k] {
                Side::Album(id) => self.albums_hit(Side::AlbumEdit(id)),
                Side::Group(g) => self.albums_hit(Side::GroupEdit(g)),
                _ => {}
            },
            NamedKey::Escape => {
                lv.focus = None;
                self.gallery.go(filterbar::Stop::Grid);
            }
            NamedKey::Tab => self.gallery.tab(shift),
            _ => return false,
        }
        self.redraw = true;
        true
    }

    /// AccessKit: an action on a sidebar node; false when not ours.
    pub(super) fn library_a11y_action(&mut self, target: u64, click: bool) -> bool {
        if self.gallery.library.on && target == ID_REVIEW {
            if click {
                self.local_ask_next();
            }
            return true;
        }
        if !self.gallery.library.on || !(ID_ITEMS..A11Y_LIBRARY + (1 << 32)).contains(&target) {
            return false;
        }
        if self.albums_a11y_action(target, click) {
            self.redraw = true;
            return true;
        }
        let k = ((target - ID_ITEMS) / 2) as usize;
        let remove = (target - ID_ITEMS) % 2 == 1;
        let Some(&(side, _)) = self.gallery.library.rows.get(k) else {
            return false;
        };
        match (click, remove, side) {
            (true, true, Side::Location(id)) => self.library_remove(id),
            (true, false, s) => {
                self.gallery.library.focus = Some(k);
                self.library_hit(s);
            }
            (false, _, _) => {
                self.gallery.go(filterbar::Stop::Grid);
                self.gallery.library.focus = Some(k);
            }
            _ => {}
        }
        self.redraw = true;
        true
    }
}

fn lib_mark(m: Mark) -> lib::Mark {
    lib::Mark {
        rating: m.rating,
        label: m.label,
        flag: m.flag,
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;

    fn location(id: i64, files: u64, offline: bool) -> Location {
        Location {
            id,
            path: PathBuf::from(format!("/photos/loc{id}")),
            offline,
            files,
            pending: 0,
            last_scan: None,
        }
    }

    #[test]
    fn items_list_all_locations_then_add() {
        let lv = LibraryView {
            locations: vec![location(3, 10, false), location(7, 5, true)],
            ..LibraryView::default()
        };
        assert_eq!(
            lv.items(),
            vec![Side::All, Side::Location(3), Side::Location(7), Side::Add]
        );
    }

    /// A catalogue that failed to open takes no locations, so none is offered.
    #[test]
    fn no_add_location_when_the_catalogue_failed() {
        let lv = LibraryView {
            locations: vec![location(3, 10, false)],
            error: Some("database disk image is malformed".into()),
            ..LibraryView::default()
        };
        assert_eq!(lv.items(), vec![Side::All, Side::Location(3)]);
    }

    /// The foot's button: Cancel while indexing, Resume (and "Indexing stopped · N to
    /// index") once stopped, none when idle, opening or failed; the button is the last
    /// keyboard item.
    #[test]
    fn foot_offers_cancel_while_indexing_then_resume() {
        let root = std::env::temp_dir().join(format!("specular-libfoot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut cfg = lib::Config::new(root.join("library.sqlite3"));
        cfg.watch = false;
        let l = Arc::new(Library::open(cfg).unwrap());
        let mut lv = LibraryView {
            locations: vec![location(3, 10, false)],
            ..LibraryView::default()
        };
        // No catalogue yet: nothing to stop.
        lv.progress.phase = Phase::Walking;
        assert_eq!(lv.foot_button(), None);
        lv.lib = Some(l.clone());
        assert_eq!(lv.foot_button(), Some(Side::Stop));
        assert_eq!(lv.items().last(), Some(&Side::Stop));
        assert_eq!(lv.stopped_text(), None);
        lv.progress.phase = Phase::Idle;
        assert_eq!(lv.foot_button(), None);
        assert_eq!(lv.items().last(), Some(&Side::NewSmart));
        // Stopped with files left: Resume, and the line says how many wait.
        lv.progress.stopped = true;
        lv.locations[0].pending = 7;
        assert_eq!(lv.foot_button(), Some(Side::Resume));
        assert!(lv.stopped_text().is_some_and(|s| s.contains('7')));
        lv.locations[0].pending = 0;
        assert_eq!(lv.stopped_text(), Some(t!("library-stopped")));
        lv.error = Some("disk full".into());
        assert_eq!(lv.foot_button(), None);
        drop(lv);
        drop(l);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn test_library(name: &str) -> (PathBuf, Arc<Library>) {
        let root = std::env::temp_dir().join(format!("specular-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut cfg = lib::Config::new(root.join("library.sqlite3"));
        cfg.watch = false;
        (root.clone(), Arc::new(Library::open(cfg).unwrap()))
    }

    #[test]
    fn adding_the_home_folder_or_above_asks_and_offers_pictures() {
        let root = std::env::temp_dir().join(format!("specular-home-ask-{}", std::process::id()));
        let home = root.join("home/me");
        let pictures = home.join("Pictures");
        std::fs::create_dir_all(&pictures).unwrap();
        let asked = |f: &Path, p: &Path| home_ask(f, &home, p);
        let offer = Some(pictures.clone());
        assert_eq!(asked(&home, &pictures).unwrap().pictures, offer);
        // A folder holding the home folder asks too.
        assert_eq!(
            asked(&root.join("home"), &pictures).unwrap().pictures,
            offer
        );
        assert_eq!(asked(&root, &pictures).unwrap().folder, root);
        // Pictures itself, a folder beside home or inside it: added without a word.
        for f in [pictures.clone(), root.join("home/other"), home.join("Work")] {
            assert_eq!(asked(&f, &pictures), None, "{}", f.display());
        }
        // No Pictures folder of its own (the platform falls back to home): nothing offered.
        assert_eq!(asked(&home, &home).unwrap().pictures, None);
        assert_eq!(asked(&home, &home.join("Missing")).unwrap().pictures, None);
        // An unknown home asks nothing.
        assert_eq!(home_ask(&home, Path::new(""), &pictures), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_skip_settings_default_to_the_librarys_own_list() {
        let s = specular_settings::Settings::default();
        assert_eq!(skip_of(&s), lib::Skip::default());
        assert_eq!(LibraryView::default().skip, lib::Skip::default());
    }

    #[test]
    fn out_of_watches_adds_an_explanation_to_the_sidebar() {
        let mut lv = LibraryView::default();
        assert!(!lv.items().contains(&Side::Watches));
        lv.progress.out_of_watches = true;
        let items = lv.items();
        let add = items.iter().position(|&s| s == Side::Add).unwrap();
        assert_eq!(items[add + 1], Side::Watches);
        assert!(!t!("library-unwatched-detail", minutes = UNWATCHED_MINUTES).contains("{"));
        assert_eq!(UNWATCHED_MINUTES, 15);
    }

    fn image(path: &str) -> Entry {
        let path = PathBuf::from(path);
        Entry {
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            path,
            kind: EntryKind::Image,
            len: 1,
            modified: None,
            hidden: false,
            taken: None,
            dims: Some((4, 3)),
        }
    }

    /// The walking bar's stripe moves along the track, stays on it, and stands still
    /// (the whole track) under reduce-motion.
    #[test]
    fn the_walking_stripe_moves_unless_motion_is_reduced() {
        let mut starts = Vec::new();
        for k in 0..28 {
            let (a, w) = walking_stripe(f64::from(k) * 0.05, false);
            assert!((0.0..=1.0).contains(&a) && w >= 0.0 && a + w <= 1.0 + 1e-6);
            starts.push(a);
        }
        assert!(starts.windows(2).any(|p| p[1] > p[0]), "it moves");
        assert_eq!(walking_stripe(0.3, true), walking_stripe(9.7, true));
        assert_eq!(walking_stripe(0.3, true), (0.0, 1.0));
    }

    /// The first rows come at once, a walk's every second, a big grid's every 3 s.
    #[test]
    fn rows_refresh_soon_while_the_grid_is_empty_or_walked() {
        assert_eq!(refresh_every(0, true), REFRESH_FIRST);
        assert_eq!(refresh_every(0, false), REFRESH_FIRST);
        assert_eq!(refresh_every(500, true), REFRESH_WALKING);
        assert_eq!(refresh_every(500, false), REFRESH);
        assert_eq!(refresh_every(SMALL_GRID, true), REFRESH);
        assert_eq!(refresh_every(LARGE_GRID, true), REFRESH);
        // A million rows: every 30 s, not every 3.
        assert_eq!(refresh_every(1_000_000, false), REFRESH * 10);
    }

    /// Removing a location takes its row and its images away at once (the catalogue drops
    /// them in the background).
    #[test]
    fn a_removed_location_leaves_the_sidebar_and_grid_at_once() {
        let (root, l) = test_library("libforget");
        let mut g = Gallery {
            active: true,
            ..Gallery::default()
        };
        g.library.on = true;
        g.library.lib = Some(l.clone());
        g.library.locations = vec![location(1, 2, false), location(2, 1, false)];
        g.library.location = Some(1);
        g.add_entries(vec![
            image("/photos/loc1/a.jpg"),
            image("/photos/loc2/b.jpg"),
            image("/photos/loc1/sub/c.jpg"),
        ]);
        g.library.total = 3;
        g.resort();
        g.focus = Some(PathBuf::from("/photos/loc1/a.jpg"));
        // Its X (or Delete) only asks: nothing is removed, here or in the catalogue.
        let (title, detail) = g.library_ask_remove(1).expect("asked");
        assert_eq!(
            title,
            t!("library-remove-confirm", location = "loc1"),
            "the folder is named"
        );
        assert_eq!(detail, t!("library-remove-confirm-detail"));
        assert!(detail.contains("stay on disk"), "{detail}");
        assert_eq!(g.library.locations.len(), 2);
        assert_eq!(g.entries.len(), 3);
        assert_eq!(g.library.location, Some(1));
        assert_eq!(g.library_ask_remove(9), None, "not listed: nothing asked");
        // Cancel or Esc send nothing: asked again later, still nothing is gone.
        assert!(g.library_ask_remove(1).is_some());
        assert_eq!(g.entries.len(), 3);
        // Confirmed: gone in the same call.
        assert_eq!(g.library_remove_confirmed().as_deref(), Some("loc1"));
        assert_eq!(
            g.library_remove_confirmed(),
            None,
            "asked once, removed once"
        );
        assert_eq!(g.library.locations.len(), 1);
        assert_eq!(g.library.location, None);
        let paths: Vec<_> = g.entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(paths, [PathBuf::from("/photos/loc2/b.jpg")]);
        assert_eq!(g.by_path.len(), 1);
        assert_eq!(g.sorted.len(), 1);
        assert_eq!(g.library.total, 1);
        assert_eq!(g.focus, None);
        assert!(g.library.due.is_some(), "searched again");
        drop(g);
        drop(l);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Fewer rows than tiles (a removed location, a narrower search): the next rebuild reads
    /// the old tiles for its anchor, and they must still point at entries (it crashed with
    /// "the len is 333 but the index is 3651").
    #[test]
    fn fewer_rows_keep_the_tiles_inside_the_entries() {
        let (root, l) = test_library("libshrink");
        let mut g = Gallery {
            active: true,
            vw: 800.0,
            vh: 600.0,
            ..Gallery::default()
        };
        g.library.on = true;
        g.library.lib = Some(l.clone());
        g.library.locations = vec![location(1, 400, false), location(2, 3, false)];
        let mut rows: Vec<Entry> = (0..400)
            .map(|i| image(&format!("/photos/loc1/{i:04}.jpg")))
            .collect();
        rows.extend((0..3).map(|i| image(&format!("/photos/loc2/{i}.jpg"))));
        g.add_entries(rows);
        g.library.total = 403;
        g.resort();
        g.rebuild();
        assert_eq!(g.tiles.len(), 403);
        // Scrolled to the end: the anchor is one of the last tiles.
        g.scroll = 1e9;
        assert!(g.anchor_path().is_some());
        assert!(g.library_ask_remove(1).is_some());
        assert_eq!(g.library_remove_confirmed().as_deref(), Some("loc1"));
        assert_eq!(g.entries.len(), 3);
        assert!(g.tiles.iter().all(|t| t.entry < g.entries.len()));
        g.rebuild();
        assert_eq!(g.tiles.len(), 3);
        // Removed files (moved to the trash, say): kept tiles follow their entries.
        g.remove_entries(&[PathBuf::from("/photos/loc2/0.jpg")]);
        assert!(g.tiles.iter().all(|t| t.entry < g.entries.len()));
        let shown: Vec<_> = g.tile_paths().cloned().collect();
        assert!(!shown.contains(&PathBuf::from("/photos/loc2/0.jpg")));
        assert_eq!(shown.len(), 2);
        g.rebuild();
        drop(g);
        drop(l);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Only the grid moving holds the indexer: not a fling flag left set by a grid that
    /// went empty (it held the walk for good), nor the scroll the program sets.
    #[test]
    fn a_stale_fling_or_a_relayout_does_not_hold_the_indexer() {
        let (root, l) = test_library("libhold");
        let mut g = Gallery {
            active: true,
            ..Gallery::default()
        };
        g.library.on = true;
        g.library.lib = Some(l.clone());
        g.wish_fast = true;
        let wake: Wake = Arc::new(|| {});
        g.library_poll(&wake);
        assert!(!l.held(), "a stale wish_fast held the indexer");

        // A relayout or clamp moves the scroll: no movement.
        let t0 = Instant::now();
        g.track_velocity(t0);
        let was = g.scroll;
        g.scroll += 300.0;
        g.unmoved(was);
        g.track_velocity(t0 + Duration::from_millis(16));
        assert_eq!(g.moved_at, None);
        g.library_poll(&wake);
        assert!(!l.held());
        // A scroll by the person does.
        g.scroll += 40.0;
        g.track_velocity(t0 + Duration::from_millis(32));
        assert!(g.moved_at.is_some());
        g.library_poll(&wake);
        assert!(l.held(), "scrolling holds");
        drop(g);
        drop(l);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_states() {
        let mut g = Gallery::default();
        g.library.on = true;
        // Nothing said before the first search comes back.
        assert_eq!(g.library_empty_text(), None);
        g.library.searched = true;
        assert_eq!(g.library_empty_text(), Some(t!("library-empty")));
        g.library.locations = vec![location(1, 4, true)];
        assert_eq!(g.library_empty_text(), Some(t!("library-empty-offline")));
        g.library.locations = vec![location(1, 0, false)];
        g.library.progress.phase = Phase::Walking;
        assert_eq!(g.library_empty_text(), Some(t!("library-indexing")));
        // Walking a known folder: where, and how many so far.
        g.library.progress.location = Some(PathBuf::from("/photos/loc1"));
        g.library.progress.walked = 42;
        let s = g.library_empty_text().unwrap();
        assert_eq!(s, t!("library-indexing-in", location = "loc1", count = 42));
        assert!(s.contains("loc1") && s.contains("42"), "{s}");
        g.library.progress.location = None;
        // Stopped before anything was read: neither "no images" nor still looking.
        g.library.progress.phase = Phase::Idle;
        g.library.progress.stopped = true;
        assert_eq!(g.library_empty_text(), Some(t!("library-empty-stopped")));
        g.library.progress.stopped = false;
        g.library.progress.phase = Phase::Walking;
        g.filter.text = "camera:nikon".into();
        assert_eq!(g.library_empty_text(), Some(t!("gallery-empty-filter")));
        g.library.error = Some("disk full".into());
        assert!(
            g.library_empty_text()
                .is_some_and(|s| s.contains("disk full"))
        );
    }

    #[test]
    fn progress_in_words() {
        let mut p = Progress::default();
        assert_eq!(progress_text(&p, 0), None);
        p.phase = Phase::Reading;
        p.to_read = 400;
        p.read = 120;
        assert!(progress_text(&p, p.read).is_some_and(|s| s.contains("120") && s.contains("400")));
        p.held = true;
        assert_eq!(progress_text(&p, p.read), Some(t!("library-held")));
    }

    #[test]
    fn progress_node_is_polite_and_said_at_quarters() {
        use accesskit::{Live, NodeId, Role, TreeUpdate};
        let mut g = Gallery::default();
        g.library.on = true;
        g.library.progress = Progress {
            phase: Phase::Reading,
            to_read: 400,
            read: 130,
            ..Progress::default()
        };
        let mut tree = TreeUpdate {
            nodes: vec![(NodeId(1), accesskit::Node::new(Role::Window))],
            tree: None,
            tree_id: accesskit::TreeId::ROOT,
            focus: NodeId(1),
        };
        let bounds = |_r: Rect| accesskit::Rect::default();
        assert!(!g.library_a11y(&mut tree, &bounds));
        let (_, n) = tree
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId(ID_PROGRESS))
            .expect("progress node");
        assert_eq!(n.role(), Role::ProgressIndicator);
        assert_eq!(n.live(), Some(Live::Polite));
        // Said at the quarter (100), its value the exact count.
        assert!(
            n.label()
                .is_some_and(|l| l.contains("100") && !l.contains("130"))
        );
        assert!(n.value().is_some_and(|v| v.contains("130")));
        assert_eq!(n.numeric_value(), Some(130.0));
    }

    /// The catalogue never holds up a frame: another writer holds it for [`HOLD`] (a
    /// `BEGIN IMMEDIATE` on its own connection, as a big indexing transaction or another
    /// Specular would) while this thread, marked as the UI thread, does what the frame does
    /// with the Library and the Local AI page: opens the catalogue (which must migrate it, a
    /// write), polls it every turn, sets marks, adds a location, asks for a search, takes the
    /// results, builds the AI page and removes AI features. Every step is timed.
    ///
    /// The bar is one frame (16 ms at 60 Hz); the assertion allows [`LIMIT`] (50 ms), 34 ms
    /// of margin for a loaded CI machine or a debug build preempted mid-step (not judged when
    /// the machine is overloaded, see `specular_log::timing`; 500 ms always is). A step that
    /// waited for the catalogue would take the whole 2 s hold (40x the limit), so the margin
    /// cannot hide one; and in debug builds the UI-thread guard panics at any catalogue call
    /// here anyway.
    #[test]
    fn a_held_catalogue_never_holds_up_a_frame() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        use crate::app::ai;
        use specular_ai::{Manifest, Pack};
        use specular_widgets::{PanelEvent, ai_keys};

        const FRAME: Duration = Duration::from_millis(16);
        const LIMIT: Duration = Duration::from_millis(50);
        const HOLD: Duration = Duration::from_secs(2);

        let root = std::env::temp_dir().join(format!("specular-libnoblock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let photos = root.join("photos");
        std::fs::create_dir_all(&photos).unwrap();
        let db = root.join("library.sqlite3");
        // A catalogue not made yet (WAL, version 0): opening it migrates, which writes.
        {
            let c = rusqlite::Connection::open(&db).unwrap();
            c.pragma_update(None, "journal_mode", "WAL").unwrap();
        }
        // Another writer holds it.
        let (locked_tx, locked_rx) = std::sync::mpsc::channel();
        let locker = {
            let db = db.clone();
            std::thread::spawn(move || {
                let c = rusqlite::Connection::open(&db).unwrap();
                c.execute_batch("BEGIN IMMEDIATE; CREATE TABLE held(x);")
                    .unwrap();
                locked_tx.send(Instant::now()).unwrap();
                std::thread::sleep(HOLD);
                c.execute_batch("ROLLBACK").unwrap();
            })
        };
        let locked_at = locked_rx.recv().unwrap();

        let woken = Arc::new(AtomicUsize::new(0));
        let w = woken.clone();
        let wake: Wake = Arc::new(move || {
            w.fetch_add(1, Ordering::Relaxed);
        });
        let mut cfg = lib::Config::new(db.clone());
        cfg.watch = false;
        cfg.wake = wake.clone();
        let mut g = Gallery {
            active: true,
            ..Gallery::default()
        };
        g.library.on = true;
        let data = root.join("data");
        let make = {
            let (data, db, wake) = (data.clone(), db.clone(), wake.clone());
            move || Pack::new(Manifest::load(), &data, Some(db), Default::default(), wake)
        };

        let ui = specular_platform::UiThreadScope::enter();
        let mut steps: Vec<(&'static str, Duration)> = Vec::new();
        macro_rules! step {
            ($name:expr, $e:expr) => {{
                let t = Instant::now();
                let r = $e;
                steps.push(($name, t.elapsed()));
                r
            }};
        }
        // Into the Library: the catalogue opens on its worker, once.
        assert!(step!("open", g.library_begin_open(cfg.clone())));
        assert!(!step!("open again", g.library_begin_open(cfg)));
        step!("poll", g.library_poll(&wake));
        assert!(g.library.opening());
        assert!(g.library.pending());
        assert_eq!(g.library_empty_text(), Some(t!("library-opening")));
        // Marks and a location while it opens.
        let photo = photos.join("a.jpg");
        let mark = Mark {
            rating: 4,
            ..Mark::default()
        };
        g.marks.marks.insert(photo.clone(), mark);
        step!("marks", g.library_marks_changed(&[(photo.clone(), mark)]));
        step!(
            "add location",
            g.library_add_location(photos.clone(), Instant::now())
        );
        g.filter.text = "rating:4".into();
        step!("poll", g.library_poll(&wake));
        // No timer while it opens (its worker wakes the loop): the loop sleeps.
        assert_eq!(g.library_wake(), None);
        // The Local AI page and its removals, the pack made by the page's worker.
        let mut pages = ai::Pages::default();
        assert!(step!(
            "ai page",
            pages.request_with(make, wake.clone(), ai::Ctx::default())
        ));
        let own = Pack::new(
            Manifest::load(),
            &root.join("data-own"),
            Some(db.clone()),
            Default::default(),
            wake.clone(),
        );
        step!(
            "ai remove",
            ai::act(
                &own,
                ai_keys::PACK,
                &PanelEvent::Row("remove:people".into())
            )
        );
        assert!(!own.remove(specular_ai::Feature::People), "not twice");
        step!(
            "ai remove pack",
            ai::act(&own, ai_keys::PACK, &PanelEvent::Action)
        );

        // Frames until everything came back.
        let mut opened_at = None;
        let mut page = None;
        let mut finished = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while opened_at.is_none() || !g.library.searched || page.is_none() || finished.len() < 2 {
            assert!(
                Instant::now() < deadline,
                "stuck: opened {opened_at:?}, error {:?}, searched {}, searching {}, page {}, finished {finished:?}",
                g.library.error,
                g.library.searched,
                g.library.searching,
                page.is_some()
            );
            step!("poll", g.library_poll(&wake));
            if opened_at.is_none() && g.library.lib.is_some() {
                opened_at = Some(Instant::now());
            }
            while let Ok(m) = g.rx.try_recv() {
                if let Msg::Library(r) = m {
                    step!("results", g.library_results(*r));
                }
            }
            if let Some(p) = step!("ai page take", pages.take_all().map(|(p, _)| p)) {
                page = Some(p);
            }
            step!("ai changed", pages.pack_changed());
            finished.extend(step!("ai finished", own.take_finished()));
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(ui);

        let worst = steps.iter().max_by_key(|s| s.1).unwrap();
        let mut times: Vec<Duration> = steps.iter().map(|s| s.1).collect();
        times.sort();
        eprintln!(
            "{} UI steps while the catalogue was held {HOLD:?}: median {:?}, worst {:?} ({}); frame {FRAME:?}, limit {LIMIT:?}",
            steps.len(),
            times[times.len() / 2],
            worst.1,
            worst.0
        );
        // Over the limit is not judged on an overloaded machine (`specular_log::timing`); a
        // step that waited for the writer is still caught at any load by the UI-thread guard
        // (debug builds) and, for most of the hold, by the bound below.
        specular_log::timing::assert_within(worst.0, worst.1, LIMIT);
        assert!(worst.1 < HOLD / 4, "{} took {:?}", worst.0, worst.1);
        // The open did wait for the writer (so the steps above ran while it held on).
        let opened_at = opened_at.unwrap();
        assert!(
            opened_at.duration_since(locked_at) >= HOLD / 2,
            "opened {:?} after the lock",
            opened_at.duration_since(locked_at)
        );
        locker.join().unwrap();
        assert!(woken.load(Ordering::Relaxed) > 0);
        assert_eq!(g.library.error, None);
        assert_eq!(g.library.search_error, None);
        assert!(!g.library.opening());
        // The AI removals ran, after the writer let go.
        finished.sort();
        assert_eq!(
            finished,
            [
                ("remove pack".to_owned(), Ok(())),
                ("remove people".to_owned(), Ok(()))
            ]
        );
        assert!(page.is_some_and(|p| p.people.is_empty()));
        // What was asked while it opened is in: the location added (marks were mirrored in
        // full, the first search ran).
        let l = g.library.lib.clone().unwrap();
        assert!(l.wait_idle(Duration::from_secs(10)));
        // The Library keeps canonical paths (/var is /private/var on macOS).
        let photos = specular_platform::path::canonicalize(&photos).unwrap();
        assert!(l.locations().iter().any(|loc| loc.path == photos));
        drop(l);
        g.library.lib = None;
        own.join();
        if let Some(p) = &pages.pack {
            p.join();
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// One shared copy of each path keys the grid's tables (`by_path`, `metas`, `averages`),
    /// and paths no longer found leave them when the rows change.
    #[test]
    fn path_maps_share_one_copy_and_follow_the_rows() {
        let row = |path: &str| lib::Row {
            id: 1,
            path: PathBuf::from(path),
            name: path.rsplit('/').next().unwrap().into(),
            folder: "/photos".into(),
            ext: "jpg".into(),
            size: 1,
            mtime_ns: 0,
            state: 1,
            meta: Some(Meta {
                width: 4,
                height: 3,
                ..Meta::default()
            }),
            mark: lib::Mark::default(),
            tags: String::new(),
            content: None,
            classes: 0,
            offline: false,
            dupes: None,
            similar: None,
            place: None,
            people: None,
            words: None,
            develop: None,
            albums: None,
            iptc: None,
        };
        let view = (SortKey::Name, false);
        let mut g = Gallery {
            active: true,
            ..Gallery::default()
        };
        g.library.on = true;
        (g.view.sort, g.view.desc) = view;
        let (a, b) = (Path::new("/photos/a.jpg"), Path::new("/photos/b.jpg"));
        g.library.seq = 1;
        let r = results(
            Results {
                seq: 1,
                ..Results::default()
            },
            vec![row("/photos/a.jpg"), row("/photos/b.jpg")],
            view,
        );
        assert!(g.library_results(r));
        let key = |g: &Gallery, p: &Path| g.by_path.get_key_value(p).unwrap().0.clone();
        assert!(Arc::ptr_eq(
            &key(&g, a),
            g.metas.get_key_value(a).unwrap().0
        ));
        let shared = g.shared_path(b);
        assert!(Arc::ptr_eq(&key(&g, b), &shared));
        g.averages.insert(shared, [1, 2, 3, 255]);
        g.averages.insert(a.into(), [9, 9, 9, 255]);

        // A search without a: its facts and colour go; b's take the new rows' copy.
        g.library.seq = 2;
        let r = results(
            Results {
                seq: 2,
                ..Results::default()
            },
            vec![row("/photos/b.jpg")],
            view,
        );
        assert!(g.library_results(r));
        assert_eq!(g.by_path.len(), 1);
        assert!(!g.metas.contains_key(a));
        assert!(!g.averages.contains_key(a));
        let kb = key(&g, b);
        assert!(Arc::ptr_eq(&kb, g.metas.get_key_value(b).unwrap().0));
        assert!(Arc::ptr_eq(&kb, g.averages.get_key_value(b).unwrap().0));
    }

    /// Results made off the frame carry the grid's tables: each entry found by its path, its
    /// facts, and the order of the view asked for; a file without facts in the catalogue
    /// keeps what the gallery read itself, and a view changed meanwhile sorts again.
    #[test]
    fn results_bring_their_tables_and_order() {
        let row = |i: i64, meta: bool| lib::Row {
            id: i,
            path: PathBuf::from(format!("/lib/{i}.jpg")),
            name: format!("{i}.jpg"),
            folder: "/lib".into(),
            ext: "jpg".into(),
            size: 10 + i as u64,
            mtime_ns: 0,
            state: u8::from(meta),
            meta: meta.then(|| Meta {
                width: 30,
                height: 20,
                ..Meta::default()
            }),
            mark: lib::Mark::default(),
            tags: String::new(),
            content: None,
            classes: 0,
            offline: false,
            dupes: None,
            similar: None,
            place: None,
            people: None,
            words: None,
            develop: None,
            albums: None,
            iptc: None,
        };
        let view = (SortKey::Size, true);
        let r = results(
            Results {
                seq: 1,
                ..Results::default()
            },
            vec![row(1, true), row(3, false), row(2, true)],
            view,
        );
        assert_eq!(r.by_path[Path::new("/lib/3.jpg")], 1);
        assert_eq!(r.no_meta, [1]);
        assert_eq!(r.order, Some((view, vec![1, 2, 0])));
        let mut g = Gallery::default();
        g.library.on = true;
        (g.view.sort, g.view.desc) = view;
        g.library.seq = 1;
        let own = Arc::new(Meta {
            width: 7,
            ..Meta::default()
        });
        g.metas.insert(Path::new("/lib/3.jpg").into(), own.clone());
        g.metas
            .insert(Path::new("/elsewhere.jpg").into(), own.clone());
        assert!(g.library_results(r));
        assert_eq!(g.sorted, [1, 2, 0]);
        assert_eq!(g.by_path.len(), 3);
        assert_eq!(g.metas.len(), 3);
        assert_eq!(g.metas[Path::new("/lib/3.jpg")], own);
        assert_eq!(g.metas[Path::new("/lib/2.jpg")].width, 30);
        // The view changed while the search ran: sorted again here.
        let r = results(
            Results {
                seq: 2,
                hash: 9,
                ..Results::default()
            },
            vec![row(1, true), row(3, false), row(2, true)],
            (SortKey::Size, false),
        );
        g.library.seq = 2;
        g.library.hash = 0;
        assert!(g.library_results(r));
        assert_eq!(
            g.sorted,
            [1, 2, 0],
            "sorted for the view shown, not the one asked"
        );
    }

    /// The UI thread's share of a Library grid of `SPECULAR_GRID_N` rows (default a million):
    /// a search's results taken in, the grid rebuilt and laid out, and the same rows again
    /// (an indexer refresh). Prints the times; run with `--ignored --nocapture`.
    #[test]
    #[ignore = "a million rows (~1 GB, a minute); run with --ignored"]
    fn library_grid_at_scale() {
        let n: usize = std::env::var("SPECULAR_GRID_N")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1_000_000);
        let view = (SortKey::DateTaken, true);
        let make = |seq: u64, read: i64| {
            // Rows as the catalogue gives them (newest first), made into results as on the
            // search thread.
            let rows: Vec<lib::Row> = (0..n)
                .map(|k| {
                    let i = n - 1 - k;
                    let folder = format!("/lib/{}/d{:03}", i / 12_500, (i / 100) % 125);
                    let name = format!("IMG_{i:07}.jpg");
                    let taken = Some(1_500_000_000 + i as i64 * 60);
                    lib::Row {
                        id: i as i64,
                        path: Path::new(&folder).join(&name),
                        name,
                        folder,
                        ext: "jpg".into(),
                        size: 1000,
                        mtime_ns: read,
                        state: 1,
                        meta: Some(Meta {
                            format: "JPEG".into(),
                            width: 4000,
                            height: 3000,
                            date_taken: taken,
                            camera: Some("Camera".into()),
                            ..Meta::default()
                        }),
                        mark: lib::Mark::default(),
                        tags: String::new(),
                        content: None,
                        classes: 0,
                        offline: false,
                        dupes: None,
                        similar: None,
                        place: None,
                        people: None,
                        words: None,
                        develop: None,
                        albums: None,
                        iptc: None,
                    }
                })
                .collect();
            let t = Instant::now();
            let r = results(
                Results {
                    seq,
                    total: n,
                    ..Results::default()
                },
                rows,
                view,
            );
            (r, t.elapsed())
        };
        let mut g = Gallery {
            active: true,
            vw: 1920.0,
            vh: 1080.0,
            ..Gallery::default()
        };
        g.library.on = true;
        (g.view.sort, g.view.desc) = view;
        g.library.seq = 1;
        g.library.searching = true;
        let (r, made) = make(1, 1);
        let t = Instant::now();
        assert!(g.library_results(r));
        let taken = t.elapsed();
        let t = Instant::now();
        g.rebuild();
        let rebuilt = t.elapsed();
        assert_eq!(g.tiles.len(), n);
        assert_eq!(
            g.entries[g.tiles[0].entry].name,
            format!("IMG_{:07}.jpg", n - 1)
        );
        // A refresh: the same rows, another hash (facts read meanwhile).
        let (r, _) = make(2, 2);
        g.library.seq = 2;
        g.library.searching = true;
        let t = Instant::now();
        assert!(g.library_results(r));
        let again = t.elapsed();
        let t = Instant::now();
        g.rebuild();
        let rebuilt2 = t.elapsed();
        eprintln!(
            "grid of {n}: results made (search thread) {made:.2?}; UI thread: taken in {taken:.2?}, \
             rebuilt {rebuilt:.2?}; refresh taken in {again:.2?}, rebuilt {rebuilt2:.2?}"
        );
    }
}
