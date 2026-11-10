//! Against libavcodec: the synthetic streams of `tests/data` (see `gen_fixtures.sh`) decode
//! to its PCM within a step or two of 16 bits.

mod ogg;

use std::path::Path;

use specular_vorbis::{Decoder, Ident, split_xiph_laced};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

/// Decodes an Ogg Vorbis file: every frame decoded, the channels and the last granule
/// position (the stream's length; the rest of the last packet is padding).
fn decode(file: &[u8]) -> (Vec<f32>, usize, usize) {
    decode_counting(file).0
}

/// Also counts the packets that involve a short block.
fn decode_counting(file: &[u8]) -> ((Vec<f32>, usize, usize), usize) {
    let packets = ogg::packets(file);
    let mut d = Decoder::new(&packets[0].data, &packets[2].data).unwrap();
    let ch = d.channels();
    let mut out = Vec::new();
    let mut end = 0;
    let long = usize::from(Ident::parse(&packets[0].data).unwrap().blocksize[1]) / 2;
    let mut short = 0;
    for p in &packets[3..] {
        let frames = d.packet_frames(&p.data).unwrap();
        short += usize::from(frames != 0 && frames != long);
        let before = out.len();
        assert_eq!(d.decode(&p.data, &mut out).unwrap(), frames);
        assert_eq!(out.len() - before, frames * ch);
        if p.granule >= 0 {
            end = p.granule as usize;
        }
    }
    assert!(end * ch <= out.len());
    ((out, ch, end), short)
}

/// libavcodec's channel order for Vorbis's 5.1 (FL C FR RL RR LFE): FL FR C LFE RL RR.
const FROM_51: [usize; 6] = [0, 2, 1, 4, 5, 3];

fn reference(name: &str, ch: usize) -> Vec<f32> {
    let raw = data(name);
    let mut v: Vec<f32> = if name.ends_with(".f32") {
        raw.chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    } else {
        raw.chunks_exact(2)
            .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
            .collect()
    };
    if ch == 6 {
        for f in v.chunks_exact_mut(6) {
            let a: [f32; 6] = f.try_into().unwrap();
            for (i, &j) in FROM_51.iter().enumerate() {
                f[i] = a[j];
            }
        }
    }
    v
}

/// (largest difference in 16-bit steps, signal-to-difference ratio in dB). Against 16-bit
/// references ours is rounded to 16 bits first.
fn compare(got: &[f32], want: &[f32], s16: bool) -> (f64, f64) {
    let mut worst = 0f64;
    let (mut sig, mut err) = (0f64, 0f64);
    for (&g, &w) in got.iter().zip(want) {
        let g = if s16 {
            f64::from((g * 32768.0).round().clamp(-32768.0, 32767.0)) / 32768.0
        } else {
            f64::from(g)
        };
        let d = g - f64::from(w);
        worst = worst.max(d.abs() * 32768.0);
        sig += f64::from(w).powi(2);
        err += d * d;
    }
    (worst, 10.0 * (sig / err.max(1e-30)).log10())
}

#[test]
fn matches_libavcodec() {
    for (name, rf, frames) in [
        ("s44q3", "f32", 12348),
        ("m48low", "f32", 24000),
        ("c51", "s16", 7200),
        ("s22", "f32", 11025),
        ("clicks", "s16", 22050),
    ] {
        let (got, ch, end) = decode(&data(&format!("{name}.ogg")));
        // The last granule position is the source's length exactly.
        assert_eq!(end, frames, "{name}: length");
        let want = reference(&format!("{name}.{rf}"), ch);
        // libavcodec returns every decoded frame, except that libavformat sometimes trims
        // the last packet (by its own rule, not to the granule position).
        let (all, ffmpeg) = (got.len() / ch, want.len() / ch);
        assert!(
            ffmpeg == all || (ffmpeg <= end && ffmpeg + 256 > end),
            "{name}: libavcodec {ffmpeg} frames, {all} decoded"
        );
        let n = got.len().min(want.len());
        let (worst, snr) = compare(&got[..n], &want[..n], rf == "s16");
        eprintln!(
            "{name}: {ch} channels, {all} frames decoded, {end} kept, libavcodec {ffmpeg}; \
             within {worst:.3} 16-bit steps, {snr:.1} dB"
        );
        assert!(worst <= 2.0, "{name}: {worst} steps");
        let bar = if rf == "s16" { 90.0 } else { 120.0 };
        assert!(snr > bar, "{name}: {snr:.1} dB");
    }
}

#[test]
fn short_blocks_are_covered() {
    assert!(decode_counting(&data("clicks.ogg")).1 > 10);
}

#[test]
fn headers() {
    let p = ogg::packets(&data("s44q3.ogg"));
    let id = Ident::parse(&p[0].data).unwrap();
    assert_eq!((id.channels, id.sample_rate), (2, 44100));
    assert_eq!(id.blocksize, [256, 2048]);
    assert!(id.bitrate_nominal > 0);
    // Matroska's CodecPrivate: 2, then the Xiph lacing of the first two sizes.
    let mut private = vec![2u8];
    for p in &p[..2] {
        private.extend(std::iter::repeat_n(255, p.data.len() / 255));
        private.push((p.data.len() % 255) as u8);
    }
    for p in &p[..3] {
        private.extend_from_slice(&p.data);
    }
    let [a, b, c] = split_xiph_laced(&private).unwrap();
    assert_eq!((a, b, c), (&p[0].data[..], &p[1].data[..], &p[2].data[..]));
    assert!(split_xiph_laced(&private[..10]).is_none());
    assert!(Decoder::new(&p[0].data, &p[1].data).is_err());
}

#[test]
fn reset_and_empty_packets() {
    let p = ogg::packets(&data("clicks.ogg"));
    let mut d = Decoder::new(&p[0].data, &p[2].data).unwrap();
    let mut out = Vec::new();
    assert_eq!(d.decode(&[], &mut out).unwrap(), 0);
    assert_eq!(d.packet_frames(&[]), Some(0));
    assert_eq!(d.decode(&p[3].data, &mut out).unwrap(), 0);
    assert!(d.decode(&p[4].data, &mut out).unwrap() > 0);
    d.reset();
    assert_eq!(d.packet_frames(&p[5].data), Some(0));
    assert_eq!(d.decode(&p[5].data, &mut out).unwrap(), 0);
    // A header packet is no audio packet.
    assert_eq!(d.packet_frames(&p[0].data), None);
    assert!(d.decode(&p[0].data, &mut out).is_err());
    assert!(d.decode(&p[6].data, &mut out).unwrap() > 0);
}

/// A small deterministic generator.
fn lcg(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state >> 33
}

#[test]
fn damaged_input_does_not_panic() {
    let mut seed = 1u64;
    for name in ["s44q3", "m48low", "c51", "s22", "clicks"] {
        let p = ogg::packets(&data(&format!("{name}.ogg")));
        let mut d = Decoder::new(&p[0].data, &p[2].data).unwrap();
        let mut out = Vec::new();
        for pk in &p[3..] {
            let mut bad = pk.data.clone();
            match lcg(&mut seed) % 4 {
                0 => bad.truncate(lcg(&mut seed) as usize % (bad.len() + 1)),
                1 => {
                    for _ in 0..4 {
                        let i = lcg(&mut seed) as usize % bad.len().max(1);
                        if let Some(b) = bad.get_mut(i) {
                            *b ^= 1 << (lcg(&mut seed) % 8);
                        }
                    }
                }
                2 => bad.iter_mut().for_each(|b| *b = lcg(&mut seed) as u8),
                _ => {}
            }
            let _ = d.packet_frames(&bad);
            let _ = d.decode(&bad, &mut out);
        }
        assert!(out.iter().all(|v| (-1.0..=1.0).contains(v)));
        // Damaged setup headers: refused or decoded, never a panic.
        for _ in 0..300 {
            let mut setup = p[2].data.clone();
            match lcg(&mut seed) % 3 {
                0 => setup.truncate(lcg(&mut seed) as usize % setup.len()),
                _ => {
                    for _ in 0..1 + lcg(&mut seed) % 4 {
                        let i = 7 + lcg(&mut seed) as usize % (setup.len() - 7);
                        setup[i] ^= 1 << (lcg(&mut seed) % 8);
                    }
                }
            }
            if let Ok(mut d) = Decoder::new(&p[0].data, &setup) {
                let mut out = Vec::new();
                for pk in p[3..].iter().take(20) {
                    let _ = d.decode(&pk.data, &mut out);
                }
            }
        }
    }
}
