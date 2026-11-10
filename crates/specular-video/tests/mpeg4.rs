//! MPEG-4 Part 2 and H.263 in AVI, Matroska, MP4 and 3GPP (tests/data, made by
//! tests/data/gen_fixtures_mpeg4.sh with ffmpeg, whose single-threaded decodes are the
//! oracle): every frame exact, seeks, MP2 sound in AVI, facts and refusals. With
//! `SPECULAR_VIDEOCODECS_CORPUS` set, the FATE MPEG-4 samples too.

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

fn planes(r: &Raster) -> Vec<u8> {
    assert!(matches!(r.layout, PixelLayout::Yuv(_)), "{:?}", r.layout);
    let mut v = Vec::new();
    for p in &r.planes {
        for row in p.data.chunks(p.stride).take(p.height as usize) {
            v.extend_from_slice(&row[..p.width as usize]);
        }
    }
    v
}

/// (frames, differing frames, bytes) against ffmpeg's rawvideo `want`.
fn frames_against(s: &VideoSession, want: &[u8]) -> (usize, usize, usize) {
    let mut frames = s.video_frames().unwrap();
    frames.set_deinterlace(false);
    let cancel = CancelToken::new();
    let (mut n, mut at, mut bad) = (0, 0, 0);
    while let Some(f) = frames.next_frame(&cancel) {
        let f = f.unwrap();
        let got = planes(&f.raster);
        if at + got.len() > want.len() || got != want[at..at + got.len()] {
            bad += 1;
        }
        at += got.len();
        n += 1;
    }
    (n, bad, at)
}

fn yuv_of(name: &str) -> Vec<u8> {
    let stem = name.rsplit_once('.').unwrap().0;
    std::fs::read(data(&format!("{stem}.yuv"))).unwrap()
}

#[test]
fn mpeg4_and_h263_in_their_containers_match_ffmpeg() {
    for (name, format) in [
        ("xvid-mp2.avi", Format::Avi),
        ("mpeg4-asp.mkv", Format::Matroska),
        ("h263.3gp", Format::ThreeGpp),
    ] {
        let s = open_path(&data(name), format);
        let want = yuv_of(name);
        let (n, bad, at) = frames_against(&s, &want);
        eprintln!("{name}: {n} frames, {bad} differ");
        assert_eq!(bad, 0, "{name}: {bad} of {n} frames differ from ffmpeg");
        assert_eq!(at, want.len(), "{name}: not as many frames as ffmpeg");
        assert_eq!(Some(n as u32), s.animation().and_then(|a| a.frame_count));
    }
}

#[test]
fn seeks_land_on_the_right_frame() {
    for (name, format) in [
        ("xvid-mp2.avi", Format::Avi),
        ("mpeg4-asp.mkv", Format::Matroska),
    ] {
        let s = open_path(&data(name), format);
        let want = yuv_of(name);
        let count = s.animation().unwrap().frame_count.unwrap();
        let size = want.len() / count as usize;
        let cancel = CancelToken::new();
        let mut frames = s.video_frames().unwrap();
        for k in [5u32, 1, 7, 0, 3].map(|k| k % count) {
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

#[test]
fn mp2_sound_in_avi_matches_ffmpeg() {
    let s = open_path(&data("xvid-mp2.avi"), Format::Avi);
    let want: Vec<f32> = std::fs::read(data("xvid-mp2.pcm"))
        .unwrap()
        .chunks_exact(2)
        .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
        .collect();
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
    let t0 = Instant::now();
    let n = want.len() / 2 - 4800;
    let mut k = 0;
    while out.played.lock().len() < n * 2 {
        let before = out.played.lock().len();
        out.pull(480, t0 + Duration::from_millis(10 * k));
        k += 1;
        if out.played.lock()[before..].iter().all(|&v| v == 0.0) && k < 2000 {
            out.played.lock().truncate(before);
            k -= 1;
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    let got = out.played.lock().clone();
    // The first chunk's sound starts with the first picture in AVI.
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
        .take(n * 2 - 4000)
        .map(|(a, b)| (a - b).abs() * 32768.0)
        .fold(0f32, f32::max);
    eprintln!("AVI MP2: within {worst} steps from sound sample {lead}");
    assert!(lead <= 1 && worst <= 2.0, "lead {lead}, worst {worst}");
}

#[test]
fn facts_and_refusals() {
    let s = open_path(&data("xvid-mp2.avi"), Format::Avi);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "MPEG-4 Part 2");
    assert!(f.profile.is_some(), "{:?}", f.profile);
    assert_eq!(f.audio.as_deref(), Some("MP2 stereo 48 kHz"));
    assert_eq!(f.silent_reason, None);
    let s = open_path(&data("h263.3gp"), Format::ThreeGpp);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "H.263");
    assert_eq!(f.profile.as_deref(), Some("H.263 baseline"));
    let s = open_path(&data("msmpeg4.avi"), Format::Avi);
    assert_eq!(
        s.video().unwrap().unplayable.as_deref(),
        Some("Microsoft MPEG-4 (DivX 3) video")
    );
}

/// MPEG-4 Part 2 in Matroska with no video object layer header anywhere (none in
/// CodecPrivate, none before the key frames; each `00 00 01 2x` turned into user data) can't
/// be decoded: it is refused by name instead of playing no frames.
#[test]
fn mpeg4_without_its_vol_header_is_refused_by_name() {
    let mut bytes = std::fs::read(data("mpeg4-asp.mkv")).unwrap();
    let mut n = 0;
    for i in 0..bytes.len() - 3 {
        if bytes[i..i + 3] == [0, 0, 1] && (0x20..=0x2F).contains(&bytes[i + 3]) {
            bytes[i + 3] = 0xB2;
            n += 1;
        }
    }
    assert!(n >= 1);
    let s = session::open(
        Arc::new(Source::from_vec("novol.mkv", bytes)),
        Format::Matroska,
    )
    .unwrap();
    let f = s.video().unwrap();
    assert_eq!(f.codec, "MPEG-4 Part 2");
    assert_eq!(
        f.unplayable.as_deref(),
        Some("MPEG-4 Part 2 video without its stream header (VOL)")
    );
}

/// FATE samples (DivX packed B-frames in AVI, Xvid streams, H.263) against ffmpeg's
/// single-threaded decodes.
#[test]
fn fate_samples_in_their_containers() {
    let Some(dir) = std::env::var_os("SPECULAR_VIDEOCODECS_CORPUS") else {
        return;
    };
    let dir = PathBuf::from(dir).join("fate-mpeg4");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut total = (0, 0);
    let mut paths: Vec<PathBuf> = entries.map(|e| e.unwrap().path()).collect();
    paths.sort();
    for p in paths {
        let format = match p.extension().and_then(|e| e.to_str()) {
            Some("avi") => Format::Avi,
            Some("mp4") => Format::Mp4,
            Some("m4v") | Some("h263") => continue,
            _ => continue,
        };
        let Ok(want) = std::fs::read(p.with_extension("yuv")) else {
            continue;
        };
        let s = open_path(&p, format);
        if let Some(why) = &s.video().unwrap().unplayable {
            eprintln!("{}: refused: {why}", p.display());
            continue;
        }
        let (n, bad, at) = frames_against(&s, &want);
        eprintln!(
            "{}: {bad} of {n} frames differ ({at} bytes vs {})",
            p.display(),
            want.len()
        );
        total.0 += bad + usize::from(at != want.len());
        total.1 += n;
    }
    eprintln!("FATE MPEG-4: {} of {} frames differ", total.0, total.1);
    assert_eq!(total.0, 0);
}
