//! Screenshot tests of the depth and removal tools: the Relight and Depth blur cards (the depth
//! model missing, so the state line and its link show) and the Retouch card with a selected Remove
//! (AI) spot that is computing (its Stop button). English dark left to right and Hebrew light right
//! to left, at 1× and 1.5×.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::ai_tools_shots`, then
//! review and commit the rewritten PNGs.

use std::path::PathBuf;

use specular_develop::model::{History, ModuleKind, Params, Value};
use specular_develop::ops::retouch::{Retouch, Spot, SpotMode};
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;
use specular_widgets::develop_panel::{Body, Event};

use super::ai_remove::NetState;
use super::controls::{self, Session};
use super::depth_tools::id;

const SIZE: (f32, f32) = (480.0, 2600.0);

fn session() -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    for (kind, field, v) in [
        (ModuleKind::RELIGHT, "near", 0.7),
        (ModuleKind::RELIGHT, "far", -1.2),
        (ModuleKind::DEPTH_BLUR, "amount", 0.02),
    ] {
        let m = s.edit.module_mut(kind, 0).expect("module");
        let f = m.params.to_fields().with(field, Value::Float(v));
        m.params.set_fields(f);
    }
    // The depth map: its model is missing.
    let key = specular_develop::mask::depth::key(
        &specular_develop::mask::depth::photo(&s.edit.source),
        specular_develop::model::mask::DEPTH_MODEL,
    );
    s.ai2_state.insert(
        key,
        NetState::Missing(specular_i18n::t!("settings-ai-depth")),
    );
    // A Remove (AI) spot being filled, selected.
    let spot = Spot {
        mode: SpotMode::Remove,
        model: "big-lama".into(),
        path: vec![[0.4, 0.5], [0.45, 0.52]],
        target: [0.42, 0.51],
        ..Spot::default()
    };
    let r = Retouch {
        spots: vec![spot.clone()],
        ..Retouch::default()
    };
    let reg = specular_develop::model::Registry::builtin();
    if s.edit.module(ModuleKind::RETOUCH, 0).is_none() {
        s.edit.insert(
            reg,
            specular_develop::model::ModuleInstance::new(reg, ModuleKind::RETOUCH, 0),
        );
    }
    if let Some(m) = s.edit.module_mut(ModuleKind::RETOUCH, 0) {
        m.params = Params::from_fields(ModuleKind::RETOUCH, r.to_fields());
    }
    s.ai2_state.insert(s.remove_key(&spot), NetState::Computing);
    s.retouch_state.selected = Some(0);
    s.sync_panel();
    for c in &mut s.panel.cards {
        c.expanded = [
            ModuleKind::RELIGHT,
            ModuleKind::DEPTH_BLUR,
            ModuleKind::RETOUCH,
        ]
        .iter()
        .any(|k| c.id == u32::from(k.0));
    }
    s.panel.history_expanded = false;
    s
}

fn body(s: &Session, cid: u32) -> Option<Body> {
    s.panel.control(cid).map(|k| k.body.clone())
}

#[test]
fn depth_cards_read_the_edit_and_say_what_they_need() {
    let s = session();
    match body(&s, id::RL_NEAR) {
        Some(Body::Slider(sl)) => assert!((sl.value - 0.7).abs() < 1e-6),
        b => panic!("{b:?}"),
    }
    match body(&s, id::DB_STATE) {
        Some(Body::Note(t)) => assert!(t.contains("Depth"), "{t}"),
        b => panic!("{b:?}"),
    }
    // Missing model: the action opens Local AI.
    assert!(body(&s, id::DB_ACTION).is_some());
    let mut s = s;
    let r = s.handle(
        &[Event::Value {
            control: id::DB_ACTION,
            part: 0,
            done: true,
        }],
        5_000,
    );
    assert!(
        r.iter()
            .any(|q| matches!(q, controls::Request::OpenLocalAi("ai.depth"))),
        "{r:?}"
    );
}

#[test]
fn focus_pick_without_a_map_says_so() {
    let mut s = session();
    s.handle(
        &[Event::Value {
            control: id::DB_PICK,
            part: 0,
            done: true,
        }],
        1_000,
    );
    assert!(s.focus_picking);
    let said = s.set_focus_at((0.5, 0.5), 2_000).unwrap();
    assert!(!s.focus_picking);
    assert!(!said.is_empty());
}

#[test]
fn ai_tool_cards_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-ai-tools",
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
            goldens.check(&format!("develop-ai-tools-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}
