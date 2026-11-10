//! Streamed folder listing: entries arrive in chunks (the first after a few dozen entries),
//! so the grid appears while a 100 000-file folder is still being read.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use specular_core::CancelToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EntryKind {
    Folder,
    Archive,
    Image,
    /// Not an image (listed only with "show other files").
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    /// File name (for recursive listings, the path below the listed folder with `/`).
    pub name: String,
    pub kind: EntryKind,
    pub len: u64,
    pub modified: Option<SystemTime>,
    pub hidden: bool,
    /// Capture time (seconds, from the probe) when known.
    pub taken: Option<i64>,
    /// Displayed pixel size when known.
    pub dims: Option<(u32, u32)>,
}

impl Entry {
    pub fn ext(&self) -> String {
        self.path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    }
    pub fn is_container(&self) -> bool {
        matches!(self.kind, EntryKind::Folder | EntryKind::Archive)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListOptions {
    pub show_hidden: bool,
    pub show_other: bool,
    /// List archives (zip, cbz, rar, 7z, tar…) too, at every depth. Off: a gallery shows
    /// images (and folders) only; archives still open as books from elsewhere.
    pub show_archives: bool,
    pub show_folders: bool,
    /// Subfolder levels to descend into (0: this folder only). Folder tiles are not listed
    /// in recursive views; their images are.
    pub depth: u32,
    /// Stop after this many entries (a guard against opening a whole disk).
    pub max_entries: usize,
}

impl Default for ListOptions {
    fn default() -> ListOptions {
        ListOptions {
            show_hidden: false,
            show_other: false,
            show_archives: false,
            show_folders: true,
            depth: 0,
            max_entries: 500_000,
        }
    }
}

/// First chunk size (fast first paint), then the steady chunk size.
const FIRST_CHUNK: usize = 48;
const CHUNK: usize = 2048;
/// Directory entries handed to one stat worker at a time.
const BATCH: usize = 1024;
/// Stat workers: `stat` dominates a big listing (about 4/5 of it on a warm cache), and it
/// scales with threads; more than a few only contend on the directory's inode lock.
const MAX_WORKERS: usize = 4;

/// Lists `dir`, calling `sink` with chunks of entries in directory order. `classify` decides
/// what a file is (image, archive, other). Returns the number of entries listed.
///
/// The calling thread reads the directory and stays a batch ahead of a few scoped workers that
/// `stat` and classify; results are re-sequenced, so the order and chunking are as if serial.
pub fn list_dir(
    dir: &Path,
    opts: ListOptions,
    classify: &(dyn Fn(&Path) -> EntryKind + Sync),
    cancel: &CancelToken,
    sink: &mut dyn FnMut(Vec<Entry>),
) -> std::io::Result<usize> {
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, MAX_WORKERS);
    let mut out = Out {
        chunk: Vec::with_capacity(FIRST_CHUNK),
        first: true,
        total: 0,
        max: opts.max_entries,
        sink,
    };
    let mut queue: Vec<(PathBuf, u32)> = vec![(dir.to_path_buf(), 0)];
    let mut root_err = None;
    std::thread::scope(|scope| {
        let mut pool: Option<Pool<'_>> = None;
        while let Some((d, depth)) = queue.pop() {
            let rd = match std::fs::read_dir(&d) {
                Ok(rd) => rd,
                Err(e) => {
                    if depth == 0 {
                        root_err = Some(e);
                    }
                    continue;
                }
            };
            let mut subdirs = Vec::new();
            let mut batch: Vec<std::fs::DirEntry> = Vec::new();
            let (mut sent, mut next) = (0u64, 0u64);
            let mut parked = std::collections::BTreeMap::new();
            for e in rd.flatten() {
                if cancel.is_cancelled() || out.total >= out.max {
                    break;
                }
                batch.push(e);
                if batch.len() < BATCH {
                    continue;
                }
                let p =
                    pool.get_or_insert_with(|| Pool::start(scope, workers, dir, opts, classify));
                let _ = p.work.send((sent, depth, std::mem::take(&mut batch)));
                sent += 1;
                // Hand on whatever is ready, in order, while the directory is still read.
                while let Ok(r) = p.done.try_recv() {
                    parked.insert(r.0, (r.1, r.2));
                }
                while let Some((entries, subs)) = parked.remove(&next) {
                    next += 1;
                    subdirs.extend(subs);
                    out.push(entries);
                }
            }
            if let Some(p) = &pool {
                while next < sent {
                    while let Some((entries, subs)) = parked.remove(&next) {
                        next += 1;
                        subdirs.extend(subs);
                        out.push(entries);
                    }
                    if next < sent
                        && let Ok(r) = p.done.recv()
                    {
                        parked.insert(r.0, (r.1, r.2));
                    }
                }
            }
            // The tail (or a small folder) is done here: no thread hand-off for a few files.
            if !cancel.is_cancelled() {
                let (entries, subs) = stat_batch(batch, depth, dir, opts, classify);
                subdirs.extend(subs);
                out.push(entries);
            }
            // Depth-first, in name order of the subfolders (reverse: popped from the end).
            subdirs.sort();
            for s in subdirs.into_iter().rev() {
                queue.push((s, depth + 1));
            }
        }
        // Dropping the pool closes the work channel; the scope joins the workers.
        drop(pool);
    });
    if !out.chunk.is_empty() {
        (out.sink)(std::mem::take(&mut out.chunk));
    }
    match root_err {
        Some(e) if out.total == 0 => Err(e),
        _ => Ok(out.total),
    }
}

/// Re-chunks the sequenced results for the sink (first chunk small, then steady).
struct Out<'s> {
    chunk: Vec<Entry>,
    first: bool,
    total: usize,
    max: usize,
    sink: &'s mut dyn FnMut(Vec<Entry>),
}

impl Out<'_> {
    fn push(&mut self, entries: Vec<Entry>) {
        for e in entries {
            if self.total >= self.max {
                return;
            }
            self.chunk.push(e);
            self.total += 1;
            if self.chunk.len() >= if self.first { FIRST_CHUNK } else { CHUNK } {
                self.first = false;
                (self.sink)(std::mem::replace(
                    &mut self.chunk,
                    Vec::with_capacity(CHUNK),
                ));
            }
        }
    }
}

type Batch = (u64, u32, Vec<std::fs::DirEntry>);
type Done = (u64, Vec<Entry>, Vec<PathBuf>);

/// Scoped stat workers, started on the first full batch.
struct Pool<'scope> {
    work: std::sync::mpsc::Sender<Batch>,
    done: std::sync::mpsc::Receiver<Done>,
    _scope: std::marker::PhantomData<&'scope ()>,
}

impl<'scope> Pool<'scope> {
    fn start<'env>(
        scope: &'scope std::thread::Scope<'scope, 'env>,
        workers: usize,
        root: &'env Path,
        opts: ListOptions,
        classify: &'env (dyn Fn(&Path) -> EntryKind + Sync),
    ) -> Pool<'scope> {
        let (work, rx) = std::sync::mpsc::channel::<Batch>();
        let (tx, done) = std::sync::mpsc::channel::<Done>();
        let rx = std::sync::Arc::new(parking_lot::Mutex::new(rx));
        for wi in 0..workers {
            let (rx, tx) = (rx.clone(), tx.clone());
            specular_thread::spawn_scoped(scope, format!("thumb-list-{wi}"), move || {
                loop {
                    let next = rx.lock().recv();
                    let Ok((seq, depth, batch)) = next else { break };
                    // A panic loses this batch's entries only: the lister waits for every
                    // batch it sent, so one must come back.
                    let (entries, subs) =
                        specular_core::isolate::run("folder listing batch", || {
                            stat_batch(batch, depth, root, opts, classify)
                        })
                        .unwrap_or_default();
                    if tx.send((seq, entries, subs)).is_err() {
                        break;
                    }
                }
            });
        }
        Pool {
            work,
            done,
            _scope: std::marker::PhantomData,
        }
    }
}

/// `stat`s and classifies directory entries: the entries to list, and the subfolders to descend
/// into.
fn stat_batch(
    batch: Vec<std::fs::DirEntry>,
    depth: u32,
    root: &Path,
    opts: ListOptions,
    classify: &dyn Fn(&Path) -> EntryKind,
) -> (Vec<Entry>, Vec<PathBuf>) {
    let mut out = Vec::with_capacity(batch.len());
    let mut subdirs = Vec::new();
    for e in batch {
        let path = e.path();
        let file_name = e.file_name().to_string_lossy().into_owned();
        let Ok(ft) = e.file_type() else { continue };
        crate::slowio::stat();
        // Follow symbolic links for the kind, but never descend through them (cycles).
        let meta = if ft.is_symlink() {
            std::fs::metadata(&path).ok()
        } else {
            e.metadata().ok()
        };
        let Some(meta) = meta else { continue };
        let hidden = is_hidden(&file_name, &meta);
        if hidden && !opts.show_hidden {
            continue;
        }
        // A folder that opens as one image (a CASA image, an OME-Zarr store, a Relight
        // image) is an image tile at any depth, never descended into.
        let image_folder = meta.is_dir()
            && specular_core::is_image_folder(&path)
            && classify(&path) == EntryKind::Image;
        let kind = if image_folder {
            EntryKind::Image
        } else if meta.is_dir() {
            if depth < opts.depth && !ft.is_symlink() {
                subdirs.push(path.clone());
            }
            if opts.depth > 0 || !opts.show_folders {
                continue;
            }
            EntryKind::Folder
        } else if meta.is_file() {
            match classify(&path) {
                EntryKind::Other if !opts.show_other => continue,
                EntryKind::Archive if !opts.show_archives => continue,
                k => k,
            }
        } else {
            continue;
        };
        let name = if depth == 0 {
            file_name
        } else {
            path.strip_prefix(root)
                .unwrap_or(&path)
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        };
        // `DirEntry::path` grows the folder's path to append the name, leaving about a third
        // of it unused: the entry keeps its path for as long as it is listed.
        let mut path = path;
        path.shrink_to_fit();
        out.push(Entry {
            path,
            name,
            kind,
            len: if meta.is_dir() { 0 } else { meta.len() },
            modified: meta.modified().ok(),
            hidden,
            taken: None,
            dims: None,
        });
    }
    (out, subdirs)
}

#[cfg(windows)]
fn is_hidden(name: &str, meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    name.starts_with('.') || meta.file_attributes() & 0x2 != 0
}

#[cfg(not(windows))]
fn is_hidden(name: &str, _meta: &std::fs::Metadata) -> bool {
    name.starts_with('.')
}

/// EXIF / PNG capture time of a file, reading only its headers and metadata (no decoder
/// needed): see [`specular_decode::read_exif_tiff`] for the formats.
pub fn probe_date_taken(path: &Path) -> Option<i64> {
    use std::io::{Read, Seek};
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let mut f = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    let mut sig = [0u8; 2];
    f.read_exact(&mut sig).ok()?;
    let s = specular_decode::read_exif_tiff(&mut f)
        .and_then(|t| specular_decode::exif_date_taken(&t))
        .or_else(|| {
            // A PNG's text "Creation Time".
            (sig == [0x89, b'P'] && f.rewind().is_ok())
                .then(|| specular_decode::png_date_taken(&mut f))
                .flatten()
        })?;
    crate::generate::parse_exif_date(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(p: &Path) -> EntryKind {
        match p.extension().and_then(|e| e.to_str()) {
            Some("jpg" | "png") => EntryKind::Image,
            Some("zip") => EntryKind::Archive,
            _ => EntryKind::Other,
        }
    }

    #[test]
    fn lists_in_chunks_with_options() {
        let dir = crate::cache::test_dir("list");
        for i in 0..120 {
            std::fs::write(dir.join(format!("img{i:03}.jpg")), b"x").unwrap();
        }
        std::fs::write(dir.join(".hidden.jpg"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        std::fs::write(dir.join("book.zip"), b"x").unwrap();
        std::fs::create_dir_all(dir.join("sub/deeper")).unwrap();
        std::fs::write(dir.join("sub/a.png"), b"xy").unwrap();
        std::fs::write(dir.join("sub/deeper/b.png"), b"x").unwrap();

        let mut chunks = vec![];
        let n = list_dir(
            &dir,
            ListOptions::default(),
            &classify,
            &CancelToken::new(),
            &mut |c| chunks.push(c),
        )
        .unwrap();
        assert_eq!(n, 121); // 120 images + folder: no archive by default
        assert_eq!(chunks[0].len(), FIRST_CHUNK);
        let all: Vec<Entry> = chunks.into_iter().flatten().collect();
        assert!(
            all.iter()
                .any(|e| e.kind == EntryKind::Folder && e.name == "sub")
        );
        assert!(!all.iter().any(|e| e.kind == EntryKind::Archive));
        assert!(!all.iter().any(|e| e.hidden || e.name == "notes.txt"));
        // Listed paths keep no unused room (they live as long as the listing).
        assert!(
            all.iter()
                .all(|e| e.path.capacity() == e.path.as_os_str().len()),
            "a path with spare capacity"
        );

        // Archives only when asked for, and then in recursive listings too.
        for depth in [0, 2] {
            let opts = ListOptions {
                show_archives: true,
                depth,
                ..ListOptions::default()
            };
            let mut all = vec![];
            list_dir(&dir, opts, &classify, &CancelToken::new(), &mut |c| {
                all.extend(c)
            })
            .unwrap();
            assert!(
                all.iter()
                    .any(|e| e.kind == EntryKind::Archive && e.name == "book.zip"),
                "depth {depth}"
            );
        }

        let opts = ListOptions {
            show_hidden: true,
            show_other: true,
            ..ListOptions::default()
        };
        let mut all = vec![];
        list_dir(&dir, opts, &classify, &CancelToken::new(), &mut |c| {
            all.extend(c)
        })
        .unwrap();
        assert_eq!(all.len(), 123); // + hidden + other, still no archive
        assert!(all.iter().any(|e| e.hidden && e.name == ".hidden.jpg"));

        // Recursive: no folder or archive tiles, nested names, depth limit.
        let opts = ListOptions {
            depth: 1,
            ..ListOptions::default()
        };
        let mut all = vec![];
        list_dir(&dir, opts, &classify, &CancelToken::new(), &mut |c| {
            all.extend(c)
        })
        .unwrap();
        assert_eq!(all.len(), 121);
        let sub = all.iter().find(|e| e.name == "sub/a.png").unwrap();
        assert_eq!(sub.len, 2);
        let opts = ListOptions { depth: 5, ..opts };
        let mut all = vec![];
        list_dir(&dir, opts, &classify, &CancelToken::new(), &mut |c| {
            all.extend(c)
        })
        .unwrap();
        assert!(all.iter().any(|e| e.name == "sub/deeper/b.png"));

        // Cancelled: stops early.
        let tok = CancelToken::new();
        tok.cancel();
        let n = list_dir(&dir, ListOptions::default(), &classify, &tok, &mut |_| {}).unwrap();
        assert_eq!(n, 0);
        assert!(
            list_dir(
                &dir.join("missing"),
                ListOptions::default(),
                &classify,
                &CancelToken::new(),
                &mut |_| {}
            )
            .is_err()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parallel_stat_keeps_directory_order_and_chunks() {
        let dir = crate::cache::test_dir("list-big");
        let n = BATCH * 3 + 77;
        for i in 0..n {
            let ext = if i % 10 == 3 { "txt" } else { "jpg" };
            std::fs::write(dir.join(format!("f{i}.{ext}")), b"x").unwrap();
        }
        let serial: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".jpg"))
            .collect();
        let mut chunks = vec![];
        let total = list_dir(
            &dir,
            ListOptions::default(),
            &classify,
            &CancelToken::new(),
            &mut |c| chunks.push(c),
        )
        .unwrap();
        assert_eq!(total, serial.len());
        assert_eq!(chunks[0].len(), FIRST_CHUNK);
        assert!(chunks[1..chunks.len() - 1].iter().all(|c| c.len() == CHUNK));
        let names: Vec<String> = chunks.into_iter().flatten().map(|e| e.name).collect();
        assert_eq!(names, serial);
        // The cap holds across batches.
        let opts = ListOptions {
            max_entries: 1500,
            ..ListOptions::default()
        };
        let mut all = vec![];
        let total = list_dir(&dir, opts, &classify, &CancelToken::new(), &mut |c| {
            all.extend(c)
        })
        .unwrap();
        assert_eq!((total, all.len()), (1500, 1500));
        std::fs::remove_dir_all(&dir).ok();
    }
}
