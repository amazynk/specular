//! The astropy oracle on the dev corpus (`tools/corpus/fetch-sci.sh`): every image plane in
//! `fits/expected.tsv` must match astropy's values. Reads `$SPECULAR_SCI_CORPUS/fits`, else
//! `tools/corpus/sci/fits`; skips when absent.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn corpus_dir() -> Option<PathBuf> {
    let dir = match std::env::var_os("SPECULAR_SCI_CORPUS") {
        Some(d) => PathBuf::from(d).join("fits"),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/sci/fits"),
    };
    dir.join("expected.tsv").is_file().then_some(dir)
}

fn num(s: &str) -> f64 {
    if s == "nan" {
        f64::NAN
    } else {
        s.parse().unwrap()
    }
}

fn close(a: f64, b: f64, rel: f64, scale: f64) -> bool {
    (a.is_nan() && b.is_nan()) || (a - b).abs() <= rel * (a.abs().max(b.abs()).max(scale))
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else {
            let n = p.to_string_lossy().to_ascii_lowercase();
            if [".fits", ".fit", ".fts", ".fz"]
                .iter()
                .any(|x| n.ends_with(x))
            {
                out.push(p);
            }
        }
    }
}

#[test]
fn astropy_oracle() {
    let Some(dir) = corpus_dir() else {
        eprintln!("FITS corpus not found; skipping");
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expected.tsv")).unwrap();
    let mut cache: HashMap<String, (Vec<u8>, specular_fits::Fits)> = HashMap::new();
    let (mut planes, mut pixels, mut zscales) = (0, 0, 0);
    let mut bad = Vec::new();
    for line in tsv.lines().skip(1).filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let [name, hdu, plane, w, h, finite, sum, min, max, zlo, zhi, pts] = f[..] else {
            panic!("bad line {line:?}");
        };
        let (data, fits) = cache.entry(name.to_owned()).or_insert_with(|| {
            let data = std::fs::read(dir.join(name)).unwrap();
            let fits = specular_fits::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
            (data, fits)
        });
        let id = format!("{name}[{hdu}] plane {plane}");
        let Some(hdu) = fits.hdus.get(hdu.parse::<usize>().unwrap()) else {
            bad.push(format!("{id}: HDU missing"));
            continue;
        };
        let p = match specular_fits::read_plane(data, hdu, plane.parse().unwrap()) {
            Ok(p) => p,
            Err(e) => {
                bad.push(format!("{id}: {e}"));
                continue;
            }
        };
        if (p.width, p.height) != (w.parse().unwrap(), h.parse().unwrap()) {
            bad.push(format!(
                "{id}: size {}x{}, astropy {w}x{h}",
                p.width, p.height
            ));
            continue;
        }
        let fin: Vec<f64> = p
            .values
            .iter()
            .filter(|v| v.is_finite())
            .map(|&v| f64::from(v))
            .collect();
        let (lo, hi) = (num(min), num(max));
        let range = (hi - lo).abs();
        if fin.len() != finite.parse::<usize>().unwrap() {
            bad.push(format!("{id}: {} finite, astropy {finite}", fin.len()));
        }
        let s: f64 = fin.iter().sum();
        if !close(s, num(sum), 1e-5, range * fin.len() as f64 * 1e-3) {
            bad.push(format!("{id}: sum {s}, astropy {sum}"));
        }
        let mn = fin.iter().copied().fold(f64::INFINITY, f64::min);
        let mx = fin.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if !fin.is_empty() && (!close(mn, lo, 1e-6, 1.0) || !close(mx, hi, 1e-6, 1.0)) {
            bad.push(format!("{id}: range {mn}..{mx}, astropy {min}..{max}"));
        }
        for pt in pts.split(' ') {
            let (xy, v) = pt.split_once('=').unwrap();
            let (x, y) = xy.split_once(',').unwrap();
            let (x, y): (usize, usize) = (x.parse().unwrap(), y.parse().unwrap());
            let got = f64::from(p.values[y * p.width + x]);
            if !close(got, num(v), 1e-6, 1.0) {
                bad.push(format!("{id}: pixel {x},{y} = {got}, astropy {v}"));
            }
            pixels += 1;
        }
        if zlo != "-" {
            let z = specular_core::nd::zscale(&p.values, p.width).unwrap();
            let tol = 1e-4 * range.max(1e-30);
            if (f64::from(z.0) - num(zlo)).abs() > tol || (f64::from(z.1) - num(zhi)).abs() > tol {
                bad.push(format!("{id}: zscale {z:?}, astropy {zlo} {zhi}"));
            }
            zscales += 1;
        }
        planes += 1;
    }
    eprintln!(
        "fits oracle: {planes} planes, {pixels} pixels, {zscales} zscale limits checked; {} mismatches",
        bad.len()
    );
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// Every file of the corpus parses (or fails cleanly) and every image plane reads.
#[test]
fn every_file_reads() {
    let Some(dir) = corpus_dir() else {
        return;
    };
    let mut all = Vec::new();
    files(&dir, &mut all);
    all.sort();
    let (mut ok, mut failed) = (0, 0);
    for path in all {
        let data = std::fs::read(&path).unwrap();
        let Ok(fits) = specular_fits::parse(&data) else {
            failed += 1;
            continue;
        };
        for hdu in fits.images() {
            let img = hdu.image.as_ref().unwrap();
            for k in 0..img.planes().min(4) {
                match specular_fits::read_plane(&data, hdu, k) {
                    Ok(_) => ok += 1,
                    Err(e) => {
                        eprintln!("{} [{}] plane {k}: {e}", path.display(), hdu.index);
                        failed += 1;
                    }
                }
            }
        }
    }
    eprintln!("corpus: {ok} planes read, {failed} files or planes failed cleanly");
}

/// Open-to-first-pixels of a 4096 × 4096 float32 file (`big_f32_4096.fits`).
#[test]
fn speed_4k_float() {
    let Some(dir) = corpus_dir() else {
        return;
    };
    let path = dir.join("generated/big_f32_4096.fits");
    if !path.is_file() {
        return;
    }
    let mut best = f64::INFINITY;
    let mut best_z = f64::INFINITY;
    for _ in 0..5 {
        let t = Instant::now();
        let data = std::fs::read(&path).unwrap();
        let fits = specular_fits::parse(&data).unwrap();
        let p = specular_fits::read_plane(&data, &fits.hdus[0], 0).unwrap();
        best = best.min(t.elapsed().as_secs_f64() * 1e3);
        let t = Instant::now();
        let z = specular_core::nd::zscale(&p.values, p.width);
        best_z = best_z.min(t.elapsed().as_secs_f64() * 1e3);
        assert!(z.is_some());
    }
    eprintln!("4096x4096 f32 FITS: read + parse + plane {best:.1} ms, zscale {best_z:.1} ms");
}
