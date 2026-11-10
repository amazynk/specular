//! Seeks in software HEVC, H.264 and MPEG-2 against a straight decode of the same clip: the
//! frame shown after every seek, and the frames decoded on after it, equal the straight
//! decode's. The patterns cover seeks that resume from a cached decoder snapshot (scrubbing
//! back inside a GOP), pictures skipped on the way to a target, leading pictures of open GOPs
//! and jumps across GOPs. Clips: tests/data/{hevc,h264,mpeg2}-gop.mp4, plus every HEVC .mp4
//! in `$SPECULAR_SEEK_CLIPS` (e.g. tools/corpus/out/video-plain; slow for 4K).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{Format, Frames};
use specular_video::session;

fn planes(r: &Raster) -> Vec<u8> {
    assert!(matches!(r.layout, PixelLayout::Yuv(_)), "{:?}", r.layout);
    let mut v = Vec::new();
    for p in &r.planes {
        let bps = p.stride / p.width as usize;
        for row in p.data.chunks(p.stride).take(p.height as usize) {
            v.extend_from_slice(&row[..p.width as usize * bps]);
        }
    }
    v
}

fn frames(path: &Path) -> specular_video::play::VideoFrames {
    let s = session::open(Arc::new(Source::open(path).unwrap()), Format::Mp4).unwrap();
    s.video_frames().unwrap()
}

fn clips() -> Vec<PathBuf> {
    let mut v = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/hevc-gop.mp4")];
    if let Some(dir) = std::env::var_os("SPECULAR_SEEK_CLIPS") {
        let mut more: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|e| e == "mp4")
                    && p.file_name()
                        .is_some_and(|n| n.to_string_lossy().contains("hevc"))
            })
            .collect();
        more.sort();
        v.extend(more);
    }
    v
}

#[test]
fn hevc_seeks_match_straight_decode() {
    for path in clips() {
        seeks_match_straight_decode(&path);
    }
}

#[test]
fn h264_seeks_match_straight_decode() {
    specular_hwdec::set_enabled(false);
    seeks_match_straight_decode(&data("h264-gop.mp4"));
}

/// H.264 with open GOPs in an MP4 whose `stss` lists only the IDR picture (as ISO/IEC
/// 14496-15 has it; GPAC writes such files): seeks past the second GOP's recovery-point I
/// picture start there, found from its SEI, not at the IDR picture, and still give the
/// straight decode's frames.
#[test]
fn h264_seeks_start_at_recovery_points() {
    specular_hwdec::set_enabled(false);
    let cancel = CancelToken::new();
    let mut bytes = std::fs::read(data("h264-gop.mp4")).unwrap();
    let at = bytes.windows(4).position(|w| w == b"stss").unwrap();
    // entry_count 2 → 1: only sample 1 (the IDR picture) stays a sync sample.
    assert_eq!(bytes[at + 8..at + 12], [0, 0, 0, 2]);
    bytes[at + 11] = 1;
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("idr-only-stss");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("h264-gop-idr-sync.mp4");
    std::fs::write(&path, &bytes).unwrap();
    let mut straight = frames(&path);
    let mut want = Vec::new();
    while let Some(f) = straight.next_frame(&cancel) {
        want.push(planes(&f.unwrap().raster));
    }
    let n = want.len() as u32;
    assert_eq!(n, 120);
    for t in [100, 60, 119, 75, 10, 90] {
        let mut f = frames(&path);
        assert!(f.seek(t).unwrap());
        let got = f.next_frame(&cancel).unwrap().unwrap();
        assert_eq!(got.index, t);
        assert!(planes(&got.raster) == want[t as usize], "frame {t}");
        for i in t + 1..(t + 6).min(n) {
            let g = f.next_frame(&cancel).unwrap().unwrap();
            assert_eq!(g.index, i);
            assert!(planes(&g.raster) == want[i as usize], "frame {i} after {t}");
        }
        // From the recovery point (decoding position 60; then the reordering delay and the
        // frames after), not from the IDR picture.
        if t >= 60 {
            assert!(
                f.samples_decoded() <= u64::from(t - 60 + 20),
                "seek to {t} decoded {} samples",
                f.samples_decoded()
            );
        }
    }
}

/// H.264 with B-frames in an MP4 without `ctts`, opened while its file is still being
/// fetched (a stream's web file): the picture order counts cannot be read up front, so the
/// pictures take the samples' times as slots in the order they come out. Played straight and
/// after seeks, the frames equal those of the same clip with its `ctts`.
#[test]
fn h264_without_ctts_while_filling_plays_in_presentation_order() {
    use specular_core::fill;
    use std::time::Duration;
    specular_hwdec::set_enabled(false);
    let cancel = CancelToken::new();
    let src = data("h264-gop.mp4");
    let mut straight = frames(&src);
    let mut want = Vec::new();
    while let Some(f) = straight.next_frame(&cancel) {
        want.push(planes(&f.unwrap().raster));
    }
    let n = want.len() as u32;
    let mut bare = std::fs::read(&src).unwrap();
    let at = bare.windows(4).position(|w| w == b"ctts").unwrap();
    bare[at..at + 4].copy_from_slice(b"free");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("filling-ctts");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("h264-without-ctts.mp4");
    std::fs::write(&path, &bare).unwrap();
    // Everything there but a byte inside the renamed box: the file stays "being fetched".
    let len = bare.len() as u64;
    let filling = fill::Filling::new(len, Duration::from_secs(5));
    filling.add(0, at as u64 + 12);
    filling.add(at as u64 + 13, len);
    let _registered = fill::register(&path, &filling);

    let src = Arc::new(Source::open(&path).unwrap());
    let m = specular_video::parse_movie(src.bytes(), true).unwrap();
    assert!(
        m.video.as_ref().unwrap().slot_timing,
        "timed by slots while filling"
    );
    let s = session::open(src, Format::Mp4).unwrap();
    let mut f = s.video_frames().unwrap();
    let mut k = 0usize;
    while let Some(g) = f.next_frame(&cancel) {
        let g = g.unwrap();
        assert_eq!(g.index as usize, k);
        assert!(
            planes(&g.raster) == want[k],
            "frame {k} is not the one presented there"
        );
        k += 1;
    }
    assert_eq!(k, want.len());
    for t in [n - 1, n / 2, 3, n / 3 + 7, 0, n * 3 / 4] {
        assert!(f.seek(t).unwrap());
        let got = f.next_frame(&cancel).unwrap().unwrap();
        assert_eq!(got.index, t);
        assert!(
            planes(&got.raster) == want[t as usize],
            "frame {t} after a seek"
        );
        for i in t + 1..(t + 4).min(n) {
            let g = f.next_frame(&cancel).unwrap().unwrap();
            assert!(
                planes(&g.raster) == want[i as usize],
                "frame {i} after seeking to {t}"
            );
        }
    }
    assert!(!filling.complete(), "the file stayed in filling");
}

#[test]
fn mpeg2_seeks_match_straight_decode() {
    seeks_match_straight_decode(&data("mpeg2-gop.mp4"));
}

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn seeks_match_straight_decode(path: &Path) {
    let cancel = CancelToken::new();
    {
        let path = path.to_path_buf();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let mut straight = frames(&path);
        assert_eq!(straight.backend(), "software", "{name}");
        let mut want = Vec::new();
        while let Some(f) = straight.next_frame(&cancel) {
            let f = f.unwrap();
            assert_eq!(
                f.index as usize,
                want.len(),
                "{name}: straight decode out of order"
            );
            want.push(planes(&f.raster));
        }
        let n = want.len() as u32;
        assert!(n > 40, "{name}: {n} frames");
        // Scrubbing back through the clip, forward with gaps, then jumps.
        let mut targets: Vec<u32> = (0..n).rev().step_by(3).collect();
        targets.extend((n / 4..n).step_by(2));
        let mut x = 11u32;
        targets.extend((0..60).map(|_| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (x >> 16) % n
        }));
        let mut f = frames(&path);
        for (k, &t) in targets.iter().enumerate() {
            assert!(f.seek(t).unwrap());
            let got = f.next_frame(&cancel).unwrap().unwrap();
            assert_eq!(got.index, t, "{name}: seek {k} landed elsewhere");
            assert!(
                planes(&got.raster) == want[t as usize],
                "{name}: frame {t} after seek {k} differs from the straight decode"
            );
            if k % 4 == 0 {
                // Decoding on after a seek continues in order, nothing left out.
                for i in t + 1..(t + 6).min(n) {
                    let g = f.next_frame(&cancel).unwrap().unwrap();
                    assert_eq!(g.index, i, "{name}: after seek {k}");
                    assert!(
                        planes(&g.raster) == want[i as usize],
                        "{name}: frame {i} after seek {k} (to {t}) differs"
                    );
                }
            }
        }
        eprintln!(
            "{name}: {} seeks, {} resumed from a snapshot",
            targets.len(),
            f.seeks_resumed()
        );
        assert!(
            f.seeks_resumed() > 0,
            "{name}: the seek cache was never used"
        );
    }
}

/// The seek cache's snapshots share the decoder's pictures, and each picture holds its own
/// reservation: the budget sees every picture once, not once more for the snapshots holding it.
#[test]
fn seek_snapshots_count_once_in_the_budget() {
    use specular_core::budget::{self, MemoryBudget};
    let cancel = CancelToken::new();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/hevc-gop.mp4");
    let b = Arc::new(MemoryBudget::fixed(8 << 30));
    budget::with_budget(b.clone(), || {
        let mut f = frames(&path);
        let mut n = 0u32;
        while f.next_frame(&cancel).is_some() {
            n += 1;
        }
        let mut most = 0;
        for t in (0..n).rev().step_by(3) {
            assert!(f.seek(t).unwrap());
            let got = f.next_frame(&cancel).unwrap().unwrap();
            assert_eq!(got.index, t);
            drop(got);
            let held = f.held_picture_bytes();
            let reserved = b.reserved();
            most = most.max(held);
            assert!(
                reserved <= held,
                "seek to {t}: {reserved} bytes reserved for {held} bytes of pictures"
            );
        }
        assert!(f.seeks_resumed() > 0, "the seek cache was never used");
        assert!(most > 0);
    });
}

/// Samples decoded by each seek of `targets` after playing `path` through once, with the
/// seek cache on or off.
fn decoded_per_seek(path: &Path, targets: &[u32], cache: bool) -> Vec<u64> {
    let cancel = CancelToken::new();
    let mut f = frames(path);
    f.set_seek_cache(cache);
    while f.next_frame(&cancel).is_some() {}
    targets
        .iter()
        .map(|&t| {
            let before = f.samples_decoded();
            assert!(f.seek(t).unwrap());
            assert_eq!(f.next_frame(&cancel).unwrap().unwrap().index, t);
            f.samples_decoded() - before
        })
        .collect()
}

/// Scrubbing back through the end of a long GOP decodes a few samples per seek, not the GOP
/// up to the target: the seeks resume from snapshots taken on the first pass.
#[test]
fn long_gop_seeks_decode_a_few_samples() {
    specular_hwdec::set_enabled(false);
    // (clip, the last target): GOPs of 60 (frames 60..119), and of 32 for HEVC (32..63).
    for (name, top) in [
        ("h264-gop.mp4", 115),
        ("mpeg2-gop.mp4", 115),
        ("hevc-gop.mp4", 62),
    ] {
        let path = data(name);
        let back: Vec<u32> = (0..10).map(|i| top - 3 * i).collect();
        let with = decoded_per_seek(&path, &back, true);
        let without = decoded_per_seek(&path, &back, false);
        let sum = |v: &[u64]| v.iter().sum::<u64>();
        eprintln!(
            "{name}: samples decoded per seek {with:?} (total {}), without the cache {without:?} (total {})",
            sum(&with),
            sum(&without)
        );
        // A snapshot every 4 samples, plus up to a mini-GOP of B pictures decoded ahead.
        assert!(*with.iter().max().unwrap() <= 16, "{name}: {with:?}");
        // Far into a GOP of 60 a seek costs a fifth of what it did; GOPs of 32 gain less.
        // H.264 without the cache already leaves out the unreferenced B pictures on the way
        // (about a third of the samples), so the cache gains less over it.
        let gain = if name.starts_with("mpeg2") { 8 } else { 3 };
        assert!(
            sum(&with) * gain < sum(&without) * 2,
            "{name}: {with:?} vs {without:?}"
        );
    }
}

/// H.264 and MPEG-2 reserve their picture buffers as one block: the seek cache reserves what
/// its snapshots hold besides, and gives it back as they go.
#[test]
fn h264_and_mpeg2_snapshots_are_reserved() {
    use specular_core::budget::{self, MemoryBudget};
    specular_hwdec::set_enabled(false);
    let cancel = CancelToken::new();
    for name in ["h264-gop.mp4", "mpeg2-gop.mp4"] {
        let b = Arc::new(MemoryBudget::fixed(8 << 30));
        budget::with_budget(b.clone(), || {
            let mut f = frames(&data(name));
            while f.next_frame(&cancel).is_some() {}
            let held = f.seek_cache_reserved();
            assert!(held > 0, "{name}: nothing reserved");
            assert!(b.reserved() >= held, "{name}");
            drop(f);
            assert_eq!(b.reserved(), 0, "{name}: reservations left after the clip");
        });
    }
}
