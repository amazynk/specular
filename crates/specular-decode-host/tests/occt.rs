//! STEP and IGES through OpenCASCADE in the sandboxed host, against OCCT's own reading.
//!
//! The corpus (`tools/corpus/fetch-occt.sh`, then `tools/corpus/occt-oracle.sh` for the
//! generated models and `oracle.tsv`) is read from `SPECULAR_OCCT_CORPUS`, else
//! tools/corpus/occt. `SPECULAR_OCCT_ORACLE` names the oracle program, for the triangle
//! counts. Every test skips (with a message) when the host has no OpenCASCADE, unless
//! `SPECULAR_REQUIRE_OCCT=1`.
//!
//! What is compared, per model: part instances, faces and solids (all instances); the mesh's
//! box against OCCT's exact box (within twice the deflection); the mesh's area and enclosed
//! volume against OCCT's exact `BRepGProp` area and volume (1.5 %, or for the volume half
//! the area times the deflection when that is more); and, with the oracle
//! program, the triangle count against OCCT's BRepMesh at the same deflection (exact).
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use specular_sandbox::protocol::{CadFormat, CadLimits, ErrorKind, TestOp};
use specular_sandbox::{CadModel, Config, Error, Host, SandboxLevel};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn config(test_hooks: bool) -> Config {
    Config {
        exe: Some(env!("CARGO_BIN_EXE_specular-decode-host").into()),
        timeout: Duration::from_secs(300),
        pool_size: 1,
        memory_limit: 8 << 30,
        test_hooks,
        occt: true,
        ..Config::default()
    }
}

/// A host with OpenCASCADE, or `None` (skip) when this machine has none.
fn occt_host(test_hooks: bool) -> Option<Host> {
    let h = Host::new(config(test_hooks));
    let info = h.info().expect("host starts");
    if info.occt.is_none() {
        let must = std::env::var_os("SPECULAR_REQUIRE_OCCT").is_some_and(|v| v == "1");
        assert!(!must, "OpenCASCADE required: {}", info.occt_error);
        eprintln!("skipped: no OpenCASCADE ({})", info.occt_error);
        return None;
    }
    Some(h)
}

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_OCCT_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tools/corpus/occt"
            ))
        });
    if dir.join("oracle.tsv").is_file() {
        Some(dir)
    } else {
        eprintln!("skipped: no STEP/IGES corpus at {}", dir.display());
        None
    }
}

fn format_of(name: &str) -> CadFormat {
    let l = name.to_ascii_lowercase();
    if l.ends_with(".igs") || l.ends_with(".iges") {
        CadFormat::Iges
    } else {
        CadFormat::Step
    }
}

/// One line of oracle.tsv.
struct Oracle {
    file: String,
    leaves: u32,
    faces: u32,
    solids: u32,
    bbox: [f64; 6],
    area: f64,
    volume: f64,
}

fn oracle(dir: &std::path::Path) -> Vec<Oracle> {
    let text = std::fs::read_to_string(dir.join("oracle.tsv")).unwrap();
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 13 {
                return None;
            }
            let n = |i: usize| f[i].parse::<f64>().unwrap();
            Some(Oracle {
                file: f[0].to_owned(),
                leaves: f[1].parse().unwrap(),
                faces: f[2].parse().unwrap(),
                solids: f[3].parse().unwrap(),
                bbox: [n(4), n(5), n(6), n(7), n(8), n(9)],
                area: n(10),
                volume: n(11),
            })
        })
        .collect()
}

/// The mesh's box, area and enclosed (signed) volume.
fn measure(m: &CadModel) -> ([f64; 6], f64, f64) {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::INFINITY,
        -f64::INFINITY,
        -f64::INFINITY,
        -f64::INFINITY,
    ];
    let (mut area, mut volume) = (0.0, 0.0);
    for mesh in &m.meshes {
        let p = |i: u32| mesh.positions[i as usize].map(f64::from);
        for q in &mesh.positions {
            for c in 0..3 {
                b[c] = b[c].min(f64::from(q[c]));
                b[c + 3] = b[c + 3].max(f64::from(q[c]));
            }
        }
        for t in mesh.indices.chunks_exact(3) {
            let (a, bb, c) = (p(t[0]), p(t[1]), p(t[2]));
            let u = [bb[0] - a[0], bb[1] - a[1], bb[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            area += (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0;
            volume += (a[0] * (bb[1] * c[2] - bb[2] * c[1]) - a[1] * (bb[0] * c[2] - bb[2] * c[0])
                + a[2] * (bb[0] * c[1] - bb[1] * c[0]))
                / 6.0;
        }
    }
    (b, area, volume)
}

/// OCCT's own triangle count at this deflection, when the oracle program is available.
fn oracle_triangles(file: &std::path::Path, m: &CadModel, limits: &CadLimits) -> Option<u64> {
    let prog = std::env::var_os("SPECULAR_OCCT_ORACLE")?;
    let out = std::process::Command::new(prog)
        .arg("mesh")
        .arg(file)
        .arg(format!("{}", m.deflection))
        .arg(format!("{}", limits.angular_deflection))
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[test]
fn step_and_iges_match_occt() {
    let Some(h) = occt_host(false) else { return };
    let Some(dir) = corpus() else { return };
    let limits = CadLimits::default();
    let (mut checked, mut mismatches) = (0, Vec::new());
    for o in oracle(&dir) {
        let path = dir.join(&o.file);
        let bytes = std::fs::read(&path).unwrap();
        let t0 = Instant::now();
        let raw = h
            .cad_read(&bytes, format_of(&o.file), limits)
            .unwrap_or_else(|e| panic!("{}: {e}", o.file));
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let m = CadModel::decode(&raw, limits.max_triangles, limits.max_triangles * 3).unwrap();
        let (b, area, volume) = measure(&m);
        let diag = ((0..3)
            .map(|c| (o.bbox[c + 3] - o.bbox[c]).powi(2))
            .sum::<f64>())
        .sqrt();
        let tol = 2.0 * m.deflection + 1e-6 * diag;
        let mut bad = Vec::new();
        if (m.instances, m.faces, m.solids) != (o.leaves, o.faces, o.solids) {
            bad.push(format!(
                "counts {:?} vs OCCT {:?}",
                (m.instances, m.faces, m.solids),
                (o.leaves, o.faces, o.solids)
            ));
        }
        if (0..6).any(|i| (b[i] - o.bbox[i]).abs() > tol) {
            bad.push(format!("box {b:?} vs OCCT {:?} (±{tol})", o.bbox));
        }
        let rel = |a: f64, e: f64| ((a - e) / e.abs().max(1e-30)).abs();
        if o.area > 0.0 && rel(area, o.area) > 0.015 {
            bad.push(format!("area {area} vs OCCT {}", o.area));
        }
        // Chords cut inside curved faces by at most the deflection: the volume lost is at
        // most about half the area times it (and never more than 1.5 % on fine models).
        let vol_tol = (0.015 * o.volume).max(o.area * m.deflection / 2.0);
        if o.volume > 0.0 && (volume - o.volume).abs() > vol_tol {
            bad.push(format!("volume {volume} vs OCCT {}", o.volume));
        }
        let tris = m.triangle_count() as u64;
        let theirs = oracle_triangles(&path, &m, &limits);
        if let Some(t) = theirs
            && t != tris
        {
            bad.push(format!("{tris} triangles vs OCCT {t}"));
        }
        eprintln!(
            "{:<45} {:>8} triangles{} {:>5} parts  area {:+.3} %  volume {}  {ms:.0} ms",
            o.file,
            tris,
            theirs.map_or(String::new(), |t| format!(" (OCCT {t})")),
            m.instances,
            (area / o.area - 1.0) * 100.0,
            if o.volume > 0.0 {
                format!("{:+.3} %", (volume / o.volume - 1.0) * 100.0)
            } else {
                "-".into()
            },
        );
        checked += 1;
        if !bad.is_empty() {
            mismatches.push(format!("{}: {}", o.file, bad.join("; ")));
        }
    }
    eprintln!("{checked} models compared, {} mismatches", mismatches.len());
    assert!(checked > 0);
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn assembly_tree_names_and_colours() {
    let Some(h) = occt_host(false) else { return };
    let Some(dir) = corpus() else { return };
    for file in ["gen/assembly.step", "gen/assembly_brep.igs"] {
        let bytes = std::fs::read(dir.join(file)).unwrap();
        let raw = h
            .cad_read(&bytes, format_of(file), CadLimits::default())
            .unwrap();
        let m = CadModel::decode(&raw, 1 << 30, 1 << 30).unwrap();
        let names: Vec<&str> = m.nodes.iter().map(|n| n.name.as_str()).collect();
        eprintln!("{file}: {names:?}");
        let near = |c: Option<[f32; 4]>, want: [f32; 3]| {
            c.is_some_and(|c| (0..3).all(|i| (c[i] - want[i]).abs() < 0.02))
        };
        // Bracket blue (3 instances), cone yellow with one red face.
        let blue = m
            .meshes
            .iter()
            .filter(|x| near(x.colour, [0.2, 0.4, 0.9]))
            .count();
        // IGES has no assembly structure: OCCT writes the fixture as one shape.
        let want = if file.ends_with(".step") { 3 } else { 1 };
        assert_eq!(blue, want, "{file}: blue brackets");
        let colours: Vec<_> = m.meshes.iter().map(|x| x.colour).collect();
        eprintln!("{file}: colours {colours:?}");
        // Through IGES only the brackets keep their colour: OCCT's own XCAF reading of that
        // file has no style for the cone (`occt-oracle styles`), so neither do we.
        if file.ends_with(".step") {
            assert!(
                m.meshes.iter().any(|x| near(x.colour, [1.0, 0.0, 0.0])),
                "{file}: red face"
            );
            assert!(
                m.meshes.iter().any(|x| near(x.colour, [0.9, 0.9, 0.2])),
                "{file}: yellow cone"
            );
        }
        if file.ends_with(".step") {
            assert_eq!(m.nodes[0].name, "Fixture");
            assert!(m.nodes[0].assembly && m.nodes[0].parent.is_none());
            for n in [
                "Bracket left",
                "Bracket right",
                "Bracket mirrored",
                "Cone 1",
            ] {
                assert!(
                    names.iter().any(|x| x.starts_with(n)),
                    "{file}: {n} in {names:?}"
                );
            }
            assert!(m.nodes[1..].iter().all(|n| n.parent == Some(0)));
        }
    }
    // A real assembly with colours: AS1 (18 parts).
    let bytes = std::fs::read(dir.join("pythonocc/as1-oc-214.stp")).unwrap();
    let raw = h
        .cad_read(&bytes, CadFormat::Step, CadLimits::default())
        .unwrap();
    let m = CadModel::decode(&raw, 1 << 30, 1 << 30).unwrap();
    assert_eq!(m.instances, 18);
    assert!(m.nodes.iter().filter(|n| n.assembly).count() >= 2);
    assert!(
        m.meshes.iter().all(|x| x.colour.is_some()),
        "every AS1 part is coloured"
    );
}

#[test]
fn coarse_limits_and_bad_files() {
    let Some(h) = occt_host(true) else { return };
    let Some(dir) = corpus() else { return };
    let bytes = std::fs::read(dir.join("gen/grid_1600.step")).unwrap();
    // A triangle cap far below the model: meshed coarser, then refused.
    let tight = CadLimits {
        max_triangles: 1000,
        ..CadLimits::default()
    };
    match h.cad_read(&bytes, CadFormat::Step, tight) {
        Err(Error::Remote { kind, message }) => {
            assert_eq!(kind, ErrorKind::TooLarge, "{message}")
        }
        r => panic!("{:?}", r.map(|b| b.len())),
    }
    let loose = CadLimits {
        max_triangles: 1_000_000,
        ..CadLimits::default()
    };
    let raw = h.cad_read(&bytes, CadFormat::Step, loose).unwrap();
    let m = CadModel::decode(&raw, loose.max_triangles, u64::MAX).unwrap();
    assert!(m.triangle_count() <= 1_000_000);
    assert!(
        m.notes.iter().any(|n| n.contains("coarser")),
        "{:?}",
        m.notes
    );
    // Not STEP, truncated STEP, IGES bytes given as STEP: refused, the host survives.
    let step = std::fs::read(dir.join("gen/assembly.step")).unwrap();
    for (what, data, fmt) in [
        ("garbage", b"hello world".to_vec(), CadFormat::Step),
        ("empty", Vec::new(), CadFormat::Iges),
        (
            "truncated",
            step[..step.len() / 3].to_vec(),
            CadFormat::Step,
        ),
        (
            "header only",
            step[..step.len().min(600)].to_vec(),
            CadFormat::Step,
        ),
        ("STEP as IGES", step.clone(), CadFormat::Iges),
    ] {
        match h.cad_read(&data, fmt, CadLimits::default()) {
            Err(Error::Remote { kind, message }) => {
                eprintln!("{what}: {kind}: {message}");
                assert!(
                    matches!(kind, ErrorKind::Malformed | ErrorKind::Unsupported),
                    "{what}: {kind}"
                );
            }
            Ok(raw) => {
                // OCCT may salvage part of a truncated file; what it sends must still read.
                let m = CadModel::decode(&raw, 1 << 30, 1 << 30).unwrap();
                eprintln!("{what}: read {} triangles", m.triangle_count());
            }
            Err(e) => panic!("{what}: {e}"),
        }
    }
    assert!(
        h.test(TestOp::Echo(vec![1, 2, 3])).is_ok(),
        "host still answers"
    );
}

/// Loading OpenCASCADE happens before the sandbox: the host is as confined as any other.
#[test]
fn occt_host_is_sandboxed() {
    let Some(h) = occt_host(true) else { return };
    let info = h.info().unwrap();
    eprintln!(
        "OpenCASCADE {:?}, sandbox {:?}: {}",
        info.occt, info.sandbox, info.sandbox_detail
    );
    #[cfg(target_os = "linux")]
    assert!(info.sandbox >= SandboxLevel::Partial, "{info:?}");
    let _ = SandboxLevel::Full;
    let dir = std::env::temp_dir();
    let r = h
        .test(TestOp::Probe {
            read_path: concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml").into(),
            write_dir: dir.display().to_string(),
        })
        .unwrap();
    let specular_sandbox::protocol::Response::Probe(results) = r else {
        panic!("{r:?}")
    };
    if info.sandbox == SandboxLevel::Full {
        for p in &results {
            assert!(!p.allowed, "{} allowed: {}", p.name, p.detail);
        }
    }
}

/// A host started without `--occt` says so instead of reading.
#[test]
fn plain_host_refuses_cad() {
    let h = Host::new(Config {
        occt: false,
        ..config(false)
    });
    match h.cad_read(b"ISO-10303-21;", CadFormat::Step, CadLimits::default()) {
        Err(Error::Remote { kind, .. }) => assert_eq!(kind, ErrorKind::NotInstalled),
        r => panic!("{:?}", r.map(|b| b.len())),
    }
}
