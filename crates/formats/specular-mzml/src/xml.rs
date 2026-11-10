//! Small XML helpers: attributes of a start tag read by hand (for index checks and the fast
//! scan) and through quick-xml, entity unescaping, number parsing.

use quick_xml::events::BytesStart;

/// The five predefined entities and numeric character references replaced.
pub fn unescape(v: &[u8]) -> String {
    let s = String::from_utf8_lossy(v);
    if !s.contains('&') {
        return s.into_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest: &str = &s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = match rest.find(';') {
            Some(e) if e <= 12 => e,
            _ => {
                out.push('&');
                rest = &rest[1..];
                continue;
            }
        };
        let name = &rest[1..end];
        let c = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => {
                let code = if let Some(h) = name.strip_prefix("#x").or(name.strip_prefix("#X")) {
                    u32::from_str_radix(h, 16).ok()
                } else {
                    name.strip_prefix('#').and_then(|d| d.parse().ok())
                };
                code.and_then(char::from_u32)
            }
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The value of attribute `key` of a start tag (the bytes from `<` to `>`), unescaped.
pub fn tag_attr(tag: &[u8], key: &[u8]) -> Option<String> {
    // Past the element name.
    let mut i = tag.iter().position(|c| c.is_ascii_whitespace())?;
    while i < tag.len() {
        while i < tag.len() && tag[i].is_ascii_whitespace() {
            i += 1;
        }
        let ns = i;
        while i < tag.len()
            && !matches!(tag[i], b'=' | b'>' | b'/')
            && !tag[i].is_ascii_whitespace()
        {
            i += 1;
        }
        let name = &tag[ns..i];
        while i < tag.len() && tag[i].is_ascii_whitespace() {
            i += 1;
        }
        if tag.get(i) != Some(&b'=') {
            return None;
        }
        i += 1;
        while i < tag.len() && tag[i].is_ascii_whitespace() {
            i += 1;
        }
        let q = *tag.get(i)?;
        if q != b'"' && q != b'\'' {
            return None;
        }
        let vs = i + 1;
        let ve = vs + tag[vs..].iter().position(|&c| c == q)?;
        if name == key {
            return Some(unescape(&tag[vs..ve]));
        }
        i = ve + 1;
    }
    None
}

/// The start tag beginning at `at` (`<` to `>`), at most 64 KiB of it.
pub fn start_tag(data: &[u8], at: usize) -> Option<&[u8]> {
    let rest = data.get(at..)?;
    let n = rest.len().min(64 << 10);
    let end = memchr::memchr(b'>', &rest[..n])?;
    Some(&rest[..=end])
}

/// Attribute `key` of a quick-xml start tag, unescaped.
pub fn attr(e: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    e.attributes()
        .with_checks(false)
        .flatten()
        .find(|a| a.key.local_name().as_ref() == key)
        .map(|a| unescape(&a.value))
}

pub fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// An `xs:duration` such as `PT12.5S` or `PT1M3.2S`, in minutes; a plain number is seconds.
pub fn duration_minutes(s: &str) -> Option<f64> {
    let s = s.trim();
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s),
    };
    let Some(rest) = body.strip_prefix('P') else {
        return num(s).map(|v| v / 60.0);
    };
    let mut total = 0.0;
    let mut cur = String::new();
    let mut time = false;
    for c in rest.chars() {
        let v = num(&cur);
        match (c, time) {
            ('T', false) if cur.is_empty() => time = true,
            ('D', false) => total += v? * 1440.0,
            ('H', true) => total += v? * 60.0,
            ('M', true) => total += v?,
            ('S', true) => total += v? / 60.0,
            (c, _) if c.is_ascii_digit() || c == '.' => {
                cur.push(c);
                continue;
            }
            _ => return None,
        }
        cur.clear();
    }
    if !cur.is_empty() {
        return None;
    }
    Some(if neg { -total } else { total })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_by_hand() {
        let t = br#"<spectrum index="3" id="scan=4 &amp; more" defaultArrayLength='7'>"#;
        assert_eq!(tag_attr(t, b"id").as_deref(), Some("scan=4 & more"));
        assert_eq!(tag_attr(t, b"index").as_deref(), Some("3"));
        assert_eq!(tag_attr(t, b"defaultArrayLength").as_deref(), Some("7"));
        assert_eq!(tag_attr(t, b"x"), None);
    }

    #[test]
    fn durations() {
        assert_eq!(duration_minutes("PT60S"), Some(1.0));
        assert_eq!(duration_minutes("PT1M30S"), Some(1.5));
        assert_eq!(duration_minutes("PT1H"), Some(60.0));
        assert_eq!(duration_minutes("120"), Some(2.0));
        assert_eq!(duration_minutes("PTxS"), None);
        assert_eq!(duration_minutes("P1DT1M"), Some(1441.0));
        assert_eq!(duration_minutes("-PT30S"), Some(-0.5));
    }

    #[test]
    fn entities() {
        assert_eq!(unescape(b"a&lt;b&#x41;&#66;&bogus;"), "a<bAB&bogus;");
    }
}
