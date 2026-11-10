use super::*;

#[test]
fn dialogs_go_round_their_buttons_with_tab() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(900.0, false);
        draw(&mut c, &mut text, st);
        // The viewer's focus is where it was once a dialog closes.
        focus_bar(&mut c, st, &Target::Toolbar(T::Next));

        // Move to trash: Cancel, then the default; round and back.
        c.confirm_trash("beach.jpg".into());
        draw(&mut c, &mut text, st);
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::ConfirmCancel));
        draw(&mut c, &mut text, st);
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::ConfirmOk));
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::ConfirmCancel));
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.modal_focus, Some(Target::ConfirmOk));
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        // Enter presses the focused button: Cancel moves nothing.
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(!has_cmd(&r, |c| matches!(c, Command::ConfirmTrash)));
        assert_eq!(c.modal, None);
        assert_eq!(c.kbd, Some(Target::Toolbar(T::Next)));
        // With nothing focused Enter is still Move to trash; so is Space on it.
        c.confirm_trash("beach.jpg".into());
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(has_cmd(&r, |c| matches!(c, Command::ConfirmTrash)));
        c.confirm_trash("beach.jpg".into());
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        let r = press(&mut c, st, NamedKey::Space, Mods::NONE);
        assert!(has_cmd(&r, |c| matches!(c, Command::ConfirmTrash)));
        assert_eq!(c.modal, None);

        // Another confirmation in the same sheet sends its item, never the trash command;
        // the trash one afterwards is the trash one again.
        c.confirm("Open “x”?".into(), "Open".into(), 77);
        draw(&mut c, &mut text, st);
        assert_eq!(c.confirm_texts(), ("Open “x”?".into(), "Open".into()));
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(has_cmd(&r, |c| matches!(c, Command::Item(77))));
        assert!(!has_cmd(&r, |c| matches!(c, Command::ConfirmTrash)));
        c.confirm("Open “x”?".into(), "Open".into(), 77);
        let r = press(&mut c, st, NamedKey::Escape, Mods::NONE);
        assert!(r.commands.is_empty());
        c.confirm_trash("beach.jpg".into());
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(has_cmd(&r, |c| matches!(c, Command::ConfirmTrash)));

        // The hardware report: Copy, Close.
        c.show_report(crate::report::sample());
        draw(&mut c, &mut text, st);
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::ReportCopy));
        draw(&mut c, &mut text, st);
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(has_cmd(&r, |c| matches!(c, Command::CopyReport)));
        // Held, it copies once.
        let r = repeat(&mut c, st, NamedKey::Enter, 2.1);
        assert!(!has_cmd(&r, |c| matches!(c, Command::CopyReport)));
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        press(&mut c, st, NamedKey::Space, Mods::NONE);
        assert_eq!(c.modal, None);

        // About: Help, Renderer info…, Save diagnostics bundle…, Discord, Close.
        let m = crate::Modal::About;
        c.open_modal(m);
        draw(&mut c, &mut text, st);
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::AboutHelp));
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::AboutRenderer));
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::AboutDiagnostics));
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::AboutDiscord));
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(has_cmd(&r, |c| matches!(
            c,
            Command::Action(specular_input::Action::JoinDiscord)
        )));
        assert_eq!(c.modal, Some(m), "About stays open");
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        let t = c.accessibility_tree(st, "x", 1.0);
        assert_eq!(focused_node(&t).role(), Role::Button);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(c.modal, None, "{m:?}");

        // The shortcuts: the mode tabs, then Close. Enter or Space picks a tab; ←/→ go
        // through them (the focus with them).
        use crate::SheetMode as SM;
        c.open_modal(crate::Modal::CheatSheet);
        draw(&mut c, &mut text, st);
        assert_eq!(c.cheat_shown(st.keymap), SM::Viewer);
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::CheatTab(SM::Viewer)));
        let t = c.accessibility_tree(st, "x", 1.0);
        assert_eq!(focused_node(&t).role(), Role::Tab);
        assert_eq!(focused_node(&t).is_selected(), Some(true));
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::CheatTab(SM::Gallery)));
        press(&mut c, st, NamedKey::Space, Mods::NONE);
        assert_eq!(c.cheat_shown(st.keymap), SM::Gallery);
        press(&mut c, st, NamedKey::ArrowRight, Mods::NONE);
        assert_eq!(c.cheat_shown(st.keymap), SM::Compare);
        assert_eq!(c.modal_focus, Some(Target::CheatTab(SM::Compare)));
        press(&mut c, st, NamedKey::ArrowLeft, Mods::NONE);
        press(&mut c, st, NamedKey::ArrowLeft, Mods::NONE);
        press(&mut c, st, NamedKey::ArrowLeft, Mods::NONE);
        assert_eq!(c.cheat_shown(st.keymap), SM::Mesh, "round the start");
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert_eq!(focused_node(&t).label(), Some("3D models"));
        // The last tab, then Close.
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(c.modal, None);
        // Opened again: the tab of what is shown.
        c.open_modal(crate::Modal::CheatSheet);
        assert_eq!(c.cheat_shown(st.keymap), SM::Viewer);
        c.close_modal();
        assert_eq!(c.kbd, Some(Target::Toolbar(T::Next)));
    });
}

/// A recipe's first-run confirmation (a long title naming the command it runs) passes the
/// screen-reader audit: an alert dialog, its title read whole, both buttons named.
#[test]
fn a_recipe_confirmation_passes_the_audit() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(900.0, false);
        let title = "Run the recipe “Segment nuclei” for the first time? It runs: \
                     /opt/seg/bin/segment --in /data/cells.tif --out labels.png";
        c.confirm(title.into(), "Run".into(), 91);
        draw(&mut c, &mut text, st);
        let tree = c.accessibility_tree(st, "cells.tif", 1.0);
        assert_eq!(crate::a11y_audit::audit(&tree, false), Vec::<String>::new());
        let named = |s: &str| {
            tree.nodes
                .iter()
                .any(|(_, n)| n.label().is_some_and(|l| l.contains(s)))
        };
        assert!(named("Segment nuclei") && named("Run"));
    });
}
