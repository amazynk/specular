//! Decodes an MPEG-1/2 elementary stream to raw planar YUV (display order) and reports the
//! speed: `cargo run --example m2v_decode -- IN.m2v [OUT.yuv] [threads]`.

#![allow(clippy::disallowed_methods)]

use std::io::Write;

use specular_mpeg2::{Decoder, OutputOrder};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&args[1]).expect("input");
    let mut out = args
        .get(2)
        .filter(|p| p.as_str() != "-")
        .map(|p| std::io::BufWriter::new(std::fs::File::create(p).expect("output")));
    let threads = args.get(3).and_then(|t| t.parse().ok()).unwrap_or(0);
    let mut d = Decoder::new(threads, OutputOrder::Display);
    let t = std::time::Instant::now();
    let mut n = 0;
    let mut emit = |d: &mut Decoder, n: &mut usize| {
        while let Some(p) = d.next_picture() {
            *n += 1;
            if let Some(o) = out.as_mut() {
                for pl in &p.planes {
                    o.write_all(pl).expect("write");
                }
            }
        }
    };
    for c in data.chunks(1 << 16) {
        d.decode(c, 0).expect("decode");
        emit(&mut d, &mut n);
    }
    d.flush();
    emit(&mut d, &mut n);
    let s = t.elapsed().as_secs_f64();
    eprintln!("{n} frames in {s:.2} s: {:.1} fps", n as f64 / s);
}
