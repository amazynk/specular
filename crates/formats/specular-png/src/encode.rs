//! A small PNG encoder for what Specular writes itself: thumbnails (`specular --thumbnail`),
//! pasted clipboard images and screenshots.
//!
//! 8-bit gray, gray + alpha, RGB and RGBA. Each row gets the filter with the smallest sum of
//! absolute residuals (the libpng heuristic); the zlib stream is a single fixed-Huffman DEFLATE
//! block fed by greedy LZ77 with hash chains. That is a few times faster than a full encoder
//! and typically 10-30% larger than `zlib -6`: fine for files that are small or short-lived.
//! Safe Rust, no allocation beyond the output and one hash table.
//!
//! With a zlib level the stream comes from zlib-rs instead, and large images are filtered and
//! compressed in parallel on the current rayon pool (pigz's trick): the filtered data is cut
//! into 1 MiB pieces, each deflated on its own with the previous 32 KiB as its dictionary and
//! ended by a sync flush, and the pieces' Adler-32s are combined. The pieces do not depend on
//! the thread count, so neither do the bytes, and the cost in size is a few bytes per piece.

use crate::ColorType;
use rayon::prelude::*;

/// Why an image could not be encoded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EncodeError {
    /// Zero or more than 2^31-1 pixels wide or high.
    #[error("invalid image size {0}x{1}")]
    Size(u32, u32),
    /// `data` is not `width * height * channels` bytes.
    #[error("expected {expected} bytes of pixel data, got {got}")]
    DataLength { expected: usize, got: usize },
    /// Indexed colour is not written.
    #[error("unsupported colour type {0:?}")]
    Color(ColorType),
    /// A text keyword must be 1-79 Latin-1 characters without NUL.
    #[error("invalid text keyword")]
    Keyword,
}

/// Encodes 8-bit samples (`color` decides how many per pixel, interleaved, rows top to bottom
/// without padding) as a PNG file.
pub fn encode(
    width: u32,
    height: u32,
    color: ColorType,
    data: &[u8],
) -> Result<Vec<u8>, EncodeError> {
    encode_with_text(width, height, color, data, &[])
}

/// [`encode`] with `tEXt` chunks (keyword, Latin-1 text; other characters become `?`), e.g. the
/// freedesktop thumbnail keys `Thumb::URI` and `Thumb::MTime`.
pub fn encode_with_text(
    width: u32,
    height: u32,
    color: ColorType,
    data: &[u8],
    text: &[(&str, &str)],
) -> Result<Vec<u8>, EncodeError> {
    encode_with(
        width,
        height,
        color,
        data,
        &EncodeOptions {
            text,
            ..EncodeOptions::default()
        },
    )
}

/// What [`encode_with`] writes besides the pixels, and how hard it compresses.
#[derive(Clone, Copy, Debug, Default)]
pub struct EncodeOptions<'a> {
    /// 16-bit samples: `data` holds two big-endian bytes per sample (PNG's own order).
    pub sixteen_bit: bool,
    /// `None`: the built-in fast single-block encoder; `Some(1..=9)`: zlib at that level
    /// (smaller files, slower).
    pub level: Option<u8>,
    /// An ICC profile (`iCCP`, compressed).
    pub icc: Option<&'a [u8]>,
    /// An `sRGB` chunk (relative colorimetric) when there is no profile or `cICP`.
    pub srgb: bool,
    /// Coding-independent code points (`cICP`: primaries, transfer, matrix, full range), e.g.
    /// `[9, 16, 0, 1]` for BT.2100 PQ.
    pub cicp: Option<[u8; 4]>,
    /// TIFF-structured Exif (`eXIf`).
    pub exif: Option<&'a [u8]>,
    /// An XMP packet (`iTXt` "XML:com.adobe.xmp", uncompressed).
    pub xmp: Option<&'a [u8]>,
    /// `tEXt` chunks.
    pub text: &'a [(&'a str, &'a str)],
}

/// Encodes `data` (interleaved samples of `color`, 8 or 16 bits as `opts` says, rows top to
/// bottom without padding) as a PNG file with the chunks `opts` asks for.
pub fn encode_with(
    width: u32,
    height: u32,
    color: ColorType,
    data: &[u8],
    opts: &EncodeOptions<'_>,
) -> Result<Vec<u8>, EncodeError> {
    const MAX: u32 = i32::MAX as u32;
    if width == 0 || height == 0 || width > MAX || height > MAX {
        return Err(EncodeError::Size(width, height));
    }
    let (ct, channels) = match color {
        ColorType::Gray => (0u8, 1usize),
        ColorType::Rgb => (2, 3),
        ColorType::GrayAlpha => (4, 2),
        ColorType::Rgba => (6, 4),
        ColorType::Indexed => return Err(EncodeError::Color(color)),
    };
    let bpp = channels * if opts.sixteen_bit { 2 } else { 1 };
    let row = (width as usize)
        .checked_mul(bpp)
        .ok_or(EncodeError::Size(width, height))?;
    let expected = row
        .checked_mul(height as usize)
        .ok_or(EncodeError::Size(width, height))?;
    if data.len() != expected {
        return Err(EncodeError::DataLength {
            expected,
            got: data.len(),
        });
    }
    for (k, _) in opts.text {
        if k.is_empty() || k.len() > 79 || k.chars().any(|c| c == '\0' || c as u32 > 255) {
            return Err(EncodeError::Keyword);
        }
    }

    let filtered = if opts.level.is_some() && data.len() >= PIECE {
        filter_rows_parallel(data, row, bpp)
    } else {
        filter_rows(data, row, bpp)
    };
    let z = compress(&filtered, opts.level);
    drop(filtered);

    let mut out = Vec::with_capacity(z.len() + 256);
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[if opts.sixteen_bit { 16 } else { 8 }, ct, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    if let Some(c) = opts.cicp {
        chunk(&mut out, b"cICP", &c);
    }
    if let Some(icc) = opts.icc {
        // Profile name, NUL, compression method 0, then the zlib stream.
        let mut d = b"ICC Profile\0\0".to_vec();
        d.extend_from_slice(&compress(icc, Some(opts.level.unwrap_or(6))));
        chunk(&mut out, b"iCCP", &d);
    } else if opts.srgb && opts.cicp.is_none() {
        chunk(&mut out, b"sRGB", &[1]);
    }
    if let Some(exif) = opts.exif {
        chunk(&mut out, b"eXIf", exif);
    }
    if let Some(xmp) = opts.xmp {
        // Keyword, NUL, compression flag 0, method 0, then an empty language tag and an empty
        // translated keyword (each NUL-terminated), then the UTF-8 packet.
        let mut d = b"XML:com.adobe.xmp\0\0\0\0\0".to_vec();
        d.extend_from_slice(xmp);
        chunk(&mut out, b"iTXt", &d);
    }
    for (k, v) in opts.text {
        let mut t: Vec<u8> = k.chars().map(|c| c as u8).collect();
        t.push(0);
        t.extend(v.chars().map(|c| {
            if c == '\0' || c as u32 > 255 {
                b'?'
            } else {
                c as u8
            }
        }));
        chunk(&mut out, b"tEXt", &t);
    }
    let idat: Vec<Vec<u8>> = z
        .par_chunks(1 << 20)
        .map(|part| {
            let mut c = Vec::with_capacity(part.len() + 12);
            chunk(&mut c, b"IDAT", part);
            c
        })
        .collect();
    out.reserve(z.len() + idat.len() * 12 + 12);
    for c in &idat {
        out.extend_from_slice(c);
    }
    chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

/// Bytes of filtered data per independently deflated piece.
const PIECE: usize = 1 << 20;

/// A zlib stream of `data`: the built-in fast encoder, or zlib-rs at `level` (in parallel
/// pieces when there is more than one).
fn compress(data: &[u8], level: Option<u8>) -> Vec<u8> {
    let Some(level) = level else {
        return zlib(data);
    };
    if data.len() > PIECE
        && let Some(z) = compress_pieces(data, i32::from(level.clamp(1, 9)))
    {
        return z;
    }
    let mut out = vec![0u8; zlib_rs::compress_bound(data.len())];
    let cfg = zlib_rs::DeflateConfig::new(i32::from(level.clamp(1, 9)));
    let (c, rc) = zlib_rs::compress_slice(&mut out, data, cfg);
    if rc != zlib_rs::ReturnCode::Ok {
        return zlib(data);
    }
    let n = c.len();
    out.truncate(n);
    out
}

/// The zlib stream of `data` deflated as [`PIECE`]-sized pieces in parallel; `None` if zlib-rs
/// fails.
fn compress_pieces(data: &[u8], level: i32) -> Option<Vec<u8>> {
    let n = data.len().div_ceil(PIECE);
    let parts: Vec<Option<(Vec<u8>, u32, usize)>> = (0..n)
        .into_par_iter()
        .map(|k| {
            let start = k * PIECE;
            let piece = &data[start..(start + PIECE).min(data.len())];
            // Raw deflate (no zlib header), 32 KiB window.
            let mut d = zlib_rs::Deflate::new(level, false, 15);
            if start > 0 {
                d.set_dictionary(&data[start.saturating_sub(1 << 15)..start])
                    .ok()?;
            }
            let last = k + 1 == n;
            let flush = if last {
                zlib_rs::DeflateFlush::Finish
            } else {
                zlib_rs::DeflateFlush::SyncFlush
            };
            let mut out = vec![0u8; zlib_rs::compress_bound(piece.len()) + 64];
            let status = d.compress(piece, &mut out, flush).ok()?;
            let done = if last {
                status == zlib_rs::Status::StreamEnd
            } else {
                d.total_in() as usize == piece.len()
            };
            if !done {
                return None;
            }
            out.truncate(d.total_out() as usize);
            Some((out, zlib_rs::adler32::adler32(1, piece), piece.len()))
        })
        .collect();
    let mut z = Vec::with_capacity(parts.iter().flatten().map(|p| p.0.len()).sum::<usize>() + 6);
    // CMF/FLG: deflate, 32 KiB window, default level, no dictionary (31 | 0x789C).
    z.extend_from_slice(&[0x78, 0x9C]);
    let mut adler = 1u32;
    for part in parts {
        let (bytes, a, len) = part?;
        z.extend_from_slice(&bytes);
        adler = zlib_rs::adler32::adler32_combine(adler, a, len as u64);
    }
    z.extend_from_slice(&adler.to_be_bytes());
    Some(z)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crate::crc::crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = (
        (p - a as i16).abs(),
        (p - b as i16).abs(),
        (p - c as i16).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// [`filter_rows`] on bands of rows in parallel (each row only reads its own and the row above).
fn filter_rows_parallel(data: &[u8], row: usize, bpp: usize) -> Vec<u8> {
    let rows = data.len() / row;
    let band = (PIECE / row).max(1);
    let mut out = vec![0u8; rows * (row + 1)];
    out.par_chunks_mut(band * (row + 1))
        .enumerate()
        .for_each(|(k, dst)| {
            let y0 = k * band;
            let n = dst.len() / (row + 1);
            let above = if y0 == 0 {
                None
            } else {
                Some(&data[(y0 - 1) * row..y0 * row])
            };
            let mut v = Vec::with_capacity(dst.len());
            filter_rows_into(&data[y0 * row..(y0 + n) * row], above, row, bpp, &mut v);
            dst.copy_from_slice(&v);
        });
    out
}

/// Filter byte + filtered row for every row, picking the filter with the smallest sum of
/// absolute (signed) residuals.
fn filter_rows(data: &[u8], row: usize, bpp: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / row * (row + 1));
    filter_rows_into(data, None, row, bpp, &mut out);
    out
}

/// [`filter_rows`] of `data`, appended to `out`; `above` is the row before the first (none at
/// the top of the image).
fn filter_rows_into(data: &[u8], above: Option<&[u8]>, row: usize, bpp: usize, out: &mut Vec<u8>) {
    let rows = data.len() / row;
    let zero = vec![0u8; row];
    // One candidate row per filter type, each filled by its own loop so the
    // byte loops vectorise instead of branching on the filter per byte.
    let mut cand = vec![0u8; 5 * row];
    let cost = |r: &[u8]| -> u64 { r.iter().map(|&v| u64::from((v as i8).unsigned_abs())).sum() };
    for y in 0..rows {
        let cur = &data[y * row..][..row];
        let up = match (y, above) {
            (0, Some(a)) => a,
            (0, None) => &zero[..],
            _ => &data[(y - 1) * row..][..row],
        };
        let (c0, rest) = cand.split_at_mut(row);
        let (c1, rest) = rest.split_at_mut(row);
        let (c2, rest) = rest.split_at_mut(row);
        let (c3, c4) = rest.split_at_mut(row);
        let h = bpp.min(row);
        c0.copy_from_slice(cur);
        c1[..h].copy_from_slice(&cur[..h]);
        for ((d, &x), &a) in c1[h..].iter_mut().zip(&cur[h..]).zip(cur) {
            *d = x.wrapping_sub(a);
        }
        for ((d, &x), &b) in c2.iter_mut().zip(cur).zip(up) {
            *d = x.wrapping_sub(b);
        }
        for i in 0..h {
            c3[i] = cur[i].wrapping_sub(up[i] / 2);
            c4[i] = cur[i].wrapping_sub(up[i]);
        }
        for (((d, &x), &a), &b) in c3[h..].iter_mut().zip(&cur[h..]).zip(cur).zip(&up[h..]) {
            *d = x.wrapping_sub(((u16::from(a) + u16::from(b)) / 2) as u8);
        }
        for ((((d, &x), &a), &b), &c) in c4[h..]
            .iter_mut()
            .zip(&cur[h..])
            .zip(cur)
            .zip(&up[h..])
            .zip(up)
        {
            *d = x.wrapping_sub(paeth(a, b, c));
        }
        let mut best_ty = 0;
        let mut best_cost = u64::MAX;
        for (ty, c) in cand.chunks_exact(row).enumerate() {
            let k = cost(c);
            if k < best_cost {
                best_cost = k;
                best_ty = ty;
            }
        }
        out.push(best_ty as u8);
        out.extend_from_slice(&cand[best_ty * row..][..row]);
    }
}

/// LSB-first bit writer.
struct Bits {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    fn put(&mut self, v: u32, len: u32) {
        self.acc |= (v as u64) << self.n;
        self.n += len;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

fn reverse(code: u32, len: u32) -> u32 {
    code.reverse_bits() >> (32 - len)
}

/// Fixed-Huffman code of a literal/length symbol, already bit-reversed, and its length.
fn litlen_code(sym: u32) -> (u32, u32) {
    match sym {
        0..=143 => (reverse(0x30 + sym, 8), 8),
        144..=255 => (reverse(0x190 + sym - 144, 9), 9),
        256..=279 => (reverse(sym - 256, 7), 7),
        _ => (reverse(0xC0 + sym - 280, 8), 8),
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn put_literal(b: &mut Bits, v: u8) {
    let (c, l) = litlen_code(v as u32);
    b.put(c, l);
}

fn put_match(b: &mut Bits, len: usize, dist: usize) {
    let li = LEN_BASE.partition_point(|&x| x as usize <= len) - 1;
    let (c, l) = litlen_code(257 + li as u32);
    b.put(c, l);
    if LEN_EXTRA[li] > 0 {
        b.put((len - LEN_BASE[li] as usize) as u32, LEN_EXTRA[li] as u32);
    }
    let di = DIST_BASE.partition_point(|&x| x as usize <= dist) - 1;
    b.put(reverse(di as u32, 5), 5);
    if DIST_EXTRA[di] > 0 {
        b.put(
            (dist - DIST_BASE[di] as usize) as u32,
            DIST_EXTRA[di] as u32,
        );
    }
}

const WINDOW: usize = 32 * 1024;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const HASH_BITS: u32 = 15;
const MAX_CHAIN: usize = 48;
const NONE: u32 = u32::MAX;

fn hash3(d: &[u8], i: usize) -> usize {
    let v = (d[i] as u32) | (d[i + 1] as u32) << 8 | (d[i + 2] as u32) << 16;
    (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
}

/// A zlib stream (no preset dictionary, 32 KiB window) holding one fixed-Huffman block.
fn zlib(data: &[u8]) -> Vec<u8> {
    let mut b = Bits {
        out: Vec::with_capacity(data.len() / 2 + 64),
        acc: 0,
        n: 0,
    };
    // CMF: deflate, 32 KiB window; FLG: check bits for CMF*256+FLG % 31 == 0, level "fast".
    b.out.extend_from_slice(&[0x78, 0x01]);
    b.put(1, 1); // BFINAL
    b.put(1, 2); // BTYPE = fixed Huffman
    let mut head = vec![NONE; 1 << HASH_BITS];
    let mut prev = vec![NONE; WINDOW];
    let insert = |head: &mut [u32], prev: &mut [u32], i: usize| {
        if i + MIN_MATCH <= data.len() {
            let h = hash3(data, i);
            prev[i % WINDOW] = head[h];
            head[h] = i as u32;
        }
    };
    let mut i = 0;
    while i < data.len() {
        let mut best_len = 0;
        let mut best_dist = 0;
        if i + MIN_MATCH <= data.len() {
            let max = (data.len() - i).min(MAX_MATCH);
            let mut cand = head[hash3(data, i)];
            let mut chain = 0;
            while cand != NONE && chain < MAX_CHAIN {
                let c = cand as usize;
                if i - c > WINDOW - 1 || c >= i {
                    break;
                }
                // Only a candidate matching bytes ..=best_len can win, so
                // probe the four bytes ending there before measuring.
                let p = best_len.min(max - 1);
                let hit = if p >= 3 {
                    data[c + p - 3..c + p + 1] == data[i + p - 3..i + p + 1]
                } else {
                    data[c + p] == data[i + p]
                };
                if hit {
                    let l = match_len(&data[c..c + max], &data[i..i + max]);
                    if l > best_len {
                        best_len = l;
                        best_dist = i - c;
                        if l == max {
                            break;
                        }
                    }
                }
                let next = prev[c % WINDOW];
                // Chains only run backwards; anything else is a stale slot.
                if next != NONE && next as usize >= c {
                    break;
                }
                cand = next;
                chain += 1;
            }
        }
        if best_len >= MIN_MATCH {
            put_match(&mut b, best_len, best_dist);
            for k in i..i + best_len {
                insert(&mut head, &mut prev, k);
            }
            i += best_len;
        } else {
            put_literal(&mut b, data[i]);
            insert(&mut head, &mut prev, i);
            i += 1;
        }
    }
    let (c, l) = litlen_code(256);
    b.put(c, l);
    let mut out = b.finish();
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// Length of the common prefix of `x` and `y` (equal lengths), eight bytes
/// at a time.
fn match_len(x: &[u8], y: &[u8]) -> usize {
    let mut n = 0;
    for (p, q) in x.chunks_exact(8).zip(y.chunks_exact(8)) {
        let d =
            u64::from_le_bytes(p.try_into().unwrap()) ^ u64::from_le_bytes(q.try_into().unwrap());
        if d != 0 {
            return n + (d.trailing_zeros() / 8) as usize;
        }
        n += 8;
    }
    n + x[n..]
        .iter()
        .zip(&y[n..])
        .take_while(|(a, b)| a == b)
        .count()
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let (mut a, mut b) = (1u32, 0u32);
    for part in data.chunks(5552) {
        for &x in part {
            a += x as u32;
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    b << 16 | a
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;
    use crate::{DecodeOptions, Output};

    fn pattern(w: usize, h: usize, ch: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(w * h * ch);
        let mut s = 0x1234_5678u32;
        for y in 0..h {
            for x in 0..w {
                for c in 0..ch {
                    // Smooth gradients with some noise: every filter gets used.
                    s ^= s << 13;
                    s ^= s >> 17;
                    s ^= s << 5;
                    let noise = if (x / 7 + y / 5) % 3 == 0 {
                        s as u8 & 15
                    } else {
                        0
                    };
                    v.push(((x * 3 + y * 2 + c * 40) as u8).wrapping_add(noise));
                }
            }
        }
        v
    }

    fn roundtrip(w: u32, h: u32, color: ColorType, data: &[u8]) -> Vec<u8> {
        let png = encode(w, h, color, data).unwrap();
        let opts = DecodeOptions {
            output: Output::Rgba8,
            verify_checksums: true,
            ..Default::default()
        };
        let img = crate::decode(&png, &opts).unwrap();
        assert_eq!((img.width, img.height), (w, h));
        assert!(!img.truncated);
        // The upstream decoder agrees (test oracle).
        let dec = png::Decoder::new(std::io::Cursor::new(png.clone()));
        let mut r = dec.read_info().unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        let info = r.next_frame(&mut buf).unwrap();
        assert_eq!(&buf[..info.buffer_size()], data);
        img.to_rgba8()
    }

    #[test]
    fn rgba_rgb_gray_roundtrip() {
        for (w, h) in [(1, 1), (3, 2), (64, 48), (257, 33)] {
            let rgba = pattern(w, h, 4);
            assert_eq!(roundtrip(w as u32, h as u32, ColorType::Rgba, &rgba), rgba);
            let rgb = pattern(w, h, 3);
            let out = roundtrip(w as u32, h as u32, ColorType::Rgb, &rgb);
            for (p, q) in rgb.chunks(3).zip(out.chunks(4)) {
                assert_eq!(p, &q[..3]);
                assert_eq!(q[3], 255);
            }
            let g = pattern(w, h, 1);
            let out = roundtrip(w as u32, h as u32, ColorType::Gray, &g);
            for (p, q) in g.iter().zip(out.chunks(4)) {
                assert_eq!([*p, *p, *p, 255], q);
            }
            let ga = pattern(w, h, 2);
            roundtrip(w as u32, h as u32, ColorType::GrayAlpha, &ga);
        }
    }

    #[test]
    fn long_runs_compress() {
        // Flat colour: matches of the maximum length at distance 4.
        let flat = vec![200u8; 512 * 512 * 4];
        let png = encode(512, 512, ColorType::Rgba, &flat).unwrap();
        assert!(png.len() < 8 * 1024, "{} bytes", png.len());
        assert_eq!(roundtrip(512, 512, ColorType::Rgba, &flat), flat);
        // Repetition farther back than the window must not be referenced.
        let mut far = pattern(20_000, 4, 1);
        far.extend_from_within(..);
        assert_eq!(
            roundtrip(20_000, 8, ColorType::Gray, &far)
                .chunks(4)
                .map(|p| p[0])
                .collect::<Vec<_>>(),
            far
        );
    }

    #[test]
    fn text_chunks_and_errors() {
        let png = encode_with_text(
            1,
            1,
            ColorType::Gray,
            &[7],
            &[("Thumb::URI", "file:///a%20b.jpg"), ("Thumb::MTime", "12")],
        )
        .unwrap();
        let dec = png::Decoder::new(std::io::Cursor::new(png));
        let r = dec.read_info().unwrap();
        let t: Vec<_> = r
            .info()
            .uncompressed_latin1_text
            .iter()
            .map(|c| (c.keyword.clone(), c.text.clone()))
            .collect();
        assert_eq!(
            t,
            [
                ("Thumb::URI".to_owned(), "file:///a%20b.jpg".to_owned()),
                ("Thumb::MTime".to_owned(), "12".to_owned())
            ]
        );
        assert_eq!(
            encode(0, 1, ColorType::Gray, &[]),
            Err(EncodeError::Size(0, 1))
        );
        assert_eq!(
            encode(2, 1, ColorType::Rgb, &[0; 5]),
            Err(EncodeError::DataLength {
                expected: 6,
                got: 5
            })
        );
        assert_eq!(
            encode(1, 1, ColorType::Indexed, &[0]),
            Err(EncodeError::Color(ColorType::Indexed))
        );
        assert_eq!(
            encode_with_text(1, 1, ColorType::Gray, &[0], &[("", "x")]),
            Err(EncodeError::Keyword)
        );
    }

    #[test]
    fn sixteen_bit_with_chunks_and_levels() {
        let (w, h) = (37u32, 21u32);
        let samples: Vec<u16> = (0..w * h * 3)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 16) as u16)
            .collect();
        let be: Vec<u8> = samples.iter().flat_map(|v| v.to_be_bytes()).collect();
        let icc = vec![3u8; 600];
        let exif = b"MM\0\x2a\0\0\0\x08\0\0".to_vec();
        for level in [None, Some(1), Some(9)] {
            let opts = EncodeOptions {
                sixteen_bit: true,
                level,
                icc: Some(&icc),
                exif: Some(&exif),
                xmp: Some(b"<x:xmpmeta/>"),
                cicp: Some([9, 16, 0, 1]),
                ..EncodeOptions::default()
            };
            let png = encode_with(w, h, ColorType::Rgb, &be, &opts).unwrap();
            let info = crate::read_info(&png).unwrap();
            assert_eq!(info.bit_depth, 16);
            assert_eq!(info.icc_profile.as_deref(), Some(&icc[..]));
            assert_eq!(
                info.exif.clone().map(|r| png[r].to_vec()),
                Some(exif.clone())
            );
            assert_eq!(info.cicp.map(|c| c.transfer_function), Some(16));
            assert!(png.windows(17).any(|x| x == b"XML:com.adobe.xmp"));
            // The upstream decoder agrees on every sample (test oracle).
            let dec = png::Decoder::new(std::io::Cursor::new(png.clone()));
            let mut r = dec.read_info().unwrap();
            let mut buf = vec![0; r.output_buffer_size().unwrap()];
            let fi = r.next_frame(&mut buf).unwrap();
            assert_eq!(&buf[..fi.buffer_size()], &be[..]);
        }
        let plain = encode_with(
            1,
            1,
            ColorType::Gray,
            &[9],
            &EncodeOptions {
                srgb: true,
                ..EncodeOptions::default()
            },
        )
        .unwrap();
        assert!(plain.windows(4).any(|x| x == b"sRGB"));
    }

    #[test]
    fn parallel_pieces_decode_and_ignore_thread_count() {
        // 3.4 MB of filtered RGB: four pieces, and filter bands that start mid-image.
        let (w, h) = (1100u32, 1030u32);
        let data = pattern(w as usize, h as usize, 3);
        let enc = |threads, level| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let opts = EncodeOptions {
                level: Some(level),
                ..EncodeOptions::default()
            };
            pool.install(|| encode_with(w, h, ColorType::Rgb, &data, &opts).unwrap())
        };
        for level in [1, 6] {
            let png = enc(1, level);
            assert_eq!(png, enc(4, level));
            // Our decoder checks the combined Adler-32; the upstream one agrees on the pixels.
            let opts = DecodeOptions {
                output: Output::Rgba8,
                verify_checksums: true,
                ..Default::default()
            };
            let img = crate::decode(&png, &opts).unwrap();
            assert!(!img.truncated);
            let dec = png::Decoder::new(std::io::Cursor::new(png.clone()));
            let mut r = dec.read_info().unwrap();
            let mut buf = vec![0; r.output_buffer_size().unwrap()];
            let fi = r.next_frame(&mut buf).unwrap();
            assert_eq!(&buf[..fi.buffer_size()], &data[..]);
            // A single zlib-rs stream of the same filtered rows is hardly smaller.
            let f = filter_rows(&data, w as usize * 3, 3);
            assert_eq!(f, filter_rows_parallel(&data, w as usize * 3, 3));
            let mut one = vec![0u8; zlib_rs::compress_bound(f.len())];
            let cfg = zlib_rs::DeflateConfig::new(i32::from(level));
            let one = zlib_rs::compress_slice(&mut one, &f, cfg).0.len();
            assert!(
                png.len() as f64 <= one as f64 * 1.005 + 200.0,
                "{} vs one stream {one}",
                png.len()
            );
        }
    }

    #[test]
    fn adler_matches_reference() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        let big = vec![255u8; 100_000];
        let mut a = 1u64;
        let mut b = 0u64;
        for &x in &big {
            a = (a + x as u64) % 65521;
            b = (b + a) % 65521;
        }
        assert_eq!(adler32(&big), (b << 16 | a) as u32);
    }
}
