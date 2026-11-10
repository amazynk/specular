use super::*;

/// The palette's list takes Home and End, as the language list does; Tab stays in the
/// dialog (on its Close button) and Esc closes it.
#[test]
fn palette_home_end_tab_esc() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(900.0, false);
        c.open_modal(crate::Modal::Palette);
        draw(&mut c, &mut text, st);
        let n = c.palette.items.len();
        assert!(n > 2);
        press(&mut c, st, NamedKey::End, Mods::NONE);
        assert_eq!(c.palette.selected, n - 1);
        press(&mut c, st, NamedKey::Home, Mods::NONE);
        assert_eq!(c.palette.selected, 0);
        tab(&mut c, st);
        assert_eq!(c.modal, Some(crate::Modal::Palette));
        press(&mut c, st, NamedKey::Escape, Mods::NONE);
        assert_eq!(c.modal, None);
    });
}

/// The palette and the language list have a Close button that Tab reaches: Tab and
/// Shift+Tab go between the field and Close; on Close, Enter or Space close the dialog
/// (held, nothing), Esc still closes, and typing or the list's keys go back to the
/// field. Left to right and right to left, at text scale 1 and 2.
#[test]
fn palette_and_language_close_by_tab() {
    use crate::Modal;
    let mut text = Text::new();
    with_image(|st| {
        for m in [Modal::Palette, Modal::Language] {
            for (rtl, scale) in [(false, 1.0), (true, 1.0), (false, 2.0), (true, 2.0)] {
                let case = format!("{m:?} rtl {rtl} scale {scale}");
                let mut c = chrome(900.0, rtl);
                c.text_scale = scale;
                draw(&mut c, &mut text, st);
                focus_bar(&mut c, st, &Target::Toolbar(T::Next));
                c.open_modal(m);
                draw(&mut c, &mut text, st);

                // Close is apart from the field, at the end of its line (the palette) or
                // of the title's line above it (the language list); the start, mirrored.
                let close = hit(&c, &Target::ModalClose);
                let field = c.text_field().expect("the field has the keyboard");
                assert!(!overlaps(close, field), "{case}: {close:?} on {field:?}");
                if m == Modal::Palette {
                    let mid = (field.y + field.bottom()) * 0.5;
                    assert!(
                        (close.center().1 - mid).abs() < 1.0,
                        "{case}: {close:?} beside {field:?}"
                    );
                    if rtl {
                        assert!(close.right() <= field.x, "{case}: {close:?} {field:?}");
                    } else {
                        assert!(close.x >= field.right(), "{case}: {close:?} {field:?}");
                    }
                } else {
                    assert!(close.bottom() <= field.y, "{case}: {close:?} {field:?}");
                    if rtl {
                        assert!(close.x <= field.x + 8.0, "{case}: {close:?} {field:?}");
                    } else {
                        let end = field.right() - 8.0;
                        assert!(close.right() >= end, "{case}: {close:?} {field:?}");
                    }
                }
                assert!(close.x >= 0.0 && close.right() <= 900.0, "{case}");

                // Tab: Close; the field is drawn without the keyboard but the input
                // method stays on for it.
                assert!(tab(&mut c, st).consumed);
                assert_eq!(c.modal_focus, Some(Target::ModalClose), "{case}");
                draw(&mut c, &mut text, st);
                assert_eq!(c.text_field(), None, "{case}");
                let ime = c.ime_cursor_area().expect("the input method stays on");
                assert!(
                    ime.y >= field.y && ime.bottom() <= field.bottom(),
                    "{case}: {ime:?} in {field:?}"
                );
                let t = c.accessibility_tree(st, "x", 1.0);
                assert_eq!(focused_node(&t).role(), Role::Button, "{case}");
                // Round: Tab back to the field, Shift+Tab to Close and back.
                tab(&mut c, st);
                assert_eq!(c.modal_focus, None, "{case}");
                press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
                assert_eq!(c.modal_focus, Some(Target::ModalClose), "{case}");
                press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
                assert_eq!(c.modal_focus, None, "{case}");
                draw(&mut c, &mut text, st);
                let t = c.accessibility_tree(st, "x", 1.0);
                assert_eq!(focused_node(&t).role(), Role::SearchInput, "{case}");

                // The list's keys and typing go back to the field.
                let sel = selected(&c);
                tab(&mut c, st);
                press(&mut c, st, NamedKey::ArrowDown, Mods::NONE);
                assert_eq!(c.modal_focus, None, "{case}");
                assert_eq!(selected(&c), sel + 1, "{case}");
                tab(&mut c, st);
                c.input(&Input::Text("e".into()), st, 2.0);
                assert_eq!(c.modal_focus, None, "{case}");
                assert_eq!(query(&c), "e", "{case}");
                tab(&mut c, st);
                press(&mut c, st, NamedKey::Backspace, Mods::NONE);
                assert_eq!(c.modal_focus, None, "{case}");
                assert_eq!(query(&c), "", "{case}");

                // Enter on Close closes (and runs nothing); the viewer's focus is back.
                tab(&mut c, st);
                let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
                assert!(r.consumed && actions(&r).is_empty(), "{case}");
                assert_eq!(c.modal, None, "{case}");
                assert_eq!(c.kbd, Some(Target::Toolbar(T::Next)), "{case}");
                // Space too; a held Enter doesn't close.
                c.open_modal(m);
                draw(&mut c, &mut text, st);
                tab(&mut c, st);
                assert!(repeat(&mut c, st, NamedKey::Enter, 2.1).consumed);
                assert_eq!(c.modal, Some(m), "{case}");
                press(&mut c, st, NamedKey::Space, Mods::NONE);
                assert_eq!(c.modal, None, "{case}");
                // Esc on Close closes as from the field.
                c.open_modal(m);
                draw(&mut c, &mut text, st);
                tab(&mut c, st);
                press(&mut c, st, NamedKey::Escape, Mods::NONE);
                assert_eq!(c.modal, None, "{case}");
                // A click on Close closes as before.
                c.open_modal(m);
                draw(&mut c, &mut text, st);
                let p = hit(&c, &Target::ModalClose).center();
                c.input(&Input::PointerMove { x: p.0, y: p.1 }, st, 2.0);
                for down in [true, false] {
                    let b = Input::PointerButton {
                        x: p.0,
                        y: p.1,
                        button: Button::Primary,
                        down,
                    };
                    c.input(&b, st, 2.0);
                }
                assert_eq!(c.modal, None, "{case}");
            }
        }
    });
}
