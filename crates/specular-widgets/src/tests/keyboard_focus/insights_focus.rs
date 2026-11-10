use super::*;

#[test]
fn tab_reaches_the_insights_buttons() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.set_accessibility(true);
        draw(&mut c, &mut text, st);
        c.show_insights(content());
        draw(&mut c, &mut text, st);
        let mut seen = Vec::new();
        for _ in 0..6 {
            assert!(tab(&mut c, st).consumed);
            seen.push(c.modal_focus.clone().unwrap());
        }
        assert_eq!(
            seen,
            [
                Target::InsightAction(1),
                Target::InsightDismiss,
                Target::InsightRestore,
                Target::InsightSwitch,
                Target::ModalClose,
                Target::InsightAction(1),
            ]
        );
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        // In the tree the focused button has the focus.
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert_eq!(focused_node(&t).role(), Role::Button);
        // Enter and Space press the focused button.
        c.modal_focus = Some(Target::InsightAction(1));
        let r = press(&mut c, st, NamedKey::Space, Mods::NONE);
        assert_eq!(insight_keys(&r), ["set:display-cache"]);
        c.modal_focus = Some(Target::InsightSwitch);
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(insight_keys(&r), ["off"]);
        c.modal_focus = Some(Target::InsightRestore);
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(insight_keys(&r), ["unhide"]);
        c.modal_focus = Some(Target::InsightDismiss);
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(insight_keys(&r), ["dismiss:storage"]);
        assert!(!c.insights_open());
        // With nothing focused Enter still closes the panel; Close by keyboard too.
        c.show_insights(content());
        assert!(press(&mut c, st, NamedKey::Enter, Mods::NONE).consumed);
        assert!(!c.insights_open());
        c.show_insights(content());
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(!c.insights_open());
        assert_eq!(c.modal_focus, None);
    });
}

/// Opened from the indicator with the keyboard, the panel is a dialog like the others:
/// the first Tab lands on its first button, Shift+Tab from there wraps to Close, Esc
/// closes it and the keyboard is back on the indicator. A button that goes away while
/// the keyboard is on it (Show hidden insights again, once pressed) hands it on.
#[test]
fn the_insights_panel_is_a_keyboard_dialog() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.set_insight_indicator(Some(("Why is this slow?".into(), "x".into())));
        draw(&mut c, &mut text, st);
        c.set_kbd(Some(Target::Insight));
        draw(&mut c, &mut text, st);
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(insight_keys(&r), ["open"]);
        c.show_insights(content());
        draw(&mut c, &mut text, st);
        assert_eq!(c.modal_focus, None);
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::InsightAction(1)));
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.modal_focus, Some(Target::InsightSwitch));
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.modal_focus, Some(Target::InsightRestore));
        // Pressed: the app shows them again and the button goes; the keyboard goes to
        // the one in its place.
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(insight_keys(&r), ["unhide"]);
        c.set_insights(Insights {
            restore: None,
            ..content()
        });
        assert_eq!(c.modal_focus, Some(Target::InsightSwitch));
        draw(&mut c, &mut text, st);
        // The last one gone: Close.
        c.set_insights(Insights {
            restore: None,
            switch: None,
            ..content()
        });
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        // A button still there keeps the keyboard.
        c.modal_focus = Some(Target::InsightDismiss);
        c.set_insights(content());
        assert_eq!(c.modal_focus, Some(Target::InsightDismiss));
        draw(&mut c, &mut text, st);
        // Esc closes the panel; the keyboard is back where it was.
        assert!(press(&mut c, st, NamedKey::Escape, Mods::NONE).consumed);
        assert!(!c.insights_open());
        assert_eq!(c.modal_focus, None);
        assert_eq!(c.kbd, Some(Target::Insight));
        draw(&mut c, &mut text, st);
        assert_eq!(c.kbd, Some(Target::Insight));
    });
}

/// A button reached with Tab in a panel too short for its content scrolls into view.
#[test]
fn tab_scrolls_the_insights_button_into_view() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.set_size(1000.0, 300.0);
        let mut ins = content();
        ins.now.splice(
            0..0,
            (0..12).map(|i| InsightItem {
                text: format!("Advice number {i} that takes a line"),
                gain: Some("A little faster".into()),
                action: None,
            }),
        );
        c.show_insights(ins);
        draw(&mut c, &mut text, st);
        assert_eq!(c.insights_scroll, 0.0);
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::InsightAction(13)));
        draw(&mut c, &mut text, st);
        assert!(c.insights_scroll > 0.0, "scrolled to the button");
        let b = hit(&c, &Target::InsightAction(13));
        assert!(b.bottom() <= 300.0, "{b:?}");
    });
}

/// In the gallery the chrome takes no focus of its own, but the gallery can put the
/// keyboard on the indicator: it keeps it across frames, shows the ring, is the focus in
/// the tree, presses with Enter or Space, and Esc lets go.
#[test]
fn gallery_hands_the_indicator_the_keyboard() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.set_accessibility(true);
        c.gallery = true;
        c.gallery_bar = 40.0;
        draw(&mut c, &mut text, st);
        // Nothing to focus without the indicator; Tab and F6 are the gallery's.
        assert!(!c.focus_indicator(false));
        assert!(!tab(&mut c, st).consumed);
        assert!(!press(&mut c, st, NamedKey::F(6), Mods::NONE).consumed);
        c.set_insight_indicator(Some(("Why is this slow?".into(), "Slow".into())));
        draw(&mut c, &mut text, st);
        assert!(c.focus_indicator(false));
        draw(&mut c, &mut text, st);
        assert!(c.indicator_focused());
        assert!(c.focusable(&Target::Insight));
        assert!(!c.focusable(&Target::Toolbar(T::Next)));
        let t = c.accessibility_tree(st, "x", 1.0);
        assert_eq!(focused_node(&t).role(), Role::Button);
        assert_eq!(
            insight_keys(&press(&mut c, st, NamedKey::Enter, Mods::NONE)),
            ["open"]
        );
        assert!(c.indicator_tab(false));
        assert_eq!(c.kbd, Some(Target::InsightHide));
        assert!(!c.indicator_tab(false));
        assert!(!c.indicator_focused());
        c.focus_indicator(true);
        assert!(press(&mut c, st, NamedKey::Escape, Mods::NONE).consumed);
        assert!(!c.indicator_focused());
        // Space on Hide hides it, and the focus goes with it.
        c.focus_indicator(true);
        let r = press(&mut c, st, NamedKey::Space, Mods::NONE);
        assert_eq!(insight_keys(&r), ["hide"]);
        draw(&mut c, &mut text, st);
        assert!(!c.indicator_focused());
        assert_eq!(c.kbd, None);
    });
}
