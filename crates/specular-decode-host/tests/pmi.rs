//! STEP PMI through OpenCASCADE's XDE in the sandboxed host, against an independent reading.
//!
//! The corpus is NIST's MBE PMI test models (`tools/corpus/fetch-pmi.sh`), read from
//! `SPECULAR_PMI_CORPUS`, else tools/corpus/pmi; `tools/corpus/pmi-oracle.py` reads the same
//! files with its own STEP parser into `pmi-oracle.tsv`. Per file, the facts both give are
//! compared as multisets: dimension values with their plus/minus tolerances, limits and fits,
//! geometric tolerance types, values, cylindrical zones, modifiers and datum reference
//! frames, datum letters, datum targets and saved views. Mismatches are listed; the test
//! fails when they exceed the known count (OCCT's own gaps, named in `KNOWN`). Every test
//! skips when the host has no OpenCASCADE (unless `SPECULAR_REQUIRE_OCCT=1`) or the corpus is
//! absent.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use specular_sandbox::cad::{PMI_PLUS_MINUS, PMI_RANGE, PMI_VALUE};
use specular_sandbox::protocol::{CadFormat, CadLimits};
use specular_sandbox::{
    CadModel, Config, FORM_VARIANCES, GRADES, Host, PmiKind, TOLERANCE_MODIFIERS, TOLERANCE_TYPES,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn occt_host() -> Option<Host> {
    let h = Host::new(Config {
        exe: Some(env!("CARGO_BIN_EXE_specular-decode-host").into()),
        timeout: Duration::from_secs(300),
        pool_size: 1,
        memory_limit: 8 << 30,
        occt: true,
        ..Config::default()
    });
    let info = h.info().expect("host starts");
    if info.occt.is_none() {
        let must = std::env::var_os("SPECULAR_REQUIRE_OCCT").is_some_and(|v| v == "1");
        assert!(!must, "OpenCASCADE required: {}", info.occt_error);
        eprintln!("skipped: no OpenCASCADE ({})", info.occt_error);
        return None;
    }
    Some(h)
}

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_PMI_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tools/corpus/pmi"
            ))
        });
    if dir.join("pmi-oracle.tsv").is_file() {
        Some(dir)
    } else {
        eprintln!("skipped: no PMI corpus at {}", dir.display());
        None
    }
}

/// `%.6g`, as the oracle writes numbers.
fn g6(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let e = v.abs().log10().floor() as i32;
    let decimals = (5 - e).max(0) as usize;
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    };
    if s == "-0" { "0".into() } else { s }
}

/// The facts the oracle writes, from what the host sent (lengths in millimetres).
fn facts(m: &CadModel) -> Vec<String> {
    let mm = m.unit_metres * 1000.0;
    let mut out = Vec::new();
    let mut targets = 0;
    for p in &m.pmi {
        match p.kind {
            PmiKind::Dimension => {
                // Angles are in degrees already.
                let k = if matches!(p.code, 11 | 28) { 1.0 } else { mm };
                let g6 = |v: f64| g6(v * k);
                let value = if p.flags & PMI_VALUE != 0 {
                    g6(p.value)
                } else {
                    "-".into()
                };
                let (plus, minus) = if p.flags & PMI_PLUS_MINUS != 0 {
                    (g6(p.upper), g6(p.lower.abs()))
                } else {
                    ("-".into(), "-".into())
                };
                out.push(format!("dim\t{value}\t{plus}\t{minus}"));
                if p.flags & PMI_RANGE != 0 {
                    out.push(format!("range\t{}\t{}", g6(p.lower), g6(p.upper)));
                }
                if p.flags & specular_sandbox::cad::PMI_FIT != 0 {
                    let v = FORM_VARIANCES.get(p.fit_variance as usize).unwrap_or(&"");
                    let g = GRADES.get(p.fit_grade as usize).unwrap_or(&"");
                    out.push(format!("fit\t{v}{g}"));
                }
            }
            PmiKind::GeometricTolerance => {
                let kind = TOLERANCE_TYPES.get(p.code as usize).unwrap_or(&"");
                let mut mods: Vec<&str> = p
                    .modifiers
                    .iter()
                    .filter_map(|&c| TOLERANCE_MODIFIERS.get(c as usize).copied())
                    .collect();
                match p.material {
                    1 => mods.push("MAXIMUM_MATERIAL_REQUIREMENT"),
                    2 => mods.push("LEAST_MATERIAL_REQUIREMENT"),
                    _ => {}
                }
                mods.sort_unstable();
                mods.dedup();
                let mods = if mods.is_empty() {
                    "-".to_owned()
                } else {
                    mods.join(",")
                };
                // Datums at one position form a common datum ("A-B").
                let mut frame: BTreeMap<u8, Vec<&str>> = BTreeMap::new();
                for d in &p.datums {
                    frame.entry(d.position).or_default().push(&d.letter);
                }
                let frame = frame
                    .values()
                    .map(|v| v.join("-"))
                    .collect::<Vec<_>>()
                    .join("|");
                let frame = if frame.is_empty() { "-".into() } else { frame };
                let dia = if p.value_type == 1 { "D" } else { "-" };
                let value = g6(p.value * mm);
                out.push(format!("tol\t{kind}\t{value}\t{dia}\t{mods}\t{frame}"));
            }
            PmiKind::Datum => out.push(format!("datum\t{}", p.letter)),
            PmiKind::DatumTarget => targets += 1,
            PmiKind::Graphic => {}
        }
    }
    out.push(format!("target\t{targets}"));
    out.push(format!("views\t{}", m.views.len()));
    out
}

/// One file's facts as the oracle wrote them.
fn oracle(dir: &std::path::Path) -> BTreeMap<String, Vec<String>> {
    let text = std::fs::read_to_string(dir.join("pmi-oracle.tsv")).unwrap();
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in text.lines().filter(|l| !l.is_empty()) {
        let (file, fact) = l.split_once('\t').unwrap();
        out.entry(file.to_owned())
            .or_default()
            .push(fact.to_owned());
    }
    out
}

/// Spellings the two readings differ in that mean the same.
fn same(f: &str) -> String {
    f.replace("\tROUNDNESS\t", "\tCIRCULARITY\t")
}

/// Facts in `a` not in `b` (as multisets).
fn missing(a: &[String], b: &[String]) -> Vec<String> {
    let mut left: Vec<String> = b.iter().map(|s| same(s)).collect();
    let mut out = Vec::new();
    for f in a {
        let f = same(f);
        if let Some(i) = left.iter().position(|x| *x == f) {
            left.swap_remove(i);
        } else {
            out.push(f);
        }
    }
    out
}

/// Mismatches known, all OCCT 7.9's own gaps (the bridge already makes up for tolerance
/// magnitudes given as measure representation items, qualified values left in inches and
/// views it stops reading at a camera in a characterized drawing model), on 829 oracle facts:
/// - 34 geometric tolerances XDE does not create (STC-07 has 22, OCCT reads 10; FTC-08 and
///   FTC-10 likewise), with their datum letters (11);
/// - 19 dimensions: DIMENSIONAL_SIZE_WITH_DATUM_FEATURE and DIRECTED_DIMENSIONAL_LOCATION are
///   not read, and a lower limit next to a nominal value is taken for the nominal (2 ranges);
/// - 3 fits written "G6" in the form variance; 4 datum targets.
const KNOWN: usize = 73;

#[test]
fn nist_pmi_against_independent_reader() {
    let (Some(h), Some(dir)) = (occt_host(), corpus()) else {
        return;
    };
    let limits = CadLimits {
        relative_deflection: 0.002,
        ..CadLimits::default()
    };
    let (mut total, mut bad_total) = (0usize, 0usize);
    let mut kinds: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (file, theirs) in oracle(&dir) {
        let bytes = std::fs::read(dir.join(&file)).unwrap();
        let t0 = Instant::now();
        let raw = h
            .cad_read(&bytes, CadFormat::Step, limits)
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let m = CadModel::decode(&raw, limits.max_triangles, limits.max_triangles * 3).unwrap();
        let ours = facts(&m);
        let lost = missing(&theirs, &ours);
        let extra = missing(&ours, &theirs);
        for f in &theirs {
            let k = f.split('\t').next().unwrap_or("").to_owned();
            kinds.entry(k).or_default().0 += 1;
        }
        for f in lost.iter().chain(&extra) {
            let k = f.split('\t').next().unwrap_or("").to_owned();
            kinds.entry(k).or_default().1 += 1;
        }
        total += theirs.len();
        bad_total += lost.len() + extra.len();
        let faced = m.pmi.iter().filter(|p| !p.faces.is_empty()).count();
        eprintln!(
            "{file}: unit {} m, {} PMI ({} with faces), {} views, {} facts, {} mismatched, {ms:.0} ms",
            m.unit_metres,
            m.pmi.len(),
            faced,
            m.views.len(),
            theirs.len(),
            lost.len() + extra.len()
        );
        for f in &lost {
            eprintln!("    oracle only: {}", f.replace('\t', " "));
        }
        for f in &extra {
            eprintln!("    OCCT only:   {}", f.replace('\t', " "));
        }
    }
    for (k, (n, bad)) in &kinds {
        eprintln!("{k}: {n} oracle facts, {bad} mismatched");
    }
    eprintln!("PMI: {total} oracle facts, {bad_total} mismatched");
    assert!(total > 0);
    assert!(bad_total <= KNOWN, "{bad_total} mismatches, {KNOWN} known");
}

/// A hand-made assembly (`tests/data/pmi-twice.step`, `tools/corpus/pmi-assembly.py`) using
/// one annotated block twice, the second turned 90° about z and moved to (150, 50, 0):
/// each PMI comes with its faces on both instances, the same triangulation placed by each
/// instance's transform (what `specular-decode` turns into one annotation per instance).
#[test]
fn pmi_on_a_part_used_twice() {
    let Some(h) = occt_host() else {
        return;
    };
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/pmi-twice.step"
    ))
    .unwrap();
    let limits = CadLimits::default();
    let raw = h.cad_read(&bytes, CadFormat::Step, limits).unwrap();
    let m = CadModel::decode(&raw, limits.max_triangles, limits.max_triangles * 3).unwrap();
    let f = facts(&m);
    assert!(f.contains(&"dim\t100\t0.2\t0.2".to_owned()), "{f:?}");
    assert!(
        f.iter().any(|x| x.starts_with("tol\tFLATNESS\t0.05")),
        "{f:?}"
    );
    let nodes: std::collections::BTreeSet<u32> = m.meshes.iter().map(|x| x.node).collect();
    assert_eq!(nodes.len(), 2, "two instances");
    let mm = (m.unit_metres * 1000.0) as f32;
    for p in m.pmi.iter().filter(|p| p.kind != PmiKind::Graphic) {
        // Faces grouped by instance.
        let mut by_node: BTreeMap<u32, Vec<[u32; 3]>> = BTreeMap::new();
        for fc in &p.faces {
            by_node
                .entry(m.meshes[fc[0] as usize].node)
                .or_default()
                .push(*fc);
        }
        assert_eq!(
            by_node.len(),
            2,
            "{} on both instances: {:?}",
            p.name,
            p.faces
        );
        let groups: Vec<&Vec<[u32; 3]>> = by_node.values().collect();
        assert_eq!(groups[0].len(), groups[1].len());
        // Vertex for vertex, the second instance is the first turned and moved.
        let tri = |fc: &[u32; 3]| -> Vec<[f32; 3]> {
            let me = &m.meshes[fc[0] as usize];
            let lo = fc[1] as usize * 3;
            me.indices[lo..lo + fc[2] as usize * 3]
                .iter()
                .map(|&i| me.positions[i as usize].map(|v| v * mm))
                .collect()
        };
        let place = |p: [f32; 3], k: usize| -> [f32; 3] {
            if k == 0 {
                p
            } else {
                [150.0 - p[1], 50.0 + p[0], p[2]]
            }
        };
        // Which group is the unmoved block: the one inside 0..100 in x.
        let first = if tri(&groups[0][0])
            .iter()
            .all(|q| q[0] > -0.01 && q[0] < 100.01)
        {
            0
        } else {
            1
        };
        for (fa, fb) in groups[first].iter().zip(groups[1 - first].iter()) {
            for (a, b) in tri(fa).iter().zip(tri(fb)) {
                let want = place(*a, 1);
                let d = (0..3).map(|k| (want[k] - b[k]).abs()).fold(0f32, f32::max);
                assert!(d < 1e-3, "{}: {a:?} → {b:?}, want {want:?}", p.name);
            }
        }
    }
}
