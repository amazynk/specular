//! Keymap presets (DESIGN §11). Each is a `keymap.toml` fragment applied over the default
//! bindings, so every action keeps a key unless a preset says otherwise. The mappings follow
//! the applications' public documentation; where Specular has no matching action, or the
//! documentation could not be checked, the key stays as in the default keymap.

/// A starting point for the key bindings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Preset {
    #[default]
    Default,
    /// vim motions (`h j k l`, `g g`, `G`) with the rest from nsxiv, the vim-style viewer.
    Vim,
    /// Adobe Lightroom Classic's Library keys for culling.
    Culling,
    /// Camera Bits Photo Mechanic's tag, rating and colour class keys.
    PhotoMechanic,
}

impl Preset {
    pub const ALL: [Preset; 4] = [
        Preset::Default,
        Preset::Vim,
        Preset::Culling,
        Preset::PhotoMechanic,
    ];

    /// The name in `keymap.toml` (`preset = "vim"`).
    pub fn id(self) -> &'static str {
        match self {
            Preset::Default => "default",
            Preset::Vim => "vim",
            Preset::Culling => "lightroom-culling",
            Preset::PhotoMechanic => "photo-mechanic",
        }
    }

    pub fn from_id(s: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.id() == s.trim())
    }

    /// Fluent message ID of the preset's name.
    pub fn message_id(self) -> &'static str {
        match self {
            Preset::Default => "keys-preset-default",
            Preset::Vim => "keys-preset-vim",
            Preset::Culling => "keys-preset-culling",
            Preset::PhotoMechanic => "keys-preset-photo-mechanic",
        }
    }

    /// The preset's changes to the default keymap (`None` for the default itself).
    pub(crate) fn toml(self) -> Option<std::borrow::Cow<'static, str>> {
        match self {
            Preset::Default => None,
            // vim's Ctrl keys are the Control key, which is `Super` on macOS (`Ctrl` is
            // Command there, and Command+H hides the app).
            Preset::Vim if cfg!(target_os = "macos") => {
                Some(VIM.replace("\"Ctrl+", "\"Super+").into())
            }
            Preset::Vim => Some(VIM.into()),
            Preset::Culling => Some(CULLING.into()),
            Preset::PhotoMechanic => Some(PHOTO_MECHANIC.into()),
        }
    }
}

/// vim: motions from vim (`:help left-right-motions`, `:help up-down-motions`: `h j k l`,
/// `gg`, `G`), the rest from the nsxiv(1) manual (https://nsxiv.codeberg.page/man/): `n`/`p`
/// next and previous, `Ctrl+h/j/k` scroll a screen (to the right `Ctrl+Shift+l`: `Ctrl+l`
/// opens an address in every preset), `=` 100 %, `W` fit, `<`/`>` rotate,
/// `|`/`_` flip, `f` full screen, `r` reload, Return the thumbnails, `Ctrl+a` play.
/// On macOS these Ctrl keys are bound to the Control key (`Super`), as in vim there.
/// `:` opens the command palette, as vim's command line. Keys the motions take move their
/// default actions: flip → `|`, clipping → `C`, navigator → `N`, histogram → `H`, the
/// gallery → Enter (nsxiv's thumbnail mode), fit → `W`, play/pause → `Ctrl+a`, false colour
/// → `Alt+c`, HiLo → `Alt+h`, key image → `Alt+Shift+k`. Open: `/`
/// (library search, DESIGN §11) once the library exists; counts (`5 n`).
const VIM: &str = r#"
[viewer]
"h" = "pan-left"
"j" = "pan-down"
"k" = "pan-up"
"l" = "pan-right"
"Ctrl+h" = "pan-page-left"
"Ctrl+j" = "pan-page-down"
"Ctrl+k" = "pan-page-up"
"Ctrl+Shift+l" = "pan-page-right"
"g" = "none"
"g g" = "first"
"G" = "last"
"n" = "next"
"p" = "prev"
"=" = "zoom-to:100"
"W" = "zoom-fit"
"f" = "fullscreen"
"<" = "rotate-left"
">" = "rotate-right"
"|" = "flip-horizontal"
"_" = "flip-vertical"
"r" = "reload"
"Enter" = "gallery"
"Ctrl+a" = "play-pause"
":" = "palette"
"C" = "clipping"
"N" = "navigator"
"H" = "histogram"
"Alt+c" = "false-colour"
"Alt+h" = "hilo"
"Alt+Shift+k" = "toggle-key-image"
"#;

/// Lightroom Classic Library module (Adobe, "Keyboard shortcuts",
/// https://helpx.adobe.com/lightroom-classic/desktop/introduction-to-lightroom-classic/keyboard-shortcuts.html):
/// `P`/`X`/`U` flag, `0`–`5` rate, `6`–`9` red/yellow/green/blue labels (Specular's cull
/// defaults already match), `G` grid (the gallery), `E` loupe (the image), `Z` 1:1 zoom,
/// `Ctrl+]`/`Ctrl+[` rotate clockwise/anticlockwise, `F` full screen, `I` the info overlay,
/// `Ctrl+Backspace` delete rejected → move to trash here. The zoom rectangle moves from `z`
/// to `Shift+z`, fit keeps `Ctrl+0`. Open: `Shift+P/X/U/0–9` (mark and go to the next image),
/// Caps Lock auto-advance (Specular's cull mode has an auto-advance setting instead),
/// `C`/`N` compare and survey views.
const CULLING: &str = r#"
[viewer]
"e" = "gallery"
"z" = "zoom-to:100"
"Z" = "zoom-rectangle"
"Ctrl+]" = "rotate-right"
"Ctrl+[" = "rotate-left"
"f" = "fullscreen"
"Ctrl+Backspace" = "trash"
"#;

/// Photo Mechanic (Camera Bits documentation, "Color Class Ratings",
/// https://docs.camerabits.com/support/solutions/articles/48001142942, and "Star Ratings",
/// https://docs.camerabits.com/support/solutions/articles/48001143067): `T` toggles the tag
/// (a pick here), the number keys `1`–`8` set a colour class in the preview (Specular has
/// four labels: `1`–`4`), `Alt+1`–`Alt+5` rate and `Alt+0` clears the rating (Windows; the
/// macOS Control key is `Super` here). Open: `0` (clear the colour class), classes 5–8, `+`/`-`
/// (tag on/off) and the single-key rating preference. Specular's own cull keys that these
/// take move: copy to folder 1–3 from `Alt+1`–`3` to `Alt+7`–`9` (moving keeps
/// `Alt+Shift+1`–`3`), following new files from `t` to `Shift+t`, the subframe table from
/// `Shift+t` to `Shift+s`.
const PHOTO_MECHANIC: &str = r#"
replace = ["cull"]

[cull]
"t" = "cull-pick"
"x" = "cull-reject"
"u" = "cull-unflag"
"1" = "cull-label:red"
"2" = "cull-label:yellow"
"3" = "cull-label:green"
"4" = "cull-label:blue"
"Alt+0" = "cull-rate:0"
"Alt+1" = "cull-rate:1"
"Alt+2" = "cull-rate:2"
"Alt+3" = "cull-rate:3"
"Alt+4" = "cull-rate:4"
"Alt+5" = "cull-rate:5"
"Alt+7" = "cull-copy-to:1"
"Alt+8" = "cull-copy-to:2"
"Alt+9" = "cull-copy-to:3"
"Alt+Shift+1" = "cull-move-to:1"
"Alt+Shift+2" = "cull-move-to:2"
"Alt+Shift+3" = "cull-move-to:3"
"Ctrl+Alt+s" = "develop-sync"
"Ctrl+Alt+Shift+s" = "develop-sync-some"
"s" = "cull-burst"
"b" = "cull-keep-best"
"T" = "cull-follow"
"S" = "cull-subframes"
"o" = "cull-sort-next"
"O" = "cull-sort-reverse"
"y" = "cull-metric-next"
"Alt+-" = "cull-sigma-down"
"Alt+=" = "cull-sigma-up"
"R" = "cull-auto-reject"
"X" = "cull-move-rejects"
"e" = "cull-same-stretch"
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, KeySeq, Keymap, Mode};

    #[test]
    fn presets_load_cleanly() {
        for p in Preset::ALL {
            assert_eq!(Preset::from_id(p.id()), Some(p));
            if let Some(text) = p.toml() {
                let mut km = Keymap::viewer_default();
                let errs = km.apply_toml(&text);
                assert_eq!(errs, vec![], "{}", p.id());
            }
            let km = Keymap::from_preset(p);
            assert!(!km.changed());
            for mode in Mode::ALL {
                let b = km.bindings(mode);
                // No prefix clashes: a preset's keys never wait for the timeout.
                for (s, _) in b {
                    for (t, _) in b {
                        assert!(!s.is_prefix_of(t), "{}: {} begins {}", p.id(), s, t);
                    }
                }
            }
            // Every action of the default keymap that has a key keeps one in its mode, so a
            // preset never loses a function (cull ratings move to Alt in Photo Mechanic).
            let def = Keymap::viewer_default();
            for a in Action::BINDABLE {
                if !def.keys_in(a.mode(), *a).is_empty() {
                    assert!(
                        !km.keys_in(a.mode(), *a).is_empty(),
                        "{}: {} has no key",
                        p.id(),
                        a.id()
                    );
                }
            }
        }
        let vim = Keymap::from_preset(Preset::Vim);
        let k = |s: &str| KeySeq::parse(s).unwrap();
        assert_eq!(vim.get(Mode::Viewer, &k("g g")), Some(Action::FirstImage));
        assert_eq!(vim.get(Mode::Viewer, &k("G")), Some(Action::LastImage));
        // vim's Ctrl+k is the Control key, `Super` on macOS (where `Ctrl`, Command, keeps
        // its own meaning).
        let page_up = if cfg!(target_os = "macos") {
            "Super+k"
        } else {
            "Ctrl+k"
        };
        assert_eq!(vim.get(Mode::Viewer, &k(page_up)), Some(Action::PanPageUp));
        let lr = Keymap::from_preset(Preset::Culling);
        assert_eq!(lr.get(Mode::Viewer, &k("z")), Some(Action::ZoomTo(100.0)));
        let pm = Keymap::from_preset(Preset::PhotoMechanic);
        assert_eq!(pm.get(Mode::Cull, &k("t")), Some(Action::CullPick));
        assert_eq!(pm.get(Mode::Cull, &k("p")), None);
        assert_eq!(pm.get(Mode::Cull, &k("T")), Some(Action::CullFollow));
        assert_eq!(pm.get(Mode::Cull, &k("Alt+7")), Some(Action::CullCopyTo(1)));
        // Cull mode never hides Measure, markup or the key image: they keep working there.
        for p in Preset::ALL {
            let km = Keymap::from_preset(p);
            for a in [Action::Measure, Action::Markup, Action::ToggleKeyImage] {
                for key in km.keys_in(Mode::Viewer, a) {
                    assert_eq!(
                        km.get(Mode::Cull, &key),
                        None,
                        "{}: cull mode takes {} from {}",
                        p.id(),
                        key,
                        a.id()
                    );
                }
            }
        }
        // Medical mode is the default's in every preset.
        for p in Preset::ALL {
            let km = Keymap::from_preset(p);
            assert_eq!(
                km.bindings(Mode::Medical),
                Keymap::viewer_default().bindings(Mode::Medical)
            );
            assert_eq!(
                km.get(Mode::Medical, &k("w")),
                Some(Action::WindowLevelTool),
                "{}",
                p.id()
            );
        }
    }
}
