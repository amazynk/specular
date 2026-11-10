//! The `specular://` URL scheme (DESIGN.md §11, "URL scheme"): what a link may ask for, and
//! how far it is trusted. Only the standard library is used here, so the fuzz target
//! (`crates/specular-platform/fuzz`) can build this file on its own.
//!
//! Accepted forms (the scheme name is case-insensitive, everything else is exact):
//!
//! * `specular:///absolute/path` and `specular://localhost/absolute/path`: one local file or
//!   folder (on Windows `specular:///C:/Photos/a.jpg`).
//! * `specular://open?path=<pct-encoded absolute path>&path=…&uri=<pct-encoded file:// URL>`:
//!   one or more local files (at most [`MAX_PATHS`]).
//! * `specular://open?url=…`, `specular://study?…`, `specular://manifest?…`,
//!   `specular://dicomweb?…` and any other host name: a **remote** source (DESIGN.md §7.5).
//!   There is no remote [`Source`](../../specular_core/source/index.html) yet, so the viewer
//!   refuses these and says so.
//!
//! Rules: at most [`MAX_URL`] bytes; only printable ASCII (no spaces or control characters:
//! a space is `%20`, and `+` is a literal plus); every `%` is followed by two hex digits; the
//! decoded text is UTF-8 without NUL; paths are absolute; unknown parameters, empty values and
//! a `#fragment` are errors rather than ignored.
//!
//! A local request that is not *plainly* local gets [`Target::Confirm`]: `.` or `..` path
//! components, Windows UNC and device paths (`\\server\share`, `\\?\…`), and control or
//! bidirectional-override characters that could make the name shown differ from the file
//! opened. The viewer asks before opening those.

use std::fmt;
use std::path::PathBuf;

/// The scheme, without the colon.
pub const SCHEME: &str = "specular";

/// The longest URL accepted, in bytes.
pub const MAX_URL: usize = 8 * 1024;

/// The most paths one URL may name.
pub const MAX_PATHS: usize = 1000;

/// What a valid `specular:` URL asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Plainly local files or folders: opened directly.
    Local(Vec<PathBuf>),
    /// Local, but unusual enough to ask first.
    Confirm { paths: Vec<PathBuf>, reason: Reason },
    /// A non-local source (a server, a study, a manifest): never opened without a remote
    /// source, and only after confirmation once one exists. `source` is safe to show.
    Remote { source: String },
}

/// Why a local request needs confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// `.` or `..` components: the path shown is not the path opened.
    DotSegments,
    /// A Windows network share or device path.
    NetworkOrDevice,
    /// Control, invisible or bidirectional-override characters in a name.
    HiddenCharacters,
}

/// Why a URL was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotSpecular,
    TooLong,
    /// A byte outside printable ASCII (the offset).
    BadCharacter(usize),
    /// A `%` not followed by two hex digits (the offset).
    BadEscape(usize),
    NotUtf8,
    Nul,
    Fragment,
    UnknownAction(String),
    UnknownParameter(String),
    EmptyValue(String),
    NotAbsolute,
    BadFileUri,
    NoPaths,
    TooManyPaths,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotSpecular => write!(f, "not a {SCHEME}: URL"),
            Error::TooLong => write!(f, "longer than {MAX_URL} bytes"),
            Error::BadCharacter(i) => write!(f, "character at byte {i} must be percent-encoded"),
            Error::BadEscape(i) => write!(f, "bad percent escape at byte {i}"),
            Error::NotUtf8 => write!(f, "decoded text is not UTF-8"),
            Error::Nul => write!(f, "decoded text contains NUL"),
            Error::Fragment => write!(f, "fragments (#…) are not allowed"),
            Error::UnknownAction(a) => write!(f, "unknown action {:?}", escape_for_display(a)),
            Error::UnknownParameter(p) => {
                write!(f, "unknown parameter {:?}", escape_for_display(p))
            }
            Error::EmptyValue(p) => write!(f, "parameter {p:?} has no value"),
            Error::NotAbsolute => write!(f, "paths must be absolute"),
            Error::BadFileUri => write!(f, "uri= must be a file:// URL"),
            Error::NoPaths => write!(f, "no path given"),
            Error::TooManyPaths => write!(f, "more than {MAX_PATHS} paths"),
        }
    }
}

impl std::error::Error for Error {}

/// Whether `s` looks like a `specular:` URL (to tell it from a file name on the command line).
pub fn is_url(s: &str) -> bool {
    s.len() > SCHEME.len()
        && s.as_bytes()[SCHEME.len()] == b':'
        && s[..SCHEME.len()].eq_ignore_ascii_case(SCHEME)
}

/// Hosts that name remote sources (DESIGN.md §7.5).
const REMOTE_ACTIONS: &[&str] = &["study", "manifest", "dicomweb"];

/// Parses and classifies a `specular:` URL.
pub fn parse(url: &str) -> Result<Target, Error> {
    if !is_url(url) {
        return Err(Error::NotSpecular);
    }
    if url.len() > MAX_URL {
        return Err(Error::TooLong);
    }
    if let Some(i) = url.bytes().position(|b| !(0x21..=0x7e).contains(&b)) {
        return Err(Error::BadCharacter(i));
    }
    let rest = &url[SCHEME.len() + 1..];
    if rest.contains('#') {
        return Err(Error::Fragment);
    }
    let Some(rest) = rest.strip_prefix("//") else {
        return Err(Error::UnknownAction(String::new()));
    };
    let (authority, after) = match rest.find(['/', '?']) {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    let base = SCHEME.len() + 3 + authority.len();
    let host = decode(authority, SCHEME.len() + 3)?;
    let (path, query) = match after.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (after, None),
    };
    let lower = host.to_ascii_lowercase();
    if lower.is_empty() || lower == "localhost" {
        // specular:///abs/path
        if query.is_some() {
            return Err(Error::UnknownParameter("?".into()));
        }
        if path.is_empty() {
            return Err(Error::NoPaths);
        }
        let p = decode(path, base)?;
        return classify(vec![local_path(&p)?]);
    }
    if REMOTE_ACTIONS.contains(&lower.as_str()) {
        return Ok(Target::Remote {
            source: format!("{lower} request"),
        });
    }
    if lower != "open" {
        // Any other host: a server somewhere.
        return Ok(Target::Remote {
            source: escape_for_display(&host),
        });
    }
    if !path.is_empty() && path != "/" {
        return Err(Error::UnknownAction(escape_for_display(&decode(
            path, base,
        )?)));
    }
    let Some(query) = query else {
        return Err(Error::NoPaths);
    };
    let mut paths = Vec::new();
    let mut offset = base + path.len() + 1;
    for pair in query.split('&') {
        let at = offset;
        offset += pair.len() + 1;
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let key = decode(k, at)?;
        let value = decode(v, at + k.len() + 1)?;
        match key.as_str() {
            "path" | "uri" | "url" if value.is_empty() => return Err(Error::EmptyValue(key)),
            "path" => paths.push(local_path(&value)?),
            "uri" => paths.push(file_uri(&value)?),
            "url" => {
                return Ok(Target::Remote {
                    source: remote_display(&value),
                });
            }
            _ => return Err(Error::UnknownParameter(escape_for_display(&key))),
        }
        if paths.len() > MAX_PATHS {
            return Err(Error::TooManyPaths);
        }
    }
    if paths.is_empty() {
        return Err(Error::NoPaths);
    }
    classify(paths)
}

/// A remote URL shortened to what is safe and useful to show: its scheme and host.
fn remote_display(url: &str) -> String {
    let host = url
        .split_once("://")
        .map(|(s, r)| {
            let h = r.split(['/', '?', '#']).next().unwrap_or("");
            // Never show credentials.
            let h = h.rsplit('@').next().unwrap_or(h);
            format!("{s}://{h}")
        })
        .unwrap_or_else(|| "?".into());
    escape_for_display(&host.chars().take(200).collect::<String>())
}

/// Strict percent-decoding: `%XX` only, UTF-8 result, no NUL. `at` is the byte offset of `s`
/// in the URL, for error messages.
fn decode(s: &str, at: usize) -> Result<String, Error> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = |c: u8| (c as char).to_digit(16);
            match (
                b.get(i + 1).and_then(|&c| hex(c)),
                b.get(i + 2).and_then(|&c| hex(c)),
            ) {
                (Some(h), Some(l)) => out.push((h * 16 + l) as u8),
                _ => return Err(Error::BadEscape(at + i)),
            }
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    if out.contains(&0) {
        return Err(Error::Nul);
    }
    String::from_utf8(out).map_err(|_| Error::NotUtf8)
}

/// A decoded path from the URL as a native absolute path.
fn local_path(p: &str) -> Result<PathBuf, Error> {
    #[cfg(windows)]
    {
        // "/C:/x" and "C:/x" are drive paths; "//server/share" is UNC.
        let s = p.strip_prefix('/').filter(|r| is_drive(r)).unwrap_or(p);
        let s = s.replace('/', "\\");
        let path = PathBuf::from(&s);
        if !path.is_absolute() {
            return Err(Error::NotAbsolute);
        }
        Ok(path)
    }
    #[cfg(not(windows))]
    {
        if !p.starts_with('/') {
            return Err(Error::NotAbsolute);
        }
        Ok(PathBuf::from(p))
    }
}

#[cfg(windows)]
fn is_drive(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\')
}

/// A `file://` URL (already decoded once from the query) to a path: local host only.
fn file_uri(u: &str) -> Result<PathBuf, Error> {
    if u.len() < 7 || !u[..7].eq_ignore_ascii_case("file://") {
        return Err(Error::BadFileUri);
    }
    let rest = &u[7..];
    let (host, path) = match rest.find('/') {
        Some(i) => rest.split_at(i),
        None => return Err(Error::BadFileUri),
    };
    if !(host.is_empty() || host.eq_ignore_ascii_case("localhost")) {
        return Err(Error::BadFileUri);
    }
    if path.contains(['?', '#']) || path.bytes().any(|b| !(0x21..=0x7e).contains(&b)) {
        return Err(Error::BadFileUri);
    }
    local_path(&decode(path, 0).map_err(|_| Error::BadFileUri)?)
}

/// Local paths: plainly local, or needing confirmation.
fn classify(paths: Vec<PathBuf>) -> Result<Target, Error> {
    let mut reason = None;
    for p in &paths {
        if let Some(r) = suspicious(p) {
            reason = Some(r);
            break;
        }
    }
    Ok(match reason {
        None => Target::Local(paths),
        Some(reason) => Target::Confirm { paths, reason },
    })
}

fn suspicious(p: &std::path::Path) -> Option<Reason> {
    use std::path::Component;
    let s = p.to_string_lossy();
    if s.chars().any(hidden_char) {
        return Some(Reason::HiddenCharacters);
    }
    #[cfg(windows)]
    if s.starts_with("\\\\") {
        return Some(Reason::NetworkOrDevice);
    }
    // `components()` drops inner "." and doubled separators, so look at the text too.
    let sep = std::path::MAIN_SEPARATOR;
    if p.components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || s.split(sep).any(|c| c == "." || c == "..")
    {
        return Some(Reason::DotSegments);
    }
    None
}

/// Characters that hide or reorder what a name looks like.
fn hidden_char(c: char) -> bool {
    c.is_control()
        || matches!(c,
            '\u{200B}'..='\u{200F}' // zero-width and LRM/RLM
            | '\u{202A}'..='\u{202E}' // embeddings and overrides
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}' // isolates
            | '\u{061C}' | '\u{FEFF}' | '\u{00AD}')
}

/// `s` with control, invisible and bidirectional characters written as `\u{…}`, so that what
/// a dialog or a log shows is what is there.
pub fn escape_for_display(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if hidden_char(c) {
            out.push_str(&format!("\\u{{{:x}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

/// `specular:///…` for a local absolute path: the inverse of [`parse`] for one path.
pub fn for_path(p: &std::path::Path) -> Option<String> {
    let s = p.to_str()?;
    if !p.is_absolute() {
        return None;
    }
    let s = s.replace('\\', "/");
    let mut out = format!("{SCHEME}://");
    if !s.starts_with('/') {
        out.push('/');
    }
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~:".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    Some(out)
}

/// The fuzz target's check, kept next to the parser.
#[doc(hidden)]
pub fn fuzz_one(data: &[u8]) {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(Target::Local(paths)) = parse(s) {
            assert!(paths.iter().all(|p| p.is_absolute()));
        }
        let _ = escape_for_display(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[cfg(unix)]
    #[test]
    fn local_forms() {
        assert_eq!(
            parse("specular:///home/u/a%20b.jpg").unwrap(),
            Target::Local(vec![p("/home/u/a b.jpg")])
        );
        assert_eq!(
            parse("SPECULAR://localhost/x+y.png").unwrap(),
            Target::Local(vec![p("/x+y.png")])
        );
        assert_eq!(
            parse("specular://open?path=%2Fa.jpg&uri=file%3A%2F%2F%2Fb%2520c.jpg&path=/d").unwrap(),
            Target::Local(vec![p("/a.jpg"), p("/b c.jpg"), p("/d")])
        );
        assert_eq!(
            parse("specular://open/?uri=file://localhost/e.jpg").unwrap(),
            Target::Local(vec![p("/e.jpg")])
        );
        assert_eq!(
            parse("specular:///%C3%A9t%C3%A9.jpg").unwrap(),
            Target::Local(vec![p("/été.jpg")])
        );
        let round = for_path(std::path::Path::new("/a b/ü#?%.jpg")).unwrap();
        assert_eq!(
            parse(&round).unwrap(),
            Target::Local(vec![p("/a b/ü#?%.jpg")])
        );
    }

    #[cfg(unix)]
    #[test]
    fn confirm_forms() {
        assert_eq!(
            parse("specular:///home/u/../../etc/x").unwrap(),
            Target::Confirm {
                paths: vec![p("/home/u/../../etc/x")],
                reason: Reason::DotSegments
            }
        );
        assert!(matches!(
            parse("specular:///a/./b").unwrap(),
            Target::Confirm {
                reason: Reason::DotSegments,
                ..
            }
        ));
        // RIGHT-TO-LEFT OVERRIDE: "gpj.exe" shown as "exe.jpg".
        assert!(matches!(
            parse("specular:///x/%E2%80%AEgpj.exe").unwrap(),
            Target::Confirm {
                reason: Reason::HiddenCharacters,
                ..
            }
        ));
        assert!(matches!(
            parse("specular:///x/a%0Ab").unwrap(),
            Target::Confirm {
                reason: Reason::HiddenCharacters,
                ..
            }
        ));
        assert_eq!(escape_for_display("a\u{202E}b\n"), "a\\u{202e}b\\u{a}");
    }

    #[test]
    fn remote_forms() {
        assert_eq!(
            parse("specular://open?url=https%3A%2F%2Fuser%3Apw%40pacs.example%2Fwado%3Ftoken%3Dx")
                .unwrap(),
            Target::Remote {
                source: "https://pacs.example".into()
            }
        );
        assert!(matches!(
            parse("specular://study?uid=1.2.3&token=abc").unwrap(),
            Target::Remote { .. }
        ));
        assert!(matches!(
            parse("specular://example.com/a.jpg").unwrap(),
            Target::Remote { .. }
        ));
    }

    #[test]
    fn rejected() {
        use Error::*;
        let bad: &[(&str, Error)] = &[
            ("file:///a", NotSpecular),
            ("specular:/a", UnknownAction(String::new())),
            ("specular:///a b", BadCharacter(13)),
            ("specular:///a\u{e9}", BadCharacter(13)),
            ("specular:///a%2", BadEscape(13)),
            ("specular:///a%zz", BadEscape(13)),
            ("specular:///a%00b", Nul),
            ("specular:///a%FF", NotUtf8),
            ("specular:///a#frag", Fragment),
            ("specular://open?path=relative/x", NotAbsolute),
            ("specular://", NoPaths),
            ("specular://open?evil=1", UnknownParameter("evil".into())),
            ("specular://open?path=", EmptyValue("path".into())),
            ("specular://open", NoPaths),
            ("specular://open?uri=http%3A%2F%2Fx%2Fa", BadFileUri),
            ("specular://open?uri=file%3A%2F%2Fhost%2Fa", BadFileUri),
            ("specular://open/x?path=/a", UnknownAction("/x".into())),
            ("specular://?path=/a", UnknownParameter("?".into())),
        ];
        for (u, e) in bad {
            assert_eq!(parse(u).as_ref(), Err(e), "{u}");
        }
        let long = format!("specular:///{}", "a".repeat(MAX_URL));
        assert_eq!(parse(&long), Err(TooLong));
        // More than MAX_PATHS absolute paths. On Windows even the shortest (`C:/`) makes that
        // longer than MAX_URL, so there the length limit is what refuses it.
        let one = if cfg!(windows) {
            "path=C:/&"
        } else {
            "path=/a&"
        };
        let many = format!("specular://open?{}", one.repeat(MAX_PATHS + 1));
        let want = if cfg!(windows) { TooLong } else { TooManyPaths };
        assert_eq!(parse(&many), Err(want));
    }

    /// Random and mutated inputs never panic, and whatever parses keeps the invariants.
    #[test]
    fn random_inputs() {
        let seeds = [
            "specular:///home/u/a%20b.jpg",
            "specular://open?path=%2Fa&uri=file%3A%2F%2F%2Fb&url=https%3A%2F%2Fh",
            "specular://study?uid=1",
            "specular:///x/%E2%80%AE..%2F",
        ];
        let alphabet = b"%/?&=#:.0123456789abcdefABCDEF specular\x00\xff\\@+";
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for i in 0..50_000 {
            let mut b = seeds[i % seeds.len()].as_bytes().to_vec();
            for _ in 0..(next() % 8) {
                let pos = (next() as usize) % (b.len() + 1);
                let c = alphabet[(next() as usize) % alphabet.len()];
                match next() % 3 {
                    0 => b.insert(pos, c),
                    1 if pos < b.len() => b[pos] = c,
                    _ if pos < b.len() => {
                        b.remove(pos);
                    }
                    _ => {}
                }
            }
            check(&String::from_utf8_lossy(&b));
        }
    }

    pub(super) fn check(s: &str) {
        match parse(s) {
            Ok(Target::Local(paths)) => {
                assert!(!paths.is_empty() && paths.len() <= MAX_PATHS);
                for p in &paths {
                    assert!(p.is_absolute(), "{s}: {p:?}");
                    assert!(suspicious(p).is_none(), "{s}");
                }
            }
            Ok(Target::Confirm { paths, .. }) => {
                assert!(paths.iter().all(|p| p.is_absolute()));
            }
            Ok(Target::Remote { source }) => {
                assert!(!source.chars().any(hidden_char));
            }
            Err(e) => {
                let _ = e.to_string();
            }
        }
    }
}
