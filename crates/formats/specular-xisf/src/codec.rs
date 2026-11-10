//! Data block codecs: zlib (own inflate from `specular-png`), LZ4 / LZ4HC (an own block
//! decoder: both write the same raw LZ4 block format), Zstandard (`ruzstd`), and byte
//! un-shuffling.

use std::io::Read;

use rayon::prelude::*;

use crate::{Codec, Compression, Error, Result};

/// Decompresses a whole data block (every sub-block in turn), then un-shuffles it.
/// `expected` is the size the image needs; the stated uncompressed size must match it.
pub(crate) fn decompress(src: &[u8], c: &Compression, limit: u64) -> Result<Vec<u8>> {
    if c.uncompressed > limit {
        return Err(Error::TooLarge);
    }
    let total = usize::try_from(c.uncompressed).map_err(|_| Error::TooLarge)?;
    // A zstd run block gives gigabytes from a few bytes: the size must fit the memory budget
    // (try_reserve alone succeeds on an overcommitting system and the zeroing then aborts).
    let _held = specular_core::budget::reserve(total as u64).map_err(|_| Error::TooLarge)?;
    let mut out = Vec::new();
    out.try_reserve_exact(total).map_err(|_| Error::TooLarge)?;
    if c.subblocks.is_empty() {
        decompress_one(src, c.codec, total, &mut out)?;
    } else {
        let mut at = 0usize;
        let mut sum = 0u64;
        for &(cs, us) in &c.subblocks {
            sum = sum.saturating_add(us);
            if sum > c.uncompressed {
                return Err(Error::Malformed(
                    "sub-blocks larger than the uncompressed size".into(),
                ));
            }
            let cs = usize::try_from(cs).map_err(|_| Error::TooLarge)?;
            let end = at
                .checked_add(cs)
                .filter(|&e| e <= src.len())
                .ok_or(Error::Truncated("compressed sub-block"))?;
            decompress_one(&src[at..end], c.codec, us as usize, &mut out)?;
            at = end;
        }
    }
    if out.len() != total {
        return Err(Error::Codec(format!(
            "decompressed {} bytes, expected {total}",
            out.len()
        )));
    }
    Ok(match c.shuffle_item {
        Some(n) if n > 1 => unshuffle(&out, n as usize),
        _ => out,
    })
}

/// Appends exactly `n` decompressed bytes of `src` to `out`.
fn decompress_one(src: &[u8], codec: Codec, n: usize, out: &mut Vec<u8>) -> Result<()> {
    match codec {
        Codec::Zlib => {
            // Headroom over `n`: the inflater gives up once its buffer, which must keep a
            // match's worth of room past the output, would grow beyond the limit.
            let v = specular_png::inflate_zlib(src, n.saturating_add(1024))
                .map_err(|e| Error::Codec(format!("zlib: {e}")))?;
            if v.len() != n {
                return Err(Error::Codec(format!(
                    "zlib: {} bytes, expected {n}",
                    v.len()
                )));
            }
            out.extend_from_slice(&v);
        }
        Codec::Lz4 | Codec::Lz4Hc => lz4_block(src, n, out)?,
        Codec::Zstd => {
            let start = out.len();
            let dec = ruzstd::decoding::StreamingDecoder::new(src)
                .map_err(|e| Error::Codec(format!("zstd: {e}")))?;
            dec.take(n as u64 + 1)
                .read_to_end(out)
                .map_err(|e| Error::Codec(format!("zstd: {e}")))?;
            if out.len() - start != n {
                return Err(Error::Codec(format!(
                    "zstd: {} bytes, expected {n}",
                    out.len() - start
                )));
            }
        }
    }
    Ok(())
}

/// Decodes one raw LZ4 block (no frame, no stored size) of exactly `n` bytes onto `out`.
pub(crate) fn lz4_block(src: &[u8], n: usize, out: &mut Vec<u8>) -> Result<()> {
    let err = |m: &str| Error::Codec(format!("lz4: {m}"));
    let base = out.len();
    let end = base + n;
    let mut i = 0usize;
    let length = |i: &mut usize, mut len: usize| -> Result<usize> {
        if len == 15 {
            loop {
                let b = *src.get(*i).ok_or_else(|| err("truncated length"))?;
                *i += 1;
                len = len.checked_add(b as usize).ok_or_else(|| err("length"))?;
                if b != 255 {
                    break;
                }
            }
        }
        Ok(len)
    };
    loop {
        let token = *src.get(i).ok_or_else(|| err("truncated"))?;
        i += 1;
        let lit = length(&mut i, (token >> 4) as usize)?;
        let lit_end = i.checked_add(lit).ok_or_else(|| err("literals"))?;
        if lit_end > src.len() || out.len() + lit > end {
            return Err(err("literals overrun"));
        }
        out.extend_from_slice(&src[i..lit_end]);
        i = lit_end;
        if i == src.len() {
            break;
        }
        let off = src
            .get(i..i + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
            .ok_or_else(|| err("truncated offset"))?;
        i += 2;
        let len = length(&mut i, (token & 15) as usize)? + 4;
        if off == 0 || off > out.len() - base {
            return Err(err("bad offset"));
        }
        if out.len() + len > end {
            return Err(err("match overrun"));
        }
        let from = out.len() - off;
        if off >= len {
            out.extend_from_within(from..from + len);
        } else {
            for k in 0..len {
                let b = out[from + k];
                out.push(b);
            }
        }
    }
    if out.len() != end {
        return Err(err("short block"));
    }
    Ok(())
}

/// Reverses XISF byte shuffling: the shuffled block holds byte 0 of every item, then byte 1
/// of every item, …; bytes past the last whole item are stored as is at the end.
pub(crate) fn unshuffle(src: &[u8], item: usize) -> Vec<u8> {
    let count = src.len() / item;
    let mut out = vec![0u8; src.len()];
    let whole = count * item;
    const ITEMS: usize = 1 << 14;
    out[..whole]
        .par_chunks_mut(ITEMS * item)
        .enumerate()
        .for_each(|(k, o)| {
            let i0 = k * ITEMS;
            for j in 0..item {
                let plane = &src[j * count + i0..];
                for (d, &s) in o.iter_mut().skip(j).step_by(item).zip(plane) {
                    *d = s;
                }
            }
        });
    out[whole..].copy_from_slice(&src[whole..]);
    out
}
