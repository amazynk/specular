//! Folder scans: synthetic series written to the target directory, and the DICOMDIR media of
//! the test corpus (`SPECULAR_DICOM_CORPUS`, else `tools/corpus/dicom`; skipped when absent).

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use specular_dicom::series::{self, ScanLimits, SeriesOrder};

/// Minimal explicit VR little endian writer.
struct W(Vec<u8>);

impl W {
    fn el(&mut self, tag: u32, vr: &[u8; 2], val: &[u8]) -> &mut W {
        let mut v = val.to_vec();
        if v.len() % 2 == 1 {
            v.push(if vr == b"UI" || vr == b"OB" || vr == b"OW" {
                0
            } else {
                b' '
            });
        }
        self.0.extend(((tag >> 16) as u16).to_le_bytes());
        self.0.extend((tag as u16).to_le_bytes());
        self.0.extend(vr);
        if matches!(vr, b"OB" | b"OW" | b"SQ" | b"UN" | b"UT") {
            self.0.extend([0, 0]);
            self.0.extend((v.len() as u32).to_le_bytes());
        } else {
            self.0.extend((v.len() as u16).to_le_bytes());
        }
        self.0.extend(v);
        self
    }
    fn s(&mut self, tag: u32, vr: &[u8; 2], s: &str) -> &mut W {
        self.el(tag, vr, s.as_bytes())
    }
    fn us(&mut self, tag: u32, v: u16) -> &mut W {
        self.el(tag, b"US", &v.to_le_bytes())
    }
}

fn slice(series: &str, sop: &str, z: f64) -> Vec<u8> {
    let mut meta = W(Vec::new());
    meta.s(0x0002_0010, b"UI", "1.2.840.10008.1.2.1");
    let mut out = vec![0u8; 128];
    out.extend(b"DICM");
    let mut g = W(Vec::new());
    g.el(0x0002_0000, b"UL", &(meta.0.len() as u32).to_le_bytes());
    out.extend(g.0);
    out.extend(meta.0);
    let mut w = W(Vec::new());
    w.s(0x0008_0018, b"UI", sop)
        .s(0x0008_0060, b"CS", "MR")
        .s(0x0020_000D, b"UI", "1.9")
        .s(0x0020_000E, b"UI", series)
        .s(0x0020_0032, b"DS", &format!("0\\0\\{z}"))
        .s(0x0020_0037, b"DS", "1\\0\\0\\0\\1\\0")
        .us(0x0028_0002, 1)
        .s(0x0028_0004, b"CS", "MONOCHROME2")
        .us(0x0028_0010, 64)
        .us(0x0028_0011, 64)
        .us(0x0028_0100, 16)
        .us(0x0028_0101, 16)
        .us(0x0028_0102, 15)
        .us(0x0028_0103, 0)
        .el(0x7FE0_0010, b"OW", &vec![0u8; 64 * 64 * 2]);
    out.extend(w.0);
    out
}

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn scans_a_folder_of_two_series() {
    let d = tmp("dicom-series-scan");
    // Names out of slice order; slice 7 missing; a copy of slice 3; a second series; a
    // file that is not DICOM.
    for k in 0..12 {
        if k == 7 {
            continue;
        }
        let name = format!("IM{}", (k * 5) % 12);
        std::fs::write(d.join(name), slice("A", &format!("a.{k}"), k as f64 * 2.5)).unwrap();
    }
    std::fs::write(d.join("copy.dcm"), slice("A", "a.3", 7.5)).unwrap();
    for k in 0..3 {
        std::fs::write(
            d.join(format!("B{k}")),
            slice("B", &format!("b.{k}"), k as f64),
        )
        .unwrap();
    }
    std::fs::write(d.join("notes.txt"), b"not dicom").unwrap();
    let open = d.join("IM5");
    let s = series::scan(&open, ScanLimits::default(), &AtomicBool::new(false)).unwrap();
    assert_eq!(s.headers, 15);
    assert_eq!(s.skipped, 1);
    assert_eq!(s.series.len(), 2);
    let a = &s.series[0];
    assert_eq!(a.series_uid, "A");
    assert_eq!(a.order, SeriesOrder::Position);
    assert_eq!(a.items.len(), 11);
    assert_eq!(a.duplicates, 1);
    assert_eq!(a.spacing, Some(2.5));
    assert_eq!(a.missing(), 1);
    let zs: Vec<f64> = a.items.iter().map(|i| i.along.unwrap()).collect();
    assert!(zs.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(s.series[1].items.len(), 3);
    // Bounded: the opened file is read even past the cap.
    let s = series::scan(
        &open,
        ScanLimits {
            files: 2,
            ..ScanLimits::default()
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(s.capped);
    assert_eq!(s.headers, 3);
    // Cancelled.
    assert!(series::scan(&open, ScanLimits::default(), &AtomicBool::new(true)).is_err());
}

fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_DICOM_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/dicom")
        });
    p.is_dir().then_some(p)
}

#[test]
fn corpus_dicomdirs() {
    let Some(root) = corpus() else {
        eprintln!("DICOM corpus absent; skipping");
        return;
    };
    let dir = root.join("pydicom/src/pydicom/data/test_files/dicomdirtests");
    if !dir.is_dir() {
        eprintln!("no dicomdirtests; skipping");
        return;
    }
    let read = |name: &str| {
        let data = std::fs::read(dir.join(name)).unwrap();
        series::read_dicomdir(&data).unwrap()
    };
    let plain = read("DICOMDIR");
    assert!(!plain.is_empty());
    let mut files: Vec<_> = plain.iter().map(|e| e.file.join("/")).collect();
    files.sort();
    // The same records whatever the encoding or record order.
    for other in ["DICOMDIR-reordered", "DICOMDIR-implicit", "DICOMDIR-bigEnd"] {
        let e = read(other);
        let mut f: Vec<_> = e.iter().map(|e| e.file.join("/")).collect();
        f.sort();
        assert_eq!(f, files, "{other}");
        for a in &e {
            let b = plain.iter().find(|b| b.file == a.file).unwrap();
            assert_eq!(
                (&a.study_uid, &a.series_uid),
                (&b.study_uid, &b.series_uid),
                "{other}"
            );
        }
    }
    // Every image record names a file that exists, with its series.
    for e in &plain {
        assert!(e.path(&dir).is_file(), "{:?}", e.file);
        assert!(!e.series_uid.is_empty());
    }
    // A file of a CT series: its series from the folder.
    let ct = dir.join("77654033/CT2/17106");
    let s = series::scan(&ct, ScanLimits::default(), &AtomicBool::new(false)).unwrap();
    assert_eq!(s.dicomdir.as_deref(), Some(dir.join("DICOMDIR").as_path()));
    let mine = s
        .series
        .iter()
        .find(|s| s.items.iter().any(|i| i.key == ct))
        .unwrap();
    assert_eq!(mine.items.len(), 4);
    eprintln!(
        "CT2: order {:?}, spacing {:?}, gaps {:?}",
        mine.order, mine.spacing, mine.gaps
    );
}

/// A sequence of one item with `body` (defined lengths).
fn seq(body: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend(0xFFFEu16.to_le_bytes());
    v.extend(0xE000u16.to_le_bytes());
    v.extend((body.len() as u32).to_le_bytes());
    v.extend(body);
    v
}

/// A Grayscale Softcopy Presentation State referring to image `refers`.
fn presentation(sop_uid: &str, refers: &str, label: &str, date: &str) -> Vec<u8> {
    let mut meta = W(Vec::new());
    meta.s(0x0002_0010, b"UI", "1.2.840.10008.1.2.1");
    let mut out = vec![0u8; 128];
    out.extend(b"DICM");
    let mut g = W(Vec::new());
    g.el(0x0002_0000, b"UL", &(meta.0.len() as u32).to_le_bytes());
    out.extend(g.0);
    out.extend(meta.0);
    let mut image = W(Vec::new());
    image
        .s(0x0008_1150, b"UI", "1.2.840.10008.5.1.4.1.1.4")
        .s(0x0008_1155, b"UI", refers);
    let mut series = W(Vec::new());
    series
        .el(0x0008_1140, b"SQ", &seq(&image.0))
        .s(0x0020_000E, b"UI", "A");
    let mut w = W(Vec::new());
    w.s(0x0008_0016, b"UI", "1.2.840.10008.5.1.4.1.1.11.1")
        .s(0x0008_0018, b"UI", sop_uid)
        .s(0x0008_0060, b"CS", "PR")
        .el(0x0008_1115, b"SQ", &seq(&series.0))
        .s(0x0020_000D, b"UI", "1.9")
        .s(0x0020_000E, b"UI", "P")
        .s(0x0070_0080, b"CS", label)
        .s(0x0070_0082, b"DA", date)
        .s(0x0070_0083, b"TM", "101500");
    out.extend(w.0);
    out
}

#[test]
fn presentation_states_are_kept_apart() {
    let d = tmp("dicom-series-gsps");
    for k in 0..3 {
        std::fs::write(
            d.join(format!("IM{k}")),
            slice("A", &format!("a.{k}"), k as f64),
        )
        .unwrap();
    }
    std::fs::write(
        d.join("PS1"),
        presentation("p.1", "a.1", "FIRST", "20260101"),
    )
    .unwrap();
    std::fs::write(
        d.join("PS2"),
        presentation("p.2", "a.1", "SECOND", "20260301"),
    )
    .unwrap();
    let s = series::scan(
        &d.join("IM0"),
        ScanLimits::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    // The states are not slices of a series.
    assert_eq!(s.series.len(), 1);
    assert_eq!(s.series[0].items.len(), 3);
    let mut found: Vec<_> = s
        .presentations
        .iter()
        .map(|(p, i)| {
            assert!(i.is_presentation_state());
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                i.references.clone(),
                i.presentation_label.clone(),
                i.presentation_created.clone(),
            )
        })
        .collect();
    found.sort();
    let want = |f: &str, l: &str, c: &str| {
        (
            f.to_owned(),
            vec!["a.1".to_owned()],
            l.to_owned(),
            c.to_owned(),
        )
    };
    assert_eq!(
        found,
        [
            want("PS1", "FIRST", "20260101101500"),
            want("PS2", "SECOND", "20260301101500"),
        ]
    );
}

/// One file of a whole-slide image: `frames` tiles, in series `series`.
fn slide_file(series: &str, sop: &str, frames: u32) -> Vec<u8> {
    let mut meta = W(Vec::new());
    meta.s(0x0002_0010, b"UI", "1.2.840.10008.1.2.1");
    let mut out = vec![0u8; 128];
    out.extend(b"DICM");
    let mut g = W(Vec::new());
    g.el(0x0002_0000, b"UL", &(meta.0.len() as u32).to_le_bytes());
    out.extend(g.0);
    out.extend(meta.0);
    let mut w = W(Vec::new());
    w.s(0x0008_0016, b"UI", series::WSI_SOP_CLASS)
        .s(0x0008_0018, b"UI", sop)
        .s(0x0008_0060, b"CS", "SM")
        .s(0x0020_000D, b"UI", "1.9")
        .s(0x0020_000E, b"UI", series)
        .s(0x0028_0008, b"IS", &frames.to_string())
        .us(0x0028_0002, 1)
        .s(0x0028_0004, b"CS", "MONOCHROME2")
        .us(0x0028_0010, 8)
        .us(0x0028_0011, 8)
        .us(0x0028_0100, 8)
        .us(0x0028_0101, 8)
        .us(0x0028_0102, 7)
        .us(0x0028_0103, 0)
        .el(0x7FE0_0010, b"OB", &vec![0u8; 64 * frames as usize]);
    out.extend(w.0);
    out
}

/// A whole-slide series is one slide in several files (levels, label, overview), not
/// slices to step through: the scan keeps one file of it, the opened one.
#[test]
fn a_slide_is_not_a_stack_of_slices() {
    let d = tmp("dicom-series-slide");
    for (name, frames) in [("level0", 16), ("level1", 4), ("level2", 1), ("label", 1)] {
        std::fs::write(
            d.join(format!("{name}.dcm")),
            slide_file("S", &format!("2.{name}"), frames),
        )
        .unwrap();
    }
    for z in 0..2 {
        std::fs::write(
            d.join(format!("mr{z}.dcm")),
            slice("M", &format!("3.{z}"), z as f64),
        )
        .unwrap();
    }
    let scan = |name: &str| {
        series::scan(
            &d.join(name),
            ScanLimits::default(),
            &AtomicBool::new(false),
        )
        .unwrap()
    };
    let s = scan("level1.dcm");
    assert_eq!(s.headers, 6);
    let slide = s.series.iter().find(|s| s.series_uid == "S").unwrap();
    assert_eq!(slide.items.len(), 1);
    assert_eq!(slide.items[0].key, d.join("level1.dcm"));
    // Other series are untouched.
    let mr = s.series.iter().find(|s| s.series_uid == "M").unwrap();
    assert_eq!(mr.items.len(), 2);
    // Opened from elsewhere, the slide is its full-resolution level.
    let s = scan("mr0.dcm");
    let slide = s.series.iter().find(|s| s.series_uid == "S").unwrap();
    assert_eq!(slide.items.len(), 1);
    assert_eq!(slide.items[0].key, d.join("level0.dcm"));
}

/// One file of a whole-slide image with its Image Type, Frame of Reference and total pixel
/// matrix (what gallery grouping reads).
fn slide_part(series: &str, sop: &str, kind: &str, frame_of_ref: &str, size: u32) -> Vec<u8> {
    let mut meta = W(Vec::new());
    meta.s(0x0002_0010, b"UI", "1.2.840.10008.1.2.1");
    let mut out = vec![0u8; 128];
    out.extend(b"DICM");
    let mut g = W(Vec::new());
    g.el(0x0002_0000, b"UL", &(meta.0.len() as u32).to_le_bytes());
    out.extend(g.0);
    out.extend(meta.0);
    let mut w = W(Vec::new());
    w.s(
        0x0008_0008,
        b"CS",
        &format!("ORIGINAL\\PRIMARY\\{kind}\\NONE"),
    )
    .s(0x0008_0016, b"UI", series::WSI_SOP_CLASS)
    .s(0x0008_0018, b"UI", sop)
    .s(0x0008_0060, b"CS", "SM")
    .s(0x0020_000D, b"UI", "1.9")
    .s(0x0020_000E, b"UI", series)
    .s(0x0020_0052, b"UI", frame_of_ref)
    .el(0x0048_0006, b"UL", &size.to_le_bytes())
    .el(0x0048_0007, b"UL", &size.to_le_bytes())
    .el(0x7FE0_0010, b"OB", &[0u8; 64]);
    out.extend(w.0);
    out
}

/// The gallery's one tile per slide: each slide's files together, the largest level first;
/// a label in a series of its own joins its slide by Frame of Reference; other DICOM files
/// and single-file slides stay apart.
#[test]
fn whole_slides_group_by_slide() {
    let d = tmp("dicom-whole-slides");
    let files: [(&str, Vec<u8>); 8] = [
        ("a_l1.dcm", slide_part("A", "2.1", "VOLUME", "F1", 1024)),
        ("a_l0.dcm", slide_part("A", "2.0", "VOLUME", "F1", 4096)),
        ("a_ov.dcm", slide_part("A", "2.2", "OVERVIEW", "F1", 512)),
        ("a_label.dcm", slide_part("AL", "2.3", "LABEL", "F1", 256)),
        ("b_l0.dcm", slide_part("B", "3.0", "VOLUME", "F2", 2048)),
        ("b_l1.dcm", slide_part("B", "3.1", "VOLUME", "F2", 512)),
        ("c_only.dcm", slide_part("C", "4.0", "VOLUME", "F3", 512)),
        ("mr0.dcm", slice("M", "5.0", 0.0)),
    ];
    let mut paths = Vec::new();
    for (name, data) in files {
        std::fs::write(d.join(name), data).unwrap();
        paths.push(d.join(name));
    }
    paths.push(d.join("missing.dcm"));
    let mut slides = series::whole_slides(&paths, &|| false);
    slides.sort();
    let names = |g: &Vec<PathBuf>| -> Vec<String> {
        g.iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    };
    assert_eq!(slides.len(), 2);
    assert_eq!(
        names(&slides[0]),
        ["a_l0.dcm", "a_l1.dcm", "a_ov.dcm", "a_label.dcm"]
    );
    assert_eq!(names(&slides[1]), ["b_l0.dcm", "b_l1.dcm"]);
    assert!(series::whole_slides(&paths, &|| true).is_empty());
}
