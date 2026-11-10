//! OME-Zarr (NGFF 0.1 to 0.5) on Zarr v2 and v3, from a local folder or a `.zip`.
//!
//! - Groups: `.zattrs`/`.zgroup` (v2) or `zarr.json` (v3, attributes under `ome`); a
//!   bioformats2raw layout's first series (`0/`) when the root has no multiscales.
//! - multiscales: axes (names, types, units), datasets with scale transformations, and the
//!   multiscale's own transformation; the first multiscale only.
//! - omero: channel labels, colours and windows.
//! - Arrays: C order; v2 compressors blosc, zstd, gzip, zlib, lz4 (no filters); v3 codecs
//!   bytes, blosc, gzip, zstd, crc32c and sharding_indexed; integer and float dtypes either
//!   endian; missing chunks are the fill value.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub use specular_core::keys::{Part, Want};

use parking_lot::Mutex;
use rayon::prelude::*;
use serde_json::Value;

use crate::composite::{self, Plane};
use crate::{Channel, Error, Level, Props, ReadOptions, Region, Result, Rgba, Slide, Vendor};

/// Largest chunk or metadata document read.
const MAX_CHUNK: u64 = 1 << 30;
/// Most channels composited.
const MAX_CHANNELS: usize = 16;
/// Most array dimensions.
const MAX_DIMS: usize = 8;

/// Keyed bytes: a folder or an archive.
pub trait Store: Send + Sync {
    /// The whole value at `key` ('/'-separated, relative to the root).
    fn get(&self, key: &str) -> Option<Vec<u8>>;
    /// `len` bytes at `start` of the value (for shard indexes and inner chunks).
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
    /// [`get`](Self::get) for a store that can fail in other ways than a missing key (a
    /// remote one): `Ok(None)` is absence, an error anything else.
    fn try_get(&self, key: &str, _cancel: Option<&AtomicBool>) -> Result<Option<Vec<u8>>> {
        Ok(self.get(key))
    }
    /// [`get_range`](Self::get_range), fallible.
    fn try_get_range(
        &self,
        key: &str,
        start: u64,
        len: u64,
        _cancel: Option<&AtomicBool>,
    ) -> Result<Option<Vec<u8>>> {
        Ok(self.get_range(key, start, len))
    }
    /// The last `len` bytes of the value (all of it when shorter) and its size.
    fn try_get_suffix(
        &self,
        key: &str,
        len: u64,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<(Vec<u8>, u64)>> {
        let Some(size) = self.size(key) else {
            return Ok(None);
        };
        let n = len.min(size);
        Ok(self
            .try_get_range(key, size - n, n, cancel)?
            .map(|b| (b, size)))
    }
    /// [`size`](Self::size), fallible.
    fn try_size(&self, key: &str, _cancel: Option<&AtomicBool>) -> Result<Option<u64>> {
        Ok(self.size(key))
    }
    /// Whether reads are slow (a network): readers then say what they need up front
    /// ([`prefetch`](Self::prefetch)) and keep shard indexes.
    fn batched(&self) -> bool {
        false
    }
    /// Fetches `wants` ahead of the reads that follow, several at a time.
    fn prefetch(&self, _wants: &[Want], _cancel: Option<&AtomicBool>) -> Result<()> {
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
        let mut f = std::fs::File::open(self.path(key)?).ok()?;
        f.seek(SeekFrom::Start(start)).ok()?;
        let mut v = vec![0; usize::try_from(len).ok()?];
        f.read_exact(&mut v).ok()?;
        Some(v)
    }

    fn size(&self, key: &str) -> Option<u64> {
        let m = std::fs::metadata(self.path(key)?).ok()?;
        m.is_file().then_some(m.len())
    }
}

/// A store in a ZIP archive held in memory (or mapped).
pub struct ZipStore<T> {
    data: T,
    zip: crate::zip::Zip,
    /// The folder of the archive holding the root group ("" or "name.ome.zarr/").
    prefix: String,
}

impl<T: AsRef<[u8]> + Send + Sync> ZipStore<T> {
    pub fn new(data: T) -> Result<ZipStore<T>> {
        let zip = crate::zip::Zip::parse(data.as_ref())?;
        // The shallowest group document marks the root.
        let prefix = zip
            .entries
            .keys()
            .filter_map(|k| {
                [".zattrs", ".zgroup", "zarr.json"].iter().find_map(|d| {
                    k.strip_suffix(d)
                        .filter(|p| p.is_empty() || p.ends_with('/'))
                })
            })
            .min_by_key(|p| (p.matches('/').count(), p.len()))
            .ok_or(Error::NotSlide)?
            .to_owned();
        Ok(ZipStore { data, zip, prefix })
    }
}

impl<T: AsRef<[u8]> + Send + Sync> Store for ZipStore<T> {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        let e = self.zip.entries.get(&format!("{}{key}", self.prefix))?;
        self.zip.read(self.data.as_ref(), e).ok()
    }

    fn get_range(&self, key: &str, start: u64, len: u64) -> Option<Vec<u8>> {
        let d = self.data.as_ref();
        let e = self.zip.entries.get(&format!("{}{key}", self.prefix))?;
        match self.zip.stored(d, e).ok()? {
            Some(r) => {
                let s = r.start.checked_add(usize::try_from(start).ok()?)?;
                let end = s.checked_add(usize::try_from(len).ok()?)?;
                (end <= r.end).then(|| d[s..end].to_vec())
            }
            None => {
                let v = self.zip.read(d, e).ok()?;
                let s = usize::try_from(start).ok()?;
                v.get(s..s.checked_add(usize::try_from(len).ok()?)?)
                    .map(<[u8]>::to_vec)
            }
        }
    }

    fn size(&self, key: &str) -> Option<u64> {
        self.zip
            .entries
            .get(&format!("{}{key}", self.prefix))
            .map(|e| e.size)
    }
}

/// Whether a folder looks like a Zarr group or array.
pub fn is_zarr_dir(p: &Path) -> bool {
    ["zarr.json", ".zattrs", ".zgroup", ".zarray"]
        .iter()
        .any(|f| p.join(f).is_file())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dtype {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
    Bool,
}

impl Dtype {
    fn size(self) -> usize {
        match self {
            Dtype::U8 | Dtype::I8 | Dtype::Bool => 1,
            Dtype::U16 | Dtype::I16 => 2,
            Dtype::U32 | Dtype::I32 | Dtype::F32 => 4,
            Dtype::U64 | Dtype::I64 | Dtype::F64 => 8,
        }
    }

    fn from_v2(s: &str) -> Option<(Dtype, bool)> {
        let (endian, rest) = s.split_at(1.min(s.len()));
        let big = endian == ">";
        let t = match rest {
            "u1" => Dtype::U8,
            "i1" => Dtype::I8,
            "b1" => Dtype::Bool,
            "u2" => Dtype::U16,
            "i2" => Dtype::I16,
            "u4" => Dtype::U32,
            "i4" => Dtype::I32,
            "u8" => Dtype::U64,
            "i8" => Dtype::I64,
            "f4" => Dtype::F32,
            "f8" => Dtype::F64,
            _ => return None,
        };
        Some((t, big))
    }

    fn from_v3(s: &str) -> Option<Dtype> {
        Some(match s {
            "uint8" => Dtype::U8,
            "int8" => Dtype::I8,
            "bool" => Dtype::Bool,
            "uint16" => Dtype::U16,
            "int16" => Dtype::I16,
            "uint32" => Dtype::U32,
            "int32" => Dtype::I32,
            "uint64" => Dtype::U64,
            "int64" => Dtype::I64,
            "float32" => Dtype::F32,
            "float64" => Dtype::F64,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Dtype::U8 => "8-bit unsigned",
            Dtype::I8 => "8-bit signed",
            Dtype::U16 => "16-bit unsigned",
            Dtype::I16 => "16-bit signed",
            Dtype::U32 => "32-bit unsigned",
            Dtype::I32 => "32-bit signed",
            Dtype::U64 => "64-bit unsigned",
            Dtype::I64 => "64-bit signed",
            Dtype::F32 => "32-bit float",
            Dtype::F64 => "64-bit float",
            Dtype::Bool => "boolean",
        }
    }

    /// Element `i` of `b` as f32.
    fn get(self, b: &[u8], i: usize, big: bool) -> f32 {
        let n = self.size();
        let Some(s) = b.get(i * n..i * n + n) else {
            return f32::NAN;
        };
        let mut a = [0u8; 8];
        a[..n].copy_from_slice(s);
        if big {
            a[..n].reverse();
        }
        match self {
            Dtype::U8 | Dtype::Bool => f32::from(a[0]),
            Dtype::I8 => f32::from(a[0] as i8),
            Dtype::U16 => f32::from(u16::from_le_bytes([a[0], a[1]])),
            Dtype::I16 => f32::from(i16::from_le_bytes([a[0], a[1]])),
            Dtype::U32 => u32::from_le_bytes([a[0], a[1], a[2], a[3]]) as f32,
            Dtype::I32 => i32::from_le_bytes([a[0], a[1], a[2], a[3]]) as f32,
            Dtype::F32 => f32::from_le_bytes([a[0], a[1], a[2], a[3]]),
            Dtype::U64 => u64::from_le_bytes(a) as f32,
            Dtype::I64 => i64::from_le_bytes(a) as f32,
            Dtype::F64 => f64::from_le_bytes(a) as f32,
        }
    }
}

/// A bytes-to-bytes codec.
#[derive(Clone, Debug, PartialEq)]
enum Codec {
    Blosc,
    Zstd,
    Gzip,
    Zlib,
    /// numcodecs LZ4: a little-endian size, then an LZ4 block.
    Lz4,
    /// A trailing CRC32C (not checked).
    Crc32c,
}

fn decompress(c: &Codec, src: &[u8], max: usize) -> Result<Vec<u8>> {
    let e = |x: String| Error::Malformed(format!("Zarr chunk: {x}"));
    match c {
        Codec::Blosc => specular_hdf5::codec::blosc(src, max).map_err(|x| e(x.to_string())),
        Codec::Zstd => specular_hdf5::codec::zstd(src, max).map_err(|x| e(x.to_string())),
        Codec::Gzip => specular_png::inflate_gzip(src, max).map_err(|x| e(x.to_string())),
        Codec::Zlib => specular_png::inflate_zlib(src, max).map_err(|x| e(x.to_string())),
        Codec::Lz4 => {
            let n = src
                .get(..4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
                .ok_or_else(|| e("LZ4 size".into()))?;
            if n > max {
                return Err(Error::TooLarge);
            }
            specular_hdf5::codec::lz4_block(&src[4..], n).map_err(|x| e(x.to_string()))
        }
        Codec::Crc32c => Ok(src[..src.len().saturating_sub(4)].to_vec()),
    }
}

#[derive(Clone, Debug)]
struct Shard {
    /// Inner chunk shape.
    chunk: Vec<u64>,
    codecs: Vec<Codec>,
    big: bool,
    /// Index at the start (else the end); with a CRC32C after it.
    index_start: bool,
    index_crc: bool,
}

#[derive(Clone, Debug)]
struct Array {
    /// Key prefix ("0/").
    path: String,
    shape: Vec<u64>,
    /// Chunk (or shard) shape.
    chunks: Vec<u64>,
    dtype: Dtype,
    big: bool,
    fill: f32,
    codecs: Vec<Codec>,
    shard: Option<Shard>,
    /// How chunk keys are written: (prefix, separator).
    key_prefix: String,
    sep: String,
}

fn u64s(v: &Value) -> Option<Vec<u64>> {
    let a = v.as_array()?;
    if a.is_empty() || a.len() > MAX_DIMS {
        return None;
    }
    a.iter().map(Value::as_u64).collect()
}

fn fill_value(v: Option<&Value>) -> f32 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0) as f32,
        Some(Value::String(s)) if s == "NaN" => f32::NAN,
        Some(Value::Bool(b)) => f32::from(u8::from(*b)),
        _ => 0.0,
    }
}

fn json(store: &dyn Store, key: &str) -> Option<Value> {
    serde_json::from_slice(&store.get(key)?).ok()
}

fn v2_codec(c: &Value) -> Result<Option<Codec>> {
    if c.is_null() {
        return Ok(None);
    }
    Ok(Some(
        match c.get("id").and_then(Value::as_str).unwrap_or("") {
            "blosc" => Codec::Blosc,
            "zstd" => Codec::Zstd,
            "gzip" => Codec::Gzip,
            "zlib" => Codec::Zlib,
            "lz4" => Codec::Lz4,
            id => return Err(Error::Unsupported(format!("Zarr compressor {id}"))),
        },
    ))
}

/// v3 codecs: the bytes-to-bytes ones in order, the endianness, and sharding.
fn v3_codecs(list: &Value) -> Result<(Vec<Codec>, bool, Option<Shard>)> {
    let mut out = Vec::new();
    let (mut big, mut shard) = (false, None);
    for c in list.as_array().into_iter().flatten() {
        let name = c.get("name").and_then(Value::as_str).unwrap_or("");
        let conf = c.get("configuration");
        match name {
            "bytes" | "endian" => {
                big = conf.and_then(|x| x.get("endian")).and_then(Value::as_str) == Some("big");
            }
            "transpose" => {
                let order = conf.and_then(|x| x.get("order"));
                let identity = order.and_then(Value::as_array).is_some_and(|o| {
                    o.iter()
                        .enumerate()
                        .all(|(i, v)| v.as_u64() == Some(i as u64))
                }) || order.and_then(Value::as_str) == Some("C");
                if !identity {
                    return Err(Error::Unsupported("Zarr transpose codec".into()));
                }
            }
            "blosc" => out.push(Codec::Blosc),
            "zstd" => out.push(Codec::Zstd),
            "gzip" => out.push(Codec::Gzip),
            "crc32c" => out.push(Codec::Crc32c),
            "sharding_indexed" => {
                let conf = conf.ok_or_else(|| Error::Malformed("sharding configuration".into()))?;
                let chunk = conf
                    .get("chunk_shape")
                    .and_then(u64s)
                    .ok_or_else(|| Error::Malformed("sharding chunk shape".into()))?;
                let (codecs, b, inner) = v3_codecs(conf.get("codecs").unwrap_or(&Value::Null))?;
                if inner.is_some() {
                    return Err(Error::Unsupported("nested Zarr sharding".into()));
                }
                let (icodecs, _, _) = v3_codecs(conf.get("index_codecs").unwrap_or(&Value::Null))?;
                if icodecs.iter().any(|c| *c != Codec::Crc32c) {
                    return Err(Error::Unsupported("compressed Zarr shard index".into()));
                }
                shard = Some(Shard {
                    chunk,
                    codecs,
                    big: b,
                    index_start: conf.get("index_location").and_then(Value::as_str)
                        == Some("start"),
                    index_crc: !icodecs.is_empty(),
                });
            }
            n => return Err(Error::Unsupported(format!("Zarr codec {n}"))),
        }
    }
    Ok((out, big, shard))
}

fn read_array(store: &dyn Store, path: &str) -> Result<Array> {
    let dir = if path.is_empty() || path.ends_with('/') {
        path.to_owned()
    } else {
        format!("{path}/")
    };
    if let Some(m) = json(store, &format!("{dir}.zarray")) {
        let shape = m.get("shape").and_then(u64s);
        let chunks = m.get("chunks").and_then(u64s);
        let (Some(shape), Some(chunks)) = (shape, chunks) else {
            return Err(Error::Malformed("Zarr array shape".into()));
        };
        let dt = m.get("dtype").and_then(Value::as_str).unwrap_or("");
        let (dtype, big) =
            Dtype::from_v2(dt).ok_or_else(|| Error::Unsupported(format!("Zarr dtype {dt}")))?;
        if m.get("order").and_then(Value::as_str) == Some("F") {
            return Err(Error::Unsupported("Fortran-order Zarr arrays".into()));
        }
        if m.get("filters")
            .is_some_and(|f| !f.is_null() && f.as_array().is_none_or(|a| !a.is_empty()))
        {
            return Err(Error::Unsupported("Zarr filters".into()));
        }
        let codecs = v2_codec(m.get("compressor").unwrap_or(&Value::Null))?
            .into_iter()
            .collect();
        let sep = m
            .get("dimension_separator")
            .and_then(Value::as_str)
            .unwrap_or(".")
            .to_owned();
        return check_chunks(Array {
            path: dir.clone(),
            shape,
            chunks,
            dtype,
            big,
            fill: fill_value(m.get("fill_value")),
            codecs,
            shard: None,
            key_prefix: dir,
            sep,
        });
    }
    let m = json(store, &format!("{dir}zarr.json"))
        .ok_or_else(|| Error::Malformed(format!("Zarr array {path} missing")))?;
    if m.get("node_type").and_then(Value::as_str) != Some("array") {
        return Err(Error::Malformed("Zarr node is not an array".into()));
    }
    let shape = m
        .get("shape")
        .and_then(u64s)
        .ok_or_else(|| Error::Malformed("Zarr array shape".into()))?;
    let chunks = m
        .get("chunk_grid")
        .and_then(|g| g.get("configuration"))
        .and_then(|c| c.get("chunk_shape"))
        .and_then(u64s)
        .ok_or_else(|| Error::Unsupported("Zarr chunk grid".into()))?;
    let dt = m.get("data_type").and_then(Value::as_str).unwrap_or("");
    let dtype = Dtype::from_v3(dt).ok_or_else(|| Error::Unsupported(format!("Zarr dtype {dt}")))?;
    let (codecs, big, shard) = v3_codecs(m.get("codecs").unwrap_or(&Value::Null))?;
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
    check_chunks(Array {
        path: dir,
        shape,
        chunks,
        dtype,
        big,
        fill: fill_value(m.get("fill_value")),
        codecs,
        shard,
        key_prefix,
        sep,
    })
}

/// Rejects chunk shapes no read can use: an empty dimension, inner chunks of another rank,
/// or a chunk whose byte size overflows.
fn check_chunks(a: Array) -> Result<Array> {
    let cs = a.read_chunk_shape();
    if a.chunks.contains(&0)
        || cs.contains(&0)
        || cs.len() != a.chunks.len()
        || cs
            .iter()
            .try_fold(a.dtype.size() as u64, |n, &c| n.checked_mul(c))
            .is_none()
    {
        return Err(Error::Malformed("Zarr chunk shape".into()));
    }
    Ok(a)
}

/// A decoded chunk and its reservation.
type Decoded = (Vec<u8>, specular_core::Reservation);

/// Shard indexes most kept per slide.
const MAX_INDEXES: usize = 4096;

/// Shard indexes read from a slow store, by shard key: the index and the shard's size.
#[derive(Default)]
pub(crate) struct Indexes(Mutex<HashMap<String, Arc<ShardIndex>>>);

/// A shard's index and the shard's size.
type ShardIndex = (Vec<u8>, u64);

impl Array {
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

    /// The shard holding chunk `idx`, the chunk's place in it (C order) and the index size.
    fn shard_pos(&self, sh: &Shard, idx: &[u64]) -> Result<(String, u64, u64)> {
        let per: Vec<u64> = self
            .chunks
            .iter()
            .zip(&sh.chunk)
            .map(|(s, c)| s.div_ceil(*c))
            .collect();
        let outer: Vec<u64> = idx.iter().zip(&per).map(|(i, p)| i / p).collect();
        let inner: Vec<u64> = idx.iter().zip(&per).map(|(i, p)| i % p).collect();
        let mut lin = 0u64;
        for (i, p) in inner.iter().zip(&per) {
            lin = lin * p + i;
        }
        let total = per
            .iter()
            .try_fold(1u64, |n, &p| n.checked_mul(p))
            .filter(|&t| t <= MAX_CHUNK / 16)
            .ok_or_else(|| Error::Malformed("Zarr shard index".into()))?;
        let ilen = total * 16 + if sh.index_crc { 4 } else { 0 };
        if ilen > MAX_CHUNK {
            return Err(Error::Malformed("Zarr shard index".into()));
        }
        Ok((self.chunk_key(&outer), lin, ilen))
    }

    /// A shard's whole index and the shard's size, read once and kept (slow stores only);
    /// `None` when the shard is absent.
    fn shard_index(
        &self,
        store: &dyn Store,
        sh: &Shard,
        key: &str,
        ilen: u64,
        cancel: Option<&AtomicBool>,
        kept: &Indexes,
    ) -> Result<Option<Arc<ShardIndex>>> {
        if let Some(i) = kept.0.lock().get(key) {
            return Ok(Some(i.clone()));
        }
        let read = if sh.index_start {
            match store.try_get_range(key, 0, ilen, cancel)? {
                Some(b) => store.try_size(key, cancel)?.map(|n| (b, n)),
                None => None,
            }
        } else {
            store.try_get_suffix(key, ilen, cancel)?
        };
        let Some((bytes, size)) = read else {
            return Ok(None);
        };
        if bytes.len() as u64 != ilen || ilen > size {
            return Err(Error::Malformed("Zarr shard index".into()));
        }
        let i = Arc::new((bytes, size));
        let mut k = kept.0.lock();
        if k.len() >= MAX_INDEXES {
            k.clear();
        }
        k.insert(key.to_owned(), i.clone());
        Ok(Some(i))
    }

    /// Where chunk `idx` is inside its shard: `(shard key, offset, length)`, `None` when the
    /// chunk or the shard is absent.
    fn shard_entry(
        &self,
        store: &dyn Store,
        sh: &Shard,
        idx: &[u64],
        cancel: Option<&AtomicBool>,
        kept: &Indexes,
    ) -> Result<Option<(String, u64, u64)>> {
        let (key, lin, ilen) = self.shard_pos(sh, idx)?;
        let (entry, size) = if store.batched() {
            let Some(ix) = self.shard_index(store, sh, &key, ilen, cancel, kept)? else {
                return Ok(None);
            };
            let at = (lin * 16) as usize;
            (ix.0[at..at + 16].to_vec(), ix.1)
        } else {
            let Some(size) = store.try_size(&key, cancel)? else {
                return Ok(None);
            };
            if ilen > size {
                return Err(Error::Malformed("Zarr shard index".into()));
            }
            let istart = if sh.index_start { 0 } else { size - ilen };
            let entry = store
                .try_get_range(&key, istart + lin * 16, 16, cancel)?
                .filter(|e| e.len() == 16)
                .ok_or_else(|| Error::Truncated("Zarr shard index".into()))?;
            (entry, size)
        };
        let off = u64::from_le_bytes(entry[..8].try_into().unwrap_or_default());
        let len = u64::from_le_bytes(entry[8..16].try_into().unwrap_or_default());
        if off == u64::MAX && len == u64::MAX {
            return Ok(None);
        }
        if len > MAX_CHUNK || off.checked_add(len).is_none_or(|e| e > size) {
            return Err(Error::Malformed("Zarr shard entry".into()));
        }
        Ok(Some((key, off, len)))
    }

    /// The decoded chunk at grid position `idx` (chunk shape, C order), `None` if absent,
    /// with its bytes reserved from `budget`.
    fn chunk(
        &self,
        store: &dyn Store,
        idx: &[u64],
        budget: &specular_core::BudgetHandle,
        cancel: Option<&AtomicBool>,
        kept: &Indexes,
    ) -> Result<Option<Decoded>> {
        let Some(sh) = &self.shard else {
            let n = self.chunks.iter().product::<u64>() * self.dtype.size() as u64;
            if n > MAX_CHUNK {
                return Err(Error::TooLarge);
            }
            let Some(raw) = store.try_get(&self.chunk_key(idx), cancel)? else {
                return Ok(None);
            };
            return self.decode(&self.codecs, raw, n as usize, budget).map(Some);
        };
        let Some((key, off, len)) = self.shard_entry(store, sh, idx, cancel, kept)? else {
            return Ok(None);
        };
        let raw = store
            .try_get_range(&key, off, len, cancel)?
            .filter(|r| r.len() as u64 == len)
            .ok_or_else(|| Error::Truncated("Zarr shard chunk".into()))?;
        let n = sh.chunk.iter().product::<u64>() * self.dtype.size() as u64;
        if n > MAX_CHUNK {
            return Err(Error::TooLarge);
        }
        self.decode(&sh.codecs, raw, n as usize, budget).map(Some)
    }

    /// Asks a slow store for the chunks at `idxs` (and the shard indexes they need) up
    /// front, several at a time.
    fn prefetch(
        &self,
        store: &dyn Store,
        idxs: &[Vec<u64>],
        cancel: Option<&AtomicBool>,
        kept: &Indexes,
    ) -> Result<()> {
        let Some(sh) = &self.shard else {
            let wants: Vec<Want> = idxs
                .iter()
                .map(|i| Want::whole(self.chunk_key(i)))
                .collect();
            return store.prefetch(&wants, cancel);
        };
        let mut shards: Vec<Want> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for i in idxs {
            let (key, _, ilen) = self.shard_pos(sh, i)?;
            if !kept.0.lock().contains_key(&key) && seen.insert(key.clone()) {
                shards.push(if sh.index_start {
                    Want::range(key, 0, ilen)
                } else {
                    Want::suffix(key, ilen)
                });
            }
        }
        store.prefetch(&shards, cancel)?;
        let mut inner = Vec::new();
        for i in idxs {
            if let Some((key, off, len)) = self.shard_entry(store, sh, i, cancel, kept)? {
                inner.push(Want::range(key, off, len));
            }
        }
        store.prefetch(&inner, cancel)
    }

    /// Every chunk of a region is decoded and held until the region is put together: each
    /// reserves its decoded bytes (kept with them), and its stored ones while decoding.
    fn decode(
        &self,
        codecs: &[Codec],
        raw: Vec<u8>,
        n: usize,
        budget: &specular_core::BudgetHandle,
    ) -> Result<Decoded> {
        let kept = budget.reserve(n as u64).map_err(Error::NeedsMemory)?;
        let _stored = budget
            .reserve(raw.len() as u64)
            .map_err(Error::NeedsMemory)?;
        let mut b = raw;
        for c in codecs.iter().rev() {
            b = decompress(c, &b, n.max(16))?;
        }
        if b.len() < n {
            b.resize(n, 0);
        }
        Ok((b, kept))
    }

    /// Chunk shape as read (inner chunks when sharded).
    fn read_chunk_shape(&self) -> &[u64] {
        self.shard.as_ref().map_or(&self.chunks, |s| &s.chunk)
    }

    fn big_endian(&self) -> bool {
        self.shard.as_ref().map_or(self.big, |s| s.big)
    }
}

#[derive(Clone, Debug)]
struct Axes {
    /// Index of each named axis.
    x: usize,
    y: usize,
    c: Option<usize>,
    z: Option<usize>,
    t: Option<usize>,
    units: Vec<Option<String>>,
}

fn parse_axes(ms: &Value, ndim: usize) -> Result<Axes> {
    let names: Vec<(String, Option<String>, Option<String>)> = match ms.get("axes") {
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| match x {
                Value::String(s) => (s.clone(), None, None),
                o => (
                    o.get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    o.get("type").and_then(Value::as_str).map(str::to_owned),
                    o.get("unit").and_then(Value::as_str).map(str::to_owned),
                ),
            })
            .collect(),
        // NGFF 0.1/0.2: always t, c, z, y, x.
        _ => ["t", "c", "z", "y", "x"]
            .iter()
            .map(|s| (s.to_string(), None, None))
            .collect(),
    };
    if names.len() != ndim || ndim < 2 {
        return Err(Error::Malformed("Zarr axes and array rank differ".into()));
    }
    let find = |n: &str, ty: &str| {
        names.iter().position(|(name, t, _)| {
            name.eq_ignore_ascii_case(n) || (t.as_deref() == Some(ty) && ty != "space")
        })
    };
    let x = names
        .iter()
        .position(|a| a.0.eq_ignore_ascii_case("x"))
        .unwrap_or(ndim - 1);
    let y = names
        .iter()
        .position(|a| a.0.eq_ignore_ascii_case("y"))
        .unwrap_or(ndim - 2);
    if x == y {
        return Err(Error::Malformed("Zarr axes".into()));
    }
    Ok(Axes {
        x,
        y,
        c: find("c", "channel").filter(|&i| i != x && i != y),
        z: names.iter().position(|a| a.0.eq_ignore_ascii_case("z")),
        t: find("t", "time").filter(|&i| i != x && i != y),
        units: names.into_iter().map(|a| a.2).collect(),
    })
}

/// The product of a dataset's (and the multiscale's) scale transformations.
fn scales(v: Option<&Value>, ndim: usize) -> Option<Vec<f64>> {
    let mut s: Option<Vec<f64>> = None;
    for t in v?.as_array()? {
        if t.get("type").and_then(Value::as_str) == Some("scale") {
            let a: Vec<f64> = t
                .get("scale")?
                .as_array()?
                .iter()
                .filter_map(Value::as_f64)
                .collect();
            if a.len() == ndim {
                s = Some(match s {
                    Some(p) => p.iter().zip(&a).map(|(x, y)| x * y).collect(),
                    None => a,
                });
            }
        }
    }
    s
}

pub(crate) struct ZarrSlide {
    store: Arc<dyn Store>,
    arrays: Vec<Array>,
    axes: Axes,
    channels: Vec<Channel>,
    windows: Mutex<Option<Vec<(f64, f64)>>>,
    indexes: Indexes,
}

fn hex_colour(s: &str) -> Option<[u8; 3]> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// The group's attributes: v2 `.zattrs`, v3 `zarr.json` attributes (under `ome` for 0.5).
fn attributes(store: &dyn Store, dir: &str) -> Option<Value> {
    if let Some(a) = json(store, &format!("{dir}.zattrs")) {
        return Some(a);
    }
    let a = json(store, &format!("{dir}zarr.json"))?
        .get("attributes")?
        .clone();
    Some(a.get("ome").cloned().unwrap_or(a))
}

pub(crate) fn open(store: Arc<dyn Store>) -> Result<Slide> {
    let mut dir = String::new();
    let mut attrs = attributes(store.as_ref(), "").ok_or(Error::NotSlide)?;
    if attrs.get("multiscales").is_none() {
        // bioformats2raw: series in numbered groups.
        attrs = attributes(store.as_ref(), "0/")
            .filter(|a| a.get("multiscales").is_some())
            .ok_or(Error::NotSlide)?;
        dir = "0/".to_owned();
    }
    let ms = attrs
        .get("multiscales")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .ok_or(Error::NotSlide)?;
    let datasets = ms
        .get("datasets")
        .and_then(Value::as_array)
        .filter(|d| !d.is_empty() && d.len() <= 64)
        .ok_or_else(|| Error::Malformed("multiscales without datasets".into()))?;
    let mut arrays = Vec::new();
    let mut level_scales = Vec::new();
    for d in datasets {
        let p = d.get("path").and_then(Value::as_str).unwrap_or("");
        if p.split('/').any(|c| c == ".." || c.is_empty()) {
            return Err(Error::Malformed("Zarr dataset path".into()));
        }
        let a = read_array(store.as_ref(), &format!("{dir}{p}"))?;
        level_scales.push(scales(d.get("coordinateTransformations"), a.shape.len()));
        arrays.push(a);
    }
    let ndim = arrays[0].shape.len();
    if arrays
        .iter()
        .any(|a| a.shape.len() != ndim || a.chunks.len() != ndim)
    {
        return Err(Error::Malformed("Zarr levels of different rank".into()));
    }
    let axes = parse_axes(ms, ndim)?;
    let dim = |a: &Array, i: Option<usize>| i.map_or(1, |i| a.shape[i]);
    let a0 = &arrays[0];
    let (w, h) = (a0.shape[axes.x], a0.shape[axes.y]);
    let (w, h) = (
        u32::try_from(w).map_err(|_| Error::TooLarge)?,
        u32::try_from(h).map_err(|_| Error::TooLarge)?,
    );
    if w == 0 || h == 0 {
        return Err(Error::Malformed("empty Zarr image".into()));
    }
    // Levels must not grow; drop any that do.
    let mut keep = vec![0usize];
    for (i, a) in arrays.iter().enumerate().skip(1) {
        let prev = &arrays[*keep.last().unwrap_or(&0)];
        if a.shape[axes.x] < prev.shape[axes.x] || a.shape[axes.y] < prev.shape[axes.y] {
            keep.push(i);
        }
    }
    let arrays: Vec<Array> = keep.iter().map(|&i| arrays[i].clone()).collect();
    let multiscale_scale = scales(ms.get("coordinateTransformations"), ndim);
    let s0 = level_scales[0].clone().map(|s| match &multiscale_scale {
        Some(m) => s.iter().zip(m).map(|(a, b)| a * b).collect(),
        None => s,
    });
    let size_c = u32::try_from(dim(a0, axes.c)).map_err(|_| Error::TooLarge)?;
    let size_z = u32::try_from(dim(a0, axes.z)).map_err(|_| Error::TooLarge)?;
    let size_t = u32::try_from(dim(a0, axes.t)).map_err(|_| Error::TooLarge)?;
    let omero = attrs.get("omero");
    let ochan = omero
        .and_then(|o| o.get("channels"))
        .and_then(Value::as_array);
    let channels: Vec<Channel> = (0..size_c.min(MAX_CHANNELS as u32) as usize)
        .map(|k| {
            let c = ochan.and_then(|a| a.get(k));
            let win = c.and_then(|c| c.get("window"));
            let num = |n| win.and_then(|w| w.get(n)).and_then(Value::as_f64);
            Channel {
                name: c
                    .and_then(|c| c.get("label"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                colour: c
                    .and_then(|c| c.get("color"))
                    .and_then(Value::as_str)
                    .and_then(hex_colour),
                window: match (num("start"), num("end")) {
                    (Some(a), Some(b)) if b > a => Some((a, b)),
                    _ => None,
                },
            }
        })
        .collect();
    let mut props = Props::new(Vendor::OmeZarr);
    let unit_scale = |i: usize, f: fn(Option<&str>) -> Option<f64>| {
        let s = s0.as_ref()?.get(i).copied()?;
        let u = axes.units.get(i).cloned().flatten();
        // Without a unit, NGFF spatial scales are in micrometres by convention of the writers.
        Some(s * f(u.as_deref()).unwrap_or(1.0)).filter(|v| *v > 0.0 && v.is_finite())
    };
    let mx = unit_scale(axes.x, crate::ome::microns);
    let my = unit_scale(axes.y, crate::ome::microns);
    props.mpp = mx.map(|x| (x, my.unwrap_or(x)));
    props.z_step = axes.z.and_then(|z| unit_scale(z, crate::ome::microns));
    props.t_step = axes.t.and_then(|t| unit_scale(t, crate::ome::seconds));
    props.name = ms
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| omero.and_then(|o| o.get("name")).and_then(Value::as_str))
        .map(str::to_owned)
        .filter(|s| !s.is_empty());
    props.sample = format!(
        "{}, {}",
        a0.dtype.name(),
        if a0.path.is_empty() {
            "Zarr"
        } else if json(store.as_ref(), &format!("{}zarr.json", a0.path)).is_some() {
            "Zarr v3"
        } else {
            "Zarr v2"
        }
    );
    if let Some(v) = ms
        .get("version")
        .and_then(Value::as_str)
        .or_else(|| attrs.get("version").and_then(Value::as_str))
    {
        props
            .properties
            .push(("NGFF version".to_owned(), v.to_owned()));
    }
    let levels = arrays
        .iter()
        .map(|a| {
            let lw = a.shape[axes.x] as u32;
            Level {
                width: lw,
                height: a.shape[axes.y] as u32,
                downsample: f64::from(w) / f64::from(lw.max(1)),
            }
        })
        .collect();
    Ok(Slide {
        props,
        width: w,
        height: h,
        levels,
        channels: channels.clone(),
        size_z,
        size_t,
        associated: Vec::new(),
        files: Vec::new(),
        backend: crate::Backend::Zarr(ZarrSlide {
            store,
            arrays,
            axes,
            channels,
            windows: Mutex::new(None),
            indexes: Indexes::default(),
        }),
    })
}

impl ZarrSlide {
    /// The array to read a 1/`scale` downscale from, with the remaining factor.
    fn choose(&self, scale: u32) -> (usize, u32) {
        let (w, h) = (
            self.arrays[0].shape[self.axes.x],
            self.arrays[0].shape[self.axes.y],
        );
        let mut best = (0, scale);
        for (i, a) in self.arrays.iter().enumerate().skip(1) {
            let (lw, lh) = (a.shape[self.axes.x], a.shape[self.axes.y]);
            let mut f = 2u64;
            while f <= u64::from(scale) {
                let (vw, vh) = (w.div_ceil(f), h.div_ceil(f));
                if lw.abs_diff(vw) <= 1.max(vw / 1000)
                    && lh.abs_diff(vh) <= 1.max(vh / 1000)
                    && scale / f as u32 <= best.1
                {
                    best = (i, scale / f as u32);
                }
                f *= 2;
            }
        }
        best
    }

    pub(crate) fn read_channel(&self, channel: u32, o: &ReadOptions) -> Result<Plane> {
        let r = o
            .region
            .ok_or_else(|| Error::Malformed("no region".into()))?;
        let (ai, f) = self.choose(o.scale);
        let a = &self.arrays[ai];
        let ax = &self.axes;
        let (lw, lh) = (a.shape[ax.x] as u32, a.shape[ax.y] as u32);
        let lr = Region::clip(
            Some(Region {
                x: r.x.saturating_mul(f),
                y: r.y.saturating_mul(f),
                w: r.w.saturating_mul(f),
                h: r.h.saturating_mul(f),
            }),
            lw,
            lh,
        )
        .ok_or_else(|| Error::Malformed("region outside the level".into()))?;
        // Snap to the downscale's grid.
        let x0 = lr.x / f * f;
        let y0 = lr.y / f * f;
        let x1 = (lr.x + lr.w).div_ceil(f).saturating_mul(f).min(lw);
        let y1 = (lr.y + lr.h).div_ceil(f).saturating_mul(f).min(lh);
        let (pw, ph) = ((x1 - x0) as usize, (y1 - y0) as usize);
        if pw as u64 * ph as u64 > crate::MAX_READ_PIXELS {
            return Err(Error::TooLarge);
        }
        let cs = a.read_chunk_shape().to_vec();
        let ndim = a.shape.len();
        // The fixed coordinates of the other axes.
        let mut fixed = vec![0u64; ndim];
        let clampi = |i: Option<usize>, v: u32| {
            i.map(|i| (i, u64::from(v).min(a.shape[i].saturating_sub(1))))
        };
        for (i, v) in [
            clampi(ax.c, channel),
            clampi(ax.z, o.position.z),
            clampi(ax.t, o.position.t),
        ]
        .into_iter()
        .flatten()
        {
            fixed[i] = v;
        }
        let (cw, ch) = (cs[ax.x].max(1), cs[ax.y].max(1));
        let ny = u64::from(y1).div_ceil(ch) - u64::from(y0) / ch;
        let nx = u64::from(x1).div_ceil(cw) - u64::from(x0) / cw;
        if nx.saturating_mul(ny) > 1 << 20 {
            return Err(Error::TooLarge);
        }
        let jobs: Vec<(u64, u64)> = (u64::from(y0) / ch..u64::from(y1).div_ceil(ch))
            .flat_map(|cy| (u64::from(x0) / cw..u64::from(x1).div_ceil(cw)).map(move |cx| (cy, cx)))
            .collect();
        let mut values = vec![a.fill; pw * ph];
        let big = a.big_endian();
        // Strides of a chunk in C order.
        let mut strides = vec![1usize; ndim];
        for i in (0..ndim.saturating_sub(1)).rev() {
            strides[i] = strides[i + 1] * cs[i + 1] as usize;
        }
        // Workers reserve from this thread's budget.
        let budget = specular_core::current_budget();
        let index_of = |cy: u64, cx: u64| -> Vec<u64> {
            (0..ndim)
                .map(|i| {
                    if i == ax.x {
                        cx
                    } else if i == ax.y {
                        cy
                    } else {
                        fixed[i] / cs[i].max(1)
                    }
                })
                .collect()
        };
        // A remote store fetches the region's chunks several at a time before decoding.
        if self.store.batched() {
            let idxs: Vec<Vec<u64>> = jobs.iter().map(|&(cy, cx)| index_of(cy, cx)).collect();
            a.prefetch(self.store.as_ref(), &idxs, o.cancel, &self.indexes)?;
        }
        let load = |&(cy, cx): &(u64, u64)| -> Result<(u64, u64, Option<Decoded>, bool)> {
            crate::check_cancel(o.cancel)?;
            let idx = index_of(cy, cx);
            match a.chunk(self.store.as_ref(), &idx, &budget, o.cancel, &self.indexes) {
                Ok(c) => Ok((cy, cx, c, false)),
                Err(
                    e @ (Error::Cancelled | Error::TooLarge | Error::NeedsMemory(_) | Error::Io(_)),
                ) => Err(e),
                Err(_) => Ok((cy, cx, None, true)),
            }
        };
        let chunks: Vec<_> = if o.parallel {
            jobs.par_iter().map(load).collect::<Result<_>>()?
        } else {
            jobs.iter().map(load).collect::<Result<_>>()?
        };
        let mut truncated = false;
        let base: usize = (0..ndim)
            .filter(|&i| i != ax.x && i != ax.y)
            .map(|i| (fixed[i] % cs[i].max(1)) as usize * strides[i])
            .sum();
        for (cy, cx, data, bad) in chunks {
            truncated |= bad;
            let Some((data, _reserved)) = data else {
                continue;
            };
            let (gx0, gy0) = (cx * cw, cy * ch);
            let ys = gy0.max(u64::from(y0))..(gy0 + ch).min(u64::from(y1));
            let xs = gx0.max(u64::from(x0))..(gx0 + cw).min(u64::from(x1));
            for gy in ys {
                let row = (gy - u64::from(y0)) as usize * pw;
                for gx in xs.clone() {
                    let k = base
                        + (gy - gy0) as usize * strides[ax.y]
                        + (gx - gx0) as usize * strides[ax.x];
                    values[row + (gx - u64::from(x0)) as usize] = a.dtype.get(&data, k, big);
                }
            }
        }
        let plane = Plane {
            width: pw as u32,
            height: ph as u32,
            values,
            truncated,
        };
        let plane = composite::box_reduce_plane(&plane, f);
        // Crop to the region (the snap only extends up and left by under one output pixel).
        let (ox, oy) = (x0 / f, y0 / f);
        let mut out = vec![f32::NAN; r.w as usize * r.h as usize];
        for y in 0..r.h {
            let sy = y + r.y - oy;
            if sy >= plane.height {
                break;
            }
            let sx0 = r.x - ox;
            let n = (plane.width.saturating_sub(sx0)).min(r.w) as usize;
            let s = (sy * plane.width + sx0) as usize;
            out[y as usize * r.w as usize..][..n].copy_from_slice(&plane.values[s..s + n]);
        }
        Ok(Plane {
            width: r.w,
            height: r.h,
            values: out,
            truncated: plane.truncated,
        })
    }

    fn windows(&self, slide: &Slide, o: &ReadOptions) -> Result<Vec<(f64, f64)>> {
        if let Some(w) = self.windows.lock().clone() {
            return Ok(w);
        }
        let mut scale = 1u32;
        while scale < 1 << 16 && (slide.width / scale > 1024 || slide.height / scale > 1024) {
            scale *= 2;
        }
        let dt = self.arrays[0].dtype;
        let mut out = Vec::new();
        for (c, ch) in self.channels.iter().enumerate() {
            if let Some(w) = ch.window {
                out.push(w);
                continue;
            }
            let p = self.read_channel(
                c as u32,
                &ReadOptions {
                    scale,
                    region: Region::clip(
                        None,
                        slide.width.div_ceil(scale),
                        slide.height.div_ceil(scale),
                    ),
                    ..*o
                },
            )?;
            out.push(if dt == Dtype::U8 && self.channels.len() == 3 {
                (0.0, 255.0)
            } else {
                composite::auto_window(&p.values)
            });
        }
        *self.windows.lock() = Some(out.clone());
        Ok(out)
    }

    pub(crate) fn read(&self, slide: &Slide, o: &ReadOptions) -> Result<Rgba> {
        let windows = self.windows(slide, o)?;
        let planes = (0..self.channels.len().max(1))
            .map(|c| self.read_channel(c as u32, o))
            .collect::<Result<Vec<_>>>()?;
        Ok(composite::composite(&planes, &self.channels, &windows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// An in-memory store.
    pub(crate) struct Mem(pub HashMap<String, Vec<u8>>);
    impl Store for Mem {
        fn get(&self, key: &str) -> Option<Vec<u8>> {
            self.0.get(key).cloned()
        }
    }

    fn v2_store(compressor: &str) -> Mem {
        let mut m = HashMap::new();
        m.insert(
            ".zattrs".into(),
            br#"{"multiscales":[{"version":"0.4","axes":[{"name":"c","type":"channel"},{"name":"y","type":"space","unit":"micrometer"},{"name":"x","type":"space","unit":"micrometer"}],
            "datasets":[{"path":"0","coordinateTransformations":[{"type":"scale","scale":[1,0.5,0.5]}]},{"path":"1","coordinateTransformations":[{"type":"scale","scale":[1,1,1]}]}]}],
            "omero":{"channels":[{"label":"A","color":"FF0000","window":{"start":0,"end":100}},{"label":"B","color":"00FF00","window":{"start":0,"end":100}}]}}"#
                .to_vec(),
        );
        m.insert(".zgroup".into(), br#"{"zarr_format":2}"#.to_vec());
        for (lvl, n) in [("0", 8u64), ("1", 4)] {
            m.insert(
                format!("{lvl}/.zarray"),
                format!(r#"{{"zarr_format":2,"shape":[2,{n},{n}],"chunks":[1,3,3],"dtype":"<u2","compressor":{compressor},"fill_value":0,"order":"C","filters":null,"dimension_separator":"/"}}"#).into_bytes(),
            );
            for c in 0..2u64 {
                for cy in 0..n.div_ceil(3) {
                    for cx in 0..n.div_ceil(3) {
                        let mut v = Vec::new();
                        for y in 0..3 {
                            for x in 0..3 {
                                let val = c * 1000 + (cy * 3 + y) * 10 + cx * 3 + x;
                                v.extend_from_slice(&(val as u16).to_le_bytes());
                            }
                        }
                        let v = if compressor.contains("gzip") {
                            gzip(&v)
                        } else {
                            v
                        };
                        m.insert(format!("{lvl}/{c}/{cy}/{cx}"), v);
                    }
                }
            }
        }
        Mem(m)
    }

    /// A stored-block gzip stream.
    fn gzip(d: &[u8]) -> Vec<u8> {
        let mut o = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
        o.push(1);
        o.extend_from_slice(&(d.len() as u16).to_le_bytes());
        o.extend_from_slice(&(!(d.len() as u16)).to_le_bytes());
        o.extend_from_slice(d);
        let crc = crc32(d);
        o.extend_from_slice(&crc.to_le_bytes());
        o.extend_from_slice(&(d.len() as u32).to_le_bytes());
        o
    }

    fn crc32(d: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in d {
            c ^= u32::from(b);
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

    /// Decoded chunks are held until the region is put together, on the read's parallel
    /// workers: they reserve from the reading thread's budget, and a refusal says so.
    #[test]
    fn chunks_count_against_the_budget() {
        let s = open(Arc::new(v2_store("null"))).unwrap();
        let crate::Backend::Zarr(z) = &s.backend else {
            panic!()
        };
        let read = || {
            z.read_channel(
                0,
                &ReadOptions {
                    scale: 1,
                    parallel: true,
                    region: Some(Region {
                        x: 0,
                        y: 0,
                        w: 8,
                        h: 8,
                    }),
                    ..Default::default()
                },
            )
        };
        // 9 chunks of 18 bytes.
        let r = specular_core::budget::with_limit(100, read);
        assert!(matches!(r, Err(Error::NeedsMemory(_))), "{:?}", r.err());
        assert!(specular_core::budget::with_limit(1 << 20, read).is_ok());
    }

    #[test]
    fn reads_v2_multiscales() {
        for comp in ["null", r#"{"id":"gzip","level":1}"#] {
            let s = open(Arc::new(v2_store(comp))).unwrap();
            assert_eq!((s.width, s.height, s.channels.len()), (8, 8, 2));
            assert_eq!(s.props.mpp, Some((0.5, 0.5)));
            assert_eq!(s.channels[1].colour, Some([0, 255, 0]));
            assert_eq!(s.levels.len(), 2);
            let crate::Backend::Zarr(z) = &s.backend else {
                panic!()
            };
            let p = z
                .read_channel(
                    1,
                    &ReadOptions {
                        scale: 1,
                        region: Some(Region {
                            x: 2,
                            y: 3,
                            w: 3,
                            h: 2,
                        }),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(
                p.values,
                vec![1032.0, 1033.0, 1034.0, 1042.0, 1043.0, 1044.0]
            );
            // Scale 2 comes from level 1 as stored.
            let p = z
                .read_channel(
                    0,
                    &ReadOptions {
                        scale: 2,
                        region: Some(Region {
                            x: 0,
                            y: 0,
                            w: 4,
                            h: 4,
                        }),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(p.values[5], 11.0);
            // Scale 4 box-reduces level 1.
            let p = z
                .read_channel(
                    0,
                    &ReadOptions {
                        scale: 4,
                        region: Some(Region {
                            x: 0,
                            y: 0,
                            w: 2,
                            h: 2,
                        }),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(p.values[0], 5.5);
            let rgba = s
                .read(
                    crate::Files { data: &[] },
                    &ReadOptions {
                        scale: 1,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!((rgba.width, rgba.height), (8, 8));
        }
    }

    #[test]
    fn reads_v3_sharded() {
        let mut m = HashMap::new();
        m.insert(
            "zarr.json".into(),
            br#"{"zarr_format":3,"node_type":"group","attributes":{"ome":{"version":"0.5","multiscales":[{"axes":[{"name":"y","type":"space"},{"name":"x","type":"space"}],"datasets":[{"path":"s0","coordinateTransformations":[{"type":"scale","scale":[2,2]}]}]}]}}}"#.to_vec(),
        );
        m.insert(
            "s0/zarr.json".into(),
            br#"{"zarr_format":3,"node_type":"array","shape":[4,4],"data_type":"uint8","chunk_grid":{"name":"regular","configuration":{"chunk_shape":[4,4]}},
            "chunk_key_encoding":{"name":"default","configuration":{"separator":"/"}},"fill_value":7,
            "codecs":[{"name":"sharding_indexed","configuration":{"chunk_shape":[2,2],"codecs":[{"name":"bytes"}],"index_codecs":[{"name":"bytes","configuration":{"endian":"little"}},{"name":"crc32c"}],"index_location":"end"}}]}"#.to_vec(),
        );
        // Four inner 2x2 chunks; the last one is absent.
        let mut shard = Vec::new();
        let mut index = Vec::new();
        for k in 0..4u8 {
            if k == 3 {
                index.extend_from_slice(&u64::MAX.to_le_bytes());
                index.extend_from_slice(&u64::MAX.to_le_bytes());
                continue;
            }
            index.extend_from_slice(&(shard.len() as u64).to_le_bytes());
            index.extend_from_slice(&4u64.to_le_bytes());
            shard.extend_from_slice(&[k * 10, k * 10 + 1, k * 10 + 2, k * 10 + 3]);
        }
        shard.extend_from_slice(&index);
        shard.extend_from_slice(&[0; 4]);
        m.insert("s0/c/0/0".into(), shard);
        let s = open(Arc::new(Mem(m))).unwrap();
        assert_eq!(s.props.mpp, Some((2.0, 2.0)));
        let crate::Backend::Zarr(z) = &s.backend else {
            panic!()
        };
        let p = z
            .read_channel(
                0,
                &ReadOptions {
                    scale: 1,
                    region: Some(Region {
                        x: 0,
                        y: 0,
                        w: 4,
                        h: 4,
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            p.values,
            vec![
                0., 1., 10., 11., 2., 3., 12., 13., 20., 21., 7., 7., 22., 23., 7., 7.
            ]
        );
    }

    #[test]
    fn zip_store_and_damage() {
        let m = v2_store("null");
        let mut files: Vec<(String, Vec<u8>)> =
            m.0.iter()
                .map(|(k, v)| (format!("img.ome.zarr/{k}"), v.clone()))
                .collect();
        files.sort();
        let refs: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_slice()))
            .collect();
        let z = crate::zip::tests::stored_zip(&refs);
        let st = ZipStore::new(z.clone()).unwrap();
        let s = open(Arc::new(st)).unwrap();
        assert_eq!(s.width, 8);
        // Damaged chunks read as fill, flagged truncated; no panics on any truncation.
        let mut m = v2_store("null");
        m.0.insert("0/0/0/0".into(), vec![1, 2, 3]);
        m.0.remove("0/1/1/1");
        let s = open(Arc::new(m)).unwrap();
        let crate::Backend::Zarr(zs) = &s.backend else {
            panic!()
        };
        let p = zs
            .read_channel(
                0,
                &ReadOptions {
                    scale: 1,
                    region: Some(Region {
                        x: 0,
                        y: 0,
                        w: 8,
                        h: 8,
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(p.values.len(), 64);
        for n in (0..z.len()).step_by(7) {
            if let Ok(st) = ZipStore::new(z[..n].to_vec()) {
                let _ = open(Arc::new(st));
            }
        }
    }

    mod hostile {
        use super::*;

        /// A one-level v3 image whose array has `shape`, `chunks` and `codecs`.
        fn v3(shape: &str, chunks: &str, codecs: &str) -> Mem {
            let mut m = HashMap::new();
            m.insert(
                "zarr.json".into(),
                br#"{"zarr_format":3,"node_type":"group","attributes":{"ome":{"version":"0.5","multiscales":[{"axes":[{"name":"y","type":"space"},{"name":"x","type":"space"}],"datasets":[{"path":"s0"}]}]}}}"#.to_vec(),
            );
            m.insert(
                "s0/zarr.json".into(),
                format!(r#"{{"zarr_format":3,"node_type":"array","shape":{shape},"data_type":"uint8","chunk_grid":{{"name":"regular","configuration":{{"chunk_shape":{chunks}}}}},"fill_value":0,"codecs":{codecs}}}"#).into_bytes(),
            );
            Mem(m)
        }

        fn sharded(inner: &str) -> String {
            format!(
                r#"[{{"name":"sharding_indexed","configuration":{{"chunk_shape":{inner},"codecs":[{{"name":"bytes"}}],"index_codecs":[{{"name":"bytes"}}]}}}}]"#
            )
        }

        fn read_all(s: &Slide) -> Result<Plane> {
            let crate::Backend::Zarr(z) = &s.backend else {
                panic!()
            };
            z.read_channel(
                0,
                &ReadOptions {
                    scale: 1,
                    region: Some(Region {
                        x: 0,
                        y: 0,
                        w: s.width,
                        h: s.height,
                    }),
                    ..Default::default()
                },
            )
        }

        #[test]
        fn zero_chunk_dimension() {
            // An empty outer shard once divided by zero; an empty chunk read past its data.
            let bytes = r#"[{"name":"bytes"}]"#;
            assert!(open(Arc::new(v3("[4,4]", "[0,4]", &sharded("[2,2]")))).is_err());
            assert!(open(Arc::new(v3("[4,4]", "[4,4]", &sharded("[0,2]")))).is_err());
            assert!(open(Arc::new(v3("[4,4]", "[0,4]", bytes))).is_err());
            // Inner chunks of another rank than the shard.
            assert!(open(Arc::new(v3("[4,4]", "[4,4]", &sharded("[2]")))).is_err());
        }

        #[test]
        fn chunk_size_overflows() {
            // 2^32 x 2^32 bytes once overflowed the chunk size and the strides.
            let huge = "[4294967296,4294967296]";
            assert!(open(Arc::new(v3("[4,4]", huge, r#"[{"name":"bytes"}]"#))).is_err());
            assert!(open(Arc::new(v3("[4,4]", huge, &sharded(huge)))).is_err());
        }

        #[test]
        fn shard_index_overflows() {
            // 2^40 x 2^40 one-byte inner chunks per shard once overflowed the index size.
            let mut m = v3("[4,4]", "[1099511627776,1099511627776]", &sharded("[1,1]"));
            m.0.insert("s0/c/0/0".into(), vec![0; 64]);
            let s = open(Arc::new(m)).unwrap();
            assert!(read_all(&s).unwrap().truncated);
        }

        #[test]
        fn chunk_count_checked_before_listing() {
            // 2048 x 2048 one-pixel chunks: refused before 4M jobs are listed.
            let s = open(Arc::new(v3(
                "[2048,2048]",
                "[1,1]",
                r#"[{"name":"bytes"}]"#,
            )))
            .unwrap();
            assert!(matches!(read_all(&s), Err(Error::TooLarge)));
        }
    }
}
