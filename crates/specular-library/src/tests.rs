#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rusqlite::params;

use super::*;

/// Facts from the file's text (`key=value` lines), so tests need no real images.
struct TextProber;

impl Prober for TextProber {
    fn probe(&self, path: &Path) -> Option<Probed> {
        // A decoder bug on this file (the name carries the test mark).
        specular_core::isolate::trip(path);
        let s = std::fs::read_to_string(path).ok()?;
        if s.starts_with("broken") {
            return None;
        }
        let mut p = Probed::default();
        p.meta.format = "JPEG".into();
        p.meta.width = 40;
        p.meta.height = 30;
        p.meta.gps = Some(false);
        for line in s.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k {
                "w" => p.meta.width = v.parse().unwrap(),
                "h" => p.meta.height = v.parse().unwrap(),
                "camera" => p.meta.camera = Some(v.into()),
                "lens" => p.meta.lens = Some(v.into()),
                "iso" => p.meta.iso = Some(v.parse().unwrap()),
                "taken" => p.meta.date_taken = Some(v.parse().unwrap()),
                "hdr" => p.meta.hdr = true,
                // A movie (`movie`) or sound file (`sound`), as its content says.
                "av" => {
                    p.meta.video = true;
                    p.meta.sound = v == "sound";
                }
                // A file (or drive) that takes this long to answer.
                "sleep-ms" => std::thread::sleep(Duration::from_millis(v.parse().unwrap())),
                "space" => p.meta.space = Some(v.into()),
                "pos" => {
                    let (a, o) = v.split_once(',').unwrap();
                    p.meta.position = Some((a.parse().unwrap(), o.parse().unwrap()));
                    p.meta.gps = Some(true);
                }
                "kw" => p.keywords = v.split(',').map(str::to_owned).collect(),
                // Exif: the maker, the Flash bits, and any tag (`tag=a406:3`).
                "make" | "flash" | "tag" => {
                    let e = p.meta.exif.get_or_insert_with(|| {
                        Box::new(specular_thumbs::ExifMeta {
                            tags: Some(String::new()),
                            ..Default::default()
                        })
                    });
                    let tags = e.tags.get_or_insert_with(String::new);
                    match k {
                        "make" => {
                            e.make = Some(v.into());
                            specular_query::exif::push_tag(tags, false, 0x010F, v);
                        }
                        "flash" => {
                            e.flash = Some(v.parse().unwrap());
                            specular_query::exif::push_tag(tags, false, 0x9209, v);
                        }
                        _ => {
                            let (key, value) = v.split_once(':').unwrap();
                            if !tags.is_empty() {
                                tags.push('\n');
                            }
                            tags.push_str(&format!("{key}={value}"));
                        }
                    }
                }
                // A song's lyrics, `|` between lines.
                "lyrics" => p.text = Some(v.replace('|', "\n")),
                // An astronomy subframe's header.
                "object" | "filter" => {
                    let a = p.meta.astro.get_or_insert_with(Default::default);
                    if k == "object" {
                        a.object = Some(v.into());
                    } else {
                        a.filter = Some(v.into());
                    }
                }
                _ => {}
            }
        }
        // A sound's `.lrc` beside it, preferred, as the sound reader has it.
        if p.meta.sound
            && let Ok(l) = std::fs::read_to_string(path.with_extension("lrc"))
        {
            p.text = Some(l);
        }
        Some(p)
    }

    /// `fwhm=N`: a subframe whose stars measure N px.
    fn measure(&self, path: &Path) -> Option<specular_decode::astro::FrameMetrics> {
        let s = std::fs::read_to_string(path).ok()?;
        let fwhm: f32 = s
            .lines()
            .find_map(|l| l.strip_prefix("fwhm="))?
            .parse()
            .ok()?;
        Some(specular_decode::astro::FrameMetrics {
            stars: 500,
            measured: 300,
            fwhm: Some(fwhm),
            hfd: Some(fwhm * 1.1),
            background: 1000.0,
            noise: 10.0,
            ..Default::default()
        })
    }
}

/// Signatures from the file's text: `look=N` (an image's look; equal looks are the same
/// pixels) or `near=N` (a different shot of look N: close hashes, other pixels).
struct TextSigner {
    /// Signatures made.
    made: std::sync::atomic::AtomicUsize,
}

fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

impl TextSigner {
    fn look(path: &Path) -> Option<(u64, bool)> {
        let s = std::fs::read_to_string(path).ok()?;
        s.lines().find_map(|l| match l.split_once('=')? {
            ("look", v) => Some((v.parse().ok()?, false)),
            ("near", v) => Some((v.parse().ok()?, true)),
            _ => None,
        })
    }
}

impl Signer for TextSigner {
    fn signature(
        &self,
        path: &Path,
        _: &specular_thumbs::ThumbKey,
        _: Option<&DiskCache>,
        _: u32,
    ) -> Option<specular_similar::Signature> {
        // A decoder bug that shows only when the pixels are read.
        if std::fs::read_to_string(path).is_ok_and(|s| s.contains("panic-on-pixels")) {
            panic!("test panic reading the pixels of {}", path.display());
        }
        let (n, near) = Self::look(path)?;
        self.made.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut sig = specular_similar::Signature {
            tier1: mix(n),
            phash: mix(n + 1),
            dhash: [mix(n + 2), mix(n + 3), mix(n + 4), mix(n + 5)],
            moments: [120, 128, 128, 40, 10, 10],
            aspect: 40.0 / 30.0,
            ..Default::default()
        };
        // The second shot is the blurrier one.
        let sharp = if near { 2.0 } else { 20.0 };
        sig.quality.grid = [sharp; 16];
        sig.quality.detail = sharp;
        sig.quality.mean = 0.45;
        if near {
            sig.tier1 = !sig.tier1;
            sig.phash ^= 0b101;
            sig.dhash[0] ^= 0b11;
        }
        Some(sig)
    }

    fn luma(
        &self,
        path: &Path,
        _: &specular_thumbs::ThumbKey,
        _: Option<&DiskCache>,
        _: u32,
    ) -> Option<specular_similar::Luma> {
        let (n, near) = Self::look(path)?;
        let seed = if near { mix(!n) } else { n };
        let px = (0..32 * 32)
            .map(|i| (mix(seed.wrapping_add(i)) % 256) as f32 / 255.0)
            .collect();
        Some(specular_similar::Luma { w: 32, h: 32, px })
    }
}

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let d =
            std::env::temp_dir().join(format!("specular-library-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        // The library keeps canonical paths (a location is canonicalised when added), and
        // the temporary directory can be behind a symlink (/var is /private/var on macOS).
        Dir(specular_platform::path::canonicalize(&d).unwrap())
    }

    fn put(&self, rel: &str, body: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, body).unwrap();
        p
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn config(db: PathBuf, watch: bool) -> Config {
    let mut c = Config::new(db);
    c.is_image = Arc::new(|p: &Path| {
        matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("jpg" | "png" | "cr2" | "JPG")
        )
    });
    c.prober = Arc::new(TextProber);
    c.signer = Some(Arc::new(TextSigner {
        made: Default::default(),
    }));
    c.watch = watch;
    c.offline_every = Duration::from_millis(50);
    c.hold = Duration::from_millis(50);
    c
}

fn open(dir: &Dir, watch: bool) -> Library {
    Library::open(config(dir.0.join("db/library.sqlite3"), watch)).unwrap()
}

fn idle(lib: &Library) {
    assert!(
        lib.wait_idle(Duration::from_secs(20)),
        "indexer did not settle"
    );
}

fn names(lib: &Library, q: &str) -> Vec<String> {
    let mut v: Vec<String> = lib
        .search(&Search {
            query: q.into(),
            ..Search::default()
        })
        .unwrap()
        .rows
        .into_iter()
        .map(|r| r.name)
        .collect();
    v.sort();
    v
}

fn row(lib: &Library, name: &str) -> Row {
    lib.search(&Search::default())
        .unwrap()
        .rows
        .into_iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("{name} not catalogued"))
}

/// Waits for `cond` (watcher tests).
fn eventually(mut cond: impl FnMut() -> bool) -> bool {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(15) {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

const SIDECAR: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmp:Rating="RATING" xmp:Label="Green"><dc:subject><rdf:Bag><rdf:li>Holiday</rdf:li><rdf:li>Beach</rdf:li></rdf:Bag></dc:subject></rdf:Description></rdf:RDF></x:xmpmeta>"#;

#[test]
fn migrations_bring_a_new_catalogue_to_the_current_schema() {
    let d = Dir::new("migrate");
    let db = d.0.join("c.sqlite3");
    let mut c = Connection::open(&db).unwrap();
    schema::configure(&c).unwrap();
    assert_eq!(schema::migrate(&mut c).unwrap(), 0);
    assert_eq!(schema::migrate(&mut c).unwrap(), schema::VERSION);
    let v: u32 = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(v, schema::VERSION);
    // Migration 3: the colour space, and the index that finds rows to read it for.
    c.prepare("SELECT space FROM files").unwrap();
    // Migration 4: signatures, groups and duplicate checks.
    c.prepare("SELECT file_id, size, mtime_ns, version, sig, blake3, seq FROM image_hashes")
        .unwrap();
    c.prepare("SELECT file_id, cat, grp, keep FROM match_groups")
        .unwrap();
    c.prepare("SELECT a, b, tier, score FROM match_verify")
        .unwrap();
    for t in [
        "image_hashes_seq",
        "match_groups_grp",
        "match_verify_b",
        "files_space_unread",
        "folders",
        "dirs",
        "files",
        "thumbs",
        "tags",
        "file_tags",
        "fts_index",
        "fts_dirs",
        "info",
    ] {
        let n: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
                [t],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "{t}");
    }
    // A newer catalogue is opened for search only.
    c.pragma_update(None, "user_version", schema::VERSION + 1)
        .unwrap();
    drop(c);
    let lib = Library::open(config(db, false)).unwrap();
    assert!(lib.read_only());
    assert!(lib.search(&Search::default()).unwrap().rows.is_empty());
}

const CACHEDIR_TAG: &str = "Signature: 8a477f597d28d172789f06886806bc55\n# a cache\n";

/// Counts the files probed (slowly), so a test can stop the reading part way.
struct SlowProber(Arc<std::sync::atomic::AtomicUsize>);

impl Prober for SlowProber {
    fn probe(&self, path: &Path) -> Option<Probed> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(2));
        TextProber.probe(path)
    }
}

/// Waits until `n` reaches `at`.
fn reach(n: &std::sync::atomic::AtomicUsize, at: usize) {
    let t = Instant::now();
    while n.load(std::sync::atomic::Ordering::SeqCst) < at {
        assert!(t.elapsed() < Duration::from_secs(20), "never reached {at}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A fold no index row agrees with by accident: text reversed.
fn reversed(s: &str) -> std::borrow::Cow<'_, str> {
    std::borrow::Cow::Owned(s.chars().rev().collect())
}

fn fts_rows(c: &Connection) -> (i64, i64) {
    c.query_row(
        "SELECT (SELECT COUNT(*) FROM fts_index), (SELECT COUNT(*) FROM fts_dirs)",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}

/// A synthetic catalogue of `n` files in an offline location (nothing is walked), with no
/// fold named: the indexer builds the full-text index when it opens it.
fn unindexed_catalogue(d: &Dir, n: usize) -> PathBuf {
    let db = d.0.join("c.sqlite3");
    let mut c = Connection::open(&db).unwrap();
    schema::configure(&c).unwrap();
    schema::migrate(&mut c).unwrap();
    c.execute_batch("BEGIN").unwrap();
    let gone = d.0.join("gone");
    c.execute(
        "INSERT INTO folders (path, added) VALUES (?1, 0)",
        [gone.to_str().unwrap()],
    )
    .unwrap();
    c.execute(
        "INSERT INTO dirs (folder_id, path) VALUES (1, ?1)",
        [gone.to_str().unwrap()],
    )
    .unwrap();
    for i in 1..=n {
        c.execute(
            "INSERT INTO files (id, dir_id, name, ext, size, mtime_ns, state, width, height)
             VALUES (?1, 1, ?2, 'jpg', 1000, 0, 1, 40, 30)",
            params![i as i64, format!("photo{i}.jpg")],
        )
        .unwrap();
    }
    c.execute_batch("COMMIT").unwrap();
    db
}

/// The counts of a `dupes:`/`similar:` value list: [any, keep, discard].
fn group_counts(lib: &Library, field: specular_query::facet::FacetField) -> [u32; 3] {
    let f = lib
        .search(&Search {
            facets: true,
            ..Search::default()
        })
        .unwrap()
        .facets;
    let mut out = [0; 3];
    if let Some(f) = f.iter().find(|f| f.field == field) {
        for v in &f.values {
            let i = specular_query::facet::GROUP_WORDS
                .iter()
                .position(|w| *w == v.key)
                .unwrap();
            out[i] = v.count;
        }
    }
    out
}

fn paths(lib: &Library, q: &str) -> Vec<PathBuf> {
    lib.search(&Search {
        query: q.into(),
        ..Search::default()
    })
    .unwrap()
    .rows
    .into_iter()
    .map(|r| r.path)
    .collect()
}

/// Signatures written by the bench; byte copies share a BLAKE3 (by name), every pixel and
/// visual check passes (stubbed, as in specular-similar's grouping bench).
struct BenchSigner;

impl Signer for BenchSigner {
    fn signature(
        &self,
        _: &Path,
        _: &specular_thumbs::ThumbKey,
        _: Option<&DiskCache>,
        _: u32,
    ) -> Option<specular_similar::Signature> {
        None
    }

    fn luma(
        &self,
        _: &Path,
        _: &specular_thumbs::ThumbKey,
        _: Option<&DiskCache>,
        _: u32,
    ) -> Option<specular_similar::Luma> {
        Some(specular_similar::Luma {
            w: 32,
            h: 32,
            px: (0..1024).map(|i| (i % 17) as f32 / 17.0).collect(),
        })
    }

    fn file_hash(&self, path: &Path) -> Option<[u8; 32]> {
        let name = path.file_stem()?.to_str()?;
        let base: u64 = name.split('_').next()?.parse().ok()?;
        let mut h = [0; 32];
        h[..8].copy_from_slice(&mix(base).to_le_bytes());
        Some(h)
    }
}

fn rss_kib() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |k: &str| {
        s.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    (field("VmRSS:"), field("VmHWM:"))
}

/// A synthetic signature in a scene of up to four shots (`j`: which), as the grouping bench
/// makes them.
fn bench_sig(r: &mut u64, base: (u64, [u64; 4], [u8; 6]), j: usize) -> specular_similar::Signature {
    let mut next = || {
        *r = r
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *r ^ (*r >> 29)
    };
    let (mut ph, mut dh, moments) = base;
    for _ in 0..j * 3 {
        ph ^= 1 << (next() % 64);
        dh[(next() % 4) as usize] ^= 1 << (next() % 64);
    }
    specular_similar::Signature {
        tier1: next(),
        phash: ph,
        dhash: dh,
        moments,
        aspect: 4.0 / 3.0,
        ..Default::default()
    }
}

fn lcg(r: &mut u64) -> u64 {
    *r = r
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *r ^ (*r >> 29)
}

/// A synthetic catalogue for the matcher, as the grouping bench makes one: `n` signed images
/// in scenes of up to four shots a second apart, one in fifty also kept as a byte copy (its
/// name `{original}_1.jpg`, a second newer, so the original is the one to keep). Returns how
/// many are copies.
fn synthetic_catalogue(c: &Connection, n: usize, seed: u64) -> usize {
    c.execute_batch("BEGIN").unwrap();
    c.execute("INSERT INTO folders (path, added) VALUES ('/bench', 0)", [])
        .unwrap();
    c.execute(
        "INSERT INTO dirs (folder_id, path) VALUES (1, '/bench')",
        [],
    )
    .unwrap();
    let mut r = seed;
    let mut taken = 1_600_000_000i64;
    let mut i = 0usize;
    let mut copies = 0;
    while i < n {
        let base = (
            lcg(&mut r),
            [lcg(&mut r), lcg(&mut r), lcg(&mut r), lcg(&mut r)],
            [
                (lcg(&mut r) % 256) as u8,
                (lcg(&mut r) % 256) as u8,
                (lcg(&mut r) % 256) as u8,
                (lcg(&mut r) % 128) as u8,
                (lcg(&mut r) % 64) as u8,
                (lcg(&mut r) % 64) as u8,
            ],
        );
        taken += (lcg(&mut r) % 600) as i64;
        let k = 1 + (lcg(&mut r) % 4) as usize;
        for j in 0..k {
            if i >= n {
                break;
            }
            let sig = bench_sig(&mut r, base, j);
            // One in fifty is also kept as a byte copy (a backup).
            let n_copies = if lcg(&mut r).is_multiple_of(50) { 2 } else { 1 };
            let size = 3_000_000 + (lcg(&mut r) % 1_000_000) as i64;
            let content = lcg(&mut r) as i64;
            for cp in 0..n_copies {
                if i >= n {
                    break;
                }
                copies += cp;
                i += 1;
                c.execute(
                    "INSERT INTO files (id, dir_id, name, ext, size, mtime_ns, state, width, height, taken)
                     VALUES (?1, 1, ?2, 'jpg', ?3, ?5, 1, 4000, 3000, ?4)",
                    params![
                        i as i64,
                        format!("{}_{cp}.jpg", i - cp),
                        size,
                        taken + j as i64,
                        cp as i64 * 1_000_000_000
                    ],
                )
                .unwrap();
                c.execute(
                    "INSERT INTO thumbs (file_id, content) VALUES (?1, ?2)",
                    params![i as i64, content],
                )
                .unwrap();
                c.execute(
                    "INSERT INTO image_hashes (file_id, size, mtime_ns, version, sig, seq)
                     VALUES (?1, ?2, ?5, ?3, ?4, ?1)",
                    params![
                        i as i64,
                        size,
                        i64::from(specular_similar::scan::SIG_DIGEST_ID),
                        sig.to_bytes().to_vec(),
                        cp as i64 * 1_000_000_000
                    ],
                )
                .unwrap();
            }
        }
    }
    c.execute_batch("COMMIT").unwrap();
    copies
}

/// New signatures for `ids` (a re-edit: another picture), numbered after the newest.
fn resign(c: &Connection, ids: &mut dyn Iterator<Item = i64>, r: &mut u64) {
    let mut seq: i64 = c
        .query_row("SELECT MAX(seq) FROM image_hashes", [], |r| r.get(0))
        .unwrap();
    c.execute_batch("BEGIN").unwrap();
    for id in ids {
        seq += 1;
        let base = (
            lcg(r),
            [lcg(r), lcg(r), lcg(r), lcg(r)],
            [128, 128, 128, 40, 10, 10],
        );
        let sig = bench_sig(r, base, 0);
        c.execute(
            "UPDATE image_hashes SET sig = ?2, seq = ?3 WHERE file_id = ?1",
            params![id, sig.to_bytes().to_vec(), seq],
        )
        .unwrap();
    }
    c.execute_batch("COMMIT").unwrap();
}

/// A new catalogue for the matcher alone (no indexer), signed by [`BenchSigner`].
fn matcher_catalogue(name: &str) -> (Dir, Connection, Config) {
    let d = Dir::new(name);
    let db = d.0.join("c.sqlite3");
    let mut c = Connection::open(&db).unwrap();
    schema::configure(&c).unwrap();
    schema::migrate(&mut c).unwrap();
    let mut cfg = config(db, false);
    cfg.signer = Some(Arc::new(BenchSigner));
    (d, c, cfg)
}

/// `match_groups`, in order: (file, category, group, keep).
fn match_rows(c: &Connection) -> Vec<(i64, i64, i64, bool)> {
    c.prepare("SELECT file_id, cat, grp, keep FROM match_groups ORDER BY file_id, cat")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

type Groups = [Vec<Vec<(i64, bool)>>; 2];

fn sorted_groups(by: [HashMap<i64, Vec<(i64, bool)>>; 2]) -> Groups {
    by.map(|m| {
        let mut v: Vec<Vec<(i64, bool)>> = m
            .into_values()
            .map(|mut g| {
                g.sort_unstable();
                g
            })
            .collect();
        v.sort();
        v
    })
}

/// The groups of `match_groups` rows, as sets of (file, keep).
fn groups_of(rows: &[(i64, i64, i64, bool)]) -> Groups {
    let mut by: [HashMap<i64, Vec<(i64, bool)>>; 2] = Default::default();
    for &(f, cat, g, keep) in rows {
        by[cat as usize].entry(g).or_default().push((f, keep));
    }
    sorted_groups(by)
}

/// The groups a scan of every signature at once finds (the gallery's way), with what
/// [`BenchSigner`] checks: byte copies by name, every pixel and visual check passing.
fn scan_groups(c: &Connection) -> Groups {
    use specular_similar::{Hints, Item, Kind, analyse, group};
    let mut keys = Vec::new();
    let mut items = Vec::new();
    let mut st = c
        .prepare(
            "SELECT h.file_id, h.sig, f.name, f.ext, f.size, f.mtime_ns, f.width, f.height, f.taken
             FROM image_hashes h JOIN files f ON f.id = h.file_id ORDER BY h.seq",
        )
        .unwrap();
    let mut q = st.query([]).unwrap();
    while let Some(r) = q.next().unwrap() {
        let Some(sig) = hashes::signature_of(r.get_ref(1).unwrap().as_blob().unwrap()) else {
            continue;
        };
        let name: String = r.get(2).unwrap();
        let ext: String = r.get(3).unwrap();
        let size = r.get::<_, i64>(4).unwrap() as u64;
        let taken: Option<i64> = r.get(8).unwrap();
        keys.push(r.get::<_, i64>(0).unwrap());
        items.push(Item {
            blake3: BenchSigner.file_hash(Path::new(&name)),
            name,
            kind: Kind::from_ext(&ext),
            width: r.get(6).unwrap(),
            height: r.get(7).unwrap(),
            bytes: size,
            len: size,
            modified: r.get::<_, i64>(5).unwrap().div_euclid(1_000_000_000),
            sig,
            hints: Hints {
                time_ms: taken.map(|t| t * 1000),
                has_exif: taken.is_some(),
                ..Hints::default()
            },
            ..Item::default()
        });
    }
    let index = analyse::index(&items);
    let dups = analyse::duplicates(&items, &index, &|_, _| false, &mut |_| Some(1.0));
    let mut sim_items = items.clone();
    for g in &dups {
        for (m, p) in g.members.iter().zip(&g.picks) {
            if !p.keep {
                sim_items[*m as usize].gone = true;
            }
        }
    }
    let edges = group::similar_edges(&sim_items, &index, &[]);
    let sims = analyse::similar(
        &sim_items,
        &edges,
        specular_similar::live::DEFAULT_SENSITIVITY,
        &|_, _| false,
    );
    let mut by: [HashMap<i64, Vec<(i64, bool)>>; 2] = Default::default();
    for (cat, gs) in [dups, sims].into_iter().enumerate() {
        for (n, g) in gs.into_iter().enumerate() {
            for (m, p) in g.members.iter().zip(&g.picks) {
                by[cat]
                    .entry(n as i64)
                    .or_default()
                    .push((keys[*m as usize], p.keep));
            }
        }
    }
    sorted_groups(by)
}

/// Every state a reader connection sees of `match_groups` while `run` goes on.
fn observed(db: &Path, run: impl FnOnce()) -> Vec<Vec<(i64, i64, i64, bool)>> {
    use std::sync::atomic::{AtomicBool, Ordering};
    let stop = AtomicBool::new(false);
    std::thread::scope(|s| {
        let reader = s.spawn(|| {
            let r = Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
            schema::configure_reader(&r).unwrap();
            let mut seen: Vec<Vec<(i64, i64, i64, bool)>> = Vec::new();
            let mut reads = 0;
            loop {
                let last = stop.load(Ordering::Acquire);
                let rows = match_rows(&r);
                reads += 1;
                if !seen.contains(&rows) {
                    seen.push(rows);
                }
                if last {
                    break;
                }
            }
            assert!(reads > 10, "{reads}");
            seen
        });
        run();
        stop.store(true, Ordering::Release);
        reader.join().unwrap()
    })
}

fn percentile(v: &mut [f64], p: usize) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get((v.len() * p / 100).min(v.len().saturating_sub(1)))
        .copied()
        .unwrap_or(0.0)
}

/// A tree that takes a while to walk: `folders` folders of `per` images, each listed in
/// about 1 ms (counted in the returned counter).
fn slow_tree(
    d: &Dir,
    root: &str,
    folders: usize,
    per: usize,
) -> (Config, Arc<std::sync::atomic::AtomicUsize>) {
    for f in 0..folders {
        for i in 0..per {
            d.put(&format!("{root}/f{f}/{i}.jpg"), "camera=Canon");
        }
    }
    let listed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut c = config(d.0.join("db/library.sqlite3"), false);
    let l2 = listed.clone();
    c.is_image = Arc::new(move |p: &Path| {
        l2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(1));
        p.extension().is_some_and(|e| e == "jpg")
    });
    (c, listed)
}

/// Waits for `cond`, polling every 2 ms; how long it took.
fn within(limit: Duration, mut cond: impl FnMut() -> bool) -> Option<Duration> {
    let t = Instant::now();
    while t.elapsed() < limit {
        if cond() {
            return Some(t.elapsed());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    None
}

mod albums_tests;
mod concurrency_tests;
mod develop_tests;
mod exif_tests;
mod facts_tests;
mod groups_tests;
mod iptc_tests;
mod local_tests;
mod search_tests;
mod sidecar_tests;
mod walk_tests;
