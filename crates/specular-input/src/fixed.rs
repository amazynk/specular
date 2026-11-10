//! Keys the gallery and compare handle themselves, outside the keymap: one table per view
//! that both the key handlers (through [`find`]) and the cheat sheet read, so what the sheet
//! lists is what the keys do.

use crate::{Key, KeyCombo, Mods};

/// A command of the gallery grid's own keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GalleryCmd {
    /// The arrows (the key says which way; Shift extends the selection, Ctrl moves the
    /// focus only).
    Move,
    /// Page Up / Page Down.
    Page,
    /// Home / End.
    Ends,
    /// Up to the parent folder.
    Up,
    /// The folder shown before this one, and forward again (as a browser's history).
    HistoryBack,
    HistoryForward,
    /// Esc: close quick look or compare, the filter, the selection, then up a folder.
    Back,
    Open,
    NewWindow,
    Properties,
    /// Back to the viewer's image.
    Viewer,
    NextUnrated,
    /// Select or deselect the focused item.
    Select,
    SelectAll,
    SelectNone,
    InvertSelection,
    /// 1–5 stars (the digit says how many).
    Rate,
    RateNone,
    /// Colour label 1–4 on 6–9 (again: off).
    Label,
    Pick,
    Reject,
    Unflag,
    /// The selection into the target album (the last one used).
    AlbumAdd,
    /// The selection into an album, asked which.
    AlbumChoose,
    /// Quick look while the key is held.
    QuickLook,
    Compare,
    Stack,
    Filter,
    Bigger,
    Smaller,
    DefaultSize,
    Trash,
    CopyImage,
    CopyPaths,
    Refresh,
    ContextMenu,
}

/// A command of compare's own keys (before the gallery's while compare is open).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompareCmd {
    /// Overlay, or back to side by side.
    Overlay,
    /// The difference view, then its next kind, then side by side.
    Difference,
    Flicker,
    /// Flicker by itself, or stop.
    AutoFlicker,
    /// The split view, or back to side by side.
    Split,
    /// The split's divider across or down.
    SplitDirection,
    /// The next alignment of images of different sizes.
    Align,
    /// The other metric space.
    MetricSpace,
    /// The whole image's metrics, or what is in view.
    WholeImage,
    /// The pixel probe on or off.
    Probe,
    /// Side by side: zoom and pan each image on its own, or together.
    Link,
    /// `[` / `]`: the overlay's blend, the split's divider or the difference's gain.
    StepDown,
    StepUp,
    /// ← / →: the overlay's top image, else the image the keys act on.
    Pick,
    ZoomIn,
    ZoomOut,
    Fit,
    Actual,
    /// `c`, and Esc (one layer at a time).
    Close,
}

impl CompareCmd {
    /// The action of the registry that does the same (palette, remote control), if any.
    pub fn action(self) -> Option<crate::Action> {
        use crate::Action as A;
        Some(match self {
            CompareCmd::Overlay => A::CompareOverlay,
            CompareCmd::Difference => A::CompareDifference,
            CompareCmd::Flicker => A::CompareFlicker,
            CompareCmd::AutoFlicker => A::CompareAutoFlicker,
            CompareCmd::Split => A::CompareSplit,
            CompareCmd::SplitDirection => A::CompareSplitDirection,
            CompareCmd::Align => A::CompareAlign,
            CompareCmd::MetricSpace => A::CompareMetricSpace,
            CompareCmd::WholeImage => A::CompareWholeImage,
            CompareCmd::Probe => A::CompareProbe,
            CompareCmd::Link => A::CompareLink,
            CompareCmd::Close => A::CompareClose,
            CompareCmd::StepDown
            | CompareCmd::StepUp
            | CompareCmd::Pick
            | CompareCmd::ZoomIn
            | CompareCmd::ZoomOut
            | CompareCmd::Fit
            | CompareCmd::Actual => return None,
        })
    }
}

/// One line of a view's keys.
#[derive(Clone, Copy, Debug)]
pub struct FixedKey<C: 'static> {
    pub cmd: C,
    /// The combinations in `keymap.toml` spelling ([`KeyCombo::parse`]).
    pub keys: &'static [&'static str],
    /// How many of `keys` the cheat sheet shows (the rest are alternatives).
    pub listed: u8,
    /// Modifiers ignored when matching (passed on to the handler).
    pub free: Mods,
    /// Whether a held key repeats the command.
    pub repeats: bool,
    /// Works while held (listed as "Hold Q").
    pub hold: bool,
    /// Fluent message ID of the line.
    pub label: &'static str,
    /// Fluent message ID of the cheat sheet heading the line is under.
    pub group: &'static str,
}

const NONE: Mods = Mods::NONE;
const SHIFT: Mods = Mods::SHIFT;
const SHIFT_CTRL: Mods = Mods::CTRL_SHIFT;
const ANY: Mods = Mods {
    ctrl: true,
    shift: true,
    alt: true,
    meta: true,
};

const fn line<C: Copy>(
    cmd: C,
    keys: &'static [&'static str],
    label: &'static str,
    group: &'static str,
) -> FixedKey<C> {
    FixedKey {
        cmd,
        keys,
        listed: 2,
        free: NONE,
        repeats: false,
        hold: false,
        label,
        group,
    }
}

impl<C: Copy> FixedKey<C> {
    const fn listed(mut self, n: u8) -> Self {
        self.listed = n;
        self
    }
    const fn free(mut self, m: Mods) -> Self {
        self.free = m;
        self
    }
    const fn repeats(mut self) -> Self {
        self.repeats = true;
        self
    }
    const fn hold(mut self) -> Self {
        self.hold = true;
        self
    }

    /// The combinations, parsed.
    pub fn combos(&self) -> impl Iterator<Item = KeyCombo> + '_ {
        self.keys.iter().filter_map(|k| KeyCombo::parse(k))
    }

    /// The combinations the cheat sheet shows.
    pub fn listed_combos(&self) -> impl Iterator<Item = KeyCombo> + '_ {
        self.combos().take(self.listed as usize)
    }
}

const BROWSE: &str = "cheatsheet-gallery-browse";
const SELECT: &str = "cheatsheet-gallery-select";
const MARK: &str = "cheatsheet-gallery-mark";
const VIEW: &str = "cheatsheet-view";
const FILE: &str = "cheatsheet-file";

/// Back and forward through the folders shown: Finder's ⌘[ / ⌘] on macOS (listed first),
/// a file manager's Alt+← / Alt+→ everywhere. The Lightroom preset's Ctrl+[ ] rotate the
/// viewer's image; the grid has nothing to rotate, so here they are free for this.
const HISTORY_BACK: &[&str] = if cfg!(target_os = "macos") {
    &["Ctrl+[", "Alt+Left"]
} else {
    &["Alt+Left"]
};
const HISTORY_FORWARD: &[&str] = if cfg!(target_os = "macos") {
    &["Ctrl+]", "Alt+Right"]
} else {
    &["Alt+Right"]
};

/// The gallery grid's keys, in cheat sheet order.
pub const GALLERY_KEYS: &[FixedKey<GalleryCmd>] = {
    use GalleryCmd as G;
    &[
        line(
            G::Move,
            &["Left", "Right", "Up", "Down"],
            "cheatsheet-gallery-move",
            BROWSE,
        )
        .listed(4)
        .free(SHIFT_CTRL)
        .repeats(),
        line(
            G::Page,
            &["PageUp", "PageDown"],
            "cheatsheet-gallery-page",
            BROWSE,
        )
        .free(SHIFT_CTRL)
        .repeats(),
        line(G::Ends, &["Home", "End"], "cheatsheet-gallery-ends", BROWSE).free(SHIFT_CTRL),
        line(G::Open, &["Enter"], "gallery-open", BROWSE).free(SHIFT),
        line(G::NewWindow, &["Ctrl+Enter"], "gallery-new-window", BROWSE).free(SHIFT),
        line(
            G::Up,
            &["Backspace", "Alt+Up"],
            "cheatsheet-gallery-up",
            BROWSE,
        )
        .free(ANY),
        line(
            G::HistoryBack,
            HISTORY_BACK,
            "cheatsheet-gallery-history-back",
            BROWSE,
        ),
        line(
            G::HistoryForward,
            HISTORY_FORWARD,
            "cheatsheet-gallery-history-forward",
            BROWSE,
        ),
        line(G::Back, &["Escape"], "cheatsheet-gallery-back", BROWSE).free(ANY),
        line(G::Viewer, &["g"], "cheatsheet-gallery-viewer", BROWSE),
        line(G::NextUnrated, &["n"], "gallery-next-unrated", BROWSE),
        line(
            G::Select,
            &["Space"],
            "cheatsheet-gallery-select-one",
            SELECT,
        )
        .free(ANY),
        line(G::SelectAll, &["Ctrl+a"], "gallery-select-all", SELECT),
        line(
            G::SelectNone,
            &["Ctrl+Shift+a"],
            "gallery-select-none",
            SELECT,
        ),
        line(
            G::InvertSelection,
            &["Ctrl+i", "Ctrl+Shift+i"],
            "gallery-select-invert",
            SELECT,
        )
        .listed(1),
        line(G::Pick, &["p"], "gallery-pick", MARK),
        line(G::Reject, &["x"], "gallery-reject", MARK),
        line(G::Unflag, &["u"], "gallery-unflag", MARK),
        line(G::AlbumAdd, &["b"], "cheatsheet-gallery-album-add", MARK),
        line(G::AlbumChoose, &["Shift+b"], "albums-add-menu", MARK),
        line(
            G::Rate,
            &["1", "2", "3", "4", "5"],
            "gallery-facet-rating",
            MARK,
        )
        .listed(5),
        line(G::RateNone, &["0"], "gallery-rate-none", MARK),
        line(G::Label, &["6", "7", "8", "9"], "gallery-facet-label", MARK).listed(4),
        line(G::QuickLook, &["q"], "cheatsheet-gallery-quick-look", VIEW).hold(),
        line(G::Compare, &["c"], "gallery-palette-compare", VIEW),
        line(G::Stack, &["s"], "gallery-stack-toggle", VIEW),
        line(
            G::Filter,
            &["/", "Ctrl+f", "Ctrl+Shift+f"],
            "cheatsheet-gallery-filter",
            VIEW,
        ),
        line(
            G::Bigger,
            &["+", "Ctrl+Plus", "=", "Ctrl+="],
            "cheatsheet-gallery-bigger",
            VIEW,
        )
        .repeats(),
        line(
            G::Smaller,
            &["-", "Ctrl+-"],
            "cheatsheet-gallery-smaller",
            VIEW,
        )
        .repeats(),
        line(
            G::DefaultSize,
            &["Ctrl+0"],
            "cheatsheet-gallery-default-size",
            VIEW,
        ),
        line(G::Properties, &["Alt+Enter"], "gallery-properties", FILE).free(SHIFT_CTRL),
        line(G::Trash, &["Delete"], "palette-move-to-trash", FILE).free(ANY),
        line(G::CopyImage, &["Ctrl+c"], "palette-copy-image", FILE),
        line(G::CopyPaths, &["Ctrl+Shift+c"], "gallery-copy-paths", FILE),
        line(G::Refresh, &["F5"], "cheatsheet-gallery-refresh", FILE).free(ANY),
        line(
            G::ContextMenu,
            &["Shift+F10"],
            "cheatsheet-gallery-menu",
            FILE,
        ),
    ]
};

const COMPARE: &str = "cheatsheet-compare-views";
const ZOOM: &str = "cheatsheet-zoom-pan";

/// Compare's keys, in cheat sheet order.
pub const COMPARE_KEYS: &[FixedKey<CompareCmd>] = {
    use CompareCmd as C;
    &[
        line(C::Overlay, &["o"], "cheatsheet-compare-overlay", COMPARE),
        line(
            C::Difference,
            &["d"],
            "cheatsheet-compare-difference",
            COMPARE,
        ),
        line(C::Split, &["s"], "cheatsheet-compare-split", COMPARE),
        line(
            C::SplitDirection,
            &["h"],
            "cheatsheet-compare-split-direction",
            COMPARE,
        ),
        line(C::Flicker, &["t"], "compare-flicker", COMPARE),
        line(
            C::AutoFlicker,
            &["Shift+t"],
            "cheatsheet-compare-auto-flicker",
            COMPARE,
        ),
        line(C::StepDown, &["["], "cheatsheet-compare-step-down", COMPARE).repeats(),
        line(C::StepUp, &["]"], "cheatsheet-compare-step-up", COMPARE).repeats(),
        line(
            C::Pick,
            &["Left", "Right"],
            "cheatsheet-compare-pick",
            COMPARE,
        )
        .free(SHIFT_CTRL)
        .repeats(),
        line(C::Align, &["a"], "cheatsheet-compare-align", COMPARE),
        line(C::MetricSpace, &["m"], "cheatsheet-compare-space", COMPARE),
        line(
            C::WholeImage,
            &["Shift+m"],
            "cheatsheet-compare-whole",
            COMPARE,
        ),
        line(C::Probe, &["p"], "cheatsheet-compare-probe", COMPARE),
        line(C::Link, &["l"], "cheatsheet-compare-link", COMPARE),
        line(
            C::Close,
            &["c", "Escape"],
            "cheatsheet-compare-close",
            COMPARE,
        ),
        line(
            C::ZoomIn,
            &["+", "Ctrl+Plus", "=", "Ctrl+="],
            "palette-zoom-in",
            ZOOM,
        )
        .repeats(),
        line(C::ZoomOut, &["-", "Ctrl+-"], "palette-zoom-out", ZOOM).repeats(),
        line(C::Fit, &["f", "Ctrl+0"], "palette-zoom-fit", ZOOM),
        line(C::Actual, &["Ctrl+1"], "palette-zoom-100", ZOOM),
    ]
};

/// Whether `spec` (a table's combination) matches the pressed `input`, ignoring `free`.
/// Shift is ignored on keys that are not letters: it is how some layouts type `+`, `/` or a
/// digit, and the handlers never told those apart.
fn matches(spec: KeyCombo, input: &KeyCombo, free: Mods) -> bool {
    if spec.key != input.key {
        return false;
    }
    let shift_free = free.shift || matches!(spec.key, Key::Char(c) if !c.is_alphabetic());
    let (a, b) = (spec.mods, input.mods);
    (free.ctrl || a.ctrl == b.ctrl)
        && (free.alt || a.alt == b.alt)
        && (free.meta || a.meta == b.meta)
        && (shift_free || a.shift == b.shift)
}

/// The line of `table` a key press runs, if any (the first that matches).
pub fn find<C: Copy>(
    table: &'static [FixedKey<C>],
    input: &KeyCombo,
) -> Option<&'static FixedKey<C>> {
    table
        .iter()
        .find(|l| l.combos().any(|s| matches(s, input, l.free)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Every key of a table parses and runs its own line (no line hides another's key),
    /// and every command has a line: the cheat sheet lists exactly what the handlers do.
    fn check<C: Copy + Eq + std::hash::Hash + std::fmt::Debug>(table: &'static [FixedKey<C>]) {
        let mut cmds = HashSet::new();
        for l in table {
            assert!(cmds.insert(l.cmd), "{:?} listed twice", l.cmd);
            assert!(!l.keys.is_empty() && l.listed >= 1);
            for k in l.keys {
                let c = KeyCombo::parse(k).unwrap_or_else(|| panic!("bad key {k}"));
                let got = find(table, &c).map(|l| l.cmd);
                assert_eq!(got, Some(l.cmd), "{k} runs {got:?}, not {:?}", l.cmd);
            }
        }
    }

    /// Every command has a line: a new command fails to compile here until it is numbered,
    /// and then this fails until the table lists it.
    #[test]
    fn every_listed_key_runs_its_own_line() {
        use CompareCmd as C;
        use GalleryCmd as G;
        check(GALLERY_KEYS);
        check(COMPARE_KEYS);
        let g = |g: G| match g {
            G::Move => 0,
            G::Page => 1,
            G::Ends => 2,
            G::Up => 3,
            G::Back => 4,
            G::Open => 5,
            G::NewWindow => 6,
            G::Properties => 7,
            G::Viewer => 8,
            G::NextUnrated => 9,
            G::Select => 10,
            G::SelectAll => 11,
            G::SelectNone => 12,
            G::InvertSelection => 13,
            G::Rate => 14,
            G::RateNone => 15,
            G::Label => 16,
            G::Pick => 17,
            G::Reject => 18,
            G::Unflag => 19,
            G::QuickLook => 20,
            G::Compare => 21,
            G::Stack => 22,
            G::Filter => 23,
            G::Bigger => 24,
            G::Smaller => 25,
            G::DefaultSize => 26,
            G::Trash => 27,
            G::CopyImage => 28,
            G::CopyPaths => 29,
            G::Refresh => 30,
            G::ContextMenu => 31,
            G::HistoryBack => 32,
            G::HistoryForward => 33,
            G::AlbumAdd => 34,
            G::AlbumChoose => 35,
        };
        let c = |c: C| match c {
            C::Overlay => 0,
            C::Difference => 1,
            C::Flicker => 2,
            C::StepDown => 3,
            C::StepUp => 4,
            C::Pick => 5,
            C::ZoomIn => 6,
            C::ZoomOut => 7,
            C::Fit => 8,
            C::Actual => 9,
            C::Close => 10,
            C::AutoFlicker => 11,
            C::Split => 12,
            C::SplitDirection => 13,
            C::Align => 14,
            C::MetricSpace => 15,
            C::WholeImage => 16,
            C::Probe => 17,
            C::Link => 18,
        };
        let mut gi: Vec<usize> = GALLERY_KEYS.iter().map(|l| g(l.cmd)).collect();
        gi.sort_unstable();
        assert_eq!(gi, (0..36).collect::<Vec<_>>());
        let mut ci: Vec<usize> = COMPARE_KEYS.iter().map(|l| c(l.cmd)).collect();
        ci.sort_unstable();
        assert_eq!(ci, (0..19).collect::<Vec<_>>());
    }

    #[test]
    fn modifiers_match_as_the_handlers_expect() {
        let k = |s: &str| KeyCombo::parse(s).unwrap();
        let g = |s: &str| find(GALLERY_KEYS, &k(s)).map(|l| l.cmd);
        let c = |s: &str| find(COMPARE_KEYS, &k(s)).map(|l| l.cmd);
        use GalleryCmd as G;
        // Shift and Ctrl ride along with the arrows; Alt+Up is the parent folder.
        assert_eq!(g("Shift+Left"), Some(G::Move));
        assert_eq!(g("Ctrl+Shift+Down"), Some(G::Move));
        assert_eq!(g("Alt+Up"), Some(G::Up));
        // B puts the selection in the target album, Shift+B asks which.
        assert_eq!(g("b"), Some(G::AlbumAdd));
        assert_eq!(g("Shift+b"), Some(G::AlbumChoose));
        // Alt+← / → go back and forward through the folders shown, not along the grid.
        assert_eq!(g("Alt+Left"), Some(G::HistoryBack));
        assert_eq!(g("Alt+Right"), Some(G::HistoryForward));
        assert_eq!(g("Ctrl+Backspace"), Some(G::Up));
        assert_eq!(g("Alt+Enter"), Some(G::Properties));
        assert_eq!(g("Ctrl+Enter"), Some(G::NewWindow));
        assert_eq!(g("Enter"), Some(G::Open));
        // Letters keep Shift apart; symbols and digits don't (US Ctrl++ is Ctrl+Shift+=).
        assert_eq!(g("Ctrl+a"), Some(G::SelectAll));
        assert_eq!(g("Ctrl+Shift+a"), Some(G::SelectNone));
        assert_eq!(g("Ctrl+Shift+Plus"), Some(G::Bigger));
        assert_eq!(g("P"), None);
        assert_eq!(g("Alt+p"), None);
        assert_eq!(g("Shift+3"), Some(G::Rate));
        assert_eq!(c("Ctrl+Shift+="), Some(CompareCmd::ZoomIn));
        assert_eq!(c("Shift+Escape"), None);
        assert_eq!(c("Up"), None);
    }
}
