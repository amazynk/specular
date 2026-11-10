//! The decoder against ffmpeg on the generated corpus (tools/corpus/gen-aac.sh,
//! `SPECULAR_AAC_GEN`): every ADTS stream `NAME.aac` with ffmpeg's float decode `NAME.f32`
//! and its channel count `NAME.ch` is decoded (framing and implicit SBR/PS settled from the
//! first frames, as the player does) and must agree to a tenth of a 16-bit step. Frames
//! that fail (faac writes some scalefactors out of range) are dropped as ffmpeg drops them.
//! Prints the largest error and the SNR of each stream; skips without the corpus.

use specular_aac::{Options, StreamDecoder, adts};

const LSB: f64 = 1.0 / 32768.0;
const MAX_LSB: f64 = 0.1;

/// Streams whose reference does not follow the stream: ffmpeg's encoder writes the LFE of
/// 5.1 as a side channel in its program config element, and its decoder plays it as LFE.
const KNOWN: [&str; 1] = ["lc_51_pce"];

/// Largest error (in 16-bit steps) and SNR of `got` against `want`, at the frame offset that
/// matches best (ffmpeg drops faac's first frame).
fn compare(got: &[f32], want: &[f32], ch: usize) -> (f64, f64, isize) {
    let mut best = (f64::INFINITY, 0.0, 0isize);
    for frames in [0usize, 1024, 2048] {
        for (g, w, sign) in [
            (got.get(frames * ch..).unwrap_or(&[]), want, 1),
            (got, want.get(frames * ch..).unwrap_or(&[]), -1),
        ] {
            let n = g.len().min(w.len());
            if n == 0 {
                continue;
            }
            let (mut sig, mut noise, mut max) = (0.0f64, 0.0f64, 0.0f64);
            for (a, b) in g[..n].iter().zip(&w[..n]) {
                let (a, b) = (f64::from(*a), f64::from(*b));
                sig += b * b;
                noise += (a - b) * (a - b);
                max = max.max((a - b).abs());
            }
            let r = (
                max / LSB,
                10.0 * (sig / noise.max(1e-30)).log10(),
                sign * frames as isize,
            );
            if r.0 < best.0 {
                best = r;
            }
        }
    }
    best
}

#[test]
fn generated_streams_match_ffmpeg() {
    let Some(dir) = std::env::var_os("SPECULAR_AAC_GEN") else {
        eprintln!("skipped: set SPECULAR_AAC_GEN (tools/corpus/gen-aac.sh)");
        return;
    };
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "aac"))
        .collect();
    names.sort();
    let (mut checked, mut failed) = (0, Vec::new());
    for path in names {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let (Ok(want), Ok(ch)) = (
            std::fs::read(path.with_extension("f32")),
            std::fs::read_to_string(path.with_extension("ch")),
        ) else {
            continue;
        };
        let want_ch: usize = ch.trim().parse().unwrap_or(2);
        let want: Vec<f32> = want
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let data = std::fs::read(&path).unwrap();
        let frames: Vec<&[u8]> = adts::frames(&data).map(|(_, f)| f).collect();
        let mut dec = StreamDecoder::new(&[], &frames[..frames.len().min(48)], Options::default())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        // the player conceals frames that fail; ffmpeg drops them
        dec.set_concealment(false);
        let mut got = Vec::new();
        let mut errors = 0;
        for f in &frames {
            if dec.decode(f, &mut got).is_err() {
                errors += 1;
            }
        }
        let ch = usize::from(dec.channels());
        if ch != want_ch {
            failed.push(format!("{name}: {ch} channels, ffmpeg {want_ch}"));
            continue;
        }
        let (max, snr, off) = compare(&got, &want, ch);
        eprintln!(
            "{name}: {ch} ch {} Hz, max error {max:.3} LSB, SNR {snr:.1} dB, offset {off}, {errors} frames dropped",
            dec.sample_rate()
        );
        if max > MAX_LSB && !KNOWN.contains(&name.as_str()) {
            failed.push(format!("{name}: max error {max:.3} LSB"));
        }
        checked += 1;
    }
    eprintln!("{checked} streams compared with ffmpeg");
    assert!(failed.is_empty(), "{failed:#?}");
}
