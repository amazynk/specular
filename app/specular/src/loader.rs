//! Background loading: opening files, decoding previews and levels, region decodes for
//! detail tiles, and folder scans. Results come back over a channel; the UI thread is woken
//! through a callback so an idle viewer uses no CPU.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};
use specular_core::{CancelToken, JobHandle, JobPool, Priority, Raster};
use specular_decode::{DecodeRequest, Rect, Registry, Session};
use specular_log::{DECODE, FOLDER, METADATA, TILES, debug, trace};
use specular_settings::SortOrder;

use crate::display_cache::{DisplayCache, Key};
use crate::open_report::DecodeStage;

pub type ImageId = u64;

/// What an overview raster is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewKind {
    Thumbnail,
    /// Whole image at 1/scale.
    Level(u32),
}

impl OverviewKind {
    /// Lower is sharper (thumbnail is worst).
    pub fn rank(self) -> u32 {
        match self {
            OverviewKind::Thumbnail => 1000,
            OverviewKind::Level(s) => s,
        }
    }
}

/// The stages of an open job before decoding (ms), for the open report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OpenTimes {
    pub read_ms: f64,
    pub parse_ms: f64,
    pub colour_ms: f64,
    pub bytes: u64,
    pub in_archive: bool,
}

/// A job of an image that ended without delivering its result (cancelled, or the decoder
/// gave up without an error).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfinished {
    /// The open job: no `Opened` was sent.
    Open,
    /// A whole-image level (from an open job's plan or `decode_level`).
    Level(u32),
}

/// A motion clip's frames, decoder open.
pub struct MotionClip {
    pub frames: Box<dyn specular_decode::Frames>,
    pub info: specular_decode::AnimationInfo,
}

/// A plane read for the science view, with its display limits (`None`: no finite value) and,
/// for histogram equalisation, its table.
pub struct SciencePlane {
    pub plane: Arc<specular_decode::science::NdPlane>,
    pub limits: Option<(f32, f32)>,
    pub lut: Vec<f32>,
    /// The midtones balance of the automatic stretch ([`specular_core::nd::stf`]); 0.5
    /// otherwise.
    pub mid: f32,
}

pub enum Event {
    /// A [`Loader::background`] task panicked (`task`: its thread name; `error`: the
    /// panic's brief): the app clears what waited for its reply and says so.
    BackgroundFailed {
        task: String,
        error: String,
    },
    /// Vector maps beside a georeferenced image and geotagged photos, placed on it.
    GeoOverlay {
        id: ImageId,
        overlay: Arc<crate::app::GeoOverlay>,
    },
    /// A base map tile for the map view (`tile`: width, height, RGBA8; `None` when the source
    /// has none or it failed), from the fetch threads of base map `generation`.
    BasemapTile {
        generation: u64,
        key: (u8, u32, u32),
        tile: Option<(u32, u32, Arc<[u8]>)>,
    },
    /// `token`: the token the open job ran with (the image may have a newer one by now).
    Opened {
        id: ImageId,
        session: Arc<dyn Session>,
        lut: Option<Arc<specular_render::icc::SourceLut>>,
        /// The overview levels the open job goes on to decode (now in flight).
        plan: Vec<u32>,
        ms: f64,
        at: f64,
        token: CancelToken,
        /// The open's stages, for the open report.
        times: OpenTimes,
    },
    Overview {
        id: ImageId,
        kind: OverviewKind,
        raster: Arc<Raster>,
        ms: f64,
        at: f64,
        /// How a level was made (the open report); `None` for embedded previews.
        stage: Option<DecodeStage>,
    },
    Tiles {
        id: ImageId,
        level: u8,
        tiles: Vec<(u32, u32)>,
        raster: Raster,
        ms: f64,
    },
    TilesFailed {
        id: ImageId,
        level: u8,
        tiles: Vec<(u32, u32)>,
    },
    Failed {
        id: ImageId,
        error: String,
    },
    /// Showing the image will likely need more memory than the system has available
    /// (bytes, estimated from the headers before decoding starts).
    MemoryShort {
        id: ImageId,
        need: u64,
        available: u64,
    },
    /// A job ended without a result: every job of an image reports either its result,
    /// `Failed`, or this, so the app always knows what is still in flight.
    Unfinished {
        id: ImageId,
        token: CancelToken,
        job: Unfinished,
        cancelled: bool,
    },
    /// The images in `dir` (the scan that was asked for; older scans are stale).
    Folder {
        dir: PathBuf,
        files: Vec<PathBuf>,
    },
    /// The images in `dir` for a slideshow on another display (its own listing, with
    /// subfolders; the viewer's listing stays as it is).
    ShowListing {
        dir: PathBuf,
        files: Vec<PathBuf>,
    },
    Metadata {
        id: ImageId,
        report: Arc<specular_metadata::MetadataReport>,
    },
    /// The DICOM series around `file` (app/series.rs), found in `ms`.
    DicomSeries {
        file: PathBuf,
        scan: crate::app::ScanResult,
        /// The whole slides among the files found, each slide's files together, its
        /// full-resolution level first (`specular_dicom::series::whole_slides`).
        slides: Vec<Vec<PathBuf>>,
        ms: f64,
    },
    /// A folder opened is a patient's medium (app/studies.rs).
    StudiesDetected {
        generation: u64,
        found: specular_dicom::media::Detected,
    },
    /// What the reading of a medium found so far (app/studies.rs).
    Studies {
        generation: u64,
        update: crate::app::StudiesUpdate,
    },
    /// A series' thumbnail for the studies browser (`None`: none could be made).
    StudyThumb {
        generation: u64,
        id: u64,
        thumb: Option<specular_dicom::media::Thumb>,
    },
    /// The headers of a series chosen in the studies browser, assembled.
    StudySeries {
        generation: u64,
        id: u64,
        scan: crate::app::ScanResult,
    },
    /// A removable medium with a DICOMDIR was mounted at `mount`.
    // Mounts are watched on Linux only so far (app/studies.rs, `watch_media`).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    StudiesMedium {
        mount: PathBuf,
    },
    /// The data set of the DICOM file `file`, for the tag browser.
    DicomTags {
        file: PathBuf,
        tree: Result<specular_dicom::TagTree, String>,
        /// The file is a structured report: its content tree (browsed in its place).
        report: Option<Box<specular_dicom::sr::Report>>,
    },
    /// What a report item points at, found in the report's folder (file and 0-based frame),
    /// or the message key for why not.
    ReportJump {
        file: PathBuf,
        found: Result<(PathBuf, u32), &'static str>,
    },
    /// The record of the waveform file `file`, for the plot sheet.
    Waveform {
        file: PathBuf,
        waves: Result<Arc<specular_dicom::waveform::Waveforms>, String>,
    },
    /// The frame locations of the OCT volume `file` (`None`: it is not one) and its
    /// localiser's file, when found.
    Oct {
        file: PathBuf,
        oct: Option<Arc<specular_dicom::oct::Oct>>,
        localiser: Option<PathBuf>,
    },
    /// The image is a Motion Photo or Live Photo (found by the metadata job).
    Motion {
        id: ImageId,
        info: Arc<specular_video::MotionInfo>,
    },
    /// The motion clip asked for by [`Loader::motion_clip`], ready to play (its decoder open),
    /// or why it cannot play. `ms`: from the request to the decoder being open.
    MotionClip {
        id: ImageId,
        clip: Result<MotionClip, specular_video::play::PlayError>,
        ms: f64,
    },
    /// The image's gain map (Ultra HDR / ISO 21496-1), decoded after its first level.
    GainMap {
        id: ImageId,
        map: Arc<specular_core::GainMap>,
    },
    /// A DICOM frame's modality values for window / level (`app/medical.rs`): the frame,
    /// and its values as f32 for the GPU (`None` for colour frames), or why there are none.
    MedicalValues {
        id: ImageId,
        index: u32,
        values: Result<(Arc<specular_decode::ModalityFrame>, Option<Vec<f32>>), String>,
    },
    /// A scientific image's planes for the science view (`app/science.rs`): one, or one per
    /// composited channel, with their limits and tables, or why there are none.
    ScienceValues {
        id: ImageId,
        planes: Result<Vec<SciencePlane>, String>,
    },
    /// A remote Zarr store's images listed and their links written (`app/zarr.rs`): the
    /// first image's link, or why not. `address` is the store's, without query or user.
    ZarrOpened {
        address: String,
        result: Result<std::path::PathBuf, String>,
    },
    /// A DICOM presentation state (GSPS) read for the image (`app/medical.rs`).
    MedicalPresentation {
        id: ImageId,
        path: std::path::PathBuf,
        ps: Result<Arc<specular_dicom::gsps::PresentationState>, String>,
    },
    /// The open book again with its ComicInfo, read off the UI thread
    /// ([`crate::book::Book::read_comic_info`], [`crate::book::Book::with_comic`]).
    ComicInfo {
        book: Arc<crate::book::Book>,
    },
    /// File dialog result.
    Picked(Option<PathBuf>),
    /// From the screen-reader adapter (any thread).
    Accessibility(crate::a11y::A11yEvent),
    Trashed {
        path: PathBuf,
        result: Result<(), String>,
    },
    Restored {
        path: PathBuf,
        result: Result<(), String>,
    },
    Copied {
        what: Copied,
        result: Result<(), String>,
    },
}

impl Event {
    /// The variant's name (the app gates' record of what drew a frame).
    pub fn name(&self) -> &'static str {
        match self {
            Event::GeoOverlay { .. } => "GeoOverlay",
            Event::BasemapTile { .. } => "BasemapTile",
            Event::Opened { .. } => "Opened",
            Event::Overview { .. } => "Overview",
            Event::Tiles { .. } => "Tiles",
            Event::TilesFailed { .. } => "TilesFailed",
            Event::Failed { .. } => "Failed",
            Event::MemoryShort { .. } => "MemoryShort",
            Event::Unfinished { .. } => "Unfinished",
            Event::Folder { .. } => "Folder",
            Event::ShowListing { .. } => "ShowListing",
            Event::Metadata { .. } => "Metadata",
            Event::DicomSeries { .. } => "DicomSeries",
            Event::StudiesDetected { .. } => "StudiesDetected",
            Event::Studies { .. } => "Studies",
            Event::StudyThumb { .. } => "StudyThumb",
            Event::StudySeries { .. } => "StudySeries",
            Event::StudiesMedium { .. } => "StudiesMedium",
            Event::DicomTags { .. } => "DicomTags",
            Event::ReportJump { .. } => "ReportJump",
            Event::Waveform { .. } => "Waveform",
            Event::Oct { .. } => "Oct",
            Event::Motion { .. } => "Motion",
            Event::MotionClip { .. } => "MotionClip",
            Event::GainMap { .. } => "GainMap",
            Event::MedicalValues { .. } => "MedicalValues",
            Event::ScienceValues { .. } => "ScienceValues",
            Event::ZarrOpened { .. } => "ZarrOpened",
            Event::MedicalPresentation { .. } => "MedicalPresentation",
            Event::ComicInfo { .. } => "ComicInfo",
            Event::Trashed { .. } => "Trashed",
            Event::Restored { .. } => "Restored",
            Event::Copied { .. } => "Copied",
            Event::Picked(_) => "Picked",
            Event::Accessibility(_) => "Accessibility",
            Event::BackgroundFailed { .. } => "BackgroundFailed",
        }
    }
}

/// What a clipboard copy put there, for its toast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Copied {
    Path,
    Image,
    Report,
    /// A metadata value ("Copy value" in the info panel).
    Value,
    /// Text copied or cut in the settings.toml editor (no toast: it is an editor).
    Editor,
    /// The measurement list as CSV.
    Measurements,
    /// A map position ("Copy coordinate").
    Coordinate,
    /// The key images, side by side.
    KeyImages,
    /// "Copy view as command": a shell or Python snippet.
    ViewCommand,
}

pub type Wake = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct Loader {
    pool: Arc<JobPool>,
    /// Thumbnails (gallery, similar-image scans), at background priority; made on first use.
    thumbs: Arc<OnceLock<Arc<JobPool>>>,
    registry: Arc<Registry>,
    tx: Sender<Event>,
    wake: Arc<parking_lot::Mutex<Option<Wake>>>,
    /// The window size once known, and the open that planned before it was.
    viewport: Arc<parking_lot::Mutex<ViewportHint>>,
    /// The persistent display cache and the file version of each open image.
    display: Arc<DisplayCacheSlot>,
    /// Open archives: their pages' virtual paths open from the archive.
    books: crate::book::Books,
}

/// The display cache, once the settings say where (or that there is none).
#[derive(Default)]
struct DisplayCacheSlot {
    state: parking_lot::Mutex<CacheState>,
    ready: parking_lot::Condvar,
    keys: parking_lot::Mutex<std::collections::HashMap<ImageId, Key>>,
}

#[derive(Default)]
enum CacheState {
    #[default]
    Off,
    /// The settings are still loading (the first open starts before them). Entries found in
    /// this folder (where the cache will likely be) are worth waiting for.
    Pending(PathBuf),
    On(Arc<DisplayCache>),
}

/// Longest a level decode waits for the settings to say whether there is a display cache.
const CACHE_CONFIG_WAIT: std::time::Duration = std::time::Duration::from_millis(50);
/// Levels that decode faster than this are not worth a disk entry.
const CACHE_MIN_DECODE_MS: f64 = 10.0;

/// The window's size for opens submitted before the window existed. An open that starts
/// after the window appears plans with it; one that planned before (the usual case at
/// startup: the header is read in about 2 ms, the window takes 10–15) leaves its sharp level
/// here, and the hint starts it as soon as the size is known instead of when the GPU is ready.
#[derive(Default)]
struct ViewportHint {
    size: Option<(u32, u32)>,
    waiting: Option<WaitingOpen>,
    /// Levels started by the hint, until the app records them (`take_late_levels`).
    late: Vec<(ImageId, CancelToken, u32)>,
}

struct WaitingOpen {
    id: ImageId,
    session: Arc<dyn Session>,
    size: (u32, u32),
    plan: Vec<u32>,
    prio: Priority,
    token: CancelToken,
}

/// Sends [`Event::Unfinished`] when dropped before [`Outcome::finish`]: a job cancelled
/// before it started is dropped by the pool without running, and one cancelled midway
/// returns early; either way the app learns that nothing more will come.
struct Outcome {
    loader: Loader,
    id: ImageId,
    token: CancelToken,
    job: Unfinished,
    finished: bool,
}

impl Outcome {
    fn new(loader: &Loader, id: ImageId, token: &CancelToken, job: Unfinished) -> Outcome {
        Outcome {
            loader: loader.clone(),
            id,
            token: token.clone(),
            job,
            finished: false,
        }
    }
    fn finish(&mut self) {
        self.finished = true;
    }
}

impl Drop for Outcome {
    fn drop(&mut self) {
        if !self.finished && std::thread::panicking() {
            // The job's boundary ([`Loader::guarded`]) sends `Event::Failed` with the panic;
            // "unfinished" would make the app ask again, and panic again.
            specular_log::debug!(target: DECODE, "img={} {:?} ended by a panic", self.id, self.job);
        } else if !self.finished {
            let cancelled = self.token.is_cancelled();
            specular_log::debug!(target: DECODE, "img={} {:?} ended without a result (cancelled: {cancelled})", self.id, self.job);
            self.loader.send(Event::Unfinished {
                id: self.id,
                token: self.token.clone(),
                job: self.job,
                cancelled,
            });
        }
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Longest wait one major page fault is charged with: a disk seek or a network round trip
/// with readahead. The decoding thread also waits for its helper threads, which is not I/O;
/// this keeps a fault or two from turning that wait into "reading".
const MAX_FAULT_MS: f64 = 25.0;

/// A level's decode for the open report: the path note the decoder left on this thread, and
/// the time spent reading the memory-mapped file (estimated from this thread's major faults
/// and its wall minus CPU time since `before`). `probed`: the hardware decoder probe had
/// registered its findings before the decode started.
fn decode_stage(
    scale: u32,
    took: f64,
    before: Option<specular_core::paths::ThreadUsage>,
    probed: bool,
) -> DecodeStage {
    let path = specular_core::paths::take_note().map(|n| crate::open_report::path_facts(n, probed));
    let (faults, io_ms) = match (before, specular_core::paths::thread_usage()) {
        (Some(a), Some(b)) => {
            let faults = b.major_faults.saturating_sub(a.major_faults);
            let idle = (took - (b.cpu_us - a.cpu_us) / 1e3).max(0.0);
            (faults, idle.min(faults as f64 * MAX_FAULT_MS))
        }
        _ => (0, 0.0),
    };
    DecodeStage {
        scale,
        ms: took,
        io_ms,
        faults,
        from_cache: false,
        path,
    }
}

/// Overview decode plan for an image: whole-image scales, coarsest first. `scaled_cheap`:
/// the decoder's reduced scales cost less than its full one ([`Session::scaled_decode_cheap`]);
/// without that a quick 1/8 is never planned, since it would take as long as the sharp level.
pub fn overview_plan(
    (w, h): (u32, u32),
    progressive: bool,
    scaled_cheap: bool,
    viewport: Option<(u32, u32)>,
    prefetch: bool,
    cap: u32,
) -> Vec<u32> {
    let target = viewport
        .map(|(vw, vh)| needed_scale(w, h, vw, vh, cap))
        .unwrap_or_else(|| guess_scale(w, h, cap));
    let mut v = Vec::new();
    let mp_at = |s: u32| (w as f64 / s as f64) * (h as f64 / s as f64);
    // A quick 1/8 first when the sharp level takes noticeably longer (big or progressive).
    // Not before a sequential 1/4: both parse all the entropy data, 1/4 costs only 1.2-1.8x
    // the 1/8 (108 MP: 131 vs 156 ms), and the two running together took 228 ms each.
    let cheap_sharp = !progressive && target == 4;
    if target < 8
        && scaled_cheap
        && !prefetch
        && !cheap_sharp
        && (progressive || mp_at(target) > 2.0e6)
        && w >= 1024
    {
        v.push(8);
    }
    v.push(target);
    v
}

/// The coarsest power-of-two scale up to `cap` (8, 4, 2, 1 for JPEG; a slide pyramid's
/// coarsest level, [`scale_cap`]) whose image still covers the viewport at fit.
pub fn needed_scale(w: u32, h: u32, vw: u32, vh: u32, cap: u32) -> u32 {
    let fit = (vw as f32 / w as f32).min(vh as f32 / h as f32).min(1.0);
    let mut s = cap.max(1).next_power_of_two();
    while s > 1 {
        if 1.0 / s as f32 >= fit * 0.999 {
            return s;
        }
        s /= 2;
    }
    1
}

/// The coarsest scale whole-image decodes of this image may use: 8 (the JPEG scales), or
/// more for a decoder with coarser cheap levels (a whole-slide pyramid).
pub fn scale_cap(info: &specular_decode::ImageInfo) -> u32 {
    info.scales.last().copied().unwrap_or(8).max(8)
}

/// Without a viewport: the scale for a 4K screen, for images (slides) whose coarsest scale
/// is beyond 8; 8 otherwise.
fn guess_scale(w: u32, h: u32, cap: u32) -> u32 {
    if cap <= 8 {
        8
    } else {
        needed_scale(w, h, 3840, 2160, cap)
    }
}

impl Loader {
    pub fn new(threads: usize) -> (Loader, Receiver<Event>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let l = Loader {
            pool: Arc::new(JobPool::new(threads, "dec")),
            thumbs: Arc::default(),
            registry: Arc::new(Registry::new()),
            tx,
            wake: Arc::new(parking_lot::Mutex::new(None)),
            viewport: Arc::default(),
            display: Arc::default(),
            books: Default::default(),
        };
        (l, rx)
    }

    /// Until [`Loader::set_display_cache`], levels with an entry in `dir` wait (briefly) for
    /// it; others decode right away.
    pub fn expect_display_cache(&self, dir: PathBuf) {
        *self.display.state.lock() = CacheState::Pending(dir);
    }

    pub fn set_display_cache(&self, cache: Option<Arc<DisplayCache>>) {
        *self.display.state.lock() = cache.map_or(CacheState::Off, CacheState::On);
        self.display.ready.notify_all();
    }

    /// The cache to look `key`'s level `scale` up in.
    fn display_cache_for(&self, key: &Key, scale: u32) -> Option<Arc<DisplayCache>> {
        let mut g = self.display.state.lock();
        if let CacheState::Pending(dir) = &*g {
            if !dir.join(key.file_name(scale)).is_file() {
                return None;
            }
            let t0 = Instant::now();
            let _ = self.display.ready.wait_for(&mut g, CACHE_CONFIG_WAIT);
            debug!(target: DECODE, "waited {:.1} ms for the display cache settings", ms(t0));
        }
        match &*g {
            CacheState::On(c) => Some(c.clone()),
            _ => None,
        }
    }

    /// Drops the display cache's levels of `key` (see [`DisplayCache::forget`]).
    pub fn forget_display(&self, key: &Key) {
        if let Some(c) = self.display_cache_now() {
            c.forget(key);
        }
    }

    /// The cache to store into, if configured by now.
    fn display_cache_now(&self) -> Option<Arc<DisplayCache>> {
        match &*self.display.state.lock() {
            CacheState::On(c) => Some(c.clone()),
            _ => None,
        }
    }

    fn cache_key(&self, id: ImageId) -> Option<Key> {
        self.display.keys.lock().get(&id).cloned()
    }

    pub fn set_wake(&self, w: Wake) {
        *self.wake.lock() = Some(w);
    }

    /// Calls the event loop's wake from any thread (animation players).
    pub fn waker(&self) -> Wake {
        let wake = self.wake.clone();
        Arc::new(move || {
            if let Some(w) = wake.lock().as_ref() {
                w();
            }
        })
    }

    /// The table archive pages are opened through.
    pub fn books(&self) -> &crate::book::Books {
        &self.books
    }

    /// Whether the decoders handle files named like `path` (archive page lists).
    pub fn supports_extension(&self, path: &Path) -> bool {
        self.registry.supports_extension(path)
    }

    /// Whether a file on disk belongs in folder listings (also extensionless DICOM files).
    pub fn supports_file(&self, path: &Path) -> bool {
        self.registry.supports_file(path)
    }

    pub fn threads(&self) -> usize {
        self.pool.threads()
    }

    pub fn queued(&self) -> usize {
        self.pool.queued()
    }

    /// The decoder registry (the gallery's thumbnail engine opens files through it).
    pub fn registry(&self) -> Arc<Registry> {
        self.registry.clone()
    }

    /// Shared handle to the pool, for the watchdog thread.
    pub fn pool(&self) -> Arc<JobPool> {
        self.pool.clone()
    }

    /// The thumbnail pool: as many workers as the decode pool, at background OS priority
    /// ([`JobPool::background`]), so making thumbnails never takes the CPU from the UI thread
    /// or from the image being viewed.
    pub fn thumb_pool(&self) -> Arc<JobPool> {
        self.thumbs
            .get_or_init(|| Arc::new(JobPool::background(self.pool.threads(), "thumb")))
            .clone()
    }

    /// What the stall watchdog prints: the jobs of both pools.
    pub fn jobs_dump(&self) -> impl Fn() -> Vec<String> + Send + Sync + 'static {
        let (pool, thumbs) = (self.pool.clone(), self.thumbs.clone());
        move || {
            let mut v = pool.dump();
            if let Some(t) = thumbs.get() {
                v.extend(t.dump().into_iter().map(|l| format!("thumbs: {l}")));
            }
            v
        }
    }

    /// Posts an event from any thread (dialogs, trash, clipboard).
    pub fn post(&self, e: Event) {
        self.send(e);
    }

    /// Runs `f` on a plain background thread (blocking work that is not decoding). A panic
    /// in `f` is logged, ends only that thread, and is posted as
    /// [`Event::BackgroundFailed`], so whatever waits for `f`'s reply stops waiting.
    pub fn background(&self, name: &str, f: impl FnOnce(&Loader) + Send + 'static) {
        let me = self.clone();
        let task = name.to_owned();
        let what = format!("background task {name}");
        let _ = specular_thread::builder(name).spawn(move || {
            if let Err(p) = specular_core::isolate::run(what, || f(&me)) {
                me.post(Event::BackgroundFailed {
                    task,
                    error: p.brief(),
                });
            }
        });
    }

    /// The boundary of a job about image `id`: runs `f`, and if it panics (a decoder bug on
    /// this file) the job ends with `Event::Failed`, so the viewer shows an error instead of
    /// waiting, and the worker goes on with its next job. `outcome`: the job's own
    /// [`Outcome`], finished on a panic so the app does not also hear "unfinished" (and ask
    /// again, to panic again).
    fn guarded(
        &self,
        id: ImageId,
        what: &str,
        outcome: &mut Outcome,
        f: impl FnOnce(&mut Outcome),
    ) {
        let r = specular_core::isolate::run(format_args!("img={id} {what}"), || f(outcome));
        if r.is_err() {
            outcome.finish();
            // "Can't open …: something went wrong inside Specular. The log has the details."
            // (the log has the panic, its location and crate).
            self.send(Event::Failed {
                id,
                error: specular_i18n::t!("error-reason-internal"),
            });
        }
    }

    /// Reads the metadata report of an opened image in `lang`.
    /// Reads the image's metadata report. `places`: add the nearest town for GPS positions
    /// (a second report follows once the offline geocoder has answered); `hide_gps`: leave
    /// positions and places out (settings `metadata.reverse_geocoding` / `hide_gps`);
    /// `unit`: a thermal still's temperatures in the viewer's temperature unit.
    pub fn metadata(
        &self,
        id: ImageId,
        session: Arc<dyn Session>,
        lang: specular_i18n::LanguageIdentifier,
        places: bool,
        hide_gps: bool,
        unit: specular_decode::science::TemperatureUnit,
    ) {
        let me = self.clone();
        let lang_for_panic = lang.clone();
        self.pool.submit_labeled(Priority::Near, CancelToken::new(), format!("img={id} metadata"), move |_| {
            // A panic costs the report only (no `Event::Failed`: the image itself is fine); Info
            // then shows a warning instead of staying empty.
            let done = specular_core::isolate::run(format_args!("img={id} metadata"), || {
            let t0 = Instant::now();
            let loc = meta_localizer(lang);
            let (mut report, motion) = metadata_report_in(session.as_ref(), &loc, unit);
            if let Some(m) = motion {
                me.send(Event::Motion { id, info: Arc::new(m) });
            }
            if hide_gps {
                report.strip_location(&loc);
            }
            let geocode = places && !hide_gps && report.gps.is_some();
            if geocode && !specular_metadata::geocoder_ready() {
                // Show everything else now; the place follows.
                me.send(Event::Metadata { id, report: Arc::new(report.clone()) });
            }
            if geocode {
                specular_metadata::attach_place(&mut report, &loc);
            }
            debug!(target: METADATA, "img={id} report: {} summary groups, {} field groups, {} warnings in {:.1} ms", report.summary.len(), report.all_fields.len(), report.warnings.len(), ms(t0));
            me.send(Event::Metadata { id, report: Arc::new(report) });
            });
            if done.is_err() {
                let loc = meta_localizer(lang_for_panic);
                let report = specular_metadata::MetadataReport {
                    warnings: vec![loc.text("warn-metadata-failed")],
                    language: loc.langid().to_string(),
                    ..Default::default()
                };
                me.send(Event::Metadata { id, report: Arc::new(report) });
            }
        });
    }

    /// Reads the motion clip of a still (`orientation`, `display`: the still's stored
    /// orientation and its size as shown) and opens its decoder; sends `Event::MotionClip`.
    pub fn motion_clip(
        &self,
        id: ImageId,
        session: Arc<dyn Session>,
        info: Arc<specular_video::MotionInfo>,
        orientation: specular_core::Orientation,
        display: (u32, u32),
    ) {
        let me = self.clone();
        let t0 = Instant::now();
        self.pool.submit_labeled(Priority::Immediate, CancelToken::new(), format!("img={id} motion clip"), move |_| {
            use specular_video::play::{Clip, PlayError};
            // A panic costs the clip only: the still stays, the clip "cannot play".
            let r = specular_core::isolate::run(format_args!("img={id} motion clip"), || {
            let src = session.source();
            let clip = match &info.source {
                specular_video::Source::Embedded(r) => src
                    .bytes()
                    .get(r.clone())
                    .ok_or_else(|| PlayError::Malformed("clip outside the file".into()))
                    .and_then(|b| Clip::from_bytes(b.to_vec())),
                specular_video::Source::Sibling(_) => Clip::load(src.path(), &info),
            };
            clip.and_then(|c| c.frames(orientation, display)).map(|(frames, info)| MotionClip {
                frames: Box::new(frames),
                info,
            })
            });
            let clip = r.unwrap_or_else(|p| Err(PlayError::Malformed(p.brief())));
            let ms = ms(t0);
            match &clip {
                Ok(c) => debug!(target: DECODE, "img={id} motion clip: {:?} frames, decoder open after {ms:.1} ms", c.info.frame_count),
                Err(e) => debug!(target: DECODE, "img={id} motion clip cannot play: {e}"),
            }
            me.send(Event::MotionClip { id, clip, ms });
        });
    }

    fn send(&self, e: Event) {
        let _ = self.tx.send(e);
        if let Some(w) = self.wake.lock().as_ref() {
            w();
        }
    }

    /// The window's size in pixels, for opens that were submitted without one. Starts the
    /// sharp level of an open that already planned without it.
    pub fn hint_viewport(&self, w: u32, h: u32) {
        if w <= 1 || h <= 1 {
            return;
        }
        let waiting = {
            let mut v = self.viewport.lock();
            v.size = Some((w, h));
            v.waiting.take()
        };
        if let Some(open) = waiting {
            self.start_sharp(open, (w, h));
        }
    }

    fn viewport_hint(&self) -> Option<(u32, u32)> {
        self.viewport.lock().size
    }

    /// An open planned without a viewport: start its sharp level once the window's size is
    /// known (now, if it became known meanwhile).
    fn await_viewport(&self, open: WaitingOpen) {
        let size = {
            let mut v = self.viewport.lock();
            if v.size.is_none() {
                v.waiting = Some(open);
                return;
            }
            v.size
        };
        if let Some(size) = size {
            self.start_sharp(open, size);
        }
    }

    fn start_sharp(&self, open: WaitingOpen, (vw, vh): (u32, u32)) {
        let cap = scale_cap(open.session.info());
        let need = needed_scale(open.size.0, open.size.1, vw, vh, cap);
        if open.token.is_cancelled() || open.plan.iter().any(|&s| s <= need) {
            return;
        }
        debug!(target: DECODE, "img={} level 1/{need} started by the window size {vw}×{vh}", open.id);
        self.viewport
            .lock()
            .late
            .push((open.id, open.token.clone(), need));
        self.decode_level(open.id, open.session, need, open.prio, open.token);
    }

    /// Levels the viewport hint started for the open job that ran with `token`: in flight,
    /// like the levels of the job's plan.
    pub fn take_late_levels(&self, id: ImageId, token: &CancelToken) -> Vec<u32> {
        let mut v = self.viewport.lock();
        let mut out = Vec::new();
        v.late.retain(|(i, t, s)| {
            let mine = *i == id && t.same(token);
            if mine {
                out.push(*s);
            }
            !mine && !t.is_cancelled()
        });
        out
    }

    /// Opens a file: session, embedded thumbnail, then the overview scales of the plan
    /// (decided here when `viewport` is known, else 1/8 only).
    pub fn open(
        &self,
        id: ImageId,
        path: PathBuf,
        prio: Priority,
        viewport: Option<(u32, u32)>,
        token: CancelToken,
    ) -> JobHandle {
        self.open_page(id, path, 0, None, prio, viewport, token)
    }

    /// Opens page `page` of a multi-image file (ICNS size, texture layer, TIFF page) as
    /// image `id`. With `base`, the image and session of another page of the same file, the
    /// file is not read again: the session opens the page from the bytes it holds, and the
    /// display cache keys the page by the base's file version.
    #[allow(clippy::too_many_arguments)]
    pub fn open_page(
        &self,
        id: ImageId,
        path: PathBuf,
        page: u32,
        base: Option<(ImageId, Arc<dyn Session>)>,
        prio: Priority,
        viewport: Option<(u32, u32)>,
        token: CancelToken,
    ) -> JobHandle {
        let me = self.clone();
        let prefetch = prio <= Priority::Prefetch;
        let name = file_name(&path);
        let base = base.map(|(from, s)| {
            if let Some(k) = self.cache_key(from) {
                self.display.keys.lock().insert(id, k.with_page(page));
            }
            s
        });
        let mut outcome = Outcome::new(self, id, &token, Unfinished::Open);
        self.pool.submit_labeled(prio, token.clone(), format!("img={id} {name} open"), move |cancel| {
            let guard = me.clone();
            guard.guarded(id, &format!("{name} open"), &mut outcome, |outcome| {
            let t0 = Instant::now();
            let fail = |outcome: &mut Outcome, e: String| {
                outcome.finish();
                me.send(Event::Failed { id, error: e });
            };
            let (session, read_ms, parse_ms, bytes, in_archive) = if let Some(base) = base {
                // Another page of a file already open: no read, no probing.
                match base.open_page(page) {
                    Ok(s) => (s, 0.0, ms(t0), 0, me.books.get(&path).is_some()),
                    Err(e) => return fail(outcome, open_error(&e)),
                }
            } else {
                // An archive page: its bytes come out of the open archive.
                let book_page = me.books.get(&path);
                let in_archive = book_page.is_some();
                // A page of an encrypted archive is never kept in the display cache.
                let private = book_page.as_ref().is_some_and(|(b, _)| b.encrypted());
                let opened = match book_page {
                    Some((book, p)) => book.source(p),
                    None => specular_core::Source::open(&path).map_err(|e| crate::error_text::io_reason(&e)),
                };
                let src = match opened {
                    Ok(s) => Arc::new(s),
                    Err(e) => {
                        debug!(target: DECODE, "img={id} {name} cannot open: {e}");
                        return fail(outcome, e.to_string());
                    }
                };
                let read_ms = ms(t0);
                debug!(target: DECODE, "img={id} {name} opened: {} bytes in {read_ms:.1} ms", src.len());
                let bytes = src.len() as u64;
                if let Some(k) = Key::new(src.path(), src.len() as u64, src.modified())
                    && !private
                {
                    me.display.keys.lock().insert(id, k.with_page(page));
                }
                let t = Instant::now();
                let session = match me.registry.open(src).and_then(|s| if page > 0 { s.open_page(page) } else { Ok(s) }) {
                    Ok(s) => s,
                    Err(e) => return fail(outcome, open_error(&e)),
                };
                (session, read_ms, ms(t), bytes, in_archive)
            };
            if page > 0 {
                debug!(target: DECODE, "img={id} {name} page {} of {} opened in {:.1} ms", page + 1, session.page_count(), ms(t0));
            }
            let info = session.info().clone();
            let t = Instant::now();
            let lut = info.icc_profile.as_ref().and_then(|p| specular_render::icc::build_lut(p)).map(Arc::new);
            let times = OpenTimes { read_ms, parse_ms, colour_ms: ms(t), bytes, in_archive };
            let viewport = viewport.or_else(|| me.viewport_hint());
            let (dw, dh) = info.display_size();
            let cap = scale_cap(&info);
            // An office document without a stored preview has properties but no pixels: nothing
            // to decode (the viewer says so).
            let no_pixels = session.document().is_some_and(|d| d.preview.is_none());
            let plan = if no_pixels {
                Vec::new()
            } else {
                overview_plan((dw, dh), info.progressive, session.scaled_decode_cheap(), viewport, prefetch, cap)
            };
            if viewport.is_none() && !prefetch && !no_pixels {
                me.await_viewport(WaitingOpen { id, session: session.clone(), size: (dw, dh), plan: plan.clone(), prio, token: token.clone() });
            }
            outcome.finish();
            me.send(Event::Opened { id, session: session.clone(), lut, plan: plan.clone(), ms: ms(t0), at: specular_core::ms_since_start(), token: token.clone(), times });
            if no_pixels {
                return;
            }
            // From here on the plan's levels are in flight (the app records them on `Opened`).
            let (first, rest) = plan.split_first().map(|(f, r)| (*f, r.to_vec())).unwrap_or((8, Vec::new()));
            let mut level_outcome = Outcome::new(&me, id, &token, Unfinished::Level(first));
            debug!(target: DECODE, "img={id} {name} plan: levels {plan:?} (viewport {viewport:?}, prefetch {prefetch})");
            // The plan plus the level a sharp view will ask for (a 4K screen if the viewport
            // is not known yet).
            let (vw, vh) = viewport.unwrap_or((3840, 2160));
            let mut levels = plan.clone();
            let sharp = needed_scale(dw, dh, vw, vh, cap);
            if !levels.contains(&sharp) {
                levels.push(sharp);
            }
            // Decoders that produce several levels in one pass (PNG) decode the plan together,
            // with the full-size level for zooming in when the image is being shown (a sharp
            // level guessed without a viewport is left out: it may never be asked for, and
            // the decoder can derive it from the full-size level).
            let mut wanted = plan.clone();
            if !prefetch {
                wanted.push(1);
            }
            session.want_levels(&wanted);
            // On top of the decoder's own peak, the app holds GPU staging copies and cached
            // rasters: measured growth stayed under 1.5× the estimate plus 64 MiB for 12–650 MP
            // sequential and progressive files.
            let need = session.open_memory(&levels) * 3 / 2 + (64 << 20);
            let available = specular_core::available_memory();
            debug!(target: DECODE, "img={id} {name} needs ~{} MiB, {} MiB available", need >> 20, available.map_or(-1, |a| (a >> 20) as i64));
            if let Some(available) = available
                && need > available
            {
                me.send(Event::MemoryShort { id, need, available });
            }
            if cancel.is_cancelled() {
                // Every planned level reports back.
                for s in rest {
                    drop(Outcome::new(&me, id, &token, Unfinished::Level(s)));
                }
                return;
            }
            match session.decode_preview() {
                Some(Ok(r)) => {
                    // Only previews with the image's aspect ratio (some cameras letterbox).
                    let a = info.width as f32 / info.height as f32;
                    let b = r.width as f32 / r.height.max(1) as f32;
                    if (a / b - 1.0).abs() < 0.04 {
                        debug!(target: DECODE, "img={id} {name} embedded preview {}×{}", r.width, r.height);
                        me.send(Event::Overview { id, kind: OverviewKind::Thumbnail, raster: Arc::new(r), ms: ms(t0), at: specular_core::ms_since_start(), stage: None });
                    } else {
                        debug!(target: DECODE, "img={id} {name} embedded preview {}×{} has another shape; not used", r.width, r.height);
                    }
                }
                Some(Err(e)) => debug!(target: DECODE, "img={id} {name} embedded preview: {e}"),
                None => {}
            }
            // Later (sharper) levels run in parallel with the first.
            for s in rest {
                me.decode_level(id, session.clone(), s, prio, cancel.clone());
            }
            me.run_level(id, &session, first, cancel, t0, &mut level_outcome);
            // The gain map after the first level, so the SDR image is never held up by it.
            if session.has_gain_map() && !cancel.is_cancelled() {
                let t = Instant::now();
                if let Some(map) = session.gain_map(cancel) {
                    debug!(target: DECODE, "img={id} {name} gain map {}×{} in {:.1} ms", map.map.width, map.map.height, ms(t));
                    me.send(Event::GainMap { id, map: Arc::new(map) });
                }
            }
            })
        })
    }

    fn run_level(
        &self,
        id: ImageId,
        session: &Arc<dyn Session>,
        scale: u32,
        cancel: &CancelToken,
        t0: Instant,
        outcome: &mut Outcome,
    ) {
        if cancel.is_cancelled() {
            return;
        }
        let req = DecodeRequest {
            scale,
            region: None,
            parallel: true,
            prepare_regions: true,
        };
        let t = Instant::now();
        let key = (scale > 1).then(|| self.cache_key(id)).flatten();
        if let Some(key) = &key
            && let Some(cache) = self.display_cache_for(key, scale)
            && let Some(r) = cache.get(key, scale)
        {
            let took = ms(t);
            debug!(target: DECODE, "img={id} level 1/{scale}: {}×{} from the display cache in {took:.1} ms", r.width, r.height);
            outcome.finish();
            self.send(Event::Overview {
                id,
                kind: OverviewKind::Level(scale),
                raster: Arc::new(r),
                ms: ms(t0),
                at: specular_core::ms_since_start(),
                stage: Some(DecodeStage {
                    scale,
                    ms: took,
                    from_cache: true,
                    ..DecodeStage::default()
                }),
            });
            return;
        }
        // What the decode cost besides its wall time: the path the selector took (a note the
        // decoder leaves on this thread) and the file pages read meanwhile (major faults).
        let _ = specular_core::paths::take_note();
        let usage = specular_core::paths::thread_usage();
        let probed = specular_hwdec::probed().is_some();
        let t = Instant::now();
        match session.decode(&req, cancel) {
            Ok(r) => {
                let took = ms(t);
                let stage = decode_stage(scale, took, usage, probed);
                debug!(target: DECODE, "img={id} level 1/{scale} decoded: {}×{} in {took:.1} ms", r.width, r.height);
                outcome.finish();
                let r = Arc::new(r);
                let store = key
                    .filter(|_| took >= CACHE_MIN_DECODE_MS)
                    .and_then(|k| Some((self.display_cache_now()?, k)));
                self.send(Event::Overview {
                    id,
                    kind: OverviewKind::Level(scale),
                    raster: r.clone(),
                    ms: ms(t0),
                    at: specular_core::ms_since_start(),
                    stage: Some(stage),
                });
                if let Some((cache, key)) = store {
                    cache.put(&key, r);
                }
            }
            Err(specular_decode::DecodeError::Cancelled) => {
                debug!(target: DECODE, "img={id} level 1/{scale} cancelled after {:.1} ms", ms(t))
            }
            Err(e) => {
                debug!(target: DECODE, "img={id} level 1/{scale} failed: {e}");
                outcome.finish();
                self.send(Event::Failed {
                    id,
                    error: open_error(&e),
                })
            }
        }
    }

    /// Decodes the whole image at 1/scale.
    pub fn decode_level(
        &self,
        id: ImageId,
        session: Arc<dyn Session>,
        scale: u32,
        prio: Priority,
        token: CancelToken,
    ) -> JobHandle {
        let me = self.clone();
        let mut outcome = Outcome::new(self, id, &token, Unfinished::Level(scale));
        self.pool.submit_labeled(
            prio,
            token,
            format!("img={id} level 1/{scale}"),
            move |cancel| {
                let t0 = Instant::now();
                me.guarded(id, &format!("level 1/{scale}"), &mut outcome, |outcome| {
                    me.run_level(id, &session, scale, cancel, t0, outcome)
                });
            },
        )
    }

    /// Decodes a region of `level` covering `tiles` (rect in level pixels).
    pub fn decode_tiles(
        &self,
        id: ImageId,
        session: Arc<dyn Session>,
        level: u8,
        rect: (u32, u32, u32, u32),
        tiles: Vec<(u32, u32)>,
    ) -> JobHandle {
        let me = self.clone();
        let label = format!(
            "img={id} tiles L{level} ×{} {}×{}+{}+{}",
            tiles.len(),
            rect.2,
            rect.3,
            rect.0,
            rect.1
        );
        let what = label.clone();
        self.pool.submit_labeled(Priority::Immediate, CancelToken::new(), label, move |cancel| {
            // A panic fails these tiles like a decode error (the app may ask again later).
            let failed = tiles.clone();
            let r = specular_core::isolate::run(&what, || {
            let t0 = Instant::now();
            let scale = 1u32 << level;
            // Every JPEG goes through the row index: sequential files index during the
            // first level decode (or in ~8 ms with restart markers), progressive ones with a
            // low-memory coefficient pass (~20 ms for 34 MP). Jobs that arrive while it is
            // being built wait on the session's lock.
            if !session.regions_ready() && session.prepare_regions(cancel).is_err() {
                me.send(Event::TilesFailed { id, level, tiles });
                return;
            }
            let req = DecodeRequest { scale, region: Some(Rect { x: rect.0, y: rect.1, w: rect.2, h: rect.3 }), parallel: true, prepare_regions: false };
            match session.decode(&req, cancel) {
                Ok(raster) => {
                    trace!(target: TILES, "img={id} L{level} {} tiles decoded in {:.1} ms", tiles.len(), ms(t0));
                    me.send(Event::Tiles { id, level, tiles, raster, ms: ms(t0) })
                }
                Err(e) => {
                    debug!(target: TILES, "img={id} L{level} {} tiles failed: {e}", tiles.len());
                    me.send(Event::TilesFailed { id, level, tiles })
                }
            }
            });
            if r.is_err() {
                me.send(Event::TilesFailed { id, level, tiles: failed });
            }
        })
    }

    /// Lists the images in `dir` for a slideshow on another display, in the background;
    /// `Event::ShowListing` follows.
    pub fn scan_show(&self, dir: PathBuf, opts: ListOptions) {
        let me = self.clone();
        specular_thread::builder("io-scan-show")
            .spawn(move || {
                let t0 = Instant::now();
                // Sniffing is caught per file; anything else that panics lists nothing,
                // rather than leaving the show waiting.
                let what = format_args!("listing {}", dir.display());
                let files = specular_core::isolate::run(what, || list_folder(&dir, &me.registry, opts)).unwrap_or_default();
                debug!(target: FOLDER, "show scan {}: {} images in {:.1} ms ({opts:?})", dir.display(), files.len(), ms(t0));
                me.send(Event::ShowListing { dir, files });
            })
            .ok();
    }

    /// Lists the images in `dir` in the given order, in the background.
    pub fn scan_folder(&self, dir: PathBuf, opts: ListOptions) {
        let me = self.clone();
        specular_thread::builder("io-scan-dir")
            .spawn(move || {
                let t0 = Instant::now();
                // Sniffing is caught per file; anything else that panics lists nothing,
                // rather than leaving the viewer without its folder.
                let what = format_args!("listing {}", dir.display());
                let files = specular_core::isolate::run(what, || list_folder(&dir, &me.registry, opts)).unwrap_or_default();
                debug!(target: FOLDER, "scan {}: {} images in {:.1} ms ({opts:?})", dir.display(), files.len(), ms(t0));
                me.send(Event::Folder { dir, files });
            })
            .ok();
    }
}

/// How a folder is listed (from `[browsing]` settings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListOptions {
    pub order: SortOrder,
    pub descending: bool,
    /// Also list images in subfolders (`browsing.include_subfolders`).
    pub recursive: bool,
}

/// Subfolder levels followed with `recursive`, and the most files listed: a guard against
/// opening a whole disk.
const MAX_DEPTH: usize = 16;
const MAX_FILES: usize = 200_000;

/// The images in `dir`, sorted. Ties (and files without the sort key) fall back to
/// natural order of the path below `dir`, so with subfolders each folder's images stay
/// together; `descending` reverses the whole list. Hidden entries (".name") are skipped in
/// subfolders, and symbolic links to folders are not followed (no cycles).
pub fn list_folder(dir: &Path, registry: &Registry, opts: ListOptions) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect(dir, registry, opts.recursive, 0, &mut paths);
    let order = opts.order;
    let files: Vec<FolderEntry> = paths
        .into_iter()
        .map(|p| FolderEntry::new(dir, p, order))
        .collect();
    let loc = specular_i18n::I18n::global().localizer();
    let mut files = sort_entries(&loc, files, order);
    if opts.descending {
        files.reverse();
    }
    files.into_iter().map(|f| f.path).collect()
}

/// `files` in `order`, ties in natural order of their paths. Natural keys are made once per
/// name, so the sort compares bytes instead of collating text at every comparison.
fn sort_entries(
    loc: &specular_i18n::Localizer,
    files: Vec<FolderEntry>,
    order: SortOrder,
) -> Vec<FolderEntry> {
    let mut keyer = loc.natural_keyer();
    let mut keyed: Vec<(FolderEntry, Vec<NaturalKey>, NaturalKey)> = files
        .into_iter()
        .map(|f| {
            let parts = f.name.split('/').map(|p| keyer.key(p)).collect();
            let ext = if order == SortOrder::Type {
                keyer.key(&f.ext)
            } else {
                NaturalKey::default()
            };
            (f, parts, ext)
        })
        .collect();
    drop(keyer);
    keyed.sort_by(|(a, pa, ea), (b, pb, eb)| {
        let primary = match order {
            SortOrder::Natural => std::cmp::Ordering::Equal,
            SortOrder::Modified => a.modified.cmp(&b.modified),
            SortOrder::Size => a.size.cmp(&b.size),
            SortOrder::Type => loc.natural_cmp_keyed((ea, &a.ext), (eb, &b.ext)),
            SortOrder::DateTaken => a.taken.cmp(&b.taken),
        };
        primary.then_with(|| natural_path_cmp_keyed(loc, (pa, &a.name), (pb, &b.name)))
    });
    keyed.into_iter().map(|(f, _, _)| f).collect()
}

use specular_i18n::NaturalKey;

/// [`natural_path_cmp`] with each path component's natural key.
fn natural_path_cmp_keyed(
    loc: &specular_i18n::Localizer,
    (ka, a): (&[NaturalKey], &str),
    (kb, b): (&[NaturalKey], &str),
) -> std::cmp::Ordering {
    let (mut ia, mut ib) = (a.split('/').zip(ka), b.split('/').zip(kb));
    loop {
        match (ia.next(), ib.next()) {
            (Some((x, kx)), Some((y, ky))) => {
                // A file sorts before a folder at the same level.
                let (xf, yf) = (ia.clone().next().is_none(), ib.clone().next().is_none());
                let o = yf
                    .cmp(&xf)
                    .then_with(|| loc.natural_cmp_keyed((kx, x), (ky, y)));
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
        }
    }
}

fn collect(dir: &Path, registry: &Registry, recursive: bool, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut subdirs = Vec::new();
    // Every file's name, for what the folder says of files whose kind is unclear
    // (`ident::folder_hints`): handed over so opening one never lists the folder again.
    let mut names = Vec::new();
    for e in rd.filter_map(|e| e.ok()) {
        if out.len() >= MAX_FILES {
            return;
        }
        let Ok(ft) = e.file_type() else { continue };
        let path = e.path();
        if !ft.is_dir() {
            names.push(PathBuf::from(e.file_name()));
        }
        if ft.is_dir() && specular_core::is_image_folder(&path) {
            // An OME-Zarr store, a Relight or a CASA image is one image, never a folder to
            // descend into.
            if registry.supports_entry(&path, Some(true)) {
                out.push(path);
            }
        } else if ft.is_dir() {
            if recursive && depth < MAX_DEPTH && !e.file_name().to_string_lossy().starts_with('.') {
                subdirs.push(path);
            }
        } else if (ft.is_file() || ft.is_symlink())
            && registry.supports_entry(&path, ft.is_file().then_some(false))
        {
            out.push(path);
        }
    }
    specular_decode::ident::set_listing(dir, names.iter().map(PathBuf::as_path));
    for d in subdirs {
        collect(&d, registry, recursive, depth + 1, out);
    }
}

/// Natural order of relative paths, folder by folder ("a/2.jpg" < "a/10.jpg" < "b/1.jpg"),
/// with a folder's own files before its subfolders' files. Collates at every comparison: the
/// reference [`sort_entries`] is tested against.
#[cfg(test)]
fn natural_path_cmp(i18n: &specular_i18n::I18n, a: &str, b: &str) -> std::cmp::Ordering {
    let (mut ia, mut ib) = (a.split('/'), b.split('/'));
    loop {
        match (ia.next(), ib.next()) {
            (Some(x), Some(y)) => {
                // A file sorts before a folder at the same level.
                let (xf, yf) = (ia.clone().next().is_none(), ib.clone().next().is_none());
                let o = yf.cmp(&xf).then_with(|| i18n.natural_cmp(x, y));
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
        }
    }
}

struct FolderEntry {
    path: PathBuf,
    name: String,
    ext: String,
    modified: Option<std::time::SystemTime>,
    size: u64,
    /// EXIF capture time, else the modification time, as "YYYY:MM:DD HH:MM:SS".
    taken: String,
}

impl FolderEntry {
    fn new(root: &Path, path: PathBuf, order: SortOrder) -> FolderEntry {
        // The path below the listed folder, with "/" between components.
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let name = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let meta = match order {
            SortOrder::Natural | SortOrder::Type => None,
            _ => std::fs::metadata(&path).ok(),
        };
        let modified = meta.as_ref().and_then(|m| m.modified().ok());
        let size = meta.as_ref().map_or(0, |m| m.len());
        let taken = if order == SortOrder::DateTaken {
            date_taken(&path)
                .or_else(|| modified.map(stamp))
                .unwrap_or_default()
        } else {
            String::new()
        };
        FolderEntry {
            path,
            name,
            ext,
            modified,
            size,
            taken,
        }
    }
}

/// EXIF date taken from the first 128 KiB of a JPEG (APP1 is at most 64 KiB and comes
/// first, after an optional APP0).
fn date_taken(path: &Path) -> Option<String> {
    use std::io::{Read, Seek};
    let mut f = open_regular(path)?;
    let mut sig = [0u8; 2];
    f.read_exact(&mut sig).ok()?;
    f.rewind().ok()?;
    if sig == [0x89, b'P'] {
        // PNG: eXIf, else tEXt "Creation Time" (None: the caller falls back to mtime).
        return specular_decode::png_date_taken(&mut std::io::BufReader::new(f));
    }
    let mut head = Vec::with_capacity(128 << 10);
    f.take(128 << 10).read_to_end(&mut head).ok()?;
    specular_decode::jpeg_exif(&head).and_then(specular_decode::exif_date_taken)
}

/// Opens `path` only when it is a regular file: opening a FIFO without a writer blocks, and
/// these reads happen before the window exists (the loader's `Source::open` reports the error).
fn open_regular(path: &Path) -> Option<std::fs::File> {
    if !std::fs::metadata(path).ok()?.is_file() {
        specular_log::debug!(target: specular_log::IO, "{}: not a regular file; not reading its header", path.display());
        return None;
    }
    std::fs::File::open(path).ok()
}

/// Displayed size of an image from its header alone (JPEG SOF plus EXIF orientation, PNG
/// IHDR), read with a few small reads: the window is created at the image's shape before
/// any decoding has finished.
pub fn header_display_size(path: &Path) -> Option<(u32, u32)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::io::BufReader::with_capacity(16 << 10, open_regular(path)?);
    let mut sig = [0u8; 2];
    f.read_exact(&mut sig).ok()?;
    if sig == [0x89, b'P'] {
        let mut h = [0u8; 22];
        f.read_exact(&mut h).ok()?;
        let be = |o: usize| u32::from_be_bytes([h[o], h[o + 1], h[o + 2], h[o + 3]]);
        return (&h[10..14] == b"IHDR").then(|| (be(14), be(18)));
    }
    if sig != [0xFF, 0xD8] {
        return None;
    }
    let mut swap = false;
    for _ in 0..256 {
        let mut m = [0u8; 4];
        f.read_exact(&mut m[..2]).ok()?;
        while m[1] == 0xFF {
            f.read_exact(&mut m[1..2]).ok()?;
        }
        if m[0] != 0xFF {
            return None;
        }
        f.read_exact(&mut m[2..4]).ok()?;
        let len = u16::from_be_bytes([m[2], m[3]]) as usize;
        let body = len.checked_sub(2)?;
        match m[1] {
            0xC0..=0xCF if !matches!(m[1], 0xC4 | 0xC8 | 0xCC) => {
                let mut sof = [0u8; 5];
                f.read_exact(&mut sof).ok()?;
                let (h, w) = (
                    u16::from_be_bytes([sof[1], sof[2]]) as u32,
                    u16::from_be_bytes([sof[3], sof[4]]) as u32,
                );
                return (w > 0 && h > 0).then_some(if swap { (h, w) } else { (w, h) });
            }
            0xE1 => {
                let mut seg = vec![0u8; body];
                f.read_exact(&mut seg).ok()?;
                if let Some(tiff) = seg.strip_prefix(b"Exif\0\0") {
                    swap = specular_decode::exif_orientation(tiff).is_some_and(|o| o >= 5);
                }
            }
            0xDA | 0xD9 => return None,
            _ => {
                f.seek(SeekFrom::Current(body as i64)).ok()?;
            }
        }
    }
    None
}

/// A UTC timestamp in EXIF's "YYYY:MM:DD HH:MM:SS" form.
fn stamp(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!(
        "{y:04}:{m:02}:{d:02} {:02}:{:02}:{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// The info panel's facts of a point cloud: those of the points read (the classes are
/// counted while reading, so this waits for the loading, on the metadata worker), or the
/// header's when loading failed or was cancelled.
pub(crate) fn points_facts(p: &specular_decode::PointsModel) -> specular_metadata::PointsFacts {
    let cloud = p.loading.wait().ok();
    let i = cloud.as_ref().map_or(&p.loading.info, |c| &c.info);
    let mut format = i.format.map_or("", |f| f.name()).to_owned();
    if let Some(v) = &i.version {
        format = format!("{format} {v}");
    }
    if i.copc {
        format.push_str(" (COPC)");
    }
    let size = i
        .extent
        .map(|e| [e[3] - e[0], e[4] - e[1], e[5] - e[2]])
        .or_else(|| {
            let b = cloud.as_ref().map_or(&p.loading.bounds, |c| &c.bounds);
            (!b.is_empty()).then(|| b.size().map(f64::from))
        });
    let ch = &i.channels;
    let channels = [
        (ch.rgb, "rgb"),
        (ch.intensity, "intensity"),
        (ch.classification, "classification"),
        (ch.returns, "returns"),
    ]
    .into_iter()
    .filter_map(|(on, id)| on.then_some(id))
    .collect();
    specular_metadata::PointsFacts {
        format,
        point_format: i.point_format,
        coding: i.coding.clone(),
        total: i.total,
        loaded: i.loaded,
        size,
        crs: i.crs.clone(),
        channels,
        classes: i.classes.clone(),
        scans: i.scans.iter().map(|s| (s.name.clone(), s.points)).collect(),
    }
}

/// The info panel's facts of Gaussian splats.
pub(crate) fn splat_facts(m: &specular_decode::SplatModel) -> specular_metadata::SplatFacts {
    let s = &m.splats;
    specular_metadata::SplatFacts {
        format: s.format.name().to_owned(),
        total: s.in_file,
        shown: s.len() as u64,
        sh_degree: s.sh_degree,
        size: (!s.bounds.is_empty()).then(|| s.bounds.size().map(f64::from)),
        up: Some(s.up.label().to_owned()),
        antialiased: s.antialiased,
    }
}

/// The info panel's facts of a reflectance transformation image.
pub(crate) fn rti_facts(r: &specular_decode::specular_rti::Rti) -> specular_metadata::RtiFacts {
    use specular_decode::specular_rti::Container;
    let (format, compression, scale, bias) = match &r.container {
        Container::Ptm {
            version,
            format,
            scale,
            bias,
            quality,
        } => {
            let jpeg = format.contains("JPEG");
            (
                format!("{version} ({})", format.trim_start_matches("PTM_FORMAT_")),
                jpeg.then(|| match quality {
                    Some(q) => format!("JPEG, quality {q}"),
                    None => "JPEG".to_owned(),
                }),
                scale.to_vec(),
                bias.iter().map(|&b| b as f32).collect(),
            )
        }
        Container::Hsh { scale, bias, .. } => (
            r.container.name().to_owned(),
            None,
            scale.clone(),
            bias.clone(),
        ),
        Container::Relight { quality, .. } => (
            r.container.name().to_owned(),
            Some(match quality {
                Some(q) => format!("JPEG, quality {q}"),
                None => "JPEG".to_owned(),
            }),
            Vec::new(),
            Vec::new(),
        ),
    };
    specular_metadata::RtiFacts {
        format,
        basis: r.basis.name().to_owned(),
        order: r.basis.order().map(|o| o as u32),
        terms: r.basis.terms() as u32,
        colour: r.colour.name().to_owned(),
        planes: r.planes as u32,
        width: r.width,
        height: r.height,
        compression,
        scale,
        bias,
    }
}

/// The info panel's facts of a molecular structure.
/// The info panel's font facts of the face shown.
pub(crate) fn font_facts(m: &specular_decode::FontModel) -> specular_metadata::FontFacts {
    use specular_decode::specular_font::{OutlineKind, Permission};
    let f = m.face();
    let n = &f.names;
    let text = |a: &str, b: &str| {
        let v = if a.trim().is_empty() { b } else { a };
        Some(v.trim().to_owned()).filter(|v| !v.is_empty())
    };
    let c = &f.colour;
    let mut colour = Vec::new();
    match c.colr {
        Some((0, _)) => colour.push("colr0"),
        Some(_) => colour.push("colr1"),
        None => {}
    }
    if !c.sbix.is_empty() {
        colour.push("sbix");
    }
    if !c.cbdt.is_empty() {
        colour.push("cbdt");
    }
    if c.svg {
        colour.push("svg");
    }
    specular_metadata::FontFacts {
        family: text(&n.typographic_family, &n.family),
        style: text(&n.typographic_subfamily, &n.subfamily),
        version: text(&n.version, ""),
        designer: text(&n.designer, ""),
        manufacturer: text(&n.manufacturer, ""),
        licence: text(&n.licence, ""),
        licence_url: text(&n.licence_url, ""),
        copyright: text(&n.copyright, ""),
        trademark: text(&n.trademark, ""),
        face: Some((m.face as u64, m.file.faces.len() as u64)),
        glyphs: u64::from(f.glyph_count),
        outlines: match f.outlines {
            OutlineKind::TrueType => "truetype",
            OutlineKind::Cff => "cff",
            OutlineKind::Cff2 => "cff2",
            OutlineKind::BitmapOnly => "bitmap",
            OutlineKind::None => "none",
        },
        embedding: f.embedding.map(|e| match e.permission {
            Permission::Installable => "installable",
            Permission::Restricted => "restricted",
            Permission::PreviewPrint => "preview-print",
            Permission::Editable => "editable",
        }),
        no_subsetting: f.embedding.is_some_and(|e| e.no_subsetting),
        bitmap_embedding_only: f.embedding.is_some_and(|e| e.bitmap_only),
        axes: f
            .axes
            .iter()
            .filter(|a| !a.hidden)
            .map(|a| {
                (
                    a.tag_str(),
                    a.name.clone(),
                    f64::from(a.min),
                    f64::from(a.default),
                    f64::from(a.max),
                )
            })
            .collect(),
        instances: f.instances.len() as u64,
        colour,
        tables: f
            .tables
            .iter()
            .map(|t| String::from_utf8_lossy(&t.tag).trim_end().to_owned())
            .collect(),
        units_per_em: u64::from(f.metrics.units_per_em),
        weight: Some(u64::from(f.weight)).filter(|w| *w > 0),
    }
}

/// The info panel's book facts.
pub(crate) fn document_facts(
    d: &specular_decode::DocumentFacts,
) -> specular_metadata::DocumentFacts {
    let p = &d.props;
    specular_metadata::DocumentFacts {
        kind: d.kind.to_owned(),
        legacy: d.legacy,
        template: d.template,
        macros: d.macros,
        preview: d.preview.map(str::to_owned),
        title: p.title.clone(),
        subject: p.subject.clone(),
        author: p.author.clone(),
        keywords: p.keywords.clone(),
        description: p.description.clone(),
        category: p.category.clone(),
        last_modified_by: p.last_modified_by.clone(),
        revision: p.revision.clone(),
        created: p.created.clone(),
        modified: p.modified.clone(),
        printed: p.printed.clone(),
        application: p.application.clone(),
        company: p.company.clone(),
        template_name: p.template.clone(),
        pages: p.pages,
        words: p.words,
        characters: p.characters,
        paragraphs: p.paragraphs,
        lines: p.lines,
        slides: p.slides,
        notes: p.notes,
        hidden_slides: p.hidden_slides,
        sheets: p.sheets,
        edit_minutes: p.edit_minutes,
        items: d.items,
    }
}

pub(crate) fn book_facts(b: &specular_decode::BookModel) -> specular_metadata::BookFacts {
    specular_metadata::BookFacts {
        title: b.title.clone(),
        authors: b.authors.clone(),
        publisher: b.publisher.clone(),
        language: b.language.clone(),
        date: b.date.clone(),
        identifier: b.identifier.clone(),
        subjects: b.subjects.clone(),
        rights: b.rights.clone(),
        version: b.version.clone(),
        fixed_layout: b.fixed_layout,
        right_to_left: b.right_to_left,
        contents: b.contents as u64,
        pages: b.pages as u64,
        fonts: b.fonts as u64,
        obfuscated_fonts: b.obfuscated_fonts as u64,
    }
}

pub(crate) fn structure_facts(
    m: &specular_decode::MoleculeModel,
) -> specular_metadata::StructureFacts {
    use specular_decode::specular_mol::{MolFormat, SsSource};
    let m = &*m.molecule;
    let i = &m.info;
    let polymer = matches!(m.format, Some(MolFormat::Pdb | MolFormat::Mmcif));
    specular_metadata::StructureFacts {
        id: i.id.clone(),
        title: i.title.clone(),
        classification: i.classification.clone(),
        method: i.method.clone(),
        resolution: i.resolution,
        date: i.date.clone(),
        formula: i.formula.clone(),
        chains: if polymer { m.chains.len() as u64 } else { 0 },
        residues: m.polymer_residues() as u64,
        atoms: m.atoms.len() as u64,
        asymmetric_atoms: m.asymmetric_atoms as u64,
        bonds: m.bonds.len() as u64,
        models: m.models as u64,
        records: m.records as u64,
        ligands: if polymer { m.ligands() } else { Vec::new() },
        // A placeholder cell (CRYST1 1 1 1 of NMR and EM entries) says nothing.
        cell: m
            .cell
            .filter(|c| c.is_valid() && !c.is_placeholder())
            .map(|c| [c.a, c.b, c.c, c.alpha, c.beta, c.gamma]),
        space_group: i.space_group.clone(),
        secondary: match m.ss_source {
            SsSource::File => Some(specular_metadata::StructureSecondary::File),
            SsSource::Computed => Some(specular_metadata::StructureSecondary::Computed),
            SsSource::None => None,
        },
    }
}

/// Why a file did not open, in the user's language where the reason is one they can act
/// on (a GIFTI or CIFTI file that needs a surface beside it).
fn open_error(e: &specular_decode::DecodeError) -> String {
    match specular_decode::neuro_dec::needs_surface(e) {
        Some(n) => specular_i18n::t!("error-needs-surface", vertices = n as u64),
        None => crate::error_text::decode_reason(e),
    }
}

/// The info panel's facts of a GIFTI, CIFTI or tractogram file.
pub(crate) fn neuro_facts(
    n: &specular_decode::neuro_dec::NeuroFacts,
) -> specular_metadata::NeuroFacts {
    let gifti = n.gifti.as_ref().map(|g| specular_metadata::GiftiFacts {
        version: g.version.clone(),
        arrays: g.arrays as u64,
        role: g.role.to_owned(),
        vertices: g.vertices.map(|v| v as u64),
        triangles: g.triangles.map(|v| v as u64),
        structure: g.structure.clone(),
        geometry: g.geometry.clone(),
        array: g.array.clone(),
        intent: g.intent.clone(),
        data_type: g.data_type.map(str::to_owned),
        encoding: g.encoding.to_owned(),
        labels: g.labels as u64,
        transform: g.transform.clone(),
        surface: g.surface.clone(),
    });
    let cifti = n.cifti.as_ref().map(|c| specular_metadata::CiftiFacts {
        version: c.version.clone(),
        intent: c.intent.to_owned(),
        shape: c.shape.map(|x| x as u64),
        map_kind: c.map_kind.to_owned(),
        maps: c.maps as u64,
        series: c.series.clone(),
        models: c.models as u64,
        parcels: c.parcels.map(|p| p as u64),
        voxels: c.voxels as u64,
        structures: c.structures.clone(),
        surfaces: c
            .surfaces
            .iter()
            .map(|(s, v, f)| (s.clone(), *v as u64, f.clone()))
            .collect(),
        shown: c.shown.to_owned(),
        maps_left_out: c.maps_left_out as u64,
        dense: c.dense.map(|(s, r)| (s as u64, r as u64)),
    });
    let tracts = n.tracts.as_ref().map(|t| specular_metadata::TractFacts {
        format: t.format.clone(),
        streamlines: t.streamlines as u64,
        points: t.points as u64,
        shown_streamlines: t.shown_streamlines as u64,
        scalars: t.scalars.clone(),
        properties: t.properties.clone(),
        dims: t.dims.map(|d| d.map(i64::from)),
        voxel_size: t.voxel_size.map(|v| v.map(f64::from)),
        voxel_order: t.voxel_order.clone(),
        space: t.space.to_owned(),
        step: t.step.clone(),
        mean_length: t.mean_length,
    });
    specular_metadata::NeuroFacts {
        gifti,
        cifti,
        tracts,
    }
}

/// The info panel's facts of a 3D model.
pub(crate) fn mesh_facts(
    m: &specular_decode::MeshModel,
    format: &str,
) -> specular_metadata::MeshFacts {
    use specular_decode::specular_mesh::{ImageData, Up};
    let mesh = &m.mesh;
    let missing = mesh
        .images
        .iter()
        .enumerate()
        .filter(|(i, d)| {
            matches!(d, ImageData::Missing { .. })
                || m.textures.get(*i).is_some_and(Option::is_none)
        })
        .count();
    specular_metadata::MeshFacts {
        format: format.to_owned(),
        triangles: mesh.triangle_count() as u64,
        vertices: mesh.vertex_count() as u64,
        size: mesh.bounds.size().map(f64::from),
        unit: mesh.unit.symbol().map(str::to_owned),
        up: Some(match mesh.up {
            Up::Z => "Z".to_owned(),
            Up::Y => "Y".to_owned(),
        }),
        materials: mesh
            .materials
            .iter()
            .map(|x| x.name.trim().to_owned())
            .filter(|n| !n.is_empty())
            .collect(),
        textures: mesh.images.len(),
        textures_missing: missing,
        title: mesh.title.clone(),
        generator: mesh.generator.clone(),
    }
}

/// A web tile set's credit line and its "Tile set" fields.
fn tile_meta_facts(
    f: &mut specular_metadata::GeoFacts,
    m: &specular_decode::geo::specular_gis::tiles::TileMeta,
) {
    f.attribution = m.attribution.clone().filter(|a| !a.trim().is_empty());
    let mut rows = vec![
        ("Container".to_owned(), m.container.to_owned()),
        ("Tiles".to_owned(), m.format.name().to_owned()),
        (
            "Zoom levels".to_owned(),
            format!("{}–{}", m.min_zoom, m.max_zoom),
        ),
        (
            "Bounds".to_owned(),
            format!(
                "{:.5}, {:.5}, {:.5}, {:.5}",
                m.bounds[0], m.bounds[1], m.bounds[2], m.bounds[3]
            ),
        ),
    ];
    if let Some(n) = &m.name {
        rows.push(("Name".to_owned(), n.clone()));
    }
    if let Some(d) = &m.description {
        rows.push(("Description".to_owned(), d.clone()));
    }
    if let Some(n) = m.tiles {
        rows.push(("Tile count".to_owned(), n.to_string()));
    }
    if !m.vector_layers.is_empty() {
        rows.push(("Vector layers".to_owned(), m.vector_layers.join(", ")));
    }
    rows.extend(m.extra.iter().cloned());
    f.fields.push(("tileset", rows));
}

/// The info panel's facts of a georeferenced image or a vector map: its CRS, bounds (in
/// longitude and latitude too), overviews, layers, and the GeoKeys, GDAL metadata and CRS
/// definition as "all fields".
pub(crate) fn geo_facts(g: &specular_decode::geo::GeoRef) -> specular_metadata::GeoFacts {
    use specular_decode::geo::GeoFacts as F;
    use specular_decode::geo::specular_gis::crs::CrsKind;
    let crs = g.crs.as_ref();
    let geographic = crs.is_some_and(|c| c.kind == CrsKind::Geographic);
    let mut f = specular_metadata::GeoFacts {
        crs: crs.map(|c| c.name.clone()).filter(|n| !n.trim().is_empty()),
        epsg: crs.and_then(|c| c.epsg),
        crs_note: g.crs_note.clone(),
        datum: crs.and_then(|c| c.datum.clone()),
        projection: crs.and_then(|c| {
            c.projection
                .as_ref()
                .map(|p| p.method.name().to_owned())
                .or_else(|| c.method_name.clone())
                .filter(|m| !m.is_empty())
        }),
        projection_unsupported: crs
            .is_some_and(|c| c.kind == CrsKind::Projected && c.projection.is_none()),
        to_wgs84: crs.filter(|c| c.kind != CrsKind::Other).map(|c| {
            use specular_decode::geo::specular_gis::datum::ToWgs84;
            match &c.datum_shift {
                ToWgs84::Same => "same",
                ToWgs84::One(h) if h.is_null() => "same",
                ToWgs84::One(_) => "helmert",
                ToWgs84::Choose(s) if s.len() == 1 && s[0].helmert.is_null() => "same",
                ToWgs84::Choose(s) if s.len() == 1 => "helmert",
                ToWgs84::Choose(_) => "by-area",
                ToWgs84::Unknown => "unknown",
            }
        }),
        unit: crs
            .filter(|c| c.kind == CrsKind::Projected)
            .and_then(|c| c.unit.as_ref())
            .map(|u| u.name.clone()),
        bounds: Some(g.bounds),
        geographic,
        ..Default::default()
    };
    // The bounds in longitude and latitude: the corners and edge midpoints of the map
    // bounds, transformed.
    if let Some(c) = crs.filter(|c| c.kind == CrsKind::Projected) {
        let b = g.bounds;
        let mut ll = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for i in 0..=4 {
            for j in 0..=4 {
                let x = b[0] + (b[2] - b[0]) * f64::from(i) / 4.0;
                let y = b[1] + (b[3] - b[1]) * f64::from(j) / 4.0;
                if let Some((lon, lat)) = c.to_wgs84(x, y).or_else(|| c.to_lonlat(x, y)) {
                    ll = [
                        ll[0].min(lon),
                        ll[1].min(lat),
                        ll[2].max(lon),
                        ll[3].max(lat),
                    ];
                }
            }
        }
        f.lonlat = ll.iter().all(|v| v.is_finite()).then_some(ll);
    }
    if let Some(c) = crs {
        if let Some(w) = &c.wkt {
            f.fields.push(("wkt", vec![("WKT".to_owned(), w.clone())]));
        }
        if let Some(j) = &c.projjson {
            f.fields
                .push(("projjson", vec![("PROJJSON".to_owned(), j.clone())]));
        }
    }
    match &g.facts {
        F::Raster(r) => {
            f.pixel_size = Some(g.to_map.pixel_size());
            f.levels = r.levels.clone();
            f.nodata = r.nodata;
            f.vertical = r.vertical.clone();
            f.citation = r.citation.clone();
            f.pixel_is_point = r.pixel_is_point;
            let mut keys = r.keys.clone();
            if let Some(v) = &r.version {
                keys.insert(0, ("Version".to_owned(), v.clone()));
            }
            if r.gcps > 0 {
                keys.push(("Ground control points".to_owned(), r.gcps.to_string()));
            }
            f.sidecars = r.sidecars.clone();
            // Formats with their own header (ENVI, ASCII grid, DTED, IMAGINE) list its lines.
            let kind = if r.keys_kind == "header" {
                "header"
            } else {
                "geokeys"
            };
            f.fields.insert(0, (kind, keys));
            let gdal = r
                .gdal
                .items
                .iter()
                .map(|i| {
                    let mut tag = i.name.clone();
                    if let Some(b) = i.sample {
                        tag = format!("{tag} (band {})", b + 1);
                    }
                    if let Some(role) = &i.role {
                        tag = format!("{tag} [{role}]");
                    }
                    (tag, i.value.clone())
                })
                .collect();
            f.fields.insert(1, ("gdal", gdal));
        }
        F::Vector(v) => {
            f.layers = v
                .layers
                .iter()
                .map(|l| specular_metadata::GeoLayer {
                    name: l.name.clone(),
                    features: l.features as u64,
                    types: l
                        .types
                        .iter()
                        .map(|(t, n)| ((*t).to_owned(), *n as u64))
                        .collect(),
                    fields: l.fields as u64,
                })
                .collect();
            f.features = Some(v.features as u64);
            f.vertices = Some(v.vertices as u64);
            f.encoding = v.encoding.clone();
            f.warnings = v.warnings.clone();
            if let Some(m) = &v.tile_meta {
                tile_meta_facts(&mut f, m);
            }
        }
        F::Tiles(t) => {
            if let Some(m) = &t.tile_meta {
                tile_meta_facts(&mut f, m);
            }
            f.pixel_size = Some(g.to_map.pixel_size());
            f.tiles = Some((t.table.clone(), t.zoom, t.tiles as u64));
            f.levels = t
                .levels
                .iter()
                .rev()
                .map(|l| {
                    (
                        (l.matrix_w as u64 * u64::from(l.tile_w)).min(u64::from(u32::MAX)) as u32,
                        (l.matrix_h as u64 * u64::from(l.tile_h)).min(u64::from(u32::MAX)) as u32,
                    )
                })
                .collect();
        }
    }
    f
}

/// The info panel's facts of a scientific image: its description, axes, and the sky position
/// of its centre (sexagesimal and degrees are not localised), frame, projection and scale.
/// The info panel's facts of a drawing page.
pub(crate) fn drawing_facts(
    d: &specular_decode::drawing::DrawingPage,
) -> specular_metadata::DrawingFacts {
    use specular_decode::drawing::specular_vector::SheetKind;
    let scene = &d.prepared.scene;
    let mut version = scene.version.clone();
    if let Some(r) = &scene.release {
        version = format!("{version} ({r})");
    }
    let model = scene.sheets.iter().find(|s| s.kind == SheetKind::Model);
    let layout = d
        .sheet()
        .filter(|s| s.kind != SheetKind::Model)
        .map(|s| crate::app::cad::shown_name(&s.name, s.generated));
    specular_metadata::DrawingFacts {
        format: scene.format.clone(),
        version,
        units: scene.units.code,
        unit_symbol: scene.units.symbol().map(str::to_owned),
        // A plot's pages have no model space: the extents are the page's; a layout's pages
        // are its top cells.
        extents: if scene.plot.is_some() {
            d.sheet().and_then(|s| s.fit)
        } else if scene.layout.is_some() {
            scene.sheets.get(d.sheet).and_then(|s| s.extents)
        } else {
            model.and_then(|m| m.extents)
        }
        .map(|r| (r.width(), r.height())),
        layout,
        layouts: d.prepared.sheets.len().saturating_sub(1) as u32,
        layers: scene.layers.len() as u32,
        layers_off: scene.layers.iter().filter(|l| !l.visible()).count() as u32,
        counts: scene.counts.clone(),
        codepage: scene.codepage.clone(),
        missing_fonts: d
            .prepared
            .missing_fonts
            .iter()
            .map(|m| specular_metadata::MissingFontFact {
                file: m.file.clone(),
                styles: m.styles.clone(),
                substitute: m.substitute.clone(),
            })
            .collect(),
        shx_fonts: {
            let mut v: Vec<String> = scene
                .fonts
                .iter()
                .filter_map(|f| f.shx.as_ref())
                .flat_map(|x| x.main.iter().chain(x.big.iter()))
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .collect();
            v.sort_by_key(|s| s.to_ascii_lowercase());
            v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
            v
        },
        warnings: scene.warnings.clone(),
        plot: scene.plot.as_ref().map(|p| specular_metadata::PlotFacts {
            paper_mm: p.paper_mm,
            pens: p.pens.clone(),
            page: d.sheet as u32 + 1,
            pages: d.prepared.sheets.len() as u32,
            job: p.pcl.then(|| "PCL 5".to_owned()),
        }),
        ic: scene.layout.as_ref().map(|l| {
            let i = &l.info;
            specular_metadata::IcLayoutFacts {
                library: i.library.clone(),
                db_unit_m: i.db_unit_m,
                user_unit_m: i.db_unit_m / i.user_unit,
                cell: d.sheet().map(|s| s.name.clone()).unwrap_or_default(),
                cells: i.cells as u64,
                top_cells: i.top_cells.clone(),
                layers: i.layers as u32,
                shapes: i.shapes,
                texts: i.texts,
                references: i.references,
                placements: i.placements,
                flat_shapes: i.flat_shapes,
                bounds_um: scene
                    .sheets
                    .get(d.sheet)
                    .and_then(|s| s.extents)
                    .map(|r| [r.x0, r.y0, r.x1, r.y1]),
            }
        }),
        survey: scene.survey.as_ref().map(|s| {
            let imperial = !matches!(scene.units.code, 4..=7);
            let unit = scene.units.symbol().unwrap_or("").to_owned();
            specular_metadata::SurveyFacts {
                crs: s.crs.clone(),
                unit,
                application: s.application.clone(),
                project: s.project.clone(),
                surfaces: s
                    .surfaces
                    .iter()
                    .map(|f| specular_metadata::SurveySurface {
                        name: f.name.clone(),
                        points: f.points,
                        triangles: f.triangles,
                        area_2d: f.area_2d,
                        area_3d: f.area_3d,
                        volume: f.volume,
                        z_min: f.z_min,
                        z_max: f.z_max,
                        contour_interval: f.contour_interval,
                    })
                    .collect(),
                alignments: s
                    .alignments
                    .iter()
                    .map(|a| specular_metadata::SurveyAlignment {
                        name: a.name.clone(),
                        length: a.length,
                        sta_start: specular_decode::station_label(a.sta_start, imperial),
                        sta_end: specular_decode::station_label(a.sta_end, imperial),
                    })
                    .collect(),
            }
        }),
        pcb: scene.pcb.as_ref().map(|p| {
            use specular_decode::drawing::specular_vector::PcbKind;
            let schematic = p.kind == PcbKind::KicadSchematic;
            specular_metadata::PcbFacts {
                schematic,
                // A board's view, or the sheet of a schematic or board set shown.
                view: (d.prepared.sheets.len() > 1)
                    .then(|| d.sheet())
                    .flatten()
                    .map(|s| {
                        let v = crate::app::cad::shown_name(&s.name, s.generated);
                        match &s.group {
                            Some(g) => specular_i18n::t!(
                                "page-label-board-step",
                                step = g.clone(),
                                view = v
                            ),
                            None => v,
                        }
                    }),
                generator: p.generator.clone(),
                project: p.project.clone(),
                revision: p.revision.clone(),
                date: p.date.clone(),
                size_mm: p.size_mm,
                thickness_mm: p.thickness_mm,
                copper_layers: p.copper_layers,
                layers: p
                    .layers
                    .iter()
                    .map(|l| (l.name.clone(), l.role.clone()))
                    .collect(),
                holes: p.holes,
                drill_sizes: p.drill_sizes,
                components: p.components,
                nets: p.nets,
                tracks: p.tracks,
                vias: p.vias,
                sheets: p.sheets,
                apertures: p.apertures,
                macros: p.macros,
            }
        }),
    }
}

/// A spectrum file's facts for the info panel.
fn spectrum_facts(s: &specular_decode::SpectrumFacts) -> specular_metadata::SpectrumFacts {
    specular_metadata::SpectrumFacts {
        format: s.format.clone(),
        version: s.version.clone(),
        technique: s.technique.clone(),
        title: s.title.clone(),
        origin: s.origin.clone(),
        instrument: s.instrument.clone(),
        date: s.date.clone(),
        spectra: s.spectra,
        points: s.points,
        nucleus: s.nucleus.clone(),
        frequency_mhz: s.frequency_mhz,
        solvent: s.solvent.clone(),
        pulse_program: s.pulse_program.clone(),
        page: s.page,
        pages: s.pages,
        page_name: s.page_name.clone(),
        x_axis: s.x_axis.clone(),
        y_axis: s.y_axis.clone(),
        notes: s.notes.clone(),
        warnings: s.warnings.clone(),
    }
}

/// A thermal still's facts for the info panel: the camera's recorded object parameters and
/// the temperatures under them, in degrees Celsius as recorded.
fn thermal_facts(t: &specular_decode::science::ThermalImage) -> specular_metadata::ThermalFacts {
    use specular_decode::science::{TemperatureUnit, ThermalImage};
    let c = t.file_conditions();
    let cam = t.camera();
    let some = |s: &str| Some(s.trim().to_owned()).filter(|s| !s.is_empty());
    let range = t
        .plane(&c, TemperatureUnit::Celsius, &CancelToken::default())
        .ok()
        .and_then(|p| ThermalImage::range_of(&p))
        .map(|(lo, hi)| (f64::from(lo), f64::from(hi)));
    specular_metadata::ThermalFacts {
        format: t.kind_name().to_owned(),
        camera: some(&cam.model),
        lens: some(&cam.lens),
        emissivity: c.emissivity,
        distance: c.distance,
        reflected: c.reflected,
        atmospheric: c.atmospheric,
        humidity: c.humidity,
        raw_range: t.raw_range(),
        temperature_range: range,
        unit: TemperatureUnit::Celsius.symbol().to_owned(),
    }
}

/// `f` (in degrees Celsius, as [`thermal_facts`] makes them) with its temperatures in `unit`.
fn thermal_facts_in(
    f: &mut specular_metadata::ThermalFacts,
    unit: specular_decode::science::TemperatureUnit,
) {
    let to = |v: f64| unit.from_celsius(v);
    f.reflected = f.reflected.map(to);
    f.atmospheric = f.atmospheric.map(to);
    f.temperature_range = f.temperature_range.map(|(lo, hi)| (to(lo), to(hi)));
    f.unit = unit.symbol().to_owned();
}

pub(crate) fn science_facts(
    s: &specular_decode::science::ScienceSummary,
    format: &str,
) -> specular_metadata::ScienceFacts {
    use specular_decode::science::ScienceKind;
    use specular_decode::science::wcs::{self, Frame};
    let mut dims = vec![u64::from(s.width), u64::from(s.height)];
    dims.extend(s.axes.iter().map(|a| u64::from(a.len)));
    let mut f = specular_metadata::ScienceFacts {
        format: format.to_owned(),
        object: s.object.clone(),
        telescope: s.telescope.clone(),
        instrument: s.instrument.clone(),
        filter: s.filter.clone(),
        date_obs: s.date_obs.clone(),
        exposure: s.exposure,
        dims,
        sample: Some(s.sample.clone()),
        unit: s.unit.clone(),
        hdu: s.hdu.clone(),
        dataset: s.dataset.clone(),
        pixel_size: s
            .pixel
            .as_ref()
            .map(|c| (c.x.step.abs(), c.y.step.abs(), c.unit_x.clone())),
        keywords: s.keywords.as_ref().clone(),
        domain: match s.kind {
            ScienceKind::Astronomy => specular_metadata::ScienceDomain::Astronomy,
            ScienceKind::ElectronMicroscopy => specular_metadata::ScienceDomain::ElectronMicroscopy,
            ScienceKind::ScanningProbe => specular_metadata::ScienceDomain::ScanningProbe,
            ScienceKind::Slide => specular_metadata::ScienceDomain::Slide,
            ScienceKind::Gridded => specular_metadata::ScienceDomain::Gridded,
            ScienceKind::Microscopy => specular_metadata::ScienceDomain::Microscopy,
            ScienceKind::Diffraction => specular_metadata::ScienceDomain::Diffraction,
            ScienceKind::Surface => specular_metadata::ScienceDomain::Surface,
            ScienceKind::Ebsd => specular_metadata::ScienceDomain::Ebsd,
            ScienceKind::Seismic => specular_metadata::ScienceDomain::Seismic,
            ScienceKind::MassSpec => specular_metadata::ScienceDomain::MassSpec,
            ScienceKind::Thermal => specular_metadata::ScienceDomain::Thermal,
            ScienceKind::Spectroscopy => specular_metadata::ScienceDomain::Spectroscopy,
            ScienceKind::Cytometry => specular_metadata::ScienceDomain::Cytometry,
        },
        thermal: s.thermal.as_ref().map(|t| thermal_facts(t)),
        vendor: s.vendor.clone(),
        levels: s.levels.clone(),
        title: s.title.clone(),
        voltage: s.voltage,
        magnification: s.magnification,
        mode: s.mode.clone(),
        detector: s.detector.clone(),
        observer: s.observer.clone(),
        bayer: s.cfa.map(|c| c.name()),
        frames: s.video.as_ref().map(|v| u64::from(v.frames)),
        frames_declared: s.video.as_ref().map(|v| u64::from(v.declared)),
        capture_start: s.video.as_ref().and_then(|v| v.start.clone()),
        capture_end: s.video.as_ref().and_then(|v| v.end.clone()),
        frame_rate: s.video.as_ref().and_then(|v| v.frame_rate),
        diffraction: s
            .diffraction
            .as_ref()
            .map(|g| specular_metadata::DiffractionFacts {
                wavelength: g.wavelength,
                energy: g.energy_kev(),
                distance: g.distance,
                beam: g.beam,
                two_theta: g.two_theta,
                phi: g.phi,
                omega: g.omega,
                chi: g.chi,
                kappa: g.kappa,
                oscillation: g
                    .osc_start
                    .zip(g.osc_width)
                    .map(|(a, w)| (a, w, g.osc_axis.clone())),
                overload: g.overload,
                resolution: g.edge_resolution(s.width, s.height),
                complete: g.complete(),
                curved: g.curved,
                binning: g.binning,
            }),
        hdf4: s.hdf4.as_ref().map(|h| specular_metadata::Hdf4Facts {
            library: h.library.clone(),
            datasets: h.datasets,
            images: h.images,
            vgroups: h.vgroups,
            vdatas: h.vdatas,
            eos: h.eos.as_ref().map(|e| specular_metadata::HdfEosFacts {
                version: e.version.clone(),
                grids: e.grids.clone(),
                swaths: e.swaths.clone(),
                points: e.points,
                grid: e.grid.clone(),
                swath: e.swath.clone(),
                projection: e.projection.clone(),
                grid_size: e.grid_size,
                corners: e.corners,
                corners_degrees: e.corners_degrees,
                short_name: e.short_name.clone(),
                begin: e.begin.clone(),
                end: e.end.clone(),
            }),
        }),
        planetary: s.planetary.as_ref().map(|p| {
            let m = p.map.as_ref();
            specular_metadata::PlanetaryFacts {
                target: p.target.clone(),
                mission: p.mission.clone(),
                spacecraft: p.spacecraft.clone(),
                instrument: p.instrument.clone(),
                start: p.start.clone(),
                stop: p.stop.clone(),
                product_id: p.product_id.clone(),
                projection: m.map(|m| m.projection.clone()),
                metres_per_pixel: m.and_then(|m| m.metres_per_pixel),
                pixels_per_degree: m.and_then(|m| m.pixels_per_degree),
                centre: m.and_then(|m| m.centre),
                latitudes: m.and_then(|m| m.latitudes),
                longitudes: m.and_then(|m| m.longitudes),
                radius_km: m.and_then(|m| m.radius_km),
            }
        }),
        omero: s.omero.as_ref().map(|o| specular_metadata::OmeroFacts {
            server: o.server.clone(),
            user: o.user.clone(),
            image_id: o.image_id,
            name: o.name.clone(),
            project: o.project.clone(),
            dataset: o.dataset.clone(),
            screen: o.screen.clone(),
            plate: o.plate.clone(),
            well: o.well.clone(),
            owner: o.owner.clone(),
            group: o.group.clone(),
            acquired: o.acquired.clone(),
            size: o.size,
            pixels_type: o.pixels_type.clone(),
            physical: o.physical.clone(),
            channels: o
                .channels
                .iter()
                .map(|c| (c.name.clone(), c.active))
                .collect(),
            tiles: o.tiles,
            encoding: o.encoding.to_string(),
        }),
        ..Default::default()
    };
    if let Some(c) = &s.pixel {
        let q = crate::app::quantity_text;
        let (dx, dy) = (c.x.step.abs(), c.y.step.abs());
        f.field_of_view = Some(format!(
            "{} × {}",
            q(dx * f64::from(s.width), &c.unit_x),
            q(dy * f64::from(s.height), &c.unit_y)
        ));
    }
    if let Some(w) = &s.wcs
        && let Some(c) = &w.celestial
    {
        let rest = vec![0; w.axes.len().saturating_sub(2)];
        let centre = w.sky_at(f64::from(s.width) / 2.0, f64::from(s.height) / 2.0, &rest);
        f.centre = centre.map(|(lng, lat)| match c.frame {
            Frame::Equatorial { .. } => (
                format!("{}  {}", wcs::hms(lng), wcs::dms(lat)),
                format!("{:.5}°  {:+.5}°", lng.rem_euclid(360.0), lat).replace('-', "\u{2212}"),
            ),
            _ => (
                format!("{:.4}°  {:+.4}°", lng.rem_euclid(360.0), lat).replace('-', "\u{2212}"),
                format!("{:.5}°  {:+.5}°", lng.rem_euclid(360.0), lat).replace('-', "\u{2212}"),
            ),
        });
        f.frame = Some(match &c.frame {
            Frame::Equatorial { system, equinox } => match equinox {
                // Besselian epochs for FK4, Julian otherwise.
                Some(e) if system.starts_with("FK") => {
                    let b = if system == "FK4" { 'B' } else { 'J' };
                    format!("{system} {b}{e}")
                }
                _ => system.clone(),
            },
            Frame::Galactic => "Galactic".to_owned(),
            Frame::Ecliptic => "Ecliptic".to_owned(),
            Frame::Other(s) => s.clone(),
        });
        f.projection = Some(c.projection.code().to_owned());
        f.pixel_scale = w.arcsec_per_pixel();
        let d = w.distortions();
        f.distortion = (!d.is_empty()).then(|| d.join(", "));
    }
    if let Some(why) = s.wcs.as_ref().and_then(|w| w.unsupported.as_ref()) {
        f.wcs_unsupported = Some(why.to_string());
    }
    f.wcs_approximate = s.wcs_approximate;
    f.asdf = s.asdf.as_ref().map(|a| specular_metadata::AsdfFacts {
        version: a.version.clone(),
        standard: a.standard.clone(),
        library: a.library.clone(),
        model: a.model.clone(),
    });
    f.root = s.root.as_deref().map(|r| specular_metadata::RootFacts {
        version: r.version.clone(),
        compression: r.compression.clone(),
        keys: r.keys,
        class: r.class.clone(),
        path: r.path.clone(),
        title: r.title.clone(),
        entries: r.entries,
        sum_weights: r.sum_weights,
        axes: r
            .axes
            .iter()
            .map(|a| specular_metadata::RootAxisFacts {
                title: a.title.clone(),
                unit: a.unit.clone(),
                bins: a.bins,
                min: a.min,
                max: a.max,
                variable: a.variable,
                mean_rms: a.mean_rms,
            })
            .collect(),
        points: (!r.points.is_empty()).then_some(r.points.len() as u64),
        trees: r.trees.clone(),
    });
    f.casa = s.casa.as_ref().map(|c| specular_metadata::CasaFacts {
        image_type: c.image_type.to_owned(),
        info_type: c.info_type.clone(),
        beam: c.beam,
        plane_beams: c.plane_beams.map(|b| (b.count, b.major)),
        rest_frequency: c.rest_frequency,
        spectral_frame: c.spectral_frame.clone(),
        velocity: c.velocity.map(str::to_owned),
        mask: c.mask.clone(),
        storage: c.storage.to_owned(),
        big_endian: c.big_endian,
    });
    f.ugrid = s.mesh.as_ref().map(|m| {
        let m = m.facts();
        specular_metadata::UgridFacts {
            name: match &m.long_name {
                Some(l) if *l != m.name => format!("{} ({l})", m.name),
                _ => m.name.clone(),
            },
            convention: m.convention.to_owned(),
            nodes: m.nodes,
            faces: m.faces,
            edges: m.edges,
            triangles: m.triangles,
            quads: m.quads,
            polygons: m.polygons,
            max_face_nodes: m.max_face_nodes,
            geographic: m.geographic,
            unit: m.unit.clone(),
            on_faces: m
                .location
                .map(|l| l == specular_decode::science::MeshLocation::Face),
            split_faces: m.split_faces,
        }
    });
    f.annotations = s
        .annotations
        .as_deref()
        .map(|a| annotation_facts(a, s.pixel.as_ref()));
    f
}

/// A slide's annotations for the info panel, with areas and lengths when the pixel size is
/// known.
fn annotation_facts(
    a: &specular_decode::science::AnnotationSet,
    pixel: Option<&specular_decode::science::PixelCalibration>,
) -> specular_metadata::AnnotationFacts {
    use specular_decode::science::annot::Shape;
    // Square pixels in one unit, else pixels.
    let (dx, dy, unit) = match pixel {
        Some(c) if c.unit_x == c.unit_y => (c.x.step.abs(), c.y.step.abs(), c.unit_x.as_str()),
        _ => (1.0, 1.0, ""),
    };
    let items = a
        .set
        .items
        .iter()
        .map(|it| {
            let measure = match it.shape {
                Shape::Point | Shape::Points => None,
                s if s.closed() => Some(crate::app::area_text(it.area() * dx * dy, unit)),
                _ if unit.is_empty() => Some(format!(
                    "{} px",
                    crate::app::quantity_text(it.length(1.0, 1.0), "")
                )),
                _ => Some(crate::app::quantity_text(it.length(dx, dy), unit)),
            };
            specular_metadata::AnnotationItem {
                name: it.name.clone(),
                group: it.group.clone(),
                shape: it.shape.id().to_owned(),
                measure,
            }
        })
        .collect();
    specular_metadata::AnnotationFacts {
        file: a.file.clone(),
        format: a.set.format.name().to_owned(),
        items,
        skipped: a.set.skipped,
    }
}

/// The sample depth a slide's coding names: "16-bit grey, JPEG 2000", "8-bit RGB, JPEG",
/// "uint16, uncompressed" (OME pixel types), "32-bit float, Zarr", "tiled 256×256, LZW,
/// MinIsBlack 12-bit"; `None` when it names none.
fn slide_sample_bits(coding: &str) -> Option<u8> {
    coding
        .split([',', ' '])
        .find_map(|w| {
            if let Some(n) = w.strip_suffix("-bit") {
                return n.parse::<u8>().ok();
            }
            match w {
                "bit" => Some(1),
                "float" => Some(32),
                "double" => Some(64),
                _ => w
                    .strip_prefix("uint")
                    .or_else(|| w.strip_prefix("int"))
                    .and_then(|n| n.parse::<u8>().ok()),
            }
        })
        .filter(|&b| b > 0)
}

/// The metadata localizer for `lang`, joining the parts of a line ("Rectangle · Group 1")
/// in that language's direction.
pub(crate) fn meta_localizer(
    lang: specular_i18n::LanguageIdentifier,
) -> specular_metadata::Localizer {
    let dir = specular_i18n::direction(&lang);
    specular_metadata::Localizer::new(lang).with_join(Arc::new(move |parts, sep| {
        specular_i18n::join_parts(parts, sep, dir)
    }))
}

/// The info panel's report for an open image: the file facts the session knows (size, bit
/// depth, colour model, subsampling) and the Exif, XMP and ICC found from headers alone.
/// The image's metadata report, and its motion clip when it is a Motion Photo or Live Photo
/// (found from the same Exif and XMP, plus a `stat` of the Live Photo sibling names).
pub(crate) fn metadata_report(
    session: &dyn Session,
    loc: &specular_metadata::Localizer,
) -> (
    specular_metadata::MetadataReport,
    Option<specular_video::MotionInfo>,
) {
    metadata_report_in(
        session,
        loc,
        specular_decode::science::TemperatureUnit::Celsius,
    )
}

/// [`metadata_report`] with a thermal still's temperatures in `unit` (the viewer's
/// temperature unit, as the readout shows them).
pub(crate) fn metadata_report_in(
    session: &dyn Session,
    loc: &specular_metadata::Localizer,
    unit: specular_decode::science::TemperatureUnit,
) -> (
    specular_metadata::MetadataReport,
    Option<specular_video::MotionInfo>,
) {
    let (mut report, motion) = metadata_report_of(session, loc, unit);
    darktable_row(&mut report, session.source().path());
    foreign_sidecar_rows(&mut report, session.source().path());
    (report, motion)
}

/// "From darktable sidecar" (and the other apps'): what each sidecar another app wrote beside
/// the photo says, in the File group; then darktable's duplicates.
fn foreign_sidecar_rows(report: &mut specular_metadata::MetadataReport, path: &std::path::Path) {
    use specular_metadata::sidecars;
    if path.as_os_str().is_empty() || !path.is_file() {
        return;
    }
    let found = sidecars::found_for(path, sidecars::stem_is_own_on_disk(path));
    if found.is_empty() {
        return;
    }
    let mut items = Vec::new();
    let mut dups = Vec::new();
    for f in &found {
        let what = sidecar_values_text(&f.read.values);
        if f.candidate.duplicate > 0 {
            let what = match &f.read.version_name {
                Some(n) if !n.is_empty() => format!("{n} · {what}"),
                _ => what,
            };
            dups.push(specular_i18n::t!(
                "info-sidecar-duplicate",
                number = i64::from(f.candidate.duplicate),
                what = what
            ));
            continue;
        }
        let label = match f.read.app.and_then(sidecars::App::name) {
            Some(app) => specular_i18n::t!("info-sidecar-from", app = app),
            None => specular_i18n::t!("info-sidecar-from-other"),
        };
        items.push(specular_metadata::SummaryItem {
            key: "foreign-sidecar",
            label,
            value: what,
            detail: Some(specular_i18n::t!(
                "info-sidecar-file",
                file = f.candidate.name.as_str()
            )),
        });
    }
    if !dups.is_empty() {
        items.push(specular_metadata::SummaryItem {
            key: "darktable-duplicates",
            label: specular_i18n::t!("info-sidecar-duplicates"),
            value: dups.join("\n"),
            detail: None,
        });
    }
    match report.summary.iter_mut().find(|g| g.key == "file") {
        Some(g) => g.items.extend(items),
        None => report.summary.push(specular_metadata::SummaryGroup {
            key: "file",
            title: specular_i18n::t!("info-darktable-group"),
            items,
        }),
    }
}

/// What a sidecar says, for people: "3 stars, Green, Rejected, Keywords: a, b".
fn sidecar_values_text(v: &specular_metadata::sidecars::Values) -> String {
    use specular_i18n::t;
    let mut parts = Vec::new();
    if let Some(r) = v.rating.filter(|&r| r > 0) {
        parts.push(t!("info-sidecar-stars", count = i64::from(r)));
    }
    if let Some(l) = v.label.filter(|&l| l > 0) {
        parts.push(match l {
            1 => t!("gallery-label-red"),
            2 => t!("gallery-label-yellow"),
            3 => t!("gallery-label-green"),
            _ => t!("gallery-label-blue"),
        });
    }
    match v.flag {
        Some(-1) => parts.push(t!("info-sidecar-rejected")),
        Some(1) => parts.push(t!("info-sidecar-picked")),
        _ => {}
    }
    if let Some(k) = v.keywords.as_ref().filter(|k| !k.is_empty()) {
        let list: Vec<String> = k
            .iter()
            .map(|k| specular_metadata::keywords::display(k))
            .collect();
        parts.push(t!("info-sidecar-keywords", keywords = list.join(", ")));
    }
    for s in [&v.title, &v.caption].into_iter().flatten() {
        parts.push(specular_i18n::isolate(s));
    }
    if parts.is_empty() {
        return t!("info-sidecar-nothing");
    }
    parts.join(", ")
}

/// "Edited in darktable" with the module names, in the File group, when the photo has a
/// darktable sidecar (names only, the edit is not converted).
fn darktable_row(report: &mut specular_metadata::MetadataReport, path: &std::path::Path) {
    let Some(d) = specular_develop::interop::darktable::read_for(path) else {
        return;
    };
    let item = specular_metadata::SummaryItem {
        key: "darktable",
        label: specular_i18n::t!("info-darktable-edited"),
        value: if d.modules.is_empty() {
            specular_i18n::t!("info-darktable-no-modules")
        } else {
            d.modules.join(", ")
        },
        detail: None,
    };
    match report.summary.iter_mut().find(|g| g.key == "file") {
        Some(g) => g.items.push(item),
        None => report.summary.push(specular_metadata::SummaryGroup {
            key: "file",
            title: specular_i18n::t!("info-darktable-group"),
            items: vec![item],
        }),
    }
}

fn metadata_report_of(
    session: &dyn Session,
    loc: &specular_metadata::Localizer,
    unit: specular_decode::science::TemperatureUnit,
) -> (
    specular_metadata::MetadataReport,
    Option<specular_video::MotionInfo>,
) {
    let src = session.source();
    let mut facts = specular_metadata::FileFacts {
        path: Some(src.path().to_path_buf()),
        file_size: Some(src.len() as u64),
        modified: src.modified(),
        ..Default::default()
    };
    let info = session.info();
    facts.format_name = Some(info.format.name().to_owned());
    // A drawn picture's pixel size is the viewer's choice, not the file's; an SVG states its
    // own size.
    facts.drawn = info.format.is_drawn();
    if !facts.drawn || info.format == specular_decode::Format::Svg {
        facts.width = Some(info.width);
        facts.height = Some(info.height);
    }
    // A camera RAW whose sensor data is not developed shows its embedded preview: the
    // session's depth and coding are the preview's, while the file's samples (the colour
    // model it reports) are the sensor's, so the row names only the sensor layout.
    // A Krita document without a merged image shows its preview the same way, as do
    // Procreate and Affinity documents (always).
    let raw_preview = matches!(
        info.format,
        specular_decode::Format::Raw
            | specular_decode::Format::Kra
            | specular_decode::Format::Procreate
            | specular_decode::Format::Affinity
            | specular_decode::Format::Cdr
            | specular_decode::Format::Visio
            | specular_decode::Format::Sketch
            | specular_decode::Format::Figma
            | specular_decode::Format::OpenDocument
            | specular_decode::Format::Ooxml
            | specular_decode::Format::MsOffice
            | specular_decode::Format::Iwork
    ) && info.coding.contains("embedded preview only");
    facts.bit_depth = (!raw_preview).then_some(info.bit_depth);
    // A slide or OME image is shown through an 8-bit composite; the Image group gives the
    // file's sample depth (the coding names it: "16-bit grey, …", "uint16, …").
    if matches!(
        info.format,
        specular_decode::Format::Svs
            | specular_decode::Format::Ndpi
            | specular_decode::Format::Scn
            | specular_decode::Format::OmeTiff
            | specular_decode::Format::OmeZarr
            | specular_decode::Format::PyramidTiff
            | specular_decode::Format::DicomWsi
            | specular_decode::Format::Mrxs
            | specular_decode::Format::Bif
            | specular_decode::Format::Vsi
            | specular_decode::Format::Czi
    ) && let Some(b) = slide_sample_bits(&info.coding)
    {
        facts.bit_depth = Some(b);
    }
    // Baseline versus progressive is a JPEG distinction ("8-bit RGB, baseline" would be
    // noise for a PNG); other formats name only progressive (interlaced) coding.
    facts.progressive = if info.format == specular_decode::Format::Jpeg {
        Some(info.progressive)
    } else {
        (info.progressive && !raw_preview).then_some(true)
    };
    // A hierarchical JPEG says so in place of baseline or progressive.
    facts.hierarchical = info.format == specular_decode::Format::Jpeg
        && specular_decode::jpeg_is_hierarchical(src.bytes());
    facts.colour_model = session.colour_model().map(str::to_owned);
    facts.chroma_subsampling = session.chroma_subsampling().map(str::to_owned);
    facts.alpha = session.alpha().map(|a| a.key().to_owned());
    let geo = session.geo();
    // Models, point clouds, tractograms, structures, drawings and vector maps are drawn at
    // whatever size the view asks for: the session's raster is a canvas, not the file's
    // pixels, so they get no Image group (their own group describes the data).
    let vector = session.tracts().is_some()
        || session.mesh().is_some()
        || session.points().is_some()
        || session.splats().is_some()
        || session.molecule().is_some()
        || session.font().is_some()
        || session.text().is_some()
        || info.format.is_text()
        || session.drawing().is_some()
        || geo
            .as_deref()
            .is_some_and(|g| matches!(g.facts, specular_decode::geo::GeoFacts::Vector(_)));
    if vector {
        facts.width = None;
        facts.height = None;
        facts.bit_depth = None;
        facts.progressive = None;
        facts.colour_model = None;
        facts.chroma_subsampling = None;
        facts.alpha = None;
    }
    // A document encapsulated in a DICOM file (PDF, CDA, STL, OBJ): the study summary and
    // the document's title and type (no patient attributes), and the model's facts for STL /
    // OBJ. The document's own metadata is not read: it could name the patient.
    if let Some(e) = session.encapsulated() {
        let m = &e.summary;
        facts.format_name = Some(format!("DICOM ({} document)", e.kind));
        let kind = match &e.mime {
            Some(mime) => format!("{} ({mime})", e.kind),
            None => e.kind.to_owned(),
        };
        let medical = specular_metadata::MedicalFacts {
            modality: m.modality.clone(),
            body_part: m.body_part.clone(),
            study_description: m.study_description.clone(),
            series_description: m.series_description.clone(),
            document_title: e.title.clone(),
            document_type: Some(kind),
            ..Default::default()
        };
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                medical: Some(medical),
                mesh: session.mesh().map(|x| mesh_facts(x, e.kind)),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A DICOM image: its medical summary (no patient attributes) and no Exif, XMP or ICC.
    // The preamble is free-form, so it is not scanned as a JPEG.
    if let Some(m) = session.medical() {
        // A research volume (NIfTI, Analyze, NRRD, MetaImage) has its own group: no transfer
        // syntax, and its description and slice spacing are told there.
        let volume = session
            .research_volume()
            .map(|v| specular_metadata::VolumeFacts {
                format: v.format.to_owned(),
                dims: v.dims.iter().map(|&d| d as u64).collect(),
                voxel_size: Some(v.spacing),
                data_type: if v.colour { "RGB" } else { v.data_type }.to_owned(),
                volumes: v.volumes as u64,
                orientation: v.orientation.to_owned(),
                reliable: v.reliable,
                description: v.description.clone(),
                intent: v.intent.clone(),
                compression: v.compression.map(str::to_owned),
                params: v
                    .params
                    .iter()
                    .map(|p| {
                        let value = match &p.value {
                            specular_decode::medvol::ParamValue::Number(x, u) => {
                                specular_metadata::VolumeParam::Number(*x, (*u).to_owned())
                            }
                            specular_decode::medvol::ParamValue::Text(t) => {
                                specular_metadata::VolumeParam::Text(t.clone())
                            }
                        };
                        (p.key.to_owned(), value)
                    })
                    .collect(),
            });
        let research = volume.is_some();
        let medical = specular_metadata::MedicalFacts {
            modality: m.modality.clone(),
            body_part: m.body_part.clone(),
            study_description: m.study_description.clone(),
            series_description: (!research).then(|| m.series_description.clone()).flatten(),
            slice_thickness: m.slice_thickness.filter(|_| !research),
            pixel_spacing: m.pixel_spacing,
            imager_pixel_spacing: m.imager_pixel_spacing,
            frames: Some(m.frames),
            transfer_syntax: (!research).then(|| m.transfer_syntax.clone()),
            lossy: m.lossy,
            nde: m.nde.as_ref().map(|n| specular_metadata::NdeFacts {
                component_shape: n.component_shape.clone(),
                component_manufacturer: n.component_manufacturer.clone(),
                material_grade: n.material_grade.clone(),
                material_thickness: n.material_thickness,
                pipe_diameter: n.pipe_diameter,
                environment: n.environment.clone(),
                iqi: n.iqi.clone(),
                indications: n.indications,
                source: n.source.map(|(x, u)| (x, u.to_owned())),
            }),
            ..Default::default()
        };
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                file: facts,
                medical: Some(medical),
                volume,
                neuro: session.neuro().map(neuro_facts),
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A font face: its names, glyphs, outlines, embedding, axes and colour tables.
    if let Some(m) = session.font() {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                font: Some(font_facts(m)),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // An office document: what it is, its preview and its properties (with or without a
    // preview).
    if let Some(d) = session.document() {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                document: Some(document_facts(d)),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A book: title, authors, language, layout, contents (and its XMP, which repeats them,
    // is left out).
    if let Some(b) = session.book() {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                book: Some(book_facts(b)),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A molecular structure: entry, method, resolution, chains, atoms, cell.
    if let Some(m) = session.molecule() {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                structure: Some(structure_facts(m)),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A tractogram: its streamlines, grid and space.
    if session.tracts().is_some() {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                neuro: session.neuro().map(neuro_facts),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A 3D model: its triangles, size, unit, materials and textures (and a brain
    // surface's GIFTI or CIFTI facts, a CAD model's header and assembly tree).
    if let Some(m) = session.mesh() {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                mesh: Some(mesh_facts(m, info.format.name())),
                neuro: session.neuro().map(neuro_facts),
                header: session.header().map(|h| specular_metadata::HeaderFacts {
                    format: info.format.name().to_owned(),
                    summary: h.summary.clone(),
                    fields: h.fields.clone(),
                }),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A reflectance transformation image: its container, basis, planes and mapping.
    if let Some(r) = session.rti() {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                rti: Some(rti_facts(&r)),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A point cloud: its counts, box, CRS, attributes and classes (once read).
    if let Some(p) = session.points() {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                points: Some(points_facts(p)),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // Gaussian splats: their count, harmonics, box and up axis.
    if let Some(sp) = session.splats() {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                splats: Some(splat_facts(sp)),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A vector map or a GeoPackage tile pyramid: its CRS, bounds and layers.
    if let Some(g) = geo
        .as_deref()
        .filter(|g| !matches!(g.facts, specular_decode::geo::GeoFacts::Raster(_)))
    {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                geo: Some(geo_facts(g)),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A FITS or XISF image: what was observed, its axes and sky coordinates, and the header.
    // (A GeoTIFF shown as values keeps the TIFF's own metadata, with its geospatial group.)
    // A georeferenced grid with its own header (ASCII grid, DTED, ENVI, IMAGINE) gives both:
    // its values and header as science facts, where it sits as the geospatial group (the
    // header lines once, with the science facts).
    if let Some(s) = session.science().filter(|s| {
        s.geo.is_none()
            || !matches!(
                info.format,
                specular_decode::Format::Tiff | specular_decode::Format::GeoTiff
            )
    }) {
        let geo = geo.as_deref().map(|g| {
            let mut f = geo_facts(g);
            f.fields.retain(|(kind, _)| *kind != "header");
            f
        });
        let mut science = science_facts(s, info.format.name());
        if let Some(t) = science.thermal.as_mut() {
            thermal_facts_in(t, unit);
        }
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                science: Some(science),
                geo,
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A spectrum (JCAMP-DX, SPC, mzML, Bruker NMR): what was measured, the page's axes and
    // how it was processed, and the file's labelled fields.
    if let Some(sp) = session.spectrum() {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                spectrum: Some(spectrum_facts(sp)),
                header: session.header().map(|h| specular_metadata::HeaderFacts {
                    format: info.format.name().to_owned(),
                    summary: h.summary.clone(),
                    fields: h.fields.clone(),
                }),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    // A drawing (DXF): version, units, sheets, layers, entity counts and substituted fonts.
    if let Some(d) = session.drawing() {
        let report = specular_metadata::read(
            &specular_metadata::MetadataInput {
                drawing: Some(drawing_facts(d)),
                file: facts,
                ..Default::default()
            },
            loc,
        );
        return (report, None);
    }
    let motion = |exif: Option<&[u8]>, xmp: Option<&[u8]>| {
        let t0 = Instant::now();
        let xmp = xmp.and_then(|x| std::str::from_utf8(x).ok());
        let m = specular_video::probe_embedded(src.bytes(), xmp)
            .or_else(|| specular_video::probe_live(src.path(), exif));
        debug!(target: METADATA, "motion: {:?} in {:.2} ms", m.as_ref().map(|m| m.kind.key()), ms(t0));
        m
    };
    let add_motion = |r: &mut specular_metadata::MetadataReport,
                      m: &Option<specular_video::MotionInfo>| {
        if let Some(m) = m {
            r.add_motion(loc, m.kind.key(), m.duration_us, m.codec.as_deref());
        }
    };
    match specular_metadata::jpeg::scan(src.bytes()) {
        Ok(seg) => {
            let m = motion(seg.exif, seg.xmp);
            // A JPEG georeferenced by a world file keeps its Exif, with the geospatial group.
            let mut input = seg.input(facts);
            input.geo = geo.as_deref().map(geo_facts);
            let mut r = specular_metadata::read(&input, loc);
            add_motion(&mut r, &m);
            (r, m)
        }
        Err(_) => {
            // The Exif of the other formats that carry one, found without decoding.
            use specular_decode::Format;
            // A CR3 gives its CMT1/CMT2/CMT4 blocks merged into one TIFF structure; a
            // TIFF or TIFF-based RAW (DNG, NEF, CR2, ARW, ORF, RW2…) its IFD0, Exif and GPS
            // IFDs rebuilt without the image layout; a RAF the Exif of its JPEG preview, an
            // MRW its TTW block's. The search goes by the file's signature, so it runs for
            // every format (one it does not know gives nothing).
            let bytes = src.bytes();
            let exif = specular_decode::exif_tiff(bytes)
                // BPG and FLIF keep it in extensions the decoder has read.
                .or_else(|| session.embedded_exif().map(std::borrow::Cow::Owned))
                // A RAW whose metadata is not Exif (Canon CRW, Sigma X3F…): the camera and
                // exposure its decoder read.
                .or_else(|| {
                    let c = session.camera().filter(|_| {
                        matches!(
                            info.format,
                            Format::Raw | Format::CinemaRawLight | Format::ProResRaw
                        )
                    })?;
                    c.exif_tiff().map(std::borrow::Cow::Owned)
                });
            // XMP and ICC from headers alone: PNG iTXt / iCCP, WebP XMP / ICCP, the HEIF and
            // AVIF XMP item and `colr` profile, TIFF tags 700 / 34675, the JPEG XL `xml `
            // box, PSD resources 1060 / 1039, the CR3 XMP box; a JPEG XL profile comes from
            // the codestream the session has parsed.
            // Likewise found by signature, for every format.
            let embedded = specular_decode::embedded_metadata(src.bytes());
            let icc = embedded.icc.as_deref().or_else(|| session.embedded_icc());
            // A PDF's XMP (or its document information as XMP) comes from the parsed document.
            let session_xmp = embedded
                .xmp
                .is_none()
                .then(|| session.embedded_xmp())
                .flatten();
            // A movie's XMP and text tags (title, comment, location…) come from its demuxer;
            // a sound file's tags are its Sound group.
            let movie = session.video().filter(|v| v.sound.is_none());
            let xmp = embedded
                .xmp
                .as_deref()
                .or(session_xmp.as_deref())
                .or_else(|| movie.and_then(|v| v.xmp.as_deref()));
            let m = (info.format == Format::Heic)
                .then(|| motion(exif.as_deref(), xmp))
                .flatten();
            // PNG text chunks and GIF comments, found by signature; a movie's tags.
            let tags = specular_decode::text_tags(bytes)
                .into_iter()
                .chain(movie.and_then(|v| v.tags.clone()))
                .map(|t| specular_metadata::TagFacts {
                    kind: t.kind.to_owned(),
                    props: t
                        .props
                        .into_iter()
                        .map(|(k, v)| (k.to_owned(), v))
                        .collect(),
                    fields: t.fields,
                })
                .collect();
            let mut r = specular_metadata::read(
                &specular_metadata::MetadataInput {
                    exif_tiff: exif.as_deref(),
                    xmp,
                    icc,
                    iptc: embedded.iptc.as_deref(),
                    tags,
                    file: facts,
                    geo: geo.as_deref().map(geo_facts),
                    header: session.header().map(|h| specular_metadata::HeaderFacts {
                        format: info.format.name().to_owned(),
                        summary: h.summary.clone(),
                        fields: h.fields.clone(),
                    }),
                    ..Default::default()
                },
                loc,
            );
            add_motion(&mut r, &m);
            (r, m)
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {

    /// A film scan's format and slate in a right-to-left UI: one row, each part kept in its
    /// own order (isolated), the first read on the right; left to right, a plain join.
    #[test]
    fn metadata_joins_follow_the_ui_direction() {
        use specular_metadata::{HeaderFacts, MetadataInput};
        let input = MetadataInput {
            header: Some(HeaderFacts {
                format: "DPX".into(),
                summary: vec![("film", "Academy".into()), ("film", "Scene 4".into())],
                fields: Vec::new(),
            }),
            ..Default::default()
        };
        let film = |lang: &str| {
            let r = specular_metadata::read(&input, &super::meta_localizer(lang.parse().unwrap()));
            let g = r.summary.iter().find(|g| g.key == "header").unwrap();
            g.items[0].value.clone()
        };
        assert_eq!(film("en"), "Academy · Scene 4");
        let he = film("he");
        assert_eq!(
            he,
            specular_i18n::join_parts(
                &["Academy", "Scene 4"],
                " · ",
                specular_i18n::Direction::Rtl
            )
        );
        assert!(
            he.contains("\u{2068}Academy\u{2069} · \u{2068}Scene 4\u{2069}"),
            "{he:?}"
        );
    }

    #[test]
    fn header_reads_do_not_block_on_a_fifo() {
        let dir = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/fifo-header"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fifo = dir.join("pipe.jpg");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        // Opening a FIFO without a writer blocks: run on a thread and require a prompt answer.
        let (tx, rx) = std::sync::mpsc::channel();
        let f = fifo.clone();
        std::thread::spawn(move || {
            let _ = tx.send((super::header_display_size(&f), super::date_taken(&f)));
        });
        let r = rx.recv_timeout(std::time::Duration::from_secs(5));
        if r.is_err() {
            // Unblock the reader so the test process can exit.
            let _ = std::fs::OpenOptions::new().write(true).open(&fifo);
        }
        assert_eq!(r.expect("header read blocked on a FIFO"), (None, None));
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn stamp_formats_utc() {
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        assert_eq!(super::stamp(t), "2023:11:14 22:13:20");
    }
    #[test]
    fn lists_subfolders_grouped_in_natural_order() {
        let root = std::env::temp_dir().join(format!("specular-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for f in [
            "10.jpg",
            "2.jpg",
            "a/1.png",
            "a/b/3.jpg",
            "b/1.jpg",
            ".hidden/1.jpg",
            "a/notes.txt",
        ] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"x").unwrap();
        }
        let reg = specular_decode::Registry::new();
        let list = |recursive| {
            let opts = super::ListOptions {
                order: specular_settings::SortOrder::Natural,
                descending: false,
                recursive,
            };
            super::list_folder(&root, &reg, opts)
                .iter()
                .map(|p| {
                    // `/`-separated on every platform, to compare with the lists below.
                    p.strip_prefix(&root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/")
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(list(false), ["2.jpg", "10.jpg"]);
        assert_eq!(
            list(true),
            [
                "2.jpg",
                "10.jpg",
                "a/1.png",
                "a/notes.txt",
                "a/b/3.jpg",
                "b/1.jpg"
            ]
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The keyed folder sort gives the order of collating at every comparison, faster.
    #[test]
    fn keyed_folder_sort_matches_collating_each_comparison() {
        use super::{FolderEntry, natural_path_cmp, sort_entries};
        use specular_settings::SortOrder;
        let mut r = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            r ^= r << 13;
            r ^= r >> 7;
            r ^= r << 17;
            r
        };
        let stems = [
            "IMG_", "img_", "DSC", "Äpfel ", "apfel ", "scan-", "Ölbild", "photo",
        ];
        let exts = ["jpg", "JPG", "png", "cr3", "tif"];
        let entries = |n: usize, next: &mut dyn FnMut() -> u64| -> Vec<FolderEntry> {
            (0..n)
                .map(|_| {
                    let v = next();
                    let stem = stems[v as usize % stems.len()];
                    let ext = exts[(v >> 8) as usize % exts.len()];
                    let num = (v >> 16) % 5000;
                    let zeros = if v >> 40 & 1 == 1 { "0" } else { "" };
                    let name = match (v >> 32) % 4 {
                        0 => format!("{stem}{zeros}{num}.{ext}"),
                        1 => format!("d{}/{stem}{num}.{ext}", (v >> 44) % 12),
                        2 => format!("d{}/s{}/{stem}{num}.{ext}", (v >> 44) % 12, (v >> 50) % 3),
                        _ => format!("{stem}{num} ({}).{ext}", (v >> 50) % 9),
                    };
                    FolderEntry {
                        path: std::path::PathBuf::from(&name),
                        name,
                        ext: ext.to_lowercase(),
                        modified: None,
                        size: num % 4,
                        taken: String::new(),
                    }
                })
                .collect()
        };
        let i18n = specular_i18n::I18n::global();
        let loc = i18n.localizer();
        for order in [SortOrder::Natural, SortOrder::Type, SortOrder::Size] {
            let n = if order == SortOrder::Natural {
                100_000
            } else {
                20_000
            };
            let files = entries(n, &mut next);
            let mut old: Vec<String> = files.iter().map(|f| f.name.clone()).collect();
            let by_name: std::collections::HashMap<&str, (&str, u64)> = files
                .iter()
                .map(|f| (f.name.as_str(), (f.ext.as_str(), f.size)))
                .collect();
            let t0 = std::time::Instant::now();
            old.sort_by(|a, b| {
                let (ea, sa) = by_name[a.as_str()];
                let (eb, sb) = by_name[b.as_str()];
                let primary = match order {
                    SortOrder::Type => i18n.natural_cmp(ea, eb),
                    SortOrder::Size => sa.cmp(&sb),
                    _ => std::cmp::Ordering::Equal,
                };
                primary.then_with(|| natural_path_cmp(i18n, a, b))
            });
            let t_old = t0.elapsed();
            let t0 = std::time::Instant::now();
            let new: Vec<String> = sort_entries(&loc, files, order)
                .into_iter()
                .map(|f| f.name)
                .collect();
            let t_new = t0.elapsed();
            eprintln!("{order:?}, {n} names: collating {t_old:.1?}, keyed {t_new:.1?}");
            assert_eq!(new, old, "{order:?}");
        }
    }

    use super::{Event, Loader, OverviewKind, Unfinished, overview_plan};
    use specular_core::{CancelToken, Priority};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    /// 64×48 progressive JPEG.
    fn fixture() -> PathBuf {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/specular-metadata/tests/fixtures/ocean.jpg"
        ))
    }

    fn events(
        rx: &crossbeam_channel::Receiver<Event>,
        until: impl Fn(&Event) -> bool,
    ) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(e) = rx.recv_timeout(Duration::from_secs(10)) {
            let done = until(&e);
            out.push(e);
            if done {
                return out;
            }
        }
        panic!("timed out; got {} events", out.len());
    }

    /// The /tmp/2.jpeg spinner: the image leaves the prefetch window (its token is
    /// cancelled) while its open job is past opening. The job still sends the header, then
    /// stops. Each level it would have decoded must be reported unfinished so the app does
    /// not keep them marked as in flight.
    #[test]
    fn an_open_cancelled_after_the_header_reports_its_levels_unfinished() {
        let (loader, rx) = Loader::new(1);
        let token = CancelToken::new();
        let t = token.clone();
        // Cancel at the first event the job sends: the header.
        loader.set_wake(Arc::new(move || t.cancel()));
        let vp = Some((800, 600));
        loader.open(1, fixture(), Priority::Prefetch, vp, token.clone());
        let plan = overview_plan((64, 48), true, true, vp, true, 8);
        let mut left = plan.clone();
        let evs = events(
            &rx,
            |e| matches!(e, Event::Unfinished { job: Unfinished::Level(s), .. } if *s == *plan.last().unwrap()),
        );
        match &evs[0] {
            Event::Opened {
                id: 1, token: t, ..
            } => assert!(t.same(&token)),
            _ => panic!("the header comes first"),
        }
        for e in &evs[1..] {
            match e {
                Event::Unfinished {
                    id: 1,
                    token: t,
                    job: Unfinished::Level(s),
                    cancelled: true,
                } => {
                    assert!(t.same(&token));
                    left.retain(|l| l != s);
                }
                Event::Overview { .. } => panic!("a cancelled job decoded a level"),
                _ => panic!("unexpected event"),
            }
        }
        assert!(
            left.is_empty(),
            "levels {left:?} of {plan:?} were not reported"
        );
    }

    /// A decoder that panics on a file (the test hook trips on the name) fails that image
    /// with an internal error the viewer shows, never "unfinished" (which would make the app
    /// open it again, and panic again); the worker is still there and opens the next image.
    #[test]
    fn a_decoder_panic_fails_the_image_and_the_next_one_opens() {
        let dir = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/loader-panic"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join(format!(
            "ocean-{}.jpg",
            specular_core::isolate::TEST_PANIC_MARK
        ));
        std::fs::copy(fixture(), &bad).unwrap();
        let (loader, rx) = Loader::new(1);
        let vp = Some((800, 600));
        loader.open(1, bad, Priority::Immediate, vp, CancelToken::new());
        loader.open(2, fixture(), Priority::Visible, vp, CancelToken::new());
        let evs = events(&rx, |e| {
            matches!(
                e,
                Event::Overview {
                    id: 2,
                    kind: OverviewKind::Level(_),
                    ..
                }
            )
        });
        let failed: Vec<&String> = evs
            .iter()
            .filter_map(|e| match e {
                Event::Failed { id: 1, error } => Some(error),
                _ => None,
            })
            .collect();
        assert_eq!(failed.len(), 1);
        // What the viewer shows: "Can't open …: something went wrong inside Specular. The
        // log has the details." (the panic, its location and crate are in the log).
        assert_eq!(*failed[0], specular_i18n::t!("error-reason-internal"));
        assert!(!failed[0].contains("test panic"), "{}", failed[0]);
        assert!(
            !evs.iter()
                .any(|e| matches!(e, Event::Unfinished { id: 1, .. })),
            "a panicked open must not be reported unfinished"
        );
        assert!(evs.iter().any(|e| matches!(e, Event::Opened { id: 2, .. })));
        assert_eq!(
            loader.pool.panicked(),
            0,
            "caught inside the job, not by the pool"
        );
        assert_eq!(loader.pool.alive(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_open_cancelled_before_it_starts_reports_unfinished() {
        let (loader, rx) = Loader::new(1);
        let token = CancelToken::new();
        token.cancel();
        loader.open(2, fixture(), Priority::Prefetch, None, token.clone());
        match events(&rx, |_| true).pop().unwrap() {
            Event::Unfinished {
                id: 2,
                token: t,
                job: Unfinished::Open,
                cancelled: true,
            } => assert!(t.same(&token)),
            _ => panic!("expected Unfinished(Open)"),
        }
    }

    /// Opening one image with `SPECULAR_LOG=all` logs its way from request to pixels.
    #[test]
    fn opening_an_image_logs_its_state_sequence() {
        let Some(buf) = specular_log::capture("all") else {
            return;
        };
        let (loader, rx) = Loader::new(1);
        loader.open(
            7,
            fixture(),
            Priority::Immediate,
            Some((800, 600)),
            CancelToken::new(),
        );
        events(&rx, |e| {
            matches!(
                e,
                Event::Overview {
                    id: 7,
                    kind: OverviewKind::Level(1),
                    ..
                }
            )
        });
        // Let the job's last lines (finish) land.
        drop(loader);
        std::thread::sleep(Duration::from_millis(100));
        let text = String::from_utf8(buf.lock().clone()).unwrap();
        let lines: Vec<String> = text
            .lines()
            .filter(|l| {
                !l.contains("img=1 ") && !l.contains("img=2 ") && !l.contains("specular-list")
            })
            .map(|l| {
                // Drop time and thread; numbers that vary (ms, job numbers) become _.
                let cols: Vec<&str> = l.split_whitespace().skip(2).collect();
                let l = cols.join(" ");
                let re_ms = |s: &str| {
                    let w: Vec<String> = s
                        .split(' ')
                        .map(|t| {
                            if t.starts_with('#') && t[1..].chars().all(|c| c.is_ascii_digit()) {
                                "#_".to_owned()
                            } else if t.parse::<f64>().is_ok() && t.contains('.') {
                                "_".to_owned()
                            } else if t.contains('/') && t.ends_with("ocean.jpg:") {
                                "ocean.jpg:".to_owned()
                            } else {
                                t.to_owned()
                            }
                        })
                        .collect();
                    w.join(" ")
                };
                re_ms(&l)
            })
            .collect();
        let expected = [
            "jobs submit #_ img=7 ocean.jpg open",
            "jobs start #_ img=7 ocean.jpg open",
            "io open ocean.jpg:",
            "decode img=7 ocean.jpg opened:",
            "decode JPEG decoder: 64×48 progressive 4:2:0 8-bit",
            "decode img=7 ocean.jpg plan: levels [1]",
            "decode jpeg: decode 1/1 whole image",
            "decode img=7 level 1/1 decoded: 64×48",
            "jobs finish #_ img=7 ocean.jpg open done",
        ];
        let mut at = 0;
        for want in expected {
            match lines[at..].iter().position(|l| l.starts_with(want)) {
                Some(i) => at += i + 1,
                None => panic!("missing {want:?} after line {at} in:\n{}", lines.join("\n")),
            }
        }
    }

    /// A Motion Photo (clip appended to the JPEG, described by its XMP) and a Live Photo (a
    /// sibling movie) are found by the metadata job, with the info panel's "Motion" row.
    /// Uses the generated reference clips (tools/corpus/video-refs.sh); skips without them.
    #[test]
    fn motion_photos_are_found_with_the_metadata() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let clip = root.join("tools/corpus/out/video-ref/clips/hevc-main.mp4");
        let Ok(mp4) = std::fs::read(&clip) else {
            eprintln!("skipped: run tools/corpus/video-refs.sh");
            return;
        };
        let jpeg =
            std::fs::read(root.join("crates/specular-metadata/tests/fixtures/ocean.jpg")).unwrap();
        let dir = root.join("target/test-tmp/motion-metadata");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let loc = specular_metadata::Localizer::new("en".parse().unwrap());
        let probe = |path: &std::path::Path| {
            let s = specular_decode::Registry::default()
                .open_path(path)
                .unwrap();
            let (report, motion) = super::metadata_report(s.as_ref(), &loc);
            let row = report
                .summary
                .iter()
                .find(|g| g.key == "image")
                .and_then(|g| g.items.iter().find(|i| i.key == "motion"))
                .map(|i| i.value.replace(['\u{2068}', '\u{2069}'], ""));
            (motion, row)
        };

        // Motion Photo: an XMP APP1 after the SOI, the clip after the EOI.
        let xmp = format!(
            r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF><rdf:Description GCamera:MotionPhoto="1"><Container:Directory><rdf:Seq><rdf:li rdf:parseType="Resource"><Container:Item Item:Semantic="Primary" Item:Mime="image/jpeg"/></rdf:li><rdf:li rdf:parseType="Resource"><Container:Item Item:Semantic="MotionPhoto" Item:Mime="video/mp4" Item:Length="{}"/></rdf:li></rdf:Seq></Container:Directory></rdf:Description></rdf:RDF></x:xmpmeta>"#,
            mp4.len()
        );
        let sig = b"http://ns.adobe.com/xap/1.0/\0";
        let mut f = jpeg[..2].to_vec();
        f.extend([0xff, 0xe1]);
        f.extend(((2 + sig.len() + xmp.len()) as u16).to_be_bytes());
        f.extend(sig);
        f.extend(xmp.as_bytes());
        f.extend(&jpeg[2..]);
        f.extend(&mp4);
        let mp = dir.join("PXL_0001.MP.jpg");
        std::fs::write(&mp, &f).unwrap();
        let (m, row) = probe(&mp);
        let m = m.expect("motion photo");
        assert_eq!(m.kind, specular_video::Kind::MotionPhoto);
        assert_eq!(
            m.source,
            specular_video::Source::Embedded(f.len() - mp4.len()..f.len())
        );
        assert_eq!(row.as_deref(), Some("Motion Photo · 1.5 s · HEVC"));

        // Live Photo: the plain still and a movie of the same name.
        let still = dir.join("IMG_0001.JPG");
        std::fs::write(&still, &jpeg).unwrap();
        std::fs::write(dir.join("IMG_0001.MOV"), &mp4).unwrap();
        let (m, row) = probe(&still);
        assert_eq!(m.map(|m| m.kind), Some(specular_video::Kind::LivePhoto));
        assert_eq!(row.as_deref(), Some("Live Photo · 1.5 s · HEVC"));

        // The plain still alone: nothing.
        std::fs::remove_file(dir.join("IMG_0001.MOV")).unwrap();
        assert_eq!(probe(&still), (None, None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Vector data drawn at the view's size has no Image group: its raster is a canvas, so
    /// "4096 × 3286 (13.5 MP), 8-bit" would describe nothing in the file. Its own group does.
    #[test]
    fn vector_files_have_no_image_group() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = root.join("target/test-tmp/vector-info");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let geojson = dir.join("parks.geojson");
        std::fs::write(
            &geojson,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"a"},"geometry":{"type":"Polygon","coordinates":[[[4.0,52.0],[4.1,52.0],[4.1,52.1],[4.0,52.0]]]}},{"type":"Feature","properties":{"name":"b"},"geometry":{"type":"Point","coordinates":[4.05,52.05]}}]}"#,
        )
        .unwrap();
        let loc = specular_metadata::Localizer::new("en".parse().unwrap());
        let crates = root.join("crates");
        let mut wrong = Vec::new();
        for (path, own) in [
            (geojson.clone(), "geospatial"),
            (
                crates.join("formats/specular-cad/tests/data/entities.dxf"),
                "drawing",
            ),
            (
                crates.join("formats/specular-neuro/tests/data/v1.trk"),
                "tractography",
            ),
            (
                crates.join("formats/specular-mol/tests/data/caffeine.xyz"),
                "structure",
            ),
            (
                crates.join("formats/specular-points/tests/data/p1.las"),
                "points",
            ),
        ] {
            let s = specular_decode::Registry::default()
                .open_path(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let (report, _) = super::metadata_report(s.as_ref(), &loc);
            let keys: Vec<&str> = report.summary.iter().map(|g| g.key).collect();
            if keys.contains(&"image") || !keys.contains(&own) {
                wrong.push(format!("{}: {keys:?}", path.display()));
            }
        }
        // A raster keeps its Image group.
        let s = specular_decode::Registry::default()
            .open_path(&crates.join("specular-metadata/tests/fixtures/ocean.jpg"))
            .unwrap();
        let (report, _) = super::metadata_report(s.as_ref(), &loc);
        assert!(report.summary.iter().any(|g| g.key == "image"));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    /// A slide's Image group gives the file's sample depth, not the 8-bit composite's.
    #[test]
    fn slide_sample_depth_from_its_coding() {
        for (coding, bits) in [
            ("uint16, uncompressed", Some(16)),
            ("uint8 RGB, JPEG, 3 levels", Some(8)),
            ("float, LZW", Some(32)),
            ("16-bit grey, JPEG 2000 Lossless", Some(16)),
            ("8-bit RGB, JPEG", Some(8)),
            ("32-bit float, Zarr v3", Some(32)),
            ("tiled 256×256, LZW, MinIsBlack 12-bit", Some(12)),
            ("bit, uncompressed", Some(1)),
            ("Zarr", None),
        ] {
            assert_eq!(super::slide_sample_bits(coding), bits, "{coding}");
        }
    }

    /// The info panel's encoding row per format family: the colour model from the session (or
    /// the JPEG scan), subsampling for YCbCr, baseline/progressive for JPEG only.
    /// The thermal info group's temperatures follow the viewer's unit (they were always °C).
    #[test]
    fn thermal_info_follows_the_temperature_unit() {
        use specular_decode::science::TemperatureUnit;
        let loc = specular_metadata::Localizer::new("en".parse().unwrap());
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/formats/specular-fmt-thermal/tests/data/rjpeg.jpg");
        let s = specular_decode::Registry::default()
            .open_path(&path)
            .unwrap();
        let rows = |unit| {
            let (report, _) = super::metadata_report_in(s.as_ref(), &loc, unit);
            let g = report
                .summary
                .iter()
                .find(|g| g.key == "science")
                .unwrap_or_else(|| {
                    panic!(
                        "no thermal group: {:?}",
                        report.summary.iter().map(|g| &g.key).collect::<Vec<_>>()
                    )
                });
            let row = |k: &str| {
                g.items
                    .iter()
                    .find(|i| i.key == k)
                    .map(|i| i.value.replace(['\u{2068}', '\u{2069}'], ""))
            };
            (row("temperature_range").expect("range"), row("reflected"))
        };
        let (c, rc) = rows(TemperatureUnit::Celsius);
        let (f, rf) = rows(TemperatureUnit::Fahrenheit);
        let (k, _) = rows(TemperatureUnit::Kelvin);
        assert!(c.contains("°C") && !c.contains("°F"), "{c}");
        assert!(f.contains("°F") && !f.contains("°C"), "{f}");
        assert!(k.contains(" K") && !k.contains('°'), "{k}");
        if let (Some(rc), Some(rf)) = (rc, rf) {
            assert!(rc.contains("°C") && rf.contains("°F"), "{rc} / {rf}");
            let num = |t: &str| -> f64 {
                t.split_whitespace()
                    .next()
                    .unwrap()
                    .replace('−', "-")
                    .parse()
                    .unwrap()
            };
            assert!(
                (num(&rc) * 1.8 + 32.0 - num(&rf)).abs() < 0.02,
                "{rc} / {rf}"
            );
        }
    }

    #[test]
    fn encoding_row_per_format_family() {
        let loc = specular_metadata::Localizer::new("en".parse().unwrap());
        let encoding = |rel: &str| {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../crates")
                .join(rel);
            let s = specular_decode::Registry::default()
                .open_path(&path)
                .unwrap();
            let (report, _) = super::metadata_report(s.as_ref(), &loc);
            report
                .summary
                .iter()
                .find(|g| g.key == "image")
                .and_then(|g| g.items.iter().find(|i| i.key == "encoding"))
                .map(|i| i.value.clone())
                .unwrap_or_else(|| panic!("{rel}: no encoding row"))
        };
        let mut wrong = Vec::new();
        for (rel, want) in [
            (
                "specular-metadata/tests/fixtures/ocean.jpg",
                "8-bit YCbCr 4:2:0, progressive",
            ),
            (
                "formats/specular-jpeg/tests/fixtures/hier/grey_y2_prog.jpg",
                "8-bit Greyscale, hierarchical",
            ),
            (
                "specular-decode/tests/alpha/rgba8.png",
                "8-bit RGB with alpha",
            ),
            (
                "specular-decode/tests/alpha/rgba8-assoc.tif",
                "8-bit RGB with premultiplied alpha",
            ),
            (
                "specular-decode/tests/alpha/index.gif",
                "4-bit Indexed colour with transparency",
            ),
            (
                "specular-decode/tests/alpha/dxt2.dds",
                "8-bit RGB with premultiplied alpha",
            ),
            (
                "specular-decode/tests/alpha/half.exr",
                "16-bit RGB with premultiplied alpha",
            ),
            (
                "specular-decode/tests/alpha/bgr24-mask.ico",
                "8-bit RGB with transparency",
            ),
            (
                "specular-decode/tests/alpha/palette-trns.png",
                "8-bit Indexed colour with transparency",
            ),
            (
                "specular-decode/tests/alpha/lossy.webp",
                "8-bit YCbCr 4:2:0 with alpha",
            ),
            (
                "formats/specular-heic/fuzz/seeds/c422_10.heic",
                "10-bit YCbCr 4:2:2",
            ),
            (
                "formats/specular-avif/tests/data/gray.avif",
                "8-bit Greyscale",
            ),
            ("formats/specular-jxl/tests/data/vardct.jxl", "8-bit XYB"),
            (
                "formats/specular-jp2/tests/data/palette.jp2",
                "8-bit Indexed colour",
            ),
            ("formats/specular-psd/fuzz/seeds/lab8.psd", "8-bit L*a*b*"),
            (
                "formats/specular-psd/fuzz/seeds/duotone.psd",
                "8-bit Duotone",
            ),
            (
                "specular-decode/tests/alpha/argb1555.tga",
                "5-bit RGB with alpha",
            ),
            (
                "formats/specular-legacy/fuzz/seeds/pal.pcx",
                "8-bit Indexed colour",
            ),
            (
                "formats/specular-exr/fuzz/seeds/ycc.exr",
                "16-bit Luminance/chroma (YC) with premultiplied alpha",
            ),
            (
                "formats/specular-radiance/fuzz/seeds/rle.hdr",
                "32-bit RGBE (shared exponent)",
            ),
            (
                "formats/specular-texture/fuzz/seeds/bc5.dds",
                "8-bit Two channels (RG)",
            ),
        ] {
            // Fluent wraps placeables in bidi isolates; compare the visible text.
            let got = encoding(rel).replace(['\u{2068}', '\u{2069}'], "");
            if got != want {
                wrong.push(format!("{rel}: got {got:?}, want {want:?}"));
            }
        }
        assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
    }

    /// `DUMP_FILES=@list.txt cargo test -p specular --bin specular dump_info -- --ignored
    /// --nocapture` prints each file's info panel report as the app builds it.
    #[test]
    #[ignore]
    fn dump_info() {
        specular_video::install();
        let loc = specular_metadata::Localizer::new("en".parse().unwrap());
        let mut files = std::env::var("DUMP_FILES").unwrap_or_default();
        if let Some(list) = files.strip_prefix('@') {
            files = std::fs::read_to_string(list).unwrap().replace('\n', ":");
        }
        for p in files.split(':').filter(|s| !s.is_empty()) {
            let s = match specular_decode::Registry::default().open_path(std::path::Path::new(p)) {
                Ok(s) => s,
                Err(e) => {
                    println!("===== {p}: {e}");
                    continue;
                }
            };
            let (report, _) = super::metadata_report(s.as_ref(), &loc);
            println!("===== {p}: {:?}", s.info().format);
            for g in &report.summary {
                println!("[{}]", specular_metadata::strip_isolates(&g.title));
                for i in &g.items {
                    println!(
                        "  {}: {}",
                        specular_metadata::strip_isolates(&i.label),
                        specular_metadata::strip_isolates(&i.value)
                    );
                }
            }
            if let Some(v) = s.video() {
                println!("  video: {v:?}");
            }
        }
    }
}
