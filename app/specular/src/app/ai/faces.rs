//! People's and Culling's background work (DESIGN.md §10 "Local AI pack"): one low-priority
//! `ai-faces` worker that, for each of the two features that is on and installed, loads its
//! models lazily ([`specular_ai::faces::FaceRuntime`]) and runs a [`Job`] over the
//! catalogue's photos, read from the files at up to [`specular_ai::faces::SIDE`] pixels
//! (a thumbnail is too small for the faces in a group photo).
//!
//! People runs only while "Group faces into people" is on for this library (off by default).
//! After each People pass the faces are grouped again
//! ([`specular_ai::faces::regroup`]). The two jobs never run at once (they share the GPU),
//! and both pause while the user is busy, as Search's does.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};
use specular_ai::faces::{FaceRuntime, face_parts, threshold};
use specular_ai::job::{Decode, Env, Image, Job, LibrarySource};
use specular_ai::runtime::{Dispatch, Runtime};
use specular_ai::{Feature, Pack};
use specular_core::CancelToken;
use specular_core::paths::selector;
use specular_decode::Registry;
use specular_gfx::wgpu;
use specular_thumbs::DiskCache;
use specular_thumbs::generate::{self, GenOptions};

use super::device::Session;

/// How long the worker sleeps when nothing woke it.
const TICK: Duration = Duration::from_secs(2);
/// A finished pass runs again after this (photos added since are looked at).
const AGAIN: Duration = Duration::from_secs(60);
/// A size that failed to load is tried again after this.
const RETRY: Duration = Duration::from_secs(600);

/// What the app tells the worker.
#[derive(Clone, Default)]
pub(crate) struct Config {
    pub(crate) people: bool,
    pub(crate) culling: bool,
    pub(crate) cache: Option<Arc<DiskCache>>,
    pub(crate) flags: u32,
    pub(crate) gpu: Option<(wgpu::Device, wgpu::Queue)>,
    pub(crate) registry: Option<Arc<Registry>>,
}

struct Shared {
    cfg: Mutex<(Config, bool)>,
    cv: Condvar,
}

/// The worker (it lives as long as the app).
pub(crate) struct FaceIndexer {
    shared: Arc<Shared>,
}

impl FaceIndexer {
    pub(crate) fn start(
        pack: Arc<Pack>,
        cfg: Config,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> FaceIndexer {
        let shared = Arc::new(Shared {
            cfg: Mutex::new((cfg, true)),
            cv: Condvar::new(),
        });
        let sh = shared.clone();
        let spawned = specular_thread::builder("ai-faces").spawn(move || {
            let mut pause = Duration::from_secs(60);
            while specular_core::isolate::run("ai faces", || run(&pack, &sh, &wake)).is_err() {
                std::thread::sleep(pause);
                pause = (pause * 2).min(Duration::from_secs(30 * 60));
            }
        });
        if let Err(e) = spawned {
            specular_log::warn!(target: specular_log::JOBS, "ai: no faces worker: {e}");
        }
        FaceIndexer { shared }
    }

    /// The switches as last told.
    pub(crate) fn on(&self) -> (bool, bool) {
        let g = self.shared.cfg.lock();
        (g.0.people, g.0.culling)
    }

    pub(crate) fn set_on(&self, people: bool, culling: bool) {
        let mut g = self.shared.cfg.lock();
        if (g.0.people, g.0.culling) != (people, culling) {
            g.0.people = people;
            g.0.culling = culling;
            g.1 = true;
            self.shared.cv.notify_one();
        }
    }

    /// Look again now (a size installed, grouping turned on or off).
    pub(crate) fn poke(&self) {
        self.shared.cfg.lock().1 = true;
        self.shared.cv.notify_one();
    }
}

/// Reads a photo for faces: the gallery's thumbnail generator at `side` (an embedded
/// preview when large enough, else the decoder's nearest scale, oriented).
fn decoder(registry: Arc<Registry>) -> Decode {
    Arc::new(move |path, side| {
        let src = specular_core::Source::open(path).ok()?;
        let session = registry.open_for_thumbnail(Arc::new(src)).ok()?;
        let opts = GenOptions {
            prefer_embedded: false,
            ..GenOptions::default()
        };
        let px =
            generate::generate(&*session, side, opts, &CancelToken::new(), &mut |_| {}).ok()?;
        Some(Image {
            width: px.width,
            height: px.height,
            rgba: px.rgba,
        })
    })
}

struct Loaded {
    key: String,
    runtime: Arc<FaceRuntime>,
    job: Option<Job>,
    ended: Option<Instant>,
}

#[derive(Default)]
struct Lane {
    loaded: Option<Loaded>,
    failed: Option<(String, Instant)>,
}

fn run(pack: &Pack, sh: &Shared, wake: &Arc<dyn Fn() + Send + Sync>) {
    specular_platform::lower_thread_priority();
    let mut lanes: [Lane; 2] = Default::default();
    let mut session: Option<Session> = None;
    loop {
        let cfg = {
            let mut g = sh.cfg.lock();
            if !g.1 {
                sh.cv.wait_for(&mut g, TICK);
            }
            g.1 = false;
            g.0.clone()
        };
        // After a GPU reset the models are loaded again on the new device.
        let s = session.get_or_insert_with(|| Session::new(cfg.gpu.clone()));
        if s.stale() {
            for lane in &mut lanes {
                if let Some(l) = lane.loaded.take()
                    && let Some(j) = &l.job
                {
                    j.cancel();
                }
            }
        }
        let ctx = s.context().cloned();
        // Grouping is per library and off until the user turns it on.
        let grouping = cfg.people && pack.face_grouping() == Some(true);
        let busy_other = |lanes: &[Lane; 2], i: usize| {
            lanes[1 - i]
                .loaded
                .as_ref()
                .is_some_and(|l| l.job.as_ref().is_some_and(|j| !j.ended()))
        };
        for (i, (feature, on)) in [(Feature::Culling, cfg.culling), (Feature::People, grouping)]
            .into_iter()
            .enumerate()
        {
            let cur = if on {
                pack.index_models(feature).0
            } else {
                None
            };
            let Some(cur) = cur else {
                if let Some(l) = lanes[i].loaded.take()
                    && let Some(j) = &l.job
                {
                    j.cancel();
                }
                continue;
            };
            let lane = &mut lanes[i];
            if lane.loaded.as_ref().is_none_or(|l| l.key != cur.key) {
                if lane
                    .failed
                    .as_ref()
                    .is_some_and(|(k, at)| *k == cur.key && at.elapsed() < RETRY)
                {
                    continue;
                }
                lane.loaded = None;
                let Some(parts) = face_parts(pack, &cur) else {
                    continue;
                };
                let t = Instant::now();
                match FaceRuntime::load(feature, &cur.key, &parts, ctx.as_ref(), selector(), 4) {
                    Ok(rt) => {
                        specular_log::info!(target: specular_log::JOBS, "ai: {} model {} loaded in {:.0?}", feature.id(), cur.key, t.elapsed());
                        lane.loaded = Some(Loaded {
                            key: cur.key.clone(),
                            runtime: Arc::new(rt),
                            job: None,
                            ended: None,
                        });
                    }
                    Err(e) => {
                        specular_log::warn!(target: specular_log::JOBS, "ai: {} model {}: {e}", feature.id(), cur.key);
                        lane.failed = Some((cur.key.clone(), Instant::now()));
                        continue;
                    }
                }
            }
            let other_busy = busy_other(&lanes, i);
            let l = lanes[i].loaded.as_mut().expect("loaded above");
            if l.job.as_ref().is_some_and(Job::ended) {
                let p = l.job.take().expect("checked").progress();
                specular_log::info!(target: specular_log::JOBS, "ai: {} pass {:?}: {} of {} done, {} failed", feature.id(), p.state, p.done, p.total, p.failed);
                l.ended = Some(Instant::now());
                if feature == Feature::People {
                    regroup(pack, &l.key, &l.runtime);
                }
                wake();
            }
            let due = l.ended.is_none_or(|t| t.elapsed() >= AGAIN);
            if l.job.is_none()
                && due
                && !other_busy
                && let (Some(cache), Some(db), Some(reg)) =
                    (cfg.cache.clone(), pack.catalogue(), cfg.registry.clone())
            {
                match specular_library::ai::open(db) {
                    Ok(conn) => {
                        let source = LibrarySource::new(conn, cache, cfg.flags, feature, &l.key)
                            .with_decoder(decoder(reg));
                        let rt: Arc<dyn Runtime> = l.runtime.clone();
                        let w = wake.clone();
                        let env = Env {
                            browsing: Arc::new(super::index::busy),
                            wake: Arc::new(move || w()),
                            on_thread_start: Some(specular_platform::lower_thread_priority),
                            ..Env::platform()
                        };
                        l.job = Some(Job::start(
                            Box::new(source),
                            Dispatch::new(rt, selector()),
                            env,
                        ));
                    }
                    Err(e) => {
                        specular_log::warn!(target: specular_log::JOBS, "ai: catalogue: {e}");
                        l.ended = Some(Instant::now());
                    }
                }
            }
        }
    }
}

/// Groups the faces found into people (on this worker; the catalogue may be busy).
fn regroup(pack: &Pack, key: &str, rt: &FaceRuntime) {
    let (Some(db), Some(kind)) = (pack.catalogue(), rt.kind()) else {
        return;
    };
    let t = Instant::now();
    match specular_library::ai::open(db)
        .map_err(|e| e.to_string())
        .and_then(|mut c| {
            specular_ai::faces::regroup(&mut c, key, threshold(kind)).map_err(|e| e.to_string())
        }) {
        Ok(n) => {
            specular_log::info!(target: specular_log::JOBS, "ai: grouped {n} faces in {:.0?}", t.elapsed())
        }
        Err(e) => specular_log::warn!(target: specular_log::JOBS, "ai: grouping faces: {e}"),
    }
}
