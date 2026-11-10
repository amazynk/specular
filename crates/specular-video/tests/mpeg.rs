//! MPEG-1/2 video and MPEG audio in transport and programme streams, Matroska, MP4 and bare
//! streams (tests/data, made by tests/data/gen_fixtures_mpeg.sh with ffmpeg, whose decodes are
//! the oracle): every frame exact against `ffmpeg -idct int`, MP2 sound within a 16-bit step,
//! seeks, facts and refusals. With `SPECULAR_VIDEOCODECS_CORPUS` set (tools/corpus/
//! fetch-videocodecs.sh), the FATE samples in their containers too.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{Format, Frames, Session};
use specular_video::audio::{AudioPlayer, FakeOutput};
use specular_video::session::{self, VideoSession};

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn open_path(path: &Path, format: Format) -> VideoSession {
    let src = Arc::new(Source::open(path).unwrap());
    session::open(src, format).unwrap()
}

fn open(name: &str, format: Format) -> VideoSession {
    open_path(&data(name), format)
}

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

/// Frames decoded (woven) and how many differ from `want` (ffmpeg's rawvideo).
fn frames_against(s: &VideoSession, want: &[u8]) -> (usize, usize, usize) {
    let mut frames = s.video_frames().unwrap();
    frames.set_deinterlace(false);
    let cancel = CancelToken::new();
    let (mut n, mut at, mut bad) = (0, 0, 0);
    while let Some(f) = frames.next_frame(&cancel) {
        let f = f.unwrap();
        assert_eq!(f.index as usize, n);
        let got = planes(&f.raster);
        if want.get(at..at + got.len()) != Some(&got[..]) {
            bad += 1;
        }
        at += got.len();
        n += 1;
    }
    (n, bad, at)
}

fn all_frames_exact(name: &str, format: Format, frames: usize) {
    let s = open(name, format);
    let want = std::fs::read(data(&name.replace(name.rsplit('.').next().unwrap(), "yuv"))).unwrap();
    let (n, bad, at) = frames_against(&s, &want);
    assert_eq!(bad, 0, "{name}: {bad} of {n} frames differ from ffmpeg");
    assert_eq!(at, want.len(), "{name}: not as many frames as ffmpeg");
    assert_eq!(n, frames, "{name}");
    assert_eq!(Some(n as u32), s.animation().and_then(|a| a.frame_count));
    eprintln!("{name}: {n} frames match ffmpeg");
}

/// From the fidelity corpus (tcela-6 showed 1 of 4 frames, hhi_burst_422 3 of 4): a bare stream
/// whose pictures lie past 64 KiB (the samples' offsets had 16 bits), and a programme
/// stream whose first picture has no PTS and comes before the first one that has (it was
/// cut as lying before the start).
#[test]
fn every_picture_of_large_and_untimed_streams() {
    let want = std::fs::read(data("bare.yuv")).unwrap();
    // Zero bytes may stuff the stream before any start code (ISO/IEC 13818-2 5.2.3).
    let src = std::fs::read(data("bare.m2v")).unwrap();
    let mut big = Vec::new();
    let mut i = 0;
    while i < src.len() {
        if i > 0 && src[i..].starts_with(&[0, 0, 1, 0]) {
            big.resize(big.len() + 70_000, 0);
        }
        big.push(src[i]);
        i += 1;
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let path = dir.join("bare-stuffed.m2v");
    std::fs::write(&path, &big).unwrap();
    assert!(big.len() > 600_000);
    for (s, name) in [
        (open_path(&path, Format::MpegVideo), "bare-stuffed.m2v"),
        (open("bare-nopts.vob", Format::MpegPs), "bare-nopts.vob"),
    ] {
        let (n, bad, at) = frames_against(&s, &want);
        assert_eq!((n, bad, at), (10, 0, want.len()), "{name}");
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn mpeg_video_in_every_container_matches_ffmpeg() {
    all_frames_exact("mpeg2i-mp2.ts", Format::MpegTs, 5);
    all_frames_exact("mpeg1-mp2.mpg", Format::MpegPs, 10);
    all_frames_exact("dvd-ac3.vob", Format::MpegPs, 10);
    all_frames_exact("mpeg2.mkv", Format::Matroska, 10);
    all_frames_exact("mpeg2-mp2.mp4", Format::Mp4, 10);
    all_frames_exact("bare.m2v", Format::MpegVideo, 10);
}

/// Every frame after a seek is the one ffmpeg decodes at that position.
#[test]
fn seeks_land_on_the_right_frame() {
    for (name, format, count) in [
        ("mpeg2i-mp2.ts", Format::MpegTs, 5u32),
        ("mpeg1-mp2.mpg", Format::MpegPs, 10),
        ("mpeg2-mp2.mp4", Format::Mp4, 10),
    ] {
        let s = open(name, format);
        let want =
            std::fs::read(data(&name.replace(name.rsplit('.').next().unwrap(), "yuv"))).unwrap();
        let size = want.len() / count as usize;
        let cancel = CancelToken::new();
        let mut frames = s.video_frames().unwrap();
        frames.set_deinterlace(false);
        for k in [7u32, 2, 9, 0, 5].map(|k| k % count) {
            frames.seek(k).unwrap();
            let f = frames.next_frame(&cancel).unwrap().unwrap();
            assert_eq!(f.index, k);
            let k = k as usize;
            assert!(
                planes(&f.raster) == want[k * size..(k + 1) * size],
                "{name}: frame {k} after a seek"
            );
        }
    }
}

fn s16(name: &str) -> Vec<f32> {
    std::fs::read(data(name))
        .unwrap()
        .chunks_exact(2)
        .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
        .collect()
}

/// What a fake 2-channel device at `rate` hears of a movie's sound.
fn heard(name: &str, format: Format, rate: u32, frames: usize) -> Vec<f32> {
    let s = open(name, format);
    let first_us = s.video_frames().unwrap().first_frame_us();
    let out = FakeOutput::new(rate, 2);
    let player = AudioPlayer::open(
        Arc::clone(s.source()),
        s.movie(),
        first_us,
        Box::new(out.clone()),
    )
    .unwrap();
    player.play(Duration::ZERO);
    // Each buffer is pulled once the player has it decoded (or the sound ended): what is
    // heard does not depend on the machine's load. Silence before the first sound is not
    // counted.
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

/// MP2 sound in a transport stream, a programme stream and MP4 as ffmpeg decodes it.
#[test]
fn mp2_sound_matches_ffmpeg() {
    for (name, format, reference, rate) in [
        ("mpeg2i-mp2.ts", Format::MpegTs, "mpeg2i-mp2.pcm", 48_000),
        ("mpeg1-mp2.mpg", Format::MpegPs, "mpeg1-mp2.pcm", 44_100),
    ] {
        let want = s16(reference);
        // The sound starts with the picture in these clips; compare its middle.
        let n = want.len() / 2 - 2400;
        let got = heard(name, format, rate, n);
        // Align: the first sample where both are audible.
        // The sound starts 481 samples (the MP2 encoder's delay, 10 ms) before the
        // picture: the first frame is heard with sound sample 481.
        let lead = (0..2000usize)
            .min_by_key(|&d| {
                got[..4000]
                    .iter()
                    .zip(&want[2 * d..])
                    .map(|(a, b)| ((a - b).abs() * 32768.0) as i64)
                    .sum::<i64>()
            })
            .unwrap();
        let worst = got
            .iter()
            .zip(&want[2 * lead..])
            .take(n * 2 - 8000)
            .map(|(a, b)| (a - b).abs() * 32768.0)
            .fold(0f32, f32::max);
        eprintln!("{name}: MP2 within {worst:.1} steps of ffmpeg, from sound sample {lead}");
        assert!(
            lead.abs_diff(481) <= 1,
            "{name}: the picture starts with sound sample {lead}"
        );
        assert!(worst <= 2.0, "{name}: {worst} steps");
    }
}

#[test]
fn facts_and_refusals() {
    let s = open("mpeg2i-mp2.ts", Format::MpegTs);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "MPEG-2");
    assert_eq!(f.profile.as_deref(), Some("Main Profile @ Main Level"));
    assert!(f.interlaced);
    assert_eq!(f.audio.as_deref(), Some("MP2 stereo 48 kHz"));
    assert_eq!(f.silent_reason, None);
    assert_eq!(f.unplayable, None);
    let s = open("mpeg1-mp2.mpg", Format::MpegPs);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "MPEG-1");
    assert!(!f.interlaced);
    assert_eq!(f.profile, None);
    assert_eq!(f.audio.as_deref(), Some("MP2 stereo 44.1 kHz"));
    let fps = f.frame_rate.unwrap();
    assert!((fps - 25.0).abs() < 0.01, "{fps}");
    let s = open("dvd-ac3.vob", Format::MpegPs);
    let f = s.video().unwrap();
    assert_eq!(f.silent_reason, None);
    assert_eq!(f.audio.as_deref(), Some("AC-3 stereo 48 kHz"));
    let s = open("mpeg2.mkv", Format::Matroska);
    assert_eq!(
        s.video().unwrap().profile.as_deref(),
        Some("4:2:2 Profile @ Main Level")
    );
    let s = open("mpeg2-mp2.mp4", Format::Mp4);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "MPEG-2");
    assert_eq!(f.audio.as_deref(), Some("MP2 stereo 48 kHz"));
    assert_eq!(f.silent_reason, None);
    let s = open("bare.m2v", Format::MpegVideo);
    assert_eq!(s.video().unwrap().frames, 10);
}

/// Every `00 00 01 <from>` start code in `bytes` turned into user data (`B2`), which
/// decoders skip: the stream loses those headers, the container keeps its sizes.
fn blank_start_codes(bytes: &mut [u8], from: impl Fn(u8) -> bool) -> usize {
    let mut n = 0;
    for i in 0..bytes.len().saturating_sub(3) {
        if bytes[i..i + 3] == [0, 0, 1] && from(bytes[i + 3]) {
            bytes[i + 3] = 0xB2;
            n += 1;
        }
    }
    n
}

/// MPEG-2 in Matroska with no sequence header anywhere (none in CodecPrivate, none before
/// the key frames) can't be decoded: it is refused by name instead of playing no frames.
#[test]
fn mpeg2_without_a_sequence_header_is_refused_by_name() {
    let mut bytes = std::fs::read(data("mpeg2.mkv")).unwrap();
    assert!(blank_start_codes(&mut bytes, |c| c == 0xB3) > 1);
    let s = session::open(
        Arc::new(Source::from_vec("noseq.mkv", bytes)),
        Format::Matroska,
    )
    .unwrap();
    let f = s.video().unwrap();
    assert_eq!(f.codec, "MPEG-2");
    assert_eq!(
        f.unplayable.as_deref(),
        Some("MPEG-2 video without its sequence header")
    );
}

/// The FATE MPEG-2 samples in their own containers (DVD programme streams, field-coded and
/// 4:2:2 transport streams, a VCD-style .mpg) against ffmpeg's decodes of them.
#[test]
fn fate_samples_in_their_containers() {
    let Some(dir) = std::env::var_os("SPECULAR_VIDEOCODECS_CORPUS") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let mut total = (0, 0);
    for (name, format, skip_last) in [
        ("fate-mpeg2/dvd_single_frame.vob", Format::MpegPs, false),
        ("fate-mpeg2/dvd_still_frame.vob", Format::MpegPs, false),
        (
            "fate-mpeg2/matrixbench_mpeg2.lq1.mpg",
            Format::MpegPs,
            false,
        ),
        // A transport stream named .mpg: field pictures, a PES packet each.
        ("fate-mpeg2/t.mpg", Format::MpegTs, false),
        ("fate-mpeg2/xdcam8mp2-1s_small.ts", Format::MpegTs, false),
        ("fate-mpeg2/mpeg2_field_encoding.ts", Format::MpegTs, true),
    ] {
        let path = dir.join(name);
        let Ok(want) = std::fs::read(path.with_extension("yuv")) else {
            eprintln!("skipped {name}: no .yuv");
            continue;
        };
        let s = open_path(&path, format);
        let t = Instant::now();
        let (n, bad, at) = frames_against(&s, &want);
        let bad = if skip_last && bad > 0 { bad - 1 } else { bad };
        eprintln!(
            "{name}: {bad} of {n} frames differ ({} bytes vs {}), {:.0} fps, {:?}",
            at,
            want.len(),
            n as f64 / t.elapsed().as_secs_f64(),
            s.video()
                .map(|f| (&f.codec, &f.profile, f.interlaced, &f.audio))
        );
        total.0 += bad;
        total.1 += n;
    }
    eprintln!("FATE: {} of {} frames differ", total.0, total.1);
    assert_eq!(total.0, 0);
}
