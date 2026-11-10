//! MicroDVD (`.sub`): one cue a line, timed in frames: `{start}{end}text`, `|` between
//! lines. A first cue `{1}{1}23.976` gives the frame rate (else 23.976 is assumed, the
//! commonest). Styling by control codes before the text, for the cue (`{Y:i}`) or one line
//! (`{y:b}`): italic, bold, underline, colour (`{c:$BBGGRR}`) and size (`{s:N}`); a `/` at
//! a line's start is italic too. Other codes (font, position) are dropped, their text kept.

use crate::{Body, Cue, Error, Result, Span, TextCue, Track};

/// The frame rate when the file does not give one.
pub const DEFAULT_FPS: f64 = 23.976;

/// Whether `text` reads as MicroDVD: its first non-blank lines are `{n}{n}` cues.
pub fn sniff(text: &str) -> bool {
    let mut seen = 0;
    let lines = text
        .lines()
        .map(|l| l.trim_start_matches('\u{feff}').trim());
    for l in lines.filter(|l| !l.is_empty()).take(4) {
        if frames(l).is_none() {
            return false;
        }
        seen += 1;
    }
    seen > 0
}

/// `{start}{end}rest` → (start, end or `None` for `{}`, rest).
fn frames(l: &str) -> Option<(u64, Option<u64>, &str)> {
    let l = l.strip_prefix('{')?;
    let (a, l) = l.split_once('}')?;
    let l = l.strip_prefix('{')?;
    let (b, rest) = l.split_once('}')?;
    let a: u64 = a.trim().parse().ok()?;
    let b = if b.trim().is_empty() {
        None
    } else {
        Some(b.trim().parse().ok()?)
    };
    Some((a, b, rest))
}

#[derive(Clone, Copy, Default)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    colour: Option<[u8; 4]>,
    scale: Option<f32>,
}

/// Applies the control codes at the start of `s` (cue-wide ones to `cue` as well), returns
/// the rest.
fn codes<'a>(mut s: &'a str, line: &mut Style, cue: &mut Style) -> &'a str {
    loop {
        let Some(rest) = s.strip_prefix('{') else {
            return s;
        };
        let Some((code, after)) = rest.split_once('}') else {
            return s;
        };
        let Some((k, v)) = code.split_once(':') else {
            return s;
        };
        let k = k.trim();
        if k.len() != 1 {
            return s;
        }
        let wide = k.chars().all(|c| c.is_ascii_uppercase());
        let apply = |st: &mut Style| match k.to_ascii_lowercase().as_str() {
            "y" => {
                for f in v.split(',').map(|f| f.trim().to_ascii_lowercase()) {
                    match f.as_str() {
                        "i" => st.italic = true,
                        "b" => st.bold = true,
                        "u" => st.underline = true,
                        _ => {}
                    }
                }
            }
            "c" => {
                let hex = v.trim().trim_start_matches(['$', '#']);
                if let Ok(n) = u32::from_str_radix(hex, 16)
                    && hex.len() == 6
                {
                    // $BBGGRR.
                    st.colour = Some([n as u8, (n >> 8) as u8, (n >> 16) as u8, 255]);
                }
            }
            "s" => {
                if let Ok(px) = v.trim().parse::<f32>()
                    && px > 0.0
                {
                    // Sizes are points against a nominal 24.
                    st.scale = Some((px / 24.0).clamp(0.25, 4.0));
                }
            }
            _ => {}
        };
        apply(line);
        if wide {
            apply(cue);
        }
        s = after;
    }
}

/// Parses a MicroDVD file. `fps` overrides the frame rate the file gives (or the default).
pub fn parse(text: &str, fps: Option<f64>) -> Result<Track> {
    let mut rate = fps.unwrap_or(DEFAULT_FPS);
    let mut raw: Vec<(u64, Option<u64>, &str)> = Vec::new();
    for l in text.lines() {
        let l = l.trim_start_matches('\u{feff}').trim();
        if let Some(f) = frames(l) {
            raw.push(f);
        }
    }
    if raw.is_empty() {
        return Err(Error::Malformed("no MicroDVD cues".into()));
    }
    // `{1}{1}25.000`: the frame rate, not a cue.
    if let Some(&(a, Some(b), rest)) = raw.first()
        && a <= 1
        && b <= 1
        && let Ok(r) = rest.trim().replace(',', ".").parse::<f64>()
    {
        if fps.is_none() && (1.0..=200.0).contains(&r) {
            rate = r;
        }
        raw.remove(0);
    }
    let us = |f: u64| (f as f64 * 1e6 / rate).round() as i64;
    let mut cues = Vec::with_capacity(raw.len());
    for (i, &(a, b, rest)) in raw.iter().enumerate() {
        let start = us(a);
        // `{}` for the end: until the next cue (or a few seconds).
        let end = match b {
            Some(b) => us(b),
            None => raw
                .get(i + 1)
                .map(|n| us(n.0))
                .unwrap_or(start + crate::OPEN_TEXT_US),
        };
        let mut cue_style = Style::default();
        let mut lines = Vec::new();
        for part in rest.split('|') {
            let mut st = cue_style;
            let t = codes(part, &mut st, &mut cue_style);
            let (t, slash) = match t.strip_prefix('/') {
                Some(t) => (t, true),
                None => (t, false),
            };
            let span = Span {
                text: t.trim().to_owned(),
                bold: st.bold,
                italic: st.italic || slash,
                underline: st.underline,
                colour: st.colour,
                outline: None,
                scale: st.scale.unwrap_or(1.0),
            };
            lines.push(vec![span]);
        }
        let cue = TextCue {
            lines,
            align: 2,
            pos: None,
            margin_v: None,
        };
        if !cue.is_empty() {
            cues.push(Cue {
                start_us: start,
                end_us: end,
                body: Body::Text(cue),
            });
        }
    }
    Ok(Track::new(cues))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_rate_and_styles() {
        let t = parse(
            "{1}{1}25\n{25}{50}Hello|{y:i}world\n{75}{100}{Y:b}{c:$0000FF}All bold|second\n\
             {125}{}/slashed\n{150}{175}{f:Arial}plain\n",
            None,
        )
        .unwrap();
        assert_eq!(t.cues.len(), 4);
        assert_eq!(t.cues[0].start_us, 1_000_000);
        assert_eq!(t.cues[0].end_us, 2_000_000);
        let c = t.cues[0].text().unwrap();
        assert_eq!(c.plain_text(), "Hello\nworld");
        assert!(!c.lines[0][0].italic && c.lines[1][0].italic);
        let c = t.cues[1].text().unwrap();
        assert!(c.lines[0][0].bold && c.lines[1][0].bold);
        assert_eq!(c.lines[0][0].colour, Some([255, 0, 0, 255]));
        // An open end runs to the next cue.
        assert_eq!(t.cues[2].end_us, 6_000_000);
        assert!(t.cues[2].text().unwrap().lines[0][0].italic);
        assert_eq!(t.cues[3].text().unwrap().plain_text(), "plain");
    }

    #[test]
    fn default_and_forced_rate() {
        let t = parse("{0}{24}a\n", None).unwrap();
        assert_eq!(t.cues[0].end_us, (24.0 * 1e6 / DEFAULT_FPS).round() as i64);
        let t = parse("{1}{1}25\n{0}{30}a\n", Some(30.0)).unwrap();
        assert_eq!(t.cues[0].end_us, 1_000_000);
    }

    #[test]
    fn sniffs() {
        assert!(sniff("\u{feff}{1}{1}23.976\r\n{10}{20}x"));
        assert!(!sniff("1\n00:00:01,000 --> 00:00:02,000\nx"));
        assert!(!sniff(""));
        assert!(parse("nothing", None).is_err());
    }
}
