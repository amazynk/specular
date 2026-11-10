//! Companion files beside an opened file: Shapefile parts, world files and `.prj`
//! projections, GDAL `.aux.xml` metadata, ENVI and ESRI `.hdr` headers and the raw data
//! they describe.
//!
//! A sidecar shares the file's stem (`scene.jpg` → `scene.jgw`, `scene.prj`) or its whole
//! name (`scene.jpg.aux.xml`); extensions match whatever their case. Only regular files are
//! read, read-only, and each up to a size limit. Names are looked up in a listing of the
//! folder, read once and kept while the folder's modification time stays the same, so
//! browsing a folder of photos costs one listing, not a dozen `stat`s per photo.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use crate::auxxml::AuxXml;
use crate::crs::Crs;
use crate::geotiff::Affine2;

/// Folders whose listings are kept.
const DIRS_KEPT: usize = 8;
/// Entries listed at most per folder (a larger folder is listed partly: sidecars beyond
/// that are found by their exact lower- or upper-case name only).
const MAX_ENTRIES: usize = 500_000;
/// Text sidecars (world files, `.prj`, `.aux.xml`, headers) read at most.
pub const TEXT_MAX: u64 = 16 << 20;

/// A folder listing: lower-case name → name on disk.
struct Listing {
    dir: PathBuf,
    modified: Option<SystemTime>,
    names: Arc<Names>,
}

/// A folder's names by lower-case name, compact: the lower-case names sorted and joined in
/// one string (a 100,000-file folder of photos takes about 1.6 MB instead of about 9 MB as a
/// map of strings), and the name on disk only where it differs.
#[derive(Debug, Default)]
struct Names {
    lower: String,
    /// End of each name in `lower`, in sorted order (the start is the previous end).
    ends: Vec<u32>,
    /// Names on disk that differ from their lower-case form, by position in `ends`.
    on_disk: HashMap<u32, Box<str>>,
}

impl Names {
    /// From the names on disk in listing order; of names equal without regard to case the
    /// first is kept.
    fn new(names: impl IntoIterator<Item = String>) -> Names {
        let mut all: Vec<(String, String)> =
            names.into_iter().map(|n| (n.to_lowercase(), n)).collect();
        // Stable: the first of equal lower-case names stays first.
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all.dedup_by(|b, a| a.0 == b.0);
        let mut out = Names {
            lower: String::with_capacity(all.iter().map(|(l, _)| l.len()).sum()),
            ends: Vec::with_capacity(all.len()),
            on_disk: HashMap::new(),
        };
        for (l, n) in all {
            let Ok(end) = u32::try_from(out.lower.len() + l.len()) else {
                break;
            };
            out.lower.push_str(&l);
            if n != l {
                out.on_disk
                    .insert(out.ends.len() as u32, n.into_boxed_str());
            }
            out.ends.push(end);
        }
        out.on_disk.shrink_to_fit();
        out
    }

    fn at(&self, i: usize) -> &str {
        let start = i.checked_sub(1).map_or(0, |p| self.ends[p] as usize);
        &self.lower[start..self.ends[i] as usize]
    }

    /// The name on disk whose lower-case form is `lower`.
    fn get(&self, lower: &str) -> Option<&str> {
        let (mut lo, mut hi) = (0, self.ends.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.at(mid).cmp(lower) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => {
                    return Some(match self.on_disk.get(&(mid as u32)) {
                        Some(n) => n,
                        None => self.at(mid),
                    });
                }
            }
        }
        None
    }
}

static LISTINGS: LazyLock<Mutex<Vec<Listing>>> = LazyLock::new(|| Mutex::new(Vec::new()));

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// The names in `dir` by lower-case name, from the cache while the folder is unchanged.
fn listing(dir: &Path) -> Option<Arc<Names>> {
    let modified = std::fs::metadata(dir).ok()?.modified().ok();
    if let Ok(l) = LISTINGS.lock()
        && let Some(e) = l
            .iter()
            .find(|e| e.dir == dir && e.modified == modified && modified.is_some())
    {
        return Some(e.names.clone());
    }
    let names = Arc::new(Names::new(
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .take(MAX_ENTRIES)
            .filter_map(|d| d.file_name().into_string().ok()),
    ));
    if let Ok(mut l) = LISTINGS.lock() {
        l.retain(|e| e.dir != dir);
        if l.len() >= DIRS_KEPT {
            l.remove(0);
        }
        l.push(Listing {
            dir: dir.to_path_buf(),
            modified,
            names: names.clone(),
        });
    }
    Some(names)
}

fn is_file(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file())
}

/// The file in `path`'s folder named `name`, matched without regard to case; `None` when
/// there is none or it is not a regular file.
pub fn find_name(path: &Path, name: &str) -> Option<PathBuf> {
    let dir = parent(path);
    // The exact name, then all lower and all upper case, before listing the folder.
    for n in [
        name.to_string(),
        name.to_ascii_lowercase(),
        name.to_ascii_uppercase(),
    ] {
        let p = dir.join(&n);
        if is_file(&p) {
            return Some(p);
        }
    }
    let names = listing(dir)?;
    let n = names.get(&name.to_lowercase())?;
    let p = dir.join(n);
    is_file(&p).then_some(p)
}

/// The sidecar `<stem>.<ext>` of `path` (case-insensitive).
pub fn find(path: &Path, ext: &str) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_str()?;
    find_stem_ext(path, stem, ext)
}

fn find_stem_ext(path: &Path, stem: &str, ext: &str) -> Option<PathBuf> {
    let dir = parent(path);
    // The stem as it is with the extension in lower, upper and the file's own case.
    let own_upper = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.chars().any(|c| c.is_ascii_uppercase()));
    let mut tries = vec![
        format!("{stem}.{}", ext.to_ascii_lowercase()),
        format!("{stem}.{}", ext.to_ascii_uppercase()),
    ];
    if own_upper {
        tries.swap(0, 1);
    }
    for n in &tries {
        let p = dir.join(n);
        if is_file(&p) {
            return Some(p);
        }
    }
    let names = listing(dir)?;
    let n = names.get(&format!("{stem}.{ext}").to_lowercase())?;
    let p = dir.join(n);
    is_file(&p).then_some(p)
}

/// The sidecar `<name>.<suffix>` of `path` (`scene.tif.aux.xml`), case-insensitive.
pub fn find_full(path: &Path, suffix: &str) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    find_stem_ext(path, name, suffix)
}

/// The first of `exts` that exists as `<stem>.<ext>`.
pub fn find_any(path: &Path, exts: &[&str]) -> Option<PathBuf> {
    exts.iter().find_map(|e| find(path, e))
}

/// Reads a sidecar of at most `max` bytes (a regular file only).
pub fn read_limited(path: &Path, max: u64) -> Option<Vec<u8>> {
    use std::io::Read;
    // Checked before opening: opening a FIFO blocks.
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let f = std::fs::File::open(path).ok()?;
    let m = f.metadata().ok()?;
    if !m.is_file() || m.len() > max {
        return None;
    }
    let mut v = Vec::with_capacity(m.len() as usize);
    f.take(max).read_to_end(&mut v).ok()?;
    Some(v)
}

/// Reads a text sidecar (lossy UTF-8, a byte-order mark dropped).
pub fn read_text(path: &Path) -> Option<String> {
    let b = read_limited(path, TEXT_MAX)?;
    let s = String::from_utf8_lossy(&b).into_owned();
    Some(s.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(s))
}

/// Reads the sidecar `<stem>.<ext>` of `path`, if there is one.
pub fn read(path: &Path, ext: &str) -> Option<Vec<u8>> {
    read_limited(&find(path, ext)?, TEXT_MAX)
}

/// World-file extensions for an image extension, in the order GDAL tries them: the first
/// and last letters and `w` (`tif` → `tfw`, `jpeg` → `jgw`), the extension and `w`
/// (`tifw`), then `wld`.
pub fn world_extensions(ext: &str) -> Vec<String> {
    let e = ext.to_ascii_lowercase();
    let mut v = Vec::new();
    let b = e.as_bytes();
    if b.len() >= 2 && b.iter().all(u8::is_ascii_alphanumeric) {
        v.push(format!("{}{}w", b[0] as char, b[b.len() - 1] as char));
        v.push(format!("{e}w"));
    }
    v.push("wld".into());
    v.dedup();
    v
}

/// The world file of an image, if any.
pub fn find_world(path: &Path) -> Option<PathBuf> {
    let ext = path.extension()?.to_str()?;
    world_extensions(ext).iter().find_map(|w| find(path, w))
}

/// The georeference sidecars of a raster: its world file, `.prj` and `.aux.xml`.
#[derive(Clone, Debug, Default)]
pub struct SidecarGeo {
    /// Pixel corner to map: the `.aux.xml` geotransform, else the world file's.
    pub transform: Option<Affine2>,
    /// The `.aux.xml` SRS, else the `.prj`.
    pub crs: Option<Crs>,
    pub aux: Option<AuxXml>,
    /// The sidecar file names used, for the info panel ("scene.jgw", "scene.prj").
    pub files: Vec<String>,
}

impl SidecarGeo {
    pub fn is_empty(&self) -> bool {
        self.transform.is_none() && self.crs.is_none() && self.aux.is_none()
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Reads the georeference sidecars of `path`. `world` says whether to look for a world
/// file (formats that carry their own transform skip it).
pub fn georef(path: &Path, world: bool) -> SidecarGeo {
    let mut g = SidecarGeo::default();
    if let Some(p) = find_full(path, "aux.xml")
        && let Some(t) = read_text(&p)
        && let Some(aux) = crate::auxxml::parse(&t)
    {
        g.files.push(file_name(&p));
        g.transform = aux.transform;
        g.crs = aux.crs.clone();
        g.aux = Some(aux);
    }
    if world
        && g.transform.is_none()
        && let Some(p) = find_world(path)
        && let Some(t) = read_text(&p)
        && let Some(w) = crate::worldfile::parse(&t)
    {
        g.files.push(file_name(&p));
        g.transform = Some(w);
    }
    if g.crs.is_none()
        && let Some(p) = find(path, "prj")
        && let Some(t) = read_text(&p)
        && let Some(c) = crate::worldfile::parse_prj(&t)
    {
        g.files.push(file_name(&p));
        g.crs = Some(c);
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compact listing finds every name by its lower-case form, keeps the name on disk,
    /// and of names equal without regard to case the first listed.
    #[test]
    fn names_find_by_lower_case() {
        let n = Names::new(
            [
                "b.TFW",
                "a.jpg",
                "Scene.PRJ",
                "SCENE.prj",
                "c.wld",
                "Ünï.TXT",
            ]
            .map(String::from),
        );
        assert_eq!(n.get("a.jpg"), Some("a.jpg"));
        assert_eq!(n.get("b.tfw"), Some("b.TFW"));
        assert_eq!(n.get("scene.prj"), Some("Scene.PRJ"));
        assert_eq!(n.get("c.wld"), Some("c.wld"));
        assert_eq!(n.get("ünï.txt"), Some("Ünï.TXT"));
        assert_eq!(n.get("d.jpg"), None);
        assert_eq!(n.get(""), None);
        assert_eq!(n.ends.len(), 5);
        assert_eq!(n.on_disk.len(), 3);
        assert_eq!(Names::new(Vec::new()).get("a"), None);
    }

    /// A FIFO named like a sidecar is not opened (that blocks until a writer comes).
    #[cfg(unix)]
    #[test]
    fn a_fifo_sidecar_is_not_read() {
        let d = std::env::temp_dir().join(format!("specular-gis-fifo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let fifo = d.join("a.tfw");
        let ok = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|s| s.success());
        assert!(ok, "mkfifo");
        let (tx, rx) = std::sync::mpsc::channel();
        let p = fifo.clone();
        #[allow(clippy::disallowed_methods)]
        std::thread::spawn(move || {
            let _ = tx.send(read_limited(&p, 1 << 20).is_none());
        });
        let none = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("blocked on a FIFO");
        assert!(none);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn world_names() {
        assert_eq!(world_extensions("tif"), ["tfw", "tifw", "wld"]);
        assert_eq!(world_extensions("JPEG"), ["jgw", "jpegw", "wld"]);
        assert_eq!(world_extensions("png"), ["pgw", "pngw", "wld"]);
        assert_eq!(world_extensions("bil"), ["blw", "bilw", "wld"]);
    }

    /// The found sidecar is `want`. On a case-insensitive file system (Windows, macOS) the first
    /// spelling tried already finds the file, so only the case may differ: the same file.
    fn same(found: Option<PathBuf>, want: PathBuf) {
        let k = |p: &Path| {
            let s = p.to_string_lossy();
            if cfg!(any(windows, target_os = "macos")) {
                s.to_lowercase()
            } else {
                s.into_owned()
            }
        };
        assert_eq!(found.as_deref().map(k), Some(k(&want)));
    }

    #[test]
    fn finds_case_insensitively() {
        let dir = std::env::temp_dir().join(format!("specular-sidecar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let img = dir.join("Scene.JPG");
        std::fs::write(&img, b"x").unwrap();
        std::fs::write(dir.join("Scene.JgW"), b"1\n0\n0\n-1\n10\n20\n").unwrap();
        std::fs::write(dir.join("scene.PRJ"), b"GEOGCS[]").unwrap();
        std::fs::write(dir.join("Scene.JPG.aux.xml"), b"<PAMDataset/>").unwrap();
        same(find_world(&img), dir.join("Scene.JgW"));
        // Any case matches; the stem's own case first.
        same(find(&img, "prj"), dir.join("scene.PRJ"));
        std::fs::write(dir.join("Scene.prj"), b"GEOGCS[]").unwrap();
        same(find(&img, "prj"), dir.join("Scene.prj"));
        same(find_full(&img, "aux.xml"), dir.join("Scene.JPG.aux.xml"));
        assert_eq!(read(&img, "prj").as_deref(), Some(&b"GEOGCS[]"[..]));
        // Folders are not files.
        std::fs::create_dir_all(dir.join("Scene.wld")).unwrap();
        assert_eq!(find(&img, "wld"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
