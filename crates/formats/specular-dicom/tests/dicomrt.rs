//! Structured reports, waveforms, RT Dose and OCT on the fixtures of gen/dicomrt_make.py,
//! against tests/data/{sr,waveform,dvh}_oracle.tsv from gen/dicomrt_oracle.py (pydicom for the
//! SR content trees and the scaled waveform samples, dicompyler-core for the DVHs).
//!
//! DVH tolerances (dicompyler-core's own method, 1 cGy bins): volume within 1e-6 cm³; max, min,
//! D95 and D100 exact to the bin (0.01 Gy, compared at 1e-6); mean within 1e-5 Gy; V20Gy and
//! every 50th cumulative bin within 1e-5 cm³.

use std::path::{Path, PathBuf};

use specular_dicom::rtdose::{self, DoseGrid, Prescription};
use specular_dicom::rtstruct::StructureSet;
use specular_dicom::sr::{self, Code, Jump, Node, Report, Value};
use specular_dicom::waveform::{self, Paper, Waveforms};
use specular_dicom::{Dicom, oct::Oct};

fn path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(rel)
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(path(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn rows(table: &'static str) -> Vec<Vec<&'static str>> {
    table
        .lines()
        .skip(1)
        .map(|l| l.split('\t').collect())
        .collect()
}

fn code(c: Option<&Code>) -> String {
    c.map(|c| format!("{}|{}|{}", c.value, c.scheme, c.meaning))
        .unwrap_or_default()
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\r', '\n'], " ").trim().to_owned()
}

/// The canonical value text of gen/dicomrt_oracle.py's `value`.
fn canonical(n: &Node) -> String {
    let refs = |r: &sr::SopRef| {
        format!(
            "{}|{}|{}",
            r.sop_class,
            r.sop_instance,
            r.frames
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let coords = |c: &mut dyn Iterator<Item = f64>| {
        c.map(|v| format!("{v:.4}")).collect::<Vec<_>>().join(",")
    };
    match &n.value {
        Value::Container {
            continuous,
            template,
        } => format!(
            "{}|{}",
            if *continuous {
                "CONTINUOUS"
            } else {
                "SEPARATE"
            },
            template.as_deref().unwrap_or("")
        ),
        Value::Text(s)
        | Value::DateTime(s)
        | Value::Date(s)
        | Value::Time(s)
        | Value::PersonName(s)
        | Value::UidRef(s) => clean(s),
        Value::Code(c) => code(Some(c)),
        Value::Num(m) => format!(
            "{}|{}|{}",
            m.text,
            code(m.unit.as_ref()),
            code(m.qualifier.as_ref())
        ),
        Value::Image(r) | Value::Composite(r) | Value::Waveform(r) => refs(r),
        Value::Scoord {
            graphic_type,
            points,
        } => format!(
            "{graphic_type}|{}",
            coords(&mut points.iter().flat_map(|p| [p[0] as f64, p[1] as f64]))
        ),
        Value::Scoord3d {
            graphic_type,
            points,
            frame_of_reference,
        } => format!(
            "{graphic_type}|{frame_of_reference}|{}",
            coords(&mut points.iter().flat_map(|p| *p))
        ),
        Value::Tcoord { range_type, values } => format!("{range_type}|{}", values.join(",")),
        Value::Reference(p) => format!(
            "ref|{}",
            p.iter().map(u32::to_string).collect::<Vec<_>>().join(".")
        ),
        Value::Table(_) | Value::Other => String::new(),
    }
}

/// Compares the content trees of `files` with oracle rows (file, path, relationship, value
/// type, concept, value); returns (items compared, mismatches).
fn compare_sr(oracle: &[Vec<&str>], open: impl Fn(&str) -> Vec<u8>) -> (usize, usize) {
    let mut files: Vec<&str> = oracle.iter().map(|r| r[0]).collect();
    files.dedup();
    let (mut items, mut bad) = (0, 0);
    for f in files {
        let want: Vec<&Vec<&str>> = oracle.iter().filter(|r| r[0] == f).collect();
        let data = open(f);
        let d = Dicom::parse(&data).unwrap();
        let report = Report::read(&d, &data).unwrap_or_else(|e| panic!("{f}: {e}"));
        let got = report.walk();
        if got.len() != want.len() {
            eprintln!("{f}: {} items, oracle {}", got.len(), want.len());
            bad += got.len().abs_diff(want.len());
        }
        for ((p, n), w) in got.iter().zip(&want) {
            items += 1;
            let path = p.iter().map(u32::to_string).collect::<Vec<_>>().join(".");
            let row = [
                path,
                n.relationship.clone(),
                n.value_type.clone(),
                code(n.concept.as_ref()),
                canonical(n),
            ];
            if row.iter().map(String::as_str).ne(w[1..6].iter().copied()) {
                bad += 1;
                eprintln!("{f}: {row:?} != {:?}", &w[1..6]);
            }
        }
    }
    (items, bad)
}

#[test]
fn sr_content_trees_match_pydicom() {
    let oracle = rows(include_str!("data/sr_oracle.tsv"));
    let (items, bad) = compare_sr(&oracle, |f| read(&format!("dicomrt/{f}")));
    eprintln!("SR: {items} content items, {bad} mismatches against pydicom");
    assert_eq!(items, oracle.len());
    assert_eq!(bad, 0);
}

#[test]
fn sr_kinds_titles_and_jumps() {
    let open = |f: &str| {
        let data = read(&format!("dicomrt/study/{f}"));
        let d = Dicom::parse(&data).unwrap();
        Report::read(&d, &data).unwrap()
    };
    let ct4 = {
        let data = read("dicomrt/study/ct_04.dcm");
        let d = Dicom::parse(&data).unwrap();
        d.dataset
            .get(0x0008_0018)
            .map(|e| {
                String::from_utf8_lossy(d.values(&data).text_bytes(e))
                    .trim_end_matches('\0')
                    .trim()
                    .to_owned()
            })
            .unwrap()
    };

    let basic = open("sr_basic.dcm");
    assert_eq!(basic.kind, sr::Kind::Basic);
    assert_eq!(basic.title(), "Radiology Report");
    assert!(!basic.is_measurement_report());
    let lines = basic.lines(false);
    assert!(
        lines
            .iter()
            .any(|(d, l)| *d == 1 && l == "Person Observer Name: •••")
    );
    assert!(
        basic
            .lines(true)
            .iter()
            .any(|(_, l)| l == "Person Observer Name: Dr Marie Curie")
    );
    assert!(
        lines
            .iter()
            .any(|(d, l)| *d == 2 && l.ends_with("Größe stabil."))
    );

    let enhanced = open("sr_enhanced.dcm");
    assert_eq!(enhanced.kind, sr::Kind::Enhanced);
    assert_eq!(enhanced.measurements(), 4);
    let lines: Vec<String> = enhanced.lines(true).into_iter().map(|(_, l)| l).collect();
    for want in [
        "Length: 30.5 mm",
        "Area: 706.86 mm2",
        "Attenuation Coefficient: 60 [hnsf'U]",
        "Ratio: 0.75",
    ] {
        assert!(lines.iter().any(|l| l == want), "{want} not in {lines:?}");
    }
    // The polyline SCOORD jumps to its image at the centroid of its points.
    let region = enhanced.at(&[1, 5]).unwrap();
    match region.jump() {
        Some(Jump::Image {
            sop_instance,
            frame,
            point: Some(p),
        }) => {
            assert_eq!(sop_instance, ct4);
            assert_eq!(frame, None);
            assert!(
                (p[0] - 14.4).abs() < 1e-4 && (p[1] - 15.0).abs() < 1e-4,
                "{p:?}"
            );
        }
        j => panic!("{j:?}"),
    }
    assert_eq!(enhanced.references().len(), 3);

    let comprehensive = open("sr_comprehensive.dcm");
    assert_eq!(comprehensive.kind, sr::Kind::Comprehensive);
    let failed = comprehensive.at(&[1, 2]).unwrap();
    assert_eq!(failed.display_value(), "Measurement failure");

    let tid = open("sr_tid1500.dcm");
    assert!(tid.is_measurement_report());
    assert_eq!(tid.kind, sr::Kind::Comprehensive3d);
    assert_eq!(tid.measurements(), 2);
    let tid3d = open("sr_tid1500_3d.dcm");
    let (_, region) = tid3d
        .walk()
        .into_iter()
        .find(|(_, n)| n.value_type == "SCOORD3D")
        .unwrap();
    match region.jump() {
        Some(Jump::Patient { point, .. }) => {
            // Centroid of the closed polygon's five points (the first repeated).
            assert!((point[0] + 4.0).abs() < 1e-9 && (point[1] + 4.0).abs() < 1e-9);
            assert!((point[2] - 20.0).abs() < 1e-9);
        }
        j => panic!("{j:?}"),
    }
}

/// Compares waveform channels with oracle rows (file, group, channel, label, frequency,
/// samples, min, max, mean, every 37th sample); returns (values compared, mismatches).
fn compare_waveforms(oracle: &[Vec<&str>], open: impl Fn(&str) -> Vec<u8>) -> (usize, usize) {
    let (mut values, mut bad) = (0usize, 0usize);
    // The oracle prints 6 significant digits.
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-3 + 1e-5 * b.abs();
    for r in oracle {
        let data = open(r[0]);
        let d = Dicom::parse(&data).unwrap();
        assert!(Waveforms::present(&d), "{}", r[0]);
        let w = Waveforms::read(&d, &data).unwrap();
        let g = &w.groups[r[1].parse::<usize>().unwrap()];
        let c = &g.channels[r[2].parse::<usize>().unwrap()];
        assert_eq!(c.label, r[3]);
        assert_eq!(g.frequency, r[4].parse::<f64>().unwrap());
        assert_eq!(c.samples.len(), r[5].parse::<usize>().unwrap());
        let x: Vec<f64> = c.samples.iter().map(|&v| v as f64).collect();
        let min = x.iter().copied().fold(f64::INFINITY, f64::min);
        let max = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mean = x.iter().sum::<f64>() / x.len() as f64;
        let stride = r[9].split(',').map(|s| s.parse::<f64>().unwrap());
        let want = [r[6], r[7], r[8]].map(|s| s.parse::<f64>().unwrap());
        for (got, want) in [min, max, mean]
            .into_iter()
            .zip(want)
            .chain(x.iter().step_by(37).copied().zip(stride))
        {
            values += 1;
            if !close(got, want) {
                bad += 1;
                eprintln!("{} {} {}: {got} != {want}", r[0], r[1], r[2]);
            }
        }
    }
    (values, bad)
}

#[test]
fn waveforms_match_pydicom() {
    let oracle = rows(include_str!("data/waveform_oracle.tsv"));
    let (values, bad) = compare_waveforms(&oracle, |f| read(&format!("dicomrt/{f}")));
    eprintln!(
        "waveforms: {} channels, {values} values, {bad} mismatches against pydicom",
        oracle.len()
    );
    assert_eq!(bad, 0);
}

#[test]
fn twelve_lead_layout_and_annotations() {
    let data = read("dicomrt/ecg/ecg_12lead.dcm");
    let d = Dicom::parse(&data).unwrap();
    let w = Waveforms::read(&d, &data).unwrap();
    assert_eq!(w.kind, waveform::Kind::TwelveLead);
    let g = &w.groups[0];
    assert_eq!(g.duration(), 10.0);
    let leads = g.twelve_leads().expect("12 leads");
    assert_eq!(leads, std::array::from_fn(|i| i));
    assert_eq!(g.channels[0].unit_symbol(), "uV");
    assert_eq!(g.channels[0].millivolts(), Some(0.001));
    let layout = waveform::layout(g, Paper::default(), 0.0);
    assert!(layout.twelve_lead);
    assert_eq!((layout.width, layout.height), waveform::page_size(g));
    // Four columns of three leads and the lead II rhythm strip.
    assert_eq!(layout.cells.len(), 13);
    assert_eq!(layout.cells[12].label, "II");
    assert_eq!(layout.cells[12].seconds, 10.0);
    assert_eq!(layout.cells[3].label, "aVR");
    assert_eq!(layout.cells[3].t0, 2.5);
    assert_eq!(waveform::rate(0.8), Some(75.0));
    assert!(w.annotations.iter().any(|a| {
        a.text == "75 /min"
            && a.concept
                .as_ref()
                .is_some_and(|c| c.meaning == "Ventricular Heart Rate")
    }));
    assert!(
        w.annotations
            .iter()
            .any(|a| a.text == "Sinus rhythm. Synthetic test tracing.")
    );

    let data = read("dicomrt/ecg/hemodynamic.dcm");
    let d = Dicom::parse(&data).unwrap();
    let w = Waveforms::read(&d, &data).unwrap();
    assert_eq!(w.kind, waveform::Kind::Hemodynamic);
    assert_eq!(w.groups.len(), 2);
    assert_eq!(w.groups[0].channels[0].unit_symbol(), "mm[Hg]");
    let layout = waveform::layout(&w.groups[0], Paper::default(), 0.0);
    assert!(!layout.twelve_lead);
    assert_eq!(layout.cells.len(), 2);
}

struct Study {
    set: StructureSet,
}

fn study() -> Study {
    let data = read("dicomrt/study/rtstruct.dcm");
    let d = Dicom::parse(&data).unwrap();
    Study {
        set: StructureSet::read(&d, &data).unwrap(),
    }
}

fn dose(name: &str) -> DoseGrid {
    let data = read(&format!("dicomrt/study/{name}"));
    let d = Dicom::parse(&data).unwrap();
    DoseGrid::read(&d, &data).unwrap()
}

/// Compares the DVHs of `set` over `grid` with oracle rows (dose, ROI number, name, volume,
/// max, min, mean, D95, D100, V20Gy, cumulative every 50 cGy); returns (values, mismatches).
fn compare_dvhs(oracle: &[&[&str]], set: &StructureSet, grid: &DoseGrid) -> (usize, usize) {
    let mut bad = 0;
    let mut values = 0;
    for r in oracle {
        let number: i64 = r[1].parse().unwrap();
        let roi = set
            .rois
            .iter()
            .find(|r| r.segment.number == number)
            .unwrap();
        assert_eq!(clean(&roi.segment.label), r[2]);
        let dvh = grid.dvh(roi);
        let num = |i: usize| r[i].parse::<f64>().unwrap();
        let cum = dvh.cumulative();
        let mut check = |what: &str, got: f64, want: f64, tol: f64| {
            values += 1;
            // Plus the oracle's print precision on large volumes.
            if (got - want).abs() > tol + 1e-9 * want.abs() {
                bad += 1;
                eprintln!("{} {what}: {got} != {want}", r[2]);
            }
        };
        check("volume", dvh.volume, num(3), 1e-6);
        check("max", dvh.max(), num(4), 1e-6);
        check("min", dvh.min(), num(5), 1e-6);
        check("mean", dvh.mean(), num(6), 1e-5);
        check("D95", dvh.dose_at(95.0), num(7), 1e-6);
        check("D100", dvh.dose_at(100.0), num(8), 1e-6);
        check("V20Gy", dvh.volume_at(20.0), num(9), 1e-5);
        let want: Vec<f64> = r[10].split(',').map(|s| s.parse().unwrap()).collect();
        check(
            "bins",
            (cum.len().div_ceil(50)) as f64,
            want.len() as f64,
            0.0,
        );
        for (i, w) in want.iter().enumerate() {
            check(
                "cumulative",
                cum.get(i * 50).copied().unwrap_or(0.0),
                *w,
                1e-5,
            );
        }
    }
    (values, bad)
}

#[test]
fn dvh_matches_dicompyler() {
    let s = study();
    let grid = dose("rtdose.dcm");
    let oracle = rows(include_str!("data/dvh_oracle.tsv"));
    assert_eq!(oracle.len(), 4);
    let oracle: Vec<&[&str]> = oracle.iter().map(Vec::as_slice).collect();
    let (values, bad) = compare_dvhs(&oracle, &s.set, &grid);
    eprintln!(
        "DVH: {} ROIs, {values} values, {bad} mismatches against dicompyler-core",
        oracle.len()
    );
    assert_eq!(bad, 0);
}

#[test]
fn absolute_grid_offsets_give_the_same_dose() {
    let s = study();
    let relative = dose("rtdose.dcm");
    let absolute = dose("rtdose_absolute.dcm");
    assert_eq!(relative.frames(), 15);
    for f in 0..relative.frames() {
        let (a, b) = (relative.frame_position(f), absolute.frame_position(f));
        assert!((a[2] - b[2]).abs() < 1e-9, "{f}: {a:?} {b:?}");
        assert!((a[2] - (1.0 + 3.0 * f as f64)).abs() < 1e-9);
    }
    for roi in &s.set.rois {
        assert_eq!(
            relative.dvh(roi),
            absolute.dvh(roi),
            "{}",
            roi.segment.label
        );
    }
    let all = rtdose::dvhs(&relative, &s.set);
    assert_eq!(all.len(), 4);
    assert_eq!(relative.units, "GY");
    let peak = relative.values.iter().copied().fold(0.0, f64::max) * relative.scaling;
    assert_eq!(relative.max_dose(), peak);
    assert!((peak - 59.924075).abs() < 1e-9, "{peak}");
}

/// The highest dose (Gy) of frame 7 of rtdose.dcm.
fn relative_peak() -> f64 {
    let g = dose("rtdose.dcm");
    g.frame(7).iter().copied().fold(0.0, f64::max) * g.scaling
}

#[test]
fn prescription_and_isodose_levels() {
    let data = read("dicomrt/study/rtplan.dcm");
    let d = Dicom::parse(&data).unwrap();
    let p = Prescription::read(&d, &data).unwrap();
    assert_eq!(p.dose, 56.0);
    assert_eq!(p.label, "Phantom plan");
    let lines = rtdose::isodose_lines(p.dose);
    assert_eq!(lines.len(), 9);
    assert!(
        lines
            .iter()
            .any(|l| l.1 == 95.0 && (l.0 - 53.2).abs() < 1e-9),
        "{lines:?}"
    );
    // The dose loads as a volume: frame positions from the Grid Frame Offset Vector and
    // Dose Grid Scaling as the rescale slope, in Gy.
    let data = read("dicomrt/study/rtdose.dcm");
    let d = Dicom::parse(&data).unwrap();
    let m = d.modality_frame(&data, 7, None).unwrap();
    assert_eq!(
        *m.modality(),
        specular_dicom::display::Modality::Rescale {
            slope: 2.5e-5,
            intercept: 0.0
        }
    );
    assert_eq!(m.unit(), Some("Gy"));
    let peak = (0..26)
        .flat_map(|y| (0..26).map(move |x| (x, y)))
        .filter_map(|(x, y)| m.value_at(x, y))
        .fold(0.0, f64::max);
    assert!((peak - relative_peak()).abs() < 1e-9, "{peak}");
}

#[test]
fn oct_frames_are_located_on_the_localiser() {
    let data = read("dicomrt/oct/oct.dcm");
    let d = Dicom::parse(&data).unwrap();
    let o = Oct::read(&d, &data).unwrap();
    assert_eq!(o.frames, 8);
    assert_eq!(o.locations.len(), 8);
    let op = {
        let data = read("dicomrt/oct/op.dcm");
        let d = Dicom::parse(&data).unwrap();
        assert!(Oct::read(&d, &data).is_none());
        d.dataset
            .get(0x0008_0018)
            .map(|e| {
                String::from_utf8_lossy(d.values(&data).text_bytes(e))
                    .trim_end_matches('\0')
                    .trim()
                    .to_owned()
            })
            .unwrap()
    };
    assert_eq!(o.localiser(), Some(op.as_str()));
    let l = o.location(3).unwrap();
    assert_eq!(l.orientation, "LINEAR");
    // (row, column) pairs on file, (column, row) here.
    assert_eq!(l.points, vec![[8.0, 27.5], [56.0, 27.5]]);
    // The header alone gives the same.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/dicomrt/oct/oct.dcm");
    assert_eq!(Oct::read_file(&path).unwrap(), Some(o));
    let op = path.with_file_name("op.dcm");
    assert_eq!(Oct::read_file(&op).unwrap(), None);
}

// ----------------------------------------------------------------------------------- corpus

/// The corpus of tools/corpus/fetch-dicomrt.sh: `SPECULAR_DICOMRT_CORPUS`, else
/// tools/corpus/dicomrt of this checkout; None (the tests skip) when absent.
fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_DICOMRT_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/dicomrt")
        });
    p.join("oracle.tsv").is_file().then_some(p)
}

/// The rows of one kind (S, W or D) of the corpus oracle, without the kind.
fn corpus_rows(dir: &Path, kind: &str) -> Vec<Vec<String>> {
    std::fs::read_to_string(dir.join("oracle.tsv"))
        .unwrap()
        .lines()
        .map(|l| l.split('\t').map(str::to_owned).collect::<Vec<_>>())
        .filter(|f| f[0] == kind)
        .map(|f| f[1..].to_vec())
        .collect()
}

fn borrowed(rows: &[Vec<String>]) -> Vec<Vec<&str>> {
    rows.iter()
        .map(|r| r.iter().map(String::as_str).collect())
        .collect()
}

#[test]
fn corpus_matches_pydicom_and_dicompyler() {
    let Some(dir) = corpus() else {
        eprintln!("dicomrt corpus absent; skipped");
        return;
    };
    let open = |f: &str| std::fs::read(dir.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));

    let sr = corpus_rows(&dir, "S");
    let (items, sr_bad) = compare_sr(&borrowed(&sr), open);
    eprintln!("corpus SR: {items} content items, {sr_bad} mismatches against pydicom");

    let w = corpus_rows(&dir, "W");
    let (values, w_bad) = compare_waveforms(&borrowed(&w), open);
    eprintln!(
        "corpus waveforms: {} channels, {values} values, {w_bad} mismatches against pydicom",
        w.len()
    );

    let d = corpus_rows(&dir, "D");
    let mut pairs: Vec<(&str, &str)> = d.iter().map(|r| (r[0].as_str(), r[1].as_str())).collect();
    pairs.dedup();
    let (mut values, mut dvh_bad) = (0, 0);
    for (ss, rd) in &pairs {
        let data = open(ss);
        let set = StructureSet::read(&Dicom::parse(&data).unwrap(), &data).unwrap();
        let data = open(rd);
        let grid = DoseGrid::read(&Dicom::parse(&data).unwrap(), &data).unwrap();
        let rows: Vec<Vec<&str>> = d
            .iter()
            .filter(|r| r[0] == *ss && r[1] == *rd)
            .map(|r| r[1..].iter().map(String::as_str).collect())
            .collect();
        let rows: Vec<&[&str]> = rows.iter().map(Vec::as_slice).collect();
        let (v, b) = compare_dvhs(&rows, &set, &grid);
        values += v;
        dvh_bad += b;
    }
    eprintln!(
        "corpus DVH: {} ROIs over {} dose/structure pairs, {values} values, \
         {dvh_bad} mismatches against dicompyler-core",
        d.len(),
        pairs.len()
    );
    assert_eq!((sr_bad, w_bad, dvh_bad), (0, 0, 0));
}

#[test]
fn corpus_opens_timed() {
    let Some(dir) = corpus() else {
        eprintln!("dicomrt corpus absent; skipped");
        return;
    };
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "dcm") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&dir, &mut files);
    files.sort();
    for f in files {
        let data = std::fs::read(&f).unwrap();
        let t = std::time::Instant::now();
        let d = Dicom::parse(&data).unwrap();
        let what = if Waveforms::present(&d) {
            let w = Waveforms::read(&d, &data).unwrap();
            format!("{} waveform groups", w.groups.len())
        } else if let Ok(r) = Report::read(&d, &data) {
            format!("{} SR items", r.root.count())
        } else if let Ok(g) = DoseGrid::read(&d, &data) {
            format!("dose {}x{}x{}", g.columns, g.rows, g.frames())
        } else if let Ok(s) = StructureSet::read(&d, &data) {
            format!("{} ROIs", s.rois.len())
        } else {
            "other".into()
        };
        eprintln!(
            "{:>8.2} ms  {}  {what}",
            t.elapsed().as_secs_f64() * 1e3,
            f.strip_prefix(&dir).unwrap().display()
        );
    }
}
