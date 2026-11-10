//! Compares Gwyddion and Igor planes, axes, units, titles and labels with the oracles
//! (`specular-em/gen/oracle.py`: gwyfile, or its own reader for Gwyddion 1 files, and igor2)
//! on the committed fixtures (`tests/data`) and on the corpus (`$SPECULAR_EM_CORPUS`, else
//! `tools/corpus/em`; skipped when absent), and opens every corpus file.

#[path = "../../specular-em/tests/common/expect.rs"]
mod expect;

use std::path::{Path, PathBuf};
use std::time::Instant;

use expect::{Expect, close, compare};
use specular_spm::{gwy, ibw};

fn corpus() -> Option<PathBuf> {
    let dir = match std::env::var_os("SPECULAR_EM_CORPUS") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/em"),
    };
    dir.join("expected").is_dir().then_some(dir)
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn ext(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

#[derive(Default)]
struct Tally {
    images: usize,
    planes: usize,
    failures: Vec<String>,
}

/// Reads plane `k` into the buffer.
type ReadPlane<'a> = Box<dyn Fn(usize, &mut [f32]) -> Result<bool, specular_spm::Error> + 'a>;

/// The common part: size, planes, axes and value unit.
struct Got<'a> {
    size: (usize, usize, usize),
    axes: Vec<(f64, f64, String)>,
    unit: String,
    read: ReadPlane<'a>,
}

fn check(e: &Expect, g: Got, t: &mut Tally) {
    if g.size != e.size {
        t.failures
            .push(format!("{}: size {:?} vs {:?}", e.name(), g.size, e.size));
        return;
    }
    t.images += 1;
    for (i, &p) in e.planes.iter().enumerate() {
        let mut out = vec![0f32; g.size.0 * g.size.1];
        if let Err(err) = (g.read)(p, &mut out) {
            t.failures.push(format!("{} plane {p}: {err}", e.name()));
            continue;
        }
        let (bad, first) = compare(&out, e.plane(i), e.complex);
        t.planes += 1;
        if bad > 0 {
            t.failures.push(format!(
                "{} plane {p}: {bad} mismatches, first {}",
                e.name(),
                first.unwrap()
            ));
        }
    }
    for (i, (scale, offset, unit)) in e.axes.iter().enumerate() {
        let Some((s, o, u)) = g.axes.get(i) else {
            t.failures.push(format!("{}: no axis {i}", e.name()));
            continue;
        };
        if !close(*s, *scale) || !close(*o, *offset) || u != unit {
            t.failures.push(format!(
                "{} axis {i}: {s} {o} {u:?} vs {scale} {offset} {unit:?}",
                e.name()
            ));
        }
    }
    if let Some(want) = &e.unit
        && *want != g.unit
    {
        t.failures
            .push(format!("{}: unit {:?} vs {want:?}", e.name(), g.unit));
    }
}

fn check_gwy(e: &Expect, file: &[u8], t: &mut Tally) {
    let g = match gwy::parse(file) {
        Ok(g) => g,
        Err(err) => {
            t.failures.push(format!("{}: {err}", e.name()));
            return;
        }
    };
    let Some(c) = g.channels.get(e.k) else {
        t.failures
            .push(format!("{}: only {} channels", e.name(), g.channels.len()));
        return;
    };
    // Gwyddion ignores a unit's SI prefix (values are in base units); so do we.
    let base = |u: &str| gwy::base_unit(u);
    let want_axes: Vec<_> = e.axes.iter().map(|(s, o, u)| (*s, *o, base(u))).collect();
    let e2 = Expect {
        rel: e.rel.clone(),
        k: e.k,
        size: e.size,
        planes: e.planes.clone(),
        axes: want_axes,
        unit: e.unit.as_deref().map(base),
        complex: e.complex,
        values: e.values.clone(),
        ..Expect::default()
    };
    if e.title != c.title {
        t.failures.push(format!(
            "{}: title {:?} vs {:?}",
            e.name(),
            c.title,
            e.title
        ));
    }
    check(
        &e2,
        Got {
            size: (c.xres, c.yres, 1),
            axes: vec![
                (c.xreal / c.xres as f64, c.xoff, c.unit_xy.clone()),
                (c.yreal / c.yres as f64, c.yoff, c.unit_xy.clone()),
            ],
            unit: c.unit_z.clone(),
            read: Box::new(|_, out| c.read_plane(file, out)),
        },
        t,
    );
}

fn check_ibw(e: &Expect, file: &[u8], t: &mut Tally) {
    let w = match ibw::parse(file) {
        Ok(w) => w,
        Err(err) => {
            t.failures.push(format!("{}: {err}", e.name()));
            return;
        }
    };
    for (j, name) in &e.labels {
        let got = w.dims.get(2).and_then(|d| d.labels.get(*j));
        if got != Some(name) {
            t.failures
                .push(format!("{}: label {j} {got:?} vs {name:?}", e.name()));
        }
    }
    check(
        e,
        Got {
            size: (w.width(), w.height(), w.planes()),
            axes: w
                .dims
                .iter()
                .map(|d| (d.delta, d.offset, d.unit.clone()))
                .collect(),
            unit: w.data_unit.clone(),
            read: Box::new(|p, out| w.read_plane(file, p, out)),
        },
        t,
    );
}

fn run(root: &Path, label: &str) -> Tally {
    // Starts the `ORACLE` lines on their own line (libtest prints `test NAME ... ` first).
    println!();
    let mut t = Tally::default();
    let mut cache: Option<(String, Vec<u8>)> = None;
    let mut verdicts: std::collections::BTreeMap<String, bool> = Default::default();
    for e in expect::all(root) {
        let x = ext(Path::new(&e.rel));
        if !matches!(x.as_str(), "gwy" | "ibw") {
            continue;
        }
        if cache.as_ref().is_none_or(|(r, _)| *r != e.rel) {
            cache = Some((e.rel.clone(), std::fs::read(root.join(&e.rel)).unwrap()));
        }
        let file = &cache.as_ref().unwrap().1;
        let before = t.failures.len();
        let rel = e.rel.clone();
        if e.error.is_some() {
            // The oracle refuses text and empty waves; so must we.
            if x == "ibw" && ibw::parse(file).is_ok() {
                t.failures.push(format!(
                    "{}: opened, the oracle says {:?}",
                    e.name(),
                    e.error
                ));
            }
            *verdicts.entry(rel).or_default() |= t.failures.len() > before;
            continue;
        }
        if x == "gwy" {
            check_gwy(&e, file, &mut t);
        } else {
            check_ibw(&e, file, &mut t);
        }
        *verdicts.entry(rel).or_default() |= t.failures.len() > before;
    }
    // One line per file for tools/ci/oracle-pass-rates.sh (a file fails when any of its
    // images does).
    for (rel, failed) in &verdicts {
        let fmt = if label == "fixtures" {
            "FIXTURE".to_string()
        } else {
            ext(Path::new(rel)).to_ascii_uppercase()
        };
        let status = if *failed { "fail" } else { "pass" };
        println!("ORACLE\tSPM/{fmt}\t{rel}\t{status}");
    }
    eprintln!(
        "{label}: {} images, {} planes compared, {} failures",
        t.images,
        t.planes,
        t.failures.len()
    );
    for f in &t.failures {
        eprintln!("  {f}");
    }
    t
}

#[test]
fn fixtures_match_oracle() {
    let t = run(&fixtures(), "fixtures");
    assert!(t.images >= 7);
    assert!(t.failures.is_empty());
}

#[test]
fn fixture_metadata() {
    let f = std::fs::read(fixtures().join("height.ibw")).unwrap();
    let w = ibw::parse(&f).unwrap();
    let note = w.note_pairs();
    assert!(note.iter().any(|(k, _)| k == "ScanSize"), "{note:?}");
    let f = std::fs::read(fixtures().join("channels.gwy")).unwrap();
    let g = gwy::parse(&f).unwrap();
    assert_eq!(g.channels.len(), 2);
    let f = std::fs::read(fixtures().join("old.gwy")).unwrap();
    let g = gwy::parse(&f).unwrap();
    assert!(g.old);
    assert_eq!(g.channels[0].unit_z, "m");
}

#[test]
fn corpus_matches_oracle() {
    let Some(root) = corpus() else {
        eprintln!("EM corpus not found; skipping");
        return;
    };
    let t = run(&root, "corpus");
    assert!(t.images >= 10);
    assert!(t.failures.is_empty());
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "expected") {
                files(&p, out);
            }
        } else if matches!(ext(&p).as_str(), "gwy" | "ibw") {
            out.push(p);
        }
    }
}

/// Every corpus file opens and its last plane reads, except the text and empty waves the
/// oracle refuses too.
#[test]
fn corpus_opens() {
    let Some(root) = corpus() else {
        eprintln!("EM corpus not found; skipping");
        return;
    };
    let mut all = Vec::new();
    files(&root, &mut all);
    let mut bad = Vec::new();
    let mut refused = 0;
    for p in &all {
        let f = std::fs::read(p).unwrap();
        let r = if ext(p) == "gwy" {
            gwy::parse(&f).map(|g| {
                for c in &g.channels {
                    let mut out = vec![0f32; c.xres * c.yres];
                    c.read_plane(&f, &mut out).unwrap();
                }
            })
        } else {
            ibw::parse(&f).map(|w| {
                let mut out = vec![0f32; w.width() * w.height()];
                w.read_plane(&f, w.planes() - 1, &mut out).unwrap();
            })
        };
        let rel = p.strip_prefix(&root).unwrap();
        if let Err(e) = r {
            let txt = root
                .join("expected")
                .join(format!("{}.0.txt", rel.display()));
            let oracle_refuses = std::fs::read_to_string(txt).is_ok_and(|s| s.starts_with("error"));
            if oracle_refuses {
                refused += 1;
            } else {
                bad.push(format!("{}: {e}", rel.display()));
            }
        }
    }
    eprintln!(
        "{} files, {refused} refused as the oracle does, {} refused",
        all.len(),
        bad.len()
    );
    for b in &bad {
        eprintln!("  {b}");
    }
    assert!(all.len() > 10);
    assert!(bad.is_empty());
}

/// Header plus first plane of the largest file of each format.
#[test]
#[ignore = "timing; run with --ignored --release"]
fn open_times() {
    let Some(root) = corpus() else {
        return;
    };
    let mut all = Vec::new();
    files(&root, &mut all);
    for want in ["gwy", "ibw"] {
        let Some(p) = all
            .iter()
            .filter(|p| ext(p) == want)
            .max_by_key(|p| std::fs::metadata(p).unwrap().len())
        else {
            continue;
        };
        let f = std::fs::read(p).unwrap();
        let mut best = f64::INFINITY;
        for _ in 0..5 {
            let t = Instant::now();
            if want == "gwy" {
                let g = gwy::parse(&f).unwrap();
                let c = &g.channels[0];
                let mut out = vec![0f32; c.xres * c.yres];
                c.read_plane(&f, &mut out).unwrap();
            } else {
                let w = ibw::parse(&f).unwrap();
                let mut out = vec![0f32; w.width() * w.height()];
                w.read_plane(&f, 0, &mut out).unwrap();
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        eprintln!(
            "{}: {:.1} MB, header + first plane {:.2} ms",
            p.strip_prefix(&root).unwrap().display(),
            f.len() as f64 / 1e6,
            best * 1e3
        );
    }
}
