//! Raw Snappy blocks (Google's format description): a varint length, then literals and
//! back-references. Adapted from the in-tree reader in `specular-gis` (`codec::snappy`) to
//! write into a slice of the exact declared size, so the chunks of a frame decompress side by
//! side into one texture buffer. Every copy is bounds-checked.

/// The decompressed length a block declares, and where its tags start.
pub(crate) fn length(src: &[u8]) -> Result<(usize, usize), &'static str> {
    let mut len = 0usize;
    let mut shift = 0;
    for (i, &b) in src.iter().enumerate() {
        len |= usize::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            return Ok((len, i + 1));
        }
        shift += 7;
        if shift > 28 {
            break;
        }
    }
    Err("Snappy length is malformed")
}

/// Decompresses `src` into `out`, whose length must equal the declared length.
pub(crate) fn decompress_into(src: &[u8], out: &mut [u8]) -> Result<(), &'static str> {
    let (len, mut i) = length(src)?;
    if len != out.len() {
        return Err("Snappy block has another size than its chunk");
    }
    let mut o = 0usize;
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
                let n = n.checked_add(1).ok_or("Snappy literal is malformed")?;
                let end = i.checked_add(n).ok_or("Snappy literal is malformed")?;
                let lit = src.get(i..end).ok_or("Snappy literal is truncated")?;
                out.get_mut(o..o + n)
                    .ok_or("Snappy data overruns its length")?
                    .copy_from_slice(lit);
                o += n;
                i = end;
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
        if off == 0 || off > o {
            return Err("a Snappy back-reference points before the data");
        }
        if o + n > out.len() {
            return Err("Snappy data overruns its length");
        }
        let start = o - off;
        if off >= n {
            out.copy_within(start..start + n, o);
        } else {
            for k in 0..n {
                out[o + k] = out[start + k];
            }
        }
        o += n;
    }
    if o != out.len() {
        return Err("Snappy data is shorter than its length");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_and_overlapping_copy() {
        // Length 10: literal "ab", then a 1-byte-offset copy of 8 ("abababab").
        let src = [10, 1 << 2, b'a', b'b', 0b001 | ((8 - 4) << 2), 2];
        let mut out = [0u8; 10];
        decompress_into(&src, &mut out).unwrap();
        assert_eq!(&out, b"ababababab");
        assert!(decompress_into(&src, &mut [0u8; 9]).is_err());
        // A copy from before the start.
        assert!(decompress_into(&[4, 0b001, 5], &mut [0u8; 4]).is_err());
        // Truncated literal; unterminated varint.
        assert!(decompress_into(&[4, 3 << 2, b'a'], &mut [0u8; 4]).is_err());
        assert!(length(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x80]).is_err());
    }
}
