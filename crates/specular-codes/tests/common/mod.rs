//! Shared by the oracle tests: PGM pictures and a clock that ignores other load.
#![allow(dead_code)]

use std::path::Path;

/// A binary (P5) PGM picture.
pub fn pgm(p: &Path) -> specular_codes::Gray {
    let d = std::fs::read(p).unwrap();
    let mut fields = Vec::new();
    let mut at = 0;
    while fields.len() < 4 {
        while d[at].is_ascii_whitespace() {
            at += 1;
        }
        if d[at] == b'#' {
            while d[at] != b'\n' {
                at += 1;
            }
            continue;
        }
        let s = at;
        while !d[at].is_ascii_whitespace() {
            at += 1;
        }
        fields.push(String::from_utf8_lossy(&d[s..at]).to_string());
    }
    at += 1;
    let w: usize = fields[1].parse().unwrap();
    let h: usize = fields[2].parse().unwrap();
    specular_codes::Gray {
        w,
        h,
        px: d[at..at + w * h].to_vec(),
    }
}

/// This thread's CPU time in ms where the system tells it (Linux), so timings hold on a
/// busy machine; otherwise `None`.
pub fn cpu_ms() -> Option<f64> {
    let s = std::fs::read_to_string("/proc/thread-self/schedstat").ok()?;
    let ns: f64 = s.split_whitespace().next()?.parse().ok()?;
    Some(ns / 1e6)
}

/// Times `f`: CPU ms where known, else wall-clock ms.
pub fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let (c0, t) = (cpu_ms(), std::time::Instant::now());
    let r = f();
    let e = match (c0, cpu_ms()) {
        (Some(a), Some(b)) => b - a,
        _ => t.elapsed().as_secs_f64() * 1000.0,
    };
    (r, e)
}
