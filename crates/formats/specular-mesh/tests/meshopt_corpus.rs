//! Meshopt-compressed glTF models (tools/corpus/gen-meshopt.py) against their uncompressed
//! twins, built from meshoptimizer's own decoding of every view: the same triangles,
//! positions, normals, colours and UVs. Reads SPECULAR_MESHOPT_CORPUS, else
//! tools/corpus/meshopt; skips when absent.

use std::path::{Path, PathBuf};

use specular_mesh::{Limits, read_path};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_MESHOPT_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/meshopt")
        });
    dir.join("models").is_dir().then_some(dir)
}

#[test]
fn compressed_models_match_their_twins() {
    let Some(dir) = corpus() else {
        eprintln!("no meshopt corpus; run tools/corpus/gen-meshopt.py");
        return;
    };
    let mut models: Vec<_> = std::fs::read_dir(dir.join("models"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.ends_with(".glb") && !n.ends_with(".plain.glb")
        })
        .collect();
    models.sort();
    let mut checked = 0;
    for p in &models {
        let twin = p.with_extension("plain.glb");
        let t0 = std::time::Instant::now();
        let a = read_path(p, &Limits::default()).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let b = read_path(&twin, &Limits::default()).unwrap();
        let name = p.file_name().unwrap().to_string_lossy();
        assert!(a.indices == b.indices, "{name}: indices differ");
        assert!(a.positions == b.positions, "{name}: positions differ");
        // Octahedral normals are specified to one unit in the last place (1/127 for 8 bits).
        assert_eq!(a.normals.len(), b.normals.len(), "{name}: normals");
        let off = a
            .normals
            .iter()
            .zip(&b.normals)
            .filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| (p - q).abs() > 1e-6))
            .count();
        let worst = a
            .normals
            .iter()
            .zip(&b.normals)
            .flat_map(|(x, y)| x.iter().zip(y.iter()).map(|(p, q)| (p - q).abs()))
            .fold(0f32, f32::max);
        assert!(worst <= 0.02, "{name}: a normal differs by {worst}");
        assert!(a.colors == b.colors, "{name}: colours differ");
        assert!(a.uvs == b.uvs, "{name}: UVs differ");
        assert!(
            !a.notes.iter().any(|n| n.contains("compressed")),
            "{name}: {:?}",
            a.notes
        );
        eprintln!(
            "{name}: {} triangles, {} vertices, {off} normals within 1 ulp, {ms:.1} ms",
            a.indices.len() / 3,
            a.positions.len()
        );
        checked += 1;
    }
    assert!(checked >= 2);
}
