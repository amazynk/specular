//! Player behaviour against a fake decoder with a simulated clock.
//!
//! The worker thread is real, but time inside the player is only what the test passes to
//! `tick`, and `wait_idle` lets the worker settle before each step, so results are exact.

use specular_anim::{
    DelayPolicy, FakeFrames, FakeProbe, FakeSpec, LoopMode, Player, PlayerConfig, TickResult,
};
use specular_decode::AnimationInfo;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const SETTLE: Duration = Duration::from_secs(10);

fn ms(v: u64) -> Duration {
    Duration::from_millis(v)
}

fn player(
    spec: FakeSpec,
    count_known: bool,
    loops: u32,
    cfg: PlayerConfig,
) -> (Player, Arc<FakeProbe>) {
    let info = AnimationInfo {
        frame_count: count_known.then_some(spec.count),
        loop_count: loops,
    };
    let (frames, probe) = FakeFrames::new(spec);
    (Player::new(Box::new(frames), &info, cfg, None), probe)
}

fn cfg() -> PlayerConfig {
    PlayerConfig {
        delay_policy: DelayPolicy::AS_STORED,
        ..PlayerConfig::default()
    }
}

/// Tick after letting the worker settle.
fn tick(p: &mut Player, now: Instant) -> TickResult {
    assert!(p.wait_idle(SETTLE), "worker did not settle");
    let r = p.tick(now);
    // A second tick at the same time never changes anything.
    if r.show.is_some() || r.next_deadline.is_some() {
        assert!(p.wait_idle(SETTLE));
    }
    r
}

fn shown(r: &TickResult) -> Option<u32> {
    r.show.as_ref().map(|f| f.index)
}

#[test]
fn plays_on_schedule_and_loops() {
    let (mut p, _) = player(FakeSpec::new(4, ms(40), 8, 8), false, 0, cfg());
    let t0 = Instant::now();
    let r = tick(&mut p, t0);
    assert_eq!(shown(&r), Some(0));
    assert_eq!(r.next_deadline, Some(t0 + ms(40)));
    // Nothing changes before the deadline.
    let r = tick(&mut p, t0 + ms(39));
    assert!(r.show.is_none());
    assert_eq!(r.next_deadline, Some(t0 + ms(40)));
    for (i, at) in [(1, 40), (2, 80), (3, 120), (0, 160), (1, 200)] {
        let r = tick(&mut p, t0 + ms(at));
        assert_eq!(shown(&r), Some(i), "at {at} ms");
        assert_eq!(r.dropped, 0);
        assert_eq!(r.next_deadline, Some(t0 + ms(at + 40)));
    }
    assert_eq!(p.frame_count(), Some(4));
    assert_eq!(p.position().lap, 1);
}

#[test]
fn late_tick_skips_without_drift() {
    let (mut p, _) = player(FakeSpec::new(10, ms(40), 8, 8), true, 0, cfg());
    let t0 = Instant::now();
    let r = tick(&mut p, t0);
    // The first frame starts the schedule.
    assert_eq!(r.due, Some(t0));
    // 130 ms late: frame 3 is due (120..160); frames 1 and 2 are dropped.
    let r = tick(&mut p, t0 + ms(130));
    assert_eq!(shown(&r), Some(3));
    assert_eq!(r.dropped, 2);
    // Its own start, not the missed deadline of frame 1.
    assert_eq!(r.due, Some(t0 + ms(120)));
    // The next deadline is still on the original grid.
    assert_eq!(r.next_deadline, Some(t0 + ms(160)));
    assert_eq!(p.stats().dropped, 2);
}

#[test]
fn browser_delay_clamp() {
    let mut spec = FakeSpec::new(3, ms(0), 8, 8);
    spec.delays = vec![ms(0), ms(10), ms(50)];
    let (mut p, _) = player(
        spec,
        true,
        0,
        PlayerConfig {
            delay_policy: DelayPolicy::BROWSER,
            ..PlayerConfig::default()
        },
    );
    let t0 = Instant::now();
    assert_eq!(tick(&mut p, t0).next_deadline, Some(t0 + ms(100)));
    assert_eq!(tick(&mut p, t0 + ms(100)).next_deadline, Some(t0 + ms(200)));
    assert_eq!(tick(&mut p, t0 + ms(200)).next_deadline, Some(t0 + ms(250)));
}

#[test]
fn finite_loops_stop_on_last_frame() {
    let (mut p, _) = player(FakeSpec::new(3, ms(10), 8, 8), true, 2, cfg());
    let t0 = Instant::now();
    tick(&mut p, t0);
    let r = tick(&mut p, t0 + ms(1000));
    assert_eq!(shown(&r), Some(2));
    assert!(r.next_deadline.is_none());
    assert!(p.position().finished);
    assert!(!p.is_playing());
    // Loop-forever override, then play again from the start.
    p.set_loop_mode(LoopMode::Forever);
    p.play(t0 + ms(2000));
    let r = tick(&mut p, t0 + ms(2000));
    assert_eq!(shown(&r), Some(0));
    assert_eq!(r.next_deadline, Some(t0 + ms(2010)));
    let r = tick(&mut p, t0 + ms(2075));
    assert_eq!(shown(&r), Some(1)); // 7 frames later, forever
    assert!(p.is_playing());
}

#[test]
fn pause_resume_keeps_position_in_frame() {
    let (mut p, _) = player(FakeSpec::new(5, ms(100), 8, 8), true, 0, cfg());
    let t0 = Instant::now();
    tick(&mut p, t0);
    p.pause(t0 + ms(30));
    let r = tick(&mut p, t0 + ms(500));
    assert!(r.show.is_none() && r.next_deadline.is_none());
    p.play(t0 + ms(1000));
    // 70 ms of frame 0 were left.
    assert_eq!(
        tick(&mut p, t0 + ms(1000)).next_deadline,
        Some(t0 + ms(1070))
    );
}

#[test]
fn speed_changes_without_jump() {
    let (mut p, _) = player(FakeSpec::new(5, ms(100), 8, 8), true, 0, cfg());
    let t0 = Instant::now();
    tick(&mut p, t0);
    p.set_speed(2.0, t0 + ms(50));
    // 50 ms of frame 0 left at double speed.
    assert_eq!(tick(&mut p, t0 + ms(50)).next_deadline, Some(t0 + ms(75)));
    assert_eq!(shown(&tick(&mut p, t0 + ms(75))), Some(1));
    p.set_speed(0.25, t0 + ms(75));
    assert_eq!(tick(&mut p, t0 + ms(75)).next_deadline, Some(t0 + ms(475)));
    p.set_speed(100.0, t0 + ms(75));
    assert_eq!(p.speed(), 4.0);
}

#[test]
fn step_and_seek() {
    let (mut p, probe) = player(FakeSpec::new(6, ms(40), 8, 8), true, 0, cfg());
    let t0 = Instant::now();
    tick(&mut p, t0);
    p.step_forward();
    assert!(!p.is_playing());
    let r = tick(&mut p, t0 + ms(1));
    assert_eq!(shown(&r), Some(1));
    assert!(r.next_deadline.is_none());
    p.step_back();
    assert_eq!(shown(&tick(&mut p, t0 + ms(2))), Some(0));
    p.step_back(); // wraps: count is known
    assert_eq!(shown(&tick(&mut p, t0 + ms(3))), Some(5));
    p.seek(2);
    assert_eq!(shown(&tick(&mut p, t0 + ms(4))), Some(2));
    // Small animation: everything was cached, stepping never rewound the decoder.
    assert!(p.position().fully_cached);
    assert_eq!(probe.rewinds.load(Ordering::SeqCst), 0);
    // Seek while playing continues from the target on a fresh schedule.
    p.play(t0 + ms(10));
    p.seek(4);
    let r = tick(&mut p, t0 + ms(10));
    assert_eq!(shown(&r), Some(4));
    assert_eq!(r.next_deadline, Some(t0 + ms(50)));
}

#[test]
fn step_back_while_streaming_replays() {
    // 20 frames of 4 KiB with a budget of 4 frames: streams.
    let spec = FakeSpec::new(20, ms(40), 32, 32);
    let fb = spec.frame_bytes();
    let (mut p, probe) = player(
        spec,
        false,
        0,
        PlayerConfig {
            budget_bytes: 4 * fb,
            max_ahead: 2,
            ..cfg()
        },
    );
    let t0 = Instant::now();
    tick(&mut p, t0);
    for k in 1..=12u64 {
        let r = tick(&mut p, t0 + ms(40 * k));
        assert_eq!(shown(&r), Some(k as u32));
    }
    assert!(p.stats().cached_bytes <= 4 * fb);
    // Frame 11 is still cached (just behind the playhead).
    p.step_back();
    assert_eq!(shown(&tick(&mut p, t0 + ms(1000))), Some(11));
    assert_eq!(probe.rewinds.load(Ordering::SeqCst), 0);
    // Frame 2 was evicted long ago: rewind and replay.
    p.seek(2);
    assert_eq!(shown(&tick(&mut p, t0 + ms(1001))), Some(2));
    assert_eq!(probe.rewinds.load(Ordering::SeqCst), 1);
    assert!(!p.position().fully_cached);
}

#[test]
fn small_animation_cached_after_first_loop() {
    let (mut p, probe) = player(FakeSpec::new(5, ms(20), 16, 16), false, 0, cfg());
    let t0 = Instant::now();
    tick(&mut p, t0);
    for k in 1..=5u64 {
        tick(&mut p, t0 + ms(20 * k));
    }
    assert!(p.wait_idle(SETTLE));
    assert!(p.position().fully_cached);
    let decoded = probe.decoded.load(Ordering::SeqCst);
    assert_eq!(decoded, 5);
    // The decoder is dropped once everything is cached.
    assert!(probe.dropped.load(Ordering::SeqCst));
    // Many more loops: zero decodes.
    for k in 6..=100u64 {
        tick(&mut p, t0 + ms(20 * k));
    }
    assert_eq!(probe.decoded.load(Ordering::SeqCst), decoded);
    assert_eq!(p.position().index, Some(0));
}

#[test]
fn large_animation_streams_within_budget() {
    let spec = FakeSpec::new(50, ms(10), 64, 64);
    let fb = spec.frame_bytes();
    let budget = 8 * fb;
    let (mut p, probe) = player(
        spec,
        true,
        0,
        PlayerConfig {
            budget_bytes: budget,
            max_ahead: 4,
            ..cfg()
        },
    );
    let t0 = Instant::now();
    for k in 0..300u64 {
        let r = tick(&mut p, t0 + ms(10 * k));
        assert_eq!(shown(&r), Some((k % 50) as u32), "tick {k}");
        let st = p.stats();
        assert!(st.cached_bytes <= budget, "{st:?}");
    }
    assert!(!p.position().fully_cached);
    // Each loop decodes every frame once more; 6 loops of 50 plus a little lookahead.
    let decoded = probe.decoded.load(Ordering::SeqCst);
    assert!((300..=300 + 4 + 1).contains(&decoded), "{decoded}");
    assert_eq!(probe.rewinds.load(Ordering::SeqCst), 6);
}

#[test]
fn eager_when_count_known_and_fits() {
    let (p, probe) = player(FakeSpec::new(40, ms(10), 8, 8), true, 0, cfg());
    // Without a single tick the worker decodes everything and ends.
    assert!(p.wait_idle(SETTLE));
    assert!(probe.dropped.load(Ordering::SeqCst));
    assert_eq!(probe.decoded.load(Ordering::SeqCst), 40);
    assert_eq!(p.stats().cached_frames, 40);
}

#[test]
fn ping_pong_once_cached() {
    let (mut p, _) = player(
        FakeSpec::new(4, ms(10), 8, 8),
        true,
        0,
        PlayerConfig {
            ping_pong: true,
            ..cfg()
        },
    );
    let t0 = Instant::now();
    let mut seen = vec![];
    for k in 0..10u64 {
        if let Some(i) = shown(&tick(&mut p, t0 + ms(10 * k))) {
            seen.push(i);
        }
    }
    assert!(p.position().ping_pong);
    assert_eq!(seen, [0, 1, 2, 3, 2, 1, 0, 1, 2, 3]);
}

#[test]
fn slow_decoder_stalls_and_resyncs() {
    let mut spec = FakeSpec::new(30, ms(10), 8, 8);
    spec.decode_cost = ms(5);
    let (mut p, _) = player(
        spec,
        true,
        0,
        PlayerConfig {
            max_ahead: 1,
            budget_bytes: 2 * 8 * 8 * 4,
            max_lag: ms(100),
            ..cfg()
        },
    );
    let t0 = Instant::now();
    assert_eq!(shown(&tick(&mut p, t0)), Some(0));
    // Without waiting for the worker: time jumps far ahead of what is decoded.
    std::thread::sleep(ms(1));
    let mut now = t0 + ms(500);
    let mut last = 0;
    for _ in 0..200 {
        let r = p.tick(now);
        if let Some(f) = &r.show {
            assert!(f.index > last || f.index == 0, "frames go forward");
            last = f.index;
        }
        if last >= 20 {
            break;
        }
        // The simulated clock runs 10x faster than the decoder can deliver.
        std::thread::sleep(ms(2));
        now += ms(20);
    }
    assert!(last >= 20, "playback progressed ({last})");
    let st = p.stats();
    assert!(st.stalls > 0 && st.resyncs > 0, "{st:?}");
}

#[test]
fn unknown_count_discovered() {
    let (mut p, _) = player(FakeSpec::new(3, ms(10), 8, 8), false, 0, cfg());
    assert_eq!(p.frame_count(), None);
    let t0 = Instant::now();
    tick(&mut p, t0);
    tick(&mut p, t0 + ms(10));
    tick(&mut p, t0 + ms(20));
    assert!(p.wait_idle(SETTLE));
    tick(&mut p, t0 + ms(25));
    assert_eq!(p.frame_count(), Some(3));
    assert_eq!(shown(&tick(&mut p, t0 + ms(30))), Some(0));
}

#[test]
fn decode_error_truncates() {
    let mut spec = FakeSpec::new(10, ms(10), 8, 8);
    spec.fail_at = Some(4);
    let (mut p, _) = player(spec, false, 0, cfg());
    let t0 = Instant::now();
    for k in 0..12u64 {
        tick(&mut p, t0 + ms(10 * k));
    }
    assert_eq!(p.frame_count(), Some(4));
    assert_eq!(p.position().index, Some(3)); // 11 % 4

    let mut spec = FakeSpec::new(10, ms(10), 8, 8);
    spec.fail_at = Some(0);
    let (mut p, _) = player(spec, false, 0, cfg());
    let r = tick(&mut p, t0);
    assert!(r.show.is_none() && r.next_deadline.is_none());
    assert!(p.error().is_some());
}

/// A frame refused for memory stops playback with the amount, instead of looping the frames
/// before it as a shorter clip.
#[test]
fn needs_memory_stops_instead_of_truncating() {
    let mut spec = FakeSpec::new(10, ms(10), 8, 8);
    spec.memory_at = Some(4);
    let (mut p, _) = player(spec, false, 0, cfg());
    let t0 = Instant::now();
    for k in 0..12u64 {
        tick(&mut p, t0 + ms(10 * k));
    }
    assert!(p.wait_idle(SETTLE));
    let n = p.needs_memory().expect("needs memory");
    assert_eq!(n.needed, 8 << 30);
    assert!(p.error().is_some());
    // No loop back to frame 0: the clip is not 4 frames long.
    assert_ne!(p.frame_count(), Some(4));
    for k in 12..20u64 {
        assert_ne!(shown(&tick(&mut p, t0 + ms(10 * k))), Some(0));
    }
    // A plain decode error still truncates without a memory note.
    let mut spec = FakeSpec::new(10, ms(10), 8, 8);
    spec.fail_at = Some(4);
    let (p, _) = player(spec, false, 0, cfg());
    assert!(p.needs_memory().is_none());
}

/// A decoder that panics on a frame fails the animation like a decode error: the worker
/// exits, the player reports the error instead of waiting for the frame, and the next
/// player works.
#[test]
fn a_panicking_decoder_fails_the_animation_only() {
    let mut spec = FakeSpec::new(10, ms(10), 8, 8);
    spec.panic_at = Some(2);
    let (mut p, probe) = player(spec, true, 0, cfg());
    let t0 = Instant::now();
    for k in 0..6u64 {
        tick(&mut p, t0 + ms(10 * k));
        assert!(p.wait_idle(SETTLE), "the worker settles after the panic");
    }
    let e = p.error().expect("the panic is the player's error");
    assert!(e.starts_with("internal error"), "{e}");
    assert_eq!(probe.decoded.load(Ordering::SeqCst), 2);

    let (mut p, _) = player(FakeSpec::new(3, ms(10), 8, 8), true, 0, cfg());
    assert_eq!(shown(&tick(&mut p, t0)), Some(0));
    assert!(p.error().is_none());
}

#[test]
fn drop_cancels_running_decode() {
    let mut spec = FakeSpec::new(10, ms(10), 8, 8);
    spec.decode_cost = Duration::from_secs(30);
    let (p, probe) = player(spec, true, 0, cfg());
    std::thread::sleep(ms(20)); // the worker is inside next_frame
    let start = Instant::now();
    drop(p); // must not block on the decode
    assert!(start.elapsed() < ms(100));
    while !probe.dropped.load(Ordering::SeqCst) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "worker did not stop"
        );
        std::thread::sleep(ms(1));
    }
    assert_eq!(probe.decoded.load(Ordering::SeqCst), 0);
}

#[test]
fn shutdown_joins() {
    let (p, probe) = player(FakeSpec::new(1000, ms(10), 8, 8), false, 0, cfg());
    p.shutdown();
    assert!(probe.dropped.load(Ordering::SeqCst));
}

#[test]
fn no_lock_held_during_decode() {
    let mut spec = FakeSpec::new(10, ms(10), 8, 8);
    spec.decode_cost = ms(300);
    let (mut p, _) = player(spec, true, 0, cfg());
    std::thread::sleep(ms(20));
    let start = Instant::now();
    for k in 0..100 {
        p.tick(start + ms(k));
        p.position();
        p.stats();
    }
    assert!(
        start.elapsed() < ms(100),
        "ticks blocked: {:?}",
        start.elapsed()
    );
}

#[test]
fn waker_fires_when_frame_arrives() {
    let mut spec = FakeSpec::new(3, ms(10), 8, 8);
    spec.decode_cost = ms(30);
    let (frames, _) = FakeFrames::new(spec);
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let info = AnimationInfo {
        frame_count: Some(3),
        loop_count: 0,
    };
    let mut p = Player::new(
        Box::new(frames),
        &info,
        cfg(),
        Some(Arc::new(move || {
            let _ = tx.lock().unwrap().send(());
        })),
    );
    let r = p.tick(Instant::now());
    assert!(
        r.show.is_none() && r.next_deadline.is_none(),
        "waits for the waker"
    );
    rx.recv_timeout(Duration::from_secs(5)).expect("waker");
    assert_eq!(shown(&p.tick(Instant::now())), Some(0));
}

#[test]
fn hammer_controls_no_deadlock() {
    let mut spec = FakeSpec::new(37, ms(7), 16, 16);
    spec.decode_cost = Duration::from_micros(200);
    let fb = spec.frame_bytes();
    let (mut p, probe) = player(
        spec,
        false,
        0,
        PlayerConfig {
            budget_bytes: 5 * fb,
            max_ahead: 3,
            ..cfg()
        },
    );
    let start = Instant::now();
    let mut now = start;
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..3000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        now += Duration::from_micros(x % 9000);
        match x % 23 {
            0 => p.step_forward(),
            1 => p.step_back(),
            2 => p.seek((x >> 8) as u32 % 40),
            3 => p.toggle(now),
            4 => p.set_speed(0.25 + (x % 16) as f64 / 4.0, now),
            5 => p.set_ping_pong(x & 256 != 0, now),
            _ => {}
        }
        p.tick(now);
        assert!(p.stats().cached_bytes <= 5 * fb);
        assert!(probe.busy.load(Ordering::SeqCst) <= 1);
    }
    assert!(start.elapsed() < Duration::from_secs(30));
    p.shutdown();
}

/// A movie-like source: every frame time known up front, the decoder seeks itself.
fn movie(count: u32, delay: Duration, budget_frames: usize) -> (Player, Arc<FakeProbe>) {
    let mut spec = FakeSpec::new(count, delay, 4, 4);
    spec.seekable = true;
    let fb = spec.frame_bytes();
    player(
        spec,
        false,
        0,
        PlayerConfig {
            budget_bytes: budget_frames * fb,
            max_ahead: 4,
            loop_mode: LoopMode::Times(1),
            ..cfg()
        },
    )
}

#[test]
fn movie_timeline_is_known_up_front_and_seeks_jump() {
    let (mut p, probe) = movie(3000, ms(40), 8);
    let t0 = Instant::now();
    assert_eq!(shown(&tick(&mut p, t0)), Some(0));
    // Count and duration come from the sample table, not from playing through.
    assert_eq!(p.frame_count(), Some(3000));
    assert_eq!(p.duration(), Some(ms(120_000)));
    assert_eq!(p.index_at(ms(100_000)), Some(2500));
    assert_eq!(p.index_at(ms(10_000_000)), Some(2999));
    p.pause(t0);
    // Far ahead: the decoder seeks there instead of decoding 2500 frames.
    p.seek(2500);
    assert_eq!(p.time(t0), Some(ms(100_000)));
    assert_eq!(shown(&tick(&mut p, t0 + ms(1))), Some(2500));
    assert_eq!(p.time(t0 + ms(1)), Some(ms(100_000)));
    assert!(probe.seeks.load(Ordering::SeqCst) >= 1);
    assert!(probe.decoded.load(Ordering::SeqCst) < 40, "{probe:?}");
    // Back, past the evicted frames: a seek again, never a rewind and replay.
    p.seek(10);
    assert_eq!(shown(&tick(&mut p, t0 + ms(2))), Some(10));
    p.step_back();
    assert_eq!(shown(&tick(&mut p, t0 + ms(3))), Some(9));
    assert_eq!(probe.rewinds.load(Ordering::SeqCst), 0);
    assert!(probe.decoded.load(Ordering::SeqCst) < 80, "{probe:?}");
    // Every seek target is the frame asked for, wherever the decoder was.
    let mut x = 12345u32;
    for k in 0..40 {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let i = (x >> 8) % 3000;
        p.seek(i);
        tick(&mut p, t0 + ms(10 + k));
        assert_eq!(p.current().map(|f| f.index), Some(i));
        assert_eq!(
            p.current().map(|f| f.raster.planes[0].data[0]),
            Some((i % 256) as u8)
        );
    }
}

/// A/V sync: ten minutes of a 25 fps movie against a sound clock that runs 0.1% fast (a
/// typical crystal mismatch, 0.6 s over ten minutes). The picture follows the sound to within
/// a frame the whole way and ends where the sound ends.
#[test]
fn ten_minutes_follow_the_sound_clock_without_drift() {
    let frame = ms(40);
    let count = 15_000; // 600 s
    let (mut p, _) = movie(count, frame, 32);
    let t0 = Instant::now();
    let heard = move |now: Instant| {
        let real = now.saturating_duration_since(t0).as_nanos() as f64;
        Duration::from_nanos((real * 1.001) as u64)
    };
    p.set_clock(Some(Arc::new(move |now| Some(heard(now)))));
    assert_eq!(shown(&tick(&mut p, t0)), Some(0));
    let mut worst = 0i64;
    let mut now = t0;
    let mut last = 0;
    // Tick every 20 ms of wall time up to 599 s (the sound reaches 600 s at 599.4 s).
    for _ in 0..29_950 {
        now += ms(20);
        tick(&mut p, now);
        let i = p.current().map_or(0, |f| f.index);
        assert!(i >= last, "frames go forward");
        last = i;
        let want = (heard(now).as_nanos() / frame.as_nanos()) as i64;
        worst = worst.max((i64::from(i) - want).abs());
    }
    assert!(worst <= 1, "picture {worst} frames off the sound");
    // Without the clock the picture would trail by 0.6 s (15 frames) by now.
    assert!(last >= 14_960, "ended at frame {last}");
    let st = p.stats();
    assert!(st.clock_syncs > 0 && st.resyncs == 0, "{st:?}");
    eprintln!("A/V sync over 599 s, sound 0.1% fast: worst {worst} frame(s), {st:?}");
}

#[test]
fn wall_clock_without_a_running_master() {
    let (mut p, _) = movie(100, ms(40), 8);
    p.set_clock(Some(Arc::new(|_| None)));
    let t0 = Instant::now();
    tick(&mut p, t0);
    for k in 1..=10 {
        assert_eq!(shown(&tick(&mut p, t0 + ms(40 * k))), Some(k as u32));
    }
    assert_eq!(p.stats().clock_syncs, 0);
}

/// An animation's decoder cannot seek: a jump ahead decodes on from where it is (the
/// worker asks once, is declined, and never rewinds for it).
#[test]
fn a_jump_ahead_decodes_on_when_the_decoder_cannot_seek() {
    let spec = FakeSpec::new(40, ms(10), 8, 8);
    let fb = spec.frame_bytes();
    let (mut p, probe) = player(
        spec,
        true,
        0,
        PlayerConfig {
            budget_bytes: 4 * fb,
            max_ahead: 2,
            ..cfg()
        },
    );
    // Play the first loop through, so every frame's time is known.
    let t0 = Instant::now();
    tick(&mut p, t0);
    for k in 1..=41u64 {
        tick(&mut p, t0 + ms(10 * k));
    }
    let now = t0 + ms(415);
    p.pause(now);
    let rewinds = probe.rewinds.load(Ordering::SeqCst);
    p.seek(30);
    assert_eq!(shown(&tick(&mut p, now + ms(1))), Some(30));
    assert_eq!(probe.rewinds.load(Ordering::SeqCst), rewinds);
    assert_eq!(probe.seeks.load(Ordering::SeqCst), 0);
}
