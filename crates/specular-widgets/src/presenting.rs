//! The "presenting on" control: while a slideshow runs on another display, or the viewer
//! shows the selection there, the main window keeps a small glass pill at the bottom end
//! corner ("Presenting on HDMI-A-1") with Pause / Resume (a slideshow) and Stop.

use specular_gfx::{Icon, Rect, draw_icon};
use specular_i18n::{isolate_ltr, t};
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Presenting};

/// What the pill says.
pub(crate) fn text(p: &Presenting) -> String {
    let display = isolate_ltr(&p.display);
    if p.paused.is_some() {
        t!("presenting-on", display = display)
    } else {
        t!("presenting-viewer-on", display = display)
    }
}

/// The pause button's name (and tooltip): what pressing it does.
pub(crate) fn pause_label(paused: bool) -> String {
    if paused {
        t!("presenting-resume")
    } else {
        t!("presenting-pause")
    }
}

/// The stop button's name (and tooltip): a slideshow's, or the viewer's on the other display.
pub(crate) fn stop_label(show: bool) -> String {
    if show {
        t!("presenting-stop")
    } else {
        t!("presenting-close-viewer")
    }
}

impl Chrome {
    /// Draws the pill. Above the toolbar in the viewer, above the gallery's bar in the gallery;
    /// at the end side (the insights indicator takes the start side).
    pub(crate) fn draw_presenting(&self, cx: &mut Ctx) {
        let Some(p) = &self.presenting_on else {
            return;
        };
        let style = cx.style(metrics::FONT_SMALL).weight(550.0);
        let max_w = (cx.w * 0.45).max(80.0);
        let b = cx.layout(&text(p), style, Some(max_w));
        let ph = cx.u(28.0).max(b.height + 8.0);
        let btn = ph;
        let buttons = if p.paused.is_some() { 2.0 } else { 1.0 };
        let pw = 12.0 + b.width + 6.0 + btn * buttons;
        let y = if self.gallery {
            cx.h - self.gallery_bar - 10.0 - ph
        } else {
            cx.h - metrics::TOOLBAR_MARGIN - metrics::TOOLBAR_HEIGHT - 10.0 - ph
        };
        let x = cx.w - metrics::TOOLBAR_MARGIN - pw.ceil();
        let r = cx.m(Rect::new(x, y.round(), pw.ceil(), ph));
        cx.dl
            .glass(r, ph * 0.5, cx.pal.glass, cx.pal.glass_border, 1.0);
        let tr = cx.m_in(Rect::new(r.x + 12.0, r.y, b.width, ph), r);
        cx.block_in(&b, tr, HAlign::Start, cx.pal.text);
        let mut bx = r.x + 12.0 + b.width + 6.0;
        let mut button = |cx: &mut Ctx, t: Target, icon: Icon| {
            let br = cx.m_in(Rect::new(bx, r.y, btn, ph), r);
            if cx.is_pressed(&t) {
                cx.dl.rect(br, ph * 0.5, cx.pal.surface_pressed);
            } else if cx.is_hover(&t) {
                cx.dl.rect(br, ph * 0.5, cx.pal.surface_hover);
            }
            let s = 12.0;
            let ir = Rect::new(br.x + (btn - s) * 0.5, br.y + (ph - s) * 0.5, s, s);
            draw_icon(cx.dl, icon, ir, cx.pal.text, 1.5, false);
            cx.ring_if(&t, br, ph * 0.5);
            cx.hit(br, t);
            bx += btn;
        };
        if let Some(paused) = p.paused {
            let icon = if paused { Icon::Play } else { Icon::Pause };
            button(cx, Target::PresentingPause(paused), icon);
        }
        button(cx, Target::PresentingStop(p.paused.is_some()), Icon::Close);
    }
}
