//! The AAC decoder (`specular-aac`) against ffmpeg, through the container demuxers: every
//! `NAME.mp4` / `NAME.m4a` with a `NAME.f32` (ffmpeg's float decode, interleaved, at the
//! stream's output rate) in `SPECULAR_AAC_CORPUS` (tools/corpus/fetch-aac.sh: the MPEG-4
//! conformance streams, AAC-LC, HE-AAC, HE-AAC v2) and `SPECULAR_AAC_GEN`
//! (tools/corpus/gen-aac.sh) is demuxed and decoded, and must agree with ffmpeg to the
//! error bound below. Prints the largest error and the SNR of each stream; streams the
//! decoder refuses (error resilient, SSR) are listed with the reason.
//! Skips without either corpus.

use specular_video::mp4::{self, AudioCodec};

const LSB: f64 = 1.0 / 32768.0;
/// Largest error allowed against ffmpeg's float decode, in 16-bit steps. Coupling channels
/// (the conformance stream al15) reach 2.1; everything else stays under 0.2.
const MAX_LSB: f64 = 2.5;

struct Outcome {
    max_lsb: f64,
    snr_db: f64,
    offset: isize,
}

/// Compares `got` with `want` (both interleaved, `ch` channels) at the frame offset that
/// matches best (the encoder's priming may or may not be in either).
fn compare(got: &[f32], want: &[f32], ch: usize) -> Outcome {
    let mut best: Option<Outcome> = None;
    for frames in [0usize, 1024, 2048, 2112] {
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
            let o = Outcome {
                max_lsb: max / LSB,
                snr_db: 10.0 * (sig / noise.max(1e-30)).log10(),
                offset: sign * frames as isize,
            };
            if best.as_ref().is_none_or(|b| o.max_lsb < b.max_lsb) {
                best = Some(o);
            }
        }
    }
    best.unwrap_or(Outcome {
        max_lsb: f64::INFINITY,
        snr_db: 0.0,
        offset: 0,
    })
}

/// ffmpeg's output in our channel layout: 3.0 (L R C) is the first three of our 5.1; 7.1
/// with left and right of centre (ffmpeg's FL FR FC LFE BL BR FLC FRC) folds the inner pair
/// into left and right at equal power. (A mono element in a stereo layout ffmpeg plays on
/// the left only, we in both; the left is compared alone.)
fn to_ours(want: &[f32], want_ch: usize, ch: usize) -> Option<(Vec<f32>, usize)> {
    if want_ch == ch {
        return Some((want.to_vec(), ch));
    }
    let frames = want.len() / want_ch;
    match (want_ch, ch) {
        (3, 6) => Some((
            (0..frames)
                .flat_map(|i| {
                    let f = &want[i * 3..i * 3 + 3];
                    [f[0], f[1], f[2], 0.0, 0.0, 0.0]
                })
                .collect(),
            6,
        )),
        (8, 6) => {
            let h = std::f32::consts::FRAC_1_SQRT_2;
            Some((
                (0..frames)
                    .flat_map(|i| {
                        let f = &want[i * 8..i * 8 + 8];
                        [(f[0] + f[6]) * h, (f[1] + f[7]) * h, f[2], f[3], f[4], f[5]]
                    })
                    .collect(),
                6,
            ))
        }
        _ => None,
    }
}

#[test]
fn aac_matches_ffmpeg() {
    let dirs: Vec<_> = ["SPECULAR_AAC_CORPUS", "SPECULAR_AAC_GEN"]
        .iter()
        .filter_map(std::env::var_os)
        .collect();
    if dirs.is_empty() {
        eprintln!("skipped: set SPECULAR_AAC_CORPUS / SPECULAR_AAC_GEN (tools/corpus/)");
        return;
    }
    let mut names: Vec<_> = dirs
        .iter()
        .flat_map(|d| std::fs::read_dir(d).unwrap().flatten().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "mp4" || e == "m4a"))
        .collect();
    names.sort();
    let (mut checked, mut refused, mut failed) = (0, Vec::new(), Vec::new());
    for path in names {
        let Ok(want) = std::fs::read(path.with_extension("f32")) else {
            continue;
        };
        let want: Vec<f32> = want
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(&path).unwrap();
        let movie = match mp4::parse(&data, true) {
            Ok(m) => m,
            Err(e) => {
                refused.push(format!("{name}: demux: {e}"));
                continue;
            }
        };
        let Some(a) = movie.audio.as_ref() else {
            refused.push(format!("{name}: no sound track"));
            continue;
        };
        let AudioCodec::Aac(asc) = &a.codec else {
            refused.push(format!("{name}: {}", a.codec.name()));
            continue;
        };
        let first: Vec<&[u8]> = a
            .samples
            .iter()
            .take(48)
            .map(|s| mp4::sample_bytes(&data, s).unwrap())
            .collect();
        let opts = specular_aac::Options {
            stereo_downmix: false,
        };
        let mut dec = match specular_aac::StreamDecoder::new(asc, &first, opts) {
            Ok(d) => d,
            Err(e) => {
                refused.push(format!("{name}: {e}"));
                continue;
            }
        };
        let mut got = Vec::new();
        for s in &a.samples {
            let b = mp4::sample_bytes(&data, s).unwrap();
            if let Err(e) = dec.decode(b, &mut got) {
                failed.push(format!("{name}: {e}"));
                break;
            }
        }
        let ch = usize::from(dec.channels().max(1));
        let frames = got.len() / ch;
        // ffmpeg's channel count from its output length
        let want_ch = (1..=8)
            .min_by_key(|&c| (want.len() / c).abs_diff(frames))
            .unwrap_or(ch);
        let right_silent = want_ch == 2 && want.iter().skip(1).step_by(2).all(|&v| v == 0.0);
        let (o, how) = match to_ours(&want, want_ch, ch) {
            Some((w, c)) if !right_silent => (compare(&got, &w, c), format!("{c} ch")),
            _ => {
                // ffmpeg plays a lone mono element of a stereo layout on the left only
                let left: Vec<f32> = got.iter().step_by(ch).copied().collect();
                let wl: Vec<f32> = want.iter().step_by(want_ch).copied().collect();
                (compare(&left, &wl, 1), format!("{ch} ch, left only"))
            }
        };
        eprintln!(
            "{name} ({}): {how}, {} Hz, max error {:.3} LSB, SNR {:.1} dB (offset {} frames)",
            dec.config().name(),
            dec.sample_rate(),
            o.max_lsb,
            o.snr_db,
            o.offset
        );
        if o.max_lsb > MAX_LSB {
            failed.push(format!("{name}: max error {:.2} LSB", o.max_lsb));
        }
        checked += 1;
    }
    for r in &refused {
        eprintln!("not decoded: {r}");
    }
    eprintln!(
        "{checked} streams compared with ffmpeg, {} not decoded",
        refused.len()
    );
    assert!(failed.is_empty(), "{failed:#?}");
}
