//! Safe writes: whole-file replacement through a temporary in the same folder, no-clobber
//! placement of new names, and the size + modification-time stamps undo checks.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// What a file looked like after an operation: undo only touches it if it still does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub len: u64,
    pub modified: Option<SystemTime>,
}

impl Stamp {
    pub fn of(path: &Path) -> io::Result<Stamp> {
        let m = std::fs::symlink_metadata(path)?;
        Ok(Stamp {
            len: m.len(),
            modified: m.modified().ok(),
        })
    }

    /// Whether `path` still matches this stamp.
    pub fn matches(&self, path: &Path) -> bool {
        Stamp::of(path).is_ok_and(|s| s == *self)
    }
}

/// Whether a name is taken (by anything, a dangling symbolic link included).
pub fn taken(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

/// A hidden temporary name beside `target`.
pub fn temp_beside(target: &Path) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    target.with_file_name(format!(".{name}.specular-{}-{n}.part", std::process::id()))
}

/// Flush a folder's entries (the new name) to disk. Best effort: not every platform can open
/// a folder.
pub fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Replace `path` with `bytes`: written to a temporary in the same folder, given the old
/// file's permissions (and its modification time when `keep_mtime`), flushed, then renamed
/// over the original, so a crash leaves either the old file or the new one.
pub fn replace_file(path: &Path, bytes: &[u8], keep_mtime: bool) -> io::Result<()> {
    let old = std::fs::metadata(path)?;
    let tmp = temp_beside(path);
    let result = (|| {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.set_permissions(old.permissions())?;
        if keep_mtime && let Ok(t) = old.modified() {
            f.set_modified(t)?;
        }
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result?;
    if let Some(d) = path.parent() {
        sync_dir(d);
    }
    Ok(())
}

/// Write a new file at `path`, which must not exist (the name is claimed atomically).
pub fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = temp_beside(path);
    let result = (|| {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        place(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Give `from` the name `to` without replacing anything at `to`: a hard link then an unlink
/// within one file system (fails if `to` exists), else a checked rename. Case-only changes of
/// one file (`a.jpg` to `A.jpg` on a case-insensitive disk) are renamed directly.
pub fn place(from: &Path, to: &Path) -> io::Result<()> {
    if same_file(from, to) {
        return std::fs::rename(from, to);
    }
    match std::fs::hard_link(from, to) {
        Ok(()) => {
            if let Err(e) = std::fs::remove_file(from) {
                let _ = std::fs::remove_file(to);
                return Err(e);
            }
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
        Err(_) => {
            // No hard links here (FAT, some network shares): rename, having checked the name.
            if taken(to) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "the name is taken",
                ));
            }
            std::fs::rename(from, to)
        }
    }
}

/// Whether two paths name the same file (both exist).
pub fn same_file(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ma.dev() == mb.dev() && ma.ino() == mb.ino()
    }
    #[cfg(not(unix))]
    {
        ma.len() == mb.len()
            && ma.modified().ok() == mb.modified().ok()
            && std::fs::canonicalize(a).ok() == std::fs::canonicalize(b).ok()
    }
}

/// Whether `a` and `b` (existing paths) are on the same file system.
pub fn same_device(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(a), std::fs::metadata(b)) {
            (Ok(x), Ok(y)) => x.dev() == y.dev(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        // Same drive or share prefix.
        let root = |p: &Path| {
            std::fs::canonicalize(p)
                .ok()
                .and_then(|c| c.components().next().map(|c| c.as_os_str().to_owned()))
        };
        root(a).is_some() && root(a) == root(b)
    }
}

/// Copy `src` to the new name `dst` (which must not exist): through a hidden temporary in
/// the destination folder, with the source's permissions and modification time, flushed,
/// read back and compared, then placed. `step` gets the bytes copied so far and returns false
/// to cancel (the temporary is removed).
pub fn copy_verified(src: &Path, dst: &Path, step: &mut dyn FnMut(u64) -> bool) -> io::Result<()> {
    let tmp = temp_beside(dst);
    let result = (|| {
        let meta = std::fs::metadata(src)?;
        let mut from = File::open(src)?;
        let mut to = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        let mut buf = vec![0u8; 1 << 20];
        let mut done = 0u64;
        loop {
            let n = from.read(&mut buf)?;
            if n == 0 {
                break;
            }
            to.write_all(&buf[..n])?;
            done += n as u64;
            if !step(done) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
            }
        }
        to.set_permissions(meta.permissions())?;
        if let Ok(t) = meta.modified() {
            to.set_modified(t)?;
        }
        to.sync_all()?;
        drop(to);
        if !files_equal(src, &tmp)? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the copy does not read back the same",
            ));
        }
        place(&tmp, dst)?;
        if let Some(d) = dst.parent() {
            sync_dir(d);
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Byte-for-byte comparison.
pub fn files_equal(a: &Path, b: &Path) -> io::Result<bool> {
    let (mut fa, mut fb) = (File::open(a)?, File::open(b)?);
    if fa.metadata()?.len() != fb.metadata()?.len() {
        return Ok(false);
    }
    let mut ba = vec![0u8; 1 << 20];
    let mut bb = vec![0u8; 1 << 20];
    loop {
        let n = read_full(&mut fa, &mut ba)?;
        let m = read_full(&mut fb, &mut bb)?;
        if n != m || ba[..n] != bb[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}

fn read_full(f: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        let k = f.read(&mut buf[n..])?;
        if k == 0 {
            break;
        }
        n += k;
    }
    Ok(n)
}

/// Whether new files can be created in `dir` (probed with a hidden file, removed at once).
pub fn dir_writable(dir: &Path) -> bool {
    let probe = temp_beside(&dir.join("probe"));
    match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Set a file's modification time (used to put back the time a rotation preserved).
pub fn set_modified(path: &Path, t: SystemTime) -> io::Result<()> {
    OpenOptions::new().write(true).open(path)?.set_modified(t)
}
