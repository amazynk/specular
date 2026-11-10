//! AI denoise in Develop: the Denoise feature's network
//! (`specular_ai::denoise`, NAFNet on SIDD) mixed with the classic result
//! (`specular_develop::ops::detail::ai`).
//!
//! - **The Detail card** gets "Denoise: Classic / AI", the AI result's share and a status
//!   line with one button (Stop while it runs, Local AI when the model is missing).
//! - **When it runs.** At 100 % view (and closer): the cells of the frame on screen, in the
//!   background, with the classic result as the draft. Exports run the whole frame first
//!   (through the runner set here, [`install_runner`]). Nothing downloads from here.
//! - **The worker** (`ai-denoise`) develops the frame at full size without the warp (its
//!   own [`Developer`] on the app's GPU), up to denoise, and keeps it for the next view;
//!   the network fills the result cell row by cell row, the preview showing each.
//! - **Cache.** Results go to `<cache>/develop-denoise`, by key (photo, model and every
//!   module before denoise), never into the sidecar; the folder is kept under
//!   [`CACHE_BYTES`], oldest first.
//! - **Memory** is reserved up front: a refusal says how much is needed.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use specular_ai::denoise::{self as aid, DenoiseModel, Denoiser};
use specular_develop::model::{Edit, ModuleKind};
use specular_develop::ops::detail::NoiseModel;
use specular_develop::ops::detail::ai::{self, AiParams, AiResult, Encoding, Mode};
use specular_develop::preview::Developer;
use specular_i18n::{I18n, t};
use specular_infer::tiles::{Pixels, Rows};

use super::controls::Session;
use crate::app::App;

/// Bytes of results kept on disk.
pub(super) const CACHE_BYTES: u64 = 3 << 30;
/// How long the installed models are trusted before being looked up again (s).
const FILES_TTL_S: f64 = 3.0;
/// Zoom (screen pixels per photo pixel) from which the view counts as 100 %.
const FULL_ZOOM: f32 = 0.98;
/// While a run goes, the preview shows new rows at most this often (s).
const SHOW_EVERY_S: f64 = 1.5;

/// What the Denoise card says about the AI mode.
#[derive(Clone, Debug, PartialEq, Default)]
pub(super) enum State {
    #[default]
    Classic,
    /// The model is not installed: its name.
    Missing(String),
    /// Not computed here: shown at 100 % and in exports.
    Draft,
    /// Running: per mille done.
    Running(u32),
    /// Every cell on screen is done (or the whole frame).
    Ready,
    Stopped,
    Failed(String),
}

/// A session's AI denoise state.
#[derive(Debug, Default)]
pub(super) struct SessionDenoise {
    pub state: State,
}

/// The installed Denoise models, looked up now.
fn installed(prefer: Option<&str>) -> Option<DenoiseModel> {
    let data = specular_settings::Locations::detect().data_dir;
    let store = specular_ai::install::Store::new(&data);
    aid::find(
        &specular_ai::Manifest::load(),
        store.root(),
        &store.installed(),
        prefer,
    )
}

/// The folder results are kept in.
fn cache_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        if cfg!(test) {
            return std::env::temp_dir()
                .join(format!("specular-ai-denoise-{}", std::process::id()));
        }
        specular_settings::Locations::detect()
            .cache_dir
            .join("develop-denoise")
    })
    .clone()
}

fn cache_file(key: &[u8; 32]) -> PathBuf {
    cache_dir().join(format!("{}.dn", ai::hex(key)))
}

/// Writes `r` for `key`, then trims the folder to `limit` bytes (oldest first; `key`'s own
/// file stays).
fn save(key: &[u8; 32], r: &AiResult, limit: u64) {
    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    let tmp = cache_file(key).with_extension("part");
    if std::fs::write(&tmp, r.encode()).is_ok() {
        let _ = std::fs::rename(&tmp, cache_file(key));
    }
    trim(&dir, limit, &cache_file(key), &["dn"]);
}

/// Deletes the oldest files of `dir` with one of the extensions `exts` until those hold at
/// most `limit` bytes (`keep` stays).
pub(super) fn trim(dir: &Path, limit: u64, keep: &Path, exts: &[&str]) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = rd
        .filter_map(|e| {
            let e = e.ok()?;
            let p = e.path();
            exts.contains(&p.extension()?.to_str()?).then_some(())?;
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), p))
        })
        .collect();
    files.sort();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    for (_, len, p) in files {
        if total <= limit {
            break;
        }
        if p == keep {
            continue;
        }
        if std::fs::remove_file(&p).is_ok() {
            total -= len;
        }
    }
}

/// The frame's linear RGBA, encoded for the network as the tiler reads it.
struct Frame<'a> {
    w: usize,
    h: usize,
    rgba: &'a [f32],
    enc: Encoding,
}

impl Pixels for Frame<'_> {
    fn size(&self) -> (usize, usize) {
        (self.w, self.h)
    }

    fn fill(&self, x: i64, y: i64, w: usize, h: usize, out: &mut [f32]) {
        let cx = |v: i64| v.clamp(0, self.w as i64 - 1) as usize;
        let cy = |v: i64| v.clamp(0, self.h as i64 - 1) as usize;
        let n = w * h;
        for r in 0..h {
            let row = cy(y + r as i64) * self.w;
            for k in 0..w {
                let i = 4 * (row + cx(x + k as i64));
                let e = self
                    .enc
                    .encode([self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]);
                for (c, v) in e.into_iter().enumerate() {
                    out[c * n + r * w + k] = v;
                }
            }
        }
    }
}

/// The app's runner: the installed model on the GPU (checked against the CPU first) or
/// the CPU.
pub(super) struct Runner {
    /// The app's device, and the context checked on it (made on first use, off the UI
    /// thread: the check runs small networks; checked again on the new device after a GPU
    /// reset).
    gpu: Mutex<(
        crate::app::ai::device::Session,
        Option<Option<specular_infer::gpu::Context>>,
    )>,
    net: Mutex<Option<Denoiser>>,
    /// Set to stop the run in progress.
    pub stop: Arc<AtomicBool>,
}

impl Runner {
    fn new(gpu: Option<(specular_gfx::wgpu::Device, specular_gfx::wgpu::Queue)>) -> Runner {
        Runner {
            gpu: Mutex::new((crate::app::ai::device::Session::new(gpu), None)),
            net: Mutex::new(None),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    fn context(&self) -> Option<specular_infer::gpu::Context> {
        let mut g = self.gpu.lock().unwrap_or_else(|p| p.into_inner());
        let (session, chosen) = &mut *g;
        if chosen.is_none() || session.stale() {
            let ctx = aid::choose_gpu(session.context(), specular_core::paths::selector());
            *chosen = Some(ctx.cloned());
        }
        chosen.clone().flatten()
    }

    /// Whether the network must be built again on a new device.
    fn stale(&self) -> bool {
        self.gpu.lock().unwrap_or_else(|p| p.into_inner()).0.stale()
    }
}

impl ai::Runner for Runner {
    fn run(
        &self,
        key: &[u8; 32],
        model: &str,
        (w, h): (u32, u32),
        input: &[f32],
        noise: Option<NoiseModel>,
        rect: Option<[u32; 4]>,
        progress: &mut dyn FnMut(f32) -> bool,
    ) -> Result<bool, String> {
        let found = installed(Some(model)).filter(|m| m.id == model);
        let Some(found) = found else {
            return Err(t!(
                "develop-noise-ai-needs",
                model = aid::model_name(&specular_ai::Manifest::load(), model)
            ));
        };
        let mut guard = self.net.lock().unwrap_or_else(|p| p.into_inner());
        if guard.as_ref().is_none_or(|d| d.id() != found.id) || self.stale() {
            *guard = None;
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
            *guard = Some(
                Denoiser::new(found, self.context().as_ref(), threads)
                    .map_err(|e| e.to_string())?,
            );
        }
        let net = guard.as_mut().expect("loaded above");
        let tiling = net.tiling();
        let mb = specular_ai::Manifest::load()
            .model(model)
            .map_or(1400, |m| u64::from(m.runtime.min_memory_mb));
        // The network's memory and the result (a frame of packed f16), reserved up front.
        let need = aid::memory_needed(mb, tiling, w as usize) + u64::from(w) * u64::from(h) * 8;
        let _hold = specular_core::budget::reserve(need).map_err(|n| {
            t!(
                "develop-noise-ai-memory",
                needed = n.needed.div_ceil(1 << 30) as i64
            )
        })?;
        let (wu, hu) = (w as usize, h as usize);
        let cells = match rect {
            Some([x0, y0, x1, y1]) => {
                tiling.cells_of((wu, hu), x0 as usize, y0 as usize, x1 as usize, y1 as usize)
            }
            None => tiling.cells_of((wu, hu), 0, 0, wu, hu),
        };
        let shape = (w, h, tiling.stride() as u32);
        if ai::store()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .covers(
                key,
                (
                    cells.x0 as u32,
                    cells.y0 as u32,
                    cells.x1 as u32,
                    cells.y1 as u32,
                ),
            )
        {
            return Ok(true);
        }
        let enc = if net.takes_sigma() {
            Encoding::gat(&noise.unwrap_or(NoiseModel::FALLBACK))
        } else {
            Encoding::Srgb
        };
        let src = Frame {
            w: wu,
            h: hu,
            rgba: input,
            enc,
        };
        let stop = self.stop.clone();
        stop.store(false, Ordering::Relaxed);
        let t = Instant::now();
        let done = net
            .run(
                &src,
                cells,
                enc.sigma(),
                &mut |r: Rows<'_>| {
                    ai::store()
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .update(key, shape, |res| {
                            let at = (r.x as u32, r.y as u32, r.w as u32, r.h as u32);
                            res.put(at, r.data, input, &enc);
                        });
                },
                &mut |d, n| progress(d as f32 / n.max(1) as f32) && !stop.load(Ordering::Relaxed),
            )
            .map_err(|e| e.to_string())?;
        specular_log::info!(target: specular_log::JOBS,
            "develop: AI denoise {}×{} cells {:?} in {:.1} s on the {}",
            w, h, cells, t.elapsed().as_secs_f64(), if net.on_gpu() { "GPU" } else { "CPU" });
        drop(guard);
        if let Some(r) = ai::store()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(key)
        {
            save(key, &r, CACHE_BYTES);
        }
        Ok(done)
    }
}

/// Sets the process's runner (once) and the store's disk loader, so previews and exports
/// find results.
pub(super) fn install_runner(
    gpu: Option<(specular_gfx::wgpu::Device, specular_gfx::wgpu::Queue)>,
) -> Arc<Runner> {
    static R: std::sync::OnceLock<Arc<Runner>> = std::sync::OnceLock::new();
    R.get_or_init(|| {
        let r = Arc::new(Runner::new(gpu));
        *ai::runner().lock().unwrap_or_else(|p| p.into_inner()) = Some(r.clone());
        ai::store()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .set_loader(Some(Box::new(|k| {
                std::fs::read(cache_file(k))
                    .ok()
                    .and_then(|b| AiResult::decode(&b))
            })));
        r
    })
    .clone()
}

/// The last frame the worker developed: its denoise key, size, pixels and noise.
type FrameKept = ([u8; 32], u32, u32, Vec<f32>, Option<NoiseModel>);

struct Job {
    path: PathBuf,
    edit: Edit,
    key: [u8; 32],
    model: String,
    /// Frame fractions `[x0, y0, x1, y1]` to cover.
    rect: [f64; 4],
}

struct Done {
    key: [u8; 32],
    result: Result<bool, String>,
}

/// The view worker and what it runs.
#[derive(Default)]
pub(super) struct Worker {
    tx: Option<Sender<Job>>,
    rx: Option<Receiver<Done>>,
    runner: Option<Arc<Runner>>,
    progress: Arc<AtomicU32>,
    /// The key and region in flight.
    inflight: Option<([u8; 32], [f64; 4])>,
    /// Keys stopped or failed (not started again until the edit changes them).
    held: Vec<([u8; 32], State)>,
    files: Option<(Option<DenoiseModel>, Instant)>,
    shown_at: Option<Instant>,
    shown_version: Option<(usize, usize)>,
    /// The view on screen at 100 %: frame fractions.
    pub view: Option<[f64; 4]>,
    /// The last job sent (not sent again for the same key and region).
    sent: Option<([u8; 32], [f64; 4])>,
}

/// The edit with the warp left out (the frame, upright as the sensor is).
fn frame_edit(edit: &Edit) -> Edit {
    let mut e = edit.clone();
    for m in &mut e.modules {
        if matches!(
            m.kind,
            ModuleKind::CROP_ORIENTATION | ModuleKind::LENS | ModuleKind::PERSPECTIVE
        ) {
            m.enabled = false;
        }
    }
    e
}

/// The frame's denoise input: the photo developed at full size up to denoise, no warp.
fn frame_input(
    dev: &mut Developer,
    registry: &specular_decode::Registry,
    job: &Job,
    photo: [u8; 32],
) -> Result<(u32, u32, Vec<f32>), String> {
    let read = super::preview::read_source_full(registry, &job.path)?;
    dev.set_input(read.input).map_err(|e| e.to_string())?;
    dev.set_photo(Some(photo));
    dev.base = specular_develop::preview::Orient::default();
    dev.crop_on = false;
    dev.highlight_stats = false;
    let e = frame_edit(&job.edit);
    dev.prepare(&e, u32::MAX).map_err(|e| e.to_string())?;
    dev.scene_pixels(&e, ModuleKind::DENOISE)
        .map_err(|e| e.to_string())
}

impl Worker {
    fn start(
        &mut self,
        gpu: Option<(specular_gfx::wgpu::Device, specular_gfx::wgpu::Queue)>,
        registry: Arc<specular_decode::Registry>,
        waker: crate::loader::Wake,
    ) {
        let runner = install_runner(gpu.clone());
        self.runner = Some(runner.clone());
        let (jtx, jrx) = channel::<Job>();
        let (dtx, drx) = channel::<Done>();
        let progress = self.progress.clone();
        let spawned = specular_thread::builder("ai-denoise").spawn(move || {
            let mut dev = gpu.as_ref().map(|(d, q)| {
                Developer::new(
                    d,
                    q,
                    specular_develop::gpu::pool::Budget::for_device(true, None, None),
                )
            });
            // The last frame developed: its denoise key, size, pixels and noise.
            let mut frame: Option<FrameKept> = None;
            while let Ok(job) = jrx.recv() {
                let photo = ai::photo_of_path(&job.path);
                let result = (|| {
                    let Some(dev) = dev.as_mut() else {
                        return Err(t!("develop-noise-ai-no-gpu"));
                    };
                    if frame.as_ref().is_none_or(|f| f.0 != job.key) {
                        frame = None;
                        let (w, h, px) = frame_input(dev, &registry, &job, photo)?;
                        frame = Some((job.key, w, h, px, dev.noise_model()));
                    }
                    let (_, w, h, px, noise) = frame.as_ref().expect("made above");
                    let px_rect = [
                        (job.rect[0] * f64::from(*w)).floor().max(0.0) as u32,
                        (job.rect[1] * f64::from(*h)).floor().max(0.0) as u32,
                        (job.rect[2] * f64::from(*w)).ceil().min(f64::from(*w)) as u32,
                        (job.rect[3] * f64::from(*h)).ceil().min(f64::from(*h)) as u32,
                    ];
                    let p = progress.clone();
                    ai::Runner::run(
                        runner.as_ref(),
                        &job.key,
                        &job.model,
                        (*w, *h),
                        px,
                        *noise,
                        Some(px_rect),
                        &mut |f| {
                            p.store((f * 1000.0) as u32, Ordering::Relaxed);
                            waker();
                            true
                        },
                    )
                })();
                if dtx
                    .send(Done {
                        key: job.key,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
                waker();
            }
        });
        if spawned.is_ok() {
            self.tx = Some(jtx);
            self.rx = Some(drx);
        }
    }

    fn model(&mut self, prefer: &str) -> Option<DenoiseModel> {
        let fresh = self
            .files
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed().as_secs_f64() < FILES_TTL_S);
        if !fresh {
            self.files = Some((installed(Some(prefer)), Instant::now()));
        }
        self.files.as_ref().and_then(|f| f.0.clone())
    }

    /// Stops the run in progress (it keeps what it finished).
    pub fn stop(&mut self) {
        if let Some(r) = &self.runner {
            r.stop.store(true, Ordering::Relaxed);
        }
    }
}

/// The frame fractions of the screen rectangle `vis` over the picture, through `map`.
pub(super) fn view_rect(map: &super::masks::Map, vis: specular_gfx::Rect) -> [f64; 4] {
    let pts = [
        (vis.x, vis.y),
        (vis.right(), vis.y),
        (vis.x, vis.bottom()),
        (vis.right(), vis.bottom()),
        (vis.center().0, vis.y),
        (vis.center().0, vis.bottom()),
        (vis.x, vis.center().1),
        (vis.right(), vis.center().1),
    ];
    let mut r = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in pts {
        let f = map.to_frame(p);
        r[0] = r[0].min(f.0);
        r[1] = r[1].min(f.1);
        r[2] = r[2].max(f.0);
        r[3] = r[3].max(f.1);
    }
    [
        r[0].clamp(0.0, 1.0),
        r[1].clamp(0.0, 1.0),
        r[2].clamp(0.0, 1.0),
        r[3].clamp(0.0, 1.0),
    ]
}

/// Whether the picture shows at 100 % or closer: `shown` (logical px) of a crop
/// `crop_px` photo pixels wide, at `scale` physical px per logical px.
pub(super) fn at_full_size(shown_w: f32, scale: f32, crop_px: f64) -> bool {
    crop_px > 0.0 && f64::from(shown_w * scale) >= f64::from(FULL_ZOOM) * crop_px
}

impl App {
    /// AI denoise: the state for the card, the view's cells run in the background.
    pub(super) fn develop_denoise_tick(&mut self) {
        let Some(s) = self.develop.session.as_mut() else {
            return;
        };
        let d = &mut self.develop.denoise;
        let mut arrived = false;
        if let Some(rx) = &d.rx {
            while let Ok(done) = rx.try_recv() {
                if d.inflight.is_some_and(|(k, _)| k == done.key) {
                    d.inflight = None;
                }
                match done.result {
                    Ok(true) => {}
                    Ok(false) => d.held.push((done.key, State::Stopped)),
                    Err(e) => d.held.push((done.key, State::Failed(e))),
                }
                arrived = true;
            }
        }
        let a = ai::ai_module(&s.edit);
        // Exports find the runner too (they run the whole frame first).
        if a.is_some() && d.runner.is_none() {
            let gpu = self
                .gfx
                .as_ref()
                .map(|g| (g.gpu.device.clone(), g.gpu.queue.clone()));
            d.runner = Some(install_runner(gpu));
        }
        let state = match a {
            None => State::Classic,
            Some(a) => {
                let photo = *s.ai.photo.get_or_insert_with(|| ai::photo_of_path(&s.path));
                let model = if a.model.is_empty() {
                    aid::default_model(&specular_ai::Manifest::load()).unwrap_or_default()
                } else {
                    a.model.clone()
                };
                let key = ai::key(&photo, &s.edit, &model);
                let found = d.model(&model).filter(|m| m.id == model);
                let held = d.held.iter().find(|h| h.0 == key).map(|h| h.1.clone());
                if found.is_none() {
                    State::Missing(aid::model_name(&specular_ai::Manifest::load(), &model))
                } else if let Some((k, _)) = d.inflight.filter(|(k, _)| *k == key) {
                    let _ = k;
                    State::Running(d.progress.load(Ordering::Relaxed))
                } else if let Some(h) = held {
                    h
                } else {
                    match d.view {
                        Some(rect) => {
                            let covered = ai::store()
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .covers_fraction(&key, rect);
                            if covered {
                                State::Ready
                            } else if d.inflight.is_some() || d.sent == Some((key, rect)) {
                                State::Draft
                            } else {
                                if d.tx.is_none() {
                                    let gpu = self
                                        .gfx
                                        .as_ref()
                                        .map(|g| (g.gpu.device.clone(), g.gpu.queue.clone()));
                                    d.start(gpu, self.loader.registry(), self.loader.waker());
                                }
                                let job = Job {
                                    path: s.path.clone(),
                                    edit: s.edit.clone(),
                                    key,
                                    model: model.clone(),
                                    rect,
                                };
                                d.progress.store(0, Ordering::Relaxed);
                                if d.tx.as_ref().is_some_and(|tx| tx.send(job).is_ok()) {
                                    d.inflight = Some((key, rect));
                                    d.sent = Some((key, rect));
                                    State::Running(0)
                                } else {
                                    State::Failed(t!("develop-ai-failed-worker"))
                                }
                            }
                        }
                        None => {
                            let full = ai::store()
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .progress(&key)
                                .is_some_and(|(x, n)| x == n);
                            if full { State::Ready } else { State::Draft }
                        }
                    }
                }
            }
        };
        // New rows show while it runs (not more often than SHOW_EVERY_S).
        if matches!(state, State::Running(_)) {
            let due = d
                .shown_at
                .is_none_or(|t| t.elapsed().as_secs_f64() >= SHOW_EVERY_S);
            if due {
                d.shown_at = Some(Instant::now());
                let v = d
                    .inflight
                    .and_then(|(k, _)| ai::store().lock().ok()?.progress(&k));
                if v != d.shown_version {
                    d.shown_version = v;
                    arrived = true;
                }
            }
        }
        if arrived {
            self.develop.preview.invalidate();
            self.redraw = true;
        }
        if state != s.denoise.state {
            s.denoise.state = state;
            s.sync_panel();
            self.redraw = true;
        }
    }

    /// The card's button: Stop while it runs, Local AI when the model is missing.
    pub(super) fn develop_denoise_action(&mut self) {
        let Some(s) = self.develop.session.as_ref() else {
            return;
        };
        match &s.denoise.state {
            State::Running(_) => {
                self.develop.denoise.stop();
                self.chrome.announce_status(t!("develop-noise-ai-stopped"));
            }
            State::Missing(_) => {
                self.chrome.open_settings_at("ai.denoise");
            }
            State::Stopped | State::Failed(_) => {
                // Try again: forget why it stopped.
                if let Some(a) = ai::ai_module(&s.edit)
                    && let Some(p) = s.ai.photo
                {
                    let key = ai::key(&p, &s.edit, &a.model);
                    self.develop.denoise.held.retain(|h| h.0 != key);
                }
            }
            _ => {}
        }
        self.redraw = true;
    }
}

impl Session {
    /// The AI mode's status line for the Detail card.
    pub(super) fn denoise_line(&self) -> String {
        match &self.denoise.state {
            State::Classic => String::new(),
            State::Missing(m) => t!("develop-noise-ai-needs", model = m.clone()),
            State::Draft => t!("develop-noise-ai-draft"),
            State::Running(pm) => t!(
                "develop-noise-ai-running",
                percent = I18n::global()
                    .localizer()
                    .percent(f64::from(*pm / 10) / 100.0, 0)
            ),
            State::Ready => t!("develop-noise-ai-ready"),
            State::Stopped => t!("develop-noise-ai-stopped"),
            State::Failed(e) => t!("develop-noise-ai-failed", error = e.clone()),
        }
    }

    /// The card's button label (None: no button).
    pub(super) fn denoise_button(&self) -> Option<String> {
        match &self.denoise.state {
            State::Running(_) => Some(t!("develop-noise-ai-stop")),
            State::Missing(_) => Some(t!("develop-ai-open-settings")),
            State::Stopped | State::Failed(_) => Some(t!("develop-noise-ai-retry")),
            _ => None,
        }
    }
}

/// The fields for switching denoise to `mode`: the model named when switching to AI
/// (the installed one, else the default), the share kept.
pub(super) fn write_mode(f: &mut specular_develop::model::Fields, mode: Mode) {
    use specular_develop::model::Value;
    f.set("mode", Value::Text(mode.name().into()));
    if mode == Mode::Ai {
        let p = AiParams::from_fields(f);
        if p.model.is_empty() {
            let m = installed(None)
                .map(|m| m.id)
                .or_else(|| aid::default_model(&specular_ai::Manifest::load()));
            if let Some(m) = m {
                f.set("model", Value::Text(m));
            }
        }
        if f.get("blend").is_none() {
            f.set("blend", Value::Float(ai::DEFAULT_BLEND));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_size_is_one_screen_pixel_per_photo_pixel() {
        assert!(at_full_size(3000.0, 2.0, 6000.0));
        assert!(!at_full_size(1500.0, 2.0, 6000.0));
        assert!(!at_full_size(100.0, 1.0, 0.0));
    }

    #[test]
    fn the_cache_is_trimmed_oldest_first() {
        let dir = std::env::temp_dir().join(format!("specular-dn-trim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (i, n) in ["a", "b", "c"].iter().enumerate() {
            let p = dir.join(format!("{n}.dn"));
            std::fs::write(&p, vec![0u8; 100]).unwrap();
            let t =
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000 + i as u64);
            std::fs::File::options()
                .write(true)
                .open(&p)
                .unwrap()
                .set_modified(t)
                .unwrap();
        }
        std::fs::write(dir.join("other.txt"), vec![0u8; 500]).unwrap();
        trim(&dir, 200, &dir.join("a.dn"), &["dn"]);
        assert!(dir.join("a.dn").exists(), "the one just written stays");
        assert!(!dir.join("b.dn").exists());
        assert!(dir.join("c.dn").exists());
        assert!(dir.join("other.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn switching_to_ai_names_a_model_and_a_share() {
        let mut f = specular_develop::model::Fields::new();
        write_mode(&mut f, Mode::Ai);
        let p = AiParams::from_fields(&f);
        assert_eq!(p.mode, Mode::Ai);
        assert!(!p.model.is_empty());
        assert_eq!(p.blend, ai::DEFAULT_BLEND);
        write_mode(&mut f, Mode::Classic);
        assert_eq!(AiParams::from_fields(&f).mode, Mode::Classic);
    }

    #[test]
    fn frames_read_encoded() {
        let rgba = vec![0.18f32, 0.18, 0.18, 1.0, 1.0, 0.0, 0.0, 1.0];
        let f = Frame {
            w: 2,
            h: 1,
            rgba: &rgba,
            enc: Encoding::Srgb,
        };
        let mut out = vec![0f32; 3 * 3];
        f.fill(-1, 0, 3, 1, &mut out);
        // Clamped at the edge: the first two columns are the grey.
        assert_eq!(out[0], out[1]);
        assert!((out[0] - ai::oetf(0.18 * 1.0)).abs() < 0.05);
    }
}
