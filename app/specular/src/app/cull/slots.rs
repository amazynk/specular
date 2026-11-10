//! Copy and move slots: the current item (every member of a RAW+JPEG pair and their XMP
//! sidecars) goes to one of the folders set in `[cull]` (DESIGN.md §6.3).
//!
//! These are explicit file operations, never edits of an image:
//! - Nothing is overwritten. A name that is taken either gets a ` (2)` suffix, the same for
//!   every file of the item so the pair and its sidecars still match, or the item is skipped.
//! - Within one file system a file is hard-linked under its new name (which fails rather than
//!   replace anything) and then unlinked from the old one, so it is never missing; where hard
//!   links are not supported it is renamed. Across file systems it is copied to a hidden
//!   temporary name in the destination, flushed to disk, and only then linked under its
//!   name; a move removes the original after that.
//! - New names are created without following links (`O_EXCL`, `link`, `rename` act on the
//!   name itself), so a symbolic link planted in the destination is never written through.
//! - Undo moves the files back the same way, or deletes a copy only when it is still the
//!   size and age it was made with.
//!
//! The functions block; cull mode runs them on a background thread.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// What happens when a name is taken in the destination (`cull.collisions`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Collision {
    /// `IMG_1 (2).CR3`.
    Suffix,
    /// The item stays where it is.
    Skip,
}

/// A copy's size and modification time when it was made.
pub(crate) type Stamp = (u64, Option<SystemTime>);

/// One file put in place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    pub from: PathBuf,
    pub to: PathBuf,
    /// For a copy: what it looked like, so undo deletes it only unchanged.
    pub stamp: Option<Stamp>,
}

/// The result of sending one item to a slot.
#[derive(Debug, Default)]
pub(crate) struct Outcome {
    pub done: Vec<Placed>,
    /// Names were taken: the item's new stem.
    pub renamed: Option<String>,
    /// Names were taken and the item was left alone.
    pub skipped: bool,
    /// The destination is the item's own folder.
    pub same_folder: bool,
    pub failed: Vec<(PathBuf, String)>,
}

/// Whether a name is taken (by anything, a dangling symbolic link included).
fn taken(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

/// The destination folder, resolved once: it must be an existing folder.
pub(crate) fn check_dest(dest: &Path) -> io::Result<PathBuf> {
    let d = std::fs::canonicalize(dest)?;
    if !std::fs::metadata(&d)?.is_dir() {
        return Err(io::Error::new(io::ErrorKind::NotADirectory, "not a folder"));
    }
    Ok(d)
}

/// The files that travel with `members`: the members, then the XMP sidecars in their folder
/// named after the item's stem (`IMG_1.xmp`) or a member (`IMG_1.CR3.xmp`), any case.
pub(crate) fn with_sidecars(members: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = members.to_vec();
    let Some(dir) = members.first().and_then(|m| m.parent()) else {
        return out;
    };
    let lower = |s: &std::ffi::OsStr| s.to_string_lossy().to_lowercase();
    let mut names: Vec<String> = members
        .iter()
        .filter_map(|m| m.file_name().map(|n| format!("{}.xmp", lower(n))))
        .collect();
    if let Some(stem) = members[0].file_stem() {
        names.push(format!("{}.xmp", lower(stem)));
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut sidecars: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| names.contains(&lower(&e.file_name())))
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| !out.contains(p))
        .collect();
    sidecars.sort();
    out.extend(sidecars);
    out
}

/// `name` split after the item's stem: `("IMG_1", ".CR3.xmp")`. The stem is matched without
/// regard to case (a pair may be `img_1.nef` and `IMG_1.JPG`); a name that does not start
/// with it keeps its own stem.
fn split_stem<'a>(name: &'a str, stem: &str) -> (&'a str, &'a str) {
    let n = stem.chars().count();
    let cut = name.char_indices().nth(n).map_or(name.len(), |(i, _)| i);
    let (head, rest) = name.split_at(cut);
    if head.to_lowercase() == stem.to_lowercase() {
        return (head, rest);
    }
    match name.find('.') {
        Some(i) if i > 0 => name.split_at(i),
        _ => (name, ""),
    }
}

/// The destination names of `files` (an item stemmed `stem`): their own names, or with
/// `Suffix` the first ` (n)` that frees all of them. `None`: skipped. The bool: renamed.
pub(crate) fn plan(
    files: &[PathBuf],
    stem: &str,
    dest: &Path,
    collision: Collision,
) -> Option<(Vec<PathBuf>, bool)> {
    let names: Vec<String> = files
        .iter()
        .map(|f| {
            f.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .collect();
    let plain: Vec<PathBuf> = names.iter().map(|n| dest.join(n)).collect();
    if !plain.iter().any(|p| taken(p)) {
        return Some((plain, false));
    }
    if collision == Collision::Skip {
        return None;
    }
    (2..10_000).find_map(|k| {
        let v: Vec<PathBuf> = names
            .iter()
            .map(|n| {
                let (head, rest) = split_stem(n, stem);
                dest.join(format!("{head} ({k}){rest}"))
            })
            .collect();
        (!v.iter().any(|p| taken(p))).then_some((v, true))
    })
}

/// Sends `files` (an item stemmed `stem`, its members and sidecars) to `dest`.
pub(crate) fn place(
    files: &[PathBuf],
    stem: &str,
    dest: &Path,
    copy: bool,
    collision: Collision,
) -> Outcome {
    let mut out = Outcome::default();
    let dest = match check_dest(dest) {
        Ok(d) => d,
        Err(e) => {
            out.failed = files
                .iter()
                .map(|f| (f.clone(), crate::error_text::reason(&e)))
                .collect();
            return out;
        }
    };
    let here = files
        .first()
        .and_then(|f| f.parent())
        .and_then(|p| std::fs::canonicalize(p).ok());
    if here.as_deref() == Some(dest.as_path()) {
        out.same_folder = true;
        return out;
    }
    let Some((targets, renamed)) = plan(files, stem, &dest, collision) else {
        out.skipped = true;
        return out;
    };
    if renamed {
        out.renamed = targets.first().and_then(|t| {
            let n = t.file_name()?.to_string_lossy().into_owned();
            let (head, _) = split_stem(&n, &format!("{stem} (0)"));
            Some(head.to_owned())
        });
    }
    for (from, to) in files.iter().zip(targets) {
        match transfer(from, &to, copy) {
            Ok(stamp) => out.done.push(Placed {
                from: from.clone(),
                to,
                stamp,
            }),
            Err(e) => out
                .failed
                .push((from.clone(), crate::error_text::reason(&e))),
        }
    }
    sync_dir(&dest);
    if !copy && let Some(h) = &here {
        sync_dir(h);
    }
    out
}

/// Undo of a move: every file back to where it was (never over anything). The files back,
/// and the failures.
pub(crate) fn put_back(placed: &[Placed]) -> (Vec<PathBuf>, Vec<(PathBuf, String)>) {
    let (mut done, mut failed) = (Vec::new(), Vec::new());
    for p in placed {
        match transfer(&p.to, &p.from, false) {
            Ok(_) => done.push(p.from.clone()),
            Err(e) => failed.push((p.to.clone(), crate::error_text::reason(&e))),
        }
    }
    (done, failed)
}

/// Undo of a copy: the copies deleted, each only if it is as it was made. The deleted, the
/// changed ones kept, and the failures.
pub(crate) fn remove_copies(
    placed: &[Placed],
) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<(PathBuf, String)>) {
    let (mut done, mut kept, mut failed) = (Vec::new(), Vec::new(), Vec::new());
    for p in placed {
        let now = std::fs::symlink_metadata(&p.to)
            .ok()
            .filter(|m| m.is_file())
            .map(|m| (m.len(), m.modified().ok()));
        if now.is_none() {
            // Gone already: nothing to undo.
            continue;
        }
        if now != p.stamp {
            kept.push(p.to.clone());
            continue;
        }
        match std::fs::remove_file(&p.to) {
            Ok(()) => done.push(p.to.clone()),
            Err(e) => failed.push((p.to.clone(), crate::error_text::reason(&e))),
        }
    }
    (done, kept, failed)
}

/// Puts `from` at `to`, never replacing anything there: a copy's stamp, or `None` for a
/// move.
fn transfer(from: &Path, to: &Path, copy: bool) -> io::Result<Option<Stamp>> {
    if taken(to) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "the name is taken",
        ));
    }
    if copy {
        copy_atomic(from, to)?;
        return Ok(Some(stamp_of(to)?));
    }
    match std::fs::hard_link(from, to) {
        Ok(()) => {
            if let Err(e) = std::fs::remove_file(from) {
                // Both names stay possible: take the new one back.
                let _ = std::fs::remove_file(to);
                return Err(e);
            }
            return Ok(None);
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(e),
        Err(e) if e.kind() != io::ErrorKind::CrossesDevices && same_device(from, to) => {
            // No hard links here (FAT, exFAT): a rename, checked just before.
            if taken(to) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "the name is taken",
                ));
            }
            std::fs::rename(from, to)?;
            return Ok(None);
        }
        Err(_) => {}
    }
    // Another file system: copy, then remove the original.
    copy_atomic(from, to)?;
    let a = std::fs::metadata(from)?.len();
    let b = std::fs::metadata(to)?.len();
    if a != b {
        let _ = std::fs::remove_file(to);
        return Err(io::Error::other("the copy is incomplete"));
    }
    if let Err(e) = std::fs::remove_file(from) {
        let _ = std::fs::remove_file(to);
        return Err(e);
    }
    Ok(None)
}

fn stamp_of(p: &Path) -> io::Result<Stamp> {
    let m = std::fs::symlink_metadata(p)?;
    Ok((m.len(), m.modified().ok()))
}

#[cfg(unix)]
fn same_device(from: &Path, to: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let dev = |p: &Path| std::fs::symlink_metadata(p).ok().map(|m| m.dev());
    let parent = to.parent().unwrap_or(Path::new("."));
    dev(from).is_some() && dev(from) == dev(parent)
}

#[cfg(not(unix))]
fn same_device(from: &Path, to: &Path) -> bool {
    let root = |p: &Path| p.components().next().map(|c| c.as_os_str().to_owned());
    root(from) == root(to)
}

static TEMP: AtomicU64 = AtomicU64::new(0);

/// Copies `from` to a new hidden name next to `to`, flushes it, and links it as `to`.
fn copy_atomic(from: &Path, to: &Path) -> io::Result<()> {
    let dir = to.parent().unwrap_or(Path::new("."));
    let name = to
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(
        ".{name}.specular-{}-{}.part",
        std::process::id(),
        TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let r = (|| {
        let mut src = File::open(from)?;
        let meta = src.metadata()?;
        let mut dst = File::options().write(true).create_new(true).open(&tmp)?;
        io::copy(&mut src, &mut dst)?;
        dst.set_permissions(meta.permissions())?;
        if let Ok(t) = meta.modified() {
            dst.set_modified(t)?;
        }
        dst.sync_all()?;
        drop(dst);
        match std::fs::hard_link(&tmp, to) {
            Ok(()) => std::fs::remove_file(&tmp),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
            Err(_) => {
                if taken(to) {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "the name is taken",
                    ));
                }
                std::fs::rename(&tmp, to)
            }
        }
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

/// Makes a folder's new entries durable (where folders can be opened).
fn sync_dir(dir: &Path) {
    if let Ok(f) = File::open(dir) {
        let _ = f.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let base = std::env::var_os("SPECULAR_TEST_TMP").map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-tmp"),
            PathBuf::from,
        );
        let d = base.join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    /// A RAW+JPEG pair and both its sidecars, and an unrelated file with a similar name.
    fn shoot(d: &Path) -> Vec<PathBuf> {
        for (f, b) in [
            ("img_1.CR3", &b"raw bytes"[..]),
            ("IMG_1.JPG", b"jpeg bytes"),
            ("IMG_1.xmp", b"<x:xmpmeta/>"),
            ("IMG_1.JPG.XMP", b"<x:xmpmeta/>"),
            ("IMG_10.JPG", b"another"),
        ] {
            std::fs::write(d.join(f), b).unwrap();
        }
        vec![d.join("img_1.CR3"), d.join("IMG_1.JPG")]
    }

    #[test]
    fn sidecars_travel_with_the_pair() {
        let d = scratch("cull-slots-sidecars");
        let members = shoot(&d);
        let files = with_sidecars(&members);
        assert_eq!(
            files,
            vec![
                d.join("img_1.CR3"),
                d.join("IMG_1.JPG"),
                d.join("IMG_1.JPG.XMP"),
                d.join("IMG_1.xmp"),
            ]
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn move_and_put_back() {
        let d = scratch("cull-slots-move");
        let src = d.join("card");
        let dest = d.join("picks");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        let files = with_sidecars(&shoot(&src));
        let o = place(&files, "IMG_1", &dest, false, Collision::Suffix);
        assert!(o.failed.is_empty(), "{:?}", o.failed);
        assert_eq!(o.done.len(), 4);
        assert!(o.renamed.is_none() && !o.skipped);
        assert_eq!(names(&src), vec!["IMG_10.JPG"]);
        assert_eq!(
            names(&dest),
            vec!["IMG_1.JPG", "IMG_1.JPG.XMP", "IMG_1.xmp", "img_1.CR3"]
        );
        assert_eq!(std::fs::read(dest.join("img_1.CR3")).unwrap(), b"raw bytes");
        let (back, failed) = put_back(&o.done);
        assert!(failed.is_empty());
        assert_eq!(back.len(), 4);
        assert!(names(&dest).is_empty());
        assert_eq!(names(&src).len(), 5);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn collisions_get_one_suffix_or_are_skipped() {
        let d = scratch("cull-slots-collide");
        let src = d.join("card");
        let dest = d.join("picks");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        let files = with_sidecars(&shoot(&src));
        // Only the JPEG's name is taken, and the first suffix for the sidecar.
        std::fs::write(dest.join("IMG_1.JPG"), b"older").unwrap();
        std::fs::write(dest.join("IMG_1 (2).xmp"), b"older").unwrap();
        let skipped = place(&files, "IMG_1", &dest, true, Collision::Skip);
        assert!(skipped.skipped && skipped.done.is_empty());
        assert_eq!(names(&dest).len(), 2);
        let o = place(&files, "IMG_1", &dest, true, Collision::Suffix);
        assert!(o.failed.is_empty(), "{:?}", o.failed);
        assert_eq!(o.renamed.as_deref(), Some("img_1 (3)"));
        assert_eq!(
            names(&dest),
            vec![
                "IMG_1 (2).xmp",
                "IMG_1 (3).JPG",
                "IMG_1 (3).JPG.XMP",
                "IMG_1 (3).xmp",
                "IMG_1.JPG",
                "img_1 (3).CR3",
            ]
        );
        // Nothing was overwritten, and the originals stay (a copy).
        assert_eq!(std::fs::read(dest.join("IMG_1.JPG")).unwrap(), b"older");
        assert_eq!(names(&src).len(), 5);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn copy_undo_keeps_a_changed_copy() {
        let d = scratch("cull-slots-copy");
        let src = d.join("card");
        let dest = d.join("picks");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        let files = with_sidecars(&shoot(&src));
        let o = place(&files, "IMG_1", &dest, true, Collision::Suffix);
        assert_eq!(o.done.len(), 4);
        assert!(o.done.iter().all(|p| p.stamp.is_some()));
        // The copy keeps the original's modification time.
        let mt = |p: &Path| std::fs::metadata(p).unwrap().modified().unwrap();
        assert_eq!(mt(&dest.join("IMG_1.JPG")), mt(&src.join("IMG_1.JPG")));
        // No temporary file is left behind.
        assert!(!names(&dest).iter().any(|n| n.starts_with('.')));
        // Someone edits one copy.
        std::fs::write(dest.join("IMG_1.xmp"), b"<x:xmpmeta rating='5'/>").unwrap();
        let (removed, kept, failed) = remove_copies(&o.done);
        assert!(failed.is_empty());
        assert_eq!(removed.len(), 3);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].file_name().unwrap(), "IMG_1.xmp");
        assert_eq!(names(&dest), vec!["IMG_1.xmp"]);
        assert_eq!(names(&src).len(), 5);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn same_folder_and_missing_destination() {
        let d = scratch("cull-slots-same");
        let files = with_sidecars(&shoot(&d));
        let o = place(&files, "IMG_1", &d, false, Collision::Suffix);
        assert!(o.same_folder && o.done.is_empty() && o.failed.is_empty());
        let o = place(
            &files,
            "IMG_1",
            &d.join("nowhere"),
            false,
            Collision::Suffix,
        );
        assert_eq!(o.failed.len(), 4);
        assert_eq!(names(&d).len(), 5);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A symbolic link in the destination with the item's name is a taken name: it is never
    /// written through, and what it points at is left alone.
    #[cfg(unix)]
    #[test]
    fn links_in_the_destination_are_not_followed() {
        let d = scratch("cull-slots-link");
        let src = d.join("card");
        let dest = d.join("picks");
        let outside = d.join("outside");
        for p in [&src, &dest, &outside] {
            std::fs::create_dir_all(p).unwrap();
        }
        std::fs::write(outside.join("victim.jpg"), b"keep me").unwrap();
        std::os::unix::fs::symlink(outside.join("victim.jpg"), dest.join("IMG_1.JPG")).unwrap();
        // A dangling one too.
        std::os::unix::fs::symlink(outside.join("nothing"), dest.join("img_1.CR3")).unwrap();
        let files = with_sidecars(&shoot(&src));
        let o = place(&files, "IMG_1", &dest, true, Collision::Suffix);
        assert!(o.failed.is_empty(), "{:?}", o.failed);
        assert!(o.renamed.is_some());
        assert_eq!(
            std::fs::read(outside.join("victim.jpg")).unwrap(),
            b"keep me"
        );
        assert_eq!(names(&outside), vec!["victim.jpg"]);
        // And a direct transfer onto the link is refused.
        assert!(transfer(&src.join("IMG_10.JPG"), &dest.join("IMG_1.JPG"), false).is_err());
        assert!(src.join("IMG_10.JPG").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn stems_split_without_case() {
        assert_eq!(split_stem("img_1.CR3", "IMG_1"), ("img_1", ".CR3"));
        assert_eq!(split_stem("IMG_1.JPG.xmp", "IMG_1"), ("IMG_1", ".JPG.xmp"));
        assert_eq!(split_stem("other.jpg", "IMG_1"), ("other", ".jpg"));
        assert_eq!(split_stem("čšř.jpg", "ČŠŘ"), ("čšř", ".jpg"));
    }
}
