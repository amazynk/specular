//! Public FCS files (fcsparser's and FlowIO's test data; tools/corpus/fetch-oracle4.sh fcs)
//! against FlowIO, dataset by dataset: channel values, scale values and compensated values
//! (FlowIO's scale values times the inverse spillover matrix), which are what the density
//! plots and histograms bin. Per parameter: sum, min, max and six sampled events. Reads
//! `SPECULAR_ORACLE4_CORPUS`/fcs, else `$CARGO_TARGET_DIR/corpus/oracle4/fcs`; skips when
//! absent. Run with `cargo test -p specular-fcs --test oracle -- --ignored --nocapture`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use specular_fcs::{Events, parse};

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE4_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| PathBuf::from(t).join("corpus/oracle4"))
        })?;
    let dir = base.join("fcs");
    dir.join("expect.tsv").is_file().then_some(dir)
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    a == b || (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-3)
}

/// Checks `ev` against one stage's `sum:min:max:v,v,...;...` columns.
fn stage(what: &str, ev: &Events, picks: &[usize], want: &str) -> Result<(), String> {
    let cols: Vec<&str> = want.split(';').collect();
    if cols.len() != ev.params {
        return Err(format!(
            "{what}: {} parameters vs {}",
            ev.params,
            cols.len()
        ));
    }
    for (p, col) in cols.iter().enumerate() {
        let f: Vec<&str> = col.split(':').collect();
        let num = |s: &str| s.parse::<f64>().unwrap();
        let sum: f64 = ev.column(p).map(f64::from).sum();
        let lo = ev.column(p).fold(f32::INFINITY, f32::min) as f64;
        let hi = ev.column(p).fold(f32::NEG_INFINITY, f32::max) as f64;
        // f32 events summed in f64 against FlowIO's f64: relative 1e-6.
        if !close(sum, num(f[0]), 1e-6)
            || !close(lo, num(f[1]), 1e-6)
            || !close(hi, num(f[2]), 1e-6)
        {
            return Err(format!(
                "{what} p{p}: sum/min/max {sum}/{lo}/{hi} vs {}/{}/{}",
                f[0], f[1], f[2]
            ));
        }
        for (&e, v) in picks.iter().zip(f[3].split(',')) {
            let got = f64::from(ev.get(e, p));
            if !close(got, num(v), 1e-6) {
                return Err(format!("{what} p{p} event {e}: {got} vs {v}"));
            }
        }
    }
    Ok(())
}

#[test]
#[ignore]
fn corpus_matches_flowio() {
    let Some(dir) = corpus() else {
        eprintln!("oracle4 fcs corpus missing: run tools/corpus/fetch-oracle4.sh fcs");
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    // file -> verdicts per dataset (None: skipped)
    let mut per: BTreeMap<&str, Vec<Option<Result<(), String>>>> = BTreeMap::new();
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let (name, k): (&str, usize) = (f[0], f[1].parse().unwrap());
        let data = std::fs::read(dir.join("files").join(name)).unwrap();
        let verdict = if f[2] == "error" {
            eprintln!(
                "  {name}#{k}: FlowIO fails ({}); ours parses: {}",
                f[3],
                parse(&data).is_ok()
            );
            None
        } else {
            Some(check(&data, k, &f))
        };
        per.entry(name).or_default().push(verdict);
    }
    let mut failures = Vec::new();
    for (name, v) in &per {
        let errs: Vec<&String> = v
            .iter()
            .flatten()
            .filter_map(|r| r.as_ref().err())
            .collect();
        let status = if !errs.is_empty() {
            "fail"
        } else if v.iter().all(Option::is_none) {
            "skip"
        } else {
            "pass"
        };
        println!("ORACLE\tFCS\t{name}\t{status}");
        failures.extend(errs.into_iter().map(|e| format!("{name}: {e}")));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

fn check(data: &[u8], k: usize, f: &[&str]) -> Result<(), String> {
    let fcs = parse(data).map_err(|e| e.to_string())?;
    let ds = fcs.datasets.get(k).ok_or("dataset missing")?;
    let (n, params): (usize, usize) = (f[2].parse().unwrap(), f[3].parse().unwrap());
    if (ds.events, ds.params.len()) != (n, params) {
        return Err(format!(
            "{} events x {} parameters vs FlowIO {n} x {params}",
            ds.events,
            ds.params.len()
        ));
    }
    let picks: Vec<usize> = f[4].split(' ').filter_map(|s| s.parse().ok()).collect();
    let raw = ds.read_channels(data).map_err(|e| e.to_string())?;
    stage("channel values", &raw, &picks, f[5])?;
    if f[6] != "-" {
        let scaled = ds.read_events(data, false).map_err(|e| e.to_string())?;
        stage("scale values", &scaled, &picks, f[6])?;
    }
    if f[7] != "-" {
        let comp = ds.read_events(data, true).map_err(|e| e.to_string())?;
        if !comp.compensated {
            return Err("not compensated; FlowIO's text has a spillover matrix".into());
        }
        stage("compensated", &comp, &picks, f[7])?;
    }
    Ok(())
}
