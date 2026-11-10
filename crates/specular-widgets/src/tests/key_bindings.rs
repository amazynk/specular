use super::enabled_state::{STATES, with_state};
use crate::ctx::Target;
use crate::keys_edit::{Choice, KEY};
use crate::settings::SettingsTarget as S;
use crate::{Chrome, Command, Input, ViewerState};
use specular_gfx::{DrawList, Text};
use specular_input::{Action, Displace, Key, KeyCombo, KeySeq, KeymapEdit, Mode, Mods, NamedKey};
use specular_theme::Palette;

fn press(combo: KeyCombo) -> Input {
    Input::Key {
        combo,
        repeat: false,
    }
}

fn named(k: NamedKey) -> Input {
    press(KeyCombo::new(Key::Named(k), Mods::NONE))
}

fn draw(c: &mut Chrome, text: &mut Text, st: &ViewerState) {
    let mut dl = DrawList::new(1.0);
    for i in 0..30 {
        dl.clear(1.0);
        c.draw(&mut dl, text, st, &Palette::dark(), 1.0 + i as f64 * 0.016);
    }
}
fn chrome(rtl: bool) -> Chrome {
    let mut c = Chrome::new();
    c.set_size(1000.0, 800.0);
    c.last_rtl = rtl;
    c.poke(1.0);
    c
}

fn focus(c: &Chrome) -> Option<S> {
    match &c.modal_focus {
        Some(Target::Settings(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Tabs from the search field until the keyboard is on the list.
fn tab_to_list(c: &mut Chrome, st: &ViewerState) -> (Mode, Action) {
    for _ in 0..20 {
        c.input(&named(NamedKey::Tab), st, 2.0);
        if let Some(S::KeyRow(m, a)) = focus(c) {
            return (m, a);
        }
    }
    panic!("Tab never reached the list: {:?}", focus(c));
}

fn edit_in(cmds: &[Command]) -> Option<&KeymapEdit> {
    cmds.iter().find_map(|c| match c {
        Command::Keymap(e) => Some(e),
        _ => None,
    })
}

fn focused_node(t: &crate::accesskit::TreeUpdate) -> &crate::accesskit::Node {
    t.nodes
        .iter()
        .find(|(id, _)| *id == t.focus)
        .map(|(_, n)| n)
        .expect("the focus is a node")
}

#[test]
fn record_a_free_key_by_keyboard() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome(false);
        c.open_settings_at(KEY);
        draw(&mut c, &mut text, st);
        let (m0, a0) = tab_to_list(&mut c, st);
        assert_eq!(m0, Mode::Viewer);
        // ↓ moves down the rows, Home back to the first.
        c.input(&named(NamedKey::ArrowDown), st, 2.0);
        let Some(S::KeyRow(_, a1)) = focus(&c) else {
            panic!("{:?}", focus(&c))
        };
        assert_ne!(a1, a0);
        c.input(&named(NamedKey::Home), st, 2.0);
        assert_eq!(focus(&c), Some(S::KeyRow(m0, a0)));
        // → walks the row's parts (its keys, Add), ← back.
        c.input(&named(NamedKey::ArrowRight), st, 2.0);
        assert!(
            matches!(focus(&c), Some(S::KeyChip(..) | S::KeyAdd(..))),
            "{:?}",
            focus(&c)
        );
        c.input(&named(NamedKey::ArrowLeft), st, 2.0);
        assert_eq!(focus(&c), Some(S::KeyRow(m0, a0)));

        // Enter records; the recording is announced.
        c.input(&named(NamedKey::Enter), st, 2.0);
        assert!(c.recording_keys());
        assert!(
            c.announce
                .as_ref()
                .is_some_and(|a| a.1.contains("Recording"))
        );
        assert_eq!(focus(&c), Some(S::KeyRecordCancel));
        // Every key goes to the recorder, typed text too.
        let free = KeyCombo::new(
            Key::Named(NamedKey::F(9)),
            Mods {
                shift: true,
                ..Mods::NONE
            },
        );
        assert!(
            st.keymap
                .lookup_in(&[Mode::Viewer], &KeySeq::one(free))
                .is_none()
        );
        let r = c.input(&press(free), st, 2.0);
        assert!(r.consumed && r.commands.is_empty());
        assert!(c.input(&Input::Text("x".into()), st, 2.0).consumed);
        assert!(c.settings.search.is_empty());
        assert!(c.recording_keys());
        // No key follows: the keys are bound, without asking (they are free).
        assert_eq!(c.keys_tick(st, 9.0), None);
        assert!(!c.recording_keys());
        let cmds = c.take_commands();
        assert_eq!(
            edit_in(&cmds),
            Some(&KeymapEdit::Bind {
                mode: m0,
                keys: KeySeq::one(free),
                action: a0,
                old: None,
                displace: Displace::Keep,
            })
        );
        assert_eq!(
            focus(&c),
            Some(S::KeyRow(m0, a0)),
            "the keyboard comes back"
        );
    });
}

/// Esc on the Keys page goes back one step: to the page it was reached from, at its
/// scroll, the keyboard on that section. Opened straight at Keys, there is no such step.
#[test]
fn esc_goes_back_to_the_page_keys_came_from() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome(false);
        c.open_settings_at("viewing");
        draw(&mut c, &mut text, st);
        let from = c.settings.section;
        c.settings.scroll = 120.0;
        draw(&mut c, &mut text, st);
        let scroll = c.settings.scroll;
        assert!(scroll > 0.0);
        let keys = crate::settings::section_index(
            specular_settings::find(KEY)
                .expect("keys setting")
                .section
                .key(),
        )
        .expect("keys section");
        c.settings_click(S::Section(keys), st);
        c.modal_focus = Some(Target::Settings(S::Section(keys)));
        draw(&mut c, &mut text, st);
        assert!(c.settings.keys.on_page);
        // A question first, then the page.
        c.settings.keys.confirm = Some(crate::keys_edit::Confirm::ResetAll);
        assert!(c.input(&named(NamedKey::Escape), st, 2.0).consumed);
        assert!(c.settings.keys.confirm.is_none());
        assert_eq!(c.settings.section, keys);
        assert!(c.input(&named(NamedKey::Escape), st, 2.0).consumed);
        assert_eq!(c.modal, Some(crate::Modal::Settings), "Settings stays open");
        assert_eq!((c.settings.section, c.settings.scroll), (from, scroll));
        assert_eq!(focus(&c), Some(S::Section(from)));
        draw(&mut c, &mut text, st);
        assert_eq!(c.settings.scroll, scroll);
        // Customise keys… opens Keys itself: nothing to go back to here.
        c.close_modal();
        c.open_settings_at(KEY);
        draw(&mut c, &mut text, st);
        assert!(!c.keys_return());
    });
}

#[test]
fn a_taken_key_asks_and_esc_closes() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome(false);
        c.open_settings_at(KEY);
        draw(&mut c, &mut text, st);
        let (m, a) = tab_to_list(&mut c, st);
        // A key of another action of the mode.
        let taken = m
            .actions()
            .filter(|b| *b != a)
            .find_map(|b| {
                st.keymap
                    .keys_in(m, b)
                    .into_iter()
                    .find(|k| k.len() == 1 && !k.keys()[0].is_escape())
            })
            .expect("a bound key");
        c.input(&named(NamedKey::Enter), st, 2.0);
        c.input(&press(taken.keys()[0]), st, 2.0);
        assert_eq!(c.keys_tick(st, 9.0), None);
        assert!(
            c.take_commands().is_empty(),
            "nothing bound before the answer"
        );
        let p = c
            .settings
            .keys
            .prompt
            .as_ref()
            .expect("a conflict to answer");
        assert_eq!((p.action, p.keys), (a, taken));
        assert!(p.choices.contains(&Choice::Replace));
        assert_eq!(p.choices.last(), Some(&Choice::Cancel));
        assert_eq!(focus(&c), Some(S::KeyChoice(0)));
        assert!(c.announce.as_ref().is_some_and(|a| !a.1.is_empty()));

        // Esc closes the question, the keyboard is back on the row.
        assert!(c.input(&named(NamedKey::Escape), st, 2.0).consumed);
        assert!(c.settings.keys.prompt.is_none());
        assert!(c.modal.is_some(), "Settings stays open");
        assert_eq!(focus(&c), Some(S::KeyRow(m, a)));

        // Again, answering Replace.
        c.input(&named(NamedKey::Enter), st, 3.0);
        c.input(&press(taken.keys()[0]), st, 3.0);
        c.keys_tick(st, 9.0);
        let i = c.settings.keys.prompt.as_ref().unwrap().choices.iter();
        let i = i.clone().position(|ch| *ch == Choice::Replace).unwrap();
        draw(&mut c, &mut text, st);
        for _ in 0..i {
            c.input(&named(NamedKey::Tab), st, 9.0);
        }
        assert_eq!(focus(&c), Some(S::KeyChoice(i as u8)));
        let r = c.input(&named(NamedKey::Enter), st, 9.0);
        assert_eq!(
            edit_in(&r.commands),
            Some(&KeymapEdit::Bind {
                mode: m,
                keys: taken,
                action: a,
                old: None,
                displace: Displace::Replace,
            })
        );
    });
}

#[test]
fn esc_is_recorded_when_short_and_cancels_when_held() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome(false);
        c.open_settings_at(KEY);
        draw(&mut c, &mut text, st);
        tab_to_list(&mut c, st);
        c.input(&named(NamedKey::Enter), st, 2.0);
        let esc = KeyCombo::new(Key::Named(NamedKey::Escape), Mods::NONE);
        // Held: cancelled, nothing bound, Settings stays open.
        c.input(&press(esc), st, 2.0);
        assert!(c.recording_keys());
        c.input(
            &Input::Key {
                combo: esc,
                repeat: true,
            },
            st,
            3.0,
        );
        assert!(!c.recording_keys());
        assert!(c.modal.is_some());
        c.input(&Input::KeyUp { combo: esc }, st, 3.0);
        assert!(c.modal.is_some(), "the release does not close Settings");
        assert!(c.take_commands().is_empty());
        assert!(
            c.announce
                .as_ref()
                .is_some_and(|a| a.1.contains("cancelled"))
        );

        // Short: Esc is the key recorded.
        c.input(&named(NamedKey::Enter), st, 4.0);
        assert!(c.recording_keys());
        c.input(&press(esc), st, 4.0);
        c.input(&Input::KeyUp { combo: esc }, st, 4.1);
        assert!(c.recording_keys());
        assert_eq!(
            c.settings.keys.recording.as_ref().unwrap().rec.keys(),
            [esc]
        );
    });
}

#[test]
fn search_finds_actions_by_key() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome(false);
        c.open_settings_at(KEY);
        c.settings.search = "ctrl+shift+k".into();
        draw(&mut c, &mut text, st);
        let found = |a: Action| {
            c.hits
                .iter()
                .any(|(_, t)| matches!(t, Target::Settings(S::KeyRow(_, b)) if *b == a))
        };
        assert!(
            found(Action::ToggleCull),
            "Ctrl+Shift+K finds cull mode's key"
        );
        assert!(!found(Action::NextImage));
    });
}

#[test]
fn rtl_arrows_follow_the_reading_order() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome(true);
        c.open_settings_at(KEY);
        draw(&mut c, &mut text, st);
        let row = tab_to_list(&mut c, st);
        c.last_rtl = true;
        c.input(&named(NamedKey::ArrowLeft), st, 2.0);
        assert!(
            matches!(focus(&c), Some(S::KeyChip(..) | S::KeyAdd(..))),
            "{:?}",
            focus(&c)
        );
        c.input(&named(NamedKey::ArrowRight), st, 2.0);
        assert_eq!(focus(&c), Some(S::KeyRow(row.0, row.1)));
    });
}

#[test]
fn accessible_rows_and_recording() {
    use crate::accesskit::Role;
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome(false);
        c.set_accessibility(true);
        c.open_settings_at(KEY);
        draw(&mut c, &mut text, st);
        let (_, a) = tab_to_list(&mut c, st);
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        let name = crate::action_label(a);
        let n = focused_node(&t);
        assert!(
            n.label().is_some_and(|l| l.contains(&name)),
            "{:?}",
            n.label()
        );
        // The modes are tabs.
        let tabs = t
            .nodes
            .iter()
            .filter(|(_, n)| n.role() == Role::Tab)
            .count();
        assert!(tabs >= Mode::ALL.len(), "{tabs}");
        // Recording: the cancel button has the keyboard and is named.
        c.input(&named(NamedKey::Enter), st, 2.0);
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        let n = focused_node(&t);
        assert_eq!(n.role(), Role::Button);
        assert!(n.label().is_some_and(|l| !l.is_empty()));
    });
}
