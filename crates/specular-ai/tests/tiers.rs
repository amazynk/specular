//! Model sizes: install one, switch to another keeping the old one until the new index is
//! complete, roll back on failure, adopt a pre-size install, and a transport that writes the
//! download itself (how the app's HTTPS client plugs in). `file://` sources only.

mod common;

use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use common::{Dir, bytes, catalogue, entry, sha_hex};
use specular_ai::install::{InstallError, Store, Transfer};
use specular_ai::transport::{Body, Transport, Transports};
use specular_ai::{Feature, FeatureState, Manifest, Pack, Tier};

/// Search in three sizes sharing a tokenizer, as the built-in manifest has it.
struct Sized {
    manifest: String,
    merges: Vec<u8>,
    small: Vec<u8>,
    standard: Vec<u8>,
    large: Vec<u8>,
}

fn sized(dir: &Path) -> Sized {
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let merges = bytes(5_000, 11);
    let small = bytes(30_000, 12);
    let standard = bytes(60_000, 13);
    let large = bytes(90_000, 14);
    let e = |id: &str, tier: &str, body: &[u8]| {
        entry(&src, "search", id, body, &[]).replacen(
            "[[model]]\n",
            &format!("[[model]]\ntier = {tier}\n"),
            1,
        )
    };
    let manifest = format!(
        "schema = 2\n{}{}{}{}",
        e("merges", r#"["small", "standard", "large"]"#, &merges),
        e("clip-s", r#""small""#, &small),
        e("clip-b", r#""standard""#, &standard),
        e("clip-l", r#""large""#, &large),
    );
    Sized {
        manifest,
        merges,
        small,
        standard,
        large,
    }
}

fn pack(d: &Dir, m: &Sized) -> (Pack, std::path::PathBuf) {
    let db = d.path().join("library.sqlite3");
    if !db.exists() {
        catalogue(&db, 4, |_| true);
    }
    let pack = Pack::new(
        Manifest::parse(&m.manifest).unwrap(),
        d.path(),
        Some(db.clone()),
        Transports::default(),
        Arc::new(|| {}),
    )
    .with_placeholders();
    (pack, db)
}

fn search(pack: &Pack) -> specular_ai::FeatureStatus {
    pack.status()
        .into_iter()
        .find(|s| s.feature == Feature::Search)
        .unwrap()
}

fn model_dirs(d: &Dir) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(d.path().join("ai/models"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn switching_sizes_keeps_the_old_one_until_the_new_index_is_complete() {
    let d = Dir::new("switch");
    let m = sized(d.path());
    let (pack, db) = pack(&d, &m);
    let st = search(&pack);
    assert_eq!(st.state, FeatureState::Available);
    assert_eq!(st.offer.as_ref().unwrap().tier_ids(), Tier::ALL.to_vec());
    assert_eq!(
        pack.download_size(Feature::Search, Tier::Small),
        Some((m.merges.len() + m.small.len()) as u64)
    );

    assert!(pack.install_for(Feature::Search, Tier::Small, Some("gpu A".into())));
    pack.join();
    let st = search(&pack);
    assert_eq!(st.state, FeatureState::Installed);
    assert_eq!((st.tier, st.previous), (Some(Tier::Small), None));
    assert_eq!(st.basis.as_deref(), Some("gpu A"));
    let (small, prev) = pack.index_models(Feature::Search);
    let small = small.unwrap();
    assert!(prev.is_none());
    assert_eq!(small.key, "small/clip-s-1+merges-1");
    assert_eq!(model_dirs(&d), ["clip-s-1", "merges-1"]);

    // Embeddings under the small size, and some from before sizes.
    let conn = specular_library::ai::open(&db).unwrap();
    for id in 1..=4 {
        specular_library::ai::put_embedding(&conn, id, &small.key, &[0x3c00, 0]).unwrap();
    }
    specular_library::ai::put_embedding(&conn, 1, "clip-old-1", &[0x3c00, 0]).unwrap();

    // Only the new size's own file is downloaded: the tokenizer is shared.
    assert_eq!(
        pack.download_size(Feature::Search, Tier::Standard),
        Some(m.standard.len() as u64)
    );
    assert!(pack.install(Feature::Search, Tier::Standard));
    pack.join();
    let st = search(&pack);
    assert_eq!(st.state, FeatureState::Installed);
    assert_eq!(st.tier, Some(Tier::Standard));
    assert_eq!(st.previous, Some(Tier::Small), "kept until re-indexed");
    assert_eq!(st.basis, None);
    let (cur, prev) = pack.index_models(Feature::Search);
    let (cur, prev) = (cur.unwrap(), prev.unwrap());
    assert_eq!(cur.key, "standard/clip-b-1+merges-1");
    assert_eq!(prev.key, small.key);
    assert_eq!(model_dirs(&d), ["clip-b-1", "clip-s-1", "merges-1"]);
    let file = |name: &str| d.path().join("ai").join(&cur.model(name).unwrap().file);
    assert_eq!(std::fs::read(file("clip-b")).unwrap(), m.standard);
    assert_eq!(std::fs::read(file("merges")).unwrap(), m.merges);
    // The old size's embeddings are still there to search with.
    assert_eq!(
        specular_library::ai::embedding_count(&conn, &small.key).unwrap(),
        4
    );
    // Going back to the previous size downloads nothing.
    assert_eq!(pack.download_size(Feature::Search, Tier::Small), Some(0));

    // Re-indexed: the old size, its embeddings and the pre-size ones go.
    for id in 1..=4 {
        specular_library::ai::put_embedding(&conn, id, &cur.key, &[0, 0x3c00]).unwrap();
    }
    assert_eq!(pack.counts(Feature::Search), Some((4, 4)));
    pack.finish_index(Feature::Search, cur.key.clone());
    pack.join();
    for (what, r) in pack.take_finished() {
        assert!(r.is_ok(), "{what}: {r:?}");
    }
    let st = search(&pack);
    assert_eq!((st.tier, st.previous), (Some(Tier::Standard), None));
    assert_eq!(model_dirs(&d), ["clip-b-1", "merges-1"]);
    assert_eq!(
        specular_library::ai::embedding_count(&conn, &small.key).unwrap(),
        0
    );
    assert_eq!(
        specular_library::ai::embedding_count(&conn, "clip-old-1").unwrap(),
        0
    );
    assert_eq!(
        specular_library::ai::embedding_count(&conn, &cur.key).unwrap(),
        4
    );
    assert_eq!(pack.index_models(Feature::Search).1, None);

    // The state survives a restart (a new pack over the same files).
    drop(pack);
    let (pack, _) = self::pack(&d, &m);
    let st = search(&pack);
    assert_eq!((st.tier, st.previous), (Some(Tier::Standard), None));
}

#[test]
fn a_second_switch_before_the_first_finished_drops_the_oldest_size() {
    let d = Dir::new("switch2");
    let m = sized(d.path());
    let (pack, _) = pack(&d, &m);
    pack.install(Feature::Search, Tier::Small);
    pack.join();
    pack.install(Feature::Search, Tier::Standard);
    pack.join();
    pack.install(Feature::Search, Tier::Large);
    pack.join();
    let st = search(&pack);
    assert_eq!(
        (st.tier, st.previous),
        (Some(Tier::Large), Some(Tier::Standard))
    );
    assert_eq!(model_dirs(&d), ["clip-b-1", "clip-l-1", "merges-1"]);
    // Back to the size still installed: nothing to download, nothing lost.
    assert_eq!(pack.download_size(Feature::Search, Tier::Standard), Some(0));
    pack.install(Feature::Search, Tier::Standard);
    pack.join();
    let st = search(&pack);
    assert_eq!(
        (st.tier, st.previous),
        (Some(Tier::Standard), Some(Tier::Large))
    );
    assert_eq!(model_dirs(&d), ["clip-b-1", "clip-l-1", "merges-1"]);
    // The same size again: a reinstall, no switch.
    pack.install(Feature::Search, Tier::Standard);
    pack.join();
    let st = search(&pack);
    assert_eq!(
        (st.tier, st.previous),
        (Some(Tier::Standard), Some(Tier::Large))
    );
    // Removing the feature takes every size.
    pack.remove(Feature::Search);
    pack.join();
    let st = search(&pack);
    assert_eq!((st.tier, st.previous), (None, None));
    assert_eq!(st.state, FeatureState::Available);
    assert!(model_dirs(&d).is_empty());
}

#[test]
fn a_failed_switch_leaves_the_installed_size_in_use() {
    let d = Dir::new("rollback");
    let m = sized(d.path());
    let (pack, _) = pack(&d, &m);
    pack.install(Feature::Search, Tier::Small);
    pack.join();
    let before = pack.index_models(Feature::Search).0.unwrap();
    // The large file on the mirror is not the one the manifest names.
    std::fs::write(d.path().join("src/clip-l.bin"), bytes(90_000, 99)).unwrap();
    assert!(pack.install(Feature::Search, Tier::Large));
    pack.join();
    let st = search(&pack);
    assert!(
        matches!(&st.state, FeatureState::Failed(e) if e.contains("SHA-256")),
        "{:?}",
        st.state
    );
    assert_eq!(st.tier, Some(Tier::Small), "still installed");
    assert_eq!(st.target, Some(Tier::Large));
    assert_eq!(st.previous, None);
    assert_eq!(pack.index_models(Feature::Search), (Some(before), None));
    assert_eq!(model_dirs(&d), ["clip-s-1", "merges-1"]);
    pack.dismiss(Feature::Search);
    let st = search(&pack);
    assert_eq!(st.state, FeatureState::Installed);
    assert_eq!(st.target, None);

    // A cancelled switch changes nothing either, and keeps what came to resume.
    std::fs::write(d.path().join("src/clip-l.bin"), &m.large).unwrap();
    let offer = Manifest::parse(&m.manifest)
        .unwrap()
        .offer(Feature::Search, true)
        .unwrap()
        .with_tier(Tier::Large)
        .unwrap();
    let t = Transfer::default();
    t.cancel();
    let e = pack
        .store()
        .install(&offer, &Transports::default(), &t, None)
        .unwrap_err();
    assert!(matches!(e, InstallError::Cancelled));
    assert_eq!(pack.store().tier(Feature::Search), Some(Tier::Small));
}

#[test]
fn an_install_from_before_sizes_is_adopted() {
    let d = Dir::new("adopt");
    let m = sized(d.path());
    let ai = d.path().join("ai");
    for (key, body) in [("merges-1", &m.merges), ("clip-b-1", &m.standard)] {
        std::fs::create_dir_all(ai.join("models").join(key)).unwrap();
        std::fs::write(ai.join("models").join(key).join("model.bin"), body).unwrap();
    }
    let model = |id: &str, body: &[u8]| {
        format!(
            "\n[[model]]\nfeature = \"search\"\nid = {id:?}\nversion = \"1\"\nsha256 = {:?}\nsize = {}\nfile = \"models/{id}-1/model.bin\"\n",
            sha_hex(body),
            body.len()
        )
    };
    std::fs::write(
        ai.join("installed.toml"),
        format!(
            "schema = 1\n{}{}",
            model("merges", &m.merges),
            model("clip-b", &m.standard)
        ),
    )
    .unwrap();
    let store = Store::at(ai.clone());
    assert_eq!(store.tier(Feature::Search), None);
    assert_eq!(store.installed().len(), 2, "schema 1 still reads");

    let (pack, _) = pack(&d, &m);
    let st = search(&pack);
    assert_eq!(st.state, FeatureState::Installed);
    assert_eq!(st.tier, Some(Tier::Standard));
    let text = std::fs::read_to_string(ai.join("installed.toml")).unwrap();
    assert!(text.contains("schema = 2"), "{text}");
    assert_eq!(model_dirs(&d), ["clip-b-1", "merges-1"], "tidy kept them");
}

/// A client that writes the download to the file itself (the shape of the app's HTTPS
/// client), copying from `file://` sources; `open` is refused, so only `download_to` works.
struct Push {
    /// Write these bytes instead of the source's (a corrupt download).
    wrong: Option<Vec<u8>>,
    calls: parking_lot::Mutex<Vec<u64>>,
}

impl Transport for Push {
    fn serves(&self, url: &str) -> bool {
        url.starts_with("file://")
    }

    fn open(&self, _url: &str, _offset: u64) -> io::Result<Body> {
        Err(io::Error::other("open is not used"))
    }

    fn download_to(
        &self,
        url: &str,
        part: &Path,
        offset: u64,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Option<io::Result<()>> {
        self.calls.lock().push(offset);
        let r = (|| {
            let src = std::fs::read(url.strip_prefix("file://").unwrap())?;
            let body = self.wrong.clone().unwrap_or(src);
            let mut have = std::fs::read(part).unwrap_or_default();
            have.truncate(offset as usize);
            for chunk in body[offset as usize..].chunks(4096) {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                have.extend_from_slice(chunk);
                progress(have.len() as u64);
            }
            std::fs::write(part, &have)
        })();
        Some(r)
    }
}

#[test]
fn a_transport_that_writes_the_file_itself() {
    let d = Dir::new("push");
    let m = sized(d.path());
    let offer = Manifest::parse(&m.manifest)
        .unwrap()
        .offer(Feature::Search, true)
        .unwrap()
        .with_tier(Tier::Standard)
        .unwrap();
    let store = Store::at(d.path().join("ai"));
    let push = Push {
        wrong: None,
        calls: Default::default(),
    };
    // Half of the weights from an earlier run: resumed from there.
    let part = store.part_path(&offer.models[1]);
    std::fs::create_dir_all(part.parent().unwrap()).unwrap();
    std::fs::write(&part, &m.standard[..20_000]).unwrap();
    let t = Transfer::default();
    store.install(&offer, &push, &t, None).unwrap();
    assert_eq!(*push.calls.lock(), vec![0, 20_000]);
    let total = (m.merges.len() + m.standard.len()) as u64;
    assert_eq!(t.get(), (total, total));
    assert_eq!(store.tier(Feature::Search), Some(Tier::Standard));

    // A wrong file is caught by the hash and deleted.
    let d2 = Dir::new("push-bad");
    let m2 = sized(d2.path());
    let offer = Manifest::parse(&m2.manifest)
        .unwrap()
        .offer(Feature::Search, true)
        .unwrap();
    let store = Store::at(d2.path().join("ai"));
    let bad = Push {
        wrong: Some(bytes(m2.merges.len(), 1)),
        calls: Default::default(),
    };
    let e = store
        .install(&offer, &bad, &Transfer::default(), None)
        .unwrap_err();
    assert!(matches!(e, InstallError::Hash { .. }), "{e}");
    assert!(!store.part_path(&offer.models[0]).exists());
    assert!(store.installed().is_empty());
    // Through the app's transport list too.
    let ts = Transports::default().with(Box::new(Push {
        wrong: None,
        calls: Default::default(),
    }));
    store
        .install(&offer, &ts, &Transfer::default(), None)
        .unwrap();
    assert_eq!(store.tier(Feature::Search), Some(Tier::Standard));
}
