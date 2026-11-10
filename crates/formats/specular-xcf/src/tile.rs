//! One 64×64 tile: uncompressed, RLE or zlib, to interleaved pixel bytes (byte order as
//! stored; [`crate::pixel::Loader`] reads it).

use crate::parse::{Compression, Plane};

pub(crate) const TILE: u32 = 64;

/// Size of tile `index` of `plane`.
pub(crate) fn tile_size(plane: &Plane, index: usize) -> (u32, u32) {
    let tx = (index as u32) % plane.tiles_x();
    let ty = (index as u32) / plane.tiles_x();
    (
        TILE.min(plane.width - tx * TILE),
        TILE.min(plane.height - ty * TILE),
    )
}

/// Decodes tile `index`. `Ok(None)`: the tile is absent from the table (transparent).
/// `Err(())`: its data is cut short or corrupt (whatever decoded is returned in the `Vec`).
pub(crate) fn decode(
    data: &[u8],
    plane: &Plane,
    index: usize,
    compression: Compression,
) -> std::result::Result<Option<Vec<u8>>, Vec<u8>> {
    let Some(&start) = plane.tiles.get(index) else {
        return Ok(None);
    };
    let (tw, th) = tile_size(plane, index);
    let n = (tw * th) as usize;
    let bpp = plane.bpp;
    let size = n * bpp;
    let mut out = vec![0u8; size];
    let start = start as usize;
    if start >= data.len() {
        return Err(out);
    }
    // RLE and zlib stop by themselves once the tile is full, so the next tile's offset is not
    // needed as a bound.
    let src = &data[start..];
    let ok = match compression {
        Compression::None => {
            let k = size.min(src.len());
            out[..k].copy_from_slice(&src[..k]);
            k == size
        }
        Compression::Rle => rle(src, &mut out, bpp, n),
        Compression::Zlib => zlib(src, &mut out),
    };
    if ok { Ok(Some(out)) } else { Err(out) }
}

/// GIMP's tile RLE: one stream per byte of the pixel (all first bytes, then all second
/// bytes, …), each a run of opcodes:
///
/// * `0..=126` — the next byte repeated `n + 1` times,
/// * `127` — a 16-bit big-endian count, then the byte to repeat,
/// * `128` — a 16-bit big-endian count of literal bytes,
/// * `129..=255` — `256 - n` literal bytes.
fn rle(src: &[u8], out: &mut [u8], bpp: usize, n: usize) -> bool {
    let mut p = 0usize;
    for k in 0..bpp {
        let mut i = 0usize;
        while i < n {
            let Some(&op) = src.get(p) else {
                return false;
            };
            p += 1;
            let (count, literal) = match op {
                0..=126 => (op as usize + 1, false),
                127 | 128 => {
                    let Some(b) = src.get(p..p + 2) else {
                        return false;
                    };
                    p += 2;
                    (usize::from(b[0]) << 8 | usize::from(b[1]), op == 128)
                }
                _ => (256 - op as usize, true),
            };
            if count > n - i {
                return false;
            }
            if literal {
                let Some(b) = src.get(p..p + count) else {
                    return false;
                };
                p += count;
                for (j, &v) in b.iter().enumerate() {
                    out[(i + j) * bpp + k] = v;
                }
            } else {
                let Some(&v) = src.get(p) else {
                    return false;
                };
                p += 1;
                if v != 0 {
                    for j in i..i + count {
                        out[j * bpp + k] = v;
                    }
                }
            }
            i += count;
        }
    }
    true
}

fn zlib(src: &[u8], out: &mut [u8]) -> bool {
    let mut inf = zlib_rs::Inflate::new(true, 15);
    match inf.decompress(src, out, zlib_rs::InflateFlush::Finish) {
        Ok(zlib_rs::Status::StreamEnd) => inf.total_out() as usize == out.len(),
        // The output filled exactly; a few trailing bytes (the Adler-32) may be unread.
        Ok(_) => inf.total_out() as usize == out.len(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rle_ops() {
        // bpp 2, 4 pixels: stream 0 = run of 4×7; stream 1 = literal 1,2 then long run 3×2.
        let src = [3, 7, 254, 1, 2, 127, 0, 2, 9];
        let mut out = [0u8; 8];
        assert!(rle(&src, &mut out, 2, 4));
        assert_eq!(out, [7, 1, 7, 2, 7, 9, 7, 9]);
        // Long literal.
        let src = [128, 0, 3, 4, 5, 6];
        let mut out = [0u8; 3];
        assert!(rle(&src, &mut out, 1, 3));
        assert_eq!(out, [4, 5, 6]);
        // Overrun and truncation are errors, not panics.
        assert!(!rle(&[5, 1], &mut [0u8; 3], 1, 3));
        assert!(!rle(&[129], &mut [0u8; 3], 1, 3));
    }
}
