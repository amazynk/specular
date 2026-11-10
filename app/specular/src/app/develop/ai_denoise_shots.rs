//! Screenshot tests of the Detail card in denoise's AI mode: the mode, the AI
//! share, the status line and the Stop button while it runs; and the line and button when
//! the model is missing. English dark left to right and Hebrew light right to left, at 1×
//! and 1.5×, compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::ai_denoise_shots`.

use std::path::PathBuf;

use specular_develop::model::{History, ModuleKind, Value};
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;
use specular_widgets::develop_panel::Item;

use super::ai_denoise::State;
use super::controls::{self, Session};

const SIZE: (f32, f32) = (480.0, 1000.0);

fn session(state: State) -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    let m = s.edit.module_mut(ModuleKind::DENOISE, 0).expect("denoise");
    let mut f = m.params.to_fields();
    f.set("luminance", Value::Float(0.2));
    f.set("mode", Value::Text("ai".into()));
    f.set("model", Value::Text("ffdnet-color".into()));
    f.set("blend", Value::Float(0.8));
    m.params.set_fields(f);
    s.denoise.state = state;
    s.sync_panel();
    for c in &mut s.panel.cards {
        c.expanded = c.id == controls::DETAIL_CARD;
    }
    s.panel.history_expanded = false;
    // The button (else the mode) focused: the panel scrolls the AI rows into view.
    let (card, control) = s
        .panel
        .cards
        .iter()
        .enumerate()
        .find_map(|(ci, c)| {
            c.controls
                .iter()
                .position(|k| k.id == controls::id::NOISE_AI_ACTION)
                .or_else(|| {
                    c.controls
                        .iter()
                        .position(|k| k.id == controls::id::NOISE_MODE)
                })
                .map(|k| (ci, k))
        })
        .expect("the mode row");
    s.panel.set_focus(Some(Item::Control {
        card,
        control,
        part: 0,
    }));
    s
}

#[test]
fn ai_denoise_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-ai-denoise",
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
        for (what, state) in [
            ("running", State::Running(450)),
            ("missing", State::Missing("FFDNet".into())),
        ] {
            for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
                let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                    let mut s = session(state.clone());
                    let window = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
                    h.render(
                        SIZE,
                        scale,
                        |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                        |dl, text| s.panel.draw(dl, text, &pal, window, rtl, 1.0),
                    )
                });
                goldens.check(&format!("develop-ai-denoise-{what}-{name}-{suffix}"), &shot);
            }
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

#[test]
fn the_card_shows_the_ai_rows_only_in_ai_mode() {
    use specular_widgets::develop_panel::Body;
    let ids = |s: &Session| -> Vec<u32> {
        s.panel
            .cards
            .iter()
            .find(|c| c.id == controls::DETAIL_CARD)
            .map(|c| c.controls.iter().map(|k| k.id).collect())
            .unwrap_or_default()
    };
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    let classic = ids(&s);
    assert!(classic.contains(&controls::id::NOISE_MODE));
    assert!(!classic.contains(&controls::id::NOISE_BLEND));
    let s = session(State::Running(450));
    let on = ids(&s);
    assert!(on.contains(&controls::id::NOISE_BLEND));
    assert!(on.contains(&controls::id::NOISE_AI_ACTION));
    let card = s
        .panel
        .cards
        .iter()
        .find(|c| c.id == controls::DETAIL_CARD)
        .unwrap();
    let note = card
        .controls
        .iter()
        .find(|k| k.id == controls::id::NOISE_AI_NOTE)
        .unwrap();
    assert!(matches!(&note.body, Body::Note(t) if t.contains("45")));
    // Ready: no button.
    let s = session(State::Ready);
    assert!(!ids(&s).contains(&controls::id::NOISE_AI_ACTION));
}
