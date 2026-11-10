//! Background work: the indicator chip shown while something runs in the background
//! (library indexing, AI search, faces, text, model downloads, similar shots), in the
//! gallery's top bar and above the viewer's toolbar, and the "Background work" panel it and
//! the palette open, listing every job with its step, progress, time left and what follows.
//! The app localises the content ([`WorkChip`], [`WorkPanel`]); the panel's buttons send
//! [`Command::Work`] with the key the app gave them.

use std::sync::Arc;

use specular_gfx::{Icon, Rect, TextBlock};
use specular_i18n::t;
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Command, Modal, Response};

/// The indicator: shown only while something runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkChip {
    /// The main job and how far it is ("AI search · 1,240 of 9,800"), or why it waits.
    pub label: String,
    /// What screen readers hear as the button's name ("Background work: AI search, 1,240 of
    /// 9,800 photos").
    pub name: String,
    /// The main job waits (a pause icon instead of the spinner).
    pub paused: bool,
}

/// The panel's content, localised by the app.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkPanel {
    pub rows: Vec<WorkRow>,
}

/// One job of the panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkRow {
    /// The job ("AI search").
    pub title: String,
    /// What it does now, and on what ("Reading photo facts · IMG_0042.jpg").
    pub step: String,
    /// How far ("1,240 of 9,800 photos · 8,560 to go").
    pub count: String,
    /// Done out of total, when the total is known (else the bar shows no fill).
    pub fraction: Option<f32>,
    /// The time left ("About 25 minutes left", "Estimating…") or why it waits.
    pub status: String,
    /// It waits (the bar and the status are drawn muted).
    pub paused: bool,
    /// What follows ("Then: Grouping similar shots").
    pub then: Option<String>,
    /// Pause or Resume: label and key.
    pub pause: Option<(String, String)>,
    /// The job's settings: label and key.
    pub settings: Option<(String, String)>,
}

impl Chrome {
    /// Shows (or, with `None`, hides) the indicator. The app announces starts and finishes
    /// itself ([`Chrome::announce_status`]): the chip's label changes too often for that.
    pub fn set_work_chip(&mut self, chip: Option<WorkChip>) {
        if chip.is_none() && matches!(self.kbd, Some(Target::Work)) {
            self.kbd = None;
        }
        self.work_chip = chip;
    }

    /// Whether the indicator is up.
    pub fn work_chip_shown(&self) -> bool {
        self.work_chip.is_some()
    }

    /// The width the indicator took in the gallery's top bar last frame (0 when not drawn
    /// there): the gallery's breadcrumbs leave it that much room.
    pub fn work_chip_width(&self) -> f32 {
        self.work_chip_w
    }

    /// Replaces the panel's content (it stays open or closed). A row's button the keyboard
    /// was on that went away hands the focus on to Close.
    pub fn set_work(&mut self, p: WorkPanel) {
        self.work = p;
        if self.work_open()
            && let Some(f) = self.modal_focus.clone()
            && !self.work_stops().contains(&f)
        {
            self.modal_focus = Some(Target::ModalClose);
        }
    }

    /// Opens the panel with this content.
    pub fn show_work(&mut self, p: WorkPanel) {
        self.work = p;
        self.open_modal(Modal::Work);
    }

    /// Whether the panel is open.
    pub fn work_open(&self) -> bool {
        self.modal == Some(Modal::Work)
    }

    pub(crate) fn work_click(&mut self, t: &Target) -> Response {
        let key = match t {
            Target::Work => {
                self.open_modal(Modal::Work);
                return Response::used();
            }
            Target::WorkPause(i) => self.work.rows.get(*i).and_then(|r| r.pause.clone()),
            Target::WorkSettings(i) => self.work.rows.get(*i).and_then(|r| r.settings.clone()),
            _ => None,
        };
        match key {
            Some((_, k)) => Response::cmd(Command::Work(k)),
            None => Response::used(),
        }
    }

    /// The buttons of the panel in Tab order: each row's, then Close.
    pub(crate) fn work_stops(&self) -> Vec<Target> {
        let mut v = Vec::new();
        for (i, r) in self.work.rows.iter().enumerate() {
            if r.pause.is_some() {
                v.push(Target::WorkPause(i));
            }
            if r.settings.is_some() {
                v.push(Target::WorkSettings(i));
            }
        }
        v.push(Target::ModalClose);
        v
    }

    /// The indicator: a glass pill with a spinner (still under reduced motion; a pause icon
    /// while the main job waits) and the label. In the gallery it sits in the top bar, its
    /// end at [`Chrome::gallery_work_end`]; in the viewer above the toolbar at the end side
    /// (above the "Presenting on" control when that is up), coming and going with the
    /// toolbar. Returns whether the spinner turns (the chrome draws again soon).
    pub(crate) fn draw_work_chip(&mut self, cx: &mut Ctx, alpha: f32) -> bool {
        self.work_chip_w = 0.0;
        let Some(chip) = self.work_chip.clone() else {
            return false;
        };
        if alpha <= 0.0 {
            return false;
        }
        let style = cx.style(metrics::FONT_SMALL).weight(550.0);
        let max_w = (cx.w * 0.4).min(cx.u(320.0)).max(60.0);
        let label = cx.ellipsize(&chip.label, style, max_w, false);
        let b = cx.layout(&label, style, None);
        let ph = cx.u(28.0).max(b.height + 8.0);
        let icon = cx.u(14.0);
        let pw = (10.0 + icon + 8.0 + b.width + 12.0).ceil();
        let in_bar = self.gallery && self.gallery_work_end.is_some();
        let (x, y) = if let Some(end) = self.gallery_work_end.filter(|_| self.gallery) {
            // Centred on the gallery's 32 px buttons (6 px from the top).
            (end - pw, (6.0 + (32.0 - ph) * 0.5).max(2.0))
        } else if self.gallery {
            let y = cx.h - self.gallery_bar - 10.0 - ph;
            (cx.w - metrics::TOOLBAR_MARGIN - pw, y)
        } else {
            let mut y = cx.h - metrics::TOOLBAR_MARGIN - metrics::TOOLBAR_HEIGHT - 10.0 - ph;
            if self.presenting_on.is_some() {
                y -= ph + 8.0;
            }
            (cx.w - metrics::TOOLBAR_MARGIN - pw, y)
        };
        if in_bar {
            self.work_chip_w = pw;
        }
        let r = cx.m(Rect::new(x.round(), y.round(), pw, ph));
        cx.dl.push_opacity(alpha);
        if in_bar {
            cx.dl
                .rect_border(r, ph * 0.5, cx.pal.surface_raised, cx.pal.separator, 1.0);
        } else {
            cx.dl
                .glass(r, ph * 0.5, cx.pal.glass, cx.pal.glass_border, 1.0);
        }
        if cx.is_pressed(&Target::Work) {
            cx.dl.rect(r, ph * 0.5, cx.pal.surface_pressed);
        } else if cx.is_hover(&Target::Work) {
            cx.dl.rect(r, ph * 0.5, cx.pal.surface_hover);
        }
        let ir = cx.m_in(
            Rect::new(r.x + 10.0, r.y + (ph - icon) * 0.5, icon, icon),
            r,
        );
        let turning = !chip.paused && !self.reduce_motion;
        if chip.paused {
            let s = icon * 0.8;
            let pr = Rect::new(ir.x + (icon - s) * 0.5, ir.y + (icon - s) * 0.5, s, s);
            specular_gfx::draw_icon(cx.dl, Icon::Pause, pr, cx.pal.text_muted, 1.5, false);
        } else {
            let c = (ir.x + icon * 0.5, ir.y + icon * 0.5);
            let rad = icon * 0.5 - 1.0;
            // A faint track, and an arc that turns (a still quarter-past under reduced
            // motion: it reads as "working" without moving).
            cx.dl
                .arc(c, rad, 0.0, std::f32::consts::TAU, 2.0, cx.pal.separator);
            let a = if turning {
                (cx.now * 4.0) as f32 % std::f32::consts::TAU
            } else {
                -std::f32::consts::FRAC_PI_2
            };
            cx.dl.arc(c, rad, a, 2.2, 2.0, cx.pal.accent);
        }
        let tr = cx.m_in(Rect::new(r.x + 10.0 + icon + 8.0, r.y, b.width, ph), r);
        cx.block_in(&b, tr, HAlign::Start, cx.pal.text);
        cx.ring_if(&Target::Work, r, ph * 0.5);
        cx.hit(r, Target::Work);
        cx.dl.pop_opacity();
        turning
    }
}

/// A row laid out.
struct Row {
    title: Arc<TextBlock>,
    step: Arc<TextBlock>,
    count: Arc<TextBlock>,
    /// The status beside the count, or (when both do not fit) under it.
    status: Arc<TextBlock>,
    status_below: bool,
    then: Option<Arc<TextBlock>>,
    pause: Option<Arc<TextBlock>>,
    settings: Option<Arc<TextBlock>>,
}

const BAR_H: f32 = 6.0;
/// Space between rows (a separator in its middle).
const ROW_GAP: f32 = 24.0;

fn row_height(r: &Row, bh: f32) -> f32 {
    let mut h = r.title.height + 2.0 + r.step.height + 8.0 + BAR_H + 6.0 + r.count.height;
    if r.status_below {
        h += 2.0 + r.status.height;
    }
    if let Some(t) = &r.then {
        h += 4.0 + t.height;
    }
    if r.pause.is_some() || r.settings.is_some() {
        h += 10.0 + bh;
    }
    h
}

/// A row's button; `hit`: it is in view (scrolled out, it takes no clicks).
fn button(cx: &mut Ctx, r: Rect, b: &TextBlock, t: Target, hit: bool) {
    let fill = if cx.is_hover(&t) {
        cx.pal.surface_hover
    } else {
        cx.pal.surface_raised
    };
    cx.dl
        .rect_border(r, metrics::RADIUS_SM + 2.0, fill, cx.pal.separator, 1.0);
    cx.block_in(b, r, HAlign::Center, cx.pal.text);
    if hit {
        cx.hit(r, t);
    }
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx) {
    let w = (cx.w - 48.0).min(cx.u(520.0));
    let inner_w = w - 48.0;
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let head_s = cx.style(metrics::FONT_BODY).weight(600.0);
    let small_s = cx.style(metrics::FONT_SMALL);
    let button_s = cx.style(metrics::FONT_LABEL).weight(550.0);
    let bh = cx.u(30.0);
    let panel = ch.work.clone();

    let title = cx.layout(&t!("work-title"), title_s, Some(inner_w - 40.0));
    let mut rows = Vec::with_capacity(panel.rows.len());
    for r in &panel.rows {
        let step = cx.ellipsize(&r.step, small_s, inner_w, true);
        let count = cx.layout(&r.count, small_s, Some(inner_w));
        let st_style = small_s.weight(if r.paused { 400.0 } else { 550.0 });
        let status = cx.layout(&r.status, st_style, Some(inner_w));
        let status_below = count.width + 16.0 + status.width > inner_w;
        rows.push(Row {
            title: cx.layout(&r.title, head_s, Some(inner_w)),
            step: cx.layout(&step, small_s, None),
            count,
            status,
            status_below,
            then: r
                .then
                .as_ref()
                .map(|t| cx.layout(t, small_s, Some(inner_w))),
            pause: r.pause.as_ref().map(|(l, _)| cx.layout(l, button_s, None)),
            settings: r
                .settings
                .as_ref()
                .map(|(l, _)| cx.layout(l, button_s, None)),
        });
    }
    let empty = rows
        .is_empty()
        .then(|| cx.layout(&t!("work-empty"), small_s, Some(inner_w)));
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
    // A button reached with Tab scrolls into view.
    if std::mem::take(&mut ch.reveal_focus)
        && let Some(Target::WorkPause(want) | Target::WorkSettings(want)) = ch.modal_focus
    {
        let mut top = 0.0;
        for (i, row) in rows.iter().enumerate() {
            let rh = row_height(row, bh);
            if i == want {
                let s = &mut ch.work_scroll;
                *s = s.min(top + rh - bh).max(top + rh - body.h);
                break;
            }
            top += rh + ROW_GAP;
        }
    }
    ch.work_scroll = ch.work_scroll.clamp(0.0, max);
    cx.dl.push_clip(body);
    if let Some(e) = &empty {
        let er = cx.m_in(Rect::new(body.x, body.y, e.width, e.height), body);
        cx.block_top(e, er, HAlign::Start, cx.pal.text_muted);
    }
    let visible = |r: Rect| r.y >= body.y - 0.5 && r.bottom() <= body.bottom() + 0.5;
    let mut rings = Vec::new();
    let mut y = body.y - ch.work_scroll;
    for (i, (row, data)) in rows.iter().zip(&panel.rows).enumerate() {
        let rh = row_height(row, bh);
        let line = |cx: &mut Ctx, b: &TextBlock, y: f32, color| {
            let lr = cx.m_in(Rect::new(body.x, y, b.width, b.height), body);
            cx.block_top(b, lr, HAlign::Start, color);
        };
        let mut ly = y;
        line(cx, &row.title, ly, cx.pal.text);
        ly += row.title.height + 2.0;
        line(cx, &row.step, ly, cx.pal.text_muted);
        ly += row.step.height + 8.0;
        // The bar: accent while it runs, muted while it waits; no fill with no total.
        let bar = Rect::new(body.x, ly, inner_w, BAR_H);
        cx.dl.rect(bar, BAR_H * 0.5, cx.pal.separator);
        if let Some(f) = data.fraction {
            let fw = (f.clamp(0.0, 1.0) * inner_w).max(if f > 0.0 { BAR_H } else { 0.0 });
            if fw > 0.0 {
                let fill = if data.paused {
                    cx.pal.text_muted
                } else {
                    cx.pal.accent
                };
                let fr = cx.m_in(Rect::new(body.x, ly, fw, BAR_H), bar);
                cx.dl.rect(fr, BAR_H * 0.5, fill);
            }
        }
        // The progress bar is a node of its own for screen readers (while in view).
        if visible(bar) {
            cx.hit(bar, Target::WorkBar(i));
        }
        ly += BAR_H + 6.0;
        line(cx, &row.count, ly, cx.pal.text);
        let status_c = if data.paused {
            cx.pal.text_muted
        } else {
            cx.pal.text
        };
        if row.status_below {
            ly += row.count.height + 2.0;
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
            ly += row.count.height;
        }
        if let Some(t) = &row.then {
            ly += 4.0;
            line(cx, t, ly, cx.pal.text_muted);
            ly += t.height;
        }
        if row.pause.is_some() || row.settings.is_some() {
            ly += 10.0;
            let mut bx = body.x;
            for (b, t) in [
                (row.pause.as_ref(), Target::WorkPause(i)),
                (row.settings.as_ref(), Target::WorkSettings(i)),
            ] {
                let Some(b) = b else { continue };
                let bw = b.width + 28.0;
                let br = cx.m_in(Rect::new(bx, ly, bw, bh), body);
                if cx.is_focused(&t) {
                    rings.push(br);
                }
                button(cx, br, b, t, visible(br));
                bx += bw + 8.0;
            }
        }
        y += rh;
        if i + 1 < rows.len() {
            let sep = Rect::new(body.x, y + ROW_GAP * 0.5, inner_w, 1.0);
            cx.dl.rect(sep, 0.0, cx.pal.separator);
            y += ROW_GAP;
        }
    }
    cx.dl.pop_clip();
    if !rings.is_empty() {
        // Scrolled with the body but not cut at its sides, where the buttons may end.
        cx.dl
            .push_clip(Rect::new(body.x - 4.0, body.y, body.w + 8.0, body.h));
        for br in rings {
            cx.button_ring(br, metrics::RADIUS_SM + 2.0);
        }
        cx.dl.pop_clip();
    }
    crate::scrollbar::draw(
        ch,
        cx,
        crate::scrollbar::Area::Work,
        body,
        Rect::new(r.x, body.y, r.w, body.h),
        max,
    );
}
