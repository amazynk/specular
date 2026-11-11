//! Thumbnails of archive pages through the thumbnail engine.
//!
//! A page's virtual path (`<archive>/<entry>`, see [`crate::book`]) is keyed by the archive's
//! size and time stamp and the entry's name ([`ThumbKey::of_member`]), so a cached thumbnail
//! is found without opening the archive and goes stale when the archive changes. The archives
//! themselves are opened once and kept for the next pages (the last [`KEEP`], one open at a
//! time per archive). Pages of a solid stream (a solid 7z block, a solid RAR archive) are
//! told to the engine as such ([`Opener::stream`]), which has one worker read them in stream
//! order ([`Book::read_pages`]): the stream is decoded once, not once per page; a solid 7z
//! block's LZMA2 stream on several threads where it was compressed in independent parts
//! ([`decode_threads`]). Anything else is a plain file ([`FileOpener`]).
//!
//! An archive's tile pages ([`ArchiveOpener::first_pages`], ComicInfo's front cover first and
//! its deleted pages left out, as far as ComicInfo is cheap to read: [`Book::for_thumbnails`])
//! are kept in the thumbnail cache too, keyed like the pages, so a folder of archives whose
//! thumbnails are cached is shown without opening any of them. An archive the viewer has open
//! is read through the viewer's book rather than opened again, unless a solid stream keeps
//! state between reads ([`Book::shares_reads`]).
//!
//! What an archive's ComicInfo says about its pages (reading order, spreads, deleted pages,
//! front cover) is kept in the cache too, keyed like the pages ([`known_comic`],
//! [`remember_comic`]): found cheaply by the thumbnails, or read in full by the reading mode,
//! it is then known to both on the next open of the archive as it is, and a changed archive
//! (another stamp) is read again. ComicInfo the thumbnails could not reach cheaply (late in a
//! solid 7z stream) is kept as soon as their own reads decode past it (the archive's pages
//! made into thumbnails): the tile pages are rewritten then ([`ArchiveOpener::with_cover_updates`]
//! hears of it), or once the reading mode has read it.
//!
//! An archive that fails to open is not opened again for [`RETRY_FAILED`] (for this session
//! only): not on the next visit to its folder, nor when the kept archives have moved on.
//! It is tried again sooner when it changes, or its folder does (a missing RAR volume put
//! next to it).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use specular_thumbs::engine::{FileOpener, Opened, Opener, StreamPos};
use specular_thumbs::{DiskCache, Stamp, ThumbKey};

use specular_archive::{ComicInfo, ComicLookup, Manga, PageInfo, PageType};

use crate::book::{Book, BookError, Books, KnownComic, is_archive_path};

/// Archives kept open (a handle, a listing and up to 16 MiB of a solid 7z block's entries
/// each).
pub const KEEP: usize = 4;

/// Tile pages kept in the cache per archive (the gallery's mosaic shows four).
pub const COVERS: usize = 4;

/// Cache blob tag of an archive's tile pages ("CVR1"; a new layout takes a new tag).
const COVERS_TAG: u32 = u32::from_le_bytes(*b"CVR1");

/// Cache blob tag of what an archive's ComicInfo says about its pages ("CIN1").
const COMIC_TAG: u32 = u32::from_le_bytes(*b"CIN1");

/// [`ComicInfo::page`] flags kept in the cache.
const PAGE_DELETED: u8 = 1;
const PAGE_FRONT_COVER: u8 = 2;
const PAGE_DOUBLE: u8 = 4;

/// What `comic` says about an archive's pages as a cache blob: `0` for no ComicInfo, else `1`,
/// the `Manga` field, the count of pages listed, and per page (the first entry of an image,
/// as [`ComicInfo::page`] finds it, when it is deleted, the front cover or a double page) its
/// image number (`u32`, little-endian) and flags. Title, writer and the like are not kept: the
/// viewer does not use them.
fn encode_comic(comic: Option<&ComicInfo>) -> Vec<u8> {
    let Some(c) = comic else {
        return vec![0];
    };
    let manga = match c.manga {
        Manga::Unknown => 0,
        Manga::No => 1,
        Manga::Yes => 2,
        Manga::YesAndRightToLeft => 3,
    };
    let mut seen = std::collections::HashSet::new();
    let pages: Vec<(u32, u8)> = (c.pages.iter())
        .filter(|p| seen.insert(p.image))
        .map(|p| {
            let mut f = 0;
            match p.kind {
                PageType::Deleted => f |= PAGE_DELETED,
                PageType::FrontCover => f |= PAGE_FRONT_COVER,
                _ => {}
            }
            if p.double_page {
                f |= PAGE_DOUBLE;
            }
            (p.image, f)
        })
        .filter(|&(_, f)| f != 0)
        .collect();
    let mut b = Vec::with_capacity(6 + pages.len() * 5);
    b.extend_from_slice(&[1, manga]);
    b.extend_from_slice(&(pages.len() as u32).to_le_bytes());
    for (image, f) in pages {
        b.extend_from_slice(&image.to_le_bytes());
        b.push(f);
    }
    b
}

/// What [`encode_comic`] wrote (`Some(None)`: no ComicInfo), or `None`.
fn decode_comic(b: &[u8]) -> KnownComic {
    let (&present, rest) = b.split_first()?;
    match present {
        0 if rest.is_empty() => return Some(None),
        1 => {}
        _ => return None,
    }
    let (&manga, rest) = rest.split_first()?;
    let manga = match manga {
        0 => Manga::Unknown,
        1 => Manga::No,
        2 => Manga::Yes,
        3 => Manga::YesAndRightToLeft,
        _ => return None,
    };
    let n = u32::from_le_bytes(rest.get(..4)?.try_into().ok()?) as usize;
    let rest = &rest[4..];
    if rest.len() != n.checked_mul(5)? {
        return None;
    }
    let pages = rest
        .chunks_exact(5)
        .map(|p| {
            let f = p[4];
            let kind = if f & PAGE_DELETED != 0 {
                PageType::Deleted
            } else if f & PAGE_FRONT_COVER != 0 {
                PageType::FrontCover
            } else {
                PageType::Story
            };
            PageInfo {
                image: u32::from_le_bytes([p[0], p[1], p[2], p[3]]),
                kind,
                double_page: f & PAGE_DOUBLE != 0,
                ..PageInfo::default()
            }
        })
        .collect();
    Some(Some(ComicInfo {
        manga,
        pages,
        ..ComicInfo::default()
    }))
}

/// What the cache knows of the ComicInfo of the archive at `archive` as of `stamp`
/// ([`remember_comic`]); `None` when it knows nothing (never kept, or kept for another
/// version of the archive).
pub fn known_comic(cache: &DiskCache, archive: &Path, stamp: Stamp) -> KnownComic {
    decode_comic(&cache.get_blob(&ArchiveOpener::covers_key(archive, stamp), COMIC_TAG)?)
}

/// Keeps `comic`, the ComicInfo of `book` (`None`: it has none), in the cache, with the
/// book's tile pages when it has one; nothing for a book not opened from a file as it is now.
pub fn remember_comic(cache: &DiskCache, book: &Book, comic: Option<&ComicInfo>) {
    // Nothing of an encrypted archive is written to the cache.
    let Some(stamp) = book.stamp().filter(|_| !book.encrypted()) else {
        return;
    };
    let archive = book.path();
    if Stamp::of_path(archive).ok() != Some(stamp) {
        return;
    }
    put_comic(cache, archive, stamp, comic);
    if comic.is_some()
        && let Some(blob) = encode_covers(archive, &book.covers(COVERS))
    {
        let key = ArchiveOpener::covers_key(archive, stamp);
        let _ = cache.put_blob(&key, COVERS_TAG, &blob);
    }
}

/// Keeps `comic` as the ComicInfo of the archive at `archive` as of `stamp`.
fn put_comic(cache: &DiskCache, archive: &Path, stamp: Stamp, comic: Option<&ComicInfo>) {
    let key = ArchiveOpener::covers_key(archive, stamp);
    let _ = cache.put_blob(&key, COMIC_TAG, &encode_comic(comic));
}

/// How long an archive that failed to open (no decode host for RAR, an I/O error, a damaged
/// file) is not tried again, unless it or its folder changes.
pub const RETRY_FAILED: Duration = Duration::from_secs(30);

/// Why an archive failed to open, and when.
struct Failed {
    stamp: Stamp,
    /// Its folder's stamp then (`None` if that could not be read).
    dir: Option<Stamp>,
    at: Instant,
    why: String,
}

/// Which entries are pages.
pub type IsImage = Arc<dyn Fn(&Path) -> bool + Send + Sync>;

/// Threads one solid 7z stream is decoded on for thumbnails: half the cores, at most 8 (the
/// engine's other workers keep making thumbnails meanwhile, and each thread holds a part of
/// the stream decoded).
pub fn decode_threads() -> u32 {
    let n = std::thread::available_parallelism().map_or(1, |n| n.get());
    (n / 2).clamp(1, 8) as u32
}

/// An archive as opened for thumbnails, or why it could not be.
struct Held {
    stamp: Stamp,
    book: Result<Arc<Book>, String>,
    /// Virtual path → page.
    pages: HashMap<PathBuf, usize>,
    /// The tile pages ComicInfo gave once a read decoded past it ([`ArchiveOpener::held`]),
    /// when the book was opened without it.
    tile: Mutex<Option<Vec<PathBuf>>>,
    /// Encrypted, with no password known this session ([`BookError::Locked`]).
    locked: bool,
}

/// Told the tile pages of an archive whose ComicInfo a thumbnail read came across.
pub type CoverUpdates = Arc<dyn Fn(&Path, Vec<PathBuf>) + Send + Sync>;

/// `covers` (pages of `archive`) as a cache blob: their count, then per page its path below
/// the archive as UTF-8 after its little-endian `u16` length. `None` if one is not UTF-8.
fn encode_covers(archive: &Path, covers: &[PathBuf]) -> Option<Vec<u8>> {
    let mut b = vec![u8::try_from(covers.len()).ok()?];
    for p in covers {
        let rel = p.strip_prefix(archive).ok()?.to_str()?;
        b.extend_from_slice(&u16::try_from(rel.len()).ok()?.to_le_bytes());
        b.extend_from_slice(rel.as_bytes());
    }
    Some(b)
}

/// What [`encode_covers`] wrote, or `None`.
fn decode_covers(archive: &Path, b: &[u8]) -> Option<Vec<PathBuf>> {
    let (&n, mut rest) = b.split_first()?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let len = u16::from_le_bytes(rest.get(..2)?.try_into().ok()?) as usize;
        let rel = Path::new(std::str::from_utf8(rest.get(2..2 + len)?).ok()?);
        // Only paths below the archive.
        let below = rel
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
        if rel.as_os_str().is_empty() || !below {
            return None;
        }
        out.push(archive.join(rel));
        rest = &rest[2 + len..];
    }
    rest.is_empty().then_some(out)
}

pub struct ArchiveOpener {
    is_image: IsImage,
    /// Where the tile pages are kept.
    cache: Option<Arc<DiskCache>>,
    /// The viewer's open archives, read through rather than opened again.
    books: Option<Books>,
    /// See [`decode_threads`].
    decode_threads: u32,
    /// See [`ArchiveOpener::with_cover_updates`].
    on_covers: Option<CoverUpdates>,
    /// Most recently used first.
    recent: Mutex<Vec<(PathBuf, Arc<Held>)>>,
    /// One open at a time per archive (the others wait for it).
    opening: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
    /// Archives that failed to open lately ([`RETRY_FAILED`]).
    failed: Mutex<HashMap<PathBuf, Failed>>,
    retry_failed: Duration,
    /// Archives opened (or failed), for tests and benchmarks.
    opens: std::sync::atomic::AtomicUsize,
    /// Archives read through the viewer's book instead, for tests.
    shared: std::sync::atomic::AtomicUsize,
    /// Keys the Library catalogued for files that cannot be read now (an offline location):
    /// their cached thumbnails still show.
    known: Mutex<HashMap<PathBuf, ThumbKey>>,
}

/// The archive and entry a virtual path names, by name only (no file system access): the
/// nearest ancestor with an archive's extension.
fn split(p: &Path) -> Option<(&Path, String)> {
    let a = p.ancestors().skip(1).find(|a| is_archive_path(a))?;
    let member = p.strip_prefix(a).ok()?.to_string_lossy().replace('\\', "/");
    Some((a, member))
}

/// The archive file a virtual path is a page of, and its stat; `None` for anything else
/// (a plain file, a folder named like an archive).
fn archive_of(p: &Path) -> Option<(&Path, String, std::fs::Metadata)> {
    let (a, member) = split(p)?;
    let meta = std::fs::metadata(a).ok()?;
    meta.is_file().then_some((a, member, meta))
}

impl ArchiveOpener {
    pub fn new(is_image: IsImage) -> ArchiveOpener {
        ArchiveOpener {
            is_image,
            cache: None,
            books: None,
            decode_threads: decode_threads(),
            on_covers: None,
            recent: Mutex::new(Vec::new()),
            opening: Mutex::new(HashMap::new()),
            failed: Mutex::new(HashMap::new()),
            retry_failed: RETRY_FAILED,
            opens: Default::default(),
            shared: Default::default(),
            known: Mutex::new(HashMap::new()),
        }
    }

    /// Keeps the archives' tile pages in `cache` (the thumbnails' cache).
    pub fn with_cache(mut self, cache: Option<Arc<DiskCache>>) -> ArchiveOpener {
        self.cache = cache;
        self
    }

    /// Reads the archives the viewer has open (`books`) through its books.
    pub fn with_books(mut self, books: Books) -> ArchiveOpener {
        self.books = Some(books);
        self
    }

    /// Tells `f` the new tile pages of an archive whose ComicInfo was not cheap to reach
    /// when it was opened, once reading its pages decoded past it (kept in the cache with
    /// what ComicInfo says, as the reading mode does; on the thread that decoded it).
    pub fn with_cover_updates(mut self, f: CoverUpdates) -> ArchiveOpener {
        self.on_covers = Some(f);
        self
    }

    /// Decodes a solid 7z stream on at most `n` threads (default [`decode_threads`]).
    #[cfg(test)]
    fn with_decode_threads(mut self, n: u32) -> ArchiveOpener {
        self.decode_threads = n.max(1);
        self
    }

    /// Tries archives that failed again after `d` instead of [`RETRY_FAILED`].
    #[cfg(test)]
    fn with_retry_failed(mut self, d: Duration) -> ArchiveOpener {
        self.retry_failed = d;
        self
    }

    /// Why `archive` (as of `stamp`, in a folder as of `dir`) failed to open lately, if it
    /// did and is not to be tried again yet.
    fn failed_lately(&self, archive: &Path, stamp: Stamp, dir: Option<Stamp>) -> Option<String> {
        let mut f = self.failed.lock();
        let e = f.get(archive)?;
        if e.stamp == stamp && e.dir == dir && e.at.elapsed() < self.retry_failed {
            return Some(e.why.clone());
        }
        f.remove(archive);
        None
    }

    /// Remembers that `archive` failed to open (or forgets it, `why` `None`).
    fn note_failure(&self, archive: &Path, stamp: Stamp, dir: Option<Stamp>, why: Option<&str>) {
        let mut f = self.failed.lock();
        let Some(why) = why else {
            f.remove(archive);
            return;
        };
        // Expired entries go when another is added: the map holds what failed lately.
        let retry = self.retry_failed;
        f.retain(|_, e| e.at.elapsed() < retry);
        f.insert(
            archive.to_path_buf(),
            Failed {
                stamp,
                dir,
                at: Instant::now(),
                why: why.to_owned(),
            },
        );
    }

    /// The pages of the archive at `archive`'s tile (its first `n`, ComicInfo's front cover
    /// first), as virtual paths: from the open archive, else from the cache (up to
    /// [`COVERS`]), else opening it.
    pub fn first_pages(&self, archive: &Path, n: usize) -> Vec<PathBuf> {
        let Ok(stamp) = Stamp::of_path(archive) else {
            return Vec::new();
        };
        let kept = self.kept(archive).filter(|h| h.stamp == stamp);
        if let Some(t) = kept.as_ref().and_then(|h| h.tile.lock().clone()) {
            return t.into_iter().take(n).collect();
        }
        if kept.is_none()
            && n <= COVERS
            && let Some(c) = &self.cache
            && let Some(b) = c.get_blob(&Self::covers_key(archive, stamp), COVERS_TAG)
            && let Some(mut covers) = decode_covers(archive, &b)
        {
            covers.truncate(n);
            return covers;
        }
        let h = match kept {
            Some(h) => h,
            None => match self.held(archive) {
                Ok(h) => h,
                Err(_) => return Vec::new(),
            },
        };
        match &h.book {
            Ok(b) => b.covers(n),
            Err(_) => Vec::new(),
        }
    }

    /// The cache key of the archive's tile pages: its pages' key with a name no entry has.
    fn covers_key(archive: &Path, stamp: Stamp) -> ThumbKey {
        ThumbKey::of_member(archive, stamp, "")
    }

    /// Keeps the tile pages of `archive` (`book`, as of `stamp`) in the cache.
    fn store_covers(&self, archive: &Path, stamp: Stamp, book: &Result<Arc<Book>, BookError>) {
        let Some(c) = &self.cache else { return };
        let covers = match book {
            // Page names of an encrypted archive stay in memory.
            Ok(b) if b.encrypted() => return,
            // Opened as it is now (not changed while it was).
            Ok(b) if b.stamp() == Some(stamp) => b.covers(COVERS),
            // No pages lasts as long as pages do; anything else (no decode host, an I/O
            // error) may pass, so it is not kept.
            Err(BookError::Empty) => Vec::new(),
            _ => return,
        };
        if let Some(blob) = encode_covers(archive, &covers) {
            let _ = c.put_blob(&Self::covers_key(archive, stamp), COVERS_TAG, &blob);
        }
    }

    /// How many times an archive was opened.
    #[cfg(test)]
    fn opens(&self) -> usize {
        self.opens.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// How many times the viewer's book of an archive was used instead.
    #[cfg(test)]
    fn shared(&self) -> usize {
        self.shared.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The viewer's book of the archive, if it is of the archive as it is now and may be read
    /// here too.
    fn viewers(&self, archive: &Path, stamp: Stamp) -> Option<Arc<Book>> {
        let b = self.books.as_ref()?.book(archive)?;
        (b.stamp() == Some(stamp) && b.shares_reads()).then_some(b)
    }

    /// Lets go of the kept archive at `archive` (a reader has learnt more about its pages than
    /// the thumbnails could: its tile pages are asked for again, from the cache).
    pub fn forget(&self, archive: &Path) {
        self.recent.lock().retain(|(p, _)| p != archive);
    }

    /// Thumbnail keys of files the Library holds but cannot read now (offline locations),
    /// used when a file cannot be read for its own key. Replaces the ones known before.
    pub fn know_keys(&self, keys: Vec<(PathBuf, ThumbKey)>) {
        *self.known.lock() = keys.into_iter().collect();
    }

    /// Whether the archive at `archive` was found encrypted, with no password known for it
    /// (its tile shows a lock).
    pub fn locked(&self, archive: &Path) -> bool {
        self.kept(archive).is_some_and(|h| h.locked)
    }

    /// The kept archive at `archive`, without looking at the file.
    fn kept(&self, archive: &Path) -> Option<Arc<Held>> {
        let mut r = self.recent.lock();
        let i = r.iter().position(|(p, _)| p == archive)?;
        let e = r.remove(i);
        let h = e.1.clone();
        r.insert(0, e);
        Some(h)
    }

    /// The archive at `archive` as it is now: the kept one if its stamp still matches, else
    /// opened again.
    fn held(&self, archive: &Path) -> io::Result<Arc<Held>> {
        let stamp = Stamp::of_path(archive)?;
        if let Some(h) = self.kept(archive).filter(|h| h.stamp == stamp) {
            return Ok(h);
        }
        let gate = self
            .opening
            .lock()
            .entry(archive.to_path_buf())
            .or_default()
            .clone();
        let _one = gate.lock();
        if let Some(h) = self.kept(archive).filter(|h| h.stamp == stamp) {
            return Ok(h);
        }
        let dir = archive.parent().and_then(|d| Stamp::of_path(d).ok());
        let book = match self.viewers(archive, stamp) {
            // The viewer opened it: whatever failed before has passed.
            Some(b) => {
                self.shared
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Ok(b)
            }
            None if let Some(why) = self.failed_lately(archive, stamp, dir) => {
                self.opening.lock().remove(archive);
                return Ok(Arc::new(Held {
                    stamp,
                    book: Err(why),
                    pages: HashMap::new(),
                    tile: Mutex::new(None),
                    locked: false,
                }));
            }
            None => {
                self.opens
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let mut cached = false;
                let known = |stamp| {
                    let k = known_comic(self.cache.as_deref()?, archive, stamp);
                    cached = k.is_some();
                    k
                };
                let book = Book::for_thumbnails(
                    archive,
                    |p| (self.is_image)(p),
                    self.decode_threads,
                    known,
                );
                // ComicInfo found (or found absent) cheaply: the reading mode knows it too.
                // (Its tile pages are kept below.)
                if let (Ok(b), Some(c), false) = (&book, &self.cache, cached)
                    && let Some(comic) = b.comic_known()
                    && b.stamp() == Some(stamp)
                    && !b.encrypted()
                {
                    put_comic(c, archive, stamp, comic);
                }
                book.map(Arc::new)
            }
        };
        self.store_covers(archive, stamp, &book);
        // Locked until the viewer unlocks it ([`ArchiveOpener::forget`] then): kept as such
        // while it is, not noted as a failure.
        let locked = matches!(book, Err(BookError::Locked | BookError::WrongPassword));
        let book = book.map_err(|e| e.message());
        let failed = book.as_ref().err().filter(|_| !locked);
        self.note_failure(archive, stamp, dir, failed.map(|s| s.as_str()));
        let pages = book.as_ref().map_or_else(
            |_| HashMap::new(),
            |b| {
                (b.paths().iter().cloned())
                    .zip(0..)
                    .collect::<HashMap<PathBuf, usize>>()
            },
        );
        let h = Arc::new(Held {
            stamp,
            book,
            pages,
            tile: Mutex::new(None),
            locked,
        });
        if let (Ok(b), Some(cache)) = (&h.book, &self.cache)
            && b.comic_known().is_none()
            && b.stamp() == Some(stamp)
        {
            self.comic_in_passing(archive, &h, b, cache.clone());
        }
        {
            let mut r = self.recent.lock();
            r.retain(|(p, _)| p != archive);
            r.insert(0, (archive.to_path_buf(), h.clone()));
            r.truncate(KEEP);
        }
        self.opening.lock().remove(archive);
        Ok(h)
    }

    /// Keeps the ComicInfo of `book` (`h`'s, opened without it) once reading its pages
    /// decodes past it: in the cache with its tile pages, which `h` shows from then on and
    /// [`ArchiveOpener::with_cover_updates`] is told.
    fn comic_in_passing(&self, archive: &Path, h: &Arc<Held>, book: &Book, cache: Arc<DiskCache>) {
        let held = Arc::downgrade(h);
        let (archive, on_covers) = (archive.to_path_buf(), self.on_covers.clone());
        book.on_comic_info(move |found| {
            // The book holds this: it is gone when the book is.
            let Some(h) = held.upgrade() else { return };
            let Ok(book) = &h.book else { return };
            match found {
                ComicLookup::Found(c) => {
                    let Some(b) = book.with_comic(c) else { return };
                    remember_comic(&cache, &b, b.comic_known().flatten());
                    let covers = b.covers(COVERS);
                    *h.tile.lock() = Some(covers.clone());
                    if let Some(f) = on_covers {
                        f(&archive, covers);
                    }
                }
                ComicLookup::Absent => {
                    if let Some(stamp) = book.stamp()
                        && !book.encrypted()
                        && Stamp::of_path(&archive).ok() == Some(stamp)
                    {
                        put_comic(&cache, &archive, stamp, None);
                    }
                }
                ComicLookup::Unread => {}
            }
        });
    }

    /// The book and page of virtual path `p` in `h`.
    fn page(h: &Held, p: &Path) -> io::Result<(Arc<Book>, usize)> {
        let book = h.book.clone().map_err(io::Error::other)?;
        let page = *h
            .pages
            .get(p)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such page"))?;
        Ok((book, page))
    }

    fn position(archive: &Path, h: &Held, p: &Path) -> Option<StreamPos> {
        let (book, page) = Self::page(h, p).ok()?;
        let (block, at) = book.stream_position(page)?;
        let mut s = std::hash::DefaultHasher::new();
        (archive, block).hash(&mut s);
        Some(StreamPos {
            stream: s.finish(),
            at,
        })
    }
}

impl Opener for ArchiveOpener {
    fn key(&self, path: &Path) -> io::Result<ThumbKey> {
        match archive_of(path) {
            Some((a, member, meta)) => Ok(ThumbKey::of_member(a, Stamp::of(&meta), &member)),
            None => FileOpener
                .key(path)
                .or_else(|e| self.known.lock().get(path).copied().ok_or(e)),
        }
    }

    fn open(&self, path: &Path) -> io::Result<Opened> {
        let Some((a, _, _)) = archive_of(path) else {
            return FileOpener.open(path);
        };
        let (book, page) = Self::page(&*self.held(a)?, path)?;
        let source = book.source(page).map_err(io::Error::other)?;
        Ok(Opened {
            source: Arc::new(source),
            private: book.encrypted(),
        })
    }

    fn stream(&self, path: &Path, open: bool) -> Option<StreamPos> {
        if !open {
            let (a, _) = split(path)?;
            let h = self.kept(a)?;
            return Self::position(a, &h, path);
        }
        let (a, _, _) = archive_of(path)?;
        let h = self.held(a).ok()?;
        Self::position(a, &h, path)
    }

    fn open_stream(
        &self,
        paths: &[&Path],
        each: &mut dyn FnMut(usize, io::Result<Opened>) -> bool,
    ) {
        // One stream is one archive: its pages in one pass, anything else on its own.
        let held = paths
            .first()
            .and_then(|p| archive_of(p))
            .and_then(|(a, _, _)| self.held(a).ok());
        let Some(Ok(book)) = held.as_ref().map(|h| h.book.clone()) else {
            for (i, p) in paths.iter().enumerate() {
                if !each(i, self.open(p)) {
                    return;
                }
            }
            return;
        };
        let h = held.as_deref().expect("held");
        let mut pages = Vec::with_capacity(paths.len());
        let mut at = Vec::with_capacity(paths.len());
        for (i, p) in paths.iter().enumerate() {
            match h.pages.get(*p) {
                Some(&page) => {
                    pages.push(page);
                    at.push(i);
                }
                None => {
                    if !each(i, self.open(p)) {
                        return;
                    }
                }
            }
        }
        book.read_pages(&pages, &mut |k, r| {
            each(
                at[k],
                r.map(|s| Opened {
                    source: Arc::new(s),
                    private: book.encrypted(),
                })
                .map_err(io::Error::other),
            )
        });
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;

    use specular_thumbs::DiskCache;
    use specular_thumbs::engine::{Engine, EngineConfig, Outcome, Request};
    use std::time::{Duration, Instant};

    fn scratch(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("specular-archthumbs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn jpeg(shade: u8) -> Vec<u8> {
        let (w, h) = (64, 48);
        let mut px = Vec::with_capacity(w * h * 4);
        for i in 0..w * h {
            px.extend_from_slice(&[shade, (i % 251) as u8, 255 - shade, 255]);
        }
        specular_thumbs::codec::encode_jpeg(&px, w, h, 85)
    }

    /// A CBZ of `n` JPEG pages (`01.jpg`…), `shade` varying their content.
    fn cbz(path: &Path, n: usize, shade: u8) {
        let pages: Vec<(String, Vec<u8>)> = (0..n)
            .map(|i| {
                (
                    format!("{:02}.jpg", i + 1),
                    jpeg(shade.wrapping_add(i as u8 * 9)),
                )
            })
            .collect();
        let files: Vec<(&str, &[u8])> = pages
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_slice()))
            .collect();
        std::fs::write(path, crate::book::tests::zip(&files)).unwrap();
    }

    fn opener() -> Arc<ArchiveOpener> {
        Arc::new(ArchiveOpener::new(Arc::new(|p: &Path| {
            p.extension().is_some_and(|e| e == "jpg")
        })))
    }

    fn engine(opener: Arc<ArchiveOpener>, cache: &Path) -> Engine {
        engine_with(opener, cache, 2)
    }

    fn engine_with(opener: Arc<dyn Opener>, cache: &Path, workers: usize) -> Engine {
        let cfg = EngineConfig {
            workers,
            ..EngineConfig::default()
        };
        Engine::new(
            cfg,
            Arc::new(specular_decode::Registry::new()),
            Arc::new(specular_core::JobPool::new(workers, "thumb-arc")),
            opener,
            Some(Arc::new(DiskCache::new(cache, 64 << 20))),
            Arc::new(|| {}),
        )
    }

    /// Asks for `paths` at class 128 and waits for all of them: (from the cache, made).
    fn run(e: &Engine, paths: &[PathBuf]) -> (usize, usize) {
        run_at(e, paths, 128)
    }

    /// [`run`] at `class`.
    fn run_at(e: &Engine, paths: &[PathBuf], class: u32) -> (usize, usize) {
        let reqs: Vec<Request> = paths
            .iter()
            .map(|p| Request {
                path: p.clone(),
                class,
            })
            .collect();
        e.want(reqs, paths.len());
        let (mut cached, mut made) = (0, 0);
        let mut done = std::collections::HashSet::new();
        let t = Instant::now();
        while done.len() < paths.len() {
            assert!(t.elapsed() < Duration::from_secs(300), "timed out");
            for o in e.take_ready() {
                match o {
                    Outcome::Ready(r) if r.sharp => {
                        if done.insert(r.path) {
                            if r.from_cache {
                                cached += 1;
                            } else {
                                made += 1;
                            }
                        }
                    }
                    Outcome::Ready(_) => {}
                    Outcome::Failed { path, error, .. } => panic!("{}: {error}", path.display()),
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        (cached, made)
    }

    #[test]
    fn member_keys_follow_the_archive_and_the_entry() {
        let d = scratch("keys");
        let a = d.join("b.cbz");
        cbz(&a, 3, 0);
        let o = opener();
        let k1 = o.key(&a.join("01.jpg")).unwrap();
        let stamp = Stamp::of_path(&a).unwrap();
        assert_eq!(k1, ThumbKey::of_member(&a, stamp, "01.jpg"));
        // Another page, another key; the key alone never opens the archive.
        assert_ne!(k1, o.key(&a.join("02.jpg")).unwrap());
        assert_eq!(o.opens(), 0);
        // A folder named like an archive holds plain files.
        let f = d.join("dir.zip");
        std::fs::create_dir_all(&f).unwrap();
        std::fs::write(f.join("x.jpg"), jpeg(1)).unwrap();
        assert_eq!(
            o.key(&f.join("x.jpg")).unwrap(),
            ThumbKey::of_file(&f.join("x.jpg")).unwrap()
        );
        assert_eq!(o.open(&f.join("x.jpg")).unwrap().source.bytes(), jpeg(1));
        // The archive rewritten: new keys, and its pages read again.
        let before = o.open(&a.join("02.jpg")).unwrap().source.bytes().to_vec();
        cbz(&a, 4, 100);
        let k2 = o.key(&a.join("01.jpg")).unwrap();
        assert_ne!(k1, k2);
        let after = o.open(&a.join("02.jpg")).unwrap().source.bytes().to_vec();
        assert_ne!(before, after);
        assert_eq!(o.opens(), 2);
        assert!(o.open(&a.join("09.jpg")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn archive_pages_go_through_the_engine_and_its_cache() {
        let d = scratch("engine");
        let a = d.join("b.cbz");
        cbz(&a, 12, 0);
        let cache = d.join("cache");
        let pages: Vec<PathBuf> = (1..=12).map(|i| a.join(format!("{i:02}.jpg"))).collect();
        // Cold: made, the archive opened once for all its pages.
        let o = opener();
        assert_eq!(run(&engine(o.clone(), &cache), &pages), (0, 12));
        assert_eq!(o.opens(), 1);
        assert_eq!(o.first_pages(&a, 4), pages[..4].to_vec());
        // Warm: all from the cache, without opening the archive.
        let o = opener();
        assert_eq!(run(&engine(o.clone(), &cache), &pages), (12, 0));
        assert_eq!(o.opens(), 0);
        // Changed: made again.
        cbz(&a, 12, 77);
        let o = opener();
        assert_eq!(run(&engine(o.clone(), &cache), &pages), (0, 12));
        assert_eq!(o.opens(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    fn cached_opener(cache: &Path) -> ArchiveOpener {
        let o = Arc::into_inner(opener()).unwrap();
        o.with_cache(Some(Arc::new(DiskCache::new(cache, 64 << 20))))
    }

    #[test]
    fn tile_pages_are_cached_and_a_warm_visit_opens_nothing() {
        let d = scratch("covers");
        let a = d.join("b.cbz");
        cbz(&a, 6, 0);
        let empty = d.join("e.cbz");
        std::fs::write(&empty, crate::book::tests::zip(&[("a.txt", b"x")])).unwrap();
        let cache = d.join("cache");
        let pages: Vec<PathBuf> = (1..=4).map(|i| a.join(format!("{i:02}.jpg"))).collect();
        // Cold: opened once for the tile pages and their thumbnails.
        let o = Arc::new(cached_opener(&cache));
        assert_eq!(o.first_pages(&a, 4), pages);
        assert!(o.first_pages(&empty, 4).is_empty());
        assert_eq!(run(&engine(o.clone(), &cache), &pages), (0, 4));
        assert_eq!(o.opens(), 2);
        // Warm (the next visit): tile pages and thumbnails from the cache, nothing opened.
        let o = Arc::new(cached_opener(&cache));
        assert_eq!(o.first_pages(&a, 4), pages);
        assert_eq!(o.first_pages(&a, 2), pages[..2].to_vec());
        assert!(o.first_pages(&empty, 4).is_empty());
        assert_eq!(run(&engine(o.clone(), &cache), &pages), (4, 0));
        assert_eq!(o.opens(), 0);
        // More pages than are kept: opened.
        assert_eq!(o.first_pages(&a, 5).len(), 5);
        assert_eq!(o.opens(), 1);
        // Changed: opened again, the new pages listed.
        cbz(&a, 3, 50);
        let o = Arc::new(cached_opener(&cache));
        assert_eq!(o.first_pages(&a, 4), pages[..3].to_vec());
        assert_eq!(o.opens(), 1);
        let o = Arc::new(cached_opener(&cache));
        assert_eq!(o.first_pages(&a, 4), pages[..3].to_vec());
        assert_eq!(o.opens(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn tile_pages_put_the_front_cover_first_and_leave_deleted_pages_out() {
        let d = scratch("comicinfo");
        let a = d.join("b.cbz");
        // Images 0…5 are 01.jpg…06.jpg: 01 deleted, 04 the front cover.
        let xml = br#"<?xml version="1.0"?><ComicInfo><Pages><Page Image="0" Type="Deleted"/>
            <Page Image="3" Type="FrontCover"/></Pages></ComicInfo>"#;
        let jpgs: Vec<Vec<u8>> = (0..6).map(|i| jpeg(i * 30)).collect();
        let names: Vec<String> = (1..=6).map(|i| format!("{i:02}.jpg")).collect();
        let mut files: Vec<(&str, &[u8])> = vec![("ComicInfo.xml", xml)];
        files.extend(
            names
                .iter()
                .map(|n| n.as_str())
                .zip(jpgs.iter().map(|j| j.as_slice())),
        );
        std::fs::write(&a, crate::book::tests::zip(&files)).unwrap();
        let o = cached_opener(&d.join("cache"));
        let want: Vec<PathBuf> = ["04", "02", "03", "05"]
            .iter()
            .map(|n| a.join(format!("{n}.jpg")))
            .collect();
        assert_eq!(o.first_pages(&a, 4), want);
        assert_eq!(o.first_pages(&a, 1), want[..1].to_vec());
        // The same from the cache.
        let o = cached_opener(&d.join("cache"));
        assert_eq!(o.first_pages(&a, 4), want);
        assert_eq!(o.opens(), 0);
        // The reader keeps ComicInfo's order (the cover where it is), deleted page left out.
        let b = Book::open(&a, |p| p.extension().is_some_and(|e| e == "jpg")).unwrap();
        assert_eq!(b.len(), 5);
        assert_eq!(b.entry_name(0), Some("02.jpg"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_archive_the_viewer_has_open_is_read_through_its_book() {
        let d = scratch("shared");
        let a = d.join("b.cbz");
        cbz(&a, 5, 0);
        let books = Books::default();
        let jpg = |p: &Path| p.extension().is_some_and(|e| e == "jpg");
        books.insert(Arc::new(Book::open(&a, jpg).unwrap()));
        let o = Arc::new(Arc::into_inner(opener()).unwrap().with_books(books.clone()));
        let pages: Vec<PathBuf> = (1..=5).map(|i| a.join(format!("{i:02}.jpg"))).collect();
        assert_eq!(o.first_pages(&a, 4), pages[..4].to_vec());
        assert_eq!(run(&engine(o.clone(), &d.join("cache")), &pages), (0, 5));
        assert_eq!((o.opens(), o.shared()), (0, 1));
        // The viewer's book of an older version is not used.
        cbz(&a, 5, 60);
        let o = Arc::new(Arc::into_inner(opener()).unwrap().with_books(books));
        assert_eq!(o.first_pages(&a, 4), pages[..4].to_vec());
        assert_eq!((o.opens(), o.shared()), (1, 0));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_cache_warmer_makes_archive_tiles_too() {
        let d = scratch("warm");
        let books = d.join("books");
        std::fs::create_dir_all(&books).unwrap();
        let (a, b) = (books.join("a.cbz"), books.join("b.cbz"));
        cbz(&a, 6, 0);
        cbz(&b, 2, 90);
        std::fs::write(books.join("c.jpg"), jpeg(7)).unwrap();
        let cache = d.join("cache");
        let mut s = specular_settings::Settings::default();
        s.gallery.cache_location = Some(cache.clone());
        s.gallery.show_archives = true;
        assert_eq!(crate::app::warm_cache(&books, 0, &s), 0);
        // The gallery's next visit: tile pages and their thumbnails (a quarter of the tile
        // with four pages, the whole tile with fewer) from the cache, nothing opened.
        let side = s.gallery.thumbnail_size as f32;
        let o = Arc::new(cached_opener(&cache));
        let (pa, pb) = (o.first_pages(&a, COVERS), o.first_pages(&b, COVERS));
        assert_eq!((pa.len(), pb.len()), (4, 2));
        let e = engine(o.clone(), &cache);
        let at = |side: f32| specular_thumbs::class_for(side);
        assert_eq!(run_at(&e, &pa, at(side * 0.5)), (4, 0));
        assert_eq!(run_at(&e, &pa, at(side)), (4, 0));
        assert_eq!(run_at(&e, &pb, at(side)), (2, 0));
        assert_eq!(run_at(&e, &pb, at(side * 2.0)), (2, 0));
        assert_eq!(o.opens(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    #[cfg_attr(
        windows,
        ignore = "the folder's change after new.txt is not seen on GitHub's Windows runner: to look into"
    )]
    fn an_archive_that_failed_is_not_reopened_for_a_while() {
        let d = scratch("failed");
        let bad = d.join("bad.cbz");
        std::fs::write(&bad, b"not an archive, and never a book").unwrap();
        let others: Vec<PathBuf> = (0..KEEP + 1).map(|i| d.join(format!("{i}.cbz"))).collect();
        for (i, a) in others.iter().enumerate() {
            cbz(a, 2, i as u8 * 20);
        }
        let o = opener();
        assert!(o.first_pages(&bad, 4).is_empty());
        assert_eq!(o.opens(), 1);
        // The kept archives move on; the next visit does not open it again.
        for a in &others {
            assert_eq!(o.first_pages(a, 4).len(), 2);
        }
        assert!(o.kept(&bad).is_none());
        let opens = o.opens();
        assert!(o.first_pages(&bad, 4).is_empty());
        assert!(o.open(&bad.join("01.jpg")).is_err());
        assert_eq!(o.opens(), opens);
        // Its folder changed: tried again (and failed again).
        std::fs::write(d.join("new.txt"), b"x").unwrap();
        for a in &others {
            o.first_pages(a, 4);
        }
        let opens = o.opens();
        assert!(o.first_pages(&bad, 4).is_empty());
        assert_eq!(o.opens(), opens + 1);
        // Fixed: it changed, so it is opened and its pages listed.
        cbz(&bad, 3, 7);
        assert_eq!(o.first_pages(&bad, 4).len(), 3);
        assert_eq!(o.opens(), opens + 2);
        assert!(o.failed.lock().is_empty());
        // After the retry time: tried again.
        let o = Arc::into_inner(opener())
            .unwrap()
            .with_retry_failed(Duration::ZERO);
        std::fs::write(&bad, b"broken again").unwrap();
        assert!(o.first_pages(&bad, 4).is_empty());
        for a in &others {
            o.first_pages(a, 4);
        }
        let opens = o.opens();
        assert!(o.first_pages(&bad, 4).is_empty());
        assert_eq!(o.opens(), opens + 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A CBZ of `n` JPEG pages with `xml` as its `ComicInfo.xml` (after the pages).
    fn cbz_with(path: &Path, n: usize, xml: &[u8]) {
        let pages: Vec<(String, Vec<u8>)> = (0..n)
            .map(|i| (format!("{:02}.jpg", i + 1), jpeg(i as u8 * 30)))
            .collect();
        let mut files: Vec<(&str, &[u8])> = pages
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_slice()))
            .collect();
        files.push(("ComicInfo.xml", xml));
        std::fs::write(path, crate::book::tests::zip(&files)).unwrap();
    }

    fn jpg(p: &Path) -> bool {
        p.extension().is_some_and(|e| e == "jpg")
    }

    #[test]
    fn comic_blobs_keep_what_the_viewer_uses() {
        let xml = br#"<ComicInfo><Title>T</Title><Manga>YesAndRightToLeft</Manga><Pages>
            <Page Image="0" Type="Story"/><Page Image="0" Type="Deleted"/>
            <Page Image="2" Type="FrontCover" DoublePage="true"/><Page Image="3" Type="Deleted"/>
            <Page Image="4" DoublePage="True"/><Page Image="5" Type="Letters"/></Pages></ComicInfo>"#;
        let c = ComicInfo::parse(xml).unwrap();
        let back = decode_comic(&encode_comic(Some(&c))).unwrap().unwrap();
        assert_eq!(back.manga, c.manga);
        for n in 0..8 {
            let kind = |c: &ComicInfo| c.page(n).map(|p| p.kind.clone());
            let double = |c: &ComicInfo| c.page(n).is_some_and(|p| p.double_page);
            // Image 0: its first entry (a story page) is the one that counts.
            let deleted = |c: &ComicInfo| kind(c) == Some(PageType::Deleted);
            let cover = |c: &ComicInfo| kind(c) == Some(PageType::FrontCover);
            assert_eq!(deleted(&back), deleted(&c), "{n}");
            assert_eq!(cover(&back), cover(&c), "{n}");
            assert_eq!(double(&back), double(&c), "{n}");
        }
        assert_eq!(back.spreads().collect::<Vec<_>>(), [2, 4]);
        assert_eq!(decode_comic(&encode_comic(None)), Some(None));
        // Anything else reads as unknown.
        let b = encode_comic(Some(&c));
        assert_eq!(decode_comic(&b[..b.len() - 1]), None);
        assert_eq!(decode_comic(&[]), None);
        assert_eq!(decode_comic(&[0, 0]), None);
        assert_eq!(decode_comic(&[1, 9, 0, 0, 0, 0]), None);
        assert_eq!(decode_comic(&[1, 3, 255, 255, 255, 255]), None);
    }

    #[test]
    fn comicinfo_is_cached_per_archive_stamp() {
        let d = scratch("comicstamp");
        let a = d.join("b.cbz");
        let xml = br#"<ComicInfo><Manga>YesAndRightToLeft</Manga><Pages>
            <Page Image="1" Type="Deleted"/><Page Image="3" Type="FrontCover"/></Pages></ComicInfo>"#;
        cbz_with(&a, 5, xml);
        let cache = d.join("cache");
        let disk = DiskCache::new(&cache, 64 << 20);
        let stamp = Stamp::of_path(&a).unwrap();
        // Miss: nothing known, the reading mode opens the book without it.
        assert_eq!(known_comic(&disk, &a, stamp), None);
        let known = |s| known_comic(&disk, &a, s);
        let b = Book::open_for_reading(&a, jpg, 1, known).unwrap();
        assert!(b.comic_known().is_none());
        assert_eq!(b.len(), 5);
        // The thumbnails read it (cheap here) and keep it.
        let o = cached_opener(&cache);
        let tile = o.first_pages(&a, 4);
        assert_eq!(tile[0], a.join("04.jpg"));
        assert!(!tile.contains(&a.join("02.jpg")));
        let c = known_comic(&disk, &a, stamp).unwrap().unwrap();
        assert!(c.right_to_left());
        // Hit: the reading mode opens the book with it, deleted page left out, right to left.
        let known = |s| known_comic(&disk, &a, s);
        let b = Book::open_for_reading(&a, jpg, 1, known).unwrap();
        assert!(b.comic_known().is_some_and(|c| c.is_some()));
        assert!(b.right_to_left());
        assert_eq!(b.len(), 4);
        assert_eq!(b.entry_name(1), Some("03.jpg"));
        // Changed (another stamp): the old ComicInfo is not used.
        cbz_with(&a, 6, b"<ComicInfo/>");
        let stamp2 = Stamp::of_path(&a).unwrap();
        assert_ne!(stamp, stamp2);
        assert_eq!(known_comic(&disk, &a, stamp2), None);
        let known = |s| known_comic(&disk, &a, s);
        let b = Book::open_for_reading(&a, jpg, 1, known).unwrap();
        assert!(b.comic_known().is_none());
        assert!(!b.right_to_left());
        assert_eq!(b.len(), 6);
        // Read in full (as the reading mode does) and kept: none to apply now.
        let read = b.read_comic_info().found().unwrap();
        let b = b.with_comic(read).unwrap();
        remember_comic(&disk, &b, b.comic_known().flatten());
        let c = known_comic(&disk, &a, stamp2).unwrap().unwrap();
        assert!(!c.right_to_left() && c.pages.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn comicinfo_the_thumbnails_cannot_reach_comes_from_the_cache() {
        let d = scratch("comiclate");
        let a = d.join("b.cbz");
        cbz(&a, 5, 0);
        let cache = d.join("cache");
        let disk = DiskCache::new(&cache, 64 << 20);
        let stamp = Stamp::of_path(&a).unwrap();
        // An archive with no ComicInfo is known to have none, after the thumbnails' open.
        let o = cached_opener(&cache);
        assert_eq!(o.first_pages(&a, 4).len(), 4);
        assert_eq!(known_comic(&disk, &a, stamp), Some(None));
        let b = Book::open_for_reading(&a, jpg, 1, |s| known_comic(&disk, &a, s)).unwrap();
        assert!(b.comic_known().is_some_and(|c| c.is_none()));
        // What the reading mode read (here made up: the thumbnails would not have reached it in
        // a solid stream) is kept with the tile pages, and a thumbnail open applies it too.
        let c = ComicInfo::parse(
            br#"<ComicInfo><Pages><Page Image="0" Type="Deleted"/><Page Image="2" Type="FrontCover"/></Pages></ComicInfo>"#,
        )
        .unwrap();
        let read = b.with_comic(c).unwrap();
        remember_comic(&disk, &read, read.comic_known().flatten());
        let want: Vec<PathBuf> = ["03", "02", "04", "05"]
            .iter()
            .map(|n| a.join(format!("{n}.jpg")))
            .collect();
        // The opener that kept the archive lets go of it and lists the tile from the cache.
        o.forget(&a);
        assert_eq!(o.first_pages(&a, 4), want);
        let o = cached_opener(&cache);
        assert_eq!(o.first_pages(&a, 4), want);
        assert_eq!(o.opens(), 0);
        let t = Book::for_thumbnails(&a, jpg, 1, |s| known_comic(&disk, &a, s)).unwrap();
        assert_eq!(t.covers(4), want);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn comicinfo_that_deletes_every_page_is_cached_and_every_page_shown() {
        let d = scratch("comicalldeleted");
        let a = d.join("b.cbz");
        let xml = br#"<ComicInfo><Manga>YesAndRightToLeft</Manga><Pages>
            <Page Image="0" Type="Deleted"/><Page Image="1" Type="Deleted"/>
            <Page Image="2" Type="Deleted"/></Pages></ComicInfo>"#;
        cbz_with(&a, 3, xml);
        let cache = d.join("cache");
        let disk = DiskCache::new(&cache, 64 << 20);
        let stamp = Stamp::of_path(&a).unwrap();
        let all: Vec<PathBuf> = ["01", "02", "03"]
            .iter()
            .map(|n| a.join(format!("{n}.jpg")))
            .collect();
        // The thumbnails read it: kept, the tile shows the pages as without it.
        let o = cached_opener(&cache);
        assert_eq!(o.first_pages(&a, 4), all);
        let c = known_comic(&disk, &a, stamp).unwrap().unwrap();
        assert!(c.right_to_left() && c.pages.len() == 3);
        // Known at the reading mode's open, which shows every page left to right.
        let b = Book::open_for_reading(&a, jpg, 1, |s| known_comic(&disk, &a, s)).unwrap();
        assert!(b.comic_known().is_some_and(|c| c.is_some()));
        assert!(!b.comic_applies() && !b.right_to_left() && !b.suggests_spreads());
        assert_eq!(b.paths(), all);
        // Read by the reading mode: the same book, and kept.
        let _ = std::fs::remove_dir_all(&cache);
        let disk = DiskCache::new(&cache, 64 << 20);
        let b = Book::open_for_reading(&a, jpg, 1, |s| known_comic(&disk, &a, s)).unwrap();
        assert!(b.comic_known().is_none());
        let w = b.with_comic(b.read_comic_info().found().unwrap()).unwrap();
        assert!(!w.comic_applies() && !w.right_to_left());
        assert_eq!(w.paths(), b.paths());
        remember_comic(&disk, &w, w.comic_known().flatten());
        assert!(known_comic(&disk, &a, stamp).is_some_and(|c| c.is_some()));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A solid 7z (one LZMA2 unit: ComicInfo is reached from its start) of `n` pages of `len` bytes that barely
    /// compress, `xml` as its `ComicInfo.xml` last in the stream.
    fn solid_cb7(path: &Path, n: usize, len: usize, xml: &[u8]) {
        use sevenz_rust2::encoder_options::Lzma2Options;
        use sevenz_rust2::{ArchiveEntry, ArchiveWriter, EncoderConfiguration, SourceReader};
        let mut w = ArchiveWriter::new(std::io::Cursor::new(Vec::new())).unwrap();
        w.set_content_methods(vec![EncoderConfiguration::from(Lzma2Options::from_level(
            1,
        ))]);
        let mut entries: Vec<ArchiveEntry> = (0..n)
            .map(|i| ArchiveEntry::new_file(&format!("{:02}.jpg", i + 1)))
            .collect();
        entries.push(ArchiveEntry::new_file("ComicInfo.xml"));
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        let mut data: Vec<Vec<u8>> = (0..n)
            .map(|_| {
                (0..len)
                    .map(|_| {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        x as u8
                    })
                    .collect()
            })
            .collect();
        data.push(xml.to_vec());
        let readers = data
            .into_iter()
            .map(|d| SourceReader::new(std::io::Cursor::new(d)))
            .collect();
        w.push_archive_entries(entries, readers).unwrap();
        std::fs::write(path, w.finish().unwrap().into_inner()).unwrap();
    }

    #[test]
    fn comicinfo_late_in_a_solid_stream_is_kept_once_the_thumbnails_pass_it() {
        let d = scratch("comicpass");
        let a = d.join("b.cb7");
        let xml = br#"<ComicInfo><Manga>YesAndRightToLeft</Manga><Pages>
            <Page Image="0" Type="Deleted"/><Page Image="3" Type="FrontCover"/></Pages></ComicInfo>"#;
        // 1.8 MB of pages before it: past what the thumbnails decode to reach it.
        solid_cb7(&a, 12, 150_000, xml);
        let cache = d.join("cache");
        let disk = DiskCache::new(&cache, 64 << 20);
        let stamp = Stamp::of_path(&a).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = Mutex::new(tx);
        let o = cached_opener(&cache).with_cover_updates(Arc::new(move |a: &Path, covers| {
            tx.lock().send((a.to_path_buf(), covers)).unwrap();
        }));
        let page = |n: u32| a.join(format!("{n:02}.jpg"));
        assert_eq!(o.first_pages(&a, 4), [page(1), page(2), page(3), page(4)]);
        assert_eq!(known_comic(&disk, &a, stamp), None);
        // The archive's pages read for their thumbnails decode past ComicInfo.
        let paths: Vec<PathBuf> = (1..=12).map(page).collect();
        let refs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
        let mut n = 0;
        o.open_stream(&refs, &mut |_, r| {
            assert!(r.is_ok());
            n += 1;
            true
        });
        assert_eq!(n, 12);
        let want = vec![page(4), page(2), page(3), page(5)];
        let (got_a, covers) = rx.recv_timeout(Duration::from_secs(30)).unwrap();
        assert_eq!((got_a.as_path(), &covers), (a.as_path(), &want));
        // Kept with its tile pages: the kept archive and a new opener's cache show them.
        assert!(
            known_comic(&disk, &a, stamp).is_some_and(|c| c.is_some_and(|c| c.right_to_left()))
        );
        assert_eq!(o.first_pages(&a, 4), want);
        assert_eq!(o.opens(), 1);
        let o2 = cached_opener(&cache);
        assert_eq!(o2.first_pages(&a, 4), want);
        assert_eq!(o2.opens(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn cover_blobs_hold_paths_below_the_archive_only() {
        let a = Path::new("/x/b.cbz");
        let covers = vec![a.join("ch 1/01.jpg"), a.join("02.jpg")];
        let b = encode_covers(a, &covers).unwrap();
        assert_eq!(decode_covers(a, &b), Some(covers));
        assert_eq!(
            decode_covers(a, &encode_covers(a, &[]).unwrap()),
            Some(vec![])
        );
        assert_eq!(decode_covers(a, &b[..b.len() - 1]), None);
        assert_eq!(
            decode_covers(a, &[1, 5, 0, b'.', b'.', b'/', b'x', b'y']),
            None
        );
        assert_eq!(decode_covers(a, &[1, 3, 0, b'/', b'e', b't']), None);
    }

    /// Every page on its own, the archive opened again for each (what reading pages one by
    /// one through [`Book`] costs): the baseline of [`bench_archive_thumbnails`].
    struct Reopen(IsImage);

    impl Opener for Reopen {
        fn key(&self, path: &Path) -> io::Result<ThumbKey> {
            ArchiveOpener::new(self.0.clone()).key(path)
        }
        fn open(&self, path: &Path) -> io::Result<Opened> {
            let (a, _, _) = archive_of(path).ok_or_else(|| io::Error::other("not a page"))?;
            let b = Book::for_thumbnails(a, |p| (self.0)(p), 1, |_| None)
                .map_err(|e| io::Error::other(e.message()))?;
            let page = b
                .paths()
                .iter()
                .position(|p| p == path)
                .ok_or_else(|| io::Error::other("no page"))?;
            Ok(Opened::new(Arc::new(
                b.source(page).map_err(io::Error::other)?,
            )))
        }
    }

    /// Thumbnails of the archives in `$SPECULAR_ARCHBENCH` (`*.cbz`, `*.7z`, `*.cbr`): cold
    /// (an empty cache) through [`ArchiveOpener`] and through [`Reopen`] (its first
    /// `$SPECULAR_ARCHBENCH_REOPEN` pages, default 60: solid archives cost it O(n²)), and
    /// warm. With no archives there, writes `pages/NNN.jpg` (500 generated 1600×1200 JPEGs)
    /// to pack them from.
    #[test]
    #[ignore]
    fn bench_archive_thumbnails() {
        let Some(dir) = std::env::var_os("SPECULAR_ARCHBENCH").map(PathBuf::from) else {
            return;
        };
        let mut archives: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| is_archive_path(p) && p.is_file())
            .collect();
        archives.sort();
        if archives.is_empty() {
            let pages = dir.join("pages");
            std::fs::create_dir_all(&pages).unwrap();
            let (w, h) = (1600usize, 1200usize);
            for i in 0..500usize {
                let mut px = Vec::with_capacity(w * h * 4);
                let mut x = (i as u32).wrapping_mul(2_654_435_761) | 1;
                for y in 0..h {
                    for c in 0..w {
                        x ^= x << 13;
                        x ^= x >> 17;
                        x ^= x << 5;
                        let n = (x & 31) as usize;
                        let r = ((c * 255 / w + i * 7 + n) & 255) as u8;
                        let g = ((y * 255 / h + n) & 255) as u8;
                        let b = (((c + y) * 128 / (w + h) + i * 3) & 255) as u8;
                        px.extend_from_slice(&[r, g, b, 255]);
                    }
                }
                let jpg = specular_thumbs::codec::encode_jpeg(&px, w, h, 85);
                std::fs::write(pages.join(format!("{:03}.jpg", i + 1)), jpg).unwrap();
            }
            eprintln!("wrote 500 pages to {}", pages.display());
            return;
        }
        let reopen_n: usize = std::env::var("SPECULAR_ARCHBENCH_REOPEN")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
        let is_image: IsImage = Arc::new(|p: &Path| p.extension().is_some_and(|e| e == "jpg"));
        // `$SPECULAR_ARCHBENCH_7Z_THREADS`: a solid 7z stream's decode threads (A/B runs).
        let threads = std::env::var("SPECULAR_ARCHBENCH_7Z_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(decode_threads);
        eprintln!("7z decode threads: {threads}");
        let bench_opener = || ArchiveOpener::new(is_image.clone()).with_decode_threads(threads);
        // `$SPECULAR_ARCHBENCH_CACHES`: where the caches go (default next to the archives).
        let caches = std::env::var_os("SPECULAR_ARCHBENCH_CACHES")
            .map_or_else(|| dir.join("caches"), PathBuf::from);
        for a in archives {
            let name = a.file_name().unwrap().to_string_lossy().into_owned();
            let pages = match Book::for_thumbnails(&a, |p| is_image(p), 1, |_| None) {
                Ok(b) => b.paths().to_vec(),
                // RAR without a decode host: nothing to measure here.
                Err(BookError::NeedsSandbox(f)) => {
                    eprintln!("{name}: skipped, {} needs the decode host", f.name());
                    continue;
                }
                Err(e) => panic!("{name}: {e:?}"),
            };
            let fresh = |what: &str| {
                let c = caches.join(format!("{name}-{what}"));
                let _ = std::fs::remove_dir_all(&c);
                c
            };
            let timed = |o: Arc<dyn Opener>, cache: &Path, pages: &[PathBuf]| {
                let e = engine_with(o, cache, workers);
                let t = Instant::now();
                let got = run(&e, pages);
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                let tm = e.try_take_timings().unwrap_or_default();
                let open = tm.iter().map(|t| t.open_ms).sum::<f64>() / tm.len().max(1) as f64;
                (ms, got, open)
            };
            // 0 leaves out the first-pages runs.
            let n = reopen_n.min(pages.len());
            if n > 0 {
                let (ms, got, open) = timed(
                    Arc::new(Reopen(is_image.clone())),
                    &fresh("reopen"),
                    &pages[..n],
                );
                eprintln!(
                    "{name}: reopen cold {n} pages: {ms:.0} ms ({:.1} ms/page, open {open:.1} ms) {got:?}",
                    ms / n as f64
                );
                let o = Arc::new(bench_opener());
                let (ms, got, open) = timed(o.clone(), &fresh("shared"), &pages[..n]);
                eprintln!(
                    "{name}: shared cold {n} pages: {ms:.0} ms ({:.1} ms/page, open {open:.1} ms) {got:?}, {} opens",
                    ms / n as f64,
                    o.opens()
                );
            }
            let o = Arc::new(bench_opener());
            let cache = fresh("all");
            let (ms, got, open) = timed(o.clone(), &cache, &pages);
            eprintln!(
                "{name}: shared cold {} pages: {ms:.0} ms ({:.2} ms/page, open {open:.1} ms) {got:?}, {} opens",
                pages.len(),
                ms / pages.len() as f64,
                o.opens()
            );
            let o = Arc::new(bench_opener());
            let (ms, got, _) = timed(o.clone(), &cache, &pages);
            eprintln!(
                "{name}: shared warm {} pages: {ms:.0} ms ({:.2} ms/page) {got:?}, {} opens",
                pages.len(),
                ms / pages.len() as f64,
                o.opens()
            );
        }
    }

    /// CPU time of the process so far (ms; Linux only, 0 elsewhere).
    fn cpu_ms() -> f64 {
        let Ok(s) = std::fs::read_to_string("/proc/self/stat") else {
            return 0.0;
        };
        let f: Vec<&str> = s
            .rsplit(')')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect();
        let t = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        (t(11) + t(12)) * 10.0
    }

    /// Peak resident memory (MiB) since [`reset_peak_rss`] (Linux only, 0 elsewhere).
    fn peak_rss_mib() -> f64 {
        let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        s.lines()
            .find_map(|l| l.strip_prefix("VmHWM:"))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<f64>().ok())
            .map_or(0.0, |kb| kb / 1024.0)
    }

    fn reset_peak_rss() {
        let _ = std::fs::write("/proc/self/clear_refs", "5");
    }

    /// The reading mode's open of the archive at `$SPECULAR_COMICBENCH` (a solid 7z with
    /// ComicInfo.xml at the end of its stream), `$SPECULAR_COMICBENCH_ROUNDS` (3) rounds of:
    /// ComicInfo not in the cache (read on a thread of its own, aside from the reading streamer, while
    /// the first page and 20 more are read, then kept), and in the cache (known at open). Prints
    /// open, first page, 20 pages, when ComicInfo is applied, CPU time and peak RSS.
    #[test]
    #[ignore]
    fn bench_comic_info() {
        let Some(a) = std::env::var_os("SPECULAR_COMICBENCH").map(PathBuf::from) else {
            return;
        };
        let rounds: usize = std::env::var("SPECULAR_COMICBENCH_ROUNDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);
        let threads = crate::book::reading_threads(
            std::thread::available_parallelism().map_or(4, |n| n.get()),
        );
        let caches = std::env::var_os("SPECULAR_ARCHBENCH_CACHES")
            .map_or_else(|| std::env::temp_dir().join("comicbench"), PathBuf::from);
        let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
        eprintln!("{}: {threads} decode threads", a.display());
        for round in 0..rounds {
            for hit in [false, true] {
                let dir = caches.join("comic-cache");
                if !hit {
                    let _ = std::fs::remove_dir_all(&dir);
                }
                let disk = Arc::new(DiskCache::new(&dir, 64 << 20));
                reset_peak_rss();
                let rss0 = peak_rss_mib();
                let cpu0 = cpu_ms();
                let t = Instant::now();
                let known = |s| if hit { known_comic(&disk, &a, s) } else { None };
                let b = Arc::new(Book::open_for_reading(&a, jpg, threads, known).unwrap());
                let open = ms(t);
                let comic = (b.comic_known().is_none()).then(|| {
                    let (b, disk) = (b.clone(), disk.clone());
                    std::thread::spawn(move || {
                        let r = b.read_comic_info().found().unwrap();
                        let w = b.with_comic(r).unwrap();
                        remember_comic(&disk, &w, w.comic_known().flatten());
                        (ms(t), w.len())
                    })
                });
                let first_len = b.source(0).unwrap().bytes().len();
                let first = ms(t);
                let t2 = Instant::now();
                for p in 1..=20 {
                    assert!(!b.source(p).unwrap().bytes().is_empty());
                }
                let flip = ms(t2);
                let (comic_at, pages) = match comic {
                    Some(h) => h.join().unwrap(),
                    None => (open, b.len()),
                };
                let total = ms(t);
                eprintln!(
                    "round {round} {}: open {open:.1} ms, first page {first:.0} ms ({first_len} B), \
                     20 pages {flip:.0} ms, ComicInfo at {comic_at:.0} ms ({pages} pages; at open {} pages, rtl {}), \
                     total {total:.0} ms, CPU {:.0} ms, peak RSS +{:.0} MiB",
                    if hit { "cached" } else { "not cached" },
                    b.len(),
                    b.right_to_left(),
                    cpu_ms() - cpu0,
                    peak_rss_mib() - rss0,
                );
            }
        }
        eprintln!(
            "load {}",
            std::fs::read_to_string("/proc/loadavg")
                .unwrap_or_default()
                .trim()
        );
    }

    #[test]
    fn virtual_paths_split_by_name() {
        let (a, m) = split(Path::new("/x/b.cbz/ch 1/003.jpg")).unwrap();
        assert_eq!(a, Path::new("/x/b.cbz"));
        assert_eq!(m, "ch 1/003.jpg");
        assert!(split(Path::new("/x/b/003.jpg")).is_none());
        assert!(split(Path::new("/x/b.cbz")).is_none());
    }
}
