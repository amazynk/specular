//! Buttons of navigation devices (gamepads, 3Dconnexion SpaceMouse) as bindable keys.
//!
//! A gamepad button is [`NamedKey::Pad`], a SpaceMouse button [`NamedKey::Puck`]. They bind in
//! `keymap.toml` and the key editor like any key (`"Pad-A" = "zoom-fit"`). Buttons with no
//! binding of their own run [`DEVICE_KEYS`], so the keyboard's layers (and the cheat sheet)
//! stay as they were for people without a device.

use crate::{Action, Key, KeyCombo, Mods, NamedKey};

/// A gamepad button, named by position with the Xbox labels (A is the bottom face button:
/// Cross on PlayStation, B on Nintendo).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PadButton {
    /// Bottom face button.
    A,
    /// Right face button.
    B,
    /// Left face button.
    X,
    /// Top face button.
    Y,
    /// Left shoulder.
    Lb,
    /// Right shoulder.
    Rb,
    /// Left stick pressed.
    Ls,
    /// Right stick pressed.
    Rs,
    /// Back / Select / Share / −.
    Back,
    /// Start / Menu / Options / +.
    Start,
    /// The logo button (the system often takes it).
    Guide,
    Up,
    Down,
    Left,
    Right,
}

impl PadButton {
    pub const ALL: [PadButton; 15] = [
        PadButton::A,
        PadButton::B,
        PadButton::X,
        PadButton::Y,
        PadButton::Lb,
        PadButton::Rb,
        PadButton::Ls,
        PadButton::Rs,
        PadButton::Back,
        PadButton::Start,
        PadButton::Guide,
        PadButton::Up,
        PadButton::Down,
        PadButton::Left,
        PadButton::Right,
    ];

    /// The name after `Pad-` in `keymap.toml`.
    pub fn id(self) -> &'static str {
        match self {
            PadButton::A => "A",
            PadButton::B => "B",
            PadButton::X => "X",
            PadButton::Y => "Y",
            PadButton::Lb => "LB",
            PadButton::Rb => "RB",
            PadButton::Ls => "LS",
            PadButton::Rs => "RS",
            PadButton::Back => "Back",
            PadButton::Start => "Start",
            PadButton::Guide => "Guide",
            PadButton::Up => "Up",
            PadButton::Down => "Down",
            PadButton::Left => "Left",
            PadButton::Right => "Right",
        }
    }

    /// The display form after "Pad " (arrows for the d-pad).
    pub fn label(self) -> &'static str {
        match self {
            PadButton::Up => "↑",
            PadButton::Down => "↓",
            PadButton::Left => "←",
            PadButton::Right => "→",
            o => o.id(),
        }
    }

    pub fn from_id(s: &str) -> Option<PadButton> {
        PadButton::ALL
            .into_iter()
            .find(|b| b.id().eq_ignore_ascii_case(s.trim()))
    }

    /// The key this button stands for.
    pub fn combo(self) -> KeyCombo {
        KeyCombo::new(Key::Named(NamedKey::Pad(self)), Mods::NONE)
    }
}

/// The highest SpaceMouse button number accepted (the Enterprise has 31).
pub const PUCK_BUTTONS: u8 = 32;

/// The key a SpaceMouse button (1-based) stands for, if in range.
pub fn puck_combo(n: u8) -> Option<KeyCombo> {
    (1..=PUCK_BUTTONS)
        .contains(&n)
        .then(|| KeyCombo::new(Key::Named(NamedKey::Puck(n)), Mods::NONE))
}

/// What device buttons do when the keymap binds them to nothing: next and previous on the
/// d-pad and shoulders, fit, 1:1 and Info on the face buttons; the SpaceMouse's two buttons
/// fit and 1:1.
pub const DEVICE_KEYS: &[(&str, Action)] = &[
    ("Pad-Right", Action::NextImage),
    ("Pad-RB", Action::NextImage),
    ("Pad-Left", Action::PrevImage),
    ("Pad-LB", Action::PrevImage),
    ("Pad-Up", Action::ZoomIn),
    ("Pad-Down", Action::ZoomOut),
    ("Pad-A", Action::ZoomFit),
    ("Pad-X", Action::ZoomTo(100.0)),
    ("Pad-Y", Action::ToggleInfo),
    ("Pad-B", Action::Back),
    ("Puck-1", Action::ZoomFit),
    ("Puck-2", Action::ZoomTo(100.0)),
];

/// The default action of a device button (`None` for a keyboard key or an unbound button).
pub fn device_default(combo: &KeyCombo) -> Option<Action> {
    if !combo.is_device() {
        return None;
    }
    DEVICE_KEYS
        .iter()
        .find(|(k, _)| KeyCombo::parse(k).as_ref() == Some(combo))
        .map(|(_, a)| *a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_keys_parse_and_round_trip() {
        for (k, _) in DEVICE_KEYS {
            let c = KeyCombo::parse(k).unwrap_or_else(|| panic!("{k}"));
            assert!(c.is_device(), "{k}");
            assert_eq!(KeyCombo::parse(&c.spec()), Some(c), "{k}");
        }
        for b in PadButton::ALL {
            let c = b.combo();
            assert_eq!(KeyCombo::parse(&c.spec()), Some(c));
            assert_eq!(KeyCombo::parse(&c.spec().to_lowercase()), Some(c));
        }
        assert_eq!(KeyCombo::parse("Puck-3"), puck_combo(3));
        assert_eq!(KeyCombo::parse("Puck-0"), None);
        assert_eq!(KeyCombo::parse("Puck-33"), None);
        assert_eq!(KeyCombo::parse("Pad-Q"), None);
        assert_eq!(PadButton::A.combo().to_string(), "Pad A");
        assert_eq!(PadButton::Left.combo().to_string(), "Pad ←");
        assert_eq!(
            puck_combo(1).map(|c| c.to_string()),
            Some("SpaceMouse 1".into())
        );
    }

    #[test]
    fn defaults_cover_the_brief() {
        let d = |k: &str| device_default(&KeyCombo::parse(k).unwrap());
        assert_eq!(d("Pad-Right"), Some(Action::NextImage));
        assert_eq!(d("Pad-Left"), Some(Action::PrevImage));
        assert_eq!(d("Pad-A"), Some(Action::ZoomFit));
        assert_eq!(d("Pad-X"), Some(Action::ZoomTo(100.0)));
        assert_eq!(d("Pad-Y"), Some(Action::ToggleInfo));
        assert_eq!(d("Puck-1"), Some(Action::ZoomFit));
        assert_eq!(d("Puck-2"), Some(Action::ZoomTo(100.0)));
        assert_eq!(d("Pad-Guide"), None);
        assert_eq!(device_default(&KeyCombo::parse("f").unwrap()), None);
    }

    #[test]
    fn keymap_binding_overrides_default() {
        let mut km = crate::Keymap::viewer_default();
        let errs = km.apply_toml("[viewer]\n\"Pad-A\" = \"next\"\n");
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(km.lookup(&PadButton::A.combo()), Some(Action::NextImage));
        // "none" switches a default off, and survives a round trip through the file.
        let mut km = crate::Keymap::viewer_default();
        let errs = km.apply_toml("[viewer]\n\"Pad-A\" = \"none\"\n");
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(km.device_action(&PadButton::A.combo()), None);
        assert_eq!(
            km.device_action(&PadButton::X.combo()),
            Some(Action::ZoomTo(100.0))
        );
        let text = km.to_toml();
        assert!(text.contains("\"Pad-A\" = \"none\""), "{text}");
        let mut again = crate::Keymap::viewer_default();
        assert!(again.apply_toml(&text).is_empty());
        assert_eq!(again.device_action(&PadButton::A.combo()), None);
        // Binding it again, or resetting, brings it back.
        again.apply_toml("[viewer]\n\"Pad-A\" = \"next\"\n");
        assert_eq!(
            again.device_action(&PadButton::A.combo()),
            Some(Action::NextImage)
        );
        km.apply_edit(&crate::KeymapEdit::ResetAll);
        assert_eq!(
            km.device_action(&PadButton::A.combo()),
            Some(Action::ZoomFit)
        );
        // The editor's unbind of a default does the same.
        assert!(km.apply_edit(&crate::KeymapEdit::Unbind {
            mode: crate::Mode::Viewer,
            keys: crate::KeySeq::one(PadButton::A.combo()),
        }));
        assert_eq!(km.device_action(&PadButton::A.combo()), None);
        // Default keymap layers bind no device key.
        let km = crate::Keymap::viewer_default();
        for m in crate::Mode::ALL {
            assert!(km.bindings(m).iter().all(|(s, _)| !s.first().is_device()));
        }
    }
}
