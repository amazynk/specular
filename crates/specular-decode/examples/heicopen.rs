//! HEIC through the registry, as the viewer opens it: open-to-first-pixel and full decode.
//!
//! `cargo run --release -p specular-decode --example heicopen -- [--runs N] file.heic...`
//!
//! - `open`: `Registry::open_path` (parse; the old libheif-host path decoded here).
//! - `first pixel`: open + the embedded preview (`decode_preview`) when there is one,
//!   otherwise open + a 1/8 decode (the loader's overview level).
//! - `full`: open + a whole 1:1 decode (parallel), on a fresh session.
//!
//! Each run uses a fresh session, so nothing is cached between runs. Best of N, milliseconds.
//! `SPECULAR_HEIC_HW=0|1` (read by the decoder) forces software or hardware. `--calibrate`
//! first probes the hardware and runs the HEIC calibration, as the app does when idle, so
//! the path selector chooses as it would in the app.
use specular_core::CancelToken;
use specular_decode::{DecodeRequest, Registry};
use std::path::Path;
use std::time::Instant;

fn main() {
    let mut runs = 3usize;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--runs" => runs = args.next().and_then(|s| s.parse().ok()).expect("runs"),
            "--calibrate" => {
                specular_hwdec::set_enabled(true);
                specular_hwdec::probe();
                specular_decode::calibrate::heic(&|| false);
            }
            _ => files.push(a),
        }
    }
    let reg = Registry::new();
    let cancel = CancelToken::new();
    for path in &files {
        let (mut open, mut first, mut full) = (f64::MAX, f64::MAX, f64::MAX);
        let mut how = "";
        let mut dims = (0, 0);
        for _ in 0..runs {
            let t = Instant::now();
            let s = match reg.open_path(Path::new(path)) {
                Ok(s) => s,
                Err(e) => {
                    println!("{path}: open failed: {e}");
                    break;
                }
            };
            open = open.min(t.elapsed().as_secs_f64());
            dims = (s.info().width, s.info().height);
            match s.decode_preview() {
                Some(Ok(r)) => {
                    how = "preview";
                    std::hint::black_box(&r);
                }
                _ => {
                    how = "1/8";
                    let req = DecodeRequest {
                        parallel: true,
                        ..DecodeRequest::full(8)
                    };
                    match s.decode(&req, &cancel) {
                        Ok(r) => std::hint::black_box(&r),
                        Err(e) => {
                            println!("{path}: decode failed: {e}");
                            open = f64::MAX;
                            break;
                        }
                    };
                }
            }
            first = first.min(t.elapsed().as_secs_f64());
            drop(s);

            let t = Instant::now();
            let s = reg.open_path(Path::new(path)).expect("open");
            let req = DecodeRequest {
                parallel: true,
                ..DecodeRequest::full(1)
            };
            let Ok(r) = s.decode(&req, &cancel) else {
                break;
            };
            full = full.min(t.elapsed().as_secs_f64());
            std::hint::black_box(&r);
        }
        if open < f64::MAX {
            println!(
                "{path}: {}x{} open {:.1} ms | first pixel ({how}) {:.1} ms | full {:.1} ms",
                dims.0,
                dims.1,
                open * 1e3,
                first * 1e3,
                full * 1e3
            );
        }
    }
}
