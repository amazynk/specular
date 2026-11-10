//! Sound in video: G.711, IMA and Microsoft ADPCM, ALAC and MP3 in AVI, QuickTime, MP4, Matroska,
//! FLV and MPEG streams (tests/data, made by tests/data/gen_fixtures_sound_adpcm_alac_mp3.sh with
//! ffmpeg, whose s16 decodes are the oracle). The lossless and table-driven codings must match
//! ffmpeg sample for sample; MP3 within a 16-bit step or two. Then through the player against a
//! fake device: the sound starts with the picture, and lands where a seek says.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::Source;
use specular_decode::{Format, Session};
use specular_video::audio::{self, AudioPlayer, FakeOutput};
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

fn oracle(name: &str) -> Vec<i16> {
    std::fs::read(data(&format!("{}.pcm", name.rsplit_once('.').unwrap().0)))
        .unwrap()
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

/// ffmpeg's decode as 32-bit samples: its `.s32` when there is one, else its `.pcm`; with
/// the bits that count (16 or 32).
fn oracle32(name: &str) -> (Vec<i32>, u32) {
    let stem = name.rsplit_once('.').unwrap().0;
    match std::fs::read(data(&format!("{stem}.s32"))) {
        Ok(b) => (
            b.chunks_exact(4)
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
            32,
        ),
        Err(_) => (oracle(name).iter().map(|&v| i32::from(v)).collect(), 16),
    }
}

/// (samples compared, how many differ, the largest difference in steps of the oracle's
/// resolution).
fn against(name: &str, format: Format) -> (usize, usize, i64) {
    let s = open(name, format);
    let a = s.movie().audio.as_ref().expect("a sound track");
    let (pcm, ch, rate) = audio::decode_track(a, s.source()).unwrap();
    let (want, bits) = oracle32(name);
    let scale = (1u64 << (bits - 1)) as f64;
    let got: Vec<i64> = pcm
        .iter()
        .map(|&v| (f64::from(v) * scale).round().clamp(-scale, scale - 1.0) as i64)
        .collect();
    let n = got.len().min(want.len());
    let diffs = || {
        got[..n]
            .iter()
            .zip(&want[..n])
            .map(|(a, b)| (a - i64::from(*b)).abs())
    };
    let bad = diffs().filter(|&d| d != 0).count();
    if std::env::var_os("SOUND3_DEBUG").is_some() {
        let first: Vec<(usize, i64, i32)> = (0..n)
            .filter(|&i| got[i] != i64::from(want[i]))
            .take(12)
            .map(|i| (i, got[i], want[i]))
            .collect();
        eprintln!("{name}: first differences {first:?}");
        if let Some(dir) = std::env::var_os("SOUND3_DUMP") {
            let bytes: Vec<u8> = got.iter().flat_map(|&v| (v as i32).to_le_bytes()).collect();
            std::fs::write(Path::new(&dir).join(format!("{name}.s32")), bytes).unwrap();
        }
    }
    let worst = diffs().max().unwrap_or(0);
    eprintln!(
        "{name}: {ch} ch {rate} Hz, {} of ffmpeg's {} samples, {bad} differ (worst {worst})",
        got.len(),
        want.len()
    );
    // The whole stream, give or take ffmpeg's handling of a last part block.
    assert!(
        n + 2048 * usize::from(ch) >= want.len(),
        "{name}: {} of {} samples",
        got.len(),
        want.len()
    );
    (n, bad, worst)
}

fn oracle_line(kind: &str, name: &str, pass: bool) {
    println!(
        "ORACLE\tSound/{kind}\t{name}\t{}",
        if pass { "pass" } else { "fail" }
    );
}

#[test]
fn g711_adpcm_and_alac_match_ffmpeg_exactly() {
    let mut failed = Vec::new();
    for (name, format, kind) in [
        ("ulaw.avi", Format::Avi, "G.711"),
        ("alaw.mov", Format::QuickTime, "G.711"),
        ("flv-alaw.flv", Format::Flv, "G.711"),
        ("ima.avi", Format::Avi, "ADPCM"),
        ("ima4.mov", Format::QuickTime, "ADPCM"),
        ("msadpcm.avi", Format::Avi, "ADPCM"),
        ("msadpcm-mono.mkv", Format::Matroska, "ADPCM"),
        ("alac.mp4", Format::Mp4, "ALAC"),
        ("alac-mono.mkv", Format::Matroska, "ALAC"),
        ("alac24.mov", Format::QuickTime, "ALAC"),
        ("alac51.mov", Format::QuickTime, "ALAC"),
        ("mxf-4mono.mxf", Format::Mxf, "PCM"),
        ("opatom/clip_V1.mxf", Format::Mxf, "PCM"),
    ] {
        let (n, bad, _) = against(name, format);
        oracle_line(kind, name, bad == 0 && n > 1000);
        if bad > 0 || n <= 1000 {
            failed.push(format!("{name}: {bad} of {n} samples differ from ffmpeg"));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

/// MP3 decoded whole against ffmpeg's decode: within a 16-bit step or two (both decoders
/// work in floating point), at the best offset (ffmpeg applies MP4 edit lists).
#[test]
fn mp3_in_every_container_matches_ffmpeg() {
    let mut failed = Vec::new();
    for (name, format) in [
        ("mp3-avi.avi", Format::Avi),
        ("mp3-mp4.mp4", Format::Mp4),
        ("mp3-mkv.mkv", Format::Matroska),
        ("mp3-flv.flv", Format::Flv),
        ("mp3-ts.ts", Format::MpegTs),
        ("mp3-mpg.mpg", Format::MpegPs),
    ] {
        let s = open(name, format);
        let a = s.movie().audio.as_ref().expect("a sound track");
        let (pcm, ch, rate) = audio::decode_track(a, s.source()).unwrap();
        let ch = usize::from(ch);
        let got: Vec<i32> = pcm
            .iter()
            .map(|&v| (v * 32768.0).round().clamp(-32768.0, 32767.0) as i32)
            .collect();
        let want: Vec<i32> = oracle(name).iter().map(|&v| i32::from(v)).collect();
        // ffmpeg may start later (an edit list's priming) or earlier.
        let (lead, worst) = (0..2400usize)
            .flat_map(|d| [d as i64, -(d as i64)])
            .map(|d| {
                let (g, w) = if d >= 0 {
                    (got.get(d as usize * ch..).unwrap_or_default(), &want[..])
                } else {
                    (&got[..], want.get((-d) as usize * ch..).unwrap_or_default())
                };
                let n = g.len().min(w.len());
                let worst = g[..n]
                    .iter()
                    .zip(&w[..n])
                    .map(|(a, b)| (a - b).abs())
                    .max()
                    .unwrap_or(i32::MAX);
                (d, if n * 2 < want.len() { i32::MAX } else { worst })
            })
            .min_by_key(|&(_, w)| w)
            .unwrap();
        eprintln!("{name}: {ch} ch {rate} Hz, within {worst} steps of ffmpeg from {lead}");
        let pass = worst <= 2;
        oracle_line("MP3", name, pass);
        if !pass {
            failed.push(format!("{name}: {worst} steps"));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn facts_name_the_sound() {
    for (name, format, sound) in [
        ("ulaw.avi", Format::Avi, "G.711 µ-law mono 8 kHz"),
        ("alaw.mov", Format::QuickTime, "G.711 A-law stereo 8 kHz"),
        ("ima.avi", Format::Avi, "IMA ADPCM stereo 22.05 kHz"),
        ("ima4.mov", Format::QuickTime, "IMA ADPCM stereo 22.05 kHz"),
        (
            "msadpcm.avi",
            Format::Avi,
            "Microsoft ADPCM stereo 22.05 kHz",
        ),
        ("alac.mp4", Format::Mp4, "ALAC stereo 44.1 kHz"),
        ("alac-mono.mkv", Format::Matroska, "ALAC mono 48 kHz"),
        ("alac51.mov", Format::QuickTime, "ALAC 5.1 48 kHz"),
        ("mp3-avi.avi", Format::Avi, "MP3 stereo 44.1 kHz"),
        ("mp3-mp4.mp4", Format::Mp4, "MP3 stereo 44.1 kHz"),
        ("mp3-flv.flv", Format::Flv, "MP3 mono 22.05 kHz"),
        ("mp3-ts.ts", Format::MpegTs, "MP3 stereo 48 kHz"),
        ("mxf-4mono.mxf", Format::Mxf, "PCM stereo 48 kHz"),
        ("opatom/clip_V1.mxf", Format::Mxf, "PCM stereo 48 kHz"),
    ] {
        let s = open(name, format);
        let f = s.video().unwrap();
        assert_eq!(f.audio.as_deref(), Some(sound), "{name}");
        assert_eq!(f.silent_reason, None, "{name}");
    }
}

/// What a fake stereo device at the track's rate hears, from `at` on the movie's timeline.
fn heard(s: &VideoSession, rate: u32, at: Duration, frames: usize) -> Vec<f32> {
    let first_us = s.video_frames().unwrap().first_frame_us();
    let out = FakeOutput::new(rate, 2);
    let player = AudioPlayer::open(
        Arc::clone(s.source()),
        s.movie(),
        first_us,
        Box::new(out.clone()),
    )
    .unwrap();
    player.seek(at);
    player.play(at);
    let t0 = Instant::now();
    let mut k = 0;
    while out.played.lock().len() < frames * 2 && k < 400 {
        // Each buffer once the decoder has it ready: no silence of a busy machine.
        player.wait_queued(256);
        out.pull(256, t0 + Duration::from_millis(5 * k));
        k += 1;
    }
    let mut v = out.played.lock().clone();
    v.truncate(frames * 2);
    v
}

/// The player's sound from the start and after a seek is ffmpeg's from the same moment
/// (stereo tracks at their own rate: nothing is resampled or folded).
#[test]
fn player_starts_with_the_picture_and_seeks_sample_accurately() {
    for (name, format) in [
        ("ima.avi", Format::Avi),
        ("alaw.mov", Format::QuickTime),
        ("msadpcm.avi", Format::Avi),
        ("alac.mp4", Format::Mp4),
        ("mp3-mp4.mp4", Format::Mp4),
        ("mp3-avi.avi", Format::Avi),
        ("mp3-mkv.mkv", Format::Matroska),
        ("opatom/clip_V1.mxf", Format::Mxf),
    ] {
        let s = open(name, format);
        let a = s.movie().audio.as_ref().unwrap();
        let rate = a.sample_rate;
        let (want, bits) = oracle32(name);
        let scale = (1u64 << (bits - 1)) as f64;
        let want: Vec<f32> = want
            .iter()
            .map(|&v| (f64::from(v) / scale) as f32)
            .collect();
        for at_ms in [0u64, 120, 205, 300] {
            let at = Duration::from_millis(at_ms);
            let n = 800;
            let got = heard(&s, rate, at, n);
            let from = (at_ms * u64::from(rate) / 1000) as usize;
            // The best alignment within a few samples of where the seek asked.
            let wide = if std::env::var_os("SOUND3_DEBUG").is_some() {
                1200
            } else {
                4
            };
            let (lag, err) = (0..=wide)
                .flat_map(|d: i64| [d, -d])
                .map(|d| {
                    let w0 = (from as i64 + d).max(0) as usize * 2;
                    let e: f32 = got
                        .iter()
                        .zip(want.get(w0..).unwrap_or_default())
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0, f32::max);
                    (d, e)
                })
                .reduce(|a, b| if b.1 < a.1 { b } else { a })
                .unwrap();
            eprintln!("{name} at {at_ms} ms: lag {lag} samples, worst {:.4}", err);
            assert!(got.len() == n * 2, "{name}: heard {} values", got.len());
            assert!(lag.abs() <= 1, "{name} at {at_ms} ms: {lag} samples off");
            assert!(err < 1e-3, "{name} at {at_ms} ms: {err}");
        }
    }
}

/// Decoding speed in multiples of real time (`--ignored`; the fixtures over and over).
#[test]
#[ignore]
fn decode_speed() {
    for (name, format) in [
        ("mp3-avi.avi", Format::Avi),
        ("alac.mp4", Format::Mp4),
        ("alac24.mov", Format::QuickTime),
        ("ima.avi", Format::Avi),
        ("msadpcm.avi", Format::Avi),
        ("ulaw.avi", Format::Avi),
    ] {
        let s = open(name, format);
        let a = s.movie().audio.as_ref().unwrap();
        let t0 = Instant::now();
        let mut heard = 0f64;
        for _ in 0..100 {
            let (pcm, ch, rate) = audio::decode_track(a, s.source()).unwrap();
            heard += pcm.len() as f64 / f64::from(ch) / f64::from(rate);
        }
        let took = t0.elapsed().as_secs_f64();
        eprintln!("{name}: {:.0}x real time", heard / took);
    }
}
