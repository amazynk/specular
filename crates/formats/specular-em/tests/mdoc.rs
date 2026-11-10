//! SerialEM `.mdoc` files against the `mdocfile` package's reading of them
//! (expected-mdoc.tsv, written by gen/make_mdoc.py): the hand-made tilt series in
//! tests/data/mdoc, and the public examples tools/corpus/fetch-mdoc.sh fetches (read from
//! SPECULAR_MDOC_CORPUS, else tools/corpus/mdoc; skipped when absent).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specular_em::mdoc::{self, Mdoc, SectionKind};

/// Checks every row of `dir/expected-mdoc.tsv`; returns (files, checks).
fn check(dir: &Path) -> (usize, usize) {
    let text = std::fs::read_to_string(dir.join("expected-mdoc.tsv")).unwrap();
    let mut docs: BTreeMap<String, Mdoc> = BTreeMap::new();
    let mut checks = 0;
    let same = |got: &str, want: &str, what: &str| {
        let words = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
        match (got.parse::<f64>(), want.parse::<f64>()) {
            (Ok(a), Ok(b)) => assert!(
                (a - b).abs() <= 1e-9 * b.abs().max(1.0),
                "{what}: {a} vs {b}"
            ),
            _ => assert_eq!(words(got), words(want), "{what}"),
        }
    };
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let n = |i: usize| f[i].parse::<usize>().unwrap();
        match f[0] {
            "file" => {
                let t = std::fs::read_to_string(dir.join(f[1])).unwrap();
                let m = mdoc::parse(&t).unwrap_or_else(|| panic!("{}: not read", f[1]));
                assert_eq!(m.sections.len(), n(2), "{}: sections", f[1]);
                assert_eq!(m.titles.len(), n(3), "{}: titles", f[1]);
                docs.insert(f[1].to_owned(), m);
            }
            "global" => {
                let got = docs[f[1]].get(f[2]).unwrap_or_default();
                same(got, f[3], &format!("{} {}", f[1], f[2]));
            }
            "section" => {
                let s = &docs[f[1]].sections[n(2)];
                let kind = match s.kind {
                    SectionKind::ZValue => "ZValue",
                    SectionKind::MontSection => "MontSection",
                    SectionKind::FrameSet => "FrameSet",
                };
                assert_eq!((kind, s.index), (f[3], f[4].parse().unwrap()), "{line}");
            }
            "field" => {
                let s = &docs[f[1]].sections[n(2)];
                same(s.get(f[3]).unwrap_or_default(), f[4], line);
            }
            t => panic!("unknown row {t}"),
        }
        checks += 1;
    }
    (docs.len(), checks)
}

#[test]
fn mdoc_samples_match_mdocfile() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/mdoc");
    let (files, checks) = check(&dir);
    eprintln!("mdoc samples: {files} files, {checks} checks, 0 mismatches");
    let m = mdoc::read_beside(&dir.join("tilt.mrc")).unwrap();
    let tilts = m.tilts(5).unwrap();
    assert_eq!(tilts.len(), 5);
    assert!((tilts[3] + 5.99988).abs() < 1e-9);
}

#[test]
fn mdoc_corpus_matches_mdocfile() {
    let dir = std::env::var_os("SPECULAR_MDOC_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/mdoc")
        });
    if !dir.join("expected-mdoc.tsv").is_file() {
        eprintln!("mdoc corpus absent: run tools/corpus/fetch-mdoc.sh");
        return;
    }
    let (files, checks) = check(&dir);
    eprintln!("mdoc corpus: {files} files, {checks} checks, 0 mismatches");
    assert!(files > 0);
}
