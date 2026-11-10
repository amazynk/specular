//! The filter pipeline, run in reverse: deflate, shuffle, Fletcher-32 (verified and
//! stripped), N-bit, scale-offset, LZF, LZ4, Zstandard, bitshuffle (with LZ4 or Zstandard),
//! Blosc and Blosc2 (BloscLZ, LZ4, zlib, Zstandard, byte and bit shuffling).

use std::io::Read;

use crate::layout::Filter;
use crate::{Error, Result};

pub(crate) const DEFLATE: u16 = 1;
pub(crate) const SHUFFLE: u16 = 2;
pub(crate) const FLETCHER32: u16 = 3;
pub(crate) const SZIP: u16 = 4;
pub(crate) const NBIT: u16 = 5;
pub(crate) const SCALEOFFSET: u16 = 6;
pub(crate) const LZF: u16 = 32000;
pub(crate) const BLOSC: u16 = 32001;
pub(crate) const LZ4: u16 = 32004;
pub(crate) const BITSHUFFLE: u16 = 32008;
pub(crate) const ZSTD: u16 = 32015;
pub(crate) const BLOSC2: u16 = 32026;

pub(crate) fn name(id: u16) -> &'static str {
    match id {
        DEFLATE => "deflate",
        SHUFFLE => "shuffle",
        FLETCHER32 => "fletcher32",
        SZIP => "szip",
        NBIT => "nbit",
        SCALEOFFSET => "scaleoffset",
        LZF => "lzf",
        BLOSC => "blosc",
        LZ4 => "lz4",
        BITSHUFFLE => "bitshuffle",
        ZSTD => "zstd",
        BLOSC2 => "blosc2",
        _ => "unknown",
    }
}

/// Whether every filter in the pipeline can be undone here.
pub(crate) fn supported(filters: &[Filter]) -> std::result::Result<(), String> {
    for f in filters {
        if !matches!(
            f.id,
            DEFLATE
                | SHUFFLE
                | FLETCHER32
                | SZIP
                | NBIT
                | SCALEOFFSET
                | LZF
                | BLOSC
                | LZ4
                | BITSHUFFLE
                | ZSTD
                | BLOSC2
        ) {
            return Err(format!("{} filter ({})", name(f.id), f.id));
        }
    }
    Ok(())
}

fn bad(what: &str) -> Error {
    Error::Malformed(format!("{what} stream"))
}

/// Undoes the pipeline on one stored chunk; `expected` is the decoded chunk size and
/// `elem` the element size. Filters whose bit is set in `mask` were skipped when writing.
pub(crate) fn decode(
    filters: &[Filter],
    mask: u32,
    input: &[u8],
    expected: usize,
    elem: usize,
) -> Result<Vec<u8>> {
    let mut buf: Option<Vec<u8>> = None;
    for (i, f) in filters.iter().enumerate().rev() {
        if i < 32 && mask & (1 << i) != 0 {
            continue;
        }
        let cur: &[u8] = buf.as_deref().unwrap_or(input);
        let out = match f.id {
            DEFLATE => specular_png::inflate_zlib(cur, expected).map_err(|_| bad("deflate"))?,
            SHUFFLE => {
                let size = f.params.first().map_or(elem, |&s| s as usize);
                unshuffle(cur, size)
            }
            FLETCHER32 => fletcher32_strip(cur)?,
            LZF => lzf(cur, expected)?,
            LZ4 => hdf5_lz4(cur, expected)?,
            ZSTD => zstd(cur, expected)?,
            BITSHUFFLE => bitshuffle_filter(cur, &f.params, elem, expected)?,
            BLOSC => blosc(cur, expected)?,
            BLOSC2 => crate::blosc2::decode(cur, expected)?,
            SZIP => szip(cur, &f.params, expected)?,
            NBIT => crate::packed::nbit(cur, &f.params, expected)?,
            SCALEOFFSET => crate::packed::scale_offset(cur, &f.params, expected)?,
            id => return Err(Error::Unsupported(format!("{} filter ({id})", name(id)))),
        };
        buf = Some(out);
    }
    Ok(buf.unwrap_or_else(|| input.to_vec()))
}

/// SZIP (libaec): a little-endian u32 of the decoded size, then the raw AEC stream;
/// the parameters are the options mask, pixels per block, bits per pixel and pixels per scanline.
fn szip(src: &[u8], params: &[u32], expected: usize) -> Result<Vec<u8>> {
    let [
        options,
        pixels_per_block,
        bits_per_pixel,
        pixels_per_scanline,
        ..,
    ] = *params
    else {
        return Err(bad("SZIP parameters"));
    };
    let head = src.get(..4).ok_or_else(|| bad("SZIP"))?;
    let size = u32::from_le_bytes([head[0], head[1], head[2], head[3]]) as usize;
    if size > expected.max(1) * 2 + 64 {
        return Err(bad("SZIP"));
    }
    let p = specular_aec::szip::SzParams {
        options,
        bits_per_pixel,
        pixels_per_block,
        pixels_per_scanline,
    };
    specular_aec::szip::decode(&src[4..], &p, size).map_err(|e| match e {
        specular_aec::Error::Params(w) => Error::Unsupported(w.to_owned()),
        _ => bad("SZIP"),
    })
}

/// Byte un-shuffle: `n` elements of `size` bytes stored as `size` planes of `n` bytes, the
/// bytes past the last whole element kept as they are.
pub(crate) fn unshuffle(src: &[u8], size: usize) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    unshuffle_into(src, size, &mut out);
    out
}

pub(crate) fn unshuffle_into(src: &[u8], size: usize, out: &mut [u8]) {
    if size <= 1 || src.len() < size {
        out.copy_from_slice(src);
        return;
    }
    let n = src.len() / size;
    for j in 0..size {
        let plane = &src[j * n..(j + 1) * n];
        for (i, &b) in plane.iter().enumerate() {
            out[i * size + j] = b;
        }
    }
    let tail = n * size;
    out[tail..].copy_from_slice(&src[tail..]);
}

/// HDF5's Fletcher-32 over big-endian 16-bit words.
pub(crate) fn fletcher32(data: &[u8]) -> u32 {
    let (mut s1, mut s2) = (0u32, 0u32);
    let words = data.chunks_exact(2);
    let rest = words.remainder();
    // 360 words (720 bytes) per block; iterated in place, not collected (8 bytes per word).
    for block in data[..data.len() - rest.len()].chunks(720) {
        for w in block.chunks_exact(2) {
            s1 += (u32::from(w[0]) << 8) | u32::from(w[1]);
            s2 += s1;
        }
        s1 = (s1 & 0xffff) + (s1 >> 16);
        s2 = (s2 & 0xffff) + (s2 >> 16);
    }
    if let [b] = rest {
        s1 += u32::from(*b) << 8;
        s2 += s1;
        s1 = (s1 & 0xffff) + (s1 >> 16);
        s2 = (s2 & 0xffff) + (s2 >> 16);
    }
    s1 = (s1 & 0xffff) + (s1 >> 16);
    s2 = (s2 & 0xffff) + (s2 >> 16);
    (s2 << 16) | s1
}

fn fletcher32_strip(src: &[u8]) -> Result<Vec<u8>> {
    let n = src.len().checked_sub(4).ok_or_else(|| bad("fletcher32"))?;
    let (body, stored) = src.split_at(n);
    let stored = u32::from_le_bytes([stored[0], stored[1], stored[2], stored[3]]);
    let sum = fletcher32(body);
    if stored != sum && stored != sum.swap_bytes() {
        return Err(Error::Malformed("Fletcher-32 checksum mismatch".into()));
    }
    Ok(body.to_vec())
}

/// LZF (liblzf's format, as the h5py filter writes it).
pub(crate) fn lzf(src: &[u8], max: usize) -> Result<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(max);
    let mut i = 0usize;
    while i < src.len() {
        let ctrl = src[i] as usize;
        i += 1;
        if ctrl < 32 {
            let n = ctrl + 1;
            let lit = src.get(i..i + n).ok_or_else(|| bad("LZF"))?;
            if out.len() + n > max {
                return Err(bad("LZF"));
            }
            out.extend_from_slice(lit);
            i += n;
        } else {
            let mut len = ctrl >> 5;
            if len == 7 {
                len += *src.get(i).ok_or_else(|| bad("LZF"))? as usize;
                i += 1;
            }
            let back = ((ctrl & 0x1f) << 8) + *src.get(i).ok_or_else(|| bad("LZF"))? as usize + 1;
            i += 1;
            let len = len + 2;
            if back > out.len() || out.len() + len > max {
                return Err(bad("LZF"));
            }
            copy_match(&mut out, back, len);
        }
    }
    Ok(out)
}

/// Appends `len` bytes copied from `back` bytes before the end (overlap allowed).
fn copy_match(out: &mut Vec<u8>, back: usize, len: usize) {
    let start = out.len() - back;
    if back >= len {
        out.extend_from_within(start..start + len);
    } else {
        for k in 0..len {
            let b = out[start + k];
            out.push(b);
        }
    }
}

/// One raw LZ4 block, decoding to exactly `n` bytes.
pub(crate) fn lz4_block(src: &[u8], n: usize, out: &mut Vec<u8>) -> Result<()> {
    let end = out.len() + n;
    let base = out.len();
    let mut i = 0usize;
    let byte = |i: usize| src.get(i).copied().ok_or_else(|| bad("LZ4"));
    loop {
        let token = byte(i)? as usize;
        i += 1;
        let mut lit = token >> 4;
        if lit == 15 {
            loop {
                let b = byte(i)? as usize;
                i += 1;
                lit += b;
                if b != 255 {
                    break;
                }
            }
        }
        let l = src.get(i..i + lit).ok_or_else(|| bad("LZ4"))?;
        if out.len() + lit > end {
            return Err(bad("LZ4"));
        }
        out.extend_from_slice(l);
        i += lit;
        if i >= src.len() {
            break;
        }
        let off = byte(i)? as usize | (byte(i + 1)? as usize) << 8;
        i += 2;
        let mut ml = token & 15;
        if ml == 15 {
            loop {
                let b = byte(i)? as usize;
                i += 1;
                ml += b;
                if b != 255 {
                    break;
                }
            }
        }
        let ml = ml + 4;
        if off == 0 || off > out.len() - base || out.len() + ml > end {
            return Err(bad("LZ4"));
        }
        copy_match(out, off, ml);
    }
    if out.len() != end {
        return Err(bad("LZ4"));
    }
    Ok(())
}

fn be32(b: &[u8], at: usize) -> Result<usize> {
    let s = b.get(at..at + 4).ok_or_else(|| bad("header"))?;
    Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as usize)
}

fn be64(b: &[u8], at: usize) -> Result<u64> {
    let s = b.get(at..at + 8).ok_or_else(|| bad("header"))?;
    Ok(u64::from_be_bytes(s.try_into().unwrap_or([0; 8])))
}

/// The HDF5 LZ4 filter: total size, block size, then sized LZ4 blocks (raw when a block did
/// not compress).
fn hdf5_lz4(src: &[u8], max: usize) -> Result<Vec<u8>> {
    let total = be64(src, 0)?;
    let block = be32(src, 8)?;
    if total > max as u64 || (block == 0 && total > 0) {
        return Err(bad("LZ4"));
    }
    let total = total as usize;
    let mut out = Vec::with_capacity(total);
    let mut i = 12;
    while out.len() < total {
        let want = block.min(total - out.len());
        let cs = be32(src, i)?;
        i += 4;
        let s = src.get(i..i + cs).ok_or_else(|| bad("LZ4"))?;
        if cs == want {
            out.extend_from_slice(s);
        } else {
            lz4_block(s, want, &mut out)?;
        }
        i += cs;
    }
    Ok(out)
}

pub(crate) fn zstd(src: &[u8], max: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(max);
    let mut d = ruzstd::decoding::StreamingDecoder::new(src).map_err(|_| bad("Zstandard"))?;
    (&mut d)
        .take(max as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|_| bad("Zstandard"))?;
    if out.len() > max {
        return Err(bad("Zstandard"));
    }
    Ok(out)
}

/// Bit un-transpose of one block of `n` elements (`n` a multiple of 8) of `size` bytes: bit
/// plane `j*8 + b` holds bit `b` of byte `j` of every element, least significant bit first.
fn bit_untranspose(src: &[u8], n: usize, size: usize, out: &mut [u8]) {
    let row = n / 8;
    out[..n * size].fill(0);
    for j in 0..size {
        for b in 0..8 {
            let plane = &src[(j * 8 + b) * row..(j * 8 + b + 1) * row];
            for (k, &byte) in plane.iter().enumerate() {
                if byte == 0 {
                    continue;
                }
                for t in 0..8 {
                    if byte & (1 << t) != 0 {
                        out[(k * 8 + t) * size + j] |= 1 << b;
                    }
                }
            }
        }
    }
}

/// Undoes bitshuffle over `src` in blocks of `block` elements: whole blocks, then the last
/// partial block rounded down to 8 elements, then the leftover bytes as they are.
pub(crate) fn bitunshuffle(src: &[u8], size: usize, block: usize, out: &mut [u8]) {
    let n = src.len() / size.max(1);
    let mut done = 0usize;
    while done < n {
        let mut m = block.min(n - done);
        m -= m % 8;
        if m == 0 {
            break;
        }
        let a = done * size;
        bit_untranspose(&src[a..a + m * size], m, size, &mut out[a..a + m * size]);
        done += m;
    }
    let tail = done * size;
    out[tail..].copy_from_slice(&src[tail..]);
}

/// The bitshuffle filter's default block size for an element size.
fn bshuf_default_block(size: usize) -> usize {
    ((8192 / size.max(1)) / 8 * 8).max(128)
}

fn bitshuffle_filter(src: &[u8], params: &[u32], elem: usize, max: usize) -> Result<Vec<u8>> {
    let size = params.get(2).map_or(elem, |&s| s as usize).max(1);
    let comp = params.get(4).copied().unwrap_or(0);
    if comp == 0 {
        let block = match params.get(3) {
            Some(&b) if b > 0 => b as usize,
            _ => bshuf_default_block(size),
        };
        let mut out = vec![0u8; src.len()];
        bitunshuffle(src, size, block, &mut out);
        return Ok(out);
    }
    let total = be64(src, 0)?;
    if total > max as u64 {
        return Err(bad("bitshuffle"));
    }
    let total = total as usize;
    let block = be32(src, 8)? / size;
    if block == 0 || !block.is_multiple_of(8) {
        return Err(bad("bitshuffle"));
    }
    let n = total / size;
    let mut out = vec![0u8; total];
    let mut tmp = Vec::with_capacity((block * size).min(total));
    let mut i = 12usize;
    let mut done = 0usize;
    while done < n {
        let mut m = block.min(n - done);
        m -= m % 8;
        if m == 0 {
            break;
        }
        let cs = be32(src, i)?;
        i += 4;
        let s = src.get(i..i + cs).ok_or_else(|| bad("bitshuffle"))?;
        i += cs;
        tmp.clear();
        match comp {
            2 => lz4_block(s, m * size, &mut tmp)?,
            3 => tmp = zstd(s, m * size)?,
            c => return Err(Error::Unsupported(format!("bitshuffle compression {c}"))),
        }
        if tmp.len() != m * size {
            return Err(bad("bitshuffle"));
        }
        let a = done * size;
        bit_untranspose(&tmp, m, size, &mut out[a..a + m * size]);
        done += m;
    }
    let tail = done * size;
    let rest = src
        .get(i..i + (total - tail))
        .ok_or_else(|| bad("bitshuffle"))?;
    out[tail..].copy_from_slice(rest);
    Ok(out)
}

/// BloscLZ (FastLZ level 2 with Blosc's long-distance extension), to exactly `n` bytes.
pub(crate) fn blosclz(src: &[u8], n: usize, out: &mut Vec<u8>) -> Result<()> {
    let base = out.len();
    let end = base + n;
    let byte = |i: usize| src.get(i).copied().ok_or_else(|| bad("BloscLZ"));
    let mut i = 0usize;
    let mut ctrl = (byte(0)? & 31) as usize;
    i += 1;
    loop {
        if ctrl >= 32 {
            let mut len = (ctrl >> 5) - 1;
            let mut ofs = (ctrl & 31) << 8;
            if len == 6 {
                loop {
                    let c = byte(i)? as usize;
                    i += 1;
                    len += c;
                    if c != 255 {
                        break;
                    }
                }
            }
            let code = byte(i)? as usize;
            i += 1;
            len += 3;
            let mut back = ofs + code + 1;
            if code == 255 && ofs == 31 << 8 {
                ofs = (byte(i)? as usize) << 8 | byte(i + 1)? as usize;
                i += 2;
                back = ofs + 8191 + 1;
            }
            if back > out.len() - base || out.len() + len > end {
                return Err(bad("BloscLZ"));
            }
            copy_match(out, back, len);
        } else {
            let n = ctrl + 1;
            let l = src.get(i..i + n).ok_or_else(|| bad("BloscLZ"))?;
            if out.len() + n > end {
                return Err(bad("BloscLZ"));
            }
            out.extend_from_slice(l);
            i += n;
        }
        if i >= src.len() {
            break;
        }
        ctrl = byte(i)? as usize;
        i += 1;
    }
    if out.len() != end {
        return Err(bad("BloscLZ"));
    }
    Ok(())
}

fn le32(b: &[u8], at: usize) -> Result<usize> {
    let s = b.get(at..at + 4).ok_or_else(|| bad("Blosc"))?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize)
}

/// Undoes Blosc's delta filter on one block. The first block of a chunk is the reference:
/// each of its words was XORed with the word before it. Every later block was XORed with the
/// reference block, word by word. Words are the element size when it is 1, 2, 4 or 8 bytes,
/// 8 bytes for other multiples of 8, else single bytes.
pub(crate) fn undelta(block: &mut [u8], typesize: usize, reference: Option<&[u8]>) {
    let w = match typesize {
        1 | 2 | 4 | 8 => typesize,
        t if t.is_multiple_of(8) => 8,
        _ => 1,
    };
    match reference {
        None => {
            // A running XOR over the words (in place, so each XORs the decoded one before).
            let n = block.len() / w * w;
            for i in w..n {
                block[i] ^= block[i - w];
            }
        }
        Some(r) => {
            for (b, r) in block.iter_mut().zip(r) {
                *b ^= r;
            }
        }
    }
}

/// A Blosc (version 1 format) frame.
pub(crate) fn blosc(src: &[u8], max: usize) -> Result<Vec<u8>> {
    if src.len() < 16 {
        return Err(bad("Blosc"));
    }
    let flags = src[2];
    let typesize = (src[3] as usize).max(1);
    let nbytes = le32(src, 4)?;
    let blocksize = le32(src, 8)?;
    if nbytes > max {
        return Err(bad("Blosc"));
    }
    if flags & 0x02 != 0 {
        return src
            .get(16..16 + nbytes)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| bad("Blosc"));
    }
    let delta = flags & 0x08 != 0;
    if blocksize == 0 {
        return if nbytes == 0 {
            Ok(Vec::new())
        } else {
            Err(bad("Blosc"))
        };
    }
    let codec = flags >> 5;
    let nblocks = nbytes.div_ceil(blocksize);
    let mut out = vec![0u8; nbytes];
    let mut tmp: Vec<u8> = Vec::with_capacity(blocksize.min(nbytes));
    for b in 0..nblocks {
        let start = le32(src, 16 + 4 * b)?;
        let bsize = blocksize.min(nbytes - b * blocksize);
        let leftover = bsize < blocksize;
        let splits = if flags & 0x10 == 0 && typesize <= 16 && bsize / typesize >= 128 && !leftover
        {
            typesize
        } else {
            1
        };
        let per = bsize / splits;
        tmp.clear();
        let mut p = start;
        for _ in 0..splits {
            let cs = le32(src, p)?;
            p += 4;
            let s = src.get(p..p + cs).ok_or_else(|| bad("Blosc"))?;
            p += cs;
            if cs == per {
                tmp.extend_from_slice(s);
                continue;
            }
            match codec {
                0 => blosclz(s, per, &mut tmp)?,
                1 => lz4_block(s, per, &mut tmp)?,
                3 => {
                    let v = specular_png::inflate_zlib(s, per).map_err(|_| bad("Blosc zlib"))?;
                    if v.len() != per {
                        return Err(bad("Blosc zlib"));
                    }
                    tmp.extend_from_slice(&v);
                }
                4 => {
                    let v = zstd(s, per)?;
                    if v.len() != per {
                        return Err(bad("Blosc Zstandard"));
                    }
                    tmp.extend_from_slice(&v);
                }
                c => return Err(Error::Unsupported(format!("Blosc codec {c}"))),
            }
        }
        if tmp.len() != bsize {
            return Err(bad("Blosc"));
        }
        let (head, rest) = out.split_at_mut(b * blocksize);
        let dst = &mut rest[..bsize];
        if flags & 0x01 != 0 && typesize > 1 {
            unshuffle_into(&tmp, typesize, dst);
        } else if flags & 0x04 != 0 && typesize >= 1 {
            bitunshuffle(&tmp, typesize, bsize / typesize, dst);
        } else {
            dst.copy_from_slice(&tmp);
        }
        if delta {
            undelta(
                dst,
                typesize,
                (b > 0).then_some(&head[..bsize.min(blocksize)]),
            );
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fletcher-32 iterates the words in place (it collected 8 bytes per 2-byte word, 4 GiB
    /// for a 1 GiB chunk); same sums as the plain definition.
    #[test]
    fn fletcher32_matches_definition() {
        fn plain(d: &[u8]) -> u32 {
            let (mut s1, mut s2) = (0u64, 0u64);
            for w in d.chunks(2) {
                let v = (u64::from(w[0]) << 8) | w.get(1).map_or(0, |&b| u64::from(b));
                s1 = (s1 + v) % 65535;
                s2 = (s2 + s1) % 65535;
            }
            ((s2 << 16) | s1) as u32
        }
        for len in [0usize, 1, 2, 719, 720, 721, 5000, 5001] {
            let d: Vec<u8> = (0..len).map(|i| (i * 7 + 3) as u8 | 0x80).collect();
            let (a, b) = (fletcher32(&d), plain(&d));
            // Same value modulo 65535 in each half (0 and 65535 are the same residue).
            assert_eq!((a & 0xffff) % 65535, b & 0xffff, "len {len}");
            assert_eq!((a >> 16) % 65535, b >> 16, "len {len}");
        }
    }

    #[test]
    fn shuffle_round_trip() {
        let src: Vec<u8> = (0..23).collect();
        // Shuffle by hand (4-byte elements, 3 trailing bytes).
        let n = 5;
        let mut sh = vec![0u8; 23];
        for i in 0..n {
            for j in 0..4 {
                sh[j * n + i] = src[i * 4 + j];
            }
        }
        sh[20..].copy_from_slice(&src[20..]);
        assert_eq!(unshuffle(&sh, 4), src);
    }

    #[test]
    fn lzf_literals_and_matches() {
        // "abcabcabc": literal "abc", then a 6-byte back-reference of distance 3.
        let enc = [2, b'a', b'b', b'c', (4 << 5), 2];
        assert_eq!(lzf(&enc, 64).unwrap(), b"abcabcabc");
        assert!(lzf(&[0x20, 0], 64).is_err());
    }

    #[test]
    fn lz4_overlap() {
        // Literal "ab", a 6-byte match at distance 2, then the literal "z".
        let enc = [0x22, b'a', b'b', 2, 0, 0x10, b'z'];
        let mut out = Vec::new();
        lz4_block(&enc, 9, &mut out).unwrap();
        assert_eq!(out, b"ababababz");
        assert!(lz4_block(&enc, 8, &mut Vec::new()).is_err());
    }

    #[test]
    fn fletcher_known_value() {
        // Two words 0x0102, 0x0304: s1 = 0x0406, s2 = 0x0102 + 0x0406 = 0x0508.
        assert_eq!(fletcher32(&[1, 2, 3, 4]), 0x0508_0406);
    }
}
