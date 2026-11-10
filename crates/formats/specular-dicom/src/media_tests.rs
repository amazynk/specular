//! Patient media on synthetic discs: detection, DICOMDIR records with their quirks (File IDs
//! in the wrong case, ISO 9660 versions, `/` separators, missing files, icons, character
//! sets) and the header scan of discs without a DICOMDIR. No real patient data.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use super::*;
use crate::parse::PIXEL_DATA;
use crate::tests::{W, file};

const EXPLICIT: &str = "1.2.840.10008.1.2.1";

/// A fresh folder below TMPDIR.
fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("specular-media-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn put(root: &Path, rel: &str, data: &[u8]) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, data).unwrap();
}

/// A path as the file system tells names apart: `/`-separated, and on Windows (case-insensitive,
/// trailing dots dropped) lower case without trailing dots. The fixtures spell names the way
/// discs do; there those spellings name the files the resolver finds under their real names.
fn key(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        s.split('/')
            .map(|c| c.trim_end_matches('.').to_lowercase())
            .collect::<Vec<_>>()
            .join("/")
    } else if cfg!(target_os = "macos") {
        // APFS ignores case too (and keeps trailing dots).
        s.to_lowercase()
    } else {
        s
    }
}

struct Leaf<'a> {
    study: &'a str,
    date: &'a str,
    series: &'a str,
    modality: &'a str,
    number: &'a str,
    instance: &'a str,
}

/// A 2 x 2 image of a synthetic patient (Latin-1 name).
fn leaf(l: &Leaf) -> Vec<u8> {
    let mut w = W::new(true, false);
    w.s(0x0008_0005, b"CS", "ISO_IR 100")
        .s(0x0008_0018, b"UI", &format!("{}.{}", l.series, l.instance))
        .s(0x0008_0020, b"DA", l.date)
        .s(0x0008_0060, b"CS", l.modality)
        .s(0x0008_1030, b"LO", "Synthetic study")
        .s(0x0008_103E, b"LO", &format!("{} series", l.modality))
        .raw(0x0010_0010, b"PN", b"M\xfcller^Test")
        .s(0x0010_0020, b"LO", "SYN-1")
        .s(0x0010_0030, b"DA", "19700101")
        .s(0x0020_000D, b"UI", l.study)
        .s(0x0020_000E, b"UI", l.series)
        .s(0x0020_0011, b"IS", l.number)
        .s(0x0020_0013, b"IS", l.instance)
        .image(2, 2, 1, "MONOCHROME2", (8, 8, 7), false)
        .raw(PIXEL_DATA, b"OB", &[0, 80, 160, 240]);
    file(EXPLICIT, &w.out)
}

/// An 8 x 8 icon item, a gradient.
fn icon() -> W {
    let mut w = W::new(true, false);
    w.image(8, 8, 1, "MONOCHROME2", (8, 8, 7), false).raw(
        PIXEL_DATA,
        b"OB",
        &(0..64u8).map(|i| i * 4).collect::<Vec<_>>(),
    );
    w
}

/// Records in sequence order: one patient, two studies; the newer has a CT series (series
/// icon, three images: one stored as `dicom/st1/ct1;1`, one as `DICOM/ST1/CT2.`, one missing)
/// and an MR series (icon on the image record, `/` separators); the older study has only a
/// presentation state.
fn dicomdir_bytes() -> Vec<u8> {
    let rec = |ty: &str, f: &dyn Fn(&mut W)| {
        let mut r = W::new(true, false);
        r.s(0x0004_1430, b"CS", ty);
        f(&mut r);
        r
    };
    let image = |id: &str, n: &str, with_icon: bool| {
        rec("IMAGE", &|r: &mut W| {
            r.s(0x0004_1500, b"CS", id)
                .s(0x0004_1511, b"UI", &format!("sop.{n}"))
                .s(0x0020_0013, b"IS", n);
            if with_icon {
                r.seq(0x0088_0200, &[icon()]);
            }
        })
    };
    let records = vec![
        rec("PATIENT", &|r: &mut W| {
            r.raw(0x0010_0010, b"PN", b"M\xfcller^Test")
                .s(0x0010_0020, b"LO", "SYN-1")
                .s(0x0010_0030, b"DA", "19700101");
        }),
        rec("STUDY", &|r: &mut W| {
            r.s(0x0008_0020, b"DA", "20240102")
                .raw(0x0008_1030, b"LO", b"Th\xf6rax")
                .s(0x0020_000D, b"UI", "1.2.9.1");
        }),
        rec("SERIES", &|r: &mut W| {
            r.s(0x0008_0060, b"CS", "CT")
                .s(0x0020_000E, b"UI", "1.2.9.1.1")
                .s(0x0020_0011, b"IS", "1")
                .seq(0x0088_0200, &[icon()]);
        }),
        image("DICOM\\ST1\\CT2", "2", false),
        image("DICOM\\ST1\\CT1", "1", false),
        image("DICOM\\ST1\\CT3", "3", false),
        rec("SERIES", &|r: &mut W| {
            r.s(0x0008_0060, b"CS", "MR")
                .s(0x0020_000E, b"UI", "1.2.9.1.2")
                .s(0x0020_0011, b"IS", "2");
        }),
        image("DICOM/ST1/MR1", "1", true),
        rec("STUDY", &|r: &mut W| {
            r.s(0x0008_0020, b"DA", "20230101")
                .s(0x0020_000D, b"UI", "1.2.9.2");
        }),
        rec("SERIES", &|r: &mut W| {
            r.s(0x0008_0060, b"CS", "PR")
                .s(0x0020_000E, b"UI", "1.2.9.2.1");
        }),
        rec("PRESENTATION", &|r: &mut W| {
            r.s(0x0004_1500, b"CS", "DICOM\\ST2\\PR1");
        }),
        // A top-level record outside any patient.
        rec("HANGING PROTOCOL", &|r: &mut W| {
            r.s(0x0004_1500, b"CS", "HP\\HP1");
        }),
    ];
    let mut w = W::new(true, false);
    w.s(0x0004_1130, b"CS", "SYNTHETIC")
        .s(0x0008_0005, b"CS", "ISO_IR 100")
        .seq_defined(0x0004_1220, &records);
    file(EXPLICIT, &w.out)
}

/// The disc: DICOMDIR, the files (one missing), an autorun file and a viewer.
fn disc(name: &str, with_dicomdir: bool) -> PathBuf {
    let root = tmp(name);
    let ct = |n| Leaf {
        study: "1.2.9.1",
        date: "20240102",
        series: "1.2.9.1.1",
        modality: "CT",
        number: "1",
        instance: n,
    };
    put(&root, "dicom/st1/ct1;1", &leaf(&ct("1")));
    put(&root, "DICOM/ST1/CT2.", &leaf(&ct("2")));
    let mr = Leaf {
        series: "1.2.9.1.2",
        modality: "MR",
        number: "2",
        ..ct("1")
    };
    put(&root, "DICOM/ST1/MR1", &leaf(&mr));
    put(&root, "AUTORUN.INF", b"[autorun]\r\nopen=VIEWER.EXE\r\n");
    put(&root, "VIEWER.EXE", &leaf(&ct("9")));
    put(&root, "README", &[b'x'; 300]);
    if with_dicomdir {
        put(&root, "DICOMDIR", &dicomdir_bytes());
    }
    root
}

#[test]
fn detects_media() {
    let root = disc("detect", true);
    let dd = root.join("DICOMDIR");
    assert_eq!(detect(&root), Some(Detected::Dicomdir(dd.clone())));
    assert_eq!(detect(&dd), Some(Detected::Dicomdir(dd)));
    // A DICOMDIR one level down (IHE_PDI/), from the medium's root.
    let pdi = tmp("detect-pdi");
    put(&pdi, "IHE_PDI/dicomdir", &dicomdir_bytes());
    put(&pdi, "INDEX.HTM", b"<html></html>");
    assert_eq!(
        detect(&pdi),
        Some(Detected::Dicomdir(pdi.join("IHE_PDI/dicomdir")))
    );
    // No DICOMDIR: extensionless DICOM files, two levels down.
    let files = disc("detect-files", false);
    assert_eq!(detect(&files), Some(Detected::Files(files.clone())));
    // Photos are not a medium; neither is a single image.
    let photos = tmp("detect-photos");
    put(&photos, "a.jpg", &[0xFF, 0xD8, 0xFF]);
    put(&photos, "sub/README", &[b'x'; 300]);
    assert_eq!(detect(&photos), None);
    assert_eq!(detect(&files.join("DICOM/ST1/MR1")), None);
    for d in [root, pdi, files, photos] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn resolves_file_ids_despite_quirks() {
    let root = tmp("resolve");
    put(&root, "dicom/st1/im1;1", b"1");
    put(&root, "DICOM/ST1/IM2.", b"2");
    put(&root, "DICOM/ST1/Im3", b"3");
    let mut r = Resolver::default();
    let get = |r: &mut Resolver, id: &str| r.resolve(&root, &file_id(id)).map(|p| key(&p));
    let want = |rel: &str| Some(key(&root.join(rel)));
    assert_eq!(get(&mut r, "DICOM\\ST1\\IM1"), want("dicom/st1/im1;1"));
    assert_eq!(get(&mut r, "DICOM\\ST1\\IM2"), want("DICOM/ST1/IM2."));
    assert_eq!(get(&mut r, "dicom/st1/IM3;1"), want("DICOM/ST1/Im3"));
    // `.` and `..` are dropped: nothing outside the medium.
    assert_eq!(
        get(&mut r, "..\\DICOM\\.\\ST1\\IM1"),
        want("dicom/st1/im1;1")
    );
    assert_eq!(get(&mut r, "DICOM\\ST1\\IM4"), None);
    // A folder is not a file, nor a file a folder.
    assert_eq!(get(&mut r, "DICOM\\ST1"), None);
    assert_eq!(get(&mut r, "DICOM\\ST1\\IM1\\X"), None);
    assert_eq!(file_id(" A\\B \\\\C\0"), ["A", "B", "C"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn reads_a_dicomdir_with_quirks() {
    let root = disc("dicomdir", true);
    let m = read_dicomdir(&root.join("DICOMDIR")).unwrap();
    assert_eq!(m.dicomdir, Some(root.join("DICOMDIR")));
    assert_eq!(m.patients.len(), 1, "{m:?}");
    let p = &m.patients[0];
    assert_eq!(p.name, "Müller^Test");
    assert_eq!(
        (p.id.as_str(), p.birth_date.as_str()),
        ("SYN-1", "19700101")
    );
    // Patient data stays out of Debug output.
    let dbg = format!("{m:?}");
    assert!(!dbg.contains("Müller") && !dbg.contains("SYN-1") && !dbg.contains("1970"));
    // Newest study first.
    assert_eq!(p.studies.len(), 2);
    let st = &p.studies[0];
    assert_eq!(
        (st.date.as_str(), st.description.as_str()),
        ("20240102", "Thörax")
    );
    assert_eq!(st.modalities(), ["CT", "MR"]);
    let ct = &st.series[0];
    assert_eq!((ct.modality.as_str(), ct.number), ("CT", Some(1)));
    let names: Vec<_> = ct
        .images
        .iter()
        .map(|f| {
            (
                f.instance_number,
                f.present,
                key(f.path.strip_prefix(&root).unwrap()),
            )
        })
        .collect();
    let k = |rel: &str| key(Path::new(rel));
    assert_eq!(
        names,
        [
            (Some(1), true, k("dicom/st1/ct1;1")),
            (Some(2), true, k("DICOM/ST1/CT2.")),
            (Some(3), false, k("DICOM/ST1/CT3")),
        ]
    );
    assert_eq!(ct.missing(), 1);
    assert_eq!(m.missing, 2, "CT3 and the presentation state");
    assert_eq!(
        ct.middle().map(key),
        Some(key(&root.join("DICOM/ST1/CT2.")))
    );
    let icon = ct.icon.as_ref().unwrap();
    assert_eq!((icon.width, icon.height, icon.rgba.len()), (8, 8, 256));
    let mr = &st.series[1];
    assert_eq!(mr.modality, "MR");
    assert!(mr.images[0].present);
    assert!(mr.icon.is_some(), "the image record's icon");
    // The older study keeps its presentation-only series; the hanging protocol is not
    // anyone's.
    let old = &p.studies[1];
    assert_eq!(old.date, "20230101");
    assert!(old.series[0].images.is_empty());
    assert_eq!(old.series[0].others.len(), 1);
    assert_eq!((m.study_count(), m.series_count()), (2, 3));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scans_a_disc_without_dicomdir() {
    let root = disc("scan", false);
    let mut m = Medium::default();
    let mut calls = Vec::new();
    let cancel = AtomicBool::new(false);
    scan(
        &root,
        ScanLimits::default(),
        &cancel,
        &mut |batch, progress| {
            calls.push(progress);
            m.add(batch);
        },
    )
    .unwrap();
    let last = *calls.last().unwrap();
    // CT1, CT2, MR1 and README; not the viewer or the autorun file.
    assert_eq!(
        (last.done, last.total, last.skipped, last.capped),
        (4, 4, 1, false)
    );
    assert_eq!(calls[0].done, 0);
    assert_eq!(m.patients.len(), 1);
    let p = &m.patients[0];
    assert_eq!(p.name, "Müller^Test");
    let st = &p.studies[0];
    assert_eq!(st.description, "Synthetic study");
    assert_eq!(st.series.len(), 2);
    assert_eq!(st.series[0].images.len(), 2);
    assert_eq!(st.series[0].images[0].instance_number, Some(1));
    assert_eq!(st.series[1].description, "MR series");
    assert!(!format!("{:?}", calls).is_empty());
    // A capped scan says so; a cancelled one stops.
    let mut capped = Progress::default();
    let limits = ScanLimits {
        files: 2,
        ..ScanLimits::default()
    };
    scan(&root, limits, &cancel, &mut |_, p| capped = p).unwrap();
    assert!(capped.capped && capped.done == 2);
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(matches!(
        scan(&root, ScanLimits::default(), &cancel, &mut |_, _| {}),
        Err(Error::Cancelled)
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn thumbnails() {
    let img = Image {
        width: 4,
        height: 2,
        pixels: Rendered::Gray16(vec![0, 0, 65535, 65535, 0, 0, 65535, 65535]),
        truncated: false,
    };
    let t = thumb(&img, 2);
    assert_eq!((t.width, t.height), (2, 1));
    assert_eq!(t.rgba, [0, 0, 0, 255, 255, 255, 255, 255]);
    // Never larger than the image.
    assert_eq!(thumb(&img, 64).width, 4);
    let root = disc("thumb", false);
    let cancel = AtomicBool::new(false);
    let t = file_thumb(&root.join("DICOM/ST1/MR1"), 64, &cancel).unwrap();
    assert_eq!((t.width, t.height), (2, 2));
    assert!(file_thumb(&root.join("README"), 64, &cancel).is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn dicomdir_records_by_offsets() {
    let root = tmp("offsets");
    for offsets in [false, true] {
        let data = crate::series_tests::dicomdir(offsets);
        let m = dicomdir_medium(&data, &root, &mut Resolver::default()).unwrap();
        let st = &m.patients[0].studies[0];
        assert_eq!(st.uid, "S1");
        let uids: Vec<_> = st
            .series
            .iter()
            .map(|s| (s.uid.as_str(), s.images.len(), s.missing()))
            .collect();
        assert_eq!(uids, [("A", 2, 2), ("B", 1, 1)], "offsets {offsets}");
        assert_eq!(m.missing, 3);
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn person_names_for_display() {
    assert_eq!(display_name("DOE^JANE"), "DOE, JANE");
    assert_eq!(display_name("Doe^Jane^Ann^Dr^PhD"), "Doe, Dr Jane Ann, PhD");
    assert_eq!(display_name("Doe"), "Doe");
    assert_eq!(display_name("^Jane"), "Jane");
    assert_eq!(display_name("=山田^太郎=やまだ^たろう"), "山田, 太郎");
    assert_eq!(display_name(" ^ ^ "), "");
    assert_eq!(display_name(""), "");
}
