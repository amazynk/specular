//! Sideloading: model files copied to this computer by hand (offline and air-gapped
//! machines), installed exactly like downloaded ones.
//!
//! - **Install from file** (Settings › Local AI): each picked file (or every file in a picked
//!   folder) is hashed and looked up in the manifest by its SHA-256 and size. A known file is
//!   copied to where its download would end (`downloads/<sha256>.part`, verified again while
//!   copying), so the next install of its feature finds it complete and needs no network.
//! - A file whose hash is not in the manifest is refused, unless its name is a model's file
//!   name and the user confirms using it unverified: it is copied to
//!   `downloads/<id>-<version>.unverified` and installed as that model, marked unverified.
//! - **Admin folders** ([`Store::with_folders`]): the policy's `model_folder` and the system
//!   folder (`/etc/specular/models` on Linux). An install looks there first for a file of the
//!   model's size and hash and copies it in; no network is needed.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::install::{InstallError, Store};
use crate::manifest::{Manifest, Model};

/// Remembered hashes of admin-folder files: path → ((length, modified), SHA-256).
pub type HashCache =
    std::collections::HashMap<PathBuf, ((u64, Option<std::time::SystemTime>), [u8; 32])>;

/// Folders deeper than this under a picked or admin folder are not looked in.
const MAX_DEPTH: usize = 3;
/// At most this many files are looked at in one folder tree.
const MAX_FILES: usize = 4096;
const CHUNK: usize = 1 << 20;

/// What a file is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Match {
    /// The file is this model: its SHA-256 and size are the manifest's.
    Verified(Model),
    /// Its hash is not in the manifest, but its name is this model's file name: usable only
    /// when the user confirms.
    Unverified(Model),
    /// Not a model the manifest knows.
    Unknown,
}

/// A file looked at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: [u8; 32],
    pub found: Match,
}

/// The files of `paths`: files as they are, folders' files (up to [`MAX_DEPTH`] levels,
/// hidden ones skipped), sorted, without duplicates.
pub fn files_in(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            walk(p, 0, &mut out);
        } else if p.is_file() {
            out.push(p.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if out.len() >= MAX_FILES {
            return;
        }
        if p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'))
        {
            continue;
        }
        if p.is_dir() {
            if depth < MAX_DEPTH {
                walk(&p, depth + 1, out);
            }
        } else if p.is_file() {
            out.push(p);
        }
    }
}

/// The SHA-256 and length of a file.
pub fn hash_file(path: &Path) -> io::Result<([u8; 32], u64)> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut n_all = 0u64;
    loop {
        let n = match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        h.update(&buf[..n]);
        n_all += n as u64;
    }
    Ok((h.finalize().into(), n_all))
}

/// The file name a model is published under (its first URL's last part, without a query).
pub fn file_name(m: &Model) -> Option<&str> {
    let url = m.urls.first()?;
    let path = url.split(['?', '#']).next()?;
    path.rsplit('/').next().filter(|s| !s.is_empty())
}

/// What `path` is (hashing it).
pub fn identify(manifest: &Manifest, path: &Path) -> io::Result<Candidate> {
    let (sha256, size) = hash_file(path)?;
    let found = if let Some(m) = manifest
        .models
        .iter()
        .find(|m| m.sha256 == sha256 && m.size == size)
    {
        Match::Verified(m.clone())
    } else {
        let name = path.file_name().and_then(|n| n.to_str());
        match manifest
            .models
            .iter()
            .find(|m| name.is_some() && file_name(m) == name)
        {
            Some(m) => Match::Unverified(m.clone()),
            None => Match::Unknown,
        }
    };
    Ok(Candidate {
        path: path.to_owned(),
        size,
        sha256,
        found,
    })
}

/// Copies `from` to `to` through a temporary file beside it, hashing what is copied; the
/// hash and length of the copy.
fn copy_hashed(from: &Path, to: &Path) -> io::Result<([u8; 32], u64)> {
    if let Some(d) = to.parent() {
        fs::create_dir_all(d)?;
    }
    let tmp = to.with_extension(format!("copy-{}", std::process::id()));
    let r = (|| {
        let mut src = File::open(from)?;
        let mut dst = File::create(&tmp)?;
        let mut h = Sha256::new();
        let mut buf = vec![0u8; CHUNK];
        let mut len = 0u64;
        loop {
            let n = match src.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            dst.write_all(&buf[..n])?;
            h.update(&buf[..n]);
            len += n as u64;
        }
        dst.sync_all()?;
        Ok((h.finalize().into(), len))
    })();
    match r {
        Ok(v) => {
            fs::rename(&tmp, to)?;
            Ok(v)
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

impl Store {
    /// Puts the file `from` where the download of `m` would end, checking its SHA-256
    /// against the manifest's again while copying (a file that changed since it was looked
    /// at is refused). The next install of `m`'s feature uses it.
    pub fn sideload(&self, m: &Model, from: &Path) -> Result<(), InstallError> {
        let part = self.part_path(m);
        let tmp = part.with_extension("sideload");
        let (sha, len) = copy_hashed(from, &tmp)?;
        if sha != m.sha256 || len != m.size {
            let _ = fs::remove_file(&tmp);
            return Err(InstallError::Hash {
                model: m.id.clone(),
            });
        }
        fs::rename(&tmp, &part)?;
        specular_log::info!(target: specular_log::JOBS, "ai: sideloaded {}", m.key());
        Ok(())
    }

    /// Puts the file `from` in as model `m` without a hash check (the user confirmed using
    /// an unverified model). Installed as `m`, marked unverified.
    pub fn sideload_unverified(&self, m: &Model, from: &Path) -> Result<(), InstallError> {
        let to = self.unverified_path(m);
        copy_hashed(from, &to)?;
        specular_log::warn!(target: specular_log::JOBS, "ai: sideloaded {} unverified", m.key());
        Ok(())
    }

    /// Where an unverified file for `m` waits to be installed.
    pub fn unverified_path(&self, m: &Model) -> PathBuf {
        self.root()
            .join("downloads")
            .join(format!("{}.unverified", m.key()))
    }

    /// A file of `m` in the admin folders, by size and hash (the first that matches; a
    /// file's hash is remembered while its length and time stay the same).
    pub fn find_in_folders(&self, m: &Model) -> Option<PathBuf> {
        if self.folders().is_empty() {
            return None;
        }
        files_in(self.folders()).into_iter().find(|p| {
            let Ok(md) = fs::metadata(p) else {
                return false;
            };
            if md.len() != m.size {
                return false;
            }
            let stamp = (md.len(), md.modified().ok());
            if let Some((s, h)) = self.hashes.lock().get(p)
                && *s == stamp
            {
                return *h == m.sha256;
            }
            match hash_file(p) {
                Ok((h, n)) => {
                    self.hashes.lock().insert(p.clone(), (stamp, h));
                    h == m.sha256 && n == m.size
                }
                Err(_) => false,
            }
        })
    }

    /// Whether the file of `m` is here to install without the network: its download is
    /// complete, it was sideloaded (unverified too) or it is in an admin folder.
    pub fn local(&self, m: &Model) -> bool {
        fs::metadata(self.part_path(m)).is_ok_and(|md| md.len() == m.size)
            || self.unverified_path(m).is_file()
            || self.find_in_folders(m).is_some()
    }
}
