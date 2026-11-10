use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

const RATE: u32 = 48_000;

/// Writes `frames` stereo frames of a ramp (each sample its index from `first`, L
/// positive, R negative), the first heard at media time `first / RATE`.
fn ramp(tap: &Tap, first: u64, frames: usize) {
    let buf: Vec<f32> = (0..frames)
        .flat_map(|i| {
            let v = (first + i as u64) as f32;
            [v, -v]
        })
        .collect();
    let ns = (first as f64 * 1e9 / f64::from(RATE)) as i64;
    tap.write(&buf, 2, RATE, ns, 1.0);
}

#[test]
fn reads_the_frames_heard_at_a_time() {
    let tap = Tap::new();
    let mut out = Vec::new();
    assert_eq!(tap.read_at(0.0, 16, &mut out), None);
    for k in 0..10 {
        ramp(&tap, k * 512, 512);
    }
    // 0.05 s = frame 2400: the 16 frames before it.
    let h = tap.read_at(0.05, 16, &mut out).unwrap();
    assert_eq!(
        h,
        Heard {
            channels: 2,
            rate: RATE,
            frames: 16
        }
    );
    assert_eq!(out.len(), 32);
    assert_eq!(out[0], 2384.0);
    assert_eq!(out[1], -2384.0);
    assert_eq!(out[31], -2399.0);
    // A little past what was written (the clock runs between buffers): the newest.
    let h = tap.read_at(5120.0 / 48_000.0 + 0.02, 4, &mut out).unwrap();
    assert_eq!(h.frames, 4);
    assert_eq!(out[6], 5119.0);
    // Far past it (a seek not heard yet): nothing.
    assert_eq!(tap.read_at(1.0, 4, &mut out), None);
    // Before this run started: nothing.
    assert_eq!(tap.read_at(0.001, 100, &mut out), None);
}

#[test]
fn idle_and_a_new_run() {
    let tap = Tap::new();
    let mut out = Vec::new();
    ramp(&tap, 0, 4800);
    assert!(tap.read_at(0.05, 100, &mut out).is_some());
    tap.idle();
    assert!(!tap.is_live());
    assert_eq!(tap.read_at(0.05, 100, &mut out), None);
    // After a seek to 10 s: the old run's frames are not read with the new one.
    ramp(&tap, 480_000, 256);
    assert_eq!(tap.read_at(10.0 + 256.0 / 48_000.0, 512, &mut out), None);
    let h = tap.read_at(10.0 + 256.0 / 48_000.0, 256, &mut out).unwrap();
    assert_eq!(h.frames, 256);
    assert_eq!(out[0], 480_000.0);
}

#[test]
fn overrun_drops_what_went_round() {
    let tap = Tap::new();
    let mut out = Vec::new();
    // Four times the ring.
    let per = 1024;
    let total = CAPACITY * 4 / 2;
    for k in (0..total).step_by(per) {
        ramp(&tap, k as u64, per);
    }
    let t_end = total as f64 / f64::from(RATE);
    // The newest half ring reads; more than the ring holds does not.
    let h = tap.read_at(t_end, CAPACITY / 4, &mut out).unwrap();
    assert_eq!(h.frames, CAPACITY / 4);
    assert_eq!(out[0], (total - CAPACITY / 4) as f32);
    assert_eq!(tap.read_at(t_end, CAPACITY / 2 + 1, &mut out), None);
    // A time long gone round: nothing.
    assert_eq!(tap.read_at(0.5, 64, &mut out), None);
    // A huge buffer keeps its end, at its own time.
    let big: Vec<f32> = (0..CAPACITY * 2).map(|i| i as f32).collect();
    tap.write(&big, 2, RATE, 100_000_000_000, 1.0);
    let t = 100.0 + CAPACITY as f64 / f64::from(RATE);
    let h = tap.read_at(t, 2, &mut out).unwrap();
    assert_eq!(h.frames, 2);
    assert_eq!(out[3], (CAPACITY * 2 - 1) as f32);
}

#[test]
fn speed_and_channels() {
    let tap = Tap::new();
    let mut out = Vec::new();
    // At 2× the media time runs twice as fast as the frames.
    let buf: Vec<f32> = (0..4800).map(|i| i as f32).collect();
    tap.write(&buf, 1, RATE, 0, 2.0);
    let h = tap.read_at(0.1, 10, &mut out).unwrap();
    assert_eq!(h.channels, 1);
    assert_eq!(out[9], 2399.0);
    // A new format starts a new run.
    let six: Vec<f32> = vec![0.5; 6 * 100];
    tap.write(&six, 6, 44_100, 0, 1.0);
    let h = tap.read_at(100.0 / 44_100.0, 100, &mut out).unwrap();
    assert_eq!((h.channels, h.rate, h.frames), (6, 44_100, 100));
    assert!(out.iter().all(|&v| v == 0.5));
}

/// A writer going round the ring as fast as it can while readers copy: every read is a run
/// of consecutive frames, never a mix of two passes.
#[test]
fn no_torn_reads() {
    let tap = Arc::new(Tap::new());
    let stop = Arc::new(AtomicBool::new(false));
    let w = {
        let tap = Arc::clone(&tap);
        let stop = Arc::clone(&stop);
        specular_thread::builder("tap-writer")
            .spawn(move || {
                let mut k = 0u64;
                let per = 4096;
                while !stop.load(Ordering::Relaxed) {
                    // Values wrap below 2^24 so f32 holds them exactly.
                    let buf: Vec<f32> = (0..per)
                        .flat_map(|i| {
                            let v = ((k + i) % (1 << 23)) as f32;
                            [v, v + 0.5]
                        })
                        .collect();
                    let ns = (k as f64 * 1e9 / f64::from(RATE)) as i64;
                    tap.write(&buf, 2, RATE, ns, 1.0);
                    k += per;
                }
            })
            .unwrap()
    };
    let mut out = Vec::new();
    let (mut good, mut none) = (0, 0);
    let started = std::time::Instant::now();
    while good < 2000 && started.elapsed().as_secs() < 20 {
        let frames = tap.written.load(Ordering::Acquire) / 2;
        let t = frames as f64 / f64::from(RATE);
        match tap.read_at(t, 20_000, &mut out) {
            Some(h) => {
                for f in 0..h.frames {
                    let (l, r) = (out[f * 2], out[f * 2 + 1]);
                    assert_eq!(r, l + 0.5, "frame {f}");
                    if f > 0 {
                        let prev = out[f * 2 - 2];
                        assert!(
                            l == prev + 1.0 || (prev == ((1 << 23) - 1) as f32 && l == 0.0),
                            "frame {f}: {prev} then {l}"
                        );
                    }
                }
                good += 1;
            }
            None => none += 1,
        }
    }
    stop.store(true, Ordering::Relaxed);
    w.join().unwrap();
    assert!(good > 0, "no read got through ({none} gave up)");
}
