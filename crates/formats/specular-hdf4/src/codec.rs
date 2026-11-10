//! The codings of compressed elements.

use crate::nt::Nt;
use crate::{Error, Result, bad, be16, be32, limits};

/// How a compressed element is coded.
#[derive(Clone, Debug, PartialEq)]
pub enum Coder {
    None,
    /// Run-length coding of bytes.
    Rle,
    /// N-bit packing: `bit_len` bits of every number, the bits `start_bit` down.
    Nbit {
        nt: Nt,
        sign_ext: bool,
        fill_one: bool,
        start_bit: u32,
        bit_len: u32,
    },
    /// Adaptive (splay-tree) Huffman coding with `skip` interleaved trees.
    SkipHuff {
        skip: u32,
    },
    Deflate {
        level: u16,
    },
    Szip {
        options: u32,
        bits_per_pixel: u32,
        pixels_per_block: u32,
        pixels_per_scanline: u32,
    },
}

impl Coder {
    /// Parses the coder number and its information; returns the coder and the bytes used.
    pub(crate) fn parse(coder: u16, info: &[u8]) -> Result<(Coder, usize)> {
        Ok(match coder {
            0 => (Coder::None, 0),
            1 => (Coder::Rle, 0),
            2 => {
                let nt = be32(info, 0).ok_or_else(|| bad("NBIT information"))?;
                let sign = be16(info, 4).ok_or_else(|| bad("NBIT information"))?;
                let fill = be16(info, 6).ok_or_else(|| bad("NBIT information"))?;
                let start = be32(info, 8).ok_or_else(|| bad("NBIT information"))?;
                let len = be32(info, 12).ok_or_else(|| bad("NBIT information"))?;
                let nt = Nt::from_code(nt as u16)?;
                if len == 0 || len > 64 || start >= 64 || len > start + 1 {
                    return Err(bad("NBIT bit range"));
                }
                (
                    Coder::Nbit {
                        nt,
                        sign_ext: sign != 0,
                        fill_one: fill != 0,
                        start_bit: start,
                        bit_len: len,
                    },
                    16,
                )
            }
            3 => {
                let skip = be32(info, 0).ok_or_else(|| bad("Huffman information"))?;
                if skip == 0 || skip > 64 {
                    return Err(bad("Huffman skip size"));
                }
                (Coder::SkipHuff { skip }, 8)
            }
            4 => (
                Coder::Deflate {
                    level: be16(info, 0).unwrap_or(0),
                },
                2,
            ),
            5 => {
                // pixels, pixels per scanline, options (32 bits each), bits per pixel and
                // pixels per block (8 bits each).
                let b = info.get(..14).ok_or_else(|| bad("SZIP information"))?;
                let u = |k: usize| u32::from_be_bytes([b[k], b[k + 1], b[k + 2], b[k + 3]]);
                (
                    Coder::Szip {
                        pixels_per_scanline: u(4),
                        options: u(8),
                        bits_per_pixel: u32::from(b[12]),
                        pixels_per_block: u32::from(b[13]),
                    },
                    14,
                )
            }
            6 => return Err(Error::Unsupported("IMCOMP coding".into())),
            7 => return Err(Error::Unsupported("JPEG coding of a dataset".into())),
            c => return Err(Error::Unsupported(format!("compression {c}"))),
        })
    }

    /// The coding's name, as the HDF4 tools print it.
    pub fn name(&self) -> &'static str {
        match self {
            Coder::None => "none",
            Coder::Rle => "RLE",
            Coder::Nbit { .. } => "NBIT",
            Coder::SkipHuff { .. } => "skipping Huffman",
            Coder::Deflate { .. } => "deflate",
            Coder::Szip { .. } => "SZIP",
        }
    }

    /// Decodes `src` into `len` bytes.
    pub fn decode(&self, src: &[u8], len: usize) -> Result<Vec<u8>> {
        if len as u64 > limits::MAX_ELEMENT {
            return Err(Error::TooLarge);
        }
        let _res = specular_core::budget::reserve(len as u64)?;
        let mut out = match self {
            Coder::None => src.to_vec(),
            Coder::Rle => rle(src, len),
            Coder::Nbit {
                nt,
                sign_ext,
                fill_one,
                start_bit,
                bit_len,
            } => nbit(src, len, *nt, *sign_ext, *fill_one, *start_bit, *bit_len),
            Coder::SkipHuff { skip } => skip_huffman(src, len, *skip as usize),
            Coder::Deflate { .. } => {
                specular_png::inflate_zlib(src, len).map_err(|_| bad("deflate data"))?
            }
            Coder::Szip {
                options,
                bits_per_pixel,
                pixels_per_block,
                pixels_per_scanline,
            } => szip(
                src,
                len,
                *options,
                *bits_per_pixel,
                *pixels_per_block,
                *pixels_per_scanline,
            )?,
        };
        out.resize(len, 0);
        Ok(out)
    }
}

/// Run-length coding: a control byte with the top bit set repeats the next byte
/// `(c & 0x7f) + 3` times; otherwise `c + 1` literal bytes follow.
pub fn rle(src: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    while out.len() < len && i < src.len() {
        let c = src[i];
        i += 1;
        if c & 0x80 != 0 {
            let Some(&v) = src.get(i) else { break };
            i += 1;
            let n = (usize::from(c & 0x7f) + 3).min(len - out.len());
            out.resize(out.len() + n, v);
        } else {
            let n = (usize::from(c) + 1).min(len - out.len());
            let end = (i + n).min(src.len());
            out.extend_from_slice(&src[i..end]);
            i = end;
        }
    }
    out
}

/// The run-length coding of old raster images (compression tag 11): a count byte with the
/// top bit set repeats the next byte `c & 0x7f` times; otherwise `c` literal bytes follow.
pub fn raster_rle(src: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    while out.len() < len && i < src.len() {
        let c = src[i];
        i += 1;
        let n = usize::from(c & 0x7f).min(len - out.len());
        if c & 0x80 != 0 {
            let Some(&v) = src.get(i) else { break };
            i += 1;
            out.resize(out.len() + n, v);
        } else {
            let end = (i + n).min(src.len());
            out.extend_from_slice(&src[i..end]);
            i = end;
        }
    }
    out
}

/// N-bit packing: every number keeps `bit_len` bits, packed most significant first; they go
/// back at bits `start_bit` down, the bits above sign-extended (or set when `fill_one`), the
/// bits below set when `fill_one`.
fn nbit(
    src: &[u8],
    len: usize,
    nt: Nt,
    sign_ext: bool,
    fill_one: bool,
    start_bit: u32,
    bit_len: u32,
) -> Vec<u8> {
    let size = nt.size();
    let n = len / size;
    let mut out = Vec::with_capacity(len);
    let low = start_bit + 1 - bit_len;
    let width = (size * 8) as u32;
    let all: u64 = if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    let field_mask: u64 = if bit_len >= 64 {
        u64::MAX
    } else {
        ((1u64 << bit_len) - 1) << low
    };
    let above: u64 = all
        & !(if start_bit + 1 >= 64 {
            u64::MAX
        } else {
            (1u64 << (start_bit + 1)) - 1
        });
    let below: u64 = (1u64 << low) - 1;
    let mut bitpos = 0usize;
    let total_bits = src.len() * 8;
    for _ in 0..n {
        if bitpos + bit_len as usize > total_bits {
            break;
        }
        let mut v = 0u64;
        for _ in 0..bit_len {
            let b = (src[bitpos >> 3] >> (7 - (bitpos & 7))) & 1;
            v = (v << 1) | u64::from(b);
            bitpos += 1;
        }
        let mut x = (v << low) & field_mask;
        let top = v >> (bit_len - 1) & 1 == 1;
        if sign_ext {
            if top {
                x |= above;
            }
        } else if fill_one {
            x |= above;
        }
        if fill_one {
            x |= below;
        }
        let x = x & all;
        let be = x.to_be_bytes();
        let bytes = &be[8 - size..];
        if nt.little {
            out.extend(bytes.iter().rev());
        } else {
            out.extend_from_slice(bytes);
        }
    }
    out
}

/// Adaptive Huffman coding on splay trees (Jones): one tree per byte position modulo `skip`.
/// Node 0 is the root; node `j` (`j < 256`) has children `2j` and `2j + 1`, so the root's left
/// child is itself; nodes from 256 on are the leaves of byte `node − 256`. Bits are read most
/// significant first, and the tree splays after every byte.
pub fn skip_huffman(src: &[u8], len: usize, skip: usize) -> Vec<u8> {
    const MAXCHAR: usize = 256;
    struct Tree {
        up: [u16; 2 * MAXCHAR],
        left: [u16; MAXCHAR],
        right: [u16; MAXCHAR],
    }
    let mut trees: Vec<Tree> = (0..skip)
        .map(|_| {
            let mut t = Tree {
                up: [0; 2 * MAXCHAR],
                left: [0; MAXCHAR],
                right: [0; MAXCHAR],
            };
            for (i, u) in t.up.iter_mut().enumerate() {
                *u = (i >> 1) as u16;
            }
            for j in 0..MAXCHAR {
                t.left[j] = (2 * j) as u16;
                t.right[j] = (2 * j + 1) as u16;
            }
            t
        })
        .collect();
    let mut out = Vec::with_capacity(len);
    let total = src.len() * 8;
    let mut bit = 0usize;
    // A malformed stream could spin on the root's self loop; bound the bits per byte.
    'bytes: while out.len() < len {
        let t = &mut trees[out.len() % skip];
        let mut a = 0usize;
        let mut steps = 0;
        while a < MAXCHAR {
            if bit >= total {
                break 'bytes;
            }
            let b = (src[bit >> 3] >> (7 - (bit & 7))) & 1;
            bit += 1;
            a = usize::from(if b == 1 { t.right[a] } else { t.left[a] });
            steps += 1;
            if steps > 4 * MAXCHAR {
                break 'bytes;
            }
        }
        out.push((a - MAXCHAR) as u8);
        // Splay the leaf towards the root.
        loop {
            let c = usize::from(t.up[a]);
            if c == 0 {
                break;
            }
            let d = usize::from(t.up[c]);
            let mut b = usize::from(t.left[d]);
            if c == b {
                b = usize::from(t.right[d]);
                t.right[d] = a as u16;
            } else {
                t.left[d] = a as u16;
            }
            if a == usize::from(t.left[c]) {
                t.left[c] = b as u16;
            } else {
                t.right[c] = b as u16;
            }
            t.up[a] = d as u16;
            t.up[b] = c as u16;
            a = d;
            if a == 0 {
                break;
            }
        }
    }
    out
}

/// The option bit HDF4 4.2 sets on SZIP data it writes: the element starts with a 5-byte
/// header (a 4-byte field, then a flag byte) before the AEC stream.
const SZ_H4_REV_2: u32 = 0x1_0000;

/// SZIP (CCSDS 121 AEC). An element no smaller than the decoded size was stored raw. Data
/// written by HDF4 4.2 (option bit [`SZ_H4_REV_2`]) has a 5-byte header before the AEC
/// stream; older data is tried as it is, then after a 4-byte length prefix. HDF4 numbers are
/// big-endian unless the options say little-endian.
fn szip(
    src: &[u8],
    len: usize,
    options: u32,
    bits_per_pixel: u32,
    pixels_per_block: u32,
    pixels_per_scanline: u32,
) -> Result<Vec<u8>> {
    use specular_aec::szip::{LSB, MSB_OPTION, RAW, SzParams};
    let rev2 = options & SZ_H4_REV_2 != 0;
    let body = if rev2 {
        src.get(5..).unwrap_or_default()
    } else {
        src
    };
    if body.len() >= len {
        return Ok(body[..len].to_vec());
    }
    let mut options = (options & 0xffff) | RAW;
    if options & LSB == 0 {
        options |= MSB_OPTION;
    }
    let p = SzParams {
        options,
        bits_per_pixel,
        pixels_per_block,
        pixels_per_scanline,
    };
    let map = |e: specular_aec::Error| match e {
        specular_aec::Error::Params(w) => Error::Unsupported(format!("SZIP: {w}")),
        _ => bad("SZIP data"),
    };
    match specular_aec::szip::decode(body, &p, len) {
        Ok(v) => Ok(v),
        Err(e) => {
            if !rev2
                && src.len() > 4
                && u32::from_le_bytes([src[0], src[1], src[2], src[3]]) as usize == len
            {
                specular_aec::szip::decode(&src[4..], &p, len).map_err(map)
            } else {
                Err(map(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_lengths() {
        assert_eq!(rle(&[0x81, 7, 1, 1, 2], 6), vec![7, 7, 7, 7, 1, 2]);
        assert_eq!(raster_rle(&[0x83, 9, 2, 4, 5], 5), vec![9, 9, 9, 4, 5]);
    }

    /// A 1 GiB element (under the cap) is refused under a 64 MiB memory budget before its
    /// output is allocated; a small one decodes.
    #[test]
    fn element_within_memory_budget() {
        use specular_core::budget::with_limit;
        let e = with_limit(64 << 20, || Coder::Rle.decode(&[0x81, 7], 1 << 30)).unwrap_err();
        let Error::NeedsMemory(n) = e else {
            panic!("{e:?}")
        };
        assert!(n.needed >= 1 << 30 && n.needed > n.free, "{n:?}");
        let out = with_limit(64 << 20, || Coder::Rle.decode(&[0x81, 7], 2)).unwrap();
        assert_eq!(out, [7, 7]);
    }

    #[test]
    fn refusals() {
        assert!(matches!(Coder::parse(6, &[]), Err(Error::Unsupported(_))));
        assert!(matches!(Coder::parse(9, &[]), Err(Error::Unsupported(_))));
        assert!(Coder::parse(3, &[0, 0, 0, 0, 0, 0, 0, 0]).is_err());
    }

    /// SDSszip of the HDF4 library's hdp test file sds_compressed.hdf (written by HDF4 4.2,
    /// options EC | RAW | SZ_H4_REV_2): a 16 × 5 int32 dataset whose (i, j) value is
    /// i + j + 1, the same as its gzip-compressed sibling.
    #[test]
    fn szip_written_by_hdf4() {
        let mut src = vec![0u8, 0, 0, 0, 0x73];
        src.extend_from_slice(&[0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x81].repeat(6));
        src.extend_from_slice(&[
            0x29, 0x49, 0x91, 0xc9, 0x54, 0x4a, 0x86, 0x24, 0xcd, 0x59, 0x0e, 0x44, 0xad, 0x6d,
            0x06, 0x35, 0x66, 0x9c, 0x41, 0xcd, 0x6d, 0x92, 0x2c, 0xe1, 0xa7, 0x19, 0x2c, 0xcf,
            0x19, 0x22, 0xc9, 0xb6, 0x30, 0x64, 0xb4, 0x57, 0x18, 0xd1, 0x93, 0x71, 0x65, 0x63,
            0x88, 0xae, 0x45, 0xb9, 0x8f, 0x22, 0xcb, 0x17, 0x76, 0x18, 0x45, 0xba, 0x27, 0x86,
            0x1a, 0x45, 0xde, 0x12, 0x0b, 0x0e, 0x22, 0x78, 0x84, 0x94, 0xc3, 0xc8, 0x48, 0x30,
            0x94, 0xe1, 0xc0,
        ]);
        assert_eq!(src.len(), 120);
        let out = szip(&src, 320, 0x1_0084, 32, 2, 5).unwrap();
        let want: Vec<u8> = (0..16)
            .flat_map(|i| (0..5).map(move |j| i + j + 1))
            .flat_map(|v: i32| v.to_be_bytes())
            .collect();
        assert_eq!(out, want);
    }
}
