//! bzip2-, xz- and Zstandard-compressed tars (`.tar.bz2`/`.tbz2`, `.tar.xz`/`.txz`,
//! `.tar.zst`/`.tzst`, and the same after `.cbt`): unpacked into memory once, at open, by the own
//! decoders in [`crate::codec`], then listed and read like a plain tar (as `.tar.gz` is, in
//! [`crate::gz`]).
//!
//! None of the three formats is required to state its unpacked size up front, so the output is
//! not sized from a header: it grows as the stream decodes, and the decoder stops before it would
//! pass the cap. The cap is the smallest of [`MAX_UNPACKED`], the archive-wide
//! [`Limits::max_total_size`], and what the ratio limit allows for this many compressed bytes;
//! a bomb costs at most that much memory. The output is reserved from the memory budget as it
//! grows (bzip2 and Zstandard through [`codec::reserving`]; xz reserves its own), before it is
//! allocated, and the unpacked tar's size stays reserved for as long as the archive holds it.
//! bzip2 and xz decode their independent blocks on [`Options::decode_threads`] threads.

use crate::codec::{self, Threads};
use crate::gz::MAX_UNPACKED;
use crate::{Error, Format, Limits, Options, Result};

/// The most bytes a compressed tar of `packed` bytes may unpack to.
fn cap(limits: &Limits, packed: usize) -> usize {
    let by_ratio = limits
        .max_ratio
        .saturating_add(1)
        .saturating_mul(packed as u64)
        .saturating_sub(1)
        .max(limits.ratio_threshold);
    let cap = MAX_UNPACKED.min(limits.max_total_size).min(by_ratio);
    usize::try_from(cap).unwrap_or(usize::MAX)
}

/// The tar inside the compressed tar `b` of format `format` (one of the formats this module
/// reads), and the memory budget held for it (it lives as long as the archive).
pub(crate) fn unpack(
    format: Format,
    b: &[u8],
    options: &Options,
) -> Result<(Vec<u8>, specular_core::budget::Reservation)> {
    let limits = &options.limits;
    let limit = cap(limits, b.len());
    let threads = Threads::of(options);
    let mut out = Vec::new();
    let ((name, r), grown) = match format {
        Format::TarBz2 => {
            codec::reserving(|| ("bzip2", codec::bzip2::decode(b, &mut out, limit, threads)))
        }
        Format::TarXz => (
            (
                "xz",
                codec::xz::decode(b, &mut out, limit, limits.max_decoder_memory, threads),
            ),
            specular_core::Reservation::none(),
        ),
        Format::TarZst => {
            codec::reserving(|| ("Zstandard", codec::zstd::decode(b, &mut out, limit)))
        }
        _ => return Err(Error::NotAnArchive),
    };
    r.map_err(|e| e.into_error(name, limit, false))?;
    out.shrink_to_fit();
    // What decoded stays reserved while the archive holds it (the growth reservation may
    // have run ahead of it: exchanged for the exact size).
    drop(grown);
    let held = crate::reserve(out.len() as u64)?;
    Ok((out, held))
}

/// The compressed-tar format `b` starts like, from magic numbers.
pub(crate) fn detect(b: &[u8]) -> Option<Format> {
    if codec::bzip2::detect(b) {
        Some(Format::TarBz2)
    } else if codec::xz::detect(b) {
        Some(Format::TarXz)
    } else if codec::zstd::detect(b) {
        Some(Format::TarZst)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps() {
        let l = Limits::default();
        // Small streams may still unpack to the ratio threshold.
        assert_eq!(cap(&l, 10) as u64, l.ratio_threshold);
        assert_eq!(cap(&l, 1 << 20) as u64, (l.max_ratio + 1) * (1 << 20) - 1);
        assert_eq!(cap(&l, 1 << 30) as u64, MAX_UNPACKED);
    }
}
