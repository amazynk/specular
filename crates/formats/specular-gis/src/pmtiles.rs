//! PMTiles version 3: a single-file tile pyramid read by byte ranges, locally or from an
//! HTTP server (the reader asks a [`RangeRead`] for bytes; `specular-remote` supplies one
//! over HTTP).
//!
//! Layout: a 127-byte header, a root directory, JSON metadata, leaf directories and the
//! tile data. Tiles are numbered along a Hilbert curve per zoom level; a directory lists runs
//! of tile ids with their offsets and lengths, or points to a leaf directory. Directories
//! and metadata may be gzip, Brotli or Zstandard compressed, and so may tiles.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use crate::tiles::{self, TILE_LIMIT, TileFormat, TileMeta, TileStore};

/// Bytes the header takes.
pub const HEADER_LEN: usize = 127;
/// Directory levels followed at most (the format needs at most four).
const MAX_DEPTH: usize = 4;
/// Entries a directory may hold.
const MAX_ENTRIES: u64 = 1 << 24;
/// Directories kept parsed (each holds up to a few thousand entries).
const CACHE: usize = 64;

/// Whether the first bytes are a PMTiles v3 archive.
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(b"PMTiles") && head.get(7) == Some(&3)
}

/// Bytes by range: a file in memory or on a server.
pub trait RangeRead: Send + Sync {
    fn read_range(&self, offset: u64, len: u64) -> Result<Vec<u8>, String>;
}

impl RangeRead for Arc<[u8]> {
    fn read_range(&self, offset: u64, len: u64) -> Result<Vec<u8>, String> {
        slice(self, offset, len).map(<[u8]>::to_vec)
    }
}

impl RangeRead for Vec<u8> {
    fn read_range(&self, offset: u64, len: u64) -> Result<Vec<u8>, String> {
        slice(self, offset, len).map(<[u8]>::to_vec)
    }
}

/// A memory-mapped or borrowed file kept alive by its owner.
pub struct Bytes<T: AsRef<[u8]> + Send + Sync>(pub T);

impl<T: AsRef<[u8]> + Send + Sync> RangeRead for Bytes<T> {
    fn read_range(&self, offset: u64, len: u64) -> Result<Vec<u8>, String> {
        slice(self.0.as_ref(), offset, len).map(<[u8]>::to_vec)
    }
}

fn slice(b: &[u8], offset: u64, len: u64) -> Result<&[u8], String> {
    let (Ok(o), Ok(l)) = (usize::try_from(offset), usize::try_from(len)) else {
        return Err("PMTiles offset out of range".into());
    };
    o.checked_add(l)
        .and_then(|e| b.get(o..e))
        .ok_or_else(|| "PMTiles section past the end of the file".into())
}

/// Compression codes of the header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    Unknown,
    None,
    Gzip,
    Brotli,
    Zstd,
}

impl Compression {
    fn from_byte(b: u8) -> Compression {
        match b {
            1 => Compression::None,
            2 => Compression::Gzip,
            3 => Compression::Brotli,
            4 => Compression::Zstd,
            _ => Compression::Unknown,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Compression::Unknown => "unknown",
            Compression::None => "none",
            Compression::Gzip => "gzip",
            Compression::Brotli => "Brotli",
            Compression::Zstd => "Zstandard",
        }
    }

    fn undo(self, b: Vec<u8>, limit: usize) -> Result<Vec<u8>, String> {
        match self {
            Compression::None => Ok(b),
            // Unknown: gzip when it looks like gzip (writers that leave the byte at 0).
            Compression::Unknown if !b.starts_with(&[0x1F, 0x8B]) => Ok(b),
            Compression::Gzip | Compression::Unknown => tiles::gunzip(&b, limit),
            Compression::Brotli => specular_brotli::decompress(&b, limit)
                .map_err(|e| format!("Brotli-compressed PMTiles data: {e}")),
            Compression::Zstd => {
                use std::io::Read;
                let mut out = Vec::new();
                ruzstd::decoding::StreamingDecoder::new(&b[..])
                    .map_err(|e| format!("Zstandard-compressed PMTiles data: {e}"))?
                    .take(limit as u64 + 1)
                    .read_to_end(&mut out)
                    .map_err(|e| format!("Zstandard-compressed PMTiles data: {e}"))?;
                if out.len() > limit {
                    return Err("PMTiles data larger than allowed".into());
                }
                Ok(out)
            }
        }
    }
}

/// The header.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub root: (u64, u64),
    pub metadata: (u64, u64),
    pub leaves: (u64, u64),
    pub data: (u64, u64),
    pub addressed_tiles: u64,
    pub tile_entries: u64,
    pub tile_contents: u64,
    pub clustered: bool,
    pub internal: Compression,
    pub tile_compression: Compression,
    pub tile_type: TileFormat,
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// West, south, east, north (degrees).
    pub bounds: [f64; 4],
    pub center: (u8, f64, f64),
}

pub fn parse_header(b: &[u8]) -> Result<Header, String> {
    if b.len() < HEADER_LEN || !sniff(b) {
        return Err(match b.get(7) {
            Some(&v) if b.starts_with(b"PMTiles") => {
                format!("PMTiles version {v}: only version 3 is supported")
            }
            _ => "not a PMTiles archive".into(),
        });
    }
    let u64_at = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().expect("8 bytes"));
    let deg = |o: usize| f64::from(i32::from_le_bytes(b[o..o + 4].try_into().expect("4"))) / 1e7;
    let tile_type = match b[99] {
        1 => TileFormat::Mvt,
        2 => TileFormat::Png,
        3 => TileFormat::Jpeg,
        4 => TileFormat::WebP,
        5 => TileFormat::Avif,
        _ => TileFormat::Unknown,
    };
    let mut bounds = [deg(102), deg(106), deg(110), deg(114)];
    if !(bounds[0] < bounds[2] && bounds[1] < bounds[3]) {
        bounds = tiles::WORLD;
    }
    Ok(Header {
        root: (u64_at(8), u64_at(16)),
        metadata: (u64_at(24), u64_at(32)),
        leaves: (u64_at(40), u64_at(48)),
        data: (u64_at(56), u64_at(64)),
        addressed_tiles: u64_at(72),
        tile_entries: u64_at(80),
        tile_contents: u64_at(88),
        clustered: b[96] == 1,
        internal: Compression::from_byte(b[97]),
        tile_compression: Compression::from_byte(b[98]),
        tile_type,
        min_zoom: b[100],
        max_zoom: b[101].max(b[100]),
        bounds: [
            bounds[0].max(-180.0),
            bounds[1].max(tiles::WORLD[1]),
            bounds[2].min(180.0),
            bounds[3].min(tiles::WORLD[3]),
        ],
        center: (b[118], deg(119), deg(123)),
    })
}

/// The tile id of z/x/y: the tiles of lower zooms, then the Hilbert index.
pub fn tile_id(z: u8, x: u32, y: u32) -> u64 {
    let z = u32::from(z.min(31));
    let acc = ((1u64 << (2 * z)) - 1) / 3;
    let n = 1u64 << z;
    let (mut x, mut y) = (u64::from(x), u64::from(y));
    let mut d = 0u64;
    let mut s = n / 2;
    while s > 0 {
        let rx = u64::from(x & s > 0);
        let ry = u64::from(y & s > 0);
        d += s * s * ((3 * rx) ^ ry);
        if ry == 0 {
            if rx == 1 {
                x = n - 1 - x;
                y = n - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        s /= 2;
    }
    acc + d
}

/// z/x/y of a tile id (the inverse of [`tile_id`]).
pub fn tile_zxy(id: u64) -> (u8, u32, u32) {
    let mut acc = 0u64;
    for z in 0..32u32 {
        let count = 1u64 << (2 * z);
        if id < acc + count {
            let n = 1u64 << z;
            let mut t = id - acc;
            let (mut x, mut y) = (0u64, 0u64);
            let mut s = 1u64;
            while s < n {
                let rx = 1 & (t / 2);
                let ry = 1 & (t ^ rx);
                if ry == 0 {
                    if rx == 1 {
                        x = s - 1 - x;
                        y = s - 1 - y;
                    }
                    std::mem::swap(&mut x, &mut y);
                }
                x += s * rx;
                y += s * ry;
                t /= 4;
                s *= 2;
            }
            return (z as u8, x as u32, y as u32);
        }
        acc += count;
    }
    (31, 0, 0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub tile_id: u64,
    pub offset: u64,
    pub length: u64,
    /// 0: a leaf directory.
    pub run_length: u64,
}

fn varint(b: &[u8], i: &mut usize) -> Result<u64, String> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*i).ok_or("PMTiles directory truncated")?;
        *i += 1;
        v |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err("PMTiles directory has a malformed number".into())
}

/// A directory's entries (uncompressed bytes).
pub fn parse_directory(b: &[u8]) -> Result<Vec<Entry>, String> {
    let mut i = 0;
    let n = varint(b, &mut i)?;
    if n > MAX_ENTRIES || n > b.len() as u64 {
        return Err("PMTiles directory lists more entries than it holds".into());
    }
    let n = n as usize;
    let mut e = vec![
        Entry {
            tile_id: 0,
            offset: 0,
            length: 0,
            run_length: 0,
        };
        n
    ];
    let mut last = 0u64;
    for x in &mut e {
        last = last
            .checked_add(varint(b, &mut i)?)
            .ok_or("PMTiles tile id overflows")?;
        x.tile_id = last;
    }
    for x in &mut e {
        x.run_length = varint(b, &mut i)?;
    }
    for x in &mut e {
        x.length = varint(b, &mut i)?;
    }
    for k in 0..n {
        let v = varint(b, &mut i)?;
        e[k].offset = if v == 0 && k > 0 {
            e[k - 1].offset.saturating_add(e[k - 1].length)
        } else {
            v.saturating_sub(1)
        };
    }
    Ok(e)
}

/// The entry covering `id`: the last with tile_id ≤ id.
fn find(entries: &[Entry], id: u64) -> Option<&Entry> {
    let i = entries.partition_point(|e| e.tile_id <= id);
    let e = entries.get(i.checked_sub(1)?)?;
    (e.run_length == 0 || id < e.tile_id + e.run_length).then_some(e)
}

/// Leaf directories read, by (offset, length).
type LeafCache = HashMap<(u64, u64), Arc<Vec<Entry>>>;

/// A PMTiles archive.
pub struct PmTiles {
    src: Box<dyn RangeRead>,
    pub header: Header,
    meta: TileMeta,
    root: Vec<Entry>,
    /// Leaf directories by (offset, length).
    leaves: Mutex<LeafCache>,
}

impl PmTiles {
    /// Reads the header, root directory and metadata (one range for the first 16 KiB, as
    /// writers put the root directory there, then the metadata).
    pub fn open(src: Box<dyn RangeRead>) -> Result<PmTiles, String> {
        let head = src.read_range(0, HEADER_LEN as u64)?;
        let header = parse_header(&head)?;
        if header.root.1 > 16 << 20 || header.metadata.1 > 16 << 20 {
            return Err("PMTiles root directory or metadata over 16 MB".into());
        }
        let root_raw = src.read_range(header.root.0, header.root.1)?;
        let root = parse_directory(&header.internal.undo(root_raw, 64 << 20)?)?;
        let mut meta = TileMeta {
            container: "PMTiles",
            format: header.tile_type,
            min_zoom: header.min_zoom,
            max_zoom: header.max_zoom,
            bounds: header.bounds,
            tiles: Some(header.addressed_tiles),
            ..TileMeta::default()
        };
        if header.metadata.1 > 0 {
            let raw = src.read_range(header.metadata.0, header.metadata.1)?;
            if let Ok(text) = header.internal.undo(raw, 64 << 20)
                && let Ok(serde_json::Value::Object(m)) = serde_json::from_slice(&text)
            {
                for (k, v) in &m {
                    let s = v.as_str().map(str::to_string);
                    match k.as_str() {
                        "name" => meta.name = s,
                        "description" => meta.description = s.map(|d| tiles::plain_text(&d)),
                        "attribution" => meta.attribution = s.map(|d| tiles::plain_text(&d)),
                        "vector_layers" => {
                            meta.vector_layers = tiles::vector_layer_names(
                                &serde_json::json!({ "vector_layers": v }).to_string(),
                            );
                        }
                        _ => {
                            if let Some(s) = s.filter(|s| s.len() < 500) {
                                meta.extra.push((k.clone(), s));
                            }
                        }
                    }
                }
            }
        }
        Ok(PmTiles {
            src,
            header,
            meta,
            root,
            leaves: Mutex::new(HashMap::new()),
        })
    }

    fn leaf(&self, offset: u64, length: u64) -> Result<Arc<Vec<Entry>>, String> {
        let key = (offset, length);
        if let Some(d) = self
            .leaves
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return Ok(d.clone());
        }
        if length > 64 << 20 {
            return Err("PMTiles leaf directory over 64 MB".into());
        }
        let raw = self
            .src
            .read_range(self.header.leaves.0.saturating_add(offset), length)?;
        let d = Arc::new(parse_directory(
            &self.header.internal.undo(raw, 256 << 20)?,
        )?);
        let mut cache = self.leaves.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.len() >= CACHE {
            cache.clear();
        }
        cache.insert(key, d.clone());
        Ok(d)
    }

    /// Where tile `id` is in the tile data section: (offset, length).
    pub fn locate(&self, id: u64) -> Result<Option<(u64, u64)>, String> {
        let mut leaf: Option<Arc<Vec<Entry>>> = None;
        for _ in 0..MAX_DEPTH {
            let entries: &[Entry] = match &leaf {
                Some(d) => d,
                None => &self.root,
            };
            let Some(e) = find(entries, id).copied() else {
                return Ok(None);
            };
            if e.run_length > 0 {
                return Ok(Some((e.offset, e.length)));
            }
            leaf = Some(self.leaf(e.offset, e.length)?);
        }
        Err("PMTiles directories nest too deep".into())
    }

    /// The tile's bytes as stored (still compressed).
    pub fn raw_tile(&self, z: u8, x: u32, y: u32) -> Result<Option<Vec<u8>>, String> {
        if z < self.header.min_zoom || z > self.header.max_zoom || z > 31 {
            return Ok(None);
        }
        let n = 1u64 << z;
        if u64::from(x) >= n || u64::from(y) >= n {
            return Ok(None);
        }
        let Some((off, len)) = self.locate(tile_id(z, x, y))? else {
            return Ok(None);
        };
        if len as usize > TILE_LIMIT {
            return Err("a PMTiles tile over 64 MB".into());
        }
        self.src
            .read_range(self.header.data.0.saturating_add(off), len)
            .map(Some)
    }
}

impl TileStore for PmTiles {
    fn meta(&self) -> &TileMeta {
        &self.meta
    }

    fn tile(&self, z: u8, x: u32, y: u32) -> Result<Option<Vec<u8>>, String> {
        match self.raw_tile(z, x, y)? {
            Some(b) => self.header.tile_compression.undo(b, TILE_LIMIT).map(Some),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hilbert_ids() {
        assert_eq!(tile_id(0, 0, 0), 0);
        assert_eq!(
            [(0, 0), (0, 1), (1, 1), (1, 0)].map(|(x, y)| tile_id(1, x, y)),
            [1, 2, 3, 4]
        );
        // The spec's examples.
        assert_eq!(tile_id(2, 0, 0), 5);
        assert_eq!(tile_id(12, 3423, 1763), 19_078_479);
        for id in [0u64, 1, 4, 5, 20, 21, 1000, 19_078_479, 123_456_789] {
            let (z, x, y) = tile_zxy(id);
            assert_eq!(tile_id(z, x, y), id, "{id} -> {z}/{x}/{y}");
        }
    }

    #[test]
    fn reads_a_written_archive() {
        for (internal, leaves) in [(2u8, false), (1, true)] {
            let file = crate::fixtures::pmtiles(TileFormat::Png, 3, internal, leaves);
            let p = PmTiles::open(Box::new(file.clone())).unwrap();
            assert_eq!(p.meta().format, TileFormat::Png);
            assert_eq!(p.meta().attribution.as_deref(), Some("© Test contributors"));
            for z in 0..=3u8 {
                for y in 0..(1u32 << z) {
                    for x in 0..(1u32 << z) {
                        let t = p.tile(z, x, y).unwrap().expect("tile");
                        assert_eq!(t, crate::fixtures::tile_png(z, x, y), "{z}/{x}/{y}");
                    }
                }
            }
            assert_eq!(p.tile(4, 0, 0).unwrap(), None);
        }
    }

    #[test]
    fn hostile_directories() {
        // A count far beyond the bytes, and a truncated number.
        assert!(parse_directory(&[0xFF, 0xFF, 0xFF, 0x7F]).is_err());
        assert!(parse_directory(&[0x02, 0x80]).is_err());
        let mut h = vec![0u8; HEADER_LEN];
        h[..7].copy_from_slice(b"PMTiles");
        h[7] = 2;
        assert!(parse_header(&h).unwrap_err().contains("version 2"));
    }
}
