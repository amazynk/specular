//! Screenshot tests of the batch UI: the Styles, History and snapshots, and Versions cards
//! (a snapshot compared, the keyboard on a style), and the sheet choosing modules to paste.
//! English dark left to right and Hebrew light right to left, at 1× and 1.5×. Compared with
//! the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::batch_shots`, then review
//! and commit the rewritten PNGs.

use std::path::PathBuf;

use specular_develop::model::{History, ModuleKind, Snapshot, Style};
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;
use specular_widgets::develop_panel::Item;

use super::batch::{self, STYLES_CARD, State};
use super::controls::{self, Session};
use super::sheet::{DevelopSheet, Purpose};
use crate::app::fileops::{Part, draw_sheet};

const SIZE: (f32, f32) = (480.0, 1640.0);

fn session() -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    s.edit.snapshots = vec![
        Snapshot::take("Before colour", &s.edit),
        Snapshot::take("Warm look", &s.edit),
    ];
    let mut st = State::default();
    st.shelf.put_style(Style {
        name: "Wedding warm".into(),
        items: Vec::new(),
        masks: Vec::new(),
    });
    st.versions = vec![
        (0, String::new()),
        (1, "Square crop".into()),
        (2, String::new()),
    ];
    st.version = 1;
    st.compare = Some(1);
    let snaps = s.edit.snapshots.clone();
    batch::refresh_cards(&mut s.panel, &st, &snaps);
    for c in &mut s.panel.cards {
        c.expanded = c.id >= STYLES_CARD;
    }
    s.panel.history_expanded = false;
    let card = s
        .panel
        .cards
        .iter()
        .position(|c| c.id == STYLES_CARD)
        .expect("styles");
    s.panel.set_focus(Some(Item::Control {
        card,
        control: 1,
        part: 0,
    }));
    s
}

fn sheet() -> DevelopSheet {
    let rows = vec![
        (
            controls::module_title(ModuleKind::EXPOSURE),
            ModuleKind::EXPOSURE,
            true,
        ),
        (
            controls::module_title(ModuleKind::COLOUR_CALIBRATION),
            ModuleKind::COLOUR_CALIBRATION,
            true,
        ),
        (
            controls::module_title(ModuleKind::CROP_ORIENTATION),
            ModuleKind::CROP_ORIENTATION,
            false,
        ),
        (
            controls::module_title(ModuleKind::VIEW_TRANSFORM),
            ModuleKind::VIEW_TRANSFORM,
            true,
        ),
        (
            controls::module_title(ModuleKind::LENS),
            ModuleKind::LENS,
            false,
        ),
    ];
    let mut s = DevelopSheet::modules(
        Purpose::PasteSelective,
        specular_i18n::t!("develop-paste-title"),
        rows,
        specular_i18n::t!("develop-paste-ok"),
    );
    s.mode = Some(1);
    s
}

#[test]
fn batch_ui_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-batch",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (name, lang, rtl) in [("en-dark", en, false), ("he-light", he, true)] {
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut s = session();
                let window = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
                h.render(
                    SIZE,
                    scale,
                    |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                    |dl, text| s.panel.draw(dl, text, &pal, window, rtl, 1.0),
                )
            });
            goldens.check(&format!("develop-batch-panel-{name}-{suffix}"), &shot);
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let v = sheet().view(Part::Row(1));
                let size = (720.0, 560.0);
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| {
                        dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas)
                    },
                    |dl, text| {
                        draw_sheet(dl, text, &pal, &v, size, rtl, 1.0);
                    },
                )
            });
            goldens.check(&format!("develop-batch-paste-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}
