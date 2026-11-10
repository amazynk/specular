//! Frames, geometry and scattering angles against fabio and pyFAI (`gen/oracle.py`).
//!
//! The synthetic fixtures in `tests/data` always run; the public corpus fetched by
//! `tools/corpus/fetch-xtal.sh` into `tools/corpus/xtal` runs when present.

use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_xtal::{Image, sniff, uncompress};

struct Expected {
    kind: String,
    size: (u32, u32, usize),
    stats: Vec<(usize, f64, f64, f64, usize)>,
    geom: Vec<(String, f64)>,
    scatter: Vec<[f64; 5]>,
}

fn expected(txt: &Path) -> Option<Expected> {
    let text = std::fs::read_to_string(txt).ok()?;
    let mut e = Expected {
        kind: String::new(),
        size: (0, 0, 0),
        stats: Vec::new(),
        geom: Vec::new(),
        scatter: Vec::new(),
    };
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let n = |i: usize| f[i].parse::<f64>().unwrap_or(f64::NAN);
        match f[0] {
            "error" => return None,
            "kind" => e.kind = f[1].to_owned(),
            "size" => e.size = (n(1) as u32, n(2) as u32, n(3) as usize),
            "stats" => e
                .stats
                .push((n(1) as usize, n(2), n(3), n(4), n(5) as usize)),
            "geom" => e.geom.push((f[1].to_owned(), n(2))),
            "scatter" => e.scatter.push([n(1), n(2), n(3), n(4), n(5)]),
            _ => {}
        }
    }
    Some(e)
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-9)
}

#[derive(Default)]
struct Tally {
    files: usize,
    pixels: usize,
    value_mismatches: usize,
    geom_mismatches: usize,
    scatter_checked: usize,
    scatter_mismatches: usize,
    problems: Vec<String>,
}

fn check(root: &Path, out: &Path, t: &mut Tally) {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(d) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p == out {
                continue;
            }
            if p.is_dir() {
                dirs.push(p);
            } else {
                files.push(p);
            }
        }
    }
    files.sort();
    for path in files {
        let rel = path.strip_prefix(root).unwrap();
        let base = out.join(rel);
        let Some(e) = expected(&PathBuf::from(format!("{}.txt", base.display()))) else {
            continue;
        };
        t.files += 1;
        let name = rel.display().to_string();
        let start = Instant::now();
        let raw = std::fs::read(&path).unwrap();
        let data = match uncompress(&raw) {
            Ok(Some((d, _))) => d,
            Ok(None) => raw,
            Err(err) => {
                t.problems.push(format!("{name}: uncompress {err}"));
                continue;
            }
        };
        let kind = sniff(&data[..data.len().min(specular_xtal::SNIFF_HEAD)]);
        if kind.map(|k| format!("{k:?}")) != Some(e.kind.clone()) {
            t.problems
                .push(format!("{name}: sniffed {kind:?}, fabio {}", e.kind));
        }
        let img = match Image::parse(&data) {
            Ok(i) => i,
            Err(err) => {
                t.problems.push(format!("{name}: {err}"));
                continue;
            }
        };
        let first = img.frame(&data, 0);
        let open = start.elapsed();
        eprintln!(
            "{name}: {}x{} x{} {} open {:.1} ms",
            img.width,
            img.height,
            img.frames.len(),
            img.kind.name(),
            open.as_secs_f64() * 1e3
        );
        if (img.width, img.height, img.frames.len()) != e.size {
            t.problems.push(format!(
                "{name}: size {}x{}x{} vs {:?}",
                img.width,
                img.height,
                img.frames.len(),
                e.size
            ));
            continue;
        }
        for &(k, ..) in &e.stats {
            let ours = if k == 0 {
                first.as_ref().map_err(|e| e.to_string()).cloned()
            } else {
                img.frame(&data, k).map_err(|e| e.to_string())
            };
            let ours = match ours {
                Ok(v) => v,
                Err(err) => {
                    t.problems.push(format!("{name}[{k}]: {err}"));
                    continue;
                }
            };
            let gz = std::fs::read(format!("{}.{k}.f32.gz", base.display())).unwrap();
            let theirs: Vec<f32> = uncompress(&gz)
                .unwrap()
                .expect("gzipped plane")
                .0
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            let bad = ours
                .iter()
                .zip(&theirs)
                .filter(|(a, b)| !(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())))
                .count()
                + ours.len().abs_diff(theirs.len());
            if bad > 0 {
                let (i, a, b) = ours
                    .iter()
                    .zip(&theirs)
                    .enumerate()
                    .find(|(_, (a, b))| !(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())))
                    .map(|(i, (a, b))| (i, *a, *b))
                    .unwrap_or((0, 0.0, 0.0));
                t.problems.push(format!(
                    "{name}[{k}]: {bad} values differ, first at {i}: {a} vs {b}"
                ));
            }
            t.pixels += theirs.len();
            t.value_mismatches += bad;
        }
        let g = &img.geometry;
        for (key, want) in &e.geom {
            let got = match key.as_str() {
                "wavelength" => g.wavelength,
                "distance" => g.distance,
                "beam_x" => g.beam.map(|b| b.0),
                "beam_y" => g.beam.map(|b| b.1),
                "pixel_x" => g.pixel.map(|p| p.0),
                "pixel_y" => g.pixel.map(|p| p.1),
                "two_theta" => Some(g.two_theta.unwrap_or(0.0)),
                "overload" => g.overload,
                _ => continue,
            };
            if !got.is_some_and(|v| close(v, *want, 1e-6) || (v - want).abs() < 1e-9) {
                t.geom_mismatches += 1;
                t.problems.push(format!("{name}: {key} {got:?} vs {want}"));
            }
        }
        for &[x, y, tth, q, d] in &e.scatter {
            t.scatter_checked += 1;
            let s = g.at(x + 0.5, y + 0.5);
            let ok = s.is_some_and(|s| {
                close(s.two_theta, tth, 1e-6) && close(s.q, q, 1e-6) && close(s.d, d, 1e-6)
            });
            if !ok {
                t.scatter_mismatches += 1;
                t.problems.push(format!(
                    "{name}: at ({x},{y}) {s:?} vs 2θ {tth} q {q} d {d}"
                ));
            }
        }
    }
}

fn run(root: &Path) -> Tally {
    let mut t = Tally::default();
    check(root, &root.join("expected"), &mut t);
    eprintln!(
        "{}: {} files, {} pixels, {} value / {} geometry / {} of {} scatter mismatches",
        root.display(),
        t.files,
        t.pixels,
        t.value_mismatches,
        t.geom_mismatches,
        t.scatter_mismatches,
        t.scatter_checked
    );
    for p in &t.problems {
        eprintln!("  {p}");
    }
    t
}

#[test]
fn fixtures_match_fabio_and_pyfai() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let t = run(&root);
    assert!(t.files >= 12, "only {} fixtures had oracle output", t.files);
    assert!(t.problems.is_empty(), "{} problems", t.problems.len());
}

#[test]
fn corpus_matches_fabio_and_pyfai() {
    let root = std::env::var_os("SPECULAR_XTAL_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/xtal"),
        PathBuf::from,
    );
    if !root.join("expected").is_dir() {
        eprintln!("no corpus; run tools/corpus/fetch-xtal.sh");
        return;
    }
    let t = run(&root);
    assert!(t.problems.is_empty(), "{} problems", t.problems.len());
}
