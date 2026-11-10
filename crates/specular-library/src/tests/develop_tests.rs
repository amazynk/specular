//! Develop edits in search: `edited:`, `module:`, `style:`, `version:` and their facets.

use super::*;
use crate::edits::StoredEdit;

fn stored(modules: Option<&str>, styles: Option<&str>) -> StoredEdit {
    StoredEdit {
        edit: vec![1; 8],
        hash: vec![2; 32],
        steps: Vec::new(),
        position: 0,
        updated: 1_730_000_000,
        name: String::new(),
        modules: modules.map(str::to_owned),
        styles: styles.map(str::to_owned),
    }
}

#[test]
fn develop_terms_search_the_edits() {
    let d = Dir::new("develop-terms");
    for i in 0..12 {
        d.put(&format!("p/img_{i:02}.jpg"), "camera=X\niso=100");
    }
    let lib = open(&d, false);
    lib.add_location(&d.0.join("p"));
    idle(&lib);
    let p = |i: u32| d.0.join(format!("p/img_{i:02}.jpg"));
    lib.save_edit(p(0), 0, stored(Some("exposure\ndenoise"), Some("Punchy")));
    lib.save_edit(p(1), 0, stored(Some("exposure"), Some("")));
    lib.save_edit(p(1), 1, stored(Some("black_white"), Some("Mono")));
    lib.save_edit(p(1), 2, stored(Some("exposure"), None));
    // Saved by an older build: modules not known.
    lib.save_edit(p(2), 0, stored(None, None));
    // A file outside the catalogue is skipped.
    lib.save_edit(d.0.join("elsewhere.jpg"), 0, stored(Some("x"), None));
    idle(&lib);
    let cases: [(&str, &[&str]); 12] = [
        ("edited:yes", &["img_00.jpg", "img_01.jpg", "img_02.jpg"]),
        ("-edited:no", &["img_00.jpg", "img_01.jpg", "img_02.jpg"]),
        (
            "edited:no img_0",
            &[
                "img_03.jpg",
                "img_04.jpg",
                "img_05.jpg",
                "img_06.jpg",
                "img_07.jpg",
                "img_08.jpg",
                "img_09.jpg",
            ],
        ),
        // Not known passes (as unknown facts do).
        (
            "module:exposure",
            &["img_00.jpg", "img_01.jpg", "img_02.jpg"],
        ),
        ("module=denoise", &["img_00.jpg", "img_02.jpg"]),
        // Versions' own modules are theirs, not the photo's.
        ("module:black_white", &["img_02.jpg"]),
        ("style:punchy", &["img_00.jpg", "img_02.jpg"]),
        ("-style:punchy edited:yes", &["img_01.jpg", "img_02.jpg"]),
        ("version:yes", &["img_01.jpg"]),
        ("version:no edited:yes", &["img_00.jpg", "img_02.jpg"]),
        ("-version:yes edited:yes", &["img_00.jpg", "img_02.jpg"]),
        ("version:copy", &[]),
    ];
    for (q, want) in cases {
        for no_pushdown in [false, true] {
            let mut got: Vec<String> = lib
                .search(&Search {
                    query: q.into(),
                    no_pushdown,
                    ..Search::default()
                })
                .unwrap()
                .rows
                .into_iter()
                .map(|r| r.name)
                .collect();
            got.sort();
            assert_eq!(got, want, "{q} (pushdown off: {no_pushdown})");
        }
    }
    // Pushdown narrows: only edited rows leave SQLite.
    let r = lib
        .search(&Search {
            query: "edited:yes".into(),
            ..Search::default()
        })
        .unwrap();
    assert_eq!(r.candidates, 3);
    // Facets, with and without pushdown.
    let req = Search {
        query: "edited:yes".into(),
        facets: true,
        ..Search::default()
    };
    let a = lib.search(&req).unwrap();
    let b = lib
        .search(&Search {
            no_pushdown: true,
            ..req.clone()
        })
        .unwrap();
    assert_eq!(a.facets, b.facets);
    use specular_query::facet::FacetField as F;
    let get = |f: F| {
        a.facets
            .iter()
            .find(|x| x.field == f)
            .map(|x| {
                x.values
                    .iter()
                    .map(|v| (v.key.clone(), v.count))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    assert_eq!(get(F::Edited), [("yes".into(), 3), ("no".into(), 9)]);
    assert_eq!(
        get(F::Module),
        [("exposure".into(), 2), ("denoise".into(), 1)]
    );
    assert_eq!(get(F::Style), [("Punchy".into(), 1)]);
    assert_eq!(get(F::Version), [("yes".into(), 1), ("no".into(), 2)]);
    // Rows carry what the gallery badges.
    let r = row(&lib, "img_01.jpg");
    let dv = r.develop.expect("edits");
    assert!(dv.edited);
    assert_eq!(dv.versions, 3);
    assert!(row(&lib, "img_05.jpg").develop.is_none());
}

/// A walk takes the edits of Develop sidecars the catalogue has
/// none for, so `edited:` in the Library agrees with the folder view (which reads the
/// sidecars); a catalogued edit is never replaced by its sidecar.
#[test]
fn walks_take_sidecar_edits_the_catalogue_lacks() {
    let d = Dir::new("develop-sidecars");
    for i in 0..4 {
        d.put(&format!("p/img_{i}.jpg"), "camera=X");
    }
    // Edited before the folder was in the Library: the first version and a second one.
    d.put("p/img_0.jpg.specular.xmp", "exposure");
    d.put("p/img_1.jpg.v2.specular.xmp", "black_white");
    // A sidecar without its photo, and another tool's sidecar.
    d.put("p/gone.jpg.specular.xmp", "exposure");
    d.put("p/img_2.jpg.xmp", "rating");
    let mut cfg = config(d.0.join("db/library.sqlite3"), false);
    let reads = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = reads.clone();
    cfg.edit_import = Some(crate::EditImport {
        suffix: ".specular.xmp",
        read: Arc::new(move |p: &Path, v: u32| {
            seen.lock().unwrap().push((p.to_path_buf(), v));
            let side = if v == 0 {
                format!("{}.specular.xmp", p.display())
            } else {
                format!("{}.v{v}.specular.xmp", p.display())
            };
            let modules = std::fs::read_to_string(side).ok()?;
            Some(stored(Some(&modules), Some("")))
        }),
    });
    let lib = Library::open(cfg).unwrap();
    lib.add_location(&d.0.join("p"));
    idle(&lib);
    assert_eq!(names(&lib, "edited:yes"), ["img_0.jpg"]);
    assert_eq!(names(&lib, "module:exposure"), ["img_0.jpg"]);
    assert_eq!(
        lib.edited_in(&d.0.join("p")).unwrap(),
        [d.0.join("p/img_0.jpg"), d.0.join("p/img_1.jpg")]
    );
    // A catalogued edit stays as it is; a sidecar new since the last walk comes in.
    lib.save_edit(
        d.0.join("p/img_0.jpg"),
        0,
        stored(Some("denoise"), Some("")),
    );
    d.put("p/img_0.jpg.specular.xmp", "tone_curve");
    d.put("p/img_3.jpg.specular.xmp", "denoise");
    idle(&lib);
    lib.rescan(None);
    idle(&lib);
    assert_eq!(names(&lib, "module:denoise"), ["img_0.jpg", "img_3.jpg"]);
    assert!(names(&lib, "module:tone_curve").is_empty());
    assert_eq!(names(&lib, "edited:yes"), ["img_0.jpg", "img_3.jpg"]);
    // Each sidecar was read once: catalogued ones are not read again.
    let r = reads.lock().unwrap();
    assert_eq!(r.len(), 3, "{r:?}");
}
