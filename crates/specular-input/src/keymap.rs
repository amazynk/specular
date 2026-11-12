use crate::{Action, Mode, Os, Preset, Profile};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

/// Keys that have no character.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NamedKey {
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Space,
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F(u8),
    /// A gamepad button (`Pad-A`), see [`crate::device`].
    Pad(crate::PadButton),
    /// A SpaceMouse button, 1-based (`Puck-1`), see [`crate::device`].
    Puck(u8),
}

/// A key, by its logical meaning on the current layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Named(NamedKey),
    Char(char),
}

/// Modifier state. On macOS `ctrl` is Command and `meta` is Control (see [`Mods::held`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Super (Windows, Linux) or Control (macOS).
    pub meta: bool,
}

impl Mods {
    pub const NONE: Mods = Mods {
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
    };
    pub const CTRL: Mods = Mods {
        ctrl: true,
        shift: false,
        alt: false,
        meta: false,
    };
    pub const SHIFT: Mods = Mods {
        ctrl: false,
        shift: true,
        alt: false,
        meta: false,
    };
    pub const CTRL_SHIFT: Mods = Mods {
        ctrl: true,
        shift: true,
        alt: false,
        meta: false,
    };
    pub fn any(self) -> bool {
        self.ctrl || self.shift || self.alt || self.meta
    }

    /// The modifiers of the keys held on `os`: the Control, Shift, Alt (Option) and logo
    /// (Super, Windows, Command) keys. On macOS Command is `ctrl` and Control is `meta`, as
    /// the keymap writes and shows them (`Ctrl+f` is ⌘F), so one keymap gives each platform
    /// its usual keys.
    pub fn held(os: Os, control: bool, shift: bool, alt: bool, logo: bool) -> Mods {
        let (ctrl, meta) = if os == Os::MacOs {
            (logo, control)
        } else {
            (control, logo)
        };
        Mods {
            ctrl,
            shift,
            alt,
            meta,
        }
    }
}

/// A key with modifiers, normalised so that equal presses compare equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub key: Key,
    pub mods: Mods,
}

impl KeyCombo {
    /// Build from an event. For character keys, Shift is folded into the character when no
    /// other command modifier is held (`?` rather than `Shift+/`), except for digits
    /// (`Shift+1`), and letters with Ctrl/Alt are lower-cased with Shift kept as a modifier
    /// (`Ctrl+Shift+C`).
    pub fn new(key: Key, mods: Mods) -> KeyCombo {
        match key {
            Key::Char(c) => {
                if mods.ctrl || mods.alt || mods.meta {
                    let lc = c.to_lowercase().next().unwrap_or(c);
                    let shift = mods.shift || (c != lc);
                    KeyCombo {
                        key: Key::Char(lc),
                        mods: Mods { shift, ..mods },
                    }
                } else {
                    // Digits keep Shift: on layouts that need Shift to type a digit (AZERTY),
                    // `Shift+1` is still the 1 key with Shift, as on every other layout.
                    KeyCombo {
                        key: Key::Char(c),
                        mods: Mods {
                            shift: mods.shift && c.is_ascii_digit(),
                            ..mods
                        },
                    }
                }
            }
            Key::Named(NamedKey::Space) => KeyCombo { key, mods },
            _ => KeyCombo { key, mods },
        }
    }

    /// Parse `Ctrl+Shift+C`, `Right`, `?`, `Plus`, `F11`.
    pub fn parse(s: &str) -> Option<KeyCombo> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        // A lone "+" is the plus key.
        if s == "+" {
            return Some(KeyCombo::new(Key::Char('+'), Mods::NONE));
        }
        let (parts, last) = match s.strip_suffix("++") {
            Some(head) => (head.split('+').collect::<Vec<_>>(), "+"),
            None => {
                let mut v: Vec<&str> = s.split('+').collect();
                let last = v.pop()?;
                (v, last)
            }
        };
        let mut mods = Mods::NONE;
        for p in parts {
            match p.trim().to_ascii_lowercase().as_str() {
                "ctrl" | "control" => mods.ctrl = true,
                "shift" => mods.shift = true,
                "alt" | "option" => mods.alt = true,
                "meta" | "super" | "cmd" | "command" | "win" => mods.meta = true,
                "" => {}
                _ => return None,
            }
        }
        let key = parse_key(last)?;
        let mut combo = KeyCombo::new(key, mods);
        // "Shift+i" means capital I. "Shift+1" stays Shift plus the digit key, whatever the
        // layout prints on it (see `Keymap::resolve`).
        if let Key::Char(c) = key
            && mods.shift
            && !(mods.ctrl || mods.alt || mods.meta)
            && !c.is_ascii_digit()
        {
            combo = KeyCombo {
                key: Key::Char(c.to_uppercase().next().unwrap_or(c)),
                mods: Mods::NONE,
            };
        }
        Some(combo)
    }
}

fn parse_key(s: &str) -> Option<Key> {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    let named = match lower.as_str() {
        "left" | "arrowleft" => NamedKey::ArrowLeft,
        "right" | "arrowright" => NamedKey::ArrowRight,
        "up" | "arrowup" => NamedKey::ArrowUp,
        "down" | "arrowdown" => NamedKey::ArrowDown,
        "space" => NamedKey::Space,
        "enter" | "return" => NamedKey::Enter,
        "esc" | "escape" => NamedKey::Escape,
        "backspace" => NamedKey::Backspace,
        "del" | "delete" => NamedKey::Delete,
        "tab" => NamedKey::Tab,
        "home" => NamedKey::Home,
        "end" => NamedKey::End,
        "pageup" | "pgup" => NamedKey::PageUp,
        "pagedown" | "pgdn" => NamedKey::PageDown,
        "insert" | "ins" => NamedKey::Insert,
        "plus" => return Some(Key::Char('+')),
        "minus" => return Some(Key::Char('-')),
        "comma" => return Some(Key::Char(',')),
        "period" => return Some(Key::Char('.')),
        _ => {
            if let Some(b) = lower.strip_prefix("pad-") {
                return crate::PadButton::from_id(b).map(|b| Key::Named(NamedKey::Pad(b)));
            }
            if let Some(n) = lower.strip_prefix("puck-") {
                let n = n.parse::<u8>().ok()?;
                return (1..=crate::device::PUCK_BUTTONS)
                    .contains(&n)
                    .then_some(Key::Named(NamedKey::Puck(n)));
            }
            if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
                && (1..=24).contains(&n)
            {
                return Some(Key::Named(NamedKey::F(n)));
            }
            let mut cs = t.chars();
            let c = cs.next()?;
            if cs.next().is_some() {
                return None;
            }
            return Some(Key::Char(c));
        }
    };
    Some(Key::Named(named))
}

impl fmt::Display for KeyCombo {
    /// Display form for tooltips and the cheat sheet, e.g. `Ctrl+Shift+C`, `→`, `?`; on macOS
    /// the symbols in Apple's order, Control Option Shift Command (`⇧⌘C`). Letters show as on
    /// the keycap, in upper case, so a bare upper-case letter (Shift folded into it, see
    /// [`KeyCombo::new`]) shows its Shift: `G` is g, `⇧G` / `Shift+G` is Shift+g.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.mods;
        let command = m.ctrl || m.alt || m.meta;
        let folded_shift = matches!(self.key, Key::Char(c) if !command && c.is_uppercase());
        let shift = m.shift || folded_shift;
        // Ctrl is Command on macOS and Super (meta) is Control.
        let mods: [(bool, &str); 4] = if cfg!(target_os = "macos") {
            [(m.meta, "⌃"), (m.alt, "⌥"), (shift, "⇧"), (m.ctrl, "⌘")]
        } else {
            [
                (m.ctrl, "Ctrl+"),
                (m.alt, "Alt+"),
                (m.meta, "Super+"),
                (shift, "Shift+"),
            ]
        };
        for (on, label) in mods {
            if on {
                f.write_str(label)?;
            }
        }
        // macOS names its keys with the symbols on Apple's keycaps and in its menus.
        if cfg!(target_os = "macos")
            && let Key::Named(n) = self.key
        {
            let glyph = match n {
                NamedKey::Enter => Some("↩"),
                NamedKey::Escape => Some("⎋"),
                NamedKey::Backspace => Some("⌫"),
                NamedKey::Delete => Some("⌦"),
                NamedKey::Tab => Some("⇥"),
                NamedKey::PageUp => Some("⇞"),
                NamedKey::PageDown => Some("⇟"),
                NamedKey::Home => Some("↖"),
                NamedKey::End => Some("↘"),
                _ => None,
            };
            if let Some(g) = glyph {
                return f.write_str(g);
            }
        }
        match self.key {
            Key::Char(' ') => f.write_str("Space"),
            Key::Char(c) => write!(f, "{}", c.to_uppercase()),
            Key::Named(n) => match n {
                NamedKey::ArrowLeft => f.write_str("←"),
                NamedKey::ArrowRight => f.write_str("→"),
                NamedKey::ArrowUp => f.write_str("↑"),
                NamedKey::ArrowDown => f.write_str("↓"),
                NamedKey::Space => f.write_str("Space"),
                NamedKey::Enter => f.write_str("Enter"),
                NamedKey::Escape => f.write_str("Esc"),
                NamedKey::Backspace => f.write_str("Backspace"),
                NamedKey::Delete => f.write_str("Del"),
                NamedKey::Tab => f.write_str("Tab"),
                NamedKey::Home => f.write_str("Home"),
                NamedKey::End => f.write_str("End"),
                NamedKey::PageUp => f.write_str("PgUp"),
                NamedKey::PageDown => f.write_str("PgDn"),
                NamedKey::Insert => f.write_str("Ins"),
                NamedKey::F(n) => write!(f, "F{n}"),
                NamedKey::Pad(b) => write!(f, "Pad {}", b.label()),
                NamedKey::Puck(n) => write!(f, "SpaceMouse {n}"),
            },
        }
    }
}

impl KeyCombo {
    /// The canonical `keymap.toml` spelling (`Ctrl+Shift+c`, `Right`, `Shift+1`, `Plus`, `F5`):
    /// [`KeyCombo::parse`] reads it back to the same combination on every platform.
    pub fn spec(&self) -> String {
        let m = self.mods;
        let mut s = String::new();
        if m.ctrl {
            s.push_str("Ctrl+");
        }
        if m.alt {
            s.push_str("Alt+");
        }
        if m.meta {
            s.push_str("Super+");
        }
        if m.shift {
            s.push_str("Shift+");
        }
        match self.key {
            Key::Char('+') => s.push_str("Plus"),
            Key::Char(' ') => s.push_str("Space"),
            Key::Char(c) => s.push(c),
            Key::Named(n) => match n {
                NamedKey::ArrowLeft => s.push_str("Left"),
                NamedKey::ArrowRight => s.push_str("Right"),
                NamedKey::ArrowUp => s.push_str("Up"),
                NamedKey::ArrowDown => s.push_str("Down"),
                NamedKey::Space => s.push_str("Space"),
                NamedKey::Enter => s.push_str("Enter"),
                NamedKey::Escape => s.push_str("Escape"),
                NamedKey::Backspace => s.push_str("Backspace"),
                NamedKey::Delete => s.push_str("Delete"),
                NamedKey::Tab => s.push_str("Tab"),
                NamedKey::Home => s.push_str("Home"),
                NamedKey::End => s.push_str("End"),
                NamedKey::PageUp => s.push_str("PageUp"),
                NamedKey::PageDown => s.push_str("PageDown"),
                NamedKey::Insert => s.push_str("Insert"),
                NamedKey::F(n) => {
                    s.push('F');
                    s.push_str(&n.to_string());
                }
                NamedKey::Pad(b) => {
                    s.push_str("Pad-");
                    s.push_str(b.id());
                }
                NamedKey::Puck(n) => {
                    s.push_str("Puck-");
                    s.push_str(&n.to_string());
                }
            },
        }
        s
    }

    /// A navigation device's button rather than a keyboard key.
    pub fn is_device(&self) -> bool {
        matches!(self.key, Key::Named(NamedKey::Pad(_) | NamedKey::Puck(_)))
    }

    /// Plain Escape: the recorder's cancel key.
    pub fn is_escape(&self) -> bool {
        self.key == Key::Named(NamedKey::Escape) && !self.mods.any()
    }
}

/// A key sequence: one combination (`Ctrl+K`) or a chord of up to [`KeySeq::MAX`] presses
/// (`g g`, `r 1`), pressed one after the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeySeq {
    keys: [KeyCombo; KeySeq::MAX],
    len: u8,
}

/// Fills the unused slots so that equal sequences compare and hash equal.
const PAD: KeyCombo = KeyCombo {
    key: Key::Named(NamedKey::F(0)),
    mods: Mods::NONE,
};

impl From<KeyCombo> for KeySeq {
    fn from(c: KeyCombo) -> KeySeq {
        KeySeq::one(c)
    }
}

impl KeySeq {
    /// The longest sequence.
    pub const MAX: usize = 3;

    pub fn one(c: KeyCombo) -> KeySeq {
        KeySeq {
            keys: [c, PAD, PAD],
            len: 1,
        }
    }

    /// A sequence of 1 to [`KeySeq::MAX`] keys.
    pub fn from_keys(keys: &[KeyCombo]) -> Option<KeySeq> {
        if keys.is_empty() || keys.len() > KeySeq::MAX {
            return None;
        }
        let mut s = KeySeq::one(keys[0]);
        for k in &keys[1..] {
            s = s.push(*k)?;
        }
        Some(s)
    }

    pub fn keys(&self) -> &[KeyCombo] {
        &self.keys[..usize::from(self.len)]
    }

    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    /// Never true: a sequence has at least one key.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn first(&self) -> KeyCombo {
        self.keys[0]
    }

    /// This sequence followed by one more key, if that is not too long.
    pub fn push(self, c: KeyCombo) -> Option<KeySeq> {
        let n = self.len();
        if n >= KeySeq::MAX {
            return None;
        }
        let mut s = self;
        s.keys[n] = c;
        s.len += 1;
        Some(s)
    }

    /// Whether `self` is a strict beginning of `longer` (`g` of `g g`).
    pub fn is_prefix_of(&self, longer: &KeySeq) -> bool {
        self.len < longer.len && longer.keys()[..self.len()] == *self.keys()
    }

    /// Parse `Ctrl+K` or a sequence separated by spaces (`g g`, `r 1`).
    pub fn parse(s: &str) -> Option<KeySeq> {
        let mut keys = Vec::new();
        for part in s.split_whitespace() {
            keys.push(KeyCombo::parse(part)?);
        }
        KeySeq::from_keys(&keys)
    }

    /// The canonical `keymap.toml` spelling.
    pub fn spec(&self) -> String {
        let v: Vec<String> = self.keys().iter().map(KeyCombo::spec).collect();
        v.join(" ")
    }
}

impl fmt::Display for KeySeq {
    /// Display form: the keys' display forms separated by spaces (`G G`, `Ctrl+K`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, k) in self.keys().iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{k}")?;
        }
        Ok(())
    }
}

/// A problem in `keymap.toml`. Only [`KeymapError::Syntax`] stops the file from loading; for
/// the others the rest of the file still applies.
#[derive(Clone, Debug, PartialEq)]
pub enum KeymapError {
    Syntax(String),
    UnknownKey(String),
    UnknownAction {
        key: String,
        action: String,
    },
    /// A table that names no mode.
    UnknownMode(String),
    /// Two spellings of the same keys in one mode (`"Ctrl+K"` and `"ctrl+k"`): the later one
    /// wins.
    Duplicate {
        mode: Mode,
        keys: KeySeq,
        action: Action,
        other: Action,
    },
    /// `keys` begin the longer binding `longer` in the same mode: `keys` runs only after the
    /// sequence timeout.
    Prefix {
        mode: Mode,
        keys: KeySeq,
        longer: KeySeq,
    },
    /// An unknown preset name.
    UnknownPreset(String),
    /// `keys` may never reach Specular: the operating system or the input method takes them
    /// ([`crate::reserved`]). The binding still applies.
    Reserved {
        mode: Mode,
        keys: KeySeq,
        reason: crate::Reserved,
    },
}

impl KeymapError {
    /// Whether the file can be kept as it is: the binding applies, though it may not work as
    /// expected (a prefix clash, a reserved key). Settings → Keys lets the user bind those
    /// anyway; the other problems drop an entry or make it ambiguous.
    pub fn is_warning(&self) -> bool {
        matches!(
            self,
            KeymapError::Prefix { .. } | KeymapError::Reserved { .. }
        )
    }
}

impl fmt::Display for KeymapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeymapError::Syntax(s) => write!(f, "keymap.toml: {s}"),
            KeymapError::UnknownKey(k) => write!(f, "keymap.toml: unknown key {k:?}"),
            KeymapError::UnknownAction { key, action } => {
                write!(f, "keymap.toml: unknown action {action:?} for {key:?}")
            }
            KeymapError::UnknownMode(m) => write!(f, "keymap.toml: unknown mode [{m}]"),
            KeymapError::Duplicate {
                mode,
                keys,
                action,
                other,
            } => write!(
                f,
                "keymap.toml: [{}] {:?} is given twice ({} and {}); the second wins",
                mode.id(),
                keys.spec(),
                other.id(),
                action.id()
            ),
            KeymapError::Prefix { mode, keys, longer } => write!(
                f,
                "keymap.toml: [{}] {:?} begins {:?}, so it waits for the sequence timeout",
                mode.id(),
                keys.spec(),
                longer.spec()
            ),
            KeymapError::UnknownPreset(p) => write!(f, "keymap.toml: unknown preset {p:?}"),
            KeymapError::Reserved { mode, keys, reason } => write!(
                f,
                "keymap.toml: [{}] {:?} may never reach Specular ({reason:?})",
                mode.id(),
                keys.spec()
            ),
        }
    }
}

/// A [`KeymapError`] with where it is in the file ([`Keymap::check`]).
#[derive(Clone, Debug, PartialEq)]
pub struct KeymapProblem {
    pub error: KeymapError,
    /// Byte range in the text: the key, the value or the table at fault (`None` when the
    /// problem has no single place, e.g. a file that could not be read).
    pub span: Option<Range<usize>>,
}

impl KeymapProblem {
    fn new(error: KeymapError, span: Option<Range<usize>>) -> KeymapProblem {
        KeymapProblem { error, span }
    }

    /// The 1-based line and column (in characters) where the problem starts in `text`.
    pub fn line_col(&self, text: &str) -> Option<(usize, usize)> {
        let start = self.span.as_ref()?.start.min(text.len());
        let before = text.get(..start)?;
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        Some((line, col))
    }
}

/// One mode's bindings.
#[derive(Clone, Debug, Default)]
struct Layer {
    /// In definition order, for display (the first key of an action is its primary key).
    order: Vec<(KeySeq, Action)>,
    map: HashMap<KeySeq, Action>,
    /// Strict beginnings of bound sequences.
    prefixes: HashSet<KeySeq>,
}

impl Layer {
    fn reindex(&mut self) {
        self.map = self.order.iter().copied().collect();
        self.prefixes.clear();
        for (s, _) in &self.order {
            for n in 1..s.len() {
                if let Some(p) = KeySeq::from_keys(&s.keys()[..n]) {
                    self.prefixes.insert(p);
                }
            }
        }
    }

    fn get(&self, s: &KeySeq) -> Option<Action> {
        self.map.get(s).copied()
    }

    /// Bind at the end, taking the keys from any other action.
    fn bind(&mut self, s: KeySeq, a: Action) {
        // Incremental: a file of thousands of bindings is checked on every keystroke in the
        // config editor, so no full reindex per binding.
        if self.map.insert(s, a).is_some() {
            self.order.retain(|(k, _)| *k != s);
        }
        self.order.push((s, a));
        for n in 1..s.len() {
            if let Some(p) = KeySeq::from_keys(&s.keys()[..n]) {
                self.prefixes.insert(p);
            }
        }
    }

    fn unbind(&mut self, s: KeySeq) -> bool {
        if self.map.remove(&s).is_none() {
            return false;
        }
        self.order.retain(|(k, _)| *k != s);
        if s.len() > 1 {
            self.reindex();
        }
        true
    }

    fn keys_for(&self, a: Action) -> Vec<KeySeq> {
        self.order
            .iter()
            .filter(|(_, x)| *x == a)
            .map(|(k, _)| *k)
            .collect()
    }

    fn same(&self, other: &Layer) -> bool {
        self.order.len() == other.order.len()
            && self.order.iter().all(|(k, a)| other.get(k) == Some(*a))
    }
}

/// How a new binding treats the bindings it collides with ([`crate::conflicts`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Displace {
    /// Leave sequences that begin, or are begun by, the new keys (the shorter one then waits
    /// for the sequence timeout). The same keys can only run one action, so their old binding
    /// goes regardless.
    Keep,
    /// Remove every binding of the mode that clashes with the new keys.
    Replace,
    /// Give the old keys of the re-recorded binding to the action that had the new keys.
    Swap,
}

/// A change made in Settings → Keys.
#[derive(Clone, Debug, PartialEq)]
pub enum KeymapEdit {
    /// Bind `keys` to `action` in `mode`; with `old`, in place of that binding of the action.
    Bind {
        mode: Mode,
        keys: KeySeq,
        action: Action,
        old: Option<KeySeq>,
        displace: Displace,
    },
    Unbind {
        mode: Mode,
        keys: KeySeq,
    },
    /// Back to the preset's keys for one action; keys another action took since stay there.
    ResetAction {
        mode: Mode,
        action: Action,
    },
    ResetMode(Mode),
    ResetAll,
    /// Start again from a preset.
    Preset(Preset),
}

/// The outcome of one key press against the bindings ([`Keymap::step`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    /// Actions to run now, in order (a sequence that did not continue runs its own binding
    /// before the new key's).
    pub run: Vec<Action>,
    /// The keys so far of a sequence that may continue: wait for the next key, or for the
    /// timeout ([`Keymap::expire`]).
    pub pending: Option<KeySeq>,
}

/// Key bindings: one layer per [`Mode`], from a [`Preset`] with the user's changes.
#[derive(Clone, Debug)]
pub struct Keymap {
    layers: Vec<Layer>,
    preset: Preset,
    /// The preset's layers, for "changed from preset" and the diff `keymap.toml` stores.
    base: Arc<Vec<Layer>>,
    /// What loading `keymap.toml` found wrong, for the editor.
    problems: Vec<KeymapError>,
    /// The modes in effect, innermost first and ending with the viewer
    /// ([`Keymap::set_active`]): what [`Keymap::lookup`], [`Keymap::keys_for`] and
    /// [`Keymap::primary_key`] answer for.
    active: Vec<Mode>,
    /// The capabilities in effect ([`Keymap::set_profile`]): keys of hidden actions do
    /// nothing here, so they fall through to the next binding of the same keys.
    profile: Profile,
    /// Device buttons switched off (`"Pad-A" = "none"`): their default
    /// ([`crate::DEVICE_KEYS`]) does not run either.
    device_off: Vec<KeyCombo>,
}

impl Default for Keymap {
    fn default() -> Self {
        Keymap::viewer_default()
    }
}

/// The default bindings (DESIGN §11, §6.3 and §12.6).
fn default_layers() -> Vec<Layer> {
    use Action::*;
    let viewer: &[(&str, Action)] = &[
        ("Right", NextImage),
        ("Space", NextImage),
        ("PageDown", NextImage),
        ("Left", PrevImage),
        ("Backspace", PrevImage),
        ("PageUp", PrevImage),
        ("Home", FirstImage),
        ("End", LastImage),
        ("Ctrl+g", GoToImage),
        ("Up", PanUp),
        ("Down", PanDown),
        ("Shift+Left", PanPageLeft),
        ("Shift+Right", PanPageRight),
        ("Shift+Up", PanPageUp),
        ("Shift+Down", PanPageDown),
        ("+", ZoomIn),
        ("=", ZoomIn),
        ("Ctrl+=", ZoomIn),
        ("Ctrl+Plus", ZoomIn),
        ("-", ZoomOut),
        ("Ctrl+-", ZoomOut),
        ("f", ZoomFit),
        ("Ctrl+0", ZoomFit),
        // The interface's size, beside the image's zoom.
        ("Ctrl+Alt+=", InterfaceLarger),
        ("Ctrl+Alt+Plus", InterfaceLarger),
        ("Ctrl+Alt+-", InterfaceSmaller),
        ("Ctrl+Alt+0", InterfaceReset),
        ("1", ZoomTo(100.0)),
        ("Ctrl+1", ZoomTo(100.0)),
        ("2", ZoomTo(200.0)),
        ("3", ZoomTo(300.0)),
        ("4", ZoomTo(400.0)),
        ("z", ZoomRectangle),
        ("[", RotateLeft),
        ("]", RotateRight),
        ("h", FlipHorizontal),
        ("v", FlipVertical),
        ("n", ToggleNavigator),
        ("q", Loupe),
        ("g", ToggleGallery),
        // `L` (DESIGN §10: `Tab` / `L`). Tab stays the chrome's way in for now.
        ("Shift+l", ToggleLibrary),
        ("Shift+k", Keywords),
        ("Ctrl+Alt+i", EditMetadata),
        ("Shift+q", ToggleQueue),
        // Animations and movies: loop forever (or as the file says).
        ("Alt+l", ToggleLoopForever),
        ("Alt+g", TogglePixelGrid),
        ("j", ToggleClipping),
        ("Shift+f", ToggleFocusPeaking),
        ("Shift+g", ToggleHistogram),
        ("Shift+i", TogglePixelInspector),
        // Keyboard-only inspection: a crosshair moved by the arrow keys, and the value under
        // it (or the pointer) read out to a screen reader.
        ("x", ToggleCrosshair),
        ("Alt+r", AnnounceCursor),
        ("Shift+1", ChannelRed),
        ("Shift+2", ChannelGreen),
        ("Shift+3", ChannelBlue),
        ("Shift+4", ChannelAlpha),
        ("Shift+5", ChannelLuma),
        ("Shift+c", ToggleFalseColour),
        ("Shift+b", NextColourMap),
        ("Shift+h", ToggleHiLo),
        ("Shift+v", NextColourVision),
        ("i", ToggleInfo),
        // Medical mode keeps `m` for its own length tool.
        ("m", Measure),
        // Markup on a sheet, and comparing two revisions of it.
        ("Alt+m", Markup),
        ("Alt+d", CompareSheets),
        ("F11", ToggleFullscreen),
        ("Shift+F11", ViewerOnOtherMonitor),
        ("Enter", ToggleFullscreen),
        ("s", Slideshow),
        ("Ctrl+Shift+k", ToggleCull),
        ("Escape", Back),
        ("Ctrl+c", CopyImage),
        ("Ctrl+Shift+c", CopyPath),
        ("Ctrl+Shift+t", FindText),
        ("Ctrl+Shift+o", OpenWith),
        ("Ctrl+p", Print),
        // The report (PDF) of the image shown: thermal or inspection.
        ("Alt+Shift+p", ThermalReport),
        ("Ctrl+Shift+e", ExportFigure),
        ("k", ToggleKeyImage),
        ("Ctrl+Shift+m", Import),
        ("Ctrl+v", Paste),
        // Develop edits: copy and paste them, match to a reference (Ctrl+Shift+C copies
        // the path, so Ctrl+Alt).
        ("Ctrl+Alt+c", DevelopCopyEdit),
        ("Ctrl+Alt+Shift+c", DevelopCopySelective),
        ("Ctrl+Alt+v", DevelopPasteEdit),
        ("Ctrl+Alt+Shift+v", DevelopPasteSelective),
        ("Ctrl+Alt+r", DevelopSetReference),
        ("Ctrl+Alt+m", DevelopMatchReference),
        ("Delete", MoveToTrash),
        ("Ctrl+z", Undo),
        // Redo: Ctrl+Shift+Z (GNOME, KDE, macOS) and Ctrl+Y (Windows).
        ("Ctrl+Shift+z", Redo),
        ("Ctrl+y", Redo),
        // File operations: F2 renames everywhere (file managers on every desktop), Shift
        // renames many; F7 moves and F8 copies to a folder (IrfanView's keys, which image
        // viewer users know; F5 and F6, the file managers' copy and move, are reload here
        // and free for the desktop's pane switching).
        ("F2", Rename),
        ("Shift+F2", BatchRename),
        ("F7", MoveToFolder),
        ("F8", CopyToFolder),
        // `[` `]` turn the view only; with Alt they turn the file itself. Not Ctrl: the
        // Lightroom preset gives Ctrl+[ ] to the view's rotation, as Lightroom does.
        ("Alt+[", RotateFileLeft),
        ("Alt+]", RotateFileRight),
        ("Ctrl+o", Open),
        ("Ctrl+l", OpenAddress),
        ("Ctrl+n", OpenStream),
        ("Ctrl+,", Settings),
        ("Ctrl+k", Palette),
        ("?", CheatSheet),
        ("F1", Help),
        ("F12", Hud),
        ("Ctrl+Shift+f", Hud),
        ("F5", Reload),
        ("Ctrl+q", Quit),
        ("Ctrl+w", Close),
        // Animations (Space and [ ] are taken by next image and rotate).
        ("p", PlayPause),
        (".", NextFrame),
        (",", PrevFrame),
        ("}", FasterPlayback),
        ("{", SlowerPlayback),
        ("\\", NormalPlayback),
        // Reading mode.
        ("d", ToggleSpreads),
        ("Ctrl+r", ToggleRightToLeft),
        ("Alt+t", ToggleContents),
        // DICOM: the tag browser, and series mode's next / previous series.
        ("t", DicomTags),
        ("Ctrl+PageDown", NextSeries),
        ("Ctrl+PageUp", PrevSeries),
        // Pages of a multi-image file (ICNS sizes, texture layers, TIFF pages): the image
        // keys with Alt, as Ctrl+PageUp/PageDown already step series. None is reserved by a
        // desktop, and medical mode routes them to its frame stepping.
        ("Alt+PageDown", NextPage),
        ("Alt+PageUp", PrevPage),
        ("Alt+Home", FirstPage),
        ("Alt+End", LastPage),
        // One-shot colour cameras' raw frames (FITS, SER): debayer on or off, and the Bayer
        // pattern when the file's is wrong.
        ("b", ToggleDebayer),
        ("Alt+b", NextBayerPattern),
        // A whole-slide image's annotations (NDPA, Aperio, ASAP sidecars) on or off.
        ("Shift+a", ToggleAnnotations),
        // A thermal image's visible-light photo, or the thermal image again.
        ("Alt+v", ToggleThermalVisible),
        // Multiplanar views of any volume, whatever the profile (the views' own keys are
        // medical mode's, which MPR turns on).
        ("Shift+r", Mpr),
        // Develop: non-destructive editing of the photo shown.
        ("Shift+d", ToggleDevelop),
    ];
    // Cull mode (DESIGN §6.3): flags, ratings 0–5 and colour labels on 6–9, the folder
    // slots on Alt(+Shift)+1–3, bursts on `s` and `b`, following new files on `t`.
    let cull: &[(&str, Action)] = &[
        ("p", CullPick),
        ("x", CullReject),
        ("u", CullUnflag),
        ("0", CullRate(0)),
        ("1", CullRate(1)),
        ("2", CullRate(2)),
        ("3", CullRate(3)),
        ("4", CullRate(4)),
        ("5", CullRate(5)),
        ("6", CullLabel(1)),
        ("7", CullLabel(2)),
        ("8", CullLabel(3)),
        ("9", CullLabel(4)),
        ("Alt+1", CullCopyTo(1)),
        ("Alt+2", CullCopyTo(2)),
        ("Alt+3", CullCopyTo(3)),
        ("Alt+Shift+1", CullMoveTo(1)),
        ("Alt+Shift+2", CullMoveTo(2)),
        ("Alt+Shift+3", CullMoveTo(3)),
        ("Ctrl+Alt+s", DevelopSync),
        ("Ctrl+Alt+Shift+s", DevelopSyncSelective),
        ("s", CullBurst),
        ("b", CullKeepBest),
        ("t", CullFollow),
        // Astronomy subframes.
        ("Shift+t", CullSubframes),
        ("o", CullSortNext),
        ("Shift+o", CullSortReverse),
        // Off `m` and `k`, which stay Measure and the key image in cull mode.
        ("y", CullMetricNext),
        ("Alt+-", CullSigmaDown),
        ("Alt+=", CullSigmaUp),
        ("Shift+r", CullAutoReject),
        ("Shift+x", CullMoveRejects),
        ("e", CullSameStretch),
    ];
    // Medical mode (DESIGN §11, medical keys): the digits pick window presets instead of
    // zoom levels (Ctrl+0 and Ctrl+1 still fit and show 100 %), `,` `.` step slices, `p`
    // plays them (cine), `m` `a` `e` `r` measure, `o` shows or hides overlays, `l` the
    // orientation labels, `Shift+p` steps through the presentation states.
    let medical: &[(&str, Action)] = &[
        ("w", WindowLevelTool),
        ("Ctrl+i", InvertGrey),
        (".", NextSlice),
        (",", PrevSlice),
        ("p", Cine),
        ("m", MeasureLength),
        ("a", MeasureAngle),
        ("e", MeasureEllipse),
        ("r", MeasureRectangle),
        ("Shift+m", ClearMeasurements),
        ("o", ToggleOverlays),
        ("l", ToggleOrientationLabels),
        ("Shift+p", NextPresentationState),
        ("Shift+s", MprSlab),
        ("Alt+s", MprTractSlab),
        ("Shift+v", MprPreset),
        ("Shift+c", MprRender),
        ("Shift+x", MprClip),
        ("Shift+z", MprMaximise),
        ("Shift+e", MprExportPlanes),
        ("Alt+Shift+r", MprCurve),
        ("Shift+y", CompareSeries),
        ("Alt+j", CompareLayout),
        ("F6", NextViewport),
        ("Shift+F6", PrevViewport),
        ("Alt+h", ReferenceLines),
        ("Shift+w", SyncWindow),
        ("Alt+z", SyncZoom),
        ("Shift+j", AlignSeries),
        ("Alt+Shift+j", LinkSeries),
        ("Shift+o", Segments),
        ("Alt+o", SegmentOpacity),
        ("Shift+u", Fusion),
        ("Alt+u", FusionColormap),
        ("Shift+b", FusionBlend),
        ("Shift+n", FusionNudge),
        ("Shift+t", Tmtv),
        ("Alt+t", TmtvThreshold),
        ("Shift+k", TimeCurve),
        ("Alt+k", ExportCurve),
        ("0", WindowPreset(0)),
        ("1", WindowPreset(1)),
        ("2", WindowPreset(2)),
        ("3", WindowPreset(3)),
        ("4", WindowPreset(4)),
        ("5", WindowPreset(5)),
        ("6", WindowPreset(6)),
        ("7", WindowPreset(7)),
        ("8", WindowPreset(8)),
        ("9", WindowPreset(9)),
    ];
    // Science mode (FITS, XISF, DM, MRC, Gwyddion, Igor): `s` the stretch, `l` the limits, `,` `.` step the planes of
    // the axis `a` picks, `c` composites the channels and `1`–`9` show or hide them (`Alt+0`–`Alt+6`
    // channels 10 to 16), `w`
    // the coordinate grid, `p` levels a height map, `h` shades an elevation model's relief,
    // `Shift+f` the power spectrum, `Shift+w` wiggle traces (seismic), `Shift+m` an ion image
    // at a typed m/z (mass spectrometry), `u` the temperature unit, `Shift+t` an isotherm,
    // `Shift+e` the emissivity and reflected temperature, `Ctrl+e` saves the temperatures
    // (thermal images), `e` an unstructured mesh's edges (UGRID), `Shift+o` overlays a source
    // catalogue and `Alt+o` removes the overlaid ones, `0` the automatic look.
    let science: &[(&str, Action)] = &[
        ("s", ScienceStretch),
        ("l", ScienceLimits),
        (".", ScienceNextPlane),
        (",", SciencePrevPlane),
        ("a", ScienceNextAxis),
        ("c", ScienceComposite),
        ("1", ScienceChannel(1)),
        ("2", ScienceChannel(2)),
        ("3", ScienceChannel(3)),
        ("4", ScienceChannel(4)),
        ("5", ScienceChannel(5)),
        ("6", ScienceChannel(6)),
        ("7", ScienceChannel(7)),
        ("8", ScienceChannel(8)),
        ("9", ScienceChannel(9)),
        ("Alt+0", ScienceChannel(10)),
        ("Alt+1", ScienceChannel(11)),
        ("Alt+2", ScienceChannel(12)),
        ("Alt+3", ScienceChannel(13)),
        ("Alt+4", ScienceChannel(14)),
        ("Alt+5", ScienceChannel(15)),
        ("Alt+6", ScienceChannel(16)),
        ("w", ScienceGrid),
        ("p", ScienceLevel),
        ("h", ScienceHillshade),
        ("Shift+f", ScienceFft),
        ("Shift+w", ScienceWiggle),
        ("e", ScienceWireframe),
        ("Shift+m", ScienceIonImage),
        ("u", ScienceTemperatureUnit),
        ("Shift+t", ScienceIsotherm),
        ("Shift+e", ScienceThermalParameters),
        ("Ctrl+e", ScienceExportTemperatures),
        ("Alt+p", ScienceThermalPlay),
        ("Alt+e", ScienceThermalSeriesCsv),
        ("Shift+o", ScienceCatalogue),
        ("Alt+o", ScienceClearCatalogues),
        ("0", ScienceReset),
    ];
    // Drawing mode (DXF): `l` the layers panel, `Shift+l` every layer on, `b` dark or light
    // paper, `w` lineweights.
    let drawing: &[(&str, Action)] = &[
        ("l", DrawingLayers),
        ("Shift+l", DrawingAllLayers),
        ("b", DrawingPaper),
        ("w", DrawingLineweights),
    ];
    // Mesh mode (STL, OBJ, PLY, glTF, 3MF): Shift+arrows orbit (as in MPR's 3D view),
    // `1`–`4` the front, top, side and iso views, `s` the shading, `o` the projection, Alt+g
    // the ground grid (the pixel grid's key); with a point cloud `]` `[` make the points
    // larger and smaller (over the viewer's rotation), `c` steps the colouring and `e` turns
    // eye-dome lighting on and off; `+` `-` zoom, `f` fits and the viewer's pan
    // keys pan, through the viewer's keys.
    let mesh: &[(&str, Action)] = &[
        ("Shift+Left", MeshOrbitLeft),
        ("Shift+Right", MeshOrbitRight),
        ("Shift+Up", MeshOrbitUp),
        ("Shift+Down", MeshOrbitDown),
        ("1", MeshViewFront),
        ("2", MeshViewTop),
        ("3", MeshViewSide),
        ("4", MeshViewIso),
        ("s", MeshShading),
        ("o", MeshProjection),
        ("Alt+g", MeshGrid),
        ("]", MeshPointsBigger),
        ("[", MeshPointsSmaller),
        ("c", MeshPointColour),
        ("e", MeshEdl),
        // Molecules: `m` the next style, `Shift+C` the next colouring.
        ("m", MeshMolStyle),
        ("Shift+c", MeshMolColour),
        // Tractograms: Alt+O the next streamline opacity.
        ("Alt+o", MeshTractOpacity),
        // Review: Alt+W walk, Alt+F fly, Alt+P the saved views, Shift+S save one, Alt+X a
        // section plane, Shift+X the section box, Alt+Z flip, Alt+Shift+X no cuts, Shift+M
        // measure on the cut, Alt+I the BCF topics, Alt+Q the model checks, Alt+Shift+M the
        // models in the scene.
        ("Alt+w", MeshWalk),
        ("Alt+f", MeshFly),
        ("Alt+p", MeshViewpoints),
        ("Shift+s", MeshSaveViewpoint),
        ("Alt+x", MeshSectionPlane),
        ("Shift+x", MeshSectionBox),
        ("Alt+z", MeshSectionFlip),
        ("Alt+Shift+x", MeshSectionClear),
        ("Shift+m", MeshCutMeasure),
        ("Alt+i", MeshBcf),
        ("Alt+q", MeshChecks),
        ("Alt+Shift+m", MeshModels),
        // A CAD model's PMI and saved views (Shift+A shows or hides the annotations).
        ("Alt+a", MeshPmi),
    ];
    // Relight mode (PTM, RTI, Relight): Shift+arrows move the light (as mesh mode's orbit),
    // `s` the next shading, `l` the light widget, `0` the light straight on.
    let relight: &[(&str, Action)] = &[
        ("Shift+Left", RelightLeft),
        ("Shift+Right", RelightRight),
        ("Shift+Up", RelightUp),
        ("Shift+Down", RelightDown),
        ("s", RelightMode),
        ("l", RelightWidget),
        ("0", RelightReset),
    ];
    // Movies, as web players and mpv have them: Space or k plays and pauses, the arrows seek
    // 5 s (Shift+arrows change file), j / l 10 s, Up / Down set the volume, m mutes, digits jump to a tenth
    // of the movie (0 the start), `,` `.` step frames (from the viewer's keys). Shift+n /
    // Shift+p play the next / previous item (the queue's, else the folder's files of the same
    // kind; the arrows seek here), Alt+Shift+l cycles repeat (r moves subtitles), Shift+s shuffles. z / x move the subtitles 50 ms earlier /
    // later (Alt: 10 ms), Shift+z / Shift+x the sound, Shift+Backspace resets both;
    // Ctrl+Shift+Left / Right shift the subtitles so the previous / next one starts now. r and
    // Shift+r move the subtitles up and down (as in mpv), Ctrl+Shift+Up / Down make them
    // larger and smaller, Ctrl+Shift+r puts both back. c cycles
    // subtitles, a sound tracks, Shift+PageUp / Shift+PageDown go by chapter, b sets A-B
    // repeat, Ctrl+c copies the frame and Ctrl+s saves it, Shift+m the mini player, Alt+a the
    // picture's shape and Alt+c its crop, Ctrl+t goes to a time typed, Alt+p the picture
    // adjustments; d turns dialogue boost on and off, e opens the equaliser; Shift+o "Play
    // on…" (output to a TV or speaker).
    let video: &[(&str, Action)] = &[
        ("Space", PlayPauseVideo),
        ("k", PlayPauseVideo),
        ("Right", SeekForward),
        ("Left", SeekBack),
        ("Shift+Right", NextImage),
        ("Shift+Left", PrevImage),
        ("l", SeekForwardLong),
        ("j", SeekBackLong),
        ("Up", VolumeUp),
        ("Down", VolumeDown),
        ("0", SeekPercent(0)),
        ("1", SeekPercent(10)),
        ("2", SeekPercent(20)),
        ("3", SeekPercent(30)),
        ("4", SeekPercent(40)),
        ("5", SeekPercent(50)),
        ("6", SeekPercent(60)),
        ("7", SeekPercent(70)),
        ("8", SeekPercent(80)),
        ("9", SeekPercent(90)),
        ("Shift+n", NextTrack),
        ("Shift+p", PrevTrack),
        ("Alt+Shift+l", CycleRepeat),
        ("Shift+s", ToggleShuffle),
        ("z", SubtitleEarlier),
        ("x", SubtitleLater),
        ("Alt+z", SubtitleEarlierFine),
        ("Alt+x", SubtitleLaterFine),
        ("Shift+z", SoundEarlier),
        ("Shift+x", SoundLater),
        ("Alt+Shift+z", SoundEarlierFine),
        ("Alt+Shift+x", SoundLaterFine),
        ("Shift+Backspace", ResetDelays),
        ("Ctrl+Shift+Left", SubtitleStepPrev),
        ("Ctrl+Shift+Right", SubtitleStepNext),
        ("r", SubtitleUp),
        ("Shift+r", SubtitleDown),
        ("Ctrl+Shift+Up", SubtitleBigger),
        ("Ctrl+Shift+Down", SubtitleSmaller),
        ("Ctrl+Shift+r", SubtitleResetPlace),
        ("c", CycleSubtitles),
        ("a", CycleSound),
        ("d", DialogueBoost),
        ("e", Equaliser),
        ("Shift+PageUp", PrevChapter),
        ("Shift+PageDown", NextChapter),
        ("b", AbLoop),
        ("Ctrl+c", CopyFrame),
        ("Ctrl+s", SaveFrame),
        ("Shift+m", MiniPlayer),
        ("Alt+a", CycleAspect),
        ("Alt+c", CycleCrop),
        ("w", CycleSoundVisuals),
        ("y", ToggleLyrics),
        ("Ctrl+t", GoToTime),
        ("Alt+p", VideoPicture),
        ("Shift+o", CastMenu),
        ("m", ToggleMute),
        // Fullscreen play mode; `f` stays fit to window for still images.
        ("f", VideoFullscreen),
    ];
    // Map mode (Maps profile, over a map's drawing or science keys): `l` the layer panel,
    // `Shift+z` zooms to the chosen layer, `Shift+a` adds a layer from a file, `t` the
    // attribute table, `c` the coordinate format, `u` metric or imperial, `Alt+c` copies the
    // coordinate, `Ctrl+Shift+g` goes to one, `r` measures a distance, `Shift+r` an area,
    // `e` an elevation profile, `o` the overview map; `Alt+s` slope, `Alt+a` aspect, `k`
    // contours, `Shift+d` terrain in 3-D; `y` the time slider, `Alt+p` plays it; `Alt+w` wind
    // arrows, `Alt+y` weather colours; `Alt+n` online services; `Alt+t` photos on a track,
    // `Shift+q` geotagging; `Ctrl+Shift+e` export, `Ctrl+Shift+p` print to scale, `Alt+x`
    // copies the extent.
    let map: &[(&str, Action)] = &[
        ("l", MapLayers),
        ("Shift+a", MapAddLayer),
        ("Shift+z", MapZoomToLayer),
        ("t", MapAttributeTable),
        ("c", MapCoordinateFormat),
        ("u", MapUnits),
        ("Alt+c", MapCopyCoordinate),
        ("Ctrl+Shift+g", MapGoToCoordinate),
        ("r", MapMeasureDistance),
        ("Shift+r", MapMeasureArea),
        ("e", MapElevationProfile),
        ("o", MapOverview),
        ("Alt+s", MapSlope),
        ("Alt+a", MapAspect),
        ("k", MapContours),
        ("Shift+d", MapTerrain3d),
        ("y", MapTime),
        ("Alt+p", MapPlay),
        ("Alt+w", MapWind),
        ("Alt+y", MapWeatherColours),
        ("Alt+n", MapServices),
        ("Alt+t", MapPhotosOnTrack),
        ("Shift+q", MapGeotag),
        ("Ctrl+Shift+e", MapExport),
        ("Ctrl+Shift+p", MapPrint),
        ("Alt+x", MapCopyExtent),
    ];
    // Develop (DESIGN §6.10): before/after, the panel, and speed edit (hold a key and scroll
    // or press arrows).
    let develop: &[(&str, Action)] = &[
        ("\\", DevelopBeforeAfter),
        ("|", DevelopSplit),
        ("p", DevelopFocusPanel),
        // F6 between the picture, the panel and the filmstrip, as the gallery's regions.
        ("F6", DevelopNextRegion),
        ("Shift+F6", DevelopPrevRegion),
        ("Ctrl+Shift+e", DevelopExport),
        ("r", DevelopCrop),
        ("o", DevelopSpotOutlines),
        // The keyboard crosshair (the viewer's `x`) places retouch spots.
        ("x", DevelopSpotCrosshair),
        ("Shift+g", DevelopScopes),
        ("j", DevelopClipDisplay),
        ("Shift+j", DevelopClipScene),
        ("Alt+j", DevelopClipGamut),
        ("Shift+p", DevelopSoftProof),
        ("e", DevelopSpeedExposure),
        ("c", DevelopSpeedContrast),
        ("h", DevelopSpeedHighlights),
        ("s", DevelopSpeedShadows),
        ("w", DevelopSpeedWhites),
        ("b", DevelopSpeedBlacks),
        ("t", DevelopSpeedTemperature),
        ("v", DevelopSpeedVibrance),
        ("Ctrl+Alt+n", DevelopSnapshot),
        // Masks: `[` `]` size the brush (over the viewer's turns), with Shift its
        // softness; `m` shows the mask (over Measure); Alt+E paints or erases.
        ("[", DevelopBrushSmaller),
        ("]", DevelopBrushLarger),
        ("{", DevelopBrushSofter),
        ("}", DevelopBrushHarder),
        ("Alt+e", DevelopBrushErase),
        ("m", DevelopMaskOverlay),
    ];
    let mut layers = vec![Layer::default(); Mode::ALL.len()];
    for (mode, defs) in [
        (Mode::Viewer, viewer),
        (Mode::Cull, cull),
        (Mode::Medical, medical),
        (Mode::Science, science),
        (Mode::Drawing, drawing),
        (Mode::Mesh, mesh),
        (Mode::Relight, relight),
        (Mode::Video, video),
        (Mode::Map, map),
        (Mode::Develop, develop),
    ] {
        let l = &mut layers[mode.index()];
        for (k, a) in defs {
            let s = KeySeq::parse(k).unwrap_or_else(|| panic!("bad default key {k}"));
            l.order.retain(|(x, _)| *x != s);
            l.order.push((s, *a));
        }
        l.reindex();
    }
    layers
}

/// A TOML string literal.
fn quote(s: &str) -> String {
    let mut q = String::with_capacity(s.len() + 2);
    q.push('"');
    for c in s.chars() {
        match c {
            '"' => q.push_str("\\\""),
            '\\' => q.push_str("\\\\"),
            c if c.is_control() => q.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => q.push(c),
        }
    }
    q.push('"');
    q
}

impl Keymap {
    /// The default keymap (DESIGN §11 and §12.6).
    pub fn viewer_default() -> Keymap {
        Keymap::from_preset(Preset::Default)
    }

    /// A preset's bindings, with no changes.
    pub fn from_preset(p: Preset) -> Keymap {
        let mut layers = default_layers();
        if let Some(text) = p.toml() {
            let text = &*text;
            let mut km = Keymap {
                base: Arc::new(layers.clone()),
                layers,
                preset: Preset::Default,
                problems: Vec::new(),
                active: vec![Mode::Viewer],
                profile: Profile::UNRESTRICTED,
                device_off: Vec::new(),
            };
            let errs = km.apply_entries(text);
            debug_assert!(errs.is_empty(), "preset {}: {errs:?}", p.id());
            layers = km.layers;
        }
        Keymap {
            base: Arc::new(layers.clone()),
            layers,
            preset: p,
            problems: Vec::new(),
            active: vec![Mode::Viewer],
            profile: Profile::UNRESTRICTED,
            device_off: Vec::new(),
        }
    }

    /// The modes in effect, innermost first (the application follows what is shown: cull
    /// mode, a medical image). The viewer is always last; each mode's parents follow it.
    pub fn set_active(&mut self, modes: &[Mode]) {
        let mut v: Vec<Mode> = Vec::with_capacity(modes.len() + 1);
        for m in modes.iter().flat_map(|m| m.chain()) {
            if m != Mode::Viewer && !v.contains(&m) {
                v.push(m);
            }
        }
        v.push(Mode::Viewer);
        self.active = v;
    }

    /// The modes in effect, innermost first, ending with [`Mode::Viewer`].
    pub fn active(&self) -> &[Mode] {
        &self.active
    }

    /// The capabilities in effect. Actions the profile hides keep their bindings (the key
    /// configurator lists them) but their keys are inactive: lookups skip them, so a key
    /// another mode binds too runs that binding instead.
    pub fn set_profile(&mut self, profile: Profile) {
        self.profile = profile;
    }

    pub fn profile(&self) -> Profile {
        self.profile
    }

    /// Whether the profile shows the action (its keys work; the cheat sheet and the key
    /// configurator list it).
    pub fn shows(&self, a: Action) -> bool {
        self.profile.shows_action(a)
    }

    /// Whether the profile shows any of a mode's actions (its tab in the cheat sheet and the
    /// key configurator). MPR's tools, General in every profile, do not show medical mode's
    /// tab by themselves unless MPR is open.
    pub fn mode_shown(&self, m: Mode) -> bool {
        m.actions()
            .any(|a| self.shows(a) && (self.profile.mpr || !a.is_mpr_view()))
    }

    /// A layer's live binding of `keys`.
    fn get_live(&self, m: Mode, keys: &KeySeq) -> Option<Action> {
        self.layer(m).get(keys).filter(|a| self.shows(*a))
    }

    /// Whether a layer has a live binding that `keys` begins.
    fn wait_live(&self, m: Mode, keys: &KeySeq) -> bool {
        let l = self.layer(m);
        l.prefixes.contains(keys)
            && l.order
                .iter()
                .any(|(s, a)| s.len() > keys.len() && keys.is_prefix_of(s) && self.shows(*a))
    }

    fn layer(&self, m: Mode) -> &Layer {
        &self.layers[m.index()]
    }

    fn layer_mut(&mut self, m: Mode) -> &mut Layer {
        &mut self.layers[m.index()]
    }

    fn base_layer(&self, m: Mode) -> &Layer {
        &self.base[m.index()]
    }

    /// The preset these bindings start from.
    pub fn preset(&self) -> Preset {
        self.preset
    }

    /// What loading `keymap.toml` found wrong.
    pub fn problems(&self) -> &[KeymapError] {
        &self.problems
    }

    pub fn set_problems(&mut self, p: Vec<KeymapError>) {
        self.problems = p;
    }

    /// Bind a single key in the action's mode.
    pub fn bind(&mut self, combo: KeyCombo, action: Action) {
        self.layer_mut(action.mode())
            .bind(KeySeq::one(combo), action);
    }

    /// Remove a single key's viewer binding.
    pub fn unbind(&mut self, combo: KeyCombo) {
        self.layer_mut(Mode::Viewer).unbind(KeySeq::one(combo));
    }

    /// What a device button runs: its binding in the active modes, else its default
    /// ([`crate::device_default`]) unless the keymap switched it off.
    pub fn device_action(&self, combo: &KeyCombo) -> Option<Action> {
        self.lookup(combo).or_else(|| {
            if self.device_off.contains(combo) {
                None
            } else {
                crate::device_default(combo)
            }
        })
    }

    /// Whether `keymap.toml` or the editor switched this device button off.
    pub fn device_switched_off(&self, combo: &KeyCombo) -> bool {
        self.device_off.contains(combo)
    }

    /// Notes a device button unbound (`off`) or bound again.
    fn note_device(&mut self, keys: &KeySeq, off: bool) {
        if keys.len() != 1 || !keys.first().is_device() {
            return;
        }
        let c = keys.first();
        self.device_off.retain(|d| *d != c);
        if off && crate::device_default(&c).is_some() {
            self.device_off.push(c);
        }
    }

    /// The action of a single key in the active modes.
    pub fn lookup(&self, combo: &KeyCombo) -> Option<Action> {
        self.lookup_in(&self.active, &KeySeq::one(*combo))
    }

    /// The action of keys in the first of `modes` that binds them.
    pub fn lookup_in(&self, modes: &[Mode], keys: &KeySeq) -> Option<Action> {
        modes.iter().find_map(|m| self.get_live(*m, keys))
    }

    /// The action of keys in one mode alone.
    pub fn get(&self, mode: Mode, keys: &KeySeq) -> Option<Action> {
        self.layer(mode).get(keys)
    }

    /// Whether an active mode binds something that starts with this key.
    fn starts_any(&self, combo: &KeyCombo) -> bool {
        self.active.iter().any(|m| {
            self.layer(*m)
                .order
                .iter()
                .any(|(s, a)| s.first() == *combo && self.shows(*a))
        })
    }

    /// The combination a key press stands for. Digit-row bindings follow the key's position,
    /// not its character, so they work on every layout: `physical_digit` is the digit of the
    /// key's place in the row (US layout), `shift` whether Shift is held.
    ///
    /// - Shift with a digit-row key is `Shift+<digit>` when that is bound (US `!`, German `"`
    ///   and AZERTY `1` all mean `Shift+1`).
    /// - A digit-row key whose character has no binding of its own falls back to the digit
    ///   (AZERTY `&` is `1`).
    /// - With Alt, likewise (`Alt+Shift+1` whatever Shift types on the key).
    ///
    /// Only the active modes' bindings count.
    pub fn resolve(&self, combo: KeyCombo, shift: bool, physical_digit: Option<char>) -> KeyCombo {
        let Some(d) = physical_digit.filter(char::is_ascii_digit) else {
            return combo;
        };
        if combo.mods.ctrl || combo.mods.meta {
            return combo;
        }
        let by_place = KeyCombo {
            key: Key::Char(d),
            mods: Mods {
                shift,
                alt: combo.mods.alt,
                ..Mods::NONE
            },
        };
        let use_place = if shift {
            self.starts_any(&by_place)
        } else {
            !self.starts_any(&combo) && self.starts_any(&by_place)
        };
        if use_place { by_place } else { combo }
    }

    /// All keys that run an action in the active modes, in definition order: the innermost
    /// mode's first, leaving out keys an inner mode takes over.
    pub fn keys_for(&self, action: Action) -> Vec<KeySeq> {
        let mut v = Vec::new();
        if !self.shows(action) {
            return v;
        }
        for (i, m) in self.active.iter().enumerate() {
            for s in self.layer(*m).keys_for(action) {
                if self.active[..i]
                    .iter()
                    .all(|o| self.get_live(*o, &s).is_none())
                {
                    v.push(s);
                }
            }
        }
        v
    }

    /// All keys bound to an action in one mode.
    pub fn keys_in(&self, mode: Mode, action: Action) -> Vec<KeySeq> {
        self.layer(mode).keys_for(action)
    }

    /// The first key that runs an action in the active modes, for tooltips.
    pub fn primary_key(&self, action: Action) -> Option<KeySeq> {
        self.keys_for(action).first().copied()
    }

    /// A mode's bindings in definition order.
    pub fn bindings(&self, mode: Mode) -> &[(KeySeq, Action)] {
        &self.layer(mode).order
    }

    /// The preset's keys for an action.
    pub fn preset_keys(&self, mode: Mode, action: Action) -> Vec<KeySeq> {
        self.base_layer(mode).keys_for(action)
    }

    /// Whether an action's keys differ from the preset's.
    pub fn action_changed(&self, mode: Mode, action: Action) -> bool {
        let mut a = self.keys_in(mode, action);
        let mut b = self.preset_keys(mode, action);
        if a.len() != b.len() {
            return true;
        }
        let key = |s: &KeySeq| s.spec();
        a.sort_by_key(key);
        b.sort_by_key(key);
        a != b
    }

    /// Whether a mode's bindings differ from the preset's.
    pub fn mode_changed(&self, mode: Mode) -> bool {
        !self.layer(mode).same(self.base_layer(mode))
    }

    /// Whether any binding differs from the preset's.
    pub fn changed(&self) -> bool {
        Mode::ALL.iter().any(|m| self.mode_changed(*m))
    }

    /// Whether two keymaps bind the same keys to the same actions (order aside).
    pub fn same_bindings(&self, other: &Keymap) -> bool {
        self.preset == other.preset
            && self
                .layers
                .iter()
                .zip(other.layers.iter())
                .all(|(a, b)| a.same(b))
    }

    /// Feed one key press. `modes` are looked up in order (the active mode first, then its
    /// parents), `pending` is the [`Step::pending`] of the previous press, if it has not timed
    /// out. An earlier mode's binding wins over a later one's, a longer sequence waits.
    pub fn step(&self, modes: &[Mode], pending: Option<KeySeq>, combo: KeyCombo) -> Step {
        if let Some(p) = pending {
            if let Some(s) = p.push(combo) {
                match self.match_keys(modes, &s) {
                    Match::Run(a) => {
                        return Step {
                            run: vec![a],
                            pending: None,
                        };
                    }
                    Match::Wait => {
                        return Step {
                            run: Vec::new(),
                            pending: Some(s),
                        };
                    }
                    Match::Miss => {}
                }
            }
            // The sequence did not continue: what it bound runs, then the key counts afresh.
            let mut st = self.step(modes, None, combo);
            if let Some(a) = self.lookup_in(modes, &p) {
                st.run.insert(0, a);
            }
            return st;
        }
        let s = KeySeq::one(combo);
        match self.match_keys(modes, &s) {
            Match::Run(a) => Step {
                run: vec![a],
                pending: None,
            },
            Match::Wait => Step {
                run: Vec::new(),
                pending: Some(s),
            },
            Match::Miss => Step::default(),
        }
    }

    fn match_keys(&self, modes: &[Mode], s: &KeySeq) -> Match {
        for m in modes {
            if self.wait_live(*m, s) {
                return Match::Wait;
            }
            if let Some(a) = self.get_live(*m, s) {
                return Match::Run(a);
            }
        }
        Match::Miss
    }

    /// A pending sequence timed out: the action its keys bind, if any.
    pub fn expire(&self, modes: &[Mode], pending: KeySeq) -> Option<Action> {
        self.lookup_in(modes, &pending)
    }

    /// The bindings a pending sequence can still become (for the which-key hint), in the
    /// first mode that has any.
    pub fn continuations(&self, modes: &[Mode], pending: &KeySeq) -> Vec<(KeySeq, Action)> {
        for m in modes {
            let v: Vec<_> = self
                .layer(*m)
                .order
                .iter()
                .filter(|(s, a)| pending.is_prefix_of(s) && self.shows(*a))
                .copied()
                .collect();
            if !v.is_empty() {
                return v;
            }
        }
        Vec::new()
    }

    /// Apply an edit from Settings → Keys. Returns whether anything changed.
    pub fn apply_edit(&mut self, edit: &KeymapEdit) -> bool {
        let before = self.layers.clone();
        let device_before = self.device_off.clone();
        let preset = self.preset;
        match *edit {
            KeymapEdit::Bind {
                mode,
                keys,
                action,
                old,
                displace,
            } => {
                let l = self.layer_mut(mode);
                let holder = l.get(&keys).filter(|a| *a != action);
                if displace == Displace::Replace {
                    l.order.retain(|(s, a)| {
                        Some(*s) == old && *a == action
                            || !(s.is_prefix_of(&keys) || keys.is_prefix_of(s))
                    });
                }
                let swap = (displace == Displace::Swap)
                    .then_some(())
                    .and(holder)
                    .zip(old);
                let at_old =
                    old.and_then(|o| l.order.iter().position(|(s, a)| *s == o && *a == action));
                let at_keys = l.order.iter().position(|(s, _)| *s == keys);
                match (at_old, swap) {
                    (Some(i), Some((other, o))) => {
                        l.order[i] = (keys, action);
                        if let Some(j) = at_keys {
                            l.order[j] = (o, other);
                        }
                    }
                    (Some(i), None) => {
                        l.order[i] = (keys, action);
                        if let Some(j) = at_keys
                            && j != i
                        {
                            l.order.remove(j);
                        }
                    }
                    (None, _) => {
                        l.order.retain(|(s, _)| *s != keys);
                        l.order.push((keys, action));
                    }
                }
                l.reindex();
                self.note_device(&keys, false);
            }
            KeymapEdit::Unbind { mode, keys } => {
                // A device button with a default and no binding of its own is switched off.
                let had = self.layer_mut(mode).unbind(keys);
                if !had {
                    self.note_device(&keys, true);
                }
            }
            KeymapEdit::ResetAction { mode, action } => {
                let want = self.preset_keys(mode, action);
                let l = self.layer_mut(mode);
                l.order.retain(|(_, a)| *a != action);
                for s in want {
                    if l.order.iter().all(|(k, _)| *k != s) {
                        l.order.push((s, action));
                    }
                }
                l.reindex();
            }
            KeymapEdit::ResetMode(mode) => {
                let b = self.base_layer(mode).clone();
                *self.layer_mut(mode) = b;
                if mode == Mode::Viewer {
                    self.device_off.clear();
                }
            }
            KeymapEdit::ResetAll => {
                self.layers = (*self.base).clone();
                self.device_off.clear();
            }
            KeymapEdit::Preset(p) => {
                let fresh = Keymap::from_preset(p);
                self.layers = fresh.layers;
                self.base = fresh.base;
                self.preset = p;
                self.device_off.clear();
            }
        }
        preset != self.preset
            || device_before != self.device_off
            || self
                .layers
                .iter()
                .zip(before.iter())
                .any(|(a, b)| a.order != b.order)
    }

    /// Apply a user `keymap.toml` on top of these bindings (see [`Keymap::to_toml`] for the
    /// format). Returns the problems found; valid entries are applied regardless, except
    /// that a syntax error applies nothing.
    pub fn apply_toml(&mut self, text: &str) -> Vec<KeymapError> {
        self.apply_toml_at(text)
            .into_iter()
            .map(|p| p.error)
            .collect()
    }

    /// [`Keymap::apply_toml`], with where each problem is.
    fn apply_toml_at(&mut self, text: &str) -> Vec<KeymapProblem> {
        let mut errs = Vec::new();
        let doc = match toml_edit::Document::parse(text) {
            Ok(d) => d,
            Err(e) => return vec![syntax(&e)],
        };
        if let Some(p) = doc.get("preset") {
            let at = p.span();
            match p.as_str().map(|s| (s, Preset::from_id(s))) {
                Some((_, Some(p))) => {
                    self.apply_edit(&KeymapEdit::Preset(p));
                }
                Some((s, None)) => errs.push(KeymapProblem::new(
                    KeymapError::UnknownPreset(s.to_string()),
                    at,
                )),
                None => errs.push(KeymapProblem::new(
                    KeymapError::Syntax("preset must be a string".into()),
                    at,
                )),
            }
        }
        errs.extend(self.apply_doc(&doc));
        errs
    }

    /// `replace` and the mode tables of a keymap file (not its preset).
    fn apply_entries(&mut self, text: &str) -> Vec<KeymapError> {
        let errs = match toml_edit::Document::parse(text) {
            Ok(d) => self.apply_doc(&d),
            Err(e) => vec![syntax(&e)],
        };
        errs.into_iter().map(|p| p.error).collect()
    }

    fn apply_doc(&mut self, doc: &toml_edit::Document<&str>) -> Vec<KeymapProblem> {
        let mut errs = Vec::new();
        let mut err = |e: KeymapError, at: Option<Range<usize>>| {
            errs.push(KeymapProblem::new(e, at));
        };
        if let Some(r) = doc.get("replace") {
            let names: Vec<(&str, Option<Range<usize>>)> = r
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| Some((v.as_str()?, v.span())))
                        .collect()
                })
                .unwrap_or_default();
            if names.is_empty() && r.as_array().is_none_or(|a| !a.is_empty()) {
                err(
                    KeymapError::Syntax("replace must be a list of modes, e.g. [\"cull\"]".into()),
                    r.span(),
                );
            }
            for (n, at) in names {
                match Mode::from_id(n) {
                    Some(m) => *self.layer_mut(m) = Layer::default(),
                    None => err(KeymapError::UnknownMode(n.to_string()), at),
                }
            }
        }
        let mut touched: Vec<(Mode, KeySeq, Action, Option<Range<usize>>)> = Vec::new();
        for (name, item) in doc.iter() {
            if name == "preset" || name == "replace" {
                continue;
            }
            let name_at = doc.key(name).and_then(|k| k.span());
            let Some(mode) = Mode::from_id(name) else {
                err(KeymapError::UnknownMode(name.to_string()), name_at);
                continue;
            };
            let Some(table) = item.as_table_like() else {
                err(
                    KeymapError::Syntax(format!("[{name}] must be a table")),
                    item.span().or(name_at),
                );
                continue;
            };
            let mut seen: HashMap<KeySeq, Action> = HashMap::new();
            for (k, v) in table.iter() {
                let key_at = table.key(k).and_then(|k| k.span());
                let Some(keys) = KeySeq::parse(k) else {
                    err(KeymapError::UnknownKey(k.to_string()), key_at);
                    continue;
                };
                let Some(a) = v.as_str() else {
                    err(
                        KeymapError::Syntax(format!("value for {k:?} must be a string")),
                        v.span().or(key_at),
                    );
                    continue;
                };
                if a.trim() == "none" {
                    self.layer_mut(mode).unbind(keys);
                    self.note_device(&keys, true);
                    seen.remove(&keys);
                    continue;
                }
                let Some(action) = Action::from_id(a) else {
                    err(
                        KeymapError::UnknownAction {
                            key: k.to_string(),
                            action: a.to_string(),
                        },
                        v.span().or(key_at),
                    );
                    continue;
                };
                if let Some(other) = seen.insert(keys, action)
                    && other != action
                {
                    err(
                        KeymapError::Duplicate {
                            mode,
                            keys,
                            action,
                            other,
                        },
                        key_at.clone(),
                    );
                }
                self.layer_mut(mode).bind(keys, action);
                self.note_device(&keys, false);
                touched.push((mode, keys, action, key_at));
            }
        }
        // Prefix clashes and reserved keys the file introduced (the presets have none). Each
        // binding looks up its own beginnings and, only when it begins others, scans for
        // them: a file of thousands of bindings is checked as it is typed.
        let mut reported: HashSet<(Mode, KeySeq, KeySeq)> = HashSet::new();
        for (mode, keys, action, at) in touched {
            let l = self.layer(mode);
            if l.get(&keys) != Some(action) {
                continue;
            }
            if let Some(reason) = keys.keys().iter().find_map(|c| crate::reserved(c, action))
                && reported.insert((mode, keys, keys))
            {
                errs.push(KeymapProblem::new(
                    KeymapError::Reserved { mode, keys, reason },
                    at.clone(),
                ));
            }
            let mut pairs: Vec<(KeySeq, KeySeq)> = (1..keys.len())
                .filter_map(|n| KeySeq::from_keys(&keys.keys()[..n]))
                .filter(|p| l.map.contains_key(p))
                .map(|p| (p, keys))
                .collect();
            if l.prefixes.contains(&keys) {
                pairs.extend(
                    l.order
                        .iter()
                        .filter(|(s, _)| keys.is_prefix_of(s))
                        .map(|(s, _)| (keys, *s)),
                );
            }
            for (short, long) in pairs {
                if reported.insert((mode, short, long)) {
                    let e = KeymapError::Prefix {
                        mode,
                        keys: short,
                        longer: long,
                    };
                    errs.push(KeymapProblem::new(e, at.clone()));
                }
            }
        }
        errs
    }

    /// The problems of a `keymap.toml` text, each with its place in the text (for an editor
    /// or a linter); the same checks as loading it.
    pub fn check(text: &str) -> Vec<KeymapProblem> {
        Keymap::viewer_default().apply_toml_at(text)
    }

    /// Read a keymap file: the default bindings, or its preset's, with its changes.
    pub fn from_toml(text: &str) -> (Keymap, Vec<KeymapError>) {
        let mut km = Keymap::viewer_default();
        let errs = km.apply_toml(text);
        km.problems = errs.clone();
        (km, errs)
    }

    /// Load `keymap.toml` from the config directory if present (a missing file is no
    /// problem: the defaults apply).
    pub fn load_user(path: &std::path::Path) -> (Keymap, Vec<KeymapError>) {
        match std::fs::read_to_string(path) {
            Ok(text) => Keymap::from_toml(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                (Keymap::viewer_default(), Vec::new())
            }
            Err(e) => {
                let errs = vec![KeymapError::Syntax(e.to_string())];
                let mut km = Keymap::viewer_default();
                km.problems = errs.clone();
                (km, errs)
            }
        }
    }

    /// Import a keymap file chosen by the user. A file that says `replace = [...]` (one
    /// mode's export) changes only those modes; any other file replaces every binding, from
    /// its preset or the defaults. A syntax error imports nothing.
    pub fn import(&mut self, text: &str) -> Result<Vec<KeymapError>, KeymapError> {
        let doc: toml_edit::DocumentMut = text
            .parse()
            .map_err(|e: toml_edit::TomlError| KeymapError::Syntax(e.to_string()))?;
        let partial = doc.get("replace").is_some() && doc.get("preset").is_none();
        let mut km = if partial {
            self.clone()
        } else {
            Keymap::viewer_default()
        };
        let errs = km.apply_toml(text);
        km.problems = errs.clone();
        km.active = std::mem::take(&mut self.active);
        *self = km;
        Ok(errs)
    }

    /// The `keymap.toml` text: the preset, then each mode's differences from it. Keys map to
    /// action IDs; `"none"` removes one of the preset's bindings.
    pub fn to_toml(&self) -> String {
        let mut out = String::from(HEADER);
        if self.preset != Preset::Default {
            out.push_str(&format!("preset = {}\n", quote(self.preset.id())));
        }
        for mode in Mode::ALL {
            let l = self.layer(mode);
            let b = self.base_layer(mode);
            let mut lines = Vec::new();
            for (s, a) in &b.order {
                if l.get(s).is_none() {
                    lines.push(format!("{} = \"none\"  # was {}", quote(&s.spec()), a.id()));
                }
            }
            for (s, a) in &l.order {
                if b.get(s) != Some(*a) {
                    lines.push(format!("{} = {}", quote(&s.spec()), quote(&a.id())));
                }
            }
            if mode == Mode::Viewer {
                for c in &self.device_off {
                    let was = crate::device_default(c).map_or_else(String::new, |a| a.id());
                    lines.push(format!("{} = \"none\"  # was {was}", quote(&c.spec())));
                }
            }
            if !lines.is_empty() {
                out.push_str(&format!("\n[{}]\n", mode.id()));
                for line in lines {
                    out.push_str(&line);
                    out.push('\n');
                }
            }
        }
        out
    }

    /// One mode's bindings in full, to share: importing the file replaces only that mode.
    pub fn mode_to_toml(&self, mode: Mode) -> String {
        let mut out = String::from(HEADER);
        out.push_str(&format!(
            "# The {} mode only: importing this file replaces that mode's bindings.\n",
            mode.id()
        ));
        out.push_str(&format!(
            "replace = [{}]\n\n[{}]\n",
            quote(mode.id()),
            mode.id()
        ));
        for (s, a) in &self.layer(mode).order {
            out.push_str(&format!("{} = {}\n", quote(&s.spec()), quote(&a.id())));
        }
        out
    }
}

enum Match {
    Run(Action),
    Wait,
    Miss,
}

fn syntax(e: &toml_edit::TomlError) -> KeymapProblem {
    KeymapProblem::new(KeymapError::Syntax(e.to_string()), e.span())
}

const HEADER: &str = "\
# Specular key bindings. Settings → Keys edits this file; changes made by hand are picked up
# while Specular runs. Each [table] is a mode: keys (\"Ctrl+Shift+c\", sequences \"g g\") map
# to action IDs, and \"none\" removes a binding of the preset.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_is_ctrl_on_macos() {
        // ⌘F on a Mac is the keymap's Ctrl+F; the Control key is its Super.
        let cmd = Mods::held(Os::MacOs, false, false, false, true);
        assert_eq!(cmd, Mods::CTRL);
        let control = Mods::held(Os::MacOs, true, false, false, false);
        assert!(control.meta && !control.ctrl);
        // Elsewhere Ctrl is Ctrl and the logo key is Super.
        for os in [Os::Windows, Os::Unix] {
            assert_eq!(Mods::held(os, true, false, false, false), Mods::CTRL);
            let logo = Mods::held(os, false, false, false, true);
            assert!(logo.meta && !logo.ctrl);
        }
        assert_eq!(
            Mods::held(Os::MacOs, false, true, true, true),
            Mods {
                ctrl: true,
                shift: true,
                alt: true,
                meta: false
            }
        );
    }

    fn seq(s: &str) -> KeySeq {
        KeySeq::parse(s).unwrap()
    }

    /// MPR's keys (Shift+R opens it, Shift+S cycles the slab) reach MPR in every preset:
    /// no preset binds them in a mode that comes first, nor clashes with them in Medical.
    #[test]
    fn mpr_keys_work_in_every_preset() {
        use crate::{Capabilities, Capability, Preset};
        let medical = Capabilities::GENERAL.with(Capability::Medical);
        for p in Preset::ALL {
            let mut km = Keymap::from_preset(p);
            km.set_profile(Profile::of(medical));
            km.set_active(&[Mode::Medical, Mode::Viewer]);
            let run = |k: &str| km.step(km.active(), None, seq(k).first()).run;
            assert_eq!(run("Shift+r"), vec![Action::Mpr], "{}", p.id());
            assert_eq!(run("Shift+s"), vec![Action::MprSlab], "{}", p.id());
            assert_eq!(run("R"), vec![Action::Mpr], "{}: R is Shift+r", p.id());
        }
    }

    /// MPR opens on any volume in every profile: from the viewer's and science mode's
    /// layers, with General only. While it is open (medical mode's layer on top) its tier-1
    /// keys work and the medical workflows' fall through.
    #[test]
    fn mpr_keys_work_without_medical() {
        use crate::{Capabilities, Preset};
        for p in Preset::ALL {
            let mut km = Keymap::from_preset(p);
            km.set_profile(Profile::of(Capabilities::GENERAL));
            for modes in [
                &[Mode::Viewer][..],
                &[Mode::Science, Mode::Viewer],
                &[Mode::Medical, Mode::Viewer],
            ] {
                km.set_active(modes);
                let run = |k: &str| km.step(km.active(), None, seq(k).first()).run;
                assert_eq!(run("Shift+r"), vec![Action::Mpr], "{} {modes:?}", p.id());
            }
            km.set_active(&[Mode::Medical, Mode::Viewer]);
            km.set_profile(Profile {
                mpr: true,
                ..Profile::of(Capabilities::GENERAL)
            });
            let run = |k: &str| km.step(km.active(), None, seq(k).first()).run;
            assert_eq!(run("Shift+s"), vec![Action::MprSlab], "{}", p.id());
            assert_eq!(run("Shift+z"), vec![Action::MprMaximise], "{}", p.id());
            assert_eq!(run("Shift+e"), vec![Action::MprExportPlanes], "{}", p.id());
            assert_eq!(run("."), vec![Action::NextSlice], "{}", p.id());
            assert_eq!(run("0"), vec![Action::WindowPreset(0)], "{}", p.id());
            // CT presets and fusion are the Medical profile's.
            assert_ne!(run("1"), vec![Action::WindowPreset(1)], "{}", p.id());
            assert_eq!(run("Shift+u"), vec![], "{}", p.id());
            // So are the 3D view's CT and MR presets (a microscopy stack showed "MR").
            assert_ne!(run("Shift+v"), vec![Action::MprPreset], "{}", p.id());
        }
        let p = Profile {
            mpr: true,
            ..Profile::of(Capabilities::GENERAL)
        };
        assert!(!p.shows_action(Action::MprPreset));
        assert!(p.shows_action(Action::MprRender) && p.shows_action(Action::MprSlab));
        let medical = Profile {
            mpr: true,
            ..Profile::of(Capabilities::GENERAL.with(crate::Capability::Medical))
        };
        assert!(medical.shows_action(Action::MprPreset));
    }

    #[test]
    fn hidden_actions_keys_fall_through() {
        use crate::{Capabilities, Capability};
        let mut km = Keymap::viewer_default();
        km.set_active(&[Mode::Medical]);
        let medical = Capabilities::GENERAL.with(Capability::Medical);
        let run = |km: &Keymap, k: &str| km.step(km.active(), None, seq(k).first()).run;
        // A CT in a General profile: viewing tools work (MPR among them), workflows do not.
        km.set_profile(Profile {
            content: medical,
            ..Profile::of(Capabilities::GENERAL)
        });
        assert_eq!(run(&km, "Shift+r"), vec![Action::Mpr]);
        assert_eq!(run(&km, "Shift+u"), vec![]);
        assert_eq!(run(&km, "1"), vec![Action::WindowPreset(1)]);
        assert_eq!(run(&km, "m"), vec![Action::MeasureLength]);
        assert_eq!(km.primary_key(Action::Fusion), None);
        assert!(km.lookup(&seq("Shift+u").first()).is_none());
        // The binding stays for the key configurator.
        assert_eq!(
            km.keys_in(Mode::Medical, Action::Fusion),
            vec![seq("Shift+u")]
        );
        // Turned on for the session: the workflows' keys work.
        km.set_profile(Profile {
            content: medical,
            session: medical,
            ..Profile::of(Capabilities::GENERAL)
        });
        assert_eq!(run(&km, "Shift+u"), vec![Action::Fusion]);
        assert_eq!(run(&km, "Shift+s"), vec![Action::MprSlab]);
        // No medical content and Medical off: the digit falls through to the viewer's zoom.
        km.set_profile(Profile::of(Capabilities::GENERAL));
        assert_eq!(run(&km, "1"), vec![Action::ZoomTo(100.0)]);
        assert_eq!(run(&km, "Shift+u"), vec![]);
        assert_eq!(km.primary_key(Action::ZoomTo(100.0)), Some(seq("1")));
        // On in the settings: everything as before.
        km.set_profile(Profile::of(medical));
        assert_eq!(run(&km, "Shift+u"), vec![Action::Fusion]);
        assert_eq!(run(&km, "1"), vec![Action::WindowPreset(1)]);
    }

    #[test]
    fn parse_and_lookup() {
        let km = Keymap::viewer_default();
        let c = KeyCombo::new(Key::Char('K'), Mods::CTRL);
        assert_eq!(c, KeyCombo::new(Key::Char('k'), Mods::CTRL_SHIFT));
        assert_eq!(
            km.lookup(&KeyCombo::new(Key::Char('k'), Mods::CTRL)),
            Some(Action::Palette)
        );
        assert_eq!(
            km.lookup(&KeyCombo::new(Key::Char('?'), Mods::SHIFT)),
            Some(Action::CheatSheet)
        );
        assert_eq!(
            km.lookup(&KeyCombo::new(Key::Char('C'), Mods::CTRL_SHIFT)),
            Some(Action::CopyPath)
        );
        assert_eq!(
            km.lookup(&KeyCombo::new(Key::Named(NamedKey::ArrowRight), Mods::NONE)),
            Some(Action::NextImage)
        );
        assert_eq!(
            KeyCombo::parse("Ctrl+Shift+c").unwrap().to_string(),
            if cfg!(target_os = "macos") {
                "⇧⌘C"
            } else {
                "Ctrl+Shift+C"
            }
        );
        assert_eq!(Action::from_id("zoom-to:200"), Some(Action::ZoomTo(200.0)));
    }

    #[test]
    fn toml_overrides() {
        let mut km = Keymap::viewer_default();
        let errs = km
            .apply_toml("[viewer]\n\"x\" = \"trash\"\n\"Delete\" = \"none\"\n\"y\" = \"bogus\"\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            km.lookup(&KeyCombo::parse("x").unwrap()),
            Some(Action::MoveToTrash)
        );
        assert_eq!(km.lookup(&KeyCombo::parse("Delete").unwrap()), None);
    }

    #[test]
    fn digit_row_follows_the_key_position() {
        let km = Keymap::viewer_default();
        let shift1 = KeyCombo::parse("Shift+1").unwrap();
        assert_eq!(
            shift1,
            KeyCombo {
                key: Key::Char('1'),
                mods: Mods::SHIFT
            }
        );
        if !cfg!(target_os = "macos") {
            assert_eq!(shift1.to_string(), "Shift+1");
        }
        assert_eq!(km.lookup(&shift1), Some(Action::ChannelRed));
        assert_eq!(
            km.lookup(&KeyCombo::parse("Shift+5").unwrap()),
            Some(Action::ChannelLuma)
        );
        let press = |c: char, shift: bool, digit: Option<char>| {
            let mods = if shift { Mods::SHIFT } else { Mods::NONE };
            km.lookup(&km.resolve(KeyCombo::new(Key::Char(c), mods), shift, digit))
        };
        // US: Shift+1 types "!", German Shift+2 types '"', AZERTY Shift+& types "1".
        assert_eq!(press('!', true, Some('1')), Some(Action::ChannelRed));
        assert_eq!(press('"', true, Some('2')), Some(Action::ChannelGreen));
        assert_eq!(press('3', true, Some('3')), Some(Action::ChannelBlue));
        assert_eq!(press('$', true, Some('4')), Some(Action::ChannelAlpha));
        assert_eq!(press('%', true, Some('5')), Some(Action::ChannelLuma));
        // Unshifted: the character when bound, else the digit (AZERTY "&" → 1, "é" → 2).
        assert_eq!(press('1', false, Some('1')), Some(Action::ZoomTo(100.0)));
        assert_eq!(press('&', false, Some('1')), Some(Action::ZoomTo(100.0)));
        assert_eq!(press('é', false, Some('2')), Some(Action::ZoomTo(200.0)));
        // A bound character keeps its own action ("-" on the AZERTY 6 key).
        assert_eq!(press('-', false, Some('6')), Some(Action::ZoomOut));
        // Shift+digit with nothing bound keeps the character ("?" on the AZERTY comma
        // key is not a digit-row key; Shift+9 is unbound).
        assert_eq!(press('(', true, Some('9')), None);
        assert_eq!(press('?', true, None), Some(Action::CheatSheet));
        // Ctrl shortcuts are untouched.
        let ctrl1 = KeyCombo::new(Key::Char('1'), Mods::CTRL);
        assert_eq!(km.resolve(ctrl1, false, Some('1')), ctrl1);
        // Only the active modes' digits count: AZERTY "ç" on the 9 key is 9 in cull mode,
        // where 9 is bound, and stays "ç" in the viewer, where it is not.
        let cedilla = KeyCombo::new(Key::Char('ç'), Mods::NONE);
        assert_eq!(km.resolve(cedilla, false, Some('9')), cedilla);
        let mut cull = km.clone();
        cull.set_active(&[Mode::Cull]);
        assert_eq!(
            cull.resolve(cedilla, false, Some('9')),
            KeyCombo::parse("9").unwrap()
        );
        // Alt with the digit row: US Alt+Shift+1 types "!", the folder slot still sees 1.
        let alt_shift = Mods {
            alt: true,
            shift: true,
            ..Mods::NONE
        };
        let bang = KeyCombo::new(Key::Char('!'), alt_shift);
        assert_eq!(
            cull.lookup(&cull.resolve(bang, true, Some('1'))),
            Some(Action::CullMoveTo(1))
        );
        let alt_amp = KeyCombo::new(
            Key::Char('&'),
            Mods {
                alt: true,
                ..Mods::NONE
            },
        );
        assert_eq!(
            cull.lookup(&cull.resolve(alt_amp, false, Some('1'))),
            Some(Action::CullCopyTo(1))
        );
        assert_eq!(km.resolve(alt_amp, false, Some('1')), alt_amp);
    }

    #[test]
    fn specs_round_trip() {
        for s in [
            "Ctrl+Shift+c",
            "Right",
            "Shift+1",
            "Plus",
            "Ctrl+Plus",
            "F5",
            "?",
            "G",
            "Ctrl+-",
            "Alt+g",
            "Super+Space",
            "\\",
            "\"",
            "#",
            "Escape",
        ] {
            let c = KeyCombo::parse(s).unwrap_or_else(|| panic!("{s}"));
            assert_eq!(KeyCombo::parse(&c.spec()), Some(c), "{s} -> {}", c.spec());
        }
        let g = seq("g g");
        assert_eq!(g.len(), 2);
        assert_eq!(g.spec(), "g g");
        assert_eq!(seq("  r   1 ").spec(), "r 1");
        assert!(seq("g").is_prefix_of(&g));
        assert!(!g.is_prefix_of(&g));
        assert_eq!(KeySeq::parse("a b c d"), None);
        assert_eq!(KeySeq::parse(""), None);
        assert_eq!(KeySeq::parse("Ctrl+Bogus"), None);
        assert_eq!(seq("Shift+i"), seq("I"));
    }

    #[test]
    fn modes_and_merge() {
        let text = "\
[viewer]
\"g g\" = \"first\"
\"g\" = \"none\"
\"Ctrl+k\" = \"palette\"
\"ctrl+k\" = \"settings\"
\"q\" = \"cull-pick\"
\"Hyper+x\" = \"next\"

[cull]
\"t\" = \"cull-pick\"
\"p\" = \"none\"

[library]
\"x\" = \"next\"
";
        let (km, errs) = Keymap::from_toml(text);
        assert!(errs.contains(&KeymapError::Duplicate {
            mode: Mode::Viewer,
            keys: seq("Ctrl+k"),
            action: Action::Settings,
            other: Action::Palette,
        }));
        assert!(errs.contains(&KeymapError::UnknownKey("Hyper+x".into())));
        assert!(errs.contains(&KeymapError::UnknownMode("library".into())));
        assert_eq!(errs.len(), 3, "{errs:?}");
        assert_eq!(km.problems(), &errs[..]);
        assert_eq!(km.get(Mode::Viewer, &seq("g g")), Some(Action::FirstImage));
        assert_eq!(km.get(Mode::Viewer, &seq("g")), None);
        assert_eq!(km.get(Mode::Viewer, &seq("Ctrl+k")), Some(Action::Settings));
        // A cull action may be bound in the viewer table; it only runs in cull mode.
        assert_eq!(km.get(Mode::Viewer, &seq("q")), Some(Action::CullPick));
        assert_eq!(km.get(Mode::Cull, &seq("t")), Some(Action::CullPick));
        assert_eq!(km.get(Mode::Cull, &seq("p")), None);
        // The rest of the defaults stay.
        assert_eq!(km.get(Mode::Cull, &seq("x")), Some(Action::CullReject));
        assert_eq!(
            km.get(Mode::Viewer, &seq("Delete")),
            Some(Action::MoveToTrash)
        );
        // A syntax error applies nothing.
        let (km, errs) = Keymap::from_toml("[viewer\n");
        assert!(matches!(errs[..], [KeymapError::Syntax(_)]));
        assert!(!km.changed());
    }

    #[test]
    fn prefix_clashes_are_reported() {
        let (km, errs) = Keymap::from_toml("[viewer]\n\"g g\" = \"first\"\n");
        assert_eq!(
            errs,
            vec![KeymapError::Prefix {
                mode: Mode::Viewer,
                keys: seq("g"),
                longer: seq("g g"),
            }]
        );
        // Both stay: `g` waits for the timeout.
        assert_eq!(km.get(Mode::Viewer, &seq("g")), Some(Action::ToggleGallery));
        let g = seq("g").first();
        let s1 = km.step(&[Mode::Viewer], None, g);
        assert_eq!(s1.run, vec![]);
        assert_eq!(s1.pending, Some(seq("g")));
        assert_eq!(
            km.expire(&[Mode::Viewer], seq("g")),
            Some(Action::ToggleGallery)
        );
        let s2 = km.step(&[Mode::Viewer], s1.pending, g);
        assert_eq!(s2.run, vec![Action::FirstImage]);
        assert_eq!(s2.pending, None);
        // `g` then `i`: the gallery, then the info panel.
        let i = seq("i").first();
        let s3 = km.step(&[Mode::Viewer], s1.pending, i);
        assert_eq!(s3.run, vec![Action::ToggleGallery, Action::ToggleInfo]);
        assert_eq!(
            km.continuations(&[Mode::Viewer], &seq("g")),
            vec![(seq("g g"), Action::FirstImage)]
        );
    }

    #[test]
    fn modes_shadow_their_parents() {
        let km = Keymap::viewer_default();
        let p = seq("p").first();
        let cull = [Mode::Cull, Mode::Viewer];
        assert_eq!(km.step(&cull, None, p).run, vec![Action::CullPick]);
        assert_eq!(
            km.step(&[Mode::Viewer], None, p).run,
            vec![Action::PlayPause]
        );
        // Keys the cull layer leaves alone fall through to the viewer.
        let right = seq("Right").first();
        assert_eq!(km.step(&cull, None, right).run, vec![Action::NextImage]);
        assert_eq!(
            km.step(&cull, None, seq("1").first()).run,
            vec![Action::CullRate(1)]
        );
        assert_eq!(km.step(&cull, None, seq("F9").first()), Step::default());
    }

    #[test]
    fn f_is_fit_for_images_and_full_screen_play_for_movies() {
        let km = Keymap::viewer_default();
        let f = seq("f").first();
        assert_eq!(km.step(&[Mode::Viewer], None, f).run, vec![Action::ZoomFit]);
        let video = [Mode::Video, Mode::Viewer];
        assert_eq!(km.step(&video, None, f).run, vec![Action::VideoFullscreen]);
        // In cull mode over a movie too.
        let cull_video = [Mode::Cull, Mode::Video, Mode::Viewer];
        assert_eq!(
            km.step(&cull_video, None, f).run,
            vec![Action::VideoFullscreen]
        );
        // F11 and Enter still reach the window's full screen in a movie.
        assert_eq!(
            km.step(&video, None, seq("F11").first()).run,
            vec![Action::ToggleFullscreen]
        );
        assert_eq!(
            km.get(Mode::Video, &seq("f")),
            Some(Action::VideoFullscreen)
        );
        assert_eq!(km.get(Mode::Viewer, &seq("f")), Some(Action::ZoomFit));
        assert_eq!(
            Action::from_id("video-fullscreen"),
            Some(Action::VideoFullscreen)
        );
    }

    #[test]
    fn a_user_keymap_moves_full_screen_play() {
        let (km, errs) =
            Keymap::from_toml("[video]\n\"F10\" = \"video-fullscreen\"\n\"f\" = \"none\"\n");
        assert!(errs.is_empty(), "{errs:?}");
        let video = [Mode::Video, Mode::Viewer];
        assert_eq!(
            km.step(&video, None, seq("F10").first()).run,
            vec![Action::VideoFullscreen]
        );
        // With `f` unbound in movies it falls through to fit.
        assert_eq!(
            km.step(&video, None, seq("f").first()).run,
            vec![Action::ZoomFit]
        );
    }

    #[test]
    fn export_import_round_trip() {
        let mut km = Keymap::viewer_default();
        assert_eq!(km.to_toml(), HEADER);
        km.apply_edit(&KeymapEdit::Bind {
            mode: Mode::Viewer,
            keys: seq("x"),
            action: Action::MoveToTrash,
            old: Some(seq("Delete")),
            displace: Displace::Replace,
        });
        km.apply_edit(&KeymapEdit::Bind {
            mode: Mode::Viewer,
            keys: seq("g g"),
            action: Action::FirstImage,
            old: None,
            displace: Displace::Keep,
        });
        km.apply_edit(&KeymapEdit::Bind {
            mode: Mode::Cull,
            keys: seq("Ctrl+Shift+\""),
            action: Action::CullLabel(4),
            old: None,
            displace: Displace::Keep,
        });
        km.apply_edit(&KeymapEdit::Unbind {
            mode: Mode::Cull,
            keys: seq("u"),
        });
        let text = km.to_toml();
        let (back, errs) = Keymap::from_toml(&text);
        // The kept `g`/`g g` prefix clash is reported again on load.
        assert!(
            errs.iter().all(|e| matches!(e, KeymapError::Prefix { .. })),
            "{errs:?}"
        );
        assert!(back.same_bindings(&km), "{text}");
        assert!(back.action_changed(Mode::Viewer, Action::MoveToTrash));
        assert!(!back.action_changed(Mode::Viewer, Action::NextImage));
        assert!(back.mode_changed(Mode::Cull));
        // The same through import, which replaces everything.
        let mut other = Keymap::from_preset(Preset::Vim);
        other.import(&text).unwrap();
        assert!(other.same_bindings(&km));
        // A preset with changes keeps its name.
        let mut vim = Keymap::from_preset(Preset::Vim);
        vim.apply_edit(&KeymapEdit::Unbind {
            mode: Mode::Viewer,
            keys: seq("l"),
        });
        let (back, errs) = Keymap::from_toml(&vim.to_toml());
        assert_eq!(errs, vec![]);
        assert_eq!(back.preset(), Preset::Vim);
        assert!(back.same_bindings(&vim));
    }

    #[test]
    fn single_mode_export_merges_on_import() {
        let mut src = Keymap::viewer_default();
        src.apply_edit(&KeymapEdit::Bind {
            mode: Mode::Cull,
            keys: seq("t"),
            action: Action::CullPick,
            old: Some(seq("p")),
            displace: Displace::Replace,
        });
        let cull = src.mode_to_toml(Mode::Cull);
        let mut dst = Keymap::from_preset(Preset::Vim);
        dst.apply_edit(&KeymapEdit::Unbind {
            mode: Mode::Viewer,
            keys: seq("h"),
        });
        assert_eq!(dst.import(&cull), Ok(vec![]));
        assert_eq!(dst.preset(), Preset::Vim);
        assert_eq!(dst.get(Mode::Cull, &seq("t")), Some(Action::CullPick));
        assert_eq!(dst.get(Mode::Cull, &seq("p")), None);
        // The viewer keeps its own changes.
        assert_eq!(dst.get(Mode::Viewer, &seq("h")), None);
        assert_eq!(dst.bindings(Mode::Cull), src.bindings(Mode::Cull));
        // Rejected: nothing changes.
        let before = dst.clone();
        assert!(dst.import("replace = [").is_err());
        assert!(dst.same_bindings(&before));
    }

    #[test]
    fn edits() {
        let mut km = Keymap::viewer_default();
        // Swap: re-record next-image's Space as `f`; fit gets Space.
        assert!(km.apply_edit(&KeymapEdit::Bind {
            mode: Mode::Viewer,
            keys: seq("f"),
            action: Action::NextImage,
            old: Some(seq("Space")),
            displace: Displace::Swap,
        }));
        assert_eq!(km.get(Mode::Viewer, &seq("f")), Some(Action::NextImage));
        assert_eq!(km.get(Mode::Viewer, &seq("Space")), Some(Action::ZoomFit));
        // In place: the swapped key keeps its position.
        assert_eq!(
            km.keys_for(Action::NextImage),
            vec![seq("Right"), seq("f"), seq("PageDown")]
        );
        assert_eq!(km.primary_key(Action::ZoomFit), Some(seq("Space")));
        // Replace removes prefix clashes too.
        km.apply_edit(&KeymapEdit::Bind {
            mode: Mode::Viewer,
            keys: seq("d d"),
            action: Action::MoveToTrash,
            old: None,
            displace: Displace::Replace,
        });
        assert_eq!(km.get(Mode::Viewer, &seq("d")), None);
        assert_eq!(km.get(Mode::Viewer, &seq("d d")), Some(Action::MoveToTrash));
        // Reset one action: its keys come back unless another action took them.
        km.apply_edit(&KeymapEdit::Bind {
            mode: Mode::Viewer,
            keys: seq("Right"),
            action: Action::LastImage,
            old: None,
            displace: Displace::Replace,
        });
        km.apply_edit(&KeymapEdit::ResetAction {
            mode: Mode::Viewer,
            action: Action::NextImage,
        });
        // Right went to last image and Space to fit: only Page Down comes back.
        assert_eq!(km.keys_for(Action::NextImage), vec![seq("PageDown")]);
        assert_eq!(km.get(Mode::Viewer, &seq("f")), None);
        assert!(km.action_changed(Mode::Viewer, Action::NextImage));
        assert!(!km.apply_edit(&KeymapEdit::ResetMode(Mode::Cull)));
        assert!(km.apply_edit(&KeymapEdit::ResetMode(Mode::Viewer)));
        assert!(!km.changed());
        assert!(km.apply_edit(&KeymapEdit::Preset(Preset::Vim)));
        assert!(!km.changed());
        assert_eq!(km.get(Mode::Viewer, &seq("h")), Some(Action::PanLeft));
        km.apply_edit(&KeymapEdit::Unbind {
            mode: Mode::Viewer,
            keys: seq("h"),
        });
        assert!(km.apply_edit(&KeymapEdit::ResetAll));
        assert_eq!(km.preset(), Preset::Vim);
        assert!(!km.changed());
    }

    /// Medical mode is a mode over the viewer: its keys win while it is active, the viewer's
    /// other keys keep working, and nothing changes in the viewer.
    #[test]
    fn medical_mode() {
        let mut km = Keymap::viewer_default();
        let key = |s: &str| KeyCombo::parse(s).unwrap();
        let viewer = |km: &Keymap| {
            assert_eq!(km.lookup(&key("1")), Some(Action::ZoomTo(100.0)));
            assert_eq!(km.lookup(&key("w")), None);
            assert_eq!(km.lookup(&key("Ctrl+i")), None);
            assert_eq!(km.lookup(&key(".")), Some(Action::NextFrame));
            assert!(km.keys_for(Action::WindowLevelTool).is_empty());
            assert_eq!(km.primary_key(Action::ZoomTo(100.0)), Some(seq("1")));
        };
        viewer(&km);
        km.set_active(&[Mode::Medical]);
        assert_eq!(km.active(), &[Mode::Medical, Mode::Viewer]);
        assert_eq!(km.lookup(&key("w")), Some(Action::WindowLevelTool));
        assert_eq!(km.lookup(&key("Ctrl+i")), Some(Action::InvertGrey));
        assert_eq!(km.lookup(&key("Ctrl+Shift+i")), None);
        assert_eq!(
            km.lookup(&key("Shift+i")),
            Some(Action::TogglePixelInspector)
        );
        for n in 0..=9u8 {
            let c = key(&n.to_string());
            assert_eq!(km.lookup(&c), Some(Action::WindowPreset(n)));
            assert_eq!(km.keys_for(Action::WindowPreset(n)), vec![KeySeq::one(c)]);
        }
        assert_eq!(km.lookup(&key(".")), Some(Action::NextSlice));
        assert_eq!(km.lookup(&key(",")), Some(Action::PrevSlice));
        // The viewer's other keys still work; the ones the mode takes over are not shown.
        assert_eq!(km.lookup(&key("Right")), Some(Action::NextImage));
        assert_eq!(km.lookup(&key("Ctrl+0")), Some(Action::ZoomFit));
        assert_eq!(km.lookup(&key("Shift+1")), Some(Action::ChannelRed));
        assert_eq!(km.keys_for(Action::ZoomTo(100.0)), vec![seq("Ctrl+1")]);
        assert!(km.keys_for(Action::NextFrame).is_empty());
        assert_eq!(km.primary_key(Action::InvertGrey), Some(seq("Ctrl+i")));
        // Digits follow the key position here too (AZERTY "&" is 1, Shift+1 stays a channel).
        let press = |km: &Keymap, c: char, shift: bool, digit: Option<char>| {
            let mods = if shift { Mods::SHIFT } else { Mods::NONE };
            km.lookup(&km.resolve(KeyCombo::new(Key::Char(c), mods), shift, digit))
        };
        assert_eq!(
            press(&km, '&', false, Some('1')),
            Some(Action::WindowPreset(1))
        );
        assert_eq!(
            press(&km, 'à', false, Some('0')),
            Some(Action::WindowPreset(0))
        );
        assert_eq!(press(&km, '!', true, Some('1')), Some(Action::ChannelRed));
        // The step machine sees the same thing.
        assert_eq!(
            km.step(km.active(), None, key("w")).run,
            vec![Action::WindowLevelTool]
        );
        km.set_active(&[]);
        assert_eq!(km.active(), &[Mode::Viewer]);
        viewer(&km);
        assert_eq!(
            press(&km, '&', false, Some('1')),
            Some(Action::ZoomTo(100.0))
        );
        // Cull over medical: both, the innermost first.
        km.set_active(&[Mode::Cull, Mode::Medical]);
        assert_eq!(km.active(), &[Mode::Cull, Mode::Medical, Mode::Viewer]);
        assert_eq!(km.lookup(&key("1")), Some(Action::CullRate(1)));
        assert_eq!(km.lookup(&key("w")), Some(Action::WindowLevelTool));
    }

    #[test]
    fn medical_toml() {
        let mut km = Keymap::viewer_default();
        let errs = km.apply_toml(
            "[viewer]\n\"w\" = \"zoom-fit\"\n[medical]\n\"l\" = \"window-preset:1\"\n\"1\" = \"none\"\n\"k\" = \"window-preset:12\"\n",
        );
        assert_eq!(errs.len(), 1, "{errs:?}");
        let key = |s: &str| KeyCombo::parse(s).unwrap();
        assert_eq!(km.lookup(&key("w")), Some(Action::ZoomFit));
        km.set_active(&[Mode::Medical]);
        assert_eq!(km.lookup(&key("w")), Some(Action::WindowLevelTool));
        assert_eq!(km.lookup(&key("l")), Some(Action::WindowPreset(1)));
        // Unbound in the mode: the viewer's binding shows through.
        assert_eq!(km.lookup(&key("1")), Some(Action::ZoomTo(100.0)));
        assert!(km.mode_changed(Mode::Medical));
        assert!(!km.mode_changed(Mode::Cull));
        let text = km.to_toml();
        assert!(text.contains("[medical]"), "{text}");
        let (back, errs) = Keymap::from_toml(&text);
        assert!(errs.is_empty(), "{errs:?}");
        assert!(back.same_bindings(&km));
        for a in [
            Action::WindowPreset(7),
            Action::WindowLevelTool,
            Action::InvertGrey,
            Action::NextSlice,
            Action::PrevSlice,
        ] {
            assert_eq!(Action::from_id(&a.id()), Some(a));
        }
        assert!(km.apply_toml("medical = 3\n").len() == 1);
    }

    /// Channels 10 to 16 of a composite are Alt+0 to Alt+6 in science mode (the same keys
    /// stay the cull layer's own there), and the catalogue actions are Shift+o and Alt+o.
    #[test]
    fn science_channels_past_nine() {
        let km = Keymap::viewer_default();
        let seq = |s: &str| KeySeq::one(KeyCombo::parse(s).unwrap());
        for n in 0..=6u8 {
            let k = seq(&format!("Alt+{n}"));
            assert_eq!(
                km.lookup_in(&[Mode::Science, Mode::Viewer], &k),
                Some(Action::ScienceChannel(10 + n))
            );
            assert_eq!(km.get(Mode::Viewer, &k), None);
        }
        assert_eq!(
            km.lookup_in(&[Mode::Science, Mode::Viewer], &seq("Alt+7")),
            km.get(Mode::Viewer, &seq("Alt+7"))
        );
        let sci = |s: &str| km.lookup_in(&[Mode::Science, Mode::Viewer], &seq(s));
        assert_eq!(sci("Shift+o"), Some(Action::ScienceCatalogue));
        assert_eq!(sci("Alt+o"), Some(Action::ScienceClearCatalogues));
        for n in 1..=16 {
            let a = Action::ScienceChannel(n);
            assert_eq!(Action::from_id(&a.id()), Some(a));
            assert!(Action::BINDABLE.contains(&a));
        }
    }

    /// Cull mode's file keys: the slots, bursts and following new files are bindings of
    /// the cull layer, over the viewer's `s` (slideshow) and `t` (DICOM tags).
    #[test]
    fn cull_mode_keys() {
        let mut km = Keymap::viewer_default();
        let key = |s: &str| KeyCombo::parse(s).unwrap();
        assert_eq!(km.lookup(&key("s")), Some(Action::Slideshow));
        assert_eq!(km.lookup(&key("t")), Some(Action::DicomTags));
        assert_eq!(km.lookup(&key("Alt+1")), None);
        km.set_active(&[Mode::Cull]);
        assert_eq!(km.lookup(&key("s")), Some(Action::CullBurst));
        assert_eq!(km.lookup(&key("b")), Some(Action::CullKeepBest));
        assert_eq!(km.lookup(&key("t")), Some(Action::CullFollow));
        for n in 1..=3u8 {
            assert_eq!(
                km.lookup(&key(&format!("Alt+{n}"))),
                Some(Action::CullCopyTo(n))
            );
            assert_eq!(
                km.lookup(&key(&format!("Alt+Shift+{n}"))),
                Some(Action::CullMoveTo(n))
            );
        }
        assert!(km.keys_for(Action::Slideshow).is_empty());
        assert_eq!(km.primary_key(Action::CullFollow), Some(seq("t")));
    }

    /// Problems carry their place in the file, for an editor or linter.
    #[test]
    fn problems_have_spans() {
        let text = "[viewer]\n\"x\" = \"trash\"\n\"Hyper+y\" = \"next\"\n\"y\" = \"bogus\"\n\n[nope]\n\"a\" = \"next\"\n";
        let ps = Keymap::check(text);
        assert_eq!(ps.len(), 3, "{ps:?}");
        let at = |i: usize| &text[ps[i].span.clone().unwrap()];
        assert!(matches!(ps[0].error, KeymapError::UnknownKey(_)));
        assert_eq!(at(0), "\"Hyper+y\"");
        assert_eq!(ps[0].line_col(text), Some((3, 1)));
        assert!(matches!(ps[1].error, KeymapError::UnknownAction { .. }));
        assert_eq!(at(1), "\"bogus\"");
        assert_eq!(ps[1].line_col(text), Some((4, 7)));
        assert!(matches!(ps[2].error, KeymapError::UnknownMode(_)));
        assert_eq!(at(2), "nope");
        let bad = Keymap::check("[viewer\n");
        assert!(matches!(bad[0].error, KeymapError::Syntax(_)));
        assert_eq!(bad[0].line_col("[viewer\n").map(|p| p.0), Some(1));
        assert!(Keymap::check(&Keymap::viewer_default().to_toml()).is_empty());
    }

    /// Keys the desktop keeps are reported where they are bound, as a warning like a prefix
    /// clash; a key bound twice is an error.
    #[test]
    fn page_keys() {
        let km = Keymap::viewer_default();
        let pages = [
            ("Alt+PageDown", Action::NextPage),
            ("Alt+PageUp", Action::PrevPage),
            ("Alt+Home", Action::FirstPage),
            ("Alt+End", Action::LastPage),
        ];
        let mut text = String::from("[viewer]\n");
        for (k, a) in pages {
            assert_eq!(km.get(Mode::Viewer, &seq(k)), Some(a), "{k}");
            // Medical and cull mode fall through to them (medical steps its frames).
            assert_eq!(km.get(Mode::Medical, &seq(k)), None, "{k}");
            assert_eq!(km.get(Mode::Cull, &seq(k)), None, "{k}");
            text.push_str(&format!("\"{k}\" = \"{}\"\n", a.id()));
        }
        // The image and series keys keep theirs.
        assert_eq!(
            km.get(Mode::Viewer, &seq("PageDown")),
            Some(Action::NextImage)
        );
        assert_eq!(
            km.get(Mode::Viewer, &seq("Ctrl+PageUp")),
            Some(Action::PrevSeries)
        );
        // No desktop reserves them, and nothing else is bound to them.
        assert_eq!(Keymap::check(&text), vec![]);
    }

    #[test]
    fn reserved_and_prefix_are_warnings() {
        let text = "[viewer]\n\"Alt+F4\" = \"next\"\n\"g\" = \"prev\"\n\"g g\" = \"first\"\n";
        let ps = Keymap::check(text);
        // Alt+F4 closes windows everywhere but macOS.
        if !cfg!(target_os = "macos") {
            let reserved = ps
                .iter()
                .find(|p| matches!(p.error, KeymapError::Reserved { .. }))
                .expect("reserved");
            assert!(reserved.error.is_warning());
            assert_eq!(reserved.line_col(text), Some((2, 1)));
        }
        let prefix = ps
            .iter()
            .find(|p| matches!(p.error, KeymapError::Prefix { .. }))
            .expect("prefix");
        assert!(prefix.error.is_warning());
        assert_eq!(prefix.line_col(text).map(|p| p.0), Some(3));
        let dup = "[viewer]\n\"Ctrl+q\" = \"next\"\n\"ctrl+q\" = \"prev\"\n";
        let ps = Keymap::check(dup);
        assert!(
            ps.iter()
                .any(|p| matches!(p.error, KeymapError::Duplicate { .. }) && !p.error.is_warning())
        );
    }
}
