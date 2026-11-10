//! Screenshot tests of the Tone card with its zones, and the batch
//! question about photos whose edit files changed outside Specular (three ways on). English
//! dark left to right and Hebrew light right to left, at 1× and 1.5×. Compared with the
//! PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::tone_batch_shots`, then
//! review and commit the rewritten PNGs.

use std::path::PathBuf;

use specular_develop::model::{History, ModuleKind};
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use super::controls::{self, Session};

const PANEL: (f32, f32) = (480.0, 800.0);
const SHEET: (f32, f32) = (720.0, 480.0);

/// The panel with only the Tone zones card, open, a few zones moved.
fn session() -> Session {
    let p = PathBuf::from("/photos/IMG_2041.CR2");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    s.panel.cards.retain(|c| c.id == controls::ZONES_CARD);
    for c in &mut s.panel.cards {
        c.expanded = true;
    }
    for (z, v) in [(2u32, 0.6f32), (5, -0.35), (8, -1.2)] {
        let mut f = s.fields_of(ModuleKind::TONE_EQUALIZER);
        super::zones::write(&mut f, controls::id::ZONE + z, v);
        s.set_fields(ModuleKind::TONE_EQUALIZER, f, i64::from(z) * 2000);
    }
    s.panel.history_expanded = false;
    s
}

#[test]
fn tone_zones_and_batch_conflicts_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-tone-batch",
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
                let window = Rect::new(0.0, 0.0, PANEL.0, PANEL.1);
                h.render(
                    PANEL,
                    scale,
                    |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                    |dl, text| s.panel.draw(dl, text, &pal, window, rtl, 1.0),
                )
            });
            goldens.check(&format!("develop-tone-zones-{name}-{suffix}"), &shot);
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let (title, detail) = super::batch::conflict_question(3);
                let mut c = specular_widgets::Chrome::new();
                c.reduce_motion = true;
                c.set_size(SHEET.0, SHEET.1);
                c.poke(1.0);
                c.confirm_choice3(
                    title,
                    detail,
                    specular_i18n::t!("develop-batch-conflict-file"),
                    1,
                    specular_i18n::t!("develop-batch-conflict-library"),
                    2,
                    specular_i18n::t!("develop-batch-conflict-skip"),
                    3,
                );
                super::interop_shots::with_state(|st| {
                    h.render(
                        SHEET,
                        scale,
                        |dl: &mut DrawList| {
                            dl.rect(Rect::new(0.0, 0.0, SHEET.0, SHEET.1), 0.0, pal.canvas)
                        },
                        |dl, text| {
                            let mut warm = DrawList::new(scale);
                            c.draw(&mut warm, text, st, &pal, 1.0);
                            c.draw(dl, text, st, &pal, 1.0);
                        },
                    )
                })
            });
            goldens.check(&format!("develop-batch-conflict-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}
