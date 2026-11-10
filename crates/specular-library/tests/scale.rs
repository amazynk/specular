//! The Library's work per file stays flat as the catalogue grows (numbers at
//! 10k / 100k / 1M come from `examples/library_bench.rs`).
//!
//! 50 000 files, each a hard link to one of 50 small files (so every file has a thousand
//! byte-identical copies: the worst case for duplicate grouping, and what a tree of hard
//! links or a folder copied many times looks like). Counted, not timed, so the bounds hold
//! on a loaded machine:
//!
//! * duplicate checks per file (byte and pixel comparisons): every copy against every other
//!   was 500 million here;
//! * full-text rows rewritten: a new file's row is written once, with its facts;
//! * SQLite steps finding the next files for the facts and signature passes: each batch
//!   scanned the catalogue from the start;
//! * duplicate and similar-shot matching after a restart with nothing changed: every
//!   signature was taken in and every group formed again at each start.
//!
//! In its own test binary: the counters are process-wide.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use specular_library::work_counts::{FTS_REWRITES, MATCH_OPS, TODO_STEPS};
use specular_library::{Config, Library, Probed, Prober, Search, Signer};
use specular_similar::{Luma, Signature};
use specular_thumbs::{DiskCache, ThumbKey};

const FILES: usize = 50_000;
const LOOKS: u64 = 50;

/// The look of a file: its text is `look=N`.
fn look(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path)
        .ok()?
        .trim()
        .strip_prefix("look=")?
        .parse()
        .ok()
}

fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

struct Facts;

impl Prober for Facts {
    fn probe(&self, path: &Path) -> Option<Probed> {
        let n = look(path)?;
        let mut p = Probed::default();
        p.meta.format = "JPEG".into();
        p.meta.width = 40;
        p.meta.height = 30;
        p.meta.gps = Some(false);
        p.meta.camera = Some(format!("Camera {}", n % 5));
        p.meta.date_taken = Some(1_600_000_000 + n as i64 * 3600);
        Some(p)
    }
}

/// Signatures from the look; counts the comparisons the duplicate checks ask for.
#[derive(Default)]
struct Looks {
    byte_checks: AtomicU64,
    pixel_checks: AtomicU64,
}

impl Signer for Looks {
    fn signature(
        &self,
        path: &Path,
        _: &ThumbKey,
        _: Option<&DiskCache>,
        _: u32,
    ) -> Option<Signature> {
        let n = look(path)?;
        let mut s = Signature {
            tier1: mix(n),
            phash: mix(n + 1),
            dhash: [mix(n + 2), mix(n + 3), mix(n + 4), mix(n + 5)],
            moments: [120, 128, 128, 40, 10, 10],
            aspect: 40.0 / 30.0,
            ..Default::default()
        };
        s.quality.detail = 10.0;
        s.quality.grid = [10.0; 16];
        Some(s)
    }

    fn luma(&self, path: &Path, _: &ThumbKey, _: Option<&DiskCache>, _: u32) -> Option<Luma> {
        self.pixel_checks.fetch_add(1, Ordering::Relaxed);
        let n = look(path)?;
        let px = (0..32 * 32)
            .map(|i| (mix(n.wrapping_add(i)) % 256) as f32 / 255.0)
            .collect();
        Some(Luma { w: 32, h: 32, px })
    }

    fn file_hash(&self, path: &Path) -> Option<[u8; 32]> {
        self.byte_checks.fetch_add(1, Ordering::Relaxed);
        let mut h = [0; 32];
        h[..8].copy_from_slice(&mix(look(path)?).to_le_bytes());
        Some(h)
    }
}

struct Tree(PathBuf);

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wait(lib: &Library, what: &str) {
    assert!(
        lib.wait_idle(Duration::from_secs(900)),
        "{what}: not settled: {:?}",
        lib.progress()
    );
}

#[test]
fn work_per_file_stays_flat_at_50k_files() {
    let t = Tree(std::env::temp_dir().join(format!("specular-scale-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&t.0);
    let seed = t.0.join("seed");
    let root = t.0.join("photos");
    std::fs::create_dir_all(&seed).unwrap();
    for k in 0..LOOKS {
        std::fs::write(seed.join(format!("{k}")), format!("look={k}\n")).unwrap();
    }
    let made = Instant::now();
    for i in 0..FILES {
        let dir = root.join(format!("{}/{}", i / 10_000, (i / 250) % 40));
        if i % 250 == 0 {
            std::fs::create_dir_all(&dir).unwrap();
        }
        std::fs::hard_link(
            seed.join(format!("{}", i as u64 % LOOKS)),
            dir.join(format!("IMG_{i:05}.jpg")),
        )
        .unwrap();
    }
    eprintln!("made {FILES} links in {:.1?}", made.elapsed());

    let signer = Arc::new(Looks::default());
    let mut cfg = Config::new(t.0.join("library.sqlite3"));
    cfg.prober = Arc::new(Facts);
    cfg.signer = Some(signer.clone());
    let lib = Library::open(cfg).unwrap();
    let t0 = Instant::now();
    lib.add_location(&root);
    wait(&lib, "first index");
    let first = t0.elapsed();
    let (rewrites, steps) = (
        FTS_REWRITES.load(Ordering::Relaxed),
        TODO_STEPS.load(Ordering::Relaxed),
    );
    let (bytes, pixels) = (
        signer.byte_checks.load(Ordering::Relaxed),
        signer.pixel_checks.load(Ordering::Relaxed),
    );
    let n = FILES as u64;
    eprintln!(
        "first index {first:.1?}: {bytes} byte and {pixels} pixel checks, {rewrites} full-text \
         rewrites, {steps} steps finding work ({:.1} per file)",
        steps as f64 / n as f64
    );
    assert_eq!(lib.counts().unwrap(), (n, n));
    assert!(
        bytes + pixels <= 4 * n,
        "{bytes} + {pixels} duplicate checks"
    );
    assert_eq!(rewrites, 0, "full-text rows written twice");
    assert!(steps <= 120 * n, "{steps} steps finding work");

    // Every copy is in its look's group, one kept per group.
    let f = |q: &str| {
        lib.search(&Search {
            query: q.into(),
            ..Search::default()
        })
        .unwrap()
        .total
    };
    assert_eq!(f("dupes:any"), FILES);
    assert_eq!(f("dupes:keep"), LOOKS as usize);

    // A rescan with nothing changed reads nothing and finds no work.
    let t0 = Instant::now();
    lib.rescan(None);
    wait(&lib, "rescan");
    let steps2 = TODO_STEPS.load(Ordering::Relaxed) - steps;
    eprintln!("rescan {:.1?}: {steps2} steps finding work", t0.elapsed());
    assert!(steps2 <= n, "{steps2} steps finding work after a rescan");
    assert_eq!(FTS_REWRITES.load(Ordering::Relaxed), rewrites);
    drop(lib);

    // Restarted with nothing changed: no matching at all, the same groups.
    let ops = MATCH_OPS.load(Ordering::Relaxed);
    assert!(ops > n, "{ops} matching operations at first");
    let mut cfg = Config::new(t.0.join("library.sqlite3"));
    cfg.prober = Arc::new(Facts);
    cfg.signer = Some(signer.clone());
    let lib = Library::open(cfg).unwrap();
    let t0 = Instant::now();
    wait(&lib, "restart");
    let again = MATCH_OPS.load(Ordering::Relaxed) - ops;
    eprintln!("restart {:.1?}: {again} matching operations", t0.elapsed());
    assert_eq!(again, 0, "matching after a restart with nothing changed");
    assert_eq!(found(&lib, "dupes:any"), FILES);
    assert_eq!(found(&lib, "dupes:keep"), LOOKS as usize);
    drop(lib);
}

fn found(lib: &Library, q: &str) -> usize {
    lib.search(&Search {
        query: q.into(),
        ..Search::default()
    })
    .unwrap()
    .total
}
