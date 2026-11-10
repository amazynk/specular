//! EVRC against FFmpeg's floating-point `evrc` decoder (system ffmpeg 8.0.1).
//!
//! - `fixture`: the committed `tests/data/mixed.qcp` (made by tools/corpus/evrc-fixtures.py)
//!   against FFmpeg's decodes stored beside it, with its postfilter (`mixed.ffmpeg.s16`)
//!   and without (`mixed.nopf.s16`).
//! - `corpus` (ignored): every EVRC `.qcp` under `$SPECULAR_WTV_CORPUS` (default
//!   target/corpus/wtv1, fetched by tools/corpus/fetch-wtv-qcelp.sh), decoded by
//!   FFmpeg at run time (float output):
//!   `cargo test -p specular-evrc --test oracle -- --ignored --nocapture`
//!
//! FFmpeg is a separate float implementation of C.S0014, so the comparison is by SNR and
//! largest difference. Without the postfilter, rate 1 and 1/2 frames agree to within float
//! rounding except right after rate 1/8 frames: those are noise from a generator the
//! specification leaves unsynchronised, so they are measured apart. The postfilter is
//! optional in C.S0014 (§1.1) and FFmpeg's differs from §5.4 in some subframes, so with it
//! the agreement is looser. FFmpeg outputs nothing for blank and erased packets: our frames
//! for them are left out of the comparison.

use std::path::{Path, PathBuf};

use specular_evrc::{Decoder, FRAME_SAMPLES, packet_len};

const EVRC_GUID: [u8; 16] = [
    0x8d, 0xd4, 0x89, 0xe6, 0x76, 0x90, 0xb5, 0x46, 0x91, 0xef, 0x73, 0x6a, 0x51, 0x00, 0xce, 0xb4,
];

/// The packets of a QCP file (RIFF `QLCM`) and whether its codec is EVRC.
fn qcp_packets(d: &[u8]) -> Option<(bool, Vec<&[u8]>)> {
    if d.len() < 12 || &d[..4] != b"RIFF" || &d[8..12] != b"QLCM" {
        return None;
    }
    let mut at = 12;
    let mut evrc = false;
    while at + 8 <= d.len() {
        let id = &d[at..at + 4];
        let n = u32::from_le_bytes(d[at + 4..at + 8].try_into().ok()?) as usize;
        let body = d.get(at + 8..(at + 8 + n).min(d.len()))?;
        if id == b"fmt " && body.len() >= 18 {
            evrc = body[2..18] == EVRC_GUID;
        }
        if id == b"data" {
            let mut packets = Vec::new();
            let mut i = 0;
            while i < body.len() {
                let len = packet_len(body[i])?;
                packets.push(&body[i..(i + len).min(body.len())]);
                i += len;
            }
            return Some((evrc, packets));
        }
        at += 8 + n + (n & 1);
    }
    None
}

/// Our decode in 16-bit units, without the frames of one-byte packets (blank, erased).
fn ours(packets: &[&[u8]], postfilter: bool) -> Vec<f32> {
    let mut d = Decoder::new();
    d.set_postfilter(postfilter);
    let mut out = Vec::with_capacity(packets.len() * FRAME_SAMPLES);
    let mut frame = Vec::with_capacity(FRAME_SAMPLES);
    for p in packets {
        frame.clear();
        d.decode(p, &mut frame);
        if p.len() > 1 {
            out.extend(frame.iter().map(|v| v * 32768.0));
        }
    }
    out
}

/// A frame FFmpeg outputs: its rate byte, and whether it is "settled": rate 1 or 1/2, not
/// within two frames after a rate 1/8 frame (whose noise is unsynchronised) nor within ten
/// after a blank or erased packet (FFmpeg drops those, so it never conceals them).
struct Kept {
    rate: u8,
    settled: bool,
}

fn kept(packets: &[&[u8]]) -> Vec<Kept> {
    let mut out = Vec::new();
    for (i, p) in packets.iter().enumerate() {
        if p.len() <= 1 {
            continue;
        }
        let back = |n: usize| packets[i.saturating_sub(n)..i].iter();
        let settled = matches!(p[0], 3 | 4)
            && back(2).all(|q| q.first() != Some(&1))
            && back(10).all(|q| q.len() > 1);
        out.push(Kept {
            rate: p[0],
            settled,
        });
    }
    out
}

fn ffmpeg_f32(path: &Path, postfilter: bool) -> Vec<f32> {
    let mut c = std::process::Command::new("ffmpeg");
    c.args(["-hide_banner", "-loglevel", "error"]);
    if !postfilter {
        c.args(["-postfilter", "0"]);
    }
    let o = c
        .arg("-i")
        .arg(path)
        .args(["-f", "f32le", "-ac", "1", "-"])
        .output()
        .expect("ffmpeg");
    o.stdout
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]) * 32768.0)
        .collect()
}

fn s16(path: &Path) -> Vec<f32> {
    std::fs::read(path)
        .unwrap()
        .chunks_exact(2)
        .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])))
        .collect()
}

struct Score {
    snr: f64,
    max: f32,
    frames: usize,
}

/// SNR and largest difference over the frames `keep` selects; `round` rounds ours to 16
/// bits first (for FFmpeg's 16-bit output).
fn score(a: &[f32], b: &[f32], round: bool, keep: impl Fn(usize) -> bool) -> Score {
    let n = a.len().min(b.len()) / FRAME_SAMPLES;
    let (mut sig, mut err, mut max, mut frames) = (0f64, 0f64, 0f32, 0);
    for f in (0..n).filter(|&f| keep(f)) {
        frames += 1;
        for i in f * FRAME_SAMPLES..(f + 1) * FRAME_SAMPLES {
            let x = if round {
                a[i].round().clamp(-32768.0, 32767.0)
            } else {
                a[i]
            };
            let e = x - b[i];
            sig += f64::from(b[i]).powi(2);
            err += f64::from(e).powi(2);
            max = max.max(e.abs());
        }
    }
    let snr = if err == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (sig / err).log10()
    };
    Score { snr, max, frames }
}

/// All frames, rate 1 and 1/2 frames, and the settled ones (see [`Kept`]).
fn report(name: &str, frames: &[Kept], a: &[f32], b: &[f32], round: bool) -> (Score, Score) {
    let coded = |f: usize| frames.get(f).is_some_and(|k| matches!(k.rate, 3 | 4));
    let settled = |f: usize| frames.get(f).is_some_and(|k| k.settled);
    let all = score(a, b, round, |_| true);
    let c = score(a, b, round, coded);
    let s = score(a, b, round, settled);
    println!(
        "{name}: {} frames: all SNR {:.2} dB max {:.0}; rate 1 and 1/2 ({}) SNR {:.2} dB \
         max {:.0}; settled ({}) SNR {:.2} dB max {:.0}",
        frames.len(),
        all.snr,
        all.max,
        c.frames,
        c.snr,
        c.max,
        s.frames,
        s.snr,
        s.max
    );
    assert_eq!(a.len(), b.len(), "{name}: length");
    (c, s)
}

#[test]
fn fixture() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let d = std::fs::read(dir.join("mixed.qcp")).unwrap();
    let (evrc, packets) = qcp_packets(&d).unwrap();
    assert!(evrc);
    let k = kept(&packets);
    let (_, plain) = report(
        "mixed.qcp, no postfilter",
        &k,
        &ours(&packets, false),
        &s16(&dir.join("mixed.nopf.s16")),
        true,
    );
    assert!(plain.snr > 45.0 && plain.max <= 16.0, "{:.2} dB", plain.snr);
    let (pf, _) = report(
        "mixed.qcp, postfilter",
        &k,
        &ours(&packets, true),
        &s16(&dir.join("mixed.ffmpeg.s16")),
        true,
    );
    assert!(pf.snr > 10.0, "{:.2} dB", pf.snr);
}

fn corpus_dir() -> PathBuf {
    std::env::var_os("SPECULAR_WTV_CORPUS").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/corpus/wtv1"),
        PathBuf::from,
    )
}

#[test]
#[ignore]
fn corpus() {
    let root = corpus_dir();
    let mut files = Vec::new();
    for sub in ["qcp", "evrc", "suite-qcp"] {
        if let Ok(rd) = std::fs::read_dir(root.join(sub)) {
            files.extend(rd.flatten().map(|e| e.path()));
        }
    }
    files.sort();
    let mut seen = 0;
    for p in files {
        let Ok(d) = std::fs::read(&p) else { continue };
        let Some((true, packets)) = qcp_packets(&d) else {
            continue;
        };
        seen += 1;
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let k = kept(&packets);
        let t = std::time::Instant::now();
        let a = ours(&packets, false);
        let dt = t.elapsed();
        let (_, plain) = report(
            &format!("{name}, no postfilter"),
            &k,
            &a,
            &ffmpeg_f32(&p, false),
            false,
        );
        let a = ours(&packets, true);
        let (pf, _) = report(
            &format!("{name}, postfilter"),
            &k,
            &a,
            &ffmpeg_f32(&p, true),
            false,
        );
        println!(
            "  decode {:.1} ms for {:.1} s of sound",
            dt.as_secs_f64() * 1e3,
            a.len() as f64 / 8000.0
        );
        assert!(plain.snr > 35.0, "{name}: {:.2} dB", plain.snr);
        assert!(pf.snr > 15.0, "{name}: {:.2} dB", pf.snr);
    }
    assert!(seen > 0, "no EVRC files under {}", root.display());
}
