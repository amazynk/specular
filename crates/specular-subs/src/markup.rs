//! The light markup of SubRip, Matroska text and WebVTT cue text: `<b>`, `<i>`, `<u>`,
//! `<font color=…>`, WebVTT's `<c>`, `<v>`, `<lang>` and `<ruby>` (kept as plain text), and
//! the ASS override blocks some SubRip files carry (`{\an8}`, `{\i1}`). Unknown tags are
//! dropped, their text kept.

use crate::{Rgba, Span, TextCue};

/// Style state while reading.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub colour: Option<Rgba>,
    pub outline: Option<Rgba>,
    pub scale: Option<f32>,
}

impl Style {
    pub(crate) fn span(&self, text: String) -> Span {
        Span {
            text,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            colour: self.colour,
            outline: self.outline,
            scale: self.scale.unwrap_or(1.0),
        }
    }
}

/// Builds lines of spans.
#[derive(Default)]
pub(crate) struct Lines {
    pub lines: Vec<Vec<Span>>,
    cur: String,
    style: Style,
}

impl Lines {
    pub(crate) fn with_style(style: Style) -> Lines {
        Lines {
            lines: vec![Vec::new()],
            cur: String::new(),
            style,
        }
    }

    pub(crate) fn push_char(&mut self, c: char) {
        self.cur.push(c);
    }

    fn flush(&mut self) {
        if !self.cur.is_empty() {
            let t = std::mem::take(&mut self.cur);
            let span = self.style.span(t);
            if self.lines.is_empty() {
                self.lines.push(Vec::new());
            }
            let line = self.lines.last_mut().expect("a line");
            // Runs of the same style merge.
            if let Some(last) = line.last_mut()
                && same_style(last, &span)
            {
                last.text.push_str(&span.text);
            } else {
                line.push(span);
            }
        }
    }

    pub(crate) fn style_mut(&mut self) -> &mut Style {
        self.flush();
        &mut self.style
    }

    pub(crate) fn style(&self) -> &Style {
        &self.style
    }

    pub(crate) fn newline(&mut self) {
        self.flush();
        self.lines.push(Vec::new());
    }

    pub(crate) fn finish(mut self) -> Vec<Vec<Span>> {
        self.flush();
        // Trailing and leading empty lines go; inner ones stay (a deliberate gap).
        while self.lines.last().is_some_and(|l| line_empty(l)) {
            self.lines.pop();
        }
        while self.lines.first().is_some_and(|l| line_empty(l)) {
            self.lines.remove(0);
        }
        self.lines
    }
}

fn same_style(a: &Span, b: &Span) -> bool {
    (a.bold, a.italic, a.underline, a.colour, a.outline)
        == (b.bold, b.italic, b.underline, b.colour, b.outline)
        && a.scale == b.scale
}

fn line_empty(l: &[Span]) -> bool {
    l.iter().all(|s| s.text.trim().is_empty())
}

/// A colour name or `#rgb`/`#rrggbb`/`rrggbb` (SubRip's `<font color>`, WebVTT classes).
pub(crate) fn colour(s: &str) -> Option<Rgba> {
    let s = s.trim().trim_matches(['"', '\'']);
    let named = match s.to_ascii_lowercase().as_str() {
        "white" => Some([255, 255, 255, 255]),
        "black" => Some([0, 0, 0, 255]),
        "red" => Some([255, 0, 0, 255]),
        "lime" => Some([0, 255, 0, 255]),
        "green" => Some([0, 128, 0, 255]),
        "blue" => Some([0, 0, 255, 255]),
        "yellow" => Some([255, 255, 0, 255]),
        "cyan" | "aqua" => Some([0, 255, 255, 255]),
        "magenta" | "fuchsia" => Some([255, 0, 255, 255]),
        "silver" => Some([192, 192, 192, 255]),
        "gray" | "grey" => Some([128, 128, 128, 255]),
        "orange" => Some([255, 165, 0, 255]),
        "purple" => Some([128, 0, 128, 255]),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let h = s.strip_prefix('#').unwrap_or(s);
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    match h.len() {
        3 => {
            let (r, g, b) = ((v >> 8) & 15, (v >> 4) & 15, v & 15);
            Some([(r * 17) as u8, (g * 17) as u8, (b * 17) as u8, 255])
        }
        6 => Some([(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]),
        _ => None,
    }
}

/// An HTML entity's character (`amp`, `lt`, `nbsp`, `lrm`, `#x5d0`, …).
pub(crate) fn entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "lrm" => '\u{200e}',
        "rlm" => '\u{200f}',
        _ => {
            let n = name.strip_prefix('#')?;
            let v = match n.strip_prefix(['x', 'X']) {
                Some(h) => u32::from_str_radix(h, 16).ok()?,
                None => n.parse().ok()?,
            };
            char::from_u32(v)?
        }
    })
}

/// What the ASS override blocks of a SubRip or Matroska cue set for the whole cue.
#[derive(Default)]
pub(crate) struct Placement {
    pub align: Option<u8>,
}

/// Parses SubRip-style cue text into a cue at the default bottom-centre place.
pub fn parse(text: &str) -> TextCue {
    let mut place = Placement::default();
    let lines = parse_into(text, Style::default(), &mut place, true);
    TextCue {
        lines,
        align: place.align.unwrap_or(2),
        pos: None,
        margin_v: None,
    }
}

/// Parses into lines; `entities`: `&amp;` and friends are decoded (WebVTT, and SubRip as
/// players treat it).
pub(crate) fn parse_into(
    text: &str,
    base: Style,
    place: &mut Placement,
    entities: bool,
) -> Vec<Vec<Span>> {
    let mut out = Lines::with_style(base.clone());
    let mut stack: Vec<(String, Style)> = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\r' => {}
            '\n' => out.newline(),
            '<' => {
                if let Some(end) = chars[i + 1..].iter().position(|&c| c == '>') {
                    let tag: String = chars[i + 1..i + 1 + end].iter().collect();
                    apply_tag(&tag, &mut out, &mut stack);
                    i += end + 2;
                    continue;
                }
                out.push_char(c);
            }
            '{' if chars.get(i + 1) == Some(&'\\') => {
                if let Some(end) = chars[i + 1..].iter().position(|&c| c == '}') {
                    let block: String = chars[i + 1..i + 1 + end].iter().collect();
                    srt_override(&block, &mut out, place);
                    i += end + 2;
                    continue;
                }
                out.push_char(c);
            }
            '&' if entities => {
                let rest: String = chars[i + 1..chars.len().min(i + 12)].iter().collect();
                if let Some(semi) = rest.find(';')
                    && let Some(ch) = entity(&rest[..semi])
                {
                    out.push_char(ch);
                    i += 1 + rest[..semi].chars().count() + 1;
                    continue;
                }
                out.push_char(c);
            }
            '\\' if matches!(chars.get(i + 1), Some('N') | Some('n')) => {
                // ASS-style line breaks in SubRip files.
                out.newline();
                i += 2;
                continue;
            }
            _ => out.push_char(c),
        }
        i += 1;
    }
    out.finish()
}

fn apply_tag(tag: &str, out: &mut Lines, stack: &mut Vec<(String, Style)>) {
    let tag = tag.trim();
    if let Some(close) = tag.strip_prefix('/') {
        let name = close
            .trim()
            .split(['.', ' '])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if let Some(at) = stack.iter().rposition(|(n, _)| *n == name) {
            let (_, saved) = stack[at].clone();
            stack.truncate(at);
            *out.style_mut() = saved;
        }
        return;
    }
    // WebVTT timestamps inside a cue (`<00:00:01.500>`) and anything unnamed.
    let name_end = tag
        .find(|c: char| c == '.' || c.is_whitespace())
        .unwrap_or(tag.len());
    let name = tag[..name_end].to_ascii_lowercase();
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        return;
    }
    let before = out.style().clone();
    let mut s = before.clone();
    match name.as_str() {
        "b" => s.bold = true,
        "i" => s.italic = true,
        "u" => s.underline = true,
        "font" => {
            let lower = tag.to_ascii_lowercase();
            if let Some(at) = lower.find("color") {
                let v = tag[at + 5..]
                    .trim_start()
                    .trim_start_matches('=')
                    .trim_start();
                let v: String = v
                    .chars()
                    .take_while(|c| !c.is_whitespace() || *c == '"')
                    .collect();
                let v = v.trim_matches(['"', '\'']);
                let v = v.split(['"', '\'']).next().unwrap_or(v);
                if let Some(c) = colour(v) {
                    s.colour = Some(c);
                }
            }
        }
        "c" => {
            // WebVTT classes: the default colour classes.
            for class in tag[name_end..].split('.').skip(1) {
                if let Some(c) = colour(class.split_whitespace().next().unwrap_or("")) {
                    s.colour = Some(c);
                }
            }
        }
        // Voice, language, ruby: text kept as is.
        _ => {}
    }
    stack.push((name, before));
    if s != *out.style() {
        *out.style_mut() = s;
    }
}

/// An ASS override block in a SubRip cue: alignment, bold, italic, underline, colour.
fn srt_override(block: &str, out: &mut Lines, place: &mut Placement) {
    for t in block.split('\\').filter(|t| !t.is_empty()) {
        if let Some(v) = t.strip_prefix("an") {
            if let Ok(a) = v.trim().parse::<u8>()
                && (1..=9).contains(&a)
            {
                place.align = Some(a);
            }
        } else if let Some(v) = t.strip_prefix('b') {
            if let Ok(n) = v.trim().parse::<u32>() {
                out.style_mut().bold = n != 0;
            }
        } else if let Some(v) = t.strip_prefix('i') {
            if let Ok(n) = v.trim().parse::<u32>() {
                out.style_mut().italic = n != 0;
            }
        } else if let Some(v) = t.strip_prefix('u') {
            if let Ok(n) = v.trim().parse::<u32>() {
                out.style_mut().underline = n != 0;
            }
        } else if let Some(v) = t.strip_prefix("1c").or_else(|| t.strip_prefix('c'))
            && let Some(c) = crate::ass::colour(v)
        {
            out.style_mut().colour = Some(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_and_lines() {
        let c = parse("<i>Hello</i> <b>world</b>\n<font color=\"#ff0000\">red</font> &amp; done");
        assert_eq!(c.lines.len(), 2);
        assert_eq!(c.lines[0][0].text, "Hello");
        assert!(c.lines[0][0].italic);
        assert_eq!(c.lines[0][1].text, " ");
        assert!(c.lines[0][2].bold);
        assert_eq!(c.lines[1][0].colour, Some([255, 0, 0, 255]));
        assert_eq!(c.lines[1][1].text, " & done");
        assert_eq!(c.align, 2);
    }

    #[test]
    fn override_blocks_in_subrip() {
        let c = parse("{\\an8}{\\i1}Top{\\i0} line");
        assert_eq!(c.align, 8);
        assert!(c.lines[0][0].italic);
        assert!(!c.lines[0][1].italic);
        assert_eq!(c.plain_text(), "Top line");
    }

    #[test]
    fn unknown_tags_keep_text() {
        let c = parse("<v Bob>Hi <lang he>שלום</lang></v>");
        assert_eq!(c.plain_text(), "Hi שלום");
        let c = parse("a < b");
        assert_eq!(c.plain_text(), "a < b");
    }

    #[test]
    fn colours() {
        assert_eq!(colour("#0f0"), Some([0, 255, 0, 255]));
        assert_eq!(colour("FFFF00"), Some([255, 255, 0, 255]));
        assert_eq!(colour("yellow"), Some([255, 255, 0, 255]));
        assert_eq!(colour("nope"), None);
    }
}
