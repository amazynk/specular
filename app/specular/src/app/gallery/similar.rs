//! Duplicates and similar shots (DESIGN §10): two gallery categories that group copies of one
//! image and near-identical shots, suggest which members to keep (each with a reason), and
//! let the user review them group by group: keepers large, the rest in a strip with keep /
//! discard badges, a synchronised compare of 2–4 members, and a bulk "accept suggestions".
//!
//! The matching itself lives in `specular-similar`; this module runs its scan on a thread of
//! its own (a small thumbnail engine sharing the gallery's registry, pool and disk cache),
//! holds the results, regroups them live (the sensitivity slider, the user's marks, splits),
//! and carries out discards the way the user chose: to the trash (undoable, the default),
//! into a folder, or only flagged as rejected. Nothing is removed without a confirmation.

use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use specular_core::work::{Kind as WorkKind, Step as WorkStep, Work};
use specular_gfx::{Align, DrawList, Text, TextStyle};
use specular_similar::scan::{
    self, FocusInput, FocusWish, Measured, Phase, Results, ScanSetup, Unit,
};
use specular_similar::{
    Category, Decision, DiscardAction, Group, Pick, Protect, Reason, Store as SimStore, analyse,
};
use specular_theme::{Color, Palette};

use specular_widgets::Button;

use super::atlas::Placed;
use super::draw::Frame;
use super::*;

/// Folders with more images than this are scanned when a category is opened, not on their own.
const AUTO_SCAN_MAX: usize = 5000;
/// Quiet time after the folder changed before scanning on its own.
const AUTO_SCAN_DELAY: Duration = Duration::from_millis(1500);
/// After the delay, while the thumbnail engine is still busy: how often to look again.
const AUTO_SCAN_RECHECK: Duration = Duration::from_millis(250);
/// Group list rows and the review strip, logical px at the standard text size.
const ROW_H: f32 = 116.0;
/// The part of a row's height its three lines of text take (grows with the text).
const ROW_TEXT: f32 = 60.0;
const ROW_THUMB: f32 = 92.0;
const STRIP_THUMB: f32 = 108.0;
/// Under each strip thumbnail: its reason and state lines.
const STRIP_TEXT: f32 = 46.0;
const HEADER_H: f32 = 64.0;
/// A footer of one row ([`footer_block`] lays footers out).
#[cfg(test)]
const FOOTER_H: f32 = 56.0;
/// The review's group line under the header.
const GROUP_LINE: f32 = 36.0;
/// A large image's caption line in the review.
const CAPTION_H: f32 = 22.0;

/// The screens' metrics at a text scale: bars, buttons and the rows' text grow with larger
/// text (never shrinking below the standard size, as the gallery's `Gallery::u`); the
/// thumbnails keep their size.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Metrics {
    grow: f32,
}

impl Metrics {
    fn new(text_scale: f32) -> Metrics {
        Metrics {
            grow: if text_scale.is_finite() {
                text_scale.clamp(1.0, 3.0)
            } else {
                1.0
            },
        }
    }

    /// `px` at the text scale, on whole logical px.
    fn u(self, px: f32) -> f32 {
        (px * self.grow).round()
    }

    fn header(self) -> f32 {
        self.u(HEADER_H)
    }

    #[cfg(test)]
    fn footer(self) -> f32 {
        self.u(FOOTER_H)
    }

    /// A group row: the thumbnails' height, and the text's grown.
    fn row(self) -> f32 {
        ROW_H - ROW_TEXT + self.u(ROW_TEXT)
    }

    /// The review's strip: the thumbnails and their two lines.
    fn strip(self) -> f32 {
        STRIP_THUMB + 10.0 + self.u(STRIP_TEXT)
    }

    /// A strip cell: the thumbnail with room for its two lines, which grow with the text.
    fn strip_cell(self) -> f32 {
        self.u(STRIP_THUMB + 40.0)
    }
}

/// The review strip's thumbnails in a compact review (no lines under them).
const STRIP_THUMB_COMPACT: f32 = 64.0;
/// Between the footers' buttons, and between their rows (at text scale 1).
const FOOTER_GAP: f32 = 8.0;
const FOOTER_ROW_GAP: f32 = 6.0;
/// The review is compact (smaller strip without its lines, tighter footer; then without the
/// large images' captions too) when its large images would get less than this share of the
/// window's height.
const REVIEW_MIN_SHARE: f32 = 0.25;

/// The review footer's buttons in rows, laid out left to right (drawn through `flip`).
#[derive(Clone, Debug, PartialEq)]
struct FooterRows {
    /// The labels carry their keys ("Keep  (P)"); else (when they do not fit on one row)
    /// the keys show in a tooltip.
    keys: bool,
    /// Each row's buttons: (index, x, width).
    rows: Vec<Vec<(usize, f32, f32)>>,
}

/// The review footer's buttons: label, key, primary, action.
fn review_buttons() -> [(String, &'static str, bool, Act); 8] {
    [
        (t!("similar-keep"), false, Act::Set(Decision::Keep)),
        (t!("similar-discard"), false, Act::Set(Decision::Discard)),
        (
            t!("similar-undecided"),
            false,
            Act::Set(Decision::Undecided),
        ),
        (t!("similar-accept-next"), true, Act::AcceptNext),
        (t!("similar-not-similar"), false, Act::Split),
        (t!("similar-skip"), false, Act::Skip),
        (t!("similar-compare"), false, Act::Compare),
        (t!("similar-zoom"), false, Act::Zoom),
    ]
    .map(|(l, primary, a)| (l, act_key(a, true).unwrap_or_default(), primary, a))
}

/// The review footer's buttons of (with key, without) widths in a view `w` wide: on one row
/// with their keys if they fit, else without; wrapped onto more rows when even that is too
/// wide (every button stays on screen, one wider than the window cut to fit).
fn footer_rows(widths: &[(f32, f32)], w: f32) -> FooterRows {
    let avail = (w - 32.0).max(1.0);
    let n = widths.len();
    let gaps = FOOTER_GAP * n.saturating_sub(1) as f32;
    let keys = widths.iter().map(|p| p.0).sum::<f32>() + gaps <= avail;
    let mut rows: Vec<Vec<(usize, f32, f32)>> = vec![Vec::new()];
    let mut x = 16.0;
    for (i, &(full, short)) in widths.iter().enumerate() {
        let bw = if keys { full } else { short }.min(avail);
        if x > 16.0 && x + bw > w - 16.0 {
            rows.push(Vec::new());
            x = 16.0;
        }
        rows.last_mut().expect("a row").push((i, x, bw));
        x += bw + FOOTER_GAP;
    }
    FooterRows { keys, rows }
}

/// A footer of `rows` rows of buttons at metrics `m`: its height and the padding above its
/// first row and below its last (tighter when `compact`). One row, not compact: `m.footer()`.
fn footer_block(rows: usize, m: Metrics, compact: bool) -> (f32, f32) {
    let rows = rows.max(1) as f32;
    let pad = m.u(if compact { 6.0 } else { 12.0 });
    (
        2.0 * pad + rows * m.u(32.0) + (rows - 1.0) * m.u(FOOTER_ROW_GAP),
        pad,
    )
}

/// The top of a footer button's row `row` in a footer at `fy` padded `pad`.
fn footer_row_y(fy: f32, pad: f32, row: usize, m: Metrics) -> f32 {
    fy + pad + row as f32 * (m.u(32.0) + m.u(FOOTER_ROW_GAP))
}

/// The review's layout in a view of `w` × `h` logical px at metrics `m`, its footer's
/// buttons `widths` wide (see [`footer_rows`]).
#[derive(Clone, Debug, PartialEq)]
struct ReviewLayout {
    footer: FooterRows,
    footer_h: f32,
    /// Above the footer's first row and below its last.
    footer_pad: f32,
    /// The strip: its height, thumbnails' side, cells' width, and whether the reason and
    /// state lines show under the thumbnails.
    strip_h: f32,
    strip_thumb: f32,
    strip_cell: f32,
    strip_text: bool,
    /// The large images' area (between the group line and the strip).
    main: Rect,
    /// Under each large image, its caption line: none in the most compact review.
    caption: f32,
}

fn review_layout(w: f32, h: f32, m: Metrics, widths: &[(f32, f32)]) -> ReviewLayout {
    let footer = footer_rows(widths, w);
    let top = m.header() + m.u(GROUP_LINE);
    let rows = footer.rows.len();
    let build = |compact: bool, caption: bool| {
        let (footer_h, footer_pad) = footer_block(rows, m, compact);
        let (strip_h, strip_thumb, strip_cell) = if compact {
            (
                STRIP_THUMB_COMPACT + 20.0,
                STRIP_THUMB_COMPACT,
                STRIP_THUMB_COMPACT + 16.0,
            )
        } else {
            (m.strip(), STRIP_THUMB, m.strip_cell())
        };
        ReviewLayout {
            footer: footer.clone(),
            footer_h,
            footer_pad,
            strip_h,
            strip_thumb,
            strip_cell,
            strip_text: !compact,
            main: Rect::new(
                16.0,
                top,
                w - 32.0,
                (h - top - strip_h - footer_h - 8.0).max(40.0),
            ),
            caption: if caption { m.u(CAPTION_H) } else { 0.0 },
        }
    };
    let roomy = |l: &ReviewLayout| l.main.h >= h * REVIEW_MIN_SHARE;
    [build(false, true), build(true, true)]
        .into_iter()
        .find(roomy)
        .unwrap_or_else(|| build(true, false))
}

const TITLE: TextStyle = TextStyle::new(18.0).weight(650.0);
const BODY: TextStyle = TextStyle::new(13.0);
const SMALL: TextStyle = TextStyle::new(11.5);

const KEEP_COLOR: u32 = 0x30A46C;
const DISCARD_COLOR: u32 = 0xE5484D;

/// Keep / discard as text ("Suggested: keep") on the panel surfaces: darker in the light theme
/// and lighter in the dark one than the badge fills, so small text passes WCAG AA (4.5:1) on
/// `surface`, `surface_hover` and `selection` (DESIGN.md §3).
fn state_text(pal: &Palette, keep: bool) -> Color {
    match (pal.mode, keep) {
        (specular_theme::Mode::Dark, true) => Color::hex(0x4CC38A),
        (specular_theme::Mode::Dark, false) => pal.danger,
        (specular_theme::Mode::Light, true) => Color::hex(0x18794E),
        (specular_theme::Mode::Light, false) => Color::hex(0xC62A2F),
    }
}

/// Secondary text in a group row: `text_muted`, or `text` on the tinted (focused or hovered)
/// rows, where `text_muted` falls below 4.5:1 in the dark theme.
fn row_secondary(pal: &Palette, tinted: bool) -> Color {
    if tinted { pal.text } else { pal.text_muted }
}

/// Where the keep / discard badge of thumbnail `r` goes (also its click target in the strip):
/// about a quarter of the thumbnail, 24–32 logical px, on whole device pixels at `scale`.
/// In the top trailing corner: top right, top left for right-to-left layouts.
fn badge_rect(r: Rect, scale: f32, rtl: bool) -> Rect {
    let snap = |v: f32| (v * scale).round() / scale;
    let d = snap((r.w.min(r.h) * 0.24).clamp(24.0, 32.0));
    let m = snap((d * 0.25).max(5.0));
    let x = if rtl {
        snap(r.x + m)
    } else {
        snap(r.right() - m - d)
    };
    Rect::new(x, snap(r.y + m), d, d)
}

/// Palette and menu codes (see `super::code`; these use 300–399).
pub(super) mod code {
    pub const DUPES: u64 = 300;
    pub const SIMILAR: u64 = 301;
    pub const ACCEPT_ALL: u64 = 302;
    pub const ACCEPT_ALL_YES: u64 = 303;
    pub const CANCEL: u64 = 304;
    pub const ACTION_MENU: u64 = 305;
    pub const ACTION: u64 = 306; // + 0..3 (trash, move, reject)
    pub const CHOOSE_FOLDER: u64 = 310;
    pub const UNSKIP: u64 = 311;

    pub fn owns(c: u64) -> bool {
        (300..400).contains(&c)
    }
}

/// What a click or key does in the review screens.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Act {
    Close,
    Back,
    Tab(Category),
    Open(usize),
    AcceptAll,
    ActionMenu,
    Unskip,
    Focus(usize),
    Set(Decision),
    Toggle(usize),
    AcceptNext,
    Split,
    Skip,
    Compare,
    Zoom,
    Slider,
    /// The header's Cancel while a scan runs.
    Stop,
    /// The header's "Scan again" once a scan was stopped.
    Rescan,
}

/// The key that does what `a` does (`similar_key`) while `reviewing` or not: in the footers'
/// labels and tooltips, and the controls' shortcuts for screen readers.
fn act_key(a: Act, reviewing: bool) -> Option<&'static str> {
    Some(match a {
        Act::Set(Decision::Keep) => "P",
        Act::Set(Decision::Discard) => "X",
        Act::Set(Decision::Undecided) => "U",
        Act::AcceptNext => "Space",
        Act::Split => "N",
        Act::Skip => "S",
        Act::Compare => "C",
        Act::Zoom => "Z",
        Act::AcceptAll => "A",
        Act::ActionMenu => "D",
        Act::Unskip => "K",
        Act::Back => "Esc",
        Act::Close if !reviewing => "Esc",
        Act::Stop if !reviewing => "Esc",
        Act::Tab(_) => "Tab",
        _ => return None,
    })
}

/// The controls with an AccessKit node of their own (header, slider, footers): node
/// `A11Y_BASE + A11Y_OFFSET + A11Y_CONTROLS + i` is `CONTROLS[i]`.
const CONTROLS: [Act; 18] = [
    Act::Tab(Category::Duplicates),
    Act::Tab(Category::Similar),
    Act::Back,
    Act::Close,
    Act::Slider,
    Act::AcceptAll,
    Act::ActionMenu,
    Act::Unskip,
    Act::Set(Decision::Keep),
    Act::Set(Decision::Discard),
    Act::Set(Decision::Undecided),
    Act::AcceptNext,
    Act::Split,
    Act::Skip,
    Act::Compare,
    Act::Zoom,
    Act::Stop,
    Act::Rescan,
];
/// The footers' controls, in [`CONTROLS`].
const FOOTER_CONTROLS: std::ops::Range<usize> = 5..16;
/// After the options' ids (one per group or member).
const A11Y_CONTROLS: u64 = 1 << 32;
/// Nodes without an action: the tab list, the summary line and the scan's progress.
const A11Y_TABS: u64 = A11Y_CONTROLS + 100;
const A11Y_SUMMARY: u64 = A11Y_CONTROLS + 101;
const A11Y_PROGRESS: u64 = A11Y_CONTROLS + 102;

/// The header's progress of a scan in `phase`: `done` of `total` images.
fn progress_text(phase: Phase, done: usize, total: usize) -> String {
    match phase {
        Phase::Signatures => t!("similar-scanning", done = done as i64, total = total as i64),
        Phase::Focus => t!(
            "similar-checking-sharpness",
            done = done.min(total) as i64,
            total = total as i64
        ),
    }
}

/// `s` without bidi isolation marks, for a node's value (the adapter strips Fluent's from
/// names and descriptions only).
pub(super) fn without_isolates(s: &str) -> String {
    s.replace(['\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}'], "")
}

/// The progress screen readers are told of: `done` down to a quarter of `total`, so a scan
/// is announced at its start and three times on the way, not at every image.
pub(super) fn said_progress(done: usize, total: usize) -> usize {
    let q = total.div_ceil(4).max(1);
    done.min(total) / q * q
}

/// What the scan thread sends: the groups (best-pick from the thumbnails' measures) as soon
/// as they are known, then the focus pass's full-resolution measures as they come, then the
/// end.
enum ScanMsg {
    Groups(Box<Results>),
    Focus(Vec<Measured>),
    /// Finished (false: cancelled).
    Done(bool),
}

/// Least time between two refines of the picks while the focus pass runs.
const REFINE_EVERY: Duration = Duration::from_millis(300);

/// A running scan.
struct Scan {
    cancel: Arc<AtomicBool>,
    done: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
    /// 1 once the scan is in its focus pass ([`Phase::Focus`]).
    phase: Arc<AtomicUsize>,
    rx: Receiver<ScanMsg>,
    key: u64,
    /// The members the focus pass measures first: the reviewed and the visible groups'.
    wish: FocusWish,
}

/// The review of one group.
#[derive(Clone, Debug, Default)]
struct Review {
    /// The group (its key; groups are rebuilt when anything changes).
    group: u64,
    /// Focused member (position in the group).
    focus: usize,
    /// Compare: member positions shown side by side (2–4).
    compare: Option<Vec<usize>>,
    /// Zoom over "fit" (1: fit), shared by every pane, about `at` (normalised image point).
    zoom: f32,
    at: (f32, f32),
}

/// The open category: its group list, and the group under review.
#[derive(Clone, Debug)]
pub(super) struct View {
    cat: Category,
    cursor: usize,
    scroll: f32,
    review: Option<Review>,
    slider_drag: bool,
}

/// A group as the screens show it: the group, its member keys, and the user's decisions.
struct Shown {
    key: u64,
    resolved: bool,
    skipped: bool,
}

#[derive(Default)]
pub(super) struct Similar {
    store: Option<Arc<Mutex<SimStore>>>,
    scan: Option<Scan>,
    results: Option<Results>,
    /// Protection read from sidecars by the scan, per item (marks are added at regroup).
    sidecar: Vec<Option<Protect>>,
    /// The image list `results` are for, and the one waiting for a scan.
    results_key: u64,
    changed_at: Option<(u64, Instant)>,
    /// When the pending automatic scan should be looked at again (its quiet delay ends, or
    /// the thumbnail engine may have gone idle); None when no scan is pending, so an idle
    /// gallery schedules no wake-ups for it.
    pub(super) auto_at: Option<Instant>,
    /// Images in the list of this key (counting 100k entries per tick was a frame cost).
    image_count: Option<(u64, usize)>,
    generation: u64,
    dups: Vec<Group>,
    sims: Vec<Group>,
    dup_info: Vec<Shown>,
    sim_info: Vec<Shown>,
    /// The scan the user stopped (the list's key, and the phase it was in): no automatic
    /// scan of that list until "Scan again", another folder or a change to the list. Stopped
    /// in [`Phase::Signatures`] nothing is grouped (the signatures made stay in the store,
    /// so the next scan goes on from them); in [`Phase::Focus`] the groups stay, their best
    /// picks from the thumbnails' measures.
    stopped: Option<(u64, Phase)>,
    /// Path → (in a duplicate group: kept, in a similar group: kept).
    roles: HashMap<PathBuf, [Option<bool>; 2]>,
    /// Similar group of each member path (stacks).
    stack_of: HashMap<PathBuf, (usize, bool)>,
    sens: f32,
    pub(super) view: Option<View>,
    /// Discards by undo id, to undo the decisions with the files.
    undo: HashMap<u64, Vec<(Category, u64)>>,
    hits: Vec<(Rect, Act)>,
    /// The tooltip drawn with the last frame (a footer button's key, a large image's
    /// caption), where it was drawn.
    tip: Option<(Rect, String)>,
    /// The sensitivity slider's track (drags follow the pointer along it).
    hits_slider: Option<(Rect, ())>,
    /// Thumbnails the screens want (path, class) and their hash for the feed.
    pub(super) wanted: Vec<(PathBuf, u32)>,
    pub(super) wanted_key: u64,
    folder_rx: Option<Receiver<PathBuf>>,
    /// `--show`: what to open once the scan is done.
    show: Option<String>,
    /// `--show gallery-similar-stopped`: the scan is stopped once it began.
    show_stop: bool,
    regroup_due: bool,
    /// Focus measures came in: the picks are redone (at most every [`REFINE_EVERY`]), the
    /// groups under the user's eyes keeping their member order.
    refine_due: bool,
    refined_at: Option<Instant>,
    /// Groups that kept their old member order at the last refine (the one reviewed, the
    /// row under the pointer): put in the suggested order once let go.
    held: Vec<u64>,
    /// Gallery stack covers in view (on screen or under the pointer) while the focus pass
    /// runs, set each tick: a refine keeps them as their stacks' covers.
    pub(super) covers: Vec<PathBuf>,
    /// Covers the last refine kept although another member became the best: given the
    /// suggested cover once out of view or when the pass ends.
    covers_held: Vec<PathBuf>,
    /// The list row under the pointer: its group's key (set while drawing).
    hover_group: Option<u64>,
    last_click: Option<(f64, Act)>,
    /// Region decodes for the review's zoomed images (`quick.rs`), apart from the viewer's
    /// (which keeps only the current image's).
    pub(super) tile_jobs: Vec<super::super::TileJob>,
    /// The layout direction and text scale the screens were last drawn at (the gallery's;
    /// [`Similar::review_panes`] lays out as they are drawn).
    rtl: bool,
    text_scale: f32,
    /// The review footer's buttons' widths (with their keys, without) as last drawn: the
    /// layout outside drawing wraps them as drawn ([`review_layout`]).
    footer_widths: Vec<(f32, f32)>,
    /// The review's large images as last drawn (their hits are `Act::Focus` as the
    /// strip's).
    panes: Vec<Rect>,
}

/// The member a group is shown by, its best: the first in the suggested order, which is the
/// first member unless a refine held the old order ([`Similar::regroup_held`]).
fn lead(g: &Group) -> Option<usize> {
    let better = |a: &Pick, b: &Pick| {
        b.keep.cmp(&a.keep).then(match g.cat {
            Category::Duplicates => a.score.total_cmp(&b.score),
            Category::Similar => b.score.total_cmp(&a.score),
        })
    };
    (0..g.picks.len()).min_by(|&a, &b| better(&g.picks[a], &g.picks[b]).then(a.cmp(&b)))
}

/// Puts `g`'s members (and their picks) in the order of `old`; false (and `g` as it was)
/// when `old` is not the same set.
fn hold_order(g: &mut Group, old: &[u32]) -> bool {
    if old.len() != g.members.len() {
        return false;
    }
    let mut pos = Vec::with_capacity(old.len());
    for m in old {
        match g.members.iter().position(|x| x == m) {
            Some(p) => pos.push(p),
            None => return false,
        }
    }
    g.picks = pos.iter().map(|&p| g.picks[p]).collect();
    g.members = old.to_vec();
    true
}

fn tr_reason(r: Reason) -> String {
    match r {
        Reason::Protected(p) => protect_text(p),
        Reason::Original => t!("similar-reason-original"),
        Reason::RawOriginal => t!("similar-reason-raw-original"),
        Reason::HighestResolution => t!("similar-reason-highest-resolution"),
        Reason::DeeperColour => t!("similar-reason-deeper-colour"),
        Reason::LeastCompressed => t!("similar-reason-least-compressed"),
        Reason::HasMetadata => t!("similar-reason-has-metadata"),
        Reason::Oldest => t!("similar-reason-oldest"),
        Reason::Sharpest => t!("similar-reason-sharpest"),
        Reason::Steadiest => t!("similar-reason-steadiest"),
        Reason::BestExposure => t!("similar-reason-best-exposure"),
        Reason::LeastNoise => t!("similar-reason-least-noise"),
        Reason::AlsoGood => t!("similar-reason-also-good"),
        Reason::SameFile => t!("similar-reason-same-file"),
        Reason::Derivative => t!("similar-reason-derivative"),
        Reason::RenderingOfRaw => t!("similar-reason-rendering-of-raw"),
        Reason::SmallerCopy => t!("similar-reason-smaller-copy"),
        Reason::ShallowerColour => t!("similar-reason-shallower-colour"),
        Reason::MoreCompressed => t!("similar-reason-more-compressed"),
        Reason::NoMetadata => t!("similar-reason-no-metadata"),
        Reason::NewerCopy => t!("similar-reason-newer-copy"),
        Reason::Blurrier => t!("similar-reason-blurrier"),
        Reason::MotionBlur => t!("similar-reason-motion-blur"),
        Reason::Overexposed => t!("similar-reason-overexposed"),
        Reason::Underexposed => t!("similar-reason-underexposed"),
        Reason::Noisier => t!("similar-reason-noisier"),
        Reason::LowerResolution => t!("similar-reason-lower-resolution"),
    }
}

fn protect_text(p: Protect) -> String {
    match p {
        Protect::Rated => t!("similar-reason-rated"),
        Protect::Flagged => t!("similar-reason-flagged"),
        Protect::Labelled => t!("similar-reason-labelled"),
        Protect::Tagged => t!("similar-reason-tagged"),
        Protect::Edited => t!("similar-reason-edited"),
    }
}

fn cat_name(c: Category) -> String {
    match c {
        Category::Duplicates => t!("similar-duplicates"),
        Category::Similar => t!("similar-similar"),
    }
}

fn action_name(a: DiscardAction, folder: Option<&Path>) -> String {
    match (a, folder) {
        (DiscardAction::Trash, _) => t!("similar-action-trash"),
        (DiscardAction::Move, Some(f)) => t!("similar-action-move-to", folder = file_name(f)),
        (DiscardAction::Move, None) => t!("similar-action-move"),
        (DiscardAction::Reject, _) => t!("similar-action-reject"),
    }
}

fn size_text(bytes: u64) -> String {
    I18n::global()
        .localizer()
        .bytes(bytes, specular_i18n::ByteUnits::Decimal)
}

/// Protection from the gallery's marks (rated, picked, labelled).
fn marks_protect(m: Mark) -> Option<Protect> {
    if m.rating > 0 {
        Some(Protect::Rated)
    } else if m.flag == 1 {
        Some(Protect::Flagged)
    } else if m.label > 0 {
        Some(Protect::Labelled)
    } else {
        None
    }
}

impl Similar {
    pub(super) fn is_open(&self) -> bool {
        self.view.is_some()
    }

    fn groups(&self, c: Category) -> &[Group] {
        match c {
            Category::Duplicates => &self.dups,
            Category::Similar => &self.sims,
        }
    }

    fn info(&self, c: Category) -> &[Shown] {
        match c {
            Category::Duplicates => &self.dup_info,
            Category::Similar => &self.sim_info,
        }
    }

    /// Indices of the groups still to review (not resolved, not skipped).
    fn open_groups(&self, c: Category) -> Vec<usize> {
        self.info(c)
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.resolved && !s.skipped)
            .map(|(i, _)| i)
            .collect()
    }

    fn decision(&self, c: Category, m: u32) -> Decision {
        let (Some(store), Some(r)) = (&self.store, &self.results) else {
            return Decision::Undecided;
        };
        store
            .lock()
            .map_or(Decision::Undecided, |s| s.decision(c, r.keys[m as usize]))
    }

    /// Whether member `m` (with suggestion `p`) is kept: the user's decision, else the pick.
    fn keeps(&self, c: Category, m: u32, p: &Pick) -> bool {
        match self.decision(c, m) {
            Decision::Keep => true,
            Decision::Discard => false,
            Decision::Undecided => p.keep,
        }
    }

    /// Bytes the group's discards free, with the user's decisions.
    fn reclaim(&self, c: Category, g: &Group) -> u64 {
        let Some(r) = &self.results else { return 0 };
        g.members
            .iter()
            .zip(&g.picks)
            .filter(|(m, p)| !self.keeps(c, **m, p))
            .map(|(m, _)| r.items[*m as usize].bytes)
            .sum()
    }

    /// (groups to review, files they discard, bytes freed).
    fn totals(&self, c: Category) -> (usize, usize, u64) {
        let open = self.open_groups(c);
        let mut files = 0;
        let mut bytes = 0;
        for &i in &open {
            let g = &self.groups(c)[i];
            files += g
                .members
                .iter()
                .zip(&g.picks)
                .filter(|(m, p)| !self.keeps(c, **m, p))
                .count();
            bytes += self.reclaim(c, g);
        }
        (open.len(), files, bytes)
    }

    /// The header's summary line once results are in (empty before): groups to review,
    /// files and size, how many groups are resolved and skipped.
    fn summary_line(&self, c: Category) -> String {
        if self.results.is_none() {
            return String::new();
        }
        let (groups, files, bytes) = self.totals(c);
        let total = self.info(c).len();
        let resolved = self.info(c).iter().filter(|s| s.resolved).count();
        let skipped = self
            .info(c)
            .iter()
            .filter(|s| s.skipped && !s.resolved)
            .count();
        let mut s = t!(
            "similar-summary",
            groups = groups as i64,
            files = files as i64,
            size = size_text(bytes)
        );
        if total > 0 {
            s.push_str("  ·  ");
            s.push_str(&t!(
                "similar-progress",
                resolved = resolved as i64,
                total = total as i64
            ));
        }
        if skipped > 0 {
            s.push_str("  ·  ");
            s.push_str(&t!("similar-skipped", count = skipped as i64));
        }
        s
    }

    /// The phase a scan was stopped in, while none runs.
    fn stopped_phase(&self) -> Option<Phase> {
        self.stopped.filter(|_| self.scan.is_none()).map(|(_, p)| p)
    }

    /// A running scan's phase and progress.
    fn progress(&self) -> Option<(Phase, usize, usize)> {
        self.scan.as_ref().map(|s| {
            let phase = if s.phase.load(Ordering::Relaxed) == 1 {
                Phase::Focus
            } else {
                Phase::Signatures
            };
            (
                phase,
                s.done.load(Ordering::Relaxed),
                s.total.load(Ordering::Relaxed),
            )
        })
    }

    /// The groups whose member order must not change under the user: the one reviewed, else
    /// the list row under the pointer.
    fn hold_keys(&self) -> Vec<u64> {
        let Some(v) = &self.view else {
            return Vec::new();
        };
        match (&v.review, self.hover_group) {
            (Some(r), _) => vec![r.group],
            (None, Some(k)) => vec![k],
            (None, None) => Vec::new(),
        }
    }

    /// Redoes the picks after focus measures came in, the held groups keeping their order.
    fn refine(&mut self, marks: &super::marks::Store) {
        self.refine_due = false;
        self.refined_at = Some(Instant::now());
        let hold = self.hold_keys();
        self.held = self.regroup_held(marks, &hold);
    }

    /// Whether a group or stack cover held at the last refine is no longer under the user's
    /// eyes.
    fn released(&self) -> bool {
        if self.covers_held.iter().any(|c| !self.covers.contains(c)) {
            return true;
        }
        if self.held.is_empty() {
            return false;
        }
        let hold = self.hold_keys();
        self.held.iter().any(|k| !hold.contains(k))
    }

    /// Tells the focus pass what to measure first: the reviewed group's members, then those
    /// of `rows` (indices of the open category's groups on screen).
    fn wish(&self, rows: &[usize]) {
        let (Some(s), Some(v)) = (&self.scan, &self.view) else {
            return;
        };
        if v.cat != Category::Similar {
            return;
        }
        let mut items: Vec<u32> = Vec::new();
        if let Some(i) = v
            .review
            .as_ref()
            .and_then(|r| self.group_of_key(v.cat, r.group))
        {
            items.extend(&self.sims[i].members);
        }
        for &gi in rows {
            if let Some(g) = self.sims.get(gi) {
                items.extend(&g.members);
            }
        }
        s.wish.set(items);
    }

    fn path_of(&self, m: u32) -> Option<&Path> {
        self.results
            .as_ref()
            .and_then(|r| r.units.get(m as usize))
            .map(|u| u.sig_path.as_path())
    }

    fn group_of_key(&self, c: Category, key: u64) -> Option<usize> {
        self.info(c).iter().position(|s| s.key == key)
    }

    /// Rebuilds the groups from the results: marks protect, splits and discards apply.
    fn regroup(&mut self, marks: &super::marks::Store) {
        self.held.clear();
        let covers = std::mem::take(&mut self.covers);
        self.regroup_held(marks, &[]);
        self.covers = covers;
    }

    /// [`Similar::regroup`], the groups keyed in `hold` keeping their members' order (their
    /// picks, badges and labels update in place), and the gallery stacks whose cover is in
    /// [`Similar::covers`] keeping it. Returns the groups whose order then differs from the
    /// suggested one (the covers kept so are left in `covers_held`).
    fn regroup_held(&mut self, marks: &super::marks::Store, hold: &[u64]) -> Vec<u64> {
        // The results carry every focus measure received so far: this is the refine too.
        self.regroup_due = false;
        self.refine_due = false;
        let old_order: HashMap<u64, Vec<u32>> = hold
            .iter()
            .filter_map(|&k| {
                [Category::Duplicates, Category::Similar]
                    .into_iter()
                    .find_map(|c| {
                        let i = self.group_of_key(c, k)?;
                        Some((k, self.groups(c)[i].members.clone()))
                    })
            })
            .collect();
        let Some(r) = self.results.as_mut() else {
            self.dups.clear();
            self.sims.clear();
            self.dup_info.clear();
            self.sim_info.clear();
            self.roles.clear();
            self.stack_of.clear();
            return Vec::new();
        };
        for (i, u) in r.units.iter().enumerate() {
            let m = u.files.iter().find_map(|f| marks_protect(marks.mark(f)));
            r.items[i].protect = m.or(self.sidecar.get(i).copied().flatten());
        }
        let store = self.store.clone();
        let mut guard = store.as_ref().and_then(|s| s.lock().ok());
        let split = |a: u32, b: u32| {
            guard
                .as_ref()
                .is_some_and(|s| s.is_split(r.keys[a as usize], r.keys[b as usize]))
        };
        let mut dups = Vec::new();
        for g in &r.duplicates {
            let mut members: Vec<u32> = Vec::new();
            for &m in &g.members {
                if !r.items[m as usize].gone && !members.iter().any(|&o| split(o, m)) {
                    members.push(m);
                }
            }
            if members.len() >= 2 {
                members.sort_unstable();
                // On the UI thread: a panic in one group's suggestion leaves that group out
                // (logged), never the app.
                let what = format!("duplicate group ({} files) skipped", members.len());
                if let Ok(g) = specular_core::isolate::run(what, || {
                    analyse::suggest(&r.items, Category::Duplicates, &members, g.tier)
                }) {
                    dups.push(g);
                }
            }
        }
        dups.sort_by(|a, b| b.reclaim.cmp(&a.reclaim).then(a.members.cmp(&b.members)));
        let items = &r.items;
        let mut sims = specular_core::isolate::run("similar groups skipped", || {
            r.similar(self.sens, &|a, b| {
                split(a, b) || items[a as usize].gone || items[b as usize].gone
            })
        })
        .unwrap_or_default();
        let when = |g: &Group| {
            g.members
                .iter()
                .filter_map(|&m| items[m as usize].hints.time_ms)
                .min()
                .unwrap_or(i64::MAX)
        };
        // Ties by the member set, not the suggested order (which refining changes).
        let set = |g: &Group| {
            let mut m = g.members.clone();
            m.sort_unstable();
            m
        };
        sims.sort_by(|a, b| when(a).cmp(&when(b)).then_with(|| set(a).cmp(&set(b))));
        let info = |c: Category, gs: &[Group]| -> Vec<Shown> {
            gs.iter()
                .map(|g| {
                    let keys: Vec<u64> = g.members.iter().map(|&m| r.keys[m as usize]).collect();
                    let key = specular_similar::store::group_key(&keys);
                    let (resolved, skipped) = guard.as_ref().map_or((false, false), |s| {
                        (
                            keys.iter()
                                .all(|&k| s.decision(c, k) != Decision::Undecided),
                            s.skipped(c, key),
                        )
                    });
                    Shown {
                        key,
                        resolved,
                        skipped,
                    }
                })
                .collect()
        };
        self.dup_info = info(Category::Duplicates, &dups);
        self.sim_info = info(Category::Similar, &sims);
        // Held groups: their old member order, with the new picks.
        let mut held = Vec::new();
        for (gs, shown) in [(&mut dups, &self.dup_info), (&mut sims, &self.sim_info)] {
            for (g, sh) in gs.iter_mut().zip(shown.iter()) {
                if let Some(old) = old_order.get(&sh.key)
                    && *old != g.members
                    && hold_order(g, old)
                {
                    held.push(sh.key);
                }
            }
        }
        // Groups skipped before their members were kept: keep them now, so the skip follows
        // a renamed member.
        if let Some(s) = guard.as_mut() {
            for (c, gs, shown) in [
                (Category::Duplicates, &dups, &self.dup_info),
                (Category::Similar, &sims, &self.sim_info),
            ] {
                for (g, sh) in gs.iter().zip(shown.iter()) {
                    if sh.skipped {
                        let keys: Vec<u64> =
                            g.members.iter().map(|&m| r.keys[m as usize]).collect();
                        s.note_skip_members(c, &keys);
                    }
                }
            }
        }
        drop(guard);
        self.dups = dups;
        self.sims = sims;
        // Roles (filter terms) and stacks.
        let mut roles: HashMap<PathBuf, [Option<bool>; 2]> = HashMap::new();
        let mut stack_of = HashMap::new();
        let mut covers_held = Vec::new();
        let in_view: HashSet<&PathBuf> = self.covers.iter().collect();
        for (ci, c) in [Category::Duplicates, Category::Similar]
            .into_iter()
            .enumerate()
        {
            for (gi, g) in self.groups(c).iter().enumerate() {
                if self.info(c)[gi].resolved {
                    continue;
                }
                // The stack's cover: one in view stays, else the best.
                let units = self.results.as_ref().map(|r| &r.units);
                let kept = (c == Category::Similar && !in_view.is_empty())
                    .then(|| {
                        g.members.iter().position(|&m| {
                            units
                                .and_then(|u| u.get(m as usize))
                                .is_some_and(|u| u.files.iter().any(|f| in_view.contains(f)))
                        })
                    })
                    .flatten();
                let cover = kept.or_else(|| lead(g));
                if let (Some(k), Some(u)) = (kept, units)
                    && kept != lead(g)
                {
                    let files = &u[g.members[k] as usize].files;
                    covers_held.extend(files.iter().filter(|f| in_view.contains(f)).cloned());
                }
                for (k, (&m, p)) in g.members.iter().zip(&g.picks).enumerate() {
                    let keep = self.keeps(c, m, p);
                    let Some(u) = self.results.as_ref().map(|r| &r.units[m as usize]) else {
                        continue;
                    };
                    for f in &u.files {
                        roles.entry(f.clone()).or_insert([None; 2])[ci] = Some(keep);
                        if c == Category::Similar {
                            stack_of.insert(f.clone(), (gi, Some(k) == cover));
                        }
                    }
                }
            }
        }
        self.roles = roles;
        self.stack_of = stack_of;
        self.covers_held = covers_held;
        held
    }

    /// The role of `path` in the duplicate and the similar sets: Some(suggested keeper) for a
    /// member (the query's `dupes:` / `similar:` terms).
    pub(super) fn role(&self, path: &Path) -> (Option<bool>, Option<bool>) {
        self.roles.get(path).map_or((None, None), |r| (r[0], r[1]))
    }

    /// Collapses the members of each similar-shot group into one stack, shown by its best.
    pub(super) fn merge_stacks(
        &self,
        stacks: Vec<(usize, Vec<usize>)>,
        entries: &[Entry],
    ) -> Vec<(usize, Vec<usize>)> {
        if self.stack_of.is_empty() {
            return stacks;
        }
        let mut out: Vec<(usize, Vec<usize>)> = Vec::with_capacity(stacks.len());
        let mut slot: HashMap<usize, usize> = HashMap::new();
        for (shown, members) in stacks {
            let g = members
                .iter()
                .find_map(|&m| self.stack_of.get(&entries[m].path).copied());
            let Some((g, _)) = g else {
                out.push((shown, members));
                continue;
            };
            let best = members
                .iter()
                .copied()
                .find(|&m| self.stack_of.get(&entries[m].path).is_some_and(|s| s.1));
            match slot.get(&g) {
                Some(&s) => {
                    out[s].1.extend(members);
                    if let Some(b) = best {
                        out[s].0 = b;
                    }
                }
                None => {
                    slot.insert(g, out.len());
                    out.push((best.unwrap_or(shown), members));
                }
            }
        }
        out
    }

    /// The members the review shows large: the comparison, else the keepers (four at
    /// most), else the focus. The group's index and the members' positions in it.
    fn review_shown(&self, v: &View, rv: &Review) -> Option<(usize, Vec<usize>)> {
        let gi = self.group_of_key(v.cat, rv.group)?;
        let g = &self.groups(v.cat)[gi];
        let shown = match &rv.compare {
            Some(c) => c.clone(),
            None => {
                let mut k: Vec<usize> = g
                    .members
                    .iter()
                    .zip(&g.picks)
                    .enumerate()
                    .filter(|(_, (m, p))| self.keeps(v.cat, **m, p))
                    .map(|(k, _)| k)
                    .take(4)
                    .collect();
                if k.is_empty() {
                    k.push(rv.focus);
                }
                k
            }
        };
        Some((gi, shown))
    }

    /// The review's large images in a view of `w` × `h` logical px, for rendering them
    /// from the decoded image (`quick.rs`): None without a review.
    pub(super) fn review_panes(&self, w: f32, h: f32) -> Option<ReviewPanes> {
        let v = self.view.as_ref()?;
        let rv = v.review.as_ref()?;
        let (gi, shown) = self.review_shown(v, rv)?;
        let g = &self.groups(v.cat)[gi];
        let r = self.results.as_ref()?;
        let cells = self.review_cells(w, h, shown.len().max(1));
        let members = shown
            .iter()
            .zip(cells)
            .filter_map(|(&k, cell)| {
                let u = r.units.get(*g.members.get(k)? as usize)?;
                Some((u.sig_path.clone(), u.main.clone(), cell))
            })
            .collect();
        Some(ReviewPanes {
            members,
            zoom: rv.zoom,
            at: rv.at,
        })
    }

    /// The zoom over fit at which the focused member's pixels meet the screen's (100 %), in
    /// the review as laid out now in a view of `w` × `h` logical px at `scale`; the first
    /// large image's when the focused one is not among them.
    fn full_zoom(&self, w: f32, h: f32, scale: f32) -> Option<f32> {
        let v = self.view.as_ref()?;
        let rv = v.review.as_ref()?;
        Some(self.pixel_zoom(v, rv, w, h, scale)?.clamp(1.0, 16.0))
    }

    /// Whether "100 %" can zoom in: false when the focused member already shows at 100 % or
    /// larger (smaller than its pane) in the layout the zoom would show in (the comparison,
    /// else the focused member alone). Not zoomed only; unknown sizes count as zoomable.
    fn can_zoom_in(&self, w: f32, h: f32, scale: f32) -> bool {
        let Some(v) = self.view.as_ref() else {
            return true;
        };
        let Some(rv) = v.review.as_ref() else {
            return true;
        };
        if rv.zoom > 1.0 {
            return true;
        }
        let mut alone = rv.clone();
        alone.compare.get_or_insert_with(|| vec![rv.focus]);
        // 1 % of slack: a pane a pixel short of the image is not worth a zoom.
        self.pixel_zoom(v, &alone, w, h, scale)
            .is_none_or(|z| z > 1.01)
    }

    /// The zoom over fit (unclamped) at which the focused member's pixels meet the screen's
    /// in the review `rv` laid out in `w` × `h` at `scale`.
    fn pixel_zoom(&self, v: &View, rv: &Review, w: f32, h: f32, scale: f32) -> Option<f32> {
        let (gi, shown) = self.review_shown(v, rv)?;
        let slot = shown.iter().position(|&k| k == rv.focus).unwrap_or(0);
        let m = *self.groups(v.cat)[gi].members.get(*shown.get(slot)?)?;
        let it = self.results.as_ref()?.items.get(m as usize)?;
        if it.width == 0 || it.height == 0 {
            return None;
        }
        let cell = *self.review_cells(w, h, shown.len().max(1)).get(slot)?;
        let fit = fit_in(it.width as f32, it.height as f32, cell);
        Some(it.width as f32 / (fit.w * scale).max(1.0))
    }

    fn metrics(&self) -> Metrics {
        Metrics::new(self.text_scale)
    }

    /// The cells of the review's `n` large images in a view of `w` × `h`, as drawn: in the
    /// reading direction, first on the right for right-to-left layouts.
    fn review_cells(&self, w: f32, h: f32, n: usize) -> Vec<Rect> {
        let lay = self.review_layout(w, h);
        review_cells(lay.main, n, lay.caption, self.rtl)
    }

    /// The review's layout in a view of `w` × `h`, as drawn.
    fn review_layout(&self, w: f32, h: f32) -> ReviewLayout {
        review_layout(w, h, self.metrics(), &self.footer_widths)
    }

    /// The sensitivity at `x` on the slider's `track` (as drawn): strict at its leading end.
    fn slider_at(&self, x: f32, track: Rect) -> f32 {
        let along = if self.rtl {
            track.right() - x
        } else {
            x - track.x
        };
        (along / track.w.max(1.0)).clamp(0.0, 1.0)
    }

    /// For screenshots: nothing left to wait for.
    pub(super) fn settled(&self) -> bool {
        self.scan.is_none() && self.show.is_none() && !self.regroup_due && !self.refine_due
    }

    fn set_wanted(&mut self, w: Vec<(PathBuf, u32)>) {
        let mut h = std::hash::DefaultHasher::new();
        w.hash(&mut h);
        let k = h.finish();
        if k != self.wanted_key {
            self.wanted_key = k;
            self.wanted = w;
        }
    }
}

#[cfg(test)]
impl Similar {
    /// For the screenshot tests (`shots.rs`): `units` with their `items` as one similar group
    /// under review, its members at `compare` side by side and zoomed to 100 % in a view of
    /// `size` logical px at `scale`, as Z leaves them; laid out for `look` (right to left,
    /// text scale) as the gallery will draw it.
    pub(super) fn for_shots(
        units: Vec<Unit>,
        items: Vec<specular_similar::Item>,
        compare: Vec<usize>,
        size: (f32, f32),
        scale: f32,
        look: (bool, f32),
    ) -> Similar {
        let n = items.len();
        let members: Vec<u32> = (0..n as u32).collect();
        let group = analyse::suggest(&items, Category::Similar, &members, None);
        let mut sm = Similar {
            results: Some(Results {
                units,
                items,
                ids: vec![specular_similar::FileId::default(); n],
                keys: (1..=n as u64).collect(),
                ..Results::default()
            }),
            sims: vec![group],
            sim_info: vec![Shown {
                key: 1,
                resolved: false,
                skipped: false,
            }],
            view: Some(View {
                cat: Category::Similar,
                cursor: 0,
                scroll: 0.0,
                review: Some(Review {
                    group: 1,
                    focus: compare[0],
                    compare: Some(compare),
                    zoom: 1.0,
                    at: (0.5, 0.5),
                }),
                slider_drag: false,
            }),
            rtl: look.0,
            text_scale: look.1,
            ..Similar::default()
        };
        let zoom = sm.full_zoom(size.0, size.1, scale);
        if let Some(rv) = sm.view.as_mut().and_then(|v| v.review.as_mut()) {
            rv.zoom = zoom.unwrap_or(1.0);
        }
        sm
    }

    /// For the screenshot tests: the centre of the review's Compare button as last drawn.
    #[cfg(test)]
    pub(super) fn compare_center_for_shots(&self) -> Option<(f32, f32)> {
        self.hits
            .iter()
            .find(|(_, a)| *a == Act::Compare)
            .map(|(r, _)| r.center())
    }

    /// For the screenshot tests: the centre of the review's "100 %" button as last drawn.
    #[cfg(test)]
    pub(super) fn zoom_center_for_shots(&self) -> Option<(f32, f32)> {
        self.hits
            .iter()
            .find(|(_, a)| *a == Act::Zoom)
            .map(|(r, _)| r.center())
    }

    /// For the screenshot tests: the slider's knob at `s` (the groups are not rebuilt).
    pub(super) fn set_sensitivity_for_shots(&mut self, s: f32) {
        self.sens = s;
    }

    /// For the screenshot tests: `units` as one similar group under review, not zoomed, the
    /// first member focused; the large images are the two frames compared.
    pub(super) fn for_shots_review(
        units: Vec<Unit>,
        items: Vec<specular_similar::Item>,
    ) -> Similar {
        let mut sm =
            Similar::for_shots(units, items, vec![0, 1], (800.0, 600.0), 1.0, (false, 1.0));
        if let Some(rv) = sm.view.as_mut().and_then(|v| v.review.as_mut()) {
            rv.zoom = 1.0;
        }
        sm
    }

    /// For the screenshot tests: `units` as one similar group in the list (no review), the
    /// scan still refining best-pick, `done` of `total` measured.
    pub(super) fn for_shots_list(
        units: Vec<Unit>,
        items: Vec<specular_similar::Item>,
        done: usize,
        total: usize,
    ) -> Similar {
        let mut sm =
            Similar::for_shots(units, items, vec![0, 1], (800.0, 600.0), 1.0, (false, 1.0));
        if let Some(v) = sm.view.as_mut() {
            v.review = None;
        }
        sm.scan = Some(Scan::fake(Phase::Focus, done, total));
        sm
    }

    /// For the screenshot tests: a second, skipped copy of the first group (the list's
    /// footer then offers to show it).
    pub(super) fn add_skipped_for_shots(&mut self) {
        let g = self.sims[0].clone();
        self.sims.push(g);
        self.sim_info.push(Shown {
            key: 2,
            resolved: false,
            skipped: true,
        });
    }

    /// For the screenshot tests: the centre of the list's Accept all button as last drawn.
    pub(super) fn accept_all_center_for_shots(&self) -> Option<(f32, f32)> {
        self.hits
            .iter()
            .find(|(_, a)| *a == Act::AcceptAll)
            .map(|(r, _)| r.center())
    }
}

#[cfg(test)]
impl Scan {
    /// A scan that sends nothing, at `done` of `total` of `phase`.
    fn fake(phase: Phase, done: usize, total: usize) -> Scan {
        let (_, rx) = crossbeam_channel::unbounded();
        Scan {
            cancel: Arc::default(),
            done: Arc::new(AtomicUsize::new(done)),
            total: Arc::new(AtomicUsize::new(total)),
            phase: Arc::new(AtomicUsize::new(usize::from(phase == Phase::Focus))),
            rx,
            key: 0,
            wish: FocusWish::default(),
        }
    }
}

// ----- Scanning and the App side ----------------------------------------------------------

impl App {
    /// The image list the scan works on: its key, and the paths.
    fn similar_list_key(&self) -> u64 {
        let ga = &self.gallery;
        let mut h = std::hash::DefaultHasher::new();
        (ga.generation, ga.entries.len(), &ga.dir).hash(&mut h);
        h.finish()
    }

    fn similar_store(&mut self) -> Arc<Mutex<SimStore>> {
        if let Some(s) = &self.gallery.similar.store {
            return s.clone();
        }
        let dir = if self.args.screenshot.is_some() {
            None
        } else {
            let g = &self.settings.gallery;
            g.cache_location
                .clone()
                .or_else(default_cache_dir)
                .map(|d| d.parent().map_or(d.join("similar"), |p| p.join("similar")))
        };
        let store = dir.map_or_else(SimStore::memory, |d| SimStore::open(&d));
        let sens = store.sensitivity;
        let s = Arc::new(Mutex::new(store));
        self.gallery.similar.store = Some(s.clone());
        self.gallery.similar.sens = sens;
        s
    }

    /// The header's Cancel (and Esc while signatures are made): stops the scan where it
    /// is ([`Similar::stopped`] says what stays).
    fn similar_stop(&mut self) {
        let sm = &mut self.gallery.similar;
        let Some(phase) = sm.progress().map(|p| p.0) else {
            return;
        };
        if let Some(s) = sm.scan.take() {
            s.cancel.store(true, Ordering::Relaxed);
            info!(target: UI, "similar: scan stopped ({phase:?})");
            sm.stopped = Some((s.key, phase));
        }
        self.gallery.announce = Some(match phase {
            Phase::Signatures => t!("similar-stopped"),
            Phase::Focus => t!("similar-stopped-sharpness"),
        });
        self.redraw = true;
    }

    /// Starts a scan of the gallery's images (replacing one running).
    pub(super) fn similar_start(&mut self) {
        let key = self.similar_list_key();
        self.gallery.similar.stopped = None;
        if let Some(s) = self.gallery.similar.scan.take() {
            s.cancel.store(true, Ordering::Relaxed);
        }
        let store = self.similar_store();
        let ga = &self.gallery;
        let paths: Vec<PathBuf> = ga
            .entries
            .iter()
            .filter(|e| e.kind == EntryKind::Image)
            .map(|e| e.path.clone())
            .collect();
        let g = &self.settings.gallery;
        let cache_dir = g.cache_location.clone().or_else(default_cache_dir);
        let budget = (g.cache_size.max(64) as u64) << 20;
        let cache = cache_dir.map(|d| Arc::new(DiskCache::new(d, budget)));
        let keys = specular_thumbs::KeyMemo::default();
        let (engine, wake_rx) = scan::engine(
            self.loader.registry(),
            self.loader.thumb_pool(),
            cache,
            EngineConfig::default().flags,
            g.prefer_embedded,
            keys.clone(),
            None,
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicUsize::new(0));
        let total = Arc::new(AtomicUsize::new(paths.len()));
        let phase = Arc::new(AtomicUsize::new(0));
        let wish = FocusWish::default();
        let (tx, rx) = crossbeam_channel::unbounded();
        let setup = ScanSetup {
            engine,
            wake: wake_rx,
            store,
            cancel: cancel.clone(),
            keys,
            tier3: Default::default(),
        };
        let wake = self.loader.waker();
        let (d2, t2, p2, w2) = (done.clone(), total.clone(), phase.clone(), wish.clone());
        info!(target: UI, "similar: scanning {} images", paths.len());
        self.loader.background("sim-scan", move |_| {
            // In the background-work panel while it scans (gone when this returns).
            let work = Work::start(WorkKind::Similar, WorkStep::Signatures);
            work.set_then(&[WorkStep::Focus]);
            // A panic in the scan (a decoder or parser bug) ends it like a failed scan: the
            // gallery stops waiting, and the decisions store it may have held stays usable.
            let r = specular_core::isolate::run("similar scan", || {
                let units = scan::units(&paths);
                t2.store(units.len(), Ordering::Relaxed);
                let protect = |u: &Unit| scan::sidecar_protect(&u.files);
                let mut last = Instant::now();
                let mut progress = |p: scan::Progress| {
                    p2.store(usize::from(p.phase == Phase::Focus), Ordering::Relaxed);
                    t2.store(p.total, Ordering::Relaxed);
                    d2.store(p.done, Ordering::Relaxed);
                    if p.phase == Phase::Focus {
                        work.set_step(WorkStep::Focus, Some(p.total as u64));
                        work.set_then(&[]);
                    }
                    work.set_progress(p.done as u64, Some(p.total as u64));
                    if last.elapsed() > Duration::from_millis(100) {
                        last = Instant::now();
                        wake();
                    }
                };
                let Some(r) = scan::run_groups(&setup, units, &protect, &mut progress) else {
                    let _ = tx.send(ScanMsg::Done(false));
                    wake();
                    return;
                };
                // The groups first, best-pick from the thumbnails; then the refinement.
                let mut input = FocusInput::of(&r);
                let todo = input.jobs(true).len();
                p2.store(1, Ordering::Relaxed);
                t2.store(todo, Ordering::Relaxed);
                d2.store(0, Ordering::Relaxed);
                let _ = tx.send(ScanMsg::Groups(Box::new(r)));
                wake();
                let measured = scan::run_focus(
                    &setup,
                    &mut input,
                    &w2,
                    &mut |m| {
                        let _ = tx.send(ScanMsg::Focus(m.to_vec()));
                        wake();
                    },
                    &mut progress,
                );
                let _ = tx.send(ScanMsg::Done(measured.is_some()));
                wake();
            });
            if r.is_err() {
                setup.store.clear_poison();
                let _ = tx.send(ScanMsg::Done(false));
                wake();
            }
        });
        self.gallery.similar.scan = Some(Scan {
            cancel,
            done,
            total,
            phase,
            rx,
            key,
            wish,
        });
    }

    /// Once per event-loop turn (from `gallery_tick`): scan results, rescans, folder dialog.
    pub(super) fn similar_tick(&mut self) {
        let key = self.similar_list_key();
        let now = Instant::now();
        let sm = &mut self.gallery.similar;
        // The scan's groups, its focus measures, its end.
        let mut msgs = Vec::new();
        if let Some(s) = &sm.scan {
            msgs.extend(s.rx.try_iter().map(|m| (s.key, m)));
        }
        let got = !msgs.is_empty();
        for (k, msg) in msgs {
            match msg {
                ScanMsg::Groups(r) => {
                    info!(
                        target: UI,
                        "similar: {} items, {} duplicate groups, {} edges in {:.0} ms \
                         ({} hashed, {} signatures from the thumbnail engine, {} of them \
                         cached); refining best-pick",
                        r.items.len(),
                        r.duplicates.len(),
                        r.edges.len(),
                        r.timings.groups.as_secs_f64() * 1e3,
                        r.timings.hashed,
                        r.timings.digests,
                        r.timings.digests_cached
                    );
                    sm.sidecar = r.items.iter().map(|it| it.protect).collect();
                    sm.results = Some(*r);
                    sm.results_key = k;
                    sm.regroup_due = true;
                    sm.refine_due = false;
                }
                ScanMsg::Focus(m) => {
                    if let Some(r) = sm.results.as_mut().filter(|_| sm.results_key == k) {
                        r.apply_focus(&m);
                        sm.refine_due = true;
                    }
                }
                ScanMsg::Done(complete) => {
                    if complete {
                        info!(target: UI, "similar: best-pick refined");
                    }
                    sm.scan = None;
                }
            }
        }
        if got || (sm.scan.is_some() && sm.view.is_some()) {
            // New groups or picks; progress shows.
            self.redraw = true;
        }
        let sm = &mut self.gallery.similar;
        // A different folder: the old results no longer apply.
        if sm.generation != self.gallery.generation {
            sm.generation = self.gallery.generation;
            sm.results = None;
            sm.results_key = 0;
            sm.regroup_due = true;
            sm.stopped = None;
            if let Some(s) = sm.scan.take() {
                s.cancel.store(true, Ordering::Relaxed);
            }
        }
        // Rescan when the images changed and things are quiet.
        sm.auto_at = None;
        let stale = sm.results_key != key
            && sm.scan.as_ref().is_none_or(|s| s.key != key)
            && sm.stopped.is_none_or(|(k, _)| k != key);
        if stale && !self.gallery.listing && self.gallery.dir.is_some() {
            let since = match sm.changed_at {
                Some((k, t)) if k == key => t,
                _ => {
                    sm.changed_at = Some((key, now));
                    now
                }
            };
            let images = match self.gallery.similar.image_count {
                Some((k, n)) if k == key => n,
                _ => {
                    let n = self
                        .gallery
                        .entries
                        .iter()
                        .filter(|e| e.kind == EntryKind::Image)
                        .count();
                    self.gallery.similar.image_count = Some((key, n));
                    n
                }
            };
            let wanted = self.gallery.similar.view.is_some() || self.gallery.similar.show.is_some();
            // The engine check last and without waiting: its locks are shared with the
            // thumbnail workers, and a blocking check held a frame for up to 54 ms.
            let auto = self.gallery.active
                && self.gallery.bench.is_none()
                && (2..=AUTO_SCAN_MAX).contains(&images);
            let due = now.duration_since(since) >= AUTO_SCAN_DELAY;
            let quiet = auto
                && due
                && self
                    .gallery
                    .engine
                    .as_ref()
                    .is_some_and(|e| e.try_idle() == Some(true));
            if images >= 2 && (wanted || quiet) {
                self.similar_start();
                if std::mem::take(&mut self.gallery.similar.show_stop) {
                    self.similar_stop();
                }
            } else if auto {
                // Nothing else need wake the loop when the delay ends (the next unrelated
                // wake was the 30 s power poll): wake for it, and while the thumbnail engine
                // is busy, look again now and then. Only while a scan is pending.
                let at = if due {
                    now + AUTO_SCAN_RECHECK
                } else {
                    since + AUTO_SCAN_DELAY
                };
                self.gallery.similar.auto_at = Some(at);
                self.wake_by(at, crate::app::Cause::Gallery);
            } else if images < 2 && self.gallery.similar.show.is_some() {
                self.gallery.similar.show = None;
            }
        }
        if let Some(rx) = &self.gallery.similar.folder_rx
            && let Ok(dir) = rx.try_recv()
        {
            self.gallery.similar.folder_rx = None;
            if let Some(s) = &self.gallery.similar.store
                && let Ok(mut s) = s.lock()
            {
                s.discard = DiscardAction::Move;
                s.folder = Some(dir);
                s.options_changed();
            }
            self.similar_flush();
            self.redraw = true;
        }
        // The stack covers in view stay while the pass runs (none once it ended).
        let covers = if self.gallery.similar.scan.is_some() {
            self.gallery.stack_covers_in_view()
        } else {
            Vec::new()
        };
        self.gallery.similar.covers = covers;
        // Picks refined by focus measures: at most every REFINE_EVERY while the pass runs
        // (at once when it ended), and when a held group or cover is let go.
        let sm = &self.gallery.similar;
        if !sm.regroup_due && (sm.refine_due || sm.released()) {
            let at = if sm.scan.is_none() || sm.released() {
                now
            } else {
                sm.refined_at.map_or(now, |t| t + REFINE_EVERY)
            };
            if at <= now {
                let ga = &mut self.gallery;
                ga.similar.refine(&ga.marks);
                ga.dirty |= ga.stacks_on || ga.filter.uses_sets();
                self.similar_fix_view();
                self.redraw = true;
            } else {
                self.wake_by(at, crate::app::Cause::Gallery);
            }
        }
        if self.gallery.similar.regroup_due {
            let ga = &mut self.gallery;
            ga.similar.regroup(&ga.marks);
            ga.dirty |= ga.stacks_on || ga.filter.uses_sets();
            self.similar_fix_view();
            self.redraw = true;
            if self.gallery.similar.results.is_some()
                && let Some(s) = self.gallery.similar.show.take()
            {
                self.similar_apply_show(&s);
            }
        }
    }

    /// After a regroup: the cursor and the reviewed group stay valid.
    fn similar_fix_view(&mut self) {
        let sm = &mut self.gallery.similar;
        let Some(v) = sm.view.clone() else { return };
        let open = sm.open_groups(v.cat);
        let mut v = v;
        v.cursor = v.cursor.min(open.len().saturating_sub(1));
        if let Some(r) = &v.review
            && sm.group_of_key(v.cat, r.group).is_none_or(|i| {
                let s = &sm.info(v.cat)[i];
                s.resolved || s.skipped
            })
        {
            // Gone (resolved, split apart): on to the next open group.
            v.review = open.get(v.cursor).map(|&i| Review {
                group: sm.info(v.cat)[i].key,
                zoom: 1.0,
                at: (0.5, 0.5),
                ..Review::default()
            });
        }
        if let Some(r) = v.review.as_mut()
            && let Some(i) = sm.group_of_key(v.cat, r.group)
        {
            let n = sm.groups(v.cat)[i].members.len();
            r.focus = r.focus.min(n.saturating_sub(1));
            if let Some(c) = r.compare.as_mut() {
                c.retain(|&k| k < n);
                if c.len() < 2 {
                    r.compare = None;
                }
            }
        }
        sm.view = Some(v);
    }

    fn similar_flush(&mut self) {
        if let Some(s) = self.gallery.similar.store.clone() {
            self.loader.background("sim-store", move |_| {
                if let Ok(mut s) = s.lock()
                    && let Err(e) = s.flush()
                {
                    warn!(target: UI, "similar: saving decisions: {e}");
                }
            });
        }
    }

    /// Opens a category (its group list).
    pub(super) fn similar_open(&mut self, cat: Category) {
        self.similar_store();
        let sm = &mut self.gallery.similar;
        sm.view = Some(View {
            cat,
            cursor: 0,
            scroll: 0.0,
            review: None,
            slider_drag: false,
        });
        sm.regroup_due = true;
        self.gallery.quick = None;
        self.gallery.compare = None;
        self.redraw = true;
    }

    fn similar_close(&mut self) {
        self.gallery.similar.view = None;
        self.gallery.similar.set_wanted(Vec::new());
        self.gallery.wish_key = 0;
        self.redraw = true;
    }

    /// The View menu's and the palette's entries: each category with its count and size.
    pub(super) fn similar_menu_items(&self) -> Vec<PaletteItem> {
        let sm = &self.gallery.similar;
        [Category::Duplicates, Category::Similar]
            .into_iter()
            .map(|c| {
                let name = cat_name(c);
                let label = if let Some((_, d, t)) = sm.progress().filter(|_| sm.results.is_none())
                {
                    t!(
                        "similar-menu-scanning",
                        name = name,
                        percent = (d * 100 / t.max(1)) as i64
                    )
                } else if sm.results.is_some() {
                    let (groups, _, bytes) = sm.totals(c);
                    if groups == 0 {
                        t!("similar-menu-none", name = name)
                    } else {
                        t!(
                            "similar-menu-entry",
                            name = name,
                            groups = groups as i64,
                            size = size_text(bytes)
                        )
                    }
                } else {
                    name
                };
                let code = match c {
                    Category::Duplicates => code::DUPES,
                    Category::Similar => code::SIMILAR,
                };
                item(code, label, None, Icon::Image)
            })
            .collect()
    }

    /// One of our palette or menu codes.
    pub(super) fn similar_code(&mut self, c: u64) {
        let now = self.now();
        match c {
            code::DUPES => self.similar_open(Category::Duplicates),
            code::SIMILAR => self.similar_open(Category::Similar),
            code::ACCEPT_ALL => self.similar_confirm_all(),
            code::ACCEPT_ALL_YES => self.similar_accept_all(now),
            code::ACTION_MENU => self.similar_action_menu(),
            c if (code::ACTION..code::ACTION + 3).contains(&c) => {
                let a = [
                    DiscardAction::Trash,
                    DiscardAction::Move,
                    DiscardAction::Reject,
                ][(c - code::ACTION) as usize];
                let has_folder = self
                    .similar_store()
                    .lock()
                    .is_ok_and(|s| s.folder.is_some());
                if a == DiscardAction::Move && !has_folder {
                    self.similar_choose_folder();
                } else {
                    if let Ok(mut s) = self.similar_store().lock() {
                        s.discard = a;
                        s.options_changed();
                    }
                    self.similar_flush();
                }
            }
            code::CHOOSE_FOLDER => self.similar_choose_folder(),
            code::UNSKIP => self.similar_act(Act::Unskip),
            _ => {}
        }
        self.redraw = true;
    }

    fn similar_choose_folder(&mut self) {
        let (tx, rx) = crossbeam_channel::bounded::<PathBuf>(1);
        let start = self.gallery.dir.clone();
        self.loader.background("dlg-similar", move |l| {
            let mut d = rfd::FileDialog::new();
            if let Some(s) = start {
                d = d.set_directory(s);
            }
            if let Some(p) = d.pick_folder() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
        self.gallery.similar.folder_rx = Some(rx);
    }

    fn similar_action_menu(&mut self) {
        let (cur, folder) = self
            .similar_store()
            .lock()
            .map(|s| (s.discard, s.folder.clone()))
            .unwrap_or_default();
        let mut items: Vec<PaletteItem> = [
            DiscardAction::Trash,
            DiscardAction::Move,
            DiscardAction::Reject,
        ]
        .iter()
        .enumerate()
        .map(|(i, &a)| {
            check(
                code::ACTION + i as u64,
                action_name(a, folder.as_deref()),
                a == cur,
                None,
            )
        })
        .collect();
        items.push(item(
            code::CHOOSE_FOLDER,
            t!("similar-choose-folder"),
            None,
            Icon::Folder,
        ));
        self.chrome.open_list(
            t!(
                "similar-discard-with",
                action = action_name(cur, folder.as_deref())
            ),
            items,
            None,
        );
    }

    /// Asks before accepting every open group's suggestions.
    fn similar_confirm_all(&mut self) {
        let Some(cat) = self.gallery.similar.view.as_ref().map(|v| v.cat) else {
            return;
        };
        let (groups, files, bytes) = self.gallery.similar.totals(cat);
        if groups == 0 {
            return;
        }
        let (a, folder) = self
            .similar_store()
            .lock()
            .map(|s| (s.discard, s.folder.clone()))
            .unwrap_or_default();
        let title = t!(
            "similar-confirm",
            groups = groups as i64,
            files = files as i64,
            size = size_text(bytes),
            action = action_name(a, folder.as_deref())
        );
        let items = vec![
            item(
                code::ACCEPT_ALL_YES,
                t!("similar-confirm-yes", files = files as i64),
                None,
                Icon::Check,
            ),
            item(code::CANCEL, t!("similar-cancel"), None, Icon::Minus),
        ];
        self.chrome.open_list(title, items, None);
    }

    fn similar_accept_all(&mut self, now: f64) {
        let Some(cat) = self.gallery.similar.view.as_ref().map(|v| v.cat) else {
            return;
        };
        let groups: Vec<usize> = self.gallery.similar.open_groups(cat);
        let n = groups.len();
        self.similar_accept(cat, &groups, now);
        if n > 0 {
            self.chrome
                .toast(t!("similar-toast-accepted", count = n as i64), None, now);
        }
        if let Some(v) = self.gallery.similar.view.as_mut() {
            v.review = None;
            v.cursor = 0;
        }
    }

    /// Accepts groups: every member gets its decision (the user's, else the suggestion), and
    /// the discarded ones are carried out.
    fn similar_accept(&mut self, cat: Category, groups: &[usize], now: f64) {
        let store = self.similar_store();
        let sm = &mut self.gallery.similar;
        let Some(r) = sm.results.as_ref() else { return };
        let mut discard_units: Vec<u32> = Vec::new();
        let mut decided: Vec<(Category, u64)> = Vec::new();
        {
            let Ok(mut s) = store.lock() else { return };
            for &gi in groups {
                let g = &sm.groups(cat)[gi];
                for (&m, p) in g.members.iter().zip(&g.picks) {
                    let k = r.keys[m as usize];
                    let d = match s.decision(cat, k) {
                        Decision::Undecided if p.keep => Decision::Keep,
                        Decision::Undecided => Decision::Discard,
                        d => d,
                    };
                    // Never discard what the user marked, whatever was pressed before.
                    let d = if r.items[m as usize].protect.is_some() {
                        Decision::Keep
                    } else {
                        d
                    };
                    s.decide(cat, k, d);
                    if d == Decision::Discard {
                        discard_units.push(m);
                        decided.push((cat, k));
                    }
                }
            }
        }
        let mut files: Vec<PathBuf> = Vec::new();
        let mut images: Vec<PathBuf> = Vec::new();
        for &m in &discard_units {
            let u = &r.units[m as usize];
            for f in &u.files {
                images.push(f.clone());
                files.push(f.clone());
                files.extend(scan::sidecars(f));
            }
        }
        if let Some(r) = sm.results.as_mut() {
            for &m in &discard_units {
                r.items[m as usize].gone = true;
            }
        }
        sm.regroup_due = true;
        let (action, folder) = store
            .lock()
            .map(|s| (s.discard, s.folder.clone()))
            .unwrap_or_default();
        // Files carried away: their decisions never follow a copy of the same content.
        if (action == DiscardAction::Trash || action == DiscardAction::Move && folder.is_some())
            && !images.is_empty()
            && let Ok(mut s) = store.lock()
        {
            let keys: Vec<u64> = decided.iter().map(|&(_, k)| k).collect();
            s.carried_out(&keys);
        }
        self.similar_flush();
        if images.is_empty() {
            return;
        }
        let undo = match (action, folder) {
            (DiscardAction::Trash, _) => self.gallery_trash_paths(files),
            (DiscardAction::Move, Some(dir)) => self.gallery_move(files, dir, false),
            (DiscardAction::Move, None) | (DiscardAction::Reject, _) => {
                // Flag only: the files stay, marked as rejected (undoable like any mark).
                let mut old = Vec::new();
                let mut changed = Vec::new();
                for p in &images {
                    let m = self.gallery.marks.mark(p);
                    let n = Mark { flag: -1, ..m };
                    if n != m {
                        old.push((p.clone(), m));
                        self.gallery.marks.set_mark(p, n);
                        changed.push((p.clone(), n));
                    }
                }
                self.gallery_marks_changed(changed);
                let u = self.next_undo;
                self.next_undo += 1;
                self.gallery.undo.insert(u, UndoOp::Marks(old));
                self.last_undo = Some(u);
                self.chrome.toast(
                    t!("similar-toast-rejected", count = images.len() as i64),
                    Some(u),
                    now,
                );
                Some(u)
            }
        };
        if let Some(u) = undo {
            self.gallery.similar.undo.insert(u, decided);
        }
    }

    /// An undo of ours: the files are back, so their decisions are undone too.
    pub(super) fn similar_undone(&mut self, u: u64) {
        let Some(keys) = self.gallery.similar.undo.remove(&u) else {
            return;
        };
        let store = self.similar_store();
        if let Ok(mut s) = store.lock() {
            for &(c, k) in &keys {
                s.decide(c, k, Decision::Undecided);
            }
        }
        let sm = &mut self.gallery.similar;
        if let Some(r) = sm.results.as_mut() {
            for (i, k) in r.keys.iter().enumerate() {
                if keys.iter().any(|(_, x)| x == k) {
                    r.items[i].gone = false;
                }
            }
        }
        sm.regroup_due = true;
        self.similar_flush();
    }

    /// The marks changed: protection follows.
    pub(super) fn similar_marks_changed(&mut self) {
        if self.gallery.similar.results.is_some() {
            self.gallery.similar.regroup_due = true;
        }
    }

    /// The reviewed group's index and the view's category.
    fn similar_reviewed(&self) -> Option<(Category, usize)> {
        let sm = &self.gallery.similar;
        let v = sm.view.as_ref()?;
        let r = v.review.as_ref()?;
        Some((v.cat, sm.group_of_key(v.cat, r.group)?))
    }

    fn similar_act(&mut self, a: Act) {
        let now = self.now();
        let Some(v) = self.gallery.similar.view.clone() else {
            return;
        };
        let cat = v.cat;
        match a {
            Act::Close => self.similar_close(),
            Act::Stop => self.similar_stop(),
            Act::Rescan => self.similar_start(),
            Act::Back => {
                let sm = &mut self.gallery.similar;
                let Some(v) = sm.view.as_mut() else { return };
                match v.review.as_mut() {
                    Some(r) if r.zoom > 1.0 => r.zoom = 1.0,
                    Some(r) if r.compare.is_some() => r.compare = None,
                    Some(_) => v.review = None,
                    None => self.similar_close(),
                }
            }
            Act::Tab(c) => {
                if c != cat {
                    self.similar_open(c);
                }
            }
            Act::Open(i) => {
                let sm = &mut self.gallery.similar;
                let open = sm.open_groups(cat);
                if let Some(&gi) = open.get(i) {
                    let key = sm.info(cat)[gi].key;
                    if let Some(v) = sm.view.as_mut() {
                        v.cursor = i;
                        v.review = Some(Review {
                            group: key,
                            zoom: 1.0,
                            at: (0.5, 0.5),
                            ..Review::default()
                        });
                    }
                }
            }
            Act::AcceptAll => self.similar_confirm_all(),
            Act::ActionMenu => self.similar_action_menu(),
            Act::Unskip => {
                if let Ok(mut s) = self.similar_store().lock() {
                    s.unskip_all(cat);
                }
                self.gallery.similar.regroup_due = true;
                self.similar_flush();
            }
            Act::Slider => {}
            Act::Focus(k) => {
                if let Some(r) = self
                    .gallery
                    .similar
                    .view
                    .as_mut()
                    .and_then(|v| v.review.as_mut())
                {
                    r.focus = k;
                }
            }
            Act::Set(d) => {
                let Some((cat, gi)) = self.similar_reviewed() else {
                    return;
                };
                let sm = &self.gallery.similar;
                let focus = sm
                    .view
                    .as_ref()
                    .and_then(|v| v.review.as_ref())
                    .map_or(0, |r| r.focus);
                let g = &sm.groups(cat)[gi];
                let Some(&m) = g.members.get(focus) else {
                    return;
                };
                let protected = sm
                    .results
                    .as_ref()
                    .is_some_and(|r| r.items[m as usize].protect.is_some());
                if protected && d == Decision::Discard {
                    self.chrome.toast(t!("similar-toast-protected"), None, now);
                    return;
                }
                let k = sm.results.as_ref().map(|r| r.keys[m as usize]);
                let n = g.members.len();
                if let Some(k) = k
                    && let Ok(mut s) = self.similar_store().lock()
                {
                    s.decide(cat, k, d);
                }
                // Moves on along the strip (a culling pass), unless it is the last one.
                if let Some(r) = self
                    .gallery
                    .similar
                    .view
                    .as_mut()
                    .and_then(|v| v.review.as_mut())
                    && r.focus + 1 < n
                    && d != Decision::Undecided
                {
                    r.focus += 1;
                }
                self.similar_refresh_open_review();
                self.similar_flush();
            }
            Act::Toggle(k) => {
                self.similar_act(Act::Focus(k));
                let Some((cat, gi)) = self.similar_reviewed() else {
                    return;
                };
                let sm = &self.gallery.similar;
                let g = &sm.groups(cat)[gi];
                let Some((&m, p)) = g.members.iter().zip(&g.picks).nth(k) else {
                    return;
                };
                let d = if sm.keeps(cat, m, p) {
                    Decision::Discard
                } else {
                    Decision::Keep
                };
                if let Some(r) = self
                    .gallery
                    .similar
                    .view
                    .as_mut()
                    .and_then(|v| v.review.as_mut())
                {
                    r.focus = k;
                }
                // Set without moving on.
                let k2 = self
                    .gallery
                    .similar
                    .results
                    .as_ref()
                    .map(|r| (r.keys[m as usize], r.items[m as usize].protect.is_some()));
                if let Some((key, protected)) = k2 {
                    if protected && d == Decision::Discard {
                        self.chrome.toast(t!("similar-toast-protected"), None, now);
                        return;
                    }
                    if let Ok(mut s) = self.similar_store().lock() {
                        s.decide(cat, key, d);
                    }
                }
                self.similar_refresh_open_review();
                self.similar_flush();
            }
            Act::AcceptNext => {
                let Some((cat, gi)) = self.similar_reviewed() else {
                    if self
                        .gallery
                        .similar
                        .view
                        .as_ref()
                        .is_some_and(|v| v.review.is_none())
                    {
                        let c = self.gallery.similar.view.as_ref().map_or(0, |v| v.cursor);
                        self.similar_act(Act::Open(c));
                    }
                    return;
                };
                self.similar_accept(cat, &[gi], now);
                self.similar_next_after(cat, gi);
            }
            Act::Split => {
                let Some((cat, gi)) = self.similar_reviewed() else {
                    return;
                };
                let sm = &self.gallery.similar;
                let focus = sm
                    .view
                    .as_ref()
                    .and_then(|v| v.review.as_ref())
                    .map_or(0, |r| r.focus);
                let g = &sm.groups(cat)[gi];
                let Some(r) = sm.results.as_ref() else { return };
                let Some(&m) = g.members.get(focus) else {
                    return;
                };
                let k = r.keys[m as usize];
                let others: Vec<u64> = g
                    .members
                    .iter()
                    .filter(|&&o| o != m)
                    .map(|&o| r.keys[o as usize])
                    .collect();
                let members: Vec<u64> = g.members.iter().map(|&o| r.keys[o as usize]).collect();
                if let Ok(mut s) = self.similar_store().lock() {
                    for o in others {
                        s.split(k, o);
                    }
                }
                // The rest of the group stays under review (its key changes).
                let rest: Vec<u64> = members.into_iter().filter(|&x| x != k).collect();
                let new_key = specular_similar::store::group_key(&rest);
                let sm = &mut self.gallery.similar;
                if let Some(r) = sm.view.as_mut().and_then(|v| v.review.as_mut()) {
                    r.group = new_key;
                    r.focus = 0;
                    r.compare = None;
                }
                let ga = &mut self.gallery;
                ga.similar.regroup(&ga.marks);
                self.similar_fix_view();
                self.similar_flush();
            }
            Act::Skip => {
                let Some((cat, gi)) = self.similar_reviewed() else {
                    return;
                };
                let sm = &self.gallery.similar;
                let Some(r) = sm.results.as_ref() else { return };
                let keys: Vec<u64> = sm.groups(cat)[gi]
                    .members
                    .iter()
                    .map(|&m| r.keys[m as usize])
                    .collect();
                if let Ok(mut s) = self.similar_store().lock() {
                    s.skip_group(cat, &keys);
                }
                self.similar_next_after(cat, gi);
                self.similar_flush();
            }
            Act::Compare => {
                let Some((cat, gi)) = self.similar_reviewed() else {
                    return;
                };
                let sm = &self.gallery.similar;
                let g = &sm.groups(cat)[gi];
                let kept: Vec<usize> = g
                    .members
                    .iter()
                    .zip(&g.picks)
                    .enumerate()
                    .filter(|(_, (m, p))| sm.keeps(cat, **m, p))
                    .map(|(k, _)| k)
                    .collect();
                let n = g.members.len();
                let Some(r) = self
                    .gallery
                    .similar
                    .view
                    .as_mut()
                    .and_then(|v| v.review.as_mut())
                else {
                    return;
                };
                if r.compare.is_some() {
                    r.compare = None;
                    self.redraw = true;
                    return;
                }
                // The focused one beside the keepers (else the first ones), 2–4 in all.
                let mut set = vec![r.focus];
                for k in kept.into_iter().chain(0..n) {
                    if set.len() >= 4.min(n) {
                        break;
                    }
                    if !set.contains(&k) {
                        set.push(k);
                    }
                }
                set.sort_unstable();
                if set.len() >= 2 {
                    r.compare = Some(set);
                }
            }
            Act::Zoom => {
                // Nothing to zoom to: say why (as Discard does for a protected photo).
                if let Some(why) = self.gallery.similar_disabled(Act::Zoom) {
                    self.chrome.toast(why, None, now);
                    return;
                }
                let pointer = self.gallery.pointer;
                let (vw, vh) = (self.gallery.vw, self.gallery.vh);
                let scale = self.gallery.scale.max(0.5);
                let sm = &mut self.gallery.similar;
                // The pane under the pointer gives the point (the focus point when zoomed).
                let at = sm.hits.iter().find_map(|(rect, a)| {
                    (matches!(a, Act::Focus(_))
                        && sm.panes.contains(rect)
                        && pointer.is_some_and(|p| rect.contains(p)))
                    .then(|| {
                        let p = pointer.unwrap_or_default();
                        ((p.0 - rect.x) / rect.w, (p.1 - rect.y) / rect.h)
                    })
                });
                let Some(r) = sm.view.as_mut().and_then(|v| v.review.as_mut()) else {
                    return;
                };
                if r.zoom > 1.0 {
                    r.zoom = 1.0;
                } else {
                    r.at = at.unwrap_or((0.5, 0.5));
                    if r.compare.is_none() {
                        // Zoom shows in the comparison (or the focused one alone).
                        r.compare = Some(vec![r.focus]);
                    }
                    // 100 %: from the layout the zoom is shown in (not the last frame's,
                    // which had other panes before the focused one went alone, or none yet).
                    let z = sm.full_zoom(vw, vh, scale).unwrap_or(4.0);
                    if let Some(r) = sm.view.as_mut().and_then(|v| v.review.as_mut()) {
                        r.zoom = z;
                    }
                }
            }
        }
        self.gallery.wish_key = 0;
        self.redraw = true;
    }

    /// After a decision: groups now fully decided drop out of the list.
    fn similar_refresh_open_review(&mut self) {
        let ga = &mut self.gallery;
        let key = ga
            .similar
            .view
            .as_ref()
            .and_then(|v| v.review.as_ref())
            .map(|r| r.group);
        ga.similar.regroup(&ga.marks);
        // Keep reviewing a group the user just decided fully (Space accepts and moves on).
        if let (Some(v), Some(k)) = (ga.similar.view.as_ref(), key)
            && let Some(i) = ga.similar.group_of_key(v.cat, k)
        {
            let c = v.cat;
            match c {
                Category::Duplicates => ga.similar.dup_info[i].resolved = false,
                Category::Similar => ga.similar.sim_info[i].resolved = false,
            }
        }
        self.similar_fix_view();
    }

    /// On to the group after `gi` (or back to the list when none is left).
    fn similar_next_after(&mut self, cat: Category, gi: usize) {
        let ga = &mut self.gallery;
        ga.similar.regroup(&ga.marks);
        let sm = &mut ga.similar;
        let open = sm.open_groups(cat);
        let next = open
            .iter()
            .position(|&i| i > gi)
            .or_else(|| (!open.is_empty()).then_some(0));
        let key = next.map(|p| (p, sm.info(cat)[open[p]].key));
        if let Some(v) = sm.view.as_mut() {
            match key {
                Some((p, k)) => {
                    v.cursor = p;
                    v.review = Some(Review {
                        group: k,
                        zoom: 1.0,
                        at: (0.5, 0.5),
                        ..Review::default()
                    });
                }
                None => {
                    v.review = None;
                    v.cursor = 0;
                }
            }
        }
        self.similar_fix_view();
    }

    /// Keys while a category is open. True: used (everything is, but app shortcuts).
    pub(super) fn similar_key(&mut self, combo: Option<&specular_input::KeyCombo>) -> bool {
        use specular_input::{Key, NamedKey};
        let Some(combo) = combo else { return true };
        let m = combo.mods;
        if m.ctrl || m.alt || m.meta {
            return false;
        }
        // Shift+digit is a shortcut (the channel views, Shift+1…5): the action goes to the
        // review's large images (`gallery_channel`).
        if m.shift
            && matches!(combo.key, Key::Char(c) if c.is_ascii_digit())
            && self.keymap.lookup(combo).is_some()
        {
            return false;
        }
        let Some(v) = self.gallery.similar.view.clone() else {
            return false;
        };
        let reviewing = v.review.is_some();
        let n_open = self.gallery.similar.open_groups(v.cat).len();
        // While the signatures are made (nothing to review yet), Esc stops the scan; then
        // it leaves.
        let making = matches!(
            self.gallery.similar.progress(),
            Some((Phase::Signatures, _, _))
        );
        let a = match combo.key {
            Key::Named(NamedKey::Escape) if making && !reviewing => Some(Act::Stop),
            Key::Named(NamedKey::Escape) => Some(Act::Back),
            Key::Named(NamedKey::Tab) => Some(Act::Tab(match v.cat {
                Category::Duplicates => Category::Similar,
                Category::Similar => Category::Duplicates,
            })),
            Key::Named(NamedKey::Enter) if !reviewing => Some(Act::Open(v.cursor)),
            Key::Named(NamedKey::Enter) => Some(Act::Compare),
            Key::Named(NamedKey::Space) => Some(Act::AcceptNext),
            Key::Named(NamedKey::ArrowDown) if !reviewing => {
                self.similar_cursor(v.cursor as isize + 1, n_open);
                None
            }
            Key::Named(NamedKey::ArrowUp) if !reviewing => {
                self.similar_cursor(v.cursor as isize - 1, n_open);
                None
            }
            Key::Named(NamedKey::PageDown) if !reviewing => {
                self.similar_cursor(v.cursor as isize + 5, n_open);
                None
            }
            Key::Named(NamedKey::PageUp) if !reviewing => {
                self.similar_cursor(v.cursor as isize - 5, n_open);
                None
            }
            // The sensitivity slider (similar shots' list): the arrows move its knob the way
            // they point, strict at the leading end (on the right for right-to-left layouts).
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowLeft)
                if !reviewing && v.cat == Category::Similar =>
            {
                let right = matches!(combo.key, Key::Named(NamedKey::ArrowRight));
                let s = self.gallery.similar.sens + slider_key_step(right, self.gallery.rtl);
                self.similar_sensitivity(s);
                None
            }
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowLeft) if reviewing => {
                let right =
                    matches!(combo.key, Key::Named(NamedKey::ArrowRight)) != self.gallery.rtl;
                let n = self.similar_reviewed().map_or(0, |(c, gi)| {
                    self.gallery.similar.groups(c)[gi].members.len()
                });
                let f = v.review.as_ref().map_or(0, |r| r.focus) as isize;
                let f = (f + if right { 1 } else { -1 }).clamp(0, n as isize - 1);
                Some(Act::Focus(f.max(0) as usize))
            }
            Key::Char(c) => match c.to_ascii_lowercase() {
                'p' if reviewing => Some(Act::Set(Decision::Keep)),
                'x' if reviewing => Some(Act::Set(Decision::Discard)),
                'u' if reviewing => Some(Act::Set(Decision::Undecided)),
                'n' if reviewing => Some(Act::Split),
                's' if reviewing => Some(Act::Skip),
                'c' if reviewing => Some(Act::Compare),
                'z' if reviewing => Some(Act::Zoom),
                'a' if !reviewing => Some(Act::AcceptAll),
                'k' if !reviewing => Some(Act::Unskip),
                'd' => Some(Act::ActionMenu),
                '[' | ']' if v.cat == Category::Similar => {
                    let s = self.gallery.similar.sens + bracket_step(c, self.gallery.rtl);
                    self.similar_sensitivity(s);
                    None
                }
                '+' | '=' if reviewing => {
                    self.similar_zoom_by(1.25);
                    None
                }
                '-' if reviewing => {
                    self.similar_zoom_by(0.8);
                    None
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(a) = a {
            self.similar_act(a);
        }
        self.redraw = true;
        true
    }

    fn similar_cursor(&mut self, to: isize, n: usize) {
        if let Some(v) = self.gallery.similar.view.as_mut() {
            v.cursor = to.clamp(0, n.saturating_sub(1) as isize) as usize;
        }
    }

    fn similar_zoom_by(&mut self, f: f32) {
        if let Some(r) = self
            .gallery
            .similar
            .view
            .as_mut()
            .and_then(|v| v.review.as_mut())
        {
            r.zoom = (r.zoom * f).clamp(1.0, 16.0);
            if r.zoom > 1.0 && r.compare.is_none() {
                r.compare = Some(vec![r.focus]);
            }
        }
    }

    fn similar_sensitivity(&mut self, s: f32) {
        let s = (s * 100.0).round() / 100.0;
        let s = s.clamp(0.0, 1.0);
        let ga = &mut self.gallery;
        if (ga.similar.sens - s).abs() < 1e-4 {
            return;
        }
        ga.similar.sens = s;
        if let Some(st) = &ga.similar.store
            && let Ok(mut st) = st.lock()
        {
            st.sensitivity = s;
            st.options_changed();
        }
        ga.similar.regroup(&ga.marks);
        ga.dirty |= ga.stacks_on;
        self.similar_fix_view();
        self.redraw = true;
    }

    /// A mouse button while a category is open.
    pub(super) fn similar_button(&mut self, down: bool, b: Button, p: (f32, f32)) {
        let now = self.now();
        if !down {
            if let Some(v) = self.gallery.similar.view.as_mut() {
                v.slider_drag = false;
            }
            return;
        }
        if b != Button::Primary {
            if b == Button::Secondary {
                self.similar_act(Act::Back);
            }
            return;
        }
        let hit = self
            .gallery
            .similar
            .hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(p))
            .map(|(r, a)| (*r, *a));
        let Some((r, a)) = hit else { return };
        let double = self
            .gallery
            .similar
            .last_click
            .is_some_and(|(t, x)| x == a && now - t < 0.4);
        self.gallery.similar.last_click = Some((now, a));
        match a {
            Act::Slider => {
                if let Some(v) = self.gallery.similar.view.as_mut() {
                    v.slider_drag = true;
                }
                let track = self.gallery.similar.hits_slider.map_or(r, |(t, _)| t);
                let s = self.gallery.similar.slider_at(p.0, track);
                self.similar_sensitivity(s);
            }
            Act::Focus(k) if double => {
                self.similar_act(Act::Focus(k));
                self.similar_act(Act::Compare);
            }
            Act::Focus(k) => {
                // A click in a zoomed pane moves the shared focus point there.
                let zoomed = self
                    .gallery
                    .similar
                    .view
                    .as_ref()
                    .and_then(|v| v.review.as_ref())
                    .is_some_and(|r| r.zoom > 1.0);
                if zoomed && self.gallery.similar.panes.contains(&r) {
                    if let Some(rv) = self
                        .gallery
                        .similar
                        .view
                        .as_mut()
                        .and_then(|v| v.review.as_mut())
                    {
                        let (dx, dy) = ((p.0 - r.x) / r.w - 0.5, (p.1 - r.y) / r.h - 0.5);
                        rv.at.0 = (rv.at.0 + dx / rv.zoom).clamp(0.0, 1.0);
                        rv.at.1 = (rv.at.1 + dy / rv.zoom).clamp(0.0, 1.0);
                    }
                    self.redraw = true;
                } else {
                    self.similar_act(Act::Focus(k));
                }
            }
            a => self.similar_act(a),
        }
    }

    /// The wheel while a category is open: scrolls the list, zooms the panes.
    pub(super) fn similar_wheel(&mut self, lines: (f32, f32), pixels: bool, p: (f32, f32)) {
        let reviewing = self
            .gallery
            .similar
            .view
            .as_ref()
            .is_some_and(|v| v.review.is_some());
        let dy = if pixels { lines.1 } else { lines.1 * 40.0 };
        if reviewing {
            if dy.abs() > 0.0 {
                let at = self
                    .gallery
                    .similar
                    .hits
                    .iter()
                    .find(|(r, a)| {
                        matches!(a, Act::Focus(_))
                            && self.gallery.similar.panes.contains(r)
                            && r.contains(p)
                    })
                    .map(|(r, _)| ((p.0 - r.x) / r.w, (p.1 - r.y) / r.h));
                if let Some(r) = self
                    .gallery
                    .similar
                    .view
                    .as_mut()
                    .and_then(|v| v.review.as_mut())
                    && r.zoom <= 1.0
                    && let Some(at) = at
                {
                    r.at = at;
                }
                self.similar_zoom_by(if dy > 0.0 { 1.15 } else { 1.0 / 1.15 });
            }
        } else if let Some(v) = self.gallery.similar.view.as_mut() {
            v.scroll = (v.scroll - dy).max(0.0);
        }
        self.redraw = true;
    }

    /// `--show gallery-dupes…` / `gallery-similar…`: the screen to show once scanned.
    pub(super) fn similar_show(&mut self, rest: &str) -> bool {
        let known = [
            "-dupes",
            "-dupes-review",
            "-dupes-confirm",
            "-similar",
            "-similar-review",
            "-similar-compare",
            "-similar-zoom",
            "-similar-stacks",
            "-similar-filter",
            "-similar-menu",
            "-similar-scanning",
            "-similar-stopped",
        ];
        if !known.contains(&rest) {
            return false;
        }
        self.similar_store();
        if rest == "-similar-scanning" || rest == "-similar-stopped" {
            // The list while the scan runs (its Cancel), or stopped as soon as it began.
            self.similar_open(Category::Similar);
            self.gallery.similar.show_stop = rest == "-similar-stopped";
            return true;
        }
        self.gallery.similar.show = Some(rest.to_owned());
        true
    }

    fn similar_apply_show(&mut self, s: &str) {
        let cat = if s.starts_with("-dupes") {
            Category::Duplicates
        } else {
            Category::Similar
        };
        match s {
            "-similar-stacks" => {
                self.gallery.stacks_on = true;
                self.gallery.dirty = true;
            }
            "-similar-filter" => {
                self.gallery.filter_open = true;
                self.gallery.filter.text = "similar:".into();
                self.gallery.dirty = true;
            }
            "-similar-menu" => self.gallery_menu_view(None),
            _ => {
                self.similar_open(cat);
                let ga = &mut self.gallery;
                ga.similar.regroup(&ga.marks);
                if s.ends_with("-review") || s.ends_with("-compare") || s.ends_with("-zoom") {
                    self.similar_act(Act::Open(0));
                    // Show a decision made by hand beside the suggestions.
                    if s.ends_with("-compare") || s.ends_with("-zoom") {
                        self.similar_act(Act::Compare);
                    }
                    // The compare at 100 % about the images' centre, synchronised.
                    if s.ends_with("-zoom") {
                        self.gallery.pointer = None;
                        self.similar_act(Act::Zoom);
                    }
                }
                if s.ends_with("-confirm") {
                    self.similar_confirm_all();
                }
            }
        }
    }

    /// AccessKit: an action on one of our nodes (an option, a control); false when not ours.
    /// A control does what a click on it does, once it is on screen and not disabled; the
    /// slider takes Increment, Decrement and SetValue (in percent).
    pub(in crate::app) fn similar_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        use accesskit::{Action, ActionData};
        let base = A11Y_BASE + A11Y_OFFSET;
        let target = req.target_node.0;
        if !self.gallery.active || self.gallery.similar.view.is_none() || target < base {
            return false;
        }
        let k = (target - base) as usize;
        if target - base >= A11Y_CONTROLS {
            let a = usize::try_from(target - base - A11Y_CONTROLS)
                .ok()
                .and_then(|i| CONTROLS.get(i).copied());
            let shown = a.filter(|a| {
                self.gallery.similar.hits.iter().any(|h| h.1 == *a)
                    && self.gallery.similar_disabled(*a).is_none()
            });
            match (shown, req.action) {
                (Some(Act::Slider), Action::Increment | Action::Decrement) => {
                    let up = req.action == Action::Increment;
                    let s = self.gallery.similar.sens + slider_key_step(up, false);
                    self.similar_sensitivity(s);
                }
                (Some(Act::Slider), Action::SetValue) => {
                    if let Some(ActionData::NumericValue(n)) = req.data {
                        self.similar_sensitivity(n as f32 / 100.0);
                    }
                }
                (Some(Act::Slider), _) => {}
                (Some(a), Action::Click) => self.similar_act(a),
                _ => {}
            }
            self.redraw = true;
            return true;
        }
        if k == 0 || !matches!(req.action, Action::Click | Action::Focus) {
            return true;
        }
        let click = req.action == Action::Click;
        let k = k - 1;
        let reviewing = self
            .gallery
            .similar
            .view
            .as_ref()
            .is_some_and(|v| v.review.is_some());
        if reviewing {
            self.similar_act(if click { Act::Toggle(k) } else { Act::Focus(k) });
        } else if click {
            self.similar_act(Act::Open(k));
        } else {
            let n = self
                .gallery
                .similar
                .view
                .as_ref()
                .map_or(0, |v| self.gallery.similar.open_groups(v.cat).len());
            self.similar_cursor(k as isize, n);
        }
        self.redraw = true;
        true
    }
}

/// AccessKit ids of the review screens (beside the grid's).
const A11Y_OFFSET: u64 = 1 << 40;

// ----- Drawing ----------------------------------------------------------------------------

/// The review's large images: what each shows (its signature path, and its main file for
/// a RAW + JPEG pair), the cell it fits in, and the zoom over fit and its centre shared by
/// every image.
pub(super) struct ReviewPanes {
    pub(super) members: Vec<(PathBuf, PathBuf, Rect)>,
    pub(super) zoom: f32,
    pub(super) at: (f32, f32),
}

/// The cells of `n` large images in `main`, side by side or a 2×2 grid for four in a
/// narrow window, each above its caption line (`cap` high); in the reading direction (the
/// first on the right when `rtl`).
fn review_cells(main: Rect, n: usize, cap: f32, rtl: bool) -> Vec<Rect> {
    let gap = 12.0;
    let (cols, rows) = if n == 4 && main.w < main.h * 2.0 {
        (2, 2)
    } else {
        (n, 1)
    };
    let cw = (main.w - gap * (cols - 1) as f32) / cols as f32;
    let ch = (main.h - gap * (rows - 1) as f32) / rows as f32;
    (0..n)
        .map(|slot| {
            let col = slot % cols;
            let col = if rtl { cols - 1 - col } else { col };
            Rect::new(
                main.x + col as f32 * (cw + gap),
                main.y + (slot / cols) as f32 * (ch + gap),
                cw,
                (ch - cap).max(1.0),
            )
        })
        .collect()
}

/// What → (`right`) or ← adds to the sensitivity: the knob moves the way the arrow points,
/// towards loose on the trailing side (the left in right-to-left layouts).
fn slider_key_step(right: bool, rtl: bool) -> f32 {
    if right != rtl { 0.05 } else { -0.05 }
}

/// What `[` or `]` adds to the sensitivity: as ← and →, the knob moves the way the bracket
/// points (`]` right: towards loose left to right, towards strict right to left).
fn bracket_step(c: char, rtl: bool) -> f32 {
    slider_key_step(c == ']', rtl)
}

fn fit_in(w: f32, h: f32, r: Rect) -> Rect {
    let s = (r.w / w.max(1.0)).min(r.h / h.max(1.0));
    r.centered(w * s, h * s)
}

/// `uv` cropped to fill a square-ish `r` (centre crop).
fn crop(p: &Placed, r: Rect) -> [f32; 4] {
    let [u, v, uw, vh] = p.uv;
    let ia = p.width.max(1) as f32 / p.height.max(1) as f32;
    let ta = r.w.max(1.0) / r.h.max(1.0);
    if ia > ta {
        let nw = uw * ta / ia;
        [u + (uw - nw) * 0.5, v, nw, vh]
    } else {
        let nh = vh * ia / ta;
        [u, v + (vh - nh) * 0.5, uw, nh]
    }
}

/// The part of `p` that shows at `zoom` about `at` (normalised), clamped to the image.
fn zoom_uv(p: &Placed, zoom: f32, at: (f32, f32)) -> [f32; 4] {
    let [u, v, uw, vh] = p.uv;
    let z = zoom.max(1.0);
    let half = 0.5 / z;
    let cx = at.0.clamp(half, 1.0 - half);
    let cy = at.1.clamp(half, 1.0 - half);
    [u + uw * (cx - half), v + vh * (cy - half), uw / z, vh / z]
}

fn ellipsize(text: &mut Text, s: &str, style: TextStyle, w: f32) -> String {
    ellipsize_by(&mut |t| text.measure(t, style), s, w)
}

/// `s` cut to `w` (as `measure` measures it) with an ellipsis.
fn ellipsize_by(measure: &mut dyn FnMut(&str) -> f32, s: &str, w: f32) -> String {
    if measure(s) <= w {
        return s.to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let c = specular_i18n::balance_isolates(&(chars[..mid].iter().collect::<String>() + "…"));
        if measure(&c) <= w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    specular_i18n::balance_isolates(&(chars[..lo].iter().collect::<String>() + "…"))
}

/// Separates the header's summary from the focus pass's progress.
const HEADER_SEP: &str = "  ·  ";

/// The header line in `w`: the summary, then (after [`HEADER_SEP`]) the focus pass's
/// progress, which narrow windows keep whole: the summary is cut first, and left out when
/// fewer than a few characters of it would show. Returns the two parts as drawn, the
/// separator starting the second.
fn header_line(
    measure: &mut dyn FnMut(&str) -> f32,
    summary: &str,
    progress: &str,
    w: f32,
) -> (String, String) {
    if progress.is_empty() {
        return (ellipsize_by(measure, summary, w), String::new());
    }
    if summary.is_empty() {
        return (String::new(), ellipsize_by(measure, progress, w));
    }
    let tail = format!("{HEADER_SEP}{progress}");
    let room = w - measure(&tail);
    // Below this, a cut summary says nothing ("Sim…"): the progress alone.
    if room >= measure("0000000…") {
        (ellipsize_by(measure, summary, room), tail)
    } else {
        (String::new(), ellipsize_by(measure, progress, w))
    }
}

/// The sensitivity slider's track: as wide as this when there is room, never narrower than
/// [`TRACK_MIN`] beside its label.
const TRACK_MAX: f32 = 240.0;
const TRACK_MIN: f32 = 96.0;
/// The slider's row under the header, and what a label on a line of its own adds.
const SLIDER_ROW: f32 = 44.0;
const SLIDER_STACK: f32 = 22.0;

/// How the sensitivity slider's row fits a view `w` wide, from the widths of its label
/// ("Sensitivity"), its ends ("Strict", "Loose") and its widest value ("100").
#[derive(Clone, Copy, Debug, PartialEq)]
struct SliderLayout {
    /// The label sits on a line of its own above the slider: beside it the track would be
    /// narrower than [`TRACK_MIN`].
    stacked: bool,
    /// The track's width: what is left of the line, at most [`TRACK_MAX`].
    track: f32,
}

impl SliderLayout {
    /// The row's height under the header at metrics `m`.
    fn height(self, m: Metrics) -> f32 {
        m.u(SLIDER_ROW) + if self.stacked { m.u(SLIDER_STACK) } else { 0.0 }
    }
}

/// The track shrinks with the window down to [`TRACK_MIN`]; narrower still, the label moves
/// above the slider, whose track then takes the whole line (it never runs past the window
/// unless even that line is too short for the ends' names).
fn slider_layout(w: f32, label: f32, strict: f32, loose: f32, value: f32) -> SliderLayout {
    // 18 px margins; "Strict" 12 [track] 12 "Loose" 12 value.
    let line = w - 36.0 - (strict + 12.0 + 12.0 + loose + 12.0 + value);
    let beside = line - label - 16.0;
    if beside >= TRACK_MIN {
        SliderLayout {
            stacked: false,
            track: beside.min(TRACK_MAX),
        }
    } else {
        SliderLayout {
            stacked: true,
            track: line.clamp(24.0, TRACK_MAX),
        }
    }
}

pub(super) fn put_centered(
    text: &mut Text,
    dl: &mut DrawList,
    s: &str,
    style: TextStyle,
    r: Rect,
    color: Color,
) {
    let b = text.layout(s, style, None, Align::Start);
    text.draw(
        dl,
        &b,
        r.x + (r.w - b.width) * 0.5,
        r.y + (r.h - b.height) * 0.5,
        color,
    );
}

impl Gallery {
    /// The covers of the stacks on screen or under the pointer, while the grid shows with
    /// stacks on: the tiles whose cover must not change under the user.
    pub(super) fn stack_covers_in_view(&self) -> Vec<PathBuf> {
        if !self.active || !self.stacks_on || self.similar.is_open() || self.tiles.is_empty() {
            return Vec::new();
        }
        let shown = self
            .layout
            .items_in(self.scroll, self.scroll + self.grid_h());
        let hover = self.hover.map(|(i, _)| i).filter(|i| !shown.contains(i));
        let mut out: Vec<PathBuf> = Vec::new();
        for i in shown.chain(hover) {
            let Some(t) = self.tiles.get(i).filter(|t| !t.members.is_empty()) else {
                continue;
            };
            // An expanded stack shows every member; its cover is the one it was opened by.
            let cover = t
                .members
                .iter()
                .filter_map(|&m| self.entries.get(m))
                .map(|e| &e.path)
                .find(|p| self.expanded.contains(*p))
                .or_else(|| self.entries.get(t.entry).map(|e| &e.path));
            if let Some(c) = cover
                && !out.contains(c)
            {
                out.push(c.clone());
            }
        }
        out
    }

    /// `s` at `(x, y)` given left to right: mirrored in right-to-left layouts, where the
    /// text ends at the mirror of `x` and reads as a right-to-left paragraph. Returns its
    /// width.
    fn similar_put(
        &self,
        text: &mut Text,
        dl: &mut DrawList,
        s: &str,
        style: TextStyle,
        (x, y): (f32, f32),
        color: Color,
    ) -> f32 {
        let s = self.similar_bidi(s);
        let b = text.layout(&s, style, None, Align::Start);
        let x = if self.rtl { self.vw - x - b.width } else { x };
        text.draw(dl, &b, x, y, color);
        b.width
    }

    /// A member's caption after its name: its size, the zoom when it shows zoomed in the
    /// large image `pane` (as drawn), and the channel view (the viewer's channel badge is not
    /// drawn in the review). `isolate`: the size as a left-to-right isolate, for drawing
    /// (screen readers get it without: the adapter strips Fluent's marks, not this one).
    fn member_details(
        &self,
        mb: u32,
        rv: &Review,
        pane: Option<Rect>,
        isolate: bool,
    ) -> Vec<String> {
        let it = self
            .similar
            .results
            .as_ref()
            .and_then(|r| r.items.get(mb as usize));
        let mut parts = Vec::new();
        if let Some(it) = it
            && it.width > 0
        {
            // Width × height reads left to right in every language.
            let size = format!("{} × {}", it.width, it.height);
            parts.push(if isolate {
                specular_i18n::isolate_ltr(&size)
            } else {
                size
            });
        }
        if let Some(r) = pane
            && rv.zoom > 1.0
        {
            let ratio = rv.zoom * r.w * self.scale / it.map_or(1.0, |i| i.width.max(1) as f32);
            parts.push(
                specular_i18n::I18n::global()
                    .localizer()
                    .percent(f64::from(ratio), 0),
            );
        }
        let channel = self.similar.path_of(mb).and_then(|path| {
            self.pane_tex(path).or_else(|| {
                let main = self.similar.results.as_ref()?.units.get(mb as usize)?;
                self.pane_tex(&main.main)
            })
        });
        if let Some((_, _, c)) = channel.filter(|p| p.2 != ChannelView::Normal) {
            parts.push(specular_widgets::channel_label(c));
        }
        parts
    }

    /// A line put together here (a file name and a reason, a caption) as a right-to-left
    /// paragraph in right-to-left layouts, so its parts follow the reading direction even
    /// when it starts with a Latin file name or a number.
    fn similar_bidi(&self, s: &str) -> String {
        if self.rtl && specular_i18n::base_direction(s) != Some(specular_i18n::Direction::Rtl) {
            format!("{}{}", specular_i18n::RLM, specular_i18n::resolve_fsi(s))
        } else {
            s.to_owned()
        }
    }

    /// A button at `(x, y)` given left to right (mirrored in right-to-left layouts); returns
    /// its width.
    fn similar_button_draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        (x, y): (f32, f32),
        label: &str,
        (act, primary): (Act, bool),
    ) -> f32 {
        let w = text.measure(label, self.ts(BODY)) + 24.0;
        let r = self.flip(Rect::new(x, y, w, self.u(32.0)));
        self.similar_button_in(dl, text, pal, r, label, (act, primary));
        w
    }

    /// A button drawn in `r` (as drawn, not mirrored).
    fn similar_button_in(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        r: Rect,
        label: &str,
        (act, primary): (Act, bool),
    ) {
        let style = self.ts(BODY);
        let hover = self.pointer.is_some_and(|p| r.contains(p));
        // Unavailable (`similar_disabled`): dimmed, no hover, as in the viewer's toolbar.
        let (fill, fg) = if self.similar_disabled(act).is_some() {
            (pal.surface_raised, pal.text_faint)
        } else if primary {
            (pal.accent_fill, pal.accent_text)
        } else if hover {
            (pal.surface_hover, pal.text)
        } else {
            (pal.surface_raised, pal.text)
        };
        dl.rect(r, 8.0, fill);
        put_centered(text, dl, label, style, r, fg);
        self.similar.hits.push((r, act));
    }

    /// A tooltip with `s` centred on `cx` (as drawn), its bottom 6 px above `edge` when
    /// `above`, else its top 6 px below; inside the window (by 8 px at the sides).
    fn similar_tip(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        s: &str,
        (cx, edge): (f32, f32),
        above: bool,
    ) {
        let (w, h) = (self.vw, self.vh);
        let s = self.similar_bidi(s);
        let b = text.layout(&s, self.ts(SMALL), None, Align::Start);
        let (tw, th) = (b.width + 16.0, b.height + 8.0);
        let x = (cx - tw * 0.5).clamp(8.0, (w - 8.0 - tw).max(8.0));
        let y = if above { edge - th - 6.0 } else { edge + 6.0 };
        let tr = Rect::new(x, y.clamp(4.0, (h - 4.0 - th).max(4.0)), tw, th);
        dl.rect(tr, 6.0, pal.tooltip_bg);
        text.draw(dl, &b, tr.x + 8.0, tr.y + 4.0, pal.tooltip_text);
        self.similar.tip = Some((tr, s));
    }

    /// The open category over the grid: the group list, or the review of one group. True:
    /// the review's layout changed with this frame's text (draw again, so the large images
    /// rendered before drawing follow it).
    pub(super) fn draw_similar(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) -> bool {
        let Some(v) = self.similar.view.clone() else {
            return false;
        };
        // The view's layout outside drawing (review cells, slider hits) follows the gallery's.
        self.similar.rtl = self.rtl;
        self.similar.text_scale = self.text_scale;
        self.similar.hits.clear();
        self.similar.tip = None;
        let pal = f.pal;
        let (w, h) = (self.vw, self.vh);
        dl.rect(Rect::new(0.0, 0.0, w, h), 0.0, pal.canvas);
        // Slider drag: follows the pointer (the release ends it).
        if v.slider_drag
            && let Some(p) = self.pointer
            && let Some((r, _)) = self.similar.hits_slider
        {
            let s = self.similar.slider_at(p.0, r);
            let s = (s * 100.0).round() / 100.0;
            if (s - self.similar.sens).abs() > 1e-3 {
                self.similar.sens = s;
                if let Some(st) = &self.similar.store
                    && let Ok(mut st) = st.lock()
                {
                    st.sensitivity = s;
                    st.options_changed();
                }
                self.similar.regroup(&self.marks);
                self.dirty |= self.stacks_on;
            }
        }
        match v.review.as_ref() {
            Some(r) => {
                self.similar.hover_group = None;
                self.similar.wish(&[]);
                self.draw_similar_review(dl, text, pal, &v, r)
            }
            None => {
                self.draw_similar_list(dl, text, pal, &v);
                false
            }
        }
    }

    /// The header: category tabs, the summary and progress, and Done. Returns where the tabs
    /// end (from the leading edge, as laid out left to right).
    fn draw_similar_header(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        v: &View,
    ) -> f32 {
        let w = self.vw;
        let m = self.similar.metrics();
        let (title, body, small) = (self.ts(TITLE), self.ts(BODY), self.ts(SMALL));
        dl.rect(Rect::new(0.0, 0.0, w, m.header()), 0.0, pal.surface);
        dl.rect(Rect::new(0.0, m.header() - 1.0, w, 1.0), 0.0, pal.separator);
        let mut x = 16.0;
        for c in [Category::Duplicates, Category::Similar] {
            let label = cat_name(c);
            let tw = text.measure(&label, title);
            let r = self.flip(Rect::new(x, m.u(10.0), tw + 20.0, m.u(30.0)));
            if c == v.cat {
                dl.rect(r, 8.0, pal.accent_soft);
            }
            put_centered(
                text,
                dl,
                &label,
                title,
                r,
                if c == v.cat { pal.text } else { pal.text_muted },
            );
            self.similar.hits.push((r, Act::Tab(c)));
            x += r.w + 8.0;
        }
        // Summary under the tabs.
        // While the focus pass refines the picks, the groups show with it.
        let progress = self.similar.progress();
        let stopped = self.similar.stopped_phase();
        let line = match (progress, stopped) {
            (Some((Phase::Signatures, d, t)), _) => progress_text(Phase::Signatures, d, t),
            (None, Some(Phase::Signatures)) => t!("similar-stopped"),
            _ => self.similar.summary_line(v.cat),
        };
        let checking = match (progress, stopped) {
            (Some((Phase::Focus, d, t)), _) => progress_text(Phase::Focus, d, t),
            (None, Some(Phase::Focus)) => t!("similar-stopped-sharpness"),
            _ => String::new(),
        };
        // The progress stays whole in narrow windows; the summary is cut first.
        let (line, checking) = header_line(
            &mut |s| text.measure(s, small),
            &line,
            &checking,
            (w - 32.0).max(0.0),
        );
        let y = m.u(44.0);
        let lw = self.similar_put(text, dl, &line, small, (18.0, y), pal.text_muted);
        if !checking.is_empty() {
            self.similar_put(text, dl, &checking, small, (18.0 + lw, y), pal.text_muted);
        }
        // Done.
        let label = t!("similar-done");
        let bw = text.measure(&label, body) + 24.0;
        self.similar_button_draw(
            dl,
            text,
            pal,
            (w - bw - 16.0, m.u(14.0)),
            &label,
            (Act::Close, false),
        );
        // Before it in the list: Cancel while scanning, "Scan again" once stopped.
        // Cancel says its key while Esc stops the scan (the signatures), as Back does.
        let scan_button = if v.review.is_some() {
            None
        } else if let Some((phase, _, _)) = progress {
            let l = t!("similar-cancel");
            let labels = if phase == Phase::Signatures {
                vec![format!("{l}  (Esc)"), l]
            } else {
                vec![l]
            };
            Some((Act::Stop, labels))
        } else if stopped.is_some() {
            Some((Act::Rescan, vec![t!("similar-scan-again")]))
        } else {
            None
        };
        // Only where it does not cover the tabs (narrow windows: Esc still stops the
        // signatures, and the sharpness check runs on behind usable groups); without its key
        // first.
        let scan_button = scan_button.and_then(|(a, labels)| {
            labels
                .into_iter()
                .map(|l| (a, text.measure(&l, body) + 24.0, l))
                .find(|(_, cw, _)| w - bw - 16.0 - 8.0 - cw >= x)
                .map(|(a, cw, l)| (a, l, cw))
        });
        if let Some((act, label, cw)) = scan_button {
            self.similar_button_draw(
                dl,
                text,
                pal,
                (w - bw - 16.0 - 8.0 - cw, m.u(14.0)),
                &label,
                (act, false),
            );
        }
        x
    }

    fn draw_similar_list(&mut self, dl: &mut DrawList, text: &mut Text, pal: &Palette, v: &View) {
        let (w, h) = (self.vw, self.vh);
        let m = self.similar.metrics();
        let (body, small) = (self.ts(BODY), self.ts(SMALL));
        let strong = self.ts(BODY.weight(600.0));
        self.draw_similar_header(dl, text, pal, v);
        let open = self.similar.open_groups(v.cat);
        let top = m.header()
            + if v.cat == Category::Similar {
                self.draw_similar_slider(dl, text, pal, m.header() + m.u(6.0))
            } else {
                0.0
            };
        // The footer's buttons wrap like the review's (`footer_rows`); its padding tightens
        // when the list would not get one whole row.
        let buttons = self.similar_list_buttons(v, !open.is_empty());
        let widths: Vec<(f32, f32)> = buttons
            .iter()
            .map(|(full, label, _, _)| {
                (
                    text.measure(full, body) + 24.0,
                    text.measure(label, body) + 24.0,
                )
            })
            .collect();
        let foot = footer_rows(&widths, w);
        let (mut foot_h, mut foot_pad) = footer_block(foot.rows.len(), m, false);
        if h - top - foot_h < m.row() {
            (foot_h, foot_pad) = footer_block(foot.rows.len(), m, true);
        }
        let area = Rect::new(0.0, top, w, (h - top - foot_h).max(0.0));
        let class = specular_thumbs::class_for(ROW_THUMB * self.scale);
        let mut wanted = Vec::new();
        self.similar.hover_group = None;
        if open.is_empty() {
            self.similar.wish(&[]);
            let msg = if self.similar.results.is_none() && self.similar.stopped_phase().is_some() {
                t!("similar-stopped-empty")
            } else if self.similar.results.is_none() {
                // Scanning (its progress is in the header): what will show here.
                t!("similar-waiting")
            } else if self.similar.info(v.cat).is_empty() {
                match v.cat {
                    Category::Duplicates => t!("similar-empty-duplicates"),
                    Category::Similar => t!("similar-empty-similar"),
                }
            } else {
                t!("similar-all-done")
            };
            put_centered(text, dl, &msg, body, area, pal.text_muted);
        } else {
            // Rows grow with the text; the thumbnails keep their size.
            let row_h = m.row();
            // Keep the cursor row in view; its top when it is taller than the list (large
            // text in a short window).
            let rows_h = open.len() as f32 * row_h;
            let mut scroll = v.scroll.min((rows_h - area.h).max(0.0));
            let cy = v.cursor as f32 * row_h;
            if cy < scroll {
                scroll = cy;
            } else if cy + row_h > scroll + area.h {
                scroll = (cy + row_h - area.h).min(cy);
            }
            if let Some(vm) = self.similar.view.as_mut() {
                vm.scroll = scroll;
            }
            dl.push_clip(area);
            let first = (scroll / row_h) as usize;
            let last = (((scroll + area.h) / row_h).ceil() as usize).min(open.len());
            // The focus pass measures the rows on screen first.
            self.similar.wish(open.get(first..last).unwrap_or_default());
            let mut atlas = self.atlas.take();
            for (row, &gi) in open.iter().enumerate().take(last).skip(first) {
                let y = area.y + row as f32 * row_h - scroll;
                // Laid out left to right (the row itself is symmetric); its contents are
                // drawn through `flip` and `similar_put`.
                let rr = Rect::new(12.0, y + 4.0, w - 24.0, row_h - 8.0);
                let focused = row == v.cursor;
                let hover = self
                    .pointer
                    .is_some_and(|p| rr.contains(p) && area.contains(p));
                if hover {
                    self.similar.hover_group = self.similar.info(v.cat).get(gi).map(|s| s.key);
                }
                dl.rect(
                    rr,
                    10.0,
                    if focused {
                        pal.selection
                    } else if hover {
                        pal.surface_hover
                    } else {
                        pal.surface
                    },
                );
                if focused {
                    dl.stroke_rect(rr, 10.0, pal.focus_ring, 2.0);
                }
                self.similar
                    .hits
                    .push((rr.intersect(&area), Act::Open(row)));
                let g = self.similar.groups(v.cat)[gi].clone();
                let mut x = rr.x + 12.0;
                let max_thumbs = (((rr.w * 0.62) / (ROW_THUMB + 8.0)) as usize).max(1);
                for (k, (&mb, p)) in g.members.iter().zip(&g.picks).enumerate() {
                    if k >= max_thumbs {
                        let more = format!("+{}", g.members.len() - k);
                        self.similar_put(
                            text,
                            dl,
                            &more,
                            body,
                            (x + 4.0, rr.y + rr.h * 0.5 - m.u(8.0)),
                            pal.text_muted,
                        );
                        break;
                    }
                    let tr = self.flip(Rect::new(
                        x,
                        rr.y + (rr.h - ROW_THUMB) * 0.5,
                        ROW_THUMB,
                        ROW_THUMB,
                    ));
                    let Some(path) = self.similar.path_of(mb).map(Path::to_path_buf) else {
                        continue;
                    };
                    wanted.push((path.clone(), class));
                    match atlas.as_mut().and_then(|a| a.best(&path, class)) {
                        Some(pl) => dl.image_over(tr, crop(&pl, tr), pl.tex, 6.0, self.backdrop),
                        None => {
                            let c =
                                self.averages
                                    .get(path.as_path())
                                    .map_or(pal.surface_raised, |a| {
                                        Color::rgba(
                                            f32::from(a[0]) / 255.0,
                                            f32::from(a[1]) / 255.0,
                                            f32::from(a[2]) / 255.0,
                                            1.0,
                                        )
                                    });
                            dl.rect(tr, 6.0, c);
                        }
                    }
                    let keep = self.similar.keeps(v.cat, mb, p);
                    self.draw_badge(
                        dl,
                        tr,
                        keep,
                        self.similar.decision(v.cat, mb) != Decision::Undecided,
                    );
                    x += ROW_THUMB + 8.0;
                }
                // Text: count, size, and why the first is kept; the lines centred in the row.
                let tx = (rr.x
                    + 12.0
                    + max_thumbs.min(g.members.len()) as f32 * (ROW_THUMB + 8.0)
                    + 24.0)
                    .min(rr.right() - m.u(120.0));
                let tw = (rr.right() - tx - 12.0).max(0.0);
                let ty = rr.y + ((rr.h - m.u(ROW_TEXT)) * 0.5).round() + 2.0;
                let secondary = row_secondary(pal, focused || hover);
                let reclaim = self.similar.reclaim(v.cat, &g);
                let l1 = specular_i18n::I18n::global().join(
                    &[
                        t!("similar-group-photos", count = g.members.len() as i64),
                        t!("similar-to-free", size = size_text(reclaim)),
                    ],
                    "  ·  ",
                );
                let l1 = ellipsize(text, &l1, strong, tw);
                self.similar_put(text, dl, &l1, strong, (tx, ty), pal.text);
                if let Some((&best, p)) =
                    lead(&g).and_then(|k| g.members.get(k).zip(g.picks.get(k)))
                {
                    let name = self
                        .similar
                        .results
                        .as_ref()
                        .map(|r| r.items[best as usize].name.clone())
                        .unwrap_or_default();
                    // The name keeps its own direction inside the line.
                    let l2 = format!("{}: {}", specular_i18n::isolate(&name), tr_reason(p.reason));
                    let l2 = ellipsize(text, &l2, small, tw);
                    self.similar_put(text, dl, &l2, small, (tx, ty + m.u(24.0)), secondary);
                }
                if let Some(p) = g.picks.iter().find(|p| !p.keep) {
                    let l3 = format!(
                        "{} {}",
                        t!("similar-discard"),
                        tr_reason(p.reason).to_lowercase()
                    );
                    let l3 = ellipsize(text, &l3, small, tw);
                    self.similar_put(text, dl, &l3, small, (tx, ty + m.u(44.0)), secondary);
                }
            }
            self.atlas = atlas;
            dl.pop_clip();
        }
        self.similar.set_wanted(wanted);
        // Footer: the buttons with their keys and the key hints on one row when they fit;
        // else without the keys (in a tooltip) and the hints, on more rows when needed.
        let fy = h - foot_h;
        dl.rect(Rect::new(0.0, fy, w, foot_h), 0.0, pal.surface);
        dl.rect(Rect::new(0.0, fy, w, 1.0), 0.0, pal.separator);
        let mut tip = None;
        let mut end = 16.0;
        for (row, items) in foot.rows.iter().enumerate() {
            let by = footer_row_y(fy, foot_pad, row, m);
            for &(i, x, bw) in items {
                let (full, label, primary, act) = &buttons[i];
                let shown = if foot.keys {
                    full.clone()
                } else {
                    ellipsize(text, label, body, bw - 24.0)
                };
                let r = self.flip(Rect::new(x, by, bw, m.u(32.0)));
                self.similar_button_in(dl, text, pal, r, &shown, (*act, *primary));
                if full != label && !foot.keys && self.pointer.is_some_and(|p| r.contains(p)) {
                    tip = Some((r, full.clone()));
                }
                end = x + bw + FOOTER_GAP;
            }
        }
        let hw = (w - end - 16.0).max(0.0);
        // No hints over an empty list: nothing to choose, review or accept.
        if foot.keys && hw > 80.0 && !open.is_empty() {
            let hint = ellipsize(text, &t!("similar-hint-list"), small, hw);
            let tw = text.measure(&hint, small);
            let ty = footer_row_y(fy, foot_pad, 0, m) + m.u(9.0);
            self.similar_put(text, dl, &hint, small, (w - 16.0 - tw, ty), pal.text_muted);
        }
        // The key of the footer button under the pointer, above the footer.
        if let Some((r, full)) = tip {
            self.similar_tip(dl, text, pal, &full, (r.center().0, fy), true);
        }
    }

    /// The group list's footer buttons: (label with its key, label, primary, action); Accept
    /// all with groups to review, Show skipped with skipped groups.
    fn similar_list_buttons(&self, v: &View, open: bool) -> Vec<(String, String, bool, Act)> {
        let mut b = Vec::new();
        if open {
            let l = t!("similar-accept-all");
            b.push((format!("{l}  (A)"), l, true, Act::AcceptAll));
        }
        let (a, folder) = self
            .similar
            .store
            .as_ref()
            .and_then(|s| s.lock().ok().map(|s| (s.discard, s.folder.clone())))
            .unwrap_or_default();
        let l = t!(
            "similar-discard-with",
            action = action_name(a, folder.as_deref())
        );
        b.push((l.clone(), l, false, Act::ActionMenu));
        if self
            .similar
            .info(v.cat)
            .iter()
            .any(|s| s.skipped && !s.resolved)
        {
            let l = t!("similar-show-skipped");
            b.push((l.clone(), l, false, Act::Unskip));
        }
        b
    }

    /// The sensitivity slider under the header at `y`; returns the height of its row (the
    /// label goes above the slider in narrow windows, [`slider_layout`]). Strict is at the
    /// leading end: on the right in right-to-left layouts, where the fill grows leftwards.
    fn draw_similar_slider(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        y: f32,
    ) -> f32 {
        let m = self.similar.metrics();
        let (body, small) = (self.ts(BODY), self.ts(SMALL));
        let label = t!("similar-sensitivity");
        let strict = t!("similar-sensitivity-strict");
        let loose = t!("similar-sensitivity-loose");
        let lay = slider_layout(
            self.vw,
            text.measure(&label, body),
            text.measure(&strict, small),
            text.measure(&loose, small),
            text.measure("100", small),
        );
        // The slider's line: beside the label, or under it.
        let (x0, y) = if lay.stacked {
            self.similar_put(text, dl, &label, body, (18.0, y + m.u(4.0)), pal.text);
            (18.0, y + m.u(SLIDER_STACK))
        } else {
            let lw = self.similar_put(text, dl, &label, body, (18.0, y + m.u(8.0)), pal.text);
            (18.0 + lw + 16.0, y)
        };
        let ends = y + m.u(9.0);
        let sw = self.similar_put(text, dl, &strict, small, (x0, ends), pal.text_muted);
        // Laid out left to right, drawn mirrored.
        let line = Rect::new(x0 + sw + 12.0, y + m.u(14.0), lay.track, 4.0);
        self.similar_put(
            text,
            dl,
            &loose,
            small,
            (line.right() + 12.0, ends),
            pal.text_muted,
        );
        let track = self.flip(line);
        dl.rect(track, 2.0, pal.surface_raised);
        let s = self.similar.sens;
        let fill = self.flip(Rect::new(line.x, line.y, line.w * s, line.h));
        dl.rect(fill, 2.0, pal.accent);
        let knob = if self.rtl {
            track.right() - track.w * s
        } else {
            track.x + track.w * s
        };
        dl.circle((knob, track.y + 2.0), 8.0, pal.accent);
        let hit = Rect::new(track.x - 8.0, y, track.w + 16.0, m.u(32.0));
        self.similar.hits.push((hit, Act::Slider));
        self.similar.hits_slider = Some((track, ()));
        let pct = specular_i18n::fixed(s * 100.0, 0);
        let px = line.right() + 24.0 + text.measure(&loose, small);
        self.similar_put(text, dl, &pct, small, (px, ends), pal.text_muted);
        lay.height(m)
    }

    /// A keep (✓) or discard (✕) badge in the corner of `r` (the top trailing corner: top
    /// left in right-to-left layouts); filled when decided. Sized with the thumbnail and
    /// drawn on whole device pixels ([`badge_rect`]).
    fn draw_badge(&self, dl: &mut DrawList, r: Rect, keep: bool, decided: bool) {
        if !keep {
            // Discards are dimmed a little.
            dl.rect(r, 6.0, Color::rgba(0.0, 0.0, 0.0, 0.18));
        }
        let c = Color::hex(if keep { KEEP_COLOR } else { DISCARD_COLOR });
        let b = badge_rect(r, self.scale, self.rtl);
        let d = b.w;
        // Strokes: about a tenth of the badge, at least 2 device px, whole device px.
        let sw = ((d * 0.1 * self.scale).round().max(2.0)) / self.scale;
        // Undecided: a dark disc with a coloured ring, so the badge reads on any photo.
        let mark = if decided {
            dl.rect(b, d * 0.5, c);
            Color::WHITE
        } else {
            dl.rect_border(b, d * 0.5, Color::rgba(0.0, 0.0, 0.0, 0.6), c, sw);
            c
        };
        // The glyph as strokes (no font has to carry ✓ / ✕), in badge-relative units.
        let at = |u: f32, v: f32| (b.x + u * d, b.y + v * d);
        if keep {
            dl.line(at(0.28, 0.52), at(0.43, 0.67), sw, mark);
            dl.line(at(0.43, 0.67), at(0.72, 0.34), sw, mark);
        } else {
            dl.line(at(0.33, 0.33), at(0.67, 0.67), sw, mark);
            dl.line(at(0.67, 0.33), at(0.33, 0.67), sw, mark);
        }
    }

    fn draw_similar_review(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        v: &View,
        rv: &Review,
    ) -> bool {
        let (w, h) = (self.vw, self.vh);
        let m = self.similar.metrics();
        let (body, small) = (self.ts(BODY), self.ts(SMALL));
        // The footer's buttons, with their keys and without: measured first, as the layout
        // depends on how many rows they take.
        let buttons = review_buttons();
        let widths: Vec<(f32, f32)> = buttons
            .iter()
            .map(|(label, key, _, _)| {
                (
                    text.measure(&format!("{label}  ({key})"), body) + 24.0,
                    text.measure(label, body) + 24.0,
                )
            })
            .collect();
        let relayout = widths != self.similar.footer_widths;
        self.similar.footer_widths = widths;
        let lay = self.similar.review_layout(w, h);
        self.similar.panes.clear();
        let tabs_end = self.draw_similar_header(dl, text, pal, v);
        let Some(gi) = self.similar.group_of_key(v.cat, rv.group) else {
            return relayout;
        };
        let g = self.similar.groups(v.cat)[gi].clone();
        let open = self.similar.open_groups(v.cat);
        let pos = open.iter().position(|&i| i == gi).unwrap_or(0);
        // Group line.
        let title = format!(
            "{}  ·  {}  ·  {}",
            t!(
                "similar-group-title",
                index = (pos + 1) as i64,
                total = open.len().max(1) as i64
            ),
            t!("similar-group-photos", count = g.members.len() as i64),
            t!(
                "similar-to-free",
                size = size_text(self.similar.reclaim(v.cat, &g))
            )
        );
        self.similar_put(
            text,
            dl,
            &title,
            self.ts(BODY.weight(600.0)),
            (18.0, m.header() + m.u(10.0)),
            pal.text,
        );
        let strip_h = lay.strip_h;
        // Large: the comparison, else the keepers; in the reading direction.
        let shown = self
            .similar
            .review_shown(v, rv)
            .map(|s| s.1)
            .unwrap_or_default();
        let cells = self.similar.review_cells(w, h, shown.len().max(1));
        let big_class = 512;
        let small_class = specular_thumbs::class_for(lay.strip_thumb * self.scale);
        let mut wanted: Vec<(PathBuf, u32)> = Vec::new();
        let mut atlas = self.atlas.take();
        let mut cap_tip = None;
        for (&k, cell) in shown.iter().zip(cells) {
            let Some(&mb) = g.members.get(k) else {
                continue;
            };
            let p = g.picks[k];
            let Some(path) = self.similar.path_of(mb).map(Path::to_path_buf) else {
                continue;
            };
            wanted.push((path.clone(), big_class));
            // Zoomed or in a channel view: the decoded image (`quick.rs`) once it is on the
            // GPU, sharpening as its tiles come; the thumbnail until then.
            let pane = self.pane_tex(&path).or_else(|| {
                let main = self.similar.results.as_ref()?.units.get(mb as usize)?;
                self.pane_tex(&main.main)
            });
            let it = self
                .similar
                .results
                .as_ref()
                .map(|r| r.items[mb as usize].clone());
            let (iw, ih) = it.as_ref().map_or((3.0, 2.0), |it| {
                (it.width.max(1) as f32, it.height.max(1) as f32)
            });
            let mut r = fit_in(iw, ih, cell);
            match (pane, atlas.as_mut().and_then(|a| a.best(&path, big_class))) {
                (Some((tex, pr, _)), _) => {
                    r = pr;
                    dl.shadow(r, 4.0, pal.shadow, 18.0, 4.0);
                    dl.image(r, [0.0, 0.0, 1.0, 1.0], tex, 4.0);
                }
                (None, Some(pl)) => {
                    r = fit_in(pl.width as f32, pl.height as f32, cell);
                    dl.shadow(r, 4.0, pal.shadow, 18.0, 4.0);
                    let uv = if rv.zoom > 1.0 {
                        zoom_uv(&pl, rv.zoom, rv.at)
                    } else {
                        pl.uv
                    };
                    dl.image_over(r, uv, pl.tex, 4.0, self.backdrop);
                }
                (None, None) => dl.rect(r, 4.0, pal.surface_raised),
            }
            self.similar.hits.push((r, Act::Focus(k)));
            self.similar.panes.push(r);
            if k == rv.focus {
                dl.stroke_rect(r.inset(-4.0), 8.0, pal.focus_ring, 2.0);
            }
            let keep = self.similar.keeps(v.cat, mb, &p);
            self.draw_badge(
                dl,
                r,
                keep,
                self.similar.decision(v.cat, mb) != Decision::Undecided,
            );
            // The name keeps its own direction; the caption reads in the UI's.
            let mut parts: Vec<String> = it
                .as_ref()
                .map(|i| specular_i18n::isolate(&i.name))
                .into_iter()
                .collect();
            parts.extend(self.member_details(mb, rv, Some(r), true));
            let cap = specular_i18n::I18n::global().join(&parts, "  ·  ");
            if lay.caption == 0.0 {
                if self.pointer.is_some_and(|p| r.contains(p)) {
                    cap_tip = Some((r, cap));
                }
                continue;
            }
            let cap = self.similar_bidi(&ellipsize(text, &cap, small, cell.w));
            let cb = Rect::new(cell.x, r.bottom() + 6.0, cell.w, m.u(18.0));
            put_centered(text, dl, &cap, small, cb, pal.text_muted);
        }
        // The strip: every member with its badge and reason, the first at the leading end.
        let sy = h - lay.footer_h - strip_h;
        dl.rect(Rect::new(0.0, sy, w, strip_h), 0.0, pal.surface);
        // The cells grow with the text under the thumbnails (none in a compact review).
        let (thumb, cell_w) = (lay.strip_thumb, lay.strip_cell);
        let total_w = g.members.len() as f32 * cell_w;
        let start = ((w - total_w) * 0.5).max(16.0);
        // Scroll the strip so the focus shows.
        let off = {
            let fx = start + rv.focus as f32 * cell_w;
            if fx + cell_w > w - 16.0 {
                fx + cell_w - (w - 16.0)
            } else {
                0.0
            }
        };
        dl.push_clip(Rect::new(0.0, sy, w, strip_h));
        for (k, (&mb, p)) in g.members.iter().zip(&g.picks).enumerate() {
            // Laid out left to right; what is drawn goes through `flip`.
            let x = start + k as f32 * cell_w - off;
            let tr = self.flip(Rect::new(
                x + ((cell_w - thumb) * 0.5).round(),
                sy + 10.0,
                thumb,
                thumb,
            ));
            let Some(path) = self.similar.path_of(mb).map(Path::to_path_buf) else {
                continue;
            };
            wanted.push((path.clone(), small_class));
            match atlas.as_mut().and_then(|a| a.best(&path, small_class)) {
                Some(pl) => dl.image_over(tr, crop(&pl, tr), pl.tex, 6.0, self.backdrop),
                None => dl.rect(tr, 6.0, pal.surface_raised),
            }
            let keep = self.similar.keeps(v.cat, mb, p);
            let decided = self.similar.decision(v.cat, mb) != Decision::Undecided;
            self.draw_badge(dl, tr, keep, decided);
            if k == rv.focus {
                dl.stroke_rect(tr.inset(-3.0), 8.0, pal.focus_ring, 2.0);
            }
            self.similar.hits.push((tr, Act::Focus(k)));
            // The badge toggles keep / discard.
            let badge = badge_rect(tr, self.scale, self.rtl).inset(-4.0);
            self.similar.hits.push((badge, Act::Toggle(k)));
            if !lay.strip_text {
                continue;
            }
            // The two lines under the thumbnail, `dy` below it.
            let under = Rect::new(x + 3.0, tr.bottom(), cell_w - 6.0, m.u(16.0));
            let line = |dy: f32| {
                self.flip(Rect {
                    y: under.y + dy,
                    ..under
                })
            };
            let reason = tr_reason(p.reason);
            let reason = ellipsize(text, &reason, small, cell_w - 6.0);
            put_centered(
                text,
                dl,
                &reason,
                small,
                line(m.u(4.0)),
                if keep { pal.text } else { pal.text_muted },
            );
            let state = if decided {
                if keep {
                    t!("similar-keep")
                } else {
                    t!("similar-discard")
                }
            } else if keep {
                t!("similar-suggested-keep")
            } else {
                t!("similar-suggested-discard")
            };
            let state = ellipsize(text, &state, small, cell_w - 6.0);
            put_centered(
                text,
                dl,
                &state,
                small,
                line(m.u(22.0)),
                state_text(pal, keep),
            );
        }
        dl.pop_clip();
        self.atlas = atlas;
        self.similar.set_wanted(wanted);
        // Footer: the actions with their keys, else (too wide) without them and in a tooltip,
        // on more rows when needed.
        let fy = h - lay.footer_h;
        dl.rect(Rect::new(0.0, fy, w, lay.footer_h), 0.0, pal.surface);
        dl.rect(Rect::new(0.0, fy, w, 1.0), 0.0, pal.separator);
        let mut tip = None;
        for (row, items) in lay.footer.rows.iter().enumerate() {
            let by = footer_row_y(fy, lay.footer_pad, row, m);
            for &(i, x, bw) in items {
                let (label, key, primary, act) = &buttons[i];
                let full = format!("{label}  ({key})");
                let shown = if lay.footer.keys {
                    full.clone()
                } else {
                    ellipsize(text, label, body, bw - 24.0)
                };
                let r = self.flip(Rect::new(x, by, bw, m.u(32.0)));
                self.similar_button_in(dl, text, pal, r, &shown, (*act, *primary));
                let hover = self.pointer.is_some_and(|p| r.contains(p));
                if let Some(why) = self.similar_disabled(*act).filter(|_| hover) {
                    // Unavailable: why, as the viewer's tooltips say it.
                    tip = Some((r, t!("tooltip-unavailable", label = label, reason = why)));
                } else if !lay.footer.keys && hover {
                    tip = Some((r, full));
                }
            }
        }
        // Back, before Done: without its key when the tabs leave too little room, left out
        // (Esc still goes back) when they leave none.
        let done = t!("similar-done");
        let dw = text.measure(&done, body) + 24.0;
        let room = w - 16.0 - dw - 8.0 - tabs_end;
        let back = [format!("{}  (Esc)", t!("similar-back")), t!("similar-back")]
            .into_iter()
            .map(|l| (text.measure(&l, body) + 24.0, l))
            .find(|(bw, _)| *bw <= room);
        if let Some((bw, back)) = back {
            self.similar_button_draw(
                dl,
                text,
                pal,
                (w - dw - 16.0 - bw - 8.0, m.u(14.0)),
                &back,
                (Act::Back, false),
            );
        }
        // The key of the footer button under the pointer: above the footer, so it covers
        // none of its rows.
        if let Some((r, full)) = tip {
            self.similar_tip(dl, text, pal, &full, (r.center().0, fy), true);
        }
        // Without captions (the most compact review): the caption of the large image under
        // the pointer, below it.
        if let Some((r, cap)) = cap_tip {
            self.similar_tip(dl, text, pal, &cap, (r.center().0, r.bottom()), false);
        }
        relayout
    }

    /// Why the control `a` does nothing now (screen readers announce it unavailable), or
    /// None: Discard while the focused member is protected; "100 %" while the focused member
    /// already shows at 100 % or larger (as in the viewer's toolbar, the control stays,
    /// dimmed, and says why in its tooltip and a11y description).
    fn similar_disabled(&self, a: Act) -> Option<String> {
        let v = self.similar.view.as_ref()?;
        let rv = v.review.as_ref()?;
        if a == Act::Zoom {
            let scale = self.scale.max(0.5);
            return (!self.similar.can_zoom_in(self.vw, self.vh, scale))
                .then(|| t!("similar-zoom-already"));
        }
        if a != Act::Set(Decision::Discard) {
            return None;
        }
        let gi = self.similar.group_of_key(v.cat, rv.group)?;
        let m = *self.similar.groups(v.cat)[gi].members.get(rv.focus)?;
        let protected = self
            .similar
            .results
            .as_ref()
            .and_then(|r| r.items.get(m as usize))
            .is_some_and(|it| it.protect.is_some());
        protected.then(|| t!("similar-toast-protected"))
    }

    /// The label of the control `a` as drawn this frame (without its key), None when it is
    /// not on screen.
    fn similar_control_label(&self, v: &View, a: Act) -> Option<String> {
        self.similar
            .hits
            .iter()
            .any(|(_, x)| *x == a)
            .then_some(())?;
        Some(match a {
            Act::Tab(c) => cat_name(c),
            Act::Back => t!("similar-back"),
            Act::Close => t!("similar-done"),
            Act::Stop => t!("similar-cancel"),
            Act::Rescan => t!("similar-scan-again"),
            Act::Slider => t!("similar-sensitivity"),
            Act::AcceptAll | Act::ActionMenu | Act::Unskip => {
                self.similar_list_buttons(v, true)
                    .into_iter()
                    .find(|b| b.3 == a)?
                    .1
            }
            _ => review_buttons().into_iter().find(|b| b.3 == a)?.0,
        })
    }

    /// AccessKit: the open category as a dialog: its tabs, summary and scan progress, Back
    /// and Done, the sensitivity slider, the groups (or members) as options of a list box,
    /// and the footer's buttons, each where it was drawn. True: it covers the grid (the
    /// grid's nodes are left out).
    pub(super) fn similar_a11y(&self, tree: &mut accesskit::TreeUpdate, scale: f64) -> bool {
        use accesskit::{Action, Live, Node, NodeId, Orientation, Role, Toggled};
        let Some(v) = &self.similar.view else {
            return false;
        };
        let base = A11Y_BASE + A11Y_OFFSET;
        let bounds = |r: Rect| accesskit::Rect {
            x0: f64::from(r.x) * scale,
            y0: f64::from(r.y) * scale,
            x1: f64::from(r.x + r.w) * scale,
            y1: f64::from(r.y + r.h) * scale,
        };
        let (groups, files, bytes) = self.similar.totals(v.cat);
        let mut dialog = Node::new(Role::Dialog);
        dialog.set_label(format!(
            "{}: {}",
            cat_name(v.cat),
            t!(
                "similar-summary",
                groups = groups as i64,
                files = files as i64,
                size = size_text(bytes)
            )
        ));
        dialog.set_bounds(bounds(Rect::new(0.0, 0.0, self.vw, self.vh)));
        // Each node where it was last drawn (a member's in the strip): in a right-to-left
        // layout the first is on the right, but the nodes stay in reading order.
        let drawn = |act: Act| {
            self.similar
                .hits
                .iter()
                .rev()
                .find(|(_, a)| *a == act)
                .map(|(r, _)| bounds(*r))
        };
        let reviewed = v
            .review
            .as_ref()
            .and_then(|r| self.similar.group_of_key(v.cat, r.group).map(|gi| (r, gi)));
        let control = |a: Act| {
            let i = CONTROLS.iter().position(|c| *c == a)?;
            let label = self.similar_control_label(v, a)?;
            let id = NodeId(base + A11Y_CONTROLS + i as u64);
            let mut n = Node::new(Role::Button);
            // The key as a property (UIA, macOS) and, as the chrome's tooltips give it, in
            // the description ("Keep (P)"): AT-SPI has no key-shortcut property.
            if let Some(k) = act_key(a, v.review.is_some()) {
                n.set_keyboard_shortcut(k);
                n.set_description(format!("{label} ({k})"));
            }
            n.set_label(label);
            if let Some(b) = drawn(a) {
                n.set_bounds(b);
            }
            if let Some(why) = self.similar_disabled(a) {
                n.set_disabled();
                n.set_description(why);
            } else if a != Act::Slider {
                n.add_action(Action::Click);
            }
            Some((id, n))
        };
        let mut children = Vec::new();
        let mut nodes = Vec::new();
        // The header: the tabs, the summary and the scan's progress, Back and Done.
        let tabs: Vec<(NodeId, Node)> = [Category::Duplicates, Category::Similar]
            .into_iter()
            .filter_map(|c| {
                let (id, mut n) = control(Act::Tab(c))?;
                n.set_role(Role::Tab);
                n.set_selected(c == v.cat);
                Some((id, n))
            })
            .collect();
        if !tabs.is_empty() {
            let mut list = Node::new(Role::TabList);
            list.set_children(tabs.iter().map(|t| t.0).collect::<Vec<_>>());
            children.push(NodeId(base + A11Y_TABS));
            nodes.push((NodeId(base + A11Y_TABS), list));
            nodes.extend(tabs);
        }
        let summary = self.similar.summary_line(v.cat);
        if !summary.is_empty() {
            let mut n = Node::new(Role::Label);
            n.set_value(without_isolates(&summary.replace("  ·  ", ", ")));
            children.push(NodeId(base + A11Y_SUMMARY));
            nodes.push((NodeId(base + A11Y_SUMMARY), n));
        }
        // The scan's progress: its value as it goes, its name (announced politely) only at
        // the quarters, so a screen reader is not talked over at every image.
        if let Some((phase, done, total)) = self.similar.progress() {
            let mut n = Node::new(Role::ProgressIndicator);
            n.set_label(progress_text(phase, said_progress(done, total), total));
            n.set_live(Live::Polite);
            n.set_numeric_value(done.min(total) as f64);
            n.set_min_numeric_value(0.0);
            n.set_max_numeric_value(total.max(1) as f64);
            n.set_value(without_isolates(&progress_text(phase, done, total)));
            children.push(NodeId(base + A11Y_PROGRESS));
            nodes.push((NodeId(base + A11Y_PROGRESS), n));
        } else if let Some(phase) = self.similar.stopped_phase() {
            // Stopped: said where the progress was.
            let mut n = Node::new(Role::Label);
            n.set_value(match phase {
                Phase::Signatures => t!("similar-stopped"),
                Phase::Focus => t!("similar-stopped-sharpness"),
            });
            children.push(NodeId(base + A11Y_PROGRESS));
            nodes.push((NodeId(base + A11Y_PROGRESS), n));
        }
        for a in [Act::Back, Act::Stop, Act::Rescan, Act::Close] {
            if let Some((id, n)) = control(a) {
                children.push(id);
                nodes.push((id, n));
            }
        }
        // The sensitivity slider (similar shots' list), in percent as drawn.
        if let Some((id, mut n)) = control(Act::Slider) {
            n.set_role(Role::Slider);
            n.set_orientation(Orientation::Horizontal);
            n.set_numeric_value(f64::from((self.similar.sens * 100.0).round()));
            n.set_min_numeric_value(0.0);
            n.set_max_numeric_value(100.0);
            n.set_numeric_value_step(f64::from((slider_key_step(true, false) * 100.0).round()));
            // Which end is which: strict at 0.
            n.set_description(format!(
                "{} – {}",
                t!("similar-sensitivity-strict"),
                t!("similar-sensitivity-loose")
            ));
            n.add_action(Action::Increment);
            n.add_action(Action::Decrement);
            n.add_action(Action::SetValue);
            children.push(id);
            nodes.push((id, n));
        }
        // The options.
        let mut ids = Vec::new();
        let mut focus = None;
        match reviewed {
            Some((r, gi)) => {
                let g = &self.similar.groups(v.cat)[gi];
                for (k, (&m, p)) in g.members.iter().zip(&g.picks).enumerate() {
                    let id = NodeId(base + 1 + k as u64);
                    let mut n = Node::new(Role::ListBoxOption);
                    let name = self
                        .similar
                        .results
                        .as_ref()
                        .map(|r| r.items[m as usize].name.clone())
                        .unwrap_or_default();
                    let keep = self.similar.keeps(v.cat, m, p);
                    let decided = self.similar.decision(v.cat, m) != Decision::Undecided;
                    let state = match (decided, keep) {
                        (true, true) => t!("similar-keep"),
                        (true, false) => t!("similar-discard"),
                        (false, true) => t!("similar-suggested-keep"),
                        (false, false) => t!("similar-suggested-discard"),
                    };
                    n.set_label(t!(
                        "similar-a11y-member",
                        name = name,
                        state = state,
                        reason = tr_reason(p.reason)
                    ));
                    // The large image's caption (the most compact review leaves it out): the
                    // size, and the zoom and channel view while it shows large.
                    let pane = self
                        .similar
                        .hits
                        .iter()
                        .find(|(rect, a)| *a == Act::Focus(k) && self.similar.panes.contains(rect))
                        .map(|(rect, _)| *rect);
                    let details = self.member_details(m, r, pane, false);
                    if !details.is_empty() {
                        n.set_description(details.join(", "));
                    }
                    if let Some(b) = drawn(Act::Focus(k)) {
                        n.set_bounds(b);
                    }
                    n.set_selected(k == r.focus);
                    n.add_action(Action::Click);
                    n.add_action(Action::Focus);
                    if k == r.focus {
                        focus = Some(id);
                    }
                    ids.push(id);
                    nodes.push((id, n));
                }
            }
            None => {
                for (row, &gi) in self.similar.open_groups(v.cat).iter().enumerate() {
                    let g = &self.similar.groups(v.cat)[gi];
                    let id = NodeId(base + 1 + row as u64);
                    let mut n = Node::new(Role::ListBoxOption);
                    n.set_label(format!(
                        "{}, {}",
                        t!("similar-group-photos", count = g.members.len() as i64),
                        t!(
                            "similar-to-free",
                            size = size_text(self.similar.reclaim(v.cat, g))
                        )
                    ));
                    // The row's lines under it: the best pick and why, why the others go.
                    let mut said = Vec::new();
                    if let Some((&best, p)) =
                        lead(g).and_then(|k| g.members.get(k).zip(g.picks.get(k)))
                        && let Some(r) = &self.similar.results
                    {
                        said.push(format!(
                            "{}: {}",
                            r.items[best as usize].name,
                            tr_reason(p.reason)
                        ));
                    }
                    if let Some(p) = g.picks.iter().find(|p| !p.keep) {
                        said.push(format!(
                            "{} {}",
                            t!("similar-discard"),
                            tr_reason(p.reason).to_lowercase()
                        ));
                    }
                    if !said.is_empty() {
                        n.set_description(said.join(", "));
                    }
                    if let Some(b) = drawn(Act::Open(row)) {
                        n.set_bounds(b);
                    }
                    n.set_selected(row == v.cursor);
                    n.add_action(Action::Click);
                    n.add_action(Action::Focus);
                    if row == v.cursor {
                        focus = Some(id);
                    }
                    ids.push(id);
                    nodes.push((id, n));
                    if row > 500 {
                        break;
                    }
                }
            }
        }
        let list_id = NodeId(base);
        let mut list = Node::new(Role::ListBox);
        list.set_label(cat_name(v.cat));
        list.set_children(ids);
        children.push(list_id);
        nodes.push((list_id, list));
        // The footer's buttons, in its order (as drawn, the reading order).
        let footer = self
            .similar
            .hits
            .iter()
            .map(|h| h.1)
            .filter(|a| CONTROLS[FOOTER_CONTROLS].contains(a));
        for a in footer {
            if let Some((id, mut n)) = control(a) {
                match (a, &reviewed) {
                    (Act::Compare, Some((r, _))) => {
                        n.set_toggled(Toggled::from(r.compare.is_some()))
                    }
                    (Act::Zoom, Some((r, _))) => n.set_toggled(Toggled::from(r.zoom > 1.0)),
                    _ => {}
                }
                children.push(id);
                nodes.push((id, n));
            }
        }
        let dialog_id = NodeId(base - 1);
        dialog.set_children(children);
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
            root.push_child(dialog_id);
        }
        tree.nodes.push((dialog_id, dialog));
        tree.nodes.extend(nodes);
        if tree.focus == NodeId(1) {
            tree.focus = focus.unwrap_or(list_id);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG contrast ratio of `a` on `b` (opaque colours).
    fn contrast(a: Color, b: Color) -> f32 {
        let (x, y) = (a.luminance(), b.luminance());
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// `c` drawn over the opaque `bg`, blended in linear light as the renderer does.
    fn over(c: Color, bg: Color) -> Color {
        let lin = |v: f32| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let srgb = |v: f32| {
            if v <= 0.0031308 {
                v * 12.92
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }
        };
        let m = |f: f32, b: f32| srgb(lin(f) * c.a + lin(b) * (1.0 - c.a));
        Color::rgba(m(c.r, bg.r), m(c.g, bg.g), m(c.b, bg.b), 1.0)
    }

    /// Cancel stops the scan with Esc (outside a review): its key is announced as Back's is.
    #[test]
    fn cancel_names_its_key() {
        assert_eq!(act_key(Act::Stop, false), Some("Esc"));
        assert_eq!(act_key(Act::Stop, true), None);
        assert_eq!(act_key(Act::Rescan, false), None);
    }

    #[test]
    fn small_text_passes_aa_in_both_themes() {
        for pal in [Palette::dark(), Palette::light()] {
            // Plain rows, the strip and the footer are `surface`; focused and hovered rows are
            // `selection` / `surface_hover` over `canvas`.
            let mut cases = vec![(pal.surface, row_secondary(&pal, false), "secondary")];
            for (keep, what) in [(true, "keep"), (false, "discard")] {
                cases.push((pal.surface, state_text(&pal, keep), what));
            }
            for tint in [pal.selection, pal.surface_hover] {
                cases.push((
                    over(tint, pal.canvas),
                    row_secondary(&pal, true),
                    "tinted row",
                ));
            }
            for (bg, fg, what) in cases {
                let c = contrast(fg, bg);
                assert!(c >= 4.5, "{:?} {what} on {bg:?}: {c:.2}", pal.mode);
            }
        }
    }

    #[test]
    fn badges_grow_with_the_thumbnail_on_whole_pixels() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let mut last = 0.0;
            for t in [ROW_THUMB, STRIP_THUMB, 200.0, 600.0] {
                let r = Rect::new(13.3, 7.7, t, t);
                let b = badge_rect(r, scale, false);
                assert!(
                    b.w >= 24.0 - 0.5 / scale && b.w <= 32.0 + 0.5 / scale,
                    "{t} @{scale}"
                );
                assert!(b.w >= last);
                last = b.w;
                for v in [b.x, b.y, b.w] {
                    let px = v * scale;
                    assert!(
                        (px - px.round()).abs() < 1e-3,
                        "{v} @{scale} not on a device pixel"
                    );
                }
                assert!(b.x >= r.x && b.right() <= r.right() && b.bottom() <= r.bottom());
                // Right to left: the same badge in the mirrored corner.
                let m = badge_rect(r, scale, true);
                assert_eq!((m.y, m.w, m.h), (b.y, b.w, b.h));
                assert!((m.x - r.x - (r.right() - b.right())).abs() <= 1.0 / scale);
                assert!(((m.x * scale) - (m.x * scale).round()).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn marks_protect_rated_flagged_labelled() {
        let m = |rating, label, flag| Mark {
            rating,
            label,
            flag,
        };
        assert_eq!(marks_protect(m(3, 0, 0)), Some(Protect::Rated));
        assert_eq!(marks_protect(m(0, 0, 1)), Some(Protect::Flagged));
        assert_eq!(marks_protect(m(0, 2, 0)), Some(Protect::Labelled));
        assert_eq!(marks_protect(m(0, 0, -1)), None);
        assert_eq!(marks_protect(Mark::default()), None);
    }

    /// Three shots of one burst, all alike: one similar group.
    fn burst() -> Similar {
        let n = 3;
        let paths: Vec<PathBuf> = (0..n)
            .map(|k| PathBuf::from(format!("/burst/{k}.jpg")))
            .collect();
        let edge = |a, b| specular_similar::group::Edge {
            a,
            b,
            visual: 0.0,
            dt: 0.5,
            dist: f32::NAN,
            same_body: Some(true),
            burst: true,
            zoom: 1.0,
        };
        let r = Results {
            units: paths
                .iter()
                .map(|p| Unit {
                    main: p.clone(),
                    files: vec![p.clone()],
                    sig_path: p.clone(),
                })
                .collect(),
            items: paths
                .iter()
                .map(|p| specular_similar::Item {
                    name: p.file_name().unwrap().to_string_lossy().into_owned(),
                    width: 1200,
                    height: 800,
                    bit_depth: 8,
                    bytes: 1_000_000,
                    len: 1_000_000,
                    ..specular_similar::Item::default()
                })
                .collect(),
            ids: vec![specular_similar::FileId::default(); n],
            keys: (1..=n as u64).collect(),
            edges: vec![edge(0, 1), edge(1, 2), edge(0, 2)],
            ..Results::default()
        };
        Similar {
            sidecar: vec![None; n],
            results: Some(r),
            sens: 0.5,
            view: Some(View {
                cat: Category::Similar,
                cursor: 0,
                scroll: 0.0,
                review: None,
                slider_drag: false,
            }),
            ..Similar::default()
        }
    }

    #[test]
    fn a_refine_keeps_the_order_of_the_group_under_the_pointer() {
        let marks = super::super::marks::Store::new(None);
        let mut sm = burst();
        sm.regroup(&marks);
        assert_eq!(sm.sims.len(), 1, "one group");
        let before = sm.sims[0].members.clone();
        let key = sm.sim_info[0].key;
        // Measures arrive that make the last member the best (here: more pixels).
        let last = *before.last().unwrap();
        if let Some(r) = sm.results.as_mut() {
            r.items[last as usize].width = 4800;
            r.items[last as usize].height = 3200;
        }
        // Under the pointer: the order stays, the picks and the lead follow the measures.
        sm.hover_group = Some(key);
        sm.refine(&marks);
        let g = &sm.sims[0];
        assert_eq!(g.members, before);
        assert_eq!(sm.held, vec![key]);
        assert_eq!(lead(g), Some(before.len() - 1), "{:?}", g.picks);
        assert!(!sm.released());
        let path = sm.path_of(last).unwrap().to_path_buf();
        assert_eq!(sm.stack_of.get(&path).map(|s| s.1), Some(true));
        // Let go: the suggested order, best first.
        sm.hover_group = None;
        assert!(sm.released());
        sm.refine(&marks);
        let g = &sm.sims[0];
        assert_eq!(g.members[0], last);
        assert_eq!(lead(g), Some(0));
        assert!(sm.held.is_empty());
        assert_eq!(sm.sim_info[0].key, key, "the group keeps its key");
    }

    /// A gallery stack's cover in view stays while the focus pass refines the picks, and
    /// becomes the best once out of view (or when the pass ends: no covers are in view then).
    #[test]
    fn a_refine_keeps_the_cover_of_a_stack_in_view() {
        let marks = super::super::marks::Store::new(None);
        let mut sm = burst();
        sm.view = None;
        sm.regroup(&marks);
        let path = |sm: &Similar, m: u32| sm.path_of(m).unwrap().to_path_buf();
        let first = sm.sims[0].members[0];
        let cover = path(&sm, first);
        assert_eq!(sm.stack_of.get(&cover).map(|s| s.1), Some(true));
        let entries: Vec<Entry> = (0..3)
            .map(|k| Entry {
                path: PathBuf::from(format!("/burst/{k}.jpg")),
                name: format!("{k}.jpg"),
                kind: EntryKind::Image,
                len: 1,
                modified: None,
                hidden: false,
                taken: None,
                dims: None,
            })
            .collect();
        let stacks = vec![(0, vec![0]), (1, vec![1]), (2, vec![2])];
        let shown = |sm: &Similar| {
            let s = sm.merge_stacks(stacks.clone(), &entries);
            assert_eq!(s.len(), 1, "one stack");
            entries[s[0].0].path.clone()
        };
        assert_eq!(shown(&sm), cover);
        // Another member becomes the best while the cover is on screen: it stays.
        let last = *sm.sims[0].members.last().unwrap();
        if let Some(r) = sm.results.as_mut() {
            r.items[last as usize].width = 4800;
            r.items[last as usize].height = 3200;
        }
        sm.covers = vec![cover.clone()];
        sm.refine(&marks);
        assert_eq!(
            sm.sims[0].members[0], last,
            "the suggested order is the new one"
        );
        assert_eq!(shown(&sm), cover);
        assert_eq!(sm.covers_held, vec![cover.clone()]);
        assert!(!sm.released());
        // Scrolled away (or the pass ended): the best becomes the cover.
        sm.covers.clear();
        assert!(sm.released());
        sm.refine(&marks);
        assert_eq!(shown(&sm), path(&sm, last));
        assert!(sm.covers_held.is_empty());
        assert!(!sm.released());
        // A cover in view that is already the best holds nothing.
        sm.covers = vec![path(&sm, last)];
        sm.refine(&marks);
        assert!(sm.covers_held.is_empty());
        // A full regroup (marks, a new scan) ignores the covers.
        sm.covers = vec![cover.clone()];
        sm.regroup(&marks);
        assert_eq!(shown(&sm), path(&sm, last));
        assert_eq!(sm.covers, vec![cover], "kept for the next refine");
    }

    #[test]
    fn lead_is_the_best_whatever_the_order() {
        let pick = |keep, score| Pick {
            keep,
            reason: Reason::AlsoGood,
            score,
        };
        let mut g = Group {
            cat: Category::Similar,
            members: vec![4, 5, 6],
            picks: vec![pick(false, 0.9), pick(true, 0.5), pick(true, 0.7)],
            reclaim: 0,
            tier: None,
        };
        assert_eq!(lead(&g), Some(2));
        g.picks[1].score = 0.7;
        assert_eq!(lead(&g), Some(1), "ties: the first");
        g.cat = Category::Duplicates;
        g.picks = vec![pick(true, 2.0), pick(true, 0.0), pick(false, 1.0)];
        assert_eq!(lead(&g), Some(1), "duplicates: rank 0 is the best");
        // Held in another order: members and picks move together.
        assert!(hold_order(&mut g, &[6, 5, 4]));
        assert_eq!(g.members, vec![6, 5, 4]);
        assert_eq!(g.picks[0].score, 1.0);
        assert_eq!(lead(&g), Some(1));
        assert!(!hold_order(&mut g, &[6, 5, 7]));
        assert_eq!(g.members, vec![6, 5, 4]);
    }

    /// Narrow windows cut the header's summary, never the focus pass's progress, until
    /// the summary would say nothing; then the progress shows alone.
    #[test]
    fn the_header_keeps_the_progress_whole() {
        // One unit a character.
        let mut m = |s: &str| s.chars().count() as f32;
        let summary = "3 groups to review, 4 files, 12 MB  ·  0 of 3 done";
        let progress = "Checking sharpness… 5 of 40";
        let wide = header_line(&mut m, summary, progress, 200.0);
        assert_eq!(
            wide,
            (summary.to_owned(), format!("{HEADER_SEP}{progress}"))
        );
        let (s, p) = header_line(&mut m, summary, progress, 50.0);
        assert_eq!(p, format!("{HEADER_SEP}{progress}"));
        assert!(s.ends_with('…') && m(&s) + m(&p) <= 50.0, "{s:?}");
        assert!(summary.starts_with(s.trim_end_matches('…')));
        // Too narrow for any summary: the progress alone, whole while it fits.
        assert_eq!(
            header_line(&mut m, summary, progress, 34.0),
            (String::new(), progress.to_owned())
        );
        let (s, p) = header_line(&mut m, summary, progress, 20.0);
        assert!(s.is_empty() && p.ends_with('…') && m(&p) <= 20.0, "{p:?}");
        // No pass running: the summary as before; none yet (the scan): the progress.
        assert_eq!(
            header_line(&mut m, summary, "", 20.0).0,
            ellipsize_by(&mut m, summary, 20.0)
        );
        assert_eq!(
            header_line(&mut m, "", progress, 200.0),
            (String::new(), progress.to_owned())
        );
    }

    #[test]
    fn the_header_shows_the_focus_pass() {
        let mut sm = burst();
        sm.regroup(&super::super::marks::Store::new(None));
        assert_eq!(sm.progress(), None);
        sm.scan = Some(Scan::fake(Phase::Focus, 3, 10));
        assert_eq!(sm.progress(), Some((Phase::Focus, 3, 10)));
        assert!(!sm.settled());
        // The pass is told the visible groups' members.
        sm.wish(&[0]);
        let mut wished = sm.scan.as_ref().unwrap().wish.get();
        wished.sort_unstable();
        assert_eq!(wished, vec![0, 1, 2]);
        sm.scan = None;
        sm.refine_due = true;
        assert!(!sm.settled(), "a refine pending");
    }

    /// The sensitivity slider's row fits the window: full width when there is room, then a
    /// shorter track, then the label above it; nothing runs past the right margin. Widths as
    /// the English (Sensitivity, Strict, Loose) and German (Empfindlichkeit, Streng, Locker)
    /// texts measure at 13 and 11.5 px.
    #[test]
    fn the_sensitivity_slider_fits_narrow_windows() {
        let right = |l: SliderLayout, label: f32, ends: (f32, f32), value: f32| {
            let x0 = if l.stacked { 18.0 } else { 18.0 + label + 16.0 };
            x0 + ends.0 + 12.0 + l.track + 12.0 + ends.1 + 12.0 + value
        };
        for (label, ends) in [(68.0, (32.0, 31.0)), (98.0, (37.0, 37.0))] {
            let value = 20.0;
            let lay = |w| slider_layout(w, label, ends.0, ends.1, value);
            // A wide window: the full track beside the label.
            assert_eq!(
                lay(720.0),
                SliderLayout {
                    stacked: false,
                    track: TRACK_MAX
                }
            );
            assert_eq!(lay(720.0).height(Metrics::new(1.0)), SLIDER_ROW);
            assert_eq!(lay(720.0).height(Metrics::new(2.0)), SLIDER_ROW * 2.0);
            let mut was = TRACK_MAX;
            let mut stacked_at = None;
            for w in (300..=720).rev().step_by(4) {
                let w = w as f32;
                let l = lay(w);
                assert!(
                    right(l, label, ends, value) <= w - 18.0 + 0.01,
                    "{w}: {l:?}"
                );
                if l.stacked {
                    stacked_at.get_or_insert(w);
                    assert_eq!(l.height(Metrics::new(1.0)), SLIDER_ROW + SLIDER_STACK);
                } else {
                    assert!(stacked_at.is_none(), "{w}: back beside the label");
                    assert!(l.track >= TRACK_MIN && l.track <= was, "{w}: {l:?}");
                    was = l.track;
                }
            }
            // The English row stays on one line down to the narrowest window (360); the
            // longer German label goes above it there.
            let one_line_at_360 = !lay(360.0).stacked;
            assert_eq!(one_line_at_360, label < 80.0, "{label}");
        }
        // Far too narrow: the track keeps a stub rather than a negative width.
        assert_eq!(slider_layout(100.0, 68.0, 32.0, 31.0, 20.0).track, 24.0);
    }

    /// The shots' burst as the gallery draws it, `w` × 450, right to left or not, at text
    /// scale `ts`: the group list (Similar, with the slider), or its review.
    fn drawn(w: f32, rtl: bool, ts: f32, review: bool) -> Gallery {
        drawn_at((w, 450.0), rtl, ts, review)
    }

    /// [`drawn`] in a view of `size`.
    fn drawn_at((w, h): (f32, f32), rtl: bool, ts: f32, review: bool) -> Gallery {
        let paths: Vec<PathBuf> = (1..=3)
            .map(|k| PathBuf::from(format!("/shots/burst-{k}.jpg")))
            .collect();
        let units: Vec<Unit> = paths
            .iter()
            .map(|p| Unit {
                main: p.clone(),
                files: vec![p.clone()],
                sig_path: p.clone(),
            })
            .collect();
        let items: Vec<specular_similar::Item> = paths
            .iter()
            .map(|p| specular_similar::Item {
                name: p.file_name().unwrap().to_string_lossy().into_owned(),
                width: 800,
                height: 600,
                bit_depth: 8,
                bytes: 2_000_000,
                len: 2_000_000,
                ..specular_similar::Item::default()
            })
            .collect();
        let mut g = Gallery {
            active: true,
            vw: w,
            vh: h,
            rtl,
            ..Gallery::default()
        };
        g.set_text_scale(ts);
        g.similar = if review {
            Similar::for_shots_review(units, items)
        } else {
            Similar::for_shots_list(units, items, 7, 12)
        };
        redraw(&mut g);
        g
    }

    /// Draws `g`'s similar view again (twice when the first frame changed the layout, as
    /// the app does).
    fn redraw(g: &mut Gallery) {
        let pal = Palette::dark();
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion: true,
            size: (g.vw, g.vh),
            scale: 1.0,
            rtl: g.rtl,
            text_scale: g.text_scale,
            backdrop: Backdrop::None,
        };
        let mut dl = DrawList::new(1.0);
        let mut text = Text::new();
        if g.draw_similar(&mut dl, &mut text, &f) {
            let mut dl = DrawList::new(1.0);
            assert!(!g.draw_similar(&mut dl, &mut text, &f), "settled");
        }
    }

    /// The group list with a skipped group (so its footer has three buttons).
    fn drawn_skipped(size: (f32, f32), rtl: bool, ts: f32) -> Gallery {
        let mut g = drawn_at(size, rtl, ts, false);
        g.similar.add_skipped_for_shots();
        redraw(&mut g);
        g
    }

    fn hit(g: &Gallery, act: Act) -> Rect {
        g.similar
            .hits
            .iter()
            .rev()
            .find(|(_, a)| *a == act)
            .map(|(r, _)| *r)
            .unwrap_or_else(|| panic!("{act:?} drawn"))
    }

    /// While a scan runs the list's header offers Cancel before Done; once stopped, "Scan
    /// again" in its place and the header says what was stopped; neither in the review.
    #[test]
    fn the_header_offers_cancel_then_scan_again() {
        let mut g = drawn(720.0, false, 1.0, false);
        let (stop, done) = (hit(&g, Act::Stop), hit(&g, Act::Close));
        assert!(stop.right() <= done.x, "{stop:?} before {done:?}");
        assert!(!g.similar.hits.iter().any(|(_, a)| *a == Act::Rescan));
        // Stopped in the focus pass: the groups stay.
        g.similar.scan = None;
        g.similar.stopped = Some((0, Phase::Focus));
        redraw(&mut g);
        let again = hit(&g, Act::Rescan);
        assert!(again.right() <= done.x);
        assert!(!g.similar.hits.iter().any(|(_, a)| *a == Act::Stop));
        assert!(
            g.similar
                .hits
                .iter()
                .any(|(_, a)| matches!(a, Act::Open(_)))
        );
        // Stopped before anything was grouped: the list says so.
        g.similar.stopped = Some((0, Phase::Signatures));
        g.similar.results = None;
        assert_eq!(g.similar.stopped_phase(), Some(Phase::Signatures));
        redraw(&mut g);
        hit(&g, Act::Rescan);
        // A scan running again: no stopped state shows.
        g.similar.scan = Some(Scan::fake(Phase::Signatures, 0, 10));
        assert_eq!(g.similar.stopped_phase(), None);
        // The review keeps its header to the group.
        let r = drawn(720.0, false, 1.0, true);
        assert!(
            !r.similar
                .hits
                .iter()
                .any(|(_, a)| matches!(a, Act::Stop | Act::Rescan))
        );
    }

    fn mirrored(a: Rect, b: Rect, w: f32) -> bool {
        let m = a.mirror_x(w);
        (m.x - b.x).abs() < 0.01 && m.y == b.y && m.w == b.w && m.h == b.h
    }

    /// Right to left, the list and the review are the mirror of left to right (same text, so
    /// the same widths): tabs, Done and Back, the rows, the slider (strict on the right, its
    /// fill growing leftwards) and the footer's buttons; the review's first image and the
    /// strip's first member on the right, the badges in the top-left corner.
    #[test]
    fn right_to_left_mirrors_the_list_and_the_review() {
        let w = 720.0;
        let (l, r) = (drawn(w, false, 1.0, false), drawn(w, true, 1.0, false));
        for act in [
            Act::Tab(Category::Duplicates),
            Act::Tab(Category::Similar),
            Act::Close,
            Act::Open(0),
            Act::Slider,
            Act::AcceptAll,
            Act::ActionMenu,
        ] {
            assert!(mirrored(hit(&l, act), hit(&r, act), w), "{act:?}");
        }
        let (lt, rt) = (
            l.similar.hits_slider.unwrap().0,
            r.similar.hits_slider.unwrap().0,
        );
        assert!(mirrored(lt, rt, w));
        // Strict at the leading end: the knob at sensitivity s is where `slider_at` reads s.
        for s in [0.0, 0.25, 0.5, 1.0] {
            assert!((r.similar.slider_at(rt.right() - rt.w * s, rt) - s).abs() < 1e-5);
            assert!((l.similar.slider_at(lt.x + lt.w * s, lt) - s).abs() < 1e-5);
        }
        assert_eq!(r.similar.slider_at(rt.right() + 50.0, rt), 0.0);
        assert_eq!(r.similar.slider_at(rt.x - 50.0, rt), 1.0);
        // The arrows move the knob the way they point.
        assert!(slider_key_step(true, false) > 0.0 && slider_key_step(false, false) < 0.0);
        assert!(slider_key_step(true, true) < 0.0 && slider_key_step(false, true) > 0.0);

        let (l, r) = (drawn(w, false, 1.0, true), drawn(w, true, 1.0, true));
        for act in [
            Act::Back,
            Act::Close,
            Act::AcceptNext,
            Act::Set(Decision::Keep),
        ] {
            assert!(mirrored(hit(&l, act), hit(&r, act), w), "{act:?}");
        }
        for k in 0..3 {
            // The strip (the last Focus hit of a member) and its badge.
            assert!(
                mirrored(hit(&l, Act::Focus(k)), hit(&r, Act::Focus(k)), w),
                "{k}"
            );
            let (thumb, badge) = (hit(&r, Act::Focus(k)), hit(&r, Act::Toggle(k)));
            assert!(
                badge.center().0 < thumb.center().0,
                "{k}: badge on the left"
            );
        }
        let large = |g: &Gallery, k: usize| {
            g.similar
                .hits
                .iter()
                .find(|(_, a)| *a == Act::Focus(k))
                .map(|(r, _)| *r)
                .unwrap()
        };
        assert!(large(&l, 0).x < large(&l, 1).x);
        assert!(large(&r, 0).x > large(&r, 1).x);
        assert!(mirrored(large(&l, 0), large(&r, 0), w));
        // The layout outside drawing (zoom, panes) agrees with what was drawn.
        assert!(r.similar.rtl && !l.similar.rtl);
        let cells = r.similar.review_cells(w, 450.0, 2);
        assert!(cells[0].contains(large(&r, 0).center()));
    }

    /// At text scale 2 the header, footer, buttons, slider row and rows grow; the thumbnails
    /// keep their size; everything stays in the window.
    #[test]
    fn text_scale_grows_the_similar_view() {
        let w = 720.0;
        let (a, b) = (drawn(w, false, 1.0, false), drawn(w, false, 2.0, false));
        assert_eq!(hit(&b, Act::Close).h, 2.0 * hit(&a, Act::Close).h);
        assert_eq!(hit(&b, Act::AcceptAll).h, 2.0 * hit(&a, Act::AcceptAll).h);
        assert!(hit(&b, Act::Close).w > hit(&a, Act::Close).w);
        assert!(hit(&b, Act::AcceptAll).bottom() <= 450.0);
        assert!(hit(&b, Act::Close).bottom() <= Metrics::new(2.0).header());
        let (ra, rb) = (hit(&a, Act::Open(0)), hit(&b, Act::Open(0)));
        assert_eq!(ra.h, ROW_H - 8.0);
        assert!(rb.y > ra.y, "below a taller header and slider");
        assert_eq!(
            (rb.y - (Metrics::new(2.0).header() + Metrics::new(2.0).u(SLIDER_ROW))),
            4.0
        );
        assert!(hit(&b, Act::Slider).y >= Metrics::new(2.0).header());
        assert_eq!(Metrics::new(1.0).row(), ROW_H);
        assert_eq!(Metrics::new(2.0).row(), ROW_H + ROW_TEXT);
        assert_eq!(Metrics::new(1.0).strip(), STRIP_THUMB + 56.0);
        // The review (tall enough not to be compact): the strip's thumbnails keep their
        // size, the strip and its cells grow with the text.
        let tall = (w, 900.0);
        let (a, b) = (
            drawn_at(tall, false, 1.0, true),
            drawn_at(tall, false, 2.0, true),
        );
        let (ta, tb) = (hit(&a, Act::Focus(0)), hit(&b, Act::Focus(0)));
        assert_eq!((ta.w, ta.h), (tb.w, tb.h));
        assert!(tb.y < ta.y);
        let step = |g: &Gallery| hit(g, Act::Focus(1)).x - hit(g, Act::Focus(0)).x;
        assert_eq!(step(&a), STRIP_THUMB + 40.0);
        assert_eq!(step(&b), 2.0 * (STRIP_THUMB + 40.0));
        // Back never covers the tabs: it drops its key, then itself (Esc still goes back).
        for (g, w) in [(&b, w), (&drawn(480.0, false, 1.0, true), 480.0)] {
            let tabs = hit(g, Act::Tab(Category::Similar));
            if let Some((back, _)) = g.similar.hits.iter().find(|(_, a)| *a == Act::Back) {
                assert!(
                    back.x >= tabs.right() + 8.0 - 0.01,
                    "{w}: {back:?} {tabs:?}"
                );
                assert!(back.bottom() <= Metrics::new(g.text_scale).header());
            }
        }
    }

    /// The footer's buttons in one row with their keys when they fit, else without; wrapped
    /// when even that is too wide, each row within the window.
    #[test]
    fn footer_rows_wrap_without_keys() {
        let widths = [(100.0, 60.0); 4];
        let one = footer_rows(&widths, 32.0 + 4.0 * 100.0 + 3.0 * FOOTER_GAP);
        assert!(one.keys && one.rows.len() == 1 && one.rows[0].len() == 4);
        let short = footer_rows(&widths, 400.0);
        assert!(!short.keys && short.rows.len() == 1);
        assert_eq!(short.rows[0][1], (1, 16.0 + 60.0 + FOOTER_GAP, 60.0));
        let wrapped = footer_rows(&widths, 180.0);
        assert!(!wrapped.keys);
        assert_eq!(wrapped.rows.len(), 2);
        for row in &wrapped.rows {
            let &(_, x, bw) = row.last().unwrap();
            assert!(x + bw <= 180.0 - 16.0);
        }
        // Wider than the window: cut to it, alone on its row.
        let wide = footer_rows(&[(50.0, 40.0), (900.0, 800.0)], 300.0);
        assert_eq!(
            wide.rows,
            vec![vec![(0, 16.0, 40.0)], vec![(1, 16.0, 268.0)]]
        );
        // Before the first draw (no widths): one row, as tall as ever.
        let m = Metrics::new(1.0);
        let lay = review_layout(720.0, 900.0, m, &[]);
        assert_eq!(lay.footer_h, m.footer());
        assert_eq!(lay.strip_h, m.strip());
    }

    /// Every footer button is drawn (none dropped) inside the window and apart from the
    /// others, at text scale 2 and in narrow windows, mirrored right to left; a short window
    /// at text scale 2 makes the review compact so its large images keep room, and the
    /// layout outside drawing agrees with what was drawn.
    #[test]
    fn the_review_footer_keeps_every_button() {
        let acts: Vec<Act> = review_buttons().iter().map(|b| b.3).collect();
        for (size, ts) in [
            ((720.0, 600.0), 2.0),
            ((720.0, 450.0), 1.0),
            ((480.0, 450.0), 1.0),
            ((1600.0, 900.0), 1.0),
            ((420.0, 700.0), 2.0),
        ] {
            let (l, r) = (
                drawn_at(size, false, ts, true),
                drawn_at(size, true, ts, true),
            );
            let mut rects = Vec::new();
            for &act in &acts {
                let (a, b) = (hit(&l, act), hit(&r, act));
                assert!(mirrored(a, b, size.0), "{size:?} {act:?}");
                assert!(
                    a.x >= 0.0 && a.right() <= size.0 && a.bottom() <= size.1,
                    "{size:?} {act:?} {a:?}"
                );
                for o in &rects {
                    let i = a.intersect(o);
                    assert!(i.w * i.h == 0.0, "{size:?} {act:?} {a:?} {o:?}");
                }
                rects.push(a);
            }
            let lay = l.similar.review_layout(size.0, size.1);
            let keys = lay.footer.keys;
            assert_eq!(keys, size.0 >= 1600.0, "{size:?}");
            // The large images: where the layout outside drawing puts them.
            let cells = l.similar.review_cells(size.0, size.1, 2);
            for (pane, cell) in l.similar.panes.iter().zip(&cells) {
                assert!(cell.contains(pane.center()), "{size:?}");
            }
            // Below the strip, above the footer.
            let strip = hit(&l, Act::Focus(0));
            assert!(rects.iter().all(|b| b.y >= strip.bottom()), "{size:?}");
        }
        // 720 × 600 at text scale 2: compact (a small strip without its lines, a tighter
        // footer on two rows, no captions): the large images get about 144 px where the full
        // layout left its 40 px minimum.
        let g = drawn_at((720.0, 600.0), false, 2.0, true);
        let lay = g.similar.review_layout(720.0, 600.0);
        assert!(!lay.strip_text && lay.footer.rows.len() == 2 && lay.caption == 0.0);
        assert_eq!(hit(&g, Act::Focus(0)).h, STRIP_THUMB_COMPACT);
        assert!(lay.main.h >= 140.0, "{lay:?}");
        assert!(
            g.similar.panes.iter().all(|p| p.h >= 130.0),
            "{:?}",
            g.similar.panes
        );
        // Text scale 1 in the shots' window: as before (the strip with its lines).
        let lay = drawn(720.0, false, 1.0, true)
            .similar
            .review_layout(720.0, 450.0);
        assert!(lay.strip_text && lay.footer.rows.len() == 1 && lay.caption == CAPTION_H);
    }

    /// `[` and `]` step the sensitivity as ← and → do: the knob moves the way the bracket
    /// points, mirrored right to left.
    #[test]
    fn brackets_follow_the_visual_direction() {
        for rtl in [false, true] {
            assert_eq!(bracket_step(']', rtl), slider_key_step(true, rtl));
            assert_eq!(bracket_step('[', rtl), slider_key_step(false, rtl));
        }
        // Left to right `]` loosens; right to left (strict on the right) it tightens.
        assert!(bracket_step(']', false) > 0.0 && bracket_step('[', false) < 0.0);
        assert!(bracket_step(']', true) < 0.0 && bracket_step('[', true) > 0.0);
    }

    /// Right to left, the options keep their reading order (the first member first) and
    /// labels; their bounds are where they are drawn, the first on the right.
    #[test]
    fn right_to_left_keeps_the_a11y_order() {
        use accesskit::{NodeId, Role, TreeUpdate};
        let tree_of = |g: &Gallery| {
            let mut root = accesskit::Node::new(Role::Window);
            root.set_label("x");
            let mut tree = TreeUpdate {
                nodes: vec![(NodeId(1), root)],
                tree: None,
                tree_id: accesskit::TreeId::ROOT,
                focus: NodeId(1),
            };
            assert!(g.similar_a11y(&mut tree, 1.0));
            tree
        };
        let node = |t: &TreeUpdate, id: u64| {
            t.nodes
                .iter()
                .find(|(i, _)| *i == NodeId(id))
                .map(|(_, n)| n.clone())
                .expect("node")
        };
        let base = A11Y_BASE + A11Y_OFFSET;
        for review in [false, true] {
            let (l, r) = (
                drawn(720.0, false, 1.0, review),
                drawn(720.0, true, 1.0, review),
            );
            let (tl, tr) = (tree_of(&l), tree_of(&r));
            let n = if review { 3 } else { 1 };
            let order: Vec<NodeId> = (1..=n).map(|k| NodeId(base + k)).collect();
            assert_eq!(node(&tr, base).children(), &order[..]);
            assert_eq!(node(&tl, base).children(), &order[..]);
            for k in 1..=n {
                let (a, b) = (node(&tl, base + k), node(&tr, base + k));
                assert_eq!(a.label(), b.label());
                let (ba, bb) = (a.bounds().expect("bounds"), b.bounds().expect("bounds"));
                assert!(((720.0 - ba.x1) - bb.x0).abs() < 0.01, "{k}: mirrored");
            }
            if review {
                let x = |k: u64| node(&tr, base + k).bounds().unwrap().x0;
                assert!(x(1) > x(2) && x(2) > x(3), "first on the right");
            }
            assert_eq!(tr.focus, tl.focus);
        }
    }

    /// The list's footer keeps every button (none dropped) inside the window, apart from
    /// each other and below the list's rows, at text scale 2 and in narrow windows, mirrored
    /// right to left; their keys and the key hints only when all fit on one row.
    #[test]
    fn the_list_footer_keeps_every_button() {
        let acts = [Act::AcceptAll, Act::ActionMenu, Act::Unskip];
        for (size, ts, rows) in [
            ((720.0, 450.0), 2.0, 2),
            ((720.0, 560.0), 2.0, 2),
            ((420.0, 700.0), 2.0, 3),
            ((480.0, 450.0), 1.0, 2),
            ((1600.0, 900.0), 1.0, 1),
        ] {
            let (l, r) = (
                drawn_skipped(size, false, ts),
                drawn_skipped(size, true, ts),
            );
            let row = hit(&l, Act::Open(0));
            let mut rects: Vec<Rect> = Vec::new();
            for act in acts {
                let (a, b) = (hit(&l, act), hit(&r, act));
                assert!(mirrored(a, b, size.0), "{size:?} {act:?}");
                assert!(
                    a.x >= 0.0 && a.right() <= size.0 && a.bottom() <= size.1,
                    "{size:?} {act:?} {a:?}"
                );
                assert!(a.y >= row.bottom(), "{size:?} {act:?} {a:?} {row:?}");
                for o in &rects {
                    let i = a.intersect(o);
                    assert!(i.w * i.h == 0.0, "{size:?} {act:?} {a:?} {o:?}");
                }
                rects.push(a);
            }
            let mut ys: Vec<f32> = rects.iter().map(|r| r.y).collect();
            ys.dedup();
            ys.sort_by(f32::total_cmp);
            ys.dedup();
            assert_eq!(ys.len(), rows, "{size:?}: rows");
            // The list keeps a whole row where the header, slider and two footer rows leave
            // room for one (720 × 450 at text scale 2 does not: the footer tightens and the
            // row scrolls).
            if size != (720.0, 450.0) {
                assert!(row.h >= Metrics::new(ts).row() - 8.0, "{size:?} {row:?}");
            }
        }
    }

    /// A footer button's key shows in a tooltip above the footer, covering none of its rows
    /// (the review's second row too), inside the window; mirrored right to left.
    #[test]
    fn footer_tooltips_cover_no_row() {
        let review = |rtl: bool, act: Act| {
            let mut g = drawn_at((720.0, 560.0), rtl, 2.0, true);
            g.pointer = Some(hit(&g, act).center());
            redraw(&mut g);
            g
        };
        let footer_top = |g: &Gallery, acts: &[Act]| {
            acts.iter()
                .map(|&a| hit(g, a).y)
                .fold(f32::INFINITY, f32::min)
        };
        let acts: Vec<Act> = review_buttons().iter().map(|b| b.3).collect();
        for act in [Act::Compare, Act::Set(Decision::Keep), Act::Zoom] {
            let (l, r) = (review(false, act), review(true, act));
            let (tl, s) = l.similar.tip.clone().expect("tooltip");
            assert!(s.contains("  ("), "{s}");
            let tr = r.similar.tip.clone().expect("tooltip").0;
            assert!(tl.bottom() <= footer_top(&l, &acts) - 1.0, "{act:?} {tl:?}");
            for &a in &acts {
                let i = tl.intersect(&hit(&l, a));
                assert!(i.w * i.h == 0.0, "{act:?} covers {a:?}");
            }
            assert!(tl.x >= 8.0 && tl.right() <= 720.0 - 8.0 && tl.y >= 0.0);
            assert!(mirrored(tl, tr, 720.0), "{act:?} {tl:?} {tr:?}");
        }
        // The list's footer: the same.
        let mut g = drawn_skipped((720.0, 450.0), false, 2.0);
        g.pointer = Some(hit(&g, Act::AcceptAll).center());
        redraw(&mut g);
        let (t, s) = g.similar.tip.clone().expect("tooltip");
        assert!(s.ends_with("(A)"), "{s}");
        let acts = [Act::AcceptAll, Act::ActionMenu, Act::Unskip];
        assert!(t.bottom() <= footer_top(&g, &acts) - 1.0, "{t:?}");
        // No tooltip for a button without a key.
        g.pointer = Some(hit(&g, Act::Unskip).center());
        redraw(&mut g);
        assert!(g.similar.tip.is_none());
    }

    /// The most compact review leaves the captions out (the images keep their room): the
    /// caption of the large image under the pointer shows in a tooltip below it, inside the
    /// window, and the members' accessible descriptions give their size.
    #[test]
    fn a_compact_review_keeps_the_captions_reachable() {
        let mut g = drawn_at((720.0, 600.0), false, 2.0, true);
        assert_eq!(g.similar.review_layout(720.0, 600.0).caption, 0.0);
        let pane = g.similar.panes[0];
        g.pointer = Some(pane.center());
        redraw(&mut g);
        let (t, s) = g.similar.tip.clone().expect("caption tooltip");
        assert!(s.contains("burst-1.jpg") && s.contains("800 × 600"), "{s}");
        assert!(
            t.y >= pane.bottom() && t.bottom() <= 600.0,
            "{t:?} {pane:?}"
        );
        // With captions, no tooltip.
        let mut g = drawn_at((720.0, 900.0), false, 2.0, true);
        assert!(g.similar.review_layout(720.0, 900.0).caption > 0.0);
        g.pointer = Some(g.similar.panes[0].center());
        redraw(&mut g);
        assert!(g.similar.tip.is_none());
        let mut root = accesskit::Node::new(accesskit::Role::Window);
        root.set_label("x");
        let mut tree = accesskit::TreeUpdate {
            nodes: vec![(accesskit::NodeId(1), root)],
            tree: None,
            tree_id: accesskit::TreeId::ROOT,
            focus: accesskit::NodeId(1),
        };
        assert!(g.similar_a11y(&mut tree, 1.0));
        let id = accesskit::NodeId(A11Y_BASE + A11Y_OFFSET + 1);
        let n = &tree.nodes.iter().find(|(i, _)| *i == id).expect("member").1;
        assert!(n.description().is_some_and(|d| d.contains("800 × 600")));
    }

    /// The similar view's AccessKit tree over a bare window.
    fn a11y_tree(g: &Gallery) -> accesskit::TreeUpdate {
        use accesskit::{NodeId, Role, TreeUpdate};
        let mut root = accesskit::Node::new(Role::Window);
        root.set_label("x");
        let mut tree = TreeUpdate {
            nodes: vec![(NodeId(1), root)],
            tree: None,
            tree_id: accesskit::TreeId::ROOT,
            focus: NodeId(1),
        };
        assert!(g.similar_a11y(&mut tree, 1.0));
        // As the adapter sends it (`a11y.rs`).
        specular_widgets::strip_isolates(&mut tree);
        tree
    }

    fn a11y_node(t: &accesskit::TreeUpdate, id: u64) -> &accesskit::Node {
        &t.nodes
            .iter()
            .find(|(i, _)| i.0 == id)
            .unwrap_or_else(|| panic!("node {id:#x}"))
            .1
    }

    /// The node of control `a`.
    fn control_node(t: &accesskit::TreeUpdate, a: Act) -> &accesskit::Node {
        let i = CONTROLS.iter().position(|c| *c == a).expect("a control");
        a11y_node(t, A11Y_BASE + A11Y_OFFSET + A11Y_CONTROLS + i as u64)
    }

    fn same_rect(a: accesskit::Rect, r: Rect) -> bool {
        (a.x0 - f64::from(r.x)).abs() < 0.01
            && (a.y0 - f64::from(r.y)).abs() < 0.01
            && (a.x1 - f64::from(r.right())).abs() < 0.01
            && (a.y1 - f64::from(r.bottom())).abs() < 0.01
    }

    /// The group list's controls are nodes in reading order: the tabs (the open one
    /// selected), the summary, the sharpness check's progress, its Cancel, Done, the slider,
    /// the list,
    /// and the footer's buttons, each named without its key, its key as the shortcut, where
    /// it was drawn and pressable; in both directions.
    #[test]
    fn the_list_controls_are_buttons_in_the_tree() {
        use accesskit::{Action, Role};
        let base = A11Y_BASE + A11Y_OFFSET;
        for rtl in [false, true] {
            let g = drawn_skipped((900.0, 600.0), rtl, 1.0);
            let t = a11y_tree(&g);
            let dialog = a11y_node(&t, base - 1);
            let order: Vec<u64> = dialog.children().iter().map(|i| i.0 - base).collect();
            let c = |a: Act| A11Y_CONTROLS + CONTROLS.iter().position(|c| *c == a).unwrap() as u64;
            assert_eq!(
                order,
                vec![
                    A11Y_TABS,
                    A11Y_SUMMARY,
                    A11Y_PROGRESS,
                    c(Act::Stop),
                    c(Act::Close),
                    c(Act::Slider),
                    0,
                    c(Act::AcceptAll),
                    c(Act::ActionMenu),
                    c(Act::Unskip),
                ],
                "rtl {rtl}"
            );
            let tabs = a11y_node(&t, base + A11Y_TABS);
            assert_eq!(tabs.role(), Role::TabList);
            assert_eq!(tabs.children().len(), 2);
            let sim = control_node(&t, Act::Tab(Category::Similar));
            assert_eq!(sim.role(), Role::Tab);
            assert!(sim.is_selected() == Some(true));
            assert_eq!(
                control_node(&t, Act::Tab(Category::Duplicates)).is_selected(),
                Some(false)
            );
            for (a, label, key) in [
                (Act::AcceptAll, t!("similar-accept-all"), Some("A")),
                (Act::Unskip, t!("similar-show-skipped"), Some("K")),
                (Act::Close, t!("similar-done"), Some("Esc")),
            ] {
                let n = control_node(&t, a);
                assert_eq!(n.role(), Role::Button, "{a:?}");
                assert_eq!(n.label(), Some(label.as_str()));
                assert_eq!(n.keyboard_shortcut(), key);
                assert!(n.supports_action(Action::Click) && !n.is_disabled());
                assert!(same_rect(n.bounds().expect("bounds"), hit(&g, a)), "{a:?}");
            }
            let menu = control_node(&t, Act::ActionMenu);
            assert!(menu.label().is_some_and(|l| l.starts_with("Discard: ")));
            assert_eq!(menu.keyboard_shortcut(), Some("D"));
            let summary = a11y_node(&t, base + A11Y_SUMMARY);
            assert_eq!(summary.role(), Role::Label);
            assert!(
                summary
                    .value()
                    .is_some_and(|v| v.contains("1 skipped") && !v.contains('·'))
            );
        }
    }

    /// The sensitivity slider is a slider in percent, 0 (strict) to 100 (loose), in steps of
    /// the arrow keys', set, raised and lowered by screen readers (not clicked).
    #[test]
    fn the_sensitivity_slider_is_a_slider() {
        use accesskit::{Action, Orientation, Role};
        let mut g = drawn(900.0, false, 1.0, false);
        g.similar.set_sensitivity_for_shots(0.35);
        redraw(&mut g);
        let t = a11y_tree(&g);
        let n = control_node(&t, Act::Slider);
        assert_eq!(n.role(), Role::Slider);
        assert_eq!(n.label(), Some(t!("similar-sensitivity").as_str()));
        assert_eq!(n.numeric_value(), Some(35.0));
        assert_eq!(n.min_numeric_value(), Some(0.0));
        assert_eq!(n.max_numeric_value(), Some(100.0));
        assert_eq!(n.numeric_value_step(), Some(5.0));
        assert_eq!(n.orientation(), Some(Orientation::Horizontal));
        assert!(n.description().is_some_and(|d| d.starts_with("Strict")));
        for a in [Action::Increment, Action::Decrement, Action::SetValue] {
            assert!(n.supports_action(a), "{a:?}");
        }
        assert!(!n.supports_action(Action::Click));
        assert!(same_rect(n.bounds().unwrap(), hit(&g, Act::Slider)));
        // Duplicates have no slider.
        let mut g = drawn(900.0, false, 1.0, false);
        g.similar.view.as_mut().unwrap().cat = Category::Duplicates;
        redraw(&mut g);
        let t = a11y_tree(&g);
        let i = CONTROLS.iter().position(|c| *c == Act::Slider).unwrap() as u64;
        let id = A11Y_BASE + A11Y_OFFSET + A11Y_CONTROLS + i;
        assert!(!t.nodes.iter().any(|(n, _)| n.0 == id));
    }

    /// The scan's progress is a progress bar with the count as its value, announced
    /// politely by a name that changes only at the quarters.
    #[test]
    fn the_scan_progress_is_announced_at_the_quarters() {
        use accesskit::{Live, Role};
        assert_eq!(said_progress(0, 12), 0);
        assert_eq!(said_progress(2, 12), 0);
        assert_eq!(said_progress(7, 12), 6);
        assert_eq!(said_progress(12, 12), 12);
        assert_eq!(said_progress(20, 12), 12);
        assert_eq!(said_progress(5, 0), 0);
        assert_eq!(said_progress(999, 1000), 750);
        // Four names in a pass of 1000.
        let names: std::collections::BTreeSet<usize> =
            (0..1000).map(|d| said_progress(d, 1000)).collect();
        assert_eq!(names.len(), 4);
        let g = drawn(900.0, false, 1.0, false);
        let t = a11y_tree(&g);
        let n = a11y_node(&t, A11Y_BASE + A11Y_OFFSET + A11Y_PROGRESS);
        assert_eq!(n.role(), Role::ProgressIndicator);
        assert_eq!(n.live(), Some(Live::Polite));
        assert_eq!(n.numeric_value(), Some(7.0));
        assert_eq!(n.max_numeric_value(), Some(12.0));
        assert_eq!(n.label(), Some("Checking sharpness… 6 of 12"));
        assert_eq!(n.value(), Some("Checking sharpness… 7 of 12"));
        // No scan, no progress.
        let mut g = drawn(900.0, false, 1.0, false);
        g.similar.scan = None;
        redraw(&mut g);
        let t = a11y_tree(&g);
        assert!(
            !t.nodes
                .iter()
                .any(|(i, _)| i.0 == A11Y_BASE + A11Y_OFFSET + A11Y_PROGRESS)
        );
    }

    /// The review's footer buttons are nodes in its order with their keys; Back has Esc;
    /// Compare and 100 % say whether they are on; Discard is unavailable (with the reason)
    /// while the focused member is protected.
    #[test]
    fn the_review_footer_buttons_are_in_the_tree() {
        use accesskit::{Action, Role, Toggled};
        let base = A11Y_BASE + A11Y_OFFSET;
        for text in [1.0, 2.0] {
            let g = drawn_at((720.0, 600.0), false, text, true);
            let t = a11y_tree(&g);
            let dialog = a11y_node(&t, base - 1);
            let footer: Vec<Act> = dialog
                .children()
                .iter()
                .filter_map(|i| {
                    let k = i.0.checked_sub(base + A11Y_CONTROLS)?;
                    CONTROLS.get(k as usize).copied()
                })
                .filter(|a| CONTROLS[FOOTER_CONTROLS].contains(a))
                .collect();
            let want: Vec<Act> = review_buttons().iter().map(|b| b.3).collect();
            assert_eq!(footer, want, "text {text}");
            for (label, key, _, a) in review_buttons() {
                let n = control_node(&t, a);
                assert_eq!(n.role(), Role::Button);
                assert_eq!(n.label(), Some(label.as_str()));
                assert_eq!(n.keyboard_shortcut(), Some(key));
                // AT-SPI has no shortcut property: the key is in the description too.
                assert_eq!(n.description(), Some(format!("{label} ({key})").as_str()));
                assert!(n.supports_action(Action::Click) && !n.is_disabled());
                assert!(same_rect(n.bounds().unwrap(), hit(&g, a)), "{a:?}");
            }
            // The fixture compares the first two.
            assert_eq!(
                control_node(&t, Act::Compare).toggled(),
                Some(Toggled::True)
            );
            assert_eq!(control_node(&t, Act::Zoom).toggled(), Some(Toggled::False));
            let back = control_node(&t, Act::Back);
            assert_eq!(back.label(), Some(t!("similar-back").as_str()));
            assert_eq!(back.keyboard_shortcut(), Some("Esc"));
            assert_eq!(control_node(&t, Act::Close).keyboard_shortcut(), None);
            assert_eq!(control_node(&t, Act::Close).description(), None);
        }
        // Zoomed; not comparing.
        let mut g = drawn_at((720.0, 600.0), false, 1.0, true);
        g.similar
            .view
            .as_mut()
            .unwrap()
            .review
            .as_mut()
            .unwrap()
            .zoom = 2.0;
        redraw(&mut g);
        let t = a11y_tree(&g);
        assert_eq!(control_node(&t, Act::Zoom).toggled(), Some(Toggled::True));
        g.similar
            .view
            .as_mut()
            .unwrap()
            .review
            .as_mut()
            .unwrap()
            .compare = None;
        g.similar
            .view
            .as_mut()
            .unwrap()
            .review
            .as_mut()
            .unwrap()
            .zoom = 1.0;
        redraw(&mut g);
        let t = a11y_tree(&g);
        assert_eq!(
            control_node(&t, Act::Compare).toggled(),
            Some(Toggled::False)
        );
        // The focused member protected: Discard is unavailable, and says why.
        let mut g = drawn_at((720.0, 600.0), false, 1.0, true);
        let gi = {
            let v = g.similar.view.as_ref().unwrap();
            g.similar
                .group_of_key(v.cat, v.review.as_ref().unwrap().group)
                .unwrap()
        };
        let m = g.similar.sims[gi].members[0] as usize;
        g.similar.results.as_mut().unwrap().items[m].protect = Some(Protect::Rated);
        redraw(&mut g);
        let t = a11y_tree(&g);
        let n = control_node(&t, Act::Set(Decision::Discard));
        assert!(n.is_disabled() && !n.supports_action(Action::Click));
        assert_eq!(
            n.description(),
            Some(t!("similar-toast-protected").as_str())
        );
        assert!(!control_node(&t, Act::Set(Decision::Keep)).is_disabled());
    }

    fn review_of(g: &mut Gallery) -> &mut Review {
        g.similar
            .view
            .as_mut()
            .and_then(|v| v.review.as_mut())
            .expect("a review")
    }

    /// "100 %" on a photo smaller than its pane (already shown at 100 % or larger) is
    /// unavailable: dimmed, with the reason in its hover tooltip and a11y description; a
    /// photo larger than the pane alone (where the zoom shows it) keeps it, and so does a
    /// zoomed review (the toggle turns the zoom off).
    #[test]
    fn zoom_on_a_small_photo_says_why_it_is_off() {
        use accesskit::{Action, Toggled};
        // 800 × 600 in 1600 × 1400 at 1×: the focused member alone has more than 800 px.
        // Not comparing (the zoom would show it alone).
        let mut g = drawn_at((1600.0, 1400.0), false, 1.0, true);
        review_of(&mut g).compare = None;
        redraw(&mut g);
        assert_eq!(
            g.similar_disabled(Act::Zoom),
            Some(t!("similar-zoom-already"))
        );
        let t = a11y_tree(&g);
        let n = control_node(&t, Act::Zoom);
        assert!(n.is_disabled() && !n.supports_action(Action::Click));
        assert_eq!(n.toggled(), Some(Toggled::False));
        let reason = specular_i18n::strip_isolates(&t!("similar-zoom-already"));
        assert_eq!(n.description(), Some(reason.as_str()));
        // Hovered: the reason in the tooltip.
        g.pointer = Some(hit(&g, Act::Zoom).center());
        redraw(&mut g);
        let tip = g.similar.tip.clone().expect("tooltip").1;
        assert!(tip.contains(&t!("similar-zoom-already")), "{tip}");
        // Comparing two side by side: each pane (784 px) is narrower than the photo.
        review_of(&mut g).compare = Some(vec![0, 1]);
        redraw(&mut g);
        assert_eq!(g.similar_disabled(Act::Zoom), None);
        // In a window where the photo is larger than its pane.
        let g = drawn_at((720.0, 600.0), false, 1.0, true);
        assert_eq!(g.similar_disabled(Act::Zoom), None);
        let t = a11y_tree(&g);
        assert!(control_node(&t, Act::Zoom).supports_action(Action::Click));
        // Zoomed (by the wheel or +): the toggle turns it off, whatever the size.
        let mut g = drawn_at((1600.0, 1400.0), false, 1.0, true);
        review_of(&mut g).zoom = 1.5;
        redraw(&mut g);
        assert_eq!(g.similar_disabled(Act::Zoom), None);
    }

    /// A member's description is its large image's caption after the name: the size, and
    /// while it shows large the zoom (as the caption says it) and the channel view; without
    /// bidi marks.
    #[test]
    fn members_describe_the_zoom_and_channel_view() {
        let base = A11Y_BASE + A11Y_OFFSET;
        let mut g = drawn_at((720.0, 600.0), false, 1.0, true);
        let rv = g.similar.view.as_mut().unwrap().review.as_mut().unwrap();
        rv.compare = Some(vec![0, 1]);
        rv.zoom = 2.0;
        redraw(&mut g);
        let t = a11y_tree(&g);
        let pane = g.similar.panes[0];
        let pct = specular_i18n::I18n::global()
            .localizer()
            .percent(f64::from(2.0 * pane.w / 800.0), 0);
        assert_eq!(
            a11y_node(&t, base + 1).description(),
            Some(format!("800 × 600, {pct}").as_str())
        );
        // Not shown large: the size alone.
        assert_eq!(a11y_node(&t, base + 3).description(), Some("800 × 600"));
        // In a channel view.
        let path = g
            .similar
            .path_of(g.similar.sims[0].members[0])
            .unwrap()
            .to_path_buf();
        g.quick_tex = vec![(path, TexId(1), pane, ChannelView::Red)];
        let rv = g.similar.view.as_mut().unwrap().review.as_mut().unwrap();
        rv.zoom = 1.0;
        redraw(&mut g);
        let t = a11y_tree(&g);
        let red = specular_widgets::channel_label(ChannelView::Red);
        assert_eq!(
            a11y_node(&t, base + 1).description(),
            Some(format!("800 × 600, {red}").as_str())
        );
        for (_, n) in &t.nodes {
            for s in [n.label(), n.description(), n.value()]
                .into_iter()
                .flatten()
            {
                assert!(!s.contains(['\u{2066}', '\u{2068}', '\u{2069}']), "{s:?}");
            }
        }
    }

    /// The group rows say which member is the best pick and why (and why the others go, when
    /// any is suggested for discarding), as their lines do.
    #[test]
    fn group_rows_name_the_best_pick() {
        let g = drawn(900.0, false, 1.0, false);
        let t = a11y_tree(&g);
        let row = a11y_node(&t, A11Y_BASE + A11Y_OFFSET + 1);
        let d = row.description().expect("description");
        let gr = &g.similar.sims[0];
        let k = lead(gr).unwrap();
        let name = &g.similar.results.as_ref().unwrap().items[gr.members[k] as usize].name;
        assert!(
            d.starts_with(&format!("{name}: {}", tr_reason(gr.picks[k].reason))),
            "{d}"
        );
    }
}
