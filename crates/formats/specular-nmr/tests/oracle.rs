//! specular-nmr against nmrglue (tests/oracle.py): the FID as read, the processed `1r`
//! and `2rr` with their ppm axes, the magnitude spectrum of our processing, and our
//! automatically phased spectrum against the vendor's `1r`.
//!
//! ```sh
//! python -I crates/formats/specular-nmr/tests/oracle.py tools/corpus/pcb-spectra/nmr \
//!     tools/corpus/pcb-spectra/nmr-expect
//! SPECULAR_NMR_CORPUS=tools/corpus/pcb-spectra/nmr \
//! SPECULAR_NMR_EXPECT=tools/corpus/pcb-spectra/nmr-expect \
//!     cargo test -p specular-nmr --test oracle -- --ignored --nocapture
//! ```
//! The same with `crates/formats/specular-nmr/tests/data` as the corpus covers the fixtures.

use std::path::{Path, PathBuf};

use serde_json::Value;
use specular_nmr::process;

fn f64s(path: &Path) -> Option<Vec<f64>> {
    let b = std::fs::read(path).ok()?;
    Some(
        b.chunks_exact(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect(),
    )
}

fn experiments(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    if root.join("acqus").is_file() && specular_nmr::is_experiment_dir(root) {
        out.push(root.to_path_buf());
    }
    for d in dirs {
        if d.file_name().is_some_and(|n| n != "pdata") {
            experiments(&d, out);
        }
    }
}

/// Local maxima of `y` over 5 % of its maximum that are the largest within ±`w` points, the
/// `n` highest.
fn top_peaks(y: &[f64], n: usize, w: usize) -> Vec<usize> {
    let top = y.iter().cloned().fold(0.0, f64::max);
    let mut p: Vec<usize> = (w..y.len().saturating_sub(w))
        .filter(|&i| y[i] > 0.05 * top && (i - w..=i + w).all(|j| y[j] <= y[i]))
        .collect();
    p.sort_by(|&a, &b| y[b].total_cmp(&y[a]));
    p.truncate(n);
    p
}

fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len()) as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut s, mut sa, mut sb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        s += (x - ma) * (y - mb);
        sa += (x - ma) * (x - ma);
        sb += (y - mb) * (y - mb);
    }
    s / (sa * sb).sqrt()
}

#[derive(Default)]
struct Tally {
    checks: usize,
    mismatches: Vec<String>,
}

impl Tally {
    fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        self.checks += 1;
        if !ok {
            self.mismatches.push(what());
        }
    }
}

#[test]
#[ignore]
fn corpus_matches_nmrglue() {
    // Starts the `ORACLE` lines on their own line (libtest prints `test NAME ... ` first).
    println!();
    let (Some(root), Some(expect)) = (
        std::env::var_os("SPECULAR_NMR_CORPUS"),
        std::env::var_os("SPECULAR_NMR_EXPECT"),
    ) else {
        eprintln!("SPECULAR_NMR_CORPUS / SPECULAR_NMR_EXPECT not set");
        return;
    };
    let (root, expect) = (PathBuf::from(root), PathBuf::from(expect));
    let mut exps = Vec::new();
    experiments(&root, &mut exps);
    let mut t = Tally::default();
    let mut phased = Vec::new();
    for exp in &exps {
        let rel = exp.strip_prefix(&root).unwrap().to_path_buf();
        let name = rel.display().to_string();
        let ex = expect.join(&rel);
        // One line per experiment for tools/ci/oracle-pass-rates.sh.
        let before = t.mismatches.len();
        let Ok(meta) = std::fs::read_to_string(ex.join("meta.json")) else {
            t.check(false, || format!("{name}: no oracle output"));
            println!("ORACLE\tNMR/Bruker\t{name}\tfail");
            continue;
        };
        let meta: Value = serde_json::from_str(&meta).unwrap();
        let t0 = std::time::Instant::now();
        let opened = specular_nmr::open(exp);
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let spectra = match opened {
            Ok(s) => s,
            Err(e) => {
                t.check(false, || format!("{name}: open failed: {e}"));
                println!("ORACLE\tNMR/Bruker\t{name}\tfail");
                continue;
            }
        };
        println!(
            "{name}: {} pages, opened and processed in {ms:.0} ms",
            spectra.pages.len()
        );
        // The FID and the magnitude of our processing.
        if let Some(m) = meta.get("fid") {
            let (acqus, fid) = specular_nmr::fid(exp).unwrap();
            let want = f64s(&ex.join("fid.f64")).unwrap();
            let got: Vec<f64> = fid.iter().flat_map(|c| [c.re, c.im]).collect();
            t.check(got == want, || format!("{name}: FID differs"));
            let procs = specular_nmr::first_procs(exp);
            let s = specular_nmr::settings(&acqus, procs.as_ref()).unwrap();
            let gd = m["group_delay"].as_f64().unwrap();
            t.check(
                s.size as u64 == m["size"].as_u64().unwrap()
                    && s.lb_hz == m["lb"].as_f64().unwrap()
                    && (s.group_delay - gd).abs() < 1e-12,
                || format!("{name}: settings {s:?} vs {m}"),
            );
            let spec = process::spectrum(&fid, &s);
            let mag = f64s(&ex.join("mag.f64")).unwrap();
            let top = mag.iter().cloned().fold(0.0, f64::max);
            let worst = spec
                .iter()
                .zip(&mag)
                .map(|(a, b)| (a.abs() - b).abs())
                .fold(0.0, f64::max);
            t.check(spec.len() == mag.len() && worst <= 1e-9 * top, || {
                format!(
                    "{name}: magnitude differs by {:.3e} of its peak",
                    worst / top
                )
            });
        }
        // Processed spectra as stored.
        let procnos = meta["procnos"].as_object().unwrap();
        let mut page = 0;
        for (n, p) in procnos {
            let file = p["file"].as_str().unwrap();
            let want = f64s(&ex.join(format!("{n}.{file}.f64"))).unwrap();
            let ppm = &p["ppm"];
            let Some(plot) = spectra.pages.get(page) else {
                t.check(false, || format!("{name}: no page for procno {n}"));
                continue;
            };
            page += 1;
            // nmrglue's axis comes from the acquisition (SFO1 − SF, SW_h), ours from TopSpin's
            // stored OFFSET and SW_p: they agree to a small fraction of a point.
            let near =
                |a: f64, b: &Value, step: f64| (a - b.as_f64().unwrap()).abs() <= 0.01 * step;
            if file == "1r" {
                let tr = &plot.traces[0];
                let step = (tr.x[0] - tr.x[1]).abs();
                t.check(tr.y == want, || format!("{name}: 1r of procno {n} differs"));
                t.check(
                    near(tr.x[0], &ppm[0][0], step)
                        && near(*tr.x.last().unwrap(), &ppm[0][1], step),
                    || {
                        format!(
                            "{name}: 1r ppm {}..{} vs {ppm}",
                            tr.x[0],
                            tr.x.last().unwrap()
                        )
                    },
                );
                // Our automatic phase against the vendor's: the processed page.
                if let Some(ours) = spectra
                    .pages
                    .iter()
                    .find(|p| p.title == "Spectrum from the FID")
                    && ours.traces[0].y.len() == tr.y.len()
                {
                    let c = correlation(&ours.traces[0].y, &tr.y);
                    let step = (tr.x[0] - tr.x[1]).abs();
                    let mine = &ours.traces[0];
                    let missed = top_peaks(&tr.y, 5, 8)
                        .into_iter()
                        .filter(|&i| {
                            !top_peaks(&mine.y, 40, 8)
                                .iter()
                                .any(|&j| (mine.x[j] - tr.x[i]).abs() <= 1.01 * step)
                        })
                        .count();
                    t.check(c > 0.99 && missed == 0, || {
                        format!("{name}: automatic phase off the vendor's (correlation {c:.4}, {missed} peaks)")
                    });
                    phased.push(format!(
                        "{name}: correlation with the vendor 1r {c:.5}, {missed} of its 5 highest peaks not within a point; {}",
                        ours.notes.last().unwrap()
                    ));
                }
            } else {
                let g = plot.grid.as_ref().unwrap();
                let shape = &p["shape"];
                let (h, w) = (
                    shape[0].as_u64().unwrap() as usize,
                    shape[1].as_u64().unwrap() as usize,
                );
                let same = g.width == w
                    && g.height == h
                    && g.z.iter().zip(&want).all(|(a, b)| *a == *b as f32);
                t.check(same, || format!("{name}: 2rr of procno {n} differs"));
                let sy = (g.y_range.0 - g.y_range.1).abs() / (h - 1) as f64;
                let sx = (g.x_range.0 - g.x_range.1).abs() / (w - 1) as f64;
                t.check(
                    near(g.y_range.0, &ppm[0][0], sy)
                        && near(g.y_range.1, &ppm[0][1], sy)
                        && near(g.x_range.0, &ppm[1][0], sx)
                        && near(g.x_range.1, &ppm[1][1], sx),
                    || format!("{name}: 2rr ppm {:?} {:?} vs {ppm}", g.x_range, g.y_range),
                );
            }
        }
        let status = if t.mismatches.len() > before {
            "fail"
        } else {
            "pass"
        };
        println!("ORACLE\tNMR/Bruker\t{name}\t{status}");
    }
    for p in &phased {
        println!("{p}");
    }
    println!(
        "{} experiments, {} checks, {} mismatches",
        exps.len(),
        t.checks,
        t.mismatches.len()
    );
    for m in &t.mismatches {
        println!("MISMATCH {m}");
    }
    assert!(t.mismatches.is_empty());
}
