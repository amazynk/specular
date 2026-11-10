//! The Exif fields: named ones (`flash:`, `wb:`, `ev:`…) and `exif:Name=value`.

use super::*;
use crate::exif::{FLASH_WORDS, push_tag, tag_named};
use crate::facet::{FacetField, Tally};

fn m(q: &str) -> Matcher {
    let q = parse(q);
    assert!(q.errors.is_empty(), "{:?}", q.errors);
    Matcher::new(&q, ascii_fold)
}

fn lib(q: &str) -> Matcher {
    let q = parse_in(q, Scope::Library);
    assert!(q.errors.is_empty(), "{:?}", q.errors);
    Matcher::new(&q, ascii_fold)
}

fn errs(q: &str, scope: Scope) -> Vec<ErrorKind> {
    parse_in(q, scope)
        .errors
        .into_iter()
        .map(|e| e.kind)
        .collect()
}

/// A tag text: SceneCaptureType night, Flash fired in auto mode with red-eye (0x59),
/// Contrast soft, ExposureTime 1/250, Model, GPS altitude, ExposureBiasValue -1/3.
fn tags() -> String {
    let mut t = String::new();
    push_tag(&mut t, false, 0xA406, "3");
    push_tag(&mut t, false, 0x9209, "89");
    push_tag(&mut t, false, 0xA408, "1");
    push_tag(&mut t, false, 0x829A, "0.004");
    push_tag(&mut t, false, 0x0110, "Canon EOS R5");
    push_tag(&mut t, true, 0x0006, "123.5");
    push_tag(&mut t, false, 0x9204, "-0.333333");
    push_tag(&mut t, false, 0x0131, "Adobe Photoshop\nLightroom");
    t
}

fn exif(tags: Option<&str>) -> ExifFacts<'_> {
    ExifFacts {
        make: Some("Canon"),
        serial: Some("012345678"),
        lens_serial: Some("LS99"),
        software: Some("Adobe Lightroom Classic 13.0"),
        artist: Some("Jane Doe"),
        copyright: Some("(c) 2024 Jane Doe"),
        description: Some("Harbour at dusk"),
        flash: Some(0x59),
        white_balance: Some(1),
        light_source: Some(10),
        exposure_bias: Some(-1.0 / 3.0),
        exposure_program: Some(3),
        exposure_mode: Some(0),
        metering: Some(5),
        focal35: Some(36.0),
        altitude: Some(-12.5),
        tags,
    }
}

fn photo<'a>(e: Option<ExifFacts<'a>>) -> Facts<'a> {
    Facts {
        name: "IMG_1.CR3",
        ext: "CR3",
        known: true,
        width: 8192,
        height: 5464,
        camera: Some("Canon EOS R5"),
        focal: Some(24.0),
        exif: e,
        ..Facts::default()
    }
}

#[test]
fn aliases_do_not_clash() {
    let mut seen = std::collections::HashSet::new();
    for (n, _) in parse::FIELDS {
        assert!(seen.insert(*n), "{n} twice");
    }
    for n in [
        "mode",
        "program",
        "wb",
        "whitebalance",
        "ev",
        "expcomp",
        "alt",
    ] {
        assert!(Field::from_name(n).is_some(), "{n}");
    }
    assert_eq!(Field::from_name("mode"), Some(Field::Program));
    assert_eq!(Field::from_name("author"), Some(Field::Artist));
    assert_eq!(Field::from_name("caption"), Some(Field::Description));
    assert_eq!(Field::from_name("bodyserial"), Some(Field::Serial));
}

#[test]
fn named_text_fields() {
    let f = photo(Some(exif(None)));
    for q in [
        "make:canon",
        "make=CANON",
        "serial:2345",
        "bodyserial:012345678",
        "lensserial:ls99",
        "software:lightroom",
        "artist:jane",
        "author:doe",
        "copyright:2024",
        "description:harbour",
        "caption:dusk",
    ] {
        assert!(m(q).matches(&f), "{q}");
    }
    for q in ["make:nikon", "make=can", "software:capture", "artist:john"] {
        assert!(!m(q).matches(&f), "{q}");
    }
    assert!(m("make:nikon,canon").matches(&f));
    assert!(m("-make:nikon").matches(&f));
    // `camera:` keeps matching maker and model.
    assert!(m("camera:\"canon eos\"").matches(&f));
    assert_eq!(
        errs("make>canon", Scope::Gallery),
        vec![ErrorKind::NoCompare]
    );
    // A field the file does not have: no; not read yet: unknown (passes).
    let bare = photo(Some(ExifFacts::default()));
    assert!(!m("artist:jane").matches(&bare));
    assert!(m("-artist:jane").matches(&bare));
    assert!(m("artist:jane").matches(&photo(None)));
    assert_eq!(m("artist:jane").tri(&photo(None)), Tri::Unknown);
    let unread = Facts {
        known: false,
        ..photo(Some(exif(None)))
    };
    assert_eq!(m("make:nikon").tri(&unread), Tri::Unknown);
    assert!(m("make:x").needs_meta());
}

#[test]
fn flash_words() {
    // 0x59: fired, auto mode, red-eye reduction.
    let f = photo(Some(exif(None)));
    for q in [
        "flash:yes",
        "flash:fired",
        "flash:on",
        "flash:auto",
        "flash:redeye",
    ] {
        assert!(m(q).matches(&f), "{q}");
    }
    for q in ["flash:no", "flash:off", "flash:forced"] {
        assert!(!m(q).matches(&f), "{q}");
    }
    // 0x10: did not fire, forced off; 0x09: fired, forced on.
    let off = photo(Some(ExifFacts {
        flash: Some(0x10),
        ..exif(None)
    }));
    assert!(m("flash:no").matches(&off));
    assert!(m("flash:off").matches(&off));
    assert!(!m("flash:yes").matches(&off));
    let forced = photo(Some(ExifFacts {
        flash: Some(0x09),
        ..exif(None)
    }));
    assert!(m("flash:forced").matches(&forced));
    assert!(m("flash:no,forced").matches(&forced));
    // No Flash tag: none of the words.
    let none = photo(Some(ExifFacts::default()));
    assert!(!m("flash:yes").matches(&none));
    assert!(!m("flash:no").matches(&none));
    assert!(matches!(
        errs("flash:maybe", Scope::Gallery)[..],
        [ErrorKind::BadValue {
            expected: Expected::Word(w),
            ..
        }] if w == FLASH_WORDS
    ));
}

#[test]
fn white_balance_program_and_metering_words() {
    // Manual white balance, cloudy; aperture priority; matrix metering.
    let f = photo(Some(exif(None)));
    for q in [
        "wb:manual",
        "whitebalance:cloudy",
        "wb:overcast",
        "program:aperture",
        "mode:av",
        "program:a",
        "metering:matrix",
        "metering:evaluative",
        "metering:Matrix",
    ] {
        assert!(m(q).matches(&f), "{q}");
    }
    for q in [
        "wb:auto",
        "wb:daylight",
        "wb:tungsten",
        "program:manual",
        "program:shutter",
        "mode:tv",
        "program:auto",
        "metering:spot",
        "metering:centre",
    ] {
        assert!(!m(q).matches(&f), "{q}");
    }
    let g = photo(Some(ExifFacts {
        white_balance: Some(0),
        light_source: Some(3),
        exposure_program: None,
        exposure_mode: Some(0),
        metering: Some(2),
        ..exif(None)
    }));
    assert!(m("wb:auto").matches(&g));
    assert!(m("wb:tungsten").matches(&g));
    assert!(m("wb:incandescent").matches(&g));
    // Automatic exposure where no program is named.
    assert!(m("program:auto").matches(&g));
    assert!(m("metering:center").matches(&g));
    assert!(m("metering:centre-weighted").matches(&g));
    // Manual by ExposureMode alone; bracketing.
    let manual = photo(Some(ExifFacts {
        exposure_program: None,
        exposure_mode: Some(1),
        ..exif(None)
    }));
    assert!(m("program:manual").matches(&manual));
    let bracket = photo(Some(ExifFacts {
        exposure_mode: Some(2),
        ..exif(None)
    }));
    assert!(m("program:bracket").matches(&bracket));
    // Spot is not multi-spot.
    let multi = photo(Some(ExifFacts {
        metering: Some(4),
        ..exif(None)
    }));
    assert!(!m("metering:spot").matches(&multi));
    assert!(m("metering:multi-spot").matches(&multi));
    assert_eq!(
        errs("metering<3", Scope::Gallery),
        vec![ErrorKind::NoCompare]
    );
}

#[test]
fn exposure_compensation_focal35_and_altitude() {
    let f = photo(Some(exif(None)));
    for q in [
        "ev<0",
        "ev:-1/3",
        "ev:-0.3",
        "ev=-0.33",
        "expcomp:-0.3ev",
        "ev:-1..0",
        "ev>=-1",
        "ev<=-1/3",
        "focal35:36",
        "focal35:36mm",
        "focal35>=35",
        "focal35:24..50",
        "altitude<0",
        "alt:-12.5",
        "alt:-20..0",
        "altitude>-13m",
    ] {
        assert!(m(q).matches(&f), "{q}");
    }
    for q in [
        "ev>0",
        "ev:0",
        "ev:-1",
        "ev:-2/3",
        "ev:+1/3",
        "focal35:50",
        "altitude>0",
        "alt:12.5",
    ] {
        assert!(!m(q).matches(&f), "{q}");
    }
    // -0.3 written as -3/10 matches a typed third.
    let tenth = photo(Some(ExifFacts {
        exposure_bias: Some(-0.3),
        ..exif(None)
    }));
    assert!(m("ev:-1/3").matches(&tenth));
    let zero = photo(Some(ExifFacts {
        exposure_bias: Some(0.0),
        ..exif(None)
    }));
    assert!(m("ev:0").matches(&zero));
    assert!(m("ev:+0").matches(&zero));
    assert!(matches!(
        errs("ev:abc", Scope::Gallery)[..],
        [ErrorKind::BadValue {
            expected: Expected::Number,
            ..
        }]
    ));
    // Not read: unknown.
    assert_eq!(m("ev<0").tri(&photo(None)), Tri::Unknown);
    let none = photo(Some(ExifFacts::default()));
    assert!(!m("focal35>0").matches(&none));
}

#[test]
fn exif_is_the_librarys() {
    assert_eq!(
        errs("exif:Flash", Scope::Gallery),
        vec![ErrorKind::Unavailable]
    );
    assert!(errs("exif:Flash", Scope::Library).is_empty());
    assert!(Field::Exif.library_only());
}

#[test]
fn exif_generic_terms() {
    let t = tags();
    let f = photo(Some(exif(Some(&t))));
    for q in [
        "exif:SceneCaptureType",
        "exif:scenecapturetype=night",
        "exif:SceneCaptureType=3",
        "exif:SceneCaptureType=night-scene",
        "exif:SceneCaptureType:night",
        "exif:SceneCaptureType=landscape,night",
        "exif:SceneCaptureType!=landscape",
        "exif:Contrast=soft",
        "exif:Contrast=low",
        "exif:Flash=fired",
        "exif:Flash=redeye",
        "exif:Flash=89",
        "exif:ExposureTime<0.01",
        "exif:ExposureTime=1/250",
        "exif:ExposureTime<=1/250",
        "exif:Model=eos",
        "exif:model=\"EOS R5\"",
        "exif:GPSAltitude>100",
        "exif:GPSAltitude=123.5",
        "exif:ExposureBiasValue<0",
        "exif:ExposureCompensation=-1/3",
        "exif:0xa406=3",
        "exif:g6>100",
        "exif:Software=lightroom",
        "exif:Saturation,SceneCaptureType",
        "-exif:Saturation",
        "-exif:Contrast=hard",
    ] {
        assert!(lib(q).matches(&f), "{q}");
    }
    for q in [
        "exif:Saturation",
        "exif:SceneCaptureType=portrait",
        "exif:SceneCaptureType=2",
        "exif:SceneCaptureType!=night",
        "exif:Contrast=hard",
        "exif:Flash=no",
        "exif:ExposureTime>0.01",
        "exif:Model=nikon",
        "exif:GPSAltitude<100",
        "exif:DigitalZoomRatio>1",
        "-exif:SceneCaptureType",
    ] {
        assert!(!lib(q).matches(&f), "{q}");
    }
    // Tags not loaded or not read: unknown.
    assert_eq!(
        lib("exif:Flash").tri(&photo(Some(exif(None)))),
        Tri::Unknown
    );
    assert_eq!(lib("exif:Flash").tri(&photo(None)), Tri::Unknown);
    // Read, no tags: no.
    assert!(!lib("exif:Flash").matches(&photo(Some(exif(Some(""))))));
}

#[test]
fn exif_errors() {
    let bad = |q: &str| errs(q, Scope::Library);
    assert!(matches!(
        bad("exif:NoSuchTag=1")[..],
        [ErrorKind::BadValue {
            expected: Expected::ExifTag,
            ..
        }]
    ));
    assert!(matches!(
        bad("exif:SceneCaptureType=sunset")[..],
        [ErrorKind::BadValue {
            expected: Expected::TagWord(_),
            ..
        }]
    ));
    assert!(matches!(
        bad("exif:ExposureTime<fast")[..],
        [ErrorKind::BadValue {
            expected: Expected::Number,
            ..
        }]
    ));
    assert_eq!(bad("exif:Flash="), vec![ErrorKind::MissingValue]);
    assert_eq!(bad("exif>3"), vec![ErrorKind::NoCompare]);
    // The error underlines the bad value only.
    let q = parse_in("iso:100 exif:Flash,Nope", Scope::Library);
    assert_eq!(q.errors.len(), 1);
    assert_eq!(
        &"iso:100 exif:Flash,Nope"[q.errors[0].span.start..q.errors[0].span.end],
        "Nope"
    );
}

#[test]
fn exif_tag_names() {
    assert_eq!(tag_named("iso").map(|t| t.id), Some(0x8827));
    assert_eq!(tag_named("ISOSpeedRatings").map(|t| t.id), Some(0x8827));
    assert_eq!(tag_named("bodyserialnumber").map(|t| t.id), Some(0xA431));
    assert!(tag_named("MakerNote").is_none());
    assert!(tag_named("0x927c").is_none());
    // Every name is unique (any case) and every key too.
    let mut names = std::collections::HashSet::new();
    let mut keys = std::collections::HashSet::new();
    for t in exif::TAGS {
        assert!(names.insert(t.name.to_ascii_lowercase()), "{}", t.name);
        assert!(keys.insert(t.key()), "{}", t.name);
        assert_eq!(tag_named(t.name).map(|x| x.key()), Some(t.key()));
    }
    let n: Vec<&str> = exif::names("scene").collect();
    assert_eq!(n, ["SceneType", "SceneCaptureType"]);
    // Every enumerated tag's words are lower case without spaces.
    for t in exif::TAGS {
        for (_, w) in t.words() {
            assert!(
                w.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
                "{w}"
            );
        }
    }
}

#[test]
fn exif_facets_and_completion() {
    let t = tags();
    let files = [
        photo(Some(exif(Some(&t)))),
        photo(Some(ExifFacts {
            make: Some("Nikon"),
            flash: Some(0x10),
            metering: Some(3),
            exposure_bias: Some(0.0),
            ..exif(Some(""))
        })),
        photo(None),
    ];
    let q = parse_in("make:canon", Scope::Library);
    let mt = Matcher::new(&q, ascii_fold);
    let mut tally = Tally::default();
    for f in &files {
        tally.add(f, mt.fail_mask(f));
    }
    let facets = tally.finish(&q);
    let get = |ff: FacetField| {
        facets
            .iter()
            .find(|f| f.field == ff)
            .map(|f| {
                f.values
                    .iter()
                    .map(|v| (v.key.clone(), v.count))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    // The make facet counts disjunctively (both makes listed).
    let make = get(FacetField::Make);
    assert!(make.contains(&("Canon".into(), 1)) && make.contains(&("Nikon".into(), 1)));
    // Other facets count what make:canon lets through.
    assert_eq!(
        get(FacetField::Flash),
        vec![("yes".into(), 1), ("auto".into(), 1), ("redeye".into(), 1)]
    );
    assert_eq!(get(FacetField::Metering), vec![("matrix".into(), 1)]);
    assert_eq!(get(FacetField::ExpComp), vec![("-0.3".into(), 1)]);
    assert_eq!(get(FacetField::Focal35), vec![("36".into(), 1)]);
    let ex = get(FacetField::Exif);
    assert!(ex.contains(&("SceneCaptureType".into(), 1)));
    assert!(ex.contains(&("SceneCaptureType=night".into(), 1)));
    assert!(ex.contains(&("Contrast=soft".into(), 1)));
    assert!(ex.contains(&("Flash=redeye".into(), 1)));
    assert!(ex.contains(&("GPSAltitude".into(), 1)));
    // Every facet value selects what it counts.
    for (k, _) in &ex {
        let q = format!("exif:{}", parse::quote(k));
        assert!(lib(&q).matches(&files[0]), "{q}");
    }
    for (k, _) in get(FacetField::ExpComp) {
        assert!(m(&format!("ev:{k}")).matches(&files[0]));
    }
    // Completion offers the new names.
    let names: Vec<&str> = complete::names("fla").iter().map(|(n, _)| *n).collect();
    assert_eq!(names, ["flash"]);
    let names: Vec<&str> = complete::names("ex").iter().map(|(n, _)| *n).collect();
    assert!(names.contains(&"exif") && names.contains(&"exposure") && names.contains(&"expcomp"));
    let Some(complete::Context::Value(v)) = complete::context("exif:Scene", 10) else {
        panic!("a value context");
    };
    assert_eq!(v.facet, FacetField::Exif);
    let (text, _) = complete::pick("exif:Scene", &v, "SceneCaptureType=night");
    assert_eq!(text, "exif:SceneCaptureType=night");
    let Some(complete::Context::Value(v)) = complete::context("mode:", 5) else {
        panic!("a value context");
    };
    assert_eq!(v.facet, FacetField::Program);
}

/// Matching stays allocation-free per file for the named fields, and `exif:` is cheap: a
/// rough timing over 50 000 files (printed; asserted only loosely, the machine is shared).
#[test]
fn exif_matching_speed() {
    let t = tags();
    let mut other = String::new();
    push_tag(&mut other, false, 0xA406, "0");
    push_tag(&mut other, false, 0x9209, "16");
    for i in 0..30u16 {
        push_tag(&mut other, false, 0x9000 + i, "x");
    }
    let e1 = exif(Some(&t));
    let e2 = ExifFacts {
        tags: Some(&other),
        ..exif(None)
    };
    let files: Vec<Facts> = (0..50_000)
        .map(|i| photo(Some(if i % 3 == 0 { e1 } else { e2 })))
        .collect();
    for q in [
        "exif:SceneCaptureType=night",
        "exif:ExposureTime<0.01",
        "flash:yes wb:manual ev<0",
    ] {
        let mt = lib(q);
        let t0 = std::time::Instant::now();
        let n = files.iter().filter(|f| mt.matches(f)).count();
        let dt = t0.elapsed();
        eprintln!("{q}: {n} of 50000 in {dt:?}");
        assert!(n > 0);
        assert!(dt.as_secs_f64() < 5.0);
    }
}
