//! Whisper's segments into subtitle cues people can read: at most two lines of 42
//! characters and 7 s each, at least 0.8 s on screen (when the next cue leaves room), never
//! overlapping. A long segment is split at sentence ends, then commas, then the space
//! nearest its middle (anywhere for scripts without spaces), its time shared by length.
//!
//! Cue edges are aligned to the speech found by [`super::vad`]: a cue that starts in
//! silence starts when the speech does (within 1 s), one that ends in silence ends when the
//! speech did.

use super::decode::Segment;

/// Characters per line, and lines per cue.
pub const LINE: usize = 42;
pub const LINES: usize = 2;
/// Seconds on screen at most and at least.
pub const MAX_SECS: f64 = 7.0;
pub const MIN_SECS: f64 = 0.8;

/// A subtitle: seconds on the movie's timeline, lines joined by `\n`.
#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

fn chars(s: &str) -> usize {
    s.chars().count()
}

/// Where to split `s` (a byte index of a char boundary): the best break by kind, nearest the
/// middle. `None` when it has no break.
fn split_at(s: &str) -> Option<usize> {
    let n = chars(s);
    if n < 2 {
        return None;
    }
    let mid = n / 2;
    let idx: Vec<(usize, char)> = s.char_indices().collect();
    let best = |ok: &dyn Fn(usize) -> bool| -> Option<usize> {
        (1..idx.len())
            .filter(|&i| ok(i))
            .min_by_key(|&i| i.abs_diff(mid))
    };
    let after = |i: usize, set: &[char]| set.contains(&idx[i - 1].1) && idx[i].1 == ' ';
    let sentence = best(&|i| after(i, &['.', '?', '!', '…']) || "。？！".contains(idx[i - 1].1));
    let clause = best(&|i| after(i, &[',', ';', ':']) || "，、；".contains(idx[i - 1].1));
    let space = best(&|i| idx[i].1 == ' ');
    // A sentence end in the middle half wins; else a clause; else a space; else anywhere.
    let near = |i: &usize| i.abs_diff(mid) * 4 <= n;
    let i = sentence
        .filter(near)
        .or(clause.filter(near))
        .or(space)
        .unwrap_or(mid);
    Some(idx[i].0)
}

/// `text` into pieces of at most `LINE · LINES` characters, recursively halved.
fn pieces(text: &str, out: &mut Vec<String>) {
    let t = text.trim();
    if t.is_empty() {
        return;
    }
    if chars(t) <= LINE * LINES {
        out.push(t.to_owned());
        return;
    }
    match split_at(t) {
        Some(i) => {
            pieces(&t[..i], out);
            pieces(&t[i..], out);
        }
        None => out.push(t.to_owned()),
    }
}

/// One piece as up to two lines, broken at the space nearest the middle (or the middle).
pub fn wrap(text: &str) -> String {
    let t = text.trim();
    if chars(t) <= LINE {
        return t.to_owned();
    }
    let idx: Vec<(usize, char)> = t.char_indices().collect();
    let mid = idx.len() / 2;
    let at = (1..idx.len())
        .filter(|&i| idx[i].1 == ' ')
        .min_by_key(|&i| i.abs_diff(mid));
    match at {
        Some(i) => format!(
            "{}\n{}",
            t[..idx[i].0].trim_end(),
            t[idx[i].0..].trim_start()
        ),
        None => format!("{}\n{}", &t[..idx[mid].0], &t[idx[mid].0..]),
    }
}

/// Moves `t` into the nearest speech within `reach` seconds when it falls in silence:
/// forward to a start (`to_start`) or back to an end.
fn snap(t: f64, speech: &[(f64, f64)], to_start: bool, reach: f64) -> f64 {
    if speech.is_empty() || speech.iter().any(|&(a, b)| t >= a && t <= b) {
        return t;
    }
    if to_start {
        speech
            .iter()
            .map(|s| s.0)
            .filter(|&a| a > t && a - t <= reach)
            .fold(t, |best, a| if best == t || a < best { a } else { best })
    } else {
        speech
            .iter()
            .map(|s| s.1)
            .filter(|&b| b < t && t - b <= reach)
            .fold(t, |best, b| if best == t || b > best { b } else { best })
    }
}

/// Cues from `segments` (seconds, in order), aligned to `speech` (seconds; empty: as they
/// are).
pub fn cues(segments: &[Segment], speech: &[(f64, f64)]) -> Vec<Cue> {
    let mut out: Vec<Cue> = Vec::new();
    for s in segments {
        let mut start = snap(f64::from(s.start), speech, true, 1.0);
        let mut end = snap(f64::from(s.end), speech, false, 1.0);
        if end <= start {
            (start, end) = (f64::from(s.start), f64::from(s.end));
        }
        let mut ps = Vec::new();
        pieces(&s.text, &mut ps);
        // Also split by time: more pieces when a short text is said slowly.
        while (end - start) / ps.len().max(1) as f64 > MAX_SECS {
            let longest = ps
                .iter()
                .enumerate()
                .filter(|(_, p)| split_at(p).is_some() && chars(p) > 12)
                .max_by_key(|(_, p)| chars(p))
                .map(|(i, _)| i);
            let Some(i) = longest else { break };
            let p = ps.remove(i);
            let at = split_at(&p).unwrap_or(0);
            ps.insert(i, p[at..].trim().to_owned());
            ps.insert(i, p[..at].trim().to_owned());
        }
        let total: usize = ps.iter().map(|p| chars(p).max(1)).sum();
        let mut t = start;
        for p in ps {
            let share = chars(&p).max(1) as f64 / total.max(1) as f64;
            let e = t + (end - start) * share;
            out.push(Cue {
                start: t,
                end: e.min(t + MAX_SECS),
                text: wrap(&p),
            });
            t = e;
        }
    }
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    // No overlaps; at least MIN_SECS when there is room.
    for i in 0..out.len() {
        let next = out.get(i + 1).map_or(f64::INFINITY, |c| c.start);
        let c = &mut out[i];
        if c.end > next {
            c.end = next;
        }
        if c.end - c.start < MIN_SECS {
            c.end = (c.start + MIN_SECS).min(next);
        }
    }
    out.retain(|c| c.end > c.start && !c.text.is_empty());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: f32, end: f32, text: &str) -> Segment {
        Segment {
            start,
            end,
            text: text.into(),
        }
    }

    #[test]
    fn short_segments_are_one_cue_each() {
        let c = cues(&[seg(1.0, 2.5, "Hello there."), seg(3.0, 3.2, "Hi.")], &[]);
        assert_eq!(c.len(), 2);
        assert_eq!(
            (c[0].start, c[0].end, c[0].text.as_str()),
            (1.0, 2.5, "Hello there.")
        );
        // Lengthened to the minimum.
        assert!((c[1].end - 3.8).abs() < 1e-6);
    }

    #[test]
    fn long_segments_split_at_sentences_and_wrap() {
        let text = "This is the first sentence of a rather long passage. And here is a second one that goes on for a while, with a clause.";
        let c = cues(&[seg(0.0, 10.0, text)], &[]);
        assert_eq!(c.len(), 2, "{c:?}");
        assert!(c[0].text.ends_with("passage."), "{c:?}");
        assert!(c[1].text.starts_with("And here"), "{c:?}");
        for q in &c {
            assert!(q.text.lines().count() <= 2);
            assert!(q.text.lines().all(|l| l.chars().count() <= LINE), "{q:?}");
        }
        // Time shared by length, back to back.
        assert_eq!(c[0].end, c[1].start);
        assert!((c[1].end - 10.0).abs() < 1e-9);
    }

    #[test]
    fn slow_speech_is_split_by_time_and_scripts_without_spaces_wrap() {
        let c = cues(
            &[seg(
                0.0,
                20.0,
                "We waited, and waited, and waited for a long time.",
            )],
            &[],
        );
        assert!(c.len() >= 3, "{c:?}");
        assert!(c.iter().all(|q| q.end - q.start <= MAX_SECS + 1e-9));
        let w = wrap(&"字".repeat(50));
        let lines: Vec<&str> = w.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].chars().count(), 25);
    }

    #[test]
    fn edges_follow_the_speech() {
        // Whisper says 0.0–4.0; the speech was 0.6–3.5.
        let c = cues(&[seg(0.0, 4.0, "Words.")], &[(0.6, 3.5)]);
        assert_eq!((c[0].start, c[0].end), (0.6, 3.5));
        // Too far from any speech: left alone.
        let c = cues(&[seg(0.0, 4.0, "Words.")], &[(2.0, 2.5)]);
        assert_eq!((c[0].start, c[0].end), (0.0, 4.0));
        // Overlaps are cut.
        let c = cues(&[seg(0.0, 3.0, "One."), seg(2.0, 4.0, "Two.")], &[]);
        assert_eq!(c[0].end, 2.0);
    }
}
