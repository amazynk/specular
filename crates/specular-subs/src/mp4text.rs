//! MP4 subtitle samples: 3GPP timed text (`tx3g`, also QuickTime `text` as chapter tracks
//! use) and ISO WebVTT (`wvtt`). The container gives each sample its time span.
//!
//! - `tx3g`: a 16-bit length, the text (UTF-8, or UTF-16 with a BOM), then boxes; `styl`
//!   gives bold, italic, underline and colour for character ranges.
//! - `wvtt`: boxes: `vttc` (a cue: `payl` text, `sttg` settings), `vtte` (nothing shown).

use crate::{Span, TextCue, markup, vtt};

/// A tx3g / QuickTime text sample's plain text (no styling), for chapter names.
pub fn tx3g_text(sample: &[u8]) -> Option<String> {
    let n = usize::from(u16::from_be_bytes([*sample.first()?, *sample.get(1)?]));
    let b = sample.get(2..2 + n)?;
    Some(if b.starts_with(&[0xFE, 0xFF]) {
        let u: Vec<u16> = b[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u)
    } else if b.starts_with(&[0xFF, 0xFE]) {
        let u: Vec<u16> = b[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u)
    } else {
        String::from_utf8_lossy(b).into_owned()
    })
}

/// Boxes in `d`: (type, body).
fn boxes(d: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut p = 0usize;
    std::iter::from_fn(move || {
        if p + 8 > d.len() {
            return None;
        }
        let size = u32::from_be_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]) as usize;
        let kind = [d[p + 4], d[p + 5], d[p + 6], d[p + 7]];
        if size < 8 || p + size > d.len() {
            return None;
        }
        let body = &d[p + 8..p + size];
        p += size;
        Some((kind, body))
    })
}

/// A tx3g sample as a cue (`None` when empty: a gap between subtitles).
pub fn tx3g(sample: &[u8]) -> Option<TextCue> {
    let text = tx3g_text(sample)?;
    if text.trim().is_empty() {
        return None;
    }
    let n = usize::from(u16::from_be_bytes([sample[0], sample[1]]));
    let rest = sample.get(2 + n..).unwrap_or_default();
    // Style records: (start, end, flags, colour).
    let mut styles: Vec<(usize, usize, u8, [u8; 4])> = Vec::new();
    for (kind, body) in boxes(rest) {
        if &kind == b"styl" && body.len() >= 2 {
            let count = usize::from(u16::from_be_bytes([body[0], body[1]]));
            for r in body[2..].chunks_exact(12).take(count) {
                let s = usize::from(u16::from_be_bytes([r[0], r[1]]));
                let e = usize::from(u16::from_be_bytes([r[2], r[3]]));
                styles.push((s, e, r[6], [r[8], r[9], r[10], r[11]]));
            }
        }
    }
    let mut lines: Vec<Vec<Span>> = vec![Vec::new()];
    let mut last: Option<Option<usize>> = None;
    for (i, ch) in text.chars().enumerate() {
        match ch {
            '\r' => continue,
            '\n' => {
                lines.push(Vec::new());
                last = None;
                continue;
            }
            _ => {}
        }
        let st = styles.iter().position(|s| s.0 <= i && i < s.1);
        let line = lines.last_mut().expect("a line");
        if last == Some(st)
            && let Some(span) = line.last_mut()
        {
            span.text.push(ch);
            continue;
        }
        let mut span = Span::plain(ch.to_string());
        if let Some(&(_, _, f, c)) = st.and_then(|k| styles.get(k)) {
            span.bold = f & 1 != 0;
            span.italic = f & 2 != 0;
            span.underline = f & 4 != 0;
            // White (the usual default) stays the renderer's default.
            if c[..3] != [255, 255, 255] {
                span.colour = Some(c);
            }
        }
        line.push(span);
        last = Some(st);
    }
    Some(TextCue {
        lines,
        align: 2,
        pos: None,
        margin_v: None,
    })
}

/// A wvtt sample's cues (none for an empty `vtte` sample).
pub fn wvtt(sample: &[u8]) -> Vec<TextCue> {
    let mut out = Vec::new();
    for (kind, body) in boxes(sample) {
        if &kind != b"vttc" {
            continue;
        }
        let (mut text, mut settings) = (String::new(), String::new());
        for (k, b) in boxes(body) {
            match &k {
                b"payl" => text = String::from_utf8_lossy(b).into_owned(),
                b"sttg" => settings = String::from_utf8_lossy(b).into_owned(),
                _ => {}
            }
        }
        let c = vtt::cue_text(&text, &settings);
        if !c.is_empty() {
            out.push(c);
        }
    }
    out
}

/// Matroska `S_TEXT/UTF8` block text (SubRip markup).
pub fn utf8_block(payload: &[u8]) -> Option<TextCue> {
    let c = markup::parse(&String::from_utf8_lossy(payload));
    (!c.is_empty()).then_some(c)
}

/// Test samples.
#[doc(hidden)]
pub mod fixture {
    pub fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    /// A tx3g sample; `bold`: the first `n` characters bold.
    pub fn tx3g(text: &str, bold: Option<u16>) -> Vec<u8> {
        let mut v = (text.len() as u16).to_be_bytes().to_vec();
        v.extend_from_slice(text.as_bytes());
        if let Some(n) = bold {
            let mut s = 1u16.to_be_bytes().to_vec();
            s.extend_from_slice(&0u16.to_be_bytes());
            s.extend_from_slice(&n.to_be_bytes());
            s.extend_from_slice(&[0, 1, 1, 18, 255, 255, 0, 255]);
            v.extend_from_slice(&bx(b"styl", &s));
        }
        v
    }

    pub fn wvtt(text: &str, settings: &str) -> Vec<u8> {
        let mut c = bx(b"payl", text.as_bytes());
        if !settings.is_empty() {
            c.extend_from_slice(&bx(b"sttg", settings.as_bytes()));
        }
        bx(b"vttc", &c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_text() {
        let c = tx3g(&fixture::tx3g("Bold rest\nnext", Some(4))).unwrap();
        assert_eq!(c.plain_text(), "Bold rest\nnext");
        assert!(c.lines[0][0].bold);
        assert_eq!(c.lines[0][0].text, "Bold");
        assert_eq!(c.lines[0][0].colour, Some([255, 255, 0, 255]));
        assert!(!c.lines[0][1].bold);
        assert!(tx3g(&fixture::tx3g("", None)).is_none());
        assert_eq!(
            tx3g_text(&fixture::tx3g("Chapter 1", None)).as_deref(),
            Some("Chapter 1")
        );
    }

    #[test]
    fn web_vtt_samples() {
        let s = fixture::wvtt("<i>שלום</i>", "line:0");
        let cues = wvtt(&s);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].plain_text(), "שלום");
        assert!(cues[0].lines[0][0].italic);
        assert_eq!(cues[0].align, 8);
        assert!(wvtt(&fixture::bx(b"vtte", &[])).is_empty());
    }
}
