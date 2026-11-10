//! Against the reference decoders' output (not committed): each `X.mpc` beside `X.mpc.ref`
//! (16-bit PCM from libmpcdec's `mpcdec`, float output truncated to 16 bits) and FFmpeg's
//! FATE files `inside-mp7.mpc`/`inside-mp8.mpc` beside their `.pcm` (FFmpeg's output: every
//! frame from the first, delay not dropped, rounded).
//!
//!   SPECULAR_MPC_CORPUS=<dir> cargo test -p specular-mpc --test oracle -- --ignored --nocapture

use std::path::PathBuf;

fn corpus() -> Option<PathBuf> {
    std::env::var_os("SPECULAR_MPC_CORPUS").map(PathBuf::from)
}

fn words(d: &[u8]) -> Vec<i16> {
    d.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

struct Cmp {
    n: usize,
    diff: usize,
    max: i32,
    snr: f64,
}

fn compare(got: &[i16], want: &[i16]) -> Cmp {
    let n = got.len().min(want.len());
    let (mut sig, mut err, mut diff, mut max) = (0f64, 0f64, 0, 0);
    for i in 0..n {
        let d = i32::from(got[i]) - i32::from(want[i]);
        sig += f64::from(want[i]).powi(2);
        err += f64::from(d).powi(2);
        if d != 0 {
            diff += 1;
            max = max.max(d.abs());
        }
    }
    let snr = if err == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (sig / err).log10()
    };
    Cmp { n, diff, max, snr }
}

/// libmpcdec's conversion: ×32768, truncated toward zero, clipped.
fn truncate(x: &[f32]) -> Vec<i16> {
    x.iter()
        .map(|&v| ((v * 32768.0) as i32).clamp(-32768, 32767) as i16)
        .collect()
}

fn rounded(x: &[f32]) -> Vec<i16> {
    x.iter()
        .map(|&v| (v * 32768.0).round().clamp(-32768.0, 32767.0) as i16)
        .collect()
}

#[test]
#[ignore]
fn matches_mpcdec_and_ffmpeg() {
    let Some(dir) = corpus() else {
        eprintln!("SPECULAR_MPC_CORPUS not set");
        return;
    };
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "mpc"))
        .collect();
    names.sort();
    let (mut files, mut exact_files, mut total, mut total_diff) = (0, 0, 0, 0);
    for p in names {
        let d = std::fs::read(&p).unwrap();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let t = std::time::Instant::now();
        let (info, pcm) = match specular_mpc::decode_all(&d) {
            Ok(x) => x,
            Err(e) => {
                println!("{name}: error {e}");
                continue;
            }
        };
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        for (label, r) in [("mpcdec", p.with_extension("mpc.ref"))] {
            if let Ok(want) = std::fs::read(&r) {
                let want = words(&want);
                let c = compare(&truncate(&pcm), &want);
                println!(
                    "{name} vs {label}: SV{} {} Hz {} ch, {} of {} samples, {} differ (max {}), SNR {:.1} dB, {:.1} ms",
                    info.version,
                    info.sample_rate,
                    info.channels,
                    c.n,
                    want.len(),
                    c.diff,
                    c.max,
                    c.snr,
                    ms
                );
                files += 1;
                total += c.n;
                total_diff += c.diff;
                if c.diff == 0 && c.n == want.len() {
                    exact_files += 1;
                }
            }
        }
        // FFmpeg's FATE reference: all frames from the first, nothing dropped
        if let Ok(want) = std::fs::read(p.with_extension("pcm")) {
            let s = specular_mpc::scan(&d).unwrap();
            let mut dec = specular_mpc::Decoder::new(&s.info);
            let mut raw = Vec::new();
            for pk in &s.packets {
                let _ = dec.decode(pk, &d[pk.offset..pk.offset + pk.size], &mut raw);
            }
            let want = words(&want);
            let got = rounded(&raw);
            // FATE's SV8 test starts after a seek: find where its first samples are ours.
            let at = (0..got.len().saturating_sub(want.len()) + 1)
                .step_by(2)
                .find(|&o| (0..64).all(|i| (got[o + i] - want[i]).abs() <= 1))
                .unwrap_or(0);
            let c = compare(&got[at..], &want);
            println!(
                "{name} vs ffmpeg (from sample {}): {} of {} samples, {} differ (max {}), SNR {:.1} dB",
                at / 2,
                c.n,
                want.len(),
                c.diff,
                c.max,
                c.snr
            );
        }
    }
    println!("mpcdec: {exact_files}/{files} files exact, {total_diff} of {total} samples differ");
}
