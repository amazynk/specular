//! Download, install and removal against `file://` sources in private directories (no
//! network).
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

mod common;

use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::{Dir, bytes, catalogue, entry};
use specular_ai::install::{InstallError, Store, Transfer};
use specular_ai::transport::{Body, FileTransport, NO_HTTPS, Transport, Transports};
use specular_ai::{Feature, FeatureState, Manifest, Pack, Tier};

fn manifest(entries: &[String]) -> Manifest {
    Manifest::parse(&format!("schema = 1\n{}", entries.concat())).unwrap()
}

fn table_exists(path: &std::path::Path, name: &str) -> bool {
    let c = rusqlite::Connection::open(path).unwrap();
    c.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |r| r.get::<_, i64>(0),
    )
    .unwrap()
        == 1
}

/// What the pack's background work reported, each asserted to have succeeded.
fn finished_ok(pack: &Pack) -> Vec<String> {
    pack.take_finished()
        .into_iter()
        .map(|(what, r)| {
            assert!(r.is_ok(), "{what}: {r:?}");
            what
        })
        .collect()
}

#[test]
fn installs_feature_atomically_and_removes_it_with_its_tables() {
    let d = Dir::new("install");
    let src = d.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    let a = bytes(300_000, 1);
    let b = bytes(70_000, 2);
    let m = manifest(&[
        entry(&src, "people", "detector", &a, &[]),
        entry(&src, "people", "faces", &b, &[]),
        entry(&src, "search", "embed", &bytes(1000, 3), &[]),
    ]);
    let db = d.path().join("library.sqlite3");
    catalogue(&db, 3, |_| true);
    let data = d.path().join("data");
    let pack = Pack::new(
        m,
        &data,
        Some(db.clone()),
        Transports::default(),
        Arc::new(|| {}),
    )
    .with_placeholders();
    assert!(pack.has_offers());
    let st = pack.status();
    assert_eq!(st[3].feature, Feature::People);
    assert_eq!(st[3].state, FeatureState::Available);
    assert_eq!(st[3].offer.as_ref().unwrap().size(), 370_000);
    assert_eq!(st[1].state, FeatureState::Unavailable, "no text model");

    assert!(pack.install(Feature::People, Tier::Standard));
    pack.join();
    assert_eq!(pack.installed(), vec![Feature::People]);
    let models = pack.installed_models();
    assert_eq!(models.len(), 2);
    for (m, body) in models.iter().zip([&a, &b]) {
        let file = data.join("ai").join(&m.file);
        assert_eq!(&std::fs::read(file).unwrap(), body);
    }
    assert!(table_exists(&db, "faces"));
    assert!(!table_exists(&db, "embeddings"));
    // No partial downloads or temporary directories left.
    let leftovers: Vec<_> = std::fs::read_dir(data.join("ai/downloads"))
        .unwrap()
        .collect();
    assert!(leftovers.is_empty());
    let dirs: Vec<String> = std::fs::read_dir(data.join("ai/models"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(dirs.len(), 2, "{dirs:?}");
    assert!(dirs.iter().all(|n| !n.starts_with('.')));
    assert_eq!(pack.counts(Feature::People), Some((3, 0)));

    assert!(pack.install(Feature::Search, Tier::Standard));
    pack.join();
    assert_eq!(pack.installed(), vec![Feature::Search, Feature::People]);

    assert!(pack.remove(Feature::People));
    pack.join();
    assert_eq!(finished_ok(&pack), ["remove people"]);
    assert_eq!(pack.status()[3].state, FeatureState::Available);
    assert_eq!(pack.installed(), vec![Feature::Search]);
    assert!(!table_exists(&db, "faces"));
    assert!(!table_exists(&db, "people"));
    assert!(table_exists(&db, "embeddings"));
    assert!(!data.join("ai/models/detector-1").exists());

    assert!(pack.remove_all());
    pack.join();
    assert_eq!(finished_ok(&pack), ["remove pack"]);
    assert!(pack.installed().is_empty());
    assert!(!table_exists(&db, "embeddings"));
    assert!(!data.join("ai").exists());
    // The core register stays, empty.
    assert!(table_exists(&db, "ai_schema"));
}

/// Serves a file but stops after `cut` bytes on the first open (a dropped connection).
struct Flaky {
    cut: usize,
    opens: AtomicUsize,
    offsets: parking_lot::Mutex<Vec<u64>>,
}

struct Cut<R> {
    inner: R,
    left: usize,
}

impl<R: Read> Read for Cut<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Err(io::Error::new(io::ErrorKind::ConnectionReset, "reset"));
        }
        let n = buf.len().min(self.left);
        let n = self.inner.read(&mut buf[..n])?;
        self.left -= n;
        Ok(n)
    }
}

impl Transport for Flaky {
    fn serves(&self, url: &str) -> bool {
        url.starts_with("file://")
    }

    fn open(&self, url: &str, offset: u64) -> io::Result<Body> {
        self.offsets.lock().push(offset);
        let mut b = FileTransport.open(url, offset)?;
        if self.opens.fetch_add(1, Ordering::Relaxed) == 0 {
            b.reader = Box::new(Cut {
                inner: b.reader,
                left: self.cut,
            });
        }
        Ok(b)
    }
}

#[test]
fn resumes_an_interrupted_download_and_verifies_it() {
    let d = Dir::new("resume");
    let body = bytes(500_000, 7);
    let src = d.path();
    // Two mirrors of the same file: the first connection drops, the second resumes.
    let first = format!("file://{}", src.join("embed.bin").display());
    let m = manifest(&[entry(src, "search", "embed", &body, &[first])]);
    let offer = m.offer(Feature::Search, true).unwrap();
    let store = Store::at(d.path().join("ai"));
    let flaky = Flaky {
        cut: 123_457,
        opens: AtomicUsize::new(0),
        offsets: Default::default(),
    };
    let t = Transfer::default();
    store.install(&offer, &flaky, &t, None).unwrap();
    assert_eq!(*flaky.offsets.lock(), vec![0, 123_457]);
    assert_eq!(t.get(), (500_000, 500_000));
    assert_eq!(store.features(), vec![Feature::Search]);
}

#[test]
fn resumes_from_a_partial_file_left_by_an_earlier_run() {
    let d = Dir::new("partial");
    let body = bytes(200_000, 9);
    let m = manifest(&[entry(d.path(), "text", "ocr", &body, &[])]);
    let offer = m.offer(Feature::Text, true).unwrap();
    let store = Store::at(d.path().join("ai"));
    let part = store.part_path(&offer.models[0]);
    std::fs::create_dir_all(part.parent().unwrap()).unwrap();
    std::fs::write(&part, &body[..80_000]).unwrap();
    let flaky = Flaky {
        cut: usize::MAX,
        opens: AtomicUsize::new(1),
        offsets: Default::default(),
    };
    store
        .install(&offer, &flaky, &Transfer::default(), None)
        .unwrap();
    assert_eq!(*flaky.offsets.lock(), vec![80_000]);
    assert!(store.has(Feature::Text));

    // A corrupt partial file (wrong prefix) is caught by the hash and deleted.
    let d2 = Dir::new("partial-bad");
    let m = manifest(&[entry(d2.path(), "text", "ocr", &body, &[])]);
    let offer = m.offer(Feature::Text, true).unwrap();
    let store = Store::at(d2.path().join("ai"));
    let part = store.part_path(&offer.models[0]);
    std::fs::create_dir_all(part.parent().unwrap()).unwrap();
    std::fs::write(&part, vec![0u8; 80_000]).unwrap();
    let e = store
        .install(&offer, &FileTransport, &Transfer::default(), None)
        .unwrap_err();
    assert!(matches!(e, InstallError::Hash { .. }), "{e}");
    assert!(!part.exists());
    assert!(!store.has(Feature::Text));
    // The next try starts over and succeeds.
    store
        .install(&offer, &FileTransport, &Transfer::default(), None)
        .unwrap();
    assert!(store.has(Feature::Text));
}

#[test]
fn refuses_wrong_hash_and_wrong_size() {
    let d = Dir::new("refuse");
    let body = bytes(10_000, 4);
    let good = entry(d.path(), "culling", "cull", &body, &[]);
    // The source file differs from what the manifest names.
    std::fs::write(d.path().join("cull.bin"), bytes(10_000, 5)).unwrap();
    let m = manifest(std::slice::from_ref(&good));
    let offer = m.offer(Feature::Culling, true).unwrap();
    let store = Store::at(d.path().join("ai"));
    let e = store
        .install(&offer, &FileTransport, &Transfer::default(), None)
        .unwrap_err();
    assert!(matches!(e, InstallError::Hash { .. }), "{e}");
    assert!(store.installed().is_empty());
    // Bigger than the manifest says.
    std::fs::write(d.path().join("cull.bin"), bytes(10_001, 4)).unwrap();
    let e = store
        .install(&offer, &FileTransport, &Transfer::default(), None)
        .unwrap_err();
    assert!(matches!(e, InstallError::Size { .. }), "{e}");
    assert!(store.installed().is_empty());
}

#[test]
fn cancel_keeps_the_partial_download() {
    let d = Dir::new("cancel");
    let body = bytes(400_000, 6);
    let m = manifest(&[entry(d.path(), "search", "embed", &body, &[])]);
    let offer = m.offer(Feature::Search, true).unwrap();
    let store = Store::at(d.path().join("ai"));
    let t = Transfer::default();
    t.cancel();
    let e = store.install(&offer, &FileTransport, &t, None).unwrap_err();
    assert!(matches!(e, InstallError::Cancelled));
    assert!(store.installed().is_empty());
}

/// A body stopped from outside (the app's "Downloads" panel) after `left` bytes.
struct StoppedAfter<R> {
    inner: R,
    left: usize,
}

impl<R: Read> Read for StoppedAfter<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Err(specular_ai::transport::stopped());
        }
        let n = buf.len().min(self.left);
        let n = self.inner.read(&mut buf[..n])?;
        self.left -= n;
        Ok(n)
    }
}

struct StopsMidway(AtomicUsize);

impl Transport for StopsMidway {
    fn serves(&self, url: &str) -> bool {
        url.starts_with("file://")
    }

    fn open(&self, url: &str, offset: u64) -> io::Result<Body> {
        self.0.fetch_add(1, Ordering::Relaxed);
        let mut b = FileTransport.open(url, offset)?;
        b.reader = Box::new(StoppedAfter {
            inner: b.reader,
            left: 100_000,
        });
        Ok(b)
    }
}

/// Stop in "Downloads" ends the install as cancelled (the row goes back, no failure), keeps
/// what came for a resume, and tries no other mirror.
#[test]
fn a_download_stopped_elsewhere_is_a_cancel_not_a_failure() {
    let d = Dir::new("stopped");
    let body = bytes(300_000, 8);
    let src = d.path();
    let mirror = format!("file://{}", src.join("embed.bin").display());
    let m = manifest(&[entry(src, "search", "embed", &body, &[mirror])]);
    let offer = m.offer(Feature::Search, true).unwrap();
    let store = Store::at(d.path().join("ai"));
    let t = StopsMidway(AtomicUsize::new(0));
    let e = store
        .install(&offer, &t, &Transfer::default(), None)
        .unwrap_err();
    assert!(matches!(e, InstallError::Cancelled), "{e}");
    assert_eq!(t.0.load(Ordering::Relaxed), 1, "no second mirror");
    let part = store.part_path(&offer.models[0]);
    assert_eq!(std::fs::metadata(&part).unwrap().len(), 100_000);
    assert!(store.installed().is_empty());
    assert!(!specular_ai::transport::is_stopped(&io::Error::other(
        "cancelled"
    )));
}

#[test]
fn https_needs_a_transport_this_build_lacks() {
    let d = Dir::new("https");
    let body = bytes(100, 1);
    let mut e = entry(d.path(), "search", "embed", &body, &[]);
    // Quoted as `entry` writes it ({:?}: a Windows path's `\` is escaped).
    let file_url = format!("file://{}", d.path().join("embed.bin").display());
    e = e.replace(
        &format!("{file_url:?}"),
        "\"https://models.invalid/embed.bin\"",
    );
    let m = manifest(&[e]);
    let offer = m.offer(Feature::Search, true).unwrap();
    let store = Store::at(d.path().join("ai"));
    let err = store
        .install(&offer, &Transports::default(), &Transfer::default(), None)
        .unwrap_err();
    assert!(err.to_string().contains(NO_HTTPS), "{err}");
}

#[test]
fn empty_manifest_offers_nothing() {
    let d = Dir::new("empty");
    let pack = Pack::new(
        Manifest::default(),
        d.path(),
        None,
        Transports::default(),
        Arc::new(|| {}),
    );
    assert!(!pack.has_offers());
    assert!(
        pack.status()
            .iter()
            .all(|s| s.state == FeatureState::Unavailable)
    );
    assert!(!pack.install(Feature::People, Tier::Standard));
    assert!(!d.path().join("ai").exists(), "nothing written");
}

#[test]
fn tidy_removes_leftovers() {
    let d = Dir::new("tidy");
    let ai = d.path().join("ai");
    std::fs::create_dir_all(ai.join("models/.tmp-x-1")).unwrap();
    std::fs::create_dir_all(ai.join("models/old-1")).unwrap();
    std::fs::create_dir_all(ai.join("downloads")).unwrap();
    std::fs::write(ai.join("downloads/ab.part"), b"x").unwrap();
    Store::at(ai.clone()).tidy(&Manifest::default());
    assert_eq!(std::fs::read_dir(ai.join("models")).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(ai.join("downloads")).unwrap().count(), 0);
}

#[test]
fn people_controls() {
    let d = Dir::new("people");
    let db = d.path().join("library.sqlite3");
    catalogue(&db, 2, |_| true);
    let m = manifest(&[entry(d.path(), "people", "det", &bytes(64, 1), &[])]);
    let pack = Pack::new(
        m,
        d.path(),
        Some(db.clone()),
        Transports::default(),
        Arc::new(|| {}),
    )
    .with_placeholders();
    assert!(pack.people().is_empty());
    pack.install(Feature::People, Tier::Standard);
    pack.join();
    let mut conn = specular_library::ai::open(&db).unwrap();
    let face = specular_library::ai::Face {
        x: 0.1,
        y: 0.1,
        w: 0.2,
        h: 0.2,
        score: 0.9,
        vec: vec![],
        roll: 0.0,
    };
    specular_library::ai::put_faces(&mut conn, 1, "det-1", &[face.clone(), face]).unwrap();
    let ana = specular_library::ai::add_person(&conn, Some("Ana")).unwrap();
    let ids = specular_library::ai::faces_of(&conn, 1).unwrap();
    specular_library::ai::assign_faces(&mut conn, Some(ana), &ids[..1]).unwrap();
    let ben = specular_library::ai::add_person(&conn, None).unwrap();
    specular_library::ai::assign_faces(&mut conn, Some(ben), &ids[1..]).unwrap();
    assert_eq!(pack.people().len(), 2);
    pack.forget_person(ana);
    pack.join();
    assert_eq!(pack.people().len(), 1);
    pack.forget_all_faces();
    pack.join();
    assert!(pack.people().is_empty());
    assert_eq!(
        finished_ok(&pack),
        [format!("forget person {ana}"), "delete face data".into()]
    );
    assert_eq!(pack.counts(Feature::People), Some((2, 0)));
    // Grouping is off in a library until turned on there.
    assert_eq!(pack.face_grouping(), Some(false));
    pack.set_face_grouping(true);
    pack.join();
    assert_eq!(pack.face_grouping(), Some(true));
    assert_eq!(finished_ok(&pack), ["face grouping on"]);
}

/// Removing waits out another writer's transaction on the pack's own thread: the call returns
/// at once, the feature shows as removing meanwhile and can be neither removed nor installed
/// again until the removal ends.
#[test]
fn removal_runs_off_the_caller_while_a_writer_holds_the_catalogue() {
    let d = Dir::new("remove-busy");
    let db = d.path().join("library.sqlite3");
    catalogue(&db, 2, |_| true);
    let m = manifest(&[entry(d.path(), "people", "det", &bytes(64, 1), &[])]);
    let woken = Arc::new(AtomicUsize::new(0));
    let w = woken.clone();
    let pack = Pack::new(
        m,
        &d.path().join("data"),
        Some(db.clone()),
        Transports::default(),
        Arc::new(move || {
            w.fetch_add(1, Ordering::Relaxed);
        }),
    )
    .with_placeholders();
    assert!(pack.install(Feature::People, Tier::Standard));
    pack.join();
    assert_eq!(pack.status()[3].state, FeatureState::Installed);

    // Another writer holds the catalogue for a while.
    let hold = std::time::Duration::from_millis(1500);
    let (locked_tx, locked_rx) = std::sync::mpsc::channel();
    let locker = {
        let db = db.clone();
        std::thread::spawn(move || {
            let c = rusqlite::Connection::open(&db).unwrap();
            c.execute_batch("BEGIN IMMEDIATE; CREATE TABLE held(x);")
                .unwrap();
            locked_tx.send(()).unwrap();
            std::thread::sleep(hold);
            c.execute_batch("ROLLBACK").unwrap();
        })
    };
    locked_rx.recv().unwrap();
    let t = std::time::Instant::now();
    let changes = pack.changes();
    assert!(pack.remove(Feature::People));
    let call = t.elapsed();
    assert!(
        call < std::time::Duration::from_millis(50),
        "remove took {call:?}"
    );
    assert_eq!(pack.status()[3].state, FeatureState::Removing);
    assert!(pack.removing());
    assert!(!pack.remove(Feature::People), "not started twice");
    assert!(
        !pack.install(Feature::People, Tier::Standard),
        "not installed while removing"
    );
    assert!(pack.take_finished().is_empty());
    pack.join();
    assert!(t.elapsed() >= hold / 2, "the removal waited for the writer");
    locker.join().unwrap();
    assert_eq!(finished_ok(&pack), ["remove people"]);
    assert!(pack.changes() > changes);
    assert!(woken.load(Ordering::Relaxed) > 0);
    assert!(!pack.removing());
    assert_eq!(pack.status()[3].state, FeatureState::Available);
    assert!(!table_exists(&db, "faces"));

    // The whole pack, the same way.
    assert!(pack.install(Feature::People, Tier::Standard));
    pack.join();
    assert_eq!(pack.status()[3].state, FeatureState::Installed);
    assert!(pack.remove_all());
    assert!(!pack.remove_all(), "not started twice");
    pack.join();
    assert_eq!(finished_ok(&pack), ["remove pack"]);
    assert_eq!(pack.status()[3].state, FeatureState::Available);
}

/// An install or a removal that panics (the test hook trips on a catalogue named for it)
/// ends as a failure the app can name, never as a row silently back at "available" or a
/// removal that never reports.
#[test]
fn a_panicking_install_or_removal_is_reported() {
    let d = Dir::new("panic");
    let db = d.path().join("x-specular-test-panic.sqlite3");
    catalogue(&db, 2, |_| true);
    let m = manifest(&[entry(d.path(), "people", "det", &bytes(64, 1), &[])]);
    let woken = Arc::new(AtomicUsize::new(0));
    let w = woken.clone();
    let pack = Pack::new(
        m,
        &d.path().join("data"),
        Some(db),
        Transports::default(),
        Arc::new(move || {
            w.fetch_add(1, Ordering::Relaxed);
        }),
    )
    .with_placeholders();
    assert!(pack.install(Feature::People, Tier::Standard));
    pack.join();
    assert_eq!(
        pack.status()[3].state,
        FeatureState::Failed(specular_ai::pack::INTERNAL_ERROR.into())
    );
    let before = (woken.load(Ordering::Relaxed), pack.changes());
    assert!(pack.remove(Feature::People));
    pack.join();
    assert!(!pack.removing());
    let finished = pack.take_finished();
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].0, "remove people");
    assert_eq!(
        finished[0].1,
        Err(specular_ai::pack::INTERNAL_ERROR.to_owned())
    );
    assert!(pack.changes() > before.1, "the page is told to rebuild");
    assert!(woken.load(Ordering::Relaxed) > before.0);
}

/// Waits for an add-on's install or removal to end (its state leaves Downloading/Removing).
fn settle_addon(pack: &Pack, id: &str) -> FeatureState {
    for _ in 0..500 {
        let st = pack.addon_status(Feature::Text);
        let s = st.iter().find(|a| a.id == id).unwrap().state.clone();
        if !matches!(s, FeatureState::Downloading { .. } | FeatureState::Removing) {
            return s;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("{id} did not settle");
}

#[test]
fn addons_install_beside_the_size_stay_across_reinstalls_and_go_with_the_feature() {
    let d = Dir::new("addons");
    let src = d.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    let with = |e: String, slot: &str| e.replacen("\nid = ", &format!("\n{slot}\nid = "), 1);
    let text = format!(
        "schema = 3\n{}{}{}\n[[choice]]\nfeature = \"text\"\ntier = \"standard\"\nname = \"OCR\"\ndefault = true\n\n[[choice]]\nfeature = \"text\"\naddon = \"arabic\"\nname = \"OCR Arabic\"\nlanguages = \"arabic\"\n",
        with(
            entry(&src, "text", "det", &bytes(5000, 1), &[]),
            "tier = \"standard\""
        ),
        with(
            entry(&src, "text", "rec", &bytes(4000, 2), &[]),
            "tier = \"standard\""
        ),
        with(
            entry(&src, "text", "arabic", &bytes(3000, 3), &[]),
            "addon = \"arabic\""
        ),
    );
    let m = Manifest::parse(&text).unwrap();
    assert_eq!(m.addon_models(Feature::Text, "arabic", true).len(), 1);
    let data = d.path().join("data");
    let pack =
        Pack::new(m, &data, None, Transports::default(), Arc::new(|| {})).with_placeholders();
    let st = pack.addon_status(Feature::Text);
    assert_eq!(st.len(), 1);
    assert_eq!(
        (st[0].state.clone(), st[0].size),
        (FeatureState::Available, 3000)
    );

    assert!(pack.install(Feature::Text, Tier::Standard));
    pack.join();
    assert!(pack.install_addon(Feature::Text, "arabic"));
    assert_eq!(settle_addon(&pack, "arabic"), FeatureState::Installed);
    assert_eq!(pack.store().addons(Feature::Text), ["arabic"]);
    assert_eq!(
        pack.installed(),
        vec![Feature::Text],
        "an add-on is not a feature"
    );
    assert_eq!(pack.installed_models().len(), 3);

    // Installing the size again keeps the add-on.
    assert!(pack.install(Feature::Text, Tier::Standard));
    pack.join();
    assert_eq!(pack.store().addons(Feature::Text), ["arabic"]);

    // Removing the add-on leaves the feature.
    assert!(pack.remove_addon(Feature::Text, "arabic"));
    assert_eq!(settle_addon(&pack, "arabic"), FeatureState::Available);
    assert!(pack.store().addons(Feature::Text).is_empty());
    assert_eq!(pack.installed(), vec![Feature::Text]);

    // Removing the feature takes its add-ons.
    assert!(pack.install_addon(Feature::Text, "arabic"));
    assert_eq!(settle_addon(&pack, "arabic"), FeatureState::Installed);
    assert!(pack.remove(Feature::Text));
    pack.join();
    assert!(pack.installed_models().is_empty());
    let left: Vec<_> = std::fs::read_dir(data.join("ai/models")).unwrap().collect();
    assert!(left.is_empty(), "{left:?}");
}
