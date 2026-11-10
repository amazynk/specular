//! Screenshot tests of the equaliser's panel (a preset, boosted and cut bands, the keyboard
//! on a band): English dark left to right and Hebrew light right to left, at 1× and 1.5×.
//! Rendered offscreen on lavapipe and compared with the PNGs in `tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular-widgets --test eq_shots`, then review and commit
//! the rewritten PNGs.

use specular_gfx::Rect;
use specular_i18n::{I18n, LanguageIdentifier, langid, t};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;
use specular_widgets::develop_panel::Item;
use specular_widgets::eq_panel::{self, EqView};

const SIZE: (f32, f32) = (900.0, 760.0);

fn view() -> EqView {
    let names = [
        "flat",
        "bass-boost",
        "treble-boost",
        "vocal",
        "classical",
        "rock",
        "pop",
        "jazz",
        "loudness",
        "spoken-word",
    ];
    let mut presets: Vec<String> = names
        .iter()
        .map(|n| I18n::global().tr(&format!("eq-preset-{n}")))
        .collect();
    presets.push(t!("eq-user-preset-name", n = 1));
    EqView {
        on: true,
        presets,
        preset: 10,
        preamp: -1.5,
        labels: [31.25, 62.5, 125.0, 250.0, 500.0, 1e3, 2e3, 4e3, 8e3, 16e3]
            .iter()
            .map(|&hz| eq_panel::band_label(hz))
            .collect(),
        bands: vec![5.0, 4.0, 2.5, 1.0, -1.0, -1.5, 0.5, 2.5, 3.5, 4.5],
        can_delete: true,
    }
}

#[test]
fn eq_panel_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-widgets",
    );
    let cases: [(&str, LanguageIdentifier, bool); 2] = [
        ("eq-en-dark", langid!("en-US"), false),
        ("eq-he-light", langid!("he"), true),
    ];
    for (name, lang, rtl) in cases {
        I18n::global().set_language(lang.clone());
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for scale in [1.0f32, 1.5] {
            let mut p = eq_panel::panel(&view());
            p.set_focus(Some(Item::Control {
                card: 0,
                control: 7,
                part: 0,
            }));
            let window = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
            let shot = h.render(
                SIZE,
                scale,
                |dl| dl.rect(window, 0.0, pal.canvas),
                |dl, text| p.draw(dl, text, &pal, window, rtl, 1.0),
            );
            goldens.check(&format!("{name}-{scale}x"), &shot);
        }
    }
    I18n::global().set_language(langid!("en-US"));
    goldens.finish();
}
