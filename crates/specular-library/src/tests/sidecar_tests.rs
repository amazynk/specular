//! Other apps' sidecars (sidecar interop 2): read on the walk, filling in what the user has
//! not set, read again when they change, darktable duplicates as versions. The fixtures are
//! hand-written from the public formats (`specular-metadata/tests/fixtures/sidecars`).

use super::*;

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../../../specular-metadata/tests/fixtures/sidecars/",
            $name
        ))
    };
}

fn tags(lib: &Library, name: &str) -> Vec<String> {
    let mut t: Vec<String> = row(lib, name).tags.lines().map(str::to_owned).collect();
    t.sort();
    t
}

fn mark(lib: &Library, name: &str) -> (u8, u8, i8) {
    let m = row(lib, name).mark;
    (m.rating, m.label, m.flag)
}

#[test]
fn each_apps_sidecar_fills_the_library() {
    let d = Dir::new("foreign-apps");
    // darktable: the long form; Lightroom and digiKam: the stem form.
    d.put("s/dt.cr2", "");
    d.put("s/dt.cr2.xmp", fixture!("darktable.CR2.xmp"));
    d.put("s/lr.jpg", "kw=Embedded");
    d.put("s/lr.xmp", fixture!("lightroom.xmp"));
    d.put("s/dk.jpg", "");
    d.put("s/dk.xmp", fixture!("digikam.xmp"));
    d.put("s/rt.cr2", "");
    d.put("s/rt.cr2.pp3", fixture!("rawtherapee.CR2.pp3"));
    d.put("s/trash.cr2", "");
    d.put("s/trash.pp3", fixture!("rawtherapee-trash.pp3"));
    d.put("s/c1.cr2", "");
    d.put(
        "s/CaptureOne/Settings153/c1.cr2.cos",
        fixture!("captureone.CR2.cos"),
    );
    let lib = open(&d, false);
    lib.add_location(&d.0.join("s"));
    idle(&lib);
    // darktable: 3 stars, green (purple is none of ours), its own tags left out.
    assert_eq!(mark(&lib, "dt.cr2"), (3, 3, 0));
    assert_eq!(tags(&lib, "dt.cr2"), ["Places|France|Paris", "boats"]);
    assert!(names(&lib, "tag:darktable").is_empty());
    // Title and caption are searched.
    assert_eq!(names(&lib, "quay"), ["dt.cr2"]);
    assert_eq!(names(&lib, "boats"), ["dt.cr2"]);
    // Lightroom: 4 stars, yellow, picked; its keywords stand in for the embedded ones.
    assert_eq!(mark(&lib, "lr.jpg"), (4, 2, 1));
    assert_eq!(tags(&lib, "lr.jpg"), ["Places|Portugal|Lisbon", "market"]);
    assert!(names(&lib, "tag:embedded").is_empty());
    // digiKam: 2 stars, green, accepted, its tag hierarchy.
    assert_eq!(mark(&lib, "dk.jpg"), (2, 3, 1));
    assert_eq!(tags(&lib, "dk.jpg"), ["People|Family|Anna", "garden"]);
    // RawTherapee: rank and label, keywords; a profile in the trash is a reject.
    assert_eq!(mark(&lib, "rt.cr2"), (5, 4, 0));
    assert_eq!(tags(&lib, "rt.cr2"), ["Lisbon", "night", "tram"]);
    assert_eq!(mark(&lib, "trash.cr2"), (0, 0, -1));
    // Capture One: rating and label only.
    assert_eq!(mark(&lib, "c1.cr2"), (3, 4, 0));
    // Where they came from, read-only.
    let f = lib.foreign_sidecars(&d.0.join("s/dt.cr2")).unwrap();
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].name, "dt.cr2.xmp");
    assert_eq!(f[0].app, Some(specular_metadata::sidecars::App::Darktable));
    let f = lib.foreign_sidecars(&d.0.join("s/c1.cr2")).unwrap();
    assert_eq!(f[0].name, "CaptureOne/Settings153/c1.cr2.cos");
    // Nothing beside the images was written.
    assert_eq!(
        std::fs::read_to_string(d.0.join("s/dt.cr2.xmp")).unwrap(),
        fixture!("darktable.CR2.xmp")
    );
}

#[test]
fn the_users_values_win_and_the_sidecars_changes_follow() {
    let d = Dir::new("foreign-conflicts");
    d.put("s/a.cr2", "");
    let side = d.put("s/a.cr2.xmp", fixture!("darktable.CR2.xmp"));
    // Specular's own sidecar beside another app's: its marks are the user's.
    d.put("s/b.cr2", "");
    d.put("s/b.xmp", fixture!("specular.xmp"));
    d.put("s/b.cr2.xmp", fixture!("darktable.CR2.xmp"));
    let lib = open(&d, false);
    lib.add_location(&d.0.join("s"));
    idle(&lib);
    assert_eq!(mark(&lib, "a.cr2"), (3, 3, 0));
    // 1 star from Specular's sidecar; darktable's label fills the gap.
    assert_eq!(mark(&lib, "b.cr2"), (1, 3, 0));
    // The user rates a.cr2 (the gallery sends the whole mark: the label is unchanged).
    lib.set_marks(vec![(
        d.0.join("s/a.cr2"),
        Mark {
            rating: 1,
            label: 3,
            flag: 0,
        },
    )]);
    idle(&lib);
    assert_eq!(mark(&lib, "a.cr2"), (1, 3, 0));
    // darktable changes its sidecar: 5 stars, red, rejected.
    std::thread::sleep(Duration::from_millis(20));
    let changed = fixture!("darktable.CR2.xmp")
        .replace("xmp:Rating=\"3\"", "xmp:Rating=\"-1\"")
        .replace("<rdf:li>4</rdf:li>", "<rdf:li>0</rdf:li>");
    std::fs::write(&side, &changed).unwrap();
    lib.rescan(None);
    idle(&lib);
    // The user's stars stay; the label and flag followed the sidecar (red, rejected).
    assert_eq!(mark(&lib, "a.cr2"), (1, 1, -1));
    // "Prefer the other app's values" for the folder: what darktable says goes (it says
    // nothing of a.cr2's stars now).
    lib.set_prefer_foreign(vec![d.0.join("s")]);
    idle(&lib);
    assert_eq!(mark(&lib, "a.cr2"), (1, 1, -1));
    assert_eq!(mark(&lib, "b.cr2"), (3, 3, 0));
    // And back: the user's own sidecar wins again.
    lib.set_prefer_foreign(Vec::new());
    idle(&lib);
    assert_eq!(mark(&lib, "b.cr2"), (1, 3, 0));
    assert_eq!(std::fs::read_to_string(&side).unwrap(), changed);
}

#[test]
fn own_keywords_win_over_another_apps() {
    let d = Dir::new("foreign-keywords");
    d.put("k/a.cr2", "");
    d.put("k/a.cr2.xmp", fixture!("darktable.CR2.xmp"));
    // Specular's own sidecar names keywords: they are the user's.
    d.put(
        "k/a.xmp",
        &SIDECAR
            .replace("RATING", "2")
            .replace("<x:xmpmeta ", "<x:xmpmeta x:xmptk=\"Specular\" "),
    );
    let lib = open(&d, false);
    lib.add_location(&d.0.join("k"));
    idle(&lib);
    assert_eq!(tags(&lib, "a.cr2"), ["Beach", "Holiday"]);
    lib.set_prefer_foreign(vec![d.0.join("k")]);
    idle(&lib);
    assert_eq!(tags(&lib, "a.cr2"), ["Places|France|Paris", "boats"]);
    lib.set_prefer_foreign(Vec::new());
    idle(&lib);
    assert_eq!(tags(&lib, "a.cr2"), ["Beach", "Holiday"]);
}

#[test]
fn darktable_duplicates_are_versions() {
    let d = Dir::new("foreign-duplicates");
    d.put("v/IMG_1.cr2", "");
    d.put("v/IMG_1.cr2.xmp", fixture!("darktable.CR2.xmp"));
    d.put("v/IMG_1_01.cr2.xmp", fixture!("darktable_01.CR2.xmp"));
    d.put("v/IMG_1_02.cr2.xmp", fixture!("darktable_01.CR2.xmp"));
    // A photo whose name looks like a duplicate keeps its own sidecar.
    d.put("v/IMG_2.cr2", "");
    d.put("v/IMG_2_01.cr2", "");
    d.put("v/IMG_2_01.cr2.xmp", fixture!("darktable_01.CR2.xmp"));
    let lib = open(&d, false);
    lib.add_location(&d.0.join("v"));
    idle(&lib);
    // Duplicates are not photos of their own, and do not mark the photo.
    assert_eq!(names(&lib, ""), ["IMG_1.cr2", "IMG_2.cr2", "IMG_2_01.cr2"]);
    assert_eq!(mark(&lib, "IMG_1.cr2"), (3, 3, 0));
    assert_eq!(mark(&lib, "IMG_2_01.cr2"), (0, 4, -1));
    let photo = d.0.join("v/IMG_1.cr2");
    let counts = lib
        .version_counts(&[photo.clone(), d.0.join("v/IMG_2.cr2")])
        .unwrap();
    assert_eq!(counts, [(photo.clone(), 3)]);
    let f = lib.foreign_sidecars(&photo).unwrap();
    let dups: Vec<(u32, &str, Option<i8>, Option<&str>)> = f
        .iter()
        .map(|f| {
            (
                f.duplicate,
                f.name.as_str(),
                f.values.flag,
                f.version_name.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        dups,
        [
            (0, "IMG_1.cr2.xmp", None, None),
            (1, "IMG_1_01.cr2.xmp", Some(-1), Some("black and white")),
            (2, "IMG_1_02.cr2.xmp", Some(-1), Some("black and white")),
        ]
    );
    // A duplicate removed in darktable leaves.
    std::fs::remove_file(d.0.join("v/IMG_1_02.cr2.xmp")).unwrap();
    lib.rescan(None);
    idle(&lib);
    assert_eq!(
        lib.version_counts(std::slice::from_ref(&photo)).unwrap(),
        [(photo, 2)]
    );
}
