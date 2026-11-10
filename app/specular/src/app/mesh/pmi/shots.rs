//! Screenshot tests of a CAD model's PMI: labels with leaders, a feature control frame with
//! its symbol, a datum feature symbol, the list with the chosen annotation's faces lit, at
//! 1× and 1.5×, in English (dark) and right to left in Hebrew (light). A plate with a boss,
//! drawn by the app's mesh path on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`; the panel is audited for screen readers.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular pmi::shots`, then review and
//! commit the rewritten PNGs.

use std::sync::Arc;

use specular_decode::MeshModel;
use specular_decode::pmi::{Characteristic, DimKind, Label, Pmi, PmiItem, PmiKind, PmiView};
use specular_decode::specular_mesh::{Bounds, Material, Mesh, Part, Unit};
use specular_gfx::DrawList;
use specular_render::PxRect;
use specular_render_3d::{Shading, StandardView, canonical_bounds};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use super::super::MeshView;
use super::super::bim::{PanelKind, a11y, draw_panel};
use super::draw_overlay;
use crate::app::App;

const SIZE: (f32, f32) = (760.0, 420.0);

/// A closed box from `lo` to `hi`, faces outward; returns its first triangle.
fn cuboid(pos: &mut Vec<[f32; 3]>, idx: &mut Vec<u32>, lo: [f32; 3], hi: [f32; 3]) -> u32 {
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
    first
}

/// A 100 × 60 × 10 mm plate with a 20 mm boss on top, and its PMI.
fn part() -> (MeshModel, Pmi) {
    let (mut pos, mut idx) = (Vec::new(), Vec::new());
    let plate = cuboid(&mut pos, &mut idx, [0.0, 0.0, 0.0], [100.0, 60.0, 10.0]);
    let boss = cuboid(&mut pos, &mut idx, [60.0, 20.0, 10.0], [80.0, 40.0, 30.0]);
    let mut bounds = Bounds::EMPTY;
    for p in &pos {
        bounds.add(*p);
    }
    let mesh = Mesh {
        normals: vec![[0.0, 0.0, 1.0]; pos.len()],
        positions: pos,
        parts: vec![Part {
            first: 0,
            count: (idx.len() / 3) as u32,
            material: Some(0),
        }],
        indices: idx,
        materials: vec![Material {
            name: "Steel".into(),
            base_color: [0.55, 0.57, 0.6, 1.0],
            texture: None,
        }],
        unit: Unit::Millimetre,
        bounds,
        flat: true,
        ..Mesh::default()
    };
    let item = |kind, label: Label, text: &str, anchor, target, tris: Vec<(u32, u32)>| PmiItem {
        kind,
        dim: (kind == PmiKind::Dimension).then_some(DimKind::Linear),
        characteristic: match &label {
            Label::Frame { characteristic, .. } => *characteristic,
            _ => None,
        },
        name: String::new(),
        label,
        text: text.into(),
        anchor: Some(anchor),
        target: Some(target),
        polylines: Vec::new(),
        triangles: tris,
        bounds,
    };
    let pmi = Pmi {
        items: vec![
            item(
                PmiKind::Dimension,
                Label::Text("100 ±0.2".into()),
                "100 ±0.2",
                [50.0, -18.0, 0.0],
                [50.0, 0.0, 0.0],
                vec![],
            ),
            item(
                PmiKind::GeometricTolerance,
                Label::Frame {
                    characteristic: Some(Characteristic::Position),
                    cells: vec!["Ø0.25 Ⓜ".into(), "A".into(), "B".into(), "C".into()],
                },
                "Ø0.25 Ⓜ | A | B | C",
                [0.0, -10.0, 60.0],
                [70.0, 30.0, 30.0],
                vec![(boss, 12)],
            ),
            item(
                PmiKind::GeometricTolerance,
                Label::Frame {
                    characteristic: Some(Characteristic::Flatness),
                    cells: vec!["0.05".into()],
                },
                "0.05",
                [-10.0, 10.0, 30.0],
                [15.0, 55.0, 10.0],
                vec![(plate + 2, 2)],
            ),
            item(
                PmiKind::Datum,
                Label::Datum("A".into()),
                "A",
                [-14.0, 30.0, 0.0],
                [0.0, 30.0, 5.0],
                vec![(plate, 2)],
            ),
        ],
        views: vec![PmiView {
            name: "MBD_A".into(),
            direction: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            orthographic: true,
            items: vec![0, 3],
        }],
    };
    let model = MeshModel {
        mesh: Arc::new(mesh),
        textures: Arc::new(Vec::new()),
        ifc: None,
    };
    (model, pmi)
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, name: &str, rtl: bool, scale: f32) {
    let (model, pmi) = part();
    let px = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let mut mv = MeshView {
        bounds: canonical_bounds(&model.mesh),
        model: Some(model),
        shading: Shading::Edges,
        grid: true,
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: px.0 as f32,
            h: px.1 as f32,
        }),
        ..Default::default()
    };
    mv.look(StandardView::Iso);
    mv.pmi.data = Some(Arc::new(pmi));
    mv.bim.panel = Some(PanelKind::Pmi);
    // The position tolerance chosen: the boss lit.
    mv.bim.row = 2;
    mv.pmi_light();
    assert!(mv.bim.restyled.is_some(), "faces lit");
    let pal = if rtl {
        Palette::light()
    } else {
        Palette::dark()
    };
    let panel = mv.panel();
    let overlay = mv.pmi_overlay(f64::from(scale));
    assert_eq!(overlay.as_ref().map(|o| o.labels.len()), Some(4));
    let mut drawn = None;
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            App::mesh_render(&mut mv, gpu, enc, target, size, scale);
        },
        |dl, text| {
            // The panel's place first (as the app has it from the last frame).
            if let Some(p) = &panel {
                drawn = Some(draw_panel(
                    &mut DrawList::new(1.0),
                    text,
                    &pal,
                    p,
                    (0.0, 0.0, SIZE.0, SIZE.1),
                    rtl,
                    1.0,
                ));
            }
            if let Some(o) = &overlay {
                let mut o = o.clone();
                o.avoid = drawn.as_ref().map(|d| d.0);
                draw_overlay(dl, text, &pal, &o, 1.0);
            }
            if let Some(p) = &panel {
                draw_panel(dl, text, &pal, p, (0.0, 0.0, SIZE.0, SIZE.1), rtl, 1.0);
            }
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
    crate::app::a11y_audit::audit_screen(name, SIZE, rtl, |t| {
        if let (Some(p), Some((pr, rows, ..))) = (&panel, &drawn) {
            a11y(t, p, *pr, rows, 1.0);
        }
    });
}

#[test]
fn pmi_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-pmi",
    );
    // The file's own names are data, in any language.
    h.data_text("MBD_A");
    for scale in [1.0, 1.5] {
        shoot(&mut h, &mut goldens, "pmi-labels", false, scale);
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        for scale in [1.0, 1.5] {
            shoot(&mut h, &mut goldens, "pmi-labels-he", true, scale);
        }
    });
    goldens.finish();
}

#[test]
fn panel_lists_views_then_annotations() {
    let (model, pmi) = part();
    let mut mv = MeshView {
        model: Some(model),
        ..Default::default()
    };
    mv.pmi.data = Some(Arc::new(pmi));
    mv.bim.panel = Some(PanelKind::Pmi);
    assert_eq!(mv.pmi_rows_len(), 5);
    let p = mv.pmi_panel();
    assert_eq!(p.rows[0].text, "MBD_A");
    assert_eq!(p.rows[2].text, "Ø0.25 Ⓜ | A | B | C");
    // A view row lights nothing; an annotation with faces lights them, and back.
    mv.bim.row = 0;
    mv.pmi_light();
    assert!(mv.bim.restyled.is_none());
    mv.bim.row = 4;
    mv.pmi_light();
    assert!(mv.bim.restyled.is_some());
    mv.bim.panel = None;
    mv.pmi_light();
    assert!(mv.bim.restyled.is_none());
    // The saved view shows only its annotations (the chosen one besides).
    mv.pmi.view = Some(0);
    assert!(mv.pmi.shown(0) && !mv.pmi.shown(1) && mv.pmi.shown(3));
}

/// A Latin annotation keeps its order in a right-to-left UI ("100 ±0.2", not "±0.2 100");
/// a Hebrew one is left to the text's own direction.
#[test]
fn annotation_text_keeps_its_order_right_to_left() {
    let (_, pmi) = part();
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let s = super::shown_row_text(&pmi.items[0]);
        assert!(
            s.starts_with('\u{2066}') && s.ends_with('\u{2069}') && s.contains("100 ±0.2"),
            "{s:?}"
        );
        let mut hebrew = pmi.items[0].clone();
        hebrew.text = "קדח 12".into();
        assert_eq!(super::shown_row_text(&hebrew), "קדח 12");
    });
    assert_eq!(super::shown_row_text(&pmi.items[0]), "100 ±0.2");
}
