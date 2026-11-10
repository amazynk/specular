use super::*;

/// Tab and F6 leave the go-to field (nothing is gone to) and move the focus as usual.
#[test]
fn tab_leaves_the_goto_field() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(900.0, false);
        draw(&mut c, &mut text, st);
        // Opened from the readout: Tab goes on from it.
        focus_bar(&mut c, st, &Target::Position);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        c.input(&Input::Text("3".into()), st, 2.0);
        assert_eq!(
            c.goto.as_ref().map(crate::line_edit::LineEdit::text),
            Some("3")
        );
        let r = tab(&mut c, st);
        assert!(r.consumed && actions(&r).is_empty());
        assert!(c.goto.is_none());
        assert_ne!(c.kbd, Some(Target::Position));
        // Opened with its shortcut: F6 goes into the chrome.
        c.set_kbd(None);
        c.handle_action(Action::GoToImage);
        assert!(c.goto.is_some());
        let r = press(&mut c, st, NamedKey::F(6), Mods::NONE);
        assert!(r.consumed && actions(&r).is_empty());
        assert!(c.goto.is_none());
        assert!(c.kbd.is_some());
    });
}

/// The go-to field edits as the other text fields do: a selection (keys, Shift+click, a
/// drag, AccessKit), the clipboard keys (as commands the app answers), undo, and where
/// there is a primary selection, copy-on-select and middle-click paste. What is drawn is
/// the number grouped in the language's digits; clicks map to the typed digits through
/// it. Pastes keep only the digits that leave a valid number. Nothing here touches a real
/// clipboard: the commands are only looked at.
#[test]
fn the_goto_field_selects_copies_and_pastes() {
    use crate::ctx::digit_bytes;
    use crate::line_edit::LineEdit;
    let mut text = Text::new();
    let primary = |cmds: &[Command]| -> Vec<String> {
        cmds.iter()
            .filter_map(|c| match c {
                Command::PrimaryCopy(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    };
    let goto = |c: &Chrome| c.goto.as_ref().map(LineEdit::text).map(str::to_owned);
    with_image(|st| {
        // A folder big enough for a grouped number to fit the readout.
        let st = &ViewerState {
            count: 20_000,
            actions: ActionState {
                count: 20_000,
                ..st.actions
            },
            ..*st
        };
        for rtl in [false, true] {
            let case = format!("rtl {rtl}");
            let mut c = chrome(900.0, rtl);
            c.primary_selection = true;
            draw(&mut c, &mut text, st);
            c.handle_action(Action::GoToImage);
            draw(&mut c, &mut text, st);
            // Empty: the range is drawn as a hint.
            let g = c.field_geom.clone().expect("the field is drawn");
            assert!(g.text.is_none(), "{case}");
            c.input(&Input::Text("12345".into()), st, 1.0);
            draw(&mut c, &mut text, st);
            let g = c.field_geom.clone().expect("the field is drawn");
            let (b, x0) = g.text.clone().expect("the number is drawn");
            let shown = g.digits.clone().expect("drawn as a number");
            assert_eq!(shown, "12,345", "{case}");
            let (starts, ends) = digit_bytes(&shown);
            // Between digit k-1 and k (a group separator goes with the digit after it).
            let x_at = |k: usize| {
                let at = if k == 0 { starts[0] } else { ends[k - 1] };
                x0 + b.caret_x(at) + 0.25
            };
            let y = g.rect.center().1;
            let click = |c: &mut Chrome, x: f32, button: Button, t: f64| {
                let mut cmds = Vec::new();
                for down in [true, false] {
                    let ev = Input::PointerButton { x, y, button, down };
                    cmds.extend(c.input(&ev, st, t).commands);
                }
                cmds
            };
            // A click puts the caret between digits; Shift+click selects to it.
            let r = click(&mut c, x_at(1), Button::Primary, 2.0);
            assert!(primary(&r).is_empty(), "{case}");
            assert_eq!(c.goto.as_ref().unwrap().caret(), 1, "{case}");
            c.input(&Input::Modifiers(Mods::SHIFT), st, 2.5);
            let r = click(&mut c, x_at(3), Button::Primary, 3.0);
            c.input(&Input::Modifiers(Mods::NONE), st, 3.5);
            assert!(
                c.goto.is_some(),
                "{case}: a click in the field keeps it open"
            );
            assert_eq!(c.goto.as_ref().unwrap().selected_text(), "23", "{case}");
            assert_eq!(primary(&r), ["23"], "{case}");
            assert_eq!(c.field_a11y_selection(), Some((1, 3)), "{case}");
            draw(&mut c, &mut text, st);
            // Ctrl+C and Ctrl+X ask the app to copy; the cut is undone with Ctrl+Z.
            let ctrl = |c: &mut Chrome, ch: char| {
                c.input(
                    &Input::Key {
                        combo: KeyCombo::new(Key::Char(ch), Mods::CTRL),
                        repeat: false,
                    },
                    st,
                    4.0,
                )
            };
            let r = ctrl(&mut c, 'c');
            assert_eq!(r.commands, [Command::EditorCopy("23".into())], "{case}");
            let r = ctrl(&mut c, 'x');
            assert_eq!(r.commands, [Command::EditorCopy("23".into())], "{case}");
            assert_eq!(goto(&c).as_deref(), Some("145"), "{case}");
            ctrl(&mut c, 'z');
            assert_eq!(goto(&c).as_deref(), Some("12345"), "{case}");
            assert!(c.goto.is_some(), "{case}: the edit keys keep it open");
            // Ctrl+V asks for the clipboard; what comes back keeps only digits that leave a
            // valid number (at most 9, no leading zero).
            press(&mut c, st, NamedKey::Home, Mods::NONE);
            let r = ctrl(&mut c, 'v');
            assert_eq!(r.commands, [Command::EditorPaste], "{case}");
            c.paste_text("0");
            assert_eq!(goto(&c).as_deref(), Some("12345"), "{case}");
            c.paste_text("No. 9–8");
            assert_eq!(goto(&c).as_deref(), Some("9812345"), "{case}");
            c.paste_text("٧٧٧");
            assert_eq!(goto(&c).as_deref(), Some("987712345"), "{case}");
            ctrl(&mut c, 'z');
            ctrl(&mut c, 'z');
            assert_eq!(goto(&c).as_deref(), Some("12345"), "{case}");
            // Keys select too, offered once the key goes up.
            press(&mut c, st, NamedKey::End, Mods::NONE);
            let r = press(&mut c, st, NamedKey::ArrowLeft, Mods::SHIFT);
            assert!(primary(&r.commands).is_empty(), "{case}");
            let up = Input::KeyUp {
                combo: combo(NamedKey::ArrowLeft, Mods::SHIFT),
            };
            let r = c.input(&up, st, 2.1);
            assert_eq!(primary(&r.commands), ["5"], "{case}");
            // A drag selects; offered on release.
            let drag = |c: &mut Chrome, x: f32, down: bool, t: f64| {
                let ev = Input::PointerButton {
                    x,
                    y,
                    button: Button::Primary,
                    down,
                };
                c.input(&ev, st, t).commands
            };
            assert!(
                primary(&drag(&mut c, x_at(0), true, 5.0)).is_empty(),
                "{case}"
            );
            let r = c.input(&Input::PointerMove { x: x_at(2), y }, st, 5.1);
            assert!(primary(&r.commands).is_empty(), "{case}");
            assert_eq!(
                primary(&drag(&mut c, x_at(2), false, 5.2)),
                ["12"],
                "{case}"
            );
            // A middle click puts the caret there and asks for the primary selection.
            let r = click(&mut c, x_at(5), Button::Middle, 6.0);
            assert!(r.contains(&Command::PrimaryPaste), "{case}");
            assert_eq!(c.goto.as_ref().unwrap().caret(), 5, "{case}");
            c.paste_text("6 7");
            assert_eq!(goto(&c).as_deref(), Some("1234567"), "{case}");
            // Enter and Esc as before: Enter goes (clamped to the folder).
            let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
            assert!(matches!(r.commands[..], [Command::GoTo(19_999)]), "{case}");
            assert!(c.goto.is_none(), "{case}");
            c.handle_action(Action::GoToImage);
            c.input(&Input::Text("2".into()), st, 7.0);
            assert!(c.back(), "{case}");
            assert!(c.goto.is_none(), "{case}");
        }
    });
}

/// The go-to field takes the keys of every other text field (`LineEdit`'s, through the
/// same path): the caret and selection keys, typing over the selection, Ctrl+A, word
/// moves, Ctrl+Insert, Shift+Delete, Shift+Insert, undo and redo, all keeping it open.
/// Left goes towards the start in right-to-left languages too (digits run left to right).
#[test]
fn the_goto_field_takes_the_text_field_keys() {
    use crate::line_edit::LineEdit;
    let mut text = Text::new();
    with_image(|st| {
        let st = &ViewerState {
            count: 20_000,
            actions: ActionState {
                count: 20_000,
                ..st.actions
            },
            ..*st
        };
        for rtl in [false, true] {
            let case = format!("rtl {rtl}");
            let mut c = chrome(900.0, rtl);
            draw(&mut c, &mut text, st);
            c.handle_action(Action::GoToImage);
            c.input(&Input::Text("12345".into()), st, 1.0);
            draw(&mut c, &mut text, st);
            let edit = |c: &Chrome| {
                let e = c.goto.as_ref().expect("the field stays open");
                (LineEdit::text(e).to_owned(), e.selection())
            };
            let key = |c: &mut Chrome, k: Key, m: Mods| {
                let ev = Input::Key {
                    combo: KeyCombo::new(k, m),
                    repeat: false,
                };
                c.input(&ev, st, 2.0)
            };
            press(&mut c, st, NamedKey::ArrowLeft, Mods::NONE);
            press(&mut c, st, NamedKey::ArrowLeft, Mods::SHIFT);
            assert_eq!(edit(&c), ("12345".into(), 3..4), "{case}");
            c.input(&Input::Text("9".into()), st, 2.0);
            assert_eq!(edit(&c), ("12395".into(), 4..4), "{case}");
            key(&mut c, Key::Char('a'), Mods::CTRL);
            assert_eq!(edit(&c), ("12395".into(), 0..5), "{case}");
            let r = key(&mut c, Key::Named(NamedKey::Insert), Mods::CTRL);
            assert_eq!(r.commands, [Command::EditorCopy("12395".into())], "{case}");
            let r = key(&mut c, Key::Named(NamedKey::Delete), Mods::SHIFT);
            assert_eq!(r.commands, [Command::EditorCopy("12395".into())], "{case}");
            assert_eq!(edit(&c), (String::new(), 0..0), "{case}");
            key(&mut c, Key::Char('z'), Mods::CTRL);
            assert_eq!(edit(&c).0, "12395", "{case}");
            key(&mut c, Key::Char('y'), Mods::CTRL);
            assert_eq!(edit(&c).0, "", "{case}");
            key(&mut c, Key::Char('z'), Mods::CTRL_SHIFT);
            assert_eq!(edit(&c).0, "", "{case}: nothing more to redo");
            let r = key(&mut c, Key::Named(NamedKey::Insert), Mods::SHIFT);
            assert_eq!(r.commands, [Command::EditorPaste], "{case}");
            c.paste_text("42");
            assert_eq!(edit(&c), ("42".into(), 2..2), "{case}");
            key(&mut c, Key::Named(NamedKey::ArrowLeft), Mods::CTRL);
            assert_eq!(edit(&c), ("42".into(), 0..0), "{case}");
            key(&mut c, Key::Named(NamedKey::ArrowRight), Mods::CTRL_SHIFT);
            assert_eq!(edit(&c), ("42".into(), 0..2), "{case}");
            key(&mut c, Key::Named(NamedKey::End), Mods::NONE);
            key(&mut c, Key::Named(NamedKey::Home), Mods::SHIFT);
            assert_eq!(edit(&c), ("42".into(), 0..2), "{case}");
            let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
            assert!(matches!(r.commands[..], [Command::GoTo(41)]), "{case}");
        }
    });
}
