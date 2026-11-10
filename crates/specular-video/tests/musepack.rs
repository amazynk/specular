//! Musepack sound files (tests/data, made by tests/data/gen_fixtures_musepack.sh): against
//! libmpcdec's output (`.pcm`, its float output truncated to 16 bits), the facts name the
//! stream, and damaged or older streams are refused in words or played as far as they go.

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

/// The track decoded as a player hears it: the edit's start dropped, its length kept.
fn decoded(name: &str) -> (Vec<i32>, u16, u32) {
    decoded_at(&data(name))
}

fn decoded_at(path: &Path) -> (Vec<i32>, u16, u32) {
    let src = Arc::new(Source::open(path).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let t = movie.audio.as_ref().expect("a sound track");
    let (pcm, ch, rate) = audio::decode_track(t, &src).unwrap();
    // The edit in the track's time scale (the rate for .mpc, microseconds in Matroska).
    let skip_frames = (t.edit.start as u64 * u64::from(rate)).div_ceil(u64::from(t.timescale));
    let skip = skip_frames as usize * usize::from(ch);
    let len = (t.edit.length_us.unwrap() * u64::from(rate)).div_ceil(1_000_000) as usize;
    let got = pcm[skip..]
        .iter()
        .take(len * usize::from(ch))
        .map(|&v| ((v * 32768.0) as i32).clamp(-32768, 32767))
        .collect();
    (got, ch, rate)
}

#[test]
fn musepack_matches_mpcdec() {
    for (name, pcm, chans) in [
        ("snd-mpc-mono.mpc", "snd-mpc-mono.pcm", 1),
        ("snd-mpc-stereo.mpc", "snd-mpc-stereo.pcm", 2),
    ] {
        let (got, ch, rate) = decoded(name);
        assert_eq!((ch, rate), (chans, 44100), "{name}");
        let want = oracle(pcm);
        assert_eq!(got.len(), want.len(), "{name}");
        let diff: Vec<i32> = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).collect();
        let differ = diff.iter().filter(|&&d| d != 0).count();
        // libmpcdec filters in single precision, we in double: a sample in a few thousand
        // truncates the other way.
        let pass = diff.iter().all(|&d| d <= 1) && differ * 500 < want.len();
        println!(
            "ORACLE\tSound/Musepack\t{name}\t{}\t{differ} of {} differ by 1",
            if pass { "pass" } else { "fail" },
            want.len()
        );
        assert!(pass, "{name}: {differ} of {} differ", want.len());
    }
}

#[test]
fn facts_name_the_stream() {
    specular_video::install();
    for (name, sound) in [
        ("snd-mpc-mono.mpc", "Musepack SV8 (Radio) mono 44.1 kHz"),
        (
            "snd-mpc-stereo.mpc",
            "Musepack SV8 (Standard) stereo 44.1 kHz",
        ),
    ] {
        let src = Arc::new(Source::open(data(name)).unwrap());
        let s: Arc<dyn Session> = Registry::new().open(src).unwrap();
        assert_eq!(s.info().format, Format::Mpc, "{name}");
        let f = s.video().unwrap();
        assert_eq!(f.audio.as_deref(), Some(sound), "{name}");
        assert_eq!(f.silent_reason, None, "{name}");
        assert!(
            f.duration_us
                .is_some_and(|d| (499_000..=501_000).contains(&d)),
            "{name}: {:?}",
            f.duration_us
        );
        let snd = f.sound.as_ref().expect("sound facts");
        assert!(snd.kbps.is_some(), "{name}");
        let enc = snd.fields.iter().find(|(k, _)| *k == "encoder");
        assert_eq!(enc.map(|e| e.1.as_str()), Some("Musepack 1.30.1"), "{name}");
    }
}

#[test]
fn older_and_damaged_streams() {
    use specular_video::sound_file::{Kind, parse, sniff};
    let d = std::fs::read(data("snd-mpc-stereo.mpc")).unwrap();
    assert_eq!(sniff(&d), Some(Kind::Mpc));
    // SV7 framing with an older version number: refused in words
    let mut sv6 = b"MP+\x06".to_vec();
    sv6.resize(64, 0);
    let err = parse(&sv6, true).unwrap_err();
    assert!(err.to_string().contains("version 6"), "{err}");
    // cut in the middle of its audio: what is there plays, nothing panics
    let cut = &d[..d.len() / 2];
    let m = parse(cut, true).unwrap();
    let t = m.audio.as_ref().unwrap();
    let src = Arc::new(Source::from_vec("cut.mpc", cut.to_vec()));
    let r = audio::decode_track(t, &src);
    assert!(r.is_ok() || matches!(r, Err(audio::Silence::Unsupported(_))));
    // damaged bytes inside the audio: decodes to the end without panicking
    let mut bad = d.clone();
    for i in (60..bad.len()).step_by(97) {
        bad[i] ^= 0x5A;
    }
    if let Ok(m) = parse(&bad, true) {
        let t = m.audio.as_ref().unwrap();
        let src = Arc::new(Source::from_vec("bad.mpc", bad));
        let _ = audio::decode_track(t, &src);
    }
}

/// An EBML element with an 8-byte size.
fn el(id: u32, body: &[u8]) -> Vec<u8> {
    let idb = id.to_be_bytes();
    let skip = idb.iter().position(|&b| b != 0).unwrap_or(3);
    let mut v = idb[skip..].to_vec();
    v.push(0x01);
    v.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
    v.extend_from_slice(body);
    v
}

fn uel(id: u32, x: u64) -> Vec<u8> {
    el(id, &x.to_be_bytes())
}

/// An SV8 file's packets: key, the whole packet, where its body starts in it.
fn sv8_packets(d: &[u8]) -> Vec<([u8; 2], &[u8], usize)> {
    let mut out = Vec::new();
    let mut at = 4;
    while at + 3 <= d.len() {
        let key = [d[at], d[at + 1]];
        let mut p = at + 2;
        let mut size = 0usize;
        loop {
            let b = d[p];
            p += 1;
            size = (size << 7) | usize::from(b & 0x7F);
            if b & 0x80 == 0 {
                break;
            }
        }
        out.push((key, &d[at..at + size], p - at));
        if &key == b"SE" {
            break;
        }
        at += size;
    }
    out
}

/// The SV8 file in Matroska (`A_MPC`): the header packets as the codec private (with the
/// `MPCK` magic or without), each audio packet a block (whole, or only its body).
fn in_matroska(mpc: &[u8], magic: bool, whole: bool) -> Vec<u8> {
    let packets = sv8_packets(mpc);
    let mut private = if magic { b"MPCK".to_vec() } else { Vec::new() };
    for (_, p, _) in packets.iter().take_while(|(k, _, _)| k != b"AP") {
        private.extend_from_slice(p);
    }
    let mut a = uel(0xD7, 1);
    a.extend(uel(0x83, 2));
    a.extend(el(0x86, b"A_MPC"));
    a.extend(el(0x63A2, &private));
    let mut audio = el(0xB5, &44100f64.to_be_bytes());
    audio.extend(uel(0x9F, 2));
    a.extend(el(0xE1, &audio));
    let mut seg = el(0x1549_A966, &uel(0x2AD7B1, 1_000_000));
    seg.extend(el(0x1654_AE6B, &el(0xAE, &a)));
    // One cluster; block times in ms from it.
    let mut cl = uel(0xE7, 0);
    let aps = packets.iter().filter(|(k, _, _)| k == b"AP");
    for (frames, (_, p, body)) in aps.enumerate() {
        let ms = (frames as f64 * 1152.0 * 1000.0 / 44100.0).round() as i16;
        let mut b = vec![0x81];
        b.extend_from_slice(&ms.to_be_bytes());
        b.push(0x80);
        b.extend_from_slice(if whole { p } else { &p[*body..] });
        cl.extend(el(0xA3, &b));
    }
    seg.extend(el(0x1F43_B675, &cl));
    let mut f = el(0x1A45_DFA3, &el(0x4282, b"matroska"));
    f.extend(el(0x1853_8067, &seg));
    f
}

#[test]
fn musepack_in_matroska_plays_as_the_file() {
    let mpc = std::fs::read(data("snd-mpc-stereo.mpc")).unwrap();
    let want = oracle("snd-mpc-stereo.pcm");
    let (direct, _, _) = decoded("snd-mpc-stereo.mpc");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    for (magic, whole) in [(true, true), (false, true), (true, false), (false, false)] {
        let path = dir.join(format!("mpc-in-mkv-{magic}-{whole}.mka"));
        std::fs::write(&path, in_matroska(&mpc, magic, whole)).unwrap();
        let src = Source::open(&path).unwrap();
        let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
        let t = movie.audio.as_ref().unwrap();
        assert_eq!(t.codec.name(), "Musepack SV8 (Standard)");
        let (got, ch, rate) = decoded_at(&path);
        assert_eq!((ch, rate), (2, 44100));
        assert_eq!(got.len(), want.len(), "magic {magic}, whole {whole}");
        let differ = got.iter().zip(&direct).filter(|(a, b)| a != b).count();
        println!(
            "ORACLE\tSound/Musepack in Matroska\tmagic {magic}, whole {whole}\t{}\t{differ} of {} differ from the .mpc",
            if differ == 0 { "pass" } else { "fail" },
            want.len()
        );
        assert_eq!(differ, 0, "the same samples as the .mpc itself");
        drop(src);
        let _ = std::fs::remove_file(&path);
    }
    // A damaged header is said when played, not hidden.
    let mut bad = in_matroska(&mpc, true, true);
    let at = bad.windows(4).position(|w| w == b"MPCK").unwrap() + 10;
    bad[at] ^= 0xFF;
    let movie = specular_video::parse_movie(&bad, true).unwrap();
    let t = movie.audio.as_ref().unwrap();
    let src = Arc::new(Source::from_vec("bad.mka", bad.clone()));
    let e = audio::decode_track(t, &src).unwrap_err();
    assert!(format!("{e:?}").contains("Musepack"), "{e:?}");
}
