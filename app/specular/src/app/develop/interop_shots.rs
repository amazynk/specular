//! Screenshot tests of the interop UI: the sidecar conflict question, and the Other tools card
//! (a darktable edit's modules, Open in darktable, Import Lightroom settings). English dark
//! left to right and Hebrew light right to left, at 1× and 1.5×. Compared with the PNGs in
//! `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::interop_shots`, then
//! review and commit the rewritten PNGs.

use std::path::PathBuf;

use specular_develop::interop::{crs, darktable};
use specular_develop::model::History;
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use super::controls::{self, Session};
use super::interop::{self, OTHERS_CARD, Others};
use super::store::{Conflict, Found};

const PANEL: (f32, f32) = (480.0, 360.0);
const SHEET: (f32, f32) = (720.0, 420.0);

fn others() -> Others {
    Others {
        darktable: Some(darktable::Darktable {
            modules: [
                "rawprepare",
                "exposure",
                "sigmoid",
                "colorbalancergb",
                "denoiseprofile",
            ]
            .map(String::from)
            .to_vec(),
        }),
        darktable_bin: Some(PathBuf::from("/usr/bin/darktable")),
        crs: crs::read(include_str!(
            "../../../../../crates/specular-develop/tests/fixtures/xmp/lightroom.xmp"
        )),
    }
}

/// The panel with only the Other tools card, open.
fn session() -> Session {
    let p = PathBuf::from("/photos/IMG_2041.CR2");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    for c in &mut s.panel.cards {
        c.expanded = false;
    }
    s.panel
        .cards
        .insert(0, interop::card(&others(), true).expect("card"));
    s.panel.history_expanded = false;
    s
}

fn conflict() -> Conflict {
    let p = PathBuf::from("/photos/IMG_2041.CR2");
    Conflict {
        file_s: 1_761_998_400,
        library_s: 1_761_912_000,
        writer: Some("Specular 0.2.0".into()),
        file: Found {
            edit: controls::new_edit(&p),
            history: History::default(),
            notices: Vec::new(),
            name: String::new(),
        },
    }
}

/// A fixed time text, so the shots do not depend on the machine's zone.
fn fixed(secs: i64) -> String {
    let [y, mo, d, h, mi, _] = specular_convert::civil_from_unix(secs);
    specular_i18n::I18n::global()
        .localizer()
        .date_time((y as i32, mo as u8, d as u8), (h as u8, mi as u8))
        .unwrap_or_default()
}

#[test]
fn others_card_and_conflict_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-interop",
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
                assert_eq!(s.panel.cards[0].id, OTHERS_CARD);
                let window = Rect::new(0.0, 0.0, PANEL.0, PANEL.1);
                h.render(
                    PANEL,
                    scale,
                    |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                    |dl, text| s.panel.draw(dl, text, &pal, window, rtl, 1.0),
                )
            });
            goldens.check(&format!("develop-others-{name}-{suffix}"), &shot);
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let (title, detail) = interop::conflict_text_at(
                    &PathBuf::from("/photos/IMG_2041.CR2"),
                    &conflict(),
                    &fixed,
                );
                let mut c = specular_widgets::Chrome::new();
                c.reduce_motion = true;
                c.set_size(SHEET.0, SHEET.1);
                c.poke(1.0);
                c.confirm_choice(
                    title,
                    detail,
                    specular_i18n::t!("develop-conflict-use-file"),
                    1,
                    specular_i18n::t!("develop-conflict-keep-library"),
                    2,
                );
                with_state(|st| {
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
            goldens.check(&format!("develop-conflict-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// The viewer's state the chrome draws over (a photo shown at fit).
pub(in crate::app) fn with_state<R>(f: impl FnOnce(&specular_widgets::ViewerState) -> R) -> R {
    let settings = specular_settings::Settings::default();
    let store = specular_settings::SettingsStore::in_memory();
    let hud = specular_widgets::HudStats::default();
    let keymap = specular_input::Keymap::default();
    let st = specular_widgets::ViewerState {
        actions: specular_input::ActionState {
            image: specular_input::ImageState::Ready,
            count: 1,
            pages: 1,
            ..specular_input::ActionState::default()
        },
        has_image: true,
        index: 0,
        count: 1,
        zoom: 0.86,
        at_fit: true,
        fullscreen: false,
        file_name: Some("IMG_2041.CR2"),
        dims: Some((6000, 4000)),
        loading: false,
        error: None,
        truncated: false,
        metadata: None,
        hud: &hud,
        keymap: &keymap,
        store: &store,
        settings: &settings,
        overlays: specular_widgets::Overlays::default(),
        playback: None,
        motion: None,
        spread: None,
        pages: None,
    };
    f(&st)
}
