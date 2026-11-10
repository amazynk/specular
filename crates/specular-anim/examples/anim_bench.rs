//! Frame ring throughput with a fake 4K 60 fps source.
//!
//! ```text
//! CARGO_TARGET_DIR=target/main cargo run --release -p specular-anim --example anim_bench
//! ```
//!
//! 1. Throughput: the clock advances one frame per tick as soon as the frame is decoded, so
//!    the rate is what decode-ahead plus hand-off sustain (the fake "decode" is allocating and
//!    filling a 33 MB canvas).
//! 2. Real time: 3 s of 60 fps playback on the wall clock, sleeping until each deadline;
//!    reports dropped frames and stalls.

use specular_anim::{DelayPolicy, FakeFrames, FakeSpec, Player, PlayerConfig};
use specular_decode::AnimationInfo;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

const W: u32 = 3840;
const H: u32 = 2160;
const FRAMES: u32 = 600; // 10 s at 60 fps; never fits the budget, so it streams

fn make(budget: usize) -> (Player, mpsc::Receiver<()>) {
    let spec = FakeSpec::new(FRAMES, Duration::from_nanos(16_666_667), W, H);
    let (frames, _) = FakeFrames::new(spec);
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let cfg = PlayerConfig {
        delay_policy: DelayPolicy::AS_STORED,
        budget_bytes: budget,
        max_ahead: 8,
        ..PlayerConfig::default()
    };
    let info = AnimationInfo {
        frame_count: None,
        loop_count: 0,
    };
    let waker = Arc::new(move || {
        let _ = tx.lock().map(|t| t.send(()));
    });
    (Player::new(Box::new(frames), &info, cfg, Some(waker)), rx)
}

fn main() {
    let fb = W as usize * H as usize * 4;
    let budget = 12 * fb;
    println!(
        "4K RGBA frame {:.1} MB, budget {:.0} MB",
        fb as f64 / 1e6,
        budget as f64 / 1e6
    );

    // 1. Throughput with a simulated clock.
    let (mut p, rx) = make(budget);
    let n = 300u64;
    let mut sim = Instant::now();
    let start = Instant::now();
    let mut got = 0u64;
    while got < n {
        let r = p.tick(sim);
        if r.show.is_some() {
            got += 1;
        }
        match r.next_deadline {
            Some(d) => sim = sim.max(d),
            None => {
                let _ = rx.recv_timeout(Duration::from_secs(5));
            }
        }
    }
    let secs = start.elapsed().as_secs_f64();
    let st = p.stats();
    println!(
        "throughput: {n} frames in {secs:.2} s = {:.0} fps, {:.2} GB/s; dropped {}, stalls {}, peak ring {:.0} MB",
        n as f64 / secs,
        n as f64 * fb as f64 / secs / 1e9,
        st.dropped,
        st.stalls,
        st.cached_bytes as f64 / 1e6
    );
    p.shutdown();

    // 2. Real time at 60 fps.
    let (mut p, rx) = make(budget);
    let start = Instant::now();
    let mut worst_late = Duration::ZERO;
    let mut shown = 0u64;
    while start.elapsed() < Duration::from_secs(3) {
        let now = Instant::now();
        let r = p.tick(now);
        if r.show.is_some() {
            shown += 1;
        }
        match r.next_deadline {
            Some(d) => {
                let wait = d.saturating_duration_since(Instant::now());
                std::thread::sleep(wait);
                worst_late = worst_late.max(Instant::now().saturating_duration_since(d));
            }
            None => {
                let _ = rx.recv_timeout(Duration::from_millis(100));
            }
        }
    }
    let st = p.stats();
    println!(
        "real time 3 s: shown {shown}, dropped {}, stalls {}, resyncs {}, worst wake-up lateness {:.2} ms",
        st.dropped,
        st.stalls,
        st.resyncs,
        worst_late.as_secs_f64() * 1e3
    );
    p.shutdown();
}
