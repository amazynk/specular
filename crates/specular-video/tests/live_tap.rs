//! The live tap through the player: what the device is handed is what the tap holds, at the
//! clock's time, after the whole chain (volume and mute included); paused or seeking, the
//! tap has nothing. Generated WAVs played on a [`FakeOutput`].

use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::Source;
use specular_video::audio::{AudioPlayer, FakeOutput};

/// A 16-bit PCM WAV of interleaved stereo samples at 48 kHz.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&48_000u32.to_le_bytes());
    v.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
    v.extend_from_slice(&4u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let q = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        v.extend_from_slice(&q.to_le_bytes());
    }
    v
}

/// Two seconds: a 440 Hz sine left, the same turned over right.
fn signal() -> Vec<f32> {
    (0..96_000)
        .flat_map(|i| {
            let v = 0.5 * (std::f32::consts::TAU * 440.0 * i as f32 / 48_000.0).sin();
            [v, -v]
        })
        .collect()
}

fn player(out: &FakeOutput) -> AudioPlayer {
    let src = Arc::new(Source::from_vec("tap.wav", wav(&signal())));
    specular_video::session::open_sound(src, Box::new(out.clone())).unwrap()
}

#[test]
fn the_tap_holds_what_is_heard_at_the_clock() {
    let out = FakeOutput::new(48_000, 2);
    let p = player(&out);
    let tap = p.tap();
    let mut got = Vec::new();
    assert!(!tap.is_live());
    p.play(Duration::ZERO);
    p.set_volume(0.5);
    let t0 = Instant::now();
    for k in 0..20u64 {
        assert!(p.wait_queued(2_000));
        out.pull(480, t0 + Duration::from_millis(10 * k));
    }
    assert!(tap.is_live());
    // At the clock's time the tap's newest frames are the last buffer's, as played.
    let now = t0 + Duration::from_millis(200);
    let t = p.heard(now).unwrap().as_secs_f64();
    let h = tap.read_at(t, 480, &mut got).unwrap();
    assert_eq!((h.channels, h.rate, h.frames), (2, 48_000, 480));
    let played = out.played.lock().clone();
    let last = &played[played.len() - 960..];
    let gain = specular_video::audio::gain_of(0.5);
    assert_eq!(&got[..], last);
    // The sound itself: frame 9120 onwards of the file.
    let sig = signal();
    for (i, v) in got.iter().enumerate() {
        let want = gain * (sig[9120 * 2 + i] * 32767.0).round() / 32767.0;
        assert!((v - want).abs() < 1e-4, "{i}: {v} {want}");
    }
    // Out of phase.
    assert!(
        specular_video::stereo::correlation_of(&got, 2).unwrap() < -0.99,
        "left and right turned over"
    );
    // Muted: what is heard is silence.
    p.set_muted(true);
    assert!(p.wait_queued(2_000));
    out.pull(480, t0 + Duration::from_millis(200));
    let t = p
        .heard(t0 + Duration::from_millis(210))
        .unwrap()
        .as_secs_f64();
    tap.read_at(t, 480, &mut got).unwrap();
    assert!(got.iter().all(|v| *v == 0.0));
    p.set_muted(false);
    p.set_volume(1.0);
    // Paused: nothing.
    p.pause();
    out.pull(480, t0 + Duration::from_millis(210));
    assert!(!tap.is_live());
    assert_eq!(tap.read_at(t, 480, &mut got), None);
    // A seek: nothing until heard there, then the new place only.
    p.seek(Duration::from_secs(1));
    p.play(Duration::from_secs(1));
    assert!(p.wait_queued(2_000));
    out.pull(480, t0 + Duration::from_millis(300));
    let t = p
        .heard(t0 + Duration::from_millis(310))
        .unwrap()
        .as_secs_f64();
    assert!((t - 1.01).abs() < 1e-3, "{t}");
    assert!(tap.read_at(t, 960, &mut got).is_none());
    let h = tap.read_at(t, 480, &mut got).unwrap();
    assert_eq!(h.frames, 480);
    let want = (sig[48_000 * 2] * 32767.0).round() / 32767.0;
    assert!((got[0] - want).abs() < 1e-4);
}
