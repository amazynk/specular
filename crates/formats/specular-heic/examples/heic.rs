//! Developer tool for specular-heic.
//!
//! * `heic info FILE...` — container info, layout, colour.
//! * `heic decode FILE [--st] [--repeat N] [--ppm OUT]` — decode, print
//!   time and peak RSS (`--st`: one thread; default: rayon pool).
//! * `heic robust FILE [CASES] [SEED]` — the mutation scheme of the
//!   2026-10 HEIC evaluation (24 truncations, then 1–8 random byte
//!   edits per case, xorshift64 from SEED, default 250 / 7): counts
//!   decoded / rejected / panicked cases and the slowest case. Every
//!   case is decoded on a worker thread with a 30 s watchdog.
//! * `heic case FILE SEED K OUT` — write mutation case K.
//!
//! `--heic-rs` anywhere selects the heic-rs core (default: oxideav).
//! With it, `robust` also decodes every case threaded and counts cases
//! whose output differs from the serial decode (`mismatch=`).
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use specular_heic::{DecodeOptions, HeicFile, HeicRs, HevcCore};

static HEIC_RS: AtomicBool = AtomicBool::new(false);

fn core() -> Option<&'static dyn HevcCore> {
    HEIC_RS
        .load(Ordering::Relaxed)
        .then_some(&HeicRs as &dyn HevcCore)
}

fn peak_rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
        })
        .map_or(0.0, |kb| kb / 1024.0)
}

fn mutation(base: &[u8], cases: u64, seed: u64) -> impl Iterator<Item = Vec<u8>> + '_ {
    let mut seed = seed;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    (0..cases).map(move |k| {
        if k < 24 {
            base[..(base.len() as u64 * (k + 1) / 25) as usize].to_vec()
        } else {
            let mut v = base.to_vec();
            let n = 1 + rnd() % 8;
            for _ in 0..n {
                let pos = if rnd() % 2 == 0 {
                    (rnd() % 4096.min(v.len() as u64)) as usize
                } else {
                    (rnd() % v.len() as u64) as usize
                };
                v[pos] = match rnd() % 4 {
                    0 => 0,
                    1 => 0xff,
                    2 => v[pos] ^ (1 << (rnd() % 8)),
                    _ => rnd() as u8,
                };
            }
            v
        }
    })
}

fn decode(data: Vec<u8>, parallel: bool) -> Result<specular_heic::Image, String> {
    let f = HeicFile::open(data).map_err(|e| e.to_string())?;
    f.decode(&DecodeOptions {
        parallel,
        core: core(),
        ..Default::default()
    })
    .map_err(|e| e.to_string())
}

fn main() {
    let mut a: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = a.iter().position(|x| x == "--heic-rs") {
        a.remove(i);
        HEIC_RS.store(true, Ordering::Relaxed);
    }
    match a.first().map(String::as_str) {
        Some("info") => {
            for p in &a[1..] {
                let data = std::fs::read(p).expect("read");
                match HeicFile::open(data) {
                    Ok(f) => println!("{p}: {:?}", f.info()),
                    Err(e) => println!("{p}: ERR {e}"),
                }
            }
        }
        Some("decode") => {
            let data = std::fs::read(&a[1]).expect("read");
            let st = a.iter().any(|x| x == "--st");
            let repeat: usize = a
                .iter()
                .position(|x| x == "--repeat")
                .map_or(1, |i| a[i + 1].parse().unwrap());
            let mut best = f64::MAX;
            let mut img = None;
            for _ in 0..repeat {
                let t = Instant::now();
                let r = decode(data.clone(), !st);
                best = best.min(t.elapsed().as_secs_f64() * 1e3);
                img = Some(r);
            }
            match img.unwrap() {
                Ok(img) => {
                    println!(
                        "OK {}x{} {:?} ms={best:.1} rss_mb={:.0} colour={:?} ycbcr={:?} warnings={:?}",
                        img.width(),
                        img.height(),
                        img.frame.format,
                        peak_rss_mb(),
                        img.colour.cicp(),
                        img.colour.ycbcr(),
                        img.warnings
                    );
                    if let Some(i) = a.iter().position(|x| x == "--ppm") {
                        let rgba = img.to_rgba8().expect("rgb");
                        let mut out =
                            format!("P6\n{} {}\n255\n", img.width(), img.height()).into_bytes();
                        out.extend(rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
                        std::fs::write(&a[i + 1], out).expect("write");
                    }
                }
                Err(e) => println!("ERR {e} ms={best:.1}"),
            }
        }
        Some("robust") => {
            let data = std::fs::read(&a[1]).expect("read");
            let cases = a.get(2).map_or(250, |c| c.parse().unwrap());
            let seed = a.get(3).map_or(7, |c| c.parse().unwrap());
            let verbose = std::env::var_os("HEIC_ROBUST_VERBOSE").is_some();
            std::panic::set_hook(Box::new(|_| {}));
            let both = HEIC_RS.load(Ordering::Relaxed);
            let (mut ok, mut err, mut panics, mut slow) = (0, 0, 0, (0.0f64, 0));
            let mut mismatch = 0;
            for (k, v) in mutation(&data, cases, seed).enumerate() {
                let t = Instant::now();
                let done = std::sync::Arc::new(AtomicBool::new(false));
                let d2 = done.clone();
                let h = std::thread::spawn(move || {
                    let r = std::panic::catch_unwind(|| {
                        let s = decode(v.clone(), false);
                        // Same planes and outcome when threaded?
                        let same = !both
                            || match (&s, &decode(v, true)) {
                                (Ok(a), Ok(b)) => a.frame == b.frame && a.warnings == b.warnings,
                                (Err(_), Err(_)) => true,
                                _ => false,
                            };
                        (s, same)
                    });
                    d2.store(true, Ordering::Release);
                    r
                });
                while !done.load(Ordering::Acquire) {
                    if t.elapsed() > Duration::from_secs(30) {
                        println!("HANG at case {k}");
                        std::process::exit(1);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                let r = h.join().unwrap().map(|(r, same)| {
                    if !same {
                        mismatch += 1;
                        println!("MISMATCH case {k}");
                    }
                    r
                });
                if verbose {
                    match &r {
                        Ok(Ok(i)) => println!("{k} OK {:?}", i.warnings),
                        Ok(Err(e)) => println!("{k} ERR {e}"),
                        Err(_) => {}
                    }
                }
                match r {
                    Ok(Ok(_)) => ok += 1,
                    Ok(Err(_)) => err += 1,
                    Err(p) => {
                        panics += 1;
                        let msg = p
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_default();
                        println!("PANIC case {k}: {msg}");
                    }
                }
                let s = t.elapsed().as_secs_f64();
                if s > slow.0 {
                    slow = (s, k);
                }
            }
            println!(
                "FUZZ ok={ok} err={err} panic={panics} mismatch={mismatch} slowest_s={:.2} (case {}) rss_mb={:.0}",
                slow.0,
                slow.1,
                peak_rss_mb()
            );
        }
        Some("case") => {
            let data = std::fs::read(&a[1]).expect("read");
            let seed = a[2].parse().unwrap();
            let k: usize = a[3].parse().unwrap();
            let v = mutation(&data, k as u64 + 1, seed).nth(k).unwrap();
            std::fs::write(&a[4], v).expect("write");
        }
        _ => eprintln!("usage: heic info|decode|robust|case …"),
    }
}
