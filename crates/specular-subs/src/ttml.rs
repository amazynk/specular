//! TTML and its older name DFXP (`.ttml`, `.dfxp`, also `.xml`): XML whose `<p>` elements
//! are the cues, timed by `begin`, `end` or `dur` (clock times `01:02:03.5`, frames
//! `01:02:03:12`, or offsets `12.5s`, `300ms`, `90f`, `1000t`), relative to their parents'
//! `begin`. `<br/>` breaks lines; `<span>` and referenced `<style>`s give italic, bold,
//! underline and colour. Regions, animation and the rest of the styling are left out
//! quietly: the words still show, plainly.
//!
//! Attributes are matched by their local names, so the 2006 DFXP namespaces read as well as
//! TTML 1 and 2 (IMSC, EBU-TT-D, SMPTE-TT).

use std::collections::HashMap;

use crate::{Body, Cue, Error, Result, Rgba, Span, TextCue, Track};

/// Deepest element nesting read (roxmltree recurses).
const MAX_DEPTH: usize = 256;
/// Most XML nodes read.
const MAX_NODES: u32 = 4_000_000;

/// Whether `text` reads as TTML: a `tt` root in the first few KiB.
pub fn sniff(text: &str) -> bool {
    let head = &text[..floor(text, 4096)];
    let head = head.trim_start_matches('\u{feff}');
    head.contains("<tt ")
        || head.contains("<tt>")
        || head.contains("<tt:tt")
        || head.contains("ns/ttml\"")
        || head.contains("ttaf1")
}

fn floor(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// The deepest element nesting of `text`, roughly (a `>` inside an attribute may end a tag
/// early; the count only guards the parser's recursion).
fn depth(text: &str) -> usize {
    let b = text.as_bytes();
    let (mut d, mut max, mut i) = (0usize, 0usize, 0usize);
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let end = b[i..]
            .iter()
            .position(|&c| c == b'>')
            .map_or(b.len(), |p| i + p);
        match b.get(i + 1) {
            Some(b'/') => d = d.saturating_sub(1),
            Some(b'?' | b'!') => {}
            _ if b.get(end.wrapping_sub(1)) == Some(&b'/') => {}
            _ => {
                d += 1;
                max = max.max(d);
            }
        }
        i = end + 1;
    }
    max
}

/// Timing parameters of the document.
#[derive(Clone, Copy)]
struct Timing {
    frame_rate: f64,
    tick_rate: f64,
}

fn attr<'a>(n: roxmltree::Node<'a, '_>, local: &str) -> Option<&'a str> {
    n.attributes()
        .find(|a| a.name() == local)
        .map(|a| a.value())
}

/// A time expression in microseconds.
fn time(s: &str, t: Timing) -> Option<i64> {
    let s = s.trim();
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        let n = |p: &str| p.trim().parse::<f64>().ok().filter(|v| *v >= 0.0);
        let secs = match parts.as_slice() {
            [h, m, sec] => n(h)? * 3600.0 + n(m)? * 60.0 + n(sec)?,
            [h, m, sec, f] => {
                n(h)? * 3600.0 + n(m)? * 60.0 + n(sec)? + n(f)? / t.frame_rate.max(1.0)
            }
            _ => return None,
        };
        return Some((secs * 1e6).round() as i64);
    }
    let split = s.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, metric) = s.split_at(split);
    let v: f64 = num.trim().parse().ok().filter(|v: &f64| *v >= 0.0)?;
    let secs = match metric {
        "h" => v * 3600.0,
        "m" => v * 60.0,
        "s" => v,
        "ms" => v / 1000.0,
        "f" => v / t.frame_rate.max(1.0),
        "t" => v / t.tick_rate.max(1.0),
        _ => return None,
    };
    Some((secs * 1e6).round() as i64)
}

#[derive(Clone, Copy, Default)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    colour: Option<Rgba>,
}

/// A TTML colour: `#rrggbb`, `#rrggbbaa`, `rgb(…)`, `rgba(…)` or a few names.
fn colour(s: &str) -> Option<Rgba> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        let v = u32::from_str_radix(h, 16).ok()?;
        return match h.len() {
            6 => Some([(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]),
            8 => Some([(v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8]),
            _ => None,
        };
    }
    if let Some(inner) = s
        .strip_prefix("rgba(")
        .or_else(|| s.strip_prefix("rgb("))
        .and_then(|r| r.strip_suffix(')'))
    {
        let v: Vec<u8> = inner
            .split(',')
            .filter_map(|p| p.trim().parse::<u8>().ok())
            .collect();
        return match v.as_slice() {
            [r, g, b] => Some([*r, *g, *b, 255]),
            [r, g, b, a] => Some([*r, *g, *b, *a]),
            _ => None,
        };
    }
    Some(match s.to_ascii_lowercase().as_str() {
        "white" => [255, 255, 255, 255],
        "black" => [0, 0, 0, 255],
        "red" => [255, 0, 0, 255],
        "lime" => [0, 255, 0, 255],
        "green" => [0, 128, 0, 255],
        "blue" => [0, 0, 255, 255],
        "yellow" => [255, 255, 0, 255],
        "cyan" | "aqua" => [0, 255, 255, 255],
        "magenta" | "fuchsia" => [255, 0, 255, 255],
        _ => return None,
    })
}

/// The style attributes on `n` over `base`.
fn own_style(n: roxmltree::Node, base: Style) -> Style {
    let mut s = base;
    for a in n.attributes() {
        match a.name() {
            "fontStyle" => s.italic = a.value().contains("italic") || a.value() == "oblique",
            "fontWeight" => s.bold = a.value() == "bold",
            "textDecoration" => s.underline = a.value().contains("underline"),
            "color" => {
                if let Some(c) = colour(a.value()) {
                    s.colour = Some(c);
                }
            }
            _ => {}
        }
    }
    s
}

struct Reader<'s> {
    timing: Timing,
    styles: HashMap<&'s str, roxmltree::Node<'s, 's>>,
    cues: Vec<Cue>,
}

impl<'s> Reader<'s> {
    /// `n`'s style: its referenced styles, then its own attributes.
    fn style_of(&self, n: roxmltree::Node, base: Style, depth: usize) -> Style {
        let mut s = base;
        if let Some(refs) = attr(n, "style") {
            for r in refs.split_whitespace() {
                if let Some(st) = self.styles.get(r)
                    && depth < 8
                {
                    s = self.style_of(*st, s, depth + 1);
                }
            }
        }
        own_style(n, s)
    }

    /// The (begin, end) of `n` from its attributes, given its parent's interval.
    fn interval(&self, n: roxmltree::Node, parent: (i64, Option<i64>)) -> (i64, Option<i64>) {
        let begin = attr(n, "begin")
            .and_then(|v| time(v, self.timing))
            .map_or(parent.0, |b| parent.0 + b);
        let end = attr(n, "end")
            .and_then(|v| time(v, self.timing))
            .map(|e| parent.0 + e)
            .or_else(|| {
                attr(n, "dur")
                    .and_then(|v| time(v, self.timing))
                    .map(|d| begin + d)
            })
            .or(parent.1);
        let end = match (end, parent.1) {
            (Some(e), Some(p)) => Some(e.min(p)),
            (e, _) => e,
        };
        (begin, end)
    }

    fn walk(&mut self, n: roxmltree::Node<'s, 's>, span: (i64, Option<i64>), style: Style) {
        for c in n.children().filter(|c| c.is_element()) {
            let name = c.tag_name().name();
            if matches!(name, "head" | "metadata" | "styling" | "layout") {
                continue;
            }
            let iv = self.interval(c, span);
            let st = self.style_of(c, style, 0);
            if name == "p" {
                self.paragraph(c, iv, st);
            } else {
                self.walk(c, iv, st);
            }
        }
    }

    /// A `<p>`: its spans and breaks as one cue (spans timed apart are left in it).
    fn paragraph(&mut self, p: roxmltree::Node, (begin, end): (i64, Option<i64>), style: Style) {
        let Some(end) = end else { return };
        let mut lines: Vec<Vec<Span>> = vec![Vec::new()];
        self.inline(p, style, &mut lines, 0);
        // Whitespace between elements is layout, not text: trim each line.
        for l in &mut lines {
            if let Some(f) = l.first_mut() {
                f.text = f.text.trim_start().to_owned();
            }
            if let Some(f) = l.last_mut() {
                f.text = f.text.trim_end().to_owned();
            }
            l.retain(|s| !s.text.is_empty());
        }
        let cue = TextCue {
            lines,
            align: 2,
            pos: None,
            margin_v: None,
        };
        if !cue.is_empty() {
            self.cues.push(Cue {
                start_us: begin,
                end_us: end,
                body: Body::Text(cue),
            });
        }
    }

    fn inline(&self, n: roxmltree::Node, style: Style, lines: &mut Vec<Vec<Span>>, depth: usize) {
        for c in n.children() {
            if let Some(t) = c.text().filter(|_| c.is_text()) {
                // Runs of whitespace (newlines in the source) are one space.
                let mut s = String::with_capacity(t.len());
                let mut space = false;
                for ch in t.chars() {
                    if ch.is_whitespace() {
                        if !space {
                            s.push(' ');
                        }
                        space = true;
                    } else {
                        s.push(ch);
                        space = false;
                    }
                }
                let span = Span {
                    text: s,
                    bold: style.bold,
                    italic: style.italic,
                    underline: style.underline,
                    colour: style.colour,
                    outline: None,
                    scale: 1.0,
                };
                if let Some(l) = lines.last_mut() {
                    l.push(span);
                }
            } else if c.is_element() {
                match c.tag_name().name() {
                    "br" => lines.push(Vec::new()),
                    "metadata" => {}
                    _ if depth < 32 => {
                        let st = self.style_of(c, style, 0);
                        self.inline(c, st, lines, depth + 1);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Parses a TTML or DFXP document.
pub fn parse(text: &str) -> Result<Track> {
    let text = text.trim_start_matches('\u{feff}');
    if depth(text) > MAX_DEPTH {
        return Err(Error::Malformed("TTML nested too deeply".into()));
    }
    let opts = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: MAX_NODES,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, opts)
        .map_err(|e| Error::Malformed(format!("TTML: {e}")))?;
    let root = doc.root_element();
    if root.tag_name().name() != "tt" {
        return Err(Error::Malformed("not a TTML document".into()));
    }
    let num = |k: &str| attr(root, k).and_then(|v| v.trim().parse::<f64>().ok());
    let mut frame_rate = num("frameRate").filter(|r| *r > 0.0).unwrap_or(30.0);
    if let Some(m) = attr(root, "frameRateMultiplier") {
        let v: Vec<f64> = m
            .split_whitespace()
            .filter_map(|p| p.parse().ok())
            .collect();
        if let [a, b] = v.as_slice()
            && *a > 0.0
            && *b > 0.0
        {
            frame_rate *= a / b;
        }
    }
    let sub = num("subFrameRate").filter(|r| *r > 0.0).unwrap_or(1.0);
    let tick_rate =
        num("tickRate")
            .filter(|r| *r > 0.0)
            .unwrap_or(if attr(root, "frameRate").is_some() {
                frame_rate * sub
            } else {
                1.0
            });
    let mut styles = HashMap::new();
    for s in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "style")
    {
        if let Some(id) = attr(s, "id") {
            styles.insert(id, s);
        }
    }
    let mut r = Reader {
        timing: Timing {
            frame_rate,
            tick_rate,
        },
        styles,
        cues: Vec::new(),
    };
    r.walk(root, (0, None), Style::default());
    Ok(Track::new(r.cues))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling"
    xmlns:ttp="http://www.w3.org/ns/ttml#parameter" ttp:frameRate="25" ttp:tickRate="10000000">
  <head>
    <styling><style xml:id="it" tts:fontStyle="italic"/><style xml:id="y" tts:color="#ffff00"/></styling>
  </head>
  <body>
    <div begin="10s">
      <p begin="00:00:01.000" end="00:00:02.500">Hello<br/>
        <span style="it">there</span></p>
      <p begin="00:00:03:05" dur="25f" style="y">Frames</p>
      <p begin="40000000t" end="50000000t"><span tts:fontWeight="bold">Ticks</span> too</p>
    </div>
  </body>
</tt>"##;

    #[test]
    fn cues_times_and_styles() {
        let t = parse(DOC).unwrap();
        assert_eq!(t.cues.len(), 3);
        assert_eq!(t.cues[0].start_us, 11_000_000);
        assert_eq!(t.cues[0].end_us, 12_500_000);
        let c = t.cues[0].text().unwrap();
        assert_eq!(c.plain_text(), "Hello\nthere");
        assert!(c.lines[1][0].italic && !c.lines[0][0].italic);
        // 3 s and 5 frames at 25 fps, for 25 frames.
        assert_eq!(t.cues[1].start_us, 13_200_000);
        assert_eq!(t.cues[1].end_us, 14_200_000);
        assert_eq!(
            t.cues[1].text().unwrap().lines[0][0].colour,
            Some([255, 255, 0, 255])
        );
        assert_eq!(t.cues[2].start_us, 14_000_000);
        let c = t.cues[2].text().unwrap();
        assert_eq!(c.plain_text(), "Ticks too");
        assert!(c.lines[0][0].bold && !c.lines[0][1].bold);
    }

    #[test]
    fn dfxp_namespace_and_offsets() {
        let d = r#"<tt xmlns="http://www.w3.org/2006/10/ttaf1"><body><div>
            <p begin="1.5s" end="2500ms">Old</p><p begin="0.05m" dur="1s">Minutes</p></div></body></tt>"#;
        assert!(sniff(d));
        let t = parse(d).unwrap();
        assert_eq!(t.cues[0].start_us, 1_500_000);
        assert_eq!(t.cues[0].end_us, 2_500_000);
        assert_eq!(t.cues[1].start_us, 3_000_000);
        assert!(parse("<html/>").is_err());
        assert!(parse(&"<a>".repeat(400)).is_err());
        assert!(!sniff("<html><body/></html>"));
    }
}
