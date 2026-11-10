//! Side files: an OBJ's MTL and textures, a glTF's buffers and images, a PLY's texture.
//!
//! Only relative names are followed, and only to files inside the model's own folder tree:
//! no absolute paths, no drive letters, no URLs of any scheme, no `..` that climbs out, no
//! symbolic link that leads out. Scene formats built as asset libraries (USD, COLLADA) may
//! climb a few folders above the model's ([`DirResolver::climbing`]); the same rules hold
//! below that folder. Only regular files are read. Nothing here touches the network.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Reads side files named by a model.
pub trait Resolver: Sync {
    /// The bytes of the side file `name` (as written in the model, already percent-decoded
    /// for URIs), at most `max` bytes; an error is a short reason for the info panel.
    fn read(&self, name: &str, max: usize) -> std::result::Result<Vec<u8>, String>;
}

/// Reads no side files (a model given as bytes alone, or inside an archive).
pub struct NoResolver;

impl Resolver for NoResolver {
    fn read(&self, _name: &str, _max: usize) -> std::result::Result<Vec<u8>, String> {
        Err("side files are not read for this model".into())
    }
}

/// Reads side files from a folder and the folders below it.
pub struct DirResolver {
    dir: PathBuf,
    /// Folders above `dir` a name may climb to with leading `..`.
    climb: usize,
}

impl DirResolver {
    pub fn new(dir: impl Into<PathBuf>) -> DirResolver {
        DirResolver {
            dir: dir.into(),
            climb: 0,
        }
    }

    /// Lets names climb up to `levels` folders above the model's (USD and COLLADA asset
    /// libraries keep shared layers, materials and textures beside the asset's folder).
    pub fn climbing(mut self, levels: usize) -> DirResolver {
        self.climb = levels;
        self
    }

    /// For the model file at `path`: its folder.
    pub fn for_file(path: &Path) -> DirResolver {
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        DirResolver::new(dir)
    }
}

impl Resolver for DirResolver {
    fn read(&self, name: &str, max: usize) -> std::result::Result<Vec<u8>, String> {
        let (up, rel) = relative_climbing(name, self.climb)
            .ok_or("not a relative path inside the model's folder")?;
        let mut root = self.dir.canonicalize().map_err(|e| e.to_string())?;
        for _ in 0..up {
            root = root
                .parent()
                .ok_or("outside the model's folder")?
                .to_path_buf();
        }
        let full = root.join(&rel);
        // Symbolic links: the real file must still be under the real folder.
        let real = full.canonicalize().map_err(|_| "not found".to_string())?;
        if !real.starts_with(&root) {
            return Err("outside the model's folder".into());
        }
        // Checked before opening: a FIFO or a device would block or never end.
        if !std::fs::metadata(&real)
            .map_err(|e| e.to_string())?
            .is_file()
        {
            return Err("not a file".into());
        }
        let file = std::fs::File::open(&real).map_err(|e| e.to_string())?;
        let meta = file.metadata().map_err(|e| e.to_string())?;
        if !meta.is_file() {
            return Err("not a file".into());
        }
        if meta.len() > max as u64 {
            return Err("too large".into());
        }
        let mut out = Vec::with_capacity(meta.len() as usize);
        file.take(max as u64 + 1)
            .read_to_end(&mut out)
            .map_err(|e| e.to_string())?;
        if out.len() > max {
            return Err("too large".into());
        }
        Ok(out)
    }
}

/// `name` as a relative path that stays inside its base folder, or `None`: absolute paths,
/// drive letters, UNC paths, URLs (anything with a scheme), empty names and `..` that would
/// climb out are refused. Backslashes are read as separators (Windows exporters).
pub fn safe_relative(name: &str) -> Option<PathBuf> {
    match relative_climbing(name, 0) {
        Some((0, p)) => Some(p),
        _ => None,
    }
}

/// Like [`safe_relative`], but leading `..` may climb up to `climb` folders: how many it
/// climbs, and the rest.
pub fn relative_climbing(name: &str, climb: usize) -> Option<(usize, PathBuf)> {
    let name = name.trim();
    if name.is_empty() || name.contains('\0') {
        return None;
    }
    // A scheme (`http:`, `file:`, `data:`) or a drive letter (`C:`).
    if let Some(colon) = name.find(':') {
        let scheme = &name[..colon];
        if !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        {
            return None;
        }
    }
    let name = name.replace('\\', "/");
    if name.starts_with('/') {
        return None;
    }
    let mut out: Vec<&str> = Vec::new();
    let mut up = 0usize;
    for part in name.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if out.pop().is_none() {
                    up += 1;
                    if up > climb {
                        return None;
                    }
                }
            }
            p => out.push(p),
        }
    }
    if out.is_empty() {
        return None;
    }
    let path: PathBuf = out.iter().collect();
    // Belt and braces: only plain components remain.
    path.components()
        .all(|c| matches!(c, Component::Normal(_)))
        .then_some((up, path))
}
