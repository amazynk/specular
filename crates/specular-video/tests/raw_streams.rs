//! Raw elementary streams (tests/data/gen_fixtures*.sh: the H.264, HEVC and AV1 fixtures
//! without their containers): recognised through the registry, every frame exact against
//! ffmpeg's decode of the same video in presentation order (B-frames reordered by their
//! order counts), the missing timing said in the facts; and QOA sound.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{Format, Frames, Registry, Session};
use specular_video::session::{self, VideoSession};

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn open(name: &str) -> Arc<dyn Session> {
    specular_video::install();
    let src = Arc::new(Source::open(data(name)).unwrap());
    Registry::new().open(src).unwrap()
}

/// The movie session the registry's opener gives, for its frame decoder.
fn movie(name: &str) -> VideoSession {
    let src = Arc::new(Source::open(data(name)).unwrap());
    session::open(src, Format::VideoStream).unwrap()
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

/// Every frame against ffmpeg's; returns the frame count.
fn all_frames_exact(name: &str, reference: &str) -> usize {
    all_frames_within(name, reference, 0)
}

/// Every frame against ffmpeg's, samples within `tol`; returns the frame count.
fn all_frames_within(name: &str, reference: &str, tol: u8) -> usize {
    assert_eq!(open(name).info().format, Format::VideoStream, "{name}");
    let s = movie(name);
    let want = std::fs::read(data(reference)).unwrap();
    let mut frames = s.video_frames().unwrap();
    let cancel = CancelToken::new();
    let (mut n, mut at, mut bad, mut off) = (0, 0, 0, 0);
    while let Some(f) = frames.next_frame(&cancel) {
        let got = planes(&f.unwrap().raster);
        assert!(
            at + got.len() <= want.len(),
            "{name}: more frames than ffmpeg"
        );
        let diff = got
            .iter()
            .zip(&want[at..at + got.len()])
            .filter(|(a, b)| a.abs_diff(**b) > tol)
            .count();
        off += got
            .iter()
            .zip(&want[at..at + got.len()])
            .filter(|(a, b)| a != b)
            .count();
        bad += diff;
        at += got.len();
        n += 1;
    }
    println!("{name}: {n} frames, {off} bytes differ from ffmpeg, {bad} by more than {tol}");
    assert_eq!(bad, 0, "{name}: {bad} bytes differ from ffmpeg");
    assert_eq!(at, want.len(), "{name}: fewer frames than ffmpeg");
    n
}

#[test]
fn h264_annex_b_with_b_frames() {
    assert_eq!(all_frames_exact("es-bframes.h264", "rot90.yuv"), 50);
    let s = open("es-bframes.h264");
    let v = s.video().unwrap();
    assert!(v.codec.starts_with("H.264"), "{}", v.codec);
    assert!(v.codec.contains("25 fps"), "{}", v.codec);
    assert_eq!(v.frames, 50);
}

#[test]
fn hevc_annex_b_with_b_frames() {
    all_frames_exact("es-bframes.hevc", "hevc.yuv");
}

#[test]
fn av1_obu_stream() {
    assert_eq!(all_frames_exact("es.obu", "av1.yuv"), 10);
}

/// Motion JPEG: opened by name only (the JPEG decoder keeps `.jpg` files of two pictures).
#[test]
fn motion_jpeg_stream() {
    assert_eq!(all_frames_within("es.mjpeg", "mjpeg.yuv", 1), 10);
}

#[test]
fn other_bytes_are_not_streams() {
    for d in [&b"\0\0\0\x01\x80bad"[..], b"\0\0\x01\xBA", b"text"] {
        assert!(specular_video::es::sniff(d).is_none());
    }
}

/// QOA (tests/data/tones.qoa, written by tools/corpus/fmt500d-oracle.py from the
/// specification; tones-qoa.pcm is FFmpeg's native decode): every sample exact.
#[test]
fn qoa_matches_ffmpeg() {
    let path = data("tones.qoa");
    let s = open("tones.qoa");
    assert_eq!(s.info().format, Format::Qoa);
    let v = s.video().unwrap();
    assert!(
        v.audio.as_deref().unwrap().starts_with("QOA stereo 22.05"),
        "{:?}",
        v.audio
    );
    let src = Arc::new(Source::open(&path).unwrap());
    let movie = specular_video::parse_source(&src, true, None).unwrap();
    let (got, ch, rate) =
        specular_video::audio::decode_track(movie.audio.as_ref().unwrap(), &src).unwrap();
    assert_eq!((ch, rate), (2, 22050));
    let want: Vec<i16> = std::fs::read(data("tones-qoa.pcm"))
        .unwrap()
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    let got: Vec<i16> = got
        .iter()
        .map(|&x| (x * 32768.0).round().clamp(-32768.0, 32767.0) as i16)
        .collect();
    assert_eq!(got.len(), want.len());
    let diff = got.iter().zip(&want).filter(|(a, b)| a != b).count();
    println!(
        "tones.qoa: {} samples, {diff} differ from ffmpeg",
        want.len()
    );
    assert_eq!(diff, 0);
}

/// APV through the registry and the player, against FFmpeg's decode of the FATE sample
/// (no APV encoder exists to make a fixture: tools/corpus/fetch-fmt500d.sh, then
/// tools/corpus/fmt500d-oracle.py). Run with `SPECULAR_FMT500D_CORPUS=… cargo test -p
/// specular-video --test raw_streams -- --ignored`.
#[test]
#[ignore]
fn apv_file_matches_ffmpeg() {
    let Some(root) = std::env::var_os("SPECULAR_FMT500D_CORPUS") else {
        eprintln!("SPECULAR_FMT500D_CORPUS not set");
        return;
    };
    let root = Path::new(&root);
    specular_video::install();
    let path = root.join("profile_422-10.apv");
    let src = Arc::new(Source::open(&path).unwrap());
    let s = Registry::new().open(src.clone()).unwrap();
    assert_eq!(s.info().format, Format::Apv);
    let v = s.video().unwrap();
    assert!(v.codec.starts_with("APV"), "{}", v.codec);
    assert_eq!(v.profile.as_deref(), Some("422-10, level 1.1, band 3"));
    let want = std::fs::read(root.join("gen/profile_422-10.yuv")).unwrap();
    let m = session::open(src, Format::Apv).unwrap();
    let mut frames = m.video_frames().unwrap();
    let cancel = CancelToken::new();
    let (mut n, mut at, mut bad) = (0, 0, 0);
    while let Some(f) = frames.next_frame(&cancel) {
        let got = planes(&f.unwrap().raster);
        bad += got
            .iter()
            .zip(&want[at..at + got.len()])
            .filter(|(a, b)| a != b)
            .count();
        at += got.len();
        n += 1;
    }
    println!("profile_422-10.apv: {n} frames, {bad} bytes differ from ffmpeg");
    assert_eq!((n, bad, at), (3, 0, want.len()));
}
