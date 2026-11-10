//! Seek-bar scrubbing. The tests check every frame shown while scrubbing small long-GOP clips
//! (and frames after decodes stopped half-way) against a straight decode, and that letting
//! go lands on the exact frame.
//!
//! The timings drag a pointer steadily along the seek bar of a movie in the animation player
//! the way the app drives it (a move and a tick per display frame), then let go. They report
//! the frames shown per second during the drag, how far the picture trails the pointer, and
//! the time to the exact frame after letting go. Ignored: they need the movies of
//! `tools/corpus/scrub.sh`. Run with
//! `SCRUB_CORPUS=DIR cargo test -p specular-video --test scrub -- --ignored --nocapture`.
//! Each drag runs twice, as plain seeks (how the seek bar drove the player before scrubbing
//! had a mode of its own) and as scrubbing; `SCRUB_MODE=seek` or `scrub` runs one only.
//! `SCRUB_ONLY=TEXT` takes the movies whose names contain it; `SCRUB_HW=1` allows hardware
//! decoding. The scrub strip runs unless `SCRUB_STRIP=0`; each drag starts once it is done
//! (at most `SCRUB_WARM` seconds, default 300; 0: at once).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use specular_anim::{DelayPolicy, LoopMode, Player, PlayerConfig, StripConfig};
use specular_core::{CancelToken, Raster, Source};
use specular_decode::{DecodeError, Format, Frames, Session};

/// The display's frame time (60 Hz): one pointer move and one tick each.
const VSYNC: Duration = Duration::from_micros(16_667);
/// How long each drag lasts.
const DRAG: Duration = Duration::from_secs(2);

fn session(path: &Path) -> specular_video::session::VideoSession {
    specular_video::session::open(Arc::new(Source::open(path).unwrap()), Format::Mp4).unwrap()
}

fn open(path: &Path) -> Player {
    open_with(path, PlayerConfig::default().budget_bytes)
}

/// A player whose frame ring takes `budget` bytes.
fn open_with(path: &Path, budget: usize) -> Player {
    let s = session(path);
    let info = s.animation().cloned().unwrap();
    let frames = s.video_frames().unwrap();
    let cfg = PlayerConfig {
        autoplay: false,
        delay_policy: DelayPolicy::AS_STORED,
        loop_mode: LoopMode::Times(1),
        budget_bytes: budget,
        ..PlayerConfig::default()
    };
    Player::new(Box::new(frames), &info, cfg, None)
}

fn shown(p: &Player) -> Option<u32> {
    p.current().map(|f| f.index)
}

/// Ticks until frame `want` is on screen and nothing is pending; the time it took.
fn settle(p: &mut Player, want: u32, limit: Duration) -> Option<Duration> {
    let t0 = Instant::now();
    loop {
        p.tick(Instant::now());
        if shown(p) == Some(want) && p.position().pending.is_none() {
            return Some(t0.elapsed());
        }
        if t0.elapsed() > limit {
            return None;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn planes(r: &Raster) -> Vec<u8> {
    let mut v = Vec::new();
    for p in &r.planes {
        let bps = p.stride / p.width as usize;
        for row in p.data.chunks(p.stride).take(p.height as usize) {
            v.extend_from_slice(&row[..p.width as usize * bps]);
        }
    }
    v
}

/// Every frame of `path`, decoded straight through.
fn straight(path: &Path) -> Vec<Vec<u8>> {
    let mut f = session(path).video_frames().unwrap();
    let cancel = CancelToken::new();
    let mut out = Vec::new();
    while let Some(r) = f.next_frame(&cancel) {
        out.push(planes(&r.unwrap().raster));
    }
    out
}

const CLIPS: [&str; 3] = ["h264-gop.mp4", "hevc-gop.mp4", "mpeg2-gop.mp4"];

/// A decode stopped between two samples goes on to the same frame afterwards, and the frames
/// after it are the straight decode's.
#[test]
fn an_interrupted_decode_goes_on_to_the_right_frames() {
    specular_hwdec::set_enabled(false);
    for name in CLIPS {
        let want = straight(&data(name));
        let n = want.len() as u32;
        let mut f = session(&data(name)).video_frames().unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        assert!(f.set_interrupt(Arc::clone(&flag)));
        let sync = f.sync_frames().unwrap();
        assert!(sync.len() > 1 && sync[0] == 0, "{name}: {sync:?}");
        let cancel = CancelToken::new();
        let mut stopped = 0;
        for (i, target) in [n * 3 / 4, n / 3, n / 3 + 5, n - 2, 7]
            .into_iter()
            .enumerate()
        {
            f.seek(target).unwrap();
            // Stopped before it starts, then (from another thread) somewhere on the way.
            // (A frame decoded already, waiting to be handed out, needs no decode to stop.)
            flag.store(true, Ordering::SeqCst);
            let first = f.next_frame(&cancel).unwrap();
            flag.store(false, Ordering::SeqCst);
            if let Ok(got) = first {
                assert_eq!(got.index, target, "{name}");
                assert!(
                    planes(&got.raster) == want[target as usize],
                    "{name}: frame {target}"
                );
                continue;
            }
            assert!(matches!(first, Err(DecodeError::Cancelled)));
            let after = Duration::from_micros(300 * i as u64);
            let setter = {
                let flag = Arc::clone(&flag);
                specular_thread::builder("test-interrupt")
                    .spawn(move || {
                        std::thread::sleep(after);
                        flag.store(true, Ordering::SeqCst);
                    })
                    .unwrap()
            };
            let got = loop {
                match f.next_frame(&cancel).unwrap() {
                    Err(DecodeError::Cancelled) => {
                        stopped += 1;
                        flag.store(false, Ordering::SeqCst);
                    }
                    r => break r.unwrap(),
                }
            };
            setter.join().unwrap();
            flag.store(false, Ordering::SeqCst);
            assert_eq!(got.index, target, "{name}");
            assert!(
                planes(&got.raster) == want[target as usize],
                "{name}: frame {target}"
            );
            let next = f.next_frame(&cancel).unwrap().unwrap();
            assert_eq!(next.index, target + 1, "{name}");
            assert!(
                planes(&next.raster) == want[target as usize + 1],
                "{name}: frame {}",
                target + 1
            );
        }
        eprintln!("{name}: {stopped} decodes stopped half-way");
    }
}

/// Scrubbing back and forth across GOPs with a tick per move: every frame shown on the way
/// is the frame it says it is, and letting go shows exactly the frame there.
#[test]
fn scrubbing_shows_true_frames_and_lands_exactly() {
    specular_hwdec::set_enabled(false);
    for name in CLIPS {
        let want = straight(&data(name));
        let n = want.len() as u32;
        let mut p = open(&data(name));
        settle(&mut p, 0, Duration::from_secs(30)).unwrap();
        let path: Vec<u32> = (0..40)
            .map(|i| (i * 7) % n)
            .chain((0..40).map(|i| n - 1 - (i * 3) % n))
            .chain([n / 2, 3, n - 1, n / 3])
            .collect();
        let mut shows = 0;
        for (k, &t) in path.iter().enumerate() {
            p.scrub(t);
            if let Some(f) = p.tick(Instant::now()).show {
                shows += 1;
                assert!(
                    planes(&f.raster) == want[f.index as usize],
                    "{name}: frame {}",
                    f.index
                );
            }
            std::thread::sleep(Duration::from_millis(1 + (k % 3) as u64));
        }
        for land in [n / 3, n - 1, 1] {
            p.scrub(land.saturating_sub(4));
            p.tick(Instant::now());
            p.seek(land);
            settle(&mut p, land, Duration::from_secs(30)).unwrap();
            assert!(!p.position().scrubbing);
            let f = p.current().unwrap();
            assert!(
                planes(&f.raster) == want[land as usize],
                "{name}: landed on {land}"
            );
        }
        eprintln!(
            "{name}: {shows} frames shown while scrubbing, {} decodes stopped",
            p.stats().interrupted
        );
    }
}

/// Every frame, decoded straight through.
fn straight_rasters(path: &Path) -> Vec<Raster> {
    let mut f = session(path).video_frames().unwrap();
    let cancel = CancelToken::new();
    let mut out = Vec::new();
    while let Some(r) = f.next_frame(&cancel) {
        out.push(r.unwrap().raster);
    }
    out
}

/// With the scrub strip: each strip frame is the straight decode's frame made small, a drag
/// shows strip frames and full frames that are what they say, and letting go lands exactly.
#[test]
fn strip_frames_are_true_and_the_drag_lands_exactly() {
    specular_hwdec::set_enabled(false);
    for name in CLIPS {
        let want = straight_rasters(&data(name));
        let n = want.len() as u32;
        // A small frame ring (a few frames): the strip stands in for frames not decoded.
        let mut p = open_with(&data(name), 64 << 10);
        let path = data(name);
        p.start_strip(
            Box::new(move || Ok(Box::new(session(&path).video_frames()?) as Box<dyn Frames>)),
            StripConfig {
                height: 36,
                min_speed: 0.0,
                ..StripConfig::default()
            },
        );
        settle(&mut p, 0, Duration::from_secs(30)).unwrap();
        let t0 = Instant::now();
        while !p.strip_stats().unwrap().done {
            assert!(t0.elapsed() < Duration::from_secs(60), "{name}");
            std::thread::sleep(Duration::from_millis(5));
        }
        let st = p.strip_stats().unwrap();
        assert_eq!(st.frames, st.planned, "{name}: {st:?}");
        assert_eq!((st.width, st.height), (64, 36), "{name}");
        let mut mismatches = 0;
        for i in 0..n {
            if let Some(f) = p.strip_frame(i, 0) {
                let small = specular_anim::strip_shrink(&want[i as usize], 64, 36).unwrap();
                mismatches += usize::from(planes(&f.raster) != planes(&small));
            }
        }
        assert_eq!(mismatches, 0, "{name}");
        let mut strip_shows = 0;
        for k in 0..40u32 {
            let t = (k * 11 + 5) % n;
            p.scrub(t);
            if let Some(f) = p.tick(Instant::now()).show {
                let truth = &want[f.index as usize];
                if f.strip {
                    strip_shows += 1;
                    let small = specular_anim::strip_shrink(truth, 64, 36).unwrap();
                    assert!(planes(&f.raster) == planes(&small), "{name}: {}", f.index);
                } else {
                    assert!(planes(&f.raster) == planes(truth), "{name}: {}", f.index);
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(strip_shows > 0, "{name}");
        for land in [n / 3, n - 1, 1] {
            p.scrub(land.saturating_sub(4));
            p.tick(Instant::now());
            p.seek(land);
            settle(&mut p, land, Duration::from_secs(30)).unwrap();
            let f = p.current().unwrap();
            assert!(!f.strip);
            assert!(
                planes(&f.raster) == planes(&want[land as usize]),
                "{name}: {land}"
            );
        }
        eprintln!(
            "{name}: {} strip frames, 0 mismatches, {strip_shows} shown while dragging",
            st.frames
        );
    }
}

struct Run {
    /// Strip frames held when the drag started, of those planned, and whether frames
    /// between sync frames were decoded.
    strip: Option<(usize, usize, bool)>,
    fps: f64,
    lag: f64,
    exact: Option<Duration>,
    peak_mb: f64,
    stats: specular_anim::Stats,
}

/// Drags from frame `from` to `to` over [`DRAG`], then lets go.
fn drag(path: &Path, from: u32, to: u32, plain_seek: bool) -> Run {
    let mut p = open(path);
    if std::env::var("SCRUB_STRIP").map_or(true, |v| v != "0") {
        let path = path.to_path_buf();
        p.start_strip(
            Box::new(move || Ok(Box::new(session(&path).video_frames()?) as Box<dyn Frames>)),
            StripConfig::default(),
        );
    }
    settle(&mut p, 0, Duration::from_secs(60)).expect("first frame");
    p.seek(from);
    settle(&mut p, from, Duration::from_secs(120)).expect("start frame");
    let warm = std::env::var("SCRUB_WARM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300u64);
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(warm) && p.strip_stats().is_some_and(|s| !s.done) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let strip = p.strip_stats().map(|s| (s.frames, s.planned, s.between));
    let ticks = (DRAG.as_secs_f64() / VSYNC.as_secs_f64()).round() as u32;
    let start = Instant::now();
    let mut shows = 0u32;
    let mut lag = 0f64;
    let mut peak = 0usize;
    for n in 1..=ticks {
        let due = start + VSYNC * n;
        std::thread::sleep(due.saturating_duration_since(Instant::now()));
        let at =
            i64::from(from) + (i64::from(to) - i64::from(from)) * i64::from(n) / i64::from(ticks);
        let at = at as u32;
        if plain_seek {
            p.seek(at);
        } else {
            p.scrub(at);
        }
        if p.tick(Instant::now()).show.is_some() {
            shows += 1;
        }
        lag += f64::from(shown(&p).unwrap_or(from).abs_diff(at));
        peak = peak.max(p.stats().cached_bytes);
    }
    let secs = start.elapsed().as_secs_f64();
    let stats = p.stats();
    p.seek(to);
    let exact = settle(&mut p, to, Duration::from_secs(60));
    peak = peak.max(p.stats().cached_bytes);
    Run {
        strip,
        fps: f64::from(shows) / secs,
        lag: lag / f64::from(ticks),
        exact,
        peak_mb: peak as f64 / (1 << 20) as f64,
        stats,
    }
}

#[test]
#[ignore = "needs tools/corpus/scrub.sh's movies (SCRUB_CORPUS)"]
fn scrub_timings() {
    specular_hwdec::set_enabled(std::env::var_os("SCRUB_HW").is_some());
    let mode = std::env::var("SCRUB_MODE").unwrap_or_default();
    let modes: Vec<bool> = match mode.as_str() {
        "seek" => vec![true],
        "scrub" => vec![false],
        _ => vec![true, false],
    };
    let dir = PathBuf::from(std::env::var_os("SCRUB_CORPUS").expect("SCRUB_CORPUS"));
    let only = std::env::var("SCRUB_ONLY").ok();
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            only.as_ref()
                .is_none_or(|o| p.to_string_lossy().contains(o.as_str()))
        })
        .collect();
    names.sort();
    // (name, from, to): 500 frames, sync samples at 0 and 250 in the long-GOP movies.
    let drags = [
        ("slow-fwd", 20, 120),
        ("fast-fwd", 20, 320),
        ("slow-back", 470, 370),
        ("fast-back", 470, 170),
    ];
    for path in &names {
        for (name, from, to) in drags {
            for &plain_seek in &modes {
                let r = drag(path, from, to, plain_seek);
                println!(
                    "{:<17} {name:<9} {:<5}: {:5.1} frames/s shown ({} from the strip), trails {:5.1} frames, exact frame {} after letting go, ring peak {:.0} MB, strip {}",
                    path.file_name().unwrap().to_string_lossy(),
                    if plain_seek { "seek" } else { "scrub" },
                    r.fps,
                    r.stats.strip_shown,
                    r.lag,
                    r.exact.map_or("never (60 s)".into(), |d| format!(
                        "{:.0} ms",
                        d.as_secs_f64() * 1e3
                    )),
                    r.peak_mb,
                    r.strip.map_or("off".into(), |(n, of, between)| format!(
                        "{n}/{of}{}",
                        if between { "" } else { " (sync frames only)" }
                    )),
                );
                if std::env::var_os("SCRUB_STATS").is_some() {
                    println!("    {:?}", r.stats);
                }
            }
        }
    }
}
