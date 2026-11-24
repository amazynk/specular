//! The application's menu bar: on macOS the system menu bar at the top of the screen, with
//! the application menu (About, Settings…, Services, Hide, Quit), the app's own menus, and
//! the Window and Help menus the system looks after. Elsewhere there is no such bar and
//! [`MenuBar::install`] does nothing.
//!
//! The caller says what the menus hold ([`Menu`], [`Entry`]); choosing an item of the app's
//! own calls its handler with the item's id.
//!
//! An item can have a key ([`KeyEquivalent`]). The system acts on such a key itself: a
//! press of it runs the item and never reaches the window, whatever the window is showing.
//! So a caller gives a key only to an item that needs no more than "nothing modal is up"
//! (Settings… with ⌘,), and leaves the keys of everything else to its own key handling.
//!
//! While the app has something modal up it turns its own items off
//! ([`MenuBar::set_items_enabled`]): they are greyed, cannot be chosen, and a press of their
//! key goes to the window like any other. The system's items are the system's to enable.

/// A key of a [`KeyEquivalent`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKey {
    /// A character key, as typed without Shift (`o`, `,`, `1`).
    Char(char),
    /// F1 to F35.
    F(u8),
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Escape,
    Enter,
    Tab,
    Space,
    /// The Delete key above Return (Backspace on other keyboards).
    Backspace,
    /// Forward delete.
    Delete,
}

/// The key shown at the trailing edge of a menu item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEquivalent {
    pub key: MenuKey,
    pub command: bool,
    pub shift: bool,
    pub option: bool,
    pub control: bool,
}

/// An item the system carries out itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    /// The Services submenu, which the system fills.
    Services,
    /// Hide the application (⌘H).
    Hide,
    /// Hide every other application (⌥⌘H).
    HideOthers,
    ShowAll,
    /// Quit (⌘Q): the application terminates, as when quit from the Dock.
    Quit,
    /// Minimise the window (⌘M).
    Minimize,
    /// The window's zoom (the green button's old meaning).
    Zoom,
    /// The window into and out of full screen, as its green button does. The system adds
    /// such an item itself to a menu titled "View" in English only; given here, it is in
    /// the menu in every language, once.
    FullScreen,
    BringAllToFront,
}

/// One line of a menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// An item of the app's own: choosing it, or pressing its key, calls the handler with
    /// `id`.
    Item {
        id: usize,
        title: String,
        key: Option<KeyEquivalent>,
    },
    /// An item the system carries out, under the title given (its key is the system's).
    System {
        what: System,
        title: String,
    },
    Separator,
}

/// What a menu is to the system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The application menu, first in the bar. The system titles it with the app's name.
    Application,
    /// A menu of the app's own (File, View…).
    Plain,
    /// The Window menu: the system adds the open windows to it.
    Window,
    /// The Help menu: the system adds its search field.
    Help,
}

/// One menu of the bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    pub title: String,
    pub role: Role,
    pub entries: Vec<Entry>,
}

/// The installed menu bar. Keep it for as long as its menus are in the bar: their items call
/// into it. Installing another replaces the menus; drop the old one after that.
pub struct MenuBar {
    #[cfg(target_os = "macos")]
    _installed: macos::Installed,
}

impl MenuBar {
    /// Puts `menus` in the system's menu bar; `on_item` is called, on the main thread, with
    /// the id of an [`Entry::Item`] that was chosen. Call on the main thread once the
    /// application has started. `None` where there is no system menu bar (every system but
    /// macOS), or off the main thread.
    pub fn install(menus: &[Menu], on_item: impl Fn(usize) + 'static) -> Option<MenuBar> {
        #[cfg(target_os = "macos")]
        {
            macos::install(menus, Box::new(on_item)).map(|i| MenuBar { _installed: i })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (menus, &on_item);
            None
        }
    }

    /// Whether the app's own items ([`Entry::Item`]) can be chosen. Off, the system greys
    /// them and does not act on their keys. On after [`MenuBar::install`].
    pub fn set_items_enabled(&self, on: bool) {
        #[cfg(target_os = "macos")]
        self._installed.set_items_enabled(on);
        #[cfg(not(target_os = "macos"))]
        let _ = on;
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::cell::Cell;

    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
    use objc2_foundation::NSString;

    use super::{Entry, KeyEquivalent, Menu, MenuKey, Role, System};

    /// What AppKit holds without retaining (the items' target) and the menus themselves,
    /// kept for as long as the bar is installed.
    pub(super) struct Installed {
        target: Retained<MenuTarget>,
        _main: Retained<NSMenu>,
    }

    impl Installed {
        pub(super) fn set_items_enabled(&self, on: bool) {
            self.target.ivars().enabled.set(on);
        }
    }

    struct Ivars {
        on_item: Box<dyn Fn(usize)>,
        /// Whether the items can be chosen now (`validateMenuItem:`).
        enabled: Cell<bool>,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and `MenuTarget` has no `Drop`.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "SpecularMenuTarget"]
        #[ivars = Ivars]
        struct MenuTarget;

        impl MenuTarget {
            /// The action of every item of the app's own: the item's tag is its id.
            #[unsafe(method(specularMenuItem:))]
            fn item_chosen(&self, sender: &NSMenuItem) {
                (self.ivars().on_item)(sender.tag() as usize);
            }

            /// AppKit asks this of an item's target before it shows the item and before it
            /// acts on the item's key.
            #[unsafe(method(validateMenuItem:))]
            fn validate(&self, _item: &NSMenuItem) -> bool {
                self.ivars().enabled.get()
            }
        }

        // SAFETY: `NSObjectProtocol` has no requirements beyond NSObject's.
        unsafe impl NSObjectProtocol for MenuTarget {}

    );

    impl MenuTarget {
        #[allow(unsafe_code)]
        fn new(mtm: MainThreadMarker, on_item: Box<dyn Fn(usize)>) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(Ivars {
                on_item,
                enabled: Cell::new(true),
            });
            // SAFETY: NSObject's `init` on a freshly allocated object whose ivars are set.
            unsafe { msg_send![super(this), init] }
        }
    }

    pub(super) fn install(menus: &[Menu], on_item: Box<dyn Fn(usize)>) -> Option<Installed> {
        let mtm = MainThreadMarker::new()?;
        let app = NSApplication::sharedApplication(mtm);
        let target = MenuTarget::new(mtm, on_item);
        let main = NSMenu::new(mtm);
        for menu in menus {
            let m = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str(&menu.title));
            for entry in &menu.entries {
                match entry {
                    Entry::Separator => m.addItem(&NSMenuItem::separatorItem(mtm)),
                    Entry::Item { id, title, key } => {
                        let item = own_item(mtm, &target, *id, title);
                        if let Some(key) = key {
                            set_key(&item, *key);
                        }
                        m.addItem(&item);
                    }
                    Entry::System { what, title } => {
                        m.addItem(&system_item(mtm, &app, *what, title));
                    }
                }
            }
            match menu.role {
                Role::Window => app.setWindowsMenu(Some(&m)),
                Role::Help => app.setHelpMenu(Some(&m)),
                Role::Application | Role::Plain => {}
            }
            let top = NSMenuItem::new(mtm);
            top.setTitle(&NSString::from_str(&menu.title));
            top.setSubmenu(Some(&m));
            main.addItem(&top);
        }
        app.setMainMenu(Some(&main));
        Some(Installed {
            target,
            _main: main,
        })
    }

    #[allow(unsafe_code)]
    fn own_item(
        mtm: MainThreadMarker,
        target: &MenuTarget,
        id: usize,
        title: &str,
    ) -> Retained<NSMenuItem> {
        let item = NSMenuItem::new(mtm);
        item.setTitle(&NSString::from_str(title));
        item.setTag(id as isize);
        // SAFETY: the caller keeps `target` alive for as long as the menus are installed
        // (`Installed`), which AppKit, holding it weakly, relies on.
        unsafe { item.setTarget(Some(target)) };
        // SAFETY: `target` answers `specularMenuItem:` with an `NSMenuItem` argument (the
        // class above).
        unsafe { item.setAction(Some(sel!(specularMenuItem:))) };
        item
    }

    /// An item whose action goes to the first responder or the application, as in every
    /// AppKit app's menu.
    #[allow(unsafe_code)]
    fn system_item(
        mtm: MainThreadMarker,
        app: &NSApplication,
        what: System,
        title: &str,
    ) -> Retained<NSMenuItem> {
        let item = NSMenuItem::new(mtm);
        item.setTitle(&NSString::from_str(title));
        let cmd = |key: char, option: bool| KeyEquivalent {
            key: MenuKey::Char(key),
            command: true,
            shift: false,
            option,
            control: false,
        };
        let (action, key) = match what {
            System::Services => {
                let services = NSMenu::new(mtm);
                item.setSubmenu(Some(&services));
                app.setServicesMenu(Some(&services));
                return item;
            }
            System::Hide => (sel!(hide:), Some(cmd('h', false))),
            System::HideOthers => (sel!(hideOtherApplications:), Some(cmd('h', true))),
            System::ShowAll => (sel!(unhideAllApplications:), None),
            System::Quit => (sel!(terminate:), Some(cmd('q', false))),
            System::Minimize => (sel!(performMiniaturize:), Some(cmd('m', false))),
            System::Zoom => (sel!(performZoom:), None),
            System::FullScreen => (sel!(toggleFullScreen:), None),
            System::BringAllToFront => (sel!(arrangeInFront:), None),
        };
        // SAFETY: each selector is a standard action of NSApplication or NSWindow taking
        // the sender; with no target, AppKit sends it along the responder chain and
        // disables the item when nothing answers.
        unsafe { item.setAction(Some(action)) };
        if let Some(key) = key {
            set_key(&item, key);
        }
        item
    }

    fn set_key(item: &NSMenuItem, key: KeyEquivalent) {
        let Some(c) = key_char(key.key) else {
            return;
        };
        let mut flags = NSEventModifierFlags::empty();
        for (held, flag) in [
            (key.command, NSEventModifierFlags::Command),
            (key.shift, NSEventModifierFlags::Shift),
            (key.option, NSEventModifierFlags::Option),
            (key.control, NSEventModifierFlags::Control),
        ] {
            if held {
                flags |= flag;
            }
        }
        item.setKeyEquivalent(&NSString::from_str(&c.to_string()));
        item.setKeyEquivalentModifierMask(flags);
    }

    /// The character AppKit knows the key by (its function-key code points for the keys
    /// that type nothing).
    pub(super) fn key_char(key: MenuKey) -> Option<char> {
        let code = match key {
            // An upper-case letter would mean "with Shift" to AppKit; Shift is its own flag.
            MenuKey::Char(c) => return c.to_lowercase().next(),
            MenuKey::F(n @ 1..=35) => 0xF704 + u32::from(n) - 1,
            MenuKey::F(_) => return None,
            MenuKey::Up => 0xF700,
            MenuKey::Down => 0xF701,
            MenuKey::Left => 0xF702,
            MenuKey::Right => 0xF703,
            MenuKey::Delete => 0xF728,
            MenuKey::Home => 0xF729,
            MenuKey::End => 0xF72B,
            MenuKey::PageUp => 0xF72C,
            MenuKey::PageDown => 0xF72D,
            MenuKey::Escape => 0x1B,
            MenuKey::Enter => 0x0D,
            MenuKey::Tab => 0x09,
            MenuKey::Space => 0x20,
            MenuKey::Backspace => 0x08,
        };
        char::from_u32(code)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The keys that type nothing are AppKit's function-key code points; letters lose
        /// their case (Shift is a flag of its own).
        #[test]
        fn keys_are_the_characters_appkit_knows() {
            assert_eq!(key_char(MenuKey::Char('O')), Some('o'));
            assert_eq!(key_char(MenuKey::Char(',')), Some(','));
            assert_eq!(key_char(MenuKey::F(1)), Some('\u{F704}'));
            assert_eq!(key_char(MenuKey::F(11)), Some('\u{F70E}'));
            assert_eq!(key_char(MenuKey::F(0)), None);
            assert_eq!(key_char(MenuKey::F(36)), None);
            assert_eq!(key_char(MenuKey::Left), Some('\u{F702}'));
            assert_eq!(key_char(MenuKey::Escape), Some('\u{1b}'));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Off the main thread (a test's thread) nothing is installed, and nothing is called.
    #[test]
    fn nothing_is_installed_off_the_main_thread() {
        let menus = [Menu {
            title: "File".to_owned(),
            role: Role::Plain,
            entries: vec![Entry::Separator],
        }];
        assert!(MenuBar::install(&menus, |_| panic!("no item was chosen")).is_none());
    }
}
