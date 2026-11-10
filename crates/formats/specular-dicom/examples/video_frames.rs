//! Decodes a DICOM video (H.264 / HEVC pixel data) for the oracle and for timings.
//!
//! `video_frames FILE [OUT_DIR [FRAME...]]` prints one JSON line: the frame count and size,
//! the open time, the first frame's time, the steady cine time per frame (front to back with
//! [`specular_dicom::video::Sequence`], over up to 120 frames after the first) and a random
//! access time. With OUT_DIR, frames FRAME... (default 0, 1, the middle and the last) are
//! written there as `NNNNN.rgb` (interleaved RGB, 8 bits; 16-bit samples shifted down).

use std::time::Instant;

use specular_dicom::{Dicom, Frame, Pixels};

fn rgb8(f: &Frame) -> Vec<u8> {
    match &f.pixels {
        Pixels::Rgb8(p) => p.clone(),
        Pixels::Rgb16(p) => p
            .iter()
            .map(|&v| (u32::from(v) >> f.bits.saturating_sub(8)).min(255) as u8)
            .collect(),
        Pixels::Grey(_) => Vec::new(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: video_frames FILE [OUT_DIR [FRAME...]]");
        std::process::exit(2);
    };
    let data = std::fs::read(path).expect("read");
    let t0 = Instant::now();
    let d = match Dicom::parse(&data).and_then(|d| d.check_supported().map(|()| d)) {
        Ok(d) => d,
        Err(e) => {
            println!("{{\"error\": {:?}}}", e.to_string());
            return;
        }
    };
    let open_ms = t0.elapsed().as_secs_f64() * 1e3;
    let n = d.frame_count();
    let t1 = Instant::now();
    let first = d.decode_frame(&data, 0, None).expect("frame 0");
    let first_ms = t1.elapsed().as_secs_f64() * 1e3;

    // Steady cine: the sequence decoder front to back, timing frames 1..=120.
    let mut seq = d.video_sequence().expect("video");
    let buf = d.buf(&data);
    let _ = seq.next_frame(buf, None);
    let t2 = Instant::now();
    let mut steady = 0u32;
    while steady < 120 {
        match seq.next_frame(buf, None) {
            Some(r) => {
                r.expect("cine frame");
                steady += 1;
            }
            None => break,
        }
    }
    let cine_ms = t2.elapsed().as_secs_f64() * 1e3 / f64::from(steady.max(1));

    // Random access: a frame in the middle, then one just before it (cache / restart).
    let mid = n / 2;
    let t3 = Instant::now();
    d.decode_frame(&data, mid, None).expect("middle frame");
    let seek_ms = t3.elapsed().as_secs_f64() * 1e3;
    let t4 = Instant::now();
    d.decode_frame(&data, mid.saturating_sub(1), None)
        .expect("step back");
    let back_ms = t4.elapsed().as_secs_f64() * 1e3;

    if let Some(out) = args.get(2) {
        std::fs::create_dir_all(out).expect("out dir");
        let mut frames: Vec<u32> = args[3..].iter().filter_map(|a| a.parse().ok()).collect();
        if frames.is_empty() {
            frames = vec![0, 1.min(n - 1), mid, n - 1];
        }
        for k in frames {
            let f = d.decode_frame(&data, k, None).expect("frame");
            std::fs::write(format!("{out}/{k:05}.rgb"), rgb8(&f)).expect("write");
        }
    }
    println!(
        "{{\"frames\": {n}, \"width\": {}, \"height\": {}, \"bits\": {}, \"open_ms\": {open_ms:.2}, \
         \"first_ms\": {first_ms:.2}, \"cine_ms\": {cine_ms:.2}, \"cine_frames\": {steady}, \
         \"seek_ms\": {seek_ms:.2}, \"back_ms\": {back_ms:.2}, \"restarts\": {}}}",
        first.width,
        first.height,
        first.bits,
        d.video().map_or(0, |v| v.restart_points())
    );
}
