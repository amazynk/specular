//! Keyed byte stores that live elsewhere: a Zarr store on an HTTP server or in an S3 bucket.
//!
//! Decoders read local stores through their own traits; a remote store implements
//! [`KeyStore`] (in `specular-remote`) and the decoders adapt it. Unlike a folder, a remote
//! store can fail for reasons other than a missing key, so every read says which:
//! `Ok(None)` is "no such key" (a Zarr chunk that was never written), an error is anything
//! else, worded for the user.

use std::sync::atomic::AtomicBool;

/// Why a remote read failed (the application words it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyError {
    /// The read was stopped (the view moved on, the image closed).
    Cancelled,
    /// The server refused access (HTTP 401 or 403).
    Denied,
    /// The server answered with this error status.
    Status(u16),
    /// The server could not be reached (offline, wrong address), with the system's words.
    Offline(String),
    /// No answer in time.
    Timeout,
    /// Anything else (a broken answer, a disk error), in English.
    Failed(String),
    /// The no-network switch is on: nothing was sent ([`crate::net::off`]).
    NetworkOff,
}

impl KeyError {
    /// The I/O error kind that says the same (for decoders' `io::Error`s).
    pub fn kind(&self) -> std::io::ErrorKind {
        use std::io::ErrorKind as K;
        match self {
            KeyError::Cancelled => K::Interrupted,
            KeyError::Denied => K::PermissionDenied,
            KeyError::Status(404 | 410) => K::NotFound,
            KeyError::Status(_) | KeyError::Failed(_) => K::Other,
            KeyError::Offline(_) => K::NotConnected,
            KeyError::NetworkOff => K::PermissionDenied,
            KeyError::Timeout => K::TimedOut,
        }
    }

    /// As an `io::Error` that keeps this error inside (`get_ref` downcasts to it).
    pub fn into_io(self) -> std::io::Error {
        std::io::Error::new(self.kind(), self)
    }
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyError::Cancelled => write!(f, "cancelled"),
            KeyError::Denied => write!(f, "the server refused access"),
            KeyError::Status(s) => write!(f, "the server answered HTTP {s}"),
            KeyError::Offline(m) => write!(f, "cannot connect: {m}"),
            KeyError::Timeout => write!(f, "the server did not answer in time"),
            KeyError::Failed(m) => write!(f, "{m}"),
            KeyError::NetworkOff => f.write_str(crate::net::off::MESSAGE),
        }
    }
}

impl std::error::Error for KeyError {}

/// Which bytes of a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Whole,
    /// `len` bytes at `start`.
    At(u64, u64),
    /// The last `len` bytes (a shard index at the end), with the value's size.
    Suffix(u64),
}

/// One value (or some bytes of one) a reader is about to need.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Want {
    pub key: String,
    pub part: Part,
}

impl Want {
    pub fn whole(key: impl Into<String>) -> Want {
        Want {
            key: key.into(),
            part: Part::Whole,
        }
    }
    pub fn range(key: impl Into<String>, start: u64, len: u64) -> Want {
        Want {
            key: key.into(),
            part: Part::At(start, len),
        }
    }
    pub fn suffix(key: impl Into<String>, len: u64) -> Want {
        Want {
            key: key.into(),
            part: Part::Suffix(len),
        }
    }
}

/// A store of keyed values ('/'-separated keys relative to its root) read over a network.
pub trait KeyStore: Send + Sync {
    /// The whole value at `key`; `Ok(None)` when there is none.
    fn get(&self, key: &str, cancel: Option<&AtomicBool>) -> Result<Option<Vec<u8>>, KeyError>;
    /// `len` bytes at `start` of the value at `key` (fewer at its end).
    fn get_range(
        &self,
        key: &str,
        start: u64,
        len: u64,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Vec<u8>>, KeyError>;
    /// The last `len` bytes of the value at `key` (all of it when shorter) and the value's
    /// size.
    fn get_suffix(
        &self,
        key: &str,
        len: u64,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<(Vec<u8>, u64)>, KeyError>;
    /// The size of the value at `key`.
    fn size(&self, key: &str, cancel: Option<&AtomicBool>) -> Result<Option<u64>, KeyError>;
    /// Fetches `wants` ahead of the reads that follow, several at a time; the reads then
    /// come from memory. Missing keys are no error; the first other failure is returned.
    fn prefetch(&self, wants: &[Want], cancel: Option<&AtomicBool>) -> Result<(), KeyError>;
    /// The names of the folders directly under `dir` ("" for the root, else ending in '/');
    /// empty when the store cannot list (plain HTTP).
    fn subdirs(&self, dir: &str, cancel: Option<&AtomicBool>) -> Result<Vec<String>, KeyError>;
    /// Where the store is, for the info panel: the address without credentials or query.
    fn location(&self) -> String;
    /// Whether the server refused a request that was then taken as "absent" (S3 answers
    /// 403 for missing keys): an empty store may in fact be a private one.
    fn refused(&self) -> bool {
        false
    }
}
