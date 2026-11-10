//! ROOT's compression framing: an object's bytes are one or more blocks, each a 9-byte
//! header (two algorithm letters, a method byte, the compressed and uncompressed sizes as
//! 24-bit little-endian) and its payload. `ZL` is a zlib stream, `CS` ROOT's old DEFLATE
//! without the zlib wrapper, `XZ` an xz stream, `L4` an xxHash64 of the block then a raw
//! LZ4 block, `ZS` a Zstandard frame.

use crate::{Error, Result};

/// The algorithms named by fCompress / 100.
pub fn algorithm_name(alg: i32) -> &'static str {
    match alg {
        0 => "zlib",
        1 => "zlib",
        2 => "LZMA",
        3 => "old ROOT",
        4 => "LZ4",
        5 => "ZSTD",
        _ => "unknown",
    }
}

/// Decompresses `src` (the bytes after the key) into exactly `objlen` bytes, block by block;
/// with `prefix`, stops once that many bytes are out (for headers of big objects).
pub(crate) fn unzip(src: &[u8], objlen: usize, prefix: Option<usize>) -> Result<Vec<u8>> {
    let want = prefix.map_or(objlen, |p| p.min(objlen));
    let mut out = Vec::with_capacity(want);
    let mut p = 0usize;
    while out.len() < want {
        let Some(h) = src.get(p..p + 9) else {
            return Err(Error::Truncated(format!(
                "compressed object ends at {} of {objlen} bytes",
                out.len()
            )));
        };
        let csize = u32::from_le_bytes([h[3], h[4], h[5], 0]) as usize;
        let usize_ = u32::from_le_bytes([h[6], h[7], h[8], 0]) as usize;
        let Some(data) = src.get(p + 9..p + 9 + csize) else {
            return Err(Error::Truncated("a compressed block past its key".into()));
        };
        if usize_ == 0 || out.len() + usize_ > objlen {
            return Err(Error::Compression(format!(
                "a block of {usize_} bytes overruns the object's {objlen}"
            )));
        }
        let block = match &h[..2] {
            b"ZL" => specular_png::inflate_zlib(data, usize_)
                .map_err(|e| Error::Compression(format!("zlib: {e}")))?,
            // ROOT's own DEFLATE from before zlib (files of the 1990s): raw DEFLATE.
            b"CS" => specular_png::inflate_raw(data, usize_)
                .map_err(|e| Error::Compression(format!("old ROOT algorithm: {e}")))?,
            b"XZ" => xz(data, usize_)?,
            b"L4" => {
                if data.len() < 8 {
                    return Err(Error::Compression("LZ4 block without its checksum".into()));
                }
                let sum = u64::from_be_bytes(data[..8].try_into().unwrap_or_default());
                if xxh64(&data[8..]) != sum {
                    return Err(Error::Compression("LZ4 block checksum mismatch".into()));
                }
                specular_hdf5::codec::lz4_block(&data[8..], usize_)
                    .map_err(|e| Error::Compression(format!("LZ4: {e}")))?
            }
            b"ZS" => specular_hdf5::codec::zstd(data, usize_)
                .map_err(|e| Error::Compression(format!("ZSTD: {e}")))?,
            other => {
                return Err(Error::Compression(format!(
                    "unknown algorithm {:?}",
                    String::from_utf8_lossy(other)
                )));
            }
        };
        if block.len() != usize_ {
            return Err(Error::Compression(format!(
                "a block gave {} bytes, its header says {usize_}",
                block.len()
            )));
        }
        out.extend_from_slice(&block);
        p += 9 + csize;
    }
    if prefix.is_none() && out.len() != objlen {
        return Err(Error::Compression(format!(
            "{} bytes out, the key says {objlen}",
            out.len()
        )));
    }
    Ok(out)
}

/// An xz stream (one or more LZMA2 blocks, as liblzma writes for ROOT) to exactly `n` bytes.
fn xz(input: &[u8], n: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    const MAGIC: [u8; 6] = [0xFD, b'7', b'z', b'X', b'Z', 0];
    let bad = |m: &str| Error::Compression(format!("XZ: {m}"));
    if !input.starts_with(&MAGIC) || input.len() < 12 {
        return Err(bad("no stream header"));
    }
    let check = match input[7] & 0x0f {
        0 => 0,
        1..=3 => 4,
        4..=6 => 8,
        7..=9 => 16,
        _ => 32,
    };
    let mut out = vec![0u8; n];
    let mut p = 12usize;
    let mut got = 0usize;
    while got < n {
        let Some(&hs) = input.get(p) else {
            return Err(bad("stream ends early"));
        };
        if hs == 0 {
            return Err(bad("index before the data ends"));
        }
        let hlen = (hs as usize + 1) * 4;
        let h = input.get(p..p + hlen).ok_or_else(|| bad("block header"))?;
        let flags = h[1];
        let mut q = 2usize;
        let varint = |q: &mut usize| -> Option<u64> {
            let mut v = 0u64;
            for i in 0..9 {
                let b = *h.get(*q)?;
                *q += 1;
                v |= u64::from(b & 0x7f) << (7 * i);
                if b & 0x80 == 0 {
                    return Some(v);
                }
            }
            None
        };
        let mut packed = None;
        if flags & 0x40 != 0 {
            packed = varint(&mut q);
        }
        if flags & 0x80 != 0 && varint(&mut q).is_none() {
            return Err(bad("block header"));
        }
        if flags & 3 != 0 {
            return Err(bad("filters other than LZMA2"));
        }
        let (Some(0x21), Some(1)) = (varint(&mut q), varint(&mut q)) else {
            return Err(bad("filters other than LZMA2"));
        };
        let props = *h.get(q).ok_or_else(|| bad("block header"))?;
        let bits = u32::from(props & 0x3f);
        if bits > 40 {
            return Err(bad("dictionary size"));
        }
        let dict = if bits == 40 {
            u32::MAX
        } else {
            (2 | (bits & 1)) << (bits / 2 + 11)
        };
        p += hlen;
        let mut rest = input.get(p..).unwrap_or_default();
        let before = rest.len();
        let mut rd = lzma_rust2::Lzma2Reader::new(&mut rest, dict, None);
        loop {
            if got == n {
                // The block must end here: one more read gives 0.
                let mut one = [0u8; 1];
                match rd.read(&mut one) {
                    Ok(0) => break,
                    _ => return Err(bad("more data than the block header says")),
                }
            }
            match rd.read(&mut out[got..]) {
                Ok(0) => break,
                Ok(k) => got += k,
                Err(e) => return Err(bad(&e.to_string())),
            }
        }
        drop(rd);
        let used = packed.map_or(before - rest.len(), |c| c as usize);
        p += used.div_ceil(4) * 4 + check;
    }
    Ok(out)
}

/// xxHash64 with seed 0 (ROOT's checksum of LZ4 blocks).
pub(crate) fn xxh64(data: &[u8]) -> u64 {
    const P1: u64 = 0x9E37_79B1_85EB_CA87;
    const P2: u64 = 0xC2B2_AE3D_27D4_EB4F;
    const P3: u64 = 0x1656_67B1_9E37_79F9;
    const P4: u64 = 0x85EB_CA77_C2B2_AE63;
    const P5: u64 = 0x27D4_EB2F_1656_67C5;
    let rd64 = |s: &[u8]| u64::from_le_bytes(s[..8].try_into().unwrap_or_default());
    let round = |acc: u64, v: u64| {
        acc.wrapping_add(v.wrapping_mul(P2))
            .rotate_left(31)
            .wrapping_mul(P1)
    };
    let merge = |acc: u64, v: u64| (acc ^ round(0, v)).wrapping_mul(P1).wrapping_add(P4);
    let n = data.len();
    let mut s = data;
    let mut h = if n >= 32 {
        let mut v = [P1.wrapping_add(P2), P2, 0, 0u64.wrapping_sub(P1)];
        while s.len() >= 32 {
            for (k, acc) in v.iter_mut().enumerate() {
                *acc = round(*acc, rd64(&s[k * 8..]));
            }
            s = &s[32..];
        }
        let mut h = v[0]
            .rotate_left(1)
            .wrapping_add(v[1].rotate_left(7))
            .wrapping_add(v[2].rotate_left(12))
            .wrapping_add(v[3].rotate_left(18));
        for acc in v {
            h = merge(h, acc);
        }
        h
    } else {
        P5
    };
    h = h.wrapping_add(n as u64);
    while s.len() >= 8 {
        h ^= round(0, rd64(s));
        h = h.rotate_left(27).wrapping_mul(P1).wrapping_add(P4);
        s = &s[8..];
    }
    if s.len() >= 4 {
        let v = u64::from(u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
        h ^= v.wrapping_mul(P1);
        h = h.rotate_left(23).wrapping_mul(P2).wrapping_add(P3);
        s = &s[4..];
    }
    for &b in s {
        h ^= u64::from(b).wrapping_mul(P5);
        h = h.rotate_left(11).wrapping_mul(P1);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(P2);
    h ^= h >> 29;
    h = h.wrapping_mul(P3);
    h ^ (h >> 32)
}

#[cfg(test)]
mod tests {
    use super::xxh64;

    #[test]
    fn xxh64_known_values() {
        assert_eq!(xxh64(b""), 0xEF46_DB37_51D8_E999);
        assert_eq!(xxh64(b"a"), 0xD24E_C4F1_A98C_6E5B);
        assert_eq!(
            xxh64(b"Nobody inspects the spammish repetition"),
            0xFBCE_A83C_8A37_8BF1
        );
    }
}
