//! A run's CPU time counts every thread that works for it, alone and beside other runs
//! (paths/cpu.rs). Its own test binary: the ledger is process-wide, so no other test may
//! open runs meanwhile.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
#![cfg(target_os = "linux")]

use std::sync::Barrier;
use std::time::Instant;

use specular_core::paths::{Engine, GpuClass, Key, PathSelector, Task, Trust};

/// Spins until the calling thread has used `ms` of CPU time.
fn burn(ms: f64) {
    let t0 = specular_core::paths::thread_usage().unwrap().cpu_us;
    let mut x = 1u64;
    while specular_core::paths::thread_usage().unwrap().cpu_us - t0 < ms * 1e3 {
        for _ in 0..1000 {
            x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
        }
    }
}

fn cpu_ms(sel: &PathSelector, task: Task, path: usize, key: Key) -> f64 {
    // One sample per cell: the average is the run.
    let line = sel
        .to_table()
        .lines()
        .find(|l| {
            l.starts_with(&format!(
                "cell {} {} {} {} ",
                task.key(),
                task.engines()[path].key(),
                key.variant,
                key.bucket
            ))
        })
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("no cell for {task:?} {path} in\n{}", sel.to_table()));
    let w: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(w[8], "1", "{line}");
    w[9].parse::<f64>().unwrap() * f64::from(MPX) / 1e3
}

const MPX: f32 = 2.0;

#[test]
fn cpu_time_counts_all_threads() {
    let sel = PathSelector::new();
    sel.set_machine(GpuClass::Integrated, 16);
    sel.set_trust(Task::PngUnfilter, Engine::CpuParallel, Trust::Trusted);
    let px = (MPX * 1e6) as u64;

    // Alone: the calling thread waits while four helpers burn 40 ms each.
    let c = sel.calibration(Task::PngUnfilter, 1, px, 0);
    let t = Instant::now();
    let run = sel.start(&c);
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| burn(40.0));
        }
    });
    run.finish();
    let wall = t.elapsed().as_secs_f64() * 1e3;
    let cpu = cpu_ms(&sel, Task::PngUnfilter, 1, c.key);
    assert!(
        cpu >= 4.0 * 40.0 * 0.98,
        "4 × 40 ms of helpers counted as {cpu:.1} ms"
    );
    assert!(
        cpu <= 4.0 * 40.0 + wall * 2.0 + 20.0,
        "{cpu:.1} ms in {wall:.1} ms"
    );

    // Overlapping: a parallel decode (four helpers, 40 ms each) and a single-threaded colour
    // conversion (30 ms on its own thread) at the same time.
    let sel2 = PathSelector::new();
    sel2.set_machine(GpuClass::Integrated, 16);
    let par = sel2.calibration(Task::JpegDecode, 0, px, 0);
    let one = sel2.calibration(Task::YcbcrToRgb, 0, px, 0);
    let go = Barrier::new(2);
    let both = Barrier::new(2);
    std::thread::scope(|s| {
        s.spawn(|| {
            go.wait();
            let run = sel2.start(&par);
            both.wait();
            std::thread::scope(|h| {
                for _ in 0..4 {
                    h.spawn(|| burn(40.0));
                }
            });
            run.finish();
        });
        s.spawn(|| {
            go.wait();
            let run = sel2.start(&one);
            both.wait();
            burn(30.0);
            run.finish();
        });
    });
    let p = cpu_ms(&sel2, Task::JpegDecode, 0, par.key);
    let o = cpu_ms(&sel2, Task::YcbcrToRgb, 0, one.key);
    // The conversion gets its own 30 ms and a small share of the helpers; the decode the
    // rest. Nothing is lost: together they are at least what was burnt.
    assert!(o >= 30.0 * 0.98, "own thread: {o:.1} ms");
    assert!(
        o <= 30.0 + 0.1 * 160.0,
        "single-threaded run charged {o:.1} ms"
    );
    assert!(p >= 160.0 * 0.85, "parallel run charged {p:.1} ms");
    assert!(p + o >= 190.0 * 0.98, "{p:.1} + {o:.1} ms");
}
