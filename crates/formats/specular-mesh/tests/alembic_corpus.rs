//! Alembic files from OpenUSD's tests (tools/corpus/fetch-alembic.sh), against the USD
//! baseline usdAbc wrote for one of them. Run with
//! `SPECULAR_ALEMBIC_CORPUS=<folder> cargo test -p specular-mesh --test alembic_corpus -- --ignored --nocapture`.

use specular_mesh::alembic;
use std::path::Path;

/// `name: [(x, y, z), …]` or `[a, b, …]` values of the first time sample of `attr` in the
/// first prim of a usda file.
fn usda_values(text: &str, attr: &str) -> Vec<f64> {
    let at = text.find(&format!("{attr}.timeSamples")).expect(attr);
    let rest = &text[at..];
    let open = rest.find("0: [").expect("sample") + 4;
    let close = rest[open..].find(']').expect("end") + open;
    rest[open..close]
        .split(|c: char| c == ',' || c == '(' || c == ')' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect()
}

#[test]
#[ignore]
fn corpus() {
    let Some(root) = std::env::var_os("SPECULAR_ALEMBIC_CORPUS") else {
        eprintln!("SPECULAR_ALEMBIC_CORPUS not set");
        return;
    };
    let root = Path::new(&root);
    let mut names: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "abc"))
        .collect();
    names.sort();
    let mut read = 0;
    for p in &names {
        let d = std::fs::read(p).unwrap();
        let a = alembic::archive(&d);
        let m = alembic::meshes(&d);
        let mesh = specular_mesh::read(
            specular_mesh::MeshFormat::Alembic,
            &d,
            &specular_mesh::NoResolver,
            &Default::default(),
        );
        eprintln!(
            "{}: {} objects, {} meshes, {}",
            p.file_name().unwrap().to_string_lossy(),
            a.as_ref().map_or(0, |a| a.objects.len()),
            m.as_ref().map_or(0, |m| m.0.len()),
            match &mesh {
                Ok(m) => format!("{} triangles", m.triangle_count()),
                Err(e) => e.to_string(),
            }
        );
        assert!(a.is_ok(), "{a:?}");
        if mesh.is_ok() {
            read += 1;
        }
    }
    eprintln!("{read} of {} archives drawn", names.len());
    // The mesh usdAbc converted, value for value.
    let d = std::fs::read(root.join("hasColorSet.abc")).unwrap();
    let usda =
        std::fs::read_to_string(root.join("good_primvars_namespace_after_fix.usda")).unwrap();
    let (meshes, _, _) = alembic::meshes(&d).unwrap();
    let m = meshes
        .iter()
        .find(|m| m.path == "/hasColorSet/hasColorSetShape")
        .expect("hasColorSet");
    let points: Vec<f64> = m.positions.iter().flatten().map(|v| *v as f64).collect();
    let counts: Vec<f64> = m.counts.iter().map(|v| *v as f64).collect();
    let indices: Vec<f64> = m.indices.iter().map(|v| *v as f64).collect();
    let mut mismatches = 0;
    for (ours, attr) in [
        (&points, "points"),
        (&counts, "faceVertexCounts"),
        (&indices, "faceVertexIndices"),
    ] {
        let theirs = usda_values(&usda, attr);
        let bad = ours.len().abs_diff(theirs.len())
            + ours
                .iter()
                .zip(&theirs)
                .filter(|(a, b)| (*a - *b).abs() > 1e-6)
                .count();
        eprintln!("{attr}: {} values, {bad} mismatches", theirs.len());
        mismatches += bad;
    }
    // Its transform: usdAbc's xformOp:transform (row vectors) against ours (columns).
    let at = usda.find("xformOp:transform.timeSamples").unwrap();
    let line = usda[at..].lines().nth(1).unwrap();
    let t: Vec<f64> = line
        .trim()
        .trim_start_matches("0:")
        .split(|c: char| c == ',' || c == '(' || c == ')' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect();
    let ours = m.transform;
    let mut bad = 0;
    for r in 0..3 {
        for c in 0..4 {
            if (ours[r][c] - t[c * 4 + r]).abs() > 1e-9 {
                bad += 1;
            }
        }
    }
    eprintln!("transform: 12 values, {bad} mismatches");
    mismatches += bad;
    assert_eq!(mismatches, 0);
}
