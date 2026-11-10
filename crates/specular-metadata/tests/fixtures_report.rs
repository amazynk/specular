//! End-to-end checks on the generated fixtures (tests/fixtures/make_fixtures.sh):
//! JPEG scan -> read -> attach_place, asserting the exact plain-language strings.
//! Fluent wraps every interpolated value in FSI/PDI isolates; assertions strip them
//! (and one test checks that they are there).

use specular_metadata::*;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 2025-01-01: after every fixture's capture date.
fn now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_735_689_600)
}

fn fixture(name: &str) -> Option<Vec<u8>> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    match std::fs::read(&p) {
        Ok(d) => Some(d),
        Err(_) => {
            eprintln!(
                "skipping: fixture {} missing (run tests/fixtures/make_fixtures.sh)",
                p.display()
            );
            None
        }
    }
}

fn report_from(data: &[u8], name: &str, l: &Localizer) -> MetadataReport {
    let seg = jpeg::scan(data).expect("valid JPEG");
    let file = FileFacts {
        path: Some(PathBuf::from(name)),
        file_size: Some(data.len() as u64),
        ..Default::default()
    };
    let mut r = read_at(&seg.input(file), l, now());
    attach_place(&mut r, l);
    r
}

fn report(name: &str, lang: &str) -> Option<MetadataReport> {
    let data = fixture(name)?;
    Some(report_from(
        &data,
        name,
        &Localizer::new(lang.parse().unwrap()),
    ))
}

/// The item's value (isolates stripped), or None.
fn val(r: &MetadataReport, key: &str) -> Option<String> {
    r.summary
        .iter()
        .flat_map(|g| &g.items)
        .find(|i| i.key == key)
        .map(|i| strip_isolates(&i.value))
}

fn v(r: &MetadataReport, key: &str) -> String {
    val(r, key).unwrap_or_else(|| {
        let keys: Vec<_> = r
            .summary
            .iter()
            .flat_map(|g| &g.items)
            .map(|i| i.key)
            .collect();
        panic!("no summary item {key:?}; have {keys:?}")
    })
}

fn detail(r: &MetadataReport, key: &str) -> String {
    let i = r
        .summary
        .iter()
        .flat_map(|g| &g.items)
        .find(|i| i.key == key)
        .expect(key);
    strip_isolates(i.detail.as_deref().unwrap_or(""))
}

fn warnings(r: &MetadataReport) -> Vec<String> {
    r.warnings.iter().map(|w| strip_isolates(w)).collect()
}

#[test]
fn lisbon_english() {
    let Some(r) = report("lisbon.jpg", "en") else {
        return;
    };
    assert_eq!(v(&r, "camera"), "Canon EOS R5"); // "Canon" + "Canon EOS R5" deduplicated
    assert_eq!(v(&r, "software"), "Firmware 1.8.1");
    assert_eq!(v(&r, "shutter"), "1/250 s");
    assert_eq!(v(&r, "aperture"), "f/2.8");
    assert_eq!(v(&r, "iso"), "ISO 400");
    assert_eq!(v(&r, "focal_length"), "35 mm (52 mm equiv.)");
    assert_eq!(v(&r, "exposure_bias"), "+0.7 EV");
    assert_eq!(v(&r, "program"), "Aperture priority");
    assert_eq!(v(&r, "metering"), "Matrix (evaluative)");
    assert_eq!(v(&r, "white_balance"), "Auto");
    assert_eq!(v(&r, "flash"), "Flash fired, auto mode"); // 0x19
    assert_eq!(v(&r, "lens"), "RF24-70mm F2.8 L IS USM");
    assert_eq!(detail(&r, "lens"), "24–70 mm f/2.8");
    assert_eq!(v(&r, "taken"), "Tuesday 14 May 2024, 18:32 (UTC+1)");
    assert_eq!(v(&r, "edited"), "Monday 20 May 2024, 09:15");
    assert_eq!(v(&r, "place"), "Lisbon, Portugal");
    assert_eq!(detail(&r, "place"), "0.8 km from Lisbon");
    assert_eq!(v(&r, "coordinates"), "38.7223° N, 9.1393° W");
    assert_eq!(v(&r, "altitude"), "112 m above sea level");
    assert_eq!(v(&r, "direction"), "Facing west-northwest (287°)");
    assert_eq!(detail(&r, "direction"), "Relative to true north");
    assert_eq!(v(&r, "accuracy"), "±5 m");
    assert_eq!(v(&r, "gps_time"), "14 May 2024, 17:32:10 UTC");
    assert_eq!(v(&r, "dimensions"), "64 × 48");
    assert_eq!(v(&r, "encoding"), "8-bit YCbCr 4:2:0, progressive");
    assert_eq!(v(&r, "colour_profile"), "sP3");
    assert_eq!(v(&r, "orientation"), "Rotated 90° clockwise (applied)");
    assert_eq!(v(&r, "name"), "lisbon.jpg");
    assert_eq!(v(&r, "format"), "JPEG");
    assert_eq!(v(&r, "title"), "Tram 28 in Alfama");
    assert_eq!(v(&r, "creator"), "Ana Silva");
    assert_eq!(v(&r, "copyright"), "© 2024 Ana Silva");
    assert_eq!(v(&r, "keywords"), "tram, lisbon, alfama");
    assert_eq!(v(&r, "rating"), "★★★★☆ (4 of 5)");
    assert_eq!(v(&r, "label"), "Red");
    assert_eq!(val(&r, "c2pa"), None);
    assert_eq!(
        warnings(&r),
        [
            "This file records where it was taken. Remove the location before sharing if that should stay private."
        ]
    );
    assert_eq!(
        one_line_summary(&r).map(|s| strip_isolates(&s)).as_deref(),
        Some("1/250 s · f/2.8 · ISO 400 · 35 mm (52 mm equiv.)")
    );

    // Structured fields.
    assert_eq!(r.orientation, Some(6));
    assert_eq!(r.rating, Some(4));
    assert_eq!(r.label.as_deref(), Some("Red"));
    assert_eq!(r.icc_description.as_deref(), Some("sP3"));
    assert_eq!(r.capture_time.as_deref(), Some("2024-05-14T18:32:10+01:00"));
    assert_eq!(r.language, "en");
    let gps = r.gps.as_ref().unwrap();
    assert!((gps.lat - 38.7223).abs() < 1e-6 && (gps.lon + 9.1393).abs() < 1e-6);
    let place = gps.place.as_ref().unwrap();
    assert_eq!(
        (
            place.city.as_str(),
            place.country_code.as_str(),
            place.country.as_str()
        ),
        ("Lisbon", "PT", "Portugal")
    );
    assert!(place.distance_km < 30.0);

    // Group order and titles.
    let titles: Vec<_> = r.summary.iter().map(|g| strip_isolates(&g.title)).collect();
    assert_eq!(
        titles,
        [
            "Camera",
            "Exposure",
            "Lens",
            "Date and time",
            "Location",
            "Image",
            "File",
            "Rights and description"
        ]
    );
}

#[test]
fn lisbon_german() {
    let Some(r) = report("lisbon.jpg", "de") else {
        return;
    };
    assert_eq!(r.language, "de");
    assert_eq!(strip_isolates(&r.summary[0].title), "Kamera");
    // Dates: ICU weekday and month names, German punctuation.
    assert_eq!(v(&r, "taken"), "Dienstag, 14. Mai 2024, 18:32 (UTC+1)");
    assert_eq!(v(&r, "gps_time"), "14. Mai 2024, 17:32:10 UTC");
    // Numbers: decimal comma.
    assert_eq!(v(&r, "coordinates"), "38,7223° N, 9,1393° W");
    assert_eq!(v(&r, "aperture"), "f/2,8");
    assert_eq!(v(&r, "exposure_bias"), "+0,7 LW");
    assert_eq!(v(&r, "focal_length"), "35 mm (entspricht 52 mm)");
    assert_eq!(v(&r, "flash"), "Blitz ausgelöst, Automatik");
    assert_eq!(v(&r, "direction"), "Blickrichtung Westnordwest (287°)");
    assert_eq!(v(&r, "altitude"), "112 m über dem Meeresspiegel");
    assert_eq!(
        v(&r, "orientation"),
        "Um 90° im Uhrzeigersinn gedreht (angewendet)"
    );
    assert_eq!(v(&r, "encoding"), "8 Bit YCbCr 4:2:0, progressiv");
    assert_eq!(v(&r, "rating"), "★★★★☆ (4 von 5)");
    assert_eq!(detail(&r, "place"), "0,8 km von Lisbon");
    // GeoNames city names are not translated; the country name is (ICU), and
    // "Portugal" happens to be the same in German.
    assert_eq!(v(&r, "place"), "Lisbon, Portugal");
    assert_eq!(
        one_line_summary(&r).map(|s| strip_isolates(&s)).as_deref(),
        Some("1/250 s · f/2,8 · ISO 400 · 35 mm (entspricht 52 mm)")
    );
}

#[test]
fn buenos_aires_english_and_german() {
    let Some(data) = fixture("buenos_aires.jpg") else {
        return;
    };
    let en = report_from(&data, "buenos_aires.jpg", &Localizer::new(langid!("en")));
    let de = report_from(&data, "buenos_aires.jpg", &Localizer::new(langid!("de")));

    assert_eq!(v(&en, "camera"), "NIKON D850"); // "NIKON CORPORATION" dropped
    assert_eq!(v(&en, "shutter"), "2.5 s");
    assert_eq!(v(&en, "aperture"), "f/8");
    assert_eq!(v(&en, "iso"), "ISO 64");
    assert_eq!(v(&en, "exposure_bias"), "−1.3 EV");
    assert_eq!(v(&en, "program"), "Manual");
    assert_eq!(v(&en, "metering"), "Centre-weighted average");
    assert_eq!(v(&en, "white_balance"), "Manual (Daylight)");
    assert_eq!(v(&en, "flash"), "Flash did not fire, forced off");
    // No OffsetTime tag: the zone is worked out from the GPS clock (23:05 UTC).
    assert_eq!(v(&en, "taken"), "Thursday 2 November 2023, 20:05 (UTC−3)");
    assert_eq!(
        detail(&en, "taken"),
        "Time zone worked out from the GPS clock"
    );
    assert_eq!(v(&en, "coordinates"), "34.6132° S, 58.3772° W");
    assert_eq!(v(&en, "altitude"), "3 m below sea level");
    assert_eq!(v(&en, "direction"), "Facing east (90°)");
    assert_eq!(detail(&en, "direction"), "Relative to magnetic north");
    assert_eq!(v(&en, "colour_profile"), "Compatible with Adobe RGB (1998)");
    assert_eq!(v(&en, "place"), "Buenos Aires, Argentina");
    assert_eq!(detail(&en, "place"), "Buenos Aires F.D.");

    // German: date, numbers and the ICU country name.
    assert_eq!(
        v(&de, "taken"),
        "Donnerstag, 2. November 2023, 20:05 (UTC−3)"
    );
    assert_eq!(v(&de, "coordinates"), "34,6132° S, 58,3772° W");
    assert_eq!(v(&de, "exposure_bias"), "−1,3 LW");
    assert_eq!(v(&de, "shutter"), "2,5 s");
    assert_eq!(v(&de, "place"), "Buenos Aires, Argentinien");
    assert_eq!(
        de.gps.as_ref().unwrap().place.as_ref().unwrap().country,
        "Argentinien"
    );
}

#[test]
fn language_switch_is_just_a_reread() {
    let Some(data) = fixture("buenos_aires.jpg") else {
        return;
    };
    let seg = jpeg::scan(&data).unwrap();
    let input = seg.input(FileFacts::default());
    for (lang, want) in [
        ("en", "Argentina"),
        ("de", "Argentinien"),
        ("fr", "Argentine"),
        ("en", "Argentina"),
    ] {
        let l = Localizer::new(lang.parse().unwrap());
        let mut r = read_at(&input, &l, now());
        attach_place(&mut r, &l);
        assert_eq!(r.gps.unwrap().place.unwrap().country, want, "{lang}");
    }
}

#[test]
fn interpolated_values_are_isolated() {
    let Some(r) = report("lisbon.jpg", "en") else {
        return;
    };
    let raw = |k: &str| {
        r.summary
            .iter()
            .flat_map(|g| &g.items)
            .find(|i| i.key == k)
            .unwrap()
            .value
            .clone()
    };
    assert_eq!(
        raw("place"),
        "\u{2068}Lisbon\u{2069}, \u{2068}Portugal\u{2069}"
    );
    assert_eq!(raw("aperture"), "f/\u{2068}2.8\u{2069}");
    assert!(raw("camera").contains("Canon EOS R5"));
}

#[test]
fn ocean_is_near() {
    let Some(r) = report("ocean.jpg", "en") else {
        return;
    };
    assert_eq!(v(&r, "camera"), "Apple iPhone 15 Pro");
    assert_eq!(v(&r, "taken"), "Monday 1 July 2024, 12:00 (UTC−2)");
    assert_eq!(v(&r, "coordinates"), "40.0000° N, 40.0000° W");
    assert_eq!(v(&r, "place"), "near Ribeira Grande, Portugal");
    assert_eq!(detail(&r, "place"), "Azores · 986 km from Ribeira Grande");
    assert!(r.gps.unwrap().place.unwrap().distance_km > 30.0);
    let de = report("ocean.jpg", "de").unwrap();
    assert_eq!(v(&de, "place"), "in der Nähe von Ribeira Grande, Portugal");
}

#[test]
fn missing_date_and_profile() {
    let Some(r) = report("no_date.jpg", "en") else {
        return;
    };
    assert_eq!(v(&r, "camera"), "FUJIFILM X-T5");
    assert_eq!(val(&r, "taken"), None);
    assert_eq!(r.capture_time, None);
    assert_eq!(v(&r, "encoding"), "8-bit Greyscale, baseline");
    // No profile means sRGB: no row, and a missing value is not a warning.
    assert_eq!(val(&r, "colour_profile"), None);
    assert_eq!(r.icc_description, None);
    assert!(warnings(&r).is_empty(), "{:?}", warnings(&r));
    assert!(r.summary.iter().all(|g| g.key != "date"));
    assert_eq!(one_line_summary(&r), None);
    assert!(r.gps.is_none());
}

#[test]
fn orientations() {
    let want = [
        "Upright (no rotation needed)",
        "Mirrored horizontally (applied)",
        "Rotated 180° (applied)",
        "Mirrored vertically (applied)",
        "Mirrored horizontally and rotated 90° counter-clockwise (applied)",
        "Rotated 90° clockwise (applied)",
        "Mirrored horizontally and rotated 90° clockwise (applied)",
        "Rotated 90° counter-clockwise (applied)",
    ];
    for (i, want) in want.iter().enumerate() {
        let o = i + 1;
        let Some(r) = report(&format!("orientation_{o}.jpg"), "en") else {
            return;
        };
        assert_eq!(r.orientation, Some(o as u8));
        assert_eq!(v(&r, "orientation"), *want);
        assert_eq!(v(&r, "taken"), "Monday 1 January 2024, 10:00");
        // A missing time zone is simply not mentioned.
        assert_eq!(detail(&r, "taken"), "");
    }
}

#[test]
fn orientation_conflict_warns() {
    let Some(r) = report("orientation_conflict.jpg", "en") else {
        return;
    };
    assert_eq!(r.orientation, Some(6));
    assert_eq!(v(&r, "orientation"), "Rotated 90° clockwise (applied)");
    assert!(warnings(&r).contains(
        &"EXIF and XMP disagree about orientation (EXIF: Rotated 90° clockwise, XMP: Upright). The EXIF value is used.".to_owned()
    ));
}

#[test]
fn c2pa_marker() {
    let Some(data) = fixture("no_date.jpg") else {
        return;
    };
    // Splice an APP11 JUMBF segment carrying a "c2pa" box label after SOI.
    let payload: &[u8] =
        b"JP\x00\x01\x00\x00\x00\x01\x00\x00\x00\x20jumbjumd\x63\x32\x70\x61c2pa\x00";
    let len = (payload.len() + 2) as u16;
    let mut jpg = data[..2].to_vec();
    jpg.extend_from_slice(&[0xFF, 0xEB]);
    jpg.extend_from_slice(&len.to_be_bytes());
    jpg.extend_from_slice(payload);
    jpg.extend_from_slice(&data[2..]);
    let r = report_from(&jpg, "c2pa.jpg", &Localizer::english());
    assert_eq!(v(&r, "c2pa"), "Present (not verified)");
    assert_eq!(
        detail(&r, "c2pa"),
        "This file carries a C2PA manifest; its signature has not been checked."
    );
}

/// Little-endian TIFF with Make, an unknown 300-character ASCII tag and an
/// unknown 1000-byte UNDEFINED tag.
fn odd_tiff() -> Vec<u8> {
    let make = b"Test\0".to_vec();
    let mut long = "x".repeat(300).into_bytes();
    long.push(0);
    let blob = vec![0xAB_u8; 1000];
    let entries: [(u16, u16, &[u8]); 3] =
        [(0x010F, 2, &make), (0xC000, 2, &long), (0xC001, 7, &blob)];
    let mut t = b"II*\0\x08\0\0\0".to_vec();
    t.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut data_off = 8 + 2 + 12 * entries.len() as u32 + 4;
    let mut data = Vec::new();
    for (tag, ty, bytes) in entries {
        t.extend_from_slice(&tag.to_le_bytes());
        t.extend_from_slice(&ty.to_le_bytes());
        t.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        t.extend_from_slice(&data_off.to_le_bytes());
        data.extend_from_slice(bytes);
        data_off += bytes.len() as u32;
    }
    t.extend_from_slice(&0u32.to_le_bytes());
    t.extend_from_slice(&data);
    t
}

#[test]
fn all_fields_truncate_and_summarise_blobs() {
    let tiff = odd_tiff();
    let input = MetadataInput {
        exif_tiff: Some(&tiff),
        xmp: None,
        icc: None,
        has_c2pa: false,
        file: FileFacts::default(),
        medical: None,
        science: None,
        drawing: None,
        spectrum: None,
        mesh: None,
        points: None,
        splats: None,
        rti: None,
        structure: None,
        font: None,
        book: None,
        document: None,
        volume: None,
        geo: None,
        header: None,
        neuro: None,
        iptc: None,
        tags: Vec::new(),
    };
    let r = read_at(&input, &Localizer::english(), now());
    let fields: Vec<_> = r.all_fields.iter().flat_map(|g| &g.fields).collect();
    let unknown = fields
        .iter()
        .find(|f| f.tag.contains("0xC000"))
        .expect("unknown tag listed");
    assert!(
        strip_isolates(&unknown.tag).starts_with("Unknown tag 0xC000"),
        "{}",
        unknown.tag
    );
    assert!(unknown.display.ends_with('…'), "{}", unknown.display);
    assert!(unknown.display.chars().count() <= 162);
    // "Copy value" copies the whole stored text, not the shortened display.
    let full = unknown
        .full
        .as_deref()
        .expect("truncated value keeps the full text");
    assert_eq!(full, "x".repeat(300));
    assert_eq!(unknown.copy_text(), full);
    assert!(unknown.raw.chars().count() <= 162, "raw is shortened too");
    let blob = fields
        .iter()
        .find(|f| f.tag.contains("0xC001"))
        .expect("blob listed");
    assert_eq!(strip_isolates(&blob.display), "(1,000 bytes)");
    // Short values are not duplicated; copying them copies the display text.
    assert_eq!(blob.full, None);
    assert_eq!(blob.copy_text(), blob.display);
    assert_eq!(v(&r, "camera"), "Test");
}

#[test]
fn geonames_attribution_is_localised() {
    assert!(GEONAMES_ATTRIBUTION.contains("CC BY 4.0"));
    let de = Localizer::new(langid!("de"));
    assert!(de.text("geonames-attribution").contains("GeoNames"));
    assert_ne!(
        de.text("geonames-attribution"),
        Localizer::english().text("geonames-attribution")
    );
}

/// Writes a little-endian TIFF: IFD0 entries, and an Exif sub-IFD when `exif` is not empty.
/// Each entry is (tag, type, count, value bytes).
fn tiff(ifd0: &[(u16, u16, u32, Vec<u8>)], exif: &[(u16, u16, u32, Vec<u8>)]) -> Vec<u8> {
    fn ifd(
        entries: &[(u16, u16, u32, Vec<u8>)],
        at: u32,
        next_ptr: Option<u32>,
    ) -> (Vec<u8>, Vec<u8>) {
        let n = entries.len() + next_ptr.is_some() as usize;
        let mut dir = (n as u16).to_le_bytes().to_vec();
        let mut data = Vec::new();
        let mut data_off = at + 2 + 12 * n as u32 + 4;
        let mut all: Vec<(u16, u16, u32, Vec<u8>)> = entries.to_vec();
        if let Some(p) = next_ptr {
            all.push((0x8769, 4, 1, p.to_le_bytes().to_vec()));
        }
        all.sort_by_key(|e| e.0);
        for (tag, ty, count, bytes) in &all {
            dir.extend_from_slice(&tag.to_le_bytes());
            dir.extend_from_slice(&ty.to_le_bytes());
            dir.extend_from_slice(&count.to_le_bytes());
            if bytes.len() <= 4 {
                let mut v = bytes.clone();
                v.resize(4, 0);
                dir.extend_from_slice(&v);
            } else {
                dir.extend_from_slice(&data_off.to_le_bytes());
                data.extend_from_slice(bytes);
                data_off += bytes.len() as u32;
            }
        }
        dir.extend_from_slice(&0u32.to_le_bytes());
        (dir, data)
    }
    let size = |e: &[(u16, u16, u32, Vec<u8>)], extra: usize| {
        let n = e.len() + extra;
        2 + 12 * n
            + 4
            + e.iter()
                .filter(|x| x.3.len() > 4)
                .map(|x| x.3.len())
                .sum::<usize>()
    };
    let exif_at = 8 + size(ifd0, (!exif.is_empty()) as usize) as u32;
    let (d0, x0) = ifd(ifd0, 8, (!exif.is_empty()).then_some(exif_at));
    let mut t = b"II*\0\x08\0\0\0".to_vec();
    t.extend(d0);
    t.extend(x0);
    if !exif.is_empty() {
        let (d1, x1) = ifd(exif, exif_at, None);
        t.extend(d1);
        t.extend(x1);
    }
    t
}

fn ascii(s: &str) -> (u16, u32, Vec<u8>) {
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    (2, b.len() as u32, b)
}
fn rational(n: u32, d: u32) -> Vec<u8> {
    [n.to_le_bytes(), d.to_le_bytes()].concat()
}

/// Sparse EXIF: no date, no profile, a blank software string, an empty lens model, zero focal
/// length and aperture, "unknown" codes, rating 0 and padding. Nothing of that may show up as
/// a row, a group or a placeholder; a real 0 EV compensation stays.
#[test]
fn sparse_exif_shows_no_empty_rows_groups_or_placeholders() {
    let a = |tag, s: &str| {
        let (ty, n, b) = ascii(s);
        (tag, ty, n, b)
    };
    let ifd0 = vec![
        a(0x010F, "Test"),
        a(0x0110, "Model 1"),
        a(0x0131, "   "),                            // Software: blank
        (0x4746, 3, 1, 0u16.to_le_bytes().to_vec()), // Rating 0
    ];
    let exif = vec![
        (0x829D, 5, 1, rational(0, 1)),              // FNumber 0
        (0x920A, 5, 1, rational(0, 1)),              // FocalLength 0
        (0x9204, 10, 1, rational(0, 1)),             // ExposureBiasValue 0 EV: real data
        (0x9206, 5, 1, rational(0, 1)),              // SubjectDistance 0: unknown
        (0x8822, 3, 1, 0u16.to_le_bytes().to_vec()), // ExposureProgram 0: not defined
        (0x9207, 3, 1, 0u16.to_le_bytes().to_vec()), // MeteringMode 0: unknown
        (0xA405, 3, 1, 0u16.to_le_bytes().to_vec()), // FocalLengthIn35mmFilm 0
        a(0xA434, ""),                               // LensModel: empty
        (0xEA1C, 7, 8, vec![0; 8]),                  // Padding
    ];
    let t = tiff(&ifd0, &exif);
    let input = MetadataInput {
        exif_tiff: Some(&t),
        xmp: None,
        icc: None,
        has_c2pa: false,
        file: FileFacts::default(),
        medical: None,
        science: None,
        drawing: None,
        spectrum: None,
        mesh: None,
        points: None,
        splats: None,
        rti: None,
        structure: None,
        font: None,
        book: None,
        document: None,
        volume: None,
        geo: None,
        header: None,
        neuro: None,
        iptc: None,
        tags: Vec::new(),
    };
    for lang in ["en", "de"] {
        let l = Localizer::new(lang.parse().unwrap());
        let r = read_at(&input, &l, now());
        let keys: Vec<_> = r
            .summary
            .iter()
            .flat_map(|g| &g.items)
            .map(|i| i.key)
            .collect();
        assert_eq!(keys, ["camera", "exposure_bias"], "{lang}");
        assert!(
            r.summary
                .iter()
                .all(|g| !g.items.is_empty() && !g.title.trim().is_empty())
        );
        assert!(r.summary.iter().flat_map(|g| &g.items).all(|i| {
            !strip_isolates(&i.value).trim().is_empty()
                && i.detail
                    .as_deref()
                    .is_none_or(|d| !strip_isolates(d).trim().is_empty())
        }));
        assert!(r.warnings.is_empty(), "{lang}: {:?}", r.warnings);
        assert_eq!(r.rating, None);
        if lang == "en" {
            assert_eq!(v(&r, "exposure_bias"), "0 EV");
        }
        // All fields: the uninformative ones are flagged (hidden in the info panel).
        let fields: Vec<_> = r.all_fields.iter().flat_map(|g| &g.fields).collect();
        let empty = |needle: &str| {
            fields
                .iter()
                .find(|f| f.tag.contains(needle))
                .unwrap_or_else(|| panic!("{needle} listed"))
                .empty
        };
        for tag in [
            "Software",
            "FNumber",
            "FocalLength",
            "SubjectDistance",
            "ExposureProgram",
            "MeteringMode",
            "FocalLengthIn35mmFilm",
            "LensModel",
            "Padding",
        ] {
            assert!(empty(tag), "{lang}: {tag} should count as empty");
        }
        for tag in ["Make", "Model", "ExposureBiasValue"] {
            assert!(!empty(tag), "{lang}: {tag} is real data");
        }
        // No shown text is a placeholder for something missing.
        let shown: Vec<String> = r
            .summary
            .iter()
            .flat_map(|g| &g.items)
            .flat_map(|i| [i.value.clone(), i.detail.clone().unwrap_or_default()])
            .chain(
                fields
                    .iter()
                    .filter(|f| !f.empty)
                    .map(|f| f.display.clone()),
            )
            .map(|s| strip_isolates(&s).to_lowercase())
            .collect();
        for bad in [
            "unknown",
            "not defined",
            "no rating",
            "not recorded",
            "srgb assumed",
            "unused",
        ] {
            assert!(
                shown.iter().all(|s| !s.contains(bad)),
                "{lang}: {bad:?} in {shown:?}"
            );
        }
    }
}

// ----- Tags beyond kamadak-exif's tables (synthetic EXIF only) ----------------

type Entry = (u16, u16, u32, Vec<u8>);

fn synth(ifd0: &[Entry], exif: &[Entry], lang: &str) -> MetadataReport {
    let t = tiff(ifd0, exif);
    let input = MetadataInput {
        exif_tiff: Some(&t),
        ..Default::default()
    };
    read_at(&input, &Localizer::new(lang.parse().unwrap()), now())
}

/// The All-fields row whose tag is exactly `tag`, as (group title, display, raw, empty).
fn row(r: &MetadataReport, tag: &str) -> (String, String, String, bool) {
    r.all_fields
        .iter()
        .flat_map(|g| g.fields.iter().map(move |f| (g, f)))
        .find(|(_, f)| strip_isolates(&f.tag) == tag)
        .map(|(g, f)| {
            (
                g.title.clone(),
                strip_isolates(&f.display),
                strip_isolates(&f.raw),
                f.empty,
            )
        })
        .unwrap_or_else(|| panic!("no field {tag}"))
}

fn unknown_tags(r: &MetadataReport) -> Vec<String> {
    r.all_fields
        .iter()
        .flat_map(|g| &g.fields)
        .map(|f| strip_isolates(&f.tag))
        .filter(|t| t.starts_with("Unknown"))
        .collect()
}

fn srational(n: i32, d: i32) -> Vec<u8> {
    [n.to_le_bytes(), d.to_le_bytes()].concat()
}

fn utf16(s: &str) -> Vec<u8> {
    let mut b: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
    b.extend([0, 0]);
    b
}

#[test]
fn tiff_ep_tags_in_ifd0_and_tiff_tags_in_the_exif_ifd_are_named_and_used() {
    // TIFF/EP layout: exposure facts in IFD0, some as signed types; a TIFF tag misplaced in
    // the Exif IFD.
    let ifd0 = [
        (0x829A, 10, 1, srational(1, 125)), // ExposureTime, SRATIONAL
        (0x829D, 10, 1, srational(28, 10)), // FNumber, SRATIONAL
        (0x8827, 8, 1, 200i16.to_le_bytes().to_vec()), // ISOSpeedRatings, SSHORT
        (0x9207, 3, 1, 5u16.to_le_bytes().to_vec()), // MeteringMode
    ];
    let exif = [(0x0100, 4, 1, 4000u32.to_le_bytes().to_vec())]; // ImageWidth
    let r = synth(&ifd0, &exif, "en");
    assert_eq!(unknown_tags(&r), Vec::<String>::new());
    assert_eq!(v(&r, "shutter"), "1/125 s");
    assert_eq!(v(&r, "aperture"), "f/2.8");
    assert_eq!(v(&r, "iso"), "ISO 200");
    assert_eq!(v(&r, "metering"), "Matrix (evaluative)");
    assert_eq!(row(&r, "FNumber").1, "f/2.8");
    assert_eq!(row(&r, "ExposureTime").1, "1/125 s");
    assert_eq!(row(&r, "ImageWidth").1, "4000 pixels");
}

#[test]
fn apex_values_stand_in_for_missing_exposure_time_and_f_number() {
    let exif = [
        (0x9201, 10, 1, srational(7, 1)), // ShutterSpeedValue Tv 7: 1/128 s
        (0x9202, 5, 1, rational(4, 1)),   // ApertureValue Av 4: f/4
    ];
    let r = synth(&[], &exif, "en");
    assert_eq!(v(&r, "shutter"), "1/128 s");
    assert_eq!(v(&r, "aperture"), "f/4");
}

#[test]
fn windows_xp_tags_decode_as_utf16_and_feed_the_summary() {
    let x = |tag: u16, s: &str| {
        let b = utf16(s);
        (tag, 1u16, b.len() as u32, b)
    };
    let ifd0 = [
        x(0x9C9B, "Synthetic title ü"),
        x(0x9C9C, "A comment"),
        x(0x9C9D, "Test Author"),
        x(0x9C9E, "alpha; beta;"),
        x(0x9C9F, ""),
    ];
    let r = synth(&ifd0, &[], "en");
    assert_eq!(v(&r, "title"), "Synthetic title ü");
    assert_eq!(v(&r, "description"), "A comment");
    assert_eq!(v(&r, "creator"), "Test Author");
    assert_eq!(v(&r, "keywords"), "alpha, beta");
    assert_eq!(row(&r, "XPTitle").1, "Synthetic title ü");
    assert!(row(&r, "XPSubject").3, "an empty XPSubject is hidden");
}

#[test]
fn exif_3_utf8_type_is_text_inline_and_by_offset() {
    let utf8 = |tag: u16, s: &str| {
        let mut b = s.as_bytes().to_vec();
        b.push(0);
        (tag, 129u16, b.len() as u32, b)
    };
    let ifd0 = [utf8(0x010E, "Abc")]; // ImageDescription, 4 bytes: inline
    let exif = [
        utf8(0xA436, "Grüße aus Köln"), // ImageTitle: by offset
        utf8(0xA437, "Ŝynthetic Photographer"),
    ];
    let r = synth(&ifd0, &exif, "en");
    assert_eq!(unknown_tags(&r), Vec::<String>::new());
    assert_eq!(v(&r, "title"), "Grüße aus Köln");
    assert_eq!(v(&r, "description"), "Abc");
    assert_eq!(v(&r, "creator"), "Ŝynthetic Photographer");
    let (_, display, raw, empty) = row(&r, "ImageTitle");
    assert_eq!(
        (display.as_str(), raw.as_str(), empty),
        ("Grüße aus Köln", "Grüße aus Köln", false)
    );
    assert_eq!(row(&r, "ImageDescription").1, "Abc");
}

#[test]
fn tiff_and_dng_structure_tags_are_named_and_decoded() {
    let shorts = |v: &[u16]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let matrix: Vec<u8> = [
        (1, 2),
        (-1, 4),
        (0, 1),
        (3, 4),
        (1, 1),
        (0, 1),
        (0, 1),
        (0, 1),
        (1, 1),
    ]
    .iter()
    .flat_map(|&(n, d)| srational(n, d))
    .collect();
    let ifd0 = [
        (0x00FE, 4, 1, 1u32.to_le_bytes().to_vec()), // NewSubfileType: reduced resolution
        (0x013D, 3, 1, shorts(&[2])),                // Predictor
        (0x0153, 3, 3, shorts(&[1, 1, 1])),          // SampleFormat per sample
        (0x4746, 3, 1, shorts(&[3])),                // Rating
        (0xC612, 1, 4, vec![1, 4, 0, 0]),            // DNGVersion
        (0xC621, 10, 9, matrix),                     // ColorMatrix1
        (0xC65A, 3, 1, shorts(&[21])),               // CalibrationIlluminant1: D65
        (0xC4A5, 7, 32, vec![0x50; 32]),             // PrintIM: internal
        (0xEA1D, 9, 1, 0i32.to_le_bytes().to_vec()), // OffsetSchema: internal
        (0xBEEF, 3, 1, shorts(&[7])),                // genuinely unknown
    ];
    let r = synth(&ifd0, &[], "en");
    assert_eq!(unknown_tags(&r), ["Unknown tag 0xBEEF (IFD)"]);
    assert_eq!(row(&r, "NewSubfileType").1, "Reduced-resolution image");
    assert_eq!(row(&r, "Predictor").1, "Horizontal differencing");
    assert_eq!(row(&r, "SampleFormat").1, "Unsigned integer");
    assert_eq!(row(&r, "DNGVersion").1, "1.4.0.0");
    assert_eq!(
        row(&r, "ColorMatrix1").1,
        "0.5, -0.25, 0, 0.75, 1, 0, 0, 0, 1"
    );
    assert!(row(&r, "CalibrationIlluminant1").1.contains("D65"));
    assert_eq!(row(&r, "Rating").1, "★★★☆☆ (3 of 5)");
    assert_eq!(r.rating, Some(3));
    for internal in ["PrintIM", "OffsetSchema"] {
        assert!(row(&r, internal).3, "{internal} is hidden");
    }
    assert!(!row(&r, "DNGVersion").3);
    // Coded values are localised.
    let de = synth(&ifd0, &[], "de");
    assert_eq!(row(&de, "SampleFormat").1, "Ganzzahl ohne Vorzeichen");
}

#[test]
fn strip_location_removes_every_gps_trace() {
    for lang in ["en", "de"] {
        let Some(mut r) = report("lisbon.jpg", lang) else {
            return;
        };
        let l = Localizer::new(lang.parse().unwrap());
        let gps_group = l.text("fields-gps");
        assert!(r.all_fields.iter().any(|g| g.title == gps_group), "{lang}");
        r.strip_location(&l);
        assert!(r.gps.is_none());
        assert!(r.summary.iter().all(|g| g.key != "location"));
        assert!(r.all_fields.iter().all(|g| g.title != gps_group), "{lang}");
        let left: Vec<_> = r
            .all_fields
            .iter()
            .flat_map(|g| &g.fields)
            .filter(|f| f.tag.contains("GPS"))
            .map(|f| &f.tag)
            .collect();
        assert!(left.is_empty(), "{lang}: {left:?}");
    }
}

/// An ASCII value ends at its first NUL: Samsung's raw files pad Make to 20 bytes with
/// whatever was in the buffer.
#[test]
fn ascii_values_end_at_the_first_nul() {
    let make = b"SAMSUNG\0\x01\x02junk\0\0\0\0\0\0\0\0".to_vec();
    let model = b"NX1\0\0\0\0\0".to_vec();
    let ifd0 = [
        (0x010F, 2, make.len() as u32, make),
        (0x0110, 2, model.len() as u32, model),
    ];
    let lens = b"\0\0 Lens \0x".to_vec();
    let exif = [(0xA434, 2, lens.len() as u32, lens)];
    let r = synth(&ifd0, &exif, "en");
    assert_eq!(row(&r, "Make").1, "\"SAMSUNG\"");
    assert!(!row(&r, "Make").2.contains("junk"));
    assert_eq!(row(&r, "Model").1, "\"NX1\"");
    assert_eq!(v(&r, "camera"), "SAMSUNG NX1");
    assert_eq!(v(&r, "lens"), "Lens");
}

/// Serial numbers and owner names identify a person or a device, not the picture: like a
/// maker note's serials they are not listed at all, from Exif or XMP. ImageUniqueID stays.
#[test]
fn serial_numbers_and_owner_names_are_not_listed() {
    let a = |tag, s: &str| {
        let (ty, n, b) = ascii(s);
        (tag, ty, n, b)
    };
    let ifd0 = [
        a(0x010F, "Test"),
        a(0xC62F, "DNG-SERIAL-1"), // CameraSerialNumber
    ];
    let exif = [
        a(0xA420, "0123456789abcdef0123456789abcdef"), // ImageUniqueID
        a(0xA430, "Synthetic Owner"),                  // CameraOwnerName
        a(0xA431, "BODY-SERIAL-2"),                    // BodySerialNumber
        a(0xA434, "Test Lens"),
        a(0xA435, "LENS-SERIAL-3"), // LensSerialNumber
    ];
    let t = tiff(&ifd0, &exif);
    let xmp = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:aux="http://ns.adobe.com/exif/1.0/aux/" xmlns:exifEX="http://cipa.jp/exif/1.0/" xmlns:xmp="http://ns.adobe.com/xap/1.0/" aux:SerialNumber="XMP-SERIAL-4" aux:LensSerialNumber="XMP-SERIAL-5" aux:OwnerName="XMP Owner" exifEX:BodySerialNumber="XMP-SERIAL-6" exifEX:LensSerialNumber="XMP-SERIAL-7" exifEX:CameraOwnerName="XMP Owner 2" xmp:CreatorTool="Synthetic Tool"/></rdf:RDF></x:xmpmeta>"#;
    let input = MetadataInput {
        exif_tiff: Some(&t),
        xmp: Some(xmp),
        ..Default::default()
    };
    let r = read_at(&input, &Localizer::english(), now());
    let fields: Vec<(String, String)> = r
        .all_fields
        .iter()
        .flat_map(|g| &g.fields)
        .map(|f| (strip_isolates(&f.tag), strip_isolates(&f.display)))
        .collect();
    for (tag, display) in &fields {
        assert!(
            !display.contains("SERIAL") && !display.contains("Owner"),
            "{tag}: {display}"
        );
    }
    assert_eq!(
        row(&r, "ImageUniqueID").1,
        "\"0123456789abcdef0123456789abcdef\""
    );
    assert_eq!(row(&r, "LensModel").1, "\"Test Lens\"");
    assert!(
        fields.iter().any(|(_, d)| d == "Synthetic Tool"),
        "other XMP stays: {fields:?}"
    );
}

/// A zoom's widest apertures when LensSpecification has none (as Canon writes it, with zero
/// apertures): from the lens name, when its focal range is the lens's and the widest
/// aperture at the focal length used (MaxApertureValue) lies within them; a prime's from
/// MaxApertureValue.
#[test]
fn zoom_apertures_from_the_lens_name() {
    let spec = |a: u32, b: u32| {
        let v = [
            rational(a, 1),
            rational(b, 1),
            rational(0, 1),
            rational(0, 1),
        ]
        .concat();
        (0xA432, 5, 4, v)
    };
    let apex = |av: u32| (0x9205, 5, 1, rational(av, 100));
    let ifd0 = [(0x010F, 2, 6, b"Canon\0".to_vec())];
    for (name, min, max, now, want) in [
        (
            "EF-S17-85mm f/4-5.6 IS USM",
            17,
            85,
            Some(464),
            "17–85 mm f/4–5.6",
        ),
        ("RF24-70mm F2.8 L IS USM", 24, 70, None, "24–70 mm f/2.8"),
        (
            "XF18-55mmF2.8-4 R LM OIS",
            18,
            55,
            Some(300),
            "18–55 mm f/2.8–4",
        ),
        // A teleconverter: not the name's focal range.
        (
            "EF70-200mm f/2.8L IS II USM",
            98,
            280,
            Some(300),
            "98–280 mm",
        ),
        // The aperture used is past the name's.
        ("EF24-105mm f/4L IS USM", 24, 105, Some(500), "24–105 mm"),
        // A prime: the widest aperture used is the lens's.
        ("Samyang AF 35mm", 35, 35, Some(200), "35 mm f/2"),
        ("Lens", 18, 55, Some(300), "18–55 mm"),
    ] {
        let (ty, n, b) = ascii(name);
        let mut exif = vec![spec(min, max), (0xA434, ty, n, b)];
        exif.extend(now.map(apex));
        exif.sort_by_key(|e| e.0);
        let r = synth(&ifd0, &exif, "en");
        assert_eq!(detail(&r, "lens"), want, "{name}");
    }
}

/// Without LensSpecification (as in Panasonic's DNGs) the range the lens name states, when
/// the focal length used lies within it; its apertures as above.
#[test]
fn lens_range_from_the_lens_name() {
    let apex = |av: u32| (0x9205, 5, 1, rational(av, 100));
    let focal = |mm: u32| (0x920A, 5, 1, rational(mm, 1));
    let ifd0 = [(0x010F, 2, 10, b"Panasonic\0".to_vec())];
    for (name, used, now, want) in [
        ("LEICA DG 12-60/F2.8-4.0", 27, Some(370), "12–60 mm f/2.8–4"),
        ("LUMIX S 24-105/F4", 105, Some(400), "24–105 mm f/4"),
        ("LUMIX G 25/F1.7", 25, Some(153), "25 mm f/1.7"),
        ("LEICA DG SUMMILUX 15/F1.7", 15, None, "15 mm f/1.7"),
        // The aperture used is past the name's.
        ("LUMIX S 20-60/F3.5-5.6", 40, Some(800), "20–60 mm"),
        // Through a converter, or another lens: not the name's range.
        ("LUMIX G VARIO 12-60/F3.5-5.6", 120, Some(500), ""),
        ("Lens", 50, Some(300), ""),
    ] {
        let (ty, n, b) = ascii(name);
        let mut exif = vec![focal(used), (0xA434, ty, n, b)];
        exif.extend(now.map(apex));
        exif.sort_by_key(|e| e.0);
        let r = synth(&ifd0, &exif, "en");
        assert_eq!(detail(&r, "lens"), want, "{name}");
    }
}

/// A drawn file (a PDF, a drawing, a model): the program that made it is the File group's
/// application, not camera software, and the picture's sample coding is not shown.
#[test]
fn drawn_files_name_their_application_not_a_camera() {
    let xmp = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:CreatorTool="Synthetic Writer"/></rdf:RDF></x:xmpmeta>"#;
    let file = |drawn| FileFacts {
        format_name: Some("PDF".into()),
        bit_depth: Some(8),
        colour_model: Some("RGB".into()),
        drawn,
        ..Default::default()
    };
    let read = |drawn| {
        let input = MetadataInput {
            xmp: Some(xmp),
            file: file(drawn),
            ..Default::default()
        };
        read_at(&input, &Localizer::english(), now())
    };
    let group = |r: &MetadataReport, key: &str| {
        r.summary
            .iter()
            .find(|g| g.items.iter().any(|i| i.key == key))
            .map(|g| g.key)
    };
    let photo = read(false);
    assert_eq!(group(&photo, "software"), Some("camera"));
    assert!(val(&photo, "encoding").is_some());
    let drawn = read(true);
    assert_eq!(group(&drawn, "software"), Some("file"));
    assert_eq!(v(&drawn, "software"), "Synthetic Writer");
    assert!(drawn.summary.iter().all(|g| g.key != "camera"));
    assert_eq!(val(&drawn, "encoding"), None);
}

/// A DJI drone photo's XMP: the gimbal and aircraft attitude and the height above take-off; a
/// thermal camera's R-JPEG says why its temperatures are not shown.
#[test]
fn dji_drone_group() {
    let xmp = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:drone-dji="http://www.dji.com/drone-dji/1.0/" drone-dji:RelativeAltitude="+52.30" drone-dji:GimbalRollDegree="+0.00" drone-dji:GimbalYawDegree="-158.30" drone-dji:GimbalPitchDegree="-90.00" drone-dji:FlightRollDegree="+1.10" drone-dji:FlightYawDegree="-160.20" drone-dji:FlightPitchDegree="-3.40" drone-dji:ImageSource="InfraredCamera"/></rdf:RDF></x:xmpmeta>"#;
    let input = MetadataInput {
        xmp: Some(xmp),
        ..Default::default()
    };
    let r = read_at(&input, &Localizer::english(), now());
    assert_eq!(v(&r, "gimbal"), "yaw -158.3°, pitch -90.0°, roll 0.0°");
    assert_eq!(v(&r, "flight"), "yaw -160.2°, pitch -3.4°, roll 1.1°");
    assert!(v(&r, "relative_altitude").contains("52"));
    assert!(v(&r, "dji_thermal").contains("licence"));
    // A visible-camera photo has no thermal line.
    let visible = String::from_utf8(xmp.to_vec())
        .unwrap()
        .replace("InfraredCamera", "WideCamera");
    let input = MetadataInput {
        xmp: Some(visible.as_bytes()),
        ..Default::default()
    };
    let r = read_at(&input, &Localizer::english(), now());
    assert!(val(&r, "dji_thermal").is_none());
    assert!(val(&r, "gimbal").is_some());
}
