//! A file's own descriptive tags outside Exif, XMP and IPTC, for the info panel: PNG text
//! chunks (`tEXt`, `zTXt`, `iTXt`), GIF comment extensions, and a movie's tags (QuickTime /
//! MP4 items and Matroska simple tags, read by the video demuxers and named here). Each comes
//! with its XMP equivalents (the PNG keywords of the PNG specification, the QuickTime and
//! iTunes item names, the Matroska tag names), which the info panel reads where the XMP has
//! none. Only chunk and block headers and the text itself are read; lengths are checked and
//! capped, walks bounded.

use crate::exif_locate::MAX_STEPS;

/// The longest text kept (a long comment or description is cut).
const MAX_TEXT: usize = 64 << 10;
/// Most tags kept per file.
const MAX_TAGS: usize = 256;

/// A file's descriptive tags.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextTags {
    /// What they are: "PNG", "GIF", "QuickTime", "Matroska".
    pub kind: &'static str,
    /// The tags as XMP properties (see `specular_metadata::TagFacts::props`).
    pub props: Vec<(&'static str, String)>,
    /// Every tag as (name, value), in file order.
    pub fields: Vec<(String, String)>,
}

impl TextTags {
    fn new(kind: &'static str) -> Self {
        Self {
            kind,
            ..Default::default()
        }
    }

    fn push(&mut self, name: &str, value: String, prop: Option<&'static str>) {
        let value = value.trim().to_owned();
        if value.is_empty() || self.fields.len() >= MAX_TAGS {
            return;
        }
        if let Some(p) = prop {
            match p {
                // A date as ISO 8601, when it reads as one.
                "xmp:CreateDate" | "photoshop:DateCreated" => {
                    if let Some(d) = iso_date(&value) {
                        self.props.push((p, d));
                    }
                }
                "dc:subject" => self.props.extend(
                    value
                        .split([',', ';'])
                        .map(str::trim)
                        .filter(|k| !k.is_empty())
                        .map(|k| (p, k.to_owned())),
                ),
                // An ISO 6709 position: latitude, longitude and the height.
                "iso6709" => {
                    if let Some((lat, lon, alt)) = iso6709(&value) {
                        self.props.push(("exif:GPSLatitude", coord(lat, 'N', 'S')));
                        self.props.push(("exif:GPSLongitude", coord(lon, 'E', 'W')));
                        if let Some(a) = alt {
                            self.props.push(("exif:GPSAltitude", format!("{a}")));
                        }
                    }
                }
                _ => self.props.push((p, value.clone())),
            }
        }
        self.fields.push((name.to_owned(), value));
    }
}

/// "48.8584N" from a signed coordinate.
fn coord(v: f64, pos: char, neg: char) -> String {
    format!("{:.7}{}", v.abs(), if v < 0.0 { neg } else { pos })
}

/// The descriptive tags of a whole file in memory (PNG, APNG, GIF); empty for other formats.
pub fn text_tags(bytes: &[u8]) -> Vec<TextTags> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        png(bytes)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        gif(bytes).0
    } else {
        Vec::new()
    }
    .into_iter()
    .filter(|t| !t.fields.is_empty())
    .collect()
}

/// The XMP packet of a GIF (an "XMP DataXMP" application extension).
pub fn gif_xmp(bytes: &[u8]) -> Option<Vec<u8>> {
    if !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return None;
    }
    gif(bytes).1
}

/// A PNG keyword's XMP property (the keywords the PNG specification defines).
fn png_prop(keyword: &str) -> Option<&'static str> {
    Some(match keyword {
        "Title" => "dc:title",
        "Author" => "dc:creator",
        "Description" => "dc:description",
        "Copyright" => "dc:rights",
        "Creation Time" => "xmp:CreateDate",
        "Software" => "xmp:CreatorTool",
        "Comment" => "exif:UserComment",
        "Source" => "tiff:Model",
        _ => return None,
    })
}

/// Text as UTF-8 when valid, else Latin-1, cut at [`MAX_TEXT`].
fn text(b: &[u8]) -> String {
    let b = &b[..b.len().min(MAX_TEXT)];
    match std::str::from_utf8(b) {
        Ok(s) => s.to_owned(),
        Err(e) if e.error_len().is_none() => {
            // Cut inside a character: keep what is whole.
            String::from_utf8_lossy(&b[..e.valid_up_to()]).into_owned()
        }
        Err(_) => b.iter().map(|&c| char::from(c)).collect(),
    }
}

fn png(b: &[u8]) -> Vec<TextTags> {
    let mut t = TextTags::new("PNG");
    let mut i = 8usize;
    for _ in 0..MAX_STEPS {
        let Some(h) = b.get(i..i + 8) else { break };
        let len = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as usize;
        let Some(body) = b.get(i + 8..(i + 8).saturating_add(len)) else {
            break;
        };
        match &h[4..8] {
            b"IEND" => break,
            b"tEXt" | b"zTXt" | b"iTXt" => {
                if let Some((k, v)) = png_text(&h[4..8], body) {
                    // Metadata blocks read elsewhere (XMP, Exif and IPTC profiles).
                    if k != "XML:com.adobe.xmp" && !k.starts_with("Raw profile type") {
                        t.push(&k, v, png_prop(&k));
                    }
                }
            }
            _ => {}
        }
        i = i.saturating_add(12).saturating_add(len);
    }
    vec![t]
}

/// A text chunk's keyword and text.
fn png_text(kind: &[u8], body: &[u8]) -> Option<(String, String)> {
    let k = body.iter().take(80).position(|&c| c == 0)?;
    let keyword = text(&body[..k]);
    let rest = body.get(k + 1..)?;
    let value = match kind {
        b"tEXt" => text(rest),
        b"zTXt" => {
            if rest.first() != Some(&0) {
                return None;
            }
            text(&specular_png::inflate_zlib(&rest[1..], MAX_TEXT).ok()?)
        }
        _ => {
            let (flag, method) = (*rest.first()?, *rest.get(1)?);
            let lang = 2 + rest.get(2..)?.iter().position(|&c| c == 0)?;
            let tkw = lang + 1 + rest.get(lang + 1..)?.iter().position(|&c| c == 0)?;
            let t = rest.get(tkw + 1..)?;
            match (flag, method) {
                (0, _) => text(t),
                (1, 0) => text(&specular_png::inflate_zlib(t, MAX_TEXT).ok()?),
                _ => return None,
            }
        }
    };
    Some((keyword, value))
}

/// A GIF's comments and XMP packet.
fn gif(b: &[u8]) -> (Vec<TextTags>, Option<Vec<u8>>) {
    let mut t = TextTags::new("GIF");
    let mut xmp = None;
    let Some(&flags) = b.get(10) else {
        return (vec![t], None);
    };
    let mut i = 13usize;
    if flags & 0x80 != 0 {
        i += 3 << ((flags & 7) + 1);
    }
    // The data sub-blocks from `at`: their bytes, and where they end.
    let blocks = |mut at: usize, keep: bool| -> Option<(Vec<u8>, usize)> {
        let mut out = Vec::new();
        for _ in 0..MAX_STEPS * 4 {
            let n = usize::from(*b.get(at)?);
            at += 1;
            if n == 0 {
                return Some((out, at));
            }
            if keep && out.len() < MAX_TEXT {
                out.extend_from_slice(b.get(at..at + n)?);
            }
            at += n;
        }
        None
    };
    for _ in 0..MAX_STEPS {
        match b.get(i) {
            Some(0x21) => {
                let Some(&label) = b.get(i + 1) else { break };
                match label {
                    0xFE => {
                        let Some((c, end)) = blocks(i + 2, true) else {
                            break;
                        };
                        t.push("Comment", text(&c), Some("exif:UserComment"));
                        i = end;
                    }
                    0xFF if b.get(i + 2) == Some(&11)
                        && b.get(i + 3..i + 14) == Some(b"XMP DataXMP") =>
                    {
                        // The packet's bytes are written as they are (its "sub-block
                        // lengths" are its own text), ended by a 258-byte magic trailer.
                        let start = i + 14;
                        let Some(tail) = b.get(start..) else { break };
                        let end = find(tail, b"<?xpacket end=")
                            .and_then(|e| find(&tail[e..], b"?>").map(|q| e + q + 2));
                        if let Some(e) = end
                            && xmp.is_none()
                        {
                            xmp = Some(tail[..e].to_vec());
                        }
                        let Some((_, end)) = blocks(start, false) else {
                            break;
                        };
                        i = end;
                    }
                    _ => {
                        let Some((_, end)) = blocks(i + 2, false) else {
                            break;
                        };
                        i = end;
                    }
                }
            }
            Some(0x2C) => {
                let Some(&f) = b.get(i + 9) else { break };
                i += 10;
                if f & 0x80 != 0 {
                    i += 3 << ((f & 7) + 1);
                }
                // LZW minimum code size, then the image data.
                let Some((_, end)) = blocks(i + 1, false) else {
                    break;
                };
                i = end;
            }
            _ => break,
        }
    }
    (vec![t], xmp)
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// A movie's tags (QuickTime / MP4 items as the demuxer names them: "©nam", "©day",
/// "com.apple.quicktime.location.ISO6709"…; Matroska simple tag names) with their XMP
/// equivalents. `kind`: "QuickTime" or "Matroska".
pub fn movie_tags(kind: &'static str, items: &[(String, String)]) -> TextTags {
    let mut t = TextTags::new(kind);
    for (k, v) in items {
        // QuickTime `keys` names with or without Apple's prefix, Matroska's upper-case names,
        // a sound file's tag keys: compared without case.
        let bare = k.strip_prefix("com.apple.quicktime.").unwrap_or(k);
        let prop = match bare.to_ascii_lowercase().as_str() {
            "©nam" | "title" => Some("dc:title"),
            "©des" | "desc" | "ldes" | "description" | "summary" | "synopsis" => {
                Some("dc:description")
            }
            "©art" | "©aut" | "artist" | "author" | "director" => Some("dc:creator"),
            "cprt" | "©cpy" | "copyright" => Some("dc:rights"),
            "©cmt" | "comment" => Some("exif:UserComment"),
            "keyw" | "keywords" => Some("dc:subject"),
            "©too" | "©swr" | "software" | "encoder" | "writingapp" | "encoded_by" => {
                Some("xmp:CreatorTool")
            }
            "©xyz" | "location.iso6709" | "location" => Some("iso6709"),
            _ => None,
        };
        t.push(k, v.clone(), prop);
    }
    t
}

/// An ISO 6709 point ("+48.8584+002.2945+035.000/"): latitude, longitude and the height in
/// metres, in decimal degrees (the degrees-and-minutes forms are read too).
pub fn iso6709(s: &str) -> Option<(f64, f64, Option<f64>)> {
    let s = s.trim().trim_end_matches('/');
    // Split at each sign after the first character.
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, c) in s.char_indices().skip(1) {
        if c == '+' || c == '-' {
            parts.push(&s[start..i]);
            start = i;
        }
    }
    parts.push(&s[start..]);
    if !(2..=3).contains(&parts.len()) || !parts.iter().all(|p| p.starts_with(['+', '-'])) {
        return None;
    }
    // ±DD.D, ±DDMM.M or ±DDMMSS.S (latitude); one more degree digit for longitude.
    let angle = |p: &str, deg_digits: usize| -> Option<f64> {
        let neg = p.starts_with('-');
        let body = &p[1..];
        let int_len = body.find('.').unwrap_or(body.len());
        let v: f64 = body.parse().ok()?;
        let d = match int_len.checked_sub(deg_digits)? {
            0 => v,
            2 => {
                let deg = (v / 100.0).trunc();
                deg + (v - deg * 100.0) / 60.0
            }
            4 => {
                let deg = (v / 10_000.0).trunc();
                let min = ((v - deg * 10_000.0) / 100.0).trunc();
                deg + min / 60.0 + (v - deg * 10_000.0 - min * 100.0) / 3600.0
            }
            _ => return None,
        };
        Some(if neg { -d } else { d })
    };
    let lat = angle(parts[0], 2)?;
    let lon = angle(parts[1], 3)?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    let alt = parts.get(2).and_then(|a| {
        a.trim_end_matches("CRSWGS_84")
            .parse::<f64>()
            .ok()
            .filter(|a| a.is_finite())
    });
    Some((lat, lon, alt))
}

/// A date as ISO 8601 ("2021-03-24T16:29:00+01:00"), from ISO 8601 with or without the
/// colons, "YYYY:MM:DD HH:MM:SS" (Exif style), or RFC 1123 / RFC 2822 ("Wed, 24 Mar 2021
/// 16:29:00 +0100", as PNG's Creation Time suggests). Only the date when that is all there is.
pub fn iso_date(s: &str) -> Option<String> {
    let s = s.trim();
    if let Some(d) = rfc2822(s) {
        return Some(d);
    }
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| {
        b.get(r.clone())
            .filter(|x| x.iter().all(u8::is_ascii_digit))
            .map(|_| &s[r])
    };
    let (y, mo, d) = (digits(0..4)?, digits(5..7)?, digits(8..10)?);
    if !matches!(b.get(4), Some(b'-' | b':')) || b.get(7) != b.get(4) {
        return None;
    }
    let date = format!("{y}-{mo}-{d}");
    if !(1..=12).contains(&mo.parse::<u8>().ok()?) || !(1..=31).contains(&d.parse::<u8>().ok()?) {
        return None;
    }
    if !matches!(b.get(10), Some(b'T' | b' ')) {
        return (b.len() == 10).then_some(date);
    }
    let (h, mi) = (digits(11..13)?, digits(14..16)?);
    let sec = (b.get(16) == Some(&b':'))
        .then(|| digits(17..19))
        .flatten()
        .unwrap_or("00");
    let rest = s.get(if b.get(16) == Some(&b':') { 19 } else { 16 }..)?;
    // Fractions of a second are dropped; the zone kept ("Z", "+01:00", "+0100").
    let rest = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let zone = match rest.trim() {
        "" => String::new(),
        "Z" | "z" => "Z".to_owned(),
        z if z.len() == 6 && z.as_bytes()[3] == b':' && z.starts_with(['+', '-']) => z.to_owned(),
        z if z.len() == 5
            && z.starts_with(['+', '-'])
            && z[1..].bytes().all(|c| c.is_ascii_digit()) =>
        {
            format!("{}:{}", &z[..3], &z[3..])
        }
        _ => String::new(),
    };
    Some(format!("{date}T{h}:{mi}:{sec}{zone}"))
}

/// "Wed, 24 Mar 2021 16:29:00 +0100" (the weekday optional) as ISO 8601.
fn rfc2822(s: &str) -> Option<String> {
    let s = s.split_once(", ").map_or(s, |(_, r)| r);
    let mut w = s.split_whitespace();
    let day: u8 = w.next()?.parse().ok()?;
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| w.clone().next().is_some_and(|x| x.eq_ignore_ascii_case(m)))?
        + 1;
    w.next();
    let year: u16 = w.next()?.parse().ok()?;
    let time = w.next()?;
    let t: Vec<&str> = time.split(':').collect();
    if !(2..=3).contains(&t.len())
        || !t
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let zone = match w.next() {
        Some(z) if z.len() == 5 && z.starts_with(['+', '-']) => format!("{}:{}", &z[..3], &z[3..]),
        Some("GMT" | "UT" | "UTC" | "Z") => "Z".to_owned(),
        _ => String::new(),
    };
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{}:{}:{}{zone}",
        t[0],
        t[1],
        t.get(2).unwrap_or(&"00")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
        let mut c = (data.len() as u32).to_be_bytes().to_vec();
        c.extend(kind);
        c.extend(data);
        c.extend([0; 4]);
        c
    }

    #[test]
    fn png_text_chunks() {
        let mut f = b"\x89PNG\r\n\x1a\n".to_vec();
        f.extend(chunk(b"IHDR", &[0; 13]));
        f.extend(chunk(b"tEXt", b"Title\0A t\xeftle"));
        f.extend(chunk(b"iTXt", b"Author\0\0\0en\0\0Ana"));
        f.extend(chunk(
            b"tEXt",
            b"Creation Time\0Wed, 24 Mar 2021 16:29:00 +0100",
        ));
        f.extend(chunk(b"tEXt", b"Raw profile type exif\0junk"));
        f.extend(chunk(b"IEND", b""));
        let t = text_tags(&f);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].kind, "PNG");
        assert_eq!(
            t[0].props,
            [
                ("dc:title", "A tïtle".to_owned()),
                ("dc:creator", "Ana".to_owned()),
                ("xmp:CreateDate", "2021-03-24T16:29:00+01:00".to_owned()),
            ]
        );
        assert_eq!(t[0].fields.len(), 3);
        for n in 0..f.len() {
            let _ = text_tags(&f[..n]);
        }
    }

    #[test]
    fn gif_comment_and_xmp() {
        let mut f = b"GIF89a\x01\x00\x01\x00\x00\x00\x00".to_vec();
        f.extend(b"\x21\xFE\x05hello\x00");
        f.extend(b"\x21\xFF\x0bXMP DataXMP");
        let packet =
            b"<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?><x:xmpmeta/><?xpacket end='w'?>";
        f.extend(packet);
        // The magic trailer: 0x01, 0xFF down to 0x00, then the terminator.
        f.push(1);
        f.extend((0..=255u8).rev());
        f.push(0);
        f.extend(b"\x2C\0\0\0\0\x01\0\x01\0\0\x02\x02\x44\x01\x00\x3B");
        let t = text_tags(&f);
        assert_eq!(t[0].fields, [("Comment".to_owned(), "hello".to_owned())]);
        assert_eq!(gif_xmp(&f).as_deref(), Some(&packet[..]));
    }

    #[test]
    fn positions_and_dates() {
        assert_eq!(
            iso6709("+48.8584+002.2945+035.000/"),
            Some((48.8584, 2.2945, Some(35.0)))
        );
        let (lat, lon, alt) = iso6709("-3351.30+15112.50/").unwrap();
        assert!((lat + 33.855).abs() < 1e-9 && (lon - 151.208_333).abs() < 1e-6 && alt.is_none());
        assert_eq!(iso6709("nonsense"), None);
        assert_eq!(
            iso_date("2021-03-24T16:29:00+0100").as_deref(),
            Some("2021-03-24T16:29:00+01:00")
        );
        assert_eq!(
            iso_date("2021:03:24 16:29:00").as_deref(),
            Some("2021-03-24T16:29:00")
        );
        assert_eq!(iso_date("2021-03-24").as_deref(), Some("2021-03-24"));
        assert_eq!(
            iso_date("2024-01-02T03:04:05.678Z").as_deref(),
            Some("2024-01-02T03:04:05Z")
        );
        assert_eq!(iso_date("yesterday"), None);
        let t = movie_tags(
            "QuickTime",
            &[
                ("©xyz".to_owned(), "+48.8584+002.2945/".to_owned()),
                ("©nam".to_owned(), "Trip".to_owned()),
            ],
        );
        assert_eq!(t.props[0], ("exif:GPSLatitude", "48.8584000N".to_owned()));
        assert_eq!(t.props[1], ("exif:GPSLongitude", "2.2945000E".to_owned()));
        assert_eq!(t.props[2], ("dc:title", "Trip".to_owned()));
    }
}
