//! Decodes a bare MPEG-4 Part 2 stream to raw planar YUV (display order) and reports the
//! speed: `cargo run --example m4v_decode -- IN.m4v [OUT.yuv]`.

#![allow(clippy::disallowed_methods)]

use std::io::Write;

use specular_mpeg4::{Decoder, OutputOrder};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&args[1]).expect("input");
    let mut out = args
        .get(2)
        .filter(|p| p.as_str() != "-")
        .map(|p| std::io::BufWriter::new(std::fs::File::create(p).expect("output")));
    let mut d = Decoder::new(OutputOrder::Display);
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
    if specular_mpeg4::is_short_header(&data) {
        d.set_aligned(true);
        let mut starts: Vec<usize> = (0..data.len().saturating_sub(3))
            .filter(|&i| data[i] == 0 && data[i + 1] == 0 && data[i + 2] & 0xFC == 0x80)
            .collect();
        starts.push(data.len());
        for w in starts.windows(2) {
            d.decode(&data[w[0]..w[1]], 0).expect("decode");
            emit(&mut d, &mut n);
        }
    } else {
        for c in data.chunks(1 << 16) {
            d.decode(c, 0).expect("decode");
            emit(&mut d, &mut n);
        }
    }
    d.flush();
    emit(&mut d, &mut n);
    let s = t.elapsed().as_secs_f64();
    eprintln!("{n} frames in {s:.2} s: {:.1} fps", n as f64 / s);
}
