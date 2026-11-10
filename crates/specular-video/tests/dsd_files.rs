//! DSD (1-bit) WavPack (tests/data/snd-dsd.wv: 0.1 s of DSD64 stereo, "high" mode, made by
//! `tools/corpus/dsd-samples.py OUT fixtures`): recognised with its 1-bit facts and played as
//! 88.2 kHz PCM matching snd-dsd.pcm (numpy's run of the same decimation filter).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::Source;
use specular_decode::{Format, Registry};
use specular_video::audio;

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

#[test]
fn recognised_with_dsd_facts() {
    specular_video::install();
    let src = Arc::new(Source::open(data("snd-dsd.wv")).unwrap());
    let s = Registry::new().open(src.clone()).unwrap();
    assert_eq!(s.info().format, Format::WavPack);
    let v = s.video().expect("sound facts");
    assert!(v.silent_reason.is_none(), "{:?}", v.silent_reason);
    assert_eq!(v.sound.as_ref().unwrap().bits, Some(1));
    assert!(
        v.duration_us
            .is_some_and(|d| (99_000..101_000).contains(&d)),
        "{:?}",
        v.duration_us
    );
    let audio = v.audio.as_deref().unwrap_or_default();
    assert!(audio.contains("88.2 kHz"), "{audio}");
    let movie = specular_video::parse_movie(src.bytes(), false).unwrap();
    assert_eq!(
        movie.audio.unwrap().codec.name(),
        "WavPack DSD64 (2.8224 MHz, 282240 samples)"
    );
}

#[test]
fn played_as_the_filters_pcm() {
    let src = Arc::new(Source::open(data("snd-dsd.wv")).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let t = movie.audio.clone().unwrap();
    let (got, ch, rate) = audio::decode_track(&t, &src).unwrap();
    assert_eq!((ch, rate), (2, 88_200));
    let want: Vec<f32> = std::fs::read(data("snd-dsd.pcm"))
        .unwrap()
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    assert_eq!(got.len(), want.len());
    let diff = got
        .iter()
        .zip(&want)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    assert!(diff < 2e-5, "{diff}");
}
