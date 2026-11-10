//! `ffv1_cmp STREAM.frames [REF.yuv]`: decodes a stream cut by `tests/data/avi_frames.py`
//! and compares each frame with the next frame of `REF.yuv` (FFmpeg's planar output). Prints
//! the mismatching bytes and the best and mean decode times (set SERIAL for one thread).

use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let d = std::fs::read(&args[1]).expect("stream");
    let reference = args.get(2).map(|p| std::fs::read(p).expect("reference"));
    let be = |at: usize| u32::from_be_bytes(d[at..at + 4].try_into().unwrap()) as usize;
    let (w, h, n) = (be(0) as u32, be(4) as u32, be(8));
    let mut dec = specular_ffv1::Decoder::new(w, h, &d[12..12 + n]).expect("configuration");
    dec.parallel = std::env::var_os("SERIAL").is_none();
    let mut at = 12 + n;
    let (mut ref_at, mut mismatches, mut frames) = (0, 0usize, 0);
    let (mut best, mut total) = (f64::MAX, 0.0);
    while at + 4 <= d.len() {
        let size = be(at);
        let f = &d[at + 4..at + 4 + size];
        at += 4 + size;
        let t = Instant::now();
        let p = dec.decode(f).expect("frame");
        let s = t.elapsed().as_secs_f64();
        best = best.min(s);
        total += s;
        frames += 1;
        if let Some(r) = &reference {
            for pl in p.planes.iter().chain(p.alpha.iter()) {
                let want = &r[ref_at..ref_at + pl.len()];
                mismatches += pl.iter().zip(want).filter(|(a, b)| a != b).count();
                ref_at += pl.len();
            }
        }
    }
    if let Some(p) = dec.params() {
        println!("{}×{} {}", w, h, p.describe());
    }
    println!(
        "{frames} frames, best {:.2} ms, mean {:.2} ms, {mismatches} mismatching bytes",
        best * 1e3,
        total * 1e3 / f64::from(frames.max(1))
    );
}
