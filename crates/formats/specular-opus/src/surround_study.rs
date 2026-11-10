//! A study of the 7.1 fixture (`specular-video/tests/data/snd-opus-71.opus`, libopus at
//! 256 kb/s, one tone per channel) against libopus's decode of it: which streams use which
//! mode, and per channel the SNR, gain and phase (as a delay) of its tone. Black-box: only
//! libopus's decoded output is used.
//!
//! `cargo test -p specular-opus --lib surround_study -- --ignored --nocapture`

use std::path::Path;

use crate::packet::{self, Mode};
use crate::{Decoder, Head};

fn data(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../specular-video/tests/data")
        .join(name)
}

/// The packets of the first logical stream of an Ogg file (RFC 3533), headers included.
pub(crate) fn ogg_packets(b: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    let mut p = 0;
    while p + 27 <= b.len() && &b[p..p + 4] == b"OggS" {
        let n = usize::from(b[p + 26]);
        let lacing = &b[p + 27..p + 27 + n];
        let mut q = p + 27 + n;
        for &l in lacing {
            cur.extend_from_slice(&b[q..q + usize::from(l)]);
            q += usize::from(l);
            if l < 255 {
                out.push(std::mem::take(&mut cur));
            }
        }
        p = q;
    }
    out
}

/// Amplitude and phase (radians) of the `hz` tone in `x` (48 kHz).
fn tone(x: &[f64], hz: f64) -> (f64, f64) {
    let (mut c, mut s) = (0.0, 0.0);
    for (i, v) in x.iter().enumerate() {
        let w = 2.0 * std::f64::consts::PI * hz * i as f64 / 48_000.0;
        c += v * w.cos();
        s += v * w.sin();
    }
    let n = x.len() as f64;
    ((c * c + s * s).sqrt() * 2.0 / n, s.atan2(c))
}

/// Per channel: SNR (dB), gain (dB) and delay (samples).
type PerChannel = Vec<(f64, f64, f64)>;
/// Per stream: each packet's mode and bandwidth.
type Modes = Vec<Vec<(Mode, packet::Bandwidth)>>;

/// Per channel (WAVE order): SNR against libopus, our gain relative to it (dB) and our
/// delay relative to it (48 kHz samples, from the tone's phase); and the modes per stream.
pub(crate) fn study() -> Option<(PerChannel, Modes)> {
    let file = std::fs::read(data("snd-opus-71.opus")).ok()?;
    let reference = std::fs::read(data("snd-opus-71.pcm")).ok()?;
    let packets = ogg_packets(&file);
    let head = Head::parse(&packets[0]).unwrap();
    let mut d = Decoder::new(&head).unwrap();
    let mut pcm = Vec::new();
    let mut modes = vec![Vec::new(); usize::from(head.streams)];
    for p in &packets[2..] {
        let mut pos = 0;
        for (s, m) in modes.iter_mut().enumerate() {
            let parsed = packet::parse(&p[pos..], s + 1 < usize::from(head.streams)).unwrap();
            m.push((parsed.toc.mode, parsed.toc.bandwidth));
            pos += parsed.len;
        }
        d.decode(p, &mut pcm).unwrap();
    }
    let ch = usize::from(head.channels);
    let ours = &pcm[usize::from(head.pre_skip) * ch..];
    let r: Vec<f64> = reference
        .chunks_exact(2)
        .map(|c| f64::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
        .collect();
    // WAVE FL FR FC LFE BL BR SL SR from Vorbis FL FC FR SL SR BL BR LFE.
    let from = [0, 2, 1, 7, 5, 6, 3, 4];
    let hz = [300.0, 500.0, 700.0, 60.0, 900.0, 1100.0, 1300.0, 1500.0];
    let frames = (r.len() / 8).min(ours.len() / ch);
    // Skip the first 20 ms (the encoder's start) and the last 20 ms (padding).
    let span = 960..frames - 960;
    let mut per = Vec::new();
    for c in 0..8 {
        let want: Vec<f64> = span.clone().map(|i| r[i * 8 + c]).collect();
        let got: Vec<f64> = span
            .clone()
            .map(|i| f64::from(ours[i * ch + from[c]]))
            .collect();
        let snr = crate::resample_study::snr(&want, &got);
        let (aw, pw) = tone(&want, hz[c]);
        let (ag, pg) = tone(&got, hz[c]);
        let mut dp = pg - pw;
        while dp > std::f64::consts::PI {
            dp -= 2.0 * std::f64::consts::PI;
        }
        while dp < -std::f64::consts::PI {
            dp += 2.0 * std::f64::consts::PI;
        }
        // A later output has a phase behind: x(t - d) gives phase - w d.
        let delay = -dp * 48_000.0 / (2.0 * std::f64::consts::PI * hz[c]);
        per.push((snr, 20.0 * (ag / aw).log10(), delay));
    }
    Some((per, modes))
}

#[test]
#[ignore = "a study aid"]
fn surround_study() {
    let Some((per, modes)) = study() else {
        return;
    };
    for (s, m) in modes.iter().enumerate() {
        let mut kinds: Vec<(Mode, packet::Bandwidth, usize)> = Vec::new();
        for &(mode, bw) in m {
            match kinds.iter_mut().find(|k| k.0 == mode && k.1 == bw) {
                Some(k) => k.2 += 1,
                None => kinds.push((mode, bw, 1)),
            }
        }
        eprintln!("stream {s}: {kinds:?}");
    }
    let names = ["FL", "FR", "FC", "LFE", "BL", "BR", "SL", "SR"];
    for (c, (snr, gain, delay)) in per.iter().enumerate() {
        eprintln!(
            "{}: SNR {snr:.1} dB, gain {gain:+.4} dB, delay {delay:+.4} samples",
            names[c]
        );
    }
}

/// The 7.1 fixture against libopus channel by channel: the CELT pairs near exact, the
/// hybrid centre and LFE (a 700 Hz and a 60 Hz tone through SILK) as far as our SILK
/// synthesis goes (it is 0.025 dB quiet on that tone with any upsampler).
#[test]
fn surround_channels_match_libopus() {
    let (per, modes) = study().unwrap();
    assert!(modes[..3].iter().flatten().all(|m| m.0 == Mode::Celt));
    assert!(modes[3..].iter().flatten().all(|m| m.0 == Mode::Hybrid));
    for (c, (snr, gain, delay)) in per.iter().enumerate() {
        let min = match c {
            2 => 48.0,
            3 => 53.0,
            _ => 80.0,
        };
        assert!(*snr >= min, "channel {c}: {snr:.1} dB");
        assert!(
            gain.abs() < 0.03 && delay.abs() < 0.05,
            "channel {c}: {gain} dB, {delay} samples"
        );
    }
}
