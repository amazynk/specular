//! Screenshot tests of Develop's Export… sheet at 1× and 1.5×: the setup in English (dark)
//! and in Hebrew (light, right to left), and an export running. Rendered offscreen on
//! lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::export::shots`, then
//! review the rewritten PNGs.

use std::path::Path;
use std::time::Duration;

use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::{Color, Palette};

use super::super::super::convert::{Hit, PhaseView, a11y, draw, progress_text_for, source_label};
use super::*;

const SIZE: (f32, f32) = (900.0, 620.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.42, 0.33, 0.24, 1.0),
    );
    dl.rect(
        Rect::new(0.0, SIZE.1 * 0.55, SIZE.0, SIZE.1 * 0.45),
        0.0,
        Color::rgba(0.2, 0.3, 0.42, 1.0),
    );
}

/// The web preset with Display P3, for the folder of 36 raws.
fn export() -> Export {
    let mut e = Export::new(Vec::new(), Preset::find("web", &[]));
    e.preset.space = Space::DisplayP3;
    e
}

fn setup_view(e: &Export, pattern: &str) -> convert::DialogView {
    let src = source_label(Source::Folder, 36);
    let first = Path::new("/photos/2026/IMG_2041.CR3");
    let probe = Some(((6000, 4000), Some([2026, 7, 14, 18, 3, 9]), None));
    let v = convert::DialogView {
        phase: PhaseView::Setup {
            start: e.row_views(&e.start_rows(), &src, pattern),
            end: e.row_views(&e.end_rows(), &src, pattern),
            name: pattern.to_owned(),
            caret: pattern.len(),
            selection: (pattern.len(), pattern.len()),
            preview: e.preview(pattern, Some(first), probe),
            preedit: String::new(),
        },
        focus: None,
        export: true,
        delete_preset: None,
    };
    // On the preset row.
    let focus = v
        .focusables()
        .iter()
        .position(|h| *h == Hit::Row(Row::Preset));
    convert::DialogView { focus, ..v }
}

fn running_view() -> convert::DialogView {
    let p = Progress {
        done: 12,
        total: 36,
        failed: 0,
        skipped: 0,
        elapsed: Duration::from_secs(9),
        eta: Some(Duration::from_secs(18)),
    };
    convert::DialogView {
        phase: PhaseView::Running {
            status: progress_text_for(&p, true),
            progress: 12.4 / 36.0,
            stopping: false,
        },
        focus: Some(1),
        export: true,
        delete_preset: None,
    }
}

#[test]
fn export_sheet_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-export",
    );
    let e = export();
    let mut shots = Vec::new();
    for (lang, rtl, light) in [("en", false, false), ("he", true, true)] {
        let id = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        specular_i18n::I18n::with_thread_language(id, || {
            shots.push((
                format!("setup-{lang}"),
                setup_view(&e, "{name}-{w}"),
                rtl,
                light,
            ));
            if !rtl {
                shots.push(("running-en".to_owned(), running_view(), false, false));
            }
        });
    }
    for (scale, suffix) in SCALES {
        for (name, v, rtl, light) in &shots {
            let pal = if *light {
                Palette::light()
            } else {
                Palette::dark()
            };
            let id = if *rtl {
                specular_i18n::langid!("he")
            } else {
                specular_i18n::langid!("en-US")
            };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(id, || {
                let mut drawn = None;
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    drawn = Some(draw(dl, text, &pal, v, SIZE, *rtl, 1.0));
                });
                let mut tree = super::super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
                (shot, tree)
            });
            goldens.check(&format!("develop-export-{name}-{suffix}"), &shot);
            super::super::super::a11y_audit::expect_clean(
                &format!("develop-export-{name}-{suffix}"),
                &tree,
                *rtl,
            );
        }
    }
    goldens.finish();
}

/// The sheet's controls sit inside it without overlapping, Save preset among them.
#[test]
fn controls_fit_the_sheet() {
    let Some(mut h) = Harness::new() else { return };
    let pal = Palette::dark();
    let v = setup_view(&export(), "{name}");
    let mut drawn = None;
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        drawn = Some(draw(dl, text, &pal, &v, SIZE, false, 1.0));
    });
    let d = drawn.unwrap();
    let controls: Vec<_> = d
        .hits
        .iter()
        .filter(|(_, h)| !matches!(h, Hit::Row(_)))
        .collect();
    assert!(controls.iter().any(|(_, h)| *h == Hit::SavePreset));
    for (i, (a, ha)) in controls.iter().enumerate() {
        assert!(
            a.x >= d.sheet.x && a.right() <= d.sheet.right() && a.bottom() <= d.sheet.bottom(),
            "{ha:?} outside the sheet"
        );
        for (b, hb) in &controls[i + 1..] {
            let overlap =
                a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom();
            assert!(!overlap, "{ha:?} overlaps {hb:?}");
        }
    }
}
