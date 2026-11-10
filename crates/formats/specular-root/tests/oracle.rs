//! The fixtures in tests/data (written by gen/make_fixtures.py with uproot) against uproot's
//! reading of them (tests/data/expected.tsv, written by gen/oracle.py): file version,
//! compression and keys, every histogram's class, title, entries, axes (bins, range, title,
//! variable edges, labels), stored sums, cells with flow (count, sum, sum of squares and up
//! to 64 strided values), sumw2, tree entry counts and graph points.
//!
//! The fetched corpus (tools/corpus/fetch-root.sh: scikit-hep-testdata and hsimple.root) is
//! checked the same way when present, at `SPECULAR_ROOT_DIR` with `SPECULAR_ROOT_ORACLE`
//! (default: tools/corpus/root and its expected.tsv).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_root::{Hist, Object, RootFile};

#[derive(Default)]
struct Case {
    file: Option<Vec<String>>,
    keys: Vec<(String, String)>,
    /// Rows by object key (`path;cycle`).
    objs: BTreeMap<String, Vec<Vec<String>>>,
    /// uproot could not open the file at all.
    file_error: Option<String>,
}

fn num(s: &str) -> f64 {
    match s {
        "nan" => f64::NAN,
        "inf" => f64::INFINITY,
        "-inf" => f64::NEG_INFINITY,
        t => t.parse().unwrap_or_else(|_| panic!("number {t}")),
    }
}

fn load(tsv: &Path) -> BTreeMap<String, Case> {
    let text = std::fs::read_to_string(tsv).expect("oracle tsv");
    let mut cases: BTreeMap<String, Case> = BTreeMap::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<String> = line.split('\t').map(str::to_owned).collect();
        let c = cases.entry(f[1].clone()).or_default();
        match f[0].as_str() {
            "file" => c.file = Some(f[2..].to_vec()),
            "key" => c.keys.push((f[2].clone(), f[3].clone())),
            "error" if f[2].is_empty() => c.file_error = Some(f[3].clone()),
            _ => c.objs.entry(f[2].clone()).or_default().push(f),
        }
    }
    cases
}

/// Equal within a relative tolerance (sums are accumulated in a different order).
fn close(a: f64, b: f64, rel: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b || (a - b).abs() <= rel * a.abs().max(b.abs())
}

fn escape(s: &str) -> String {
    s.replace('\t', " ")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn fmt_edges(v: &[f64]) -> Vec<f64> {
    v.to_vec()
}

#[derive(Default)]
struct Tally {
    files: usize,
    keys: usize,
    hists: usize,
    cells: usize,
    trees: usize,
    graphs: usize,
    refused_both: usize,
    /// uproot refused it; we read it.
    ours_only: usize,
    mismatches: Vec<String>,
    largest: Option<(u64, String, f64, f64)>,
}

/// Compares one histogram with its rows; mismatch descriptions go to `bad`.
fn check_hist(h: &Hist, rows: &[Vec<String>], t: &mut Tally, bad: &mut Vec<String>) {
    t.hists += 1;
    for r in rows {
        match r[0].as_str() {
            "hist" => {
                if h.class != r[3] {
                    bad.push(format!("class {} vs {}", h.class, r[3]));
                }
                if h.dims.to_string() != r[4] {
                    bad.push(format!("dims {} vs {}", h.dims, r[4]));
                }
                if escape(&h.title) != r[5] {
                    bad.push(format!("title {:?} vs {:?}", h.title, r[5]));
                }
                if !close(h.entries, num(&r[6]), 0.0) {
                    bad.push(format!("entries {} vs {}", h.entries, r[6]));
                }
            }
            "axis" => {
                let k = match r[3].as_str() {
                    "x" => 0,
                    "y" => 1,
                    _ => 2,
                };
                let Some(a) = h.axes.get(k) else {
                    bad.push(format!("no axis {}", r[3]));
                    continue;
                };
                let edges: Vec<f64> = if r[8].is_empty() {
                    Vec::new()
                } else {
                    r[8].split(',').map(num).collect()
                };
                let labels: Vec<String> = if r[9].is_empty() {
                    Vec::new()
                } else {
                    r[9].split('|').map(str::to_owned).collect()
                };
                let ours: Vec<String> = a.labels.iter().map(|(_, s)| escape(s)).collect();
                if a.nbins.to_string() != r[4]
                    || a.min != num(&r[5])
                    || a.max != num(&r[6])
                    || escape(&a.title) != r[7]
                    || fmt_edges(&a.edges) != edges
                    || ours != labels
                {
                    bad.push(format!(
                        "axis {}: {} [{}, {}] {:?} {:?} {:?} vs {:?}",
                        r[3],
                        a.nbins,
                        a.min,
                        a.max,
                        a.title,
                        a.edges.len(),
                        ours,
                        &r[4..]
                    ));
                }
            }
            "stat" => {
                let want = num(&r[4]);
                let got = match r[3].as_str() {
                    "fMaximum" => Some(h.maximum),
                    "fMinimum" => Some(h.minimum),
                    m => h.sum(m),
                };
                match got {
                    Some(g) if close(g, want, 0.0) => {}
                    g => bad.push(format!("{} {g:?} vs {want}", r[3])),
                }
            }
            "cells" => {
                let n: usize = r[3].parse().expect("count");
                let sum: f64 = h.cells.iter().sum();
                let sq: f64 = h.cells.iter().map(|v| v * v).sum();
                t.cells += h.cells.len();
                if h.cells.len() != n
                    || !close(sum, num(&r[4]), 1e-9)
                    || !close(sq, num(&r[5]), 1e-9)
                {
                    bad.push(format!(
                        "cells {} {sum} {sq} vs {n} {} {}",
                        h.cells.len(),
                        r[4],
                        r[5]
                    ));
                }
            }
            "cell" => {
                let i: usize = r[3].parse().expect("index");
                let want = num(&r[4]);
                match h.cells.get(i) {
                    Some(&v) if close(v, want, if h.profile { 1e-12 } else { 0.0 }) => {}
                    v => bad.push(format!("cell {i}: {v:?} vs {want}")),
                }
            }
            "sumw2" => {
                let n: usize = r[3].parse().expect("count");
                let sum: f64 = h.sumw2.iter().sum();
                if h.sumw2.len() != n || !close(sum, num(&r[4]), 1e-9) {
                    bad.push(format!("sumw2 {} {sum} vs {n} {}", h.sumw2.len(), r[4]));
                }
            }
            "error" => {}
            other => bad.push(format!("row {other} for a histogram")),
        }
    }
}

fn check(dir: &Path, cases: &BTreeMap<String, Case>) -> Tally {
    let mut t = Tally::default();
    for (name, case) in cases {
        let path = dir.join(name);
        let Ok(data) = std::fs::read(&path) else {
            t.mismatches.push(format!("{name}: missing"));
            continue;
        };
        t.files += 1;
        let t0 = Instant::now();
        let opened = RootFile::open(&data);
        let open_ms = t0.elapsed().as_secs_f64() * 1e3;
        let f = match (opened, &case.file_error) {
            (Ok(f), _) => f,
            (Err(_), Some(_)) => {
                t.refused_both += 1;
                continue;
            }
            (Err(e), None) => {
                t.mismatches.push(format!("{name}: open: {e}"));
                continue;
            }
        };
        if case.file_error.is_some() {
            t.ours_only += 1;
        }
        if let Some(row) = &case.file {
            let ok = row[0].parse::<u32>().ok().map(|v| v % 1_000_000) == Some(f.version)
                && row[1] == f.compress.to_string()
                && row[2] == f.keys.len().to_string();
            if !ok {
                t.mismatches.push(format!(
                    "{name}: file {} {} {} vs {row:?} ({:?})",
                    f.version,
                    f.compress,
                    f.keys.len(),
                    f.warnings
                ));
            }
        }
        // Keys by path;cycle.
        let ours: BTreeMap<String, &specular_root::Key> = f
            .keys
            .iter()
            .map(|k| (format!("{};{}", k.path(), k.cycle), k))
            .collect();
        for (key, class) in &case.keys {
            t.keys += 1;
            match ours.get(key) {
                Some(k) if &k.class == class => {}
                Some(k) => t
                    .mismatches
                    .push(format!("{name}: {key}: class {} vs {class}", k.class)),
                None => t.mismatches.push(format!("{name}: no key {key}")),
            }
        }
        let mut first_ms = None;
        for (key, rows) in &case.objs {
            let Some(k) = ours.get(key) else {
                t.mismatches
                    .push(format!("{name}: no key {key} for its rows"));
                continue;
            };
            let uproot_failed = rows.iter().any(|r| r[0] == "error");
            let t1 = Instant::now();
            let got = f.read(&data, k);
            if first_ms.is_none() {
                first_ms = Some(t1.elapsed().as_secs_f64() * 1e3);
            }
            let mut bad = Vec::new();
            match (got, uproot_failed) {
                (Err(_), true) => {
                    t.refused_both += 1;
                    continue;
                }
                (Ok(_), true) => {
                    t.ours_only += 1;
                    continue;
                }
                (Err(e), false) => bad.push(format!("refused: {e}")),
                (Ok(Object::Hist(h)), false) => check_hist(&h, rows, &mut t, &mut bad),
                (Ok(Object::Tree(tr)), false) => {
                    t.trees += 1;
                    for r in rows {
                        if r[0] != "tree" || num(&r[3]) != tr.entries as f64 {
                            bad.push(format!("tree {} vs {r:?}", tr.entries));
                        }
                    }
                }
                (Ok(Object::Graph(g)), false) => {
                    t.graphs += 1;
                    for r in rows {
                        let sx: f64 = g.x.iter().sum();
                        let sy: f64 = g.y.iter().sum();
                        if r[0] != "graph"
                            || r[3] != g.x.len().to_string()
                            || !close(sx, num(&r[4]), 1e-12)
                            || !close(sy, num(&r[5]), 1e-12)
                        {
                            bad.push(format!("graph {} {sx} {sy} vs {r:?}", g.x.len()));
                        }
                    }
                }
            }
            for b in bad {
                t.mismatches.push(format!("{name}: {key}: {b}"));
            }
        }
        let size = data.len() as u64;
        if t.largest.as_ref().is_none_or(|l| size > l.0) {
            t.largest = Some((size, name.clone(), open_ms, first_ms.unwrap_or(0.0)));
        }
    }
    t
}

fn report(what: &str, t: &Tally) {
    eprintln!(
        "{what}: {} files, {} keys, {} histograms ({} cells), {} trees, {} graphs; \
         refused by both {}, read only by us {}; {} mismatches",
        t.files,
        t.keys,
        t.hists,
        t.cells,
        t.trees,
        t.graphs,
        t.refused_both,
        t.ours_only,
        t.mismatches.len()
    );
    if let Some((size, name, open, first)) = &t.largest {
        eprintln!(
            "{what}: largest {name} ({:.1} MB): open {open:.2} ms, first object {first:.2} ms",
            *size as f64 / 1e6
        );
    }
    for m in t.mismatches.iter().take(60) {
        eprintln!("  {m}");
    }
}

#[test]
fn fixtures_match_uproot() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let cases = load(&dir.join("expected.tsv"));
    let t = check(&dir, &cases);
    report("fixtures", &t);
    assert!(t.files >= 11 && t.hists >= 30, "too few cases");
    assert!(t.mismatches.is_empty());
}

#[test]
fn corpus_matches_uproot() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::var_os("SPECULAR_ROOT_DIR")
        .map_or_else(|| root.join("tools/corpus/root"), PathBuf::from);
    let tsv = std::env::var_os("SPECULAR_ROOT_ORACLE")
        .map_or_else(|| dir.join("expected.tsv"), PathBuf::from);
    if !tsv.exists() {
        eprintln!("no corpus oracle at {}; skipped", tsv.display());
        return;
    }
    let cases = load(&tsv);
    let t = check(&dir, &cases);
    report("corpus", &t);
    assert!(t.mismatches.is_empty());
}
