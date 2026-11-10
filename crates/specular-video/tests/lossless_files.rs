//! WavPack and Monkey's Audio sound files (tests/data/snd-wv*, snd-ape*, made by
//! gen_fixtures_lossless.sh with the reference coders): recognised, opened with their facts,
//! decoded sample-exact against the reference decoders; a hybrid WavPack file with and
//! without its `.wvc` correction file; a lone correction file refused in words.

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

fn pcm(name: &str) -> Vec<i32> {
    std::fs::read(data(name))
        .unwrap()
        .chunks_exact(2)
        .map(|c| i32::from(i16::from_le_bytes([c[0], c[1]])))
        .collect()
}

/// The file's sound decoded through the player's decoder, as 16-bit values; with the
/// correction file beside it when `correction`.
fn decoded(name: &str, correction: bool) -> Vec<i32> {
    let src = Arc::new(Source::open(data(name)).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let mut t = movie.audio.clone().unwrap();
    if correction {
        t.extra.correction = specular_video::session::wavpack_correction(&data(name));
        assert!(t.extra.correction.is_some());
    }
    let (out, ch, rate) = audio::decode_track(&t, &src).unwrap();
    assert_eq!((ch, rate), (2, 44100), "{name}");
    out.iter()
        .map(|&v| (f64::from(v) * 32768.0).round() as i32)
        .collect()
}

#[test]
fn recognised_with_facts() {
    specular_video::install();
    for (name, format, codec) in [
        ("snd-wv.wv", Format::WavPack, "WavPack"),
        ("snd-wvh.wv", Format::WavPack, "WavPack hybrid"),
        ("snd-ape.ape", Format::Ape, "Monkey's Audio (extra high)"),
    ] {
        let src = Arc::new(Source::open(data(name)).unwrap());
        let s = Registry::new().open(src.clone()).unwrap();
        assert_eq!(s.info().format, format, "{name}");
        let v = s.video().expect("sound facts");
        assert!(v.silent_reason.is_none(), "{name}: {:?}", v.silent_reason);
        let sound = v.sound.as_ref().unwrap();
        assert_eq!(sound.bits, Some(16), "{name}");
        assert!(
            v.duration_us
                .is_some_and(|d| (240_000..260_000).contains(&d)),
            "{name}: {:?}",
            v.duration_us
        );
        let movie = specular_video::parse_movie(src.bytes(), false).unwrap();
        assert_eq!(movie.audio.unwrap().codec.name(), codec);
    }
}

#[test]
fn sample_exact_against_the_reference_decoders() {
    for (name, with, reference) in [
        ("snd-wv.wv", false, "snd-wv.pcm"),
        ("snd-wvh.wv", true, "snd-wvh.pcm"),
        ("snd-wvh.wv", false, "snd-wvh-lossy.pcm"),
        ("snd-ape.ape", false, "snd-ape.pcm"),
    ] {
        let got = decoded(name, with);
        let want = pcm(reference);
        assert_eq!(got.len(), want.len(), "{name} ({reference})");
        let diff = got.iter().zip(&want).filter(|(a, b)| a != b).count();
        assert_eq!(diff, 0, "{name} ({reference}): {diff} samples differ");
    }
    // The lossy decode really differs from the lossless one.
    assert_ne!(pcm("snd-wvh.pcm"), pcm("snd-wvh-lossy.pcm"));
}

#[test]
fn a_lone_correction_file_is_refused_in_words() {
    let d = std::fs::read(data("snd-wvh.wvc")).unwrap();
    let e = specular_wavpack::info(&d).unwrap_err();
    assert!(e.to_string().contains("open the .wv file beside it"), "{e}");
}
