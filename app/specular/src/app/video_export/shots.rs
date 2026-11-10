//! Screenshot tests of the export dialog at 1× and 1.5×: the choices, the question before
//! replacing (light theme, right to left), an export under way, large text, and the
//! compatibility choice in Hebrew (light, right to left) and at text scale 2. Rendered
//! offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`; see
//! `specular-shots` for the tolerance.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular video_export::shots`, then review
//! and commit the rewritten PNGs.

use super::*;
use specular_shots::{Goldens, Harness};

const SIZE: (f32, f32) = (760.0, 520.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

/// What stands for the viewer under the scrim.
fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.2, 0.32, 0.45, 1.0),
    );
    dl.rect(
        Rect::new(0.0, SIZE.1 * 0.6, SIZE.0, SIZE.1 * 0.4),
        0.0,
        Color::rgba(0.25, 0.38, 0.22, 1.0),
    );
}

fn view(c: &Choices) -> DialogView {
    let size = c.resolution.size((1600, 900));
    let (secs, bytes) = timeline::estimate(
        24,
        5.0,
        specular_settings::SlideshowTransition::KenBurns,
        c.fps,
        size,
        c.quality,
        c.profile,
    );
    DialogView {
        rows: row_views(c, (1600, 900)),
        images: t!("video-export-images", count = 24),
        summary: t!(
            "video-export-summary",
            length = clock_text(secs),
            size = size_text(bytes)
        ),
        status: None,
        progress: None,
        busy: false,
        confirm: false,
        focus: Some(ROWS.len() + 1),
    }
}

fn blank_tree() -> accesskit::TreeUpdate {
    accesskit::TreeUpdate {
        nodes: vec![(
            accesskit::NodeId(1),
            accesskit::Node::new(accesskit::Role::Window),
        )],
        tree: None,
        tree_id: accesskit::TreeId::ROOT,
        focus: accesskit::NodeId(1),
    }
}

#[test]
fn export_dialog_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-video-export-dialog",
    );
    let choices = view(&Choices::default());
    // Right to left in Hebrew: made in the language it is drawn in.
    let confirm = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let mut confirm = view(&Choices {
            resolution: Resolution::Window,
            fps: 60,
            quality: Quality::High,
            profile: Profile::High,
        });
        confirm.confirm = true;
        confirm.status = Some(t!(
            "video-export-replace-question",
            file = "Holiday 2026.mp4"
        ));
        confirm
    });
    let mut running = view(&Choices::default());
    running.busy = true;
    running.focus = Some(ROWS.len());
    running.status = Some(t!("video-export-progress", percent = "42%"));
    running.progress = Some(0.42);
    // Maximum compatibility (Baseline), focused on its row.
    let baseline = Choices {
        profile: Profile::Baseline,
        ..Choices::default()
    };
    let compat_focus = ROWS.iter().position(|r| *r == Row::Compatibility);
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let mut compat_he = specular_i18n::I18n::with_thread_language(he.clone(), || view(&baseline));
    compat_he.focus = compat_focus;
    let mut compat_large = view(&baseline);
    compat_large.focus = compat_focus;
    let pal_dark = Palette::dark();
    let pal_light = Palette::light();
    for (scale, suffix) in SCALES {
        for (name, v, pal, rtl, ts) in [
            ("choices", &choices, &pal_dark, false, 1.0),
            ("confirm-rtl-light", &confirm, &pal_light, true, 1.0),
            ("running", &running, &pal_dark, false, 1.0),
            ("large-text", &choices, &pal_dark, false, 1.6),
            ("compat-he-light", &compat_he, &pal_light, true, 1.0),
            ("compat-text-2", &compat_large, &pal_dark, false, 2.0),
        ] {
            let lang = if rtl { &he } else { &en };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut drawn = None;
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    drawn = Some(draw(dl, text, pal, v, SIZE, rtl, ts));
                });
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
                (shot, tree)
            });
            goldens.check(&format!("video-export-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(
                &format!("video-export-{name}-{suffix}"),
                &tree,
                rtl,
            );
        }
    }
    goldens.finish();
}

/// Every control is inside the sheet, nothing overlaps, right-to-left mirrors, and large
/// text still fits.
#[test]
fn controls_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let mut v = view(&Choices::default());
    v.status = Some(t!("video-export-progress", percent = "42%"));
    v.progress = Some(0.42);
    let pal = Palette::dark();
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, true, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.6));
        out.push(draw(dl, text, &pal, &v, SIZE, false, 2.0));
    });
    let (l, r) = (&out[0], &out[1]);
    assert_eq!(l.sheet, r.sheet);
    for d in [&out[0], &out[2], &out[3]] {
        let s = d.sheet;
        let controls: Vec<_> = d
            .hits
            .iter()
            .filter(|(_, h)| !matches!(h, Hit::Row(_)))
            .collect();
        for (i, (a, ha)) in controls.iter().enumerate() {
            assert!(
                a.x >= s.x && a.right() <= s.right() && a.y >= s.y && a.bottom() <= s.bottom(),
                "{ha:?} {a:?} outside {s:?}"
            );
            for (b, hb) in &controls[i + 1..] {
                let apart =
                    a.right() <= b.x || b.right() <= a.x || a.bottom() <= b.y || b.bottom() <= a.y;
                assert!(apart, "{ha:?} {a:?} overlaps {hb:?} {b:?}");
            }
        }
    }
    for ((a, ha), (b, hb)) in l.hits.iter().zip(&r.hits) {
        assert_eq!(ha, hb);
        assert!((a.x - (SIZE.0 - b.right())).abs() < 0.01, "{ha:?}");
    }
    for rv in &v.rows {
        assert!(l.hits.iter().any(|(_, h)| *h == Hit::Inc(rv.row)));
        assert!(l.hits.iter().any(|(_, h)| *h == Hit::Dec(rv.row)));
    }
    assert_eq!(v.focusables()[v.rows.len()..], [Hit::Cancel, Hit::Export]);
}

#[test]
fn a11y_nodes_map_back_to_controls() {
    let Some(mut h) = Harness::new() else { return };
    let mut v = view(&Choices::default());
    v.status = Some(t!("video-export-progress", percent = "10%"));
    v.progress = Some(0.1);
    v.busy = true;
    v.focus = Some(ROWS.len());
    let pal = Palette::dark();
    let mut d = None;
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        d = Some(draw(dl, text, &pal, &v, SIZE, false, 1.0));
    });
    let d = d.unwrap();
    let mut tree = blank_tree();
    a11y(&mut tree, &v, &d, 1.0);
    assert_eq!(a11y_target(&v, &d, tree.focus.0), Some(Hit::Cancel));
    let role = |r| tree.nodes.iter().filter(|(_, n)| n.role() == r).count();
    assert_eq!(role(accesskit::Role::SpinButton), 4);
    assert_eq!(role(accesskit::Role::Button), 2);
    assert_eq!(role(accesskit::Role::Status), 1);
    assert_eq!(a11y_target(&v, &d, A11Y_BASE + 2), Some(Hit::Row(Row::Fps)));
    assert_eq!(
        a11y_target(&v, &d, A11Y_BASE + 4),
        Some(Hit::Row(Row::Compatibility))
    );
    let compat = &tree
        .nodes
        .iter()
        .find(|(id, _)| id.0 == A11Y_BASE + 4)
        .unwrap()
        .1;
    assert_eq!(
        compat.label(),
        Some(t!("video-export-compatibility").as_str())
    );
    assert_eq!(
        compat.value(),
        Some(t!("video-export-compatibility-current").as_str())
    );
    // While exporting, the rows and Export take no actions.
    let spin = &tree
        .nodes
        .iter()
        .find(|(id, _)| id.0 == A11Y_BASE + 1)
        .unwrap()
        .1;
    assert!(spin.is_disabled());
    assert_eq!(tree.nodes[0].1.children(), &[accesskit::NodeId(A11Y_BASE)]);
    let status = &tree
        .nodes
        .iter()
        .find(|(id, _)| id.0 == A11Y_BASE + 0x1ff)
        .unwrap()
        .1;
    assert!((status.numeric_value().unwrap() - 10.0).abs() < 1e-3);
}
