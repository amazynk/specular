//! SubRip (`.srt`): numbered blocks of a time line (`00:01:02,500 --> 00:01:04,000`) and
//! text lines, separated by blank lines. Read leniently: missing numbers, `.` for `,`,
//! one-digit hours, stray spaces, extra blank lines inside a cue (a line that is a number
//! followed by a time line starts the next cue).

use crate::{Body, Cue, Error, Result, Track, markup};

/// A SubRip timestamp (`00:01:02,500`, also `0:1:2.5`) in microseconds.
pub(crate) fn time(s: &str) -> Option<i64> {
    let s = s.trim();
    let (hms, frac) = match s.rfind([',', '.']) {
        Some(i) => (&s[..i], &s[i + 1..]),
        None => (s, ""),
    };
    let parts: Vec<&str> = hms.split(':').map(str::trim).collect();
    let n = |p: &str| p.parse::<i64>().ok().filter(|v| *v >= 0);
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (n(h)?, n(m)?, n(s)?),
        [m, s] => (0, n(m)?, n(s)?),
        _ => return None,
    };
    Some(crate::hms_us(h, m, sec, crate::frac_us(frac.trim())?))
}

/// A time line: `start --> end` (any position settings after the end are ignored).
pub(crate) fn time_line(l: &str) -> Option<(i64, i64)> {
    let (a, b) = l.split_once("-->")?;
    let b = b.split_whitespace().next()?;
    Some((time(a)?, time(b)?))
}

pub fn parse(text: &str) -> Result<Track> {
    let lines: Vec<&str> = text.lines().collect();
    let mut cues = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((start, end)) = time_line(lines[i]) else {
            i += 1;
            continue;
        };
        i += 1;
        let mut body: Vec<&str> = Vec::new();
        while i < lines.len() {
            let l = lines[i];
            // The next cue: a number line then a time line (also without the blank line).
            if l.trim().chars().all(|c| c.is_ascii_digit())
                && !l.trim().is_empty()
                && lines.get(i + 1).is_some_and(|n| time_line(n).is_some())
            {
                break;
            }
            if time_line(l).is_some() {
                break;
            }
            body.push(l);
            i += 1;
        }
        while body.last().is_some_and(|l| l.trim().is_empty()) {
            body.pop();
        }
        let cue = markup::parse(&body.join("\n"));
        if !cue.is_empty() {
            cues.push(Cue {
                start_us: start,
                end_us: end,
                body: Body::Text(cue),
            });
        }
    }
    if cues.is_empty() && !text.trim().is_empty() {
        return Err(Error::Malformed("no SubRip cues".into()));
    }
    Ok(Track::new(cues))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(time("00:01:02,500"), Some(62_500_000));
        assert_eq!(time("1:02:03.04"), Some(3_723_040_000));
        assert_eq!(time(" 00:00:01,5 "), Some(1_500_000));
        assert_eq!(time("x"), None);
    }

    #[test]
    fn cues() {
        let t = parse(
            "\u{feff}1\r\n00:00:01,000 --> 00:00:02,500\r\nHello\r\n<i>there</i>\r\n\r\n\
             2\n00:00:03,000 --> 00:00:04,000 X1:10 X2:20\nשלום עולם\n\n\n\
             3\n00:00:05,000 --> 00:00:06,000\nno blank before next\n\
             4\n00:00:07,000 --> 00:00:08,000\nlast",
        )
        .unwrap();
        assert_eq!(t.cues.len(), 4);
        assert_eq!(t.cues[0].start_us, 1_000_000);
        assert_eq!(t.cues[0].end_us, 2_500_000);
        let c0 = t.cues[0].text().unwrap();
        assert_eq!(c0.plain_text(), "Hello\nthere");
        assert!(c0.lines[1][0].italic);
        assert_eq!(t.cues[1].text().unwrap().plain_text(), "שלום עולם");
        assert_eq!(
            t.cues[2].text().unwrap().plain_text(),
            "no blank before next"
        );
        assert_eq!(t.cues[3].text().unwrap().plain_text(), "last");
    }

    #[test]
    fn garbage_is_malformed() {
        assert!(parse("hello\nworld").is_err());
        assert!(parse("").unwrap().cues.is_empty());
    }
}
