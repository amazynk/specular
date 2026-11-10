//! ASDF (Advanced Scientific Data Format) 1.x: the `#ASDF` header, the YAML tree (read with
//! [`yaml`], a bounded subset reader of our own), binary blocks and ndarrays.
//!
//! [`Asdf::open`] reads the header, the tree and the block headers only; array data are read
//! when asked for ([`Asdf::block_data`], [`NdArray::plane`]), so opening a multi-gigabyte file
//! touches a few kilobytes. Compressed blocks use our own decoders: `zlib` (inflate), `bzp2`
//! (bzip2) and `lz4` (ASDF's chunked LZ4 blocks); any other compression is refused with
//! [`Error::Unsupported`]. Block MD5 checksums are verified only on request
//! ([`Asdf::verify_checksum`]).

use std::borrow::Cow;

pub mod yaml;

mod array;
mod meta;

pub use array::{ArraySource, Dtype, NdArray};
pub use meta::{Meta, approx_wcs_cards, flatten, science_meta};
pub use yaml::{Node, Value, YamlError};

/// The largest tree read, in bytes (inline arrays can make trees big).
pub const MAX_TREE: usize = 256 << 20;
/// The largest block decompressed, in bytes.
pub const MAX_BLOCK: u64 = 1 << 34;
/// Most blocks in a file.
pub const MAX_BLOCKS: usize = 1 << 16;
/// Most arrays collected from a tree.
pub const MAX_ARRAYS: usize = 4096;

const MAGIC: &[u8; 4] = b"\xd3BLK";
const INDEX_MARK: &[u8] = b"#ASDF BLOCK INDEX";
/// How far from the end the block index is looked for.
const INDEX_TAIL: usize = 16 << 20;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("not an ASDF file")]
    NotAsdf,
    #[error("malformed ASDF file: {0}")]
    Malformed(String),
    #[error("unsupported ASDF feature: {0}")]
    Unsupported(String),
    #[error("{0}")]
    Yaml(#[from] YamlError),
    #[error("ASDF data too large")]
    TooLarge,
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Whether `head` starts like an ASDF file (`#ASDF 1.0.0`).
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(b"#ASDF ") && head.get(6).is_some_and(u8::is_ascii_digit)
}

/// One binary block's header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// Offset of the block magic.
    pub offset: u64,
    /// Offset of the data, after the header.
    pub data: u64,
    pub flags: u32,
    /// The compression label: four zero bytes for none, else `zlib`, `bzp2`, `lz4\0`, ...
    pub compression: [u8; 4],
    pub allocated: u64,
    pub used: u64,
    /// Size of the data once decompressed.
    pub data_size: u64,
    /// MD5 of the stored (used) bytes; all zero when not given.
    pub checksum: [u8; 16],
}

impl Block {
    /// The compression label as text, `None` when uncompressed.
    pub fn compression_name(&self) -> Option<String> {
        if self.compression == [0; 4] {
            return None;
        }
        let end = self.compression.iter().position(|&b| b == 0).unwrap_or(4);
        Some(String::from_utf8_lossy(&self.compression[..end]).into_owned())
    }

    /// A streamed block runs to the end of the file.
    pub fn streamed(&self) -> bool {
        self.flags & 1 != 0
    }
}

/// An opened ASDF file: everything but the array data.
#[derive(Clone, Debug)]
pub struct Asdf {
    /// The file format version (`#ASDF 1.0.0`).
    pub version: String,
    /// The standard version (`#ASDF_STANDARD 1.6.0`), when given.
    pub standard: Option<String>,
    pub tree: Value,
    pub blocks: Vec<Block>,
    /// Whether the blocks were found through the block index (else by walking headers).
    pub indexed: bool,
    /// The arrays in the tree, in tree order.
    pub arrays: Vec<NdArray>,
    /// Arrays not usable, with why: (tree path, reason).
    pub skipped: Vec<(String, String)>,
}

impl Asdf {
    /// Reads the header, the tree and the block headers.
    pub fn open(data: &[u8]) -> Result<Asdf> {
        if !sniff(data) {
            return Err(Error::NotAsdf);
        }
        let mut version = String::new();
        let mut standard = None;
        let mut i = 0;
        while data.get(i) == Some(&b'#') {
            let end = data[i..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(data.len(), |p| i + p);
            let line = String::from_utf8_lossy(&data[i..end]);
            let line = line.trim_end();
            if let Some(v) = line.strip_prefix("#ASDF_STANDARD ") {
                standard = Some(v.trim().to_owned());
            } else if let Some(v) = line.strip_prefix("#ASDF ")
                && version.is_empty()
            {
                version = v.trim().to_owned();
            }
            i = (end + 1).min(data.len());
            if data[i..].starts_with(MAGIC) {
                break;
            }
        }
        if !version.starts_with("1.") {
            return Err(Error::Unsupported(format!(
                "ASDF file format version {version}"
            )));
        }
        let rest = &data[i..];
        let has_tree = rest.starts_with(b"%") || rest.starts_with(b"---");
        let (tree, blocks_from) = if has_tree {
            let (text_end, tree_end) = tree_end(data, i)?;
            let text = std::str::from_utf8(&data[..text_end])
                .map_err(|_| Error::Malformed("the tree is not UTF-8".into()))?;
            (yaml::parse(text)?, tree_end)
        } else {
            (
                Value {
                    tag: None,
                    node: Node::Null,
                },
                i,
            )
        };
        let (blocks, indexed) = find_blocks(data, blocks_from)?;
        let mut a = Asdf {
            version,
            standard,
            tree,
            blocks,
            indexed,
            arrays: Vec::new(),
            skipped: Vec::new(),
        };
        let (arrays, skipped) = array::collect(&a.tree, &a.blocks);
        a.arrays = arrays;
        a.skipped = skipped;
        Ok(a)
    }

    /// The writing library's name and version (`asdf_library`).
    pub fn library(&self) -> Option<(String, String)> {
        let lib = self.tree.get("asdf_library")?;
        let name = lib
            .get("name")
            .and_then(Value::scalar_text)
            .unwrap_or_default();
        let version = lib
            .get("version")
            .and_then(Value::scalar_text)
            .unwrap_or_default();
        (!name.is_empty() || !version.is_empty()).then_some((name, version))
    }

    /// A block's data, decompressed (its memory reserved from the budget while it is held).
    pub fn block_data<'d>(&self, data: &'d [u8], i: usize) -> Result<BlockData<'d>> {
        let b = self
            .blocks
            .get(i)
            .ok_or_else(|| Error::Malformed(format!("no block {i}")))?;
        let raw = stored(data, b)?;
        let Some(name) = b.compression_name() else {
            return Ok(BlockData::borrowed(raw));
        };
        if b.data_size > MAX_BLOCK {
            return Err(Error::TooLarge);
        }
        let n = b.data_size as usize;
        // zlib and bzip2 reserve their output from the memory budget as it grows.
        let out = match &b.compression {
            b"zlib" => specular_png::inflate_zlib(raw, n).map_err(|e| match e {
                specular_png::InflateError::NeedsMemory(m) => Error::NeedsMemory(m),
                e => Error::Malformed(format!("zlib block {i}: {e}")),
            })?,
            b"bzp2" => specular_xcf::bunzip2(raw, b.data_size).map_err(|e| match e {
                specular_xcf::Error::NeedsMemory(m) => Error::NeedsMemory(m),
                e => Error::Malformed(format!("bzip2 block {i}: {e}")),
            })?,
            b"lz4\0" => {
                // LZ4 expands at most 255:1: reserve what the input can become.
                let _res = specular_core::budget::reserve(
                    (n as u64).min((raw.len() as u64).saturating_mul(255)),
                )?;
                lz4_chunks(raw, n).map_err(|e| Error::Malformed(format!("lz4 block {i}: {e}")))?
            }
            _ => {
                return Err(Error::Unsupported(format!(
                    "block compression \"{name}\" (zlib, bzp2 and lz4 are read)"
                )));
            }
        };
        if out.len() != n {
            return Err(Error::Malformed(format!(
                "block {i} decompressed to {} bytes, header says {n}",
                out.len()
            )));
        }
        // The decompressors' reservations end with them; the block's goes with it.
        let reserved = specular_core::budget::reserve(out.len() as u64)?;
        Ok(BlockData {
            bytes: Cow::Owned(out),
            reserved,
        })
    }

    /// Checks a block's MD5 over its stored bytes; `None` when the header gives none.
    pub fn verify_checksum(&self, data: &[u8], i: usize) -> Result<Option<bool>> {
        use md5::Digest;
        let b = self
            .blocks
            .get(i)
            .ok_or_else(|| Error::Malformed(format!("no block {i}")))?;
        if b.checksum == [0; 16] {
            return Ok(None);
        }
        let sum = md5::Md5::digest(stored(data, b)?);
        Ok(Some(sum.as_slice() == b.checksum))
    }

    /// The bytes an array reads from: its block's data (external sources are the caller's:
    /// open that file and take [`external_data`] of it).
    pub fn array_data<'d>(&self, data: &'d [u8], a: &NdArray) -> Result<BlockData<'d>> {
        match &a.source {
            ArraySource::Block(i) => self.block_data(data, *i),
            ArraySource::Inline(_) => Ok(BlockData::borrowed(&[])),
            ArraySource::External(name) => Err(Error::Unsupported(format!(
                "array {} is in the external file {name}",
                a.path
            ))),
        }
    }

    /// One plane of `a` with its length-1 axes squeezed ([`NdArray::squeezed`]; `lead` indexes
    /// the squeezed leading axes) and its mask array applied. `external` gives the data of an
    /// external source by file name (the caller decides which files may be opened; see
    /// [`external_data`]).
    pub fn plane(
        &self,
        data: &[u8],
        a: &NdArray,
        lead: &[u64],
        external: &mut dyn FnMut(&str) -> Result<Vec<u8>>,
    ) -> Result<Vec<f32>> {
        let mut read = |x: &NdArray| -> Result<Vec<f32>> {
            let bytes = match &x.source {
                ArraySource::External(name) => BlockData::borrowed(&[]).with(external(name)?),
                _ => self.array_data(data, x)?,
            };
            x.squeezed().plane(&bytes, lead)
        };
        let mut values = read(a)?;
        if let Some(m) = &a.mask_array {
            let mask = read(m)?;
            NdArray::apply_mask(&mut values, &mask);
        }
        Ok(values)
    }
}

/// The data of an external (exploded) array file: its first block.
pub fn external_data(data: &[u8]) -> Result<BlockData<'_>> {
    let a = Asdf::open(data)?;
    if a.blocks.is_empty() {
        return Err(Error::Malformed("the external file has no block".into()));
    }
    a.block_data(data, 0)
}

/// A block's bytes: in the file, or decompressed with their memory reserved from the budget
/// until dropped (a caller keeping the bytes keeps the reservation: [`BlockData::into_parts`]).
#[derive(Debug)]
pub struct BlockData<'d> {
    bytes: Cow<'d, [u8]>,
    reserved: specular_core::Reservation,
}

impl<'d> BlockData<'d> {
    fn borrowed(bytes: &'d [u8]) -> BlockData<'d> {
        BlockData {
            bytes: Cow::Borrowed(bytes),
            reserved: specular_core::Reservation::none(),
        }
    }

    /// Owned bytes in place of these, keeping the reservation (the caller's bytes are its own
    /// to account for).
    fn with(self, bytes: Vec<u8>) -> BlockData<'d> {
        BlockData {
            bytes: Cow::Owned(bytes),
            reserved: self.reserved,
        }
    }

    /// The bytes, and the reservation that covers them when they were decompressed.
    pub fn into_parts(self) -> (Cow<'d, [u8]>, specular_core::Reservation) {
        (self.bytes, self.reserved)
    }

    /// Bytes reserved for the data (0: it is in the file).
    pub fn reserved(&self) -> u64 {
        self.reserved.bytes()
    }
}

impl std::ops::Deref for BlockData<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

fn stored<'d>(data: &'d [u8], b: &Block) -> Result<&'d [u8]> {
    let start = usize::try_from(b.data).map_err(|_| Error::TooLarge)?;
    let used = usize::try_from(b.used).map_err(|_| Error::TooLarge)?;
    data.get(start..start.saturating_add(used))
        .ok_or_else(|| Error::Malformed("a block runs past the end of the file".into()))
}

/// Where the tree ends: (end of the YAML text including the `...` line, offset after it).
fn tree_end(data: &[u8], from: usize) -> Result<(usize, usize)> {
    let limit = data.len().min(from.saturating_add(MAX_TREE));
    let mut i = from;
    while i < limit {
        let Some(p) = data[i..limit].iter().position(|&b| b == b'\n') else {
            break;
        };
        let ls = i + p + 1;
        if data[ls..].starts_with(b"...") {
            let after = ls + 3;
            match data.get(after) {
                None => return Ok((after, after)),
                Some(b'\n') => return Ok((after, after + 1)),
                Some(b'\r') if data.get(after + 1) == Some(&b'\n') => {
                    return Ok((after, after + 2));
                }
                Some(b' ' | b'\t') => {
                    let e = data[after..]
                        .iter()
                        .position(|&b| b == b'\n')
                        .map_or(data.len(), |q| after + q);
                    return Ok((after, (e + 1).min(data.len())));
                }
                _ => {}
            }
        }
        if data[ls..].starts_with(MAGIC) {
            // Blocks without a document end marker.
            return Ok((ls, ls));
        }
        i = ls;
    }
    if data.len() > limit {
        return Err(Error::Unsupported(format!(
            "a tree over {} MiB",
            MAX_TREE >> 20
        )));
    }
    Ok((data.len(), data.len()))
}

fn be16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(o..o + 2)?.try_into().ok()?))
}
fn be32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}
fn be64(d: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_be_bytes(d.get(o..o + 8)?.try_into().ok()?))
}

fn read_header(data: &[u8], off: usize) -> Result<Block> {
    let bad = |m: &str| Error::Malformed(format!("block at {off}: {m}"));
    if data.get(off..off + 4) != Some(MAGIC) {
        return Err(bad("no block magic"));
    }
    let hsize = usize::from(be16(data, off + 4).ok_or_else(|| bad("truncated header"))?);
    if hsize < 48 {
        return Err(bad("header too small"));
    }
    let h = off + 6;
    if data.len() < h + hsize {
        return Err(bad("truncated header"));
    }
    let flags = be32(data, h).ok_or_else(|| bad("truncated header"))?;
    let mut compression = [0u8; 4];
    compression.copy_from_slice(&data[h + 4..h + 8]);
    let mut allocated = be64(data, h + 8).ok_or_else(|| bad("truncated header"))?;
    let mut used = be64(data, h + 16).ok_or_else(|| bad("truncated header"))?;
    let mut data_size = be64(data, h + 24).ok_or_else(|| bad("truncated header"))?;
    let mut checksum = [0u8; 16];
    checksum.copy_from_slice(&data[h + 32..h + 48]);
    let start = (h + hsize) as u64;
    let len = data.len() as u64;
    if flags & 1 != 0 {
        allocated = len - start;
        used = allocated;
        data_size = used;
    }
    if used > allocated || start.checked_add(used).is_none_or(|e| e > len) {
        return Err(bad("data runs past the end of the file"));
    }
    if compression == [0; 4] && data_size != used {
        data_size = used;
    }
    Ok(Block {
        offset: off as u64,
        data: start,
        flags,
        compression,
        allocated,
        used,
        data_size,
        checksum,
    })
}

/// The blocks after `from`: through the block index when it is valid, else by walking.
fn find_blocks(data: &[u8], from: usize) -> Result<(Vec<Block>, bool)> {
    if let Some(offsets) = block_index(data, from) {
        let blocks: Result<Vec<Block>> = offsets.iter().map(|&o| read_header(data, o)).collect();
        if let Ok(b) = blocks {
            return Ok((b, true));
        }
    }
    // Walk: the first block follows the tree (after any padding).
    let mut blocks = Vec::new();
    let Some(first) = find(&data[from.min(data.len())..], MAGIC) else {
        return Ok((blocks, false));
    };
    let mut pos = from + first;
    while data.get(pos..pos + 4) == Some(MAGIC) {
        if blocks.len() >= MAX_BLOCKS {
            return Err(Error::Unsupported(format!("more than {MAX_BLOCKS} blocks")));
        }
        let b = read_header(data, pos)?;
        let next = b.data.checked_add(b.allocated).ok_or(Error::TooLarge)?;
        let streamed = b.streamed();
        blocks.push(b);
        if streamed {
            break;
        }
        pos = usize::try_from(next).map_err(|_| Error::TooLarge)?;
    }
    Ok((blocks, false))
}

/// The block index's offsets, when present and consistent.
fn block_index(data: &[u8], from: usize) -> Option<Vec<usize>> {
    let tail_start = data.len().saturating_sub(INDEX_TAIL).max(from);
    let tail = data.get(tail_start..)?;
    // The index follows the last block's bytes directly (no newline before it).
    let p = rfind(tail, INDEX_MARK)?;
    let text = std::str::from_utf8(&tail[p..]).ok()?;
    let v = yaml::parse(text).ok()?;
    let mut out = Vec::new();
    let mut last = None;
    for o in v.as_seq()? {
        let o = usize::try_from(o.as_i64()?).ok()?;
        if o < from || last.is_some_and(|l| o <= l) || data.get(o..o + 4) != Some(MAGIC) {
            return None;
        }
        if out.len() >= MAX_BLOCKS {
            return None;
        }
        out.push(o);
        last = Some(o);
    }
    (!out.is_empty()).then_some(out)
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    let first = needle[0];
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        let p = hay[i..=hay.len() - needle.len()]
            .iter()
            .position(|&b| b == first)?;
        if hay[i + p..].starts_with(needle) {
            return Some(i + p);
        }
        i += p + 1;
    }
    None
}

fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if hay.len() < needle.len() {
        return None;
    }
    (0..=hay.len() - needle.len())
        .rev()
        .find(|&i| hay[i..].starts_with(needle))
}

/// ASDF's LZ4 framing: chunks of (u32 big-endian length, then that many bytes holding a u32
/// little-endian decoded size and one raw LZ4 block).
fn lz4_chunks(raw: &[u8], n: usize) -> std::result::Result<Vec<u8>, String> {
    // LZ4 expands at most 255:1; don't trust the header's size for the up-front reserve.
    let mut out = Vec::with_capacity(n.min(raw.len().saturating_mul(255)));
    let mut pos = 0;
    while pos < raw.len() {
        let len = be32(raw, pos).ok_or("truncated chunk length")? as usize;
        let chunk = raw.get(pos + 4..pos + 4 + len).ok_or("truncated chunk")?;
        let size = u32::from_le_bytes(
            chunk
                .get(..4)
                .ok_or("truncated chunk")?
                .try_into()
                .unwrap_or([0; 4]),
        ) as usize;
        if out.len() + size > n {
            return Err("more data than the header says".into());
        }
        let part = specular_hdf5::codec::lz4_block(&chunk[4..], size).map_err(|e| e.to_string())?;
        out.extend_from_slice(&part);
        pos += 4 + len;
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
