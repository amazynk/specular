//! Messy real-world trees (real-world shakedown): Python
//! environments and `node_modules`, symbolic link loops and dangling links, unreadable files
//! and folders, zero-byte and truncated files of every format, wrong extensions, non-Latin,
//! decomposed and invalid UTF-8 names, very long paths, FIFOs, sockets and links to devices.
//! The Library's walk and facts pass, the folder listing and the thumbnail pipeline must get
//! through all of it without hanging, aborting or looping, and catalogue what is there.
//!
//! The trees are made under the temporary folder (`TMPDIR`) and removed afterwards. The test
//! with 200 000 files is `#[ignore]`d: `cargo test -p specular-library --test messy_trees --
//! --ignored`.
#![cfg(unix)]

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::{CancelToken, Source};
use specular_decode::Registry;
use specular_library::{Config, Library, Search};
use specular_thumbs::generate::{GenOptions, generate};
use specular_thumbs::list::{EntryKind, ListOptions, list_dir};

/// A temporary tree, made writable again and removed on drop.
struct Tree(PathBuf);

impl Tree {
    fn new(name: &str) -> Tree {
        let d = std::env::temp_dir().join(format!("specular-messy-{}-{name}", std::process::id()));
        unlock(&d);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        // The library keeps canonical paths; the temporary directory can be behind a symlink
        // (/var is /private/var on macOS).
        Tree(std::fs::canonicalize(&d).unwrap())
    }

    fn put(&self, rel: impl AsRef<Path>, body: &[u8]) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, body).unwrap();
        p
    }
}

/// Gives back the permissions the tree took away, so it can be removed.
fn unlock(d: &Path) {
    let Ok(rd) = std::fs::read_dir(d) else {
        let _ = std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o755));
        if let Ok(rd) = std::fs::read_dir(d) {
            drop(rd);
            unlock(d);
        }
        return;
    };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            let _ = std::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(0o755));
            unlock(&e.path());
        } else if ft.is_file() {
            let _ = std::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(0o644));
        }
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        unlock(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let px: Vec<u8> = (0..w * h * 3).map(|i| (i * 7) as u8).collect();
    specular_png::encode(w, h, specular_png::ColorType::Rgb, &px).unwrap()
}

fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let px: Vec<u8> = (0..w * h * 3).map(|i| (i * 13) as u8).collect();
    specular_jpeg::encode::encode(
        w,
        h,
        specular_jpeg::encode::Samples::Rgb(&px),
        &specular_jpeg::encode::EncodeOptions::default(),
    )
    .unwrap()
}

fn ppm(w: u32, h: u32) -> Vec<u8> {
    let mut v = format!("P6\n{w} {h}\n255\n").into_bytes();
    v.extend((0..w * h * 3).map(|i| (i * 5) as u8));
    v
}

fn bmp(w: u32, h: u32) -> Vec<u8> {
    let row = (w * 3).div_ceil(4) * 4;
    let size = 54 + row * h;
    let mut v = b"BM".to_vec();
    for x in [size, 0, 54, 40, w, h] {
        v.extend(x.to_le_bytes());
    }
    v.extend(1u16.to_le_bytes());
    v.extend(24u16.to_le_bytes());
    for x in [0u32, row * h, 2835, 2835, 0, 0] {
        v.extend(x.to_le_bytes());
    }
    v.extend((0..row * h).map(|i| (i * 3) as u8));
    v
}

fn mkfifo(p: &Path) {
    let ok = std::process::Command::new("mkfifo")
        .arg(p)
        .status()
        .is_ok_and(|s| s.success());
    assert!(ok, "mkfifo {}", p.display());
}

/// What the messy tree holds: the names the Library must catalogue (by name, not path).
struct Messy {
    tree: Tree,
    /// Valid images (name in the catalogue).
    good: Vec<String>,
}

/// Builds the messy tree. `long_path` adds a folder chain past `PATH_MAX`.
fn messy(name: &str) -> Messy {
    let t = Tree::new(name);
    let mut good = Vec::new();
    let reg = Registry::new();
    let (p, j, m, b) = (png(40, 30), jpeg(48, 32), ppm(9, 7), bmp(5, 3));

    // Plain images with plain, non-Latin, decomposed and composed names.
    for (n, body) in [
        ("photos/ok.png", &p),
        ("photos/ok.jpg", &j),
        ("photos/ok.ppm", &m),
        ("photos/ok.bmp", &b),
        ("photos/日本語の写真.png", &p),
        ("photos/صورة.jpg", &j),
        ("photos/Cafe\u{301}.png", &p),
        ("photos/Caf\u{e9}.png", &p),
        ("photos/emoji 📷 photo.jpg", &j),
        ("photos/space at end .png", &p),
        ("photos/new\nline.png", &p),
    ] {
        let path = t.put(n, body);
        // A file system that compares names by Unicode equivalence (APFS) stores the two
        // spellings of Café as one file, under the first name: expect only names on disk.
        let on_disk = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .any(|e| Some(e.unwrap().file_name().as_os_str()) == path.file_name());
        if !on_disk {
            continue;
        }
        good.push(
            Path::new(n)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        );
    }
    // A name that is not UTF-8.
    let bad =
        t.0.join("photos")
            .join(OsStr::from_bytes(b"bad\xff\xfename.png"));
    // A file system that keeps names as UTF-8 (APFS) refuses it.
    if let Err(e) = std::fs::write(&bad, &p)
        && !cfg!(target_os = "macos")
    {
        panic!("{}: {e}", bad.display());
    }

    // Zero-byte and garbage files of every registered extension; truncated real ones.
    for ext in reg.extensions() {
        t.put(format!("broken/zero.{ext}"), b"");
        t.put(format!("broken/garbage.{ext}"), &[0xa5; 300]);
        t.put(
            format!("broken/text.{ext}"),
            b"hello, this is not an image\n",
        );
    }
    for (stem, body) in [("png", &p), ("jpg", &j), ("ppm", &m), ("bmp", &b)] {
        for cut in [1, 8, 20, 33, 60, body.len() / 2, body.len() - 1] {
            if cut < body.len() {
                t.put(format!("broken/trunc-{cut}.{stem}"), &body[..cut]);
            }
        }
    }
    // Wrong extensions: the content wins.
    t.put("wrong/png-named.jpg", &p);
    t.put("wrong/jpeg-named.png", &j);
    t.put("wrong/ppm-named.tif", &m);
    good.extend(["png-named.jpg", "jpeg-named.png", "ppm-named.tif"].map(String::from));
    t.put("wrong/jpeg-named.dcm", &j);
    t.put("wrong/png-named.fits", &p);

    // Tool folders: not walked by the Library.
    t.put("project/venv/pyvenv.cfg", b"home = /usr/bin\n");
    t.put("project/venv/lib/python3.12/site-packages/pkg/icon.png", &p);
    let mut deep = PathBuf::from("project/web/node_modules");
    for i in 0..40 {
        deep.push(format!("dep{i}"));
        t.put(deep.join("logo.png"), &p);
        deep.push("node_modules");
    }

    // Links: loops, dangling, to devices, to a file (counted once per name).
    std::fs::create_dir_all(t.0.join("loops/a/b")).unwrap();
    symlink(".", t.0.join("loops/self")).unwrap();
    symlink("..", t.0.join("loops/up")).unwrap();
    symlink("../../..", t.0.join("loops/a/b/back")).unwrap();
    symlink("loops/a", t.0.join("loops/a/b/ring")).unwrap();
    symlink("nowhere.png", t.0.join("loops/dangling.png")).unwrap();
    symlink("dangling2.jpg", t.0.join("loops/dangling1.jpg")).unwrap();
    symlink("dangling1.jpg", t.0.join("loops/dangling2.jpg")).unwrap();
    symlink("../photos/ok.png", t.0.join("loops/linked.png")).unwrap();
    good.push("linked.png".into());
    // Links to the root of the file system, with and without an image's name.
    symlink("/", t.0.join("loops/root")).unwrap();
    symlink("/", t.0.join("loops/root.png")).unwrap();
    for (n, dev) in [
        ("loops/null.png", "/dev/null"),
        ("loops/zero.dcm", "/dev/zero"),
        ("loops/random.jpg", "/dev/urandom"),
        ("loops/IM0001", "/dev/zero"),
        ("loops/tty", "/dev/tty"),
    ] {
        symlink(dev, t.0.join(n)).unwrap();
    }

    // Unreadable files and folders.
    let locked = t.put("locked/secret.png", &p);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    t.put("locked/fine.png", &p);
    good.push("fine.png".into());
    t.put("lockeddir/x.png", &p);
    std::fs::set_permissions(
        t.0.join("lockeddir"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    t.put("noexec/y.png", &p);
    std::fs::set_permissions(t.0.join("noexec"), std::fs::Permissions::from_mode(0o644)).unwrap();

    // FIFOs and a socket with image names: opening one must never block.
    std::fs::create_dir_all(t.0.join("special")).unwrap();
    for n in [
        "pipe.png",
        "pipe.dcm",
        "pipe",
        "IM0002",
        "pipe.fits",
        "pipe.map",
    ] {
        mkfifo(&t.0.join("special").join(n));
    }
    // A socket path must be short: bind next to the test (relative), then move it in.
    let rel = format!("messy-{}-{name}.sock", std::process::id());
    let sock = std::os::unix::net::UnixListener::bind(&rel).unwrap();
    std::fs::rename(&rel, t.0.join("special/sock.jpg")).unwrap();
    drop(sock);

    // Near the platform's PATH_MAX (4 096 bytes on Linux, 1 024 on macOS): the long chain
    // and the deep nesting below stay under it.
    let (long_to, depth) = if cfg!(target_os = "macos") {
        (700, 300)
    } else {
        (3700, 1500)
    };
    // Long paths: a chain of 200-byte names to about 3 900 bytes (900 on macOS), an image at
    // the bottom.
    let mut long = PathBuf::from("long");
    while t.0.join(&long).as_os_str().len() < long_to {
        long.push("d".repeat(200));
    }
    t.put(long.join("deep.png"), &p);
    good.push("deep.png".into());
    // Deep nesting: 1 500 folders one in another (300 on macOS), an image at the bottom and
    // half way.
    let mut nest = PathBuf::from("nest");
    for i in 0..depth {
        nest.push("n");
        if i == depth / 2 {
            t.put(nest.join("middle.png"), &p);
        }
    }
    t.put(nest.join("bottom.png"), &p);
    good.extend(["middle.png", "bottom.png"].map(String::from));

    Messy { tree: t, good }
}

/// Runs `f` on a thread and fails if it takes longer than `limit` (a hang).
fn within<T: Send + 'static>(
    what: &str,
    limit: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    #[allow(clippy::disallowed_methods)]
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(limit) {
        Ok(v) => v,
        Err(_) => panic!("{what} did not finish within {limit:?}"),
    }
}

fn catalogued(lib: &Library) -> BTreeSet<String> {
    lib.search(&Search::default())
        .unwrap()
        .rows
        .into_iter()
        .map(|r| r.name)
        .collect()
}

#[test]
fn the_library_gets_through_a_messy_tree() {
    let m = messy("library");
    let root = m.tree.0.clone();
    let db = root.join(".catalogue/library.sqlite3");
    let lib = within("opening the Library", Duration::from_secs(30), move || {
        let mut cfg = Config::new(db);
        cfg.hold = Duration::from_millis(10);
        Library::open(cfg).unwrap()
    });
    lib.add_location(&root);
    assert!(
        lib.wait_idle(Duration::from_secs(120)),
        "the indexer did not settle on the messy tree: {:?}",
        lib.progress()
    );
    let names = catalogued(&lib);
    for g in &m.good {
        assert!(
            names.contains(g),
            "{g:?} not catalogued; catalogued: {names:?}"
        );
    }
    for skipped in [
        "icon.png", "logo.png", "x.png", "y.png", "pipe.png", "sock.jpg",
    ] {
        assert!(!names.contains(skipped), "{skipped} catalogued");
    }
    let loc = lib.locations().into_iter().next().expect("location");
    assert!(loc.files as usize >= m.good.len(), "{loc:?}");
    assert!(!loc.offline);
    // The pass over every file ended: nothing waits for its facts.
    assert_eq!(loc.pending, 0, "{loc:?}");
}

#[test]
fn listing_and_thumbnails_get_through_a_messy_tree() {
    let m = messy("thumbs");
    let root = m.tree.0.clone();
    let reg = Arc::new(Registry::new());
    let r = reg.clone();
    let entries = within("listing the tree", Duration::from_secs(60), move || {
        let mut all = Vec::new();
        let opts = ListOptions {
            show_hidden: true,
            show_other: true,
            depth: 4000,
            ..ListOptions::default()
        };
        let classify = move |p: &Path| {
            if r.supports_file(p) {
                EntryKind::Image
            } else {
                EntryKind::Other
            }
        };
        list_dir(&root, opts, &classify, &CancelToken::new(), &mut |c| {
            all.extend(c)
        })
        .unwrap();
        all
    });
    let names: BTreeSet<String> = entries
        .iter()
        .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    for g in &m.good {
        assert!(names.contains(g), "{g:?} not listed");
    }
    // Links into loops were not followed: every path is listed once, and none goes through
    // a link.
    let unique: BTreeSet<&PathBuf> = entries.iter().map(|e| &e.path).collect();
    assert_eq!(unique.len(), entries.len());
    assert!(
        entries
            .iter()
            .all(|e| !e.path.to_string_lossy().contains("/self/"))
    );
    // Special files are never listed.
    assert!(!names.contains("pipe.png") && !names.contains("sock.jpg"));

    // Every listed image: opened and thumbnailed as the gallery does, each within a budget,
    // without a panic; the good ones succeed.
    let images: Vec<PathBuf> = entries
        .iter()
        .filter(|e| e.kind == EntryKind::Image)
        .map(|e| e.path.clone())
        .collect();
    let good = m.good.clone();
    let failures = within(
        "thumbnailing the tree",
        Duration::from_secs(240),
        move || {
            let mut failures = Vec::new();
            for p in images {
                let t0 = Instant::now();
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let s = reg.open(Arc::new(Source::open(&p)?))?;
                    generate(
                        s.as_ref(),
                        256,
                        GenOptions::default(),
                        &CancelToken::new(),
                        &mut |_| {},
                    )
                }));
                let took = t0.elapsed();
                match r {
                    Err(_) => failures.push(format!("{}: panicked", p.display())),
                    Ok(Err(e)) if good.contains(&name) => {
                        failures.push(format!("{}: {e}", p.display()))
                    }
                    _ => {}
                }
                if took > Duration::from_secs(10) {
                    failures.push(format!("{}: took {took:?}", p.display()));
                }
            }
            failures
        },
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Opening special files through the decoders' front door fails at once (never blocks).
#[test]
fn special_files_fail_fast() {
    let t = Tree::new("special");
    let fifo = t.0.join("pipe.png");
    mkfifo(&fifo);
    symlink("/dev/zero", t.0.join("zero.dcm")).unwrap();
    let reg = Registry::new();
    for n in ["pipe.png", "zero.dcm"] {
        let p = t.0.join(n);
        let r = within(n, Duration::from_secs(10), move || {
            Registry::new().open_path(&p).map(|_| ())
        });
        assert!(r.is_err(), "{n} opened");
        assert!(reg.supports_extension(&t.0.join(n)));
    }
    let p = fifo.clone();
    let key = within("thumbnail key", Duration::from_secs(10), move || {
        specular_thumbs::ThumbKey::of_file(&p).is_err()
    });
    assert!(key, "a FIFO got a thumbnail key");
    let p = fifo.clone();
    let taken = within("date probe", Duration::from_secs(10), move || {
        specular_thumbs::list::probe_date_taken(&p)
    });
    assert!(taken.is_none());
}

/// 200 000 near-empty files in one folder: the walk lists them all and stays responsive.
#[test]
#[ignore = "makes and removes 200 000 files (~1 min); run with --ignored"]
fn a_folder_of_200k_files() {
    const N: usize = 200_000;
    let t = Tree::new("200k");
    let dir = t.0.join("many");
    std::fs::create_dir_all(&dir).unwrap();
    let made = Instant::now();
    for i in 0..N {
        std::fs::write(dir.join(format!("img{i:06}.jpg")), [0xffu8, 0xd8]).unwrap();
    }
    eprintln!("made {N} files in {:.1} s", made.elapsed().as_secs_f64());

    let d = dir.clone();
    let (listed, first) = within("listing", Duration::from_secs(120), move || {
        let t0 = Instant::now();
        let mut first = None;
        let mut n = 0;
        list_dir(
            &d,
            ListOptions::default(),
            &|_| EntryKind::Image,
            &CancelToken::new(),
            &mut |c| {
                first.get_or_insert(t0.elapsed());
                n += c.len();
            },
        )
        .unwrap();
        (n, first.unwrap())
    });
    assert_eq!(listed, N);
    eprintln!("first chunk after {first:?}");
    assert!(
        first < Duration::from_secs(5),
        "first chunk after {first:?}"
    );

    let db = t.0.join(".catalogue/library.sqlite3");
    let lib = within("opening", Duration::from_secs(30), move || {
        Library::open(Config::new(db)).unwrap()
    });
    let t0 = Instant::now();
    lib.add_location(&dir);
    assert!(
        lib.wait_idle(Duration::from_secs(900)),
        "not settled: {:?}",
        lib.progress()
    );
    let loc = lib.locations().into_iter().next().unwrap();
    eprintln!(
        "catalogued {} in {:.1} s ({} pending)",
        loc.files,
        t0.elapsed().as_secs_f64(),
        loc.pending
    );
    assert_eq!(loc.files as usize, N);
    assert_eq!(loc.pending, 0);
}

/// The image paths below `root` on disk now (regular files ending in `.png`).
fn pngs_on_disk(root: &Path) -> BTreeSet<PathBuf> {
    let mut out = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            if ft.is_dir() {
                if !p.ends_with(".catalogue") {
                    stack.push(p);
                }
            } else if ft.is_file() && p.extension().is_some_and(|x| x == "png") {
                out.insert(p);
            }
        }
    }
    out
}

fn catalogued_paths(lib: &Library) -> BTreeSet<PathBuf> {
    lib.search(&Search::default())
        .unwrap()
        .rows
        .into_iter()
        .map(|r| r.path)
        .collect()
}

fn library_at(root: &Path) -> Library {
    let db = root.join(".catalogue/library.sqlite3");
    within("opening the Library", Duration::from_secs(30), move || {
        let mut cfg = Config::new(db);
        cfg.signer = None;
        Library::open(cfg).unwrap()
    })
}

/// Files and folders vanish, and folders are replaced by files, while the walk goes on: each
/// costs only itself. The walk finishes, the location stays online, and a rescan agrees with
/// what is on disk.
#[test]
fn files_vanishing_mid_walk_cost_only_themselves() {
    let t = Tree::new("vanish");
    let p = png(4, 3);
    for k in 0..60 {
        for i in 0..100 {
            t.put(format!("f{k}.png/{i}.png"), &p);
        }
    }
    let root = t.0.clone();
    let lib = library_at(&root);
    lib.add_location(&root);
    let t0 = Instant::now();
    while lib.progress().phase != specular_library::Phase::Walking
        && t0.elapsed() < Duration::from_secs(5)
    {
        std::thread::sleep(Duration::from_millis(1));
    }
    // Meanwhile: every third folder goes, every fifth becomes a file of its name, and files
    // go from the rest.
    let r = root.clone();
    within("changing the tree", Duration::from_secs(60), move || {
        for k in 0..60 {
            let d = r.join(format!("f{k}.png"));
            if k % 3 == 0 {
                std::fs::remove_dir_all(&d).unwrap();
            } else if k % 5 == 0 {
                std::fs::remove_dir_all(&d).unwrap();
                std::fs::write(&d, png(2, 2)).unwrap();
            } else {
                for i in (0..100).step_by(7) {
                    let _ = std::fs::remove_file(d.join(format!("{i}.png")));
                }
            }
        }
    });
    assert!(
        lib.wait_idle(Duration::from_secs(120)),
        "not settled: {:?}",
        lib.progress()
    );
    let loc = lib.locations().into_iter().next().expect("location");
    assert!(!loc.offline, "{loc:?}");
    assert!(lib.progress().halt.is_none(), "{:?}", lib.progress());
    // Whatever the first walk saw, the next agrees with the disk.
    lib.rescan(None);
    assert!(lib.wait_idle(Duration::from_secs(120)));
    assert_eq!(catalogued_paths(&lib), pngs_on_disk(&root));
}

/// Between walks a folder becomes a file of the same name, a file becomes a folder, and a
/// folder becomes a link to `/`: the rows follow, and nothing else changes.
#[test]
fn a_folder_replaced_by_a_file_and_back() {
    let t = Tree::new("replaced");
    let p = png(4, 3);
    for n in [
        "a/1.png",
        "a/2.png",
        "b.png/3.png",
        "c/4.png",
        "keep/5.png",
        "keep/6.png",
    ] {
        t.put(n, &p);
    }
    t.put("d.png", &p);
    let root = t.0.clone();
    let lib = library_at(&root);
    lib.add_location(&root);
    assert!(lib.wait_idle(Duration::from_secs(60)));
    assert_eq!(catalogued_paths(&lib), pngs_on_disk(&root));
    assert_eq!(catalogued_paths(&lib).len(), 7);

    // b.png/ (a folder) becomes the image b.png; d.png (a file) becomes a folder holding
    // one; c/ becomes a link to the root of the file system.
    std::fs::remove_dir_all(root.join("b.png")).unwrap();
    std::fs::write(root.join("b.png"), &p).unwrap();
    std::fs::remove_file(root.join("d.png")).unwrap();
    t.put("d.png/7.png", &p);
    std::fs::remove_dir_all(root.join("c")).unwrap();
    symlink("/", root.join("c")).unwrap();
    lib.rescan(None);
    assert!(
        lib.wait_idle(Duration::from_secs(60)),
        "not settled: {:?}",
        lib.progress()
    );
    let after = catalogued_paths(&lib);
    assert_eq!(after, pngs_on_disk(&root));
    for (gone, there) in [("b.png/3.png", "b.png"), ("d.png", "d.png/7.png")] {
        assert!(!after.contains(&root.join(gone)), "{gone} still catalogued");
        assert!(after.contains(&root.join(there)), "{there} not catalogued");
    }
    assert!(!after.contains(&root.join("c/4.png")));
    assert!(after.contains(&root.join("keep/5.png")));
    assert!(!lib.locations()[0].offline);
}
