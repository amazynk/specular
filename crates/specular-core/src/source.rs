use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

enum Bytes {
    Mapped(memmap2::Mmap),
    Owned(Vec<u8>),
    /// A range of another source (an entry stored uncompressed in an archive).
    Slice {
        parent: std::sync::Arc<Source>,
        range: std::ops::Range<usize>,
    },
    /// A folder read by its decoder (an OME-Zarr store, a Relight image, a CASA image).
    Folder,
}

/// Opens `path` for reading if it is a regular file (links followed). Opening a FIFO blocks
/// until a writer comes and reading a device may never end, so anything that reads a file it
/// found by name (a sidecar, a marker, a folder's metadata document) opens it with this; other
/// kinds of file are an `InvalidInput` error, at once.
pub fn open_regular(path: &Path) -> io::Result<File> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    File::open(path)
}

/// Whether `dir` is a Zarr group or array (its metadata document is there).
pub fn is_zarr_folder(dir: &Path) -> bool {
    ["zarr.json", ".zattrs", ".zgroup", ".zarray"]
        .iter()
        .any(|f| dir.join(f).is_file())
}

/// Whether `dir` is a CNR-ISTI Relight image: an `info.json` with a basis type and colour
/// space (checked on its first 4 KiB, without parsing).
pub fn is_relight_folder(dir: &Path) -> bool {
    use std::io::Read;
    let Ok(f) = open_regular(&dir.join("info.json")) else {
        return false;
    };
    let mut head = Vec::with_capacity(4096);
    if f.take(4096).read_to_end(&mut head).is_err() {
        return false;
    }
    is_relight_head(&head)
}

/// Whether the start of a JSON document looks like a Relight `info.json`: an object with
/// `"type"` and `"colorspace"` keys (the decoder checks the rest).
pub fn is_relight_head(head: &[u8]) -> bool {
    let text = String::from_utf8_lossy(head);
    let t = text.trim_start();
    t.starts_with('{') && t.contains("\"colorspace\"") && t.contains("\"type\"")
}

/// Whether `dir` is a CASA image: a casacore table whose `table.dat` starts with the AipsIO
/// magic and a `Table` object and names a `coords` keyword (checked on its first 256 KiB,
/// without parsing; the decoder checks the rest).
pub fn is_casa_folder(dir: &Path) -> bool {
    use std::io::Read;
    let Ok(f) = open_regular(&dir.join("table.dat")) else {
        return false;
    };
    let mut head = Vec::with_capacity(4096);
    if f.take(256 << 10).read_to_end(&mut head).is_err() {
        return false;
    }
    is_casa_head(&head)
}

/// Whether the start of a `table.dat` describes a CASA image (see [`is_casa_folder`]).
pub fn is_casa_head(head: &[u8]) -> bool {
    head.len() >= 18
        && head[..4] == [0xbe; 4]
        && &head[12..17] == b"Table"
        && head.windows(10).any(|w| w == b"\0\0\0\x06coords")
}

/// Whether `dir` is an Esri File Geodatabase: a folder named `*.gdb` holding its system
/// catalog table.
pub fn is_filegdb_folder(dir: &Path) -> bool {
    dir.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("gdb"))
        && dir.join("a00000001.gdbtable").is_file()
}

/// Whether `dir` is an Arc/Info binary grid: a folder holding `hdr.adf` (starting
/// `GRID1.2`) and the first tile's index `w001001x.adf` (names in any case).
pub fn is_aig_folder(dir: &Path) -> bool {
    use std::io::Read;
    let find = |name: &str| {
        [name.to_string(), name.to_ascii_uppercase()]
            .into_iter()
            .map(|n| dir.join(n))
            .find(|p| p.is_file())
    };
    let (Some(hdr), Some(_)) = (find("hdr.adf"), find("w001001x.adf")) else {
        return false;
    };
    let mut head = [0u8; 8];
    open_regular(&hdr).is_ok_and(|mut f| f.read_exact(&mut head).is_ok() && &head == b"GRID1.2\0")
}

/// Whether `dir` is a Bruker NMR experiment (an `acqus` with its `fid` or `ser`) or one of its
/// processing folders (`pdata/<n>`: `procs` with `1r` or `2rr`).
pub fn is_bruker_folder(dir: &Path) -> bool {
    let f = |n: &str| dir.join(n).is_file();
    (f("acqus") && (f("fid") || f("ser"))) || (f("procs") && (f("1r") || f("2rr")))
}

/// Extensions of Gerber and Excellon files (Protel/Altium, KiCad, Eagle and generic names).
pub const GERBER_EXTENSIONS: &[&str] = &[
    "gbr", "ger", "gbx", "pho", "art", "gtl", "gbl", "gto", "gbo", "gts", "gbs", "gtp", "gbp",
    "gko", "gm1", "gm2", "gm3", "gm4", "gm13", "gm15", "gml", "gp1", "gp2", "g1", "g2", "g3", "g4",
    "g5", "g6", "gd1", "gg1", "cmp", "sol", "stc", "sts", "plc", "pls", "crc", "crs", "dim", "drl",
    "xln", "drd", "exc", "gbrjob",
];

/// Files beside a board's Gerbers that say nothing either way (reports, pick and place,
/// pictures, the job file's companions).
const BOARD_NEUTRAL: &[&str] = &[
    "txt", "rpt", "md", "pdf", "csv", "pos", "ipc", "xlsx", "xls", "png", "jpg", "jpeg", "zip",
    "json", "html", "htm", "log", "ini", "lst", "rep", "extrep", "apr", "d356", "rul", "drc",
];

/// Whether `dir` holds one board's fabrication files: at least two Gerber or drill files by
/// name, one of them copper or the outline, and they are at least as many as the other
/// files there (reports, pictures and the like aside). Name-only; at most 400 entries read.
pub fn is_gerber_folder(dir: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    let (mut gerbers, mut others, mut key) = (0usize, 0usize, false);
    for e in rd.flatten().take(400) {
        let name = e.file_name();
        let Some(name) = name.to_str() else {
            others += 1;
            continue;
        };
        if name.starts_with('.') || !e.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        let ext = lower.rsplit_once('.').map_or("", |(_, e)| e);
        if GERBER_EXTENSIONS.contains(&ext) {
            gerbers += 1;
            key |= matches!(
                ext,
                "gtl" | "gbl" | "gko" | "gm1" | "gml" | "cmp" | "sol" | "dim"
            ) || [
                "cu.",
                "copper",
                "edge_cuts",
                "edge.cuts",
                "outline",
                "profile",
            ]
            .iter()
            .any(|k| lower.contains(k));
        } else if !BOARD_NEUTRAL.contains(&ext) {
            others += 1;
            // A folder of photos or documents: no need to read on.
            if others > 32 && gerbers == 0 {
                return false;
            }
        }
    }
    gerbers >= 2 && key && gerbers >= others
}

/// Whether `dir` is an ODB++ job: `matrix/matrix` and a `steps` folder in it.
pub fn is_odb_folder(dir: &Path) -> bool {
    let m = dir.join("matrix");
    (m.join("matrix").is_file() || m.join("matrix.z").is_file() || m.join("matrix.Z").is_file())
        && dir.join("steps").is_dir()
}

/// Whether `dir` is a folder that opens as one image or map (an OME-Zarr store, a Relight
/// image, a CASA image, a File Geodatabase), a board's Gerber files or a Bruker NMR
/// experiment.
pub fn is_image_folder(dir: &Path) -> bool {
    is_zarr_folder(dir)
        || is_relight_folder(dir)
        || is_casa_folder(dir)
        || is_filegdb_folder(dir)
        || is_bruker_folder(dir)
        || is_gerber_folder(dir)
        || is_odb_folder(dir)
        || is_sentinel2_folder(dir)
        || is_tiles3d_folder(dir)
        || is_aig_folder(dir)
}

/// Whether `dir` is a 3D Tiles tileset: a folder holding `tileset.json`.
pub fn is_tiles3d_folder(dir: &Path) -> bool {
    std::fs::metadata(dir.join("tileset.json")).is_ok_and(|m| m.is_file())
}

/// Whether `dir` is a Sentinel-2 product in its SAFE layout: a folder holding the product
/// metadata `MTD_MSIL1C.xml` or `MTD_MSIL2A.xml`.
pub fn is_sentinel2_folder(dir: &Path) -> bool {
    ["MTD_MSIL2A.xml", "MTD_MSIL1C.xml"]
        .iter()
        .any(|n| std::fs::metadata(dir.join(n)).is_ok_and(|m| m.is_file()))
}

/// Maps a file read-only. A plain `read` of the 105 MB goats file costs 70-86 ms even from a warm
/// page cache; mapping it and scanning every byte costs 7.6 ms (UNSAFE.md).
#[allow(unsafe_code)]
fn map_file(file: &File) -> io::Result<memmap2::Mmap> {
    // SAFETY: the map is read-only and never handed out mutably. If another process truncates
    // the file while it is mapped, reads past the new end raise SIGBUS; that is the accepted
    // risk of every mmap-based viewer, recorded in UNSAFE.md.
    unsafe { memmap2::MmapOptions::new().map(file) }
}

/// The complete bytes of an image file.
///
/// Files are memory-mapped, so opening a 100 MB JPEG costs nothing until pages are touched,
/// and the page cache is shared with the OS. Tiny or special files are read into memory.
pub struct Source {
    path: PathBuf,
    bytes: Bytes,
    modified: Option<std::time::SystemTime>,
    /// How many files this one is inside (0: a file the user opened); see [`MAX_NESTING`].
    nesting: u8,
    /// The memory budget's share of bytes read into memory, held as long as they are
    /// ([`Source::with_reservation`]).
    _reserved: Option<crate::budget::Reservation>,
    /// The file is still being filled (a stream's web file): reads wait for its bytes.
    _filling: Option<crate::fill::Mapped>,
}

/// The deepest a file may sit inside other files (a thumbnail inside a document inside an
/// archive...). Decoders open what a file embeds through the same entry point that opened
/// the file, so a file that contains itself (a ZIP quine as an OpenDocument thumbnail, an
/// SVG `data:` image nesting SVGs) would otherwise recurse until the stack overflows.
pub const MAX_NESTING: u8 = 6;

thread_local! {
    /// The nesting of the source being opened on this thread ([`Source::enter`]): sources
    /// made meanwhile are nested one deeper.
    static AMBIENT: std::cell::Cell<Option<u8>> = const { std::cell::Cell::new(None) };
}

/// The nesting of a source made now: one below the source being opened on this thread.
fn ambient_nesting() -> u8 {
    AMBIENT.with(|a| a.get().map_or(0, |d| d.saturating_add(1)))
}

/// Restores the previous ambient nesting when dropped ([`Source::enter`]).
pub struct NestingGuard(Option<u8>);

impl Drop for NestingGuard {
    fn drop(&mut self) {
        AMBIENT.with(|a| a.set(self.0));
    }
}

impl Source {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Source> {
        let path = path.as_ref().to_path_buf();
        let t0 = std::time::Instant::now();
        // Only regular files: opening a FIFO or a device blocks, and reading one may never end.
        let kind = match std::fs::metadata(&path) {
            Ok(k) => k,
            // An offline original with a local copy: the copy is read.
            Err(e) => match crate::local_copy::stand_in(&path) {
                Some(copy) => {
                    specular_log::debug!(target: specular_log::IO, "open {}: offline, reading its local copy", path.display());
                    return Source::open(copy);
                }
                None => return Err(e),
            },
        };
        // A Zarr store is a folder of chunk files: the source has no bytes of its own and the
        // decoder reads the folder.
        if kind.is_dir() && is_image_folder(&path) {
            return Ok(Source {
                path,
                bytes: Bytes::Folder,
                modified: kind.modified().ok(),
                nesting: ambient_nesting(),
                _reserved: None,
                _filling: None,
            });
        }
        if !kind.is_file() {
            specular_log::debug!(target: specular_log::IO, "open {}: not a regular file", path.display());
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        let file = File::open(&path)?;
        let meta = file.metadata()?;
        let modified = meta.modified().ok();
        // A file being filled is mapped (reads wait for its bytes through the mapping);
        // when it cannot be, it is read once whole.
        let filling = crate::fill::of_path(&path);
        if let Some(f) = &filling
            && meta.len() == f.len()
            && !f.is_empty()
            && let Ok(m) = map_file(&file)
        {
            let mapped = crate::fill::map(&m, std::sync::Arc::clone(f));
            return Ok(Source {
                path,
                bytes: Bytes::Mapped(m),
                modified,
                nesting: ambient_nesting(),
                _reserved: None,
                _filling: Some(mapped),
            });
        }
        if let Some(f) = &filling
            && !f.wait(0, f.len())
        {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the file did not arrive in time",
            ));
        }
        let bytes = if meta.is_file() && meta.len() >= 64 * 1024 {
            match map_file(&file) {
                Ok(m) => {
                    #[cfg(unix)]
                    let _ = m.advise(memmap2::Advice::Sequential);
                    // macOS: start reading the whole file asynchronously (F_RDADVISE), so the
                    // decoder's page faults find it in flight. No-op elsewhere.
                    specular_platform::advise_will_read(&file, 0, meta.len());
                    Bytes::Mapped(m)
                }
                Err(_) => Bytes::Owned(std::fs::read(&path)?),
            }
        } else {
            use std::io::Read;
            let mut v = Vec::with_capacity(meta.len() as usize);
            let mut f = file;
            f.read_to_end(&mut v)?;
            Bytes::Owned(v)
        };
        specular_log::debug!(
            target: specular_log::IO,
            "open {}: {} bytes, {} in {:.2} ms",
            path.display(),
            meta.len(),
            if matches!(bytes, Bytes::Mapped(_)) { "mapped" } else { "read" },
            t0.elapsed().as_secs_f64() * 1e3
        );
        Ok(Source {
            path,
            bytes,
            modified,
            nesting: ambient_nesting(),
            _reserved: None,
            _filling: None,
        })
    }

    /// Wrap bytes that are already in memory (tests, clipboard).
    pub fn from_vec(path: impl Into<PathBuf>, data: Vec<u8>) -> Source {
        Source {
            path: path.into(),
            bytes: Bytes::Owned(data),
            modified: None,
            nesting: ambient_nesting(),
            _reserved: None,
            _filling: None,
        }
    }

    /// A view of `part`, which must lie inside `parent.bytes()` (an archive entry stored
    /// uncompressed: no copy). `None` if it does not.
    pub fn from_slice(
        path: impl Into<PathBuf>,
        parent: std::sync::Arc<Source>,
        part: &[u8],
    ) -> Option<Source> {
        let base = parent.bytes().as_ptr() as usize;
        let start = (part.as_ptr() as usize).checked_sub(base)?;
        let end = start.checked_add(part.len())?;
        if end > parent.len() {
            return None;
        }
        let modified = parent.modified;
        let nesting = ambient_nesting().max(parent.nesting.saturating_add(1));
        Some(Source {
            path: path.into(),
            bytes: Bytes::Slice {
                parent,
                range: start..end,
            },
            modified,
            nesting,
            _reserved: None,
            _filling: None,
        })
    }

    /// Wraps bytes read out of `parent` (a compressed archive entry); the modification time is
    /// the parent's.
    pub fn from_entry(path: impl Into<PathBuf>, parent: &Source, data: Vec<u8>) -> Source {
        Source {
            path: path.into(),
            bytes: Bytes::Owned(data),
            modified: parent.modified,
            nesting: ambient_nesting().max(parent.nesting.saturating_add(1)),
            _reserved: None,
            _filling: None,
        }
    }

    /// This source holding `reserved` (the budget's share of its bytes) until it is dropped.
    pub fn with_reservation(mut self, reserved: crate::budget::Reservation) -> Source {
        self._reserved = Some(reserved);
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many files this one is inside: 0 for a file the user opened, one more for each
    /// file it was read out of (see [`MAX_NESTING`]).
    pub fn nesting(&self) -> u8 {
        self.nesting
    }

    /// This source, read out of `parent`: at least one deeper than it. For what a decoder
    /// opens while decoding (not while opening, which [`Source::enter`] covers), or on
    /// another thread.
    pub fn within(mut self, parent: &Source) -> Source {
        self.nesting = self.nesting.max(parent.nesting.saturating_add(1));
        self
    }

    /// While the guard lives, sources made on this thread are nested one below this one (a
    /// decoder opening what the file embeds). Call where the file is opened.
    pub fn enter(&self) -> NestingGuard {
        NestingGuard(AMBIENT.with(|a| a.replace(Some(self.nesting))))
    }

    pub fn bytes(&self) -> &[u8] {
        match &self.bytes {
            Bytes::Mapped(m) => m,
            Bytes::Owned(v) => v,
            Bytes::Slice { parent, range } => &parent.bytes()[range.clone()],
            Bytes::Folder => &[],
        }
    }

    /// The source is a folder its decoder reads (an OME-Zarr store, a Relight image), with
    /// no bytes.
    pub fn is_folder(&self) -> bool {
        matches!(self.bytes, Bytes::Folder)
    }

    pub fn len(&self) -> usize {
        self.bytes().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn modified(&self) -> Option<std::time::SystemTime> {
        self.modified
    }

    /// Ask the kernel to start reading the whole file in the background.
    pub fn prefetch(&self) {
        #[cfg(unix)]
        if let Bytes::Mapped(m) = &self.bytes {
            let _ = m.advise(memmap2::Advice::WillNeed);
        }
    }

    /// Tells the kernel reads will jump around (archives: pages are read in viewing order,
    /// not file order), which turns read-ahead down.
    pub fn advise_random(&self) {
        #[cfg(unix)]
        if let Bytes::Mapped(m) = &self.bytes {
            let _ = m.advise(memmap2::Advice::Random);
        }
    }
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Source")
            .field("path", &self.path)
            .field("len", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod slice_tests {
    use super::Source;
    use std::sync::Arc;

    /// Sources read out of a file, or made while it is being opened, are nested below it;
    /// leaving the open restores the outer nesting.
    #[test]
    fn nesting_follows_parents_and_opens() {
        let top = Arc::new(Source::from_vec("a.odt", vec![1, 2, 3]));
        assert_eq!(top.nesting(), 0);
        let slice = Source::from_slice("t.png", top.clone(), &top.bytes()[1..]).unwrap();
        assert_eq!(slice.nesting(), 1);
        assert_eq!(Source::from_entry("e", &slice, vec![]).nesting(), 2);
        assert_eq!(Source::from_vec("v", vec![]).within(&slice).nesting(), 2);
        {
            let _open = slice.enter();
            assert_eq!(Source::from_vec("inner", vec![]).nesting(), 2);
            let deeper = Source::from_vec("inner", vec![]);
            {
                let _open = deeper.enter();
                assert_eq!(Source::from_vec("x", vec![]).nesting(), 3);
            }
            assert_eq!(Source::from_vec("x", vec![]).nesting(), 2);
        }
        assert_eq!(Source::from_vec("after", vec![]).nesting(), 0);
        // Saturates instead of wrapping.
        let mut s = Source::from_vec("deep", vec![]);
        for _ in 0..300 {
            s = Source::from_vec("d", vec![]).within(&s);
        }
        assert_eq!(s.nesting(), u8::MAX);
    }

    #[test]
    fn a_slice_views_its_parent_and_rejects_foreign_bytes() {
        let parent = Arc::new(Source::from_vec("a.cbz", (0u8..100).collect()));
        let part = &parent.bytes()[10..20];
        let s = Source::from_slice("a.cbz/p1.jpg", parent.clone(), part).unwrap();
        assert_eq!(s.bytes(), &(10u8..20).collect::<Vec<_>>()[..]);
        assert_eq!(s.path(), std::path::Path::new("a.cbz/p1.jpg"));
        let other = vec![0u8; 4];
        assert!(Source::from_slice("x", parent, &other).is_none());
    }
}

#[cfg(all(test, unix))]
mod fifo_tests {
    use super::*;

    /// A FIFO where a folder's metadata document or an image is expected is not opened (that
    /// blocks until a writer comes): the checks answer at once.
    #[test]
    fn fifos_are_not_opened() {
        let d = std::env::temp_dir().join(format!("specular-core-fifo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for n in ["info.json", "table.dat", "pipe.png"] {
            let ok = std::process::Command::new("mkfifo")
                .arg(d.join(n))
                .status()
                .is_ok_and(|s| s.success());
            assert!(ok, "mkfifo");
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let dir = d.clone();
        #[allow(clippy::disallowed_methods)]
        std::thread::spawn(move || {
            let r = (
                is_relight_folder(&dir),
                is_casa_folder(&dir),
                is_image_folder(&dir),
                open_regular(&dir.join("pipe.png"))
                    .map(|_| ())
                    .map_err(|e| e.kind()),
            );
            let _ = tx.send(r);
        });
        let r = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("blocked on a FIFO");
        assert_eq!(r, (false, false, false, Err(io::ErrorKind::InvalidInput)));
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod folder_tests {
    use super::*;

    fn folder(name: &str, files: &[&str]) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("specular-core-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for f in files {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        d
    }

    #[test]
    fn boards_and_nmr_experiments_are_folders_that_open() {
        let kicad = folder(
            "kicad",
            &[
                "b-F_Cu.gbr",
                "b-B_Cu.gbr",
                "b-Edge_Cuts.gbr",
                "b-PTH.drl",
                "b-job.gbrjob",
                "README.txt",
            ],
        );
        assert!(is_gerber_folder(&kicad) && is_image_folder(&kicad));
        let protel = folder("protel", &["x.GTL", "x.GBL", "x.GTO", "x.TXT"]);
        assert!(is_gerber_folder(&protel));
        // Two drill files are not a board; nor is a folder of photos with two Gerbers.
        let drills = folder("drills", &["a.drl", "b.drl"]);
        assert!(!is_gerber_folder(&drills));
        let photos = folder("photos", &["a.arw", "b.cr2", "c.nef", "x.gtl", "y.gbl"]);
        assert!(!is_gerber_folder(&photos));
        let nmr = folder("nmr", &["acqus", "fid", "pulseprogram"]);
        assert!(is_bruker_folder(&nmr) && is_image_folder(&nmr));
        let proc = folder("proc", &["procs", "1r", "1i"]);
        assert!(is_bruker_folder(&proc));
        let half = folder("half", &["acqus"]);
        assert!(!is_bruker_folder(&half));
        for d in [kicad, protel, drills, photos, nmr, proc, half] {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}
