//! What is on a source: media files under its DCIM folders, grouped into shots by folder and
//! name stem, with sidecars attached and the facts import needs (capture date, camera, a
//! content key per file).

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use crate::{Key, Ledger};

/// Camera and phone stills.
const PHOTO: &[&str] = &[
    "jpg", "jpeg", "jpe", "heic", "heif", "hif", "avif", "jxl", "png", "webp", "gif", "tif", "tiff",
];
/// Camera RAW (DNG included: it pairs with a JPEG like any other RAW).
const RAW: &[&str] = &[
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "rwl", "raf",
    "pef", "srw", "x3f", "3fr", "fff", "iiq", "erf", "mef", "mos", "kdc", "dcr", "mrw", "gpr",
];
/// Video, including camera AVCHD streams and 360° files.
const VIDEO: &[&str] = &[
    "mp4", "mov", "m4v", "mts", "m2ts", "avi", "3gp", "mkv", "webm", "insv", "lrf", "mpg", "mpeg",
    "wmv", "mxf",
];
/// Sidecars that follow their file (`.lrv`, GoPro's low-resolution proxy, only on request).
const SIDECAR: &[&str] = &["xmp", "aae", "thm"];
const LOW_RES_VIDEO: &str = "lrv";

/// What a media file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MediaKind {
    Photo,
    Raw,
    Video,
}

impl MediaKind {
    /// The kind for a file name's extension.
    pub fn of(path: &Path) -> Option<MediaKind> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        let ext = ext.as_str();
        if PHOTO.contains(&ext) {
            Some(MediaKind::Photo)
        } else if RAW.contains(&ext) {
            Some(MediaKind::Raw)
        } else if VIDEO.contains(&ext) {
            Some(MediaKind::Video)
        } else {
            None
        }
    }
}

/// One media file on the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Media {
    pub path: PathBuf,
    pub size: u64,
    pub mtime: SystemTime,
    pub kind: MediaKind,
    /// Sidecars that follow it.
    pub sidecars: Vec<PathBuf>,
    /// Content key (zero until [`identify`]).
    pub key: Key,
}

/// Files that belong together: a RAW and its JPEG, a Live Photo's still and movie, with their
/// sidecars. The unit of selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    /// Members, RAW first, then stills, then videos.
    pub members: Vec<Media>,
    /// Capture date (local wall clock) from metadata, else the modification time.
    pub date: [u32; 6],
    /// `date` came from the file's metadata (not its modification time).
    pub dated: bool,
    pub camera: Option<String>,
}

impl Shot {
    pub fn bytes(&self) -> u64 {
        self.members
            .iter()
            .map(|m| {
                m.size
                    + m.sidecars
                        .iter()
                        .filter_map(|s| std::fs::metadata(s).ok())
                        .map(|md| md.len())
                        .sum::<u64>()
            })
            .sum()
    }

    /// The file to show for it: the still if there is one (fast to thumbnail), else the RAW,
    /// else the video.
    pub fn preview(&self) -> &Path {
        let m = self
            .members
            .iter()
            .find(|m| m.kind == MediaKind::Photo)
            .unwrap_or(&self.members[0]);
        &m.path
    }

    /// A RAW with a still beside it.
    pub fn is_pair(&self) -> bool {
        self.has(MediaKind::Raw) && self.has(MediaKind::Photo)
    }

    pub fn has(&self, k: MediaKind) -> bool {
        self.members.iter().any(|m| m.kind == k)
    }

    /// Members not yet in `ledger`.
    pub fn new_members<'a>(&'a self, ledger: &'a Ledger) -> impl Iterator<Item = &'a Media> {
        self.members.iter().filter(|m| !ledger.contains(&m.key))
    }
}

/// The case-folded name up to the first dot: `IMG_0001.CR2.xmp` and `img_0001.jpg` share
/// `img_0001`.
fn stem(name: &str) -> String {
    name.split('.').next().unwrap_or(name).to_lowercase()
}

fn ext(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// The files under `dir` (not hidden, at most `depth` folders down), sorted.
fn walk(dir: &Path, depth: u32, cancel: &AtomicBool, out: &mut Vec<PathBuf>) {
    if cancel.load(Ordering::Relaxed) {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        // `.Trashes`, `._IMG_0001.JPG` (AppleDouble), `.thumbnails`.
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(t) = e.file_type() else { continue };
        if t.is_dir() {
            if depth > 0 {
                walk(&e.path(), depth - 1, cancel, out);
            }
        } else if t.is_file() {
            out.push(e.path());
        }
    }
}

/// The shots under `roots`, without their facts read ([`identify`] does that). Sidecars named
/// for one member (`IMG_0001.CR2.xmp`) follow that member; a sidecar named for the stem
/// follows the RAW (`.xmp`), the still (`.aae`) or the video (`.thm`, `.lrv`).
pub fn list(roots: &[PathBuf], low_res_videos: bool, cancel: &AtomicBool) -> Vec<Shot> {
    let mut files = Vec::new();
    for r in roots {
        walk(r, 6, cancel, &mut files);
    }
    // Group by (folder, stem).
    let mut groups: BTreeMap<(PathBuf, String), Vec<PathBuf>> = BTreeMap::new();
    for f in files {
        let Some(name) = f.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let parent = f.parent().map(Path::to_path_buf).unwrap_or_default();
        groups.entry((parent, stem(&name))).or_default().push(f);
    }
    let mut shots = Vec::new();
    for (_, files) in groups {
        let mut members = Vec::new();
        let mut sidecars = Vec::new();
        for f in files {
            let e = ext(&f);
            if let Some(kind) = MediaKind::of(&f) {
                let Ok(md) = std::fs::metadata(&f) else {
                    continue;
                };
                members.push(Media {
                    path: f,
                    size: md.len(),
                    mtime: md.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    kind,
                    sidecars: Vec::new(),
                    key: [0; 32],
                });
            } else if SIDECAR.contains(&e.as_str()) || (low_res_videos && e == LOW_RES_VIDEO) {
                sidecars.push(f);
            }
        }
        if members.is_empty() {
            continue;
        }
        // RAW first (the stem's `.xmp` is the RAW's), then stills, then videos; by name within.
        members.sort_by_key(|m| match m.kind {
            MediaKind::Raw => 0,
            MediaKind::Photo => 1,
            MediaKind::Video => 2,
        });
        for s in sidecars {
            let name = s
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            // `IMG_0001.CR2.xmp`: the member whose full name it extends.
            let named = members.iter().position(|m| {
                m.path.file_name().is_some_and(|mn| {
                    let mn = mn.to_string_lossy().to_lowercase();
                    name.len() > mn.len() + 1
                        && name.starts_with(&mn)
                        && name.as_bytes()[mn.len()] == b'.'
                })
            });
            let want = match ext(&s).as_str() {
                "aae" => MediaKind::Photo,
                "thm" | "lrv" => MediaKind::Video,
                _ => MediaKind::Raw,
            };
            let i = named
                .or_else(|| members.iter().position(|m| m.kind == want))
                .unwrap_or(0);
            members[i].sidecars.push(s);
        }
        let mtime = members
            .iter()
            .map(|m| m.mtime)
            .min()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        shots.push(Shot {
            members,
            date: crate::exif::local_mtime(mtime),
            dated: false,
            camera: None,
        });
    }
    shots
}

const EDGE: u64 = 64 * 1024;

/// The content key of a file: blake3 over its first and last 64 KiB, its size and its capture
/// date when the metadata records one. Cheap on slow cards (at most 128 KiB read) and the same
/// whatever the file is called or wherever it is.
pub fn quick_key(path: &Path, size: u64, date: Option<[u32; 6]>) -> std::io::Result<Key> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; EDGE as usize];
    let head = size.min(EDGE) as usize;
    f.read_exact(&mut buf[..head])?;
    h.update(&buf[..head]);
    if size > EDGE {
        let tail = (size - EDGE).min(EDGE);
        f.seek(SeekFrom::Start(size - tail))?;
        f.read_exact(&mut buf[..tail as usize])?;
        h.update(&buf[..tail as usize]);
    }
    h.update(&size.to_le_bytes());
    if let Some(d) = date {
        for v in d {
            h.update(&v.to_le_bytes());
        }
    }
    Ok(*h.finalize().as_bytes())
}

/// Reads a shot's capture date and camera (from the first member that has them) and computes
/// each member's key.
pub fn identify(shot: &mut Shot) -> std::io::Result<()> {
    // Stills first: their Exif is near the start; a RAW's may be too, a video's is at the end.
    let mut order: Vec<usize> = (0..shot.members.len()).collect();
    order.sort_by_key(|&i| match shot.members[i].kind {
        MediaKind::Photo => 0,
        MediaKind::Raw => 1,
        MediaKind::Video => 2,
    });
    for i in order {
        let m = &shot.members[i];
        let f = crate::exif::read(&m.path, m.kind == MediaKind::Video);
        if shot.camera.is_none() {
            shot.camera = f.camera;
        }
        if let Some(d) = f.date {
            shot.date = d;
            shot.dated = true;
            break;
        }
    }
    let date = shot.dated.then_some(shot.date);
    for m in &mut shot.members {
        m.key = quick_key(&m.path, m.size, date)?;
    }
    Ok(())
}

/// The result of [`scan`].
#[derive(Clone, Debug, Default)]
pub struct Found {
    /// Shots with at least one member not imported before, oldest first.
    pub new: Vec<Shot>,
    /// Shots whose members were all imported before.
    pub already: usize,
    /// Files that could not be read.
    pub unreadable: Vec<PathBuf>,
}

/// Lists and identifies the shots under `roots`, keeping those with something new.
/// `progress(done, total)` is called as shots are identified.
pub fn scan(
    roots: &[PathBuf],
    low_res_videos: bool,
    ledger: &Ledger,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(usize, usize),
) -> Found {
    let shots = list(roots, low_res_videos, cancel);
    let total = shots.len();
    let mut found = Found::default();
    for (i, mut s) in shots.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        match identify(&mut s) {
            Ok(()) => {
                if s.new_members(ledger).next().is_some() {
                    found.new.push(s);
                } else {
                    found.already += 1;
                }
            }
            Err(_) => found
                .unreadable
                .extend(s.members.iter().map(|m| m.path.clone())),
        }
        progress(i + 1, total);
    }
    found.new.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.preview().cmp(b.preview()))
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{card, fresh, write};

    fn names(s: &Shot) -> Vec<String> {
        s.members
            .iter()
            .map(|m| {
                let mut n = m.path.file_name().unwrap().to_string_lossy().into_owned();
                for c in &m.sidecars {
                    n += "+";
                    n += &c.file_name().unwrap().to_string_lossy();
                }
                n
            })
            .collect()
    }

    #[test]
    fn shots_pairs_and_sidecars() {
        let root = fresh("pairs");
        let c = card(&root);
        let no = AtomicBool::new(false);
        let shots = list(&[c.join("DCIM")], false, &no);
        let all: Vec<_> = shots.iter().map(names).collect();
        assert_eq!(
            all,
            [
                vec!["IMG_0001.CR2+IMG_0001.CR2.xmp", "IMG_0001.JPG"],
                vec!["IMG_0002.JPG"],
                vec!["MVI_0003.MP4+MVI_0003.THM"],
                vec!["IMG_0101.JPG"],
            ]
        );
        assert!(shots[0].is_pair());
        assert_eq!(shots[0].preview().file_name().unwrap(), "IMG_0001.JPG");

        // Low-resolution proxies on request; a stem-named XMP follows the RAW, an AAE the still.
        let d = c.join("DCIM/100CANON");
        write(&d.join("IMG_0002.AAE"), b"aae");
        write(&d.join("IMG_0001.xmp"), b"x");
        write(&d.join("IMG_0004.xmp"), b"orphan");
        let shots = list(&[c.join("DCIM")], true, &no);
        let all: Vec<_> = shots.iter().map(names).collect();
        assert_eq!(
            all,
            [
                vec!["IMG_0001.CR2+IMG_0001.CR2.xmp+IMG_0001.xmp", "IMG_0001.JPG"],
                vec!["IMG_0002.JPG+IMG_0002.AAE"],
                vec!["MVI_0003.MP4+MVI_0003.LRV+MVI_0003.THM"],
                vec!["IMG_0101.JPG"],
            ]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn live_photo_is_one_shot() {
        let root = fresh("live");
        let d = root.join("iPhone/DCIM/100APPLE");
        write(&d.join("IMG_0007.HEIC"), b"heic");
        write(&d.join("IMG_0007.MOV"), b"mov");
        write(&d.join("IMG_0007.AAE"), b"aae");
        write(&d.join("._IMG_0007.HEIC"), b"appledouble");
        let shots = list(&[root.join("iPhone/DCIM")], false, &AtomicBool::new(false));
        assert_eq!(
            shots.iter().map(names).collect::<Vec<_>>(),
            [vec!["IMG_0007.HEIC+IMG_0007.AAE", "IMG_0007.MOV"]]
        );
        assert!(!shots[0].is_pair(), "a pair is RAW + still");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn identify_reads_dates_cameras_and_keys() {
        let root = fresh("ident");
        let c = card(&root);
        let mut shots = list(&[c.join("DCIM")], false, &AtomicBool::new(false));
        for s in &mut shots {
            identify(s).unwrap();
        }
        assert_eq!(shots[0].date, [2024, 3, 9, 14, 5, 59]);
        assert!(shots[0].dated);
        assert_eq!(shots[0].camera.as_deref(), Some("Canon EOS R5"));
        assert!(shots[2].dated, "the video's movie header");
        assert_eq!(shots[2].camera, None);
        // Keys differ per file, and do not depend on the name or folder.
        let k = shots[0].members[1].key;
        assert_ne!(k, shots[0].members[0].key);
        let moved = root.join("elsewhere/renamed.jpg");
        write(&moved, &std::fs::read(&shots[0].members[1].path).unwrap());
        let size = std::fs::metadata(&moved).unwrap().len();
        assert_eq!(quick_key(&moved, size, Some(shots[0].date)).unwrap(), k);
        let _ = std::fs::remove_dir_all(root);
    }
}
