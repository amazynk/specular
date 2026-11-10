//! Search's background work (DESIGN.md §10 "Local AI pack"): one low-priority `ai-index`
//! worker loads the installed size's CLIP lazily, embeds the catalogue's photos from their
//! cached thumbnails through a [`Job`] (paused while the user is busy, on battery or under
//! memory pressure), and hands the text half to [`super::semantic`] for the Library's search
//! box.
//!
//! During a size switch the size installed before stays in use: searches read its
//! embeddings with its text tower while the new size embeds the library under its own key;
//! once the new key covers every readable photo, [`Pack::finish_index`] deletes the old
//! embeddings and files, and searches move to the new size. Nothing here runs on the UI
//! thread: the app only updates [`Config`] and stamps frames ([`frame_drawn`]).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};
use specular_ai::infer::{ClipTextEncoder, GpuCheck, InferRuntime};
use specular_ai::job::{Env, Job, LibrarySource};
use specular_ai::runtime::{Dispatch, RunsOn, Runtime};
use specular_ai::{Feature, IndexModel, Pack};
use specular_core::paths::{Engine, selector};
use specular_gfx::wgpu;
use specular_thumbs::DiskCache;

use super::device::Session;
use super::semantic::{self, Model};

/// How long the worker sleeps when nothing woke it.
const TICK: Duration = Duration::from_secs(2);
/// A finished pass runs again after this (photos added since are embedded).
const AGAIN: Duration = Duration::from_secs(60);
/// A size that failed to load is tried again after this.
const RETRY: Duration = Duration::from_secs(600);
/// A frame drawn this recently means the user is busy: the job pauses.
const BUSY: Duration = Duration::from_millis(1500);

/// What the app tells the worker.
#[derive(Clone, Default)]
pub(crate) struct Config {
    /// Search's switch.
    pub(crate) on: bool,
    /// The thumbnail cache the job reads photos from, and its key flags.
    pub(crate) cache: Option<Arc<DiskCache>>,
    pub(crate) flags: u32,
    /// The app's device, for the GPU path (None: CPU only).
    pub(crate) gpu: Option<(wgpu::Device, wgpu::Queue)>,
}

struct Shared {
    cfg: Mutex<(Config, bool)>,
    cv: Condvar,
}

/// The worker (it lives as long as the app).
pub(crate) struct Indexer {
    shared: Arc<Shared>,
}

impl Indexer {
    /// Starts the `ai-index` worker on `pack`; `wake` is called when indexing progressed.
    pub(crate) fn start(
        pack: Arc<Pack>,
        cfg: Config,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Indexer {
        let shared = Arc::new(Shared {
            cfg: Mutex::new((cfg, true)),
            cv: Condvar::new(),
        });
        let sh = shared.clone();
        let spawned = specular_thread::builder("ai-index").spawn(move || {
            // A panic stops the indexing, not Search: the worker starts again after a pause
            // (longer each time, so a panic on every start does not fill the log).
            let mut pause = Duration::from_secs(60);
            while specular_core::isolate::run("ai search index", || run(&pack, &sh, &wake)).is_err()
            {
                std::thread::sleep(pause);
                pause = (pause * 2).min(Duration::from_secs(30 * 60));
            }
        });
        if let Err(e) = spawned {
            specular_log::warn!(target: specular_log::JOBS, "ai: no index worker: {e}");
        }
        Indexer { shared }
    }

    /// Whether Search is on, as last told.
    pub(crate) fn on(&self) -> bool {
        self.shared.cfg.lock().0.on
    }

    /// Search switched on or off.
    pub(crate) fn set_on(&self, on: bool) {
        let mut g = self.shared.cfg.lock();
        if g.0.on != on {
            g.0.on = on;
            g.1 = true;
            self.shared.cv.notify_one();
        }
    }

    /// Look again now (a size installed or switched).
    pub(crate) fn poke(&self) {
        self.shared.cfg.lock().1 = true;
        self.shared.cv.notify_one();
    }
}

fn epoch() -> Instant {
    static T: OnceLock<Instant> = OnceLock::new();
    *T.get_or_init(Instant::now)
}

static LAST_FRAME: AtomicU64 = AtomicU64::new(0);

/// The app drew a frame (the user may be busy).
pub(crate) fn frame_drawn(at: Instant) {
    let ms = at.saturating_duration_since(epoch()).as_millis() as u64;
    LAST_FRAME.fetch_max(ms, Ordering::Relaxed);
}

pub(crate) fn busy() -> bool {
    let now = epoch().elapsed().as_millis() as u64;
    now.saturating_sub(LAST_FRAME.load(Ordering::Relaxed)) < BUSY.as_millis() as u64
}

/// The loaded search model's engines, GPU check at load and input side, for
/// [`search_runs_on`] (None: none loaded).
static LOADED: Mutex<Option<(Vec<Engine>, GpuCheck, u32)>> = Mutex::new(None);

/// What the loaded search model's indexing runs on now (None: none loaded). Reads the path
/// selector: call it on a worker (the Local AI page's), not the UI thread.
pub(crate) fn search_runs_on() -> Option<RunsOn> {
    let g = LOADED.lock();
    let (engines, check, side) = g.as_ref()?;
    Some(RunsOn::of(engines, check, selector(), *side))
}

fn set_loaded(rt: Option<&InferRuntime>) {
    *LOADED.lock() = rt.map(|r| (r.engines().to_vec(), r.gpu_check().clone(), r.input_side()));
    // The Local AI page says what the model runs on: build it again.
    super::STALE.store(true, Ordering::Relaxed);
}

/// The size loaded: its runtime (the image half, for the job) and the text half searched
/// with.
struct Loaded {
    key: String,
    prev: Option<String>,
    runtime: Arc<InferRuntime>,
    /// The text half searches use: the previous size's during a switch.
    searched: Arc<Model>,
    job: Option<Job>,
    ended: Option<Instant>,
    finished: bool,
}

/// The key searches read while `cur` indexes (and `prev`, the size installed before a
/// switch, is still in use).
pub(crate) fn searched_key<'a>(cur: &'a str, prev: Option<&'a str>) -> &'a str {
    prev.unwrap_or(cur)
}

/// Whether a switch's new size covers the library (`counts`: readable files and those
/// embedded under the new key), so the old size can go.
pub(crate) fn switch_done(counts: Option<(u64, u64)>) -> bool {
    counts.is_some_and(|(total, done)| total > 0 && done >= total)
}

fn load(
    pack: &Pack,
    im: &IndexModel,
    gpu: Option<&specular_infer::gpu::Context>,
) -> Result<(InferRuntime, ClipTextEncoder), String> {
    let (w, m, norm) = pack
        .clip_parts(im)
        .ok_or_else(|| format!("{}: not a CLIP size", im.key))?;
    InferRuntime::load_installed_clip(pack.store(), &w, &m, norm, gpu)
        .map_err(|e| crate::error_text::reason(&e))
}

fn run(pack: &Pack, sh: &Shared, wake: &Arc<dyn Fn() + Send + Sync>) {
    specular_platform::lower_thread_priority();
    let mut loaded: Option<Loaded> = None;
    let mut failed: Option<(String, Instant)> = None;
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
        // After a GPU reset the model is loaded again on the new device.
        let s = session.get_or_insert_with(|| Session::new(cfg.gpu.clone()));
        if s.stale()
            && let Some(l) = loaded.take()
        {
            if let Some(j) = &l.job {
                j.cancel();
            }
            semantic::set(None);
            set_loaded(None);
        }
        let ctx = s.context().cloned();
        let gpu = Session::device_of(ctx.as_ref());
        let (cur, prev) = if cfg.on {
            pack.index_models(Feature::Search)
        } else {
            (None, None)
        };
        let Some(cur) = cur else {
            if loaded.take().is_some() {
                semantic::set(None);
                set_loaded(None);
            }
            continue;
        };
        let prev_key = prev.as_ref().map(|p| p.key.clone());
        if loaded.as_ref().is_none_or(|l| l.key != cur.key) {
            if failed
                .as_ref()
                .is_some_and(|(k, at)| *k == cur.key && at.elapsed() < RETRY)
            {
                continue;
            }
            loaded = None;
            semantic::set(None);
            set_loaded(None);
            let t = Instant::now();
            match load(pack, &cur, ctx.as_ref()) {
                Ok((rt, text)) => {
                    specular_log::info!(target: specular_log::JOBS, "ai: search model {} loaded in {:.0?}", cur.key, t.elapsed());
                    let searched =
                        Arc::new(Model::new(cur.key.clone(), Arc::new(text), gpu.clone()));
                    set_loaded(Some(&rt));
                    wake();
                    loaded = Some(Loaded {
                        key: cur.key.clone(),
                        // Set below, with the previous size's text half.
                        prev: Some(String::new()),
                        runtime: Arc::new(rt),
                        searched,
                        job: None,
                        ended: None,
                        finished: false,
                    });
                }
                Err(e) => {
                    specular_log::warn!(target: specular_log::JOBS, "ai: search model {}: {e}", cur.key);
                    failed = Some((cur.key.clone(), Instant::now()));
                    continue;
                }
            }
        }
        let l = loaded.as_mut().expect("loaded above");
        if l.prev != prev_key {
            // Searches read the size in use: the previous one until the new one has indexed.
            let key = searched_key(&cur.key, prev_key.as_deref());
            if key != l.searched.key {
                let text = match prev.as_ref().map(|p| load(pack, p, ctx.as_ref())) {
                    Some(Ok((_, text))) => Some(Arc::new(text)),
                    Some(Err(e)) => {
                        specular_log::warn!(target: specular_log::JOBS, "ai: previous search model: {e}");
                        None
                    }
                    None => Some(l.searched.text.clone()),
                };
                if let Some(text) = text {
                    l.searched = Arc::new(Model::new(key.to_owned(), text, gpu.clone()));
                }
            }
            l.prev = prev_key.clone();
            semantic::set(Some(l.searched.clone()));
        }
        if l.job.as_ref().is_some_and(Job::ended) {
            let p = l.job.take().expect("checked").progress();
            specular_log::info!(target: specular_log::JOBS, "ai: search pass {:?}: {} of {} embedded, {} failed", p.state, p.done, p.total, p.failed);
            l.ended = Some(Instant::now());
            wake();
        }
        if l.job.is_none()
            && prev.is_some()
            && !l.finished
            && switch_done(pack.counts(Feature::Search))
        {
            pack.finish_index(Feature::Search, cur.key.clone());
            l.finished = true;
            continue;
        }
        let due = l.ended.is_none_or(|t| t.elapsed() >= AGAIN);
        if l.job.is_none()
            && due
            && let (Some(cache), Some(db)) = (cfg.cache.clone(), pack.catalogue())
        {
            match specular_library::ai::open(db) {
                Ok(conn) => {
                    let source =
                        LibrarySource::new(conn, cache, cfg.flags, Feature::Search, &cur.key);
                    let rt: Arc<dyn Runtime> = l.runtime.clone();
                    let w = wake.clone();
                    let env = Env {
                        browsing: Arc::new(busy),
                        wake: Arc::new(move || w()),
                        on_thread_start: Some(specular_platform::lower_thread_priority),
                        switching: prev.is_some(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switch_searches_the_old_size_until_the_new_one_is_complete() {
        assert_eq!(searched_key("standard/b", Some("small/a")), "small/a");
        assert_eq!(searched_key("standard/b", None), "standard/b");
        assert!(!switch_done(None));
        assert!(
            !switch_done(Some((0, 0))),
            "an empty catalogue proves nothing"
        );
        assert!(!switch_done(Some((10, 9))));
        assert!(switch_done(Some((10, 10))));
    }
}
