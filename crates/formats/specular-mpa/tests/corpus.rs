//! The ISO/IEC 11172-4 Layer III compliance streams (tools/corpus/fetch-mp3.sh) against
//! ffmpeg's float decode, both as stereo: within 1.5 steps of 16 bits at the best alignment,
//! leaving out the first three frames (a stream cut mid-reservoir starts differently in every
//! decoder). Free format is refused (counted as skipped). Skips without the corpus.
//!
//!   SPECULAR_MP3_CORPUS=<dir> cargo test -p specular-mpa --test corpus -- --ignored

use std::path::PathBuf;

use specular_mpa::{Decoder, Header};

/// Each frame's channel count, in order.
fn frame_channels(data: &[u8]) -> Vec<usize> {
    let mut v = Vec::new();
    let mut i = 0;
    while i + 4 <= data.len() {
        match Header::parse(&data[i..]).and_then(|h| Some((h, h.frame_len()?))) {
            Some((h, len))
                if data
                    .get(i + len..i + len + 4)
                    .is_none_or(|n| Header::parse(n).is_some()) =>
            {
                v.push(h.channels());
                i += len;
            }
            _ => i += 1,
        }
    }
    v
}

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_MP3_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| PathBuf::from(t).join("corpus/mp3"))
        })?;
    dir.is_dir().then_some(dir)
}

#[test]
#[ignore]
fn compliance_streams_match_ffmpeg() {
    let Some(dir) = corpus() else {
        eprintln!("no MP3 corpus: skipped (tools/corpus/fetch-mp3.sh)");
        return;
    };
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "bit"))
        .collect();
    names.sort();
    let mut failed = Vec::new();
    for path in names {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(&path).unwrap();
        let mut d = Decoder::with_channels(2);
        let mut got = Vec::new();
        let r = d.decode(&data, &mut got).and_then(|_| d.flush(&mut got));
        // ffmpeg spreads mono frames at -3 dB to each side.
        for (k, &c) in frame_channels(&data).iter().enumerate() {
            if c == 1 {
                for v in got.iter_mut().skip(k * 2304).take(2304) {
                    *v *= std::f32::consts::FRAC_1_SQRT_2;
                }
            }
        }
        let Ok(want) = std::fs::read(path.with_extension("f32")) else {
            println!("ORACLE\tSound/MP3\t{name}\tskip");
            eprintln!("{name}: no oracle ({r:?})");
            continue;
        };
        if let Err(e) = r {
            println!("ORACLE\tSound/MP3\t{name}\tfail");
            failed.push(format!("{name}: {e}"));
            continue;
        }
        let want: Vec<f32> = want
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        if let Some(d) = std::env::var_os("MPA_DUMP") {
            let b: Vec<u8> = got.iter().flat_map(|v| v.to_le_bytes()).collect();
            std::fs::write(PathBuf::from(d).join(format!("{name}.f32")), b).unwrap();
        }
        let ch = 2;
        let skip = 3 * 1152 * ch;
        // The best alignment: ffmpeg may drop a damaged first frame.
        let (lead, worst, snr) = (0..=2304usize)
            .flat_map(|k| [k as i64, -(k as i64)])
            .filter_map(|k| {
                let (g, w) = if k >= 0 {
                    (got.get(k as usize * ch..)?, &want[..])
                } else {
                    (&got[..], want.get((-k) as usize * ch..)?)
                };
                let n = g.len().min(w.len());
                if n * 10 < want.len() * 9 {
                    return None;
                }
                let (mut worst, mut sig, mut err) = (0f64, 0f64, 0f64);
                for (a, b) in g[skip..n].iter().zip(&w[skip..n]) {
                    let e = f64::from(a - b);
                    worst = worst.max(e.abs() * 32768.0);
                    sig += f64::from(*b).powi(2);
                    err += e * e;
                }
                Some((k, worst, 10.0 * (sig / err.max(1e-30)).log10()))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, f64::MAX, 0.0));
        let pass = worst <= 1.5;
        eprintln!(
            "{name}: {} of {} samples, from {lead}: within {worst:.2} steps, {snr:.1} dB",
            got.len(),
            want.len()
        );
        println!(
            "ORACLE\tSound/MP3\t{name}\t{}",
            if pass { "pass" } else { "fail" }
        );
        if !pass {
            failed.push(format!("{name}: {worst:.2} steps"));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}
