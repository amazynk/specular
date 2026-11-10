//! DTS sound in Matroska, MP4, MPEG-TS and AVI, and carried as 16-bit PCM (tests/data, made
//! by tests/data/gen_fixtures_dts.sh with ffmpeg, whose decodes are the oracle): within a
//! 16-bit step or two of ffmpeg (both decoders work in floating point), and the facts.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::Source;
use specular_decode::{Format, Session};
use specular_video::audio;
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

fn oracle(name: &str) -> Vec<i32> {
    std::fs::read(data(&format!("{}.pcm", name.rsplit_once('.').unwrap().0)))
        .unwrap()
        .chunks_exact(2)
        .map(|c| i32::from(i16::from_le_bytes([c[0], c[1]])))
        .collect()
}

#[test]
fn dts_in_every_container_matches_ffmpeg() {
    let mut failed = Vec::new();
    for (name, format) in [
        ("dts-mkv.mkv", Format::Matroska),
        ("dts-mp4.mp4", Format::Mp4),
        ("dts-ts.ts", Format::MpegTs),
        ("dts-avi.avi", Format::Avi),
        ("dts-pcm.avi", Format::Avi),
    ] {
        let s = open(name, format);
        let a = s.movie().audio.as_ref().expect("a sound track");
        let (pcm, ch, rate) = audio::decode_track(a, s.source()).unwrap();
        assert_eq!(ch, 2, "{name}");
        let got: Vec<i32> = pcm
            .iter()
            .map(|&v| (v * 32768.0).round().clamp(-32768.0, 32767.0) as i32)
            .collect();
        let want = oracle(name);
        let n = got.len().min(want.len());
        let worst = got[..n]
            .iter()
            .zip(&want[..n])
            .map(|(a, b)| (a - b).abs())
            .max()
            .unwrap_or(i32::MAX);
        eprintln!(
            "{name}: {rate} Hz, {} of ffmpeg's {} samples, within {worst} steps",
            got.len(),
            want.len()
        );
        let pass = worst <= 2 && n * 10 >= want.len() * 9;
        println!(
            "ORACLE\tSound/DTS\t{name}\t{}",
            if pass { "pass" } else { "fail" }
        );
        if !pass {
            failed.push(format!("{name}: {worst} steps over {n} of {}", want.len()));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn facts_name_the_sound() {
    for (name, format, sound) in [
        ("dts-mkv.mkv", Format::Matroska, "DTS stereo 48 kHz"),
        ("dts-mp4.mp4", Format::Mp4, "DTS stereo 48 kHz"),
        ("dts-ts.ts", Format::MpegTs, "DTS stereo 48 kHz"),
        ("dts-avi.avi", Format::Avi, "DTS stereo 44.1 kHz"),
        ("dts-pcm.avi", Format::Avi, "DTS stereo 44.1 kHz"),
        ("dts51.mkv", Format::Matroska, "DTS 5.1 48 kHz"),
    ] {
        let s = open(name, format);
        let f = s.video().unwrap();
        assert_eq!(f.audio.as_deref(), Some(sound), "{name}");
        assert_eq!(f.silent_reason, None, "{name}");
    }
}
