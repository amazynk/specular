//! Sound at playback speeds other than 1×: generated WAVs (no corpus files) played through
//! [`AudioPlayer`] on a [`FakeOutput`]. 1× is bit-identical to the player before the
//! time-stretcher; other speeds keep the pitch, scale the length and keep the clock on what
//! is heard.

use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::Source;
use specular_video::audio::{AudioPlayer, FakeOutput};

/// A 16-bit PCM WAV of interleaved `ch`-channel samples.
fn wav(rate: u32, ch: u16, samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
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
        let q = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        v.extend_from_slice(&q.to_le_bytes());
    }
    v
}

fn player(rate: u32, ch: u16, samples: &[f32], out: &FakeOutput) -> AudioPlayer {
    let src = Arc::new(Source::from_vec("tempo.wav", wav(rate, ch, samples)));
    specular_video::session::open_sound(src, Box::new(out.clone())).unwrap()
}

/// Plays `buffers` buffers of `frames`, 10 ms apart from `t0`, waiting for the decoder
/// before each so none runs short.
fn pull(
    p: &AudioPlayer,
    out: &FakeOutput,
    t0: Instant,
    from: usize,
    buffers: usize,
    frames: usize,
) {
    for k in from..from + buffers {
        p.wait_queued(12_000);
        out.pull(frames, t0 + Duration::from_millis(10 * k as u64));
    }
}

fn fnv(v: &[f32]) -> u64 {
    v.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, s| {
        (h ^ u64::from(s.to_bits())).wrapping_mul(0x0100_0000_01b3)
    })
}

/// 44.1 kHz stereo (a 440 Hz sine left, a chirp right) resampled to a 48 kHz device.
fn identity_signal() -> Vec<f32> {
    let rate = 44_100.0f32;
    (0..44_100)
        .flat_map(|i| {
            let t = i as f32 / rate;
            let l = 0.5 * (std::f32::consts::TAU * 440.0 * t).sin();
            let r = 0.4 * (std::f32::consts::TAU * (200.0 + 1500.0 * t) * t).sin();
            [l, r]
        })
        .collect()
}

/// What the player played at 1× before the time-stretcher went in, on Linux: the signal and
/// the resampler's filter come from the C library's `sin`/`cos`, whose last bits differ
/// between platforms.
const IDENTITY_HASH: u64 = 0xd04a_5903_53f5_0b75;

/// The hash of what the player plays at 1×.
fn identity_hash() -> u64 {
    let out = FakeOutput::new(48_000, 2);
    let p = player(44_100, 2, &identity_signal(), &out);
    p.play(Duration::ZERO);
    pull(&p, &out, Instant::now(), 0, 110, 480);
    let played = out.played.lock().clone();
    assert!(played.iter().any(|&v| v != 0.0));
    fnv(&played)
}

#[test]
fn one_x_is_bit_identical() {
    let h = identity_hash();
    eprintln!("identity hash {h:#x}");
    if cfg!(target_os = "linux") {
        assert_eq!(h, IDENTITY_HASH);
    } else {
        // Elsewhere the reference bits are not known: only that 1× plays the same bits every
        // time. Linux CI checks the reference.
        assert_eq!(identity_hash(), h);
    }
}

const RATE: u32 = 48_000;
const SPEEDS: [f64; 5] = [0.5, 0.75, 1.25, 1.5, 2.0];

fn sine(secs: f32, f: f32, amp: f32) -> Vec<f32> {
    (0..(secs * RATE as f32) as usize)
        .map(|i| amp * (std::f32::consts::TAU * f * i as f32 / RATE as f32).sin())
        .collect()
}

/// Something like voiced speech: a pulse train gliding 100–160 Hz through two formant
/// resonators (700 and 1200 Hz), then a chirp 100 Hz to 4 kHz.
fn speechy(secs: f32) -> Vec<f32> {
    let n = (secs * RATE as f32) as usize;
    let mut out = Vec::with_capacity(n);
    let mut phase = 0.0f32;
    let res = |f: f32, bw: f32| {
        let r = (-std::f32::consts::PI * bw / RATE as f32).exp();
        let c = 2.0 * r * (std::f32::consts::TAU * f / RATE as f32).cos();
        (c, -r * r)
    };
    let (a1, b1) = res(700.0, 90.0);
    let (a2, b2) = res(1200.0, 110.0);
    let (mut y1, mut y11, mut y2, mut y22) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let half = secs / 2.0;
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let v = if t < half {
            let f0 = 100.0 + 60.0 * (t / half);
            phase += f0 / RATE as f32;
            let pulse = if phase >= 1.0 {
                phase -= 1.0;
                1.0
            } else {
                0.0
            };
            let o1 = pulse + a1 * y1 + b1 * y11;
            y11 = y1;
            y1 = o1;
            let o2 = o1 + a2 * y2 + b2 * y22;
            y22 = y2;
            y2 = o2;
            o2
        } else {
            let u = t - half;
            let k = (4000.0 - 100.0) / half;
            (std::f32::consts::TAU * (100.0 * u + 0.5 * k * u * u)).sin()
        };
        out.push(v);
    }
    // Both halves at about the same level.
    let (a, b) = out.split_at_mut(n / 2);
    for part in [a, b] {
        let peak = part.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        part.iter_mut().for_each(|v| *v *= 0.6 / peak);
    }
    out
}

/// Plays mono `x` at `speed` on a 48 kHz stereo device to the end; the left channel.
fn play_all(x: &[f32], speed: f64) -> Vec<f32> {
    let out = FakeOutput::new(RATE, 2);
    let p = player(RATE, 1, x, &out);
    p.set_speed(speed);
    p.play(Duration::ZERO);
    let t0 = Instant::now();
    let limit = (x.len() as f64 / speed / 480.0) as usize + 200;
    let mut quiet = 0;
    for k in 0..limit {
        let more = p.wait_queued(12_000);
        let before = out.played.lock().len();
        out.pull(480, t0 + Duration::from_millis(10 * k as u64));
        let silent = out.played.lock()[before..].iter().all(|&v| v == 0.0);
        quiet = if !more && silent { quiet + 1 } else { 0 };
        if quiet > 20 {
            break;
        }
    }
    let played = out.played.lock().clone();
    played.chunks_exact(2).map(|f| f[0]).collect()
}

/// The strongest frequency of `x` between `lo` and `hi` Hz (a DFT scanned in 0.05 Hz steps).
fn peak_hz(x: &[f32], lo: f64, hi: f64) -> f64 {
    let mut best = (lo, 0.0);
    let mut f = lo;
    while f <= hi {
        let w = std::f64::consts::TAU * f / f64::from(RATE);
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, &v) in x.iter().enumerate() {
            let a = w * i as f64;
            re += f64::from(v) * a.cos();
            im -= f64::from(v) * a.sin();
        }
        let m = re * re + im * im;
        if m > best.1 {
            best = (f, m);
        }
        f += 0.05;
    }
    best.0
}

/// Where before/after WAVs for listening go (under the target dir), when set.
fn listen_dir() -> Option<std::path::PathBuf> {
    let d = std::path::PathBuf::from(std::env::var_os("CARGO_TARGET_DIR")?).join("tempo-listen");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

fn save(name: &str, x: &[f32]) {
    if let Some(d) = listen_dir() {
        let _ = std::fs::write(d.join(name), wav(RATE, 1, x));
    }
}

#[test]
fn sine_keeps_pitch_and_length_scales() {
    let x = sine(2.0, 440.0, 0.5);
    save("sine-440-1x.wav", &x);
    for speed in SPEEDS {
        let y = play_all(&x, speed);
        save(&format!("sine-440-{speed}x.wav"), &y);
        let first = y.iter().position(|v| v.abs() > 0.01).unwrap();
        let last = y.iter().rposition(|v| v.abs() > 0.01).unwrap();
        let len = (last - first + 1) as f64;
        let want = x.len() as f64 / speed;
        assert!(
            (len / want - 1.0).abs() < 0.01,
            "{speed}×: {len} frames, want {want}"
        );
        let mid = first + (last - first) / 2;
        let f = peak_hz(&y[mid - 12_000..mid + 12_000], 400.0, 480.0);
        assert!((f / 440.0 - 1.0).abs() < 0.01, "{speed}×: peak {f} Hz");
        eprintln!(
            "{speed}×: length {:.4} of 1/speed, peak {f:.2} Hz",
            len / want
        );
    }
}

#[test]
fn speech_has_no_dropouts() {
    let x = speechy(3.0);
    save("speechy-1x.wav", &x);
    for speed in SPEEDS {
        let y = play_all(&x, speed);
        save(&format!("speechy-{speed}x.wav"), &y);
        let first = y.iter().position(|v| v.abs() > 1e-4).unwrap();
        let last = y.iter().rposition(|v| v.abs() > 1e-4).unwrap();
        let e: Vec<f64> = y[first..=last]
            .chunks_exact(480)
            .map(|b| b.iter().map(|&v| f64::from(v) * f64::from(v)).sum::<f64>() / 480.0)
            .collect();
        let mut worst = 0.0f64;
        for k in 1..e.len().saturating_sub(2) {
            let around = (e[k - 1] + e[k + 1]) * 0.5;
            let db = 10.0 * (e[k].max(1e-20) / around.max(1e-20)).log10();
            worst = worst.min(db);
        }
        eprintln!("{speed}×: worst block {worst:.1} dB against its neighbours");
        assert!(
            worst > -40.0,
            "{speed}×: a block {worst:.1} dB below its neighbours"
        );
    }
}

#[test]
fn speed_changes_mid_play_without_clicks() {
    let amp = 0.5f32;
    let x = sine(6.0, 440.0, amp);
    let out = FakeOutput::new(RATE, 2);
    let p = player(RATE, 1, &x, &out);
    p.play(Duration::ZERO);
    let t0 = Instant::now();
    let speeds = [1.0, 1.5, 0.75, 1.0, 2.0, 0.5, 1.25];
    for (i, &s) in speeds.iter().enumerate() {
        p.set_speed(s);
        pull(&p, &out, t0, i * 40, 40, 480);
        // The new speed is heard from the next buffer.
        let at = t0 + Duration::from_millis(10 * (i * 40 + 39) as u64);
        let a = p.heard(at).unwrap().as_secs_f64();
        let b = p
            .heard(at + Duration::from_millis(10))
            .unwrap()
            .as_secs_f64();
        assert!(((b - a) / 0.010 - s).abs() < 1e-6, "clock at {s}×");
    }
    let y: Vec<f32> = out.played.lock().chunks_exact(2).map(|f| f[0]).collect();
    save("sine-440-speed-changes.wav", &y);
    // A sine's steepest step between samples, and some room for the cross-fades.
    let natural = amp * std::f32::consts::TAU * 440.0 / RATE as f32;
    let worst = y
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    eprintln!("steepest step {worst:.4}, the sine's {natural:.4}");
    assert!(
        worst < natural * 1.25,
        "a click: step {worst}, sine's {natural}"
    );
    let quiet = y[480..]
        .chunks_exact(480)
        .filter(|b| b.iter().all(|v| v.abs() < 0.05))
        .count();
    assert_eq!(quiet, 0, "a gap");
}

#[test]
fn clock_follows_what_is_heard_at_1_5x() {
    // Bursts of 1 kHz, 40 ms long, at every whole second of 90 s of silence: 60 s heard
    // at 1.5×.
    let speed = 1.5;
    let secs = 91usize;
    let mut x = vec![0.0f32; secs * RATE as usize];
    for s in 1..secs {
        for i in 0..1920 {
            x[s * RATE as usize + i] =
                0.5 * (std::f32::consts::TAU * 1000.0 * i as f32 / RATE as f32).sin();
        }
    }
    let out = FakeOutput::new(RATE, 2);
    let p = player(RATE, 1, &x, &out);
    p.set_speed(speed);
    p.play(Duration::ZERO);
    let t0 = Instant::now();
    let buffers = 6000;
    let mut clock = Vec::with_capacity(buffers);
    for k in 0..buffers {
        p.wait_queued(12_000);
        let at = t0 + Duration::from_millis(10 * k as u64);
        out.pull(480, at);
        clock.push(p.heard(at).unwrap().as_secs_f64());
    }
    let y: Vec<f32> = out.played.lock().chunks_exact(2).map(|f| f[0]).collect();
    // The clock runs at 1.5×: 15 ms of sound per 10 ms buffer.
    for w in clock.windows(2) {
        assert!(
            (w[1] - w[0] - 0.015).abs() < 0.004,
            "clock step {}",
            w[1] - w[0]
        );
    }
    let (mut onsets, mut worst, mut last) = (0, 0.0f64, None::<usize>);
    for (j, v) in y.iter().enumerate() {
        if v.abs() <= 0.1 {
            continue;
        }
        if last.is_none_or(|l| j > l + 9600) {
            let heard = clock[j / 480] + (j % 480) as f64 / f64::from(RATE) * speed;
            worst = worst.max((heard - heard.round()).abs());
            onsets += 1;
        }
        last = Some(j);
    }
    eprintln!(
        "{onsets} bursts, worst clock offset {:.1} ms",
        worst * 1000.0
    );
    assert!(onsets >= 85, "{onsets} bursts heard");
    assert!(
        worst < 0.020,
        "clock {:.1} ms off what is heard",
        worst * 1000.0
    );
}

/// 0.25× and 4× (not offered for movies): pitch and length still hold; WAVs to listen to.
#[test]
fn extreme_speeds_keep_pitch() {
    let x = sine(2.0, 440.0, 0.5);
    let s = speechy(3.0);
    for speed in [0.25, 4.0] {
        let y = play_all(&x, speed);
        save(&format!("sine-440-{speed}x.wav"), &y);
        let first = y.iter().position(|v| v.abs() > 0.01).unwrap();
        let last = y.iter().rposition(|v| v.abs() > 0.01).unwrap();
        let len = (last - first + 1) as f64 / (x.len() as f64 / speed);
        let mid = first + (last - first) / 2;
        let f = peak_hz(&y[mid - 4_800..mid + 4_800], 400.0, 480.0);
        eprintln!("{speed}×: length {len:.4} of 1/speed, peak {f:.2} Hz");
        assert!((len - 1.0).abs() < 0.02, "{speed}×: length {len}");
        assert!((f / 440.0 - 1.0).abs() < 0.01, "{speed}×: peak {f} Hz");
        save(&format!("speechy-{speed}x.wav"), &play_all(&s, speed));
    }
}
