//! Indexes a generated tree of tiny JPEGs (with EXIF, some with XMP sidecars) and times the
//! walk, the facts pass, a rescan with nothing changed, an incremental rescan, and typical
//! searches; prints memory and the machine's load.
//!
//! `cargo run -p specular-library --example library_bench [files] [options]`
//! The tree and the catalogue are written under `$CARGO_TARGET_DIR/library-bench/` (else the
//! workspace `target/library-bench/`) and deleted at the end unless `--keep`. Options:
//!
//! * `--links`: every image and sidecar is a hard link to one of a few files (a million
//!   files in directory entries and inodes, not bytes; each look has thousands of copies);
//! * `--no-signer`: no signatures, duplicates or similar shots;
//! * `--quick`: the indexing only (no searches);
//! * `--reuse`: search the tree and catalogue a `--keep` run left (and keep them);
//! * `--profile`: the slowest search 30 times more, for a profiler.
//!
//! Each indexing step prints when each phase ended with the CPU time of the indexer thread
//! and of the process (steadier than wall time on a loaded machine).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_library::{Config, Library, Search, Sort};

const CAMERAS: [(&str, &str); 5] = [
    ("Canon", "Canon EOS R5"),
    ("NIKON CORPORATION", "NIKON Z 8"),
    ("SONY", "ILCE-7M4"),
    ("FUJIFILM", "X-T5"),
    ("Google", "Pixel 8"),
];

fn ascii(v: &mut Vec<u8>, s: &str) {
    v.extend_from_slice(s.as_bytes());
    v.push(0);
}

/// A little-endian EXIF block: Make, Model, ISO and DateTimeOriginal.
fn exif(make: &str, model: &str, iso: u16, date: &str) -> Vec<u8> {
    let entry = |v: &mut Vec<u8>, tag: u16, ty: u16, count: u32, value: u32| {
        v.extend_from_slice(&tag.to_le_bytes());
        v.extend_from_slice(&ty.to_le_bytes());
        v.extend_from_slice(&count.to_le_bytes());
        v.extend_from_slice(&value.to_le_bytes());
    };
    let ifd0_len = 2 + 3 * 12 + 4;
    let make_at = 8 + ifd0_len;
    let model_at = make_at + make.len() + 1;
    let exif_at = (model_at + model.len() + 1 + 1) & !1;
    let exif_len = 2 + 2 * 12 + 4;
    let date_at = exif_at + exif_len;
    let mut v = b"II*\0".to_vec();
    v.extend_from_slice(&8u32.to_le_bytes());
    v.extend_from_slice(&3u16.to_le_bytes());
    entry(&mut v, 0x010F, 2, make.len() as u32 + 1, make_at as u32);
    entry(&mut v, 0x0110, 2, model.len() as u32 + 1, model_at as u32);
    entry(&mut v, 0x8769, 4, 1, exif_at as u32);
    v.extend_from_slice(&0u32.to_le_bytes());
    ascii(&mut v, make);
    ascii(&mut v, model);
    v.resize(exif_at, 0);
    v.extend_from_slice(&2u16.to_le_bytes());
    entry(&mut v, 0x8827, 3, 1, u32::from(iso));
    entry(&mut v, 0x9003, 2, 20, date_at as u32);
    v.extend_from_slice(&0u32.to_le_bytes());
    ascii(&mut v, date);
    v
}

fn jpeg(base: &[u8], exif: &[u8]) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    let len = (exif.len() + 8) as u16;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(exif);
    out.extend_from_slice(&base[2..]);
    out
}

const SIDECAR: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmp:Rating="RATING"><dc:subject><rdf:Bag><rdf:li>Holiday</rdf:li><rdf:li>KEYWORD</rdf:li></rdf:Bag></dc:subject></rdf:Description></rdf:RDF></x:xmpmeta>"#;

/// Writes `files` images in folders of 100 (`yyyy/dNN/` like a camera import). With `seed`,
/// each image and sidecar is a hard link to one of a few files written there: a million
/// files cost directory entries and inodes, not their bytes (cameras, dates and keywords
/// then repeat every 240 images).
fn generate(root: &Path, files: usize, seed: Option<&Path>) -> u64 {
    let bases: Vec<Vec<u8>> = (0..4u8)
        .map(|k| {
            let px: Vec<u8> = (0..8 * 8)
                .flat_map(|i| [i as u8 * 4, k * 60, 255 - i as u8, 255])
                .collect();
            specular_thumbs::codec::encode_jpeg(&px, 8, 8, 80)
        })
        .collect();
    let mut bytes = 0u64;
    for i in 0..files {
        let dir = root.join(format!("{}/d{:03}", 2016 + i / 12_500, (i / 100) % 125));
        if i % 100 == 0 {
            std::fs::create_dir_all(&dir).unwrap();
        }
        let (make, model) = CAMERAS[i % CAMERAS.len()];
        let date = format!(
            "{}:{:02}:{:02} {:02}:{:02}:{:02}",
            2016 + i / 12_500,
            1 + (i / 1000) % 12,
            1 + (i / 40) % 28,
            (i / 7) % 24,
            i % 60,
            (i * 7) % 60
        );
        let dest = dir.join(format!("IMG_{i:06}.jpg"));
        if let Some(seed) = seed {
            let v = seed.join(format!("v{:03}.jpg", i % 240));
            if !v.exists() {
                let data = jpeg(&bases[i % 4], &exif(make, model, 100 << (i % 6), &date));
                bytes += data.len() as u64;
                std::fs::write(&v, &data).unwrap();
            }
            std::fs::hard_link(&v, &dest).unwrap();
            if i % 50 == 0 {
                let k = (i / 50) % 24;
                let x = seed.join(format!("x{k:02}.xmp"));
                if !x.exists() {
                    let body = SIDECAR
                        .replace("RATING", &(k % 6).to_string())
                        .replace("KEYWORD", ["beach", "mountain", "family", "city"][k % 4]);
                    std::fs::write(&x, body).unwrap();
                }
                std::fs::hard_link(&x, dir.join(format!("IMG_{i:06}.xmp"))).unwrap();
            }
            continue;
        }
        let data = jpeg(&bases[i % 4], &exif(make, model, 100 << (i % 6), &date));
        bytes += data.len() as u64;
        std::fs::write(&dest, &data).unwrap();
        if i % 50 == 0 {
            let x = SIDECAR
                .replace("RATING", &((i / 50) % 6).to_string())
                .replace(
                    "KEYWORD",
                    ["beach", "mountain", "family", "city"][(i / 50) % 4],
                );
            std::fs::write(dir.join(format!("IMG_{i:06}.xmp")), x).unwrap();
        }
    }
    bytes
}

fn rss() -> String {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let get = |k: &str| {
        s.lines()
            .find(|l| l.starts_with(k))
            .map(|l| l[k.len()..].trim().to_owned())
            .unwrap_or_default()
    };
    format!("RSS {} (peak {})", get("VmRSS:"), get("VmHWM:"))
}

fn load() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

/// CPU seconds used so far by this process's threads named `name` (Linux; 0 elsewhere): a
/// measure that holds up better than wall time on a loaded machine.
/// `""`: the whole process, threads that ended included.
fn thread_cpu(name: &str) -> f64 {
    let stats: Vec<String> = if name.is_empty() {
        std::fs::read_to_string("/proc/self/stat")
            .into_iter()
            .collect()
    } else {
        let Ok(rd) = std::fs::read_dir("/proc/self/task") else {
            return 0.0;
        };
        rd.flatten()
            .filter_map(|e| std::fs::read_to_string(e.path().join("stat")).ok())
            .collect()
    };
    let mut ticks = 0u64;
    for stat in stats {
        // pid (comm) state … utime stime: fields 14 and 15, counted after the comm.
        let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else {
            continue;
        };
        if !name.is_empty() && &stat[open + 1..close] != name {
            continue;
        }
        let f: Vec<&str> = stat[close + 2..].split(' ').collect();
        ticks += f.get(11).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        ticks += f.get(12).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    }
    ticks as f64 / 100.0
}

/// Follows the indexer from `t` until it is idle: prints when each phase ended (walking,
/// reading facts, then signatures and groups under Idle) and the memory at the end.
fn follow(lib: &Library, t: Instant, what: &str) -> Duration {
    let mut last = None;
    let mut ends: Vec<String> = Vec::new();
    let mut said = Instant::now();
    loop {
        let p = lib.progress();
        if last != Some(p.phase) {
            if let Some(l) = last {
                ends.push(format!(
                    "{l:?} to {:.2?} (CPU: indexer {:.2}s, process {:.2}s)",
                    t.elapsed(),
                    thread_cpu("lib-index"),
                    thread_cpu("")
                ));
            }
            last = Some(p.phase);
        }
        if lib.wait_idle(Duration::ZERO) {
            break;
        }
        if said.elapsed() > Duration::from_secs(30) {
            said = Instant::now();
            eprintln!(
                "  … {what}: {:?}, read {} of {}",
                p.phase, p.read, p.to_read
            );
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let all = t.elapsed();
    println!("{what}: {} → idle at {all:.2?}; {}", ends.join(", "), rss());
    all
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let files: usize = args.iter().find_map(|a| a.parse().ok()).unwrap_or(100_000);
    let keep = args.iter().any(|a| a == "--keep");
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
        .join("library-bench");
    let root = base.join("tree");
    let db = base.join("library.sqlite3");
    // `--reuse`: the tree and catalogue a `--keep` run left, searched again (and kept).
    let reuse = args.iter().any(|a| a == "--reuse") && db.exists();
    let keep = keep || reuse;
    println!(
        "machine: {} cores, load {}",
        std::thread::available_parallelism().map_or(0, |n| n.get()),
        load()
    );
    if !reuse {
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&root).unwrap();
        let links = args.iter().any(|a| a == "--links");
        let seed = base.join("seed");
        if links {
            std::fs::create_dir_all(&seed).unwrap();
        }
        let t = Instant::now();
        let bytes = generate(&root, files, links.then_some(seed.as_path()));
        println!(
            "generated {files} JPEGs ({:.1} MB) in {:.1?}",
            bytes as f64 / 1e6,
            t.elapsed()
        );
    }

    let config = || {
        let mut cfg = Config::new(db.clone());
        if args.iter().any(|a| a == "--no-signer") {
            cfg.signer = None;
        }
        cfg.on_thread_start = Some(specular_platform::lower_thread_priority);
        cfg.wake = Arc::new(|| {});
        cfg
    };
    let mut lib = Library::open(config()).unwrap();
    println!("open: {}", rss());

    if !reuse {
        index(&lib, &root, files, &args, keep, &base);
    }
    // A restart with nothing changed: what the indexer does before it is idle again (the
    // duplicate and similar-shot groups come back from the catalogue).
    if !args.iter().any(|a| a == "--quick") {
        drop(lib);
        // The peak from here on (Linux: resets VmHWM).
        let _ = std::fs::write("/proc/self/clear_refs", "5");
        let ops =
            specular_library::work_counts::MATCH_OPS.load(std::sync::atomic::Ordering::Relaxed);
        let t = Instant::now();
        lib = Library::open(config()).unwrap();
        follow(&lib, t, "restart, nothing changed");
        println!(
            "  matching operations at restart: {}",
            specular_library::work_counts::MATCH_OPS.load(std::sync::atomic::Ordering::Relaxed)
                - ops
        );
        if !reuse {
            // One file changed after the restart: the stored links are loaded, and only its
            // neighbourhood is matched.
            let i = files / 2;
            let p = root.join(format!(
                "{}/d{:03}/IMG_{i:06}.jpg",
                2016 + i / 12_500,
                (i / 100) % 125
            ));
            let data = std::fs::read(&p).unwrap();
            std::fs::remove_file(&p).unwrap();
            std::fs::write(&p, [&data[..], b"\0\0"].concat()).unwrap();
            let ops =
                specular_library::work_counts::MATCH_OPS.load(std::sync::atomic::Ordering::Relaxed);
            let t = Instant::now();
            lib.rescan(None);
            follow(&lib, t, "restarted, 1 file changed, rescan");
            println!(
                "  matching operations: {}",
                specular_library::work_counts::MATCH_OPS.load(std::sync::atomic::Ordering::Relaxed)
                    - ops
            );
        }
    }
    if args.iter().any(|a| a == "--quick") {
        return;
    }
    search(&lib, files, &args);
    println!("end: {}; load {}", rss(), load());
    let db_size: u64 = ["", "-wal", "-shm"]
        .iter()
        .filter_map(|s| {
            std::fs::metadata(format!("{}{s}", db.display()))
                .ok()
                .map(|m| m.len())
        })
        .sum();
    println!("catalogue on disk: {:.1} MB", db_size as f64 / 1e6);
    if keep {
        return;
    }
    let t = Instant::now();
    let id = lib.locations()[0].id;
    lib.remove_location(id);
    assert!(lib.wait_idle(Duration::from_secs(600)));
    assert_eq!(lib.counts().unwrap().0, 0);
    println!("location removed: {:.2?}", t.elapsed());
    drop(lib);
    std::fs::remove_dir_all(&base).unwrap();
}

/// The first index, a rescan with nothing changed, and one after 100 files changed.
fn index(lib: &Library, root: &Path, files: usize, args: &[String], keep: bool, base: &Path) {
    let t = Instant::now();
    lib.add_location(root);
    follow(lib, t, "first index");
    let (n, known) = lib.counts().unwrap();
    assert_eq!((n, known), (files as u64, files as u64));

    let t = Instant::now();
    lib.rescan(None);
    follow(lib, t, "rescan, nothing changed");
    // `--quick`: the indexing only.
    if args.iter().any(|a| a == "--quick") {
        if !keep {
            std::fs::remove_dir_all(base).unwrap();
        }
        return;
    }

    for i in (0..files).step_by(files / 100) {
        let p = root.join(format!(
            "{}/d{:03}/IMG_{i:06}.jpg",
            2016 + i / 12_500,
            (i / 100) % 125
        ));
        let data = std::fs::read(&p).unwrap();
        // A hard link is replaced, not written through (that would change its siblings).
        std::fs::remove_file(&p).unwrap();
        std::fs::write(&p, [&data[..], b"\0"].concat()).unwrap();
    }
    let t = Instant::now();
    lib.rescan(None);
    follow(
        lib,
        t,
        "100 files changed, rescan (the watcher sees them too)",
    );
}

/// Typical searches, and everything in each order.
fn search(lib: &Library, files: usize, args: &[String]) {
    // Develop edits: one photo in 50 edited, one in 500 with a second version.
    let rows = lib.search(&Search::default()).unwrap().rows;
    let mut edited = 0;
    for (k, r) in rows.iter().enumerate().filter(|(k, _)| k % 50 == 0) {
        let e = |v: u32| specular_library::edits::StoredEdit {
            edit: vec![v as u8; 64],
            hash: vec![k as u8; 32],
            steps: Vec::new(),
            position: 0,
            updated: 1_730_000_000,
            name: String::new(),
            modules: Some(
                [
                    "exposure\ncolour_balance",
                    "exposure",
                    "black_white\ncurves",
                ][k % 3]
                    .into(),
            ),
            styles: Some(if k % 100 == 0 {
                "Punchy".into()
            } else {
                String::new()
            }),
        };
        lib.save_edit(r.path.clone(), 0, e(0));
        if k % 500 == 0 {
            lib.save_edit(r.path.clone(), 1, e(1));
        }
        edited += 1;
    }
    drop(rows);
    assert!(lib.wait_idle(Duration::from_secs(600)));
    println!("{edited} photos given Develop edits");
    let queries = [
        ("everything", "", None, false),
        ("everything, first 500", "", Some(500), false),
        ("everything + facets", "", None, true),
        ("word (FTS)", "canon", None, false),
        ("file name", "IMG_04242", None, false),
        ("camera:", "camera:nikon", None, false),
        ("tag:", "tag:beach", None, false),
        ("rating>=3", "rating>=3", None, false),
        ("date:2019", "date:2019", None, false),
        ("in: folder", "in:2018/d042", None, false),
        ("iso>800 is:landscape", "iso>800 is:landscape", None, false),
        ("camera + facets, 500", "camera:sony", Some(500), true),
        ("negated text", "-canon", Some(500), false),
        ("edited:yes", "edited:yes", None, false),
        ("edited:no, first 500", "edited:no", Some(500), false),
        ("module:exposure", "module:exposure", None, false),
        ("style:punchy", "style:punchy", None, false),
        ("version:yes", "version:yes", None, false),
        ("edited:yes + facets", "edited:yes", None, true),
    ];
    println!("queries (median of 5; matches / SQL candidates):");
    for (what, q, limit, facets) in queries {
        let mut times = Vec::new();
        let mut total = 0;
        let mut cands = 0;
        for _ in 0..5 {
            let f = lib
                .search(&Search {
                    query: q.into(),
                    sort: Sort::Date,
                    descending: true,
                    limit,
                    facets,
                    ..Search::default()
                })
                .unwrap();
            times.push(f.elapsed);
            total = f.total;
            cands = f.candidates;
        }
        times.sort();
        println!(
            "  {what:<24} {:>9.2?}  {total:>6} / {cands:>6}",
            times[times.len() / 2]
        );
    }
    println!("everything, by sort (median of 3):");
    for sort in [
        Sort::Name,
        Sort::Date,
        Sort::Modified,
        Sort::Size,
        Sort::Rating,
        Sort::Folder,
        Sort::Unordered,
    ] {
        let mut times: Vec<Duration> = (0..3)
            .map(|_| {
                let t = Instant::now();
                let f = lib
                    .search(&Search {
                        sort,
                        ..Search::default()
                    })
                    .unwrap();
                assert_eq!(f.total, files);
                t.elapsed()
            })
            .collect();
        times.sort();
        println!("  {:<24} {:>9.2?}", format!("{sort:?}"), times[1]);
    }
    // `--profile`: the slowest search again and again, for a profiler.
    if args.iter().any(|a| a == "--profile") {
        for _ in 0..30 {
            let _ = lib.search(&Search {
                sort: Sort::Date,
                descending: true,
                ..Search::default()
            });
        }
    }
}
