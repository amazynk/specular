//! Conflict checks for a new binding (DESIGN §11): the same keys in the mode, prefix clashes
//! (`g` when `g g` exists), shadowing between a mode and its parent, and shortcuts the
//! operating system or input methods take before an application sees them.

use crate::{Action, Key, KeyCombo, KeySeq, Keymap, Mode, NamedKey};

/// Something a new binding collides with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Conflict {
    /// The keys are already bound to this action.
    Already,
    /// The keys run `other` in this mode.
    SameKey { other: Action },
    /// The new keys begin `keys` (bound to `other`), or `keys` begin the new ones: the shorter
    /// one then runs only after the sequence timeout.
    Prefix { keys: KeySeq, other: Action },
    /// `mode` (the parent or a child of this mode) binds the same keys to `other`; in the
    /// child mode its binding wins.
    Shadow { mode: Mode, other: Action },
    /// The keys may never reach Specular.
    Reserved(Reserved),
}

/// Why a shortcut is reserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reserved {
    /// Window switching (Alt+Tab, Cmd+Tab).
    SwitchWindows,
    /// Closes the window (Alt+F4).
    CloseWindow,
    /// Ctrl+Alt+Delete.
    Secure,
    /// Super (Windows) key shortcuts belong to the desktop.
    Desktop,
    /// Ctrl+Alt+F1…F12 switch virtual terminals; Ctrl+Alt+arrows switch workspaces.
    Workspaces,
    /// Input method and keyboard layout switching (Ctrl+Space, Super+Space).
    InputMethod,
    /// macOS system shortcuts (Cmd+H, Cmd+M, Cmd+Space, Cmd+Q outside Quit).
    MacOs,
}

impl Reserved {
    /// Fluent message ID explaining it.
    pub fn message_id(self) -> &'static str {
        match self {
            Reserved::SwitchWindows => "keys-reserved-switch",
            Reserved::CloseWindow => "keys-reserved-close",
            Reserved::Secure => "keys-reserved-secure",
            Reserved::Desktop => "keys-reserved-desktop",
            Reserved::Workspaces => "keys-reserved-workspaces",
            Reserved::InputMethod => "keys-reserved-input",
            Reserved::MacOs => "keys-reserved-macos",
        }
    }
}

/// Whether the operating system or the input method takes this key before Specular sees it.
/// `action` exempts the shortcut that already means it (Cmd+Q for Quit).
pub fn reserved(c: &KeyCombo, action: Action) -> Option<Reserved> {
    let m = c.mods;
    let named = |n: NamedKey| c.key == Key::Named(n);
    let ch = |x: char| c.key == Key::Char(x);
    let f = match c.key {
        Key::Named(NamedKey::F(n)) => Some(n),
        _ => None,
    };
    let arrow = matches!(
        c.key,
        Key::Named(
            NamedKey::ArrowLeft | NamedKey::ArrowRight | NamedKey::ArrowUp | NamedKey::ArrowDown
        )
    );
    let space = named(NamedKey::Space) || ch(' ');
    if cfg!(target_os = "macos") {
        // Command is `ctrl` here (the keymap's primary modifier).
        if m.ctrl && named(NamedKey::Tab) {
            return Some(Reserved::SwitchWindows);
        }
        if m.ctrl && space || m.meta && space {
            return Some(Reserved::InputMethod);
        }
        // Command+H hides the app and Command+M minimises the window; with Shift they are
        // free.
        if m.ctrl && !m.alt && !m.shift && (ch('h') || ch('m')) {
            return Some(Reserved::MacOs);
        }
        if m.ctrl && ch('q') && action != Action::Quit {
            return Some(Reserved::MacOs);
        }
        return None;
    }
    if m.alt && !m.ctrl && named(NamedKey::Tab) {
        return Some(Reserved::SwitchWindows);
    }
    if m.alt && !m.ctrl && f == Some(4) {
        return Some(Reserved::CloseWindow);
    }
    if m.ctrl && m.alt && (named(NamedKey::Delete) || named(NamedKey::Backspace)) {
        return Some(Reserved::Secure);
    }
    if m.ctrl && m.alt && (f.is_some_and(|n| n <= 12) || arrow) {
        return Some(Reserved::Workspaces);
    }
    if space && (m.meta || m.ctrl && !m.alt && !m.shift) {
        return Some(Reserved::InputMethod);
    }
    if m.meta {
        return Some(Reserved::Desktop);
    }
    None
}

/// What binding `keys` to `action` in `mode` collides with. `old` is the binding being
/// re-recorded, which goes away. An empty list means the binding is free.
pub fn conflicts(
    km: &Keymap,
    mode: Mode,
    keys: KeySeq,
    action: Action,
    old: Option<KeySeq>,
) -> Vec<Conflict> {
    let mut out = Vec::new();
    match km.get(mode, &keys) {
        Some(a) if a == action => return vec![Conflict::Already],
        Some(other) => out.push(Conflict::SameKey { other }),
        None => {}
    }
    for (s, a) in km.bindings(mode) {
        if Some(*s) == old && *a == action {
            continue;
        }
        if (s.is_prefix_of(&keys) || keys.is_prefix_of(s)) && *a != action {
            out.push(Conflict::Prefix {
                keys: *s,
                other: *a,
            });
        }
    }
    for m in Mode::ALL {
        if m == mode {
            continue;
        }
        let related = mode.chain().contains(&m) || m.chain().contains(&mode);
        if !related {
            continue;
        }
        if let Some(other) = km.get(m, &keys)
            && other != action
        {
            out.push(Conflict::Shadow { mode: m, other });
        }
    }
    if let Some(r) = keys.keys().iter().find_map(|c| reserved(c, action)) {
        out.push(Conflict::Reserved(r));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Displace, KeymapEdit};

    fn seq(s: &str) -> KeySeq {
        KeySeq::parse(s).unwrap()
    }

    #[test]
    fn conflicts_in_the_mode() {
        let km = Keymap::viewer_default();
        let v = Mode::Viewer;
        assert_eq!(
            conflicts(&km, v, seq("Right"), Action::NextImage, None),
            vec![Conflict::Already]
        );
        assert_eq!(
            conflicts(&km, v, seq("Right"), Action::LastImage, None),
            vec![
                Conflict::SameKey {
                    other: Action::NextImage
                },
                // Movies take the arrows for seeking.
                Conflict::Shadow {
                    mode: Mode::Video,
                    other: Action::SeekForward
                }
            ]
        );
        assert_eq!(
            conflicts(&km, v, seq("F9"), Action::LastImage, None),
            vec![]
        );
        // `g g` is begun by the gallery's `g`.
        assert_eq!(
            conflicts(&km, v, seq("g g"), Action::FirstImage, None),
            vec![Conflict::Prefix {
                keys: seq("g"),
                other: Action::ToggleGallery
            }]
        );
        // Re-recording the gallery's own `g` as `g g` clashes with nothing.
        assert_eq!(
            conflicts(&km, v, seq("g g"), Action::ToggleGallery, Some(seq("g"))),
            vec![]
        );
        let mut km2 = km.clone();
        km2.apply_edit(&KeymapEdit::Bind {
            mode: v,
            keys: seq("g g"),
            action: Action::FirstImage,
            old: None,
            displace: Displace::Keep,
        });
        assert_eq!(
            conflicts(&km2, v, seq("g"), Action::LastImage, None),
            vec![
                Conflict::SameKey {
                    other: Action::ToggleGallery
                },
                Conflict::Prefix {
                    keys: seq("g g"),
                    other: Action::FirstImage
                }
            ]
        );
    }

    #[test]
    fn shadowing_between_modes() {
        let km = Keymap::viewer_default();
        // Binding `f` in cull mode hides fit-to-window there.
        assert_eq!(
            conflicts(&km, Mode::Cull, seq("f"), Action::CullPick, None),
            vec![Conflict::Shadow {
                mode: Mode::Viewer,
                other: Action::ZoomFit
            }]
        );
        // Binding `x` in the viewer takes it from the crosshair, and is hidden in cull mode
        // by reject, in video mode by "subtitles later" and in Develop by its own crosshair
        // (the one that places retouch spots).
        assert_eq!(
            conflicts(&km, Mode::Viewer, seq("x"), Action::NextImage, None),
            vec![
                Conflict::SameKey {
                    other: Action::ToggleCrosshair
                },
                Conflict::Shadow {
                    mode: Mode::Cull,
                    other: Action::CullReject
                },
                Conflict::Shadow {
                    mode: Mode::Video,
                    other: Action::SubtitleLater
                },
                Conflict::Shadow {
                    mode: Mode::Develop,
                    other: Action::DevelopSpotCrosshair
                }
            ]
        );
    }

    #[test]
    fn movies_take_f_from_the_viewer() {
        let km = Keymap::viewer_default();
        // Rebinding `f` in the viewer is hidden in movies by full-screen play.
        assert_eq!(
            conflicts(&km, Mode::Viewer, seq("f"), Action::NextImage, None),
            vec![
                Conflict::SameKey {
                    other: Action::ZoomFit
                },
                Conflict::Shadow {
                    mode: Mode::Video,
                    other: Action::VideoFullscreen
                }
            ]
        );
        // Binding full-screen play to `f` again is no change; to F10, free.
        assert_eq!(
            conflicts(&km, Mode::Video, seq("f"), Action::VideoFullscreen, None),
            vec![Conflict::Already]
        );
        assert_eq!(
            conflicts(&km, Mode::Video, seq("F10"), Action::VideoFullscreen, None),
            vec![]
        );
    }

    #[test]
    fn reserved_shortcuts() {
        let r = |s: &str| reserved(&KeyCombo::parse(s).unwrap(), Action::NextImage);
        if cfg!(target_os = "macos") {
            assert_eq!(r("Ctrl+Tab"), Some(Reserved::SwitchWindows));
            assert_eq!(r("Ctrl+q"), Some(Reserved::MacOs));
            assert_eq!(
                reserved(&KeyCombo::parse("Ctrl+q").unwrap(), Action::Quit),
                None
            );
        } else {
            assert_eq!(r("Alt+Tab"), Some(Reserved::SwitchWindows));
            assert_eq!(r("Alt+Shift+Tab"), Some(Reserved::SwitchWindows));
            assert_eq!(r("Alt+F4"), Some(Reserved::CloseWindow));
            assert_eq!(r("Ctrl+Alt+Delete"), Some(Reserved::Secure));
            assert_eq!(r("Ctrl+Alt+F2"), Some(Reserved::Workspaces));
            assert_eq!(r("Ctrl+Alt+Left"), Some(Reserved::Workspaces));
            assert_eq!(r("Ctrl+Space"), Some(Reserved::InputMethod));
            assert_eq!(r("Super+Space"), Some(Reserved::InputMethod));
            assert_eq!(r("Super+e"), Some(Reserved::Desktop));
            assert_eq!(r("Ctrl+Tab"), None);
            assert_eq!(r("Ctrl+Shift+Space"), None);
        }
        assert_eq!(r("Tab"), None);
        assert_eq!(r("Escape"), None);
        // Every default binding is usable.
        let km = Keymap::viewer_default();
        for m in Mode::ALL {
            for (s, a) in km.bindings(m) {
                for c in s.keys() {
                    assert_eq!(reserved(c, *a), None, "{s}");
                }
            }
        }
        let km = Keymap::viewer_default();
        assert_eq!(
            conflicts(&km, Mode::Viewer, seq("Alt+F4"), Action::Quit, None),
            if cfg!(target_os = "macos") {
                vec![]
            } else {
                vec![Conflict::Reserved(Reserved::CloseWindow)]
            }
        );
    }
}
