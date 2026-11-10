use super::*;

/// ↑/↓ on a control of Settings go to the control before or after it: ↑ from the first
/// goes back to the search field, ↓ stays on the last, Close keeps its place, an open
/// dropdown keeps its own arrows. The search field shows as unfocused (no caret, no IME
/// area) while a control has the keyboard.
#[test]
fn settings_arrows_move_between_controls() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(900.0, false);
        c.open_settings_at("viewing");
        draw(&mut c, &mut text, st);
        assert!(
            c.text_field().is_some(),
            "the search field has the keyboard"
        );
        let stops = c.settings_stops();
        let n = stops.len();
        let section = c.settings.section;
        let plain = |t: &Target| matches!(t, Target::Settings(s) if !matches!(s, S::ToolRow(_)));
        let i = (0..n - 2)
            .find(|&i| plain(&stops[i]) && plain(&stops[i + 1]))
            .expect("two controls in a row");
        let Target::Settings(second) = stops[i + 1].clone() else {
            unreachable!()
        };
        tab_to(&mut c, &mut text, st, &second);
        assert!(
            c.text_field().is_none(),
            "the search field still looks focused"
        );
        press(&mut c, st, NamedKey::ArrowUp, Mods::NONE);
        assert_eq!(c.modal_focus.as_ref(), Some(&stops[i]));
        draw(&mut c, &mut text, st);
        let (y, h, exact) = c.settings_focus_span().expect("laid out");
        let (top, vh) = c.settings.view;
        assert!(exact && y >= top - 1.0 && y + h <= top + vh + 1.0);
        press(&mut c, st, NamedKey::ArrowDown, Mods::NONE);
        assert_eq!(c.modal_focus.as_ref(), Some(&stops[i + 1]));
        assert_eq!(
            c.settings.section, section,
            "the arrows changed the section"
        );

        // ↑ from the first control: the search field.
        if plain(&stops[0]) {
            c.modal_focus = None;
            tab(&mut c, st);
            assert_eq!(c.modal_focus.as_ref(), Some(&stops[0]));
            press(&mut c, st, NamedKey::ArrowUp, Mods::NONE);
            assert_eq!((&c.modal_focus, c.settings.tools.focus), (&None, None));
            assert_eq!(c.settings.section, section);
            draw(&mut c, &mut text, st);
            assert!(c.text_field().is_some());
        }

        // Close keeps its place; ↓ on the last control stays there.
        c.modal_focus = None;
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        press(&mut c, st, NamedKey::ArrowDown, Mods::NONE);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        press(&mut c, st, NamedKey::ArrowUp, Mods::NONE);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        let last = c.modal_focus.clone();
        assert_eq!(last.as_ref(), Some(&stops[n - 2]));
        if plain(&stops[n - 2]) {
            press(&mut c, st, NamedKey::ArrowDown, Mods::NONE);
            assert_eq!(c.modal_focus, last);
        }

        // An open dropdown keeps its arrows.
        let dd = settings_find(
            &mut c,
            &mut text,
            st,
            |s| matches!(s, S::Dropdown(k) if crate::settings::option_values(k, st).len() > 1),
        );
        let S::Dropdown(k) = dd else { unreachable!() };
        c.modal_focus = None;
        tab_to(&mut c, &mut text, st, &dd);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        draw(&mut c, &mut text, st);
        assert!(c.text_field().is_none());
        press(&mut c, st, NamedKey::ArrowDown, Mods::NONE);
        press(&mut c, st, NamedKey::ArrowUp, Mods::NONE);
        assert_eq!(c.settings.dropdown, Some(k));
        assert!(matches!(
            c.modal_focus,
            Some(Target::Settings(S::Choose(..)))
        ));
    });
}

/// The first Tab from the search field goes to the first control of the page, scrolled
/// into view, wherever the page was scrolled to.
#[test]
fn first_tab_from_search_is_the_first_control() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(900.0, false);
        c.open_settings_at("viewing");
        draw(&mut c, &mut text, st);
        c.settings.scroll = 1.0e6;
        draw(&mut c, &mut text, st);
        assert!(c.settings.scroll > 0.0, "the page fits");
        let first = c.settings_stops()[0].clone();
        tab(&mut c, st);
        if first == Target::Settings(S::ToolRow(0)) {
            assert_eq!(c.settings.tools.focus, Some(0));
        } else {
            assert_eq!(c.modal_focus, Some(first.clone()));
        }
        draw(&mut c, &mut text, st);
        let (y, h, exact) = c.settings_focus_span().expect("laid out");
        let (top, vh) = c.settings.view;
        assert!(exact, "not drawn");
        assert!(
            y >= top - 1.0 && y + h <= top + vh + 1.0,
            "{y}+{h} not in {top}+{vh}"
        );

        // Opened at a setting, the first Tab starts at it; once.
        c.close_modal();
        c.open_settings_at("viewing.toolbar_items");
        draw(&mut c, &mut text, st);
        tab(&mut c, st);
        assert_eq!(c.settings.tools.focus, Some(0));
        assert_ne!(c.settings_stops()[0], Target::Settings(S::ToolRow(0)));
        c.settings.tools.focus = None;
        c.modal_focus = None;
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(first));
    });
}

/// An open menu of the bar is drawn over the toasts and takes the clicks where they
/// overlap.
#[test]
fn menus_are_above_toasts() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(900.0, false);
        draw(&mut c, &mut text, st);
        c.toast(
            "Moved “a-rather-long-file-name-from-the-beach-2026-and-more.jpg” to the trash".into(),
            Some(1),
            1.0,
        );
        focus_bar(&mut c, st, &Target::Toolbar(T::ZoomLevel));
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(c.zoom_menu);
        draw(&mut c, &mut text, st);
        let toast = hit(&c, &Target::Toast);
        let mut overlaps = 0;
        for (r, t) in c.hits.clone() {
            if !matches!(t, Target::ZoomPreset(_)) {
                continue;
            }
            let (x0, x1) = (r.x.max(toast.x), r.right().min(toast.right()));
            let (y0, y1) = (r.y.max(toast.y), r.bottom().min(toast.bottom()));
            if x1 > x0 + 1.0 && y1 > y0 + 1.0 {
                overlaps += 1;
                let p = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
                assert_eq!(c.hit_at(p), Some(t));
            }
        }
        assert!(overlaps > 0, "the toast {toast:?} doesn't reach the menu");
    });
}
