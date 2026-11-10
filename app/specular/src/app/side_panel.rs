//! The shell of a panel docked at the trailing edge (the keyword panel; the IPTC editor is
//! to use it too): its frame, a title with a close button, one text field, a scrolling body
//! of rows with headers, and a row of buttons at its foot. Each part is drawn in the panel's
//! own left-to-right coordinates and mirrored inside it for right-to-left languages.
//!
//! The owner keeps the state (the field's [`LineEdit`], the scroll, the focus) and turns the
//! hit rectangles it gets back into actions; nothing here knows what the rows mean.

use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_theme::{Color, Palette};
use specular_widgets::line_edit::LineEdit;

/// Sizes and styles at a text scale.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Metrics {
    pub ts: f32,
    pub pad: f32,
    pub title_h: f32,
    pub field_h: f32,
    pub row_h: f32,
    pub head_h: f32,
    pub foot_h: f32,
    pub title: TextStyle,
    pub head: TextStyle,
    pub row: TextStyle,
    pub small: TextStyle,
}

impl Metrics {
    pub fn new(text_scale: f32) -> Metrics {
        let ts = text_scale.clamp(0.75, 3.0);
        let u = |v: f32| (v * ts).round();
        Metrics {
            ts,
            pad: u(12.0),
            title_h: u(40.0),
            field_h: u(28.0),
            row_h: u(28.0),
            head_h: u(28.0),
            foot_h: u(36.0),
            title: TextStyle::new(14.0 * ts).weight(600.0),
            head: TextStyle::new(11.0 * ts).weight(600.0),
            row: TextStyle::new(13.0 * ts),
            small: TextStyle::new(12.0 * ts),
        }
    }

    /// A length at this scale.
    pub fn u(&self, v: f32) -> f32 {
        (v * self.ts).round()
    }
}

/// The width a docked panel takes in a window `vw` wide.
pub(crate) fn width(vw: f32, text_scale: f32) -> f32 {
    let ts = text_scale.clamp(0.75, 3.0);
    let w = (280.0 * (1.0 + (ts - 1.0).max(0.0) * 0.5)).round();
    w.min(vw * 0.45).max(vw.min(200.0))
}

/// Draws into one panel.
pub(crate) struct Painter<'a> {
    pub dl: &'a mut DrawList,
    pub text: &'a mut Text,
    pub pal: &'a Palette,
    pub m: Metrics,
    pub panel: Rect,
    pub rtl: bool,
    pub pointer: Option<(f32, f32)>,
}

impl Painter<'_> {
    /// `r` (in the panel's left-to-right layout) as drawn.
    pub fn flip(&self, r: Rect) -> Rect {
        if self.rtl {
            Rect::new(self.panel.x + self.panel.right() - r.right(), r.y, r.w, r.h)
        } else {
            r
        }
    }

    pub fn hovered(&self, r: Rect) -> bool {
        self.pointer.is_some_and(|p| r.contains(p))
    }

    /// The frame and the title (with `note` after it, muted); returns the close button's
    /// rectangle (as drawn) and where the content starts.
    pub fn frame(&mut self, title: &str, note: Option<&str>) -> (Rect, f32) {
        let (pal, m, panel) = (self.pal, self.m, self.panel);
        self.dl.shadow(panel, 0.0, pal.shadow, 10.0, 0.0);
        self.dl.rect(panel, 0.0, pal.surface_raised);
        let edge = self.flip(Rect::new(panel.x, panel.y, 1.0, panel.h));
        self.dl.rect(edge, 0.0, pal.separator);
        let cs = m.u(28.0);
        let close = self.flip(Rect::new(
            panel.right() - m.pad - cs + m.u(6.0),
            panel.y + (m.title_h - cs) * 0.5,
            cs,
            cs,
        ));
        if self.hovered(close) {
            self.dl.rect(close, m.u(6.0), pal.surface_hover);
        }
        let ic = m.u(14.0);
        draw_icon(
            self.dl,
            Icon::Close,
            Rect::new(close.x + (cs - ic) * 0.5, close.y + (cs - ic) * 0.5, ic, ic),
            pal.text_muted,
            1.5,
            false,
        );
        let tw = panel.w - 2.0 * m.pad - cs;
        let tr = Rect::new(panel.x + m.pad, panel.y, tw, m.title_h);
        let t = fit(
            self.text,
            title,
            m.title,
            tw * if note.is_some() { 0.6 } else { 1.0 },
        );
        let used = self.label(&t, m.title, tr, pal.text);
        if let Some(n) = note {
            let nr = Rect::new(
                tr.x + used + m.u(8.0),
                panel.y,
                (tw - used - m.u(8.0)).max(0.0),
                m.title_h,
            );
            let n = fit(self.text, n, m.small, nr.w);
            self.label(&n, m.small, nr, pal.text_muted);
        }
        (close, panel.y + m.title_h)
    }

    /// `s` in `r` (left-to-right layout), vertically centred; returns its width.
    pub fn label(&mut self, s: &str, style: TextStyle, r: Rect, c: Color) -> f32 {
        let d = self.flip(r);
        let b = self.text.layout(s, style, None, Align::Start);
        let x = if self.rtl { d.right() - b.width } else { d.x };
        self.text
            .draw(self.dl, &b, x, d.y + (d.h - b.height) * 0.5, c);
        b.width
    }

    /// `s` at the trailing end of `r`.
    pub fn label_end(&mut self, s: &str, style: TextStyle, r: Rect, c: Color) -> f32 {
        let d = self.flip(r);
        let b = self.text.layout(s, style, None, Align::Start);
        let x = if self.rtl { d.x } else { d.right() - b.width };
        self.text
            .draw(self.dl, &b, x, d.y + (d.h - b.height) * 0.5, c);
        b.width
    }

    /// The text field at `y` (full width less padding) with `edit`'s text or `placeholder`,
    /// the caret when `focused`; returns its rectangle as drawn.
    pub fn field(&mut self, y: f32, edit: &LineEdit, placeholder: &str, focused: bool) -> Rect {
        let (pal, m, panel) = (self.pal, self.m, self.panel);
        let lr = Rect::new(panel.x + m.pad, y, panel.w - 2.0 * m.pad, m.field_h);
        let r = self.flip(lr);
        self.dl.rect_border(
            r,
            m.u(6.0),
            pal.canvas,
            if focused {
                pal.focus_ring
            } else {
                pal.separator
            },
            if focused { 1.5 } else { 1.0 },
        );
        let inner = Rect::new(lr.x + m.u(8.0), y, lr.w - m.u(16.0), m.field_h);
        if edit.is_empty() {
            let ph = fit(self.text, placeholder, m.row, inner.w);
            self.label(&ph, m.row, inner, pal.text_faint);
        } else {
            let sel = edit.selection();
            if focused && !sel.is_empty() {
                let x0 = self.text.measure(&edit.text()[..sel.start], m.row);
                let x1 = self.text.measure(&edit.text()[..sel.end], m.row);
                let s = self.flip(Rect::new(
                    inner.x + x0,
                    y + m.u(5.0),
                    x1 - x0,
                    m.field_h - m.u(10.0),
                ));
                self.dl.rect(s, 0.0, pal.selection);
            }
            self.dl.push_clip(r);
            self.label(edit.text(), m.row, inner, pal.text);
            self.dl.pop_clip();
        }
        if focused {
            let cx = self
                .text
                .measure(&edit.text()[..edit.caret()], m.row)
                .min(inner.w);
            let ch = (m.row.size * 1.25).round();
            let c = self.flip(Rect::new(
                inner.x + cx,
                y + ((m.field_h - ch) * 0.5).round(),
                1.5,
                ch,
            ));
            self.dl.rect(c, 0.0, pal.text);
        }
        r
    }

    /// A section header at `y` in the body.
    pub fn header(&mut self, y: f32, s: &str) {
        let (m, panel) = (self.m, self.panel);
        let r = Rect::new(
            panel.x + m.pad,
            y + m.u(6.0),
            panel.w - 2.0 * m.pad,
            m.head_h - m.u(6.0),
        );
        let s = fit(self.text, s, m.head, r.w);
        self.label(&s, m.head, r, self.pal.text_muted);
    }

    /// The highlight of a row: hover fill and the focus ring. `r` as drawn.
    pub fn row_back(&mut self, r: Rect, focused: bool) {
        let rad = self.m.u(6.0);
        if self.hovered(r) {
            self.dl.rect(r, rad, self.pal.surface_hover);
        }
        if focused {
            self.dl.stroke_rect(r, rad, self.pal.focus_ring, 1.5);
        }
    }

    /// A check box: on, partly on (a dash) or off. `r` as drawn.
    pub fn check(&mut self, r: Rect, state: Check) {
        let (pal, rad) = (self.pal, self.m.u(3.0));
        match state {
            Check::On => {
                self.dl.rect(r, rad, pal.accent_fill);
                draw_icon(
                    self.dl,
                    Icon::Check,
                    r.inset(1.0),
                    pal.accent_text,
                    2.0,
                    false,
                );
            }
            Check::Some => {
                self.dl.rect(r, rad, pal.accent_fill);
                let h = (r.h * 0.14).max(1.5);
                self.dl.rect(
                    Rect::new(r.x + r.w * 0.25, r.y + (r.h - h) * 0.5, r.w * 0.5, h),
                    0.0,
                    pal.accent_text,
                );
            }
            Check::Off => self.dl.stroke_rect(r, rad, pal.text_muted, 1.2),
        }
    }

    /// A row of buttons `foot_h` tall at `y`, from the leading edge, under a separator;
    /// returns each one's rectangle as drawn. Each is (label, switched on, enabled).
    pub fn foot(&mut self, y: f32, labels: &[(&str, bool, bool)]) -> Vec<Rect> {
        let (pal, m, panel) = (self.pal, self.m, self.panel);
        self.dl.rect(
            self.flip(Rect::new(panel.x, y, panel.w, 1.0)),
            0.0,
            pal.separator,
        );
        let mut x = panel.x + m.pad;
        let bh = m.u(26.0);
        let mut out = Vec::new();
        for &(l, on, enabled) in labels {
            let w = self.text.measure(l, m.small) + m.u(16.0);
            let lr = Rect::new(x, y + (m.foot_h - bh) * 0.5, w, bh);
            let r = self.flip(lr);
            let fill = if on {
                pal.accent_soft
            } else if enabled && self.hovered(r) {
                pal.surface_hover
            } else {
                pal.surface
            };
            self.dl.rect_border(r, m.u(6.0), fill, pal.separator, 1.0);
            let c = if enabled { pal.text } else { pal.text_faint };
            self.label(
                l,
                m.small,
                Rect::new(lr.x + m.u(8.0), lr.y, lr.w - m.u(16.0), bh),
                c,
            );
            out.push(r);
            x += w + m.u(6.0);
        }
        out
    }
}

/// A check box's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Check {
    On,
    Some,
    Off,
}

/// `s` cut at the end with an ellipsis to fit `w`.
pub(crate) fn fit(text: &mut Text, s: &str, style: TextStyle, w: f32) -> String {
    if text.measure(s, style) <= w {
        return s.to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let t: String = chars[..mid].iter().collect::<String>() + "…";
        if text.measure(&t, style) <= w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().collect::<String>() + "…"
}

/// The scroll that keeps `y..y + h` (content coordinates) inside a body `body_h` tall.
pub(crate) fn scroll_to(scroll: f32, y: f32, h: f32, body_h: f32) -> f32 {
    if y < scroll {
        y
    } else if y + h > scroll + body_h {
        y + h - body_h
    } else {
        scroll
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolls_rows_into_view() {
        assert_eq!(scroll_to(0.0, 10.0, 20.0, 100.0), 0.0);
        assert_eq!(scroll_to(50.0, 10.0, 20.0, 100.0), 10.0);
        assert_eq!(scroll_to(0.0, 100.0, 20.0, 100.0), 20.0);
    }

    #[test]
    fn width_follows_the_window() {
        assert_eq!(width(1600.0, 1.0), 280.0);
        assert!(width(400.0, 1.0) <= 200.0);
        assert!(width(1600.0, 2.0) > 280.0);
    }
}
