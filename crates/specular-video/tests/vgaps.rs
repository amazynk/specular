//! FFV1, uncompressed video and the old codecs (QuickTime Animation, Cinepak, Microsoft
//! Video 1 and RLE, Apple SMC and RPZA) in AVI, QuickTime and Matroska, opened as sessions:
//! every frame against FFmpeg's decode (tests/data/vg-*, made by gen_fixtures_vgaps.sh),
//! the facts, alpha, and the refusals.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{Format, Frames, Session};
use specular_video::session::{self, VideoSession};

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn open(name: &str) -> VideoSession {
    let format = match name.rsplit('.').next() {
        Some("avi") => Format::Avi,
        Some("mkv") => Format::Matroska,
        Some("mxf") => Format::Mxf,
        _ => Format::QuickTime,
    };
    let src = Arc::new(Source::open(data(name)).unwrap());
    session::open(src, format).unwrap()
}

/// The first `count` planes of a frame, little-endian samples.
fn planes(r: &Raster, count: usize) -> Vec<u8> {
    assert!(matches!(r.layout, PixelLayout::Yuv(_)), "{:?}", r.layout);
    let mut v = Vec::new();
    for p in r.planes.iter().take(count) {
        let bps = p.stride / p.width as usize;
        for row in p.data.chunks(p.stride).take(p.height as usize) {
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

/// Every frame of `name` against FFmpeg's `.yuv`, its first `count` planes (each frame of
/// the oracle has `stride` bytes); the frames that differ and the frame count.
fn against(name: &str, count: usize, stride: Option<usize>) -> (usize, usize) {
    let s = open(name);
    let yuv = name.rsplit_once('.').unwrap().0.to_owned() + ".yuv";
    let want = std::fs::read(data(&yuv)).unwrap();
    let mut frames = s.video_frames().unwrap();
    let cancel = CancelToken::new();
    let (mut n, mut at, mut bad) = (0, 0, 0);
    while let Some(f) = frames.next_frame(&cancel) {
        let got = planes(&f.unwrap().raster, count);
        // RGB555 is widened with its top bits repeated (31 is 255); FFmpeg shifts (31 is
        // 248): the same 5-bit values.
        let same = |w: &[u8]| {
            if name.contains("555") || name.contains("rpza") || name.contains("msvideo1") {
                w.iter().zip(&got).all(|(a, b)| a >> 3 == b >> 3)
            } else {
                w == &got[..]
            }
        };
        if !want.get(at..at + got.len()).is_some_and(same) {
            let w = &want[at..(at + got.len()).min(want.len())];
            let first = got.iter().zip(w).position(|(a, b)| a != b);
            eprintln!("{name}: frame {n} differs, first at byte {first:?}");
            bad += 1;
        }
        at += stride.unwrap_or(got.len());
        n += 1;
    }
    assert_eq!(at, want.len(), "{name}: frame count");
    (bad, n)
}

fn exact(name: &str) {
    let (bad, n) = against(name, 3, None);
    assert_eq!(bad, 0, "{name}: {bad} of {n} frames differ from FFmpeg");
    assert_eq!(n, 3, "{name}");
}

#[test]
fn ffv1_in_every_container() {
    exact("vg-ffv1-420.mkv");
    exact("vg-ffv1-v1.avi");
    exact("vg-ffv1-rgb.mov");
    // Greyscale: the luma plane against FFmpeg's grey.
    let (bad, n) = against("vg-ffv1-gray.mkv", 1, None);
    assert_eq!((bad, n), (0, 3));
    let s = open("vg-ffv1-420.mkv");
    let v = s.video().unwrap();
    assert_eq!(v.codec, "FFV1");
    assert_eq!(
        v.profile.as_deref(),
        Some("version 3, Golomb-Rice, 8-bit 4:2:0, 4 slices, CRC")
    );
    assert_eq!(open("vg-ffv1-v1.avi").info().bit_depth, 10);
}

#[test]
fn ffv1_alpha_is_kept() {
    let s = open("vg-ffv1-alpha.mkv");
    assert!(s.info().has_alpha);
    let mut frames = s.video_frames().unwrap();
    let f = frames.next_frame(&CancelToken::new()).unwrap().unwrap();
    assert!(
        matches!(f.raster.layout, PixelLayout::Rgba16(_)),
        "{:?}",
        f.raster.layout
    );
}

#[test]
fn uncompressed_avi() {
    for name in [
        "vg-raw-i420.avi",
        "vg-raw-bgr24.avi",
        "vg-raw-pal8.avi",
        "vg-raw-555.avi",
        "vg-raw-nv12.avi",
        "vg-raw-yuy2.avi",
        "vg-raw-y41p.avi",
        "vg-raw-yuv4.avi",
        "vg-raw-v410.avi",
        "vg-raw-r10k.avi",
    ] {
        exact(name);
    }
}

#[test]
fn uncompressed_quicktime() {
    for name in [
        "vg-raw-v210.mov",
        "vg-raw-v308.mov",
        "vg-raw-r210.mov",
        "vg-raw-2vuy.mov",
        "vg-raw-yuvs.mov",
        "vg-raw-rgb24.mov",
        "vg-raw-pal8.mov",
        "vg-raw-b48r.mov",
    ] {
        exact(name);
    }
    let s = open("vg-raw-v210.mov");
    assert_eq!(
        s.video().unwrap().profile.as_deref(),
        Some("v210 10-bit 4:2:2")
    );
    assert_eq!(s.info().bit_depth, 10);
    assert!(open("vg-raw-argb.mov").info().has_alpha);
}

#[test]
fn uncompressed_matroska() {
    for name in [
        "vg-raw-i420.mkv",
        "vg-raw-uyvy.mkv",
        "vg-raw-y800.mkv",
        "vg-raw-y42b.mkv",
    ] {
        exact(name);
    }
    assert_eq!(
        open("vg-raw-uyvy.mkv").video().unwrap().codec,
        "uncompressed"
    );
}

#[test]
fn old_codecs() {
    for name in [
        "vg-qtrle24.mov",
        "vg-qtrle-gray.mov",
        "vg-smc.mov",
        "vg-rpza.mov",
        "vg-cinepak.avi",
        "vg-cinepak.mov",
        "vg-msvideo1.avi",
        "vg-msrle.avi",
    ] {
        exact(name);
    }
    let s = open("vg-qtrle-argb.mov");
    assert!(s.info().has_alpha);
    assert_eq!(s.video().unwrap().codec, "QuickTime Animation");
    assert_eq!(
        open("vg-cinepak.avi").video().unwrap().profile.as_deref(),
        Some("24-bit RGB")
    );
}

#[test]
fn seeking_lands_on_the_right_frame() {
    // Inter-coded old codecs and FFV1 with contexts carried over: a seek decodes from the
    // keyframe before. (Not SMC from FFmpeg's encoder: its keyframes use colour caches
    // filled by the frames before.)
    for name in [
        "vg-qtrle24.mov",
        "vg-msrle.avi",
        "vg-ffv1-420.mkv",
        "vg-cinepak.avi",
    ] {
        let s = open(name);
        let yuv = name.rsplit_once('.').unwrap().0.to_owned() + ".yuv";
        let want = std::fs::read(data(&yuv)).unwrap();
        let mut frames = s.video_frames().unwrap();
        frames.seek(2).unwrap();
        let f = frames.next_frame(&CancelToken::new()).unwrap().unwrap();
        let got = planes(&f.raster, 3);
        assert_eq!(&want[2 * got.len()..3 * got.len()], &got[..], "{name}");
    }
}
