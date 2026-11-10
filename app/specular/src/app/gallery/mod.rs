//! The gallery (thumbnail mode, DESIGN §10): a grid of a folder's images with the comfort
//! features in plain sight and the advanced ones behind the View menu, the palette and the
//! bottom bar's overflow button.
//!
//! The state lives in [`Gallery`]; this file holds the model (listing, sorting, filtering,
//! stacks, groups), the thumbnail feed (engine wish list, GPU uploads), switching to and from
//! the viewer and the file operations. Drawing is in `draw.rs`, input in `input.rs`.

pub(in crate::app) mod albums;
mod atlas;
pub(in crate::app) mod basemap;
mod bench;
mod compare;
pub(in crate::app) use compare::CmpMode;
#[cfg(test)]
mod develop_shots;
mod draw;
pub(in crate::app) mod drone;
mod edited;
mod facets;
mod filterbar;
mod import;
mod input;
mod library;
pub(in crate::app) mod local;
mod map;
pub(super) mod marks;
mod merged;
pub(in crate::app) use merged::MergeItem;
mod model;
mod people;
mod picker;
mod quick;
mod scrollbench;
#[cfg(test)]
mod shots;

pub(super) use draw::truncate;
pub(super) use library::{
    LIBRARY_HOME_KEEP_ITEM, LIBRARY_HOME_PICTURES_ITEM, LIBRARY_REMOVE_ITEM,
    skip_of as library_skip_of,
};
pub(super) use quick::render_quick;
mod similar;

use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use specular_core::{CancelToken, Priority};
use specular_gfx::{Backdrop, Gpu, Icon, Rect, TexId, UiRenderer};
use specular_i18n::{I18n, Localizer, NaturalKey, t};
use specular_inspect::ChannelView;
use specular_log::{CACHE, UI, debug, info, warn};
use specular_settings::{GalleryGroup, Origin};
use specular_thumbs::engine::Outcome;
use specular_thumbs::layout::{Item, Layout, Mode, Params};
use specular_thumbs::list::{ListOptions, list_dir};
use specular_thumbs::pixels::Pixels;
use specular_thumbs::sort::{self, SortKey};
use specular_thumbs::{DiskCache, Engine, EngineConfig, Entry, EntryKind, Meta, Request};
use specular_widgets::PaletteItem;
use specular_widgets::accesskit;

use self::atlas::Atlas;
use self::marks::Store;
use self::model::{Filter, GroupKey, GroupNeeds, ViewOpts};
pub(super) use self::model::{Mark, is_raw_ext};
use super::App;
use crate::archive_thumbs::ArchiveOpener;
use crate::thumbnail::default_cache_dir;

pub(super) use self::draw::Frame as GalleryFrame;

/// Bars, logical px.
pub(super) const TOP_BAR: f32 = 44.0;
pub(super) const FILTER_BAR: f32 = 40.0;
pub(super) const BOTTOM_BAR: f32 = 44.0;
/// The selection bar's height, and the room it takes over the grid's end (its gap to the
/// bottom bar and a margin above it) that the grid scrolls past while it shows.
pub(super) const SEL_BAR: f32 = 40.0;
pub(super) const SEL_BAR_ROOM: f32 = SEL_BAR + 12.0 + 8.0;
pub(super) const DETAILS_HEADER: f32 = 28.0;
/// Palette ids of the gallery's entries ([`App::gallery_item`]).
pub(super) const ITEM_BASE: u64 = 1 << 48;
/// AccessKit ids of the gallery's nodes.
const A11Y_BASE: u64 = 0x4000_0000_0000_0000;
/// The Properties card's nodes: the dialog, its close button, then its facts.
const A11Y_INFO: u64 = A11Y_BASE + (14 << 40);
/// The selection bar's nodes: the toolbar, then its buttons.
const A11Y_SELBAR: u64 = A11Y_BASE + (15 << 40);
/// GPU atlas pages (2048² each, 16 MiB): about 2000 thumbnails of 128 px.
const ATLAS_PAGES: usize = 10;
/// Time per frame spent uploading thumbnails: a quarter of a 120 Hz frame. Each upload's
/// cost is predicted from the measured cost per byte *before* it starts, and what does not fit
/// waits for the next frame.
const UPLOAD_BUDGET: Duration = Duration::from_micros(2000);
/// Bytes uploaded per frame at most (four 512 px thumbnails).
const UPLOAD_BYTES: u64 = 4 << 20;
/// Pixel bytes waiting for upload at most (about 48 thumbnails of 512 px): what is over is
/// dropped, newest first (the engine keeps them in memory should they be wanted again).
/// Without it, results for tiles scrolled past while flinging piled up behind the first one
/// still wished, each holding its pixels after the engine let them go (350 MB in a 100k-file
/// scroll).
const UPLOAD_QUEUE_BYTES: u64 = 48 << 20;
/// Scrolling faster than this (viewports per second) is a fling or a scrollbar drag: only
/// visible tiles are uploaded, nothing behind is prefetched, and the prefetch reaches further
/// ahead. Full quality returns once the view rests.
const FAST_VIEWPORTS_S: f32 = 2.5;
/// How far ahead the prefetch reaches: this much time at the current speed, 2 to 12 viewports.
const PREFETCH_LEAD_S: f32 = 0.5;
/// Meta-only requests per wish list when every image's meta is needed, and entries looked at.
const META_CHUNK: usize = 1024;
const META_SCAN: usize = 8192;
/// Thumbnail requests per wish list at most (the visible ones always go in): the prefetch
/// past them waits for the next list.
const WISH_MAX: usize = 2048;
/// Time per frame for building the wish list past the visible tiles; a list cut short by it
/// is finished on the next frame.
const WISH_BUDGET: Duration = Duration::from_micros(1000);
const TRANSITION_S: f64 = 0.28;
const FADE_S: f64 = 0.15;
/// A thumbnail that arrives while its tile is on screen fades in from the placeholder (s).
pub(super) const THUMB_FADE_S: f64 = 0.12;
const HOVER_PREFETCH_S: f64 = 0.4;
/// Folders the back history keeps.
const HISTORY_MAX: usize = 100;
/// A fresh listing shows its grid when done, or after this long (then it streams in): no
/// reshuffling of a grid that is being looked at for folders that list within it.
const LIST_HOLD: Duration = Duration::from_millis(250);
const TOOLTIP_S: f64 = 0.7;
/// Seconds after closing before the gallery's GPU pages are dropped.
const IDLE_RELEASE_S: f64 = 60.0;

/// One tile: an entry, or a stack shown by one of its members.
#[derive(Clone, Debug)]
pub(super) struct Tile {
    pub entry: usize,
    /// Every member of a stack (the shown one included); empty for single files.
    pub members: Vec<usize>,
    pub group: u32,
}

enum Msg {
    Chunk {
        generation: u64,
        entries: Vec<Entry>,
        /// Natural sort keys of the entries' names, made on the listing thread with `loc`.
        keys: Vec<NaturalKey>,
        loc: Arc<Localizer>,
    },
    Done {
        generation: u64,
        error: Option<String>,
        /// The listing in display order, sorted on the listing thread (while the main thread
        /// still takes in the last chunks); used when nothing else changed the entries.
        presorted: Option<Presorted>,
    },
    Sidecars {
        generation: u64,
        marks: Vec<(PathBuf, Mark)>,
    },
    /// Photos with more than one Develop version (their `.vN.specular.xmp` sidecars).
    Versions {
        generation: u64,
        counts: Vec<(PathBuf, u32)>,
    },
    /// HDR merges of the listing and their sources.
    Merges {
        generation: u64,
        found: Vec<(PathBuf, Vec<PathBuf>)>,
    },
    /// The listing's photos with a Develop sidecar.
    Edited {
        generation: u64,
        photos: Vec<PathBuf>,
    },
    /// Their edits, read: each photo's versions with what they use.
    DevelopFacts {
        generation: u64,
        facts: Vec<(PathBuf, Vec<crate::app::develop::edited::Facts>)>,
    },
    /// The DICOM whole-slide images of the listing, each slide's files with the shown one
    /// first (`specular_dicom::series::whole_slides`).
    Slides {
        generation: u64,
        slides: Vec<Vec<PathBuf>>,
    },
    Dates {
        generation: u64,
        dates: Vec<(PathBuf, i64)>,
    },
    Cover {
        dir: PathBuf,
        images: Vec<PathBuf>,
        /// An encrypted archive not unlocked this session: its tile shows a lock.
        locked: bool,
    },
    /// The thumbnails came across the ComicInfo of an archive opened without it: its tile
    /// pages ([`ArchiveOpener::with_cover_updates`]).
    TileKnown {
        archive: PathBuf,
        images: Vec<PathBuf>,
    },
    /// The offline geocoder's index is built: event headers can name their places.
    PlacesReady,
    Trashed {
        undo: u64,
        done: Vec<PathBuf>,
        failed: Vec<(PathBuf, String)>,
    },
    Restored {
        done: Vec<PathBuf>,
        failed: Vec<(PathBuf, String)>,
    },
    Moved {
        undo: Option<u64>,
        copy: bool,
        done: Vec<(PathBuf, PathBuf)>,
        failed: Vec<(PathBuf, String)>,
    },
    /// A Library search came back.
    Library(Box<library::Results>),
    /// An album change (or the album list) came back.
    Albums(Box<albums::Done>),
}

/// What Undo reverses.
enum UndoOp {
    Trash(Vec<PathBuf>),
    Move(Vec<(PathBuf, PathBuf)>),
    Copy(Vec<PathBuf>),
    Marks(Vec<(PathBuf, Mark)>),
    /// Each photo's keywords before a keyword change.
    Keywords(Vec<(PathBuf, Vec<String>)>),
}

/// The flight of a thumbnail between its tile and the viewer (or a cross-fade).
#[derive(Clone, Debug)]
pub(super) struct Transition {
    pub start: f64,
    pub duration: f64,
    /// Gallery → viewer.
    pub opening: bool,
    pub path: Option<PathBuf>,
    pub tile: Rect,
    pub image: Rect,
    pub fade_only: bool,
}

/// A press in the grid that may become a click, a rubber band or a drag.
#[derive(Clone, Debug)]
pub(super) struct Press {
    pub at: (f32, f32),
    /// Content y of the press (the band follows scrolling).
    pub content_y: f32,
    pub tile: Option<usize>,
    pub moved: bool,
    pub band: bool,
    pub drag: bool,
    /// Selection before the band started (Ctrl/Shift bands add to it).
    pub base: HashSet<PathBuf>,
}

/// The folder tile a drag would land in (gallery files, or files from another application).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct DropTarget {
    pub tile: usize,
    pub dest: PathBuf,
    /// Copies rather than moves (modifier keys, or another volume).
    pub copy: bool,
    /// The modifier keys that decided `copy` (forced copy, forced move).
    forced: (bool, bool),
}

/// Files from another application dropped in one event burst (winit sends one event per file).
struct Dropped {
    dest: PathBuf,
    forced: (bool, bool),
    files: Vec<PathBuf>,
}

/// The Properties card (Alt+Enter, the right-click menu's Properties): the facts of what is
/// selected, beside the tile they are about. Read-only, so a card rather than a list to choose
/// from; Esc, a click elsewhere, scrolling or another key closes it.
pub(super) struct InfoCard {
    /// The tile it sits beside (none for several targets: then by the focused one).
    pub tile: Option<usize>,
    /// What it is about (Properties again for the same closes it).
    pub targets: Vec<PathBuf>,
    /// The file or folder whose thumbnail heads it (one target only).
    pub path: Option<PathBuf>,
    pub kind: Option<EntryKind>,
    /// The name (or "3 items"), and what it is under it ("PNG · 1.2 MB").
    pub title: String,
    pub subtitle: String,
    /// Label and value.
    pub rows: Vec<(String, String)>,
    /// When it was first drawn (NaN until then): it fades in from there.
    pub opened: f64,
    /// Where it was drawn last, and its close button (the screen reader's bounds).
    pub rect: Option<Rect>,
    pub close: Option<Rect>,
}

/// The selection bar as last drawn: the pill, and its buttons with their names (the screen
/// reader's nodes and the tooltips). Shown while two or more items are selected.
pub(super) struct SelBar {
    pub rect: Rect,
    pub buttons: Vec<(Rect, Hit, String)>,
    /// When it appeared (it rises into place from there).
    pub since: f64,
}

/// Bar buttons and other hit areas, recorded while drawing.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Hit {
    /// Back to the start screen.
    Home,
    Crumb(PathBuf),
    Up,
    Filter,
    ViewMenu,
    More,
    Layout(Mode),
    SizeDown,
    SizeUp,
    Slider,
    Sort,
    Chip(model::Chip),
    FilterField,
    CloseFilter,
    /// The facets buttons (top bar and filter bar).
    FacetsButton,
    /// A value row of the facets panel.
    Facet(usize),
    /// The facets panel's search field.
    FacetsFind,
    /// The rest of the panel (takes clicks from the grid under it).
    FacetsPanel,
    /// The keyword panel (its own controls are its own hits).
    KeywordsPanel,
    /// The metadata editor (its own controls are its own hits).
    MetaEditPanel,
    /// A row of the search field's value picker (`true`: its check box).
    PickerRow(usize, bool),
    /// An operator button over the picker's list (an index into its operators).
    PickerOp(u8),
    /// The rest of the picker, and its scrollbar.
    PickerPanel,
    PickerScroll,
    Column(SortKey),
    Scrollbar,
    /// The Properties card, and its close button.
    InfoCard,
    InfoClose,
    /// A button of the selection bar (a gallery code), its More button, and the rest of it.
    SelBar(u64),
    SelMore,
    SelBarBg,
    /// The Library's sidebar and its top bar toggle.
    Library(library::Side),
}

/// A colour label's English name (remote control's `state`), None for no label.
pub(super) fn label_english(label: u8) -> Option<&'static str> {
    model::LABELS
        .get(usize::from(label).checked_sub(1)?)
        .copied()
}

/// The tile-size slider's range (logical px).
pub(super) fn slider_value(size: f32) -> f32 {
    ((size - model::MIN_SIZE) / (model::MAX_SIZE - model::MIN_SIZE)).clamp(0.0, 1.0)
}

#[derive(Default)]
pub(super) struct FillStats {
    pub entered: Option<Instant>,
    pub first_grid: Option<Duration>,
    pub filled: Option<Duration>,
    pub sharp: Option<Duration>,
    pub frames: Vec<f32>,
}

pub(super) struct Gallery {
    pub active: bool,
    /// The folder shown.
    dir: Option<PathBuf>,
    generation: u64,
    listing: bool,
    list_token: Option<CancelToken>,
    list_error: Option<String>,
    entries: Vec<Entry>,
    /// Index of each entry by path. The key is the one shared copy of the path: `metas` and
    /// `averages` key by it too (see [`Gallery::shared_path`]).
    by_path: HashMap<Arc<Path>, usize>,
    /// Natural sort key of each entry's name (parallel to `entries`), for `keys_loc`.
    keys: Vec<NaturalKey>,
    keys_loc: Option<Arc<Localizer>>,
    /// When the current listing started (the grid is held back for [`LIST_HOLD`]).
    list_started: Option<Instant>,
    /// Entries a fresh listing appended so far, in its order, with nothing else changing
    /// them (None otherwise): the condition for using its [`Presorted`] order.
    list_appended: Option<usize>,
    /// Entries in sort order.
    sorted: Vec<usize>,
    tiles: Vec<Tile>,
    /// Tile of each entry (by index; `NO_TILE` when hidden), stack members included: see
    /// [`Gallery::tile_of`]. A path map rebuilt per rebuild cost ~60 ms at 100 000 entries.
    entry_tile: Vec<u32>,
    group_labels: Vec<String>,
    /// Event headers may name the place (settings `metadata.reverse_geocoding` on and
    /// `hide_gps` off).
    places: bool,
    /// Settings `metadata.hide_gps`: the map shows no positions.
    hide_gps: bool,
    /// Event headers were made without place names because the offline geocoder's index is
    /// not built yet: it is built off the frame and they are remade once it is ready.
    places_waiting: bool,
    /// The geocoder's index is being built for the headers.
    places_asked: bool,
    /// Country names in the UI language for event places, made on first use.
    place_loc: Option<specular_metadata::Localizer>,
    layout: Layout,
    view: ViewOpts,
    /// Animated tile size (follows `view.size`).
    size_shown: f32,
    filter: Filter,
    filter_open: bool,
    filter_focus: bool,
    /// IME composition shown at the filter field's caret while the field has the focus.
    preedit: String,
    /// The filter field's caret, selection and undo history. `filter.text` is the truth: once
    /// it changes another way (a chip removed, a facet toggled, cleared) the field catches up
    /// as one undo step with the caret at the end (see `field`).
    filter_field: specular_widgets::line_edit::LineEdit,
    /// The filter text as last drawn and the x it starts at (a click puts the caret).
    filter_drawn: Option<(Arc<specular_gfx::TextBlock>, f32)>,
    /// The filter field's caret, or the composition ending at it, as last drawn (where the
    /// IME candidate window goes).
    ime_area: Option<Rect>,
    /// The query has terms that need metadata: the scan reads every entry's (see `feed_wishes`).
    filter_needs_meta: bool,
    /// The facets panel and its counts.
    facets: facets::Panel,
    /// The keyword panel (also over cull mode's photo).
    pub(in crate::app) keywords: crate::app::keywords::Keywords,
    /// The listing's keywords for the filter (folder mode).
    pub(in crate::app) listing_tags: crate::app::keywords::ListingTags,
    /// The metadata editor (also over cull mode's photo and the viewer).
    pub(in crate::app) meta_edit: crate::app::meta_edit::MetaEdit,
    /// The search field's value picker.
    picker: picker::Picker,
    marks: Store,
    /// Whether `marks` was made for this session's tables (the gallery's first opening, or
    /// cull mode's).
    marks_made: bool,
    /// Photos with more than one Develop version, and how many (the tile's badge).
    versions: HashMap<PathBuf, u32>,
    /// HDR merges' sources → the merge (stacked under it; `merged.rs`).
    merge_of: HashMap<PathBuf, PathBuf>,
    /// Develop edits of the listing: facts, versions' entries, edited thumbnails.
    edited: edited::Edited,
    /// The Library's edited photos of the last rows, to read.
    edited_pending: Option<Vec<PathBuf>>,
    expanded: HashSet<PathBuf>,
    /// DICOM whole-slide files that are one slide (levels, label, overview): each file's
    /// slide number, and whether it is the file the slide's tile shows.
    slides: HashMap<PathBuf, (u32, bool)>,
    /// Facts and average colours, keyed by the listed paths' shared copies; trimmed to the
    /// listing when it changes ([`Gallery::trim_to_listing`]).
    metas: HashMap<Arc<Path>, Arc<Meta>>,
    averages: HashMap<Arc<Path>, [u8; 4]>,
    /// Thumbnails first uploaded while their tile was on screen, and when (the frame
    /// clock): they fade in over [`THUMB_FADE_S`]. Ones already in the atlas show at once.
    fade_in: HashMap<PathBuf, f64>,
    covers: HashMap<PathBuf, Vec<PathBuf>>,
    /// Encrypted archives without a known password (their tiles show a lock).
    locked: HashSet<PathBuf>,
    cover_pending: HashSet<PathBuf>,
    /// Lists the folder mosaics' images (made with the engine, off the frame).
    covers_in: Option<CoverLister>,
    /// The engine's archives (kept open for their pages).
    archives: Option<Arc<ArchiveOpener>>,
    engine: Option<Engine>,
    atlas: Option<Atlas>,
    uploads: VecDeque<Upload>,
    /// The tiles of the last wish list (visible, ahead, behind), and the images it asked for
    /// outside them (quick look, compare, similar shots, folder mosaics): queued uploads of
    /// anything else are dropped (the engine keeps them in memory should they come back).
    wish_tiles: [Range<usize>; 3],
    wish_extra: HashSet<PathBuf>,
    /// Wish lists cut short by [`WISH_BUDGET`] in a row (each doubles the next one's time).
    wish_short: u32,
    /// Whether the last wish list was made while scrolling fast.
    wish_fast: bool,
    /// Where the next chunk of meta requests starts, and the state in which a full pass
    /// found nothing missing (generation, tiles, metas, failed).
    meta_cursor: usize,
    meta_complete: Option<(u64, usize, usize, usize)>,
    /// Entries looked at since the scan last found one missing (a pass ends at all tiles),
    /// counted for this order of the tiles (generation, tiles).
    meta_clean: usize,
    meta_clean_at: (u64, usize),
    /// Smoothed scroll speed (px/s, signed), measured once per frame in `feed`.
    scroll_vel: f32,
    /// The last measurement (time, scroll) and when the view last moved.
    vel_mark: Option<(Instant, f32)>,
    moved_at: Option<Instant>,
    /// The pages for this listing were asked for ahead; more only at rest.
    paged_ahead: bool,
    /// Measured upload cost (ns per byte), to predict whether the next upload fits the frame.
    upload_ns_per_byte: f32,
    /// Visible tiles left at the front of `uploads` by the last frame.
    uploads_front: usize,
    /// The last frame stopped at the upload budget: draw again soon.
    upload_more: bool,
    /// A worker held the engine's lock when the frame asked: ask again next frame.
    engine_retry: bool,
    /// Files whose thumbnail failed, with how much free RAM it needed when that was why.
    failed: HashMap<PathBuf, Arc<specular_decode::DecodeError>>,
    /// When to look again whether thumbnails refused for memory now fit (seconds).
    memory_retry_at: f64,
    wish_key: u64,
    scroll: f32,
    scroll_target: f32,
    /// Content y of the last wish list (the scroll direction).
    scroll_dir: f32,
    scrolled_at: f64,
    focus: Option<PathBuf>,
    anchor: Option<PathBuf>,
    selected: HashSet<PathBuf>,
    positions: HashMap<PathBuf, (f32, Option<PathBuf>)>,
    /// The folders shown before this one (the last is the latest) and the ones gone back
    /// from, as a browser's history: Alt+← / Alt+→, ⌘[ / ⌘], the mouse's side buttons.
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    /// The next folder change is a step through that history, not a new place.
    history_step: bool,
    /// The folder's watch, set up on its own thread (kept across relists of the same view).
    watch: Option<specular_thumbs::watch::TreeWatch>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    pub transition: Option<Transition>,
    quick: Option<PathBuf>,
    quick_held: bool,
    /// The textures quick look and compare render channel views into, and this frame's use
    /// of them: per image, its texture, where it goes and the view drawn (see `quick.rs`).
    quick_targets: Vec<quick::QuickTarget>,
    quick_tex: Vec<(PathBuf, TexId, Rect, ChannelView)>,
    /// Compare's difference view: the textures it draws into, and this frame's (the image
    /// compared with the first, its texture and where it goes).
    diff_targets: Vec<quick::QuickTarget>,
    diff_tex: Vec<(usize, TexId, Rect)>,
    compare: Option<Vec<PathBuf>>,
    /// Compare's mode, zoom and pan, overlay, difference view and metrics (`compare.rs`).
    cmp: compare::CompareState,
    /// "Compare with…": the current image and the file dialog's answer to come.
    compare_with: Option<(PathBuf, std::sync::mpsc::Receiver<PathBuf>)>,
    /// The Properties card, while open.
    info: Option<InfoCard>,
    /// Said to screen readers (a polite live region) after going up a folder.
    announce: Option<String>,
    hover: Option<(usize, f64)>,
    prefetched: Option<PathBuf>,
    press: Option<Press>,
    jump: (String, f64),
    /// The last input came from the pointer (Space shows quick look under it).
    pointer_input: bool,
    /// The chrome has the keyboard (the "why is this slow?" indicator), set before each
    /// frame: the focused tile then shows no focus ring.
    pub(in crate::app) chrome_kbd: bool,
    sel_bar: Option<SelBar>,
    /// The background work indicator's width in the top bar (0 when not shown), set before
    /// each frame: the breadcrumb leaves it room.
    pub(in crate::app) work_chip: f32,
    /// Where the indicator's end goes in the top bar (left-to-right), from the last frame.
    pub(in crate::app) work_end: f32,
    /// The bar button under the pointer this frame and its tooltip text (set while drawing),
    /// and the one whose tooltip is waiting or shown, since when (infinite: pressed).
    bar_tip_now: Option<(Rect, String)>,
    bar_tip: Option<(Rect, f64)>,
    /// The bottom bar's free stretch after the item count (left-to-right from, to), set by
    /// this frame's [`Gallery::draw`] before the chrome draws: downloads show there.
    pub(in crate::app) status_free: (f32, f32),
    pointer: Option<(f32, f32)>,
    hits: Vec<(Rect, Hit)>,
    slider_drag: bool,
    scrollbar_drag: Option<f32>,
    undo: HashMap<u64, UndoOp>,
    relayout_at: Option<f64>,
    resort_at: Option<Instant>,
    marks_due: Option<Instant>,
    /// Drag and drop onto folder tiles: the tile under the pointer, the files another
    /// application is dragging over the window, and a drop waiting to be carried out.
    drop_target: Option<DropTarget>,
    drop_hover: Vec<PathBuf>,
    dropped: Option<Dropped>,
    /// Logical window size and scale of the last frame.
    vw: f32,
    vh: f32,
    scale: f32,
    /// The accessibility text scale of the last frame (1 = 100 %): captions, group headers,
    /// the filter bar and the quick look caption follow it ([`Gallery::u`]).
    text_scale: f32,
    /// What shows through transparent thumbnails (the viewer's transparency setting).
    backdrop: Backdrop,
    /// The viewer was showing an image when the gallery opened (Esc / g go back to it).
    pub from_viewer: bool,
    /// Opened from the gallery: Back in the viewer returns here.
    pub viewer_from_gallery: bool,
    pending_copy: Option<PathBuf>,
    pub stats: FillStats,
    dirty: bool,
    relayout: bool,
    /// Scroll the focus into view after the next rebuild.
    reveal: bool,
    /// Entries seen by a refreshing relist, by index (the rest is gone when it ends; a relist
    /// only replaces or appends entries until then).
    refresh_seen: Option<Vec<bool>>,
    // Display options (settings, changed from the View menu too).
    show_captions: bool,
    show_badges: bool,
    /// The "Edited" badge (gallery view menu).
    edited_badges: bool,
    stacks_on: bool,
    info_overlay: bool,
    show_hidden: bool,
    show_other: bool,
    show_archives: bool,
    depth: u32,
    /// Right-to-left UI: the grid fills from the right.
    pub rtl: bool,
    last_frame: f64,
    bench: Option<bench::Bench>,
    /// `--show gallery-scroll-bench`: scripted scrolling with frame-time statistics.
    sbench: Option<scrollbench::ScrollBench>,
    /// Truncated captions by path (and the width they were cut for).
    cap_cache: HashMap<PathBuf, (u32, String)>,
    /// Captions shaped this frame (each new one costs a text layout).
    cap_new: u32,
    /// When this frame's first new caption was shaped; new captions stop after
    /// [`draw::CAPTION_BUDGET`] and the rest wait for the next frame.
    cap_start: Option<std::time::Instant>,
    /// Captions left for the next frame (draw again at once).
    cap_more: bool,
    /// Time and tile of the last primary click (double clicks open).
    last_click: Option<(f64, usize)>,
    /// Duplicates and similar shots (DESIGN §10).
    similar: similar::Similar,
    /// The map view (open instead of the grid).
    map: map::MapView,
    people: people::PeopleView,
    pub(in crate::app) import: import::ImportView,
    /// The filter bar's focus, layout and result announcements.
    bar: filterbar::Bar,
    /// The Library mode (`L`): the catalogue's search as the grid's source.
    library: library::LibraryView,
}

impl Default for Gallery {
    fn default() -> Gallery {
        let (tx, rx) = crossbeam_channel::unbounded();
        Gallery {
            active: false,
            dir: None,
            generation: 0,
            listing: false,
            list_token: None,
            list_error: None,
            entries: Vec::new(),
            by_path: HashMap::new(),
            keys: Vec::new(),
            keys_loc: None,
            list_started: None,
            list_appended: None,
            sorted: Vec::new(),
            tiles: Vec::new(),
            entry_tile: Vec::new(),
            group_labels: Vec::new(),
            places: false,
            places_waiting: false,
            places_asked: false,
            place_loc: None,
            layout: Layout::default(),
            view: ViewOpts {
                mode: Mode::Squares,
                size: 160.0,
                sort: SortKey::Name,
                desc: false,
                group: GalleryGroup::None,
            },
            size_shown: 160.0,
            filter: Filter::default(),
            filter_open: false,
            filter_focus: false,
            preedit: String::new(),
            filter_field: specular_widgets::line_edit::LineEdit::default().with_terms(),
            filter_drawn: None,
            ime_area: None,
            filter_needs_meta: false,
            facets: facets::Panel::default(),
            keywords: Default::default(),
            listing_tags: Default::default(),
            meta_edit: Default::default(),
            picker: picker::Picker::default(),
            marks: Store::new(None),
            marks_made: false,
            versions: HashMap::new(),
            merge_of: HashMap::new(),
            edited: edited::Edited::default(),
            edited_pending: None,
            expanded: HashSet::new(),
            slides: HashMap::new(),
            metas: HashMap::new(),
            averages: HashMap::new(),
            fade_in: HashMap::new(),
            covers: HashMap::new(),
            locked: HashSet::new(),
            cover_pending: HashSet::new(),
            covers_in: None,
            archives: None,
            engine: None,
            atlas: None,
            uploads: VecDeque::new(),
            wish_tiles: [0..0, 0..0, 0..0],
            wish_extra: HashSet::new(),
            wish_short: 0,
            wish_fast: false,
            meta_cursor: 0,
            meta_complete: None,
            meta_clean: 0,
            meta_clean_at: (0, 0),
            scroll_vel: 0.0,
            vel_mark: None,
            moved_at: None,
            paged_ahead: false,
            upload_ns_per_byte: 1.0,
            uploads_front: 0,
            upload_more: false,
            engine_retry: false,
            failed: HashMap::new(),
            memory_retry_at: 0.0,
            wish_key: 0,
            scroll: 0.0,
            scroll_target: 0.0,
            scroll_dir: 1.0,
            scrolled_at: 0.0,
            focus: None,
            anchor: None,
            selected: HashSet::new(),
            positions: HashMap::new(),
            back: Vec::new(),
            forward: Vec::new(),
            history_step: false,
            watch: None,
            tx,
            rx,
            transition: None,
            quick: None,
            quick_held: false,
            quick_targets: Vec::new(),
            quick_tex: Vec::new(),
            diff_targets: Vec::new(),
            diff_tex: Vec::new(),
            compare: None,
            cmp: compare::CompareState::default(),
            compare_with: None,
            info: None,
            announce: None,
            hover: None,
            prefetched: None,
            press: None,
            jump: (String::new(), 0.0),
            pointer_input: false,
            chrome_kbd: false,
            sel_bar: None,
            work_chip: 0.0,
            work_end: 0.0,
            bar_tip_now: None,
            bar_tip: None,
            status_free: (0.0, 0.0),
            pointer: None,
            hits: Vec::new(),
            slider_drag: false,
            scrollbar_drag: None,
            undo: HashMap::new(),
            relayout_at: None,
            resort_at: None,
            marks_due: None,
            drop_target: None,
            drop_hover: Vec::new(),
            dropped: None,
            vw: 800.0,
            vh: 600.0,
            scale: 1.0,
            backdrop: Backdrop::None,
            from_viewer: false,
            viewer_from_gallery: false,
            pending_copy: None,
            stats: FillStats::default(),
            dirty: false,
            relayout: false,
            reveal: false,
            refresh_seen: None,
            show_captions: true,
            show_badges: true,
            edited_badges: true,
            stacks_on: false,
            info_overlay: false,
            show_hidden: false,
            show_other: false,
            show_archives: false,
            depth: 0,
            rtl: false,
            last_frame: 0.0,
            bench: None,
            sbench: None,
            cap_cache: HashMap::with_capacity(draw::CAPTIONS_KEPT),
            cap_new: 0,
            cap_start: None,
            cap_more: false,
            text_scale: 1.0,
            last_click: None,
            similar: similar::Similar::default(),
            hide_gps: false,
            map: map::MapView::default(),
            people: people::PeopleView::default(),
            import: import::ImportView::default(),
            bar: filterbar::Bar::default(),
            library: library::LibraryView::default(),
        }
    }
}

// ----- Geometry ---------------------------------------------------------------------------

impl Gallery {
    /// The open catalogue, when there is one (Develop stores edits in it).
    pub(in crate::app) fn library_handle(
        &self,
    ) -> Option<std::sync::Arc<specular_library::Library>> {
        self.library.handle()
    }

    /// The GPS positions (latitude, longitude) of the listed photos whose metadata is in,
    /// none when positions are hidden (settings `metadata.hide_gps`).
    pub(in crate::app) fn photo_positions(&self) -> Vec<(f64, f64)> {
        if self.hide_gps {
            return Vec::new();
        }
        self.metas
            .values()
            .filter_map(|m| m.position)
            .filter(|(lat, lon)| lat.is_finite() && lon.is_finite() && (*lat, *lon) != (0.0, 0.0))
            .map(|(lat, lon)| (f64::from(lat), f64::from(lon)))
            .collect()
    }
}

impl Gallery {
    /// Where the IME candidate window goes: by the filter field's caret while it has the focus.
    pub fn ime_area(&self) -> Option<Rect> {
        if !self.active {
            return None;
        }
        if self.filter_focus {
            self.ime_area
        } else {
            self.import_ime_area()
        }
    }

    /// `px` (a height or padding laid out around text at the standard size) at the current
    /// text scale, rounded to whole logical pixels. Like the viewer chrome's `Ctx::u`, boxes
    /// grow with larger text but never shrink below the standard size.
    pub(super) fn u(&self, px: f32) -> f32 {
        (px * self.text_scale.max(1.0)).round()
    }

    /// A text style at the current text scale.
    pub(super) fn ts(&self, s: specular_gfx::TextStyle) -> specular_gfx::TextStyle {
        specular_gfx::TextStyle {
            size: s.size * self.text_scale,
            ..s
        }
    }

    /// Sets the text scale; captions, headers and bars are laid out again (the tile under the
    /// pointer or the focus stays put). Returns whether it changed.
    pub(super) fn set_text_scale(&mut self, ts: f32) -> bool {
        let ts = if ts.is_finite() {
            ts.clamp(0.5, 3.0)
        } else {
            1.0
        };
        if (ts - self.text_scale).abs() < 1e-3 {
            return false;
        }
        self.text_scale = ts;
        self.cap_cache.clear();
        self.relayout = true;
        true
    }

    /// Height of the filter bar.
    pub(super) fn filter_bar_h(&self) -> f32 {
        self.u(FILTER_BAR)
    }

    /// Height of the column header row of the details view.
    pub(super) fn details_header_h(&self) -> f32 {
        self.u(DETAILS_HEADER)
    }

    /// Height of the caption strip below a tile (0 without captions). Only this grows with
    /// the text: the thumbnails keep their size.
    pub(super) fn caption_h(&self) -> f32 {
        // Two lines: a long name wraps before it is cut.
        if self.captions() { self.u(38.0) } else { 0.0 }
    }

    /// Height of quick look's caption (and of the comparison's), which grows with the text
    /// while the image gives way.
    pub(super) fn quick_caption_h(&self) -> f32 {
        self.u(30.0)
    }

    /// Screen y of content y 0.
    pub(super) fn grid_top(&self) -> f32 {
        TOP_BAR
            + if self.filter_open {
                self.filter_bar_h()
            } else {
                0.0
            }
            + self.timeline_h()
            + self.local_banner_h()
            + if self.view.mode == Mode::Details {
                self.details_header_h()
            } else {
                0.0
            }
    }

    /// The grid's width: the window less the docked facets panel.
    pub(super) fn grid_w(&self) -> f32 {
        (self.vw - self.facets.dock).max(1.0)
    }

    pub(super) fn grid_h(&self) -> f32 {
        (self.vh - self.grid_top() - BOTTOM_BAR).max(1.0)
    }

    /// The tile's rect on screen (logical).
    pub(super) fn tile_rect(&self, i: usize) -> Option<Rect> {
        let c = self.layout.cells.get(i)?;
        let x = if self.rtl { self.vw - c.x - c.w } else { c.x };
        Some(Rect::new(x, self.grid_top() + c.y - self.scroll, c.w, c.h))
    }

    /// The tile under a logical point.
    pub(super) fn tile_at(&self, p: (f32, f32)) -> Option<usize> {
        let top = self.grid_top();
        if p.1 < top || p.1 > top + self.grid_h() {
            return None;
        }
        let x = if self.rtl { self.vw - p.0 } else { p.0 };
        self.layout.hit(x, p.1 - top + self.scroll)
    }

    pub(super) fn focus_index(&self) -> Option<usize> {
        self.focus.as_ref().and_then(|p| self.tile_of(p))
    }

    /// The tile showing `p` (for a member of a collapsed stack, the stack's tile).
    pub(super) fn tile_of(&self, p: &Path) -> Option<usize> {
        tile_in(&self.by_path, &self.entry_tile, p)
    }

    /// The focused tile's file (not a folder), for user commands and open-with.
    pub(super) fn focus_file(&self) -> Option<PathBuf> {
        self.focus_index()
            .and_then(|i| self.entry_of(i))
            .filter(|e| !e.is_container() && e.path.is_file())
            .map(|e| e.path.clone())
    }

    pub(super) fn entry_of(&self, tile: usize) -> Option<&Entry> {
        self.tiles.get(tile).map(|t| &self.entries[t.entry])
    }

    pub(super) fn path_of(&self, tile: usize) -> Option<PathBuf> {
        self.entry_of(tile).map(|e| e.path.clone())
    }

    fn params(&self) -> Params {
        let captions = self.captions();
        Params {
            width: self.grid_w(),
            tile: self.size_shown,
            caption: if captions && self.view.mode != Mode::Details {
                self.caption_h()
            } else {
                0.0
            },
            header: self.u(Params::default().header),
            ..Params::default()
        }
    }

    fn captions(&self) -> bool {
        self.show_captions
    }

    /// The covers a folder's (or archive's) tile shows: its first images, but not one whose
    /// thumbnail could not be made (it would leave a blank cell), at most four.
    pub(super) fn shown_covers(&self, dir: &Path) -> Vec<&PathBuf> {
        self.covers.get(dir).map_or_else(Vec::new, |c| {
            c.iter()
                .filter(|p| !self.failed.contains_key(p.as_path()))
                .take(4)
                .collect()
        })
    }

    /// Thumbnail class of a folder's cover images: a quarter of the tile with four covers, up
    /// to the whole tile with fewer.
    pub(super) fn cover_class(&self, covers: usize) -> u32 {
        let side = self
            .layout
            .cells
            .first()
            .map_or(self.size_shown, |c| c.w.max(c.h));
        let f = if covers >= 4 { 0.5 } else { 1.0 };
        specular_thumbs::class_for(side * f * self.scale)
    }

    /// The thumbnail class the tiles need (device px).
    pub(super) fn class(&self) -> u32 {
        let side = match self.view.mode {
            Mode::Details => 24.0,
            Mode::Justified => self.size_shown * 1.6,
            _ => self
                .layout
                .cells
                .first()
                .map_or(self.size_shown, |c| c.w.max(c.h)),
        };
        specular_thumbs::class_for(side * self.scale)
    }

    /// The displayed aspect ratio of an entry, when known.
    /// The picture's display size from its header (the Library's row, else the thumbnail
    /// engine's facts), when known.
    pub(super) fn source_size(&self, e: &Entry) -> Option<(u32, u32)> {
        match e.dims {
            Some(d) if self.library.on => Some(d),
            _ => self
                .metas
                .get(e.path.as_path())
                .filter(|m| m.width > 0 && m.height > 0)
                .map(|m| (m.width, m.height))
                .or(e.dims),
        }
    }

    fn aspect_of(&self, e: &Entry) -> Option<f32> {
        // The Library's entries carry their size from the same catalogue row as their facts:
        // no path lookup per tile (a million of them were most of a grid rebuild, 0.28 s).
        let (w, h) = self.source_size(e)?;
        Some(w as f32 / h.max(1) as f32)
    }

    pub(super) fn mark(&self, p: &Path) -> Mark {
        self.marks.mark(p)
    }

    /// The paths an operation applies to: the selection when the focus is in it (or nothing
    /// is focused), else the focused tile; stacks count with all their members.
    /// The selected files and folders in tile order, without `dest` and its parents (what
    /// "Move selected here" on the folder tile `dest` moves).
    pub(super) fn selected_except(&self, dest: &Path) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        for t in &self.tiles {
            let p = &self.entries[t.entry].path;
            if !self.selected.contains(p) || dest.starts_with(p) {
                continue;
            }
            if t.members.len() > 1 && !self.expanded.contains(p) {
                out.extend(t.members.iter().map(|&m| self.entries[m].path.clone()));
            } else {
                out.push(p.clone());
            }
        }
        out.dedup();
        // Develop versions are not files to move (their photo's sidecars go with it).
        out.retain(|p| !self.edited.photo_of.contains_key(p));
        out
    }

    /// Every file of the folder shown (not folders or archives' covers), in the gallery's
    /// order: what batch rename works on without a selection.
    pub(super) fn files(&self) -> Vec<PathBuf> {
        self.tiles
            .iter()
            .map(|t| &self.entries[t.entry])
            .filter(|e| !e.is_container() && !self.edited.photo_of.contains_key(&e.path))
            .map(|e| e.path.clone())
            .collect()
    }

    /// The photos the keyword panel acts on: the selection, or the focused tile.
    pub(in crate::app) fn keyword_targets(&self) -> Vec<PathBuf> {
        self.targets()
    }

    /// The listing whose keywords the filter wants read: a folder (not the Library) with
    /// the filter bar open, not read yet.
    pub(in crate::app) fn listing_tags_wanted(&self) -> Option<u64> {
        (self.active
            && self.filter_open
            && !self.library.on
            && self.dir.is_some()
            && !self.listing_tags.asked_for(self.generation))
        .then_some(self.generation)
    }

    /// The listing's photos (not folders, archives or versions).
    pub(in crate::app) fn listing_photos(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .filter(|e| e.kind == EntryKind::Image && !self.edited.photo_of.contains_key(&e.path))
            .map(|e| e.path.clone())
            .collect()
    }

    /// Keywords read or written: the filter and its facets look again.
    pub(in crate::app) fn keyword_facts_changed(&mut self) {
        if !self.library.on && !self.filter.text.is_empty() {
            self.dirty = true;
        }
        self.facets.stale = true;
    }

    pub(super) fn targets(&self) -> Vec<PathBuf> {
        let focus_in = self
            .focus
            .as_ref()
            .is_some_and(|f| self.selected.contains(f));
        let mut out: Vec<PathBuf> =
            if !self.selected.is_empty() && (focus_in || self.focus.is_none()) {
                self.tiles
                    .iter()
                    .filter(|t| self.selected.contains(&self.entries[t.entry].path))
                    .map(|t| self.entries[t.entry].path.clone())
                    .collect()
            } else {
                self.focus.iter().cloned().collect()
            };
        // Collapsed stacks act on every member.
        let mut extra = Vec::new();
        for p in &out {
            if let Some(ti) = self.tile_of(p)
                && let Some(t) = self.tiles.get(ti)
                && t.members.len() > 1
                && !self.expanded.contains(p)
            {
                extra.extend(
                    t.members
                        .iter()
                        .map(|&m| self.entries[m].path.clone())
                        .filter(|m| m != p),
                );
            }
        }
        out.extend(extra);
        // Develop versions are not files to act on (their photo's sidecars go with it).
        out.retain(|p| !self.edited.photo_of.contains_key(p));
        out
    }

    /// Tiles shown (files, folders and archives).
    pub(super) fn tiles_len(&self) -> usize {
        self.tiles.len()
    }

    /// What the tiles are: the album shown (its full name), and whether a search narrows
    /// them.
    pub(super) fn shown_scope(&self) -> (Option<String>, bool) {
        let album = self
            .library
            .albums
            .open
            .and_then(|id| self.album(id))
            .map(|a| a.name.clone());
        (album, !self.filter.is_empty())
    }

    /// Image paths of the tiles, in gallery order (what the viewer steps through).
    pub(super) fn image_order(&self) -> Vec<PathBuf> {
        self.tiles
            .iter()
            .map(|t| &self.entries[t.entry])
            .filter(|e| e.kind == EntryKind::Image && !self.edited.photo_of.contains_key(&e.path))
            .map(|e| e.path.clone())
            .collect()
    }
}

// ----- Model ------------------------------------------------------------------------------

impl Gallery {
    fn set_dir(&mut self, dir: PathBuf) {
        self.library_off();
        // A new place: where it came from goes on the history, and forward is forgotten.
        let step = std::mem::take(&mut self.history_step);
        if let Some(old) = self.dir.as_ref().filter(|&o| *o != dir)
            && !step
        {
            self.back.push(old.clone());
            if self.back.len() > HISTORY_MAX {
                self.back.remove(0);
            }
            self.forward.clear();
        }
        if let Some(old) = self.dir.take() {
            self.positions
                .insert(old, (self.scroll, self.focus.clone()));
        }
        self.dir = Some(dir);
        self.announce = None;
        self.entries.clear();
        self.by_path.clear();
        self.keys.clear();
        self.sorted.clear();
        self.tiles.clear();
        self.entry_tile.clear();
        self.layout = Layout::default();
        self.selected.clear();
        self.expanded.clear();
        self.edited.clear();
        self.versions.clear();
        self.merge_of.clear();
        self.filter = Filter::default();
        self.filter_open = false;
        self.filter_focus = false;
        self.facets.close();
        self.facets.facets.clear();
        self.facets.generation += 1;
        self.compare = None;
        self.quick = None;
        self.info = None;
        self.hover = None;
        self.press = None;
        self.list_error = None;
        let (scroll, focus) = self
            .dir
            .as_ref()
            .and_then(|d| self.positions.get(d).cloned())
            .unwrap_or((0.0, None));
        self.scroll = scroll;
        self.scroll_target = scroll;
        self.focus = focus;
        self.anchor = self.focus.clone();
        self.wish_key = 0;
    }

    fn add_entries(&mut self, chunk: Vec<Entry>) {
        let loc = I18n::global().localizer();
        let mut keyer = loc.natural_keyer();
        let keys = chunk.iter().map(|e| keyer.key(&e.name)).collect();
        drop(keyer);
        self.add_keyed(chunk, keys, &loc);
    }

    /// Adds (or replaces) entries with their names' sort keys, made with `loc`.
    /// Whether a listing's [`Presorted`] order is this gallery's: the same entries (all of
    /// them appended by that listing, `appended`), keys, locale and sort.
    fn presorted_fits(&self, p: &Presorted, appended: Option<usize>) -> bool {
        let n = self.entries.len();
        let loc = I18n::global().localizer();
        appended == Some(n)
            && p.order.len() == n
            && self.keys.len() == n
            && p.view == (self.view.sort, self.view.desc)
            && Arc::ptr_eq(&p.loc, &loc)
            && self.keys_loc.as_ref().is_some_and(|l| Arc::ptr_eq(l, &loc))
    }

    fn add_keyed(&mut self, chunk: Vec<Entry>, keys: Vec<NaturalKey>, loc: &Arc<Localizer>) {
        // Keys from another locale (switched while listing) are remade by the next sort.
        if !self.keys_loc.as_ref().is_some_and(|l| Arc::ptr_eq(l, loc)) {
            if self.entries.is_empty() {
                self.keys_loc = Some(loc.clone());
            } else {
                self.keys_loc = None;
            }
        }
        self.keys.resize(self.entries.len(), NaturalKey::default());
        for (e, k) in chunk.into_iter().zip(keys) {
            if let Some(&i) = self.by_path.get(e.path.as_path()) {
                self.entries[i] = e;
                self.keys[i] = k;
            } else {
                let shared = self.shared_path(&e.path);
                self.by_path.insert(shared, self.entries.len());
                self.entries.push(e);
                self.keys.push(k);
            }
        }
    }

    fn remove_entries(&mut self, paths: &[PathBuf]) {
        let gone: HashSet<&PathBuf> = paths.iter().collect();
        if !self.entries.iter().any(|e| gone.contains(&e.path)) {
            return;
        }
        // The focus moves to the tile that takes the removed one's place (found while the
        // tiles still match the entries).
        if let Some(f) = self.focus.clone()
            && gone.contains(&f)
            && let Some(ti) = self.tile_of(&f)
        {
            let next = (ti + 1..self.tiles.len())
                .chain((0..ti).rev())
                .filter_map(|k| self.tile_path(k))
                .find(|p| !gone.contains(p))
                .cloned();
            self.focus = next;
        }
        if self.keys.len() == self.entries.len() {
            let mut k = std::mem::take(&mut self.keys).into_iter();
            self.keys = self
                .entries
                .iter()
                .filter_map(|e| k.next().filter(|_| !gone.contains(&e.path)))
                .collect();
        }
        // Entry indices shift: the tile of each kept entry moves with it.
        if !self.entry_tile.is_empty() {
            let mut t = std::mem::take(&mut self.entry_tile).into_iter();
            self.entry_tile = self
                .entries
                .iter()
                .map_while(|e| t.next().map(|t| (e, t)))
                .filter(|(e, _)| !gone.contains(&e.path))
                .map(|(_, t)| t)
                .collect();
        }
        if let Some(seen) = self.refresh_seen.as_mut() {
            let mut s = std::mem::take(seen).into_iter();
            *seen = self
                .entries
                .iter()
                .map(|e| (e, s.next().unwrap_or(false)))
                .filter(|(e, _)| !gone.contains(&e.path))
                .map(|(_, s)| s)
                .collect();
        }
        let kept: Vec<bool> = self
            .entries
            .iter()
            .map(|e| !gone.contains(&e.path))
            .collect();
        self.drop_tiles_of(&kept);
        self.entries.retain(|e| !gone.contains(&e.path));
        self.reindex();
        for p in paths {
            self.metas.remove(p.as_path());
            self.averages.remove(p.as_path());
            self.selected.remove(p);
            if let Some(a) = self.atlas.as_mut() {
                a.remove(p);
            }
        }
        self.resort();
    }

    fn resort(&mut self) {
        if self.library.on && self.library.relevance {
            // Found by description: best first, as the search returned them.
            self.sorted = (0..self.entries.len()).collect();
            self.dirty = true;
            return;
        }
        let loc = I18n::global().localizer();
        let t0 = Instant::now();
        // Natural keys turn most name comparisons into a byte compare (sorting 100 000 names
        // with the collator directly took 120–180 ms). Remade if the locale changed or an
        // entry came in without one.
        let fresh = self.keys_loc.as_ref().is_some_and(|l| Arc::ptr_eq(l, &loc));
        if !fresh || self.keys.len() != self.entries.len() {
            let mut keyer = loc.natural_keyer();
            self.keys = self.entries.iter().map(|e| keyer.key(&e.name)).collect();
            drop(keyer);
            self.keys_loc = Some(loc.clone());
        }
        let t_keys = t0.elapsed();
        let view = (self.view.sort, self.view.desc);
        let order = sort_order(&self.entries, &self.keys, view, &loc);
        debug!(target: UI, "gallery: sorted {} in {:.1} ms (keys {:.1} ms)", order.len(), t0.elapsed().as_secs_f64() * 1e3, t_keys.as_secs_f64() * 1e3);
        self.sorted = order;
        self.dirty = true;
    }

    /// Sort order → filter → stacks → groups → tiles → layout.
    fn rebuild(&mut self) {
        self.dirty = false;
        let anchor = self.anchor_path();
        let stacks_on = self.stacks_on;
        let marks = &self.marks;
        let metas = &self.metas;
        // The Library's rows already match its query (searched in the catalogue).
        let matcher = if self.library.on {
            specular_query::Matcher::new(&specular_query::Query::default(), model::fold)
        } else {
            self.filter.matcher()
        };
        self.filter_needs_meta = matcher.needs_meta();
        self.facets.stale = true;
        let sets = self.filter.uses_sets();
        let sim = &self.similar;
        let entries = &self.entries;
        let (edited, versions) = (&self.edited, &self.versions);
        let tags = &self.listing_tags;
        // No filter: everything shows (skips 2 path lookups per entry, ~5 ms at 100 000).
        let all = matcher.is_empty();
        let visible: Vec<usize> = self
            .sorted
            .iter()
            .copied()
            .filter(|&i| {
                if all {
                    return true;
                }
                let e = &entries[i];
                let role = if sets {
                    sim.role(&e.path)
                } else {
                    (None, None)
                };
                let mut f = edited::develop_facts(
                    edited,
                    versions,
                    e,
                    model::facts(
                        e,
                        metas.get(e.path.as_path()).map(|m| m.as_ref()),
                        marks.mark(&e.path),
                        role,
                    ),
                );
                f.tags = tags.of(&e.path);
                matcher.matches(&f)
            })
            .collect();
        let groups: Vec<(usize, Vec<usize>)> = if stacks_on {
            let meta = |p: &Path| metas.get(p).map(|m| (m.date_taken, m.camera.clone()));
            sim.merge_stacks(model::stacks(entries, &visible, &meta), entries)
        } else {
            visible.iter().map(|&i| (i, vec![i])).collect()
        };
        // A whole-slide image stored as several DICOM files is one tile, stacks on or off.
        let groups = model::merge_slides(groups, entries, &self.slides);
        // Develop versions stack under their photo, stacks on or off.
        let groups = edited::group_versions(groups, entries, &self.edited.photo_of);
        // HDR merges stack with their sources, stacks on or off.
        let groups = merged::group_merges(groups, entries, &self.merge_of);
        let mut tiles: Vec<Tile> = Vec::with_capacity(groups.len());
        for (shown, members) in groups {
            let path = &entries[shown].path;
            if members.len() > 1 && self.expanded.contains(path) {
                for &m in &members {
                    tiles.push(Tile {
                        entry: m,
                        members: members.clone(),
                        group: 0,
                    });
                }
            } else {
                tiles.push(Tile {
                    entry: shown,
                    members: if members.len() > 1 {
                        members
                    } else {
                        Vec::new()
                    },
                    group: 0,
                });
            }
        }
        // Groups: contiguous runs; tiles are ordered by group first (stable within).
        self.group_labels.clear();
        if self.view.group != GalleryGroup::None {
            let root = self.dir.clone().unwrap_or_default();
            let mut keys: Vec<GroupKey> = tiles
                .iter()
                .map(|t| {
                    let e = &entries[t.entry];
                    model::group_key(
                        self.view.group,
                        e,
                        metas.get(e.path.as_path()).map(|m| m.as_ref()),
                        marks.mark(&e.path),
                        &root,
                    )
                })
                .collect();
            if self.view.group == GalleryGroup::Event {
                let shots: Vec<_> = tiles
                    .iter()
                    .map(|t| {
                        let e = &entries[t.entry];
                        model::shot(e, metas.get(e.path.as_path()).map(|m| m.as_ref()))
                    })
                    .collect();
                keys = model::event_keys(&shots);
            }
            let places = self.event_places(&keys);
            let desc = self.view.desc;
            let mut idx: Vec<usize> = (0..tiles.len()).collect();
            idx.sort_by(|&a, &b| group_order(&keys[a], &keys[b], desc));
            let mut out = Vec::with_capacity(tiles.len());
            let mut last: Option<&GroupKey> = None;
            for i in idx {
                if last != Some(&keys[i]) {
                    self.group_labels
                        .push(group_label(&keys[i], places.as_ref()));
                    last = Some(&keys[i]);
                }
                let mut t = tiles[i].clone();
                t.group = (self.group_labels.len() - 1) as u32;
                out.push(t);
            }
            tiles = out;
        }
        self.tiles = tiles;
        self.entry_tile.clear();
        self.entry_tile.resize(self.entries.len(), NO_TILE);
        for (i, t) in self.tiles.iter().enumerate() {
            for &m in &t.members {
                if self.entry_tile[m] == NO_TILE {
                    self.entry_tile[m] = i as u32;
                }
            }
            self.entry_tile[t.entry] = i as u32;
        }
        let (by_path, entry_tile) = (&self.by_path, &self.entry_tile);
        self.selected
            .retain(|p| tile_in(by_path, entry_tile, p).is_some());
        self.relayout_now(anchor, true);
    }

    /// The path under the anchor point (pointer, else focus, else top) and where.
    fn anchor_path(&self) -> Option<(PathBuf, f32, f32)> {
        let py = self
            .pointer
            .filter(|p| self.pointer_input && p.1 >= self.grid_top())
            .map_or(0.0, |p| p.1 - self.grid_top());
        let px = self.pointer.map_or(0.0, |p| p.0);
        let (i, frac) = self.layout.anchor(self.scroll, px, py)?;
        let path = self.tile_path(i)?.clone();
        Some((path, frac, py))
    }

    fn relayout_now(&mut self, anchor: Option<(PathBuf, f32, f32)>, rebuilt: bool) {
        self.relayout = false;
        let items: Vec<Item> = self
            .tiles
            .iter()
            .map(|t| {
                let e = &self.entries[t.entry];
                Item {
                    aspect: if e.is_container() {
                        1.0
                    } else {
                        self.aspect_of(e).unwrap_or(1.0)
                    },
                    group: t.group,
                }
            })
            .collect();
        let params = self.params();
        self.layout = Layout::build(&items, &params, self.view.mode);
        self.layout.tail = if self.sel_bar.is_some() {
            SEL_BAR_ROOM
        } else {
            0.0
        };
        let vh = self.grid_h();
        let anchor = anchor.and_then(|(p, frac, py)| Some(((self.tile_of(&p)?, frac), py)));
        let was = self.scroll;
        let before = self.scroll.max(self.scroll_target);
        if let Some(s) = model::relaid_scroll(&self.layout, before, anchor, vh, rebuilt) {
            self.scroll = s;
            self.scroll_target = s;
        }
        self.scroll = self.layout.clamp_scroll(self.scroll, vh);
        self.scroll_target = self.layout.clamp_scroll(self.scroll_target, vh);
        self.unmoved(was);
        self.wish_key = 0;
        if self.stats.first_grid.is_none()
            && !self.tiles.is_empty()
            && let Some(t) = self.stats.entered
        {
            self.stats.first_grid = Some(t.elapsed());
        }
    }

    /// Puts the focus on tile `i` and scrolls it into view.
    pub(super) fn set_focus(&mut self, i: usize) {
        let Some(p) = self.path_of(i) else { return };
        self.focus = Some(p);
        // Clear of the selection bar too, when it shows over the grid's end.
        let vh = (self.grid_h() - self.layout.tail).max(1.0);
        let inset = if self.layout.headers.is_empty() {
            0.0
        } else {
            self.layout.params.map_or(0.0, |p| p.header)
        };
        self.scroll_target = self.layout.scroll_to_show(i, self.scroll_target, vh, inset);
    }
}

impl Gallery {
    /// Names of the event keys' places, for their headers (by the key's place). None when the
    /// headers name no places: not grouped by event, the setting is off, no event has a
    /// position, or the geocoder's index is not built yet (then `places_waiting` asks for it).
    fn event_places(&mut self, keys: &[GroupKey]) -> Option<HashMap<(i32, i32), String>> {
        self.places_waiting = false;
        if !self.places || self.view.group != GalleryGroup::Event {
            return None;
        }
        let mut names: HashMap<(i32, i32), String> = HashMap::new();
        let mut at = keys.iter().filter_map(|k| match k {
            GroupKey::Event(_, _, p) => *p,
            _ => None,
        });
        let first = at.next()?;
        if !specular_metadata::geocoder_ready() {
            self.places_waiting = true;
            return None;
        }
        let lang = I18n::global().language();
        if self.place_loc.as_ref().is_none_or(|l| *l.langid() != lang) {
            self.place_loc = Some(specular_metadata::Localizer::new(lang));
        }
        let loc = self.place_loc.as_ref()?;
        for p in std::iter::once(first).chain(at) {
            if names.contains_key(&p) {
                continue;
            }
            let deg = |m: i32| f64::from(m) / 1e6;
            if let Some(mut place) = specular_metadata::geocode(deg(p.0), deg(p.1)) {
                place.country = loc.country_name(&place.country_code);
                names.insert(p, place.display_name(loc));
            }
        }
        Some(names)
    }
}

fn group_order(a: &GroupKey, b: &GroupKey, desc: bool) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (GroupKey::Unknown, GroupKey::Unknown) => Ordering::Equal,
        (GroupKey::Unknown, _) => Ordering::Greater,
        (_, GroupKey::Unknown) => Ordering::Less,
        (GroupKey::Date(..), GroupKey::Date(..)) | (GroupKey::Event(..), GroupKey::Event(..))
            if desc =>
        {
            b.cmp(a)
        }
        // Best ratings first.
        (GroupKey::Rating(x), GroupKey::Rating(y)) => y.cmp(x),
        (GroupKey::Text(x), GroupKey::Text(y)) => I18n::global().natural_cmp(x, y),
        _ => a.cmp(b),
    }
}

/// `entry_tile` value of an entry without a tile (filtered out, or listed since the last
/// rebuild).
const NO_TILE: u32 = u32::MAX;

impl Gallery {
    /// Keeps the tiles on their entries when entries are dropped (`kept[i]`: old entry `i`
    /// stays): indices move with the entries, and a tile whose entries all left goes.
    fn drop_tiles_of(&mut self, kept: &[bool]) {
        let mut next = 0;
        let map: Vec<usize> = kept
            .iter()
            .map(|&k| {
                if k {
                    next += 1;
                    next - 1
                } else {
                    usize::MAX
                }
            })
            .collect();
        remap_tiles(&mut self.tiles, |i| {
            map.get(i).copied().filter(|&n| n != usize::MAX)
        });
    }

    /// The path of tile `i` (its entry's).
    fn tile_path(&self, i: usize) -> Option<&PathBuf> {
        self.tiles.get(i).map(|t| &self.entries[t.entry].path)
    }

    /// The paths of the tiles, in order.
    fn tile_paths(&self) -> impl DoubleEndedIterator<Item = &PathBuf> + ExactSizeIterator + '_ {
        self.tiles.iter().map(|t| &self.entries[t.entry].path)
    }

    /// The one shared copy of `p` that keys `by_path`, `metas` and `averages` (a new one for
    /// a path none of them has).
    fn shared_path(&self, p: &Path) -> Arc<Path> {
        shared_in(&self.by_path, &self.metas, &self.averages, p)
    }

    /// Indexes `entries` by path again (indices moved), keeping each path's shared copy.
    fn reindex(&mut self) {
        let mut old = std::mem::take(&mut self.by_path);
        let mut by_path = HashMap::with_capacity(self.entries.len());
        for (i, e) in self.entries.iter().enumerate() {
            let k = match old.remove_entry(e.path.as_path()) {
                Some((k, _)) => k,
                None => shared_in(&old, &self.metas, &self.averages, &e.path),
            };
            by_path.insert(k, i);
        }
        self.by_path = by_path;
    }

    /// Drops the facts, average colours and folder covers of paths no longer listed (a
    /// folder's cover images keep their average colours). At 100 000 files the maps held a
    /// path copy each for every folder ever shown.
    /// `metas_too`: false when `metas` was just made for the listing (the Library's rows).
    /// The average colours kept take the listing's shared copy of their path.
    fn trim_to_listing(&mut self, metas_too: bool) {
        // A listing grows its entries by doubling: up to half of them is unused room once it
        // is complete (13.6 MB for 10.4 MB of entries at 100,000 files).
        shrink_loose(&mut self.entries);
        shrink_loose(&mut self.keys);
        let by_path = &self.by_path;
        self.covers
            .retain(|dir, _| by_path.contains_key(dir.as_path()));
        let covers: HashSet<&Path> = self
            .covers
            .values()
            .flatten()
            .map(PathBuf::as_path)
            .collect();
        if metas_too {
            self.metas.retain(|p, _| by_path.contains_key(p));
            self.metas.shrink_to_fit();
        }
        let averages = std::mem::take(&mut self.averages);
        self.averages = averages
            .into_iter()
            .filter_map(|(p, a)| match by_path.get_key_value(&*p) {
                Some((k, _)) => Some((k.clone(), a)),
                None => covers.contains(&*p).then_some((p, a)),
            })
            .collect();
    }
}

/// Points the tiles at their entries' new indices (`new(old)`, `None` when the entry left);
/// a tile whose entries all left goes. Until the next rebuild the tiles are still read (the
/// anchor, the focus), so they must never point past the entries.
fn remap_tiles(tiles: &mut Vec<Tile>, new: impl Fn(usize) -> Option<usize>) {
    tiles.retain_mut(|t| {
        t.members.retain_mut(|m| new(*m).map(|n| *m = n).is_some());
        match new(t.entry).or_else(|| t.members.first().copied()) {
            Some(n) => {
                t.entry = n;
                true
            }
            None => false,
        }
    });
}

/// Gives back `v`'s unused room when it is more than an eighth of what is used.
fn shrink_loose<T>(v: &mut Vec<T>) {
    if v.capacity() - v.len() > v.len() / 8 {
        v.shrink_to_fit();
    }
}

#[cfg(test)]
#[test]
fn shrink_loose_gives_back_only_much_room() {
    let mut v: Vec<u8> = Vec::with_capacity(200);
    v.extend([0; 100]);
    shrink_loose(&mut v);
    assert_eq!(v.capacity(), 100);
    v.reserve_exact(10);
    let cap = v.capacity();
    shrink_loose(&mut v);
    assert_eq!(v.capacity(), cap, "a little room stays");
}

/// [`Gallery::shared_path`] over the three maps.
fn shared_in(
    by_path: &HashMap<Arc<Path>, usize>,
    metas: &HashMap<Arc<Path>, Arc<Meta>>,
    averages: &HashMap<Arc<Path>, [u8; 4]>,
    p: &Path,
) -> Arc<Path> {
    by_path
        .get_key_value(p)
        .map(|(k, _)| k)
        .or_else(|| metas.get_key_value(p).map(|(k, _)| k))
        .or_else(|| averages.get_key_value(p).map(|(k, _)| k))
        .cloned()
        .unwrap_or_else(|| Arc::from(p))
}

/// [`Gallery::tile_of`] over the two fields, for callers holding other fields mutably.
fn tile_in(by_path: &HashMap<Arc<Path>, usize>, entry_tile: &[u32], p: &Path) -> Option<usize> {
    let &e = by_path.get(p)?;
    entry_tile
        .get(e)
        .copied()
        .filter(|&t| t != NO_TILE)
        .map(|t| t as usize)
}

/// A queued upload: path, class, pixels, sharp.
type Upload = (PathBuf, u32, Arc<Pixels>, bool);

/// Drops queued uploads no longer wished (scrolled out of the wish list `wish_tiles`, not in
/// `wish_extra`), wherever they are in the queue, then the newest while the queue holds more
/// than [`UPLOAD_QUEUE_BYTES`].
fn trim_uploads(
    uploads: &mut VecDeque<Upload>,
    by_path: &HashMap<Arc<Path>, usize>,
    entry_tile: &[u32],
    wish_tiles: &[Range<usize>; 3],
    wish_extra: &HashSet<PathBuf>,
) {
    uploads.retain(|(path, ..)| {
        wish_extra.contains(path)
            || tile_in(by_path, entry_tile, path)
                .is_some_and(|t| wish_tiles.iter().any(|r| r.contains(&t)))
    });
    let mut bytes = 0u64;
    let keep = uploads
        .iter()
        .take_while(|u| {
            bytes += u.2.rgba.len() as u64;
            bytes <= UPLOAD_QUEUE_BYTES
        })
        .count();
    uploads.truncate(keep.max(1));
}

/// A group's header; `places`: the names of event places (see [`Gallery::event_places`]).
fn group_label(k: &GroupKey, places: Option<&HashMap<(i32, i32), String>>) -> String {
    let l = I18n::global().localizer();
    match k {
        GroupKey::None => String::new(),
        GroupKey::Date(y, 0, 0) => y.to_string(),
        GroupKey::Date(y, m, 0) => {
            // "May 2024" in the locale's own order and forms; a heading starts with a capital
            // even where the month name is lower case ("Květen 2024").
            let Some(label) = l.year_month(*y, *m) else {
                return format!("{y}-{m:02}");
            };
            let mut chars = label.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        }
        GroupKey::Date(y, m, d) => l
            .date(*y, *m, *d, specular_i18n::DateStyle::Long)
            .unwrap_or_else(|| format!("{y}-{m:02}-{d:02}")),
        GroupKey::Event(a, b, at) => {
            let day = |t: i64| {
                let (y, m, d) = model::civil_from_days(t.div_euclid(86_400));
                l.date(y, m, d, specular_i18n::DateStyle::Long)
                    .unwrap_or_else(|| format!("{y}-{m:02}-{d:02}"))
            };
            let (start, end) = (day(*a), day(*b));
            let dates = if start == end {
                start
            } else {
                t!("gallery-group-event", start = start, end = end)
            };
            match at.and_then(|p| places?.get(&p)) {
                Some(place) => t!(
                    "gallery-group-event-place",
                    place = place.as_str(),
                    dates = dates
                ),
                None => dates,
            }
        }
        GroupKey::Text(s) if s.is_empty() => t!("gallery-group-this-folder"),
        GroupKey::Text(s) => s.clone(),
        GroupKey::Rating(0) => t!("gallery-group-unrated"),
        GroupKey::Rating(r) => t!("gallery-stars", stars = *r as i64),
        GroupKey::Unknown => t!("gallery-group-unknown"),
    }
}

/// A file or folder entry for a path seen by the watcher.
fn entry_for(path: &Path, classify: &dyn Fn(&Path) -> EntryKind) -> Option<Entry> {
    let md = std::fs::metadata(path).ok()?;
    let name = path.file_name()?.to_string_lossy().into_owned();
    let kind = if !md.is_dir() {
        classify(path)
    } else if specular_core::is_image_folder(path) && classify(path) == EntryKind::Image {
        // A folder that opens as one image (CASA, OME-Zarr, Relight), as the listing has it.
        EntryKind::Image
    } else {
        EntryKind::Folder
    };
    Some(Entry {
        path: path.to_path_buf(),
        hidden: name.starts_with('.'),
        name,
        kind,
        len: if md.is_dir() { 0 } else { md.len() },
        modified: md.modified().ok(),
        taken: None,
        dims: None,
    })
}

/// A listing sorted on its own thread (see [`Msg::Done`]).
struct Presorted {
    order: Vec<usize>,
    view: (SortKey, bool),
    loc: Arc<Localizer>,
}

/// The display order of `entries` (indices) for the sort `view`, with `keys` their natural
/// keys made by `loc`. The order is total (names are unique in a listing; the Library's rows
/// fall back to their paths), so an unstable sort is exact, on any number of threads (large
/// folders use four).
fn sort_order(
    entries: &[Entry],
    keys: &[NaturalKey],
    (key, desc): (SortKey, bool),
    loc: &Localizer,
) -> Vec<usize> {
    let cmp = |a: &str, b: &str| loc.natural_cmp(a, b);
    let mut order: Vec<usize> = (0..entries.len()).collect();
    sort::par_sort_unstable_by(&mut order, 4, |&a, &b| {
        let (ea, eb) = (&entries[a], &entries[b]);
        sort::compare_by(ea, eb, key, desc, &cmp, || {
            loc.natural_cmp_keyed((&keys[a], &ea.name), (&keys[b], &eb.name))
        })
        .then_with(|| ea.path.cmp(&eb.path))
    });
    order
}

/// The archive a book page belongs to (`…/book.cbz/page.jpg` → `…/book.cbz`).
fn archive_of(page: &Path) -> Option<PathBuf> {
    page.ancestors()
        .skip(1)
        .find(|a| crate::book::is_archive_path(a) && a.is_file())
        .map(Path::to_path_buf)
}

// ----- Entering, leaving, opening ---------------------------------------------------------

impl App {
    /// The folder listing panicked: the gallery stops showing it as listing, and says why.
    pub(in crate::app) fn gallery_lost(&mut self, task: &str) {
        if task == "gal-list" {
            let ga = &mut self.gallery;
            ga.listing = false;
            ga.list_error = Some(t!("error-reason-internal"));
        }
    }

    /// The files with these extensions get their thumbnails made again (an optional decoder
    /// was found or lost): their failures and cached tiles are forgotten.
    pub(in crate::app) fn gallery_retry_extensions(&mut self, exts: &[&str]) {
        let paths: Vec<std::path::PathBuf> = self
            .gallery
            .entries
            .iter()
            .filter(|e| {
                e.path
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| exts.iter().any(|w| x.eq_ignore_ascii_case(w)))
            })
            .map(|e| e.path.clone())
            .collect();
        for p in paths {
            if let Some(engine) = &self.gallery.engine {
                engine.invalidate(&p);
            }
            if let Some(a) = self.gallery.atlas.as_mut() {
                a.remove(&p);
            }
            self.gallery.metas.remove(p.as_path());
            self.gallery.failed.remove(&p);
        }
    }

    /// The gallery takes the window's input and drawing.
    pub(super) fn gallery_active(&self) -> bool {
        self.gallery.active
    }

    /// The window title while the gallery shows: the Library, or the folder's name.
    pub(super) fn gallery_title(&self) -> String {
        if self.gallery.library.on {
            return t!("library-title");
        }
        match self.gallery.dir.as_deref() {
            Some(d) if d.file_name().is_some() => file_name(d),
            Some(d) => d.display().to_string(),
            None => "Specular".into(),
        }
    }

    /// Compare is open over the gallery.
    pub(super) fn gallery_comparing(&self) -> bool {
        self.gallery.active && self.gallery.compare.is_some()
    }

    /// `g`: into the gallery from the viewer; in the gallery, open the focused image (or go
    /// back to the one the viewer showed).
    pub(super) fn gallery_toggle(&mut self) {
        if !self.gallery.active {
            self.gallery_enter();
            return;
        }
        match self.gallery.focus_index() {
            Some(i)
                if self
                    .gallery
                    .entry_of(i)
                    .is_some_and(|e| e.kind == EntryKind::Image) =>
            {
                self.gallery_open(i)
            }
            _ => self.gallery_leave(),
        }
    }

    /// Shows the gallery of the current image's folder (the archive's folder for a book).
    pub(super) fn gallery_enter(&mut self) {
        let cur = self.current.and_then(|i| self.entries.get(i)).cloned();
        let (dir, focus) = match &cur {
            Some(p) => match archive_of(p) {
                Some(a) => (a.parent().map(Path::to_path_buf), Some(a)),
                None => (p.parent().map(Path::to_path_buf), Some(p.clone())),
            },
            None => (self.folder.clone(), None),
        };
        self.gallery_enter_at(dir, focus);
    }

    pub(super) fn gallery_enter_at(&mut self, dir: Option<PathBuf>, focus: Option<PathBuf>) {
        let now = self.now();
        self.gallery_prepare();
        self.gallery.from_viewer = self.current.is_some();
        self.gallery.stats = FillStats {
            entered: Some(Instant::now()),
            ..FillStats::default()
        };
        if let Some(dir) = dir {
            if self.gallery.dir.as_ref() != Some(&dir) || self.gallery.list_error.is_some() {
                self.gallery_open_dir(dir);
            } else if !self.gallery.tiles.is_empty() {
                self.gallery.stats.first_grid = Some(Duration::ZERO);
            }
        }
        if let Some(f) = focus {
            self.gallery.focus = Some(f.clone());
            self.gallery.anchor = Some(f);
            self.gallery.reveal = true;
        }
        let was = self.gallery.active;
        self.gallery.active = true;
        self.chrome.gallery = true;
        self.chrome.gallery_bar = BOTTOM_BAR;
        self.hold = None;
        if !was {
            self.gallery.transition = Some(Transition {
                start: now,
                duration: if self.reduce_motion() {
                    FADE_S
                } else {
                    TRANSITION_S
                },
                opening: false,
                path: self.gallery.focus.clone(),
                tile: Rect::default(),
                image: self.gallery_image_rect(),
                fade_only: self.reduce_motion() || !self.gallery.from_viewer,
            });
        }
        debug!(target: UI, "gallery: enter {:?}", self.gallery.dir);
        // The thumbnails' insights indicator replaces the opens'.
        self.refresh_insights();
        self.redraw = true;
    }

    /// The thumbnail cache: the gallery's, else one on the folder the gallery would use (an
    /// archive opened before the gallery keeps what its ComicInfo says there,
    /// [`crate::archive_thumbs::remember_comic`]).
    pub(super) fn thumb_cache(&self) -> Option<Arc<DiskCache>> {
        if let Some(c) = self.gallery.engine.as_ref().and_then(|e| e.cache()) {
            return Some(c.clone());
        }
        let g = &self.settings.gallery;
        let dir = g.cache_location.clone().or_else(default_cache_dir)?;
        Some(Arc::new(DiskCache::new(
            dir,
            (g.cache_size.max(64) as u64) << 20,
        )))
    }

    /// The thumbnail engine's cache-key flags (0 before the gallery made one).
    pub(super) fn thumb_flags(&self) -> u32 {
        self.gallery.engine.as_ref().map_or(0, |e| e.config().flags)
    }

    /// The reading mode has read the ComicInfo of `book` (an archive the thumbnails may not
    /// have reached it in): its tile shows the pages the reader does.
    pub(super) fn book_tile_known(&mut self, book: &crate::book::Book) {
        let ga = &mut self.gallery;
        if let Some(a) = &ga.archives {
            a.forget(book.path());
        }
        // Unlocked: its pages make the tile now.
        ga.locked.remove(book.path());
        if let Some(c) = ga.covers.get_mut(book.path()) {
            let now = book.covers(crate::archive_thumbs::COVERS);
            if *c != now {
                *c = now;
                ga.wish_key = 0;
            }
        }
    }

    /// Leaves Develop (its edit saved: it draws over the viewer, not the gallery); then the
    /// engine, marks and settings, the first time the gallery opens.
    fn gallery_prepare(&mut self) {
        self.develop_leave();
        if self.gallery.engine.is_none() {
            let g = &self.settings.gallery;
            let cache_dir = g.cache_location.clone().or_else(default_cache_dir);
            let budget = (g.cache_size.max(64) as u64) << 20;
            let cache = cache_dir.map(|d| {
                debug!(target: CACHE, "thumbnail cache {}", d.display());
                Arc::new(DiskCache::new(d, budget))
            });
            let mut cfg = EngineConfig::default();
            cfg.generate.prefer_embedded = g.prefer_embedded;
            // Class-256 tiles carry their duplicate-finder signature (DESIGN.md §10).
            cfg.digester = Some(specular_similar::scan::digester());
            // Archive pages (the archive tiles' mosaics) through the engine too, each archive
            // opened once for its pages (or read through the viewer's open book), the tiles'
            // pages kept in the cache.
            let registry = self.loader.registry();
            let (tx, wake) = (self.gallery.tx.clone(), self.loader.waker());
            let archives = Arc::new(
                ArchiveOpener::new(Arc::new(move |p: &Path| registry.supports_extension(p)))
                    .with_cache(cache.clone())
                    .with_books(self.loader.books().clone())
                    .with_cover_updates(Arc::new(move |a: &Path, images| {
                        let archive = a.to_path_buf();
                        if tx.send(Msg::TileKnown { archive, images }).is_ok() {
                            wake();
                        }
                    })),
            );
            self.gallery.covers_in =
                CoverLister::start(&self.loader, archives.clone(), self.gallery.tx.clone());
            self.gallery.archives = Some(archives.clone());
            let wake = self.loader.waker();
            self.gallery.engine = Some(Engine::new(
                cfg,
                self.loader.registry(),
                self.loader.thumb_pool(),
                archives,
                cache,
                Arc::new(move || wake()),
            ));
        }
        self.gallery_marks_ready();
        self.gallery_sync_settings();
    }

    /// `(rating, label, flag)` of each of `paths` (remote control, `--print-marked`): the
    /// gallery's marks once it has opened or anything marked (the viewer, cull, the remote
    /// control: their marks may not be saved yet), else the saved tables (read afresh).
    pub(super) fn marks_for(&self, paths: &[PathBuf]) -> Vec<(u8, u8, i8)> {
        let saved;
        let store = if self.gallery.engine.is_some() || self.gallery.marks_made {
            &self.gallery.marks
        } else {
            let dir = (!self.headless())
                .then(|| {
                    self.store
                        .path()
                        .and_then(|p| p.parent().map(Path::to_path_buf))
                })
                .flatten();
            let mut s = Store::new(dir);
            s.load();
            saved = s;
            &saved
        };
        paths
            .iter()
            .map(|p| {
                let m = store.mark(p);
                (m.rating, m.label, m.flag)
            })
            .collect()
    }

    /// The `[gallery]` settings into the gallery (on entering and when they change).
    pub(super) fn gallery_sync_settings(&mut self) {
        let g = self.settings.gallery.clone();
        let ga = &mut self.gallery;
        let relist = ga.show_hidden != g.show_hidden
            || ga.show_other != g.show_other_files
            || ga.show_archives != g.show_archives
            || ga.depth != g.subfolder_depth.clamp(0, 16) as u32;
        ga.show_hidden = g.show_hidden;
        ga.show_other = g.show_other_files;
        ga.show_archives = g.show_archives;
        ga.depth = g.subfolder_depth.clamp(0, 16) as u32;
        if ga.show_captions != g.captions {
            ga.show_captions = g.captions;
            ga.relayout = true;
        }
        ga.show_badges = g.badges;
        if ga.edited_badges != g.edited_badges {
            ga.edited_badges = g.edited_badges;
            ga.dirty = true;
        }
        ga.info_overlay = g.info_overlay;
        // Raws' edited thumbnails at the new develop size (their own cache entries; the
        // old ones show until the new are made).
        if ga
            .edited
            .worker
            .set_raw_long(specular_develop::thumb::raw_long(g.sharper_raw_thumbnails))
        {
            ga.edited.ask_raws_again();
        }
        if ga.stacks_on != g.stacks {
            ga.stacks_on = g.stacks;
            ga.dirty = true;
        }
        if !g.per_folder_view {
            let v = ViewOpts::from_settings(&self.settings);
            if v != ga.view {
                if v.sort != ga.view.sort || v.desc != ga.view.desc {
                    ga.view = v;
                    ga.resort();
                } else {
                    ga.view = v;
                    ga.dirty = true;
                }
            }
        }
        if let Some(e) = &ga.engine {
            let mut cfg = e.config();
            if cfg.generate.prefer_embedded != g.prefer_embedded {
                cfg.generate.prefer_embedded = g.prefer_embedded;
                e.set_config(cfg);
            }
            if let Some(c) = e.cache() {
                c.set_budget((g.cache_size.max(64) as u64) << 20);
            }
        }
        if relist && let Some(d) = self.gallery.dir.clone() {
            self.gallery_relist(d);
        }
        self.redraw = true;
    }

    /// Lists `dir` and shows it with its remembered view.
    pub(super) fn gallery_open_dir(&mut self, dir: PathBuf) {
        let dir = std::path::absolute(&dir).unwrap_or(dir);
        let per_folder = self.settings.gallery.per_folder_view;
        let view = per_folder
            .then(|| self.gallery.marks.views.get(&dir).copied())
            .flatten()
            .unwrap_or_else(|| ViewOpts::from_settings(&self.settings));
        self.gallery.set_dir(dir.clone());
        self.gallery.view = view;
        self.gallery.size_shown = view.size;
        self.gallery_relist(dir);
    }

    fn gallery_relist(&mut self, dir: PathBuf) {
        let ga = &mut self.gallery;
        if let Some(t) = ga.list_token.take() {
            t.cancel();
        }
        ga.generation += 1;
        ga.listing = true;
        ga.paged_ahead = false;
        ga.list_error = None;
        let keep: Vec<Entry> = std::mem::take(&mut ga.entries);
        let keep_keys = std::mem::take(&mut ga.keys);
        ga.by_path.clear();
        ga.list_started = Some(Instant::now());
        // A relist (watcher overflow, options) keeps showing the old tiles until it is done.
        let refresh = !keep.is_empty();
        ga.list_appended = (!refresh).then_some(0);
        if refresh {
            ga.entries = keep;
            ga.keys = keep_keys;
            ga.reindex();
        }
        ga.refresh_seen = refresh.then(Vec::new);
        let token = CancelToken::new();
        ga.list_token = Some(token.clone());
        let generation = ga.generation;
        let opts = ListOptions {
            show_hidden: ga.show_hidden,
            show_other: ga.show_other,
            show_archives: ga.show_archives,
            show_folders: true,
            depth: ga.depth,
            ..ListOptions::default()
        };
        let tx = ga.tx.clone();
        let wake = self.loader.waker();
        // Sorts that only use what the listing read (not dates or sizes probed later, which
        // may also change meanwhile) are sorted on the listing thread.
        let view = (ga.view.sort, ga.view.desc);
        let presort = !refresh && matches!(view.0, SortKey::Name | SortKey::Type);
        // Off this thread: a deep tree takes one watch per folder. A relist of the same
        // folder and depth (a change, an option) keeps the watch it has.
        if !ga
            .watch
            .as_ref()
            .is_some_and(|w| w.is(&dir, ga.depth, ga.show_hidden) && !w.failed())
        {
            ga.watch = Some(specular_thumbs::watch::TreeWatch::start(
                "gal-watch",
                &dir,
                ga.depth,
                ga.show_hidden,
                wake.clone(),
            ));
        }
        self.loader.background("gal-list", move |l| {
            let classify = |p: &Path| {
                if crate::book::is_archive_path(p) {
                    EntryKind::Archive
                } else if l.supports_file(p) {
                    EntryKind::Image
                } else {
                    EntryKind::Other
                }
            };
            let t0 = Instant::now();
            // Sort keys are made here, off the main thread, while the stat workers run ahead.
            let loc = I18n::global().localizer();
            let mut keyer = loc.natural_keyer();
            let (mut all, mut all_keys) = (Vec::new(), Vec::new());
            let r = list_dir(&dir, opts, &classify, &token, &mut |entries| {
                let keys: Vec<NaturalKey> = entries.iter().map(|e| keyer.key(&e.name)).collect();
                if presort {
                    all.extend_from_slice(&entries);
                    all_keys.extend_from_slice(&keys);
                }
                let _ = tx.send(Msg::Chunk {
                    generation,
                    entries,
                    keys,
                    loc: loc.clone(),
                });
                wake();
            });
            debug!(target: UI, "gallery: listed {} in {:.1} ms: {r:?}", dir.display(), t0.elapsed().as_secs_f64() * 1e3);
            drop(keyer);
            let presorted = (presort && !token.is_cancelled()).then(|| {
                let t0 = Instant::now();
                let order = sort_order(&all, &all_keys, view, &loc);
                debug!(target: UI, "gallery: presorted {} in {:.1} ms", order.len(), t0.elapsed().as_secs_f64() * 1e3);
                Presorted { order, view, loc }
            });
            let _ = tx.send(Msg::Done {
                generation,
                error: r.err().map(|e| crate::error_text::io_reason(&e)),
                presorted,
            });
            wake();
        });
        self.redraw = true;
    }

    /// Back to the image the viewer showed, without opening another.
    pub(super) fn gallery_leave(&mut self) {
        if self.current.is_none() {
            return;
        }
        let now = self.now();
        let path = self.current.and_then(|i| self.entries.get(i)).cloned();
        self.gallery_close(now, path);
    }

    fn gallery_close(&mut self, now: f64, path: Option<PathBuf>) {
        let ga = &mut self.gallery;
        if let Some(d) = ga.dir.clone() {
            ga.positions.insert(d, (ga.scroll, ga.focus.clone()));
        }
        let tile = path
            .as_ref()
            .and_then(|p| ga.tile_of(p))
            .and_then(|i| ga.tile_rect(i))
            .unwrap_or_default();
        ga.active = false;
        ga.quick = None;
        ga.compare = None;
        ga.info = None;
        ga.press = None;
        ga.hover = None;
        let fade = self.reduce_motion() || tile.w <= 0.0;
        let image = self.gallery_image_rect();
        let ga = &mut self.gallery;
        ga.transition = Some(Transition {
            start: now,
            duration: if fade { FADE_S } else { TRANSITION_S },
            opening: true,
            path,
            tile,
            image,
            fade_only: fade,
        });
        self.chrome.gallery = false;
        // Back to the opens' insights indicator.
        self.refresh_insights();
        self.redraw = true;
    }

    /// Where the viewer shows (or will show) the current image, logical px: fitted in the
    /// window, from the thumbnail's aspect ratio.
    fn gallery_image_rect(&self) -> Rect {
        let (w, h) = (self.gallery.vw, self.gallery.vh);
        let aspect = self
            .current
            .and_then(|i| self.entries.get(i))
            .and_then(|p| {
                self.gallery
                    .metas
                    .get(p.as_path())
                    .filter(|m| m.width > 0 && m.height > 0)
                    .map(|m| m.width as f32 / m.height as f32)
            })
            .unwrap_or(1.5);
        let (mut iw, mut ih) = (w, w / aspect);
        if ih > h {
            ih = h;
            iw = h * aspect;
        }
        Rect::new((w - iw) * 0.5, (h - ih) * 0.5, iw, ih)
    }

    /// Opens tile `i`: an image in the viewer, a folder in the gallery, an archive as a book.
    pub(super) fn gallery_open(&mut self, i: usize) {
        let Some(e) = self.gallery.entry_of(i).cloned() else {
            return;
        };
        let now = self.now();
        match e.kind {
            EntryKind::Folder => self.gallery_open_dir(e.path),
            EntryKind::Archive => {
                self.gallery_close(now, None);
                self.gallery.viewer_from_gallery = true;
                self.open_target(e.path);
            }
            EntryKind::Other => {
                let name = e.name.clone();
                self.chrome
                    .toast(t!("gallery-not-an-image", file = name), None, now);
            }
            // A playlist plays as the queue (queue.rs).
            EntryKind::Image if crate::app::queue::is_playlist(&e.path) => {
                self.gallery_close(now, None);
                self.queue_open_playlist(&e.path);
            }
            EntryKind::Image if self.gallery.edited.photo(&e.path).is_some() => {
                self.gallery_open_version(&e.path);
                return;
            }
            EntryKind::Image => {
                let order = self.gallery.image_order();
                if self.in_book() {
                    // Leaves the archive (a plain file closes the book).
                    self.open_book(&e.path);
                }
                if self.entries != order {
                    self.set_entries(order);
                }
                let Some(idx) = self.index_of(&e.path) else {
                    return;
                };
                self.folder = self.gallery.dir.clone();
                // Opened from the grid it starts at fit, the tile growing into it, whatever
                // zoom the last image had (`viewing.keep_zoom` is for stepping through them).
                self.view.zoom_fit();
                self.gallery_close(now, Some(e.path.clone()));
                self.gallery.viewer_from_gallery = true;
                self.go_to(idx);
                self.update_title();
            }
        }
        self.redraw = true;
    }

    /// Whether the plain viewer has a gallery to go back to (its back button shows). The
    /// modes on top of it (MPR, linked views, meshes, cull, a slideshow) go back their own way.
    pub(super) fn gallery_back_offered(&self) -> bool {
        self.current.is_some()
            && !(self.linked_active()
                || self.mpr_active()
                || self.mesh_active()
                || self.cull.active()
                || self.slideshow.running())
            && !self.gallery.active
            && (self.gallery.viewer_from_gallery || self.settings.gallery.escape_opens_gallery)
    }

    /// The Back ladder ran out in the viewer (or its back button was pressed).
    pub(super) fn gallery_back_from_viewer(&mut self) -> bool {
        // Nothing open (the start screen): no gallery to go back to.
        if self.gallery.active
            || self.current.is_none()
            || !(self.gallery.viewer_from_gallery || self.settings.gallery.escape_opens_gallery)
        {
            return false;
        }
        if self.gallery.library.on {
            self.library_enter();
        } else {
            self.gallery_enter();
        }
        true
    }

    /// Opening a folder from the command line, the open dialog or a drop.
    pub(super) fn gallery_open_folder(&mut self, dir: &Path) -> bool {
        if !self.settings.gallery.open_folders_in_gallery {
            return false;
        }
        self.gallery_enter_at(Some(dir.to_path_buf()), None);
        self.gallery.from_viewer = false;
        true
    }

    /// A selection bar button `h` at `r` was pressed: its gallery code, or More (the
    /// right-click menu for the selection, opening upwards from the button).
    pub(super) fn gallery_sel_bar(&mut self, h: Hit, r: Rect) {
        match h {
            Hit::SelBar(c) => self.gallery_code(c),
            Hit::SelMore => self.gallery_context_menu(Some((r.x, r.y))),
            _ => {}
        }
        self.redraw = true;
    }

    /// Back to the folder shown before (or forward again), with the scroll and focus it had.
    /// False when there is none.
    pub(super) fn gallery_history(&mut self, forward: bool) -> bool {
        let ga = &mut self.gallery;
        let Some(cur) = ga.dir.clone() else {
            return false;
        };
        let (from, to) = if forward {
            (&mut ga.forward, &mut ga.back)
        } else {
            (&mut ga.back, &mut ga.forward)
        };
        // Not checked on disk here (a stalled network folder would hold the frame): one gone
        // since lists as missing, as any folder that went away does.
        let Some(dir) = from.pop() else {
            return false;
        };
        to.push(cur);
        ga.history_step = true;
        self.gallery_open_dir(dir.clone());
        // The list's new label is not always read out: the folder is said.
        self.gallery.announce = Some(file_name(&dir));
        self.redraw = true;
        true
    }

    /// Up one folder (Esc, Backspace, Alt+↑), focusing the folder we came from.
    pub(super) fn gallery_up(&mut self) {
        let Some(dir) = self.gallery.dir.clone() else {
            return;
        };
        if let Some(parent) = dir.parent() {
            let parent = parent.to_path_buf();
            self.gallery.positions.remove(&parent);
            self.gallery_open_dir(parent.clone());
            self.gallery.announce = Some(t!("gallery-a11y-up", folder = file_name(&parent)));
            self.gallery.focus = Some(dir.clone());
            self.gallery.anchor = Some(dir);
            self.gallery.reveal = true;
        }
    }
}

// ----- The event-loop turn ----------------------------------------------------------------

impl App {
    /// Once per event-loop turn: listing chunks, finished thumbnails, folder changes, timers.
    pub(super) fn gallery_tick(&mut self) {
        if self.gallery.engine.is_none() && self.gallery.transition.is_none() {
            return;
        }
        use scrollbench::{T_DRIVE, T_LAYOUT, T_MSG, T_READY, T_REST, T_SIMILAR, T_WATCH};
        let t_tick = Instant::now();
        let mut lap = t_tick;
        self.scroll_bench_drive();
        self.gallery.lap(T_DRIVE, &mut lap);
        let now = self.now();
        let mut changed = false;
        while let Ok(m) = self.gallery.rx.try_recv() {
            changed = true;
            self.gallery_msg(m, now);
        }
        self.library_tick();
        self.people_poll();
        self.gallery.lap(T_MSG, &mut lap);
        changed |= self.gallery_take_ready();
        changed |= self.gallery_edited_tick(now);
        if self.gallery.active && !self.gallery.edited.facts.is_empty() {
            let ga = &self.gallery;
            let vis = ga.layout.items_in(
                ga.scroll,
                ga.scroll + (ga.vh - TOP_BAR - BOTTOM_BAR).max(1.0),
            );
            self.gallery_edited_want(vis);
        }
        self.gallery.lap(T_READY, &mut lap);
        // Folder changes.
        if let Some(w) = &self.gallery.watch
            && let Some(ch) = w.take(Instant::now())
        {
            self.gallery_changes(ch);
            changed = true;
        }
        self.gallery.lap(T_WATCH, &mut lap);
        self.similar_tick();
        self.gallery.lap(T_SIMILAR, &mut lap);
        let m = &self.settings.metadata;
        let places = m.reverse_geocoding && !m.hide_gps;
        let ga = &mut self.gallery;
        ga.hide_gps = m.hide_gps;
        if ga.map.open {
            let cfg = basemap::Config {
                choice: m.basemap,
                url: m.basemap_url.clone(),
                path: m.basemap_path.clone(),
                service: m.basemap_service.clone(),
                cache_mb: m.basemap_cache,
                allowed: m
                    .basemap_allowed
                    .split(',')
                    .map(|h| h.trim().to_ascii_lowercase())
                    .filter(|h| !h.is_empty())
                    .collect(),
                dark: ga.map.dark,
            };
            ga.map.basemap.configure(cfg, &self.loader);
            if ga.map.feed_directions(&self.loader) {
                self.redraw = true;
            }
        }
        if let Some(msg) = self.gallery.map.take_export() {
            let now = self.now();
            self.chrome.toast(msg, None, now);
            self.redraw = true;
        }
        let ga = &mut self.gallery;
        if ga.places != places {
            ga.places = places;
            ga.dirty |= ga.view.group == GalleryGroup::Event;
        }
        // New sizes re-sort and re-lay out every tile (whole frames at 100k): not during a
        // fling, where the view would jump anyway; they run as soon as it rests.
        let flinging = ga.scrolling_fast();
        if ga.resort_at.is_some_and(|t| t <= Instant::now()) && !flinging {
            ga.resort_at = None;
            ga.resort();
        }
        if ga.dirty {
            ga.rebuild();
            changed = true;
        }
        // Event headers want place names: build the geocoder's index off the frame (a few
        // hundred milliseconds, once per run), then remake them.
        if ga.places_waiting && !ga.places_asked {
            ga.places_asked = true;
            let (tx, wake) = (ga.tx.clone(), self.loader.waker());
            self.loader.background("gal-places", move |_| {
                let _ = specular_metadata::geocode(0.0, 0.0);
                let _ = tx.send(Msg::PlacesReady);
                wake();
            });
        }
        if ga.relayout || ga.relayout_at.is_some_and(|t| t <= now && !flinging) {
            ga.relayout_at = None;
            let a = ga.anchor_path();
            ga.relayout_now(a, false);
            changed = true;
        }
        if ga.reveal
            && let Some(i) = ga.focus_index()
        {
            ga.reveal = false;
            ga.set_focus(i);
            ga.scroll = ga.scroll_target;
        }
        ga.lap(T_LAYOUT, &mut lap);
        // Marks: written a moment after the last change.
        self.gallery_flush_marks(false);
        self.gallery_pending_copy();
        self.gallery_drop_tick();
        self.gallery_hover_prefetch(now);
        self.gallery_retry_memory(now);
        if changed {
            self.redraw = true;
        }
        self.gallery.lap(T_REST, &mut lap);
        if let Some(b) = self.gallery.sbench.as_mut() {
            b.prof.tick += t_tick.elapsed().as_secs_f32() * 1000.0;
        }
    }

    fn gallery_msg(&mut self, m: Msg, now: f64) {
        let ga = &mut self.gallery;
        match m {
            Msg::Chunk {
                generation,
                entries,
                keys,
                loc,
            } if generation == ga.generation => {
                let (before, added) = (ga.entries.len(), entries.len());
                // The chunk's paths, to mark their entries seen once added (a chunk is small).
                let chunk: Vec<PathBuf> = if ga.refresh_seen.is_some() {
                    entries.iter().map(|e| e.path.clone()).collect()
                } else {
                    Vec::new()
                };
                ga.add_keyed(entries, keys, &loc);
                if let Some(seen) = ga.refresh_seen.as_mut() {
                    seen.resize(ga.entries.len(), false);
                    for p in &chunk {
                        if let Some(&i) = ga.by_path.get(p.as_path()) {
                            seen[i] = true;
                        }
                    }
                }
                let appended = ga.entries.len() == before + added;
                ga.list_appended = ga
                    .list_appended
                    .filter(|&n| n == before && appended)
                    .map(|n| n + added);
                // A fresh grid waits for the whole listing (sorted once, no reshuffle) unless
                // that takes longer than LIST_HOLD; then it streams, re-sorting at most ~7
                // times a second.
                if ga.tiles.is_empty() {
                    let at = ga.list_started.map_or_else(Instant::now, |t| t + LIST_HOLD);
                    ga.resort_at = Some(ga.resort_at.map_or(at, |r| r.min(at)));
                } else if ga.resort_at.is_none() {
                    ga.resort_at = Some(Instant::now() + Duration::from_millis(150));
                }
            }
            Msg::Done {
                generation,
                error,
                presorted,
            } if generation == ga.generation => {
                ga.listing = false;
                ga.list_error = error;
                if let Some(seen) = ga.refresh_seen.take() {
                    let gone: Vec<PathBuf> = ga
                        .entries
                        .iter()
                        .enumerate()
                        .filter(|&(i, _)| !seen.get(i).copied().unwrap_or(false))
                        .map(|(_, e)| e.path.clone())
                        .collect();
                    ga.remove_entries(&gone);
                }
                ga.trim_to_listing(true);
                ga.resort_at = None;
                let appended = ga.list_appended.take();
                match presorted {
                    Some(p) if ga.presorted_fits(&p, appended) => {
                        ga.sorted = p.order;
                        ga.dirty = true;
                    }
                    _ => ga.resort(),
                }
                self.gallery_after_listing();
                self.cull_gallery_listed();
            }
            Msg::Versions { generation, counts } if generation == self.gallery.generation => {
                let ga = &mut self.gallery;
                ga.versions.extend(counts);
                ga.dirty = true;
            }
            Msg::Edited { generation, photos } => self.gallery_edited_listed(generation, photos),
            Msg::Merges { generation, found } if generation == self.gallery.generation => {
                self.gallery.merges_add(found);
            }
            Msg::DevelopFacts { generation, facts } => {
                self.gallery_edited_facts(generation, facts);
            }
            Msg::Sidecars { generation, marks } if generation == self.gallery.generation => {
                let ga = &mut self.gallery;
                // The table wins over a sidecar.
                ga.marks.merge_read(marks);
                ga.dirty = true;
            }
            Msg::Slides { generation, slides } if generation == self.gallery.generation => {
                let ga = &mut self.gallery;
                let map = model::slide_map(slides);
                if map != ga.slides {
                    ga.slides = map;
                    ga.dirty = true;
                }
            }
            Msg::Dates { generation, dates } if generation == self.gallery.generation => {
                let ga = &mut self.gallery;
                for (p, d) in dates {
                    if let Some(&i) = ga.by_path.get(p.as_path()) {
                        ga.entries[i].taken = Some(d);
                    }
                }
                if ga.resort_at.is_none() {
                    ga.resort_at = Some(Instant::now() + Duration::from_millis(200));
                }
            }
            Msg::PlacesReady => {
                let ga = &mut self.gallery;
                ga.places_asked = false;
                ga.dirty |= ga.places_waiting;
            }
            Msg::Cover {
                dir,
                images,
                locked,
            } => {
                let ga = &mut self.gallery;
                ga.cover_pending.remove(&dir);
                if locked {
                    ga.locked.insert(dir.clone());
                } else {
                    ga.locked.remove(&dir);
                }
                ga.covers.insert(dir, images);
                ga.wish_key = 0;
            }
            Msg::TileKnown { archive, images } => {
                let ga = &mut self.gallery;
                if let Some(c) = ga.covers.get_mut(&archive)
                    && *c != images
                {
                    *c = images;
                    ga.wish_key = 0;
                }
            }
            Msg::Trashed { undo, done, failed } => {
                let n = done.len();
                self.gallery_removed(&done);
                if n > 0 {
                    let msg = if n == 1 {
                        t!("toast-moved-to-trash-named", file = file_name(&done[0]))
                    } else {
                        t!("toast-moved-to-trash-count", count = n as i64)
                    };
                    self.gallery.undo.insert(undo, UndoOp::Trash(done));
                    self.last_undo = Some(undo);
                    self.chrome.toast(msg, Some(undo), now);
                }
                if let Some((p, e)) = failed.first() {
                    self.chrome.toast(
                        t!(
                            "error-trash-failed",
                            file = file_name(p),
                            reason = e.clone()
                        ),
                        None,
                        now,
                    );
                }
            }
            Msg::Restored { done, failed } => {
                let classify = self.gallery_classifier();
                let entries: Vec<Entry> = done
                    .iter()
                    .filter(|p| self.gallery.dir.as_deref() == p.parent())
                    .filter_map(|p| entry_for(p, &classify))
                    .collect();
                if !entries.is_empty() {
                    self.gallery.add_entries(entries);
                    self.gallery.resort();
                }
                if let Some((p, e)) = failed.first() {
                    self.chrome.toast(
                        t!(
                            "app-restore-failed",
                            file = file_name(p),
                            reason = e.clone()
                        ),
                        None,
                        now,
                    );
                } else if let Some(p) = done.first() {
                    self.chrome
                        .toast(t!("toast-restored", file = file_name(p)), None, now);
                }
            }
            Msg::Moved {
                undo,
                copy,
                done,
                failed,
            } => {
                if !copy {
                    let from: Vec<PathBuf> = done.iter().map(|d| d.0.clone()).collect();
                    self.gallery_removed(&from);
                }
                let classify = self.gallery_classifier();
                let here: Vec<Entry> = done
                    .iter()
                    .filter(|(_, to)| self.gallery.dir.as_deref() == to.parent())
                    .filter_map(|(_, to)| entry_for(to, &classify))
                    .collect();
                if !here.is_empty() {
                    self.gallery.add_entries(here);
                    self.gallery.resort();
                }
                if let Some(u) = undo
                    && !done.is_empty()
                {
                    let n = done.len() as i64;
                    let dest = done[0].1.parent().map(file_name).unwrap_or_default();
                    let msg = if copy {
                        t!("gallery-copied-to", count = n, folder = dest)
                    } else {
                        t!("gallery-moved-to", count = n, folder = dest)
                    };
                    self.gallery.undo.insert(
                        u,
                        if copy {
                            UndoOp::Copy(done.into_iter().map(|d| d.1).collect())
                        } else {
                            UndoOp::Move(done)
                        },
                    );
                    self.last_undo = Some(u);
                    self.chrome.toast(msg, Some(u), now);
                }
                if let Some((p, e)) = failed.first() {
                    self.chrome.toast(
                        t!(
                            "gallery-move-failed",
                            file = file_name(p),
                            reason = e.clone()
                        ),
                        None,
                        now,
                    );
                }
            }
            Msg::Library(r) => self.library_results(*r),
            Msg::Albums(d) => self.albums_done(*d),
            _ => {}
        }
    }

    fn gallery_classifier(&self) -> impl Fn(&Path) -> EntryKind + use<> {
        let loader = self.loader.clone();
        move |p: &Path| {
            if crate::book::is_archive_path(p) {
                EntryKind::Archive
            } else if loader.supports_file(p) {
                EntryKind::Image
            } else {
                EntryKind::Other
            }
        }
    }

    /// Sidecar ratings and capture dates, once the folder is listed.
    fn gallery_after_listing(&mut self) {
        let ga = &self.gallery;
        let generation = ga.generation;
        let images: Vec<(PathBuf, Option<i64>)> = ga
            .entries
            .iter()
            .filter(|e| e.kind == EntryKind::Image)
            .map(|e| (e.path.clone(), e.taken))
            .collect();
        if images.is_empty() {
            return;
        }
        let want_dates = ga.view.sort == SortKey::DateTaken
            || matches!(
                ga.view.group,
                GalleryGroup::Day | GalleryGroup::Month | GalleryGroup::Year | GalleryGroup::Event
            );
        let sidecars = self.settings.gallery.rating_sidecars;
        let prefer = ga.marks.prefer.clone();
        let lib = ga.library_handle();
        let tx = ga.tx.clone();
        let wake = self.loader.waker();
        let token = ga.list_token.clone().unwrap_or_default();
        let dicom: Vec<PathBuf> = images
            .iter()
            .map(|(p, _)| p)
            .filter(|p| crate::app::series::is_dicom_name(p))
            .cloned()
            .collect();
        self.loader.background("gal-probe", move |_| {
            // Whole slides first: they change the tiles.
            if !dicom.is_empty() {
                let slides = specular_dicom::series::whole_slides(&dicom, &|| token.is_cancelled());
                if token.is_cancelled() {
                    return;
                }
                let _ = tx.send(Msg::Slides { generation, slides });
                wake();
            }
            // Develop versions: one listing of each folder.
            let mut dirs: Vec<&std::path::Path> =
                images.iter().filter_map(|(p, _)| p.parent()).collect();
            dirs.sort();
            dirs.dedup();
            let mut counts: Vec<(PathBuf, u32)> = dirs
                .iter()
                .flat_map(|d| specular_develop::sidecar::version_counts_in(d))
                .collect();
            // darktable's duplicates (`IMG_01.CR2.xmp`) are versions of the photo too.
            for (photo, d) in dirs
                .iter()
                .flat_map(|d| specular_metadata::sidecars::duplicate_counts_in(d))
            {
                match counts.iter_mut().find(|(p, _)| *p == photo) {
                    Some((_, n)) => *n = (*n).max(1) + d,
                    None => counts.push((photo, 1 + d)),
                }
            }
            if !counts.is_empty() {
                let _ = tx.send(Msg::Versions { generation, counts });
                wake();
            }
            // HDR merges: their sources stack under them.
            let paths: Vec<PathBuf> = images.iter().map(|(p, _)| p.clone()).collect();
            let found = merged::probe(&paths);
            if !found.is_empty() {
                let _ = tx.send(Msg::Merges { generation, found });
                wake();
            }
            // Edited photos (any version's sidecar, or the catalogue's edit when the folder
            // is in the Library, as the Library answers): their edits are read next.
            let mut photos: Vec<PathBuf> = dirs
                .iter()
                .flat_map(|d| crate::app::develop::edited::edited_in(d))
                .collect();
            if let Some(l) = &lib {
                photos.extend(dirs.iter().flat_map(|d| l.edited_in(d).unwrap_or_default()));
                photos.sort();
                photos.dedup();
            }
            let _ = tx.send(Msg::Edited { generation, photos });
            wake();
            if sidecars {
                let marks =
                    marks::read_sidecars(images.iter().map(|(p, _)| p.clone()), &prefer, &|| {
                        token.is_cancelled()
                    });
                if token.is_cancelled() {
                    return;
                }
                if !marks.is_empty() {
                    let _ = tx.send(Msg::Sidecars { generation, marks });
                    wake();
                }
            }
            if want_dates {
                let mut dates = Vec::new();
                for (p, taken) in &images {
                    if token.is_cancelled() {
                        return;
                    }
                    if taken.is_some() {
                        continue;
                    }
                    if let Some(d) = specular_thumbs::list::probe_date_taken(p) {
                        dates.push((p.clone(), d));
                    }
                    if dates.len() >= 500 {
                        let _ = tx.send(Msg::Dates {
                            generation,
                            dates: std::mem::take(&mut dates),
                        });
                        wake();
                    }
                }
                let _ = tx.send(Msg::Dates { generation, dates });
                wake();
            }
        });
    }

    /// Files renamed, moved, copied or rewritten by a file operation (`moved`: renames and
    /// moves, from and to): the selection and focus follow renamed files, and the rest is
    /// read again as the folder watcher's changes are.
    pub(super) fn gallery_files_changed(
        &mut self,
        paths: Vec<PathBuf>,
        moved: &[(PathBuf, PathBuf)],
    ) {
        let ga = &mut self.gallery;
        for (from, to) in moved {
            if ga.focus.as_ref() == Some(from) {
                ga.focus = Some(to.clone());
            }
            if ga.selected.remove(from) {
                ga.selected.insert(to.clone());
            }
        }
        self.gallery_changes(specular_thumbs::watch::Changes {
            paths,
            rescan: false,
        });
    }

    /// Changes the folder watcher saw.
    fn gallery_changes(&mut self, ch: specular_thumbs::watch::Changes) {
        let Some(dir) = self.gallery.dir.clone() else {
            return;
        };
        if ch.rescan || self.gallery.depth > 0 {
            self.gallery_relist(dir);
            return;
        }
        let classify = self.gallery_classifier();
        let mut gone = Vec::new();
        let mut add = Vec::new();
        for p in ch.paths {
            if p.parent() != Some(dir.as_path()) {
                continue;
            }
            match entry_for(&p, &classify) {
                Some(e) => {
                    if e.kind == EntryKind::Other && !self.gallery.show_other
                        || e.kind == EntryKind::Archive && !self.gallery.show_archives
                        || e.hidden && !self.gallery.show_hidden
                    {
                        continue;
                    }
                    if let Some(engine) = &self.gallery.engine {
                        engine.invalidate(&p);
                    }
                    if let Some(a) = self.gallery.atlas.as_mut() {
                        a.remove(&p);
                    }
                    self.gallery.metas.remove(p.as_path());
                    self.gallery.failed.remove(&p);
                    // A changed archive's pages: listed and read again (their cache keys
                    // follow the archive's stamp).
                    if e.kind == EntryKind::Archive
                        && let Some(pages) = self.gallery.covers.remove(&p)
                    {
                        for c in &pages {
                            if let Some(engine) = &self.gallery.engine {
                                engine.invalidate(c);
                            }
                            if let Some(a) = self.gallery.atlas.as_mut() {
                                a.remove(c);
                            }
                            self.gallery.failed.remove(c);
                        }
                    }
                    add.push(e);
                }
                None => gone.push(p),
            }
        }
        debug!(target: UI, "gallery: {} changed, {} gone", add.len(), gone.len());
        if !add.is_empty() {
            self.gallery.add_entries(add);
            self.gallery.wish_key = 0;
        }
        self.gallery_removed(&gone);
        self.gallery.resort();
    }

    /// Files that left the folder (trash, move, watcher): out of the gallery and the viewer.
    fn gallery_removed(&mut self, paths: &[PathBuf]) {
        if paths.is_empty() {
            return;
        }
        self.gallery.remove_entries(paths);
        let gone: HashSet<&PathBuf> = paths.iter().collect();
        if self.entries.iter().any(|p| gone.contains(p)) {
            let cur = self.current.and_then(|i| self.entries.get(i)).cloned();
            let keep: Vec<PathBuf> = self
                .entries
                .iter()
                .filter(|p| !gone.contains(p))
                .cloned()
                .collect();
            self.set_entries(keep);
            self.current = cur.and_then(|c| self.index_of(&c));
            if self.current.is_none() {
                self.shown = None;
            }
        }
    }

    /// Finished thumbnails from the engine. True when something changed.
    fn gallery_take_ready(&mut self) -> bool {
        let Some(engine) = &self.gallery.engine else {
            return false;
        };
        // Never wait for a worker holding the lock: ask again next frame.
        let Some(out) = engine.try_take_ready() else {
            self.gallery.engine_retry = true;
            return false;
        };
        if out.is_empty() {
            return false;
        }
        // Those just made (pushed with them): for the hardware advisor.
        let timings = engine.try_take_timings().unwrap_or_default();
        self.thumb_timings(timings);
        let ga = &mut self.gallery;
        let mut aspects = false;
        for o in out {
            match o {
                Outcome::Ready(r) => {
                    let known = ga
                        .metas
                        .get(r.path.as_path())
                        .is_some_and(|m| m.width == r.meta.width && m.height == r.meta.height);
                    if !known {
                        aspects = true;
                        if let Some(&i) = ga.by_path.get(r.path.as_path())
                            && r.meta.width > 0
                        {
                            ga.entries[i].dims = Some((r.meta.width, r.meta.height));
                            if ga.entries[i].taken.is_none() {
                                ga.entries[i].taken = r.meta.date_taken;
                            }
                        }
                    }
                    let k = shared_in(&ga.by_path, &ga.metas, &ga.averages, &r.path);
                    ga.metas.insert(k.clone(), r.meta.clone());
                    ga.averages.insert(k, r.average);
                    if let Some(px) = r.pixels
                        && !ga.edited.thumbs.contains_key(&r.path)
                    {
                        ga.uploads.push_back((r.path, r.class, px, r.sharp));
                    }
                }
                Outcome::Failed { path, error, .. } => {
                    debug!(target: CACHE, "thumbnail of {} failed: {error}", path.display());
                    ga.failed.insert(path, error);
                }
            }
        }
        if aspects {
            let now = self.t0.elapsed().as_secs_f64();
            let ga = &mut self.gallery;
            if ga.view.mode == Mode::Justified && ga.relayout_at.is_none() {
                ga.relayout_at = Some(now + 0.15);
            }
            if ga.view.sort == SortKey::Dimensions && ga.resort_at.is_none() {
                ga.resort_at = Some(Instant::now() + Duration::from_millis(300));
            }
            if ga.filter_needs_meta || ga.view.group.needs_meta() {
                ga.dirty = true;
            }
        }
        true
    }

    /// Ctrl+C on an image that was not open yet: copied once its decoder is ready.
    fn gallery_pending_copy(&mut self) {
        let Some(p) = self.gallery.pending_copy.clone() else {
            return;
        };
        let Some(&id) = self.by_path.get(&p) else {
            self.gallery.pending_copy = None;
            return;
        };
        let Some(img) = self.images.get(&id) else {
            return;
        };
        if img.error.is_some() {
            self.gallery.pending_copy = None;
            return;
        }
        if let Some(s) = img.session.clone() {
            let orient = img.orient.unwrap_or_default();
            self.gallery.pending_copy = None;
            self.clipboard.copy_image(&self.loader, s, orient);
        }
    }

    /// Thumbnails refused for memory are asked for again once that much is free (looked at
    /// every few seconds).
    fn gallery_retry_memory(&mut self, now: f64) {
        const EVERY_S: f64 = 3.0;
        let ga = &mut self.gallery;
        if now < ga.memory_retry_at || !ga.failed.values().any(|e| model::needs_memory(e).is_some())
        {
            return;
        }
        ga.memory_retry_at = now + EVERY_S;
        let free = specular_core::budget::global().free(specular_core::budget::Share::Thumbnails);
        let ready = model::memory_ready(&ga.failed, free);
        if ready.is_empty() {
            return;
        }
        debug!(target: CACHE, "gallery: {} thumbnails refused for memory fit now", ready.len());
        for p in ready {
            if let Some(engine) = &ga.engine {
                engine.invalidate(&p);
            }
            ga.failed.remove(&p);
        }
        ga.wish_key = 0;
        self.redraw = true;
    }

    /// Resting the pointer on an image starts its full decode, so opening it is instant.
    fn gallery_hover_prefetch(&mut self, now: f64) {
        if !self.gallery.active {
            return;
        }
        let Some((i, since)) = self.gallery.hover else {
            return;
        };
        if now - since < HOVER_PREFETCH_S {
            return;
        }
        let Some(e) = self.gallery.entry_of(i).cloned() else {
            return;
        };
        if e.kind != EntryKind::Image || self.gallery.prefetched.as_ref() == Some(&e.path) {
            return;
        }
        self.gallery.prefetched = Some(e.path.clone());
        self.gallery_viewer_open(&e.path, Priority::Prefetch);
    }

    /// Opens a decoder for `path` (prefetch, copy), keeping the viewer's list in gallery order.
    fn gallery_viewer_open(&mut self, path: &Path, prio: Priority) {
        let idx = match self.index_of(path) {
            Some(i) => i,
            None => {
                let cur = self.current.and_then(|i| self.entries.get(i)).cloned();
                let mut order = self.gallery.image_order();
                // Compare's images from elsewhere (`--compare`, "Compare with…") come after
                // the folder's, so they open like the listed ones.
                for p in self.gallery.compare.iter().flatten() {
                    if !order.contains(p) {
                        order.push(p.clone());
                    }
                }
                self.set_entries(order);
                self.current = cur.and_then(|c| self.index_of(&c));
                match self.index_of(path) {
                    Some(i) => i,
                    None => return,
                }
            }
        };
        let id = self.id_for(idx);
        self.ensure_open(id, prio);
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

// ----- Thumbnail feed ---------------------------------------------------------------------

impl Gallery {
    /// Tells the engine what is on screen and what comes next (only when that changed).
    pub(super) fn feed(&mut self) {
        let t0 = Instant::now();
        self.track_velocity(t0);
        self.feed_wishes();
        if let Some(b) = self.sbench.as_mut() {
            b.prof.feed += t0.elapsed().as_secs_f32() * 1000.0;
        }
    }

    /// The scroll was set from `was` by the program (a relayout keeping its anchor, a clamp,
    /// a new view), not by the person: not a movement (it would hold the Library's indexer
    /// as browsing, and count as speed).
    fn unmoved(&mut self, was: f32) {
        if let Some((_, y)) = &mut self.vel_mark {
            *y += self.scroll - was;
        }
    }

    /// Updates the smoothed scroll speed (called once per frame).
    fn track_velocity(&mut self, now: Instant) {
        if let Some((t, y)) = self.vel_mark {
            let dt = now.duration_since(t).as_secs_f32();
            if dt < 0.002 {
                return;
            }
            let v = (self.scroll - y) / dt;
            if (self.scroll - y).abs() > 0.5 {
                self.moved_at = Some(now);
            }
            // About 80 ms of smoothing; a long pause starts over.
            let k = if dt > 0.25 {
                1.0
            } else {
                1.0 - (-dt / 0.08).exp()
            };
            self.scroll_vel += (v - self.scroll_vel) * k;
        }
        self.vel_mark = Some((now, self.scroll));
    }

    /// Scrolling fast (a fling, a scrollbar drag, keys held): degrade quality, not frames.
    fn scrolling_fast(&self) -> bool {
        self.scroll_vel.abs() > FAST_VIEWPORTS_S * self.grid_h()
            && self
                .moved_at
                .is_some_and(|t| t.elapsed() < Duration::from_millis(120))
    }

    fn feed_wishes(&mut self) {
        let Some(engine) = &self.engine else { return };
        if self.layout.is_empty() {
            if self.wish_key != 1 && engine.try_want(Vec::new(), 0) {
                self.wish_key = 1;
            }
            // Nothing to scroll: a fling that emptied the grid has ended.
            self.wish_fast = false;
            return;
        }
        let vh = self.grid_h();
        let (y0, y1) = (self.scroll, self.scroll + vh);
        let vis = self.layout.items_in(y0, y1);
        let class = self.class();
        let fast = self.scrolling_fast();
        // Prefetch ahead by the distance covered in PREFETCH_LEAD_S (whole viewports, so the
        // list is not rebuilt for every small change of speed).
        let lead = (self.scroll_vel.abs() * PREFETCH_LEAD_S / vh.max(1.0))
            .clamp(2.0, 12.0)
            .ceil();
        let meta_all = self.view.sort == SortKey::Dimensions
            || self.view.group.needs_meta()
            || self.filter_needs_meta
            || self.facets.open
            || self.picker_needs_meta()
            || self.stacks_on
            || self.map.open;
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::hash::DefaultHasher::new();
            (
                vis.start,
                vis.end,
                class,
                self.tiles.len(),
                self.scroll_dir > 0.0,
            )
                .hash(&mut h);
            (fast, lead as u32).hash(&mut h);
            // Sorting by size, camera groups, filters and stacks need every image's meta: the
            // list asks for it a chunk at a time, renewed as results come in.
            if meta_all {
                (self.metas.len() >> 8, self.failed.len()).hash(&mut h);
            }
            (
                self.covers.len(),
                meta_all,
                self.view.mode.id(),
                self.generation,
            )
                .hash(&mut h);
            (&self.quick, &self.compare, self.similar.wanted_key).hash(&mut h);
            (self.map.open, &self.map.wanted).hash(&mut h);
            (self.people.open, &self.people.wanted).hash(&mut h);
            (self.import.open, &self.import.wanted).hash(&mut h);
            h.finish() | 2
        };
        if key == self.wish_key {
            return;
        }
        let t_list = Instant::now();
        let meta0 = (self.meta_cursor, self.meta_clean, self.meta_clean_at);
        let mut reqs: Vec<Request> = Vec::with_capacity(512);
        // Images asked for outside the tiles' own (each image tile is one entry, so only
        // these can repeat).
        let mut extra: HashSet<PathBuf> = HashSet::new();
        let mut covers_needed = Vec::new();
        let image = |g: &Gallery, i: usize| {
            let e = &g.entries[g.tiles[i].entry];
            (e.kind == EntryKind::Image
                && !g.failed.contains_key(&e.path)
                && !g.edited.photo_of.contains_key(&e.path))
            .then(|| e.path.clone())
        };
        for i in vis.clone() {
            if let Some(path) = image(self, i) {
                reqs.push(Request { path, class });
                continue;
            }
            let e = &self.entries[self.tiles[i].entry];
            if !matches!(e.kind, EntryKind::Folder | EntryKind::Archive) {
                continue;
            }
            match self.covers.get(&e.path) {
                Some(_) => {
                    let imgs = self.shown_covers(&e.path);
                    let class = self.cover_class(imgs.len());
                    for p in imgs {
                        if extra.insert(p.clone()) {
                            reqs.push(Request {
                                path: p.clone(),
                                class,
                            });
                        }
                    }
                }
                None => covers_needed.push(e.path.clone()),
            }
        }
        // Quick look and compare show the largest class.
        for p in self.quick.iter().chain(self.compare.iter().flatten()) {
            extra.insert(p.clone());
            reqs.push(Request {
                path: p.clone(),
                class: 512,
            });
        }
        for (p, class) in self
            .similar
            .wanted
            .iter()
            .chain(&self.map.wanted)
            .chain(&self.people.wanted)
            .chain(&self.import.wanted)
        {
            extra.insert(p.clone());
            reqs.push(Request {
                path: p.clone(),
                class: *class,
            });
        }
        let visible = reqs.len();
        // Past the visible tiles: at most WISH_MAX requests and the time budget (longer after
        // each list it cut short, so a slow machine still gets a whole list in the end).
        let budget = WISH_BUDGET * (1u32 << self.wish_short.min(3));
        let out_of_time = |k: usize| k % 32 == 31 && t_list.elapsed() > budget;
        let mut short = false;
        // Nothing behind while flinging: it would only be cancelled.
        let back = if fast { 0.0 } else { vh };
        let (ahead, behind) = if self.scroll_dir >= 0.0 {
            (
                self.layout.items_in(y1, y1 + lead * vh),
                self.layout.items_in(y0 - back, y0),
            )
        } else {
            (
                self.layout.items_in(y0 - lead * vh, y0),
                self.layout.items_in(y1, y1 + back),
            )
        };
        // Ahead in the scroll direction, nearest first.
        let rev = self.scroll_dir < 0.0;
        let mut got_ahead = 0;
        for k in 0..ahead.len() {
            if reqs.len() >= WISH_MAX {
                break;
            }
            if out_of_time(k) {
                short = true;
                break;
            }
            let i = if rev {
                ahead.end - 1 - k
            } else {
                ahead.start + k
            };
            if let Some(path) = image(self, i) {
                reqs.push(Request { path, class });
            }
            got_ahead = k + 1;
        }
        let ahead = if rev {
            ahead.end - got_ahead..ahead.end
        } else {
            ahead.start..ahead.start + got_ahead
        };
        let mut got_behind = 0;
        if !short {
            for (k, i) in behind.clone().enumerate() {
                if reqs.len() >= WISH_MAX {
                    break;
                }
                if out_of_time(k) {
                    short = true;
                    break;
                }
                if let Some(path) = image(self, i) {
                    reqs.push(Request { path, class });
                }
                got_behind = k + 1;
            }
        }
        let behind = behind.start..behind.start + got_behind;
        // Sizes and capture data for sorting, filtering, stacks and justified rows.
        let missing_meta = |i: usize| {
            let e = &self.entries[self.tiles[i].entry];
            (e.kind == EntryKind::Image
                && !self.metas.contains_key(e.path.as_path())
                && !self.failed.contains_key(&e.path))
            .then(|| e.path.clone())
        };
        if self.view.mode == Mode::Justified && !short {
            for (k, i) in self
                .layout
                .items_in(y0 - 6.0 * vh, y1 + 10.0 * vh)
                .enumerate()
            {
                if out_of_time(k) {
                    short = true;
                    break;
                }
                if let Some(path) = missing_meta(i) {
                    reqs.push(Request { path, class: 0 });
                }
            }
        }
        // The meta scan found nothing this time but has not been round every tile yet.
        let mut meta_more = false;
        if meta_all && !short {
            // From where the last list stopped, wrapping: at most META_CHUNK requests and
            // META_SCAN entries looked at (each frame of a scroll rebuilds the list; scanning
            // all 100k entries cost ~10 ms). A scan that finds nothing goes on next frame
            // until it has been round every tile without finding one.
            let n = self.tiles.len();
            let complete = (self.generation, n, self.metas.len(), self.failed.len());
            if n > 0 && self.meta_complete != Some(complete) {
                if self.meta_clean_at != (self.generation, n) {
                    self.meta_clean_at = (self.generation, n);
                    self.meta_clean = 0;
                }
                let (mut found, mut scanned) = (0, 0);
                while scanned < n.min(META_SCAN) && found < META_CHUNK && !out_of_time(scanned) {
                    let i = (self.meta_cursor + scanned) % n;
                    scanned += 1;
                    if let Some(path) = missing_meta(i) {
                        reqs.push(Request { path, class: 0 });
                        found += 1;
                        self.meta_clean = 0;
                    } else {
                        self.meta_clean += 1;
                    }
                }
                self.meta_cursor = (self.meta_cursor + scanned) % n;
                if self.meta_clean >= n {
                    self.meta_complete = Some(complete);
                    self.meta_clean = 0;
                } else if found == 0 {
                    meta_more = true;
                }
            }
        }
        let list_ms = t_list.elapsed().as_secs_f32() * 1000.0;
        let t_post = Instant::now();
        // Posted to the engine's feeder thread, which applies it: a frame never waits for
        // the engine's or the pool's locks. If the feeder is taking the last list this
        // instant, the list is built again next frame, the meta scan from where it was.
        if !engine.try_want(reqs, visible) {
            (self.meta_cursor, self.meta_clean, self.meta_clean_at) = meta0;
            self.engine_retry = true;
            if let Some(b) = self.sbench.as_mut() {
                b.prof.f_list += list_ms;
            }
            return;
        }
        let t_rest = Instant::now();
        if let Some(b) = self.sbench.as_mut() {
            b.prof.f_list += list_ms;
            b.prof.f_post += (t_rest - t_post).as_secs_f32() * 1000.0;
        }
        // A list cut short (or a meta scan not done) is finished next frame.
        if short || meta_more {
            self.wish_key = 0;
            self.engine_retry = true;
        } else {
            self.wish_key = key;
        }
        self.wish_short = if short { self.wish_short + 1 } else { 0 };
        self.wish_tiles = [vis, ahead, behind];
        self.wish_extra = extra;
        self.wish_fast = fast;
        // Folder mosaics: the first images of each visible folder, listed by the cover threads
        // (a send on a channel: a thread spawned per folder took the process's memory-map lock
        // on the frame).
        if let Some(lister) = &self.covers_in {
            for dir in covers_needed {
                if self.cover_pending.insert(dir.clone())
                    && !lister.ask(dir.clone(), self.show_hidden)
                {
                    self.cover_pending.remove(&dir);
                }
            }
        }
        if let Some(b) = self.sbench.as_mut() {
            b.prof.f_rest += t_rest.elapsed().as_secs_f32() * 1000.0;
        }
    }

    /// GPU uploads, at most [`UPLOAD_BUDGET`] per frame, visible tiles first.
    pub(super) fn upload(&mut self, gpu: &Gpu, ui: &mut UiRenderer) {
        let t0 = Instant::now();
        let (n, bytes, page_ms) = self.upload_some(gpu, ui);
        if let Some(b) = self.sbench.as_mut() {
            b.prof.upload += t0.elapsed().as_secs_f32() * 1000.0;
            b.prof.uploads += n;
            b.prof.upload_bytes += bytes;
            if let Some(ms) = page_ms {
                b.prof.pages += 1;
                b.prof.page += ms;
            }
            let queued: u64 = self.uploads.iter().map(|u| u.2.rgba.len() as u64).sum();
            b.queue_peak = b.queue_peak.max(queued);
            if let Some(e) = &self.engine {
                b.engine_peak = b.engine_peak.max(e.stats().memory_bytes as u64);
            }
        }
    }

    /// Uploads (count, bytes, and the time (ms) of the one that added an atlas page).
    fn upload_some(&mut self, gpu: &Gpu, ui: &mut UiRenderer) -> (u32, u64, Option<f32>) {
        let Some(engine) = &self.engine else {
            return (0, 0, None);
        };
        // Sub-phase times for the scroll bench ([`scrollbench::SUBS`] from `U_BEGIN`).
        use scrollbench::{U_BEGIN, U_END, U_INSERT, U_QUEUE, U_SCAN};
        let mut us = [0f32; 5];
        let mut ut = Instant::now();
        let mut ulap = |k: usize, t: &mut Instant| {
            let n = Instant::now();
            us[k - U_BEGIN] += (n - *t).as_secs_f32() * 1000.0;
            *t = n;
        };
        let (cover_small, cover_big) = (self.cover_class(4), self.cover_class(1));
        let fast = self.scrolling_fast();
        // Pages ahead as soon as a listing is in, even if the view moves: the helper's batch
        // overlaps the first frames, which upload nothing yet, at no measured cost. Later it
        // cost a submit 11-45 ms (the helper's texture creation against the frame's uploads),
        // and a page made on demand mid-scroll 10-20 ms (the GPU allocator's new memory
        // block). More pages after that (a bigger size) only at rest.
        let at_rest = self.scroll_vel.abs() < 1.0
            && self
                .moved_at
                .is_none_or(|t| t.elapsed() > Duration::from_millis(250));
        let quiet = !self.listing && (at_rest || !self.paged_ahead);
        let class = self.class();
        // The pages this folder can fill at the current size, and one for another class
        // (quick look, folder mosaics).
        let per_page = (atlas::PAGE / class.max(1)).pow(2) as usize;
        let want = self.tiles.len().div_ceil(per_page.max(1)) + 1;
        let atlas = self.atlas.get_or_insert_with(|| Atlas::new(ATLAS_PAGES));
        let ahead = atlas.begin_frame(gpu, quiet, want);
        self.paged_ahead |= quiet && ahead;
        ulap(U_BEGIN, &mut ut);
        // Last frame's visible tiles that were not uploaded: pushed again below if still
        // visible (so the queue does not grow by a screenful per frame while flinging).
        let stale = self.uploads_front.min(self.uploads.len());
        self.uploads.drain(..stale);
        self.uploads_front = 0;
        trim_uploads(
            &mut self.uploads,
            &self.by_path,
            &self.entry_tile,
            &self.wish_tiles,
            &self.wish_extra,
        );
        // How many at the front of the queue are on screen (pushed below this frame).
        let mut front = 0usize;
        // A worker holds the engine's lock: look again next frame rather than wait.
        let mut busy = false;
        let class = {
            // `class()` borrows self; recompute from the layout here.
            let side = match self.view.mode {
                Mode::Details => 24.0,
                Mode::Justified => self.size_shown * 1.6,
                _ => self
                    .layout
                    .cells
                    .first()
                    .map_or(self.size_shown, |c| c.w.max(c.h)),
            };
            specular_thumbs::class_for(side * self.scale)
        };
        // Tiles on screen whose slot was reused: back from the engine's memory.
        let vis = self.layout.items_in(
            self.scroll,
            self.scroll + (self.vh - TOP_BAR - BOTTOM_BAR).max(1.0),
        );
        for i in vis {
            let Some(t) = self.tiles.get(i) else { break };
            let e = &self.entries[t.entry];
            let paths: Vec<(&PathBuf, u32)> = match e.kind {
                EntryKind::Image => vec![(&e.path, class)],
                EntryKind::Folder | EntryKind::Archive => self
                    .covers
                    .get(&e.path)
                    .map(|c| {
                        let class = if c.len() >= 4 { cover_small } else { cover_big };
                        c.iter().map(|p| (p, class)).collect()
                    })
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            for (p, c) in paths {
                // An edited thumbnail made since: the old one goes as it comes in.
                if self.edited.swap.remove(p.as_path()) {
                    atlas.remove(p);
                }
                if atlas.contains(p, c) {
                    continue;
                }
                if let Some((got, px)) = self.edited.pixels(p, c) {
                    if !atlas.contains(p, got) {
                        self.uploads.push_front((p.clone(), got, px, true));
                        front += 1;
                    }
                    continue;
                }
                if busy {
                    continue;
                }
                match engine.try_mem_get(p, c) {
                    Ok(Some((got, px, avg))) => {
                        let k = shared_in(&self.by_path, &self.metas, &self.averages, p);
                        self.averages.insert(k, avg);
                        if !atlas.contains(p, got) {
                            self.uploads.push_front((p.clone(), got, px, true));
                            front += 1;
                        }
                    }
                    Ok(None) => {}
                    Err(()) => busy = true,
                }
            }
        }
        for p in self.quick.iter().chain(self.compare.iter().flatten()) {
            if !atlas.contains(p, 512)
                && let Ok(Some((got, px, _))) = engine.try_mem_get(p, 512)
                && !atlas.contains(p, got)
            {
                self.uploads.push_front((p.clone(), got, px, true));
                front += 1;
            }
        }
        for (p, c) in self
            .similar
            .wanted
            .iter()
            .chain(&self.map.wanted)
            .chain(&self.people.wanted)
            .chain(&self.import.wanted)
        {
            if !atlas.contains(p, *c)
                && let Ok(Some((got, px, _))) = engine.try_mem_get(p, *c)
                && !atlas.contains(p, got)
            {
                self.uploads.push_front((p.clone(), got, px, true));
                front += 1;
            }
        }
        ulap(U_SCAN, &mut ut);
        let t0 = Instant::now();
        let (mut n, mut bytes) = (0u32, 0u64);
        let mut page_ms = None;
        // Entries popped from the front (the first `front` are this frame's visible tiles).
        let mut taken = 0usize;
        self.upload_more = false;
        while let Some((path, c, px, sharp)) = self.uploads.pop_front() {
            let on_screen = taken < front;
            taken += 1;
            if atlas.contains(&path, c) && atlas.is_sharp(&path, c) && sharp {
                continue;
            }
            if !on_screen {
                let wished = self.wish_extra.contains(&path)
                    || tile_in(&self.by_path, &self.entry_tile, &path)
                        .is_some_and(|t| self.wish_tiles.iter().any(|r| r.contains(&t)));
                if !wished {
                    // Scrolled out of the wish list: dropped (the engine still has it).
                    continue;
                }
                if fast {
                    // Flinging: only what is on screen now; the rest once the view rests.
                    self.uploads.push_front((path, c, px, sharp));
                    taken -= 1;
                    break;
                }
            }
            let len = px.rgba.len() as u64;
            let predicted = Duration::from_nanos((len as f32 * self.upload_ns_per_byte) as u64);
            if n > 0 && (t0.elapsed() + predicted > UPLOAD_BUDGET || bytes + len > UPLOAD_BYTES) {
                self.uploads.push_front((path, c, px, sharp));
                taken -= 1;
                self.upload_more = true;
                break;
            }
            let t = Instant::now();
            ulap(U_QUEUE, &mut ut);
            let pages = atlas.pages();
            // The path's first thumbnail (not a sharper or bigger one replacing another).
            let first = on_screen
                && !specular_thumbs::CLASSES
                    .iter()
                    .any(|&k| atlas.contains(&path, k));
            let slot = atlas.insert(gpu, ui, &path, c, &px, sharp);
            ulap(U_INSERT, &mut ut);
            if slot.is_none() {
                // Every slot is on screen this frame, or the next page is still being made:
                // try again next frame.
                self.uploads.push_front((path, c, px, sharp));
                taken -= 1;
                self.upload_more = atlas.growing();
                break;
            }
            n += 1;
            bytes += len;
            if first {
                self.fade_in.insert(path, self.last_frame);
            }
            if atlas.pages() != pages {
                page_ms = Some(t.elapsed().as_secs_f32() * 1000.0);
                // A new page (texture and bind group): not an upload rate, and enough for
                // one frame.
                self.upload_more = !self.uploads.is_empty();
                break;
            }
            let per_byte = (t.elapsed().as_nanos() as f32 / len.max(1) as f32).min(50.0);
            self.upload_ns_per_byte += (per_byte - self.upload_ns_per_byte) * 0.1;
        }
        ulap(U_QUEUE, &mut ut);
        atlas.end_frame(gpu);
        ulap(U_END, &mut ut);
        if let Some(b) = self.sbench.as_mut() {
            for (k, v) in us.into_iter().enumerate() {
                b.prof.sub[U_BEGIN + k] += v;
            }
        }
        // This frame's visible tiles not reached are pushed again next frame if still visible.
        self.uploads_front = front.saturating_sub(taken);
        (n, bytes, page_ms)
    }

    /// Drops the GPU pages once the gallery has been closed for a minute (coming straight
    /// back keeps them, so the grid returns filled).
    pub(super) fn release_idle(&mut self, now: f64, ui: &mut UiRenderer) {
        if self.active || self.atlas.is_none() || now - self.last_frame < IDLE_RELEASE_S {
            return;
        }
        if let Some(a) = &self.atlas {
            debug!(target: UI, "gallery: releasing {} thumbnails in {} pages ({} MB)", a.len(), a.pages(), a.bytes() >> 20);
        }
        self.release_gpu(ui);
    }

    /// Drops the GPU pages while the gallery is not shown (memory pressure).
    pub(super) fn release_hidden(&mut self, ui: &mut UiRenderer) {
        if !self.active && self.atlas.is_some() {
            self.release_gpu(ui);
        }
    }

    /// The GPU device was replaced: the pages, quick look's and compare's textures and the
    /// map are made again, and the thumbnails uploaded again as the grid draws.
    pub(super) fn gpu_reset(&mut self) {
        self.atlas = None;
        self.quick_tex.clear();
        self.diff_tex.clear();
        self.quick_targets.clear();
        self.diff_targets.clear();
        self.map.gpu_reset();
        self.dirty = true;
    }

    /// Drops the GPU pages.
    fn release_gpu(&mut self, ui: &mut UiRenderer) {
        if let Some(a) = self.atlas.as_mut() {
            a.release(ui);
        }
        self.atlas = None;
        self.quick_tex.clear();
        self.diff_tex.clear();
        for t in self
            .quick_targets
            .drain(..)
            .chain(self.diff_targets.drain(..))
        {
            ui.unregister_texture(t.id);
        }
    }
}

/// Threads listing folder and archive mosaics' images, asked through a channel so a frame never spawns a
/// thread or waits for a lock. They end when the gallery drops its sender.
struct CoverLister {
    tx: Sender<(PathBuf, bool)>,
}

/// Folders listed at once (each `stat`s up to 4096 entries; slow storage keeps one busy).
const COVER_THREADS: usize = 2;

impl CoverLister {
    fn start(
        loader: &crate::loader::Loader,
        archives: Arc<ArchiveOpener>,
        out: Sender<Msg>,
    ) -> Option<CoverLister> {
        let (tx, rx) = crossbeam_channel::unbounded::<(PathBuf, bool)>();
        let mut any = false;
        for k in 0..COVER_THREADS {
            let (rx, out, l, wake) = (rx.clone(), out.clone(), loader.clone(), loader.waker());
            let archives = archives.clone();
            any |= specular_thread::builder(format!("gal-cover-{k}"))
                .spawn(move || {
                    specular_platform::lower_thread_priority();
                    while let Ok((dir, hidden)) = rx.recv() {
                        // An archive's first pages, opened (and kept) for the engine to read.
                        // A panic (an archive reader's bug) leaves this tile without a
                        // mosaic; the thread goes on with the next folder.
                        let archive = crate::book::is_archive_path(&dir) && dir.is_file();
                        let what = format_args!("listing the cover of {}", dir.display());
                        let (images, locked) = specular_core::isolate::run(what, || {
                            let images = if archive {
                                archives.first_pages(&dir, 4)
                            } else {
                                cover_images(&dir, hidden, &|p| l.supports_file(p))
                            };
                            let locked = archive && images.is_empty() && archives.locked(&dir);
                            (images, locked)
                        })
                        .unwrap_or_default();
                        if out
                            .send(Msg::Cover {
                                dir,
                                images,
                                locked,
                            })
                            .is_err()
                        {
                            break;
                        }
                        wake();
                    }
                })
                .is_ok();
        }
        any.then_some(CoverLister { tx })
    }

    /// Queues `dir` (never blocks); false if the threads are gone.
    fn ask(&self, dir: PathBuf, hidden: bool) -> bool {
        self.tx.send((dir, hidden)).is_ok()
    }
}

/// The first four images of a folder by name (its mosaic).
fn cover_images(dir: &Path, hidden: bool, is_image: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<PathBuf> = rd
        .flatten()
        .take(4096)
        .map(|e| e.path())
        .filter(|p| {
            hidden
                || !p
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .filter(|p| is_image(p) && p.is_file())
        .collect();
    let l = I18n::global().localizer();
    names.sort_by(|a, b| l.natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
    names.truncate(4);
    names
}

// ----- Marks and listing for cull mode ----------------------------------------------------

impl App {
    /// The marks tables, made and read once (by the gallery's first opening, or cull mode's).
    /// Screenshots and benchmarks leave the user's tables alone.
    pub(super) fn gallery_marks_ready(&mut self) {
        if !self.gallery.marks_made {
            self.gallery.marks_made = true;
            let dir = if self.args.screenshot.is_some() {
                None
            } else {
                self.store
                    .path()
                    .and_then(|p| p.parent().map(Path::to_path_buf))
            };
            let prefer = std::mem::take(&mut self.gallery.marks.prefer);
            self.gallery.marks = Store::new(dir);
            self.gallery.marks.prefer = prefer;
        }
        self.gallery.marks.load();
    }

    pub(super) fn gallery_mark_of(&self, p: &Path) -> Mark {
        self.gallery.marks.mark(p)
    }

    /// Develop made or deleted a version of `p`: its tile's badge says `count` (none for 1).
    pub(super) fn gallery_set_versions(&mut self, p: &Path, count: u32) {
        let ga = &mut self.gallery;
        let changed = if count > 1 {
            ga.versions.insert(p.to_path_buf(), count) != Some(count)
        } else {
            ga.versions.remove(p).is_some()
        };
        ga.dirty |= changed;
    }

    /// Marks read from sidecars; the table wins over a sidecar.
    pub(super) fn gallery_merge_sidecar_marks(&mut self, marks: Vec<(PathBuf, Mark)>) {
        self.gallery.marks.merge_read(marks);
        self.gallery.dirty = true;
    }

    /// Folders where other apps' sidecars have the last word on marks (the setting).
    pub(super) fn gallery_prefer_folders(&self) -> Vec<PathBuf> {
        self.gallery.marks.prefer.clone()
    }

    /// The setting "Prefer other apps' sidecars in" changed (and at start): the gallery's
    /// marks and the Library follow it.
    pub(in crate::app) fn gallery_set_prefer_sidecars(&mut self, setting: &str) {
        let folders = marks::prefer_folders(setting);
        self.gallery.marks.prefer = folders.clone();
        self.gallery.dirty = true;
        self.gallery.library_set_prefer(folders);
    }

    /// The files an undo of marks puts back, or `None` when `u` is something else.
    pub(super) fn gallery_undo_paths(&self, u: u64) -> Option<Vec<PathBuf>> {
        match self.gallery.undo.get(&u)? {
            UndoOp::Marks(old) => Some(old.iter().map(|(p, _)| p.clone()).collect()),
            UndoOp::Keywords(old) => Some(old.iter().map(|(p, _)| p.clone()).collect()),
            _ => None,
        }
    }

    /// Writes the marks tables once they are due (a moment after the last change), or at once
    /// with `now` (on exit, on this thread).
    pub(super) fn gallery_flush_marks(&mut self, now: bool) {
        let ga = &mut self.gallery;
        if !now && !ga.marks_due.is_some_and(|t| t <= Instant::now()) {
            return;
        }
        ga.marks_due = None;
        let Some((dir, m, v)) = ga.marks.snapshot() else {
            return;
        };
        let write = move || {
            if let Err(e) = marks::write_tables(&dir, &m, &v) {
                warn!(target: UI, "gallery marks: {e}");
            }
        };
        if now {
            write();
        } else {
            self.loader.background("gal-marks", move |_| write());
        }
    }

    /// What cull mode culls when entered from the gallery: its images in the gallery's order
    /// (filter included), collapsed stacks' members after their shown file, and the focused
    /// image.
    pub(super) fn gallery_cull_listing(&mut self) -> (Vec<PathBuf>, Option<PathBuf>) {
        if self.gallery.dirty {
            self.gallery.rebuild();
        }
        let ga = &self.gallery;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for t in &ga.tiles {
            for i in std::iter::once(t.entry).chain(t.members.iter().copied()) {
                let e = &ga.entries[i];
                if e.kind == EntryKind::Image && seen.insert(i) {
                    out.push(e.path.clone());
                }
            }
        }
        let focus = ga.focus.clone().filter(|f| out.contains(f));
        (out, focus)
    }

    /// The gallery gives way to the viewer at `path` (cull mode sets the listing).
    pub(super) fn gallery_hand_over(&mut self, path: &Path) {
        if self.in_book() {
            // Leaves the archive (a plain file closes the book).
            self.open_book(path);
        }
        self.folder = self.gallery.dir.clone();
        let now = self.now();
        self.gallery_close(now, Some(path.to_path_buf()));
        self.gallery.viewer_from_gallery = true;
    }

    /// The folder the gallery shows.
    pub(super) fn gallery_dir(&self) -> Option<&Path> {
        self.gallery.dir.as_deref()
    }

    /// Every image the gallery lists, in no particular order.
    pub(super) fn gallery_images(&self) -> Vec<PathBuf> {
        let ga = &self.gallery;
        ga.entries
            .iter()
            .filter(|e| e.kind == EntryKind::Image)
            .map(|e| e.path.clone())
            .collect()
    }
}

pub(super) fn mark_label_name(l: u8) -> String {
    label_name(l)
}

// ----- Operations -------------------------------------------------------------------------

impl App {
    /// Applies `f` to the marks of the targets (culling keys), undoably.
    pub(super) fn gallery_mark(&mut self, f: impl Fn(Mark) -> Mark) {
        let targets: Vec<PathBuf> = self
            .gallery
            .targets()
            .into_iter()
            .filter(|p| {
                self.gallery
                    .by_path
                    .get(p.as_path())
                    .is_some_and(|&i| self.gallery.entries[i].kind == EntryKind::Image)
            })
            .collect();
        self.gallery_set_marks(&targets, f);
        self.redraw = true;
    }

    /// Applies `f` to the marks of `paths`, undoably: the undo id, or `None` when nothing
    /// changed.
    pub(super) fn gallery_set_marks(
        &mut self,
        paths: &[PathBuf],
        f: impl Fn(Mark) -> Mark,
    ) -> Option<u64> {
        let mut old = Vec::with_capacity(paths.len());
        let mut changed = Vec::new();
        for p in paths {
            let m = self.gallery.marks.mark(p);
            let n = f(m);
            if n != m {
                old.push((p.clone(), m));
                self.gallery.marks.set_mark(p, n);
                changed.push((p.clone(), n));
            }
        }
        if changed.is_empty() {
            return None;
        }
        self.gallery_marks_changed(changed);
        let u = self.next_undo;
        self.next_undo += 1;
        self.gallery.undo.insert(u, UndoOp::Marks(old));
        self.last_undo = Some(u);
        Some(u)
    }

    /// A keyword change was written: Undo (Ctrl+Z) puts back each photo's keywords `before`.
    pub(in crate::app) fn gallery_keywords_undoable(
        &mut self,
        before: Vec<(PathBuf, Vec<String>)>,
    ) {
        let u = self.next_undo;
        self.next_undo += 1;
        self.gallery.undo.insert(u, UndoOp::Keywords(before));
        self.last_undo = Some(u);
    }

    /// Sets each path's own mark, undoably as one: the undo id, or `None` when nothing
    /// changed.
    pub(super) fn gallery_set_marks_each(&mut self, marks: &[(PathBuf, Mark)]) -> Option<u64> {
        let mut old = Vec::with_capacity(marks.len());
        let mut changed = Vec::new();
        for (p, n) in marks {
            let m = self.gallery.marks.mark(p);
            if *n != m {
                old.push((p.clone(), m));
                self.gallery.marks.set_mark(p, *n);
                changed.push((p.clone(), *n));
            }
        }
        if changed.is_empty() {
            return None;
        }
        self.gallery_marks_changed(changed);
        let u = self.next_undo;
        self.next_undo += 1;
        self.gallery.undo.insert(u, UndoOp::Marks(old));
        self.last_undo = Some(u);
        Some(u)
    }

    /// Files moved (or with `copy`, copied) from `.0` to `.1` by cull mode: their marks
    /// follow them in the marks table (the sidecars travelled with the files).
    pub(super) fn gallery_marks_follow(&mut self, moves: &[(PathBuf, PathBuf)], copy: bool) {
        let ga = &mut self.gallery;
        let mut any = false;
        for (from, to) in moves {
            let m = ga.marks.mark(from);
            if m.is_empty() {
                continue;
            }
            ga.marks.set_mark(to, m);
            if !copy {
                ga.marks.set_mark(from, Mark::default());
            }
            any = true;
        }
        if any {
            ga.marks_due = Some(Instant::now() + Duration::from_millis(800));
            ga.dirty = true;
        }
    }

    /// Files cull mode removed (its copies, undone): their marks leave the table.
    pub(super) fn gallery_marks_forget(&mut self, paths: &[PathBuf]) {
        let ga = &mut self.gallery;
        let mut any = false;
        for p in paths {
            if !ga.marks.mark(p).is_empty() {
                ga.marks.set_mark(p, Mark::default());
                any = true;
            }
        }
        if any {
            ga.marks_due = Some(Instant::now() + Duration::from_millis(800));
            ga.dirty = true;
        }
    }

    fn gallery_marks_changed(&mut self, changed: Vec<(PathBuf, Mark)>) {
        self.events_marked(changed.iter().map(|(p, m)| (p, m.rating, m.label, m.flag)));
        self.gallery.marks_due = Some(Instant::now() + Duration::from_millis(800));
        self.similar_marks_changed();
        self.library_marks_changed(&changed);
        let ga = &mut self.gallery;
        // Filters and rating groups follow; the focus stays.
        if !ga.filter.is_empty() || ga.view.group == GalleryGroup::Rating {
            ga.dirty = true;
        }
        if self.settings.gallery.rating_sidecars && self.args.screenshot.is_none() {
            let ticket = marks::sidecar_ticket();
            self.loader.background("gal-sidecars", move |_| {
                // Offline originals: their sidecars wait (local copies).
                let changed = crate::app::local_copies::queue_marks(changed);
                for (p, e) in marks::write_sidecars(ticket, changed) {
                    warn!(target: UI, "sidecar of {}: {e}", p.display());
                }
            });
        }
    }

    /// Moves the targets to the trash (undoable).
    pub(super) fn gallery_trash(&mut self) {
        let targets: Vec<PathBuf> = self
            .gallery
            .targets()
            .into_iter()
            .filter(|p| p.exists())
            .collect();
        self.gallery_trash_paths(targets);
    }

    /// Moves `targets` to the trash in the background; the undo id the result toasts with.
    fn gallery_trash_paths(&mut self, targets: Vec<PathBuf>) -> Option<u64> {
        if targets.is_empty() {
            return None;
        }
        let undo = self.next_undo;
        self.next_undo += 1;
        let tx = self.gallery.tx.clone();
        let wake = self.loader.waker();
        self.loader.background("gal-trash", move |_| {
            let (mut done, mut failed) = (Vec::new(), Vec::new());
            for p in targets {
                match crate::clipboard::trash_with_sidecars(&p) {
                    Ok(()) => done.push(p),
                    Err(e) => failed.push((p, crate::error_text::trash_reason(&e))),
                }
            }
            let _ = tx.send(Msg::Trashed { undo, done, failed });
            wake();
        });
        Some(undo)
    }

    /// Moves (or copies) `files` into `dest` in the background (undoable).
    pub(super) fn gallery_move(
        &mut self,
        files: Vec<PathBuf>,
        dest: PathBuf,
        copy: bool,
    ) -> Option<u64> {
        let files: Vec<PathBuf> = files
            .into_iter()
            .filter(|f| f.parent() != Some(dest.as_path()) && *f != dest && !dest.starts_with(f))
            .collect();
        if files.is_empty() {
            return None;
        }
        let undo = self.next_undo;
        self.next_undo += 1;
        let tx = self.gallery.tx.clone();
        let wake = self.loader.waker();
        self.loader.background("gal-move", move |_| {
            let (mut done, mut failed) = (Vec::new(), Vec::new());
            for f in files {
                let Some(name) = f.file_name() else { continue };
                let to = free_name(&dest.join(name));
                match move_or_copy(&f, &to, copy) {
                    Ok(()) => done.push((f, to)),
                    Err(e) => failed.push((f, crate::error_text::io_reason(&e))),
                }
            }
            let _ = tx.send(Msg::Moved {
                undo: Some(undo),
                copy,
                done,
                failed,
            });
            wake();
        });
        Some(undo)
    }

    /// Undo of a gallery operation; false when `u` is not ours.
    pub(super) fn gallery_undo(&mut self, u: u64) -> bool {
        let Some(op) = self.gallery.undo.remove(&u) else {
            return false;
        };
        self.similar_undone(u);
        self.chrome.dismiss_undo(u);
        if self.last_undo == Some(u) {
            self.last_undo = None;
        }
        let tx = self.gallery.tx.clone();
        let wake = self.loader.waker();
        match op {
            UndoOp::Marks(old) => {
                for (p, m) in &old {
                    self.gallery.marks.set_mark(p, *m);
                }
                self.gallery_marks_changed(old);
            }
            UndoOp::Keywords(old) => self.keywords_restore(old),
            UndoOp::Trash(paths) => {
                self.loader.background("gal-restore", move |_| {
                    let (mut done, mut failed) = (Vec::new(), Vec::new());
                    for p in paths {
                        match crate::clipboard::restore_now(&p) {
                            Ok(()) => done.push(p),
                            Err(e) => failed.push((p, e)),
                        }
                    }
                    let _ = tx.send(Msg::Restored { done, failed });
                    wake();
                });
            }
            UndoOp::Move(moves) => {
                self.loader.background("gal-move", move |_| {
                    let (mut done, mut failed) = (Vec::new(), Vec::new());
                    for (from, to) in moves {
                        match move_or_copy(&to, &from, false) {
                            Ok(()) => done.push((to, from)),
                            Err(e) => failed.push((to, crate::error_text::io_reason(&e))),
                        }
                    }
                    let _ = tx.send(Msg::Moved {
                        undo: None,
                        copy: false,
                        done,
                        failed,
                    });
                    wake();
                });
            }
            UndoOp::Copy(copies) => {
                self.loader.background("gal-trash", move |_| {
                    let (mut done, mut failed) = (Vec::new(), Vec::new());
                    for p in copies {
                        match crate::clipboard::trash_with_sidecars(&p) {
                            Ok(()) => done.push(p),
                            Err(e) => failed.push((p, crate::error_text::trash_reason(&e))),
                        }
                    }
                    // Not undoable again: an unused id.
                    let _ = tx.send(Msg::Trashed {
                        undo: u64::MAX,
                        done,
                        failed,
                    });
                    wake();
                });
            }
        }
        self.redraw = true;
        true
    }

    /// Ctrl+C: the focused image's pixels (decoded first when needed).
    pub(super) fn gallery_copy_image(&mut self) {
        let Some(p) = self.gallery.focus.clone() else {
            return;
        };
        if !self
            .gallery
            .by_path
            .get(p.as_path())
            .is_some_and(|&i| self.gallery.entries[i].kind == EntryKind::Image)
        {
            return;
        }
        self.gallery.pending_copy = Some(p.clone());
        self.gallery_viewer_open(&p, Priority::Visible);
    }

    /// Ctrl+Shift+C: the targets' paths, one per line.
    pub(super) fn gallery_copy_paths(&mut self) {
        let t = self.gallery.targets();
        match t.as_slice() {
            [] => {}
            [one] => self.clipboard.copy_path(&self.loader, one),
            many => {
                let text = many
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                self.clipboard
                    .copy_text(&self.loader, text, crate::loader::Copied::Path);
            }
        }
    }

    /// Shows the focused file in the file manager.
    pub(super) fn gallery_reveal(&mut self) {
        let Some(p) = self.gallery.focus.clone() else {
            return;
        };
        self.loader.background("int-reveal", move |_| reveal(&p));
    }

    /// Opens the targets in a new Specular window each (up to 8).
    pub(super) fn gallery_new_window(&mut self) {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        for p in self.gallery.targets().into_iter().take(8) {
            if let Err(e) = std::process::Command::new(&exe)
                .arg("--new-window")
                .arg(&p)
                .spawn()
            {
                warn!(target: UI, "new window: {e}");
            }
        }
    }

    pub(super) fn gallery_select_all(&mut self) {
        let ga = &mut self.gallery;
        ga.selected = ga.tile_paths().cloned().collect();
        self.redraw = true;
    }

    /// IPC `select`: the gallery's selection becomes `paths` (with `add`, gains them), the
    /// focus on the first. The files it does not show are returned; `None` when the gallery
    /// is not open.
    pub(in crate::app) fn gallery_select_paths(
        &mut self,
        paths: &[PathBuf],
        add: bool,
    ) -> Option<(usize, Vec<PathBuf>)> {
        if !self.gallery.active {
            return None;
        }
        let ga = &mut self.gallery;
        let (shown, missing): (Vec<&PathBuf>, Vec<&PathBuf>) = paths
            .iter()
            .partition(|p| ga.by_path.contains_key(p.as_path()));
        if !add {
            ga.selected.clear();
        }
        ga.selected.extend(shown.iter().map(|p| (*p).clone()));
        if let Some(first) = shown.first()
            && let Some(i) = ga
                .tiles
                .iter()
                .position(|t| &ga.entries[t.entry].path == *first)
        {
            ga.set_focus(i);
        }
        let n = ga.selected.len();
        self.redraw = true;
        Some((n, missing.into_iter().cloned().collect()))
    }

    /// Focuses the next image without a rating (culling).
    pub(super) fn gallery_next_unrated(&mut self) {
        let ga = &mut self.gallery;
        let start = ga.focus_index().map_or(0, |i| i + 1);
        let n = ga.tiles.len();
        let found = (0..n).map(|k| (start + k) % n).find(|&i| {
            let e = &ga.entries[ga.tiles[i].entry];
            e.kind == EntryKind::Image
                && ga.marks.mark(&e.path).rating == 0
                && ga.marks.mark(&e.path).flag == 0
        });
        if let Some(i) = found {
            ga.set_focus(i);
            ga.selected.clear();
        }
        self.redraw = true;
    }

    /// `c`: compares the selected images (2–4) side by side.
    pub(super) fn gallery_compare(&mut self) {
        let ga = &mut self.gallery;
        if ga.compare.take().is_some() {
            self.redraw = true;
            return;
        }
        let mut imgs: Vec<PathBuf> = ga
            .tile_paths()
            .filter(|p| ga.selected.contains(*p))
            .filter(|p| {
                ga.by_path
                    .get(p.as_path())
                    .is_some_and(|&i| ga.entries[i].kind == EntryKind::Image)
            })
            .cloned()
            .collect();
        if imgs.len() < 2 {
            // The focus and the next image.
            if let Some(i) = ga.focus_index() {
                imgs = (i..ga.tiles.len())
                    .filter_map(|k| ga.entry_of(k))
                    .filter(|e| e.kind == EntryKind::Image)
                    .take(2)
                    .map(|e| e.path.clone())
                    .collect();
            }
        }
        imgs.truncate(4);
        if imgs.len() >= 2 {
            self.compare_start(imgs, compare::CmpMode::Side);
        } else {
            let now = self.now();
            self.chrome.toast(t!("gallery-compare-hint"), None, now);
        }
        self.redraw = true;
    }

    /// Saves the view options (per folder, or as the settings).
    pub(super) fn gallery_view_changed(&mut self) {
        let v = self.gallery.view;
        if self.settings.gallery.per_folder_view {
            if let Some(d) = self.gallery.dir.clone() {
                self.gallery.marks.set_view(&d, v);
                self.gallery.marks_due = Some(Instant::now() + Duration::from_millis(800));
            }
        } else if self.args.screenshot.is_none() {
            self.store.update(Origin::Program, |s| {
                s.gallery.layout = model::gallery_layout(v.mode);
                s.gallery.thumbnail_size = v.size.round() as i64;
                s.gallery.sort_order = model::gallery_sort(v.sort);
                s.gallery.sort_descending = v.desc;
                s.gallery.group_by = v.group;
            });
            self.settings = self.store.snapshot();
        }
    }

    /// Changes a display setting from the View menu (saved like the Settings window would).
    pub(super) fn gallery_setting(&mut self, f: impl FnOnce(&mut specular_settings::Settings)) {
        if self.args.screenshot.is_some() {
            let mut s = (*self.settings).clone();
            f(&mut s);
            self.settings = Arc::new(s);
        } else {
            self.store.update(Origin::Program, f);
            self.settings = self.store.snapshot();
        }
        self.gallery_sync_settings();
    }

    /// Settings → Gallery → Clear thumbnail cache.
    pub(super) fn gallery_clear_cache(&mut self) {
        self.gallery_prepare();
        let Some(cache) = self
            .gallery
            .engine
            .as_ref()
            .and_then(|e| e.cache().cloned())
        else {
            return;
        };
        if let Some(e) = &self.gallery.engine {
            e.forget_all();
        }
        let tx = self.gallery.tx.clone();
        let _ = tx;
        let now = self.now();
        match cache.clear() {
            Ok(()) => self.chrome.toast(t!("gallery-cache-cleared"), None, now),
            Err(e) => self.chrome.toast(
                t!(
                    "gallery-cache-clear-failed",
                    reason = crate::error_text::io_reason(&e)
                ),
                None,
                now,
            ),
        };
        self.gallery.wish_key = 0;
        self.redraw = true;
    }
}

/// A name in `p`'s folder that does not exist yet (`a.jpg`, `a (2).jpg`, …).
fn free_name(p: &Path) -> PathBuf {
    if !p.exists() {
        return p.to_path_buf();
    }
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (2..10_000)
        .map(|n| p.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|c| !c.exists())
        .unwrap_or_else(|| p.to_path_buf())
}

/// Renames, or copies (and removes the original when moving) across file systems.
/// [`move_or_copy_one`] for `from` and then its sidecars, which take `to`'s name (a sidecar
/// whose new name is taken stays).
fn move_or_copy(from: &Path, to: &Path, copy: bool) -> std::io::Result<()> {
    let sidecars = if from.is_file() {
        specular_fileops::names::sidecars(from, false)
    } else {
        Vec::new()
    };
    move_or_copy_one(from, to, copy)?;
    let Some(dir) = to.parent() else {
        return Ok(());
    };
    for s in sidecars {
        let Some(n) = specular_fileops::names::sidecar_new_name(&s, from, to) else {
            continue;
        };
        if let Err(e) = move_or_copy_one(&s, &dir.join(&n), copy) {
            specular_log::warn!(target: specular_log::FOLDER, "gallery: sidecar {}: {e}", s.display());
        }
    }
    Ok(())
}

fn move_or_copy_one(from: &Path, to: &Path, copy: bool) -> std::io::Result<()> {
    if to.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "the name is taken",
        ));
    }
    if !copy && std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if from.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "folders are moved within one drive only",
        ));
    }
    std::fs::copy(from, to)?;
    if !copy {
        std::fs::remove_file(from)?;
    }
    Ok(())
}

/// Opens the file manager at `p` with it selected.
pub(super) fn reveal(p: &Path) {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg("-R").arg(p).status();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer")
        .arg(format!("/select,{}", p.display()))
        .status();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let r = {
        // The freedesktop FileManager1 interface selects the file; else open the folder.
        let uri = format!("file://{}", percent_path(p));
        let shown = std::process::Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.FileManager1",
                "/org/freedesktop/FileManager1",
                "org.freedesktop.FileManager1.ShowItems",
            ])
            .arg(format!("array:string:{uri}"))
            .arg("string:")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        match shown {
            Ok(s) if s.success() => shown,
            _ => {
                let mut cmd = std::process::Command::new("xdg-open");
                specular_platform::launch::scrub_env(&mut cmd);
                cmd.arg(p.parent().unwrap_or(p)).status()
            }
        }
    };
    if let Err(e) = r {
        warn!(target: UI, "reveal {}: {e}", p.display());
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn percent_path(p: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut s = String::new();
    for &b in p.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

// ----- Menus and palette entries ----------------------------------------------------------

/// Codes of the gallery's palette and menu entries (`ITEM_BASE + code`).
/// A quick filter's query term and the terms it replaces (palette codes `F_*`); an empty
/// term clears the query.
fn quick_filter(c: u64) -> (String, &'static [&'static str]) {
    const LABELS: [&str; 4] = ["red", "yellow", "green", "blue"];
    match c {
        code::F_PICKS => ("is:pick".into(), &[]),
        code::F_REJECTS => ("-is:reject".into(), &[]),
        code::F_UNRATED => ("is:unrated".into(), &[]),
        c if (code::F_RATING..code::F_RATING + 5).contains(&c) => {
            (format!("rating>={}", c - code::F_RATING + 1), &["rating>="])
        }
        c if (code::F_LABEL..code::F_LABEL + 4).contains(&c) => (
            format!("label:{}", LABELS[(c - code::F_LABEL) as usize]),
            &["label:"],
        ),
        code::F_ANIMATED => ("is:animated".into(), &[]),
        code::F_VIDEO => ("is:video".into(), &[]),
        code::F_SOUND => ("is:sound".into(), &[]),
        code::F_HDR => ("is:hdr".into(), &[]),
        code::F_RAW => ("is:raw".into(), &[]),
        code::F_LANDSCAPE => ("orientation:landscape".into(), &["orientation:"]),
        code::F_PORTRAIT => ("orientation:portrait".into(), &["orientation:"]),
        _ => (String::new(), &[]),
    }
}

mod code {
    pub const LAYOUT: u64 = 1; // + 0..4 (squares, justified, uncropped, details)
    pub const SORT: u64 = 10; // + SortKey::ALL index
    pub const REVERSE: u64 = 16;
    pub const CAPTIONS: u64 = 30;
    pub const BADGES: u64 = 31;
    pub const EDITED_BADGES: u64 = 37;
    pub const INFO: u64 = 32;
    pub const STACKS: u64 = 33;
    pub const HIDDEN: u64 = 34;
    pub const OTHER: u64 = 35;
    pub const DEPTH: u64 = 36; // + 0..5 (0, 1, 2, 3, all)
    pub const FILTER: u64 = 42;
    pub const SETTINGS: u64 = 43;
    pub const EMBEDDED: u64 = 44;
    pub const ARCHIVES: u64 = 45;
    pub const OPEN: u64 = 50;
    pub const NEW_WINDOW: u64 = 51;
    pub const OPEN_WITH: u64 = 52;
    pub const REVEAL: u64 = 53;
    pub const COPY_IMAGE: u64 = 54;
    pub const COPY_PATHS: u64 = 55;
    pub const TRASH: u64 = 56;
    pub const PROPERTIES: u64 = 57;
    pub const SELECT_ALL: u64 = 58;
    pub const SELECT_NONE: u64 = 59;
    pub const INVERT: u64 = 60;
    pub const COMPARE: u64 = 61;
    pub const NEXT_UNRATED: u64 = 62;
    pub const STACK: u64 = 63;
    pub const UP: u64 = 64;
    pub const MOVE_HERE: u64 = 65;
    pub const COPY_HERE: u64 = 66;
    /// Share with Zynk / Send with Zynk (the selection).
    pub const ZYNK_SHARE: u64 = 67;
    pub const ZYNK_SEND: u64 = 68;
    /// Open as… (which format to read the file as).
    pub const OPEN_AS: u64 = 69;
    pub const F_PICKS: u64 = 70;
    pub const F_REJECTS: u64 = 71;
    pub const F_UNRATED: u64 = 72;
    pub const F_RATING: u64 = 73; // + 0..5 (1–5 stars)
    pub const F_LABEL: u64 = 78; // + 0..4
    pub const F_ANIMATED: u64 = 82;
    pub const F_HDR: u64 = 83;
    pub const F_RAW: u64 = 84;
    pub const F_LANDSCAPE: u64 = 85;
    pub const F_PORTRAIT: u64 = 86;
    pub const F_CLEAR: u64 = 87;
    pub const F_VIDEO: u64 = 88;
    pub const F_SOUND: u64 = 89;
    pub const RATE: u64 = 90; // + 0..6
    pub const PICK: u64 = 96;
    pub const REJECT: u64 = 97;
    pub const UNFLAG: u64 = 98;
    pub const LABEL: u64 = 99; // + 0..5 (none, red, yellow, green, blue)
    pub const MENU_VIEW: u64 = 110;
    pub const MENU_SORT: u64 = 111;
    pub const MENU_GROUP: u64 = 112;
    pub const MENU_LAYOUT: u64 = 113;
    pub const MENU_DEPTH: u64 = 114;
    pub const MENU_FILTER: u64 = 115;
    pub const MENU_MORE: u64 = 116;
    pub const MENU_RATE: u64 = 117;
    pub const FACETS: u64 = 118;
    pub const GROUP: u64 = 120; // + GROUPS index (up to 79)
    pub const NOTHING: u64 = 200;
    /// Play next / Add to queue (the selection's sound files and videos).
    pub const QUEUE_NEXT: u64 = 201;
    pub const QUEUE_ADD: u64 = 202;
    /// "Play on…": opens the file and the player's menu of devices.
    pub const CAST: u64 = 203;
    /// Merge to HDR…: the selected raws, or the bracket around the one.
    pub const MERGE_HDR: u64 = 204;
    /// Albums: put the selection in one, and take it out of the album shown.
    pub const ALBUM_ADD: u64 = 205;
    pub const ALBUM_REMOVE: u64 = 206;
    /// The metadata editor on the selection.
    pub const EDIT_METADATA: u64 = 207;
    /// Merge to panorama…: the selected photos, or the panorama around the one.
    pub const MERGE_PANO: u64 = 208;
    /// Focus stack…: the selected raws, or the focus stack around the one.
    pub const FOCUS_STACK: u64 = 209;
}

const DEPTHS: [u32; 5] = [0, 1, 2, 3, 16];
const MODES: [Mode; 4] = [
    Mode::Squares,
    Mode::Justified,
    Mode::Uncropped,
    Mode::Details,
];

/// More choices to pick from: in a menu at `at`, or where the right-click menu that led to
/// them was, else in the palette's list.
fn choices(
    chrome: &mut specular_widgets::Chrome,
    at: Option<(f32, f32)>,
    title: String,
    items: Vec<PaletteItem>,
) {
    // Pointed at in the right-click menu: beside the row, as its submenu.
    let Err(items) = chrome.open_submenu(items, None) else {
        return;
    };
    match at.or(chrome.menu_origin()) {
        Some(at) => chrome.open_menu(at, Some(title), vec![items]),
        None => chrome.open_list(title, items, None),
    }
}

fn item(c: u64, label: String, key: Option<&str>, icon: Icon) -> PaletteItem {
    PaletteItem {
        id: ITEM_BASE + c,
        label,
        key: key.map(str::to_owned),
        unavailable: None,
        icon,
    }
}

fn check(c: u64, label: String, on: bool, key: Option<&str>) -> PaletteItem {
    item(c, label, key, if on { Icon::Check } else { Icon::Minus })
}

fn tr(id: &str) -> String {
    I18n::global().tr(id)
}

pub(super) fn mode_label(m: Mode) -> String {
    tr(&format!(
        "settings-gallery-layout-{}",
        model::gallery_layout(m).to_string().replace('_', "-")
    ))
}

pub(super) fn sort_label(k: SortKey) -> String {
    tr(&format!(
        "settings-gallery-sort-order-{}",
        model::gallery_sort(k).to_string().replace('_', "-")
    ))
}

pub(super) fn group_name(g: GalleryGroup) -> String {
    tr(&format!("settings-gallery-group-by-{}", model::group_id(g)))
}

fn label_name(l: u8) -> String {
    match l {
        1 => t!("gallery-label-red"),
        2 => t!("gallery-label-yellow"),
        3 => t!("gallery-label-green"),
        4 => t!("gallery-label-blue"),
        _ => t!("gallery-label-none"),
    }
}

fn depth_label(d: u32) -> String {
    match d {
        0 => t!("gallery-depth-off"),
        16.. => t!("gallery-depth-all"),
        n => t!("gallery-depth-levels", depth = n as i64),
    }
}

impl App {
    /// The gallery's entries among the palette's actions (always listed; choosing one
    /// opens the gallery first).
    pub(super) fn gallery_palette_items(&self) -> Vec<PaletteItem> {
        vec![
            item(
                code::MENU_VIEW,
                t!("gallery-palette-view"),
                None,
                Icon::Grid,
            ),
            item(
                code::MENU_LAYOUT,
                t!("gallery-palette-layout"),
                None,
                Icon::Grid,
            ),
            item(
                code::MENU_SORT,
                t!("gallery-palette-sort"),
                None,
                Icon::Grid,
            ),
            item(
                code::MENU_GROUP,
                t!("gallery-palette-group"),
                None,
                Icon::Grid,
            ),
            item(
                code::MENU_DEPTH,
                t!("gallery-palette-subfolders"),
                None,
                Icon::Folder,
            ),
            item(
                code::FILTER,
                t!("gallery-palette-filter"),
                Some("Ctrl+F"),
                Icon::Search,
            ),
            item(code::FACETS, t!("gallery-facets"), None, Icon::Search),
            item(
                code::MENU_FILTER,
                t!("gallery-palette-quick-filters"),
                None,
                Icon::Search,
            ),
            item(
                code::LAYOUT + 3,
                t!("gallery-palette-details"),
                None,
                Icon::Grid,
            ),
            item(
                code::INFO,
                t!("gallery-palette-info-overlay"),
                None,
                Icon::Info,
            ),
            item(
                code::STACKS,
                t!("gallery-palette-stacks"),
                None,
                Icon::Image,
            ),
            item(
                code::COMPARE,
                t!("gallery-palette-compare"),
                Some("C"),
                Icon::Image,
            ),
            item(
                code::SELECT_ALL,
                t!("gallery-select-all"),
                Some("Ctrl+A"),
                Icon::Check,
            ),
            item(code::INVERT, t!("gallery-select-invert"), None, Icon::Check),
            item(
                code::NEXT_UNRATED,
                t!("gallery-next-unrated"),
                None,
                Icon::StepForward,
            ),
            item(
                code::MENU_RATE,
                t!("gallery-palette-rate"),
                None,
                Icon::Check,
            ),
            item(
                code::F_PICKS,
                t!("gallery-filter-picks"),
                None,
                Icon::Search,
            ),
            item(
                code::SETTINGS,
                t!("gallery-palette-settings"),
                None,
                Icon::Settings,
            ),
            item(
                map::code::OPEN,
                t!("gallery-palette-map"),
                None,
                Icon::Globe,
            ),
        ]
        .into_iter()
        .chain(self.similar_menu_items())
        .chain(self.compare_menu_items())
        .chain(self.map_menu_items())
        .chain(self.people_menu_items())
        .chain(self.import_menu_items())
        .chain(self.local_menu_items(false))
        .collect()
    }

    /// One of our palette or menu entries was chosen; false when `id` is not ours.
    pub(super) fn gallery_item(&mut self, id: u64) -> bool {
        if !(ITEM_BASE..ITEM_BASE + 1000).contains(&id) {
            return false;
        }
        let c = id - ITEM_BASE;
        if !self.gallery.active && c != code::NOTHING {
            self.gallery_enter();
        }
        self.gallery_code(c);
        self.redraw = true;
        true
    }

    fn gallery_code(&mut self, c: u64) {
        if local::code::owns(c) {
            self.local_code(c);
            return;
        }
        if import::code::owns(c) {
            self.import_code(c);
            return;
        }
        if map::code::owns(c) {
            self.map_code(c);
            return;
        }
        if people::code::owns(c) {
            self.people_code(c);
            return;
        }
        if similar::code::owns(c) {
            self.similar_code(c);
            return;
        }
        if compare::code::owns(c) {
            self.compare_code(c);
            return;
        }
        let ga = &mut self.gallery;
        match c {
            c if (code::LAYOUT..code::LAYOUT + 4).contains(&c) => {
                let m = MODES[(c - code::LAYOUT) as usize];
                // The details entry toggles back to the grid.
                ga.view.mode = if m == Mode::Details && ga.view.mode == Mode::Details {
                    Mode::Squares
                } else {
                    m
                };
                ga.relayout = true;
                self.gallery_view_changed();
            }
            c if (code::SORT..code::SORT + 6).contains(&c) => {
                ga.view.sort = SortKey::ALL[(c - code::SORT) as usize];
                ga.resort();
                self.gallery_view_changed();
                self.gallery_after_listing();
            }
            code::REVERSE => {
                ga.view.desc = !ga.view.desc;
                ga.resort();
                self.gallery_view_changed();
            }
            c if (code::GROUP..code::GROUP + model::GROUPS.len() as u64).contains(&c) => {
                ga.view.group = model::GROUPS[(c - code::GROUP) as usize];
                ga.dirty = true;
                self.gallery_view_changed();
                self.gallery_after_listing();
            }
            code::CAPTIONS => self.gallery_setting(|s| s.gallery.captions = !s.gallery.captions),
            code::BADGES => self.gallery_setting(|s| s.gallery.badges = !s.gallery.badges),
            code::EDITED_BADGES => {
                self.gallery_setting(|s| s.gallery.edited_badges = !s.gallery.edited_badges);
            }
            code::INFO => {
                self.gallery_setting(|s| s.gallery.info_overlay = !s.gallery.info_overlay)
            }
            code::STACKS => self.gallery_setting(|s| s.gallery.stacks = !s.gallery.stacks),
            code::HIDDEN => {
                self.gallery_setting(|s| s.gallery.show_hidden = !s.gallery.show_hidden)
            }
            code::OTHER => {
                self.gallery_setting(|s| s.gallery.show_other_files = !s.gallery.show_other_files)
            }
            code::ARCHIVES => {
                self.gallery_setting(|s| s.gallery.show_archives = !s.gallery.show_archives)
            }
            code::EMBEDDED => {
                self.gallery_setting(|s| s.gallery.prefer_embedded = !s.gallery.prefer_embedded);
                if let Some(e) = &self.gallery.engine {
                    e.forget_all();
                }
                self.gallery.failed.clear();
                if let Some(a) = self.gallery.atlas.as_mut() {
                    a.clear();
                }
                self.gallery.wish_key = 0;
            }
            c if (code::DEPTH..code::DEPTH + 5).contains(&c) => {
                let d = DEPTHS[(c - code::DEPTH) as usize] as i64;
                self.gallery_setting(|s| s.gallery.subfolder_depth = d);
            }
            code::FILTER => {
                ga.filter_open = true;
                ga.filter_focus = true;
            }
            code::FACETS => {
                let open = !ga.facets.open;
                ga.facets_show(open, open);
            }
            code::SETTINGS => self.chrome.open_settings_at("gallery"),
            code::OPEN => {
                if let Some(i) = ga.focus_index() {
                    self.gallery_open(i);
                }
            }
            code::NEW_WINDOW => self.gallery_new_window(),
            code::OPEN_WITH => self.open_with(),
            code::ALBUM_ADD => self.albums_add_key(true),
            code::EDIT_METADATA => self.meta_open(),
            code::ALBUM_REMOVE => self.albums_remove_selected(),
            code::OPEN_AS => self.file_kind_open_as(),
            code::MERGE_HDR | code::MERGE_PANO | code::FOCUS_STACK => {
                let targets = ga.targets();
                let item = match c {
                    code::MERGE_PANO => merged::MergeItem::Pano,
                    code::FOCUS_STACK => merged::MergeItem::Stack,
                    _ => merged::MergeItem::Hdr,
                };
                self.gallery_merge_item(item, targets);
            }
            code::ZYNK_SHARE => self.share_with_zynk(),
            code::ZYNK_SEND => self.send_with_zynk(),
            code::QUEUE_NEXT => {
                let paths = ga.targets();
                self.queue_add_paths(&paths, true);
            }
            code::QUEUE_ADD => {
                let paths = ga.targets();
                self.queue_add_paths(&paths, false);
            }
            code::CAST => {
                if let Some(i) = ga.focus_index() {
                    self.gallery_open(i);
                    self.cast.menu_wanted = Some(std::time::Instant::now());
                }
            }
            code::REVEAL => self.gallery_reveal(),
            code::MOVE_HERE => self.gallery_move_selected_here(false),
            code::COPY_HERE => self.gallery_move_selected_here(true),
            code::COPY_IMAGE => self.gallery_copy_image(),
            code::COPY_PATHS => self.gallery_copy_paths(),
            code::TRASH => self.gallery_trash(),
            code::PROPERTIES => self.gallery_properties(),
            code::SELECT_ALL => self.gallery_select_all(),
            code::SELECT_NONE => ga.selected.clear(),
            code::INVERT => {
                ga.selected = ga
                    .tile_paths()
                    .filter(|p| !ga.selected.contains(*p))
                    .cloned()
                    .collect();
            }
            code::COMPARE => self.gallery_compare(),
            code::NEXT_UNRATED => self.gallery_next_unrated(),
            code::STACK => self.gallery_toggle_stack(),
            code::UP => self.gallery_up(),
            c if (code::F_PICKS..=code::F_SOUND).contains(&c) => {
                let (term, family) = quick_filter(c);
                if term.is_empty() {
                    ga.filter.text.clear();
                } else {
                    ga.filter.toggle(&term, family);
                }
                ga.filter_open = ga.filter_open || !ga.filter.is_empty();
                ga.dirty = true;
            }
            c if (code::RATE..code::RATE + 6).contains(&c) => {
                let r = (c - code::RATE) as u8;
                self.gallery_mark(move |m| Mark { rating: r, ..m });
            }
            code::PICK => self.gallery_mark(|m| Mark {
                flag: if m.flag == 1 { 0 } else { 1 },
                ..m
            }),
            code::REJECT => self.gallery_mark(|m| Mark {
                flag: if m.flag == -1 { 0 } else { -1 },
                ..m
            }),
            code::UNFLAG => self.gallery_mark(|m| Mark { flag: 0, ..m }),
            c if (code::LABEL..code::LABEL + 5).contains(&c) => {
                let l = (c - code::LABEL) as u8;
                self.gallery_mark(move |m| Mark { label: l, ..m });
            }
            code::MENU_VIEW => self.gallery_menu_view(None),
            code::MENU_SORT => {
                let v = ga.view;
                let mut items: Vec<PaletteItem> = SortKey::ALL
                    .iter()
                    .enumerate()
                    .map(|(i, k)| check(code::SORT + i as u64, sort_label(*k), v.sort == *k, None))
                    .collect();
                items.push(check(
                    code::REVERSE,
                    tr("settings-gallery-sort-descending"),
                    v.desc,
                    None,
                ));
                choices(&mut self.chrome, None, t!("gallery-palette-sort"), items);
            }
            code::MENU_GROUP => {
                let g = ga.view.group;
                let items = model::GROUPS
                    .iter()
                    .enumerate()
                    .map(|(i, k)| check(code::GROUP + i as u64, group_name(*k), g == *k, None))
                    .collect();
                choices(&mut self.chrome, None, t!("gallery-palette-group"), items);
            }
            code::MENU_LAYOUT => {
                let m = ga.view.mode;
                let items = MODES
                    .iter()
                    .enumerate()
                    .map(|(i, k)| check(code::LAYOUT + i as u64, mode_label(*k), m == *k, None))
                    .collect();
                choices(&mut self.chrome, None, t!("gallery-palette-layout"), items);
            }
            code::MENU_DEPTH => {
                let d = ga.depth;
                let items = DEPTHS
                    .iter()
                    .enumerate()
                    .map(|(i, k)| check(code::DEPTH + i as u64, depth_label(*k), d == *k, None))
                    .collect();
                choices(
                    &mut self.chrome,
                    None,
                    t!("gallery-palette-subfolders"),
                    items,
                );
            }
            code::MENU_FILTER => {
                let items = self.gallery_filter_items();
                choices(
                    &mut self.chrome,
                    None,
                    t!("gallery-palette-quick-filters"),
                    items,
                );
            }
            code::MENU_RATE => {
                let items = self.gallery_rate_items();
                choices(&mut self.chrome, None, t!("gallery-palette-rate"), items);
            }
            code::MENU_MORE => self.gallery_menu_more(),
            _ => {}
        }
    }

    fn gallery_filter_items(&self) -> Vec<PaletteItem> {
        let f = &self.gallery.filter;
        let on = |c: u64| f.has(&quick_filter(c).0);
        let mut v = vec![
            check(
                code::F_PICKS,
                t!("gallery-filter-picks"),
                on(code::F_PICKS),
                None,
            ),
            check(
                code::F_REJECTS,
                t!("gallery-filter-hide-rejects"),
                on(code::F_REJECTS),
                None,
            ),
            check(
                code::F_UNRATED,
                t!("gallery-filter-unrated"),
                on(code::F_UNRATED),
                None,
            ),
        ];
        for r in 1..=5u8 {
            let c = code::F_RATING + u64::from(r - 1);
            v.push(check(
                c,
                t!("gallery-filter-rating", stars = i64::from(r)),
                on(c),
                None,
            ));
        }
        for l in 1..=4u8 {
            let c = code::F_LABEL + u64::from(l - 1);
            v.push(check(
                c,
                t!("gallery-filter-label", label = label_name(l)),
                on(c),
                None,
            ));
        }
        for (c, label) in [
            (code::F_ANIMATED, t!("gallery-filter-animated")),
            (code::F_VIDEO, t!("gallery-filter-video")),
            (code::F_SOUND, t!("gallery-filter-sound")),
            (code::F_HDR, t!("gallery-filter-hdr")),
            (code::F_RAW, t!("gallery-filter-raw")),
            (code::F_LANDSCAPE, t!("gallery-filter-landscape")),
            (code::F_PORTRAIT, t!("gallery-filter-portrait")),
        ] {
            v.push(check(c, label, on(c), None));
        }
        v.push(item(
            code::F_CLEAR,
            t!("gallery-filter-clear"),
            None,
            Icon::Close,
        ));
        v
    }

    fn gallery_rate_items(&self) -> Vec<PaletteItem> {
        let m = self
            .gallery
            .focus
            .as_deref()
            .map(|p| self.gallery.marks.mark(p))
            .unwrap_or_default();
        let mut v: Vec<PaletteItem> = (0..=5u8)
            .map(|r| {
                let label = if r == 0 {
                    t!("gallery-rate-none")
                } else {
                    t!("gallery-stars", stars = i64::from(r))
                };
                check(
                    code::RATE + u64::from(r),
                    label,
                    m.rating == r,
                    Some(&r.to_string()),
                )
            })
            .collect();
        v.push(check(
            code::PICK,
            t!("gallery-pick"),
            m.flag == 1,
            Some("P"),
        ));
        v.push(check(
            code::REJECT,
            t!("gallery-reject"),
            m.flag == -1,
            Some("X"),
        ));
        v.push(item(
            code::UNFLAG,
            t!("gallery-unflag"),
            Some("U"),
            Icon::Minus,
        ));
        for l in 0..=4u8 {
            let key = if l == 0 {
                None
            } else {
                Some((l + 5).to_string())
            };
            v.push(check(
                code::LABEL + u64::from(l),
                label_name(l),
                m.label == l,
                key.as_deref(),
            ));
        }
        v
    }

    /// The View menu (top bar).
    /// The view menu (layout, sort, grouping, depth, filters); `at`: opened by a right click
    /// there, as a menu.
    pub(super) fn gallery_menu_view(&mut self, at: Option<(f32, f32)>) {
        let ga = &self.gallery;
        let v = ga.view;
        let mut items: Vec<PaletteItem> = MODES
            .iter()
            .enumerate()
            .map(|(i, k)| check(code::LAYOUT + i as u64, mode_label(*k), v.mode == *k, None))
            .collect();
        items.push(item(
            code::MENU_SORT,
            format!("{} — {}", t!("gallery-palette-sort"), sort_label(v.sort)),
            None,
            Icon::ChevronRight,
        ));
        items.push(item(
            code::MENU_GROUP,
            format!("{} — {}", t!("gallery-palette-group"), group_name(v.group)),
            None,
            Icon::ChevronRight,
        ));
        items.push(item(
            code::MENU_DEPTH,
            format!(
                "{} — {}",
                t!("gallery-palette-subfolders"),
                depth_label(ga.depth)
            ),
            None,
            Icon::ChevronRight,
        ));
        items.push(item(
            code::FILTER,
            t!("gallery-palette-filter"),
            Some("Ctrl+F"),
            Icon::Search,
        ));
        items.push(check(
            code::FACETS,
            t!("gallery-facets"),
            ga.facets.open,
            None,
        ));
        items.push(item(
            code::MENU_FILTER,
            t!("gallery-palette-quick-filters"),
            None,
            Icon::ChevronRight,
        ));
        items.push(check(
            code::CAPTIONS,
            tr("settings-gallery-captions"),
            ga.show_captions,
            None,
        ));
        items.push(check(
            code::BADGES,
            tr("settings-gallery-badges"),
            ga.show_badges,
            None,
        ));
        items.push(check(
            code::EDITED_BADGES,
            tr("settings-gallery-edited-badges"),
            ga.edited_badges,
            None,
        ));
        items.push(check(
            code::INFO,
            tr("settings-gallery-info-overlay"),
            ga.info_overlay,
            None,
        ));
        items.push(check(
            code::STACKS,
            tr("settings-gallery-stacks"),
            ga.stacks_on,
            None,
        ));
        items.push(check(
            code::HIDDEN,
            tr("settings-gallery-show-hidden"),
            ga.show_hidden,
            None,
        ));
        items.push(check(
            code::OTHER,
            tr("settings-gallery-show-other-files"),
            ga.show_other,
            None,
        ));
        items.push(check(
            code::ARCHIVES,
            tr("settings-gallery-show-archives"),
            ga.show_archives,
            None,
        ));
        items.push(check(
            code::EMBEDDED,
            tr("settings-gallery-prefer-embedded"),
            self.settings.gallery.prefer_embedded,
            None,
        ));
        items.push(item(
            code::SETTINGS,
            t!("gallery-palette-settings"),
            None,
            Icon::Settings,
        ));
        items.push(item(
            map::code::OPEN,
            t!("gallery-palette-map"),
            None,
            Icon::Globe,
        ));
        items.extend(self.similar_menu_items());
        choices(&mut self.chrome, at, t!("gallery-view-menu"), items);
    }

    /// The overflow (⋯) menu of the bottom bar: selection, culling and batch actions.
    pub(super) fn gallery_menu_more(&mut self) {
        let mut items = vec![
            item(
                code::SELECT_ALL,
                t!("gallery-select-all"),
                Some("Ctrl+A"),
                Icon::Check,
            ),
            item(
                code::SELECT_NONE,
                t!("gallery-select-none"),
                None,
                Icon::Close,
            ),
            item(code::INVERT, t!("gallery-select-invert"), None, Icon::Check),
            item(
                code::COMPARE,
                t!("gallery-palette-compare"),
                Some("C"),
                Icon::Image,
            ),
            item(
                code::NEXT_UNRATED,
                t!("gallery-next-unrated"),
                None,
                Icon::StepForward,
            ),
            item(
                code::MENU_RATE,
                t!("gallery-palette-rate"),
                None,
                Icon::ChevronRight,
            ),
            item(
                code::MENU_FILTER,
                t!("gallery-palette-quick-filters"),
                None,
                Icon::ChevronRight,
            ),
            item(
                code::COPY_PATHS,
                t!("gallery-copy-paths"),
                Some("Ctrl+Shift+C"),
                Icon::Copy,
            ),
        ];
        items.extend(self.gallery_batch_items());
        items.push(item(
            code::TRASH,
            tr("palette-move-to-trash"),
            Some("Delete"),
            Icon::Trash,
        ));
        items.push(item(
            code::PROPERTIES,
            t!("gallery-properties"),
            Some("Alt+Enter"),
            Icon::Info,
        ));
        choices(&mut self.chrome, None, t!("gallery-more-menu"), items);
    }

    /// User commands, run on the whole selection.
    fn gallery_batch_items(&self) -> Vec<PaletteItem> {
        let n = self.gallery.targets().len();
        self.command_items()
            .into_iter()
            .map(|mut i| {
                if n > 1 {
                    i.label = t!("gallery-run-on", name = i.label, count = n as i64);
                }
                i
            })
            .collect()
    }

    /// Right click on a tile at `at`, or the menu key (`None`: at the focused tile). A right
    /// click on no tile gives the view menu.
    pub(super) fn gallery_context_menu(&mut self, at: Option<(f32, f32)>) {
        let ga = &self.gallery;
        let Some(i) = ga.focus_index() else {
            self.gallery_menu_view(at);
            return;
        };
        let Some(e) = ga.entry_of(i) else { return };
        // From the keyboard, at the tile: just inside its top corner on the start side.
        let Some(at) = at.or_else(|| {
            ga.tile_rect(i).map(|r| {
                let x = if ga.rtl { r.right() - 16.0 } else { r.x + 16.0 };
                (x, r.y + 16.0)
            })
        }) else {
            return;
        };
        let kind = e.kind;
        let name = e.name.clone();
        let stack = ga.tiles[i].members.len() > 1;
        let n = ga.targets().len();
        let file = kind != EntryKind::Folder;

        // Groups by what one does with it: open it, hand it on, organise it, the file itself
        // (copy, find, facts), and trash it last, apart. A folder has fewer rows, so its
        // groups merge rather than each row standing alone between dividers.
        let mut open = vec![item(
            code::OPEN,
            t!("gallery-open"),
            Some("Enter"),
            if file { Icon::Image } else { Icon::Folder },
        )];
        if file {
            open.push(item(
                code::NEW_WINDOW,
                t!("gallery-new-window"),
                None,
                Icon::Fullscreen,
            ));
        }
        let mut commands = Vec::new();
        if kind == EntryKind::Image || kind == EntryKind::Other {
            // Its applications open beside it (`open_with`).
            open.push(item(
                code::OPEN_WITH,
                tr("palette-open-with"),
                None,
                Icon::ChevronRight,
            ));
            commands = self.gallery_batch_items();
            // A file whose extension several formats share: which one to read it as.
            if ga
                .path_of(i)
                .is_some_and(|p| crate::app::file_kind::offers_open_as(&p))
            {
                open.push(item(
                    code::OPEN_AS,
                    tr("palette-open-as"),
                    None,
                    Icon::ChevronRight,
                ));
            }
        }

        // Folders too: Zynk sends and shares them whole. Offered without Zynk as well, which
        // they then explain.
        let zynk = vec![
            item(
                code::ZYNK_SHARE,
                tr("palette-share-with-zynk"),
                None,
                Icon::Zynk,
            ),
            item(
                code::ZYNK_SEND,
                tr("palette-send-with-zynk"),
                None,
                Icon::Zynk,
            ),
        ];

        let mut file_rows = Vec::new();
        if kind == EntryKind::Image {
            file_rows.push(item(
                code::COPY_IMAGE,
                tr("palette-copy-image"),
                Some("Ctrl+C"),
                Icon::Copy,
            ));
        }
        file_rows.push(item(
            code::COPY_PATHS,
            if n > 1 {
                t!("gallery-copy-paths")
            } else {
                tr("palette-copy-path")
            },
            Some("Ctrl+Shift+C"),
            Icon::Copy,
        ));

        file_rows.push(item(code::REVEAL, t!("gallery-reveal"), None, Icon::Folder));
        file_rows.push(item(
            code::PROPERTIES,
            t!("gallery-properties"),
            Some("Alt+Enter"),
            Icon::Info,
        ));
        // A folder with files selected elsewhere: the keyboard way of dropping them on it.
        let mut place = Vec::new();
        if kind == EntryKind::Folder
            && let Some(dest) = ga.path_of(i)
            && !ga.selected_except(&dest).is_empty()
        {
            place.push(item(
                code::MOVE_HERE,
                t!("gallery-move-here"),
                None,
                Icon::Folder,
            ));
            place.push(item(
                code::COPY_HERE,
                t!("gallery-copy-here"),
                None,
                Icon::Copy,
            ));
        }

        let mut organise = Vec::new();
        if kind == EntryKind::Image {
            organise.push(item(
                code::MENU_RATE,
                t!("gallery-palette-rate"),
                None,
                Icon::ChevronRight,
            ));
            organise.push(item(
                code::EDIT_METADATA,
                t!("metaedit-menu"),
                Some("Ctrl+Alt+I"),
                Icon::Info,
            ));
        }
        // Merges: HDR (raws into a raw, other photos by exposure fusion), panoramas, and
        // focus stacks of raws.
        if kind == EntryKind::Image {
            let any_raw = ga.targets().iter().any(|p| {
                p.extension()
                    .is_some_and(|e| model::is_raw_ext(&e.to_string_lossy()))
            });
            organise.push(item(
                code::MERGE_HDR,
                t!("gallery-merge-hdr"),
                None,
                Icon::Image,
            ));
            organise.push(item(
                code::MERGE_PANO,
                t!("gallery-merge-pano"),
                None,
                Icon::Image,
            ));
            if any_raw {
                organise.push(item(
                    code::FOCUS_STACK,
                    t!("gallery-focus-stack"),
                    None,
                    Icon::Image,
                ));
            }
        }
        // Albums: in the Library's catalogue.
        if kind == EntryKind::Image && ga.library.lib.as_ref().is_some_and(|l| !l.read_only()) {
            organise.push(item(
                code::ALBUM_ADD,
                t!("albums-add-menu"),
                None,
                Icon::Plus,
            ));
            if ga.library.on && ga.albums_open_manual().is_some() {
                organise.push(item(
                    code::ALBUM_REMOVE,
                    t!("albums-remove-menu"),
                    None,
                    Icon::Minus,
                ));
            }
        }
        if kind == EntryKind::Image {
            organise.extend(self.local_menu_items(true));
        }
        if stack {
            organise.push(item(
                code::STACK,
                t!("gallery-stack-toggle"),
                Some("S"),
                Icon::Grid,
            ));
        }

        // Sound files, videos, playlists and folders go into the play queue (queue.rs).
        let playable = ga.targets().iter().any(|p| {
            p.is_dir()
                || crate::app::queue::Kind::of(p).is_some()
                || specular_playlist::Kind::of_path(p).is_some()
        });
        let mut queue = Vec::new();
        if playable {
            queue.push(item(
                code::QUEUE_NEXT,
                tr("palette-queue-play-next"),
                None,
                Icon::Play,
            ));
            queue.push(item(
                code::QUEUE_ADD,
                tr("palette-queue-add"),
                None,
                Icon::Queue,
            ));
        }
        // One movie or sound file: it can play on a TV or speaker.
        if n == 1
            && file
            && ga
                .path_of(i)
                .is_some_and(|p| crate::app::queue::Kind::of(&p).is_some())
        {
            queue.push(item(
                code::CAST,
                tr("palette-cast-menu"),
                Some("Shift+O"),
                Icon::Play,
            ));
        }

        let trash = vec![item(
            code::TRASH,
            if n > 1 {
                t!("gallery-trash-count", count = n as i64)
            } else {
                tr("palette-move-to-trash")
            },
            Some("Delete"),
            Icon::Trash,
        )];

        // What it acts on: the file, or how many when more are selected.
        let title = if n > 1 {
            t!("gallery-items", count = n as i64)
        } else {
            name
        };
        let groups = vec![
            open, queue, zynk, commands, organise, file_rows, place, trash,
        ];
        self.chrome.open_menu(at, Some(title), groups);
        // The tile it opened on shows no tooltip once it closes, until the pointer moves.
        self.gallery.hover = None;
    }

    /// `s`: expands or collapses the focused stack.
    pub(super) fn gallery_toggle_stack(&mut self) {
        let ga = &mut self.gallery;
        let Some(i) = ga.focus_index() else { return };
        let Some(first) = ga.tiles[i]
            .members
            .first()
            .map(|&m| ga.entries[m].path.clone())
        else {
            return;
        };
        if !ga.expanded.remove(&first) {
            // A member of an expanded stack collapses it too.
            let key = ga
                .expanded
                .iter()
                .find(|k| ga.tile_of(k) == Some(i))
                .cloned();
            match key {
                Some(k) => {
                    ga.expanded.remove(&k);
                }
                None => {
                    ga.expanded.insert(first);
                }
            }
        }
        ga.dirty = true;
    }

    /// Alt+Enter: the facts of the targets, in the palette's sheet.
    pub(super) fn gallery_properties(&mut self) {
        let targets = self.gallery.targets();
        // Asked again for the same: it closes (the key toggles it).
        if let Some(card) = self.gallery.info.take() {
            self.redraw = true;
            if card.targets == targets {
                return;
            }
        }
        let loc = I18n::global().localizer();
        let bytes = |n: u64| loc.bytes(n, specular_i18n::ByteUnits::Decimal);
        let mut rows: Vec<(String, String)> = Vec::new();
        let (title, subtitle, path, kind);
        if let [one] = targets.as_slice() {
            let Some(&ix) = self.gallery.by_path.get(one.as_path()) else {
                return;
            };
            let e = &self.gallery.entries[ix];
            let meta = self.gallery.metas.get(one.as_path());
            title = e.name.clone();
            // What it is and how big, under the name; the rows hold the rest.
            subtitle = match e.kind {
                EntryKind::Folder => t!("gallery-prop-folder"),
                _ => {
                    let format = meta
                        .map(|m| m.format.clone())
                        .filter(|f| !f.is_empty())
                        .unwrap_or_else(|| e.ext().to_uppercase());
                    if format.is_empty() {
                        bytes(e.len)
                    } else {
                        specular_i18n::dot_join(&[format, bytes(e.len)])
                    }
                }
            };
            path = Some(one.clone());
            kind = Some(e.kind);
            rows.push((
                t!("gallery-prop-folder"),
                one.parent().map(home_relative).unwrap_or_default(),
            ));
            if let Some(m) = meta {
                if m.width > 0 {
                    rows.push((
                        t!("gallery-prop-dimensions"),
                        format!(
                            "{} × {} ({:.1} MP)",
                            loc.integer(m.width as i64),
                            loc.integer(m.height as i64),
                            m.width as f64 * m.height as f64 / 1e6
                        ),
                    ));
                }
                if let Some(c) = &m.camera {
                    rows.push((t!("gallery-prop-camera"), c.clone()));
                }
                if let Some(l) = &m.lens {
                    rows.push((t!("gallery-prop-lens"), l.clone()));
                }
            }
            if let Some(d) = model::when(e, meta.map(|m| &**m)) {
                rows.push((
                    t!("gallery-prop-date"),
                    date_text(d, specular_i18n::DateStyle::Long),
                ));
            }
            let mk = self.gallery.marks.mark(one);
            if mk.rating > 0 {
                rows.push((
                    t!("gallery-prop-rating"),
                    t!("gallery-stars", stars = i64::from(mk.rating)),
                ));
            }
            if mk.label > 0 {
                rows.push((t!("gallery-prop-label"), label_name(mk.label)));
            }
        } else if targets.is_empty() {
            return;
        } else {
            let total: u64 = targets
                .iter()
                .filter_map(|p| self.gallery.by_path.get(p.as_path()))
                .map(|&i| self.gallery.entries[i].len)
                .sum();
            title = t!("gallery-items", count = targets.len() as i64);
            subtitle = bytes(total);
            path = None;
            kind = None;
            // One folder for all of them: where they are.
            let parent = targets[0].parent();
            if targets.iter().all(|t| t.parent() == parent)
                && let Some(p) = parent
            {
                rows.push((t!("gallery-prop-folder"), home_relative(p)));
            }
        }
        let ga = &mut self.gallery;
        let tile = path
            .as_deref()
            .and_then(|p| ga.tile_of(p))
            .or_else(|| ga.focus_index());
        ga.info = Some(InfoCard {
            tile,
            targets,
            path,
            kind,
            title,
            subtitle,
            rows,
            opened: f64::NAN,
            rect: None,
            close: None,
        });
        ga.hover = None;
        self.redraw = true;
    }
}

/// A folder as people read it: under the home folder from `~` (`~/Pictures/2024`).
fn home_relative(p: &Path) -> String {
    let rest = draw::home_dir().and_then(|h| p.strip_prefix(&h).ok().map(Path::to_path_buf));
    match rest {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => p.display().to_string(),
    }
}

/// A date (seconds since 1970, local wall time) as text.
pub(super) fn date_text(secs: i64, style: specular_i18n::DateStyle) -> String {
    let (y, m, d) = model::civil_from_days(secs.div_euclid(86_400));
    I18n::global()
        .localizer()
        .date(y, m, d, style)
        .unwrap_or_else(|| format!("{y:04}-{m:02}-{d:02}"))
}

// ----- Accessibility, screenshots, cache warm-up -----------------------------------------

impl App {
    /// An assistive technology clicked or focused one of our nodes; false when not ours.
    pub(super) fn gallery_a11y_action(&mut self, target: u64, click: bool) -> bool {
        if (A11Y_SELBAR..A11Y_SELBAR + (1 << 32)).contains(&target) {
            let k = (target - A11Y_SELBAR).wrapping_sub(1) as usize;
            let button = self
                .gallery
                .sel_bar
                .as_ref()
                .and_then(|b| b.buttons.get(k))
                .cloned();
            if click && let Some((r, h, _)) = button {
                self.gallery_sel_bar(h, r);
            }
            return true;
        }
        if (A11Y_INFO..A11Y_INFO + (1 << 32)).contains(&target) {
            // Its close button; the facts take no action.
            if click && target == A11Y_INFO + 1 {
                self.gallery.info = None;
                self.redraw = true;
            }
            return true;
        }
        if self.gallery.active && self.picker_a11y_action(target, click) {
            return true;
        }
        if self.gallery.active && self.facets_a11y_action(target, click) {
            return true;
        }
        if self.gallery.active && self.bar_a11y_action(target, click) {
            return true;
        }
        if self.gallery.active && self.library_a11y_action(target, click) {
            return true;
        }
        if !self.gallery.active || target <= A11Y_BASE {
            return false;
        }
        let i = (target - A11Y_BASE - 1) as usize;
        if i >= self.gallery.tiles.len() {
            return false;
        }
        self.gallery.set_focus(i);
        if click {
            self.gallery_open(i);
        }
        self.redraw = true;
        true
    }

    /// `--show gallery…` for screenshots; false when `s` is not ours.
    pub(super) fn gallery_show(&mut self, s: &str) -> bool {
        if let Some(rest) = s.strip_prefix("library") {
            return self.library_show(rest);
        }
        let Some(rest) = s.strip_prefix("gallery") else {
            return false;
        };
        if !self.gallery.active {
            self.gallery_enter();
            // Screenshots show the grid, not the way in.
            self.gallery.transition = None;
        }
        if self.similar_show(rest)
            || self.compare_show(rest)
            || self.map_show(rest)
            || self.import_show(rest)
        {
            self.gallery.dirty = true;
            return true;
        }
        let ga = &mut self.gallery;
        match rest {
            "" => {}
            "-justified" => ga.view.mode = Mode::Justified,
            "-uncropped" => ga.view.mode = Mode::Uncropped,
            "-details" => ga.view.mode = Mode::Details,
            "-small" => ga.view.size = 96.0,
            "-large" => ga.view.size = 320.0,
            "-no-captions" => ga.show_captions = false,
            "-group-month" => ga.view.group = GalleryGroup::Month,
            "-group-folder" => ga.view.group = GalleryGroup::Folder,
            "-group-event" => ga.view.group = GalleryGroup::Event,
            "-sort-date" => ga.view.sort = SortKey::DateTaken,
            "-sort-size" => {
                ga.view.sort = SortKey::Size;
                ga.view.desc = true;
            }
            "-filter" => {
                ga.filter_open = true;
                ga.filter_focus = true;
            }
            "-facets" => ga.facets_show(true, false),
            "-facets-focus" => ga.facets_show(true, true),
            "-query-error" => {
                ga.filter_open = true;
                ga.filter.text = "rating>=3 camra:X-T5 iso:<abc".into();
            }
            // The keyboard focus on the filter bar's first chip.
            "-bar-focus" => {
                ga.filter_open = true;
                ga.go(filterbar::Stop::Chip(0));
            }
            "-query" => {
                ga.filter_open = true;
                ga.filter.text = "rating>=1 ext:png,jpg w>100".into();
            }
            "-picks" => {
                ga.filter_open = true;
                ga.filter.toggle("is:pick", &[]);
            }
            "-stacks" => ga.stacks_on = true,
            "-bench" => ga.bench = Some(bench::Bench::default()),
            "-scroll-bench" => ga.sbench = Some(scrollbench::ScrollBench::from_env()),
            "-info" => ga.info_overlay = true,
            "-recursive" => {
                ga.depth = 2;
                if let Some(d) = ga.dir.clone() {
                    self.gallery_relist(d);
                }
            }
            "-select" => {
                // A few tiles selected and marked, to show selection and badges.
                let paths: Vec<PathBuf> = ga.tile_paths().take(7).cloned().collect();
                for (k, p) in paths.iter().enumerate() {
                    if k % 2 == 1 {
                        ga.selected.insert(p.clone());
                    }
                    ga.marks.marks.insert(
                        p.clone(),
                        Mark {
                            rating: (k % 6) as u8,
                            label: (k % 5) as u8,
                            flag: [0, 1, -1][k % 3],
                        },
                    );
                }
                if let Some(i) = (!paths.is_empty()).then_some(1.min(paths.len() - 1)) {
                    ga.set_focus(i);
                }
            }
            "-scrolled" => {
                let h = ga.layout.height;
                ga.scroll_target = (h * 0.4).max(0.0);
                ga.scroll = ga.scroll_target;
                ga.scrolled_at = 0.0;
            }
            "-view-menu" => self.gallery_menu_view(None),
            "-more-menu" => self.gallery_menu_more(),
            "-context" => {
                if self.gallery.focus_index().is_none() && !self.gallery.tiles.is_empty() {
                    self.gallery.set_focus(0);
                }
                self.gallery_context_menu(None);
            }
            "-compare" => {
                let p: Vec<PathBuf> = ga
                    .tile_paths()
                    .filter(|p| {
                        ga.by_path
                            .get(p.as_path())
                            .is_some_and(|&i| ga.entries[i].kind == EntryKind::Image)
                    })
                    .take(3)
                    .cloned()
                    .collect();
                ga.selected = p.iter().cloned().collect();
                self.compare_start(p, compare::CmpMode::Side);
            }
            "-quick" => {
                let p = ga.focus.clone().or_else(|| {
                    ga.tile_paths()
                        .find(|p| {
                            ga.by_path
                                .get(p.as_path())
                                .is_some_and(|&i| ga.entries[i].kind == EntryKind::Image)
                        })
                        .cloned()
                });
                if let Some(p) = p {
                    ga.focus = Some(p.clone());
                    ga.quick = Some(p.clone());
                    self.gallery_viewer_open(&p, Priority::Visible);
                }
            }
            "-properties" => {
                if self.gallery.focus_index().is_none() && !self.gallery.tiles.is_empty() {
                    self.gallery.set_focus(0);
                }
                self.gallery_properties();
            }
            _ => warn!(target: UI, "--show {s}: unknown"),
        }
        self.gallery.dirty = true;
        true
    }

    /// For screenshots, before `--show gallery…` is applied: enters the gallery (without the
    /// way in) and tells whether its listing is in and laid out, so the shows that pick
    /// tiles (compare, select, quick look…) find them.
    pub(super) fn gallery_shot_listed(&mut self) -> bool {
        if !self.gallery.active {
            self.gallery_enter();
            self.gallery.transition = None;
            return false;
        }
        let ga = &self.gallery;
        !ga.listing && !ga.dirty && ga.resort_at.is_none()
    }

    /// For screenshots: the listing is done and every visible tile is sharp.
    pub(super) fn gallery_settled(&self) -> bool {
        let ga = &self.gallery;
        if !ga.active {
            return true;
        }
        if ga.listing
            || ga.dirty
            || ga.transition.is_some()
            || !ga.uploads.is_empty()
            || ga.bench.as_ref().is_some_and(|b| !b.done)
            || ga.bench_busy()
            || (ga.library.on && ga.library.opening())
        {
            return false;
        }
        let Some(atlas) = &ga.atlas else {
            return ga.tiles.is_empty();
        };
        let class = ga.class();
        let range = ga.layout.items_in(ga.scroll, ga.scroll + ga.grid_h());
        let tiles_ok = range.into_iter().all(|i| {
            let Some(e) = ga.entry_of(i) else { return true };
            match e.kind {
                EntryKind::Image => {
                    ga.failed.contains_key(&e.path) || atlas.is_sharp(&e.path, class)
                }
                EntryKind::Folder | EntryKind::Archive => {
                    !ga.cover_pending.contains(&e.path)
                        && ga.covers.get(&e.path).is_none_or(|_| {
                            let c = ga.shown_covers(&e.path);
                            let class = ga.cover_class(c.len());
                            c.iter().all(|p| atlas.is_sharp(p, class))
                        })
                }
                _ => true,
            }
        });
        // Compare shows its images at the largest class.
        let compare_ok = ga
            .compare
            .iter()
            .flatten()
            .all(|p| ga.failed.contains_key(p) || atlas.is_sharp(p, 512));
        let quick_ok = ga.quick.as_ref().is_none_or(|p| {
            self.by_path
                .get(p)
                .and_then(|id| self.images.get(id))
                .is_some_and(|img| img.gpu_rank.is_some() || img.error.is_some())
        });
        let similar_ok = ga.similar.settled()
            && ga
                .similar
                .wanted
                .iter()
                .all(|(p, c)| ga.failed.contains_key(p) || atlas.is_sharp(p, *c));
        // Facet counts and event groups are only right once every image's metadata is in.
        let counts = ga.facets.open || ga.picker_counts();
        let metas_ok = !(counts || ga.view.group == GalleryGroup::Event)
            || ((!counts || ga.facets.current(ga.metas.len()))
                // The listed ones: the scan asks for those (see `feed_wishes`).
                && ga.tiles.iter().map(|t| &ga.entries[t.entry]).all(|e| {
                    e.kind != EntryKind::Image
                        || ga.metas.contains_key(e.path.as_path())
                        || ga.failed.contains_key(&e.path)
                }));
        tiles_ok
            && compare_ok
            && quick_ok
            && similar_ok
            && metas_ok
            && self.compare_channel_settled()
    }
}

impl Gallery {
    /// The engine runs (the gallery was entered once).
    pub(super) fn started(&self) -> bool {
        self.engine.is_some()
    }

    /// Records one frame's CPU time for the scroll measurements.
    pub(super) fn note_frame(&mut self, d: Duration) {
        if !self.active {
            return;
        }
        if self.stats.frames.len() >= 4096 {
            self.stats.frames.drain(..2048);
        }
        self.stats.frames.push(d.as_secs_f32() * 1000.0);
        if let Some(b) = self.sbench.as_mut() {
            b.prof.gallery += d.as_secs_f32() * 1000.0;
        }
    }

    /// The grid as a list box with one option per visible tile (focus follows the tile).
    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, scale: f64) {
        use accesskit::{Node, NodeId, Role};
        let ga = self;
        if !ga.active
            || ga.import_a11y(tree, scale)
            || ga.people_a11y(tree, scale)
            || ga.map_a11y(tree, scale)
            || ga.similar_a11y(tree, scale)
        {
            return;
        }
        let bounds = |r: Rect| accesskit::Rect {
            x0: f64::from(r.x) * scale,
            y0: f64::from(r.y) * scale,
            x1: f64::from(r.x + r.w) * scale,
            y1: f64::from(r.y + r.h) * scale,
        };
        let list_id = NodeId(A11Y_BASE);
        let mut list = Node::new(Role::ListBox);
        list.set_label(match &ga.dir {
            _ if ga.library.on => t!("library-a11y-list", count = ga.tiles.len() as i64),
            Some(d) => t!(
                "gallery-a11y-list",
                folder = file_name(d),
                count = ga.tiles.len() as i64
            ),
            None => t!("gallery-view-menu"),
        });
        list.set_bounds(bounds(ga.flip(Rect::new(
            0.0,
            ga.grid_top(),
            ga.grid_w(),
            ga.grid_h(),
        ))));
        if ga.selected.len() > 1 {
            list.set_multiselectable();
        }
        let range = ga.layout.items_in(ga.scroll, ga.scroll + ga.grid_h());
        let focus = ga.focus_index();
        let mut ids = Vec::new();
        let mut nodes = Vec::new();
        // The focused tile is listed even when scrolled away, so focus always has a node.
        let extra = focus.filter(|f| !range.contains(f));
        for i in range.chain(extra) {
            let Some(e) = ga.entry_of(i) else { continue };
            let id = NodeId(A11Y_BASE + 1 + i as u64);
            let mut n = Node::new(Role::ListBoxOption);
            n.set_label(self.tile_label(i, e));
            if let Some(r) = ga.tile_rect(i) {
                n.set_bounds(bounds(r));
            }
            n.set_selected(ga.selected.contains(&e.path) || focus == Some(i));
            n.add_action(accesskit::Action::Click);
            n.add_action(accesskit::Action::Focus);
            ids.push(id);
            nodes.push((id, n));
        }
        list.set_children(ids);
        // The query's first error is announced as it appears.
        let error = ga
            .filter_open
            .then(|| ga.filter.query().errors.first().map(facets::query_error))
            .flatten()
            .map(|text| {
                let mut n = Node::new(Role::Status);
                n.set_live(accesskit::Live::Polite);
                n.set_label(text);
                (NodeId(A11Y_BASE - 2), n)
            });
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1))
            && let Some((id, _)) = &error
        {
            root.push_child(*id);
        }
        tree.nodes.extend(error);
        // The filter bar reads before the grid.
        ga.bar_a11y(tree, &bounds);
        let info_focus = ga.info_a11y(tree, &bounds);
        ga.sel_bar_a11y(tree, &bounds);
        ga.picker_a11y(tree, &bounds);
        ga.compare_a11y(tree, &bounds);
        let facets_focus = ga.facets_a11y(tree, &bounds) && ga.facets.focus.is_some();
        let library_focus = ga.library_a11y(tree, &bounds);
        ga.timeline_a11y(tree, &bounds);
        // The channel view in quick look or compare is announced as it changes, as the
        // viewer's is (its own node says the normal view while the gallery shows).
        let mut channel = Node::new(Role::Status);
        channel.set_live(accesskit::Live::Polite);
        channel.set_label(specular_widgets::channel_label(ga.channel_shown()));
        let channel = (NodeId(A11Y_BASE - 3), channel);
        // Going up a folder is announced (the list's new label alone often is not).
        let said = ga.announce.as_ref().map(|text| {
            let mut n = Node::new(Role::Status);
            n.set_live(accesskit::Live::Polite);
            n.set_label(text.as_str());
            (NodeId(A11Y_BASE - 1), n)
        });
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
            root.push_child(list_id);
            root.push_child(channel.0);
            if let Some((id, _)) = &said {
                root.push_child(*id);
            }
        }
        tree.nodes.push(channel);
        tree.nodes.extend(said);
        tree.nodes.push((list_id, list));
        tree.nodes.extend(nodes);
        if let Some(id) = info_focus {
            tree.focus = id;
        } else if tree.focus == NodeId(1) && !facets_focus && !library_focus {
            tree.focus = focus.map_or(list_id, |i| NodeId(A11Y_BASE + 1 + i as u64));
        }
    }

    /// The selection bar as a toolbar of buttons, named as their tooltips are.
    fn sel_bar_a11y(
        &self,
        tree: &mut accesskit::TreeUpdate,
        bounds: &dyn Fn(Rect) -> accesskit::Rect,
    ) {
        use accesskit::{Action, Node, NodeId, Role};
        let Some(bar) = self.sel_bar.as_ref() else {
            return;
        };
        let mut toolbar = Node::new(Role::Toolbar);
        toolbar.set_label(t!(
            "gallery-selected",
            count = self.selected.len() as i64,
            total = self.tiles.len() as i64
        ));
        toolbar.set_bounds(bounds(bar.rect));
        let mut ids = Vec::new();
        for (k, (r, _, name)) in bar.buttons.iter().enumerate() {
            let id = NodeId(A11Y_SELBAR + 1 + k as u64);
            let mut n = Node::new(Role::Button);
            n.set_label(name.as_str());
            n.set_bounds(bounds(*r));
            n.add_action(Action::Click);
            n.add_action(Action::Focus);
            ids.push(id);
            tree.nodes.push((id, n));
        }
        toolbar.set_children(ids);
        tree.nodes.push((NodeId(A11Y_SELBAR), toolbar));
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
            root.push_child(NodeId(A11Y_SELBAR));
        }
    }

    /// The Properties card as a dialog: its title, the facts as labels ("Folder: ~/Pictures"),
    /// and a close button, which takes the focus while it is open (returned).
    fn info_a11y(
        &self,
        tree: &mut accesskit::TreeUpdate,
        bounds: &dyn Fn(Rect) -> accesskit::Rect,
    ) -> Option<accesskit::NodeId> {
        use accesskit::{Action, Node, NodeId, Role};
        let card = self.info.as_ref()?;
        let mut dialog = Node::new(Role::Dialog);
        dialog.set_label(format!("{}, {}", t!("gallery-properties"), card.title));
        if let Some(r) = card.rect {
            dialog.set_bounds(bounds(r));
        }
        let mut ids = Vec::new();
        let close_id = NodeId(A11Y_INFO + 1);
        let mut close = Node::new(Role::Button);
        close.set_label(t!("app-close"));
        if let Some(r) = card.close {
            close.set_bounds(bounds(r));
        }
        close.add_action(Action::Click);
        close.add_action(Action::Focus);
        ids.push(close_id);
        tree.nodes.push((close_id, close));
        let facts = std::iter::once(card.subtitle.clone())
            .chain(card.rows.iter().map(|(k, v)| format!("{k}: {v}")));
        for (k, fact) in facts.enumerate() {
            let id = NodeId(A11Y_INFO + 2 + k as u64);
            let mut n = Node::new(Role::Label);
            n.set_value(fact);
            ids.push(id);
            tree.nodes.push((id, n));
        }
        dialog.set_children(ids);
        let dialog_id = NodeId(A11Y_INFO);
        tree.nodes.push((dialog_id, dialog));
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
            root.push_child(dialog_id);
        }
        Some(close_id)
    }

    /// "IMG_4862.HEIC, 4032 by 3024, 12 March 2024, 3 stars".
    pub(super) fn tile_label(&self, i: usize, e: &Entry) -> String {
        let ga = self;
        let loc = I18n::global().localizer();
        let mut parts = vec![e.name.clone()];
        match e.kind {
            EntryKind::Folder => parts.push(t!("gallery-a11y-folder")),
            EntryKind::Archive => parts.push(t!("gallery-a11y-archive")),
            _ => {}
        }
        let meta = ga.metas.get(e.path.as_path());
        let dims = meta
            .map(|m| (m.width, m.height))
            .filter(|d| d.0 > 0)
            .or(e.dims);
        if let Some((w, h)) = dims {
            parts.push(t!(
                "gallery-a11y-dims",
                width = loc.integer(w as i64),
                height = loc.integer(h as i64)
            ));
        }
        if e.kind == EntryKind::Image
            && let Some(d) = model::when(e, meta.map(|m| &**m))
        {
            parts.push(date_text(d, specular_i18n::DateStyle::Long));
        }
        let m = ga.marks.mark(&e.path);
        if m.rating > 0 {
            parts.push(t!("gallery-stars", stars = i64::from(m.rating)));
        }
        if m.label > 0 {
            parts.push(label_name(m.label));
        }
        match m.flag {
            1 => parts.push(t!("gallery-pick")),
            -1 => parts.push(t!("gallery-reject")),
            _ => {}
        }
        parts.extend(ga.develop_a11y(e));
        let members = ga.tiles.get(i).map_or(0, |t| t.members.len());
        if members > 1 {
            parts.push(t!("gallery-a11y-stack", count = members as i64));
        }
        parts.join(", ")
    }
}

/// `specular --thumbnail-cache DIR [--depth N]`: fills the thumbnail cache for a folder
/// without opening a window (a nightly job, or before a slideshow on a slow disk).
pub(crate) fn warm_cache(dir: &Path, depth: u32, settings: &specular_settings::Settings) -> i32 {
    let g = &settings.gallery;
    let Some(cache_dir) = g.cache_location.clone().or_else(default_cache_dir) else {
        eprintln!("specular: no thumbnail cache folder");
        return 1;
    };
    let cache = Arc::new(DiskCache::new(
        cache_dir.clone(),
        (g.cache_size.max(64) as u64) << 20,
    ));
    let registry = Arc::new(specular_decode::Registry::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let pool = Arc::new(specular_core::JobPool::new(threads, "thumb-warm"));
    let mut cfg = EngineConfig::default();
    cfg.generate.prefer_embedded = g.prefer_embedded;
    cfg.digester = Some(specular_similar::scan::digester());
    let (wtx, wrx) = crossbeam_channel::bounded::<()>(1);
    // Archive tiles' pages as the gallery reads them (and their list kept in the cache).
    let is_image = registry.clone();
    let archives = Arc::new(
        ArchiveOpener::new(Arc::new(move |p: &Path| is_image.supports_extension(p)))
            .with_cache(Some(cache.clone())),
    );
    let engine = Engine::new(
        cfg,
        registry.clone(),
        pool,
        archives.clone(),
        Some(cache),
        Arc::new(move || {
            let _ = wtx.try_send(());
        }),
    );
    let opts = ListOptions {
        show_hidden: g.show_hidden,
        show_other: false,
        show_archives: g.show_archives,
        show_folders: false,
        depth,
        ..ListOptions::default()
    };
    let classify = move |p: &Path| {
        if crate::book::is_archive_path(p) {
            EntryKind::Archive
        } else if registry.supports_file(p) {
            EntryKind::Image
        } else {
            EntryKind::Other
        }
    };
    let (mut files, mut books) = (Vec::new(), Vec::new());
    let token = CancelToken::new();
    if let Err(e) = list_dir(dir, opts, &classify, &token, &mut |chunk| {
        for e in chunk {
            match e.kind {
                EntryKind::Image => files.push(e.path),
                EntryKind::Archive => books.push(e.path),
                _ => {}
            }
        }
    }) {
        eprintln!("specular: {}: {e}", dir.display());
        return 1;
    }
    let started = Instant::now();
    if books.is_empty() {
        println!("{} image(s) → {}", files.len(), cache_dir.display());
    } else {
        println!(
            "{} image(s), {} archive(s) → {}",
            files.len(),
            books.len(),
            cache_dir.display()
        );
    }
    // The classes the gallery asks for at its default size and on HiDPI screens.
    let side = g.thumbnail_size as f32;
    let classes = |f: f32| {
        [
            specular_thumbs::class_for(side * f),
            specular_thumbs::class_for(side * f * 2.0),
        ]
    };
    let mut failed = 0usize;
    // Asks for `reqs` and waits for them.
    let mut make = |reqs: Vec<Request>| {
        let want = reqs.len();
        engine.want(reqs, want);
        let mut got = 0usize;
        while got < want {
            let _ = wrx.recv_timeout(Duration::from_millis(200));
            for o in engine.take_ready() {
                got += 1;
                if let Outcome::Failed { path, error, .. } = o {
                    failed += 1;
                    eprintln!("{}: {error}", path.display());
                }
            }
            if engine.idle() && engine.stats().pending == 0 {
                break;
            }
        }
        engine.forget_all();
    };
    let mut done = 0usize;
    for chunk in files.chunks(256) {
        let mut reqs = Vec::new();
        for c in classes(1.0) {
            reqs.extend(chunk.iter().map(|p| Request {
                path: p.clone(),
                class: c,
            }));
        }
        make(reqs);
        done += chunk.len();
        println!("{done}/{}", files.len());
    }
    // As many archives at a time as stay open, so each is opened once.
    let mut done = 0usize;
    for chunk in books.chunks(crate::archive_thumbs::KEEP) {
        let mut reqs = Vec::new();
        for a in chunk {
            let covers = archives.first_pages(a, crate::archive_thumbs::COVERS);
            // As `cover_class`: a quarter of the tile each with four pages.
            let f = if covers.len() >= 4 { 0.5 } else { 1.0 };
            for c in classes(f) {
                reqs.extend(covers.iter().map(|p| Request {
                    path: p.clone(),
                    class: c,
                }));
            }
        }
        make(reqs);
        done += chunk.len();
        if done % 64 < chunk.len() || done == books.len() {
            println!("{done}/{} archives", books.len());
        }
    }
    let s = engine.stats();
    println!(
        "done in {:.1} s: {} generated, {} already cached, {failed} failed",
        started.elapsed().as_secs_f64(),
        s.generated,
        s.from_cache
    );
    i32::from(failed > 0)
}
