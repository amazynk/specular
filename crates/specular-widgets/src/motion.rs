//! The motion badge: a still with a short video clip (Motion Photo, Live Photo) shows a small
//! glass pill at the top start corner, "▶ Live" or "▶ Motion". It is the play control: pressed,
//! the clip plays in place of the still, and the badge (now "✕ Live") brings the still back.

use specular_gfx::{Icon, Rect, draw_icon};
use specular_i18n::t;
use specular_theme::metrics;

use crate::MotionBadge;
use crate::ctx::{Ctx, HAlign, Target};

/// What the badge says.
pub(crate) fn badge_text(m: &MotionBadge) -> String {
    if m.live {
        t!("motion-badge-live")
    } else {
        t!("motion-badge")
    }
}

/// What pressing the badge does (its tooltip and accessible name).
pub(crate) fn label(m: &MotionBadge) -> String {
    match (m.playing, m.live) {
        (true, _) => t!("motion-stop"),
        (false, true) => t!("motion-play-live"),
        (false, false) => t!("motion-play"),
    }
}

pub(crate) fn draw(cx: &mut Ctx, m: &MotionBadge, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let style = cx.style(metrics::FONT_SMALL).weight(600.0);
    let b = cx.layout(&badge_text(m), style, None);
    let ph = cx.u(28.0).max(b.height + 8.0);
    let icon = 14.0;
    let pw = 10.0 + icon + 6.0 + b.width + 12.0;
    let t = Target::Motion;
    let r = cx.m(Rect::new(
        metrics::TOOLBAR_MARGIN,
        metrics::TOOLBAR_MARGIN,
        pw.ceil(),
        ph,
    ));
    cx.dl.push_opacity(alpha);
    cx.dl
        .glass(r, ph * 0.5, cx.pal.glass, cx.pal.glass_border, 1.0);
    if cx.is_pressed(&t) {
        cx.dl.rect(r, ph * 0.5, cx.pal.surface_pressed);
    } else if cx.is_hover(&t) {
        cx.dl.rect(r, ph * 0.5, cx.pal.surface_hover);
    }
    let colour = if m.playing {
        cx.pal.accent
    } else {
        cx.pal.text
    };
    let ir = cx.m_in(
        Rect::new(r.x + 10.0, r.y + (ph - icon) * 0.5, icon, icon),
        r,
    );
    let glyph = if m.playing { Icon::Close } else { Icon::Play };
    draw_icon(cx.dl, glyph, ir, colour, 1.5, false);
    let tr = cx.m_in(Rect::new(r.x + 10.0 + icon + 6.0, r.y, b.width, ph), r);
    cx.block_in(&b, tr, HAlign::Start, colour);
    cx.ring_if(&t, r, ph * 0.5);
    cx.hit(r, t);
    cx.dl.pop_opacity();
}
