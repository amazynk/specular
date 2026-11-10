//! Export slideshow as video… (DESIGN.md §6.5), in the palette next to Slideshow.
//!
//! A small modal sheet drawn like the print dialog: the resolution (720p, 1080p, 4K or the
//! window's size), the frame rate (24, 30, 60), the quality and the compatibility (H.264
//! High for current players, the default, or Constrained Baseline for old ones: larger
//! files), with the length and an estimate of the file size under them. Export… asks where to save (the system's save
//! dialog); a file that exists is replaced only after a second, explicit Replace. The show
//! is then rendered offscreen and encoded on a worker thread ([`render`]) while the sheet
//! shows the progress; Cancel stops it and leaves nothing behind. When done, a toast names
//! the file and offers Show in folder.
//!
//! The video is the show as it is set up (interval, transition, captions, shuffle), from the
//! current image through the listing once; animations play once. The choices last for the
//! session.

mod render;
#[cfg(test)]
mod shots;
mod timeline;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::Receiver;
use specular_gfx::{Align, Backdrop, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_h264enc::Profile;
use specular_i18n::{I18n, t};
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_render::RenderOptions;
use specular_settings::{HighZoomSmoothing, InitialZoom};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use self::render::{Job, Outcome, Progress};
use self::timeline::{FPS, PROFILES, Quality, Resolution};
use super::App;

const A11Y_BASE: u64 = 0x5200_0000_0000_0000;
/// Toast keys of "Show in folder" (apart from the undo ids, which count up from 1).
const REVEAL_KEYS: u64 = 1 << 62;

/// One line of the choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Row {
    Resolution,
    Fps,
    Quality,
    Compatibility,
}

const ROWS: [Row; 4] = [Row::Resolution, Row::Fps, Row::Quality, Row::Compatibility];

/// What a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    Dec(Row),
    Inc(Row),
    Row(Row),
    Cancel,
    Export,
}

/// The dialog's choices, kept for the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Choices {
    pub resolution: Resolution,
    pub fps: u32,
    pub quality: Quality,
    /// High (current players) or Constrained Baseline (every player).
    pub profile: Profile,
}

impl Default for Choices {
    fn default() -> Choices {
        Choices {
            resolution: Resolution::P1080,
            fps: 30,
            quality: Quality::Standard,
            profile: Profile::High,
        }
    }
}

impl Choices {
    /// Steps `row` by `d` (±1), round and round.
    fn step(&mut self, row: Row, d: i32) {
        fn cycle<T: Copy + PartialEq>(all: &[T], x: T, d: i32) -> T {
            let i = all.iter().position(|v| *v == x).unwrap_or(0) as i32;
            all[(i + d).rem_euclid(all.len() as i32) as usize]
        }
        match row {
            Row::Resolution => self.resolution = cycle(&Resolution::ALL, self.resolution, d),
            Row::Fps => self.fps = cycle(&FPS, self.fps, d),
            Row::Quality => self.quality = cycle(&Quality::ALL, self.quality, d),
            Row::Compatibility => self.profile = cycle(&PROFILES, self.profile, d),
        }
    }
}

/// What the draw function shows for one row.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RowView {
    pub row: Row,
    pub label: String,
    pub value: String,
}

/// Everything the dialog draws (so screenshots need no app).
#[derive(Clone, Debug)]
pub(super) struct DialogView {
    pub rows: Vec<RowView>,
    /// "12 images, in the slideshow's order…"
    pub images: String,
    /// "About 1:00 long, about 38 MB"
    pub summary: String,
    /// The question before replacing, or the progress.
    pub status: Option<String>,
    pub progress: Option<f32>,
    /// Exporting (or the save dialog is up): the choices are fixed.
    pub busy: bool,
    /// The default button reads Replace.
    pub confirm: bool,
    /// Index into [`focusables`](DialogView::focusables).
    pub focus: Option<usize>,
}

impl DialogView {
    /// The keyboard focus order: the rows, then Cancel and Export….
    pub fn focusables(&self) -> Vec<Hit> {
        self.rows
            .iter()
            .map(|r| Hit::Row(r.row))
            .chain([Hit::Cancel, Hit::Export])
            .collect()
    }

    fn export_label(&self) -> String {
        if self.confirm {
            t!("video-export-replace")
        } else {
            t!("video-export-export")
        }
    }
}

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawn {
    pub sheet: Rect,
    pub hits: Vec<(Rect, Hit)>,
    pub rows: Vec<(Rect, Row)>,
}

/// Draws the dialog over a scrim in a window of `size` logical px.
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &DialogView,
    size: (f32, f32),
    rtl: bool,
    ts: f32,
) -> Drawn {
    let (ww, wh) = size;
    dl.rect(Rect::new(0.0, 0.0, ww, wh), 0.0, pal.scrim);
    let pad = 20.0;
    let w = (ww - 32.0).clamp(240.0, 500.0);
    let inner = w - 2.0 * pad;
    let m = |r: Rect| if rtl { r.mirror_x(ww) } else { r };
    let start_x = |x: f32, bw: f32| if rtl { ww - x - bw } else { x };

    let title_st = TextStyle::new(FONT_TITLE * ts).weight(650.0);
    let label_st = TextStyle::new(FONT_LABEL * ts);
    let value_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);

    // Lay everything out first: the sheet is as tall as its content.
    let title = text.layout(
        &t!("video-export-title"),
        title_st,
        Some(inner),
        Align::Start,
    );
    let images = text.layout(&v.images, small_st, Some(inner), Align::Start);
    let summary = text.layout(&v.summary, small_st, Some(inner), Align::Start);
    let status = v
        .status
        .as_ref()
        .map(|s| text.layout(s, label_st, Some(inner), Align::Start));
    let row_h = (30.0 * ts).max(34.0);
    let bh = (24.0 * ts).max(32.0);
    let status_h = status.as_ref().map_or(0.0, |s| {
        s.height + 12.0 + if v.progress.is_some() { 10.0 } else { 0.0 }
    });
    let h = pad
        + title.height
        + 12.0
        + row_h * v.rows.len() as f32
        + 12.0
        + images.height
        + 4.0
        + summary.height
        + status_h
        + 18.0
        + bh
        + pad;
    let h = h.min(wh - 16.0).max(120.0);
    let sheet = Rect::new(((ww - w) * 0.5).round(), ((wh - h) * 0.5).round(), w, h);
    dl.shadow(sheet, RADIUS_LG, pal.shadow, 40.0, 14.0);
    dl.rect_border(sheet, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    let x0 = sheet.x + pad;
    let mut y = sheet.y + pad;
    text.draw(dl, &title, start_x(x0, title.width), y, pal.text);
    y += title.height + 12.0;

    let focus = v.focus.map(|i| v.focusables()[i]);
    let mut drawn = Drawn {
        sheet,
        ..Drawn::default()
    };
    let arrow = (row_h - 6.0).min(26.0 * ts.max(1.0));
    for rv in &v.rows {
        let r = Rect::new(x0, y, inner, row_h);
        if focus == Some(Hit::Row(rv.row)) {
            dl.rect_border(
                m(r.inset(-2.0)),
                RADIUS_SM,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        let lab = text.layout(&rv.label, label_st, None, Align::Start);
        text.draw(
            dl,
            &lab,
            start_x(r.x + 4.0, lab.width),
            r.y + (row_h - lab.height) * 0.5,
            pal.text_muted,
        );
        // ‹ value ›, at the end of the row.
        let sw = (inner * 0.6)
            .min(240.0 * ts.max(1.0))
            .min(inner - lab.width - 8.0);
        let sx = r.right() - sw;
        let dec = Rect::new(sx, r.y + (row_h - arrow) * 0.5, arrow, arrow);
        let inc = Rect::new(r.right() - arrow, dec.y, arrow, arrow);
        for (b, icon, hit) in [
            (dec, Icon::ChevronLeft, Hit::Dec(rv.row)),
            (inc, Icon::ChevronRight, Hit::Inc(rv.row)),
        ] {
            let b = m(b);
            dl.rect(b, RADIUS_SM, pal.surface_raised);
            let c = if v.busy { pal.text_faint } else { pal.text };
            draw_icon(dl, icon, b.inset(arrow * 0.18), c, 1.6, rtl);
            drawn.hits.push((b, hit));
        }
        let val = text.layout(&rv.value, value_st, None, Align::Start);
        let room = (sw - 2.0 * arrow).max(0.0);
        let mid = sx + arrow + (room - val.width.min(room)) * 0.5;
        dl.push_clip(m(Rect::new(sx + arrow, r.y, room, row_h)));
        let ink = if v.busy { pal.text_muted } else { pal.text };
        text.draw(
            dl,
            &val,
            start_x(mid, val.width),
            r.y + (row_h - val.height) * 0.5,
            ink,
        );
        dl.pop_clip();
        drawn.hits.push((m(r), Hit::Row(rv.row)));
        drawn.rows.push((m(r), rv.row));
        y += row_h;
    }
    y += 12.0;
    text.draw(dl, &images, start_x(x0, images.width), y, pal.text_muted);
    y += images.height + 4.0;
    text.draw(dl, &summary, start_x(x0, summary.width), y, pal.text_muted);
    y += summary.height;
    if let Some(s) = &status {
        y += 12.0;
        text.draw(dl, s, start_x(x0, s.width), y, pal.text);
        y += s.height;
        if let Some(p) = v.progress {
            let bar = m(Rect::new(x0, y + 6.0, inner, 4.0));
            dl.rect(bar, 2.0, pal.surface_raised);
            let fill = Rect::new(bar.x, bar.y, bar.w * p.clamp(0.0, 1.0), bar.h);
            // The bar fills from the start edge.
            let fill = if rtl {
                Rect::new(bar.right() - fill.w, fill.y, fill.w, fill.h)
            } else {
                fill
            };
            dl.rect(fill, 2.0, pal.accent);
        }
    }

    // Buttons at the end: Cancel, then Export… (the default: filled).
    let by = sheet.bottom() - pad - bh;
    let mut x = sheet.right() - pad;
    for (hit, label) in [
        (Hit::Export, v.export_label()),
        (Hit::Cancel, t!("app-cancel")),
    ] {
        let b = text.layout(&label, button_st, None, Align::Start);
        let bw = (b.width + 28.0).max(84.0);
        x -= bw;
        let r = Rect::new(x, by, bw, bh);
        x -= 8.0;
        let enabled = hit == Hit::Cancel || !v.busy;
        let rm = m(r);
        let default = hit == Hit::Export;
        let (fill, ink) = if default {
            (pal.accent_fill, pal.accent_text)
        } else {
            (pal.surface_raised, pal.text)
        };
        let fill = if enabled { fill } else { fill.with_alpha(0.45) };
        let ink = if enabled { ink } else { ink.with_alpha(0.5) };
        if default {
            dl.rect(rm, RADIUS_SM + 2.0, fill);
        } else {
            dl.rect_border(rm, RADIUS_SM + 2.0, fill, pal.separator, 1.0);
        }
        if focus == Some(hit) {
            dl.rect_border(
                rm.inset(-4.0),
                RADIUS_SM + 6.0,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        text.draw(
            dl,
            &b,
            rm.x + (rm.w - b.width) * 0.5,
            rm.y + (bh - b.height) * 0.5,
            ink,
        );
        drawn.hits.push((rm, hit));
    }
    drawn
}

/// The dialog's AccessKit nodes: a modal dialog with a spin button per row, the summary,
/// the status and the buttons.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, v: &DialogView, d: &Drawn, scale: f64) {
    use accesskit::{Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    };
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(t!("video-export-title"));
    dialog.set_bounds(bounds(d.sheet));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    let focusables = v.focusables();
    let focus_hit = v.focus.map(|i| focusables[i]);
    let mut focus_id = None;
    for (i, rv) in v.rows.iter().enumerate() {
        let id = NodeId(A11Y_BASE + 1 + i as u64);
        let mut n = Node::new(Role::SpinButton);
        n.set_label(rv.label.clone());
        n.set_value(rv.value.clone());
        if let Some((r, _)) = d.rows.iter().find(|(_, row)| *row == rv.row) {
            n.set_bounds(bounds(*r));
        }
        if v.busy {
            n.set_disabled();
        } else {
            n.add_action(accesskit::Action::Increment);
            n.add_action(accesskit::Action::Decrement);
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(Hit::Row(rv.row)) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    for (k, s) in [&v.images, &v.summary].into_iter().enumerate() {
        let id = NodeId(A11Y_BASE + 0xf0 + k as u64);
        let mut n = Node::new(Role::Label);
        n.set_value(s.clone());
        ids.push(id);
        nodes.push((id, n));
    }
    for (k, (r, hit)) in buttons(d).into_iter().enumerate() {
        let id = NodeId(A11Y_BASE + 0x101 + k as u64);
        let mut n = Node::new(Role::Button);
        n.set_label(match hit {
            Hit::Cancel => t!("app-cancel"),
            _ => v.export_label(),
        });
        n.set_bounds(bounds(r));
        if hit == Hit::Export && v.busy {
            n.set_disabled();
        } else {
            n.add_action(accesskit::Action::Click);
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(hit) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    if let Some(s) = &v.status {
        let mut n = Node::new(Role::Status);
        n.set_live(accesskit::Live::Polite);
        n.set_label(s.clone());
        if let Some(p) = v.progress {
            n.set_numeric_value(f64::from(p) * 100.0);
            n.set_min_numeric_value(0.0);
            n.set_max_numeric_value(100.0);
        }
        let id = NodeId(A11Y_BASE + 0x1ff);
        ids.push(id);
        nodes.push((id, n));
    }
    dialog.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), dialog));
    tree.nodes.extend(nodes);
    tree.focus = focus_id.unwrap_or(NodeId(A11Y_BASE));
}

/// The node `id` stands for: a row's spin button or a button.
fn a11y_target(v: &DialogView, d: &Drawn, id: u64) -> Option<Hit> {
    let k = id.checked_sub(A11Y_BASE)?;
    if (1..=v.rows.len() as u64).contains(&k) {
        return Some(Hit::Row(v.rows[k as usize - 1].row));
    }
    let k = k.checked_sub(0x101)? as usize;
    buttons(d).get(k).map(|b| b.1)
}

/// Cancel and Export as drawn, in reading order (as Tab visits them).
fn buttons(d: &Drawn) -> Vec<(Rect, Hit)> {
    [Hit::Cancel, Hit::Export]
        .iter()
        .filter_map(|o| d.hits.iter().find(|(_, h)| h == o).copied())
        .collect()
}

fn row_label(row: Row) -> String {
    match row {
        Row::Resolution => t!("video-export-resolution"),
        Row::Fps => t!("video-export-fps"),
        Row::Quality => t!("video-export-quality"),
        Row::Compatibility => t!("video-export-compatibility"),
    }
}

/// The rows as the dialog shows them; `window` is the window's size for that choice.
pub(super) fn row_views(c: &Choices, window: (u32, u32)) -> Vec<RowView> {
    ROWS.iter()
        .map(|&row| {
            let value = match row {
                Row::Resolution => match c.resolution {
                    Resolution::P720 => "720p".to_owned(),
                    Resolution::P1080 => "1080p".to_owned(),
                    Resolution::P2160 => "4K".to_owned(),
                    Resolution::Window => {
                        let (w, h) = Resolution::Window.size(window);
                        t!(
                            "video-export-resolution-window",
                            width = w.to_string(),
                            height = h.to_string()
                        )
                    }
                },
                Row::Fps => t!("video-export-fps-value", fps = i64::from(c.fps)),
                Row::Quality => match c.quality {
                    Quality::High => t!("video-export-quality-high"),
                    Quality::Standard => t!("video-export-quality-standard"),
                    Quality::Compact => t!("video-export-quality-compact"),
                },
                Row::Compatibility => match c.profile {
                    Profile::Baseline => t!("video-export-compatibility-all"),
                    Profile::Main | Profile::High => t!("video-export-compatibility-current"),
                },
            };
            RowView {
                row,
                label: row_label(row),
                value,
            }
        })
        .collect()
}

/// `m:ss`, or `h:mm:ss` from an hour.
pub(super) fn clock_text(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

fn size_text(bytes: u64) -> String {
    I18n::global()
        .localizer()
        .bytes(bytes, specular_i18n::ByteUnits::Decimal)
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Where the dialog is.
enum Phase {
    Idle,
    /// The save dialog is up.
    Choosing(Receiver<PathBuf>),
    /// The chosen file exists: Replace?
    Confirm(PathBuf),
    Running {
        progress: Arc<Progress>,
        rx: Receiver<Outcome>,
        cancelling: bool,
        /// The per mille last drawn.
        shown: u32,
    },
}

/// The open dialog.
pub(super) struct Dialog {
    /// The listing and the image the show starts at.
    entries: Vec<PathBuf>,
    start: usize,
    /// The window's size (physical px) when it opened.
    window: (u32, u32),
    phase: Phase,
    focus: Option<usize>,
    drawn: Drawn,
}

/// The app's export state.
#[derive(Default)]
pub(super) struct ExportState {
    pub dialog: Option<Dialog>,
    choices: Choices,
    /// "Show in folder" toasts: toast key → the video.
    reveal: HashMap<u64, PathBuf>,
    next_key: u64,
    /// The export running, started again once if a GPU reset ends it.
    retry: Option<Retry>,
}

/// An export to start again after a GPU reset.
#[derive(Clone, Debug)]
struct Retry {
    dest: PathBuf,
    replace: bool,
    /// [`super::gpu_reset::gpu_resets`] when it started.
    since: u64,
    /// Already started again once.
    retried: bool,
    /// Ended by a reset: starts again once the new device is open.
    waiting: bool,
}

impl std::fmt::Debug for ExportState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportState")
            .field("open", &self.dialog.is_some())
            .field("choices", &self.choices)
            .finish_non_exhaustive()
    }
}

/// The parts of the settings the dialog's estimate needs.
#[derive(Clone, Copy, Debug)]
pub(super) struct Show {
    pub interval: f64,
    pub transition: specular_settings::SlideshowTransition,
}

impl ExportState {
    /// The dialog as the draw function sees it.
    pub(super) fn view(&self, show: Show) -> Option<DialogView> {
        let d = self.dialog.as_ref()?;
        let c = self.choices;
        let slides = d.entries.len();
        let size = c.resolution.size(d.window);
        let (secs, bytes) = timeline::estimate(
            slides,
            show.interval,
            show.transition,
            c.fps,
            size,
            c.quality,
            c.profile,
        );
        let (status, progress, busy, confirm) = match &d.phase {
            Phase::Idle => (None, None, false, false),
            Phase::Choosing(_) => (None, None, true, false),
            Phase::Confirm(p) => (
                Some(t!("video-export-replace-question", file = file_name(p))),
                None,
                false,
                true,
            ),
            Phase::Running {
                progress,
                cancelling,
                ..
            } => {
                let f = progress.fraction();
                let s = if *cancelling {
                    t!("video-export-cancelling")
                } else {
                    let pc = I18n::global().localizer().percent(f64::from(f), 0);
                    t!("video-export-progress", percent = pc)
                };
                (Some(s), Some(f), true, false)
            }
        };
        Some(DialogView {
            rows: row_views(&c, d.window),
            images: t!("video-export-images", count = slides as i64),
            summary: t!(
                "video-export-summary",
                length = clock_text(secs),
                size = size_text(bytes)
            ),
            status,
            progress,
            busy,
            confirm,
            focus: d.focus,
        })
    }

    /// Draws the dialog `v` (when open) over everything.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        v: Option<&DialogView>,
        size: (f32, f32),
        rtl: bool,
        ts: f32,
    ) {
        if let (Some(v), Some(d)) = (v, self.dialog.as_mut()) {
            d.drawn = draw(dl, text, pal, v, size, rtl, ts);
        }
    }

    /// The dialog's nodes in the accessibility tree.
    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &DialogView, scale: f64) {
        if let Some(d) = &self.dialog {
            a11y(tree, v, &d.drawn, scale);
        }
    }
}

impl App {
    fn video_export_show(&self) -> Show {
        let s = &self.settings.slideshow;
        Show {
            interval: s.interval.clamp(1, 60) as f64,
            transition: s.transition,
        }
    }

    pub(super) fn video_export_view(&self) -> Option<DialogView> {
        self.video_export.view(self.video_export_show())
    }

    pub(super) fn video_export_open(&mut self) {
        if self.video_export.dialog.is_some() || self.print.dialog.is_some() {
            return;
        }
        let Some(start) = self.current.filter(|&c| c < self.entries.len()) else {
            return;
        };
        if self.slideshow.running() {
            self.slideshow_stop();
        }
        let window = self.viewport_px().unwrap_or((1920, 1080));
        self.video_export.dialog = Some(Dialog {
            entries: self.entries.clone(),
            start,
            window,
            phase: Phase::Idle,
            focus: Some(ROWS.len() + 1),
            drawn: Drawn::default(),
        });
        self.chrome.announce_status(t!("video-export-title"));
        self.redraw = true;
    }

    fn video_export_close(&mut self) {
        if let Some(d) = self.video_export.dialog.take()
            && let Phase::Running { progress, .. } = d.phase
        {
            progress
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.redraw = true;
    }

    /// The save dialog's answer and the worker's outcome.
    pub(super) fn video_export_tick(&mut self) {
        if self.video_export.dialog.is_none() {
            self.video_export.retry = None;
            return;
        }
        if let Some(r) = self.video_export.retry.as_ref()
            && r.waiting
            && self.gpu_ready()
        {
            let (dest, replace) = (r.dest.clone(), r.replace);
            self.video_export_start(dest, replace);
        }
        let Some(d) = self.video_export.dialog.as_mut() else {
            return;
        };
        match &d.phase {
            Phase::Choosing(rx) => match rx.try_recv() {
                Ok(path) => {
                    if path.exists() {
                        let q = t!("video-export-replace-question", file = file_name(&path));
                        d.phase = Phase::Confirm(path);
                        d.focus = Some(ROWS.len() + 1);
                        self.chrome.announce_status(q);
                    } else {
                        d.phase = Phase::Idle;
                        self.video_export_start(path, false);
                    }
                    self.redraw = true;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    // Dismissed.
                    d.phase = Phase::Idle;
                    self.redraw = true;
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            },
            Phase::Running { rx, .. } => match rx.try_recv() {
                Ok(out) => self.video_export_finished(out),
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.video_export_finished(Outcome::Failed("the export stopped".into()))
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    // The progress bar moves.
                    if let Phase::Running {
                        progress, shown, ..
                    } = &mut d.phase
                    {
                        let now = (progress.fraction() * 1000.0) as u32;
                        if now != *shown {
                            *shown = now;
                            self.redraw = true;
                        }
                    }
                }
            },
            Phase::Idle | Phase::Confirm(_) => {}
        }
    }

    fn video_export_finished(&mut self, out: Outcome) {
        let now = self.now();
        // Ended by a GPU reset: started again once on the new device.
        if matches!(out, Outcome::Failed(_))
            && let Some(since) = self
                .video_export
                .retry
                .as_ref()
                .filter(|r| !r.retried)
                .map(|r| r.since)
            && self.gpu_reset_since(since)
            && let Some(d) = self.video_export.dialog.as_mut()
        {
            d.phase = Phase::Idle;
            if let Some(r) = self.video_export.retry.as_mut() {
                r.waiting = true;
            }
            self.chrome.toast(t!("toast-gpu-reset-retry"), None, now);
            self.redraw = true;
            return;
        }
        self.video_export.retry = None;
        self.video_export.dialog = None;
        match out {
            Outcome::Done(done) => {
                let mut msg = t!(
                    "video-export-saved",
                    file = file_name(&done.path),
                    size = size_text(done.bytes)
                );
                if done.skipped > 0 {
                    msg = format!(
                        "{msg}. {}",
                        t!("video-export-skipped", count = done.skipped as i64)
                    );
                }
                self.video_export.next_key += 1;
                let key = REVEAL_KEYS | self.video_export.next_key;
                self.video_export.reveal.insert(key, done.path);
                self.chrome
                    .toast_action(msg, t!("video-export-show-in-folder"), key, now);
            }
            Outcome::Cancelled => {
                self.chrome.toast(t!("video-export-cancelled"), None, now);
            }
            Outcome::Failed(e) => {
                self.chrome
                    .warning(t!("video-export-failed", reason = e), now);
            }
        }
        self.redraw = true;
    }

    /// Show in folder, from the toast; false when `key` is not one of ours.
    pub(super) fn video_export_reveal(&mut self, key: u64) -> bool {
        let Some(path) = self.video_export.reveal.remove(&key) else {
            return false;
        };
        self.chrome.dismiss_undo(key);
        self.loader
            .background("int-reveal", move |_| super::gallery::reveal(&path));
        true
    }

    /// Asks where to save (the system's dialog, on a thread).
    fn video_export_choose(&mut self) {
        let dir = self.folder.clone();
        let Some(d) = self.video_export.dialog.as_mut() else {
            return;
        };
        let stem = dir
            .as_deref()
            .map(file_name)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Slideshow".to_owned());
        let title = t!("video-export-title");
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-vx-save", move |l| {
            let mut dlg = rfd::FileDialog::new()
                .set_title(title)
                .add_filter("MP4", &["mp4"])
                .set_file_name(format!("{stem}.mp4"));
            if let Some(dir) = dir {
                dlg = dlg.set_directory(dir);
            }
            if let Some(mut p) = dlg.save_file() {
                if p.extension().is_none() {
                    p.set_extension("mp4");
                }
                let _ = tx.send(p);
            }
            drop(tx);
            l.waker()();
        });
        d.phase = Phase::Choosing(rx);
    }

    /// The render options of the show: the full-screen canvas, the transparency backdrop at
    /// the video's scale, high-quality resampling.
    fn video_export_options(&self, scale: f32) -> RenderOptions {
        let v = &self.settings.viewing;
        let canvas = super::settings_color(v.fullscreen_background);
        let lin = canvas.to_linear_premul();
        let mut opts = RenderOptions {
            hq: true,
            canvas: lin,
            ..RenderOptions::default()
        };
        let pal = self.palette();
        (opts.checker_a, opts.checker_b, opts.checker_cell) =
            match self.transparency(&pal, Some(canvas)) {
                Backdrop::Checker { a, b, cell } => (
                    a.to_linear_premul(),
                    b.to_linear_premul(),
                    (cell * scale).round().max(1.0),
                ),
                Backdrop::Solid(c) => (c.to_linear_premul(), c.to_linear_premul(), 1.0e6),
                Backdrop::None => (lin, lin, 1.0e6),
            };
        opts.nearest_from = match v.smoothing_high_zoom {
            HighZoomSmoothing::Auto => 4.0,
            HighZoomSmoothing::Smooth => f32::INFINITY,
            HighZoomSmoothing::Pixelated => 1.0001,
        };
        opts
    }

    /// Starts the worker on `dest`.
    fn video_export_start(&mut self, dest: PathBuf, replace: bool) {
        let Some(gpu) = self.gfx.as_ref().map(|g| g.gpu.clone()) else {
            return;
        };
        let Some(d) = self.video_export.dialog.as_ref() else {
            return;
        };
        let c = self.video_export.choices;
        let s = &self.settings.slideshow;
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x5EED, |d| d.as_nanos() as u64);
        let slides = timeline::order(d.entries.len(), d.start, s.shuffle, seed)
            .into_iter()
            .map(|i| (i, d.entries[i].clone()))
            .collect();
        let size = c.resolution.size(d.window);
        // Captions and the backdrop's cells scale with the video as the live show's do with
        // a 1080-line screen.
        let ui_scale = size.1 as f32 / 1080.0;
        let job = Job {
            slides,
            seed,
            size,
            fps: c.fps,
            qp: c.quality.qp(),
            profile: c.profile,
            interval: self.video_export_show().interval,
            transition: s.transition,
            caption: s.caption,
            delays: crate::anim::delay_policy(self.settings.viewing.animation_short_delays),
            upscale_small: self.settings.viewing.initial_zoom == InitialZoom::Fit,
            opts: self.video_export_options(ui_scale),
            ui_scale,
            text_scale: self.chrome.text_scale,
            rtl: self.rtl(),
            lang: I18n::global().language(),
            dest,
            replace,
        };
        let progress = Arc::new(Progress::default());
        let retried = self.video_export.retry.take().is_some_and(|r| r.waiting);
        self.video_export.retry = Some(Retry {
            dest: job.dest.clone(),
            replace,
            since: super::gpu_reset::gpu_resets(),
            retried,
            waiting: false,
        });
        match render::spawn(
            gpu,
            self.loader.registry(),
            job,
            progress.clone(),
            self.loader.waker(),
        ) {
            Ok(rx) => {
                if let Some(d) = self.video_export.dialog.as_mut() {
                    d.phase = Phase::Running {
                        progress,
                        rx,
                        cancelling: false,
                        shown: 0,
                    };
                    // Focus on Cancel.
                    d.focus = Some(ROWS.len());
                }
            }
            Err(e) => {
                let now = self.now();
                self.chrome.warning(
                    t!(
                        "video-export-failed",
                        reason = crate::error_text::reason(&e)
                    ),
                    now,
                );
            }
        }
        self.redraw = true;
    }

    fn video_export_step(&mut self, row: Row, delta: i32) {
        let Some(v) = self.video_export_view() else {
            return;
        };
        if v.busy {
            return;
        }
        self.video_export.choices.step(row, delta);
        if let Some(d) = self.video_export.dialog.as_mut() {
            // A changed choice asks again before replacing.
            if matches!(d.phase, Phase::Confirm(_)) {
                d.phase = Phase::Idle;
            }
            d.focus = v.focusables().iter().position(|h| *h == Hit::Row(row));
        }
        if let Some(rv) = self
            .video_export_view()
            .and_then(|v| v.rows.into_iter().find(|r| r.row == row))
        {
            self.chrome.announce_status(rv.value);
        }
        self.redraw = true;
    }

    fn video_export_hit(&mut self, h: Hit) {
        match h {
            Hit::Dec(r) => self.video_export_step(r, -1),
            Hit::Inc(r) => self.video_export_step(r, 1),
            Hit::Row(r) => {
                if let Some(d) = self.video_export.dialog.as_mut() {
                    d.focus = ROWS.iter().position(|x| *x == r);
                }
            }
            Hit::Cancel => self.video_export_cancel(),
            Hit::Export => {
                let Some(d) = self.video_export.dialog.as_mut() else {
                    return;
                };
                match std::mem::replace(&mut d.phase, Phase::Idle) {
                    Phase::Idle => self.video_export_choose(),
                    Phase::Confirm(path) => self.video_export_start(path, true),
                    other => d.phase = other,
                }
            }
        }
        self.redraw = true;
    }

    /// Cancel: stops an export, steps back from Replace?, else closes.
    fn video_export_cancel(&mut self) {
        let Some(d) = self.video_export.dialog.as_mut() else {
            return;
        };
        match &mut d.phase {
            Phase::Running {
                progress,
                cancelling,
                ..
            } => {
                progress
                    .cancel
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                *cancelling = true;
                self.chrome.announce_status(t!("video-export-cancelling"));
            }
            Phase::Confirm(_) => d.phase = Phase::Idle,
            Phase::Choosing(_) => {}
            Phase::Idle => self.video_export_close(),
        }
    }

    /// Keys while the dialog is open (it takes them all).
    pub(super) fn video_export_key(&mut self, combo: Option<&KeyCombo>) -> bool {
        let Some(v) = self.video_export_view() else {
            return false;
        };
        let Some(combo) = combo else { return true };
        let focusables = v.focusables();
        let n = focusables.len() as i32;
        let focus = v.focus.map(|i| focusables[i]);
        let rtl = self.rtl();
        let move_focus = |me: &mut App, d: i32| {
            if let Some(dl) = me.video_export.dialog.as_mut() {
                dl.focus = Some(
                    dl.focus
                        .map_or(0, |i| (i as i32 + d).rem_euclid(n) as usize),
                );
            }
        };
        let plain = combo.mods == Mods::NONE;
        match combo.key {
            Key::Named(NamedKey::Escape) => self.video_export_hit(Hit::Cancel),
            Key::Named(NamedKey::Tab) if combo.mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Named(NamedKey::ArrowUp) if plain => move_focus(self, -1),
            Key::Named(NamedKey::ArrowDown) if plain => move_focus(self, 1),
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if plain => {
                let forward = (k == NamedKey::ArrowRight) != rtl;
                match focus {
                    Some(Hit::Row(r)) => self.video_export_step(r, if forward { 1 } else { -1 }),
                    _ => move_focus(self, if forward { 1 } else { -1 }),
                }
            }
            Key::Named(NamedKey::Enter | NamedKey::Space) => match focus {
                Some(h @ (Hit::Cancel | Hit::Export)) => self.video_export_hit(h),
                Some(Hit::Row(r)) if combo.key == Key::Named(NamedKey::Space) => {
                    self.video_export_step(r, 1)
                }
                _ => self.video_export_hit(Hit::Export),
            },
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// A click while the dialog is open (it takes them all; outside the sheet does nothing).
    pub(super) fn video_export_click(&mut self, p: (f32, f32), down: bool) -> bool {
        let Some(d) = self.video_export.dialog.as_ref() else {
            return false;
        };
        if down
            && let Some(h) = d
                .drawn
                .hits
                .iter()
                .find(|(r, h)| r.contains(p) && !matches!(h, Hit::Row(_)))
                .or_else(|| d.drawn.hits.iter().find(|(r, _)| r.contains(p)))
                .map(|x| x.1)
        {
            self.video_export_hit(h);
        }
        true
    }

    /// A screen reader's request on one of the dialog's nodes; false when not ours.
    pub(super) fn video_export_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(v) = self.video_export_view() else {
            return false;
        };
        let Some(d) = self.video_export.dialog.as_ref() else {
            return false;
        };
        let Some(hit) = a11y_target(&v, &d.drawn, req.target_node.0) else {
            return req.target_node.0 & !0xfff == A11Y_BASE;
        };
        match (req.action, hit) {
            (accesskit::Action::Increment, Hit::Row(r)) => self.video_export_step(r, 1),
            (accesskit::Action::Decrement, Hit::Row(r)) => self.video_export_step(r, -1),
            (accesskit::Action::Click, h) => self.video_export_hit(h),
            (accesskit::Action::Focus, h) => {
                if let Some(i) = v.focusables().iter().position(|x| *x == h)
                    && let Some(d) = self.video_export.dialog.as_mut()
                {
                    d.focus = Some(i);
                }
            }
            _ => {}
        }
        self.redraw = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_cycle_both_ways() {
        let mut c = Choices::default();
        c.step(Row::Fps, 1);
        assert_eq!(c.fps, 60);
        c.step(Row::Fps, 1);
        assert_eq!(c.fps, 24);
        c.step(Row::Fps, -1);
        assert_eq!(c.fps, 60);
        c.step(Row::Resolution, -1);
        assert_eq!(c.resolution, Resolution::P720);
        c.step(Row::Resolution, -1);
        assert_eq!(c.resolution, Resolution::Window);
        c.step(Row::Quality, 1);
        assert_eq!(c.quality, Quality::Compact);
        assert_eq!(c.profile, Profile::High, "High is the default");
        c.step(Row::Compatibility, 1);
        assert_eq!(c.profile, Profile::Baseline);
        c.step(Row::Compatibility, 1);
        assert_eq!(c.profile, Profile::High);
        c.step(Row::Compatibility, -1);
        assert_eq!(c.profile, Profile::Baseline);
    }

    #[test]
    fn clock_reads_minutes_and_hours() {
        assert_eq!(clock_text(0.0), "0:00");
        assert_eq!(clock_text(59.6), "1:00");
        assert_eq!(clock_text(605.0), "10:05");
        assert_eq!(clock_text(3725.0), "1:02:05");
    }

    #[test]
    fn rows_show_the_window_size() {
        let c = Choices {
            resolution: Resolution::Window,
            ..Choices::default()
        };
        let v = row_views(&c, (1001, 701));
        assert_eq!(v.len(), 4);
        assert!(v[0].value.contains("1000"), "{}", v[0].value);
        assert!(v[0].value.contains("700"), "{}", v[0].value);
    }
}
