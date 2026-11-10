//! AI masks in Develop: "Subject", "Sky" and "Object"
//! rows in a module's Masks section, computed by the Masks feature's networks
//! (`specular_ai::masks`) and kept as rasters over the source frame
//! (`specular_develop::mask::ai`).
//!
//! - **Rows.** A new row names the installed model. Its key ties it to this photo; the
//!   sidecar keeps the row (kind, clicks, box, model), never the mask.
//! - **Computing.** A row without a raster asks the preview for the picture the networks
//!   see, then the `ai-masks` worker runs the network, upsamples the result guided by the
//!   picture and puts the raster in the store and the disk cache (`<cache>/develop-masks`).
//!   The panel says "Finding the subject…" meanwhile. A photo's EfficientSAM embedding is
//!   kept, so each new click costs a few milliseconds.
//! - **Missing model.** The row says which add-on it needs and links to Settings › Local
//!   AI; it masks nothing. Nothing downloads from here.
//! - **Clicks.** While an Object row is edited, a click adds a point on the object,
//!   Alt+click a point off it, a drag draws a box. Each is one history step.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use specular_ai::masks::{self as aim, Kind, MaskFiles, Masker, Rgba};
use specular_develop::mask::Geometry;
use specular_develop::mask::ai::{self, Picture};
use specular_develop::model::ModuleKind;
use specular_develop::model::mask::{AI_MAX_POINTS, AiKind, AiMask, AiPoint, Layer, MaskExpr};
use specular_develop::ops::geometry::Plan;
use specular_i18n::t;
use specular_infer::sam::{PointKind, Prompt};
use specular_widgets::mask_handles::{Handle, Outline, Role};

use super::controls::Session;
use super::masks::Map;
use crate::app::App;

/// How long the installed models are trusted before being looked up again (s).
const FILES_TTL_S: f64 = 3.0;
/// A press that moves less than this (logical px) is a click, else a box.
const CLICK_SLOP: f32 = 6.0;

/// The picture the networks see, ready to hand to the worker.
pub(super) struct AiPicture {
    pub w: u32,
    pub h: u32,
    /// The look's exposure (AI remove takes it out of removal patches).
    pub gain: f32,
    pub rgba8: Vec<u8>,
    luma: Vec<f32>,
    plan: Option<Plan>,
    /// The source frame's size (sensor pixels).
    pub frame: (f64, f64),
}

impl AiPicture {
    pub fn new(p: Picture, plan: Option<Plan>, frame: (f64, f64)) -> AiPicture {
        AiPicture {
            w: p.w,
            h: p.h,
            gain: p.gain(),
            rgba8: p.rgba8(),
            luma: p.luma(),
            plan,
            frame,
        }
    }

    pub(super) fn geometry(&self) -> Geometry<'_> {
        match &self.plan {
            Some(p) => Geometry::Warp(p),
            None => Geometry::Frame {
                w: self.frame.0,
                h: self.frame.1,
            },
        }
    }

    /// Where source-frame point `f` (fractions) is on the picture (fractions).
    pub(super) fn picture_point(&self, f: (f32, f32)) -> (f32, f32) {
        let Some(p) = &self.plan else {
            return f;
        };
        let [fx, fy, fw, fh] = p.frame;
        let s = (fx + f64::from(f.0) * fw, fy + f64::from(f.1) * fh);
        match p.to_output(s, 1.0) {
            Some(o) => {
                let c = p.crop_rect();
                ((o.0 / c[2].max(1e-9)) as f32, (o.1 / c[3].max(1e-9)) as f32)
            }
            None => f,
        }
    }

    /// The network's prompt for an object row.
    fn prompt(&self, m: &AiMask) -> Prompt {
        let mut points = Vec::new();
        if let Some([x0, y0, x1, y1]) = m.rect {
            let a = self.picture_point((x0, y0));
            let b = self.picture_point((x1, y1));
            points.push(([a.0.min(b.0), a.1.min(b.1)], PointKind::BoxTopLeft));
            points.push(([a.0.max(b.0), a.1.max(b.1)], PointKind::BoxBottomRight));
        }
        for p in &m.points {
            let q = self.picture_point((p.x, p.y));
            let kind = if p.positive {
                PointKind::Positive
            } else {
                PointKind::Negative
            };
            points.push(([q.0, q.1], kind));
        }
        points.truncate(specular_infer::sam::MAX_POINTS);
        Prompt { points }
    }
}

/// What an AI row is doing, for the panel.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum RowState {
    Computing,
    Ready,
    /// Its model is not installed: the model's name.
    Missing(String),
    /// An object row without clicks yet.
    NeedsClicks,
    Failed(String),
}

/// A Session's AI rows.
#[derive(Debug, Default)]
pub(super) struct SessionAi {
    /// The photo the rows are keyed for.
    pub photo: Option<[u8; 32]>,
    pub state: HashMap<[u8; 32], RowState>,
    /// The installed model per kind (subject, sky, object), for new rows.
    pub models: [Option<String>; 3],
    /// A press on the picture while an object row is edited: where (logical px).
    pub press: Option<(f32, f32)>,
}

fn kind_index(k: AiKind) -> usize {
    match k {
        AiKind::Subject => 0,
        AiKind::Sky => 1,
        AiKind::Object => 2,
    }
}

fn net_kind(k: AiKind) -> Kind {
    match k {
        AiKind::Subject => Kind::Subject,
        AiKind::Sky => Kind::Sky,
        AiKind::Object => Kind::Object,
    }
}

/// The photo's identity for keys and the cache: its path, size and modification time.
pub(super) fn photo_id(path: &Path) -> [u8; 32] {
    let (mut len, mut ns) = (0, 0);
    if let Ok(m) = std::fs::metadata(path) {
        len = m.len();
        if let Ok(t) = m.modified()
            && let Ok(d) = t.duration_since(std::time::UNIX_EPOCH)
        {
            ns = d.as_nanos();
        }
    }
    ai::photo_id(path.to_string_lossy().as_bytes(), len, ns)
}

/// The folder computed masks are kept in.
fn cache_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        if cfg!(test) {
            return std::env::temp_dir().join(format!("specular-ai-masks-{}", std::process::id()));
        }
        specular_settings::Locations::detect()
            .cache_dir
            .join("develop-masks")
    })
    .clone()
}

fn cache_file(key: &[u8; 32]) -> PathBuf {
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    cache_dir().join(format!("{hex}.mask"))
}

/// The Masks feature's installed files.
fn installed() -> MaskFiles {
    let data = specular_settings::Locations::detect().data_dir;
    let store = specular_ai::install::Store::new(&data);
    MaskFiles::find(
        &specular_ai::Manifest::load(),
        store.root(),
        &store.installed(),
    )
}

struct Job {
    photo: [u8; 32],
    row: AiMask,
    pic: Arc<AiPicture>,
    files: MaskFiles,
}

struct Done {
    key: [u8; 32],
    result: Result<(), String>,
}

/// The networks' worker thread and what is in flight.
#[derive(Default)]
pub(super) struct Worker {
    tx: Option<Sender<Job>>,
    rx: Option<Receiver<Done>>,
    files: Option<(MaskFiles, Instant)>,
    /// The picture for the open photo.
    pic: Option<([u8; 32], Arc<AiPicture>)>,
    asked: bool,
    inflight: HashSet<[u8; 32]>,
    failed: HashMap<[u8; 32], String>,
    loader_set: bool,
}

impl Worker {
    /// The picture for photo `photo` (the AI masks' photo id), once read (AI remove shares it).
    pub(super) fn picture(&self, photo: &[u8; 32]) -> Option<Arc<AiPicture>> {
        self.pic
            .as_ref()
            .filter(|(p, _)| p == photo)
            .map(|(_, a)| a.clone())
    }

    /// Asks the preview for the picture once: it arrives through the masks' tick.
    pub(super) fn ask_picture(&mut self, preview: &mut super::preview::Preview) {
        if !self.asked && self.pic.is_none() {
            self.asked = true;
            preview.want_ai_picture();
        }
    }
}

/// Runs one row: the network, the guided upsampling, the raster into the store and the
/// disk cache.
fn compute(
    m: &mut Masker,
    emb: &mut Option<([u8; 32], aim::Embedding)>,
    job: &Job,
) -> Result<(), String> {
    let pic = &job.pic;
    let img = Rgba {
        w: pic.w,
        h: pic.h,
        data: &pic.rgba8,
    };
    let e = |e: specular_infer::Error| e.to_string();
    let prob = match job.row.kind {
        AiKind::Subject => m.subject(img).map_err(e)?,
        AiKind::Sky => m.sky(img).map_err(e)?,
        AiKind::Object => {
            let model = m.model_id(Kind::Object).unwrap_or_default().to_owned();
            if emb
                .as_ref()
                .is_none_or(|(p, x)| *p != job.photo || x.model != model)
            {
                *emb = Some((job.photo, m.embed(img).map_err(e)?));
            }
            let (_, x) = emb.as_ref().expect("set above");
            m.object(x, &pic.prompt(&job.row)).map_err(e)?
        }
    };
    let (gw, gh) = (pic.w as usize, pic.h as usize);
    let cells = prob.w.max(prob.h);
    let q = ai::guided_upsample(
        &prob.data,
        (prob.w, prob.h),
        &pic.luma,
        (gw, gh),
        ai::guide_radius(gw.max(gh), cells),
        1e-3,
    );
    let r = ai::frame_raster(&q, (gw, gh), &pic.geometry(), pic.frame);
    let _ = std::fs::create_dir_all(cache_dir());
    let tmp = cache_file(&job.row.key).with_extension("part");
    if std::fs::write(&tmp, ai::encode(&r)).is_ok() {
        let _ = std::fs::rename(&tmp, cache_file(&job.row.key));
    }
    ai::store()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .put(job.row.key, r);
    Ok(())
}

impl Worker {
    fn start(
        &mut self,
        gpu: Option<(specular_gfx::wgpu::Device, specular_gfx::wgpu::Queue)>,
        waker: crate::loader::Wake,
    ) {
        let (jtx, jrx) = channel::<Job>();
        let (dtx, drx) = channel::<Done>();
        let spawned = specular_thread::builder("ai-masks").spawn(move || {
            // After a GPU reset the networks are built again on the new device.
            let mut ai = crate::app::ai::device::Session::new(gpu);
            let mut ctx = aim::choose_gpu(ai.context(), specular_core::paths::selector()).cloned();
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
            let mut masker: Option<Masker> = None;
            let mut emb = None;
            while let Ok(job) = jrx.recv() {
                if ai.stale() {
                    ctx = aim::choose_gpu(ai.context(), specular_core::paths::selector()).cloned();
                    masker = None;
                }
                if masker.as_ref().is_none_or(|m| *m.files() != job.files) {
                    masker = Some(Masker::new(job.files.clone(), ctx.clone(), threads));
                    emb = None;
                }
                let m = masker.as_mut().expect("made above");
                let t = Instant::now();
                let result = compute(m, &mut emb, &job);
                specular_log::info!(target: specular_log::JOBS,
                    "develop: AI {} mask in {:.0} ms{}",
                    kind_id(job.row.kind),
                    t.elapsed().as_secs_f64() * 1e3,
                    result.as_ref().err().map_or(String::new(), |e| format!(" ({e})")));
                if dtx
                    .send(Done {
                        key: job.row.key,
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

    fn files(&mut self) -> MaskFiles {
        let fresh = self
            .files
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed().as_secs_f64() < FILES_TTL_S);
        if !fresh {
            self.files = Some((installed(), Instant::now()));
        }
        self.files.as_ref().map(|f| f.0.clone()).unwrap_or_default()
    }
}

pub(super) fn kind_id(k: AiKind) -> &'static str {
    net_kind(k).id()
}

/// The name of the model `m` needs (for "needs the … add-on").
fn model_name(m: &AiMask) -> String {
    let manifest = specular_ai::Manifest::load();
    manifest
        .choices
        .iter()
        .find(|c| {
            c.feature == specular_ai::Feature::Masks
                && manifest.models.iter().any(|x| {
                    x.id == m.model
                        && x.feature == specular_ai::Feature::Masks
                        && match (&c.slot, &x.addon) {
                            (specular_ai::manifest::Slot::Addon(a), Some(b)) => a == b,
                            (specular_ai::manifest::Slot::Tier(t), None) => x.tiers.contains(t),
                            _ => false,
                        }
                })
        })
        .map_or_else(
            || aim::model_name(&manifest, net_kind(m.kind)),
            |c| c.name.clone(),
        )
}

/// What `m` needs before it can be computed: None when it can be.
fn blocked(m: &AiMask, files: &MaskFiles) -> Option<RowState> {
    if m.kind == AiKind::Object && m.points.is_empty() && m.rect.is_none() {
        return Some(RowState::NeedsClicks);
    }
    match files.get(net_kind(m.kind)) {
        Some(f) if m.model.is_empty() || f.id == m.model => None,
        _ => Some(RowState::Missing(model_name(m))),
    }
}

impl App {
    /// AI rows: keys, cached or computed rasters, the panel's states.
    pub(super) fn develop_ai_tick(&mut self) {
        let Some(s) = self.develop.session.as_mut() else {
            return;
        };
        let photo = *s.ai.photo.get_or_insert_with(|| photo_id(&s.path));
        let mut changed = ai::rekey(&mut s.edit, &photo);
        let rows = ai::rows(&s.edit);
        let w = &mut self.develop.ai;
        if !w.loader_set {
            w.loader_set = true;
            ai::store()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .set_loader(Some(Box::new(|k| {
                    std::fs::read(cache_file(k))
                        .ok()
                        .and_then(|b| ai::decode(&b))
                })));
        }
        let files = if rows.is_empty() && s.ai.models.iter().all(Option::is_some) {
            MaskFiles::default()
        } else {
            w.files()
        };
        if !rows.is_empty() || s.ai.models.iter().all(Option::is_none) {
            let ids = [Kind::Subject, Kind::Sky, Kind::Object]
                .map(|k| files.get(k).map(|f| f.id.clone()));
            if ids != s.ai.models {
                s.ai.models = ids;
                changed = true;
            }
        }
        // Finished work.
        let mut arrived = false;
        if let Some(rx) = &w.rx {
            while let Ok(d) = rx.try_recv() {
                w.inflight.remove(&d.key);
                if let Err(e) = d.result {
                    w.failed.insert(d.key, e);
                }
                arrived = true;
            }
        }
        if w.pic.as_ref().is_some_and(|(p, _)| *p != photo) {
            w.pic = None;
            w.asked = false;
        }
        if let Some(r) = self.develop.preview.take_ai_picture() {
            w.asked = false;
            match r {
                Ok(p) => w.pic = Some((photo, Arc::new(p))),
                Err(e) => specular_log::warn!(target: specular_log::JOBS,
                    "develop: the picture for AI masks: {e}"),
            }
        }
        let mut states = HashMap::new();
        let mut wanted = Vec::new();
        {
            let mut st = ai::store().lock().unwrap_or_else(|p| p.into_inner());
            for m in rows {
                let state = if st.get(&m.key).is_some() {
                    RowState::Ready
                } else if w.inflight.contains(&m.key) {
                    RowState::Computing
                } else if let Some(e) = w.failed.get(&m.key) {
                    RowState::Failed(e.clone())
                } else if let Some(b) = blocked(&m, &files) {
                    b
                } else {
                    wanted.push(m.clone());
                    RowState::Computing
                };
                states.insert(m.key, state);
            }
        }
        if !wanted.is_empty() {
            match w.pic.clone() {
                Some((_, pic)) => {
                    if w.tx.is_none() {
                        let gpu = self
                            .gfx
                            .as_ref()
                            .map(|g| (g.gpu.device.clone(), g.gpu.queue.clone()));
                        w.start(gpu, self.loader.waker());
                    }
                    for m in wanted {
                        let job = Job {
                            photo,
                            row: m.clone(),
                            pic: pic.clone(),
                            files: files.clone(),
                        };
                        let sent = w.tx.as_ref().is_some_and(|tx| tx.send(job).is_ok());
                        if sent {
                            w.inflight.insert(m.key);
                        } else {
                            w.failed.insert(m.key, t!("develop-ai-failed-worker"));
                        }
                    }
                }
                None if !w.asked => {
                    w.asked = true;
                    self.develop.preview.want_ai_picture();
                }
                None => {}
            }
        }
        let s = self.develop.session.as_mut().expect("checked above");
        if states != s.ai.state {
            s.ai.state = states;
            changed = true;
        }
        if arrived {
            self.develop.preview.invalidate();
            self.redraw = true;
        }
        if changed {
            s.sync_panel();
            self.redraw = true;
        }
    }

    /// A press on the picture while an Object row is edited: remembered for the release.
    pub(super) fn develop_ai_press(&mut self, lp: (f32, f32)) -> bool {
        let Some(s) = self.develop.session.as_mut() else {
            return false;
        };
        if s.ai_object_active().is_none() {
            return false;
        }
        s.ai.press = Some(lp);
        true
    }

    /// The press ended: a click adds a point, a drag sets the box.
    pub(super) fn develop_ai_release(&mut self) -> bool {
        let ms = self.develop_ms();
        let lp = self.develop.pointer;
        let alt = self.mods.alt_key();
        let (Some(s), Some(map)) = (self.develop.session.as_mut(), self.develop.map.as_ref())
        else {
            return false;
        };
        let Some(start) = s.ai.press.take() else {
            return false;
        };
        let end = lp.unwrap_or(start);
        let said = s.ai_add(map, start, end, !alt, ms);
        if let Some(said) = said {
            self.chrome.announce_status(said);
            self.develop.unsaved = Some(self.now());
        }
        self.redraw = true;
        true
    }
}

impl Session {
    /// A new AI row of `kind` with the installed model (or the default one, which the row
    /// will say is missing).
    pub(super) fn new_ai_row(&self, kind: AiKind) -> AiMask {
        let model = self.ai.models[kind_index(kind)].clone().unwrap_or_else(|| {
            let manifest = specular_ai::Manifest::load();
            let k = net_kind(kind);
            manifest
                .models
                .iter()
                .find(|m| {
                    m.feature == specular_ai::Feature::Masks
                        && m.role == k.role()
                        && (k.addon().is_some()
                            || m.tiers.first() == Some(&specular_ai::Tier::Small))
                })
                .map(|m| m.id.clone())
                .unwrap_or_default()
        });
        let mut m = AiMask::new(kind, &model);
        if let Some(p) = &self.ai.photo {
            ai::rekey_one(&mut m, p);
        }
        m
    }

    /// The Object row being edited: its module and row.
    pub(super) fn ai_object_active(&self) -> Option<(ModuleKind, usize)> {
        let a = self.masks.active?;
        if self.inst(a.kind) != a.inst {
            return None;
        }
        let layers = self.layers_of(a.kind);
        match &layers.get(a.row)?.item {
            MaskExpr::Ai(m) if m.kind == AiKind::Object => Some((a.kind, a.row)),
            _ => None,
        }
    }

    fn ai_row_mut(
        &mut self,
        f: impl FnOnce(&mut AiMask) -> Option<String>,
        now_ms: i64,
    ) -> Option<String> {
        let (kind, row) = self.ai_object_active()?;
        let mut layers: Vec<Layer> = self.layers_of(kind);
        let MaskExpr::Ai(m) = &mut layers.get_mut(row)?.item else {
            return None;
        };
        let said = f(m)?;
        if let Some(p) = &self.ai.photo {
            ai::rekey_one(m, p);
        }
        self.begin(kind);
        self.set_layers(kind, &layers);
        self.commit(now_ms);
        Some(said)
    }

    /// A click (`start` ≈ `end`) adds a point on (`positive`) or off the object; a drag
    /// sets the box. What to say, or None when nothing changed.
    pub(super) fn ai_add(
        &mut self,
        map: &Map,
        start: (f32, f32),
        end: (f32, f32),
        positive: bool,
        now_ms: i64,
    ) -> Option<String> {
        let a = map.to_frame(start);
        let b = map.to_frame(end);
        let drag = (start.0 - end.0).hypot(start.1 - end.1) > CLICK_SLOP;
        self.ai_row_mut(
            |m| {
                let f = |v: f64| v.clamp(0.0, 1.0) as f32;
                if drag {
                    if m.points.len() + 2 > AI_MAX_POINTS {
                        return Some(t!("develop-ai-too-many", n = AI_MAX_POINTS as i64));
                    }
                    m.rect = Some([
                        f(a.0.min(b.0)),
                        f(a.1.min(b.1)),
                        f(a.0.max(b.0)),
                        f(a.1.max(b.1)),
                    ]);
                    return Some(t!("develop-ai-box-set"));
                }
                if m.prompt_len() + 1 > AI_MAX_POINTS {
                    return Some(t!("develop-ai-too-many", n = AI_MAX_POINTS as i64));
                }
                m.points.push(AiPoint {
                    x: f(a.0),
                    y: f(a.1),
                    positive,
                });
                Some(if positive {
                    t!("develop-ai-point-added")
                } else {
                    t!("develop-ai-point-removed")
                })
            },
            now_ms,
        )
    }

    /// Takes every click and the box off the edited Object row.
    pub(super) fn ai_clear(&mut self, now_ms: i64) -> Option<String> {
        self.ai_row_mut(
            |m| {
                if m.points.is_empty() && m.rect.is_none() {
                    return None;
                }
                m.points.clear();
                m.rect = None;
                Some(t!("develop-ai-cleared"))
            },
            now_ms,
        )
    }

    /// The edited Object row's clicks and box on screen.
    pub(super) fn ai_handles(&self, map: &Map) -> Option<(Vec<Handle>, Vec<Outline>)> {
        let (kind, row) = self.ai_object_active()?;
        let layers = self.layers_of(kind);
        let MaskExpr::Ai(m) = &layers.get(row)?.item else {
            return None;
        };
        let mut hs = Vec::new();
        for (i, p) in m.points.iter().enumerate() {
            if let Some(at) = map.to_screen((f64::from(p.x), f64::from(p.y))) {
                hs.push(Handle {
                    role: if p.positive {
                        Role::Centre
                    } else {
                        Role::Point
                    },
                    at,
                    name: if p.positive {
                        t!("develop-ai-point-on", n = i as i64 + 1)
                    } else {
                        t!("develop-ai-point-off", n = i as i64 + 1)
                    },
                });
            }
        }
        let mut ol = Vec::new();
        if let Some([x0, y0, x1, y1]) = m.rect {
            let pts: Vec<(f32, f32)> = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
                .into_iter()
                .filter_map(|(x, y)| map.to_screen((f64::from(x), f64::from(y))))
                .collect();
            ol.push(Outline {
                points: pts,
                closed: true,
                faint: false,
            });
        }
        Some((hs, ol))
    }

    /// The state line of an AI row for the panel.
    pub(super) fn ai_state_line(&self, m: &AiMask) -> String {
        match self.ai.state.get(&m.key) {
            Some(RowState::Ready) => match m.kind {
                AiKind::Object => format!(
                    "{}\n{}",
                    t!("develop-ai-object-clicks", n = m.prompt_len() as i64),
                    t!("develop-ai-click-help")
                ),
                _ => t!("develop-ai-ready"),
            },
            Some(RowState::Missing(name)) => t!("develop-ai-needs", model = name.clone()),
            Some(RowState::NeedsClicks) => t!("develop-ai-click-help"),
            Some(RowState::Failed(e)) => t!("develop-ai-failed", error = e.clone()),
            Some(RowState::Computing) | None => t!("develop-ai-computing", kind = kind_id(m.kind)),
        }
    }

    /// Whether the row's model is missing (its link to Local AI shows).
    pub(super) fn ai_missing(&self, m: &AiMask) -> bool {
        matches!(self.ai.state.get(&m.key), Some(RowState::Missing(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photo_ids_differ_by_path() {
        let a = photo_id(Path::new("/nonexistent/a.jpg"));
        let b = photo_id(Path::new("/nonexistent/b.jpg"));
        assert_ne!(a, b);
        assert_eq!(a, photo_id(Path::new("/nonexistent/a.jpg")));
    }

    #[test]
    fn rows_say_what_they_need() {
        let files = MaskFiles::default();
        let o = AiMask::new(AiKind::Object, "efficientsam-ti");
        assert_eq!(blocked(&o, &files), Some(RowState::NeedsClicks));
        let s = AiMask::new(AiKind::Subject, "isnet-general-use");
        assert_eq!(
            blocked(&s, &files),
            Some(RowState::Missing("IS-Net".into()))
        );
        let k = AiMask::new(AiKind::Sky, "upernet-convnext-tiny-ade20k");
        assert_eq!(
            blocked(&k, &files),
            Some(RowState::Missing("UperNet ConvNeXt-T".into()))
        );
        let mut o = AiMask::new(AiKind::Object, "efficientsam-s");
        o.points.push(AiPoint {
            x: 0.5,
            y: 0.5,
            positive: true,
        });
        assert_eq!(
            blocked(&o, &files),
            Some(RowState::Missing("EfficientSAM S".into()))
        );
        let files = MaskFiles {
            subject: Some(aim::MaskModel {
                path: PathBuf::from("/x"),
                id: "isnet-general-use".into(),
            }),
            ..Default::default()
        };
        assert_eq!(blocked(&s, &files), None);
    }

    #[test]
    fn the_prompt_follows_the_picture() {
        let pic = AiPicture {
            w: 4,
            h: 2,
            gain: 1.0,
            rgba8: vec![0; 32],
            luma: vec![0.0; 8],
            plan: None,
            frame: (4.0, 2.0),
        };
        let mut m = AiMask::new(AiKind::Object, "efficientsam-ti");
        m.rect = Some([0.6, 0.7, 0.2, 0.1]);
        m.points.push(AiPoint {
            x: 0.3,
            y: 0.4,
            positive: false,
        });
        let p = pic.prompt(&m);
        assert_eq!(p.points.len(), 3);
        assert_eq!(p.points[0], ([0.2, 0.1], PointKind::BoxTopLeft));
        assert_eq!(p.points[1], ([0.6, 0.7], PointKind::BoxBottomRight));
        assert_eq!(p.points[2], ([0.3, 0.4], PointKind::Negative));
    }
}
