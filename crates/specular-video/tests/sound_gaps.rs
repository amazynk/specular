//! Sound gaps (format gaps item 12) against ffmpeg's decodes (tests/data, made by
//! tests/data/gen_fixtures_sound_gaps.sh): DVD LPCM at 20 and 24 bits must match sample for
//! sample; lossy codings within a stated number of 16-bit steps.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::Source;

use specular_video::audio;
use specular_video::mp4::AudioTrack;

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// The file's sound track and the file.
fn track(name: &str) -> (AudioTrack, Arc<Source>) {
    let src = Arc::new(Source::open(data(name)).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    (movie.audio.expect("a sound track"), src)
}

/// The whole track decoded: interleaved samples, channels, rate.
fn decode(name: &str) -> (Vec<f32>, u16, u32) {
    let (t, src) = track(name);
    audio::decode_track(&t, &src).unwrap()
}

/// ffmpeg's decode: its `.s32` when there is one, else its `.pcm` (s16); with its bits.
fn oracle(name: &str) -> (Vec<i64>, u32) {
    let stem = name.rsplit_once('.').unwrap().0;
    match std::fs::read(data(&format!("{stem}.s32"))) {
        Ok(b) => (
            b.chunks_exact(4)
                .map(|c| i64::from(i32::from_le_bytes([c[0], c[1], c[2], c[3]])))
                .collect(),
            32,
        ),
        Err(_) => (
            std::fs::read(data(&format!("{stem}.pcm")))
                .unwrap()
                .chunks_exact(2)
                .map(|c| i64::from(i16::from_le_bytes([c[0], c[1]])))
                .collect(),
            16,
        ),
    }
}

/// The decode against ffmpeg's at the best lead (within `max_lead` frames either way, as
/// decoders differ in priming): the largest difference in 16-bit steps, the lead, and the
/// number of samples compared.
fn best(name: &str, max_lead: usize) -> (f64, i64, usize) {
    let (pcm, ch, rate) = decode(name);
    let ch = usize::from(ch);
    let (want, bits) = oracle(name);
    let step = (1u64 << (bits - 16)) as f64;
    let scale = (1u64 << (bits - 1)) as f64;
    let got: Vec<f64> = pcm
        .iter()
        .map(|&v| (f64::from(v) * scale).round().clamp(-scale, scale - 1.0) / step)
        .collect();
    let want: Vec<f64> = want.iter().map(|&v| v as f64 / step).collect();
    let (worst, lead, n) = (0..=max_lead)
        .flat_map(|d| [d as i64, -(d as i64)])
        .map(|d| {
            let (g, w) = if d >= 0 {
                (got.get(d as usize * ch..).unwrap_or_default(), &want[..])
            } else {
                (&got[..], want.get((-d) as usize * ch..).unwrap_or_default())
            };
            let n = g.len().min(w.len());
            let worst = g[..n]
                .iter()
                .zip(&w[..n])
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);
            (
                if n * 10 < want.len() * 9 {
                    f64::MAX
                } else {
                    worst
                },
                d,
                n,
            )
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap();
    eprintln!(
        "{name}: {ch} ch {rate} Hz, {} of ffmpeg's {} samples; within {worst:.2} steps over {n} from lead {lead}",
        got.len(),
        want.len()
    );
    (worst, lead, n)
}

fn exact_24(name: &str) -> (usize, usize) {
    let (pcm, _, _) = decode(name);
    let (want, _) = oracle(name);
    let got: Vec<i64> = pcm
        .iter()
        .map(|&v| (f64::from(v) * 2_147_483_648.0) as i64)
        .collect();
    assert_eq!(got.len(), want.len(), "{name}: sample count");
    if std::env::var_os("GAPS_DEBUG").is_some() {
        eprintln!("{name}: got {:?}", &got[..12]);
        eprintln!("{name}: want {:?}", &want[..12]);
    }
    (
        got.len(),
        got.iter().zip(&want).filter(|(a, b)| a != b).count(),
    )
}

#[test]
fn dvd_lpcm_20_and_24_bit_match_ffmpeg_exactly() {
    for name in [
        "lpcm24.vob",
        "lpcm24-96.vob",
        "lpcm20.vob",
        "lpcm24-odd.vob",
    ] {
        let (a, _) = track(name);
        assert!(audio::playable(&a).is_ok(), "{name}: {:?}", a.codec);
        let (n, bad) = exact_24(name);
        eprintln!("{name}: {n} samples, {bad} differ");
        assert_eq!(bad, 0, "{name}");
    }
}

#[test]
fn free_format_mpeg_audio_matches_ffmpeg() {
    let mut failed = Vec::new();
    for (name, tol) in [
        ("free-mp2.mp2", 1.0),
        ("free-mp3.mp3", 2.0),
        ("free-mp3lsf.mp3", 2.0),
        ("free-mkv.mkv", 2.0),
    ] {
        let (worst, _, _) = best(name, 2400);
        if worst > tol {
            failed.push(format!("{name}: {worst:.2} steps"));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn flash_adpcm_matches_ffmpeg_exactly() {
    for name in ["swf-mono.flv", "swf-stereo.flv"] {
        let (a, _) = track(name);
        assert!(audio::playable(&a).is_ok(), "{name}");
        let (worst, lead, n) = best(name, 0);
        assert!(
            worst == 0.0 && lead == 0 && n > 4000,
            "{name}: {worst} steps"
        );
    }
}

/// Signal-to-error ratio in dB of the decode against ffmpeg's (16-bit), at lead 0.
fn snr(name: &str) -> f64 {
    let (pcm, _, _) = decode(name);
    let (want, bits) = oracle(name);
    let scale = (1u64 << (bits - 1)) as f64;
    let n = pcm.len().min(want.len());
    assert!(
        n + 512 >= want.len() && n > 0,
        "{name}: {} of {}",
        pcm.len(),
        want.len()
    );
    let (mut sig, mut err) = (0.0, 0.0);
    for i in 0..n {
        let w = want[i] as f64 / scale;
        sig += w * w;
        err += (f64::from(pcm[i]) - w).powi(2);
    }
    let db = 10.0 * (sig / err.max(1e-30)).log10();
    eprintln!("{name}: {n} samples, {db:.1} dB against ffmpeg");
    db
}

/// Nellymoser against ffmpeg: the coded lines match; the noise lines have random signs, so
/// the bar is a signal-to-error ratio.
#[test]
fn nellymoser_matches_ffmpeg() {
    for name in ["nelly-8k.flv", "nelly-16k.flv", "nelly-44k.flv"] {
        let (a, _) = track(name);
        assert!(audio::playable(&a).is_ok(), "{name}: {:?}", a.codec);
        assert_eq!(a.channels, 1);
        let db = snr(name);
        assert!(db > 25.0, "{name}: {db:.1} dB");
    }
}

/// Speex against libspeex (ffmpeg's `-c:a libspeex`), narrowband to ultra-wideband, stereo
/// and FLV: within a few 16-bit steps (both run libspeex's floating-point arithmetic; the
/// oracle is rounded to 16 bits).
#[test]
fn speex_matches_libspeex() {
    let mut failed = Vec::new();
    for name in [
        "spx-nb4.ogg",
        "spx-nb8.ogg",
        "spx-nb10.ogg",
        "spx-wb.ogg",
        "spx-uwb.ogg",
        "spx-stereo.ogg",
        "spx.flv",
    ] {
        let (a, _) = track(name);
        assert!(audio::playable(&a).is_ok(), "{name}: {:?}", a.codec);
        let (worst, lead, _) = best(name, 0);
        let db = snr(name);
        if worst > 4.0 || lead != 0 || db < 60.0 {
            failed.push(format!("{name}: {worst:.2} steps, {db:.1} dB"));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

/// The new decoders run well ahead of real time (even in a test build, on a loaded machine):
/// each fixture decoded 20 times.
#[test]
fn new_decoders_run_faster_than_real_time() {
    for name in [
        "spx-uwb.ogg",
        "spx-nb10.ogg",
        "nelly-44k.flv",
        "swf-stereo.flv",
        "lpcm24.vob",
        "free-mp3.mp3",
        "opus-foa6.ogg",
    ] {
        let (t, src) = track(name);
        let start = std::time::Instant::now();
        let mut frames = 0usize;
        let mut rate = 1;
        for _ in 0..20 {
            let (pcm, ch, r) = audio::decode_track(&t, &src).unwrap();
            frames += pcm.len() / usize::from(ch.max(1));
            rate = r;
        }
        let secs = start.elapsed().as_secs_f64();
        let x = frames as f64 / f64::from(rate) / secs.max(1e-9);
        eprintln!("{name}: {x:.0}× real time");
        assert!(x > 2.0, "{name}: {x:.1}× real time");
    }
}

/// Opus mapping families 2, 3 and 255 against libopus, rendered to stereo the player's way.
#[test]
fn opus_ambisonics_and_unmapped_families_match_libopus() {
    let mut failed = Vec::new();
    for name in [
        "opus-foa.ogg",
        "opus-foa6.ogg",
        "opus-fam3.ogg",
        "opus-255.ogg",
    ] {
        let (a, _) = track(name);
        assert!(audio::playable(&a).is_ok(), "{name}");
        // The decode keeps the encoder's pre-skip (312 samples); libopus drops it.
        let (worst, lead, _) = best(name, 960);
        if worst > 1.0 || lead != 312 {
            failed.push(format!("{name}: {worst:.2} steps from lead {lead}"));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

/// OGM sound (fixtures from gen_fixtures_vgaps3.sh, FFmpeg's decode of each `.ogm`): PCM
/// timed by packet size matches exactly, MP3 frames behind two length bytes within 2 steps.
#[test]
fn ogm_sound_matches_ffmpeg() {
    let (t, _) = track("vg3-ogm-pcm.ogm");
    assert!(audio::playable(&t).is_ok(), "{:?}", t.codec);
    assert_eq!((t.channels, t.sample_rate), (2, 44_100));
    assert_eq!(t.duration, 13_230);
    let (worst, lead, _) = best("vg3-ogm-pcm.ogm", 0);
    assert_eq!((worst, lead), (0.0, 0));
    let (t, _) = track("vg3-ogm-mp3.ogm");
    assert!(audio::playable(&t).is_ok(), "{:?}", t.codec);
    assert_eq!(t.duration, 1152 * u64::from(t.sample_count));
    let (worst, _, _) = best("vg3-ogm-mp3.ogm", 2400);
    assert!(worst <= 2.0, "{worst:.2} steps");
}

/// A real DirectShow OGM with AC-3 sound (tools/corpus/fetch-ogm.sh) against ffmpeg's decode:
/// every whole frame (ffmpeg also conceals the cut last one), at the signal-to-difference
/// ratio the AC-3 corpus test accepts for the two decoders' independent dither (30 dB).
#[test]
#[ignore = "needs tools/corpus/fetch-ogm.sh"]
fn ogm_corpus() {
    let dir = std::env::var_os("SPECULAR_OGM_CORPUS").map_or_else(
        || Path::new(option_env!("CARGO_TARGET_DIR").unwrap_or("../../target")).join("corpus/ogm"),
        PathBuf::from,
    );
    let path = dir.join("bots01.ogm");
    if !path.exists() {
        eprintln!("skipped: run tools/corpus/fetch-ogm.sh");
        return;
    }
    let src = Arc::new(Source::open(&path).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let t = movie.audio.expect("a sound track");
    assert_eq!((t.channels, t.sample_rate), (2, 48_000));
    let (pcm, _, _) = audio::decode_track(&t, &src).unwrap();
    let want: Vec<f64> = std::fs::read(dir.join("bots01.pcm"))
        .unwrap()
        .chunks_exact(2)
        .map(|c| f64::from(i16::from_le_bytes([c[0], c[1]])))
        .collect();
    let got: Vec<f64> = pcm
        .iter()
        .map(|&a| (f64::from(a) * 32768.0).round().clamp(-32768.0, 32767.0))
        .collect();
    let n = got.len().min(want.len());
    // Our decode applies the stream's dialogue normalisation, ffmpeg's does not: one gain
    // for the whole track (0.631, −4 dB, on this file) is fitted first.
    let signal: f64 = want[..n].iter().map(|w| w * w).sum();
    let gain = got[..n]
        .iter()
        .zip(&want[..n])
        .map(|(g, w)| g * w)
        .sum::<f64>()
        / signal;
    let noise: f64 = got[..n]
        .iter()
        .zip(&want[..n])
        .map(|(g, w)| (g - gain * w).powi(2))
        .sum();
    let snr = 10.0 * (signal * gain * gain / noise.max(1.0)).log10();
    println!(
        "ORACLE\tOGM AC-3\tbots01.ogm\t{} of {} samples, gain {gain:.4}, {snr:.1} dB",
        got.len(),
        want.len()
    );
    assert!(want.len() - n <= 1536 * 2, "{n} of {} samples", want.len());
    assert!(
        (0.5..=1.01).contains(&gain) && snr > 30.0,
        "gain {gain:.4}, {snr:.1} dB"
    );
}
