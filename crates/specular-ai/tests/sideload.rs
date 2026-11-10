//! Sideloading: model files copied in by hand (Install from file…, an admin's folder) are
//! checked against the manifest's SHA-256 and installed without any download. The sources
//! here are `file://` files deleted before the install, and an `https://` mirror this crate
//! cannot reach: an install that worked can only have used the sideloaded copies.

mod common;

use std::sync::Arc;

use common::{Dir, bytes, entry};
use specular_ai::install::parse_state;
use specular_ai::sideload::{self, Match};
use specular_ai::transport::Transports;
use specular_ai::{Feature, Manifest, Pack, Tier};

const MIRROR: &str = "https://models.invalid";

fn manifest(entries: &[String]) -> Manifest {
    Manifest::parse(&format!("schema = 1\n{}", entries.concat())).unwrap()
}

/// People's two models, published at the mirror; their `file://` copies are deleted.
fn people(d: &Dir) -> (Manifest, Vec<u8>, Vec<u8>) {
    let src = d.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    let a = bytes(200_000, 4);
    let b = bytes(50_000, 5);
    let m = manifest(&[
        entry(
            &src,
            "people",
            "detector",
            &a,
            &[format!("{MIRROR}/detector.onnx")],
        ),
        entry(
            &src,
            "people",
            "faces",
            &b,
            &[format!("{MIRROR}/faces.onnx")],
        ),
    ]);
    std::fs::remove_dir_all(&src).unwrap();
    (m, a, b)
}

fn pack(d: &Dir, m: Manifest, folders: Vec<std::path::PathBuf>) -> Pack {
    Pack::new(
        m,
        &d.path().join("data"),
        None,
        Transports::default(),
        Arc::new(|| {}),
    )
    .with_placeholders()
    .with_model_folders(folders)
}

#[test]
fn picked_files_install_without_the_network_and_unknown_ones_are_refused() {
    let d = Dir::new("sideload-pick");
    let (m, a, b) = people(&d);
    let usb = d.path().join("usb");
    std::fs::create_dir_all(usb.join("sub")).unwrap();
    // Any names: the hash says what a file is.
    std::fs::write(usb.join("one.onnx"), &a).unwrap();
    std::fs::write(usb.join("sub").join("two.bin"), &b).unwrap();
    std::fs::write(usb.join("cat.jpg"), b"not a model").unwrap();
    std::fs::write(usb.join(".hidden"), &a).unwrap();
    let p = pack(&d, m, Vec::new());
    assert!(p.needs_network(Feature::People, Tier::Standard));

    let s = p.sideload(std::slice::from_ref(&usb));
    assert_eq!(s.ready.len(), 2, "{s:?}");
    assert!(s.ready.iter().all(|(_, f)| *f == Feature::People));
    assert_eq!(s.unknown, vec![usb.join("cat.jpg")]);
    assert!(s.unverified.is_empty() && s.failed.is_empty());
    assert!(!p.needs_network(Feature::People, Tier::Standard));

    assert!(p.install(Feature::People, Tier::Standard));
    p.join();
    assert_eq!(p.installed(), vec![Feature::People]);
    let data = d.path().join("data").join("ai");
    let models = p.installed_models();
    for (id, body) in [("detector", &a), ("faces", &b)] {
        let im = models.iter().find(|i| i.id == id).unwrap();
        assert!(!im.unverified);
        assert_eq!(&std::fs::read(data.join(&im.file)).unwrap(), body);
    }
}

#[test]
fn an_unknown_hash_with_a_model_name_is_used_only_when_confirmed() {
    let d = Dir::new("sideload-unverified");
    let (m, _, _) = people(&d);
    let usb = d.path().join("usb");
    std::fs::create_dir_all(&usb).unwrap();
    // The models' published names, other bytes (a newer build, a converted file).
    let other = bytes(123_456, 9);
    std::fs::write(usb.join("detector.onnx"), &other).unwrap();
    std::fs::write(usb.join("faces.onnx"), bytes(10, 1)).unwrap();
    let c = sideload::identify(&m, &usb.join("detector.onnx")).unwrap();
    assert!(matches!(&c.found, Match::Unverified(x) if x.id == "detector"));
    assert_eq!(c.size, other.len() as u64);
    let p = pack(&d, m.clone(), Vec::new());
    let s = p.sideload(std::slice::from_ref(&usb));
    assert!(s.ready.is_empty());
    assert_eq!(s.unverified.len(), 2, "both names are known: {s:?}");
    // Nothing is copied until the user confirms.
    assert!(p.needs_network(Feature::People, Tier::Standard));

    // The user confirms both.
    for (path, model) in &s.unverified {
        p.sideload_unverified(model, path).unwrap();
    }
    assert!(!p.needs_network(Feature::People, Tier::Standard));
    assert!(p.install(Feature::People, Tier::Standard));
    p.join();
    assert_eq!(p.installed(), vec![Feature::People]);
    let det = p
        .installed_models()
        .into_iter()
        .find(|i| i.id == "detector")
        .unwrap();
    assert!(det.unverified);
    assert_eq!(det.size, other.len() as u64);
    let data = d.path().join("data").join("ai");
    assert_eq!(std::fs::read(data.join(&det.file)).unwrap(), other);
    // Kept as unverified across restarts, and still counted as installed.
    let text = std::fs::read_to_string(data.join("installed.toml")).unwrap();
    assert!(text.contains("unverified = true"), "{text}");
    let st = parse_state(&text);
    assert!(st.models.iter().any(|i| i.id == "detector" && i.unverified));
    let again = pack(&d, m, Vec::new());
    assert_eq!(again.installed(), vec![Feature::People]);
    assert!(!again.needs_network(Feature::People, Tier::Standard));
}

#[test]
fn an_admin_folder_is_used_before_any_download() {
    let d = Dir::new("sideload-admin");
    let (m, a, b) = people(&d);
    let admin = d.path().join("admin");
    std::fs::create_dir_all(&admin).unwrap();
    std::fs::write(admin.join("x"), &a).unwrap();
    // Only one of the two: the feature still needs the network.
    let p = pack(&d, m.clone(), vec![admin.clone()]);
    assert!(p.needs_network(Feature::People, Tier::Standard));
    assert!(p.install(Feature::People, Tier::Standard));
    p.join();
    assert!(p.installed().is_empty(), "faces has no source");
    std::fs::write(admin.join("y"), &b).unwrap();
    let p = pack(&d, m, vec![admin.clone(), d.path().join("absent")]);
    assert!(!p.needs_network(Feature::People, Tier::Standard));
    assert!(p.install(Feature::People, Tier::Standard));
    p.join();
    assert_eq!(p.installed(), vec![Feature::People]);
    // The admin's files stay where they are.
    assert!(admin.join("x").is_file() && admin.join("y").is_file());
}

#[test]
fn a_file_changed_between_look_and_copy_is_refused() {
    let d = Dir::new("sideload-changed");
    let (m, a, _) = people(&d);
    let f = d.path().join("detector.onnx");
    std::fs::write(&f, &a).unwrap();
    let Match::Verified(model) = sideload::identify(&m, &f).unwrap().found else {
        panic!("not recognised");
    };
    let mut changed = a.clone();
    changed[0] ^= 1;
    std::fs::write(&f, &changed).unwrap();
    let store = specular_ai::install::Store::at(d.path().join("store"));
    assert!(matches!(
        store.sideload(&model, &f),
        Err(specular_ai::install::InstallError::Hash { .. })
    ));
    assert!(!store.part_path(&model).exists());
    assert_eq!(sideload::file_name(&model), Some("detector.onnx"));
}
