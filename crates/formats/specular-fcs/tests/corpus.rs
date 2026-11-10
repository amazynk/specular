//! Public FCS files (tools/corpus/fetch-cytometry.sh: fcsparser's and FlowIO's test data)
//! against fcsparser and FlowIO (raw channel values, scale values), flowutils
//! (compensation, logicle). Reads SPECULAR_CYTOMETRY_CORPUS, else tools/corpus/cytometry; skips
//! when it has no expected.tsv.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use specular_fcs::scale::Logicle;
use specular_fcs::{Fcs, parse};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_CYTOMETRY_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/cytometry")
        });
    dir.join("expected.tsv").is_file().then_some(dir)
}

fn nums(s: &str) -> Vec<f64> {
    s.split(',').map(|v| v.parse::<f64>().unwrap()).collect()
}

/// Sums agree to a relative 1e-6 (f32 events summed in f64 against doubles).
fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1.0)
}

#[test]
fn corpus_matches_fcsparser_flowio_and_flowutils() {
    let Some(dir) = corpus() else {
        eprintln!("cytometry corpus absent: run tools/corpus/fetch-cytometry.sh");
        return;
    };
    let text = std::fs::read_to_string(dir.join("expected.tsv")).unwrap();
    let mut files: HashMap<String, (Vec<u8>, Fcs)> = HashMap::new();
    let mut mismatches = Vec::new();
    let mut checked = 0;
    let mut values = 0u64;
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f[0] == "logicle" {
            let (t, w, m, a) = (nums(f[1])[0], nums(f[2])[0], nums(f[3])[0], nums(f[4])[0]);
            let l = Logicle::new(t, w, m, a).unwrap();
            for (x, y) in nums(f[5]).into_iter().zip(nums(f[6])) {
                checked += 1;
                if (l.scale(x) - y).abs() > 1e-6 {
                    mismatches.push(format!("logicle T={t} W={w} of {x}: {} vs {y}", l.scale(x)));
                }
            }
            continue;
        }
        if !matches!(f[0], "fcs" | "flowio" | "comp") {
            continue;
        }
        let rel = f[1].to_owned();
        let (data, fcs) = files.entry(rel.clone()).or_insert_with(|| {
            let d = std::fs::read(dir.join(&rel)).unwrap();
            let p = parse(&d).unwrap_or_else(|e| panic!("{rel}: {e}"));
            (d, p)
        });
        let ds: usize = f[2].parse().unwrap();
        let Some(set) = fcs.datasets.get(ds) else {
            mismatches.push(format!("{rel}: no dataset {ds}"));
            continue;
        };
        checked += 1;
        let col_sums = |ev: &specular_fcs::Events| -> Vec<f64> {
            (0..ev.params)
                .map(|p| ev.column(p).map(f64::from).sum())
                .collect()
        };
        match f[0] {
            "fcs" => {
                let ev = set.read_channels(data).unwrap();
                values += ev.values.len() as u64;
                if ev.count != f[3].parse::<usize>().unwrap() {
                    mismatches.push(format!("{rel}: {} events, fcsparser {}", ev.count, f[3]));
                }
                let names: Vec<&str> = f[4].split(';').collect();
                let ours: Vec<&str> = set.params.iter().map(|p| p.name.trim()).collect();
                if names != ours {
                    mismatches.push(format!("{rel}: names {ours:?}, fcsparser {names:?}"));
                }
                for (k, (a, b)) in col_sums(&ev).into_iter().zip(nums(f[5])).enumerate() {
                    if !close(a, b, 1e-6) {
                        mismatches.push(format!("{rel}: P{} sum {a}, fcsparser {b}", k + 1));
                    }
                }
                let first: Vec<f64> = (0..ev.params).map(|p| f64::from(ev.get(0, p))).collect();
                let last: Vec<f64> = (0..ev.params)
                    .map(|p| f64::from(ev.get(ev.count - 1, p)))
                    .collect();
                for (what, ours, want) in [("first", first, nums(f[6])), ("last", last, nums(f[7]))]
                {
                    if ours.iter().zip(&want).any(|(a, b)| !close(*a, *b, 1e-6)) {
                        mismatches
                            .push(format!("{rel}: {what} event {ours:?}, fcsparser {want:?}"));
                    }
                }
            }
            "flowio" => {
                let ev = set.read_channels(data).unwrap();
                if ev.count != f[3].parse::<usize>().unwrap() {
                    mismatches.push(format!("{rel}#{ds}: {} events, FlowIO {}", ev.count, f[3]));
                }
                for (k, (a, b)) in col_sums(&ev).into_iter().zip(nums(f[4])).enumerate() {
                    if !close(a, b, 1e-6) {
                        mismatches.push(format!("{rel}#{ds}: raw P{} sum {a}, FlowIO {b}", k + 1));
                    }
                }
                if f[5] != "-" {
                    let sc = set.read_events(data, false).unwrap();
                    for (k, (a, b)) in col_sums(&sc).into_iter().zip(nums(f[5])).enumerate() {
                        if !close(a, b, 1e-5) {
                            mismatches
                                .push(format!("{rel}#{ds}: scale P{} sum {a}, FlowIO {b}", k + 1));
                        }
                    }
                }
            }
            "comp" => {
                let p: usize = f[3].parse().unwrap();
                let want: f64 = f[4].parse().unwrap();
                let ev = set.read_events(data, true).unwrap();
                let got: f64 = ev.column(p).map(f64::from).sum();
                if !ev.compensated || !close(got, want, 1e-4) {
                    mismatches.push(format!(
                        "{rel}#{ds}: compensated P{} sum {got}, flowutils {want}",
                        p + 1
                    ));
                }
            }
            _ => {}
        }
    }
    eprintln!(
        "FCS corpus: {checked} checks over {} files ({values} values), {} mismatches",
        files.len(),
        mismatches.len()
    );
    for m in &mismatches {
        eprintln!("  {m}");
    }
    assert!(checked > 0);
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}
