//! The card the viewer shows in place of a file that opens only with an optional decoder
//! from another vendor that is not on this computer (Blackmagic RAW, DWG): a title, a few
//! plain lines, and its buttons in a row ("Get it", "I've installed it", "Locate…"). The
//! first button is the primary one.
//!
//! The widgets know nothing of decoders: the application builds the text
//! ([`Chrome::set_decoder_card`]) and gets [`Command::DecoderCard`] back with the button's
//! position.

use specular_gfx::{Icon, Rect, draw_icon};
use specular_theme::metrics;

use crate::Chrome;
use crate::ctx::{Ctx, HAlign, Target};

/// What the card says (already in the user's language).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecoderCard {
    /// Sent back with [`crate::Command::DecoderCard`].
    pub id: String,
    /// "Blackmagic RAW needs Blackmagic Design's free decoder".
    pub title: String,
    /// One idea per line.
    pub lines: Vec<String>,
    /// Button labels, in order (the first is the primary one).
    pub buttons: Vec<String>,
}

impl Chrome {
    /// Shows `card` in place of the current file's error message (or, with `None`, the plain
    /// message again). Returns whether anything changed.
    pub fn set_decoder_card(&mut self, card: Option<DecoderCard>) -> bool {
        let changed = self.decoder_card != card;
        self.decoder_card = card;
        changed
    }

    /// Whether the viewer's centre message is a notice rather than an error (an office
    /// document without a preview): drawn in the muted colour. Returns whether it changed.
    pub fn set_notice(&mut self, on: bool) -> bool {
        let changed = self.notice != on;
        self.notice = on;
        changed
    }

    /// The card on screen now, if any.
    pub fn decoder_card(&self) -> Option<&DecoderCard> {
        self.decoder_card.as_ref()
    }

    pub(crate) fn decoder_card_click(&self, i: u8) -> Option<crate::Command> {
        let c = self.decoder_card.as_ref()?;
        (usize::from(i) < c.buttons.len()).then(|| crate::Command::DecoderCard {
            id: c.id.clone(),
            button: usize::from(i),
        })
    }

    /// The label of button `i` (accessibility, tooltips).
    pub(crate) fn decoder_card_button(&self, i: u8) -> Option<&str> {
        self.decoder_card
            .as_ref()?
            .buttons
            .get(usize::from(i))
            .map(String::as_str)
    }
}

/// Draws the card in the middle of the window.
pub(crate) fn draw(cx: &mut Ctx, card: &DecoderCard) {
    let pad = 24.0;
    let maxw = (cx.w - 48.0).clamp(160.0, 520.0);
    let text_w = maxw - pad * 2.0;
    let title_s = cx.style(metrics::FONT_BODY).weight(650.0);
    let line_s = cx.style(metrics::FONT_BODY);
    let title = cx.layout(&card.title, title_s, Some(text_w));
    let lines: Vec<_> = card
        .lines
        .iter()
        .map(|l| cx.layout(l, line_s, Some(text_w)))
        .collect();
    let labels: Vec<_> = card
        .buttons
        .iter()
        .map(|b| cx.layout(b, line_s, None))
        .collect();
    let bh = cx.u(32.0);
    let gap = 8.0;
    // Buttons in one row when they fit, else one per row.
    let widths: Vec<f32> = labels.iter().map(|b| b.width + 28.0).collect();
    let row_w: f32 = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
    let one_row = row_w <= text_w;
    let buttons_h = if labels.is_empty() {
        0.0
    } else if one_row {
        bh
    } else {
        labels.len() as f32 * (bh + gap) - gap
    };
    let icon = 28.0;
    let lines_h: f32 = lines.iter().map(|b| b.height + 6.0).sum();
    let h = pad + icon + 12.0 + title.height + 10.0 + lines_h + 12.0 + buttons_h + pad;
    let r = Rect::new(
        ((cx.w - maxw) * 0.5).round(),
        ((cx.h - h) * 0.5 - 20.0).max(8.0).round(),
        maxw,
        h,
    );
    cx.dl.rect_border(
        r,
        metrics::RADIUS_MD,
        cx.pal.surface_raised,
        cx.pal.separator,
        1.0,
    );
    let ir = cx.m_in(Rect::new(r.x + pad, r.y + pad, icon, icon), r);
    draw_icon(cx.dl, Icon::Alert, ir, cx.pal.accent, 1.8, false);
    let mut y = r.y + pad + icon + 12.0;
    let tr = cx.m_in(Rect::new(r.x + pad, y, text_w, title.height), r);
    cx.block_top(&title, tr, HAlign::Start, cx.pal.text);
    y += title.height + 10.0;
    for b in &lines {
        let lr = cx.m_in(Rect::new(r.x + pad, y, text_w, b.height), r);
        cx.block_top(b, lr, HAlign::Start, cx.pal.text_muted);
        y += b.height + 6.0;
    }
    y += 12.0;
    let mut x = r.x + pad;
    for (i, (b, w)) in labels.iter().zip(&widths).enumerate() {
        let w = if one_row { *w } else { text_w };
        let br = cx.m_in(Rect::new(x, y, w, bh), r);
        let t = Target::DecoderCard(i as u8);
        let hov = cx.is_hover(&t);
        let radius = metrics::RADIUS_SM + 2.0;
        if i == 0 {
            let fill = if hov {
                cx.pal.accent_fill.lerp(cx.pal.accent_text, 0.12)
            } else {
                cx.pal.accent_fill
            };
            cx.dl.rect(br, radius, fill);
            cx.block_in(b, br, HAlign::Center, cx.pal.accent_text);
        } else {
            let fill = if hov {
                cx.pal.surface_hover
            } else {
                cx.pal.surface
            };
            cx.dl.rect_border(br, radius, fill, cx.pal.separator, 1.0);
            cx.block_in(b, br, HAlign::Center, cx.pal.text);
        }
        if cx.is_focused(&t) {
            cx.button_ring(br, radius);
        }
        cx.hit(br, t);
        if one_row {
            x += w + gap;
        } else {
            y += bh + gap;
        }
    }
}
