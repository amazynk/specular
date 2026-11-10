//! Strip / tile decompression (PackBits, LZW, Deflate, Zstandard, LZMA) and predictors.
//!
//! Every decoder fills at most `out.len()` bytes and returns how many it wrote, so a caller can
//! stop early when it only needs the first rows of a strip. Damaged input never panics: the
//! decoders stop and the caller treats the rest as missing.

/// PackBits (Apple / TIFF compression 32773).
pub(crate) fn packbits(input: &[u8], out: &mut [u8]) -> usize {
    let (mut i, mut o) = (0usize, 0usize);
    while i < input.len() && o < out.len() {
        let n = input[i] as i8;
        i += 1;
        if n >= 0 {
            let cnt = (n as usize + 1).min(input.len() - i).min(out.len() - o);
            out[o..o + cnt].copy_from_slice(&input[i..i + cnt]);
            i += n as usize + 1;
            o += cnt;
        } else if n != -128 {
            let Some(&b) = input.get(i) else { break };
            i += 1;
            let cnt = (1 - n as isize) as usize;
            let cnt = cnt.min(out.len() - o);
            out[o..o + cnt].fill(b);
            o += cnt;
        }
    }
    o
}

/// TIFF LZW: MSB-first codes of 9..12 bits with "early change", or old-style LZW (pre-TIFF 5
/// writers): LSB-first codes that widen one code later, recognised as libtiff does by a
/// stream that starts with a clear code written LSB first (`00 01`).
///
/// Each table entry is a (start, length) reference into the output already written: an entry
/// is always the previous string plus the first byte of the following one, and the following
/// string starts right where the previous one ended, so the bytes are already in place. This
/// avoids a separate string table and turns every code into one `copy_within`.
///
/// Returns the bytes written.
pub(crate) fn lzw(input: &[u8], out: &mut [u8]) -> usize {
    if input.len() >= 2 && input[0] == 0 && input[1] & 1 != 0 {
        lzw_with::<true>(input, out)
    } else {
        lzw_with::<false>(input, out)
    }
}

/// [`lzw`] for one bit order: `OLD` reads LSB-first codes without early change.
fn lzw_with<const OLD: bool>(input: &[u8], out: &mut [u8]) -> usize {
    const CLEAR: u32 = 256;
    const EOI: u32 = 257;
    // On the stack: a strip or tile is often small, and two zeroed heap tables per strip
    // were a measurable part of decoding one.
    let mut start = [0u32; 4096];
    let mut len = [0u32; 4096];
    let mut next = 258u32;
    let mut width = 9u32;
    let mut prev: Option<(u32, u32)> = None;
    let mut acc: u64 = 0;
    let mut nbits = 0u32;
    let mut i = 0usize;
    let mut o = 0usize;
    let n = out.len();
    loop {
        let code = if OLD {
            while nbits < width && i < input.len() {
                acc |= (input[i] as u64) << nbits;
                nbits += 8;
                i += 1;
            }
            if nbits < width {
                break;
            }
            let c = (acc & ((1u64 << width) - 1)) as u32;
            acc >>= width;
            nbits -= width;
            c
        } else {
            // Refill: read up to 6 bytes at once when possible.
            if nbits < width {
                if i + 8 <= input.len() {
                    let mut b = [0u8; 8];
                    b.copy_from_slice(&input[i..i + 8]);
                    let v = u64::from_be_bytes(b);
                    let take = (64 - nbits) / 8;
                    acc = if take == 8 {
                        v
                    } else {
                        (acc << (take * 8)) | (v >> (64 - take * 8))
                    };
                    nbits += take * 8;
                    i += take as usize;
                } else {
                    while nbits < width && i < input.len() {
                        acc = (acc << 8) | input[i] as u64;
                        nbits += 8;
                        i += 1;
                    }
                    if nbits < width {
                        break;
                    }
                }
            }
            let c = ((acc >> (nbits - width)) & ((1u64 << width) - 1)) as u32;
            nbits -= width;
            c
        };
        if code == CLEAR {
            next = 258;
            width = 9;
            prev = None;
            continue;
        }
        if code == EOI {
            break;
        }
        let cur = if code < 256 {
            if o >= n {
                break;
            }
            out[o] = code as u8;
            o += 1;
            (o as u32 - 1, 1)
        } else if code < next {
            let (s, l) = (start[code as usize] as usize, len[code as usize] as usize);
            let c = l.min(n - o);
            copy_back(out, s, o, c);
            let at = o;
            o += c;
            if c < l {
                break;
            }
            (at as u32, l as u32)
        } else if code == next
            && let Some((ps, pl)) = prev
        {
            let (ps, pl) = (ps as usize, pl as usize);
            let c = pl.min(n - o);
            copy_back(out, ps, o, c);
            let at = o;
            o += c;
            if o >= n {
                break;
            }
            out[o] = out[ps];
            o += 1;
            (at as u32, pl as u32 + 1)
        } else {
            break; // corrupt code
        };
        if let Some((ps, pl)) = prev
            && next < 4096
        {
            start[next as usize] = ps;
            len[next as usize] = pl + 1;
            next += 1;
        }
        prev = Some(cur);
        // New-style LZW widens one code early; old-style when the table reaches the limit.
        if next + u32::from(!OLD) >= (1 << width) && width < 12 {
            width += 1;
        }
        if o >= n {
            break;
        }
    }
    o
}

/// Copies `out[s..s + c]` to `o`; `s + c <= o`, as an LZW string is always output already
/// written. Most strings are a few bytes, so they go eight at a time as words while that stays
/// inside `out` (bytes written past `c` are overwritten later), else through `copy_within`
/// (one `memmove` call per string). A word never reads what this copy wrote: `o - s >= c`, so
/// for strings longer than a word the source stays a word or more behind the destination.
#[inline(always)]
fn copy_back(out: &mut [u8], s: usize, o: usize, c: usize) {
    if o + c + 8 <= out.len() {
        let mut k = 0;
        while k < c {
            let mut w = [0u8; 8];
            w.copy_from_slice(&out[s + k..s + k + 8]);
            out[o + k..o + k + 8].copy_from_slice(&w);
            k += 8;
        }
    } else {
        out.copy_within(s..s + c, o);
    }
}

/// zlib (or raw) Deflate.
pub(crate) fn inflate(input: &[u8], out: &mut [u8]) -> usize {
    let zlib = input.len() >= 2
        && input[0] & 0x0f == 8
        && (input[0] as u16 * 256 + input[1] as u16).is_multiple_of(31);
    let cfg = zlib_rs::InflateConfig {
        window_bits: if zlib { 15 } else { -15 },
    };
    let (written, _rc) = zlib_rs::decompress_slice(out, input, cfg);
    written.len()
}

/// Deflate into a growing buffer, for streams of unknown size (LERC blobs), at most `limit`
/// bytes.
pub(crate) fn inflate_vec(input: &[u8], hint: usize, limit: usize) -> Vec<u8> {
    let mut cap = hint.clamp(4096, limit.max(4096));
    loop {
        let mut out = vec![0u8; cap];
        let n = inflate(input, &mut out);
        if n < cap || cap >= limit {
            out.truncate(n);
            return out;
        }
        cap = (cap * 2).min(limit);
    }
}

/// JBIG1 (compression 34661): one bi-level image entity (T.82), its rows packed into `out`
/// (`row` bytes each, MSB first) with 1 for black, as JBIG codes them. Returns the bytes
/// written: the decoded rows, or 0 when the entity does not decode.
pub(crate) fn jbig(input: &[u8], out: &mut [u8], row: usize) -> usize {
    let Ok(img) = specular_jbig::decode(input, 0) else {
        return 0;
    };
    let specular_jbig::Pixels::Gray8(px) = img.pixels else {
        return 0;
    };
    let w = img.width as usize;
    if w == 0 || row == 0 || w.div_ceil(8) > row {
        return 0;
    }
    let rows = (out.len() / row).min(img.height as usize);
    for (y, r) in out.chunks_exact_mut(row).take(rows).enumerate() {
        r.fill(0);
        for (x, &g) in px[y * w..(y + 1) * w].iter().enumerate() {
            if g == 0 {
                r[x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    // Cut data decodes as white rows: count only what the entity held.
    if img.truncated { 0 } else { rows * row }
}

/// Zstandard (pure Rust, `ruzstd`): the first `out.len()` bytes of the stream.
pub(crate) fn zstd(input: &[u8], out: &mut [u8]) -> usize {
    use std::io::Read;
    let Ok(mut dec) = ruzstd::decoding::StreamingDecoder::new(input) else {
        return 0;
    };
    let mut n = 0;
    while n < out.len() {
        match dec.read(&mut out[n..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => n += k,
        }
    }
    n
}

/// Zstandard into a growing buffer of at most `limit` bytes.
pub(crate) fn zstd_vec(input: &[u8], limit: usize) -> Vec<u8> {
    use std::io::Read;
    let mut out = Vec::new();
    if let Ok(dec) = ruzstd::decoding::StreamingDecoder::new(input) {
        let _ = dec.take(limit as u64).read_to_end(&mut out);
    }
    out
}

/// LZMA (TIFF compression 34925): an xz stream (as liblzma writes it) of LZMA2 blocks; the
/// first `out.len()` bytes.
pub(crate) fn xz(input: &[u8], out: &mut [u8]) -> usize {
    use std::io::Read;
    const MAGIC: [u8; 6] = [0xFD, b'7', b'z', b'X', b'Z', 0];
    if !input.starts_with(&MAGIC) || input.len() < 12 {
        return 0;
    }
    let check_size = match input[7] & 0x0f {
        0 => 0,
        1..=3 => 4,
        4..=6 => 8,
        7..=9 => 16,
        _ => 32,
    };
    let mut p = 12usize;
    let mut n = 0usize;
    while n < out.len() {
        // Block header (or the index, which ends the blocks).
        let Some(&hs) = input.get(p) else { break };
        if hs == 0 {
            break;
        }
        let hlen = (hs as usize + 1) * 4;
        let Some(h) = input.get(p..p + hlen) else {
            break;
        };
        let bflags = h[1];
        let mut q = 2usize;
        let varint = |q: &mut usize| -> Option<u64> {
            let mut v = 0u64;
            for i in 0..9 {
                let b = *h.get(*q)?;
                *q += 1;
                v |= ((b & 0x7f) as u64) << (7 * i);
                if b & 0x80 == 0 {
                    return Some(v);
                }
            }
            None
        };
        let mut packed = None;
        if bflags & 0x40 != 0 {
            packed = varint(&mut q);
        }
        if bflags & 0x80 != 0 && varint(&mut q).is_none() {
            break;
        }
        // Filters as listed for encoding: optional Delta (0x03) filters, then LZMA2 (0x21)
        // with its dictionary size. libtiff writes Delta + LZMA2.
        let mut deltas = Vec::new();
        for _ in 0..bflags & 3 {
            let (Some(0x03), Some(1)) = (varint(&mut q), varint(&mut q)) else {
                return n;
            };
            let Some(&d) = h.get(q) else { return n };
            q += 1;
            deltas.push(d as usize + 1);
        }
        let (Some(0x21), Some(1)) = (varint(&mut q), varint(&mut q)) else {
            break;
        };
        let Some(&props) = h.get(q) else { break };
        let bits = (props & 0x3f) as u32;
        if bits > 40 {
            break;
        }
        let dict = if bits == 40 {
            u32::MAX
        } else {
            (2 | (bits & 1)) << (bits / 2 + 11)
        };
        // The dictionary is allocated (and zeroed) up front, and the header may ask for 4 GiB:
        // no match reaches further back than the bytes this block decodes, so no more than
        // the output still wanted is needed (lzma_rust2 rounds up to its 4 KiB minimum).
        let dict = dict.min(u32::try_from(out.len() - n).unwrap_or(u32::MAX));
        p += hlen;
        let mut rest = &input[p.min(input.len())..];
        let before = rest.len();
        let mut rd = lzma_rust2::Lzma2Reader::new(&mut rest, dict, None);
        let start = n;
        let mut ended = false;
        while n < out.len() {
            match rd.read(&mut out[n..]) {
                Ok(0) => {
                    ended = true;
                    break;
                }
                Ok(k) => n += k,
                Err(_) => {
                    ended = false;
                    break;
                }
            }
        }
        // Undo the Delta filters, last-applied first; each block starts with zero history.
        for &dist in deltas.iter().rev() {
            let b = &mut out[start..n];
            for i in dist..b.len() {
                b[i] = b[i].wrapping_add(b[i - dist]);
            }
        }
        if !ended {
            break;
        }
        drop(rd);
        let used = packed.map_or(before - rest.len(), |c| c as usize);
        // The block is padded to four bytes, then its check.
        p += used.div_ceil(4) * 4 + check_size;
    }
    n
}

/// Undo the horizontal-differencing predictor (2) on one row in file byte order.
/// `stride` is the samples per pixel in this row (1 for planar data).
pub(crate) fn unpredict_horizontal(row: &mut [u8], bits: u16, stride: usize, le: bool) {
    match (bits, stride) {
        // The previous pixel kept in registers, not loaded back from the row just stored.
        (8, 1) => unpredict8::<1>(row),
        (8, 2) => unpredict8::<2>(row),
        (8, 3) => unpredict8::<3>(row),
        (8, 4) => unpredict8::<4>(row),
        (16, 1) if !le => unpredict16::<false, 1>(row),
        (16, 3) if !le => unpredict16::<false, 3>(row),
        (16, 4) if !le => unpredict16::<false, 4>(row),
        (16, 1) => unpredict16::<true, 1>(row),
        (16, 3) => unpredict16::<true, 3>(row),
        (16, 4) => unpredict16::<true, 4>(row),
        _ => unpredict_any(row, bits, stride, le),
    }
}

fn unpredict8<const S: usize>(row: &mut [u8]) {
    let Some((first, rest)) = row.split_at_mut_checked(S) else {
        return;
    };
    let mut prev: [u8; S] = first.try_into().unwrap_or([0; S]);
    let mut px = rest.chunks_exact_mut(S);
    for p in &mut px {
        for k in 0..S {
            prev[k] = prev[k].wrapping_add(p[k]);
            p[k] = prev[k];
        }
    }
    for (k, b) in px.into_remainder().iter_mut().enumerate() {
        *b = b.wrapping_add(prev[k]);
    }
}

fn unpredict16<const LE: bool, const S: usize>(row: &mut [u8]) {
    let rd = |b: &[u8]| {
        if LE {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        }
    };
    let n = row.len() / 2 * 2;
    let Some((first, rest)) = row[..n].split_at_mut_checked(2 * S) else {
        return;
    };
    let mut prev = [0u16; S];
    for (k, p) in prev.iter_mut().enumerate() {
        *p = rd(&first[2 * k..]);
    }
    let mut px = rest.chunks_exact_mut(2 * S);
    for p in &mut px {
        for k in 0..S {
            prev[k] = prev[k].wrapping_add(rd(&p[2 * k..]));
            let b = if LE {
                prev[k].to_le_bytes()
            } else {
                prev[k].to_be_bytes()
            };
            p[2 * k..2 * k + 2].copy_from_slice(&b);
        }
    }
    for (k, b) in px.into_remainder().chunks_exact_mut(2).enumerate() {
        let v = rd(b).wrapping_add(prev[k]);
        b.copy_from_slice(&if LE { v.to_le_bytes() } else { v.to_be_bytes() });
    }
}

/// [`unpredict_horizontal`] for any depth and stride.
fn unpredict_any(row: &mut [u8], bits: u16, stride: usize, le: bool) {
    match bits {
        8 => {
            for i in stride..row.len() {
                row[i] = row[i].wrapping_add(row[i - stride]);
            }
        }
        16 => {
            let n = row.len() / 2;
            let rd = |r: &[u8], i: usize| {
                let b = [r[2 * i], r[2 * i + 1]];
                if le {
                    u16::from_le_bytes(b)
                } else {
                    u16::from_be_bytes(b)
                }
            };
            for i in stride..n {
                let v = rd(row, i).wrapping_add(rd(row, i - stride));
                let b = if le { v.to_le_bytes() } else { v.to_be_bytes() };
                row[2 * i..2 * i + 2].copy_from_slice(&b);
            }
        }
        24 => {
            let n = row.len() / 3;
            let rd = |r: &[u8], i: usize| {
                let b = &r[3 * i..3 * i + 3];
                if le {
                    u32::from_le_bytes([b[0], b[1], b[2], 0])
                } else {
                    u32::from_be_bytes([0, b[0], b[1], b[2]])
                }
            };
            for i in stride..n {
                let v = rd(row, i).wrapping_add(rd(row, i - stride)) & 0xff_ffff;
                let b = if le { v.to_le_bytes() } else { v.to_be_bytes() };
                if le {
                    row[3 * i..3 * i + 3].copy_from_slice(&b[..3]);
                } else {
                    row[3 * i..3 * i + 3].copy_from_slice(&b[1..]);
                }
            }
        }
        32 => {
            let n = row.len() / 4;
            let rd = |r: &[u8], i: usize| {
                let b = [r[4 * i], r[4 * i + 1], r[4 * i + 2], r[4 * i + 3]];
                if le {
                    u32::from_le_bytes(b)
                } else {
                    u32::from_be_bytes(b)
                }
            };
            for i in stride..n {
                let v = rd(row, i).wrapping_add(rd(row, i - stride));
                let b = if le { v.to_le_bytes() } else { v.to_be_bytes() };
                row[4 * i..4 * i + 4].copy_from_slice(&b);
            }
        }
        64 => {
            let n = row.len() / 8;
            let rd = |r: &[u8], i: usize| {
                let mut b = [0u8; 8];
                b.copy_from_slice(&r[8 * i..8 * i + 8]);
                if le {
                    u64::from_le_bytes(b)
                } else {
                    u64::from_be_bytes(b)
                }
            };
            for i in stride..n {
                let v = rd(row, i).wrapping_add(rd(row, i - stride));
                let b = if le { v.to_le_bytes() } else { v.to_be_bytes() };
                row[8 * i..8 * i + 8].copy_from_slice(&b);
            }
        }
        // 40, 48 and 56 bits: whole bytes in the file's byte order.
        b if b.is_multiple_of(8) && b < 64 => {
            let nb = b as usize / 8;
            let mask = (1u64 << b) - 1;
            let rd = |r: &[u8], i: usize| {
                let s = &r[nb * i..nb * i + nb];
                if le {
                    s.iter().rev().fold(0u64, |a, &x| (a << 8) | x as u64)
                } else {
                    s.iter().fold(0u64, |a, &x| (a << 8) | x as u64)
                }
            };
            for i in stride..row.len() / nb {
                let v = rd(row, i).wrapping_add(rd(row, i - stride)) & mask;
                for k in 0..nb {
                    let byte = (v >> (8 * if le { k } else { nb - 1 - k })) as u8;
                    row[nb * i + k] = byte;
                }
            }
        }
        b if b < 64 => unpredict_packed(row, b as usize, stride),
        _ => {}
    }
}

/// The horizontal predictor on samples that are not whole bytes (bit-packed MSB first, as the
/// row stores them): each sample adds the one `stride` samples before it, modulo `2^bits`.
/// libtiff refuses these depths; the arithmetic is the TIFF 6.0 predictor's, on the samples.
fn unpredict_packed(row: &mut [u8], bits: usize, stride: usize) {
    let n = row.len() * 8 / bits;
    let mask = (1u64 << bits) - 1;
    let get = |row: &[u8], i: usize| -> u64 {
        let p = i * bits;
        let mut v = 0u64;
        let mut got = 0;
        while got < bits {
            let off = (p + got) % 8;
            let take = (8 - off).min(bits - got);
            let byte = row[(p + got) / 8] as u64;
            v = (v << take) | ((byte >> (8 - off - take)) & ((1 << take) - 1));
            got += take;
        }
        v
    };
    let put = |row: &mut [u8], i: usize, v: u64| {
        let p = i * bits;
        let mut left = bits;
        while left > 0 {
            let pos = p + bits - left;
            let off = pos % 8;
            let take = (8 - off).min(left);
            let part = ((v >> (left - take)) & ((1 << take) - 1)) as u8;
            let shift = 8 - off - take;
            let m = (((1u16 << take) - 1) as u8) << shift;
            row[pos / 8] = (row[pos / 8] & !m) | (part << shift);
            left -= take;
        }
    };
    for i in stride..n {
        let v = get(row, i).wrapping_add(get(row, i - stride)) & mask;
        put(row, i, v);
    }
}

/// Undo the floating-point predictor (3) on one row: byte-wise differencing, then the bytes of
/// each sample are gathered from the planes (most significant first), into the file's byte
/// order (`le`).
pub(crate) fn unpredict_float(
    row: &mut [u8],
    bytes_per_sample: usize,
    stride: usize,
    le: bool,
    tmp: &mut Vec<u8>,
) {
    for i in stride..row.len() {
        row[i] = row[i].wrapping_add(row[i - stride]);
    }
    let wc = row.len() / bytes_per_sample;
    tmp.clear();
    tmp.extend_from_slice(row);
    for c in 0..wc {
        for b in 0..bytes_per_sample {
            let plane = if le { bytes_per_sample - b - 1 } else { b };
            row[c * bytes_per_sample + b] = tmp[plane * wc + c];
        }
    }
}

/// Reverse the bit order of every byte (FillOrder = 2).
pub(crate) fn reverse_bits(data: &mut [u8]) {
    for b in data {
        *b = b.reverse_bits();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predictor_fast_paths_match_the_general_one() {
        let mut x = 0x1234_5678u32;
        let mut rnd = || {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        };
        for bits in [8u16, 16] {
            for stride in 1..=5 {
                for le in [false, true] {
                    // Odd lengths too: a partial pixel at the end of the row.
                    for len in [0, 1, 2, 7, 64, 101] {
                        let row: Vec<u8> = (0..len).map(|_| rnd()).collect();
                        let (mut a, mut b) = (row.clone(), row);
                        unpredict_horizontal(&mut a, bits, stride, le);
                        unpredict_any(&mut b, bits, stride, le);
                        assert_eq!(a, b, "bits {bits} stride {stride} le {le} len {len}");
                    }
                }
            }
        }
    }

    #[test]
    fn packbits_example() {
        // Apple's example.
        let enc = [
            0xFE, 0xAA, 0x02, 0x80, 0x00, 0x2A, 0xFD, 0xAA, 0x03, 0x80, 0x00, 0x2A, 0x22, 0xF7,
            0xAA,
        ];
        let mut out = [0u8; 24];
        assert_eq!(packbits(&enc, &mut out), 24);
        assert_eq!(&out[..5], &[0xAA, 0xAA, 0xAA, 0x80, 0x00]);
    }

    #[test]
    fn lzw_round_trip() {
        let mut data = Vec::new();
        for i in 0..20000u32 {
            data.push((i * 7 % 13) as u8 ^ (i / 100) as u8);
        }
        let enc = weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
            .encode(&data)
            .unwrap();
        let mut out = vec![0u8; data.len()];
        assert_eq!(lzw(&enc, &mut out), data.len());
        assert_eq!(out, data);
        // Partial output and truncated input don't panic.
        let mut part = vec![0u8; 1000];
        assert_eq!(lzw(&enc, &mut part), 1000);
        assert_eq!(part, data[..1000]);
        let mut out2 = vec![0u8; data.len()];
        let n = lzw(&enc[..enc.len() / 2], &mut out2);
        assert!(n < data.len());
        assert_eq!(out2[..n], data[..n]);
    }

    #[test]
    fn old_style_lzw_round_trip() {
        // Old-style TIFF LZW is LSB-first with the width switch at the table limit (as GIF).
        let mut data = Vec::new();
        for i in 0..30000u32 {
            data.push((i * 5 % 11) as u8 ^ (i / 300) as u8);
        }
        let enc = weezl::encode::Encoder::new(weezl::BitOrder::Lsb, 8)
            .encode(&data)
            .unwrap();
        assert_eq!(&enc[..2], &[0, 1]);
        let mut out = vec![0u8; data.len()];
        assert_eq!(lzw(&enc, &mut out), data.len());
        assert_eq!(out, data);
        let mut part = vec![0u8; 777];
        assert_eq!(lzw(&enc, &mut part), 777);
        assert_eq!(part, data[..777]);
    }

    #[test]
    fn lzw_garbage() {
        let mut s = 12345u32;
        for len in [1usize, 2, 7, 100, 5000] {
            let input: Vec<u8> = (0..len)
                .map(|_| {
                    s = s.wrapping_mul(1103515245).wrapping_add(12345);
                    (s >> 16) as u8
                })
                .collect();
            let mut out = vec![0u8; 4096];
            let _ = lzw(&input, &mut out);
            let _ = packbits(&input, &mut out);
            let _ = inflate(&input, &mut out);
        }
    }

    mod hostile {
        use super::*;

        #[test]
        fn xz_dictionary_of_4_gib_is_not_allocated() {
            // Stream header, then a block whose LZMA2 properties ask for a 4 GiB dictionary
            // (40), holding one uncompressed chunk of 4 bytes.
            let mut input = vec![0xFD, b'7', b'z', b'X', b'Z', 0, 0, 0, 0, 0, 0, 0];
            input.extend_from_slice(&[2, 0, 0x21, 1, 40, 0, 0, 0, 0, 0, 0, 0]);
            input.extend_from_slice(&[0x01, 0x00, 0x03, b'a', b'b', b'c', b'd', 0x00]);
            let mut out = [0u8; 4];
            assert_eq!(xz(&input, &mut out), 4);
            assert_eq!(&out, b"abcd");
        }
    }
}
