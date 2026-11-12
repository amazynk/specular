//! Whether a key press types a character into a text field, from its modifiers and the text
//! the platform made of it.
//!
//! The same physical keys reach the app differently on each platform:
//!
//! - **Windows** reports AltGr as Ctrl+Alt (both held), with the character the layout puts
//!   there (`@` on German AltGr+Q, `€` on AltGr+E, `ą` on Polish AltGr+A). A Ctrl+Alt press
//!   that the layout does not map types nothing (no text, or a control character).
//! - **macOS** reports Option as Alt, with the character Option makes (`å`, `ø`, `@` on
//!   German Option+L). Command and Control presses (`ctrl` and `meta`, [`Mods::held`]) are
//!   commands.
//! - **Linux and the BSDs** (X11 and Wayland, through xkb) treat AltGr as its own level
//!   (ISO_Level3_Shift): no Ctrl or Alt is reported, only the character. Alt and Ctrl+Alt
//!   presses are commands.
//!
//! A text field types what [`typed_text`] returns and leaves everything else to shortcuts,
//! so a Develop Ctrl+Alt shortcut never fires while AltGr types a character.

use crate::keymap::Mods;

/// The platform whose key events are read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    /// Linux, the BSDs and anything else using xkb.
    Unix,
}

impl Os {
    /// The platform this build runs on.
    pub const fn current() -> Os {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Unix
        }
    }
}

/// What a press with `mods` types on `os`, given the `text` the platform made of it: the
/// printable characters, or `None` when it types nothing and is a command (or nothing).
pub fn typed_text(os: Os, mods: Mods, text: Option<&str>) -> Option<String> {
    let s: String = text?.chars().filter(|c| !c.is_control()).collect();
    if s.is_empty() || mods.meta {
        return None;
    }
    let types = match (mods.ctrl, mods.alt) {
        (false, false) => true,
        // AltGr on Windows.
        (true, true) => os == Os::Windows,
        // Option on macOS.
        (false, true) => os == Os::MacOs,
        (true, false) => false,
    };
    types.then_some(s)
}

/// True when the press is AltGr (Windows) or Option (macOS) typing a character: it has Ctrl
/// or Alt held but [`typed_text`] says it types. A text field takes it as text, never as a
/// shortcut.
pub fn is_altgr_text(os: Os, mods: Mods, text: Option<&str>) -> bool {
    (mods.ctrl || mods.alt) && typed_text(os, mods, text).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL_ALT: Mods = Mods {
        ctrl: true,
        shift: false,
        alt: true,
        meta: false,
    };
    const ALT: Mods = Mods {
        ctrl: false,
        shift: false,
        alt: true,
        meta: false,
    };

    #[test]
    fn windows_altgr_arrives_as_ctrl_alt_and_types() {
        let w = Os::Windows;
        // German AltGr+Q, AltGr+E; Polish AltGr+A; with Shift too.
        assert_eq!(typed_text(w, CTRL_ALT, Some("@")).as_deref(), Some("@"));
        assert_eq!(typed_text(w, CTRL_ALT, Some("€")).as_deref(), Some("€"));
        let shifted = Mods {
            shift: true,
            ..CTRL_ALT
        };
        assert_eq!(typed_text(w, shifted, Some("Ą")).as_deref(), Some("Ą"));
        assert!(is_altgr_text(w, CTRL_ALT, Some("€")));
        // A US layout's Ctrl+Alt+E types nothing: a shortcut.
        assert_eq!(typed_text(w, CTRL_ALT, None), None);
        assert_eq!(typed_text(w, CTRL_ALT, Some("\u{5}")), None);
        assert!(!is_altgr_text(w, CTRL_ALT, None));
        // Ctrl+C makes a control character; Alt alone is the menu key.
        assert_eq!(typed_text(w, Mods::CTRL, Some("\u{3}")), None);
        assert_eq!(typed_text(w, Mods::CTRL, Some("c")), None);
        assert_eq!(typed_text(w, ALT, Some("e")), None);
    }

    #[test]
    fn macos_option_types_and_command_does_not() {
        let m = Os::MacOs;
        assert_eq!(typed_text(m, ALT, Some("å")).as_deref(), Some("å"));
        // German Option+L.
        assert_eq!(typed_text(m, ALT, Some("@")).as_deref(), Some("@"));
        assert!(is_altgr_text(m, ALT, Some("ø")));
        let cmd = Mods {
            meta: true,
            ..Mods::NONE
        };
        assert_eq!(typed_text(m, cmd, Some("c")), None);
        assert_eq!(typed_text(m, CTRL_ALT, Some("e")), None);
    }

    #[test]
    fn xkb_altgr_arrives_without_modifiers() {
        let u = Os::Unix;
        // AltGr is its own level: only the character arrives.
        assert_eq!(typed_text(u, Mods::NONE, Some("€")).as_deref(), Some("€"));
        assert!(!is_altgr_text(u, Mods::NONE, Some("€")));
        // Ctrl+Alt and Alt are commands, whatever text comes with them.
        assert_eq!(typed_text(u, CTRL_ALT, Some("e")), None);
        assert_eq!(typed_text(u, ALT, Some("e")), None);
        assert!(!is_altgr_text(u, CTRL_ALT, Some("e")));
    }

    #[test]
    fn plain_typing_everywhere() {
        for os in [Os::Windows, Os::MacOs, Os::Unix] {
            assert_eq!(typed_text(os, Mods::NONE, Some("a")).as_deref(), Some("a"));
            assert_eq!(typed_text(os, Mods::SHIFT, Some("A")).as_deref(), Some("A"));
            assert_eq!(typed_text(os, Mods::NONE, Some("\r")), None);
            assert_eq!(typed_text(os, Mods::NONE, None), None);
        }
    }
}
