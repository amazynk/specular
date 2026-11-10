//! Matroska, WebM and MPEG transport streams (tests/data, made by
//! tests/data/gen_fixtures_mkv_ts.sh with ffmpeg, whose decodes are the oracle): every frame
//! exact against ffmpeg for H.264, HEVC, AV1, VP8 and VP9 (8 and 10-bit) in these containers,
//! interlaced H.264 in an AVCHD-style stream (woven frames exact, deinterlaced frames without
//! combing), times across the 33-bit PTS wrap, frame-accurate seeks, Opus and Blu-ray LPCM
//! sound against ffmpeg's decodes, A/V sync, and the reasons given for what does not play.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{DecodeError, Format, Frames, Session};
use specular_video::audio::{AudioPlayer, FakeOutput};
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

/// Decodes every frame (woven, for interlaced video) and compares each with ffmpeg's decode;
/// returns the frame count.
fn all_frames_exact(name: &str, format: Format, reference: &str) -> usize {
    let s = open(name, format);
    let want = std::fs::read(data(reference)).unwrap();
    let mut frames = s.video_frames().unwrap();
    frames.set_deinterlace(false);
    let cancel = CancelToken::new();
    let (mut n, mut at, mut bad) = (0, 0, 0);
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
        if diff > 0 {
            eprintln!("{name}: frame {n} differs from ffmpeg in {diff} bytes");
            bad += 1;
        }
        at += got.len();
        n += 1;
    }
    assert_eq!(bad, 0, "{name}: {bad} of {n} frames differ from ffmpeg");
    assert_eq!(at, want.len(), "{name}: fewer frames than ffmpeg");
    assert_eq!(Some(n as u32), s.animation().and_then(|a| a.frame_count));
    eprintln!("{name}: {n} frames match ffmpeg");
    n
}

#[test]
fn h264_and_hevc_and_av1_in_matroska_match_ffmpeg() {
    assert_eq!(
        all_frames_exact("h264-opus.mkv", Format::Matroska, "h264-opus.yuv"),
        20
    );
    assert_eq!(
        all_frames_exact("hevc.mkv", Format::Matroska, "hevc.yuv"),
        20
    );
    assert_eq!(
        all_frames_exact("av1.mkv", Format::Matroska, "av1-mkv.yuv"),
        10
    );
}

/// HEVC field coding (fields woven in pairs, as ffmpeg's weave filter does) and range
/// extension clips, made by tests/data/gen_fixtures_rext.sh.
#[test]
fn hevc_fields_and_range_extensions_match_ffmpeg() {
    assert_eq!(
        all_frames_exact("hevc-fields.mp4", Format::Mp4, "hevc-fields.yuv"),
        6
    );
    assert_eq!(
        all_frames_exact("hevc-fields.ts", Format::MpegTs, "hevc-fields.yuv"),
        6
    );
    let s = open("hevc-fields.mp4", Format::Mp4);
    assert!(s.video().unwrap().interlaced);
    assert_eq!(
        all_frames_exact("hevc-444-12.mp4", Format::Mp4, "hevc-444-12.yuv"),
        8
    );
    assert_eq!(
        all_frames_exact("hevc-422-12.mkv", Format::Matroska, "hevc-422-12.yuv"),
        8
    );
    // 4:0:0 plays as 4:2:0 with mid-grey chroma; luma as ffmpeg decodes it.
    let s = open("hevc-gray.mp4", Format::Mp4);
    let want = std::fs::read(data("hevc-gray.yuv")).unwrap();
    let mut frames = s.video_frames().unwrap();
    let (mut n, mut at) = (0, 0);
    while let Some(f) = frames.next_frame(&CancelToken::new()) {
        let r = f.unwrap().raster;
        let got = planes(&r);
        let luma = (r.width * r.height) as usize;
        assert_eq!(
            &got[..luma],
            &want[at..at + luma],
            "hevc-gray.mp4 frame {n}"
        );
        assert!(got[luma..].iter().all(|&v| v == 128));
        at += luma;
        n += 1;
    }
    assert_eq!((n, at), (4, want.len()));
}

/// H.264 High 4:4:4 Predictive (CABAC and CAVLC, 8x8 transforms, B-frames, 10 bits,
/// MBAFF), made by tests/data/gen_fixtures_rext.sh.
#[test]
fn h264_444_matches_ffmpeg() {
    assert_eq!(
        all_frames_exact("h264-444.mp4", Format::Mp4, "h264-444.yuv"),
        8
    );
    assert_eq!(
        all_frames_exact("h264-444-cavlc.mkv", Format::Matroska, "h264-444-cavlc.yuv"),
        8
    );
    assert_eq!(
        all_frames_exact("h264-444-10.mp4", Format::Mp4, "h264-444-10.yuv"),
        8
    );
    assert_eq!(
        all_frames_exact("h264-444-mbaff.mp4", Format::Mp4, "h264-444-mbaff.yuv"),
        6
    );
    assert_eq!(
        all_frames_exact(
            "h264-444-lossless.mp4",
            Format::Mp4,
            "h264-444-lossless.yuv"
        ),
        4
    );
}

#[test]
fn vp8_and_vp9_in_webm_match_ffmpeg() {
    assert_eq!(
        all_frames_exact("vp9-opus.webm", Format::WebM, "vp9-opus.yuv"),
        20
    );
    assert_eq!(
        all_frames_exact("vp9-10bit.webm", Format::WebM, "vp9-10bit.yuv"),
        10
    );
    assert_eq!(
        all_frames_exact("vp8-vorbis.webm", Format::WebM, "vp8-vorbis.yuv"),
        20
    );
    // Written to a pipe: unknown sizes, no cues, no duration.
    assert_eq!(all_frames_exact("live.webm", Format::WebM, "live.yuv"), 10);
}

#[test]
fn transport_streams_match_ffmpeg() {
    // Times wrap past 2^33 two frames in.
    assert_eq!(
        all_frames_exact("hevc-wrap.ts", Format::MpegTs, "hevc-wrap.yuv"),
        20
    );
    assert_eq!(
        all_frames_exact("interlaced.m2ts", Format::MpegTs, "interlaced.yuv"),
        20
    );
}

/// How much a frame combs: luma pixels of the bottom field lying more than 24 steps outside
/// the range of the lines above and below.
fn comb_energy(r: &Raster) -> u64 {
    let p = &r.planes[0];
    let px = |x: usize, y: usize| i32::from(p.data[y * p.stride + x]);
    let (w, h) = (p.width as usize, p.height as usize);
    (1..h - 1)
        .step_by(2)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let (a, b, c) = (px(x, y - 1), px(x, y + 1), px(x, y));
            c < a.min(b) - 24 || c > a.max(b) + 24
        })
        .count() as u64
}

/// The moving square of the interlaced clip combs as decoded and does not once deinterlaced;
/// the still quadrants keep their edges.
#[test]
fn interlaced_video_is_deinterlaced() {
    let s = open("interlaced.m2ts", Format::MpegTs);
    assert!(s.video().unwrap().interlaced);
    let cancel = CancelToken::new();
    let mut woven = s.video_frames().unwrap();
    woven.set_deinterlace(false);
    let mut shown = s.video_frames().unwrap();
    assert!(shown.deinterlacing());
    let (mut before, mut after) = (0, 0);
    while let (Some(a), Some(b)) = (woven.next_frame(&cancel), shown.next_frame(&cancel)) {
        let (a, b) = (a.unwrap(), b.unwrap());
        before += comb_energy(&a.raster);
        after += comb_energy(&b.raster);
        // Top field lines are kept.
        let (pa, pb) = (&a.raster.planes[0], &b.raster.planes[0]);
        for y in (0..pa.height as usize).step_by(2) {
            let r = y * pa.stride..y * pa.stride + pa.width as usize;
            assert_eq!(pa.data[r.clone()], pb.data[r]);
        }
    }
    eprintln!("comb energy woven {before}, deinterlaced {after}");
    assert!(after * 3 < before, "woven {before}, deinterlaced {after}");
}

#[test]
fn facts_of_each_container() {
    let s = open("h264-opus.mkv", Format::Matroska);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "H.264");
    assert_eq!(f.frames, 20);
    assert_eq!(f.duration_us, Some(800_000));
    assert!((f.frame_rate.unwrap() - 25.0).abs() < 0.01);
    assert_eq!(f.audio.as_deref(), Some("Opus stereo 48 kHz"));
    assert_eq!(f.silent_reason, None);
    assert!(!f.interlaced);

    let s = open("vp9-opus.webm", Format::WebM);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "VP9");
    assert_eq!(f.audio.as_deref(), Some("Opus mono 48 kHz"));
    assert_eq!(f.frames, 20);

    let s = open("vp8-vorbis.webm", Format::WebM);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "VP8");
    assert!(f.audio.as_deref().unwrap().starts_with("Vorbis stereo"));
    assert_eq!(f.silent_reason, None);

    let s = open("live.webm", Format::WebM);
    assert_eq!(s.video().unwrap().frames, 10);
    assert_eq!(s.video().unwrap().duration_us, Some(400_000));

    let s = open("interlaced.m2ts", Format::MpegTs);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "H.264");
    assert!(f.interlaced);
    assert_eq!(f.frames, 20);
    assert_eq!(f.audio.as_deref(), Some("PCM stereo 48 kHz"));
    assert_eq!(f.silent_reason, None);
    assert!((f.frame_rate.unwrap() - 25.0).abs() < 0.01);

    let s = open("ac3.mts", Format::MpegTs);
    let f = s.video().unwrap();
    assert_eq!(f.audio.as_deref(), Some("AC-3 mono 48 kHz"));
    assert_eq!(f.silent_reason, None);
    assert_eq!(f.frames, 10);

    let s = open("hevc-wrap.ts", Format::MpegTs);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "HEVC");
    assert_eq!(f.duration_us, Some(800_000));
}

/// What does not play still opens (or is refused) with a reason in words.
#[test]
fn refusals_say_why() {
    // VP9 4:4:4 plays since round 4 (tests/round4.rs).
    let s = open("vp9-444.webm", Format::WebM);
    assert_eq!(s.video().unwrap().unplayable, None);
    let src = Arc::new(Source::open(data("sound.mka")).unwrap());
    match session::open(src, Format::Matroska) {
        Err(DecodeError::Unsupported(m)) => assert_eq!(m, "sound only, no picture"),
        Err(e) => panic!("{e:?}"),
        Ok(_) => panic!("sound.mka opened"),
    }
}

/// The registry recognises the containers by content and hands them to the opener.
#[test]
fn registry_opens_them_through_the_installed_opener() {
    specular_video::install();
    let reg = specular_decode::Registry::new();
    for (name, format) in [
        ("h264-opus.mkv", Format::Matroska),
        ("vp9-opus.webm", Format::WebM),
        ("interlaced.m2ts", Format::MpegTs),
        ("hevc-wrap.ts", Format::MpegTs),
        ("ac3.mts", Format::MpegTs),
    ] {
        let s = reg
            .open(Arc::new(Source::open(data(name)).unwrap()))
            .unwrap();
        assert_eq!(s.info().format, format, "{name}");
        assert!(s.video().is_some());
        assert!(s.animation().is_some(), "{name}");
        // The gallery's thumbnail.
        let r = s.decode_sampled(256, &CancelToken::new()).unwrap().unwrap();
        assert_eq!((r.width, r.height), (64, 48), "{name}");
    }
}

/// Seeking to any frame gives exactly that frame, in each container.
#[test]
fn seeks_are_frame_accurate() {
    for (name, format, reference, frames, bytes) in [
        ("h264-opus.mkv", Format::Matroska, "h264-opus.yuv", 20, 4608),
        ("vp9-opus.webm", Format::WebM, "vp9-opus.yuv", 20, 4608),
        ("vp8-vorbis.webm", Format::WebM, "vp8-vorbis.yuv", 20, 4608),
        ("hevc-wrap.ts", Format::MpegTs, "hevc-wrap.yuv", 20, 4608),
        (
            "interlaced.m2ts",
            Format::MpegTs,
            "interlaced.yuv",
            20,
            4608,
        ),
    ] {
        let s = open(name, format);
        let want = std::fs::read(data(reference)).unwrap();
        let mut f = s.video_frames().unwrap();
        f.set_deinterlace(false);
        let cancel = CancelToken::new();
        let mut x = 11u32;
        let mut targets: Vec<u32> = vec![frames - 1, 0, 8, 7, 9, 1, frames / 2, 3, 3];
        targets.extend((0..20).map(|_| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (x >> 16) % frames
        }));
        let mut worst = Duration::ZERO;
        for i in targets {
            let t = Instant::now();
            assert!(f.seek(i).unwrap());
            let got = f.next_frame(&cancel).unwrap().unwrap();
            worst = worst.max(t.elapsed());
            assert_eq!(got.index, i, "{name}");
            let at = i as usize * bytes;
            assert!(
                planes(&got.raster) == want[at..at + bytes],
                "{name}: frame {i} after a seek"
            );
        }
        eprintln!("{name}: seeks exact, worst {worst:?}");
    }
}

/// Plays a movie's sound through a fake 48 kHz device and returns what was heard (the first
/// `frames` device frames, interleaved stereo).
fn heard(name: &str, format: Format, frames: usize) -> Vec<f32> {
    let s = open(name, format);
    let first_us = s.video_frames().unwrap().first_frame_us();
    let out = FakeOutput::new(48_000, 2);
    let player = AudioPlayer::open(
        Arc::clone(s.source()),
        s.movie(),
        first_us,
        Box::new(out.clone()),
    )
    .unwrap();
    player.play(Duration::ZERO);
    // Each 10 ms buffer is pulled once the player has it decoded (or the sound ended), so
    // none runs short however loaded the machine: what is heard does not depend on timing.
    // Silence before the first sound is not counted (the comparisons start at the sound).
    let t0 = Instant::now();
    let mut k = 0;
    let mut started = false;
    while out.played.lock().len() < frames * 2 {
        let more = player.wait_queued(480);
        let before = out.played.lock().len();
        out.pull(480, t0 + Duration::from_millis(10 * k));
        k += 1;
        if !started {
            started = out.played.lock()[before..].iter().any(|&v| v != 0.0);
            if !started {
                out.played.lock().truncate(before);
            }
        }
        if !more {
            break;
        }
    }
    let mut v = out.played.lock().clone();
    v.truncate(frames * 2);
    v
}

/// Signal-to-noise ratio of `got` against `want` (dB) and the largest difference in 16-bit
/// steps.
fn compare(got: &[f32], want: &[f32]) -> (f64, f64) {
    let (mut sig, mut err, mut worst) = (0f64, 0f64, 0f64);
    for (g, w) in got.iter().zip(want) {
        let (g, w) = (f64::from(*g), f64::from(*w));
        sig += w * w;
        err += (g - w) * (g - w);
        worst = worst.max((g - w).abs() * 32768.0);
    }
    (10.0 * (sig / err.max(1e-30)).log10(), worst)
}

fn s16(name: &str, channels: usize) -> Vec<f32> {
    let b = std::fs::read(data(name)).unwrap();
    let mono: Vec<f32> = b
        .chunks_exact(2)
        .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
        .collect();
    if channels == 2 {
        mono
    } else {
        mono.iter().flat_map(|&v| [v, v]).collect()
    }
}

/// Opus (CELT stereo, SILK mono) as heard against libopus through ffmpeg, pre-skip and all.
#[test]
fn opus_sound_matches_libopus() {
    for (name, format, reference, channels, min_snr) in [
        ("h264-opus.mkv", Format::Matroska, "h264-opus.pcm", 2, 60.0),
        ("vp9-opus.webm", Format::WebM, "vp9-opus.pcm", 1, 50.0),
    ] {
        let want = s16(reference, channels);
        // Skip the last 20 ms (ffmpeg trims the final packet's padding, we play it).
        let n = want.len() / 2 - 960;
        let got = heard(name, format, n);
        let (snr, worst) = compare(&got, &want[..n * 2]);
        eprintln!("{name}: Opus SNR {snr:.1} dB against libopus, worst {worst:.1} LSB");
        assert!(snr > min_snr, "{name}: SNR {snr:.1} dB");
    }
}

/// The best match of `got` against `want` (interleaved stereo) over frame offsets within
/// ±`span` (containers differ in whether the encoder's priming is trimmed): (SNR dB, worst
/// LSB, offset).
fn best_compare(got: &[f32], want: &[f32], span: usize) -> (f64, f64, isize) {
    let mut best = (f64::MIN, f64::MAX, 0isize);
    for off in -(span as isize)..=span as isize {
        let (g, w) = if off >= 0 {
            (got.get(2 * off as usize..).unwrap_or(&[]), want)
        } else {
            (got, want.get(2 * (-off) as usize..).unwrap_or(&[]))
        };
        let n = g.len().min(w.len()).min(2 * 24_000);
        if n < 2 * 4800 {
            continue;
        }
        let (snr, worst) = compare(&g[..n], &w[..n]);
        if snr > best.0 {
            best = (snr, worst, off);
        }
    }
    best
}

/// AAC-LC sound as ffmpeg decodes it, from MP4 and Matroska (raw blocks with their
/// configuration) and MPEG-TS (ADTS and LOAS/LATM framing).
#[test]
fn aac_sound_matches_ffmpeg() {
    let want = s16("h264-aac.pcm", 2);
    for (name, format, span) in [
        ("h264-aac.mp4", Format::Mp4, 0),
        ("h264-aac.mkv", Format::Matroska, 1024),
        ("h264-aac.ts", Format::MpegTs, 2112),
        ("h264-aac-latm.ts", Format::MpegTs, 2112),
    ] {
        let s = open(name, format);
        let f = s.video().unwrap();
        assert_eq!(f.silent_reason, None, "{name}");
        assert_eq!(f.audio.as_deref(), Some("AAC stereo 48 kHz"), "{name}");
        let got = heard(name, format, want.len() / 2 - 2112);
        let (snr, worst, off) = best_compare(&got, &want, span);
        eprintln!("{name}: AAC SNR {snr:.1} dB against ffmpeg, worst {worst:.2} LSB, offset {off}");
        // the reference is 16-bit: half a step of rounding, about 90 dB here
        assert!(
            snr > 80.0 && worst <= 1.0,
            "{name}: SNR {snr:.1} dB, worst {worst} LSB"
        );
    }
}

/// 5.1 AAC comes to a stereo device folded, every channel heard.
#[test]
fn aac_surround_folds_to_stereo() {
    let s = open("h264-aac51.mp4", Format::Mp4);
    assert_eq!(s.video().unwrap().audio.as_deref(), Some("AAC 5.1 48 kHz"));
    let got = heard("h264-aac51.mp4", Format::Mp4, 24_000);
    let rms = |c: usize| {
        let v: f64 = got
            .iter()
            .skip(c)
            .step_by(2)
            .map(|&x| f64::from(x).powi(2))
            .sum();
        (v / 12_000.0).sqrt()
    };
    let (l, r) = (rms(0), rms(1));
    assert!(l > 0.05 && r > 0.05, "left {l:.3}, right {r:.3}");
    assert!(got.iter().all(|v| v.abs() <= 1.0));
}

/// Blu-ray LPCM in the AVCHD-style stream plays as ffmpeg decodes it.
#[test]
fn lpcm_sound_matches_ffmpeg() {
    let want = s16("interlaced.pcm", 2);
    let n = want.len() / 2 - 480;
    let got = heard("interlaced.m2ts", Format::MpegTs, n);
    let first_bad = got
        .iter()
        .zip(&want)
        .position(|(a, b)| (a - b).abs() * 32768.0 > 1.0);
    if let Some(i) = first_bad {
        eprintln!(
            "first bad sample {i}: got {:?} want {:?}",
            &got[i.saturating_sub(2)..i + 4],
            &want[i.saturating_sub(2)..i + 4]
        );
    }
    let (snr, worst) = compare(&got, &want[..n * 2]);
    eprintln!("LPCM: SNR {snr:.1} dB, worst {worst:.1} LSB");
    assert!(worst <= 1.0, "worst {worst} LSB");
}

/// A/V sync: the picture of the Matroska clip follows its Opus sound's clock from a device
/// running 0.5 % fast, within a frame.
#[test]
fn picture_follows_the_opus_sound() {
    use specular_anim::{LoopMode, Player, PlayerConfig};
    let s = open("h264-opus.mkv", Format::Matroska);
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
    let out = FakeOutput::new(48_000, 2);
    let audio = AudioPlayer::open(
        Arc::clone(s.source()),
        s.movie(),
        first_us,
        Box::new(out.clone()),
    )
    .unwrap();
    p.set_clock(Some(audio.clock()));
    let t0 = Instant::now();
    assert!(p.wait_idle(Duration::from_secs(10)));
    p.tick(t0);
    audio.play(Duration::ZERO);
    std::thread::sleep(Duration::from_millis(50));
    let mut worst = 0i64;
    let mut now = t0;
    for k in 0..70u64 {
        let heard_at = now + Duration::from_millis(20);
        loop {
            let before = out.played.lock().len();
            assert!(out.pull(480, heard_at));
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

/// Timings of plain playback on larger clips made by tools/corpus/video2-large.sh (1080p and
/// 4K VP9, VP8 and H.264 in WebM / Matroska, 1080i AVCHD): opening, first frame, seek latency
/// and decode rate. Skips without them.
#[test]
fn timings_on_large_clips() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/video2/large");
    let Ok(list) = std::fs::read_dir(&dir) else {
        eprintln!("skipped: run tools/corpus/video2-large.sh");
        return;
    };
    let mut names: Vec<PathBuf> = list.flatten().map(|e| e.path()).collect();
    names.sort();
    for path in names {
        let format = match path.extension().and_then(|e| e.to_str()) {
            Some("webm") => Format::WebM,
            Some("mkv") => Format::Matroska,
            Some("m2ts" | "mts" | "ts") => Format::MpegTs,
            _ => continue,
        };
        let t = Instant::now();
        let s = session::open(Arc::new(Source::open(&path).unwrap()), format).unwrap();
        let opened = t.elapsed();
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
            "{}: {}x{} {}, open {opened:?}, first frame {first:?}, {fps:.1} fps, seek median {:?} worst {:?}",
            path.file_name().unwrap().to_string_lossy(),
            s.info().width,
            s.info().height,
            s.video().unwrap().codec,
            seeks[seeks.len() / 2],
            seeks[seeks.len() - 1]
        );
    }
}

/// H.264 in MP4 without `ctts` (some remuxers drop it even with B-frames): the pictures'
/// order counts give the same presentation order the offsets would.
#[test]
fn mp4_h264_without_ctts_orders_by_poc() {
    use specular_video::mp4;
    let data = std::fs::read(data("h264-gop.mp4")).unwrap();
    let order = |m: &mp4::Movie| {
        let s = &m.video.as_ref().unwrap().samples;
        let mut o: Vec<usize> = (0..s.len()).collect();
        o.sort_by_key(|&i| s[i].cts);
        o
    };
    let full = mp4::parse(&data, true).unwrap();
    let want = order(&full);
    assert_ne!(want, (0..want.len()).collect::<Vec<_>>(), "no B-frames");
    let at = data.windows(4).position(|w| w == b"ctts").unwrap();
    let mut bare = data.clone();
    bare[at..at + 4].copy_from_slice(b"free");
    let m = mp4::parse(&bare, true).unwrap();
    assert_eq!(order(&m), want);
    // The same times too: the edit's start (the reorder delay) is kept, so the first frames
    // are not trimmed away.
    let cts = |m: &mp4::Movie| -> Vec<i64> {
        m.video
            .as_ref()
            .unwrap()
            .samples
            .iter()
            .map(|s| s.cts)
            .collect()
    };
    assert_eq!(cts(&m), cts(&full));
}

/// MPEG-TS sound streams (ffmpeg: four MP2 streams, ISO 639 descriptors with `audio_type`
/// from the dispositions): languages and who each is for, as ffprobe reads them; each
/// plays its own tone when picked.
#[test]
fn ts_audio_types() {
    let name = "ts-audio-types.ts";
    let bytes = std::fs::read(data(name)).unwrap();
    let x = specular_video::extras::read(&bytes);
    let got: Vec<_> = x
        .audio
        .iter()
        .map(|t| {
            (
                t.lang.as_deref(),
                t.default,
                t.hearing_impaired,
                t.visual_impaired,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (Some("eng"), true, false, false),
            (Some("eng"), false, false, true),
            (Some("deu"), false, true, false),
            (Some("eng"), false, false, false),
        ]
    );
    let src = Arc::new(Source::open(data(name)).unwrap());
    for (t, hz) in x.audio.iter().zip([440.0, 660.0, 880.0, 990.0]) {
        let movie = specular_video::parse_movie_with(&bytes, true, Some(t.id)).unwrap();
        let a = movie.audio.as_ref().unwrap();
        let (pcm, ch, rate) = specular_video::audio::decode_track(a, &src).unwrap();
        let ch = usize::from(ch);
        let mono: Vec<f32> = pcm.chunks_exact(ch).map(|f| f[0]).collect();
        // the tone from zero crossings, away from the edges
        let mid = &mono[mono.len() / 4..mono.len() * 3 / 4];
        let crossings = mid
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        let f = crossings as f64 * f64::from(rate) / (2.0 * mid.len() as f64);
        assert!((f - hz).abs() < 15.0, "{}: {f:.0} Hz, want {hz}", t.id);
    }
}
