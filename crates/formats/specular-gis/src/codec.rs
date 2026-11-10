//! Block decompressors Parquet and Arrow IPC need beside the in-tree ones (Deflate through
//! flate2, Zstandard through ruzstd, Brotli through specular-brotli): Snappy (raw blocks)
//! and LZ4 (raw blocks, the Hadoop framing and the LZ4 frame format). Own code from the
//! format descriptions; every copy is bounds-checked and the output is capped at the size
//! the container declares.

/// Raw Snappy: a varint length, then literals and back-references.
pub fn snappy(src: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    let mut i = 0usize;
    let mut len = 0usize;
    let mut shift = 0;
    loop {
        let b = *src.get(i).ok_or("Snappy data is truncated")?;
        i += 1;
        len |= usize::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift > 35 {
            return Err("Snappy length is malformed".into());
        }
    }
    if len > limit {
        return Err("Snappy block is larger than declared".into());
    }
    // Snappy expands at most about 21 times (a 3-byte copy of 64 bytes).
    let mut out = Vec::with_capacity(len.min(src.len().saturating_mul(22)));
    while i < src.len() {
        let tag = src[i];
        i += 1;
        let (n, off) = match tag & 3 {
            0 => {
                let mut n = usize::from(tag >> 2);
                if n >= 60 {
                    let k = n - 59;
                    let b = src.get(i..i + k).ok_or("Snappy literal is truncated")?;
                    n = b
                        .iter()
                        .rev()
                        .fold(0usize, |a, &x| (a << 8) | usize::from(x));
                    i += k;
                }
                n += 1;
                let lit = src
                    .get(i..i.checked_add(n).ok_or("bad literal")?)
                    .ok_or("Snappy literal is truncated")?;
                if out.len() + n > len {
                    return Err("Snappy data overruns its length".into());
                }
                out.extend_from_slice(lit);
                i += n;
                continue;
            }
            1 => {
                let b = *src.get(i).ok_or("Snappy copy is truncated")?;
                i += 1;
                (
                    usize::from((tag >> 2) & 7) + 4,
                    (usize::from(tag >> 5) << 8) | usize::from(b),
                )
            }
            2 => {
                let b = src.get(i..i + 2).ok_or("Snappy copy is truncated")?;
                i += 2;
                (
                    usize::from(tag >> 2) + 1,
                    usize::from(u16::from_le_bytes([b[0], b[1]])),
                )
            }
            _ => {
                let b = src.get(i..i + 4).ok_or("Snappy copy is truncated")?;
                i += 4;
                (
                    usize::from(tag >> 2) + 1,
                    u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize,
                )
            }
        };
        copy_back(&mut out, off, n, len)?;
    }
    if out.len() != len {
        return Err("Snappy data is shorter than its length".into());
    }
    Ok(out)
}

fn copy_back(out: &mut Vec<u8>, off: usize, n: usize, limit: usize) -> Result<(), String> {
    if off == 0 || off > out.len() {
        return Err("a back-reference points before the data".into());
    }
    if out.len() + n > limit {
        return Err("decompressed data overruns its size".into());
    }
    let start = out.len() - off;
    if off >= n {
        out.extend_from_within(start..start + n);
    } else {
        for k in 0..n {
            let b = out[start + k];
            out.push(b);
        }
    }
    Ok(())
}

/// One raw LZ4 block, appended to `out` (whose earlier bytes may be referenced), up to
/// `limit` bytes in all.
pub fn lz4_block_into(src: &[u8], out: &mut Vec<u8>, limit: usize) -> Result<(), String> {
    let mut i = 0usize;
    let ext = |i: &mut usize, mut n: usize| -> Result<usize, String> {
        if n == 15 {
            loop {
                let b = *src.get(*i).ok_or("LZ4 length is truncated")?;
                *i += 1;
                n = n
                    .checked_add(usize::from(b))
                    .ok_or("LZ4 length overflows")?;
                if b != 255 {
                    break;
                }
            }
        }
        Ok(n)
    };
    while i < src.len() {
        let token = src[i];
        i += 1;
        let lit = ext(&mut i, usize::from(token >> 4))?;
        let l = src
            .get(i..i.checked_add(lit).ok_or("bad literal")?)
            .ok_or("LZ4 literal is truncated")?;
        if out.len() + lit > limit {
            return Err("LZ4 data overruns its size".into());
        }
        out.extend_from_slice(l);
        i += lit;
        if i >= src.len() {
            break; // the last sequence has literals only
        }
        let o = src.get(i..i + 2).ok_or("LZ4 offset is truncated")?;
        let off = usize::from(u16::from_le_bytes([o[0], o[1]]));
        i += 2;
        let n = ext(&mut i, usize::from(token & 15))? + 4;
        copy_back(out, off, n, limit)?;
    }
    Ok(())
}

/// LZ4 as Parquet writes it: a raw block (LZ4_RAW), or for the older LZ4 codec the Hadoop
/// framing (big-endian decompressed and compressed sizes before each block), falling back
/// to a raw block when the framing does not add up.
pub fn lz4_parquet(src: &[u8], size: usize, raw: bool) -> Result<Vec<u8>, String> {
    if !raw && let Ok(v) = lz4_hadoop(src, size) {
        return Ok(v);
    }
    // An LZ4 block expands at most 255 times: the declared size is not trusted further.
    let mut out = Vec::with_capacity(size.min(src.len().saturating_mul(255)));
    lz4_block_into(src, &mut out, size)?;
    Ok(out)
}

fn lz4_hadoop(src: &[u8], size: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(size.min(src.len().saturating_mul(255)));
    let mut i = 0usize;
    while i < src.len() {
        let h = src.get(i..i + 8).ok_or("short")?;
        let dsize = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as usize;
        let csize = u32::from_be_bytes([h[4], h[5], h[6], h[7]]) as usize;
        i += 8;
        let block = src
            .get(i..i.checked_add(csize).ok_or("bad")?)
            .ok_or("short")?;
        let before = out.len();
        // Checked before anything is allocated: the block size is the file's word.
        if before.saturating_add(dsize) > size || dsize > csize.saturating_mul(255) {
            return Err("framing mismatch".into());
        }
        let mut part = Vec::with_capacity(dsize);
        lz4_block_into(block, &mut part, dsize)?;
        if part.len() != dsize {
            return Err("framing mismatch".into());
        }
        out.extend_from_slice(&part);
        i += csize;
    }
    if out.len() != size {
        return Err("framing mismatch".into());
    }
    Ok(out)
}

/// The LZ4 frame format (Arrow IPC buffers): magic, frame descriptor, blocks (each raw or
/// stored), an end mark; checksums are skipped.
pub fn lz4_frame(src: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    if src.get(..4) != Some(&[0x04, 0x22, 0x4D, 0x18]) {
        return Err("not an LZ4 frame".into());
    }
    let flg = *src.get(4).ok_or("LZ4 frame is truncated")?;
    let mut i = 6; // FLG, BD
    if flg & 0x08 != 0 {
        i += 8; // content size
    }
    if flg & 0x01 != 0 {
        i += 4; // dictionary id
    }
    i += 1; // header checksum
    let block_checksum = flg & 0x10 != 0;
    let mut out = Vec::new();
    loop {
        let h = src.get(i..i + 4).ok_or("LZ4 frame is truncated")?;
        let v = u32::from_le_bytes([h[0], h[1], h[2], h[3]]);
        i += 4;
        if v == 0 {
            break;
        }
        let n = (v & 0x7FFF_FFFF) as usize;
        let b = src
            .get(i..i.checked_add(n).ok_or("bad block")?)
            .ok_or("LZ4 frame block is truncated")?;
        if v & 0x8000_0000 != 0 {
            if out.len() + n > limit {
                return Err("LZ4 data overruns its size".into());
            }
            out.extend_from_slice(b);
        } else {
            lz4_block_into(b, &mut out, limit)?;
        }
        i += n + if block_checksum { 4 } else { 0 };
    }
    Ok(out)
}

/// Gzip or zlib (Parquet's GZIP codec is gzip; some writers use zlib).
pub fn inflate(src: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut out = Vec::new();
    let r: Box<dyn Read> = if src.starts_with(&[0x1F, 0x8B]) {
        Box::new(flate2::read::MultiGzDecoder::new(src))
    } else if src.first().is_some_and(|b| b & 0x0F == 8) {
        Box::new(flate2::read::ZlibDecoder::new(src))
    } else {
        Box::new(flate2::read::DeflateDecoder::new(src))
    };
    r.take(limit as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("Deflate data is damaged: {e}"))?;
    if out.len() > limit {
        return Err("Deflate data overruns its size".into());
    }
    Ok(out)
}

/// Zstandard frames.
pub fn zstd(src: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut d = ruzstd::decoding::StreamingDecoder::new(src)
        .map_err(|e| format!("Zstandard data is damaged: {e}"))?;
    let mut out = Vec::new();
    (&mut d)
        .take(limit as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("Zstandard data is damaged: {e}"))?;
    if out.len() > limit {
        return Err("Zstandard data overruns its size".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snappy_literals_and_copies() {
        // "abcabcabcabcX": literal "abc", copy offset 3 length 9, literal "X".
        let src = [
            13u8,
            2 << 2,
            b'a',
            b'b',
            b'c',
            ((9 - 4) << 2) | 1,
            3,
            0,
            b'X',
        ];
        assert_eq!(snappy(&src, 100).unwrap(), b"abcabcabcabcX");
        assert!(snappy(&src, 5).is_err());
        assert!(snappy(&src[..6], 100).is_err());
        // A copy reaching before the start.
        // A 1-byte-offset copy of length 4 with offset 9 into nothing.
        assert!(snappy(&[4, 1, 9], 100).is_err());
    }

    #[test]
    fn lz4_block_and_framings() {
        // Literal "abcd", match offset 4 length 8 ("abcdabcd"), last literals "!".
        let block = [0x44, b'a', b'b', b'c', b'd', 4, 0, 0x10, b'!'];
        let mut out = Vec::new();
        lz4_block_into(&block, &mut out, 100).unwrap();
        assert_eq!(out, b"abcdabcdabcd!");
        assert_eq!(lz4_parquet(&block, 13, true).unwrap(), b"abcdabcdabcd!");
        let mut hadoop = vec![0, 0, 0, 13, 0, 0, 0, block.len() as u8];
        hadoop.extend_from_slice(&block);
        assert_eq!(lz4_parquet(&hadoop, 13, false).unwrap(), b"abcdabcdabcd!");
        let mut frame = vec![0x04, 0x22, 0x4D, 0x18, 0x40, 0x40, 0];
        frame.extend_from_slice(&(block.len() as u32).to_le_bytes());
        frame.extend_from_slice(&block);
        frame.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(lz4_frame(&frame, 100).unwrap(), b"abcdabcdabcd!");
    }
}
