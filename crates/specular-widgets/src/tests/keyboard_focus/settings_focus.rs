use super::*;

/// Settings → Performance → "Show hidden insights again": a button that asks the app,
/// greyed out (no Tab stop, no click, unavailable in the tree) while nothing is hidden;
/// greyed out while the keyboard is on it, the keyboard goes on.
#[test]
fn show_hidden_insights_again_from_settings() {
    const KEY: &str = "performance.insights_restore";
    let me = Target::Settings(S::Act(KEY));
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.set_accessibility(true);
        c.open_settings_at(KEY);
        draw(&mut c, &mut text, st);
        tab_to(&mut c, &mut text, st, &S::Act(KEY));
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(
            r.commands
                .iter()
                .any(|c| matches!(c, Command::SettingAction(k) if *k == KEY)),
            "{:?}",
            r.commands
        );
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(!focused_node(&t).is_disabled());
        let next = {
            let stops = c.settings_stops();
            let i = stops.iter().position(|s| *s == me).unwrap();
            Some(stops[i + 1].clone()).filter(|n| *n != Target::ModalClose)
        };
        // Nothing hidden any more: greyed out, and the keyboard goes on.
        assert!(c.set_setting_disabled(KEY, true));
        assert!(!c.set_setting_disabled(KEY, true));
        assert_eq!(c.modal_focus, next);
        draw(&mut c, &mut text, st);
        assert!(!c.settings_stops().contains(&me));
        let t = c.accessibility_tree(st, "x", 1.0);
        let node = t
            .nodes
            .iter()
            .find(|(_, n)| n.label() == Some("Show hidden insights again"))
            .map(|(_, n)| n)
            .expect("still in the tree");
        assert!(node.is_disabled());
        assert!(c.settings_click(S::Act(KEY), st).commands.is_empty());
        // Hidden again: back as a stop that presses.
        assert!(c.set_setting_disabled(KEY, false));
        draw(&mut c, &mut text, st);
        assert!(c.settings_stops().contains(&me));
        assert!(
            c.settings_click(S::Act(KEY), st)
                .commands
                .iter()
                .any(|c| matches!(c, Command::SettingAction(k) if *k == KEY))
        );
    });
}

/// Settings → Advanced → "Reset all settings…" asks first: a click or Enter opens the
/// confirmation over Settings (an alert dialog, the keyboard on Cancel) and changes
/// nothing. Cancel, Esc, Alt+← and Enter on Cancel go back to the button with every value
/// as it was, and Enter with nothing focused does nothing. Reset all resets, says so, and
/// Ctrl+Z in Settings brings the values back.
#[test]
fn reset_all_settings_asks_first() {
    use crate::Modal;
    use crate::accesskit::Role;
    use specular_settings::{Origin, Value};
    const KEY: &str = crate::settings::RESET_ALL;
    const SET: [&str; 2] = ["browsing.confirm_trash", "browsing.loop_folder"];
    let me = Target::Settings(S::Act(KEY));
    let mut text = Text::new();
    with_image(|st| {
        // Two settings changed: the reset is one undo step of its own (a single change
        // and its reset in quick succession would coalesce into none).
        for k in SET {
            st.store
                .set(k, Value::Bool(true), Origin::SettingsWindow)
                .unwrap();
        }
        let changed = || {
            SET.iter()
                .all(|k| st.store.get(k).unwrap() == Value::Bool(true))
        };
        let mut c = chrome(1000.0, false);
        c.set_accessibility(true);
        c.open_settings_at(KEY);
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        let button = t
            .nodes
            .iter()
            .find(|(_, n)| n.label() == Some("Reset all settings…"))
            .map(|(_, n)| n)
            .expect("the button is in the tree");
        assert_eq!(button.has_popup(), Some(crate::accesskit::HasPopup::Dialog));
        let open = |c: &Chrome| c.modal == Some(Modal::ConfirmTrash) && c.confirm_reset;
        let back = |c: &Chrome| {
            assert_eq!(c.modal, Some(Modal::Settings));
            assert_eq!(c.modal_focus.as_ref(), Some(&me));
            assert!(changed(), "nothing was reset");
        };

        // A click asks; the keyboard is on Cancel.
        let r = click_on(&mut c, st, &me);
        assert!(r.commands.is_empty(), "{:?}", r.commands);
        assert!(open(&c) && changed());
        assert_eq!(c.modal_focus, Some(Target::ConfirmCancel));
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        let dialog = t
            .nodes
            .iter()
            .find(|(_, n)| n.role() == Role::AlertDialog)
            .map(|(_, n)| n)
            .expect("an alert dialog");
        assert!(dialog.is_modal());
        assert_eq!(dialog.label(), Some("Reset every setting to its default?"));
        assert!(dialog.description().unwrap().contains("undone"));
        assert_eq!(focused_node(&t).label(), Some("Cancel"));
        // Cancel goes back to the button.
        click_on(&mut c, st, &Target::ConfirmCancel);
        back(&c);
        assert!(!c.confirm_reset);

        // Enter on the button asks too; Esc goes back.
        draw(&mut c, &mut text, st);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(open(&c) && changed());
        assert!(press(&mut c, st, NamedKey::Escape, Mods::NONE).consumed);
        back(&c);
        // Enter on Cancel (where the keyboard starts) cancels.
        draw(&mut c, &mut text, st);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(open(&c));
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        back(&c);
        // So does Alt+←.
        draw(&mut c, &mut text, st);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(open(&c));
        let alt = Mods {
            alt: true,
            ..Mods::NONE
        };
        press(&mut c, st, NamedKey::ArrowLeft, alt);
        back(&c);
        // With nothing focused Enter has no default here.
        draw(&mut c, &mut text, st);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        c.modal_focus = None;
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(open(&c) && changed());
        press(&mut c, st, NamedKey::Escape, Mods::NONE);
        back(&c);

        // Tab to Reset all, Enter: every setting is reset, announced, back on the button.
        draw(&mut c, &mut text, st);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        draw(&mut c, &mut text, st);
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::ConfirmOk));
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(r.commands.is_empty(), "{:?}", r.commands);
        assert!(
            SET.iter()
                .all(|k| st.store.get(k).unwrap() == Value::Bool(false)),
            "reset"
        );
        assert_eq!(c.modal, Some(Modal::Settings));
        assert_eq!(c.modal_focus.as_ref(), Some(&me));
        assert_eq!(
            c.announce.as_ref().map(|a| a.1.as_str()),
            Some("All settings were reset to their defaults.")
        );
        // Undo brings the values back.
        let z = c.input(
            &Input::Key {
                combo: KeyCombo::new(Key::Char('z'), Mods::CTRL),
                repeat: false,
            },
            st,
            2.0,
        );
        assert!(z.consumed);
        assert!(changed(), "undone");

        // The pointer on Reset all resets too.
        draw(&mut c, &mut text, st);
        click_on(&mut c, st, &me);
        draw(&mut c, &mut text, st);
        click_on(&mut c, st, &Target::ConfirmOk);
        assert!(!changed(), "reset");
        assert_eq!(c.modal, Some(Modal::Settings));
    });
}

/// A destructive confirmation (removing a Library location): an alert with the line
/// under its question as its description, the keyboard on Cancel so Enter and Esc keep
/// things as they are; only Remove sends its item.
#[test]
fn a_destructive_confirmation_starts_on_cancel() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(900.0, false);
        c.set_accessibility(true);
        let ask = |c: &mut Chrome| {
            c.confirm_destructive(
                "Remove “Pictures” from the Library?".into(),
                "Your photos stay on disk.".into(),
                "Remove".into(),
                42,
            );
        };
        let removed = |r: &Response| has_cmd(r, |c| matches!(c, Command::Item(42)));

        ask(&mut c);
        assert_eq!(c.modal, Some(crate::Modal::ConfirmTrash));
        assert_eq!(c.modal_focus, Some(Target::ConfirmCancel));
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        let dialog = t
            .nodes
            .iter()
            .find(|(_, n)| n.role() == Role::AlertDialog)
            .map(|(_, n)| n)
            .expect("an alert dialog");
        assert!(dialog.is_modal());
        assert_eq!(dialog.label(), Some("Remove “Pictures” from the Library?"));
        assert_eq!(dialog.description(), Some("Your photos stay on disk."));
        assert_eq!(focused_node(&t).label(), Some("Cancel"));
        // Enter on Cancel, Esc, a click on Cancel: nothing sent.
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(!removed(&r) && c.modal.is_none());
        ask(&mut c);
        let r = press(&mut c, st, NamedKey::Escape, Mods::NONE);
        assert!(r.commands.is_empty() && c.modal.is_none());
        ask(&mut c);
        draw(&mut c, &mut text, st);
        let r = click_on(&mut c, st, &Target::ConfirmCancel);
        assert!(!removed(&r) && c.modal.is_none());
        // Tab to Remove and Enter, or a click on it: sent, and the sheet is closed.
        ask(&mut c);
        draw(&mut c, &mut text, st);
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::ConfirmOk));
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(removed(&r) && c.modal.is_none());
        ask(&mut c);
        draw(&mut c, &mut text, st);
        let r = click_on(&mut c, st, &Target::ConfirmOk);
        assert!(removed(&r) && c.modal.is_none());
        // The next confirmation is a plain one again.
        c.confirm("Open “x”?".into(), "Open".into(), 77);
        assert_eq!(c.confirm_detail(), None);
        assert_ne!(c.modal_focus, Some(Target::ConfirmCancel));
    });
}

/// The reset confirmation fits the window at text scale 1 and 2, its buttons side by side
/// and mirrored right to left.
#[test]
fn reset_all_confirmation_layout() {
    let mut text = Text::new();
    with_image(|st| {
        for rtl in [false, true] {
            for ts in [1.0, 2.0] {
                let case = format!("rtl {rtl} text scale {ts}");
                let mut c = chrome(1000.0, rtl);
                c.text_scale = ts;
                c.open_settings_at(crate::settings::RESET_ALL);
                c.confirm_reset_all();
                draw(&mut c, &mut text, st);
                let ok = hit(&c, &Target::ConfirmOk);
                let cancel = hit(&c, &Target::ConfirmCancel);
                for r in [ok, cancel] {
                    assert!(
                        r.x >= 0.0 && r.y >= 0.0 && r.right() <= 1000.0 && r.bottom() <= 700.0,
                        "{case}: {r:?}"
                    );
                }
                // Side by side, Cancel first in reading order.
                assert_eq!(ok.y, cancel.y, "{case}");
                if rtl {
                    assert!(cancel.x >= ok.right(), "{case}");
                } else {
                    assert!(ok.x >= cancel.right(), "{case}");
                }
            }
        }
    });
}

/// A greyed-out action button does not light up under the pointer (an enabled one does).
/// The pointer keeps its default shape: the chrome never changes it.
#[test]
fn greyed_out_setting_has_no_hover() {
    const KEY: &str = "performance.insights_restore";
    let me = Target::Settings(S::Act(KEY));
    let mut text = Text::new();
    with_image(|st| {
        let frame = |c: &mut Chrome, text: &mut Text| {
            let mut dl = DrawList::new(1.0);
            c.draw(&mut dl, text, st, &Palette::dark(), 1.0);
            format!("{:?}", dl.prims)
        };
        for disabled in [true, false] {
            let mut c = chrome(1000.0, false);
            c.open_settings_at(KEY);
            c.set_setting_disabled(KEY, disabled);
            // Let the scroll to the setting settle.
            let mut away = frame(&mut c, &mut text);
            for _ in 0..20 {
                let next = frame(&mut c, &mut text);
                if next == away {
                    break;
                }
                away = next;
            }
            let r = c.hits.iter().rev().find(|(_, t)| *t == me).unwrap().0;
            let (x, y) = (r.x + r.w * 0.5, r.y + r.h * 0.5);
            c.input(&Input::PointerMove { x, y }, st, 1.0);
            assert_eq!(c.hover.as_ref(), Some(&me));
            let over = frame(&mut c, &mut text);
            assert_eq!(away == over, disabled, "disabled {disabled}");
        }
    });
}

#[test]
fn settings_by_tab() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(900.0, false);
        c.open_settings_at("viewing");
        draw(&mut c, &mut text, st);
        let stops = c.settings_stops();
        assert_eq!(stops.last(), Some(&Target::ModalClose));
        // From the search field through every control, each scrolled into view with its
        // ring, to Close, then back to the field.
        let mut seen = 0;
        let mut scrolled = false;
        loop {
            tab(&mut c, st);
            draw(&mut c, &mut text, st);
            seen += 1;
            assert!(seen <= stops.len(), "Tab doesn't reach Close");
            if c.modal_focus == Some(Target::ModalClose) {
                break;
            }
            assert!(
                c.modal_focus.is_some() || c.settings.tools.focus.is_some(),
                "Tab {seen} left the controls"
            );
            let (y, h, exact) = c.settings_focus_span().expect("focus laid out");
            let (top, vh) = c.settings.view;
            assert!(exact, "{:?} not drawn", c.modal_focus);
            assert!(
                y >= top - 1.0 && y + h <= top + vh + 1.0,
                "{:?} at {y}+{h} out of view {top}+{vh}",
                c.modal_focus
            );
            scrolled |= c.settings.scroll > 0.0;
        }
        assert_eq!(seen, stops.len());
        assert!(scrolled, "the page fits: nothing to scroll");
        let t = c.accessibility_tree(st, "x", 1.0);
        assert_eq!(focused_node(&t).role(), Role::Button);
        tab(&mut c, st);
        assert_eq!(
            (c.modal_focus.clone(), c.settings.tools.focus),
            (None, None)
        );
        // Shift+Tab from the field: Close; again: the last control.
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.modal_focus.as_ref(), stops.iter().rev().nth(1));
        // Typing goes to the search field.
        c.input(&Input::Text("zo".into()), st, 2.0);
        assert_eq!(
            (c.modal_focus.clone(), c.settings.search.as_str()),
            (None, "zo")
        );
        // Enter on Close closes.
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(c.modal, None);
    });
}

#[test]
fn settings_controls_by_keyboard() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(900.0, false);
        c.open_settings_at("viewing");
        let value = |k: &str| st.store.get(k).unwrap();

        // A switch: Space flips it (its text is not typed); its reset comes before it.
        let toggle = settings_find(&mut c, &mut text, st, |s| matches!(s, S::Toggle(_)));
        let S::Toggle(k) = toggle else { unreachable!() };
        let was = value(k);
        tab_to(&mut c, &mut text, st, &toggle);
        press(&mut c, st, NamedKey::Space, Mods::NONE);
        c.input(&Input::Text(" ".into()), st, 2.0);
        assert!(c.settings.search.is_empty());
        assert_ne!(value(k), was);
        // Held, it doesn't flip back and forth.
        repeat(&mut c, st, NamedKey::Space, 2.1);
        assert_ne!(value(k), was);
        draw(&mut c, &mut text, st);
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.modal_focus, Some(Target::Settings(S::Reset(k))));
        // Reset: the keyboard goes on to the switch.
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(value(k), was);
        assert_eq!(c.modal_focus, Some(Target::Settings(toggle.clone())));

        // A stepper's button repeats while held.
        let plus = settings_find(&mut c, &mut text, st, |s| matches!(s, S::Step(_, 1)));
        let S::Step(k, _) = plus else { unreachable!() };
        c.modal_focus = None;
        tab_to(&mut c, &mut text, st, &plus);
        let v0 = value(k);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        let v1 = value(k);
        repeat(&mut c, st, NamedKey::Enter, 2.1);
        let v2 = value(k);
        assert!(v0 != v1 && v1 != v2, "{v0:?} {v1:?} {v2:?}");
        st.store
            .reset(k, specular_settings::Origin::SettingsWindow)
            .unwrap();

        // A dropdown: Enter opens it on the option chosen now, the arrows move, Enter
        // chooses and the keyboard is back on its button; Esc closes it the same way.
        let dd = settings_find(
            &mut c,
            &mut text,
            st,
            |s| matches!(s, S::Dropdown(k) if crate::settings::option_values(k, st).len() > 1),
        );
        let S::Dropdown(k) = dd else { unreachable!() };
        let opts = crate::settings::option_values(k, st);
        let was = value(k).to_string();
        let i = opts.iter().position(|o| *o == was).unwrap();
        c.modal_focus = None;
        tab_to(&mut c, &mut text, st, &dd);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(c.settings.dropdown, Some(k));
        assert_eq!(
            c.modal_focus,
            Some(Target::Settings(S::Choose(k, was.clone())))
        );
        draw(&mut c, &mut text, st);
        let (step, want) = if i + 1 < opts.len() {
            (NamedKey::ArrowDown, i + 1)
        } else {
            (NamedKey::ArrowUp, i - 1)
        };
        press(&mut c, st, step, Mods::NONE);
        assert_eq!(
            c.modal_focus,
            Some(Target::Settings(S::Choose(k, opts[want].clone())))
        );
        draw(&mut c, &mut text, st);
        hit(&c, &Target::Settings(S::Choose(k, opts[want].clone())));
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(value(k).to_string(), opts[want]);
        assert_eq!(c.settings.dropdown, None);
        assert_eq!(c.modal_focus, Some(Target::Settings(dd.clone())));
        press(&mut c, st, NamedKey::Space, Mods::NONE);
        press(&mut c, st, NamedKey::End, Mods::NONE);
        assert_eq!(
            c.modal_focus,
            Some(Target::Settings(S::Choose(k, opts[opts.len() - 1].clone())))
        );
        press(&mut c, st, NamedKey::Escape, Mods::NONE);
        assert_eq!(c.settings.dropdown, None);
        assert_eq!(c.modal, Some(crate::Modal::Settings));
        assert_eq!(c.modal_focus, Some(Target::Settings(dd.clone())));
        // Tab leaves an open dropdown for the next control.
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        tab(&mut c, st);
        assert_eq!(c.settings.dropdown, None);
        assert!(!matches!(
            c.modal_focus,
            Some(Target::Settings(S::Choose(..)))
        ));
        assert_ne!(c.modal_focus, Some(Target::Settings(dd.clone())));
        // Backspace edits the search field again.
        press(&mut c, st, NamedKey::Backspace, Mods::NONE);
        assert_eq!(c.modal_focus, None);
    });
}
