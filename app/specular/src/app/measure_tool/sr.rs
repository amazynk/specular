//! The measurement list as a DICOM SR measurement report (TID 1500,
//! [`specular_dicom::sr_write`]): one Measurement Group per measurement, its numbers in
//! UCUM units and its outline on the image. A geometry template's group has the template as
//! its finding, every readout as a number and the landmarks as its points; Specular's own
//! concepts are in the local scheme. Concept meanings are English (the report says en-US).

use specular_dicom::sr_write::{Code, Group, Num, Region};
use specular_i18n::I18n;
use specular_measure::calib::{Kind as QKind, Quantity, Unit};
use specular_measure::clinical::Value;
use specular_measure::geom::{P, Shape};
use specular_measure::{ChannelStats, Measurement};

/// A length unit's UCUM code and name; areas are its square. `None`: pixels (no physical
/// size), or a unit UCUM has no length for (arc seconds are an angle).
fn ucum(u: Unit) -> Option<(&'static str, &'static str)> {
    Some(match u {
        Unit::Nm => ("nm", "nanometer"),
        Unit::Um => ("um", "micrometer"),
        Unit::Mm => ("mm", "millimeter"),
        Unit::Cm => ("cm", "centimeter"),
        Unit::M => ("m", "meter"),
        Unit::Km => ("km", "kilometer"),
        Unit::Inch => ("[in_i]", "inch"),
        Unit::Ft => ("[ft_i]", "foot"),
        Unit::Yd => ("[yd_i]", "yard"),
        Unit::Mi => ("[mi_i]", "mile"),
        Unit::Arcsec => ("''", "second of arc"),
        Unit::Px => return None,
    })
}

/// The UCUM unit of a quantity (pixels: `{pixels}`, as a UCUM annotation).
fn unit_of(q: &Quantity) -> Code {
    match q.kind {
        QKind::Angle => Code::ucum("deg", "degree"),
        QKind::Area => match ucum(q.unit) {
            Some((c, n)) => Code::ucum(&format!("{c}2"), &format!("square {n}")),
            None => Code::ucum("{pixels}2", "square pixels"),
        },
        _ => match ucum(q.unit) {
            Some((c, n)) => Code::ucum(c, n),
            None => Code::ucum("{pixels}", "pixels"),
        },
    }
}

fn tr(id: &str) -> String {
    I18n::global().tr(id)
}

fn xy(p: &P) -> (f32, f32) {
    (p.x as f32, p.y as f32)
}

/// The outline as SCOORD: closed outlines repeat their first point (PS3.3 C.18.6.1.2).
fn region(s: &Shape) -> Region {
    let closed = |pts: &[P]| {
        let mut v: Vec<(f32, f32)> = pts.iter().map(xy).collect();
        if let Some(&f) = v.first() {
            v.push(f);
        }
        v
    };
    match s {
        Shape::Point(p) => Region {
            graphic_type: "POINT",
            points: vec![xy(p)],
        },
        Shape::Line(a, b) => Region {
            graphic_type: "POLYLINE",
            points: vec![xy(a), xy(b)],
        },
        Shape::Polyline(v) => Region {
            graphic_type: "POLYLINE",
            points: v.iter().map(xy).collect(),
        },
        Shape::Angle { a, vertex, b } => Region {
            graphic_type: "POLYLINE",
            points: vec![xy(a), xy(vertex), xy(b)],
        },
        Shape::Rect { x0, y0, x1, y1 } => Region {
            graphic_type: "POLYGON",
            points: closed(&[
                P::new(*x0, *y0),
                P::new(*x1, *y0),
                P::new(*x1, *y1),
                P::new(*x0, *y1),
            ]),
        },
        // The major axis's ends, then the minor axis's.
        Shape::Ellipse { cx, cy, rx, ry } => {
            let (cx, cy, rx, ry) = (*cx as f32, *cy as f32, *rx as f32, *ry as f32);
            let h = [(cx - rx, cy), (cx + rx, cy)];
            let v = [(cx, cy - ry), (cx, cy + ry)];
            let (major, minor) = if rx >= ry { (h, v) } else { (v, h) };
            Region {
                graphic_type: "ELLIPSE",
                points: vec![major[0], major[1], minor[0], minor[1]],
            }
        }
        Shape::Polygon(v) | Shape::Freehand(v) => Region {
            graphic_type: "POLYGON",
            points: closed(v),
        },
    }
}

fn num(concept: Code, q: &Quantity) -> Num {
    Num {
        concept,
        value: q.value,
        unit: unit_of(q),
    }
}

/// The pixel statistics of the first channel: mean, SD, minimum, maximum (Hounsfield units
/// for CT, else no unit).
fn stats_nums(m: &Measurement, s: &ChannelStats) -> Vec<Num> {
    let unit = if m.channels.first().is_some_and(|c| c == "HU") {
        Code::ucum("[hnsf'U]", "Hounsfield unit")
    } else {
        Code::ucum("1", "no units")
    };
    [
        (Code::new("373098007", "SCT", "Mean"), s.mean),
        (Code::new("386136009", "SCT", "Standard Deviation"), s.sd),
        (Code::new("255605001", "SCT", "Minimum"), s.min),
        (Code::new("56851009", "SCT", "Maximum"), s.max),
    ]
    .into_iter()
    .filter(|(_, v)| v.is_finite())
    .map(|(concept, value)| Num {
        concept,
        value,
        unit: unit.clone(),
    })
    .collect()
}

fn group(m: &Measurement) -> Group {
    if let Some(t) = m.template {
        let name = tr(&format!("geometry-{}", t.key()));
        let nums = m
            .outcome()
            .map(|o| {
                o.readouts
                    .iter()
                    .map(|r| {
                        let concept = Code::local(r.key, &tr(&format!("geometry-out-{}", r.key)));
                        match r.value {
                            Value::Quantity(q) => num(concept, &q),
                            Value::Ratio(v) => Num {
                                concept,
                                value: v,
                                unit: Code::ucum("1", "ratio"),
                            },
                            Value::Vertebrae(v) => Num {
                                concept,
                                value: v,
                                unit: Code::ucum("{vertebrae}", "vertebrae"),
                            },
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        return Group {
            tracking: format!("{} {}", name, m.id),
            finding: Some(Code::local(t.key(), &name)),
            nums,
            region: Some(Region {
                graphic_type: "MULTIPOINT",
                points: m.shape.points().iter().map(xy).collect(),
            }),
        };
    }
    let mut nums = Vec::new();
    if let Some(q) = m.length() {
        nums.push(num(Code::new("410668003", "SCT", "Length"), &q));
    }
    if let Some(q) = m.angle() {
        nums.push(num(Code::local("angle", "Angle"), &q));
    }
    if let Some(q) = m.area() {
        nums.push(num(Code::new("42798000", "SCT", "Area"), &q));
    }
    if let Some(q) = m.perimeter() {
        nums.push(num(Code::new("131191004", "SCT", "Perimeter"), &q));
    }
    if let Some(s) = m.stats.as_ref().and_then(|s| s.first()) {
        nums.extend(stats_nums(m, s));
    }
    let shape = match &m.shape {
        Shape::Point(_) => "Point",
        Shape::Line(..) => "Line",
        Shape::Polyline(_) => "Path",
        Shape::Angle { .. } => "Angle",
        Shape::Rect { .. } => "Rectangle",
        Shape::Ellipse { .. } => "Ellipse",
        Shape::Polygon(_) => "Polygon",
        Shape::Freehand(_) => "Freehand",
    };
    let tracking = if m.label.trim().is_empty() {
        format!("{shape} {}", m.id)
    } else {
        m.label.trim().to_owned()
    };
    Group {
        tracking,
        finding: None,
        nums,
        region: Some(region(&m.shape)),
    }
}

/// Every measurement's group, in list order, with English meanings.
pub(super) fn groups(items: &[Measurement]) -> Vec<Group> {
    I18n::with_thread_language(specular_i18n::langid!("en-US"), || {
        items.iter().map(group).collect()
    })
}

/// The frame the measurements are on (1-based), when they say one.
pub(super) fn frame(items: &[Measurement]) -> Option<u32> {
    items.iter().find_map(|m| m.frame).map(|f| f + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_measure::calib::{Calibration, Source};
    use specular_measure::clinical::Template;

    fn mm() -> Calibration {
        let mut c = Calibration::uncalibrated();
        c.unit = Unit::Mm;
        c.source = Source::DicomPixelSpacing;
        c
    }

    #[test]
    fn measurements_become_groups() {
        let cal = mm();
        let line = Measurement::new(Shape::Line(P::new(0.0, 0.0), P::new(3.0, 4.0)), cal);
        let ell = Measurement::new(
            Shape::Ellipse {
                cx: 10.0,
                cy: 10.0,
                rx: 2.0,
                ry: 5.0,
            },
            cal,
        );
        let t = Template::Tpa;
        let pts: Vec<P> = (0..t.landmarks().len())
            .map(|i| P::new(10.0 + 9.0 * i as f64, 20.0 + 4.0 * (i * i) as f64))
            .collect();
        let geo = Measurement::clinical(t, pts.clone(), cal);
        let g = groups(&[line, ell, geo.clone()]);
        assert_eq!(g.len(), 3);
        assert_eq!(g[0].nums[0].concept.value, "410668003");
        assert_eq!(g[0].nums[0].unit.value, "mm");
        assert_eq!(g[0].region.as_ref().unwrap().graphic_type, "POLYLINE");
        let e = g[1].region.as_ref().unwrap();
        assert_eq!(e.graphic_type, "ELLIPSE");
        // The major axis (vertical here) first.
        assert_eq!(e.points[0], (10.0, 5.0));
        assert!(g[1].nums.iter().any(|n| n.unit.value == "mm2"));
        let r = &g[2];
        assert_eq!(r.finding.as_ref().unwrap().scheme, "99SPECULAR");
        assert_eq!(r.region.as_ref().unwrap().points.len(), pts.len());
        let o = geo.outcome().unwrap();
        assert_eq!(r.nums.len(), o.readouts.len());
        for (n, ro) in r.nums.iter().zip(&o.readouts) {
            assert_eq!(n.concept.value, ro.key);
            assert_eq!(n.value, ro.value.number());
            assert!(
                !n.concept.meaning.starts_with("geometry-"),
                "{}",
                n.concept.meaning
            );
        }
        // Pixels have no physical unit: a UCUM annotation.
        let px = Measurement::new(
            Shape::Line(P::new(0.0, 0.0), P::new(3.0, 4.0)),
            Calibration::uncalibrated(),
        );
        assert_eq!(groups(&[px])[0].nums[0].unit.value, "{pixels}");
    }

    /// A synthetic CT instance to measure on.
    fn instance() -> Vec<u8> {
        use specular_dicom::dict::Vr;
        use specular_dicom::write::{self, FileMeta, Object};
        let mut o = Object::default();
        o.put_text(0x0008_0016, Vr::UI, "1.2.840.10008.5.1.4.1.1.2");
        o.put_text(0x0008_0018, Vr::UI, "2.25.4242");
        o.put_text(0x0010_0010, Vr::PN, "ANON^DOG");
        o.put_text(0x0010_0020, Vr::LO, "A1");
        o.put_text(0x0020_000D, Vr::UI, "2.25.1");
        o.put_text(0x0020_000E, Vr::UI, "2.25.2");
        write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE)).unwrap()
    }

    /// The listing the pydicom script prints, from the groups meant to be written.
    fn expected(want: &[Group]) -> String {
        let pts = |r: &Region| {
            r.points
                .iter()
                .flat_map(|(x, y)| [*x, *y])
                .map(|v| format!("{:?}", f64::from(v)))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut expect = String::new();
        for g in want {
            let r = g.region.as_ref().unwrap();
            let area = matches!(r.graphic_type, "POLYGON" | "ELLIPSE");
            expect += &format!("G {}\n", if area { "1410" } else { "1501" });
            expect += &format!("T {}\n", g.tracking);
            if area {
                expect += &format!("S {} {}\n", r.graphic_type, pts(r));
            }
            for n in &g.nums {
                let v: f64 = specular_dicom::sr_write::ds(n.value).parse().unwrap();
                expect += &format!("N {} {v:?} {}\n", n.concept.value, n.unit.value);
                if !area {
                    expect += &format!("S {} {}\n", r.graphic_type, pts(r));
                }
            }
        }
        expect
    }

    /// pydicom lists every group's template, tracking text, numbers (concept, value, unit)
    /// and the points of each outline.
    const PYDICOM: &str = "import sys, pydicom
d = pydicom.dcmread(sys.argv[1])
assert d.SOPClassUID == '1.2.840.10008.5.1.4.1.1.88.33'
assert d.ContentTemplateSequence[0].TemplateIdentifier == '1500'
assert d.ConceptNameCodeSequence[0].CodeValue == '126000'
assert d.PatientName == 'ANON^DOG' and d.StudyInstanceUID == '2.25.1'
ev = d.CurrentRequestedProcedureEvidenceSequence[0].ReferencedSeriesSequence[0]
assert ev.ReferencedSOPSequence[0].ReferencedSOPInstanceUID == '2.25.4242'
code = lambda c: c.ConceptNameCodeSequence[0].get('CodeValue') or c.ConceptNameCodeSequence[0].LongCodeValue
lib = [c for c in d.ContentSequence if code(c) == '111028'][0]
img = lib.ContentSequence[0].ContentSequence[0]
assert img.ReferencedSOPSequence[0].ReferencedSOPInstanceUID == '2.25.4242'
meas = [c for c in d.ContentSequence if code(c) == '126010'][0]
pts = lambda s: ' '.join(repr(float(v)) for v in s.GraphicData)
for g in meas.ContentSequence:
    assert code(g) == '125007'
    print('G', g.ContentTemplateSequence[0].TemplateIdentifier)
    for c in g.ContentSequence:
        if c.ValueType == 'NUM':
            mv = c.MeasuredValueSequence[0]
            u = mv.MeasurementUnitsCodeSequence[0].CodeValue
            print('N', code(c), repr(float(mv.NumericValue)), u)
            for s in c.get('ContentSequence', []):
                assert s.RelationshipType == 'INFERRED FROM'
                r = s.ContentSequence[0].ReferencedSOPSequence[0]
                assert r.ReferencedSOPInstanceUID == '2.25.4242'
                print('S', s.GraphicType, pts(s))
        elif c.ValueType == 'SCOORD':
            print('S', c.GraphicType, pts(c))
        elif c.ValueType == 'TEXT':
            print('T', c.TextValue)
";

    /// The saved report reads back with Specular's SR reader and, when `SPECULAR_PYDICOM`
    /// names a Python with pydicom, with pydicom: TID 1500, the image referenced, every
    /// group's numbers in order with their units, and the outlines' points.
    #[test]
    fn report_round_trips_and_pydicom_reads_it() {
        let cal = mm();
        let t = Template::Vhs;
        let pts: Vec<P> = (0..t.landmarks().len())
            .map(|i| P::new(40.0 + 13.0 * i as f64, 30.0 + 7.0 * ((i * 5) % 11) as f64))
            .collect();
        let items = vec![
            Measurement::new(Shape::Line(P::new(1.5, 2.5), P::new(31.5, 42.5)), cal),
            Measurement::new(
                Shape::Angle {
                    a: P::new(0.0, 10.0),
                    vertex: P::new(0.0, 0.0),
                    b: P::new(10.0, 0.0),
                },
                cal,
            ),
            Measurement::new(
                Shape::Rect {
                    x0: 2.0,
                    y0: 3.0,
                    x1: 12.0,
                    y1: 8.0,
                },
                cal,
            ),
            Measurement::clinical(t, pts, cal),
        ];
        let src = instance();
        let b = super::super::sr_bytes(&items, &src, 1_761_390_000).unwrap();
        let d = specular_dicom::Dicom::parse(&b).unwrap();
        let rep = specular_dicom::sr::Report::read(&d, &b).unwrap();
        assert!(rep.is_measurement_report());
        let want = groups(&items);
        let total: usize = want.iter().map(|g| g.nums.len()).sum();
        assert_eq!(rep.measurements(), total);
        assert!(super::super::sr_bytes(&items, b"not dicom", 0).is_err());
        let Ok(py) = std::env::var("SPECULAR_PYDICOM") else {
            eprintln!("skipped the pydicom check: SPECULAR_PYDICOM is not set");
            return;
        };
        let dir = std::env::temp_dir().join(format!("specular-sr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("sr.dcm");
        std::fs::write(&file, &b).unwrap();
        let o = std::process::Command::new(py)
            .args(["-I", "-c", PYDICOM])
            .arg(&file)
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let out = String::from_utf8_lossy(&o.stdout);
        let expect = expected(&want);
        let mut mismatches = 0;
        for (a, b) in out.lines().zip(expect.lines()) {
            if a != b {
                mismatches += 1;
                eprintln!("pydicom: {a}\nwanted:  {b}");
            }
        }
        let lines = expect.lines().count();
        eprintln!("SR against pydicom: {lines} lines, {mismatches} mismatches");
        assert_eq!(out.lines().count(), lines, "{out}");
        assert_eq!(mismatches, 0);
    }
}
