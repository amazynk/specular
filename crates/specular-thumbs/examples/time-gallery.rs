//! Wall time of gallery thumbnails: each file's thumbnail made single-threaded and threaded,
//! then all of them through the engine as the gallery makes them (its worker pool, no disk
//! cache), where very large images may decode threaded (`engine::decode_in_parallel`).
//!
//! `cargo run --release -p specular-thumbs --example time-gallery -- [--workers N] [--class C]
//! [--engine-only] <file>...`

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::{CancelToken, JobPool, Source};
use specular_decode::Registry;
use specular_thumbs::engine::{FileOpener, Outcome};
use specular_thumbs::generate::{GenOptions, generate};
use specular_thumbs::{Engine, EngineConfig, Request};

fn main() {
    let mut workers = EngineConfig::default().workers;
    let mut class = 256;
    let mut files = Vec::new();
    let mut direct = true;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--workers" => workers = args.next().and_then(|n| n.parse().ok()).expect("workers"),
            "--engine-only" => direct = false,
            "--class" => class = args.next().and_then(|n| n.parse().ok()).expect("class"),
            _ => files.push(PathBuf::from(a)),
        }
    }
    let registry = Arc::new(Registry::new());
    for f in files.iter().filter(|_| direct) {
        let s = registry
            .open(Arc::new(Source::open(f).expect("open")))
            .expect("decode");
        let info = s.info();
        let mp = f64::from(info.width) * f64::from(info.height) / 1e6;
        for parallel in [false, true] {
            let opts = GenOptions {
                parallel,
                ..GenOptions::default()
            };
            let t = Instant::now();
            let px = generate(s.as_ref(), class, opts, &CancelToken::new(), &mut |_| {});
            let ok = px.map(|p| format!("{}x{}", p.width, p.height));
            println!(
                "{} ({mp:.0} MP) parallel={parallel}: {ok:?} in {:.0} ms",
                f.display(),
                t.elapsed().as_secs_f64() * 1e3
            );
        }
    }
    let cfg = EngineConfig {
        workers,
        write_cache: false,
        ..EngineConfig::default()
    };
    let pool = Arc::new(JobPool::background(workers, "thumb"));
    let engine = Engine::new(
        cfg,
        registry,
        pool,
        Arc::new(FileOpener),
        None,
        Arc::new(|| {}),
    );
    let reqs: Vec<Request> = files
        .iter()
        .map(|p| Request {
            path: p.clone(),
            class,
        })
        .collect();
    let t = Instant::now();
    let n = reqs.len();
    engine.want(reqs, n);
    let mut done = 0;
    while done < n {
        for o in engine.take_ready() {
            match o {
                Outcome::Ready(r) if r.sharp => {
                    done += 1;
                    println!(
                        "engine ({workers} workers): {} at {:.0} ms",
                        r.path.display(),
                        t.elapsed().as_secs_f64() * 1e3
                    );
                }
                Outcome::Ready(_) => {}
                Outcome::Failed { path, error, .. } => {
                    done += 1;
                    println!("engine: {} failed: {error}", path.display());
                }
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
