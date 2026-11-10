//! Screenshot tests of reviewing a model: walking (its readout), the section box with its
//! hatched caps and handles, and the BCF topics panel, at 1× and 1.5×, in English and
//! right to left in Hebrew. A small two-room building in metres, drawn by the app's mesh
//! path on lavapipe and compared with the PNGs in `app/specular/tests/shots/`; the panel,
//! readout and handles are audited for screen readers.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular bim::shots`, then review and
//! commit the rewritten PNGs.

use std::sync::Arc;

use specular_bim::bcf::{Comment, Topic};
use specular_decode::specular_mesh::{Material, Mesh, Part, Unit};
use specular_render::PxRect;
use specular_render_3d::{StandardView, canonical_bounds};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use super::*;
use crate::app::medical::draw_readout;
use crate::app::mesh::readout;

const SIZE: (f32, f32) = (520.0, 340.0);

/// A closed box from `lo` to `hi`, faces outward, as part of `material`.
fn cuboid(
    pos: &mut Vec<[f32; 3]>,
    idx: &mut Vec<u32>,
    parts: &mut Vec<Part>,
    lo: [f32; 3],
    hi: [f32; 3],
    material: u32,
) {
    let [x0, y0, z0] = lo;
    let [x1, y1, z1] = hi;
    let quads = [
        [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]],
        [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
        [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
        [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
        [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
        [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]],
    ];
    let first = (idx.len() / 3) as u32;
    for q in quads {
        let b = pos.len() as u32;
        pos.extend_from_slice(&q);
        idx.extend([b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    parts.push(Part {
        first,
        count: 12,
        material: Some(material),
    });
}

/// Two rooms on a slab, a step at the door, walls of brick and concrete.
fn building() -> MeshModel {
    let (mut pos, mut idx, mut parts) = (Vec::new(), Vec::new(), Vec::new());
    let mut c = |lo, hi, m| cuboid(&mut pos, &mut idx, &mut parts, lo, hi, m);
    c([-1.0, -1.0, -0.3], [13.0, 9.0, 0.0], 0);
    c([0.0, 0.0, 0.0], [12.0, 0.25, 2.8], 1);
    c([0.0, 7.75, 0.0], [12.0, 8.0, 2.8], 1);
    c([0.0, 0.25, 0.0], [0.25, 7.75, 2.8], 1);
    c([11.75, 0.25, 0.0], [12.0, 7.75, 2.8], 1);
    c([6.0, 0.25, 0.0], [6.2, 3.0, 2.8], 2);
    c([6.0, 4.2, 0.0], [6.2, 7.75, 2.8], 2);
    c([7.0, 5.0, 0.0], [9.0, 7.0, 0.2], 2);
    let normals = vec![[0.0, 0.0, 1.0]; pos.len()];
    let mut bounds = specular_decode::specular_mesh::Bounds::EMPTY;
    for p in &pos {
        bounds.add(*p);
    }
    let mat = |name: &str, c: [f32; 4]| Material {
        name: name.into(),
        base_color: c,
        texture: None,
    };
    let mesh = Mesh {
        positions: pos,
        normals,
        indices: idx,
        parts,
        materials: vec![
            mat("Slab", [0.35, 0.36, 0.38, 1.0]),
            mat("Brick", [0.55, 0.17, 0.09, 1.0]),
            mat("Concrete", [0.62, 0.62, 0.58, 1.0]),
        ],
        unit: Unit::Metre,
        bounds,
        flat: true,
        ..Mesh::default()
    };
    MeshModel {
        mesh: Arc::new(mesh),
        textures: Arc::new(Vec::new()),
        ifc: None,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum What {
    Walk,
    SectionBox,
    Topics,
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    what: What,
    rtl: bool,
    scale: f32,
}

fn view(model: &MeshModel, scale: f32) -> MeshView {
    let size_px = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let mut mv = MeshView {
        bounds: canonical_bounds(&model.mesh),
        model: Some(model.clone()),
        shading: Shading::Edges,
        grid: true,
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: size_px.0 as f32,
            h: size_px.1 as f32,
        }),
        ..Default::default()
    };
    mv.look(StandardView::Iso);
    mv
}

fn archive() -> Bcf {
    let topic =
        |title: &str, status: &str, prio: Option<&str>, who: Option<&str>, n: usize| Topic {
            guid: specular_bim::ids::new_guid(),
            title: title.into(),
            status: Some(status.into()),
            priority: prio.map(Into::into),
            assigned_to: who.map(Into::into),
            creation_date: "2026-10-25T09:30:00Z".into(),
            creation_author: "marc".into(),
            comments: (0..n)
                .map(|i| Comment {
                    guid: specular_bim::ids::new_guid(),
                    date: "2026-10-25T09:31:00Z".into(),
                    author: if i % 2 == 0 { "marc" } else { "dana" }.into(),
                    text: if i % 2 == 0 {
                        "The door is 80 cm; the plan says 90 cm."
                    } else {
                        "Widened in the next revision."
                    }
                    .into(),
                    ..Comment::default()
                })
                .collect(),
            ..Topic::default()
        };
    Bcf {
        path: Some(PathBuf::from("review-2026-10.bcfzip")),
        archive: Archive {
            version: Version::V3_0,
            version_id: "3.0".into(),
            topics: vec![
                topic("Door too narrow", "Open", Some("High"), Some("dana"), 2),
                topic("Step at the entrance", "In Progress", None, Some("marc"), 1),
                topic("Wall finish missing", "Closed", Some("Low"), None, 0),
            ],
            ..Archive::default()
        },
        dirty: true,
        viewpoint: 0,
    }
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, model: &MeshModel, c: &Case) {
    let mut mv = view(model, c.scale);
    match c.what {
        What::Walk => {
            mv.bim.scene = Some(Arc::new(NavScene::from_mesh(&model.mesh)));
            let (w, hh) = mv.size();
            let body = Body::for_model(&mv.bounds, Some(1.0));
            let cam = Camera::first_person([3.0, 1.2, 1.6], 0.55, -0.08, NAV_FOV);
            let mut nav = Nav::start(NavMode::Walk, &cam, &mv.bounds, w, hh, body);
            nav.pos = [2.0, 1.5, 0.5];
            nav.yaw = 0.62;
            nav.pitch = -0.05;
            nav.settle(mv.bim.scene.as_deref());
            mv.camera = nav.camera(NAV_FOV);
            mv.bim.nav = Some(nav);
        }
        What::SectionBox => {
            mv.bim.scene = Some(Arc::new(NavScene::from_mesh(&model.mesh)));
            mv.bim.sections.section_box = Some(Bounds {
                min: [1.5, -0.5, -0.5],
                max: [10.5, 8.5, 1.4],
            });
            mv.bim.chosen = Some(Handle::BoxFace(4));
        }
        What::Topics => {
            mv.bim.bcf = Some(archive());
            mv.bim.panel = Some(PanelKind::Topics);
            mv.bim.row = 0;
        }
    }
    let mut lines = readout(&model.mesh, &mv.bounds, mv.shading, mv.projection, mv.grid);
    lines.extend(mv.review_lines());
    let panel = mv.panel();
    let pal = Palette::dark();
    let rtl = c.rtl;
    let mut readout_rect = None;
    let mut drawn = None;
    let overlay = mv.overlay(f64::from(c.scale));
    let shot = h.render_gpu(
        SIZE,
        c.scale,
        |gpu, enc, target, size| {
            App::mesh_render(&mut mv, gpu, enc, target, size, c.scale);
        },
        |dl, text| {
            if let Some(o) = &overlay {
                draw_overlay(dl, &pal, o, 1.0);
            }
            readout_rect = Some(draw_readout(
                dl,
                text,
                &lines,
                (0.0, 0.0, SIZE.0, SIZE.1),
                rtl,
                1.0,
            ));
            if let Some(p) = &panel {
                drawn = Some(draw_panel(
                    dl,
                    text,
                    &pal,
                    p,
                    (0.0, 0.0, SIZE.0, SIZE.1),
                    rtl,
                    1.0,
                ));
            }
        },
    );
    if c.what == What::SectionBox {
        let caps = mv.bim.caps.as_ref().expect("caps computed");
        assert!(caps.loops.iter().all(|l| l.closed), "watertight caps");
        assert!(caps.mesh.is_some(), "caps drawn");
    }
    goldens.check(&format!("{}-{}x", c.name, c.scale), &shot);
    let r = readout_rect.expect("the readout drawn");
    let handles = overlay.map(|o| o.handles).unwrap_or_default();
    crate::app::a11y_audit::audit_screen(c.name, SIZE, rtl, |t| {
        crate::app::mesh::a11y(t, &lines, r, 1.0);
        if let (Some(p), Some((pr, rows, ..))) = (&panel, &drawn) {
            a11y(t, p, *pr, rows, 1.0);
        }
        handle_nodes(t, &handles, rtl, 1.0);
    });
}

#[test]
fn review_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-review",
    );
    let model = building();
    // The topics' own words (statuses, priorities, names) are data, in any language.
    for d in [
        "Open",
        "In Progress",
        "Closed",
        "High",
        "Low",
        "marc",
        "dana",
    ] {
        h.data_text(d);
    }
    let cases = [
        ("review-walk", What::Walk),
        ("review-section-box", What::SectionBox),
        ("review-bcf-topics", What::Topics),
    ];
    for (name, what) in cases {
        for scale in [1.0, 1.5] {
            let c = Case {
                name,
                what,
                rtl: false,
                scale,
            };
            shoot(&mut h, &mut goldens, &model, &c);
        }
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        for (name, what) in [
            ("review-walk-he", What::Walk),
            ("review-section-box-he", What::SectionBox),
            ("review-bcf-topics-he", What::Topics),
        ] {
            for scale in [1.0, 1.5] {
                let c = Case {
                    name,
                    what,
                    rtl: true,
                    scale,
                };
                shoot(&mut h, &mut goldens, &model, &c);
            }
        }
    });
    goldens.finish();
}
