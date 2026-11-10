//! Seek benchmark for MP4 movies (made by tools/corpus/video-plain.sh: 1080p and 4K, H.264 and
//! HEVC, a sync sample every 60 frames).
//!
//! ```text
//! seek_bench time FILE...                wall-clock of every scenario's seeks (each seek is
//!                                        `seek` plus the `next_frame` that shows it)
//! seek_bench run SCENARIO FILE [base]    one scenario; with `base` only its set-up
//! seek_bench count FILE...               samples decoded per seek (and wall-clock), with
//!                                        the seek cache and without, for long GOPs:
//!                                        scrubbing back from frame 240, and jumps
//! ```
//!
//! Scenarios: `forward` (frame 0 shown, then frame 165), `backward` (frame 250 shown, then
//! frame 105), `scrub-back` (frames 200, 197, ... 173: dragging backwards inside one GOP),
//! `scrub-fwd` (frames 140, 143, ... 167). Count instructions on one CPU, where the decoders
//! size their thread pools to one thread and the count is deterministic (to a few hundred
//! instructions): `taskset -c 3 perf stat -e instructions:u` (or callgrind) on
//! `run SCENARIO FILE` and on `run SCENARIO FILE base`; the difference is the seeks.
//! `SEEK_BENCH_HW=1` allows hardware decoding; `SEEK_BENCH_VERBOSE=1` names the decoder.

#![allow(clippy::disallowed_methods)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::{CancelToken, Source};
use specular_decode::{Format, Frames};

/// (name, frames shown before, seek targets).
fn scenarios() -> Vec<(&'static str, u32, Vec<u32>)> {
    vec![
        ("forward", 0, vec![165]),
        ("backward", 250, vec![105]),
        ("scrub-back", 230, (0..10).map(|i| 200 - 3 * i).collect()),
        ("scrub-fwd", 0, (0..10).map(|i| 140 + 3 * i).collect()),
    ]
}

fn open(path: &str) -> (Box<dyn Frames>, CancelToken) {
    let s = specular_video::session::open(
        Arc::new(Source::open(std::path::Path::new(path)).unwrap()),
        Format::Mp4,
    )
    .unwrap();
    let frames = s.video_frames().unwrap();
    if std::env::var_os("SEEK_BENCH_VERBOSE").is_some() {
        eprintln!("{path}: {} decoder", frames.backend());
    }
    let mut frames: Box<dyn Frames> = Box::new(frames);
    let cancel = CancelToken::new();
    frames.next_frame(&cancel).unwrap().unwrap();
    (frames, cancel)
}

/// Shows frame `at` the way the player does (a seek from far away, then the frame).
fn set_up(frames: &mut dyn Frames, cancel: &CancelToken, at: u32) {
    if at > 0 {
        frames.seek(at).unwrap();
        frames.next_frame(cancel).unwrap().unwrap();
    }
}

fn seek(frames: &mut dyn Frames, cancel: &CancelToken, to: u32) -> u32 {
    frames.seek(to).unwrap();
    frames.next_frame(cancel).unwrap().unwrap().index
}

/// Samples decoded and milliseconds per seek of `targets`, after playing to `play_to`.
fn count(path: &str, targets: &[u32], play_to: u32, cache: bool) -> (Vec<u64>, f64) {
    let s = specular_video::session::open(
        Arc::new(Source::open(std::path::Path::new(path)).unwrap()),
        Format::Mp4,
    )
    .unwrap();
    let mut f = s.video_frames().unwrap();
    f.set_seek_cache(cache);
    let cancel = CancelToken::new();
    for _ in 0..=play_to {
        f.next_frame(&cancel).unwrap().unwrap();
    }
    let t = Instant::now();
    let per = targets
        .iter()
        .map(|&to| {
            let before = f.samples_decoded();
            f.seek(to).unwrap();
            assert_eq!(f.next_frame(&cancel).unwrap().unwrap().index, to);
            f.samples_decoded() - before
        })
        .collect();
    (per, t.elapsed().as_secs_f64() * 1e3 / targets.len() as f64)
}

fn main() {
    // `SEEK_BENCH_HW=1`: allow hardware decoding (VA-API, D3D12), as the app does by default.
    specular_hwdec::set_enabled(std::env::var_os("SEEK_BENCH_HW").is_some());
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("time") => {
            for path in &args[1..] {
                for (name, at, targets) in scenarios() {
                    let (mut frames, cancel) = open(path);
                    set_up(frames.as_mut(), &cancel, at);
                    let mut t: Vec<Duration> = Vec::new();
                    for &to in &targets {
                        let s = Instant::now();
                        let got = seek(frames.as_mut(), &cancel, to);
                        t.push(s.elapsed());
                        assert_eq!(got, to, "{name}: landed on {got}, not {to}");
                    }
                    let total: Duration = t.iter().sum();
                    let worst = t.iter().max().unwrap();
                    println!(
                        "{} {name}: {} seeks, total {:.1} ms, mean {:.1} ms, worst {:.1} ms",
                        std::path::Path::new(path)
                            .file_name()
                            .unwrap()
                            .to_string_lossy(),
                        t.len(),
                        total.as_secs_f64() * 1e3,
                        total.as_secs_f64() * 1e3 / t.len() as f64,
                        worst.as_secs_f64() * 1e3,
                    );
                }
            }
        }
        Some("run") if args.len() >= 3 => {
            let (_, at, targets) = scenarios()
                .into_iter()
                .find(|s| s.0 == args[1])
                .expect("unknown scenario");
            let (mut frames, cancel) = open(&args[2]);
            set_up(frames.as_mut(), &cancel, at);
            if args.get(3).map(String::as_str) != Some("base") {
                for to in targets {
                    assert_eq!(seek(frames.as_mut(), &cancel, to), to);
                }
            }
        }
        Some("count") => {
            let sets: [(&str, u32, Vec<u32>); 2] = [
                ("scrub-back", 245, (0..10).map(|i| 240 - 7 * i).collect()),
                ("jumps", 245, vec![200, 120, 230, 60, 180, 100, 220]),
            ];
            for path in &args[1..] {
                let name = std::path::Path::new(path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy();
                for (set, play_to, targets) in &sets {
                    for cache in [false, true] {
                        let (per, ms) = count(path, targets, *play_to, cache);
                        let total: u64 = per.iter().sum();
                        println!(
                            "{name} {set} cache {}: samples per seek {per:?} (mean {:.1}), {ms:.1} ms per seek",
                            if cache { "on " } else { "off" },
                            total as f64 / per.len() as f64,
                        );
                    }
                }
            }
        }
        _ => {
            eprintln!("usage: seek_bench time FILE... | run SCENARIO FILE [base] | count FILE...");
            std::process::exit(2);
        }
    }
}
