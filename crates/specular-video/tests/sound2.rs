//! AC-3, E-AC-3, Vorbis, Opus and FLAC sound in Matroska, WebM, MP4, transport and programme
//! streams and Ogg; Theora in Ogg; VP8, VP9 and AV1 in IVF (tests/data, made by
//! tests/data/gen_fixtures_sound2.sh with ffmpeg, whose decodes are the oracle): pictures
//! exact, sound against ffmpeg's stereo decode as the player hears it (dither and noise
//! substitution are random in both decoders), facts.

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

fn open(name: &str, format: Format) -> VideoSession {
    let src = Arc::new(Source::open(data(name)).unwrap());
    session::open(src, format).unwrap()
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

/// Every frame against ffmpeg's rawvideo.
fn all_frames_exact(name: &str, format: Format, frames: usize) {
    let s = open(name, format);
    let want = std::fs::read(data(&format!("{}.yuv", name.rsplit_once('.').unwrap().0))).unwrap();
    let mut it = s.video_frames().unwrap();
    let cancel = CancelToken::new();
    let (mut n, mut at, mut bad) = (0, 0, 0);
    while let Some(f) = it.next_frame(&cancel) {
        let got = planes(&f.unwrap().raster);
        if want.get(at..at + got.len()) != Some(&got[..]) {
            bad += 1;
        }
        at += got.len();
        n += 1;
    }
    eprintln!("{name}: {n} frames, {bad} differ from ffmpeg");
    assert_eq!(bad, 0, "{name}: {bad} of {n} frames differ");
    assert_eq!(at, want.len(), "{name}: not as many frames as ffmpeg");
    assert_eq!(n, frames, "{name}");
}

#[test]
fn theora_and_ivf_pictures_match_ffmpeg() {
    all_frames_exact("theora-vorbis.ogv", Format::Ogg, 10);
    all_frames_exact("ivf-vp8.ivf", Format::Ivf, 10);
    all_frames_exact("ivf-vp9.ivf", Format::Ivf, 10);
    all_frames_exact("ivf-av1.ivf", Format::Ivf, 10);
}

/// A Theora decoder whose buffers the memory budget refuses is a memory refusal (retried
/// once memory is free, worded "needs N of free RAM"), not an unsupported video.
#[test]
fn theora_refused_for_memory_says_so() {
    let s = open("theora-vorbis.ogv", Format::Ogg);
    let r = specular_core::budget::with_limit(4096, || s.video_frames().map(|_| ()));
    assert!(
        matches!(r, Err(specular_decode::DecodeError::NeedsMemory(_))),
        "{r:?}"
    );
    assert!(s.video_frames().is_ok());
}

#[test]
fn seeks_land_on_the_right_frame() {
    for (name, format) in [
        ("theora-vorbis.ogv", Format::Ogg),
        ("ivf-vp8.ivf", Format::Ivf),
        ("ivf-vp9.ivf", Format::Ivf),
        ("ivf-av1.ivf", Format::Ivf),
    ] {
        let s = open(name, format);
        let want =
            std::fs::read(data(&format!("{}.yuv", name.rsplit_once('.').unwrap().0))).unwrap();
        let size = want.len() / 10;
        let cancel = CancelToken::new();
        let mut frames = s.video_frames().unwrap();
        for k in [7u32, 2, 9, 0, 5] {
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
fn heard(s: &VideoSession, rate: u32, frames: usize) -> Vec<f32> {
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

/// Sound as heard against ffmpeg's decode: (offset into ffmpeg's in samples, worst
/// difference in 16-bit steps, SNR in dB) over the middle of the clip.
fn sound_against(name: &str, format: Format, rate: u32) -> (usize, f32, f64) {
    let s = open(name, format);
    let mut want = s16(&format!("{}.pcm", name.rsplit_once('.').unwrap().0));
    if s.movie().audio.as_ref().is_some_and(|a| a.channels == 1) {
        // ffmpeg spreads mono at −3 dB to each side; the player plays it at full level
        for v in &mut want {
            *v *= std::f32::consts::SQRT_2;
        }
    }
    let n = want.len() / 2 - rate as usize / 10;
    let got = heard(&s, rate, n);
    // Aligned on the sound's start: the tones are steady, so past their onset every whole
    // number of periods matches as well as the true offset, and random dither picked one
    // of those now and then (dvd-ac3.vob's 500 Hz at 96 kbit/s: 1983 = 255 + 18 periods).
    let lead = (0..rate as usize / 20)
        .min_by_key(|&d| {
            got[..6000]
                .iter()
                .zip(&want[2 * d..])
                .map(|(a, b)| ((a - b).abs() * 32768.0) as i64)
                .sum::<i64>()
        })
        .unwrap();
    let (mut worst, mut sig, mut err) = (0f32, 0f64, 0f64);
    for (a, b) in got
        .iter()
        .zip(&want[2 * lead..])
        .skip(2000)
        .take(n * 2 - rate as usize / 5)
    {
        let d = (a - b).abs() * 32768.0;
        worst = worst.max(d);
        sig += f64::from(*b).powi(2);
        err += f64::from(a - b).powi(2);
    }
    let snr = 10.0 * (sig / err.max(1e-30)).log10();
    eprintln!("{name}: within {worst:.1} steps, {snr:.1} dB of ffmpeg, from sound sample {lead}");
    (lead, worst, snr)
}

#[test]
fn sound_matches_ffmpeg_in_every_container() {
    // (movie, format, rate, the lowest SNR: ffmpeg rounds to 16 bits, its tones are at
    // -18 dBFS: about 80 dB is exact; AC-3 dither and Opus noise are random)
    for (name, format, rate, min_snr) in [
        ("theora-vorbis.ogv", Format::Ogg, 44_100, 70.0),
        ("theora-opus.ogv", Format::Ogg, 48_000, 40.0),
        ("theora-flac.ogv", Format::Ogg, 48_000, 70.0),
        ("vp8-vorbis-mono.webm", Format::WebM, 48_000, 70.0),
        ("vp8-flac.mkv", Format::Matroska, 48_000, 60.0),
        ("mpeg4-flac.mp4", Format::Mp4, 48_000, 70.0),
        ("vp8-ac3.mkv", Format::Matroska, 48_000, 40.0),
        ("mpeg4-ac3.mp4", Format::Mp4, 48_000, 40.0),
        ("mpeg4-eac3.mp4", Format::Mp4, 48_000, 40.0),
        ("mpeg2-ac3.ts", Format::MpegTs, 48_000, 40.0),
        ("mpeg2-eac3.ts", Format::MpegTs, 48_000, 40.0),
        ("dvd-ac3.vob", Format::MpegPs, 48_000, 40.0),
    ] {
        let (lead, _, snr) = sound_against(name, format, rate);
        assert!(snr > min_snr, "{name}: {snr:.1} dB");
        // the picture and the sound start together (within the encoders' delays)
        assert!(
            lead < rate as usize / 40,
            "{name}: from sound sample {lead}"
        );
    }
}

#[test]
fn facts_name_the_sound() {
    for (name, format, codec, audio) in [
        (
            "theora-vorbis.ogv",
            Format::Ogg,
            "Theora",
            "Vorbis stereo 44.1 kHz",
        ),
        (
            "theora-opus.ogv",
            Format::Ogg,
            "Theora",
            "Opus stereo 48 kHz",
        ),
        (
            "theora-flac.ogv",
            Format::Ogg,
            "Theora",
            "FLAC stereo 48 kHz",
        ),
        ("vp8-ac3.mkv", Format::Matroska, "VP8", "AC-3 5.1 48 kHz"),
        (
            "mpeg4-eac3.mp4",
            Format::Mp4,
            "MPEG-4 Part 2",
            "E-AC-3 stereo 48 kHz",
        ),
        (
            "mpeg4-ac3.mp4",
            Format::Mp4,
            "MPEG-4 Part 2",
            "AC-3 stereo 48 kHz",
        ),
        (
            "mpeg4-flac.mp4",
            Format::Mp4,
            "MPEG-4 Part 2",
            "FLAC stereo 48 kHz",
        ),
        (
            "mpeg2-ac3.ts",
            Format::MpegTs,
            "MPEG-2",
            "AC-3 stereo 48 kHz",
        ),
        (
            "mpeg2-eac3.ts",
            Format::MpegTs,
            "MPEG-2",
            "E-AC-3 stereo 48 kHz",
        ),
        (
            "vp8-vorbis-mono.webm",
            Format::WebM,
            "VP8",
            "Vorbis mono 48 kHz",
        ),
        ("vp8-flac.mkv", Format::Matroska, "VP8", "FLAC 5.1 48 kHz"),
        (
            "dvd-ac3.vob",
            Format::MpegPs,
            "MPEG-2",
            "AC-3 stereo 48 kHz",
        ),
    ] {
        let s = open(name, format);
        let f = s.video().unwrap();
        assert_eq!(f.codec, codec, "{name}");
        assert_eq!(f.audio.as_deref(), Some(audio), "{name}");
        assert_eq!(f.silent_reason, None, "{name}");
        assert_eq!(f.unplayable, None, "{name}");
    }
    for (name, codec) in [
        ("ivf-vp8.ivf", "VP8"),
        ("ivf-vp9.ivf", "VP9"),
        ("ivf-av1.ivf", "AV1"),
    ] {
        let s = open(name, Format::Ivf);
        let f = s.video().unwrap();
        assert_eq!(f.codec, codec);
        assert_eq!(f.audio, None);
        assert_eq!(f.frames, 10);
        assert!((f.frame_rate.unwrap() - 25.0).abs() < 0.01, "{name}");
    }
    let s = open("theora-vorbis.ogv", Format::Ogg);
    let f = s.video().unwrap();
    assert_eq!(f.frames, 10);
    assert!((f.frame_rate.unwrap() - 25.0).abs() < 0.01);
}

/// A Theora decoder that does not fit in the memory budget says how much it needs (a memory
/// refusal, retried once RAM is free), not "unsupported".
#[test]
fn theora_over_the_budget_needs_memory() {
    let s = open("theora-vorbis.ogv", Format::Ogg);
    let r = specular_core::budget::with_limit(16 << 10, || s.video_frames().map(drop));
    match r {
        Err(specular_decode::DecodeError::NeedsMemory(n)) => {
            assert!(n.to_string().contains("free RAM"), "{n}")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn movie_loudness_matches_the_ffmpeg_decode() {
    // Info's loudness of a movie's sound: every sample of the track, as measured on
    // ffmpeg's decode of the same (lossless) track.
    let src = Arc::new(Source::open(data("theora-flac.ogv")).unwrap());
    let got = specular_video::sound_view::movie_loudness(&src, None, &CancelToken::new())
        .unwrap()
        .expect("a track");
    let mut meter = specular_video::loudness::Meter::new(48_000, 2);
    meter.push(&s16("theora-flac.pcm"));
    let want = meter.report();
    let (g, w) = (got.integrated.unwrap(), want.integrated.unwrap());
    assert!((g - w).abs() < 0.05, "{g:.2} LUFS, ffmpeg's decode {w:.2}");
    let none = specular_video::sound_view::movie_loudness(
        &Arc::new(Source::open(data("bare.m2v")).unwrap()),
        None,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(none.is_none(), "no sound track");
}
