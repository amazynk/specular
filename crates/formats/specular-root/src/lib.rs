//! CERN ROOT files: our own reader for what an image viewer shows of them.
//!
//! * The file header, small (32-bit seeks) and large (fVersion > 1000000, 64-bit seeks).
//! * Keys (TKey) of the top directory and every subdirectory (TDirectory, TDirectoryFile),
//!   with cycles; keys of either seek width.
//! * Compression blocks: ZL (zlib), L4 (LZ4 with its xxHash64 checked), ZS (Zstandard),
//!   XZ (LZMA) and CS (ROOT's old algorithm, as raw DEFLATE).
//! * The streamer info list, read only as far as each class's name and version.
//! * Objects: TH1, TH2 and TH3 in C, S, I, L, F and D; TProfile and TProfile2D (mean per bin);
//!   TAxis with fixed or variable bins and labels; TGraph (and the points of TGraphErrors and
//!   TGraphAsymmErrors) and TGraph2D; the entry count of a TTree or TNtuple. Every class
//!   version is checked against the layouts known here and others are refused by name.
//!
//! All integers are big-endian. Everything else in a file (tree branches, RNTuples, user
//! classes) is listed by its key and not read.

mod buf;
mod compress;
mod object;

use std::borrow::Cow;
use std::collections::HashSet;

pub use buf::Storage;
pub use compress::algorithm_name;
pub use object::{
    Axis, Graph, Hist, Object, Tree, UNSET, hist_kind, is_graph_class, is_hist_class,
    is_tree_class, split_titles, split_unit,
};

use buf::{Any, Cur};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a ROOT file")]
    NotRoot,
    #[error("truncated: {0}")]
    Truncated(String),
    #[error("corrupt: {0}")]
    Corrupt(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("{class} version {version} is not supported (known: {known})")]
    Version {
        class: String,
        version: i16,
        known: String,
    },
    #[error("compression: {0}")]
    Compression(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The magic at offset 0.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 8 && head.starts_with(b"root")
}

/// Bounds on what a file may make us do.
const MAX_KEYS: usize = 1 << 20;
const MAX_DEPTH: usize = 64;
/// Most bytes of directory paths copied into keys (each key holds its directory's path: a
/// long directory name over a million keys would otherwise take terabytes).
const MAX_DIR_BYTES: usize = 64 << 20;
/// The largest object decompressed in one piece (histograms of 100 million cells).
pub const MAX_OBJECT: usize = 1 << 30;

/// One key: an object's name, class and where its bytes are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    /// The directory path ("" at the top, `a/b` below).
    pub dir: String,
    pub class: String,
    pub name: String,
    pub title: String,
    pub cycle: i16,
    /// The key header and object as stored.
    pub nbytes: u32,
    /// The object uncompressed.
    pub objlen: u32,
    pub keylen: u16,
    pub seek: u64,
    /// Packed TDatime of the write.
    pub datime: u32,
}

impl Key {
    /// `dir/name` (or `name` at the top).
    pub fn path(&self) -> String {
        if self.dir.is_empty() {
            self.name.clone()
        } else {
            format!("{}/{}", self.dir, self.name)
        }
    }

    pub fn is_directory(&self) -> bool {
        matches!(self.class.as_str(), "TDirectory" | "TDirectoryFile")
    }

    /// Stored compressed (the object is longer than its bytes on disk).
    pub fn compressed(&self) -> bool {
        u64::from(self.objlen) > u64::from(self.nbytes).saturating_sub(u64::from(self.keylen))
    }
}

/// A packed TDatime as `YYYY-MM-DD hh:mm:ss`.
pub fn datime_string(d: u32) -> Option<String> {
    if d == 0 {
        return None;
    }
    let year = (d >> 26) + 1995;
    let month = (d >> 22) & 0xf;
    let day = (d >> 17) & 0x1f;
    let hour = (d >> 12) & 0x1f;
    let min = (d >> 6) & 0x3f;
    let sec = d & 0x3f;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || min > 59 {
        return None;
    }
    Some(format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02}"
    ))
}

/// An opened file: the header, every key, and the streamer list's classes.
#[derive(Clone, Debug)]
pub struct RootFile {
    /// fVersion without the large-file flag (62400 is ROOT 6.24/00).
    pub version: u32,
    /// Written as a large file: 64-bit seeks throughout.
    pub large: bool,
    pub begin: u64,
    pub end: u64,
    /// fCompress: algorithm × 100 + level.
    pub compress: i32,
    pub seek_info: u64,
    pub nbytes_info: u32,
    /// The file's name and title as written.
    pub name: String,
    pub title: String,
    /// Where the top directory's key list is.
    pub seek_keys: u64,
    /// The top directory's creation and modification times (packed TDatime).
    pub created: u32,
    pub modified: u32,
    /// Every key of every directory, depth first in key-list order (directories included).
    pub keys: Vec<Key>,
    /// (class, version) of each streamer info, when the list could be read.
    pub streamers: Vec<(String, i32)>,
    /// What could not be read (subdirectories, the streamer list), in words.
    pub warnings: Vec<String>,
    /// Bytes of directory paths held by the keys so far ([`MAX_DIR_BYTES`]).
    dir_bytes: usize,
}

impl RootFile {
    pub fn open(data: &[u8]) -> Result<RootFile> {
        if !sniff(data) {
            return Err(Error::NotRoot);
        }
        let mut c = Cur::at(data, 4);
        let raw = c.i32()?;
        let raw = u32::try_from(raw).map_err(|_| Error::Corrupt(format!("file version {raw}")))?;
        let large = raw >= 1_000_000;
        let version = raw % 1_000_000;
        let begin = c.seek(false)?;
        let end = c.seek(large)?;
        let _seek_free = c.seek(large)?;
        let _nbytes_free = c.i32()?;
        let _nfree = c.i32()?;
        let nbytes_name = c.i32()?;
        let _units = c.u8()?;
        let compress = c.i32()?;
        let seek_info = c.seek(large)?;
        let nbytes_info = c.i32()?;
        let nbytes_name = usize::try_from(nbytes_name)
            .map_err(|_| Error::Corrupt(format!("fNbytesName {nbytes_name}")))?;
        let begin_us = usize::try_from(begin).unwrap_or(usize::MAX);
        if begin_us >= data.len() {
            return Err(Error::Truncated(format!(
                "the top directory at {begin} is past the end ({} bytes)",
                data.len()
            )));
        }
        // The TFile key: its header, then the file's name and title.
        let fkey = key_header(&mut Cur::at(data, begin_us))?;
        let mut nc = Cur::at(data, begin_us + usize::from(fkey.keylen));
        let name = nc.tstring().unwrap_or_default();
        let title = nc.tstring().unwrap_or_default();
        // The TDirectory record after the name.
        let at = begin_us
            .checked_add(nbytes_name)
            .filter(|&a| a < data.len())
            .ok_or_else(|| Error::Truncated("the top directory record".into()))?;
        let top = dir_record(&mut Cur::at(data, at))?;
        let mut f = RootFile {
            version,
            large,
            begin,
            end,
            compress,
            seek_info,
            nbytes_info: u32::try_from(nbytes_info).unwrap_or(0),
            name,
            title,
            seek_keys: top.seek_keys,
            created: top.created,
            modified: top.modified,
            keys: Vec::new(),
            streamers: Vec::new(),
            warnings: Vec::new(),
            dir_bytes: 0,
        };
        let mut seen = HashSet::new();
        f.read_dir(data, top.seek_keys, String::new(), 0, &mut seen)?;
        match f.read_streamers(data) {
            Ok(s) => f.streamers = s,
            Err(e) => f.warnings.push(format!("streamer info: {e}")),
        }
        Ok(f)
    }

    /// The keys of the directory at `seek_keys`, then those of its subdirectories.
    fn read_dir(
        &mut self,
        data: &[u8],
        seek_keys: u64,
        path: String,
        depth: usize,
        seen: &mut HashSet<u64>,
    ) -> Result<()> {
        if seek_keys == 0 {
            return Ok(());
        }
        if depth > MAX_DEPTH || !seen.insert(seek_keys) {
            return Err(Error::Corrupt(format!(
                "directory {path:?} loops or nests too deep"
            )));
        }
        let at = usize::try_from(seek_keys)
            .ok()
            .filter(|&a| a < data.len())
            .ok_or_else(|| Error::Truncated(format!("the key list of {path:?}")))?;
        let mut c = Cur::at(data, at);
        let _list = key_header(&mut c)?;
        let n = c.i32()?;
        let n = usize::try_from(n)
            .ok()
            .filter(|&n| n <= MAX_KEYS && self.keys.len() + n <= MAX_KEYS)
            .ok_or_else(|| Error::Corrupt(format!("{n} keys")))?;
        self.dir_bytes = self.dir_bytes.saturating_add(path.len().saturating_mul(n));
        if self.dir_bytes > MAX_DIR_BYTES {
            return Err(Error::Corrupt(format!(
                "directory {:?} has too long a path",
                path.get(..64).unwrap_or("")
            )));
        }
        let start = self.keys.len();
        for _ in 0..n {
            let mut k = key_header(&mut c)?;
            k.dir.clone_from(&path);
            self.keys.push(k);
        }
        // Subdirectories after this directory's own keys.
        for i in start..start + n {
            if !self.keys[i].is_directory() {
                continue;
            }
            let k = self.keys[i].clone();
            let sub = self
                .payload(data, &k)
                .and_then(|b| dir_record(&mut Cur::at(&b, 0)));
            match sub {
                Ok(d) => {
                    if let Err(e) = self.read_dir(data, d.seek_keys, k.path(), depth + 1, seen) {
                        self.warnings.push(format!("{}: {e}", k.path()));
                    }
                }
                Err(e) => self.warnings.push(format!("{}: {e}", k.path())),
            }
        }
        Ok(())
    }

    /// The (class, version) of every TStreamerInfo in the streamer list.
    fn read_streamers(&self, data: &[u8]) -> Result<Vec<(String, i32)>> {
        if self.seek_info == 0 {
            return Ok(Vec::new());
        }
        let at = usize::try_from(self.seek_info)
            .ok()
            .filter(|&a| a < data.len())
            .ok_or_else(|| Error::Truncated("the streamer info".into()))?;
        let key = key_header(&mut Cur::at(data, at))?;
        let bytes = self.payload(data, &key)?;
        let mut c = Cur::new(&bytes, usize::from(key.keylen));
        let (_, _end) = c.version()?;
        c.tobject()?;
        let _name = c.tstring()?;
        let n = c.i32()?;
        let n = usize::try_from(n).map_err(|_| Error::Corrupt("streamer count".into()))?;
        let mut out = Vec::new();
        for _ in 0..n.min(c.left()) {
            match c.any()? {
                Any::Obj { class, end } => {
                    if class == "TStreamerInfo" {
                        let (_, _e) = c.version()?;
                        let (name, _) = c.tnamed()?;
                        let _check = c.u32()?;
                        let v = c.i32()?;
                        out.push((name, v));
                    }
                    match end {
                        Some(e) => c.seek_end(e, "streamer")?,
                        None => break,
                    }
                }
                Any::Null | Any::Ref => {}
            }
            let _option = c.tstring()?;
        }
        Ok(out)
    }

    /// The object of `key`, decompressed.
    pub fn payload<'d>(&self, data: &'d [u8], key: &Key) -> Result<Cow<'d, [u8]>> {
        self.payload_prefix(data, key, None)
    }

    /// The first `prefix` bytes of the object (or all of it), decompressing only the blocks
    /// needed.
    pub fn payload_prefix<'d>(
        &self,
        data: &'d [u8],
        key: &Key,
        prefix: Option<usize>,
    ) -> Result<Cow<'d, [u8]>> {
        let start = usize::try_from(key.seek)
            .ok()
            .and_then(|s| s.checked_add(usize::from(key.keylen)));
        let stored = (key.nbytes as usize).saturating_sub(usize::from(key.keylen));
        let Some(bytes) = start.and_then(|s| data.get(s..s.checked_add(stored)?)) else {
            return Err(Error::Truncated(format!(
                "{} ({} bytes at {}) is past the end of the file",
                key.path(),
                key.nbytes,
                key.seek
            )));
        };
        let objlen = key.objlen as usize;
        if objlen > MAX_OBJECT {
            return Err(Error::Unsupported(format!(
                "{} is {objlen} bytes uncompressed",
                key.path()
            )));
        }
        if !key.compressed() {
            return Ok(Cow::Borrowed(&bytes[..objlen.min(bytes.len())]));
        }
        compress::unzip(bytes, objlen, prefix).map(Cow::Owned)
    }

    /// Reads the object of `key`: a histogram, graph or tree header.
    pub fn read(&self, data: &[u8], key: &Key) -> Result<Object> {
        let class = key.class.as_str();
        if is_tree_class(class) {
            let bytes = self.payload_prefix(data, key, Some(4096))?;
            let mut c = Cur::new(&bytes, usize::from(key.keylen));
            return object::read_tree(&mut c, class).map(Object::Tree);
        }
        let read_hist = is_hist_class(class);
        if !read_hist && !is_graph_class(class) {
            return Err(Error::Unsupported(format!("class {class}")));
        }
        let bytes = self.payload(data, key)?;
        let mut c = Cur::new(&bytes, usize::from(key.keylen));
        if read_hist {
            object::read_hist(&mut c, class).map(|h| Object::Hist(Box::new(h)))
        } else {
            object::read_graph(&mut c, class).map(|g| Object::Graph(Box::new(g)))
        }
    }

    /// A description of fCompress ("zlib level 4", "none").
    pub fn compression(&self) -> String {
        describe_compress(self.compress)
    }

    /// The ROOT release that wrote the file: 62804 is "6.28/04".
    pub fn version_string(&self) -> String {
        let v = self.version;
        format!("{}.{:02}/{:02}", v / 10000, v / 100 % 100, v % 100)
    }
}

/// fCompress in words: algorithm and level.
pub fn describe_compress(c: i32) -> String {
    let level = c.rem_euclid(100);
    if level == 0 {
        return "none".to_owned();
    }
    format!("{} level {level}", algorithm_name(c / 100))
}

/// A TKey header.
fn key_header(c: &mut Cur) -> Result<Key> {
    let nbytes = c.i32()?;
    let version = c.i16()?;
    let objlen = c.i32()?;
    let datime = c.u32()?;
    let keylen = c.i16()?;
    let cycle = c.i16()?;
    let wide = version > 1000;
    let seek = c.seek(wide)?;
    let _pdir = c.seek(wide)?;
    let class = c.tstring()?;
    let name = c.tstring()?;
    let title = c.tstring()?;
    let nbytes = u32::try_from(nbytes)
        .map_err(|_| Error::Corrupt(format!("key {name:?} of {nbytes} bytes")))?;
    let objlen = u32::try_from(objlen)
        .map_err(|_| Error::Corrupt(format!("key {name:?} of length {objlen}")))?;
    let keylen = u16::try_from(keylen)
        .ok()
        .filter(|&k| k >= 18 && u32::from(k) <= nbytes)
        .ok_or_else(|| Error::Corrupt(format!("key {name:?} header of {keylen} bytes")))?;
    Ok(Key {
        dir: String::new(),
        class,
        name,
        title,
        cycle,
        nbytes,
        objlen,
        keylen,
        seek,
        datime,
    })
}

struct DirRecord {
    created: u32,
    modified: u32,
    seek_keys: u64,
}

/// A TDirectory record: version, times, sizes, then three seeks (64-bit past version 1000).
fn dir_record(c: &mut Cur) -> Result<DirRecord> {
    let v = c.i16()?;
    let created = c.u32()?;
    let modified = c.u32()?;
    let _nbytes_keys = c.i32()?;
    let _nbytes_name = c.i32()?;
    let wide = v > 1000;
    let _seek_dir = c.seek(wide)?;
    let _seek_parent = c.seek(wide)?;
    let seek_keys = c.seek(wide)?;
    Ok(DirRecord {
        created,
        modified,
        seek_keys,
    })
}

#[cfg(test)]
mod tests;
