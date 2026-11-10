//! Downloads: what the app is fetching from servers, shown only while something is (and a
//! moment after). With room, the whole of it is shown in place: beside the viewer's
//! toolbar, or in the gallery's bottom bar (what, how much, how fast, time left and a
//! progress bar). Without room (a narrow window, the toolbar hidden in full screen, a
//! slideshow), a small indicator stands in for it. Both open the "Downloads" panel, which
//! lists every download with its figures, a progress bar and Stop.
//!
//! The app localises the content ([`Downloads`]); Stop sends [`Command::Downloads`] with the
//! key the app gave it.

use std::sync::Arc;

use specular_gfx::{Icon, Rect, TextBlock};
use specular_i18n::t;
use specular_theme::{Color, metrics};

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Command, Modal, Response};

/// What the indicator and the panel show, localised by the app.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Downloads {
    /// Newest last.
    pub rows: Vec<DownloadRow>,
    /// The small indicator's text ("45%", "3").
    pub short: String,
    /// What screen readers hear as the indicator's name ("Downloads: Map tiles from
    /// tile.example.org, 12 of 40").
    pub name: String,
    /// All of it together, when known (the small indicator's bar; else it moves).
    pub fraction: Option<f32>,
}

/// One download.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DownloadRow {
    /// What, from where ("Map tiles from tile.example.org").
    pub title: String,
    /// How much and how fast ("45 MB of 120 MB · 2.1 MB/s").
    pub detail: String,
    /// The time left, or how it ended ("About 40 seconds left", "Done", "Failed: no
    /// connection").
    pub status: String,
    /// Done out of total; `None`: not known (the bar moves).
    pub fraction: Option<f32>,
    pub state: DownloadState,
    /// Stop: label and key.
    pub stop: Option<(String, String)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DownloadState {
    #[default]
    Running,
    Done,
    /// Failed or stopped (the bar and the status in the warning colour).
    Failed,
}

/// How the downloads were drawn on the last frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadsForm {
    /// In full, beside the toolbar or in the gallery's bottom bar.
    Inline,
    /// The small indicator.
    Compact,
}

/// The inline card's width, and the least room it takes.
const CARD_W: f32 = 380.0;
const CARD_MIN: f32 = 280.0;
/// The gallery's bottom-bar line needs this much room.
const LINE_MIN: f32 = 300.0;
/// Rows shown in place (the rest are counted: "2 more").
const INLINE_ROWS: usize = 3;
const BAR_H: f32 = 4.0;
const PANEL_BAR_H: f32 = 6.0;

impl Chrome {
    /// Shows (or, with `None`, hides) the downloads. The app announces starts and finishes
    /// itself ([`Chrome::announce_status`]).
    pub fn set_downloads(&mut self, d: Option<Downloads>) {
        match d {
            Some(d) => {
                self.downloads = d;
                self.downloads_on = true;
            }
            None => {
                if matches!(self.kbd, Some(Target::Downloads)) {
                    self.kbd = None;
                }
                self.downloads.rows.clear();
                self.downloads_on = false;
            }
        }
        if self.downloads_open()
            && let Some(f) = self.modal_focus.clone()
            && !self.downloads_stops().contains(&f)
        {
            self.modal_focus = Some(Target::ModalClose);
        }
    }

    /// Whether the downloads are up (in place or as the indicator).
    pub fn downloads_shown(&self) -> bool {
        self.downloads_on
    }

    /// How they were drawn last frame (`None`: not at all).
    pub fn downloads_form(&self) -> Option<DownloadsForm> {
        self.downloads_form
    }

    /// Opens the panel (the palette; the indicator opens it itself).
    pub fn show_downloads(&mut self) {
        self.open_modal(Modal::Downloads);
    }

    pub fn downloads_open(&self) -> bool {
        self.modal == Some(Modal::Downloads)
    }

    pub(crate) fn downloads_click(&mut self, t: &Target) -> Response {
        match t {
            Target::Downloads => {
                self.open_modal(Modal::Downloads);
                Response::used()
            }
            Target::DownloadStop(i) => {
                match self.downloads.rows.get(*i).and_then(|r| r.stop.clone()) {
                    Some((_, k)) => Response::cmd(Command::Downloads(k)),
                    None => Response::used(),
                }
            }
            _ => Response::used(),
        }
    }

    /// The panel's buttons in Tab order: each row's Stop (newest first, as shown), then
    /// Close.
    pub(crate) fn downloads_stops(&self) -> Vec<Target> {
        let mut v: Vec<Target> = (0..self.downloads.rows.len())
            .rev()
            .filter(|i| self.downloads.rows[*i].stop.is_some())
            .map(Target::DownloadStop)
            .collect();
        v.push(Target::ModalClose);
        v
    }

    /// Draws the downloads in place when there is room, else the small indicator. `bar`:
    /// the viewer's toolbar (with what sits on it) and how visible it is. Returns whether
    /// something moves (the chrome draws again soon).
    pub(crate) fn draw_downloads(&mut self, cx: &mut Ctx, bar: Option<(Rect, f32)>) -> bool {
        self.downloads_form = None;
        if !self.downloads_on || self.downloads.rows.is_empty() {
            return false;
        }
        let mut moving = false;
        if self.gallery {
            let line = self.gallery_free.filter(|&(a, b)| b - a >= cx.u(LINE_MIN));
            if let Some((a, b)) = line {
                let y = cx.h - self.gallery_bar;
                moving |= self.draw_line(cx, Rect::new(a, y, b - a, self.gallery_bar));
                self.downloads_form = Some(DownloadsForm::Inline);
            } else {
                let ph = chip_height(cx);
                let pos = match self.gallery_free {
                    // In the bottom bar when it fits, else above it.
                    Some((a, b)) if b - a >= chip_width(cx, &self.downloads.short) => {
                        (a, cx.h - self.gallery_bar + (self.gallery_bar - ph) * 0.5)
                    }
                    _ => (metrics::TOOLBAR_MARGIN, cx.h - self.gallery_bar - 10.0 - ph),
                };
                moving |= self.draw_chip(cx, pos, 1.0);
                self.downloads_form = Some(DownloadsForm::Compact);
            }
            return moving;
        }
        let margin = metrics::TOOLBAR_MARGIN;
        // Room beside the toolbar, on its start side (mirrored for right-to-left).
        let room = bar.map(|(b, _)| {
            let start = if cx.rtl { cx.w - b.right() } else { b.x };
            start - margin - 12.0
        });
        let alpha = bar.map_or(0.0, |(_, a)| a);
        let card_w = cx.u(CARD_W).min(room.unwrap_or(0.0));
        let inline = alpha > 0.0 && card_w >= cx.u(CARD_MIN);
        if inline {
            let b = bar.map(|(b, _)| b).unwrap_or_default();
            moving |= self.draw_card(cx, card_w, b.bottom(), alpha);
            self.downloads_form = Some(DownloadsForm::Inline);
        }
        // The toolbar fading out: the indicator fades in as the card goes.
        let chip_alpha = if inline { 1.0 - alpha } else { 1.0 };
        if chip_alpha > 0.0 {
            let ph = chip_height(cx);
            let pw = chip_width(cx, &self.downloads.short);
            let mut y = cx.h - margin - ph;
            // Over the toolbar's span: above it.
            if let Some((b, a)) = bar
                && a > 0.0
            {
                let x = if cx.rtl { cx.w - margin - pw } else { margin };
                if x < b.right() && x + pw > b.x {
                    y = b.y - 10.0 - ph;
                }
            }
            moving |= self.draw_chip(cx, (margin, y), chip_alpha);
            if !inline {
                self.downloads_form = Some(DownloadsForm::Compact);
            }
        }
        moving
    }

    /// The small indicator at `pos` (logical, left-to-right; mirrored here): a pill with a
    /// download arrow, the short text and a thin bar under them.
    fn draw_chip(&mut self, cx: &mut Ctx, pos: (f32, f32), alpha: f32) -> bool {
        let d = self.downloads.clone();
        let style = cx.style(metrics::FONT_SMALL).weight(550.0);
        let b = cx.layout(&d.short, style, None);
        let ph = chip_height(cx);
        let pw = chip_width(cx, &d.short);
        let icon = cx.u(14.0);
        let r = cx.m(Rect::new(pos.0.round(), pos.1.round(), pw, ph));
        cx.dl.push_opacity(alpha);
        cx.dl
            .glass(r, ph * 0.5, cx.pal.glass, cx.pal.glass_border, 1.0);
        if cx.is_pressed(&Target::Downloads) {
            cx.dl.rect(r, ph * 0.5, cx.pal.surface_pressed);
        } else if cx.is_hover(&Target::Downloads) {
            cx.dl.rect(r, ph * 0.5, cx.pal.surface_hover);
        }
        let ir = cx.m_in(
            Rect::new(r.x + 10.0, r.y + (ph - icon) * 0.5 - 2.0, icon, icon),
            r,
        );
        let failed = d.rows.iter().all(|r| r.state == DownloadState::Failed);
        let tint = if failed { cx.pal.warning } else { cx.pal.text };
        specular_gfx::draw_icon(cx.dl, Icon::Download, ir, tint, 1.6, false);
        let tr = cx.m_in(
            Rect::new(r.x + 10.0 + icon + 6.0, r.y, b.width, ph - 4.0),
            r,
        );
        cx.block_in(&b, tr, HAlign::Start, cx.pal.text);
        let track = Rect::new(r.x + 10.0, r.bottom() - 7.0, pw - 20.0, 2.0);
        let running = d.rows.iter().any(|r| r.state == DownloadState::Running);
        let moving = bar(cx, track, d.fraction, running, failed, !self.reduce_motion);
        cx.ring_if(&Target::Downloads, r, ph * 0.5);
        cx.hit(r, Target::Downloads);
        cx.dl.pop_opacity();
        moving
    }

    /// The downloads in full beside the toolbar: a glass card at the bottom start corner,
    /// its bottom at `bottom`, the newest rows (a count of the rest under them).
    fn draw_card(&mut self, cx: &mut Ctx, w: f32, bottom: f32, alpha: f32) -> bool {
        let d = self.downloads.clone();
        let pad = 10.0;
        let inner = w - 2.0 * pad;
        let title_s = cx.style(metrics::FONT_SMALL).weight(600.0);
        let small = cx.style(metrics::FONT_SMALL);
        let shown: Vec<&DownloadRow> = d.rows.iter().rev().take(INLINE_ROWS).collect();
        let more = d.rows.len().saturating_sub(INLINE_ROWS);
        // A row: what (one line), then the figures at the start and the time left at the
        // end; when both do not fit, the time left keeps up to half the line.
        let mut laid = Vec::with_capacity(shown.len());
        for r in &shown {
            let title = cx.ellipsize(&r.title, title_s, inner, true);
            let sw = cx.text.measure(&r.status, small);
            let dw = cx.text.measure(&r.detail, small);
            let status_w = if dw + 12.0 + sw <= inner {
                sw
            } else {
                sw.min(inner * 0.5)
            };
            let status = cx.ellipsize(&r.status, small, status_w, false);
            let detail = cx.ellipsize(&r.detail, small, (inner - status_w - 12.0).max(0.0), false);
            laid.push((
                cx.layout(&title, title_s, None),
                cx.layout(&detail, small, None),
                cx.layout(&status, small, None),
            ));
        }
        let more_b =
            (more > 0).then(|| cx.layout(&t!("net-more", count = more as i64), small, None));
        let row_h = |t: &TextBlock, l: &TextBlock| t.height + 2.0 + l.height + 5.0 + BAR_H;
        let laid_h = |(t, l, _): &(Arc<TextBlock>, Arc<TextBlock>, Arc<TextBlock>)| row_h(t, l);
        let gap = 10.0;
        let mut h = pad * 2.0
            + laid.iter().map(laid_h).sum::<f32>()
            + gap * laid.len().saturating_sub(1) as f32;
        if let Some(m) = &more_b {
            h += 6.0 + m.height;
        }
        let r = cx.m(Rect::new(
            metrics::TOOLBAR_MARGIN,
            (bottom - h).round(),
            w.round(),
            h.round(),
        ));
        cx.dl.push_opacity(alpha);
        cx.dl.glass(
            r,
            metrics::RADIUS_SM + 4.0,
            cx.pal.glass,
            cx.pal.glass_border,
            1.0,
        );
        if cx.is_pressed(&Target::Downloads) {
            cx.dl
                .rect(r, metrics::RADIUS_SM + 4.0, cx.pal.surface_pressed);
        } else if cx.is_hover(&Target::Downloads) {
            cx.dl
                .rect(r, metrics::RADIUS_SM + 4.0, cx.pal.surface_hover);
        }
        let mut moving = false;
        let mut y = r.y + pad;
        for (i, ((tb, lb, sb), row)) in laid.iter().zip(&shown).enumerate() {
            let tr = cx.m_in(Rect::new(r.x + pad, y, tb.width, tb.height), r);
            cx.block_top(tb, tr, HAlign::Start, cx.pal.text);
            y += tb.height + 2.0;
            let lr = cx.m_in(Rect::new(r.x + pad, y, lb.width, lb.height), r);
            cx.block_top(lb, lr, HAlign::Start, cx.pal.text_muted);
            let sr = cx.m_in(
                Rect::new(r.right() - pad - sb.width, y, sb.width, sb.height),
                r,
            );
            let c = status_colour(cx, row.state);
            cx.block_top(sb, sr, HAlign::Start, c);
            y += lb.height + 5.0;
            let track = Rect::new(r.x + pad, y, inner, BAR_H);
            moving |= bar(
                cx,
                track,
                row.fraction,
                row.state == DownloadState::Running,
                row.state == DownloadState::Failed,
                !self.reduce_motion,
            );
            y += BAR_H;
            if i + 1 < laid.len() {
                y += gap;
            }
        }
        if let Some(m) = &more_b {
            y += 6.0;
            let mr = cx.m_in(Rect::new(r.x + pad, y, m.width, m.height), r);
            cx.block_top(m, mr, HAlign::Start, cx.pal.text_muted);
        }
        cx.ring_if(&Target::Downloads, r, metrics::RADIUS_SM + 4.0);
        cx.hit(r, Target::Downloads);
        cx.dl.pop_opacity();
        moving
    }

    /// The newest download on one line in the gallery's bottom bar (`r`: the bar's free
    /// stretch, left-to-right): arrow, what, figures, a bar at the end.
    fn draw_line(&mut self, cx: &mut Ctx, r: Rect) -> bool {
        let d = self.downloads.clone();
        let Some(row) = d.rows.last() else {
            return false;
        };
        let small = cx.style(metrics::FONT_SMALL);
        let title_s = small.weight(600.0);
        let icon = cx.u(14.0);
        let more = d.rows.len() - 1;
        let figures = join(&row.detail, &row.status);
        let figures = if more > 0 {
            join(&figures, &t!("net-more", count = more as i64))
        } else {
            figures
        };
        // Everything at its natural width when it fits (the bar takes what is left, up to
        // 120 px); else the bar shrinks to 48 px, then the title and figures are cut.
        let fixed = 6.0 + icon + 8.0 + 10.0 + 12.0 + 8.0;
        let natural = cx.text.measure(&row.title, title_s) + cx.text.measure(&figures, small);
        let bar_w = (r.w - fixed - natural).clamp(48.0, 120.0);
        let text_w = (r.w - fixed - bar_w).max(0.0);
        let title_room = cx.text.measure(&row.title, title_s).min(text_w * 0.5);
        let title = cx.ellipsize(&row.title, title_s, title_room, true);
        let tb = cx.layout(&title, title_s, None);
        let fig = cx.ellipsize(&figures, small, (text_w - tb.width).max(0.0), false);
        let fb = cx.layout(&fig, small, None);
        let used = 6.0 + icon + 8.0 + tb.width + 10.0 + fb.width + 12.0 + bar_w + 8.0;
        let hit = cx.m(Rect::new(r.x, r.y + 6.0, used.min(r.w), r.h - 12.0));
        if cx.is_pressed(&Target::Downloads) {
            cx.dl.rect(hit, 6.0, cx.pal.surface_pressed);
        } else if cx.is_hover(&Target::Downloads) {
            cx.dl.rect(hit, 6.0, cx.pal.surface_hover);
        }
        let mut x = r.x + 6.0;
        let ir = cx.m(Rect::new(x, r.y + (r.h - icon) * 0.5, icon, icon));
        specular_gfx::draw_icon(cx.dl, Icon::Download, ir, cx.pal.text_muted, 1.6, false);
        x += icon + 8.0;
        let tr = cx.m(Rect::new(x, r.y, tb.width, r.h));
        cx.block_in(&tb, tr, HAlign::Start, cx.pal.text);
        x += tb.width + 10.0;
        let fr = cx.m(Rect::new(x, r.y, fb.width, r.h));
        let c = status_colour(cx, row.state);
        cx.block_in(&fb, fr, HAlign::Start, c);
        x += fb.width + 12.0;
        let track = cx.m(Rect::new(x, r.y + (r.h - BAR_H) * 0.5, bar_w, BAR_H));
        let moving = bar(
            cx,
            track,
            row.fraction,
            row.state == DownloadState::Running,
            row.state == DownloadState::Failed,
            !self.reduce_motion,
        );
        cx.ring_if(&Target::Downloads, hit, 6.0);
        cx.hit(hit, Target::Downloads);
        moving
    }
}

fn chip_height(cx: &Ctx) -> f32 {
    cx.u(30.0)
}

fn chip_width(cx: &mut Ctx, short: &str) -> f32 {
    let style = cx.style(metrics::FONT_SMALL).weight(550.0);
    let w = cx.text.measure(short, style);
    (10.0 + cx.u(14.0) + 6.0 + w + 12.0).ceil()
}

fn join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_owned(),
        (_, true) => a.to_owned(),
        _ => t!("net-join", first = a, second = b),
    }
}

fn status_colour(cx: &Ctx, s: DownloadState) -> Color {
    match s {
        DownloadState::Running => cx.pal.text,
        DownloadState::Done => cx.pal.text_muted,
        DownloadState::Failed => cx.pal.warning,
    }
}

/// A progress bar in `r` (already mirrored): the fraction filled from the start side, or,
/// with no fraction while it runs, a stretch that moves along it (`animate`; a still,
/// faint full bar under reduced motion). Returns whether it moves.
pub(crate) fn bar(
    cx: &mut Ctx,
    r: Rect,
    fraction: Option<f32>,
    running: bool,
    failed: bool,
    animate: bool,
) -> bool {
    let radius = r.h * 0.5;
    cx.dl.rect(r, radius, cx.pal.separator);
    let fill = if failed {
        cx.pal.warning
    } else {
        cx.pal.accent
    };
    match fraction {
        Some(f) => {
            let fw = (f.clamp(0.0, 1.0) * r.w).max(if f > 0.0 { r.h } else { 0.0 });
            if fw > 0.0 {
                let fr = cx.m_in(Rect::new(r.x, r.y, fw, r.h), r);
                cx.dl.rect(fr, radius, fill);
            }
            false
        }
        None if running && animate => {
            let seg = (r.w * 0.3).max(r.h);
            let t = (cx.now * 0.7).fract() as f32;
            let x = r.x - seg + t * (r.w + seg);
            let a = x.max(r.x);
            let b = (x + seg).min(r.right());
            if b > a {
                let fr = cx.m_in(Rect::new(a, r.y, b - a, r.h), r);
                cx.dl.rect(fr, radius, fill);
            }
            true
        }
        None if running => {
            cx.dl.rect(r, radius, fill.with_alpha(0.35));
            false
        }
        None => false,
    }
}

/// A row of the panel laid out.
struct Row {
    title: Arc<TextBlock>,
    detail: Arc<TextBlock>,
    status: Arc<TextBlock>,
    status_below: bool,
    stop: Option<Arc<TextBlock>>,
}

const ROW_GAP: f32 = 24.0;

fn row_height(r: &Row, bh: f32) -> f32 {
    let mut h = r.title.height + 8.0 + PANEL_BAR_H + 6.0 + r.detail.height;
    if r.status_below {
        h += 2.0 + r.status.height;
    }
    if r.stop.is_some() {
        h += 10.0 + bh;
    }
    h
}

/// The "Downloads" panel: every download, newest first, with its figures, its bar and Stop.
pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx) {
    let w = (cx.w - 48.0).min(cx.u(520.0));
    let inner_w = w - 48.0;
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let head_s = cx.style(metrics::FONT_BODY).weight(600.0);
    let small_s = cx.style(metrics::FONT_SMALL);
    let button_s = cx.style(metrics::FONT_LABEL).weight(550.0);
    let bh = cx.u(30.0);
    let d = ch.downloads.clone();
    let order: Vec<usize> = (0..d.rows.len()).rev().collect();

    let title = cx.layout(&t!("net-title"), title_s, Some(inner_w - 40.0));
    let mut rows = Vec::with_capacity(order.len());
    for &i in &order {
        let r = &d.rows[i];
        let detail = cx.layout(&r.detail, small_s, Some(inner_w));
        let st_style = small_s.weight(550.0);
        let status = cx.layout(&r.status, st_style, Some(inner_w));
        let status_below = detail.width + 16.0 + status.width > inner_w;
        rows.push(Row {
            title: cx.layout(&r.title, head_s, Some(inner_w)),
            detail,
            status,
            status_below,
            stop: r.stop.as_ref().map(|(l, _)| cx.layout(l, button_s, None)),
        });
    }
    let empty = rows
        .is_empty()
        .then(|| cx.layout(&t!("net-empty"), small_s, Some(inner_w)));
    let content: f32 = match &empty {
        Some(e) => e.height,
        None => {
            rows.iter().map(|r| row_height(r, bh)).sum::<f32>()
                + ROW_GAP * rows.len().saturating_sub(1) as f32
        }
    };
    let head = title.height + 18.0;
    let h = (24.0 + head + content + 24.0).min(cx.h - 32.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let close = cx.m_in(Rect::new(r.right() - 46.0, r.y + 14.0, 32.0, 32.0), r);
    cx.close_button(close);
    let tr = cx.m_in(
        Rect::new(r.x + 24.0, r.y + 24.0, inner_w - 40.0, title.height),
        r,
    );
    cx.block_top(&title, tr, HAlign::Start, cx.pal.text);
    let body = Rect::new(
        r.x + 24.0,
        r.y + 24.0 + head,
        inner_w,
        (h - 24.0 - head - 24.0).max(0.0),
    );
    let max = (content - body.h).max(0.0);
    if std::mem::take(&mut ch.reveal_focus)
        && let Some(Target::DownloadStop(want)) = ch.modal_focus
    {
        let mut top = 0.0;
        for (k, row) in rows.iter().enumerate() {
            let rh = row_height(row, bh);
            if order[k] == want {
                let s = &mut ch.downloads_scroll;
                *s = s.min(top + rh - bh).max(top + rh - body.h);
                break;
            }
            top += rh + ROW_GAP;
        }
    }
    ch.downloads_scroll = ch.downloads_scroll.clamp(0.0, max);
    cx.dl.push_clip(body);
    if let Some(e) = &empty {
        let er = cx.m_in(Rect::new(body.x, body.y, e.width, e.height), body);
        cx.block_top(e, er, HAlign::Start, cx.pal.text_muted);
    }
    let visible = |r: Rect| r.y >= body.y - 0.5 && r.bottom() <= body.bottom() + 0.5;
    let mut rings = Vec::new();
    let mut moving = false;
    let mut y = body.y - ch.downloads_scroll;
    for (k, row) in rows.iter().enumerate() {
        let i = order[k];
        let data = &d.rows[i];
        let rh = row_height(row, bh);
        let line = |cx: &mut Ctx, b: &TextBlock, y: f32, color| {
            let lr = cx.m_in(Rect::new(body.x, y, b.width, b.height), body);
            cx.block_top(b, lr, HAlign::Start, color);
        };
        let mut ly = y;
        line(cx, &row.title, ly, cx.pal.text);
        ly += row.title.height + 8.0;
        let track = Rect::new(body.x, ly, inner_w, PANEL_BAR_H);
        moving |= bar(
            cx,
            track,
            data.fraction,
            data.state == DownloadState::Running,
            data.state == DownloadState::Failed,
            !ch.reduce_motion,
        );
        if visible(track) {
            cx.hit(track, Target::DownloadBar(i));
        }
        ly += PANEL_BAR_H + 6.0;
        line(cx, &row.detail, ly, cx.pal.text);
        let status_c = status_colour(cx, data.state);
        if row.status_below {
            ly += row.detail.height + 2.0;
            line(cx, &row.status, ly, status_c);
            ly += row.status.height;
        } else {
            let sr = cx.m_in(
                Rect::new(
                    body.right() - row.status.width,
                    ly,
                    row.status.width,
                    row.status.height,
                ),
                body,
            );
            cx.block_top(&row.status, sr, HAlign::Start, status_c);
            ly += row.detail.height;
        }
        if let Some(b) = &row.stop {
            ly += 10.0;
            let t = Target::DownloadStop(i);
            let bw = b.width + 28.0;
            let br = cx.m_in(Rect::new(body.x, ly, bw, bh), body);
            if cx.is_focused(&t) {
                rings.push(br);
            }
            let fill = if cx.is_hover(&t) {
                cx.pal.surface_hover
            } else {
                cx.pal.surface_raised
            };
            cx.dl
                .rect_border(br, metrics::RADIUS_SM + 2.0, fill, cx.pal.separator, 1.0);
            cx.block_in(b, br, HAlign::Center, cx.pal.text);
            if visible(br) {
                cx.hit(br, t);
            }
        }
        y += rh;
        if k + 1 < rows.len() {
            let sep = Rect::new(body.x, y + ROW_GAP * 0.5, inner_w, 1.0);
            cx.dl.rect(sep, 0.0, cx.pal.separator);
            y += ROW_GAP;
        }
    }
    cx.dl.pop_clip();
    if !rings.is_empty() {
        cx.dl
            .push_clip(Rect::new(body.x - 4.0, body.y, body.w + 8.0, body.h));
        for br in rings {
            cx.button_ring(br, metrics::RADIUS_SM + 2.0);
        }
        cx.dl.pop_clip();
    }
    ch.downloads_moving = moving;
    crate::scrollbar::draw(
        ch,
        cx,
        crate::scrollbar::Area::Downloads,
        body,
        Rect::new(r.x, body.y, r.w, body.h),
        max,
    );
}
