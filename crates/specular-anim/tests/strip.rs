//! The scrub strip: what it holds and in what order, its memory bound, the strip frame
//! nearest the pointer while dragging, the hand-over to the full frame, holding still while
//! the exact frame decodes, and ending with the player.

use specular_anim::{
    DelayPolicy, FakeFrames, FakeProbe, FakeSpec, LoopMode, Player, PlayerConfig, StripConfig,
    StripKind, strip_layout, strip_members, strip_pick, strip_shrink,
};
use specular_core::budget::{MemoryBudget, with_budget};
use specular_core::{HdrEncoding, PixelLayout, Raster, RasterPlane, YuvEncoding, YuvMatrix};
use specular_decode::{AnimationInfo, Frames};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(20);
const FRAME: Duration = Duration::from_millis(40);

fn spec(gop: u32, cost: Duration) -> FakeSpec {
    let mut s = FakeSpec::new(400, FRAME, 64, 36);
    s.seekable = true;
    s.gop = Some(gop);
    s.decode_cost = cost;
    s.interruptible = true;
    s
}

/// A 400-frame, 16 s movie at 25 fps, 64×36, a sync frame every `gop`, `cost` a frame
/// for the player; the strip's decoder costs `strip_cost` a frame.
fn movie(gop: u32, cost: Duration) -> (Player, Arc<FakeProbe>) {
    let info = AnimationInfo {
        frame_count: Some(400),
        loop_count: 1,
    };
    let cfg = PlayerConfig {
        autoplay: false,
        delay_policy: DelayPolicy::AS_STORED,
        loop_mode: LoopMode::Times(1),
        max_ahead: 2,
        ..PlayerConfig::default()
    };
    let (frames, probe) = FakeFrames::new(spec(gop, cost));
    (Player::new(Box::new(frames), &info, cfg, None), probe)
}

fn strip_cfg() -> StripConfig {
    StripConfig {
        height: 18,
        min_height: 9,
        min_speed: 0.0,
        ..StripConfig::default()
    }
}

/// Starts the strip on a fake decoder; its probe.
fn start_strip(p: &mut Player, gop: u32, cost: Duration, cfg: StripConfig) -> Arc<FakeProbe> {
    let (frames, probe) = FakeFrames::new(spec(gop, cost));
    p.start_strip(
        Box::new(move || Ok(Box::new(frames) as Box<dyn Frames>)),
        cfg,
    );
    probe
}

fn settle(p: &mut Player, want: u32) -> Vec<(u32, bool)> {
    let t0 = Instant::now();
    let mut seen = Vec::new();
    loop {
        if let Some(f) = p.tick(Instant::now()).show {
            seen.push((f.index, f.strip));
        }
        if p.current().is_some_and(|f| f.index == want && !f.strip)
            && p.position().pending.is_none()
        {
            return seen;
        }
        assert!(
            t0.elapsed() < LIMIT,
            "frame {want} never showed; saw {seen:?}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn strip_done(p: &Player) {
    let t0 = Instant::now();
    while !p.strip_stats().unwrap().done {
        assert!(t0.elapsed() < LIMIT, "{:?}", p.strip_stats());
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn starts(n: u32) -> Vec<u64> {
    (0..u64::from(n)).map(|i| i * 40_000_000).collect()
}

#[test]
fn members_are_spaced_and_snap_to_sync_frames() {
    let s = starts(400);
    let sync: Vec<u32> = (0..400).step_by(100).collect();
    let m = strip_members(&s, 400 * 40_000_000, &sync, 250_000_000);
    // 16 s at 0.25 s: 64 places, frames 0, 6, 12, 18, 25...; 100, 200 and 300 as they are.
    assert_eq!(m.len(), 64);
    for k in &sync {
        assert!(m.contains(k), "{k}: {m:?}");
    }
    assert!(m.windows(2).all(|w| w[1] - w[0] <= 7), "{m:?}");
    // Every frame a sync frame (intra): the frame nearest each place.
    let all: Vec<u32> = (0..400).collect();
    let m2 = strip_members(&s, 400 * 40_000_000, &all, 250_000_000);
    assert_eq!(m2.len(), 64);
    assert_eq!(&m2[..4], &[0, 6, 12, 19]);
}

#[test]
fn the_layout_stays_within_its_budget() {
    let s = starts(400);
    let total = 400 * 40_000_000;
    let sync: Vec<u32> = (0..400).step_by(100).collect();
    let kind = StripKind::Yuv420;
    let cfg = StripConfig::default();
    // 1080p: 854×480 frames, 614,880 bytes each.
    let per = kind.bytes(854, 480);
    assert_eq!(per, 854 * 480 + 2 * 427 * 240);
    let roomy = strip_layout(&s, total, &sync, (1920, 1080), kind, 64 << 20, &cfg);
    assert_eq!(
        (roomy.width, roomy.height, roomy.members.len()),
        (853, 480, 64)
    );
    // Half the frames at full size, then smaller ones, then fewer: never over the budget.
    for budget in [40 << 20, 20 << 20, 4 << 20, 1 << 20, 100_000, 1] {
        let l = strip_layout(&s, total, &sync, (1920, 1080), kind, budget, &cfg);
        let bytes = l.members.len() * kind.bytes(l.width, l.height);
        assert!(!l.members.is_empty());
        assert!(
            bytes <= budget.max(kind.bytes(l.width, l.height)),
            "{budget}: {} × {}×{}",
            l.members.len(),
            l.width,
            l.height
        );
        assert!(l.height >= cfg.min_height && l.height <= 480);
        assert_eq!(l.width, (1920 * l.height + 540) / 1080);
    }
    let half = strip_layout(&s, total, &sync, (1920, 1080), kind, 20 << 20, &cfg);
    assert_eq!((half.height, half.members.len()), (480, 32));
    // A small movie is never enlarged.
    let small = strip_layout(&s, total, &sync, (320, 180), kind, 64 << 20, &cfg);
    assert_eq!((small.width, small.height), (320, 180));
}

#[test]
fn sync_frames_come_first_nearest_the_pointer_then_gops_in_order() {
    let sync: Vec<u32> = (0..400).step_by(100).collect();
    let m = strip_members(&starts(400), 400 * 40_000_000, &sync, 250_000_000);
    let open = |held: &[u32]| {
        let held = held.to_vec();
        move |k: u32| !held.contains(&k)
    };
    assert_eq!(strip_pick(&m, &sync, open(&[]), 260, None, true), Some(300));
    assert_eq!(
        strip_pick(&m, &sync, open(&[300]), 260, None, true),
        Some(200)
    );
    assert_eq!(strip_pick(&m, &sync, open(&sync), 260, None, false), None);
    // The GOP the pointer is in, from its sync frame on...
    let first = strip_pick(&m, &sync, open(&sync), 260, None, true).unwrap();
    assert!((200..210).contains(&first), "{first}");
    // ...going on from where the decoder is.
    let next = strip_pick(
        &m,
        &sync,
        open(&[0, 100, 200, 300, first]),
        260,
        Some(first + 1),
        true,
    )
    .unwrap();
    assert!(next > first && next < first + 8, "{first} {next}");
}

fn yuv(w: u32, h: u32, bits: u8, (hd, vd): (u8, u8), v: [u16; 3]) -> Raster {
    let e = YuvEncoding {
        matrix: YuvMatrix::Bt709,
        full_range: false,
        bits,
        cosited_x: true,
        cosited_y: false,
        bottom_y: false,
        right_x: false,
        alpha: false,
        colour: HdrEncoding::SRGB,
    };
    let bps = e.sample_bytes();
    let plane = |pw: u32, ph: u32, val: u16, d: (u8, u8)| {
        let mut data = Vec::new();
        for y in 0..ph {
            for x in 0..pw {
                // A left/right split, to see the box filter average across it.
                let s = if x < pw / 2 || y > ph { val } else { val / 2 };
                if bps == 2 {
                    data.extend_from_slice(&s.to_ne_bytes());
                } else {
                    data.push(s as u8);
                }
            }
        }
        RasterPlane {
            width: pw,
            height: ph,
            stride: pw as usize * bps,
            data,
            h_div: d.0,
            v_div: d.1,
        }
    };
    let (cw, ch) = (w.div_ceil(u32::from(hd)), h.div_ceil(u32::from(vd)));
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Yuv(e),
        planes: vec![
            plane(w, h, v[0], (1, 1)),
            plane(cw, ch, v[1], (hd, vd)),
            plane(cw, ch, v[2], (hd, vd)),
        ],
        truncated: false,
    }
}

#[test]
fn video_frames_shrink_to_8_bit_420() {
    // 10-bit 4:2:2, 3840×2160 to 854×480.
    let src = yuv(3840, 2160, 10, (2, 1), [800, 512, 400]);
    let s = strip_shrink(&src, 854, 480).unwrap();
    assert_eq!((s.width, s.height), (854, 480));
    let PixelLayout::Yuv(e) = s.layout else {
        panic!()
    };
    assert_eq!(e.bits, 8);
    assert_eq!(e.matrix, YuvMatrix::Bt709);
    assert!(!e.full_range);
    let sizes: Vec<_> = s
        .planes
        .iter()
        .map(|p| (p.width, p.height, p.h_div, p.v_div))
        .collect();
    assert_eq!(
        sizes,
        [(854, 480, 1, 1), (427, 240, 2, 2), (427, 240, 2, 2)]
    );
    assert_eq!(s.memory_bytes(), StripKind::Yuv420.bytes(854, 480));
    let y = &s.planes[0];
    let row = &y.data[..y.width as usize];
    // Left half 800 → 200, right half 400 → 100; the column on the split in between.
    assert_eq!(row[0], 200);
    assert_eq!(row[853], 100);
    assert!(row.iter().all(|&v| (100..=200).contains(&v)));
    assert_eq!(s.planes[1].data[0], 128);
    assert_eq!(s.planes[2].data[0], 100);
    // 8-bit 4:2:0 with odd sizes.
    let src = yuv(1919, 1079, 8, (2, 2), [90, 120, 140]);
    let s = strip_shrink(&src, 853, 479).unwrap();
    assert_eq!(s.planes[1].width, 427);
    assert_eq!(s.planes[1].height, 240);
    assert_eq!(
        (
            s.planes[0].data[0],
            s.planes[1].data[0],
            s.planes[2].data[0]
        ),
        (90, 120, 140)
    );
}

#[test]
fn rgba_frames_shrink_as_rgba() {
    let w = 64u32;
    let mut data = Vec::new();
    for _y in 0..36 {
        for x in 0..w {
            data.extend_from_slice(&[x as u8 * 4, 10, 20, 255]);
        }
    }
    let src = Raster {
        width: 64,
        height: 36,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: 64,
            height: 36,
            stride: 256,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    };
    let s = strip_shrink(&src, 32, 18).unwrap();
    assert_eq!(s.layout, PixelLayout::Rgba);
    assert_eq!(s.planes[0].data[..8], [2, 10, 20, 255, 10, 10, 20, 255]);
}

#[test]
fn the_strip_waits_for_the_first_frame_then_fills_and_ends() {
    let (mut p, _) = movie(100, Duration::ZERO);
    let probe = start_strip(&mut p, 100, Duration::ZERO, strip_cfg());
    std::thread::sleep(Duration::from_millis(100));
    // Nothing before the player's first frame (the decoder is not even opened).
    assert_eq!(probe.decoded.load(Ordering::SeqCst), 0);
    assert_eq!(p.strip_stats().unwrap().frames, 0);
    settle(&mut p, 0);
    strip_done(&p);
    let s = p.strip_stats().unwrap();
    assert_eq!((s.planned, s.frames), (64, 64), "{s:?}");
    assert_eq!((s.width, s.height), (32, 18));
    assert!(s.between);
    assert_eq!(s.bytes, 64 * 32 * 18 * 4);
    // Complete: the strip's decoder is gone.
    let t0 = Instant::now();
    while !probe.dropped.load(Ordering::SeqCst) {
        assert!(t0.elapsed() < LIMIT);
        std::thread::sleep(Duration::from_millis(1));
    }
    // GOPs are decoded through once each, not from their sync frame for every frame.
    assert!(probe.decoded.load(Ordering::SeqCst) <= 400, "{probe:?}");
    // Each strip frame is that frame.
    for i in [0u32, 6, 100, 237, 393] {
        let f = p.strip_frame(i, 3).unwrap();
        assert!(f.strip && f.index.abs_diff(i) <= 3);
        assert!(f.raster.planes[0].data.iter().all(|&v| v == f.index as u8));
    }
}

#[test]
fn slow_decoding_keeps_to_the_sync_frames() {
    let (mut p, _) = movie(100, Duration::ZERO);
    // 30 ms a frame for a 40 ms frame: under the half of real time asked for.
    let cfg = StripConfig {
        min_speed: 2.0,
        ..strip_cfg()
    };
    start_strip(&mut p, 100, Duration::from_millis(30), cfg);
    settle(&mut p, 0);
    strip_done(&p);
    let s = p.strip_stats().unwrap();
    // The sync frames, and the one frame between them that showed decoding is too slow.
    assert_eq!(s.frames, 5, "{s:?}");
    assert!(!s.between);
    for k in [0, 100, 200, 300] {
        assert_eq!(p.strip_frame(k, 0).map(|f| f.index), Some(k));
    }
}

#[test]
fn the_strip_frames_are_reserved_in_the_budget() {
    let (mut p, _) = movie(100, Duration::ZERO);
    let budget = Arc::new(MemoryBudget::fixed(1 << 30));
    let cfg = StripConfig {
        budget_bytes: 20 * 32 * 18 * 4,
        ..strip_cfg()
    };
    with_budget(Arc::clone(&budget), || {
        start_strip(&mut p, 100, Duration::ZERO, cfg.clone());
    });
    settle(&mut p, 0);
    strip_done(&p);
    let s = p.strip_stats().unwrap();
    assert!(s.bytes <= cfg.budget_bytes, "{s:?}");
    assert!(s.frames >= 16, "{s:?}");
    assert_eq!(budget.reserved(), s.bytes as u64);
    p.shutdown();
    assert_eq!(budget.reserved(), 0);
    // A budget with no room: the strip stops, the movie plays.
    let (mut p, _) = movie(100, Duration::ZERO);
    let tight = Arc::new(MemoryBudget::fixed(100));
    with_budget(tight, || {
        start_strip(&mut p, 100, Duration::ZERO, strip_cfg());
    });
    settle(&mut p, 0);
    strip_done(&p);
    assert_eq!(p.strip_stats().unwrap().frames, 0);
    p.seek(150);
    settle(&mut p, 150);
}

#[test]
fn dragging_shows_the_nearest_strip_frame_at_once_then_the_full_one() {
    // 10 ms a frame: frame 290 is 90 frames from its sync frame, about a second.
    let (mut p, _) = movie(100, Duration::from_millis(10));
    start_strip(&mut p, 100, Duration::ZERO, strip_cfg());
    settle(&mut p, 0);
    strip_done(&p);
    p.scrub(290);
    let first = p.tick(Instant::now()).show.unwrap();
    assert!(
        first.strip && first.index.abs_diff(290) <= 3,
        "{}",
        first.index
    );
    assert_eq!(p.stats().strip_shown, 1);
    // A steady drag: a strip frame near the pointer on every move.
    let mut behind = 0;
    for k in 1..=30u32 {
        p.scrub(290 + k * 3);
        p.tick(Instant::now());
        let now = p.current().unwrap().index;
        behind = behind.max(now.abs_diff(290 + k * 3));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(behind <= 4, "{behind}");
    // Resting: the full frame takes over; nothing on the way is further than the one before.
    p.scrub(250);
    let seen = settle(&mut p, 250);
    let last = *seen.last().unwrap();
    assert_eq!(last, (250, false));
    let dist: Vec<u32> = seen.iter().map(|(i, _)| i.abs_diff(250)).collect();
    assert!(dist.windows(2).all(|w| w[1] <= w[0]), "{seen:?}");
    assert!(p.position().scrubbing);
    p.seek(250);
    settle(&mut p, 250);
}

#[test]
fn a_full_frame_takes_over_from_the_strip_frame_at_the_same_place() {
    let (mut p, _) = movie(100, Duration::from_millis(3));
    start_strip(&mut p, 100, Duration::ZERO, strip_cfg());
    settle(&mut p, 0);
    strip_done(&p);
    // Frame 100 is a sync frame: in the strip, and one decode for the player.
    p.scrub(100);
    let f = p.tick(Instant::now()).show.unwrap();
    assert_eq!((f.index, f.strip), (100, true));
    let seen = settle(&mut p, 100);
    assert_eq!(seen, [(100, false)]);
}

#[test]
fn letting_go_holds_the_strip_until_the_exact_frame_shows() {
    let (mut p, _) = movie(400, Duration::from_millis(5));
    // A slow strip: still at work when the drag is let go.
    let probe = start_strip(&mut p, 400, Duration::from_millis(20), strip_cfg());
    settle(&mut p, 0);
    std::thread::sleep(Duration::from_millis(100));
    p.scrub(150);
    p.tick(Instant::now());
    p.seek(200);
    p.tick(Instant::now());
    // The strip's decode in progress stops, and no other starts until frame 200 shows.
    let t0 = Instant::now();
    while !p.wait_strip(Duration::from_millis(50)) {
        assert!(t0.elapsed() < LIMIT);
    }
    let held = probe.decoded.load(Ordering::SeqCst);
    let interrupted = probe.interrupted.load(Ordering::SeqCst);
    let seen = settle(&mut p, 200);
    assert!(seen.last() == Some(&(200, false)), "{seen:?}");
    let s = p.strip_stats().unwrap();
    assert!(s.interrupted >= 1 && interrupted >= 1, "{s:?}");
    // At most the frame that was being made when the hold began.
    assert!(probe.decoded.load(Ordering::SeqCst) <= held + 1);
    // Then the strip goes on.
    std::thread::sleep(Duration::from_millis(200));
    assert!(probe.decoded.load(Ordering::SeqCst) > held + 1);
}

#[test]
fn a_seek_shows_the_strip_frame_near_it_meanwhile() {
    let (mut p, _) = movie(100, Duration::from_millis(5));
    start_strip(&mut p, 100, Duration::ZERO, strip_cfg());
    settle(&mut p, 0);
    strip_done(&p);
    p.seek(260);
    let f = p.tick(Instant::now()).show.unwrap();
    assert!(f.strip && f.index.abs_diff(260) <= 3);
    let seen = settle(&mut p, 260);
    assert_eq!(seen.last(), Some(&(260, false)));
    // A step shows the exact frame only.
    p.step_forward();
    let seen = settle(&mut p, 261);
    assert_eq!(seen, [(261, false)]);
}

#[test]
fn dropping_the_player_ends_the_strip() {
    let (mut p, _) = movie(400, Duration::ZERO);
    let probe = start_strip(&mut p, 400, Duration::from_millis(30), strip_cfg());
    settle(&mut p, 0);
    std::thread::sleep(Duration::from_millis(50));
    let t0 = Instant::now();
    p.shutdown();
    assert!(probe.dropped.load(Ordering::SeqCst));
    // The decode in progress was stopped, not finished frame by frame.
    assert!(
        t0.elapsed() < Duration::from_millis(500),
        "{:?}",
        t0.elapsed()
    );
}

#[test]
fn without_a_strip_nothing_changes() {
    let (mut p, _) = movie(100, Duration::from_millis(1));
    settle(&mut p, 0);
    assert!(p.strip_stats().is_none());
    p.seek(250);
    let seen = settle(&mut p, 250);
    assert_eq!(seen, [(250, false)]);
    assert_eq!(p.stats().strip_shown, 0);
}
