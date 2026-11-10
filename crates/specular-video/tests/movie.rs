//! Movies opened as images (tests/data, made by tests/data/gen_fixtures.sh with ffmpeg, whose
//! decodes are the oracle): every frame exact against ffmpeg for H.264 with B-frames, HEVC
//! Main 10 HLG and AV1; frame-accurate seeking; the rotation golden; the reasons given for
//! what does not play; and the sound path with a fake device, including A/V sync in the
//! animation player.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::hdr::{Primaries, Transfer};
use specular_core::{CancelToken, PixelLayout, Raster, Source, YuvMatrix};
use specular_decode::{DecodeError, DecodeRequest, Format, Frames, Session};
use specular_video::audio::{AudioPlayer, FakeOutput, Silence};
use specular_video::session::{self, VideoSession};

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn open(name: &str, format: Format) -> VideoSession {
    let src = Arc::new(Source::open(data(name)).unwrap());
    session::open(src, format).unwrap()
}

/// The planes of a YCbCr frame, tightly packed, as ffmpeg's rawvideo writes them.
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

/// Decodes every frame and compares each with ffmpeg's decode; returns the frame count.
fn all_frames_exact(name: &str, format: Format, reference: &str) -> usize {
    let s = open(name, format);
    let want = std::fs::read(data(reference)).unwrap();
    let mut frames = s.video_frames().unwrap();
    let cancel = CancelToken::new();
    let mut n = 0;
    let mut at = 0;
    while let Some(f) = frames.next_frame(&cancel) {
        let f = f.unwrap();
        assert_eq!(f.index as usize, n);
        let got = planes(&f.raster);
        assert!(
            at + got.len() <= want.len(),
            "{name}: more frames than ffmpeg"
        );
        let diff = got
            .iter()
            .zip(&want[at..at + got.len()])
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(
            diff, 0,
            "{name}: frame {n} differs from ffmpeg in {diff} bytes"
        );
        at += got.len();
        n += 1;
    }
    assert_eq!(at, want.len(), "{name}: fewer frames than ffmpeg");
    assert_eq!(Some(n as u32), s.animation().and_then(|a| a.frame_count));
    n
}

#[test]
fn h264_with_b_frames_matches_ffmpeg() {
    let n = all_frames_exact("rot90.mov", Format::QuickTime, "rot90.yuv");
    assert_eq!(n, 50);
}

#[test]
fn hevc_main10_hlg_matches_ffmpeg_and_goes_the_hdr_way() {
    all_frames_exact("hlg10.mp4", Format::Mp4, "hlg10.yuv");
    let s = open("hlg10.mp4", Format::Mp4);
    assert_eq!(s.video().unwrap().hdr, Some("HLG"));
    let f = s
        .video_frames()
        .unwrap()
        .next_frame(&CancelToken::new())
        .unwrap()
        .unwrap();
    let PixelLayout::Yuv(e) = f.raster.layout else {
        panic!("{:?}", f.raster.layout)
    };
    assert_eq!(
        (e.bits, e.full_range, e.matrix),
        (10, false, YuvMatrix::Bt2020)
    );
    assert_eq!(e.colour.transfer, Transfer::Hlg);
    assert_eq!(e.colour.primaries, Primaries::Bt2020);
}

#[test]
fn av1_matches_ffmpeg() {
    all_frames_exact("av1.mp4", Format::Mp4, "av1.yuv");
    let s = open("av1.mp4", Format::Mp4);
    assert_eq!(s.video().unwrap().codec, "AV1");
}

/// Seeking to any frame gives exactly that frame (decoded from the nearest preceding sync
/// sample), forwards, backwards, onto key frames and B-frames.
#[test]
fn seeks_are_frame_accurate() {
    let s = open("rot90.mov", Format::QuickTime);
    let want = std::fs::read(data("rot90.yuv")).unwrap();
    let size = 64 * 48 * 3 / 2;
    let mut frames = s.video_frames().unwrap();
    let cancel = CancelToken::new();
    let mut x = 7u32;
    let mut worst = Duration::ZERO;
    let mut targets: Vec<u32> = vec![49, 0, 12, 11, 13, 25, 24, 1, 2, 48, 47, 30, 30, 31];
    targets.extend((0..40).map(|_| {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (x >> 16) % 50
    }));
    for i in targets {
        let t = Instant::now();
        assert!(frames.seek(i).unwrap());
        let f = frames.next_frame(&cancel).unwrap().unwrap();
        worst = worst.max(t.elapsed());
        assert_eq!(f.index, i);
        let i = i as usize;
        assert!(
            planes(&f.raster) == want[i * size..(i + 1) * size],
            "frame {i} after a seek differs from ffmpeg"
        );
    }
    // Decoding on after a seek continues in order.
    frames.seek(20).unwrap();
    for i in 20..26 {
        let f = frames.next_frame(&cancel).unwrap().unwrap();
        assert_eq!(f.index, i);
        assert!(planes(&f.raster) == want[i as usize * size..(i as usize + 1) * size]);
    }
    eprintln!("64x48 H.264 seek: worst {worst:?} (keyint 12, 3 B-frames)");
}

/// Rotation golden: the track says "turn 90° clockwise"; the session's orientation does it,
/// and frame 0 turned that way matches ffmpeg's rotated rendering.
#[test]
fn rotation_matches_ffmpeg() {
    let s = open("rot90.mov", Format::QuickTime);
    let info = s.info();
    assert_eq!((info.width, info.height), (64, 48));
    assert_eq!(info.orientation.0, 6);
    assert_eq!(info.display_size(), (48, 64));
    assert_eq!(s.video().unwrap().rotation, 90);
    let rgb = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!(rgb.layout, PixelLayout::Rgba);
    let want = std::fs::read(data("rot90.rgb")).unwrap();
    let p = &rgb.planes[0];
    let (w, h) = (rgb.width as usize, rgb.height as usize);
    let mut worst = 0u8;
    let mut sum = 0u64;
    // Displayed (x, y) in the 48x64 picture comes from stored (y, h - 1 - x).
    for y in 0..w {
        for x in 0..h {
            let (sx, sy) = (y, h - 1 - x);
            for c in 0..3 {
                let got = p.data[sy * p.stride + sx * 4 + c];
                let exp = want[(y * h + x) * 3 + c];
                let d = got.abs_diff(exp);
                sum += u64::from(d);
                // Chroma edges: our co-sited upsampling against swscale's.
                // Away from the chroma edges (the quadrants' and the moving square's), where
                // our co-sited upsampling and swscale's interpolate differently.
                if !(28..=36).contains(&sx) && !(16..=31).contains(&sy) {
                    worst = worst.max(d);
                }
            }
        }
    }
    let mean = sum as f64 / (w * h * 3) as f64;
    eprintln!("rotation golden: mean |diff| {mean:.2}, worst away from edges {worst}");
    assert!(worst <= 3 && mean < 6.0, "worst {worst}, mean {mean}");
}

#[test]
fn thumbnail_frame_is_a_tenth_in() {
    let s = open("rot90.mov", Format::QuickTime);
    let r = s.decode_sampled(256, &CancelToken::new()).unwrap().unwrap();
    assert_eq!((r.width, r.height), (64, 48));
    // Frame 5: the moving square's left edge is at x = 10.
    let p = &r.planes[0];
    let px = |x: usize, y: usize| &p.data[y * p.stride + x * 4..][..3];
    assert!(px(13, 23).iter().all(|&v| v > 200), "{:?}", px(13, 23));
    assert!(px(4, 23)[1] < 60, "{:?}", px(4, 23));
}

#[test]
fn facts_and_formats() {
    let s = open("rot90.mov", Format::QuickTime);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "H.264");
    assert_eq!(f.frames, 50);
    assert_eq!(f.duration_us, Some(2_000_000));
    assert!((f.frame_rate.unwrap() - 25.0).abs() < 0.01);
    assert_eq!(f.audio.as_deref(), Some("PCM stereo 16 kHz"));
    assert_eq!(f.silent_reason, None);
    assert_eq!(f.unplayable, None);
    assert!(
        s.info().coding.starts_with("H.264, 25 fps"),
        "{}",
        s.info().coding
    );

    let s = open("clip.3gp", Format::ThreeGpp);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "H.264");
    assert!(f.audio.as_deref().unwrap().starts_with("AAC mono"));
    // AAC plays.
    assert_eq!(f.silent_reason, None);
    specular_video::audio::playable(s.movie().audio.as_ref().unwrap()).unwrap();
    s.decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
}

/// Movies that don't play still open; decoding says why, in words.
#[test]
fn unplayable_movies_say_why() {
    // Interlaced H.264 plays, deinterlaced.
    let s = open("interlaced.mp4", Format::Mp4);
    assert_eq!(s.video().unwrap().unplayable, None);
    assert!(s.video().unwrap().interlaced);
    assert!(s.video_frames().unwrap().deinterlacing());
    s.decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    // MPEG-4 Part 2 plays (tests/mpeg4.rs compares it with ffmpeg); a codec without a
    // decoder says its name.
    let s = open("mpeg4.mp4", Format::Mp4);
    assert_eq!(s.video().unwrap().unplayable, None);
    s.decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    let why = "Microsoft MPEG-4 (DivX 3) video";
    let s = open("msmpeg4.avi", Format::Avi);
    assert_eq!(s.video().unwrap().unplayable.as_deref(), Some(why));
    assert!(s.animation().is_none());
    match s.decode(&DecodeRequest::full(1), &CancelToken::new()) {
        Err(DecodeError::Unsupported(m)) => assert_eq!(m, why),
        other => panic!("msmpeg4.avi: {other:?}"),
    }
}

/// The registry recognises movies by their `ftyp` brand and hands them to the opener.
#[test]
fn registry_opens_movies_through_the_installed_opener() {
    specular_video::install();
    let reg = specular_decode::Registry::new();
    for (name, format) in [
        ("rot90.mov", Format::QuickTime),
        ("hlg10.mp4", Format::Mp4),
        ("clip.3gp", Format::ThreeGpp),
    ] {
        let s = reg
            .open(Arc::new(Source::open(data(name)).unwrap()))
            .unwrap();
        assert_eq!(s.info().format, format, "{name}");
        assert!(s.video().is_some());
        assert!(s.animation().is_some());
    }
}

/// PCM sound through a fake device at the track's rate: the samples come out as stored, the
/// clock follows the buffers, a seek lands on the right sample, muting keeps the clock.
#[test]
fn pcm_sound_plays_and_keeps_time() {
    let src = Arc::new(Source::open(data("rot90.mov")).unwrap());
    let movie = specular_video::mp4::parse(src.bytes(), true).unwrap();
    let out = FakeOutput::new(16_000, 2);
    let player = AudioPlayer::open(Arc::clone(&src), &movie, 0, Box::new(out.clone())).unwrap();
    // The sine as stored: 1 kHz at 16 kHz, both channels alike.
    let a = movie.audio.as_ref().unwrap();
    let stored: Vec<f32> = a
        .samples
        .iter()
        .flat_map(|s| {
            let b = specular_video::mp4::sample_bytes(src.bytes(), s).unwrap();
            b.chunks_exact(2)
                .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
                .collect::<Vec<_>>()
        })
        .collect();
    let t0 = Instant::now();
    assert_eq!(player.heard(t0), None, "not playing yet");
    player.play(Duration::ZERO);
    // Let the decoder fill the queue, then play 0.25 s in 10 ms buffers heard 20 ms later.
    let pull_all = |from: Instant, buffers: usize| {
        for k in 0..buffers {
            let heard = from + Duration::from_millis(20 + 10 * k as u64);
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let before = out.played.lock().len();
                out.pull(160, heard);
                let got = out.played.lock()[before..].iter().any(|&v| v != 0.0);
                if got || Instant::now() > deadline {
                    break;
                }
                out.played.lock().truncate(before);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    };
    pull_all(t0, 25);
    let played = out.played.lock().clone();
    assert_eq!(played.len(), 25 * 160 * 2);
    let diff = played
        .iter()
        .zip(&stored)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(diff < 1e-6, "played differs from stored by {diff}");
    // The last buffer (frames 3840..4000) is heard from t0 + 260 ms.
    let heard = player.heard(t0 + Duration::from_millis(265)).unwrap();
    assert_eq!(heard, Duration::from_millis(245));
    // Seek to 1 s: the next buffer starts with sample 16000.
    player.seek(Duration::from_secs(1));
    assert_eq!(player.heard(t0 + Duration::from_millis(300)), None);
    out.played.lock().clear();
    let t1 = t0 + Duration::from_secs(1);
    pull_all(t1, 1);
    let played = out.played.lock().clone();
    let at = 16_000 * 2;
    assert!(
        played
            .iter()
            .zip(&stored[at..])
            .all(|(a, b)| (a - b).abs() < 1e-6),
        "a seek lands on its sample"
    );
    assert_eq!(
        player.heard(t1 + Duration::from_millis(25)),
        Some(Duration::from_millis(1005))
    );
    // Muted: silence, same clock.
    player.set_muted(true);
    out.played.lock().clear();
    out.pull(160, t1 + Duration::from_millis(30));
    assert!(out.played.lock().iter().all(|&v| v == 0.0));
    assert_eq!(
        player.heard(t1 + Duration::from_millis(30)),
        Some(Duration::from_millis(1010))
    );
    player.pause();
    assert_eq!(player.heard(t1 + Duration::from_millis(31)), None);
}

#[test]
fn sound_resampled_to_the_device() {
    let src = Arc::new(Source::open(data("rot90.mov")).unwrap());
    let movie = specular_video::mp4::parse(src.bytes(), true).unwrap();
    let out = FakeOutput::new(48_000, 2);
    let player = AudioPlayer::open(Arc::clone(&src), &movie, 0, Box::new(out.clone())).unwrap();
    player.play(Duration::ZERO);
    let t0 = Instant::now();
    std::thread::sleep(Duration::from_millis(50));
    for k in 0..10u64 {
        out.pull(480, t0 + Duration::from_millis(10 * k));
    }
    let played = out.played.lock().clone();
    // A 1 kHz sine of ffmpeg's amplitude (1/8, −3 dB per channel of its stereo) at 48 kHz.
    let peak = played.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let want = 0.125 * std::f32::consts::FRAC_1_SQRT_2;
    assert!((peak - want).abs() < 0.002, "peak {peak}");
    let crossings = played
        .chunks_exact(2)
        .map(|f| f[0])
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
        .count();
    assert!(
        (99..=101).contains(&crossings),
        "{crossings} cycles in 0.1 s"
    );
}

#[test]
fn no_device_and_lost_device_go_silent() {
    let src = Arc::new(Source::open(data("rot90.mov")).unwrap());
    let movie = specular_video::mp4::parse(src.bytes(), true).unwrap();
    let r = AudioPlayer::open(
        Arc::clone(&src),
        &movie,
        0,
        Box::new(FakeOutput::failing("no sound server")),
    );
    assert!(matches!(r, Err(Silence::NoDevice(_))));
    let out = FakeOutput::new(16_000, 2);
    let player = AudioPlayer::open(src, &movie, 0, Box::new(out.clone())).unwrap();
    assert!(!player.take_lost());
    out.unplug();
    assert!(player.take_lost(), "said once");
    assert!(!player.take_lost(), "and only once");
}

/// A/V sync on the clip: the animation player follows the sound's clock from a device whose
/// crystal runs 0.5 % fast; the picture stays within a frame of what is heard.
#[test]
fn picture_follows_the_sound() {
    use specular_anim::{LoopMode, Player, PlayerConfig};
    let s = open("rot90.mov", Format::QuickTime);
    let frames = s.video_frames().unwrap();
    let first_us = frames.first_frame_us();
    let info = s.animation().unwrap().clone();
    let mut p = Player::new(
        Box::new(frames),
        &info,
        PlayerConfig {
            delay_policy: specular_anim::DelayPolicy::AS_STORED,
            loop_mode: LoopMode::Times(1),
            ..PlayerConfig::default()
        },
        None,
    );
    let src = Arc::clone(s.source());
    let out = FakeOutput::new(16_000, 2);
    let audio = AudioPlayer::open(src, s.movie(), first_us, Box::new(out.clone())).unwrap();
    p.set_clock(Some(audio.clock()));
    let t0 = Instant::now();
    assert!(p.wait_idle(Duration::from_secs(10)));
    p.tick(t0);
    audio.play(Duration::ZERO);
    std::thread::sleep(Duration::from_millis(50));
    // The device takes 160 frames every 9.95 ms of wall time (0.5 % fast), heard 20 ms later.
    let mut worst = 0i64;
    let mut now = t0;
    for k in 0..190u64 {
        let heard_at = now + Duration::from_millis(20);
        // Wait for the decoder rather than starve the device (the loop runs faster than
        // real time): a buffer of the sine is never all zero.
        loop {
            let before = out.played.lock().len();
            assert!(out.pull(160, heard_at));
            if out.played.lock()[before..].iter().any(|&v| v != 0.0) {
                break;
            }
            out.played.lock().truncate(before);
            std::thread::sleep(Duration::from_millis(1));
        }
        now = t0 + Duration::from_micros(9_950 * (k + 1));
        assert!(p.wait_idle(Duration::from_secs(10)));
        p.tick(now);
        let Some(heard) = audio.heard(now) else {
            continue;
        };
        let i = p.current().map_or(0, |f| f.index) as i64;
        let want = (heard.as_millis() / 40) as i64;
        worst = worst.max((i - want).abs());
    }
    assert!(worst <= 1, "picture {worst} frames off the sound");
    assert!(p.stats().clock_syncs > 0);
}

/// Timings of plain playback on larger clips made by tools/corpus/video-plain.sh (1080p and
/// 4K, H.264 and HEVC): first frame, seek latency and decode rate. Skips without them.
#[test]
fn timings_on_large_clips() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/out/video-plain");
    let Ok(list) = std::fs::read_dir(&dir) else {
        eprintln!("skipped: run tools/corpus/video-plain.sh");
        return;
    };
    let mut names: Vec<PathBuf> = list.flatten().map(|e| e.path()).collect();
    names.sort();
    for path in names {
        if path.extension().is_none_or(|e| e != "mp4") {
            continue;
        }
        let t = Instant::now();
        let s = session::open(Arc::new(Source::open(&path).unwrap()), Format::Mp4).unwrap();
        let mut frames = s.video_frames().unwrap();
        let cancel = CancelToken::new();
        frames.next_frame(&cancel).unwrap().unwrap();
        let first = t.elapsed();
        let n = s.animation().unwrap().frame_count.unwrap();
        let t = Instant::now();
        let mut k = 1;
        while k < n.min(120) {
            frames.next_frame(&cancel).unwrap().unwrap();
            k += 1;
        }
        let fps = f64::from(k - 1) / t.elapsed().as_secs_f64();
        let mut seeks = Vec::new();
        for i in [n / 2, n / 5, n * 4 / 5, n / 3 + 7] {
            let t = Instant::now();
            frames.seek(i).unwrap();
            frames.next_frame(&cancel).unwrap().unwrap();
            seeks.push(t.elapsed());
        }
        seeks.sort();
        eprintln!(
            "{}: {} {}x{}, first frame {first:?}, {fps:.1} fps, seek median {:?} worst {:?}",
            path.file_name().unwrap().to_string_lossy(),
            frames.backend(),
            s.info().width,
            s.info().height,
            seeks[seeks.len() / 2],
            seeks[seeks.len() - 1]
        );
    }
}
