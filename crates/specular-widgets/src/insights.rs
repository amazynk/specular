//! Performance insights (DESIGN.md §12.7): the small "Why is this slow?" indicator in the
//! bottom corner (in the viewer and the gallery), shown when the advisor found a recurring
//! slowdown, and the panel it opens
//! (also from the palette): the last slow open's stages, the bottleneck in plain language,
//! what can be done now, then which hardware would help. The app localises the content
//! ([`Insights`]); buttons send [`Command::Insight`] with the key the app gave them.

use std::sync::Arc;

use specular_gfx::{Icon, Rect, TextBlock};
use specular_i18n::t;
use specular_theme::{Color, metrics};

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Command, Modal, Response};

/// The Performance insights panel's content, localised by the app.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Insights {
    /// What holds opens up, in plain language.
    pub bottleneck: String,
    /// More about it (how many opens, why), or why there is nothing to say yet.
    pub detail: String,
    /// The last slow open: a caption ("2.4 s until sharp") and its stages.
    pub last: Option<(String, Vec<InsightStage>)>,
    /// What can be done now, without new hardware.
    pub now: Vec<InsightItem>,
    /// Hardware that would help, by capability.
    pub hardware: Vec<InsightItem>,
    /// "Don't show again" for this kind of slowdown: label and key.
    pub dismiss: Option<(String, String)>,
    /// The global switch (turn insights off, or on again): label and key.
    pub switch: Option<(String, String)>,
    /// "Show hidden insights again" (undoes the indicator's ×), when some are hidden: label
    /// and key.
    pub restore: Option<(String, String)>,
}

/// One stage of the stage bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InsightStage {
    pub name: String,
    pub ms: f64,
    /// The duration as shown ("1.2 s").
    pub value: String,
    /// The bottleneck (drawn in the accent colour).
    pub main: bool,
}

/// Something that would help.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InsightItem {
    pub text: String,
    /// The estimated gain ("About 3 to 6 times faster").
    pub gain: Option<String>,
    /// A button that does it: label and key.
    pub action: Option<(String, String)>,
}

/// What the indicator shows: its label and what screen readers are told once.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Indicator {
    pub label: String,
    pub announce: String,
}

impl Chrome {
    /// Shows (or, with `None`, hides) the indicator: its label ("Why is this slow?") and the
    /// sentence screen readers hear when it appears (once, politely).
    pub fn set_insight_indicator(&mut self, ind: Option<(String, String)>) {
        let ind = ind.map(|(label, announce)| Indicator { label, announce });
        if let Some(i) = &ind
            && self.insight_indicator.is_none()
        {
            self.announce(i.announce.clone());
        }
        self.insight_indicator = ind;
    }

    /// Whether the indicator is up.
    pub fn insight_indicator_shown(&self) -> bool {
        self.insight_indicator.is_some()
    }

    /// Replaces the panel's content (it stays open or closed).
    pub fn set_insights(&mut self, i: Insights) {
        let was = self.insights_open().then(|| self.insight_stops());
        self.insights = i;
        // The button the keyboard was on went away (Show hidden insights again, once
        // pressed; Try again, once it helped): the keyboard goes to the one now in its
        // place, Close after the last, as it stays in the dialog.
        if let (Some(was), Some(f)) = (was, self.modal_focus.clone()) {
            let now = self.insight_stops();
            if !now.contains(&f)
                && let Some(at) = was.iter().position(|s| *s == f)
            {
                self.modal_focus = Some(now[at.min(now.len() - 1)].clone());
            }
        }
    }

    /// Opens the panel with this content.
    pub fn show_insights(&mut self, i: Insights) {
        self.insights = i;
        self.open_modal(Modal::Insights);
    }

    /// Whether the panel is open.
    pub fn insights_open(&self) -> bool {
        self.modal == Some(Modal::Insights)
    }

    pub(crate) fn insight_click(&mut self, t: &Target) -> Response {
        let key = match t {
            Target::Insight => "open".to_owned(),
            Target::InsightHide => {
                self.insight_indicator = None;
                "hide".to_owned()
            }
            Target::InsightAction(i) => {
                match self.insights.now.get(*i).and_then(|n| n.action.as_ref()) {
                    Some((_, k)) => k.clone(),
                    None => return Response::used(),
                }
            }
            Target::InsightDismiss => match &self.insights.dismiss {
                Some((_, k)) => {
                    let k = k.clone();
                    self.close_modal();
                    k
                }
                None => return Response::used(),
            },
            Target::InsightSwitch => match &self.insights.switch {
                Some((_, k)) => k.clone(),
                None => return Response::used(),
            },
            Target::InsightRestore => match &self.insights.restore {
                Some((_, k)) => k.clone(),
                None => return Response::used(),
            },
            _ => return Response::used(),
        };
        Response::cmd(Command::Insight(key))
    }

    /// The indicator: a small glass pill at the bottom start corner, above the toolbar's
    /// row, that comes and goes with the toolbar; in the gallery, above its bottom bar.
    pub(crate) fn draw_insight_indicator(&self, cx: &mut Ctx, alpha: f32) {
        let Some(ind) = &self.insight_indicator else {
            return;
        };
        if alpha <= 0.0 {
            return;
        }
        let style = cx.style(metrics::FONT_SMALL).weight(550.0);
        let b = cx.layout(&ind.label, style, Some((cx.w * 0.5).max(80.0)));
        let ph = cx.u(28.0).max(b.height + 8.0);
        let close_w = ph;
        let pw = 10.0 + 16.0 + 8.0 + b.width + 4.0 + close_w;
        let y = if self.gallery {
            cx.h - self.gallery_bar - 10.0 - ph
        } else {
            cx.h - metrics::TOOLBAR_MARGIN - metrics::TOOLBAR_HEIGHT - 10.0 - ph
        };
        let r = cx.m(Rect::new(metrics::TOOLBAR_MARGIN, y.round(), pw.ceil(), ph));
        cx.dl.push_opacity(alpha);
        cx.dl
            .glass(r, ph * 0.5, cx.pal.glass, cx.pal.glass_border, 1.0);
        let main = cx.m_in(Rect::new(r.x, r.y, r.w - close_w, r.h), r);
        if cx.is_hover(&Target::Insight) {
            cx.dl.rect(main, ph * 0.5, cx.pal.surface_hover);
        }
        let icon = cx.m_in(
            Rect::new(r.x + 10.0, r.y + (ph - 16.0) * 0.5, 16.0, 16.0),
            r,
        );
        specular_gfx::draw_icon(cx.dl, Icon::Info, icon, cx.pal.accent, 1.5, false);
        let tr = cx.m_in(Rect::new(r.x + 34.0, r.y, b.width, ph), r);
        cx.block_in(&b, tr, HAlign::Start, cx.pal.text);
        cx.ring_if(&Target::Insight, main, ph * 0.5);
        cx.hit(main, Target::Insight);
        let close = cx.m_in(Rect::new(r.right() - close_w, r.y, close_w, ph), r);
        if cx.is_hover(&Target::InsightHide) {
            cx.dl.rect(close, ph * 0.5, cx.pal.surface_hover);
        }
        let ci = Rect::new(
            close.x + (close_w - 12.0) * 0.5,
            close.y + (ph - 12.0) * 0.5,
            12.0,
            12.0,
        );
        specular_gfx::draw_icon(cx.dl, Icon::Close, ci, cx.pal.text_muted, 1.5, false);
        cx.ring_if(&Target::InsightHide, close, ph * 0.5);
        cx.hit(close, Target::InsightHide);
        cx.dl.pop_opacity();
    }
}

/// A laid-out piece of the panel's scrolling body.
enum Part {
    Gap(f32),
    Text {
        b: Arc<TextBlock>,
        color: Color,
        gap: f32,
    },
    Bar {
        stages: Vec<(f32, Color)>,
        gap: f32,
    },
    Legend {
        swatch: Color,
        name: Arc<TextBlock>,
        value: Arc<TextBlock>,
    },
    /// An item with an optional button (the index into `now`).
    Item {
        text: Arc<TextBlock>,
        gain: Option<Arc<TextBlock>>,
        button: Option<(Arc<TextBlock>, usize)>,
    },
}

const BAR_H: f32 = 10.0;
const LEGEND_H: f32 = 20.0;

fn part_height(p: &Part, bh: f32) -> f32 {
    match p {
        Part::Gap(g) => *g,
        Part::Text { b, gap, .. } => b.height + gap,
        Part::Bar { gap, .. } => BAR_H + gap,
        Part::Legend { name, .. } => name.height.max(LEGEND_H),
        Part::Item { text, gain, button } => {
            let t = text.height + gain.as_ref().map_or(0.0, |g| g.height + 2.0);
            t.max(if button.is_some() { bh } else { 0.0 }) + 12.0
        }
    }
}

pub(crate) fn button(cx: &mut Ctx, r: Rect, b: &TextBlock, t: Target, primary: bool) {
    let hov = cx.is_hover(&t);
    let (fill, text) = if primary {
        (
            if hov {
                cx.pal.accent_fill.with_alpha(0.85)
            } else {
                cx.pal.accent_fill
            },
            cx.pal.accent_text,
        )
    } else {
        (
            if hov {
                cx.pal.surface_hover
            } else {
                cx.pal.surface_raised
            },
            cx.pal.text,
        )
    };
    cx.dl
        .rect_border(r, metrics::RADIUS_SM + 2.0, fill, cx.pal.separator, 1.0);
    cx.block_in(b, r, HAlign::Center, text);
    cx.hit(r, t);
}

/// The keyboard focus ring of a panel button: outside it, so it shows on the accent fill too.
fn button_ring(cx: &mut Ctx, r: Rect) {
    cx.button_ring(r, metrics::RADIUS_SM + 2.0);
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx) {
    let w = (cx.w - 48.0).min(cx.u(560.0));
    let inner_w = w - 48.0;
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let head_s = cx.style(metrics::FONT_BODY).weight(600.0);
    let section_s = cx.style(metrics::FONT_LABEL).weight(650.0);
    let small_s = cx.style(metrics::FONT_SMALL);
    let button_s = cx.style(metrics::FONT_LABEL).weight(550.0);
    let bh = cx.u(30.0);
    let ins = ch.insights.clone();

    let title = cx.layout(
        &t!("open-report-insights-title"),
        title_s,
        Some(inner_w - 40.0),
    );
    let mut parts = Vec::new();
    if !ins.bottleneck.is_empty() {
        parts.push(Part::Text {
            b: cx.layout(&ins.bottleneck, head_s, Some(inner_w)),
            color: cx.pal.text,
            gap: 4.0,
        });
    }
    if !ins.detail.is_empty() {
        parts.push(Part::Text {
            b: cx.layout(&ins.detail, small_s, Some(inner_w)),
            color: cx.pal.text_muted,
            gap: 14.0,
        });
    }
    if let Some((caption, stages)) = &ins.last {
        parts.push(Part::Text {
            b: cx.layout(caption, section_s, Some(inner_w)),
            color: cx.pal.text,
            gap: 8.0,
        });
        let total: f64 = stages.iter().map(|s| s.ms.max(0.0)).sum();
        let mut quiet = 0;
        let colours: Vec<Color> = stages
            .iter()
            .map(|s| {
                if s.main {
                    cx.pal.accent
                } else {
                    quiet += 1;
                    cx.pal
                        .text_muted
                        .with_alpha(if quiet % 2 == 1 { 0.55 } else { 0.3 })
                }
            })
            .collect();
        if total > 0.0 {
            parts.push(Part::Bar {
                stages: stages
                    .iter()
                    .zip(&colours)
                    .map(|(s, c)| ((s.ms.max(0.0) / total) as f32, *c))
                    .collect(),
                gap: 8.0,
            });
        }
        for (s, c) in stages.iter().zip(&colours) {
            let st = if s.main {
                small_s.weight(600.0)
            } else {
                small_s
            };
            let value = cx.layout(&s.value, st, None);
            parts.push(Part::Legend {
                swatch: *c,
                name: cx.layout(&s.name, st, Some(inner_w - value.width - 40.0)),
                value,
            });
        }
        parts.push(Part::Gap(10.0));
    }
    let item = |cx: &mut Ctx, parts: &mut Vec<Part>, it: &InsightItem, index: Option<usize>| {
        let button = match (&it.action, index) {
            (Some((label, _)), Some(i)) => Some((cx.layout(label, button_s, None), i)),
            _ => None,
        };
        let text_w = inner_w - button.as_ref().map_or(0.0, |b| b.0.width + 32.0 + 12.0);
        parts.push(Part::Item {
            text: cx.layout(&it.text, small_s, Some(text_w.max(80.0))),
            gain: it
                .gain
                .as_ref()
                .map(|g| cx.layout(g, small_s.weight(600.0), Some(text_w.max(80.0)))),
            button,
        });
    };
    for (heading, items, actions) in [
        (t!("open-report-insights-now"), &ins.now, true),
        (t!("open-report-insights-hardware"), &ins.hardware, false),
    ] {
        if items.is_empty() {
            continue;
        }
        parts.push(Part::Text {
            b: cx.layout(&heading, section_s, Some(inner_w)),
            color: cx.pal.text,
            gap: 8.0,
        });
        for (i, it) in items.iter().enumerate() {
            item(cx, &mut parts, it, actions.then_some(i));
        }
        parts.push(Part::Gap(4.0));
    }
    let foot_buttons: Vec<(Arc<TextBlock>, Target)> = [
        ins.dismiss.as_ref().map(|d| (d, Target::InsightDismiss)),
        ins.restore.as_ref().map(|d| (d, Target::InsightRestore)),
        ins.switch.as_ref().map(|s| (s, Target::InsightSwitch)),
    ]
    .into_iter()
    .flatten()
    .map(|((label, _), t)| (cx.layout(label, button_s, None), t))
    .collect();
    // The footer's buttons in rows that fit the width: (row, x offset) of each.
    let mut foot_at = Vec::with_capacity(foot_buttons.len());
    let (mut row, mut x) = (0usize, 0.0f32);
    for (b, _) in &foot_buttons {
        let bw = b.width + 32.0;
        if x > 0.0 && x + bw > inner_w {
            row += 1;
            x = 0.0;
        }
        foot_at.push((row, x));
        x += bw + 10.0;
    }
    let rows = foot_at.last().map_or(0, |a| a.0 + 1);

    let content: f32 = parts.iter().map(|p| part_height(p, bh)).sum();
    let head = title.height + 18.0;
    let foot = if rows == 0 {
        0.0
    } else {
        20.0 + bh * rows as f32 + 8.0 * (rows - 1) as f32
    };
    let h = (24.0 + head + content + foot + 20.0).min(cx.h - 32.0);
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
        (h - 24.0 - head - foot - 20.0).max(0.0),
    );
    let max = (content - body.h).max(0.0);
    // A button reached with Tab scrolls into view.
    if std::mem::take(&mut ch.reveal_focus)
        && let Some(Target::InsightAction(want)) = ch.modal_focus
    {
        let mut top = 0.0;
        for p in &parts {
            if let Part::Item {
                button: Some((_, i)),
                ..
            } = p
                && *i == want
            {
                let s = &mut ch.insights_scroll;
                *s = s.min(top).max(top + bh - body.h);
                break;
            }
            top += part_height(p, bh);
        }
    }
    ch.insights_scroll = ch.insights_scroll.clamp(0.0, max);
    cx.dl.push_clip(body);
    let mut ring = None;
    let mut y = body.y - ch.insights_scroll;
    for p in &parts {
        let ph = part_height(p, bh);
        match p {
            Part::Gap(_) => {}
            Part::Text { b, color, .. } => {
                let tr = cx.m_in(Rect::new(body.x, y, inner_w, b.height), body);
                cx.block_top(b, tr, HAlign::Start, *color);
            }
            Part::Bar { stages, .. } => {
                let bar = Rect::new(body.x, y, inner_w, BAR_H);
                cx.dl.rect(bar, BAR_H * 0.5, cx.pal.separator);
                cx.dl.push_clip(bar);
                let mut x = 0.0;
                for (share, c) in stages {
                    let sw = share * inner_w;
                    if sw > 0.0 {
                        // Hairline gaps between segments.
                        let seg = Rect::new(body.x + x, y, (sw - 1.0).max(1.0), BAR_H);
                        cx.dl.rect(cx.m_in(seg, bar), 0.0, *c);
                    }
                    x += sw;
                }
                cx.dl.pop_clip();
            }
            Part::Legend {
                swatch,
                name,
                value,
            } => {
                let row = Rect::new(body.x, y, inner_w, ph);
                let sw = cx.m_in(Rect::new(body.x, y + (ph - 10.0) * 0.5, 10.0, 10.0), row);
                cx.dl.rect(sw, 3.0, *swatch);
                let nr = cx.m_in(Rect::new(body.x + 18.0, y, name.width, ph), row);
                cx.block_in(name, nr, HAlign::Start, cx.pal.text_muted);
                let vr = cx.m_in(
                    Rect::new(row.right() - value.width, y, value.width, ph),
                    row,
                );
                cx.block_in(value, vr, HAlign::Start, cx.pal.text);
            }
            Part::Item {
                text,
                gain,
                button: b,
            } => {
                let row = Rect::new(body.x, y, inner_w, ph);
                let tr = cx.m_in(Rect::new(body.x, y, text.width, text.height), row);
                cx.block_top(text, tr, HAlign::Start, cx.pal.text);
                if let Some(g) = gain {
                    let gr = cx.m_in(
                        Rect::new(body.x, y + text.height + 2.0, g.width, g.height),
                        row,
                    );
                    cx.block_top(g, gr, HAlign::Start, cx.pal.accent);
                }
                if let Some((label, i)) = b {
                    let bw = label.width + 32.0;
                    let br = cx.m_in(Rect::new(row.right() - bw, y, bw, bh), row);
                    let t = Target::InsightAction(*i);
                    if cx.is_focused(&t) {
                        ring = Some(br);
                    }
                    button(cx, br, label, t, true);
                }
            }
        }
        y += ph;
    }
    cx.dl.pop_clip();
    if let Some(br) = ring {
        // Scrolled with the body but not cut at its sides, where the buttons end.
        cx.dl
            .push_clip(Rect::new(body.x - 4.0, body.y, body.w + 8.0, body.h));
        button_ring(cx, br);
        cx.dl.pop_clip();
    }
    crate::scrollbar::draw(
        ch,
        cx,
        crate::scrollbar::Area::Insights,
        body,
        Rect::new(r.x, body.y, r.w, body.h),
        max,
    );

    // The rows end 20 px above the sheet's bottom.
    let foot_y = r.bottom() - foot;
    for ((b, t), (row, x)) in foot_buttons.into_iter().zip(foot_at) {
        let bw = b.width + 32.0;
        let y = foot_y + row as f32 * (bh + 8.0);
        let br = cx.m_in(Rect::new(r.x + 24.0 + x, y, bw, bh), r);
        let focused = cx.is_focused(&t);
        button(cx, br, &b, t, false);
        if focused {
            button_ring(cx, br);
        }
    }
}

/// The panel's text for screen readers, in reading order (the buttons are nodes of their own).
pub(crate) fn lines(i: &Insights) -> Vec<String> {
    let mut out = Vec::new();
    for s in [&i.bottleneck, &i.detail] {
        if !s.is_empty() {
            out.push(s.clone());
        }
    }
    if let Some((caption, stages)) = &i.last {
        out.push(caption.clone());
        out.extend(stages.iter().map(|s| format!("{}: {}", s.name, s.value)));
    }
    for (heading, items) in [
        (t!("open-report-insights-now"), &i.now),
        (t!("open-report-insights-hardware"), &i.hardware),
    ] {
        if items.is_empty() {
            continue;
        }
        out.push(heading);
        for it in items {
            out.push(match &it.gain {
                Some(g) => format!("{} {g}", it.text),
                None => it.text.clone(),
            });
        }
    }
    out
}
