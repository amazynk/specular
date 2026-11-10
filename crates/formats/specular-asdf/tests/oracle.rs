//! The fixtures in tests/data (written by gen/make_fixtures.py) against asdf-python's reading
//! of them (tests/data/expected.tsv, written by gen/oracle.py): file versions and library,
//! every ndarray's datatype, byte order, shape and unit, arrays refused on both sides, the
//! sum and strided values of up to three planes per array, and the plain scalars of the tree.
//!
//! The fetched corpus (tools/corpus/fetch-asdf.sh: the asdf-standard reference files of
//! every standard version and Roman simulated products) is checked the same way when present,
//! at `SPECULAR_ASDF_DIR` with `SPECULAR_ASDF_ORACLE` (default: tools/corpus/asdf and its
//! expected.tsv).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_asdf::{ArraySource, Asdf, NdArray, Node, Value, external_data};

#[derive(Default)]
struct Case {
    file: Option<Vec<String>>,
    arr: Vec<Vec<String>>,
    skip: Vec<String>,
    /// path, lead, sum, finite count
    plane: Vec<(String, String, f64, usize)>,
    /// path, lead, index, value
    val: Vec<(String, String, usize, f64)>,
    /// path, kind, value
    tree: Vec<(String, String, String)>,
    error: Option<String>,
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
        let f: Vec<&str> = line.split('\t').collect();
        let c = cases.entry(f[1].to_owned()).or_default();
        let s = |i: usize| f.get(i).copied().unwrap_or("").to_owned();
        match f[0] {
            "file" => c.file = Some(f[2..].iter().map(|s| (*s).to_owned()).collect()),
            "arr" => c.arr.push(f[2..].iter().map(|s| (*s).to_owned()).collect()),
            "skip" => c.skip.push(s(2)),
            "plane" => c.plane.push((s(2), s(3), num(f[4]), f[5].parse().unwrap())),
            "val" => c.val.push((s(2), s(3), f[4].parse().unwrap(), num(f[5]))),
            "tree" => c.tree.push((s(2), s(3), s(4))),
            "error" => c.error = Some(s(2)),
            t => panic!("unknown row {t}"),
        }
    }
    cases
}

/// Equal as f32 (NaN equals NaN).
fn same(a: f32, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b as f32 || (f64::from(a) - b).abs() <= 1e-6 * b.abs()
}

/// A tree path (`a.b[2].c`) followed through mappings and sequences.
fn at<'v>(tree: &'v Value, path: &str) -> Option<&'v Value> {
    let mut v = tree;
    for part in path.split('.') {
        let (key, mut rest) = match part.find('[') {
            Some(p) => (&part[..p], &part[p..]),
            None => (part, ""),
        };
        if !key.is_empty() {
            v = v.get(key)?;
        }
        while let Some(r) = rest.strip_prefix('[') {
            let end = r.find(']')?;
            let i: usize = r[..end].parse().ok()?;
            v = v.as_seq()?.get(i)?;
            rest = &r[end + 1..];
        }
    }
    Some(v)
}

fn escape(s: &str) -> String {
    s.replace('\t', " ")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn tree_matches(v: &Value, kind: &str, want: &str) -> bool {
    match (kind, &v.node) {
        ("null", Node::Null) => true,
        ("bool", Node::Bool(b)) => b.to_string() == want,
        ("int", Node::Int(i)) => i.to_string() == want,
        ("float", Node::Float(f)) => {
            let w = num(want);
            (f.is_nan() && w.is_nan()) || *f == w
        }
        ("float", Node::Int(i)) => *i as f64 == num(want),
        ("str", Node::Str(s)) => escape(s) == want,
        _ => false,
    }
}

#[derive(Default)]
struct Tally {
    files: usize,
    arrays: usize,
    planes: usize,
    values: usize,
    tree: usize,
    refused_both: usize,
    errors_both: usize,
    /// asdf-python refused; we read it.
    errors_python_only: Vec<String>,
    mismatches: Vec<String>,
}

fn dtype_name(a: &NdArray) -> &'static str {
    match a.dtype.name() {
        "bool8" => "bool",
        n => n,
    }
}

/// An external source's data: the named file beside `name`.
fn external(dir: &Path, name: &str, file: &str) -> specular_asdf::Result<Vec<u8>> {
    let base = Path::new(name).parent().unwrap_or(Path::new(""));
    let ext = std::fs::read(dir.join(base).join(file))
        .map_err(|e| specular_asdf::Error::Malformed(e.to_string()))?;
    external_data(&ext).map(|c| c.to_vec())
}

/// Checks every case of `tsv`; prints one `ORACLE` line per file under format `fmt` (for
/// tools/ci/oracle-pass-rates.sh).
fn check(dir: &Path, tsv: &Path, fmt: &str) -> Tally {
    // Starts the `ORACLE` lines on their own line (libtest prints `test NAME ... ` first).
    println!();
    let cases = load(tsv);
    let mut t = Tally::default();
    for (name, case) in &cases {
        let data = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let verdict = |status: &str| println!("ORACLE\t{fmt}\t{name}\t{status}");
        let before = t.mismatches.len();
        let mut bad = |what: String| t.mismatches.push(format!("{name}: {what}"));
        let asdf = match Asdf::open(&data) {
            Ok(a) => a,
            Err(e) => {
                if case.error.is_some() {
                    t.errors_both += 1;
                    verdict("pass");
                } else {
                    bad(format!("open: {e}"));
                    verdict("fail");
                }
                continue;
            }
        };
        if let Some(err) = &case.error {
            t.errors_python_only.push(format!("{name}: {err}"));
            verdict("skip");
            continue;
        }
        t.files += 1;
        let file = case.file.as_ref().expect("file row");
        if asdf.version != file[0] {
            bad(format!("version {} vs {}", asdf.version, file[0]));
        }
        if let Some(std) = &asdf.standard
            && *std != file[1]
        {
            bad(format!("standard {std} vs {}", file[1]));
        }
        if let Some((n, v)) = asdf.library()
            && (n != file[2] || v != file[3])
        {
            bad(format!("library {n} {v} vs {} {}", file[2], file[3]));
        }
        // Arrays: the same set, the same facts.
        let mine: BTreeMap<&str, &NdArray> =
            asdf.arrays.iter().map(|a| (a.path.as_str(), a)).collect();
        for path in mine.keys() {
            if !case.arr.iter().any(|r| r[0] == *path) {
                bad(format!("array {path} not in asdf-python's tree"));
            }
        }
        for r in &case.arr {
            let Some(a) = mine.get(r[0].as_str()) else {
                bad(format!("array {} missing", r[0]));
                continue;
            };
            t.arrays += 1;
            if dtype_name(a) != r[1] {
                bad(format!("{} dtype {} vs {}", r[0], dtype_name(a), r[1]));
            }
            let order = if a.big_endian { "big" } else { "little" };
            if a.dtype.size() > 1 && order != r[2] {
                bad(format!("{} byteorder {order} vs {}", r[0], r[2]));
            }
            let shape = a
                .shape
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            if shape != r[3] {
                bad(format!("{} shape {shape} vs {}", r[0], r[3]));
            }
            let unit = a.unit.clone().unwrap_or_default();
            if !r[4].is_empty() && unit.replace(' ', "") != r[4].replace(' ', "") {
                bad(format!("{} unit {unit:?} vs {:?}", r[0], r[4]));
            }
        }
        for path in &case.skip {
            if mine.contains_key(path.as_str()) {
                bad(format!(
                    "{path} read, asdf-python's is {path} of string/structured data"
                ));
            } else {
                t.refused_both += 1;
            }
        }
        // Planes and values.
        let mut planes: BTreeMap<(String, String), Vec<f32>> = BTreeMap::new();
        let mut plane_of =
            |path: &str, lead: &str, bad: &mut dyn FnMut(String)| -> Option<Vec<f32>> {
                let key = (path.to_owned(), lead.to_owned());
                if let Some(p) = planes.get(&key) {
                    return Some(p.clone());
                }
                let a = mine.get(path)?;
                let lead: Vec<u64> = if lead == "-" {
                    Vec::new()
                } else {
                    lead.split(',').map(|x| x.parse().unwrap()).collect()
                };
                match asdf.plane(&data, a, &lead, &mut |f| external(dir, name, f)) {
                    Ok(p) => {
                        planes.insert(key, p.clone());
                        Some(p)
                    }
                    Err(e) => {
                        bad(format!("{path} plane {lead:?}: {e}"));
                        None
                    }
                }
            };
        for (path, lead, sum, finite) in &case.plane {
            let Some(p) = plane_of(path, lead, &mut bad) else {
                continue;
            };
            t.planes += 1;
            let fin: Vec<f64> = p
                .iter()
                .filter(|v| v.is_finite())
                .map(|&v| f64::from(v))
                .collect();
            let s: f64 = fin.iter().sum();
            let abs: f64 = fin.iter().map(|v| v.abs()).sum();
            if fin.len() != *finite || (s - sum).abs() > 1e-6 * abs.max(1.0) {
                bad(format!(
                    "{path} [{lead}] sum {s} ({} finite) vs {sum} ({finite})",
                    fin.len()
                ));
            }
        }
        for (path, lead, i, v) in &case.val {
            let Some(p) = plane_of(path, lead, &mut bad) else {
                continue;
            };
            t.values += 1;
            match p.get(*i) {
                Some(&x) if same(x, *v) => {}
                got => bad(format!("{path} [{lead}] value {i}: {got:?} vs {v}")),
            }
        }
        for (path, kind, want) in &case.tree {
            t.tree += 1;
            match at(&asdf.tree, path) {
                Some(v) if tree_matches(v, kind, want) => {}
                got => bad(format!(
                    "tree {path}: {:?} vs {kind} {want}",
                    got.map(|v| &v.node)
                )),
            }
        }
        verdict(if t.mismatches.len() > before {
            "fail"
        } else {
            "pass"
        });
    }
    t
}

fn report(label: &str, t: &Tally) {
    eprintln!(
        "{label}: {} files, {} arrays, {} planes, {} values, {} tree values; {} refused by both, \
         {} unreadable by both, {} mismatches",
        t.files,
        t.arrays,
        t.planes,
        t.values,
        t.tree,
        t.refused_both,
        t.errors_both,
        t.mismatches.len()
    );
    for e in &t.errors_python_only {
        eprintln!("  asdf-python refused, we read: {e}");
    }
    for m in t.mismatches.iter().take(60) {
        eprintln!("  MISMATCH {m}");
    }
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

#[test]
fn fixtures_match_asdf_python() {
    let dir = data_dir();
    let t = check(&dir, &dir.join("expected.tsv"), "ASDF/fixture");
    report("fixtures", &t);
    assert!(
        t.files >= 15 && t.arrays >= 80 && t.tree >= 100,
        "too little checked"
    );
    assert!(t.mismatches.is_empty());
}

fn corpus() -> Option<(PathBuf, PathBuf)> {
    let dir = std::env::var_os("SPECULAR_ASDF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/asdf")
        });
    let tsv = std::env::var_os("SPECULAR_ASDF_ORACLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.join("expected.tsv"));
    (dir.is_dir() && tsv.is_file()).then_some((dir, tsv))
}

/// The asdf-standard reference files (conformance) and the Roman products.
#[test]
fn corpus_matches_asdf_python() {
    let Some((dir, tsv)) = corpus() else {
        eprintln!("no ASDF corpus (tools/corpus/fetch-asdf.sh); skipped");
        return;
    };
    let t = check(&dir, &tsv, "ASDF");
    report("corpus", &t);
    assert!(t.mismatches.is_empty());
}

/// Every fixture's blocks pass their checksums.
#[test]
fn fixture_checksums() {
    let mut checked = 0;
    for e in std::fs::read_dir(data_dir()).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|x| x != "asdf") {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        let a = Asdf::open(&data).unwrap();
        for i in 0..a.blocks.len() {
            if a.blocks[i].streamed() {
                continue;
            }
            assert_eq!(
                a.verify_checksum(&data, i).unwrap(),
                Some(true),
                "{} block {i}",
                p.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 20);
}

/// Open and first plane of the largest corpus file: `cargo test -p specular-asdf --test
/// oracle corpus_timings -- --ignored --nocapture`.
#[test]
#[ignore = "timing on the fetched corpus"]
fn corpus_timings() {
    let Some((dir, _)) = corpus() else { return };
    let mut files: Vec<(u64, PathBuf)> = Vec::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "asdf") {
                files.push((std::fs::metadata(&p).unwrap().len(), p));
            }
        }
    }
    files.sort();
    for (size, p) in files.iter().rev().take(3) {
        let data = std::fs::read(p).unwrap();
        let t0 = Instant::now();
        let a = Asdf::open(&data).unwrap();
        let open = t0.elapsed();
        let pick = a
            .arrays
            .iter()
            .find(|x| x.name() == "data")
            .or_else(|| a.arrays.iter().max_by_key(|x| x.len()))
            .unwrap();
        let t1 = Instant::now();
        let bytes = a.array_data(&data, pick).unwrap();
        let s = pick.squeezed();
        let lead = vec![0; s.shape.len().saturating_sub(2)];
        let plane = s.plane(&bytes, &lead).unwrap();
        let first = t1.elapsed();
        eprintln!(
            "{} ({:.1} MB): open {:.1} ms ({} arrays, {} blocks); first plane of {} {:?} {} ({} values, block {}) {:.1} ms",
            p.file_name().unwrap().to_string_lossy(),
            *size as f64 / 1e6,
            open.as_secs_f64() * 1e3,
            a.arrays.len(),
            a.blocks.len(),
            pick.path,
            pick.shape,
            pick.dtype.name(),
            plane.len(),
            match &pick.source {
                ArraySource::Block(i) => a.blocks[*i]
                    .compression_name()
                    .unwrap_or_else(|| "raw".into()),
                _ => "-".into(),
            },
            first.as_secs_f64() * 1e3
        );
    }
}
