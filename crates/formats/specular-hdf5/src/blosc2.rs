//! The Blosc2 filter (32026): a chunk is a Blosc2 contiguous frame (or, from early filter
//! versions, a bare Blosc2 chunk). Frames hold a msgpack header, the data chunks, an offsets
//! chunk and a trailer; a "b2nd" metalayer marks an N-dimensional array whose chunks are
//! stored block by block, which is re-tiled to row-major order here.
//!
//! Codecs: BloscLZ, LZ4/LZ4HC, zlib and Zstandard; filters: byte shuffle, bit shuffle and
//! delta and precision truncation (nothing to undo). Dictionaries and user codecs are
//! refused.

use crate::filters::{bitunshuffle, blosclz, lz4_block, undelta, unshuffle_into, zstd};
use crate::{Error, Result};

fn bad(what: &str) -> Error {
    Error::Malformed(format!("Blosc2 {what}"))
}

fn le32(b: &[u8], at: usize) -> Result<usize> {
    let s = b.get(at..at + 4).ok_or_else(|| bad("header"))?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize)
}

fn le_i32(b: &[u8], at: usize) -> Result<i32> {
    let s = b.get(at..at + 4).ok_or_else(|| bad("header"))?;
    Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

const FRAME_MAGIC: &[u8] = b"b2frame\0";

/// Decodes one stored chunk of the HDF5 Blosc2 filter to at most `max` bytes.
pub(crate) fn decode(src: &[u8], max: usize) -> Result<Vec<u8>> {
    if src.len() > 10 && src[0] & 0xf0 == 0x90 && src[1] == 0xa8 && &src[2..10] == FRAME_MAGIC {
        frame(src, max)
    } else {
        chunk(src, max).map(|(v, _)| v)
    }
}

/// A msgpack value, as much as frames use.
#[derive(Debug, Clone)]
enum Mp {
    Int(i64),
    Bool,
    Nil,
    Str(Vec<u8>),
    Bin(Vec<u8>),
    Arr(Vec<Mp>),
    Map(Vec<(Mp, Mp)>),
    Ext,
}

impl Mp {
    fn int(&self) -> Option<i64> {
        match self {
            Mp::Int(v) => Some(*v),
            _ => None,
        }
    }
}

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl Rd<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let s = self
            .b
            .get(self.p..self.p.checked_add(n).ok_or_else(|| bad("msgpack"))?)
            .ok_or_else(|| bad("msgpack"))?;
        self.p += n;
        Ok(s)
    }
    fn be(&mut self, n: usize) -> Result<u64> {
        Ok(self
            .take(n)?
            .iter()
            .fold(0u64, |a, &x| (a << 8) | u64::from(x)))
    }
    fn value(&mut self, depth: u32) -> Result<Mp> {
        if depth > 8 {
            return Err(bad("msgpack nesting"));
        }
        let t = self.take(1)?[0];
        Ok(match t {
            0x00..=0x7f => Mp::Int(i64::from(t)),
            0xe0..=0xff => Mp::Int(i64::from(t as i8)),
            0xc0 => Mp::Nil,
            0xc2 | 0xc3 => Mp::Bool,
            0xcc => Mp::Int(self.be(1)? as i64),
            0xcd => Mp::Int(self.be(2)? as i64),
            0xce => Mp::Int(self.be(4)? as i64),
            0xcf => Mp::Int(self.be(8)? as i64),
            0xd0 => Mp::Int(i64::from(self.be(1)? as u8 as i8)),
            0xd1 => Mp::Int(i64::from(self.be(2)? as u16 as i16)),
            0xd2 => Mp::Int(i64::from(self.be(4)? as u32 as i32)),
            0xd3 => Mp::Int(self.be(8)? as i64),
            0xca => {
                self.take(4)?;
                Mp::Nil
            }
            0xcb => {
                self.take(8)?;
                Mp::Nil
            }
            0xa0..=0xbf => Mp::Str(self.take(usize::from(t & 31))?.to_vec()),
            0xd9 | 0xc4 => {
                let n = self.be(1)? as usize;
                let v = self.take(n)?.to_vec();
                if t == 0xd9 { Mp::Str(v) } else { Mp::Bin(v) }
            }
            0xda | 0xc5 => {
                let n = self.be(2)? as usize;
                let v = self.take(n)?.to_vec();
                if t == 0xda { Mp::Str(v) } else { Mp::Bin(v) }
            }
            0xdb | 0xc6 => {
                let n = self.be(4)? as usize;
                let v = self.take(n)?.to_vec();
                if t == 0xdb { Mp::Str(v) } else { Mp::Bin(v) }
            }
            0x90..=0x9f => self.arr(usize::from(t & 15), depth)?,
            0xdc => {
                let n = self.be(2)? as usize;
                self.arr(n, depth)?
            }
            0xdd => {
                let n = self.be(4)? as usize;
                self.arr(n, depth)?
            }
            0x80..=0x8f => self.map(usize::from(t & 15), depth)?,
            0xde => {
                let n = self.be(2)? as usize;
                self.map(n, depth)?
            }
            0xdf => {
                let n = self.be(4)? as usize;
                self.map(n, depth)?
            }
            0xd4..=0xd8 => {
                self.take(1 + (1usize << (t - 0xd4)))?;
                Mp::Ext
            }
            0xc7..=0xc9 => {
                let n = self.be(1 << (t - 0xc7))? as usize;
                self.take(1 + n)?;
                Mp::Ext
            }
            _ => return Err(bad("msgpack type")),
        })
    }
    fn arr(&mut self, n: usize, depth: u32) -> Result<Mp> {
        if n > 1 << 16 {
            return Err(bad("msgpack array"));
        }
        let mut v = Vec::with_capacity(n.min(64));
        for _ in 0..n {
            v.push(self.value(depth + 1)?);
        }
        Ok(Mp::Arr(v))
    }
    fn map(&mut self, n: usize, depth: u32) -> Result<Mp> {
        if n > 1 << 16 {
            return Err(bad("msgpack map"));
        }
        let mut v = Vec::with_capacity(n.min(64));
        for _ in 0..n {
            let k = self.value(depth + 1)?;
            let x = self.value(depth + 1)?;
            v.push((k, x));
        }
        Ok(Mp::Map(v))
    }
}

/// The b2nd metalayer: array shape, chunk shape and block shape.
struct Nd {
    shape: Vec<u64>,
    chunk: Vec<u64>,
    block: Vec<u64>,
}

fn ints(v: &Mp) -> Option<Vec<u64>> {
    match v {
        Mp::Arr(a) => a
            .iter()
            .map(|x| x.int().filter(|&i| i >= 0).map(|i| i as u64))
            .collect(),
        _ => None,
    }
}

fn b2nd(meta: &[u8]) -> Option<Nd> {
    let v = Rd { b: meta, p: 0 }.value(0).ok()?;
    let Mp::Arr(a) = v else { return None };
    let ndim = a.get(1)?.int()? as usize;
    let nd = Nd {
        shape: ints(a.get(2)?)?,
        chunk: ints(a.get(3)?)?,
        block: ints(a.get(4)?)?,
    };
    ((1..=16).contains(&ndim)
        && nd.shape.len() == ndim
        && nd.chunk.len() == ndim
        && nd.block.len() == ndim
        && !nd.chunk.contains(&0)
        && !nd.block.contains(&0))
    .then_some(nd)
}

fn frame(src: &[u8], max: usize) -> Result<Vec<u8>> {
    let mut r = Rd { b: src, p: 0 };
    let Mp::Arr(h) = r.value(0)? else {
        return Err(bad("frame header"));
    };
    let get = |i: usize| {
        h.get(i)
            .and_then(Mp::int)
            .ok_or_else(|| bad("frame header"))
    };
    let header_len = get(1)? as usize;
    let frame_len = get(2)? as u64;
    let nbytes = get(4)?;
    let cbytes = get(5)?;
    let typesize = get(6)?.max(1) as usize;
    let chunksize = get(8)?;
    if frame_len > src.len() as u64 || nbytes < 0 || cbytes < 0 || header_len > src.len() {
        return Err(bad("frame sizes"));
    }
    // The metalayers: [index size, {name: offset}, [content]].
    let mut nd = None;
    if let Some(Mp::Arr(m)) = h.get(13)
        && let (Some(Mp::Map(names)), Some(Mp::Arr(content))) = (m.get(1), m.get(2))
    {
        for ((k, _), c) in names.iter().zip(content) {
            if let (Mp::Str(k), Mp::Bin(c)) = (k, c)
                && k == b"b2nd"
            {
                nd = b2nd(c);
            }
        }
    }
    // A b2nd chunk is padded to whole blocks: less than twice its size along each dimension.
    let limit = match &nd {
        Some(nd) => (max as u64).saturating_mul(1 << nd.shape.len().min(4)),
        None => max as u64,
    };
    if nbytes as u64 > limit {
        return Err(bad("frame sizes"));
    }
    let nbytes = nbytes as usize;
    if nbytes == 0 {
        return Ok(Vec::new());
    }
    if chunksize <= 0 {
        return Err(bad("frame chunk size"));
    }
    let chunksize = chunksize as usize;
    let nchunks = nbytes.div_ceil(chunksize);
    // Every chunk has an 8-byte offset: the offsets cannot outweigh the data by more than
    // the tiniest real chunks would (a 1-byte chunk size would ask for 8x `max`).
    if nchunks > max / 8 + 1024 {
        return Err(bad("frame chunk size"));
    }
    let off_at = header_len
        .checked_add(cbytes as usize)
        .filter(|&p| p <= src.len())
        .ok_or_else(|| bad("frame offsets"))?;
    let (offsets, _) = chunk(&src[off_at..], nchunks.saturating_mul(8))?;
    if offsets.len() < nchunks * 8 {
        return Err(bad("frame offsets"));
    }
    let mut flat = Vec::with_capacity(nbytes);
    for i in 0..nchunks {
        let o = i64::from_le_bytes(offsets[i * 8..i * 8 + 8].try_into().unwrap_or_default());
        let want = chunksize.min(nbytes - i * chunksize);
        if o < 0 {
            // A special chunk folded into its offset: zeros (or uninitialised), NaN or a
            // repeated value cannot be told apart here beyond zeros.
            let kind = ((o as u64) >> 56) as u8 & 0x7f;
            if !matches!(kind, 0 | 1 | 4) {
                return Err(Error::Unsupported("Blosc2 special frame chunk".into()));
            }
            flat.resize(flat.len() + want, 0);
            continue;
        }
        let at = header_len
            .checked_add(o as usize)
            .filter(|&p| p < off_at)
            .ok_or_else(|| bad("chunk offset"))?;
        let (c, _) = chunk(&src[at..off_at], want)?;
        if c.len() != want {
            return Err(bad("chunk size"));
        }
        flat.extend_from_slice(&c);
    }
    match nd {
        Some(nd) if nd.block != nd.shape || nd.chunk != nd.shape => {
            retile(&flat, &nd, typesize, max)
        }
        _ => Ok(flat),
    }
}

/// Re-tiles b2nd storage (chunks of padded blocks, each row-major) to a row-major array.
fn retile(flat: &[u8], nd: &Nd, ts: usize, max: usize) -> Result<Vec<u8>> {
    let n = nd.shape.len();
    let total = nd
        .shape
        .iter()
        .try_fold(ts as u64, |a, &d| a.checked_mul(d))
        .filter(|&t| t <= max as u64)
        .ok_or_else(|| bad("b2nd shape"))? as usize;
    let mut out = vec![0u8; total];
    // Padded chunk extent: whole blocks.
    // The chunk and block shapes are the file's: every product is checked, and the padded
    // chunks must all be in `flat`, which bounds the loops below.
    let ext: Vec<u64> = (0..n)
        .map(|d| nd.chunk[d].div_ceil(nd.block[d]).checked_mul(nd.block[d]))
        .collect::<Option<_>>()
        .ok_or_else(|| bad("b2nd chunks"))?;
    let product = |v: &[u64]| v.iter().try_fold(1u64, |a, &x| a.checked_mul(x));
    let chunk_elems = product(&ext).ok_or_else(|| bad("b2nd chunks"))?;
    let block_elems = product(&nd.block).ok_or_else(|| bad("b2nd chunks"))?;
    let cgrid: Vec<u64> = (0..n).map(|d| nd.shape[d].div_ceil(nd.chunk[d])).collect();
    let bgrid: Vec<u64> = (0..n).map(|d| ext[d] / nd.block[d]).collect();
    let nchunks: u64 = cgrid.iter().product();
    let nblocks: u64 = bgrid.iter().product();
    if nchunks
        .checked_mul(chunk_elems)
        .and_then(|e| e.checked_mul(ts as u64))
        .is_none_or(|b| b > flat.len() as u64)
    {
        return Err(bad("b2nd chunks"));
    }
    let mut stride = vec![1u64; n];
    for d in (0..n.saturating_sub(1)).rev() {
        stride[d] = stride[d + 1] * nd.shape[d + 1];
    }
    let unravel = |mut i: u64, grid: &[u64], out: &mut [u64]| {
        for d in (0..n).rev() {
            out[d] = i % grid[d];
            i /= grid[d];
        }
    };
    let (mut cpos, mut bpos, mut rpos) = (vec![0u64; n], vec![0u64; n], vec![0u64; n]);
    let rows_per_block = block_elems / nd.block[n - 1];
    let mut rgrid = nd.block.clone();
    rgrid[n - 1] = 1;
    for ci in 0..nchunks {
        unravel(ci, &cgrid, &mut cpos);
        for bi in 0..nblocks {
            unravel(bi, &bgrid, &mut bpos);
            let base = (ci * chunk_elems + bi * block_elems) as usize * ts;
            for ri in 0..rows_per_block {
                unravel(ri, &rgrid, &mut rpos);
                // Coordinates of the row's first element in the chunk and in the array.
                let mut inside = true;
                let mut at = 0u64;
                for d in 0..n {
                    let local = bpos[d] * nd.block[d] + rpos[d];
                    let g = cpos[d] * nd.chunk[d] + local;
                    if local >= nd.chunk[d] || g >= nd.shape[d] {
                        inside = false;
                        break;
                    }
                    at += g * stride[d];
                }
                if !inside {
                    continue;
                }
                let last = n - 1;
                let x0 = cpos[last] * nd.chunk[last] + bpos[last] * nd.block[last];
                let lim_chunk = (cpos[last] + 1) * nd.chunk[last];
                let len = nd.block[last]
                    .min(lim_chunk.saturating_sub(x0))
                    .min(nd.shape[last].saturating_sub(x0)) as usize;
                let s = base + (ri * nd.block[last]) as usize * ts;
                let o = at as usize * ts;
                out[o..o + len * ts].copy_from_slice(&flat[s..s + len * ts]);
            }
        }
    }
    Ok(out)
}

/// One Blosc2 chunk (extended header) to its `nbytes`, and the chunk's stored size. Blosc1
/// chunks go through the Blosc1 decoder.
pub(crate) fn chunk(src: &[u8], max: usize) -> Result<(Vec<u8>, usize)> {
    if src.len() < 16 {
        return Err(bad("chunk"));
    }
    let flags = src[2];
    let typesize = (src[3] as usize).max(1);
    let nbytes = le32(src, 4)?;
    let blocksize = le32(src, 8)?;
    let cbytes = le32(src, 12)?;
    if nbytes > max || cbytes > src.len() {
        return Err(bad("chunk sizes"));
    }
    let src = &src[..cbytes];
    if flags & 0x05 != 0x05 {
        return crate::filters::blosc(src, max).map(|v| (v, cbytes));
    }
    if src.len() < 32 {
        return Err(bad("chunk"));
    }
    let filters = &src[16..22];
    let b2flags = src[31];
    match (b2flags >> 4) & 7 {
        0 => {}
        1 | 4 => return Ok((vec![0; nbytes], cbytes)),
        2 => {
            let nan: &[u8] = match typesize {
                4 => &f32::NAN.to_le_bytes(),
                8 => &f64::NAN.to_le_bytes(),
                _ => return Err(bad("NaN chunk")),
            };
            return Ok((nan.repeat(nbytes / typesize), cbytes));
        }
        3 => {
            let v = src
                .get(32..32 + typesize)
                .ok_or_else(|| bad("value chunk"))?;
            let mut out = v.repeat(nbytes / typesize);
            out.resize(nbytes, 0);
            return Ok((out, cbytes));
        }
        _ => return Err(Error::Unsupported("Blosc2 special chunk".into())),
    }
    if flags & 0x02 != 0 {
        return src
            .get(32..32 + nbytes)
            .map(|v| (v.to_vec(), cbytes))
            .ok_or_else(|| bad("chunk"));
    }
    if b2flags & 0x01 != 0 {
        return Err(Error::Unsupported("Blosc2 dictionary".into()));
    }
    if nbytes == 0 {
        return Ok((Vec::new(), cbytes));
    }
    if blocksize == 0 {
        return Err(bad("block size"));
    }
    let codec = flags >> 5;
    if !matches!(codec, 0 | 1 | 3 | 4) {
        return Err(Error::Unsupported(format!("Blosc2 codec {codec}")));
    }
    for &f in filters {
        if !matches!(f, 0..=4) {
            return Err(Error::Unsupported(format!("Blosc2 filter {f}")));
        }
    }
    let nblocks = nbytes.div_ceil(blocksize);
    let mut out = vec![0u8; nbytes];
    let mut tmp: Vec<u8> = Vec::with_capacity(blocksize.min(nbytes));
    let mut tmp2: Vec<u8> = Vec::new();
    for b in 0..nblocks {
        let start = le_i32(src, 32 + 4 * b)?;
        let bsize = blocksize.min(nbytes - b * blocksize);
        let delta = filters.contains(&3);
        if start == 0 {
            // An all-zero block (with delta, the reference block itself).
            if delta && b > 0 {
                let (head, rest) = out.split_at_mut(b * blocksize);
                rest[..bsize].copy_from_slice(&head[..bsize]);
            }
            continue;
        }
        let start = usize::try_from(start).map_err(|_| bad("block start"))?;
        let split_flag = flags & 0x10 == 0;
        let splits = if split_flag && typesize <= 16 && bsize.is_multiple_of(typesize) {
            typesize
        } else {
            1
        };
        tmp.clear();
        if block(src, start, splits, bsize, codec, &mut tmp).is_err() {
            // The other split choice, for writers that decided per block.
            tmp.clear();
            let other = if splits == 1 { typesize } else { 1 };
            if other == 1 || bsize.is_multiple_of(other) {
                block(src, start, other, bsize, codec, &mut tmp)?;
            } else {
                return Err(bad("block"));
            }
        }
        if tmp.len() != bsize {
            return Err(bad("block"));
        }
        // Undo the filters, last applied first.
        let mut cur: &mut Vec<u8> = &mut tmp;
        let mut other: &mut Vec<u8> = &mut tmp2;
        for &f in filters.iter().rev() {
            match f {
                1 if typesize > 1 => {
                    other.resize(bsize, 0);
                    unshuffle_into(cur, typesize, other);
                    std::mem::swap(&mut cur, &mut other);
                }
                2 => {
                    other.resize(bsize, 0);
                    bitunshuffle(cur, typesize, bsize / typesize, other);
                    std::mem::swap(&mut cur, &mut other);
                }
                3 => undelta(cur, typesize, (b > 0).then(|| &out[..bsize])),
                _ => {}
            }
        }
        out[b * blocksize..b * blocksize + bsize].copy_from_slice(cur);
    }
    Ok((out, cbytes))
}

/// The streams of one block, decoded into `tmp`.
fn block(
    src: &[u8],
    start: usize,
    splits: usize,
    bsize: usize,
    codec: u8,
    tmp: &mut Vec<u8>,
) -> Result<()> {
    let per = bsize / splits;
    let mut p = start;
    for _ in 0..splits {
        let cs = le_i32(src, p)?;
        p += 4;
        if cs == 0 {
            tmp.resize(tmp.len() + per, 0);
            continue;
        }
        if cs < 0 {
            // A run of one byte value.
            let v = (cs.unsigned_abs() & 0xff) as u8;
            tmp.resize(tmp.len() + per, v);
            continue;
        }
        let cs = cs as usize;
        let s = src.get(p..p + cs).ok_or_else(|| bad("stream"))?;
        p += cs;
        if cs == per {
            tmp.extend_from_slice(s);
            continue;
        }
        match codec {
            0 => blosclz(s, per, tmp)?,
            1 => lz4_block(s, per, tmp)?,
            3 => {
                let v = specular_png::inflate_zlib(s, per).map_err(|_| bad("zlib"))?;
                if v.len() != per {
                    return Err(bad("zlib"));
                }
                tmp.extend_from_slice(&v);
            }
            _ => {
                let v = zstd(s, per)?;
                if v.len() != per {
                    return Err(bad("Zstandard"));
                }
                tmp.extend_from_slice(&v);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msgpack_values() {
        let b = [0x93, 0x01, 0xd2, 0, 0, 1, 0, 0xa2, b'h', b'i'];
        let v = Rd { b: &b, p: 0 }.value(0).unwrap();
        let Mp::Arr(a) = v else { panic!() };
        assert_eq!(a[1].int(), Some(256));
        assert!(matches!(&a[2], Mp::Str(s) if s == b"hi"));
        assert!(Rd { b: &[0xc1], p: 0 }.value(0).is_err());
    }

    #[test]
    fn retiles_blocks() {
        // A 3×5 array, chunk 3×5, block 2×2: blocks (padded to 4×6) hold rows of 2.
        let nd = Nd {
            shape: vec![3, 5],
            chunk: vec![3, 5],
            block: vec![2, 2],
        };
        let want: Vec<u8> = (0..15).collect();
        let mut flat = vec![0xffu8; 24];
        let mut k = 0;
        for by in 0..2 {
            for bx in 0..3 {
                for y in 0..2 {
                    for x in 0..2 {
                        let (gy, gx) = (by * 2 + y, bx * 2 + x);
                        if gy < 3 && gx < 5 {
                            flat[k] = (gy * 5 + gx) as u8;
                        }
                        k += 1;
                    }
                }
            }
        }
        assert_eq!(retile(&flat, &nd, 1, 100).unwrap(), want);
    }

    #[test]
    fn special_and_memcpy_chunks() {
        let mut h = vec![5u8, 1, 0x07, 2];
        h.extend_from_slice(&6u32.to_le_bytes());
        h.extend_from_slice(&6u32.to_le_bytes());
        h.extend_from_slice(&38u32.to_le_bytes());
        h.resize(32, 0);
        h.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(chunk(&h, 6).unwrap().0, [1, 2, 3, 4, 5, 6]);
        h[31] = 0x10;
        assert_eq!(chunk(&h, 6).unwrap().0, [0; 6]);
        h[31] = 0x30;
        h[32] = 9;
        h[33] = 1;
        assert_eq!(chunk(&h, 6).unwrap().0, [9, 1, 9, 1, 9, 1]);
        assert!(chunk(&h, 5).is_err());
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    #[test]
    fn retile_with_overflowing_chunk_shape() {
        let nd = Nd {
            shape: vec![2, 2],
            chunk: vec![u64::MAX / 2, 2],
            block: vec![3, 2],
        };
        assert!(retile(&[0; 64], &nd, 1, 1 << 20).is_err());
    }

    #[test]
    fn frame_of_one_byte_chunks() {
        // 1 MiB in 1-byte chunks would need an 8 MiB offsets chunk (8 GiB for a 1 GiB chunk);
        // the offsets chunk here is a "zeros" special chunk that would expand to it.
        let mut f = vec![0x99, 0xa8];
        f.extend_from_slice(b"b2frame\0");
        // header length, frame length, -, nbytes (1 MiB), cbytes, typesize, -, chunk size.
        f.extend_from_slice(&[
            0x16, 0x00, 0x00, 0xce, 0, 0x10, 0, 0, 0x00, 0x01, 0x00, 0x01,
        ]);
        let mut chunk = vec![0u8; 32];
        chunk[2] = 0x05;
        chunk[3] = 1;
        chunk[4..8].copy_from_slice(&(8u32 << 20).to_le_bytes());
        chunk[12..16].copy_from_slice(&32u32.to_le_bytes());
        chunk[31] = 0x10;
        f.extend_from_slice(&chunk);
        assert!(frame(&f, 1 << 20).is_err());
    }
}
