//! The sound chain through [`AudioPlayer`] on a [`FakeOutput`], with generated files (no
//! corpus): gapless splicing across files (same and different rates), crossfades, the
//! equaliser's bypass and cut, balance and mono, and levelling from a ReplayGain tag.

use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::Source;
use specular_video::audio::{AudioPlayer, FakeOutput};
use specular_video::dsp::SoundSettings;
use specular_video::eq::EqSettings;
use specular_video::levelling::{Levelling, Mode, Source as LevelSource};

/// A 16-bit PCM WAV of interleaved `ch`-channel samples, with `extra` chunks after.
fn wav_with(rate: u32, ch: u16, samples: &[f32], extra: &[u8]) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len + extra.len() as u32).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&ch.to_le_bytes());
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * u32::from(ch) * 2).to_le_bytes());
    v.extend_from_slice(&(ch * 2).to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&q(*s).to_le_bytes());
    }
    v.extend_from_slice(extra);
    v
}

fn q(s: f32) -> i16 {
    (s.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

fn wav(rate: u32, ch: u16, samples: &[f32]) -> Vec<u8> {
    wav_with(rate, ch, samples, &[])
}

fn src(name: &str, bytes: Vec<u8>) -> Arc<Source> {
    Arc::new(Source::from_vec(name, bytes))
}

/// A stereo sine (`amp`, `f` Hz) from frame `from` for `n` frames at `rate`.
fn sine(rate: u32, f: f64, amp: f64, from: usize, n: usize) -> Vec<f32> {
    (from..from + n)
        .flat_map(|i| {
            let v = (amp * (std::f64::consts::TAU * f * i as f64 / f64::from(rate)).sin()) as f32;
            [v, v]
        })
        .collect()
}

fn open(s: &Arc<Source>, out: &FakeOutput, sound: &SoundSettings) -> AudioPlayer {
    specular_video::session::open_sound_with(Arc::clone(s), Box::new(out.clone()), sound).unwrap()
}

/// Plays until `frames` device frames are out (10 ms buffers), waiting for the decoder.
fn play_out(p: &AudioPlayer, out: &FakeOutput, frames: usize) -> u32 {
    play_until(p, out, frames).0
}

/// [`play_out`], also saying when the last buffer is heard.
fn play_until(p: &AudioPlayer, out: &FakeOutput, frames: usize) -> (u32, Instant) {
    let t0 = Instant::now();
    let mut advanced = 0;
    let mut k = 0;
    let mut last = t0;
    while out.played.lock().len() / 2 < frames {
        p.wait_queued(480);
        last = t0 + Duration::from_millis(10 * k);
        out.pull(480, last);
        advanced += p.take_advanced();
        k += 1;
    }
    (advanced, last)
}

#[test]
fn gapless_splice_is_seamless() {
    // One sine split in two files mid-cycle; the second half heard right after the first.
    let rate = 48_000;
    let (n1, n2) = (31_237, 24_000);
    let a = sine(rate, 441.0, 0.5, 0, n1);
    let b = sine(rate, 441.0, 0.5, n1, n2);
    let out = FakeOutput::new(rate, 2);
    let p = open(
        &src("a.wav", wav(rate, 2, &a)),
        &out,
        &SoundSettings::default(),
    );
    p.set_next(Some(src("b.wav", wav(rate, 2, &b))));
    assert!(p.has_next());
    p.play(Duration::ZERO);
    let (advanced, last) = play_until(&p, &out, n1 + 12_000);
    assert_eq!(advanced, 1, "the next file is heard once");
    let played = out.played.lock().clone();
    // Exactly the two files' samples back to back: no gap, no repeat, no click.
    let want: Vec<f32> = a
        .iter()
        .chain(&b)
        .map(|&s| f32::from(q(s)) / 32768.0)
        .collect();
    let mismatches = played
        .iter()
        .zip(&want)
        .filter(|(x, y)| x.to_bits() != y.to_bits())
        .count();
    assert_eq!(mismatches, 0, "of {} samples", played.len());
    // The clock runs in the second file now.
    let t = p.heard(last).unwrap();
    let heard_from = out.played.lock().len() / 2 - 480;
    let want = Duration::from_secs_f64((heard_from - n1) as f64 / 48_000.0);
    assert!(t.abs_diff(want) < Duration::from_millis(1), "{t:?}");
}

#[test]
fn gapless_through_the_resampler() {
    // The next file is at 44.1 kHz on a 48 kHz device: no silence, no jump at the join.
    let out = FakeOutput::new(48_000, 2);
    let n1 = 24_000;
    let a = sine(48_000, 300.0, 0.5, 0, n1);
    // The same sine continued, in the other file's rate and phase.
    let t1 = n1 as f64 / 48_000.0;
    let b: Vec<f32> = (0..22_050)
        .flat_map(|i| {
            let t = t1 + i as f64 / 44_100.0;
            let v = (0.5 * (std::f64::consts::TAU * 300.0 * t).sin()) as f32;
            [v, v]
        })
        .collect();
    let p = open(
        &src("a.wav", wav(48_000, 2, &a)),
        &out,
        &SoundSettings::default(),
    );
    p.set_next(Some(src("b.wav", wav(44_100, 2, &b))));
    p.play(Duration::ZERO);
    assert_eq!(play_out(&p, &out, n1 + 12_000), 1);
    let played = out.played.lock().clone();
    let left: Vec<f32> = played.iter().step_by(2).copied().collect();
    // A 300 Hz sine at 0.5 moves at most 0.0197 per frame; allow for the resampler.
    let worst = left
        .windows(2)
        .take(n1 + 10_000)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(worst < 0.03, "a jump of {worst}");
    // No gap: never two silent frames in a row at the join (the sine's own zeros are one).
    let gap = left[n1 - 200..n1 + 200]
        .windows(2)
        .any(|w| w[0].abs() < 1e-4 && w[1].abs() < 1e-4);
    assert!(!gap, "silence at the join");
}

#[test]
fn crossfade_overlaps_two_files() {
    let rate = 48_000;
    let a = sine(rate, 440.0, 0.5, 0, 48_000);
    let b = sine(rate, 660.0, 0.5, 0, 48_000);
    let out = FakeOutput::new(rate, 2);
    let s = SoundSettings {
        crossfade: Duration::from_millis(500),
        ..SoundSettings::default()
    };
    let p = open(&src("a.wav", wav(rate, 2, &a)), &out, &s);
    p.set_next(Some(src("b.wav", wav(rate, 2, &b))));
    p.play(Duration::ZERO);
    assert_eq!(play_out(&p, &out, 72_000), 1);
    let played = out.played.lock().clone();
    // Both tones under the overlap; the second file is heard from 0.5 s before the first
    // ends, so 1.5 s in it is 1 s into the second file: at its own amplitude, near its end.
    let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
    let mid = rms(&played[2 * 47_000..2 * 47_200]);
    assert!(mid > 0.2 && mid < 0.5, "{mid}");
    let total = 48_000 + 48_000 - 24_000;
    assert!(rms(&played[2 * (total - 2000)..2 * (total - 1000)]) > 0.3);
    // The fade has no clicks: no step larger than both tones could make.
    let worst = played
        .iter()
        .step_by(2)
        .collect::<Vec<_>>()
        .windows(2)
        .take(70_000)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(worst < 0.1, "{worst}");
}

#[test]
fn flat_equaliser_is_bit_identical_and_a_cut_cuts() {
    let rate = 48_000;
    let a = sine(rate, 1000.0, 0.5, 0, 24_000);
    let run = |s: &SoundSettings| {
        let out = FakeOutput::new(rate, 2);
        let p = open(&src("a.wav", wav(rate, 2, &a)), &out, s);
        p.play(Duration::ZERO);
        play_out(&p, &out, 19_200);
        out.played.lock().clone()
    };
    let plain = run(&SoundSettings::default());
    let flat = run(&SoundSettings {
        eq: Some(EqSettings::FLAT),
        ..SoundSettings::default()
    });
    assert!(
        plain
            .iter()
            .zip(&flat)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
    let mut g = [0.0; 10];
    g[5] = -12.0;
    let cut = run(&SoundSettings {
        eq: Some(EqSettings {
            bands: g,
            preamp: 0.0,
        }),
        ..SoundSettings::default()
    });
    let rms =
        |v: &[f32]| (v[9600..].iter().map(|s| s * s).sum::<f32>() / (v.len() - 9600) as f32).sqrt();
    let db = 20.0 * (rms(&cut) / rms(&plain)).log10();
    assert!((db + 12.0).abs() < 0.5, "{db}");
}

#[test]
fn mono_and_balance_place_the_sound() {
    let rate = 48_000;
    // Left only.
    let a: Vec<f32> = sine(rate, 500.0, 0.5, 0, 12_000)
        .chunks(2)
        .flat_map(|f| [f[0], 0.0])
        .collect();
    let out = FakeOutput::new(rate, 2);
    let s = SoundSettings {
        mono: true,
        balance: 0.5,
        ..SoundSettings::default()
    };
    let p = open(&src("a.wav", wav(rate, 2, &a)), &out, &s);
    p.play(Duration::ZERO);
    play_out(&p, &out, 4800);
    let v = out.played.lock().clone();
    for f in v.chunks(2).skip(10) {
        // Mono halves the left into both, then the balance halves the left again.
        assert!((f[0] - f[1] * 0.5).abs() < 1e-6, "{f:?}");
    }
}

#[test]
fn a_picked_track_has_the_sound_settings_from_its_first_buffer() {
    // The sound-track menu reopens the sound: the balance (all right) holds from the first
    // frame heard, not a few buffers later.
    let rate = 48_000;
    let a = sine(rate, 500.0, 0.5, 0, 12_000);
    let out = FakeOutput::new(rate, 2);
    let s = SoundSettings {
        balance: 1.0,
        ..SoundSettings::default()
    };
    let p = specular_video::session::open_sound_track_with(
        src("a.wav", wav(rate, 2, &a)),
        Box::new(out.clone()),
        None,
        &s,
    )
    .unwrap();
    p.play(Duration::ZERO);
    play_out(&p, &out, 960);
    let v = out.played.lock().clone();
    assert!(v.chunks(2).any(|f| f[1].abs() > 0.1), "sound is heard");
    let left = v.chunks(2).filter(|f| f[0] != 0.0).count();
    assert_eq!(left, 0, "frames heard on the left of {}", v.len() / 2);
}

#[test]
fn untagged_surround_is_measured_without_its_lfe() {
    // An untagged 5.1 file on a 5.1 device (ALSA order: the LFE sixth), levelling measured:
    // the loudness is the source's by speaker, so the LFE alone gives nothing to level and a
    // surround does.
    let rate = 48_000;
    let secs = 6;
    let measured = |k: usize| {
        let samples: Vec<f32> = (0..rate as usize * secs)
            .flat_map(|i| {
                let mut f = [0.0f32; 6];
                f[k] = (0.3 * (std::f64::consts::TAU * 997.0 * i as f64 / 48_000.0).sin()) as f32;
                f
            })
            .collect();
        let out = FakeOutput::new(rate, 6).with_order(specular_video::speakers::DeviceOrder::Alsa);
        let s = SoundSettings {
            levelling: Levelling {
                mode: Mode::Track,
                measure: true,
                ..Levelling::default()
            },
            ..SoundSettings::default()
        };
        let p = open(&src("s.wav", wav(rate, 6, &samples)), &out, &s);
        p.play(Duration::ZERO);
        // `play_out` counts pairs of samples: three per 5.1 frame.
        play_out(&p, &out, rate as usize * (secs - 1) * 3);
        p.levelling()
    };
    // L R C LFE Ls Rs.
    assert_eq!(measured(3), None, "the LFE alone is not measured");
    assert!(
        matches!(measured(4), Some((_, LevelSource::Measured))),
        "a surround is"
    );
}

/// An ID3v2.3 tag with one TXXX frame./// An ID3v2.3 tag with one TXXX frame.
fn id3_txxx(desc: &str, value: &str) -> Vec<u8> {
    let mut body = vec![0u8];
    body.extend_from_slice(desc.as_bytes());
    body.push(0);
    body.extend_from_slice(value.as_bytes());
    let mut frame = b"TXXX".to_vec();
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(&body);
    let n = frame.len();
    let mut tag = b"ID3\x03\x00\x00".to_vec();
    tag.extend_from_slice(&[
        (n >> 21) as u8 & 0x7f,
        (n >> 14) as u8 & 0x7f,
        (n >> 7) as u8 & 0x7f,
        n as u8 & 0x7f,
    ]);
    tag.extend_from_slice(&frame);
    tag
}

#[test]
fn replaygain_tag_levels_the_file() {
    let rate = 48_000;
    let a = sine(rate, 1000.0, 0.5, 0, 24_000);
    let tag = id3_txxx("REPLAYGAIN_TRACK_GAIN", "-6.0206 dB");
    let mut chunk = b"id3 ".to_vec();
    chunk.extend_from_slice(&(tag.len() as u32).to_le_bytes());
    chunk.extend_from_slice(&tag);
    if tag.len() % 2 == 1 {
        chunk.push(0);
    }
    let file = src("a.wav", wav_with(rate, 2, &a, &chunk));
    let tags = specular_video::sound_tags::read(file.bytes());
    assert_eq!(tags.gain.track_gain, Some(-6.0206));
    let out = FakeOutput::new(rate, 2);
    let s = SoundSettings {
        levelling: Levelling {
            mode: Mode::Track,
            ..Levelling::default()
        },
        ..SoundSettings::default()
    };
    let p = open(&file, &out, &s);
    p.play(Duration::ZERO);
    play_out(&p, &out, 9600);
    let v = out.played.lock().clone();
    let peak = v[4800..].iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!((peak - 0.25).abs() < 0.002, "{peak}");
    let (db, from) = p.levelling().unwrap();
    assert!((db + 6.0206).abs() < 1e-3 && from == LevelSource::Track);
}

/// One MP3 stream cut into two pieces at a frame boundary (as HLS cuts a stream): played
/// as pieces of one stream, one decoder goes on across the boundary and the sound is the
/// whole stream's, sample for sample; reopened per piece (as two files) it is not (the
/// bit reservoir and the filter bank start again).
#[test]
fn pieces_go_on_with_one_decoder() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/snd-pieces.mp3");
    let bytes = std::fs::read(path).unwrap();
    let movie = specular_video::parse_movie(&bytes, true).unwrap();
    let samples = &movie.audio.as_ref().unwrap().samples;
    // Cut after a third of the frames.
    let cut = samples[samples.len() / 3].offset as usize;
    let (a, b) = (bytes[..cut].to_vec(), bytes[cut..].to_vec());
    let rate = 44_100;
    let frames = 60_000;
    let run = |pieces: Option<bool>| {
        let out = FakeOutput::new(rate, 2);
        let first = if pieces.is_some() {
            a.clone()
        } else {
            bytes.clone()
        };
        let p = open(&src("p1.mp3", first), &out, &SoundSettings::default());
        match pieces {
            Some(true) => p.set_next_piece(Some(src("p2.mp3", b.clone()))),
            Some(false) => p.set_next(Some(src("p2.mp3", b.clone()))),
            None => {}
        }
        p.play(Duration::ZERO);
        let advanced = play_out(&p, &out, frames);
        let v = out.played.lock()[..2 * frames].to_vec();
        (v, advanced)
    };
    let (whole, _) = run(None);
    let (one, advanced) = run(Some(true));
    assert_eq!(advanced, 1, "the second piece is heard");
    let differ = |x: &[f32]| {
        x.iter()
            .zip(&whole)
            .filter(|(p, q)| p.to_bits() != q.to_bits())
            .count()
    };
    let (one_differ, reopened_differ) = (differ(&one), differ(&run(Some(false)).0));
    println!(
        "ORACLE\tSound/stream pieces\tMP3 cut at a frame\t{}\tone decoder: {one_differ} of {} samples differ from the whole stream; reopened: {reopened_differ}",
        if one_differ == 0 { "pass" } else { "fail" },
        whole.len()
    );
    assert_eq!(one_differ, 0, "one decoder: the whole stream's sound");
    assert!(reopened_differ > 0, "the test tells the two apart");
    // No gap: the join is never silent.
    let silent = whole.iter().filter(|s| s.abs() < 1e-6).count();
    assert!(silent < 4000, "{silent}");
}
