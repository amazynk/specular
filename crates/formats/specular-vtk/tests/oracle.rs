//! Files written by VTK's own writers, compared with what VTK's readers get from them
//! (`gen/make_samples.py`): the committed fixtures always, a larger corpus when
//! `SPECULAR_VTK_CORPUS` names its folder. `SPECULAR_VTK_BENCH` names a folder of big files
//! to time.

use std::path::{Path, PathBuf};

use serde_json::Value;
use specular_vtk::{DataArray, Dataset, MeshData};

#[derive(Default, Debug)]
struct Tally {
    files: usize,
    arrays: usize,
    values: u64,
    meshes: usize,
    triangles: u64,
    exact_meshes: usize,
    area_meshes: usize,
    refused: usize,
    mismatches: Vec<String>,
}

fn floats(bin: &[f32], off: u64, n: u64) -> &[f32] {
    &bin[off as usize..(off + n) as usize]
}

fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) || (a == 0.0 && b == 0.0)
}

fn compare_arrays(
    t: &mut Tally,
    name: &str,
    what: &str,
    ours: &[DataArray],
    want: &Value,
    bin: &[f32],
) {
    for w in want.as_array().into_iter().flatten() {
        let n = w["name"].as_str().unwrap_or("");
        let c = w["components"].as_u64().unwrap_or(1);
        let tuples = w["tuples"].as_u64().unwrap_or(0);
        let off = w["offset"].as_u64().unwrap_or(0);
        let Some(a) = ours
            .iter()
            .find(|a| a.name == n && a.components as u64 == c)
        else {
            t.mismatches
                .push(format!("{name}: {what} array {n:?} missing"));
            continue;
        };
        let exp = floats(bin, off, c * tuples);
        t.arrays += 1;
        t.values += exp.len() as u64;
        if a.values.len() != exp.len() {
            t.mismatches.push(format!(
                "{name}: {what} {n}: {} values, {} expected",
                a.values.len(),
                exp.len()
            ));
            continue;
        }
        let bad = a
            .values
            .iter()
            .zip(exp)
            .filter(|(x, y)| !same(**x, **y))
            .count();
        if bad > 0 {
            let i = a
                .values
                .iter()
                .zip(exp)
                .position(|(x, y)| !same(*x, *y))
                .unwrap_or(0);
            t.mismatches.push(format!(
                "{name}: {what} {n}: {bad} values differ (first at {i}: {} vs {})",
                a.values[i], exp[i]
            ));
        }
    }
}

fn canonical(m: &MeshData) -> (Vec<[u32; 9]>, f64) {
    let mut out = Vec::with_capacity(m.triangles.len());
    let mut area = 0.0;
    for tri in &m.triangles {
        let mut p: Vec<[f32; 3]> = tri.iter().map(|&i| m.points[i as usize]).collect();
        let d = |a: [f32; 3]| a.map(f64::from);
        let (a, b, c) = (d(p[0]), d(p[1]), d(p[2]));
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let x = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        area += 0.5 * (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
        p.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut k = [0u32; 9];
        for (i, q) in p.iter().flatten().enumerate() {
            k[i] = (*q + 0.0).to_bits();
        }
        out.push(k);
    }
    out.sort_by(|a, b| {
        let fa: Vec<f32> = a.iter().map(|&x| f32::from_bits(x)).collect();
        let fb: Vec<f32> = b.iter().map(|&x| f32::from_bits(x)).collect();
        fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
    });
    (out, area)
}

fn check_file(t: &mut Tally, path: &Path) {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let base = path.with_extension("");
    let json = base.with_extension("json");
    let data = std::fs::read(path).unwrap();
    let parsed = specular_vtk::parse(&data);
    if !json.exists() {
        // Written by VTK but meant to be refused (LZMA).
        match parsed {
            Err(specular_vtk::Error::Unsupported(_)) => t.refused += 1,
            other => t
                .mismatches
                .push(format!("{name}: refusal expected, got {:?}", other.err())),
        }
        return;
    }
    let want: Value = serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
    let raw = std::fs::read(base.with_extension("bin")).unwrap();
    let bin: Vec<f32> = raw
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let v = match parsed {
        Ok(v) => v,
        Err(e) => {
            t.mismatches.push(format!("{name}: {e}"));
            return;
        }
    };
    t.files += 1;
    compare_arrays(t, &name, "field", &v.field_data, &want["field_data"], &bin);
    match &v.dataset {
        Dataset::Grid(g) => {
            let dims: Vec<usize> = want["dims"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d.as_u64().unwrap() as usize)
                .collect();
            if g.dims.to_vec() != dims {
                t.mismatches
                    .push(format!("{name}: dims {:?}, {dims:?} expected", g.dims));
                return;
            }
            if let Some(o) = want["origin"].as_array() {
                let sp = want["spacing"].as_array().unwrap();
                for k in 0..3 {
                    let (eo, es) = (o[k].as_f64().unwrap(), sp[k].as_f64().unwrap());
                    if (g.origin[k] - eo).abs() > 1e-9 * (1.0 + eo.abs())
                        || (g.spacing[k] - es).abs() > 1e-9 * (1.0 + es.abs())
                    {
                        t.mismatches.push(format!(
                            "{name}: origin/spacing {:?} {:?} vs {o:?} {sp:?}",
                            g.origin, g.spacing
                        ));
                    }
                }
            }
            if let Some(c) = want["coords"].as_array() {
                let ours = g.coords.as_ref().expect("coordinates");
                for k in 0..3 {
                    let e = floats(&bin, c[k].as_u64().unwrap(), dims[k] as u64);
                    if ours[k].iter().zip(e).any(|(a, b)| !same(*a as f32, *b)) {
                        t.mismatches.push(format!("{name}: coordinates {k} differ"));
                    }
                }
            }
            if let Some(p) = want["points"].as_u64() {
                let ours = g.points.as_ref().expect("points");
                let e = floats(&bin, p, ours.len() as u64 * 3);
                if ours.iter().flatten().zip(e).any(|(a, b)| !same(*a, *b)) {
                    t.mismatches.push(format!("{name}: points differ"));
                }
            }
            compare_arrays(t, &name, "point", &g.point_data, &want["point_data"], &bin);
            compare_arrays(t, &name, "cell", &g.cell_data, &want["cell_data"], &bin);
        }
        Dataset::Mesh(m) => {
            t.meshes += 1;
            if m.points.len() as u64 != want["points"].as_u64().unwrap() {
                t.mismatches.push(format!(
                    "{name}: {} points, {} expected",
                    m.points.len(),
                    want["points"]
                ));
            }
            if m.cells != want["cells"].as_u64().unwrap() {
                t.mismatches.push(format!(
                    "{name}: {} cells, {} expected",
                    m.cells, want["cells"]
                ));
            }
            compare_arrays(t, &name, "point", &m.point_data, &want["point_data"], &bin);
            compare_arrays(t, &name, "cell", &m.cell_data, &want["cell_data"], &bin);
            let n = want["triangles"].as_u64().unwrap();
            t.triangles += n;
            if m.triangles.len() as u64 != n {
                t.mismatches.push(format!(
                    "{name}: {} triangles, {n} expected",
                    m.triangles.len()
                ));
                return;
            }
            let (ours, area) = canonical(m);
            let e = floats(&bin, want["tri_offset"].as_u64().unwrap(), n * 9);
            let exact = ours
                .iter()
                .flatten()
                .zip(e)
                .all(|(a, b)| same(f32::from_bits(*a), *b));
            let ea = want["area"].as_f64().unwrap();
            if exact {
                t.exact_meshes += 1;
            } else if (area - ea).abs() <= 1e-5 * ea.max(1e-12) {
                // Polygons triangulated along other diagonals: the same surface.
                t.area_meshes += 1;
            } else {
                t.mismatches
                    .push(format!("{name}: surface area {area}, {ea} expected"));
            }
        }
    }
}

fn run(dir: &Path) -> Tally {
    let mut t = Tally::default();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("vtk" | "vti" | "vtr" | "vts" | "vtu" | "vtp")
            )
        })
        .collect();
    files.sort();
    for f in &files {
        check_file(&mut t, f);
    }
    t
}

fn report(label: &str, t: &Tally) {
    println!(
        "{label}: {} files, {} arrays, {} values, {} meshes ({} triangles; {} exact, {} same surface), {} refused, {} mismatches",
        t.files,
        t.arrays,
        t.values,
        t.meshes,
        t.triangles,
        t.exact_meshes,
        t.area_meshes,
        t.refused,
        t.mismatches.len()
    );
    for m in t.mismatches.iter().take(40) {
        println!("  {m}");
    }
}

#[test]
fn fixtures_match_vtk() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let t = run(&dir);
    report("fixtures", &t);
    assert!(t.files >= 40, "{}", t.files);
    assert!(t.refused >= 1);
    assert!(t.mismatches.is_empty());
}

#[test]
fn corpus_matches_vtk() {
    let Some(dir) = std::env::var_os("SPECULAR_VTK_CORPUS") else {
        return;
    };
    let t = run(Path::new(&dir));
    report("corpus", &t);
    assert!(t.mismatches.is_empty());
}

#[test]
fn bench() {
    let Some(dir) = std::env::var_os("SPECULAR_VTK_BENCH") else {
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    for f in files {
        let data = std::fs::read(&f).unwrap();
        let mut best = f64::INFINITY;
        let mut what = String::new();
        for _ in 0..3 {
            let t0 = std::time::Instant::now();
            let v = specular_vtk::parse(&data).unwrap();
            best = best.min(t0.elapsed().as_secs_f64());
            what = match &v.dataset {
                Dataset::Grid(g) => format!("grid {:?}", g.dims),
                Dataset::Mesh(m) => format!("{} triangles", m.triangles.len()),
            };
        }
        println!(
            "{}: {} MB, {what}, {} : {:.1} ms",
            f.file_name().unwrap().to_string_lossy(),
            data.len() >> 20,
            specular_vtk::parse(&data).unwrap().encoding,
            best * 1e3
        );
    }
}

/// Truncated and corrupted copies of every fixture are refused or read, never a panic.
#[test]
fn damaged_fixtures_never_panic() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if !matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("vtk" | "vti" | "vtr" | "vts" | "vtu" | "vtp")
        ) {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        let len = data.len();
        for k in 1..24 {
            let _ = specular_vtk::parse(&data[..len * k / 24]);
        }
        let mut x = data.clone();
        let mut seed = 0x9e37_79b9_7f4a_7c15u64 ^ len as u64;
        for _ in 0..64 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let i = (seed as usize) % len;
            let old = x[i];
            x[i] = (seed >> 32) as u8;
            let _ = specular_vtk::parse(&x);
            x[i] = old;
        }
        n += 1;
    }
    assert!(n >= 40);
}
