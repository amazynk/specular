//! Addresses: joining a playlist's relative references to its own address, telling what an
//! address probably is before anything is fetched, and what a server's answer is.

/// What an address or a server's answer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guess {
    /// An HLS playlist (master or media).
    Hls,
    /// A DASH manifest (MPD).
    Dash,
    /// A playlist of addresses (PLS, or an M3U that is not HLS): a radio station's.
    Playlist,
    /// A media file or an endless sound stream.
    Media,
    /// A web page (only yt-dlp can find a stream in it).
    Page,
}

/// `rel` as an absolute address, read against `base` (RFC 3986 §5.2, for the forms playlists
/// use: absolute, scheme-relative, root-relative and relative with `.` and `..`).
pub fn resolve(base: &str, rel: &str) -> String {
    let rel = rel.trim();
    if has_scheme(rel) {
        return rel.to_owned();
    }
    let Some((scheme, rest)) = base.split_once("://") else {
        return rel.to_owned();
    };
    if let Some(r) = rel.strip_prefix("//") {
        return format!("{scheme}://{r}");
    }
    let auth_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..auth_end];
    let base_path = rest[auth_end..].split(['?', '#']).next().unwrap_or("");
    if rel.is_empty() {
        return base.split('#').next().unwrap_or(base).to_owned();
    }
    if rel.starts_with('?') {
        return format!("{scheme}://{authority}{base_path}{rel}");
    }
    if rel.starts_with('#') {
        return format!("{scheme}://{authority}{base_path}{rel}");
    }
    let (rel_path, tail) = match rel.find(['?', '#']) {
        Some(i) => rel.split_at(i),
        None => (rel, ""),
    };
    let merged = if rel_path.starts_with('/') {
        rel_path.to_owned()
    } else {
        let dir = match base_path.rfind('/') {
            Some(i) => &base_path[..=i],
            None => "/",
        };
        format!("{dir}{rel_path}")
    };
    format!("{scheme}://{authority}{}{tail}", remove_dots(&merged))
}

fn has_scheme(s: &str) -> bool {
    let Some(i) = s.find(':') else { return false };
    let scheme = &s[..i];
    !scheme.is_empty()
        && scheme
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && s[i..].starts_with("://")
}

/// A path without `.` and `..` segments.
fn remove_dots(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let segs: Vec<&str> = path.split('/').collect();
    for (i, s) in segs.iter().enumerate() {
        let last = i + 1 == segs.len();
        match *s {
            "." => {
                if last {
                    out.push("");
                }
            }
            ".." => {
                if out.len() > 1 {
                    out.pop();
                }
                if last {
                    out.push("");
                }
            }
            s => out.push(s),
        }
    }
    let j = out.join("/");
    if j.starts_with('/') {
        j
    } else {
        format!("/{j}")
    }
}

/// The lower-case extension of the address's path (`m3u8`, `mp3`), if any.
pub fn extension(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let path = rest.split(['?', '#']).next()?;
    let path = &path[path.find('/')?..];
    let name = path.rsplit('/').next()?;
    let (_, ext) = name.rsplit_once('.')?;
    (!ext.is_empty() && ext.len() <= 5).then(|| ext.to_ascii_lowercase())
}

/// The name of the address's last path segment (for a title), if any.
pub fn file_name(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let path = rest.split(['?', '#']).next()?;
    let path = &path[path.find('/')?..];
    let name = path.rsplit('/').next()?;
    (!name.is_empty()).then(|| name.to_owned())
}

/// What the address's extension says it is (`None`: nothing, ask the server).
pub fn guess(url: &str) -> Option<Guess> {
    let ext = extension(url)?;
    Some(match ext.as_str() {
        "m3u8" => Guess::Hls,
        "mpd" => Guess::Dash,
        "pls" | "m3u" => Guess::Playlist,
        "htm" | "html" | "php" | "asp" | "aspx" | "jsp" => Guess::Page,
        e if is_media_ext(e) => Guess::Media,
        _ => return None,
    })
}

/// Extensions of the files the player reads.
pub fn is_media_ext(ext: &str) -> bool {
    matches!(
        ext,
        "mp3"
            | "aac"
            | "m4a"
            | "ogg"
            | "oga"
            | "opus"
            | "flac"
            | "wav"
            | "mp4"
            | "m4v"
            | "mov"
            | "mkv"
            | "webm"
            | "mka"
            | "ts"
            | "m2ts"
            | "mts"
            | "avi"
            | "flv"
            | "3gp"
            | "mpg"
            | "mpeg"
            | "ogv"
            | "wma"
            | "wmv"
            | "asf"
    )
}

/// What a `Content-Type` says (`None`: nothing useful, like `application/octet-stream`).
pub fn from_content_type(ct: &str) -> Option<Guess> {
    let ct = ct
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    Some(match ct.as_str() {
        "application/vnd.apple.mpegurl" | "application/x-mpegurl" => Guess::Hls,
        "audio/mpegurl" | "audio/x-mpegurl" => Guess::Playlist,
        "application/dash+xml" => Guess::Dash,
        "audio/x-scpls" | "application/pls+xml" => Guess::Playlist,
        "text/html" | "application/xhtml+xml" => Guess::Page,
        c if c.starts_with("audio/") || c.starts_with("video/") => Guess::Media,
        "application/ogg" | "application/mp4" => Guess::Media,
        _ => return None,
    })
}

/// What the first bytes of an answer are (`None`: binary of some kind, taken as media).
pub fn sniff(head: &[u8]) -> Option<Guess> {
    let text = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    let t = text.trim_start_matches('\u{feff}').trim_start();
    if t.starts_with("#EXTM3U") {
        return Some(if t.contains("#EXT-X-") {
            Guess::Hls
        } else {
            Guess::Playlist
        });
    }
    if t.starts_with("[playlist]") || t.to_ascii_lowercase().starts_with("[playlist]") {
        return Some(Guess::Playlist);
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("<?xml") || lower.starts_with("<mpd") {
        if lower.contains("<mpd") {
            return Some(Guess::Dash);
        }
        return None;
    }
    if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
        return Some(Guess::Page);
    }
    // A bare list of addresses (an `.m3u` without its header).
    if t.starts_with("http://") || t.starts_with("https://") {
        return Some(Guess::Playlist);
    }
    None
}

/// What the recent list keeps of an address: scheme, host (and port) and path, without a
/// user name or password, query or fragment.
pub fn kept(url: &str) -> String {
    let s = url.trim();
    let Some((scheme, rest)) = s.split_once("://") else {
        return String::new();
    };
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (authority, path) = match rest.find('/') {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    let host = authority.rsplit('@').next().unwrap_or("");
    format!("{}://{host}{path}", scheme.to_ascii_lowercase())
}

/// Whether `url` is an address this crate opens: `http://` or `https://` with a host.
pub fn is_stream_url(url: &str) -> bool {
    let s = url.trim();
    let Some((scheme, rest)) = s.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    !host.is_empty() && !s.contains(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_resolve_like_rfc3986() {
        let b = "http://a/b/c/d;p?q";
        // RFC 3986 §5.4.1, the forms playlists use.
        for (r, want) in [
            ("g", "http://a/b/c/g"),
            ("./g", "http://a/b/c/g"),
            ("g/", "http://a/b/c/g/"),
            ("/g", "http://a/g"),
            ("//g", "http://g"),
            ("?y", "http://a/b/c/d;p?y"),
            ("g?y", "http://a/b/c/g?y"),
            ("g#s", "http://a/b/c/g#s"),
            (".", "http://a/b/c/"),
            ("./", "http://a/b/c/"),
            ("..", "http://a/b/"),
            ("../", "http://a/b/"),
            ("../g", "http://a/b/g"),
            ("../..", "http://a/"),
            ("../../g", "http://a/g"),
            ("../../../g", "http://a/g"),
            ("https://x/y.ts", "https://x/y.ts"),
        ] {
            assert_eq!(resolve(b, r), want, "{r}");
        }
        assert_eq!(
            resolve("https://h:8443/live/master.m3u8?token=1", "v1/index.m3u8"),
            "https://h:8443/live/v1/index.m3u8"
        );
        assert_eq!(resolve("https://h", "seg.ts"), "https://h/seg.ts");
    }

    #[test]
    fn addresses_and_answers_are_told_apart() {
        assert_eq!(guess("https://h/a/master.m3u8?x=1"), Some(Guess::Hls));
        assert_eq!(guess("https://h/m.MPD"), Some(Guess::Dash));
        assert_eq!(guess("http://h/radio.pls"), Some(Guess::Playlist));
        assert_eq!(guess("http://h/song.mp3"), Some(Guess::Media));
        assert_eq!(guess("http://h/watch?v=abc"), None);
        assert_eq!(guess("http://h/index.html"), Some(Guess::Page));
        assert_eq!(guess("http://h:8000/stream"), None);
        assert_eq!(guess("http://h.example"), None);
        assert_eq!(
            from_content_type("application/vnd.apple.mpegurl; charset=utf-8"),
            Some(Guess::Hls)
        );
        assert_eq!(from_content_type("audio/mpeg"), Some(Guess::Media));
        assert_eq!(from_content_type("text/html"), Some(Guess::Page));
        assert_eq!(from_content_type("application/octet-stream"), None);
        assert_eq!(
            sniff(b"#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:4,\na.ts\n"),
            Some(Guess::Hls)
        );
        assert_eq!(
            sniff(b"#EXTM3U\n#EXTINF:-1,Radio\nhttp://h/s\n"),
            Some(Guess::Playlist)
        );
        assert_eq!(
            sniff(b"[playlist]\nFile1=http://h/s\n"),
            Some(Guess::Playlist)
        );
        assert_eq!(
            sniff(b"<?xml version=\"1.0\"?>\n<MPD xmlns=\"urn:mpeg:dash:schema:mpd:2011\">"),
            Some(Guess::Dash)
        );
        assert_eq!(sniff(b"<!DOCTYPE html><html>"), Some(Guess::Page));
        assert_eq!(sniff(&[0xFF, 0xFB, 0x90, 0x64]), None);
    }

    #[test]
    fn kept_addresses_hold_no_secrets() {
        assert_eq!(
            kept("HTTPS://me:pw@h.example:8443/live/a.m3u8?token=abc#t"),
            "https://h.example:8443/live/a.m3u8"
        );
        assert!(is_stream_url("http://h:8000/stream"));
        assert!(!is_stream_url("ftp://h/a.mp3"));
        assert!(!is_stream_url("https:///a.mp3"));
        assert!(!is_stream_url("https://h/a b.mp3"));
        assert_eq!(extension("https://h/a/b.MP3?x=y.z"), Some("mp3".into()));
        assert_eq!(file_name("https://h/a/b.mp3?x"), Some("b.mp3".into()));
    }
}
