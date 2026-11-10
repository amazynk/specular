//! Where downloads come from. A transport opens a URL at an offset (to resume); the
//! installer does the rest (hashing, size checks, atomic install), so a transport is small.
//!
//! This crate has no HTTP client of its own: [`Transports::default`] serves `file://` and
//! refuses `https://` with [`NO_HTTPS`]. The app adds its HTTPS transport with
//! [`Transports::with`]: `specular-remote`'s client (ureq + rustls, resuming with `Range` +
//! `If-Range`), in `app/specular/src/app/ai/https.rs`. Tests here stay offline.
//! A client that writes downloads to a file itself implements [`Transport::download_to`]
//! (its [`Transport::open`] may then refuse).

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// An open download.
pub struct Body {
    pub reader: Box<dyn Read + Send>,
    /// The body starts at the offset asked for (false: from the beginning, the server
    /// ignored the range).
    pub resumed: bool,
    /// Bytes of the whole file, if the source says.
    pub total: Option<u64>,
}

/// A source of downloads for some URL schemes.
pub trait Transport: Send + Sync {
    /// Whether it serves `url`.
    fn serves(&self, url: &str) -> bool;
    /// Opens `url`, asking to start at byte `offset`.
    fn open(&self, url: &str, offset: u64) -> io::Result<Body>;

    /// [`open`](Self::open) for the model `name` (a client that reports its downloads by
    /// name: the app's "Downloads" indicator). The default ignores the name.
    fn open_named(&self, url: &str, offset: u64, name: &str) -> io::Result<Body> {
        let _ = name;
        self.open(url, offset)
    }

    /// Downloads `url` into `part` itself, resuming from the `offset` bytes already in it
    /// (a client that writes files on its own: the app's HTTPS client). `progress` gets the
    /// bytes in `part` so far; `cancel` stops it. The installer checks the size and hash
    /// afterwards and deletes a file that does not match. None: not supported, the
    /// installer reads [`Transport::open`]'s body instead (the default).
    fn download_to(
        &self,
        url: &str,
        part: &Path,
        offset: u64,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Option<io::Result<()>> {
        let _ = (url, part, offset, cancel, progress);
        None
    }
}

/// A download stopped from outside the installer (the app's "Downloads" panel): the
/// install ends as cancelled, not failed, and no other URL is tried.
#[derive(Debug)]
struct Stopped;

impl std::fmt::Display for Stopped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("stopped")
    }
}

impl std::error::Error for Stopped {}

/// The error a transport returns (from `open`, a body's `read` or `download_to`) when its
/// download was stopped elsewhere.
pub fn stopped() -> io::Error {
    io::Error::other(Stopped)
}

/// Whether `e` is [`stopped`].
pub fn is_stopped(e: &io::Error) -> bool {
    e.get_ref().is_some_and(|i| i.is::<Stopped>())
}

/// `file://` URLs (tests, and models copied in by hand).
pub struct FileTransport;

impl Transport for FileTransport {
    fn serves(&self, url: &str) -> bool {
        url.starts_with("file://")
    }

    fn open(&self, url: &str, offset: u64) -> io::Result<Body> {
        let path = url
            .strip_prefix("file://")
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a file:// URL"))?;
        let mut f = File::open(path)?;
        let total = f.metadata()?.len();
        let resumed = offset > 0 && offset <= total;
        if resumed {
            f.seek(SeekFrom::Start(offset))?;
        }
        Ok(Body {
            reader: Box::new(f),
            resumed,
            total: Some(total),
        })
    }
}

/// Why an `https://` download cannot start in this build.
pub const NO_HTTPS: &str = "this build has no HTTPS downloader";

/// The transports a download may use, by URL scheme; the first that serves a URL opens it.
pub struct Transports(Vec<Box<dyn Transport>>);

impl Default for Transports {
    fn default() -> Transports {
        Transports(vec![Box::new(FileTransport)])
    }
}

impl Transports {
    /// Adds a transport, tried before the others.
    pub fn with(mut self, t: Box<dyn Transport>) -> Transports {
        self.0.insert(0, t);
        self
    }
}

impl Transport for Transports {
    fn serves(&self, url: &str) -> bool {
        self.0.iter().any(|t| t.serves(url))
    }

    fn download_to(
        &self,
        url: &str,
        part: &Path,
        offset: u64,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Option<io::Result<()>> {
        self.0
            .iter()
            .find(|t| t.serves(url))?
            .download_to(url, part, offset, cancel, progress)
    }

    fn open(&self, url: &str, offset: u64) -> io::Result<Body> {
        self.open_named(url, offset, "")
    }

    fn open_named(&self, url: &str, offset: u64, name: &str) -> io::Result<Body> {
        match self.0.iter().find(|t| t.serves(url)) {
            Some(t) if name.is_empty() => t.open(url, offset),
            Some(t) => t.open_named(url, offset, name),
            None if url.starts_with("https://") => {
                Err(io::Error::new(io::ErrorKind::Unsupported, NO_HTTPS))
            }
            None => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("no transport for {url}"),
            )),
        }
    }
}
