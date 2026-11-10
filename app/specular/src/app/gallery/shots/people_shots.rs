//! Screenshot tests of the People view: five people (two named, one selected, one
//! being named) in dark, in Hebrew light, and a person's faces; at 1× and 1.5×. The faces are
//! placeholders in their photo's average colour (no files are read).
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular gallery::shots::people_shots`.

use super::super::people::{Card, FaceCard, Loaded};
use super::*;

fn people_gallery() -> Gallery {
    let mut g = gallery();
    let photo = |i: usize| PathBuf::from(format!("/shots/beach-{:02}.jpg", i));
    let card = |id, name: Option<&str>, faces, i| Card {
        id,
        name: name.map(str::to_owned),
        faces,
        cover: Some((photo(i), [0.4, 0.3, 0.2, 0.25])),
    };
    g.people.open = true;
    g.people.data = Loaded {
        grouping: Some(true),
        people: vec![
            card(3, Some("Ana"), 412, 1),
            card(7, Some("Tomás"), 57, 2),
            card(12, None, 23, 4),
            card(15, None, 9, 5),
            card(21, None, 2, 7),
        ],
        faces: None,
    };
    g.people.focus = 1;
    g.people.selected.insert(12);
    g
}

fn frame(pal: &Palette, scale: f32, rtl: bool) -> Frame<'_> {
    Frame {
        pal,
        now: 1.0,
        reduce_motion: true,
        size: SIZE,
        scale,
        rtl,
        text_scale: 1.0,
        backdrop: Backdrop::None,
    }
}

fn shoot(h: &mut Harness, g: &mut Gallery, f: &Frame) -> specular_shots::Shot {
    let mut warm = DrawList::new(f.scale);
    h.text.begin_frame(f.scale);
    g.draw(&mut warm, &mut h.text, f);
    let canvas = f.pal.canvas;
    h.render(
        SIZE,
        f.scale,
        |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, canvas),
        |dl, text| {
            g.draw(dl, text, f);
        },
    )
}

#[test]
fn people_view_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-people",
    );
    let (dark, light) = (Palette::dark(), Palette::light());
    for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
        let mut g = people_gallery();
        let f = frame(&dark, scale, false);
        let shot = shoot(&mut h, &mut g, &f);
        goldens.check(&format!("people-view-dark-{suffix}"), &shot);
        let mut tree = super::super::super::a11y_audit::window(SIZE, 1.0);
        g.a11y(&mut tree, 1.0);
        super::super::super::a11y_audit::expect_clean("people-view-dark", &tree, false);

        let shot = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
            let mut g = people_gallery();
            g.people.data.people[0].name = Some("אנה".into());
            let f = frame(&light, scale, true);
            shoot(&mut h, &mut g, &f)
        });
        goldens.check(&format!("people-view-he-light-{suffix}"), &shot);

        // Naming the focused person.
        let mut g = people_gallery();
        g.people.naming = Some((7, "Tomás Ri".into()));
        let shot = shoot(&mut h, &mut g, &frame(&dark, scale, false));
        goldens.check(&format!("people-name-dark-{suffix}"), &shot);

        // One person's faces.
        let mut g = people_gallery();
        g.people.person = Some(3);
        g.people.focus = 2;
        g.people.data.faces = Some((
            3,
            (1..=7)
                .map(|i| FaceCard {
                    id: i,
                    path: PathBuf::from(format!("/shots/beach-{:02}.jpg", i)),
                    rect: [0.3, 0.2, 0.25, 0.3],
                })
                .collect(),
        ));
        let shot = shoot(&mut h, &mut g, &frame(&dark, scale, false));
        goldens.check(&format!("people-faces-dark-{suffix}"), &shot);
    }
    goldens.finish();
}

/// The viewer's state the chrome draws over (nothing open behind the People view).
fn with_state<R>(f: impl FnOnce(&specular_widgets::ViewerState) -> R) -> R {
    let settings = specular_settings::Settings::default();
    let store = specular_settings::SettingsStore::in_memory();
    let hud = specular_widgets::HudStats::default();
    let keymap = specular_input::Keymap::default();
    let st = specular_widgets::ViewerState {
        actions: specular_input::ActionState::default(),
        has_image: false,
        index: 0,
        count: 0,
        zoom: 1.0,
        at_fit: true,
        fullscreen: false,
        file_name: None,
        dims: None,
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

/// Dragging one person onto another: the card it would merge into ringed and its name at
/// the pointer (after it in the reading direction); then, dropped, the question that asks
/// first. English dark and Hebrew light, 1× and 1.5×.
#[test]
fn people_drag_merge_matches_goldens() {
    use super::super::people::{Drag, merge_question};
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-people",
    );
    let (dark, light) = (Palette::dark(), Palette::light());
    for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
        for (name, pal, rtl) in [("dark", &dark, false), ("he-light", &light, true)] {
            let lang = if rtl {
                specular_i18n::langid!("he")
            } else {
                specular_i18n::langid!("en-US")
            };
            let (drag, ask) = specular_i18n::I18n::with_thread_language(lang, || {
                let mut g = people_gallery();
                if rtl {
                    g.people.data.people[0].name = Some("אנה".into());
                }
                let f = frame(pal, scale, rtl);
                // Laid out once for the cards' places; "Tomás" (1) dragged onto Ana (0).
                shoot(&mut h, &mut g, &f);
                let from = g.people.card_rect(1).expect("card 1");
                let onto = g.people.card_rect(0).expect("card 0");
                let at = (onto.x + onto.w * 0.6, onto.y + onto.h * 0.45);
                g.people.drag = Some(Drag {
                    from: 1,
                    start: (from.x + from.w * 0.5, from.y + from.h * 0.5),
                    at,
                    moving: true,
                });
                let drag = shoot(&mut h, &mut g, &f);

                // Dropped: the question over the view.
                g.people.drag = None;
                let (title, detail) =
                    merge_question(&g.people.data.people[1], &g.people.data.people[0]);
                let mut c = specular_widgets::Chrome::new();
                c.reduce_motion = true;
                // The gallery (People) is open: no start screen under the question.
                c.gallery = true;
                c.set_size(SIZE.0, SIZE.1);
                c.poke(1.0);
                c.confirm_with_detail(title, detail, t!("people-merge"), 1);
                let canvas = pal.canvas;
                let ask = with_state(|st| {
                    h.render(
                        SIZE,
                        scale,
                        |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, canvas),
                        |dl, text| {
                            g.draw(dl, text, &f);
                            let mut warm = DrawList::new(scale);
                            c.draw(&mut warm, text, st, pal, 1.0);
                            c.draw(dl, text, st, pal, 1.0);
                        },
                    )
                });
                (drag, ask)
            });
            goldens.check(&format!("people-drag-{name}-{suffix}"), &drag);
            goldens.check(&format!("people-merge-ask-{name}-{suffix}"), &ask);
        }
    }
    goldens.finish();
}

/// Merging from the keyboard. `M` merges the selected people into the focused one after
/// the same question a drag asks (one person: the drag's words; several: how many). The
/// question is a dialog named by it that starts on Merge; Tab goes round Merge and Cancel,
/// Enter or Space presses the focused one, Esc keeps them apart.
#[test]
fn people_merge_question_is_keyboard_operable() {
    use super::super::people::merge_question;
    use crate::app::items::PEOPLE_MERGE;
    use specular_gfx::Text;
    use specular_input::{Key, KeyCombo, Mods, NamedKey};
    use specular_widgets::accesskit::Role;
    use specular_widgets::{Chrome, Command, Input, Response};

    let plain = |s: &str| -> String {
        s.chars()
            .filter(|c| !('\u{2066}'..='\u{2069}').contains(c))
            .collect()
    };
    let mut g = people_gallery();
    // Tomás (7) focused; only he selected: nothing to merge.
    g.people.selected.clear();
    g.people.selected.insert(7);
    assert!(g.people.merge_request().is_none());
    // One other selected: the drag's question.
    g.people.selected.insert(12);
    let (from, into) = g.people.merge_request().expect("one to merge");
    assert_eq!((from.clone(), into), (vec![12], 7));
    let people = &g.people.data.people;
    assert_eq!(
        g.people.merge_question_for(&from, into),
        Some(merge_question(&people[2], &people[1]))
    );
    // Two: how many.
    g.people.selected.insert(15);
    let (from, into) = g.people.merge_request().expect("two to merge");
    assert_eq!(from, vec![12, 15]);
    let (title, detail) = g.people.merge_question_for(&from, into).unwrap();
    assert_eq!(plain(&title), "Merge 2 people into Tomás?");

    with_state(|st| {
        let mut text = Text::new();
        let mut c = Chrome::new();
        c.set_size(1000.0, 700.0);
        c.set_accessibility(true);
        c.poke(1.0);
        let draw = |c: &mut Chrome, text: &mut Text| {
            let mut dl = DrawList::new(1.0);
            for i in 0..3 {
                dl.clear(1.0);
                c.draw(
                    &mut dl,
                    text,
                    st,
                    &Palette::dark(),
                    1.0 + f64::from(i) * 0.016,
                );
            }
        };
        let key = |c: &mut Chrome, k: NamedKey, mods: Mods| -> Response {
            c.input(
                &Input::Key {
                    combo: KeyCombo::new(Key::Named(k), mods),
                    repeat: false,
                },
                st,
                2.0,
            )
        };
        let focused = |c: &Chrome| -> Option<String> {
            let t = c.accessibility_tree(st, "People", 1.0);
            t.nodes
                .iter()
                .find(|(i, _)| *i == t.focus)
                .and_then(|(_, n)| n.label().map(plain))
        };
        let merged = |r: &Response| {
            r.commands
                .iter()
                .any(|c| matches!(c, Command::Item(PEOPLE_MERGE)))
        };
        let ask = |c: &mut Chrome, text: &mut Text| {
            c.confirm_with_detail(
                title.clone(),
                detail.clone(),
                t!("people-merge"),
                PEOPLE_MERGE,
            );
            draw(c, text);
        };
        let merge = t!("people-merge");

        // A dialog named by the question, starting on Merge.
        ask(&mut c, &mut text);
        let t = c.accessibility_tree(st, "People", 1.0);
        let dialog = t.nodes.iter().find(|(_, n)| {
            matches!(n.role(), Role::Dialog | Role::AlertDialog)
                && n.label().is_some_and(|l| plain(l).contains("Tomás"))
        });
        assert!(dialog.is_some(), "a dialog named by the question");
        assert_eq!(focused(&c).as_deref(), Some(merge.as_str()));
        // Tab: Cancel; Tab: Merge; Shift+Tab: Cancel.
        key(&mut c, NamedKey::Tab, Mods::NONE);
        draw(&mut c, &mut text);
        let cancel = focused(&c).expect("a focused button");
        assert_ne!(cancel, merge);
        key(&mut c, NamedKey::Tab, Mods::NONE);
        draw(&mut c, &mut text);
        assert_eq!(focused(&c).as_deref(), Some(merge.as_str()));
        key(&mut c, NamedKey::Tab, Mods::SHIFT);
        draw(&mut c, &mut text);
        assert_eq!(focused(&c).as_deref(), Some(cancel.as_str()));
        // Enter on Cancel: kept apart, the question gone.
        let r = key(&mut c, NamedKey::Enter, Mods::NONE);
        assert!(!merged(&r));
        assert!(c.modal().is_none());
        // Esc: kept apart.
        ask(&mut c, &mut text);
        let r = key(&mut c, NamedKey::Escape, Mods::NONE);
        assert!(!merged(&r));
        assert!(c.modal().is_none());
        // Enter on Merge merges; so does Space.
        ask(&mut c, &mut text);
        let r = key(&mut c, NamedKey::Enter, Mods::NONE);
        assert!(merged(&r));
        assert!(c.modal().is_none());
        ask(&mut c, &mut text);
        let r = key(&mut c, NamedKey::Space, Mods::NONE);
        assert!(merged(&r));
    });
}
