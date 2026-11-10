//! "Go to time…": the small field over a movie's time readout and what it understands.
//!
//! - `1:23:45`, `83:20`, `5025`: a time from the start (hours, minutes and seconds; a bare
//!   number is seconds; a fraction of a second after a point).
//! - `50%`: that part of the movie.
//! - `+30`, `-1:00`: forward or back from where it is.
//!
//! Digits of any script are taken (the field keeps them as ASCII); the place found is kept
//! within the movie.

use std::sync::Arc;

use specular_gfx::{Rect, TextStyle};
use specular_i18n::t;
use specular_theme::metrics;

use crate::ctx::{Ctx, FieldGeom, HAlign};
use crate::line_edit::{Filter, LineEdit};

/// The field's filter: digits of any script, `:`, `.`, `%`, `+` and `-` (a minus sign, a
/// decimal comma and the Arabic percent sign are taken for them).
pub(crate) const FILTER: Filter = Filter {
    map: field_char,
    valid: field_valid,
};

fn field_char(c: char) -> Option<char> {
    if let Some(d) = crate::digit_value(c) {
        return Some(char::from(b'0' + d));
    }
    match c {
        ':' | '.' | '%' | '+' | '-' => Some(c),
        '\u{2212}' | '\u{2013}' => Some('-'),
        ',' | '\u{066B}' => Some('.'),
        '\u{066A}' | '\u{FF05}' => Some('%'),
        '\u{FF1A}' => Some(':'),
        _ => None,
    }
}

fn field_valid(t: &str) -> bool {
    t.len() <= 16
}

/// `h:m:s`, `m:s` or `s` (each part a number; the last may have a fraction).
fn clock(t: &str) -> Option<f64> {
    let parts: Vec<&str> = t.split(':').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let mut secs = 0.0;
    for (i, p) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        if !p.bytes().all(|b| b.is_ascii_digit() || (last && b == b'.')) {
            return None;
        }
        let v: f64 = p.parse().ok()?;
        // Minutes and seconds after the first part are below 60.
        if i > 0 && v >= 60.0 {
            return None;
        }
        secs = secs * 60.0 + v;
    }
    Some(secs)
}

/// Where `text` goes in a movie of `duration` seconds now at `position`; `None` when it
/// says nothing understood.
pub fn parse(text: &str, position: f64, duration: f64) -> Option<f64> {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let t = t.as_str();
    if t.is_empty() {
        return None;
    }
    let to = if let Some(p) = t.strip_suffix('%') {
        if !p.bytes().all(|b| b.is_ascii_digit() || b == b'.') || p.is_empty() {
            return None;
        }
        let f: f64 = p.parse().ok()?;
        if f > 100.0 {
            return None;
        }
        duration * f / 100.0
    } else if let Some(r) = t.strip_prefix('+') {
        position + clock(r)?
    } else if let Some(r) = t.strip_prefix('-') {
        position - clock(r)?
    } else {
        clock(t)?
    };
    to.is_finite().then(|| to.clamp(0.0, duration.max(0.0)))
}

/// Draws the field in `r` (over the time readout): the typed text left to right (as times
/// are), its selection and caret, or a hint while empty.
pub(crate) fn draw_field(cx: &mut Ctx, r: Rect, edit: &LineEdit, style: TextStyle) {
    cx.dl.rect_border(
        r,
        metrics::RADIUS_SM + 2.0,
        cx.pal.surface_raised,
        cx.pal.focus_ring.with_alpha(0.7),
        1.0,
    );
    cx.text_field = Some(r);
    let composing = !cx.preedit.is_empty();
    let inner = r.inset(4.0);
    if edit.is_empty() && !composing {
        let b = cx.layout(&t!("player-go-to-time-hint"), style, None);
        cx.block_in(&b, r, HAlign::Center, cx.pal.text_faint);
        let ch = b.height.min(r.h - 4.0).max(r.h - 16.0).round();
        let x = r.x + ((r.w - b.width) * 0.5).max(4.0) - 2.5;
        let caret = Rect::new(x, r.center().1 - ch * 0.5, 1.5, ch);
        cx.dl.rect(caret, 0.0, cx.pal.accent);
        cx.ime_area = Some(caret);
        cx.field_geom = Some(FieldGeom {
            rect: r,
            text: None,
            digits: None,
        });
        return;
    }
    let typed = edit.text();
    let at = edit.caret().min(typed.len());
    let text = format!("{}{}{}", &typed[..at], cx.preedit, &typed[at..]);
    let b = cx.layout(&text, style, None);
    let c = at + cx.preedit.len();
    let x0 = if b.width <= inner.w {
        r.x + (r.w - b.width) * 0.5
    } else {
        (inner.right() - b.width).max(inner.x - b.caret_x(c))
    };
    let ch = b.height.min(r.h - 4.0).max(r.h - 16.0).round();
    let top = r.center().1 - ch * 0.5;
    cx.dl.push_clip(r);
    if !composing && edit.has_selection() {
        for (a, z) in b.range_spans(edit.selection()) {
            cx.dl
                .rect(Rect::new(x0 + a, top, z - a, ch), 0.0, cx.pal.selection);
        }
    }
    let y = r.y + (r.h - b.height) * 0.5;
    cx.text.draw(cx.dl, &b, x0, y, cx.pal.text);
    cx.dl.pop_clip();
    let x = x0 + b.caret_x(c) + if c == text.len() { 1.0 } else { -0.75 };
    let caret = Rect::new(x, top, 1.5, ch);
    cx.dl.rect(caret, 0.0, cx.pal.accent);
    cx.ime_area = Some(caret);
    cx.field_geom = Some(FieldGeom {
        rect: r,
        text: Some((Arc::clone(&b), x0)),
        digits: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: f64 = 2.0 * 3600.0;

    #[test]
    fn times_from_the_start() {
        assert_eq!(parse("1:23:45", 0.0, D), Some(5025.0));
        assert_eq!(parse("83:20", 0.0, D), Some(5000.0));
        assert_eq!(parse("5025", 0.0, D), Some(5025.0));
        assert_eq!(parse("0:07", 0.0, D), Some(7.0));
        assert_eq!(parse("12.5", 0.0, D), Some(12.5));
        assert_eq!(parse(" 1:00 ", 0.0, D), Some(60.0));
    }

    #[test]
    fn parts_and_steps() {
        assert_eq!(parse("50%", 0.0, D), Some(3600.0));
        assert_eq!(parse("0%", 99.0, D), Some(0.0));
        assert_eq!(parse("100%", 0.0, D), Some(D));
        assert_eq!(parse("+30", 100.0, D), Some(130.0));
        assert_eq!(parse("-1:00", 100.0, D), Some(40.0));
    }

    #[test]
    fn kept_within_the_movie() {
        assert_eq!(parse("-1:00", 20.0, D), Some(0.0));
        assert_eq!(parse("+30", D - 10.0, D), Some(D));
        assert_eq!(parse("9:00:00", 0.0, D), Some(D));
    }

    #[test]
    fn nonsense_is_refused() {
        for t in [
            "", "abc", "1::2", ":30", "1:2:3:4", "1:75", "150%", "%", "+", "1.5:00", "--3",
            "1:00:60",
        ] {
            assert_eq!(parse(t, 10.0, D), None, "{t:?}");
        }
    }

    #[test]
    fn any_script_digits_and_signs() {
        let typed: String = "\u{2212}\u{0661}:\u{0660}\u{0660}"
            .chars()
            .filter_map(field_char)
            .collect();
        assert_eq!(typed, "-1:00");
        assert_eq!(parse(&typed, 100.0, D), Some(40.0));
        let pct: String = "\u{0665}\u{0660}\u{066A}"
            .chars()
            .filter_map(field_char)
            .collect();
        assert_eq!(parse(&pct, 0.0, D), Some(3600.0));
        assert_eq!(field_char('x'), None);
    }
}
