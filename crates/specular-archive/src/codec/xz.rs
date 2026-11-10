//! xz (ZIP method 95, `.tar.xz`): own reader of the container, from the .xz file format
//! specification (version 1.2).
//!
//! A stream is a header (magic, flags naming the check, their CRC32), blocks, an index listing
//! every block's sizes, and a footer; streams may be concatenated, with zero padding between
//! them. A block has a header (its filter chain and, optionally, its sizes; CRC32-protected),
//! the filtered data, padding to four bytes, and the check of its output: none, CRC32, CRC64 or
//! SHA-256, all verified (the other check ids, reserved by the format, are skipped by size).
//!
//! The last filter of a chain is LZMA2, whose range decoder is `lzma_rust2`'s (the one the 7z
//! reader already runs); before it there may be Delta and the BCJ filters, which reuse the 7z
//! reader's ([`Filter`]). Everything else here is this module's own: header, varint and index
//! parsing, the walk over LZMA2 chunk headers that finds where a block's data ends and how much
//! it unpacks to without decoding it, the checks, and the caps.
//!
//! **Parallel blocks.** As every block's unpacked size is known before any is decoded, the
//! output is allocated once and blocks decode on several threads straight into their own part
//! of it (`xz -T`, the default since xz 5.4, writes many blocks; a single-block stream decodes
//! on one thread).

use std::io::Read;

use parking_lot::Mutex;

use super::check::{crc64, sha256};
use super::{CResult, CodecError, Threads, room};
use crate::lzma2mt::Filter;

const MAGIC: [u8; 6] = [0xFD, b'7', b'z', b'X', b'Z', 0];
const FOOTER_MAGIC: [u8; 2] = *b"YZ";
const LZMA2: u64 = 0x21;

/// Whether `b` starts with the xz stream magic.
pub(crate) fn detect(b: &[u8]) -> bool {
    b.starts_with(&MAGIC)
}

/// Bytes of a block's check, by check id.
fn check_size(id: u8) -> usize {
    match id {
        0 => 0,
        1..=3 => 4,
        4..=6 => 8,
        7..=9 => 16,
        10..=12 => 32,
        _ => 64,
    }
}

/// A multibyte integer (7 bits a byte, low first, at most 9 bytes, no redundant zero byte).
fn varint(b: &[u8], at: &mut usize) -> CResult<u64> {
    let mut v = 0u64;
    for i in 0..9 {
        let byte = *b.get(*at).ok_or(CodecError::Truncated)?;
        *at += 1;
        v |= u64::from(byte & 0x7F) << (7 * i);
        if byte & 0x80 == 0 {
            if byte == 0 && i > 0 {
                return Err(CodecError::Corrupt("redundant varint byte"));
            }
            return Ok(v);
        }
    }
    Err(CodecError::Corrupt("varint too long"))
}

fn crc32(b: &[u8]) -> u32 {
    crc32fast::hash(b)
}

fn le32(b: &[u8], at: usize) -> CResult<u32> {
    let w = b.get(at..at + 4).ok_or(CodecError::Truncated)?;
    Ok(u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}

/// One block, found and checked for shape but not yet decoded.
struct Block {
    /// The LZMA2 data, in the input.
    packed: std::ops::Range<usize>,
    /// The check, in the input.
    check: std::ops::Range<usize>,
    unpacked: usize,
    /// Header, data and check: the size the index records.
    unpadded: usize,
    dict: u32,
    /// Filters before LZMA2, in the order the header lists them (the decoder applies them last
    /// to first).
    filters: Vec<Filter>,
}

/// The dictionary size an LZMA2 properties byte names.
fn lzma2_dict(p: u8) -> CResult<u32> {
    match p {
        0..=39 => Ok((2 | u32::from(p & 1)) << (p / 2 + 11)),
        40 => Ok(u32::MAX),
        _ => Err(CodecError::Corrupt("bad LZMA2 dictionary size")),
    }
}

/// Walks the LZMA2 chunk headers from `b[0]` to the end marker: (packed bytes including the end
/// marker, unpacked bytes). Nothing is decoded.
fn lzma2_extent(b: &[u8]) -> CResult<(usize, u64)> {
    let mut at = 0usize;
    let mut total = 0u64;
    let mut first = true;
    loop {
        let control = *b.get(at).ok_or(CodecError::Truncated)?;
        if control == 0 {
            return Ok((at + 1, total));
        }
        let word = |i: usize| -> CResult<u64> {
            let w = b.get(at + i..at + i + 2).ok_or(CodecError::Truncated)?;
            Ok(u64::from(u16::from_be_bytes([w[0], w[1]])) + 1)
        };
        // The first chunk must reset the dictionary (and an LZMA one set the properties).
        if first && !(control == 1 || control >= 0xE0) {
            return Err(CodecError::Corrupt(
                "LZMA2 stream without a dictionary reset",
            ));
        }
        first = false;
        let (unpacked, data, head) = match control {
            1 | 2 => {
                let n = word(1)?;
                (n, n, 3)
            }
            0x80.. => (
                (u64::from(control & 0x1F) << 16) + word(1)?,
                word(3)?,
                if control >= 0xC0 { 6 } else { 5 },
            ),
            _ => return Err(CodecError::Corrupt("bad LZMA2 chunk header")),
        };
        at = at
            .checked_add(head + data as usize)
            .ok_or(CodecError::Truncated)?;
        if at > b.len() {
            return Err(CodecError::Truncated);
        }
        total += unpacked;
    }
}

/// Parses the block whose header starts at `at`, returning it and the offset after its check.
fn block(b: &[u8], at: usize, check_id: u8, max_memory: u64) -> CResult<(Block, usize)> {
    let size = (usize::from(*b.get(at).ok_or(CodecError::Truncated)?) + 1) * 4;
    let head = b.get(at..at + size).ok_or(CodecError::Truncated)?;
    if crc32(&head[..size - 4]) != le32(head, size - 4)? {
        return Err(CodecError::Corrupt("block header CRC mismatch"));
    }
    let flags = head[1];
    if flags & 0x3C != 0 {
        return Err(CodecError::Unsupported("block header flags"));
    }
    let body = &head[..size - 4];
    let mut p = 2usize;
    let declared_packed = if flags & 0x40 != 0 {
        Some(varint(body, &mut p)?)
    } else {
        None
    };
    let declared_unpacked = if flags & 0x80 != 0 {
        Some(varint(body, &mut p)?)
    } else {
        None
    };
    let count = usize::from(flags & 3) + 1;
    let mut filters = Vec::new();
    let mut dict = None;
    for i in 0..count {
        let id = varint(body, &mut p)?;
        let props_len = usize::try_from(varint(body, &mut p)?)
            .map_err(|_| CodecError::Corrupt("bad filter properties"))?;
        let props = body
            .get(p..p.saturating_add(props_len))
            .ok_or(CodecError::Corrupt("filter properties past the header"))?;
        p += props_len;
        let last = i + 1 == count;
        match id {
            LZMA2 if last && props_len == 1 => dict = Some(lzma2_dict(props[0])?),
            LZMA2 => return Err(CodecError::Corrupt("misplaced LZMA2 filter")),
            _ if last => return Err(CodecError::Unsupported("filter chain not ending in LZMA2")),
            3 if props_len == 1 => filters.push(Filter::Delta(usize::from(props[0]) + 1)),
            4..=0x0B if props_len == 0 || (props_len == 4 && props == [0; 4]) => {
                filters.push(match id {
                    4 => Filter::X86,
                    5 => Filter::Ppc,
                    6 => Filter::Ia64,
                    7 => Filter::Arm,
                    8 => Filter::ArmThumb,
                    9 => Filter::Sparc,
                    0x0A => Filter::Arm64,
                    _ => Filter::RiscV,
                });
            }
            4..=0x0B => return Err(CodecError::Unsupported("BCJ start offset")),
            _ => return Err(CodecError::Unsupported("filter")),
        }
    }
    if body[p..].iter().any(|&x| x != 0) {
        return Err(CodecError::Corrupt("block header padding"));
    }
    let dict = dict.ok_or(CodecError::Corrupt("no LZMA2 filter"))?;
    let start = at + size;
    let (packed_len, unpacked) = lzma2_extent(&b[start..])?;
    if declared_packed.is_some_and(|d| d != packed_len as u64)
        || declared_unpacked.is_some_and(|d| d != unpacked)
    {
        return Err(CodecError::Corrupt("block sizes differ from its header"));
    }
    let unpacked = usize::try_from(unpacked).map_err(|_| CodecError::Limit)?;
    let dict = u64::from(dict).min(unpacked as u64).max(4096);
    if dict > max_memory {
        return Err(CodecError::Unsupported(
            "LZMA2 dictionary larger than allowed",
        ));
    }
    let mut end = start + packed_len;
    let pad = (4 - (size + packed_len) % 4) % 4;
    let padding = b.get(end..end + pad).ok_or(CodecError::Truncated)?;
    if padding.iter().any(|&x| x != 0) {
        return Err(CodecError::Corrupt("block padding"));
    }
    end += pad;
    let check = end..end + check_size(check_id);
    if check.end > b.len() {
        return Err(CodecError::Truncated);
    }
    end = check.end;
    Ok((
        Block {
            packed: start..start + packed_len,
            check,
            unpacked,
            unpadded: size + packed_len + check_size(check_id),
            dict: dict as u32,
            filters,
        },
        end,
    ))
}

/// A reader of block `blk`'s unpacked bytes (LZMA2, then its filters).
fn block_reader<'b>(b: &'b [u8], blk: &Block) -> Box<dyn Read + 'b> {
    let lzma2 = lzma_rust2::Lzma2Reader::new(&b[blk.packed.clone()], blk.dict, None);
    let mut r: Box<dyn Read + 'b> = Box::new(lzma2);
    for f in blk.filters.iter().rev() {
        r = f.wrap(r);
    }
    r
}

/// Verifies block `blk`'s check over its unpacked bytes `data`.
fn verify(b: &[u8], blk: &Block, check_id: u8, data: &[u8]) -> CResult<()> {
    let want = &b[blk.check.clone()];
    let ok = match check_id {
        1 => crc32(data).to_le_bytes() == want,
        4 => crc64(data).to_le_bytes() == want,
        10 => sha256(data) == want,
        // None, and the reserved ids: nothing to verify.
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(CodecError::Checksum)
    }
}

/// Decodes one block into `dst` (exactly its unpacked size) and verifies its check.
fn decode_block(b: &[u8], blk: &Block, check_id: u8, dst: &mut [u8]) -> CResult<()> {
    let mut r = block_reader(b, blk);
    let mut got = 0usize;
    while got < dst.len() {
        match r.read(&mut dst[got..]) {
            Ok(0) => return Err(CodecError::Truncated),
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(CodecError::Corrupt("bad LZMA2 data")),
        }
    }
    // The chunk walk sized the block, so the stream must end here.
    let mut extra = [0u8; 1];
    if !matches!(r.read(&mut extra), Ok(0)) {
        return Err(CodecError::Corrupt("block longer than its chunks"));
    }
    verify(b, blk, check_id, dst)
}

/// Decodes one block onto the end of `out`, which grows as the data decodes (the sizes the
/// chunk headers declare are not trusted with an allocation up front), and verifies its check.
fn append_block(b: &[u8], blk: &Block, check_id: u8, out: &mut Vec<u8>) -> CResult<()> {
    let start = out.len();
    super::lzma::read_exact_into(block_reader(b, blk), blk.unpacked, out).map_err(|e| match e {
        CodecError::Limit => CodecError::Corrupt("block longer than its chunks"),
        CodecError::Corrupt(_) => CodecError::Corrupt("bad LZMA2 data"),
        e => e,
    })?;
    verify(b, blk, check_id, &out[start..])
}

/// Decodes the xz data `data` (one or more concatenated streams), appending to `out` without
/// letting it pass `limit` bytes. `max_memory` caps one block's dictionary. Bytes after the last
/// stream (and its padding) are ignored.
pub(crate) fn decode(
    data: &[u8],
    out: &mut Vec<u8>,
    limit: usize,
    max_memory: u64,
    threads: Threads,
) -> CResult<()> {
    let mut at = 0usize;
    loop {
        at = stream(data, at, out, limit, max_memory, threads)?;
        // Stream padding: zero bytes, a multiple of four.
        let zeros = data[at..].iter().take_while(|&&x| x == 0).count();
        let next = at + zeros;
        if zeros % 4 != 0 || !data[next..].starts_with(&MAGIC) {
            return Ok(());
        }
        at = next;
    }
}

/// One stream from `at`: returns the offset after its footer.
fn stream(
    b: &[u8],
    at: usize,
    out: &mut Vec<u8>,
    limit: usize,
    max_memory: u64,
    threads: Threads,
) -> CResult<usize> {
    let head = b.get(at..at + 12).ok_or(CodecError::Truncated)?;
    if head[..6] != MAGIC {
        return Err(CodecError::Corrupt("not an xz stream"));
    }
    let flags = [head[6], head[7]];
    if crc32(&flags) != le32(head, 8)? {
        return Err(CodecError::Corrupt("stream header CRC mismatch"));
    }
    if flags[0] != 0 || flags[1] & 0xF0 != 0 {
        return Err(CodecError::Unsupported("stream flags"));
    }
    let check_id = flags[1];

    // Find every block (cheap: headers and chunk headers only), then decode them.
    let mut blocks = Vec::new();
    let mut p = at + 12;
    let mut total = 0usize;
    loop {
        let first = *b.get(p).ok_or(CodecError::Truncated)?;
        if first == 0 {
            break;
        }
        let (blk, end) = block(b, p, check_id, max_memory)?;
        total = total.checked_add(blk.unpacked).ok_or(CodecError::Limit)?;
        room(out, total, limit)?;
        blocks.push(blk);
        p = end;
    }

    // Index: indicator, record count, (unpadded, unpacked) per block, padding, CRC32.
    let index_start = p;
    let mut q = p + 1;
    let records = varint(b, &mut q)?;
    if records != blocks.len() as u64 {
        return Err(CodecError::Corrupt("index does not match the blocks"));
    }
    for blk in &blocks {
        let unpadded = varint(b, &mut q)?;
        let unpacked = varint(b, &mut q)?;
        if unpadded != blk.unpadded as u64 || unpacked != blk.unpacked as u64 {
            return Err(CodecError::Corrupt("index does not match the blocks"));
        }
    }
    while !(q - index_start).is_multiple_of(4) {
        if *b.get(q).ok_or(CodecError::Truncated)? != 0 {
            return Err(CodecError::Corrupt("index padding"));
        }
        q += 1;
    }
    let index = b.get(index_start..q).ok_or(CodecError::Truncated)?;
    if crc32(index) != le32(b, q)? {
        return Err(CodecError::Corrupt("index CRC mismatch"));
    }
    q += 4;
    let index_len = q - index_start;

    // Footer: CRC32 of (backward size, flags), backward size, flags, "YZ".
    let foot = b.get(q..q + 12).ok_or(CodecError::Truncated)?;
    if foot[10..] != FOOTER_MAGIC
        || crc32(&foot[4..10]) != le32(foot, 0)?
        || foot[8..10] != flags
        || (u64::from(le32(foot, 4)?) + 1) * 4 != index_len as u64
    {
        return Err(CodecError::Corrupt("bad stream footer"));
    }

    let base = out.len();
    let result = if threads.n > 1 && blocks.len() > 1 {
        // In parallel: one allocation for the stream, each block into its own part of it. The
        // declared sizes are reserved from the memory budget first, for as long as the blocks
        // decode (the caller owns the bytes after).
        let _budget = specular_core::budget::reserve(total as u64)
            .map_err(|e| CodecError::Memory(e.to_string()))?;
        out.resize(base + total, 0);
        let mut parts = Vec::with_capacity(blocks.len());
        let mut rest = &mut out[base..];
        for blk in &blocks {
            let (part, tail) = rest.split_at_mut(blk.unpacked);
            parts.push(Mutex::new(part));
            rest = tail;
        }
        let error: Mutex<Option<CodecError>> = Mutex::new(None);
        threads.run("arc-xz", blocks.len(), |i| {
            if error.lock().is_some() {
                return;
            }
            let mut part = parts[i].lock();
            if let Err(e) = decode_block(b, &blocks[i], check_id, &mut part) {
                error.lock().get_or_insert(e);
            }
        });
        error.into_inner().map_or(Ok(()), Err)
    } else {
        // One after another, each growing the output as it decodes.
        blocks
            .iter()
            .try_for_each(|blk| append_block(b, blk, check_id, out))
    };
    if let Err(e) = result {
        out.truncate(base);
        return Err(e);
    }
    Ok(q + 12)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put_varint(x: &mut Vec<u8>, mut v: u64) {
        while v >= 0x80 {
            x.push((v as u8) | 0x80);
            v >>= 7;
        }
        x.push(v as u8);
    }

    /// A well-formed xz stream (no check) of `blocks` blocks of `chunks` LZMA chunks each,
    /// every chunk declaring 2 MiB unpacked from one packed byte: gigabytes declared by a file
    /// of a few KiB, whose data fails at the first chunk.
    fn liar(blocks: usize, chunks: usize) -> Vec<u8> {
        let mut x = MAGIC.to_vec();
        x.extend_from_slice(&[0, 0]);
        x.extend_from_slice(&crc32(&[0, 0]).to_le_bytes());
        let mut records = Vec::new();
        for _ in 0..blocks {
            // One filter: LZMA2 (id 0x21), one property byte (1 MiB dictionary), padding.
            let mut h = vec![2u8, 0, 0x21, 1, 16, 0, 0, 0];
            let c = crc32(&h);
            h.extend_from_slice(&c.to_le_bytes());
            let mut body = Vec::new();
            for k in 0..chunks {
                body.push(if k == 0 { 0xFF } else { 0x9F });
                body.extend_from_slice(&0xFFFFu16.to_be_bytes());
                body.extend_from_slice(&0u16.to_be_bytes());
                if k == 0 {
                    body.push(0x5D);
                }
                body.push(0);
            }
            body.push(0);
            records.push((h.len() + body.len(), chunks as u64 * (2 << 20)));
            x.extend_from_slice(&h);
            x.extend_from_slice(&body);
            while !x.len().is_multiple_of(4) {
                x.push(0);
            }
        }
        let start = x.len();
        x.push(0);
        put_varint(&mut x, blocks as u64);
        for (unpadded, unpacked) in records {
            put_varint(&mut x, unpadded as u64);
            put_varint(&mut x, unpacked);
        }
        while !(x.len() - start).is_multiple_of(4) {
            x.push(0);
        }
        let c = crc32(&x[start..]);
        x.extend_from_slice(&c.to_le_bytes());
        let backward = ((x.len() - start) / 4 - 1) as u32;
        let mut foot = backward.to_le_bytes().to_vec();
        foot.extend_from_slice(&[0, 0]);
        x.extend_from_slice(&crc32(&foot).to_le_bytes());
        x.extend_from_slice(&foot);
        x.extend_from_slice(&FOOTER_MAGIC);
        x
    }

    /// 1 GiB declared by a 3 KiB stream: decoded one block after another, the output grows
    /// only with what decodes (it used to be sized, and zeroed, from the chunk headers).
    #[test]
    fn lying_chunk_sizes_allocate_nothing() {
        let x = liar(1, 512);
        assert!(x.len() < 4096);
        let mut out = Vec::new();
        let r = specular_core::budget::with_limit(64 << 20, || {
            decode(&x, &mut out, usize::MAX, 1 << 30, Threads::ONE)
        });
        assert!(
            matches!(
                r,
                Err(CodecError::Corrupt("bad LZMA2 data") | CodecError::Truncated)
            ),
            "{r:?}"
        );
        assert!(out.capacity() <= 16 << 20, "{}", out.capacity());
        // In parallel the blocks are sized up front: only within the memory budget.
        let x = liar(2, 256);
        let two = Threads { n: 2, start: None };
        let r = specular_core::budget::with_limit(64 << 20, || {
            decode(&x, &mut out, usize::MAX, 1 << 30, two)
        });
        assert!(matches!(r, Err(CodecError::Memory(_))), "{r:?}");
        assert!(out.capacity() <= 16 << 20, "{}", out.capacity());
    }

    #[test]
    fn varints() {
        let mut at = 0;
        assert_eq!(varint(&[0x7F], &mut at), Ok(0x7F));
        let mut at = 0;
        assert_eq!(varint(&[0x80, 0x01], &mut at), Ok(0x80));
        assert_eq!(at, 2);
        let mut at = 0;
        assert!(varint(&[0x80, 0x00], &mut at).is_err());
        let mut at = 0;
        assert!(varint(&[0xFF; 10], &mut at).is_err());
        let mut at = 0;
        assert_eq!(varint(&[0x80], &mut at), Err(CodecError::Truncated));
    }

    #[test]
    fn dictionary_sizes() {
        assert_eq!(lzma2_dict(0), Ok(4096));
        assert_eq!(lzma2_dict(1), Ok(6144));
        assert_eq!(lzma2_dict(16), Ok(1 << 20));
        assert_eq!(lzma2_dict(40), Ok(u32::MAX));
        assert!(lzma2_dict(41).is_err());
    }

    #[test]
    fn chunk_walk() {
        // An uncompressed chunk of 3 bytes with a dictionary reset, then the end marker.
        let b = [1, 0, 2, b'a', b'b', b'c', 0];
        assert_eq!(lzma2_extent(&b), Ok((7, 3)));
        assert_eq!(lzma2_extent(&b[..5]), Err(CodecError::Truncated));
        // No dictionary reset first.
        assert!(lzma2_extent(&[2, 0, 0, b'a', 0]).is_err());
    }
}
