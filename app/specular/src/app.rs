//! The viewer: window and event loop, folder navigation with prefetch and caches, the
//! detail-tile scheduler, rendering, the chrome, settings, and the benchmark and screenshot
//! drivers.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use specular_core::{CancelToken, HardwareProfile, JobHandle, Orientation, Priority, Raster};
use specular_decode::{ImageInfo, Session};
use specular_gfx::{
    Backdrop, Compositor, DrawList, Frame, Gpu, Text, UiRenderer, WindowSurface, wgpu, winit,
};
use specular_i18n::{I18n, t};
use specular_input::{Action, ActionState, ImageState, Key, KeyCombo, Keymap, Mods, NamedKey};
use specular_inspect::ChannelView;
use specular_metadata::MetadataReport;
use specular_render::{GpuImageCache, ImageRenderer, PxRect, RenderOptions, View, ZoomMode, tiles};
use specular_settings::{
    Background, DoubleClick, HdrOutput, HighZoomSmoothing, HoldAcceleration, InfoDetail,
    InitialZoom, Origin, Settings, SettingsStore, SystemOnOff, Theme, Transparency, WheelBehaviour,
};
use specular_theme::{CHECKER_CELL, Color, Palette};
use specular_widgets::{Button, Chrome, Command, HudStats, Input, Modal, Overlays, ViewerState};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Fullscreen, Window, WindowId};

use crate::Args;
use crate::open_report;
use frame_gate::{Cause, FrameCauses};

#[cfg(test)]
mod a11y_audit;
mod ai;
mod annotations;
mod colour;
mod cull;
mod decoders;
mod develop;
mod devices;
mod gallery;
mod path_index;
mod side_panel;
#[cfg(test)]
mod units_tests;
pub(crate) use diagnostics::install_report_hooks;
pub(crate) use gallery::warm_cache;
pub(crate) use geo::GeoOverlay;
pub(crate) use series::ScanResult;
pub(crate) use studies::StudiesUpdate;
mod address;
mod annot_export;
mod anon_export;
mod audit;
pub(crate) mod cad;
mod cast;
mod chart;
mod codes;
mod community;
mod contents;
mod convert;
mod cursor;
mod diagnostics;
mod dicomweb;
mod events;
mod export_recipes;
#[cfg(test)]
mod faces_shots;
mod faces_view;
mod figure;
pub(crate) mod file_kind;
mod fileops;
mod font_view;
mod frame_gate;
mod geo;
mod gesture;
mod gpu_reset;
mod hooks;
mod info_loudness;
mod insights;
mod inspect;
mod integration;
mod items;
mod keys;
mod keywords;
mod label_volume;
mod linked;
mod local_copies;
mod loupe_bench;
mod map;
mod markup;
mod measure_tool;
mod medical;
mod mesh;
mod meta_edit;
mod mpr;
mod ocio;
mod ocr;
mod omero;
mod open_dialog;
mod pages;
mod password;
mod pathology;
pub(crate) mod performance;
mod playback;
mod pointer;
mod present;
mod print;
pub(crate) mod privacy;
mod profile;
mod raw_volume;
#[cfg(debug_assertions)]
mod test_hooks;
mod tether;
pub(crate) use file_kind::startup as file_kind_startup;
mod reading;
mod recipes;
mod relight;
mod remote;
mod renderer_info;
mod science;
mod script;
mod setting_fields;
mod sound_page;
mod text_view;
mod ui_guard;
pub(crate) use science::{area_text, quantity_text};
mod net;
mod oct;
mod outdev;
mod player;
mod queue;
mod quiz;
mod report;
mod series;
mod sheet_compare;
mod slideshow;
mod sound_fx;
mod stream;
mod studies;
mod video_export;
mod video_full;
mod web_gallery;
mod welcome;
mod work;
mod zarr;
mod zynk_send;
use crate::a11y::{A11yEvent, Accessibility};
use crate::clipboard::{self, Clipboard};
use crate::display_cache::DisplayCache;
use crate::loader::{
    self, Copied, Event, ImageId, ListOptions, Loader, OverviewKind, Unfinished, needed_scale,
};
use specular_log::{
    CACHE, DECODE, FOLDER, GPU, INPUT, PREFETCH, RENDER, SETTINGS, STARTUP, TILES, VIEW, debug,
    info, trace,
};

/// Tiles per region decode.
const TILE_BATCH: usize = 16;
/// Peak assumed for an HDR (scRGB) swapchain whose display reports no headroom (Linux,
/// Windows without luminance data), in units of SDR white: brighter content is tone-mapped.
const HDR_FALLBACK_HEADROOM: f32 = 2.0;
/// Region decodes in flight at once.
const MAX_TILE_JOBS: usize = 8;
/// Images kept in RAM (overview raster) beyond the prefetch window.
const RAM_SLACK: usize = 6;
const DOUBLE_CLICK_S: f64 = 0.4;
/// A loupe key let go sooner than this (seconds) was a tap: the loupe stays up.
const LOUPE_TAP: f64 = 0.3;
/// Loupe radius and navigator box, in logical pixels.
const LOUPE_RADIUS: f32 = 130.0;
/// The moving loupe's tiles are asked for where it will be this much later.
const LOUPE_AHEAD: Duration = Duration::from_millis(200);
/// A pointer still for this long has no velocity.
const LOUPE_STILL: Duration = Duration::from_millis(80);
const NAVIGATOR_BOX: (f32, f32) = (220.0, 160.0);
/// Thinnest navigator side: a panorama's navigator grows past the box (up to the window's
/// width or height) rather than becoming a sliver.
const NAVIGATOR_MIN_SIDE: f32 = 72.0;
const OVERLAY_MARGIN: f32 = 16.0;

/// Whether the image is an office document that stores no preview: it has properties but no
/// pixels, and the viewer says so instead of decoding.
fn no_preview(img: &Img) -> bool {
    img.session
        .as_ref()
        .and_then(|s| s.document())
        .is_some_and(|d| d.preview.is_none())
}

/// One image of the folder that has been touched.
struct Img {
    path: PathBuf,
    token: CancelToken,
    /// An open job is in flight (no session yet).
    opening: bool,
    plan_viewport: Option<(u32, u32)>,
    plan_prefetch: bool,
    session: Option<Arc<dyn Session>>,
    info: Option<ImageInfo>,
    lut: Option<Arc<specular_render::icc::SourceLut>>,
    /// Gain map (Apple, ISO 21496-1, Ultra HDR), applied on the GPU for the display's
    /// headroom.
    gain_map: Option<Arc<specular_core::GainMap>>,
    /// Best whole-image raster so far.
    overview: Option<(OverviewKind, Arc<Raster>)>,
    /// Whole-image scales requested (in flight or done) for the current token.
    levels: Vec<u32>,
    /// Rank of the overview on the GPU.
    gpu_rank: Option<u32>,
    error: Option<String>,
    truncated: bool,
    metadata: Option<Arc<MetadataReport>>,
    metadata_lang: Option<String>,
    /// Its motion clip, when it is a Motion Photo or Live Photo (from the metadata job).
    motion: Option<Arc<specular_video::MotionInfo>>,
    /// Orientation including the user's rotations.
    orient: Option<Orientation>,
    /// When it became the current image (HUD open time).
    shown_at: Option<Instant>,
    open_ms: Option<f64>,
    /// When a whole level (any size) was first on screen (ms since `shown_at`).
    whole_ms: Option<f64>,
    /// Load state for the log and the stall watchdog: requested → header → … → sharp.
    state: String,
    /// Stage times for the open report (DESIGN.md §12.7).
    track: open_report::Track,
    /// Page of a multi-image file (ICNS size, texture image, TIFF page) and how many it has
    /// (1 until known; see `pages.rs`).
    page: u32,
    pages: u32,
    page_label: Option<specular_decode::PageLabel>,
    /// The session of the page shown before, to open this page from without reading the
    /// file again (taken by the open).
    page_base: Option<(ImageId, Arc<dyn Session>)>,
    /// The image whose file this page belongs to, when it is another page of it (kept
    /// across page switches; `pages.rs`).
    file_id: Option<ImageId>,
    /// A page switched to: the old page stays on screen until this one has pixels.
    page_hold: bool,
}

impl Img {
    fn new(path: PathBuf) -> Img {
        Img {
            path,
            token: CancelToken::new(),
            opening: false,
            plan_viewport: None,
            plan_prefetch: false,
            session: None,
            info: None,
            lut: None,
            gain_map: None,
            overview: None,
            levels: Vec::new(),
            gpu_rank: None,
            error: None,
            truncated: false,
            metadata: None,
            metadata_lang: None,
            motion: None,
            orient: None,
            shown_at: None,
            open_ms: None,
            whole_ms: None,
            state: "new".to_owned(),
            track: open_report::Track::default(),
            page: 0,
            pages: 1,
            page_label: None,
            page_base: None,
            file_id: None,
            page_hold: false,
        }
    }

    fn rank(&self) -> u32 {
        self.overview.as_ref().map_or(u32::MAX, |o| o.0.rank())
    }

    fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// The open job that ran with `token` has the header. Only a job holding the image's
    /// live token has its first levels (`plan`) in flight: a job cancelled meanwhile (the
    /// image left the prefetch window) stops right after sending the header, and one with an
    /// older token was superseded. Recording their plan would mark levels as requested that
    /// nobody decodes, and coming back to the image would show a spinner forever.
    /// Returns whether the job was live.
    fn opened(&mut self, token: &CancelToken, plan: Vec<u32>) -> bool {
        let mine = self.token.same(token);
        let live = mine && !token.is_cancelled();
        if live {
            self.levels = plan;
        }
        if mine {
            self.opening = false;
            if !live {
                self.levels.clear();
            }
        }
        live
    }

    /// A job that ran with `token` ended without a result: its work is no longer in flight.
    /// Returns false when the job was superseded (the bookkeeping belongs to a newer token).
    fn unfinished(&mut self, token: &CancelToken, job: Unfinished) -> bool {
        if !self.token.same(token) {
            return false;
        }
        match job {
            Unfinished::Open => self.opening = false,
            Unfinished::Level(s) => self.levels.retain(|&l| l != s),
        }
        true
    }
}

/// Name, display size, loading, error text, truncated, metadata of the current image.
type StateParts = (
    Option<String>,
    Option<(u32, u32)>,
    bool,
    Option<String>,
    bool,
    Option<Arc<MetadataReport>>,
);

struct TileJob {
    id: ImageId,
    level: u8,
    tiles: Vec<(u32, u32)>,
    handle: JobHandle,
    /// Estimated peak memory of the decode.
    mem: u64,
}

struct Gfx {
    window: Arc<Window>,
    gpu: Gpu,
    surface: WindowSurface,
    comp: Compositor,
    ui: UiRenderer,
    renderer: ImageRenderer,
    text: Text,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SceneKey {
    id: Option<ImageId>,
    image_rev: u64,
    view_rev: u64,
    size: (u32, u32),
    canvas: [u32; 4],
    hq: bool,
    grid: bool,
    /// Detail tiles resident at the levels the scene reads.
    tiles: usize,
    /// Navigator on. The loupe is not part of the scene: it is drawn over its copy each
    /// frame, so moving it redraws only the disc.
    navigator: bool,
    /// Inspection overlay and pixel loupe (`inspect.rs`).
    inspect: inspect::InspectScene,
    /// Channel view and the backdrop colours (bits) it draws over.
    channel: (ChannelView, [u32; 8]),
    /// False colour, channel colours and HiLo (`inspect::apply_display_aids`).
    aids: (
        Option<specular_inspect::ColourMap>,
        specular_inspect::ChannelColours,
        Option<specular_render::HiLo>,
    ),
    /// Stain separation (`pathology.rs`).
    stain: Option<specular_render::StainView>,
    /// The OCIO table drawn with ([`specular_render::OcioLut`]).
    ocio: Option<u64>,
    /// Multiplanar reformatting's planes (`mpr.rs`; 0 when closed).
    mpr: u64,
    /// The segmentations over a medical image (`medical/analysis.rs`): revision, frame.
    medical: (u64, u32),
    /// A drawing drawn on the GPU (`cad.rs`).
    cad: Option<cad::CadFrame>,
    /// A map's layers drawn on the GPU (`map.rs`).
    map: Option<map::MapFrame>,
    /// The 3D model's view (`mesh.rs`; 0 when none is shown).
    mesh: u64,
    /// An OCT volume's localiser (`oct.rs`): its image, texture revision and rectangle (bits).
    oct: Option<(ImageId, u64, [u32; 4])>,
}

#[derive(Debug, PartialEq)]
enum BenchPhase {
    Sharp,
    Region,
    Prefetch,
    Next,
    /// `--bench-hold`: → held through the folder.
    Hold,
    /// `--bench-pan`: programmatic pan and zoom.
    Pan,
    /// `--bench-loupe`: the loupe follows a simulated pointer.
    Loupe,
}

/// Simulated key repeat: the first repeat after 500 ms, then every 30 ms (GNOME's
/// defaults; Windows and macOS repeat at similar rates).
const REPEAT_DELAY: Duration = Duration::from_millis(500);
const REPEAT_INTERVAL: Duration = Duration::from_millis(30);

/// `--bench-hold` measurements.
struct HoldBench {
    target: usize,
    /// Key-repeat delay and interval (`--bench-repeat`).
    repeat: (Duration, Duration),
    pressed: Instant,
    next_tick: Instant,
    steps: usize,
    /// Image navigated to and when; cleared once its pixels are on screen.
    pending: Option<(ImageId, Instant)>,
    /// Navigation to pixels on screen, per image that got shown.
    latencies: Vec<f64>,
    /// Images passed before their preview was ready.
    skipped: usize,
    last_present: Option<Instant>,
    /// Intervals between presented frames while held.
    gaps: Vec<f64>,
    /// Gaps over 50 ms: (ms since press, gap, surface acquire, frame CPU), to tell a slow frame
    /// from a compositor or GPU wait.
    stalls: Vec<[f64; 4]>,
    /// Longest main-thread frame (build + submit).
    frame_max_ms: f64,
    /// Main-thread time of every frame while held (build, acquire, submit, present).
    frame_ms: Vec<f64>,
    /// The same without the surface acquire (which waits for the GPU or the compositor).
    work_ms: Vec<f64>,
    /// Frames presented while held that had no image pixels (blank or spinner).
    frames_without_pixels: usize,
    /// Most video memory (images and tiles) seen while held.
    vram_max: u64,
    /// Main-thread work per key repeat (navigation and scheduling).
    step_max_ms: f64,
    released: Option<Instant>,
    settle_ms: Option<f64>,
}

/// `--bench-pan` measurements: the view follows a fixed path (`pan_view_at`), one sample per
/// presented frame.
struct PanBench {
    secs: f64,
    started: Instant,
    last_present: Option<Instant>,
    /// Per frame: [s since start, main-thread ms, ms since the previous present, acquire ms,
    /// tiles missing, scale].
    frames: Vec<[f64; 6]>,
}

/// `SPECULAR_BENCH_STARTUP=1`: when the first frame, the image's first pixels and the first
/// frame with text in the real fonts were presented (ms since `main`).
#[derive(Default)]
struct StartupBench {
    first: Option<f64>,
    pixels: Option<f64>,
    text: Option<f64>,
}

struct Bench {
    phase: BenchPhase,
    first_pixels_ms: Option<f64>,
    sharp_ms: Option<f64>,
    sharp_scale: u32,
    overview_ms: Vec<(String, f64)>,
    regions: Vec<f64>,
    region_i: usize,
    region_t: Instant,
    region_tiles: Vec<usize>,
    phase_t: Instant,
    neighbour: Option<usize>,
    prefetch_wait_ms: f64,
    next_ms: Option<f64>,
    next_first_ms: Option<f64>,
    next_file: String,
    started: Instant,
    /// Absolute times (ms since process start) of startup and first-image stages.
    stages: Vec<(String, f64)>,
    gpu_init_ms: f64,
    upload_ms: Vec<(String, f64)>,
    hold: Option<HoldBench>,
    pan: Option<PanBench>,
    loupe: Option<loupe_bench::LoupeBench>,
}

impl Bench {
    fn stage(&mut self, name: &str, at: f64) {
        if !self.stages.iter().any(|(n, _)| n == name) {
            self.stages.push((name.to_owned(), at));
        }
    }
}

const REGIONS: [(f32, f32); 7] = [
    (0.5, 0.5),
    (0.25, 0.3),
    (0.75, 0.7),
    (0.4, 0.8),
    (0.6, 0.2),
    (0.12, 0.55),
    (0.88, 0.45),
];

struct Shot {
    path: PathBuf,
    applied: bool,
    ready_since: Option<Instant>,
    started: Instant,
}

pub struct App {
    args: Args,
    hw: HardwareProfile,
    loader: Loader,
    events: Receiver<Event>,
    gpu_join: Option<JoinHandle<Result<Gpu, String>>>,
    text_join: Option<JoinHandle<Text>>,
    gfx: Option<Gfx>,
    /// The presentation window on another display (a slideshow, or the viewer there).
    presenter: Option<present::Presenter>,
    monitors: present::Monitors,
    /// Screen readers: the adapter, and its requests waiting for the event loop.
    a11y: Option<Accessibility>,
    a11y_events: Vec<A11yEvent>,
    dl: DrawList,
    chrome: Chrome,
    store: SettingsStore,
    settings: Arc<Settings>,
    settings_gen: u64,
    i18n_gen: u64,
    keymap: Keymap,
    clipboard: Clipboard,

    entries: Vec<PathBuf>,
    /// Positions in `entries` by path ([`App::index_of`]).
    path_index: path_index::PathIndex,
    current: Option<usize>,
    images: HashMap<ImageId, Img>,
    by_path: HashMap<PathBuf, ImageId>,
    /// Pages next to the current one, opened ahead (in `images`, not in `by_path`).
    page_ahead: Vec<ImageId>,
    next_id: ImageId,
    /// Image the view is set up for.
    shown: Option<ImageId>,
    pending_zoom: bool,
    view: View,
    gpu_images: GpuImageCache,
    tile_jobs: Vec<TileJob>,
    tiles_failed: HashSet<(ImageId, u8, u32, u32)>,
    tiles_missing: usize,
    /// Pointer velocity (physical px/s, smoothed) and when it last moved, while the loupe
    /// follows it.
    pointer_vel: Option<((f32, f32), Instant)>,
    /// Tile request counters (`--bench-loupe` reads them).
    tile_stats: loupe_bench::TileStats,
    /// Folder whose images are listed (browsing); re-listed when the sort settings change.
    folder: Option<PathBuf>,
    /// Detail level the view wants (None: the overview is sharp enough).
    detail_level: Option<u8>,
    navigator: bool,
    nav_drag: bool,
    /// Zoom-rectangle drag: start and current pointer (physical px).
    zoom_drag: Option<((f32, f32), (f32, f32))>,
    /// Screenshots: show the loupe without a held key.
    shot_loupe: bool,
    /// The loupe key was tapped: the loupe stays until the key or Back again (touchpads
    /// are often off while a key is held).
    loupe_latched: bool,

    pointer: Option<(f32, f32)>,
    /// Touchpad pinch state and the Linux gesture listener (gesture.rs).
    gestures: gesture::Gestures,
    mods: ModifiersState,
    dragging: Option<(f32, f32)>,
    last_click: Option<(f64, (f32, f32))>,
    /// The last click on a movie's picture played or paused it (a double click undoes it).
    click_toggled_play: bool,
    /// The window title last set (`update_title` sets it only when it changes).
    title_shown: String,
    /// The colour the title bar was last given, and whether it showed the title
    /// ([`Self::sync_title_bar`]).
    title_bar: Option<([f32; 3], bool)>,
    hold: Option<(Action, f64)>,
    nav_dir: i32,
    pixel_grid: bool,
    fullscreen: bool,
    /// A movie's full-screen play mode, and the layout it goes back to (app/video_full.rs).
    video_full: video_full::VideoFull,
    /// A movie's player: subtitles, sound tracks, chapters, resume, the screen kept awake.
    player: player::Player,
    /// Playing on a TV or speaker ("Play on…", app/cast.rs).
    cast: cast::CastState,
    /// The play queue and what plays next (app/queue.rs).
    queue: queue::PlayQueue,
    /// The sound chain: the equaliser's panel and gapless play (`sound_fx.rs`).
    sound_fx: sound_fx::SoundFx,
    /// The sound's output device (`outdev.rs`).
    outdev: outdev::OutDev,
    /// The file play mode last fitted (a new movie shown in it is fitted again).
    video_full_shown: Option<ImageId>,
    /// Pointer hidden (fullscreen with the chrome faded out, or a slideshow, after the
    /// pointer has rested; see `pointer`).
    cursor_hidden: bool,
    /// When the pointer last moved more than a couple of pixels (s, `now`).
    pointer_moved: f64,
    /// The pointer's shape as last set (a hand over cull mode's controls).
    cursor_icon: winit::window::CursorIcon,
    /// → or ← is held (key repeat): only previews are decoded until it is released.
    flipping: bool,
    /// What the chrome asked for while drawing (a held Previous or Next repeating), run in
    /// `about_to_wait`.
    chrome_commands: Vec<Command>,
    /// Input method enabled (a text field has focus), and where its candidate window goes.
    ime: Option<specular_gfx::Rect>,
    /// Text of the last key press, to drop an IME commit that repeats it.
    last_key_text: Option<(String, Instant)>,

    t0: Instant,
    last_frame: Instant,
    /// Last check of the display's HDR state (see `update_hdr_output`).
    hdr_checked: Option<Instant>,
    scene: Option<SceneKey>,
    /// The scene texture holds an image's pixels, drawn at this size on this canvas: it can
    /// stand in while the next image has none yet.
    scene_pixels: Option<((u32, u32), [u32; 4])>,
    blur_valid: bool,
    hud: HudStats,
    trash_undo: HashMap<u64, (PathBuf, usize)>,
    next_undo: u64,
    last_undo: Option<u64>,
    bench: Option<Bench>,
    shot: Option<Shot>,
    fatal: Option<String>,
    redraw: bool,
    /// Why the next frame is drawn, and the app gates' record (frame_gate.rs).
    causes: FrameCauses,
    /// The next `frame` call is one `about_to_wait` draws at once (asked for, not a frame
    /// callback).
    frame_asked: bool,
    /// New pixels for the current image (another image, or tiles) are ready: present them
    /// from `about_to_wait` instead of waiting for the compositor's frame callback, which
    /// would add up to one refresh interval before they could even be committed.
    present_now: bool,
    /// When the window and the GPU were ready (the open report's "starting up").
    ready_at: Option<Instant>,
    /// Prefetched images waiting for their GPU upload, done one per event-loop turn after
    /// any due frame so a burst of finished prefetches never holds up the current image.
    deferred_uploads: VecDeque<ImageId>,
    /// While events drain: current-image uploads, done once at the end so a burst of
    /// overviews (thumbnail, 1/8, 1/2 queued while the GPU came up) uploads only the best.
    upload_batch: Option<Vec<ImageId>>,
    /// Prefetch waits until the first image is sharp (DESIGN.md §12.5: the current image comes
    /// first); `schedule` is a no-op until then.
    prefetch_held: bool,
    wake_at: Option<Instant>,
    /// When `about_to_wait` last asked for a redraw that has not been drawn yet. Benchmarks and
    /// screenshots draw on their own when the compositor sends no frame callbacks (the window is
    /// hidden or the output is off), which would otherwise hold them up forever on Wayland.
    redraw_asked: Option<Instant>,
    ui_scale: f32,
    /// The diagnostics bundle being saved, the update check (diagnostics.rs).
    diag: diagnostics::Diag,
    /// keymap.toml, import and export of key bindings, a key sequence begun (keys.rs).
    keys: keys::Keys,
    exit_requested: bool,
    /// Warns when the current image stays loading too long (`performance.stall_warning`).
    watchdog: specular_log::Watchdog,
    frames: u64,
    /// `SPECULAR_BENCH_STARTUP=1`: a normal start that prints when its first frames were
    /// presented, then exits.
    startup_bench: Option<StartupBench>,
    /// The player of the current image when it is animated (`playback.rs`).
    anim: Option<crate::anim::Anim>,
    /// An animation that failed to start (shown as a still).
    anim_failed: Option<ImageId>,
    /// "Loop forever" toggled on (over the setting and the file).
    loop_forever: bool,
    /// The motion clip of this image plays in place of its still, or is being opened
    /// (`playback.rs`), and since when until its first frame is on screen.
    motion: Option<(ImageId, Option<Instant>)>,
    /// The open archive and reading mode (`reading.rs`).
    reading: reading::Reading,
    /// DICOM series mode (app/series.rs).
    series: series::SeriesMode,
    /// Structured report jumps and the waveform on the plot sheet (app/report.rs).
    report: report::ReportState,
    /// The studies browser of a patient's medium (app/studies.rs).
    studies: studies::StudiesMode,
    /// Profiles: session enables, the "Always" offers (app/profile.rs).
    profile_state: profile::ProfileState,
    /// Inspection tools: readbacks, source values (`inspect.rs`).
    inspect: inspect::InspectState,
    /// Open with, user commands, drag-out, paste (`integration.rs`).
    integration: integration::Integration,
    zynk: zynk_send::ZynkState,
    /// "Read codes": the QR codes in the picture shown (`codes.rs`).
    codes: codes::Codes,
    find_text: ocr::FindText,
    /// "Show faces" and Culling's flags for the picture shown (`faces_view.rs`).
    face_view: faces_view::FaceView,
    /// Optional decoders: the card, the Settings page, looking again on focus.
    decoders: decoders::Decoders,
    devices: devices::DevicesState,
    /// A path setting's Choose… waiting for its dialog (`setting_fields.rs`).
    setting_pick: setting_fields::PathPick,
    /// The "Why is this slow?" indicator (`insights.rs`).
    insights: insights::InsightState,
    /// The background work indicator and panel (`work.rs`).
    work: work::WorkState,
    /// What is downloading ([`specular_core::net`]).
    net: net::NetState,
    /// The thumbnail gallery (DESIGN §10).
    gallery: gallery::Gallery,
    /// Colour policy and display transform (`colour.rs`).
    colour: colour::ColourState,
    /// The slideshow (`slideshow.rs`).
    slideshow: slideshow::Slideshow,
    /// The teaching quiz, while one runs.
    quiz: quiz::QuizState,
    /// Cull mode (`cull.rs`).
    cull: cull::Cull,
    /// Develop (`develop.rs`).
    develop: develop::Develop,
    print: print::PrintState,
    /// Anonymised copies and patient disc folders (medical mode).
    anon: anon_export::AnonState,
    /// Stain separation, imported annotations (`pathology.rs`).
    pathology: pathology::PathologyState,
    /// OpenColorIO display/view for scene-linear images (`ocio.rs`).
    ocio: ocio::Ocio,
    /// "Export annotations…" (`annot_export.rs`).
    annot_export: annot_export::AnnotExport,
    /// "Overlay label volume…" (`label_volume.rs`).
    label_volume: label_volume::LabelVolumeState,
    /// Figures, key images and teaching files (`figure.rs`).
    figure: figure::FigureState,
    /// The measurement list of the current image and its tool.
    measure: measure_tool::MeasureState,
    /// The markup of the current file (a sidecar) and its tool.
    markup: markup::MarkupState,
    /// Two revisions of a sheet compared.
    sheets: sheet_compare::CompareState,
    convert: convert::ConvertState,
    export_recipes: export_recipes::RecipesState,
    /// Rename, copy / move to folder, rotation saved to files, their undo and redo.
    fileops: fileops::FileOps,
    /// Export slideshow as video (`video_export.rs`).
    video_export: video_export::ExportState,
    /// DICOMweb servers, study search and streamed series (`dicomweb.rs`).
    dicomweb: dicomweb::DicomWebState,
    /// OMERO servers and the IDR, browsed read-only (`omero.rs`).
    omero: omero::OmeroState,
    /// The prompt for an encrypted archive's password (`password.rs`).
    password: password::PasswordPrompt,
    /// The thermal report dialog and a report being written (`science/thermal_report.rs`).
    thermal_report: science::thermal_report::ReportState,
    /// Client galleries and client picks (`web_gallery.rs`).
    web_gallery: web_gallery::WebGallery,
    /// File › Open address… (`address.rs`).
    address: address::AddressState,
    /// The network stream playing (`stream.rs`).
    stream: stream::StreamState,
    /// The layout dialog of a headerless volume (`raw_volume.rs`).
    raw_volume: raw_volume::RawVolume,
    file_kind: file_kind::FileKind,
    /// Remote control: the socket server and `specular://` links (`remote.rs`).
    remote: remote::Remote,
    /// Recipes in flight and the ones the user trusts (`recipes.rs`).
    recipes: recipes::Recipes,
    /// DS9 regions, plate solving, SAMP and sky surveys (science mode).
    astro: science::astro::Astro,
    /// Window / level and the value readout of DICOM images (`medical.rs`).
    medical: medical::Medical,
    /// Multiplanar reformatting of a DICOM series (`mpr.rs`).
    mpr: mpr::Mpr,
    linked: linked::Linked,
    science: science::Science,
    geo: geo::Geo,
    /// The keyboard crosshair and the value under the cursor for screen readers.
    cursor: cursor::Cursor,
    cad: cad::Cad,
    /// A book's table of contents panel (`contents.rs`).
    contents: contents::Contents,
    /// Tethered capture: the panel and the camera session.
    tether: tether::Tether,
    /// The specimen of a font file (`font_view.rs`).
    font_view: font_view::FontView,
    /// Text, tables, Markdown sources and subtitle cues (`text_view.rs`).
    text_view: text_view::TextView,
    /// A sound file's page (`sound_page.rs`).
    sound_page: sound_page::SoundPage,
    /// Info's loudness of movies (`info_loudness.rs`).
    info_loudness: info_loudness::InfoLoudness,
    /// The map view of georeferenced images and vector maps (`map.rs`, the Maps profile).
    map: map::Map,
    /// A 3D model shown in the 3D view (`mesh.rs`).
    mesh: mesh::MeshView,
    /// The relightable image of the current file and its light (app/relight.rs).
    relight: relight::RelightView,
    /// The start screen's recent folders, its requests, the startup choice and the hints.
    welcome: welcome::WelcomeState,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    args: Args,
    hw: HardwareProfile,
    loader: Loader,
    events: Receiver<Event>,
    target: Option<PathBuf>,
    first: Option<(PathBuf, JobHandle, Instant)>,
    gpu: JoinHandle<Result<Gpu, String>>,
    text: JoinHandle<Text>,
) -> Result<(), String> {
    // This thread runs the event loop: the catalogue refuses it (debug builds).
    specular_platform::mark_ui_thread();
    let event_loop = EventLoop::<()>::with_user_event()
        .build()
        .map_err(|e| format!("cannot start the event loop: {e}"))?;
    let app_t0 = specular_core::ms_since_start();
    let proxy = event_loop.create_proxy();
    loader.set_wake(Arc::new(move || {
        let _ = proxy.send_event(());
    }));
    // Memory-pressure changes wake the loop; `about_to_wait` trims sessions and re-plans prefetch.
    let pressure_proxy = event_loop.create_proxy();
    specular_platform::watch_memory_pressure(move || {
        let _ = pressure_proxy.send_event(());
    });
    // Ctrl+C / SIGTERM: leave like a closed window, through `exiting`.
    let quit_proxy = event_loop.create_proxy();
    crate::quit_signal::install(move || {
        let _ = quit_proxy.send_event(());
    });

    // Hidden test hook: later opens, through the same event as the open dialog.
    if !args.then_open.is_empty() {
        let l = loader.clone();
        let paths = args.then_open.clone();
        let _ = specular_thread::builder("test-then-open").spawn(move || {
            for p in paths {
                std::thread::sleep(Duration::from_millis(1500));
                l.post(Event::Picked(Some(p)));
            }
        });
    }

    // Settings: the user's file, or a throw-away store for benchmarks and screenshots.
    setting_fields::register_checks();
    let headless = args.bench_open.is_some() || args.screenshot.is_some();
    let store = if headless {
        SettingsStore::in_memory()
    } else {
        SettingsStore::open_default().unwrap_or_else(|e| {
            specular_log::warn!(target: SETTINGS, "settings: {e}");
            SettingsStore::in_memory()
        })
    };
    for (arg, e) in store.apply_set_args(&args.sets) {
        eprintln!("specular: --set {arg}: {e}");
    }
    if let Some(l) = &args.lang
        && let Err(e) = store.set_text("general.language", l, Origin::CommandLine)
    {
        eprintln!("specular: --lang {l}: {e}");
    }
    if !headless {
        // An external edit wakes the loop, which applies it in `about_to_wait`; without the
        // wake it waited for the next unrelated event (forever when idle).
        let wake = loader.waker();
        store.set_on_reload(Arc::new(move || wake()));
        let _ = store.watch();
        // The past week's slow opens, for the advisor's patterns (DESIGN.md §12.7).
        let kept = open_report::kept_path();
        let _ = specular_thread::builder("perf-history").spawn(move || open_report::keep_at(kept));
    }
    let settings = store.snapshot();
    loader.set_display_cache(display_cache_for(&settings, headless));
    I18n::configure_global(
        settings.general.language.clone(),
        settings.general.region_formats.locale(),
    );

    let watchdog = specular_log::Watchdog::spawn(
        stall_threshold(settings.performance.stall_warning),
        loader.jobs_dump(),
    );
    let mut app = App {
        hw,
        loader,
        events,
        gpu_join: Some(gpu),
        text_join: Some(text),
        gfx: None,
        presenter: None,
        monitors: present::Monitors::default(),
        a11y: None,
        a11y_events: Vec::new(),
        dl: DrawList::new(1.0),
        chrome: Chrome::new(),
        settings_gen: u64::MAX,
        i18n_gen: u64::MAX,
        store,
        settings,
        keymap: Keymap::viewer_default(),
        clipboard: Clipboard::default(),
        entries: Vec::new(),
        path_index: path_index::PathIndex::default(),
        current: None,
        images: HashMap::new(),
        by_path: HashMap::new(),
        page_ahead: Vec::new(),
        next_id: 2,
        shown: None,
        pending_zoom: false,
        view: View::new(),
        gpu_images: GpuImageCache::default(),
        tile_jobs: Vec::new(),
        tiles_failed: HashSet::new(),
        tiles_missing: 0,
        tile_stats: loupe_bench::TileStats::default(),
        pointer_vel: None,
        folder: None,
        detail_level: None,
        navigator: false,
        nav_drag: false,
        zoom_drag: None,
        shot_loupe: false,
        loupe_latched: false,
        pointer: None,
        gestures: gesture::Gestures::default(),
        mods: ModifiersState::empty(),
        dragging: None,
        last_click: None,
        click_toggled_play: false,
        title_shown: String::new(),
        title_bar: None,
        hold: None,
        nav_dir: 1,
        pixel_grid: true,
        fullscreen: false,
        video_full: Default::default(),
        player: Default::default(),
        cast: Default::default(),
        queue: queue::PlayQueue::new(),
        sound_fx: Default::default(),
        outdev: Default::default(),
        video_full_shown: None,
        cursor_hidden: false,
        pointer_moved: 0.0,
        cursor_icon: winit::window::CursorIcon::Default,
        flipping: false,
        chrome_commands: Vec::new(),
        ime: None,
        last_key_text: None,
        t0: Instant::now(),
        last_frame: Instant::now(),
        hdr_checked: None,
        scene: None,
        scene_pixels: None,
        blur_valid: false,
        hud: HudStats::default(),
        trash_undo: HashMap::new(),
        next_undo: 1,
        last_undo: None,
        bench: None,
        shot: None,
        fatal: None,
        redraw: true,
        causes: FrameCauses::from_env(),
        frame_asked: false,
        present_now: false,
        ready_at: None,
        deferred_uploads: VecDeque::new(),
        upload_batch: None,
        prefetch_held: true,
        wake_at: None,
        redraw_asked: None,
        ui_scale: 1.0,
        diag: diagnostics::Diag::default(),
        keys: keys::Keys::default(),
        exit_requested: false,
        args,
        watchdog,
        frames: 0,
        startup_bench: std::env::var_os("SPECULAR_BENCH_STARTUP")
            .is_some_and(|v| v == "1")
            .then(StartupBench::default),
        anim: None,
        anim_failed: None,
        loop_forever: false,
        motion: None,
        reading: Default::default(),
        series: Default::default(),
        report: Default::default(),
        studies: Default::default(),
        profile_state: Default::default(),
        inspect: Default::default(),
        integration: integration::Integration::new(),
        zynk: zynk_send::ZynkState::new(),
        codes: codes::Codes::new(),
        find_text: ocr::FindText::new(),
        face_view: faces_view::FaceView::new(),
        decoders: decoders::Decoders::default(),
        devices: devices::DevicesState::default(),
        setting_pick: Default::default(),
        insights: Default::default(),
        work: Default::default(),
        net: Default::default(),
        gallery: gallery::Gallery::default(),
        colour: colour::ColourState::default(),
        slideshow: Default::default(),
        quiz: Default::default(),
        cull: cull::Cull::from_env(),
        develop: develop::Develop::default(),
        print: print::PrintState::default(),
        anon: anon_export::AnonState::default(),
        pathology: pathology::PathologyState::default(),
        ocio: ocio::Ocio::default(),
        annot_export: annot_export::AnnotExport::default(),
        label_volume: label_volume::LabelVolumeState::default(),
        figure: figure::FigureState::default(),
        measure: measure_tool::MeasureState::default(),
        markup: markup::MarkupState::default(),
        sheets: sheet_compare::CompareState::default(),
        convert: convert::ConvertState::default(),
        export_recipes: export_recipes::RecipesState::default(),
        fileops: fileops::FileOps::default(),
        video_export: video_export::ExportState::default(),
        dicomweb: dicomweb::DicomWebState::default(),
        omero: omero::OmeroState::default(),
        password: password::PasswordPrompt::default(),
        thermal_report: Default::default(),
        web_gallery: web_gallery::WebGallery::default(),
        address: address::AddressState::default(),
        stream: stream::StreamState::default(),
        raw_volume: raw_volume::RawVolume::default(),
        file_kind: file_kind::FileKind::default(),
        remote: Default::default(),
        recipes: recipes::Recipes::default(),
        astro: science::astro::Astro::default(),
        medical: medical::Medical::default(),
        mpr: mpr::Mpr::default(),
        linked: linked::Linked::default(),
        science: science::Science::default(),
        geo: geo::Geo::default(),
        cursor: cursor::Cursor::default(),
        cad: cad::Cad::default(),
        contents: contents::Contents::default(),
        tether: tether::Tether::default(),
        font_view: font_view::FontView::default(),
        text_view: text_view::TextView::default(),
        sound_page: sound_page::SoundPage::default(),
        info_loudness: info_loudness::InfoLoudness::default(),
        map: map::Map::default(),
        mesh: mesh::MeshView::default(),
        relight: relight::RelightView::default(),
        welcome: welcome::WelcomeState::default(),
    };
    // Something to show was given: the startup choice (`general.on_start`) does not apply.
    app.welcome.given = app.args.path.is_some()
        || !app.args.urls.is_empty()
        || !app.args.file_list.is_empty()
        || app.args.library
        || app.args.slideshow.is_some()
        || app.args.remote_zarr.is_some()
        || app.args.remote_fetch.is_some();
    app.remote = remote::Remote::new(std::mem::take(&mut app.args.urls));
    app.start_remote();
    if let Some(url) = app.args.remote_zarr.take() {
        app.zarr_open(url);
    }
    if let Some(url) = app.args.remote_fetch.take() {
        app.remote_fetch_open(url);
    }
    // `--compare A B…`: the gallery's compare over the first one's folder.
    let compare = std::mem::take(&mut app.args.compare);
    if !compare.is_empty() {
        let mode = app
            .args
            .compare_mode
            .as_deref()
            .and_then(gallery::CmpMode::from_id)
            .unwrap_or_default();
        app.compare_files(compare, mode);
    }
    app.apply_settings(true);
    app.keys_init();
    app.studies_init();
    app.import_init();
    app.profile_first_run();
    // The last run ended by a panic on the UI thread: offer its report once (View, Copy,
    // Save diagnostics bundle…, Dismiss). Nothing is sent.
    app.chrome
        .set_contact(specular_diag::ISSUES_URL, specular_diag::SUPPORT_EMAIL);
    if let Some(report) = app.args.last_crash.take() {
        app.crash_prompt(&report);
    }
    // At most once a day, and only when Settings › Updates turns it on.
    app.update_check(false);
    if app.settings.metadata.show_info_on_open {
        app.chrome
            .set_info(true, app.settings.metadata.info_detail == InfoDetail::All);
    }
    if app.args.bench_open.is_some() {
        let now = Instant::now();
        app.bench = Some(Bench {
            phase: BenchPhase::Sharp,
            first_pixels_ms: None,
            sharp_ms: None,
            sharp_scale: 8,
            overview_ms: Vec::new(),
            regions: Vec::new(),
            region_i: 0,
            region_t: now,
            region_tiles: Vec::new(),
            phase_t: now,
            neighbour: None,
            prefetch_wait_ms: 0.0,
            next_ms: None,
            next_first_ms: None,
            next_file: String::new(),
            started: now,
            gpu_init_ms: 0.0,
            upload_ms: Vec::new(),
            hold: None,
            pan: None,
            loupe: None,
            stages: vec![
                ("process_start".to_owned(), 0.0),
                ("event_loop_created".to_owned(), app_t0),
                ("app_built".to_owned(), specular_core::ms_since_start()),
            ],
        });
    }
    if let Some(p) = app.args.screenshot.clone() {
        app.shot = Some(Shot {
            path: p,
            applied: false,
            ready_since: None,
            started: Instant::now(),
        });
    }

    // `specular -` / `--files-from`: exactly the listed files, in their order.
    let list = std::mem::take(&mut app.args.file_list);
    // The first file was submitted by main before anything else.
    match (first, target) {
        (Some((path, handle, at)), _) if !list.is_empty() => {
            let mut img = Img::new(path.clone());
            img.token = handle.token.clone();
            img.opening = true;
            img.state = "requested (command line)".to_owned();
            img.shown_at = Some(at);
            app.images.insert(1, img);
            app.by_path.insert(path.clone(), 1);
            let at = list.iter().position(|p| *p == path).unwrap_or(0);
            app.set_entries(list);
            app.current = Some(at);
        }
        (None, Some(_)) if !list.is_empty() => {
            app.set_entries(list);
            app.go_to(0);
        }
        (Some((path, handle, at)), _) => {
            let mut img = Img::new(path.clone());
            img.token = handle.token.clone();
            img.opening = true;
            img.state = "requested (command line)".to_owned();
            debug!(target: DECODE, "img=1 {} requested (command line)", img.name());
            img.shown_at = Some(at);
            app.images.insert(1, img);
            app.by_path.insert(path.clone(), 1);
            app.set_entries(vec![path.clone()]);
            app.current = Some(0);
            app.series_start(&path);
            if let Some(dir) = path.parent() {
                app.scan_folder(dir.to_path_buf());
            }
        }
        (None, Some(p)) => app.open_target(p),
        (None, None) => {}
    }

    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("event loop: {e}"))?;
    if app.args.print_marked {
        let files = app.marked_files();
        if let Err(e) = crate::file_list::print(&files, app.args.null) {
            eprintln!("specular: --print-marked: {e}");
        }
    }
    match app.fatal {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// The persistent display cache the settings ask for. Benchmarks and screenshots use one only
/// when given a folder (`--set performance.display_cache_location=…`), so their timings stay
/// those of a first open.
pub fn default_display_cache_dir() -> PathBuf {
    specular_settings::Locations::detect()
        .cache_dir
        .join(crate::display_cache::SUBDIR)
}

/// Where the display cache will likely be, before the settings are read: the folder given on
/// the command line, else (except for benchmarks and screenshots) the default one.
pub fn probable_display_cache_dir(args: &Args) -> Option<PathBuf> {
    let given = args.sets.iter().rev().find_map(|a| {
        a.strip_prefix("performance.display_cache_location=")
            .map(PathBuf::from)
    });
    let headless = args.bench_open.is_some() || args.screenshot.is_some();
    given.or_else(|| (!headless).then(default_display_cache_dir))
}

fn display_cache_for(s: &Settings, headless: bool) -> Option<Arc<DisplayCache>> {
    let p = &s.performance;
    if !p.display_cache {
        return None;
    }
    let dir = match &p.display_cache_location {
        Some(d) => d.clone(),
        None if headless => return None,
        None => default_display_cache_dir(),
    };
    debug!(target: CACHE, "display cache: {} MiB in {}", p.display_cache_size, dir.display());
    Some(DisplayCache::new(
        dir,
        p.display_cache_size.max(1) as u64 * (1 << 20),
    ))
}

fn now_s(t0: Instant) -> f64 {
    t0.elapsed().as_secs_f64()
}

fn rss_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
        })
        .map(|pages| pages * 4096)
        .unwrap_or(0)
}

fn peak_rss_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
        })
        .map(|kb| kb * 1024)
        .unwrap_or(0)
}

/// The significant bits auto-range uses for an image, or `None` when it is not auto-ranged:
/// the setting is off, the image is DICOM, or its overview is not wide greyscale.
fn auto_range_bits(info: &ImageInfo, raster: &Raster, s: &Settings) -> Option<u8> {
    use specular_core::PixelLayout;
    let wide_grey = matches!(
        raster.layout,
        PixelLayout::Gray16 | PixelLayout::GrayAlpha16
    );
    (s.inspection.auto_range && wide_grey && info.format != specular_decode::Format::Dicom)
        .then_some(info.bit_depth.clamp(9, 16))
}

fn settings_color(c: specular_settings::Color) -> Color {
    Color::rgba(
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    )
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// The modifier keys of winit's state, as the keymap and the chrome name them (on macOS,
/// Command is the keymap's Ctrl).
fn mods_of(m: ModifiersState) -> Mods {
    Mods::held(
        specular_input::Os::current(),
        m.control_key(),
        m.shift_key(),
        m.alt_key(),
        m.super_key(),
    )
}

/// Converts a winit key event into a key combination of the keymap.
fn combo_of(ev: &KeyEvent, m: ModifiersState) -> Option<KeyCombo> {
    use winit::keyboard::{Key as WK, KeyCode, NamedKey as WN, PhysicalKey};
    let mods = mods_of(m);
    let key = match &ev.logical_key {
        WK::Named(n) => Key::Named(match n {
            WN::ArrowLeft => NamedKey::ArrowLeft,
            WN::ArrowRight => NamedKey::ArrowRight,
            WN::ArrowUp => NamedKey::ArrowUp,
            WN::ArrowDown => NamedKey::ArrowDown,
            WN::Space => NamedKey::Space,
            WN::Enter => NamedKey::Enter,
            WN::Escape => NamedKey::Escape,
            WN::Backspace => NamedKey::Backspace,
            WN::Delete => NamedKey::Delete,
            WN::Tab => NamedKey::Tab,
            WN::Home => NamedKey::Home,
            WN::End => NamedKey::End,
            WN::PageUp => NamedKey::PageUp,
            WN::PageDown => NamedKey::PageDown,
            WN::Insert => NamedKey::Insert,
            WN::F1 => NamedKey::F(1),
            WN::F2 => NamedKey::F(2),
            WN::F3 => NamedKey::F(3),
            WN::F4 => NamedKey::F(4),
            WN::F5 => NamedKey::F(5),
            WN::F6 => NamedKey::F(6),
            WN::F7 => NamedKey::F(7),
            WN::F8 => NamedKey::F(8),
            WN::F9 => NamedKey::F(9),
            WN::F10 => NamedKey::F(10),
            WN::F11 => NamedKey::F(11),
            WN::F12 => NamedKey::F(12),
            _ => return None,
        }),
        WK::Character(s) => {
            let c = s.chars().next()?;
            if c == ' ' {
                Key::Named(NamedKey::Space)
            } else if !c.is_ascii() && (mods.ctrl || mods.alt || mods.meta) {
                // Shortcuts on non-Latin layouts: use the key's Latin position.
                let PhysicalKey::Code(code) = ev.physical_key else {
                    return None;
                };
                let name = format!("{code:?}");
                let ch = match code {
                    KeyCode::Comma => ',',
                    KeyCode::Period => '.',
                    KeyCode::Equal => '=',
                    KeyCode::Minus => '-',
                    _ => name
                        .strip_prefix("Key")
                        .or_else(|| name.strip_prefix("Digit"))
                        .and_then(|s| s.chars().next())?
                        .to_ascii_lowercase(),
                };
                Key::Char(ch)
            } else {
                Key::Char(c)
            }
        }
        _ => return None,
    };
    Some(KeyCombo::new(key, mods))
}

/// The digit of a digit-row key, whatever it types with Shift, for [`Keymap::resolve`]:
///
/// - its unshifted character when that is a digit (US, German and most layouts; not the
///   keypad);
/// - a key that types the same US Shift+digit symbol with Shift as without (`!` for 1): the
///   one-level keys of virtual keyboards such as wtype's, whose key codes are arbitrary;
/// - else its position in the row (US layout): AZERTY types "&" unshifted on the 1 key. Not
///   for keys whose unshifted character is a Latin letter, which no layout puts in the digit
///   row (a virtual keyboard's arbitrary key code).
fn row_digit(ev: &KeyEvent, shift: bool) -> Option<char> {
    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
    digit_of(
        ev.physical_key,
        &ev.logical_key,
        &ev.key_without_modifiers(),
        shift,
    )
}

fn digit_of(
    physical: winit::keyboard::PhysicalKey,
    logical: &winit::keyboard::Key,
    unshifted: &winit::keyboard::Key,
    shift: bool,
) -> Option<char> {
    use winit::keyboard::{Key as WK, KeyCode as K, PhysicalKey};
    let one = |k: &WK| match k {
        WK::Character(s) => {
            let mut it = s.chars();
            it.next().filter(|_| it.next().is_none())
        }
        _ => None,
    };
    // The keypad's digits are not the row's (Shift+keypad 1 is End).
    if let PhysicalKey::Code(c) = physical
        && format!("{c:?}").starts_with("Numpad")
    {
        return None;
    }
    let base = one(unshifted);
    if let Some(c) = base.filter(char::is_ascii_digit) {
        return Some(c);
    }
    if shift
        && let Some(c) = base
        && one(logical) == Some(c)
        && let Some(d) = ")!@#$%^&*(".find(c)
    {
        return char::from_digit(d as u32, 10);
    }
    if base.is_some_and(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let PhysicalKey::Code(code) = physical else {
        return None;
    };
    let d = [
        K::Digit0,
        K::Digit1,
        K::Digit2,
        K::Digit3,
        K::Digit4,
        K::Digit5,
        K::Digit6,
        K::Digit7,
        K::Digit8,
        K::Digit9,
    ]
    .iter()
    .position(|k| *k == code)?;
    char::from_digit(d as u32, 10)
}

/// The theme of the window's frame (Windows' title bar, macOS's appearance, the decorations
/// winit draws on Wayland): Specular's own when one is chosen, else the system's. Without it a
/// dark Specular on a light Windows has a light title bar.
fn window_theme(theme: Theme) -> Option<winit::window::Theme> {
    match theme {
        Theme::Dark => Some(winit::window::Theme::Dark),
        Theme::Light => Some(winit::window::Theme::Light),
        Theme::System => None,
    }
}

/// The application id: Wayland `app_id` and X11 `WM_CLASS`, matching the desktop file and
/// `StartupWMClass` in packaging/linux (the desktop shell finds the icon and name by it), and
/// Windows' AppUserModelID, matching the installer's Start-menu shortcut (main.rs sets it).
/// macOS takes the name from the bundle.
#[cfg(any(
    windows,
    all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    )
))]
pub const APP_ID: &str = "app.specular.Specular";

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
fn with_app_id(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    use winit::platform::{wayland::WindowAttributesExtWayland, x11::WindowAttributesExtX11};
    let attrs = WindowAttributesExtWayland::with_name(attrs, APP_ID, "specular");
    WindowAttributesExtX11::with_name(attrs, APP_ID, "specular")
}

/// Windows: the icon compiled into specular.exe (build.rs, resource 1) on the title bar and the
/// taskbar button; Windows picks the image size it needs from the icon.
#[cfg(windows)]
fn with_app_id(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    use winit::platform::windows::{IconExtWindows, WindowAttributesExtWindows};
    let icon = || winit::window::Icon::from_resource(1, None).ok();
    attrs.with_window_icon(icon()).with_taskbar_icon(icon())
}

#[cfg(not(any(
    windows,
    all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    )
)))]
fn with_app_id(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    attrs
}

impl App {
    /// Records a benchmark stage at the current time.
    fn stage(&mut self, name: &str) {
        if let Some(b) = self.bench.as_mut() {
            b.stage(name, specular_core::ms_since_start());
        }
    }

    fn now(&self) -> f64 {
        now_s(self.t0)
    }

    /// Left/Right (arrow keys, page swipes) follow a right-to-left reading direction.
    fn arrows_reversed(&self) -> bool {
        (self.settings.browsing.arrows_follow_reading_direction && self.rtl()) || self.reading_rtl()
    }

    fn rtl(&self) -> bool {
        I18n::global().direction().is_rtl()
    }

    /// Records and logs a load-state transition of an image (`img=7 2.jpeg header → level 1/8`).
    fn set_state(&mut self, id: ImageId, state: String) {
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        if img.state != state {
            debug!(target: DECODE, "img={id} {} {} → {state}", img.name(), img.state);
            img.state = state;
        }
    }

    /// Whether the viewer's centre message is a notice (an office document without a preview)
    /// rather than an error, for the chrome to draw it muted.
    fn notice_sync(&mut self) {
        let notice = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .is_some_and(|i| i.error.is_none() && no_preview(i));
        if self.chrome.set_notice(notice) {
            self.redraw = true;
        }
    }

    /// Tells the watchdog whether the current image is still loading.
    fn watch_current(&self) {
        let loading = |id: ImageId, i: &Img| {
            !(self.gpu_images.contains(id) && i.gpu_rank.is_some())
                && i.error.is_none()
                && !no_preview(i)
        };
        match self
            .current_id()
            .and_then(|id| self.images.get(&id).map(|i| (id, i)))
        {
            Some((id, img)) if loading(id, img) => {
                self.watchdog.loading(id, &img.name(), &img.state)
            }
            _ => self.watchdog.clear(),
        }
    }

    /// Lists `dir` in the background with the `[browsing]` settings; `Event::Folder` follows.
    fn scan_folder(&mut self, dir: PathBuf) {
        let b = &self.settings.browsing;
        let opts = ListOptions {
            order: b.sort_order,
            descending: b.sort_descending,
            recursive: b.include_subfolders,
        };
        debug!(target: FOLDER, "scan {} ({opts:?})", dir.display());
        self.folder = Some(dir.clone());
        self.loader.scan_folder(dir, opts);
    }

    /// The images in `dir`, listed: the viewer's listing, the current image kept current.
    fn folder_listed(&mut self, dir: PathBuf, files: Vec<PathBuf>) {
        if self.folder.as_ref() != Some(&dir) {
            debug!(target: FOLDER, "ignoring the listing of {}: another folder is open", dir.display());
            return;
        }
        debug!(target: FOLDER, "{} entries in {}", files.len(), dir.display());
        // Cull mode lists one file per RAW+JPEG item.
        let mut files = self.cull_listed(&dir, files);
        let cur_path = self.current.and_then(|i| self.entries.get(i)).cloned();
        let idx = match &cur_path {
            Some(p) => match files.iter().position(|f| f == p) {
                Some(i) => Some(i),
                None => {
                    files.insert(0, p.clone());
                    Some(0)
                }
            },
            None => (!files.is_empty()).then_some(0),
        };
        self.set_entries(files);
        match (cur_path, idx) {
            (Some(_), Some(i)) => {
                self.current = Some(i);
                self.schedule();
            }
            (None, Some(i)) => self.go_to(i),
            _ => {}
        }
        self.redraw = true;
    }

    fn set_entries(&mut self, files: Vec<PathBuf>) {
        self.series_listing_replaced(&files);
        self.path_index = path_index::PathIndex::new(&files);
        let old = std::mem::replace(&mut self.entries, files);
        self.slideshow.listing_replaced(old);
    }

    /// The position of `p` in the viewer's list.
    pub(crate) fn index_of(&self, p: &Path) -> Option<usize> {
        self.path_index.get(&self.entries, p)
    }

    fn id_for(&mut self, idx: usize) -> ImageId {
        let path = self.entries[idx].clone();
        if let Some(id) = self.by_path.get(&path) {
            return *id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.images.insert(id, Img::new(path.clone()));
        self.by_path.insert(path, id);
        id
    }

    /// Reads the current file again (Reload, or a decoder it needed was found).
    fn reload_current(&mut self) {
        if let Some(id) = self.current_id()
            && let Some(img) = self.images.remove(&id)
        {
            img.token.cancel();
            self.by_path.remove(&img.path);
            self.gpu_images.remove(id);
            if let Some(g) = self.gfx.as_mut() {
                g.renderer.release_image(id);
            }
            self.shown = None;
            if let Some(i) = self.current {
                self.go_to(i);
            }
        }
    }

    fn current_id(&self) -> Option<ImageId> {
        self.current
            .and_then(|i| self.entries.get(i))
            .and_then(|p| self.by_path.get(p))
            .copied()
    }

    /// Image-area size in physical pixels.
    fn viewport_px(&self) -> Option<(u32, u32)> {
        let g = self.gfx.as_ref()?;
        let (w, h) = g.surface.size();
        (w > 1 && h > 1).then_some((w, h))
    }

    // ----- Opening and navigation ------------------------------------------------------

    fn open_target(&mut self, p: PathBuf) {
        let p = std::path::absolute(&p).unwrap_or(p);
        self.series_close();
        // A DICOMDIR opens the studies browser on its medium.
        if p.file_name()
            .is_some_and(specular_dicom::series::is_dicomdir_name)
            && p.is_file()
        {
            self.studies_start(specular_dicom::media::Detected::Dicomdir(p));
            self.redraw = true;
            return;
        }
        // A Zarr store, a Relight or a CASA image is a folder that opens as one image.
        let is_dir = p.is_dir() && !specular_core::is_image_folder(&p);
        // A folder may be a patient's medium (found in the background).
        if is_dir {
            self.studies_detect(&p);
        }
        if is_dir && self.settings.cull.auto_enter && self.args.screenshot.is_none() {
            self.cull.auto_dir = Some(p.clone());
        }
        if is_dir && self.gallery_open_folder(&p) {
            self.redraw = true;
            return;
        }
        self.gallery_dismiss();
        // A playlist opens as the play queue.
        if !is_dir && self.queue_open_playlist(&p) {
            self.redraw = true;
            return;
        }
        if self.open_book(&p) {
            self.redraw = true;
            return;
        }
        if is_dir {
            self.set_entries(Vec::new());
            self.current = None;
            self.scan_folder(p);
        } else {
            // A file the user opened may be the target of an HDF5 external link from another
            // folder (an Eiger master beside a data directory).
            specular_decode::allow_linked_file(&p);
            // Opening a file explicitly retries it if it failed or was cut short before (it may
            // have been incomplete when it was first read).
            if let Some(&id) = self.by_path.get(&p)
                && self
                    .images
                    .get(&id)
                    .is_some_and(|i| i.error.is_some() || i.truncated)
                && let Some(img) = self.images.remove(&id)
            {
                debug!(target: FOLDER, "img={id} {} failed or was incomplete before; opening it afresh", img.name());
                img.token.cancel();
                self.by_path.remove(&img.path);
                self.gpu_images.remove(id);
                if let Some(g) = self.gfx.as_mut() {
                    g.renderer.release_image(id);
                }
            }
            self.set_entries(vec![p.clone()]);
            self.go_to(0);
            self.series_start(&p);
            if let Some(dir) = p.parent() {
                self.scan_folder(dir.to_path_buf());
            }
        }
        self.redraw = true;
    }

    fn go_to(&mut self, idx: usize) {
        if idx >= self.entries.len() {
            return;
        }
        let prev = self.current_id();
        self.current = Some(idx);
        let id = self.id_for(idx);
        if let Some(img) = self.images.get(&id) {
            debug!(target: FOLDER, "go to {}/{}: img={id} {} ({})", idx + 1, self.entries.len(), img.name(), img.state);
        }
        if prev != Some(id) {
            self.present_now = true;
            for j in self.tile_jobs.drain(..) {
                j.handle.cancel();
            }
            if let Some(img) = self.images.get_mut(&id) {
                img.shown_at = Some(Instant::now());
                img.open_ms = None;
                img.whole_ms = None;
                img.track.shown();
            }
            self.chrome.set_open_report(None);
            self.report_left();
        }
        self.ensure_open(id, Priority::Immediate);
        self.request_levels(id, Priority::Immediate);
        self.upload(id);
        self.setup_view();
        self.request_metadata();
        self.report_opened();
        self.events_shown();
        self.schedule();
        let now = self.now();
        self.chrome.poke(now);
        self.update_title();
        self.redraw = true;
    }

    /// Whether `navigate(delta)` would move.
    fn can_navigate(&mut self, delta: i64) -> bool {
        if let Some(can) = self.spread_can_navigate(delta) {
            return can;
        }
        let Some(cur) = self.current else {
            return false;
        };
        self.step_target(cur, delta)
            .is_some_and(|(next, _)| next != cur)
    }

    /// Whether stepping skips entry `i` from entry `from`: text-like files (text, tables,
    /// subtitles) and the rest are stepped through apart; the kind of the file stepped from
    /// decides which.
    pub(super) fn steps_over(&self, from: usize, i: usize) -> bool {
        let text = |k: usize| {
            self.entries
                .get(k)
                .is_some_and(|p| specular_decode::text_dec::is_text_file(p))
        };
        text(from) != text(i)
    }

    /// Where stepping `delta` entries from `cur` lands (and whether it wrapped round).
    fn step_target(&self, cur: usize, delta: i64) -> Option<(usize, bool)> {
        let wrap =
            self.settings.browsing.loop_folder && self.entries.len() >= 2 && !self.series_active();
        step_among(self.entries.len(), cur, delta, wrap, |i| {
            !self.steps_over(cur, i)
        })
    }

    fn navigate(&mut self, delta: i64) {
        if self.spread_navigate(delta) {
            return;
        }
        let Some(cur) = self.current else { return };
        let Some((next, wrapped)) = self.step_target(cur, delta) else {
            return;
        };
        if next == cur {
            return;
        }
        if wrapped {
            let now = self.now();
            let msg = if delta > 0 {
                t!("toast-loop-start")
            } else {
                t!("toast-loop-end")
            };
            self.chrome.toast(msg, None, now);
        }
        self.nav_dir = if delta >= 0 { 1 } else { -1 };
        self.go_to(next);
    }

    fn ensure_open(&mut self, id: ImageId, prio: Priority) {
        let vp = self.viewport_for(id);
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        if img.opening || img.session.is_some() || img.error.is_some() {
            return;
        }
        if img.token.is_cancelled() {
            img.token = CancelToken::new();
        }
        img.opening = true;
        img.plan_viewport = vp;
        img.plan_prefetch = prio <= Priority::Prefetch;
        img.levels.clear();
        if img.page > 0 || img.page_base.is_some() {
            let base = img.page_base.take();
            self.loader.open_page(
                id,
                img.path.clone(),
                img.page,
                base,
                prio,
                vp,
                img.token.clone(),
            );
        } else {
            self.loader
                .open(id, img.path.clone(), prio, vp, img.token.clone());
        }
        self.set_state(id, format!("requested ({prio:?})"));
    }

    /// Requests the whole-image scale that is sharp at fit for the window, if no overview
    /// that good exists or is on its way.
    fn request_levels(&mut self, id: ImageId, prio: Priority) {
        if self.flipping {
            // Holding a key flips faster than sharp levels decode; previews only.
            return;
        }
        let Some(vp) = self.viewport_for(id) else {
            return;
        };
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        if no_preview(img) {
            return;
        }
        let (Some(info), Some(session)) = (&img.info, &img.session) else {
            return;
        };
        let (dw, dh) = info.display_size();
        let need = needed_scale(dw, dh, vp.0, vp.1, loader::scale_cap(info));
        if img.rank() <= need || img.levels.iter().any(|&s| s <= need) {
            return;
        }
        if img.token.is_cancelled() {
            img.token = CancelToken::new();
        }
        img.levels.push(need);
        debug!(
            target: DECODE,
            "img={id} {} request level 1/{need} ({prio:?}; on screen: {:?})",
            img.name(),
            img.overview.as_ref().map(|o| o.0)
        );
        self.loader
            .decode_level(id, session.clone(), need, prio, img.token.clone());
    }

    /// Images per key repeat after holding → or ← for `held` seconds.
    fn hold_step(&self, held: f64) -> i64 {
        match self.settings.browsing.hold_acceleration {
            HoldAcceleration::Off => 1,
            HoldAcceleration::Gentle => {
                if held > 2.5 {
                    2
                } else {
                    1
                }
            }
            HoldAcceleration::Normal => {
                if held > 3.0 {
                    4
                } else if held > 1.2 {
                    2
                } else {
                    1
                }
            }
            HoldAcceleration::Fast => {
                if held > 2.0 {
                    8
                } else if held > 0.8 {
                    3
                } else {
                    1
                }
            }
        }
    }

    /// Back while the loupe or the zoom rectangle is held: put it away.
    fn cancel_held_tool(&mut self) -> bool {
        let held = matches!(self.hold, Some((Action::Loupe | Action::ZoomRectangle, _)))
            || self.shot_loupe
            || self.loupe_latched
            || self.zoom_drag.is_some();
        if held {
            self.hold = None;
            self.shot_loupe = false;
            self.loupe_latched = false;
            self.zoom_drag = None;
            self.redraw = true;
        }
        held
    }

    /// The held arrow key was released: decode the sharp levels of where it stopped.
    fn end_flipping(&mut self) {
        if !std::mem::take(&mut self.flipping) {
            return;
        }
        if let Some(id) = self.current_id() {
            self.request_levels(id, Priority::Immediate);
        }
        self.schedule();
        self.redraw = true;
    }

    fn request_metadata(&mut self) {
        if let Some(id) = self.current_id() {
            self.request_metadata_for(id);
        }
    }

    fn request_metadata_for(&mut self, id: ImageId) {
        let lang = I18n::global().language();
        let m = &self.settings.metadata;
        let (places, hide_gps) = (m.reverse_geocoding, m.hide_gps);
        // A thermal still's temperatures follow the readout's unit.
        let unit = self.science_temperature_unit_now();
        // The request key: language, the GPS settings and the temperature unit.
        let tag = format!("{lang}/{places}/{hide_gps}/{}", unit.symbol());
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        let Some(session) = img.session.clone() else {
            return;
        };
        if img.metadata_lang.as_deref() == Some(tag.as_str()) {
            return;
        }
        img.metadata_lang = Some(tag);
        self.loader
            .metadata(id, session, lang, places, hide_gps, unit);
    }

    /// Prefetches the neighbours and lets go of images far away.
    fn schedule(&mut self) {
        if self.prefetch_held {
            // Pages of a file left are let go even so.
            self.page_neighbours(false, &mut HashSet::new());
            return;
        }
        // The presentation window's images (another display) are kept too.
        let presenting = self.presenter_ids();
        let Some(cur) = self.current else {
            for id in presenting {
                self.ensure_open(id, Priority::Prefetch);
                self.request_levels(id, Priority::Prefetch);
                self.upload_or_defer(id);
            }
            return;
        };
        let n = self.entries.len();
        // Smaller under memory pressure, on a hot machine, on battery or in a low-power mode.
        let depth = specular_platform::prefetch_depth(
            self.settings.performance.prefetch_depth.clamp(0, 16) as usize,
        );
        let wrap = self.settings.browsing.loop_folder || self.slideshow.wraps();
        // The slideshow's next slides first (shuffled, they are anywhere in the folder).
        let mut order = self.slideshow.upcoming(cur);
        let at = |d: i64| -> Option<usize> {
            let i = cur as i64 + d;
            if (0..n as i64).contains(&i) {
                Some(i as usize)
            } else if wrap && n > 0 {
                Some(i.rem_euclid(n as i64) as usize)
            } else {
                None
            }
        };
        for d in 1..=depth as i64 {
            let ahead = d * self.nav_dir as i64;
            order.extend(at(ahead));
            // Behind gets a smaller window.
            if d as usize <= depth.div_ceil(2) {
                order.extend(at(-ahead));
            }
        }
        let cur_id = self.id_for(cur);
        let mut window: HashSet<ImageId> = HashSet::from([cur_id]);
        // The pages next to the current one first: they are a key press away.
        self.page_neighbours(true, &mut window);
        // Files about to be opened: read into the page cache in one batch first (io_uring on
        // Linux), so the open jobs' page faults hit memory.
        let unopened: Vec<PathBuf> = order
            .iter()
            .map(|&idx| &self.entries[idx])
            .filter(|p| {
                self.by_path
                    .get(*p)
                    .and_then(|id| self.images.get(id))
                    .is_none_or(|img| img.session.is_none() && !img.opening && img.error.is_none())
            })
            .cloned()
            .collect();
        specular_platform::readahead::warm(unopened);
        for idx in order {
            let id = self.id_for(idx);
            window.insert(id);
            self.ensure_open(id, Priority::Prefetch);
            self.request_levels(id, Priority::Prefetch);
            self.upload_or_defer(id);
        }
        // An OCT volume's localiser, drawn beside it.
        if let Some(id) = self.oct_localiser() {
            window.insert(id);
            self.ensure_open(id, Priority::Immediate);
            self.request_levels(id, Priority::Immediate);
            self.upload_or_defer(id);
        }
        for id in presenting {
            window.insert(id);
            self.ensure_open(id, Priority::Prefetch);
            self.request_levels(id, Priority::Prefetch);
            self.upload_or_defer(id);
        }
        // Everything else: cancel work; drop sessions; drop rasters further away.
        let mut drop_ids = Vec::new();
        for (id, img) in self.images.iter_mut() {
            if window.contains(id) {
                continue;
            }
            let dist = self.path_index.get(&self.entries, &img.path).map(|i| {
                let d = i.abs_diff(cur);
                if wrap { d.min(n - d) } else { d }
            });
            if img.opening || !img.levels.is_empty() {
                debug!(target: PREFETCH, "img={id} {} left the prefetch window: cancel (opening: {}, levels: {:?})", img.name(), img.opening, img.levels);
                img.token.cancel();
                img.opening = false;
                img.levels.clear();
            }
            if img.session.is_some() && dist.is_none_or(|d| d > depth + 1) {
                debug!(target: CACHE, "img={id} {} decoder session closed", img.name());
                img.session = None;
            }
            if dist.is_none_or(|d| d > depth + RAM_SLACK) {
                drop_ids.push(*id);
            }
        }
        for id in drop_ids {
            if let Some(img) = self.images.remove(&id) {
                debug!(target: CACHE, "img={id} {} dropped from memory", img.name());
                self.by_path.remove(&img.path);
            }
        }
        self.trim_gpu(&window);
    }

    /// Lets open decoder sessions drop what they keep only to speed up later decodes (a camera
    /// RAW's held sensor data and sensor tiles, decoded again by its next region): those of
    /// every image but the ones on screen and `also_keep`, or with `all` of every image.
    fn trim_sessions(&mut self, all: bool, also_keep: Option<ImageId>) {
        let cur = self.current_id();
        let freed = trim_open_sessions(
            self.images
                .iter()
                .filter_map(|(&id, img)| Some((id, img.session.as_deref()?))),
            |id| {
                !all && (Some(id) == cur
                    || Some(id) == also_keep
                    || self.is_spread_partner(id)
                    || self.oct_localiser() == Some(id))
            },
        );
        if freed > 0 {
            debug!(target: CACHE, "decoder sessions released {} MiB", freed >> 20);
        }
    }

    /// Under memory pressure: Develop's engine gives back its caches and spare buffers, and
    /// the gallery's thumbnail pages go while the gallery is not shown (made again when it
    /// is). Video memory of images is trimmed by the prefetch re-plan.
    fn trim_gpu_caches(&mut self) {
        self.develop_trim_memory();
        if let Some(g) = self.gfx.as_mut() {
            self.gallery.release_hidden(&mut g.ui);
        }
    }

    fn trim_gpu(&mut self, keep: &HashSet<ImageId>) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        let budget = match self.settings.performance.vram_cache {
            0 => 768u64 << 20,
            mb => (mb as u64) << 20,
        };
        // Halved / quartered under memory pressure (macOS; unified memory is the same RAM).
        let budget = specular_platform::cache_budget(budget);
        let keep: Vec<ImageId> = keep.iter().copied().collect();
        self.gpu_images.trim(budget, &keep, &mut gfx.renderer);
        for (id, img) in self.images.iter_mut() {
            if !self.gpu_images.contains(*id) && img.gpu_rank.is_some() {
                debug!(target: CACHE, "img={id} {} evicted from video memory", img.name());
                img.gpu_rank = None;
            }
        }
    }

    /// Puts an image's best overview on the GPU.
    /// Uploads the current image now; queues any other.
    fn upload_or_defer(&mut self, id: ImageId) {
        if self.current_id() == Some(id)
            || self.is_spread_partner(id)
            || self.oct_localiser() == Some(id)
            || self.presenter_current() == Some(id)
        {
            match self.upload_batch.as_mut() {
                Some(batch) if !batch.contains(&id) => batch.push(id),
                Some(_) => {}
                None => self.upload(id),
            }
        } else if !self.deferred_uploads.contains(&id) {
            self.deferred_uploads.push_back(id);
        }
    }

    /// One queued prefetch upload, skipping images that have left the window since.
    /// Returns whether more are queued.
    fn upload_one_deferred(&mut self) -> bool {
        while let Some(id) = self.deferred_uploads.pop_front() {
            if self.current_id() == Some(id) || self.in_window(id) {
                self.upload(id);
                break;
            }
        }
        !self.deferred_uploads.is_empty()
    }

    fn upload(&mut self, id: ImageId) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        let (Some(info), Some((kind, raster))) = (&img.info, &img.overview) else {
            return;
        };
        if !self.gpu_images.contains(id) {
            img.gpu_rank = None;
            let g = gfx.renderer.create_image(
                &gfx.gpu,
                id,
                info.width,
                info.height,
                img.lut.as_deref(),
            );
            self.gpu_images.insert(g);
            if let Some(g) = self.gpu_images.get(id) {
                g.set_max_scale(loader::scale_cap(info));
            }
            // A reflectance transformation image keeps its coefficients for relighting.
            if let Some(rti) = img.session.as_ref().and_then(|s| s.rti())
                && let Some(g) = self.gpu_images.get(id)
            {
                gfx.renderer.set_relight(&gfx.gpu, g, &rti);
            }
            if let (Some(gm), Some(g)) = (&img.gain_map, self.gpu_images.get(id)) {
                gfx.renderer.set_gain_map(&gfx.gpu, g, Some(gm));
            }
            // Auto-range (DESIGN.md §6.2): high-bit greyscale other than DICOM (which keeps
            // its own window) is stretched from its darkest to its brightest value.
            if let Some(bits) = auto_range_bits(info, raster, &self.settings)
                && let Some(g) = self.gpu_images.get(id)
            {
                gfx.renderer.set_auto_range(&gfx.gpu, g, Some(bits));
            }
        }
        if img.gpu_rank.is_some_and(|r| r <= kind.rank()) {
            return;
        }
        let Some(g) = self.gpu_images.get(id) else {
            return;
        };
        // The renderer's colour path leaves its decision on this thread (the open report).
        let _ = specular_core::paths::take_note();
        gfx.renderer.upload_overview(&gfx.gpu, g, raster);
        img.gpu_rank = Some(kind.rank());
        img.track.upload_ms += gfx.renderer.last_upload_ms;
        if let Some(n) = specular_core::paths::take_note()
            && n.choice.task == specular_core::paths::Task::YcbcrToRgb
        {
            img.track.ycbcr = Some(n.choice.engine);
        }
        if let Some(b) = self.bench.as_mut()
            && self.current.and_then(|i| self.entries.get(i)) == Some(&img.path)
        {
            let label = match kind {
                OverviewKind::Thumbnail => "thumbnail".to_owned(),
                OverviewKind::Level(s) => format!("level_1_{s}"),
            };
            b.stage(
                &format!("{label}_uploaded"),
                specular_core::ms_since_start(),
            );
            b.upload_ms.push((label, gfx.renderer.last_upload_ms));
        }
        self.hud.upload_ms = gfx.renderer.last_upload_ms;
        debug!(target: GPU, "img={id} upload {} {}×{} in {:.1} ms", kind_name(*kind), raster.width, raster.height, gfx.renderer.last_upload_ms);
        // Relit at once for the current light (the decoder's preview is lit from above).
        if let Some(g) = self.gpu_images.get(id)
            && g.is_relightable()
            && let Some(rti) = img.session.as_ref().and_then(|s| s.rti())
        {
            let look = self.relight.look_for(id);
            if gfx.renderer.relight(&gfx.gpu, g, &rti.prepare(&look)) {
                self.relight.applied(id, look);
            }
        }
        let (rank, kname) = (kind.rank(), kind_name(*kind));
        self.presenter_uploaded(id);
        if self.oct_localiser() == Some(id) {
            self.redraw = true;
        }
        if self.current_id() == Some(id) {
            self.redraw = true;
            self.present_now = true;
            let sharp = self.sharp_scale(id).is_some_and(|s| rank <= s);
            let state = if sharp {
                "sharp".to_owned()
            } else {
                format!("{} on screen", kname)
            };
            self.set_state(id, state);
        }
    }

    /// Sets the view up for the current image once its size is known.
    fn setup_view(&mut self) {
        if self.setup_spread_view() {
            return;
        }
        let Some(id) = self.current_id() else {
            self.shown = None;
            return;
        };
        if self.shown == Some(id) {
            return;
        }
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        let Some(info) = &img.info else { return };
        let orient = *img.orient.get_or_insert(info.orientation);
        let v = &self.settings.viewing;
        // Keep zoom: going from a zoomed-in image to another keeps the scale and where the
        // view is in the frame (as a fraction of it), so the same detail of the next shot of
        // a burst is there at once.
        let kept = (v.keep_zoom && self.shown.is_some() && !self.view.is_at_fit()).then(|| {
            let (dw, dh) = self.view.display_size();
            let (cx, cy) = self.view.center();
            (self.view.target_scale(), cx / dw.max(1.0), cy / dh.max(1.0))
        });
        self.view.upscale_small = v.initial_zoom == InitialZoom::Fit;
        self.view.set_image(info.width, info.height, orient);
        // A movie with non-square pixels (DVD, DV, anamorphic) is shown stretched by them;
        // its frames are stored as coded.
        if let Some((hs, vs)) = img
            .session
            .as_ref()
            .and_then(|s| s.video())
            .and_then(|f| f.pixel_aspect)
        {
            self.view.set_pixel_aspect(vs as f32 / hs as f32);
        } else {
            // A DICOM image with non-square pixels (anisotropic Pixel Spacing or a Pixel
            // Aspect Ratio: ultrasound, CR, older MR); a presentation state's area wins later.
            if let Some(m) = img.session.as_ref().and_then(|s| s.medical()) {
                self.view.set_pixel_aspect(m.pixel_aspect() as f32);
            }
        }
        self.pending_zoom = matches!(v.initial_zoom, InitialZoom::ActualSize | InitialZoom::Fill);
        if let Some((scale, u, w)) = kept {
            self.view.zoom_to(scale, None);
            self.view.settle();
            let (dw, dh) = self.view.display_size();
            self.view.center_on((u * dw, w * dh), false);
            self.pending_zoom = false;
        }
        if let Some(width) = self.reading_fit() {
            self.zoom_fit_reading_side(width, true);
            self.pending_zoom = false;
        }
        self.shown = Some(id);
        self.scene = None;
        // The audit log (when on) records each medical image shown.
        if self.settings.advanced.audit_log
            && let Some(modality) = self
                .images
                .get(&id)
                .and_then(|i| i.session.as_ref())
                .and_then(|s| s.medical())
                .map(|m| m.modality.clone().unwrap_or_default())
            && let Some(path) = self.current.and_then(|i| self.entries.get(i)).cloned()
        {
            let extra = [("modality", serde_json::json!(modality))];
            self.audit(audit::Event::Open, "medical-image", &[&path], &extra);
        }
    }

    fn window_title(&self) -> String {
        // The gallery names what it shows (the image opened before stays behind it).
        if self.gallery_active() {
            return self.gallery_title();
        }
        if let Some(t) = self.book_title() {
            return t;
        }
        self.current_id()
            .and_then(|id| self.images.get(&id))
            .map(App::page_title)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Specular".into())
    }

    fn update_title(&mut self) {
        let Some(gfx) = &self.gfx else { return };
        let title = self.window_title();
        if self.title_shown != title {
            gfx.window.set_title(&title);
            self.title_shown = title;
        }
        self.sync_title_bar();
    }

    /// macOS: the title bar in the colour of the top of the window, the gallery's bar or else
    /// the theme's canvas (never the image's background, which the title's text may not
    /// read on); without its title over the start screen and the gallery, whose heading and
    /// path say it. Given again only when it changes.
    fn sync_title_bar(&mut self) {
        if !cfg!(target_os = "macos") {
            return;
        }
        let Some(gfx) = &self.gfx else { return };
        let pal = self.palette();
        let c = if self.gallery_active() {
            pal.surface
        } else {
            pal.canvas
        };
        let show_title = !self.chrome.welcome_shown() && !self.gallery_active();
        let want = ([c.r, c.g, c.b], show_title);
        if self.title_bar != Some(want) {
            specular_platform::title_bar::blend(&*gfx.window, want.0, want.1);
            self.title_bar = Some(want);
        }
    }

    /// Screen-reader activation and actions, queued by `on_event`.
    fn accessibility_requests(&mut self, el: &ActiveEventLoop) {
        for e in std::mem::take(&mut self.a11y_events) {
            match e {
                A11yEvent::Activated | A11yEvent::Deactivated => {
                    let on = matches!(e, A11yEvent::Activated);
                    info!(target: INPUT, "screen reader {}", if on { "active" } else { "gone" });
                    if let Some(a) = self.a11y.as_mut() {
                        a.set_active(on);
                    }
                    self.chrome.set_accessibility(on);
                    self.redraw = true;
                }
                A11yEvent::Action(req) => {
                    debug!(target: INPUT, "screen reader: {:?} on node {:?}", req.action, req.target_node);
                    // The similar-shots screens' nodes take their own actions (the slider's
                    // values too).
                    if self.password_a11y_request(&req)
                        || self.thermal_report_a11y_request(&req)
                        || self.web_gallery_a11y_request(&req)
                        || self.address_a11y_request(&req)
                        || self.mpr_curve_a11y_request(&req)
                        || self.raw_volume_a11y_request(&req)
                        || self.file_kind_a11y_request(&req)
                        || self.print_a11y_request(&req)
                        || self.anon_a11y_request(&req)
                        || self.annot_export_a11y_request(&req)
                        || self.stain_a11y_request(&req)
                        || self.figure_a11y_request(&req)
                        || self.measure_a11y_request(&req)
                        || self.markup_a11y_request(&req)
                        || self.sheets_a11y_request(&req)
                        || self.convert_a11y_request(&req)
                        || self.recipes_a11y_request(&req)
                        || self.fileops_a11y_request(&req)
                        || self.thermal_seq_a11y_request(&req)
                        || self.science_a11y_request(&req)
                        || self.cad_a11y_request(&req)
                        || self.contents_a11y_request(&req)
                        || self.tether_a11y_request(&req)
                        || self.font_a11y_request(&req)
                        || self.text_a11y_request(&req)
                        || self.sound_a11y_request(&req)
                        || self.gis_a11y_request(&req)
                        || self.gallery_field_a11y_request(&req)
                        || self.video_export_a11y_request(&req)
                        || self.dicomweb_a11y_request(&req)
                        || self.omero_a11y_request(&req)
                        || self.compare_a11y_request(&req)
                        || self.similar_a11y_request(&req)
                        || self.map_a11y_request(&req)
                        || self.import_a11y_request(&req)
                        || self.develop_a11y_request(&req)
                        || self.eq_a11y_request(&req)
                    {
                        continue;
                    }
                    let click = req.action == specular_widgets::accesskit::Action::Click;
                    if click && self.keywords_a11y_action(req.target_node.0) {
                        self.redraw = true;
                        continue;
                    }
                    if click && self.meta_a11y_action(req.target_node.0) {
                        self.redraw = true;
                        continue;
                    }
                    if click && self.cull_a11y_action(req.target_node.0) {
                        self.redraw = true;
                        continue;
                    }
                    if matches!(
                        req.action,
                        specular_widgets::accesskit::Action::Click
                            | specular_widgets::accesskit::Action::Focus
                    ) && self.gallery_a11y_action(req.target_node.0, click)
                    {
                        continue;
                    }
                    let r =
                        self.with_viewer_state(|chrome, st| chrome.accessibility_action(&req, st));
                    for c in r.commands {
                        self.command(c, el);
                    }
                    self.redraw = true;
                }
            }
        }
        if self.redraw
            && let Some(g) = &self.gfx
        {
            g.window.request_redraw();
        }
    }

    // ----- Loader events ------------------------------------------------------------------

    fn drain_events(&mut self) {
        self.upload_batch = Some(Vec::new());
        while let Ok(e) = self.events.try_recv() {
            // Anything else may need the image on the GPU (tiles) or its state: upload first.
            if !matches!(e, Event::Overview { .. }) {
                self.flush_uploads();
                self.upload_batch = Some(Vec::new());
            }
            let name = e.name();
            self.because(Cause::Loader, |a| a.on_event(e));
            self.causes.loader_event(name);
        }
        self.flush_uploads();
        self.drain_integration();
        self.drain_zynk();
        self.drain_codes();
        self.drain_find_text();
        self.drain_faces_view();
        self.welcome_poll();
    }

    /// The current-image uploads batched while events drained.
    fn flush_uploads(&mut self) {
        for id in self.upload_batch.take().unwrap_or_default() {
            self.upload_or_defer(id);
        }
    }

    fn on_event(&mut self, e: Event) {
        let cur = self.current_id();
        match e {
            Event::Opened {
                id,
                session,
                lut,
                plan,
                ms,
                at,
                token,
                times,
            } => {
                let Some(img) = self.images.get_mut(&id) else {
                    return;
                };
                img.track.opened = Some(times);
                let info = session.info().clone();
                let header = format!("header {}×{}", info.width, info.height);
                let mut plan = plan;
                plan.extend(self.loader.take_late_levels(id, &token));
                let live = img.opened(&token, plan);
                if !live {
                    debug!(target: DECODE, "img={id} {} opened by a cancelled or superseded job: its levels are not in flight", img.name());
                }
                let still_opening = img.opening;
                img.info = Some(info);
                img.note_pages(&*session);
                img.session = Some(session);
                img.lut = lut;
                self.set_state(id, header);
                self.page_patch(id);
                self.video_patch(id);
                if !self.page_ahead.contains(&id) {
                    self.note_page_size(id);
                }
                // A newer open is on its way with its own plan, or nobody wants the image now
                // (it is asked for again when it comes back into the window).
                if still_opening || (!live && cur != Some(id) && !self.in_window(id)) {
                    self.redraw = true;
                    return;
                }
                if let Some(b) = &mut self.bench
                    && cur == Some(id)
                {
                    b.overview_ms.push(("open".into(), ms));
                    b.stage("file_opened", at);
                    b.stage("file_opened_received", specular_core::ms_since_start());
                }
                let prio = if cur == Some(id) || self.presenter_current() == Some(id) {
                    Priority::Immediate
                } else {
                    Priority::Prefetch
                };
                self.request_levels(id, prio);
                self.presenter_uploaded(id);
                if cur == Some(id) {
                    self.setup_view();
                    self.request_metadata();
                    self.update_title();
                    self.report_opened();
                    self.events_shown();
                    if !self.prefetch_held {
                        self.page_neighbours(true, &mut HashSet::new());
                    }
                }
                self.redraw = true;
            }
            Event::Overview {
                id,
                kind,
                raster,
                ms,
                at,
                stage,
            } => {
                let Some(img) = self.images.get_mut(&id) else {
                    return;
                };
                img.truncated |= raster.truncated;
                if kind.rank() < img.rank() {
                    if let Some(s) = stage {
                        img.track.decode = Some((s, Instant::now()));
                    }
                    img.overview = Some((kind, raster));
                    self.set_state(id, kind_name(kind));
                }
                if cur == Some(id) {
                    self.hud.decode_ms = ms;
                    if let Some(b) = &mut self.bench {
                        let label = match kind {
                            OverviewKind::Thumbnail => "thumbnail".to_owned(),
                            OverviewKind::Level(s) => format!("level_1_{s}"),
                        };
                        b.stage(&format!("{label}_decoded"), at);
                        b.stage(
                            &format!("{label}_received"),
                            specular_core::ms_since_start(),
                        );
                        b.overview_ms.push((label, ms));
                    }
                }
                if cur == Some(id) || self.in_window(id) {
                    self.upload_or_defer(id);
                }
            }
            Event::Tiles {
                id,
                level,
                tiles,
                raster,
                ms,
            } => {
                self.tile_jobs
                    .retain(|j| !(j.id == id && j.level == level && j.tiles == tiles));
                // The similar-shots review's zoomed images are not the viewer's current one.
                let review = self.review_tiles_done(id, level, &tiles);
                if (cur == Some(id) || review)
                    && let (Some(gfx), Some(g)) = (self.gfx.as_mut(), self.gpu_images.get(id))
                {
                    gfx.renderer
                        .upload_tiles(&gfx.gpu, g, &raster, level, &tiles);
                    trace!(target: TILES, "img={id} L{level} upload {} tiles ({ms:.1} ms decode, {:.1} ms upload)", tiles.len(), gfx.renderer.last_upload_ms);
                    self.hud.upload_ms = gfx.renderer.last_upload_ms;
                    self.hud.decode_ms = ms;
                    self.redraw = true;
                    self.present_now = true;
                    let up = gfx.renderer.last_upload_ms;
                    self.loupe_bench_tiles(level, ms, tiles.len(), up);
                }
            }
            Event::TilesFailed { id, level, tiles } => {
                debug!(target: TILES, "img={id} L{level} {} tiles failed", tiles.len());
                self.tile_jobs
                    .retain(|j| !(j.id == id && j.level == level && j.tiles == tiles));
                self.review_tiles_done(id, level, &tiles);
                for (tx, ty) in tiles {
                    self.tiles_failed.insert((id, level, tx, ty));
                }
            }
            Event::MemoryShort {
                id,
                need,
                available,
            } => {
                let Some(img) = self.images.get_mut(&id) else {
                    return;
                };
                img.track.memory_short = true;
                specular_log::warn!(target: DECODE, "img={id} {} likely needs {} MiB, {} MiB available", img.name(), need >> 20, available >> 20);
                // What other open images keep only to speed up their later decodes goes first.
                self.trim_sessions(false, Some(id));
                // Prefetches stay quiet: the warning is about what the user asked to see.
                if cur == Some(id) {
                    let l = I18n::global().localizer();
                    let size = |b| l.bytes(b, specular_i18n::ByteUnits::Binary);
                    let now = self.now();
                    self.chrome.warning(
                        t!(
                            "toast-low-memory",
                            need = size(need),
                            available = size(available)
                        ),
                        now,
                    );
                    self.redraw = true;
                }
            }
            Event::BackgroundFailed { task, error } => {
                // A panic in a background task: whatever waited for its reply stops waiting,
                // and the user hears that it did not finish (the log has the panic).
                specular_log::warn!(target: specular_log::JOBS, "{task}: {error}");
                self.fileops_lost(&task);
                self.convert_lost(&task);
                self.recipes_lost(&task);
                self.print_lost(&task);
                self.series_lost(&task);
                self.anon_lost(&task);
                self.figure_lost(&task);
                self.markup_lost(&task);
                self.sheets_lost(&task);
                self.medical_lost(&task);
                self.import_lost(&task);
                self.map_tools_lost(&task);
                self.gallery_lost(&task);
                self.zynk_lost(&task);
                let studies = self.studies_lost(&task);
                // File dialogs and desktop hand-offs fail quietly: nothing was started. The
                // studies browser shows its own error, and its medium detection and
                // thumbnails were never asked for.
                if !task.starts_with("dlg-") && !studies {
                    let now = self.now();
                    self.chrome.warning(
                        t!("background-failed", reason = t!("error-reason-internal")),
                        now,
                    );
                }
                self.redraw = true;
            }
            Event::Failed { id, error } => {
                if let Some(img) = self.images.get_mut(&id) {
                    specular_log::warn!(target: DECODE, "img={id} {}: {error}", img.path.display());
                    img.opening = false;
                    // A failed sharper level keeps what is on screen.
                    if img.overview.is_none() {
                        let state = format!("failed: {error}");
                        img.error = Some(error);
                        self.set_state(id, state);
                    }
                }
                self.raw_volume_failed(id);
                self.file_kind_failed(id);
                self.redraw = true;
            }
            Event::Unfinished {
                id,
                token,
                job,
                cancelled,
            } => self.on_unfinished(id, &token, job, cancelled),
            Event::Folder { dir, files } => {
                if let Some((dir, files)) = self.series_hold_listing(dir, files) {
                    self.folder_listed(dir, files);
                }
            }
            Event::DicomSeries {
                file,
                scan,
                slides,
                ms,
            } => self.on_dicom_series(file, scan, slides, ms),
            Event::DicomTags { file, tree, report } => self.on_dicom_tags(file, tree, report),
            Event::ReportJump { file, found } => self.on_report_jump(file, found),
            Event::Waveform { file, waves } => self.on_waveform(file, waves),
            Event::Oct {
                file,
                oct,
                localiser,
            } => self.on_oct(file, oct, localiser),
            Event::GeoOverlay { id, overlay } => self.on_geo_overlay(id, overlay),
            Event::BasemapTile {
                generation,
                key,
                tile,
            } => {
                self.on_basemap_tile(generation, key, tile);
            }
            Event::StudiesDetected { generation, found } => {
                self.on_studies_detected(generation, found);
            }
            Event::Studies { generation, update } => self.on_studies(generation, update),
            Event::StudyThumb {
                generation,
                id,
                thumb,
            } => self.on_study_thumb(generation, id, thumb),
            Event::StudySeries {
                generation,
                id,
                scan,
            } => self.on_study_series(generation, id, scan),
            Event::StudiesMedium { mount } => self.on_studies_medium(mount),
            Event::ComicInfo { book } => self.comic_info_read(book),
            Event::MedicalValues { id, index, values } => self.medical_values(id, index, values),
            Event::MedicalPresentation { id, path, ps } => self.medical_presentation(id, path, ps),
            Event::ScienceValues { id, planes } => self.science_values(id, planes),
            Event::ZarrOpened { address, result } => self.zarr_opened(address, result),
            Event::GainMap { id, map } => {
                let Some(img) = self.images.get_mut(&id) else {
                    return;
                };
                debug!(target: GPU, "img={id} gain map {}×{}, alternate headroom 2^{:.2}", map.map.width, map.map.height, map.params.alt_headroom);
                // A playing motion clip has no gain map; the still gets it back afterwards.
                let playing = self.motion.is_some_and(|m| m.0 == id);
                if let (Some(gfx), Some(g)) = (self.gfx.as_mut(), self.gpu_images.get(id))
                    && !playing
                {
                    gfx.renderer.set_gain_map(&gfx.gpu, g, Some(&map));
                    if cur == Some(id) {
                        self.scene = None;
                        self.redraw = true;
                    }
                }
                if let Some(r) = &img.metadata {
                    img.metadata = Some(Arc::new(with_gain_map(r, &map)));
                    self.redraw = true;
                }
                img.gain_map = Some(map);
            }
            Event::Motion { id, info } => {
                if let Some(img) = self.images.get_mut(&id) {
                    img.motion = Some(info);
                }
                self.redraw = true;
            }
            Event::MotionClip { id, clip, ms } => self.on_motion_clip(id, clip, ms),
            Event::Metadata { id, report } => {
                if let Some(img) = self.images.get_mut(&id) {
                    img.metadata = Some(match &img.gain_map {
                        Some(gm) => Arc::new(with_gain_map(&report, gm)),
                        None => report,
                    });
                }
                self.series_patch(id);
                self.presenter_uploaded(id);
                self.page_patch(id);
                self.video_patch(id);
                self.redraw = true;
            }
            Event::ShowListing { dir, files } => self.present_show_listed(dir, files),
            Event::Accessibility(e) => {
                self.a11y_events.push(e);
                self.redraw = true;
            }
            Event::Picked(p) => {
                if let Some(p) = p {
                    self.open_target(p);
                }
            }
            Event::Trashed { path, result } => self.on_trashed(path, result),
            Event::Restored { path, result } => {
                let now = self.now();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                match result {
                    Ok(()) => {
                        let at = self
                            .trash_undo
                            .iter()
                            .find(|(_, (p, _))| *p == path)
                            .map(|(k, (_, i))| (*k, *i));
                        if let Some((k, _)) = at {
                            self.trash_undo.remove(&k);
                        }
                        let i = at.map_or(0, |a| a.1).min(self.entries.len());
                        let mut files = self.entries.clone();
                        files.insert(i, path);
                        self.set_entries(files);
                        self.go_to(i);
                        self.chrome
                            .toast(t!("toast-restored", file = name), None, now);
                    }
                    Err(e) => {
                        self.chrome.toast(
                            t!("app-restore-failed", file = name, reason = e),
                            None,
                            now,
                        );
                    }
                }
                self.redraw = true;
            }
            Event::Copied { what, result } => {
                let now = self.now();
                let msg = match result {
                    Ok(()) => match what {
                        Copied::Image => t!("toast-copied-image"),
                        Copied::Path => t!("toast-copied-path"),
                        Copied::Report => t!("toast-copied-report"),
                        Copied::Value => t!("toast-copied-value"),
                        Copied::Editor => return,
                        Copied::Measurements => t!("toast-copied-measurements"),
                        Copied::Coordinate => t!("toast-copied-coordinate"),
                        Copied::KeyImages => t!("figure-keys-copied"),
                        Copied::ViewCommand => t!("toast-copied-view-command"),
                    },
                    Err(e) => t!("app-copy-failed", reason = e),
                };
                self.chrome.toast(msg, None, now);
                self.redraw = true;
            }
        }
    }

    /// A job of image `id` ended without a result. If it ran with the image's live token,
    /// its work is no longer in flight: ask again while the image is wanted, or show an error
    /// when the decoder gave up on its own. A load that cannot progress ends in an error
    /// card, never in a permanent spinner.
    fn on_unfinished(
        &mut self,
        id: ImageId,
        token: &CancelToken,
        job: Unfinished,
        cancelled: bool,
    ) {
        let cur = self.current_id();
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        if !img.unfinished(token, job) {
            return;
        }
        if !cancelled
            && img.overview.is_none()
            && img.error.is_none()
            && !img.opening
            && img.levels.is_empty()
        {
            img.error = Some(t!("error-decoder-stopped"));
            self.set_state(
                id,
                format!("failed: the {job:?} job ended without a result"),
            );
            self.redraw = true;
            return;
        }
        let wanted = cur == Some(id) || self.in_window(id);
        debug!(target: DECODE, "img={id} {job:?} job ended without a result (cancelled: {cancelled}); requesting again: {wanted}");
        if wanted {
            let prio = if cur == Some(id) {
                Priority::Immediate
            } else {
                Priority::Prefetch
            };
            self.ensure_open(id, prio);
            self.request_levels(id, prio);
            self.redraw = true;
        }
    }

    fn in_window(&self, id: ImageId) -> bool {
        if self.presenter_wants(id) || self.oct_localiser() == Some(id) {
            return true;
        }
        let (Some(cur), Some(img)) = (self.current, self.images.get(&id)) else {
            return false;
        };
        let depth = specular_platform::prefetch_depth(
            self.settings.performance.prefetch_depth.max(0) as usize,
        );
        self.index_of(&img.path).is_some_and(|i| {
            let d = i.abs_diff(cur);
            let n = self.entries.len();
            let d = if self.settings.browsing.loop_folder {
                d.min(n - d)
            } else {
                d
            };
            d <= depth
        })
    }

    fn on_trashed(&mut self, path: PathBuf, result: Result<(), String>) {
        let now = self.now();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match result {
            Ok(()) => {
                let Some(idx) = self.index_of(&path) else {
                    return;
                };
                let mut files = self.entries.clone();
                files.remove(idx);
                if let Some(id) = self.by_path.remove(&path) {
                    if let Some(img) = self.images.remove(&id) {
                        img.token.cancel();
                    }
                    self.gpu_images.remove(id);
                    if let Some(g) = self.gfx.as_mut() {
                        g.renderer.release_image(id);
                    }
                }
                let was_current = self.current == Some(idx);
                self.set_entries(files);
                let undo = self.next_undo;
                self.next_undo += 1;
                self.trash_undo.insert(undo, (path, idx));
                self.last_undo = Some(undo);
                if self.entries.is_empty() {
                    self.current = None;
                    self.shown = None;
                } else if was_current {
                    let i = if self.nav_dir < 0 {
                        idx.saturating_sub(1)
                    } else {
                        idx.min(self.entries.len() - 1)
                    };
                    self.shown = None;
                    self.go_to(i);
                } else if let Some(c) = self.current {
                    self.current = Some(if c > idx { c - 1 } else { c });
                }
                self.chrome.toast(
                    t!("toast-moved-to-trash-named", file = name),
                    Some(undo),
                    now,
                );
            }
            Err(e) => {
                self.chrome
                    .toast(t!("error-trash-failed", file = name, reason = e), None, now);
            }
        }
        self.update_title();
        self.redraw = true;
    }

    fn undo(&mut self, undo: Option<u64>) {
        let Some(u) = undo.or(self.last_undo) else {
            return;
        };
        if self.fileops_undo(u)
            || self.gallery_undo(u)
            || self.cull_undo(u)
            || self.video_export_reveal(u)
            || self.develop_merge_toast_action(u)
            || self.player_toast_action(u)
            || self.studies_offer_taken(u)
            || self.dicomweb_stop_key(u)
            || self.ai_notice(u)
            || self.profile_offer_taken(u)
            || self.hint_turn_off(u)
            || self.import_offer_taken(u)
            || self.mpr_curve_undo(u)
            || self.measurements_cleared_undo(u)
            || self.astro_toast_action(u)
        {
            return;
        }
        if let Some((path, _)) = self.trash_undo.get(&u).cloned() {
            self.chrome.dismiss_undo(u);
            if self.last_undo == Some(u) {
                self.last_undo = None;
            }
            clipboard::restore(&self.loader, path);
        }
    }

    // ----- Settings -----------------------------------------------------------------------

    fn apply_settings(&mut self, force: bool) {
        let generation = self.store.generation();
        if generation == self.settings_gen && !force {
            return;
        }
        let changed = if force || self.settings_gen == u64::MAX {
            None
        } else {
            self.store.changes_since(self.settings_gen)
        };
        self.settings_gen = generation;
        let s = self.store.snapshot();
        debug!(target: SETTINGS, "apply generation {generation}: {}", changed.as_ref().map_or("everything".to_owned(), |c| format!("{c:?}")));
        self.watchdog
            .set_threshold(stall_threshold(s.performance.stall_warning));
        let has = |k: &str| changed.as_ref().is_none_or(|c| c.contains(&k));
        let i18n = I18n::global();
        if has("general.language") && i18n.preference() != s.general.language {
            i18n.set_preference(s.general.language.clone());
        }
        if has("general.region_formats") {
            i18n.set_region_formats(s.general.region_formats.locale());
        }
        self.chrome.text_scale = s.accessibility.text_scale as f32;
        self.chrome.reduce_motion = s.accessibility.reduce_motion == SystemOnOff::On;
        self.ui_scale = s.accessibility.ui_scale as f32;
        if has("performance.frame_time_hud") {
            self.chrome.set_hud(s.performance.frame_time_hud);
        }
        performance::apply(&s, self.gfx.as_mut().map(|g| &mut g.renderer));
        if has("library.skip_folders") || has("library.skip_marked") {
            self.gallery.library_set_skip(gallery::library_skip_of(&s));
        }
        if has("library.show_timeline") {
            self.gallery.library_show_timeline(s.library.show_timeline);
        }
        if has("library.prefer_sidecars_in") {
            self.gallery_set_prefer_sidecars(&s.library.prefer_sidecars_in);
        }
        if [
            "braw",
            "braw_path",
            "dwg",
            "dwg_path",
            "occt",
            "occt_path",
            "czi",
            "czi_path",
        ]
        .iter()
        .any(|k| has(&format!("decoders.{k}")))
        {
            self.decoders_configure(&s);
        }
        if has("devices.enabled") || has("devices.dead_zone") || has("devices.speed") {
            self.devices_configure(&s);
        }
        if has("drawings.shx_folders") {
            // SHX fonts for drawings opened from now on (the drawing's own folder first).
            use specular_decode::drawing::specular_vector::shx;
            shx::set_folders(shx::parse_folders(&s.drawings.shx_folders));
        }
        if changed.is_some()
            && (has("performance.display_cache")
                || has("performance.display_cache_size")
                || has("performance.display_cache_location"))
        {
            let headless = self.args.bench_open.is_some() || self.args.screenshot.is_some();
            self.loader
                .set_display_cache(display_cache_for(&s, headless));
        }
        let depth_changed =
            self.settings.performance.prefetch_depth != s.performance.prefetch_depth;
        let sort_changed = self.settings.browsing.sort_order != s.browsing.sort_order
            || self.settings.browsing.sort_descending != s.browsing.sort_descending
            || self.settings.browsing.include_subfolders != s.browsing.include_subfolders;
        let show_subfolders_changed =
            self.settings.slideshow.include_subfolders != s.slideshow.include_subfolders;
        if self.settings.ai != s.ai {
            let old = self.settings.clone();
            self.ai_settings_changed(&old, &s);
        }
        self.settings = s;
        self.privacy_apply();
        if has("general.theme")
            && let Some(g) = &self.gfx
        {
            g.window
                .set_theme(window_theme(self.settings.general.theme));
        }
        self.apply_colour_policy();
        if self.gallery.started() {
            self.gallery_sync_settings();
        }
        if depth_changed {
            self.schedule();
        }
        // A changed GPS setting gives a new metadata request key.
        self.request_metadata();
        // A running slideshow keeps its own listing (with subfolders), listed in the new order.
        let show_relisted = (sort_changed || show_subfolders_changed) && self.slideshow_relist();
        if sort_changed
            && !show_relisted
            && let Some(dir) = self.folder.clone()
        {
            // Re-list in the new order; the current image stays current.
            self.scan_folder(dir);
        }
        self.scene = None;
        self.refresh_insights();
        // Update checks just turned on: the first one now (then once a day).
        self.update_check(false);
        self.redraw = true;
    }

    /// HDR output (setting `colour.hdr_output`): an scRGB swapchain when the display can show
    /// HDR (auto) or whenever the surface offers one (on), with the display's headroom as the
    /// renderer's peak; otherwise SDR, where HDR content is tone-mapped to SDR white. Checked
    /// twice a second, as the headroom follows brightness and the display the window is on.
    fn update_hdr_output(&mut self, force: bool) {
        if !force
            && self
                .hdr_checked
                .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
        {
            return;
        }
        self.hdr_checked = Some(Instant::now());
        let Some(g) = self.gfx.as_mut() else { return };
        let d = g.surface.hdr_display(&g.gpu);
        // Screenshots read back 8-bit pixels.
        let want = self.args.screenshot.is_none()
            && match self.settings.colour.hdr_output {
                HdrOutput::Off => false,
                HdrOutput::On => true,
                HdrOutput::Auto => d.capable,
            };
        let was = g.surface.hdr;
        let hdr = g.surface.set_hdr(&g.gpu, want);
        let peak = if hdr {
            d.headroom.unwrap_or(HDR_FALLBACK_HEADROOM).clamp(1.0, 64.0)
        } else {
            1.0
        };
        if hdr != was || (peak - g.renderer.display_peak()).abs() > 0.01 {
            let format = g.surface.format();
            g.comp.set_output(&g.gpu, format, hdr.then_some(peak));
            g.renderer.set_display_peak(peak);
            if hdr != was {
                info!(target: GPU, "HDR output {}: {format:?}, peak {peak:.2}× SDR white (display: {d:?})", if hdr { "on" } else { "off" });
            }
            self.scene = None;
            self.redraw = true;
        }
    }

    fn on_language_changed(&mut self) {
        let i18n = I18n::global();
        if let Some(g) = &mut self.gfx {
            g.text.set_locale(Some(i18n.language().to_string()));
        }
        if let Some((t, v)) = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.track.done)
        {
            self.chrome
                .set_open_report(Some(open_report::panel(&t, &v)));
        }
        self.request_metadata();
        self.print_language_changed();
        self.redraw = true;
    }

    fn palette(&self) -> Palette {
        let dark = match self.settings.general.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => self
                .gfx
                .as_ref()
                .and_then(|g| g.window.theme())
                .is_none_or(|t| t == winit::window::Theme::Dark),
        };
        if dark {
            Palette::dark()
        } else {
            Palette::light()
        }
    }

    fn canvas(&self, pal: &Palette) -> Color {
        let v = &self.settings.viewing;
        // Full-screen play mode: a movie letterboxed on black.
        if self.video_full.active() {
            return Color::BLACK;
        }
        if self.fullscreen {
            return settings_color(v.fullscreen_background);
        }
        match v.background {
            Background::Theme => pal.canvas,
            Background::Black => Color::BLACK,
            Background::DarkGrey => Color::hex(0x202020),
            Background::LightGrey => Color::hex(0xc8c8c8),
            Background::White => Color::WHITE,
            Background::Custom => settings_color(v.background_color),
        }
    }

    /// What shows through transparent pixels (`viewing.transparency`), in the viewer over
    /// `canvas`. The gallery passes `None` for the canvas: there "background" means its own
    /// tiles show through.
    fn transparency(&self, pal: &Palette, canvas: Option<Color>) -> Backdrop {
        match self.settings.viewing.transparency {
            Transparency::Checkerboard => Backdrop::Checker {
                a: pal.checker_a,
                b: pal.checker_b,
                cell: CHECKER_CELL,
            },
            Transparency::Background => canvas.map_or(Backdrop::None, Backdrop::Solid),
            Transparency::Black => Backdrop::Solid(Color::BLACK),
            Transparency::White => Backdrop::Solid(Color::WHITE),
        }
    }

    fn reduce_motion(&self) -> bool {
        self.chrome.reduce_motion || !self.settings.viewing.zoom_animation
    }

    // ----- Actions ------------------------------------------------------------------------

    fn zoom_steps(&self, up: bool) -> f32 {
        // Next "nice" zoom level.
        const STEPS: [f32; 25] = [
            0.03125, 0.0625, 0.083, 0.125, 0.167, 0.25, 0.333, 0.5, 0.667, 0.75, 1.0, 1.5, 2.0,
            3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0, 64.0, 128.0, 256.0, 512.0,
        ];
        let s = self.view.target_scale();
        let fit = self.view.fit_scale();
        let max = self.view.max_scale();
        let mut levels: Vec<f32> = STEPS.iter().copied().filter(|&l| l <= max).collect();
        levels.push(fit);
        levels.sort_by(f32::total_cmp);
        if up {
            levels.into_iter().find(|&l| l > s * 1.01).unwrap_or(s)
        } else {
            levels
                .into_iter()
                .rev()
                .find(|&l| l < s * 0.99)
                .unwrap_or(s)
        }
    }

    fn after_view_change(&mut self) {
        if self.reduce_motion() {
            self.view.settle();
        }
        debug!(target: VIEW, "view changed (revision {})", self.view.revision);
        self.redraw = true;
    }

    fn set_orientation(&mut self, f: impl Fn(Orientation) -> Orientation) {
        // A movie's turn is kept for its file (player/picture.rs).
        if self.player_turn(&f) {
            return;
        }
        let Some(id) = self.current_id() else { return };
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        let Some(o) = img.orient else { return };
        let n = f(o);
        img.orient = Some(n);
        self.view.set_orientation(n);
        self.after_view_change();
    }

    /// The zoom presets' Full screen: a movie's play mode, else the window full screen with
    /// the image fitted.
    fn enter_fullscreen_fitted(&mut self) {
        if self.video_full_enter() || self.video_full.active() {
            return;
        }
        if !self.fullscreen {
            self.set_fullscreen(true);
        }
        if Action::ZoomFit.enabled(&self.action_state()) {
            self.view.zoom_fit();
            self.after_view_change();
        }
    }

    fn toggle_fullscreen(&mut self) {
        let Some(g) = &self.gfx else { return };
        let on = g.window.fullscreen().is_none();
        self.set_fullscreen(on);
    }

    /// Into or out of fullscreen. Back leaves it with this, never with a toggle: when the
    /// window manager took the window out of fullscreen behind our back, a toggle would put
    /// it back in.
    fn set_fullscreen(&mut self, on: bool) {
        let Some(g) = &self.gfx else { return };
        g.window
            .set_fullscreen(on.then_some(Fullscreen::Borderless(None)));
        self.fullscreen = on;
        if on {
            let now = self.now();
            self.chrome.entered_fullscreen(now);
        }
        self.scene = None;
        self.redraw = true;
    }

    fn action(&mut self, a: Action, el: &ActiveEventLoop) {
        // Esc leaves a movie's full-screen play mode (or the mini player) before anything
        // else.
        if a == Action::Back && self.video_full_leave() {
            return;
        }
        if a == Action::Back && self.player.mini.active() {
            if !self.chrome.back() {
                self.mini_leave();
            }
            self.redraw = true;
            return;
        }
        // Quick look's channel views: enabled by the image in quick look, not the viewer's.
        if self.gallery_channel(a) {
            return;
        }
        // On a slide whose stains are separated, the channel keys show or hide stains.
        if self.stain_channel_key(a) {
            return;
        }
        // An action of a capability that is off is hidden: nothing runs.
        if self.profile_hides(a) {
            return;
        }
        // The gallery's file actions work on its selection, not the viewer's image.
        if self.fileops_gallery_action(a) {
            return;
        }
        // A disabled action is a silent no-op: no beep, no toast (DESIGN.md "Enabled state").
        let st = self.action_state();
        if let Some(why) = a.disabled(&st) {
            debug!(target: INPUT, "action {} disabled: {why:?}", a.id());
            return;
        }
        debug!(target: INPUT, "action {}", a.id());
        if self.chrome.handle_action(a) {
            self.redraw = true;
            return;
        }
        if a == Action::Print {
            self.print_open();
            return;
        }
        if a == Action::SaveDiagnostics {
            self.diag_open();
            return;
        }
        if self.open_browser_link(a) {
            return;
        }
        if self.stain_action(a) {
            return;
        }
        // The gallery's compare (and "Compare with…" from the viewer).
        if self.compare_action(a) {
            return;
        }
        match a {
            Action::ExportAnnotations => return self.annot_export_open(),
            Action::ClientGallery => return self.web_gallery_open(),
            Action::ImportClientPicks => return self.client_picks_open(),
            Action::ImportAnnotations => return self.pathology_import(),
            Action::LabelVolume => return self.label_volume_open(),
            Action::ExportAnonymised => return self.anon_open(anon_export::Kind::Copy),
            Action::ExportPatientDisc => return self.anon_open(anon_export::Kind::Disc),
            Action::MaskBurnedInText => return self.anon_mask_open(),
            Action::ExportFigure => return self.figure_open(figure::Kind::Figure, false),
            Action::ToggleKeyImage => return self.figure_toggle_key(),
            Action::CopyKeyImages => return self.figure_copy_keys(),
            Action::ExportKeyImages => return self.figure_open(figure::Kind::Keys, false),
            Action::ExportTeachingFile => return self.figure_open(figure::Kind::Keys, true),
            Action::TeachingQuiz => return self.quiz_start(),
            Action::SendToPacs => return self.send_to_pacs(),
            _ => {}
        }
        if self.measure_action(a)
            || self.markup_action(a)
            || self.sheets_action(a)
            || self.script_action(a)
        {
            return;
        }
        if self.cursor_action(a) {
            return;
        }
        if a == Action::Convert {
            self.convert_open();
            return;
        }
        if a == Action::ExportRecipes {
            self.recipes_open();
            return;
        }
        if a == Action::Import {
            self.import_open(None);
            return;
        }
        if self.fileops_action(a) {
            return;
        }
        if a == Action::ExportSlideshow {
            self.video_export_open();
            return;
        }
        if a == Action::DicomServers {
            self.dicomweb_open();
            return;
        }
        if a == Action::OpenAddress {
            self.address_open();
            return;
        }
        if a == Action::OpenStream {
            self.address_open_stream();
            return;
        }
        if a == Action::StreamQuality {
            self.stream_quality();
            return;
        }
        if a == Action::OmeroServers {
            self.omero_open();
            return;
        }
        if a == Action::Keywords {
            self.keywords_toggle();
            return;
        }
        if a == Action::EditMetadata {
            self.meta_toggle();
            return;
        }
        if self.develop_action(a)
            || self.cull_action(a)
            || self.linked_action(a)
            || self.mpr_action(a)
            || self.mesh_action(a)
            || self.relight_action(a)
            || self.medical_action(a)
            || self.science_action(a)
            || self.gis_action(a)
            || self.cad_action(a)
            || self.contents_action(a)
            || self.tether_action(a)
            || self.gallery_action(a)
            || self.present_action(a, el)
            || self.slideshow_action(a)
        {
            self.redraw = true;
            return;
        }
        let now = self.now();
        self.chrome.poke(now);
        let vp = self.view.viewport;
        match a {
            Action::NextImage => self.navigate(1),
            Action::PrevImage => self.navigate(-1),
            Action::FirstImage => {
                self.nav_dir = 1;
                self.go_to(0)
            }
            Action::LastImage => {
                self.nav_dir = -1;
                self.go_to(self.entries.len().saturating_sub(1))
            }
            Action::ZoomIn | Action::ZoomOut => {
                let s = self.zoom_steps(a == Action::ZoomIn);
                self.view.zoom_to(s, None);
                self.after_view_change();
            }
            Action::ZoomFit => {
                self.set_reading_fit(None);
                self.view.zoom_fit();
                self.after_view_change();
            }
            Action::ZoomFill => {
                self.set_reading_fit(None);
                self.view.zoom_fill();
                self.after_view_change();
            }
            Action::ZoomFitWidth | Action::ZoomFitHeight => {
                let width = a == Action::ZoomFitWidth;
                self.set_reading_fit(Some(width));
                self.zoom_fit_reading_side(width, false);
                self.after_view_change();
            }
            Action::ZoomTo(p) => {
                self.view.zoom_to(p / 100.0, None);
                self.after_view_change();
            }
            Action::RotateLeft => self.set_orientation(|o| o.rotated(-1)),
            Action::RotateRight => self.set_orientation(|o| o.rotated(1)),
            Action::FlipHorizontal => self.set_orientation(|o| o.flipped_h()),
            Action::FlipVertical => self.set_orientation(|o| o.flipped_h().rotated(2)),
            Action::PanLeft => self.pan(-vp.w * 0.15, 0.0),
            Action::PanRight => self.pan(vp.w * 0.15, 0.0),
            Action::PanUp => self.pan(0.0, -vp.h * 0.15),
            Action::PanDown => self.pan(0.0, vp.h * 0.15),
            Action::PanPageLeft => self.pan(-vp.w * 0.9, 0.0),
            Action::PanPageRight => self.pan(vp.w * 0.9, 0.0),
            Action::PanPageUp => self.pan(0.0, -vp.h * 0.9),
            Action::PanPageDown => self.pan(0.0, vp.h * 0.9),
            // On a movie (with a picture) full screen is its play mode.
            Action::ToggleFullscreen
                if self.video_full.active() || self.video_picture_current() =>
            {
                self.video_full_toggle()
            }
            Action::ToggleFullscreen => self.toggle_fullscreen(),
            Action::VideoFullscreen => self.video_full_toggle(),
            Action::Back => {
                // One layer per press: overlays, then zoom/pan, then fullscreen.
                let closed = self.chrome.back()
                    || self.cancel_held_tool()
                    || self.linked_back()
                    || self.mpr_back()
                    || self.medical_back()
                    || self.stop_motion();
                // With no image there is no zoom to reset.
                let no_view =
                    !Action::ZoomFit.enabled(&self.action_state()) || self.shown.is_none();
                match back_step(closed, no_view || self.view.is_at_fit(), self.fullscreen) {
                    BackStep::Closed => {}
                    // The end of the ladder: back to the gallery it was opened from.
                    BackStep::Nothing => {
                        self.gallery_back_from_viewer();
                    }
                    BackStep::Fit => {
                        self.view.zoom_fit();
                        self.after_view_change();
                    }
                    BackStep::ExitFullscreen => self.set_fullscreen(false),
                }
            }
            Action::TogglePixelGrid => {
                self.pixel_grid = !self.pixel_grid;
                self.scene = None;
            }
            Action::CopyImage => {
                if let Some(id) = self.current_id()
                    && let Some(img) = self.images.get(&id)
                    && let (Some(s), Some(o)) = (img.session.clone(), img.orient)
                {
                    self.clipboard.copy_image(&self.loader, s, o);
                }
            }
            Action::ReadCodes => self.codes_read(),
            Action::FindText => self.find_text(),
            Action::FaceBoxes => self.toggle_face_boxes(),
            Action::CopyPath => {
                if let Some(p) = self.current.and_then(|i| self.entries.get(i)) {
                    self.clipboard.copy_path(&self.loader, p);
                }
            }
            Action::MoveToTrash => {
                if let Some(p) = self.current.and_then(|i| self.entries.get(i)).cloned() {
                    if self.settings.browsing.confirm_trash {
                        let name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        // The sidecars that go with it are named (Develop's edits among them).
                        let sidecars: Vec<String> = specular_fileops::names::sidecars(&p, false)
                            .iter()
                            .filter_map(|s| s.file_name())
                            .map(|n| n.to_string_lossy().into_owned())
                            .collect();
                        let detail = (!sidecars.is_empty())
                            .then(|| t!("app-confirm-trash-sidecars", files = sidecars.join(", ")));
                        self.chrome.confirm_trash_with(name, detail);
                    } else {
                        clipboard::trash(&self.loader, p);
                    }
                }
            }
            Action::ToggleNavigator => {
                self.navigator = !self.navigator;
                self.nav_drag = false;
            }
            // Held: the loupe shows while the key is down, the zoom rectangle takes the
            // next drag (see `on_mouse_button`).
            Action::Loupe | Action::ZoomRectangle => {}
            Action::Undo => self.undo(None),
            Action::ShowWelcome => self.welcome_show(),
            Action::Open => {
                let title = t!("app-open-dialog-title");
                // All supported files (selected), each kind of file, all files.
                let list = open_dialog::entries(&self.loader.registry());
                let dir = self
                    .current
                    .and_then(|i| self.entries.get(i))
                    .and_then(|p| p.parent())
                    .map(Path::to_path_buf);
                self.loader.background("dlg-open", move |l| {
                    l.post(Event::Picked(open_dialog::pick(
                        &title,
                        &list,
                        dir.as_deref(),
                    )));
                });
            }
            Action::ToggleTheme => {
                let dark = self.palette().mode == specular_theme::Mode::Dark;
                let _ = self.store.set_text(
                    "general.theme",
                    if dark { "light" } else { "dark" },
                    Origin::Program,
                );
            }
            Action::InterfaceLarger | Action::InterfaceSmaller | Action::InterfaceReset => {
                // Steps of 10 %, within the setting's range; the toast says where it is.
                let cur = self.settings.accessibility.ui_scale;
                let next = match a {
                    Action::InterfaceLarger => ((cur * 10.0).round() + 1.0) / 10.0,
                    Action::InterfaceSmaller => ((cur * 10.0).round() - 1.0) / 10.0,
                    _ => 1.0,
                }
                .clamp(0.5, 3.0);
                let _ = self.store.set(
                    "accessibility.ui_scale",
                    specular_settings::Value::Float(next),
                    Origin::Program,
                );
                let percent = (next * 100.0).round() as i64;
                let now = self.now();
                self.chrome
                    .toast(t!("toast-interface-size", percent = percent), None, now);
            }
            Action::Reload => self.reload_current(),
            Action::Close => {
                if !self.welcome_close_requested() {
                    el.exit();
                }
            }
            Action::Quit => el.exit(),
            Action::OpenWith => self.open_with(),
            Action::OpenAs => self.file_kind_open_as(),
            Action::Paste => self.paste(),
            Action::DicomTags => self.dicom_tags_toggle(),
            Action::Studies => self.studies_show(),
            Action::NextSeries => self.series_switch(1),
            Action::PrevSeries => self.series_switch(-1),
            _ => {
                if !self.page_action(a)
                    && !self.playback_action(a)
                    && !self.sound_action(a)
                    && !self.player_action(a)
                    && !self.queue_action(a)
                {
                    self.reading_action(a);
                }
            }
        }
        self.redraw = true;
    }

    fn pan(&mut self, dx: f32, dy: f32) {
        self.view.pan_by(dx, dy, !self.reduce_motion());
        self.redraw = true;
    }

    fn command(&mut self, c: Command, el: &ActiveEventLoop) {
        match c {
            Command::Action(a) => self.action(a, el),
            Command::EnterFullscreen => self.enter_fullscreen_fitted(),
            Command::Undo(u) => self.undo(Some(u)),
            Command::Welcome(c) => self.welcome_command(c),
            Command::DecoderCard { id, button } => self.decoders_card_button(&id, button),
            Command::Capability {
                capability,
                always,
                then,
            } => self.capability_on(capability, always, then, el),
            Command::OpenStudySeries(id) => {
                // Comparing series: it goes to the active viewport.
                self.linked_study_to_active(id);
                self.studies_open_series(id);
            }
            Command::StudiesCancel => self.studies_cancel(),
            Command::StudiesSend(sel) => self.studies_send(sel),
            Command::TagJump(row) => self.report_jump(row),
            Command::PlotAdjust { gain, speed } => self.plot_adjust(gain, speed),
            Command::GoTo(i) => {
                let cur = self.current.unwrap_or(0);
                if i != cur {
                    self.nav_dir = if i > cur { 1 } else { -1 };
                    self.go_to(i);
                }
            }
            Command::SettingAction(key) => self.setting_action(key),
            Command::ChooseSettingPath { key, directory } => {
                self.choose_setting_path(key, directory)
            }
            Command::SettingPanel(k, e) if App::is_ai_panel(k) => self.ai_panel_event(k, &e),
            Command::SettingPanel(k, e) if k == integration::EXTRA_TYPES_UNDER => {
                self.extra_types_event(&e)
            }
            Command::SettingPanel(diagnostics::UPDATE_PANEL, e) => self.update_panel_event(&e),
            Command::SettingPanel(decoders::UNDER, e) => self.decoders_panel_event(&e),
            Command::SettingPanel(file_kind::UNDER, e) => self.file_kind_panel_event(&e),
            Command::SettingPanel(hooks::UNDER, e) => self.hooks_panel_event(&e),
            Command::SettingPanel(gallery::local::PANEL, e) => self.local_panel_event(&e),
            Command::SettingPanel(map::services::PANEL_KEY, e) => self.gis_services_panel_event(&e),
            Command::DiagNames(on) => self.diag_names(on),
            Command::DiagSave { include_names } => self.diag_save(include_names),
            Command::SettingPanel(_, e) => self.paths_panel_event(&e),
            Command::CopyReport => {
                let text = self.chrome.report_text();
                self.clipboard.copy_text(&self.loader, text, Copied::Report);
            }
            Command::Motion => self.toggle_motion(),
            Command::Seek { fraction, done } => self.video_seek(fraction, done),
            Command::Volume { volume, done } => self.video_set_volume(volume, done),
            Command::Player(c) => self.player_command(c),
            Command::ShowOnMap => {
                let cur = self.current.and_then(|i| self.entries.get(i)).cloned();
                self.map_open(cur);
            }
            Command::EditMetadata => self.meta_open(),
            Command::BackToGallery => {
                self.gallery_back_from_viewer();
            }
            Command::PresentingPause => self.present_pause(),
            Command::PresentingStop => self.present_stop(),
            Command::CopyText(text) => self.clipboard.copy_text(&self.loader, text, Copied::Value),
            Command::EditorCopy(text) => {
                self.clipboard.copy_text(&self.loader, text, Copied::Editor);
            }
            Command::EditorPaste => self.paste_text(integration::PasteTo::Editor),
            Command::PrimaryCopy(text) => self.set_primary(text),
            Command::PrimaryPaste => self.paste_primary(integration::PasteTo::Editor),
            Command::ConfirmTrash => {
                if let Some(p) = self.current.and_then(|i| self.entries.get(i)).cloned() {
                    clipboard::trash(&self.loader, p);
                }
            }
            Command::Insight(k) => self.insight_command(&k),
            Command::Item(insights::ITEM) => self.open_insights(),
            Command::Work(k) => self.work_command(&k),
            Command::Item(work::ITEM) => self.open_work(),
            Command::Downloads(k) => self.net_command(&k),
            Command::Queue(c) => self.queue_command(c),
            Command::Item(net::ITEM) => self.open_downloads(),
            Command::Item(remote::CONFIRM_ITEM) => self.url_confirmed(),
            Command::Item(codes::OPEN_ITEM) => self.codes_open_confirmed(),
            Command::Item(zynk_send::GET_ITEM) => self.zynk_get(),
            Command::Item(recipes::CONFIRM_ITEM) => self.recipe_confirmed(),
            Command::Item(items::DEVELOP_MASK_REMOVE) => self.develop_mask_confirmed(),
            Command::Item(items::PEOPLE_MERGE) => self.people_merge_confirmed(),
            Command::Item(mesh::bim::VIEW_DELETE_ITEM) => self.bim_delete_confirmed(),
            Command::Item(mesh::FED_REMOVE_ITEM) => self.fed_remove_confirmed(),
            Command::Item(mesh::FED_ORIGIN_ITEM) => self.fed_origin_confirmed(),
            Command::Item(mpr::CPR_DELETE_ITEM) => self.mpr_curve_delete_confirmed(),
            Command::Item(ai::sideload::UNVERIFIED_ITEM) => self.ai_unverified_confirmed(),
            Command::Item(gallery::LIBRARY_REMOVE_ITEM) => self.library_remove_confirmed(),
            Command::Item(items::ALBUM_DELETE) => self.albums_delete_confirmed(),
            Command::Item(items::LOCAL_KEEP_ORIGINALS) => self.local_keep_confirmed(false),
            Command::Item(items::LOCAL_KEEP_PROXIES) => self.local_keep_confirmed(true),
            Command::Item(items::LOCAL_REMOVE) => self.local_remove_confirmed(),
            Command::Item(items::LOCAL_MINE) => self.local_conflict_answer(true),
            Command::Item(items::LOCAL_THEIRS) => self.local_conflict_answer(false),
            Command::Item(items::LOCAL_EXPORT_PROXY) => self.local_export_answer(true),
            Command::Item(items::LOCAL_EXPORT_WAIT) => self.local_export_answer(false),
            Command::Item(items::ALBUM_REMOVE_PHOTOS) => self.albums_remove_confirmed(),
            Command::Item(gallery::LIBRARY_HOME_PICTURES_ITEM) => self.library_home_answer(true),
            Command::Item(gallery::LIBRARY_HOME_KEEP_ITEM) => self.library_home_answer(false),
            Command::Item(map::services::REMOVE_ITEM) => self.gis_services_remove_confirmed(),
            Command::Item(decoders::FORGET_ITEM) => self.decoders_forget_confirmed(),
            Command::Item(items::INTEGRATION_SEARCH_REMOVE) => {
                self.desktop_search_remove_confirmed()
            }
            Command::Item(file_kind::FORGET_ITEM) => self.file_kind_forget_confirmed(),
            Command::Item(hooks::ALLOW_ITEM) => self.hooks_allowed(),
            Command::Item(hooks::NOT_NOW_ITEM) => self.hooks_declined(),
            Command::Item(hooks::REMOVE_ITEM) => self.hooks_remove_confirmed(),
            Command::Item(map::GEOTAG_ITEM) => self.gis_geotag_confirmed(),
            Command::Item(markup::DELETE_ITEM) => self.markup_delete_confirmed(),
            Command::Item(items::CULL_AUTO_REJECT) => self.cull_auto_reject_confirmed(),
            Command::Item(items::AI_ADDON_REMOVE) => self.ai_addon_remove_confirmed(),
            Command::Item(items::CULL_MOVE_REJECTS) => self.cull_move_rejects_confirmed(),
            Command::Item(items::EQ_PRESET_DELETE) => self.eq_delete_confirmed(),
            Command::Item(items::STREAM_FORGET) => self.stream_forget_confirmed(),
            Command::Item(id) => {
                if !self.gallery_item(id)
                    && !self.develop_item(id)
                    && !self.develop_aids_item(id)
                    && !self.welcome_item(id)
                    && !self.codes_item(id)
                    && !self.find_text_item(id)
                    && !self.player_item(id)
                    && !self.queue_item(id)
                {
                    self.integration_item(id);
                }
            }
            // Previous or Next held on the toolbar repeats as a held arrow key does.
            Command::NavHold { next, held } => {
                let step = self.hold_step(held);
                self.flipping = true;
                self.navigate(if next { step } else { -step });
            }
            Command::NavHoldEnd => self.end_flipping(),
            Command::Keymap(e) => self.keys_edit(&e),
            Command::KeymapImport => self.keys_import(),
            Command::KeymapExport(m) => self.keys_export(m),
            Command::KeymapSave(text) => self.keys_save_text(&text),
        }
    }

    /// The hardware report: the facts `--hw-report` prints (without waiting for the hardware
    /// decoder probe) and what the app chose from them.
    fn hw_report(&self) -> specular_widgets::HwReport {
        use crate::hw_report::{Facts, panel};
        let gpu = self.gfx.as_ref().map(|g| &g.gpu).ok_or_else(String::new);
        let facts = Facts::gather(self.hw.clone(), gpu, specular_hwdec::probed());
        let ram_cache = match self.settings.performance.ram_cache {
            0 => self.hw.ram_cache_budget(),
            mib => (mib as u64) << 20,
        };
        let vram_cache = match self.settings.performance.vram_cache {
            0 => 768u64 << 20,
            mb => (mb as u64) << 20,
        };
        let app = panel::AppState {
            decode_threads: self.loader.threads(),
            ram_cache,
            vram_cache,
            ram_available: specular_core::available_memory().unwrap_or(self.hw.ram_available),
            insights: self.insights_text(),
        };
        panel::build(&facts, &app)
    }

    fn setting_action(&mut self, key: &str) {
        match key {
            insights::RESTORE => self.insight_command("unhide"),
            "gallery.clear_cache" => self.gallery_clear_cache(),
            diagnostics::UPDATE_PANEL => self.update_check(true),
            decoders::UNDER => self.decoders_look_again(None),
            file_kind::UNDER => self.file_kind_forget_all(),
            devices::UNDER => self.devices_look_again(),
            "advanced.hw_report" => {
                let report = self.hw_report();
                self.chrome.show_report(report);
            }
            "advanced.renderer_info" => self.show_renderer_info(),
            "viewing.forget_playback" => self.player_forget_ask(),
            ai::sideload::FROM_FILE => self.ai_sideload(false),
            ai::sideload::FROM_FOLDER => self.ai_sideload(true),
            // Settings opens the editor itself; this is for any other way in.
            "advanced.edit_settings_file" => self.chrome.open_config_editor(&self.store),
            "advanced.export" => self.fileops_export_settings(),
            "advanced.import" => self.fileops_import_settings(),
            k if k.starts_with("integration.") || k == "general.file_associations" => {
                self.desktop_setting(k)
            }
            _ => {
                specular_log::info!(target: SETTINGS, "settings action {key} is not available in this build")
            }
        }
        self.redraw = true;
    }

    // ----- Input --------------------------------------------------------------------------

    fn viewer_state_parts(&self) -> StateParts {
        let img = self
            .current_id()
            .and_then(|id| self.images.get(&id).map(|i| (id, i)));
        let Some((id, img)) = img else {
            return (None, None, false, None, false, None);
        };
        let name = Some(img.name());
        let dims = img.info.as_ref().map(|i| {
            let o = img.orient.unwrap_or(i.orientation);
            o.display_size(i.width, i.height)
        });
        let on_gpu = self.gpu_images.contains(id) && img.gpu_rank.is_some();
        let error = img
            .error
            .as_ref()
            .map(|e| {
                t!(
                    "error-cannot-open-reason",
                    file = img.name(),
                    reason = e.clone()
                )
            })
            .or_else(|| no_preview(img).then(|| t!("viewer-no-preview")));
        let loading = !on_gpu && error.is_none();
        (
            name,
            dims,
            loading,
            error,
            img.truncated,
            img.metadata.clone(),
        )
    }

    /// The facts `Action::disabled` decides on (DESIGN.md §11, "Enabled state").
    fn action_state(&self) -> ActionState {
        let img = self
            .current_id()
            .and_then(|id| self.images.get(&id).map(|i| (id, i)));
        let image = match img {
            None if self.current.is_some() => ImageState::Opening,
            None => ImageState::None,
            Some((_, i)) if i.error.is_some() || no_preview(i) => ImageState::Failed,
            Some((id, i)) if self.gpu_images.contains(id) && i.gpu_rank.is_some() => {
                ImageState::Ready
            }
            Some((_, i)) if i.info.is_some() => ImageState::Decoding,
            Some(_) => ImageState::Opening,
        };
        let count = if self.current.is_none() {
            0
        } else {
            self.entries.len()
        };
        ActionState {
            image,
            count,
            gallery_files: if self.gallery_active() {
                self.gallery.tiles_len()
            } else {
                0
            },
            // A Motion Photo's clip starts with Play/Pause too.
            animated: self.animated_current() || self.motion_current().is_some(),
            video: self.video_current(),
            sound: self.sound_current(),
            sound_file: self.sound_page_active(),
            in_archive: self.in_book(),
            has_alpha: img.is_some_and(|(_, i)| i.info.as_ref().is_some_and(|f| f.has_alpha)),
            gallery_views: self.gallery_channel_views(),
            medical: self.medical_state(),
            science: self.science_state(),
            debayer: self.debayer_capable(),
            annotations: self.annotations_capable()
                || self.pathology_has_imported()
                || self.pmi_present(),
            stain: self.stain_capable().then_some(self.pathology.set.is_some()),
            stain_region: self.stain_region().is_some(),
            slide_annotations: self.pathology_has_annotations(),
            thermal_visible: self.thermal_visible_capable(),
            drawing: self.cad_active(),
            map: self.gis_state(),
            mesh: self.mesh_state(),
            points: self.points_state(),
            tracts: self.tracts_state(),
            molecule: self.molecule_state(),
            relight: self.relight_state(),
            import_sources: self.import_has_sources(),
            dicom: img.is_some_and(|(_, i)| match &i.info {
                Some(f) => f.format == specular_decode::Format::Dicom,
                // Not open (or failed): by its name.
                None => series::is_dicom_name(&i.path),
            }),
            series: self.series_count(),
            volume: self.mpr_volume_kind().is_some(),
            mpr: self.mpr_active(),
            comparing: self.linked_active(),
            compare_images: if self.gallery_active() {
                self.gallery.compare_count()
            } else {
                0
            },
            presentation_states: self.medical_presentation_count(),
            pages: self.page_count_current(),
            rotatable: self
                .current
                .and_then(|i| self.entries.get(i))
                .is_some_and(|p| specular_fileops::orient::rotatable_name(p)),
            wallpaper: self.wallpaper_available(),
            share: specular_platform::share::share_available(),
            email: specular_platform::share::email_available(),
            profile: self.profile(),
        }
    }

    fn chrome_input(&mut self, ev: Input) -> specular_widgets::Response {
        let now = now_s(self.t0);
        let r = self.with_viewer_state(|chrome, st| chrome.input(&ev, st, now));
        self.redraw = true;
        r
    }

    /// Runs `f` with the chrome and the viewer's state as the chrome sees it.
    fn with_viewer_state<R>(&mut self, f: impl FnOnce(&mut Chrome, &ViewerState) -> R) -> R {
        self.profile_sync();
        let (name, dims, loading, error, truncated, metadata) = self.viewer_state_parts();
        self.notice_sync();
        let overlays = self.overlays();
        let pages = self.page_badge();
        // The transport's sliders read the volume and position from it.
        let playback = self.playback();
        // The cheat sheet opens on compare's keys while comparing.
        self.chrome.comparing = self.gallery_comparing();
        let st = ViewerState {
            actions: self.action_state(),
            has_image: self.current.is_some(),
            index: self.current.unwrap_or(0),
            count: self.entries.len(),
            zoom: self.view.target_scale(),
            at_fit: self.view.mode == ZoomMode::Fit,
            fullscreen: self.fullscreen,
            file_name: name.as_deref(),
            dims,
            loading,
            error: error.as_deref(),
            truncated,
            metadata: metadata.as_deref(),
            hud: &self.hud,
            keymap: &self.keymap,
            store: &self.store,
            settings: &self.settings,
            overlays,
            playback,
            motion: self.motion_badge(),
            spread: None,
            pages: pages.as_ref().map(|p| specular_widgets::PageBadge {
                index: p.0,
                count: p.1,
                label: p.2.as_deref(),
            }),
        };
        f(&mut self.chrome, &st)
    }

    fn logical(&self, p: (f32, f32)) -> (f32, f32) {
        let s = self.dl.scale.max(0.1);
        (p.0 / s, p.1 / s)
    }

    /// The key combination of a press; digit-row keys by their digit (`Keymap::resolve`,
    /// [`row_digit`]), so `Shift+1…5` work on every layout.
    fn key_combo(&self, ev: &KeyEvent) -> Option<KeyCombo> {
        let combo = combo_of(ev, self.mods)?;
        Some(self.keymap.resolve(
            combo,
            self.mods.shift_key(),
            row_digit(ev, self.mods.shift_key()),
        ))
    }

    /// The key combination of a press for a handler that also takes the press's text (a
    /// sheet or prompt with a field): none when the press is AltGr (Ctrl+Alt on Windows) or
    /// Option (macOS) typing a character, so the field types it and no shortcut fires.
    fn field_combo(&self, ev: &KeyEvent) -> Option<KeyCombo> {
        if self.altgr_text(ev) {
            return None;
        }
        self.key_combo(ev)
    }

    /// The press is AltGr or Option typing a character (see [`specular_input::typed_text`]).
    fn altgr_text(&self, ev: &KeyEvent) -> bool {
        specular_input::is_altgr_text(
            specular_input::Os::current(),
            mods_of(self.mods),
            ev.text.as_deref(),
        )
    }

    fn on_key(&mut self, ev: KeyEvent, el: &ActiveEventLoop) {
        // The keyboard's media keys: play, pause, next and previous file.
        if ev.state == ElementState::Pressed
            && let Some(a) = player::media::key_action(&ev.logical_key, self.video_current())
        {
            self.action(a, el);
            return;
        }
        // The password prompt takes every key, before anything logs them.
        if self.password.is_open() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.password_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        if self.raw_volume.is_open() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.raw_volume_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        if self.file_kind.is_open() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                self.file_kind_key(combo.as_ref());
            }
            return;
        }
        // The client gallery and picks dialogs take every key (and what it types).
        if self.web_gallery.is_open() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.web_gallery_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        // The thermal report dialog takes every key (and what it types).
        if self.thermal_report.is_open() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.thermal_report_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        if self.address.is_open() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.address_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        // The figure and key-image dialogs take every key (and what it types).
        if self.figure.visible() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.figure_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        // The annotation export takes every key.
        if self.annot_export.dialog.is_some() {
            if ev.state == ElementState::Pressed {
                let combo = self.key_combo(&ev);
                self.annot_export_key(combo.as_ref());
            }
            return;
        }
        // The export dialog and the mask tool take every key.
        if self.anon.visible() || self.anon.tool.is_some() {
            if ev.state == ElementState::Pressed {
                let combo = self.key_combo(&ev);
                if self.anon.tool.is_some() {
                    self.anon_mask_key(combo.as_ref());
                } else {
                    self.anon_key(combo.as_ref());
                }
            }
            return;
        }
        // The print dialog is modal: it takes every key.
        if self.print.dialog.is_some() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.print_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        // So are the file operation sheets.
        if self.fileops.is_open() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.fileops_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        // So is the export recipes sheet.
        if self.export_recipes.dialog.is_some() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.recipes_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        // So is the convert dialog.
        if self.convert.dialog.is_some() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.convert_key(combo.as_ref(), text.as_deref());
            } else {
                self.convert_key_up();
            }
            return;
        }
        if self.video_export.dialog.is_some() {
            if ev.state == ElementState::Pressed {
                let combo = self.key_combo(&ev);
                self.video_export_key(combo.as_ref());
            }
            return;
        }
        if self.dicomweb.dialog.is_some() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.dicomweb_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        if self.omero.dialog.is_some() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.omero_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        // The m/z prompt of an ion image (science mode) takes every key while it is open.
        if self.science_prompting() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.science_ion_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        // The measurement tool takes every key (zoom and pan keys still work through it).
        // A map panel with the keys (its field, the table, the layer panel).
        if ev.state == ElementState::Pressed && self.gis_wants_keys() {
            let combo = self.field_combo(&ev);
            let text = ev.text.as_ref().map(|t| t.to_string());
            if self.gis_key(combo.as_ref(), text.as_deref(), el) {
                return;
            }
        }
        // Walking and flying take their held keys; the 3D view's panel (views, BCF topics)
        // takes its keys while open.
        if self.chrome.modal().is_none() && self.bim_key(&ev) {
            return;
        }
        // A centreline's name being typed (curved MPR) takes the keys.
        if self.chrome.modal().is_none() && self.mpr_renaming() {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.mpr_rename_key(combo.as_ref(), text.as_deref());
            }
            return;
        }
        // The centreline chips of curved MPR with the keyboard focus (Tab enters them).
        if self.chrome.modal().is_none()
            && ev.state == ElementState::Pressed
            && let Some(c) = self.key_combo(&ev)
            && self.mpr_chips_key(&c)
        {
            return;
        }
        // The sound page's choices and synced lyrics with the keyboard (Tab enters them).
        if self.chrome.modal().is_none()
            && ev.state == ElementState::Pressed
            && let Some(c) = self.key_combo(&ev)
            && self.sound_key(&c)
        {
            return;
        }
        // The markup tool and the sheet comparison take every key (zoom and pan still work),
        // except while a confirmation is open.
        if (self.markup.tool.is_some() || self.sheets.open.is_some())
            && self.chrome.modal().is_none()
        {
            if ev.state == ElementState::Pressed {
                let combo = self.field_combo(&ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                if self.markup.tool.is_some() {
                    self.markup_key(combo.as_ref(), text.as_deref(), el);
                } else {
                    self.sheets_key(combo.as_ref(), el);
                }
            }
            return;
        }
        if self.measure.tool.is_some() {
            if ev.state == ElementState::Pressed {
                let combo = self.key_combo(&ev);
                // The keyboard crosshair places the tool's points (not while it asks for a
                // length).
                let prompting = self.measure.prompting();
                let crosshair =
                    !prompting && combo.as_ref().is_some_and(|c| self.crosshair_key(c, el));
                if !crosshair {
                    self.measure_key(combo.as_ref(), el);
                }
            }
            return;
        }
        if ev.state == ElementState::Released {
            // Settings → Keys records keys: a released Esc ends its press.
            if self.chrome.recording_keys()
                && let Some(combo) = self.key_combo(&ev)
            {
                let r = self.chrome_input(Input::KeyUp { combo });
                for c in r.commands {
                    self.command(c, el);
                }
                return;
            }
            if self.gallery_active() {
                self.gallery_key_up(&ev);
            }
            // Enter or Space let go ends Previous or Next held from the keyboard (the
            // chrome's focused button). A key let go also offers a selection made in a text
            // field to the primary selection (put off while the keys were going).
            if let Some(combo) = self.key_combo(&ev)
                && (matches!(combo.key, Key::Named(NamedKey::Enter | NamedKey::Space))
                    || self.chrome.primary_pending())
            {
                let r = self.chrome_input(Input::KeyUp { combo });
                for c in r.commands {
                    self.command(c, el);
                }
            }
            if let Some((a, t)) = self.hold
                && self.key_combo(&ev).and_then(|c| self.keymap.lookup(&c)) == Some(a)
            {
                self.hold = None;
                // A tap (not a hold) keeps the loupe up.
                if a == Action::Loupe && self.now() - t < LOUPE_TAP {
                    self.loupe_latched = true;
                }
                if matches!(a, Action::Loupe | Action::ZoomRectangle) {
                    self.redraw = true;
                }
                self.end_flipping();
            }
            return;
        }
        self.cull_dismiss_tip();
        // The keyword panel's field and rows (gallery, Library and cull mode).
        if self.chrome.modal().is_none()
            && !self.chrome.wants_text()
            && !self.chrome.list_open()
            && !self.chrome.menu_open()
            && (self.keywords_key(&ev) || self.meta_key(&ev))
        {
            return;
        }
        let gallery_keys = self.gallery_active()
            && self.chrome.modal().is_none()
            && !self.chrome.wants_text()
            && !self.chrome.list_open()
            && !self.chrome.menu_open();
        // The digit row resolves against the modes on now.
        self.keys_sync_modes();
        // AltGr or Option typing into a field: the character, never a shortcut.
        if self.chrome.wants_text() && self.altgr_text(&ev) {
            self.key_text(&ev);
            return;
        }
        let Some(combo) = self.key_combo(&ev) else {
            trace!(target: INPUT, "key {:?}: no shortcut combination", ev.logical_key);
            if gallery_keys && self.gallery_key(&ev, None) {
                return;
            }
            self.key_text(&ev);
            return;
        };
        debug!(target: INPUT, "key {combo}{}", if ev.repeat { " (repeat)" } else { "" });
        let recording = self.chrome.recording_keys();
        if !recording && !ev.repeat && self.integration_key(&combo) {
            return;
        }
        if gallery_keys && self.gallery_key(&ev, Some(&combo)) {
            return;
        }
        let r = self.chrome_input(Input::Key {
            combo,
            repeat: ev.repeat,
        });
        let mut consumed = r.consumed;
        for c in r.commands {
            self.command(c, el);
        }
        // A dialog or the go-to field takes the key's text too.
        if self.chrome.wants_text() {
            consumed = true;
            self.key_text(&ev);
        }
        if consumed {
            debug!(target: INPUT, "key consumed by the {}", if self.chrome.wants_text() { "open overlay or field" } else { "chrome" });
            return;
        }
        // Develop's panel, and arrows while a speed-edit key is held.
        if self.develop_key(combo) {
            return;
        }
        // The equaliser's panel (`sound_fx.rs`).
        if self.eq_key(combo) {
            return;
        }
        if self.slideshow_key(&combo, ev.repeat) {
            return;
        }
        // A teaching quiz takes the arrows, Space, R and Esc while it runs.
        if self.quiz_key(&combo) {
            return;
        }
        // The keyboard crosshair takes the arrows, Enter, Space and Esc while it is on.
        if self.crosshair_key(&combo, el) {
            return;
        }
        // The keymap's modes (cull mode over the viewer), sequences waiting for their next key.
        let run = self.keys_step(combo, ev.repeat);
        let Some((&last, first)) = run.split_last() else {
            debug!(target: INPUT, "no action bound");
            return;
        };
        // A sequence that did not go on runs its own binding first.
        for a in first {
            self.action(*a, el);
        }
        let mut a = last;
        if ev.repeat && !a.repeats() {
            return;
        }
        // Left/Right follow the reading direction when asked to.
        if self.arrows_reversed()
            && matches!(
                combo.key,
                Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight)
            )
        {
            a = match a {
                Action::NextImage => Action::PrevImage,
                Action::PrevImage => Action::NextImage,
                o => o,
            };
        }
        // Disabled shortcuts do nothing, held tools included.
        if let Some(why) = a.disabled(&self.action_state()) {
            debug!(target: INPUT, "action {} disabled: {why:?}", a.id());
            return;
        }
        let now = self.now();
        // Esc during a held tool (loupe, zoom rectangle) puts the tool away (see Back).
        if a == Action::Loupe && self.loupe_latched {
            if !ev.repeat {
                self.loupe_latched = false;
                self.redraw = true;
            }
            return;
        }
        let tool_held = matches!(self.hold, Some((Action::Loupe | Action::ZoomRectangle, _)));
        if !ev.repeat && !(a == Action::Back && tool_held) {
            self.hold = Some((a, now));
        }
        if ev.repeat && matches!(a, Action::NextImage | Action::PrevImage) {
            let held = self.hold.map_or(0.0, |h| now - h.1);
            let step = self.hold_step(held);
            self.flipping = true;
            self.navigate(if a == Action::NextImage { step } else { -step });
            return;
        }
        self.action(a, el);
    }

    fn key_text(&mut self, ev: &KeyEvent) {
        if !self.chrome.wants_text() {
            return;
        }
        // Plain typing, and AltGr (Ctrl+Alt on Windows) or Option characters; not shortcuts.
        if let Some(s) = specular_input::typed_text(
            specular_input::Os::current(),
            mods_of(self.mods),
            ev.text.as_deref(),
        ) {
            self.last_key_text = Some((s.clone(), Instant::now()));
            self.chrome_input(Input::Text(s));
        }
    }

    fn on_mouse_button(&mut self, state: ElementState, button: MouseButton, el: &ActiveEventLoop) {
        let p = self.pointer.unwrap_or((0.0, 0.0));
        let lp = self.logical(p);
        let down = state == ElementState::Pressed;
        if self.print.dialog.is_some() {
            self.print_click(lp, down, button == MouseButton::Left);
            return;
        }
        if self.fileops.is_open() {
            self.fileops_click(lp, down && button == MouseButton::Left);
            return;
        }
        if self.export_recipes.dialog.is_some() {
            self.recipes_click(lp, down && button == MouseButton::Left);
            return;
        }
        if self.convert.dialog.is_some() {
            if down && button == MouseButton::Middle && self.convert_middle(lp) {
                return;
            }
            self.convert_click(lp, down && button == MouseButton::Left);
            return;
        }
        if self.video_export.dialog.is_some() {
            self.video_export_click(lp, down && button == MouseButton::Left);
            return;
        }
        if self.anon.visible() {
            self.anon_click(lp, down && button == MouseButton::Left);
            return;
        }
        if self.annot_export.dialog.is_some() {
            self.annot_export_click(lp, down && button == MouseButton::Left);
            return;
        }
        if self.figure.visible() {
            self.figure_click(lp, down && button == MouseButton::Left);
            return;
        }
        if self.dicomweb.dialog.is_some() {
            if button == MouseButton::Left {
                self.dicomweb_click(lp, down);
            }
            return;
        }
        if self.omero.dialog.is_some() {
            if button == MouseButton::Left {
                self.omero_click(lp, down);
            }
            return;
        }
        if self.password.is_open() {
            if button == MouseButton::Left {
                self.password_click(lp, down);
            }
            return;
        }
        if self.thermal_report.is_open() {
            if button == MouseButton::Left {
                self.thermal_report_click(lp, down);
            }
            return;
        }
        if self.web_gallery.is_open() {
            if button == MouseButton::Left {
                self.web_gallery_click(lp, down);
            }
            return;
        }
        if self.address.is_open() {
            if button == MouseButton::Left {
                self.address_click(lp, down);
            }
            return;
        }
        if self.raw_volume.is_open() {
            if button == MouseButton::Left {
                self.raw_volume_click(lp, down);
            }
            return;
        }
        if self.file_kind.is_open() {
            if button == MouseButton::Left {
                self.file_kind_click(lp, down);
            }
            return;
        }
        if self.anon.tool.is_some() {
            if button == MouseButton::Left {
                self.anon_mask_button(p, lp, down);
            }
            return;
        }
        if self.markup.tool.is_some()
            && button == MouseButton::Left
            && self.chrome.modal().is_none()
        {
            self.markup_button(p, lp, down);
            return;
        }
        if self.sheets.open.is_some()
            && button == MouseButton::Left
            && self.chrome.modal().is_none()
            && self.sheets_button(p, lp, down)
        {
            return;
        }
        if button == MouseButton::Left
            && self.chrome.modal().is_none()
            && self.stain_click(lp, down)
        {
            return;
        }
        if self.measure.tool.is_some() && button == MouseButton::Left {
            self.measure_button(p, lp, down);
            return;
        }
        if down {
            self.cull_dismiss_tip();
        }
        let b = match button {
            MouseButton::Left => Button::Primary,
            MouseButton::Right => Button::Secondary,
            MouseButton::Middle => Button::Middle,
            MouseButton::Back => {
                // In a dialog: back inside it, never to another image under it.
                if self.chrome.modal().is_some() {
                    if down && self.chrome.back_in_dialog() {
                        self.redraw = true;
                    }
                } else if down && self.gallery_active() {
                    // Back through the folders shown, as a file manager's back button; with
                    // none behind, up a folder as before.
                    if !self.gallery_history(false) {
                        self.gallery_up();
                    }
                } else if down {
                    self.action(Action::PrevImage, el);
                }
                return;
            }
            MouseButton::Forward => {
                if down && self.chrome.modal().is_none() && self.gallery_active() {
                    self.gallery_history(true);
                } else if down && self.chrome.modal().is_none() {
                    self.action(Action::NextImage, el);
                }
                return;
            }
            _ => return,
        };
        let r = self.chrome_input(Input::PointerButton {
            x: lp.0,
            y: lp.1,
            down,
            button: b,
        });
        let consumed = r.consumed;
        for c in r.commands {
            self.command(c, el);
        }
        if self.gallery_active() {
            if !consumed && self.chrome.modal().is_none() && !self.chrome.list_open() || !down {
                self.gallery_button(down, b, lp);
            }
            return;
        }
        // Linked views take the image area's presses (`linked.rs`), and a series dragged
        // out of the Studies browser.
        if self.linked_active() {
            if down && b == Button::Primary && self.chrome.studies_open() {
                self.linked_studies_press(lp);
            }
            if (!down || !consumed && self.chrome.modal().is_none())
                && self.linked_button(p, down, b)
            {
                self.redraw = true;
            }
            return;
        }
        // Multiplanar reformatting takes the image area's presses (`mpr.rs`).
        if self.mpr_active() {
            if (!down || !consumed && self.chrome.modal().is_none()) && self.mpr_button(p, down, b)
            {
                self.redraw = true;
            }
            return;
        }
        // A 3D model takes the image area's drags (`mesh.rs`).
        if self.mesh_active()
            && (!down || !consumed && self.chrome.modal().is_none())
            && self.mesh_button(p, down, b)
        {
            self.redraw = true;
            return;
        }
        // Relighting takes presses in the light widget and Shift-drags (`relight.rs`).
        if b == Button::Primary
            && (!down || !consumed && self.chrome.modal().is_none())
            && self.relight_button(p, down)
        {
            self.redraw = true;
            return;
        }
        // Develop's panel and filmstrip (`develop.rs`).
        if b == Button::Primary
            && (!down || !consumed && self.chrome.modal().is_none())
            && self.develop_button(lp, down)
        {
            self.redraw = true;
            return;
        }
        // The equaliser's panel (`sound_fx.rs`).
        if b == Button::Primary
            && (!down || !consumed && self.chrome.modal().is_none())
            && self.eq_button(lp, down)
        {
            self.redraw = true;
            return;
        }
        // Develop's panel: a card's or a row's menu (presets, styles, snapshots, versions).
        if b == Button::Secondary
            && down
            && !consumed
            && self.chrome.modal().is_none()
            && self.develop_secondary(lp)
        {
            self.redraw = true;
            return;
        }
        // Right drag: window / level of a medical image.
        if b == Button::Secondary {
            if down && !consumed && self.chrome.modal().is_none() && self.medical_press(p, false)
                || !down && self.medical_release(false)
            {
                self.redraw = true;
            }
            return;
        }
        if b != Button::Primary {
            return;
        }
        // A drawing's layers panel.
        // A map's panels, its measuring clicks and identify.
        if (!down || !consumed && self.chrome.modal().is_none()) && self.gis_button(p, lp, down) {
            self.redraw = true;
            return;
        }
        if down && !consumed && self.chrome.modal().is_none() && self.cad_press(lp) {
            self.redraw = true;
            return;
        }
        if down && !consumed && self.chrome.modal().is_none() && self.tether_press(lp) {
            self.redraw = true;
            return;
        }
        if down && !consumed && self.chrome.modal().is_none() && self.contents_press(lp) {
            self.redraw = true;
            return;
        }
        if down && !consumed && self.chrome.modal().is_none() && self.font_press(lp)
            || !down && self.font_release()
            || down && !consumed && self.chrome.modal().is_none() && self.text_press(lp)
            || !down && self.text_release()
            || down && !consumed && self.chrome.modal().is_none() && self.sound_press(lp)
            || !down && self.sound_release(lp)
        {
            self.redraw = true;
            return;
        }
        // A map's features: a click lists the one under it (never takes the event).
        if !down || !consumed && self.chrome.modal().is_none() {
            self.geo_button(p, down);
        }
        // A science image's plane sliders, an OCT volume's B-scan slider.
        if down
            && !consumed
            && self.chrome.modal().is_none()
            && (self.thermal_seq_press(lp) || self.science_press(lp) || self.oct_press(lp))
            || !down && (self.science_release() | self.oct_release())
        {
            self.redraw = true;
            return;
        }
        if down
            && !consumed
            && self.chrome.modal().is_none()
            && !self.gallery.active
            && (self.keywords_press(lp) || self.meta_press(lp))
        {
            self.redraw = true;
            return;
        }
        if !consumed && self.chrome.modal().is_none() && self.cull_click(lp, down) {
            self.redraw = true;
            return;
        }
        // Left drag with the W/L tool (`w`).
        if down && !consumed && self.chrome.modal().is_none() && self.medical_press(p, true)
            || !down && self.medical_release(true)
        {
            self.redraw = true;
            return;
        }
        let now = self.now();
        if self.drag_out_button(down, p, consumed) {
            return;
        }
        if down && !consumed && self.chrome.modal().is_none() {
            if self
                .navigator_view()
                .is_some_and(|(v, _)| rect_contains(v.viewport, p))
            {
                self.nav_drag = true;
                self.navigator_drag(p);
                return;
            }
            if matches!(self.hold, Some((Action::ZoomRectangle, _))) && self.shown.is_some() {
                self.zoom_drag = Some((p, p));
                self.redraw = true;
                return;
            }
            // Double click.
            if let Some((t, q)) = self.last_click
                && now - t < DOUBLE_CLICK_S
                && (q.0 - p.0).abs() < 8.0
                && (q.1 - p.1).abs() < 8.0
            {
                self.last_click = None;
                // The double click's first click played or paused the movie: undo that.
                if std::mem::take(&mut self.click_toggled_play) {
                    self.playback_action(Action::PlayPause);
                }
                // On the empty background (outside the image) a double click is fullscreen.
                let on_image =
                    self.shown.is_some() && rect_contains(self.view.image_screen_rect(), p);
                let double = self.settings.viewing.double_click;
                // A movie's play mode: a double click leaves it (unless double clicks do
                // nothing); on a movie it comes where the setting gives full screen.
                // In play mode a zoomed or panned picture fits again first.
                if double != DoubleClick::Nothing
                    && self.video_full.active()
                    && !self.view.is_at_fit()
                {
                    self.view.zoom_fit();
                    self.after_view_change();
                    return;
                }
                if double != DoubleClick::Nothing
                    && (self.video_full.active()
                        || self.video_picture_current()
                            && (double == DoubleClick::Fullscreen || !on_image))
                {
                    self.video_full_toggle();
                    return;
                }
                match double {
                    DoubleClick::Fullscreen => self.toggle_fullscreen(),
                    DoubleClick::ToggleZoom if !on_image => self.toggle_fullscreen(),
                    DoubleClick::ToggleZoom => {
                        self.view.toggle_fit_actual(p);
                        self.after_view_change();
                    }
                    DoubleClick::Nothing => {}
                }
                return;
            }
            self.last_click = Some((now, p));
            self.click_toggled_play = false;
            self.dragging = Some(p);
            self.view.begin_drag(now);
        } else if !down && self.nav_drag {
            self.nav_drag = false;
            self.after_view_change();
        } else if !down && let Some((a, b)) = self.zoom_drag.take() {
            self.view.zoom_rect(a, b);
            self.after_view_change();
        } else if !down && self.dragging.take().is_some() {
            self.view.end_drag(now);
            self.after_view_change();
            // A click (not a drag) on a movie's picture plays or pauses it; on a sound
            // file's graphs it plays from there; on a playing motion clip it goes back to
            // the still.
            if !consumed
                && self
                    .last_click
                    .is_some_and(|(_, q)| (q.0 - p.0).abs() < 4.0 && (q.1 - p.1).abs() < 4.0)
            {
                if self.video_picture_current()
                    && self
                        .anim
                        .as_ref()
                        .is_some_and(|a| a.movie.is_some() && Some(a.id) == self.current_id())
                    && rect_contains(self.view.image_screen_rect(), p)
                {
                    self.click_toggled_play = self.playback_action(Action::PlayPause);
                } else if !self.sound_click(p) {
                    self.stop_motion();
                }
            }
        }
    }

    fn on_wheel(&mut self, delta: MouseScrollDelta, el: &ActiveEventLoop) {
        if self.password.is_open()
            || self.thermal_report.is_open()
            || self.web_gallery.is_open()
            || self.address.is_open()
            || self.file_kind.is_open()
        {
            return;
        }
        if self.raw_volume.is_open() {
            let y = match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            };
            if y != 0.0 {
                self.raw_volume_wheel(y > 0.0);
            }
            return;
        }
        if self.dicomweb.dialog.is_some() {
            let y = match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            };
            self.dicomweb_wheel(y);
            return;
        }
        if self.omero.dialog.is_some() {
            let y = match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            };
            self.omero_wheel(y);
            return;
        }
        if self.fileops.is_open() {
            let y = match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            };
            self.fileops_wheel(y);
            return;
        }
        if self.print.dialog.is_some()
            || self.convert.dialog.is_some()
            || self.video_export.dialog.is_some()
            || self.anon.visible()
            || self.annot_export.dialog.is_some()
            || self.figure.visible()
        {
            return;
        }
        let (lines, pixels) = match delta {
            MouseScrollDelta::LineDelta(x, y) => ((x, y), false),
            MouseScrollDelta::PixelDelta(p) => ((p.x as f32 / 40.0, p.y as f32 / 40.0), true),
        };
        let p = self.pointer.unwrap_or_else(|| self.view.viewport.center());
        let lp = self.logical(p);
        // Develop: speed edit while its key is held, the panel's scrolling.
        if self.develop_wheel(lp, lines.0, lines.1) {
            return;
        }
        if self.eq_wheel(lp, lines.1) {
            return;
        }
        let r = self.chrome_input(Input::Wheel {
            x: lp.0,
            y: lp.1,
            dx: lines.0,
            dy: lines.1,
        });
        if r.consumed {
            return;
        }
        if self.gallery_active() {
            if self.chrome.modal().is_none() && !self.chrome.list_open() {
                self.gallery_wheel(lines, pixels, lp);
            }
            return;
        }
        if self.chrome.modal().is_none()
            && (self.keywords_wheel(lp, lines.1 * 40.0) || self.meta_wheel(lp, lines.1 * 40.0))
        {
            return;
        }
        if self.linked_active() {
            if self.chrome.modal().is_none() {
                self.linked_wheel(lines, p);
            }
            return;
        }
        if self.mpr_active() {
            if self.chrome.modal().is_none() {
                self.mpr_wheel(lines, p);
            }
            return;
        }
        if self.mesh_active() {
            if self.chrome.modal().is_none() {
                self.mesh_wheel(lines);
            }
            return;
        }
        if self.chrome.modal().is_none() && self.gis_wheel(lines.1, lp) {
            return;
        }
        if self.chrome.modal().is_none() && self.cad_wheel(lines.1, lp) {
            return;
        }
        if self.chrome.modal().is_none() && self.contents_wheel(lines.1, lp) {
            return;
        }
        if self.chrome.modal().is_none() && self.font_wheel(lines.1, pixels, lp) {
            return;
        }
        if self.chrome.modal().is_none() && self.text_wheel(lines.0, lines.1, pixels, lp) {
            return;
        }
        if self.chrome.modal().is_none() && self.sound_wheel(lines.1, pixels, lp) {
            return;
        }
        // Touchpad scrolls (pixel deltas) pan whatever the wheel setting says (DESIGN.md
        // §12.6); Ctrl zooms with either.
        // In medical mode the wheel steps the frames of a multi-frame file, and the slices
        // of a DICOM series (`medical.wheel_steps_frames`); Ctrl+wheel zooms.
        let steps = self.settings.medical.wheel_steps_frames;
        if steps
            && !pixels
            && !self.mods.control_key()
            && self.medical_state().is_some_and(|n| n >= 2)
        {
            if lines.1.abs() >= 0.5 {
                let a = if lines.1 < 0.0 {
                    Action::NextSlice
                } else {
                    Action::PrevSlice
                };
                for _ in 0..(lines.1.abs().round() as u32).clamp(1, 10) {
                    self.medical_action(a);
                }
            }
            return;
        }
        let zoom = self.mods.control_key()
            || (!pixels
                && self.settings.viewing.wheel_behaviour == WheelBehaviour::Zoom
                && !(steps && self.series_active()));
        if zoom {
            // macOS accelerates a mouse wheel's line deltas (a quick turn reports several
            // lines in one event, in fractions), so there each event is at most one notch.
            let mac_wheel = cfg!(target_os = "macos") && !pixels;
            let f = if pixels {
                1.0035f32.powf(lines.1 * 40.0)
            } else if mac_wheel {
                1.15f32.powf(lines.1.clamp(-1.0, 1.0))
            } else {
                1.2f32.powf(lines.1)
            };
            if pixels || (lines.1.fract() != 0.0 && !mac_wheel) {
                // Touchpad: Ctrl+scroll, or a Windows precision-touchpad pinch (Ctrl+wheel in
                // fractions of a notch): follow the fingers at once.
                self.view.zoom_direct_by(f, Some(p));
            } else {
                let target = self.view.target_scale() * f;
                self.view.zoom_to(target, Some(p));
            }
            self.after_view_change();
        } else if pixels {
            self.view.pan_by(-lines.0 * 40.0, -lines.1 * 40.0, false);
            self.redraw = true;
        } else if lines.1.abs() >= 0.5 {
            self.action(
                if lines.1 < 0.0 {
                    Action::NextImage
                } else {
                    Action::PrevImage
                },
                el,
            );
        }
    }

    // ----- Frames -------------------------------------------------------------------------

    fn init_gfx(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        let (w, h) = self
            .args
            .size
            .unwrap_or_else(|| self.initial_window_size(el));
        let attrs = Window::default_attributes()
            .with_title("Specular")
            .with_inner_size(winit::dpi::LogicalSize::new(w as f64, h as f64))
            .with_min_inner_size(winit::dpi::LogicalSize::new(360.0, 260.0))
            .with_theme(window_theme(self.settings.general.theme))
            // Shown once the screen-reader adapter is attached (it must come first).
            .with_visible(false);
        let attrs = with_app_id(attrs);
        self.stage("resumed");
        let window = Arc::new(
            el.create_window(attrs)
                .map_err(|e| format!("cannot create the window: {e}"))?,
        );
        self.stage("window_created");
        self.a11y = Some(Accessibility::new(el, &window, &self.loader));
        window.set_visible(true);
        self.stage("a11y_adapter");
        // The first image's open job may not have planned yet: let it plan for this window
        // rather than wait for the GPU (the window's size is only read again once it's ready).
        let size = window.inner_size();
        self.loader.hint_viewport(size.width, size.height);
        // A level the hint started for an image whose `Opened` was already handled.
        if let Some(id) = self.current_id()
            && let Some(img) = self.images.get_mut(&id)
            && img.session.is_some()
        {
            let late = self.loader.take_late_levels(id, &img.token);
            img.levels.extend(late);
        }
        let gpu = self
            .gpu_join
            .take()
            .ok_or("GPU already taken")?
            .join()
            .map_err(|_| "GPU thread panicked".to_owned())??;
        self.stage("gpu_joined");
        if let Some(b) = self.bench.as_mut() {
            b.gpu_init_ms = gpu.init_ms;
            for &(name, at) in &gpu.stages {
                b.stage(name, at);
            }
        }
        let mut text = self
            .text_join
            .take()
            .ok_or("fonts already taken")?
            .join()
            .map_err(|_| "font thread panicked".to_owned())?;
        self.stage("fonts_joined");
        text.set_locale(Some(I18n::global().language().to_string()));
        // The language lists' fallback fonts (CJK and others) are found and loaded off this
        // thread, so the first such list opens at once.
        text.set_warm_up(std::sync::Arc::new(specular_widgets::warm_up_texts));
        let budget = match self.settings.performance.vram_cache {
            0 => 384u64 << 20,
            mb => ((mb as u64) << 20) / 2,
        };
        // The image renderer (shaders, pipelines, the tile array: about 5 ms) builds on its
        // own thread while this one configures the surface, compositor and UI renderer.
        let (surface, comp, ui, mut renderer) = std::thread::scope(|s| {
            let image = specular_thread::builder("gpu-renderer")
                .spawn_scoped(s, || ImageRenderer::new(&gpu, budget));
            let surface = WindowSurface::new(&gpu, window.clone());
            let ui = surface.as_ref().ok().map(|surface| {
                self.stage("surface_configured");
                let (sw, sh) = surface.size();
                let comp = Compositor::new(&gpu, surface.format(), surface.bits, sw, sh);
                comp.set_bits(
                    &gpu,
                    if self.settings.colour.dithering {
                        surface.bits
                    } else {
                        16
                    },
                );
                self.stage("compositor_ready");
                let ui = UiRenderer::new(&gpu, &comp);
                self.stage("ui_renderer_ready");
                (comp, ui)
            });
            let renderer = match image {
                Ok(h) => h
                    .join()
                    .map_err(|_| "renderer thread panicked".to_owned())?,
                Err(_) => ImageRenderer::new(&gpu, budget),
            };
            let surface = surface?;
            let (comp, ui) = ui.expect("built with the surface");
            Ok::<_, String>((surface, comp, ui, renderer))
        })?;
        performance::apply(&self.settings, Some(&mut renderer));
        performance::init(&self.hw, &gpu);
        self.stage("image_renderer_ready");
        self.hud.backend = format!("{:?} · {}", gpu.info.backend, gpu.info.name);
        diagnostics::set_gpu(
            &gpu.info.name,
            &format!("{:?}", gpu.info.backend),
            &format!("{} {}", gpu.info.driver, gpu.info.driver_info),
        );
        self.hud.threads = self.loader.threads();
        if self.settings.viewing.fullscreen_on_open
            && self.args.screenshot.is_none()
            && self.args.bench_open.is_none()
        {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            self.fullscreen = true;
        }
        self.gfx = Some(Gfx {
            window,
            gpu,
            surface,
            comp,
            ui,
            renderer,
            text,
        });
        ai::device::publish(self.gfx.as_ref().map(|g| &g.gpu));
        debug!(target: STARTUP, "window and GPU ready at {:.1} ms", specular_core::ms_since_start());
        self.ready_at = Some(Instant::now());
        // Images that arrived before the GPU.
        if let Some(id) = self.current_id() {
            self.request_levels(id, Priority::Immediate);
            self.upload(id);
        }
        self.schedule();
        self.update_title();
        Ok(())
    }

    /// Logical window size for the first image: its shape, at 100% when that fits in 80% of
    /// the monitor, else scaled down to fit; 1280×800 without an image or a monitor.
    fn initial_window_size(&self, el: &ActiveEventLoop) -> (u32, u32) {
        const DEFAULT: (u32, u32) = (1280, 800);
        let Some(path) = self.current.and_then(|i| self.entries.get(i)) else {
            return DEFAULT;
        };
        let Some((iw, ih)) = loader::header_display_size(path) else {
            return DEFAULT;
        };
        let Some(mon) = el
            .primary_monitor()
            .or_else(|| el.available_monitors().next())
        else {
            return DEFAULT;
        };
        let sf = mon.scale_factor().max(0.5);
        let ms = mon.size();
        if ms.width == 0 || ms.height == 0 {
            return DEFAULT;
        }
        let (bw, bh) = (ms.width as f64 / sf * 0.8, ms.height as f64 / sf * 0.8);
        let (lw, lh) = (iw as f64 / sf, ih as f64 / sf);
        let k = (bw / lw).min(bh / lh).min(1.0);
        // Not smaller than a comfortable minimum (the image is centred in the extra room).
        (
            (lw * k).round().max(640.0) as u32,
            (lh * k).round().max(420.0) as u32,
        )
    }

    fn resize(&mut self, w: u32, h: u32) {
        let Some(g) = self.gfx.as_mut() else { return };
        // The window manager may take the window in or out of fullscreen itself: Back's
        // ladder follows what the window is, so it neither skips that step nor takes it twice.
        self.fullscreen = g.window.fullscreen().is_some();
        if (w, h) == g.surface.size() || w == 0 || h == 0 {
            return;
        }
        g.surface.resize(&g.gpu, w, h);
        let (sw, sh) = g.surface.size();
        g.comp.resize(&g.gpu, sw, sh);
        g.ui.compositor_resized(&g.gpu, &g.comp);
        self.scene = None;
        if let Some(id) = self.current_id() {
            self.request_levels(id, Priority::Immediate);
        }
        self.redraw = true;
    }

    /// Requests the detail tiles the view needs.
    fn loupe_active(&self) -> bool {
        (matches!(self.hold, Some((Action::Loupe, _))) || self.shot_loupe || self.loupe_latched)
            && self.shown.is_some()
            && self.chrome.modal().is_none()
            && !self.chrome.any_menu_shown()
            && !self.app_dialog_open()
            && self
                .pointer
                .is_some_and(|p| rect_contains(self.view.viewport, p))
    }

    /// One of the app's own dialogs (not the chrome's modals) is up: under it nothing
    /// hovers or drags, and the loupe gives the pointer back.
    fn app_dialog_open(&self) -> bool {
        self.print.dialog.is_some()
            || self.convert.dialog.is_some()
            || self.fileops.is_open()
            || self.video_export.dialog.is_some()
            || self.anon.visible()
            || self.annot_export.dialog.is_some()
            || self.figure.visible()
            || self.dicomweb.dialog.is_some()
            || self.omero.dialog.is_some()
            || self.password.is_open()
            || self.thermal_report.is_open()
            || self.web_gallery.is_open()
            || self.address.is_open()
            || self.raw_volume.is_open()
            || self.file_kind.is_open()
    }

    /// The loupe: a disc around the pointer showing the image magnified (at least 100%,
    /// three times the view's scale), drawn from full-resolution tiles.
    fn loupe_view(&self) -> Option<(View, (f32, f32), f32)> {
        if !self.loupe_active() || self.spread_active() {
            return None;
        }
        let p = self.pointer?;
        Some((self.loupe_view_at(p), p, LOUPE_RADIUS * self.dl.scale))
    }

    /// The loupe's view with the pointer at `p` (physical px).
    fn loupe_view_at(&self, p: (f32, f32)) -> View {
        let r = LOUPE_RADIUS * self.dl.scale;
        let scale = (self.view.scale() * 3.0).clamp(1.0, 32.0);
        let d = self.view.screen_to_display(p);
        let vp = PxRect {
            x: p.0 - r,
            y: p.1 - r,
            w: 2.0 * r,
            h: 2.0 * r,
        };
        View::fixed(
            self.view.stored_size(),
            self.view.orientation(),
            vp,
            scale,
            d,
        )
    }

    /// Where the moving loupe will be in [`LOUPE_AHEAD`]: its view there, while the pointer
    /// moves (velocity from the last moves) and the place is over the image.
    fn loupe_ahead_view(&self) -> Option<View> {
        let p = self.pointer?;
        let (v, at) = self.pointer_vel?;
        if at.elapsed() > LOUPE_STILL {
            return None;
        }
        let t = LOUPE_AHEAD.as_secs_f32();
        let vp = self.view.viewport;
        let q = (
            (p.0 + v.0 * t).clamp(vp.x, vp.x + vp.w),
            (p.1 + v.1 * t).clamp(vp.y, vp.y + vp.h),
        );
        let r = LOUPE_RADIUS * self.dl.scale;
        ((q.0 - p.0).abs() + (q.1 - p.1).abs() > r * 0.25).then(|| self.loupe_view_at(q))
    }

    /// The navigator: the whole image in a corner (top, end side), and the part of it the
    /// window shows, in physical pixels.
    fn navigator_view(&self) -> Option<(View, PxRect)> {
        if !self.navigator
            || self.shown.is_none()
            || self.shown != self.current_id()
            || self.spread_active()
        {
            return None;
        }
        let ui = self.dl.scale;
        let (dw, dh) = self.view.display_size();
        let m = OVERLAY_MARGIN * ui;
        let vp = self.view.viewport;
        let mut ns = (NAVIGATOR_BOX.0 * ui / dw).min(NAVIGATOR_BOX.1 * ui / dh);
        // Very wide (or tall) images: keep the short side readable, up to the full window.
        let min_side = NAVIGATOR_MIN_SIDE * ui;
        let (room_w, room_h) = ((vp.w - 2.0 * m).max(0.0), (vp.h - 2.0 * m).max(0.0));
        if dh * ns < min_side {
            ns = (min_side / dh).min(room_w / dw);
        } else if dw * ns < min_side {
            ns = (min_side / dw).min(room_h / dh);
        }
        let (fw, fh) = ((dw * ns).max(8.0), (dh * ns).max(8.0));
        if vp.w < fw + 2.0 * m || vp.h < fh + 2.0 * m {
            return None;
        }
        let x = if self.rtl() {
            vp.x + m
        } else {
            vp.x + vp.w - m - fw
        };
        let frame = PxRect {
            x: x.round(),
            y: (vp.y + m).round(),
            w: fw.round(),
            h: fh.round(),
        };
        let nav = View::fixed(
            self.view.stored_size(),
            self.view.orientation(),
            frame,
            ns,
            (dw * 0.5, dh * 0.5),
        );
        let (x0, y0, x1, y1) = self.view.visible_display();
        let a = nav.display_to_screen((x0, y0));
        let b = nav.display_to_screen((x1, y1));
        Some((
            nav,
            PxRect {
                x: a.0,
                y: a.1,
                w: b.0 - a.0,
                h: b.1 - a.1,
            },
        ))
    }

    /// Navigator drag: centre the view on the image point under the pointer.
    fn navigator_drag(&mut self, p: (f32, f32)) {
        let Some((nav, _)) = self.navigator_view() else {
            self.nav_drag = false;
            return;
        };
        let d = nav.screen_to_display(p);
        let (dw, dh) = self.view.display_size();
        self.view
            .center_on((d.0.clamp(0.0, dw), d.1.clamp(0.0, dh)), false);
        self.redraw = true;
    }

    /// Overlay geometry for the chrome (logical pixels).
    fn overlays(&self) -> Overlays {
        let s = self.dl.scale.max(0.1);
        let lr = |r: PxRect| specular_gfx::Rect::new(r.x / s, r.y / s, r.w / s, r.h / s);
        Overlays {
            navigator: self.navigator_view().map(|(n, v)| (lr(n.viewport), lr(v))),
            loupe: self
                .loupe_view()
                .map(|(_, c, r)| ((c.0 / s, c.1 / s), r / s)),
            zoom_rect: self.zoom_drag.map(|(a, b)| {
                lr(PxRect {
                    x: a.0.min(b.0),
                    y: a.1.min(b.1),
                    w: (a.0 - b.0).abs(),
                    h: (a.1 - b.1).abs(),
                })
            }),
        }
    }

    fn request_tiles(&mut self) {
        let t0 = Instant::now();
        self.request_tiles_now();
        self.tile_stats.calls += 1;
        self.tile_stats.ns += t0.elapsed().as_nanos() as u64;
    }

    fn request_tiles_now(&mut self) {
        self.tiles_missing = 0;
        // Until the loupe's tiles are listed below.
        self.tile_stats.loupe_missing = usize::from(self.loupe_active());
        self.detail_level = None;
        // Animation frames and spreads draw from the overview only.
        if self.animated_current() || self.spread_active() {
            return;
        }
        let loupe = self.loupe_view().map(|l| l.0);
        let ahead = loupe.as_ref().and_then(|_| self.loupe_ahead_view());
        let pixel_loupe = loupe.is_none().then(|| self.pixel_loupe_view()).flatten();
        let Some(id) = self.current_id() else { return };
        let (Some(gfx), Some(img)) = (self.gfx.as_mut(), self.images.get(&id)) else {
            return;
        };
        let Some(session) = img.session.clone() else {
            return;
        };
        let Some(g) = self.gpu_images.get(id) else {
            return;
        };
        if g.overview.is_none() {
            return;
        }
        // While the first whole level is decoding, region decodes would build the row index
        // a second time beside it (the level decode records it for free) and halve its speed;
        // the level is on screen within ~70 ms anyway. They start when it lands.
        if !session.regions_ready() && img.levels.iter().any(|&s| s < img.rank()) {
            self.tiles_missing = 1;
            return;
        }
        let main = gfx.renderer.wanted_tiles(g, &self.view);
        self.detail_level = main.as_ref().map(|m| m.0);
        // The loupe's tiles first: it is what the user is looking at.
        let mut wants: Vec<(u8, Vec<(u32, u32)>)> = Vec::new();
        let mut loupe_missing = 0;
        if let Some(l) = loupe.and_then(|v| gfx.renderer.wanted_tiles_also(g, &v)) {
            loupe_missing = l.1.len();
            wants.push(l);
        } else if let Some(l) = pixel_loupe
            .and_then(|v| gfx.renderer.wanted_tiles_also(g, &v))
            .filter(|l| Self::pixel_loupe_tiles_cheap(&*session, (g.width, g.height), l))
        {
            wants.push(l);
        }
        wants.extend(main);
        // Where the moving loupe is heading, after what is on screen (and not counted as
        // missing): its tiles are often there when it arrives.
        let shown = wants.len();
        if let Some((level, ahead)) = ahead.and_then(|v| gfx.renderer.wanted_tiles_also(g, &v)) {
            let on_screen: HashSet<(u32, u32)> = wants
                .iter()
                .filter(|w| w.0 == level)
                .flat_map(|w| w.1.iter().copied())
                .collect();
            let rest: Vec<(u32, u32)> = ahead
                .into_iter()
                .filter(|t| !on_screen.contains(t))
                .collect();
            if !rest.is_empty() {
                wants.push((level, rest));
            }
        }
        // Jobs for other images or levels are stale.
        let mut cancelled = 0;
        self.tile_jobs.retain(|j| {
            let keep = j.id == id && wants.iter().any(|w| w.0 == j.level);
            if !keep {
                j.handle.cancel();
                cancelled += 1;
            }
            keep
        });
        self.tile_stats.cancelled += cancelled;
        // Not resident yet (in flight or not asked for).
        self.tile_stats.loupe_missing = loupe_missing;
        let (w, h) = (g.width, g.height);
        // Concurrent region decodes stay within a share of the RAM budget (progressive
        // regions need ~50 MB of coefficients each); one always runs.
        let ram = match self.settings.performance.ram_cache {
            0 => self.hw.ram_cache_budget(),
            mib => (mib as u64) << 20,
        };
        let mem_budget = (ram / 8).clamp(128 << 20, 256 << 20);
        let mut mem_used: u64 = self.tile_jobs.iter().map(|j| j.mem).sum();
        'levels: for (i, (level, missing)) in wants.into_iter().enumerate() {
            let counted = i < shown;
            // A whole level of this scale is decoding (progressive files have the row index
            // before their big levels): its tiles would decode the same pixels beside it.
            if img.levels.contains(&(1u32 << level)) {
                self.tiles_missing += usize::from(counted);
                continue;
            }
            let inflight: HashSet<(u32, u32)> = self
                .tile_jobs
                .iter()
                .filter(|j| j.level == level)
                .flat_map(|j| j.tiles.iter().copied())
                .collect();
            let todo: Vec<(u32, u32)> = missing
                .into_iter()
                .filter(|t| {
                    !inflight.contains(t) && !self.tiles_failed.contains(&(id, level, t.0, t.1))
                })
                .collect();
            if counted {
                self.tiles_missing += todo.len() + inflight.len();
            }
            for batch in tiles::batches(&todo, TILE_BATCH) {
                if self.tile_jobs.len() >= MAX_TILE_JOBS {
                    break 'levels;
                }
                let rect = tiles::decode_rect(w, h, level, &batch);
                let req = specular_decode::DecodeRequest {
                    scale: 1 << level,
                    region: Some(specular_decode::Rect {
                        x: rect.0,
                        y: rect.1,
                        w: rect.2,
                        h: rect.3,
                    }),
                    parallel: true,
                    prepare_regions: false,
                };
                let mem = session.decode_memory(&req);
                if !self.tile_jobs.is_empty() && mem_used + mem > mem_budget {
                    break 'levels;
                }
                mem_used += mem;
                trace!(target: TILES, "img={id} request L{level} {} tiles, rect {rect:?}, {} KiB", batch.len(), mem >> 10);
                let handle =
                    self.loader
                        .decode_tiles(id, session.clone(), level, rect, batch.clone());
                self.tile_jobs.push(TileJob {
                    id,
                    level,
                    tiles: batch,
                    handle,
                    mem,
                });
                self.tile_stats.started += 1;
            }
        }
    }

    fn frame(&mut self, el: &ActiveEventLoop) {
        let t_frame = Instant::now();
        let asked = frame_asked(&mut self.redraw_asked, &mut self.frame_asked);
        // What this frame's own work asks for (an upload that came late, a render in parts)
        // is drawn next (see "When to draw next").
        self.redraw = false;
        self.stage("first_frame_start");
        let dt = t_frame
            .duration_since(self.last_frame)
            .as_secs_f32()
            .min(0.1);
        self.last_frame = t_frame;
        if !self.recover_lost_gpu() {
            return;
        }
        let now = self.now();
        self.apply_settings(false);
        self.video_full_follow();
        self.note_book_page();
        self.update_hdr_output(false);
        self.update_display_colour();
        let i18n = I18n::global();
        if i18n.generation() != self.i18n_gen {
            self.i18n_gen = i18n.generation();
            self.on_language_changed();
        }
        self.drive_bench(el);
        self.drive_shot_before();
        // The gallery and the start screen come and go by many paths: the title follows here.
        self.update_title();
        self.chrome.back_to_gallery = self.gallery_back_offered();
        let Some((pw, ph)) = self.viewport_px() else {
            return;
        };
        let scale = self
            .gfx
            .as_ref()
            .map_or(1.0, |g| g.window.scale_factor() as f32)
            * self.ui_scale;
        self.dl.clear(scale);
        // Develop's panel and filmstrip (logical px); the chrome stays above the filmstrip.
        let (dev_w, dev_h) = self
            .develop
            .inset(pw as f32 / scale, self.chrome.text_scale);
        self.chrome
            .set_size(pw as f32 / scale, ph as f32 / scale - dev_h);
        self.chrome.set_panel_inset(dev_w);

        // The image area leaves room for the info panel and Develop's panel and filmstrip.
        let inset = (self.chrome.info_inset() * scale).min(pw as f32 * 0.5) + dev_w * scale;
        let rtl = self.rtl();
        let vp = PxRect {
            x: if rtl { inset } else { 0.0 },
            y: 0.0,
            w: pw as f32 - inset,
            h: ph as f32 - dev_h * scale,
        };
        self.view.set_viewport(vp);
        if self.shot_loupe {
            // Screenshots: the pointer sits right of centre, over the image.
            self.pointer = Some((vp.x + vp.w * 0.62, vp.y + vp.h * 0.45));
        }
        self.setup_view();
        self.mpr_layout(vp, rtl);
        self.linked_layout(vp, rtl);
        self.mesh_layout(vp);
        if self.pending_zoom && self.shown.is_some() {
            self.pending_zoom = false;
            match self.settings.viewing.initial_zoom {
                InitialZoom::ActualSize => self.view.zoom_to(1.0, None),
                InitialZoom::Fill => self.view.zoom_fill(),
                _ => {}
            }
            self.view.settle();
        }
        self.drive_pan_bench();
        self.slideshow_ken_burns(now);
        let animating = if self.reduce_motion() && !self.view.is_dragging() {
            if self.view.is_animating() {
                self.view.settle();
            }
            false
        } else {
            self.view.step(dt)
        };
        self.request_tiles();

        // Chrome.
        let pal = self.palette();
        let canvas = self.canvas(&pal);
        let capture =
            self.shot_ready(now) || self.remote.script.screenshot.is_some() || self.bim_capturing();
        let (name, dims, loading, error, truncated, metadata) = self.viewer_state_parts();
        self.notice_sync();
        let cur = self.current_id();
        let overlays = self.overlays();
        let nav_view = self.navigator_view().map(|n| n.0);
        let oct_inset = self.oct_inset();
        let oct_slider = self.oct_shown();
        let loupe_view = self.loupe_view();
        let spread_views = self.spread_views();
        let spread = self.spread_last();
        let playback = self.playback();
        let motion = self.motion_badge();
        let pages = self.page_badge();
        let slideshow = self.slideshow_overlay();
        let cull_strip = self.cull_strip(now);
        let cull_model = cull_strip.as_ref().and_then(|_| self.cull_panel());
        // The loupe stands in for the pointer while it shows (`q` held over the image).
        let pointer_hide = self.pointer_hide(now);
        // MPR draws its own readouts, letters and measurements over the image area.
        let medical_on = !self.gallery.active && !self.mpr_active() && !self.linked_active();
        let medical_lines = self.medical_lines().filter(|_| medical_on);
        let measurements = self.medical_measurements().filter(|_| medical_on);
        let medical_marks = self.medical_marks().filter(|_| medical_on);
        let mask_view = self.anon_mask_view().filter(|_| medical_on);
        // A quiz case not revealed yet: its annotations (and key badge) stay hidden.
        let quiz_hides = self.quiz_hides();
        let quiz_card = self.quiz_view();
        let key_badge = self.figure_current_is_key() && quiz_card.is_none();
        let measure_view = self.measure_view().filter(|_| medical_on && !quiz_hides);
        let markup_view = self.markup_view().filter(|_| medical_on && !quiz_hides);
        let sheets_view = self.sheets_view().filter(|_| medical_on);
        let medical_plane = self.medical_plane().filter(|_| medical_on);
        let medical_curve = self.medical_curve().filter(|_| medical_on).cloned();
        let mpr_rev = self.mpr_rev();
        let mesh_rev = self.mesh_rev();
        let mesh_lines = self.mesh_lines();
        let relight_look = self.relight_widget();
        // A map's terrain in 3-D hides the map's values, their readout and overlays.
        let terrain_3d = self.mesh_active() && self.map.t.terrain_3d;
        let science_lines = self
            .science_lines()
            .or_else(|| self.geo_lines())
            .filter(|_| !self.gallery.active && !terrain_3d);
        // A thermal sequence's playback and temperatures over time.
        let thermal_seq_panel = if self.gallery.active || terrain_3d {
            None
        } else {
            self.thermal_seq_panel()
        };
        let science_overlay = if self.gallery.active || terrain_3d {
            None
        } else {
            self.science_overlay()
                .map(|mut o| {
                    // A DEM shown as values: the maps and photos beside it too.
                    let (c, m) = self.geo_overlay_marks();
                    o.contours.extend(c);
                    o.markers.extend(m);
                    // A map's scale bar in its units, measured on the ellipsoid.
                    if let Some(b) = self.gis_scale_bar() {
                        o.scale = Some(b);
                    }
                    o
                })
                .or_else(|| self.geo_overlay())
                .or_else(|| self.oct_overlay())
        };
        // Overlays a script sent (`script.rs`).
        let script_shapes = if self.gallery.active || terrain_3d {
            Vec::new()
        } else {
            let mut s = self.script_placed();
            s.extend(self.codes_placed());
            s.extend(self.find_text_placed());
            s.extend(self.faces_placed());
            s
        };
        let cad_frame = self.cad_frame(&pal);
        let map_frame = self.gis_frame(&pal);
        let map_view = self.gis_view();
        let map_floor = {
            let s = self.dl.scale.max(0.1);
            let vp = self.view.viewport;
            self.gis_floor((vp.x / s, vp.y / s, vp.w / s, vp.h / s))
        };
        let cad_panel = self.cad_panel(&pal);
        let bim_panel = self.bim_panel();
        let bim_overlay = self.bim_overlay();
        let pmi_overlay = self
            .pmi_present()
            .then(|| self.mesh.pmi_overlay(f64::from(self.dl.scale.max(0.1))))
            .flatten();
        let contents_panel = self.contents_panel();
        let tether_panel = self.tether_panel();
        let chart_view = self.chart_view(&pal);
        self.hud.scale = self.view.scale();
        // Only the HUD shows these: the job pool's mutex (contended by niced workers) and
        // a read of /proc per frame otherwise.
        if self.chrome.hud_open() {
            self.hud.queued = self.loader.queued();
            self.hud.ram_bytes = rss_bytes();
        }
        let actions = self.action_state();
        let a11y_title = self
            .a11y
            .as_ref()
            .is_some_and(Accessibility::is_active)
            .then(|| self.window_title());
        let gallery_on = self.gallery.active || self.gallery.transition.is_some();
        let reduce = self.reduce_motion();
        if gallery_on {
            self.gallery.feed();
        }
        let has_alpha = self.action_state().has_alpha;
        let thumb_backdrop = self.transparency(&pal, None);
        let view_backdrop = self.transparency(&pal, Some(canvas));
        let quick = if gallery_on {
            self.quick_plan(
                (pw as f32 / scale, ph as f32 / scale),
                scale,
                thumb_backdrop,
            )
        } else {
            None
        };
        self.chrome.comparing = self.gallery_comparing();
        let presenting = self.presenting_status();
        self.chrome.set_presenting_on(presenting);
        let export_view = self.video_export_view();
        let dicomweb_view = self.dicomweb_view();
        let omero_view = self.omero_view();
        let password_view = self.password_view();
        let thermal_report_view = self.thermal_report_view();
        let web_gallery_view = self.web_gallery_view();
        let address_view = self.address_view();
        let raw_volume_view = self.raw_volume_view();
        let file_kind_view = self.file_kind_view();
        let crosshair = self.crosshair_screen();
        // Pathology (`pathology.rs`, `annot_export.rs`): the stains, their legend, imported
        // annotations and the export dialog.
        let stain_view = self.stain_view();
        let (ocio_lut, ocio_upload) = self.ocio_for_current();
        let stain_legend = self.stain_legend();
        let imported_set = self.pathology_imported_set();
        let annot_view = self.annot_export_view();
        // A map field's input method window (Go to, the table's search, an address).
        let map_ime = self.map.ime_area().filter(|_| self.gis_on());
        // A movie's player: its menus and seek bar preview for the chrome, its subtitles.
        self.preview_frame();
        let player_view = self.player_view();
        // A sound file's page follows the file and the playing position.
        self.sound_sync();
        self.info_loudness_tick();
        self.chrome.set_player(player_view);
        let cues = self.player_cues();
        let picture_adjust = self.player_render_picture();
        // Where the picture is at fit: subtitles stay put while it is zoomed or panned.
        let picture = self.player_subtitle_rect();
        let Some(gfx) = self.gfx.as_mut() else { return };
        self.hud.tiles = gfx.renderer.pool.len();
        self.hud.tile_capacity = gfx.renderer.tile_capacity() as usize;
        self.hud.vram_bytes = self.gpu_images.bytes() + gfx.renderer.tile_bytes();
        // The level on screen: the detail level once its tiles are in, else the overview.
        let overview_level = cur
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.gpu_rank)
            .map_or(0, |r| if r >= 1000 { 0 } else { r });
        self.hud.level = match self.detail_level {
            Some(l) if self.tiles_missing == 0 => 1 << l,
            _ => overview_level,
        };
        if let Some(i) = cur.and_then(|id| self.images.get(&id))
            && let Some(ms) = i.open_ms
        {
            self.hud.open_ms = ms;
        }
        if self.shot.is_some() {
            self.chrome.poke(now);
        }
        gfx.text.begin_frame(scale);
        // The gallery draws over the image, under the chrome (toasts, menus, dialogs).
        let (gallery_wake, gallery_anim) = if gallery_on {
            let t = Instant::now();
            self.gallery.upload(&gfx.gpu, &mut gfx.ui);
            self.gallery.prepare_quick(gfx, quick.as_ref());
            self.gallery.prepare_map(
                &gfx.gpu,
                &mut gfx.ui,
                scale,
                (pw as f32 / scale, ph as f32 / scale),
            );
            self.gallery.chrome_kbd = self.chrome.indicator_focused();
            self.gallery.work_chip = self.chrome.work_chip_width();
            let gf = gallery::GalleryFrame {
                pal: &pal,
                now,
                reduce_motion: reduce,
                size: (pw as f32 / scale, ph as f32 / scale),
                scale,
                rtl,
                text_scale: self.chrome.text_scale,
                // Plain under transparent thumbnails: a grid of checkerboards is the loudest
                // thing on screen. Quick look and the viewer keep the transparency setting.
                backdrop: Backdrop::Solid(pal.surface_raised),
            };
            let r = self.gallery.draw(&mut self.dl, &mut gfx.text, &gf);
            self.gallery.note_frame(t.elapsed());
            self.chrome.gallery_work_end = Some(self.gallery.work_end);
            self.chrome.gallery_free = Some(self.gallery.status_free);
            r
        } else {
            self.gallery.release_idle(now, &mut gfx.ui);
            self.chrome.gallery_work_end = None;
            self.chrome.gallery_free = None;
            self.gallery
                .prepare_map(&gfx.gpu, &mut gfx.ui, scale, (0.0, 0.0));
            (None, false)
        };
        // A font's specimen covers the image area (its CPU preview) under the panels.
        self.font_view.prepare(&gfx.gpu, &mut gfx.ui, scale, canvas);
        if !gallery_on && self.font_view.active() {
            let area = (vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            let v = &mut self.font_view;
            font_view::draw(&mut self.dl, &mut gfx.text, &pal, canvas, v, area, rtl, ts);
        }
        // A text file's lines, rows or cues cover its page preview, under the panels.
        self.text_view.prepare(&gfx.gpu, &mut gfx.ui);
        if !gallery_on && self.text_view.active() {
            let area =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            let v = &mut self.text_view;
            text_view::draw(&mut self.dl, &mut gfx.text, &pal, canvas, v, area, rtl, ts);
            if self.text_view.wants_upload() {
                self.redraw = true;
            }
        }
        // A sound file's page covers its picture, under the panels.
        let page_px = (vp.w.max(1.0) as u32, vp.h.max(1.0) as u32);
        self.sound_page.prepare(&gfx.gpu, &mut gfx.ui, page_px);
        if !gallery_on && self.sound_page.active() {
            let area =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            let p = &mut self.sound_page;
            sound_page::draw(&mut self.dl, &mut gfx.text, &pal, p, area, rtl, ts, scale);
        }
        if !gallery_on && !cues.is_empty() {
            let (x, y, w, h) = picture;
            let pic = specular_gfx::Rect::new(x / scale, y / scale, w / scale, h / scale);
            let refs: Vec<&specular_subs::Cue> = cues.iter().collect();
            player::subs::draw(
                &mut self.dl,
                &mut gfx.text,
                &gfx.gpu,
                &mut gfx.ui,
                &mut self.player.bitmaps,
                &refs,
                pic,
                &player::subs::look(&self.settings),
            );
        }
        player::subs::offer_fonts(
            &mut self.player.fonts_offered,
            &mut self.chrome,
            &mut gfx.text,
        );
        let st = ViewerState {
            actions,
            has_image: self.current.is_some(),
            index: self.current.unwrap_or(0),
            count: self.entries.len(),
            zoom: self.view.target_scale(),
            at_fit: self.view.mode == ZoomMode::Fit,
            fullscreen: self.fullscreen,
            file_name: name.as_deref(),
            dims,
            loading,
            error: error.as_deref(),
            truncated,
            metadata: metadata.as_deref(),
            hud: &self.hud,
            keymap: &self.keymap,
            store: &self.store,
            settings: &self.settings,
            overlays,
            playback,
            motion,
            spread,
            pages: pages.as_ref().map(|p| specular_widgets::PageBadge {
                index: p.0,
                count: p.1,
                label: p.2.as_deref(),
            }),
        };
        if let Some((lines, paused)) = &slideshow {
            let area = (vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            slideshow::draw_overlay(&mut self.dl, &mut gfx.text, lines, *paused, area, rtl, ts);
        }
        let cull_hits = cull_strip.as_ref().map(|s| {
            let area = (vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            cull::draw_strip(&mut self.dl, &mut gfx.text, s, area, rtl, ts)
        });
        if let Some((bar, hits)) = &cull_hits {
            self.cull.set_hits(*bar, hits.clone());
        }
        // The subframe table and chart, above the strip.
        let cull_panel = cull_hits.as_ref().zip(cull_model).map(|((bar, _), p)| {
            let area = (vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            let drawn =
                cull::astro::draw_panel(&mut self.dl, &mut gfx.text, &p, area, bar.y, rtl, ts);
            (p, drawn)
        });
        self.cull
            .set_panel(cull_panel.as_ref().map(|(_, d)| d.clone()));
        // The keyword panel over cull mode's photo, above the strip.
        if let Some((bar, _)) = &cull_hits
            && self.gallery.keywords.open
        {
            let area = (vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            let r = keywords::cull_rect(area, bar.y, ts, rtl);
            let ls = self.dl.scale.max(0.1);
            let lp = self.pointer.map(|p| (p.0 / ls, p.1 / ls));
            self.gallery
                .keywords
                .draw(&mut self.dl, &mut gfx.text, &pal, r, ts, rtl, lp);
        }
        // Tethered capture's panel, over any mode.
        let tether_drawn = tether_panel.as_ref().map(|p| {
            let area = (vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            tether::draw_panel(&mut self.dl, &mut gfx.text, &pal, p, area, rtl, ts)
        });
        self.tether.set_drawn(tether_drawn.clone());
        // The metadata editor over cull mode's photo (above the strip) or the viewer's image.
        if self.gallery.meta_edit.open && !self.gallery.active {
            let area = (vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            let r = match &cull_hits {
                Some((bar, _)) => keywords::cull_rect(area, bar.y, ts, rtl),
                None => {
                    let bottom = area.1 + area.3
                        - (specular_theme::metrics::TOOLBAR_MARGIN
                            + specular_theme::metrics::TOOLBAR_HEIGHT
                            + 12.0);
                    meta_edit::viewer_rect(area, bottom, ts, rtl)
                }
            };
            let ls = self.dl.scale.max(0.1);
            let lp = self.pointer.map(|p| (p.0 / ls, p.1 / ls));
            self.gallery
                .meta_edit
                .draw(&mut self.dl, &mut gfx.text, &pal, r, ts, rtl, lp);
        }
        // The strip may have moved under a resting pointer; the hand shows over its buttons.
        let ls = self.dl.scale.max(0.1);
        let lp = self.pointer.map(|p| (p.0 / ls, p.1 / ls));
        if self.cull.hover_at(lp, now) {
            self.redraw = true;
        }
        let icon = pointer::icon(self.cull.over_button());
        if icon != self.cursor_icon {
            gfx.window.set_cursor(icon);
            self.cursor_icon = icon;
        }
        if measurements.is_some() || medical_marks.is_some() {
            let clip =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            self.dl.push_clip(clip);
            if let Some((scene, _)) = &medical_marks {
                medical::present::draw(&mut self.dl, &mut gfx.text, scene, ts);
            }
            if let Some(m) = &measurements {
                medical::measure::draw(&mut self.dl, &mut gfx.text, m, rtl, ts);
            }
            if let Some((_, edges)) = &medical_marks {
                medical::labels::draw(&mut self.dl, &mut gfx.text, edges, ts);
            }
            self.dl.pop_clip();
        }
        if let Some(c) = &medical_curve {
            let area = (vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            medical::analysis::draw_curve(&mut self.dl, &mut gfx.text, &pal, c, area, rtl, ts);
        }
        // The masks for burned-in text, with their tool bar.
        if let Some(v) = &mask_view {
            let area =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            let d = anon_export::draw_mask(&mut self.dl, &mut gfx.text, &pal, v, area, rtl, ts);
            self.anon.set_mask_drawn(d);
        }
        // Two sheets compared: the composed picture over the image, the changed regions and
        // the bar.
        self.sheets.upload(&gfx.gpu, &mut gfx.ui);
        if let Some(v) = &sheets_view {
            let area =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            if let Some((id, r, uv)) =
                sheet_compare::picture(&self.sheets, &self.view, self.dl.scale)
            {
                self.dl.push_clip(area);
                self.dl.image(r, uv, id, 0.0);
                self.dl.pop_clip();
            }
            let ts = self.chrome.text_scale;
            let d = sheet_compare::draw(&mut self.dl, &mut gfx.text, &pal, v, area, rtl, ts);
            self.sheets.set_drawn(d);
        }
        // Develop: the developed picture, the filmstrip, the panel, the speed-edit readout.
        {
            let image =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let window = specular_gfx::Rect::new(0.0, 0.0, pw as f32 / scale, ph as f32 / scale);
            let held = self.hold.map(|h| h.0);
            self.develop.set_display_peak(gfx.renderer.display_peak());
            develop::draw(
                &mut self.develop,
                &mut self.dl,
                &mut gfx.text,
                &gfx.gpu,
                &mut gfx.ui,
                &pal,
                &self.view,
                image,
                window,
                &self.entries,
                self.current,
                &self.loader,
                held,
                rtl,
                self.chrome.text_scale,
                now,
            );
        }
        // The equaliser's panel at the window's edge (`sound_fx.rs`).
        if let Some(p) = &mut self.sound_fx.eq {
            let window = specular_gfx::Rect::new(0.0, 0.0, pw as f32 / scale, ph as f32 / scale);
            p.draw(
                &mut self.dl,
                &mut gfx.text,
                &pal,
                window,
                rtl,
                self.chrome.text_scale,
            );
        }
        // The markup, with the tool's bar and list when it is open.
        if let Some(v) = &markup_view {
            let area =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            let d = markup::draw(&mut self.dl, &mut gfx.text, &pal, v, area, rtl, ts);
            self.markup.set_drawn(d);
        }
        // The measurements, with the tool's bar and list when it is open.
        if let Some(v) = &measure_view {
            let area =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            let d = measure_tool::draw(&mut self.dl, &mut gfx.text, &pal, v, area, rtl, ts);
            self.measure.set_drawn(d);
        }
        if key_badge {
            let area =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            figure::draw_badge(&mut self.dl, &mut gfx.text, &pal, area, rtl, ts);
        }
        if let Some(q) = &quiz_card {
            let area =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            quiz::draw(&mut self.dl, &mut gfx.text, &pal, q, area, rtl, ts);
        }
        if let Some(p) = crosshair {
            let clip =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            cursor::draw_crosshair(&mut self.dl, &pal, (p.0 / scale, p.1 / scale), clip);
        }
        App::mpr_overlay(
            &mut self.mpr,
            &mut self.dl,
            &mut gfx.text,
            &pal,
            self.settings.medical.orientation_labels,
            rtl,
            self.chrome.text_scale,
        );
        if !self.gallery.active {
            App::linked_overlay(
                &mut self.linked,
                &mut self.dl,
                &gfx.gpu,
                &mut gfx.ui,
                &mut gfx.text,
                &pal,
                self.settings.medical.orientation_labels,
                rtl,
                self.chrome.text_scale,
            );
        }
        let medical_box = medical_lines.as_ref().map(|l| {
            let area = (vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            let bar = self.chrome.bottom_bar();
            medical::draw_readout_clear(&mut self.dl, &mut gfx.text, l, area, rtl, ts, bar)
        });
        let mesh_box = mesh_lines.as_ref().map(|l| {
            let area = (vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let ts = self.chrome.text_scale;
            let bar = self.chrome.bottom_bar();
            medical::draw_readout_clear(&mut self.dl, &mut gfx.text, l, area, rtl, ts, bar)
        });
        // The light widget of a relightable image.
        let relight_box = {
            let area =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            let r = relight_look.as_ref().map(|look| {
                let r = relight::widget_rect(area, rtl);
                relight::draw_widget(&mut self.dl, r, look);
                r
            });
            self.relight.drawn(area, r);
            r
        };
        // Science mode: the grid under the panels, then the readout, sliders and scale bar.
        let ts = self.chrome.text_scale;
        let area = (vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
        // A sky survey around the image (clear where the image is), and its credit.
        if let Some(o) = science_overlay.as_ref() {
            let r = specular_gfx::Rect::new(area.0, area.1, area.2, area.3);
            science::astro::draw_survey(&mut self.dl, &mut gfx.text, &o.astro, r, ts, rtl);
        }
        if let Some(o) = science_overlay.as_ref().filter(|o| {
            !o.grid.is_empty()
                || o.wiggle.is_some()
                || !o.edges.is_empty()
                || !o.shapes.is_empty()
                || !o.contours.is_empty()
                || !o.markers.is_empty()
        }) {
            let ir = self.view.image_screen_rect();
            let x0 = ir.x.max(vp.x) / scale;
            let y0 = ir.y.max(vp.y) / scale;
            let x1 = (ir.x + ir.w).min(vp.x + vp.w) / scale;
            let y1 = (ir.y + ir.h).min(vp.y + vp.h) / scale;
            if x1 > x0 && y1 > y0 {
                let clip = specular_gfx::Rect::new(x0, y0, x1 - x0, y1 - y0);
                if let Some(w) = &o.wiggle {
                    science::draw_wiggles(&mut self.dl, w, clip);
                }
                science::draw_edges(&mut self.dl, &o.edges, clip);
                let ink = science::GRID_INK;
                science::draw_grid(&mut self.dl, &mut gfx.text, &o.grid, clip, ts, ink);
                let ink = science::CONTOUR_INK;
                science::draw_grid(&mut self.dl, &mut gfx.text, &o.contours, clip, ts, ink);
                annotations::draw(&mut self.dl, &mut gfx.text, &o.shapes, clip, ts);
                science::draw_markers(&mut self.dl, &o.markers, clip, o.hovered);
            }
        }
        // DS9 regions and the sources other SAMP applications highlighted.
        if let Some(o) = science_overlay
            .as_ref()
            .filter(|o| !o.astro.regions.is_empty() || !o.astro.highlights.is_empty())
        {
            let clip =
                specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
            science::astro::draw_regions(&mut self.dl, &mut gfx.text, &o.astro.regions, clip, ts);
            science::astro::draw_highlights(&mut self.dl, &o.astro.highlights, clip);
        }
        // Imported slide annotations (`pathology.rs`), drawn like a sidecar's.
        if let Some(set) = imported_set {
            let ir = self.view.image_screen_rect();
            let x0 = ir.x.max(vp.x) / scale;
            let y0 = ir.y.max(vp.y) / scale;
            let x1 = (ir.x + ir.w).min(vp.x + vp.w) / scale;
            let y1 = (ir.y + ir.h).min(vp.y + vp.h) / scale;
            if x1 > x0 && y1 > y0 {
                let clip = specular_gfx::Rect::new(x0, y0, x1 - x0, y1 - y0);
                let to_screen = self.view.stored_from_screen().invert();
                let view =
                    specular_gfx::Rect::new(vp.x / scale, vp.y / scale, vp.w / scale, vp.h / scale);
                let shapes = annotations::place(
                    &set,
                    |p| {
                        let q = to_screen.apply(p);
                        (q.0 / scale, q.1 / scale)
                    },
                    view,
                    |_| None,
                );
                annotations::draw(&mut self.dl, &mut gfx.text, &shapes, clip, ts);
            }
        }
        // The stains' legend while they are separated.
        self.pathology.legend = stain_legend.as_ref().map(|v| {
            pathology::draw_legend(
                &mut self.dl,
                &mut gfx.text,
                &pal,
                v,
                specular_gfx::Rect::new(area.0, area.1, area.2, area.3),
                rtl,
                ts,
            )
        });
        if !script_shapes.is_empty() {
            let ir = self.view.image_screen_rect();
            // A little past the image: points on its edge keep their halo.
            let x0 = (ir.x / scale - 12.0).max(vp.x / scale);
            let y0 = (ir.y / scale - 12.0).max(vp.y / scale);
            let x1 = ((ir.x + ir.w) / scale + 12.0).min((vp.x + vp.w) / scale);
            let y1 = ((ir.y + ir.h) / scale + 12.0).min((vp.y + vp.h) / scale);
            if x1 > x0 && y1 > y0 {
                let clip = specular_gfx::Rect::new(x0, y0, x1 - x0, y1 - y0);
                script::draw(&mut self.dl, &mut gfx.text, &script_shapes, clip, ts);
            }
        }
        // A plot's axes, ticks and marks over its traces.
        if let Some(v) = &chart_view {
            chart::draw(&mut self.dl, &mut gfx.text, &pal, v, area, ts);
        }
        if let Some(o) = &oct_inset {
            oct::draw_inset(&mut self.dl, o, scale);
        }
        // A map's attribute table along the bottom: the readout and scale bar sit above it.
        let low = map_floor.map_or(area, |f| (area.0, area.1, area.2, (f - area.1).max(0.0)));
        let science_box = science_lines.as_ref().map(|l| {
            let bar = self.chrome.bottom_bar();
            medical::draw_readout_clear(&mut self.dl, &mut gfx.text, l, low, rtl, ts, bar)
        });
        self.science.seq.drawn = match &thermal_seq_panel {
            Some(p) => science::thermal_seq::draw(
                &mut self.dl,
                &mut gfx.text,
                &pal,
                p,
                area,
                science_box,
                rtl,
                ts,
            ),
            None => Default::default(),
        };
        if let Some(plot) = science_overlay.as_ref().and_then(|o| o.spectrum.as_ref()) {
            science::draw_spectrum(
                &mut self.dl,
                &mut gfx.text,
                &pal,
                plot,
                area,
                science_box,
                rtl,
                ts,
            );
        }
        let science_sliders = science_overlay
            .as_ref()
            .filter(|o| !o.sliders.is_empty())
            .map(|o| {
                science::draw_sliders(&mut self.dl, &mut gfx.text, &pal, &o.sliders, low, rtl, ts)
            });
        let science_scale = science_overlay
            .as_ref()
            .and_then(|o| o.scale.as_ref())
            .map(|b| {
                let below = science_sliders.as_ref().map(|s| s.0);
                science::draw_scale(&mut self.dl, &mut gfx.text, &pal, b, low, below, rtl, ts)
            });
        // The inspection panels (histogram, pixel, colour bar) share the bottom start corner
        // with the sliders and the scale bar: they stack above them.
        self.chrome.inspect.floor = science_sliders
            .as_ref()
            .map(|s| s.0)
            .into_iter()
            .chain(science_scale)
            .map(|r| r.y)
            .reduce(f32::min);
        if let Some(tip) = science_overlay.as_ref().and_then(|o| o.tip.as_ref()) {
            science::draw_tip(&mut self.dl, &mut gfx.text, tip, area, ts);
        }
        if let Some(o) = &pmi_overlay {
            let mut o = o.clone();
            o.avoid = bim_panel.as_ref().and(self.mesh.bim.panel_rect);
            mesh::pmi::draw_overlay(&mut self.dl, &mut gfx.text, &pal, &o, ts);
        }
        if let Some(o) = &bim_overlay {
            mesh::bim::draw_overlay(&mut self.dl, &pal, o, ts);
        }
        let bim_drawn = bim_panel
            .as_ref()
            .map(|p| mesh::bim::draw_panel(&mut self.dl, &mut gfx.text, &pal, p, area, rtl, ts));
        let cad_drawn = cad_panel
            .as_ref()
            .map(|p| cad::draw_panel(&mut self.dl, &mut gfx.text, &pal, p, area, rtl, ts));
        self.cad.set_drawn(cad_drawn.clone());
        match &bim_drawn {
            Some((r, rows, ime, field)) => {
                self.mesh.bim.panel_rect = Some(*r);
                self.mesh.bim.rows = rows.clone();
                self.mesh.bim.ime = *ime;
                self.mesh.bim.field = field.clone();
            }
            None => {
                self.mesh.bim.panel_rect = None;
                self.mesh.bim.rows.clear();
                self.mesh.bim.ime = None;
                self.mesh.bim.field = None;
            }
        }
        let contents_drawn = contents_panel.as_ref().map(|p| {
            let bottom = self
                .chrome
                .bottom_bar()
                .map_or(0.0, |b| (area.1 + area.3 - b.y).max(0.0));
            contents::draw_panel(&mut self.dl, &mut gfx.text, &pal, p, area, bottom, rtl, ts)
        });
        self.contents.set_drawn(contents_drawn.clone());
        let map_drawn = map_view.as_ref().map(|v| {
            map::draw(
                &mut self.dl,
                &mut gfx.text,
                &pal,
                &self.map,
                v,
                area,
                rtl,
                ts,
            )
        });
        if let Some(d) = map_drawn {
            self.map.set_drawn(d);
        }
        if science_overlay.is_some() {
            let tracks = science_sliders
                .as_ref()
                .map(|s| s.1.clone())
                .unwrap_or_default();
            if oct_slider {
                self.report.oct.set_track(tracks.first().map(|t| t.1));
            } else {
                self.science.tracks = tracks;
            }
        }
        if self.welcome.upload(&gfx.gpu, &mut gfx.ui) {
            self.redraw = true;
        }
        if self.studies.upload(&gfx.gpu, &mut gfx.ui) && self.chrome.studies_open() {
            self.chrome.set_studies(self.studies.sheet());
        }
        // Thumbnails that came after the sheet's view was built show next frame.
        if self.omero.upload(&gfx.gpu, &mut gfx.ui) {
            self.redraw = true;
        }
        // Where the picture is (logical): a movie's bars only make room when they would cover it.
        let picture = self.shown.is_some().then(|| {
            let ir = self.view.image_screen_rect();
            specular_gfx::Rect::new(ir.x / scale, ir.y / scale, ir.w / scale, ir.h / scale)
        });
        self.chrome.set_picture(picture);
        let wake_chrome = self
            .chrome
            .draw(&mut self.dl, &mut gfx.text, &st, &pal, now);
        let print_view = self.print.draw(
            &mut self.dl,
            &gfx.gpu,
            &mut gfx.ui,
            &mut gfx.text,
            &pal,
            (pw as f32 / scale, ph as f32 / scale),
            scale,
            rtl,
            self.chrome.text_scale,
        );
        let convert_view = self.convert.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        let recipes_view = self.export_recipes.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        let fileops_view = self.fileops.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        self.video_export.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            export_view.as_ref(),
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        let anon_view = self.anon.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        self.annot_export.drawn = annot_view.as_ref().map(|v| {
            annot_export::draw(
                &mut self.dl,
                &mut gfx.text,
                &pal,
                v,
                (pw as f32 / scale, ph as f32 / scale),
                rtl,
                self.chrome.text_scale,
            )
        });
        let figure_view = self.figure.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        self.dicomweb.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            dicomweb_view.as_ref(),
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        self.omero.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            omero_view.as_ref(),
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        self.raw_volume.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            raw_volume_view.as_ref(),
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        self.file_kind.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            file_kind_view.as_ref(),
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        self.password.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            password_view.as_ref(),
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        if let (Some(v), Some(d)) = (&thermal_report_view, self.thermal_report.dialog.as_mut()) {
            d.drawn = science::thermal_report::view::draw(
                &mut self.dl,
                &mut gfx.text,
                &pal,
                v,
                (pw as f32 / scale, ph as f32 / scale),
                rtl,
                self.chrome.text_scale,
            );
        }
        if let Some(v) = &web_gallery_view {
            self.web_gallery.drawn = web_gallery::view::draw(
                &mut self.dl,
                &mut gfx.text,
                &pal,
                v,
                (pw as f32 / scale, ph as f32 / scale),
                rtl,
                self.chrome.text_scale,
            );
        }
        self.address.draw(
            &mut self.dl,
            &mut gfx.text,
            &pal,
            address_view.as_ref(),
            (pw as f32 / scale, ph as f32 / scale),
            rtl,
            self.chrome.text_scale,
        );
        self.chrome_commands.extend(self.chrome.take_commands());
        if let (Some(title), Some(a)) = (a11y_title, self.a11y.as_mut()) {
            let tree = self.chrome.accessibility_tree(
                &st,
                &title,
                gfx.window.scale_factor() * self.ui_scale as f64,
            );
            let mut tree = tree;
            if self.gallery.active {
                self.gallery
                    .a11y(&mut tree, gfx.window.scale_factor() * self.ui_scale as f64);
            }
            if let (Some(s), Some((bar, hits))) = (&cull_strip, &cull_hits) {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                cull::a11y(&mut tree, s, Some(*bar), hits, sf);
            }
            if !self.chrome.menu_open() {
                self.develop.menu_closed();
            }
            develop::a11y(
                &self.develop,
                &mut tree,
                &self.entries,
                self.current,
                gfx.window.scale_factor() * self.ui_scale as f64,
            );
            if let Some(p) = &self.sound_fx.eq {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                p.a11y(&mut tree, sound_fx::A11Y_EQ, sf);
            }
            if let Some((p, (r, hits))) = &cull_panel {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                cull::astro::a11y(&mut tree, p, *r, hits, sf);
            }
            if self.gallery.keywords.open {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.gallery.keywords.a11y(&mut tree, sf);
            }
            if self.gallery.meta_edit.open {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.gallery.meta_edit.a11y(&mut tree, sf);
            }
            if let (Some(l), Some(r)) = (&medical_lines, medical_box) {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                medical::a11y(&mut tree, l, r, sf);
            }
            if let (Some(l), Some(r)) = (&mesh_lines, mesh_box) {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                mesh::a11y(&mut tree, l, r, sf);
            }
            if let (Some(look), Some(r)) = (&relight_look, relight_box) {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                relight::a11y(&mut tree, look, r, sf);
            }
            if let Some(p) = &thermal_seq_panel {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                let frame = self.science.index.first().copied().unwrap_or(0);
                science::thermal_seq::a11y(
                    &mut tree,
                    p,
                    &self.science.seq.drawn,
                    frame as usize,
                    sf,
                );
            }
            if science_overlay.is_some() || science_box.is_some() {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                science::a11y(
                    &mut tree,
                    science_lines.as_deref().zip(science_box),
                    science_overlay.as_ref().zip(science_sliders.as_ref()).map(
                        |(o, (panel, tracks))| (*panel, o.sliders.as_slice(), tracks.as_slice()),
                    ),
                    science_overlay
                        .as_ref()
                        .and_then(|o| o.scale.as_ref())
                        .zip(science_scale),
                    sf,
                );
            }
            if let (Some(p), Some((panel, rows, ..))) = (&bim_panel, &bim_drawn) {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                mesh::bim::a11y(&mut tree, p, *panel, rows, sf);
            }
            if let Some(o) = &bim_overlay {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                mesh::bim::handle_nodes(&mut tree, &o.handles, rtl, sf);
            }
            if let (Some(p), Some((panel, rows, _))) = (&cad_panel, &cad_drawn) {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                cad::a11y(&mut tree, p, *panel, rows, sf);
            }
            if let (Some(p), Some((panel, hits))) = (&tether_panel, &tether_drawn) {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                tether::a11y(&mut tree, p, *panel, hits, sf);
            }
            if let (Some(p), Some((panel, rows, _))) = (&contents_panel, &contents_drawn) {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                contents::a11y(&mut tree, p, *panel, rows, sf);
            }
            if self.font_view.active() && !gallery_on {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                font_view::a11y(&mut tree, &self.font_view, sf);
            }
            if self.text_view.active() && !gallery_on {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                text_view::a11y(&mut tree, &self.text_view, sf);
            }
            if self.sound_page.active() && !gallery_on {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                sound_page::a11y(&mut tree, &self.sound_page, sf);
            }
            if let Some(v) = &map_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                map::a11y(&mut tree, &self.map, v, sf);
            }
            if let Some((_, edges)) = &medical_marks {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                medical::labels::a11y(&mut tree, edges, sf);
            }
            {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                let labels = self.settings.medical.orientation_labels;
                App::mpr_a11y(&self.mpr, &mut tree, labels, sf, f64::from(scale));
                App::linked_a11y(&self.linked, &mut tree, sf);
            }
            if let Some(v) = &print_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.print.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &convert_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.convert.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &recipes_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.export_recipes.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &fileops_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.fileops.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &export_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.video_export.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &mask_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.anon.mask_a11y(&mut tree, v, sf);
            }
            if let Some(v) = &measure_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.measure.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &markup_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.markup.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &sheets_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.sheets.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &anon_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.anon.a11y(&mut tree, v, sf);
            }
            if let (Some(v), Some(d)) = (&stain_legend, &self.pathology.legend) {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                pathology::legend_a11y(&mut tree, v, d, sf);
            }
            if let (Some(v), Some(d)) = (&annot_view, &self.annot_export.drawn) {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                annot_export::a11y(&mut tree, v, d, sf);
            }
            if let Some(v) = &figure_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.figure.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &dicomweb_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.dicomweb.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &omero_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.omero.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &raw_volume_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.raw_volume.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &file_kind_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.file_kind.a11y(&mut tree, v, sf);
            }
            if let (Some(v), Some(d)) = (&thermal_report_view, self.thermal_report.dialog.as_ref())
            {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                science::thermal_report::view::a11y(&mut tree, v, &d.drawn, sf);
            }
            if let Some(v) = &web_gallery_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                web_gallery::view::a11y(&mut tree, v, &self.web_gallery.drawn, sf);
            }
            if let Some(v) = &password_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.password.a11y(&mut tree, v, sf);
            }
            if let Some(v) = &address_view {
                let sf = gfx.window.scale_factor() * self.ui_scale as f64;
                self.address.a11y(&mut tree, v, sf);
            }
            a.update(tree);
        }
        let field = self
            .thermal_report
            .ime_area()
            .or_else(|| self.web_gallery.ime_area())
            .or_else(|| self.fileops.ime_area())
            .or_else(|| self.convert.ime_area())
            .or_else(|| self.export_recipes.ime_area())
            .or_else(|| self.dicomweb.ime_area())
            .or_else(|| self.omero.ime_area())
            .or_else(|| self.address.ime_area())
            .or_else(|| self.figure.ime_area())
            .or_else(|| self.mesh.bim.ime_area())
            .or_else(|| self.mpr.rename_ime_area())
            .or_else(|| self.markup.ime_area())
            .or(map_ime)
            .or_else(|| self.chrome.ime_cursor_area())
            .or_else(|| self.gallery.ime_area());
        if field != self.ime {
            debug!(target: INPUT, "ime area {field:?}");
            if field.is_some() != self.ime.is_some() {
                gfx.window.set_ime_allowed(field.is_some());
            }
            if let Some(r) = field {
                gfx.window.set_ime_cursor_area(
                    winit::dpi::LogicalPosition::new(r.x as f64, r.y as f64),
                    winit::dpi::LogicalSize::new(r.w as f64, r.h as f64),
                );
            }
            self.ime = field;
        }
        let hide = pointer_hide.hide();
        if hide != self.cursor_hidden {
            gfx.window.set_cursor_visible(!hide);
            self.cursor_hidden = hide;
        }

        // Scene.
        let lin = canvas.to_linear_premul();
        let mut opts = RenderOptions {
            hq: !animating && !self.view.is_dragging(),
            canvas: lin,
            ..RenderOptions::default()
        };
        opts.grid = self
            .pixel_grid
            .then(|| Color::rgba(0.5, 0.5, 0.5, 0.35).to_linear_premul());
        opts.grid_from = self.settings.viewing.pixel_grid_threshold as f32 / 100.0;
        let channel = inspect::apply_backdrop(
            &mut opts,
            view_backdrop,
            scale,
            &self.chrome,
            &self.settings,
            has_alpha,
        );
        let mut backdrop = [0u32; 8];
        for (i, v) in opts.checker_a.iter().chain(&opts.checker_b).enumerate() {
            backdrop[i] = v.to_bits();
        }
        opts.nearest_from = match self.settings.viewing.smoothing_high_zoom {
            HighZoomSmoothing::Auto => 4.0,
            HighZoomSmoothing::Smooth => f32::INFINITY,
            HighZoomSmoothing::Pixelated => 1.0001,
        };
        opts.gsdf = medical::gsdf_display(&self.settings.medical);
        opts.stain = stain_view;
        if let Some((l, texels)) = &ocio_upload {
            gfx.renderer.upload_ocio(&gfx.gpu, l.id, l.size, texels);
        }
        opts.ocio = ocio_lut;
        opts.picture = picture_adjust;
        let size = gfx.surface.size();
        let (spread_rev, spread_pixels) =
            reading::spread_rev(&mut self.gpu_images, spread_views.as_deref());
        let oct_key = oct_inset.as_ref().map(|o| {
            let rev = self.gpu_images.get(o.id).map_or(u64::MAX, |g| g.revision);
            let r = o.rect;
            (
                o.id,
                rev,
                [r.x.to_bits(), r.y.to_bits(), r.w.to_bits(), r.h.to_bits()],
            )
        });
        let img = cur
            .filter(|id| {
                self.shown == Some(*id) && spread_views.is_none() && mpr_rev == 0 && mesh_rev == 0
            })
            .and_then(|id| self.gpu_images.get(id));
        let inspect_scene = inspect::scene(
            &self.chrome,
            &self.settings,
            &self.view,
            self.pointer,
            img.is_some(),
            scale,
        );
        // The levels the scene reads: the view's (one finer, in case the shader rounds the
        // other way) and coarser; the pixel inspector's loupe reads full resolution.
        let from = img.as_ref().map_or(0, |g| {
            if inspect_scene.has_loupe() {
                0
            } else {
                ImageRenderer::view_level(g, &self.view).saturating_sub(1)
            }
        });
        let key = SceneKey {
            id: img.as_ref().map(|g| g.id),
            image_rev: img.as_ref().map_or(spread_rev, |g| g.revision_from(from)),
            view_rev: self.view.revision,
            size,
            canvas: lin.map(f32::to_bits),
            hq: opts.hq,
            grid: self.pixel_grid,
            tiles: img.as_ref().map_or(0, |g| gfx.renderer.tiles_from(g, from)),
            navigator: nav_view.is_some(),
            inspect: inspect_scene,
            channel: (channel, backdrop),
            aids: (opts.false_colour, opts.channel_colours, opts.hilo),
            stain: opts.stain,
            ocio: opts.ocio.map(|o| o.id),
            mpr: mpr_rev,
            medical: medical_plane.as_ref().map_or((0, 0), |_| {
                (self.medical.an.rev, self.medical.index_shown())
            }),
            cad: cad_frame,
            map: map_frame,
            mesh: mesh_rev,
            oct: oct_key,
        };
        let shown_pixels =
            img.as_ref().is_some_and(|g| g.overview.is_some()) || spread_pixels || mesh_rev != 0;
        // Never a blank frame (DESIGN.md §3.2): until the new current image has pixels, the
        // scene keeps showing the previous image, for up to `STAND_IN` after the navigation or
        // for as long as → or ← is held. The chrome (name, position) still updates.
        let stand_in = !shown_pixels
            && self.scene_pixels == Some((size, key.canvas))
            && spread_views.is_none()
            && cur
                .and_then(|id| self.images.get(&id))
                .is_some_and(|i| stands_in(i, self.flipping));
        // A page switched to holds the old page only until it has pixels.
        let page_hold = cur
            .and_then(|id| self.images.get_mut(&id))
            .is_some_and(|i| {
                i.page_hold &= !shown_pixels;
                i.page_hold
            });
        let t_acquire = Instant::now();
        let frame = match gfx.surface.acquire(&gfx.gpu) {
            Frame::Ready(f) => f,
            Frame::Skip => {
                self.redraw = true;
                return;
            }
        };
        let acquire_ms = t_acquire.elapsed().as_secs_f64() * 1000.0;
        let t_scene = Instant::now();
        let target = frame.texture.create_view(&Default::default());
        let mut enc = gfx
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        if self.scene != Some(key) && !stand_in {
            // The slideshow keeps the outgoing slide for its cross-fade.
            self.slideshow.before_scene(
                &gfx.gpu,
                &mut enc,
                &gfx.comp,
                self.scene.and_then(|s| s.id),
                key.id,
                shown_pixels && self.scene_pixels == Some((size, key.canvas)),
                now,
            );
            self.scene_pixels = shown_pixels.then_some((size, key.canvas));
            gfx.renderer.render(
                &gfx.gpu,
                &mut enc,
                &gfx.comp.scene.view,
                size,
                img.as_deref(),
                &self.view,
                &opts,
            );
            if let Some(plane) = &medical_plane
                && mpr_rev == 0
            {
                self.medical.an.render(
                    &gfx.gpu,
                    &mut enc,
                    &gfx.comp.scene.view,
                    size,
                    plane,
                    f64::from(scale),
                );
            }
            // A drawing's vectors over its CPU picture, at the exact zoom.
            if let Some(cf) = cad_frame.filter(|c| key.id == Some(c.id)) {
                cad::render(
                    &mut self.cad,
                    &gfx.gpu,
                    &mut enc,
                    &gfx.comp.scene.view,
                    size,
                    &self.view,
                    &cf,
                    f64::from(scale),
                );
            }
            // A map's layers over its picture, with the layer panel's styles.
            if let Some(mf) = map_frame.filter(|m| key.id == Some(m.id))
                && let Some(r) = self.cad.renderer_mut()
            {
                map::render(
                    &mut self.map,
                    r,
                    &gfx.gpu,
                    &mut enc,
                    &gfx.comp.scene.view,
                    size,
                    &self.view,
                    &mf,
                    f64::from(scale),
                );
            }
            // The 3D view refines over frames; grey planes on the 2D view's display function.
            self.mpr.gsdf = opts.gsdf;
            if mpr_rev != 0
                && App::mpr_render(
                    &mut self.mpr,
                    &gfx.gpu,
                    &mut enc,
                    &gfx.comp.scene.view,
                    size,
                    f64::from(scale),
                    lin,
                )
            {
                self.redraw = true;
            }
            // A point cloud fills in as it loads.
            if mesh_rev != 0
                && App::mesh_render(
                    &mut self.mesh,
                    &gfx.gpu,
                    &mut enc,
                    &gfx.comp.scene.view,
                    size,
                    scale,
                )
            {
                self.redraw = true;
            }
            if let Some(g) = img.as_deref() {
                inspect::draw_scene(
                    &mut self.inspect,
                    gfx,
                    &mut enc,
                    size,
                    g,
                    &self.view,
                    &opts,
                    &key.inspect,
                );
                let inset_opts = RenderOptions {
                    hq: true,
                    grid: None,
                    ..opts
                };
                if let Some(nv) = &nav_view {
                    gfx.renderer.render_inset(
                        &gfx.gpu,
                        &mut enc,
                        &gfx.comp.scene.view,
                        size,
                        g,
                        nv,
                        &inset_opts,
                        None,
                    );
                }
            }
            if let Some(o) = oct_inset.as_ref().filter(|_| img.is_some())
                && let Some(g) = self.gpu_images.get(o.id)
            {
                let inset_opts = oct::inset_options(&opts);
                gfx.renderer.render_inset(
                    &gfx.gpu,
                    &mut enc,
                    &gfx.comp.scene.view,
                    size,
                    g,
                    &o.view,
                    &inset_opts,
                    None,
                );
            }
            if let Some(views) = &spread_views {
                reading::render_spread(
                    &mut gfx.renderer,
                    &gfx.gpu,
                    &mut enc,
                    &gfx.comp.scene.view,
                    size,
                    &mut self.gpu_images,
                    views,
                    &opts,
                );
            }
            self.scene = Some(key);
            self.tile_stats.scenes += 1;
            self.blur_valid = false;
        }
        if let Some(plan) = &quick {
            gallery::render_quick(&self.gallery, gfx, &mut self.gpu_images, &mut enc, plan);
        }
        self.gallery.render_map(&gfx.gpu, &mut enc);
        self.sound_page.render(&gfx.gpu, &mut enc);
        if self.chrome.uses_glass() && !self.blur_valid {
            gfx.comp.blur(&mut enc);
            self.blur_valid = true;
        }
        // The colour-vision simulation, in the final pass only (exports never see it).
        let cvd = self.chrome.inspect.tools.cvd.matrix();
        if gfx.comp.cvd() != cvd {
            gfx.comp.set_cvd(&gfx.gpu, cvd);
        }
        let loupe = loupe_view
            .as_ref()
            .filter(|_| !stand_in && key.id.is_some())
            .and_then(|l| Some((l, cur.and_then(|id| self.gpu_images.get(id))?)));
        // With nothing drawn over the scene (no chrome, loupe or fade), the final pass reads
        // the scene itself: no full-window copy (66 MB read and written at 4K).
        let over = loupe.is_some() || self.slideshow.fade_started() || !self.dl.prims.is_empty();
        if over {
            gfx.comp.begin(&mut enc);
        }
        // The loupe over the scene's copy: following the pointer redraws just the disc (the
        // scene at fit of a large image is the costly part of a frame).
        if let Some(((lv, c, r), g)) = loupe {
            gfx.renderer.render_inset(
                &gfx.gpu,
                &mut enc,
                &gfx.comp.comp.view,
                size,
                g,
                lv,
                &opts,
                Some((*c, *r)),
            );
        }
        let fading = self.slideshow.draw_fade(&mut enc, &gfx.comp, now);
        let t_ui = Instant::now();
        gfx.ui.render(
            &gfx.gpu,
            &mut enc,
            &gfx.comp.comp.view,
            size,
            &self.dl,
            &mut gfx.text,
        );
        gfx.comp.finish_from(&mut enc, &target, over);
        let readback = capture.then(|| {
            let fmt = gfx.surface.format();
            let tex = gfx.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("screenshot"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: fmt,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            gfx.comp
                .finish_from(&mut enc, &tex.create_view(&Default::default()), over);
            let row = (size.0 * 4).div_ceil(256) * 256;
            let buf = gfx.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("screenshot"),
                size: row as u64 * size.1 as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            enc.copy_texture_to_buffer(
                tex.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(row),
                        rows_per_image: Some(size.1),
                    },
                },
                wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
            );
            (buf, row, fmt)
        });
        let t_submit = Instant::now();
        gfx.gpu.queue.submit([enc.finish()]);
        gfx.window.pre_present_notify();
        let t_present = Instant::now();
        gfx.gpu.queue.present(frame);
        let presented = Instant::now();
        let cpu_to_present = t_frame.elapsed().as_secs_f64() * 1e3;
        if let Some(a) = self.anim.as_mut() {
            a.presented(Instant::now());
        }
        let total = t_frame.elapsed();
        {
            let ms = |a: Instant, b: Instant| (b - a).as_secs_f32() * 1000.0;
            let now = Instant::now();
            self.gallery.bench_render([
                ms(t_frame, t_acquire),
                ms(t_scene, t_ui),
                ms(t_ui, t_submit),
                ms(t_submit, t_present),
                ms(t_present, now),
            ]);
        }
        self.gallery.scroll_bench_frame(t_frame, total, acquire_ms);
        if total > Duration::from_millis(33) {
            let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1000.0;
            debug!(target: RENDER, "slow frame {:.1} ms: before acquire {:.1}, acquire {acquire_ms:.1}, image {:.1}, ui {:.1}, submit {:.1}, present {:.1}",
                total.as_secs_f64() * 1000.0, ms(t_frame, t_acquire), ms(t_scene, t_ui), ms(t_ui, t_submit), ms(t_submit, t_present), ms(t_present, Instant::now()));
        }
        if let Some(h) = self.bench.as_mut().and_then(|b| b.hold.as_mut()) {
            let now = Instant::now();
            if h.released.is_none() {
                if let Some(l) = h.last_present {
                    // Nothing is due during the key-repeat delay, so a gap counts from the
                    // first repeat at the earliest.
                    let gap = (now - l.max(h.pressed + h.repeat.0)).as_secs_f64() * 1000.0;
                    h.gaps.push(gap);
                    if gap > 50.0 {
                        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
                        h.stalls
                            .push([ms(now - h.pressed), gap, acquire_ms, ms(now - t_frame)]);
                    }
                }
                h.last_present = Some(now);
                let frame_ms = t_frame.elapsed().as_secs_f64() * 1000.0;
                h.frame_max_ms = h.frame_max_ms.max(frame_ms);
                h.frame_ms.push(frame_ms);
                h.work_ms.push(frame_ms - acquire_ms);
                h.frames_without_pixels += usize::from(!shown_pixels && !stand_in);
                h.vram_max = h.vram_max.max(self.hud.vram_bytes);
            }
            if shown_pixels
                && let Some((id, at)) = h.pending
                && cur == Some(id)
            {
                h.latencies.push((now - at).as_secs_f64() * 1000.0);
                h.pending = None;
            }
        }
        if self.bench.as_ref().is_some_and(|b| b.loupe.is_some()) {
            let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1000.0;
            let now = Instant::now();
            if let Some(l) = self.bench.as_mut().and_then(|b| b.loupe.as_mut()) {
                l.pending = Some(loupe_bench::FrameParts {
                    total: ms(t_frame, now),
                    scene: ms(t_scene, t_ui),
                    ui: ms(t_ui, t_submit),
                    present: ms(t_submit, presented),
                    acquire: acquire_ms,
                });
            }
        }
        if let Some(p) = self.bench.as_mut().and_then(|b| b.pan.as_mut()) {
            let now = Instant::now();
            let ms = |d: Duration| d.as_secs_f64() * 1000.0;
            p.frames.push([
                (now - p.started).as_secs_f64(),
                ms(now - t_frame),
                p.last_present.map_or(0.0, |l| ms(now - l)),
                acquire_ms,
                self.tiles_missing as f64,
                self.view.scale() as f64,
            ]);
            p.last_present = Some(now);
        }
        if let Some(b) = self.bench.as_mut() {
            let t = specular_core::ms_since_start();
            // The first frame's parts (recorded once, like every stage).
            let at = |i: Instant| (i - specular_core::process_start()).as_secs_f64() * 1000.0;
            b.stage("first_frame_acquire", at(t_acquire));
            b.stage("first_frame_scene", at(t_scene));
            b.stage("first_frame_ui", at(t_ui));
            b.stage("first_frame_submit", at(t_submit));
            b.stage("first_frame_present", at(t_present));
            b.stage("first_present", t);
            if shown_pixels {
                b.stage("first_present_with_pixels", t);
            }
        }
        gfx.text.end_frame();
        let text_redraw = gfx.text.needs_redraw;
        let fonts_pending = gfx.text.fonts_pending();
        self.hud.frame_ms =
            self.hud.frame_ms * 0.8 + t_frame.elapsed().as_secs_f64() * 1000.0 * 0.2;
        let caused = self.frame_presented(asked, cpu_to_present);
        self.startup_bench_frame(shown_pixels, fonts_pending);
        // Compare: flicker → present latency (and its benchmark).
        if self.gallery.cmp_open() {
            let ready = self.compare_channel_settled();
            if self.compare_presented(ready, presented) {
                self.redraw = true;
            }
        }

        if let Some((buf, row, fmt)) = readback.as_ref().filter(|_| self.bim_capturing()) {
            // A view saved as a PNG, or a BCF viewpoint's snapshot (`mesh/bim.rs`).
            let gpu = &self.gfx.as_ref().expect("gfx").gpu;
            match read_rgba(gpu, buf, *row, size, *fmt) {
                Ok(rgba) => self.bim_captured(&rgba, size),
                Err(e) => {
                    self.mesh.bim.capture = None;
                    specular_log::warn!(target: specular_log::RENDER, "capture: {e}");
                }
            }
            self.redraw = true;
        } else if let Some((buf, row, fmt)) = readback.as_ref().filter(|_| self.shot.is_none()) {
            // A script's screenshot (`script.rs`): the pixels go to its connection.
            if let Some(call) = self.remote.script.screenshot.take() {
                let gpu = &self.gfx.as_ref().expect("gfx").gpu;
                match read_rgba(gpu, buf, *row, size, *fmt) {
                    Ok(rgba) => {
                        call.answer_pixels(Ok(serde_json::json!({})), (size.0, size.1, rgba))
                    }
                    Err(e) => call.answer(Err(remote_fail(e))),
                }
            }
        } else if let Some((buf, row, fmt)) = readback {
            let path = self
                .shot
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_default();
            match save_png(
                &self.gfx.as_ref().expect("gfx").gpu,
                &buf,
                row,
                size,
                fmt,
                &path,
            ) {
                Ok(()) => eprintln!("specular: screenshot written to {}", path.display()),
                Err(e) => self.fatal = Some(format!("screenshot: {e}")),
            }
            el.exit();
            return;
        }

        // First pixels of the current image.
        if shown_pixels
            && let Some(id) = cur
            && let Some(i) = self.images.get_mut(&id)
            && i.open_ms.is_none()
        {
            i.open_ms = i.shown_at.map(|t| t.elapsed().as_secs_f64() * 1000.0);
        }
        // The whole image: the first frame with a complete level of it.
        if shown_pixels
            && let Some(id) = cur
            && let Some(i) = self.images.get_mut(&id)
            && i.whole_ms.is_none()
            && i.gpu_rank.is_some()
        {
            i.whole_ms = i.shown_at.map(|t| t.elapsed().as_secs_f64() * 1000.0);
        }
        let before = (
            self.view.revision,
            self.current,
            self.bench.as_ref().map(|b| b.region_i),
        );
        self.after_present(shown_pixels);
        self.release_prefetch();
        let kick = before
            != (
                self.view.revision,
                self.current,
                self.bench.as_ref().map(|b| b.region_i),
            );

        // When to draw next.
        // A held button alone draws nothing: each drag move asks for its frame
        // (`PointerMove`), so the GPU idles while the pointer rests.
        let again = self.redraw;
        let pan = self
            .bench
            .as_ref()
            .is_some_and(|b| b.phase == BenchPhase::Pan);
        self.redraw = false;
        for (on, c) in [
            (animating, Cause::Animation),
            (text_redraw, Cause::Text),
            (kick, Cause::ViewChanged),
            (gallery_anim, Cause::GalleryAnim),
            (pan, Cause::Bench),
            (again, Cause::Again),
        ] {
            if on {
                self.redraw = true;
                self.causes.add(c);
            }
        }
        self.frames += 1;
        debug!(
            target: RENDER,
            "frame {} presented in {:.1} ms (acquire {acquire_ms:.1} ms; caused by {}); next frame: {}",
            self.frames,
            t_frame.elapsed().as_secs_f64() * 1000.0,
            caused.describe(),
            redraw_reasons(animating, text_redraw, kick || again)
        );
        self.wake_at = None;
        self.causes.wake = None;
        let mut wake: Option<f64> = None;
        let mut wake_cause = Cause::Timer;
        let mut sooner = |d: f64, c: Cause| {
            if wake.is_none_or(|w| d < w) {
                wake = Some(d);
                wake_cause = c;
            }
        };
        if let Some(d) = wake_chrome {
            sooner(d, Cause::Chrome);
        }
        if let Some(d) = gallery_wake {
            sooner(d, Cause::Gallery);
        }
        // A stand-in ends after `STAND_IN` even when nothing else changes.
        if stand_in && !self.flipping && !page_hold {
            let left = cur
                .and_then(|id| self.images.get(&id))
                .and_then(|i| i.shown_at)
                .map_or(0.0, |t| {
                    (STAND_IN.saturating_sub(t.elapsed())).as_secs_f64()
                });
            sooner(left, Cause::StandIn);
        }
        // Fonts swap in when they are ready; benchmarks and screenshots check for timeouts.
        if fonts_pending {
            sooner(0.02, Cause::Fonts);
        } else if self.shot.is_some() {
            sooner(0.02, Cause::Bench);
        } else if self.bench.is_some() {
            sooner(0.5, Cause::Bench);
        }
        // The slideshow's timer, fade, Ken Burns and pointer.
        if let Some(d) = self.slideshow_wake(now).or(fading.then_some(0.0)) {
            sooner(d, Cause::Slideshow);
        }
        // The pointer hiding once it has rested.
        if let Some(d) = pointer_hide.wake() {
            sooner(d, Cause::PointerHide);
        }
        // Cull mode's confirmation, tooltip and benchmark.
        if let Some(d) = self.cull_wake(now) {
            sooner(d, Cause::Cull);
        }
        // Develop's speed-edit readout going.
        if let Some(d) = self.develop_wake(now) {
            sooner(d, Cause::Cull);
        }
        // A selection the keys made in a text field, offered to the primary selection once
        // they rest (the chrome's own fields look when they draw).
        let primary = [
            self.filter_primary_tick(now),
            self.convert_primary_tick(now),
        ];
        for d in primary.into_iter().flatten() {
            sooner(d, Cause::Selection);
        }
        // The 3D view refines once a wheel or key move has settled.
        if let Some(d) = self.mpr_wake() {
            sooner(d, Cause::Mpr);
        }
        // Two sheets compared, blinking.
        if let Some(d) = self.sheets_wake(now) {
            sooner(d, Cause::Timer);
        }
        // A key sequence's timeout.
        if let Some((_, due, _)) = self.keys.pending {
            let d = (due - now).max(0.0);
            sooner(d, Cause::KeySequence);
        }
        match wake {
            Some(d) if d <= 0.0 => {
                self.redraw = true;
                self.causes.add(wake_cause);
            }
            Some(d) => {
                self.wake_at = Some(Instant::now() + Duration::from_secs_f64(d));
                self.causes.wake = Some(wake_cause);
            }
            None => {}
        }
    }

    // ----- Benchmark ----------------------------------------------------------------------

    fn sharp_scale(&self, id: ImageId) -> Option<u32> {
        let img = self.images.get(&id)?;
        let info = img.info.as_ref()?;
        let (vw, vh) = self.viewport_for(id)?;
        let (dw, dh) = info.display_size();
        Some(needed_scale(dw, dh, vw, vh, loader::scale_cap(info)))
    }

    /// The current image is sharp on screen: why it took as long as it did (DESIGN.md §12.7).
    /// Logged, shown in the information panel, and printed with `SPECULAR_OPEN_REPORT=1`.
    fn finish_open_report(&mut self, id: ImageId) {
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        let (Some(info), Some(at)) = (&img.info, img.shown_at) else {
            return;
        };
        let t = img
            .track
            .timeline(info, at, img.open_ms, img.whole_ms, self.ready_at);
        let v = open_report::verdict(&t);
        img.track.done = Some((t, v));
        let line = open_report::log_line(&t, &v);
        debug!(target: DECODE, "img={id} {} open report: {line}", img.name());
        if std::env::var_os("SPECULAR_OPEN_REPORT").is_some_and(|v| !v.is_empty() && v != "0") {
            eprintln!("open report: {}: {line}", img.path.display());
        }
        open_report::remember(&t, &v);
        self.chrome
            .set_open_report(Some(open_report::panel(&t, &v)));
        self.refresh_insights();
        self.redraw = true;
    }

    /// Starts prefetch once the current image is sharp on the GPU, failed, or there is none,
    /// or a second after the window came up (a gallery start never shows one), so neighbour
    /// decodes and uploads do not compete with the current image's levels for cores and the
    /// queue during start-up.
    fn release_prefetch(&mut self) {
        if !self.prefetch_held {
            return;
        }
        let cur = self.current_id();
        let failed = cur.is_none_or(|id| self.images.get(&id).is_none_or(|i| i.error.is_some()));
        let sharp = cur.is_some_and(|id| {
            let rank = self.images.get(&id).and_then(|i| i.gpu_rank);
            matches!((self.sharp_scale(id), rank), (Some(s), Some(r)) if r <= s)
        });
        let late = self
            .ready_at
            .is_some_and(|t| t.elapsed() > Duration::from_secs(1));
        if sharp || failed || late {
            self.prefetch_held = false;
            performance::release_probe();
            self.schedule();
        }
    }

    fn after_present(&mut self, shown_pixels: bool) {
        if let Some(parts) = self
            .bench
            .as_mut()
            .and_then(|b| b.loupe.as_mut())
            .and_then(|l| l.pending.take())
        {
            let done = self.loupe_bench_presented(parts);
            if let Some(l) = self.bench.as_mut().and_then(|b| b.loupe.as_mut()) {
                l.done |= done;
            }
        }
        let Some(cur) = self.current_id() else { return };
        let sharp = self.sharp_scale(cur);
        let rank = self.images.get(&cur).and_then(|i| i.gpu_rank);
        if let (Some(s), Some(r)) = (sharp, rank)
            && r <= s
            && self.images.get(&cur).is_some_and(|i| i.state != "sharp")
        {
            self.set_state(cur, "sharp".to_owned());
            self.cull_sharp();
        }
        if let (Some(s), Some(r)) = (sharp, rank)
            && r <= s
            && self
                .images
                .get(&cur)
                .is_some_and(|i| i.track.done.is_none())
        {
            self.finish_open_report(cur);
        }
        let tiles_done = self.tiles_missing == 0 && self.tile_jobs.is_empty();
        let mut finish = false;
        let Some(b) = self.bench.as_mut() else { return };
        let t = specular_core::ms_since_start();
        match b.phase {
            BenchPhase::Sharp => {
                if shown_pixels && b.first_pixels_ms.is_none() {
                    b.first_pixels_ms = Some(t);
                }
                if let (Some(s), Some(r)) = (sharp, rank)
                    && r <= s
                {
                    b.sharp_ms = Some(t);
                    b.sharp_scale = s;
                    if let Some(n) = self.args.bench_hold {
                        // Press →: one step now, repeats follow from `about_to_wait`.
                        let now = Instant::now();
                        let repeat = self
                            .args
                            .bench_repeat
                            .map_or((REPEAT_DELAY, REPEAT_INTERVAL), |(d, i)| {
                                (Duration::from_millis(d), Duration::from_millis(i.max(1)))
                            });
                        b.phase = BenchPhase::Hold;
                        b.hold = Some(HoldBench {
                            target: n,
                            repeat,
                            pressed: now,
                            next_tick: now + repeat.0,
                            steps: 1,
                            pending: None,
                            latencies: Vec::new(),
                            skipped: 0,
                            last_present: None,
                            gaps: Vec::new(),
                            stalls: Vec::new(),
                            frame_max_ms: 0.0,
                            frame_ms: Vec::new(),
                            work_ms: Vec::new(),
                            frames_without_pixels: 0,
                            vram_max: 0,
                            step_max_ms: 0.0,
                            released: None,
                            settle_ms: None,
                        });
                        let t = self.now();
                        self.hold = Some((Action::NextImage, t));
                        self.navigate(1);
                        if let Some(id) = self.current_id()
                            && let Some(h) = self.bench.as_mut().and_then(|b| b.hold.as_mut())
                        {
                            h.pending = Some((id, now));
                        }
                        return;
                    }
                    if let Some(secs) = self.args.bench_loupe {
                        b.phase = BenchPhase::Loupe;
                        self.start_loupe_bench(secs);
                        self.redraw = true;
                        return;
                    }
                    if let Some(secs) = self.args.bench_pan {
                        b.phase = BenchPhase::Pan;
                        b.pan = Some(PanBench {
                            secs,
                            started: Instant::now(),
                            last_present: None,
                            frames: Vec::new(),
                        });
                        self.redraw = true;
                        return;
                    }
                    b.phase = BenchPhase::Region;
                    b.region_i = 0;
                    b.region_t = Instant::now();
                    b.region_tiles.clear();
                    let (dw, dh) = self.view.display_size();
                    let (fx, fy) = REGIONS[0];
                    self.view.zoom_to(1.0, None);
                    self.view.settle();
                    self.view.center_on((fx * dw, fy * dh), false);
                    self.view.settle();
                }
            }
            BenchPhase::Region => {
                // Done when the frame just presented had every tile.
                if tiles_done && b.region_t.elapsed() > Duration::from_millis(1) {
                    b.regions.push(b.region_t.elapsed().as_secs_f64() * 1000.0);
                    b.region_tiles
                        .push(self.gfx.as_ref().map_or(0, |g| g.renderer.pool.len()));
                    b.region_i += 1;
                    if b.region_i < REGIONS.len() {
                        let (dw, dh) = self.view.display_size();
                        let (fx, fy) = REGIONS[b.region_i];
                        self.view.center_on((fx * dw, fy * dh), false);
                        self.view.settle();
                        b.region_t = Instant::now();
                    } else {
                        b.phase = BenchPhase::Prefetch;
                        b.phase_t = Instant::now();
                        let idx = self.current.unwrap_or(0);
                        b.neighbour = if idx + 1 < self.entries.len() {
                            Some(idx + 1)
                        } else if idx > 0 {
                            Some(idx - 1)
                        } else {
                            None
                        };
                        if b.neighbour.is_none() {
                            finish = true;
                        }
                    }
                }
            }
            BenchPhase::Prefetch => {
                let n = b.neighbour.expect("neighbour");
                let ready = self
                    .entries
                    .get(n)
                    .and_then(|p| self.by_path.get(p))
                    .and_then(|id| {
                        let img = self.images.get(id)?;
                        let info = img.info.as_ref()?;
                        let (vw, vh) = (self.view.viewport.w as u32, self.view.viewport.h as u32);
                        let (dw, dh) = info.display_size();
                        Some(img.gpu_rank? <= needed_scale(dw, dh, vw, vh, loader::scale_cap(info)))
                    });
                if ready == Some(true) || b.phase_t.elapsed() > Duration::from_secs(10) {
                    b.prefetch_wait_ms = b.phase_t.elapsed().as_secs_f64() * 1000.0;
                    b.phase = BenchPhase::Next;
                    b.next_file = self.entries[n]
                        .file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    b.phase_t = Instant::now();
                    let delta = n as i64 - self.current.unwrap_or(0) as i64;
                    self.nav_dir = delta.signum() as i32;
                    self.go_to(n);
                }
            }
            BenchPhase::Hold => {
                let h = b.hold.as_mut().expect("hold bench");
                if let Some(r) = h.released
                    && let (Some(s), Some(rk)) = (sharp, rank)
                    && rk <= s
                {
                    h.settle_ms = Some(r.elapsed().as_secs_f64() * 1000.0);
                    finish = true;
                }
            }
            BenchPhase::Loupe => {
                finish = b.loupe.as_ref().is_some_and(|l| l.done);
            }
            BenchPhase::Pan => {
                let p = b.pan.as_ref().expect("pan bench");
                if p.started.elapsed().as_secs_f64() >= p.secs {
                    finish = true;
                }
            }
            BenchPhase::Next => {
                if shown_pixels && b.next_first_ms.is_none() {
                    b.next_first_ms = Some(b.phase_t.elapsed().as_secs_f64() * 1000.0);
                }
                if let (Some(s), Some(r)) = (sharp, rank)
                    && r <= s
                {
                    b.next_ms = Some(b.phase_t.elapsed().as_secs_f64() * 1000.0);
                    finish = true;
                }
            }
        }
        if finish {
            self.print_bench();
            self.exit_requested = true;
        } else {
            self.bench_timeout();
        }
    }

    /// Ends a benchmark that has run too long (checked after each frame and in `about_to_wait`,
    /// so it fires even when no frame is drawn).
    fn bench_timeout(&mut self) {
        let limit = Duration::from_secs(if self.args.bench_hold.is_some() {
            300
        } else {
            60 + self
                .args
                .bench_pan
                .or(self.args.bench_loupe)
                .map_or(0, |s| s.max(0.0) as u64)
        });
        if !self.exit_requested
            && self
                .bench
                .as_ref()
                .is_some_and(|b| b.started.elapsed() > limit)
        {
            eprintln!("specular: benchmark timed out");
            self.print_bench();
            self.exit_requested = true;
        }
    }

    /// A benchmark or screenshot run (no settings file, timings or a capture wanted).
    fn headless(&self) -> bool {
        self.args.bench_open.is_some() || self.args.screenshot.is_some()
    }

    /// `--bench-hold`: key repeats of the held →, at the platform repeat rate.
    fn drive_hold_bench(&mut self) {
        let Some(h) = self.bench.as_ref().and_then(|b| b.hold.as_ref()) else {
            return;
        };
        if h.released.is_some() {
            return;
        }
        let now = Instant::now();
        if now < h.next_tick {
            let t = h.next_tick;
            self.wake_by(t, Cause::Bench);
            return;
        }
        let held = (now - h.pressed).as_secs_f64();
        let step = self.hold_step(held);
        let before = self.current;
        self.flipping = true;
        self.navigate(step);
        let moved = self.current != before;
        let id = self.current_id();
        let last = self.current.is_some_and(|c| c + 1 >= self.entries.len());
        let h = self
            .bench
            .as_mut()
            .and_then(|b| b.hold.as_mut())
            .expect("hold bench");
        h.step_max_ms = h.step_max_ms.max(now.elapsed().as_secs_f64() * 1000.0);
        if moved {
            h.steps += step as usize;
            if h.pending.is_some() {
                h.skipped += 1;
            }
            h.pending = id.map(|id| (id, now));
        }
        h.next_tick += h.repeat.1;
        if h.next_tick < now {
            h.next_tick = now + h.repeat.1;
        }
        let t = h.next_tick;
        if h.steps >= h.target || last || !moved {
            h.released = Some(Instant::now());
            self.hold = None;
            self.end_flipping();
        } else {
            self.wake_by(t, Cause::Bench);
        }
        self.redraw = true;
    }

    /// `SPECULAR_BENCH_STARTUP=1`: after a frame was presented. Prints one JSON line and exits
    /// once the window is complete: the image's first pixels (when one was opened) and text in
    /// the real fonts, or after 10 s. Times are ms since `main`; the caller adds `main`'s start
    /// (`main_start_unix_ms`) to compare them with its own clock at exec.
    fn startup_bench_frame(&mut self, shown_pixels: bool, fonts_pending: bool) {
        let wants_pixels = self.current.is_some() && !self.gallery.active;
        let Some(sb) = self.startup_bench.as_mut() else {
            return;
        };
        let t = specular_core::ms_since_start();
        sb.first.get_or_insert(t);
        if shown_pixels && wants_pixels {
            sb.pixels.get_or_insert(t);
        }
        if !fonts_pending {
            sb.text.get_or_insert(t);
        }
        let done = (!wants_pixels || sb.pixels.is_some()) && sb.text.is_some();
        if self.exit_requested || !done && t < 10_000.0 {
            return;
        }
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0);
        let opt = |v: Option<f64>| v.map_or("null".to_owned(), |v| format!("{v:.1}"));
        println!(
            "{{\"startup\": {{\"first_present_ms\": {}, \"pixels_ms\": {}, \"text_ms\": {}, \
             \"frames\": {}, \"gallery\": {}, \"main_start_unix_ms\": {:.1}, \"peak_rss_mb\": {:.1}}}}}",
            opt(sb.first),
            opt(sb.pixels),
            opt(sb.text),
            self.frames + 1,
            self.gallery.active,
            unix - t,
            peak_rss_bytes() as f64 / 1048576.0
        );
        self.exit_requested = true;
    }

    /// `--bench-pan`: puts the view where the fixed path (`pan_view_at`) has it now.
    fn drive_pan_bench(&mut self) {
        let Some(p) = self.bench.as_ref().and_then(|b| b.pan.as_ref()) else {
            return;
        };
        let t = p.started.elapsed().as_secs_f64();
        let (scale, (fx, fy)) = pan_view_at(t, p.secs, self.view.fit_scale());
        let (dw, dh) = self.view.display_size();
        self.view.zoom_to(scale, None);
        self.view.settle();
        self.view.center_on((fx * dw, fy * dh), false);
    }

    fn drive_bench(&mut self, el: &ActiveEventLoop) {
        if self.exit_requested {
            el.exit();
        }
    }

    fn print_bench(&self) {
        let Some(b) = &self.bench else { return };
        let path = self.args.bench_open.clone().unwrap_or_default();
        let first = self
            .by_path
            .get(&std::path::absolute(&path).unwrap_or(path.clone()))
            .and_then(|id| self.images.get(id));
        let (mp, prog, dims) = first
            .and_then(|i| i.info.as_ref())
            .map(|i| (i.megapixels(), i.progressive, (i.width, i.height)))
            .unwrap_or((0.0, false, (0, 0)));
        let opt = |v: Option<f64>| v.map_or("null".to_owned(), |v| format!("{v:.1}"));
        let later: Vec<f64> = b.regions.iter().skip(1).copied().collect();
        let later_max = later
            .iter()
            .copied()
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
        let later_med = {
            let mut v = later.clone();
            v.sort_by(f64::total_cmp);
            v.get(v.len() / 2).copied()
        };
        let vp = self.view.viewport;
        let mut s = String::from("{\n");
        s += &format!("  \"file\": {},\n", json_str(&path.display().to_string()));
        s += &format!(
            "  \"width\": {}, \"height\": {}, \"megapixels\": {:.1}, \"progressive\": {},\n",
            dims.0, dims.1, mp, prog
        );
        s += &format!("  \"viewport\": [{}, {}],\n", vp.w as u32, vp.h as u32);
        s += &format!("  \"first_pixels_ms\": {},\n", opt(b.first_pixels_ms));
        s += &format!("  \"sharp_ms\": {},\n", opt(b.sharp_ms));
        s += &format!("  \"sharp_scale\": \"1/{}\",\n", b.sharp_scale);
        s += &format!(
            "  \"decode_events_ms\": {{{}}},\n",
            b.overview_ms
                .iter()
                .take(16)
                .map(|(k, v)| format!("{}: {v:.1}", json_str(k)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut stages = b.stages.clone();
        stages.sort_by(|a, b| a.1.total_cmp(&b.1));
        s += "  \"stages_ms\": {\n";
        s += &stages
            .iter()
            .map(|(k, v)| format!("    {}: {v:.1}", json_str(k)))
            .collect::<Vec<_>>()
            .join(",\n");
        s += "\n  },\n";
        s += &format!("  \"gpu_device_init_ms\": {:.1},\n", b.gpu_init_ms);
        s += &format!(
            "  \"upload_ms\": {{{}}},\n",
            b.upload_ms
                .iter()
                .take(16)
                .map(|(k, v)| format!("{}: {v:.2}", json_str(k)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        s += &format!(
            "  \"first_region_ms\": {},\n",
            opt(b.regions.first().copied())
        );
        s += &format!(
            "  \"later_regions_ms\": [{}],\n",
            later
                .iter()
                .map(|v| format!("{v:.1}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        s += &format!("  \"later_region_median_ms\": {},\n", opt(later_med));
        s += &format!("  \"later_region_max_ms\": {},\n", opt(later_max));
        s += &format!("  \"next_file\": {},\n", json_str(&b.next_file));
        s += &format!("  \"prefetch_wait_ms\": {:.1},\n", b.prefetch_wait_ms);
        s += &format!(
            "  \"next_image_first_pixels_ms\": {},\n",
            opt(b.next_first_ms)
        );
        s += &format!("  \"next_image_ms\": {},\n", opt(b.next_ms));
        if let Some((t, v)) = first.and_then(|i| i.track.done) {
            let key = |k: Option<&str>| k.map_or("null".to_owned(), json_str);
            s += "  \"open_report\": {\n";
            s += &format!(
                "    \"stages_ms\": {{{}}},\n",
                t.stages()
                    .iter()
                    .map(|(k, v)| format!("{}: {v:.1}", json_str(k.key())))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            s += &format!(
                "    \"finding\": {}, \"bottleneck\": {}, \"percent\": {}, \"software\": {}, \"hint\": {},\n",
                json_str(v.finding.key()),
                key(v.bottleneck.map(|b| b.0.key())),
                v.bottleneck.map_or(0, |b| b.1),
                key(v.software.map(|s| s.key())),
                key(v.hint.map(|h| h.key()))
            );
            s += &format!(
                "    \"headline\": {},\n",
                json_str(&open_report::headline(&v).replace(['\u{2068}', '\u{2069}'], ""))
            );
            s += &format!(
                "    \"summary\": {}\n  }},\n",
                json_str(&open_report::log_line(&t, &v))
            );
        }
        s += &format!("  \"decode_threads\": {},\n", self.loader.threads());
        s += &format!("  \"gpu\": {},\n", json_str(&self.hud.backend));
        s += &format!("  \"cpu\": {},\n", json_str(&self.hw.cpu_model));
        if let Some(h) = &b.hold {
            let pct = |v: &[f64], q: f64| {
                let mut v = v.to_vec();
                v.sort_by(f64::total_cmp);
                v.get(((v.len() as f64 - 1.0) * q).round() as usize)
                    .copied()
            };
            let held_s = h.released.map_or(0.0, |r| (r - h.pressed).as_secs_f64());
            s += "  \"hold\": {\n";
            s += &format!(
                "    \"images_passed\": {}, \"held_s\": {held_s:.2}, \"images_shown\": {}, \"skipped\": {},\n",
                h.steps,
                h.latencies.len(),
                h.skipped
            );
            s += &format!(
                "    \"pixels_latency_ms\": {{\"p50\": {}, \"p95\": {}, \"p99\": {}, \"max\": {}}},\n",
                opt(pct(&h.latencies, 0.5)),
                opt(pct(&h.latencies, 0.95)),
                opt(pct(&h.latencies, 0.99)),
                opt(pct(&h.latencies, 1.0))
            );
            s += &format!(
                "    \"present_gap_ms\": {{\"p50\": {}, \"p99\": {}, \"max\": {}, \"over_50\": {}}},\n",
                opt(pct(&h.gaps, 0.5)),
                opt(pct(&h.gaps, 0.99)),
                opt(pct(&h.gaps, 1.0)),
                h.gaps.iter().filter(|&&g| g > 50.0).count()
            );
            // A gap with a quick frame at its end is the app idling (nothing changed, e.g. the
            // key-repeat delay); a stall is a frame that itself took over 50 ms to build and present.
            s += &format!(
                "    \"frames_over_50\": {},\n",
                h.stalls.iter().filter(|v| v[3] > 50.0).count()
            );
            s += &format!(
                "    \"stalls_at_gap_acquire_frame_ms\": [{}],\n",
                h.stalls
                    .iter()
                    .map(|v| format!("[{:.0}, {:.1}, {:.1}, {:.1}]", v[0], v[1], v[2], v[3]))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            s += &format!(
                "    \"frame_cpu_max_ms\": {:.1}, \"repeat_cpu_max_ms\": {:.1}, \"settle_sharp_ms\": {},\n",
                h.frame_max_ms,
                h.step_max_ms,
                opt(h.settle_ms)
            );
            s += &format!(
                "    \"repeat_ms\": [{}, {}], \"frames\": {}, \"frames_without_pixels\": {},\n",
                h.repeat.0.as_millis(),
                h.repeat.1.as_millis(),
                h.frame_ms.len(),
                h.frames_without_pixels
            );
            s += &format!(
                "    \"frame_cpu_ms\": {{\"p50\": {}, \"p95\": {}, \"p99\": {}, \"max\": {}, \"over_16_7\": {}}},\n",
                opt(pct(&h.frame_ms, 0.5)),
                opt(pct(&h.frame_ms, 0.95)),
                opt(pct(&h.frame_ms, 0.99)),
                opt(pct(&h.frame_ms, 1.0)),
                h.frame_ms.iter().filter(|&&v| v > 1000.0 / 60.0).count()
            );
            s += &format!(
                "    \"frame_work_ms\": {{\"p50\": {}, \"p95\": {}, \"p99\": {}, \"max\": {}, \"over_16_7\": {}}},\n",
                opt(pct(&h.work_ms, 0.5)),
                opt(pct(&h.work_ms, 0.95)),
                opt(pct(&h.work_ms, 0.99)),
                opt(pct(&h.work_ms, 1.0)),
                h.work_ms.iter().filter(|&&v| v > 1000.0 / 60.0).count()
            );
            s += &format!(
                "    \"present_gaps_over_25\": {}, \"present_gaps_over_33_4\": {},\n",
                h.gaps.iter().filter(|&&g| g > 25.0).count(),
                h.gaps.iter().filter(|&&g| g > 1000.0 / 30.0).count()
            );
            let per_s = |n: usize| if held_s > 0.0 { n as f64 / held_s } else { 0.0 };
            s += &format!(
                "    \"images_shown_per_s\": {:.1}, \"images_passed_per_s\": {:.1}, \"vram_max_mb\": {:.1}\n",
                per_s(h.latencies.len()),
                per_s(h.steps),
                h.vram_max as f64 / 1048576.0
            );
            s += "  },\n";
        }
        if let Some(p) = &b.pan {
            s += &pan_report(p);
        }
        if let Some(l) = &b.loupe {
            s += &loupe_bench::report(l);
        }
        s += &format!(
            "  \"peak_rss_mb\": {:.1}\n",
            peak_rss_bytes() as f64 / 1048576.0
        );
        s += "}";
        println!("{s}");
    }

    // ----- Screenshot ---------------------------------------------------------------------

    fn drive_shot_before(&mut self) {
        let Some(shot) = &self.shot else { return };
        if shot.applied {
            return;
        }
        let started = shot.started;
        // Wait for the image (sharp) before opening overlays.
        let ready = match self.current_id() {
            // A folder opened in the gallery has no current image to wait for.
            None => {
                self.gallery.active
                    || self.entries.is_empty() && started.elapsed() > Duration::from_millis(300)
            }
            Some(id) => {
                let img = self.images.get(&id);
                let err = img.is_some_and(|i| i.error.is_some());
                let sharp = self.sharp_scale(id);
                let rank = img.and_then(|i| i.gpu_rank);
                err || matches!((sharp, rank), (Some(s), Some(r)) if r <= s)
            }
        };
        // `--show gallery…` picks tiles (compare, select, quick look…) from the listing: enter
        // the gallery first and wait until the listing is in and laid out.
        let wants_gallery = self.args.show.iter().any(|s| s.starts_with("gallery"));
        let ready = ready && (!wants_gallery || self.gallery_shot_listed());
        // `--show map-…`: the map view is on once the map has been read (a DEM's values).
        let wants_map = self.args.show.iter().any(|s| s.starts_with("map-"));
        let ready = ready && (!wants_map || self.gis_shot_ready());
        if !ready && started.elapsed() < Duration::from_secs(20) {
            return;
        }
        let show = self.args.show.clone();
        let now = self.now();
        for s in &show {
            match s.as_str() {
                "info" => self.chrome.set_info(true, false),
                "info-all" => self.chrome.set_info(true, true),
                "info-performance" => {
                    self.chrome.set_info(true, false);
                    self.chrome.set_info_performance(true);
                }
                "palette" => self.chrome.open_modal(Modal::Palette),
                "find-text" => self.find_text(),
                "develop" => {
                    self.develop_action(Action::ToggleDevelop);
                }
                "cull" => self.cull_enter(),
                "cull-pick" => {
                    self.cull_action(Action::CullPick);
                }
                s if s.starts_with("cull-rate-") => {
                    let stars = s["cull-rate-".len()..].parse().unwrap_or(0);
                    self.cull_action(Action::CullRate(stars));
                }
                "cheatsheet" => self.chrome.open_modal(Modal::CheatSheet),
                "about" => self.chrome.open_modal(Modal::About),
                "insights" => self.open_insights(),
                "settings" => self.chrome.open_modal(Modal::Settings),
                "language" => self.chrome.open_modal(Modal::Language),
                "hud" => self.chrome.set_hud(true),
                "hw-report" => self.setting_action("advanced.hw_report"),
                "scrollbars" => self.chrome.pin_scrollbars(),
                "navigator" => self.navigator = true,
                "low-memory" => {
                    if let Some(id) = self.current_id() {
                        self.loader.post(Event::MemoryShort {
                            id,
                            need: 3 << 30,
                            available: 1 << 30,
                        });
                    }
                }
                "loupe" => self.shot_loupe = true,
                "goto" => {
                    self.chrome.handle_action(Action::GoToImage);
                }
                "toolbar-menu" => self.chrome.show_toolbar_menu(),
                // The medium of the current file's folder read (its progress and Cancel), or
                // stopped at once.
                "studies" => self.studies_show(),
                "studies-stopped" => {
                    self.studies_show();
                    self.studies_cancel();
                }
                "toolbar-more" => self.chrome.show_overflow_menu(),
                // After `settings-viewing`: the third row's menu in the toolbar list.
                "toolbar-row-menu" => self.chrome.show_toolbar_row_menu(2),
                // The second item of the bar dragged over the fifth.
                "toolbar-drag" => self.chrome.show_bar_drag(1, 4),
                s if s.starts_with("settings-") => {
                    self.chrome.open_settings_at(&s["settings-".len()..])
                }
                // After `settings-<section>`: that page scrolled down by N px.
                s if s.starts_with("scroll-") => {
                    let dy = s["scroll-".len()..].parse().unwrap_or(0.0);
                    self.chrome.scroll_settings(dy);
                }
                "confirm-trash" => self.chrome.confirm_trash("IMG_2041.jpg".to_owned()),
                "fullscreen-hint" => self.chrome.entered_fullscreen(now),
                "toast" => {
                    self.chrome.toast(
                        t!("toast-moved-to-trash-named", file = "IMG_2041.jpg"),
                        Some(u64::MAX),
                        now,
                    );
                }
                other => {
                    if !self.welcome_show_scene(other)
                        && !self.anon_show(other)
                        && !self.figure_show(other)
                        && !self.measure_show(other)
                        && !self.markup_show(other)
                        && !self.sheets_show(other)
                        && !self.gallery_show(other)
                        && !self.mpr_show(other)
                        && !self.dicomweb_show(other)
                        && !self.omero_show(other)
                        && !self.password_show(other)
                        && !self.thermal_report_show(other)
                        && !self.web_gallery_show(other)
                        && !self.raw_volume_show(other)
                        && !self.file_kind_show(other)
                        && !self.report_show(other)
                        && !self.gis_show(other)
                        && !self.develop_pano_show(other)
                    {
                        self.inspect_show(other);
                    }
                }
            }
        }
        if let Some(q) = self.args.query.clone() {
            self.chrome_input(Input::Text(q));
        }
        if let Some(z) = self.args.zoom {
            self.view.zoom_to(z / 100.0, None);
            self.view.settle();
        }
        if let Some(s) = self.shot.as_mut() {
            s.applied = true;
        }
    }

    fn shot_ready(&mut self, now: f64) -> bool {
        let _ = now;
        let tiles_done = self.tiles_missing == 0 && self.tile_jobs.is_empty();
        let meta_needed = self.chrome.info_open()
            && self
                .current_id()
                .and_then(|id| self.images.get(&id))
                .is_some_and(|i| i.metadata.is_none() && i.error.is_none());
        let gallery_done = self.gallery_settled()
            && self.mpr_settled()
            && self.oct_settled()
            && self.welcome.settled()
            && self.sheets_settled()
            && self.develop_settled()
            && !self.find_text.pending;
        let bench_busy = self.gallery.bench_busy();
        let Some(shot) = self.shot.as_mut() else {
            return false;
        };
        if !shot.applied {
            return false;
        }
        let settled = (tiles_done && !meta_needed && gallery_done)
            || shot.started.elapsed() > Duration::from_secs(25) && !bench_busy;
        if !settled {
            shot.ready_since = None;
            return false;
        }
        // Let fades and the spinner finish.
        let since = *shot.ready_since.get_or_insert_with(Instant::now);
        since.elapsed() > Duration::from_millis(700)
    }
}

/// What one press of Back (Esc) does, in order: close the top layer of the chrome (or the
/// loupe), else go back to fit, else leave fullscreen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackStep {
    Closed,
    Fit,
    ExitFullscreen,
    Nothing,
}

fn back_step(closed_layer: bool, at_fit: bool, fullscreen: bool) -> BackStep {
    if closed_layer {
        BackStep::Closed
    } else if !at_fit {
        BackStep::Fit
    } else if fullscreen {
        BackStep::ExitFullscreen
    } else {
        BackStep::Nothing
    }
}

fn stall_threshold(secs: f64) -> Duration {
    Duration::from_secs_f64(secs.max(0.1))
}

fn kind_name(kind: OverviewKind) -> String {
    match kind {
        OverviewKind::Thumbnail => "thumbnail".to_owned(),
        OverviewKind::Level(s) => format!("level 1/{s}"),
    }
}

/// Why another frame follows ("idle" when none does).
fn redraw_reasons(animating: bool, text: bool, kick: bool) -> String {
    let r: Vec<&str> = [
        (animating, "animating"),
        (text, "text"),
        (kick, "view changed"),
    ]
    .into_iter()
    .filter_map(|(on, n)| on.then_some(n))
    .collect();
    if r.is_empty() {
        "idle".to_owned()
    } else {
        r.join(", ")
    }
}

fn rect_contains(r: PxRect, p: (f32, f32)) -> bool {
    p.0 >= r.x && p.1 >= r.y && p.0 < r.x + r.w && p.1 < r.y + r.h
}

fn remote_fail(e: String) -> crate::remote::protocol::Fail {
    crate::remote::protocol::Fail::new(crate::remote::protocol::Code::Internal, e)
}

fn save_png(
    gpu: &Gpu,
    buf: &wgpu::Buffer,
    row: u32,
    size: (u32, u32),
    fmt: wgpu::TextureFormat,
    path: &Path,
) -> Result<(), String> {
    let rgba = read_rgba(gpu, buf, row, size, fmt)?;
    let png = specular_png::encode(size.0, size.1, specular_png::ColorType::Rgba, &rgba)
        .map_err(|e| e.to_string())?;
    std::fs::write(path, png).map_err(|e| e.to_string())
}

/// The RGBA8 pixels of a frame read back into `buf`.
fn read_rgba(
    gpu: &Gpu,
    buf: &wgpu::Buffer,
    row: u32,
    size: (u32, u32),
    fmt: wgpu::TextureFormat,
) -> Result<Vec<u8>, String> {
    let slice = buf.slice(..);
    let (tx, rx) = crossbeam_channel::bounded(1);
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = gpu.wait();
    rx.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
    let mut rgba = Vec::with_capacity(size.0 as usize * size.1 as usize * 4);
    for y in 0..size.1 as usize {
        let line = &data[y * row as usize..][..size.0 as usize * 4];
        for px in line.chunks_exact(4) {
            match fmt {
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
                    rgba.extend_from_slice(&[px[2], px[1], px[0], 255])
                }
                wgpu::TextureFormat::Rgb10a2Unorm => {
                    let v = u32::from_le_bytes([px[0], px[1], px[2], px[3]]);
                    let c = |s: u32| (((v >> s) & 0x3ff) as f32 / 1023.0 * 255.0).round() as u8;
                    rgba.extend_from_slice(&[c(0), c(10), c(20), 255]);
                }
                _ => rgba.extend_from_slice(&[px[0], px[1], px[2], 255]),
            }
        }
    }
    drop(data);
    buf.unmap();
    Ok(rgba)
}

/// Whether the app asked for the frame being drawn: a redraw request outstanding or a frame
/// drawn at once from `about_to_wait`. Both are cleared: a flag left set would make the next
/// frame the compositor asks for (the outstanding request's) look asked for, and uncaused.
fn frame_asked(redraw_asked: &mut Option<Instant>, at_once: &mut bool) -> bool {
    let requested = redraw_asked.take().is_some();
    let now = std::mem::take(at_once);
    requested || now
}

/// The event loop callbacks, which [`ApplicationHandler`] runs inside the UI's panic
/// boundary ([`App::ui_guard`]).
impl App {
    fn handle_resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        if let Err(e) = self.init_gfx(el) {
            self.fatal = Some(e);
            el.exit();
            return;
        }
        self.drain_events();
        self.causes.add(Cause::Start);
        if let Some(g) = &self.gfx {
            g.window.request_redraw();
        }
    }

    fn handle_user_event(&mut self, el: &ActiveEventLoop) {
        if crate::quit_signal::requested() {
            self.exit_requested = true;
            el.exit();
            return;
        }
        self.because(Cause::Loader, |a| a.drain_events());
        self.because(Cause::Accessibility, |a| a.accessibility_requests(el));
    }

    fn handle_window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.presenter_window() == Some(id) {
            self.present_event(el, event);
            return;
        }
        if let (Some(a), Some(g)) = (self.a11y.as_mut(), self.gfx.as_ref()) {
            a.process_event(&g.window, &event);
        }
        if let WindowEvent::RedrawRequested = event {
            self.because(Cause::Loader, |a| a.drain_events());
            self.because(Cause::Accessibility, |a| a.accessibility_requests(el));
            self.frame(el);
            return;
        }
        let cause = match &event {
            WindowEvent::KeyboardInput { .. }
            | WindowEvent::ModifiersChanged(_)
            | WindowEvent::Ime(_) => Cause::Key,
            WindowEvent::CursorMoved { .. }
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::CursorEntered { .. } => Cause::Pointer,
            WindowEvent::MouseInput { .. } => Cause::Button,
            WindowEvent::MouseWheel { .. } => Cause::Wheel,
            WindowEvent::PinchGesture { .. }
            | WindowEvent::DoubleTapGesture { .. }
            | WindowEvent::RotationGesture { .. } => Cause::Gesture,
            _ => Cause::Window,
        };
        self.because(cause, |a| a.on_window_event(el, event));
    }

    fn handle_about_to_wait(&mut self, el: &ActiveEventLoop) {
        use Cause as C;
        self.because(C::Loader, |a| a.drain_events());
        self.because(C::Decoders, |a| a.decoders_tick());
        self.because(C::Decoders, |a| a.file_kind_tick());
        self.because(C::Panels, |a| a.local_panel_tick());
        self.because(C::Panels, |a| a.setting_path_tick());
        self.because(C::Panels, |a| a.stream_tick());
        self.because(C::Remote, |a| a.drain_remote(el));
        self.because(C::Medical, |a| a.medical_sync());
        self.because(C::Mpr, |a| a.mpr_tick());
        self.because(C::Medical, |a| a.linked_tick());
        self.because(C::Science, |a| {
            a.science_sync();
            a.astro_tick();
            a.thermal_seq_sync();
            a.geo_sync();
            a.cad_sync();
            a.font_sync();
            a.text_sync();
            a.gis_sync();
            a.mesh_sync();
            a.relight_sync();
        });
        self.because(C::Commands, |a| {
            for c in std::mem::take(&mut a.chrome_commands) {
                a.command(c, el);
            }
        });
        // Settings → Performance → "How Specular runs on this computer", and Settings →
        // Local AI, while open.
        let (paths_panel_wake, ai_page_wake) = self.because(C::Panels, |a| {
            let w = (a.drain_paths_panel(), a.drain_ai_page());
            if a.chrome.modal() == Some(Modal::Settings) {
                a.sync_restore_setting();
            }
            w
        });
        self.because(C::Gesture, |a| a.drain_gestures(el));
        self.because(C::Gesture, |a| a.devices_tick(el));
        self.because(C::Watch, |a| a.tether_tick());
        self.because(C::Window, |a| a.poll_display_profile());
        self.because(C::Window, |a| a.ocio_tick());
        self.because(C::Inspect, |a| a.tick_inspect());
        let compare_wake = self.because(C::Compare, |a| a.tick_compare());
        // The background work indicator and panel, while something runs.
        let work_wake = self.because(C::Work, |a| a.work_tick());
        // What is downloading, while something is.
        let net_wake = self.because(C::Work, |a| a.net_tick());
        // The keyboard crosshair follows the view; the readout is read out once it rests.
        let cursor_wake = self.because(C::Dialogs, |a| a.cursor_tick());
        // Slices copied into the series folder (series mode).
        let series_wake = self.because(C::Watch, |a| a.series_watch_tick());
        self.because(C::Gallery, |a| a.gallery_tick());
        self.because(C::Gallery, |a| a.keywords_tick());
        self.because(C::Gallery, |a| a.meta_tick());
        // The OS memory-pressure level changed: re-plan prefetch, which drops far images and
        // trims video memory to the scaled budget.
        self.because(C::Memory, |a| {
            if on_pressure_change(|all| {
                a.trim_sessions(all, None);
                a.trim_gpu_caches();
            }) {
                a.schedule();
            }
            a.release_prefetch();
        });
        self.because(C::Watch, |a| a.watch_current());
        self.because(C::Slideshow, |a| a.slideshow_tick());
        self.because(C::Present, |a| a.present_tick());
        self.because(C::Cull, |a| a.cull_tick(el));
        self.because(C::Cull, |a| a.develop_tick());
        self.because(C::Keys, |a| a.keys_poll(el));
        self.because(C::Dialogs, |a| {
            a.print_tick();
            a.anon_tick();
            a.annot_export_tick();
            a.web_gallery_tick();
            a.pathology_tick();
            a.label_volume_tick();
            a.figure_tick();
            a.quiz_tick();
            a.measure_tick();
            a.markup_tick();
            a.sheets_tick();
            a.convert_tick();
            a.recipes_tick();
            a.import_tick();
            a.fileops_tick();
            a.video_export_tick();
            a.dicomweb_tick();
            a.omero_tick();
        });
        self.because(C::Bench, |a| {
            a.drive_hold_bench();
            a.drive_loupe_bench();
            a.bench_timeout();
        });
        self.because(C::Dialogs, |a| a.diag_tick());
        if self.store.generation() != self.settings_gen
            || I18n::global().generation() != self.i18n_gen
        {
            self.because(C::Settings, |a| a.redraw = true);
        }
        let gate_wake = self.gate_tick(el);
        if self.exit_requested {
            el.exit();
            return;
        }
        if let Some(t) = self.wake_at
            && Instant::now() >= t
        {
            self.wake_at = None;
            if !self.redraw {
                self.wake_due();
            }
            self.redraw = true;
        }
        self.because(C::Animation, |a| a.tick_animation());
        // Headless runs: a redraw asked for long ago means no frame callbacks are coming (hidden
        // window, output off); draw it from here like an event-driven frame.
        if self.headless()
            && self
                .redraw_asked
                .is_some_and(|t| t.elapsed() >= HEADLESS_FRAME_STALL)
        {
            self.redraw_asked = None;
            self.present_now = true;
            self.redraw = true;
        }
        // Headless gallery runs (screenshots, the gallery bench) draw without waiting for
        // frame callbacks, so their timings measure the gallery, not the fallback above.
        let headless_gallery = self.headless() && self.shot.is_some() && self.gallery.active;
        if headless_gallery && (self.redraw || self.redraw_asked.take().is_some()) {
            self.redraw = true;
            self.present_now = true;
        }
        // Event-driven frames (not animation) go out at once; Mailbox presentation means the
        // compositor simply takes the newest buffer at its next refresh.
        if self.present_now && self.redraw && self.gfx.is_some() {
            // At most one such frame per 4 ms (tile results arrive in bursts); the gallery's
            // scroll bench paces frames at its display refresh.
            let due = self.last_frame
                + self
                    .gallery
                    .bench_period()
                    .unwrap_or(Duration::from_millis(4));
            if Instant::now() < due {
                el.set_control_flow(ControlFlow::WaitUntil(
                    self.wake_at.map_or(due, |w| w.min(due)),
                ));
                return;
            }
            self.present_now = false;
            self.redraw = false;
            self.because(C::Loader, |a| a.drain_events());
            self.frame_asked = true;
            self.frame(el);
            if self.exit_requested {
                el.exit();
                return;
            }
        }
        self.present_now = false;
        // A frame just presented forgets `wake_at`: ask for the next animation frame again.
        self.because(C::Animation, |a| a.tick_animation());
        let more_uploads = self.gfx.is_some() && self.upload_one_deferred();
        if self.redraw {
            self.redraw = false;
            // Start region decodes for a settled view now rather than in the next frame,
            // which can be a whole frame-callback interval (up to one refresh) away.
            if !self.view.is_animating() && self.gfx.is_some() {
                self.request_tiles();
            }
            if let Some(g) = &self.gfx {
                g.window.request_redraw();
                self.redraw_asked.get_or_insert_with(Instant::now);
            }
            el.set_control_flow(ControlFlow::Wait);
        } else if let Some(t) = self.wake_at {
            el.set_control_flow(ControlFlow::WaitUntil(t));
        } else {
            el.set_control_flow(ControlFlow::Wait);
        }
        if more_uploads {
            el.set_control_flow(ControlFlow::Poll);
        }
        self.inspect_control_flow(el);
        self.present_control_flow(el);
        self.loupe_bench_control_flow(el);
        // Path selector upkeep and idle-time calibration (not in benchmarks and screenshots).
        if !self.headless()
            && let Some(g) = self.gfx.as_mut()
            && let Some(t) =
                performance::tick(&g.gpu, &mut g.renderer, self.last_frame, self.redraw)
        {
            match el.control_flow() {
                ControlFlow::Wait => el.set_control_flow(ControlFlow::WaitUntil(t)),
                ControlFlow::WaitUntil(w) if w > t => {
                    el.set_control_flow(ControlFlow::WaitUntil(t))
                }
                _ => {}
            }
        }
        if let Some(t) = paths_panel_wake
            .into_iter()
            .chain(ai_page_wake)
            .chain(compare_wake)
            .chain(work_wake)
            .chain(net_wake)
            .chain(gate_wake)
            .chain(cursor_wake)
            .chain(series_wake)
            .chain(self.remote.script.screenshot_wake())
            .chain(self.remote_test_wake())
            .chain(self.events_wake())
            .min()
        {
            match el.control_flow() {
                ControlFlow::Wait => el.set_control_flow(ControlFlow::WaitUntil(t)),
                ControlFlow::WaitUntil(w) if w > t => {
                    el.set_control_flow(ControlFlow::WaitUntil(t))
                }
                _ => {}
            }
        }
        // Headless runs never sleep longer than the stall check (above) and the benchmark
        // timeout need.
        if headless_gallery && self.redraw_asked.is_some() {
            el.set_control_flow(ControlFlow::Poll);
        } else if self.headless() {
            let t = Instant::now() + HEADLESS_FRAME_STALL;
            match el.control_flow() {
                ControlFlow::Wait => el.set_control_flow(ControlFlow::WaitUntil(t)),
                ControlFlow::WaitUntil(w) if w > t => {
                    el.set_control_flow(ControlFlow::WaitUntil(t))
                }
                _ => {}
            }
        }
    }
}

impl ApplicationHandler<()> for App {
    // Each callback runs inside the UI's panic boundary (`ui_guard`): a panic costs that
    // callback, not the process.
    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.ui_guard("resumed", |a| a.handle_resumed(el));
    }

    fn user_event(&mut self, el: &ActiveEventLoop, _e: ()) {
        self.ui_guard("user event", |a| a.handle_user_event(el));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.ui_guard("window event", |a| a.handle_window_event(el, id, event));
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.ui_guard("about to wait", |a| a.handle_about_to_wait(el));
    }

    /// Drops everything tied to the window while winit's display connection is still open:
    /// the drag source's Wayland thread, the accessibility adapter, then the surface and the
    /// window. After `exiting` winit flushes and disconnects (on Wayland: frees the
    /// `wl_display`), so anything still using it afterwards (the drag thread's dispatch, Mesa's
    /// swapchain teardown) would read a closed fd and freed memory.
    fn exiting(&mut self, _el: &ActiveEventLoop) {
        // Where the movie was is kept; the screen may sleep again.
        self.player_save();
        self.player.awake.set(false);
        self.gate_write();
        self.stop_remote();
        performance::flush();
        self.gallery_flush_marks(true);
        self.save_book_page(true);
        self.integration.close_window();
        self.close_gestures();
        self.a11y = None;
        self.present_drop();
        self.gfx = None;
    }
}

impl App {
    /// Updates [`App::pointer_vel`] for a move to `p`.
    fn track_pointer_velocity(&mut self, p: (f32, f32)) {
        let now = Instant::now();
        let Some(q) = self.pointer else { return };
        let (v0, at) = self
            .pointer_vel
            .filter(|(_, at)| at.elapsed() <= LOUPE_STILL)
            .unwrap_or(((0.0, 0.0), now));
        let dt = (now - at).as_secs_f32();
        if dt <= 0.0005 {
            // Events of one batch: keep the time of the first.
            self.pointer_vel = Some((v0, at));
            return;
        }
        let v = ((p.0 - q.0) / dt, (p.1 - q.1) / dt);
        // Smoothed over a few events (mice report at 125–1000 Hz).
        let k = (dt / 0.03).min(1.0);
        self.pointer_vel = Some(((v0.0 + (v.0 - v0.0) * k, v0.1 + (v.1 - v0.1) * k), now));
    }

    /// The pointer moved to `p` (physical px): hover, drags and the loupe follow it.
    fn on_cursor_moved(&mut self, p: (f32, f32)) {
        // Entering the window counts as a move.
        let moved = self
            .pointer
            .is_none_or(|q| (q.0 - p.0).abs() + (q.1 - p.1).abs() > 2.0);
        if moved {
            self.pointer_moved = self.now();
        }
        // The loupe follows the (hidden) pointer whatever else the move does below,
        // some of which return early.
        if moved && self.loupe_active() {
            self.redraw = true;
            self.track_pointer_velocity(p);
        }
        if self.cursor_hidden
            && moved
            && self.loupe_view().is_none()
            && let Some(g) = &self.gfx
        {
            g.window.set_cursor_visible(true);
            self.cursor_hidden = false;
            self.redraw = true;
        }
        // Under the print dialog nothing hovers or drags.
        if self.app_dialog_open() {
            // Except a drag on the print dialog's free layout.
            if self.print.dialog.is_some() {
                let lp = self.logical(p);
                self.print_drag(lp);
            }
            // And a selection in Convert's name field (or the DICOMweb sheet's).
            if self.convert.dialog.is_some() {
                let lp = self.logical(p);
                self.convert_drag(lp);
            }
            if self.export_recipes.dialog.is_some() {
                let lp = self.logical(p);
                self.recipes_drag(lp);
            }
            if self.fileops.is_open() {
                let lp = self.logical(p);
                self.fileops_drag(lp);
            }
            if self.dicomweb.dialog.is_some() {
                let lp = self.logical(p);
                self.dicomweb_drag(lp);
            }
            if self.omero.dialog.is_some() {
                let lp = self.logical(p);
                self.omero_drag(lp);
            }
            if self.address.is_open() {
                let lp = self.logical(p);
                self.address_drag(lp);
            }
            if self.thermal_report.is_open() {
                let lp = self.logical(p);
                self.thermal_report_drag(lp);
            }
            if self.web_gallery.is_open() {
                let lp = self.logical(p);
                self.web_gallery_drag(lp);
            }
            self.pointer = Some(p);
            return;
        }
        if self.anon_mask_motion(p)
            || self.markup_motion(p)
            || self.sheets_motion(p)
            || self.measure_motion(p)
        {
            self.pointer = Some(p);
            return;
        }
        self.drag_out_motion(p);
        self.slideshow_pointer(p);
        self.chart_motion();
        self.geo_motion(p);
        self.gis_motion(p);
        if self.linked_motion(p)
            || self.mpr_motion(p)
            || self.mesh_motion(p)
            || self.relight_motion(p)
            || self.font_motion(p)
            || self.text_motion(p)
            || self.sound_motion(p)
            || self.science_motion(p)
            || self.oct_motion(p)
            || self.medical_motion(p)
            || self.develop_pointer_move(Some(self.logical(p)))
            || self.eq_pointer_move(Some(self.logical(p)))
        {
        } else if self.nav_drag {
            self.navigator_drag(p);
        } else if let Some((a, _)) = self.zoom_drag {
            self.zoom_drag = Some((a, p));
            self.redraw = true;
        } else if let Some(q) = self.dragging {
            let now = self.now();
            self.view.drag_by(p.0 - q.0, p.1 - q.1, now);
            self.dragging = Some(p);
            self.redraw = true;
        }
        self.pointer = Some(p);
        self.crosshair_follow_pointer(p);
        if self.loupe_active() {
            self.redraw = true;
        }
        let lp = self.logical(p);
        self.chrome_input(Input::PointerMove { x: lp.0, y: lp.1 });
        if self.gallery_active() {
            self.gallery_pointer_move(lp);
        }
        self.cull_pointer_move(Some(lp));
    }

    /// A window event other than `RedrawRequested` (from `window_event`, which records it as
    /// the cause of the frame it asks for).
    fn on_window_event(&mut self, el: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                if !self.welcome_close_requested() {
                    el.exit();
                }
            }
            WindowEvent::Resized(s) => self.resize(s.width, s.height),
            WindowEvent::ScaleFactorChanged { .. } => {
                self.scene = None;
                self.redraw = true;
            }
            WindowEvent::ThemeChanged(_) => {
                self.scene = None;
                self.redraw = true;
            }
            WindowEvent::RedrawRequested => {}
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m.state();
                // Shift+click in a text field extends its selection.
                self.chrome_input(Input::Modifiers(mods_of(self.mods)));
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => self.on_key(event, el),
            WindowEvent::CursorMoved { position, .. } => {
                self.on_cursor_moved((position.x as f32, position.y as f32));
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer = None;
                self.cull_pointer_move(None);
                self.chrome_input(Input::PointerLeave);
                if self.gallery_active() {
                    self.gallery_pointer_leave();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.on_mouse_button(state, button, el)
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                if !self.pinch_eats_wheel(delta, phase) {
                    self.on_wheel(delta, el);
                }
            }
            // Touchpad gestures (macOS here; Linux through gesture.rs's listener; Windows
            // sends pinch as Ctrl+wheel).
            WindowEvent::PinchGesture { delta, phase, .. } => self.on_pinch(delta, phase),
            WindowEvent::DoubleTapGesture { .. } => self.on_smart_magnify(),
            // Pens (Windows gives them as touches with a force; macOS gives pressure).
            WindowEvent::Touch(t) => {
                if self.develop_touch(&t) {
                    self.redraw = true;
                }
            }
            WindowEvent::TouchpadPressure { pressure, .. } => self.develop_pressure(pressure),
            WindowEvent::RotationGesture { delta, phase, .. } => self.on_rotation(delta, phase),
            // Onto a gallery folder tile: moved or copied there; anywhere else: opened.
            WindowEvent::DroppedFile(p) => {
                self.chrome.file_hover = false;
                if !self.gallery_file_dropped(&p)
                    && !self.player_subtitle_drop(&p)
                    && !self.science_catalogue_drop(&p)
                    && !self.astro_regions_drop(&p)
                {
                    self.open_target(p);
                }
            }
            WindowEvent::HoveredFile(p) => {
                self.chrome.file_hover = true;
                self.gallery_file_hover(Some(p))
            }
            WindowEvent::HoveredFileCancelled => {
                self.chrome.file_hover = false;
                self.gallery_file_hover(None)
            }
            WindowEvent::Ime(ime) => {
                // Never what is typed into the password prompt.
                if self.password.is_open() {
                    debug!(target: INPUT, "ime event (password prompt)");
                    self.ime_preedit(String::new());
                    return;
                }
                debug!(target: INPUT, "ime {ime:?}");
                match ime {
                    winit::event::Ime::Preedit(s, _) => self.ime_preedit(s),
                    winit::event::Ime::Commit(s) => {
                        // Some platforms report a plain key both as key text and as a commit.
                        let echo = self.last_key_text.take().is_some_and(|(k, t)| {
                            k == s && t.elapsed() < Duration::from_millis(50)
                        });
                        self.ime_preedit(String::new());
                        if !echo && self.thermal_report_wants_text() {
                            self.thermal_report_text(&s);
                        } else if !echo && self.web_gallery.wants_text() {
                            self.web_gallery_text(&s);
                        } else if !echo && self.raw_volume.is_open() {
                            // The modal headerless-volume dialog takes typed digits.
                            self.raw_volume_key(None, Some(&s));
                        } else if !echo && self.fileops.wants_text() {
                            self.fileops_text(&s);
                        } else if !echo && self.convert.wants_text() {
                            self.convert_text(&s);
                        } else if !echo && self.export_recipes.wants_text() {
                            self.recipes_text(&s);
                        } else if !echo && self.dicomweb.wants_text() {
                            self.dicomweb_text(&s);
                        } else if !echo && self.omero.wants_text() {
                            self.omero_text(&s);
                        } else if !echo && self.address.wants_text() {
                            self.address.commit(&s);
                            self.redraw = true;
                        } else if !echo && self.figure.wants_text() {
                            self.figure_key(None, Some(&s));
                        } else if !echo && self.bim_wants_text() {
                            self.bim_commit(&s);
                        } else if !echo && self.mpr_renaming() {
                            self.mpr_rename_commit(&s);
                        } else if !echo && self.markup.wants_text() {
                            self.markup.commit(&s);
                            self.redraw = true;
                        } else if !echo && self.gis_wants_text() {
                            self.gis_commit(&s);
                        } else if !echo && self.chrome.wants_text() {
                            self.chrome_input(Input::Text(s));
                        } else if !echo && self.gallery_wants_text() {
                            self.gallery_filter_text(&s);
                        }
                    }
                    winit::event::Ime::Disabled => self.ime_preedit(String::new()),
                    winit::event::Ime::Enabled => {}
                }
            }
            WindowEvent::Focused(true) => {
                self.mods = ModifiersState::empty();
                self.chrome_input(Input::Modifiers(Mods::NONE));
                // A decoder may have been installed meanwhile.
                self.decoders_focused();
                self.devices.focused = true;
            }
            WindowEvent::Focused(false) => {
                // Key releases go elsewhere now.
                self.hold = None;
                self.devices.focused = false;
                self.end_flipping();
                self.redraw = true;
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            WindowEvent::ActivationTokenDone { serial, token } => self.zynk_token(serial, token),
            _ => {}
        }
    }
}

/// The `"pan"` member of the `--bench-open` JSON: frame-time distributions for the pan-only
/// half (at 100 %) and the zooming half of the run.
fn pan_report(p: &PanBench) -> String {
    let pct = |v: &mut Vec<f64>, q: f64| -> f64 {
        v.sort_by(f64::total_cmp);
        v.get(((v.len() as f64 - 1.0) * q).round() as usize)
            .copied()
            .unwrap_or(0.0)
    };
    let refresh = 1000.0 / 60.0;
    let mut s = String::from("  \"pan\": {\n");
    let halves = [
        ("pan_100", 0.0, p.secs * 0.5),
        ("pan_zoom", p.secs * 0.5, f64::INFINITY),
    ];
    for (i, (name, from, to)) in halves.into_iter().enumerate() {
        // The first frame's interval spans the start of the bench, not a refresh.
        let f: Vec<&[f64; 6]> = p
            .frames
            .iter()
            .skip(1)
            .filter(|v| v[0] >= from && v[0] < to)
            .collect();
        let span = f.last().map_or(0.0, |v| v[0]) - f.first().map_or(0.0, |v| v[0]);
        let mut cpu: Vec<f64> = f.iter().map(|v| v[1]).collect();
        let mut gap: Vec<f64> = f.iter().map(|v| v[2]).collect();
        let mut acq: Vec<f64> = f.iter().map(|v| v[3]).collect();
        let blurry = f.iter().filter(|v| v[4] > 0.0).count();
        let n = f.len();
        let over_cpu = cpu.iter().filter(|&&v| v > refresh).count();
        let over_gap = gap.iter().filter(|&&v| v > refresh * 1.5).count();
        s += &format!(
            "    \"{name}\": {{\"frames\": {n}, \"fps\": {:.1}, \
             \"frame_cpu_ms\": {{\"p50\": {:.2}, \"p95\": {:.2}, \"p99\": {:.2}, \"max\": {:.2}, \"over_16_7\": {over_cpu}}}, \
             \"present_interval_ms\": {{\"p50\": {:.2}, \"p95\": {:.2}, \"p99\": {:.2}, \"max\": {:.2}, \"over_25\": {over_gap}}}, \
             \"acquire_ms\": {{\"p50\": {:.2}, \"max\": {:.2}}}, \"frames_with_tiles_missing\": {blurry}}}{}\n",
            if span > 0.0 {
                n.saturating_sub(1) as f64 / span
            } else {
                0.0
            },
            pct(&mut cpu, 0.5),
            pct(&mut cpu, 0.95),
            pct(&mut cpu, 0.99),
            pct(&mut cpu, 1.0),
            pct(&mut gap, 0.5),
            pct(&mut gap, 0.95),
            pct(&mut gap, 0.99),
            pct(&mut gap, 1.0),
            pct(&mut acq, 0.5),
            pct(&mut acq, 1.0),
            if i == 0 { "," } else { "" }
        );
    }
    s += "  },\n";
    s
}

/// `--bench-pan`'s path at `t` seconds of `secs`: the scale and the view centre as a fraction
/// of the image. The centre follows a Lissajous curve (periods 20 s and 13 s, about 2,000
/// screen px/s at 100 % on a 16k × 12k image). The first half stays at 100 %; the second half
/// zooms from fit to 200 % and back every 6 s, as a pinch or wheel zoom would.
fn pan_view_at(t: f64, secs: f64, fit: f32) -> (f32, (f32, f32)) {
    use std::f64::consts::TAU;
    let fx = 0.5 + 0.4 * (TAU * t / 20.0).sin();
    let fy = 0.5 + 0.4 * (TAU * t / 13.0).sin();
    let half = secs * 0.5;
    let scale = if t < half || fit <= 0.0 {
        1.0
    } else {
        let k = 0.5 - 0.5 * (TAU * (t - half) / 6.0).cos();
        let (lo, hi) = (fit.min(1.0) as f64, 2.0f64);
        (lo * (hi / lo).powf(k)) as f32
    };
    (scale, (fx as f32, fy as f32))
}

/// How long the previous image stands in for one without pixels yet, after a navigation;
/// spinners start after this too (DESIGN.md §3.2).
const STAND_IN: Duration = Duration::from_millis(300);

/// Whether the scene keeps the previous image while `img`, the current one, has no pixels:
/// not after an error; while a key is held (`flipping`), for `STAND_IN` after the
/// navigation, or until a page switched to has pixels.
fn stands_in(img: &Img, flipping: bool) -> bool {
    img.error.is_none()
        && (flipping || img.page_hold || img.shown_at.is_some_and(|t| t.elapsed() < STAND_IN))
}

/// How long a headless run waits for a frame callback before drawing on its own (a visible
/// window gets one within a refresh or two).
const HEADLESS_FRAME_STALL: Duration = Duration::from_millis(100);

/// The metadata report with a row for the image's gain map in its image group: which kind
/// (Apple, ISO 21496-1, Ultra HDR) and the headroom it reaches.
fn with_gain_map(report: &MetadataReport, gm: &specular_core::GainMap) -> MetadataReport {
    let mut r = report.clone();
    let Some(g) = r.summary.iter_mut().find(|g| g.key == "image") else {
        return r;
    };
    let p = &gm.params;
    let headroom = p.alt_headroom.max(p.base_headroom).exp2();
    let ratio = I18n::global().localizer().number(f64::from(headroom), 1);
    g.items.retain(|i| i.key != "gain-map");
    g.items.push(specular_metadata::SummaryItem {
        key: "gain-map",
        label: t!("app-info-gain-map"),
        value: gm.source.to_owned(),
        detail: Some(t!("app-info-gain-map-headroom", headroom = ratio)),
    });
    r
}

/// Lets each open decoder session but those `keep` names drop what it holds only to speed up
/// later decodes ([`Session::trim_memory`]). Returns the bytes released.
fn trim_open_sessions<'a>(
    sessions: impl Iterator<Item = (ImageId, &'a dyn Session)>,
    keep: impl Fn(ImageId) -> bool,
) -> usize {
    sessions
        .filter(|&(id, _)| !keep(id))
        .map(|(_, s)| s.trim_memory())
        .sum()
}

/// Where stepping `delta` places from `cur` lands among `n` entries, counting only those
/// `keep` accepts (`cur` itself is always where it starts). At an end it stops at the last
/// kept entry, or with `wrap` goes round to the other end. Returns the entry and whether it
/// wrapped; `None` for an empty listing or `cur` out of range.
fn step_among(
    n: usize,
    cur: usize,
    delta: i64,
    wrap: bool,
    keep: impl Fn(usize) -> bool,
) -> Option<(usize, bool)> {
    if cur >= n {
        return None;
    }
    let dir = delta.signum();
    // Never more steps than a lap of the listing (Home and End ask for huge ones).
    let mut left = delta.unsigned_abs().min(n as u64);
    let (mut at, mut i, mut wrapped, mut walked) = (cur, cur as i64, false, 0usize);
    while left > 0 && walked < 2 * n {
        i += dir;
        walked += 1;
        if i < 0 || i >= n as i64 {
            if !wrap {
                break;
            }
            i = i.rem_euclid(n as i64);
            wrapped = true;
        }
        if i as usize == cur {
            // Round to where it began: nothing else of its kind.
            break;
        }
        if keep(i as usize) {
            at = i as usize;
            left -= 1;
        }
    }
    Some((at, wrapped && at != cur))
}

/// After a wake-up: when the OS memory-pressure level changed, has decoder sessions trimmed
/// (`trim(false)` on a warning: all but the images on screen; `trim(true)` when critical:
/// all) and returns true, for the caller to re-plan prefetch at the scaled budgets.
fn on_pressure_change(trim: impl FnOnce(bool)) -> bool {
    if !specular_platform::memory_pressure_changed() {
        return false;
    }
    match specular_platform::memory_pressure() {
        specular_platform::MemoryPressure::Normal => {}
        specular_platform::MemoryPressure::Warn => trim(false),
        specular_platform::MemoryPressure::Critical => trim(true),
    }
    true
}

#[cfg(test)]
mod step_tests {
    use super::step_among;

    /// Pictures at 0, 2, 3, 5; text files at 1 and 4.
    const TEXT: [bool; 6] = [false, true, false, false, true, false];

    fn kind(from: usize) -> impl Fn(usize) -> bool {
        move |i| TEXT[i] == TEXT[from]
    }

    /// Stepping from a picture skips the text files; from a text file it moves among them.
    #[test]
    fn steps_among_files_of_the_same_kind() {
        assert_eq!(step_among(6, 0, 1, false, kind(0)), Some((2, false)));
        assert_eq!(step_among(6, 3, 1, false, kind(3)), Some((5, false)));
        assert_eq!(step_among(6, 5, -2, false, kind(5)), Some((2, false)));
        assert_eq!(step_among(6, 1, 1, false, kind(1)), Some((4, false)));
        assert_eq!(step_among(6, 4, -1, false, kind(4)), Some((1, false)));
        // At the end: it stays, or wraps round past the other kind.
        assert_eq!(step_among(6, 5, 1, false, kind(5)), Some((5, false)));
        assert_eq!(step_among(6, 5, 1, true, kind(5)), Some((0, true)));
        assert_eq!(step_among(6, 4, 1, true, kind(4)), Some((1, true)));
        // A page jump stops at the last of its kind; End and Home go all the way.
        assert_eq!(step_among(6, 0, 10, false, kind(0)), Some((5, false)));
        assert_eq!(step_among(6, 5, i64::MIN, false, kind(5)), Some((0, false)));
        // The only text file: nowhere to go, even wrapping.
        let one = [false, true, false];
        assert_eq!(step_among(3, 1, 1, true, |i| one[i]), Some((1, false)));
        // Everything of one kind: plain stepping.
        assert_eq!(step_among(4, 1, 2, false, |_| true), Some((3, false)));
        assert_eq!(step_among(0, 0, 1, true, |_| true), None);
    }
}

#[cfg(test)]
mod tests {
    use super::{BackStep, back_step};

    /// A frame drawn at once while a redraw request is still out clears both: the request's
    /// own frame, coming later from the compositor, is not taken for an asked (and so
    /// uncaused) one (5-mixed, sound files playing on).
    #[test]
    fn a_frame_drawn_at_once_clears_both_asks() {
        let mut requested = Some(std::time::Instant::now());
        let mut at_once = true;
        assert!(super::frame_asked(&mut requested, &mut at_once));
        assert!(requested.is_none() && !at_once);
        assert!(!super::frame_asked(&mut requested, &mut at_once));
        let mut at_once = true;
        assert!(super::frame_asked(&mut None, &mut at_once));
        assert!(!at_once);
    }

    /// A simulated Critical level, reported the way the OS sources report it, wakes the event
    /// loop's callback and reaches the session trim with every image, the one on screen too;
    /// a warning spares the images on screen.
    #[test]
    fn critical_memory_pressure_trims_every_session() {
        use specular_decode::Session;
        use specular_platform::{MemoryPressure, simulate_memory_pressure};
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// A session whose caches hold 1 MiB until trimmed.
        struct Cached(AtomicUsize);
        impl Cached {
            fn new() -> Cached {
                Cached(AtomicUsize::new(1 << 20))
            }
        }
        impl Session for Cached {
            fn info(&self) -> &specular_decode::ImageInfo {
                unreachable!()
            }
            fn source(&self) -> &std::sync::Arc<specular_core::Source> {
                unreachable!()
            }
            fn decode_preview(&self) -> Option<specular_decode::Result<specular_core::Raster>> {
                None
            }
            fn decode(
                &self,
                _: &specular_decode::DecodeRequest,
                _: &specular_core::CancelToken,
            ) -> specular_decode::Result<specular_core::Raster> {
                unreachable!()
            }
            fn memory_bytes(&self) -> usize {
                self.0.load(Ordering::Relaxed)
            }
            fn trim_memory(&self) -> usize {
                self.0.swap(0, Ordering::Relaxed)
            }
        }

        let (tx, rx) = crossbeam_channel::unbounded();
        specular_platform::watch_memory_pressure(move || {
            let _ = tx.send(());
        });
        let sessions = [(1, Cached::new()), (2, Cached::new()), (3, Cached::new())];
        let current = 2;
        let round = |level| {
            simulate_memory_pressure(level);
            rx.recv_timeout(std::time::Duration::from_secs(5))
                .expect("the event loop is woken");
            let mut freed = None;
            let changed = super::on_pressure_change(|all| {
                freed = Some(super::trim_open_sessions(
                    sessions.iter().map(|(id, s)| (*id, s as &dyn Session)),
                    |id| !all && id == current,
                ));
            });
            assert!(changed);
            freed
        };
        assert_eq!(round(MemoryPressure::Warn), Some(2 << 20));
        assert_eq!(sessions[1].1.memory_bytes(), 1 << 20, "on screen: kept");
        assert_eq!(round(MemoryPressure::Critical), Some(1 << 20));
        assert!(sessions.iter().all(|(_, s)| s.memory_bytes() == 0));
        assert_eq!(round(MemoryPressure::Normal), None, "nothing to trim");
        assert!(!super::on_pressure_change(|_| panic!("no change")));
    }

    /// Digit-row keys reach the channel views (`Shift+1…5`) and the zoom levels (`1…`) on
    /// simulated German and French layouts and from wtype's virtual keyboard, whose key codes
    /// are arbitrary (the first key it types is Escape, the next ones count up the row).
    #[test]
    fn digit_row_keys_on_other_layouts_and_virtual_keyboards() {
        use specular_gfx::winit::keyboard::{Key as WK, KeyCode as K, PhysicalKey};
        use specular_input::{Action, Key, KeyCombo, Keymap, Mods};
        let km = Keymap::viewer_default();
        let press = |code: K, typed: char, unshifted: char, shift: bool| {
            let ch = |c: char| WK::Character(c.to_string().into());
            let digit = super::digit_of(PhysicalKey::Code(code), &ch(typed), &ch(unshifted), shift);
            let mods = if shift { Mods::SHIFT } else { Mods::NONE };
            km.lookup(&km.resolve(KeyCombo::new(Key::Char(typed), mods), shift, digit))
        };
        let red = Some(Action::ChannelRed);
        // US.
        assert_eq!(press(K::Digit1, '!', '1', true), red);
        assert_eq!(
            press(K::Digit1, '1', '1', false),
            Some(Action::ZoomTo(100.0))
        );
        // German (QWERTZ): Shift+2 types '"', Shift+3 '§', Shift+5 '%'.
        assert_eq!(press(K::Digit1, '!', '1', true), red);
        assert_eq!(press(K::Digit2, '"', '2', true), Some(Action::ChannelGreen));
        assert_eq!(press(K::Digit3, '§', '3', true), Some(Action::ChannelBlue));
        assert_eq!(press(K::Digit4, '$', '4', true), Some(Action::ChannelAlpha));
        assert_eq!(press(K::Digit5, '%', '5', true), Some(Action::ChannelLuma));
        assert_eq!(
            press(K::Digit2, '2', '2', false),
            Some(Action::ZoomTo(200.0))
        );
        // French (AZERTY): the row types "&é\"'(" unshifted and the digits with Shift.
        assert_eq!(press(K::Digit1, '1', '&', true), red);
        assert_eq!(press(K::Digit2, '2', 'é', true), Some(Action::ChannelGreen));
        assert_eq!(press(K::Digit3, '3', '"', true), Some(Action::ChannelBlue));
        assert_eq!(
            press(K::Digit4, '4', '\'', true),
            Some(Action::ChannelAlpha)
        );
        assert_eq!(press(K::Digit5, '5', '(', true), Some(Action::ChannelLuma));
        assert_eq!(
            press(K::Digit1, '&', '&', false),
            Some(Action::ZoomTo(100.0))
        );
        assert_eq!(
            press(K::Digit2, 'é', 'é', false),
            Some(Action::ZoomTo(200.0))
        );
        // Its "!" key (right of ":") is not Shift+1, with or without Shift ("§").
        assert_eq!(press(K::Slash, '!', '!', false), None);
        assert_eq!(press(K::Slash, '§', '!', true), None);
        // wtype: `-M shift -k 1` sends "1" with Shift, `-M shift '!'` a one-level "!" key.
        assert_eq!(press(K::Escape, '1', '1', true), red);
        assert_eq!(press(K::Escape, '!', '!', true), red);
        assert_eq!(press(K::Escape, '@', '@', true), Some(Action::ChannelGreen));
        assert_eq!(press(K::Digit3, '%', '%', true), Some(Action::ChannelLuma));
        // Its letters on what happen to be digit-row codes stay letters.
        assert_eq!(press(K::Digit2, 'a', 'a', false), None);
        // The keypad is not the row.
        assert_eq!(
            super::digit_of(
                PhysicalKey::Code(K::Numpad1),
                &WK::Character("1".into()),
                &WK::Character("1".into()),
                true
            ),
            None
        );
    }

    /// The /tmp/2.jpeg spinner. The image left the prefetch window while its open job was
    /// past opening; the job still sent the header and stopped. Its plan must not be recorded
    /// as in flight, or coming back to the image finds "levels requested", requests nothing,
    /// and spins forever.
    #[test]
    fn a_header_from_a_cancelled_open_leaves_nothing_in_flight() {
        use super::Img;
        use crate::loader::Unfinished;
        use specular_core::CancelToken;
        let mut img = Img::new("2.jpeg".into());
        let t1 = img.token.clone();
        img.opening = true;
        // schedule(): out of the window.
        t1.cancel();
        img.opening = false;
        img.levels.clear();
        assert!(!img.opened(&t1, vec![1]));
        assert!(
            img.levels.is_empty() && !img.opening,
            "nothing is decoding it"
        );
        // Back in the window: a new open is in flight; the old job's events change nothing.
        let t2 = CancelToken::new();
        img.token = t2.clone();
        img.opening = true;
        assert!(!img.opened(&t1, vec![1]));
        assert!(!img.unfinished(&t1, Unfinished::Level(1)));
        assert!(img.opening && img.levels.is_empty());
        // The live job's plan is in flight until each level arrives or reports back.
        assert!(img.opened(&t2, vec![8, 1]));
        assert_eq!(img.levels, [8, 1]);
        assert!(!img.opening);
        assert!(img.unfinished(&t2, Unfinished::Level(8)));
        assert_eq!(img.levels, [1]);
    }

    #[test]
    fn back_backs_out_one_layer_per_press() {
        // Panel open, zoomed, fullscreen: three presses, one layer each.
        let (mut panel, mut at_fit, mut fullscreen) = (true, false, true);
        let mut steps = Vec::new();
        for _ in 0..4 {
            let s = back_step(panel, at_fit, fullscreen);
            match s {
                BackStep::Closed => panel = false,
                BackStep::Fit => at_fit = true,
                BackStep::ExitFullscreen => fullscreen = false,
                BackStep::Nothing => {}
            }
            steps.push(s);
        }
        assert_eq!(
            steps,
            [
                BackStep::Closed,
                BackStep::Fit,
                BackStep::ExitFullscreen,
                BackStep::Nothing
            ]
        );
        // A layer is closed first even when not at fit; fullscreen waits for fit.
        assert_eq!(back_step(true, false, false), BackStep::Closed);
        assert_eq!(back_step(false, false, true), BackStep::Fit);
        assert_eq!(back_step(false, true, true), BackStep::ExitFullscreen);
        assert_eq!(back_step(false, true, false), BackStep::Nothing);
    }

    /// From any state, Back reaches the end of its ladder in at most three presses and then
    /// stays there: no step undoes another (fullscreen is only ever left, never entered).
    #[test]
    fn back_settles_at_the_end_and_stays() {
        for bits in 0..8u8 {
            let (mut panel, mut at_fit, mut fullscreen) =
                (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
            let mut steps = Vec::new();
            for _ in 0..6 {
                let s = back_step(panel, at_fit, fullscreen);
                match s {
                    BackStep::Closed => panel = false,
                    BackStep::Fit => at_fit = true,
                    BackStep::ExitFullscreen => {
                        assert!(fullscreen, "Back never enters fullscreen");
                        fullscreen = false;
                    }
                    BackStep::Nothing => {}
                }
                steps.push(s);
            }
            let end = steps.iter().position(|s| *s == BackStep::Nothing).unwrap();
            assert!(end <= 3, "{bits}: {steps:?}");
            assert!(
                steps[end..].iter().all(|s| *s == BackStep::Nothing),
                "{bits}: {steps:?}"
            );
        }
    }
}
