//! Real-world streams from FFmpeg's FATE suite against libavcodec's float decode, when
//! `SPECULAR_AC3_CORPUS` names the directory `tools/corpus/fetch-ac3.sh` filled (skipped
//! otherwise). Prints the error table and the speed.
//!
//! Both decoders fill zero-bit mantissas with random dither (and spectral extension with
//! random noise), so they can't agree beyond that noise: each stream is also decoded with
//! and without our dither, and the difference to libavcodec must stay within what our own
//! dither changes (the "noise floor" column), give or take 4 dB, or within 4 steps.
//!
//! One block per stream is left out: the first block whose channels mix long and short
//! transforms. libavcodec then "upmixes" its overlap buffer as if it had been downmixed
//! (its `downmixed` state starts set), so that block loses most of the previous block's
//! overlap in libavcodec's output; ours reconstructs it (`mdct` tests the transforms'
//! perfect reconstruction through block switches).

use std::path::Path;
use std::time::Instant;

use specular_ac3::{Decoder, Options};

fn f32s(path: &Path) -> Option<Vec<f32>> {
    let b = std::fs::read(path).ok()?;
    Some(
        b.chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
    )
}

/// (largest difference in 16-bit steps, signal-to-difference ratio in dB).
fn compare(got: &[f32], want: &[f32]) -> (f64, f64) {
    let (mut worst, mut sig, mut err) = (0f64, 0f64, 0f64);
    for (&g, &w) in got.iter().zip(want) {
        let d = f64::from(g) - f64::from(w);
        worst = worst.max(d.abs() * 32768.0);
        sig += f64::from(w).powi(2);
        err += d * d;
    }
    (worst, 10.0 * (sig / err.max(1e-30)).log10())
}

fn run(data: &[u8], stereo: bool, dither: bool) -> (Vec<f32>, Decoder, f64) {
    let mut d = Decoder::new(Options {
        stereo,
        levels: false,
        drc: 1.0,
        dither,
        lfe_mix: false,
    });
    let mut got = Vec::new();
    let t = Instant::now();
    d.decode(data, &mut got).unwrap();
    d.flush(&mut got).unwrap();
    (got, d, t.elapsed().as_secs_f64())
}

#[test]
fn fate_streams_match_libavcodec() {
    let Some(dir) = std::env::var_os("SPECULAR_AC3_CORPUS") else {
        eprintln!("SPECULAR_AC3_CORPUS not set: skipped");
        return;
    };
    let dir = Path::new(&dir);
    // (stream, compare every channel too)
    for (name, all) in [
        ("monsters_inc_2.0_192_small.ac3", true),
        ("monsters_inc_5.1_448_small.ac3", true),
        ("millers_crossing_4.0.ac3", true),
        ("matrix2_commentary1_stereo_192_small.eac3", true),
        ("serenity_english_5.1_1536_small.eac3", true),
        ("csi_miami_stereo_128_spx_small.eac3", true),
        ("csi_miami_5.1_256_spx_small.eac3", true),
        // libavcodec decodes the dependent substream too: compare the stereo folds
        ("the_great_wall_7.1.eac3", false),
    ] {
        let Ok(data) = std::fs::read(dir.join(name)) else {
            eprintln!("{name}: missing");
            continue;
        };
        let stem = name.rsplit_once('.').unwrap().0;
        for stereo in [false, true] {
            if !stereo && !all {
                continue;
            }
            let file = if stereo {
                format!("{stem}.st.f32")
            } else {
                format!("{stem}.f32")
            };
            let Some(want) = f32s(&dir.join(file)) else {
                continue;
            };
            let (got, d, secs) = run(&data, stereo, false);
            assert_eq!(got.len(), want.len(), "{name}: sample count");
            let (dithered, _, _) = run(&data, stereo, true);
            let info = d.info().unwrap();
            let media = got.len() as f64 / d.channels() as f64 / f64::from(info.sample_rate);
            // the last frame of the cut-down samples is incomplete: libavcodec makes something
            // of it, we play silence
            let body = got.len().saturating_sub(1536 * d.channels());
            let ch = d.channels();
            let (got, want, dithered) = (
                &got[..body],
                &want[..body.min(want.len())],
                &dithered[..body],
            );
            // the block with the largest difference
            let blk = 256 * ch;
            let worst_blk = (0..got.len() / blk)
                .max_by(|&a, &b| {
                    let e = |k: usize| compare(&got[k * blk..(k + 1) * blk], &want[k * blk..]).0;
                    e(a).total_cmp(&e(b))
                })
                .unwrap_or(0);
            let skip = worst_blk * blk..(worst_blk + 1) * blk;
            let keep = |v: &[f32]| -> Vec<f32> {
                v.iter()
                    .enumerate()
                    .filter(|(i, _)| !skip.contains(i))
                    .map(|(_, &x)| x)
                    .collect()
            };
            let (got, want, dithered) = (keep(got), keep(want), keep(dithered));
            let (worst, snr) = compare(&got, &want);
            let (_, floor) = compare(&dithered, &got);
            eprintln!(
                "{name} ({}{}): within {worst:.0} steps, {snr:.1} dB (noise floor {floor:.1} dB); \
                 {:.0}x real time",
                info.layout(),
                if stereo { " to stereo" } else { "" },
                media / secs
            );
            assert!(
                worst <= 4.0 || snr > floor - 4.0,
                "{name}: {snr:.1} dB against a floor of {floor:.1}"
            );
        }
    }
}

/// The AC-3 streams with their dither flags cleared ([`specular_ac3::without_dither`]),
/// decoded by ffmpeg when it is on PATH (into Cargo's test scratch directory): with no
/// random noise in either decoder, every channel agrees within a 16-bit step, the
/// difference far below full scale (the same block as above left out). This is what the
/// dithered comparison's 30 to 45 dB is made of: the two decoders' independent dither.
#[test]
fn undithered_fate_streams_match_libavcodec_exactly() {
    let Some(dir) = std::env::var_os("SPECULAR_AC3_CORPUS") else {
        eprintln!("SPECULAR_AC3_CORPUS not set: skipped");
        return;
    };
    let dir = Path::new(&dir);
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR"));
    for name in [
        "monsters_inc_2.0_192_small.ac3",
        "monsters_inc_5.1_448_small.ac3",
        "millers_crossing_4.0.ac3",
    ] {
        let Ok(data) = std::fs::read(dir.join(name)) else {
            eprintln!("{name}: missing");
            continue;
        };
        let nd = scratch.join(format!("{name}.nd"));
        let pcm = scratch.join(format!("{name}.nd.f32"));
        std::fs::write(&nd, specular_ac3::without_dither(&data)).unwrap();
        let ran = std::process::Command::new("ffmpeg")
            .args(["-nostdin", "-loglevel", "quiet", "-y", "-i"])
            .arg(&nd)
            .args(["-f", "f32le"])
            .arg(&pcm)
            .status();
        if !ran.is_ok_and(|s| s.success()) {
            eprintln!("ffmpeg not available: skipped");
            return;
        }
        let want = f32s(&pcm).unwrap();
        let (got, d, _) = run(&std::fs::read(&nd).unwrap(), false, false);
        let ch = d.channels();
        // the cut last frame and the block libavcodec's overlap loses, as above
        let body = got.len().saturating_sub(1536 * ch).min(want.len());
        let blk = 256 * ch;
        let worst_blk = (0..body / blk)
            .max_by(|&a, &b| {
                let e = |k: usize| {
                    compare(&got[k * blk..(k + 1) * blk], &want[k * blk..(k + 1) * blk]).0
                };
                e(a).total_cmp(&e(b))
            })
            .unwrap_or(0);
        let (mut worst, mut err) = (0f64, 0f64);
        for (i, (&g, &w)) in got[..body].iter().zip(&want[..body]).enumerate() {
            if i / blk == worst_blk {
                continue;
            }
            let e = f64::from(g) - f64::from(w);
            worst = worst.max(e.abs() * 32768.0);
            err += e * e;
        }
        let below = -10.0 * (err / body.max(1) as f64).max(1e-30).log10();
        eprintln!("{name} undithered: within {worst:.3} steps, {below:.1} dB below full scale");
        assert!(
            worst < 1.0 && below > 100.0,
            "{name}: {worst:.3} steps, {below:.1} dB"
        );
    }
}
