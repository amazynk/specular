//! The log file: appended to, and rotated when it grows past a size cap (`specular.log` →
//! `specular.1.log` → `specular.2.log`, the oldest dropped), so it never grows without bound.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Default cap of one log file.
pub(crate) const DEFAULT_CAP: u64 = 4 << 20;
/// Rotated files kept besides the current one.
pub(crate) const KEEP: usize = 2;

pub(crate) struct RotatingFile {
    path: PathBuf,
    file: File,
    len: u64,
    cap: u64,
}

/// `dir/name.log` → `dir/name.<n>.log`.
fn numbered(path: &Path, n: usize) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = match path.extension() {
        Some(e) => format!("{stem}.{n}.{}", e.to_string_lossy()),
        None => format!("{stem}.{n}"),
    };
    path.with_file_name(name)
}

impl RotatingFile {
    /// Opens `path` for appending (creating its folder), rotating it first if it is already
    /// over `cap`.
    pub(crate) fn open(path: &Path, cap: u64) -> io::Result<RotatingFile> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        if std::fs::metadata(path).is_ok_and(|m| m.len() >= cap) {
            shift(path);
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let len = file.metadata().map_or(0, |m| m.len());
        Ok(RotatingFile {
            path: path.to_path_buf(),
            file,
            len,
            cap: cap.max(1),
        })
    }

    fn rotate(&mut self) -> io::Result<()> {
        let _ = self.file.flush();
        shift(&self.path);
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.len = 0;
        Ok(())
    }
}

/// Moves `path` to `.1`, `.1` to `.2`, …, dropping the oldest.
fn shift(path: &Path) {
    let _ = std::fs::remove_file(numbered(path, KEEP));
    for n in (1..KEEP).rev() {
        let _ = std::fs::rename(numbered(path, n), numbered(path, n + 1));
    }
    let _ = std::fs::rename(path, numbered(path, 1));
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Whole lines arrive in one write: rotate between them, never inside one.
        if self.len > 0 && self.len + buf.len() as u64 > self.cap {
            self.rotate()?;
        }
        self.file.write_all(buf)?;
        self.len += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_at_the_cap_and_keeps_two_old_files() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-specular-log")
            .join(format!("rotate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("specular.log");
        let mut f = RotatingFile::open(&path, 100).unwrap();
        let line = [b'x'; 39];
        for _ in 0..20 {
            f.write_all(&line).unwrap();
            f.write_all(b"\n").unwrap();
        }
        drop(f);
        assert!(std::fs::metadata(&path).unwrap().len() <= 100);
        assert!(std::fs::metadata(numbered(&path, 1)).unwrap().len() <= 100);
        assert!(numbered(&path, 2).is_file());
        assert!(!numbered(&path, 3).exists());
        assert_eq!(numbered(&path, 1).file_name().unwrap(), "specular.1.log");

        // A file already over the cap is rotated when opened.
        std::fs::write(&path, [b'y'; 150]).unwrap();
        let f = RotatingFile::open(&path, 100).unwrap();
        assert_eq!(f.len, 0);
        assert_eq!(std::fs::metadata(numbered(&path, 1)).unwrap().len(), 150);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
