//! Viewer overlays drawn over the image: the navigator frame with its viewport rectangle,
//! the loupe ring and the zoom-rectangle selection, plus the move-to-trash confirmation.

use specular_gfx::Rect;
use specular_i18n::t;
use specular_theme::{Color, metrics};

use crate::Chrome;
use crate::ctx::{Ctx, HAlign, Target};

/// Overlay geometry from the application, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overlays {
    /// Navigator frame (where the application drew the mini image) and the part of it the
    /// window shows.
    pub navigator: Option<(Rect, Rect)>,
    /// Loupe centre and radius.
    pub loupe: Option<((f32, f32), f32)>,
    /// Zoom-rectangle selection being dragged.
    pub zoom_rect: Option<Rect>,
}

pub(crate) fn draw(cx: &mut Ctx, o: &Overlays) {
    if let Some((frame, view)) = o.navigator {
        // Dim what is outside the window's view, then outline both.
        let v = view.intersect(&frame);
        let dim = Color::rgba(0.0, 0.0, 0.0, 0.38);
        if v.w > 0.0 && v.h > 0.0 && (v.w < frame.w - 0.5 || v.h < frame.h - 0.5) {
            cx.dl.rect(
                Rect::from_min_max(frame.x, frame.y, frame.right(), v.y),
                0.0,
                dim,
            );
            cx.dl.rect(
                Rect::from_min_max(frame.x, v.bottom(), frame.right(), frame.bottom()),
                0.0,
                dim,
            );
            cx.dl
                .rect(Rect::from_min_max(frame.x, v.y, v.x, v.bottom()), 0.0, dim);
            cx.dl.rect(
                Rect::from_min_max(v.right(), v.y, frame.right(), v.bottom()),
                0.0,
                dim,
            );
            cx.dl
                .stroke_rect(v.inset(-1.0), 2.0, Color::rgba(0.0, 0.0, 0.0, 0.5), 1.0);
            cx.dl.stroke_rect(v, 1.5, cx.pal.accent, 1.5);
        }
        cx.dl
            .stroke_rect(frame.inset(-1.0), 3.0, cx.pal.glass_border, 1.0);
    }
    if let Some((c, r)) = o.loupe {
        cx.dl.arc(
            c,
            r + 1.5,
            0.0,
            std::f32::consts::TAU,
            3.0,
            Color::rgba(1.0, 1.0, 1.0, 0.85),
        );
        cx.dl.arc(
            c,
            r + 3.5,
            0.0,
            std::f32::consts::TAU,
            1.0,
            Color::rgba(0.0, 0.0, 0.0, 0.45),
        );
    }
    if let Some(z) = o.zoom_rect {
        cx.dl.rect(z, 0.0, cx.pal.accent.with_alpha(0.14));
        cx.dl.stroke_rect(z, 0.0, cx.pal.accent, 1.5);
    }
}

/// The "Move … to the trash?" sheet, and other confirmations ([`Chrome::confirm`]).
pub(crate) fn draw_confirm(ch: &Chrome, cx: &mut Ctx) {
    let w = cx.u(440.0).min(cx.w - 32.0);
    let ts = cx.style(metrics::FONT_BODY).weight(600.0);
    let (title, ok) = ch.confirm_texts();
    let tb = cx.layout(&title, ts, Some(w - 48.0));
    // What follows from it, under the question (resetting every setting).
    let db = ch
        .confirm_detail()
        .map(|d| cx.layout(&d, cx.style(metrics::FONT_SMALL), Some(w - 48.0)));
    let detail_h = db.as_ref().map_or(0.0, |b| 6.0 + b.height);
    let bs = cx.style(metrics::FONT_LABEL).weight(550.0);
    let cancel = t!("app-cancel");
    let cb = cx.layout(&cancel, bs, None);
    let ob = cx.layout(&ok, bs, None);
    let bh = cx.u(34.0);
    let ow = (ob.width + 32.0).min(w - 40.0);
    let cw = (cb.width + 32.0).min(w - 40.0);
    // The other choice of two ([`Chrome::confirm_choice`]), between Cancel and the default.
    let ab = ch.confirm_alt.as_ref().map(|(a, _)| cx.layout(a, bs, None));
    let aw = ab.as_ref().map_or(0.0, |b| (b.width + 32.0).min(w - 40.0));
    let alt_w = if ab.is_some() { aw + 8.0 } else { 0.0 };
    // A third choice ([`Chrome::confirm_choice3`]), between Cancel and the other.
    let a2b = ch
        .confirm_alt2
        .as_ref()
        .filter(|_| ab.is_some())
        .map(|(a, _)| cx.layout(a, bs, None));
    let a2w = a2b.as_ref().map_or(0.0, |b| (b.width + 32.0).min(w - 40.0));
    let alt2_w = if a2b.is_some() { a2w + 8.0 } else { 0.0 };
    // Side by side when they fit, else stacked (default button at the bottom).
    let stacked = ow + 8.0 + alt_w + alt2_w + cw > w - 40.0;
    let rows = 2.0 + f32::from(u8::from(ab.is_some())) + f32::from(u8::from(a2b.is_some()));
    let buttons_h = if stacked {
        bh * rows + 8.0 * (rows - 1.0)
    } else {
        bh
    };
    let h = 24.0 + tb.height + detail_h + 24.0 + buttons_h + 20.0;
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let tr = cx.m_in(Rect::new(r.x + 24.0, r.y + 24.0, w - 48.0, tb.height), r);
    cx.block_top(&tb, tr, HAlign::Start, cx.pal.text);
    if let Some(db) = &db {
        let y = r.y + 24.0 + tb.height + 6.0;
        let dr = cx.m_in(Rect::new(r.x + 24.0, y, w - 48.0, db.height), r);
        cx.block_top(db, dr, HAlign::Start, cx.pal.text_muted);
    }

    let by = r.bottom() - 20.0 - bh;
    let okr = cx.m_in(Rect::new(r.right() - 20.0 - ow, by, ow, bh), r);
    let ar = ab.as_ref().map(|_| {
        if stacked {
            cx.m_in(Rect::new(r.right() - 20.0 - aw, by - 8.0 - bh, aw, bh), r)
        } else {
            cx.m_in(Rect::new(r.right() - 20.0 - ow - 8.0 - aw, by, aw, bh), r)
        }
    });
    let a2r = a2b.as_ref().map(|_| {
        if stacked {
            cx.m_in(
                Rect::new(r.right() - 20.0 - a2w, by - 16.0 - 2.0 * bh, a2w, bh),
                r,
            )
        } else {
            cx.m_in(
                Rect::new(r.right() - 20.0 - ow - 8.0 - alt_w - a2w, by, a2w, bh),
                r,
            )
        }
    });
    let cr = if stacked {
        let y = by - (rows - 1.0) * (8.0 + bh);
        cx.m_in(Rect::new(r.right() - 20.0 - cw, y, cw, bh), r)
    } else {
        cx.m_in(
            Rect::new(
                r.right() - 20.0 - ow - 8.0 - alt_w - alt2_w - cw,
                by,
                cw,
                bh,
            ),
            r,
        )
    };
    let hov = cx.is_hover(&Target::ConfirmCancel);
    cx.dl.rect_border(
        cr,
        metrics::RADIUS_SM + 2.0,
        if hov {
            cx.pal.surface_hover
        } else {
            cx.pal.surface_raised
        },
        cx.pal.separator,
        1.0,
    );
    cx.block_in(&cb, cr, HAlign::Center, cx.pal.text);
    cx.hit(cr, Target::ConfirmCancel);
    if let (Some(ab), Some(ar)) = (&ab, ar) {
        let hov = cx.is_hover(&Target::ConfirmAlt);
        cx.dl.rect_border(
            ar,
            metrics::RADIUS_SM + 2.0,
            if hov {
                cx.pal.surface_hover
            } else {
                cx.pal.surface_raised
            },
            cx.pal.separator,
            1.0,
        );
        if cx.is_focused(&Target::ConfirmAlt) {
            cx.button_ring(ar, metrics::RADIUS_SM + 2.0);
        }
        cx.block_in(ab, ar, HAlign::Center, cx.pal.text);
        cx.hit(ar, Target::ConfirmAlt);
    }
    if let (Some(a2b), Some(a2r)) = (&a2b, a2r) {
        let hov = cx.is_hover(&Target::ConfirmAlt2);
        cx.dl.rect_border(
            a2r,
            metrics::RADIUS_SM + 2.0,
            if hov {
                cx.pal.surface_hover
            } else {
                cx.pal.surface_raised
            },
            cx.pal.separator,
            1.0,
        );
        if cx.is_focused(&Target::ConfirmAlt2) {
            cx.button_ring(a2r, metrics::RADIUS_SM + 2.0);
        }
        cx.block_in(a2b, a2r, HAlign::Center, cx.pal.text);
        cx.hit(a2r, Target::ConfirmAlt2);
    }
    let hov = cx.is_hover(&Target::ConfirmOk);
    // Moving to the trash, resetting every setting and destructive confirmations
    // ([`Chrome::confirm_destructive`]) are the dangerous ones; others use the accent.
    let base = if ch.confirm_custom.is_some() && ch.confirm_destructive.is_none() {
        cx.pal.accent_fill
    } else {
        cx.pal.danger_fill
    };
    let fill = if hov {
        base.lerp(cx.pal.accent_text, 0.12)
    } else {
        base
    };
    cx.dl.rect(okr, metrics::RADIUS_SM + 2.0, fill);
    // The default button (Enter) is the filled one; the keyboard focus (Tab) is a ring set
    // off from its button, on either.
    let radius = metrics::RADIUS_SM + 2.0;
    if cx.is_focused(&Target::ConfirmCancel) {
        cx.button_ring(cr, radius);
    } else if cx.is_focused(&Target::ConfirmOk) {
        cx.button_ring(okr, radius);
    }
    cx.block_in(&ob, okr, HAlign::Center, cx.pal.accent_text);
    cx.hit(okr, Target::ConfirmOk);
}
