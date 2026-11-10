//! Metadata outside the plain Exif/XMP path, read into the info panel's report: the Fujifilm
//! maker note of a RAF, IPTC-IIM and comments of a JPEG, IPTC of a TIFF, PNG text chunks, a
//! GIF's comment and XMP. The fixtures are built here; the expected values are exiftool's
//! (13.50) on the same bytes, written out with `SPECULAR_FIXTURE_DIR=<dir>` set.

use specular_metadata::*;

/// Writes `bytes` as `name` under `SPECULAR_FIXTURE_DIR`, when set (for the oracle).
fn keep(name: &str, bytes: &[u8]) {
    if let Some(dir) = std::env::var_os("SPECULAR_FIXTURE_DIR") {
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}

/// An IFD entry: tag, type, count, value bytes (little-endian), and a fixed offset for a
/// value that lies elsewhere.
type Ent = (u16, u16, u32, Vec<u8>, Option<u32>);

/// A little-endian IFD laid at offset `at` (offsets count from `at`'s origin), its values after
/// it.
fn ifd_at(at: u32, mut entries: Vec<Ent>) -> Vec<u8> {
    entries.sort_by_key(|e| e.0);
    let mut data = at + 2 + 12 * entries.len() as u32 + 4;
    let (mut b, mut tail) = ((entries.len() as u16).to_le_bytes().to_vec(), Vec::new());
    for (tag, typ, count, mut v, fixed) in entries {
        b.extend(tag.to_le_bytes());
        b.extend(typ.to_le_bytes());
        b.extend(count.to_le_bytes());
        if let Some(o) = fixed {
            b.extend(o.to_le_bytes());
        } else if v.len() <= 4 {
            v.resize(4, 0);
            b.extend(v);
        } else {
            b.extend(data.to_le_bytes());
            if v.len() % 2 == 1 {
                v.push(0);
            }
            data += v.len() as u32;
            tail.extend(v);
        }
    }
    b.extend([0; 4]);
    b.extend(tail);
    b
}

fn ascii(tag: u16, s: &str) -> Ent {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    (tag, 2, v.len() as u32, v, None)
}
fn short(tag: u16, v: u16) -> Ent {
    (tag, 3, 1, v.to_le_bytes().to_vec(), None)
}
fn long(tag: u16, typ: u16, v: i32) -> Ent {
    (tag, typ, 1, v.to_le_bytes().to_vec(), None)
}
fn rational(tag: u16, n: u32, d: u32) -> Ent {
    (tag, 5, 1, [n.to_le_bytes(), d.to_le_bytes()].concat(), None)
}

/// A JPEG of `segments` (marker, payload) and a tiny frame header.
fn jpeg(segments: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut f = vec![0xFF, 0xD8];
    for (m, p) in segments {
        f.extend([0xFF, *m]);
        f.extend(((p.len() + 2) as u16).to_be_bytes());
        f.extend(p);
    }
    // SOF0: 8-bit, 8 × 8, one component.
    f.extend([0xFF, 0xC0, 0, 11, 8, 0, 8, 0, 8, 1, 1, 0x11, 0]);
    f.extend([0xFF, 0xD9]);
    f
}

/// A Fujifilm camera's Exif (TIFF structure) with its maker note.
fn fuji_exif() -> Vec<u8> {
    let mut note = b"FUJIFILM".to_vec();
    note.extend(12u32.to_le_bytes());
    note.extend(ifd_at(
        12,
        vec![
            (0x0000, 7, 4, b"0130".to_vec(), None),
            ascii(0x0010, "FF02SERIAL-77"),
            short(0x1001, 6),
            short(0x1002, 1),
            short(0x1003, 384),
            short(0x100E, 640),
            long(0x100F, 9, -2000),
            long(0x102D, 4, 0x4002),
            long(0x1040, 9, -32),
            long(0x1041, 9, 16),
            long(0x1047, 9, 32),
            long(0x1048, 9, 64),
            short(0x104C, 32),
            long(0x104E, 9, 0),
            short(0x1050, 1),
            long(0x1103, 4, 1),
            short(0x1401, 1536),
            short(0x1402, 1),
            short(0x1403, 400),
            (
                0x1422,
                3,
                3,
                [2u16, 1, 0].iter().flat_map(|v| v.to_le_bytes()).collect(),
                None,
            ),
            long(0x1431, 4, 4),
        ],
    ));
    let mut f = b"II*\0\0\0\0\0".to_vec();
    let note_at = f.len() as u32;
    let note_len = note.len() as u32;
    f.extend(note);
    let exif_at = f.len() as u32;
    f.extend(ifd_at(
        exif_at,
        vec![
            rational(0x829A, 1, 250),
            rational(0x829D, 56, 10),
            short(0x8827, 400),
            ascii(0x9003, "2024:05:06 07:08:09"),
            rational(0x920A, 35, 1),
            ascii(0xA434, "XF35mmF2 R WR"),
            (0x927C, 7, note_len, vec![], Some(note_at)),
        ],
    ));
    let ifd0_at = f.len() as u32;
    f.extend(ifd_at(
        ifd0_at,
        vec![
            ascii(0x010F, "FUJIFILM"),
            ascii(0x0110, "X-T3"),
            long(0x8769, 4, exif_at as i32),
        ],
    ));
    f[4..8].copy_from_slice(&ifd0_at.to_le_bytes());
    f
}

/// A RAF: its header, the JPEG preview (with the Exif) at 148.
fn raf() -> Vec<u8> {
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend(fuji_exif());
    let preview = jpeg(&[(0xE1, app1)]);
    let mut f = b"FUJIFILMCCD-RAW 0201FF383501X-T3".to_vec();
    f.resize(60, 0);
    f.extend(b"0100");
    f.resize(84, 0);
    f.extend(148u32.to_be_bytes());
    f.extend((preview.len() as u32).to_be_bytes());
    f.resize(148, 0);
    f.extend(preview);
    f
}

fn item(r: &MetadataReport, group: &str, key: &str) -> Option<String> {
    r.summary
        .iter()
        .find(|g| g.key == group)
        .and_then(|g| g.items.iter().find(|i| i.key == key))
        .map(|i| strip_isolates(&i.value))
}

/// The RAF's Fujifilm maker note reaches the summary. exiftool on `fuji.raf`: Make FUJIFILM,
/// Model X-T3, LensModel "XF35mmF2 R WR", ExposureTime 1/250, FNumber 5.6, ISO 400,
/// FilmMode "Classic Chrome", DevelopmentDynamicRange 400, HighlightTone "-1 (medium soft)",
/// ShadowTone "+2 (hard)", Saturation "-1 (medium low)", Sharpness "+4 (hardest)",
/// NoiseReduction "-1 (medium weak)", Clarity -2, GrainEffectRoughness Weak, GrainEffectSize
/// Large, ColorChromeEffect Strong, ColorChromeFXBlue Off, WhiteBalance "Auto (white
/// priority)", FocusMode2 AF-C, DriveMode "Continuous Low", ShutterType Electronic,
/// ImageStabilization "Sensor-shift; On (mode 1, continuous); 0", Rating 4.
#[test]
fn raf_fujifilm_maker_note_reaches_the_report() {
    let f = raf();
    keep("fuji.raf", &f);
    let exif = specular_decode::exif_tiff(&f).expect("exif");
    let l = Localizer::new("en".parse().unwrap());
    let r = read(
        &MetadataInput {
            exif_tiff: Some(&exif),
            ..Default::default()
        },
        &l,
    );
    let want = [
        ("camera", "camera", "FUJIFILM X-T3"),
        ("exposure", "shutter", "1/250 s"),
        ("exposure", "aperture", "f/5.6"),
        ("exposure", "iso", "ISO 400"),
        ("exposure", "white_balance", "Auto (white priority)"),
        ("exposure", "focus_mode", "AF-C"),
        ("exposure", "shutter_type", "Electronic"),
        ("exposure", "drive_mode", "Continuous, low speed"),
        ("lens", "lens", "XF35mmF2 R WR"),
        ("lens", "stabilisation", "Sensor shift (on, continuous)"),
        ("picture", "film_simulation", "Classic Chrome"),
        ("picture", "dynamic_range", "DR400"),
        ("picture", "highlight_tone", "−1"),
        ("picture", "shadow_tone", "+2"),
        ("picture", "colour", "−1"),
        ("picture", "sharpness", "+4"),
        ("picture", "noise_reduction", "−1"),
        ("picture", "clarity", "−2"),
        ("picture", "grain", "Weak, large"),
        ("picture", "colour_chrome", "Strong"),
        ("picture", "colour_chrome_blue", "Off"),
        ("rights", "rating", "★★★★☆ (4 of 5)"),
    ];
    let mut wrong = Vec::new();
    for (g, k, v) in want {
        let got = item(&r, g, k);
        if got.as_deref() != Some(v) {
            wrong.push(format!("{g}/{k}: {got:?}, want {v:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert_eq!(r.capture_time.as_deref(), Some("2024-05-06T07:08:09"));
    let all = format!("{:?} {:?}", r.summary, r.all_fields);
    assert!(!all.contains("SERIAL-77"), "serial shown");
    let fuji = r
        .all_fields
        .iter()
        .find(|g| g.title == "Fujifilm maker note")
        .expect("maker note fields");
    assert!(
        fuji.fields
            .iter()
            .any(|f| f.tag == "FilmMode" && f.raw == "1536" && f.display == "Classic Chrome")
    );
    // German labels and values come from the catalogue.
    let de = read(
        &MetadataInput {
            exif_tiff: Some(&exif),
            ..Default::default()
        },
        &Localizer::new("de".parse().unwrap()),
    );
    assert_eq!(
        item(&de, "exposure", "drive_mode").as_deref(),
        Some("Serie, langsam")
    );
}

/// An IPTC-IIM dataset.
fn dataset(number: u8, v: &str) -> Vec<u8> {
    let mut b = vec![0x1C, 2, number];
    b.extend((v.len() as u16).to_be_bytes());
    b.extend(v.as_bytes());
    b
}

/// The IIM block: UTF-8 declared, title, keywords, creator, city, date and time, copyright,
/// caption.
fn iim() -> Vec<u8> {
    let mut b = vec![0x1C, 1, 90, 0, 3, 0x1B, b'%', b'G'];
    for (n, v) in [
        (5, "Harbour at dawn"),
        (25, "boats"),
        (25, "Mähren"),
        (80, "Ana Silva"),
        (90, "Porto"),
        (55, "20230411"),
        (60, "063000+0100"),
        (116, "© Ana Silva"),
        (120, "Fishing boats leaving"),
    ] {
        b.extend(dataset(n, v));
    }
    b
}

/// Photoshop image resources holding `iim` as resource 1028.
fn resources(iim: &[u8]) -> Vec<u8> {
    let mut r = b"8BIM\x04\x04\0\0".to_vec();
    r.extend((iim.len() as u32).to_be_bytes());
    r.extend(iim);
    if iim.len() % 2 == 1 {
        r.push(0);
    }
    r
}

fn check_iptc(r: &MetadataReport, name: &str) {
    let want = [
        ("title", "Harbour at dawn"),
        ("description", "Fishing boats leaving"),
        ("creator", "Ana Silva"),
        ("copyright", "© Ana Silva"),
        ("keywords", "boats, Mähren"),
    ];
    for (k, v) in want {
        assert_eq!(item(r, "rights", k).as_deref(), Some(v), "{name}: {k}");
    }
    assert_eq!(
        r.capture_time.as_deref(),
        Some("2023-04-11T06:30:00+01:00"),
        "{name}"
    );
    let iptc = r
        .all_fields
        .iter()
        .find(|g| strip_isolates(&g.title) == "IPTC tags")
        .unwrap_or_else(|| panic!("{name}: no IPTC fields"));
    assert!(
        iptc.fields
            .iter()
            .any(|f| f.tag == "City" && f.display == "Porto"),
        "{name}"
    );
}

/// A JPEG's IPTC (APP13) and comment (COM). exiftool on `iptc.jpg`: ObjectName "Harbour at
/// dawn", Keywords "boats, Mähren", By-line "Ana Silva", City Porto, DateCreated 2023:04:11,
/// TimeCreated 06:30:00+01:00, CopyrightNotice "© Ana Silva", Caption-Abstract "Fishing boats
/// leaving", Comment "Scanned at 600 dpi".
#[test]
fn jpeg_iptc_and_comment_reach_the_report() {
    let mut app13 = b"Photoshop 3.0\0".to_vec();
    app13.extend(resources(&iim()));
    let f = jpeg(&[(0xED, app13), (0xFE, b"Scanned at 600 dpi".to_vec())]);
    keep("iptc.jpg", &f);
    let seg = jpeg::scan(&f).unwrap();
    let r = read(
        &seg.input(FileFacts::default()),
        &Localizer::new("en".parse().unwrap()),
    );
    check_iptc(&r, "jpeg");
    assert_eq!(
        item(&r, "rights", "comment").as_deref(),
        Some("Scanned at 600 dpi")
    );
}

/// A TIFF's IPTC-NAA tag written as LONGs (as many writers do), and the same block inside its
/// Photoshop resources. exiftool on `iptc.tif` and `iptc-ps.tif`: as for the JPEG.
#[test]
fn tiff_iptc_reaches_the_report() {
    let mut block = iim();
    block.resize(block.len().div_ceil(4) * 4, 0);
    for (name, tag, typ, data) in [
        ("iptc.tif", 33723u16, 4u16, block.clone()),
        ("iptc-ps.tif", 34377, 7, resources(&iim())),
    ] {
        let count = if typ == 4 { data.len() / 4 } else { data.len() };
        let mut f = b"II*\0\x08\0\0\0".to_vec();
        f.extend(ifd_at(
            8,
            vec![
                short(256, 1),
                short(257, 1),
                short(258, 8),
                short(259, 1),
                short(262, 1),
                long(273, 4, 0),
                short(277, 1),
                long(279, 4, 1),
                (tag, typ, count as u32, data, None),
            ],
        ));
        keep(name, &f);
        let e = specular_decode::embedded_metadata(&f);
        let r = read(
            &MetadataInput {
                iptc: e.iptc.as_deref(),
                ..Default::default()
            },
            &Localizer::new("en".parse().unwrap()),
        );
        check_iptc(&r, name);
    }
}

fn tags_of(bytes: &[u8]) -> Vec<TagFacts> {
    specular_decode::text_tags(bytes)
        .into_iter()
        .map(|t| TagFacts {
            kind: t.kind.to_owned(),
            props: t
                .props
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
            fields: t.fields,
        })
        .collect()
}

fn png_chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
    let mut c = (data.len() as u32).to_be_bytes().to_vec();
    c.extend(kind);
    c.extend(data);
    let mut crc = 0xFFFF_FFFFu32;
    for &b in kind.iter().chain(data) {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    c.extend((!crc).to_be_bytes());
    c
}

/// PNG text chunks. exiftool on `text.png`: Title "Night sky", Author "Ana Silva",
/// Description-en "Orion over the bay", Copyright "CC BY 4.0", CreationTime "2022:12:01
/// 22:15:00+01:00", Software "Stellar 2.1", Comment "stacked from 40 frames".
#[test]
fn png_text_reaches_the_report() {
    let mut f = b"\x89PNG\r\n\x1a\n".to_vec();
    f.extend(png_chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]));
    f.extend(png_chunk(b"tEXt", b"Title\0Night sky"));
    f.extend(png_chunk(b"tEXt", b"Author\0Ana Silva"));
    f.extend(png_chunk(
        b"iTXt",
        b"Description\0\0\0en\0\0Orion over the bay",
    ));
    f.extend(png_chunk(b"tEXt", b"Copyright\0CC BY 4.0"));
    f.extend(png_chunk(
        b"tEXt",
        b"Creation Time\0Thu, 01 Dec 2022 22:15:00 +0100",
    ));
    f.extend(png_chunk(b"tEXt", b"Software\0Stellar 2.1"));
    f.extend(png_chunk(b"tEXt", b"Comment\0stacked from 40 frames"));
    f.extend(png_chunk(
        b"IDAT",
        &[
            0x78, 0x01, 0x01, 0x02, 0x00, 0xFD, 0xFF, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01,
        ],
    ));
    f.extend(png_chunk(b"IEND", b""));
    keep("text.png", &f);
    let r = read(
        &MetadataInput {
            tags: tags_of(&f),
            ..Default::default()
        },
        &Localizer::new("en".parse().unwrap()),
    );
    for (g, k, v) in [
        ("rights", "title", "Night sky"),
        ("rights", "creator", "Ana Silva"),
        ("rights", "description", "Orion over the bay"),
        ("rights", "copyright", "CC BY 4.0"),
        ("rights", "comment", "stacked from 40 frames"),
        ("camera", "software", "Stellar 2.1"),
    ] {
        assert_eq!(item(&r, g, k).as_deref(), Some(v), "{k}");
    }
    assert_eq!(r.capture_time.as_deref(), Some("2022-12-01T22:15:00+01:00"));
    assert!(
        r.all_fields
            .iter()
            .any(|g| strip_isolates(&g.title) == "PNG tags")
    );
}

/// A GIF's comment and XMP. exiftool on `xmp.gif`: Comment "made with care", XMP Title
/// "Loop".
#[test]
fn gif_comment_and_xmp_reach_the_report() {
    let mut f = b"GIF89a\x01\x00\x01\x00\x00\x00\x00".to_vec();
    f.extend(b"\x21\xFE\x0emade with care\x00");
    f.extend(b"\x21\xFF\x0bXMP DataXMP");
    f.extend(
        b"<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?><x:xmpmeta xmlns:x='adobe:ns:meta/'>\
<rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'><rdf:Description \
xmlns:dc='http://purl.org/dc/elements/1.1/'><dc:title><rdf:Alt><rdf:li xml:lang='x-default'>Loop\
</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end='w'?>",
    );
    f.push(1);
    f.extend((0..=255u8).rev());
    f.push(0);
    f.extend(b"\x2C\0\0\0\0\x01\0\x01\0\0\x02\x02\x44\x01\x00\x3B");
    keep("xmp.gif", &f);
    let e = specular_decode::embedded_metadata(&f);
    let r = read(
        &MetadataInput {
            xmp: e.xmp.as_deref(),
            tags: tags_of(&f),
            ..Default::default()
        },
        &Localizer::new("en".parse().unwrap()),
    );
    assert_eq!(item(&r, "rights", "title").as_deref(), Some("Loop"));
    assert_eq!(
        item(&r, "rights", "comment").as_deref(),
        Some("made with care")
    );
}

/// A movie's tags as the demuxer hands them over (QuickTime): the title, comment and the
/// ISO 6709 location reach the summary, the location as a position. exiftool on such a movie
/// (specular-video's `text_tags_are_kept`): GPSCoordinates 38.7223 -9.1393 45.
#[test]
fn movie_tags_reach_the_report() {
    let t = specular_decode::text_tags::movie_tags(
        "QuickTime",
        &[
            (
                "com.apple.quicktime.location.ISO6709".to_owned(),
                "+38.7223-009.1393+045.000/".to_owned(),
            ),
            ("©nam".to_owned(), "Tram 28".to_owned()),
            ("©cmt".to_owned(), "first ride".to_owned()),
        ],
    );
    let r = read(
        &MetadataInput {
            tags: vec![TagFacts {
                kind: t.kind.to_owned(),
                props: t
                    .props
                    .into_iter()
                    .map(|(k, v)| (k.to_owned(), v))
                    .collect(),
                fields: t.fields,
            }],
            ..Default::default()
        },
        &Localizer::new("en".parse().unwrap()),
    );
    let gps = r.gps.as_ref().expect("position");
    assert!((gps.lat - 38.7223).abs() < 1e-6 && (gps.lon + 9.1393).abs() < 1e-6);
    assert_eq!(gps.altitude_m, Some(45.0));
    assert_eq!(item(&r, "rights", "title").as_deref(), Some("Tram 28"));
    assert_eq!(item(&r, "rights", "comment").as_deref(), Some("first ride"));
}
