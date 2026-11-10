//! Professional video: ProRes in QuickTime, Matroska and MXF; MXF (OP1a and OP-Atom, frame-
//! and clip-wrapped) with MPEG-2, D-10, H.264 and PCM (tests/data, made by
//! tests/data/gen_fixtures_provideo.sh with ffmpeg, whose decodes are the oracle): every
//! frame exact, sound within a 16-bit step, the alpha channel, facts and refusals. With
//! `SPECULAR_PROVIDEO_CORPUS` set to a folder of `NAME.mov` + `NAME.yuv` pairs, those too.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{DecodeRequest, Format, Frames, Session};
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
            // Native-endian samples in the raster; ffmpeg's are little-endian.
            if bps == 2 {
                for s in row[..p.width as usize * 2].chunks_exact(2) {
                    v.extend_from_slice(&u16::from_ne_bytes([s[0], s[1]]).to_le_bytes());
                }
            } else {
                v.extend_from_slice(&row[..p.width as usize]);
            }
        }
    }
    v
}

/// Every frame (woven) and how many differ from `want` (ffmpeg's rawvideo).
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

fn all_frames_exact_path(path: &Path, format: Format, frames: Option<usize>) {
    let s = open_path(path, format);
    let want = std::fs::read(path.with_extension("yuv")).unwrap();
    let t = Instant::now();
    let (n, bad, at) = frames_against(&s, &want);
    let name = path.file_name().unwrap().to_string_lossy();
    assert_eq!(bad, 0, "{name}: {bad} of {n} frames differ from ffmpeg");
    assert_eq!(at, want.len(), "{name}: not as many frames as ffmpeg");
    if let Some(f) = frames {
        assert_eq!(n, f, "{name}");
    }
    assert_eq!(Some(n as u32), s.animation().and_then(|a| a.frame_count));
    eprintln!("{name}: {n} frames match ffmpeg ({:?})", t.elapsed());
}

fn all_frames_exact(name: &str, format: Format, frames: usize) {
    all_frames_exact_path(&data(name), format, Some(frames));
}

#[test]
fn prores_in_every_container_matches_ffmpeg() {
    all_frames_exact("prores-hq.mov", Format::QuickTime, 4);
    all_frames_exact("prores-proxy.mov", Format::QuickTime, 3);
    all_frames_exact("prores-xq.mkv", Format::Matroska, 3);
    all_frames_exact("prores-il.mov", Format::QuickTime, 2);
    all_frames_exact("prores-pcm.mxf", Format::Mxf, 4);
    all_frames_exact("prores-atom.mxf", Format::Mxf, 4);
}

#[test]
fn mxf_mpeg2_and_h264_match_ffmpeg() {
    // Long-GOP with B-frames: presented in order through the index's temporal offsets.
    all_frames_exact("mpeg2-pcm.mxf", Format::Mxf, 10);
    all_frames_exact("h264-pcm.mxf", Format::Mxf, 10);
}

/// D-10 frames are compared by their 64-bit FNV-1a hashes (d10.fnv).
#[test]
fn d10_matches_ffmpeg() {
    let s = open("d10.mxf", Format::Mxf);
    let want: Vec<String> = std::fs::read_to_string(data("d10.fnv"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut frames = s.video_frames().unwrap();
    frames.set_deinterlace(false);
    let cancel = CancelToken::new();
    let mut got = Vec::new();
    while let Some(f) = frames.next_frame(&cancel) {
        let p = planes(&f.unwrap().raster);
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for b in p {
            h = (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3);
        }
        got.push(format!("{h:016x}"));
    }
    assert_eq!(got, want);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "MPEG-2");
    assert!(
        f.audio.as_deref().is_some_and(|a| a.starts_with("PCM")),
        "{:?}",
        f.audio
    );
}

/// ProRes 4444 with alpha: straight-alpha RGBA, the alpha as ffmpeg decodes it.
#[test]
fn prores_alpha_is_kept() {
    let s = open("prores-4444a.mov", Format::QuickTime);
    assert!(s.info().has_alpha);
    assert_eq!(s.info().bit_depth, 12);
    assert_eq!(s.video().unwrap().profile.as_deref(), Some("4444, alpha"));
    let want = std::fs::read(data("prores-4444a.yuv")).unwrap();
    let (w, h) = (96usize, 64usize);
    let mut frames = s.video_frames().unwrap();
    let f = frames.next_frame(&CancelToken::new()).unwrap().unwrap();
    assert!(
        matches!(f.raster.layout, PixelLayout::Rgba16(_)),
        "{:?}",
        f.raster.layout
    );
    let px = &f.raster.planes[0];
    let plane = w * h * 2;
    let mut worst = 0i64;
    for y in 0..h {
        for x in 0..w {
            let i = 3 * plane + 2 * (y * w + x);
            let a12 = i64::from(u16::from_le_bytes([want[i], want[i + 1]]));
            let o = y * px.stride + x * 8 + 6;
            let a16 = i64::from(u16::from_ne_bytes([px.data[o], px.data[o + 1]]));
            worst = worst.max((a16 - a12 * 65535 / 4095).abs());
        }
    }
    assert!(worst <= 16, "alpha off by {worst} (16-bit steps)");
    // An opaque 4444 has no alpha.
    let s = open("prores-xq.mkv", Format::Matroska);
    assert!(!s.info().has_alpha);
    assert_eq!(s.video().unwrap().profile.as_deref(), Some("4444 XQ"));
}

fn s16(name: &str) -> Vec<f32> {
    std::fs::read(data(name))
        .unwrap()
        .chunks_exact(2)
        .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
        .collect()
}

/// What a fake 2-channel 48 kHz device hears of a movie's sound (the left channel).
fn heard(name: &str, frames: usize) -> Vec<f32> {
    let s = open(name, Format::Mxf);
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
    // none runs short whatever the machine's load: the result does not depend on timing.
    let t0 = Instant::now();
    let mut k = 0;
    while out.played.lock().len() < frames * 2 {
        let more = player.wait_queued(480);
        out.pull(480, t0 + Duration::from_millis(10 * k));
        k += 1;
        if !more {
            break;
        }
    }
    let v = out.played.lock().clone();
    v.chunks_exact(2).map(|c| c[0]).take(frames).collect()
}

/// AES3 / BWF PCM in the generic container and D-10's AES3 elements, as ffmpeg decodes them.
#[test]
fn mxf_pcm_matches_ffmpeg() {
    for (name, reference) in [
        ("prores-pcm.mxf", "prores-pcm.pcm"),
        ("mpeg2-pcm.mxf", "mpeg2-pcm.pcm"),
        ("d10.mxf", "d10.pcm"),
    ] {
        let path = data(reference);
        if !path.exists() {
            continue;
        }
        let want = s16(reference);
        let n = want.len() - 960;
        let got = heard(name, n);
        assert_eq!(got.len(), n, "{name}");
        // Every buffer is pulled once decoded: the sound matches from its first sample.
        let worst = got
            .iter()
            .zip(&want)
            .map(|(a, b)| ((a - b).abs() * 32768.0).round() as i64)
            .max()
            .unwrap();
        assert!(worst <= 1, "{name}: sound off by {worst} 16-bit steps");
    }
}

#[test]
fn mxf_facts() {
    let s = open("prores-pcm.mxf", Format::Mxf);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "ProRes");
    assert_eq!(f.profile.as_deref(), Some("422 LT"));
    assert_eq!(f.frames, 4);
    assert!(
        (f.frame_rate.unwrap() - 25.0).abs() < 0.01,
        "{:?}",
        f.frame_rate
    );
    assert_eq!(f.audio.as_deref(), Some("PCM mono 48 kHz"));
    assert_eq!(
        (s.info().width, s.info().height, s.info().bit_depth),
        (96, 64, 10)
    );
    let s = open("mpeg2-pcm.mxf", Format::Mxf);
    let f = s.video().unwrap();
    assert_eq!(f.codec, "MPEG-2");
    assert!(f.interlaced);
    let s = open("prores-il.mov", Format::QuickTime);
    let f = s.video().unwrap();
    assert!(f.interlaced);
    assert_eq!(f.profile.as_deref(), Some("422, interlaced"));
    assert!(s.video_frames().unwrap().deinterlacing());
    // The thumbnail frame and the first frame decode.
    for (name, format) in [
        ("prores-atom.mxf", Format::Mxf),
        ("h264-pcm.mxf", Format::Mxf),
        ("prores-4444a.mov", Format::QuickTime),
    ] {
        let s = open(name, format);
        s.decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        s.decode_sampled(256, &CancelToken::new()).unwrap().unwrap();
    }
}

#[test]
fn mxf_codings_without_a_decoder_say_why() {
    let s = open("dnxhr.mxf", Format::Mxf);
    assert_eq!(
        s.video().unwrap().unplayable.as_deref(),
        Some("Avid DNxHD video")
    );
    assert!(s.animation().is_none());
}

/// Seeks land on the frame decoding in order gives (long-GOP MPEG-2 in MXF, intra ProRes).
#[test]
fn mxf_seeks() {
    for (name, k) in [("mpeg2-pcm.mxf", 7u32), ("prores-pcm.mxf", 2)] {
        let s = open(name, Format::Mxf);
        let mut frames = s.video_frames().unwrap();
        frames.set_deinterlace(false);
        let cancel = CancelToken::new();
        let mut seq = Vec::new();
        while let Some(f) = frames.next_frame(&cancel) {
            seq.push(planes(&f.unwrap().raster));
        }
        let mut frames = s.video_frames().unwrap();
        frames.set_deinterlace(false);
        frames.seek(k).unwrap();
        let f = frames.next_frame(&cancel).unwrap().unwrap();
        assert_eq!(f.index, k);
        assert!(
            planes(&f.raster) == seq[k as usize],
            "{name}: frame {k} after a seek"
        );
    }
}

/// Larger clips (4K, real cameras): `SPECULAR_PROVIDEO_CORPUS` holds `NAME.{mov,mxf,mkv}`
/// with ffmpeg's `NAME.yuv` beside them.
#[test]
fn corpus_matches_ffmpeg() {
    let Some(dir) = std::env::var_os("SPECULAR_PROVIDEO_CORPUS") else {
        return;
    };
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.with_extension("yuv").exists())
        .collect();
    paths.sort();
    for p in paths {
        let format = match p.extension().and_then(|e| e.to_str()) {
            Some("mov") => Format::QuickTime,
            Some("mxf") => Format::Mxf,
            Some("mkv") => Format::Matroska,
            _ => continue,
        };
        if open_path(&p, format).info().has_alpha {
            // Alpha frames come as RGBA (prores_alpha_is_kept covers them).
            continue;
        }
        all_frames_exact_path(&p, format, None);
    }
}
