//! The advisor for the gallery's thumbnails (DESIGN.md §12.7): each thumbnail the engine made
//! from its file becomes a report like an open's, in a history of its own, so the same
//! patterns (slow storage, archives, decoding in software) are found for thumbnails and the
//! gallery shows the "Why is this slow?" indicator for them.
//!
//! A thumbnail counts as one the user waited for from [`SLOW_MS`], less than an open: a
//! gallery makes many, and each holds up its own tile. The history lasts the session only
//! (it is not kept in the advisor's file): thumbnails come back from the cache next time.

use specular_thumbs::Timing;

use super::advisor::{self, Advice, History};
use super::{DecodeStage, Report, Timeline, path_facts, store, verdict};

/// From how long (ms of work on its worker) a thumbnail counts as slow.
pub const SLOW_MS: f64 = 150.0;

/// Bytes the kernel reads per major fault of a memory-mapped file (its readahead window): a
/// thumbnail often reads only part of the file, such as an embedded preview.
const READAHEAD: u64 = 128 * 1024;

static HISTORY: parking_lot::Mutex<History> = parking_lot::Mutex::new(History::with_slow(SLOW_MS));

/// The report of one generated thumbnail (`in_archive`: a member of an archive; `probed`:
/// whether the hardware decoders were looked for).
pub fn timeline(t: &Timing, in_archive: bool, probed: bool) -> Timeline {
    let total = t.open_ms + t.parse_ms + t.decode_ms;
    // Waiting for pages counts as reading: in the decode first (as for opens), the rest of it
    // while the headers were read.
    let io = t.io_ms.max(0.0);
    let in_parse = (io - t.decode_ms).clamp(0.0, t.parse_ms);
    Timeline {
        format: store::intern(&t.format),
        bytes: if t.faults > 0 {
            t.bytes.min(t.faults.saturating_mul(READAHEAD))
        } else {
            t.bytes
        },
        in_archive,
        read_ms: t.open_ms + in_parse,
        parse_ms: t.parse_ms - in_parse,
        decode: Some(DecodeStage {
            scale: 1,
            ms: t.decode_ms,
            io_ms: io.min(t.decode_ms),
            faults: t.faults,
            from_cache: false,
            path: t.note.map(|n| path_facts(n, probed)),
        }),
        sharp_ms: total,
        ..Timeline::default()
    }
}

/// Adds a generated thumbnail's report to the thumbnails' history (logging advice that is
/// new).
pub fn remember(t: &Timeline) {
    let v = verdict(t);
    let new = HISTORY.lock().push(*t, v);
    for a in new {
        specular_log::debug!(target: specular_log::CACHE, "thumbnails: {}", advisor::log_line(&a));
    }
}

/// The advice about thumbnails now, and the newest thumbnail report and slow one.
pub fn advice() -> (Vec<Advice>, Option<Report>, Option<Report>) {
    let h = HISTORY.lock();
    (h.advise(), h.last().copied(), h.last_slow().copied())
}

#[cfg(test)]
mod tests {
    use super::super::Stage;
    use super::super::advisor::Pattern;
    use super::*;

    fn timing(open_ms: f64, decode_ms: f64) -> Timing {
        Timing {
            format: "JPEG".into(),
            bytes: 8_000_000,
            open_ms,
            parse_ms: 2.0,
            decode_ms,
            ..Timing::default()
        }
    }

    #[test]
    fn a_thumbnail_becomes_a_report_with_reading_split_out() {
        // 60 ms of 80 in the decode waited for pages: reading holds it up.
        let t = Timing {
            io_ms: 60.0,
            faults: 10,
            ..timing(100.0, 80.0)
        };
        let tl = timeline(&t, false, false);
        assert_eq!(tl.format, "JPEG");
        assert_eq!(tl.bytes, 10 * READAHEAD);
        let st = tl.stages();
        assert_eq!(st[1], (Stage::Read, 160.0));
        assert_eq!(st[4], (Stage::Decode, 20.0));
        assert_eq!(tl.sharp_ms, 182.0);
        let v = verdict(&tl);
        assert_eq!(v.bottleneck.map(|b| b.0), Some(Stage::Read));
        // More waiting than the decode took: the rest was while reading the headers.
        let t = Timing {
            io_ms: 81.0,
            faults: 10,
            ..timing(1.0, 80.0)
        };
        let tl = timeline(&t, true, false);
        assert!(tl.in_archive);
        assert_eq!(tl.read_ms, 2.0);
        assert_eq!(tl.parse_ms, 1.0);
        // Without page faults the whole file was read (or the read was simulated).
        assert_eq!(timeline(&timing(1.0, 1.0), false, false).bytes, 8_000_000);
    }

    #[test]
    fn slow_storage_shows_in_thumbnails_below_the_opens_threshold() {
        // Thumbnails of 200 ms, mostly reading at 40 MB/s: too fast for the opens' advisor,
        // a pattern for the thumbnails'.
        let reports: Vec<_> = (0..6)
            .map(|_| {
                let tl = timeline(&timing(190.0, 8.0), false, false);
                (tl, verdict(&tl))
            })
            .collect();
        assert!(
            reports
                .iter()
                .all(|(_, v)| v.total_ms < super::super::SLOW_MS)
        );
        assert!(advisor::advise(&reports).is_empty());
        let mut h = History::with_slow(SLOW_MS);
        for (t, v) in &reports {
            h.push(*t, *v);
        }
        let a = h.advise();
        assert_eq!(a.len(), 1, "{a:?}");
        assert!(matches!(
            a[0].pattern,
            Pattern::SlowStorage { archive: false, .. }
        ));
        assert!(a[0].marked, "{a:?}");
        assert!(h.last_slow().is_some());
        // Fast thumbnails are not waited for.
        let mut fast = History::with_slow(SLOW_MS);
        for _ in 0..8 {
            let tl = timeline(&timing(20.0, 30.0), false, false);
            fast.push(tl, verdict(&tl));
        }
        assert!(fast.advise().is_empty() && fast.last_slow().is_none());
    }
}
