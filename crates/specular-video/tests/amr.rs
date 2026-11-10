//! AMR and AMR-WB sound in 3GP, Matroska and storage files (tests/data, made by
//! tests/data/gen_fixtures_amr.sh): bit-exact against the 3GPP reference decoders'
//! output (`.pcm`), the facts name codec and mode, and the files open as sound files.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::Source;
use specular_decode::{Format, Registry, Session};
use specular_video::audio;

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn oracle(name: &str) -> Vec<i32> {
    std::fs::read(data(name))
        .unwrap()
        .chunks_exact(2)
        .map(|c| i32::from(i16::from_le_bytes([c[0], c[1]])))
        .collect()
}

fn decoded(name: &str) -> (Vec<i32>, u16, u32) {
    let src = Arc::new(Source::open(data(name)).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let t = movie.audio.as_ref().expect("a sound track");
    let (pcm, ch, rate) = audio::decode_track(t, &src).unwrap();
    let got = pcm
        .iter()
        .map(|&v| (f64::from(v) * 32768.0).round().clamp(-32768.0, 32767.0) as i32)
        .collect();
    (got, ch, rate)
}

#[test]
fn amr_in_every_container_is_bit_exact() {
    let mut failed = Vec::new();
    for (name, pcm, rate) in [
        ("amr-nb.3gp", "amr-nb.pcm", 8000),
        ("amr-wb.3gp", "amr-wb.pcm", 16000),
        ("amr-wb.mkv", "amr-wb.pcm", 16000),
        ("amr-wbid.mkv", "amr-wb.pcm", 16000),
        ("snd-amr.amr", "snd-amr.pcm", 8000),
        ("snd-awb.awb", "snd-awb.pcm", 16000),
    ] {
        let (got, ch, r) = decoded(name);
        assert_eq!((ch, r), (1, rate), "{name}");
        let want = oracle(pcm);
        let n = got.len().min(want.len());
        let diff = got[..n]
            .iter()
            .zip(&want[..n])
            .filter(|(a, b)| a != b)
            .count();
        let pass = diff == 0 && n * 10 >= want.len() * 9;
        println!(
            "ORACLE\tSound/AMR\t{name}\t{}",
            if pass { "pass" } else { "fail" }
        );
        if !pass {
            failed.push(format!(
                "{name}: {diff} of {n} samples differ ({} wanted)",
                want.len()
            ));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn facts_name_codec_and_mode() {
    specular_video::install();
    for (name, format, sound) in [
        ("amr-nb.3gp", Format::ThreeGpp, "AMR 12.2 kbit/s mono 8 kHz"),
        (
            "amr-wb.3gp",
            Format::ThreeGpp,
            "AMR-WB 23.85 kbit/s mono 16 kHz",
        ),
        (
            "amr-wb.mkv",
            Format::Matroska,
            "AMR-WB 23.85 kbit/s mono 16 kHz",
        ),
        (
            "amr-wbid.mkv",
            Format::Matroska,
            "AMR-WB 23.85 kbit/s mono 16 kHz",
        ),
        ("snd-amr.amr", Format::Amr, "AMR 12.2 kbit/s mono 8 kHz"),
        (
            "snd-awb.awb",
            Format::Amr,
            "AMR-WB 6.6–23.85 kbit/s mono 16 kHz",
        ),
    ] {
        let src = Arc::new(Source::open(data(name)).unwrap());
        let s: Arc<dyn Session> = Registry::new().open(src).unwrap();
        assert_eq!(s.info().format, format, "{name}");
        let f = s.video().unwrap();
        assert_eq!(f.audio.as_deref(), Some(sound), "{name}");
        assert_eq!(f.silent_reason, None, "{name}");
        if format == Format::Amr {
            let snd = f.sound.as_ref().expect("sound facts");
            assert!(snd.kbps.is_some(), "{name}");
            assert!(
                f.duration_us
                    .is_some_and(|d| (290_000..=310_000).contains(&d)),
                "{name}: {:?}",
                f.duration_us
            );
        }
    }
}

#[test]
fn storage_files_are_recognised_and_odd_ones_refused() {
    use specular_video::sound_file::{Kind, parse, sniff};
    assert_eq!(sniff(b"#!AMR\n\x3c"), Some(Kind::Amr));
    assert_eq!(sniff(b"#!AMR-WB\n"), Some(Kind::Amr));
    let mc = b"#!AMR_MC1.0\n\x00\x00\x00\x02\x3c\x3c".to_vec();
    let m = parse(&mc, true).unwrap();
    assert_eq!(m.audio.as_ref().unwrap().channels, 2);
    let err = parse(b"#!AMR_MC2.0\n\x00\x00\x00\x01", true).unwrap_err();
    assert!(err.to_string().contains("AMR"), "{err}");
}
