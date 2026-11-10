//! Series assembly, header-only reads, DICOMDIR records and the tag tree on synthetic files.

use crate::series::{self, Instance, SeriesOrder, assemble};
use crate::tags::{TagTree, TagValue};
use crate::tests::{W, file};
use crate::{Dicom, parse::PIXEL_DATA};

const EXPLICIT: &str = "1.2.840.10008.1.2.1";
const AXIAL: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

fn ds(v: &[f64]) -> String {
    v.iter()
        .map(|x| format!("{x}"))
        .collect::<Vec<_>>()
        .join("\\")
}

struct Slice<'a> {
    study: &'a str,
    series: &'a str,
    sop: &'a str,
    number: Option<i64>,
    time: Option<&'a str>,
    position: Option<[f64; 3]>,
    orientation: [f64; 6],
}

impl Default for Slice<'_> {
    fn default() -> Self {
        Slice {
            study: "1.2.3",
            series: "1.2.3.4",
            sop: "",
            number: None,
            time: None,
            position: None,
            orientation: AXIAL,
        }
    }
}

/// A 2×2 CT slice file with `pixels` bytes of pixel data.
fn slice_file(s: &Slice, pixels: usize) -> Vec<u8> {
    let mut w = W::new(true, false);
    w.s(0x0008_0016, b"UI", "1.2.840.10008.5.1.4.1.1.2")
        .s(0x0008_0018, b"UI", s.sop);
    if let Some(t) = s.time {
        w.s(0x0008_0032, b"TM", t);
    }
    w.s(0x0008_0060, b"CS", "CT")
        .s(0x0008_103E, b"LO", "Axial")
        .s(0x0010_0010, b"PN", "Doe^Jane")
        .s(0x0020_000D, b"UI", s.study)
        .s(0x0020_000E, b"UI", s.series)
        .s(0x0020_0011, b"IS", "3");
    if let Some(n) = s.number {
        w.s(0x0020_0013, b"IS", &n.to_string());
    }
    if let Some(p) = s.position {
        w.s(0x0020_0032, b"DS", &ds(&p));
    }
    w.s(0x0020_0037, b"DS", &ds(&s.orientation))
        .image(2, 2, 1, "MONOCHROME2", (16, 16, 15), false)
        .raw(PIXEL_DATA, b"OW", &vec![0u8; pixels.max(8)]);
    file(EXPLICIT, &w.out)
}

fn inst(s: &Slice) -> Instance {
    let f = slice_file(s, 8);
    Instance::read(&f, true).unwrap().unwrap()
}

#[test]
fn header_read_stops_before_pixel_data() {
    let f = slice_file(
        &Slice {
            sop: "9.1",
            number: Some(7),
            position: Some([0.0, 0.0, 12.5]),
            ..Slice::default()
        },
        1 << 16,
    );
    let pd = f.len() - (1 << 16) - 12;
    // The prefix up to the pixel data's tag is enough: the value is never needed.
    let i = Instance::read(&f[..pd + 4], false).unwrap().unwrap();
    assert_eq!(i.sop_uid, "9.1");
    assert_eq!(i.instance_number, Some(7));
    assert_eq!(i.position, Some([0.0, 0.0, 12.5]));
    assert_eq!(i.frames, 1);
    assert_eq!(i.series_number, Some(3));
    // Cut before it: more is needed.
    assert_eq!(Instance::read(&f[..pd - 2], false).unwrap(), None);
    // Unless that is the whole file: no image.
    let i = Instance::read(&f[..pd], true).unwrap().unwrap();
    assert!(!i.has_image());
}

#[test]
fn orders_by_position_along_the_normal_not_instance_number() {
    // Oblique slices: the normal of this orientation is (0, -sin, cos) for a tilt about x.
    let a = 20f64.to_radians();
    let o = [1.0, 0.0, 0.0, 0.0, a.cos(), a.sin()];
    let n = [0.0, -a.sin(), a.cos()];
    let at = |d: f64| [5.0, 3.0 + n[1] * d, -7.0 + n[2] * d];
    let found: Vec<_> = [(30.0, 1), (10.0, 2), (20.0, 3), (0.0, 4)]
        .into_iter()
        .enumerate()
        .map(|(k, (d, num))| {
            let sop = format!("1.{k}");
            (
                k,
                inst(&Slice {
                    sop: &sop,
                    number: Some(num),
                    position: Some(at(d)),
                    orientation: o,
                    ..Slice::default()
                }),
            )
        })
        .collect();
    let s = assemble(found);
    assert_eq!(s.len(), 1);
    let s = &s[0];
    assert_eq!(s.order, SeriesOrder::Position);
    assert_eq!(
        s.items.iter().map(|i| i.key).collect::<Vec<_>>(),
        [3, 1, 2, 0]
    );
    let along: Vec<f64> = s.items.iter().map(|i| i.along.unwrap()).collect();
    assert!((along[1] - along[0] - 10.0).abs() < 1e-6, "{along:?}");
    assert!((s.spacing.unwrap() - 10.0).abs() < 1e-6);
    assert!(s.gaps.is_empty() && !s.irregular && s.tilt.is_none());
    assert_eq!(s.frames(), 4);
}

#[test]
fn mixed_series_duplicates_and_gaps() {
    let mut found = Vec::new();
    let mut k = 0;
    // Series A: z = 0, 2, 4, 10, 12 (two slices missing between 4 and 10), plus a copy of
    // the slice at 2 under another file name.
    for z in [0.0, 2.0, 4.0, 10.0, 12.0] {
        let sop = format!("a.{z}");
        found.push((
            k,
            inst(&Slice {
                series: "A",
                sop: &sop,
                position: Some([0.0, 0.0, z]),
                ..Slice::default()
            }),
        ));
        k += 1;
    }
    found.push((
        k,
        inst(&Slice {
            series: "A",
            sop: "a.2",
            position: Some([0.0, 0.0, 2.0]),
            ..Slice::default()
        }),
    ));
    k += 1;
    // Series B in the same folder, without positions: by Instance Number.
    for (num, sop) in [(3, "b.3"), (1, "b.1"), (2, "b.2")] {
        found.push((
            k,
            inst(&Slice {
                series: "B",
                sop,
                number: Some(num),
                ..Slice::default()
            }),
        ));
        k += 1;
    }
    let s = assemble(found);
    assert_eq!(s.len(), 2);
    let a = s.iter().find(|s| s.series_uid == "A").unwrap();
    assert_eq!(a.items.len(), 5);
    assert_eq!(a.duplicates, 1);
    assert_eq!(a.spacing, Some(2.0));
    assert_eq!(a.gaps.len(), 1);
    assert_eq!(a.gaps[0].after, 2);
    assert_eq!(a.gaps[0].missing, 2);
    assert_eq!(a.missing(), 2);
    let b = s.iter().find(|s| s.series_uid == "B").unwrap();
    assert_eq!(b.order, SeriesOrder::InstanceNumber);
    let sops: Vec<&str> = b
        .items
        .iter()
        .map(|i| i.instance.sop_uid.as_str())
        .collect();
    assert_eq!(sops, ["b.1", "b.2", "b.3"]);
}

#[test]
fn falls_back_to_acquisition_time_then_file_order() {
    let found = vec![
        (
            0,
            inst(&Slice {
                sop: "t.1",
                time: Some("101500.5"),
                ..Slice::default()
            }),
        ),
        (
            1,
            inst(&Slice {
                sop: "t.2",
                time: Some("10:15:00"),
                ..Slice::default()
            }),
        ),
        (
            2,
            inst(&Slice {
                sop: "t.3",
                time: Some("0959"),
                ..Slice::default()
            }),
        ),
    ];
    let s = assemble(found);
    assert_eq!(s[0].order, SeriesOrder::AcquisitionTime);
    assert_eq!(
        s[0].items.iter().map(|i| i.key).collect::<Vec<_>>(),
        [2, 1, 0]
    );
    let s = assemble(vec![
        (0, inst(&Slice::default())),
        (1, inst(&Slice::default())),
    ]);
    assert_eq!(s[0].order, SeriesOrder::Found);
    // No SOP Instance UIDs: never duplicates of each other.
    assert_eq!(s[0].items.len(), 2);
}

#[test]
fn reports_gantry_tilt_and_irregular_spacing() {
    // Axial slices stacked along a direction 15° off the normal (tilted gantry).
    let t = 15f64.to_radians();
    let found: Vec<_> = (0..6)
        .map(|k| {
            let d = k as f64 * 3.0;
            let sop = format!("g.{k}");
            (
                k,
                inst(&Slice {
                    sop: &sop,
                    position: Some([0.0, d * t.tan(), d]),
                    ..Slice::default()
                }),
            )
        })
        .collect();
    let s = &assemble(found)[0];
    assert!((s.tilt.unwrap() - 15.0).abs() < 0.01, "{:?}", s.tilt);
    let found: Vec<_> = [0.0, 2.0, 4.0, 6.5, 8.5, 10.5]
        .into_iter()
        .enumerate()
        .map(|(k, z)| {
            let sop = format!("i.{k}");
            (
                k,
                inst(&Slice {
                    sop: &sop,
                    position: Some([0.0, 0.0, z]),
                    ..Slice::default()
                }),
            )
        })
        .collect();
    let s = &assemble(found)[0];
    assert!(s.irregular && s.gaps.is_empty());
}

#[test]
fn enhanced_multi_frame_positions() {
    let mut w = W::new(true, false);
    let frame = |z: f64| {
        let mut pos = W::new(true, false);
        pos.s(0x0020_0032, b"DS", &ds(&[0.0, 0.0, z]));
        let mut it = W::new(true, false);
        it.seq(0x0020_9113, &[pos]);
        it
    };
    let mut orient = W::new(true, false);
    orient.s(0x0020_0037, b"DS", &ds(&AXIAL));
    let mut shared = W::new(true, false);
    shared.seq(0x0020_9116, &[orient]);
    w.s(0x0008_0018, b"UI", "m.1")
        .s(0x0020_000D, b"UI", "1")
        .s(0x0020_000E, b"UI", "2")
        .s(0x0028_0008, b"IS", "3")
        .image(2, 2, 1, "MONOCHROME2", (8, 8, 7), false)
        .seq(0x5200_9229, &[shared])
        .seq(0x5200_9230, &[frame(1.0), frame(2.0), frame(3.0)])
        .raw(PIXEL_DATA, b"OB", &[0; 12]);
    let f = file(EXPLICIT, &w.out);
    let i = Instance::read(&f, true).unwrap().unwrap();
    assert_eq!(i.frames, 3);
    assert_eq!(i.frame_positions.len(), 3);
    assert_eq!(i.position, Some([0.0, 0.0, 1.0]));
    assert_eq!(i.normal(), Some([0.0, 0.0, 1.0]));
    let s = assemble(vec![("f", i)]);
    assert_eq!(s[0].frames(), 3);
}

/// A DICOMDIR with two series of one study, its records in sequence order and linked by
/// offsets.
pub(crate) fn dicomdir(with_offsets: bool) -> Vec<u8> {
    let rec = |ty: &str, extra: &dyn Fn(&mut W)| {
        let mut r = W::new(true, false);
        if with_offsets {
            r.raw(0x0004_1400, b"UL", &[0; 4])
                .raw(0x0004_1420, b"UL", &[0; 4]);
        }
        r.s(0x0004_1430, b"CS", ty);
        extra(&mut r);
        r
    };
    let img = |path: &str, sop: &str| {
        let p = path.to_owned();
        let s = sop.to_owned();
        rec("IMAGE", &move |r: &mut W| {
            r.s(0x0004_1500, b"CS", &p).s(0x0004_1511, b"UI", &s);
        })
    };
    let records = vec![
        rec("PATIENT", &|r: &mut W| {
            r.s(0x0010_0020, b"LO", "ID1");
        }),
        rec("STUDY", &|r: &mut W| {
            r.s(0x0020_000D, b"UI", "S1");
        }),
        rec("SERIES", &|r: &mut W| {
            r.s(0x0020_000E, b"UI", "A");
        }),
        img("DIR\\A\\IM1", "a1"),
        img("DIR\\A\\IM2", "a2"),
        rec("SERIES", &|r: &mut W| {
            r.s(0x0020_000E, b"UI", "B");
        }),
        img("DIR\\B\\IM1", "b1"),
    ];
    let mut w = W::new(true, false);
    w.s(0x0004_1130, b"CS", "SET");
    if with_offsets {
        w.raw(0x0004_1200, b"UL", &[0; 4]);
    }
    w.seq_defined(0x0004_1220, &records);
    let mut f = file(EXPLICIT, &w.out);
    if with_offsets {
        // Patch the offsets: patient → study (lower) → series A (lower) → images (lower),
        // series A → series B (next) → image (lower).
        let d = Dicom::parse(&f).unwrap();
        let items = &d.dataset.get(0x0004_1220).unwrap().items;
        let start = |i: usize| (items[i].elements[0].value.start - 16) as u32;
        let put = |f: &mut Vec<u8>, i: usize, tag: u32, to: u32| {
            let e = items[i].get(tag).unwrap();
            f[e.value.clone()].copy_from_slice(&to.to_le_bytes());
        };
        let root = d.dataset.get(0x0004_1200).unwrap().value.clone();
        f[root].copy_from_slice(&start(0).to_le_bytes());
        put(&mut f, 0, 0x0004_1420, start(1));
        put(&mut f, 1, 0x0004_1420, start(2));
        put(&mut f, 2, 0x0004_1420, start(3));
        put(&mut f, 3, 0x0004_1400, start(4));
        put(&mut f, 2, 0x0004_1400, start(5));
        put(&mut f, 5, 0x0004_1420, start(6));
    }
    f
}

#[test]
fn dicomdir_records_by_offsets_and_in_order() {
    for offsets in [false, true] {
        let e = series::read_dicomdir(&dicomdir(offsets)).unwrap();
        assert_eq!(e.len(), 3, "offsets {offsets}");
        assert_eq!(e[0].file, ["DIR", "A", "IM1"]);
        assert_eq!(
            (e[0].study_uid.as_str(), e[0].series_uid.as_str()),
            ("S1", "A")
        );
        assert_eq!(e[1].series_uid, "A");
        assert_eq!(e[2].series_uid, "B");
        assert_eq!(e[2].sop_uid, "b1");
        assert_eq!(
            e[2].path(std::path::Path::new("/m")),
            std::path::Path::new("/m/DIR/B/IM1")
        );
    }
}

#[test]
fn natural_order() {
    use std::cmp::Ordering::*;
    assert_eq!(series::natural_cmp("IM2", "IM10"), Less);
    assert_eq!(series::natural_cmp("IM010", "IM9"), Greater);
    assert_eq!(series::natural_cmp("a", "a"), Equal);
}

#[test]
fn tag_tree_marks_identifying_rows_and_counts_subtrees() {
    let mut item = W::new(true, false);
    item.s(0x0008_0100, b"SH", "T-D3000")
        .s(0x0008_0104, b"LO", "Chest");
    let mut other = W::new(true, false);
    other.s(0x0010_0020, b"LO", "OTHER-ID");
    let mut w = W::new(true, false);
    w.s(0x0008_0020, b"DA", "20240101")
        .s(0x0008_0060, b"CS", "CT")
        .s(0x0008_0090, b"PN", "Ref^Doc")
        .seq(0x0008_2218, &[item.clone(), item])
        .s(0x0009_0010, b"LO", "PRIVATE CREATOR")
        .s(0x0010_0010, b"PN", "Doe^Jane")
        .seq(0x0010_1002, &[other])
        .us(0x0028_0010, &[2, 3])
        .raw(0x0028_1052, b"DS", b"-1024")
        .raw(0x0028_1201, b"OW", &[0xAB; 40]);
    let f = file(EXPLICIT, &w.out);
    let d = Dicom::parse(&f).unwrap();
    let t = TagTree::build(&d, &f);
    let row = |tag: u32| t.rows.iter().find(|r| r.tag == tag && !r.meta).unwrap();
    assert!(row(0x0008_0020).identifying);
    assert!(row(0x0008_0090).identifying);
    assert!(row(0x0010_0010).identifying);
    assert!(row(0x0009_0010).identifying);
    assert!(!row(0x0008_0060).identifying);
    let sq = row(0x0008_2218);
    assert_eq!(sq.value, TagValue::Items(2));
    assert_eq!(sq.descendants, 6);
    assert_eq!(sq.keyword, "AnatomicRegionSequence");
    // The other patient IDs sequence hides its items too.
    let i = t.rows.iter().position(|r| r.tag == 0x0010_1002).unwrap();
    assert!(t.rows[i + 1..i + 3].iter().all(|r| r.identifying));
    // Patient name, date, physician, private creator and the other-IDs sequence.
    assert_eq!(t.identifying_count(), 5);
    assert_eq!(
        row(0x0028_0010).value,
        TagValue::Text {
            text: "2\\3".into(),
            more: false
        }
    );
    match &row(0x0028_1201).value {
        TagValue::Binary { hex, len } => {
            assert_eq!(*len, 40);
            assert!(hex.starts_with("AB AB"));
        }
        v => panic!("{v:?}"),
    }
    // The meta group comes first.
    assert!(t.rows[0].meta && t.rows[0].tag == 0x0002_0000);
}

#[test]
fn identifying_list_is_sorted() {
    assert!(crate::tags::IDENTIFYING.windows(2).all(|w| w[0] < w[1]));
    assert!(crate::tags::identifying(0x0040_A123, crate::dict::Vr::PN));
    assert!(crate::tags::identifying(0x0008_0050, crate::dict::Vr::SH));
    assert!(!crate::tags::identifying(0x0020_0013, crate::dict::Vr::IS));
}
