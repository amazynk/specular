//! Archives as ordered image sets (DESIGN.md §6.4 reading mode, §13 security).
//!
//! An [`Archive`] is opened from a path (memory-mapped through [`specular_core::Source`]) or from
//! bytes already in memory. Opening parses only the directory: the ZIP central directory, the TAR
//! headers or the 7z header. Entries are sorted in the same natural, folder-by-folder order the
//! app uses for folders ("p2.jpg" < "p10.jpg", a folder's own files before its subfolders').
//!
//! | Format | Listing | Reading |
//! |---|---|---|
//! | ZIP / CBZ | own central-directory parser, ZIP64, self-extractor prefixes | stored (zero-copy), Deflate; own Deflate64, bzip2, LZMA, Zstandard, xz decoders |
//! | TAR / CBT | own header parser: ustar, GNU long names, pax `path`/`size` | zero-copy slices |
//! | `.tar.gz` / `.tgz` | the gzip stream inflated whole at open, then as TAR | slices of the inflated tar |
//! | `.tar.bz2` / `.tar.xz` / `.tar.zst` (`.tbz2`, `.txz`, `.tzst`) | unpacked whole at open by the own decoders, then as TAR | slices of the unpacked tar |
//! | ISO 9660 | own directory walker: Joliet, Rock Ridge names | zero-copy slices |
//! | 7z / CB7 | `sevenz-rust2` | LZMA, LZMA2, PPMd, Deflate, Copy + BCJ/Delta filters; solid blocks stream |
//! | RAR / CBR | UnRAR in the sandboxed decode host (`specular-sandbox`) | UnRAR in the host; [`Error::NeedsSandbox`] without one |
//!
//! Everything is read-only and in memory: entry names are never used as file-system paths and
//! nothing is ever written to disk. `&Archive` is `Sync`, so a prefetcher can read several entries
//! from different threads at once; ZIP and TAR reads share nothing but the mapped bytes.
//!
//! **Nested archives** (a `.zip` inside a `.cbz`) are listed like any other entry, flagged by
//! [`Entry::is_archive`], and never opened recursively. A caller that wants to descend reads the
//! entry's bytes and opens them with [`Archive::from_bytes`], which applies the same limits again.

mod codec;
mod comicinfo;
mod ctar;
mod error;
mod gz;
mod inflate;
mod iso;
mod lzma2mt;
mod names;
mod password;
mod rar;
mod sevenz;
mod tar;
mod zip;
mod zip_crypto;

use std::cmp::Ordering;
use std::path::Path;
use std::sync::Arc;

pub use comicinfo::{ComicInfo, ComicLookup, Manga, PageInfo, PageType};
pub use error::Error;
pub use names::natural_path_cmp;
pub use password::Password;
pub use rar::rar_version;

pub type Result<T> = std::result::Result<T, Error>;

/// A container format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// ZIP, including CBZ, EPUB-like ZIPs and self-extracting ZIPs.
    Zip,
    /// 7-Zip, including CB7.
    SevenZ,
    /// POSIX/GNU tar, including CBT.
    Tar,
    /// A gzip-compressed tar (`.tar.gz`, `.tgz`), inflated into memory when opened.
    TarGz,
    /// A bzip2-compressed tar (`.tar.bz2`, `.tbz2`, `.tbz`), unpacked into memory when opened.
    TarBz2,
    /// An xz-compressed tar (`.tar.xz`, `.txz`), unpacked into memory when opened.
    TarXz,
    /// A Zstandard-compressed tar (`.tar.zst`, `.tzst`), unpacked into memory when opened.
    TarZst,
    /// An ISO 9660 disc image (with Joliet or Rock Ridge names).
    Iso,
    /// RAR 1.5–4.x or RAR 5, including CBR. Listed and read by the sandboxed decode host.
    Rar,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::Zip => "ZIP",
            Format::SevenZ => "7z",
            Format::Tar => "TAR",
            Format::TarGz => "TAR.GZ",
            Format::TarBz2 => "TAR.BZ2",
            Format::TarXz => "TAR.XZ",
            Format::TarZst => "TAR.ZST",
            Format::Iso => "ISO 9660",
            Format::Rar => "RAR",
        }
    }

    /// The format a file extension (without the dot, any case) usually means.
    pub fn from_extension(ext: &str) -> Option<Format> {
        let e = ext.to_ascii_lowercase();
        Some(match e.as_str() {
            "zip" | "cbz" => Format::Zip,
            "7z" | "cb7" => Format::SevenZ,
            "tar" | "cbt" => Format::Tar,
            "tgz" => Format::TarGz,
            "tbz2" | "tbz" => Format::TarBz2,
            "txz" => Format::TarXz,
            "tzst" => Format::TarZst,
            "iso" => Format::Iso,
            "rar" | "cbr" => Format::Rar,
            _ => return None,
        })
    }

    /// The format a file name usually means: [`Format::from_extension`], and `.tar.gz`,
    /// `.tar.bz2`, `.tar.xz` and `.tar.zst` (and the same after `.cbt`; any case) as the
    /// compressed tar formats.
    pub fn from_file_name(name: &str) -> Option<Format> {
        let lower = name.to_ascii_lowercase();
        for (suffix, format) in [
            ("gz", Format::TarGz),
            ("bz2", Format::TarBz2),
            ("xz", Format::TarXz),
            ("zst", Format::TarZst),
        ] {
            let Some(stem) = lower.strip_suffix(suffix).and_then(|s| s.strip_suffix('.')) else {
                continue;
            };
            if stem.ends_with(".tar") || stem.ends_with(".cbt") {
                return Some(format);
            }
        }
        let ext = lower.rsplit_once('.')?.1;
        Format::from_extension(ext)
    }
}

impl Format {
    /// A tar inside a compression stream, unpacked into memory when opened.
    pub fn is_compressed_tar(self) -> bool {
        matches!(
            self,
            Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst
        )
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The format of `bytes`, from magic numbers only. ZIPs with a prefix (self-extractors) are
/// recognised by their end-of-central-directory record.
pub fn detect(bytes: &[u8]) -> Option<Format> {
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        Some(Format::Zip)
    } else if bytes.starts_with(sevenz::SIGNATURE) {
        Some(Format::SevenZ)
    } else if rar::detect(bytes).is_some() {
        Some(Format::Rar)
    } else if iso::detect(bytes) {
        Some(Format::Iso)
    } else if tar::detect(bytes) {
        Some(Format::Tar)
    } else if gz::detect(bytes) {
        Some(Format::TarGz)
    } else if let Some(f) = ctar::detect(bytes) {
        Some(f)
    } else if zip::find_eocd(bytes).is_some() {
        Some(Format::Zip)
    } else {
        None
    }
}

/// How an entry is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    /// No compression (ZIP stored, TAR, 7z Copy). ZIP and TAR entries can be borrowed with
    /// [`Archive::stored_slice`].
    Stored,
    Deflate,
    /// Deflate with a 64 KiB window (ZIP method 9).
    Deflate64,
    /// ZIP method 12.
    Bzip2,
    Lzma,
    Lzma2,
    Ppmd,
    /// Zstandard (ZIP method 93).
    Zstd,
    /// xz (ZIP method 95).
    Xz,
    /// Anything else, named (e.g. "ZIP method 98", "PPMd", "RAR"). Listing works; reading
    /// returns [`Error::Unsupported`].
    Other(String),
}

/// Guards against hostile archives (DESIGN.md §13). Exceeding one returns [`Error::Limit`].
#[derive(Debug, Clone)]
pub struct Limits {
    /// Most entries (files and folders) a directory may declare.
    pub max_entries: usize,
    /// Largest uncompressed entry that is read.
    pub max_entry_size: u64,
    /// Largest sum of declared uncompressed sizes when opening.
    pub max_total_size: u64,
    /// Highest uncompressed:compressed ratio for entries (or 7z blocks) larger than
    /// `ratio_threshold`. Real pages are JPEG/PNG/WebP (about 1:1); blank BMP/TIFF scans can reach
    /// 100:1; deflate itself tops out near 1032:1.
    pub max_ratio: u64,
    /// Entries up to this size are not ratio-checked.
    pub ratio_threshold: u64,
    /// Largest LZMA/LZMA2 dictionary or PPMd model a 7z block may ask for.
    pub max_decoder_memory: u64,
    /// Largest `ComicInfo.xml` that is parsed.
    pub max_comic_info: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_entries: 100_000,
            max_entry_size: 2 << 30,
            max_total_size: 64 << 30,
            max_ratio: 200,
            ratio_threshold: 16 << 20,
            max_decoder_memory: 1 << 30,
            max_comic_info: 1 << 20,
        }
    }
}

impl Limits {
    pub(crate) fn check_entry(&self, size: u64, compressed: u64) -> Result<()> {
        if size > self.max_entry_size {
            return Err(Error::Limit(format!(
                "entry of {size} bytes exceeds the {} byte limit",
                self.max_entry_size
            )));
        }
        self.check_ratio(size, compressed)
    }

    pub(crate) fn check_ratio(&self, size: u64, compressed: u64) -> Result<()> {
        if size > self.ratio_threshold && size / compressed.max(1) > self.max_ratio {
            return Err(Error::Limit(format!(
                "compression ratio {}:1 exceeds {}:1",
                size / compressed.max(1),
                self.max_ratio
            )));
        }
        Ok(())
    }
}

/// Opening options.
#[derive(Debug, Clone)]
pub struct Options {
    pub limits: Limits,
    /// Check CRC-32 of ZIP and 7z entries after reading (7z blocks the `sevenz` module hands
    /// to `sevenz-rust2` are always checked by it).
    pub verify_crc: bool,
    /// Bytes of decoded entries a solid 7z block keeps: the ones passed over on the way to a
    /// requested entry, the ones read ahead, and recently read ones.
    pub solid_cache_bytes: usize,
    /// Bytes a solid 7z block's streamer decodes past the furthest entry asked for, so the
    /// next ones are ready when asked for (at most half of `solid_cache_bytes`). Its LZMA2
    /// helpers go further when the readers have come further since the stream started.
    pub solid_read_ahead: usize,
    /// Most threads a solid 7z block's LZMA2 stream is decoded with, when a multi-threaded
    /// encoder cut it into independent units (see the `sevenz` module). 1: one thread.
    pub decode_threads: u32,
    /// Run first on every thread the archive starts (a solid 7z block's streamer and its
    /// LZMA2 helpers), e.g. to lower their priority.
    pub thread_start: Option<fn()>,
    /// The password for encrypted entries and encrypted 7z/RAR headers. Without one, reading
    /// an encrypted entry fails with [`Error::Encrypted`] (and opening an archive whose file
    /// list is encrypted too); with a wrong one, with [`Error::WrongPassword`].
    pub password: Option<Password>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            limits: Limits::default(),
            verify_crc: true,
            solid_cache_bytes: 256 << 20,
            solid_read_ahead: 16 << 20,
            decode_threads: 1,
            thread_start: None,
            password: None,
        }
    }
}

/// Random-access bytes an archive is read from. Must stay unchanged while the archive is open.
pub trait ByteSource: Send + Sync + 'static {
    fn bytes(&self) -> &[u8];

    /// The file the bytes were read from, if any. RAR uses it to hand the decode host the file
    /// itself (and to find further volumes next to it) instead of a copy; it checks first
    /// that the file still holds these bytes.
    fn path(&self) -> Option<&std::path::Path> {
        None
    }
}

impl ByteSource for Vec<u8> {
    fn bytes(&self) -> &[u8] {
        self
    }
}

impl ByteSource for Box<[u8]> {
    fn bytes(&self) -> &[u8] {
        self
    }
}

impl ByteSource for &'static [u8] {
    fn bytes(&self) -> &[u8] {
        self
    }
}

impl ByteSource for specular_core::Source {
    fn bytes(&self) -> &[u8] {
        specular_core::Source::bytes(self)
    }

    fn path(&self) -> Option<&std::path::Path> {
        Some(specular_core::Source::path(self))
    }
}

impl<T: ByteSource + ?Sized> ByteSource for Arc<T> {
    fn bytes(&self) -> &[u8] {
        (**self).bytes()
    }

    fn path(&self) -> Option<&std::path::Path> {
        (**self).path()
    }
}

/// One file in an archive. Folders are not entries.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Path inside the archive: `/`-separated, without a leading `/` or `./`. It is a display
    /// and sort key only and may contain `..`: never join it onto a real path.
    pub name: String,
    /// Uncompressed size as declared by the archive (checked when read).
    pub size: u64,
    /// Bytes the entry occupies in the archive (0 when unknown, e.g. inside a solid 7z block).
    pub compressed_size: u64,
    pub method: Method,
    pub encrypted: bool,
    pub(crate) loc: Loc,
}

#[derive(Debug, Clone)]
pub(crate) enum Loc {
    Zip(zip::ZipLoc),
    /// Contiguous stored bytes at `offset` (TAR, ISO 9660).
    Plain {
        offset: u64,
    },
    SevenZ {
        file: usize,
    },
    Rar {
        index: u32,
    },
}

impl Entry {
    /// The final path component.
    pub fn file_name(&self) -> &str {
        self.name.rsplit('/').next().unwrap_or(&self.name)
    }

    /// Lower-case extension of the file name, without the dot ("" if none).
    pub fn extension(&self) -> String {
        let f = self.file_name();
        match f.rfind('.') {
            Some(i) if i > 0 => f[i + 1..].to_ascii_lowercase(),
            _ => String::new(),
        }
    }

    /// A nested archive (by extension). Listed, never opened recursively.
    pub fn is_archive(&self) -> bool {
        Format::from_file_name(self.file_name()).is_some()
    }

    /// Operating-system clutter that is never a page: anything under `__MACOSX/` (AppleDouble
    /// `._x.jpg` files carry image extensions), hidden files and folders, `Thumbs.db`,
    /// `desktop.ini`.
    pub fn is_junk(&self) -> bool {
        let f = self.file_name();
        self.name
            .split('/')
            .any(|c| c.starts_with('.') || c.eq_ignore_ascii_case("__MACOSX"))
            || f.eq_ignore_ascii_case("thumbs.db")
            || f.eq_ignore_ascii_case("desktop.ini")
    }
}

enum Backend {
    Zip(zip::ZipInfo),
    Plain,
    SevenZ(Box<sevenz::SevenZ>),
    Rar(Box<rar::Rar>),
}

/// An open archive: a sorted list of entries plus random-access reads.
pub struct Archive {
    format: Format,
    data: Arc<dyn ByteSource>,
    entries: Vec<Entry>,
    backend: Backend,
    options: Options,
    /// The memory budget held for an unpacked compressed tar while the archive holds it.
    _held: Option<specular_core::budget::Reservation>,
}

/// Reserves `bytes` of the memory budget ([`specular_core::budget`]) for a buffer about to be
/// allocated, or refuses with how much free RAM it needs.
pub(crate) fn reserve(bytes: u64) -> Result<specular_core::budget::Reservation> {
    specular_core::budget::reserve(bytes).map_err(|e| Error::Limit(e.to_string()))
}

impl std::fmt::Debug for Archive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Archive")
            .field("format", &self.format)
            .field("len", &self.data.bytes().len())
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl Archive {
    /// Opens (memory-maps) the file at `path` with default options.
    pub fn open(path: impl AsRef<Path>) -> Result<Archive> {
        Archive::open_with(path, Options::default())
    }

    pub fn open_with(path: impl AsRef<Path>, options: Options) -> Result<Archive> {
        let source = specular_core::Source::open(path)?;
        Archive::from_bytes(source, options)
    }

    /// Opens an archive held in memory (or in any [`ByteSource`], e.g. an
    /// `Arc<specular_core::Source>` the caller already has).
    pub fn from_bytes(data: impl ByteSource, options: Options) -> Result<Archive> {
        let mut data: Arc<dyn ByteSource> = Arc::new(data);
        let format = detect(data.bytes()).ok_or(Error::NotAnArchive)?;
        let limits = &options.limits;
        let mut held = None;
        if format.is_compressed_tar() {
            let (tar, r) = if format == Format::TarGz {
                gz::inflate(data.bytes(), limits)?
            } else {
                ctar::unpack(format, data.bytes(), &options)?
            };
            if !tar::detect(&tar) {
                return Err(Error::NotAnArchive);
            }
            data = Arc::new(tar);
            held = Some(r);
        }
        let bytes = data.bytes();
        let (mut entries, backend) = match format {
            Format::Zip => {
                let (entries, info) = zip::list(bytes, limits)?;
                (entries, Backend::Zip(info))
            }
            Format::Tar | Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst => {
                (tar::list(bytes, limits)?, Backend::Plain)
            }
            Format::Iso => (iso::list(bytes, limits)?, Backend::Plain),
            Format::SevenZ => {
                let (entries, sz) = sevenz::open(&data, &options)?;
                (entries, Backend::SevenZ(Box::new(sz)))
            }
            Format::Rar => {
                let (entries, r) =
                    rar::open(bytes, data.path(), limits, options.password.as_ref())?;
                (entries, Backend::Rar(Box::new(r)))
            }
        };
        let total = entries
            .iter()
            .try_fold(0u64, |acc, e| acc.checked_add(e.size));
        if total.is_none_or(|t| t > limits.max_total_size) {
            return Err(Error::Limit(format!(
                "declared total size exceeds {} bytes",
                limits.max_total_size
            )));
        }
        let i18n = specular_i18n::I18n::global();
        entries.sort_by(|a, b| natural_path_cmp(|x, y| i18n.natural_cmp(x, y), &a.name, &b.name));
        let a = Archive {
            format,
            data,
            entries,
            backend,
            options,
            _held: held,
        };
        // ComicInfo in a solid 7z block is kept whenever a read decodes past it.
        if let (Some(Loc::SevenZ { file }), Backend::SevenZ(sz)) =
            (a.comic_info_entry().map(|e| &e.loc), &a.backend)
        {
            sz.capture(*file);
        }
        Ok(a)
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// Whether any entry is encrypted (a 7z or RAR archive whose file list is encrypted too
    /// does not open without its password: [`Error::Encrypted`]).
    pub fn is_encrypted(&self) -> bool {
        self.entries.iter().any(|e| e.encrypted)
    }

    /// Checks the password the archive was opened with by reading the encrypted entry that
    /// is cheapest to reach. `Ok` when nothing is encrypted; [`Error::WrongPassword`] when the
    /// password does not decrypt it, [`Error::Encrypted`] without a password.
    pub fn check_password(&self) -> Result<()> {
        // The smallest entry read on its own or first in its stream: reaching any other
        // decodes what lies before it.
        let first = |i: usize| self.stream_position(i).is_none_or(|(_, place)| place == 0);
        let cost = |i: usize| (!first(i), self.entries[i].size);
        let Some(i) = (0..self.entries.len())
            .filter(|&i| self.entries[i].encrypted)
            .min_by_key(|&i| cost(i))
        else {
            return Ok(());
        };
        match self.read(i) {
            Err(e @ (Error::WrongPassword | Error::Encrypted | Error::Io(_))) => Err(e),
            // Damage or limits are the entry's own trouble, not the password's.
            _ => Ok(()),
        }
    }

    /// All file entries, in natural path order (or the order set by [`Archive::sort_by`]).
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Re-sorts the entries (e.g. descending, or by size). Indices change accordingly.
    pub fn sort_by(&mut self, cmp: impl FnMut(&Entry, &Entry) -> Ordering) {
        self.entries.sort_by(cmp);
    }

    /// Index of the entry named exactly `name`, else the first matching it ignoring ASCII case.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.name == name)
            .or_else(|| {
                self.entries
                    .iter()
                    .position(|e| e.name.eq_ignore_ascii_case(name))
            })
    }

    /// Indices of the pages: entries `is_image` accepts (typically by extension through the
    /// decoder registry), minus [`Entry::is_junk`] clutter, in entry order.
    pub fn pages(&self, mut is_image: impl FnMut(&Entry) -> bool) -> Vec<usize> {
        (0..self.entries.len())
            .filter(|&i| {
                let e = &self.entries[i];
                !e.is_junk() && is_image(e)
            })
            .collect()
    }

    /// Indices of nested archives (listed only; see the crate docs).
    pub fn nested_archives(&self) -> Vec<usize> {
        (0..self.entries.len())
            .filter(|&i| self.entries[i].is_archive() && !self.entries[i].is_junk())
            .collect()
    }

    /// The entry's bytes, borrowed straight from the archive when it is stored uncompressed in a
    /// ZIP or TAR (the common CBZ case: JPEG pages are rarely deflated). `None` otherwise; then
    /// use [`Archive::read`]. CRCs are not checked on this path.
    pub fn stored_slice(&self, index: usize) -> Option<&[u8]> {
        let e = self.entries.get(index)?;
        let bytes = self.data.bytes();
        match (&e.loc, &self.backend) {
            (Loc::Zip(loc), Backend::Zip(info)) if e.method == Method::Stored && !e.encrypted => {
                let (start, len) = zip::data_range(bytes, info, loc, e).ok()?;
                // A stored entry whose compressed and declared sizes differ is malformed, as
                // in `read` (else this would hand out bytes of the wrong length, unchecked).
                if u64::try_from(len).ok()? != e.size {
                    return None;
                }
                bytes.get(start..start + len)
            }
            (Loc::Plain { offset }, _) => {
                let start = usize::try_from(*offset).ok()?;
                bytes.get(start..start.checked_add(usize::try_from(e.size).ok()?)?)
            }
            _ => None,
        }
    }

    /// Reads the whole entry into a new vector.
    pub fn read(&self, index: usize) -> Result<Vec<u8>> {
        self.read_reserved(index).map(|(v, _)| v)
    }

    /// Reads the whole entry into a new vector, with the memory budget's share of it: a
    /// caller keeping the bytes keeps the reservation with them.
    pub fn read_reserved(
        &self,
        index: usize,
    ) -> Result<(Vec<u8>, specular_core::budget::Reservation)> {
        let mut out = Vec::new();
        let r = self.read_into_reserving(index, &mut out)?;
        Ok((out, r))
    }

    /// Reads the whole entry into `out` (cleared first; its capacity is reused). Safe to call
    /// from several threads at once. Size and ratio limits are checked before anything is
    /// allocated, and the output never grows past the declared size.
    pub fn read_into(&self, index: usize, out: &mut Vec<u8>) -> Result<()> {
        self.read_into_reserving(index, out).map(drop)
    }

    /// [`Archive::read_into`], returning the reservation of the entry's size.
    fn read_into_reserving(
        &self,
        index: usize,
        out: &mut Vec<u8>,
    ) -> Result<specular_core::budget::Reservation> {
        out.clear();
        let e = self.entries.get(index).ok_or(Error::NoEntry(index))?;
        let password = self.options.password.as_ref();
        if e.encrypted && password.is_none() {
            return Err(Error::Encrypted);
        }
        let limits = &self.options.limits;
        let bytes = self.data.bytes();
        // The entry's declared size, from the memory budget, before anything is allocated
        // (size and ratio limits are checked first by each reader). Handed to the caller with
        // the bytes by `read_reserved`.
        limits.check_entry(e.size, e.size.max(e.compressed_size).max(1))?;
        let budget = reserve(e.size)?;
        let read = match (&e.loc, &self.backend) {
            (Loc::Zip(loc), Backend::Zip(info)) => zip::read(
                bytes,
                info,
                loc,
                e,
                limits,
                self.options.verify_crc,
                password,
                out,
            ),
            (Loc::Plain { .. }, _) => {
                limits.check_entry(e.size, e.size)?;
                let s = self
                    .stored_slice(index)
                    .ok_or(Error::Malformed("TAR entry outside the archive".into()))?;
                out.extend_from_slice(s);
                Ok(())
            }
            (Loc::SevenZ { file }, Backend::SevenZ(sz)) => {
                sz.read(&self.data, *file, e, &self.options, out)
            }
            (Loc::Rar { index }, Backend::Rar(r)) => r.read(*index, e, limits, out),
            _ => Err(Error::Malformed(
                "entry does not belong to this archive".into(),
            )),
        };
        read.map(|()| budget)
    }

    /// Where entry `index` lies in a stream that is decoded from its start to reach it (a
    /// solid 7z block, a solid RAR archive): the stream's number and the entry's place in it.
    /// `None` when the entry is read on its own. Reading entries of one stream in ascending
    /// place, as [`Archive::read_many`] does, decodes the stream once.
    pub fn stream_position(&self, index: usize) -> Option<(u64, u64)> {
        let e = self.entries.get(index)?;
        match (&e.loc, &self.backend) {
            (Loc::SevenZ { file }, Backend::SevenZ(sz)) => sz.stream_position(*file),
            (Loc::Rar { index }, Backend::Rar(r)) if r.solid => Some((0, u64::from(*index))),
            _ => None,
        }
    }

    /// Reads the entries `indices`, handing each to `each` (its position in `indices` and its
    /// bytes) as soon as it is read; stops when `each` returns false. Entries are read in
    /// stream order ([`Archive::stream_position`]; others in the order given), so a solid
    /// stream is decoded once for all of them: a solid RAR archive in one host request per
    /// 64 MiB of entries instead of one decompression from the start per entry, a solid 7z
    /// block by its streamer without restarts (its LZMA2 helpers decoding past the last entry
    /// by three times as much as lies before it, for the next batches). Every entry is checked as by
    /// [`Archive::read`].
    pub fn read_many(
        &self,
        indices: &[usize],
        each: &mut dyn FnMut(usize, Result<Vec<u8>>) -> bool,
    ) {
        self.read_many_reserved(indices, &mut |k, r| each(k, r.map(|(v, _)| v)));
    }

    /// [`Archive::read_many`], each entry's bytes with the memory budget's share of them (as
    /// [`Archive::read_reserved`]).
    pub fn read_many_reserved(
        &self,
        indices: &[usize],
        each: &mut dyn FnMut(usize, Result<(Vec<u8>, specular_core::budget::Reservation)>) -> bool,
    ) {
        let mut order: Vec<usize> = (0..indices.len()).collect();
        order.sort_by_key(|&k| {
            self.stream_position(indices[k])
                .unwrap_or((u64::MAX, k as u64))
        });
        if let Backend::Rar(r) = &self.backend {
            // RAR entries go in one batch (encrypted ones without a password are refused by
            // their own read).
            let unlocked = self.options.password.is_some();
            let mut items = Vec::with_capacity(order.len());
            let mut pos = Vec::with_capacity(order.len());
            for &k in &order {
                match self.entries.get(indices[k]) {
                    Some(
                        e @ Entry {
                            loc: Loc::Rar { index },
                            encrypted,
                            ..
                        },
                    ) if !encrypted || unlocked => {
                        items.push((*index, e));
                        pos.push(k);
                    }
                    _ => {
                        if !each(k, self.read_reserved(indices[k])) {
                            return;
                        }
                    }
                }
            }
            // The host reads in listing order.
            let mut by_index: Vec<usize> = (0..items.len()).collect();
            by_index.sort_by_key(|&j| items[j].0);
            let items: Vec<(u32, &Entry)> = by_index.iter().map(|&j| items[j]).collect();
            let pos: Vec<usize> = by_index.iter().map(|&j| pos[j]).collect();
            // The host's bytes are reserved as they arrive.
            r.read_many(&items, &self.options.limits, &mut |j, res| {
                let res = res.and_then(|v| reserve(v.len() as u64).map(|r| (v, r)));
                each(pos[j], res)
            });
            return;
        }
        // A solid 7z block's LZMA2 helpers may decode as far as the last of them.
        let files: Vec<usize> = match &self.backend {
            Backend::SevenZ(_) => (indices.iter())
                .filter_map(|&i| match self.entries.get(i)?.loc {
                    Loc::SevenZ { file } => Some(file),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        if let Backend::SevenZ(sz) = &self.backend {
            sz.will_read(&files, true);
        }
        for k in order {
            if !each(k, self.read_reserved(indices[k])) {
                break;
            }
        }
        if let Backend::SevenZ(sz) = &self.backend {
            sz.will_read(&files, false);
        }
    }

    /// `ComicInfo.xml` (ComicRack metadata): manga right-to-left flag, page types, double-page
    /// spreads. Looked up at the archive root first, then anywhere. `None` if absent or not
    /// parseable; errors reading it are treated as absent.
    ///
    /// `PageInfo::image` indexes the archive's image pages in sorted order, i.e. the result of
    /// [`Archive::pages`].
    pub fn comic_info(&self) -> Option<ComicInfo> {
        self.comic_info_lookup(None).found()
    }

    /// [`Archive::comic_info`] when reaching it costs little: when it is read on its own, or
    /// when it lies in a solid stream ([`Archive::stream_position`]) behind at most `budget`
    /// bytes of other entries (in a 7z LZMA2 block, counted from the start of its unit, where
    /// decoding may begin). `None` (as if absent) otherwise: in a solid stream it often comes
    /// last (7-Zip groups files by extension), and reaching it could decode every page.
    pub fn comic_info_within(&self, budget: u64) -> Option<ComicInfo> {
        self.comic_info_lookup(Some(budget)).found()
    }

    /// [`Archive::comic_info`] (or [`Archive::comic_info_within`] `budget`), telling an archive
    /// without a usable `ComicInfo.xml` ([`ComicLookup::Absent`], which holds as long as the
    /// archive is unchanged) from one whose `ComicInfo.xml` was not read: beyond the budget,
    /// or failing to read (a damaged stream, a decode host gone), [`ComicLookup::Unread`].
    pub fn comic_info_lookup(&self, budget: Option<u64>) -> ComicLookup {
        let Some(idx) = self.comic_info_index() else {
            return ComicLookup::Absent;
        };
        let e = &self.entries[idx];
        if e.size > self.options.limits.max_comic_info {
            return ComicLookup::Absent;
        }
        if let Some(b) = self.comic_info_kept() {
            return comic_lookup(&b);
        }
        if let Some(budget) = budget
            && !self.within(idx, budget)
        {
            return ComicLookup::Unread;
        }
        match self.read(idx) {
            Ok(data) => comic_lookup(&data),
            Err(_) => ComicLookup::Unread,
        }
    }

    /// [`Archive::comic_info_lookup`] without a budget, for a reader that must not move
    /// the other readers' stream: `ComicInfo.xml` in a solid 7z block is decoded on the
    /// calling thread from the start of its LZMA2 unit (or of the block) without keeping the
    /// entries before it, unless the block's streamer or its helpers decode it in passing
    /// first (then this returns as soon as they have; it waits for a helper already decoding
    /// its unit). Blocking: meant for a thread of its own.
    pub fn comic_info_aside(&self) -> ComicLookup {
        let Some(e) = self.comic_info_entry() else {
            return self.comic_info_lookup(None);
        };
        if let (Loc::SevenZ { file }, Backend::SevenZ(sz)) = (&e.loc, &self.backend)
            && let Some(r) = sz.read_aside(&self.data, *file, &self.options)
        {
            return match r {
                Ok(data) => comic_lookup(&data),
                Err(_) => ComicLookup::Unread,
            };
        }
        self.comic_info_lookup(None)
    }

    /// Runs `f` with the `ComicInfo.xml` of a solid 7z block (parsed: found, or
    /// [`ComicLookup::Absent`] when it does not parse) once any read decodes past it: the
    /// block's streamer reading on, one of its LZMA2 helpers decoding ahead, or
    /// [`Archive::comic_info_aside`]. On the thread that decoded it, or now when one has.
    /// False (and `f` is dropped) when ComicInfo is not in a solid 7z block, or is too large.
    pub fn on_comic_info(&self, f: impl FnOnce(ComicLookup) + Send + 'static) -> bool {
        match (self.comic_info_entry().map(|e| &e.loc), &self.backend) {
            (Some(Loc::SevenZ { file }), Backend::SevenZ(sz)) => {
                sz.on_kept(*file, Box::new(move |b| f(comic_lookup(&b))))
            }
            _ => false,
        }
    }

    /// The bytes of ComicInfo once a read of a solid 7z block has passed over it.
    fn comic_info_kept(&self) -> Option<Arc<[u8]>> {
        match (self.comic_info_entry().map(|e| &e.loc), &self.backend) {
            (Some(Loc::SevenZ { file }), Backend::SevenZ(sz)) => sz.kept(*file),
            _ => None,
        }
    }

    /// The `ComicInfo.xml` entry, when it is small enough to be parsed.
    fn comic_info_entry(&self) -> Option<&Entry> {
        let e = &self.entries[self.comic_info_index()?];
        (e.size <= self.options.limits.max_comic_info).then_some(e)
    }

    /// Whether entry `idx` lies behind at most `budget` bytes of a solid stream (or is read
    /// on its own).
    fn within(&self, idx: usize, budget: u64) -> bool {
        if let (Loc::SevenZ { file }, Backend::SevenZ(sz)) = (&self.entries[idx].loc, &self.backend)
        {
            // From the start of its LZMA2 unit, where a block allows it.
            return sz.reach_cost(*file).is_none_or(|c| c <= budget);
        }
        if let Some((stream, at)) = self.stream_position(idx) {
            let before: u64 = (0..self.entries.len())
                .filter(|&i| {
                    self.stream_position(i)
                        .is_some_and(|(s, p)| s == stream && p < at)
                })
                .map(|i| self.entries[i].size)
                .sum();
            return before <= budget;
        }
        true
    }

    fn comic_info_index(&self) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.name.eq_ignore_ascii_case("ComicInfo.xml"))
            .or_else(|| {
                self.entries.iter().position(|e| {
                    !e.is_junk() && e.file_name().eq_ignore_ascii_case("ComicInfo.xml")
                })
            })
    }

    /// Whether reading a solid stream goes through state the archive keeps (a solid 7z
    /// block's streamer, behind a lock held while it decodes): readers in different places
    /// of one stream then contend and restart it. ZIP, TAR and RAR reads share nothing but
    /// the bytes (RAR's host requests are self-contained).
    pub fn has_stream_state(&self) -> bool {
        match &self.backend {
            Backend::SevenZ(sz) => sz.has_solid_blocks(),
            _ => false,
        }
    }

    pub fn options(&self) -> &Options {
        &self.options
    }
}

/// `data` parsed as a `ComicInfo.xml`.
fn comic_lookup(data: &[u8]) -> ComicLookup {
    ComicInfo::parse(data).map_or(ComicLookup::Absent, ComicLookup::Found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn archive_is_send_sync() {
        assert_send_sync::<Archive>();
    }

    #[test]
    fn detects_magic() {
        assert_eq!(detect(b"PK\x03\x04rest"), Some(Format::Zip));
        assert_eq!(detect(b"7z\xBC\xAF\x27\x1C\0\x04"), Some(Format::SevenZ));
        assert_eq!(detect(b"Rar!\x1a\x07\x00"), Some(Format::Rar));
        assert_eq!(detect(b"Rar!\x1a\x07\x01\x00"), Some(Format::Rar));
        assert_eq!(detect(b"hello"), None);
        assert_eq!(detect(b""), None);
    }

    #[test]
    fn junk_and_nested() {
        let e = |n: &str| Entry {
            name: n.into(),
            size: 0,
            compressed_size: 0,
            method: Method::Stored,
            encrypted: false,
            loc: Loc::Plain { offset: 0 },
        };
        assert!(e("__MACOSX/ch1/._p1.jpg").is_junk());
        assert!(e("ch1/.hidden.jpg").is_junk());
        assert!(e("Thumbs.db").is_junk());
        assert!(!e("ch1/p1.jpg").is_junk());
        assert!(e("extra/bonus.CBZ").is_archive());
        assert_eq!(e("a/b.JPG").extension(), "jpg");
        assert_eq!(e(".jpg").extension(), "");
        assert_eq!(e("a/b").file_name(), "b");
    }
}
