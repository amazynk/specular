//! Finding the hub (SAMP 1.3 Standard Profile §3.3): `SAMP_HUB=std-lockurl:URL`, else the
//! lock file `.samp` in the home folder. Only a lock file on this computer is read, and only
//! a hub on this computer is used.

use std::path::{Path, PathBuf};

/// The largest lock file read.
const MAX_LOCK: u64 = 64 << 10;

/// What the lock file says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HubInfo {
    pub secret: String,
    /// The hub's XML-RPC address (checked to be on this computer).
    pub url: String,
    pub profile_version: String,
    /// The lock file read.
    pub lock: PathBuf,
}

/// Why no hub was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoHub {
    /// No lock file: no hub is running.
    NotRunning,
    /// `SAMP_HUB` names a lock file somewhere other than this computer.
    LockNotLocal(String),
    /// The lock file names a hub somewhere other than this computer.
    HubNotLocal(String),
    /// The lock file lacks the secret or the address.
    Malformed,
}

impl std::fmt::Display for NoHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NoHub::NotRunning => f.write_str("no SAMP hub is running"),
            NoHub::LockNotLocal(u) => write!(f, "the hub's lock file {u} is not on this computer"),
            NoHub::HubNotLocal(u) => write!(f, "the hub at {u} is not on this computer"),
            NoHub::Malformed => f.write_str("the hub's lock file is incomplete"),
        }
    }
}

/// A `file:` URL's path (`file:///p`, `file://localhost/p`, `file:/p`), percent-decoded.
pub fn file_path(url: &str) -> Option<PathBuf> {
    let rest = url
        .strip_prefix("file://localhost")
        .or_else(|| url.strip_prefix("file://127.0.0.1"))
        .or_else(|| url.strip_prefix("file://"))
        .or_else(|| url.strip_prefix("file:"))?;
    if !rest.starts_with('/') {
        // `file://host/…`: another computer's file.
        return None;
    }
    let decoded = percent_decode(rest)?;
    // `/C:/x` on Windows.
    let p = if cfg!(windows) && decoded.len() > 2 && decoded.as_bytes()[2] == b':' {
        decoded[1..].to_owned()
    } else {
        decoded
    };
    Some(PathBuf::from(p))
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// A local path as a `file://` URL.
pub fn file_url(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
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
    out
}

/// Parses a lock file's text.
pub fn parse(text: &str, lock: &Path) -> Result<HubInfo, NoHub> {
    let mut secret = None;
    let mut url = None;
    let mut version = String::new();
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = l.split_once('=') {
            match k.trim() {
                "samp.secret" => secret = Some(v.trim().to_owned()),
                "samp.hub.xmlrpc.url" => url = Some(v.trim().to_owned()),
                "samp.profile.version" => version = v.trim().to_owned(),
                _ => {}
            }
        }
    }
    let (Some(secret), Some(url)) = (secret, url) else {
        return Err(NoHub::Malformed);
    };
    if crate::http::local(&url).is_err() {
        return Err(NoHub::HubNotLocal(url));
    }
    Ok(HubInfo {
        secret,
        url,
        profile_version: version,
        lock: lock.to_path_buf(),
    })
}

/// The lock file to read: from `samp_hub` (the `SAMP_HUB` variable) or in `home`.
pub fn lock_path(samp_hub: Option<&str>, home: Option<&Path>) -> Result<PathBuf, NoHub> {
    if let Some(v) = samp_hub.map(str::trim).filter(|v| !v.is_empty()) {
        let url = v.strip_prefix("std-lockurl:").unwrap_or(v);
        return file_path(url).ok_or_else(|| NoHub::LockNotLocal(url.to_owned()));
    }
    home.map(|h| h.join(".samp")).ok_or(NoHub::NotRunning)
}

/// Finds the running hub.
pub fn find(samp_hub: Option<&str>, home: Option<&Path>) -> Result<HubInfo, NoHub> {
    let path = lock_path(samp_hub, home)?;
    let m = std::fs::metadata(&path).map_err(|_| NoHub::NotRunning)?;
    if !m.is_file() || m.len() > MAX_LOCK {
        return Err(NoHub::NotRunning);
    }
    let text = std::fs::read_to_string(&path).map_err(|_| NoHub::NotRunning)?;
    parse(&text, &path)
}

/// Finds the hub as this process's environment says.
pub fn find_from_env() -> Result<HubInfo, NoHub> {
    let hub = std::env::var("SAMP_HUB").ok();
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    find(hub.as_deref(), home.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_files() {
        let l = Path::new("/h/.samp");
        let ok = parse(
            "# SAMP lockfile\nsamp.secret=abc123\nsamp.hub.xmlrpc.url=http://127.0.0.1:41234/xmlrpc\nsamp.profile.version=1.3\n",
            l,
        )
        .unwrap();
        assert_eq!(ok.secret, "abc123");
        assert_eq!(ok.url, "http://127.0.0.1:41234/xmlrpc");
        assert_eq!(
            parse("samp.secret=x\nsamp.hub.xmlrpc.url=http://10.0.0.5:1/\n", l),
            Err(NoHub::HubNotLocal("http://10.0.0.5:1/".into()))
        );
        assert_eq!(parse("samp.secret=x\n", l), Err(NoHub::Malformed));
        assert_eq!(
            lock_path(Some("std-lockurl:file:///run/user/1/hub.lock"), None).unwrap(),
            PathBuf::from("/run/user/1/hub.lock")
        );
        assert_eq!(
            lock_path(Some("std-lockurl:http://example.org/lock"), None),
            Err(NoHub::LockNotLocal("http://example.org/lock".into()))
        );
        assert_eq!(
            lock_path(None, Some(Path::new("/home/a"))).unwrap(),
            PathBuf::from("/home/a/.samp")
        );
    }

    #[test]
    fn file_urls() {
        assert_eq!(
            file_path("file://localhost/data/m%2051.fits"),
            Some(PathBuf::from("/data/m 51.fits"))
        );
        assert_eq!(file_path("file:///a"), Some(PathBuf::from("/a")));
        assert_eq!(file_path("file:/a"), Some(PathBuf::from("/a")));
        assert_eq!(file_path("file://otherhost/a"), None);
        assert_eq!(file_path("http://127.0.0.1/a"), None);
        let p = Path::new("/data/m 51 é.fits");
        assert_eq!(file_path(&file_url(p)).as_deref(), Some(p));
    }
}
