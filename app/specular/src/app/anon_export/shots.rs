//! Screenshot tests of the DICOM export dialog and the mask tool at 1× and 1.5×: an
//! anonymised copy with the burned-in text warning, a patient disc folder (light), an export
//! under way, the dialog in Hebrew (right to left), and the mask tool's bar with two masks.
//! Rendered offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular anon_export::shots`, then review
//! the rewritten PNGs.

use super::*;
use specular_shots::{Goldens, Harness};

const SIZE: (f32, f32) = (760.0, 640.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

/// What stands for a grey medical image under the scrim.
fn backdrop(dl: &mut DrawList) {
    dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, Color::BLACK);
    dl.rect(
        Rect::new(180.0, 80.0, 400.0, 480.0),
        200.0,
        Color::rgba(0.42, 0.42, 0.42, 1.0),
    );
    dl.rect(
        Rect::new(300.0, 220.0, 160.0, 200.0),
        80.0,
        Color::rgba(0.7, 0.7, 0.7, 1.0),
    );
}

fn view(kind: Kind, c: &Choices, masks: usize) -> DialogView {
    DialogView {
        title: match kind {
            Kind::Copy => t!("anon-title"),
            Kind::Disc => t!("anon-disc-title"),
        },
        rows: row_views(
            c,
            kind,
            scope_label(Scope::Series, 48),
            masks,
            Some(Path::new("/home/user/Export")),
        ),
        warning: None,
        status: None,
        progress: None,
        busy: false,
        can_export: true,
        focus: Some(0),
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

fn mask_view() -> MaskView {
    MaskView {
        areas: vec![
            Rect::new(190.0, 90.0, 180.0, 22.0),
            Rect::new(430.0, 520.0, 140.0, 30.0),
        ],
        sizes: vec![(360, 44), (280, 60)],
        selected: Some(1),
        drawing: Some(Rect::new(200.0, 300.0, 90.0, 40.0)),
        status: t!("anon-mask-suggested", count = 2),
        busy: false,
        focus: Some(3),
    }
}

#[test]
fn export_dialog_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    // The destination folder's own name, not the Export button.
    h.data_text("Export");
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-anon-export-dialog",
    );
    let mut copy = view(Kind::Copy, &Choices::default(), 0);
    copy.warning = Some(t!("anon-risk-present"));
    let disc = view(Kind::Disc, &Choices::default(), 2);
    let off = Choices {
        anonymise: false,
        ..Choices::default()
    };
    let mut disc_plain = view(Kind::Disc, &off, 0);
    disc_plain.focus = Some(1);
    let mut running = view(Kind::Copy, &Choices::default(), 2);
    running.busy = true;
    running.can_export = false;
    running.focus = Some(running.rows.len());
    running.status = Some(t!("anon-progress", done = 17, total = 48));
    running.progress = Some(16.0 / 48.0);
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let mut copy_he = specular_i18n::I18n::with_thread_language(he.clone(), || {
        view(Kind::Copy, &Choices::default(), 0)
    });
    copy_he.warning =
        specular_i18n::I18n::with_thread_language(he.clone(), || Some(t!("anon-risk-possible")));
    let pal_dark = Palette::dark();
    let pal_light = Palette::light();
    for (scale, suffix) in SCALES {
        for (name, v, pal, rtl) in [
            ("copy", &copy, &pal_dark, false),
            ("disc-light", &disc, &pal_light, false),
            ("disc-plain", &disc_plain, &pal_dark, false),
            ("running", &running, &pal_dark, false),
            ("copy-he", &copy_he, &pal_dark, true),
        ] {
            let lang = if name.contains("-he") { &he } else { &en };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut drawn = None;
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    drawn = Some(draw(dl, text, pal, v, SIZE, rtl, 1.0));
                });
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
                (shot, tree)
            });
            goldens.check(&format!("anon-export-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(
                &format!("anon-export-{name}-{suffix}"),
                &tree,
                rtl,
            );
        }
        let mv = mask_view();
        for (name, pal, rtl, lang) in [
            ("mask", &pal_dark, false, &en),
            ("mask-he-light", &pal_light, true, &he),
        ] {
            let mv = specular_i18n::I18n::with_thread_language(lang.clone(), || MaskView {
                status: t!("anon-mask-suggested", count = 2),
                ..mv.clone()
            });
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                h.render(SIZE, scale, backdrop, |dl, text| {
                    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
                    draw_mask(dl, text, pal, &mv, area, rtl, 1.0);
                })
            });
            goldens.check(&format!("anon-export-{name}-{suffix}"), &shot);
        }
    }
    goldens.finish();
}

/// Every control is inside the sheet, nothing overlaps, and right to left mirrors.
#[test]
fn controls_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let mut v = view(Kind::Copy, &Choices::default(), 1);
    v.warning = Some(t!("anon-risk-present"));
    v.status = Some(t!("anon-progress", done = 1, total = 2));
    v.progress = Some(0.5);
    let pal = Palette::dark();
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, true, 1.0));
        out.push(draw(dl, text, &pal, &v, (SIZE.0, 420.0), false, 1.6));
    });
    let (l, r) = (&out[0], &out[1]);
    assert_eq!(l.sheet, r.sheet);
    for d in [&out[0], &out[2]] {
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
        let steps = l.hits.iter().any(|(_, h)| *h == Hit::Inc(rv.row));
        assert_eq!(steps, !rv.row.is_action(), "{:?}", rv.row);
    }
    assert_eq!(v.focusables()[v.rows.len()..], [Hit::Cancel, Hit::Export]);
}

#[test]
fn a11y_nodes_map_back_to_controls() {
    let Some(mut h) = Harness::new() else { return };
    let mut v = view(Kind::Copy, &Choices::default(), 0);
    v.warning = Some(t!("anon-risk-present"));
    v.focus = Some(v.rows.len() + 1);
    let pal = Palette::dark();
    let mut d = None;
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        d = Some(draw(dl, text, &pal, &v, SIZE, false, 1.0));
    });
    let d = d.unwrap();
    let mut tree = blank_tree();
    a11y(&mut tree, &v, &d, 1.0);
    assert_eq!(a11y_target(&v, &d, tree.focus.0), Some(Hit::Export));
    let role = |r| tree.nodes.iter().filter(|(_, n)| n.role() == r).count();
    // Images, the eight options; burned-in text, folder, Cancel and Export.
    assert_eq!(role(accesskit::Role::SpinButton), 9);
    assert_eq!(role(accesskit::Role::Button), 4);
    assert_eq!(role(accesskit::Role::Dialog), 1);
    assert_eq!(role(accesskit::Role::Label), 1);
    for (i, rv) in v.rows.iter().enumerate() {
        assert_eq!(
            a11y_target(&v, &d, A11Y_BASE + 1 + i as u64),
            Some(Hit::Row(rv.row))
        );
    }
    assert_eq!(a11y_target(&v, &d, A11Y_BASE), None);

    // The mask tool: a toolbar with four buttons and a button per mask.
    let mv = mask_view();
    let mut md = None;
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        md = Some(draw_mask(
            dl,
            text,
            &pal,
            &mv,
            Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
            false,
            1.0,
        ));
    });
    let md = md.unwrap();
    let mut tree = blank_tree();
    mask_a11y(&mut tree, &mv, &md, 1.0);
    let role = |r| tree.nodes.iter().filter(|(_, n)| n.role() == r).count();
    assert_eq!(role(accesskit::Role::Toolbar), 1);
    assert_eq!(role(accesskit::Role::Button), 6);
    assert_eq!(mask_target(tree.focus.0, 2), Some(MaskHit::Done));
    assert_eq!(
        mask_target(MASK_A11Y_BASE + 0x101, 2),
        Some(MaskHit::Area(1))
    );
    assert_eq!(mask_target(MASK_A11Y_BASE + 0x102, 2), None);
}

#[test]
fn choices_step_and_hide_rows() {
    let mut c = Choices::default();
    assert_eq!(c.options.temporal, Temporal::Remove);
    assert!(c.step(Row::Dates, 1));
    assert_eq!(c.options.temporal, Temporal::Full);
    assert!(c.step(Row::Dates, 1));
    assert_eq!(c.options.temporal, Temporal::Modified);
    assert!(c.step(Row::Dates, 1));
    assert_eq!(c.options.temporal, Temporal::Remove);
    assert!(c.step(Row::Dates, -1));
    assert_eq!(c.options.temporal, Temporal::Modified);
    assert!(!c.step(Row::Images, 1));
    assert!(!c.step(Row::Folder, 1));
    assert!(c.step(Row::Uids, 1));
    assert!(c.options.retain_uids);
    // Copies are always anonymised; a disc hides the options when it is not.
    assert!(c.rows(Kind::Copy).contains(&Row::BurnedIn));
    assert!(!c.rows(Kind::Copy).contains(&Row::Anonymise));
    assert!(c.step(Row::Anonymise, 1));
    assert_eq!(
        c.rows(Kind::Disc),
        [Row::Images, Row::Anonymise, Row::Folder]
    );
    assert_eq!(c.rows(Kind::Copy).len(), 11);
}
