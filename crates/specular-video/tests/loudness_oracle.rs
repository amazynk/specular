//! Loudness against ffmpeg's `ebur128` filter (a test oracle only), on signals made here and
//! written as 32-bit float WAVE files: steps of tone (EBU Tech 3341 case 4), level-varying
//! noise, 5.1 sound, and tones whose crests fall between samples. Each file is also opened
//! as a sound file, so the session's measurement (decode, analysis) is what is compared.
//! Skipped when ffmpeg is not installed.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::Registry;
use specular_video::loudness::{self, Meter};
use specular_video::sound_view::SoundSession;

fn have_ffmpeg() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// A 32-bit float WAVE file of interleaved `pcm`.
fn write_wav(path: &Path, rate: u32, ch: u16, pcm: &[f32]) {
    let data_len = (pcm.len() * 4) as u32;
    let mut b = Vec::with_capacity(pcm.len() * 4 + 44);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes());
    b.extend_from_slice(&ch.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 4 * u32::from(ch)).to_le_bytes());
    b.extend_from_slice(&(4 * ch).to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for v in pcm {
        b.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}

/// ffmpeg's summary: integrated (LUFS), range (LU), true peak (dBFS).
fn ffmpeg_ebur128(path: &Path) -> (f64, f64, f64) {
    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-nostats", "-i"])
        .arg(path)
        .args(["-filter_complex", "ebur128=peak=true", "-f", "null", "-"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stderr);
    let summary = &text[text.rfind("Summary:").expect("ffmpeg summary")..];
    let value = |key: &str| -> f64 {
        let line = summary
            .lines()
            .find(|l| l.trim_start().starts_with(key))
            .unwrap_or_else(|| panic!("{key} in {summary}"));
        let v = line.trim_start()[key.len()..]
            .split_whitespace()
            .next()
            .unwrap();
        if v == "-inf" {
            f64::NEG_INFINITY
        } else {
            v.parse().unwrap()
        }
    };
    (value("I:"), value("LRA:"), value("Peak:"))
}

/// A small deterministic noise source.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

fn tone_steps(rate: u32) -> Vec<f32> {
    let parts = [
        (-72.0, 10.0),
        (-36.0, 10.0),
        (-23.0, 60.0),
        (-36.0, 10.0),
        (-72.0, 10.0),
    ];
    let mut out = Vec::new();
    let mut n = 0u64;
    for (dbfs, secs) in parts {
        let a = 10f64.powf(dbfs / 20.0);
        for _ in 0..(secs * f64::from(rate)) as u64 {
            let v = a * (2.0 * std::f64::consts::PI * 1000.0 * n as f64 / f64::from(rate)).sin();
            out.extend([v as f32, v as f32]);
            n += 1;
        }
    }
    out
}

/// Stereo noise, lowpassed differently per channel, its level swinging over 30 dB.
fn varying_noise(rate: u32, secs: f64) -> Vec<f32> {
    let mut r = Lcg(7);
    let (mut l, mut rr) = (0.0, 0.0);
    let len = (secs * f64::from(rate)) as usize;
    let mut out = Vec::with_capacity(len * 2);
    for n in 0..len {
        let t = n as f64 / f64::from(rate);
        let db = -30.0
            + 15.0 * (2.0 * std::f64::consts::PI * t / 17.0).sin()
            + 10.0 * ((t / 5.0).floor() % 2.0);
        let a = 10f64.powf(db / 20.0);
        l = 0.7 * l + 0.3 * r.next();
        rr = 0.2 * rr + 0.8 * r.next();
        out.extend([(a * l) as f32, (a * rr) as f32]);
    }
    out
}

/// 5.1: a different tone and level on each channel, the LFE loud (it must not count).
fn surround(rate: u32, secs: f64) -> Vec<f32> {
    let tones = [
        (440.0, -26.0),
        (550.0, -28.0),
        (660.0, -24.0),
        (50.0, -6.0),
        (770.0, -30.0),
        (880.0, -31.0),
    ];
    let len = (secs * f64::from(rate)) as usize;
    let mut out = Vec::with_capacity(len * 6);
    for n in 0..len {
        let t = n as f64 / f64::from(rate);
        for (hz, db) in tones {
            out.push((10f64.powf(db / 20.0) * (2.0 * std::f64::consts::PI * hz * t).sin()) as f32);
        }
    }
    out
}

/// Tones whose crests fall between samples: fs/4 at 45°, then 15 kHz, then 5 kHz, each
/// faded in and out over 20 ms (no clicks to overshoot); the true peak is 0.95 exactly.
fn crests(rate: u32) -> Vec<f32> {
    let mut out = Vec::new();
    let fade = 0.02 * f64::from(rate);
    for (hz, phase, a, secs) in [
        (f64::from(rate) / 4.0, 45.0f64, 0.95, 4.0),
        (15_000.0, 30.0, 0.5, 4.0),
        (5_000.0, 10.0, 0.7, 4.0),
    ] {
        let len = (secs * f64::from(rate)) as u64;
        for n in 0..len {
            let edge = (n.min(len - 1 - n) as f64 / fade).min(1.0);
            let env = 0.5 - 0.5 * (std::f64::consts::PI * edge).cos();
            let v = env
                * a
                * (2.0 * std::f64::consts::PI * hz * n as f64 / f64::from(rate)
                    + phase.to_radians())
                .sin();
            out.extend([v as f32, (v * 0.5) as f32]);
        }
    }
    out
}

fn dir() -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("loudness-oracle");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The session's loudness of the file at `path`.
fn session_report(path: &Path) -> Arc<loudness::Report> {
    specular_video::install();
    let s = Registry::new()
        .open(Arc::new(Source::open(path).unwrap()))
        .unwrap();
    let sound = s
        .as_any()
        .and_then(|a| a.downcast_ref::<SoundSession>())
        .expect("a sound session");
    sound
        .loudness(&CancelToken::new())
        .unwrap()
        .expect("measured")
}

#[test]
fn matches_ffmpeg_ebur128() {
    if !have_ffmpeg() {
        eprintln!("ffmpeg not installed: oracle skipped");
        return;
    }
    let cases: Vec<(&str, u32, u16, Vec<f32>)> = vec![
        ("steps-48k", 48_000, 2, tone_steps(48_000)),
        ("noise-44k1", 44_100, 2, varying_noise(44_100, 60.0)),
        ("surround-48k", 48_000, 6, surround(48_000, 20.0)),
        ("crests-48k", 48_000, 2, crests(48_000)),
        ("crests-44k1", 44_100, 2, crests(44_100)),
    ];
    let mut worst = (0.0f64, 0.0f64, 0.0f64);
    for (name, rate, ch, pcm) in cases {
        let path = dir().join(format!("{name}.wav"));
        write_wav(&path, rate, ch, &pcm);
        let (fi, flra, ftp) = ffmpeg_ebur128(&path);
        // Our meter on the samples, and the session on the file.
        let mut m = Meter::new(rate, usize::from(ch));
        m.push(&pcm);
        let direct = m.report();
        let ours = session_report(&path);
        assert_eq!(
            ours.integrated, direct.integrated,
            "{name}: decode changed the sound"
        );
        let i = ours.integrated.unwrap();
        let lra = ours.range.unwrap_or(0.0);
        let tp = ours.max_true_peak_db();
        eprintln!(
            "{name}: I {i:.2} / {fi:.1} LUFS, LRA {lra:.2} / {flra:.1} LU, true peak {tp:.2} / {ftp:.1} dBTP"
        );
        // ffmpeg prints one decimal: within 0.1 of its rounding.
        assert!((i - fi).abs() <= 0.15, "{name}: I {i} vs {fi}");
        // Its range comes from a 0.1 LU histogram.
        assert!((lra - flra).abs() <= 0.25, "{name}: LRA {lra} vs {flra}");
        // Where the true peak is known (the crests: 0.95), within BS.1770's +0.2 / −0.4 dB.
        if name.starts_with("crests") {
            let err = tp - 20.0 * 0.95f64.log10();
            assert!(
                (-0.4..=0.2).contains(&err),
                "{name}: true peak off by {err}"
            );
        }
        // Against ffmpeg's: different oversampling filters, so a wider margin (its reading of
        // the crests runs over the known peak).
        assert!((tp - ftp).abs() <= 0.7, "{name}: true peak {tp} vs {ftp}");
        worst.0 = worst.0.max((i - fi).abs());
        worst.1 = worst.1.max((lra - flra).abs());
        worst.2 = worst.2.max((tp - ftp).abs());
    }
    eprintln!(
        "worst differences: I {:.2} LU, LRA {:.2} LU, true peak {:.2} dB",
        worst.0, worst.1, worst.2
    );
}
