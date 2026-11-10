//! Screenshot tests of the "Which kind of file is …?" prompt: dark, left to right, on a
//! `.pic` two formats fit, remembering for the folder; and light, right to left, on
//! headerless data only raw samples can be. Rendered offscreen on lavapipe and compared with
//! the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular file_kind::shots`, then review
//! and commit the rewritten PNGs.

use super::view::{a11y, a11y_target, draw};
use super::*;
use specular_gfx::Rect;
use specular_shots::{Goldens, Harness};
use specular_theme::Color;

const SIZE: (f32, f32) = (900.0, 640.0);

fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.12, 0.12, 0.13, 1.0),
    );
}

fn two_formats() -> Dialog {
    let mut d = Dialog::new(Path::new("/pictures/scan.pic"), demo_candidates(), None);
    d.step_scope(1);
    d.focus = Part::Scope;
    d
}

fn headerless() -> Dialog {
    Dialog::new(Path::new("/data/capture_0042.bin"), Vec::new(), None)
}

#[test]
fn file_kind_prompt_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-file-kind",
    );
    let two = build_view(&two_formats());
    // Right to left in Hebrew: made and drawn in the language.
    let he = specular_i18n::langid!("he");
    let raw = specular_i18n::I18n::with_thread_language(he.clone(), || build_view(&headerless()));
    let (dark, light) = (Palette::dark(), Palette::light());
    for (name, v, pal, rtl) in [
        ("two-formats", &two, &dark, false),
        ("headerless-rtl-light", &raw, &light, true),
    ] {
        let lang = if rtl {
            he.clone()
        } else {
            specular_i18n::langid!("en-US")
        };
        let (shot, tree) = specular_i18n::I18n::with_thread_language(lang, || {
            let mut drawn = None;
            let shot = h.render(SIZE, 1.0, backdrop, |dl, text| {
                drawn = Some(draw(dl, text, pal, v, SIZE, rtl, 1.0));
            });
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
            (shot, tree)
        });
        goldens.check(&format!("file-kind-{name}-1x"), &shot);
        super::super::a11y_audit::expect_clean(&format!("file-kind-{name}-1x"), &tree, rtl);
    }
    goldens.finish();
}

/// Every row and button sits in the sheet without overlapping, right to left mirrors,
/// large text fits, and every part has a node the screen reader can act on.
#[test]
fn parts_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let pal = Palette::dark();
    let d = two_formats();
    let v = build_view(&d);
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, true, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.5));
    });
    let (l, r) = (&out[0], &out[1]);
    assert_eq!(l.sheet, r.sheet);
    for dr in [&out[0], &out[2]] {
        let s = dr.sheet;
        let parts: Vec<_> = dr
            .hits
            .iter()
            .filter(|(_, h)| matches!(h, Hit::Part(_)))
            .collect();
        assert_eq!(parts.len(), d.order().len());
        for (a, ha) in &dr.hits {
            assert!(
                a.x >= s.x - 0.01
                    && a.right() <= s.right() + 0.01
                    && a.y >= s.y - 0.01
                    && a.bottom() <= s.bottom() + 0.01,
                "{ha:?} {a:?} outside {s:?}"
            );
        }
        for (i, (a, ha)) in parts.iter().enumerate() {
            for (b, hb) in &parts[i + 1..] {
                let apart = a.right() <= b.x + 0.01
                    || b.right() <= a.x + 0.01
                    || a.bottom() <= b.y + 0.01
                    || b.bottom() <= a.y + 0.01;
                assert!(apart, "{ha:?} {a:?} overlaps {hb:?} {b:?}");
            }
        }
    }
    for ((a, ha), (b, hb)) in l.hits.iter().zip(&r.hits) {
        assert_eq!(ha, hb);
        assert!((a.x - (SIZE.0 - b.right())).abs() < 0.01, "{ha:?}");
    }
    let mut tree = accesskit::TreeUpdate {
        nodes: vec![(
            accesskit::NodeId(1),
            accesskit::Node::new(accesskit::Role::Window),
        )],
        tree: None,
        tree_id: accesskit::TreeId::ROOT,
        focus: accesskit::NodeId(1),
    };
    a11y(&mut tree, &v, l, 1.0);
    assert_eq!(a11y_target(tree.focus.0), Some(Part::Scope));
    let mut parts = Vec::new();
    for (id, n) in &tree.nodes {
        if let Some(p) = a11y_target(id.0) {
            parts.push(p);
            if p == Part::Option(0) {
                assert_eq!(n.role(), accesskit::Role::RadioButton);
                assert_eq!(n.toggled(), Some(accesskit::Toggled::True));
                assert!(n.description().is_some_and(|s| !s.is_empty()));
            }
        }
    }
    for p in d.order() {
        assert!(parts.contains(&p), "{p:?} has no node");
    }
}
