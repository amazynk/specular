//! Destination folders and new names.
//!
//! Folder patterns are `/`-separated with `{year}`, `{month}`, `{day}`, `{camera}` and `{event}`
//! (`{{` and `}}` for braces). A folder whose tokens all came out empty is dropped
//! (`{year}/{event}` without an event is just the year), and separators left dangling by an
//! empty token are trimmed (`{year}-{month} {event}` without an event is `2024-03`).
//!
//! Rename patterns are the batch-convert ones (`{name}`, `{n:03}`, `{date:%Y%m%d}`, `{ext}`)
//! plus `{camera}` and `{event}`; the output size tokens mean nothing here and are refused.

use std::path::PathBuf;

use specular_convert::{NameFacts, Pattern};

/// Why a pattern does not parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternError {
    /// A `{` without its `}`.
    Unclosed,
    /// A `}` without its `{`.
    Stray,
    /// A token the pattern does not know.
    Unknown(String),
    /// A part of the rename pattern that does not parse (the convert pattern's message).
    Rename(String),
    /// Nothing at all.
    Empty,
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternError::Unclosed => write!(f, "a {{ is not closed"),
            PatternError::Stray => write!(f, "a }} has no {{"),
            PatternError::Unknown(t) => write!(f, "unknown token {{{t}}}"),
            PatternError::Rename(e) => write!(f, "{e}"),
            PatternError::Empty => write!(f, "the pattern is empty"),
        }
    }
}

impl std::error::Error for PatternError {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tok {
    Text(String),
    Year,
    Month,
    Day,
    Camera,
    Event,
}

/// A parsed folder pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderPattern {
    segments: Vec<Vec<Tok>>,
}

/// What a folder or a name is made from.
#[derive(Clone, Copy, Debug)]
pub struct Facts<'a> {
    /// Capture date (local wall clock): year, month, day, hour, minute, second.
    pub date: [u32; 6],
    /// Camera name (empty when unknown).
    pub camera: &'a str,
    /// The event typed for this import (may be empty).
    pub event: &'a str,
}

/// `s` made safe as one path segment: no separators, control or reserved characters, no
/// leading dots or trailing spaces or dots (Windows), at most 120 bytes.
pub fn sanitize(s: &str) -> String {
    let mut o: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    o = o
        .trim()
        .trim_start_matches('.')
        .trim_end_matches(['.', ' '])
        .to_owned();
    if o.len() > 120 {
        let mut cut = 120;
        while !o.is_char_boundary(cut) {
            cut -= 1;
        }
        o.truncate(cut);
    }
    o
}

impl FolderPattern {
    pub fn parse(s: &str) -> Result<FolderPattern, PatternError> {
        let mut segments = Vec::new();
        let mut seg: Vec<Tok> = Vec::new();
        let mut text = String::new();
        let mut it = s.chars().peekable();
        let flush = |text: &mut String, seg: &mut Vec<Tok>| {
            if !text.is_empty() {
                seg.push(Tok::Text(std::mem::take(text)));
            }
        };
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
                    let mut name = String::new();
                    loop {
                        match it.next() {
                            Some('}') => break,
                            Some(c) => name.push(c),
                            None => return Err(PatternError::Unclosed),
                        }
                    }
                    flush(&mut text, &mut seg);
                    seg.push(match name.trim() {
                        "year" => Tok::Year,
                        "month" => Tok::Month,
                        "day" => Tok::Day,
                        "camera" => Tok::Camera,
                        "event" => Tok::Event,
                        _ => return Err(PatternError::Unknown(name)),
                    });
                }
                '/' | '\\' => {
                    flush(&mut text, &mut seg);
                    if !seg.is_empty() {
                        segments.push(std::mem::take(&mut seg));
                    }
                }
                c => text.push(c),
            }
        }
        flush(&mut text, &mut seg);
        if !seg.is_empty() {
            segments.push(seg);
        }
        Ok(FolderPattern { segments })
    }

    /// The folder (relative to the destination root) for `f`. Empty when every segment came
    /// out empty (the files go in the root).
    pub fn render(&self, f: &Facts<'_>) -> PathBuf {
        let mut out = PathBuf::new();
        for seg in &self.segments {
            let mut s = String::new();
            let mut tokens = 0;
            let mut empty = 0;
            for t in seg {
                let v = match t {
                    Tok::Text(t) => {
                        s.push_str(t);
                        continue;
                    }
                    Tok::Year => format!("{:04}", f.date[0]),
                    Tok::Month => format!("{:02}", f.date[1]),
                    Tok::Day => format!("{:02}", f.date[2]),
                    Tok::Camera => f.camera.trim().to_owned(),
                    Tok::Event => f.event.trim().to_owned(),
                };
                tokens += 1;
                if v.is_empty() {
                    empty += 1;
                }
                s.push_str(&v);
            }
            if tokens > 0 && empty == tokens {
                continue;
            }
            if empty > 0 {
                s = s.trim_matches([' ', '-', '_', '.', ',']).to_owned();
            }
            let s = sanitize(&s);
            if !s.is_empty() && s != ".." {
                out.push(s);
            }
        }
        out
    }
}

/// A rename pattern: the batch-convert pattern with `{camera}` and `{event}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenamePattern {
    source: String,
}

/// `s` with `{camera}` and `{event}` replaced by these values (braces in them doubled).
fn substitute(s: &str, camera: &str, event: &str) -> Result<String, PatternError> {
    let esc = |v: &str| sanitize(v).replace('{', "{{").replace('}', "}}");
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                o.push_str("{{");
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match it.next() {
                        Some('}') => break,
                        Some(c) => name.push(c),
                        None => return Err(PatternError::Unclosed),
                    }
                }
                match name.trim() {
                    "camera" => o.push_str(&esc(camera)),
                    "event" => o.push_str(&esc(event)),
                    _ => {
                        o.push('{');
                        o.push_str(&name);
                        o.push('}');
                    }
                }
            }
            c => o.push(c),
        }
    }
    Ok(o)
}

impl RenamePattern {
    /// `None` for an empty pattern (keep the camera's names).
    pub fn parse(s: &str) -> Result<Option<RenamePattern>, PatternError> {
        if s.trim().is_empty() {
            return Ok(None);
        }
        let p = Pattern::parse(&substitute(s, "x", "x")?)
            .map_err(|e| PatternError::Rename(e.to_string()))?;
        if p.uses_size() {
            return Err(PatternError::Unknown(
                if s.contains("{w") { "w" } else { "h" }.into(),
            ));
        }
        Ok(Some(RenamePattern {
            source: s.to_owned(),
        }))
    }

    /// The new file name for a file whose name was `stem` + `.ext`, the `index`-th (1-based)
    /// shot of the import. Falls back to the old name if the result is empty.
    pub fn render(&self, stem: &str, ext: &str, index: usize, f: &Facts<'_>) -> String {
        let fallback = || {
            if ext.is_empty() {
                stem.to_owned()
            } else {
                format!("{stem}.{ext}")
            }
        };
        let Ok(p) = substitute(&self.source, f.camera, f.event)
            .map_err(|_| ())
            .and_then(|s| Pattern::parse(&s).map_err(|_| ()))
        else {
            return fallback();
        };
        let name = p.render(&NameFacts {
            stem,
            index,
            date: Some(f.date),
            width: 0,
            height: 0,
            ext,
            camera: Some(f.camera),
            ..Default::default()
        });
        let name = sanitize(&name);
        if name.is_empty() || name.starts_with('.') {
            fallback()
        } else {
            name
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: Facts<'static> = Facts {
        date: [2024, 3, 9, 14, 5, 59],
        camera: "Canon EOS R5",
        event: "",
    };

    fn folder(p: &str, f: &Facts<'_>) -> String {
        FolderPattern::parse(p)
            .unwrap()
            .render(f)
            .to_string_lossy()
            .replace('\\', "/")
    }

    #[test]
    fn folders() {
        assert_eq!(folder("{year}/{year}-{month}-{day}", &F), "2024/2024-03-09");
        assert_eq!(folder("{camera}/{year}", &F), "Canon EOS R5/2024");
        let ev = Facts {
            event: "Anna's wedding",
            ..F
        };
        assert_eq!(
            folder("{year}/{year}-{month}-{day} {event}", &ev),
            "2024/2024-03-09 Anna's wedding"
        );
        // Without an event: the dangling separator goes, an all-empty folder is dropped.
        assert_eq!(
            folder("{year}/{year}-{month}-{day} {event}", &F),
            "2024/2024-03-09"
        );
        assert_eq!(folder("{year}/{event}", &F), "2024");
        assert_eq!(folder("{camera}/{day}", &Facts { camera: "", ..F }), "09");
        // Unsafe values cannot climb out or add folders.
        let bad = Facts {
            event: "../../etc/x:y",
            ..F
        };
        assert_eq!(folder("{event}", &bad), "-..-etc-x-y");
        assert_eq!(folder("/{year}//x/", &F), "2024/x");
        assert_eq!(folder("{{raw}}", &F), "{raw}");
        assert_eq!(folder("", &F), "");
    }

    #[test]
    fn folder_errors() {
        assert_eq!(
            FolderPattern::parse("{year}/{hour}"),
            Err(PatternError::Unknown("hour".into()))
        );
        assert_eq!(FolderPattern::parse("{year"), Err(PatternError::Unclosed));
        assert_eq!(FolderPattern::parse("a}"), Err(PatternError::Stray));
    }

    #[test]
    fn renames() {
        assert_eq!(RenamePattern::parse("  ").unwrap(), None);
        let p = RenamePattern::parse("{date:%Y%m%d}-{n:04}")
            .unwrap()
            .unwrap();
        assert_eq!(p.render("IMG_0001", "CR2", 7, &F), "20240309-0007.CR2");
        let p = RenamePattern::parse("{camera} {event} {name}")
            .unwrap()
            .unwrap();
        let ev = Facts { event: "Trip", ..F };
        assert_eq!(
            p.render("IMG_0001", "JPG", 1, &ev),
            "Canon EOS R5 Trip IMG_0001.JPG"
        );
        // Braces in values stay literal.
        let ev = Facts { event: "{n}", ..F };
        assert_eq!(p.render("A", "jpg", 3, &ev), "Canon EOS R5 {n} A.jpg");
        assert!(matches!(
            RenamePattern::parse("{w}x{h}"),
            Err(PatternError::Unknown(_))
        ));
        assert!(matches!(
            RenamePattern::parse("{bogus}"),
            Err(PatternError::Rename(_))
        ));
    }
}
