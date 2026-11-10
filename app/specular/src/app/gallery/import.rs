//! Import from cameras, phones and memory cards: the new photos and videos of a card, with
//! their thumbnails, copied into dated folders (`specular_import` does the work).
//!
//! - Cards are watched on thread `io-cards` ([`specular_import::source::Watcher`]); one that
//!   arrives while Specular runs is offered in a toast ("Import from EOS_DIGITAL?"), opens
//!   this view, or nothing (Settings → Import).
//! - The view shows instead of the grid: a header, the new shots as thumbnails (the gallery's
//!   own thumbnails and atlas, wished for like the map's hover card), all selected, and a
//!   side panel with the destination, the folder and rename patterns, the event, what each
//!   photo gets (a metadata template, keywords, a Develop style), the options and the Import
//!   button. Shots imported before (the ledger) are not listed.
//! - The copy runs on thread `io-import`: progress, pause and cancel in the panel, or in a
//!   toast once hidden. The template's fields and the keywords go into each copied photo's
//!   XMP sidecar in the same job, once the copies are verified; the style goes on as a Develop
//!   batch when the copy ends. Then the destination opens in the gallery, the Library rescans
//!   the location it is in, and the card is ejected if asked.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use specular_gfx::winit::event::KeyEvent;
use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextBlock, TextStyle};
use specular_i18n::{I18n, t};
use specular_import::{
    Control, FolderPattern, Ledger, MediaKind, Options, Progress, RenamePattern, Report, RunError,
    Shot, Source,
};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_settings::{ImportOnCard, Origin};
use specular_theme::Palette;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};
use specular_widgets::{PaletteItem, accesskit};

use super::draw::{Frame, text_at, text_in, truncate};
use super::{A11Y_BASE, App, Gallery, item};

/// Import's palette codes.
pub(super) mod code {
    pub const START: u64 = 751;
    pub const SELECT_ALL: u64 = 752;
    pub const SELECT_NONE: u64 = 753;
    pub const CHOOSE: u64 = 754;
    pub const CLOSE: u64 = 755;

    pub fn owns(c: u64) -> bool {
        (750..800).contains(&c)
    }
}

/// The view's screen-reader nodes: the group, its controls, the status, then the tiles.
const A11Y_IMPORT: u64 = A11Y_BASE + (12 << 40);
const A11Y_STATUS: u64 = 90;
const A11Y_TILE: u64 = 100;
/// Tiles listed for screen readers at most (those on screen, and the focused one).
const A11Y_TILES: usize = 400;

/// The keys of the offer toasts' buttons (other offers use other bits).
const OFFER_KEYS: u64 = (1 << 62) | (1 << 60);

/// How often the card watcher looks for new mounts.
const WATCH_EVERY: Duration = Duration::from_secs(2);

const TITLE: TextStyle = TextStyle::new(15.0).weight(600.0);
const BODY: TextStyle = TextStyle::new(13.0);
const LABEL: TextStyle = TextStyle::new(12.0).weight(600.0);
const SMALL: TextStyle = TextStyle::new(11.0);
const BADGE: TextStyle = TextStyle::new(10.0).weight(650.0);

/// Header height at text scale 1, logical px.
const HEADER: f32 = 44.0;
/// The side panel's width at text scale 1.
const PANEL: f32 = 300.0;
/// A tile's side at text scale 1 (grown to fill the row).
const TILE: f32 = 132.0;
const GAP: f32 = 8.0;

/// The text fields of the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Field {
    Folder,
    Event,
    Rename,
    /// Keywords every photo gets (`a; b > c`).
    Keywords,
}

const FIELDS: [Field; 3] = [Field::Folder, Field::Event, Field::Rename];

/// What a press or Enter acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Hit {
    Close,
    NextSource,
    SelectAll,
    Choose,
    Field(Field),
    KeepPairs,
    Eject,
    Delete,
    Start,
    ConfirmYes,
    ConfirmNo,
    Pause,
    Cancel,
    Hide,
    /// The metadata template row (steps through None and the templates).
    Template,
    /// The Develop style row (steps through None and the styles).
    Style,
    Tile(usize),
}

impl Hit {
    /// The screen-reader id offset of a control (tiles have their own range).
    fn a11y(self) -> u64 {
        match self {
            Hit::Close => 1,
            Hit::NextSource => 2,
            Hit::SelectAll => 3,
            Hit::Choose => 4,
            Hit::Field(Field::Folder) => 5,
            Hit::Field(Field::Event) => 6,
            Hit::Field(Field::Rename) => 7,
            Hit::KeepPairs => 8,
            Hit::Eject => 9,
            Hit::Delete => 10,
            Hit::Start => 11,
            Hit::ConfirmYes => 12,
            Hit::ConfirmNo => 13,
            Hit::Pause => 14,
            Hit::Cancel => 15,
            Hit::Hide => 16,
            Hit::Template => 17,
            Hit::Style => 18,
            Hit::Field(Field::Keywords) => 19,
            Hit::Tile(i) => A11Y_TILE + i as u64,
        }
    }
}

/// A scan of a card on thread `io-import`.
struct ScanJob {
    cancel: Arc<AtomicBool>,
    progress: Receiver<(usize, usize)>,
    done: Receiver<Scanned>,
    last: (usize, usize),
}

/// What a scan found, with each shot's bytes still to copy.
struct Scanned {
    mount: PathBuf,
    shots: Vec<Shot>,
    bytes: Vec<u64>,
    already: usize,
    unreadable: usize,
}

/// A running import.
struct RunJob {
    ctl: Arc<Control>,
    progress: Receiver<Progress>,
    done: Receiver<Result<Report, RunError>>,
    last: Progress,
    /// Its toast once the view is hidden (id, when last updated).
    toast: Option<(u64, f64)>,
    /// The card to eject when it ends well.
    eject: Option<Source>,
    dest_root: PathBuf,
    source_name: String,
    /// The Develop style for the copied photos, applied when the copy ends.
    style: Option<String>,
}

/// The import view's state; `open` while it shows instead of the grid. The watcher, the
/// ledger and a running import outlive the view.
#[derive(Default)]
pub(in crate::app) struct ImportView {
    pub open: bool,
    /// The cards, cameras and phones with photos now (`None`: not looked yet).
    sources: Option<Vec<Source>>,
    /// The one shown (its mount).
    source: Option<PathBuf>,
    scan: Option<ScanJob>,
    /// The new shots of the source shown, oldest first, and the bytes each would copy.
    shots: Vec<Shot>,
    bytes: Vec<u64>,
    already: usize,
    unreadable: usize,
    /// The mount the shots are of (`None`: no scan has come back).
    scanned: Option<PathBuf>,
    selected: BTreeSet<usize>,
    /// The tile the keyboard is on.
    cursor: usize,
    focus: Option<Hit>,
    folder: LineEdit,
    event: LineEdit,
    rename: LineEdit,
    keywords: LineEdit,
    /// The metadata template and the Develop style each photo gets (by name; None: none),
    /// and the names to choose from (as of opening the view).
    template: Option<String>,
    style: Option<String>,
    templates: Vec<String>,
    styles: Vec<String>,
    /// The chosen destination (`None`: the setting, else Pictures).
    dest: Option<PathBuf>,
    keep_pairs: bool,
    eject: bool,
    delete: bool,
    /// Asking to confirm deleting from the card.
    confirm: bool,
    run: Option<RunJob>,
    /// Why the last start did not happen.
    error: Option<String>,
    /// Free bytes at the destination (looked up when it changes).
    free: Option<(PathBuf, Option<u64>)>,
    /// Thumbnails wanted this frame (for the gallery's wish list).
    pub wanted: Vec<(PathBuf, u32)>,
    hits: Vec<(Rect, Hit)>,
    /// The fields' text as drawn (for presses and drags): block and its x.
    drawn: HashMap<Field, (Arc<TextBlock>, f32)>,
    /// Where the focused field's input method candidate window opens (as last drawn).
    ime: Option<Rect>,
    grid: Rect,
    /// The side panel's scrolling part (mirrored), how far it is scrolled, and the clip
    /// that hits are cut to while drawing.
    parea: Rect,
    pscroll: f32,
    clip: Option<Rect>,
    cols: usize,
    scroll: f32,
    scroll_to_cursor: bool,
    ledger: Option<Arc<Mutex<Ledger>>>,
    watcher: Option<specular_import::source::Watcher>,
    arrivals: Option<Receiver<Vec<Source>>>,
    offers: HashMap<u64, PathBuf>,
    next_offer: u64,
    picking: Option<Receiver<PathBuf>>,
    ejecting: Option<Receiver<String>>,
}

impl ImportView {
    fn field(&self, f: Field) -> &LineEdit {
        match f {
            Field::Folder => &self.folder,
            Field::Event => &self.event,
            Field::Rename => &self.rename,
            Field::Keywords => &self.keywords,
        }
    }

    fn field_mut(&mut self, f: Field) -> &mut LineEdit {
        match f {
            Field::Folder => &mut self.folder,
            Field::Event => &mut self.event,
            Field::Rename => &mut self.rename,
            Field::Keywords => &mut self.keywords,
        }
    }

    /// The template row's choice stepped by `d` (through None and the templates).
    fn step_template(&mut self, d: isize) {
        self.template = step_choice(&self.templates, self.template.as_deref(), d);
    }

    fn step_style(&mut self, d: isize) {
        self.style = step_choice(&self.styles, self.style.as_deref(), d);
    }

    /// What a choice row shows, and its label (for the screen reader too).
    fn choice(&self, h: Hit) -> (String, String) {
        let (label, v) = match h {
            Hit::Template => (t!("import-template"), self.template.clone()),
            _ => (t!("import-style"), self.style.clone()),
        };
        (label, v.unwrap_or_else(|| t!("import-choice-none")))
    }

    fn focused_field(&self) -> Option<Field> {
        match self.focus {
            Some(Hit::Field(f)) => Some(f),
            _ => None,
        }
    }

    fn current(&self) -> Option<&Source> {
        let m = self.source.as_ref()?;
        self.sources.as_ref()?.iter().find(|s| &s.mount == m)
    }

    fn source_name(&self) -> String {
        self.current().map(|s| s.name.clone()).unwrap_or_default()
    }

    /// Bytes of the selected shots.
    fn selected_bytes(&self) -> u64 {
        self.selected
            .iter()
            .filter_map(|&i| self.bytes.get(i))
            .sum()
    }

    /// The patterns as typed, or why they do not parse.
    fn patterns(&self) -> Result<(FolderPattern, Option<RenamePattern>), String> {
        let f = FolderPattern::parse(self.folder.text())
            .map_err(|e| t!("import-pattern-error", error = e.to_string()))?;
        let r = RenamePattern::parse(self.rename.text())
            .map_err(|e| t!("import-pattern-error", error = e.to_string()))?;
        Ok((f, r))
    }

    fn running(&self) -> bool {
        self.run.is_some()
    }

    fn ready(&self) -> bool {
        self.scan.is_none() && self.scanned.is_some() && self.scanned == self.source
    }

    /// The focus order: the controls as drawn, the grid as one stop.
    fn focusables(&self) -> Vec<Hit> {
        let mut out = Vec::new();
        let mut grid = false;
        for (_, h) in &self.hits {
            match h {
                Hit::Tile(_) => {
                    if !grid {
                        grid = true;
                        out.push(Hit::Tile(self.cursor));
                    }
                }
                h => out.push(*h),
            }
        }
        if !grid && !self.shots.is_empty() && !self.running() && !self.confirm {
            out.insert(out.len().min(3), Hit::Tile(self.cursor));
        }
        out
    }
}

/// `now` in `list` (None first) stepped by `d`, wrapping; a name no longer listed is None.
fn step_choice(list: &[String], now: Option<&str>, d: isize) -> Option<String> {
    let n = list.len() as isize + 1;
    let at = now
        .and_then(|v| list.iter().position(|x| x == v))
        .map_or(0, |i| i as isize + 1);
    let to = (at + d).rem_euclid(n);
    (to > 0).then(|| list[to as usize - 1].clone())
}

/// The folder imports go in when none is chosen.
fn default_destination(setting: &Option<PathBuf>) -> PathBuf {
    setting
        .clone()
        .unwrap_or_else(specular_platform::integration::user_pictures_dir)
}

/// Free bytes where `dir` is (or would be made): the nearest folder that exists.
fn free_at(dir: &Path) -> Option<u64> {
    let mut d = Some(dir);
    while let Some(p) = d {
        if p.is_dir() {
            return specular_platform::volumes::free_space(p);
        }
        d = p.parent();
    }
    None
}

fn size_text(bytes: u64) -> String {
    I18n::global()
        .localizer()
        .bytes(bytes, specular_i18n::ByteUnits::Decimal)
}

fn date_text(d: [u32; 6]) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        d[0], d[1], d[2], d[3], d[4]
    )
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// "Importing 3 of 20 · 1.2 GB of 4 GB · about 10 seconds left".
fn progress_text(p: &Progress, paused: bool) -> String {
    let (done, total) = (p.files_done as i64, p.files_total as i64);
    if paused {
        return t!("import-paused", done = done, total = total);
    }
    let size = format!("{} / {}", size_text(p.bytes_done), size_text(p.bytes_total));
    match p.eta {
        Some(eta) if p.files_done < p.files_total => t!(
            "import-progress-eta",
            done = done,
            total = total,
            size = size,
            time = crate::app::convert::eta_text(eta)
        ),
        _ => t!("import-progress", done = done, total = total, size = size),
    }
}

/// The plain-language message for a destination without room.
fn no_space_text(dest: &Path, need: u64, free: u64) -> String {
    t!(
        "import-no-space",
        folder = specular_i18n::isolate(&dest.display().to_string()),
        need = size_text(need),
        free = size_text(free)
    )
}

/// The deepest folder holding every one of `dirs`.
fn common_folder(dirs: &BTreeSet<PathBuf>) -> Option<PathBuf> {
    let mut it = dirs.iter();
    let mut common = it.next()?.clone();
    for d in it {
        while !d.starts_with(&common) {
            if !common.pop() {
                return None;
            }
        }
    }
    Some(common)
}

impl Gallery {
    fn import_header(&self) -> f32 {
        (HEADER * self.text_scale.max(1.0)).round()
    }

    /// Draws the import view over the gallery; true while it wants another frame soon
    /// (scanning or copying).
    pub(super) fn draw_import(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) -> bool {
        if !self.import.open {
            return false;
        }
        let pal = f.pal;
        let (w, h) = (self.vw, self.vh);
        let ts = self.text_scale.clamp(1.0, 3.0);
        let head = self.import_header();
        self.import.hits.clear();
        self.import.wanted.clear();
        dl.rect(Rect::new(0.0, 0.0, w, h), 0.0, pal.canvas);
        let panel_w = (PANEL * ts).min(w * 0.5).round();
        let grid = Rect::new(0.0, head, (w - panel_w).max(1.0), (h - head).max(1.0));
        let panel = Rect::new(w - panel_w, head, panel_w, (h - head).max(1.0));
        self.import.grid = self.flip(grid);
        self.draw_import_header(dl, text, pal);
        self.draw_import_grid(dl, text, pal, grid);
        self.draw_import_panel(dl, text, pal, panel);
        // Focus ring on the focused control.
        if let Some(fh) = self.import.focus
            && !matches!(fh, Hit::Field(_))
            && let Some(r) = self
                .import
                .hits
                .iter()
                .find(|(_, h)| match (h, fh) {
                    (Hit::Tile(a), Hit::Tile(b)) => *a == b,
                    (a, b) => *a == b,
                })
                .map(|x| x.0)
        {
            dl.stroke_rect(r.inset_xy(-2.0, -2.0), 8.0, pal.focus_ring, 2.0);
        }
        self.import.scan.is_some() || self.import.run.is_some()
    }

    fn draw_import_header(&mut self, dl: &mut DrawList, text: &mut Text, pal: &Palette) {
        let w = self.vw;
        let hh = self.import_header();
        let ts = self.text_scale.clamp(1.0, 3.0);
        dl.rect(Rect::new(0.0, 0.0, w, hh), 0.0, pal.surface);
        dl.rect(Rect::new(0.0, hh - 1.0, w, 1.0), 0.0, pal.separator);
        let (title, body) = (self.ts(TITLE), self.ts(BODY));
        let bh = (28.0 * ts).round();
        let y = ((hh - bh) * 0.5).round();
        let mut x = w - 12.0;
        let mut buttons: Vec<(Hit, String, bool)> = vec![(Hit::Close, t!("import-close"), false)];
        let many = self.import.sources.as_ref().is_some_and(|s| s.len() > 1);
        if many && !self.import.running() {
            buttons.push((Hit::NextSource, t!("import-source-next"), false));
        }
        if self.import.ready() && !self.import.shots.is_empty() && !self.import.running() {
            let all = self.import.selected.len() == self.import.shots.len();
            buttons.push((
                Hit::SelectAll,
                if all {
                    t!("import-select-none")
                } else {
                    t!("import-select-all")
                },
                false,
            ));
        }
        for (hit, label, accent) in buttons {
            let bw = text.measure(&label, body) + 24.0;
            x -= bw;
            let r = self.flip(Rect::new(x, y, bw, bh));
            x -= 6.0;
            self.import_button(dl, text, pal, r, &label, hit, accent, false);
        }
        let trailing = x;
        let lead = 16.0;
        let tt = t!("import-title");
        let tw = text.measure(&tt, title);
        text_at(
            text,
            dl,
            &tt,
            title,
            self.flip(Rect::new(lead, 0.0, tw, hh)),
            pal.text,
            self.rtl,
        );
        let name = self.import.source_name();
        let iv = &self.import;
        let mut summary = if name.is_empty() {
            String::new()
        } else {
            t!("import-from", name = specular_i18n::isolate(&name))
        };
        if iv.ready() && !iv.shots.is_empty() {
            summary = specular_i18n::dot_join(&[
                summary,
                t!(
                    "import-selected",
                    count = iv.selected.len() as i64,
                    total = iv.shots.len() as i64
                ),
            ]);
        }
        let sx = lead + tw + 12.0;
        let room = (trailing - sx - 8.0).max(0.0);
        let summary = truncate(text, &summary, body, room);
        text_at(
            text,
            dl,
            &summary,
            body,
            self.flip(Rect::new(sx, 0.0, room, hh)),
            pal.text_muted,
            self.rtl,
        );
    }

    /// A push button `r` (already mirrored) that records its hit.
    #[allow(clippy::too_many_arguments)]
    fn import_button(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        r: Rect,
        label: &str,
        hit: Hit,
        accent: bool,
        off: bool,
    ) {
        let body = self.ts(BODY);
        let hover = !off && self.pointer.is_some_and(|p| r.contains(p));
        let (fill, fg) = match (accent && !off, hover) {
            (true, _) => (pal.accent_fill, pal.accent_text),
            (false, true) => (pal.surface_hover, pal.text),
            (false, false) => (pal.surface_raised, pal.text),
        };
        dl.rect_border(r, 6.0, fill, pal.separator, if accent { 0.0 } else { 1.0 });
        let fg = if off { pal.text_faint } else { fg };
        let label = truncate(text, label, body, r.w - 12.0);
        text_in(text, dl, &label, body, r, fg);
        if !off {
            self.import_push_hit(r, hit);
        }
    }

    /// Records what a press in `r` acts on: the part of it inside the clip in effect.
    fn import_push_hit(&mut self, r: Rect, hit: Hit) {
        let r = match self.import.clip {
            Some(c) => r.intersect(&c),
            None => r,
        };
        if r.w > 0.0 && r.h > 0.0 {
            self.import.hits.push((r, hit));
        }
    }

    /// The message in the middle of the grid when there are no tiles.
    fn import_message(&self) -> Option<String> {
        let iv = &self.import;
        let name = specular_i18n::isolate(&iv.source_name());
        if iv.sources.as_ref().is_some_and(Vec::is_empty) || iv.source.is_none() {
            return Some(t!("import-no-source"));
        }
        if let Some(s) = &iv.scan {
            return Some(if s.last.1 == 0 {
                t!("import-scanning-start")
            } else {
                t!(
                    "import-scanning",
                    done = s.last.0 as i64,
                    total = s.last.1 as i64
                )
            });
        }
        if !iv.ready() {
            return Some(t!("import-scanning-start"));
        }
        if iv.shots.is_empty() {
            return Some(if iv.already > 0 {
                t!("import-none-new", name = name)
            } else {
                t!("import-empty", name = name)
            });
        }
        None
    }

    fn draw_import_grid(&mut self, dl: &mut DrawList, text: &mut Text, pal: &Palette, grid: Rect) {
        let ts = self.text_scale.clamp(1.0, 3.0);
        if let Some(msg) = self.import_message() {
            let body = self.ts(BODY);
            let b = text.layout(&msg, body, Some((grid.w - 48.0).max(40.0)), Align::Center);
            let r = self.flip(grid);
            text.draw(
                dl,
                &b,
                r.x + (r.w - b.width) * 0.5,
                r.y + (r.h - b.height) * 0.5,
                pal.text_muted,
            );
            self.import.cols = 1;
            return;
        }
        let gap = GAP * ts;
        let target = TILE * ts.min(1.6);
        let cols = (((grid.w - gap) / (target + gap)).round() as usize).max(1);
        let cell = ((grid.w - gap) / cols as f32 - gap).floor().max(16.0);
        let n = self.import.shots.len();
        let rows = n.div_ceil(cols);
        let content = rows as f32 * (cell + gap) + gap;
        let max_scroll = (content - grid.h).max(0.0);
        let iv = &mut self.import;
        iv.cols = cols;
        iv.cursor = iv.cursor.min(n.saturating_sub(1));
        if iv.scroll_to_cursor {
            iv.scroll_to_cursor = false;
            let top = (iv.cursor / cols) as f32 * (cell + gap);
            if top < iv.scroll {
                iv.scroll = top;
            } else if top + cell + gap * 2.0 > iv.scroll + grid.h {
                iv.scroll = top + cell + gap * 2.0 - grid.h;
            }
        }
        iv.scroll = iv.scroll.clamp(0.0, max_scroll);
        let scroll = iv.scroll;
        let class = specular_thumbs::class_for(cell * self.scale);
        let small = self.ts(BADGE);
        dl.push_clip(self.flip(grid));
        self.import.clip = Some(self.flip(grid));
        let first_row = (scroll / (cell + gap)).floor() as usize;
        let last_row = ((scroll + grid.h) / (cell + gap)).ceil() as usize;
        for row in first_row..=last_row.min(rows.saturating_sub(1)) {
            for col in 0..cols {
                let i = row * cols + col;
                if i >= n {
                    break;
                }
                let r = Rect::new(
                    grid.x + gap + col as f32 * (cell + gap),
                    grid.y + gap + row as f32 * (cell + gap) - scroll,
                    cell,
                    cell,
                );
                let r = self.flip(r);
                let (path, pair, raw, video) = {
                    let s = &self.import.shots[i];
                    (
                        s.preview().to_path_buf(),
                        s.is_pair(),
                        s.has(MediaKind::Raw),
                        s.has(MediaKind::Video),
                    )
                };
                let on = self.import.selected.contains(&i);
                self.import.wanted.push((path.clone(), class));
                let hover = self.pointer.is_some_and(|p| r.contains(p));
                dl.rect(
                    r,
                    6.0,
                    if hover {
                        pal.surface_hover
                    } else {
                        pal.surface_raised
                    },
                );
                let inner = r.inset_xy(4.0, 4.0);
                match self.atlas.as_mut().and_then(|a| a.best(&path, class)) {
                    Some(pl) => {
                        let (iw, ih) = (pl.width.max(1) as f32, pl.height.max(1) as f32);
                        let s = (inner.w / iw).min(inner.h / ih);
                        let (tw, th) = ((iw * s).round(), (ih * s).round());
                        let img = Rect::new(
                            inner.x + (inner.w - tw) * 0.5,
                            inner.y + (inner.h - th) * 0.5,
                            tw,
                            th,
                        );
                        dl.image_over(img, pl.uv, pl.tex, 3.0, self.backdrop);
                        if !on {
                            dl.rect(img, 3.0, pal.canvas.with_alpha(0.55));
                        }
                    }
                    None => {
                        let c = self
                            .averages
                            .get(path.as_path())
                            .map_or(pal.surface, |a| super::draw::avg_color(*a));
                        dl.rect(inner, 3.0, c);
                    }
                }
                if on {
                    dl.stroke_rect(r, 6.0, pal.accent, 2.0);
                }
                // The check at the leading top corner.
                let cs = (18.0 * ts).round();
                let cr = self.flip(Rect::new(
                    if self.rtl {
                        self.vw - r.right() + 8.0
                    } else {
                        r.x + 8.0
                    },
                    r.y + 8.0,
                    cs,
                    cs,
                ));
                dl.circle(
                    (cr.x + cs * 0.5, cr.y + cs * 0.5),
                    cs * 0.5 + 1.5,
                    pal.surface.with_alpha(0.9),
                );
                if on {
                    dl.circle(
                        (cr.x + cs * 0.5, cr.y + cs * 0.5),
                        cs * 0.5,
                        pal.accent_fill,
                    );
                    let k = (cs * 0.7).round();
                    specular_gfx::draw_icon(
                        dl,
                        Icon::Check,
                        cr.centered(k, k),
                        pal.accent_text,
                        1.8 * ts,
                        false,
                    );
                } else {
                    dl.stroke_rect(cr, cs * 0.5, pal.text_muted, 1.5);
                }
                // What kind of shot it is, at the bottom.
                let badge = if pair {
                    Some(t!("import-badge-pair"))
                } else if raw {
                    Some(t!("import-badge-raw"))
                } else if video {
                    Some(t!("import-badge-video"))
                } else {
                    None
                };
                if let Some(b) = badge {
                    let bw = text.measure(&b, small) + 10.0;
                    let bh = small.size + 6.0;
                    let br = Rect::new(r.x + 6.0, r.bottom() - bh - 6.0, bw, bh);
                    let br = if self.rtl {
                        Rect::new(r.right() - bw - 6.0, br.y, bw, bh)
                    } else {
                        br
                    };
                    dl.rect(br, 4.0, pal.surface.with_alpha(0.85));
                    text_in(text, dl, &b, small, br, pal.text);
                }
                self.import_push_hit(r, Hit::Tile(i));
            }
        }
        self.import.clip = None;
        dl.pop_clip();
    }

    /// The side panel: where and how (ready), the question (confirm), or the progress.
    fn draw_import_panel(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        panel: Rect,
    ) {
        let ts = self.text_scale.clamp(1.0, 3.0);
        let pr = self.flip(panel);
        dl.rect(pr, 0.0, pal.surface);
        let sep = if self.rtl {
            Rect::new(pr.right() - 1.0, pr.y, 1.0, pr.h)
        } else {
            Rect::new(pr.x, pr.y, 1.0, pr.h)
        };
        dl.rect(sep, 0.0, pal.separator);
        let pad = (14.0 * ts).round();
        let inner_w = panel.w - pad * 2.0;
        let x = panel.x + pad;
        let mut y = panel.y + pad;
        let (body, label, small) = (self.ts(BODY), self.ts(LABEL), self.ts(SMALL));
        let row_h = (30.0 * ts).round();
        let (rtl, vw) = (self.rtl, self.vw);
        let wrap =
            move |text: &mut Text, dl: &mut DrawList, s: &str, st: TextStyle, y: &mut f32, c| {
                let b = text.layout(s, st, Some(inner_w), Align::Start);
                let r = Rect::new(x, *y, inner_w, b.height);
                let r = if rtl { r.mirror_x(vw) } else { r };
                let bx = if rtl { r.right() - b.width } else { r.x };
                text.draw(dl, &b, bx, *y, c);
                *y += b.height;
            };

        if let Some(run) = &self.import.run {
            let paused = run.ctl.is_paused();
            let p = run.last.clone();
            let stopping = run.ctl.is_cancelled();
            let msg = if stopping {
                t!("import-stopping")
            } else {
                progress_text(&p, paused)
            };
            wrap(text, dl, &msg, body, &mut y, pal.text);
            y += 10.0;
            let bar = self.flip(Rect::new(x, y, inner_w, 6.0 * ts));
            dl.rect(bar, 3.0 * ts, pal.surface_raised);
            let k = if p.bytes_total > 0 {
                (p.bytes_done as f64 / p.bytes_total as f64).clamp(0.0, 1.0) as f32
            } else {
                0.0
            };
            let fill = if self.rtl {
                Rect::new(bar.right() - bar.w * k, bar.y, bar.w * k, bar.h)
            } else {
                Rect::new(bar.x, bar.y, bar.w * k, bar.h)
            };
            dl.rect(fill, 3.0 * ts, pal.accent);
            y += 6.0 * ts + 8.0;
            if !p.current.is_empty() {
                let c = truncate(text, &p.current, small, inner_w);
                text_at(
                    text,
                    dl,
                    &c,
                    small,
                    self.flip(Rect::new(x, y, inner_w, small.size + 6.0)),
                    pal.text_muted,
                    self.rtl,
                );
                y += small.size + 12.0;
            }
            let third = ((inner_w - 12.0) / 3.0).floor();
            for (k, (hit, l)) in [
                (
                    Hit::Pause,
                    if paused {
                        t!("import-resume")
                    } else {
                        t!("import-pause")
                    },
                ),
                (Hit::Cancel, t!("import-cancel")),
                (Hit::Hide, t!("import-hide")),
            ]
            .into_iter()
            .enumerate()
            {
                let r = self.flip(Rect::new(x + k as f32 * (third + 6.0), y, third, row_h));
                self.import_button(
                    dl,
                    text,
                    pal,
                    r,
                    &l,
                    hit,
                    false,
                    stopping && hit != Hit::Hide,
                );
            }
            return;
        }

        if self.import.confirm {
            let n: usize = self
                .import
                .selected
                .iter()
                .filter_map(|&i| self.import.shots.get(i))
                .map(|s| {
                    s.members.len() + s.members.iter().map(|m| m.sidecars.len()).sum::<usize>()
                })
                .sum();
            let msg = t!(
                "import-confirm-delete",
                count = n as i64,
                name = specular_i18n::isolate(&self.import.source_name())
            );
            wrap(text, dl, &msg, body, &mut y, pal.text);
            y += 14.0;
            let half = ((inner_w - 8.0) / 2.0).floor();
            let r = self.flip(Rect::new(x, y, half, row_h));
            self.import_button(
                dl,
                text,
                pal,
                r,
                &t!("import-confirm-no"),
                Hit::ConfirmNo,
                false,
                false,
            );
            let r = self.flip(Rect::new(x + half + 8.0, y, half, row_h));
            self.import_button(
                dl,
                text,
                pal,
                r,
                &t!("import-confirm-yes"),
                Hit::ConfirmYes,
                true,
                false,
            );
            return;
        }

        // The rest scrolls above Import, which stays at the bottom.
        let footer = row_h + 4.0 + pad * 2.0;
        let area = Rect::new(panel.x, panel.y, panel.w, (panel.h - footer).max(0.0));
        let area_f = self.flip(area);
        self.import.parea = area_f;
        dl.push_clip(area_f);
        self.import.clip = Some(area_f);
        y -= self.import.pscroll;
        if let Some(e) = self.import.error.clone() {
            wrap(text, dl, &e, small, &mut y, pal.danger);
            y += 12.0;
        }

        // Destination.
        let dest = self.import_dest();
        text_at(
            text,
            dl,
            &t!("import-destination"),
            label,
            self.flip(Rect::new(x, y, inner_w, label.size + 6.0)),
            pal.text_muted,
            self.rtl,
        );
        y += label.size + 8.0;
        let choose = t!("import-choose");
        let cw = text.measure(&choose, body) + 20.0;
        let path = dest.display().to_string();
        let pw = (inner_w - cw - 8.0).max(0.0);
        let shown = truncate(text, &path, body, pw);
        text_at(
            text,
            dl,
            &shown,
            body,
            self.flip(Rect::new(x, y, pw, row_h)),
            pal.text,
            self.rtl,
        );
        let r = self.flip(Rect::new(x + inner_w - cw, y, cw, row_h));
        self.import_button(dl, text, pal, r, &choose, Hit::Choose, false, false);
        y += row_h + 12.0;

        // The fields.
        for f in FIELDS {
            let (l, hint) = match f {
                Field::Folder => (t!("import-folder-pattern"), None),
                Field::Event => (t!("import-event"), Some(t!("import-event-hint"))),
                Field::Rename => (t!("import-rename"), Some(t!("import-rename-hint"))),
                Field::Keywords => (t!("import-keywords"), Some(t!("import-keywords-hint"))),
            };
            text_at(
                text,
                dl,
                &l,
                label,
                self.flip(Rect::new(x, y, inner_w, label.size + 6.0)),
                pal.text_muted,
                self.rtl,
            );
            y += label.size + 8.0;
            let fr = self.flip(Rect::new(x, y, inner_w, row_h));
            self.draw_import_field(dl, text, pal, fr, f, hint.as_deref());
            y += row_h + 10.0;
        }

        // The first file's path, or why the patterns do not work.
        let mut can = self.import.ready() && !self.import.selected.is_empty();
        match self.import.patterns() {
            Ok((folder, rename)) => {
                if let Some(&i) = self.import.selected.iter().next()
                    && let Some(shot) = self.import.shots.get(i)
                {
                    let o = Options {
                        dest_root: PathBuf::new(),
                        folder,
                        rename,
                        event: self.import.event.text().to_owned(),
                        keep_pairs: self.import.keep_pairs,
                        delete_after: false,
                    };
                    let p = o.preview(shot, 1).display().to_string();
                    let line = truncate(text, &t!("import-preview", path = p), small, inner_w);
                    text_at(
                        text,
                        dl,
                        &line,
                        small,
                        self.flip(Rect::new(x, y, inner_w, small.size + 6.0)),
                        pal.text_muted,
                        self.rtl,
                    );
                }
                y += small.size + 12.0;
            }
            Err(e) => {
                can = false;
                wrap(text, dl, &e, small, &mut y, pal.danger);
                y += 8.0;
            }
        }

        // What each photo gets: a template, keywords, a style.
        for h in [Hit::Template, Hit::Field(Field::Keywords), Hit::Style] {
            let l = match h {
                Hit::Field(_) => t!("import-keywords"),
                h => self.import.choice(h).0,
            };
            text_at(
                text,
                dl,
                &l,
                label,
                self.flip(Rect::new(x, y, inner_w, label.size + 6.0)),
                pal.text_muted,
                self.rtl,
            );
            y += label.size + 8.0;
            let fr = self.flip(Rect::new(x, y, inner_w, row_h));
            match h {
                Hit::Field(f) => {
                    let hint = t!("import-keywords-hint");
                    self.draw_import_field(dl, text, pal, fr, f, Some(&hint));
                }
                h => {
                    let v = self.import.choice(h).1;
                    // ‹ value ›, as Develop's choices show: a press or the arrows step it.
                    let shown = specular_widgets::choice::arrows(&v, self.rtl);
                    self.import_button(dl, text, pal, fr, &shown, h, false, false);
                }
            }
            y += row_h + 10.0;
        }

        // The options.
        let eject = self.import.current().is_some_and(Source::can_eject);
        let mut toggles = vec![(
            Hit::KeepPairs,
            t!("import-keep-pairs"),
            self.import.keep_pairs,
        )];
        if eject {
            toggles.push((Hit::Eject, t!("import-eject"), self.import.eject));
        }
        toggles.push((Hit::Delete, t!("import-delete"), self.import.delete));
        for (hit, l, on) in toggles {
            let r = self.flip(Rect::new(x, y, inner_w, row_h));
            let bs = (16.0 * ts).round();
            let lead_x = if self.rtl { r.right() - bs } else { r.x };
            let br = Rect::new(lead_x, r.y + (r.h - bs) * 0.5, bs, bs);
            if on {
                dl.rect(br, 4.0, pal.accent_fill);
                let k = (bs * 0.8).round();
                specular_gfx::draw_icon(
                    dl,
                    Icon::Check,
                    br.centered(k, k),
                    pal.accent_text,
                    1.8 * ts,
                    false,
                );
            } else {
                dl.rect_border(br, 4.0, pal.surface_raised, pal.text_muted, 1.5);
            }
            let lr = if self.rtl {
                Rect::new(r.x, r.y, r.w - bs - 8.0, r.h)
            } else {
                Rect::new(r.x + bs + 8.0, r.y, r.w - bs - 8.0, r.h)
            };
            let l = truncate(text, &l, body, lr.w);
            text_at(text, dl, &l, body, lr, pal.text, self.rtl);
            self.import_push_hit(r, hit);
            y += row_h + 2.0;
        }
        y += 8.0;

        // What it takes, and the free space.
        let bytes = self.import.selected_bytes();
        let free = self.import_free();
        if self.import.ready() && !self.import.shots.is_empty() {
            let line = match free {
                Some(fr) => t!("import-free", size = size_text(bytes), free = size_text(fr)),
                None => t!("import-free-unknown", size = size_text(bytes)),
            };
            wrap(text, dl, &line, small, &mut y, pal.text_muted);
            y += 6.0;
            if self.import.already > 0 {
                let l = t!("import-already", count = self.import.already as i64);
                wrap(text, dl, &l, small, &mut y, pal.text_muted);
                y += 6.0;
            }
            if self.import.unreadable > 0 {
                let l = t!("import-unreadable", count = self.import.unreadable as i64);
                wrap(text, dl, &l, small, &mut y, pal.warning);
                y += 6.0;
            }
        }
        let content = y + self.import.pscroll - panel.y + pad;
        self.import.pscroll = self.import.pscroll.clamp(0.0, (content - area.h).max(0.0));
        self.import.clip = None;
        dl.pop_clip();
        if content - self.import.pscroll > area.h + 0.5 {
            dl.rect(
                self.flip(Rect::new(panel.x, area.bottom() - 1.0, panel.w, 1.0)),
                0.0,
                pal.separator,
            );
        }

        // Import, at the bottom.
        let label = if self.import.selected.is_empty() {
            t!("import-nothing-selected")
        } else {
            t!("import-start", count = self.import.selected.len() as i64)
        };
        let by = panel.bottom() - pad - row_h - 4.0;
        let r = self.flip(Rect::new(x, by, inner_w, row_h + 4.0));
        self.import_button(dl, text, pal, r, &label, Hit::Start, true, !can);
    }

    fn draw_import_field(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        fr: Rect,
        f: Field,
        hint: Option<&str>,
    ) {
        let body = self.ts(BODY);
        let focused = self.import.focus == Some(Hit::Field(f));
        dl.rect_border(
            fr,
            6.0,
            pal.surface_raised,
            if focused {
                pal.focus_ring
            } else {
                pal.separator
            },
            if focused { 2.0 } else { 1.0 },
        );
        let inner = fr.inset_xy(10.0, 0.0);
        let e = self.import.field(f);
        let (s, caret, sel) = (e.text().to_owned(), e.caret(), e.selection());
        // An input method's composition sits at the caret of the focused field.
        let pre = if focused { self.preedit.as_str() } else { "" };
        let (shown, composing) = specular_widgets::line_edit::compose(&s, caret, pre);
        let block = text.layout(&shown, body, None, Align::Start);
        let cx = block.caret_x(composing.end);
        let x0 = specular_widgets::line_edit::field_x0(&block, cx, inner, self.rtl);
        let ty = fr.y + (fr.h - block.height) * 0.5;
        dl.push_clip(inner.inset_xy(-2.0, 0.0));
        if s.is_empty()
            && let Some(h) = hint
            && !focused
        {
            let h = truncate(text, h, body, inner.w);
            text_at(text, dl, &h, body, inner, pal.text_faint, self.rtl);
        }
        if focused && composing.is_empty() {
            for (a, b) in block.range_spans(sel.start..sel.end) {
                dl.rect(
                    Rect::new(x0 + a, ty, b - a, block.height),
                    0.0,
                    pal.selection,
                );
            }
        }
        text.draw(dl, &block, x0, ty, pal.text);
        if focused {
            let caret = Rect::new((x0 + cx - 0.75).round(), ty, 1.5, block.height);
            dl.rect(caret, 0.0, pal.text);
            self.import.ime = Some(if composing.is_empty() {
                caret
            } else {
                specular_widgets::line_edit::draw_preedit_underline(
                    dl,
                    &block,
                    x0,
                    composing.clone(),
                    ty + block.height,
                    pal.text_muted,
                );
                specular_widgets::line_edit::ime_area(
                    &block,
                    x0,
                    composing,
                    ty,
                    block.height,
                    inner,
                )
            });
        }
        dl.pop_clip();
        self.import.drawn.insert(f, (block, x0));
        self.import_push_hit(fr, Hit::Field(f));
    }

    /// Where the import goes.
    fn import_dest(&self) -> PathBuf {
        self.import
            .dest
            .clone()
            .unwrap_or_else(specular_platform::integration::user_pictures_dir)
    }

    /// Free bytes at the destination (looked up once per destination).
    fn import_free(&mut self) -> Option<u64> {
        let dest = self.import_dest();
        if let Some((d, f)) = &self.import.free
            && *d == dest
        {
            return *f;
        }
        let f = free_at(&dest);
        self.import.free = Some((dest, f));
        f
    }

    /// The screen-reader nodes of the view; false when it is not open.
    pub(super) fn import_a11y(&self, tree: &mut accesskit::TreeUpdate, scale: f64) -> bool {
        use accesskit::{Action, Live, Node, NodeId, Role, Toggled};
        let iv = &self.import;
        if !iv.open {
            return false;
        }
        let bounds = |r: Rect| accesskit::Rect {
            x0: f64::from(r.x) * scale,
            y0: f64::from(r.y) * scale,
            x1: f64::from(r.x + r.w) * scale,
            y1: f64::from(r.y + r.h) * scale,
        };
        let group_id = NodeId(A11Y_IMPORT);
        let mut group = Node::new(Role::Group);
        group.set_label(t!("import-a11y-group"));
        group.set_bounds(bounds(Rect::new(0.0, 0.0, self.vw, self.vh)));
        let mut children = Vec::new();
        let mut nodes = Vec::new();
        let mut tiles = 0;
        for (r, h) in &iv.hits {
            let id = NodeId(A11Y_IMPORT + h.a11y());
            let mut n;
            match h {
                Hit::Tile(i) => {
                    if tiles >= A11Y_TILES && iv.focus != Some(Hit::Tile(*i)) {
                        continue;
                    }
                    tiles += 1;
                    let Some(s) = iv.shots.get(*i) else { continue };
                    n = Node::new(Role::CheckBox);
                    n.set_label(t!(
                        "import-a11y-tile",
                        name = file_name(s.preview()),
                        date = date_text(s.date)
                    ));
                    n.set_toggled(if iv.selected.contains(i) {
                        Toggled::True
                    } else {
                        Toggled::False
                    });
                    n.add_action(Action::Focus);
                }
                Hit::Field(f) => {
                    n = Node::new(Role::TextInput);
                    n.set_label(match f {
                        Field::Folder => t!("import-folder-pattern"),
                        Field::Event => t!("import-event"),
                        Field::Rename => t!("import-rename"),
                        Field::Keywords => t!("import-keywords"),
                    });
                    n.set_value(iv.field(*f).text().to_owned());
                    n.add_action(Action::Focus);
                }
                Hit::Template | Hit::Style => {
                    // A spin-like choice: Left and Right step it, Enter takes the next.
                    n = Node::new(Role::ComboBox);
                    let (l, v) = iv.choice(*h);
                    n.set_label(l);
                    n.set_value(v);
                    n.set_expanded(false);
                    n.add_action(Action::Focus);
                }
                Hit::KeepPairs | Hit::Eject | Hit::Delete => {
                    n = Node::new(Role::CheckBox);
                    let (l, on) = match h {
                        Hit::KeepPairs => (t!("import-keep-pairs"), iv.keep_pairs),
                        Hit::Eject => (t!("import-eject"), iv.eject),
                        _ => (t!("import-delete"), iv.delete),
                    };
                    n.set_label(l);
                    n.set_toggled(if on { Toggled::True } else { Toggled::False });
                }
                h => {
                    n = Node::new(Role::Button);
                    n.set_label(match h {
                        Hit::Close => t!("import-close"),
                        Hit::NextSource => t!("import-source-next"),
                        Hit::SelectAll => {
                            if iv.selected.len() == iv.shots.len() {
                                t!("import-select-none")
                            } else {
                                t!("import-select-all")
                            }
                        }
                        Hit::Choose => t!("import-choose"),
                        Hit::Start => t!("import-start", count = iv.selected.len() as i64),
                        Hit::ConfirmYes => t!("import-confirm-yes"),
                        Hit::ConfirmNo => t!("import-confirm-no"),
                        Hit::Pause => {
                            if iv.run.as_ref().is_some_and(|r| r.ctl.is_paused()) {
                                t!("import-resume")
                            } else {
                                t!("import-pause")
                            }
                        }
                        Hit::Cancel => t!("import-cancel"),
                        _ => t!("import-hide"),
                    });
                }
            }
            n.set_bounds(bounds(*r));
            n.add_action(Action::Click);
            children.push(id);
            nodes.push((id, n));
        }
        let status_id = NodeId(A11Y_IMPORT + A11Y_STATUS);
        let mut status = Node::new(Role::Status);
        status.set_live(Live::Polite);
        status.set_label(if let Some(r) = &iv.run {
            progress_text(&r.last, r.ctl.is_paused())
        } else if let Some(m) = self.import_message() {
            m
        } else {
            t!(
                "import-summary",
                count = iv.shots.len() as i64,
                size = size_text(iv.bytes.iter().sum())
            )
        });
        children.push(status_id);
        nodes.push((status_id, status));
        // Close is drawn first but read last, after the content (as in a dialog).
        let close = NodeId(A11Y_IMPORT + Hit::Close.a11y());
        if let Some(k) = children.iter().position(|c| *c == close) {
            let c = children.remove(k);
            children.push(c);
        }
        group.set_children(children);
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
            root.push_child(group_id);
        }
        tree.nodes.push((group_id, group));
        tree.nodes.extend(nodes);
        if tree.focus == NodeId(1) {
            tree.focus = iv
                .focus
                .map_or(group_id, |h| NodeId(A11Y_IMPORT + h.a11y()));
        }
        true
    }

    /// While a text field of the view has the keyboard.
    pub(super) fn import_wants_text(&self) -> bool {
        self.import.open && self.import.focused_field().is_some()
    }

    /// Where the focused import field's input method candidate window opens.
    pub(super) fn import_ime_area(&self) -> Option<Rect> {
        self.import_wants_text()
            .then_some(self.import.ime)
            .flatten()
    }
}

// ----- The application side ---------------------------------------------------------------

impl App {
    /// The import's scan or run panicked: its progress ends (the toast says why).
    pub(in crate::app) fn import_lost(&mut self, task: &str) {
        if task == "io-import" {
            let iv = &mut self.gallery.import;
            iv.scan = None;
            iv.run = None;
        }
    }

    /// Starts watching for cards (not in screenshots, benchmarks or tests).
    pub(in crate::app) fn import_init(&mut self) {
        if self.headless() || cfg!(test) {
            return;
        }
        let (tx, rx) = crossbeam_channel::unbounded();
        let wake = self.loader.waker();
        let watcher = specular_import::source::Watcher::start(WATCH_EVERY, move |s| {
            let _ = tx.send(s);
            wake();
        });
        let iv = &mut self.gallery.import;
        iv.watcher = Some(watcher);
        iv.arrivals = Some(rx);
    }

    /// A card, camera or phone with photos is connected (Import is enabled).
    pub(in crate::app) fn import_has_sources(&self) -> bool {
        self.gallery
            .import
            .sources
            .as_ref()
            .is_some_and(|s| !s.is_empty())
            || self.gallery.import.running()
    }

    /// The ledger of imports, opened on first use.
    fn import_ledger(&mut self) -> Arc<Mutex<Ledger>> {
        if let Some(l) = &self.gallery.import.ledger {
            return l.clone();
        }
        let ledger = if self.headless() || cfg!(test) {
            Ledger::in_memory()
        } else {
            let path = specular_settings::Locations::detect()
                .data_dir
                .join(specular_import::ledger::FILE_NAME);
            match Ledger::open(&path) {
                Ok(l) => l,
                Err(e) => {
                    specular_log::warn!("import: ledger {}: {e}", path.display());
                    let now = self.now();
                    self.chrome.warning(
                        t!(
                            "import-ledger-failed",
                            error = crate::error_text::io_reason(&e)
                        ),
                        now,
                    );
                    Ledger::in_memory()
                }
            }
        };
        let l = Arc::new(Mutex::new(ledger));
        self.gallery.import.ledger = Some(l.clone());
        l
    }

    /// Opens the import view on the card at `mount` (or the first one).
    pub(in crate::app) fn import_open(&mut self, mount: Option<PathBuf>) {
        if !self.gallery.active {
            self.gallery_enter();
            self.gallery.transition = None;
        }
        if self.gallery.map.open {
            self.map_close();
        }
        if self.gallery.import.sources.is_none() {
            self.gallery.import.sources = Some(specular_import::source::sources());
        }
        // The templates and styles to choose from; a chosen one since removed is dropped.
        let dir = self.meta_dir();
        self.gallery.meta_edit.load(dir);
        let templates: Vec<String> = self
            .gallery
            .meta_edit
            .templates
            .iter()
            .map(|t| t.name.clone())
            .collect();
        let styles = self.develop_style_names();
        let iv = &mut self.gallery.import;
        iv.template = iv.template.take().filter(|t| templates.contains(t));
        iv.style = iv.style.take().filter(|s| styles.contains(s));
        iv.templates = templates;
        iv.styles = styles;
        let s = &self.settings.import;
        let (folder, rename, dest, keep_pairs, eject) = (
            s.folder_pattern.clone(),
            s.rename_pattern.clone(),
            s.destination.clone(),
            s.keep_pairs,
            s.eject,
        );
        let ga = &mut self.gallery;
        ga.quick = None;
        ga.compare = None;
        let iv = &mut ga.import;
        let reopen = iv.open || iv.running();
        iv.open = true;
        if let Some(r) = iv.run.as_mut()
            && let Some((id, _)) = r.toast.take()
        {
            self.chrome.dismiss_toast(id);
        }
        if !reopen {
            iv.folder = LineEdit::new(&folder);
            iv.rename = LineEdit::new(&rename);
            iv.dest = Some(default_destination(&dest));
            iv.keep_pairs = keep_pairs;
            iv.eject = eject;
            iv.delete = false;
            iv.confirm = false;
            iv.error = None;
            iv.focus = None;
        }
        if !iv.running() {
            let sources = iv.sources.clone().unwrap_or_default();
            let want = mount
                .and_then(|m| sources.iter().find(|s| s.mount == m))
                .or_else(|| {
                    iv.source
                        .as_ref()
                        .and_then(|m| sources.iter().find(|s| &s.mount == m))
                })
                .or_else(|| sources.first())
                .map(|s| s.mount.clone());
            let fresh = want.is_some() && iv.scanned == want;
            let pending = iv.scan.is_some() && iv.source == want;
            if !fresh && !pending {
                iv.source = want;
                self.import_scan();
            }
        }
        self.gallery.wish_key = 0;
        self.chrome.announce_status(t!("import-title"));
        self.redraw = true;
    }

    /// Closes the view (a running import goes on, in a toast).
    pub(in crate::app) fn import_close(&mut self) {
        let now = self.now();
        let iv = &mut self.gallery.import;
        if !iv.open {
            return;
        }
        iv.open = false;
        iv.confirm = false;
        iv.wanted.clear();
        iv.hits.clear();
        if let Some(r) = iv.run.as_mut() {
            let msg = progress_text(&r.last, r.ctl.is_paused());
            r.toast = Some((self.chrome.toast(msg, None, now), now));
        } else if let Some(s) = iv.scan.take() {
            s.cancel.store(true, Ordering::Relaxed);
            iv.scanned = None;
        }
        self.gallery.wish_key = 0;
        self.redraw = true;
    }

    /// Lists the new shots of the shown source on thread `io-import`.
    fn import_scan(&mut self) {
        let ledger = self.import_ledger();
        let low_res = self.settings.import.low_res_videos;
        let iv = &mut self.gallery.import;
        if let Some(s) = iv.scan.take() {
            s.cancel.store(true, Ordering::Relaxed);
        }
        iv.shots.clear();
        iv.bytes.clear();
        iv.selected.clear();
        iv.scanned = None;
        iv.cursor = 0;
        iv.scroll = 0.0;
        iv.error = None;
        let Some(src) = iv.current().cloned() else {
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let (ptx, prx) = crossbeam_channel::unbounded();
        let (dtx, drx) = crossbeam_channel::bounded(1);
        let c = cancel.clone();
        let wake = self.loader.waker();
        self.loader.background("io-import", move |_| {
            let t0 = Instant::now();
            let mut last = Instant::now() - Duration::from_secs(1);
            let w = wake.clone();
            let mut progress = |done: usize, total: usize| {
                if last.elapsed() >= Duration::from_millis(150) || done == total {
                    last = Instant::now();
                    let _ = ptx.send((done, total));
                    w();
                }
            };
            let found = {
                let l = ledger.lock().unwrap_or_else(|e| e.into_inner());
                let found = specular_import::scan::scan(&src.roots, low_res, &l, &c, &mut progress);
                let bytes = found
                    .new
                    .iter()
                    .map(|s| s.new_members(&l).map(|m| m.size).sum())
                    .collect();
                (found, bytes)
            };
            if c.load(Ordering::Relaxed) {
                return;
            }
            let (found, bytes) = found;
            specular_log::info!(
                "import: {} new shot(s), {} imported before, {} unreadable on {} in {:.1} s",
                found.new.len(),
                found.already,
                found.unreadable.len(),
                src.mount.display(),
                t0.elapsed().as_secs_f64()
            );
            let _ = dtx.send(Scanned {
                mount: src.mount.clone(),
                shots: found.new,
                bytes,
                already: found.already,
                unreadable: found.unreadable.len(),
            });
            wake();
        });
        iv.scan = Some(ScanJob {
            cancel,
            progress: prx,
            done: drx,
            last: (0, 0),
        });
    }

    /// Replies of the watcher, the scan, the folder dialog, the copy and the eject.
    pub(in crate::app) fn import_tick(&mut self) {
        let now = self.now();
        // Cards come and go.
        let mut latest = None;
        if let Some(rx) = &self.gallery.import.arrivals {
            while let Ok(s) = rx.try_recv() {
                latest = Some(s);
            }
        }
        if let Some(list) = latest {
            self.import_sources_changed(list);
        }
        let iv = &mut self.gallery.import;
        if let Some(rx) = &iv.picking
            && let Ok(p) = rx.try_recv()
        {
            iv.picking = None;
            iv.dest = Some(p.clone());
            iv.error = None;
            self.chrome.announce_status(p.display().to_string());
            self.redraw = true;
        }
        if let Some(s) = iv.scan.as_mut() {
            while let Ok(p) = s.progress.try_recv() {
                s.last = p;
                self.redraw = true;
            }
            if let Ok(found) = s.done.try_recv() {
                iv.scan = None;
                if iv.source.as_ref() == Some(&found.mount) {
                    iv.selected = (0..found.shots.len()).collect();
                    iv.shots = found.shots;
                    iv.bytes = found.bytes;
                    iv.already = found.already;
                    iv.unreadable = found.unreadable;
                    iv.scanned = Some(found.mount);
                    self.gallery.wish_key = 0;
                    let iv = &self.gallery.import;
                    let msg = if iv.shots.is_empty() {
                        t!("import-none-new", name = iv.source_name())
                    } else {
                        t!(
                            "import-summary",
                            count = iv.shots.len() as i64,
                            size = size_text(iv.bytes.iter().sum())
                        )
                    };
                    self.chrome.announce_status(msg);
                }
                self.redraw = true;
            }
        }
        let iv = &mut self.gallery.import;
        if let Some(rx) = &iv.ejecting
            && let Ok(msg) = rx.try_recv()
        {
            iv.ejecting = None;
            self.chrome.toast(msg, None, now);
            self.redraw = true;
        }
        let Some(r) = iv.run.as_mut() else {
            return;
        };
        let mut changed = false;
        while let Ok(p) = r.progress.try_recv() {
            r.last = p;
            changed = true;
        }
        if let Ok(res) = r.done.try_recv() {
            let r = iv.run.take();
            if let Some(r) = r {
                self.import_finished(res, r);
            }
            return;
        }
        if changed {
            self.redraw = true;
            if let Some((id, at)) = r.toast
                && now - at >= 1.0
            {
                let msg = progress_text(&r.last, r.ctl.is_paused());
                r.toast = Some((self.chrome.update_toast(id, msg, now), now));
            }
        }
    }

    /// The watcher's new list: arrivals are offered (Settings → Import → When a card is
    /// inserted); the first list is what was there already, and is not.
    fn import_sources_changed(&mut self, list: Vec<Source>) {
        let iv = &mut self.gallery.import;
        let first = iv.sources.is_none();
        let before: Vec<PathBuf> = iv
            .sources
            .as_ref()
            .map(|s| s.iter().map(|s| s.mount.clone()).collect())
            .unwrap_or_default();
        let arrived: Vec<Source> = list
            .iter()
            .filter(|s| !before.contains(&s.mount))
            .cloned()
            .collect();
        // Offers of cards gone are withdrawn.
        let gone: Vec<u64> = iv
            .offers
            .iter()
            .filter(|(_, m)| !list.iter().any(|s| &s.mount == *m))
            .map(|(k, _)| *k)
            .collect();
        for k in gone {
            iv.offers.remove(&k);
            self.chrome.dismiss_undo(k);
        }
        for s in &arrived {
            specular_log::info!("import: {} at {}", s.name, s.mount.display());
        }
        let shown_gone = iv
            .source
            .as_ref()
            .is_some_and(|m| !list.iter().any(|s| &s.mount == m));
        iv.sources = Some(list);
        self.redraw = true;
        if shown_gone && !iv.running() {
            if let Some(s) = iv.scan.take() {
                s.cancel.store(true, Ordering::Relaxed);
            }
            iv.source = iv
                .sources
                .as_ref()
                .and_then(|s| s.first())
                .map(|s| s.mount.clone());
            iv.shots.clear();
            iv.bytes.clear();
            iv.selected.clear();
            iv.scanned = None;
            if iv.open && iv.source.is_some() {
                self.import_scan();
            }
        }
        if first || arrived.is_empty() || self.gallery.import.running() {
            return;
        }
        let s = arrived[0].clone();
        match self.settings.import.on_card {
            ImportOnCard::Nothing => {}
            ImportOnCard::Open => self.import_open(Some(s.mount)),
            ImportOnCard::Ask => {
                let iv = &mut self.gallery.import;
                let key = OFFER_KEYS | iv.next_offer;
                iv.next_offer += 1;
                iv.offers.insert(key, s.mount.clone());
                let now = self.now();
                self.chrome.toast_action(
                    t!("import-offer", name = specular_i18n::isolate(&s.name)),
                    t!("import-offer-button"),
                    key,
                    now,
                );
            }
        }
    }

    /// The offer toast's Import button; false when `key` is not one of ours.
    pub(in crate::app) fn import_offer_taken(&mut self, key: u64) -> bool {
        let Some(mount) = self.gallery.import.offers.remove(&key) else {
            return false;
        };
        self.chrome.dismiss_undo(key);
        if self.last_undo == Some(key) {
            self.last_undo = None;
        }
        self.import_open(Some(mount));
        true
    }

    /// Keeps the patterns, destination and options chosen as the settings for next time.
    fn import_save_choices(&mut self) {
        let iv = &self.gallery.import;
        let mut s = self.settings.import.clone();
        if FolderPattern::parse(iv.folder.text()).is_ok() {
            s.folder_pattern = iv.folder.text().to_owned();
        }
        if RenamePattern::parse(iv.rename.text()).is_ok() {
            s.rename_pattern = iv.rename.text().to_owned();
        }
        let pictures = specular_platform::integration::user_pictures_dir();
        s.destination = iv.dest.clone().filter(|d| *d != pictures);
        s.keep_pairs = iv.keep_pairs;
        s.eject = iv.eject;
        if self.settings.import == s {
            return;
        }
        if self.headless() {
            let mut all = (*self.settings).clone();
            all.import = s;
            self.settings = Arc::new(all);
        } else {
            self.store.update(Origin::Program, |x| x.import = s);
            self.settings = self.store.snapshot();
        }
    }

    /// The Import button: asks first when deleting from the card, then copies.
    fn import_start(&mut self, confirmed: bool) {
        let now = self.now();
        if self.gallery.import.running() {
            self.chrome.warning(t!("import-running"), now);
            return;
        }
        let iv = &self.gallery.import;
        if !iv.ready() || iv.selected.is_empty() {
            return;
        }
        let (folder, rename) = match iv.patterns() {
            Ok(p) => p,
            Err(e) => {
                self.gallery.import.error = Some(e);
                self.gallery.import.pscroll = 0.0;
                return;
            }
        };
        if iv.delete && !confirmed {
            let iv = &mut self.gallery.import;
            iv.confirm = true;
            iv.focus = Some(Hit::ConfirmNo);
            self.redraw = true;
            return;
        }
        let dest_root = self.gallery.import_dest();
        let iv = &self.gallery.import;
        let opts = Options {
            dest_root: dest_root.clone(),
            folder,
            rename,
            event: iv.event.text().trim().to_owned(),
            keep_pairs: iv.keep_pairs,
            delete_after: iv.delete,
        };
        let shots: Vec<Shot> = iv
            .selected
            .iter()
            .filter_map(|&i| iv.shots.get(i).cloned())
            .collect();
        let eject = (iv.eject && iv.current().is_some_and(Source::can_eject))
            .then(|| iv.current().cloned())
            .flatten();
        let source_name = iv.source_name();
        let style = iv.style.clone();
        // The template's fields and the keywords typed, for every photo's sidecar.
        let template = iv.template.as_ref().and_then(|n| {
            self.gallery
                .meta_edit
                .templates
                .iter()
                .find(|t| &t.name == n)
        });
        let meta = crate::app::meta_edit::import_edit(template, iv.keywords.text());
        if let Some(e) = &meta
            && let Some(k) = &e.keywords
        {
            let added: Vec<String> = match k {
                specular_metadata::edit::KeywordEdit::Replace(v) => v.clone(),
                specular_metadata::edit::KeywordEdit::Changes(c) => c
                    .iter()
                    .filter_map(|c| match c {
                        specular_metadata::keywords::Change::Add(p) => Some(p.clone()),
                        specular_metadata::keywords::Change::Remove(_) => None,
                    })
                    .collect(),
            };
            self.gallery.keywords.learned(&added, now);
        }
        let vocab = self.gallery.keywords.vocab.clone();
        let ledger = self.import_ledger();
        let plan = {
            let l = ledger.lock().unwrap_or_else(|e| e.into_inner());
            specular_import::run::plan(&shots, &l, &opts)
        };
        let free = free_at(&dest_root);
        if let Err(e) = specular_import::run::check_space(&plan, free) {
            let msg = match e {
                RunError::NoSpace { need, free } => no_space_text(&dest_root, need, free),
                e => e.to_string(),
            };
            self.chrome.announce_status(msg.clone());
            let iv = &mut self.gallery.import;
            iv.error = Some(msg);
            iv.pscroll = 0.0;
            iv.confirm = false;
            self.redraw = true;
            return;
        }
        self.import_save_choices();
        let ctl = Arc::new(Control::default());
        let (ptx, prx) = crossbeam_channel::unbounded();
        let (dtx, drx) = crossbeam_channel::bounded(1);
        let wake = self.loader.waker();
        let c = ctl.clone();
        let root = dest_root.clone();
        let total = plan.files.len();
        specular_log::info!(
            "import: {total} file(s), {} bytes to {}",
            plan.bytes,
            dest_root.display()
        );
        self.loader.background("io-import", move |_| {
            let w = wake.clone();
            let mut res = specular_import::run::run(&plan, &root, &ledger, &c, free, &mut |p| {
                let _ = ptx.send(p.clone());
                w();
            });
            // The metadata, once every copy is verified (a cancelled import's copies too).
            if let (Ok(r), Some(e)) = (res.as_mut(), meta.as_ref()) {
                let failed = crate::app::meta_edit::write_import(&r.files, e, &vocab);
                r.failed.extend(failed);
            }
            let _ = dtx.send(res);
            wake();
        });
        let iv = &mut self.gallery.import;
        iv.run = Some(RunJob {
            ctl,
            progress: prx,
            done: drx,
            last: Progress {
                files_total: total,
                bytes_total: plan_bytes(&iv.bytes, &iv.selected),
                ..Progress::default()
            },
            toast: None,
            eject,
            dest_root,
            source_name,
            style,
        });
        iv.confirm = false;
        iv.error = None;
        iv.focus = Some(Hit::Pause);
        self.chrome.announce_status(t!(
            "import-progress",
            done = 0,
            total = total as i64,
            size = String::new()
        ));
        self.redraw = true;
    }

    /// The copy ended: say how it went, show the photos, rescan the Library, eject.
    fn import_finished(&mut self, res: Result<Report, RunError>, job: RunJob) {
        let now = self.now();
        if let Some((id, _)) = job.toast {
            self.chrome.dismiss_toast(id);
        }
        let report = match res {
            Ok(r) => r,
            Err(e) => {
                let msg = match &e {
                    RunError::NoSpace { need, free } => no_space_text(&job.dest_root, *need, *free),
                    RunError::Destination(p, err) => t!(
                        "import-failed-destination",
                        folder = specular_i18n::isolate(&p.display().to_string()),
                        error = crate::error_text::io_reason(err)
                    ),
                };
                specular_log::warn!("import: {e}");
                if self.gallery.import.open {
                    self.gallery.import.error = Some(msg.clone());
                    self.gallery.import.pscroll = 0.0;
                    self.chrome.announce_status(msg);
                } else {
                    self.chrome.warning(msg, now);
                }
                self.redraw = true;
                return;
            }
        };
        specular_log::info!(
            "import: {} copied, {} already there, {} failed, {} deleted{} in {:.1} s",
            report.copied,
            report.existed,
            report.failed.len(),
            report.deleted,
            if report.cancelled { ", cancelled" } else { "" },
            report.elapsed.as_secs_f64()
        );
        for (p, e) in &report.failed {
            specular_log::warn!("import: {}: {e}", p.display());
        }
        let folder = common_folder(&report.folders).unwrap_or_else(|| job.dest_root.clone());
        if report.copied + report.existed > 0 && self.events_listening("imported") {
            let e = crate::app::events::imported(
                report.first.iter().cloned().collect(),
                &folder,
                report.copied + report.existed,
            );
            self.emit(e);
        }
        let mut msg = if report.cancelled {
            t!("import-done-cancelled", count = report.copied as i64)
        } else if report.copied == 0 && report.existed == 0 {
            t!("import-done-nothing")
        } else {
            t!(
                "import-done",
                count = (report.copied + report.existed) as i64,
                folder = specular_i18n::isolate(&file_name(&folder))
            )
        };
        if report.existed > 0 {
            msg = format!(
                "{msg}. {}",
                t!("import-done-existed", count = report.existed as i64)
            );
        }
        if report.deleted > 0 {
            msg = format!(
                "{msg}. {}",
                t!("import-done-deleted", count = report.deleted as i64)
            );
        }
        if !report.failed.is_empty() {
            msg = format!(
                "{msg}. {}",
                t!("import-done-failed", count = report.failed.len() as i64)
            );
            self.chrome.warning(msg, now);
        } else {
            self.chrome.toast(msg, None, now);
        }
        // The Develop style chosen, as a batch on the photos it can edit.
        if let Some(style) = job.style.as_deref()
            && !report.files.is_empty()
            && self
                .develop_apply_style_named(report.files.clone(), style)
                .is_err()
        {
            self.chrome
                .warning(t!("import-style-missing", name = style.to_owned()), now);
        }
        // The Library: rescan the location the photos went into.
        if let Some(lib) = &self.gallery.library.lib
            && let Some(loc) = lib
                .locations()
                .into_iter()
                .filter(|l| folder.starts_with(&l.path))
                .max_by_key(|l| l.path.components().count())
        {
            lib.rescan(Some(loc.id));
        }
        // Eject (off the event loop: it can take a while to flush).
        if let Some(src) = job
            .eject
            .filter(|_| !report.cancelled && report.failed.is_empty())
        {
            let (tx, rx) = crossbeam_channel::bounded(1);
            let wake = self.loader.waker();
            let name = job.source_name.clone();
            self.loader.background("io-import", move |_| {
                let msg = match specular_platform::volumes::eject(&src.volume) {
                    Ok(()) => t!("import-ejected", name = specular_i18n::isolate(&name)),
                    Err(e) => t!(
                        "import-eject-failed",
                        name = specular_i18n::isolate(&name),
                        error = e.to_string()
                    ),
                };
                let _ = tx.send(msg);
                wake();
            });
            self.gallery.import.ejecting = Some(rx);
        }
        // The card's shots are scanned again next time; the new photos show.
        let iv = &mut self.gallery.import;
        iv.scanned = None;
        iv.shots.clear();
        iv.bytes.clear();
        iv.selected.clear();
        let was_open = iv.open;
        iv.open = false;
        iv.wanted.clear();
        if report.copied + report.existed > 0 || was_open {
            let focus = report.first.clone();
            let dir = focus
                .as_ref()
                .and_then(|f| f.parent())
                .filter(|_| report.folders.len() == 1)
                .map(Path::to_path_buf)
                .unwrap_or(folder);
            if dir.is_dir() {
                self.gallery_enter_at(Some(dir), focus);
            }
        }
        self.gallery.wish_key = 0;
        self.redraw = true;
    }

    /// The folder dialog for the destination (off the event loop).
    fn import_choose(&mut self) {
        let iv = &mut self.gallery.import;
        if iv.picking.is_some() {
            return;
        }
        let start = iv.dest.clone();
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-import", move |l| {
            let mut dlg = rfd::FileDialog::new();
            if let Some(dir) = start.filter(|d| d.is_dir()) {
                dlg = dlg.set_directory(dir);
            }
            if let Some(p) = dlg.pick_folder() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
        iv.picking = Some(rx);
    }

    /// Acts on a control (a click, Enter or a screen reader's click).
    fn import_hit(&mut self, h: Hit) {
        let iv = &mut self.gallery.import;
        match h {
            Hit::Close => self.import_close(),
            Hit::NextSource => {
                let Some(list) = iv.sources.clone() else {
                    return;
                };
                if list.len() > 1 {
                    let i = iv
                        .source
                        .as_ref()
                        .and_then(|m| list.iter().position(|s| &s.mount == m))
                        .map_or(0, |i| (i + 1) % list.len());
                    iv.source = Some(list[i].mount.clone());
                    self.import_scan();
                    self.gallery.wish_key = 0;
                }
            }
            Hit::SelectAll => {
                if iv.selected.len() == iv.shots.len() {
                    iv.selected.clear();
                } else {
                    iv.selected = (0..iv.shots.len()).collect();
                }
            }
            Hit::Choose => self.import_choose(),
            Hit::Field(_) => iv.focus = Some(h),
            Hit::KeepPairs => iv.keep_pairs = !iv.keep_pairs,
            Hit::Eject => iv.eject = !iv.eject,
            Hit::Delete => iv.delete = !iv.delete,
            Hit::Start => self.import_start(false),
            Hit::ConfirmYes => self.import_start(true),
            Hit::ConfirmNo => {
                iv.confirm = false;
                iv.focus = Some(Hit::Start);
            }
            Hit::Pause => {
                if let Some(r) = &iv.run {
                    let on = !r.ctl.is_paused();
                    r.ctl.set_paused(on);
                    self.chrome.announce_status(if on {
                        t!("import-pause")
                    } else {
                        t!("import-resume")
                    });
                }
            }
            Hit::Cancel => {
                if let Some(r) = &iv.run {
                    r.ctl.cancel();
                }
            }
            Hit::Hide => self.import_close(),
            Hit::Template | Hit::Style => {
                if h == Hit::Template {
                    iv.step_template(1);
                } else {
                    iv.step_style(1);
                }
                iv.focus = Some(h);
                let (l, v) = iv.choice(h);
                self.chrome
                    .announce_status(t!("import-choice-said", label = l, value = v));
            }
            Hit::Tile(i) => {
                iv.cursor = i;
                if !iv.selected.remove(&i) && i < iv.shots.len() {
                    iv.selected.insert(i);
                }
                if let Some(e) = iv.error.as_ref()
                    && !e.is_empty()
                {
                    iv.error = None;
                }
            }
        }
        self.redraw = true;
    }

    /// The view's palette entries (with it open).
    pub(super) fn import_menu_items(&self) -> Vec<PaletteItem> {
        let iv = &self.gallery.import;
        if !iv.open {
            return Vec::new();
        }
        let mut v = Vec::new();
        if iv.ready() && !iv.running() && !iv.selected.is_empty() {
            v.push(item(
                code::START,
                t!("import-start", count = iv.selected.len() as i64),
                Some("Enter"),
                Icon::Image,
            ));
        }
        if iv.ready() && !iv.running() {
            v.push(item(
                code::SELECT_ALL,
                t!("import-select-all"),
                Some("Ctrl+A"),
                Icon::Check,
            ));
            v.push(item(
                code::SELECT_NONE,
                t!("import-select-none"),
                None,
                Icon::Minus,
            ));
            v.push(item(code::CHOOSE, t!("import-choose"), None, Icon::Folder));
        }
        v.push(item(
            code::CLOSE,
            t!("import-close"),
            Some("Esc"),
            Icon::Close,
        ));
        v
    }

    pub(super) fn import_code(&mut self, c: u64) {
        let iv = &mut self.gallery.import;
        match c {
            code::START => self.import_start(false),
            code::SELECT_ALL => iv.selected = (0..iv.shots.len()).collect(),
            code::SELECT_NONE => iv.selected.clear(),
            code::CHOOSE => self.import_choose(),
            code::CLOSE => self.import_close(),
            _ => {}
        }
        self.redraw = true;
    }

    /// Typed text into the focused field (keys and IME commits).
    pub(in crate::app) fn import_text(&mut self, s: &str) {
        let iv = &mut self.gallery.import;
        if let Some(f) = iv.focused_field()
            && iv.field_mut(f).insert(s)
        {
            iv.error = None;
            self.redraw = true;
        }
    }

    /// Pasted text into the focused field.
    pub(in crate::app) fn import_paste(&mut self, s: &str) {
        let iv = &mut self.gallery.import;
        if iv.open
            && let Some(f) = iv.focused_field()
        {
            iv.field_mut(f).paste(s);
            self.redraw = true;
        }
    }

    /// A key with the view open; true when used. Without Ctrl, Alt or the logo key every
    /// key is the view's (none reaches the grid behind it).
    pub(super) fn import_key(&mut self, ev: &KeyEvent, combo: Option<&KeyCombo>) -> bool {
        let iv = &mut self.gallery.import;
        // Editing keys in a field.
        if let Some(f) = iv.focused_field() {
            if let Some(c) = combo {
                let rtl = iv.drawn.get(&f).is_some_and(|(b, _)| b.rtl);
                let mut clip = PendingClipboard::default();
                if !matches!(
                    c.key,
                    Key::Named(NamedKey::Tab | NamedKey::Escape | NamedKey::Enter)
                ) && iv.field_mut(f).key(c.key, c.mods, rtl, &mut clip).used()
                {
                    iv.error = None;
                    self.field_clipboard(clip, crate::app::integration::PasteTo::Import);
                    self.redraw = true;
                    return true;
                }
            }
            let plain = combo.is_none_or(|c| !(c.mods.ctrl || c.mods.alt || c.mods.meta));
            if plain
                && let Some(t) = &ev.text
                && !matches!(
                    combo.map(|c| c.key),
                    Some(Key::Named(
                        NamedKey::Tab | NamedKey::Escape | NamedKey::Enter
                    ))
                )
            {
                let s: String = t.chars().filter(|c| !c.is_control()).collect();
                if !s.is_empty() {
                    self.last_key_text = Some((s.clone(), Instant::now()));
                    self.import_text(&s);
                    return true;
                }
            }
        }
        let Some(combo) = combo else { return true };
        let mods = combo.mods;
        let iv = &mut self.gallery.import;
        if mods.ctrl && !mods.alt && !mods.meta && combo.key == Key::Char('a') {
            if iv.ready() && !iv.running() && !iv.confirm {
                iv.selected = (0..iv.shots.len()).collect();
                self.redraw = true;
            }
            return true;
        }
        if mods.ctrl || mods.alt || mods.meta {
            return false;
        }
        let n = iv.shots.len();
        let cols = iv.cols.max(1);
        let on_grid = matches!(iv.focus, Some(Hit::Tile(_)));
        match combo.key {
            Key::Named(NamedKey::Escape) => {
                if iv.confirm {
                    iv.confirm = false;
                    iv.focus = Some(Hit::Start);
                } else if iv.focused_field().is_some() {
                    iv.focus = None;
                } else {
                    self.import_close();
                }
            }
            Key::Named(NamedKey::Tab) => {
                let order = iv.focusables();
                if order.is_empty() {
                    return true;
                }
                let cur = iv.focus.and_then(|f| {
                    order.iter().position(|o| match (o, f) {
                        (Hit::Tile(_), Hit::Tile(_)) => true,
                        (a, b) => *a == b,
                    })
                });
                let k = order.len();
                let next = match (cur, mods.shift) {
                    (None, false) => 0,
                    (None, true) => k - 1,
                    (Some(c), false) => (c + 1) % k,
                    (Some(c), true) => (c + k - 1) % k,
                };
                iv.focus = Some(match order[next] {
                    Hit::Tile(_) => Hit::Tile(iv.cursor),
                    h => h,
                });
                if matches!(iv.focus, Some(Hit::Tile(_))) {
                    iv.scroll_to_cursor = true;
                }
            }
            // A choice row: Left and Right step it (mirrored right to left).
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight))
                if matches!(iv.focus, Some(Hit::Template | Hit::Style)) =>
            {
                let h = iv.focus.unwrap_or(Hit::Template);
                let fwd = (k == NamedKey::ArrowRight) != self.gallery.rtl;
                let iv = &mut self.gallery.import;
                let d = if fwd { 1 } else { -1 };
                if h == Hit::Template {
                    iv.step_template(d);
                } else {
                    iv.step_style(d);
                }
                let (l, v) = iv.choice(h);
                self.chrome
                    .announce_status(t!("import-choice-said", label = l, value = v));
            }
            Key::Named(
                k @ (NamedKey::ArrowLeft
                | NamedKey::ArrowRight
                | NamedKey::ArrowUp
                | NamedKey::ArrowDown
                | NamedKey::Home
                | NamedKey::End),
            ) if n > 0 && (on_grid || iv.focus.is_none()) => {
                let back = |me: &Gallery| me.rtl;
                let rtl = back(&self.gallery);
                let iv = &mut self.gallery.import;
                let c = iv.cursor as i64;
                let (l, r) = if rtl { (1, -1) } else { (-1, 1) };
                let to = match k {
                    NamedKey::ArrowLeft => c + l,
                    NamedKey::ArrowRight => c + r,
                    NamedKey::ArrowUp => c - cols as i64,
                    NamedKey::ArrowDown => c + cols as i64,
                    NamedKey::Home => 0,
                    _ => n as i64 - 1,
                };
                iv.cursor = to.clamp(0, n as i64 - 1) as usize;
                iv.focus = Some(Hit::Tile(iv.cursor));
                iv.scroll_to_cursor = true;
            }
            Key::Named(NamedKey::Space) if on_grid => {
                let c = iv.cursor;
                self.import_hit(Hit::Tile(c));
            }
            Key::Named(NamedKey::Enter | NamedKey::Space) => match iv.focus {
                Some(Hit::Tile(_)) | Some(Hit::Field(_)) | None => {
                    if iv.confirm {
                        self.import_start(true);
                    } else {
                        self.import_start(false);
                    }
                }
                Some(h) => self.import_hit(h),
            },
            _ => {
                // Plain keys are the view's; function keys go on (F11 and the like).
                return !matches!(combo.key, Key::Named(NamedKey::F(_)));
            }
        }
        self.redraw = true;
        true
    }

    /// The pointer moved with the view open: a press in a field drags a selection.
    pub(super) fn import_pointer_move(&mut self, p: (f32, f32)) {
        let iv = &mut self.gallery.import;
        if let Some(f) = iv.focused_field()
            && iv.field(f).dragging()
            && let Some((block, x0)) = iv.drawn.get(&f).cloned()
        {
            iv.field_mut(f).drag_to(block.hit_byte(p.0 - x0));
        }
        self.redraw = true;
    }

    /// A mouse button with the view open.
    pub(super) fn import_button_press(
        &mut self,
        down: bool,
        b: specular_widgets::Button,
        p: (f32, f32),
    ) {
        use specular_widgets::Button;
        let (now, shift) = (self.now(), self.mods.shift_key());
        let iv = &mut self.gallery.import;
        if !down {
            for f in FIELDS {
                iv.field_mut(f).release();
            }
            return;
        }
        if b != Button::Primary {
            return;
        }
        let hit = iv
            .hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(p))
            .map(|x| x.1);
        match hit {
            Some(Hit::Field(f)) => {
                iv.focus = Some(Hit::Field(f));
                if let Some((block, x0)) = iv.drawn.get(&f).cloned() {
                    let at = block.hit_byte(p.0 - x0);
                    iv.field_mut(f).press(at, p.0, now, shift);
                }
                self.redraw = true;
            }
            Some(h) => {
                if matches!(h, Hit::Tile(_)) || iv.focused_field().is_some() {
                    iv.focus = match h {
                        Hit::Tile(i) => Some(Hit::Tile(i)),
                        _ => None,
                    };
                }
                self.import_hit(h);
            }
            None => {
                if iv.focused_field().is_some() {
                    iv.focus = None;
                    self.redraw = true;
                }
            }
        }
    }

    /// The wheel scrolls the grid.
    pub(super) fn import_wheel(&mut self, lines: (f32, f32), pixels: bool, p: (f32, f32)) {
        let iv = &mut self.gallery.import;
        let dy = if pixels { lines.1 } else { lines.1 * 48.0 };
        if iv.parea.contains(p) {
            iv.pscroll = (iv.pscroll - dy).max(0.0);
        } else {
            iv.scroll = (iv.scroll - dy).max(0.0);
        }
        self.redraw = true;
    }

    /// Screen-reader actions on the view's nodes; false when not ours.
    pub(in crate::app) fn import_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        use accesskit::Action;
        let target = req.target_node.0;
        let iv = &self.gallery.import;
        if !self.gallery.active
            || !iv.open
            || !(A11Y_IMPORT..A11Y_IMPORT + A11Y_TILE + 1_000_000).contains(&target)
        {
            return false;
        }
        let off = target - A11Y_IMPORT;
        let hit = iv.hits.iter().map(|h| h.1).find(|h| h.a11y() == off);
        if let Some(h) = hit {
            match req.action {
                Action::Click => self.import_hit(h),
                Action::Focus => self.gallery.import.focus = Some(h),
                _ => {}
            }
        }
        self.redraw = true;
        true
    }

    /// `--show gallery-import` for screenshots: the view of the first source (the simulated
    /// cards of `SPECULAR_IMPORT_MOUNTS`).
    pub(super) fn import_show(&mut self, rest: &str) -> bool {
        if rest != "-import" {
            return false;
        }
        self.import_open(None);
        true
    }
}

/// The bytes of the selected shots.
fn plan_bytes(bytes: &[u64], selected: &BTreeSet<usize>) -> u64 {
    selected.iter().filter_map(|&i| bytes.get(i)).sum()
}

#[cfg(test)]
pub(super) mod testing {
    //! State for goldens and tests without a card or a thread.
    use super::*;

    /// Shows the view with `shots` from a card named `name` at `mount`, all selected.
    pub fn show(g: &mut Gallery, name: &str, mount: &Path, shots: Vec<Shot>, already: usize) {
        use specular_platform::volumes::{Volume, VolumeKind};
        let iv = &mut g.import;
        iv.open = true;
        iv.sources = Some(vec![Source {
            name: name.into(),
            mount: mount.into(),
            roots: vec![mount.join("DCIM")],
            volume: Volume::at(mount, name, VolumeKind::Removable),
        }]);
        iv.source = Some(mount.into());
        iv.scanned = Some(mount.into());
        iv.bytes = shots.iter().map(Shot::bytes).collect();
        iv.selected = (0..shots.len()).collect();
        iv.shots = shots;
        iv.already = already;
        iv.folder = LineEdit::new("{year}/{year}-{month}-{day}");
        iv.dest = Some(PathBuf::from("/home/me/Pictures"));
        iv.free = Some((PathBuf::from("/home/me/Pictures"), Some(212_000_000_000)));
        iv.keep_pairs = true;
    }

    /// What each photo gets: a template, keywords and a style (chosen among these names).
    pub fn set_metadata(g: &mut Gallery, template: &str, keywords: &str, style: &str) {
        let iv = &mut g.import;
        iv.templates = vec![template.to_owned()];
        iv.styles = vec![style.to_owned()];
        iv.step_template(1);
        iv.step_style(1);
        iv.keywords = LineEdit::new(keywords);
        // Scrolled down to the rows.
        iv.pscroll = 250.0;
    }

    pub fn deselect(g: &mut Gallery, i: usize) {
        g.import.selected.remove(&i);
    }

    pub fn set_error(g: &mut Gallery, e: String) {
        g.import.error = Some(e);
    }

    pub fn no_space(dest: &Path, need: u64, free: u64) -> String {
        super::no_space_text(dest, need, free)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choice_rows_step_through_none_and_the_names() {
        let l = vec!["A".to_owned(), "B".to_owned()];
        assert_eq!(step_choice(&l, None, 1).as_deref(), Some("A"));
        assert_eq!(step_choice(&l, Some("A"), 1).as_deref(), Some("B"));
        assert_eq!(step_choice(&l, Some("B"), 1), None);
        assert_eq!(step_choice(&l, None, -1).as_deref(), Some("B"));
        // A name no longer listed counts as None.
        assert_eq!(step_choice(&l, Some("gone"), 1).as_deref(), Some("A"));
        assert_eq!(step_choice(&[], None, 1), None);
    }

    #[test]
    fn common_folder_of_dated_folders() {
        let dirs: BTreeSet<PathBuf> = [
            "/p/2026/2026-09-27",
            "/p/2026/2026-09-28",
            "/p/2026/2026-09-28/raw",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect();
        assert_eq!(common_folder(&dirs), Some(PathBuf::from("/p/2026")));
        let one: BTreeSet<PathBuf> = [PathBuf::from("/p/2026/2026-09-27")].into();
        assert_eq!(
            common_folder(&one),
            Some(PathBuf::from("/p/2026/2026-09-27"))
        );
        assert_eq!(common_folder(&BTreeSet::new()), None);
    }

    #[test]
    fn no_space_says_free_space() {
        let m = no_space_text(Path::new("/home/me/Pictures"), 5_000_000_000, 1_000_000_000);
        assert!(m.contains("free space"), "{m}");
    }

    #[test]
    fn free_space_is_of_the_nearest_existing_folder() {
        let tmp = std::env::temp_dir().join("specular-import-free");
        let _ = std::fs::create_dir_all(&tmp);
        let deep = tmp.join("not/yet/made");
        assert!(
            free_at(&tmp).is_some(),
            "free space of a local folder is known"
        );
        assert!(
            free_at(&deep).is_some(),
            "a folder not made yet uses its parent's"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
