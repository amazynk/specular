//! The play queue and playlist files.
//!
//! - [`queue`]: what plays and in which order: repeat (off, one, all), shuffle (no repeats
//!   until every item has played, with a history so Previous goes back) and
//!   [`Queue::next_item`], which answers "what plays next" without changing anything (for
//!   gapless and crossfade, which look ahead).
//! - Own readers for M3U/M3U8 (`#EXTINF`, relative paths, UTF-8 or Latin-1), PLS and XSPF,
//!   and writers for M3U8 and XSPF. Network addresses in a playlist are kept as
//!   [`Location::Url`]; nothing here fetches anything.

use std::path::{Path, PathBuf};

mod m3u;
mod pls;
pub mod queue;
mod uri;
mod xspf;

pub use queue::{Queue, Repeat};
pub use uri::{file_uri, path_from_file_uri};

/// The largest playlist file read (a playlist of 100 000 tracks is about 10 MB).
pub const MAX_BYTES: usize = 32 << 20;

/// Where an item is.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Location {
    /// A file on this computer.
    File(PathBuf),
    /// A network address (`http://…`): never fetched here; the app plays `http(s)` ones
    /// as network streams.
    Url(String),
}

impl Location {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Location::File(p) => Some(p),
            Location::Url(_) => None,
        }
    }
}

/// One item of a playlist or the queue.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub location: Location,
    /// Its title as the playlist (or the file's tags) gives it.
    pub title: Option<String>,
    /// Its length in seconds, when known.
    pub duration: Option<f64>,
}

impl Entry {
    /// A file with nothing else known.
    pub fn file(path: impl Into<PathBuf>) -> Entry {
        Entry {
            location: Location::File(path.into()),
            title: None,
            duration: None,
        }
    }

    /// Whether it can be played: a file, or an `http://` or `https://` address (played as
    /// a network stream by the app); other addresses (`rtsp://`, `mms://`) are listed only.
    pub fn playable(&self) -> bool {
        match &self.location {
            Location::File(_) => true,
            Location::Url(u) => {
                let l = u.trim().to_ascii_lowercase();
                l.starts_with("http://") || l.starts_with("https://")
            }
        }
    }

    /// The name shown when it has no title: the file's name, or the address.
    pub fn name(&self) -> String {
        match &self.location {
            Location::File(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string_lossy().into_owned()),
            Location::Url(u) => u.clone(),
        }
    }
}

/// A playlist read from a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playlist {
    pub title: Option<String>,
    pub entries: Vec<Entry>,
}

/// The playlist formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// M3U and M3U8 (extended or plain).
    M3u,
    /// PLS (Winamp, Shoutcast).
    Pls,
    /// XSPF ("spiff", XML Shareable Playlist Format).
    Xspf,
}

/// The extensions of playlist files, lower case.
pub const EXTENSIONS: [&str; 4] = ["m3u", "m3u8", "pls", "xspf"];

impl Kind {
    /// By the file's extension.
    pub fn of_path(p: &Path) -> Option<Kind> {
        let ext = p.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "m3u" | "m3u8" => Some(Kind::M3u),
            "pls" => Some(Kind::Pls),
            "xspf" => Some(Kind::Xspf),
            _ => None,
        }
    }

    /// By the file's first bytes: `#EXTM3U`, `[playlist]`, or an XML `playlist` element in
    /// the XSPF namespace. A plain M3U (a list of paths) has nothing to tell it by.
    pub fn probe(head: &[u8]) -> Option<Kind> {
        let head = head
            .strip_prefix(b"\xEF\xBB\xBF".as_slice())
            .unwrap_or(head);
        let start = head
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(head.len());
        let head = &head[start..];
        if head.starts_with(b"#EXTM3U") {
            return Some(Kind::M3u);
        }
        if head.len() >= 10 && head[..10].eq_ignore_ascii_case(b"[playlist]") {
            return Some(Kind::Pls);
        }
        if head.starts_with(b"<") {
            let n = head.len().min(1024);
            let text = String::from_utf8_lossy(&head[..n]);
            if text.contains("<playlist") && text.contains("http://xspf.org/ns/0/") {
                return Some(Kind::Xspf);
            }
        }
        None
    }
}

/// Why a playlist could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Larger than [`MAX_BYTES`].
    TooLarge,
    /// Not a playlist of the kind its name says (no XSPF `playlist` element, say).
    NotPlaylist,
    /// Its XML does not parse (the reason, in English, for the log).
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::TooLarge => write!(f, "larger than {} MB", MAX_BYTES >> 20),
            Error::NotPlaylist => write!(f, "not a playlist"),
            Error::Malformed(m) => write!(f, "malformed: {m}"),
        }
    }
}

impl std::error::Error for Error {}

/// Reads playlist `data` of `kind`; relative paths are taken from `base` (the playlist's
/// folder).
pub fn read(data: &[u8], kind: Kind, base: &Path) -> Result<Playlist, Error> {
    if data.len() > MAX_BYTES {
        return Err(Error::TooLarge);
    }
    match kind {
        Kind::M3u => Ok(m3u::read(&text(data), base)),
        Kind::Pls => Ok(pls::read(&text(data), base)),
        Kind::Xspf => xspf::read(&text(data), base),
    }
}

/// Reads the playlist file at `path` (its kind from its name, else its content).
pub fn read_file(path: &Path) -> Result<Playlist, ReadFileError> {
    let meta = std::fs::metadata(path).map_err(ReadFileError::Io)?;
    if meta.len() > MAX_BYTES as u64 {
        return Err(ReadFileError::Playlist(Error::TooLarge));
    }
    let data = std::fs::read(path).map_err(ReadFileError::Io)?;
    let kind = Kind::of_path(path)
        .or_else(|| Kind::probe(&data))
        .ok_or(ReadFileError::Playlist(Error::NotPlaylist))?;
    let base = path.parent().unwrap_or(Path::new(""));
    read(&data, kind, base).map_err(ReadFileError::Playlist)
}

/// Why [`read_file`] failed.
#[derive(Debug)]
pub enum ReadFileError {
    Io(std::io::Error),
    Playlist(Error),
}

impl std::fmt::Display for ReadFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadFileError::Io(e) => e.fmt(f),
            ReadFileError::Playlist(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ReadFileError {}

/// The queue's items as an M3U8 playlist to be saved at `dest`: UTF-8, `#EXTINF` with the
/// length and title, files in the playlist's folder (or below it) by relative path, others
/// by full path.
pub fn write_m3u8(title: Option<&str>, entries: &[Entry], dest: &Path) -> String {
    m3u::write(title, entries, dest.parent().unwrap_or(Path::new("")))
}

/// The queue's items as an XSPF playlist to be saved at `dest`: file addresses relative to
/// its folder when inside it.
pub fn write_xspf(title: Option<&str>, entries: &[Entry], dest: &Path) -> String {
    xspf::write(title, entries, dest.parent().unwrap_or(Path::new("")))
}

/// Text of a playlist: UTF-8 (with or without a byte-order mark), UTF-16 with a mark, else
/// Windows-1252 (Latin-1 with the printable C1 range, as old Windows players wrote `.m3u`).
pub fn text(data: &[u8]) -> String {
    if let Some((enc, bom)) = encoding_rs::Encoding::for_bom(data) {
        return enc.decode_without_bom_handling(&data[bom..]).0.into_owned();
    }
    match std::str::from_utf8(data) {
        Ok(s) => s.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(data).0.into_owned(),
    }
}

/// A location written in a playlist: a `file:` address, another address (`http://…`),
/// or a path (relative ones from `base`; Windows backslashes read as separators).
pub(crate) fn locate(raw: &str, base: &Path) -> Option<Location> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(p) = path_from_file_uri(raw) {
        return Some(Location::File(p));
    }
    if uri::has_scheme(raw) {
        return Some(Location::Url(raw.to_owned()));
    }
    Some(Location::File(join_path(raw, base)))
}

/// `raw` (a path as a playlist writes it) from `base`.
pub(crate) fn join_path(raw: &str, base: &Path) -> PathBuf {
    // A Windows drive path stays as written (`C:\Music\a.mp3`): on Windows it is absolute,
    // elsewhere it names no file here.
    let drive = raw.len() >= 3
        && raw.as_bytes()[0].is_ascii_alphabetic()
        && raw.as_bytes()[1] == b':'
        && matches!(raw.as_bytes()[2], b'\\' | b'/');
    if drive {
        return PathBuf::from(raw);
    }
    let unified = if cfg!(windows) {
        raw.to_owned()
    } else {
        raw.replace('\\', "/")
    };
    let p = Path::new(&unified);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        normalise(&base.join(p))
    }
}

/// `a/./b/../c` as `a/c`, without touching the disk.
fn normalise(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// How `path` is written in a playlist saved in `dir`: relative when inside it (with `/`),
/// else in full.
pub(crate) fn relative_to(path: &Path, dir: &Path) -> Option<String> {
    let rel = path.strip_prefix(dir).ok()?;
    if rel.as_os_str().is_empty() {
        return None;
    }
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests;
