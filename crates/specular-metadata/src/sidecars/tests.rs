use super::*;

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/sidecars/",
            $name
        ))
        .as_slice()
    };
}

fn kw(v: &[&str]) -> Option<Vec<String>> {
    Some(v.iter().map(|s| s.to_string()).collect())
}

#[test]
fn darktable_rating_labels_and_tags_without_its_own() {
    let r = read(Kind::Xmp, fixture!("darktable.CR2.xmp"));
    assert_eq!(r.app, Some(App::Darktable));
    let v = r.values;
    assert_eq!(v.rating, Some(3));
    // Purple (4) has no label of ours; green (2) is the first we have.
    assert_eq!(v.label, Some(3));
    assert_eq!(v.flag, None);
    assert_eq!(v.keywords, kw(&["Places|France|Paris", "boats"]));
    assert_eq!(v.title.as_deref(), Some("Quay at dusk"));
    assert_eq!(v.caption.as_deref(), Some("Boats coming in"));
}

#[test]
fn darktable_reject_and_duplicate_name() {
    let r = read(Kind::Xmp, fixture!("darktable_01.CR2.xmp"));
    assert_eq!(r.app, Some(App::Darktable));
    assert_eq!(r.values.flag, Some(-1));
    assert_eq!(r.values.rating, None);
    assert_eq!(r.values.label, Some(4));
    assert_eq!(r.version_name.as_deref(), Some("black and white"));
    assert_eq!(
        duplicate_of("IMG_0001_01.CR2.xmp"),
        Some(("IMG_0001.CR2".into(), 1))
    );
    assert_eq!(
        duplicate_of("IMG_0001_123.nef.XMP"),
        Some(("IMG_0001.nef".into(), 123))
    );
    assert_eq!(duplicate_name("IMG_0001.CR2", 1), "IMG_0001_01.CR2.xmp");
    assert_eq!(duplicate_name("IMG_0001.CR2", 100), "IMG_0001_100.CR2.xmp");
    // By name alone `IMG_0001.CR2.xmp` could be duplicate 1 of `IMG.CR2`: the callers check
    // that `IMG_0001.CR2` is no image of its own.
    assert_eq!(
        duplicate_of("IMG_0001.CR2.xmp"),
        Some(("IMG.CR2".into(), 1))
    );
    for not in [
        "IMG_0001.xmp",
        "IMG_1.CR2.xmp",
        "IMG_00.CR2.xmp",
        "IMG_0001_01.CR2.specular.xmp",
        "IMG_0001_01.CR2.v2.specular.xmp",
        "_01.CR2.xmp",
        "IMG_0001_01.CR2.pp3",
    ] {
        assert_eq!(duplicate_of(not), None, "{not}");
    }
}

#[test]
fn lightroom_values() {
    let r = read(Kind::Xmp, fixture!("lightroom.xmp"));
    assert_eq!(r.app, Some(App::Adobe));
    let v = r.values;
    assert_eq!((v.rating, v.label, v.flag), (Some(4), Some(2), Some(1)));
    // dc:subject's "Portugal" and "Lisbon" are levels of the hierarchy.
    assert_eq!(v.keywords, kw(&["Places|Portugal|Lisbon", "market"]));
    assert_eq!(v.title.as_deref(), Some("Market morning"));
    assert_eq!(v.caption, None);
}

#[test]
fn digikam_values() {
    let r = read(Kind::Xmp, fixture!("digikam.xmp"));
    assert_eq!(r.app, Some(App::Digikam));
    let v = r.values;
    assert_eq!((v.rating, v.label, v.flag), (Some(2), Some(3), Some(1)));
    assert_eq!(v.keywords, kw(&["People|Family|Anna", "garden"]));
}

#[test]
fn rawtherapee_profiles() {
    let r = read(Kind::Pp3, fixture!("rawtherapee.CR2.pp3"));
    assert_eq!(r.app, Some(App::RawTherapee));
    let v = r.values;
    assert_eq!((v.rating, v.label, v.flag), (Some(5), Some(4), None));
    assert_eq!(v.keywords, kw(&["tram", "Lisbon", "night"]));
    assert_eq!(v.caption.as_deref(), Some("Night tram"));
    assert_eq!(v.title.as_deref(), Some("Line 28"));
    let t = read(Kind::Pp3, fixture!("rawtherapee-trash.pp3")).values;
    // Purple (5) is no label of ours; in the trash is rejected.
    assert_eq!((t.rating, t.label, t.flag), (Some(0), None, Some(-1)));
}

#[test]
fn capture_one_rating_and_label_only() {
    let r = read(Kind::Cos, fixture!("captureone.CR2.cos"));
    assert_eq!(r.app, Some(App::CaptureOne));
    assert_eq!(
        r.values,
        Values {
            rating: Some(3),
            label: Some(4),
            ..Values::default()
        }
    );
    // Not XML at all: nothing.
    assert!(read_cos(b"\x00\x01binary").is_empty());
}

#[test]
fn owners() {
    let s = |b: &[u8]| owner(&String::from_utf8_lossy(b));
    assert_eq!(s(fixture!("specular.xmp")), None);
    assert_eq!(s(fixture!("exiftool.xmp")), Some(App::Other));
    assert_eq!(s(fixture!("lightroom.xmp")), Some(App::Adobe));
    assert_eq!(s(fixture!("darktable.CR2.xmp")), Some(App::Darktable));
    assert_eq!(s(fixture!("digikam.xmp")), Some(App::Digikam));
    // A hand-written packet without a toolkit name: Specular may write it.
    assert_eq!(owner("<x:xmpmeta><rdf:RDF/></x:xmpmeta>"), None);
    // Capture One's XMP names itself in xmp:CreatorTool.
    let c1 = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core 5.6">
      <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
      <rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/"
        xmp:CreatorTool="Capture One 23 Macintosh"/></rdf:RDF></x:xmpmeta>"#;
    assert_eq!(owner(c1), Some(App::CaptureOne));
    for a in [
        App::Darktable,
        App::Adobe,
        App::CaptureOne,
        App::Digikam,
        App::RawTherapee,
        App::PhotoMechanic,
        App::Other,
    ] {
        assert_eq!(App::from_id(a.id()), Some(a));
    }
}

#[test]
fn darktable_internal_tags() {
    assert!(is_darktable_internal("darktable|format|cr2"));
    assert!(is_darktable_internal("Darktable|changed"));
    assert!(is_darktable_internal("darktable"));
    assert!(!is_darktable_internal("darktable fans"));
    assert!(!is_darktable_internal("Places|darktable"));
}

#[test]
fn candidate_names_in_order() {
    let have: std::collections::HashSet<&str> = [
        "IMG.xmp",
        "IMG.CR2.xmp",
        "IMG.CR2.pp3",
        "IMG.pp3",
        "CaptureOne/Settings153/IMG.CR2.cos",
    ]
    .into();
    let has = |n: &str| have.contains(n);
    let dirs = vec!["CaptureOne/Settings153".to_owned()];
    let names: Vec<String> = candidates("IMG.CR2", true, &dirs, &has)
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(
        names,
        [
            "IMG.xmp",
            "IMG.CR2.xmp",
            "IMG.CR2.pp3",
            "IMG.pp3",
            "CaptureOne/Settings153/IMG.CR2.cos"
        ]
    );
    // In a RAW+JPEG pair the JPEG has no claim on the stem forms.
    let names: Vec<String> = candidates("IMG.CR2", false, &dirs, &has)
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(
        names,
        [
            "IMG.CR2.xmp",
            "IMG.CR2.pp3",
            "CaptureOne/Settings153/IMG.CR2.cos"
        ]
    );
}

#[test]
fn values_fill_only_the_gaps() {
    let mut a = Values {
        rating: Some(2),
        ..Values::default()
    };
    let b = Values {
        rating: Some(5),
        label: Some(1),
        keywords: kw(&["x"]),
        ..Values::default()
    };
    a.fill_from(&b);
    assert_eq!(a.rating, Some(2));
    assert_eq!(a.label, Some(1));
    assert_eq!(a.keywords, kw(&["x"]));
}

#[test]
fn found_beside_a_file_and_duplicates() {
    let dir = scratch("found");
    let w = |n: &str, b: &[u8]| std::fs::write(dir.join(n), b).unwrap();
    w("IMG.CR2", b"raw");
    w("IMG.CR2.xmp", fixture!("darktable.CR2.xmp"));
    w("IMG_01.CR2.xmp", fixture!("darktable_01.CR2.xmp"));
    w("IMG_02.CR2.xmp", fixture!("darktable_01.CR2.xmp"));
    // After a gap of one, still found.
    w("IMG_04.CR2.xmp", fixture!("darktable_01.CR2.xmp"));
    w("IMG.CR2.pp3", fixture!("rawtherapee.CR2.pp3"));
    // Specular's own is not another app's.
    w("IMG.xmp", fixture!("specular.xmp"));
    std::fs::create_dir_all(dir.join("CaptureOne/Settings153")).unwrap();
    w(
        "CaptureOne/Settings153/IMG.CR2.cos",
        fixture!("captureone.CR2.cos"),
    );
    // An image of its own with a duplicate-like name keeps its sidecar.
    w("OTHER.CR2", b"raw");
    w("OTHER_01.CR2", b"raw");
    w("OTHER_01.CR2.xmp", fixture!("darktable_01.CR2.xmp"));
    let f = found_for(&dir.join("IMG.CR2"), true);
    let got: Vec<(&str, u32, Option<App>)> = f
        .iter()
        .map(|f| (f.candidate.name.as_str(), f.candidate.duplicate, f.read.app))
        .collect();
    assert_eq!(
        got,
        [
            ("IMG.CR2.xmp", 0, Some(App::Darktable)),
            ("IMG.CR2.pp3", 0, Some(App::RawTherapee)),
            (
                "CaptureOne/Settings153/IMG.CR2.cos",
                0,
                Some(App::CaptureOne)
            ),
            ("IMG_01.CR2.xmp", 1, Some(App::Darktable)),
            ("IMG_02.CR2.xmp", 2, Some(App::Darktable)),
            ("IMG_04.CR2.xmp", 4, Some(App::Darktable)),
        ]
    );
    assert!(found_for(&dir.join("OTHER.CR2"), true).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

fn scratch(tag: &str) -> PathBuf {
    // In the build's target folder (beside the test binary), never the system's temporary one.
    let exe = std::env::current_exe().unwrap();
    let base = exe
        .parent()
        .and_then(|d| d.parent())
        .unwrap_or(Path::new("."));
    let d = base
        .join("scratch")
        .join(format!("sidecars-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
