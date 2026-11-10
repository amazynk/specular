//! The downloaded corpus (tools/corpus/fetch-mesh.sh): every model file is read without a
//! panic, and where trimesh read it too (`expected.tsv`) the triangle count and bounds agree.
//! Reads SPECULAR_MESH_CORPUS, else tools/corpus/mesh; skips when neither exists.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_mesh::{Limits, read_path};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_MESH_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/mesh")
        });
    dir.is_dir().then_some(dir)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            ["stl", "obj", "ply", "gltf", "glb", "3mf"].contains(&e.to_ascii_lowercase().as_str())
        }) {
            out.push(p);
        }
    }
}

/// Files trimesh reads wrongly: it ignores sparse accessors, so it misses the displaced
/// vertices (y up to 4) of the Khronos sample made to test them.
const ORACLE_WRONG: &[&str] = &["SimpleSparseAccessor"];

struct Expected {
    triangles: usize,
    min: [f64; 3],
    max: [f64; 3],
}

fn expected(dir: &Path) -> HashMap<String, Expected> {
    let Ok(s) = std::fs::read_to_string(dir.join("expected.tsv")) else {
        return HashMap::new();
    };
    let v3 = |s: &str| {
        let v: Vec<f64> = s.split(' ').map(|x| x.parse().unwrap()).collect();
        [v[0], v[1], v[2]]
    };
    s.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() == 4).then(|| {
                (
                    f[0].to_owned(),
                    Expected {
                        triangles: f[1].parse().unwrap(),
                        min: v3(f[2]),
                        max: v3(f[3]),
                    },
                )
            })
        })
        .collect()
}

#[test]
fn corpus_reads_and_matches_trimesh() {
    let Some(dir) = corpus() else {
        eprintln!("no mesh corpus; run tools/corpus/fetch-mesh.sh");
        return;
    };
    let want = expected(&dir);
    let mut files = Vec::new();
    walk(&dir, &mut files);
    files.sort();
    let (mut read, mut failed, mut compared) = (0, 0, 0);
    let mut mismatches = Vec::new();
    for p in &files {
        let rel = p.strip_prefix(&dir).unwrap().to_string_lossy().into_owned();
        let t = Instant::now();
        let r = read_path(p, &Limits::default());
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let m = match r {
            Ok(m) => m,
            Err(e) => {
                failed += 1;
                eprintln!("  {rel}: {e}");
                if want.contains_key(&rel) {
                    mismatches.push(format!("{rel}: trimesh reads it, we do not ({e})"));
                }
                continue;
            }
        };
        read += 1;
        if ms > 500.0 {
            eprintln!("  {rel}: {} triangles in {ms:.0} ms", m.triangle_count());
        }
        let Some(x) = want.get(&rel) else { continue };
        if ORACLE_WRONG.iter().any(|w| rel.contains(w)) {
            continue;
        }
        compared += 1;
        let size = (0..3)
            .map(|i| x.max[i] - x.min[i])
            .fold(0.0f64, f64::max)
            .max(1e-6);
        let tol = size * 1e-4;
        let off = (0..3).any(|i| {
            (f64::from(m.bounds.min[i]) - x.min[i]).abs() > tol
                || (f64::from(m.bounds.max[i]) - x.max[i]).abs() > tol
        });
        if m.triangle_count() != x.triangles || off {
            mismatches.push(format!(
                "{rel}: {} triangles, bounds {:?}..{:?}; trimesh {} triangles, {:?}..{:?}",
                m.triangle_count(),
                m.bounds.min,
                m.bounds.max,
                x.triangles,
                x.min,
                x.max
            ));
        }
    }
    eprintln!(
        "mesh corpus: {} files, {read} read, {failed} refused, {compared} compared with trimesh, \
         {} mismatches",
        files.len(),
        mismatches.len()
    );
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
