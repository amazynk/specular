//! Timings of the composite built from the layers ([`Psd::compose_layers`]) over the layered
//! RGB and grey files of `$SPECULAR_FIDELITY_CORPUS/psd` (best of three runs each), to watch
//! for regressions. Ignored: run with `--ignored --nocapture`; prints one line per file and
//! the total.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use specular_psd::*;

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            // The per-type adjustment copies repeat their sources.
            if p.file_name().is_some_and(|n| n != "solo") {
                walk(&p, out);
            }
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("psd") || e.eq_ignore_ascii_case("psb"))
        {
            out.push(p);
        }
    }
}

#[test]
#[ignore = "timings: needs the fidelity corpus"]
fn composite_timings() {
    let Some(root) = std::env::var_os("SPECULAR_FIDELITY_CORPUS").map(PathBuf::from) else {
        println!("SPECULAR_FIDELITY_CORPUS unset: skipped");
        return;
    };
    let root = root.join("psd");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    let mut total = Duration::ZERO;
    let mut count = 0;
    for f in &files {
        let Ok(data) = std::fs::read(f) else { continue };
        let Ok(psd) = parse(&data) else { continue };
        let info = psd.info();
        if psd.layers().is_empty()
            || !matches!(info.mode, ColorMode::Rgb | ColorMode::Grayscale)
            || info.depth == 32
        {
            continue;
        }
        let best = (0..3)
            .map(|_| {
                let t = Instant::now();
                let r = psd.compose_layers(&data, &Options::default());
                std::hint::black_box(r.is_ok());
                t.elapsed()
            })
            .min()
            .unwrap_or_default();
        total += best;
        count += 1;
        let rel = f.strip_prefix(&root).unwrap_or(f).display();
        println!("TIME\t{rel}\t{:.2}", best.as_secs_f64() * 1e3);
    }
    println!(
        "TIME-TOTAL\t{count} files\t{:.1} ms",
        total.as_secs_f64() * 1e3
    );
}
