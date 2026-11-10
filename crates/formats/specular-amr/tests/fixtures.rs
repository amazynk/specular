//! Our decoders against the fixed-point reference (libopencore-amr, the 3GPP code) on the
//! committed fixtures: every mode, mode switching with DTX/SID, and lost, damaged and
//! empty frames. Made by tools/corpus/amr-oracle.py, which records the reference decode
//! as an FNV-1a 64 hash in tests/data/oracle.txt.

use specular_amr::{Codec, Decoder, Frame, RxType, Stream, storage};
use std::path::Path;

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

fn fnv(samples: &[i16]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for s in samples {
        for b in s.to_le_bytes() {
            h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

fn oracle() -> Vec<(String, usize, u64)> {
    String::from_utf8(data("oracle.txt"))
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            (
                f[0].to_string(),
                f[1].parse().unwrap(),
                u64::from_str_radix(f[2], 16).unwrap(),
            )
        })
        .collect()
}

#[test]
fn every_fixture_matches_the_reference_decoder() {
    let cases = oracle();
    assert_eq!(cases.len(), 21);
    let mut bad = Vec::new();
    for (name, frames, want) in &cases {
        let bytes = data(name);
        let mut s = Stream::new(&bytes).unwrap();
        let pcm = s.decode_all();
        assert_eq!(s.stats.frames as usize, *frames, "{name}");
        assert_eq!(pcm.len(), frames * s.info.codec.frame_len(), "{name}");
        if fnv(&pcm) != *want {
            bad.push(name.clone());
        }
    }
    assert!(bad.is_empty(), "differ from the reference: {bad:?}");
}

#[test]
fn stats_name_the_modes_and_frame_kinds() {
    let b = data("nb-122.amr");
    let (info, st) = storage::scan(&b).unwrap();
    assert_eq!(info.codec, Codec::Nb);
    assert_eq!(st.main_mode(), Some(7));
    assert!(!st.mode_switching());
    assert_eq!(Codec::Nb.mode_name(7).as_deref(), Some("12.2 kbit/s"));
    assert_eq!(Codec::Wb.mode_name(8).as_deref(), Some("23.85 kbit/s"));
    assert_eq!(Codec::Wb.mode_name(0).as_deref(), Some("6.6 kbit/s"));

    let b = data("wb-switch-dtx.awb");
    let (info, st) = storage::scan(&b).unwrap();
    assert_eq!(info.codec, Codec::Wb);
    assert!(st.mode_switching());
    assert!(st.sid > 0, "the pause becomes comfort noise");
    assert!(st.no_data > 0);

    let b = data("nb-lost.amr");
    let (_, st) = storage::scan(&b).unwrap();
    assert!(st.no_data >= 4 && st.bad >= 3, "{st:?}");
}

#[test]
fn headers_and_refusals() {
    assert!(storage::probe_file(b"#!AMR\n").is_ok());
    assert!(storage::probe_file(b"#!AMR-WB\n").is_ok());
    let mc = storage::probe_file(b"#!AMR_MC1.0\n\x00\x00\x00\x02").unwrap();
    assert_eq!(mc.channels, 2);
    assert!(matches!(
        storage::probe_file(b"#!AMR_MC1.0\n\x00\x00\x00\x00"),
        Err(specular_amr::Error::Malformed(_))
    ));
    assert!(matches!(
        storage::probe_file(b"#!AMR-WB+\n"),
        Err(specular_amr::Error::Unsupported(_))
    ));
    assert!(storage::probe_file(b"RIFF").is_err());
}

#[test]
fn multichannel_files_decode_each_channel() {
    // two channels made of the same mono frames: both channels equal the mono decode
    let mono = data("nb-795.amr");
    let mut st = Stream::new(&mono).unwrap();
    let want = st.decode_all();
    let mut mc = b"#!AMR_MC1.0\n\x00\x00\x00\x02".to_vec();
    let mut pos = 6;
    while let Some((_, used)) = storage::read_frame(Codec::Nb, &mono[pos..]) {
        mc.extend_from_slice(&mono[pos..pos + used]);
        mc.extend_from_slice(&mono[pos..pos + used]);
        pos += used;
    }
    let mut st = Stream::new(&mc).unwrap();
    let got = st.decode_all();
    assert_eq!(got.len(), want.len() * 2);
    for (i, w) in want.iter().enumerate() {
        assert_eq!((got[2 * i], got[2 * i + 1]), (*w, *w));
    }
}

#[test]
fn truncated_and_garbage_frames_do_not_panic() {
    let b = data("wb-2385.awb");
    for cut in [10, 33, 100, 500, b.len() - 3] {
        let mut s = Stream::new(&b[..cut]).unwrap();
        let pcm = s.decode_all();
        assert_eq!(pcm.len() % 320, 0);
    }
    let mut d = Decoder::new(Codec::Nb);
    let mut out = Vec::new();
    let mut x: u32 = 7;
    let mut junk = vec![0u8; 4096];
    for v in junk.iter_mut() {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
        *v = (x >> 16) as u8;
    }
    d.decode_packet(&junk, &mut out);
    let mut d = Decoder::new(Codec::Wb);
    d.decode_packet(&junk, &mut out);
    for rx in [
        RxType::SpeechBad,
        RxType::SidBad,
        RxType::NoData,
        RxType::SpeechLost,
        RxType::Onset,
    ] {
        let f = Frame::empty(rx, 7);
        d.decode_frame(&f, &mut out);
    }
}

#[test]
fn rtp_octet_aligned_payloads_decode_like_the_file() {
    for (name, codec, magic) in [
        ("nb-switch-dtx.amr", Codec::Nb, 6),
        ("wb-switch-dtx.awb", Codec::Wb, 9),
    ] {
        let b = data(name);
        let mut frames = Vec::new();
        let mut pos = magic;
        while let Some((_, used)) = storage::read_frame(codec, &b[pos..]) {
            frames.push(&b[pos..pos + used]);
            pos += used;
        }
        let mut want = Vec::new();
        let mut d = Decoder::new(codec);
        for f in &frames {
            d.decode_packet(f, &mut want);
        }
        // three frames a packet: CMR 15 (no request), TOCs with F set but on the last
        let mut got = Vec::new();
        let mut d = Decoder::new(codec);
        for chunk in frames.chunks(3) {
            let mut p = vec![0xf0u8];
            for (i, f) in chunk.iter().enumerate() {
                p.push(f[0] | if i + 1 < chunk.len() { 0x80 } else { 0 });
            }
            for f in chunk {
                p.extend_from_slice(&f[1..]);
            }
            assert!(!storage::is_storage_packet(codec, &p));
            assert_eq!(d.decode_packet(&p, &mut got), chunk.len());
        }
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn concealment_fades_a_long_loss() {
    // after 30 lost frames the output is (near) silent
    let b = data("nb-122.amr");
    let mut d = Decoder::new(Codec::Nb);
    let mut out = Vec::new();
    d.decode_packet(&b[6..6 + 20 * 32], &mut out);
    let mut tail = Vec::new();
    for _ in 0..30 {
        d.decode_frame(&Frame::empty(RxType::NoData, 7), &mut tail);
    }
    let last: i64 = tail[tail.len() - 160..]
        .iter()
        .map(|&s| (s as i64).abs())
        .max()
        .unwrap();
    assert!(last < 600, "{last}");
}
