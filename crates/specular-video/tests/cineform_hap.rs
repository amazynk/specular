//! CineForm, HAP, VC-2 / Dirac, Flash Screen Video 1 and 2 and OGM video, opened as
//! sessions: every frame against FFmpeg's decode (tests/data/vg3-*, made by
//! gen_fixtures_cineform_hap.sh), the facts and alpha. CineForm and HAP are compared within a
//! tolerance (FFmpeg rounds CineForm's prescaled wavelets and BC1/BC3 interpolation
//! differently); the others must match exactly.

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
        Some("flv") => Format::Flv,
        Some("ogg" | "ogm") => Format::Ogg,
        _ => Format::QuickTime,
    };
    let src = Arc::new(Source::open(data(name)).unwrap());
    session::open(src, format).unwrap()
}

/// The planes of a frame as samples.
fn planes(r: &Raster) -> Vec<u16> {
    assert!(matches!(r.layout, PixelLayout::Yuv(_)), "{:?}", r.layout);
    let mut v = Vec::new();
    for p in r.planes.iter().take(3) {
        let bps = p.stride / p.width as usize;
        for row in p.data.chunks(p.stride).take(p.height as usize) {
            if bps == 2 {
                for s in row[..p.width as usize * 2].chunks_exact(2) {
                    v.push(u16::from_ne_bytes([s[0], s[1]]));
                }
            } else {
                v.extend(row[..p.width as usize].iter().map(|&b| u16::from(b)));
            }
        }
    }
    v
}

/// Every frame of `name` against FFmpeg's `.yuv` (`bytes` per sample); (frames, differing
/// samples, largest difference).
fn compare(name: &str, bytes: usize) -> (usize, usize, u16) {
    let s = open(name);
    let yuv = name.rsplit_once('.').unwrap().0.to_owned() + ".yuv";
    let raw = std::fs::read(data(&yuv)).unwrap();
    let want: Vec<u16> = if bytes == 2 {
        raw.chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect()
    } else {
        raw.iter().map(|&b| u16::from(b)).collect()
    };
    let mut frames = s.video_frames().unwrap();
    let cancel = CancelToken::new();
    let (mut n, mut at, mut bad, mut max) = (0, 0, 0, 0u16);
    while let Some(f) = frames.next_frame(&cancel) {
        let got = planes(&f.unwrap().raster);
        let w = &want[at..(at + got.len()).min(want.len())];
        assert_eq!(w.len(), got.len(), "{name}: frame {n} size");
        for (a, b) in got.iter().zip(w) {
            if a != b {
                bad += 1;
                max = max.max(a.abs_diff(*b));
            }
        }
        at += got.len();
        n += 1;
    }
    assert_eq!(at, want.len(), "{name}: frame count");
    eprintln!("{name}: {n} frames, {bad} samples differ, max {max}");
    (n, bad, max)
}

fn exact(name: &str, bytes: usize) {
    let (n, bad, _) = compare(name, bytes);
    assert_eq!((n, bad), (3, 0), "{name}");
}

#[test]
fn cineform() {
    let (n, _, max) = compare("vg3-cfhd.mov", 2);
    assert_eq!(n, 3);
    assert!(max <= 2);
    let s = open("vg3-cfhd.mov");
    let v = s.video().unwrap();
    assert_eq!(v.codec, "CineForm");
    assert_eq!(v.profile.as_deref(), Some("YUV 4:2:2, 10-bit"));
    assert_eq!(s.info().bit_depth, 10);
    // RGBA in AVI: alpha kept.
    let s = open("vg3-cfhd-rgba.avi");
    assert!(s.info().has_alpha);
    assert_eq!(
        s.video().unwrap().profile.as_deref(),
        Some("RGBA 4:4:4:4, 12-bit")
    );
    let mut frames = s.video_frames().unwrap();
    assert!(frames.next_frame(&CancelToken::new()).unwrap().is_ok());
}

#[test]
fn hap() {
    for name in ["vg3-hap.mov", "vg3-hapq.mov"] {
        let (n, _, max) = compare(name, 1);
        assert_eq!(n, 3);
        assert!(max <= 2, "{name}");
    }
    assert_eq!(open("vg3-hapq.mov").video().unwrap().codec, "HAP Q");
    let s = open("vg3-hapa.mov");
    assert!(s.info().has_alpha);
    assert_eq!(s.video().unwrap().codec, "HAP Alpha");
    let mut frames = s.video_frames().unwrap();
    assert!(frames.next_frame(&CancelToken::new()).unwrap().is_ok());
}

#[test]
fn vc2_and_dirac() {
    exact("vg3-vc2.mkv", 1);
    exact("vg3-vc2-10.mov", 2);
    exact("vg3-dirac.ogg", 1);
    let s = open("vg3-vc2-10.mov");
    assert_eq!(s.video().unwrap().codec, "VC-2 (Dirac)");
    assert_eq!(
        s.video().unwrap().profile.as_deref(),
        Some("VC-2 high quality, 4:2:2, 10-bit")
    );
}

#[test]
fn flash_screen_video() {
    exact("vg3-flashsv.flv", 1);
    exact("vg3-flashsv2.flv", 1);
    assert_eq!(
        open("vg3-flashsv2.flv").video().unwrap().codec,
        "Flash Screen Video 2"
    );
}

#[test]
fn ogm_video() {
    exact("vg3-ogm.ogm", 1);
    assert_eq!(open("vg3-ogm.ogm").video().unwrap().codec, "MPEG-4 Part 2");
}
