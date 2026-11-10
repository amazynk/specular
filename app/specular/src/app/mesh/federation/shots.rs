//! Screenshot tests of a scene of two IFC models: the model list (coloured by model) and the
//! clashes between the models (a duct of the services model through a wall of the
//! architecture model), in English and right to left in Hebrew on the light theme; and the
//! scene's facts: placement by the two map conversions, cross-model clashes, sidecar keys
//! and the BCF topic naming both files.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular federation::shots`, then review
//! and commit the rewritten PNGs.

use std::sync::Arc;

use specular_bim::clash::{self, Mode, Selector};
use specular_bim::federation::Scoped;
use specular_decode::specular_mesh::Unit;
use specular_render::PxRect;
use specular_render_3d::{Shading, StandardView};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use super::*;
use crate::app::mesh::bim::{PanelKind, a11y, draw_panel};
use crate::app::mesh::checks::shots::{Part, ifc};

const SIZE: (f32, f32) = (560.0, 380.0);

const ARCH: &[Part] = &[
    (
        "IFCSLAB",
        "1SlabAAAAAAAAAAAAAAAAA",
        "Slab",
        [-0.5, -0.5, -0.25],
        [12.5, 8.5, 0.0],
        "",
    ),
    (
        "IFCWALL",
        "1WallSouthAAAAAAAAAAAA",
        "South",
        [0.0, 0.0, 0.0],
        [12.0, 0.25, 2.8],
        "2HR",
    ),
    (
        "IFCWALL",
        "1WallNorthAAAAAAAAAAAA",
        "North",
        [0.0, 7.75, 0.0],
        [12.0, 8.0, 2.8],
        "1HR",
    ),
    (
        "IFCWALL",
        "1WallWestAAAAAAAAAAAAA",
        "West",
        [0.0, 0.25, 0.0],
        [0.25, 7.75, 2.8],
        "",
    ),
];

/// The services model in its own coordinates: a duct along x, a pipe along y.
const MEP: &[Part] = &[
    (
        "IFCDUCTSEGMENT",
        "2DuctAAAAAAAAAAAAAAAAA",
        "Supply duct",
        [-1.0, -0.2, 1.2],
        [3.0, 0.2, 1.5],
        "",
    ),
    (
        "IFCPIPESEGMENT",
        "2PipeAAAAAAAAAAAAAAAAA",
        "Drain",
        [5.0, -4.0, 0.3],
        [5.1, -1.0, 0.4],
        "",
    ),
];

/// `parts` with a map conversion (metres, the x axis turned `deg` on the map).
fn mapped(parts: &[Part], e: f64, n: f64, deg: f64) -> Vec<u8> {
    let s = String::from_utf8(ifc(parts)).unwrap();
    let (sin, cos) = deg.to_radians().sin_cos();
    let geo = format!(
        "#10=IFCPROJECTEDCRS('EPSG:25832',$,$,$,$,$,#5);\n\
         #11=IFCMAPCONVERSION(#2,#10,{e:.3},{n:.3},0.,{cos:.6},{sin:.6},1.);\n#90="
    );
    s.replacen("#90=", &geo, 1).into_bytes()
}

fn loaded(name: &str, bytes: &[u8]) -> Loaded {
    let m = specular_decode::specular_mesh::ifc::read(bytes, &Limits::default(), None).unwrap();
    let d = ifcdata::read(bytes, &Limits::default()).unwrap();
    let mut elements = m.elements.clone();
    elements.sort_by_key(|e| e.first_vertex);
    let mut mesh = m.mesh;
    mesh.unit = Unit::Metre;
    let index = IfcIndex {
        elements,
        mesh_metres: m.mesh_metres,
        origin_metres: m.origin_metres,
        georef: m.georef,
    };
    let model = MeshModel {
        mesh: Arc::new(mesh),
        textures: Arc::new(Vec::new()),
        ifc: Some(Arc::new(index)),
    };
    let data = Arc::new(d);
    let (project, _) = project_and_date(&model, Some(&data));
    Loaded {
        path: PathBuf::from(name),
        model,
        data: Some(data),
        project,
        date: step_date(bytes),
        hold: None,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum What {
    Models,
    Clash,
}

/// The architecture model opened, the services model added; for `Clash` the clashes of
/// one against the other found and the first gone to.
fn setup(mv: &mut MeshView, what: What) {
    let arch = loaded(
        "architecture.ifc",
        &mapped(ARCH, 500_000.0, 6_000_000.0, 0.0),
    );
    let mep = loaded("services.ifc", &mapped(MEP, 500_006.0, 5_999_998.0, 90.0));
    mv.model = Some(arch.model.clone());
    mv.bim.checks.base = arch.data.clone();
    mv.bim.checks.data = arch.data.clone();
    assert!(mv.fed_seed(Some(arch.path.clone()), arch.data.clone()));
    assert!(mv.fed_place(mep).is_ok());
    mv.fed_rebuild(true);
    mv.shading = Shading::Edges;
    mv.grid = true;
    mv.look(StandardView::Iso);
    match what {
        What::Models => {
            mv.bim.fed.by_model = true;
            mv.fed_rebuild(false);
            mv.bim.panel = Some(PanelKind::Models);
            mv.bim.row = 1;
        }
        What::Clash => {
            mv.bim.panel = Some(PanelKind::Checks);
            let c = &mut mv.bim.checks;
            c.tab = super::super::checks::Tab::Clashes;
            c.setup.model_a = Some(0);
            c.setup.model_b = Some(1);
            let found = cross_clashes(mv);
            mv.checks_clashes_found(found, 3.0, None);
            mv.bim.row = 0;
            assert!(mv.checks_enter());
        }
    }
    mv.finish_anim();
}

/// What the clash test finds between the two models (as `checks_run_clashes` runs it).
fn cross_clashes(mv: &MeshView) -> Vec<clash::Clash> {
    let fed = &mv.bim.fed;
    let full = fed.full.as_ref().unwrap();
    let ifc = full.ifc.as_ref().unwrap();
    let member = |i: usize| fed.model_of(i);
    let side = |k: usize| {
        Scoped {
            model: Some(k),
            set: Selector::All,
        }
        .pick(&ifc.elements, &member, &|_| None)
    };
    let m = clash::Model {
        positions: &full.mesh.positions,
        indices: &full.mesh.indices,
        elements: &ifc.elements,
    };
    clash::run(
        &m,
        &side(0),
        &side(1),
        Mode::Hard { tolerance: 0.01 },
        &|_, _| false,
    )
}

fn shoot(
    h: &mut Harness,
    goldens: &mut Goldens,
    name: &str,
    what: What,
    rtl: bool,
    scale: f32,
    pal: &Palette,
) {
    let size_px = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let mut mv = MeshView {
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: size_px.0 as f32,
            h: size_px.1 as f32,
        }),
        ..Default::default()
    };
    setup(&mut mv, what);
    let panel = mv.panel().expect("the panel");
    let mut drawn = None;
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            App::mesh_render(&mut mv, gpu, enc, target, size, scale);
        },
        |dl, text| {
            drawn = Some(draw_panel(
                dl,
                text,
                pal,
                &panel,
                (0.0, 0.0, SIZE.0, SIZE.1),
                rtl,
                1.0,
            ));
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
    crate::app::a11y_audit::audit_screen(name, SIZE, rtl, |t| {
        if let Some((pr, rows, _, _)) = &drawn {
            a11y(t, &panel, *pr, rows, 1.0);
        }
    });
}

#[test]
fn models_panel_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-federation",
    );
    // File names, element names and CRS names are data in any language.
    for d in [
        "architecture.ifc",
        "services.ifc",
        "EPSG:25832",
        "IfcWall",
        "IfcSlab",
        "IfcDuctSegment",
        "IfcPipeSegment",
        "South",
        "North",
        "West",
        "Slab",
        "Supply duct",
        "Drain",
        "Ground floor",
        "2HR",
        "1HR",
    ] {
        h.data_text(d);
    }
    let dark = Palette::dark();
    let light = Palette::light();
    for (name, what) in [("fed-models", What::Models), ("fed-clash", What::Clash)] {
        for scale in [1.0, 1.5] {
            shoot(&mut h, &mut goldens, name, what, false, scale, &dark);
        }
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        for (name, what) in [
            ("fed-models-he-light", What::Models),
            ("fed-clash-he-light", What::Clash),
        ] {
            for scale in [1.0, 1.5] {
                shoot(&mut h, &mut goldens, name, what, true, scale, &light);
            }
        }
    });
    goldens.finish();
}

#[test]
fn a_scene_of_two_models() {
    let mut mv = MeshView::default();
    setup(&mut mv, What::Models);
    let fed = &mv.bim.fed;
    assert!(fed.active());
    assert_eq!(fed.models.len(), 2);
    assert!(matches!(fed.models[1].basis, Basis::Map { crs: Some(ref c) } if c == "EPSG:25832"));
    // The duct crosses the south wall: x ≈ 6, y from −3 to 1 in the architecture's frame.
    let full = fed.full.as_ref().unwrap();
    let ifc = full.ifc.as_ref().unwrap();
    let duct = ifc
        .elements
        .iter()
        .position(|e| e.kind == "IFCDUCTSEGMENT")
        .unwrap();
    let (lo, hi) = super::super::checks::bounds_of(&full.mesh, &ifc.elements, &[duct]).unwrap();
    assert!(
        (lo[0] - 5.8).abs() < 1e-3 && (hi[0] - 6.2).abs() < 1e-3,
        "{lo:?} {hi:?}"
    );
    assert!(
        (lo[1] + 3.0).abs() < 1e-3 && (hi[1] - 1.0).abs() < 1e-3,
        "{lo:?} {hi:?}"
    );
    // One clash between the models: the duct through the south wall (the drain is clear).
    let found = cross_clashes(&mv);
    assert_eq!(found.len(), 1, "{found:?}");
    let pair = [found[0].a, found[0].b].map(|i| ifc.elements[i].name.clone());
    assert!(pair.contains(&"South".to_owned()) && pair.contains(&"Supply duct".to_owned()));
    // Its sidecar keys and its labels name the models.
    mv.bim.checks.tab = super::super::checks::Tab::Clashes;
    mv.checks_clashes_found(found, 1.0, None);
    let keys: Vec<String> = mv
        .bim
        .checks
        .sidecar
        .clashes
        .iter()
        .flat_map(|k| [k.a.clone(), k.b.clone()])
        .collect();
    assert!(
        keys.contains(&"services.ifc#2DuctAAAAAAAAAAAAAAAAA".to_owned()),
        "{keys:?}"
    );
    let v = mv.checks_panel();
    assert!(v.rows[0].text.contains("services.ifc") && v.rows[0].text.contains("architecture.ifc"));
    // The BCF topic names both files, and reads back the same.
    let bytes = mv
        .checks_bytes(
            super::super::checks::Out::Bcf,
            Some("architecture.ifc".into()),
        )
        .unwrap();
    let a = specular_bim::bcf::read(&bytes).unwrap();
    let files: Vec<Option<String>> = a.topics[0]
        .files
        .iter()
        .map(|f| f.filename.clone())
        .collect();
    assert_eq!(
        files,
        vec![Some("architecture.ifc".into()), Some("services.ifc".into())]
    );
    // Both projects named; the added file's date from its header (the opened one is read
    // from disk, absent here).
    assert!(a.topics[0].files.iter().all(|f| f.ifc_project.is_some()));
    assert_eq!(
        a.topics[0].files[1].date.as_deref(),
        Some("2026-10-25T00:00:00")
    );
    // Hiding the services model drops its triangles from the drawn mesh, not from the scene.
    let shown = mv.model.as_ref().unwrap().mesh.triangle_count();
    mv.bim.fed.models[1].visible = false;
    mv.fed_rebuild(false);
    assert!(mv.model.as_ref().unwrap().mesh.triangle_count() < shown);
    assert_eq!(
        mv.bim.fed.full.as_ref().unwrap().mesh.triangle_count(),
        full_triangles(&mv)
    );
    // Removing it leaves the opened model alone again.
    mv.bim.fed.models.remove(1);
    mv.fed_rebuild(false);
    assert!(!mv.bim.fed.active());
    assert!(mv.bim.fed.models.is_empty());
}

fn full_triangles(mv: &MeshView) -> usize {
    mv.bim
        .fed
        .models
        .iter()
        .map(|m| m.own.mesh.triangle_count())
        .sum()
}

#[test]
fn models_that_cannot_be_aligned_say_why() {
    let mut mv = MeshView::default();
    let arch = loaded(
        "architecture.ifc",
        &mapped(ARCH, 500_000.0, 6_000_000.0, 0.0),
    );
    mv.model = Some(arch.model.clone());
    assert!(mv.fed_seed(Some(arch.path.clone()), arch.data.clone()));
    // Not georeferenced.
    let plain = loaded("services.ifc", &ifc(MEP));
    let Err(e) = mv.fed_place(plain) else {
        panic!("placed");
    };
    let why = e.1;
    assert_eq!(
        why,
        t!("fed-unaligned", why = "only-first", first = "", this = "")
    );
    // The step and date of the header.
    assert_eq!(step_date(&ifc(MEP)).as_deref(), Some("2026-10-25T00:00:00"));
}

#[test]
fn ids_and_changes_per_model() {
    let mut mv = MeshView::default();
    setup(&mut mv, What::Models);
    // The services model picked: its elements only, from where it starts in the scene.
    mv.bim.checks.pick = 1;
    mv.bim.checks.data = mv.bim.fed.models[1].data.clone();
    let els = mv.checks_elements().unwrap();
    assert_eq!(els.len(), 2);
    assert!(els.iter().all(|e| e.kind != "IFCWALL"));
    assert_eq!(mv.checks_offset(), mv.bim.fed.models[1].elements.start);
    // Changes against an older services model with the drain removed: one removal, drawn
    // in the scene's frame.
    let old_bytes = mapped(
        &[
            MEP[0],
            (
                "IFCPIPESEGMENT",
                "2PipeBBBBBBBBBBBBBBBBB",
                "Old",
                [0.0; 3],
                [0.1; 3],
                "",
            ),
        ],
        500_006.0,
        5_999_998.0,
        90.0,
    );
    mv.bim.checks.other = Some((
        PathBuf::from("services-old.ifc"),
        crate::app::mesh::checks::shots::other_bytes(&old_bytes),
    ));
    mv.checks_compare();
    let ch = mv.bim.checks.changes.as_ref().unwrap();
    let [added, removed, _] = specular_bim::diff::counts(ch);
    assert_eq!((added, removed), (1, 1));
    // The coloured view is in the scene's frame: the duct still at x ≈ 6.
    let shown = mv.bim.restyled.as_ref().unwrap();
    let xs: Vec<f32> = shown.mesh.positions.iter().map(|p| p[0]).collect();
    assert!(xs.iter().any(|x| (x - 6.2).abs() < 1e-3));
}
