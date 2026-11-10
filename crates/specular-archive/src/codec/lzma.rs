//! ZIP method 14 (LZMA): a 4-byte header (LZMA SDK version, then the size of the properties,
//! always 5), the properties (the `lc`/`lp`/`pb` byte and the dictionary size), then a raw LZMA
//! stream, ended by the entry's declared size or by an end marker (general-purpose flag bit 1).
//!
//! The range decoder is `lzma_rust2`'s (the one the 7z reader already runs); this module owns the
//! header, the size and memory caps, and the output limit.

use std::io::Read;

use lzma_rust2::LzmaReader;

use super::{CResult, CodecError, room};

/// Decodes the method-14 data `data` of an entry declaring `size` unpacked bytes, appending
/// exactly `size` bytes to `out` (more is [`CodecError::Limit`], fewer
/// [`CodecError::Truncated`]). `max_memory` caps the dictionary the stream may ask for.
pub(crate) fn decode_zip(
    data: &[u8],
    size: usize,
    out: &mut Vec<u8>,
    max_memory: u64,
) -> CResult<()> {
    let head = data.get(..9).ok_or(CodecError::Truncated)?;
    let props_len = usize::from(u16::from_le_bytes([head[2], head[3]]));
    if props_len != 5 {
        return Err(CodecError::Corrupt("unexpected LZMA properties size"));
    }
    let props = head[4];
    if props > (4 * 5 + 4) * 9 + 8 {
        return Err(CodecError::Corrupt("bad LZMA properties"));
    }
    // The reader never keeps more history than the entry's size: a stream claiming a 4 GiB
    // dictionary for a 10 KiB entry costs 10 KiB (with a 4 KiB floor).
    let dict = u32::from_le_bytes([head[5], head[6], head[7], head[8]]);
    let needed = u64::from(dict).min(size as u64).max(4096);
    if needed > max_memory {
        return Err(CodecError::Unsupported(
            "LZMA dictionary larger than allowed",
        ));
    }
    room(out, size, usize::MAX)?;
    let dict = u32::try_from(needed).map_err(|_| CodecError::Corrupt("bad LZMA dictionary"))?;
    let reader = LzmaReader::new_with_props(&data[9..], size as u64, props, dict, None)
        .map_err(|_| CodecError::Corrupt("bad LZMA stream start"))?;
    read_exact_into(reader, size, out)
}

/// Reads `r` to its end into `out`, which must then have grown by exactly `size` bytes.
pub(crate) fn read_exact_into(mut r: impl Read, size: usize, out: &mut Vec<u8>) -> CResult<()> {
    let start = out.len();
    // Grow as data arrives (a size near the cap is not trusted until it decodes).
    out.reserve(size.min(16 << 20));
    let mut got = 0usize;
    loop {
        if out.len() == start + got {
            let step = (size - got).clamp(1, 1 << 20) + if got == size { 1 } else { 0 };
            out.resize(start + got + step, 0);
        }
        match r.read(&mut out[start + got..]) {
            Ok(0) => break,
            Ok(n) => {
                got += n;
                if got > size {
                    out.truncate(start);
                    return Err(CodecError::Limit);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => {
                out.truncate(start);
                return Err(if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    CodecError::Truncated
                } else {
                    CodecError::Corrupt("bad LZMA data")
                });
            }
        }
    }
    out.truncate(start + got);
    if got < size {
        out.truncate(start);
        return Err(CodecError::Truncated);
    }
    Ok(())
}
