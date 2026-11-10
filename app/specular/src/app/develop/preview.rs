//! The developed picture: the GPU engine (`specular_develop::preview`) on the app's device.
//!
//! - **Source.** A raw's sensor data is read in the background and developed on the GPU by
//!   the raw front end; other images are decoded in the background, turned to linear
//!   Rec.2020 and reduced to about screen size, entering at the scene-linear stage.
//! - **Render.** Each frame with a changed edit (or turn, or size) renders the edit with the
//!   output transform into the display storage space and writes an `Rgba8UnormSrgb`
//!   texture on the GPU. Only when the GPU has finished the last picture is the next one
//!   submitted, so the newest change wins and nothing queues up.
//! - **HDR.** On an HDR (scRGB) swapchain the edit renders for the display's headroom
//!   capped by its HDR limit (`specular_develop::ops::view::headroom`) into an
//!   `Rgba16Float` texture, values above 1 brighter than SDR white; the SDR preview (or an
//!   SDR display) renders headroom 1, which is exactly the SDR export.
//! - **No usable GPU**: when the app's device fails, the same code runs on the
//!   software adapter and the picture is read back and uploaded.

use std::path::{Path, PathBuf};

use crossbeam_channel::{Receiver, Sender, unbounded};
use specular_core::CancelToken;
use specular_develop::drag::{self, Level, Pace};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::{Edit, ModuleKind};
use specular_develop::ops::correct::FlatField;
use specular_develop::ops::correct::calib::{self, Calibration};
use specular_develop::ops::geometry::lensfun::Shot;
use specular_develop::ops::output::OutputSpace;
use specular_develop::ops::retouch::{self, Retouch};
use specular_develop::ops::view;
use specular_develop::preview::{self as engine, Developer, Input};
use specular_gfx::{Gpu, TexId, UiRenderer, wgpu};
use std::sync::Arc;

pub(in crate::app) use specular_develop::preview::Orient;

/// Long side of a non-raw image's developed copy (px).
pub(super) const SIDE: u32 = 2560;
/// Largest long side developed for the screen (px).
const MAX_SIDE: u32 = 4096;
/// The long side a chart photo is developed at for measuring (px, at least).
const CHART_LONG: u32 = 2048;

/// The calibration a raw is developed with: the flat-field module's params and the
/// calibration folder.
type CalibKey = (Option<FlatField>, Option<PathBuf>);

/// What the background reader sends.
enum Msg {
    Source(PathBuf, Result<SourceRead, String>),
    /// The next photo read ahead (its tag).
    Next(PathBuf, u64, Result<SourceRead, String>),
    /// Flat and dark frames loaded for a key.
    Calibration(CalibKey, Result<Option<Arc<Calibration>>, String>),
}

/// What the darkroom aids ask of the next render (`aids.rs`).
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct AidsReq {
    /// Bin the scopes.
    pub scopes: bool,
    pub clip: specular_develop::clip::ClipFlags,
    /// Soft-proof through this table.
    pub proof: Option<Arc<specular_develop::proof::ProofLut>>,
}

impl PartialEq for AidsReq {
    fn eq(&self, o: &AidsReq) -> bool {
        self.scopes == o.scopes
            && self.clip == o.clip
            && self.proof.as_ref().map(|l| l.key) == o.proof.as_ref().map(|l| l.key)
    }
}

/// The uploaded picture.
struct Tex {
    tex: wgpu::Texture,
    id: TexId,
    orient: Orient,
    crop_on: bool,
}

/// The next photo: read in the background, then developed ahead on the engine
/// while the GPU is free, so opening it costs a render.
struct Next {
    path: PathBuf,
    tag: u64,
    /// The edit it opens with (its front end and corrections decide the source).
    edit: Edit,
    /// Read, waiting for the GPU.
    read: Option<SourceRead>,
    /// On the engine: what the photo needs besides its pixels.
    staged: Option<SourceRead>,
    /// Could not be read or kept ahead: read when opened.
    failed: bool,
}

/// Where the engine runs.
enum Engine {
    /// On the app's device: written straight into the UI's texture.
    App(Box<Developer>),
    /// On the software adapter: read back and uploaded.
    Software(Box<Developer>),
}

/// The engine's state for the image open in Develop.
pub(in crate::app) struct Preview {
    path: Option<PathBuf>,
    /// The image read, waiting for the GPU.
    input: Option<Input>,
    /// The as-shot white of a raw, once (for the session's edit).
    as_shot: Option<[f64; 2]>,
    /// The image's aperture and pixel pitch (set on the engine with the input).
    lens: (Option<f32>, Option<f32>),
    /// A raw's Lensfun lookup (set on the engine with the input), and the same for the
    /// session until taken.
    lensfun: Option<Arc<Shot>>,
    lensfun_new: Option<Option<Arc<Shot>>>,
    /// The file's 35 mm equivalent focal length (set on the engine with the input).
    focal35: Option<f64>,
    /// Something to say about the source, once.
    note: Option<specular_develop::source::Note>,
    engine: Option<Engine>,
    has_source: bool,
    pub error: Option<String>,
    edit: Option<Edit>,
    orient: Orient,
    long_side: u32,
    /// Apply the crop (off while the crop tool is open).
    crop_on: bool,
    /// The display peak to render for (units of SDR white; 1 = SDR).
    peak: f32,
    /// A render is wanted for the newest request.
    dirty: bool,
    /// Since when a picture with older or neutral highlight statistics is held back.
    held: Option<std::time::Instant>,
    /// The source must be read again (the engine moved to the software adapter).
    reread: bool,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    tex: Option<Tex>,
    retired: Vec<TexId>,
    /// A point to pick a colour at: fractions of the shown picture's width and height.
    pick: Option<(f32, f32)>,
    /// The picked colour (linear Rec.2020 at the input of `pick_kind`), or why not.
    picked: Option<Result<[f64; 3], String>>,
    /// The tone equalizer's mask percentiles with a pick before it.
    picked_pct: Option<[f32; 2]>,
    /// A second picture to compare with (a snapshot), made after the first.
    other: Option<Edit>,
    other_dirty: bool,
    other_tex: Option<Tex>,
    /// A raw's sensor without its samples (for building calibrations off the UI thread).
    sensor: Option<calib::SensorData>,
    /// The calibration asked for, and the one loaded and waiting for the engine.
    calib_key: Option<CalibKey>,
    calib_ready: Option<Result<Option<Arc<Calibration>>, String>>,
    /// Why the calibration frames can't be used (said once).
    pub notice: Option<String>,
    /// The module whose input a pick samples (point colour, or a masked module).
    pick_kind: ModuleKind,
    /// Show this module's mask over the picture.
    overlay: Option<((ModuleKind, u16), specular_develop::mask::Overlay)>,
    /// The darkroom aids asked for, the clipping overlay made with the picture, and the
    /// newest scope bins.
    aids: AidsReq,
    clip_tex: Option<Tex>,
    bins: Option<specular_develop::scopes::Bins>,
    /// A retouch spot whose source to find, and the answer: the spot and the
    /// source (frame fractions), `None` when nothing fits.
    find: Option<(Retouch, usize)>,
    /// A retouch spot is being dragged: renders are drafts, refined after.
    draft: bool,
    /// A slider of this heavy module is being dragged: its renders are drafts.
    draft_module: Option<ModuleKind>,
    found: Option<(usize, Option<[f32; 2]>)>,
    /// The picture before retouch, for the search: its key, size and pixels.
    scene: Option<(String, u32, u32, Vec<f32>)>,
    /// AI masks: the picture the networks see is wanted, and the answer.
    ai_want: bool,
    ai_pic: Option<Result<super::ai_masks::AiPicture, String>>,
    /// Automatic upright asked for, and its lines once read.
    upright_want: Option<specular_develop::ops::geometry::persp::Mode>,
    upright: Option<(
        specular_develop::ops::geometry::persp::Mode,
        Result<specular_develop::preview::UprightLines, String>,
    )>,
    /// A chart profile (Phase 3): the chart edit to develop the photo with, and the answer.
    chart_want: Option<Edit>,
    chart_pic: Option<Result<super::chart_profile::Shot, String>>,
    /// The raw's noise model, fitted by the reader (set on the engine with the input).
    noise: Option<specular_develop::ops::detail::RawNoise>,
    /// The next photo, read and developed ahead.
    next: Option<Next>,
    /// Tags of photos read ahead.
    tags: u64,
    /// The open photo was developed ahead: the engine's tag to take.
    take: Option<u64>,
    /// The picture shown is a draft: a refined one follows.
    shown_draft: bool,
    /// "Preview while dragging": the level a slider's drafts are drawn at.
    pace: Pace,
    /// When the edit last changed (a resting slider at [`Level::Pause`] draws).
    changed_at: Option<std::time::Instant>,
    /// A slider's draft is being timed: the divisor it was drawn at.
    timing: Option<u32>,
    /// Slider drafts timed so far, and the last one's divisor and milliseconds (the
    /// real-use runs' `ui-state`).
    timed: (u64, Option<(u32, f64)>),
}

impl std::fmt::Debug for Preview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preview")
            .field("path", &self.path)
            .field("dirty", &self.dirty)
            .finish_non_exhaustive()
    }
}

impl Default for Preview {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Preview {
            path: None,
            input: None,
            as_shot: None,
            lens: (None, None),
            lensfun: None,
            lensfun_new: None,
            focal35: None,
            note: None,
            engine: None,
            has_source: false,
            error: None,
            edit: None,
            orient: Orient::default(),
            long_side: 0,
            crop_on: true,
            peak: 1.0,
            dirty: false,
            held: None,
            reread: false,
            tx,
            rx,
            tex: None,
            retired: Vec::new(),
            pick: None,
            picked: None,
            picked_pct: None,
            other: None,
            other_dirty: false,
            other_tex: None,
            sensor: None,
            calib_key: None,
            calib_ready: None,
            notice: None,
            pick_kind: ModuleKind::POINT_COLOUR,
            overlay: None,
            aids: AidsReq::default(),
            clip_tex: None,
            bins: None,
            find: None,
            draft: false,
            draft_module: None,
            found: None,
            scene: None,
            ai_want: false,
            ai_pic: None,
            upright_want: None,
            upright: None,
            chart_want: None,
            chart_pic: None,
            noise: None,
            next: None,
            tags: 0,
            take: None,
            shown_draft: false,
            pace: Pace::default(),
            changed_at: None,
            timing: None,
            timed: (0, None),
        }
    }
}

impl Preview {
    /// Starts on `path`: it is read in the background.
    pub fn open(&mut self, path: &Path, loader: &Loader) {
        if self.path.as_deref() == Some(path) {
            return;
        }
        let next = self.next.take().filter(|n| n.path == path);
        self.close_keeping(next.as_ref().is_some_and(|n| n.staged.is_some()));
        self.path = Some(path.to_path_buf());
        match next {
            // Developed ahead: the engine has it.
            Some(Next {
                staged: Some(r),
                tag,
                ..
            }) => {
                self.accept(r);
                // Its pixels are on the engine.
                self.input = None;
                self.take = Some(tag);
            }
            // Read ahead.
            Some(Next {
                read: Some(r),
                failed: false,
                ..
            }) => self.accept(r),
            // Still being read: its message is taken as this photo's (`poll`).
            Some(n) if !n.failed => {}
            _ => self.read(loader),
        }
    }

    fn read(&self, loader: &Loader) {
        let Some(p) = self.path.clone() else { return };
        let registry = loader.registry();
        let tx = self.tx.clone();
        loader.background("develop-src", move |l| {
            let r = read_with_noise(&registry, &p);
            let _ = tx.send(Msg::Source(p, r));
            l.waker()();
        });
    }

    /// The photo to read and develop ahead with the edit it opens with: the one
    /// the user is likely to open next. `None`, or another photo, lets the last one go.
    pub fn want_next(&mut self, next: Option<(&Path, Edit)>, loader: &Loader) {
        let same = match (&self.next, &next) {
            (Some(n), Some((p, _))) => n.path == *p,
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.drop_next();
        let Some((p, edit)) = next else { return };
        if self.path.as_deref() == Some(p) {
            return;
        }
        self.tags += 1;
        let tag = self.tags;
        let p = p.to_path_buf();
        self.next = Some(Next {
            path: p.clone(),
            tag,
            edit,
            read: None,
            staged: None,
            failed: false,
        });
        let registry = loader.registry();
        let tx = self.tx.clone();
        loader.background("develop-next", move |l| {
            let r = read_with_noise(&registry, &p);
            let _ = tx.send(Msg::Next(p, tag, r));
            l.waker()();
        });
    }

    /// The photo read or developed ahead.
    pub fn next_path(&self) -> Option<&Path> {
        self.next.as_ref().map(|n| n.path.as_path())
    }

    /// Whether the next photo is on the engine, developed ahead.
    #[cfg(test)]
    pub fn next_staged(&self) -> bool {
        self.next.as_ref().is_some_and(|n| n.staged.is_some())
    }

    /// Lets the photo read or developed ahead go.
    fn drop_next(&mut self) {
        self.next = None;
        if let Some(e) = self.engine.as_mut() {
            dev_mut(e).drop_prefetched();
        }
    }

    /// Takes a photo read (here or ahead) as the open one.
    fn accept(&mut self, r: SourceRead) {
        let SourceRead {
            input,
            white,
            lens,
            note,
            noise,
            lensfun,
            focal35,
        } = r;
        self.lens = lens;
        self.focal35 = focal35;
        self.lensfun_new = Some(lensfun.clone());
        self.lensfun = lensfun;
        self.note = note;
        self.noise = noise;
        self.sensor = match &input {
            Input::Raw(s) => Some(calib::SensorData {
                samples: Vec::new(),
                float: None,
                meta: s.meta.clone(),
                ..**s
            }),
            Input::Rgb { .. } => None,
        };
        self.calib_key = None;
        self.input = Some(input);
        self.as_shot = white;
        self.dirty = true;
    }

    /// Develops the next photo ahead while nothing else waits for the GPU.
    fn ahead(&mut self) -> Result<(), specular_develop::gpu::Error> {
        let Some(e) = self.engine.as_mut() else {
            return Ok(());
        };
        if !self.has_source
            || self.dirty
            || self.other_dirty
            || self.held.is_some()
            || self.long_side == 0
            || !dev(e).idle()
        {
            return Ok(());
        }
        let d = dev_mut(e);
        let Some(n) = self.next.as_mut() else {
            return Ok(());
        };
        if n.failed {
            return Ok(());
        }
        if n.staged.is_some() {
            d.refine_prefetched()?;
            return Ok(());
        }
        if n.read.is_none() || d.refining() {
            return Ok(());
        }
        let Some(mut r) = n.read.take() else {
            return Ok(());
        };
        let input = std::mem::replace(
            &mut r.input,
            Input::Rgb {
                width: 0,
                height: 0,
                rgba: Vec::new(),
            },
        );
        if d.prefetch(
            n.tag,
            input,
            r.noise,
            &n.edit,
            self.long_side,
            Orient::default(),
        )? {
            n.staged = Some(r);
        } else {
            // Over the memory budget: it is read again when opened.
            n.failed = true;
        }
        Ok(())
    }

    /// Forgets the image (the texture goes at the next upload).
    pub fn close(&mut self) {
        self.close_keeping(false);
    }

    /// [`Preview::close`]; `ahead`: the engine keeps the photo developed ahead (it opens).
    fn close_keeping(&mut self, ahead: bool) {
        if ahead {
            self.next = None;
        } else {
            self.drop_next();
        }
        self.take = None;
        self.noise = None;
        self.shown_draft = false;
        self.path = None;
        self.input = None;
        self.as_shot = None;
        self.lens = (None, None);
        self.lensfun = None;
        self.lensfun_new = Some(None);
        self.focal35 = None;
        self.note = None;
        self.has_source = false;
        self.error = None;
        self.edit = None;
        self.dirty = false;
        self.reread = false;
        self.sensor = None;
        self.calib_key = None;
        self.calib_ready = None;
        if let Some(t) = self.tex.take() {
            self.retired.push(t.id);
        }
        if let Some(t) = self.clip_tex.take() {
            self.retired.push(t.id);
        }
        self.bins = None;
        self.set_other(None);
    }

    /// What the darkroom aids ask of the renders; a change renders again.
    pub fn set_aids(&mut self, a: AidsReq) {
        if a != self.aids {
            let proof = a.proof.as_ref().map(|l| l.key) != self.aids.proof.as_ref().map(|l| l.key);
            self.aids = a;
            self.dirty = true;
            self.other_dirty |= proof && self.other.is_some();
        }
    }

    /// The newest scope bins, once.
    pub fn take_bins(&mut self) -> Option<specular_develop::scopes::Bins> {
        self.bins.take()
    }

    /// The clipping overlay for the picture turned as `orient` and cropped as `crop_on`.
    pub fn clip_texture(&self, orient: Orient, crop_on: bool) -> Option<TexId> {
        if !self.aids.clip.any() {
            return None;
        }
        self.clip_tex
            .as_ref()
            .filter(|t| t.orient == orient && t.crop_on == crop_on)
            .map(|t| t.id)
    }

    /// The edit to show beside the picture (a snapshot), or none.
    pub fn set_other(&mut self, edit: Option<&Edit>) {
        if self.other.as_ref() == edit {
            return;
        }
        self.other = edit.cloned();
        self.other_dirty = edit.is_some();
        if edit.is_none()
            && let Some(t) = self.other_tex.take()
        {
            self.retired.push(t.id);
        }
    }

    /// The compared picture, when it is made for `orient` and `crop_on`.
    pub fn other_texture(&self, orient: Orient, crop_on: bool) -> Option<TexId> {
        self.other_tex
            .as_ref()
            .filter(|t| t.orient == orient && t.crop_on == crop_on && self.other.is_some())
            .map(|t| t.id)
    }

    /// Whether the image is still being read.
    pub fn reading(&self) -> bool {
        self.path.is_some()
            && !self.has_source
            && self.input.is_none()
            && self.take.is_none()
            && self.error.is_none()
    }

    /// Whether the picture shown is a quick draft whose refined version is on its way
    /// ("Refining…" shows quietly).
    pub fn refining_shown(&self) -> bool {
        self.error.is_none() && self.tex.is_some() && self.shown_draft
    }

    /// Whether a picture is being made or waits to be made.
    pub fn busy(&self) -> bool {
        self.error.is_none()
            && (self.dirty
                || self.other_dirty
                || self.find.is_some()
                || self.reread
                || self.input.is_some()
                || self.take.is_some()
                || self
                    .engine
                    .as_ref()
                    .is_some_and(|e| !dev(e).idle() || dev(e).refining()))
    }

    /// The as-shot white of the raw just read (taken once).
    /// Picks the colour at fraction `(u, v)` of the shown picture (at the next frame).
    pub fn pick_at(&mut self, u: f32, v: f32) {
        self.pick = Some((u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)));
    }

    /// The next pick samples the input of `kind`.
    pub fn pick_before(&mut self, kind: ModuleKind) {
        self.pick_kind = kind;
    }

    /// Shows a module's mask over the picture (`None`: no overlay).
    pub fn set_overlay(&mut self, o: Option<((ModuleKind, u16), specular_develop::mask::Overlay)>) {
        if o != self.overlay {
            self.overlay = o;
            self.dirty = true;
        }
    }

    /// The warp of the picture shown (for mask handles), once the image is developed.
    pub fn plan(&self) -> Option<specular_develop::ops::geometry::Plan> {
        let edit = self.edit.as_ref()?;
        let d = dev(self.engine.as_ref()?);
        d.plan(edit)
    }

    /// Finds a source for spot `i` of `r` when the GPU is free ([`Preview::take_found`]).
    pub fn find_source(&mut self, r: Retouch, i: usize) {
        self.find = Some((r, i));
    }

    /// Draft renders while a retouch spot is dragged; turning it off renders refined.
    pub fn set_draft(&mut self, on: bool) {
        if self.draft != on {
            self.draft = on;
            self.dirty |= !on;
        }
    }

    /// Draft renders while a slider of `kind` is dragged (`None` when it is let go: the
    /// next render refines).
    pub fn set_draft_module(&mut self, kind: Option<ModuleKind>) {
        if self.draft_module != kind {
            self.dirty |= kind.is_none();
            self.draft_module = kind;
        }
        if kind.is_none() {
            self.pace.end();
            self.timing = None;
        }
    }

    /// The module whose slider is being dragged.
    pub fn draft_module(&self) -> Option<ModuleKind> {
        self.draft_module
    }

    /// "Preview while dragging" (Settings › Performance).
    pub fn set_drag_mode(&mut self, mode: drag::Mode) {
        self.pace.mode = mode;
    }

    /// "Preview while dragging" as it stands: the setting, the level, the frame budget, the
    /// slider drafts timed and the last one's divisor and milliseconds (debug builds'
    /// `ui-state`).
    #[cfg(debug_assertions)]
    pub fn drag_state(&self) -> serde_json::Value {
        serde_json::json!({
            "mode": format!("{:?}", self.pace.mode),
            "dragging": self.pace.dragging(),
            "level": format!("{:?}", self.pace.level()),
            "budget_ms": self.pace.budget_ms(),
            "timed": self.timed.0,
            "last": self.timed.1.map(|(d, ms)| serde_json::json!({"divisor": d, "ms": ms})),
        })
    }

    /// The display's refresh rate (Hz), when known: a slider's frame budget.
    pub fn set_refresh(&mut self, hz: Option<f64>) {
        self.pace.set_refresh(hz);
    }

    /// Reads the picture AI masks are computed on when the GPU is free
    /// ([`Preview::take_ai_picture`]).
    pub fn want_ai_picture(&mut self) {
        self.ai_want = true;
    }

    /// Reads the lines automatic upright `mode` solves from when the GPU is free
    /// ([`Preview::take_upright`]).
    pub fn want_upright(&mut self, mode: specular_develop::ops::geometry::persp::Mode) {
        self.upright_want = Some(mode);
    }

    /// The lines for automatic upright, once.
    pub fn take_upright(
        &mut self,
    ) -> Option<(
        specular_develop::ops::geometry::persp::Mode,
        Result<specular_develop::preview::UprightLines, String>,
    )> {
        self.upright.take()
    }

    fn upright_now(&mut self) -> Result<(), specular_develop::gpu::Error> {
        let Some(mode) = self.upright_want else {
            return Ok(());
        };
        let (Some(engine), Some(edit)) = (self.engine.as_mut(), self.edit.as_ref()) else {
            return Ok(());
        };
        if !self.has_source || !dev(engine).idle() {
            return Ok(());
        }
        self.upright_want = None;
        let d = dev_mut(engine);
        d.base = self.orient;
        d.crop_on = self.crop_on;
        d.prepare(edit, self.long_side)?;
        self.upright = Some((mode, d.upright_lines(edit).map_err(|e| e.to_string())));
        Ok(())
    }

    /// The picture for AI masks, once.
    pub fn take_ai_picture(&mut self) -> Option<Result<super::ai_masks::AiPicture, String>> {
        self.ai_pic.take()
    }

    /// The system is short of memory: the engine gives back what only speeds up the next
    /// render.
    pub fn trim_memory(&mut self) {
        self.drop_next();
        if let Some(Engine::App(d) | Engine::Software(d)) = self.engine.as_mut() {
            d.trim_memory();
        }
    }

    /// The app opened a new GPU device after a driver reset: everything made on the old one
    /// is dropped (its textures went with the old UI renderer) and the picture is read and
    /// developed again on the new one.
    pub fn gpu_reset(&mut self) {
        self.next = None;
        self.take = None;
        self.engine = None;
        self.tex = None;
        self.other_tex = None;
        self.clip_tex = None;
        self.retired.clear();
        self.error = None;
        self.has_source = false;
        self.reread = true;
        self.dirty = true;
    }

    /// Renders again (an AI mask arrived).
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Develops the photo with `chart_edit` (the camera matrix only) when the GPU is free and
    /// reads it back for a chart profile ([`Preview::take_chart_picture`]).
    pub fn want_chart_picture(&mut self, chart_edit: Edit) {
        self.chart_want = Some(chart_edit);
    }

    /// The chart picture, once.
    pub fn take_chart_picture(&mut self) -> Option<Result<super::chart_profile::Shot, String>> {
        self.chart_pic.take()
    }

    fn chart_now(&mut self) {
        if self.chart_want.is_none() || !self.has_source {
            return;
        }
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        let Some(edit) = self.chart_want.take() else {
            return;
        };
        let d = dev_mut(engine);
        // Large enough for small charts in the frame; the next render develops the edit again.
        let long = self.long_side.max(CHART_LONG);
        let r = d.prepare(&edit, long).and_then(|_| d.ai_picture(&edit));
        let meta = d.sensor().map(|s| s.meta.clone());
        self.chart_pic = Some(
            r.map(|(picture, plan, _frame)| {
                let (w, h) = (picture.w, picture.h);
                super::chart_profile::Shot {
                    w,
                    h,
                    rgba: picture.rgba,
                    // Source-frame fractions → picture pixels, as AI masks map their points.
                    to_picture: Box::new(move |f| {
                        let q = plan
                            .as_ref()
                            .and_then(|p| {
                                let [fx, fy, fw, fh] = p.frame;
                                let o = p.to_output((fx + f.0 * fw, fy + f.1 * fh), 1.0)?;
                                let c = p.crop_rect();
                                Some((o.0 / c[2].max(1e-9), o.1 / c[3].max(1e-9)))
                            })
                            .unwrap_or(f);
                        (q.0 * f64::from(w), q.1 * f64::from(h))
                    }),
                    meta,
                }
            })
            .map_err(|e| e.to_string()),
        );
        self.dirty = true;
    }

    fn ai_now(&mut self) {
        if !self.ai_want || !self.has_source {
            return;
        }
        let (Some(engine), Some(edit)) = (self.engine.as_mut(), self.edit.as_ref()) else {
            return;
        };
        self.ai_want = false;
        let d = dev_mut(engine);
        self.ai_pic = Some(
            d.ai_picture(edit)
                .map(|(picture, plan, frame)| super::ai_masks::AiPicture::new(picture, plan, frame))
                .map_err(|e| e.to_string()),
        );
    }

    /// The source found, once.
    pub fn take_found(&mut self) -> Option<(usize, Option<[f32; 2]>)> {
        self.found.take()
    }

    /// Runs a waiting source search: the picture as retouch sees it (read back once per
    /// change before retouch), then the ring search on the CPU.
    fn find_now(&mut self) -> Result<(), specular_develop::gpu::Error> {
        let Some(engine) = self.engine.as_mut() else {
            return Ok(());
        };
        let (Some(_), Some(edit)) = (self.find.as_ref(), self.edit.as_ref()) else {
            return Ok(());
        };
        if !self.has_source || !dev(engine).idle() {
            return Ok(());
        }
        let Some((r, i)) = self.find.take() else {
            return Ok(());
        };
        let d = dev_mut(engine);
        d.base = self.orient;
        d.crop_on = self.crop_on;
        d.prepare(edit, self.long_side)?;
        // Everything before retouch decides the pixels.
        let reg = specular_develop::model::Registry::builtin();
        let cut = reg.rank(ModuleKind::RETOUCH);
        let key = format!(
            "{:?} {} {:?}",
            edit.modules
                .iter()
                .filter(|m| reg.rank(m.kind) < cut)
                .collect::<Vec<_>>(),
            self.long_side,
            (self.orient, self.crop_on)
        );
        if self.scene.as_ref().is_none_or(|s| s.0 != key) {
            let (w, h, px) = d.scene_pixels(edit, ModuleKind::RETOUCH)?;
            self.scene = Some((key, w, h, px));
        }
        let (Some(plan), Some((_, w, h, px))) = (d.plan(edit), self.scene.as_ref()) else {
            self.found = Some((i, None));
            return Ok(());
        };
        let g = specular_develop::mask::Geometry::Warp(&plan);
        let at = specular_develop::graph::Placement {
            geometry: g,
            image: (*w, *h),
            draft: false,
            sensor_bin: 1.0,
            photo: specular_develop::mask::depth::photo(&edit.source),
        };
        let one = |s: &retouch::Spot| {
            let mut s = s.clone();
            s.mode = retouch::SpotMode::Blur;
            retouch::Placed::new(
                &Retouch {
                    spots: vec![s],
                    ..Retouch::default()
                },
                &at,
            )
            .spots
            .pop()
        };
        let want = r.spots.get(i).and_then(one);
        let avoid: Vec<([f64; 2], f64)> = r
            .spots
            .iter()
            .enumerate()
            .filter(|(k, _)| *k != i)
            .filter_map(|(_, s)| one(s))
            .map(|p| {
                (
                    [f64::from(p.centre[0]), f64::from(p.centre[1])],
                    f64::from(p.radius),
                )
            })
            .collect();
        let found = want.and_then(|p| {
            let path: Vec<[f64; 2]> = p
                .path
                .iter()
                .map(|q| [f64::from(q[0]), f64::from(q[1])])
                .collect();
            retouch::search::best_source(
                &retouch::search::View { w: *w, h: *h, px },
                &retouch::search::Want {
                    centre: [f64::from(p.centre[0]), f64::from(p.centre[1])],
                    radius: f64::from(p.radius),
                    path: &path,
                    avoid: &avoid,
                },
            )
        });
        self.found = Some((i, found.map(|q| retouch::image_to_frame(&g, (*w, *h), q))));
        Ok(())
    }

    /// The colour picked, once.
    pub fn take_picked(&mut self) -> Option<Result<[f64; 3], String>> {
        self.picked.take()
    }

    /// With a pick before the tone equalizer: its automatic mask's percentiles, once.
    pub fn take_picked_percentiles(&mut self) -> Option<[f32; 2]> {
        self.picked_pct.take()
    }

    /// Samples a waiting pick when the GPU is free.
    fn pick_now(&mut self) -> Result<(), specular_develop::gpu::Error> {
        let Some(engine) = self.engine.as_mut() else {
            return Ok(());
        };
        let (Some((u, v)), Some(edit)) = (self.pick, self.edit.as_ref()) else {
            return Ok(());
        };
        if !self.has_source || !dev(engine).idle() {
            return Ok(());
        }
        self.pick = None;
        let d = dev_mut(engine);
        // The warp turns and crops: (u, v) are fractions of that picture.
        d.base = self.orient;
        d.crop_on = self.crop_on;
        d.prepare(edit, self.long_side)?;
        self.picked = Some(
            d.sample(edit, self.pick_kind, u, v)
                .map(|c| c.map(f64::from))
                .map_err(|e| e.to_string()),
        );
        // A zone pick with an automatic mask needs the mask's spread too.
        self.picked_pct = if self.pick_kind == ModuleKind::TONE_EQUALIZER {
            d.tone_eq_percentiles(edit).ok().flatten()
        } else {
            None
        };
        Ok(())
    }

    /// Something to say about the source just read (taken once).
    pub fn take_note(&mut self) -> Option<specular_develop::source::Note> {
        self.note.take()
    }

    pub fn take_as_shot(&mut self) -> Option<[f64; 2]> {
        self.as_shot.take()
    }

    /// The new photo's Lensfun lookup, once (`Some(None)`: none).
    pub fn take_lensfun(&mut self) -> Option<Option<Arc<Shot>>> {
        self.lensfun_new.take()
    }

    /// Asks for the picture of `edit`, turned by `orient` (and the edit's own turns),
    /// about `long_side` px long; `crop_on` false shows the whole straightened photo.
    pub fn request(&mut self, edit: &Edit, orient: Orient, long_side: u32, crop_on: bool) {
        let long_side = bucket(long_side);
        if self.edit.as_ref() == Some(edit)
            && orient == self.orient
            && long_side == self.long_side
            && crop_on == self.crop_on
        {
            return;
        }
        let framing =
            orient != self.orient || long_side != self.long_side || crop_on != self.crop_on;
        self.edit = Some(edit.clone());
        self.orient = orient;
        self.long_side = long_side;
        self.crop_on = crop_on;
        self.dirty = true;
        self.changed_at = Some(std::time::Instant::now());
        self.other_dirty |= framing && self.other.is_some();
    }

    /// The display peak the picture is for (1 for SDR or the SDR preview).
    pub fn set_peak(&mut self, peak: f32) {
        let peak = if peak.is_finite() { peak.max(1.0) } else { 1.0 };
        if (peak - self.peak).abs() > 0.01 {
            self.peak = peak;
            self.dirty = true;
            self.other_dirty = self.other.is_some();
        }
    }

    /// Takes what the background reader sent; whether to draw again (also while the GPU
    /// still works on a picture).
    pub fn poll(&mut self, loader: &Loader) -> bool {
        let mut changed = false;
        if std::mem::take(&mut self.reread) {
            self.read(loader);
        }
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Source(p, r) if self.path.as_deref() == Some(p.as_path()) => {
                    match r {
                        Ok(r) => self.accept(r),
                        Err(e) => {
                            specular_log::warn!(target: specular_log::GPU, "develop: {e}");
                            self.error = Some(e);
                        }
                    }
                    changed = true;
                }
                Msg::Source(..) => {}
                Msg::Next(p, tag, r) => match self.next.as_mut() {
                    Some(n) if n.tag == tag => match r {
                        Ok(r) => {
                            n.read = Some(r);
                            changed = true;
                        }
                        // Read again (and the error said) when it is opened.
                        Err(_) => n.failed = true,
                    },
                    // Opened while it was being read.
                    _ if self.path.as_deref() == Some(p.as_path())
                        && self.input.is_none()
                        && !self.has_source
                        && self.take.is_none() =>
                    {
                        match r {
                            Ok(r) => self.accept(r),
                            Err(e) => self.error = Some(e),
                        }
                        changed = true;
                    }
                    _ => {}
                },
                Msg::Calibration(k, r) => {
                    if self.calib_key.as_ref() == Some(&k) {
                        self.calib_ready = Some(r);
                        self.dirty = true;
                        changed = true;
                    }
                }
            }
        }
        self.calibrate(loader);
        if let Some(e) = self.engine.as_mut() {
            let (b, stale) = dev_mut(e).take_scopes();
            if let Some(b) = b {
                self.bins = Some(b);
                changed = true;
            }
            if stale {
                self.dirty = true;
            }
            changed |= dev(e).scopes_pending();
        }
        changed || self.busy()
    }

    /// Loads the flat and dark frames the edit asks for (in the background) when they
    /// change.
    fn calibrate(&mut self, loader: &Loader) {
        let (Some(sensor), Some(edit), Some(path)) = (&self.sensor, &self.edit, &self.path) else {
            return;
        };
        let key: CalibKey = (
            specular_develop::ops::correct::flat_field(edit),
            calib::folder(),
        );
        if self.calib_key.as_ref() == Some(&key) {
            return;
        }
        self.calib_key = Some(key.clone());
        if key.0.is_none() {
            self.calib_ready = Some(Ok(None));
            self.dirty = true;
            return;
        }
        let (sensor, edit, path, tx) =
            (sensor.clone(), edit.clone(), path.clone(), self.tx.clone());
        loader.background("develop-calib", move |l| {
            let data = specular_core::local_copy::read(&path).unwrap_or_default();
            let r = calib::for_edit(&edit, &sensor, Some((&path, &data)));
            let _ = tx.send(Msg::Calibration(key, r));
            l.waker()();
        });
    }

    /// Hands a new image to the engine and renders the newest request when the GPU is free;
    /// frees retired textures.
    pub fn upload(&mut self, gpu: &Gpu, ui: &mut UiRenderer) {
        for id in self.retired.drain(..) {
            ui.unregister_texture(id);
        }
        if self.error.is_some() {
            return;
        }
        let r = self.step(gpu, ui).and_then(|()| self.ahead());
        if let Err(e) = r {
            // A driver reset: the app opens a new device and calls `gpu_reset`.
            if e == specular_develop::gpu::Error::DeviceLost {
                return;
            }
            let software = matches!(self.engine, Some(Engine::Software(_)));
            specular_log::warn!(target: specular_log::GPU, "develop: {e}");
            if software || !gpu_error(&e) {
                self.error = Some(e.to_string());
                return;
            }
            // The app's GPU cannot develop: the software adapter (D2), from the file again.
            match engine::open_software_device() {
                Ok((device, queue)) => {
                    specular_log::info!(target: specular_log::GPU,
                        "develop: running on the software adapter");
                    let mut d =
                        Developer::new(&device, &queue, Budget::for_device(true, None, None));
                    d.time_frames(true);
                    self.engine = Some(Engine::Software(Box::new(d)));
                    // Its source is read again from the file.
                    self.has_source = false;
                    self.reread = true;
                    self.dirty = true;
                    if let Some(t) = self.tex.take() {
                        self.retired.push(t.id);
                    }
                }
                Err(s) => self.error = Some(s),
            }
        }
    }

    fn step(&mut self, gpu: &Gpu, ui: &mut UiRenderer) -> Result<(), specular_develop::gpu::Error> {
        let engine = self.engine.get_or_insert_with(|| {
            let integrated =
                gpu.info.device_type == wgpu::DeviceType::IntegratedGpu || gpu.is_software();
            let mut d = Developer::new(
                &gpu.device,
                &gpu.queue,
                Budget::for_device(integrated, None, None),
            );
            d.watch_device_loss(gpu.lost.clone());
            // "Preview while dragging" sizes a slider's drafts by how long they take.
            d.time_frames(true);
            // The first picture of a photo is a half-scale draft; it refines next.
            d.draft_first = true;
            // A refined picture after a change leaves sharpening out; the whole one
            // follows when the GPU is idle.
            d.defer_sharpen = true;
            Engine::App(Box::new(d))
        });
        // Developed ahead: the engine has its source already.
        let taken = match self.take.take() {
            Some(tag) => {
                let ok = dev_mut(engine).take_prefetched(tag);
                if !ok {
                    // Gone from the engine: read it again.
                    self.reread = true;
                }
                ok
            }
            None => false,
        };
        let input = self.input.take();
        let fresh = taken
            || match input {
                Some(input) => {
                    dev_mut(engine).set_input(input)?;
                    dev_mut(engine).set_raw_noise(self.noise.take());
                    true
                }
                None => false,
            };
        if fresh {
            // Capture sharpening's automatic radius from the lens.
            dev_mut(engine).set_lens(self.lens.0, self.lens.1);
            // Phase 3 item 11: the Lensfun profile of a raw without lens data of its own.
            dev_mut(engine).set_lensfun(self.lensfun.clone());
            dev_mut(engine).set_focal35(self.focal35);
            // The photo keys denoise's AI result.
            let photo = self
                .path
                .as_deref()
                .map(specular_develop::ops::detail::ai::photo_of_path);
            dev_mut(engine).set_photo(photo);
            self.has_source = true;
        }
        if self.has_source
            && let Some(r) = self.calib_ready.take()
        {
            let d = dev_mut(engine);
            match r {
                Ok(c) => d.set_calibration(c.as_deref())?,
                Err(e) => {
                    d.set_calibration(None)?;
                    self.notice = Some(e);
                }
            }
        }
        self.pick_now()?;
        self.find_now()?;
        self.upright_now()?;
        self.ai_now();
        self.chart_now();
        let Some(engine) = self.engine.as_mut() else {
            return Ok(());
        };
        // A slider's draft is done: how long it took sizes the next ones.
        if let Some(divisor) = self.timing
            && let Some(ms) = dev_mut(engine).take_frame_ms()
        {
            self.timing = None;
            self.pace.record(divisor, ms);
            self.timed = (self.timed.0 + 1, Some((divisor, ms)));
        }
        // Highlight statistics arrived: the picture refines.
        if self.has_source && dev_mut(engine).refine_due() {
            self.dirty = true;
        }
        // The picture first, then the one compared with it.
        let main = self.dirty;
        if !(main || self.other_dirty) || !self.has_source || !dev(engine).idle() {
            return Ok(());
        }
        let Some(edit) = (if main { &self.edit } else { &self.other }).as_ref() else {
            self.dirty = false;
            self.other_dirty = false;
            return Ok(());
        };
        if main {
            self.dirty = false;
        } else {
            self.other_dirty = false;
        }
        // The software path reads back 8-bit rows: SDR only; so does a soft proof.
        let peak = match engine {
            Engine::App(_) if self.aids.proof.is_none() => self.peak,
            _ => 1.0,
        };
        let headroom = view::headroom(edit, peak);
        let shown = view::for_headroom(edit, headroom);
        let out = OutputSpace {
            peak: headroom,
            ..OutputSpace::rec709()
        };
        let format = if headroom > 1.0 {
            wgpu::TextureFormat::Rgba16Float
        } else {
            wgpu::TextureFormat::Rgba8UnormSrgb
        };
        let d = dev_mut(engine);
        // The warp turns the picture (the file's orientation and the edit's turns).
        d.base = self.orient;
        d.crop_on = self.crop_on;
        d.mask_overlay = self.overlay;
        d.prepare(&shown, self.long_side)?;
        // Clipped highlights take their colour from statistics read back after the develop:
        // a picture made with older or neutral ones isn't shown (the earlier picture stays)
        // until they arrive and develop it again, for at most `HOLD`.
        if main {
            if d.provisional() && hold(&mut self.held, std::time::Instant::now()) {
                self.dirty = true;
                return Ok(());
            }
            self.held = None;
        }
        let focus = if self.draft {
            Some((ModuleKind::RETOUCH, 0))
        } else {
            self.draft_module.map(|k| (k, 0))
        };
        // "Preview while dragging": a slider's drafts at the level that keeps up; at the
        // last resort only once the slider rests.
        d.drag_level = None;
        let slider = match focus {
            Some(f) if main && !self.draft => {
                let key = drag_key(self.path.as_deref(), &shown, f.0, self.long_side);
                self.pace.begin(key, d.drag_default(&shown, f));
                let level = self.pace.level();
                let resting = self
                    .changed_at
                    .is_none_or(|t| t.elapsed().as_millis() >= u128::from(drag::PAUSE_MS));
                if level == Level::Pause && !resting {
                    self.dirty = true;
                    return Ok(());
                }
                d.drag_level = Some(level);
                true
            }
            _ => false,
        };
        let r = d.render(&shown, focus, &out)?;
        self.timing = slider.then_some(d.stats.draft_divisor);
        if main {
            // A quick draft (half scale, a module's own draft, the first frame) or older
            // highlight statistics: a refined picture follows.
            self.shown_draft = focus.is_some() || d.stats.half_draft || d.provisional();
        }
        let (ow, oh) = (r.roi.w, r.roi.h);
        // Proofed first, so the scopes and the overlay see what is shown.
        if let Some(lut) = &self.aids.proof {
            d.proof(&r, lut);
        }
        if main && self.aids.scopes {
            d.scopes(&r, headroom);
        }
        if main && self.aids.clip.any() && matches!(engine, Engine::App(_)) {
            let fits = self
                .clip_tex
                .as_ref()
                .is_some_and(|t| t.tex.width() == ow && t.tex.height() == oh);
            if !fits {
                let (tex, id) = texture(gpu, ui, ow, oh, wgpu::TextureFormat::Rgba8Unorm);
                if let Some(old) = self.clip_tex.replace(Tex {
                    tex,
                    id,
                    orient: self.orient,
                    crop_on: self.crop_on,
                }) {
                    ui.unregister_texture(old.id);
                }
            }
            if let Some(t) = self.clip_tex.as_mut() {
                t.orient = self.orient;
                t.crop_on = self.crop_on;
                let proofed = self.aids.proof.is_some();
                dev_mut(engine).clipping(&shown, &r, self.aids.clip, &out, proofed, &t.tex)?;
            }
        }
        let slot = if main {
            &mut self.tex
        } else {
            &mut self.other_tex
        };
        let fits = slot.as_ref().is_some_and(|t| {
            t.tex.width() == ow && t.tex.height() == oh && t.tex.format() == format
        });
        match engine {
            Engine::App(d) => {
                if !fits {
                    let (tex, id) = texture(gpu, ui, ow, oh, format);
                    if let Some(old) = slot.replace(Tex {
                        tex,
                        id,
                        orient: self.orient,
                        crop_on: self.crop_on,
                    }) {
                        ui.unregister_texture(old.id);
                    }
                }
                let t = slot.as_mut().expect("a texture");
                t.orient = self.orient;
                t.crop_on = self.crop_on;
                d.present(r, Orient::default(), &t.tex)?;
            }
            Engine::Software(d) => {
                let (w, h, px) = d.read_rgba8(r, Orient::default())?;
                let (tex, id) = upload(gpu, ui, w, h, &px);
                if let Some(old) = slot.replace(Tex {
                    tex,
                    id,
                    orient: self.orient,
                    crop_on: self.crop_on,
                }) {
                    ui.unregister_texture(old.id);
                }
            }
        }
        Ok(())
    }

    /// The picture to draw, when it is turned as `orient` asks and cropped (or not) as
    /// `crop_on` says.
    pub fn texture(&self, orient: Orient, crop_on: bool) -> Option<TexId> {
        self.tex
            .as_ref()
            .filter(|t| t.orient == orient && t.crop_on == crop_on)
            .map(|t| t.id)
    }
}

fn dev(e: &Engine) -> &Developer {
    match e {
        Engine::App(d) | Engine::Software(d) => d,
    }
}

fn dev_mut(e: &mut Engine) -> &mut Developer {
    match e {
        Engine::App(d) | Engine::Software(d) => d,
    }
}

fn gpu_error(e: &specular_develop::gpu::Error) -> bool {
    matches!(e, specular_develop::gpu::Error::Gpu(_))
}

/// The longest a picture with provisional highlight statistics is held back.
const HOLD: std::time::Duration = std::time::Duration::from_millis(500);

/// Whether a provisional picture is still held back at `now` (the hold starts at the first
/// call).
fn hold(since: &mut Option<std::time::Instant>, now: std::time::Instant) -> bool {
    now.duration_since(*since.get_or_insert(now)) < HOLD
}

/// A slider drag's photo and edit shape ([`drag::Key`]): the photo, the modules in use, the
/// module dragged and the view's size.
fn drag_key(path: Option<&Path>, edit: &Edit, kind: ModuleKind, long_side: u32) -> drag::Key {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    kind.hash(&mut h);
    long_side.hash(&mut h);
    for m in edit.modules.iter().filter(|m| m.enabled) {
        (m.kind, m.instance).hash(&mut h);
    }
    h.finish()
}

/// Long sides in steps of half an octave, so a zoom does not develop the raw again at
/// every step.
fn bucket(long: u32) -> u32 {
    let mut b = 512u32;
    while b < long.min(MAX_SIDE) {
        b = if b.is_power_of_two() {
            b + b / 2
        } else {
            b / 3 * 4
        };
    }
    b.min(MAX_SIDE)
}

/// A source read for Develop.
pub(super) struct SourceRead {
    pub input: Input,
    /// A raw's as-shot white.
    pub white: Option<[f64; 2]>,
    /// Aperture and pixel pitch (capture sharpening's automatic radius).
    pub lens: (Option<f32>, Option<f32>),
    /// Something to say about the source (a CMYK file edited in RGB).
    pub note: Option<specular_develop::source::Note>,
    /// A raw's noise model, when fitted off the UI thread ([`read_with_noise`]).
    pub noise: Option<specular_develop::ops::detail::RawNoise>,
    /// A raw's Lensfun lookup ([`specular_develop::ops::geometry::lensfun::shot_of_raw`]).
    pub lensfun: Option<Arc<Shot>>,
    /// The file's 35 mm equivalent focal length, for perspective.
    pub focal35: Option<f64>,
}

/// [`read_source_full`] with a raw's noise model fitted here (off the UI thread; denoise
/// would fit it on first use).
fn read_with_noise(
    registry: &specular_decode::Registry,
    path: &Path,
) -> Result<SourceRead, String> {
    let mut r = read_source_full(registry, path)?;
    if let Input::Raw(s) = &r.input {
        r.noise = Some(specular_develop::ops::detail::RawNoise::of_sensor(s));
    }
    Ok(r)
}

/// Reads `path`: a raw's sensor data, or the image decoded and reduced to [`SIDE`].
pub(super) fn read_source(
    registry: &specular_decode::Registry,
    path: &Path,
) -> Result<(Input, Option<[f64; 2]>), String> {
    read_source_full(registry, path).map(|r| (r.input, r.white))
}

/// [`read_source`] with the lens facts and the note.
pub(super) fn read_source_full(
    registry: &specular_decode::Registry,
    path: &Path,
) -> Result<SourceRead, String> {
    use specular_develop::ops::detail::capture::lens_of_file;
    if super::controls::is_raw(path) {
        // The lens database loads alongside the decode (once; the lookup needs it below).
        specular_lensfun::preload();
        // Mapped for a raw video clip (gigabytes; only its first frame is read).
        let data = specular_develop::thumb::raw_bytes(path)?;
        let data: &[u8] = &data;
        if let Ok(input) = engine::decode_raw(data) {
            let (white, long, lensfun) = match &input {
                Input::Raw(s) => (
                    engine::as_shot_white_of(s, data),
                    s.crop.w.max(s.crop.h),
                    specular_develop::ops::geometry::lensfun::shot_of_raw(s, data),
                ),
                Input::Rgb { width, height, .. } => (None, *width.max(height), None),
            };
            return Ok(SourceRead {
                lens: lens_of_file(data, long),
                input,
                white,
                note: None,
                noise: None,
                lensfun,
                focal35: specular_develop::ops::geometry::persp::focal35_of_file(data),
            });
        }
        // A raw the sensor path does not take develops from its decoded image.
    }
    let s = registry.open_path(path).map_err(|e| e.to_string())?;
    let info = s.info();
    let long = info.width.max(info.height).max(1) as usize;
    let scale = info
        .scales
        .iter()
        .copied()
        .filter(|&k| long / k as usize >= SIDE as usize || k == 1)
        .max()
        .unwrap_or(1);
    let icc: Option<Vec<u8>> = info
        .icc_profile
        .as_ref()
        .map(|a| a.as_ref().clone())
        .or_else(|| s.embedded_icc().map(<[u8]>::to_vec));
    let head = read_head(path, 1 << 20);
    let note = specular_develop::source::Note::of_source(s.colour_model(), icc.as_deref(), &head);
    let cancel = CancelToken::new();
    let r = s
        .decode(&specular_decode::DecodeRequest::full(scale), &cancel)
        .map_err(|e| e.to_string())?;
    // Through its profile, unclipped, transparency kept.
    let input = specular_develop::source::input(&r, icc.as_deref(), Some(SIDE), &cancel)
        .ok_or("cancelled")?;
    // Only the Exif is needed for the lens: the file's head holds it.
    Ok(SourceRead {
        input,
        white: None,
        lens: lens_of_file(&head, long as u32),
        note,
        noise: None,
        lensfun: None,
        focal35: specular_develop::ops::geometry::persp::focal35_of_file(&head),
    })
}

/// The first `n` bytes of `path` (fewer for a short file, none when unreadable).
fn read_head(path: &Path, n: u64) -> Vec<u8> {
    use std::io::Read;
    let mut v = Vec::new();
    if let Ok(f) = std::fs::File::open(path) {
        let _ = f.take(n).read_to_end(&mut v);
    }
    v
}

/// An empty texture the engine writes (`Rgba8UnormSrgb`, or `Rgba16Float` for HDR),
/// registered with the UI.
fn texture(
    gpu: &Gpu,
    ui: &mut UiRenderer,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, TexId) {
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("develop picture"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    let id = ui.register_texture(gpu, &view);
    (tex, id)
}

/// An `Rgba8UnormSrgb` texture holding `rgba` (sRGB-encoded rows), registered with the UI.
pub(super) fn upload(
    gpu: &Gpu,
    ui: &mut UiRenderer,
    w: u32,
    h: u32,
    rgba: &[u8],
) -> (wgpu::Texture, TexId) {
    let size = wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("develop preview"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * w),
            rows_per_image: None,
        },
        size,
    );
    let view = tex.create_view(&Default::default());
    let id = ui.register_texture(gpu, &view);
    (tex, id)
}

use crate::loader::Loader;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provisional_pictures_are_held_back_for_a_while() {
        let t0 = std::time::Instant::now();
        let mut held = None;
        assert!(hold(&mut held, t0));
        assert!(hold(&mut held, t0 + HOLD / 2));
        // Statistics that never arrive don't hide the picture for good.
        assert!(!hold(&mut held, t0 + HOLD));
    }

    /// A drag's remembered level is per photo, modules in use, module dragged and view
    /// size; a slider's value is not part of it.
    #[test]
    fn drag_keys_follow_photo_and_edit_shape() {
        let reg = specular_develop::model::Registry::builtin();
        let e = Edit::default_for(reg, Default::default(), false);
        let a = Path::new("a.jpg");
        let k = drag_key(Some(a), &e, ModuleKind::EXPOSURE, 1536);
        let mut moved = e.clone();
        if let Some(m) = moved.module_mut(ModuleKind::EXPOSURE, 0) {
            let mut f = m.params.to_fields();
            f.set("exposure_ev", specular_develop::model::Value::Float(1.0));
            m.params.set_fields(f);
        }
        assert_eq!(drag_key(Some(a), &moved, ModuleKind::EXPOSURE, 1536), k);
        assert_ne!(
            drag_key(Some(Path::new("b.jpg")), &e, ModuleKind::EXPOSURE, 1536),
            k
        );
        assert_ne!(
            drag_key(Some(a), &e, ModuleKind::COLOUR_CALIBRATION, 1536),
            k
        );
        assert_ne!(drag_key(Some(a), &e, ModuleKind::EXPOSURE, 2048), k);
        let mut shaped = e.clone();
        shaped.insert(
            reg,
            specular_develop::model::ModuleInstance::new(reg, ModuleKind::LOCAL_CONTRAST, 0),
        );
        assert_ne!(drag_key(Some(a), &shaped, ModuleKind::EXPOSURE, 1536), k);
    }

    #[test]
    fn sizes_come_in_half_octave_steps() {
        assert_eq!(bucket(100), 512);
        assert_eq!(bucket(600), 768);
        assert_eq!(bucket(1100), 1536);
        assert_eq!(bucket(2000), 2048);
        assert_eq!(bucket(2600), 3072);
        assert_eq!(bucket(100_000), MAX_SIDE);
        // Every step is at least the asked size (up to the cap).
        for l in (1..5000).step_by(37) {
            assert!(bucket(l) >= l.min(MAX_SIDE));
        }
    }
}
