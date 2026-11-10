//! Position and capture time of every format whose Exif is read without decoding, through
//! the gallery's path: header facts of an opened session ([`meta_of`]), the date probe of the
//! folder scan, and the query's "has location" facet and date filter. Fixtures:
//! tests/data/gpsmeta/make_fixtures.sh.

use std::path::PathBuf;

use specular_decode::Registry;
use specular_query::facet::{FacetField, Tally};
use specular_query::{Facts, Matcher, ascii_fold, parse};
use specular_thumbs::Meta;
use specular_thumbs::generate::{meta_of, parse_exif_date};
use specular_thumbs::list::probe_date_taken;

const WITH_GPS: [&str; 11] = [
    "gps.jpg",
    "gps.png",
    "gps_rawprofile.png",
    "gps.webp",
    "gps.heic",
    "gps.avif",
    "gps.jxl",
    "gps_brob.jxl",
    "gps.psd",
    "gps.cr3",
    "gps.tif",
];

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/gpsmeta")
        .join(name)
}

fn taken() -> i64 {
    parse_exif_date("2021:07:14 09:26:53").unwrap()
}

fn meta(name: &str) -> Meta {
    let s = Registry::new()
        .open_path(&path(name))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    meta_of(s.as_ref())
}

/// What the gallery tells the query about a file (app/specular gallery model).
fn facts<'a>(name: &'a str, m: &'a Meta) -> Facts<'a> {
    Facts {
        name,
        ext: name.rsplit('.').next().unwrap_or(""),
        known: true,
        width: m.width,
        height: m.height,
        taken: m.date_taken,
        camera: m.camera.as_deref(),
        lens: m.lens.as_deref(),
        pages: m.pages,
        bit_depth: m.bit_depth,
        gps: m.gps,
        ..Facts::default()
    }
}

fn mat_lens(q: &str) -> Matcher {
    let q = parse(q);
    assert!(q.errors.is_empty(), "{:?}", q.errors);
    Matcher::new(&q, ascii_fold)
}

#[test]
fn every_format_records_position_and_capture_time() {
    for name in WITH_GPS {
        let m = meta(name);
        assert_eq!(m.gps, Some(true), "{name}");
        // The fixtures' made-up position: 10 N, 20 E.
        assert_eq!(m.position, Some((10.0, 20.0)), "{name}");
        assert_eq!(m.date_taken, Some(taken()), "{name}");
        // The CR3 is Canon's (its maker note is read by Make).
        let make = if name == "gps.cr3" {
            "Canon"
        } else {
            "Specular"
        };
        assert_eq!(
            m.camera.as_deref(),
            Some(&*format!("{make} Fixture Cam")),
            "{name}"
        );
        // The folder scan's date probe reads the same Exif from the file.
        assert_eq!(probe_date_taken(&path(name)), Some(taken()), "{name}");
        // Found in memory and through a reader alike.
        let bytes = std::fs::read(path(name)).unwrap();
        let mem = specular_decode::exif_tiff(&bytes).expect(name).into_owned();
        let read = specular_decode::read_exif_tiff(std::fs::File::open(path(name)).unwrap());
        assert_eq!(read.as_ref(), Some(&mem), "{name}");
    }
    // The CR3's lens is only in its Canon maker note (CMT3): the session and the Exif agree.
    let m = meta("gps.cr3");
    assert_eq!(m.lens.as_deref(), Some("Fixture Lens 35mm F2"));
    let bytes = std::fs::read(path("gps.cr3")).unwrap();
    let t = specular_decode::exif_tiff(&bytes).unwrap();
    let (cam, _) = specular_decode::exif_facts(&t).unwrap();
    assert_eq!(cam.lens.as_deref(), Some("Fixture Lens 35mm F2"));
    // The made-up serial number (0x5E81A1) stays behind.
    assert!(!t.windows(4).any(|w| w == 0x5E81A1u32.to_le_bytes()));
    let lens = mat_lens("lens:fixture");
    assert!(lens.matches(&facts("gps.cr3", &m)));
    assert!(!lens.matches(&facts("gps.jpg", &meta("gps.jpg"))));
    let m = meta("nogps.webp");
    assert_eq!(m.gps, Some(false));
    assert_eq!(m.position, None);
    assert_eq!(m.date_taken, Some(taken()));
}

#[test]
fn has_location_facet_and_date_filter() {
    let names: Vec<&str> = WITH_GPS.iter().copied().chain(["nogps.webp"]).collect();
    let metas: Vec<Meta> = names.iter().map(|n| meta(n)).collect();
    let all: Vec<Facts> = names.iter().zip(&metas).map(|(n, m)| facts(n, m)).collect();

    let mat = |q: &str| {
        let q = parse(q);
        assert!(q.errors.is_empty(), "{:?}", q.errors);
        Matcher::new(&q, ascii_fold)
    };
    let gps = mat("has:gps");
    let no_gps = mat("-has:gps");
    let day = mat("date:2021-07-14");
    let other_day = mat("date:2021-07-15");
    for f in &all {
        let located = f.name != "nogps.webp";
        assert_eq!(gps.matches(f), located, "{}", f.name);
        assert_eq!(no_gps.matches(f), !located, "{}", f.name);
        assert!(day.matches(f), "{}", f.name);
        assert!(!other_day.matches(f), "{}", f.name);
    }

    // Facet counts: "gps" (has location) and the capture year.
    let q = parse("");
    let none = Matcher::new(&q, ascii_fold);
    let mut t = Tally::default();
    for f in &all {
        t.add(f, none.fail_mask(f));
    }
    let facets = t.finish(&q);
    let count = |field: FacetField, key: &str| {
        facets
            .iter()
            .find(|f| f.field == field)
            .and_then(|f| f.values.iter().find(|v| v.key == key))
            .map_or(0, |v| v.count)
    };
    assert_eq!(count(FacetField::Property, "gps"), WITH_GPS.len() as u32);
    assert_eq!(count(FacetField::Year, "2021"), names.len() as u32);
}
