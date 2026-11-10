//! Text: code pages, `\U+XXXX` / `\M+nXXXX` escapes, caret-encoded control characters,
//! `%%` codes, and MTEXT inline formatting into paragraphs and spans.

use encoding_rs::Encoding;
use specular_vector::{HAlign, Paragraph, Span};

use crate::aci;

/// How a file's strings are decoded.
#[derive(Clone, Copy)]
pub(crate) struct Decoder {
    /// `None` is UTF-8 (AutoCAD 2007 and later).
    enc: Option<&'static Encoding>,
}

impl Decoder {
    pub fn utf8() -> Decoder {
        Decoder { enc: None }
    }

    /// From `$ACADVER` and `$DWGCODEPAGE`.
    pub fn new(version: &str, codepage: Option<&str>) -> Decoder {
        let modern = version
            .strip_prefix("AC")
            .and_then(|v| v.parse::<u32>().ok())
            .is_some_and(|v| v >= 1021);
        if modern {
            return Decoder::utf8();
        }
        let enc = codepage
            .map(codepage_encoding)
            .unwrap_or(encoding_rs::WINDOWS_1252);
        Decoder { enc: Some(enc) }
    }

    /// Decodes a string value, resolving `\U+` and `\M+` escapes.
    pub fn decode(&self, b: &[u8]) -> String {
        let s = match self.enc {
            None => String::from_utf8_lossy(b).into_owned(),
            Some(e) => {
                if b.is_ascii() {
                    // Safe fast path: ASCII is the same in every ANSI code page.
                    String::from_utf8_lossy(b).into_owned()
                } else {
                    e.decode_without_bom_handling(b).0.into_owned()
                }
            }
        };
        if s.contains("\\U+") || s.contains("\\u+") || s.contains("\\M+") || s.contains("\\m+") {
            unescape_unicode(&s)
        } else {
            s
        }
    }
}

/// The encoding of a `$DWGCODEPAGE` name ("ANSI_1252"); DOS and unknown pages fall back to
/// Windows-1252.
pub(crate) fn codepage_encoding(cp: &str) -> &'static Encoding {
    let c = cp.trim().to_ascii_uppercase();
    let num = c
        .trim_start_matches("ANSI_")
        .trim_start_matches("DOS")
        .trim_start_matches("CP");
    match num {
        "874" => encoding_rs::WINDOWS_874,
        "932" => encoding_rs::SHIFT_JIS,
        "936" => encoding_rs::GBK,
        "949" => encoding_rs::EUC_KR,
        "950" => encoding_rs::BIG5,
        "1250" => encoding_rs::WINDOWS_1250,
        "1251" => encoding_rs::WINDOWS_1251,
        "1253" => encoding_rs::WINDOWS_1253,
        "1254" => encoding_rs::WINDOWS_1254,
        "1255" => encoding_rs::WINDOWS_1255,
        "1256" => encoding_rs::WINDOWS_1256,
        "1257" => encoding_rs::WINDOWS_1257,
        "1258" => encoding_rs::WINDOWS_1258,
        "866" => encoding_rs::IBM866,
        _ => encoding_rs::WINDOWS_1252,
    }
}

fn hex_val(s: &[u8]) -> Option<u32> {
    let mut v = 0u32;
    for &c in s {
        v = v * 16 + (c as char).to_digit(16)?;
    }
    Some(v)
}

/// Replaces `\U+XXXX` with the character and `\M+nXXXX` with the double-byte character of
/// code page n (1 932, 2 950, 3 949, 4 Johab, 5 936).
fn unescape_unicode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut last = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 2 < b.len() && b[i + 2] == b'+' {
            let kind = b[i + 1].to_ascii_uppercase();
            if kind == b'U' && i + 7 <= b.len() {
                if let Some(cp) = hex_val(&b[i + 3..i + 7])
                    && let Some(c) = char::from_u32(cp)
                {
                    out.push_str(&s[last..i]);
                    out.push(c);
                    i += 7;
                    last = i;
                    continue;
                }
            } else if kind == b'M' && i + 8 <= b.len() {
                let enc = match b[i + 3] {
                    b'1' => Some(encoding_rs::SHIFT_JIS),
                    b'2' => Some(encoding_rs::BIG5),
                    b'3' => Some(encoding_rs::EUC_KR),
                    b'5' => Some(encoding_rs::GBK),
                    _ => None,
                };
                if let (Some(enc), Some(v)) = (enc, hex_val(&b[i + 4..i + 8])) {
                    let bytes = [(v >> 8) as u8, v as u8];
                    let (d, _, bad) = enc.decode(&bytes);
                    if !bad {
                        out.push_str(&s[last..i]);
                        out.push_str(&d);
                        i += 8;
                        last = i;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    out.push_str(&s[last..]);
    out
}

/// Caret-encoded control characters (`^J` is a line feed, `^ ` a caret).
fn caret(c: char) -> Option<char> {
    match c {
        ' ' => Some('^'),
        '@'..='_' => char::from_u32(c as u32 - 64),
        _ => None,
    }
}

/// Toggles from `%%u` / `%%o` / `%%k` in single-line text.
#[derive(Default, Clone, Copy)]
pub(crate) struct Toggles {
    pub underline: bool,
    pub overline: bool,
    pub strike: bool,
}

fn percent(c: char) -> Option<char> {
    Some(match c.to_ascii_lowercase() {
        'c' => '⌀',
        'd' => '°',
        'p' => '±',
        '%' => '%',
        _ => return None,
    })
}

/// TEXT/ATTRIB content: `%%` codes and carets resolved, split into spans where `%%u`,
/// `%%o` or `%%k` toggle.
pub(crate) fn single_line(s: &str, base: &Span) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut cur = String::new();
    let mut t = Toggles::default();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let flush = |cur: &mut String, spans: &mut Vec<Span>, t: Toggles| {
        if !cur.is_empty() {
            let mut sp = base.clone();
            sp.text = std::mem::take(cur);
            sp.underline = t.underline;
            sp.overline = t.overline;
            sp.strike = t.strike;
            spans.push(sp);
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '%' && chars.get(i + 1) == Some(&'%') && i + 2 < chars.len() {
            let k = chars[i + 2];
            if let Some(r) = percent(k) {
                cur.push(r);
                i += 3;
                continue;
            }
            match k.to_ascii_lowercase() {
                'u' | 'o' | 'k' => {
                    flush(&mut cur, &mut spans, t);
                    match k.to_ascii_lowercase() {
                        'u' => t.underline = !t.underline,
                        'o' => t.overline = !t.overline,
                        _ => t.strike = !t.strike,
                    }
                    i += 3;
                    continue;
                }
                '0'..='9' => {
                    let mut j = i + 2;
                    let mut n = 0u32;
                    while j < chars.len() && j < i + 5 && chars[j].is_ascii_digit() {
                        n = n * 10 + chars[j] as u32 - '0' as u32;
                        j += 1;
                    }
                    if let Some(ch) = char::from_u32(n) {
                        cur.push(ch);
                    }
                    i = j;
                    continue;
                }
                _ => {}
            }
        }
        if c == '^'
            && i + 1 < chars.len()
            && let Some(r) = caret(chars[i + 1])
        {
            cur.push(if r.is_control() { ' ' } else { r });
            i += 2;
            continue;
        }
        cur.push(c);
        i += 1;
    }
    flush(&mut cur, &mut spans, t);
    spans
}

#[derive(Clone)]
struct State {
    span: Span,
}

/// Parses MTEXT content into paragraphs. `base` carries the style's font, the entity's
/// height and width factor.
pub(crate) fn mtext(s: &str, base: &Span) -> Vec<Paragraph> {
    let chars: Vec<char> = s.chars().collect();
    let mut paras: Vec<Paragraph> = vec![Paragraph::default()];
    let mut stack: Vec<State> = Vec::new();
    let mut st = State { span: base.clone() };
    st.span.text.clear();
    let mut cur = String::new();
    let mut i = 0;

    fn flush(cur: &mut String, paras: &mut [Paragraph], st: &State) {
        if cur.is_empty() {
            return;
        }
        let mut sp = st.span.clone();
        sp.text = std::mem::take(cur);
        if let Some(p) = paras.last_mut() {
            // Merge with the previous span when nothing but the text differs.
            if let Some(last) = p.spans.last_mut() {
                let mut a = last.clone();
                a.text.clear();
                let mut b = sp.clone();
                b.text.clear();
                if a == b {
                    last.text.push_str(&sp.text);
                    return;
                }
            }
            p.spans.push(sp);
        }
    }

    // Reads up to the next ';' (or the end), returning the argument.
    fn arg(chars: &[char], i: &mut usize) -> String {
        let mut a = String::new();
        while *i < chars.len() && chars[*i] != ';' {
            a.push(chars[*i]);
            *i += 1;
        }
        if *i < chars.len() {
            *i += 1;
        }
        a
    }

    while i < chars.len() {
        let c = chars[i];
        match c {
            '{' => {
                flush(&mut cur, &mut paras, &st);
                if stack.len() < 64 {
                    stack.push(st.clone());
                }
                i += 1;
            }
            '}' => {
                flush(&mut cur, &mut paras, &st);
                if let Some(s) = stack.pop() {
                    st = s;
                }
                i += 1;
            }
            '\\' if i + 1 < chars.len() => {
                let k = chars[i + 1];
                i += 2;
                match k {
                    'P' | 'N' | 'X' => {
                        flush(&mut cur, &mut paras, &st);
                        let align = paras.last().and_then(|p| p.align);
                        paras.push(Paragraph {
                            spans: Vec::new(),
                            align,
                        });
                    }
                    '~' => cur.push('\u{a0}'),
                    '\\' | '{' | '}' => cur.push(k),
                    'L' | 'l' | 'O' | 'o' | 'K' | 'k' => {
                        flush(&mut cur, &mut paras, &st);
                        let on = k.is_ascii_uppercase();
                        match k.to_ascii_uppercase() {
                            'L' => st.span.underline = on,
                            'O' => st.span.overline = on,
                            _ => st.span.strike = on,
                        }
                    }
                    'H' | 'W' | 'Q' | 'T' | 'A' | 'C' | 'c' | 'f' | 'F' | 'p' => {
                        flush(&mut cur, &mut paras, &st);
                        let a = arg(&chars, &mut i);
                        apply_code(k, &a, &mut st.span, base, &mut paras);
                    }
                    'S' => {
                        // Stacked text "a^b", "a/b", "a#b": shown inline as a/b.
                        let a = arg(&chars, &mut i);
                        let mut t = String::new();
                        let mut esc = false;
                        for ch in a.chars() {
                            if esc {
                                t.push(ch);
                                esc = false;
                            } else if ch == '\\' {
                                esc = true;
                            } else if ch == '^' || ch == '#' {
                                t.push('/');
                            } else {
                                t.push(ch);
                            }
                        }
                        cur.push_str(t.trim_start_matches(' '));
                    }
                    _ => {
                        // Unknown code: keep the character, as AutoCAD does.
                        cur.push(k);
                    }
                }
            }
            '%' if chars.get(i + 1) == Some(&'%') && i + 2 < chars.len() => {
                let k = chars[i + 2];
                if let Some(r) = percent(k) {
                    cur.push(r);
                    i += 3;
                } else if k.is_ascii_digit() {
                    let mut j = i + 2;
                    let mut n = 0u32;
                    while j < chars.len() && j < i + 5 && chars[j].is_ascii_digit() {
                        n = n * 10 + chars[j] as u32 - '0' as u32;
                        j += 1;
                    }
                    if let Some(ch) = char::from_u32(n) {
                        cur.push(ch);
                    }
                    i = j;
                } else if matches!(k.to_ascii_lowercase(), 'u' | 'o' | 'k') {
                    flush(&mut cur, &mut paras, &st);
                    match k.to_ascii_lowercase() {
                        'u' => st.span.underline = !st.span.underline,
                        'o' => st.span.overline = !st.span.overline,
                        _ => st.span.strike = !st.span.strike,
                    }
                    i += 3;
                } else {
                    cur.push(c);
                    i += 1;
                }
            }
            '^' if i + 1 < chars.len() && caret(chars[i + 1]).is_some() => {
                let r = caret(chars[i + 1]).unwrap_or(' ');
                i += 2;
                if r == '\n' || r == '\r' {
                    flush(&mut cur, &mut paras, &st);
                    let align = paras.last().and_then(|p| p.align);
                    paras.push(Paragraph {
                        spans: Vec::new(),
                        align,
                    });
                } else if r == '\t' {
                    cur.push('\t');
                } else if !r.is_control() {
                    cur.push(r);
                }
            }
            '\n' => {
                flush(&mut cur, &mut paras, &st);
                paras.push(Paragraph::default());
                i += 1;
            }
            '\r' => i += 1,
            _ => {
                cur.push(c);
                i += 1;
            }
        }
    }
    flush(&mut cur, &mut paras, &st);
    paras
}

fn apply_code(k: char, a: &str, sp: &mut Span, base: &Span, paras: &mut [Paragraph]) {
    let num = |s: &str| -> Option<f64> {
        let v: f64 = s.trim().parse().ok()?;
        v.is_finite().then_some(v)
    };
    match k {
        'H' => {
            let a = a.trim();
            if let Some(f) = a.strip_suffix(['x', 'X']) {
                if let Some(v) = num(f)
                    && v > 0.0
                {
                    sp.height *= v;
                }
            } else if let Some(v) = num(a)
                && v > 0.0
            {
                sp.height = v;
            }
        }
        'W' => {
            if let Some(v) = num(a.trim_end_matches(['x', 'X']))
                && v > 0.0
            {
                sp.width_factor = v;
            }
        }
        'Q' => {
            if let Some(v) = num(a) {
                sp.oblique = v;
            }
        }
        'C' => {
            if let Ok(v) = a.trim().parse::<i64>() {
                sp.color = match v {
                    0 | 256 => None,
                    v => Some(aci::color(v)),
                };
            }
        }
        'c' => {
            if let Ok(v) = a.trim().parse::<i64>() {
                sp.color = Some(aci::true_color(v));
            }
        }
        'f' | 'F' => {
            // \fArial|b1|i0|c0|p34; or \Ffontfile.shx|c0;
            let mut parts = a.split('|');
            let name = parts.next().unwrap_or("").trim().to_string();
            sp.bold = base.bold;
            sp.italic = base.italic;
            for p in parts {
                let p = p.trim();
                if let Some(v) = p.strip_prefix('b') {
                    sp.bold = v.trim() == "1";
                } else if let Some(v) = p.strip_prefix('i') {
                    sp.italic = v.trim() == "1";
                }
            }
            sp.family = if name.is_empty() { None } else { Some(name) };
        }
        'p' => {
            // Paragraph properties: only the alignment ("qc", "ql", "qr", "qj", "qd").
            for part in a.split(',') {
                if let Some(q) = part.trim().strip_prefix('q') {
                    let al = match q.chars().next() {
                        Some('l') => Some(HAlign::Left),
                        Some('c') => Some(HAlign::Center),
                        Some('r') => Some(HAlign::Right),
                        Some('j') | Some('d') => Some(HAlign::Left),
                        _ => None,
                    };
                    if let Some(p) = paras.last_mut() {
                        p.align = al;
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_vector::Color;

    fn span_colors(p: &[Paragraph]) -> Vec<Option<Color>> {
        p.iter()
            .flat_map(|p| p.spans.iter().map(|s| s.color))
            .collect()
    }

    fn base() -> Span {
        Span::new("", 0, 2.5)
    }

    fn plain(p: &[Paragraph]) -> Vec<String> {
        p.iter()
            .map(|p| p.spans.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn unicode_escapes() {
        assert_eq!(unescape_unicode("a\\U+00B0b"), "a°b");
        assert_eq!(unescape_unicode("\\U+4E2D\\U+6587"), "中文");
        assert_eq!(unescape_unicode("\\M+193FA"), "\u{65e5}");
        assert_eq!(unescape_unicode("\\U+zz"), "\\U+zz");
    }

    #[test]
    fn text_codes() {
        let s = single_line("%%c10 %%d %%p0.1 %%u%%137", &base());
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].text, "⌀10 ° ±0.1 ");
        assert!(s[1].underline);
        assert_eq!(s[1].text, "\u{89}");
    }

    #[test]
    fn mtext_codes() {
        let p = mtext(
            "{\\fArial|b1|i0;Bold}\\Pline\\~two\\\\x\\{y\\}{\\H2x;big}{\\C1;red}\\S1/2;\\pqc;\\Lu\\l",
            &base(),
        );
        assert_eq!(plain(&p), vec!["Bold", "line\u{a0}two\\x{y}bigred1/2u"]);
        assert!(p[0].spans[0].bold);
        assert_eq!(p[0].spans[0].family.as_deref(), Some("Arial"));
        let big = p[1].spans.iter().find(|s| s.text == "big").unwrap();
        assert_eq!(big.height, 5.0);
        let cols = span_colors(&p);
        assert!(cols.contains(&Some(Color::Rgb(255, 0, 0))));
        assert_eq!(p[1].align, Some(HAlign::Center));
        assert!(p[1].spans.last().unwrap().underline);
    }
}
