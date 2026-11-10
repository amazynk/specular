//! Verified, resumable copies that never replace a file.
//!
//! A file is written to a hidden temporary name beside its destination
//! (`.IMG_0001.JPG.specular-import`), synced, read back and compared by blake3 with what was
//! read from the source, given the source's modification time, and only then linked under its
//! final name (a hard link, which fails rather than replace; a no-replace rename where the file
//! system has no links). A copy cut short leaves its temporary file; the next attempt checks
//! that its bytes match the source's start and carries on from there.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::Hash;

const BUF: usize = 1 << 20;
/// The temporary name's suffix.
pub const PARTIAL_SUFFIX: &str = ".specular-import";

/// Pause and cancel for a running import, shared with the UI.
#[derive(Debug, Default)]
pub struct Control {
    cancel: AtomicBool,
    paused: AtomicBool,
}

impl Control {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, on: bool) {
        self.paused.store(on, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn cancel_flag(&self) -> &AtomicBool {
        &self.cancel
    }

    /// Blocks while paused; `false` once cancelled.
    pub fn proceed(&self) -> bool {
        while self.is_paused() && !self.is_cancelled() {
            std::thread::sleep(Duration::from_millis(40));
        }
        !self.is_cancelled()
    }
}

/// Why a copy did not finish.
#[derive(Debug)]
pub enum CopyError {
    Cancelled,
    /// The copy read back differently from the source (a failing card or disk).
    Mismatch(PathBuf),
    Io(PathBuf, std::io::Error),
}

impl std::fmt::Display for CopyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CopyError::Cancelled => write!(f, "cancelled"),
            CopyError::Mismatch(p) => write!(f, "{}: the copy does not match", p.display()),
            CopyError::Io(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for CopyError {}

/// A finished copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Copied {
    pub dest: PathBuf,
    pub hash: Hash,
    /// The same bytes were already there under that name; nothing was written.
    pub existed: bool,
}

fn io(p: &Path) -> impl Fn(std::io::Error) -> CopyError + '_ {
    move |e| CopyError::Io(p.to_path_buf(), e)
}

/// blake3 of a whole file.
pub fn hash_file(path: &Path) -> std::io::Result<Hash> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; BUF];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(*h.finalize().as_bytes())
}

/// `name` with `-N` before its first dot (`IMG_0001-2.CR2.xmp`); `n == 0` is `name`.
pub fn numbered(name: &str, n: usize) -> String {
    if n == 0 {
        return name.to_owned();
    }
    match name.find('.') {
        Some(i) if i > 0 => format!("{}-{n}{}", &name[..i], &name[i..]),
        _ => format!("{name}-{n}"),
    }
}

/// The temporary file for a copy to `dest`.
pub fn partial_path(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    dest.with_file_name(format!(".{name}{PARTIAL_SUFFIX}"))
}

/// The name in `dir` a copy of `src` (blake3 `src_hash`, if known) should take: `name`, or
/// `name-1`, `name-2`… past files that differ. `Ok((path, true))` when a file with the same
/// bytes is already there.
fn resolve(
    dir: &Path,
    name: &str,
    size: u64,
    src_hash: &mut Option<Hash>,
    src: &Path,
) -> Result<(PathBuf, bool), CopyError> {
    for n in 0..10_000 {
        let cand = dir.join(numbered(name, n));
        match std::fs::symlink_metadata(&cand) {
            Err(_) => return Ok((cand, false)),
            Ok(md) if md.is_file() && md.len() == size => {
                let h = match src_hash {
                    Some(h) => *h,
                    None => {
                        let h = hash_file(src).map_err(io(src))?;
                        *src_hash = Some(h);
                        h
                    }
                };
                if hash_file(&cand).map_err(io(&cand))? == h {
                    return Ok((cand, true));
                }
            }
            Ok(_) => {}
        }
    }
    Err(CopyError::Io(
        dir.join(name),
        std::io::Error::other("too many files with this name"),
    ))
}

fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Copies `src` into `dir` as `name` (or a numbered variant if another file has that name),
/// verified. `progress(n)` reports bytes as they are copied (including a resumed prefix).
pub fn copy_verified(
    src: &Path,
    dir: &Path,
    name: &str,
    ctl: &Control,
    progress: &mut dyn FnMut(u64),
) -> Result<Copied, CopyError> {
    let md = std::fs::metadata(src).map_err(io(src))?;
    let size = md.len();
    let mtime = md.modified().ok();
    std::fs::create_dir_all(dir).map_err(io(dir))?;
    let mut src_hash = None;
    let (dest, existed) = resolve(dir, name, size, &mut src_hash, src)?;
    if existed {
        progress(size);
        return Ok(Copied {
            dest,
            hash: src_hash.unwrap_or_default(),
            existed: true,
        });
    }
    let tmp = partial_path(&dest);
    let mut input = File::open(src).map_err(io(src))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; BUF];

    // Resume: keep the temporary file's prefix if it matches the source's.
    let mut out;
    let mut done = 0u64;
    match OpenOptions::new().read(true).write(true).open(&tmp) {
        Ok(mut t) => {
            let have = t.metadata().map_err(io(&tmp))?.len().min(size);
            let mut other = vec![0u8; BUF];
            let mut same = true;
            while done < have {
                let n = (have - done).min(BUF as u64) as usize;
                input.read_exact(&mut buf[..n]).map_err(io(src))?;
                t.read_exact(&mut other[..n]).map_err(io(&tmp))?;
                if buf[..n] != other[..n] {
                    same = false;
                    break;
                }
                hasher.update(&buf[..n]);
                done += n as u64;
                if !ctl.proceed() {
                    return Err(CopyError::Cancelled);
                }
            }
            if !same {
                done = 0;
                hasher = blake3::Hasher::new();
                input.seek(SeekFrom::Start(0)).map_err(io(src))?;
            }
            t.set_len(done).map_err(io(&tmp))?;
            t.seek(SeekFrom::Start(done)).map_err(io(&tmp))?;
            progress(done);
            out = t;
        }
        Err(_) => {
            out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
                .map_err(io(&tmp))?;
        }
    }

    loop {
        if !ctl.proceed() {
            let _ = out.sync_data();
            return Err(CopyError::Cancelled);
        }
        let n = input.read(&mut buf).map_err(io(src))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(io(&tmp))?;
        done += n as u64;
        progress(n as u64);
    }
    if let Some(t) = mtime {
        let _ = out.set_modified(t);
    }
    out.sync_all().map_err(io(&tmp))?;
    drop(out);
    let hash = *hasher.finalize().as_bytes();
    if done != size || hash_file(&tmp).map_err(io(&tmp))? != hash {
        let _ = std::fs::remove_file(&tmp);
        return Err(CopyError::Mismatch(dest));
    }

    // Link under the final name without replacing anything; a name taken meanwhile gets the
    // next number.
    let mut dest = dest;
    for _ in 0..100 {
        match std::fs::hard_link(&tmp, &dest) {
            Ok(()) => {
                let _ = std::fs::remove_file(&tmp);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let mut h = Some(hash);
                let (d, existed) = resolve(dir, name, size, &mut h, src)?;
                if existed {
                    let _ = std::fs::remove_file(&tmp);
                    return Ok(Copied {
                        dest: d,
                        hash,
                        existed: true,
                    });
                }
                dest = d;
            }
            // FAT, exFAT and some network shares have no hard links.
            Err(_) => {
                if dest.exists() {
                    let mut h = Some(hash);
                    dest = resolve(dir, name, size, &mut h, src)?.0;
                    continue;
                }
                std::fs::rename(&tmp, &dest).map_err(io(&dest))?;
                break;
            }
        }
    }
    sync_dir(dir);
    Ok(Copied {
        dest,
        hash,
        existed: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{filler, fresh, write};

    #[test]
    fn copies_verify_keep_mtime_and_never_replace() {
        let root = fresh("copy");
        let src = root.join("card/IMG_0001.JPG");
        let data = filler(3 * BUF + 123, 9);
        write(&src, &data);
        let old = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        File::options()
            .write(true)
            .open(&src)
            .unwrap()
            .set_modified(old)
            .unwrap();
        let dir = root.join("dest/2024");
        let ctl = Control::default();
        let mut seen = 0;
        let c = copy_verified(&src, &dir, "IMG_0001.JPG", &ctl, &mut |n| seen += n).unwrap();
        assert_eq!(c.dest, dir.join("IMG_0001.JPG"));
        assert!(!c.existed);
        assert_eq!(seen, data.len() as u64);
        assert_eq!(std::fs::read(&c.dest).unwrap(), data);
        assert_eq!(c.hash, hash_file(&src).unwrap());
        assert_eq!(std::fs::metadata(&c.dest).unwrap().modified().unwrap(), old);
        assert!(!partial_path(&c.dest).exists());

        // Same bytes again: found, not copied.
        let again = copy_verified(&src, &dir, "IMG_0001.JPG", &ctl, &mut |_| {}).unwrap();
        assert!(again.existed);
        assert_eq!(again.dest, c.dest);

        // Different bytes under the same name: a numbered name, the first untouched.
        let other = root.join("card2/IMG_0001.JPG");
        write(&other, b"another picture");
        let o = copy_verified(&other, &dir, "IMG_0001.JPG", &ctl, &mut |_| {}).unwrap();
        assert_eq!(o.dest, dir.join("IMG_0001-1.JPG"));
        assert_eq!(std::fs::read(&c.dest).unwrap(), data);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_partial_copy_resumes_and_a_wrong_one_restarts() {
        let root = fresh("resume");
        let src = root.join("card/MVI_0003.MP4");
        let data = filler(2 * BUF + 77, 3);
        write(&src, &data);
        let dir = root.join("dest");
        let ctl = Control::default();

        // Interrupted after 1.5 MiB.
        let tmp = partial_path(&dir.join("MVI_0003.MP4"));
        write(&tmp, &data[..BUF + BUF / 2]);
        let mut seen = 0;
        let c = copy_verified(&src, &dir, "MVI_0003.MP4", &ctl, &mut |n| seen += n).unwrap();
        assert_eq!(std::fs::read(&c.dest).unwrap(), data);
        assert_eq!(seen, data.len() as u64);
        assert!(!tmp.exists());

        // A temporary file that does not match the source is started over.
        std::fs::remove_file(&c.dest).unwrap();
        let mut junk = data[..BUF].to_vec();
        junk[10] ^= 1;
        write(&tmp, &junk);
        let c = copy_verified(&src, &dir, "MVI_0003.MP4", &ctl, &mut |_| {}).unwrap();
        assert_eq!(std::fs::read(&c.dest).unwrap(), data);

        // Cancelled: the temporary file stays for next time, nothing under the final name.
        std::fs::remove_file(&c.dest).unwrap();
        ctl.cancel();
        assert!(matches!(
            copy_verified(&src, &dir, "MVI_0003.MP4", &ctl, &mut |_| {}),
            Err(CopyError::Cancelled)
        ));
        assert!(tmp.exists());
        assert!(!dir.join("MVI_0003.MP4").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn numbering() {
        assert_eq!(numbered("IMG_0001.CR2.xmp", 2), "IMG_0001-2.CR2.xmp");
        assert_eq!(numbered("README", 1), "README-1");
        assert_eq!(numbered("a.jpg", 0), "a.jpg");
    }
}
