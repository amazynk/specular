//! "Remove (AI)" spots and the depth map in Develop,
//! computed by the Remove and Depth features' networks (`specular_ai::remove`).
//!
//! - **Removal.** A Remove spot names the installed inpainting model. Without its patch
//!   it heals from its automatic source. The worker crops the AI masks' picture
//!   around the stroke, fills the stroke with the network, and keeps the result as a patch
//!   over the source frame (`specular_develop::ops::retouch::patch`) in memory and the cache
//!   (`<cache>/develop-ai`, kept under [`CACHE_BYTES`], oldest first); the spot then heals
//!   from the patch.
//! - **Depth.** Depth rows and the depth modules name the depth model; the worker runs it
//!   once per photo and keeps the map (`specular_develop::mask::depth`) the same way.
//! - **States.** Each spot and the depth map say what they are doing for the panel:
//!   computing (with Stop), ready, needs the add-on (with a link to Settings › Local AI) or
//!   failed. Stop ends the work in hand; the spot or map is computed again only when asked
//!   (Retry, or a change). Nothing downloads from here.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use specular_ai::Feature;
use specular_ai::remove::{self as rm, DepthNet, Inpainter, NetModel};
use specular_develop::mask::depth as dm;
use specular_develop::model::Edit;
use specular_develop::ops::retouch::{Retouch, Spot, SpotMode, patch};
use specular_i18n::t;

use super::ai_masks::AiPicture;
use crate::app::App;

/// How long the installed models are trusted before being looked up again (s).
const FILES_TTL_S: f64 = 3.0;
/// The depth raster's longest side (the map is smooth; edges come from the render).
const DEPTH_LONG: usize = 1024;

/// What a removal patch or the depth map is doing, for the panel.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum NetState {
    Computing,
    Ready,
    /// Its model is not installed: the feature's name in Settings › Local AI.
    Missing(String),
    /// Stopped by the user.
    Stopped,
    Failed(String),
}

/// What the panel asks of the worker (Stop, or compute one result again).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ask {
    Stop,
    Retry([u8; 32]),
}

/// The installed models of both features.
#[derive(Clone, Debug, Default, PartialEq)]
struct Files {
    remove: Option<NetModel>,
    depth: Option<NetModel>,
}

fn installed() -> Files {
    let data = specular_settings::Locations::detect().data_dir;
    let store = specular_ai::install::Store::new(&data);
    let m = specular_ai::Manifest::load();
    let inst = store.installed();
    Files {
        remove: rm::installed_model(&m, store.root(), &inst, Feature::Remove),
        depth: rm::installed_model(&m, store.root(), &inst, Feature::Depth),
    }
}

/// The folder computed patches and depth maps are kept in.
fn cache_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        if cfg!(test) {
            return std::env::temp_dir()
                .join(format!("specular-ai-develop-{}", std::process::id()));
        }
        specular_settings::Locations::detect()
            .cache_dir
            .join("develop-ai")
    })
    .clone()
}

fn cache_file(key: &[u8; 32], ext: &str) -> PathBuf {
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    cache_dir().join(format!("{hex}.{ext}"))
}

/// Bytes of patches and depth maps kept on disk (as Denoise's `develop-denoise`).
pub(super) const CACHE_BYTES: u64 = super::ai_denoise::CACHE_BYTES;

/// The kinds of file the cache holds.
const CACHE_EXTS: [&str; 2] = ["patch", "depth"];

fn write_cache(key: &[u8; 32], ext: &str, bytes: &[u8]) {
    write_cache_in(&cache_dir(), &cache_file(key, ext), bytes, CACHE_BYTES);
}

/// Writes `bytes` to `f` in `dir`, then trims the folder's cache files to `limit` bytes,
/// oldest first (`f` stays).
fn write_cache_in(dir: &std::path::Path, f: &std::path::Path, bytes: &[u8], limit: u64) {
    let _ = std::fs::create_dir_all(dir);
    let tmp = f.with_extension("part");
    if std::fs::write(&tmp, bytes).is_ok() {
        let _ = std::fs::rename(&tmp, f);
    }
    super::ai_denoise::trim(dir, limit, f, &CACHE_EXTS);
}

enum Job {
    Remove {
        key: [u8; 32],
        spot: Spot,
        pic: Arc<AiPicture>,
        model: NetModel,
    },
    Depth {
        key: [u8; 32],
        pic: Arc<AiPicture>,
        model: NetModel,
    },
}

impl Job {
    fn key(&self) -> [u8; 32] {
        match self {
            Job::Remove { key, .. } | Job::Depth { key, .. } => *key,
        }
    }
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
    files: Option<(Files, Instant)>,
    inflight: HashSet<[u8; 32]>,
    failed: HashMap<[u8; 32], NetState>,
    /// Set by Stop: the worker drops what it has queued and the job in hand ends as soon
    /// as it can.
    stop: Arc<AtomicBool>,
    loader_set: bool,
    /// Per key, what the panel shows.
    pub state: HashMap<[u8; 32], NetState>,
}

/// The spot's stroke and size in picture pixels.
fn stroke_on(pic: &AiPicture, s: &Spot) -> (Vec<(f64, f64)>, f64) {
    let pts: Vec<[f32; 2]> = if s.path.is_empty() {
        vec![s.target]
    } else {
        s.path.clone()
    };
    let (w, h) = (f64::from(pic.w), f64::from(pic.h));
    let pts = pts
        .iter()
        .map(|p| {
            let (u, v) = pic.picture_point((p[0], p[1]));
            (f64::from(u) * w, f64::from(v) * h)
        })
        .collect();
    // The picture is the whole frame upright: its long side is the frame's long side.
    (pts, f64::from(s.size) * w.max(h))
}

/// One removal: crop, hole, network, patch.
fn remove(
    net: &mut Inpainter,
    pic: &AiPicture,
    s: &Spot,
    stop: &dyn Fn() -> bool,
) -> Result<patch::Patch, String> {
    let (pts, r) = stroke_on(pic, s);
    let crop = patch::crop_for(&pts, r, (pic.w, pic.h));
    let long = (crop[2] - crop[0]).max(crop[3] - crop[1]).round() as usize;
    let side = rm::work_side(long);
    let n = (side, side);
    let rgb = patch::crop_rgb(&pic.rgba8, (pic.w, pic.h), crop, n);
    // A margin around the stroke: the network must not see the object's fringe.
    let grow = (0.15 * r).max(2.0 * long as f64 / side as f64);
    let hole = patch::hole(&pts, r, grow, crop, n);
    let out = net
        .fill(
            &rm::Rgb {
                w: side,
                h: side,
                data: rgb,
            },
            &hole,
            stop,
        )
        .map_err(|e| e.to_string())?;
    let g = pic.geometry();
    let to_frame = |u: f64, v: f64| g.frame_point(u, v);
    let to_picture = |fx: f64, fy: f64| {
        let (u, v) = pic.picture_point((fx as f32, fy as f32));
        (f64::from(u), f64::from(v))
    };
    Ok(patch::from_network(
        &out.data,
        n,
        crop,
        (pic.w, pic.h),
        &to_frame,
        &to_picture,
        pic.gain,
    ))
}

/// The depth map of the picture.
fn depth(
    net: &mut DepthNet,
    pic: &AiPicture,
) -> Result<specular_develop::mask::brush::Raster, String> {
    let img = specular_ai::masks::Rgba {
        w: pic.w,
        h: pic.h,
        data: &pic.rgba8,
    };
    let inv = net.depth(img).map_err(|e| e.to_string())?;
    let (pw, ph) = (pic.w as usize, pic.h as usize);
    let k = (DEPTH_LONG as f64 / pw.max(ph) as f64).min(1.0);
    let (gw, gh) = (
        ((pw as f64 * k).round() as usize).max(1),
        ((ph as f64 * k).round() as usize).max(1),
    );
    Ok(dm::raster(
        &inv.data,
        (inv.w, inv.h),
        (gw, gh),
        &pic.geometry(),
        pic.frame,
    ))
}

impl Worker {
    fn start(
        &mut self,
        gpu: Option<(specular_gfx::wgpu::Device, specular_gfx::wgpu::Queue)>,
        waker: crate::loader::Wake,
    ) {
        let (jtx, jrx) = channel::<Job>();
        let (dtx, drx) = channel::<Done>();
        let stop = self.stop.clone();
        let spawned = specular_thread::builder("ai-develop").spawn(move || {
            // After a GPU reset the networks are built again on the new device.
            let mut ai = crate::app::ai::device::Session::new(gpu);
            let choose = |ai: &mut crate::app::ai::device::Session| {
                specular_ai::masks::choose_gpu(ai.context(), specular_core::paths::selector())
                    .cloned()
            };
            let mut ctx = choose(&mut ai);
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
            let mut inpainter: Option<Inpainter> = None;
            let mut depth_net: Option<DepthNet> = None;
            while let Ok(job) = jrx.recv() {
                if ai.stale() {
                    ctx = choose(&mut ai);
                    inpainter = None;
                    depth_net = None;
                }
                let key = job.key();
                let stopped = || stop.load(Ordering::Relaxed);
                let t = Instant::now();
                let result = if stopped() {
                    Err(String::new())
                } else {
                    match job {
                        Job::Remove {
                            key,
                            spot,
                            pic,
                            model,
                        } => {
                            if inpainter.as_ref().is_none_or(|n| *n.model() != model) {
                                inpainter = Some(Inpainter::new(model, ctx.clone(), threads));
                            }
                            let net = inpainter.as_mut().expect("made above");
                            remove(net, &pic, &spot, &stopped).map(|p| {
                                write_cache(&key, "patch", &patch::encode(&p));
                                patch::store()
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .put(key, p);
                            })
                        }
                        Job::Depth { key, pic, model } => {
                            if depth_net.as_ref().is_none_or(|n| *n.model() != model) {
                                depth_net = Some(DepthNet::new(model, ctx.clone(), threads));
                            }
                            let net = depth_net.as_mut().expect("made above");
                            depth(net, &pic).map(|r| {
                                write_cache(&key, "depth", &dm::encode(&r));
                                specular_develop::mask::ai::store()
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .put(key, r);
                            })
                        }
                    }
                };
                specular_log::info!(target: specular_log::JOBS,
                    "develop: AI {} in {:.0} ms{}",
                    if result.is_ok() { "result" } else { "work ended" },
                    t.elapsed().as_secs_f64() * 1e3,
                    result.as_ref().err().map_or(String::new(), |e| format!(" ({e})")));
                if dtx.send(Done { key, result }).is_err() {
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

    fn files(&mut self) -> Files {
        let fresh = self
            .files
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed().as_secs_f64() < FILES_TTL_S);
        if !fresh {
            self.files = Some((installed(), Instant::now()));
        }
        self.files.as_ref().map(|f| f.0.clone()).unwrap_or_default()
    }

    /// Stops the work in hand and queued: what was computing says "Stopped".
    pub(super) fn stop_all(&mut self) {
        if self.inflight.is_empty() {
            return;
        }
        self.stop.store(true, Ordering::Relaxed);
        for k in self.inflight.drain() {
            self.failed.insert(k, NetState::Stopped);
        }
    }

    /// Forgets a stop or failure of `key`, so it is computed again.
    pub(super) fn retry(&mut self, key: &[u8; 32]) {
        self.failed.remove(key);
    }
}

/// The removal spots of `edit`'s retouch module that name a model, with their keys.
pub(super) fn remove_spots(edit: &Edit) -> Vec<([u8; 32], Spot)> {
    let photo = dm::photo(&edit.source);
    edit.modules
        .iter()
        .filter(|m| m.enabled && m.kind == specular_develop::model::ModuleKind::RETOUCH)
        .flat_map(|m| Retouch::of(&m.params).spots)
        .filter(|s| s.mode == SpotMode::Remove && !s.model.is_empty() && s.opacity > 0.0)
        .map(|s| (patch::key(&photo, &s), s))
        .collect()
}

/// The installed model id of a feature, else its default's id.
pub(super) fn model_id(f: Feature) -> String {
    let files = installed();
    let got = match f {
        Feature::Remove => files.remove,
        _ => files.depth,
    };
    got.map(|m| m.id).unwrap_or_else(|| {
        let m = specular_ai::Manifest::load();
        m.models
            .iter()
            .find(|x| x.feature == f)
            .map(|x| x.id.clone())
            .unwrap_or_default()
    })
}

/// The panel line of a state.
pub(super) fn state_line(st: Option<&NetState>, computing: String) -> String {
    match st {
        Some(NetState::Ready) => t!("develop-ai-ready"),
        Some(NetState::Missing(feature)) => t!("develop-ai-net-needs", feature = feature.clone()),
        Some(NetState::Stopped) => t!("develop-ai-net-stopped"),
        Some(NetState::Failed(e)) => t!("develop-ai-net-failed", error = e.clone()),
        Some(NetState::Computing) | None => computing,
    }
}

impl App {
    /// Removal patches and the depth map: keys, cached or computed results, the states.
    pub(super) fn develop_ai2_tick(&mut self) {
        let Some(s) = self.develop.session.as_mut() else {
            return;
        };
        let spots = remove_spots(&s.edit);
        let photo = dm::photo(&s.edit.source);
        let depths: Vec<([u8; 32], String)> = dm::models(&s.edit)
            .into_iter()
            .map(|m| (dm::key(&photo, &m), m))
            .collect();
        let masks_photo = s.ai.photo;
        // A spot being dragged is filled once it is let go.
        let dragging = s.retouch_state.drag.is_some();
        let ask = s.ai2_request.take();
        let w = &mut self.develop.ai2;
        match ask {
            Some(Ask::Stop) => w.stop_all(),
            Some(Ask::Retry(k)) => w.retry(&k),
            None => {}
        }
        if spots.is_empty() && depths.is_empty() && w.inflight.is_empty() {
            if !w.state.is_empty() {
                w.state.clear();
                if let Some(s) = self.develop.session.as_mut() {
                    s.ai2_state.clear();
                    s.sync_panel();
                }
            }
            return;
        }
        if !w.loader_set {
            w.loader_set = true;
            patch::store()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .set_loader(Some(Box::new(|k| {
                    std::fs::read(cache_file(k, "patch"))
                        .ok()
                        .and_then(|b| patch::decode(&b))
                })));
        }
        let files = w.files();
        // Finished work.
        let mut arrived = false;
        if let Some(rx) = &w.rx {
            while let Ok(d) = rx.try_recv() {
                if w.inflight.remove(&d.key)
                    && let Err(e) = d.result
                {
                    let st = if e.is_empty() || w.stop.load(Ordering::Relaxed) {
                        NetState::Stopped
                    } else {
                        NetState::Failed(e)
                    };
                    w.failed.insert(d.key, st);
                }
                arrived = true;
            }
        }
        if w.inflight.is_empty() {
            w.stop.store(false, Ordering::Relaxed);
        }
        let pic = masks_photo.and_then(|p| self.develop.ai.picture(&p));
        let mut states = HashMap::new();
        let mut wanted: Vec<Job> = Vec::new();
        let mut need_pic = false;
        {
            let mut pst = patch::store().lock().unwrap_or_else(|e| e.into_inner());
            for (key, spot) in &spots {
                let st = if pst.get(key).is_some() {
                    NetState::Ready
                } else if w.inflight.contains(key) {
                    NetState::Computing
                } else if let Some(f) = w.failed.get(key) {
                    f.clone()
                } else {
                    match &files.remove {
                        Some(_) if dragging => NetState::Computing,
                        Some(m) if m.id == spot.model => match &pic {
                            Some(p) => {
                                wanted.push(Job::Remove {
                                    key: *key,
                                    spot: spot.clone(),
                                    pic: p.clone(),
                                    model: m.clone(),
                                });
                                NetState::Computing
                            }
                            None => {
                                need_pic = true;
                                NetState::Computing
                            }
                        },
                        _ => NetState::Missing(t!("settings-ai-remove")),
                    }
                };
                states.insert(*key, st);
            }
        }
        {
            let mut st = specular_develop::mask::ai::store()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            for (key, model) in &depths {
                if !st.contains(key)
                    && let Ok(b) = std::fs::read(cache_file(key, "depth"))
                    && let Some(r) = dm::decode(&b)
                {
                    st.put(*key, r);
                    arrived = true;
                }
                let state = if st.contains(key) {
                    NetState::Ready
                } else if w.inflight.contains(key) {
                    NetState::Computing
                } else if let Some(f) = w.failed.get(key) {
                    f.clone()
                } else {
                    match &files.depth {
                        Some(m) if m.id == *model => match &pic {
                            Some(p) => {
                                wanted.push(Job::Depth {
                                    key: *key,
                                    pic: p.clone(),
                                    model: m.clone(),
                                });
                                NetState::Computing
                            }
                            None => {
                                need_pic = true;
                                NetState::Computing
                            }
                        },
                        _ => NetState::Missing(t!("settings-ai-depth")),
                    }
                };
                states.insert(*key, state);
            }
        }
        if need_pic {
            self.develop.ai.ask_picture(&mut self.develop.preview);
        }
        let w = &mut self.develop.ai2;
        if !wanted.is_empty() {
            if w.tx.is_none() {
                let gpu = self
                    .gfx
                    .as_ref()
                    .map(|g| (g.gpu.device.clone(), g.gpu.queue.clone()));
                w.start(gpu, self.loader.waker());
            }
            for job in wanted {
                let key = job.key();
                let sent = w.tx.as_ref().is_some_and(|tx| tx.send(job).is_ok());
                if sent {
                    w.inflight.insert(key);
                } else {
                    w.failed
                        .insert(key, NetState::Failed(t!("develop-ai-net-worker-stopped")));
                    states.insert(key, w.failed[&key].clone());
                }
            }
        }
        let changed = states != w.state;
        if changed {
            w.state = states;
        }
        let mirror = w.state.clone();
        if arrived {
            self.develop.preview.invalidate();
            self.redraw = true;
        }
        if changed && let Some(s) = self.develop.session.as_mut() {
            s.ai2_state = mirror;
            s.sync_panel();
            self.redraw = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_develop::model::{ModuleInstance, ModuleKind, Params, Registry, SourceKey};

    #[test]
    fn only_named_removal_spots_are_computed() {
        let reg = Registry::builtin();
        let mut e = Edit::empty(SourceKey::default());
        let mut m = ModuleInstance::new(reg, ModuleKind::RETOUCH, 0);
        let r = Retouch {
            spots: vec![
                Spot {
                    mode: SpotMode::Remove,
                    model: "big-lama".into(),
                    ..Spot::default()
                },
                Spot {
                    mode: SpotMode::Remove,
                    ..Spot::default()
                },
                Spot::default(),
            ],
            ..Retouch::default()
        };
        m.params = Params::from_fields(ModuleKind::RETOUCH, r.to_fields());
        e.insert(reg, m);
        let v = remove_spots(&e);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].1.model, "big-lama");
    }

    #[test]
    fn the_cache_keeps_under_its_cap_oldest_first() {
        let dir = std::env::temp_dir().join(format!("specular-ai-cap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let t0 = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000);
        for (i, name) in ["1.patch", "2.depth", "3.patch"].iter().enumerate() {
            let f = dir.join(name);
            write_cache_in(&dir, &f, &[0; 100], u64::MAX);
            std::fs::File::options()
                .write(true)
                .open(&f)
                .unwrap()
                .set_modified(t0 + std::time::Duration::from_secs(i as u64))
                .unwrap();
        }
        // Other files are neither counted nor removed.
        std::fs::write(dir.join("other.txt"), [0; 500]).unwrap();
        write_cache_in(&dir, &dir.join("4.depth"), &[0; 100], 250);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        // 400 bytes over 250: the two oldest go; the newest and the one written stay.
        assert_eq!(left, ["3.patch", "4.depth", "other.txt"]);
        const { assert!(CACHE_BYTES >= 1 << 30) };
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn states_read_as_lines() {
        assert_eq!(
            state_line(Some(&NetState::Ready), String::new()),
            t!("develop-ai-ready")
        );
        assert_eq!(state_line(None, "x".into()), "x");
    }
}
