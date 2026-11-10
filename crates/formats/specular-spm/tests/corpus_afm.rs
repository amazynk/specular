//! Compares the Nanoscope, Nanonis SXM, JPK and NT-MDT readers' pages, planes, labels, units
//! and lateral calibrations with the oracles (`gen/oracle_afm.py`: pySPM, nanonispy,
//! tifffile with AFMReader's calibration, and its own NT-MDT reader) on the committed
//! fixtures (`tests/data/afm`, from `gen/gen_afm.py`) and on the corpus
//! (`$SPECULAR_MATSCI_CORPUS/afm`, else `tools/corpus/matsci/afm`; skipped when absent).

use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_em::dataset::{Dataset, Lateral};
use specular_spm::{jpk, mdt, nanoscope, sxm};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/afm")
}

fn corpus() -> Option<PathBuf> {
    let dir = match std::env::var_os("SPECULAR_MATSCI_CORPUS") {
        Some(d) => PathBuf::from(d).join("afm"),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/matsci/afm"),
    };
    dir.join("expected").is_dir().then_some(dir)
}

type Read = fn(&[u8]) -> specular_spm::Result<Dataset>;

/// The format and reader for a file name, as the oracle picks them.
fn format(p: &Path) -> Option<(&'static str, Read)> {
    let n = p.file_name()?.to_string_lossy().to_ascii_lowercase();
    let ext = n.rsplit('.').next()?;
    Some(match ext {
        "sxm" => ("sxm", sxm::read),
        "jpk" | "jpk-qi-image" => ("jpk", jpk::read),
        "mdt" => ("mdt", mdt::read),
        "spm" => ("nanoscope", nanoscope::read),
        e if e.len() == 3 && e.bytes().all(|b| b.is_ascii_digit()) => {
            ("nanoscope", nanoscope::read)
        }
        _ => return None,
    })
}

fn reader(p: &Path) -> Option<Read> {
    format(p).map(|(_, r)| r)
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
        } else if reader(&p).is_some() {
            out.push(p);
        }
    }
    out.sort();
}

/// One expected page.
#[derive(Debug, Default)]
struct Want {
    size: (usize, usize, usize),
    x: Option<(f64, f64, String)>,
    y: Option<(f64, f64, String)>,
    planes: Vec<(String, String)>,
}

fn parse(text: &str) -> Want {
    let mut w = Want::default();
    for line in text.lines() {
        let (key, rest) = line.split_once(' ').unwrap_or((line, ""));
        match key {
            "size" => {
                let v: Vec<usize> = rest.split(' ').map(|s| s.parse().unwrap()).collect();
                w.size = (v[0], v[1], v[2]);
            }
            "x" | "y" => {
                let ax = (rest != "none").then(|| {
                    let mut t = rest.splitn(3, ' ');
                    let o = t.next().unwrap().parse().unwrap();
                    let s = t.next().unwrap().parse().unwrap();
                    (o, s, t.next().unwrap_or("").to_string())
                });
                if key == "x" {
                    w.x = ax;
                } else {
                    w.y = ax;
                }
            }
            "plane" => {
                let (_, rest) = rest.split_once(' ').unwrap();
                let (unit, label) = rest.split_once('\t').unwrap();
                w.planes.push((label.to_string(), unit.to_string()));
            }
            _ => panic!("unknown expected line {line:?}"),
        }
    }
    w
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1e-30)
}

fn lateral_ok(got: Option<&Lateral>, want: &Option<(f64, f64, String)>) -> bool {
    match (got, want) {
        (None, None) => true,
        (Some(l), Some((o, s, u))) => {
            close(l.step, *s) && (l.offset - o).abs() <= 1e-6 * s.abs() && l.unit == *u
        }
        _ => false,
    }
}

/// Values within 1e-5 of the plane's largest magnitude (f32 output of integer or f64 maths);
/// NaN only where expected.
fn compare(got: &[f32], want: &[f32]) -> (usize, Option<String>) {
    let max = want
        .iter()
        .filter(|v| v.is_finite())
        .fold(0f32, |m, v| m.max(v.abs()));
    let tol = (max * 1e-5).max(f32::MIN_POSITIVE);
    let mut bad = 0;
    let mut first = None;
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        let ok = if w.is_nan() {
            g.is_nan()
        } else {
            (g - w).abs() <= tol
        };
        if !ok {
            bad += 1;
            first.get_or_insert_with(|| format!("#{i}: {g} vs {w}"));
        }
    }
    (bad, first)
}

#[derive(Default)]
struct Tally {
    files: usize,
    pages: usize,
    planes: usize,
    failures: Vec<String>,
}

fn check_file(root: &Path, p: &Path, t: &mut Tally) {
    let rel = p
        .strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let expected = root.join("expected");
    let mut wants = Vec::new();
    while let Ok(s) = std::fs::read_to_string(expected.join(format!("{rel}.{}.txt", wants.len()))) {
        wants.push(parse(&s));
    }
    if wants.is_empty() {
        return;
    }
    t.files += 1;
    let file = std::fs::read(p).unwrap();
    let d = match reader(p).unwrap()(&file) {
        Ok(d) => d,
        Err(e) => {
            t.failures.push(format!("{rel}: {e}"));
            return;
        }
    };
    if d.pages.len() != wants.len() {
        t.failures
            .push(format!("{rel}: {} pages vs {}", d.pages.len(), wants.len()));
    }
    for (k, (page, want)) in d.pages.iter().zip(&wants).enumerate() {
        let size = (page.width, page.height, page.planes.len());
        if size != want.size {
            t.failures
                .push(format!("{rel}.{k}: size {size:?} vs {:?}", want.size));
            continue;
        }
        t.pages += 1;
        if !lateral_ok(page.x.as_ref(), &want.x) || !lateral_ok(page.y.as_ref(), &want.y) {
            t.failures.push(format!(
                "{rel}.{k}: lateral {:?} {:?} vs {:?} {:?}",
                page.x, page.y, want.x, want.y
            ));
        }
        for (i, (plane, (label, unit))) in page.planes.iter().zip(&want.planes).enumerate() {
            if plane.label != *label || plane.unit != *unit {
                t.failures.push(format!(
                    "{rel}.{k}.{i}: {:?} [{}] vs {label:?} [{unit}]",
                    plane.label, plane.unit
                ));
            }
            let raw = std::fs::read(expected.join(format!("{rel}.{k}.{i}.f32"))).unwrap();
            let w: Vec<f32> = raw
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let (v, missing) = page.read_plane(&file, i).unwrap();
            if missing {
                t.failures.push(format!("{rel}.{k}.{i}: samples missing"));
            }
            let (bad, first) = compare(&v, &w);
            t.planes += 1;
            if bad > 0 {
                t.failures.push(format!(
                    "{rel}.{k}.{i}: {bad} mismatches, first {}",
                    first.unwrap()
                ));
            }
        }
    }
}

fn run(root: &Path, label: &str) -> Tally {
    // Starts the `ORACLE` lines on their own line (libtest prints `test NAME ... ` first).
    println!();
    let mut all = Vec::new();
    files(root, &mut all);
    let mut t = Tally::default();
    for p in &all {
        let before = t.failures.len();
        check_file(root, p, &mut t);
        // One line per file for tools/ci/oracle-pass-rates.sh.
        let fmt = if label == "fixtures" {
            "FIXTURE".to_string()
        } else {
            let x = p
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_uppercase())
                .unwrap_or_default();
            // Bruker Nanoscope files end in a number (.000, .002, ...).
            if x.bytes().all(|b| b.is_ascii_digit()) {
                "NANOSCOPE".to_string()
            } else {
                x
            }
        };
        let rel = p.strip_prefix(root).unwrap_or(p).display();
        let status = if t.failures.len() > before {
            "fail"
        } else {
            "pass"
        };
        println!("ORACLE\tAFM/{fmt}\t{rel}\t{status}");
    }
    eprintln!(
        "{label}: {} files, {} pages, {} planes compared, {} failures",
        t.files,
        t.pages,
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
    assert_eq!(t.files, 5);
    assert!(t.planes >= 12);
    assert!(t.failures.is_empty());
}

#[test]
fn fixture_details() {
    let f = std::fs::read(fixtures().join("scan.sxm")).unwrap();
    let d = sxm::read(&f).unwrap();
    let p = &d.pages[0];
    assert!(p.levelling);
    assert_eq!(p.date.as_deref(), Some("06.06.2019 15:12:03"));
    assert_eq!(p.title.as_deref(), Some("synthetic fixture"));
    let f = std::fs::read(fixtures().join("height.spm")).unwrap();
    assert!(nanoscope::probe(&f));
    let d = nanoscope::read(&f).unwrap();
    assert_eq!(d.pages.len(), 2);
    assert_eq!(d.pages[0].mode.as_deref(), Some("Tapping"));
    assert_eq!(d.pages[0].instrument.as_deref(), Some("MultiMode"));
    let f = std::fs::read(fixtures().join("image.jpk")).unwrap();
    assert!(jpk::probe(&f) && jpk::probe_file(&f));
    let d = jpk::read(&f).unwrap();
    assert!(matches!(
        d.pages[0].planes[1].data,
        specular_em::dataset::PlaneData::Values(_)
    ));
    assert_eq!(d.pages[0].mode.as_deref(), Some("contact"));
    let f = std::fs::read(fixtures().join("frames.mdt")).unwrap();
    assert!(mdt::probe(&f));
    let d = mdt::read(&f).unwrap();
    assert_eq!(d.pages[0].date.as_deref(), Some("2002-06-27 17:03:59"));
    assert_eq!(d.pages[1].title.as_deref(), Some("Height MDA"));
    // Every truncation of every fixture is refused or read without panicking.
    for name in [
        "height.spm",
        "old.001",
        "scan.sxm",
        "image.jpk",
        "frames.mdt",
    ] {
        let f = std::fs::read(fixtures().join(name)).unwrap();
        let read = reader(Path::new(name)).unwrap();
        for n in (0..f.len()).step_by(7) {
            if let Ok(d) = read(&f[..n]) {
                for p in &d.pages {
                    for k in 0..p.planes.len() {
                        p.read_plane(&f[..n], k);
                    }
                }
            }
        }
    }
}

#[test]
fn corpus_matches_oracle() {
    let Some(root) = corpus() else {
        eprintln!("AFM corpus not found; skipping");
        return;
    };
    let t = run(&root, "corpus");
    assert!(t.files >= 5);
    assert!(t.failures.is_empty());
}

/// Every corpus file opens and every plane reads in full.
#[test]
fn corpus_opens() {
    let Some(root) = corpus() else {
        eprintln!("AFM corpus not found; skipping");
        return;
    };
    let mut all = Vec::new();
    files(&root, &mut all);
    let mut bad = Vec::new();
    for p in &all {
        let f = std::fs::read(p).unwrap();
        match reader(p).unwrap()(&f) {
            Ok(d) => {
                for (k, page) in d.pages.iter().enumerate() {
                    for i in 0..page.planes.len() {
                        if page.read_plane(&f, i).is_none_or(|(_, missing)| missing) {
                            bad.push(format!("{}.{k}.{i}: short", p.display()));
                        }
                    }
                }
            }
            Err(e) => bad.push(format!("{}: {e}", p.display())),
        }
    }
    eprintln!("{} files, {} refused or short", all.len(), bad.len());
    for b in &bad {
        eprintln!("  {b}");
    }
    assert!(bad.is_empty());
}

/// Header plus first plane of the largest corpus file of each format.
#[test]
#[ignore = "timing; run with --ignored --release"]
fn open_times() {
    let Some(root) = corpus() else {
        return;
    };
    let mut all = Vec::new();
    files(&root, &mut all);
    all.sort_by_key(|p| std::cmp::Reverse(std::fs::metadata(p).unwrap().len()));
    let mut seen = Vec::new();
    for p in &all {
        let (name, read) = format(p).unwrap();
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let f = std::fs::read(p).unwrap();
        let mut best = f64::INFINITY;
        for _ in 0..5 {
            let t = Instant::now();
            let d = read(&f).unwrap();
            d.pages[0].read_plane(&f, 0).unwrap();
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
