//! Speech, instrument and MIDI files: GSM 06.10 (raw and in WAVE, against libgsm's decoding
//! of `specular-gsm`'s fixtures), HTK, signed linear, SoundFont 2, DLS, XI and SFZ banks
//! (their samples listed and played), MIDI songs (built-in sounds and a SoundFont).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::Source;
use specular_decode::{Format, Registry, Session};
use specular_synth::{dls, sf2, smf, xi};
use specular_video::audio;

fn gsm_data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../formats/specular-gsm/tests/data")
        .join(name)
}

/// A scratch folder of this test's own.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("fmt500-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn open(path: &Path) -> Arc<dyn Session> {
    specular_video::install();
    let src = Arc::new(Source::open(path).unwrap());
    Registry::new().open(src).unwrap()
}

/// The whole track decoded: samples, channels, rate.
fn decode(path: &Path) -> (Vec<f32>, u16, u32) {
    let src = Arc::new(Source::open(path).unwrap());
    let movie = specular_video::parse_source(&src, true, None).unwrap();
    audio::decode_track(movie.audio.as_ref().unwrap(), &src).unwrap()
}

fn pcm16(path: &Path) -> Vec<i16> {
    std::fs::read(path)
        .unwrap()
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn to_i16(v: &[f32]) -> Vec<i16> {
    v.iter()
        .map(|&x| (x * 32768.0).round().clamp(-32768.0, 32767.0) as i16)
        .collect()
}

fn fields(s: &Arc<dyn Session>) -> Vec<(&'static str, String)> {
    s.video()
        .and_then(|v| v.sound.clone())
        .map(|f| f.fields)
        .unwrap_or_default()
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

#[test]
fn gsm_raw_and_in_wave_match_libgsm() {
    for (file, oracle, format) in [
        ("sweep.gsm", "sweep.libgsm.pcm", Format::Gsm),
        ("sweep-ms.wav", "sweep-ms.libgsm.pcm", Format::Wave),
    ] {
        let path = gsm_data(file);
        let s = open(&path);
        assert_eq!(s.info().format, format, "{file}");
        let v = s.video().unwrap();
        assert!(
            v.audio.as_deref().unwrap().starts_with("GSM 06.10"),
            "{:?}",
            v.audio
        );
        let (got, ch, rate) = decode(&path);
        assert_eq!((ch, rate), (1, 8000));
        let got = to_i16(&got);
        let want = pcm16(&gsm_data(oracle));
        assert_eq!(got.len(), want.len(), "{file}");
        let diff = got.iter().zip(&want).filter(|(a, b)| a != b).count();
        eprintln!("{file}: {} samples, {diff} differ from libgsm", want.len());
        assert_eq!(diff, 0, "{file}");
    }
}

#[test]
fn htk_and_signed_linear() {
    let dir = scratch("speech");
    let wave: Vec<i16> = (0..1600).map(|i| ((i * 37) % 2000 - 1000) as i16).collect();
    // HTK: 1600 samples at 16 kHz (625 × 100 ns), 2 bytes each, waveform.
    let mut htk = Vec::new();
    htk.extend_from_slice(&1600u32.to_be_bytes());
    htk.extend_from_slice(&625u32.to_be_bytes());
    htk.extend_from_slice(&2u16.to_be_bytes());
    htk.extend_from_slice(&0u16.to_be_bytes());
    for s in &wave {
        htk.extend_from_slice(&s.to_be_bytes());
    }
    let p = dir.join("a.htk");
    std::fs::write(&p, &htk).unwrap();
    assert_eq!(open(&p).info().format, Format::Htk);
    let (got, ch, rate) = decode(&p);
    assert_eq!((ch, rate), (1, 16_000));
    assert_eq!(to_i16(&got), wave);
    // MFCC features: refused in words.
    htk[11] = 6;
    let p = dir.join("b.htk");
    std::fs::write(&p, &htk).unwrap();
    specular_video::install();
    let e = Registry::new()
        .open(Arc::new(Source::open(&p).unwrap()))
        .err()
        .unwrap();
    assert!(e.to_string().contains("MFCC"), "{e}");
    // Signed linear at 16 kHz, by extension.
    let raw: Vec<u8> = wave.iter().flat_map(|s| s.to_le_bytes()).collect();
    let p = dir.join("c.sln16");
    std::fs::write(&p, &raw).unwrap();
    assert_eq!(open(&p).info().format, Format::Sln);
    let (got, _, rate) = decode(&p);
    assert_eq!(rate, 16_000);
    assert_eq!(to_i16(&got), wave);
    // A Visual Studio solution named `.sln` is refused, not played as noise.
    let p = dir.join("d.sln");
    std::fs::write(
        &p,
        b"\xEF\xBB\xBF\r\nMicrosoft Visual Studio Solution File, Format Version 12.00\r\n",
    )
    .unwrap();
    let e = Registry::new()
        .open(Arc::new(Source::open(&p).unwrap()))
        .err()
        .unwrap();
    assert!(e.to_string().contains("Visual Studio"), "{e}");
    let _ = std::fs::remove_dir_all(dir);
}

fn tone(n: usize, period: usize, amp: f32) -> Vec<i16> {
    (0..n)
        .map(|i| ((i as f32 / period as f32 * std::f32::consts::TAU).sin() * amp) as i16)
        .collect()
}

#[test]
fn banks_list_and_play_their_samples() {
    let dir = scratch("banks");
    let a = tone(4410, 100, 12_000.0);
    let b = tone(2205, 50, 8_000.0);
    let files: Vec<(PathBuf, Vec<u8>, Format)> = vec![
        (
            dir.join("t.sf2"),
            sf2::build(
                "Test font",
                &[("Piano", 0, 0), ("Kit", 128, 0)],
                &[("A", 44_100, 60, &a), ("B", 44_100, 72, &b)],
            ),
            Format::SoundFont,
        ),
        (
            dir.join("t.dls"),
            dls::build("Organ", &a, 44_100, 60),
            Format::Dls,
        ),
        (
            dir.join("t.xi"),
            xi::build("Lead", &[("A", true, &a)]),
            Format::Xi,
        ),
    ];
    for (p, bytes, format) in files {
        std::fs::write(&p, &bytes).unwrap();
        let s = open(&p);
        assert_eq!(s.info().format, format, "{}", p.display());
        let f = fields(&s);
        let get = |k: &str| f.iter().find(|(x, _)| *x == k).map(|(_, v)| v.clone());
        assert!(get("instruments").is_some(), "{f:?}");
        assert!(get("samples").is_some(), "{f:?}");
        let (got, ch, rate) = decode(&p);
        assert_eq!((ch, rate), (2, 44_100));
        // The first sample at its own rate: XI plays its C-4 at 8363 Hz, resampled.
        if format != Format::Xi {
            let left: Vec<i16> = to_i16(&got).iter().step_by(2).copied().take(4000).collect();
            let want: Vec<i16> = a
                .iter()
                .take(4000)
                .map(|&v| (f32::from(v) * 0.8) as i16)
                .collect();
            let max = left
                .iter()
                .zip(&want)
                .map(|(x, y)| (x - y).abs())
                .max()
                .unwrap();
            assert!(max <= 2, "{}: {max}", p.display());
        } else {
            assert!(rms(&got) > 0.05);
        }
    }
    let sf = fields(&open(&dir.join("t.sf2")));
    assert!(sf.contains(&("title", "Test font".into())), "{sf:?}");
    assert!(
        sf.iter()
            .any(|(k, v)| *k == "instruments" && v.contains("Kit (drums 0)")),
        "{sf:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sfz_plays_the_samples_beside_it() {
    let dir = scratch("sfz");
    std::fs::create_dir_all(dir.join("samples")).unwrap();
    let a = tone(8000, 80, 10_000.0);
    // A 16-bit mono WAVE at 44.1 kHz.
    let mut wav = b"RIFF".to_vec();
    wav.extend_from_slice(&(36 + 2 * a.len() as u32).to_le_bytes());
    wav.extend_from_slice(
        b"WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xAC\0\0\x88\x58\x01\0\x02\0\x10\0data",
    );
    wav.extend_from_slice(&(2 * a.len() as u32).to_le_bytes());
    for s in &a {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(dir.join("samples/low c.wav"), &wav).unwrap();
    let sfz = "// test\n<control> default_path=samples/\n<region> sample=low c.wav lokey=c3 hikey=b3 pitch_keycenter=c3\n<region> sample=missing.wav key=72\n";
    let p = dir.join("t.sfz");
    std::fs::write(&p, sfz).unwrap();
    let s = open(&p);
    assert_eq!(s.info().format, Format::Sfz);
    let f = fields(&s);
    assert!(
        f.iter()
            .any(|(k, v)| *k == "samples" && v.contains("samples/low c.wav")),
        "{f:?}"
    );
    let (got, ch, rate) = decode(&p);
    assert_eq!((ch, rate), (2, 44_100));
    let left: Vec<i16> = to_i16(&got)
        .iter()
        .step_by(2)
        .copied()
        .take(a.len())
        .collect();
    let max = left
        .iter()
        .zip(&a)
        .map(|(x, y)| (x - y).abs())
        .max()
        .unwrap();
    assert!(max <= 1, "{max}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn midi_plays_with_built_in_sounds_and_a_soundfont() {
    let dir = scratch("midi");
    // A two-second scale.
    let song = smf::build("Scale", 0, &[(60, 0, 480), (64, 480, 480), (67, 960, 960)]);
    let p = dir.join("song.mid");
    std::fs::write(&p, &song).unwrap();
    // Parsed without its name, no SoundFont is looked for: the built-in sounds.
    let src = Arc::new(Source::open(&p).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let t = movie.audio.as_ref().unwrap();
    assert_eq!(t.codec.name(), "MIDI (built-in sounds)");
    // 2 s and the 2 s tail.
    let us = movie.duration_us.unwrap();
    assert!((3_900_000..=4_100_000).contains(&us), "{us}");
    let (pcm, ch, rate) = audio::decode_track(t, &src).unwrap();
    assert_eq!((ch, rate), (2, 44_100));
    assert!(rms(&pcm[..2 * 44_100]) > 0.01);
    assert!(rms(&pcm[pcm.len() - 2000..]) < 0.01);
    // A SoundFont beside it, named like it, is played through.
    let cyc = tone(1000, 100, 15_000.0);
    std::fs::write(
        dir.join("song.sf2"),
        sf2::build("Font", &[("Sine", 0, 0)], &[("sine", 44_100, 69, &cyc)]),
    )
    .unwrap();
    let s = open(&p);
    assert_eq!(s.info().format, Format::Midi);
    let movie = specular_video::parse_source(&src, true, None).unwrap();
    let t = movie.audio.as_ref().unwrap();
    assert_eq!(t.codec.name(), "MIDI (SoundFont)");
    let (pcm, _, _) = audio::decode_track(t, &src).unwrap();
    assert!(rms(&pcm[..22_050]) > 0.01);
    let f = fields(&s);
    assert!(f.contains(&("title", "Scale".into())), "{f:?}");
    assert!(
        f.iter().any(|(k, v)| *k == "midi" && v.contains("3 notes")),
        "{f:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Renders `FMT500_MIDI` through `FMT500_SF2` (or the built-in sounds) to `FMT500_OUT` as
/// 44.1 kHz stereo 16-bit, for comparing with TiMidity++ (`tools/corpus/fmt500-midi-oracle.py`).
#[test]
#[ignore]
fn render_midi_for_the_oracle() {
    let (Some(midi), Some(out)) = (
        std::env::var_os("FMT500_MIDI"),
        std::env::var_os("FMT500_OUT"),
    ) else {
        return;
    };
    let src = Arc::new(Source::open(&midi).unwrap());
    let mut movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let t = movie.audio.as_mut().unwrap();
    if let Some(sf2) = std::env::var_os("FMT500_SF2") {
        let bank = Arc::new(Source::open(sf2).unwrap());
        t.extra.bank = Some(specular_video::mp4::External(bank));
    }
    let start = std::time::Instant::now();
    let (pcm, _, _) = audio::decode_track(t, &src).unwrap();
    eprintln!(
        "rendered {:.1} s in {:?}",
        pcm.len() as f64 / 88_200.0,
        start.elapsed()
    );
    let bytes: Vec<u8> = to_i16(&pcm).iter().flat_map(|s| s.to_le_bytes()).collect();
    std::fs::write(out, bytes).unwrap();
}

#[test]
fn aliases_and_containers_are_claimed() {
    let r = Registry::new();
    let exts = r.extensions();
    for e in [
        "mp1", "mpa", "m1a", "m2a", "f4v", "trp", "tp", "evo", "evob", "dvr-ms", "gsm", "sln",
        "htk", "xi", "sf2", "sfz", "dls", "mid", "midi", "kar", "rmi",
    ] {
        assert!(exts.contains(&e), "{e}");
    }
}
