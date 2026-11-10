//! FFmpeg's FATE DTS streams (tools/corpus/fetch-dts.sh) against ffmpeg's core-only float
//! decode. Skips without the corpus.
//!
//! Every stream is decoded twice:
//! - as a player would: within 1.5 steps of 16 bits on every channel, or refused for one of
//!   the two tools whose code books the specification leaves out (counted as skipped);
//! - with [`Options::partial`] (those tools left out): the LFE channel within 1.5 steps,
//!   each primary channel at the oracle's level (least-squares gain within 15%), and the
//!   front pair below 2 kHz (never VQ coded) within `LOW_DB` of the oracle when no ADPCM
//!   prediction is involved. This checks everything but the two missing code books.
//!
//! `.dtshd` files also hold a BLACKOUT frame after the stream, which ffmpeg leaves out: only
//! the oracle's length is compared.
//!
//!   SPECULAR_DTS_CORPUS=<dir> cargo test -p specular-dts --test corpus -- --ignored

use std::path::PathBuf;

use specular_dts::{Decoder, Error, Options};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_DTS_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| PathBuf::from(t).join("corpus/dts"))
        })?;
    dir.is_dir().then_some(dir)
}

fn decode(data: &[u8], partial: bool) -> (Vec<f32>, usize, Result<(), Error>) {
    let mut d = Decoder::new(Options {
        partial,
        ..Options::default()
    });
    let mut got = Vec::new();
    let mut r = Ok(());
    // Fed in pieces, as a container would.
    for c in data.chunks(4093) {
        if let Err(e) = d.decode(c, &mut got) {
            let stop = matches!(e, Error::Unsupported(_));
            r = Err(e);
            if stop {
                break;
            }
        }
    }
    (got, d.channels(), r)
}

/// Energy below 2 kHz of `y` and of `x − y` (a plain DFT over the band's bins).
fn low_band(x: &[f32], y: &[f32], rate: f32) -> (f64, f64) {
    let n = x.len().min(y.len()).min(8192);
    let bins = (2000.0 * n as f32 / rate) as usize;
    let (mut s, mut e) = (0f64, 0f64);
    for k in 1..bins {
        let (mut yr, mut yi, mut dr, mut di) = (0f64, 0f64, 0f64, 0f64);
        for t in 0..n {
            let w = -2.0 * std::f64::consts::PI * (k * t) as f64 / n as f64;
            let (c, si) = (w.cos(), w.sin());
            let a = f64::from(y[t]);
            let d = f64::from(x[t]) - a;
            yr += a * c;
            yi += a * si;
            dr += d * c;
            di += d * si;
        }
        s += yr * yr + yi * yi;
        e += dr * dr + di * di;
    }
    (s, e)
}

/// Below 2 kHz the partial decode of the front pair must be this close to ffmpeg's.
const LOW_DB: f64 = 60.0;

#[test]
#[ignore]
fn fate_streams_match_ffmpeg() {
    let Some(dir) = corpus() else {
        eprintln!("no DTS corpus: skipped (tools/corpus/fetch-dts.sh)");
        return;
    };
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "dts" || e == "dtshd"))
        .collect();
    names.sort();
    let mut failed = Vec::new();
    for path in names {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(&path).unwrap();
        let info = specular_dts::probe(&data).expect("a DTS core frame");
        let Ok(want) = std::fs::read(path.with_extension("f32")) else {
            println!("ORACLE\tSound/DTS\t{name}\tskip");
            continue;
        };
        let want: Vec<f32> = want
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        // As played.
        let (got, ch, r) = decode(&data, false);
        let status = match &r {
            Err(Error::Unsupported(why)) => {
                assert!(
                    why.contains("not in the published specification"),
                    "{name}: {why}"
                );
                eprintln!("{name}: refused: {why}");
                "skip"
            }
            _ => {
                let n = want.len();
                let worst = (0..n.min(got.len()))
                    .map(|i| (got[i] - want[i]).abs())
                    .fold(0f32, f32::max);
                eprintln!("{name}: {ch} ch, within {:.2} steps", worst * 32768.0);
                if got.len() >= n && worst * 32768.0 <= 1.5 && r.is_ok() {
                    "pass"
                } else {
                    "fail"
                }
            }
        };
        println!("ORACLE\tSound/DTS\t{name}\t{status}");
        if status == "fail" {
            failed.push(name.clone());
        }
        // Without the missing tools.
        let (got, ch, r) = decode(&data, true);
        assert!(r.is_ok(), "{name}: {r:?}");
        assert!(got.len() >= want.len(), "{name}: short");
        assert_eq!(ch, info.first.channels);
        let frames = want.len() / ch;
        let lfe = (info.first.lfe > 0).then_some(3);
        let mut notes = Vec::new();
        for c in 0..ch {
            let x: Vec<f32> = (0..frames).map(|i| got[i * ch + c]).collect();
            let y: Vec<f32> = (0..frames).map(|i| want[i * ch + c]).collect();
            if Some(c) == lfe {
                let worst = x
                    .iter()
                    .zip(&y)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0f32, f32::max);
                notes.push(format!("LFE {:.2} steps", worst * 32768.0));
                // With a lossless (XLL) layer ffmpeg's LFE drifts from ours by about 0.1%
                // from the fourth frame on, though it was asked for the core alone.
                let limit = if name.starts_with("xll") { 64.0 } else { 1.5 };
                if worst * 32768.0 > limit {
                    failed.push(format!("{name}: LFE"));
                }
                continue;
            }
            let xy: f64 = x
                .iter()
                .zip(&y)
                .map(|(a, b)| f64::from(*a) * f64::from(*b))
                .sum();
            let xx: f64 = x.iter().map(|a| f64::from(*a) * f64::from(*a)).sum();
            let gain = xy / xx.max(1e-30);
            notes.push(format!("ch{c} gain {gain:.3}"));
            if (gain - 1.0).abs() > 0.15 {
                failed.push(format!("{name}: channel {c} level"));
            }
            // dts_es.dts predicts its low subbands (ADPCM): no band check there.
            if c < 2 && !name.starts_with("dts_es") {
                let (s, e) = low_band(&x, &y, info.first.sample_rate as f32);
                let db = 10.0 * (s / e.max(1e-30)).log10();
                notes.push(format!("low {db:.0} dB"));
                if db < LOW_DB {
                    failed.push(format!("{name}: channel {c} below 2 kHz: {db:.0} dB"));
                }
            }
        }
        eprintln!("{name} (partial): {}", notes.join(", "));
    }
    assert!(failed.is_empty(), "differ from ffmpeg: {failed:?}");
}
