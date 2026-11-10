//! Against libavcodec: the synthetic streams of `tests/data` (see `gen_fixtures.sh`) decode
//! to its PCM within a few 16-bit steps.

use std::path::Path;

use specular_mpa::Decoder;

/// Decodes `data` in pieces of `piece` bytes.
fn decode(data: &[u8], piece: usize) -> (Vec<f32>, usize) {
    let mut d = Decoder::new();
    let mut out = Vec::new();
    for c in data.chunks(piece) {
        d.decode(c, &mut out).unwrap();
    }
    d.flush(&mut out).unwrap();
    (out, d.header().map_or(0, |h| h.channels()))
}

/// (largest difference in 16-bit steps, signal-to-difference ratio in dB).
fn compare(got: &[f32], want: &[i16]) -> (i32, f64) {
    let mut worst = 0;
    let (mut sig, mut err) = (0f64, 0f64);
    for (g, &w) in got.iter().zip(want) {
        let g = (g * 32768.0).round().clamp(-32768.0, 32767.0) as i32;
        let d = g - i32::from(w);
        worst = worst.max(d.abs());
        sig += f64::from(w).powi(2);
        err += f64::from(d).powi(2);
    }
    (worst, 10.0 * (sig / err.max(1e-9)).log10())
}

#[test]
fn layer2_matches_libavcodec() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    for name in ["s48", "s44", "joint", "m48", "m32", "lsf"] {
        let data = std::fs::read(dir.join(format!("{name}.mp2"))).unwrap();
        let want: Vec<i16> = std::fs::read(dir.join(format!("{name}.s16")))
            .unwrap()
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        for piece in [data.len(), 1000, 77] {
            let (got, _) = decode(&data, piece);
            assert_eq!(got.len(), want.len(), "{name}: sample count");
            let (worst, snr) = compare(&got, &want);
            eprintln!("{name} in pieces of {piece}: within {worst} steps, {snr:.1} dB");
            assert!(
                worst <= 2 && snr > 60.0,
                "{name}: {worst} steps, {snr:.1} dB"
            );
        }
    }
}

/// Layer III: LAME's and Shine's streams (`gen_fixtures_mp3.sh`) at every sampling rate
/// family, block switching, middle/side stereo, linbits tables and the bit reservoir.
#[test]
fn layer3_matches_libavcodec() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let mut failed = Vec::new();
    for name in [
        "j44", "s48", "m32", "vbr", "lsf22", "lsf16", "m8", "s11", "shine",
    ] {
        let data = std::fs::read(dir.join(format!("l3-{name}.mp3"))).unwrap();
        let want: Vec<i16> = std::fs::read(dir.join(format!("l3-{name}.s16")))
            .unwrap()
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        let mut pass = true;
        for piece in [data.len(), 1000, 77] {
            let (got, _) = decode(&data, piece);
            let (worst, snr) = compare(&got, &want);
            if let Some(dir) = std::env::var_os("MPA_DUMP") {
                let b: Vec<u8> = got.iter().flat_map(|v| v.to_le_bytes()).collect();
                std::fs::write(Path::new(&dir).join(format!("{name}.f32")), b).unwrap();
            }
            eprintln!(
                "{name} in pieces of {piece}: {} of {} samples, within {worst} steps, {snr:.1} dB",
                got.len(),
                want.len()
            );
            if got.len() != want.len() || worst > 1 || snr < 70.0 {
                pass = false;
            }
        }
        println!(
            "ORACLE\tSound/MP3\t{name}.mp3\t{}",
            if pass { "pass" } else { "fail" }
        );
        if !pass {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}
