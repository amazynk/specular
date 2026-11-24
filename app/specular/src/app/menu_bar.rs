//! The system menu bar (macOS): the application menu with About, Settings… and Quit, the
//! File, Edit, View, Go and Help menus of the app's own actions, and the Window menu.
//!
//! Every item of the app's own is an [`Action`], named as the palette names it and run the
//! way the palette runs it.
//!
//! Only the application menu's items show a key (Settings… with ⌘,). The system acts on a
//! key that a menu shows: the press runs the item and never reaches the window, so it
//! would skip everything the key handling knows (the dialog that is open, the field being
//! typed in, the keys of the mode). An item of the application menu needs no more than
//! "nothing modal is up", and gets its key from the keymap when that key has ⌘. The keys of
//! every other action are the keymap's alone, as before there was a menu; the cheat sheet
//! and the palette show them.
//!
//! While a dialog is up or a field is typed in, the app's items are greyed and choose
//! nothing ([`App::menu_bar_gate`]): the keys of the same actions do nothing there either,
//! and the dialogs count on it (nothing changes the image under the print dialog).
//!
//! The menus are laid out again whenever the language, the keymap or the profile changes
//! ([`App::menu_bar_sync`]), and put in the bar only when they differ from what is there.
//!
//! On other systems there is no such bar, and nothing here runs.

use std::cell::RefCell;
use std::rc::Rc;

use specular_gfx::winit::event_loop::ActiveEventLoop;
use specular_i18n::t;
use specular_input::{Action, Key, KeyCombo, Keymap, Mode, NamedKey};
use specular_log::{INPUT, debug};
use specular_platform::menu_bar::{Entry, KeyEquivalent, Menu, MenuBar, MenuKey, Role, System};

use super::App;
use super::frame_gate::Cause;

/// One line of a menu, before it has a title and a key.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Line {
    Act(Action),
    Sys(System),
    Sep,
}

use Line::{Act, Sep, Sys};

const APPLICATION: &[Line] = &[
    Act(Action::About),
    Act(Action::CheckForUpdates),
    Sep,
    Act(Action::Settings),
    Sep,
    Sys(System::Services),
    Sep,
    Sys(System::Hide),
    Sys(System::HideOthers),
    Sys(System::ShowAll),
    Sep,
    Sys(System::Quit),
];

const FILE: &[Line] = &[
    Act(Action::Open),
    Act(Action::OpenAddress),
    Act(Action::OpenStream),
    Sep,
    Act(Action::Reload),
    Sep,
    Act(Action::Rename),
    Act(Action::MoveToTrash),
    Act(Action::Convert),
    Sep,
    Act(Action::Share),
    Act(Action::Print),
    Sep,
    Act(Action::Close),
];

const EDIT: &[Line] = &[
    Act(Action::Undo),
    Act(Action::Redo),
    Sep,
    Act(Action::CopyImage),
    Act(Action::Paste),
];

const VIEW: &[Line] = &[
    Act(Action::ZoomIn),
    Act(Action::ZoomOut),
    Act(Action::ZoomFit),
    Act(Action::ZoomTo(100.0)),
    Sep,
    Act(Action::RotateLeft),
    Act(Action::RotateRight),
    Sep,
    Act(Action::ToggleInfo),
    Act(Action::ToggleGallery),
    Act(Action::Slideshow),
    Sep,
    Act(Action::ToggleTheme),
    Act(Action::CustomiseToolbar),
    Sep,
    // The system's own item, so the bar has it once in every language (the system adds
    // one itself only to a menu titled "View" in English).
    Sys(System::FullScreen),
];

const GO: &[Line] = &[
    Act(Action::NextImage),
    Act(Action::PrevImage),
    Act(Action::FirstImage),
    Act(Action::LastImage),
    Sep,
    Act(Action::GoToImage),
];

const WINDOW: &[Line] = &[
    Sys(System::Minimize),
    Sys(System::Zoom),
    Sep,
    Sys(System::BringAllToFront),
];

const HELP: &[Line] = &[
    Act(Action::Help),
    Act(Action::CheatSheet),
    Act(Action::Palette),
    Act(Action::ShowWelcome),
    Sep,
    Act(Action::JoinDiscord),
    Act(Action::GetPagebara),
    Act(Action::GetDozed),
    Sep,
    Act(Action::SaveDiagnostics),
];

/// The menus in the bar's order, with what each is to the system.
const MENUS: [(Role, &[Line]); 7] = [
    (Role::Application, APPLICATION),
    (Role::Plain, FILE),
    (Role::Plain, EDIT),
    (Role::Plain, VIEW),
    (Role::Plain, GO),
    (Role::Window, WINDOW),
    (Role::Help, HELP),
];

/// The bar as it is installed, and the items chosen in it that have not been run yet.
pub(super) struct MenuBarState {
    /// What is in the bar: `None` until the first sync (and for good where there is no bar).
    shown: Option<Vec<Menu>>,
    /// The action of each item's id.
    actions: Vec<Action>,
    /// The ids chosen, in order. Filled by the menu's handler, emptied by the event loop.
    chosen: Rc<RefCell<Vec<usize>>>,
    bar: Option<MenuBar>,
    /// Whether the bar's items can be chosen ([`App::menu_bar_gate`]).
    open: bool,
}

impl Default for MenuBarState {
    fn default() -> Self {
        Self {
            shown: None,
            actions: Vec::new(),
            chosen: Rc::default(),
            bar: None,
            // A bar's items can be chosen when it is installed.
            open: true,
        }
    }
}

/// A menu's title in the interface's language. The system titles the application menu
/// itself, with the app's name.
fn title(index: usize) -> String {
    match index {
        0 => String::new(),
        1 => t!("cheatsheet-file"),
        2 => t!("menu-edit"),
        3 => t!("cheatsheet-view"),
        4 => t!("menu-go"),
        5 => t!("cheatsheet-window"),
        _ => t!("cheatsheet-help"),
    }
}

/// The title of an item the system carries out.
fn system_title(what: System) -> String {
    match what {
        System::Services => t!("menu-services"),
        System::Hide => t!("menu-hide"),
        System::HideOthers => t!("menu-hide-others"),
        System::ShowAll => t!("menu-show-all"),
        System::Quit => Action::Quit.name(),
        System::Minimize => t!("menu-minimise"),
        System::Zoom => t!("menu-zoom"),
        System::FullScreen => specular_i18n::strip_isolates(&Action::ToggleFullscreen.name()),
        System::BringAllToFront => t!("menu-bring-all-to-front"),
    }
}

/// An action's title in a menu: its name without the marks that isolate a number in running
/// text (a menu's title is the system's to draw), and for Settings with the ellipsis the
/// system's own menus give it (its name has none, being also the window's title).
fn action_title(a: Action) -> String {
    let name = specular_i18n::strip_isolates(&a.name());
    if a == Action::Settings && !name.ends_with('…') {
        format!("{name}…")
    } else {
        name
    }
}

/// The key a menu shows for one press of the keymap, if a menu can show it.
fn key_equivalent(c: KeyCombo) -> Option<KeyEquivalent> {
    let key = match c.key {
        Key::Char(ch) => MenuKey::Char(ch),
        Key::Named(n) => match n {
            NamedKey::ArrowLeft => MenuKey::Left,
            NamedKey::ArrowRight => MenuKey::Right,
            NamedKey::ArrowUp => MenuKey::Up,
            NamedKey::ArrowDown => MenuKey::Down,
            NamedKey::Space => MenuKey::Space,
            NamedKey::Enter => MenuKey::Enter,
            NamedKey::Escape => MenuKey::Escape,
            NamedKey::Backspace => MenuKey::Backspace,
            NamedKey::Delete => MenuKey::Delete,
            NamedKey::Tab => MenuKey::Tab,
            NamedKey::Home => MenuKey::Home,
            NamedKey::End => MenuKey::End,
            NamedKey::PageUp => MenuKey::PageUp,
            NamedKey::PageDown => MenuKey::PageDown,
            NamedKey::F(n) => MenuKey::F(n),
            // No key of a Mac keyboard, or not a keyboard's key at all.
            NamedKey::Insert | NamedKey::Pad(_) | NamedKey::Puck(_) => return None,
        },
    };
    // The keymap writes ⌘ as Ctrl and Control as Meta on macOS (`Mods::held`).
    Some(KeyEquivalent {
        key,
        command: c.mods.ctrl,
        shift: c.mods.shift,
        option: c.mods.alt,
        control: c.mods.meta,
    })
}

/// The key of an item of the application menu: the action's first key in the viewer's layer
/// that is one press with ⌘. The system takes a menu's key from the window, so one without
/// ⌘ is never given (a plain letter could no longer be typed anywhere).
fn application_key(keymap: &Keymap, a: Action) -> Option<KeyEquivalent> {
    keymap
        .keys_in(Mode::Viewer, a)
        .iter()
        .filter(|seq| seq.len() == 1)
        .filter_map(|seq| key_equivalent(seq.first()))
        .find(|k| k.command)
}

/// The menus for `keymap` in the interface's language, and the action of each item's id.
/// Actions the profile hides are left out, with any separator that would then stand first,
/// last or doubled.
fn layout(keymap: &Keymap) -> (Vec<Menu>, Vec<Action>) {
    let mut actions = Vec::new();
    let mut menus = Vec::new();
    for (index, (role, lines)) in MENUS.into_iter().enumerate() {
        let mut entries: Vec<Entry> = Vec::new();
        for line in lines {
            match *line {
                Sep => {
                    if !matches!(entries.last(), None | Some(Entry::Separator)) {
                        entries.push(Entry::Separator);
                    }
                }
                Sys(what) => entries.push(Entry::System {
                    what,
                    title: system_title(what),
                }),
                Act(a) => {
                    if !keymap.shows(a) {
                        continue;
                    }
                    entries.push(Entry::Item {
                        id: actions.len(),
                        title: action_title(a),
                        key: (role == Role::Application)
                            .then(|| application_key(keymap, a))
                            .flatten(),
                    });
                    actions.push(a);
                }
            }
        }
        if entries.last() == Some(&Entry::Separator) {
            entries.pop();
        }
        if !entries.is_empty() {
            menus.push(Menu {
                title: title(index),
                role,
                entries,
            });
        }
    }
    (menus, actions)
}

impl App {
    /// Puts the menus in the system's bar, or replaces them when their titles or keys have
    /// changed (another language, another keymap, another profile). Not before the window
    /// exists (the application has started by then), and not in benchmarks and screenshots,
    /// which have no one to choose from a menu.
    pub(super) fn menu_bar_sync(&mut self) {
        if !cfg!(target_os = "macos") || self.headless() || self.gfx.is_none() {
            return;
        }
        let (menus, actions) = layout(&self.keymap);
        if self.menu_bar.shown.as_ref() == Some(&menus) {
            return;
        }
        let chosen = self.menu_bar.chosen.clone();
        let wake = self.loader.waker();
        // The old bar is dropped only after the new one is in place.
        let bar = MenuBar::install(&menus, move |id| {
            chosen.borrow_mut().push(id);
            wake();
        });
        if let Some(bar) = &bar {
            bar.set_items_enabled(self.menu_bar.open);
        }
        self.menu_bar.bar = bar;
        self.menu_bar.actions = actions;
        self.menu_bar.shown = Some(menus);
    }

    /// Whether an item of the menu bar may run now: not under a dialog (the app's own or
    /// the chrome's) and not while a field is typed in, where the keys do nothing either.
    fn menu_bar_open(&self) -> bool {
        !self.app_dialog_open() && !self.chrome.wants_text()
    }

    /// Greys the app's items in the bar while they cannot run, and brings them back after.
    /// Called once every turn of the event loop, so the bar is right before the next press.
    pub(super) fn menu_bar_gate(&mut self) {
        let open = self.menu_bar_open();
        if self.menu_bar.open != open {
            self.menu_bar.open = open;
            if let Some(bar) = &self.menu_bar.bar {
                bar.set_items_enabled(open);
            }
        }
    }

    /// Runs the actions chosen from the menu bar since the last call.
    pub(super) fn menu_bar_requests(&mut self, el: &ActiveEventLoop) {
        let chosen = std::mem::take(&mut *self.menu_bar.chosen.borrow_mut());
        for id in chosen {
            let Some(&a) = self.menu_bar.actions.get(id) else {
                continue;
            };
            // The bar greys its items under a dialog; a choice that still arrives (the
            // dialog opened after it was made) is dropped like a key pressed there.
            if !self.menu_bar_open() {
                debug!(target: INPUT, "menu bar: {} not run: a dialog or a field has the keys", a.id());
                break;
            }
            debug!(target: INPUT, "menu bar: {}", a.id());
            self.because(Cause::Commands, |app| app.action(a, el));
        }
    }
}

#[cfg(test)]
mod tests {
    use specular_i18n::{I18n, langid};
    use specular_input::KeySeq;

    use super::*;

    fn english<T>(f: impl FnOnce() -> T) -> T {
        I18n::with_thread_language(langid!("en"), f)
    }

    fn items(menu: &Menu) -> Vec<(usize, &str, Option<KeyEquivalent>)> {
        menu.entries
            .iter()
            .filter_map(|e| match e {
                Entry::Item { id, title, key } => Some((*id, title.as_str(), *key)),
                _ => None,
            })
            .collect()
    }

    fn cmd(c: char) -> KeyEquivalent {
        KeyEquivalent {
            key: MenuKey::Char(c),
            command: true,
            shift: false,
            option: false,
            control: false,
        }
    }

    /// The application menu is what a Mac app's is: About, Settings… with ⌘, and the
    /// system's Services, Hide and Quit; then the app's menus, Window and Help.
    #[test]
    fn the_bar_is_laid_out_as_on_a_mac() {
        english(|| {
            let (menus, actions) = layout(&Keymap::viewer_default());
            let roles: Vec<Role> = menus.iter().map(|m| m.role).collect();
            assert_eq!(
                roles,
                [
                    Role::Application,
                    Role::Plain,
                    Role::Plain,
                    Role::Plain,
                    Role::Plain,
                    Role::Window,
                    Role::Help
                ]
            );
            let titles: Vec<&str> = menus.iter().map(|m| m.title.as_str()).collect();
            assert_eq!(titles, ["", "File", "Edit", "View", "Go", "Window", "Help"]);
            let app = items(&menus[0]);
            assert_eq!(app[0].1, "About Specular");
            let settings = app
                .iter()
                .find(|(id, ..)| actions[*id] == Action::Settings)
                .expect("Settings is in the application menu");
            assert_eq!(settings.1, "Settings…");
            assert_eq!(settings.2, Some(cmd(',')));
            let system: Vec<System> = menus[0]
                .entries
                .iter()
                .filter_map(|e| match e {
                    Entry::System { what, .. } => Some(*what),
                    _ => None,
                })
                .collect();
            assert_eq!(
                system,
                [
                    System::Services,
                    System::Hide,
                    System::HideOthers,
                    System::ShowAll,
                    System::Quit
                ]
            );
            // Help ends with the other apps and the diagnostics bundle.
            let help: Vec<Action> = items(menus.last().unwrap())
                .iter()
                .map(|(id, ..)| actions[*id])
                .collect();
            assert!(help.contains(&Action::GetPagebara));
            // Dozed is offered where it runs.
            assert_eq!(help.contains(&Action::GetDozed), cfg!(target_os = "linux"));
        });
    }

    /// Every item's id leads back to its action, each action is in the bar once, and no
    /// menu starts or ends with a separator or has two together.
    #[test]
    fn ids_name_their_actions_and_separators_stand_between_items() {
        english(|| {
            let (menus, actions) = layout(&Keymap::viewer_default());
            let mut seen = Vec::new();
            for m in &menus {
                assert!(!matches!(m.entries.first(), Some(Entry::Separator)));
                assert!(!matches!(m.entries.last(), Some(Entry::Separator)));
                for pair in m.entries.windows(2) {
                    assert!(pair != [Entry::Separator, Entry::Separator], "{}", m.title);
                }
                for (id, title, _) in items(m) {
                    assert_eq!(title, action_title(actions[id]));
                    assert!(!seen.contains(&id));
                    seen.push(id);
                }
            }
            assert_eq!(seen.len(), actions.len());
            for (i, a) in actions.iter().enumerate() {
                assert!(!actions[..i].contains(a), "{a:?} is in the bar twice");
            }
        });
    }

    /// Settings… follows the keymap: another ⌘ key for it shows in the menu. No item
    /// outside the application menu has a key, whatever the keymap binds.
    #[test]
    fn only_the_application_menu_shows_keys_and_they_are_the_keymaps() {
        english(|| {
            let mut km = Keymap::viewer_default();
            let (menus, _) = layout(&km);
            for m in &menus[1..] {
                for (_, title, key) in items(m) {
                    assert_eq!(key, None, "{title}");
                }
            }
            assert_eq!(application_key(&km, Action::Settings), Some(cmd(',')));
            km.unbind(KeySeq::parse("Ctrl+,").unwrap().first());
            km.bind(
                KeySeq::parse("Ctrl+Shift+p").unwrap().first(),
                Action::Settings,
            );
            assert_eq!(
                application_key(&km, Action::Settings),
                Some(KeyEquivalent {
                    shift: true,
                    ..cmd('p')
                })
            );
        });
    }

    /// The system acts on a key the application menu shows, so a key without ⌘ is not
    /// put there: bound to a plain letter, About shows no key, and the letter still types.
    #[test]
    fn the_application_menu_takes_only_command_keys() {
        english(|| {
            let mut km = Keymap::viewer_default();
            km.bind(KeySeq::parse("a").unwrap().first(), Action::About);
            assert_eq!(application_key(&km, Action::About), None);
        });
    }

    /// A number in an action's name comes without the marks that isolate it in running
    /// text: the system draws a menu's title.
    #[test]
    fn titles_carry_no_isolating_marks() {
        english(|| {
            let title = action_title(Action::ZoomTo(100.0));
            assert!(title.contains("100"), "{title}");
            assert!(
                !title.contains(['\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}']),
                "{title:?}"
            );
        });
    }

    /// Keys a Mac keyboard does not have, and buttons of other devices, show nothing.
    #[test]
    fn keys_a_menu_cannot_show_are_left_out() {
        let insert = KeySeq::parse("Insert").unwrap().first();
        assert_eq!(key_equivalent(insert), None);
        let f11 = KeySeq::parse("F11").unwrap().first();
        assert_eq!(key_equivalent(f11).map(|k| k.key), Some(MenuKey::F(11)));
    }
}
