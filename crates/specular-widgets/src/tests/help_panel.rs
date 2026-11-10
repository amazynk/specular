use super::enabled_state::with_state;
use crate::accesskit::Role;
use crate::ctx::Target;
use crate::{Chrome, Command, CrashSheet, DiagEntry, DiagSheet, Input, Modal, ViewerState};
use specular_gfx::{DrawList, Text};
use specular_input::{Action, ActionState, ImageState, Key, KeyCombo, Mods, NamedKey};
use specular_theme::Palette;

const ALT: Mods = Mods {
    alt: true,
    ..Mods::NONE
};

fn ready<R>(f: impl FnOnce(&ViewerState) -> R) -> R {
    with_state(
        ActionState {
            image: ImageState::Ready,
            count: 3,
            ..ActionState::default()
        },
        f,
    )
}

fn chrome() -> Chrome {
    let mut c = Chrome::new();
    c.set_size(900.0, 700.0);
    c.poke(1.0);
    c
}

fn draw(c: &mut Chrome, text: &mut Text, st: &ViewerState) {
    let mut dl = DrawList::new(1.0);
    c.draw(&mut dl, text, st, &Palette::light(), 1.5);
}

fn key(c: &mut Chrome, st: &ViewerState, k: Key, mods: Mods) -> crate::Response {
    c.input(
        &Input::Key {
            combo: KeyCombo::new(k, mods),
            repeat: false,
        },
        st,
        2.0,
    )
}

fn named(c: &mut Chrome, st: &ViewerState, k: NamedKey) -> crate::Response {
    key(c, st, Key::Named(k), Mods::NONE)
}

/// F1 opens the page for what is on screen, over a dialog too; F1 again closes it.
#[test]
fn f1_opens_the_help_for_the_context() {
    ready(|st| {
        let mut text = Text::new();
        let mut c = chrome();
        draw(&mut c, &mut text, st);
        named(&mut c, st, NamedKey::F(1));
        assert_eq!(c.modal(), Some(Modal::Help));
        assert_eq!(c.help_page().as_deref(), Some("viewer"));
        named(&mut c, st, NamedKey::F(1));
        assert_eq!(c.modal(), None);
        // Over Settings: its page; Esc goes back to Settings.
        c.open_settings_at("viewing");
        named(&mut c, st, NamedKey::F(1));
        assert_eq!(c.help_page().as_deref(), Some("settings"));
        named(&mut c, st, NamedKey::Escape);
        assert_eq!(c.modal(), Some(Modal::Settings));
        c.close_modal();
        c.gallery = true;
        assert_eq!(c.help_context(st), "gallery");
    });
    with_state(ActionState::default(), |st| {
        assert_eq!(chrome().help_context(st), "empty");
    });
}

/// Typing searches as you go; Enter opens the result; Alt+Left and Alt+Right go back and
/// forward; links are rows; Esc clears a search before it closes the panel.
#[test]
fn search_links_and_history() {
    ready(|st| {
        let mut text = Text::new();
        let mut c = chrome();
        c.show_help("viewer");
        draw(&mut c, &mut text, st);
        c.input(&Input::Text("hillshade".into()), st, 2.0);
        assert!(!c.help.hits.is_empty());
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(
            t.nodes.iter().any(|(_, n)| n.role() == Role::ListItem
                && n.label().is_some_and(|l| l.contains("Maps")))
        );
        named(&mut c, st, NamedKey::Enter);
        assert_eq!(c.help_page().as_deref(), Some("maps"));
        assert!(c.help.query.is_empty());
        key(&mut c, st, Key::Named(NamedKey::ArrowLeft), ALT);
        assert_eq!(c.help_page().as_deref(), Some("viewer"));
        key(&mut c, st, Key::Named(NamedKey::ArrowRight), ALT);
        assert_eq!(c.help_page().as_deref(), Some("maps"));
        // A link of the page: Down onto it, Enter follows it.
        c.show_help("viewer");
        draw(&mut c, &mut text, st);
        assert!(!c.help.links.is_empty());
        named(&mut c, st, NamedKey::ArrowDown);
        named(&mut c, st, NamedKey::Enter);
        assert_ne!(c.help_page().as_deref(), Some("viewer"));
        // Contents: every page is a row.
        c.activate(Target::HelpContents, st);
        assert_eq!(
            c.help.rows(),
            specular_help::Manual::current().pages().len()
        );
        // Esc: the search first, then the panel.
        c.input(&Input::Text("zoom".into()), st, 2.0);
        named(&mut c, st, NamedKey::Escape);
        assert!(c.help.query.is_empty());
        assert_eq!(c.modal(), Some(Modal::Help));
        named(&mut c, st, NamedKey::Escape);
        assert_eq!(c.modal(), None);
    });
}

/// The palette lists sections of the manual for a search ("Help: …").
#[test]
fn the_palette_searches_the_manual() {
    ready(|st| {
        let mut c = chrome();
        c.handle_action(Action::Palette);
        c.input(&Input::Text("hillshade".into()), st, 2.0);
        crate::palette::rebuild(&mut c.palette, st);
        let i = c
            .palette
            .items
            .iter()
            .position(|e| matches!(e.run, crate::palette::Run::Help(..)))
            .expect("a Help entry");
        assert!(c.palette.items[i].label.starts_with("Help: "));
        c.run_palette(i, st);
        assert_eq!(c.help_page().as_deref(), Some("maps"));
    });
}

/// The crash prompt: View report shows it, Copy hands it to the app (the clipboard is the
/// app's, never touched here), Save diagnostics bundle… asks the app, Enter dismisses.
#[test]
fn the_crash_prompt() {
    ready(|st| {
        let mut text = Text::new();
        let mut c = chrome();
        c.show_crash(CrashSheet {
            report: "panicked at ~/x.rs".into(),
        });
        draw(&mut c, &mut text, st);
        c.activate(Target::CrashView, st);
        assert!(c.crash.show_report);
        let r = c.activate(Target::CrashCopy, st);
        assert_eq!(r.commands, [Command::CopyText("panicked at ~/x.rs".into())]);
        let r = c.activate(Target::CrashSave, st);
        assert_eq!(r.commands, [Command::Action(Action::SaveDiagnostics)]);
        assert_eq!(c.modal(), None);
        c.show_crash(CrashSheet::default());
        named(&mut c, st, NamedKey::Enter);
        assert_eq!(c.modal(), None, "Enter is Dismiss");
    });
}

/// The diagnostics dialog: the files and their text, the names switch, Save….
#[test]
fn the_diagnostics_dialog() {
    ready(|st| {
        let mut text = Text::new();
        let mut c = chrome();
        let e = |n: &str| DiagEntry {
            name: n.into(),
            description: "d".into(),
            text: format!("text of {n}"),
        };
        c.show_diagnostics(DiagSheet {
            entries: vec![e("README.txt"), e("log.txt")],
            include_names: false,
            issues: "https://example.com/i".into(),
            email: "s@example.com".into(),
        });
        draw(&mut c, &mut text, st);
        named(&mut c, st, NamedKey::ArrowDown);
        assert_eq!(c.diag.selected, 1);
        let t = c.accessibility_tree(st, "x", 1.0);
        let texts: Vec<String> = t
            .nodes
            .iter()
            .filter_map(|(_, n)| n.value().or(n.label()).map(str::to_owned))
            .collect();
        assert!(
            texts.iter().any(|s| s.contains("text of log.txt")),
            "{texts:?}"
        );
        assert!(texts.iter().any(|s| s.contains("https://example.com/i")));
        let r = c.activate(Target::DiagNames, st);
        assert_eq!(r.commands, [Command::DiagNames(true)]);
        let r = c.activate(Target::DiagSave, st);
        assert_eq!(
            r.commands,
            [Command::DiagSave {
                include_names: true
            }]
        );
        assert_eq!(c.modal(), None);
    });
}

/// A bundle with more files than fit: the list scrolls with the wheel over it (the text
/// beside it keeps its own scroll), and the arrow keys bring the selected file into view.
#[test]
fn the_bundle_list_scrolls() {
    ready(|st| {
        let mut text = Text::new();
        let mut c = chrome();
        c.set_size(700.0, 420.0);
        let entries: Vec<DiagEntry> = (0..60)
            .map(|i| DiagEntry {
                name: format!("file-{i}.txt"),
                description: "d".into(),
                text: "line\n".repeat(200),
            })
            .collect();
        c.show_diagnostics(DiagSheet {
            entries,
            include_names: false,
            issues: "https://example.com/i".into(),
            email: "s@example.com".into(),
        });
        draw(&mut c, &mut text, st);
        let list = c.diag.list.expect("the list was drawn");
        let p = (list.x + list.w * 0.5, list.y + list.h * 0.5);
        let wheel = |c: &mut Chrome, dy: f32| {
            c.input(
                &Input::Wheel {
                    x: p.0,
                    y: p.1,
                    dx: 0.0,
                    dy,
                },
                st,
                2.0,
            );
        };
        wheel(&mut c, -5.0);
        draw(&mut c, &mut text, st);
        assert!(c.diag.list_scroll > 0.0, "the list scrolled");
        assert_eq!(c.diag.scroll, 0.0, "not the text");
        wheel(&mut c, 1000.0);
        draw(&mut c, &mut text, st);
        assert_eq!(c.diag.list_scroll, 0.0, "clamped at the top");
        for _ in 0..59 {
            named(&mut c, st, NamedKey::ArrowDown);
        }
        draw(&mut c, &mut text, st);
        assert_eq!(c.diag.selected, 59);
        let bottom = c.diag.list_scroll;
        assert!(bottom > 0.0, "the last file brought into view");
        wheel(&mut c, -1000.0);
        draw(&mut c, &mut text, st);
        assert_eq!(c.diag.list_scroll, bottom, "clamped at the bottom");
        for _ in 0..59 {
            named(&mut c, st, NamedKey::ArrowUp);
        }
        draw(&mut c, &mut text, st);
        assert_eq!(c.diag.list_scroll, 0.0, "the first file brought into view");
    });
}
