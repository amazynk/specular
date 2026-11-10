//! Plain Zarr stores, versions 2 and 3, in a folder or a ZIP (`.zarr.zip`): every array of the
//! hierarchy (consolidated metadata used when present, else the folders walked), with its
//! attributes and dimension names (v3 `dimension_names`, xarray's `_ARRAY_DIMENSIONS`).
//!
//! * **v2:** `.zarray` with C or Fortran chunk order; compressors blosc, zstd, gzip, zlib and
//!   lz4; filters shuffle and delta; every numeric dtype in either byte order, booleans,
//!   half floats and complex; fill values (numbers, `NaN`, `Infinity`); chunk keys with `.`
//!   or `/` separators.
//! * **v3:** `zarr.json` with a regular chunk grid; codecs bytes (endian), transpose, blosc,
//!   gzip, zstd, crc32c, numcodecs' zlib / lz4 / shuffle / delta, and sharding_indexed; the
//!   default and v2 chunk key encodings; hex and complex fill values.
//!
//! Missing chunks are the fill value. Object, string and structured dtypes are refused with a
//! reason; so are codecs and filters not listed.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::Value;

use crate::{Dtype, Elem, Error, MAX_DIMS, Part, PlaneSpec, Result, bad};

/// Largest chunk or metadata document read.
const MAX_CHUNK: u64 = 1 << 30;
/// Arrays listed at most.
const MAX_ARRAYS: usize = 4096;
/// Group nesting walked at most.
const MAX_DEPTH: usize = 16;
/// Chunks read for one plane (each is a parallel job with a buffer of its own).
const MAX_PLANE_CHUNKS: u64 = 1 << 20;
/// Entries of one folder looked at when walking at most.
const MAX_LIST: usize = 1 << 16;
/// Decoded chunks kept for reuse, bytes.
const CACHE_BYTES: usize = 256 << 20;

/// Keyed bytes: a folder or an archive.
pub trait Store: Send + Sync {
    /// The whole value at `key` ('/'-separated, relative to the root).
    fn get(&self, key: &str) -> Option<Vec<u8>>;
    /// `len` bytes at `start` of the value.
    fn get_range(&self, key: &str, start: u64, len: u64) -> Option<Vec<u8>> {
        let v = self.get(key)?;
        let s = usize::try_from(start).ok()?;
        let e = s.checked_add(usize::try_from(len).ok()?)?;
        v.get(s..e).map(<[u8]>::to_vec)
    }
    /// The value's size.
    fn size(&self, key: &str) -> Option<u64> {
        self.get(key).map(|v| v.len() as u64)
    }
    /// The names of the folders directly under `dir` ("" for the root, else ending in '/').
    fn subdirs(&self, dir: &str) -> Vec<String>;
    /// [`get`](Self::get) for a store that can fail in other ways than a missing key (a
    /// remote one): `Ok(None)` is absence, an error anything else.
    fn try_get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.get(key))
    }
    /// [`get_range`](Self::get_range), fallible.
    fn try_get_range(&self, key: &str, start: u64, len: u64) -> Result<Option<Vec<u8>>> {
        Ok(self.get_range(key, start, len))
    }
    /// [`size`](Self::size), fallible.
    fn try_size(&self, key: &str) -> Result<Option<u64>> {
        Ok(self.size(key))
    }
    /// Whether reads are slow (a network): readers then say what they need up front.
    fn batched(&self) -> bool {
        false
    }
    /// Fetches `wants` ahead of the reads that follow, several at a time.
    fn prefetch(&self, _wants: &[specular_core::keys::Want]) -> Result<()> {
        Ok(())
    }
}

/// A store in a local folder. Keys never leave it.
#[derive(Debug)]
pub struct DirStore {
    root: PathBuf,
}

impl DirStore {
    pub fn new(root: impl Into<PathBuf>) -> DirStore {
        DirStore { root: root.into() }
    }

    fn path(&self, key: &str) -> Option<PathBuf> {
        let rel = Path::new(key);
        rel.components()
            .all(|c| matches!(c, Component::Normal(_)))
            .then(|| self.root.join(rel))
    }
}

impl Store for DirStore {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        let p = self.path(key)?;
        let m = std::fs::metadata(&p).ok()?;
        if !m.is_file() || m.len() > MAX_CHUNK {
            return None;
        }
        std::fs::read(p).ok()
    }

    fn get_range(&self, key: &str, start: u64, len: u64) -> Option<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        if len > MAX_CHUNK {
            return None;
        }
        let p = self.path(key)?;
        if !std::fs::metadata(&p).ok()?.is_file() {
            return None;
        }
        let mut f = std::fs::File::open(p).ok()?;
        f.seek(SeekFrom::Start(start)).ok()?;
        let mut v = vec![0; usize::try_from(len).ok()?];
        f.read_exact(&mut v).ok()?;
        Some(v)
    }

    fn size(&self, key: &str) -> Option<u64> {
        let m = std::fs::metadata(self.path(key)?).ok()?;
        m.is_file().then_some(m.len())
    }

    fn subdirs(&self, dir: &str) -> Vec<String> {
        let base = if dir.is_empty() {
            Some(self.root.clone())
        } else {
            self.path(dir.trim_end_matches('/'))
        };
        let Some(Ok(rd)) = base.map(std::fs::read_dir) else {
            return Vec::new();
        };
        let mut out: Vec<String> = rd
            .take(MAX_LIST)
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().to_str().map(str::to_owned))
            .filter(|n| !n.starts_with('.'))
            .collect();
        out.sort();
        out
    }
}

/// A store in a ZIP archive held in memory (or mapped).
pub struct ZipStore<T> {
    data: T,
    zip: crate::Zip,
    /// The folder of the archive holding the root node ("" or "name.zarr/").
    prefix: String,
    /// The folders under the root, built on the first [`Store::subdirs`]: a scan of every
    /// entry per folder walked was quadratic in the groups.
    dirs: std::sync::OnceLock<DirTree>,
}

/// Folders as a tree: node 0 is the root; each node's subfolders by name, in order.
struct DirTree {
    nodes: Vec<std::collections::BTreeMap<String, usize>>,
}

#[cfg(test)]
thread_local! {
    /// Folder trees built (tests count them).
    static DIR_TREES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl DirTree {
    /// The folders of the entries named under `prefix`, at most [`MAX_DEPTH`] + 1 deep (the
    /// walk goes no deeper).
    fn new<'a>(names: impl Iterator<Item = &'a str>, prefix: &str) -> DirTree {
        #[cfg(test)]
        DIR_TREES.with(|n| n.set(n.get() + 1));
        let mut nodes = vec![std::collections::BTreeMap::new()];
        for name in names {
            // Every part but the last (a file, or empty for a folder entry) is a folder.
            let Some((dirs, _)) = name.strip_prefix(prefix).and_then(|r| r.rsplit_once('/')) else {
                continue;
            };
            let mut node = 0;
            for part in dirs.split('/').take(MAX_DEPTH + 1) {
                node = match nodes[node].get(part) {
                    Some(&n) => n,
                    None => {
                        let n = nodes.len();
                        nodes[node].insert(part.to_owned(), n);
                        nodes.push(std::collections::BTreeMap::new());
                        n
                    }
                };
            }
        }
        DirTree { nodes }
    }

    /// The folders directly under `dir` ("" or ending in '/'), hidden and empty names left
    /// out, at most [`MAX_LIST`].
    fn subdirs(&self, dir: &str) -> Vec<String> {
        let mut node = 0;
        if !dir.is_empty() {
            for part in dir.strip_suffix('/').unwrap_or(dir).split('/') {
                match self.nodes[node].get(part) {
                    Some(&n) => node = n,
                    None => return Vec::new(),
                }
            }
        }
        self.nodes[node]
            .keys()
            .filter(|n| !n.is_empty() && !n.starts_with('.'))
            .take(MAX_LIST)
            .cloned()
            .collect()
    }
}

impl<T: AsRef<[u8]> + Send + Sync> ZipStore<T> {
    pub fn new(data: T) -> Result<ZipStore<T>> {
        let zip = crate::Zip::parse(data.as_ref())?;
        // The shallowest metadata document marks the root.
        let prefix = zip
            .entries
            .iter()
            .filter_map(|e| {
                [".zgroup", ".zarray", "zarr.json", ".zattrs"]
                    .iter()
                    .find_map(|d| {
                        e.name
                            .strip_suffix(d)
                            .filter(|p| p.is_empty() || p.ends_with('/'))
                    })
            })
            .min_by_key(|p| (p.matches('/').count(), p.len()))
            .ok_or(Error::NotArray)?
            .to_owned();
        Ok(ZipStore {
            data,
            zip,
            prefix,
            dirs: std::sync::OnceLock::new(),
        })
    }
}

impl<T: AsRef<[u8]> + Send + Sync> Store for ZipStore<T> {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        let e = self.zip.get(&format!("{}{key}", self.prefix))?;
        self.zip.read(self.data.as_ref(), e, MAX_CHUNK).ok()
    }

    fn get_range(&self, key: &str, start: u64, len: u64) -> Option<Vec<u8>> {
        let d = self.data.as_ref();
        let e = self.zip.get(&format!("{}{key}", self.prefix))?;
        match self.zip.stored(d, e).ok()? {
            Some(r) => {
                let s = usize::try_from(start).ok()?;
                r.get(s..s.checked_add(usize::try_from(len).ok()?)?)
                    .map(<[u8]>::to_vec)
            }
            None => {
                let v = self.zip.read(d, e, MAX_CHUNK).ok()?;
                let s = usize::try_from(start).ok()?;
                v.get(s..s.checked_add(usize::try_from(len).ok()?)?)
                    .map(<[u8]>::to_vec)
            }
        }
    }

    fn size(&self, key: &str) -> Option<u64> {
        self.zip
            .get(&format!("{}{key}", self.prefix))
            .map(|e| e.size)
    }

    fn subdirs(&self, dir: &str) -> Vec<String> {
        self.dirs
            .get_or_init(|| {
                DirTree::new(
                    self.zip.entries.iter().map(|e| e.name.as_str()),
                    &self.prefix,
                )
            })
            .subdirs(dir)
    }
}

/// Whether a folder holds a Zarr node.
pub fn is_zarr_dir(p: &Path) -> bool {
    ["zarr.json", ".zattrs", ".zgroup", ".zarray"]
        .iter()
        .any(|f| p.join(f).is_file())
}

/// Whether a JSON document's first bytes are Zarr metadata (`.zarray`, `.zgroup`,
/// `zarr.json`): an object naming `zarr_format`.
pub fn is_metadata_head(head: &[u8]) -> bool {
    let t = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    t.trim_start().starts_with('{') && t.contains("\"zarr_format\"")
}

/// A bytes-to-bytes codec (or a v2 filter), in encoding order.
#[derive(Clone, Debug, PartialEq)]
pub enum Codec {
    Blosc,
    Zstd,
    Gzip,
    Zlib,
    /// numcodecs LZ4: a little-endian size, then an LZ4 block.
    Lz4,
    /// A trailing CRC32C (not checked).
    Crc32c,
    /// numcodecs Shuffle: bytes of each element gathered by position.
    Shuffle(usize),
    /// numcodecs Delta: differences of consecutive elements of this type.
    Delta(Dtype),
}

impl Codec {
    fn name(&self) -> &'static str {
        match self {
            Codec::Blosc => "blosc",
            Codec::Zstd => "zstd",
            Codec::Gzip => "gzip",
            Codec::Zlib => "zlib",
            Codec::Lz4 => "lz4",
            Codec::Crc32c => "crc32c",
            Codec::Shuffle(_) => "shuffle",
            Codec::Delta(_) => "delta",
        }
    }
}

#[derive(Clone, Debug)]
struct Shard {
    /// Inner chunk shape.
    chunk: Vec<u64>,
    codecs: Vec<Codec>,
    big: bool,
    perm: Vec<usize>,
    /// Index at the start (else the end); with a CRC32C after it.
    index_start: bool,
    index_crc: bool,
}

/// One array of a store.
#[derive(Clone, Debug)]
pub struct ZArray {
    /// Path from the root ("" for a root array, "group/name").
    pub path: String,
    pub zarr_format: u8,
    pub shape: Vec<u64>,
    /// Chunk shape (shard shape when sharded).
    pub chunks: Vec<u64>,
    pub dtype: Dtype,
    /// The dtype as the metadata names it ("<f4", "float32").
    pub dtype_name: String,
    /// Real and imaginary fill value.
    pub fill: (f64, f64),
    /// The stored order of a chunk's dimensions, slowest first (C order: 0, 1, …).
    pub perm: Vec<usize>,
    /// Bytes-to-bytes codecs and v2 filters, in encoding order.
    pub codecs: Vec<Codec>,
    shard: Option<Shard>,
    key_prefix: String,
    sep: String,
    /// The array's attributes.
    pub attrs: Value,
    /// A name per dimension, when the store gives them.
    pub dim_names: Vec<Option<String>>,
}

impl ZArray {
    /// "blosc", "zstd, sharded", "none".
    pub fn compression(&self) -> String {
        let mut names: Vec<&str> = self
            .shard
            .as_ref()
            .map_or(&self.codecs, |s| &s.codecs)
            .iter()
            .filter(|c| !matches!(c, Codec::Crc32c))
            .map(Codec::name)
            .collect();
        if names.is_empty() {
            names.push("none");
        }
        let mut s = names.join(", ");
        if self.shard.is_some() {
            s += ", sharded";
        }
        s
    }

    fn read_chunk_shape(&self) -> &[u64] {
        self.shard.as_ref().map_or(&self.chunks, |s| &s.chunk)
    }

    fn chunk_perm(&self) -> &[usize] {
        self.shard.as_ref().map_or(&self.perm, |s| &s.perm)
    }

    fn big_endian(&self) -> bool {
        self.shard.as_ref().map_or(self.dtype.big, |s| s.big)
    }

    fn chunk_key(&self, idx: &[u64]) -> String {
        let mut k = self.key_prefix.clone();
        if idx.is_empty() {
            k.push('0');
        }
        for (i, v) in idx.iter().enumerate() {
            if i > 0 {
                k.push_str(&self.sep);
            }
            k.push_str(&v.to_string());
        }
        k
    }
}

fn u64s(v: &Value) -> Option<Vec<u64>> {
    let a = v.as_array()?;
    if a.len() > MAX_DIMS {
        return None;
    }
    a.iter().map(Value::as_u64).collect()
}

fn json(store: &dyn Store, key: &str) -> Option<Value> {
    serde_json::from_slice(&store.get(key)?).ok()
}

fn num_fill(v: &Value, elem: Elem) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        Value::String(s) => match s.as_str() {
            "NaN" => Some(f64::NAN),
            "Infinity" => Some(f64::INFINITY),
            "-Infinity" => Some(f64::NEG_INFINITY),
            h if h.starts_with("0x") => {
                let bits = u64::from_str_radix(&h[2..], 16).ok()?;
                Some(match elem.real() {
                    Elem::F16 => f64::from(specular_hdf5::f16_to_f32(bits as u16)),
                    Elem::F32 => f64::from(f32::from_bits(bits as u32)),
                    Elem::F64 => f64::from_bits(bits),
                    _ => bits as f64,
                })
            }
            // A v2 fill value for bytes types is base64; not for numbers.
            _ => None,
        },
        _ => None,
    }
}

fn fill_value(v: Option<&Value>, elem: Elem) -> (f64, f64) {
    match v {
        Some(Value::Array(a)) if a.len() == 2 => (
            num_fill(&a[0], elem).unwrap_or(0.0),
            num_fill(&a[1], elem).unwrap_or(0.0),
        ),
        Some(v) => (num_fill(v, elem).unwrap_or(0.0), 0.0),
        None => (0.0, 0.0),
    }
}

fn dtype_v2(s: &str) -> Result<Dtype> {
    match crate::npy::parse_descr(s) {
        Ok((d, None)) => Ok(d),
        Ok((d, Some(_))) => Ok(d),
        Err(Error::Unsupported(m)) => Err(Error::Unsupported(m.replace("NumPy", "Zarr"))),
        Err(e) => Err(e),
    }
}

fn dtype_v3(s: &str) -> Result<Dtype> {
    let elem = match s {
        "bool" => Elem::Bool,
        "int8" => Elem::I8,
        "uint8" => Elem::U8,
        "int16" => Elem::I16,
        "uint16" => Elem::U16,
        "int32" => Elem::I32,
        "uint32" => Elem::U32,
        "int64" => Elem::I64,
        "uint64" => Elem::U64,
        "float16" => Elem::F16,
        "float32" => Elem::F32,
        "float64" => Elem::F64,
        "complex64" => Elem::C64,
        "complex128" => Elem::C128,
        "string" | "variable_length_utf8" | "fixed_length_utf32" => {
            return Err(Error::Unsupported(
                "Zarr text (string) arrays are not images".into(),
            ));
        }
        other => {
            return Err(Error::Unsupported(format!(
                "Zarr data type {other} is not supported"
            )));
        }
    };
    Ok(Dtype::le(elem))
}

fn v2_codec(c: &Value, dtype: Dtype) -> Result<Codec> {
    let id = c.get("id").and_then(Value::as_str).unwrap_or("");
    numcodec(id, c, dtype)
}

/// A numcodecs codec by id (v2 compressors and filters, v3 `numcodecs.*` codecs).
fn numcodec(id: &str, conf: &Value, dtype: Dtype) -> Result<Codec> {
    Ok(match id {
        "blosc" => Codec::Blosc,
        "zstd" => Codec::Zstd,
        "gzip" => Codec::Gzip,
        "zlib" => Codec::Zlib,
        "lz4" => Codec::Lz4,
        "crc32c" => Codec::Crc32c,
        "shuffle" => Codec::Shuffle(
            conf.get("elementsize")
                .and_then(Value::as_u64)
                .map_or(dtype.size(), |n| n as usize)
                .max(1),
        ),
        "delta" => {
            let d = conf
                .get("dtype")
                .and_then(Value::as_str)
                .map(dtype_v2)
                .transpose()?
                .unwrap_or(dtype);
            Codec::Delta(d)
        }
        "bz2" | "lzma" | "zfpy" | "fixedscaleoffset" | "quantize" | "packbits" | "categorize"
        | "astype" | "bitround" | "pcodec" | "zfp" | "jpeg2k" | "imagecodecs_jpeg2k" => {
            return Err(Error::Unsupported(format!(
                "the Zarr codec {id} is not supported"
            )));
        }
        other => {
            return Err(Error::Unsupported(format!(
                "the Zarr codec {other} is not supported"
            )));
        }
    })
}

/// v3 codecs: the bytes-to-bytes ones in order, the endianness, the chunk's memory order and
/// sharding.
fn v3_codecs(list: &Value, ndim: usize, dtype: Dtype) -> Result<V3Codecs> {
    let mut out = V3Codecs {
        codecs: Vec::new(),
        big: false,
        perm: (0..ndim).collect(),
        shard: None,
    };
    for c in list.as_array().into_iter().flatten() {
        let name = c.get("name").and_then(Value::as_str).unwrap_or("");
        let empty = Value::Null;
        let conf = c.get("configuration").unwrap_or(&empty);
        match name {
            "bytes" | "endian" => {
                out.big = conf.get("endian").and_then(Value::as_str) == Some("big");
            }
            "transpose" => {
                let order = conf.get("order");
                out.perm = match order {
                    Some(Value::String(s)) if s == "C" => (0..ndim).collect(),
                    Some(Value::String(s)) if s == "F" => (0..ndim).rev().collect(),
                    Some(v) => {
                        let p: Vec<usize> = u64s(v)
                            .ok_or_else(|| Error::Malformed("Zarr transpose order".into()))?
                            .into_iter()
                            .map(|x| x as usize)
                            .collect();
                        let mut seen = p.clone();
                        seen.sort_unstable();
                        if seen != (0..ndim).collect::<Vec<_>>() {
                            return bad("Zarr transpose order");
                        }
                        // A transpose of a transpose composes.
                        p.iter().map(|&i| out.perm[i]).collect()
                    }
                    None => return bad("Zarr transpose order"),
                };
            }
            "blosc" => out.codecs.push(Codec::Blosc),
            "zstd" => out.codecs.push(Codec::Zstd),
            "gzip" => out.codecs.push(Codec::Gzip),
            "crc32c" => out.codecs.push(Codec::Crc32c),
            n if n.starts_with("numcodecs.") => {
                out.codecs
                    .push(numcodec(&n["numcodecs.".len()..], conf, dtype)?);
            }
            "sharding_indexed" => {
                let chunk = conf
                    .get("chunk_shape")
                    .and_then(u64s)
                    .filter(|c| c.len() == ndim)
                    .ok_or_else(|| Error::Malformed("Zarr sharding chunk shape".into()))?;
                let inner = v3_codecs(conf.get("codecs").unwrap_or(&Value::Null), ndim, dtype)?;
                if inner.shard.is_some() {
                    return Err(Error::Unsupported(
                        "nested Zarr sharding is not supported".into(),
                    ));
                }
                let index = v3_codecs(
                    conf.get("index_codecs").unwrap_or(&Value::Null),
                    1,
                    Dtype::le(Elem::U64),
                )?;
                if index.codecs.iter().any(|c| *c != Codec::Crc32c) || index.big {
                    return Err(Error::Unsupported(
                        "compressed Zarr shard indexes are not supported".into(),
                    ));
                }
                out.shard = Some(Shard {
                    chunk,
                    codecs: inner.codecs,
                    big: inner.big,
                    perm: inner.perm,
                    index_start: conf.get("index_location").and_then(Value::as_str)
                        == Some("start"),
                    index_crc: !index.codecs.is_empty(),
                });
            }
            n => {
                return Err(Error::Unsupported(format!(
                    "the Zarr codec {n} is not supported"
                )));
            }
        }
    }
    Ok(out)
}

struct V3Codecs {
    codecs: Vec<Codec>,
    big: bool,
    perm: Vec<usize>,
    shard: Option<Shard>,
}

/// Reads an array's metadata document (`m`, already parsed) at `path`.
fn array_of(m: &Value, path: &str, attrs_v2: Option<Value>) -> Result<ZArray> {
    let dir = if path.is_empty() {
        String::new()
    } else {
        format!("{path}/")
    };
    let format = m.get("zarr_format").and_then(Value::as_u64).unwrap_or(0);
    if format == 2 || m.get("chunks").is_some() && m.get("node_type").is_none() {
        let shape = m.get("shape").and_then(u64s);
        let chunks = m.get("chunks").and_then(u64s);
        let (Some(shape), Some(chunks)) = (shape, chunks) else {
            return bad("Zarr array shape");
        };
        let dt = match m.get("dtype") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(_)) => {
                return Err(Error::Unsupported(
                    "Zarr structured (record) arrays are not supported".into(),
                ));
            }
            _ => return bad("Zarr dtype"),
        };
        let dtype = dtype_v2(&dt)?;
        let ndim = shape.len();
        let perm: Vec<usize> = if m.get("order").and_then(Value::as_str) == Some("F") {
            (0..ndim).rev().collect()
        } else {
            (0..ndim).collect()
        };
        let mut codecs = Vec::new();
        for f in m
            .get("filters")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            codecs.push(v2_codec(f, dtype)?);
        }
        if let Some(c) = m.get("compressor").filter(|c| !c.is_null()) {
            codecs.push(v2_codec(c, dtype)?);
        }
        let sep = m
            .get("dimension_separator")
            .and_then(Value::as_str)
            .unwrap_or(".")
            .to_owned();
        let attrs = attrs_v2.unwrap_or(Value::Null);
        let dim_names = attrs
            .get("_ARRAY_DIMENSIONS")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        return check(ZArray {
            path: path.to_owned(),
            zarr_format: 2,
            shape,
            chunks,
            dtype,
            dtype_name: dt,
            fill: fill_value(m.get("fill_value"), dtype.elem),
            perm,
            codecs,
            shard: None,
            key_prefix: dir,
            sep,
            attrs,
            dim_names,
        });
    }
    if m.get("node_type").and_then(Value::as_str) != Some("array") {
        return bad("Zarr node is not an array");
    }
    let shape = m
        .get("shape")
        .and_then(u64s)
        .ok_or_else(|| Error::Malformed("Zarr array shape".into()))?;
    let grid = m.get("chunk_grid");
    if grid.and_then(|g| g.get("name")).and_then(Value::as_str) != Some("regular") {
        return Err(Error::Unsupported(
            "Zarr chunk grids other than regular are not supported".into(),
        ));
    }
    let chunks = grid
        .and_then(|g| g.get("configuration"))
        .and_then(|c| c.get("chunk_shape"))
        .and_then(u64s)
        .ok_or_else(|| Error::Malformed("Zarr chunk shape".into()))?;
    let dt = match m.get("data_type") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(o)) => o
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned(),
        _ => return bad("Zarr data type"),
    };
    let mut dtype = dtype_v3(&dt)?;
    let c = v3_codecs(m.get("codecs").unwrap_or(&Value::Null), shape.len(), dtype)?;
    dtype.big = c.big;
    let enc = m.get("chunk_key_encoding");
    let enc_name = enc
        .and_then(|e| e.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("default");
    let sep_given = enc
        .and_then(|e| e.get("configuration"))
        .and_then(|c| c.get("separator"))
        .and_then(Value::as_str);
    let (key_prefix, sep) = if enc_name == "v2" {
        (dir.clone(), sep_given.unwrap_or(".").to_owned())
    } else {
        let s = sep_given.unwrap_or("/").to_owned();
        (format!("{dir}c{s}"), s)
    };
    let attrs = m.get("attributes").cloned().unwrap_or(Value::Null);
    let dim_names = m
        .get("dimension_names")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|v| v.as_str().map(str::to_owned)).collect())
        .or_else(|| {
            attrs
                .get("_ARRAY_DIMENSIONS")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|v| v.as_str().map(str::to_owned)).collect())
        })
        .unwrap_or_default();
    check(ZArray {
        path: path.to_owned(),
        zarr_format: 3,
        shape,
        chunks,
        dtype,
        dtype_name: dt,
        fill: fill_value(m.get("fill_value"), dtype.elem),
        perm: c.perm,
        codecs: c.codecs,
        shard: c.shard,
        key_prefix,
        sep,
        attrs,
        dim_names,
    })
}

/// Rejects chunk shapes no read can use: a rank other than the shape's, an empty dimension,
/// inner chunks that do not divide the shard, or a chunk whose byte size overflows.
fn check(a: ZArray) -> Result<ZArray> {
    let cs = a.read_chunk_shape();
    if a.chunks.len() != a.shape.len()
        || a.chunks.contains(&0) && !a.shape.contains(&0)
        || cs.len() != a.chunks.len()
        || cs.contains(&0) && !a.shape.contains(&0)
        || a.shard.as_ref().is_some_and(|s| {
            a.chunks
                .iter()
                .zip(&s.chunk)
                .any(|(o, i)| *i == 0 || o % i != 0)
        })
        || cs
            .iter()
            .try_fold(a.dtype.size() as u64, |n, &c| n.checked_mul(c))
            .is_none_or(|n| n > MAX_CHUNK)
    {
        return bad("Zarr chunk shape");
    }
    Ok(a)
}

/// A decoded chunk by key, with its memory reserved while it is cached.
type Cached = (String, Arc<Vec<u8>>, specular_core::Reservation);

/// The arrays of a store, and the root's attributes.
pub struct Zarr {
    pub store: Arc<dyn Store>,
    pub arrays: Vec<ZArray>,
    /// The root group's attributes (or the root array's).
    pub attrs: Value,
    /// 2 or 3.
    pub zarr_format: u8,
    /// Nodes that could not be read, with the reason.
    pub refused: Vec<(String, String)>,
    /// Whether the store had consolidated metadata.
    pub consolidated: bool,
    cache: Mutex<Vec<Cached>>,
}

impl std::fmt::Debug for Zarr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Zarr")
            .field("arrays", &self.arrays.len())
            .field("zarr_format", &self.zarr_format)
            .finish()
    }
}

/// Opens a store: the root node and every array under it.
pub fn open(store: Arc<dyn Store>) -> Result<Zarr> {
    let mut z = Zarr {
        store: store.clone(),
        arrays: Vec::new(),
        attrs: Value::Null,
        zarr_format: 2,
        refused: Vec::new(),
        consolidated: false,
        cache: Mutex::new(Vec::new()),
    };
    if let Some(root) = json(store.as_ref(), "zarr.json") {
        z.zarr_format = 3;
        z.attrs = root.get("attributes").cloned().unwrap_or(Value::Null);
        // Inline consolidated metadata (zarr-python 3).
        if let Some(Value::Object(meta)) = root
            .get("consolidated_metadata")
            .and_then(|c| c.get("metadata"))
        {
            z.consolidated = true;
            let mut keys: Vec<&String> = meta.keys().collect();
            keys.sort();
            for k in keys {
                let m = &meta[k];
                if m.get("node_type").and_then(Value::as_str) == Some("array") {
                    z.add(array_of(m, k, None), k);
                }
            }
        }
        if root.get("node_type").and_then(Value::as_str) == Some("array") {
            z.add(array_of(&root, "", None), "");
        } else if !z.consolidated {
            z.walk("", 0, 3);
        }
    } else if let Some(m) = json(store.as_ref(), ".zarray") {
        z.attrs = json(store.as_ref(), ".zattrs").unwrap_or(Value::Null);
        z.add(array_of(&m, "", Some(z.attrs.clone())), "");
    } else if json(store.as_ref(), ".zgroup").is_some() || store.get(".zattrs").is_some() {
        z.attrs = json(store.as_ref(), ".zattrs").unwrap_or(Value::Null);
        if let Some(Value::Object(meta)) =
            json(store.as_ref(), ".zmetadata").and_then(|c| c.get("metadata").cloned())
        {
            z.consolidated = true;
            let mut keys: Vec<&String> = meta.keys().collect();
            keys.sort();
            for k in keys {
                if let Some(path) = k.strip_suffix(".zarray") {
                    let path = path.trim_end_matches('/');
                    let attrs = meta.get(&format!("{path}/.zattrs")).cloned();
                    z.add(array_of(&meta[k], path, attrs), path);
                }
            }
        } else {
            z.walk("", 0, 2);
        }
    } else {
        return Err(Error::NotArray);
    }
    if z.arrays.is_empty() && z.refused.is_empty() {
        return Err(Error::Unsupported("the Zarr store holds no arrays".into()));
    }
    Ok(z)
}

impl Zarr {
    fn add(&mut self, a: Result<ZArray>, path: &str) {
        if self.arrays.len() >= MAX_ARRAYS {
            return;
        }
        match a {
            Ok(a) => self.arrays.push(a),
            Err(e) => self.refused.push((path.to_owned(), e.to_string())),
        }
    }

    /// Walks the folders under `dir` for nodes of `format`.
    fn walk(&mut self, dir: &str, depth: usize, format: u8) {
        if depth >= MAX_DEPTH || self.arrays.len() >= MAX_ARRAYS {
            return;
        }
        let store = self.store.clone();
        for name in store.subdirs(dir) {
            let path = format!("{dir}{name}");
            let sub = format!("{path}/");
            if format == 3 {
                let Some(m) = json(store.as_ref(), &format!("{sub}zarr.json")) else {
                    continue;
                };
                match m.get("node_type").and_then(Value::as_str) {
                    Some("array") => self.add(array_of(&m, &path, None), &path),
                    Some("group") => self.walk(&sub, depth + 1, 3),
                    _ => {}
                }
            } else if let Some(m) = json(store.as_ref(), &format!("{sub}.zarray")) {
                let attrs = json(store.as_ref(), &format!("{sub}.zattrs"));
                self.add(array_of(&m, &path, attrs), &path);
            } else if store.size(&format!("{sub}.zgroup")).is_some() {
                self.walk(&sub, depth + 1, 2);
            }
        }
    }

    /// The decoded chunk at grid position `idx` (read-chunk shape, stored order), `None` if
    /// absent.
    /// Decoding reserves the stored and decoded bytes from `budget`; a cached chunk keeps
    /// its decoded bytes reserved.
    fn chunk(
        &self,
        a: &ZArray,
        idx: &[u64],
        budget: &specular_core::BudgetHandle,
    ) -> Result<Option<Arc<Vec<u8>>>> {
        let ck = format!("{}#{idx:?}", a.path);
        {
            let mut c = self.cache.lock();
            if let Some(i) = c.iter().position(|(k, ..)| *k == ck) {
                let e = c.remove(i);
                let v = e.1.clone();
                c.push(e);
                return Ok(Some(v));
            }
        }
        let n = a
            .read_chunk_shape()
            .iter()
            .product::<u64>()
            .saturating_mul(a.dtype.size() as u64);
        if n > MAX_CHUNK {
            return Err(Error::TooLarge);
        }
        let (raw, codecs) = match &a.shard {
            None => match self.store.try_get(&a.chunk_key(idx))? {
                None => return Ok(None),
                Some(raw) => (raw, &a.codecs),
            },
            Some(sh) => match self.shard_chunk(a, sh, idx)? {
                None => return Ok(None),
                Some(raw) => (raw, &sh.codecs),
            },
        };
        // Every chunk of a plane decodes at once, each up to MAX_CHUNK: the decoded bytes,
        // and the stored ones while decoding, count against the budget.
        let kept = budget.reserve(n)?;
        let stored = budget.reserve(raw.len() as u64)?;
        let v = Arc::new(decode(codecs, raw, n as usize)?);
        drop(stored);
        let mut c = self.cache.lock();
        c.push((ck, v.clone(), kept));
        let mut total: usize = c.iter().map(|(_, b, _)| b.len()).sum();
        while total > CACHE_BYTES && c.len() > 1 {
            total -= c.remove(0).1.len();
        }
        Ok(Some(v))
    }

    fn shard_chunk(&self, a: &ZArray, sh: &Shard, idx: &[u64]) -> Result<Option<Vec<u8>>> {
        let per: Vec<u64> = a
            .chunks
            .iter()
            .zip(&sh.chunk)
            .map(|(s, c)| s / c.max(&1))
            .collect();
        let outer: Vec<u64> = idx.iter().zip(&per).map(|(i, p)| i / p.max(&1)).collect();
        let inner: Vec<u64> = idx.iter().zip(&per).map(|(i, p)| i % p.max(&1)).collect();
        let mut lin = 0u64;
        for (i, p) in inner.iter().zip(&per) {
            lin = lin * p + i;
        }
        let total = per
            .iter()
            .try_fold(1u64, |n, &p| n.checked_mul(p))
            .filter(|&t| t <= MAX_CHUNK / 16)
            .ok_or_else(|| Error::Malformed("Zarr shard index".into()))?;
        let key = a.chunk_key(&outer);
        let Some(size) = self.store.try_size(&key)? else {
            return Ok(None);
        };
        let ilen = total * 16 + if sh.index_crc { 4 } else { 0 };
        if ilen > size {
            return bad("Zarr shard index");
        }
        let istart = if sh.index_start { 0 } else { size - ilen };
        let entry = self
            .store
            .try_get_range(&key, istart + lin * 16, 16)?
            .filter(|e| e.len() == 16)
            .ok_or_else(|| Error::Truncated("Zarr shard index".into()))?;
        let off = crate::le_u64(&entry, 0).unwrap_or(0);
        let len = crate::le_u64(&entry, 8).unwrap_or(0);
        if off == u64::MAX && len == u64::MAX {
            return Ok(None);
        }
        if len > MAX_CHUNK || off.checked_add(len).is_none_or(|e| e > size) {
            return bad("Zarr shard entry");
        }
        self.store
            .try_get_range(&key, off, len)?
            .map(Some)
            .ok_or_else(|| Error::Truncated("Zarr shard chunk".into()))
    }

    /// Reads one plane of array `a`; missing chunks are the fill value, chunks that fail to
    /// decode are NaN (and make the result true).
    pub fn read_plane(
        &self,
        a: &ZArray,
        spec: &PlaneSpec,
        part: Part,
        out: &mut Vec<f32>,
    ) -> Result<bool> {
        let (w, h) = spec.check(&a.shape)?;
        crate::alloc(out, w * h)?;
        let cs = a.read_chunk_shape().to_vec();
        let cstrides = crate::strides_perm(&cs, a.chunk_perm());
        let dtype = Dtype {
            elem: a.dtype.elem,
            big: a.big_endian(),
        };
        let fill = part.of(a.fill.0, a.fill.1);
        let fill = if a.dtype.elem.is_complex() {
            fill
        } else {
            a.fill.0 as f32
        };
        let xd = spec.x_dim;
        let (cx, cy) = (cs[xd].max(1), spec.y_dim.map_or(1, |d| cs[d].max(1)));
        let idx0: Vec<u64> = (0..a.shape.len())
            .map(|d| spec.index.get(d).copied().unwrap_or(0) / cs[d].max(1))
            .collect();
        // The rectangle's first element within a chunk.
        let mut base = 0u64;
        for (d, s) in cstrides.iter().enumerate() {
            if d != xd && Some(d) != spec.y_dim {
                base += (spec.index.get(d).copied().unwrap_or(0) % cs[d].max(1)) * s;
            }
        }
        let sx = cstrides[xd];
        let sy = spec.y_dim.map_or(0, |d| cstrides[d]);
        // Each chunk of the plane is a job with a buffer of its own: 1×1 chunks over a large
        // plane were 2^28 jobs and buffers from a 100-byte `.zarray`.
        if (h as u64).div_ceil(cy) * (w as u64).div_ceil(cx) > MAX_PLANE_CHUNKS {
            return Err(Error::TooLarge);
        }
        let cells: Vec<(u64, u64)> = (0..(h as u64).div_ceil(cy))
            .flat_map(|gy| (0..(w as u64).div_ceil(cx)).map(move |gx| (gx, gy)))
            .collect();
        // A remote store fetches the plane's chunks (unsharded) several at a time first;
        // sharded ones are read by range as they decode.
        if self.store.batched() && a.shard.is_none() {
            let wants: Vec<specular_core::keys::Want> = cells
                .iter()
                .map(|&(gx, gy)| {
                    let mut idx = idx0.clone();
                    idx[xd] = gx;
                    if let Some(yd) = spec.y_dim {
                        idx[yd] = gy;
                    }
                    specular_core::keys::Want::whole(a.chunk_key(&idx))
                })
                .collect();
            self.store.prefetch(&wants)?;
        }
        // Chunks decode in parallel; each fills its rectangle of the plane, reserving from
        // this thread's budget.
        let budget = specular_core::current_budget();
        use rayon::prelude::*;
        let rects: Vec<Result<Rect>> = cells
            .par_iter()
            .map(|&(gx, gy)| {
                let mut idx = idx0.clone();
                idx[xd] = gx;
                if let Some(yd) = spec.y_dim {
                    idx[yd] = gy;
                }
                let (x0, y0) = (gx * cx, gy * cy);
                let (x1, y1) = ((x0 + cx).min(w as u64), (y0 + cy).min(h as u64));
                let (rw, rh) = ((x1 - x0) as usize, (y1 - y0) as usize);
                let mut tmp = Vec::new();
                crate::alloc(&mut tmp, rw * rh)?;
                let t = match self.chunk(a, &idx, &budget) {
                    Ok(Some(chunk)) => {
                        crate::gather_rows(&chunk, dtype, base, sx, sy, rw, part, &mut tmp)
                    }
                    Ok(None) => {
                        tmp.fill(fill);
                        false
                    }
                    Err(e @ (Error::TooLarge | Error::NeedsMemory(_) | Error::Io(_))) => {
                        return Err(e);
                    }
                    Err(_) => {
                        tmp.fill(f32::NAN);
                        true
                    }
                };
                Ok((x0, y0, rw, rh, tmp, t))
            })
            .collect();
        let mut truncated = false;
        for r in rects {
            let (x0, y0, rw, rh, tmp, t) = r?;
            truncated |= t;
            for r in 0..rh {
                let o = (y0 as usize + r) * w + x0 as usize;
                out[o..o + rw].copy_from_slice(&tmp[r * rw..(r + 1) * rw]);
            }
        }
        Ok(truncated)
    }
}

/// A chunk's rectangle of a plane: its corner, size, values and whether any were missing.
type Rect = (u64, u64, usize, usize, Vec<f32>, bool);

/// Undoes `codecs` (encoding order) on a stored chunk of `n` decoded bytes.
fn decode(codecs: &[Codec], raw: Vec<u8>, n: usize) -> Result<Vec<u8>> {
    let mut b = raw;
    let e = |x: String| Error::Malformed(format!("Zarr chunk: {x}"));
    for c in codecs.iter().rev() {
        let max = n.max(16);
        b = match c {
            Codec::Blosc => specular_hdf5::codec::blosc(&b, max).map_err(|x| e(x.to_string()))?,
            Codec::Zstd => specular_hdf5::codec::zstd(&b, max).map_err(|x| e(x.to_string()))?,
            Codec::Gzip => specular_png::inflate_gzip(&b, max).map_err(|x| e(x.to_string()))?,
            Codec::Zlib => specular_png::inflate_zlib(&b, max).map_err(|x| e(x.to_string()))?,
            Codec::Lz4 => {
                let m = crate::le_u32(&b, 0).ok_or_else(|| e("LZ4 size".into()))? as usize;
                if m > max {
                    return Err(Error::TooLarge);
                }
                specular_hdf5::codec::lz4_block(&b[4..], m).map_err(|x| e(x.to_string()))?
            }
            Codec::Crc32c => {
                let k = b.len().saturating_sub(4);
                b.truncate(k);
                b
            }
            Codec::Shuffle(es) => unshuffle(&b, *es),
            Codec::Delta(d) => {
                undelta(&mut b, *d);
                b
            }
        };
    }
    if b.len() < n {
        b.resize(n, 0);
    }
    Ok(b)
}

/// numcodecs Shuffle undone: byte `k` of every element was stored together.
fn unshuffle(b: &[u8], es: usize) -> Vec<u8> {
    let n = b.len() / es.max(1);
    let mut out = b.to_vec();
    if es <= 1 {
        return out;
    }
    for k in 0..es {
        for i in 0..n {
            out[i * es + k] = b[k * n + i];
        }
    }
    out
}

/// numcodecs Delta undone: a running sum over the chunk's elements of type `d`.
fn undelta(b: &mut [u8], d: Dtype) {
    let size = d.size();
    let n = b.len() / size.max(1);
    macro_rules! run {
        ($t:ty, $add:expr) => {{
            let mut acc: $t = Default::default();
            for i in 0..n {
                let s = &mut b[i * size..(i + 1) * size];
                let mut a = [0u8; std::mem::size_of::<$t>()];
                a.copy_from_slice(s);
                let v = if d.big {
                    <$t>::from_be_bytes(a)
                } else {
                    <$t>::from_le_bytes(a)
                };
                acc = $add(acc, v);
                s.copy_from_slice(&if d.big {
                    acc.to_be_bytes()
                } else {
                    acc.to_le_bytes()
                });
            }
        }};
    }
    match d.elem {
        Elem::I8 => run!(i8, i8::wrapping_add),
        Elem::U8 | Elem::Bool => run!(u8, u8::wrapping_add),
        Elem::I16 => run!(i16, i16::wrapping_add),
        Elem::U16 => run!(u16, u16::wrapping_add),
        Elem::I32 => run!(i32, i32::wrapping_add),
        Elem::U32 => run!(u32, u32::wrapping_add),
        Elem::I64 => run!(i64, i64::wrapping_add),
        Elem::U64 => run!(u64, u64::wrapping_add),
        Elem::F32 => run!(f32, |a: f32, v: f32| a + v),
        Elem::F64 => run!(f64, |a: f64, v: f64| a + v),
        // Half floats and complex values are not delta-coded by numcodecs in practice.
        Elem::F16 | Elem::C64 | Elem::C128 => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A store in memory.
    #[derive(Default)]
    pub(crate) struct Mem(pub HashMap<String, Vec<u8>>);

    impl Store for Mem {
        fn get(&self, key: &str) -> Option<Vec<u8>> {
            self.0.get(key).cloned()
        }
        fn subdirs(&self, dir: &str) -> Vec<String> {
            let mut out: Vec<String> = self
                .0
                .keys()
                .filter_map(|k| k.strip_prefix(dir))
                .filter_map(|r| r.split_once('/').map(|(a, _)| a.to_owned()))
                .collect();
            out.sort();
            out.dedup();
            out
        }
    }

    fn le16(v: &[u16]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    /// Four 64 MiB chunks (LZ4, 256 KiB stored each) decode on the plane's workers and stay
    /// cached: under a 100 MiB budget the read is refused for memory, and a chunk keeps its
    /// decoded bytes reserved while cached.
    #[test]
    fn chunk_output_counts_against_the_budget() {
        let mut m = Mem::default();
        m.0.insert(
            ".zarray".into(),
            br#"{"zarr_format": 2, "shape": [16384, 16384], "chunks": [8192, 8192],
                "dtype": "|u1", "order": "C", "compressor": {"id": "lz4"},
                "filters": null, "fill_value": 0}"#
                .to_vec(),
        );
        // numcodecs LZ4: the size, then one zero, a match repeating it, five zeros.
        let n = 64usize << 20;
        let rest = n - 1 - 5 - 4 - 15;
        let mut lz4 = u32::try_from(n).unwrap().to_le_bytes().to_vec();
        lz4.extend_from_slice(&[0x1f, 0, 1, 0]);
        lz4.extend(std::iter::repeat_n(255, rest / 255));
        lz4.push((rest % 255) as u8);
        lz4.extend_from_slice(&[0x50, 0, 0, 0, 0, 0]);
        assert_eq!(decode(&[Codec::Lz4], lz4.clone(), n).unwrap(), vec![0; n]);
        for k in ["0.0", "0.1", "1.0", "1.1"] {
            m.0.insert(k.into(), lz4.clone());
        }
        let store: Arc<dyn Store> = Arc::new(m);
        let z = open(store.clone()).unwrap();
        let spec = PlaneSpec {
            x_dim: 1,
            y_dim: Some(0),
            index: vec![0, 0],
        };
        let r = specular_core::budget::with_limit(100 << 20, || {
            z.read_plane(&z.arrays[0], &spec, Part::Real, &mut Vec::new())
        });
        assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
        // One chunk fits; its decoded bytes stay reserved while it is cached.
        let z = open(store).unwrap();
        let b = Arc::new(specular_core::budget::MemoryBudget::fixed(100 << 20));
        let budget = specular_core::budget::with_budget(b.clone(), specular_core::current_budget);
        assert!(z.chunk(&z.arrays[0], &[0, 0], &budget).unwrap().is_some());
        assert_eq!(b.reserved(), 64 << 20);
        drop(z);
        assert_eq!(b.reserved(), 0);
    }

    /// 3,000 groups of a zipped store, each listed: a scan of every entry per group was
    /// quadratic; the folders are now read once, and list as before.
    #[test]
    fn zip_subdirs_read_once() {
        let n = 3000;
        let mut names = vec!["s.zarr/.zgroup".to_string(), "other/x/y".to_string()];
        for i in 0..n {
            names.push(format!("s.zarr/g{i:04}/.zgroup"));
            names.push(format!("s.zarr/g{i:04}/a/0.0"));
        }
        names.push("s.zarr/.hidden/x".into());
        names.push("s.zarr/e//x".into());
        names.push("s.zarr/f/".into());
        let members: Vec<crate::zip::tests::Member<'_>> =
            names.iter().map(|n| (n.as_str(), &b""[..], None)).collect();
        let z = ZipStore::new(crate::zip::tests::build(&members, false)).unwrap();
        DIR_TREES.with(|c| c.set(0));
        let top = z.subdirs("");
        assert_eq!(top.len(), n + 2);
        assert_eq!(&top[..2], ["e", "f"]);
        for g in &top[2..] {
            assert_eq!(z.subdirs(&format!("{g}/")), ["a"]);
            assert!(z.subdirs(&format!("{g}/a/")).is_empty());
        }
        assert!(z.subdirs("e/").is_empty());
        assert!(z.subdirs("missing/").is_empty());
        assert_eq!(DIR_TREES.with(std::cell::Cell::get), 1);
    }

    /// 1×1 chunks over a 16384×16384 plane, no chunk stored: 2^28 parallel jobs, each with
    /// its own buffer, from a 100-byte `.zarray`. Refused before any job.
    #[test]
    fn tiny_chunks_over_a_large_plane() {
        let mut m = Mem::default();
        m.0.insert(
            ".zarray".into(),
            br#"{"zarr_format": 2, "shape": [16384, 16384], "chunks": [1, 1], "dtype": "<u2",
                "order": "C", "compressor": null, "filters": null, "fill_value": 0}"#
                .to_vec(),
        );
        let z = open(Arc::new(m)).unwrap();
        let a = &z.arrays[0];
        let spec = PlaneSpec {
            x_dim: 1,
            y_dim: Some(0),
            index: vec![0, 0],
        };
        let mut out = Vec::new();
        assert!(matches!(
            z.read_plane(a, &spec, Part::Real, &mut out),
            Err(Error::TooLarge)
        ));
    }

    #[test]
    fn v2_group_with_fortran_and_missing_chunks() {
        let mut m = Mem::default();
        m.0.insert(".zgroup".into(), br#"{"zarr_format": 2}"#.to_vec());
        // 3 × 4 uint16 in 2 × 2 chunks, Fortran order, fill 9; chunk (1, 1) missing.
        m.0.insert(
            "a/.zarray".into(),
            br#"{"zarr_format": 2, "shape": [3, 4], "chunks": [2, 2], "dtype": "<u2",
                "order": "F", "compressor": null, "filters": null, "fill_value": 9}"#
                .to_vec(),
        );
        m.0.insert(
            "a/.zattrs".into(),
            br#"{"_ARRAY_DIMENSIONS": ["y", "x"]}"#.to_vec(),
        );
        // Values v[y][x] = 10 y + x; chunk (cy, cx) stored column-major.
        let chunk = |cy: u16, cx: u16| {
            let mut v = Vec::new();
            for x in 0..2 {
                for y in 0..2 {
                    v.push(10 * (cy * 2 + y) + cx * 2 + x);
                }
            }
            le16(&v)
        };
        for (k, cy, cx) in [("a/0.0", 0, 0), ("a/0.1", 0, 1), ("a/1.0", 1, 0)] {
            m.0.insert(k.into(), chunk(cy, cx));
        }
        let z = open(Arc::new(m)).unwrap();
        assert_eq!(z.arrays.len(), 1);
        let a = &z.arrays[0];
        assert_eq!(a.path, "a");
        assert_eq!(a.dim_names, [Some("y".into()), Some("x".into())]);
        let mut out = Vec::new();
        let spec = PlaneSpec {
            x_dim: 1,
            y_dim: Some(0),
            index: vec![0, 0],
        };
        assert!(!z.read_plane(a, &spec, Part::Real, &mut out).unwrap());
        assert_eq!(
            out,
            [
                0.0, 1.0, 2.0, 3.0, 10.0, 11.0, 12.0, 13.0, 20.0, 21.0, 9.0, 9.0
            ]
        );
    }

    #[test]
    fn v3_transpose_shuffle_and_fill() {
        let mut m = Mem::default();
        m.0.insert(
            "zarr.json".into(),
            br#"{"zarr_format": 3, "node_type": "array", "shape": [2, 3],
                "data_type": "int16", "chunk_grid": {"name": "regular",
                "configuration": {"chunk_shape": [2, 3]}},
                "chunk_key_encoding": {"name": "default"},
                "fill_value": "0x7fff",
                "codecs": [{"name": "transpose", "configuration": {"order": [1, 0]}},
                           {"name": "bytes", "configuration": {"endian": "big"}},
                           {"name": "numcodecs.shuffle", "configuration": {"elementsize": 2}}]}"#
                .to_vec(),
        );
        // Values [[1, 2, 3], [4, 5, 6]] transposed: 1 4 2 5 3 6, big-endian, shuffled.
        let vals = [1i16, 4, 2, 5, 3, 6];
        let be: Vec<u8> = vals.iter().flat_map(|v| v.to_be_bytes()).collect();
        let mut sh = vec![0u8; 12];
        for i in 0..6 {
            sh[i] = be[i * 2];
            sh[6 + i] = be[i * 2 + 1];
        }
        m.0.insert("c/0/0".into(), sh);
        let z = open(Arc::new(m)).unwrap();
        let a = &z.arrays[0];
        assert_eq!(a.fill.0, 32767.0);
        let mut out = Vec::new();
        z.read_plane(
            a,
            &PlaneSpec {
                x_dim: 1,
                y_dim: Some(0),
                index: vec![0, 0],
            },
            Part::Real,
            &mut out,
        )
        .unwrap();
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn refusals_have_reasons() {
        let mut m = Mem::default();
        m.0.insert(
            ".zarray".into(),
            br#"{"zarr_format": 2, "shape": [2], "chunks": [2], "dtype": "|O",
                "compressor": null, "filters": null, "fill_value": 0}"#
                .to_vec(),
        );
        let z = open(Arc::new(m)).unwrap();
        assert!(z.arrays.is_empty());
        assert!(z.refused[0].1.contains("Python"), "{:?}", z.refused);
        let mut m = Mem::default();
        m.0.insert(
            ".zarray".into(),
            br#"{"zarr_format": 2, "shape": [2], "chunks": [2], "dtype": "<f4",
                "compressor": {"id": "bz2"}, "filters": null, "fill_value": 0}"#
                .to_vec(),
        );
        let z = open(Arc::new(m)).unwrap();
        assert!(z.refused[0].1.contains("bz2"));
    }

    #[test]
    fn delta_and_unshuffle() {
        let mut b: Vec<u8> = [1i32, 1, 1, -2]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        undelta(&mut b, Dtype::le(Elem::I32));
        let v: Vec<i32> = b
            .chunks(4)
            .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        assert_eq!(v, [1, 2, 3, 1]);
        assert_eq!(unshuffle(&[1, 3, 2, 4], 2), [1, 2, 3, 4]);
    }
}
