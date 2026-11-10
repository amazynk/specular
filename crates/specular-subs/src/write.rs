//! Text cues written as SubRip (`.srt`) or WebVTT (`.vtt`): plain lines (styling is not
//! kept), times to the millisecond; or as synced lyrics (`.lrc`), a line each, to the
//! hundredth. Picture cues are left out.

use crate::Track;

/// `h:mm:ss` and milliseconds, with the decimal mark `sep`.
fn time(us: i64, sep: char) -> String {
    let ms = us.max(0) / 1000;
    let (h, m, s, f) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    format!("{h:02}:{m:02}:{s:02}{sep}{f:03}")
}

/// Each text cue's lines, without blank lines (a blank line ends a cue in both formats).
fn cues(track: &Track) -> impl Iterator<Item = (i64, i64, Vec<String>)> + '_ {
    track.cues.iter().filter_map(|c| {
        let t = c.text()?;
        let lines: Vec<String> = t
            .plain_text()
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect();
        (!lines.is_empty()).then_some((c.start_us, c.end_us, lines))
    })
}

/// SubRip text.
pub fn srt(track: &Track) -> String {
    let mut out = String::new();
    for (n, (a, b, lines)) in cues(track).enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n",
            n + 1,
            time(a, ','),
            time(b, ',')
        ));
        for l in lines {
            out.push_str(&l);
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

/// WebVTT text (`<`, `>` and `&` escaped).
pub fn vtt(track: &Track) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for (a, b, lines) in cues(track) {
        out.push_str(&format!("{} --> {}\n", time(a, '.'), time(b, '.')));
        for l in lines {
            let esc = l
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            out.push_str(&esc);
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

/// LRC synced lyrics: a cue's lines joined into one, stamped `[mm:ss.xx]`; an empty stamped
/// line where a cue ends before the next starts (or at the last one's end), so the line
/// sung clears in the pause.
pub fn lrc(track: &Track) -> String {
    let stamp = |us: i64| {
        let cs = (us.max(0) + 5_000) / 10_000;
        format!("[{:02}:{:02}.{:02}]", cs / 6000, cs / 100 % 60, cs % 100)
    };
    let list: Vec<_> = cues(track).collect();
    let mut out = String::new();
    for (i, (a, b, lines)) in list.iter().enumerate() {
        out.push_str(&stamp(*a));
        out.push_str(&lines.join(" "));
        out.push('\n');
        let next = list.get(i + 1).map(|c| c.0);
        if next.is_none_or(|n| n > *b) {
            out.push_str(&stamp(*b));
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Body, Cue, Span, TextCue};

    fn cue(a: i64, b: i64, text: &str) -> Cue {
        Cue {
            start_us: a,
            end_us: b,
            body: Body::Text(TextCue {
                lines: text.lines().map(|l| vec![Span::plain(l)]).collect(),
                align: 2,
                pos: None,
                margin_v: None,
            }),
        }
    }

    #[test]
    fn round_trips_through_the_readers() {
        let t = Track::new(vec![
            cue(1_000_000, 2_500_000, "Hello there.\nSecond line"),
            cue(3_723_004_000, 3_725_000_000, "שלום <b> & more"),
        ]);
        let s = srt(&t);
        assert!(s.starts_with(
            "1\n00:00:01,000 --> 00:00:02,500\nHello there.\nSecond line\n\n2\n01:02:03,004"
        ));
        let back = crate::srt::parse(&s).unwrap();
        assert_eq!(back.cues.len(), 2);
        assert_eq!(back.cues[0].start_us, 1_000_000);
        assert_eq!(back.cues[1].end_us, 3_725_000_000);
        assert_eq!(
            back.cues[0].text().unwrap().plain_text(),
            "Hello there.\nSecond line"
        );
        let v = vtt(&t);
        assert!(v.starts_with("WEBVTT\n\n00:00:01.000 --> 00:00:02.500\n"));
        assert!(v.contains("שלום &lt;b&gt; &amp; more"));
        let back = crate::vtt::parse(&v).unwrap();
        assert_eq!(back.cues.len(), 2);
        assert_eq!(back.cues[1].text().unwrap().plain_text(), "שלום <b> & more");
        assert_eq!(srt(&Track::default()), "");
    }

    #[test]
    fn writes_lrc() {
        let t = Track::new(vec![
            cue(1_000_000, 2_500_000, "Hello there.\nSecond line"),
            cue(2_500_000, 4_000_000, "Right after"),
            cue(3_723_004_000, 3_725_000_000, "Much later"),
        ]);
        assert_eq!(
            lrc(&t),
            "[00:01.00]Hello there. Second line\n[00:02.50]Right after\n[00:04.00]\n\
             [62:03.00]Much later\n[62:05.00]\n"
        );
        assert_eq!(lrc(&Track::default()), "");
    }
}
