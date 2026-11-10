use super::embed::{self, Container, EmbedError};
use super::template::{self, KeywordMode, Template};
use super::*;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

/// A value for every field (lists with two values, text with markup characters and
/// non-Latin letters).
pub(super) fn full_record() -> Record {
    let mut r = Record::new();
    for f in Field::all() {
        let v = match f {
            Field::Keywords => s(&["Places|France|Paris", "sea & sky"]),
            Field::Creator => s(&["Ann Smith", "Bob <B> Jones"]),
            Field::PersonsShown => s(&["Zoë", "José"]),
            Field::ShownCountryCode | Field::CreatedCountryCode => s(&["FRA"]),
            Field::Description => s(&["Line one\nLine two — ok"]),
            f => vec![format!("{} «value» & <more> שלום", f.key())],
        };
        r.set(f, v);
    }
    r
}

fn all_fields() -> Vec<Field> {
    Field::all().collect()
}

#[test]
fn keys_are_unique_and_found_again() {
    let mut keys: Vec<&str> = Field::all().map(Field::key).collect();
    for f in Field::all() {
        assert_eq!(Field::from_key(f.key()), Some(f));
    }
    keys.sort_unstable();
    let n = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), n);
}

/// Every field through a new sidecar, then through one that already has other properties
/// (kept byte for byte).
#[test]
fn every_field_round_trips_through_a_sidecar() {
    let r = full_record();
    let text = write_xmp(None, &r, &all_fields(), None, true);
    assert_eq!(from_xmp_text(&text), r, "{text}");
    let other = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description rdf:about=\"\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" xmp:Rating=\"3\" xmp:Label=\"Red\"><xmp:Other>keep me</xmp:Other></rdf:Description></rdf:RDF></x:xmpmeta>";
    let text = write_xmp(Some(other), &r, &all_fields(), None, true);
    assert!(text.contains("xmp:Rating=\"3\" xmp:Label=\"Red\""));
    assert!(text.contains("<xmp:Other>keep me</xmp:Other>"));
    let back = from_xmp_text(&text);
    assert_eq!(back, r);
    let x = crate::xmp::parse(text.as_bytes());
    assert!(x.error.is_none(), "{:?}", x.error);
    assert_eq!(x.get("xmp:Rating"), Some("3"));
    // Written again unchanged: the same text.
    assert_eq!(
        write_xmp(Some(&text), &back, &all_fields(), None, true),
        text
    );
}

/// Only the fields named are written; the rest of the sidecar is untouched.
#[test]
fn only_changed_fields_are_written() {
    let r = full_record();
    let text = write_xmp(None, &r, &all_fields(), None, true);
    let mut r2 = r.clone();
    r2.set(Field::Headline, s(&["New headline"]));
    r2.set(Field::Credit, s(&["ignored: not named"]));
    let t2 = write_xmp(Some(&text), &r2, &[Field::Headline], None, true);
    let back = from_xmp_text(&t2);
    assert_eq!(back.values(Field::Headline), s(&["New headline"]));
    assert_eq!(back.values(Field::Credit), r.values(Field::Credit));
    // Everything but the headline's element is the same text.
    let cut = |t: &str| {
        let r = kw::find_property(t, "photoshop:Headline").unwrap();
        format!("{}{}", &t[..r.start], &t[r.end..])
    };
    assert_eq!(cut(&text), cut(&t2));
}

/// A field cleared in a sidecar says "none" over the file's own; left out, the file's own
/// shows.
#[test]
fn cleared_fields_override_the_files_own() {
    let mut file = Record::new();
    file.set(Field::Headline, s(&["From the camera"]));
    file.set(Field::ShownCity, s(&["Lisbon"]));
    file.set(Field::ContactEmail, s(&["a@b.c"]));
    let mut r = file.clone();
    r.set(Field::Headline, vec![]);
    r.set(Field::ShownCity, vec![]);
    r.set(Field::ContactEmail, vec![]);
    let side = write_xmp(
        None,
        &r,
        &[Field::Headline, Field::ShownCity, Field::ContactEmail],
        None,
        true,
    );
    let side_rec = from_xmp_text(&side);
    assert_eq!(side_rec.get(Field::Headline), Some(&[][..]));
    let shown = side_rec.over(&file);
    assert!(shown.values(Field::Headline).is_empty());
    assert!(shown.values(Field::ShownCity).is_empty());
    assert!(shown.values(Field::ContactEmail).is_empty());
    // Not written as empty (embedded): the properties go.
    let full = write_xmp(None, &file, &all_fields(), None, false);
    let gone = write_xmp(Some(&full), &r, &[Field::Headline], None, false);
    assert!(!gone.contains("photoshop:Headline"), "{gone}");
    assert!(from_xmp_text(&gone).get(Field::Headline).is_none());
}

/// Other languages of a language alternative stay; x-default changes.
#[test]
fn other_languages_are_kept() {
    let x = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">Old</rdf:li><rdf:li xml:lang=\"fr-FR\">Vieux</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>";
    let mut r = Record::new();
    r.set(Field::Title, s(&["New"]));
    let t = write_xmp(Some(x), &r, &[Field::Title], None, true);
    assert!(
        t.contains(">New</rdf:li>") && t.contains(">Vieux</rdf:li>"),
        "{t}"
    );
}

/// Legacy location properties (Lightroom, Photo Mechanic) read as location shown; the
/// Extension's structure wins where both are there.
#[test]
fn legacy_location_reads_as_shown() {
    let x = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description rdf:about=\"\" xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\" xmlns:Iptc4xmpCore=\"http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/\" photoshop:City=\"Porto\" Iptc4xmpCore:CountryCode=\"PRT\"/></rdf:RDF></x:xmpmeta>";
    let r = from_xmp_text(x);
    assert_eq!(r.values(Field::ShownCity), s(&["Porto"]));
    assert_eq!(r.values(Field::ShownCountryCode), s(&["PRT"]));
    // Attribute form is replaced by an element.
    let mut r2 = r.clone();
    r2.set(Field::ShownCity, s(&["Braga"]));
    let t = write_xmp(Some(x), &r2, &[Field::ShownCity], None, true);
    let back = from_xmp_text(&t);
    assert_eq!(back.values(Field::ShownCity), s(&["Braga"]));
    assert_eq!(back.values(Field::ShownCountryCode), s(&["PRT"]));
    assert!(!t.contains("photoshop:City=\"Porto\""), "{t}");
}

#[test]
fn iim_round_trips_and_keeps_other_datasets() {
    let r = full_record();
    let fields = all_fields();
    let subject = kw::subject(r.values(Field::Keywords), None);
    let b = write_iim(None, &r, &fields, &subject).unwrap();
    let back = read_iim(&b);
    for f in Field::all() {
        let Some(max) = iim_limit(f) else {
            assert!(back.get(f).is_none(), "{f:?} has no dataset");
            continue;
        };
        let want: Vec<String> = if f == Field::Keywords {
            subject.clone()
        } else {
            r.values(f)
                .iter()
                .map(|v| {
                    let mut e = max.min(v.len());
                    while !v.is_char_boundary(e) {
                        e -= 1;
                    }
                    v[..e].trim().to_owned()
                })
                .collect()
        };
        assert_eq!(back.values(f), want, "{f:?}");
    }
    // A Latin-1 block with a date: the date stays, the text becomes UTF-8.
    let old = [
        &[0x1C, 2, 0, 0, 2, 0, 4][..],
        &[0x1C, 2, 55, 0, 8],
        b"20200102",
        &[0x1C, 2, 80, 0, 4],
        b"Jos\xe9",
    ]
    .concat();
    let mut r = Record::new();
    r.set(Field::Headline, s(&["Hi"]));
    let b = write_iim(Some(&old), &r, &[Field::Headline], &[]).unwrap();
    let back = read_iim(&b);
    assert_eq!(back.values(Field::Creator), s(&["José"]));
    assert_eq!(back.values(Field::Headline), s(&["Hi"]));
    assert!(b.windows(8).any(|w| w == b"20200102"));
    // Nothing left: no block.
    assert!(write_iim(None, &Record::new(), &fields, &[]).is_none());
}

/// A small baseline JPEG: SOI, APP0, an Exif APP1, DQT, then a fake scan and EOI.
pub(super) fn tiny_jpeg(with_iim: bool) -> Vec<u8> {
    let mut j = vec![0xFF, 0xD8];
    let seg = |j: &mut Vec<u8>, m: u8, b: &[u8]| {
        j.extend([0xFF, m]);
        j.extend(((b.len() + 2) as u16).to_be_bytes());
        j.extend(b);
    };
    seg(&mut j, 0xE0, b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
    seg(&mut j, 0xE1, b"Exif\0\0MM\0*\0\0\0\x08\0\0\0\0\0\0");
    if with_iim {
        let iim = [
            &[0x1C, 2, 5, 0, 3][..],
            b"Old",
            &[0x1C, 2, 55, 0, 8],
            b"20200102",
        ]
        .concat();
        let mut ps = b"Photoshop 3.0\08BIM\x04\x04\0\0".to_vec();
        ps.extend((iim.len() as u32).to_be_bytes());
        ps.extend(&iim);
        if iim.len() % 2 == 1 {
            ps.push(0);
        }
        ps.extend(b"8BIM\x04\x25\0\0\0\0\0\x10");
        ps.extend([0u8; 16]);
        ps.extend(b"8BIM\x04\x0c\0\0\0\0\0\x02xy");
        seg(&mut j, 0xED, &ps);
    }
    seg(&mut j, 0xDB, &[0u8; 65]);
    j.extend([0xFF, 0xDA, 0, 2, 1, 2, 3, 0xFF, 0x00, 4, 0xFF, 0xD9]);
    j
}

/// Every field into a JPEG and back; every other byte where it was.
#[test]
fn every_field_round_trips_through_a_jpeg() {
    for with_iim in [false, true] {
        let j = tiny_jpeg(with_iim);
        let r = full_record();
        let before = read_embedded(&mut std::io::Cursor::new(&j)).unwrap();
        let x = write_xmp(before.xmp.as_deref(), &r, &all_fields(), None, false);
        let subject = kw::subject(r.values(Field::Keywords), None);
        let iim = write_iim(before.iim.as_deref(), &r, &all_fields(), &subject);
        let out = embed::jpeg(&j, &x, Some(iim.as_deref())).unwrap();
        let e = read_embedded(&mut std::io::Cursor::new(&out)).unwrap();
        assert_eq!(e.record(), r, "with IIM {with_iim}");
        // The scan and the other segments are byte for byte the same.
        let scan = j.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
        assert!(out.ends_with(&j[scan..]));
        let dqt = j.windows(2).position(|w| w == [0xFF, 0xDB]).unwrap();
        assert!(out.windows(69).any(|w| w == &j[dqt..dqt + 69]));
        assert!(
            out.starts_with(&j[..2 + 18 + 2 + 22]),
            "APP0 and Exif first"
        );
        if with_iim {
            // The date dataset stays; the digest is the new block's.
            let b = e.iim.as_deref().unwrap();
            assert!(b.windows(8).any(|w| w == b"20200102"));
            use md5::Digest;
            let d = md5::Md5::digest(b);
            assert!(out.windows(16).any(|w| w == &d[..]));
            assert!(
                out.windows(6).any(|w| w == b"\x0c\0\0\0\0\0"),
                "other resources kept"
            );
        }
        // Written again with the same values: the same bytes.
        let x2 = write_xmp(e.xmp.as_deref(), &r, &all_fields(), None, false);
        let iim2 = write_iim(e.iim.as_deref(), &r, &all_fields(), &subject);
        assert_eq!(embed::jpeg(&out, &x2, Some(iim2.as_deref())).unwrap(), out);
    }
}

/// When XMP and IIM disagree, XMP wins; IIM fills what XMP lacks.
#[test]
fn xmp_wins_over_iim() {
    let j = tiny_jpeg(true);
    let mut r = Record::new();
    r.set(Field::Headline, s(&["H"]));
    let x = write_xmp(None, &r, &[Field::Headline], None, false);
    // XMP says the title is "New"; the IIM still says "Old".
    let mut t = Record::new();
    t.set(Field::Title, s(&["New"]));
    let x = write_xmp(Some(&x), &t, &[Field::Title], None, false);
    let out = embed::jpeg(&j, &x, None).unwrap();
    let e = read_embedded(&mut std::io::Cursor::new(&out)).unwrap();
    assert_eq!(
        read_iim(e.iim.as_deref().unwrap()).values(Field::Title),
        s(&["Old"])
    );
    assert_eq!(e.record().values(Field::Title), s(&["New"]));
    // IIM alone: its title counts.
    let e2 = read_embedded(&mut std::io::Cursor::new(&j)).unwrap();
    assert_eq!(e2.record().values(Field::Title), s(&["Old"]));
}

/// A little-endian TIFF: one 2×1 grey strip, IFD0 with width, height, bits, compression,
/// photometric, strip offsets, rows, byte counts, and (optionally) an XMP packet already.
pub(super) fn tiny_tiff(xmp: Option<&str>, big_endian: bool) -> Vec<u8> {
    let w16 = |v: u16| {
        if big_endian {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    let w32 = |v: u32| {
        if big_endian {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    let mut t = if big_endian {
        b"MM\0*".to_vec()
    } else {
        b"II*\0".to_vec()
    };
    t.extend(w32(8));
    let pixels_at = 8u32;
    t.extend([10u8, 200]);
    let xmp_at = t.len() as u32;
    if let Some(x) = xmp {
        t.extend(x.as_bytes());
        if t.len() % 2 == 1 {
            t.push(0);
        }
    }
    let ifd = t.len() as u32;
    t[4..8].copy_from_slice(&w32(ifd));
    let mut entries: Vec<(u16, u16, u32, u32)> = vec![
        (256, 3, 1, 2),
        (257, 3, 1, 1),
        (258, 3, 1, 8),
        (259, 3, 1, 1),
        (262, 3, 1, 1),
        (273, 4, 1, pixels_at),
        (278, 3, 1, 1),
        (279, 4, 1, 2),
    ];
    if let Some(x) = xmp {
        entries.push((700, 1, x.len() as u32, xmp_at));
    }
    t.extend(w16(entries.len() as u16));
    for (tag, ty, n, v) in entries {
        t.extend(w16(tag));
        t.extend(w16(ty));
        t.extend(w32(n));
        if ty == 3 && n == 1 {
            t.extend(w16(v as u16));
            t.extend([0, 0]);
        } else {
            t.extend(w32(v));
        }
    }
    t.extend(w32(0));
    t
}

#[test]
fn every_field_round_trips_through_a_tiff() {
    let r = full_record();
    let pre = write_xmp(None, &Record::new(), &[], None, false);
    for (xmp, be) in [(None, false), (Some(pre.as_str()), true)] {
        let t = tiny_tiff(xmp, be);
        let x = write_xmp(xmp, &r, &all_fields(), None, false);
        let out = embed::tiff(&t, &x).unwrap();
        let e = read_embedded(&mut std::io::Cursor::new(&out)).unwrap();
        assert_eq!(e.record(), r);
        // The old bytes are all still there, the pixels at their place.
        if xmp.is_none() {
            assert_eq!(&out[8..t.len()], &t[8..]);
        } else {
            // Only IFD0's XMP entry changed.
            let diff = t.iter().zip(&out).filter(|(a, b)| a != b).count();
            assert!(diff <= 8, "{diff} bytes changed");
        }
        assert_eq!(&out[8..10], &[10, 200]);
        // Again: the entry now exists and is replaced.
        let out2 = embed::tiff(&out, &x).unwrap();
        let e2 = read_embedded(&mut std::io::Cursor::new(&out2)).unwrap();
        assert_eq!(e2.record(), r);
    }
    assert_eq!(
        embed::tiff(b"II+\0\x08\0\0\0", "x"),
        Err(EmbedError::BigTiff)
    );
    assert_eq!(embed::tiff(b"\x89PNG", "x"), Err(EmbedError::Unsupported));
}

#[test]
fn damaged_and_unsupported_files_are_refused() {
    assert_eq!(embed::container(b"\x89PNG\r\n"), None);
    assert_eq!(
        embed::container(&[0xFF, 0xD8, 0xFF, 0xE0]),
        Some(Container::Jpeg)
    );
    let j = tiny_jpeg(false);
    for k in 2..j.len() - 12 {
        // Cut inside the segments: refused, never a panic.
        let _ = embed::jpeg(&j[..k], "<x/>", Some(None));
    }
    assert_eq!(
        embed::jpeg(&j[..30], "<x/>", None),
        Err(EmbedError::Damaged)
    );
    let big = "x".repeat(embed::JPEG_XMP_MAX + 1);
    assert_eq!(embed::jpeg(&j, &big, None), Err(EmbedError::TooLarge));
    assert!(embed::writable_ext("JPG") && embed::writable_ext("dng"));
    assert!(!embed::writable_ext("cr2") && !embed::writable_ext("png"));
    let t = tiny_tiff(None, false);
    for k in 0..t.len() {
        let _ = embed::tiff(&t[..k], "<x/>");
        let _ = read_embedded(&mut std::io::Cursor::new(&t[..k]));
    }
}

#[test]
fn edits_change_only_what_they_name() {
    let mut r = Record::new();
    r.set(Field::Keywords, s(&["a", "b"]));
    r.set(Field::Title, s(&["T"]));
    let e = Edit {
        set: vec![(Field::Copyright, s(&["© Me"]))],
        keywords: Some(KeywordEdit::Changes(vec![
            kw::Change::Add("c".into()),
            kw::Change::Remove("A".into()),
        ])),
    };
    let after = e.apply(&r);
    assert_eq!(after.values(Field::Keywords), s(&["b", "c"]));
    assert_eq!(after.values(Field::Title), s(&["T"]));
    assert_eq!(after.values(Field::Copyright), s(&["© Me"]));
    assert_eq!(e.fields(), vec![Field::Keywords, Field::Copyright]);
    let lines = after.to_lines();
    assert!(lines.contains("title=T") && lines.contains("copyright=© Me"));
    assert!(!lines.contains("keywords"));
}

#[test]
fn typed_text_becomes_values() {
    assert_eq!(parse_text(Field::Creator, " A ; ;B;"), s(&["A", "B"]));
    assert_eq!(
        parse_text(Field::Keywords, "Places > Paris; sea"),
        s(&["Places|Paris", "sea"])
    );
    assert_eq!(parse_text(Field::Title, "  a; b "), s(&["a; b"]));
    assert!(parse_text(Field::Title, "  ").is_empty());
}

#[test]
fn templates_round_trip_as_text() {
    let mut a = Template::new("Copyright & creator");
    a.set(Field::Copyright, s(&["© 2026 Ann"]));
    a.set(Field::Creator, s(&["Ann", "Bob"]));
    a.set(Field::Keywords, s(&["Places|Paris"]));
    a.set(
        Field::Instructions,
        s(&["No crops\nCredit required \\ always"]),
    );
    a.keyword_mode = KeywordMode::Replace;
    let mut b = Template::new("[Event] crew");
    b.set(Field::Event, s(&["Festival"]));
    let text = template::to_text(&[a.clone(), b.clone()]);
    assert_eq!(template::parse(&text), vec![a.clone(), b]);
    // Fields in the editor's order whatever order they were set in.
    assert_eq!(a.fields[0].0, Field::Keywords);
    let e = a.edit();
    assert_eq!(e.keywords, Some(KeywordEdit::Replace(s(&["Places|Paris"]))));
    a.keyword_mode = KeywordMode::Add;
    assert!(matches!(a.edit().keywords, Some(KeywordEdit::Changes(_))));
    // Junk lines and unknown keys are skipped.
    let t = template::parse("x = 1\n[A]\nnope = 2\ntitle = T\ntitle = U\n[]\n");
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].get(Field::Title), Some(&s(&["T"])[..]));
}
