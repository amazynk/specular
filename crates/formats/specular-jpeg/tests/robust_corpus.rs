//! Decode-all check over the generated robustness corpus (`tools/corpus/out/robust`, written by
//! `tools/corpus/robust-corpus.sh`): every entry point at all four scales, with regions and an
//! index; zero panics, no file over the time limit, no hang, bounded peak RSS. Skipped when the
//! corpus has not been generated, unless `SPECULAR_ROBUST_CORPUS` names the corpus directory:
//! then a missing or empty corpus fails (CI sets it).
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

#[path = "common/robust_check.rs"]
mod robust_check;

use std::path::{Path, PathBuf};
use std::time::Duration;

#[test]
fn robust_corpus_decodes_without_panics() {
    let required = std::env::var_os("SPECULAR_ROBUST_CORPUS").filter(|v| !v.is_empty());
    let dir = match &required {
        Some(d) => PathBuf::from(d),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/out/robust"),
    };
    let files = robust_check::list(&dir);
    if files.is_empty() {
        assert!(
            required.is_none(),
            "SPECULAR_ROBUST_CORPUS is set but {} holds no corpus",
            dir.display()
        );
        eprintln!("skipping: no corpus in {}", dir.display());
        return;
    }
    // Debug builds are ~10x slower than release; the limits are for runaway work, not speed.
    let limit = if cfg!(debug_assertions) {
        30_000
    } else {
        3_000
    };
    let rep = robust_check::check_all(
        &files,
        std::thread::available_parallelism().map_or(4, |n| n.get().min(8)),
        Duration::from_millis(limit),
        Duration::from_millis(limit * 10),
    );
    eprintln!("{}", rep.summary());
    assert!(!rep.failed(2 << 30), "{}", rep.summary());
}

/// Concurrent decodes sharing one pool must not deadlock. The progressive pipeline used to
/// block a pool thread per decode while its stage threads waited for pool jobs; with as many
/// concurrent decodes as pool threads, nothing could run any more.
#[test]
fn concurrent_progressive_decodes_share_a_pool() {
    use specular_jpeg::*;
    let (w, h) = (97u16, 61u16);
    let px: Vec<u8> = (0..w as usize * h as usize * 3)
        .map(|i| ((i * 7) ^ (i / 577)) as u8)
        .collect();
    let mut jpg = Vec::new();
    let mut e = jpeg_encoder::Encoder::new(&mut jpg, 90);
    e.set_progressive(true);
    e.set_sampling_factor(jpeg_encoder::SamplingFactor::F_4_1);
    e.encode(&px, w, h, jpeg_encoder::ColorType::Rgb).unwrap();
    // Truncated: the stage threads then wait for bands that only pool jobs produce.
    jpg.truncate(jpg.len() * 3 / 5);
    let jpg: &'static [u8] = Box::leak(jpg.into_boxed_slice());
    let want = decode(jpg, &DecodeOptions::default()).unwrap();
    let want: &'static DecodedPlanes = Box::leak(Box::new(want));
    let handles: Vec<_> = (0..16)
        .map(|_| {
            std::thread::spawn(move || {
                for _ in 0..10 {
                    let o = DecodeOptions {
                        parallelism: Parallelism::Threads(3),
                        ..Default::default()
                    };
                    assert!(decode(jpg, &o).unwrap() == *want);
                }
            })
        })
        .collect();
    let t0 = std::time::Instant::now();
    while !handles.iter().all(|h| h.is_finished()) {
        assert!(
            t0.elapsed() < Duration::from_secs(120),
            "concurrent decodes hang"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    for h in handles {
        h.join().unwrap();
    }
}
