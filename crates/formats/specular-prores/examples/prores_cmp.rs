//! `prores_cmp FRAMES [REF.yuv]`: decodes the ProRes frames of `FRAMES` (the samples one after
//! another, as `ffmpeg -c:v copy -f data` writes them) and compares each with the next frame of
//! `REF.yuv` (FFmpeg's planar output: Y, Cb, Cr, then alpha when the frames have it). Prints
//! the mismatching samples per plane and the decode time.

use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&args[1]).expect("frames");
    let reference = args.get(2).map(|p| std::fs::read(p).expect("reference"));
    let mut frames = Vec::new();
    let mut at = 0;
    while at + 8 <= data.len() {
        let size = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        if size < 8 || at + size > data.len() {
            break;
        }
        frames.push(&data[at..at + size]);
        at += size;
    }
    let o = specular_prores::Options {
        parallel: std::env::var_os("SERIAL").is_none(),
        ..Default::default()
    };
    let mut ref_at = 0;
    let mut dump = Vec::new();
    let mut mismatches = [0usize; 4];
    let mut max_diff = 0i32;
    let mut best = f64::MAX;
    let mut total = 0.0;
    for (i, f) in frames.iter().enumerate() {
        let t = Instant::now();
        let d = specular_prores::decode(f, &o).expect("decode");
        let dt = t.elapsed().as_secs_f64();
        best = best.min(dt);
        total += dt;
        if i == 0 {
            eprintln!(
                "{}x{} {:?} {:?} alpha {:?} {}-bit encoder {}",
                d.width,
                d.height,
                d.chroma,
                d.header.interlace,
                d.header.alpha,
                d.bit_depth,
                String::from_utf8_lossy(&d.header.encoder)
            );
        }
        if d.damaged_slices > 0 {
            eprintln!("frame {i}: {} damaged slices", d.damaged_slices);
        }
        if args.len() > 3 {
            for p in d.planes.iter().chain(d.alpha.iter()) {
                dump.extend_from_slice(p);
            }
        }
        let Some(r) = &reference else { continue };
        let planes = d.planes.iter().chain(d.alpha.iter());
        for (p, plane) in planes.enumerate() {
            let want = &r[ref_at..ref_at + plane.len()];
            ref_at += plane.len();
            let mut first = None;
            for (k, (a, b)) in plane.chunks_exact(2).zip(want.chunks_exact(2)).enumerate() {
                let a = i32::from(u16::from_le_bytes([a[0], a[1]]));
                let b = i32::from(u16::from_le_bytes([b[0], b[1]]));
                if a != b {
                    mismatches[p] += 1;
                    max_diff = max_diff.max((a - b).abs());
                    first.get_or_insert((k, a, b));
                }
            }
            if let Some((k, a, b)) = first {
                let w = if p == 0 || p == 3 || d.chroma == specular_prores::Chroma::Yuv444 {
                    d.width as usize
                } else {
                    d.width.div_ceil(2) as usize
                };
                eprintln!(
                    "frame {i} plane {p}: first mismatch at ({}, {}): {a} vs {b}",
                    k % w,
                    k / w
                );
            }
        }
    }
    if let Some(p) = args.get(3) {
        std::fs::write(p, &dump).expect("dump");
    }
    println!(
        "{} frames, mismatches Y {} Cb {} Cr {} A {} (max diff {max_diff}); best {:.2} ms, mean {:.2} ms",
        frames.len(),
        mismatches[0],
        mismatches[1],
        mismatches[2],
        mismatches[3],
        best * 1e3,
        total / frames.len().max(1) as f64 * 1e3
    );
}
