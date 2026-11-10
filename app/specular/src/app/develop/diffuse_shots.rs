//! Screenshot tests of the Surface blur and Soften and sharpen cards: surface blur
//! on, diffuse at the Lens blur preset (the "only at 100%" note showing), the keyboard on
//! Speed 3. English dark left to right and Hebrew light right to left, at 1× and 1.5×.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::diffuse_shots`, then
//! review and commit the rewritten PNGs.

use std::path::PathBuf;

use specular_develop::model::{History, ModuleKind, Value};
use specular_develop::ops::detail::diffuse::Preset;
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;
use specular_widgets::develop_panel::Item;

use super::controls::{self, Session};
use super::diffuse::id;

const SIZE: (f32, f32) = (480.0, 1300.0);

fn session() -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    let m = s.edit.module_mut(ModuleKind::DIFFUSE, 0).expect("diffuse");
    let mut f = m.params.to_fields();
    f.overlay(&Preset::Deblur.params().to_fields());
    m.params.set_fields(f);
    let m = s
        .edit
        .module_mut(ModuleKind::SURFACE_BLUR, 0)
        .expect("surface");
    let mut f = m.params.to_fields();
    f.set("amount", Value::Float(0.4));
    m.params.set_fields(f);
    s.sync_panel();
    for c in &mut s.panel.cards {
        c.expanded = c.id == u32::from(ModuleKind::DIFFUSE.0)
            || c.id == u32::from(ModuleKind::SURFACE_BLUR.0);
    }
    s.panel.history_expanded = false;
    let (card, control) = s
        .panel
        .cards
        .iter()
        .enumerate()
        .find_map(|(ci, c)| {
            c.controls
                .iter()
                .position(|k| k.id == id::SPEED + 2)
                .map(|k| (ci, k))
        })
        .expect("speed 3");
    s.panel.set_focus(Some(Item::Control {
        card,
        control,
        part: 0,
    }));
    s
}

#[test]
fn diffuse_cards_read_the_edit() {
    let s = session();
    let find = |cid: u32| {
        s.panel
            .cards
            .iter()
            .flat_map(|c| &c.controls)
            .find(|k| k.id == cid)
            .map(|k| k.body.clone())
            .expect("control")
    };
    use specular_widgets::develop_panel::Body;
    match find(id::PRESET) {
        Body::Choice(c) => assert_eq!(c.selected, 2, "Lens blur"),
        b => panic!("{b:?}"),
    }
    match find(id::NOTE) {
        Body::Note(t) => assert!(!t.is_empty()),
        b => panic!("{b:?}"),
    }
    match find(id::ITERATIONS) {
        Body::Slider(sl) => assert_eq!(sl.value, 6.0),
        b => panic!("{b:?}"),
    }
}

#[test]
fn preset_choice_writes_the_preset_and_custom_follows_a_slider() {
    use specular_widgets::develop_panel::{Body, Event};
    let mut s = session();
    let set_choice = |s: &mut Session, i: usize| {
        for c in &mut s.panel.cards {
            for k in &mut c.controls {
                if let (true, Body::Choice(ch)) = (k.id == id::PRESET, &mut k.body) {
                    ch.set(i);
                }
            }
        }
    };
    set_choice(&mut s, 4);
    let ev = Event::Value {
        control: id::PRESET,
        part: 0,
        done: true,
    };
    s.handle(&[ev], 10_000);
    let f = s
        .edit
        .module(ModuleKind::DIFFUSE, 0)
        .unwrap()
        .params
        .to_fields();
    assert_eq!(Preset::matching(&f), Some(Preset::Bloom));
    assert_eq!(f.get("iterations"), Some(&Value::Int(4)));
    // Iterations moved by a slider stay an integer, and the preset reads "Custom".
    for c in &mut s.panel.cards {
        for k in &mut c.controls {
            if let (true, Body::Slider(sl)) = (k.id == id::ITERATIONS, &mut k.body) {
                sl.set(7.0);
            }
        }
    }
    let ev = Event::Value {
        control: id::ITERATIONS,
        part: 0,
        done: true,
    };
    s.handle(&[ev], 20_000);
    s.sync_panel();
    let f = s
        .edit
        .module(ModuleKind::DIFFUSE, 0)
        .unwrap()
        .params
        .to_fields();
    assert_eq!(f.get("iterations"), Some(&Value::Int(7)));
    let ch = s
        .panel
        .control(id::PRESET)
        .map(|k| k.body.clone())
        .expect("preset");
    match ch {
        Body::Choice(c) => assert_eq!(c.selected, 0, "Custom"),
        b => panic!("{b:?}"),
    }
}

#[test]
fn diffuse_cards_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-diffuse",
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
            goldens.check(&format!("develop-diffuse-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}
