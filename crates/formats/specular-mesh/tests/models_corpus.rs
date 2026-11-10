//! The 3D and CAD corpus (tools/corpus/fetch-models.sh) against its oracles
//! (`gen/models_oracle.py`: usd-core for USD, pycollada and assimp for COLLADA, assimp for 3DS,
//! LightWave and FBX): every file is read without a panic, and where an oracle read it the
//! triangle count and the bounds agree. Reads SPECULAR_MODELS_CORPUS, else
//! tools/corpus/models; skips when the oracle's facts are absent.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_mesh::{Limits, MeshFormat, read_path};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_MODELS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/models")
        });
    dir.join("oracle/models.tsv").is_file().then_some(dir)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .and_then(MeshFormat::from_extension)
            .is_some_and(|f| {
                !matches!(
                    f,
                    MeshFormat::Stl
                        | MeshFormat::Obj
                        | MeshFormat::Ply
                        | MeshFormat::Gltf
                        | MeshFormat::Glb
                        | MeshFormat::ThreeMf
                )
            })
        {
            out.push(p);
        }
    }
}

/// Files whose oracle reading is not comparable, with why.
const NOT_COMPARABLE: &[(&str, &str)] = &[
    (
        "weird_matrix_transform.usda",
        "a projective matrix (a last column other than 0, 0, 0, 1): drawn as its affine part",
    ),
    (
        "box_nested_animation.dae",
        "a skinned mesh: assimp poses it by its skeleton, we show the bind pose",
    ),
    (
        "UglyVertexColors.lwo",
        "assimp leaves out its 24-sided face; we triangulate it (22 triangles)",
    ),
    (
        "concave_self_intersecting.lwo",
        "a self-intersecting face: assimp's triangulation leaves out 2 triangles",
    ),
    (
        "nonplanar_polygon.lwo",
        "a non-planar face: assimp's triangulation leaves out 2 triangles",
    ),
    (
        "timeCodesPerSecond_-1.usda",
        "an invalid time-code rate: usd-core samples the animation at its end, we at its start",
    ),
];

#[test]
fn models_corpus_matches_the_oracles() {
    let Some(root) = corpus() else {
        eprintln!("no models corpus or oracle; skipped");
        return;
    };
    let tsv = std::fs::read_to_string(root.join("oracle/models.tsv")).unwrap();
    // file → reader → key → value
    let mut facts: HashMap<String, HashMap<String, HashMap<String, String>>> = HashMap::new();
    for l in tsv.lines() {
        let f: Vec<&str> = l.splitn(4, '\t').collect();
        if f.len() == 4 {
            facts
                .entry(f[0].to_owned())
                .or_default()
                .entry(f[1].to_owned())
                .or_default()
                .insert(f[2].to_owned(), f[3].to_owned());
        }
    }
    let mut files = Vec::new();
    walk(&root, &mut files);
    let (mut read, mut refused, mut compared, mut mismatches) = (0, 0, 0, 0);
    let mut by_format: HashMap<&str, (usize, usize)> = HashMap::new();
    let mut slowest = (0f64, String::new());
    for path in &files {
        let rel = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with("splats/") || rel.starts_with("oracle/") {
            continue;
        }
        let t = Instant::now();
        let got = std::panic::catch_unwind(|| read_path(path, &Limits::default()))
            .unwrap_or_else(|_| panic!("{rel}: panicked"));
        let ms = t.elapsed().as_secs_f64() * 1e3;
        if ms > slowest.0 {
            slowest = (ms, rel.clone());
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let fmt = MeshFormat::from_extension(&ext).map_or("?", MeshFormat::name);
        let e = by_format.entry(fmt).or_default();
        e.0 += 1;
        // The first oracle that found triangles, else the first that read the file.
        let oracle = facts.get(&rel).and_then(|r| {
            let readers: [&'static str; 3] = ["pxr", "pycollada", "assimp"];
            let ok = |k: &&'static str| {
                r.get(*k)
                    .filter(|f| !f.contains_key("error"))
                    .map(|f| (*k, f))
            };
            readers
                .iter()
                .filter_map(ok)
                .find(|(_, f)| f.get("tris").is_some_and(|t| t != "0"))
                .or_else(|| readers.iter().find_map(ok))
        });
        let m = match got {
            Ok(m) => {
                read += 1;
                e.1 += 1;
                m
            }
            Err(err) => {
                refused += 1;
                // An oracle that found triangles where we refused is a mismatch.
                if let Some((who, f)) = oracle
                    && f.get("tris").is_some_and(|t| t != "0")
                {
                    eprintln!(
                        "MISMATCH {rel}: refused ({err}) but {who} reads {} triangles",
                        f["tris"]
                    );
                    mismatches += 1;
                } else {
                    eprintln!("refused {rel}: {err}");
                }
                continue;
            }
        };
        let Some((who, f)) = oracle else {
            continue;
        };
        if NOT_COMPARABLE.iter().any(|(n, _)| rel.ends_with(n)) {
            continue;
        }
        compared += 1;
        let mut bad = Vec::new();
        let implicit = f.get("implicit").is_some_and(|v| v == "1");
        if !implicit
            && let Some(t) = f.get("tris")
            && t.parse::<usize>().unwrap() != m.triangle_count()
        {
            bad.push(format!("{} triangles, {who} {t}", m.triangle_count()));
        }
        let v3 = |s: &str| -> Vec<f64> { s.split(',').map(|v| v.parse().unwrap()).collect() };
        if let (Some(lo), Some(hi)) = (f.get("min"), f.get("max")) {
            let (lo, hi) = (v3(lo), v3(hi));
            let size = (0..3).map(|k| hi[k] - lo[k]).fold(0f64, f64::max).max(1e-9);
            let tol = size * 1e-3;
            let ok = (0..3).all(|k| {
                (f64::from(m.bounds.min[k]) - lo[k]).abs() <= tol
                    && (f64::from(m.bounds.max[k]) - hi[k]).abs() <= tol
            });
            // Implicit shapes add to the box: only that it holds the oracle's.
            let holds = (0..3).all(|k| {
                f64::from(m.bounds.min[k]) <= lo[k] + tol
                    && f64::from(m.bounds.max[k]) >= hi[k] - tol
            });
            if !(ok || implicit && holds) {
                bad.push(format!("bounds {:?} vs {who} {lo:?}..{hi:?}", m.bounds));
            }
        }
        if !bad.is_empty() {
            mismatches += 1;
            eprintln!("MISMATCH {rel}: {}", bad.join("; "));
        }
    }
    let mut fm: Vec<_> = by_format.into_iter().collect();
    fm.sort();
    for (f, (n, ok)) in fm {
        eprintln!("{f}: {ok} of {n} read");
    }
    eprintln!(
        "{} files: {read} read, {refused} refused; {compared} compared with an oracle, \
         {mismatches} mismatches; slowest {:.0} ms ({})",
        read + refused,
        slowest.0,
        slowest.1
    );
    assert_eq!(mismatches, 0);
}

/// Packaged and referenced textures are found, materials named.
#[test]
fn usd_textures_and_materials_resolve() {
    let Some(root) = corpus() else {
        return;
    };
    for (rel, min_images) in [
        ("usd/test_assets/USDZ/DamagedHelmet/DamagedHelmet.usdz", 1),
        ("usd/full_assets/McUsd/McUsd.usdz", 1),
        ("usd/full_assets/Teapot/Teapot.usd", 0),
        ("assimp/models/Collada/duck.dae", 1),
        ("assimp/models/Collada/duck.zae", 1),
        ("assimp/models/3DS/cube_with_diffuse_texture.3DS", 1),
        ("assimp/models/FBX/embedded_ascii/box.FBX", 1),
        ("assimp/models/LWO/LWO2/uvtest.lwo", 1),
    ] {
        let p = root.join(rel);
        if !p.is_file() {
            continue;
        }
        let m = read_path(&p, &Limits::default()).unwrap();
        let found = m
            .images
            .iter()
            .filter(|i| matches!(i, specular_mesh::ImageData::Bytes { .. }))
            .count();
        eprintln!(
            "{rel}: {} materials, {} images ({found} found), unit {:?}, up {:?}, notes {:?}",
            m.materials.len(),
            m.images.len(),
            m.unit,
            m.up,
            m.notes
        );
        assert!(found >= min_images, "{rel}");
        assert!(!m.materials.is_empty(), "{rel}");
    }
}
