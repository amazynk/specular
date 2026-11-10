use super::*;

/// While a control of Settings (or Close of the palette or the language list) has the
/// keyboard, the search field is drawn unfocused but keeps the input method: a
/// composition takes the keyboard back to the field and shows there, and the committed
/// text is typed into it. An open dropdown takes no text, and the input method is off.
#[test]
fn composing_while_a_control_has_the_keyboard() {
    use crate::Modal;
    let mut text = Text::new();
    with_image(|st| {
        for (m, rtl, scale) in [
            (Modal::Settings, false, 1.0),
            (Modal::Settings, true, 2.0),
            (Modal::Palette, false, 1.0),
            (Modal::Language, true, 1.0),
        ] {
            let case = format!("{m:?} rtl {rtl} scale {scale}");
            let mut c = chrome(900.0, rtl);
            c.text_scale = scale;
            if m == Modal::Settings {
                c.open_settings_at("viewing");
            } else {
                c.open_modal(m);
            }
            draw(&mut c, &mut text, st);
            let field = c.text_field().expect("a focused field");
            let caret = c.ime_cursor_area().unwrap();
            tab(&mut c, st);
            draw(&mut c, &mut text, st);
            assert!(
                c.modal_focus.is_some() || c.settings.tools.focus.is_some(),
                "{case}"
            );
            assert_eq!(c.text_field(), None, "{case}: drawn unfocused");
            // The input method stays on, its area at the field's caret.
            assert_eq!(c.ime_cursor_area(), Some(caret), "{case}");

            // Composing: back in the field, shown there, the area spans it.
            assert!(
                c.input(&Input::Preedit("にほんご".into()), st, 2.0)
                    .consumed
            );
            assert_eq!(c.modal_focus, None, "{case}");
            assert_eq!(c.settings.tools.focus, None, "{case}");
            draw(&mut c, &mut text, st);
            assert_eq!(c.text_field(), Some(field), "{case}");
            assert_eq!(c.preedit(), "にほんご");
            let area = c.ime_cursor_area().unwrap();
            assert!(area.w > 30.0, "{case}: {area:?}");
            assert!(
                area.x >= field.x && area.right() <= field.right(),
                "{case}: {area:?} in {field:?}"
            );
            // Enter is the input method's while it composes: nothing is pressed.
            assert!(press(&mut c, st, NamedKey::Enter, Mods::NONE).consumed);
            assert_eq!(c.modal, Some(m), "{case}");
            // Committed.
            c.input(&Input::Preedit(String::new()), st, 2.1);
            c.input(&Input::Text("日本語".into()), st, 2.1);
            draw(&mut c, &mut text, st);
            let typed = match m {
                Modal::Settings => c.settings.search.clone(),
                _ => query(&c),
            };
            assert_eq!(typed, "日本語", "{case}");
            assert_eq!(c.preedit(), "");
            // An empty preedit (a cancelled composition) moves nothing.
            tab(&mut c, st);
            let at = c.modal_focus.clone();
            assert!(at.is_some() || c.settings.tools.focus.is_some(), "{case}");
            c.input(&Input::Preedit(String::new()), st, 2.2);
            assert_eq!(c.modal_focus, at, "{case}");
        }

        // An open dropdown takes no text: the input method is off, a stray composition
        // moves nothing.
        let mut c = chrome(900.0, false);
        c.open_settings_at("viewing");
        let dd = settings_find(&mut c, &mut text, st, |s| matches!(s, S::Dropdown(_)));
        tab_to(&mut c, &mut text, st, &dd);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(c.settings.dropdown.is_some());
        draw(&mut c, &mut text, st);
        assert_eq!(c.ime_cursor_area(), None);
        let at = c.modal_focus.clone();
        c.input(&Input::Preedit("か".into()), st, 2.0);
        c.input(&Input::Preedit(String::new()), st, 2.0);
        c.input(&Input::Text("か".into()), st, 2.0);
        assert_eq!(c.modal_focus, at);
        assert!(c.settings.dropdown.is_some());
        assert!(c.settings.search.is_empty());
    });
}

/// The dialogs' search fields are text fields: Shift selects, typing replaces the
/// selection, the clipboard goes through the app (EditorCopy, EditorPaste and
/// `paste_text`, one line), Ctrl+Z undoes the field's edits (in Settings while it has
/// some), a double click selects a word, and AccessKit sees and sets the selection.
#[test]
fn search_fields_select_copy_paste_and_undo() {
    use crate::Modal;
    use crate::accesskit::{
        Action as AkAction, ActionData, ActionRequest, NodeId, TextPosition, TextSelection, TreeId,
    };
    let mut text = Text::new();
    let ctrl = |ch: char, shift: bool| Input::Key {
        combo: KeyCombo::new(
            Key::Char(ch),
            Mods {
                ctrl: true,
                shift,
                ..Mods::NONE
            },
        ),
        repeat: false,
    };
    let ctrl_shift = Mods {
        ctrl: true,
        shift: true,
        ..Mods::NONE
    };
    with_image(|st| {
        for m in [Modal::Settings, Modal::Palette, Modal::Language] {
            let case = format!("{m:?}");
            let mut c = chrome(900.0, false);
            if m == Modal::Settings {
                c.open_settings_at("viewing");
            } else {
                c.open_modal(m);
            }
            draw(&mut c, &mut text, st);
            let query = |c: &Chrome| match m {
                Modal::Settings => c.settings.search.clone(),
                Modal::Palette => c.palette.query.clone(),
                _ => c.language.query.clone(),
            };
            c.input(&Input::Text("zoom fit".into()), st, 1.0);
            // Shift+Ctrl+Left selects the last word; Ctrl+C asks the app to copy it.
            press(&mut c, st, NamedKey::ArrowLeft, ctrl_shift);
            let r = c.input(&ctrl('c', false), st, 1.0);
            assert_eq!(
                r.commands,
                vec![Command::EditorCopy("fit".into())],
                "{case}"
            );
            // Typing replaces the selection.
            c.input(&Input::Text("in".into()), st, 1.0);
            assert_eq!(query(&c), "zoom in", "{case}");
            // The caret is drawn where it is, and the selection under the text.
            press(&mut c, st, NamedKey::ArrowLeft, Mods::NONE);
            press(&mut c, st, NamedKey::ArrowLeft, Mods::NONE);
            c.input(&Input::Text("ed".into()), st, 1.0);
            assert_eq!(query(&c), "zoom edin", "{case}");
            // AccessKit: the selection in characters, and SetTextSelection.
            draw(&mut c, &mut text, st);
            let t = c.accessibility_tree(st, "x", 1.0);
            let field = t.nodes.iter().find(|(id, _)| *id == NodeId(7)).unwrap();
            let sel = field.1.text_selection().unwrap();
            assert_eq!(
                (sel.anchor.character_index, sel.focus.character_index),
                (7, 7),
                "{case}"
            );
            assert!(
                field.1.supports_action(AkAction::SetTextSelection),
                "{case}"
            );
            let at = |i| TextPosition {
                node: NodeId(13),
                character_index: i,
            };
            let req = ActionRequest {
                action: AkAction::SetTextSelection,
                target_tree: TreeId::ROOT,
                target_node: NodeId(13),
                data: Some(ActionData::SetTextSelection(TextSelection {
                    anchor: at(0),
                    focus: at(4),
                })),
            };
            assert!(c.accessibility_action(&req, st).consumed, "{case}");
            let r = c.input(&ctrl('x', false), st, 1.0);
            assert_eq!(
                r.commands,
                vec![Command::EditorCopy("zoom".into())],
                "{case}"
            );
            assert_eq!(query(&c), " edin", "{case}");
            // Ctrl+A, then a paste: the app is asked, the text comes as one line.
            c.input(&ctrl('a', false), st, 1.0);
            let r = c.input(&ctrl('v', false), st, 1.0);
            assert_eq!(r.commands, vec![Command::EditorPaste], "{case}");
            c.paste_text("rotate\r\nleft");
            assert_eq!(query(&c), "rotate left", "{case}");
            // Undo, step by step, and redo.
            c.input(&ctrl('z', false), st, 1.0);
            assert_eq!(query(&c), " edin", "{case}");
            c.input(&ctrl('z', false), st, 1.0);
            assert_eq!(query(&c), "zoom edin", "{case}");
            c.input(&ctrl('z', true), st, 1.0);
            assert_eq!(query(&c), " edin", "{case}");
            // A double click on a word selects it; a drag extends the selection.
            c.input(&ctrl('a', false), st, 1.0);
            press(&mut c, st, NamedKey::Backspace, Mods::NONE);
            c.input(&Input::Text("zoom fit".into()), st, 1.0);
            draw(&mut c, &mut text, st);
            let (b, x0) = c
                .field_geom
                .clone()
                .and_then(|g| g.text)
                .expect("the field's text is drawn");
            let f = c.text_field().unwrap();
            let y = f.center().1;
            let x = x0 + b.caret_x(1) + 1.0;
            for t in [3.0, 3.1] {
                for down in [true, false] {
                    let b = Input::PointerButton {
                        x,
                        y,
                        button: Button::Primary,
                        down,
                    };
                    c.input(&b, st, t);
                }
            }
            let e = c.field_edit().unwrap();
            assert_eq!(e.selected_text(), "zoom", "{case}");
            let x2 = x0 + b.caret_x(e.text().len()) - 1.0;
            let down = |down| Input::PointerButton {
                x,
                y,
                button: Button::Primary,
                down,
            };
            c.input(&down(true), st, 5.0);
            c.input(&Input::PointerMove { x: x2, y }, st, 5.1);
            c.input(&down(false), st, 5.2);
            assert_eq!(c.field_edit().unwrap().selected_text(), "oom fit", "{case}");
        }
        // In Settings, Ctrl+Z with nothing to undo in the field undoes the settings.
        let mut c = chrome(900.0, false);
        c.open_settings_at("viewing");
        draw(&mut c, &mut text, st);
        assert!(
            c.field_key(&KeyCombo::new(
                Key::Char('z'),
                Mods {
                    ctrl: true,
                    ..Mods::NONE
                }
            ))
            .is_none()
        );
        c.input(&Input::Text("x".into()), st, 1.0);
        assert!(
            c.field_key(&KeyCombo::new(
                Key::Char('z'),
                Mods {
                    ctrl: true,
                    ..Mods::NONE
                }
            ))
            .is_some()
        );
        assert!(c.settings.search.is_empty());
    });
}

/// Shift+click in the dialogs' search fields extends the selection (the app gives the
/// chrome the modifiers). Where there is a primary selection, a selection made in the
/// field (keys, a drag once released, a double click, AccessKit) is offered to it once
/// (`PrimaryCopy`), and a middle click puts the caret where it is and asks the app for
/// the primary selection (`PrimaryPaste`), which comes back through `paste_text`. With no
/// primary selection (other platforms) neither happens. Nothing here touches a real
/// clipboard: the commands are only looked at.
#[test]
fn search_fields_shift_click_and_primary_selection() {
    use crate::Modal;
    use crate::accesskit::{
        Action as AkAction, ActionData, ActionRequest, NodeId, TextPosition, TextSelection, TreeId,
    };
    let mut text = Text::new();
    let primary = |cmds: &[Command]| -> Vec<String> {
        cmds.iter()
            .filter_map(|c| match c {
                Command::PrimaryCopy(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    };
    with_image(|st| {
        for m in [Modal::Settings, Modal::Palette, Modal::Language] {
            let case = format!("{m:?}");
            let mut c = chrome(900.0, false);
            c.primary_selection = true;
            if m == Modal::Settings {
                c.open_settings_at("viewing");
            } else {
                c.open_modal(m);
            }
            draw(&mut c, &mut text, st);
            let query = |c: &Chrome| match m {
                Modal::Settings => c.settings.search.clone(),
                Modal::Palette => c.palette.query.clone(),
                _ => c.language.query.clone(),
            };
            c.input(&Input::Text("zoom fit".into()), st, 1.0);
            draw(&mut c, &mut text, st);
            let g = c.field_geom.clone().expect("the field is drawn");
            let (b, x0) = g.text.clone().expect("the field's text is drawn");
            let y = g.rect.center().1;
            let x_at = |i: usize| x0 + b.caret_x(i) + 0.5;
            let click = |c: &mut Chrome, x: f32, button: Button, t: f64| {
                let mut cmds = Vec::new();
                for down in [true, false] {
                    let ev = Input::PointerButton { x, y, button, down };
                    cmds.extend(c.input(&ev, st, t).commands);
                }
                cmds
            };
            // A click puts the caret; Shift+click selects from it to the click.
            let r = click(&mut c, x_at(2), Button::Primary, 2.0);
            assert!(primary(&r).is_empty(), "{case}");
            assert_eq!(c.field_edit().unwrap().caret(), 2, "{case}");
            c.input(&Input::Modifiers(Mods::SHIFT), st, 2.5);
            let r = click(&mut c, x_at(6), Button::Primary, 3.0);
            c.input(&Input::Modifiers(Mods::NONE), st, 3.5);
            assert_eq!(c.field_edit().unwrap().selected_text(), "om f", "{case}");
            // Copy-on-select: offered once, on release.
            assert_eq!(primary(&r), ["om f"], "{case}");
            let r = c.input(&Input::PointerMove { x: x_at(3), y }, st, 3.6);
            assert!(primary(&r.commands).is_empty(), "{case}");
            // Keys: Shift+End grows it, offered again once the key goes up (or the
            // keys rest); the copy keys don't offer it.
            let shift_end = KeyCombo::new(Key::Named(NamedKey::End), Mods::SHIFT);
            let r = c.input(
                &Input::Key {
                    combo: shift_end,
                    repeat: false,
                },
                st,
                4.0,
            );
            assert!(primary(&r.commands).is_empty(), "{case}");
            assert!(c.primary_pending(), "{case}");
            let r = c.input(&Input::KeyUp { combo: shift_end }, st, 4.05);
            assert_eq!(primary(&r.commands), ["om fit"], "{case}");
            assert!(!c.primary_pending(), "{case}");
            let r = c.input(
                &Input::Key {
                    combo: KeyCombo::new(Key::Char('c'), Mods::CTRL),
                    repeat: false,
                },
                st,
                4.1,
            );
            assert_eq!(r.commands, [Command::EditorCopy("om fit".into())], "{case}");
            // A drag offers what it selected when the button goes up.
            let down = |down| Input::PointerButton {
                x: x_at(0),
                y,
                button: Button::Primary,
                down,
            };
            let r = c.input(&down(true), st, 5.0);
            assert!(primary(&r.commands).is_empty(), "{case}");
            let r = c.input(&Input::PointerMove { x: x_at(4), y }, st, 5.1);
            assert!(primary(&r.commands).is_empty(), "{case}");
            let r = c.input(&down(false), st, 5.2);
            assert_eq!(primary(&r.commands), ["zoom"], "{case}");
            // AccessKit's SetTextSelection too.
            draw(&mut c, &mut text, st);
            let at = |i| TextPosition {
                node: NodeId(13),
                character_index: i,
            };
            let req = ActionRequest {
                action: AkAction::SetTextSelection,
                target_tree: TreeId::ROOT,
                target_node: NodeId(13),
                data: Some(ActionData::SetTextSelection(TextSelection {
                    anchor: at(5),
                    focus: at(8),
                })),
            };
            let r = c.accessibility_action(&req, st);
            assert_eq!(primary(&r.commands), ["fit"], "{case}");
            // A middle click: the caret goes there, the app is asked for the primary
            // selection, and what it reads is pasted there.
            let r = click(&mut c, x_at(4), Button::Middle, 6.0);
            assert!(r.contains(&Command::PrimaryPaste), "{case}");
            assert!(primary(&r).is_empty(), "{case}");
            assert_eq!(c.field_edit().unwrap().caret(), 4, "{case}");
            c.paste_text(" in");
            assert_eq!(query(&c), "zoom in fit", "{case}");
            assert_eq!(c.field_edit().unwrap().caret(), 7, "{case}");
            // Undone as one step.
            c.input(
                &Input::Key {
                    combo: KeyCombo::new(Key::Char('z'), Mods::CTRL),
                    repeat: false,
                },
                st,
                6.5,
            );
            assert_eq!(query(&c), "zoom fit", "{case}");
            // A middle click outside the field (just under it) is not the field's.
            let ev = Input::PointerButton {
                x: g.rect.center().0,
                y: g.rect.bottom() + 4.0,
                button: Button::Middle,
                down: true,
            };
            let r = c.input(&ev, st, 7.0);
            assert!(!r.commands.contains(&Command::PrimaryPaste), "{case}");
            let ev = Input::PointerButton {
                x: g.rect.center().0,
                y: g.rect.bottom() + 4.0,
                button: Button::Middle,
                down: false,
            };
            c.input(&ev, st, 7.1);
            assert_eq!(query(&c), "zoom fit", "{case}");

            // No primary selection: no offers, and a middle click pastes nothing.
            c.primary_selection = false;
            press(&mut c, st, NamedKey::Home, Mods::SHIFT);
            let r = c.input(
                &Input::Key {
                    combo: KeyCombo::new(Key::Named(NamedKey::End), Mods::SHIFT),
                    repeat: false,
                },
                st,
                8.0,
            );
            assert!(primary(&r.commands).is_empty(), "{case}");
            let r = click(&mut c, x_at(4), Button::Middle, 9.0);
            assert!(r.is_empty(), "{case}");
            assert_eq!(query(&c), "zoom fit", "{case}");
        }
    });
}
