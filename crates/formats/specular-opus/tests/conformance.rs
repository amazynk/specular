//! Conformance: the official Opus test vectors (RFC 8251 set, fetched by
//! tools/corpus/specular-opus-fetch.sh) and the committed libopus fixtures.

use specular_opus::{Decoder, Head};
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/video2/specular-opus")
}

fn stereo_head() -> Head {
    Head::parse(&[
        b'O', b'p', b'u', b's', b'H', b'e', b'a', b'd', 1, 2, 0, 0, 0x80, 0xbb, 0, 0, 0, 0, 0,
    ])
    .unwrap()
}

/// Packets of an opus_demo .bit file: (final range, packet).
fn read_bit(data: &[u8]) -> Vec<(u32, Vec<u8>)> {
    let mut out = Vec::new();
    let mut p = 0;
    while p + 8 <= data.len() {
        let len = u32::from_be_bytes(data[p..p + 4].try_into().unwrap()) as usize;
        let rng = u32::from_be_bytes(data[p + 4..p + 8].try_into().unwrap());
        p += 8;
        out.push((rng, data[p..p + len].to_vec()));
        p += len;
    }
    out
}

fn to_i16(x: f32) -> i16 {
    (x * 32768.0).round().clamp(-32768.0, 32767.0) as i16
}

/// Real FFT power spectrum of a 1024-point Hann-windowed frame (own radix-2 FFT).
fn power_spectrum(frame: &[f32], out: &mut [f64]) {
    let n = 1024;
    let mut re = vec![0f64; n];
    let mut im = vec![0f64; n];
    for i in 0..frame.len().min(n) {
        let w =
            0.5 - 0.5 * (2.0 * std::f64::consts::PI * (i as f64 + 0.5) / frame.len() as f64).cos();
        re[i] = frame[i] as f64 * w;
    }
    // Bit reversal.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        for s in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = ((ang * k as f64).cos(), (ang * k as f64).sin());
                let a = s + k;
                let b = a + len / 2;
                let tr = re[b] * wr - im[b] * wi;
                let ti = re[b] * wi + im[b] * wr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
    for k in 0..=n / 2 {
        out[k] = re[k] * re[k] + im[k] * im[k];
    }
}

/// Band edges in FFT bins (1024 points at 48 kHz): roughly the CELT bands up to 20 kHz.
const BANDS: [usize; 22] = [
    0, 4, 8, 13, 17, 21, 26, 30, 34, 43, 51, 60, 68, 85, 102, 120, 145, 171, 205, 256, 333, 427,
];

/// Perceptual distance in the spirit of opus_compare: per 20 ms frame and band, the squared
/// log ratio of band powers, each floored by a masking level 30 dB under the frame's loudest
/// band and an absolute floor. Returns the mean.
fn spectral_distance(reference: &[f32], test: &[f32], channels: usize) -> f64 {
    let frames = reference.len() / channels / 480;
    let mut total = 0f64;
    let mut count = 0usize;
    let mut sr = vec![0f64; 513];
    let mut st = vec![0f64; 513];
    for c in 0..channels {
        for f in 0..frames.saturating_sub(1) {
            let take = |x: &[f32]| -> Vec<f32> {
                (0..960)
                    .map(|i| x[((f * 480 + i) * channels + c).min(x.len() - 1)])
                    .collect()
            };
            power_spectrum(&take(reference), &mut sr);
            power_spectrum(&take(test), &mut st);
            let er: Vec<f64> = BANDS
                .windows(2)
                .map(|w| sr[w[0]..w[1]].iter().sum::<f64>() / (w[1] - w[0]) as f64)
                .collect();
            let et: Vec<f64> = BANDS
                .windows(2)
                .map(|w| st[w[0]..w[1]].iter().sum::<f64>() / (w[1] - w[0]) as f64)
                .collect();
            let peak = er.iter().cloned().fold(0.0, f64::max);
            let floor = peak * 1e-3 + 1e-9;
            for (a, b) in er.iter().zip(et.iter()) {
                let d = ((b + floor) / (a + floor)).log10() * 10.0;
                total += d * d;
                count += 1;
            }
        }
    }
    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

fn snr_db(reference: &[f32], test: &[f32]) -> f64 {
    let mut s = 0f64;
    let mut e = 0f64;
    for (a, b) in reference.iter().zip(test.iter()) {
        s += (*a as f64) * (*a as f64);
        e += (*a as f64 - *b as f64).powi(2);
    }
    10.0 * (s / e.max(1e-20)).log10()
}

/// Quality like opus_compare's: 100 for identical output, 0 at the distance that additive white
/// noise 48 dB under the reference gives, negative below.
fn quality(reference: &[f32], test: &[f32], channels: usize) -> f64 {
    let rms = (reference.iter().map(|v| (*v as f64).powi(2)).sum::<f64>()
        / reference.len().max(1) as f64)
        .sqrt();
    let amp = rms * 10f64.powf(-48.0 / 20.0) * 3f64.sqrt();
    let mut seed = 1u32;
    let noisy: Vec<f32> = reference
        .iter()
        .map(|v| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            v + (((seed >> 8) as f64 / (1u32 << 24) as f64 * 2.0 - 1.0) * amp) as f32
        })
        .collect();
    let thresh = spectral_distance(reference, &noisy, channels);
    let d = spectral_distance(reference, test, channels);
    100.0 * (1.0 - d / thresh.max(1e-12))
}

struct VectorResult {
    packets: usize,
    range_mismatch: usize,
    quality: f64,
    quality_m: f64,
    snr: f64,
}

fn run_vector(n: u32) -> Option<VectorResult> {
    let dir = corpus();
    let bit = std::fs::read(dir.join(format!("testvector{n:02}.bit"))).ok()?;
    let dec = std::fs::read(dir.join(format!("testvector{n:02}.dec"))).ok()?;
    let decm = std::fs::read(dir.join(format!("testvector{n:02}m.dec"))).ok()?;
    let packets = read_bit(&bit);
    let mut d = Decoder::new(&stereo_head()).unwrap();
    let mut pcm = Vec::new();
    let mut mismatch = 0;
    for (i, (rng, p)) in packets.iter().enumerate() {
        let r = d.decode(p, &mut pcm);
        if r.is_err() || (!p.is_empty() && d.final_range() != *rng) {
            if mismatch < 3 {
                eprintln!(
                    "vector {n}: packet {i} range {:08x} expected {rng:08x} ({r:?})",
                    d.final_range()
                );
            }
            mismatch += 1;
        }
    }
    let ours: Vec<f32> = pcm
        .iter()
        .map(|&v| f32::from(to_i16(v)) / 32768.0)
        .collect();
    let refs = |b: &[u8]| -> Vec<f32> {
        b.chunks_exact(2)
            .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
            .collect()
    };
    let r = refs(&dec);
    let rm = refs(&decm);
    let len = r.len().min(ours.len());
    if ours.len() != r.len() {
        eprintln!("vector {n}: {} samples, reference {}", ours.len(), r.len());
    }
    Some(VectorResult {
        packets: packets.len(),
        range_mismatch: mismatch,
        quality: quality(&r[..len], &ours[..len], 2),
        quality_m: quality(&rm[..len], &ours[..len], 2),
        snr: snr_db(&r[..len], &ours[..len]),
    })
}

#[test]
fn official_test_vectors() {
    if !corpus().join("testvector01.bit").exists() {
        eprintln!("skipped: run tools/corpus/specular-opus-fetch.sh");
        return;
    }
    let only: Option<u32> = std::env::var("SPECULAR_OPUS_VECTOR")
        .ok()
        .and_then(|v| v.parse().ok());
    let mut failed = Vec::new();
    for n in 1..=12 {
        if only.is_some_and(|o| o != n) {
            continue;
        }
        let r = run_vector(n).expect("vector files");
        eprintln!(
            "testvector{n:02}: {} packets, {} final-range mismatches, quality {:.1} (m: {:.1}), SNR {:.1} dB",
            r.packets, r.range_mismatch, r.quality, r.quality_m, r.snr
        );
        // SILK and hybrid vectors (2–6, 12) reach 57–62 dB: SILK matches the reference
        // at its internal rate, and what is left is our measured upsampler (17–23 dB with a
        // linear-phase one); 10 mixes CELT with hybrid at 71 dB.
        let min_snr = match n {
            2..=6 | 12 => 56.0,
            10 => 68.0,
            _ => 70.0,
        };
        if r.range_mismatch > 0 || r.quality.max(r.quality_m) < 0.0 || r.snr < min_snr {
            failed.push(n);
        }
    }
    assert!(failed.is_empty(), "failed vectors: {failed:?}");
}

/// Where our decode differs from the reference (`SPECULAR_OPUS_VECTOR`, default 5, a hybrid
/// one): the signal-to-error ratio by frequency range, and at small time shifts. A study
/// aid: `cargo test -p specular-opus --test conformance error_by_band -- --ignored --nocapture`.
#[test]
#[ignore = "a study aid"]
fn error_by_band() {
    let n: u32 = std::env::var("SPECULAR_OPUS_VECTOR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let dir = corpus();
    let Ok(bit) = std::fs::read(dir.join(format!("testvector{n:02}.bit"))) else {
        eprintln!("skipped: run tools/corpus/specular-opus-fetch.sh");
        return;
    };
    let dec = std::fs::read(dir.join(format!("testvector{n:02}.dec"))).unwrap();
    let mut d = Decoder::new(&stereo_head()).unwrap();
    let mut pcm = Vec::new();
    for (_, p) in read_bit(&bit) {
        let _ = d.decode(&p, &mut pcm);
    }
    let ours: Vec<f32> = pcm
        .iter()
        .map(|&v| f32::from(to_i16(v)) / 32768.0)
        .collect();
    let r: Vec<f32> = dec
        .chunks_exact(2)
        .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
        .collect();
    let len = r.len().min(ours.len());
    for shift in -2i64..=2 {
        let (mut s, mut e) = (0f64, 0f64);
        for i in 4..len / 2 - 4 {
            let a = r[2 * i] as f64;
            let b = ours[(2 * i as i64 + 2 * shift) as usize] as f64;
            s += a * a;
            e += (a - b).powi(2);
        }
        eprintln!(
            "vector {n}: shift {shift}: {:.2} dB",
            10.0 * (s / e).log10()
        );
    }
    // How the error spreads over 20 ms frames: a few bad frames point at decoding logic,
    // an even spread at arithmetic.
    let mut per_frame: Vec<f64> = (0..len / 2 / 960)
        .map(|f| {
            (f * 960..(f + 1) * 960)
                .map(|i| {
                    (0..2)
                        .map(|c| f64::from(ours[2 * i + c] - r[2 * i + c]).powi(2))
                        .sum::<f64>()
                })
                .sum()
        })
        .collect();
    let total: f64 = per_frame.iter().sum();
    per_frame.sort_by(|a, b| b.total_cmp(a));
    let top = |p: usize| 100.0 * per_frame[..per_frame.len() * p / 100].iter().sum::<f64>() / total;
    let mut loud: Vec<f64> = (0..len / 2 / 960)
        .map(|f| {
            (2 * f * 960..2 * (f + 1) * 960)
                .map(|i| f64::from(r[i]).powi(2))
                .sum()
        })
        .collect();
    let sig: f64 = loud.iter().sum();
    loud.sort_by(|a, b| b.total_cmp(a));
    let loudest = |p: usize| 100.0 * loud[..loud.len() * p / 100].iter().sum::<f64>() / sig;
    eprintln!(
        "vector {n}: the worst 1 % of frames hold {:.1} % of the error, the worst 10 % {:.1} % (the loudest hold {:.1} % and {:.1} % of the sound)",
        top(1),
        top(10),
        loudest(1),
        loudest(10)
    );
    let edges_hz = [
        0.0, 2000.0, 4000.0, 6000.0, 7000.0, 7500.0, 8000.0, 8500.0, 9000.0, 12000.0, 16000.0,
        24000.0,
    ];
    let bin = |hz: f64| ((hz / 48000.0) * 1024.0).round() as usize;
    let mut sig = vec![0f64; edges_hz.len() - 1];
    let mut err = vec![0f64; edges_hz.len() - 1];
    let mut ps = vec![0f64; 513];
    let mut pe = vec![0f64; 513];
    let frames = len / 2 / 1024;
    for c in 0..2 {
        for f in 0..frames {
            let a: Vec<f32> = (0..1024).map(|i| r[(f * 1024 + i) * 2 + c]).collect();
            let e: Vec<f32> = (0..1024)
                .map(|i| ours[(f * 1024 + i) * 2 + c] - r[(f * 1024 + i) * 2 + c])
                .collect();
            power_spectrum(&a, &mut ps);
            power_spectrum(&e, &mut pe);
            for (k, w) in edges_hz.windows(2).enumerate() {
                let (lo, hi) = (bin(w[0]), bin(w[1]).min(513));
                sig[k] += ps[lo..hi].iter().sum::<f64>();
                err[k] += pe[lo..hi].iter().sum::<f64>();
            }
        }
    }
    let total_e: f64 = err.iter().sum();
    for (k, w) in edges_hz.windows(2).enumerate() {
        eprintln!(
            "vector {n}: {:>5.0}-{:>5.0} Hz: SNR {:6.1} dB, {:5.1} % of the error",
            w[0],
            w[1],
            10.0 * (sig[k] / err[k].max(1e-30)).log10(),
            100.0 * err[k] / total_e
        );
    }
}
