//! Series groups and tag rows from synthetic series, and the series folder's watch.

use std::path::PathBuf;

use specular_dicom::series::{self, Instance};
use specular_dicom::{TagRow, TagTree, TagValue};

use super::*;

fn slice(series: &str, sop: &str, z: f64) -> Instance {
    Instance {
        study_uid: "1.9".into(),
        series_uid: series.into(),
        sop_uid: sop.into(),
        sop_class: "1.2.840.10008.5.1.4.1.1.4".into(),
        modality: "MR".into(),
        series_number: Some(3),
        position: Some([0.0, 0.0, z]),
        orientation: Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
        frames: 1,
        ..Instance::default()
    }
}

fn set() -> Vec<Series<PathBuf>> {
    // Slices 2.5 mm apart, the fifth missing, one file twice.
    let mut found = Vec::new();
    for k in [0, 1, 2, 3, 5, 6] {
        found.push((
            PathBuf::from(format!("/s/IM{k}")),
            slice("A", &format!("a.{k}"), k as f64 * 2.5),
        ));
    }
    found.push((PathBuf::from("/s/copy"), slice("A", "a.2", 5.0)));
    found.push((PathBuf::from("/s/B0"), slice("B", "b.0", 0.0)));
    series::assemble(found)
}

#[test]
fn series_group_reports_slice_location_and_gaps() {
    let sets = set();
    assert_eq!(sets.len(), 2);
    let a = &sets[0];
    let g = series_group(a, 3, &a.items[3], 0, 2);
    assert_eq!(g.key, "series");
    let keys: Vec<&str> = g.items.iter().map(|i| i.key).collect();
    assert_eq!(
        keys,
        [
            "slice",
            "location",
            "spacing",
            "which",
            "order",
            "gaps",
            "duplicates"
        ]
    );
    let value = |k: &str| g.items.iter().find(|i| i.key == k).unwrap();
    assert!(value("slice").value.contains('4') && value("slice").value.contains('6'));
    assert!(value("location").value.contains("7.5"));
    assert!(value("spacing").value.contains("2.5"));
    assert!(value("gaps").detail.as_deref().unwrap().contains('4'));
    assert!(value("which").detail.as_deref().unwrap().contains("MR"));
}

#[test]
fn with_series_replaces_its_group() {
    let sets = set();
    let a = &sets[0];
    let mut r = MetadataReport::default();
    r.summary.push(SummaryGroup {
        key: "medical",
        title: "Medical".into(),
        items: Vec::new(),
    });
    r.summary.push(SummaryGroup {
        key: "file",
        title: "File".into(),
        items: Vec::new(),
    });
    let once = with_series(&r, Some(series_group(a, 0, &a.items[0], 0, 2)));
    let twice = with_series(&once, Some(series_group(a, 1, &a.items[1], 0, 2)));
    let keys: Vec<&str> = twice.summary.iter().map(|g| g.key).collect();
    assert_eq!(keys, ["medical", "series", "file"]);
    let none = with_series(&twice, None);
    assert_eq!(none.summary.len(), 2);
}

#[test]
fn dicom_names() {
    for (p, yes) in [
        ("/a/b.dcm", true),
        ("/a/IM0001", true),
        ("/a/x.IMA", true),
        ("/a/x.jpg", false),
        ("/a/DICOMDIR", true),
    ] {
        assert_eq!(is_dicom_name(std::path::Path::new(p)), yes, "{p}");
    }
}

#[test]
fn tag_lines_localise_values() {
    let row = |depth, tag, value, identifying| TagRow {
        depth,
        tag,
        keyword: if tag == 0 { "" } else { "Keyword" },
        vr: if tag == 0 { "" } else { "LO" },
        length: 10,
        value,
        descendants: 0,
        identifying,
        meta: false,
    };
    let mut seq = row(0, 0x0008_1140, TagValue::Items(1), false);
    seq.descendants = 2;
    seq.vr = "SQ";
    let mut item = row(1, 0, TagValue::Item(1), false);
    item.descendants = 1;
    let tree = TagTree {
        rows: vec![
            row(
                0,
                0x0010_0010,
                TagValue::Text {
                    text: "Doe^Jane".into(),
                    more: false,
                },
                true,
            ),
            seq,
            item,
            row(
                2,
                0x0028_1201,
                TagValue::Binary {
                    hex: "00 01".into(),
                    len: 512,
                },
                false,
            ),
        ],
        omitted: 3,
    };
    let t = tag_lines(&tree, "f.dcm".into());
    assert_eq!(t.rows.len(), 4);
    assert_eq!(t.rows[0].tag, "(0010,0010)");
    assert!(t.rows[0].identifying);
    assert_eq!(t.rows[1].length, "");
    assert_eq!(t.rows[1].descendants, 2);
    assert_eq!(t.rows[2].tag, "");
    assert!(t.rows[2].keyword.contains('1'));
    assert!(t.rows[3].value.contains("512") && t.rows[3].value.contains("00 01"));
    assert!(t.note.is_some());
    // The value is not in what Debug prints.
    assert!(!format!("{:?}", t.rows[0]).contains("Doe"));
}

/// A whole slide's files are one image to the viewer: it steps over all but the file opened
/// (for its own slide) or the full-resolution level (for the others in the folder).
#[test]
fn the_viewer_steps_over_a_slides_other_files() {
    let p = |s: &str| PathBuf::from(format!("/w/{s}"));
    let slides = vec![
        vec![p("a-full"), p("a-half"), p("a-label")],
        vec![p("b-full"), p("b-quarter")],
    ];
    let hidden = slide_members(&slides, &p("a-half"));
    let mut v: Vec<_> = hidden.into_iter().collect();
    v.sort();
    assert_eq!(v, [p("a-full"), p("a-label"), p("b-quarter")]);
    assert!(slide_members(&[], &p("x")).is_empty());
    assert_eq!(slide_members(&slides, &p("other")).len(), 3);
}

/// In series mode a relist is read again only when files came or went: the first listing
/// (the scan was quicker) and the same listing again change nothing.
#[test]
fn a_changed_relist_reads_the_series_again() {
    let p = |s: &str| PathBuf::from(format!("/s/{s}"));
    let before = vec![p("IM0"), p("IM1"), p("IM2")];
    let print = listing_print(&before);
    assert!(!relist_changes(None, print));
    assert!(!relist_changes(Some(print), listing_print(&before.clone())));
    let mut added = before.clone();
    added.push(p("IM3"));
    assert!(relist_changes(Some(print), listing_print(&added)));
    assert!(relist_changes(Some(print), listing_print(&before[..2])));
    // Order counts: a listing is shown in its order.
    let swapped = vec![p("IM1"), p("IM0"), p("IM2")];
    assert!(relist_changes(Some(print), listing_print(&swapped)));
}

/// Series mode watches its folder: slices copied in reach the series once they settle;
/// a sidecar written next to them does not read the series again.
#[test]
fn slices_copied_into_the_series_folder_are_seen() {
    use std::time::{Duration, Instant};
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("series-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("IM0"), b"slice").unwrap();
    let w = TreeWatch::start("dcm-watch-test", &dir, 0, false, Arc::new(|| {}));
    let wait_for = |what: &dyn Fn() -> bool| {
        let end = Instant::now() + Duration::from_secs(20);
        while !what() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait_for(&|| w.ready() || w.failed());
    assert!(w.ready() && !w.failed());
    let settle = |w: &TreeWatch| {
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end {
            if let Some(ch) = w.take(Instant::now()) {
                return Some(ch);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    };
    // Three slices written one after another: one batch once they settle.
    for k in 1..=3 {
        std::fs::write(dir.join(format!("IM{k}.dcm")), b"slice").unwrap();
    }
    let ch = settle(&w).expect("the copied slices are seen");
    assert!(series_changes(&ch), "{ch:?}");
    assert!(ch.paths.iter().any(|p| p.ends_with("IM3.dcm")), "{ch:?}");
    // A sidecar alone changes nothing.
    std::fs::write(dir.join("IM1.dcm.xmp"), b"<x/>").unwrap();
    let ch = settle(&w).expect("the sidecar is seen");
    assert!(!series_changes(&ch), "{ch:?}");
    // A slice removed reads the series again.
    std::fs::remove_file(dir.join("IM2.dcm")).unwrap();
    assert!(series_changes(&settle(&w).expect("the removal is seen")));
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A series a DICOMDIR names has files in other folders of the medium: the watch covers the
/// medium (it watched only the opened file's folder); a plain series folder stays shallow.
#[test]
fn a_dicomdir_series_is_watched_through_its_medium() {
    let dir = PathBuf::from("/media/cd/DICOM/S1/E1");
    let media = PathBuf::from("/media/cd");
    assert_eq!(watch_target(&dir, None), (dir.clone(), 0));
    assert_eq!(
        watch_target(&dir, Some(&media)),
        (media.clone(), MEDIA_WATCH_DEPTH)
    );
    assert_eq!(
        watch_target(&media, Some(&media)),
        (media.clone(), MEDIA_WATCH_DEPTH)
    );
    // A DICOMDIR that is not above the folder does not widen the watch.
    let other = PathBuf::from("/elsewhere");
    assert_eq!(watch_target(&dir, Some(&other)), (dir.clone(), 0));
    // The medium's files are inside the watch's depth.
    assert!(dir.strip_prefix(&media).unwrap().components().count() <= MEDIA_WATCH_DEPTH as usize);
}
