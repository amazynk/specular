//! `file:` addresses and percent-encoding, as playlists write them.

use std::path::{Path, PathBuf};

/// Whether `s` starts with a URI scheme and `:` (`http:`, `file:`), not a drive letter.
pub(crate) fn has_scheme(s: &str) -> bool {
    let Some(colon) = s.find(':') else {
        return false;
    };
    let scheme = &s[..colon];
    scheme.len() >= 2
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
}

/// The path a `file:` address names (`file:///home/a/b%20c.mp3`, `file://localhost/…`,
/// `file:///C:/Music/a.mp3`); `None` for anything else.
pub fn path_from_file_uri(s: &str) -> Option<PathBuf> {
    let rest = s
        .get(..5)
        .filter(|p| p.eq_ignore_ascii_case("file:"))
        .map(|_| &s[5..])?;
    let rest = match rest.strip_prefix("//") {
        Some(r) => {
            // An authority: empty or `localhost` (another host's file is not ours).
            let slash = r.find('/').unwrap_or(r.len());
            let host = &r[..slash];
            if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
                return None;
            }
            &r[slash..]
        }
        None => rest,
    };
    let bytes = percent_decode(rest);
    // `/C:/Music` on Windows is `C:/Music`.
    let bytes = if bytes.len() >= 3
        && bytes[0] == b'/'
        && bytes[1].is_ascii_alphabetic()
        && bytes[2] == b':'
    {
        bytes[1..].to_vec()
    } else {
        bytes
    };
    Some(path_from_bytes(bytes))
}

/// `path` as a `file://` address, percent-encoded.
pub fn file_uri(path: &Path) -> String {
    let bytes = path_bytes(path);
    let mut s = String::from("file://");
    if !bytes.starts_with(b"/") {
        s.push('/');
    }
    s.push_str(&percent_encode(&bytes));
    s
}

/// Percent-encodes everything but unreserved characters and `/` (and `:` for drive letters).
pub(crate) fn percent_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len());
    for &b in bytes {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/' | b':') {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

/// `%xx` sequences as bytes (a stray `%` stays).
pub(crate) fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push(h << 4 | l);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(unix)]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().replace('\\', "/").into_bytes()
}
