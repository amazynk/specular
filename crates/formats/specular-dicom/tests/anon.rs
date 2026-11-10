//! The Part 10 writer, the de-identifier and burned-in text masking: synthetic files with
//! known identifying content, and the DICOM corpus (`tools/corpus/fetch-dicom.sh`, read from
//! `SPECULAR_DICOM_CORPUS` or `tools/corpus/dicom`; skipped when absent).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use specular_dicom::anon::{self, AnonError, Options, Request, Session, Temporal};
use specular_dicom::dict::Vr;
use specular_dicom::mask::Rect;
use specular_dicom::write::{self, FileMeta, Object, Value};
use specular_dicom::{Dicom, Frame, Pixels, pdi, series};

fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_DICOM_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/dicom")
        });
    p.is_dir().then_some(p)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dcm")) {
            out.push(p);
        }
    }
}

fn corpus_files() -> Option<Vec<PathBuf>> {
    let root = corpus()?;
    let mut v = Vec::new();
    walk(&root, &mut v);
    v.sort();
    Some(v)
}

fn us(v: u16) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// A small CT-like file with identifying content everywhere the profile looks.
fn sample(sop: &str, referenced: Option<&str>) -> Vec<u8> {
    let mut o = Object::default();
    o.put_text(0x0008_0005, Vr::CS, "ISO_IR 100");
    o.put_text(0x0008_0016, Vr::UI, "1.2.840.10008.5.1.4.1.1.2");
    o.put_text(0x0008_0018, Vr::UI, sop);
    o.put_text(0x0008_0020, Vr::DA, "20240115");
    o.put_text(0x0008_0030, Vr::TM, "101500");
    o.put_text(0x0008_0050, Vr::SH, "ACC98765");
    o.put_text(0x0008_0060, Vr::CS, "CT");
    o.put_text(0x0008_0080, Vr::LO, "St Elsewhere Hospital");
    o.put_text(0x0008_0090, Vr::PN, "House^Gregory");
    o.put_text(0x0008_1030, Vr::LO, "CT head for Jane Doe");
    o.put_text(0x0008_103E, Vr::LO, "Axial DOE");
    o.put_text(0x0008_1010, Vr::SH, "CTSTATION7");
    if let Some(r) = referenced {
        let mut item = Object::default();
        item.put_text(0x0008_1150, Vr::UI, "1.2.840.10008.5.1.4.1.1.2");
        item.put_text(0x0008_1155, Vr::UI, r);
        o.put(0x0008_1140, Vr::SQ, Value::Items(vec![item]));
    }
    o.put_text(0x0009_0010, Vr::LO, "ACME 1.0");
    o.put_text(0x0009_1001, Vr::LO, "Jane Doe secret");
    o.put_text(0x0010_0010, Vr::PN, "Doe^Jane");
    o.put_text(0x0010_0020, Vr::LO, "MRN-0042");
    o.put_text(0x0010_0030, Vr::DA, "19700203");
    o.put_text(0x0010_0040, Vr::CS, "F");
    o.put_text(0x0010_1010, Vr::AS, "054Y");
    o.put_text(0x0010_4000, Vr::LT, "Patient Jane");
    o.put_text(0x0018_1000, Vr::LO, "SN-777");
    o.put_text(0x0020_000D, Vr::UI, "1.2.826.0.1.3680043.2.1125.1.1");
    o.put_text(0x0020_000E, Vr::UI, "1.2.826.0.1.3680043.2.1125.1.2");
    o.put_text(0x0020_0010, Vr::SH, "ST1234");
    o.put_text(0x0020_0052, Vr::UI, "1.2.826.0.1.3680043.2.1125.1.3");
    o.put_bytes(0x0028_0002, Vr::US, us(1));
    o.put_text(0x0028_0004, Vr::CS, "MONOCHROME2");
    o.put_bytes(0x0028_0010, Vr::US, us(8));
    o.put_bytes(0x0028_0011, Vr::US, us(8));
    o.put_bytes(0x0028_0100, Vr::US, us(16));
    o.put_bytes(0x0028_0101, Vr::US, us(12));
    o.put_bytes(0x0028_0102, Vr::US, us(11));
    o.put_bytes(0x0028_0103, Vr::US, us(0));
    o.put_text(0x0032_1032, Vr::PN, "Wilson^James");
    o.put_bytes(0x6000_0010, Vr::US, us(8));
    o.put_bytes(0x6000_3000, Vr::OW, vec![0xFF; 8]);
    let px: Vec<u8> = (0..64u16)
        .flat_map(|i| (i * 50 + 7).to_le_bytes())
        .collect();
    o.put_bytes(0x7FE0_0010, Vr::OW, px);
    write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE)).unwrap()
}

fn session() -> Session {
    Session::with_key([7; 32])
}

fn req<'r>(o: &'r Options, s: &'r Session, masks: &'r [Rect]) -> Request<'r> {
    Request {
        options: o,
        session: s,
        masks,
        allow_burned_in: false,
        cancel: None,
    }
}

fn obj(file: &[u8]) -> (Dicom, Object<'static>) {
    let d = Dicom::parse(file).unwrap();
    let o = owned(&Object::from_dataset(&d.dataset, d.buf(file)));
    (d, o)
}

/// Every (tag, VR, text) of an object, sequences included.
fn texts(o: &Object, out: &mut Vec<(u32, Vr, String)>) {
    for e in &o.elements {
        match &e.value {
            Value::Items(items) => {
                for i in items {
                    texts(i, out);
                }
            }
            Value::Bytes(_) if e.vr.is_text() || e.vr == Vr::UN => {
                out.push((e.tag, e.vr, e.text().trim().to_owned()))
            }
            _ => out.push((e.tag, e.vr, String::new())),
        }
    }
}

#[test]
fn basic_profile_on_a_sample() {
    let src = sample("1.2.826.0.1.3680043.2.1125.1.4", None);
    let s = session();
    let opts = Options::default();
    let out = anon::anonymise(&src, &req(&opts, &s, &[])).unwrap();
    assert!(!out.masked && !out.transcoded);
    let (d, o) = obj(&out.bytes);
    let t = |tag| o.text(tag).map(|c| c.into_owned());
    let pseudo = s.pseudonym("MRN-0042", "Doe^Jane");
    assert_eq!(t(0x0010_0010).as_deref(), Some(pseudo.as_str()));
    assert_eq!(t(0x0010_0020).as_deref(), Some(pseudo.as_str()));
    assert_eq!(t(0x0010_0030).as_deref(), Some(""));
    assert_eq!(t(0x0008_0020).as_deref(), Some("")); // study date: Z
    assert_eq!(t(0x0008_0050).as_deref(), Some("")); // accession: Z
    assert_eq!(t(0x0008_0080).as_deref(), Some("ANONYMOUS")); // institution: X/Z/D
    assert_eq!(t(0x0008_1030), None); // study description: X
    assert_eq!(t(0x0008_103E), None);
    assert_eq!(t(0x0010_1010), None); // age: X
    assert_eq!(t(0x0010_4000), None);
    assert_eq!(t(0x0018_1000).as_deref(), Some("ANONYMOUS")); // device serial: X/Z/D
    assert_eq!(t(0x0032_1032), None); // requesting physician: X
    assert_eq!(t(0x0008_0060).as_deref(), Some("CT"));
    assert!(o.get(0x0009_0010).is_none() && o.get(0x0009_1001).is_none());
    assert!(o.get(0x6000_0010).is_none() && o.get(0x6000_3000).is_none());
    // UIDs replaced, consistently with the session.
    assert_eq!(
        t(0x0020_000D).unwrap(),
        s.uid("1.2.826.0.1.3680043.2.1125.1.1")
    );
    assert_eq!(
        t(0x0008_0018).unwrap(),
        s.uid("1.2.826.0.1.3680043.2.1125.1.4")
    );
    assert_eq!(t(0x0008_0016).as_deref(), Some("1.2.840.10008.5.1.4.1.1.2"));
    // File meta follows.
    let v = d.values(&out.bytes);
    let meta = |tag| String::from_utf8_lossy(v.text_bytes(d.meta.get(tag).unwrap())).into_owned();
    assert_eq!(meta(0x0002_0003), t(0x0008_0018).unwrap());
    assert_eq!(meta(0x0002_0012), write::IMPLEMENTATION_CLASS_UID);
    // De-identification attributes.
    assert_eq!(t(0x0012_0062).as_deref(), Some("YES"));
    assert_eq!(t(0x0028_0303).as_deref(), Some("REMOVED"));
    let codes: Vec<String> = o
        .get(0x0012_0064)
        .unwrap()
        .items()
        .iter()
        .map(|i| i.text(0x0008_0100).unwrap().into_owned())
        .collect();
    assert_eq!(codes, ["113100"]);
    // Pixels unchanged.
    let src_d = Dicom::parse(&src).unwrap();
    assert_eq!(
        src_d.decode_frame(&src, 0, None).unwrap(),
        d.decode_frame(&out.bytes, 0, None).unwrap()
    );
    // Nothing identifying survives anywhere.
    let mut all = Vec::new();
    texts(&o, &mut all);
    for (_, _, v) in &all {
        for bad in [
            "Doe",
            "Jane",
            "MRN-0042",
            "ACC98765",
            "19700203",
            "20240115",
            "House",
            "Wilson",
            "Elsewhere",
            "SN-777",
            "CTSTATION7",
            "ST1234",
        ] {
            assert!(!v.contains(bad), "{bad} survived in {v:?}");
        }
    }
}

#[test]
fn options_keep_and_clean() {
    let src = sample("1.2.3.4", None);
    let s = session();
    let opts = Options {
        temporal: Temporal::Modified,
        patient_characteristics: true,
        device_identity: true,
        clean_descriptors: true,
        retain_uids: true,
        keep_overlays: true,
        ..Options::default()
    };
    let out = anon::anonymise(&src, &req(&opts, &s, &[])).unwrap();
    let (_, o) = obj(&out.bytes);
    let t = |tag| o.text(tag).map(|c| c.into_owned());
    assert_eq!(
        t(0x0020_000D).as_deref(),
        Some("1.2.826.0.1.3680043.2.1125.1.1")
    );
    assert_eq!(t(0x0010_0040).as_deref(), Some("F"));
    assert_eq!(t(0x0010_1010).as_deref(), Some("054Y"));
    assert_eq!(t(0x0018_1000).as_deref(), Some("SN-777"));
    assert_eq!(t(0x0008_1030).as_deref(), Some("CT head for XXXX XXXX"));
    assert_eq!(t(0x0008_103E).as_deref(), Some("Axial XXXX"));
    let shifted = t(0x0008_0020).unwrap();
    assert_eq!(shifted.len(), 8);
    assert_ne!(shifted, "20240115");
    assert_eq!(t(0x0008_0030).as_deref(), Some("101500"));
    assert_eq!(t(0x0028_0303).as_deref(), Some("MODIFIED"));
    assert!(o.get(0x6000_3000).is_some());
    let codes: Vec<String> = o
        .get(0x0012_0064)
        .unwrap()
        .items()
        .iter()
        .map(|i| i.text(0x0008_0100).unwrap().into_owned())
        .collect();
    for c in [
        "113100", "113103", "113105", "113107", "113108", "113109", "113110",
    ] {
        assert!(codes.iter().any(|x| x == c), "{c} in {codes:?}");
    }
}

#[test]
fn references_stay_valid_across_files() {
    let a = sample("1.2.826.0.1.3680043.2.1125.9.1", None);
    let b = sample(
        "1.2.826.0.1.3680043.2.1125.9.2",
        Some("1.2.826.0.1.3680043.2.1125.9.1"),
    );
    let s = session();
    let opts = Options::default();
    let oa = anon::anonymise(&a, &req(&opts, &s, &[])).unwrap();
    let ob = anon::anonymise(&b, &req(&opts, &s, &[])).unwrap();
    let (_, xa) = obj(&oa.bytes);
    let (_, xb) = obj(&ob.bytes);
    assert_eq!(xa.text(0x0020_000D), xb.text(0x0020_000D));
    assert_eq!(xa.text(0x0020_000E), xb.text(0x0020_000E));
    assert_eq!(xa.text(0x0020_0052), xb.text(0x0020_0052));
    let r = &xb.get(0x0008_1140).unwrap().items()[0];
    assert_eq!(r.text(0x0008_1155), xa.text(0x0008_0018));
    assert_eq!(
        r.text(0x0008_1150).as_deref(),
        Some("1.2.840.10008.5.1.4.1.1.2")
    );
    // Another export's key gives other UIDs.
    let other = anon::anonymise(&a, &req(&opts, &Session::with_key([8; 32]), &[])).unwrap();
    let (_, xo) = obj(&other.bytes);
    assert_ne!(xo.text(0x0008_0018), xa.text(0x0008_0018));
}

#[test]
fn burned_in_needs_masks() {
    let mut src_obj_bytes = sample("1.2.3.5", None);
    // Burned In Annotation YES: rebuild with the flag.
    {
        let (_, mut o) = obj(&src_obj_bytes);
        o.put_text(0x0028_0301, Vr::CS, "YES");
        let o: Object<'static> = owned(&o);
        src_obj_bytes = write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE)).unwrap();
    }
    let s = session();
    let opts = Options::default();
    assert!(matches!(
        anon::anonymise(&src_obj_bytes, &req(&opts, &s, &[])),
        Err(AnonError::BurnedIn)
    ));
    let masks = [Rect::new(0, 0, 8, 2)];
    let out = anon::anonymise(&src_obj_bytes, &req(&opts, &s, &masks)).unwrap();
    assert!(out.masked && !out.transcoded);
    let (d, o) = obj(&out.bytes);
    assert_eq!(o.text(0x0028_0301).as_deref(), Some("NO"));
    let f = d.decode_frame(&out.bytes, 0, None).unwrap();
    let Pixels::Grey(g) = &f.pixels else { panic!() };
    for i in 0..64 {
        let want = if i < 16 {
            0
        } else {
            (i as i64 * 50 + 7) & 0xFFF
        };
        assert_eq!(g.get(i), Some(want), "pixel {i}");
    }
    let codes: Vec<String> = o
        .get(0x0012_0064)
        .unwrap()
        .items()
        .iter()
        .map(|i| i.text(0x0008_0100).unwrap().into_owned())
        .collect();
    assert!(codes.iter().any(|c| c == "113101"));
}

fn owned(o: &Object) -> Object<'static> {
    Object {
        elements: o
            .elements
            .iter()
            .map(|e| write::Elem {
                tag: e.tag,
                vr: e.vr,
                value: match &e.value {
                    Value::Bytes(b) => Value::Bytes(b.to_vec().into()),
                    Value::Items(i) => Value::Items(i.iter().map(owned).collect()),
                    Value::Fragments {
                        offset_table,
                        fragments,
                    } => Value::Fragments {
                        offset_table: offset_table.to_vec().into(),
                        fragments: fragments.iter().map(|f| f.to_vec().into()).collect(),
                    },
                },
            })
            .collect(),
    }
}

/// Samples of a frame as integers, colour interleaved.
fn samples(f: &Frame) -> Vec<i64> {
    match &f.pixels {
        Pixels::Grey(g) => (0..g.len()).map(|i| g.get(i).unwrap_or(0)).collect(),
        Pixels::Rgb8(v) => v.iter().map(|&x| x as i64).collect(),
        Pixels::Rgb16(v) => v.iter().map(|&x| x as i64).collect(),
    }
}

/// Identifying values of a file: names, IDs, dates (the strings that must not survive).
fn identifying(o: &Object) -> HashSet<String> {
    let mut all = Vec::new();
    texts(o, &mut all);
    let id_tags = [
        0x0010_0020u32,
        0x0010_1000,
        0x0008_0050,
        0x0020_0010,
        0x0010_0021,
        0x0038_0010,
        0x0040_0009,
        0x0040_1001,
        0x0040_2016,
        0x0040_2017,
        0x0040_0253,
        0x0018_1000,
    ];
    all.into_iter()
        .filter(|(tag, vr, v)| {
            v.len() >= 4
                && (matches!(vr, Vr::PN | Vr::DA | Vr::DT) || id_tags.contains(tag))
                && v != "ANONYMOUS"
                && !v.starts_with("1900")
        })
        .flat_map(|(_, _, v)| {
            // Each value of a multi-valued element, and each name component.
            v.split(['\\', '^', '='])
                .map(|s| s.trim().to_owned())
                .filter(|s| s.len() >= 4)
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn corpus_round_trips_through_the_writer() {
    let Some(files) = corpus_files() else {
        eprintln!("DICOM corpus absent; skipping");
        return;
    };
    let mut n = 0;
    for p in &files {
        let Ok(data) = std::fs::read(p) else { continue };
        let Ok(d) = Dicom::parse(&data) else { continue };
        if d.truncated {
            continue;
        }
        let o = Object::from_dataset(&d.dataset, d.buf(&data));
        let encapsulated = matches!(
            o.get(0x7FE0_0010).map(|e| &e.value),
            Some(Value::Fragments { .. })
        );
        let ts = if encapsulated {
            d.transfer_syntax.uid.clone()
        } else {
            write::EXPLICIT_LE.to_owned()
        };
        let out = write::part10(&o, &FileMeta::of(&o, &ts))
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let d2 = Dicom::parse(&out).unwrap_or_else(|e| panic!("{}: reread {e}", p.display()));
        let o2 = Object::from_dataset(&d2.dataset, d2.buf(&out));
        // The same elements and values, but for those the writer drops (group lengths).
        let keep = |o: &Object| -> Vec<(u32, Vec<u8>)> {
            let mut v = Vec::new();
            flat(o, &mut v);
            v
        };
        assert_eq!(keep(&o), keep(&o2), "{}", p.display());
        assert_eq!(d.pixel, d2.pixel, "{}", p.display());
        if d.frame_count() > 0
            && let Ok(f) = d.decode_frame(&data, 0, None)
        {
            let f2 = d2
                .decode_frame(&out, 0, None)
                .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert!(f == f2, "{}: frame 0 differs", p.display());
        }
        n += 1;
    }
    eprintln!("{n} files round-tripped");
    assert!(n > 100);
}

/// Elements in order with their padded values, sequences flattened, dropped tags skipped.
fn flat(o: &Object, out: &mut Vec<(u32, Vec<u8>)>) {
    for e in &o.elements {
        if e.tag & 0xFFFF == 0 || e.tag == 0x0008_0001 || e.tag >> 16 == 0 || e.tag >> 16 == 2 {
            continue;
        }
        match &e.value {
            Value::Bytes(b) => {
                let mut b = b.to_vec();
                if b.len() % 2 == 1 {
                    b.push(if e.vr.is_text() && e.vr != Vr::UI {
                        b' '
                    } else {
                        0
                    });
                }
                out.push((e.tag, b));
            }
            Value::Items(items) => {
                out.push((e.tag, vec![items.len() as u8]));
                for i in items {
                    flat(i, out);
                }
            }
            Value::Fragments {
                offset_table,
                fragments,
            } => {
                out.push((e.tag, offset_table.to_vec()));
                for f in fragments {
                    let mut f = f.to_vec();
                    if f.len() % 2 == 1 {
                        f.push(0);
                    }
                    out.push((e.tag, f));
                }
            }
        }
    }
}

#[test]
fn corpus_anonymises() {
    let Some(files) = corpus_files() else {
        eprintln!("DICOM corpus absent; skipping");
        return;
    };
    let s = Session::new();
    let opts = Options::default();
    let (mut done, mut masked, mut refused) = (0, 0, 0);
    for p in &files {
        let Ok(data) = std::fs::read(p) else { continue };
        let Ok(d) = Dicom::parse(&data) else { continue };
        if d.truncated {
            continue;
        }
        let src = Object::from_dataset(&d.dataset, d.buf(&data));
        let bad = identifying(&src);
        let r = Request {
            options: &opts,
            session: &s,
            masks: &[],
            allow_burned_in: true,
            cancel: None,
        };
        let out = match anon::anonymise(&data, &r) {
            Ok(o) => o,
            Err(AnonError::Document) => {
                refused += 1;
                continue;
            }
            Err(e) => panic!("{}: {e}", p.display()),
        };
        let d2 = Dicom::parse(&out.bytes).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let o2 = Object::from_dataset(&d2.dataset, d2.buf(&out.bytes));
        let mut all = Vec::new();
        texts(&o2, &mut all);
        for (tag, vr, v) in &all {
            assert!(
                tag >> 16 & 1 == 0 || tag >> 16 == 0xFFFF,
                "{}: private {tag:08X}",
                p.display()
            );
            assert!(
                !(0x6000..=0x601E).contains(&(tag >> 16)),
                "{}: overlay {tag:08X}",
                p.display()
            );
            match vr {
                Vr::PN => assert!(
                    v.is_empty() || v == "ANONYMOUS" || v.starts_with("ANON"),
                    "{}: {tag:08X} PN {v:?}",
                    p.display()
                ),
                Vr::DA | Vr::DT => assert!(
                    v.is_empty() || v.starts_with("19000101"),
                    "{}: {tag:08X} date {v:?}",
                    p.display()
                ),
                _ => {}
            }
            // Code values are concepts, not identifiers (a requested procedure ID may well
            // equal the procedure's code).
            if matches!(tag, 0x0008_0100 | 0x0008_0119 | 0x0008_0120) {
                continue;
            }
            for part in v.split(['\\', '^', '=']) {
                assert!(
                    !bad.contains(part.trim()),
                    "{}: {tag:08X} keeps {part:?}",
                    p.display()
                );
            }
        }
        // Pixels unchanged without masks.
        if d.frame_count() > 0
            && let Ok(f) = d.decode_stored(&data, 0, None)
        {
            let f2 = d2.decode_stored(&out.bytes, 0, None).unwrap();
            assert!(f == f2, "{}: pixels changed", p.display());
            // Masked: a quarter of the image in the top-left corner (even, for 4:2:2).
            let (w, h) = (f.width, f.height);
            let rect = Rect::new(0, 0, (w / 4) & !1, h / 4);
            if rect.w > 0 && rect.h > 0 && d.frame_count() <= 64 {
                let m = [rect];
                let r = Request { masks: &m, ..r };
                let out = anon::anonymise(&data, &r)
                    .unwrap_or_else(|e| panic!("{}: masked {e}", p.display()));
                let d3 = Dicom::parse(&out.bytes).unwrap();
                let n = d.frame_count();
                for k in [0, n - 1] {
                    let a = samples(&d.decode_stored(&data, k, None).unwrap());
                    let fb = d3.decode_stored(&out.bytes, k, None).unwrap();
                    let spp = samples(&fb).len() / (w * h) as usize;
                    let b = samples(&fb);
                    for y in 0..h {
                        for x in 0..w {
                            let i = (y * w + x) as usize * spp;
                            for c in 0..spp {
                                if rect.contains(x, y) {
                                    assert_eq!(b[i + c], 0, "{}: masked ({x},{y})", p.display());
                                } else if spp == a.len() / (w * h) as usize {
                                    assert_eq!(
                                        a[i + c],
                                        b[i + c],
                                        "{}: ({x},{y}) frame {k}",
                                        p.display()
                                    );
                                }
                            }
                        }
                    }
                }
                masked += 1;
            }
        }
        done += 1;
    }
    eprintln!("{done} anonymised, {masked} masked, {refused} documents refused");
    assert!(done > 100);
}

/// `sample` with its patient and study changed.
fn sample_of(sop: &str, patient: &str, study: &str) -> Vec<u8> {
    let (_, mut o) = obj(&sample(sop, None));
    o.put_text(0x0010_0020, Vr::LO, patient);
    o.put_text(0x0020_000D, Vr::UI, study);
    o.put_text(0x0020_000E, Vr::UI, &format!("{study}.1"));
    write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE)).unwrap()
}

fn fresh_dir(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// Checks a written file-set with our DICOMDIR reader; returns the number of instances.
fn check_disc(root: &Path) -> usize {
    let dir = std::fs::read(root.join("DICOMDIR")).unwrap();
    let entries = series::read_dicomdir(&dir).unwrap();
    for e in &entries {
        assert!(e.file.len() <= 8, "{:?}", e.file);
        for c in &e.file {
            assert!(
                !c.is_empty()
                    && c.len() <= 8
                    && c.bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'),
                "{c:?}"
            );
        }
        let p = e.path(root);
        let data = std::fs::read(&p).unwrap_or_else(|err| panic!("{}: {err}", p.display()));
        let (_, o) = obj(&data);
        assert_eq!(o.text(0x0008_0018).as_deref(), Some(e.sop_uid.as_str()));
        assert_eq!(o.text(0x0020_000D).as_deref(), Some(e.study_uid.as_str()));
        assert_eq!(o.text(0x0020_000E).as_deref(), Some(e.series_uid.as_str()));
    }
    assert!(root.join("README.TXT").is_file() && root.join("INDEX.HTM").is_file());
    oracle(root, entries.len());
    entries.len()
}

/// pydicom's FileSet as an oracle, when `SPECULAR_DICOM_ORACLE_PYTHON` names a Python with
/// pydicom (`gen/anon_oracle.py`).
fn oracle(root: &Path, instances: usize) {
    let Some(py) = std::env::var_os("SPECULAR_DICOM_ORACLE_PYTHON") else {
        return;
    };
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("gen/anon_oracle.py");
    let out = std::process::Command::new(py)
        .arg("-I")
        .arg(script)
        .arg(root)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "oracle failed: {text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains(&format!("instances {instances}\n")), "{text}");
}

#[test]
fn patient_disc_folder() {
    let root = fresh_dir("pdi-synthetic");
    let s = session();
    let opts = Options::default();
    let mut disc = pdi::Disc::create(&root, true).unwrap();
    let files = [
        sample_of("1.2.3.100.1", "P1", "1.2.3.10"),
        sample_of("1.2.3.100.2", "P1", "1.2.3.10"),
        sample_of("1.2.3.100.3", "P1", "1.2.3.11"),
        sample_of("1.2.3.100.4", "P2", "1.2.3.12"),
    ];
    for f in &files {
        let a = anon::anonymise(f, &req(&opts, &s, &[])).unwrap();
        assert!(disc.add(&a.bytes).unwrap().is_some());
    }
    // The same instance again is skipped.
    let again = anon::anonymise(&files[0], &req(&opts, &s, &[])).unwrap();
    assert!(disc.add(&again.bytes).unwrap().is_none());
    let w = disc.finish(None).unwrap();
    assert_eq!(
        (w.patients, w.studies, w.series, w.instances, w.duplicates),
        (2, 3, 3, 4, 1)
    );
    assert_eq!(check_disc(&root), 4);
    // A folder that holds a file-set is not written over.
    assert!(matches!(
        pdi::Disc::create(&root, true),
        Err(pdi::PdiError::NotEmpty)
    ));
    let index = std::fs::read_to_string(root.join("INDEX.HTM")).unwrap();
    assert!(!index.contains("Doe") && !index.contains("MRN-0042"));
}

#[test]
fn corpus_patient_disc_folder() {
    let Some(files) = corpus_files() else {
        eprintln!("DICOM corpus absent; skipping");
        return;
    };
    let root = fresh_dir("pdi-corpus");
    let s = Session::new();
    let opts = Options::default();
    let mut disc = pdi::Disc::create(&root, true).unwrap();
    let mut added = 0;
    for p in &files {
        let Ok(data) = std::fs::read(p) else { continue };
        let Ok(d) = Dicom::parse(&data) else { continue };
        if d.truncated {
            continue;
        }
        let r = Request {
            options: &opts,
            session: &s,
            masks: &[],
            allow_burned_in: true,
            cancel: None,
        };
        let Ok(a) = anon::anonymise(&data, &r) else {
            continue;
        };
        match disc.add(&a.bytes) {
            Ok(at) => added += at.is_some() as usize,
            // Test files without one cannot be referenced from a DICOMDIR.
            Err(pdi::PdiError::NoInstanceUid) => {}
            Err(e) => panic!("{}: {e}", p.display()),
        }
    }
    let w = disc.finish(None).unwrap();
    assert_eq!(w.instances, added);
    assert_eq!(check_disc(&root), added);
    eprintln!("{w:?}");
    let _ = std::fs::remove_dir_all(&root);
}
