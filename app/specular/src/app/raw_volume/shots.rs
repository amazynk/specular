//! Screenshot tests of the raw volume dialog: dark, left to right, on a guess that fills the
//! file; and light, right to left, on a layout too big for it. Rendered offscreen on lavapipe
//! and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular raw_volume::shots`, then review
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

fn dialog(too_big: bool, focus: Part) -> Dialog {
    let mut d = Dialog::new(
        Path::new("/scans/bracket_512x512x300.raw"),
        512 * 512 * 300 * 2,
    );
    d.layout.spacing = [0.025, 0.025, 0.025];
    if too_big {
        d.step(Row::Slices, 10);
        d.step(Row::Type, 1);
    }
    d.focus = focus;
    d
}

#[test]
fn raw_volume_dialog_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-raw-volume",
    );
    let fits = build_view(&dialog(false, Part::Row(Row::Width)));
    // Right to left in Hebrew: made and drawn in the language.
    let he = specular_i18n::langid!("he");
    let big = specular_i18n::I18n::with_thread_language(he.clone(), || {
        build_view(&dialog(true, Part::Row(Row::Slices)))
    });
    let (dark, light) = (Palette::dark(), Palette::light());
    for (name, v, pal, rtl) in [
        ("fits", &fits, &dark, false),
        ("too-big-rtl-light", &big, &light, true),
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
        goldens.check(&format!("raw-volume-{name}-1x"), &shot);
        super::super::a11y_audit::expect_clean(&format!("raw-volume-{name}-1x"), &tree, rtl);
    }
    goldens.finish();
}

/// Every row and button sits in the sheet without overlapping, right to left mirrors,
/// large text fits, and every part has a node the screen reader can act on.
#[test]
fn parts_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let pal = Palette::dark();
    let v = build_view(&dialog(true, Part::Open));
    assert!(v.bad);
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, true, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.5));
    });
    let (l, r) = (&out[0], &out[1]);
    assert_eq!(l.sheet, r.sheet);
    for d in [&out[0], &out[2]] {
        let s = d.sheet;
        let parts: Vec<_> = d
            .hits
            .iter()
            .filter(|(_, h)| matches!(h, Hit::Part(_)))
            .collect();
        assert_eq!(parts.len(), Part::order().len());
        for (a, ha) in &d.hits {
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
    assert_eq!(a11y_target(tree.focus.0), Some(Part::Open));
    let mut parts = Vec::new();
    for (id, n) in &tree.nodes {
        parts.extend(a11y_target(id.0));
        if a11y_target(id.0) == Some(Part::Open) {
            assert!(n.is_disabled(), "Open is on for a layout that does not fit");
        }
    }
    for p in Part::order() {
        assert!(parts.contains(&p), "{p:?} has no node");
    }
}
