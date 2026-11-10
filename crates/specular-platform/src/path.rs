//! Canonical paths that read like the user's.
//!
//! On Windows `std::fs::canonicalize` returns verbatim paths (`\\?\D:\photos`,
//! `\\?\UNC\server\share`). They work for opening files, but never compare equal to the paths
//! the user, the shell and the file dialogs use (`D:\photos`), and show the prefix wherever a
//! path is displayed. [`canonicalize`] drops the prefix whenever the plain form names the same
//! file; elsewhere it is `std::fs::canonicalize`.

use std::io;
use std::path::{Path, PathBuf};

/// `std::fs::canonicalize`, without Windows's `\\?\` prefix where the path does not need it.
pub fn canonicalize(p: &Path) -> io::Result<PathBuf> {
    let c = std::fs::canonicalize(p)?;
    #[cfg(windows)]
    let c = simplified(c);
    Ok(c)
}

/// The plain form of a verbatim path (`\\?\D:\a` → `D:\a`, `\\?\UNC\s\h\a` → `\\s\h\a`) when
/// Windows reads it as the same file: under the 260-character limit, and with no component it
/// would read differently without `\\?\` (a device name such as `NUL`, a trailing dot or
/// space). Anything else is returned as it is.
#[cfg(windows)]
fn simplified(c: PathBuf) -> PathBuf {
    let Some(s) = c.to_str() else {
        return c;
    };
    let plain = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        // A drive: `X:\` and nothing like `\\?\Volume{…}` or `\\?\GLOBALROOT`.
        let b = rest.as_bytes();
        if !(b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\') {
            return c;
        }
        rest.to_owned()
    } else {
        return c;
    };
    // MAX_PATH (260) counts the terminating NUL.
    if plain.encode_utf16().count() >= 260 {
        return c;
    }
    if plain.split('\\').skip(1).any(|n| !plain_name(n)) {
        return c;
    }
    PathBuf::from(plain)
}

/// Whether a component means the same without `\\?\`: not a device name (with or without an
/// extension), no trailing dot or space, nothing the Win32 path parser would reinterpret.
#[cfg(windows)]
fn plain_name(n: &str) -> bool {
    if n.is_empty() {
        return true; // a trailing separator (`D:\`)
    }
    if n.ends_with(['.', ' ']) || n.contains(['/', '<', '>', '"', '|', '?', '*', ':']) {
        return false;
    }
    let stem = n.split('.').next().unwrap_or(n).trim_end_matches(' ');
    let device = |p: &str| {
        stem.len() == 4
            && stem.get(..3).is_some_and(|h| h.eq_ignore_ascii_case(p))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0'
    };
    let reserved = ["CON", "PRN", "AUX", "NUL"]
        .iter()
        .any(|r| stem.eq_ignore_ascii_case(r))
        || device("COM")
        || device("LPT");
    !reserved
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn s(p: &str) -> String {
        simplified(PathBuf::from(p)).to_string_lossy().into_owned()
    }

    #[test]
    fn drops_the_prefix_where_the_plain_path_is_the_same() {
        assert_eq!(s(r"\\?\D:\photos\a.jpg"), r"D:\photos\a.jpg");
        assert_eq!(s(r"\\?\C:\"), r"C:\");
        assert_eq!(s(r"\\?\UNC\server\share\a.jpg"), r"\\server\share\a.jpg");
        // Not verbatim: unchanged.
        assert_eq!(s(r"D:\photos"), r"D:\photos");
    }

    #[test]
    fn keeps_it_where_the_plain_path_would_differ() {
        for p in [
            r"\\?\D:\photos\NUL",
            r"\\?\D:\photos\con.txt",
            r"\\?\D:\photos\COM1",
            r"\\?\D:\photos\dot.",
            r"\\?\D:\photos\space ",
            r"\\?\Volume{0b1c2d3e-0000-0000-0000-000000000000}\a",
            r"\\?\GLOBALROOT\Device\HarddiskVolume1\a",
        ] {
            assert_eq!(s(p), p);
        }
        // COM0 and CONSOLE are ordinary names; so is a four-byte name with a wide character.
        assert_eq!(s(r"\\?\D:\COM0"), r"D:\COM0");
        assert_eq!(s(r"\\?\D:\CONSOLE"), r"D:\CONSOLE");
        assert_eq!(s(r"\\?\D:\abé"), r"D:\abé");
        // At or over MAX_PATH the prefix is what makes it work.
        let long = format!(r"\\?\D:\{}", "a".repeat(260));
        assert_eq!(s(&long), long);
    }

    #[test]
    fn canonicalizes_real_paths_without_the_prefix() {
        let here = canonicalize(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(!here.to_string_lossy().starts_with(r"\\?\"), "{here:?}");
        assert!(here.join("Cargo.toml").is_file());
    }
}
