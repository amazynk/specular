//! LOAS/LATM streams against ffmpeg: every `NAME.loas` (the LOAS elementary stream, as
//! `ffmpeg -c copy -f data` takes it out of a transport stream) with ffmpeg's float decode
//! `NAME.f32` and its channel count `NAME.ch` in `SPECULAR_AAC_LATM` (tools/corpus/fetch-aac.sh)
//! is decoded in odd-sized pieces, as a demuxer hands them over, and must agree with ffmpeg
//! to a tenth of a 16-bit step. The first two frames are left out: the broadcast samples are
//! cut from longer recordings, and ffmpeg decodes the elements before the first
//! StreamMuxConfig with a configuration its demuxer found (we skip them), so their overlap
//! differs. Skips without the corpus.

use specular_aac::{Options, StreamDecoder};

const LSB: f64 = 1.0 / 32768.0;
const MAX_LSB: f64 = 0.15;

/// Streams whose configuration changes to more channels than the output (stereo): from
/// that sample on, we fold to stereo normalised (BS.775 scaled by 1/(1 + √2) so nothing
/// clips) and ffmpeg's command line unnormalised, so ours is scaled back there.
const SWITCH_TO_SURROUND: [(&str, usize); 1] = [("latm_stereo_to_51", 144 * 1024)];

/// Largest error (16-bit steps) and SNR at the frame offset that matches best.
fn compare(got: &[f32], want: &[f32], ch: usize) -> (f64, f64, isize) {
    let mut best = (f64::INFINITY, 0.0, 0isize);
    for frames in (0..48).map(|f| f * 1024) {
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
fn latm_streams_match_ffmpeg() {
    let Some(dir) = std::env::var_os("SPECULAR_AAC_LATM") else {
        eprintln!("skipped: set SPECULAR_AAC_LATM (tools/corpus/fetch-aac.sh)");
        return;
    };
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "loas"))
        .collect();
    names.sort();
    let mut failed = Vec::new();
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
        // pieces of a prime size, so elements straddle them
        let pieces: Vec<&[u8]> = data.chunks(1499).collect();
        let mut dec = match StreamDecoder::new(&[], &pieces[..pieces.len().min(48)], {
            Options {
                stereo_downmix: true,
            }
        }) {
            Ok(d) => d,
            Err(e) => {
                failed.push(format!("{name}: {e}"));
                continue;
            }
        };
        dec.set_concealment(false);
        let mut got = Vec::new();
        let mut errors = 0;
        for p in &pieces {
            if dec.decode(p, &mut got).is_err() {
                errors += 1;
            }
        }
        let ch = usize::from(dec.channels());
        if std::env::var_os("SPECULAR_AAC_DUMP").is_some() {
            let bytes: Vec<u8> = got.iter().flat_map(|v| v.to_le_bytes()).collect();
            std::fs::write(path.with_extension("ours.f32"), bytes).unwrap();
        }
        if ch != want_ch {
            failed.push(format!("{name}: {ch} channels, ffmpeg {want_ch}"));
            continue;
        }
        if let Some(&(_, at)) = SWITCH_TO_SURROUND.iter().find(|s| s.0 == name) {
            for v in got.iter_mut().skip(at * ch) {
                *v *= 1.0 + std::f32::consts::SQRT_2;
            }
        }
        let skip = 2 * dec.frame_len() * ch;
        let (max, snr, off) = compare(got.get(skip..).unwrap_or(&[]), &want, ch);
        eprintln!(
            "{name} ({}, now {} channels): {ch} ch {} Hz, max error {max:.3} LSB, SNR {snr:.1} dB, offset {off}, {errors} pieces with errors",
            dec.config().name(),
            dec.config().core_channels(),
            dec.sample_rate()
        );
        if max > MAX_LSB {
            failed.push(format!("{name}: max error {max:.3} LSB"));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}
