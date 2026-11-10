//! WebVTT (`.vtt`, Matroska `S_TEXT/WEBVTT`, MP4 `wvtt`): a `WEBVTT` header, then blocks of
//! an optional identifier, a time line with cue settings, and the cue text. `NOTE`, `STYLE`
//! and `REGION` blocks are skipped. Of the settings, `line` (top or bottom), `align` and
//! `position` place the cue; vertical text is shown horizontally.

use crate::{Body, Cue, Error, Result, TextCue, Track, markup};

/// A WebVTT timestamp: `mm:ss.ttt` or `hh:mm:ss.ttt`.
pub(crate) fn time(s: &str) -> Option<i64> {
    crate::srt::time(s)
}

/// Applies cue settings (`line:10% align:start position:20%`) to `cue`.
pub fn apply_settings(cue: &mut TextCue, settings: &str) {
    let mut h = 2u8; // 1 start, 2 centre, 3 end (the numpad column)
    let mut top = false;
    let mut x = None;
    for s in settings.split_whitespace() {
        let Some((k, v)) = s.split_once(':') else {
            continue;
        };
        let v = v.split(',').next().unwrap_or(v);
        match k {
            "line" => {
                if let Some(p) = v.strip_suffix('%') {
                    if let Ok(p) = p.parse::<f32>() {
                        top = p < 50.0;
                        if top && p > 0.0 {
                            cue.margin_v = Some(p / 100.0);
                        } else if !top {
                            cue.margin_v = Some(((100.0 - p) / 100.0).max(0.0));
                        }
                    }
                } else if let Ok(n) = v.parse::<i32>() {
                    // Line numbers: 0 and up count from the top, -1 and down from the bottom.
                    top = n >= 0;
                }
            }
            "align" => {
                h = match v {
                    "start" | "left" => 1,
                    "end" | "right" => 3,
                    _ => 2,
                }
            }
            "position" => {
                if let Some(p) = v.strip_suffix('%')
                    && let Ok(p) = p.parse::<f32>()
                {
                    x = Some((p / 100.0).clamp(0.0, 1.0));
                }
            }
            _ => {}
        }
    }
    cue.align = if top { 6 + h } else { h };
    if let Some(x) = x
        && x != 0.5
    {
        let y = if top { 0.05 } else { 0.95 };
        cue.pos = Some((x, cue.margin_v.map_or(y, |m| if top { m } else { 1.0 - m })));
    }
}

/// Cue text and settings to a cue.
pub fn cue_text(payload: &str, settings: &str) -> TextCue {
    let mut c = markup::parse(payload);
    apply_settings(&mut c, settings);
    c
}

pub fn parse(text: &str) -> Result<Track> {
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text.lines().peekable();
    match lines.next() {
        Some(l) if l.starts_with("WEBVTT") => {}
        _ => return Err(Error::Malformed("no WEBVTT header".into())),
    }
    let mut cues = Vec::new();
    let mut block: Vec<&str> = Vec::new();
    let mut flush = |block: &mut Vec<&str>| {
        let b = std::mem::take(block);
        if b.is_empty() {
            return;
        }
        let first = b[0].trim_start();
        if first.starts_with("NOTE") || first.starts_with("STYLE") || first.starts_with("REGION") {
            return;
        }
        let Some(ti) = b.iter().position(|l| l.contains("-->")) else {
            return;
        };
        let Some((a, rest)) = b[ti].split_once("-->") else {
            return;
        };
        let rest = rest.trim_start();
        let (b_time, settings) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let (Some(start), Some(end)) = (time(a), time(b_time)) else {
            return;
        };
        let c = cue_text(&b[ti + 1..].join("\n"), settings);
        if !c.is_empty() {
            cues.push(Cue {
                start_us: start,
                end_us: end,
                body: Body::Text(c),
            });
        }
    };
    for l in lines {
        if l.trim().is_empty() {
            flush(&mut block);
        } else {
            block.push(l);
        }
    }
    flush(&mut block);
    Ok(Track::new(cues))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_and_settings() {
        let t = parse(
            "WEBVTT - title\n\nNOTE a comment\nmore\n\nSTYLE\n::cue { color: red }\n\n\
             intro\n00:01.000 --> 00:02.000\n<v Ann>Hi</v> <c.yellow>there</c>\n\n\
             01:00:00.000 --> 01:00:01.500 line:0 align:start\nTop &lrm;left\n",
        )
        .unwrap();
        assert_eq!(t.cues.len(), 2);
        let a = t.cues[0].text().unwrap();
        assert_eq!(a.plain_text(), "Hi there");
        assert_eq!(a.lines[0][1].colour, Some([255, 255, 0, 255]));
        assert_eq!(a.align, 2);
        assert_eq!(t.cues[1].start_us, 3_600_000_000);
        let b = t.cues[1].text().unwrap();
        assert_eq!(b.align, 7);
        assert_eq!(b.plain_text(), "Top \u{200e}left");
    }

    #[test]
    fn needs_the_header() {
        assert!(parse("00:01.000 --> 00:02.000\nx").is_err());
    }
}
