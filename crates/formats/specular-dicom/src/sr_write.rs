//! Measurement reports written as Comprehensive SR (PS3.3 A.35.3) after template TID 1500
//! (Measurement Report, PS3.16): the image measured in the Image Library, then one
//! Measurement Group per measurement under Imaging Measurements, each with its tracking
//! identifier and UID, an optional finding, its numbers (NUM, UCUM units) and the outline
//! they come from (SCOORD on the image, in pixel columns and rows with 0,0 the top left
//! corner of the top left pixel).
//!
//! Concepts without a standard code (a geometry template's readouts, say) are written in the
//! local scheme [`LOCAL_SCHEME`], named in the Coding Scheme Identification Sequence.
//!
//! The document joins the measured image's study (its patient and study are copied, as
//! [`crate::kos`] does) as a series of its own (modality SR).

use crate::dict::Vr;
use crate::kos::{Source, new_uid};
use crate::write::{self, FileMeta, Object, Value};
use crate::{Error, Result};

pub const COMPREHENSIVE_SR: &str = "1.2.840.10008.5.1.4.1.1.88.33";
/// The coding scheme of the concepts Specular names itself.
pub const LOCAL_SCHEME: &str = "99SPECULAR";

/// A coded concept: code value, coding scheme designator, code meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    pub value: String,
    pub scheme: String,
    pub meaning: String,
}

impl Code {
    pub fn new(value: &str, scheme: &str, meaning: &str) -> Code {
        Code {
            value: value.to_owned(),
            scheme: scheme.to_owned(),
            meaning: meaning.to_owned(),
        }
    }

    /// A concept in [`LOCAL_SCHEME`].
    pub fn local(value: &str, meaning: &str) -> Code {
        Code::new(value, LOCAL_SCHEME, meaning)
    }

    /// A UCUM unit (PS3.16 CID 82): `code` as UCUM writes it, `meaning` its name.
    pub fn ucum(code: &str, meaning: &str) -> Code {
        Code::new(code, "UCUM", meaning)
    }
}

/// One number of a group.
#[derive(Clone, Debug, PartialEq)]
pub struct Num {
    pub concept: Code,
    pub value: f64,
    pub unit: Code,
}

/// The outline a group's numbers come from (SCOORD Graphic Type and its points).
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    /// `POINT`, `MULTIPOINT`, `POLYLINE`, `POLYGON` or `ELLIPSE`.
    pub graphic_type: &'static str,
    /// Column, row pairs.
    pub points: Vec<(f32, f32)>,
}

/// One Measurement Group.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// Tracking Identifier (shown to people: "Line 1", "VHS").
    pub tracking: String,
    pub finding: Option<Code>,
    pub nums: Vec<Num>,
    pub region: Option<Region>,
}

fn code_item(c: &Code) -> Object<'static> {
    let mut o = Object::default();
    // Code values over 16 characters go in Long Code Value (PS3.3 8.8).
    if c.value.len() > 16 {
        o.put_text(0x0008_0119, Vr::UC, &c.value);
    } else {
        o.put_text(0x0008_0100, Vr::SH, &c.value);
    }
    o.put_text(0x0008_0102, Vr::SH, &c.scheme);
    o.put_text(0x0008_0104, Vr::LO, &c.meaning);
    o
}

fn seq(v: Vec<Object<'static>>) -> Value<'static> {
    Value::Items(v)
}

/// A content item: relationship, value type and concept name.
fn item(rel: &str, vt: &str, concept: &Code) -> Object<'static> {
    let mut o = Object::default();
    o.put_text(0x0040_A010, Vr::CS, rel);
    o.put_text(0x0040_A040, Vr::CS, vt);
    o.put(0x0040_A043, Vr::SQ, seq(vec![code_item(concept)]));
    o
}

fn container(
    rel: &str,
    concept: &Code,
    template: Option<&str>,
    children: Vec<Object<'static>>,
) -> Object<'static> {
    let mut o = item(rel, "CONTAINER", concept);
    o.put_text(0x0040_A050, Vr::CS, "SEPARATE");
    if let Some(t) = template {
        o.put(0x0040_A504, Vr::SQ, seq(vec![template_item(t)]));
    }
    o.put(0x0040_A730, Vr::SQ, seq(children));
    o
}

fn template_item(id: &str) -> Object<'static> {
    let mut t = Object::default();
    t.put_text(0x0008_0105, Vr::CS, "DCMR");
    t.put_text(0x0040_DB00, Vr::CS, id);
    t
}

fn code_value(rel: &str, concept: &Code, value: &Code) -> Object<'static> {
    let mut o = item(rel, "CODE", concept);
    o.put(0x0040_A168, Vr::SQ, seq(vec![code_item(value)]));
    o
}

fn text_value(rel: &str, concept: &Code, text: &str) -> Object<'static> {
    let mut o = item(rel, "TEXT", concept);
    o.put_text(0x0040_A160, Vr::UT, text);
    o
}

fn uid_value(rel: &str, concept: &Code, uid: &str) -> Object<'static> {
    let mut o = item(rel, "UIDREF", concept);
    o.put_text(0x0040_A124, Vr::UI, uid);
    o
}

/// A decimal string of at most 16 characters (DS).
pub fn ds(v: f64) -> String {
    if !v.is_finite() || v == 0.0 {
        return "0".to_owned();
    }
    // Very small or large: exponent form, as many digits as fit.
    if !(1e-4..1e15).contains(&v.abs()) {
        for p in (0..=10).rev() {
            let s = format!("{v:.p$e}");
            if s.len() <= 16 {
                return s;
            }
        }
    }
    for p in (0..=15).rev() {
        let s = format!("{v:.p$}");
        let s = if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s
        };
        if s.len() <= 16 {
            return if s == "-0" { "0".to_owned() } else { s };
        }
    }
    format!("{v:.6e}")
}

fn image_ref(src: &Source, frame: Option<u32>) -> Object<'static> {
    let mut r = Object::default();
    r.put_text(0x0008_1150, Vr::UI, &src.reference.sop_class);
    r.put_text(0x0008_1155, Vr::UI, &src.reference.sop_instance);
    if let Some(f) = frame {
        r.put_text(0x0008_1160, Vr::IS, &f.to_string());
    }
    r
}

fn image_item(rel: &str, src: &Source, frame: Option<u32>) -> Object<'static> {
    let mut o = Object::default();
    o.put_text(0x0040_A010, Vr::CS, rel);
    o.put_text(0x0040_A040, Vr::CS, "IMAGE");
    o.put(0x0008_1199, Vr::SQ, seq(vec![image_ref(src, frame)]));
    o
}

fn scoord(
    rel: &str,
    concept: &Code,
    r: &Region,
    src: &Source,
    frame: Option<u32>,
) -> Object<'static> {
    let mut o = item(rel, "SCOORD", concept);
    o.put_text(0x0070_0023, Vr::CS, r.graphic_type);
    let mut b = Vec::with_capacity(r.points.len() * 8);
    for (x, y) in &r.points {
        b.extend_from_slice(&x.to_le_bytes());
        b.extend_from_slice(&y.to_le_bytes());
    }
    o.put_bytes(0x0070_0022, Vr::FL, b);
    o.put(
        0x0040_A730,
        Vr::SQ,
        seq(vec![image_item("SELECTED FROM", src, frame)]),
    );
    o
}

/// The report, as Part 10 bytes (Explicit VR Little Endian, UTF-8): `src` is the image
/// measured (`frame` 1-based, for multi-frame images), `groups` the measurements (at least
/// one), `when` the content date and time (`YYYYMMDD`, `HHMMSS`), `device` the observer's
/// name ("Specular 1.0").
pub fn measurement_report(
    src: &Source,
    frame: Option<u32>,
    groups: &[Group],
    when: (&str, &str),
    device: &str,
) -> Result<Vec<u8>> {
    if groups.is_empty() {
        return Err(Error::Malformed(
            "a measurement report without measurements",
        ));
    }
    let r = &src.reference;
    let mut o = Object::default();
    o.put_text(0x0008_0005, Vr::CS, "ISO_IR 192");
    o.put_text(0x0008_0016, Vr::UI, COMPREHENSIVE_SR);
    o.put_text(0x0008_0018, Vr::UI, &new_uid());
    // Coding Scheme Identification: the local scheme.
    let mut scheme = Object::default();
    scheme.put_text(0x0008_0102, Vr::SH, LOCAL_SCHEME);
    scheme.put_text(0x0008_0115, Vr::ST, "Specular local codes");
    scheme.put_text(0x0008_0116, Vr::ST, "Specular");
    o.put(0x0008_0110, Vr::SQ, seq(vec![scheme]));
    // Patient and General Study (copied from the image).
    o.put_text(0x0010_0010, Vr::PN, &src.patient_name);
    o.put_text(0x0010_0020, Vr::LO, &src.patient_id);
    o.put_text(0x0010_0030, Vr::DA, "");
    o.put_text(0x0010_0040, Vr::CS, "");
    o.put_text(0x0020_000D, Vr::UI, &r.study);
    o.put_text(0x0008_0020, Vr::DA, &src.study_date);
    o.put_text(0x0008_0030, Vr::TM, &src.study_time);
    o.put_text(0x0008_0090, Vr::PN, "");
    o.put_text(0x0020_0010, Vr::SH, &src.study_id);
    o.put_text(0x0008_0050, Vr::SH, &src.accession);
    // SR Document Series.
    o.put_text(0x0008_0060, Vr::CS, "SR");
    o.put_text(0x0020_000E, Vr::UI, &new_uid());
    o.put_text(0x0020_0011, Vr::IS, "998");
    o.put(0x0008_1111, Vr::SQ, seq(Vec::new()));
    // General Equipment.
    o.put_text(0x0008_0070, Vr::LO, "");
    // SR Document General.
    o.put_text(0x0020_0013, Vr::IS, "1");
    o.put_text(0x0008_0023, Vr::DA, when.0);
    o.put_text(0x0008_0033, Vr::TM, when.1);
    o.put_text(0x0040_A491, Vr::CS, "PARTIAL");
    o.put_text(0x0040_A493, Vr::CS, "UNVERIFIED");
    let mut sop = Object::default();
    sop.put_text(0x0008_1150, Vr::UI, &r.sop_class);
    sop.put_text(0x0008_1155, Vr::UI, &r.sop_instance);
    let mut series = Object::default();
    series.put_text(0x0020_000E, Vr::UI, &r.series);
    series.put(0x0008_1199, Vr::SQ, seq(vec![sop]));
    let mut evidence = Object::default();
    evidence.put_text(0x0020_000D, Vr::UI, &r.study);
    evidence.put(0x0008_1115, Vr::SQ, seq(vec![series]));
    o.put(0x0040_A375, Vr::SQ, seq(vec![evidence]));
    o.put(0x0040_A372, Vr::SQ, seq(Vec::new()));

    // SR Document Content: TID 1500.
    let mut content = vec![
        code_value(
            "HAS CONCEPT MOD",
            &Code::new("121049", "DCM", "Language of Content Item and Descendants"),
            &Code::new("en-US", "RFC5646", "English (United States)"),
        ),
        // TID 1001 observer context: a device.
        code_value(
            "HAS OBS CONTEXT",
            &Code::new("121005", "DCM", "Observer Type"),
            &Code::new("121007", "DCM", "Device"),
        ),
        uid_value(
            "HAS OBS CONTEXT",
            &Code::new("121012", "DCM", "Device Observer UID"),
            &new_uid(),
        ),
        text_value(
            "HAS OBS CONTEXT",
            &Code::new("121013", "DCM", "Device Observer Name"),
            device,
        ),
        code_value(
            "HAS CONCEPT MOD",
            &Code::new("121058", "DCM", "Procedure reported"),
            &Code::new("363679005", "SCT", "Imaging procedure"),
        ),
    ];
    let library = container(
        "CONTAINS",
        &Code::new("111028", "DCM", "Image Library"),
        None,
        vec![container(
            "CONTAINS",
            &Code::new("126200", "DCM", "Image Library Group"),
            None,
            vec![image_item("CONTAINS", src, frame)],
        )],
    );
    content.push(library);
    let region_concept = Code::new("111030", "DCM", "Image Region");
    let mut measured = Vec::with_capacity(groups.len());
    for g in groups {
        let mut kids = vec![
            text_value(
                "HAS OBS CONTEXT",
                &Code::new("112039", "DCM", "Tracking Identifier"),
                &g.tracking,
            ),
            uid_value(
                "HAS OBS CONTEXT",
                &Code::new("112040", "DCM", "Tracking Unique Identifier"),
                &new_uid(),
            ),
        ];
        if let Some(f) = &g.finding {
            kids.push(code_value(
                "CONTAINS",
                &Code::new("121071", "DCM", "Finding"),
                f,
            ));
        }
        // TID 1410 (planar ROI) when the outline is an area; else TID 1501 with the
        // outline under each number (INFERRED FROM).
        let area = g
            .region
            .as_ref()
            .is_some_and(|r| matches!(r.graphic_type, "POLYGON" | "ELLIPSE"));
        if let (true, Some(r)) = (area, &g.region) {
            kids.push(scoord("CONTAINS", &region_concept, r, src, frame));
        }
        for n in &g.nums {
            let mut v = Object::default();
            v.put(0x0040_08EA, Vr::SQ, seq(vec![code_item(&n.unit)]));
            v.put_text(0x0040_A30A, Vr::DS, &ds(n.value));
            let mut num = item("CONTAINS", "NUM", &n.concept);
            num.put(0x0040_A300, Vr::SQ, seq(vec![v]));
            if !area && let Some(r) = &g.region {
                num.put(
                    0x0040_A730,
                    Vr::SQ,
                    seq(vec![scoord(
                        "INFERRED FROM",
                        &region_concept,
                        r,
                        src,
                        frame,
                    )]),
                );
            }
            kids.push(num);
        }
        measured.push(container(
            "CONTAINS",
            &Code::new("125007", "DCM", "Measurement Group"),
            Some(if area { "1410" } else { "1501" }),
            kids,
        ));
    }
    content.push(container(
        "CONTAINS",
        &Code::new("126010", "DCM", "Imaging Measurements"),
        None,
        measured,
    ));
    o.put_text(0x0040_A040, Vr::CS, "CONTAINER");
    o.put(
        0x0040_A043,
        Vr::SQ,
        seq(vec![code_item(&Code::new(
            "126000",
            "DCM",
            "Imaging Measurement Report",
        ))]),
    );
    o.put_text(0x0040_A050, Vr::CS, "SEPARATE");
    o.put(0x0040_A504, Vr::SQ, seq(vec![template_item("1500")]));
    o.put(0x0040_A730, Vr::SQ, seq(content));
    write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dicom;
    use crate::kos::Reference;

    fn source() -> Source {
        Source {
            reference: Reference {
                sop_class: "1.2.840.10008.5.1.4.1.1.1.1".into(),
                sop_instance: "2.25.100".into(),
                series: "2.25.10".into(),
                study: "2.25.1".into(),
            },
            patient_name: "ANON^DOG".into(),
            patient_id: "A1".into(),
            study_date: "20261025".into(),
            study_time: "101500".into(),
            accession: "ACC1".into(),
            study_id: "1".into(),
        }
    }

    #[test]
    fn decimal_strings_fit() {
        assert_eq!(ds(10.5), "10.5");
        assert_eq!(ds(-0.0), "0");
        assert_eq!(ds(1.0 / 3.0), "0.33333333333333");
        assert!(ds(123_456_789_012.345_67).len() <= 16);
        assert!(ds(1e-30).len() <= 16);
        assert!(ds(-1.234_567_890_123e25).len() <= 16);
        for v in [1e-30, 123_456_789_012.345_67, -1.234_567_890_123e25] {
            let back: f64 = ds(v).parse().unwrap();
            assert!(((back - v) / v).abs() < 1e-6, "{v} {}", ds(v));
        }
    }

    /// Written and read back with the SR reader: a TID 1500 report with its groups, numbers
    /// and outlines on the measured image.
    #[test]
    fn report_reads_back_as_tid1500() {
        let groups = vec![
            Group {
                tracking: "Vertebral heart score (VHS)".into(),
                finding: Some(Code::local("vhs", "Vertebral heart score (VHS)")),
                nums: vec![
                    Num {
                        concept: Code::local("vhs", "VHS"),
                        value: 10.25,
                        unit: Code::ucum("{vertebrae}", "vertebrae"),
                    },
                    Num {
                        concept: Code::local("long-axis", "Long axis"),
                        value: 61.5,
                        unit: Code::ucum("mm", "millimeter"),
                    },
                ],
                region: Some(Region {
                    graphic_type: "MULTIPOINT",
                    points: vec![(10.0, 20.0), (30.5, 40.25), (50.0, 60.0)],
                }),
            },
            Group {
                tracking: "Area 2".into(),
                finding: None,
                nums: vec![Num {
                    concept: Code::new("42798000", "SCT", "Area"),
                    value: 12.0,
                    unit: Code::ucum("mm2", "square millimeter"),
                }],
                region: Some(Region {
                    graphic_type: "POLYGON",
                    points: vec![(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 0.0)],
                }),
            },
        ];
        let b = measurement_report(
            &source(),
            Some(2),
            &groups,
            ("20261025", "120000"),
            "Specular",
        )
        .unwrap();
        let d = Dicom::parse(&b).unwrap();
        let rep = crate::sr::Report::read(&d, &b).unwrap();
        assert!(rep.is_measurement_report());
        assert_eq!(rep.measurements(), 3);
        let lines: Vec<String> = rep.lines(true).into_iter().map(|(_, l)| l).collect();
        let all = lines.join("\n");
        assert!(all.contains("10.25"), "{all}");
        assert!(all.contains("Vertebral heart score (VHS)"), "{all}");
        assert!(all.contains("61.5"), "{all}");
        let refs = rep.references();
        assert!(
            !refs.is_empty() && refs.iter().all(|r| r.sop_instance == "2.25.100"),
            "{refs:?}"
        );
        assert!(measurement_report(&source(), None, &[], ("", ""), "").is_err());
    }
}
