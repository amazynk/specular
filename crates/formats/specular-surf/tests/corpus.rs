//! Compares the readers' first page with the oracles (`gen/oracle.py`: RosettaSciIO for
//! Digital Surf, h5py for Zygo `.datx`, surfalize for the other formats, a numpy reader for
//! X3P files surfalize refuses) on the committed fixtures (`tests/data`) and on the corpus
//! (`$SPECULAR_MATSCI_CORPUS`, else `tools/corpus/matsci`, fetched by
//! `tools/corpus/fetch-matsci.sh`; skipped when absent). Every file under `surface/` is
//! detected from its content, read, and its planes, NaN positions and pixel pitch checked.

use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_em::dataset::Dataset;
use specular_surf::{Error, detect_file, read};

fn corpus() -> Option<PathBuf> {
    let dir = match std::env::var_os("SPECULAR_MATSCI_CORPUS") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/matsci"),
    };
    dir.join("expected/surface").is_dir().then_some(dir)
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

/// An oracle's expectation for one file.
#[derive(Default)]
struct Expect {
    tool: String,
    size: (usize, usize, usize),
    step: (f64, f64),
    zunit: String,
    tail: usize,
    rescaled: bool,
    spectral: bool,
    error: Option<String>,
    planes: Vec<f32>,
}

fn expect(txt: &Path) -> Expect {
    let mut e = Expect::default();
    for line in std::fs::read_to_string(txt).unwrap().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "tool" => e.tool = f[1].into(),
            "size" => {
                e.size = (
                    f[1].parse().unwrap(),
                    f[2].parse().unwrap(),
                    f[3].parse().unwrap(),
                )
            }
            "step" => e.step = (f[1].parse().unwrap(), f[2].parse().unwrap()),
            "zunit" => e.zunit = f[1].into(),
            "tail" => e.tail = f[1].parse().unwrap(),
            "spectral" => e.spectral = true,
            "rescaled" => e.rescaled = true,
            "error" => e.error = Some(f[1..].join(" ")),
            _ => {}
        }
    }
    if let Ok(b) = std::fs::read(txt.with_extension("f32")) {
        e.planes = b
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
    }
    e
}

fn metres(unit: &str) -> f64 {
    match unit {
        "mm" => 1e-3,
        "um" | "\u{b5}m" => 1e-6,
        "nm" => 1e-9,
        "pm" => 1e-12,
        _ => 1.0,
    }
}

#[derive(Default)]
struct Tally {
    files: usize,
    planes: usize,
    values: usize,
    refused: usize,
    oracle_errors: usize,
    rescaled: usize,
    failures: Vec<String>,
}

/// Compares page 0 of `d` with `e`, returning the first difference.
fn compare(name: &str, d: &Dataset, file: &[u8], e: &Expect, t: &mut Tally) -> Option<String> {
    let p = d.pages.first()?;
    let (w, h, np) = e.size;
    if (p.width, p.height) != (w, h) || p.planes.len() < np {
        return Some(format!(
            "size {}×{}×{} vs {w}×{h}×{np}",
            p.width,
            p.height,
            p.planes.len()
        ));
    }
    for (axis, want) in [(&p.x, e.step.0), (&p.y, e.step.1)] {
        let got = axis.as_ref().map_or(0.0, |l| l.step);
        if want != 0.0 && ((got - want) / want).abs() > 1e-5 {
            return Some(format!("pitch {got} vs {want}"));
        }
    }
    let n = w * h;
    for k in 0..np {
        let (mut got, _) = p.read_plane(file, k)?;
        let factor = if p.planes[k].unit == "m" {
            metres(&e.zunit)
        } else {
            1.0
        };
        if p.rgb && k < 3 {
            got.iter_mut().for_each(|v| *v *= 255.0);
        }
        let want: Vec<f64> = e.planes[k * n..(k + 1) * n]
            .iter()
            .map(|&v| f64::from(v) * factor)
            .collect();
        // RosettaSciIO rounds the colour and quality layers of Digital Surf files (computing
        // in its own precision, so values at a half may round either way).
        if e.tool == "rsciio" && p.planes[k].unit != "m" && np > 1 {
            for (g, &x) in got.iter_mut().zip(&want) {
                let tie = (g.fract().abs() - 0.5).abs() < 1e-3;
                *g = if tie && (f64::from(*g) - x).abs() <= 0.5 + 1e-3 {
                    x as f32
                } else {
                    g.round()
                };
            }
        }
        let (lo, hi) = want
            .iter()
            .filter(|v| v.is_finite())
            .fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        let mag = lo.abs().max(hi.abs());
        let tol = (hi - lo).max(0.0) * 1e-5 + mag * 2e-6 + 1e-30;
        for i in 0..n - e.tail {
            let (g, x) = (f64::from(got[i]), want[i]);
            if g.is_nan() != x.is_nan() || (!x.is_nan() && (g - x).abs() > tol) {
                return Some(format!(
                    "{name} plane {k} at ({}, {}): {g} vs {x} (tolerance {tol})",
                    i % w,
                    i / w
                ));
            }
        }
        t.planes += 1;
        t.values += n - e.tail;
    }
    None
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn run(root: &Path) -> Tally {
    let mut t = Tally::default();
    let mut files = Vec::new();
    walk(&root.join("surface"), &mut files);
    for path in files {
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let txt = root.join("expected").join(format!("{rel}.txt"));
        if !txt.is_file() {
            continue;
        }
        let e = expect(&txt);
        let file = std::fs::read(&path).unwrap();
        t.files += 1;
        t.rescaled += usize::from(e.rescaled);
        let start = Instant::now();
        let Some(format) = detect_file(&file) else {
            t.failures.push(format!("{rel}: not detected"));
            continue;
        };
        let got = read(format, &file);
        let ms = start.elapsed().as_secs_f64() * 1e3;
        if e.spectral {
            match got {
                Err(Error::Unsupported(_)) => t.refused += 1,
                other => t
                    .failures
                    .push(format!("{rel}: spectra not refused: {other:?}")),
            }
            continue;
        }
        if let Some(err) = &e.error {
            t.oracle_errors += 1;
            eprintln!(
                "{rel}: oracle {} failed ({err}); read {}",
                e.tool,
                got.is_ok()
            );
            continue;
        }
        match got {
            Ok(d) => {
                if let Some(diff) = compare(&rel, &d, &file, &e, &mut t) {
                    t.failures.push(format!("{rel} ({}): {diff}", e.tool));
                } else {
                    eprintln!("{rel}: {format:?} matches {} in {ms:.1} ms", e.tool);
                }
            }
            Err(err) => t.failures.push(format!("{rel}: {err}")),
        }
    }
    eprintln!(
        "{}: {} files, {} planes, {} values, {} spectra refused, {} oracle errors, {} oracle \
         planes rescaled, {} mismatches",
        root.display(),
        t.files,
        t.planes,
        t.values,
        t.refused,
        t.oracle_errors,
        t.rescaled,
        t.failures.len()
    );
    for f in &t.failures {
        eprintln!("  {f}");
    }
    t
}

#[test]
fn fixtures_match_oracles() {
    let t = run(&fixtures());
    assert!(t.files >= 7, "fixtures missing");
    assert!(t.failures.is_empty(), "{} mismatches", t.failures.len());
}

#[test]
fn corpus_matches_oracles() {
    let Some(root) = corpus() else {
        eprintln!("surface corpus absent; skipped");
        return;
    };
    let t = run(&root);
    assert!(t.failures.is_empty(), "{} mismatches", t.failures.len());
}
