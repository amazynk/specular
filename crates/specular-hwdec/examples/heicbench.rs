//! Hardware vs software HEIC decode, one engine and file per process (for peak RSS):
//! `cargo run --release -p specular-hwdec --example heicbench -- ENGINE FILE [RUNS]`
//!
//! ENGINE: `vaapi` (VA-API, the default core for what it rejects), `heic-rs`, `oxideav` or
//! `default` (heic-rs, Oxideav for what heic-rs does not support). Decodes the whole image
//! (grid assembly included, tiles in parallel) and prints one line: best and median wall
//! time, CPU time (user + system, per decode), peak RSS, and the pictures that went to the
//! hardware. Linux reads CPU and RSS from /proc; Windows reads CPU time through sysinfo (peak
//! RSS prints as 0); elsewhere both print as 0.

use specular_heic::{DEFAULT_CORE, DecodeOptions, HeicFile, HeicRs, HevcCore, Oxideav};
use specular_hwdec::heic::{Policy, VaapiCore};
use std::time::Instant;

/// User + system CPU seconds of this process.
#[cfg(windows)]
fn cpu_s() -> f64 {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let pid = Pid::from_u32(std::process::id());
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_cpu(),
    );
    sys.process(pid)
        .map_or(0.0, |p| p.accumulated_cpu_time() as f64 / 1e3)
}

/// User + system CPU seconds of this process.
#[cfg(not(windows))]
fn cpu_s() -> f64 {
    let Ok(s) = std::fs::read_to_string("/proc/self/stat") else {
        return 0.0;
    };
    // Fields after the parenthesised command: utime and stime are the 12th and 13th.
    let rest = s.rsplit_once(')').map_or("", |(_, r)| r);
    let f: Vec<u64> = rest
        .split_whitespace()
        .skip(11)
        .take(2)
        .filter_map(|x| x.parse().ok())
        .collect();
    f.iter().sum::<u64>() as f64 / 100.0
}

/// Peak resident set size in MiB.
fn peak_rss_mib() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
        })
        .map_or(0.0, |kib| kib / 1024.0)
}

fn main() {
    let mut a = std::env::args().skip(1);
    let (Some(engine), Some(path)) = (a.next(), a.next()) else {
        eprintln!("usage: heicbench ENGINE FILE [RUNS]");
        std::process::exit(2);
    };
    let runs: usize = a.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let vaapi = VaapiCore::new().with_policy(Policy::Always);
    let core: &dyn HevcCore = match engine.as_str() {
        "vaapi" => {
            if !specular_hwdec::heic::usable() {
                eprintln!("no hardware HEVC decoder");
                std::process::exit(1);
            }
            &vaapi
        }
        "heic-rs" => &HeicRs,
        "oxideav" => &Oxideav,
        "default" => &DEFAULT_CORE,
        e => {
            eprintln!("unknown engine {e}");
            std::process::exit(2);
        }
    };
    let data = std::fs::read(&path).expect("read");
    let file = HeicFile::open(data).expect("open");
    let opts = DecodeOptions {
        parallel: true,
        cancel: None,
        core: Some(core),
    };
    // Warm-up: opens the device, fills caches and the rayon pool.
    let img = file.decode(&opts).expect("decode");
    let (w, h) = (img.width(), img.height());
    drop(img);
    let (hw0, _) = vaapi.stats();
    let mut wall = Vec::with_capacity(runs);
    let c0 = cpu_s();
    for _ in 0..runs {
        let t = Instant::now();
        let img = file.decode(&opts).expect("decode");
        wall.push(t.elapsed().as_secs_f64() * 1e3);
        drop(img);
    }
    let cpu = (cpu_s() - c0) * 1e3 / runs as f64;
    wall.sort_by(f64::total_cmp);
    let (hw, sw) = vaapi.stats();
    let name = std::path::Path::new(&path)
        .file_name()
        .map_or(path.clone(), |n| n.to_string_lossy().into_owned());
    println!(
        "{name:<28} {w:>5}x{h:<5} {engine:<8} best {:>7.1} ms  median {:>7.1} ms  cpu {:>7.1} ms  \
         peak {:>6.0} MiB  hw/sw pictures per decode {}/{}",
        wall[0],
        wall[runs / 2],
        cpu,
        peak_rss_mib(),
        (hw - hw0) / runs.max(1),
        sw / (runs + 1),
    );
}
