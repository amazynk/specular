//! Specular's own decompressors for the ZIP methods and compressed tars beyond Deflate:
//! Deflate64 ([`inflate64`]), bzip2 ([`bzip2`]), Zstandard ([`zstd`]), xz ([`xz`]) and ZIP's
//! LZMA ([`lzma`]).
//!
//! Every decoder appends to a `Vec<u8>` and never lets it grow past a `limit` the caller sets:
//! the declared size of a ZIP entry, or the size and ratio caps of a compressed tar. A stream that
//! would produce more stops with [`CodecError::Limit`] before allocating it, so a lying header
//! or a decompression bomb costs at most `limit` bytes. Corrupt and truncated input is an error,
//! never a panic. bzip2 and xz decode their independent blocks on several threads
//! ([`Threads`]).

pub(crate) mod bzip2;
pub(crate) mod check;
pub(crate) mod inflate64;
pub(crate) mod lzma;
#[cfg(test)]
mod oracle;
pub(crate) mod xz;
pub(crate) mod zstd;

use std::cell::{Cell, RefCell};

use crate::Error;

/// Why a stream did not decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CodecError {
    /// The data is not a valid stream.
    Corrupt(&'static str),
    /// The data ends early.
    Truncated,
    /// The output would pass the caller's limit.
    Limit,
    /// The data decoded, but its checksum does not match.
    Checksum,
    /// A valid stream using a feature this decoder does not have.
    Unsupported(&'static str),
    /// The memory budget cannot hold the output (the budget's refusal, worded).
    Memory(String),
}

pub(crate) type CResult<T> = std::result::Result<T, CodecError>;

impl CodecError {
    /// The crate error, for a stream of `codec` (e.g. "bzip2") whose output was limited to
    /// `limit` bytes: `declared` when that limit is a size the archive declared (then passing
    /// it means the archive is malformed rather than too large).
    pub(crate) fn into_error(self, codec: &str, limit: usize, declared: bool) -> Error {
        match self {
            CodecError::Corrupt(m) => Error::Malformed(format!("{codec}: {m}")),
            CodecError::Truncated => Error::Malformed(format!("{codec}: truncated stream")),
            CodecError::Limit if declared => {
                Error::Malformed(format!("{codec}: larger than its declared {limit} bytes"))
            }
            CodecError::Limit => Error::Limit(format!(
                "{codec} stream decompresses to more than {limit} bytes"
            )),
            CodecError::Checksum => Error::Checksum,
            CodecError::Unsupported(m) => Error::Unsupported(format!("{codec}: {m}")),
            CodecError::Memory(m) => Error::Limit(format!("{codec}: {m}")),
        }
    }
}

/// Room for `n` more bytes in `out` without passing `limit`, and, inside [`reserving`], within
/// the memory budget.
#[inline]
pub(crate) fn room(out: &[u8], n: usize, limit: usize) -> CResult<()> {
    match out.len().checked_add(n) {
        Some(end) if end <= limit => {
            if end > COVERED.with(Cell::get) {
                grow(end, limit)?;
            }
            Ok(())
        }
        _ => Err(CodecError::Limit),
    }
}

thread_local! {
    /// The output reservation of the [`reserving`] decode running on this thread.
    static GROWTH: RefCell<Option<specular_core::Reservation>> = const { RefCell::new(None) };
    /// Output bytes it covers (all of them outside [`reserving`]).
    static COVERED: Cell<usize> = const { Cell::new(usize::MAX) };
}

/// Grows the reservation to cover `end` bytes of output: doubling, at least 1 MiB, never past
/// `limit`.
#[cold]
fn grow(end: usize, limit: usize) -> CResult<()> {
    GROWTH.with(|g| {
        let mut g = g.borrow_mut();
        let Some(r) = g.as_mut() else {
            return Ok(());
        };
        let had = usize::try_from(r.bytes()).unwrap_or(usize::MAX);
        let want = end.max(had.saturating_mul(2)).max(1 << 20).min(limit);
        r.grow_to(want as u64)
            .map_err(|e| CodecError::Memory(e.to_string()))?;
        COVERED.with(|c| c.set(want));
        Ok(())
    })
}

/// Runs `decode` with the output its `room` checks allow reserved from the memory budget as
/// it grows (for streams that do not declare their size: the reservation is made before the
/// bytes are), and returns the reservation with the result. bzip2 blocks in flight on other
/// threads hold their own reservations until they are appended here.
pub(crate) fn reserving<R>(decode: impl FnOnce() -> R) -> (R, specular_core::Reservation) {
    struct Restore(Option<specular_core::Reservation>, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            let (prev, covered) = (self.0.take(), self.1);
            GROWTH.with(|g| *g.borrow_mut() = prev);
            COVERED.with(|c| c.set(covered));
        }
    }
    let prev = GROWTH.with(|g| g.borrow_mut().replace(specular_core::Reservation::none()));
    let restore = Restore(prev, COVERED.with(|c| c.replace(0)));
    let r = decode();
    let held = GROWTH.with(|g| g.borrow_mut().take());
    drop(restore);
    (r, held.unwrap_or_else(specular_core::Reservation::none))
}

/// Appends `len` bytes copied from `dist` bytes back in `out` (an LZ77 match; the regions may
/// overlap, repeating the last `dist` bytes). `dist` must be in `1..=out.len()`.
#[inline]
pub(crate) fn copy_match(out: &mut Vec<u8>, dist: usize, len: usize) {
    let start = out.len() - dist;
    if dist >= len {
        out.extend_from_within(start..start + len);
    } else if dist == 1 {
        let b = out[start];
        out.resize(out.len() + len, b);
    } else {
        // Overlapping: copy what exists, then the doubled pattern, until done.
        let mut left = len;
        while left > 0 {
            let avail = (out.len() - start).min(left);
            out.extend_from_within(start..start + avail);
            left -= avail;
        }
    }
}

/// How many threads a decoder with independent blocks may use, and what each runs first.
#[derive(Clone, Copy)]
pub(crate) struct Threads {
    pub n: usize,
    pub start: Option<fn()>,
}

impl Threads {
    pub(crate) const ONE: Threads = Threads { n: 1, start: None };

    pub(crate) fn of(options: &crate::Options) -> Threads {
        Threads {
            n: options.decode_threads.max(1) as usize,
            start: options.thread_start,
        }
    }

    /// Runs `work(i)` for every `i` in `0..jobs` on up to `self.n` threads (named `name-N`,
    /// the calling thread being one of them), handing out jobs in order.
    pub(crate) fn run<F>(self, name: &'static str, jobs: usize, work: F)
    where
        F: Fn(usize) + Sync,
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let next = AtomicUsize::new(0);
        let worker = || {
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= jobs {
                    break;
                }
                work(i);
            }
        };
        let helpers = self.n.min(jobs).saturating_sub(1);
        if helpers == 0 {
            worker();
            return;
        }
        std::thread::scope(|s| {
            for k in 0..helpers {
                let worker = &worker;
                let start = self.start;
                // A helper that cannot be started leaves its share to the others.
                let _ = specular_thread::builder(format!("{name}-{}", k + 1)).spawn_scoped(
                    s,
                    move || {
                        if let Some(f) = start {
                            f();
                        }
                        worker();
                    },
                );
            }
            worker();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_overlap() {
        let mut v = b"abc".to_vec();
        copy_match(&mut v, 3, 7);
        assert_eq!(v, b"abcabcabca");
        let mut v = b"xy".to_vec();
        copy_match(&mut v, 1, 3);
        assert_eq!(v, b"xyyyy");
        let mut v = b"0123456789".to_vec();
        copy_match(&mut v, 8, 4);
        assert_eq!(v, b"01234567892345");
        assert_eq!(room(&v, 2, 16), Ok(()));
        assert_eq!(room(&v, 3, 16), Err(CodecError::Limit));
    }
}
