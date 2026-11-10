//! Screenshot tests of the 3D view: a torus knot in millimetres drawn by the app's mesh path
//! (the camera fitted to the image area, standard views, every shading, both projections,
//! the ground grid) with its readout, dark and light at 1× and 1.5×, and right to left in
//! Hebrew. Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular mesh::shots`, then review and
//! commit the rewritten PNGs.

use std::sync::Arc;

use specular_decode::specular_mesh::{Unit, fixtures};
use specular_shots::{Goldens, Harness};

use super::*;
use crate::app::medical::draw_readout;

const SIZE: (f32, f32) = (480.0, 300.0);

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    view: StandardView,
    /// Turned from the view by this many key presses (left/right, up/down).
    turn: (i32, i32),
    shading: Shading,
    projection: Projection,
    grid: bool,
    rtl: bool,
    scale: f32,
}

fn model() -> MeshModel {
    let mut mesh = fixtures::torus_knot(160, 24);
    mesh.unit = Unit::Millimetre;
    MeshModel {
        mesh: Arc::new(mesh),
        textures: Arc::new(Vec::new()),
        ifc: None,
    }
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, model: &MeshModel, c: &Case) {
    let size_px = (
        (SIZE.0 * c.scale).round() as u32,
        (SIZE.1 * c.scale).round() as u32,
    );
    let mut mv = MeshView {
        bounds: canonical_bounds(&model.mesh),
        model: Some(model.clone()),
        shading: c.shading,
        projection: c.projection,
        grid: c.grid,
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: size_px.0 as f32,
            h: size_px.1 as f32,
        }),
        ..Default::default()
    };
    mv.camera.projection = c.projection;
    mv.look(c.view);
    mv.camera.orbit(
        f64::from(c.turn.0) * ORBIT_KEY,
        f64::from(c.turn.1) * ORBIT_KEY,
    );
    let lines = readout(&model.mesh, &mv.bounds, mv.shading, mv.projection, mv.grid);
    let rtl = c.rtl;
    let mut r = None;
    let shot = h.render_gpu(
        SIZE,
        c.scale,
        |gpu, enc, target, size| {
            App::mesh_render(&mut mv, gpu, enc, target, size, c.scale);
        },
        |dl, text| {
            r = Some(draw_readout(
                dl,
                text,
                &lines,
                (0.0, 0.0, SIZE.0, SIZE.1),
                rtl,
                1.0,
            ));
        },
    );
    goldens.check(&format!("{}-{}x", c.name, c.scale), &shot);
    audit(c.name, &lines, r, rtl);
}

/// The screen-reader audit of the readout drawn at `r`.
fn audit(name: &str, lines: &[String], r: Option<Rect>, rtl: bool) {
    let r = r.expect("the readout drawn");
    super::super::a11y_audit::audit_screen(name, SIZE, rtl, |t| a11y(t, lines, r, 1.0));
}

#[test]
fn mesh_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mesh",
    );
    let model = model();
    let base = Case {
        name: "",
        view: StandardView::Iso,
        turn: (0, 0),
        shading: Shading::Shaded,
        projection: Projection::Perspective,
        grid: true,
        rtl: false,
        scale: 1.0,
    };
    let cases = [
        Case {
            name: "mesh-iso-shaded",
            ..base
        },
        Case {
            name: "mesh-front-edges-ortho",
            view: StandardView::Front,
            shading: Shading::Edges,
            projection: Projection::Orthographic,
            scale: 1.5,
            ..base
        },
        Case {
            name: "mesh-turned-wireframe",
            turn: (2, -1),
            shading: Shading::Wireframe,
            ..base
        },
        Case {
            name: "mesh-top-xray-nogrid",
            view: StandardView::Top,
            shading: Shading::XRay,
            grid: false,
            ..base
        },
    ];
    for c in &cases {
        shoot(&mut h, &mut goldens, &model, c);
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let c = Case {
            name: "mesh-side-he",
            view: StandardView::Side,
            rtl: true,
            ..base
        };
        shoot(&mut h, &mut goldens, &model, &c);
    });
    goldens.finish();
}

/// A LandXML survey's 3D page (the CAD shots' fixture): its TIN coloured by elevation with
/// contours, and the road draped over it; English at 1×, Hebrew at 1.5×.
#[test]
fn survey_model_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mesh",
    );
    let s = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::from_vec(
            "survey.xml",
            crate::app::cad::shots::SURVEY.as_bytes().to_vec(),
        )))
        .expect("opens");
    let page = s
        .open_page(specular_decode::LANDXML_MODEL_PAGE)
        .expect("a 3D page");
    let model = page.mesh().cloned().expect("a model");
    let base = Case {
        name: "landxml-model",
        view: StandardView::Iso,
        turn: (0, 0),
        shading: Shading::Shaded,
        projection: Projection::Perspective,
        grid: true,
        rtl: false,
        scale: 1.0,
    };
    shoot(&mut h, &mut goldens, &model, &base);
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let c = Case {
            name: "landxml-model-he",
            rtl: true,
            scale: 1.5,
            ..base
        };
        shoot(&mut h, &mut goldens, &model, &c);
    });
    goldens.finish();
}

/// A lidar-like terrain (LAS, point format 3) read whole.
fn cloud() -> PointsModel {
    use specular_decode::specular_points::{Limits, PointFormat, fixtures, open};
    let bytes = fixtures::terrain_las(60.0, 10.0);
    let (loading, job) = open(PointFormat::Las, &bytes, &Limits::default()).unwrap();
    if let Some(job) = job {
        job.run(&bytes, &loading);
    }
    PointsModel { loading }
}

/// A cloud through the app's path: fitted, coloured by class with eye-dome lighting, with
/// its readout (points, box, look), left to right and right to left in Hebrew.
fn shoot_cloud(h: &mut Harness, goldens: &mut Goldens, name: &str, rtl: bool) {
    let mut mv = MeshView {
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: SIZE.0,
            h: SIZE.1,
        }),
        ..Default::default()
    };
    mv.set_cloud(cloud());
    mv.colour = PointColour::Classification;
    mv.point_size = 3.0;
    mv.look(StandardView::Iso);
    let lines = points_readout(
        &mv.cloud.as_ref().unwrap().loading,
        &mv.bounds,
        mv.point_look(),
    );
    assert!(lines.len() >= 4, "{lines:?}");
    let mut r = None;
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            // Everything is read: one frame uploads it all and draws it.
            assert!(!App::mesh_render(&mut mv, gpu, enc, target, size, 1.0));
        },
        |dl, text| {
            r = Some(draw_readout(
                dl,
                text,
                &lines,
                (0.0, 0.0, SIZE.0, SIZE.1),
                rtl,
                1.0,
            ));
        },
    );
    goldens.check(name, &shot);
    audit(name, &lines, r, rtl);
}

#[test]
fn cloud_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mesh",
    );
    shoot_cloud(&mut h, &mut goldens, "points-terrain-classes", false);
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        shoot_cloud(&mut h, &mut goldens, "points-terrain-he", true);
    });
    goldens.finish();
}

/// The point actions' steps: sizes through the list and back, colourings the cloud has.
#[test]
fn point_size_and_colour_steps() {
    let mut mv = MeshView::default();
    mv.set_cloud(cloud());
    assert_eq!(mv.colour, PointColour::Rgb);
    for _ in 0..20 {
        mv.step_size(true);
    }
    assert_eq!(mv.point_size, 16.0);
    for _ in 0..20 {
        mv.step_size(false);
    }
    assert_eq!(mv.point_size, 1.0);
    mv.step_size(true);
    assert_eq!(mv.point_size, 1.5);
    let ch = mv.cloud.as_ref().unwrap().loading.info.channels;
    let order: Vec<_> = (0..4)
        .map(|_| {
            mv.colour = mv.colour.next_for(&ch);
            mv.colour
        })
        .collect();
    assert_eq!(
        order,
        [
            PointColour::Intensity,
            PointColour::Elevation,
            PointColour::Classification,
            PointColour::Rgb
        ]
    );
}

/// A fixture of specular-mol.
fn molecule_model(name: &str) -> specular_decode::MoleculeModel {
    let path = format!(
        "{}/../../crates/formats/specular-mol/tests/data/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let data = std::fs::read(path).expect("fixture");
    let m = specular_decode::specular_mol::open(
        &data,
        None,
        &specular_decode::specular_mol::Limits::default(),
    )
    .expect(name);
    specular_decode::MoleculeModel {
        molecule: Arc::new(m),
    }
}

/// A view over `SIZE` on `name`, as the app sets it up when the file opens.
fn molecule_view(name: &str) -> MeshView {
    let mut mv = MeshView {
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: SIZE.0,
            h: SIZE.1,
        }),
        ..Default::default()
    };
    let mol = MolView::new(molecule_model(name));
    mv.bounds = mol.scene.bounds;
    mv.mol = Some(mol);
    mv.look(StandardView::Iso);
    mv
}

/// The atom drawn nearest the viewer and its projected centre (physical px).
fn front_atom(mv: &MeshView) -> (usize, (f32, f32)) {
    let mol = mv.mol.as_ref().unwrap();
    let m = mol.molecule();
    let (w, h) = mv.size();
    let p = mv.camera.project(&mv.bounds, w, h, 0.0);
    let (i, (x, y, _)) = mol
        .scene
        .targets
        .iter()
        .filter_map(|&(i, _)| {
            let i = i as usize;
            Some((i, p.to_px(m.atoms[i].pos.map(f64::from), w, h)?))
        })
        .max_by(|a, b| a.1.2.total_cmp(&b.1.2))
        .expect("atoms");
    (i, (x as f32, y as f32))
}

/// Crambin in ball-and-stick with two atoms picked (the first by a ray through its projected
/// centre, as a click does), the distance between them in the readout; left to right and
/// right to left in Hebrew.
fn shoot_molecule(h: &mut Harness, goldens: &mut Goldens, name: &str, rtl: bool) {
    use specular_decode::specular_mol::{Colouring, Style};
    let mut mv = molecule_view("1crn.pdb");
    let mol = mv.mol.as_mut().unwrap();
    assert_eq!(mol.style, Style::Cartoon);
    mol.set_look(Style::BallStick, Colouring::Element);
    let (front, at) = front_atom(&mv);
    let r = mv.rect.unwrap();
    let (o, d) = mv.camera.ray(
        &mv.bounds,
        f64::from(at.0 - r.x),
        f64::from(at.1 - r.y),
        f64::from(r.w),
        f64::from(r.h),
    );
    let mol = mv.mol.as_mut().unwrap();
    let hit = mol.scene.pick(mol.molecule(), o, d).map(|(i, _)| i);
    assert_eq!(hit, Some(front));
    let far = mol.molecule().atoms.len() - 1;
    mol.click(hit);
    mol.click(Some(far));
    let lines = mol.readout(&mv.bounds, mv.projection);
    assert!(lines.iter().any(|l| l.contains('Å')), "{lines:?}");
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            assert!(!App::mesh_render(&mut mv, gpu, enc, target, size, 1.0));
        },
        |dl, text| {
            draw_readout(dl, text, &lines, (0.0, 0.0, SIZE.0, SIZE.1), rtl, 1.0);
        },
    );
    goldens.check(name, &shot);
}

#[test]
fn molecule_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mesh",
    );
    shoot_molecule(&mut h, &mut goldens, "molecule-picked-distance", false);
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        shoot_molecule(&mut h, &mut goldens, "molecule-picked-he", true);
    });
    goldens.finish();
}

/// Clicks: a second atom adds the distance, the last again unpicks it, a third starts over,
/// nothing clears; styles skip the cartoon without a polymer; the readout follows.
#[test]
fn molecule_picks_and_steps() {
    use specular_decode::specular_mol::{Colouring, Style};
    let lines = |mol: &MolView| mol.readout(&Bounds::EMPTY, Projection::Perspective);
    let mut mv = molecule_view("1crn.pdb");
    let mol = mv.mol.as_mut().unwrap();
    let base = lines(mol).len();
    mol.click(Some(1));
    mol.click(Some(5));
    assert_eq!(mol.picks, [1, 5]);
    let l = lines(mol);
    assert_eq!(l.len(), base + 3, "{l:?}");
    assert!(l.iter().any(|s| s.contains("THR A1 CA")), "{l:?}");
    mol.click(Some(5));
    assert_eq!(mol.picks, [1]);
    mol.click(Some(5));
    mol.click(Some(9));
    assert_eq!(mol.picks, [9]);
    mol.click(None);
    assert!(mol.picks.is_empty());
    // Crambin starts as a cartoon coloured by secondary structure.
    assert_eq!((mol.style, mol.colouring), (Style::Cartoon, Colouring::Ss));
    mol.next_style();
    assert_eq!(mol.style, Style::BallStick);
    mol.next_style();
    mol.next_style();
    assert_eq!(mol.style, Style::Cartoon);
    mol.next_colouring();
    assert_eq!(mol.colouring, Colouring::BFactor);
    // A small molecule has no cartoon.
    let mut mv = molecule_view("caffeine.xyz");
    let mol = mv.mol.as_mut().unwrap();
    assert_ne!(mol.style, Style::Cartoon);
    for _ in 0..4 {
        mol.next_style();
        assert_ne!(mol.style, Style::Cartoon);
    }
}

/// A fixture of specular-neuro opened as the app opens it (surfaces beside it are found).
fn neuro_session(name: &str) -> Arc<dyn specular_decode::Session> {
    let path = format!(
        "{}/../../crates/formats/specular-neuro/tests/data/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    specular_decode::Registry::default()
        .open_path(std::path::Path::new(&path))
        .expect(name)
}

/// Three bundles of streamlines in RAS mm, one along each axis: arcs left to right (red),
/// front to back (green) and bottom to top (blue), as the direction colouring shows them.
fn bundles() -> specular_decode::TractsModel {
    let mut t = specular_decode::neuro_dec::Tractogram::default();
    let mut line = |f: &dyn Fn(f32) -> [f32; 3]| {
        t.offsets.push(t.points.len() as u32);
        for k in 0..=40 {
            t.points.push(f(k as f32 / 40.0));
        }
    };
    for i in 0..24 {
        let o = (i % 6) as f32 * 1.6 - 4.0;
        let d = (i / 6) as f32 * 1.6 - 2.4;
        // Left to right, arching up (a callosal fan).
        line(&|s| {
            let x = -60.0 + 120.0 * s;
            [x, o, 20.0 + d + 25.0 * (std::f32::consts::PI * s).sin()]
        });
        // Front to back, low and to the left.
        line(&|s| {
            [
                -25.0 + o,
                -70.0 + 140.0 * s,
                -5.0 + d + 6.0 * (6.0 * s).sin(),
            ]
        });
        // Bottom to top, to the right, fanning out.
        line(&|s| [30.0 + o * (1.0 + 2.0 * s), 10.0 + d, -50.0 + 100.0 * s]);
    }
    t.offsets.push(t.points.len() as u32);
    specular_decode::TractsModel {
        tracts: Arc::new(t),
    }
}

/// A brain surface coloured by a map (cortical thickness on the left hemisphere; the CIFTI
/// labels on both), and a tractogram coloured by direction, with their readouts.
#[test]
fn neuro_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mesh",
    );
    for (name, file) in [
        ("neuro-surface-func", "sub.L.thickness.func.gii"),
        ("neuro-surface-labels", "sub.dlabel.nii"),
    ] {
        let s = neuro_session(file);
        let model = s.mesh().expect("a surface").clone();
        assert!(model.mesh.colors.is_some(), "{file}");
        let mut mv = MeshView {
            bounds: canonical_bounds(&model.mesh),
            model: Some(model.clone()),
            rect: Some(PxRect {
                x: 0.0,
                y: 0.0,
                w: SIZE.0,
                h: SIZE.1,
            }),
            ..Default::default()
        };
        mv.look(StandardView::Front);
        let lines = readout(&model.mesh, &mv.bounds, mv.shading, mv.projection, mv.grid);
        let shot = h.render_gpu(
            SIZE,
            1.0,
            |gpu, enc, target, size| {
                App::mesh_render(&mut mv, gpu, enc, target, size, 1.0);
            },
            |dl, text| {
                draw_readout(dl, text, &lines, (0.0, 0.0, SIZE.0, SIZE.1), false, 1.0);
            },
        );
        goldens.check(name, &shot);
    }
    let tv = TractView::new(bundles());
    let mut mv = MeshView {
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: SIZE.0,
            h: SIZE.1,
        }),
        bounds: tv.bounds,
        tracts: Some(tv),
        ..Default::default()
    };
    mv.look(StandardView::Iso);
    let tv = mv.tracts.as_mut().unwrap();
    tv.step_width(true);
    let lines = tv.readout(mv.projection);
    assert!(lines.iter().any(|l| l.contains("72")), "{lines:?}");
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            assert!(!App::mesh_render(&mut mv, gpu, enc, target, size, 1.0));
        },
        |dl, text| {
            draw_readout(dl, text, &lines, (0.0, 0.0, SIZE.0, SIZE.1), false, 1.0);
        },
    );
    goldens.check("neuro-tracts-direction", &shot);
    goldens.finish();
}

/// A tractogram takes the point size and colour keys while shown, and its grid is its own.
#[test]
fn tract_keys() {
    let mut mv = MeshView::default();
    let tv = TractView::new(bundles());
    mv.bounds = tv.bounds;
    mv.tracts = Some(tv);
    assert!(mv.has_scene());
    let before = mv.tracts.as_ref().unwrap().readout(mv.projection);
    let tv = mv.tracts.as_mut().unwrap();
    assert!(tv.step_width(true));
    assert_ne!(tv.readout(mv.projection), before);
    // No per-point values: the colouring stays by direction.
    tv.next_colour();
    assert_eq!(tv.colour, tracts::TractColour::Direction);
}

/// Gaussian splats through the app's path: fitted, sorted and drawn in one frame, with their
/// readout (box, splats, harmonics, grid, projection).
#[test]
fn splat_view_matches_golden() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mesh",
    );
    use specular_decode::specular_splat::{Limits, fixtures, read_ply};
    let g = fixtures::scene(1500);
    let splats = Arc::new(read_ply(&fixtures::ply_file(&g, 3, false), &Limits::default()).unwrap());
    let mut mv = MeshView {
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: SIZE.0,
            h: SIZE.1,
        }),
        ..Default::default()
    };
    mv.bounds = splats.bounds;
    mv.splats = Some(specular_decode::SplatModel {
        splats: splats.clone(),
    });
    mv.look(StandardView::Iso);
    let lines = splats_readout(&splats, &mv.bounds, mv.grid, mv.projection);
    let plain = |l: &String| l.replace(['\u{2068}', '\u{2069}'], "");
    assert!(
        lines.iter().any(|l| plain(l) == "1,500 splats"),
        "{lines:?}"
    );
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            // Small enough to upload in one frame.
            assert!(!App::mesh_render(&mut mv, gpu, enc, target, size, 1.0));
        },
        |dl, text| {
            draw_readout(dl, text, &lines, (0.0, 0.0, SIZE.0, SIZE.1), false, 1.0);
        },
    );
    goldens.check("splats-ring", &shot);
    goldens.finish();
}

/// A tileset of a coarse cube refined (REPLACE) into four cubes, written to a scratch
/// folder and opened as a user would.
fn tileset_session() -> Arc<dyn specular_decode::Session> {
    let dir = std::env::temp_dir().join(format!("specular-shot-3dtiles-{}", std::process::id()));
    write_cube_b3dm(&dir);
    let child = |x: f64, y: f64| {
        format!(
            r#"{{"boundingVolume":{{"sphere":[{x},{y},0,1.8]}},"geometricError":0,"content":{{"uri":"cube.b3dm"}},
            "transform":[1,0,0,0, 0,1,0,0, 0,0,1,0, {x},{y},0,1]}}"#
        )
    };
    let ts = format!(
        r#"{{"asset":{{"version":"1.0"}},"geometricError":100,"root":{{"boundingVolume":{{"sphere":[0,0,0,6]}},
        "geometricError":100,"refine":"REPLACE","content":{{"uri":"cube.b3dm"}},"children":[{},{},{},{}]}}}}"#,
        child(-2.5, -2.5),
        child(2.5, -2.5),
        child(-2.5, 2.5),
        child(2.5, 2.5)
    );
    std::fs::write(dir.join("tileset.json"), ts).unwrap();
    specular_decode::Registry::default()
        .open_path(&dir.join("tileset.json"))
        .unwrap()
}

/// The fixture's two cubes as `dir/cube.b3dm`.
fn write_cube_b3dm(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    let glb = fixtures::glb();
    let ft = b"{\"BATCH_LENGTH\":0}      ";
    let mut b3dm = Vec::new();
    b3dm.extend_from_slice(b"b3dm");
    for v in [
        1u32,
        (28 + ft.len() + glb.len()) as u32,
        ft.len() as u32,
        0,
        0,
        0,
    ] {
        b3dm.extend_from_slice(&v.to_le_bytes());
    }
    b3dm.extend_from_slice(ft);
    b3dm.extend_from_slice(&glb);
    std::fs::write(dir.join("cube.b3dm"), &b3dm).unwrap();
}

fn shoot_tiles(
    h: &mut Harness,
    goldens: &mut Goldens,
    s: &Arc<dyn specular_decode::Session>,
    name: &str,
    rtl: bool,
) {
    let t = s.tiles3d().cloned().expect("a tileset");
    let mut mv = MeshView {
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: SIZE.0,
            h: SIZE.1,
        }),
        model: s.mesh().cloned(),
        bounds: t.bounds(),
        tiles: Some(tiles3d::TilesView::new(t.clone())),
        ..Default::default()
    };
    mv.look(StandardView::Iso);
    tiles3d::pass_now(&mut mv);
    let m = mv.model.clone().unwrap();
    // Four copies in place of the coarse one.
    let one = t
        .scene(&[0], &specular_core::CancelToken::new())
        .unwrap()
        .mesh
        .unwrap()
        .mesh
        .triangle_count();
    assert_eq!(m.mesh.triangle_count(), 4 * one);
    let mut lines = readout(&m.mesh, &mv.bounds, mv.shading, mv.projection, mv.grid);
    lines.extend(mv.tiles.as_ref().unwrap().lines(t.tiles_read()));
    let plain = |l: &String| l.replace(['\u{2068}', '\u{2069}'], "");
    if !rtl {
        assert!(
            lines.iter().any(|l| plain(l) == "4 tiles drawn of 5 read"),
            "{lines:?}"
        );
    }
    let mut r = None;
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            App::mesh_render(&mut mv, gpu, enc, target, size, 1.0);
        },
        |dl, text| {
            r = Some(draw_readout(
                dl,
                text,
                &lines,
                (0.0, 0.0, SIZE.0, SIZE.1),
                rtl,
                1.0,
            ));
        },
    );
    goldens.check(name, &shot);
    audit(name, &lines, r, rtl);
}

#[test]
fn tiles3d_view_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mesh",
    );
    let s = tileset_session();
    shoot_tiles(&mut h, &mut goldens, &s, "tiles3d-refined", false);
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        shoot_tiles(&mut h, &mut goldens, &s, "tiles3d-refined-he", true);
    });
    goldens.finish();
    let dir = std::env::temp_dir().join(format!("specular-shot-3dtiles-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(dir);
}

/// A tileset of the cubes (root, ADD) and a red sheet of points above them (child).
fn mixed_tileset_session(dir: &std::path::Path) -> Arc<dyn specular_decode::Session> {
    write_cube_b3dm(dir);
    let n = 41usize;
    let mut pos = Vec::new();
    for j in 0..n {
        for i in 0..n {
            for v in [i as f32 * 0.15 - 3.0, j as f32 * 0.15 - 3.0, 3.0f32] {
                pos.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    let rgb_at = pos.len();
    let mut bin = pos;
    for _ in 0..n * n {
        bin.extend_from_slice(&[230, 40, 40]);
    }
    while bin.len() % 8 != 0 {
        bin.push(0);
    }
    let mut ft = format!(
        r#"{{"POINTS_LENGTH":{},"POSITION":{{"byteOffset":0}},"RGB":{{"byteOffset":{rgb_at}}}}}"#,
        n * n
    )
    .into_bytes();
    while (28 + ft.len()) % 8 != 0 {
        ft.push(b' ');
    }
    let mut pnts = b"pnts".to_vec();
    for v in [
        1u32,
        (28 + ft.len() + bin.len()) as u32,
        ft.len() as u32,
        bin.len() as u32,
        0,
        0,
    ] {
        pnts.extend_from_slice(&v.to_le_bytes());
    }
    pnts.extend_from_slice(&ft);
    pnts.extend_from_slice(&bin);
    std::fs::write(dir.join("sheet.pnts"), &pnts).unwrap();
    let ts = r#"{"asset":{"version":"1.0"},"geometricError":100,"root":{"boundingVolume":{"sphere":[0,0,1,6]},
        "geometricError":100,"refine":"ADD","content":{"uri":"cube.b3dm"},
        "children":[{"boundingVolume":{"sphere":[0,0,3,5]},"geometricError":0,"content":{"uri":"sheet.pnts"}}]}}"#;
    std::fs::write(dir.join("tileset.json"), ts).unwrap();
    specular_decode::Registry::default()
        .open_path(&dir.join("tileset.json"))
        .unwrap()
}

#[test]
fn tiles3d_models_and_points_both_shown() {
    let dir = std::env::temp_dir().join(format!(
        "specular-shot-3dtiles-mixed-{}",
        std::process::id()
    ));
    let s = mixed_tileset_session(&dir);
    let t = s.tiles3d().cloned().expect("a tileset");
    assert!(
        s.mesh().is_some() && s.points().is_some(),
        "the first scene has both"
    );
    let mut mv = MeshView {
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: SIZE.0,
            h: SIZE.1,
        }),
        model: s.mesh().cloned(),
        cloud: s.points().cloned(),
        bounds: t.bounds(),
        tiles: Some(tiles3d::TilesView::new(t.clone())),
        ..Default::default()
    };
    mv.look(StandardView::Iso);
    tiles3d::pass_now(&mut mv);
    assert!(mv.model.is_some() && mv.cloud.is_some());
    let lines = mv.tiles.as_ref().unwrap().lines(t.tiles_read());
    let plain = |l: &String| l.replace(['\u{2068}', '\u{2069}'], "");
    assert!(
        lines.iter().any(|l| plain(l) == "2 tiles drawn of 2 read"),
        "{lines:?}"
    );
    // Walking collides with the cubes and the points.
    mv.bim.nav = Some(specular_render_3d::Nav::start(
        specular_render_3d::NavMode::Walk,
        &mv.camera,
        &mv.bounds,
        f64::from(SIZE.0),
        f64::from(SIZE.1),
        specular_render_3d::Body::for_model(&mv.bounds, None),
    ));
    mv.geometry_changed();
    mv.collision_wait();
    let tris = mv.model.as_ref().unwrap().mesh.triangle_count();
    assert_eq!(mv.collision_len(), Some(tris + 41 * 41));
    mv.bim.nav = None;
    if let Some(mut h) = Harness::new() {
        let mut goldens = Goldens::new(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
            "specular-mesh",
        );
        let shot = h.render_gpu(
            SIZE,
            1.0,
            |gpu, enc, target, size| {
                App::mesh_render(&mut mv, gpu, enc, target, size, 1.0);
            },
            |_, _| {},
        );
        goldens.check("tiles3d-mixed", &shot);
        goldens.finish();
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Walking collides with the finest tiles near the walker, not the coarse ones drawn; the
/// scene is built again when the drawn tiles change.
#[test]
fn tiles3d_walking_collides_with_the_finest_tiles_near() {
    let s = tileset_session();
    let t = s.tiles3d().cloned().expect("a tileset");
    let mut mv = MeshView {
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: SIZE.0,
            h: SIZE.1,
        }),
        // The coarse root alone first.
        model: t
            .scene(&[0], &specular_core::CancelToken::new())
            .unwrap()
            .mesh,
        bounds: t.bounds(),
        tiles: Some(tiles3d::TilesView::new(t.clone())),
        ..Default::default()
    };
    mv.look(StandardView::Iso);
    let one = mv.model.as_ref().unwrap().mesh.triangle_count();
    mv.bim.nav = Some(specular_render_3d::Nav::start(
        specular_render_3d::NavMode::Walk,
        &mv.camera,
        &mv.bounds,
        f64::from(SIZE.0),
        f64::from(SIZE.1),
        specular_render_3d::Body::for_model(&mv.bounds, None),
    ));
    mv.geometry_changed();
    mv.collision_wait();
    // The whole little tileset is within reach: its four fine tiles, though the coarse
    // root is drawn.
    assert_eq!(mv.collision_len(), Some(4 * one));
    tiles3d::pass_now(&mut mv);
    assert_eq!(mv.model.as_ref().unwrap().mesh.triangle_count(), 4 * one);
    // The old scene until the new one is there.
    assert_eq!(mv.collision_len(), Some(4 * one));
    mv.collision_wait();
    assert_eq!(mv.collision_len(), Some(4 * one));
}
