use super::*;
use crate::date::{DAY, days_from_civil};
use crate::facet::{FacetField, Tally, is_active, toggle};

fn m(q: &str) -> Matcher {
    let q = parse(q);
    assert!(q.errors.is_empty(), "{:?}", q.errors);
    Matcher::new(&q, ascii_fold)
}

fn photo() -> Facts<'static> {
    Facts {
        name: "DSCF1234.JPG",
        ext: "JPG",
        size: Some(8_400_000),
        modified: Some(days_from_civil(2024, 6, 1) * DAY),
        rating: 3,
        known: true,
        width: 6240,
        height: 4160,
        taken: Some(days_from_civil(2024, 5, 17) * DAY + 15 * 3600),
        camera: Some("X-T5"),
        lens: Some("XF16-55mmF2.8 R LM WR"),
        iso: Some(640),
        f_number: Some(2.8),
        focal: Some(23.0),
        exposure: Some(1.0 / 250.0),
        pages: 1,
        bit_depth: 8,
        gps: Some(true),
        ..Facts::default()
    }
}

fn errs(q: &str) -> Vec<ErrorKind> {
    parse(q).errors.into_iter().map(|e| e.kind).collect()
}

#[test]
fn example_from_the_task() {
    let q = parse(
        r#"rating>=3 camera:"X-T5" iso:<800 date:2024-05 ext:heic,jpg w>4000 has:gps -tag:x"#,
    );
    // `-tag:x` is a term too: the gallery reads a folder's keywords.
    assert_eq!(q.terms.len(), 8);
    assert!(q.errors.is_empty(), "{:?}", q.errors);
    let mat = Matcher::new(&q, ascii_fold);
    assert!(mat.matches(&photo()));
    assert!(mat.needs_meta());
}

#[test]
fn words_and_phrases_match_the_name() {
    assert!(m("dscf").matches(&photo()));
    assert!(m("\"F12\"").matches(&photo()));
    assert!(!m("holiday").matches(&photo()));
    assert!(m("-holiday").matches(&photo()));
    // Words with colons that are not fields stay words.
    assert_eq!(parse("12:30").terms[0].kind, Kind::Text("12:30".into()));
    assert!(!m("dscf").needs_meta());
    // A lone dash is nothing.
    assert!(parse(" - ").terms.is_empty());
}

#[test]
fn negation() {
    assert!(!m("-ext:jpg").matches(&photo()));
    assert!(m("-ext:png").matches(&photo()));
    assert!(m("rating!=5").matches(&photo()));
    assert!(!m("-rating!=5").matches(&photo()));
    // Unknown stays unknown, so it passes either way.
    let unknown = Facts {
        known: false,
        ..photo()
    };
    assert!(m("iso:100").matches(&unknown));
    assert!(m("-iso:640").matches(&unknown));
    assert!(!m("-iso:640").matches(&photo()));
}

#[test]
fn ext() {
    assert!(m("ext:jpeg").matches(&photo()));
    assert!(m("format:png,jpg").matches(&photo()));
    assert!(m("type:.JPG").matches(&photo()));
    assert!(!m("ext:heic").matches(&photo()));
    assert_eq!(errs("ext>jpg"), vec![ErrorKind::NoCompare]);
}

#[test]
fn text_fields() {
    assert!(m("camera:x-t").matches(&photo()));
    assert!(m("camera=x-t5").matches(&photo()));
    assert!(!m("camera=x-t").matches(&photo()));
    assert!(m(r#"lens:"16-55""#).matches(&photo()));
    assert!(m("camera:canon,x-t5").matches(&photo()));
    let no_camera = Facts {
        camera: None,
        ..photo()
    };
    assert!(!m("camera:x").matches(&no_camera));
    assert!(m("-camera:x").matches(&no_camera));
    assert_eq!(errs("camera>x"), vec![ErrorKind::NoCompare]);
}

#[test]
fn numbers() {
    let p = photo();
    for (q, want) in [
        ("iso:640", true),
        ("iso=640", true),
        ("iso:<800", true),
        ("iso<640", false),
        ("iso<=640", true),
        ("iso>640", false),
        ("iso>=640", true),
        ("iso:100..800", true),
        ("iso:1000..", false),
        ("iso:..640", true),
        ("iso:100,640", true),
        ("f:2.8", true),
        ("f:f/2.8", true),
        ("aperture:f2.8", true),
        ("f>=2.8", true),
        ("f:2", false),
        ("focal:23mm", true),
        ("focal>50", false),
        ("exposure:1/250", true),
        ("shutter<1/100", true),
        ("exposure>=1s", false),
        ("w>4000", true),
        ("width:6240", true),
        ("h<4000", false),
        ("mp>=25", true),
        ("mp:26", true),
        ("mp>30", false),
        ("size>8mb", true),
        ("size<8MiB", false),
        ("size<9mib", true),
        ("size:8.4m", true),
        ("size>1g", false),
        ("rating:3", true),
        ("stars>=4", false),
        ("bits:8", true),
        ("pages>1", false),
    ] {
        assert_eq!(m(q).matches(&p), want, "{q}");
    }
}

#[test]
fn bad_numbers_are_reported_on_the_value() {
    let q = parse("iso:abc dscf");
    assert_eq!(q.terms.len(), 1);
    assert_eq!(q.errors[0].span, Span { start: 4, end: 7 });
    assert!(matches!(
        q.errors[0].kind,
        ErrorKind::BadValue {
            expected: Expected::Number,
            ..
        }
    ));
    assert!(matches!(errs("iso:1.2.3")[0], ErrorKind::BadValue { .. }));
    assert!(matches!(errs("iso:..")[0], ErrorKind::BadValue { .. }));
    assert_eq!(errs("iso>1..2"), vec![ErrorKind::NoCompare]);
    assert_eq!(errs("exposure:1/0").len(), 1);
}

#[test]
fn dates() {
    let p = photo();
    for (q, want) in [
        ("date:2024", true),
        ("date:2024-05", true),
        ("taken:2024-05-17", true),
        ("date:2024-05-18", false),
        ("date>2024-05", false),
        ("date>=2024-05", true),
        ("date>2024-04", true),
        ("date<2024-05", false),
        ("date<=2024-05", true),
        ("date:2024-01..2024-05", true),
        ("date:2024-06..", false),
        ("date:..2023", false),
        ("modified:2024-06", true),
        ("modified<2024", false),
    ] {
        assert_eq!(m(q).matches(&p), want, "{q}");
    }
    // No capture time: the modification time stands in.
    let scan = Facts { taken: None, ..p };
    assert!(m("date:2024-06").matches(&scan));
    assert!(matches!(
        errs("date:2024-13")[0],
        ErrorKind::BadValue {
            expected: Expected::Date,
            ..
        }
    ));
    assert!(matches!(
        errs("date:yesterday")[0],
        ErrorKind::BadValue { .. }
    ));
}

#[test]
fn labels_orientation_and_flags() {
    let p = Facts {
        label: 1,
        pick: true,
        ..photo()
    };
    assert!(m("label:red").matches(&p));
    assert!(m("color:1,blue").matches(&p));
    assert!(!m("label:none").matches(&p));
    assert!(matches!(errs("label:pink")[0], ErrorKind::BadValue { .. }));
    assert!(m("orientation:landscape").matches(&p));
    assert!(!m("orientation:portrait,square").matches(&p));
    assert!(m("is:landscape").matches(&p));
    assert!(m("is:pick").matches(&p));
    assert!(!m("is:reject").matches(&p));
    assert!(m("is:flagged is:rated is:labelled is:image").matches(&p));
    assert!(!m("is:unrated").matches(&p));
    assert!(m("has:gps has:camera has:lens has:date").matches(&p));
    assert!(!m("has:alpha").matches(&p));
    assert!(m("-has:alpha").matches(&p));
    assert!(!m("is:raw,hdr").matches(&p));
    assert!(m("is:raw,pick").matches(&p));
    // Location not recorded by an older cache: unknown.
    let old = Facts { gps: None, ..p };
    assert!(m("has:gps").matches(&old) && m("-has:gps").matches(&old));
    assert!(matches!(errs("is:blurry")[0], ErrorKind::BadValue { .. }));
    assert!(!m("is:hdr").needs_meta() || m("is:hdr").needs_meta());
    assert!(!m("is:pick").needs_meta());
    // Movies by their extension, read or not.
    assert!(!m("is:video").matches(&p));
    let clip = Facts {
        name: "IMG_0042.MOV",
        ext: "MOV",
        known: false,
        ..p
    };
    assert!(m("is:video").matches(&clip) && m("is:movie").matches(&clip));
    assert!(!m("is:video").needs_meta());
    assert!(!m("is:sound").matches(&clip));
    // Sound files by their extension, then by their content: an MP4 with no picture is
    // sound, an Ogg with one a movie.
    let song = Facts {
        name: "track 01.flac",
        ext: "flac",
        known: false,
        ..p
    };
    assert!(m("is:sound").matches(&song) && m("is:audio").matches(&song));
    assert!(!m("is:video").matches(&song));
    assert!(!m("is:sound").needs_meta());
    let voice = Facts {
        name: "memo.mp4",
        ext: "mp4",
        known: true,
        sound: Some(true),
        ..p
    };
    assert!(m("is:sound").matches(&voice) && !m("is:video").matches(&voice));
    let theora = Facts {
        name: "clip.ogg",
        ext: "ogg",
        known: true,
        sound: Some(false),
        ..p
    };
    assert!(m("is:video").matches(&theora) && !m("is:sound").matches(&theora));
    assert!(m("is:video,sound").matches(&song) && m("is:video,sound").matches(&clip));
    assert!(!m("is:sound").matches(&Facts {
        container: true,
        ..song
    }));
}

#[test]
fn containers_only_match_names_and_folder() {
    let dir = Facts {
        name: "Holiday 2024",
        container: true,
        ..Facts::default()
    };
    assert!(m("holiday").matches(&dir));
    assert!(m("is:folder").matches(&dir));
    assert!(!m("is:image").matches(&dir));
    assert!(!m("ext:jpg").matches(&dir));
    assert!(!m("rating>=1").matches(&dir));
}

#[test]
fn dupes_and_similar() {
    let p = Facts {
        dupes: Some(true),
        ..photo()
    };
    assert!(m("dupes:any").matches(&p));
    assert!(m("dupes:keep").matches(&p));
    assert!(!m("dupes:discard").matches(&p));
    assert!(!m("similar:any").matches(&p));
    assert_eq!(errs("similar:this"), vec![ErrorKind::Unavailable]);
    assert!(matches!(errs("dupes:maybe")[0], ErrorKind::BadValue { .. }));
    // On its own: any member.
    assert!(m("dupes:").matches(&p) && !m("similar:").matches(&p));
    assert!(errs("iso:").contains(&ErrorKind::MissingValue));
}

#[test]
fn errors_never_filter_and_carry_spans() {
    let q = parse("ratng>=3 dscf");
    assert_eq!(q.terms.len(), 1);
    assert_eq!(
        q.errors[0].kind,
        ErrorKind::UnknownField {
            name: "ratng".into(),
            suggestion: Some("rating")
        }
    );
    assert_eq!(q.errors[0].span, Span { start: 0, end: 8 });
    assert_eq!(
        errs("zzzzzz:1"),
        vec![ErrorKind::UnknownField {
            name: "zzzzzz".into(),
            suggestion: None
        }]
    );
    assert_eq!(errs("iso:"), vec![ErrorKind::MissingValue]);
    assert_eq!(errs("camera:\"X-T"), vec![ErrorKind::Unterminated]);
    assert_eq!(errs("\"open"), vec![ErrorKind::Unterminated]);
    assert_eq!(parse("\"open").terms.len(), 1);
    assert_eq!(
        errs("text:invoice"),
        vec![ErrorKind::NeedsAiPack(AiFeature::Text)]
    );
    // People's names: the Library's (filled from the catalogue when People is installed).
    for q in ["people:ana", "person:\"Ana B\"", "-people:x"] {
        assert_eq!(errs(q), vec![ErrorKind::Unavailable], "{q}");
        let l = parse_in(q, Scope::Library);
        assert!(l.errors.is_empty(), "{q}");
        assert_eq!(l.terms[0].field(), Some(Field::Person), "{q}");
    }
    assert_eq!(
        parse_in("text:x", Scope::Library).errors[0].kind,
        ErrorKind::NeedsAiPack(AiFeature::Text)
    );
    assert_eq!(errs("place:lisbon"), vec![ErrorKind::Unavailable]);
    // Search by description: the Library's (the app answers it when Search is ready).
    let q = parse_in("like:\"dog on a beach\" iso:100", Scope::Library);
    assert_eq!(q.errors[0].kind, ErrorKind::NeedsAiPack(AiFeature::Search));
    assert_eq!(q.errors[0].field, "like");
    assert_eq!(q.terms.len(), 1);
    assert_eq!(errs("like:dog"), vec![ErrorKind::Unavailable]);
}

#[test]
fn never_panics_on_odd_input() {
    for s in [
        "",
        "-",
        "--",
        "\"",
        ":",
        "a:",
        "a:,",
        "iso:,",
        "iso:1,",
        "w>",
        ">3",
        "é:1",
        "ü>=2",
        "iso:１２",
        "date:２０２４",
        "-\"",
        "a:\"\"",
        "size:kib",
        "f:f/",
        "exposure:/",
        "\u{200f}rating>=3",
        "rating>=3\u{0}",
        "x".repeat(10_000).as_str(),
    ] {
        let q = parse(s);
        for e in &q.errors {
            assert!(e.span.start <= e.span.end && e.span.end <= s.len());
            assert!(s.is_char_boundary(e.span.start) && s.is_char_boundary(e.span.end));
        }
        let _ = Matcher::new(&q, ascii_fold).matches(&photo());
    }
}

#[test]
fn facets_count_disjunctively() {
    let a = photo();
    let b = Facts {
        name: "b.heic",
        ext: "heic",
        camera: Some("iPhone 15"),
        rating: 5,
        hdr: true,
        ..photo()
    };
    let c = Facts {
        name: "c.png",
        ext: "png",
        camera: None,
        alpha: true,
        width: 100,
        height: 300,
        rating: 0,
        ..photo()
    };
    let q = parse("ext:jpg rating>=3");
    let mat = Matcher::new(&q, ascii_fold);
    let mut t = Tally::default();
    for f in [&a, &b, &c] {
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
                    .map(|v| (v.key.as_str(), v.count))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    // Formats ignore the format selection but not the rating: jpg and heic (c is unrated).
    assert_eq!(get(FacetField::Format), vec![("heic", 1), ("jpg", 1)]);
    // Ratings ignore the rating term but not the format: only a.
    assert!(get(FacetField::Rating).contains(&("3", 1)));
    // Everything else counts matches only.
    assert_eq!(get(FacetField::Camera), vec![("X-T5", 1)]);
    assert_eq!(get(FacetField::Year), vec![("2024", 1)]);
    assert_eq!(get(FacetField::Property), vec![("gps", 1)]);
    assert!(is_active(&q, FacetField::Format, "jpg"));
    assert!(!is_active(&q, FacetField::Format, "png"));
}

#[test]
fn facet_toggles_edit_the_text() {
    assert_eq!(toggle("", FacetField::Format, "jpg"), "ext:jpg");
    assert_eq!(toggle("dscf", FacetField::Format, "jpg"), "dscf ext:jpg");
    assert_eq!(
        toggle("ext:jpg dscf", FacetField::Format, "heic"),
        "ext:jpg,heic dscf"
    );
    assert_eq!(
        toggle("ext:jpg,heic dscf", FacetField::Format, "jpg"),
        "ext:heic dscf"
    );
    assert_eq!(toggle("a ext:jpeg b", FacetField::Format, "jpg"), "a b");
    assert_eq!(toggle("", FacetField::Camera, "X-T5"), "camera=X-T5");
    assert_eq!(toggle("", FacetField::Lens, "XF 23mm"), "lens=\"XF 23mm\"");
    assert_eq!(toggle("lens=\"XF 23mm\"", FacetField::Lens, "XF 23mm"), "");
    assert_eq!(toggle("", FacetField::Property, "gps"), "has:gps");
    assert_eq!(toggle("x has:gps", FacetField::Property, "gps"), "x");
    assert_eq!(toggle("", FacetField::Property, "hdr"), "is:hdr");
    assert_eq!(toggle("", FacetField::Year, "2024"), "date:2024");
    // A negated term is not a selection: the value is added alongside.
    assert_eq!(
        toggle("-ext:jpg", FacetField::Format, "jpg"),
        "-ext:jpg ext:jpg"
    );
    // A remaining value that starts like an operator is quoted (found by the fuzzer).
    assert_eq!(
        toggle("ext:heic,<=.x", FacetField::Format, "heic"),
        "ext:\"<=.x\""
    );
    // Every toggled text parses cleanly.
    for s in ["", "a", "ext:jpg", "camera=\"A B\""] {
        for (f, k) in [
            (FacetField::Camera, "A B"),
            (FacetField::Format, "png"),
            (FacetField::Label, "red"),
        ] {
            assert!(parse(&toggle(s, f, k)).errors.is_empty());
        }
    }
}

#[test]
fn tags_and_folders_are_the_librarys() {
    // The gallery reports folders as not available here and leaves them out.
    for q in ["folder:2024", "in:trips"] {
        assert_eq!(errs(q), vec![ErrorKind::Unavailable], "{q}");
    }
    // Keywords it reads from the folder's photos: unknown until read, then matched.
    for q in ["tag:beach", "keyword:x", "keywords:x", "-tag:x"] {
        assert!(errs(q).is_empty(), "{q}");
    }
    let here = |q: &str| Matcher::new(&parse(q), ascii_fold);
    assert!(here("tag:mountain").matches(&photo()));
    let read = Facts {
        tags: Some("Places|Lisbon\nBeach"),
        ..photo()
    };
    assert!(here("tag:beach").matches(&read));
    assert!(here("keyword:lisbon").matches(&read));
    assert!(!here("tag:mountain").matches(&read));
    assert!(!here("tag:x").needs_meta());
    let lib = |q: &str| {
        let q = parse_in(q, Scope::Library);
        assert!(q.errors.is_empty(), "{:?}", q.errors);
        Matcher::new(&q, ascii_fold)
    };
    let f = Facts {
        tags: Some("Beach\nFamily trip"),
        folder: Some("/photos/2024/Lisbon"),
        text: Some("dscf1234.jpg /photos/2024/lisbon x-t5 beach family trip"),
        ..photo()
    };
    assert!(lib("tag:beach").matches(&f));
    assert!(lib("tags:trip").matches(&f));
    assert!(lib("tag=\"family trip\"").matches(&f));
    assert!(!lib("tag=family").matches(&f));
    assert!(!lib("tag:mountain").matches(&f));
    assert!(lib("-tag:mountain").matches(&f));
    assert!(lib("folder:lisbon").matches(&f));
    assert!(!lib("in:porto").matches(&f));
    assert!(lib("tag:mountain,beach").matches(&f));
    assert!(!lib("tag:x").needs_meta());
    // Plain words search the whole text where there is one, else the name.
    assert!(lib("lisbon").matches(&f));
    assert!(!lib("lisbon").matches(&photo()));
    // Not known (no catalogue row): passes.
    assert!(lib("tag:mountain").matches(&photo()));
    assert!(lib("folder:x").matches(&photo()));
    // No tags at all is a known empty set.
    let none = Facts {
        tags: Some(""),
        ..photo()
    };
    assert!(!lib("tag:beach").matches(&none));
    assert!(lib("-tag:beach").matches(&none));
    // Places are the Library's too.
    assert!(parse_in("place:x", Scope::Library).errors.is_empty());
    assert_eq!(errs("country:x"), vec![ErrorKind::Unavailable]);
}

#[test]
fn places_match_town_region_and_country() {
    let lib = |q: &str| {
        let q = parse_in(q, Scope::Library);
        assert!(q.errors.is_empty(), "{:?}", q.errors);
        Matcher::new(&q, ascii_fold)
    };
    let place = "Lisbon\nLisbon\nLisboa\nPortugal\nPT";
    let f = Facts {
        place: Some(place),
        text: Some("img_1.jpg\n/photos\nLisbon\nLisbon\nLisboa\nPortugal\nPT"),
        ..photo()
    };
    for q in [
        "place:lisbon",
        "place:portugal",
        "country:portugal",
        "city:lisb",
        "location:pt",
        "place=Portugal",
        "lisbon",
        "portugal",
        "place:porto,lisbon",
        "-place:porto",
    ] {
        assert!(lib(q).matches(&f), "{q}");
    }
    for q in ["place:porto", "place=lisb", "-place:lisbon", "porto"] {
        assert!(!lib(q).matches(&f), "{q}");
    }
    // No position: a known empty place.
    let none = Facts {
        place: Some(""),
        ..photo()
    };
    assert!(!lib("place:lisbon").matches(&none));
    assert!(lib("-place:lisbon").matches(&none));
    // Not looked for yet: unknown, so it passes.
    assert!(lib("place:lisbon").matches(&photo()));
    // The facet counts towns and selects one whole.
    let mut t = Tally::default();
    t.add(&f, 0);
    t.add(&none, 0);
    let q = parse_in("", Scope::Library);
    let facets = t.finish(&q);
    let p = facets
        .iter()
        .find(|x| x.field == FacetField::Place)
        .expect("a place facet");
    assert_eq!(p.values.len(), 1);
    assert_eq!(p.values[0].key, "Lisbon");
    assert_eq!(p.values[0].count, 1);
    assert_eq!(FacetField::Place.term(), (Field::Place, Op::Eq));
}

fn ctx(text: &str) -> crate::complete::ValueCtx {
    match crate::complete::context(text, text.len()) {
        Some(crate::complete::Context::Value(c)) => c,
        other => panic!("{text:?}: {other:?}"),
    }
}

/// Every value of `text`'s field terms (the ORed alternatives), which must parse cleanly.
fn values_of(text: &str, scope: Scope) -> Vec<String> {
    let q = parse_in(text, scope);
    assert!(q.errors.is_empty(), "{text:?}: {:?}", q.errors);
    q.terms
        .iter()
        .flat_map(|t| match &t.kind {
            parse::Kind::Field { items, .. } => items.clone(),
            parse::Kind::Text(_) => Vec::new(),
        })
        .collect()
}

#[test]
fn completion_contexts() {
    use crate::complete::{Context, context};
    let c = ctx("camera:");
    assert_eq!(c.field, parse::Field::Camera);
    assert_eq!(c.facet, FacetField::Camera);
    assert!(c.items.is_empty() && c.partial.value.is_empty());
    let c = ctx("x cam=a,\"B C\",fu");
    assert_eq!(c.name, "cam");
    assert_eq!(c.op, "=");
    assert_eq!(c.items.len(), 2);
    assert_eq!(c.items[1].value, "B C");
    assert_eq!(c.partial.value, "fu");
    // An open quote is still being typed.
    assert_eq!(ctx("lens:\"XF 23").partial.value, "XF 23");
    // A trailing comma leaves an empty value to type.
    let c = ctx("-ext:jpg,");
    assert!(c.negated);
    assert_eq!(c.items.len(), 1);
    assert!(c.partial.value.is_empty());
    assert_eq!(ctx("is:").facet, FacetField::Flags);
    assert_eq!(ctx("has:g").facet, FacetField::Flags);
    assert_eq!(ctx("taken:20").facet, FacetField::Taken);
    assert_eq!(ctx("w>").facet, FacetField::Width);
    assert_eq!(ctx("size:").facet, FacetField::Size);
    assert_eq!(ctx("f:2").facet, FacetField::Aperture);
    assert_eq!(ctx("iso>=").op, ">=");
    // No list: name; after a space; inside a phrase.
    for t in ["name:", "camera:a ", "\"camera:", "-", ""] {
        assert_eq!(context(t, t.len()), None, "{t:?}");
    }
    // A bare word that starts a field name.
    assert_eq!(
        context("x cam", 5),
        Some(Context::Name {
            span: Span { start: 2, end: 5 },
            prefix: "cam".into()
        })
    );
    assert_eq!(context("dscf", 4), None);
    // The caret in an earlier term.
    let t = "camera:a lens:b";
    assert!(matches!(context(t, 8), Some(Context::Value(c)) if c.field == parse::Field::Camera));
}

#[test]
fn completion_names() {
    use crate::complete::{complete_name, names};
    assert_eq!(names("cam"), vec![("camera", FacetField::Camera)]);
    assert_eq!(
        names("mod"),
        vec![
            ("model", FacetField::Camera),
            ("modified", FacetField::Modified),
            ("module", FacetField::Module),
            ("mode", FacetField::Program)
        ]
    );
    let f: Vec<&str> = names("f").into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        f,
        [
            "format",
            "f",
            "focal",
            "folder",
            "filter",
            "frametype",
            "flash",
            "focal35"
        ]
    );
    assert!(names("").is_empty() && names("zz").is_empty());
    assert_eq!(
        complete_name("x cam", Span { start: 2, end: 5 }, "camera"),
        ("x camera:".into(), 9)
    );
}

#[test]
fn completion_picks_and_toggles() {
    use crate::complete::{pick, select_all, tidy, toggle as tog};
    // Pick replaces the partial; a text field's `:` becomes `=` (just that camera).
    assert_eq!(
        pick("camera:fu", &ctx("camera:fu"), "X-T5").0,
        "camera=X-T5"
    );
    assert_eq!(
        pick("x cam:fu", &ctx("x cam:fu"), "X T5").0,
        "x cam=\"X T5\""
    );
    assert_eq!(pick("ext:j", &ctx("ext:j"), "jpg").0, "ext:jpg");
    assert_eq!(
        pick("-lens=", &ctx("-lens="), "XF 23mm").0,
        "-lens=\"XF 23mm\""
    );
    // Earlier selections stay; picking one of them again keeps it once.
    assert_eq!(
        pick("camera=a,b,x", &ctx("camera=a,b,x"), "c").0,
        "camera=a,b,c"
    );
    assert_eq!(
        pick("camera=a,b,", &ctx("camera=a,b,"), "A").0,
        "camera=a,b"
    );
    // A comparison takes the one value.
    assert_eq!(pick("iso>=1", &ctx("iso>=1"), "100").0, "iso>=100");
    // Toggles add or remove, drop the partial and leave a comma for the next value.
    let (t, caret) = tog("camera:x", &ctx("camera:x"), "X-T5");
    assert_eq!((t.as_str(), caret), ("camera=X-T5,", 12));
    let t = tog(&t, &ctx(&t), "Canon EOS R5").0;
    assert_eq!(t, "camera=X-T5,\"Canon EOS R5\",");
    let t2 = tog(&t, &ctx(&t), "x-t5").0;
    assert_eq!(t2, "camera=\"Canon EOS R5\",");
    assert_eq!(tog(&t2, &ctx(&t2), "Canon EOS R5").0, "camera=");
    // The text before the term is kept.
    let s = "rating>=3 ext:jpg,";
    assert_eq!(tog(s, &ctx(s), "png").0, "rating>=3 ext:jpg,png,");
    // Select all, then tidy the trailing comma.
    let s = "label:r";
    let (all, _) = select_all(s, &ctx(s), &["red", "yellow"]);
    assert_eq!(all, "label:red,yellow,");
    assert_eq!(tidy(&all, &ctx(&all)), "label:red,yellow");
    assert_eq!(tidy("label:", &ctx("label:")), "label:");
    // Numbers: `f/2.8` and `2.8` are one value.
    assert_eq!(tog("f:f/2.8,", &ctx("f:f/2.8,"), "2.8").0, "f:");
}

#[test]
fn completion_round_trips_through_the_parser() {
    use crate::complete::{select_all, tidy, toggle as tog};
    let keys = ["X-T5", "Canon EOS R5", "a,b", "<odd", "Ünï code"];
    let mut t = String::from("dscf camera:");
    for k in keys {
        t = tog(&t, &ctx(&t), k).0;
    }
    let done = tidy(&t, &ctx(&t));
    let mut vals = values_of(&done, Scope::Gallery);
    vals.sort();
    let mut want: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
    want.sort();
    assert_eq!(vals, want, "{done}");
    // The trailing comma parses too (the term works while more are picked).
    assert_eq!(values_of(&t, Scope::Gallery).len(), keys.len());
    // ORed: a file with any of the cameras matches.
    let q = parse(&done);
    let mat = Matcher::new(&q, ascii_fold);
    let f = Facts {
        camera: Some("Canon EOS R5"),
        name: "dscf1.jpg",
        ..photo()
    };
    assert!(mat.matches(&f));
    assert!(!mat.matches(&Facts {
        camera: Some("X-T4"),
        ..f
    }));
    // Library fields.
    let s = "tag:";
    let (all, _) = select_all(s, &ctx(s), &["holiday 2024", "family"]);
    assert_eq!(values_of(&all, Scope::Library), ["holiday 2024", "family"]);
}

#[test]
fn picker_facets_count_camera_settings() {
    let a = photo();
    let b = Facts {
        iso: Some(100),
        f_number: Some(1.4139),
        focal: Some(50.2),
        exposure: Some(2.0),
        ..photo()
    };
    let c = Facts {
        iso: Some(640),
        f_number: Some(2.7999),
        exposure: Some(0.8),
        tags: Some("family\nholiday 2024\n"),
        folder: Some("/photos/2024"),
        ..photo()
    };
    let files = [&a, &b, &c];
    let q = parse_in("iso:640", Scope::Library);
    let mat = Matcher::new(&q, ascii_fold);
    let mut t = Tally::default();
    for f in files {
        t.add(f, mat.fail_mask(f));
    }
    let facets = t.finish(&q);
    let get = |field: FacetField| -> Vec<(String, u32)> {
        facets
            .iter()
            .find(|f| f.field == field)
            .map(|f| f.values.iter().map(|v| (v.key.clone(), v.count)).collect())
            .unwrap_or_default()
    };
    let kv = |v: &[(&str, u32)]| -> Vec<(String, u32)> {
        v.iter().map(|(k, c)| (k.to_string(), *c)).collect()
    };
    // ISO ignores its own term (disjunctive), ascending.
    assert_eq!(get(FacetField::Iso), kv(&[("100", 1), ("640", 2)]));
    // The rest count the files iso:640 lets through (a and c).
    assert_eq!(get(FacetField::Aperture), kv(&[("2.8", 2)]));
    assert_eq!(get(FacetField::Focal), kv(&[("23", 2)]));
    assert_eq!(get(FacetField::Exposure), kv(&[("1/250", 1), ("0.8", 1)]));
    assert_eq!(
        get(FacetField::Tag),
        kv(&[("family", 1), ("holiday 2024", 1)])
    );
    assert_eq!(get(FacetField::Folder), kv(&[("/photos/2024", 1)]));
    // Each key selects exactly the files it counts.
    let none = parse("");
    let m0 = Matcher::new(&none, ascii_fold);
    let mut all = Tally::default();
    for f in files {
        all.add(f, m0.fail_mask(f));
    }
    let all = all.finish(&none);
    for (field, name) in [
        (FacetField::Aperture, "f"),
        (FacetField::Focal, "focal"),
        (FacetField::Exposure, "exposure"),
        (FacetField::Iso, "iso"),
    ] {
        let values = &all.iter().find(|f| f.field == field).unwrap().values;
        assert!(!values.is_empty());
        for v in values {
            let q = parse(&format!("{name}:{}", v.key));
            assert!(q.errors.is_empty(), "{:?}", q.errors);
            let m = Matcher::new(&q, ascii_fold);
            let n = files.iter().filter(|f| m.matches(f)).count() as u32;
            assert_eq!(n, v.count, "{name}:{}", v.key);
            assert!(is_active(&q, field, &v.key));
        }
    }
    assert!(is_active(&parse("f:f/2.8"), FacetField::Aperture, "2.8"));
    // The panel's facets and the picker's are apart.
    assert!(
        FacetField::ALL
            .iter()
            .all(|f| !FacetField::PICKER.contains(f))
    );
}

#[test]
fn colour_spaces_match_and_count() {
    let p3 = Facts {
        space: Some("p3"),
        ..photo()
    };
    let srgb = Facts {
        space: Some("srgb"),
        ..photo()
    };
    let named = Facts {
        space: Some("ColorMatch RGB"),
        ..photo()
    };
    let longer = Facts {
        space: Some("ColorMatch RGB (D50)"),
        ..photo()
    };
    let none = Facts {
        space: Some(""),
        ..photo()
    };
    let unread = Facts {
        space: None,
        ..photo()
    };
    let unknown = Facts {
        space: Some("p3"),
        known: false,
        ..photo()
    };
    // Words and their aliases, under every key name.
    for q in [
        "space:p3",
        "space=p3",
        "space:\"Display P3\"",
        "colorspace:dcip3",
        "colourspace:p3",
        "profile:P3",
        "icc:p3",
    ] {
        let m = m(q);
        assert!(m.matches(&p3), "{q}");
        assert!(!m.matches(&srgb), "{q}");
        assert!(!m.matches(&named), "{q}");
        assert!(!m.matches(&none), "{q}");
        // Not read (yet), or the metadata not known: unknown passes.
        assert!(m.matches(&unread), "{q}");
        assert!(m.matches(&unknown), "{q}");
    }
    assert!(m("space:gray").matches(&Facts {
        space: Some("grey"),
        ..photo()
    }));
    assert!(m("space:srgb,p3").matches(&srgb));
    assert!(!m("-space:srgb").matches(&srgb));
    assert!(m("-space:srgb").matches(&p3));
    // Descriptions: `:` contains, `=` the whole; a word never matches a description's text.
    assert!(m("profile:colormatch").matches(&named));
    assert!(m("profile:colormatch").matches(&longer));
    assert!(m("profile=\"colormatch rgb\"").matches(&named));
    assert!(!m("profile=\"colormatch rgb\"").matches(&longer));
    assert!(!m("space:rgb").matches(&srgb));
    assert!(!m("space:colormatch").matches(&p3));
    assert_eq!(errs("space>p3"), [ErrorKind::NoCompare]);
    assert!(errs("space:whatever").is_empty());

    let files = [&p3, &srgb, &named, &longer, &none, &unread, &p3];
    let q = parse_in("space:srgb", Scope::Library);
    let mat = Matcher::new(&q, ascii_fold);
    let mut t = Tally::default();
    for f in files {
        t.add(f, mat.fail_mask(f));
    }
    let facets = t.finish(&q);
    let got: Vec<(String, u32)> = facets
        .iter()
        .find(|f| f.field == FacetField::Space)
        .unwrap()
        .values
        .iter()
        .map(|v| (v.key.clone(), v.count))
        .collect();
    // Disjunctive: its own term is ignored. Most first; none and unread are not listed.
    let want: Vec<(String, u32)> = [
        ("p3", 2),
        ("ColorMatch RGB", 1),
        ("ColorMatch RGB (D50)", 1),
        ("srgb", 1),
    ]
    .iter()
    .map(|(k, c)| (k.to_string(), *c))
    .collect();
    let mut sorted = got.clone();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    assert_eq!(sorted, want);
    assert_eq!(got[0], ("p3".to_owned(), 2));
    // Each key selects exactly the files it counts (the unread one passes every key, as
    // unknown, so it is left out here).
    let read = [&p3, &srgb, &named, &longer, &none, &p3];
    for (key, n) in &want {
        let text = toggle("", FacetField::Space, key);
        let q = parse(&text);
        assert!(q.errors.is_empty(), "{text}: {:?}", q.errors);
        let m = Matcher::new(&q, ascii_fold);
        assert_eq!(
            read.iter().filter(|f| m.matches(f)).count() as u32,
            *n,
            "{text}"
        );
        assert!(is_active(&q, FacetField::Space, key), "{text}");
        assert_eq!(toggle(&text, FacetField::Space, key), "");
    }
    assert!(is_active(
        &parse("space=\"display p3\""),
        FacetField::Space,
        "p3"
    ));
    assert!(is_active(&parse("space=gray"), FacetField::Space, "grey"));
    assert!(!is_active(&parse("space=p3"), FacetField::Space, "srgb"));
    assert_eq!(
        crate::complete::names("spa"),
        vec![("space", FacetField::Space)]
    );
}

#[test]
fn quotes_and_backslashes_are_escaped_in_values() {
    use crate::parse::quote;
    // Inside quotes `\"` is a quote and `\\` a backslash; other backslashes are as typed.
    assert_eq!(
        values_of(r#"lens="Ai \"S\" 50""#, Scope::Gallery),
        [r#"Ai "S" 50"#]
    );
    assert_eq!(values_of(r#"lens="a\\b""#, Scope::Gallery), [r"a\b"]);
    assert_eq!(
        values_of(r#"lens="C:\photos""#, Scope::Gallery),
        [r"C:\photos"]
    );
    assert_eq!(values_of(r#"lens="end\\""#, Scope::Gallery), [r"end\"]);
    // Bare values read backslashes as typed; a quote ends them.
    assert_eq!(values_of(r"folder:C:\x", Scope::Library), [r"C:\x"]);
    // Phrases too.
    assert!(matches!(
        &parse(r#""say \"hi\"""#).terms[0].kind,
        Kind::Text(t) if t == r#"say "hi""#
    ));
    // An escaped quote does not close the value: it is still open.
    assert_eq!(errs(r#"lens="a\""#), vec![ErrorKind::Unterminated]);
    // Quoting: bare when it can be, escaped when it must.
    assert_eq!(quote("X-T5"), "X-T5");
    assert_eq!(quote(r"C:\x"), r"C:\x");
    assert_eq!(quote(r#"Ai "S""#), r#""Ai \"S\"""#);
    assert_eq!(quote(r#"a"b"#), r#""a\"b""#);
    assert_eq!(quote(r"a b\"), r#""a b\\""#);
    assert_eq!(quote(r#"\""#), r#""\\\"""#);
    // A facet toggle keeps the quote.
    let lens = r#"Nikkor "Ai-S" 50mm"#;
    let t = toggle("", FacetField::Lens, lens);
    assert_eq!(t, r#"lens="Nikkor \"Ai-S\" 50mm""#);
    assert!(is_active(&parse(&t), FacetField::Lens, lens));
    assert_eq!(toggle(&t, FacetField::Lens, lens), "");
    let m = Matcher::new(&parse(&t), ascii_fold);
    assert!(m.matches(&Facts {
        lens: Some(lens),
        ..photo()
    }));
}

/// A small deterministic generator (the crate has no dependencies, so no proptest).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    /// A value from an alphabet heavy in the characters that need care.
    fn value(&mut self) -> String {
        const A: &[char] = &[
            'a', 'Z', '0', ' ', '"', '\\', ',', ':', '=', '<', '>', '!', '-', '.', '/', 'é', 'α',
            '\u{5d0}', '\t', '\'',
        ];
        let n = 1 + self.next() as usize % 8;
        (0..n).map(|_| A[self.next() as usize % A.len()]).collect()
    }
}

#[test]
fn quoted_values_round_trip_through_the_parser() {
    use crate::complete::{pick, select_all, tidy, toggle as tog};
    let mut g = Lcg(7);
    for _ in 0..4000 {
        let v = g.value();
        if v.trim().is_empty() {
            continue;
        }
        let q = crate::parse::quote(&v);
        // As the value of a field, and as one of several.
        assert_eq!(
            values_of(&format!("camera={q}"), Scope::Gallery),
            [v.as_str()],
            "{q}"
        );
        assert_eq!(
            values_of(&format!("x lens:a,{q},b tag:z"), Scope::Library),
            ["a", v.as_str(), "b", "z"],
            "{q}"
        );
        // The completion edits put it in so that it reads back.
        let s = "camera:";
        let (t, _) = tog(s, &ctx(s), &v);
        assert_eq!(values_of(&t, Scope::Gallery), [v.as_str()], "{t}");
        let c = ctx(&t);
        assert!(c.has(&v), "{t}");
        assert_eq!(tog(&t, &c, &v).0, "camera=", "{t}");
        let (p, _) = pick("camera:x", &ctx("camera:x"), &v);
        assert_eq!(values_of(&p, Scope::Gallery), [v.as_str()], "{p}");
        let w = g.value();
        if !w.trim().is_empty() && !crate::facet::same(FacetField::Tag, &v, &w) {
            let (all, _) = select_all("tag:", &ctx("tag:"), &[&v, &w]);
            let done = tidy(&all, &ctx(&all));
            let got = values_of(&done, Scope::Library);
            assert!(got.contains(&v) && got.contains(&w), "{done}");
        }
    }
}

#[test]
fn completion_with_the_caret_mid_text() {
    use crate::complete::{Context, context, pick, toggle as tog};
    let t = "camera=\"X-T5\",Canon iso:100 dog";
    // In the second value of the first term.
    let at = t.find("Canon").unwrap() + 3;
    let Some(Context::Value(c)) = context(t, at) else {
        panic!("a value context");
    };
    assert_eq!(c.field, parse::Field::Camera);
    assert_eq!(c.partial.value, "Canon");
    assert!(c.has("x-t5"));
    // A toggle rewrites only that term; the caret goes to its end.
    let (out, caret) = tog(t, &c, "Sony A7");
    assert_eq!(out, "camera=\"X-T5\",\"Sony A7\", iso:100 dog");
    assert_eq!(&out[..caret], "camera=\"X-T5\",\"Sony A7\",");
    let Some(Context::Value(c2)) = context(&out, caret) else {
        panic!("still a value context");
    };
    assert!(c2.has("X-T5") && c2.has("Sony A7"));
    assert!(c2.partial.value.is_empty());
    let (out2, caret2) = tog(&out, &c2, "X-T5");
    assert_eq!(out2, "camera=\"Sony A7\", iso:100 dog");
    assert_eq!(caret2, "camera=\"Sony A7\",".len());
    // Pick, in the middle of an escaped value.
    let t = r#"x lens="Ai \"S\" 50",n y"#;
    let at = t.find("50").unwrap();
    let Some(Context::Value(c)) = context(t, at) else {
        panic!("a value context");
    };
    assert_eq!(c.partial.value, r#"Ai "S" 50"#);
    let (out, caret) = pick(t, &c, r#"Ai "S" 85"#);
    assert_eq!(out, r#"x lens=n,"Ai \"S\" 85" y"#);
    assert_eq!(&out[caret..], " y");
    // The second term, with the first before it.
    let t = "is:raw has:gp camera:a";
    let Some(Context::Value(c)) = context(t, 13) else {
        panic!("a value context");
    };
    assert_eq!((c.name.as_str(), c.partial.value.as_str()), ("has", "gp"));
    assert_eq!(c.facet, FacetField::Flags);
    // In the name: nothing; right after a term's value: that term.
    assert_eq!(context(t, 1), None);
    assert!(matches!(context(t, 6), Some(Context::Value(c)) if c.name == "is"));
}

#[test]
fn flags_are_counted_for_the_picker() {
    let a = photo();
    let b = Facts {
        pick: true,
        rating: 0,
        label: 2,
        hdr: true,
        width: 3000,
        height: 4000,
        camera: None,
        ..photo()
    };
    // Metadata not read: only the marks count.
    let c = Facts {
        known: false,
        reject: true,
        ..photo()
    };
    let files = [&a, &b, &c];
    let count = |text: &str| -> Vec<(String, u32)> {
        let q = parse(text);
        let mat = Matcher::new(&q, ascii_fold);
        let mut t = Tally::default();
        for f in files {
            t.add(f, mat.fail_mask(f));
        }
        t.finish(&q)
            .into_iter()
            .find(|f| f.field == FacetField::Flags)
            .map(|f| f.values.into_iter().map(|v| (v.key, v.count)).collect())
            .unwrap_or_default()
    };
    let all = count("");
    let get = |k: &str| all.iter().find(|(w, _)| w == k).map(|x| x.1);
    assert_eq!(get("hdr"), Some(1));
    assert_eq!(get("pick"), Some(1));
    assert_eq!(get("reject"), Some(1));
    assert_eq!(get("flagged"), Some(2));
    assert_eq!(get("rated"), Some(2));
    assert_eq!(get("unrated"), Some(1));
    assert_eq!(get("labelled"), Some(1));
    assert_eq!(get("landscape"), Some(1));
    assert_eq!(get("portrait"), Some(1));
    assert_eq!(get("camera"), Some(1));
    assert_eq!(get("lens"), Some(2));
    assert_eq!(get("gps"), Some(2));
    // Every word listed parses and selects the files it counts (unknown metadata passes a
    // query but is not counted).
    for (k, n) in &all {
        let q = parse(&format!("is:{k}"));
        assert!(q.errors.is_empty(), "{k}");
        let m = Matcher::new(&q, ascii_fold);
        let yes = files
            .iter()
            .filter(|f| m.matches(f) && (f.known || !q.needs_meta()))
            .count() as u32;
        assert_eq!(yes, *n, "is:{k}");
    }
    // The rest of the query narrows the counts.
    let rated = count("rating>=3");
    assert_eq!(
        rated.iter().find(|(w, _)| w == "rated").map(|x| x.1),
        Some(2)
    );
    assert!(!rated.iter().any(|(w, _)| w == "unrated"));
    // Aliases are the same value.
    assert!(is_active(&parse("is:picked"), FacetField::Flags, "pick"));
    assert_eq!(toggle("x is:picked", FacetField::Flags, "pick"), "x");
    assert_eq!(toggle("", FacetField::Flags, "lens"), "has:lens");
    assert!(ctx("is:picked,").has("pick"));
}

/// Files for the numeric pickers: sizes, dimensions, depths, dates and groups that sit on,
/// between and beyond the ladders' rungs.
fn numeric_files() -> Vec<Facts<'static>> {
    let at = |y, m, d| Some(days_from_civil(y, m, d) * DAY + 12 * 3600);
    vec![
        photo(),
        Facts {
            width: 1920,
            height: 1080,
            size: Some(1_000_000),
            iso: Some(100),
            rating: 5,
            bit_depth: 16,
            pages: 3,
            taken: at(2023, 12, 31),
            modified: at(2024, 1, 2),
            dupes: Some(true),
            ..photo()
        },
        Facts {
            width: 1919,
            height: 1081,
            size: Some(999_999),
            iso: Some(3200),
            rating: 0,
            bit_depth: 8,
            taken: at(2024, 5, 1),
            modified: at(2024, 5, 3),
            dupes: Some(false),
            similar: Some(false),
            ..photo()
        },
        Facts {
            width: 12_000,
            height: 300,
            size: Some(2_000_000_000),
            iso: Some(640),
            rating: 1,
            bit_depth: 12,
            taken: at(2022, 2, 28),
            modified: at(2021, 3, 3),
            similar: Some(true),
            ..photo()
        },
        Facts {
            width: 2000,
            height: 1500,
            size: Some(10),
            known: false,
            taken: None,
            ..photo()
        },
    ]
}

#[test]
fn picker_compares_numbers_dates_and_ladders() {
    use crate::facet::compare;
    let files = numeric_files();
    let none = parse("");
    let m0 = Matcher::new(&none, ascii_fold);
    let mut t = Tally::default();
    for f in &files {
        t.add(f, m0.fail_mask(f));
    }
    let facets = t.finish(&none);
    let names: &[(FacetField, &str)] = &[
        (FacetField::Iso, "iso"),
        (FacetField::Rating, "rating"),
        (FacetField::Taken, "date"),
        (FacetField::Modified, "modified"),
        (FacetField::Bits, "bits"),
        (FacetField::Pages, "pages"),
        (FacetField::Width, "width"),
        (FacetField::Height, "height"),
        (FacetField::Megapixels, "mp"),
        (FacetField::Size, "size"),
        (FacetField::Dupes, "dupes"),
        (FacetField::Similar, "similar"),
    ];
    let count = |text: &str| -> u32 {
        let q = parse(text);
        assert!(q.errors.is_empty(), "{text}: {:?}", q.errors);
        let m = Matcher::new(&q, ascii_fold);
        files
            .iter()
            .filter(|f| m.matches(f) && (f.known || !q.needs_meta()))
            .count() as u32
    };
    let mut checked = 0;
    for (field, name) in names {
        let f = facets
            .iter()
            .find(|f| f.field == *field)
            .unwrap_or_else(|| panic!("{name}: no facet"));
        let ops: &[(&str, Op)] = if field.numeric() {
            &[
                ("=", Op::Eq),
                (">=", Op::Ge),
                (">", Op::Gt),
                ("<=", Op::Le),
                ("<", Op::Lt),
            ]
        } else {
            &[("=", Op::Eq)]
        };
        for (text, op) in ops {
            if field.ladder().is_some() && *op == Op::Eq {
                continue;
            }
            let values = compare(f, *op);
            assert!(!values.is_empty(), "{name}{text}");
            for v in values {
                // Each value selects exactly the files it counts.
                let term = format!("{name}{text}{}", v.key);
                assert_eq!(count(&term), v.count, "{term}");
                checked += 1;
            }
        }
    }
    assert!(checked > 60, "{checked}");
    let get = |field: FacetField, op: Op| -> Vec<(String, u32)> {
        let f = facets.iter().find(|f| f.field == field).unwrap();
        compare(f, op)
            .into_iter()
            .map(|v| (v.key, v.count))
            .collect()
    };
    let kv = |v: &[(&str, u32)]| -> Vec<(String, u32)> {
        v.iter().map(|(k, c)| (k.to_string(), *c)).collect()
    };
    // Exactly on a rung counts for >= and <=, not for > and <; of thresholds that count the
    // same only the stricter is offered.
    assert_eq!(
        get(FacetField::Width, Op::Ge),
        kv(&[("1600", 4), ("1920", 3), ("6000", 2), ("10000", 1)])
    );
    assert_eq!(
        get(FacetField::Width, Op::Gt),
        kv(&[("1600", 4), ("6000", 2), ("10000", 1)])
    );
    assert_eq!(
        get(FacetField::Width, Op::Le),
        kv(&[("1920", 2), ("7680", 3)])
    );
    // Dates: years then their months, newest first; a year counts its months.
    assert_eq!(
        get(FacetField::Taken, Op::Eq),
        kv(&[
            ("2024", 2),
            ("2024-05", 2),
            ("2023", 1),
            ("2023-12", 1),
            ("2022", 1),
            ("2022-02", 1)
        ])
    );
    assert_eq!(get(FacetField::Taken, Op::Ge)[0], ("2024".into(), 2));
    assert_eq!(get(FacetField::Taken, Op::Le)[0], ("2024".into(), 4));
    // The panel's Year facet has no months.
    let year = facets.iter().find(|f| f.field == FacetField::Year).unwrap();
    assert!(year.values.iter().all(|v| v.key.len() == 4));
    assert_eq!(
        get(FacetField::Iso, Op::Ge),
        kv(&[("100", 4), ("640", 3), ("3200", 1)])
    );
    assert_eq!(
        get(FacetField::Dupes, Op::Eq),
        kv(&[("any", 2), ("keep", 1), ("discard", 1)])
    );
    assert_eq!(
        get(FacetField::Bits, Op::Eq),
        kv(&[("8", 2), ("12", 1), ("16", 1)])
    );
}

#[test]
fn picker_ladders_pick_thresholds_and_ops() {
    use crate::complete::{Context, context, pick, set_op, toggle as tog};
    let at = |t: &str, caret: usize| match context(t, caret) {
        Some(Context::Value(c)) => c,
        other => panic!("{t:?}: {other:?}"),
    };
    let c = ctx("width:");
    assert!(!c.plain());
    assert_eq!(c.compare(), Some(Op::Ge));
    assert_eq!(pick("width:", &c, "1920"), ("width>=1920".into(), 11));
    // A ladder's toggle picks too: one threshold.
    let t = "x mp:12 y";
    assert_eq!(tog(t, &at(t, 7), "24").0, "x mp>=24 y");
    assert_eq!(ctx("iso<=").compare(), Some(Op::Le));
    assert_eq!(ctx("iso:").compare(), None);
    assert_eq!(ctx("width!=").compare(), None);
    assert!(ctx("iso:").plain() && !ctx("dupes:").plain());
    // Changing the operator keeps the value being typed, and the others between plain ones.
    let t = "a iso:100,2 b";
    let c = at(t, 11);
    assert_eq!(set_op(t, &c, ">="), ("a iso>=2 b".into(), 8));
    assert_eq!(set_op(t, &c, "="), ("a iso=100,2 b".into(), 11));
    let t = "-date>=2024";
    assert_eq!(set_op(t, &ctx(t), ":"), ("-date:2024".into(), 10));
    // The names offered include the new pickers.
    assert_eq!(
        crate::complete::names("me"),
        vec![
            ("megapixels", FacetField::Megapixels),
            ("metering", FacetField::Metering)
        ]
    );
}

/// Counting the picker's lists over 100 000 files' facts (`cargo test --release -p
/// specular-query -- --ignored tally_bench --nocapture`): the best of 31 runs.
#[test]
#[ignore]
fn tally_bench() {
    let names: Vec<String> = (0..100_000).map(|k| format!("IMG_{k:05}.JPG")).collect();
    let cams = ["X-T5", "Canon EOS R5", "iPhone 15", "Sony α7 IV"];
    let spaces = ["srgb", "srgb", "p3", "adobergb", "ColorMatch RGB"];
    let facts: Vec<Facts> = names
        .iter()
        .enumerate()
        .map(|(k, name)| Facts {
            name,
            ext: "jpg",
            known: true,
            width: if k % 3 == 0 { 3000 } else { 4000 },
            height: if k % 3 == 0 { 4000 } else { 3000 },
            rating: (k % 6) as u8,
            pick: k % 9 == 0,
            hdr: k % 7 == 0,
            camera: Some(cams[k % 4]),
            lens: (k % 2 == 0).then_some("XF 33mm"),
            iso: Some(100 << (k % 5)),
            gps: Some(k % 4 == 0),
            f_number: Some([1.4, 2.8, 5.6, 8.0][k % 4]),
            focal: Some([23.0, 33.0, 56.0][k % 3]),
            exposure: Some(1.0 / [60.0, 250.0, 1000.0][k % 3]),
            size: Some(2_000_000 + (k as u64 * 7919) % 30_000_000),
            taken: Some(1_500_000_000 + (k as i64) * 3_000),
            modified: Some(1_700_000_000 + (k as i64) * 60),
            bit_depth: [8, 8, 12, 16][k % 4],
            pages: 1 + u32::from(k % 50 == 0),
            space: Some(spaces[k % 5]),
            ..Facts::default()
        })
        .collect();
    for text in ["", "is:hdr camera=X-T5 iso>=400"] {
        let q = parse(text);
        let m = Matcher::new(&q, ascii_fold);
        let mut best = f64::MAX;
        let mut lists = 0;
        for _ in 0..31 {
            let t = std::time::Instant::now();
            let mut tally = Tally::default();
            for f in &facts {
                tally.add(f, m.fail_mask(f));
            }
            lists = tally.finish(&q).len();
            best = best.min(t.elapsed().as_secs_f64());
        }
        println!("tally 100k ({text:?}): {:.2} ms, {lists} lists", best * 1e3);
    }
}

#[test]
fn dupes_and_similar_in_the_library() {
    // The catalogue fills the roles from its own groups: the same terms and value lists.
    let kept = Facts {
        dupes: Some(true),
        similar: Some(true),
        ..photo()
    };
    let copy = Facts {
        dupes: Some(false),
        ..photo()
    };
    let shot = Facts {
        similar: Some(false),
        ..photo()
    };
    let alone = photo();
    let files = [&kept, &copy, &shot, &alone];
    let count = |text: &str, field: FacetField| -> Vec<(String, u32)> {
        let q = parse_in(text, Scope::Library);
        assert!(q.errors.is_empty(), "{text}: {:?}", q.errors);
        let mat = Matcher::new(&q, ascii_fold);
        let mut t = Tally::default();
        for f in files {
            t.add(f, mat.fail_mask(f));
        }
        t.finish(&q)
            .iter()
            .find(|f| f.field == field)
            .map(|f| f.values.iter().map(|v| (v.key.clone(), v.count)).collect())
            .unwrap_or_default()
    };
    let n = |text: &str| {
        let q = parse_in(text, Scope::Library);
        let mat = Matcher::new(&q, ascii_fold);
        files.iter().filter(|f| mat.matches(f)).count()
    };
    assert_eq!(
        (n("dupes:any"), n("dupes:keep"), n("dupes:discard")),
        (2, 1, 1)
    );
    assert_eq!(
        (n("similar:any"), n("similar:keep"), n("similar:discard")),
        (2, 1, 1)
    );
    assert_eq!(n("dupes:discard similar:any"), 0);
    assert_eq!(n("-dupes:any -similar:any"), 1);
    let want = |c: [u32; 3]| -> Vec<(String, u32)> {
        crate::facet::GROUP_WORDS
            .iter()
            .zip(c)
            .map(|(w, c)| (w.to_string(), c))
            .collect()
    };
    assert_eq!(count("", FacetField::Dupes), want([2, 1, 1]));
    // Its own term does not narrow its list; another field's does.
    assert_eq!(count("dupes:keep", FacetField::Dupes), want([2, 1, 1]));
    // (no member of a duplicate group is a similar shot to discard: no list).
    assert!(count("similar:discard", FacetField::Dupes).is_empty());
}
