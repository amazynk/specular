//! The Develop facets: `edited:`, `module:`, `style:` and `version:`.

use super::*;
use crate::facet::{FacetField, Tally, is_active, toggle};

fn m(q: &str) -> Matcher {
    let q = parse(q);
    assert!(q.errors.is_empty(), "{:?}", q.errors);
    Matcher::new(&q, ascii_fold)
}

fn edited() -> Facts<'static> {
    Facts {
        name: "IMG_0001.CR2",
        ext: "CR2",
        known: true,
        edited: Some(true),
        modules: Some("exposure\ncolour_balance\ndenoise"),
        styles: Some("Punchy\nWarm evening"),
        versions: 3,
        ..Facts::default()
    }
}

fn plain() -> Facts<'static> {
    Facts {
        name: "IMG_0002.CR2",
        ext: "CR2",
        known: true,
        edited: Some(false),
        ..Facts::default()
    }
}

#[test]
fn edited_yes_and_no() {
    assert!(m("edited:yes").matches(&edited()));
    assert!(!m("edited:yes").matches(&plain()));
    assert!(m("edited:no").matches(&plain()));
    assert!(!m("edited:no").matches(&edited()));
    assert!(m("-edited:yes").matches(&plain()));
    // Aliases.
    assert!(m("developed:true").matches(&edited()));
    assert!(m("edited=false").matches(&plain()));
    // Not known yet (sidecars still being read): passes either way.
    let unknown = Facts {
        edited: None,
        ..plain()
    };
    assert!(m("edited:yes").matches(&unknown) && m("edited:no").matches(&unknown));
    // Only words; no comparisons.
    assert!(matches!(
        parse("edited:maybe").errors[0].kind,
        ErrorKind::BadValue { .. }
    ));
    assert_eq!(parse("edited>yes").errors[0].kind, ErrorKind::NoCompare);
    assert!(!m("edited:yes").needs_meta());
}

#[test]
fn modules_by_id_and_by_name() {
    assert!(m("module:exposure").matches(&edited()));
    assert!(m("module=denoise").matches(&edited()));
    assert!(m("module:colour").matches(&edited()));
    assert!(!m("module:sharpen").matches(&edited()));
    assert!(!m("module:exposure").matches(&plain()));
    // ORed values.
    assert!(m("module:sharpen,denoise").matches(&edited()));
    // `=` is the whole id.
    assert!(!m("module=colour").matches(&edited()));
    // Names in the UI language (one test sets them: they are global).
    set_module_names(vec![
        ("exposure".into(), "Belichtung".into()),
        ("colour_balance".into(), "Farbbalance".into()),
        ("denoise".into(), "Rauschminderung".into()),
        ("sharpen".into(), "Schärfen".into()),
    ]);
    assert!(m("module:belichtung").matches(&edited()));
    assert!(m("module=Farbbalance").matches(&edited()));
    assert!(m("module:rausch").matches(&edited()));
    assert!(!m("module:Schärfen").matches(&edited()));
    assert_eq!(module_name("denoise").as_deref(), Some("Rauschminderung"));
    // Unknown while the edit has not been read.
    let unknown = Facts {
        edited: None,
        modules: None,
        ..plain()
    };
    assert!(m("module:exposure").matches(&unknown));
}

#[test]
fn styles_from_history() {
    assert!(m("style:punchy").matches(&edited()));
    assert!(m("style=\"Warm evening\"").matches(&edited()));
    assert!(m("styles:warm").matches(&edited()));
    assert!(!m("style=warm").matches(&edited()));
    assert!(!m("style:punchy").matches(&plain()));
    assert!(m("-style:punchy").matches(&plain()));
}

#[test]
fn versions() {
    let v = Facts {
        is_version: true,
        ..edited()
    };
    assert!(m("version:yes").matches(&edited()));
    assert!(!m("version:no").matches(&edited()));
    assert!(m("version:no").matches(&plain()));
    assert!(m("version:copy").matches(&v));
    assert!(!m("version:copy").matches(&edited()));
    assert!(m("versions:has").matches(&edited()));
    assert!(m("version:yes,copy").matches(&v));
    assert!(matches!(
        parse("version:7").errors[0].kind,
        ErrorKind::BadValue { .. }
    ));
}

#[test]
fn develop_fields_work_in_the_gallery_and_the_library() {
    for scope in [Scope::Gallery, Scope::Library] {
        let q = parse_in("edited:yes module:exposure style:x version:copy", scope);
        assert!(q.errors.is_empty(), "{scope:?}: {:?}", q.errors);
        assert_eq!(q.terms.len(), 4);
    }
}

#[test]
fn develop_facets_count() {
    let files = [
        edited(),
        plain(),
        Facts {
            name: "IMG_0003.CR2",
            modules: Some("exposure"),
            styles: Some(""),
            versions: 0,
            ..edited()
        },
        Facts {
            is_version: true,
            ..edited()
        },
    ];
    let q = parse("");
    let mat = Matcher::new(&q, ascii_fold);
    let mut t = Tally::default();
    for f in &files {
        t.add(f, mat.fail_mask(f));
    }
    let facets = t.finish(&q);
    let get = |field: FacetField| {
        facets
            .iter()
            .find(|f| f.field == field)
            .map(|f| {
                f.values
                    .iter()
                    .map(|v| (v.key.clone(), v.count))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    assert_eq!(
        get(FacetField::Edited),
        [("yes".into(), 3), ("no".into(), 1)]
    );
    assert_eq!(
        get(FacetField::Module),
        [
            ("exposure".into(), 3),
            ("colour_balance".into(), 2),
            ("denoise".into(), 2)
        ]
    );
    assert_eq!(
        get(FacetField::Style),
        [("Punchy".into(), 2), ("Warm evening".into(), 2)]
    );
    assert_eq!(
        get(FacetField::Version),
        [("yes".into(), 1), ("no".into(), 2), ("copy".into(), 1)]
    );
    // A facet's own selection leaves its counts alone: selecting `edited:yes` still counts
    // the unedited photo under "no".
    let q = parse("edited:yes");
    let mat = Matcher::new(&q, ascii_fold);
    let mut t = Tally::default();
    for f in &files {
        t.add(f, mat.fail_mask(f));
    }
    let facets = t.finish(&q);
    let e = facets
        .iter()
        .find(|f| f.field == FacetField::Edited)
        .expect("edited");
    assert_eq!(e.values[1].count, 1);
    // Others narrow: only edited photos' versions are counted.
    let v = facets
        .iter()
        .find(|f| f.field == FacetField::Version)
        .expect("version");
    assert_eq!(v.values[1].count, 1);
}

#[test]
fn develop_facets_hide_without_edits() {
    let q = parse("");
    let mat = Matcher::new(&q, ascii_fold);
    let mut t = Tally::default();
    for _ in 0..5 {
        t.add(&plain(), mat.fail_mask(&plain()));
    }
    let facets = t.finish(&q);
    for f in [
        FacetField::Edited,
        FacetField::Module,
        FacetField::Style,
        FacetField::Version,
    ] {
        assert!(facets.iter().all(|x| x.field != f), "{f:?}");
    }
    // Selected, it stays listed so it can be cleared.
    let q = parse("edited:yes");
    let mat = Matcher::new(&q, ascii_fold);
    let mut t = Tally::default();
    t.add(&plain(), mat.fail_mask(&plain()));
    assert!(t.finish(&q).iter().any(|x| x.field == FacetField::Edited));
}

#[test]
fn develop_facets_toggle() {
    let s = toggle("", FacetField::Edited, "yes");
    assert_eq!(s, "edited:yes");
    assert!(is_active(&parse(&s), FacetField::Edited, "yes"));
    // One value only: picking "no" replaces "yes".
    let s = toggle(&s, FacetField::Edited, "no");
    assert!(is_active(&parse(&s), FacetField::Edited, "no"), "{s}");
    assert!(!is_active(&parse(&s), FacetField::Edited, "yes"), "{s}");
    let s = toggle("", FacetField::Module, "exposure");
    assert_eq!(s, "module=exposure");
    let s = toggle(&s, FacetField::Module, "denoise");
    assert_eq!(s, "module=exposure,denoise");
    let s = toggle("", FacetField::Style, "Warm evening");
    assert_eq!(s, "style=\"Warm evening\"");
    assert!(m(&s).matches(&edited()));
    let s = toggle("", FacetField::Version, "copy");
    assert_eq!(s, "version:copy");
}

#[test]
fn develop_names_complete() {
    let n: Vec<&str> = complete::names("mod").into_iter().map(|x| x.0).collect();
    assert!(n.contains(&"module"), "{n:?}");
    let n: Vec<&str> = complete::names("ver").into_iter().map(|x| x.0).collect();
    assert!(n.contains(&"version"), "{n:?}");
    let n: Vec<&str> = complete::names("ed").into_iter().map(|x| x.0).collect();
    assert!(n.contains(&"edited"), "{n:?}");
    let n: Vec<&str> = complete::names("sty").into_iter().map(|x| x.0).collect();
    assert!(n.contains(&"style"), "{n:?}");
}
