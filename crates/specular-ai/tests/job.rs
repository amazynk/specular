//! Background indexing over a catalogue and a thumbnail cache with the stub model.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use common::{Dir, catalogue};
use specular_ai::Feature;
use specular_ai::job::{Env, Job, LibrarySource, Pause, State};
use specular_ai::runtime::{Dispatch, Outputs, RunError, Runtime, StubRuntime, Thumb};
use specular_core::paths::Engine;
use specular_core::paths::PathSelector;
use specular_platform::{MemoryPressure, PowerState, Thermal};
use specular_thumbs::pixels::Pixels;
use specular_thumbs::{DiskCache, Meta, Stamp, ThumbKey};

static SEL: PathSelector = PathSelector::new();

fn key(i: i64) -> ThumbKey {
    ThumbKey {
        stamp: Stamp {
            len: 1000 + i as u64,
            mtime_ns: 5000 + i as i128,
        },
        content: i as u64,
    }
}

struct Setup {
    _dir: Dir,
    db: std::path::PathBuf,
    cache: Arc<DiskCache>,
}

/// `n` files; thumbnails cached for those `cached` says (and referenced for all).
fn setup(name: &str, n: i64, cached: impl Fn(i64) -> bool) -> Setup {
    let dir = Dir::new(name);
    let db = dir.path().join("library.sqlite3");
    catalogue(&db, n, |_| true);
    let mut conn = specular_library::ai::open(&db).unwrap();
    specular_library::ai::install(&mut conn, "search").unwrap();
    let cache = Arc::new(DiskCache::new(dir.path().join("thumbs"), 1 << 30));
    for i in 1..=n {
        if cached(i) {
            let c = (i * 37 % 256) as u8;
            let px = Pixels::new(8, 8, [c, 255 - c, c / 2, 255].repeat(64));
            cache.put(&key(i), 128, 0, &px, &Meta::default()).unwrap();
        }
    }
    Setup {
        _dir: dir,
        db,
        cache,
    }
}

fn env(browsing: Arc<AtomicBool>, power: Arc<parking_lot::Mutex<PowerState>>) -> Env {
    Env {
        browsing: Arc::new(move || browsing.load(Ordering::Relaxed)),
        machine: Arc::new(move || (*power.lock(), MemoryPressure::Normal)),
        wake: Arc::new(|| {}),
        pause_poll: Duration::from_millis(5),
        throttle_ratio: 1,
        on_thread_start: None,
        switching: false,
    }
}

fn start(s: &Setup, env: Env) -> Job {
    let conn = specular_library::ai::open(&s.db).unwrap();
    let source = LibrarySource::new(conn, s.cache.clone(), 0, Feature::Search, "stub-1");
    let dispatch = Dispatch::new(Arc::new(StubRuntime::new(Feature::Search, "stub-1")), &SEL);
    Job::start(Box::new(source), dispatch, env)
}

#[test]
fn indexes_cached_thumbnails_and_counts_the_rest_as_waiting() {
    let s = setup("index", 40, |i| i % 4 != 0);
    let job = start(&s, env(Arc::default(), Arc::default()));
    assert!(job.wait(Duration::from_secs(30)));
    let p = job.progress();
    assert_eq!(p.state, State::Finished);
    assert_eq!((p.total, p.done, p.waiting_thumbs), (40, 30, 10));
    let conn = specular_library::ai::open(&s.db).unwrap();
    let e = specular_library::ai::embeddings(&conn, "stub-1", 512).unwrap();
    assert_eq!(e.len(), 30);
    assert!(e.iter().all(|(id, _)| id % 4 != 0));

    // The missing thumbnails arrive: a new run picks up only those.
    for i in (4..=40).step_by(4) {
        let px = Pixels::new(4, 4, [1u8, 2, 3, 255].repeat(16));
        s.cache.put(&key(i), 256, 0, &px, &Meta::default()).unwrap();
    }
    let job = start(&s, env(Arc::default(), Arc::default()));
    assert!(job.wait(Duration::from_secs(30)));
    let p = job.progress();
    assert_eq!((p.total, p.done, p.waiting_thumbs), (40, 40, 0));
}

/// The stub model, panicking on thumbnails whose first byte is `poison`.
struct Panicky {
    stub: StubRuntime,
    poison: u8,
}

impl Runtime for Panicky {
    fn feature(&self) -> Feature {
        self.stub.feature()
    }
    fn model(&self) -> &str {
        self.stub.model()
    }
    fn input_side(&self) -> u32 {
        self.stub.input_side()
    }
    fn engines(&self) -> &[Engine] {
        self.stub.engines()
    }
    fn run(&self, engine: Engine, thumb: &Thumb) -> Result<Outputs, RunError> {
        assert_ne!(thumb.rgba[0], self.poison, "model bug");
        self.stub.run(engine, thumb)
    }
}

/// A model that panics on one file fails that file only: the job goes on with the rest and
/// finishes.
#[test]
fn a_model_panic_costs_one_file() {
    let s = setup("panic", 20, |_| true);
    let conn = specular_library::ai::open(&s.db).unwrap();
    let source = LibrarySource::new(conn, s.cache.clone(), 0, Feature::Search, "stub-1");
    // File 7's thumbnail starts with 7 * 37 % 256 = 3, and no other file's does.
    let rt = Panicky {
        stub: StubRuntime::new(Feature::Search, "stub-1"),
        poison: 3,
    };
    let dispatch = Dispatch::new(Arc::new(rt), &SEL);
    let job = Job::start(
        Box::new(source),
        dispatch,
        env(Arc::default(), Arc::default()),
    );
    assert!(job.wait(Duration::from_secs(30)));
    let p = job.progress();
    assert_eq!(p.state, State::Finished);
    assert_eq!((p.done, p.failed), (19, 1));
    let conn = specular_library::ai::open(&s.db).unwrap();
    let e = specular_library::ai::embeddings(&conn, "stub-1", 512).unwrap();
    assert_eq!(e.len(), 19);
    assert!(e.iter().all(|(id, _)| *id != 7));
}

#[test]
fn pauses_while_browsing_and_when_hot_then_resumes() {
    let s = setup("pause", 20, |_| true);
    let browsing = Arc::new(AtomicBool::new(true));
    let power = Arc::new(parking_lot::Mutex::new(PowerState::default()));
    let job = start(&s, env(browsing.clone(), power.clone()));
    std::thread::sleep(Duration::from_millis(100));
    let p = job.progress();
    assert_eq!(p.state, State::Paused(Pause::Browsing));
    assert_eq!(p.done, 0);
    power.lock().thermal = Thermal::Critical;
    browsing.store(false, Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(job.progress().state, State::Paused(Pause::Heat));
    assert_eq!(job.progress().done, 0);
    power.lock().thermal = Thermal::Nominal;
    assert!(job.wait(Duration::from_secs(30)));
    assert_eq!(job.progress().state, State::Finished);
    assert_eq!(job.progress().done, 20);
}

#[test]
fn cancel_then_resume_continues_where_it_stopped() {
    let s = setup("resume", 300, |_| true);
    // Throttled to slow it down, so the cancel lands mid-way.
    let power = Arc::new(parking_lot::Mutex::new(PowerState {
        on_battery: true,
        ..Default::default()
    }));
    let job = start(&s, env(Arc::default(), power));
    let seen = AtomicUsize::new(0);
    while job.progress().done < 5 {
        seen.fetch_add(1, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(1));
        assert!(seen.load(Ordering::Relaxed) < 30_000, "no progress");
    }
    assert_eq!(job.progress().state, State::Throttled);
    job.cancel();
    assert!(job.wait(Duration::from_secs(30)));
    let p = job.progress();
    assert_eq!(p.state, State::Cancelled);
    assert!(p.done < 300, "{}", p.done);
    let done_before = p.done;
    drop(job);

    let job = start(&s, env(Arc::default(), Arc::default()));
    assert!(job.wait(Duration::from_secs(60)));
    let p = job.progress();
    assert_eq!(p.state, State::Finished);
    assert_eq!(p.done, 300);
    assert!(done_before > 0);
}
