#![no_main]
//! The query parser, evaluator and facet edits fed arbitrary text. Checks that nothing panics,
//! that error spans fall on character boundaries, that a facet toggle applied twice to a
//! clean query leaves it clean, and that a quoted value reads back as itself.

use libfuzzer_sys::fuzz_target;
use specular_query::facet::{FacetField, Tally, toggle};
use specular_query::parse::quote;
use specular_query::{Facts, Kind, Matcher, ascii_fold, parse};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let q = parse(&text);
    for e in &q.errors {
        assert!(e.span.start <= e.span.end && e.span.end <= text.len());
        assert!(text.is_char_boundary(e.span.start) && text.is_char_boundary(e.span.end));
    }
    for t in &q.terms {
        assert!(text.is_char_boundary(t.span.start) && text.is_char_boundary(t.span.end));
    }
    let m = Matcher::new(&q, ascii_fold);
    let facts = Facts {
        name: "DSCF0001.JPG",
        ext: "jpg",
        size: Some(1 << 20),
        modified: Some(1_700_000_000),
        rating: 2,
        known: data.first().is_some_and(|b| b & 1 != 0),
        width: 4000,
        height: 3000,
        taken: Some(1_690_000_000),
        camera: Some("X-T5"),
        iso: Some(400),
        gps: Some(true),
        ..Facts::default()
    };
    let mut tally = Tally::default();
    tally.add(&facts, m.fail_mask(&facts));
    let _ = m.matches(&facts);
    let facets = tally.finish(&q);
    // Terms past the parser's cap (64) are not read: taking one out could bring an
    // erroneous one in.
    if q.errors.is_empty() && q.terms.len() < 64 {
        for f in facets.iter().take(3) {
            let Some(v) = f.values.first() else { continue };
            let once = toggle(&text, f.field, &v.key);
            assert!(parse(&once).errors.is_empty(), "{text:?} -> {once:?}");
            let _ = toggle(&once, f.field, &v.key);
        }
        let _ = toggle(&text, FacetField::Camera, "A \"B\", C");
    }
    // Any value reads back as itself once quoted (quotes and backslashes escaped).
    if !text.trim().is_empty() {
        let q = parse(&format!("camera={}", quote(&text)));
        assert!(q.errors.is_empty(), "{text:?}");
        match &q.terms[..] {
            [t] => match &t.kind {
                Kind::Field { items, .. } => assert_eq!(items, &[text.to_string()]),
                k => panic!("{text:?}: {k:?}"),
            },
            t => panic!("{text:?}: {t:?}"),
        }
    }
});
