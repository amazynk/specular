//! Atomic, never-clobbering file creation: the bytes go to a temporary file in the
//! destination folder, which is then hard-linked to the final name (the link fails if the
//! name exists, so nothing is ever replaced) and removed. File systems without hard links
//! fall back to checking for the name and renaming, which can only race with another
//! program creating the same name in the same instant.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Conflict;

/// What happened to a write.
#[derive(Debug)]
pub enum Written {
    Created(PathBuf),
    /// The name existed and conflicts are skipped; that file was left alone.
    Exists(PathBuf),
}

/// `name`, `name-2`, `name-3`, … with the extension kept.
pub fn suffixed(name: &str, k: u32) -> String {
    if k <= 1 {
        return name.to_owned();
    }
    match name.rfind('.') {
        Some(dot) if dot > 0 => format!("{}-{k}{}", &name[..dot], &name[dot..]),
        _ => format!("{name}-{k}"),
    }
}

/// Writes `bytes` as a new file named `name` in `dir` (or a suffixed name, per `conflict`).
/// `taken` says whether a candidate name is reserved by another output of the batch.
pub fn write_new(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    conflict: Conflict,
    taken: &dyn Fn(&Path) -> bool,
) -> io::Result<Written> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let tmp = dir.join(format!(
        ".{name}.specular-tmp-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    let res = (|| {
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        let mut k = 1;
        loop {
            let path = dir.join(suffixed(name, k));
            let clash = taken(&path);
            if !clash {
                match link_or_rename(&tmp, &path) {
                    Ok(true) => return Ok(Written::Created(path)),
                    Ok(false) => {}
                    Err(e) => return Err(e),
                }
            }
            match conflict {
                Conflict::Skip => return Ok(Written::Exists(path)),
                Conflict::Suffix if k < 100_000 => k += 1,
                Conflict::Suffix => {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "no free name left",
                    ));
                }
            }
        }
    })();
    // Removing the temporary name fails harmlessly when it was renamed into place.
    let _ = fs::remove_file(&tmp);
    if let Ok(Written::Created(p)) = &res {
        sync_dir(p.parent().unwrap_or(dir));
    }
    res
}

/// Moves `tmp` to `path` unless `path` exists: `Ok(false)` when it does.
fn link_or_rename(tmp: &Path, path: &Path) -> io::Result<bool> {
    match fs::hard_link(tmp, path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(_) => {
            // No hard links here (FAT, some network shares): check, then rename.
            if fs::symlink_metadata(path).is_ok() {
                return Ok(false);
            }
            fs::rename(tmp, path)?;
            Ok(true)
        }
    }
}

fn sync_dir(dir: &Path) {
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
        let d = base.join(format!(
            "specular-convert-write-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn suffixes() {
        assert_eq!(suffixed("a.jpg", 1), "a.jpg");
        assert_eq!(suffixed("a.jpg", 2), "a-2.jpg");
        assert_eq!(suffixed("a.b.png", 3), "a.b-3.png");
        assert_eq!(suffixed(".hidden", 2), ".hidden-2");
        assert_eq!(suffixed("noext", 2), "noext-2");
    }

    #[test]
    fn never_clobbers() {
        let d = tmpdir("clobber");
        fs::write(d.join("x.png"), b"original").unwrap();
        let none = |_: &Path| false;
        let r = write_new(&d, "x.png", b"new", Conflict::Skip, &none).unwrap();
        assert!(matches!(r, Written::Exists(_)));
        assert_eq!(fs::read(d.join("x.png")).unwrap(), b"original");
        let r = write_new(&d, "x.png", b"new", Conflict::Suffix, &none).unwrap();
        let Written::Created(p) = r else { panic!() };
        assert_eq!(p, d.join("x-2.png"));
        assert_eq!(fs::read(&p).unwrap(), b"new");
        assert_eq!(fs::read(d.join("x.png")).unwrap(), b"original");
        // Reserved names are passed over too; no temporary files are left behind.
        let taken = |p: &Path| p.ends_with("x-3.png");
        let Written::Created(p) = write_new(&d, "x.png", b"n", Conflict::Suffix, &taken).unwrap()
        else {
            panic!()
        };
        assert_eq!(p, d.join("x-4.png"));
        let names: Vec<_> = fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(
            names.iter().all(|n| !n.contains("specular-tmp")),
            "{names:?}"
        );
        let _ = fs::remove_dir_all(&d);
    }
}
