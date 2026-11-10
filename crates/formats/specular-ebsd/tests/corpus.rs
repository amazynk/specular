//! Compares every corpus map with the orix oracle (`gen/oracle.py`, run by
//! `tools/corpus/fetch-matsci.sh oracle_ebsd`) and times opening it. The corpus is
//! `$SPECULAR_MATSCI_CORPUS`, else `tools/corpus/matsci`; skipped when absent.

#[path = "common/oracle.rs"]
mod oracle;

use std::path::{Path, PathBuf};
use std::time::Instant;

fn corpus() -> Option<PathBuf> {
    let dir = match std::env::var_os("SPECULAR_MATSCI_CORPUS") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/matsci"),
    };
    dir.join("expected/ebsd").is_dir().then_some(dir)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else if p
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ang") || e.eq_ignore_ascii_case("ctf"))
        {
            out.push(p);
        }
    }
}

#[test]
fn corpus_against_orix() {
    let Some(root) = corpus() else {
        eprintln!("no EBSD corpus (tools/corpus/fetch-matsci.sh src_ebsd oracle_ebsd); skipped");
        return;
    };
    let src = root.join("ebsd");
    let mut files = Vec::new();
    walk(&src, &mut files);
    let mut total = oracle::Tally::default();
    let mut failures = Vec::new();
    for path in &files {
        let rel = path.strip_prefix(&src).unwrap();
        let file = std::fs::read(path).unwrap();
        let t0 = Instant::now();
        let d = match specular_ebsd::read(&file) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("{}: {e}", rel.display()));
                continue;
            }
        };
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let p = &d.pages[0];
        let rec = match oracle::load(&root.join("expected/ebsd").join(rel)) {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "{}: {}×{} in {ms:.0} ms; orix: {e}",
                    rel.display(),
                    p.width,
                    p.height
                );
                continue;
            }
        };
        let t = oracle::compare(&d, &rec);
        eprintln!(
            "{}: {}×{} ({} MB) in {ms:.0} ms; {} points, {} mismatches {t:?}",
            rel.display(),
            p.width,
            p.height,
            file.len() >> 20,
            t.points,
            t.mismatches()
        );
        total.add(&t);
    }
    eprintln!(
        "EBSD corpus: {} files, {} points, {} mismatches (indexed {}, placed {}, \
         orientation {}, laue {}, folded {}, colour {}; max colour diff {:.4}); not compared: {} \
         sharing a pixel, {} with CI < 0; mean |ours − \
         orix's own key| {:.3}",
        files.len(),
        total.points,
        total.mismatches(),
        total.indexed,
        total.placed,
        total.orientation,
        total.laue,
        total.folded,
        total.colour,
        total.colour_max,
        total.shared,
        total.ci_negative,
        total.orix_key_sum / total.orix_key_n.max(1) as f64
    );
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(total.mismatches(), 0);
}
