//! `InputProfile::File`: a user camera profile (`.dcp`) for the raw front end.
//!
//! The edit stores the path and the file's content hash. The file is read once per path,
//! size and modification time (a small cache shared by every render). A missing or
//! unreadable file falls back to the camera matrix, and [`status`] says why; a file whose
//! content changed since it was chosen is still used, and [`status`] says so.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use specular_raw::{Dcp, DcpError, SensorMeta};

use crate::model::params::{InputProfile, RawFrontEnd};

/// A loaded profile file.
#[derive(Debug)]
pub struct Loaded {
    pub dcp: Dcp,
    /// BLAKE3 of the file, hex.
    pub hash: String,
}

/// Why the profile file is not used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// `InputProfile::File` without a path.
    NoFile,
    /// The file is not there.
    Missing,
    /// It could not be read (the system's words).
    Unreadable(String),
    /// It is not a usable camera profile.
    Refused(DcpError),
    /// The photo cannot take it (no white balance to apply it with, degenerate matrices).
    NotApplicable(DcpError),
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::NoFile => write!(f, "no profile file chosen"),
            Problem::Missing => write!(f, "the profile file is missing"),
            Problem::Unreadable(e) => write!(f, "the profile file cannot be read: {e}"),
            Problem::Refused(e) | Problem::NotApplicable(e) => write!(f, "{e}"),
        }
    }
}

/// What the module status shows for the input profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Auto or Matrix: nothing to say.
    Builtin,
    /// The file is in use; `changed`: its content differs from the one chosen.
    InUse { name: String, changed: bool },
    /// The matrix is used instead, for this reason.
    Fallback(Problem),
}

type Key = (PathBuf, u64, Option<SystemTime>);
type Cache = Mutex<Vec<(Key, Result<Arc<Loaded>, Problem>)>>;

const CACHED: usize = 4;

/// Reads a profile file (no cache).
pub fn read(path: &Path) -> Result<Loaded, Problem> {
    let meta = std::fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Problem::Missing,
        _ => Problem::Unreadable(e.to_string()),
    })?;
    if meta.len() > specular_raw::dcp::MAX_BYTES as u64 {
        return Err(Problem::Refused(DcpError::TooLarge));
    }
    let bytes = std::fs::read(path).map_err(|e| Problem::Unreadable(e.to_string()))?;
    let dcp = Dcp::read(&bytes).map_err(Problem::Refused)?;
    Ok(Loaded {
        dcp,
        hash: crate::chart::output::content_hash(&bytes),
    })
}

/// The profile at `path`, loaded once per path, size and modification time.
pub fn load(path: &str) -> Result<Arc<Loaded>, Problem> {
    static CACHE: Cache = Mutex::new(Vec::new());
    if path.is_empty() {
        return Err(Problem::NoFile);
    }
    let p = PathBuf::from(path);
    let meta = std::fs::metadata(&p).ok();
    let key = (
        p.clone(),
        meta.as_ref().map_or(0, |m| m.len()),
        meta.as_ref().and_then(|m| m.modified().ok()),
    );
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, r)) = cache.iter().find(|(k, _)| *k == key) {
        return r.clone();
    }
    let r = read(&p).map(Arc::new);
    if cache.len() >= CACHED {
        drop(cache.remove(0));
    }
    cache.push((key, r.clone()));
    r
}

/// The input profile's status for params `p`.
pub fn status(p: &RawFrontEnd) -> Status {
    if p.input_profile != InputProfile::File {
        return Status::Builtin;
    }
    match load(&p.profile_path) {
        Ok(l) => Status::InUse {
            name: if l.dcp.name.is_empty() {
                Path::new(&p.profile_path)
                    .file_stem()
                    .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
            } else {
                l.dcp.name.clone()
            },
            changed: !p.profile_hash.is_empty() && p.profile_hash != l.hash,
        },
        Err(e) => Status::Fallback(e),
    }
}

/// Applies params `p`'s input profile to `meta` (the file's own colour): nothing for Auto
/// and Matrix, the profile file for File. The reason when the file was asked for and could
/// not be used (`meta` is then unchanged).
pub fn apply(meta: &mut SensorMeta, p: &RawFrontEnd) -> Option<Problem> {
    if p.input_profile != InputProfile::File {
        return None;
    }
    match load(&p.profile_path) {
        Ok(l) => meta.apply_dcp(&l.dcp).err().map(Problem::NotApplicable),
        Err(e) => Some(e),
    }
}
