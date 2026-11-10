//! Archives (ZIP/CBZ, 7z/CB7, TAR/CBT, RAR/CBR through the sandbox host) as image sets, and
//! the reading-mode layout of their pages: right-to-left order and two-page spreads.
//!
//! Every page gets a *virtual path*, `<archive path>/<entry name>`, that the app lists like a
//! file in a folder. The loader resolves it through [`Books`] to the entry's bytes; it is never
//! opened as a real file (entry names are sanitised anyway, so a `..` in an archive cannot
//! point the path outside it).

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_archive::{
    Archive, ComicInfo, ComicLookup, Error as ArchiveError, Format, Manga, Options, PageType,
    Password,
};
use specular_core::Source;
use specular_thumbs::Stamp;

/// Passwords of the archives unlocked this session, in memory only: keyed by path and stamp
/// (a changed archive asks again), never written anywhere, wiped when forgotten or at exit
/// ([`Password`] zeroizes its bytes on drop).
pub mod passwords {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{LazyLock, Mutex};

    use specular_archive::Password;
    use specular_thumbs::Stamp;

    static KNOWN: LazyLock<Mutex<HashMap<(PathBuf, Stamp), Password>>> =
        LazyLock::new(Default::default);

    fn known() -> std::sync::MutexGuard<'static, HashMap<(PathBuf, Stamp), Password>> {
        KNOWN.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The password the archive at `path` was unlocked with, as of `stamp`.
    pub fn get(path: &Path, stamp: Stamp) -> Option<Password> {
        known().get(&(path.to_path_buf(), stamp)).cloned()
    }

    /// Remembers `password` for the archive at `path` as of `stamp` (one that opened it).
    pub fn remember(path: &Path, stamp: Stamp, password: Password) {
        let mut k = known();
        // An archive changed since it was unlocked has no use for the old password.
        k.retain(|(p, _), _| p != path);
        k.insert((path.to_path_buf(), stamp), password);
    }

    #[cfg(test)]
    pub fn forget(path: &Path) {
        known().retain(|(p, _), _| p != path);
    }
}

/// Whether `p` names an archive (by extension, `.tar.gz` included; content is checked when it
/// is opened).
pub fn is_archive_path(p: &Path) -> bool {
    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
        // A zipped OME-Zarr store is one image, a zipped FictionBook one book: not folders.
        let lower = n.to_ascii_lowercase();
        Format::from_file_name(n).is_some()
            && !lower.ends_with(".zarr.zip")
            && !lower.ends_with(".fb2.zip")
    }) && !specular_decode::is_board_archive(p)
        && !specular_decode::is_odb_archive(p)
        && !specular_decode::is_sentinel2_archive(p)
}

/// Why an archive could not be browsed.
#[derive(Debug)]
pub enum BookError {
    /// RAR without the sandboxed decode host.
    NeedsSandbox(Format),
    /// No pages the decoders can show.
    Empty,
    /// Its pages (or its file list) are encrypted and no password is known for it.
    Locked,
    /// The password given does not decrypt it.
    WrongPassword,
    Failed(String),
}

impl BookError {
    /// The message shown to the user.
    pub fn message(&self) -> String {
        use specular_i18n::t;
        match self {
            BookError::NeedsSandbox(f) => t!("archive-needs-sandbox", format = f.name()),
            BookError::Empty => t!("archive-empty"),
            BookError::Locked => t!("archive-encrypted"),
            BookError::WrongPassword => t!("archive-wrong-password"),
            BookError::Failed(e) => t!("archive-failed", error = e.clone()),
        }
    }
}

impl From<ArchiveError> for BookError {
    fn from(e: ArchiveError) -> Self {
        match e {
            ArchiveError::NeedsSandbox(f) => BookError::NeedsSandbox(f),
            ArchiveError::Encrypted => BookError::Locked,
            ArchiveError::WrongPassword => BookError::WrongPassword,
            e => BookError::Failed(crate::error_text::archive_reason(&e)),
        }
    }
}

/// An open archive and its pages.
pub struct Book {
    path: PathBuf,
    source: Arc<Source>,
    /// Shared with the book [`Book::with_comic`] makes of this one.
    archive: Arc<Archive>,
    /// Archive entry of every image, ComicInfo's deleted pages included.
    images: Vec<usize>,
    /// Archive entry of each page, in page order.
    entries: Vec<usize>,
    /// ComicInfo image number of each page (pages marked deleted are left out).
    numbers: Vec<u32>,
    /// Virtual path of each page.
    paths: Vec<PathBuf>,
    comic: Option<ComicInfo>,
    /// Whether `comic` shapes the book: not when it marks every page deleted (the book then
    /// shows them all, in archive order, as without ComicInfo, but keeps it to be cached).
    comic_applies: bool,
    /// The page ComicInfo marks as the front cover, when it is not the first.
    front_cover: Option<usize>,
    /// Whether `comic` is all there is to know: ComicInfo was read (or known from the
    /// cache), not left for later or skipped as too costly to reach.
    comic_known: bool,
    /// Size and time stamp of the archive file when it was opened (`None` for bytes not
    /// from a file).
    stamp: Option<Stamp>,
    /// Whether any page is encrypted: opened with a password, so nothing of it (pixels,
    /// entry names) is written to a disk cache.
    encrypted: bool,
}

/// Whether to read `ComicInfo.xml` when opening.
enum Comic {
    #[cfg(test)]
    Read,
    /// Not now: [`Book::read_comic_info`] reads it.
    Later,
    /// Only when it costs little to reach ([`Archive::comic_info_within`] this many bytes).
    Within(u64),
    /// Known already (from the cache, [`crate::archive_thumbs::known_comic`]); `None`: the
    /// archive has none.
    Known(Option<ComicInfo>),
}

/// What is known of an archive's ComicInfo as of its stamp, without reading it: `None` when
/// nothing is, `Some(None)` when it has none.
pub type KnownComic = Option<Option<ComicInfo>>;

/// Bytes of a solid stream thumbnails decode to reach ComicInfo before giving up on it.
const THUMB_COMIC_BUDGET: u64 = 1 << 20;

impl std::fmt::Debug for Book {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Book")
            .field("path", &self.path)
            .field("pages", &self.entries.len())
            .finish()
    }
}

/// Bytes of a solid 7z block the reading mode decodes past the furthest page asked for:
/// some 20 pages of a typical book (1-4 MB each), and a few 16 MiB LZMA2 units of a book
/// 7-Zip packed with `-mmt`, so its helper threads have the next units to decode.
const READ_AHEAD: usize = 64 << 20;

/// Threads a solid 7z block's LZMA2 stream is decoded with in reading mode, for a loader of
/// `loader_threads` image decoders: half of them, at most 4. The loader decodes only a few
/// pages at once (the one shown and its prefetch, `--prefetch-depth` 2 ahead and 1 behind),
/// so most of its threads idle while the archive hands over the bytes, and the LZMA2 helpers
/// work in bursts (after opening or a jump, then only as far as [`READ_AHEAD`]) within
/// `specular_archive`'s 256 MiB of decoded units. Measured on 16 cores (books of 16 MiB and
/// of 64-128 MiB units): 4 threads read 120 pages back to back 1.2-10x faster than 1; a jump
/// is no faster with more than 2-4 (the helpers then compete with the thread serving it);
/// 8 cost more CPU to flip faster than the loader decodes; and a book has few units anyway
/// (a 300 MB book from 7-Zip's defaults has 3).
pub fn reading_threads(loader_threads: usize) -> u32 {
    (loader_threads / 2).clamp(1, 4) as u32
}

/// First thing on a reading mode archive thread (a solid 7z streamer, its LZMA2 helpers): the
/// scheduling class and cores of the loader's foreground jobs, whose pages they decode.
fn reading_thread_start() {
    specular_platform::set_thread_class(specular_platform::ThreadClass::UserInitiated);
    specular_core::placement::place_current_thread(specular_core::placement::CoreSet::Performance);
}

/// First thing on a thumbnail archive thread: what the engine's background workers do for
/// a visible job (`Utility` class; on Windows, which does not pass a thread's priority on to
/// the threads it starts, the lower priority of the background pool's workers). On Linux the
/// thread already has the nice value and the cores of the worker that started it (lowered
/// in the gallery's background pool, not in the `--thumbnail-cache` warmer's; the fast
/// cores for a visible job): lowering it again would put it below that worker.
/// `lzma-rust2`'s own multi-threaded reader spawns threads with no such hook, so the
/// archive crate drives the LZMA2 units on threads of its own.
fn thumbnail_thread_start() {
    #[cfg(not(target_os = "linux"))]
    specular_platform::lower_thread_priority();
    specular_platform::set_thread_class(specular_platform::ThreadClass::Utility);
}

impl Book {
    /// Opens the archive at `path`; `is_image` picks the pages by entry name.
    #[cfg(test)]
    pub fn open(path: &Path, is_image: impl Fn(&Path) -> bool) -> Result<Book, BookError> {
        Book::open_with(path, is_image, Options::default(), |_| Comic::Read, None)
    }

    /// Opens the archive at `path` for the reading mode: a solid 7z block decoded on
    /// `decode_threads` threads where its LZMA2 stream allows ([`reading_threads`]) and
    /// [`READ_AHEAD`] bytes past the furthest page asked for. ComicInfo is not read (in a
    /// solid archive it often comes after every page): it is taken from `known` (given the
    /// archive's stamp, so a changed archive never gets an older one's), else
    /// [`Book::read_comic_info`] reads it off the UI thread while the first page decodes.
    /// An encrypted archive opens with the password it was unlocked with this session
    /// ([`passwords`]), else fails with [`BookError::Locked`].
    #[cfg(test)]
    pub fn open_for_reading(
        path: &Path,
        is_image: impl Fn(&Path) -> bool,
        decode_threads: u32,
        known: impl FnOnce(Stamp) -> KnownComic,
    ) -> Result<Book, BookError> {
        Book::unlock(path, is_image, decode_threads, known, None)
    }

    /// Opens the archive at `path` for the reading mode: a solid 7z block decoded on
    /// `decode_threads` threads where its LZMA2 stream allows ([`reading_threads`]) and
    /// [`READ_AHEAD`] bytes past the furthest page asked for; ComicInfo from `known` (given
    /// the archive's stamp), else read later ([`Book::read_comic_info`]). An encrypted
    /// archive opens with `password`, else the one it was unlocked with this session
    /// ([`passwords`]), else fails with [`BookError::Locked`]; [`BookError::WrongPassword`]
    /// when it does not decrypt it. The caller remembers a password that works
    /// ([`passwords::remember`]).
    pub fn unlock(
        path: &Path,
        is_image: impl Fn(&Path) -> bool,
        decode_threads: u32,
        known: impl FnOnce(Stamp) -> KnownComic,
        password: Option<Password>,
    ) -> Result<Book, BookError> {
        let options = Options {
            decode_threads,
            solid_read_ahead: READ_AHEAD,
            thread_start: Some(reading_thread_start),
            ..Options::default()
        };
        let comic = |stamp: Option<Stamp>| match stamp.and_then(known) {
            Some(c) => Comic::Known(c),
            None => Comic::Later,
        };
        Book::open_with(path, is_image, options, comic, password)
    }

    /// Opens the archive at `path` with `options`, ComicInfo as `comic` says for its stamp,
    /// with `password`, else the one remembered for it.
    fn open_with(
        path: &Path,
        is_image: impl Fn(&Path) -> bool,
        mut options: Options,
        comic: impl FnOnce(Option<Stamp>) -> Comic,
        password: Option<Password>,
    ) -> Result<Book, BookError> {
        // Stat before mapping: a change in between makes the stamp stale, never wrongly fresh.
        let stamp = Stamp::of_path(path).ok();
        options.password = password.or_else(|| stamp.and_then(|s| passwords::get(path, s)));
        let source =
            Source::open(path).map_err(|e| BookError::Failed(crate::error_text::io_reason(&e)))?;
        // Pages are read in viewing order, not file order: no read-ahead.
        source.advise_random();
        let mut b = Book::build(Arc::new(source), is_image, options, comic(stamp))?;
        b.stamp = stamp;
        Ok(b)
    }

    #[cfg(test)]
    pub fn from_source(
        source: Arc<Source>,
        is_image: impl Fn(&Path) -> bool,
    ) -> Result<Book, BookError> {
        Book::build(source, is_image, Options::default(), Comic::Read)
    }

    /// The archive at `path` for its thumbnails: ComicInfo only when it is cheap to reach
    /// (stored or compressed on its own, or within the first [`THUMB_COMIC_BUDGET`] bytes of
    /// a solid stream; in solid archives it often comes last, and reaching it would decode
    /// every page) or `known` for the archive's stamp, so pages it marks deleted are left out
    /// and its front cover leads the mosaic as far as it can be known cheaply. A small cache
    /// of a solid 7z block's decoded entries (thumbnails read them in stream order,
    /// [`Book::read_pages`]) and its LZMA2 stream decoded on `decode_threads` threads where it
    /// allows, at the priority of the engine's background workers. Thumbnails never ask for a
    /// password: an encrypted archive opens only once the viewer unlocked it this session.
    pub fn for_thumbnails(
        path: &Path,
        is_image: impl Fn(&Path) -> bool,
        decode_threads: u32,
        known: impl FnOnce(Stamp) -> KnownComic,
    ) -> Result<Book, BookError> {
        let options = Options {
            solid_cache_bytes: 16 << 20,
            decode_threads,
            thread_start: Some(thumbnail_thread_start),
            ..Options::default()
        };
        let comic = |stamp: Option<Stamp>| match stamp.and_then(known) {
            Some(c) => Comic::Known(c),
            None => Comic::Within(THUMB_COMIC_BUDGET),
        };
        Book::open_with(path, is_image, options, comic, None)
    }

    fn build(
        source: Arc<Source>,
        is_image: impl Fn(&Path) -> bool,
        options: Options,
        comic: Comic,
    ) -> Result<Book, BookError> {
        let unlocked = options.password.is_some();
        let archive = Archive::from_bytes(source.clone(), options)?;
        let (comic, known) = match comic {
            #[cfg(test)]
            Comic::Read => (archive.comic_info(), true),
            Comic::Later => (None, false),
            Comic::Within(budget) => match archive.comic_info_lookup(Some(budget)) {
                ComicLookup::Found(c) => (Some(c), true),
                ComicLookup::Absent => (None, true),
                ComicLookup::Unread => (None, false),
            },
            Comic::Known(c) => (c, true),
        };
        // Encrypted pages need the password; without it the book is locked rather than
        // shown without them.
        let images = archive.pages(|e| is_image(Path::new(&e.name)));
        let encrypted = images.iter().any(|&i| archive.entries()[i].encrypted);
        if encrypted && !unlocked {
            return Err(BookError::Locked);
        }
        if encrypted || archive.is_encrypted() {
            archive.check_password()?;
        }
        let encrypted = encrypted || archive.is_encrypted();
        let mut b = Book::assemble(source, Arc::new(archive), images, comic)?;
        b.comic_known = known;
        b.encrypted = encrypted;
        Ok(b)
    }

    /// The book of `archive`'s `images` with what `comic` says about them.
    fn assemble(
        source: Arc<Source>,
        archive: Arc<Archive>,
        images: Vec<usize>,
        comic: Option<ComicInfo>,
    ) -> Result<Book, BookError> {
        let path = source.path().to_path_buf();
        let mut entries = images.clone();
        // ComicRack's "deleted" pages are hidden in reading mode. `Page Image` counts images.
        let mut numbers: Vec<u32> = (0..entries.len() as u32).collect();
        let mut comic_applies = comic.is_some();
        if let Some(c) = &comic {
            let keep = |n: &u32| c.page(*n).is_none_or(|p| p.kind != PageType::Deleted);
            // ComicInfo that deletes every page is ignored (but known).
            comic_applies = numbers.iter().any(keep);
            if comic_applies {
                entries = entries
                    .into_iter()
                    .zip(&numbers)
                    .filter(|(_, n)| keep(n))
                    .map(|(e, _)| e)
                    .collect();
                numbers.retain(keep);
            }
        }
        if entries.is_empty() {
            return Err(BookError::Empty);
        }
        let applied = comic.as_ref().filter(|_| comic_applies);
        let front_cover = applied.and_then(|c| {
            numbers
                .iter()
                .position(|&n| c.page(n).is_some_and(|p| p.kind == PageType::FrontCover))
                .filter(|&i| i > 0)
        });
        let mut seen = std::collections::HashSet::new();
        let paths = entries
            .iter()
            .enumerate()
            .map(|(i, &e)| {
                let mut p = virtual_path(&path, &archive.entries()[e].name);
                if !seen.insert(p.clone()) {
                    // Two names that sanitise alike: keep both reachable.
                    p = virtual_path(&path, &format!("{}#{i}", archive.entries()[e].name));
                    seen.insert(p.clone());
                }
                p
            })
            .collect();
        Ok(Book {
            path,
            source,
            archive,
            images,
            entries,
            numbers,
            paths,
            comic_known: comic.is_some(),
            comic,
            comic_applies,
            front_cover,
            stamp: None,
            encrypted: false,
        })
    }

    /// The ComicInfo of a book opened without it ([`Book::open_for_reading`]), for
    /// [`Book::with_comic`]. Blocking, at a lowered priority of the calling thread (meant for
    /// a thread of its own). In a solid 7z block it is decoded aside from the reading mode's
    /// streamer, which stays where the pages are read: from the start of its LZMA2 unit, or
    /// taken from the streamer (or its helpers) as soon as that decodes it in passing
    /// ([`Archive::comic_info_aside`]).
    pub fn read_comic_info(&self) -> ComicLookup {
        specular_platform::lower_thread_priority();
        specular_platform::set_thread_class(specular_platform::ThreadClass::Utility);
        self.archive.comic_info_aside()
    }

    /// Runs `f` with the ComicInfo of a solid 7z block once any read of the book decodes past
    /// it ([`Archive::on_comic_info`]); false when the book has none such.
    pub fn on_comic_info(&self, f: impl FnOnce(ComicLookup) + Send + 'static) -> bool {
        self.archive.on_comic_info(f)
    }

    /// This book with `comic` (its ComicInfo, [`Book::read_comic_info`]): its deleted pages
    /// left out, its reading order and spreads. `None` if the book has no pages.
    pub fn with_comic(&self, comic: ComicInfo) -> Option<Book> {
        let mut b = Book::assemble(
            self.source.clone(),
            self.archive.clone(),
            self.images.clone(),
            Some(comic),
        )
        .ok()?;
        b.stamp = self.stamp;
        b.encrypted = self.encrypted;
        Some(b)
    }

    /// Whether the book's ComicInfo shapes it (false without one, or when it marks every
    /// page deleted and is ignored).
    pub fn comic_applies(&self) -> bool {
        self.comic_applies
    }

    /// The ComicInfo that shapes the book.
    fn applied(&self) -> Option<&ComicInfo> {
        self.comic.as_ref().filter(|_| self.comic_applies)
    }

    /// Whether the book knows its ComicInfo (`Some`, holding it or `None` for none), rather
    /// than having left it for later or skipped it as too costly to reach.
    pub fn comic_known(&self) -> Option<Option<&ComicInfo>> {
        self.comic_known.then_some(self.comic.as_ref())
    }

    /// `other` is this book again (the same open of the same archive), maybe with more
    /// known about it ([`Book::with_comic`]).
    pub fn same_open(&self, other: &Book) -> bool {
        Arc::ptr_eq(&self.archive, &other.archive)
    }

    /// The archive's size and time stamp when it was opened from a file.
    pub fn stamp(&self) -> Option<Stamp> {
        self.stamp
    }

    /// Whether the book was opened with a password: its pages, names and covers stay in
    /// memory, never in a disk cache.
    pub fn encrypted(&self) -> bool {
        self.encrypted
    }

    /// Whether other readers (the thumbnails) may read pages through this book while it is
    /// read elsewhere: not when a solid stream keeps state between reads
    /// ([`Archive::has_stream_state`]), which readers in different places would fight over.
    pub fn shares_reads(&self) -> bool {
        !self.archive.has_stream_state()
    }

    /// The pages of the archive's tile: the first `n`, ComicInfo's front cover first when it
    /// marks one (and the reader would show it later).
    pub fn covers(&self, n: usize) -> Vec<PathBuf> {
        let lead = self.front_cover.into_iter();
        let rest = (0..self.paths.len()).filter(|&i| Some(i) != self.front_cover);
        lead.chain(rest)
            .take(n)
            .map(|i| self.paths[i].clone())
            .collect()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Virtual paths of the pages, in page order.
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// The entry name of page `page` ("chapter 1/003.jpg").
    pub fn entry_name(&self, page: usize) -> Option<&str> {
        let e = *self.entries.get(page)?;
        Some(&self.archive.entries()[e].name)
    }

    /// The page bytes of `page`: a view into the archive when stored uncompressed, otherwise
    /// decompressed into memory.
    pub fn source(&self, page: usize) -> Result<Source, String> {
        let (Some(&e), Some(vpath)) = (self.entries.get(page), self.paths.get(page)) else {
            return Err(specular_i18n::t!("error-reason-missing-entry"));
        };
        if let Some(part) = self.archive.stored_slice(e)
            && let Some(s) = Source::from_slice(vpath.clone(), self.source.clone(), part)
        {
            return Ok(s);
        }
        // The page's bytes keep their share of the memory budget while the source holds them.
        let (data, reserved) = self
            .archive
            .read_reserved(e)
            .map_err(|e| crate::error_text::archive_reason(&e))?;
        Ok(Source::from_entry(vpath.clone(), &self.source, data).with_reservation(reserved))
    }

    /// Where page `page` lies in a stream decoded from its start (a solid 7z block, a solid
    /// RAR archive): the stream's number and the page's place in it ([`Archive::stream_position`]).
    pub fn stream_position(&self, page: usize) -> Option<(u64, u64)> {
        self.archive.stream_position(*self.entries.get(page)?)
    }

    /// The pages `pages` as [`Book::source`] gives them, handed to `each` with their position
    /// in `pages` as soon as each is read; stops when `each` returns false. Pages of a solid
    /// stream are read in stream order, decoding it once ([`Archive::read_many`]); stored
    /// ones first, as views of the archive.
    pub fn read_pages(
        &self,
        pages: &[usize],
        each: &mut dyn FnMut(usize, Result<Source, String>) -> bool,
    ) {
        let mut rest = Vec::with_capacity(pages.len());
        for (k, &page) in pages.iter().enumerate() {
            let (Some(&e), Some(vpath)) = (self.entries.get(page), self.paths.get(page)) else {
                if !each(k, Err(specular_i18n::t!("error-reason-missing-entry"))) {
                    return;
                }
                continue;
            };
            if let Some(part) = self.archive.stored_slice(e)
                && let Some(s) = Source::from_slice(vpath.clone(), self.source.clone(), part)
            {
                if !each(k, Ok(s)) {
                    return;
                }
                continue;
            }
            rest.push(k);
        }
        let entries: Vec<usize> = rest.iter().map(|&k| self.entries[pages[k]]).collect();
        self.archive.read_many_reserved(&entries, &mut |j, r| {
            let k = rest[j];
            let vpath = self.paths[pages[k]].clone();
            each(
                k,
                r.map(|(data, reserved)| {
                    Source::from_entry(vpath, &self.source, data).with_reservation(reserved)
                })
                .map_err(|e| crate::error_text::archive_reason(&e)),
            )
        });
    }

    /// ComicInfo says the book reads right to left (manga).
    pub fn right_to_left(&self) -> bool {
        self.applied().is_some_and(|c| c.right_to_left())
    }

    /// ComicInfo suggests a printed-book layout: a manga, or pages marked as spreads.
    pub fn suggests_spreads(&self) -> bool {
        self.applied().is_some_and(|c| {
            matches!(c.manga, Manga::Yes | Manga::YesAndRightToLeft) || c.spreads().next().is_some()
        })
    }

    /// ComicInfo marks page `page` as a double-page image (shown alone in spread mode).
    pub fn is_double_page(&self, page: usize) -> bool {
        self.applied()
            .zip(self.numbers.get(page))
            .and_then(|(c, &n)| c.page(n))
            .is_some_and(|p| p.double_page)
    }
}

/// `<archive>/<entry>` with every `.`/`..`/empty component of the entry replaced, so the path
/// stays inside the archive's name even if something joins it like a real one.
fn virtual_path(archive: &Path, entry: &str) -> PathBuf {
    let mut s = OsString::from(archive.as_os_str());
    for part in entry.split(['/', '\\']) {
        s.push("/");
        match part {
            "" | "." | ".." => s.push("_"),
            p => s.push(p),
        }
    }
    PathBuf::from(s)
}

/// Page virtual path → (its book, page index).
type BookPages = HashMap<PathBuf, (Arc<Book>, usize)>;

/// The open archives, by page virtual path. Shared between the app and the loader threads.
#[derive(Clone, Default)]
pub struct Books {
    map: Arc<parking_lot::Mutex<BookPages>>,
}

impl Books {
    /// Makes `book`'s pages openable (replacing an earlier open of the same archive).
    pub fn insert(&self, book: Arc<Book>) {
        let mut m = self.map.lock();
        m.retain(|_, (b, _)| b.path() != book.path());
        for (i, p) in book.paths().iter().enumerate() {
            m.insert(p.clone(), (book.clone(), i));
        }
    }

    /// Forgets every archive except `keep`.
    pub fn retain(&self, keep: Option<&Path>) {
        self.map.lock().retain(|_, (b, _)| Some(b.path()) == keep);
    }

    /// The book and page a virtual path names.
    pub fn get(&self, path: &Path) -> Option<(Arc<Book>, usize)> {
        self.map.lock().get(path).cloned()
    }

    /// The open book of the archive at `archive`, if any.
    pub fn book(&self, archive: &Path) -> Option<Arc<Book>> {
        let m = self.map.lock();
        m.values()
            .find(|(b, _)| b.path() == archive)
            .map(|(b, _)| b.clone())
    }
}

/// Reading-mode layout of a book: which pages share the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spreads {
    /// First page of every screen, ascending. A screen is `starts[i]..starts[i + 1]`.
    starts: Vec<usize>,
    len: usize,
}

impl Spreads {
    /// Pairs pages two by two. The cover (page 0) is alone, as in a printed book, and so is
    /// every `wide` page (a spread already); pairing restarts after it so the pages that face
    /// each other in print stay together.
    pub fn new(len: usize, wide: impl Fn(usize) -> bool) -> Spreads {
        let mut starts = Vec::new();
        let mut i = 0;
        while i < len {
            starts.push(i);
            if i == 0 || wide(i) || i + 1 >= len || wide(i + 1) {
                i += 1;
            } else {
                i += 2;
            }
        }
        Spreads { starts, len }
    }

    /// The screen `page` is on.
    pub fn screen(&self, page: usize) -> usize {
        match self.starts.binary_search(&page) {
            Ok(s) => s,
            Err(s) => s.saturating_sub(1),
        }
    }

    /// The pages on screen with `page`.
    pub fn screen_of(&self, page: usize) -> std::ops::Range<usize> {
        let s = self.screen(page);
        let start = self.starts.get(s).copied().unwrap_or(0);
        let end = self.starts.get(s + 1).copied().unwrap_or(self.len);
        start..end.max(start + 1)
    }

    /// First page of the screen `delta` screens away from `page`'s (clamped), and whether it
    /// had to clamp.
    pub fn step(&self, page: usize, delta: i64) -> (usize, bool) {
        if self.starts.is_empty() {
            return (0, true);
        }
        let s = self.screen(page) as i64 + delta;
        let t = s.clamp(0, self.starts.len() as i64 - 1);
        (self.starts[t as usize], t != s)
    }

    pub fn screens(&self) -> usize {
        self.starts.len()
    }
}

/// The page each book was last read at, kept in `book-pages.tsv` in the config folder: one
/// line per archive, `path \t entry name`, the most recent last. The page is found again by
/// its entry name, so pages ComicInfo hides, or an archive repacked in another order, do not
/// move it. Books opened with a password are not remembered (their entry names stay in
/// memory only).
pub mod positions {
    use std::path::Path;

    /// The table's file name.
    pub const FILE: &str = "book-pages.tsv";

    /// Books remembered at most; the oldest go first.
    pub const MAX_REMEMBERED: usize = 500;

    fn parse_line(line: &str) -> Option<(&str, &str)> {
        let (p, e) = line.split_once('\t')?;
        (!p.is_empty() && !e.is_empty() && !e.contains('\t')).then_some((p, e))
    }

    /// The entry name remembered for `book` in the table's `text`.
    pub fn find<'a>(text: &'a str, book: &Path) -> Option<&'a str> {
        let p = book.to_str()?;
        text.lines()
            .filter_map(parse_line)
            .filter(|(q, _)| *q == p)
            .map(|(_, e)| e)
            .next_back()
    }

    /// The table's `text` with `book` at `entry` put last (replacing what it had), the
    /// oldest dropped past [`MAX_REMEMBERED`]; `None` when the path or name cannot be stored
    /// (a tab or line break in it).
    pub fn with_entry(text: &str, book: &Path, entry: &str) -> Option<String> {
        let p = book.to_str()?;
        let bad = |s: &str| s.is_empty() || s.contains(['\t', '\n', '\r']);
        if bad(p) || bad(entry) {
            return None;
        }
        let line = format!("{p}\t{entry}");
        let mut lines: Vec<&str> = text
            .lines()
            .filter(|l| parse_line(l).is_some_and(|(q, _)| q != p))
            .collect();
        lines.push(&line);
        let skip = lines.len().saturating_sub(MAX_REMEMBERED);
        let mut out = lines[skip..].join("\n");
        out.push('\n');
        Some(out)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
pub(crate) mod tests {
    use super::*;

    /// An encrypted archive is locked without its password, refuses a wrong or empty one,
    /// opens with the right one, and with the one remembered for it this session.
    #[test]
    fn encrypted_archives_unlock() {
        let data = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/specular-archive/tests/data"
        );
        let txt = |p: &Path| p.extension().is_some_and(|e| e == "txt");
        let open = |p: &Path, pw: Option<&str>| {
            let pw = pw.map(|s| Password::new(s.to_owned()));
            Book::unlock(p, txt, 1, |_| KnownComic::default(), pw)
        };
        for name in ["zipcrypto.zip", "aes256.zip", "enc.7z", "hdrenc.7z"] {
            let p = Path::new(data).join(name);
            assert!(matches!(open(&p, None), Err(BookError::Locked)), "{name}");
            for wrong in ["secret", ""] {
                let r = open(&p, Some(wrong));
                assert!(
                    matches!(r, Err(BookError::WrongPassword | BookError::Locked)),
                    "{name}"
                );
            }
            let b = open(&p, Some("Secret")).unwrap();
            assert!(b.encrypted(), "{name}");
            assert!(b.len() >= 1, "{name}");
            passwords::remember(&p, b.stamp().unwrap(), Password::new("Secret".into()));
            assert!(open(&p, None).unwrap().encrypted(), "{name}");
            passwords::forget(&p);
            assert!(matches!(open(&p, None), Err(BookError::Locked)), "{name}");
        }
    }

    /// A ZIP with stored entries (the common CBZ case), written by hand.
    pub(crate) fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data) in files {
            let crc = crc32(data);
            let off = out.len() as u32;
            let header = |v: &mut Vec<u8>| {
                // Version, flags, method (stored), time, date.
                v.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
                v.extend_from_slice(&crc.to_le_bytes());
                v.extend_from_slice(&(data.len() as u32).to_le_bytes());
                v.extend_from_slice(&(data.len() as u32).to_le_bytes());
                v.extend_from_slice(&(name.len() as u16).to_le_bytes());
                v.extend_from_slice(&0u16.to_le_bytes());
            };
            out.extend_from_slice(b"PK\x03\x04");
            header(&mut out);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);
            central.extend_from_slice(b"PK\x01\x02\x14\x00");
            header(&mut central);
            // Comment length, disk, internal and external attributes.
            central.extend_from_slice(&[0; 8]);
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&off.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_off = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    fn crc32(d: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in d {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    (c >> 1) ^ 0xEDB8_8320
                } else {
                    c >> 1
                };
            }
        }
        !c
    }

    fn is_image(p: &Path) -> bool {
        p.extension().is_some_and(|e| e == "png" || e == "jpg")
    }

    fn book(files: &[(&str, &[u8])]) -> Result<Book, BookError> {
        let src = Source::from_vec("/books/a.cbz", zip(files));
        Book::from_source(Arc::new(src), is_image)
    }

    #[test]
    fn archive_pages_browse_in_natural_order_without_clutter() {
        let b = book(&[
            ("p10.jpg", b"ten"),
            ("p2.jpg", b"two"),
            ("notes.txt", b"x"),
            ("__MACOSX/._p2.jpg", b"junk"),
            ("p1.png", b"one"),
        ])
        .unwrap();
        assert_eq!(b.len(), 3);
        let names: Vec<_> = (0..3).map(|i| b.entry_name(i).unwrap()).collect();
        assert_eq!(names, ["p1.png", "p2.jpg", "p10.jpg"]);
        assert_eq!(b.paths()[1], Path::new("/books/a.cbz/p2.jpg"));
        // Stored entries are views of the archive bytes, read through the virtual path.
        let s = b.source(1).unwrap();
        assert_eq!(s.bytes(), b"two");
        assert_eq!(s.path(), Path::new("/books/a.cbz/p2.jpg"));
        assert!(!b.right_to_left());
        assert!(!b.suggests_spreads());
        assert!(b.source(3).is_err());
    }

    #[test]
    fn hostile_entry_names_stay_inside_the_archive_path() {
        let b = book(&[
            ("../../etc/x.jpg", b"a"),
            ("sub/../../y.jpg", b"b"),
            ("x.jpg", b"c"),
        ])
        .unwrap();
        // `..` components count as hidden (junk) entries, so only the plain page remains.
        assert_eq!(b.len(), 1);
        assert_eq!(b.entry_name(0).unwrap(), "x.jpg");
        for p in b.paths() {
            assert!(p.starts_with("/books/a.cbz"), "{p:?}");
            assert!(!p.components().any(|c| c == std::path::Component::ParentDir));
        }
    }

    #[test]
    fn the_loader_table_resolves_virtual_paths() {
        let b = Arc::new(book(&[("1.jpg", b"a"), ("2.jpg", b"b")]).unwrap());
        let books = Books::default();
        books.insert(b.clone());
        let (got, page) = books.get(Path::new("/books/a.cbz/2.jpg")).unwrap();
        assert_eq!(page, 1);
        assert_eq!(got.source(page).unwrap().bytes(), b"b");
        assert!(books.get(Path::new("/books/a.cbz/3.jpg")).is_none());
        books.retain(Some(Path::new("/books/a.cbz")));
        assert!(books.get(Path::new("/books/a.cbz/1.jpg")).is_some());
        books.retain(None);
        assert!(books.get(Path::new("/books/a.cbz/1.jpg")).is_none());
    }

    #[test]
    fn comicinfo_sets_reading_order_and_hides_deleted_pages() {
        let xml = br#"<?xml version="1.0"?><ComicInfo><Manga>YesAndRightToLeft</Manga>
            <Pages><Page Image="1" Type="Deleted"/><Page Image="2" DoublePage="true"/></Pages></ComicInfo>"#;
        let b = book(&[
            ("ComicInfo.xml", xml),
            ("1.jpg", b"a"),
            ("2.jpg", b"b"),
            ("3.jpg", b"c"),
        ])
        .unwrap();
        assert!(b.right_to_left());
        assert!(b.suggests_spreads());
        assert_eq!(b.len(), 2);
        assert_eq!(b.entry_name(0), Some("1.jpg"));
        assert_eq!(b.entry_name(1), Some("3.jpg"));
    }

    #[test]
    fn comicinfo_that_deletes_every_page_is_known_but_ignored() {
        let xml = br#"<ComicInfo><Manga>YesAndRightToLeft</Manga><Pages>
            <Page Image="0" Type="Deleted"/><Page Image="1" Type="Deleted" DoublePage="true"/>
            <Page Image="1" Type="FrontCover"/></Pages></ComicInfo>"#;
        let b = book(&[("ComicInfo.xml", xml), ("1.jpg", b"a"), ("2.jpg", b"b")]).unwrap();
        assert!(b.comic_known().is_some_and(|c| c.is_some()));
        assert!(!b.comic_applies());
        assert!(!b.right_to_left() && !b.suggests_spreads() && !b.is_double_page(1));
        assert_eq!(b.len(), 2);
        assert_eq!(b.covers(4), b.paths());
        let none = book(&[("1.jpg", b"a")]).unwrap();
        let w = none.with_comic(ComicInfo::parse(xml).unwrap()).unwrap();
        assert!(!w.comic_applies() && w.comic_known().is_some_and(|c| c.is_some()));
        assert_eq!(w.paths(), none.paths());
    }

    #[test]
    fn comicinfo_read_later_makes_the_same_book_with_its_pages() {
        let xml = br#"<ComicInfo><Manga>YesAndRightToLeft</Manga>
            <Pages><Page Image="1" Type="Deleted"/></Pages></ComicInfo>"#;
        let files: &[(&str, &[u8])] = &[
            ("1.jpg", b"a"),
            ("2.jpg", b"b"),
            ("3.jpg", b"c"),
            ("ComicInfo.xml", xml),
        ];
        let src = Arc::new(Source::from_vec("/books/a.cbz", zip(files)));
        let first = Book::build(src, is_image, Options::default(), Comic::Later).unwrap();
        // Opened without it: every image, left to right.
        assert_eq!(first.len(), 3);
        assert!(!first.right_to_left());
        assert!(first.comic_known().is_none());
        // On a thread of its own, as the app reads it (the priority is lowered).
        let read = std::thread::scope(|s| s.spawn(|| first.read_comic_info()).join()).unwrap();
        let later = first.with_comic(read.found().unwrap()).unwrap();
        assert!(later.comic_known().is_some_and(|c| c.is_some()));
        assert!(first.same_open(&later));
        assert!(later.right_to_left());
        assert_eq!(later.len(), 2);
        assert_eq!(later.paths()[1], first.paths()[2]);
        assert_eq!(later.source(1).unwrap().bytes(), b"c");
        // No ComicInfo: nothing to tell.
        let none = book(&[("1.jpg", b"a")]).unwrap();
        assert_eq!(none.read_comic_info(), ComicLookup::Absent);
        assert!(!none.same_open(&book(&[("1.jpg", b"a")]).unwrap()));
    }

    #[test]
    fn no_pages_is_an_error_and_not_an_archive_fails() {
        assert!(matches!(book(&[("a.txt", b"x")]), Err(BookError::Empty)));
        let src = Source::from_vec("/x.cbz", b"not an archive at all".to_vec());
        assert!(matches!(
            Book::from_source(Arc::new(src), is_image),
            Err(BookError::Failed(_))
        ));
    }

    #[test]
    fn spreads_keep_the_cover_and_wide_pages_alone() {
        let s = Spreads::new(8, |i| i == 4);
        // 0 | 1 2 | 3 | 4 | 5 6 | 7
        assert_eq!(s.screen_of(0), 0..1);
        assert_eq!(s.screen_of(2), 1..3);
        assert_eq!(s.screen_of(3), 3..4);
        assert_eq!(s.screen_of(4), 4..5);
        assert_eq!(s.screen_of(6), 5..7);
        assert_eq!(s.screen_of(7), 7..8);
        assert_eq!(s.screens(), 6);
        assert_eq!(s.step(2, 1), (3, false));
        assert_eq!(s.step(1, -1), (0, false));
        assert_eq!(s.step(5, 10), (7, true));
        assert_eq!(s.step(0, -3), (0, true));
        assert_eq!(Spreads::new(0, |_| false).step(0, 1), (0, true));
    }

    #[test]
    fn reading_threads_leave_the_loader_most_cores() {
        assert_eq!(reading_threads(0), 1);
        assert_eq!(reading_threads(3), 1);
        assert_eq!(reading_threads(6), 3);
        assert_eq!(reading_threads(15), 4);
        assert_eq!(reading_threads(63), 4);
    }

    #[test]
    fn archive_extensions() {
        assert!(is_archive_path(Path::new("/a/B.CBZ")));
        assert!(is_archive_path(Path::new("x.cb7")));
        assert!(!is_archive_path(Path::new("x.jpg")));
        assert!(is_archive_path(Path::new("x.tgz")));
        assert!(is_archive_path(Path::new("/d/X.Tar.Gz")));
        assert!(is_archive_path(Path::new("disc.iso")));
        assert!(!is_archive_path(Path::new("x.svg.gz")));
        assert!(is_archive_path(Path::new("x.tar.bz2")));
        assert!(is_archive_path(Path::new("x.TXZ")));
        assert!(is_archive_path(Path::new("x.cbt.zst")));
        assert!(!is_archive_path(Path::new("x.svg.xz")));
    }

    #[test]
    fn book_positions_keep_the_latest_page_per_book() {
        let a = Path::new("/books/a.cbz");
        let b = Path::new("/books/b.cb7");
        let t = positions::with_entry("", a, "001.jpg").unwrap();
        let t = positions::with_entry(&t, b, "ch 2/010.png").unwrap();
        let t = positions::with_entry(&t, a, "005.jpg").unwrap();
        assert_eq!(positions::find(&t, a), Some("005.jpg"));
        assert_eq!(positions::find(&t, b), Some("ch 2/010.png"));
        assert_eq!(t.lines().count(), 2);
        assert!(t.ends_with("a.cbz\t005.jpg\n"));
        assert_eq!(positions::find(&t, Path::new("/books/c.cbr")), None);
        // Names that would break the table are not stored; junk lines are dropped.
        assert_eq!(positions::with_entry(&t, a, "x\ty"), None);
        let t = positions::with_entry("junk\n\n/x\t\n", a, "1.jpg").unwrap();
        assert_eq!(t, "/books/a.cbz\t1.jpg\n");
        // The oldest go first.
        let mut t = String::new();
        for i in 0..positions::MAX_REMEMBERED + 3 {
            t = positions::with_entry(&t, Path::new(&format!("/b/{i}.cbz")), "p").unwrap();
        }
        assert_eq!(t.lines().count(), positions::MAX_REMEMBERED);
        assert_eq!(positions::find(&t, Path::new("/b/0.cbz")), None);
        assert_eq!(positions::find(&t, Path::new("/b/3.cbz")), Some("p"));
    }
}
