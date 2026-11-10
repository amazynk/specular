//! The splat corpus (tools/corpus/fetch-models.sh) against the plyfile/numpy oracle
//! (`gen/splat_oracle.py`): counts, harmonic degrees, bounds and the first centres of every
//! file, and the CPU rasteriser against the oracle's reference renderings. Skipped when the
//! corpus or the oracle's facts are absent.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use specular_splat::{Limits, Splats, preview};

fn root() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_MODELS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../tools/corpus/models"
            ))
        });
    p.join("oracle/splats.tsv").is_file().then_some(p)
}

fn nums(s: &str) -> Vec<f32> {
    s.split(',').map(|v| v.parse().unwrap()).collect()
}

fn read(path: &std::path::Path) -> specular_splat::Result<Splats> {
    let bytes = std::fs::read(path).unwrap();
    let l = Limits::default();
    match path.extension().and_then(|e| e.to_str()) {
        Some("splat") => specular_splat::read_splat(&bytes, &l),
        Some("spz") => specular_splat::read_spz(&bytes, &l),
        _ => specular_splat::read_ply(&bytes, &l),
    }
}

/// Splat `i`'s centre back in the file's frame.
fn file_pos(s: &Splats, i: usize) -> [f32; 3] {
    let p = s.positions[i];
    s.to_file.map(|r| r[0] * p[0] + r[1] * p[1] + r[2] * p[2])
}

#[test]
fn corpus_matches_the_oracle() {
    let Some(root) = root() else {
        eprintln!("no splat corpus or oracle; skipped");
        return;
    };
    let tsv = std::fs::read_to_string(root.join("oracle/splats.tsv")).unwrap();
    let mut facts: HashMap<String, HashMap<String, String>> = HashMap::new();
    for line in tsv.lines() {
        let mut f = line.splitn(3, '\t');
        let (name, key, value) = (f.next().unwrap(), f.next().unwrap(), f.next().unwrap());
        facts
            .entry(name.to_owned())
            .or_default()
            .insert(key.to_owned(), value.to_owned());
    }
    let (mut checked, mut mismatches) = (0, 0);
    let mut names: Vec<_> = facts.keys().cloned().collect();
    names.sort();
    for name in names {
        let f = &facts[&name];
        let path = root.join("splats").join(&name);
        let bytes = std::fs::read(&path).unwrap();
        if f.get("kind").map(String::as_str) == Some("not-splats") {
            assert!(!specular_splat::ply_is_splats(&bytes), "{name}");
            checked += 1;
            continue;
        }
        let t = Instant::now();
        let s = read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let mut bad = Vec::new();
        if s.in_file.to_string() != f["count"] || s.len() as u64 != s.in_file {
            bad.push(format!(
                "count {} of {} vs {}",
                s.len(),
                s.in_file,
                f["count"]
            ));
        }
        if s.sh_degree.to_string() != f["degree"] {
            bad.push(format!("degree {} vs {}", s.sh_degree, f["degree"]));
        }
        let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for i in 0..s.len() {
            let p = file_pos(&s, i);
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        let tol = |v: f32| 1e-4 * v.abs().max(1.0);
        for (got, want, what) in [(lo, nums(&f["min"]), "min"), (hi, nums(&f["max"]), "max")] {
            if (0..3).any(|a| (got[a] - want[a]).abs() > tol(want[a]) * 10.0) {
                bad.push(format!("{what} {got:?} vs {want:?}"));
            }
        }
        for (i, p) in f["first"].split(';').enumerate() {
            let want = nums(p);
            let got = file_pos(&s, i);
            if (0..3).any(|a| (got[a] - want[a]).abs() > tol(want[a])) {
                bad.push(format!("centre {i} {got:?} vs {want:?}"));
            }
        }
        if let Some(cam) = f.get("camera") {
            let parts: Vec<&str> = cam.split(';').collect();
            let v3 = |s: &str| {
                let v = nums(s);
                [v[0], v[1], v[2]]
            };
            let view = preview::View {
                right: v3(parts[0]),
                down: v3(parts[1]),
                forward: v3(parts[2]),
                eye: None,
                target: v3(parts[3]),
                focal: parts[4].parse().unwrap(),
            };
            let (w, h) = (192u32, 144u32);
            let t = Instant::now();
            let ours = preview::render_view(&s, &view, w, h);
            let render_ms = t.elapsed().as_secs_f64() * 1e3;
            let stem = name.trim_end_matches(".ply");
            let theirs = std::fs::read(root.join("oracle").join(format!("{stem}.rgba"))).unwrap();
            // Over premultiplied colour on black, as a viewer shows it.
            let mut se = 0f64;
            for (a, b) in ours.chunks_exact(4).zip(theirs.chunks_exact(4)) {
                for c in 0..3 {
                    let pa = f64::from(a[c]) * f64::from(a[3]) / 255.0;
                    let pb = f64::from(b[c]) * f64::from(b[3]) / 255.0;
                    se += (pa - pb) * (pa - pb);
                }
            }
            let mse = se / f64::from(w * h * 3);
            let psnr = 10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10();
            eprintln!("{name}: rendering {render_ms:.0} ms, {psnr:.1} dB against the reference");
            if let Some(dir) = std::env::var_os("SPECULAR_SPLAT_DUMP") {
                std::fs::write(PathBuf::from(dir).join(format!("{stem}.rgba")), &ours).unwrap();
            }
            if psnr < 35.0 {
                bad.push(format!("rendering {psnr:.1} dB"));
            }
        }
        eprintln!(
            "{name}: {} splats, degree {}, read in {ms:.0} ms{}",
            s.len(),
            s.sh_degree,
            if bad.is_empty() {
                String::new()
            } else {
                format!(": {bad:?}")
            }
        );
        checked += 1;
        if !bad.is_empty() {
            mismatches += 1;
        }
    }
    // The same capture as PLY and as .splat agrees on colour and shape.
    let (a, b) = (
        read(&root.join("splats/CL.ply")).unwrap(),
        read(&root.join("splats/CL.splat")).unwrap(),
    );
    let mut worst = (0f32, 0f32);
    for i in 0..a.len() {
        for c in 0..4 {
            let want = a.colors[i][c].clamp(0.0, 1.0);
            worst.0 = worst.0.max((want - b.colors[i][c]).abs());
        }
        let m = a.covariances[i].iter().fold(0f32, |m, v| m.max(v.abs()));
        for k in 0..6 {
            worst.1 = worst
                .1
                .max((a.covariances[i][k] - b.covariances[i][k]).abs() / m.max(1e-12));
        }
    }
    eprintln!(
        "CL.ply vs CL.splat: colour {:.4}, covariance {:.3}",
        worst.0, worst.1
    );
    assert!(worst.0 < 0.004 && worst.1 < 0.1, "{worst:?}");
    eprintln!("{checked} files checked, {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}
