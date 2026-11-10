//! Element data: plain, linked blocks, external files, compressed and chunked elements.

use std::borrow::Cow;
use std::sync::Arc;

use crate::codec::Coder;
use crate::dd::{self, Dd, SPECIAL, TAG_COMPRESSED, TAG_LINKED};
use crate::vobj::Vdata;
use crate::{Ctx, Error, File, Result, bad, be16, be32, bei32, limits};

const SP_LINKED: u16 = 1;
const SP_EXTERNAL: u16 = 2;
const SP_COMPRESSED: u16 = 3;
const SP_CHUNKED: u16 = 5;
const SP_BUFFERED: u16 = 6;
const SP_COMPRAS: u16 = 7;

/// How an element's bytes are stored.
#[derive(Clone, Debug)]
pub(crate) enum Store {
    /// No data at all (the element is absent).
    Missing,
    Plain {
        offset: u32,
        length: u32,
    },
    Linked {
        length: u64,
        /// (offset, length) of the data blocks in order; `None` for a block never written.
        blocks: Vec<Option<(u32, u32)>>,
        block_len: u32,
    },
    External {
        name: String,
        offset: u64,
        length: u64,
    },
    Compressed {
        length: u64,
        coder: Coder,
        inner: Box<Store>,
    },
    Chunked(Box<Chunked>),
}

/// A chunked element.
#[derive(Clone, Debug)]
pub(crate) struct Chunked {
    /// Bytes of one number.
    pub nt_size: usize,
    pub dims: Vec<u64>,
    pub chunk: Vec<u64>,
    /// The fill of one number, as stored.
    pub fill: Vec<u8>,
    /// Compression of every chunk (chunks are compressed elements themselves when set).
    pub coder: Option<Coder>,
    /// Chunk (tag, ref) by chunk grid position.
    pub table: std::collections::HashMap<Vec<u64>, (u16, u16)>,
}

impl Store {
    /// A description: "deflate (level 6), chunked 16 × 16" and so on.
    pub(crate) fn describe(&self) -> String {
        match self {
            Store::Missing => "not written".into(),
            Store::Plain { .. } => "contiguous".into(),
            Store::Linked { blocks, .. } => format!("linked blocks ({})", blocks.len()),
            Store::External { name, .. } => format!("external file {name}"),
            Store::Compressed { coder, .. } => coder_text(coder),
            Store::Chunked(c) => {
                let shape: Vec<String> = c.chunk.iter().map(u64::to_string).collect();
                match &c.coder {
                    Some(k) => format!("{}, chunks {}", coder_text(k), shape.join(" × ")),
                    None => format!("chunks {}", shape.join(" × ")),
                }
            }
        }
    }

    pub(crate) fn coder(&self) -> Option<&Coder> {
        match self {
            Store::Compressed { coder, .. } => Some(coder),
            Store::Chunked(c) => c.coder.as_ref(),
            _ => None,
        }
    }
}

fn coder_text(c: &Coder) -> String {
    match c {
        Coder::Deflate { level } => format!("deflate (level {level})"),
        Coder::SkipHuff { skip } => format!("skipping Huffman ({skip})"),
        Coder::Nbit {
            start_bit, bit_len, ..
        } => format!("NBIT ({bit_len} bits from bit {start_bit})"),
        c => c.name().to_owned(),
    }
}

/// Resolves how (tag, ref) is stored.
pub(crate) fn store(f: &File, ctx: Ctx, tag: u16, rf: u16) -> Result<Store> {
    match f.dds.get(tag, rf) {
        None => Ok(Store::Missing),
        Some(dd) => store_of(f, ctx, dd, 0),
    }
}

fn store_of(f: &File, ctx: Ctx, dd: &Dd, depth: usize) -> Result<Store> {
    if depth > limits::MAX_SPECIAL_DEPTH {
        return Err(bad("special elements nested too deep"));
    }
    if !dd.is_special() {
        if dd.offset == u32::MAX || dd.length == u32::MAX || dd.length == 0 {
            return Ok(Store::Missing);
        }
        return Ok(Store::Plain {
            offset: dd.offset,
            length: dd.length,
        });
    }
    let h = dd::raw_of(ctx.data, dd).ok_or_else(|| bad("special element header"))?;
    let sp = be16(h, 0).ok_or_else(|| bad("special element header"))?;
    let h = &h[2..];
    let i32at = |k: usize| bei32(h, k).ok_or_else(|| bad("special element header"));
    match sp {
        SP_LINKED => {
            let length = i32at(0)?.max(0) as u64;
            let block_len = i32at(4)?.max(0) as u32;
            let num_blk = i32at(8)?.max(1) as usize;
            let link_ref = be16(h, 12).ok_or_else(|| bad("linked block header"))?;
            let blocks = linked_blocks(f, ctx, link_ref, num_blk)?;
            Ok(Store::Linked {
                length,
                blocks,
                block_len,
            })
        }
        SP_EXTERNAL => {
            let length = i32at(0)?.max(0) as u64;
            let offset = i32at(4)?.max(0) as u64;
            let n = i32at(8)?.max(0) as usize;
            let name = h.get(12..12 + n).ok_or_else(|| bad("external file name"))?;
            Ok(Store::External {
                name: crate::clean_text(name),
                offset,
                length,
            })
        }
        SP_COMPRESSED => {
            // version, length, data ref, model, coder, coder information
            let length = i32at(2)?.max(0) as u64;
            let cref = be16(h, 6).ok_or_else(|| bad("compressed header"))?;
            let model = be16(h, 8).ok_or_else(|| bad("compressed header"))?;
            let coder = be16(h, 10).ok_or_else(|| bad("compressed header"))?;
            if model != 0 {
                return Err(Error::Unsupported(format!("compression model {model}")));
            }
            let (coder, _) = Coder::parse(coder, h.get(12..).unwrap_or(&[]))?;
            let inner = match f.dds.get(TAG_COMPRESSED, cref) {
                None => Store::Missing,
                Some(d) => store_of(f, ctx, d, depth + 1)?,
            };
            Ok(Store::Compressed {
                length,
                coder,
                inner: Box::new(inner),
            })
        }
        SP_CHUNKED => chunked(f, ctx, h).map(|c| Store::Chunked(Box::new(c))),
        SP_BUFFERED => Err(Error::Unsupported("buffered element".into())),
        SP_COMPRAS => Err(Error::Unsupported("compressed raster element".into())),
        s => Err(Error::Unsupported(format!("special element {s}"))),
    }
}

fn linked_blocks(
    f: &File,
    ctx: Ctx,
    first: u16,
    num_blk: usize,
) -> Result<Vec<Option<(u32, u32)>>> {
    let mut out = Vec::new();
    let mut next = first;
    let mut seen = std::collections::HashSet::new();
    while next != 0 {
        if !seen.insert(next) {
            break;
        }
        let t = f
            .dds
            .raw(ctx.data, TAG_LINKED, next)
            .ok_or_else(|| bad("link table"))?;
        next = be16(t, 0).unwrap_or(0);
        for k in 0..num_blk {
            let Some(r) = be16(t, 2 + 2 * k) else { break };
            if out.len() >= limits::MAX_BLOCKS {
                return Err(Error::TooLarge);
            }
            out.push(if r == 0 {
                None
            } else {
                f.dds
                    .get(TAG_LINKED, r)
                    .filter(|d| d.offset != u32::MAX)
                    .map(|d| (d.offset, d.length))
            });
        }
    }
    Ok(out)
}

fn chunked(f: &File, ctx: Ctx, h: &[u8]) -> Result<Chunked> {
    let e = || bad("chunked element header");
    // header length, version, flags, length, chunk size, number size, table, special, rank
    let flag = bei32(h, 5).ok_or_else(e)?;
    let nt_size = bei32(h, 17).ok_or_else(e)?;
    let tbl_ref = be16(h, 23).ok_or_else(e)?;
    let ndims = bei32(h, 29).ok_or_else(e)?;
    if !(1..=limits::MAX_RANK as i32).contains(&ndims) || !(1..=8).contains(&nt_size) {
        return Err(e());
    }
    let mut p = 33;
    let mut dims = Vec::new();
    let mut chunk = Vec::new();
    for _ in 0..ndims {
        let d = bei32(h, p + 4).ok_or_else(e)?;
        let c = bei32(h, p + 8).ok_or_else(e)?;
        if d < 0 || c <= 0 {
            return Err(e());
        }
        dims.push(d as u64);
        chunk.push(c as u64);
        p += 12;
    }
    let fill_len = bei32(h, p).ok_or_else(e)?;
    p += 4;
    let fill = h
        .get(p..p + fill_len.max(0) as usize)
        .ok_or_else(e)?
        .to_vec();
    p += fill.len();
    let coder = if flag & 1 != 0 {
        // version, information length, model, coder, coder information
        let model = be16(h, p + 6).ok_or_else(e)?;
        let c = be16(h, p + 8).ok_or_else(e)?;
        if model != 0 {
            return Err(Error::Unsupported(format!("compression model {model}")));
        }
        Some(Coder::parse(c, h.get(p + 10..).unwrap_or(&[]))?.0)
    } else {
        None
    };
    let nchunks = dims
        .iter()
        .zip(&chunk)
        .try_fold(1u64, |a, (d, c)| a.checked_mul(d.div_ceil(*c).max(1)));
    if nchunks.is_none_or(|n| n > limits::MAX_CHUNKS) {
        return Err(Error::TooLarge);
    }
    let mut table = std::collections::HashMap::new();
    if let Some(vd) = f.vdatas.iter().find(|v| v.rf == tbl_ref) {
        read_chunk_table(f, ctx, vd, ndims as usize, &mut table)?;
    }
    Ok(Chunked {
        nt_size: nt_size as usize,
        dims,
        chunk,
        fill,
        coder,
        table,
    })
}

fn read_chunk_table(
    f: &File,
    ctx: Ctx,
    vd: &Vdata,
    ndims: usize,
    table: &mut std::collections::HashMap<Vec<u64>, (u16, u16)>,
) -> Result<()> {
    let bytes = crate::vobj::vdata_bytes(f, ctx, vd)?;
    let rec = vd.record_size();
    let need = 4 * ndims + 4;
    if rec < need {
        return Err(bad("chunk table"));
    }
    for r in bytes.chunks_exact(rec).take(vd.nrecords as usize) {
        let origin: Vec<u64> = (0..ndims)
            .map(|k| be32(r, 4 * k).unwrap_or(0) as u64)
            .collect();
        let tag = be16(r, 4 * ndims).unwrap_or(0);
        let rf = be16(r, 4 * ndims + 2).unwrap_or(0);
        table.insert(origin, (tag, rf));
    }
    Ok(())
}

/// Bytes of an element: in the file, or gathered or decoded with their memory reserved from
/// the budget until dropped.
pub(crate) struct Read<'a> {
    bytes: Cow<'a, [u8]>,
    _reserved: specular_core::Reservation,
}

impl<'a> Read<'a> {
    fn borrowed(bytes: &'a [u8]) -> Read<'a> {
        Read {
            bytes: Cow::Borrowed(bytes),
            _reserved: specular_core::Reservation::none(),
        }
    }

    fn owned(bytes: Vec<u8>, reserved: specular_core::Reservation) -> Read<'a> {
        Read {
            bytes: Cow::Owned(bytes),
            _reserved: reserved,
        }
    }

    /// The bytes, without their reservation (the caller accounts for them).
    pub(crate) fn into_owned(self) -> Vec<u8> {
        self.bytes.into_owned()
    }

    #[cfg(test)]
    pub(crate) fn reserved(&self) -> u64 {
        self._reserved.bytes()
    }
}

impl std::ops::Deref for Read<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

/// The whole decoded bytes of a store (`length` of them for plain, linked and external data;
/// shorter when the file ends early). Bytes not in the file stay reserved while held.
pub(crate) fn read<'a>(f: &File, ctx: Ctx<'a>, st: &Store, key: (u16, u16)) -> Result<Read<'a>> {
    match st {
        Store::Missing => Ok(Read::borrowed(&[])),
        Store::Plain { offset, length } => {
            let s = *offset as usize;
            let e = s.saturating_add(*length as usize).min(ctx.data.len());
            Ok(Read::borrowed(ctx.data.get(s..e).unwrap_or(&[])))
        }
        Store::Linked {
            length,
            blocks,
            block_len,
        } => {
            if *length > limits::MAX_ELEMENT {
                return Err(Error::TooLarge);
            }
            let res = specular_core::budget::reserve(*length)?;
            let mut out = Vec::with_capacity(*length as usize);
            for (k, b) in blocks.iter().enumerate() {
                if out.len() as u64 >= *length {
                    break;
                }
                let want = (*length - out.len() as u64) as usize;
                match b {
                    Some((o, l)) => {
                        // The first block is as long as its descriptor says; the rest are
                        // `block_len` long.
                        let n = if k == 0 { *l } else { (*block_len).min(*l) } as usize;
                        let n = n.min(want);
                        let s = *o as usize;
                        let e = s.saturating_add(n).min(ctx.data.len());
                        out.extend_from_slice(ctx.data.get(s..e).unwrap_or(&[]));
                    }
                    None => {
                        let n = (*block_len as usize).min(want);
                        out.resize(out.len() + n, 0);
                    }
                }
            }
            Ok(Read::owned(out, res))
        }
        Store::External {
            name,
            offset,
            length,
        } => {
            if *length > limits::MAX_ELEMENT {
                return Err(Error::TooLarge);
            }
            let r = ctx
                .external
                .ok_or_else(|| Error::Unsupported(format!("external file {name}")))?;
            let res = specular_core::budget::reserve(*length)?;
            r(name, *offset, *length)
                .map(|v| Read::owned(v, res))
                .ok_or_else(|| Error::Unsupported(format!("external file {name} not readable")))
        }
        Store::Compressed {
            length,
            coder,
            inner,
        } => {
            if *length > limits::MAX_ELEMENT {
                return Err(Error::TooLarge);
            }
            // Chunks are cached decoded by `chunk`.
            let cacheable = key.0 & !SPECIAL != dd::TAG_CHUNK;
            let ck = (key.0, key.1, u64::MAX);
            if cacheable && let Some(v) = f.cache.lock().ok().and_then(|mut c| c.get(ck)) {
                let res = specular_core::budget::reserve(v.len() as u64)?;
                return Ok(Read::owned(v.as_ref().clone(), res));
            }
            let src = read(f, ctx, inner, (TAG_COMPRESSED, 0))?;
            let v = coder.decode(&src, *length as usize)?;
            drop(src);
            // The decoder's reservation ends with it; the copy handed back keeps its own.
            let res = specular_core::budget::reserve(v.len() as u64)?;
            if cacheable && let Ok(mut c) = f.cache.lock() {
                c.put(ck, Arc::new(v.clone()));
            }
            Ok(Read::owned(v, res))
        }
        Store::Chunked(_) => Err(bad("chunked element read whole")),
    }
}

/// One chunk's bytes (`None` when the chunk was never written), decoded and cached.
pub(crate) fn chunk(
    f: &File,
    ctx: Ctx,
    c: &Chunked,
    owner: (u16, u16),
    pos: &[u64],
) -> Result<Option<Arc<Vec<u8>>>> {
    let Some(&(tag, rf)) = c.table.get(pos) else {
        return Ok(None);
    };
    let mut lin = 0u64;
    for (k, p) in pos.iter().enumerate() {
        lin = lin * c.dims[k].div_ceil(c.chunk[k]).max(1) + p;
    }
    let key = (owner.0, owner.1, lin);
    if let Some(v) = f.cache.lock().ok().and_then(|mut cc| cc.get(key)) {
        return Ok(Some(v));
    }
    let bytes = c
        .chunk
        .iter()
        .try_fold(c.nt_size as u64, |a, &d| a.checked_mul(d))
        .filter(|&b| b <= limits::MAX_ELEMENT)
        .ok_or(Error::TooLarge)?;
    let _res = specular_core::budget::reserve(bytes)?;
    let st = store(f, ctx, tag & !SPECIAL, rf)?;
    let raw = read(f, ctx, &st, (tag, rf))?;
    let mut v = raw.into_owned();
    if (v.len() as u64) < bytes {
        // A short chunk: pad with the fill value.
        let fill = if c.fill.len() == c.nt_size {
            c.fill.clone()
        } else {
            vec![0; c.nt_size]
        };
        while (v.len() as u64) < bytes {
            let k = v.len() % c.nt_size;
            v.push(fill[k]);
        }
    }
    v.truncate(bytes as usize);
    let v = Arc::new(v);
    if let Ok(mut cc) = f.cache.lock() {
        cc.put(key, v.clone());
    }
    Ok(Some(v))
}

#[cfg(test)]
mod hostile {
    use super::*;

    #[test]
    fn chunk_grid_overflow() {
        let d = crate::hostile::tiny();
        let f = File::open(&d).unwrap();
        // Three dimensions of 2^31 - 1 in chunks of one: 2^93 chunks.
        let mut h = vec![0u8; 33];
        h[17..21].copy_from_slice(&1i32.to_be_bytes());
        h[29..33].copy_from_slice(&3i32.to_be_bytes());
        for _ in 0..3 {
            h.extend_from_slice(&[0; 4]);
            h.extend_from_slice(&i32::MAX.to_be_bytes());
            h.extend_from_slice(&1i32.to_be_bytes());
        }
        h.extend_from_slice(&[0; 4]);
        assert!(matches!(
            chunked(&f, Ctx::new(&d), &h),
            Err(Error::TooLarge)
        ));
    }

    /// Linked blocks gathered into one buffer (8 MiB of blocks never written): the buffer
    /// handed back keeps its reservation until it is dropped, so two do not fit 12 MiB.
    #[test]
    fn linked_blocks_stay_reserved() {
        let d = crate::hostile::tiny();
        let f = File::open(&d).unwrap();
        let st = Store::Linked {
            length: 8 << 20,
            blocks: vec![None; 8],
            block_len: 1 << 20,
        };
        let b = Arc::new(specular_core::budget::MemoryBudget::fixed(12 << 20));
        specular_core::budget::with_budget(b.clone(), || {
            let first = read(&f, Ctx::new(&d), &st, (0, 0)).unwrap();
            assert_eq!(first.len(), 8 << 20);
            assert_eq!(first.reserved(), 8 << 20);
            assert_eq!(b.reserved(), 8 << 20);
            assert!(matches!(
                read(&f, Ctx::new(&d), &st, (0, 0)),
                Err(Error::NeedsMemory(_))
            ));
            drop(first);
            assert_eq!(b.reserved(), 0);
        });
    }

    #[test]
    fn chunk_size_overflow() {
        let d = crate::hostile::tiny();
        let f = File::open(&d).unwrap();
        let c = Chunked {
            nt_size: 8,
            dims: vec![1; 3],
            chunk: vec![i32::MAX as u64; 3],
            fill: Vec::new(),
            coder: None,
            table: [(vec![0; 3], (dd::TAG_SD, 1))].into_iter().collect(),
        };
        assert!(matches!(
            chunk(&f, Ctx::new(&d), &c, (dd::TAG_SD, 1), &[0; 3]),
            Err(Error::TooLarge)
        ));
    }

    /// A 512 MiB chunk (under the cap) is refused under a 64 MiB memory budget before it is
    /// read or padded.
    #[test]
    fn chunk_within_memory_budget() {
        let d = crate::hostile::tiny();
        let f = File::open(&d).unwrap();
        let c = Chunked {
            nt_size: 8,
            dims: vec![1024, 1024, 64],
            chunk: vec![1024, 1024, 64],
            fill: Vec::new(),
            coder: None,
            table: [(vec![0; 3], (dd::TAG_SD, 1))].into_iter().collect(),
        };
        let r = specular_core::budget::with_limit(64 << 20, || {
            chunk(&f, Ctx::new(&d), &c, (dd::TAG_SD, 1), &[0; 3])
        });
        let Err(Error::NeedsMemory(n)) = r else {
            panic!("{r:?}")
        };
        assert!(n.needed >= 512 << 20 && n.needed > n.free, "{n:?}");
    }
}
