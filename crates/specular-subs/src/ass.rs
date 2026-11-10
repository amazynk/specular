//! ASS and SSA (`.ass`, `.ssa`, Matroska `S_TEXT/ASS` and `S_TEXT/SSA`): the script's play
//! resolution and styles, and its dialogue lines with override blocks.
//!
//! Kept: bold, italic, underline, fill and outline colour, size, alignment (`\an`, SSA's
//! `\a`), position (`\pos`, and `\move` at its start), `\r` resets and `\N` breaks. Left out
//! quietly (the track says it was simplified): animation (`\t`), fades, karaoke, clipping,
//! rotation, blur, drawings (`\p`, whose shapes are not text and are dropped) and the
//! Banner/Scroll effects.

use std::collections::HashMap;

use crate::markup::{Lines, Style};
use crate::{Body, Cue, Error, Result, Rgba, TextCue, Track};

/// libass's default play resolution height, the size reference for fonts.
const DEFAULT_PLAY_RES_Y: f32 = 288.0;
/// The default style's font size at that height: the size our renderer calls 1.0.
const DEFAULT_FONT: f32 = 20.0;

/// An ASS colour: `&HAABBGGRR&` (alpha inverted: 00 opaque), also decimal in old SSA.
pub fn colour(s: &str) -> Option<Rgba> {
    let s = s.trim().trim_end_matches('&');
    let v = if let Some(h) = s
        .strip_prefix("&H")
        .or_else(|| s.strip_prefix("&h"))
        .or_else(|| s.strip_prefix('H'))
    {
        u32::from_str_radix(h.trim_end_matches('&'), 16).ok()?
    } else {
        let d: i64 = s.parse().ok()?;
        d as u32
    };
    let a = 255 - ((v >> 24) & 255) as u8;
    Some([v as u8, (v >> 8) as u8, (v >> 16) as u8, a])
}

#[derive(Clone, Debug)]
struct AssStyle {
    size: f32,
    primary: Option<Rgba>,
    outline: Option<Rgba>,
    bold: bool,
    italic: bool,
    underline: bool,
    align: u8,
    margin_v: f32,
}

impl Default for AssStyle {
    fn default() -> Self {
        AssStyle {
            size: DEFAULT_FONT,
            primary: None,
            outline: None,
            bold: false,
            italic: false,
            underline: false,
            align: 2,
            margin_v: 20.0,
        }
    }
}

/// SSA's legacy alignment (1–3 bottom, 5–7 top, 9–11 middle) as numpad alignment.
fn legacy_align(a: u8) -> u8 {
    match a {
        1..=3 => a,
        5..=7 => a + 2,
        9..=11 => a - 5,
        _ => 2,
    }
}

/// A script's header: play resolution and styles.
#[derive(Clone, Debug)]
pub struct Script {
    play_x: f32,
    play_y: f32,
    styles: HashMap<String, AssStyle>,
    /// The `[Events]` field order.
    format: Vec<String>,
    /// SSA v4 (legacy alignment numbers).
    ssa: bool,
}

impl Default for Script {
    fn default() -> Self {
        Script {
            play_x: 384.0,
            play_y: DEFAULT_PLAY_RES_Y,
            styles: HashMap::new(),
            format: [
                "layer", "start", "end", "style", "name", "marginl", "marginr", "marginv",
                "effect", "text",
            ]
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
            ssa: false,
        }
    }
}

fn bool_field(v: &str) -> bool {
    v.trim().parse::<i32>().is_ok_and(|n| n != 0)
}

impl Script {
    /// Reads the header sections (`[Script Info]`, `[V4+ Styles]`/`[V4 Styles]`, the
    /// `[Events]` format line). Dialogue lines are ignored here.
    pub fn parse_header(text: &str) -> Script {
        let mut s = Script::default();
        let mut section = String::new();
        let mut style_format: Vec<String> = Vec::new();
        let mut has_x = false;
        for line in text.lines() {
            let l = line.trim();
            if l.starts_with('[') && l.ends_with(']') {
                section = l.to_ascii_lowercase();
                if section == "[v4 styles]" {
                    s.ssa = true;
                }
                continue;
            }
            let Some((k, v)) = l.split_once(':') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            match section.as_str() {
                "[script info]" => match k.to_ascii_lowercase().as_str() {
                    "playresx" => {
                        if let Ok(x) = v.parse::<f32>() {
                            s.play_x = x.max(1.0);
                            has_x = true;
                        }
                    }
                    "playresy" => {
                        if let Ok(y) = v.parse::<f32>() {
                            s.play_y = y.max(1.0);
                        }
                    }
                    "scripttype" => s.ssa |= v.eq_ignore_ascii_case("v4.00"),
                    _ => {}
                },
                "[v4+ styles]" | "[v4 styles]" => {
                    if k.eq_ignore_ascii_case("format") {
                        style_format = v
                            .split(',')
                            .map(|f| f.trim().to_ascii_lowercase())
                            .collect();
                    } else if k.eq_ignore_ascii_case("style") {
                        let fields: Vec<&str> = v.splitn(style_format.len().max(1), ',').collect();
                        let get = |n: &str| {
                            style_format
                                .iter()
                                .position(|f| f == n)
                                .and_then(|i| fields.get(i))
                                .map(|f| f.trim())
                        };
                        let mut st = AssStyle::default();
                        if let Some(z) = get("fontsize").and_then(|z| z.parse::<f32>().ok()) {
                            st.size = z;
                        }
                        st.primary = get("primarycolour").and_then(colour);
                        st.outline = get("outlinecolour").and_then(colour);
                        st.bold = get("bold").is_some_and(bool_field);
                        st.italic = get("italic").is_some_and(bool_field);
                        st.underline = get("underline").is_some_and(bool_field);
                        if let Some(a) = get("alignment").and_then(|a| a.parse::<u8>().ok()) {
                            st.align = if s.ssa {
                                legacy_align(a)
                            } else {
                                a.clamp(1, 9)
                            };
                        }
                        if let Some(m) = get("marginv").and_then(|m| m.parse::<f32>().ok()) {
                            st.margin_v = m;
                        }
                        let name = get("name").unwrap_or("Default").to_owned();
                        s.styles.insert(name.trim_start_matches('*').to_owned(), st);
                    }
                }
                "[events]" if k.eq_ignore_ascii_case("format") => {
                    s.format = v
                        .split(',')
                        .map(|f| f.trim().to_ascii_lowercase())
                        .collect();
                }
                _ => {}
            }
        }
        if !has_x {
            // Only a height: the 4:3 width libass assumes.
            s.play_x = s.play_y * 4.0 / 3.0;
        }
        s
    }

    fn style(&self, name: &str) -> AssStyle {
        self.styles
            .get(name.trim().trim_start_matches('*'))
            .or_else(|| self.styles.get("Default"))
            .cloned()
            .unwrap_or_default()
    }

    /// A dialogue's fields (style, vertical margin, effect, text) to a cue; `simplified` is
    /// set when anything was left out.
    fn cue(
        &self,
        style: &str,
        margin_v: Option<f32>,
        effect: &str,
        text: &str,
        simplified: &mut bool,
    ) -> TextCue {
        let st = self.style(style);
        if !effect.trim().is_empty() {
            *simplified = true;
        }
        let base = Style {
            bold: st.bold,
            italic: st.italic,
            underline: st.underline,
            colour: st.primary,
            outline: st.outline,
            scale: Some(self.scale(st.size)),
        };
        let mut out = Lines::with_style(base.clone());
        let mut align = st.align;
        let mut pos = None;
        let mut drawing = false;
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '{' {
                let end = chars[i..].iter().position(|&c| c == '}').map(|e| i + e);
                if let Some(end) = end {
                    let block: String = chars[i + 1..end].iter().collect();
                    self.overrides(
                        &block,
                        &mut out,
                        &base,
                        &mut align,
                        &mut pos,
                        &mut drawing,
                        simplified,
                    );
                    i = end + 1;
                    continue;
                }
            }
            if c == '\\' {
                match chars.get(i + 1) {
                    Some('N') => {
                        out.newline();
                        i += 2;
                        continue;
                    }
                    Some('n') => {
                        // A soft break: a space unless wrapping style 2 (not tracked).
                        out.push_char(' ');
                        i += 2;
                        continue;
                    }
                    Some('h') => {
                        out.push_char('\u{a0}');
                        i += 2;
                        continue;
                    }
                    _ => {}
                }
            }
            if !drawing {
                out.push_char(c);
            }
            i += 1;
        }
        let margin_v = margin_v.filter(|m| *m > 0.0).unwrap_or(st.margin_v);
        TextCue {
            lines: out.finish(),
            align,
            pos,
            margin_v: Some((margin_v / self.play_y).clamp(0.0, 0.5)),
        }
    }

    /// A font size in script pixels as our scale (1.0 = the default size).
    fn scale(&self, size: f32) -> f32 {
        let rel = size / self.play_y;
        (rel / (DEFAULT_FONT / DEFAULT_PLAY_RES_Y)).clamp(0.3, 4.0)
    }

    #[allow(clippy::too_many_arguments)]
    fn overrides(
        &self,
        block: &str,
        out: &mut Lines,
        base: &Style,
        align: &mut u8,
        pos: &mut Option<(f32, f32)>,
        drawing: &mut bool,
        simplified: &mut bool,
    ) {
        if !block.contains('\\') {
            // A comment.
            return;
        }
        for tag in split_tags(block) {
            let t = tag.as_str();
            let num = |p: &str| t.strip_prefix(p).and_then(|v| v.trim().parse::<f32>().ok());
            if let Some(v) = t.strip_prefix("an") {
                if let Ok(a) = v.trim().parse::<u8>()
                    && (1..=9).contains(&a)
                {
                    *align = a;
                }
            } else if let Some(v) = t
                .strip_prefix("a")
                .filter(|v| v.chars().all(|c| c.is_ascii_digit()) && !v.is_empty())
            {
                if let Ok(a) = v.parse::<u8>() {
                    *align = legacy_align(a);
                }
            } else if t.starts_with("pos(") || t.starts_with("move(") {
                if t.starts_with("move(") {
                    *simplified = true;
                }
                let inner = t.split_once('(').map_or("", |x| x.1).trim_end_matches(')');
                let v: Vec<f32> = inner
                    .split(',')
                    .filter_map(|x| x.trim().parse().ok())
                    .collect();
                if v.len() >= 2 {
                    *pos = Some((
                        (v[0] / self.play_x).clamp(0.0, 1.0),
                        (v[1] / self.play_y).clamp(0.0, 1.0),
                    ));
                }
            } else if let Some(v) = t
                .strip_prefix('p')
                .filter(|v| v.chars().all(|c| c.is_ascii_digit()) && !v.is_empty())
            {
                *drawing = v != "0";
                *simplified = true;
            } else if t.starts_with("fs") && !t.starts_with("fsc") && !t.starts_with("fsp") {
                if let Some(z) = num("fs") {
                    out.style_mut().scale = Some(self.scale(z));
                }
            } else if let Some(v) = t.strip_prefix("1c").or_else(|| {
                t.strip_prefix('c')
                    .filter(|v| v.starts_with('&') || v.starts_with('H'))
            }) {
                if let Some(c) = colour(v) {
                    let s = out.style_mut();
                    let a = s.colour.map_or(255, |c| c[3]);
                    s.colour = Some([c[0], c[1], c[2], a]);
                }
            } else if let Some(v) = t.strip_prefix("3c") {
                if let Some(c) = colour(v) {
                    out.style_mut().outline = Some(c);
                }
            } else if t.starts_with('b')
                && !t.starts_with("be")
                && !t.starts_with("blur")
                && !t.starts_with("bord")
            {
                if let Some(n) = num("b") {
                    out.style_mut().bold = n != 0.0;
                }
            } else if t.starts_with('i') && !t.starts_with("iclip") {
                if let Some(n) = num("i") {
                    out.style_mut().italic = n != 0.0;
                }
            } else if t.starts_with('u') {
                if let Some(n) = num("u") {
                    out.style_mut().underline = n != 0.0;
                }
            } else if let Some(name) = t.strip_prefix('r') {
                *out.style_mut() = if name.trim().is_empty() {
                    base.clone()
                } else {
                    let st = self.style(name);
                    Style {
                        bold: st.bold,
                        italic: st.italic,
                        underline: st.underline,
                        colour: st.primary,
                        outline: st.outline,
                        scale: Some(self.scale(st.size)),
                    }
                };
            } else if [
                "t(", "k", "K", "fad", "clip", "iclip", "fr", "fa", "org", "blur", "be", "q",
                "fsc", "fsp", "shad", "bord", "xbord", "ybord", "xshad", "yshad", "fe", "fn", "s",
                "alpha", "1a", "2a", "3a", "4a", "2c", "4c", "pbo",
            ]
            .iter()
            .any(|p| t.starts_with(p))
            {
                // Animation, karaoke, clipping and the rest: left out. Font name, spacing and
                // the shadow/border sizes are the renderer's own.
                if t.starts_with("t(")
                    || t.starts_with('k')
                    || t.starts_with('K')
                    || t.starts_with("fad")
                    || t.starts_with("clip")
                    || t.starts_with("iclip")
                    || t.starts_with("fr")
                    || t.starts_with("org")
                {
                    *simplified = true;
                }
            }
        }
    }

    /// A Matroska ASS/SSA block: `ReadOrder, Layer, Style, Name, MarginL, MarginR, MarginV,
    /// Effect, Text` (SSA: `ReadOrder, Marked, Style, …`).
    pub fn mkv_block(&self, payload: &str, simplified: &mut bool) -> Option<TextCue> {
        let f: Vec<&str> = payload.splitn(9, ',').collect();
        if f.len() < 9 {
            return None;
        }
        let mv = f[6].trim().parse::<f32>().ok();
        let c = self.cue(f[2], mv, f[7], f[8], simplified);
        (!c.is_empty()).then_some(c)
    }

    /// A `Dialogue:` line's value to its times and cue.
    fn dialogue(&self, v: &str, simplified: &mut bool) -> Option<(i64, i64, TextCue)> {
        let n = self.format.len().max(1);
        let f: Vec<&str> = v.splitn(n, ',').collect();
        let get = |name: &str| {
            self.format
                .iter()
                .position(|x| x == name)
                .and_then(|i| f.get(i))
                .copied()
        };
        let start = crate::srt::time(get("start")?)?;
        let end = crate::srt::time(get("end")?)?;
        let mv = get("marginv").and_then(|m| m.trim().parse::<f32>().ok());
        let c = self.cue(
            get("style").unwrap_or("Default"),
            mv,
            get("effect").unwrap_or(""),
            get("text")?,
            simplified,
        );
        Some((start, end, c))
    }
}

/// The tags of an override block: `\b1\i1\pos(1,2)` → `b1`, `i1`, `pos(1,2)` (a `\` inside
/// parentheses, as in `\t(\fs20)`, stays in its tag).
fn split_tags(block: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in block.chars() {
        match c {
            '\\' if depth == 0 => {
                if !cur.trim().is_empty() {
                    out.push(std::mem::take(&mut cur).trim().to_owned());
                }
                cur.clear();
            }
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_owned());
    }
    out
}

/// Parses a whole `.ass` or `.ssa` file.
pub fn parse(text: &str) -> Result<Track> {
    let text = text.trim_start_matches('\u{feff}');
    if !text.trim_start().starts_with("[Script Info]") && !text.contains("[Events]") {
        return Err(Error::Malformed("no [Script Info] or [Events]".into()));
    }
    let script = Script::parse_header(text);
    let mut simplified = false;
    let mut cues = Vec::new();
    let mut in_events = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_events = l.eq_ignore_ascii_case("[events]");
            continue;
        }
        if !in_events {
            continue;
        }
        if let Some(v) = l.strip_prefix("Dialogue:")
            && let Some((start, end, c)) = script.dialogue(v.trim_start(), &mut simplified)
            && !c.is_empty()
        {
            cues.push(Cue {
                start_us: start,
                end_us: end,
                body: Body::Text(c),
            });
        }
    }
    let mut t = Track::new(cues);
    t.simplified = simplified;
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = "[Script Info]\nScriptType: v4.00+\nPlayResX: 1920\nPlayResY: 1080\n\n\
[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
Style: Default,Arial,75,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,54,1\n\
Style: Sign,Arial,60,&H0000FFFF,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,2,2,8,10,10,30,1\n\n\
[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
Dialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello, {\\i1}world{\\i0}\\Nsecond line\n\
Dialogue: 0,0:00:02.00,0:00:04.00,Sign,,0,0,0,,{\\pos(960,100)}Sign text\n\
Dialogue: 0,0:00:05.00,0:00:06.00,Default,,0,0,0,,{\\t(0,500,\\fs90)\\k20}Kara{\\c&H0000FF&}red\n\
Dialogue: 0,0:00:07.00,0:00:08.00,Default,,0,0,0,,{\\p1}m 0 0 l 100 0 100 100{\\p0}\n\
Comment: 0,0:00:09.00,0:00:10.00,Default,,0,0,0,,not shown\n";

    #[test]
    fn script_cues() {
        let t = parse(SCRIPT).unwrap();
        assert_eq!(t.cues.len(), 3, "the drawing-only line has no text");
        assert!(t.simplified);
        let a = t.cues[0].text().unwrap();
        assert_eq!(t.cues[0].start_us, 1_000_000);
        assert_eq!(t.cues[0].end_us, 3_500_000);
        assert_eq!(a.plain_text(), "Hello, world\nsecond line");
        assert!(a.lines[0][1].italic);
        assert!(!a.lines[1][0].italic);
        assert_eq!(a.align, 2);
        // 75 px at 1080 is about the default size.
        assert!(
            (a.lines[0][0].scale - 1.0).abs() < 0.05,
            "{}",
            a.lines[0][0].scale
        );
        assert!((a.margin_v.unwrap() - 0.05).abs() < 0.001);
        let b = t.cues[1].text().unwrap();
        assert_eq!(b.align, 8);
        assert_eq!(b.pos, Some((0.5, 100.0 / 1080.0)));
        assert!(b.lines[0][0].bold);
        assert_eq!(b.lines[0][0].colour, Some([255, 255, 0, 255]));
        let c = t.cues[2].text().unwrap();
        assert_eq!(c.plain_text(), "Karared");
        assert_eq!(c.lines[0][1].colour, Some([255, 0, 0, 255]));
    }

    #[test]
    fn matroska_blocks() {
        let s = Script::parse_header(SCRIPT);
        let mut simp = false;
        let c = s
            .mkv_block("3,0,Sign,,0,0,0,,Over {\\b0}here", &mut simp)
            .unwrap();
        assert_eq!(c.align, 8);
        assert_eq!(c.plain_text(), "Over here");
        assert!(c.lines[0][0].bold && !c.lines[0][1].bold);
        assert!(!simp);
        assert!(s.mkv_block("bad", &mut simp).is_none());
    }

    #[test]
    fn ssa_legacy_alignment() {
        let ssa = "[Script Info]\nScriptType: v4.00\n\n[V4 Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, TertiaryColour, BackColour, Bold, Italic, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, AlphaLevel, Encoding\nStyle: Default,Arial,20,16777215,65535,65535,0,0,0,1,2,0,6,30,30,10,0,0\n\n[Events]\nFormat: Marked, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: Marked=0,0:00:00.50,0:00:01.00,Default,,0000,0000,0000,,Top {\\a1}x\n";
        let t = parse(ssa).unwrap();
        let c = t.cues[0].text().unwrap();
        assert_eq!(c.align, 1);
        assert_eq!(c.lines[0][0].colour, Some([255, 255, 255, 255]));
        assert_eq!(t.cues[0].start_us, 500_000);
    }

    #[test]
    fn colours() {
        assert_eq!(colour("&H00FF8000&"), Some([0, 128, 255, 255]));
        assert_eq!(colour("&H80FFFFFF"), Some([255, 255, 255, 127]));
        assert_eq!(colour("16777215"), Some([255, 255, 255, 255]));
    }
}
