//! Scrubbing (dragging the seek bar) against a fake long-GOP decoder: the drag path, stopping
//! decodes the pointer left behind, the frame ring's bound and the exact frame on release.

use specular_anim::{DelayPolicy, FakeFrames, FakeProbe, FakeSpec, LoopMode, Player, PlayerConfig};
use specular_decode::AnimationInfo;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(20);

/// A 400-frame movie at 25 fps with a sync frame every `gop` frames, `cost` a frame.
fn movie(gop: u32, cost: Duration, interruptible: bool, budget: usize) -> (Player, Arc<FakeProbe>) {
    let mut spec = FakeSpec::new(400, Duration::from_millis(40), 16, 16);
    spec.seekable = true;
    spec.gop = Some(gop);
    spec.decode_cost = cost;
    spec.interruptible = interruptible;
    let info = AnimationInfo {
        frame_count: Some(400),
        loop_count: 1,
    };
    let cfg = PlayerConfig {
        autoplay: false,
        delay_policy: DelayPolicy::AS_STORED,
        loop_mode: LoopMode::Times(1),
        budget_bytes: budget,
        max_ahead: 2,
        ..PlayerConfig::default()
    };
    let (frames, probe) = FakeFrames::new(spec);
    (Player::new(Box::new(frames), &info, cfg, None), probe)
}

fn shown(p: &Player) -> Option<u32> {
    p.current().map(|f| f.index)
}

/// Ticks until frame `want` is on screen with nothing pending; every frame shown on the way.
fn settle(p: &mut Player, want: u32) -> Vec<u32> {
    let t0 = Instant::now();
    let mut seen = Vec::new();
    loop {
        if let Some(f) = p.tick(Instant::now()).show {
            seen.push(f.index);
        }
        if shown(p) == Some(want) && p.position().pending.is_none() {
            return seen;
        }
        assert!(
            t0.elapsed() < LIMIT,
            "frame {want} never showed; saw {seen:?}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn started(gop: u32, cost: Duration, interruptible: bool) -> (Player, Arc<FakeProbe>) {
    let (mut p, probe) = movie(gop, cost, interruptible, 64 << 20);
    settle(&mut p, 0);
    (p, probe)
}

#[test]
fn a_far_drag_ahead_eases_toward_the_pointer() {
    let (mut p, _) = started(400, Duration::from_millis(2), true);
    p.scrub(200);
    let seen = settle(&mut p, 200);
    // Steps on the way, each nearer the pointer than the one before.
    assert!(seen.len() >= 4, "{seen:?}");
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "{seen:?}");
    assert_eq!(seen.last(), Some(&200));
    assert!(p.stats().interim >= 3);
}

#[test]
fn a_jump_into_another_gop_shows_its_sync_frame_first() {
    let (mut p, _) = started(100, Duration::from_millis(3), true);
    p.scrub(290);
    let seen = settle(&mut p, 290);
    // (Frames decoded ahead of frame 0 may show first: they are nearer than frame 0.)
    assert_eq!(seen.iter().find(|&&f| f >= 100), Some(&200), "{seen:?}");
    assert_eq!(seen.last(), Some(&290));
}

#[test]
fn a_short_drag_back_lands_on_the_frame_without_the_sync_frame() {
    // The frame on screen is nearer than the sync frame: no detour through it.
    let (mut p, _) = started(100, Duration::from_millis(1), true);
    p.seek(190);
    settle(&mut p, 190);
    p.scrub(185);
    let seen = settle(&mut p, 185);
    assert!(!seen.contains(&100), "{seen:?}");
}

#[test]
fn moving_on_stops_a_decode_no_longer_wanted() {
    // 15 ms a frame: reaching frame 90 from its sync frame takes over a second.
    let (mut p, probe) = started(100, Duration::from_millis(15), true);
    p.scrub(390);
    p.tick(Instant::now());
    // Let the decoder get going toward 390, then move the pointer back into GOP 1.
    std::thread::sleep(Duration::from_millis(60));
    p.scrub(110);
    p.tick(Instant::now());
    let t0 = Instant::now();
    settle(&mut p, 110);
    assert!(probe.interrupted.load(Ordering::SeqCst) >= 1);
    assert!(p.stats().interrupted >= 1);
    // Ten frames from sync frame 100 (and maybe the end of one more), not 90 more of GOP 3.
    assert!(
        t0.elapsed() < Duration::from_millis(1200),
        "{:?}",
        t0.elapsed()
    );
}

#[test]
fn without_interrupts_the_decode_finishes_first() {
    let (mut p, probe) = started(100, Duration::from_millis(2), false);
    p.scrub(390);
    p.tick(Instant::now());
    std::thread::sleep(Duration::from_millis(20));
    p.scrub(110);
    settle(&mut p, 110);
    assert_eq!(probe.interrupted.load(Ordering::SeqCst), 0);
    assert_eq!(p.stats().interrupted, 0);
}

#[test]
fn a_steady_drag_shows_frames_along_the_way_and_lands_exactly() {
    let (mut p, _) = started(50, Duration::from_millis(1), true);
    let mut seen = Vec::new();
    for i in 0..60u32 {
        p.scrub(10 + i * 3);
        if let Some(f) = p.tick(Instant::now()).show {
            seen.push(f.index);
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    // Most moves show something new.
    assert!(seen.len() >= 20, "{} frames: {seen:?}", seen.len());
    // Let go somewhere else: exactly that frame, nothing pending, not scrubbing.
    p.seek(123);
    settle(&mut p, 123);
    assert!(!p.position().scrubbing);
    assert_eq!(shown(&p), Some(123));
}

#[test]
fn the_ring_stays_within_its_budget_while_scrubbing() {
    let budget = 16 * 16 * 4 * 6;
    let (mut p, _) = movie(50, Duration::ZERO, true, budget);
    settle(&mut p, 0);
    for i in 0..200u32 {
        p.scrub((i * 7) % 400);
        p.tick(Instant::now());
        assert!(
            p.stats().cached_bytes <= budget,
            "{}",
            p.stats().cached_bytes
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    p.seek(333);
    settle(&mut p, 333);
    assert!(p.stats().cached_bytes <= budget);
}

#[test]
fn playback_pauses_while_dragging_and_resumes_on_release() {
    let (mut p, _) = started(50, Duration::ZERO, true);
    p.play(Instant::now());
    assert!(p.is_playing());
    p.scrub(100);
    let pos = p.position();
    assert!(pos.scrubbing && pos.playing && !p.is_playing());
    settle(&mut p, 100);
    // Still holding: nothing plays on.
    std::thread::sleep(Duration::from_millis(100));
    p.tick(Instant::now());
    assert_eq!(shown(&p), Some(100));
    p.seek(120);
    settle(&mut p, 120);
    assert!(p.is_playing() && !p.position().scrubbing);
}

#[test]
fn play_and_pause_while_dragging_apply_on_release() {
    let (mut p, _) = started(50, Duration::ZERO, true);
    p.scrub(40);
    p.toggle(Instant::now());
    assert!(!p.is_playing() && p.position().playing);
    p.seek(40);
    assert!(p.is_playing());
}

#[test]
fn a_frame_step_ends_the_drag() {
    let (mut p, _) = started(50, Duration::ZERO, true);
    p.scrub(70);
    settle(&mut p, 70);
    p.step_forward();
    settle(&mut p, 71);
    assert!(!p.position().scrubbing);
}

#[test]
fn scrubbing_an_animation_without_seeking_works() {
    let mut spec = FakeSpec::new(30, Duration::from_millis(40), 8, 8);
    spec.decode_cost = Duration::from_millis(1);
    let info = AnimationInfo {
        frame_count: Some(30),
        loop_count: 0,
    };
    let (frames, _) = FakeFrames::new(spec);
    let mut p = Player::new(Box::new(frames), &info, PlayerConfig::default(), None);
    settle(&mut p, 0);
    // Its timeline is known once every frame has decoded (seeks clamp to the known part).
    assert!(p.wait_idle(LIMIT));
    p.tick(Instant::now());
    p.scrub(25);
    settle(&mut p, 25);
    p.scrub(5);
    settle(&mut p, 5);
    p.seek(6);
    settle(&mut p, 6);
}

#[test]
fn dragging_back_through_a_gop_keeps_the_frames_before_the_pointer() {
    // Ten frames fit half the ring.
    let budget = 16 * 16 * 4 * 20;
    let (mut p, probe) = movie(100, Duration::from_millis(2), true, budget);
    settle(&mut p, 0);
    p.seek(190);
    settle(&mut p, 190);
    p.scrub(185);
    settle(&mut p, 185);
    assert!(p.wait_idle(LIMIT));
    let decoded = probe.decoded.load(Ordering::SeqCst);
    for t in [184, 182, 179, 177] {
        p.scrub(t);
        let seen = settle(&mut p, t);
        assert_eq!(seen, [t]);
    }
    assert!(p.wait_idle(LIMIT));
    assert_eq!(probe.decoded.load(Ordering::SeqCst), decoded);
    assert!(p.stats().cached_bytes <= budget);
}
