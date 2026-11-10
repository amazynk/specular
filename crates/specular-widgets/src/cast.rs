//! The "Playing on" card: while a movie or sound file plays on a TV or speaker, a glass card
//! over the middle of the viewer names the device, says how it is going ("Connecting…",
//! "Paused", why it stopped) and offers "Stop casting". The transport below drives the
//! device.

use specular_gfx::Rect;
use specular_i18n::{isolate, t};
use specular_theme::metrics;

use crate::Chrome;
use crate::ctx::{Ctx, HAlign, Target};
use crate::player::CastView;

/// "Playing on Living Room TV".
pub(crate) fn title(c: &CastView) -> Option<String> {
    let name = c.playing_on.as_deref()?;
    Some(t!("cast-playing-on", device = isolate(name)))
}

impl Chrome {
    /// Draws the card when something plays on another device.
    pub(crate) fn draw_casting(&self, cx: &mut Ctx) {
        let Some(c) = self.player.as_ref().map(|p| &p.cast) else {
            return;
        };
        let Some(title) = title(c) else { return };
        let pad = 18.0;
        let maxw = (cx.w - 48.0).clamp(160.0, 420.0);
        let text_w = maxw - pad * 2.0;
        let title_s = cx.style(metrics::FONT_BODY).weight(650.0);
        let small = cx.style(metrics::FONT_SMALL);
        let body = cx.style(metrics::FONT_BODY);
        let tb = cx.layout(&title, title_s, Some(text_w));
        let sb = c
            .status
            .as_deref()
            .map(|s| cx.layout(s, small, Some(text_w)));
        let stop = t!("cast-stop");
        let lb = cx.layout(&stop, body, None);
        let bh = cx.u(30.0);
        let bw = (lb.width + 28.0).min(text_w);
        let h = pad + tb.height + sb.as_ref().map_or(0.0, |b| b.height + 6.0) + 14.0 + bh + pad;
        let w = tb
            .width
            .max(sb.as_ref().map_or(0.0, |b| b.width))
            .max(bw)
            .min(text_w)
            + pad * 2.0;
        // Over the middle of the picture, clear of the transport and toolbar below.
        let floor = cx.h - metrics::TOOLBAR_MARGIN - metrics::TOOLBAR_HEIGHT - 90.0;
        let y = ((cx.h - h) * 0.5 - 30.0).min(floor - h).max(8.0);
        let r = Rect::new(((cx.w - w) * 0.5).round(), y.round(), w.ceil(), h.ceil());
        cx.dl
            .shadow(r, metrics::RADIUS_MD, cx.pal.shadow, 20.0, 6.0);
        cx.dl.glass(
            r,
            metrics::RADIUS_MD,
            cx.pal.glass,
            cx.pal.glass_border,
            1.0,
        );
        cx.popovers.push(r);
        let mut yy = r.y + pad;
        let tr = Rect::new(r.x + pad, yy, w - pad * 2.0, tb.height);
        cx.block_top(&tb, tr, HAlign::Center, cx.pal.text);
        yy += tb.height;
        if let Some(b) = &sb {
            yy += 6.0;
            let sr = Rect::new(r.x + pad, yy, w - pad * 2.0, b.height);
            cx.block_top(b, sr, HAlign::Center, cx.pal.text_muted);
            yy += b.height;
        }
        yy += 14.0;
        let br = Rect::new((r.x + (w - bw) * 0.5).round(), yy.round(), bw, bh);
        let tg = Target::CastStop;
        let radius = metrics::RADIUS_SM + 2.0;
        let fill = if cx.is_pressed(&tg) {
            cx.pal.surface_pressed
        } else if cx.is_hover(&tg) {
            cx.pal.surface_hover
        } else {
            cx.pal.surface
        };
        cx.dl.rect_border(br, radius, fill, cx.pal.separator, 1.0);
        cx.block_in(&lb, br, HAlign::Center, cx.pal.text);
        cx.ring_if(&tg, br, radius);
        cx.hit(br, tg);
    }
}
