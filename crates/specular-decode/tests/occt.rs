//! STEP and IGES opened through the registry: OpenCASCADE in the sandboxed decode host, the
//! model as a mesh session with its header and assembly tree.
//!
//! Needs the decode host built next to this test (`cargo build -p specular-decode-host`, with
//! OpenCASCADE found so its bridge is built too) and the corpus (`tools/corpus/fetch-occt.sh`
//! and `tools/corpus/occt-oracle.sh`, read from `SPECULAR_OCCT_CORPUS`, else
//! tools/corpus/occt). Skips with a message otherwise, unless `SPECULAR_REQUIRE_OCCT=1`.
//! The host-level comparison with OpenCASCADE's own measurements is
//! specular-decode-host's tests/occt.rs.

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeError, DecodeRequest, Format, Registry, Session};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_OCCT_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tools/corpus/occt"
            ))
        });
    if dir.join("gen/assembly.step").is_file() {
        Some(dir)
    } else {
        eprintln!("skipped: no STEP/IGES corpus at {}", dir.display());
        None
    }
}

/// Opens a corpus file; `None` (skip) when this machine cannot run OpenCASCADE.
fn open(dir: &std::path::Path, name: &str) -> Option<Arc<dyn Session>> {
    let bytes = std::fs::read(dir.join(name)).unwrap();
    let src = Arc::new(Source::from_vec(name, bytes));
    match Registry::default().open(src) {
        Ok(s) => Some(s),
        Err(DecodeError::Unsupported(m)) if m.contains("need") => {
            let must = std::env::var_os("SPECULAR_REQUIRE_OCCT").is_some_and(|v| v == "1");
            assert!(!must, "OpenCASCADE required: {m}");
            eprintln!("skipped: {m}");
            None
        }
        Err(e) => panic!("{name}: {e}"),
    }
}

fn field<'a>(s: &'a dyn Session, key: &str) -> Option<&'a str> {
    s.header()?
        .fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

#[test]
fn step_assembly_opens_as_a_mesh_with_its_tree() {
    let Some(dir) = corpus() else { return };
    let Some(s) = open(&dir, "gen/assembly.step") else {
        return;
    };
    assert_eq!(s.info().format, Format::Step);
    assert!(Format::Step.is_mesh());
    let m = s.mesh().expect("a mesh");
    let mesh = &m.mesh;
    assert_eq!(mesh.triangle_count(), 860);
    assert_eq!(mesh.unit, specular_decode::specular_mesh::Unit::Millimetre);
    // Blue brackets, a yellow cone, its red face: three colours, named after their parts.
    let names: Vec<&str> = mesh.materials.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(mesh.materials.len(), 3, "{names:?}");
    assert!(names.iter().any(|n| n.starts_with("Bracket")), "{names:?}");
    assert_eq!(mesh.title.as_deref(), Some("Fixture"));
    assert_eq!(field(&*s, "Protocol"), Some("AP214"));
    assert_eq!(field(&*s, "Part instances"), Some("4"));
    assert_eq!(field(&*s, "Assembly 1"), Some("Fixture (assembly)"));
    assert_eq!(
        field(&*s, "Assembly 2"),
        Some("  Bracket left (Bracket) (part)")
    );
    let summary = &s.header().unwrap().summary;
    assert!(summary.iter().any(|(k, _)| *k == "created"), "{summary:?}");
    // The plain image (thumbnails, the 2-D view) renders.
    let r = s
        .decode(&DecodeRequest::full(8), &CancelToken::new())
        .unwrap();
    assert!(r.width > 0 && r.height > 0);
}

#[test]
fn iges_opens_and_ifc_is_refused() {
    let Some(dir) = corpus() else { return };
    for name in ["gen/faces.iges", "occt/bearing.iges"] {
        let Some(s) = open(&dir, name) else { return };
        assert_eq!(s.info().format, Format::Iges, "{name}");
        assert!(s.mesh().unwrap().mesh.triangle_count() > 1000, "{name}");
    }
    let src = Arc::new(Source::from_vec(
        "house.ifc",
        b"ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('IFC4'));\nENDSEC;\n".to_vec(),
    ));
    let e = Registry::default()
        .open(src)
        .err()
        .expect("refused")
        .to_string();
    assert!(e.contains("IFC"), "{e}");
}

/// Thumbnail workers get a coarser mesh; a large assembly opens in the host while the caller
/// only waits (the loader thread, never the window).
#[test]
fn thumbnails_coarser_and_large_assembly_timing() {
    let Some(dir) = corpus() else { return };
    let Some(full) = open(&dir, "gen/sphere.step") else {
        return;
    };
    let fine = full.mesh().unwrap().mesh.triangle_count();
    let coarse = {
        let _g = specular_core::budget::enter_share(specular_core::budget::Share::Thumbnails);
        open(&dir, "gen/sphere.step")
            .unwrap()
            .mesh()
            .unwrap()
            .mesh
            .triangle_count()
    };
    eprintln!("sphere: {fine} triangles, {coarse} for thumbnails");
    assert!(coarse * 4 < fine, "{coarse} vs {fine}");
    for name in [
        "gen/grid_1600.step",
        "pythonocc/as1-oc-214.stp",
        "occt/linkrods.step",
    ] {
        let t0 = Instant::now();
        let s = open(&dir, name).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let mesh = &s.mesh().unwrap().mesh;
        eprintln!(
            "{name}: {} triangles, {} materials, opened in {ms:.0} ms (machine under load)",
            mesh.triangle_count(),
            mesh.materials.len()
        );
    }
}

/// `pmi-twice.step` (specular-decode-host's tests/data, `tools/corpus/pmi-assembly.py`): one
/// annotated block (100 x 40 x 20 mm; a 100 ±0.2 dimension, a 0.05 flatness) used twice,
/// the second turned 90° about z and moved to (150, 50, 0). Opened through the registry,
/// each annotation comes once per instance: its faces on that instance only, and its label,
/// leader end and drawing moved with it.
#[test]
fn step_pmi_follows_each_part_instance() {
    use specular_decode::pmi::{Characteristic, PmiItem, PmiKind};
    let dir = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../specular-decode-host/tests/data"
    ));
    let Some(s) = open(&dir, "pmi-twice.step") else {
        return;
    };
    let mesh = &s.mesh().expect("a mesh").mesh;
    assert_eq!(mesh.up, specular_decode::specular_mesh::Up::Z);
    let pmi = s.pmi().expect("PMI");
    let items: Vec<&PmiItem> = pmi
        .items
        .iter()
        .filter(|i| i.kind != PmiKind::Graphic)
        .collect();
    let dims: Vec<&PmiItem> = items
        .iter()
        .copied()
        .filter(|i| i.kind == PmiKind::Dimension)
        .collect();
    let flat: Vec<&PmiItem> = items
        .iter()
        .copied()
        .filter(|i| i.characteristic == Some(Characteristic::Flatness))
        .collect();
    assert_eq!(
        (dims.len(), flat.len(), items.len()),
        (2, 2, 4),
        "{items:#?}"
    );
    assert!(dims.iter().all(|d| d.text.contains("100")), "{dims:?}");
    assert!(flat.iter().all(|f| f.text.contains("0.05")), "{flat:?}");
    // The second instance's place, in the file's frame (millimetres, z up as the mesh).
    let turned = |p: [f32; 3]| [150.0 - p[1], 50.0 + p[0], p[2]];
    let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3);
    let corners = |i: &PmiItem| -> Vec<[f32; 3]> {
        let mut v: Vec<[f32; 3]> = i
            .triangles
            .iter()
            .flat_map(|&(first, count)| {
                mesh.indices[first as usize * 3..(first + count) as usize * 3]
                    .iter()
                    .map(|&k| mesh.positions[k as usize])
            })
            .collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.dedup_by(|a, b| close(*a, *b));
        v
    };
    for pair in [&dims, &flat] {
        // Which copy is on the unmoved block: its faces inside 0..100 in x.
        let on_first = |i: &PmiItem| {
            corners(i)
                .iter()
                .all(|p| p[0] > -1e-3 && p[0] < 100.0 + 1e-3)
        };
        let (a, b) = if on_first(pair[0]) {
            (pair[0], pair[1])
        } else {
            (pair[1], pair[0])
        };
        assert!(on_first(a) && !on_first(b), "{a:?} {b:?}");
        assert!(!a.triangles.is_empty());
        // The second copy's faces are the first's, placed.
        let mut want: Vec<[f32; 3]> = corners(a).into_iter().map(turned).collect();
        want.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let got = corners(b);
        assert_eq!(want.len(), got.len(), "{}", a.text);
        for (w, g) in want.iter().zip(&got) {
            assert!(close(*w, *g), "{}: {w:?} vs {g:?}", a.text);
        }
        // Label, leader end and drawing moved with it.
        if let (Some(x), Some(y)) = (a.anchor, b.anchor) {
            assert!(close(turned(x), y), "{}: anchor {x:?} to {y:?}", a.text);
        }
        if let (Some(x), Some(y)) = (a.target, b.target) {
            assert!(close(turned(x), y), "{}: target {x:?} to {y:?}", a.text);
        }
        for (la, lb) in a.polylines.iter().zip(&b.polylines) {
            for (x, y) in la.iter().zip(lb) {
                assert!(close(turned(*x), *y), "{}: drawing {x:?} to {y:?}", a.text);
            }
        }
    }
    // The info panel counts both copies.
    let facts = &s.header().unwrap().fields;
    for key in ["PMI dimensions", "PMI geometric tolerances"] {
        assert!(
            facts.iter().any(|(k, v)| k == key && v == "2"),
            "{key}: {facts:?}"
        );
    }
}
