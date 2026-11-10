//! Our reading of every mzML / mzXML file against pyteomics' (`tests/oracle.py`):
//!
//! ```text
//! python -I crates/formats/specular-mzml/tests/oracle.py tools/corpus/pcb-spectra/mzml EXPECT/corpus
//! SPECULAR_MZML_CORPUS=tools/corpus/pcb-spectra/mzml SPECULAR_MZML_EXPECT=EXPECT/corpus \
//!   cargo test -p specular-mzml --test oracle -- --ignored --nocapture
//! ```
//!
//! Per spectrum and chromatogram: the id, MS level, retention time and both arrays (length
//! and a hash over their values as f64) must agree. Files pyteomics cannot read are counted
//! apart, with whether we open them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use specular_mzml::{Data, Kind, Record, XKind, open};

fn digest(v: Option<&[f64]>) -> String {
    let Some(v) = v else {
        return "-1\t-".into();
    };
    let mut h = Sha256::new();
    for x in v {
        h.update(x.to_le_bytes());
    }
    let d = h.finalize();
    let hex: String = d.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("{}\t{hex}", v.len())
}

/// Files where we differ from pyteomics on purpose, and why.
const EXPLAINED: &[(&str, &str)] = &[
    (
        "InspectOutfile_test_1.mzXML",
        "retentionTime=\"PT-1S\" is not a valid duration: we leave it out, pyteomics reads 0",
    ),
    (
        "tiny2_SRM.mzML0.99.0.mzML",
        "spectrum 1 has two arrays both labelled m/z: we take the first",
    ),
    (
        "tiny2_SRM.mzML0.99.1.mzML",
        "spectrum 1 has two arrays both labelled m/z: we take the first",
    ),
];

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|e| e == "expect") {
            out.push(p);
        }
    }
}

fn rt(v: Option<f64>, want: &str) -> bool {
    match (v, want) {
        (None, "-") => true,
        (Some(a), w) => w
            .parse::<f64>()
            .is_ok_and(|b| (a - b).abs() <= 1e-9 * b.abs().max(1.0)),
        _ => false,
    }
}

/// One line of ours for a spectrum (S) or chromatogram (C), in the oracle's columns.
fn line(kind: char, r: &Record) -> Vec<String> {
    let x = (r.x_kind != XKind::None).then_some(&r.x[..]);
    let y = (!r.y.is_empty() || r.x_kind != XKind::None).then_some(&r.y[..]);
    vec![
        kind.to_string(),
        r.index.to_string(),
        r.id.clone(),
        r.ms_level.map_or("-".into(), |l| l.to_string()),
        String::new(),
        digest(x),
        digest(y),
    ]
}

#[test]
#[ignore]
fn corpus_matches_pyteomics() {
    // Starts the `ORACLE` lines on their own line (libtest prints `test NAME ... ` first).
    println!();
    let (Some(corpus), Some(expect)) = (
        std::env::var_os("SPECULAR_MZML_CORPUS"),
        std::env::var_os("SPECULAR_MZML_EXPECT"),
    ) else {
        eprintln!("SPECULAR_MZML_CORPUS / SPECULAR_MZML_EXPECT not set");
        return;
    };
    let (corpus, expect) = (PathBuf::from(corpus), PathBuf::from(expect));
    let mut files = Vec::new();
    walk(&expect, &mut files);
    let (mut compared, mut records, mut bad_records, mut bad_files) = (0, 0, 0, 0);
    let (mut oracle_failed, mut oracle_failed_we_open, mut empty) = (0, 0, 0);
    let mut explained = 0;
    let mut shown = 0;
    let mut report = |msg: String| {
        if shown < 60 {
            println!("{msg}");
        }
        shown += 1;
    };
    for ex in files {
        let rel = ex.strip_prefix(&expect).expect("under expect");
        let src = corpus.join(rel.with_extension(""));
        let want = std::fs::read_to_string(&ex).expect("expect file");
        let bytes = std::fs::read(&src).expect("corpus file");
        let data: Data = Arc::new(bytes);
        let ours = open(data);
        // One line per file for tools/ci/oracle-pass-rates.sh.
        let fmt = format!(
            "MS/{}",
            src.extension()
                .map(|e| e.to_string_lossy().to_ascii_uppercase())
                .unwrap_or_default()
        );
        let verdict = |status: &str| println!("ORACLE\t{fmt}\t{}\t{status}", rel.display());
        if want.starts_with("E ") {
            oracle_failed += 1;
            if let Ok(f) = &ours {
                oracle_failed_we_open += 1;
                let n = f.spectrum_count();
                let ok = (0..n).filter(|&i| f.spectrum(i).is_ok()).count();
                println!(
                    "pyteomics fails, we read {ok}/{n} spectra: {} ({})",
                    rel.display(),
                    want.lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(90)
                        .collect::<String>()
                );
            }
            verdict("skip");
            continue;
        }
        compared += 1;
        let f = match ours {
            Ok(f) => f,
            // A run without spectra or chromatograms: pyteomics reads nothing, we refuse it.
            Err(specular_mzml::Error::Malformed(m))
                if m.starts_with("no spectra")
                    && want.contains("\nspectra 0\n")
                    && want.contains("\nchromatograms 0\n") =>
            {
                empty += 1;
                verdict("pass");
                continue;
            }
            Err(e) => {
                bad_files += 1;
                report(format!("{}: we do not open it: {e}", rel.display()));
                verdict("fail");
                continue;
            }
        };
        let mut lines = want.lines();
        let kind = lines.next().unwrap_or("");
        let want_kind = if f.kind() == Kind::Mzxml {
            "kind mzxml"
        } else {
            "kind mzml"
        };
        let ns: usize = lines
            .next()
            .and_then(|l| l.strip_prefix("spectra ")?.parse().ok())
            .unwrap_or(0);
        let nc: usize = lines
            .next()
            .and_then(|l| l.strip_prefix("chromatograms ")?.parse().ok())
            .unwrap_or(0);
        if kind != want_kind || ns != f.spectrum_count() || nc != f.chromatogram_count() {
            bad_files += 1;
            report(format!(
                "{}: {kind} {ns} spectra {nc} chromatograms; ours {:?} {} {}",
                rel.display(),
                f.kind(),
                f.spectrum_count(),
                f.chromatogram_count()
            ));
            verdict("fail");
            continue;
        }
        let mut file_bad = false;
        for l in lines {
            let w: Vec<&str> = l.split('\t').collect();
            if w.len() < 9 {
                continue;
            }
            records += 1;
            let i: usize = w[1].parse().unwrap_or(usize::MAX);
            let r = if w[0] == "S" {
                f.spectrum(i)
            } else {
                f.chromatogram(i)
            };
            let r = match r {
                Ok(r) => r,
                Err(e) => {
                    bad_records += 1;
                    file_bad = true;
                    report(format!("{} {} {}: {e}", rel.display(), w[0], w[1]));
                    continue;
                }
            };
            let ours = line(w[0].chars().next().unwrap_or('S'), &r);
            let theirs = format!("{}\t{}", w[5], w[6]);
            let theirs_y = format!("{}\t{}", w[7], w[8]);
            let mut bad = Vec::new();
            if ours[2] != w[2] {
                bad.push(format!("id {} vs {}", ours[2], w[2]));
            }
            if w[0] == "S" && ours[3] != w[3] {
                bad.push(format!("level {} vs {}", ours[3], w[3]));
            }
            if w[0] == "S" && !rt(r.rt_min, w[4]) {
                bad.push(format!("rt {:?} vs {}", r.rt_min, w[4]));
            }
            // An array pyteomics finds missing that we read as empty is the same.
            let same = |a: &str, b: &str| a == b || (b.starts_with("-1") && a.starts_with("0\t"));
            if !same(&ours[5], &theirs) {
                bad.push(format!("x {} vs {}", ours[5], theirs));
            }
            if !same(&ours[6], &theirs_y) {
                bad.push(format!("y {} vs {}", ours[6], theirs_y));
            }
            if !bad.is_empty()
                && let Some((_, why)) = EXPLAINED
                    .iter()
                    .find(|(f, _)| rel.to_string_lossy().ends_with(&format!("{f}.expect")))
            {
                explained += 1;
                println!("explained: {} {} {}: {why}", rel.display(), w[0], w[1]);
            } else if !bad.is_empty() {
                bad_records += 1;
                file_bad = true;
                report(format!(
                    "{} {} {}: {}",
                    rel.display(),
                    w[0],
                    w[1],
                    bad.join("; ")
                ));
            }
        }
        if file_bad {
            bad_files += 1;
        }
        verdict(if file_bad { "fail" } else { "pass" });
    }
    println!(
        "{compared} files compared, {records} spectra and chromatograms: {bad_records} mismatching \
         in {bad_files} files, {explained} explained ({empty} empty runs refused as such); pyteomics could not read \
         {oracle_failed} more ({oracle_failed_we_open} of which we open)"
    );
    assert_eq!(bad_records + bad_files, 0, "mismatches against pyteomics");
}

/// Times opening the file named by `SPECULAR_MZML_BIG`, reading its middle spectrum and
/// building its overview:
///
/// `SPECULAR_MZML_BIG=path/to/big.mzML cargo test -p specular-mzml --test oracle timing --
/// --ignored --nocapture`
#[test]
#[ignore]
fn timing() {
    use specular_spectra::SpectraFile;
    let Some(path) = std::env::var_os("SPECULAR_MZML_BIG") else {
        eprintln!("SPECULAR_MZML_BIG not set");
        return;
    };
    let bytes = std::fs::read(&path).expect("reads");
    let mb = bytes.len() as f64 / 1048576.0;
    let t = std::time::Instant::now();
    let f = open(Arc::new(bytes)).expect("opens");
    let open_ms = t.elapsed().as_secs_f64() * 1e3;
    let mid = f.spectrum_count() / 2;
    let t = std::time::Instant::now();
    let s = f.spectrum(mid).expect("a spectrum");
    let spec_ms = t.elapsed().as_secs_f64() * 1e3;
    let t = std::time::Instant::now();
    let p = f.page(0).expect("overview");
    let tic_ms = t.elapsed().as_secs_f64() * 1e3;
    println!(
        "{mb:.1} MB, {} spectra: open {open_ms:.1} ms, spectrum {mid} ({} points) {spec_ms:.2} ms, \
         overview ({} points) {tic_ms:.1} ms",
        f.spectrum_count(),
        s.x.len(),
        p.traces[0].x.len()
    );
}
