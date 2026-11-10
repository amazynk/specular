//! Rename patterns: `{name}`, `{n}` / `{n:03}` (also `{seq}`), `{date}` / `{date:%Y-%m-%d}`,
//! `{camera}`, `{rating}`, `{text}` (an export recipe's own words), `{w}`, `{h}` and `{ext}`,
//! with `{{` and `}}` for literal braces.

use std::fmt::Write as _;

/// One parsed piece of a pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Piece {
    Text(String),
    Name,
    /// Counter, zero-padded to this many digits.
    Index(usize),
    /// Capture date with a strftime-style format.
    Date(String),
    Width,
    Height,
    Ext,
    /// Camera model (Exif Model, with the make when the model doesn't start with it).
    Camera,
    /// Star rating, 0–5.
    Rating,
    /// The custom text given with the pattern.
    Custom,
}

/// Why a pattern does not parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternError {
    /// A `{` without its `}`.
    Unclosed,
    /// A `}` without its `{` (write `}}` for a literal brace).
    Stray,
    /// A token other than name, n, seq, date, camera, rating, text, w, h and ext.
    Unknown(String),
    /// A bad `{n:…}` width or `{date:…}` field.
    Format(String),
    /// Nothing but whitespace (or nothing at all).
    Empty,
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternError::Unclosed => write!(f, "a {{ is not closed"),
            PatternError::Stray => write!(f, "a }} has no {{"),
            PatternError::Unknown(t) => write!(f, "unknown token {{{t}}}"),
            PatternError::Format(t) => write!(f, "bad format in {{{t}}}"),
            PatternError::Empty => write!(f, "the pattern is empty"),
        }
    }
}

/// A parsed rename pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pieces: Vec<Piece>,
}

/// What a name is made from.
#[derive(Clone, Debug, Default)]
pub struct NameFacts<'a> {
    /// The source file name without its extension.
    pub stem: &'a str,
    /// 1-based position in the batch.
    pub index: usize,
    /// Capture date: year, month, day, hour, minute, second.
    pub date: Option<[u32; 6]>,
    /// Output size.
    pub width: u32,
    pub height: u32,
    /// Output extension, without the dot.
    pub ext: &'a str,
    /// Camera model, when known.
    pub camera: Option<&'a str>,
    /// Star rating 0–5, when known (rejected photos count as 0).
    pub rating: Option<u8>,
    /// What `{text}` becomes.
    pub text: &'a str,
}

impl Pattern {
    pub fn parse(s: &str) -> Result<Pattern, PatternError> {
        if s.trim().is_empty() {
            return Err(PatternError::Empty);
        }
        let mut pieces = Vec::new();
        let mut text = String::new();
        let mut it = s.chars().peekable();
        while let Some(c) = it.next() {
            match c {
                '{' if it.peek() == Some(&'{') => {
                    it.next();
                    text.push('{');
                }
                '}' if it.peek() == Some(&'}') => {
                    it.next();
                    text.push('}');
                }
                '}' => return Err(PatternError::Stray),
                '{' => {
                    let mut tok = String::new();
                    loop {
                        match it.next() {
                            Some('}') => break,
                            Some(c) => tok.push(c),
                            None => return Err(PatternError::Unclosed),
                        }
                    }
                    if !text.is_empty() {
                        pieces.push(Piece::Text(std::mem::take(&mut text)));
                    }
                    pieces.push(token(&tok)?);
                }
                c => text.push(c),
            }
        }
        if !text.is_empty() {
            pieces.push(Piece::Text(text));
        }
        Ok(Pattern { pieces })
    }

    /// Whether the pattern writes the extension itself (else `.ext` is appended).
    pub fn has_ext(&self) -> bool {
        self.pieces.contains(&Piece::Ext)
    }

    /// Whether the pattern needs the capture date.
    pub fn uses_date(&self) -> bool {
        self.pieces.iter().any(|p| matches!(p, Piece::Date(_)))
    }

    /// Whether the pattern needs the camera model.
    pub fn uses_camera(&self) -> bool {
        self.pieces.contains(&Piece::Camera)
    }

    /// Whether the pattern needs the output size.
    pub fn uses_rating(&self) -> bool {
        self.pieces.contains(&Piece::Rating)
    }

    pub fn uses_size(&self) -> bool {
        self.pieces
            .iter()
            .any(|p| matches!(p, Piece::Width | Piece::Height))
    }

    /// The file name for `f`: path separators and control characters become `_`, and
    /// `.ext` is appended unless the pattern has `{ext}`. A date token without a date gives
    /// "undated", a camera token without a model "unknown camera".
    pub fn render(&self, f: &NameFacts<'_>) -> String {
        let mut s = String::new();
        for p in &self.pieces {
            match p {
                Piece::Text(t) => s.push_str(t),
                Piece::Name => s.push_str(f.stem),
                Piece::Index(w) => {
                    let _ = write!(s, "{:0w$}", f.index, w = *w);
                }
                Piece::Date(fmt) => match f.date {
                    Some(d) => s.push_str(&strftime(fmt, d)),
                    None => s.push_str("undated"),
                },
                Piece::Width => {
                    let _ = write!(s, "{}", f.width);
                }
                Piece::Height => {
                    let _ = write!(s, "{}", f.height);
                }
                Piece::Ext => s.push_str(f.ext),
                Piece::Camera => s.push_str(
                    f.camera
                        .map(str::trim)
                        .filter(|c| !c.is_empty())
                        .unwrap_or("unknown camera"),
                ),
                Piece::Rating => {
                    let _ = write!(s, "{}", f.rating.unwrap_or(0).min(5));
                }
                Piece::Custom => s.push_str(f.text.trim()),
            }
        }
        if !self.has_ext() {
            s.push('.');
            s.push_str(f.ext);
        }
        sanitize(&s)
    }
}

fn token(tok: &str) -> Result<Piece, PatternError> {
    let (key, arg) = match tok.split_once(':') {
        Some((k, a)) => (k.trim(), Some(a)),
        None => (tok.trim(), None),
    };
    Ok(match (key, arg) {
        ("name", None) => Piece::Name,
        ("w", None) => Piece::Width,
        ("h", None) => Piece::Height,
        ("ext", None) => Piece::Ext,
        ("camera", None) => Piece::Camera,
        ("rating", None) => Piece::Rating,
        ("text", None) => Piece::Custom,
        ("n" | "seq", None) => Piece::Index(1),
        ("n" | "seq", Some(a)) => {
            let w = a
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|w| (1..=12).contains(w))
                .ok_or_else(|| PatternError::Format(tok.to_owned()))?;
            Piece::Index(w)
        }
        ("date", None) => Piece::Date("%Y-%m-%d".into()),
        ("date", Some(a)) => {
            let mut it = a.chars();
            while let Some(c) = it.next() {
                if c == '%'
                    && !matches!(
                        it.next(),
                        Some('Y' | 'y' | 'm' | 'd' | 'H' | 'M' | 'S' | '%')
                    )
                {
                    return Err(PatternError::Format(tok.to_owned()));
                }
            }
            Piece::Date(a.to_owned())
        }
        _ => return Err(PatternError::Unknown(tok.to_owned())),
    })
}

/// `%Y %y %m %d %H %M %S %%` of `[year, month, day, hour, minute, second]`.
fn strftime(fmt: &str, d: [u32; 6]) -> String {
    let mut s = String::new();
    let mut it = fmt.chars();
    while let Some(c) = it.next() {
        if c != '%' {
            s.push(c);
            continue;
        }
        let _ = match it.next() {
            Some('Y') => write!(s, "{:04}", d[0]),
            Some('y') => write!(s, "{:02}", d[0] % 100),
            Some('m') => write!(s, "{:02}", d[1]),
            Some('d') => write!(s, "{:02}", d[2]),
            Some('H') => write!(s, "{:02}", d[3]),
            Some('M') => write!(s, "{:02}", d[4]),
            Some('S') => write!(s, "{:02}", d[5]),
            Some(o) => {
                s.push(o);
                Ok(())
            }
            None => Ok(()),
        };
    }
    s
}

/// A file name: no path separators, NUL or control characters (each becomes `_`), no leading
/// or trailing whitespace, never "." or "..".
pub(crate) fn sanitize(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let t = t.trim().to_owned();
    if t.is_empty() || t == "." || t == ".." {
        "_".into()
    } else {
        t
    }
}

/// "YYYY:MM:DD HH:MM:SS" (Exif) to its fields; also accepts '-' as the date separator and a
/// missing time.
pub fn parse_exif_date(s: &str) -> Option<[u32; 6]> {
    let b = s.trim().as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let t = std::str::from_utf8(b.get(r)?).ok()?;
        t.parse().ok()
    };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if y == 0 || !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let (h, mi, se) = if b.len() >= 19 {
        (num(11..13)?, num(14..16)?, num(17..19)?)
    } else {
        (0, 0, 0)
    };
    Some([y, mo, d, h, mi, se])
}

/// Seconds since the Unix epoch to `[year, month, day, hour, minute, second]` (UTC).
pub fn civil_from_unix(secs: i64) -> [u32; 6] {
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400) as u32;
    // Days to a proleptic Gregorian date, counting 400-year eras from 0000-03-01.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = (yoe + era * 400 + i64::from(m <= 2)) as u32;
    [y, m, d, tod / 3600, tod / 60 % 60, tod % 60]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> NameFacts<'static> {
        NameFacts {
            stem: "IMG_0042",
            index: 7,
            date: Some([2024, 3, 9, 14, 5, 59]),
            width: 1920,
            height: 1280,
            ext: "jpg",
            camera: Some("Canon EOS R5"),
            rating: Some(4),
            text: " Lisbon trip ",
        }
    }

    /// The recipe tokens: rating, custom text and `seq` as another name for `n`.
    #[test]
    fn recipe_tokens_render() {
        let r = |p: &str| Pattern::parse(p).unwrap().render(&facts());
        assert_eq!(r("{text}_{seq:4}_{rating}"), "Lisbon trip_0007_4.jpg");
        assert_eq!(r("{seq}"), "7.jpg");
        let unrated = NameFacts {
            rating: None,
            text: "",
            ..facts()
        };
        assert_eq!(
            Pattern::parse("{name}-{rating}{text}")
                .unwrap()
                .render(&unrated),
            "IMG_0042-0.jpg"
        );
        assert!(Pattern::parse("{rating}").unwrap().uses_rating());
        assert!(!Pattern::parse("{name}").unwrap().uses_rating());
    }

    #[test]
    fn tokens_render() {
        let r = |p: &str| Pattern::parse(p).unwrap().render(&facts());
        assert_eq!(r("{name}"), "IMG_0042.jpg");
        assert_eq!(r("{n:03}-{name}"), "007-IMG_0042.jpg");
        assert_eq!(r("{n}"), "7.jpg");
        assert_eq!(r("{date}"), "2024-03-09.jpg");
        assert_eq!(r("{date:%Y%m%d_%H%M%S}"), "20240309_140559.jpg");
        assert_eq!(r("{date:%y.%d%%}"), "24.09%.jpg");
        assert_eq!(r("{name}_{w}x{h}"), "IMG_0042_1920x1280.jpg");
        assert_eq!(r("{name}.{ext}.bak"), "IMG_0042.jpg.bak");
        assert_eq!(r("{{{name}}}"), "{IMG_0042}.jpg");
        assert_eq!(r("{camera} {n:3}"), "Canon EOS R5 007.jpg");
        assert_eq!(
            Pattern::parse("{camera}").unwrap().render(&NameFacts {
                camera: None,
                ..facts()
            }),
            "unknown camera.jpg"
        );
        assert_eq!(r("a/b\\c"), "a_b_c.jpg");
        let undated = NameFacts {
            date: None,
            ..facts()
        };
        assert_eq!(
            Pattern::parse("{date}-{name}").unwrap().render(&undated),
            "undated-IMG_0042.jpg"
        );
    }

    #[test]
    fn errors() {
        assert_eq!(Pattern::parse(""), Err(PatternError::Empty));
        assert_eq!(Pattern::parse("  "), Err(PatternError::Empty));
        assert_eq!(Pattern::parse("{name"), Err(PatternError::Unclosed));
        assert_eq!(Pattern::parse("name}"), Err(PatternError::Stray));
        assert_eq!(
            Pattern::parse("{size}"),
            Err(PatternError::Unknown("size".into()))
        );
        assert!(matches!(
            Pattern::parse("{n:x}"),
            Err(PatternError::Format(_))
        ));
        assert!(matches!(
            Pattern::parse("{n:0}"),
            Err(PatternError::Format(_))
        ));
        assert!(matches!(
            Pattern::parse("{date:%Q}"),
            Err(PatternError::Format(_))
        ));
        assert!(Pattern::parse("{date:%Y}").unwrap().uses_date());
        assert!(!Pattern::parse("{name}").unwrap().uses_date());
    }

    #[test]
    fn dates() {
        assert_eq!(
            parse_exif_date("2024:03:09 14:05:59"),
            Some([2024, 3, 9, 14, 5, 59])
        );
        assert_eq!(parse_exif_date("2024-03-09"), Some([2024, 3, 9, 0, 0, 0]));
        assert_eq!(parse_exif_date("0000:00:00 00:00:00"), None);
        assert_eq!(parse_exif_date("    "), None);
        assert_eq!(civil_from_unix(0), [1970, 1, 1, 0, 0, 0]);
        assert_eq!(civil_from_unix(951_782_400), [2000, 2, 29, 0, 0, 0]);
        assert_eq!(civil_from_unix(1_710_000_000), [2024, 3, 9, 16, 0, 0]);
        assert_eq!(civil_from_unix(-1), [1969, 12, 31, 23, 59, 59]);
    }
}
