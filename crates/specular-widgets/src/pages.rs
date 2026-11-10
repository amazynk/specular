//! The page stepper: a file with several images (ICNS sizes, texture layers, faces and
//! slices, TIFF pages) shows a small glass bar above the toolbar, stacked above the
//! animation transport when both are up: previous page, "Page 3 of 12 · Face +X", next page.
//! It fades and slides with the toolbar. Unlike the media transport it follows the reading
//! direction: in right-to-left languages "previous" sits on the right, its chevron mirrored.

use specular_gfx::{Icon, Rect};
use specular_i18n::t;
use specular_input::Action;
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{PageBadge, ViewerState};

/// "Page 3 of 12".
pub(crate) fn position_text(p: &PageBadge) -> String {
    t!(
        "page-indicator",
        index = i64::from(p.index) + 1,
        count = i64::from(p.count)
    )
}

/// "Page 3 of 12, Face +X": what a screen reader hears on a page change and the page
/// buttons' value.
pub(crate) fn spoken(p: &PageBadge) -> String {
    match p.label {
        Some(l) => t!(
            "page-indicator-labelled",
            page = position_text(p),
            label = l
        ),
        None => position_text(p),
    }
}

/// Whether the page button for `a` can move: the page keys stop at the ends.
pub(crate) fn can_step(p: &PageBadge, a: Action) -> bool {
    match a {
        Action::PrevPage | Action::FirstPage => p.index > 0,
        Action::NextPage | Action::LastPage => p.index + 1 < p.count,
        _ => true,
    }
}

struct Parts {
    pos: std::sync::Arc<specular_gfx::TextBlock>,
    pos_w: f32,
    label: Option<std::sync::Arc<specular_gfx::TextBlock>>,
}

fn layout(cx: &mut Ctx, st: &ViewerState, p: &PageBadge, alpha: f32) -> (Rect, Parts) {
    let style = cx.style(metrics::FONT_LABEL).tabular().weight(500.0);
    let pos = cx.layout(&position_text(p), style, None);
    // As wide as the widest page number, so the bar keeps its size while stepping.
    let widest = t!(
        "page-indicator",
        index = i64::from(p.count),
        count = i64::from(p.count)
    );
    let pos_w = pos.width.max(cx.layout(&widest, style, None).width);
    let label = p
        .label
        .map(|l| cx.layout(l, cx.style(metrics::FONT_LABEL), None));
    // Buttons keep the toolbar's size; the bar grows only as far as large text needs.
    let b = metrics::BUTTON;
    let pad = 6.0;
    let gap = 2.0;
    let text_w = pos_w + label.as_ref().map_or(0.0, |l| 10.0 + l.width) + 20.0;
    let total = (pad * 2.0 + b * 2.0 + gap * 2.0 + text_w)
        .min(cx.w - 2.0 * metrics::TOOLBAR_MARGIN)
        .ceil();
    let h = b.max(pos.height + 8.0).ceil() + 12.0;
    let slide = (1.0 - alpha) * 10.0;
    let toolbar_top = cx.h - metrics::TOOLBAR_MARGIN - metrics::TOOLBAR_HEIGHT;
    // Above the transport bar (transport.rs) when an animation shows one.
    let mut bottom = toolbar_top - 8.0;
    if st.playback.is_some() {
        bottom -= metrics::BUTTON + 12.0 + 8.0;
    }
    let bar = Rect::new(((cx.w - total) * 0.5).round(), bottom - h + slide, total, h);
    (bar, Parts { pos, pos_w, label })
}

/// Draws the stepper; returns its top edge (where toasts start stacking).
pub(crate) fn draw(cx: &mut Ctx, st: &ViewerState, p: &PageBadge, alpha: f32) -> f32 {
    if alpha <= 0.0 {
        return cx.h;
    }
    let (bar, parts) = layout(cx, st, p, alpha);
    let h = bar.h;
    let b = metrics::BUTTON;
    let pad = 6.0;
    cx.dl.push_opacity(alpha);
    cx.dl.shadow(
        bar,
        h * 0.5,
        cx.pal.shadow,
        metrics::SHADOW_BLUR,
        metrics::SHADOW_Y,
    );
    cx.dl
        .glass(bar, h * 0.5, cx.pal.glass, cx.pal.glass_border, 1.0);
    cx.hit(bar, Target::ToolbarBg);

    let by = bar.y + (h - b) * 0.5;
    let button = |cx: &mut Ctx, x: f32, icon: Icon, a: Action| {
        let r = cx.m_in(Rect::new(x, by, b, b), bar);
        let t = Target::Transport(a);
        let enabled = t.disabled(st).is_none() && can_step(p, a);
        cx.icon_button_e(r, icon, t, cx.pal.text, cx.rtl, false, enabled);
    };
    button(cx, bar.x + pad, Icon::ChevronLeft, Action::PrevPage);
    button(
        cx,
        bar.right() - pad - b,
        Icon::ChevronRight,
        Action::NextPage,
    );

    // The text between the buttons: the position, then the page's label, muted.
    let inner = Rect::new(
        bar.x + pad + b + 2.0,
        bar.y,
        bar.w - 2.0 * (pad + b + 2.0),
        h,
    );
    let label_w = parts.label.as_ref().map_or(0.0, |l| 10.0 + l.width);
    let used = parts.pos_w + label_w;
    let x0 = inner.x + ((inner.w - used) * 0.5).max(0.0);
    let (ty, th) = (bar.y + 6.0, h - 12.0);
    let pr = cx.m_in(Rect::new(x0, ty, parts.pos_w, th), bar);
    cx.block_in(&parts.pos, pr, HAlign::Center, cx.pal.text);
    if let Some(l) = &parts.label {
        let room = (inner.right() - (x0 + parts.pos_w + 10.0)).max(0.0);
        let lr = cx.m_in(
            Rect::new(x0 + parts.pos_w + 10.0, ty, l.width.min(room), th),
            bar,
        );
        cx.block_in(l, lr, HAlign::Start, cx.pal.text_muted);
    }
    cx.dl.pop_opacity();
    bar.y
}
