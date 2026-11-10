//! Where a tileset's files come from: a folder on disk ([`DirFetch`]) or, through the caller,
//! a web server. Every reference is resolved against the file that makes it; a local tileset
//! never reads outside its own folder tree, and a remote one only from its own server.

use std::path::{Path, PathBuf};

/// Reads the files of a tileset by key: a path relative to the tileset's folder (`/`
/// separated, no `..`) or, for a remote tileset, a full `http(s)` address.
pub trait Fetch: Sync {
    /// The bytes of `key`, at most `max`; an error is a short reason.
    fn get(&self, key: &str, max: usize) -> Result<Vec<u8>, String>;
}

/// The files of a folder and the folders below it.
pub struct DirFetch {
    root: PathBuf,
}

impl DirFetch {
    pub fn new(root: impl Into<PathBuf>) -> DirFetch {
        DirFetch { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl Fetch for DirFetch {
    fn get(&self, key: &str, max: usize) -> Result<Vec<u8>, String> {
        if key.contains("://") || key.starts_with('/') || key.split('/').any(|p| p == "..") {
            return Err("outside the tileset's folder".into());
        }
        let path = self.root.join(key);
        // No symbolic link may lead out of the folder.
        let (Ok(real), Ok(root)) = (path.canonicalize(), self.root.canonicalize()) else {
            return Err(format!("{key} is missing"));
        };
        if !real.starts_with(&root) {
            return Err("outside the tileset's folder".into());
        }
        let meta = std::fs::metadata(&real).map_err(|e| e.to_string())?;
        if !meta.is_file() {
            return Err(format!("{key} is not a file"));
        }
        if meta.len() > max as u64 {
            return Err(format!("{key} is too large"));
        }
        std::fs::read(&real).map_err(|e| e.to_string())
    }
}

/// The key `uri` names, written in the file `base`; `None` when it leaves the tileset (an
/// absolute path, another server, a `..` above the folder). `data:` URIs are kept whole. On
/// a server, the query of `base` (access keys and the like) is carried over.
pub fn resolve(base: &str, uri: &str) -> Option<String> {
    let key = resolve_bare(base, uri)?;
    Some(match split_scheme(base) {
        Some(_) if !uri.trim().starts_with("data:") => with_query(&key, base),
        _ => key,
    })
}

/// `key` with the parameters of `base`'s query that it does not set itself.
fn with_query(key: &str, base: &str) -> String {
    let query = |s: &str| -> String {
        let s = s.split('#').next().unwrap_or("");
        s.split_once('?').map_or("", |(_, q)| q).to_owned()
    };
    let inherited = query(base);
    if inherited.is_empty() {
        return key.to_owned();
    }
    let (head, frag) = match key.find('#') {
        Some(i) => (&key[..i], &key[i..]),
        None => (key, ""),
    };
    let own = query(head);
    let name = |p: &str| p.split('=').next().unwrap_or("").to_owned();
    let names: Vec<String> = own.split('&').map(name).collect();
    let extra: Vec<&str> = inherited
        .split('&')
        .filter(|p| !p.is_empty() && !names.contains(&name(p)))
        .collect();
    if extra.is_empty() {
        return key.to_owned();
    }
    let sep = if own.is_empty() {
        if head.ends_with('?') { "" } else { "?" }
    } else {
        "&"
    };
    format!("{head}{sep}{}{frag}", extra.join("&"))
}

fn resolve_bare(base: &str, uri: &str) -> Option<String> {
    let uri = uri.trim();
    if uri.starts_with("data:") {
        return Some(uri.to_owned());
    }
    let decoded = percent_decode(uri);
    if let Some((scheme, rest)) = split_scheme(base) {
        // Remote: same server only.
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        let origin = format!("{scheme}://{host}");
        if let Some((s, r)) = split_scheme(uri) {
            let h = r.split('/').next().unwrap_or("");
            return (s.eq_ignore_ascii_case(scheme) && h.eq_ignore_ascii_case(host))
                .then(|| uri.to_owned());
        }
        if let Some(abs) = uri.strip_prefix('/') {
            return Some(format!("{origin}/{abs}"));
        }
        let path = path.split(['?', '#']).next().unwrap_or("");
        let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
        let joined = normalise(dir, uri)?;
        return Some(format!("{origin}/{joined}"));
    }
    if split_scheme(uri).is_some() || uri.starts_with('/') || uri.contains('\\') {
        return None;
    }
    let dir = base.rsplit_once('/').map_or("", |(d, _)| d);
    let path = decoded.split(['?', '#']).next().unwrap_or("");
    normalise(dir, path)
}

fn split_scheme(s: &str) -> Option<(&str, &str)> {
    let (scheme, rest) = s.split_once("://")?;
    (!scheme.is_empty()
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+'))
    .then_some((scheme, rest))
}

/// `dir` joined with the relative `rel`, `.` and `..` folded; `None` above the top.
fn normalise(dir: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    let (path, tail) = match rel.find(['?', '#']) {
        Some(i) => (&rel[..i], &rel[i..]),
        None => (rel, ""),
    };
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(format!("{}{tail}", parts.join("/")))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(h) = b.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(std::str::from_utf8(h).unwrap_or("zz"), 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The bytes of `key`: a `data:` URI decoded here, anything else through `fetch`.
pub fn load(fetch: &dyn Fetch, key: &str, max: usize) -> Result<Vec<u8>, String> {
    if let Some(rest) = key.strip_prefix("data:") {
        let (meta, data) = rest
            .split_once(',')
            .ok_or_else(|| "a data URI without data".to_string())?;
        return if meta.ends_with(";base64") {
            base64(data).ok_or_else(|| "a data URI that is not base64".into())
        } else {
            Ok(percent_decode(data).into_bytes())
        };
    }
    fetch.get(key, max)
}

fn base64(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolving() {
        assert_eq!(
            resolve("tileset.json", "a/b.b3dm").as_deref(),
            Some("a/b.b3dm")
        );
        assert_eq!(
            resolve("sub/t.json", "../x.b3dm").as_deref(),
            Some("x.b3dm")
        );
        assert_eq!(resolve("t.json", "../x.b3dm"), None);
        assert_eq!(resolve("t.json", "/etc/passwd"), None);
        assert_eq!(resolve("t.json", "https://e.org/x"), None);
        assert_eq!(
            resolve("a/t.json", "b%20c.glb").as_deref(),
            Some("a/b c.glb")
        );
        // The tileset's query (an access key) goes with every file on its server.
        assert_eq!(
            resolve("https://h.org/t/tileset.json?k=1", "c/0.b3dm").as_deref(),
            Some("https://h.org/t/c/0.b3dm?k=1")
        );
        assert_eq!(
            resolve("https://h.org/t/tileset.json?k=1&v=2", "c/0.b3dm?v=3#f").as_deref(),
            Some("https://h.org/t/c/0.b3dm?v=3&k=1#f")
        );
        assert_eq!(
            resolve("https://h.org/t/tileset.json?k=1", "https://h.org/x.json").as_deref(),
            Some("https://h.org/x.json?k=1")
        );
        assert_eq!(
            resolve("https://h.org/t/c/0.json?k=1", "../1.b3dm").as_deref(),
            Some("https://h.org/t/1.b3dm?k=1")
        );
        assert_eq!(
            resolve("https://h.org/t.json?k=1", "https://evil.org/x"),
            None
        );
        // Local tilesets keep names as written.
        assert_eq!(resolve("t.json", "a.b3dm?x=1").as_deref(), Some("a.b3dm"));
        assert_eq!(
            resolve("https://h.org/t/tileset.json", "../u.json").as_deref(),
            Some("https://h.org/u.json")
        );
        assert_eq!(
            resolve("https://h.org/t/tileset.json", "/v/x.glb").as_deref(),
            Some("https://h.org/v/x.glb")
        );
        assert_eq!(resolve("https://h.org/t.json", "https://evil.org/x"), None);
        assert_eq!(
            load(
                &DirFetch::new("."),
                "data:application/octet-stream;base64,aGVsbG8=",
                99
            )
            .unwrap(),
            b"hello"
        );
    }
}
