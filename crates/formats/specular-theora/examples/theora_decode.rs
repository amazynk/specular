//! Decodes an Ogg Theora file: prints each frame's MD5 (as ffmpeg's framemd5) and the decode speed,
//! and writes raw planar YUV when given an output path.
//!
//! `cargo run -p specular-theora --example theora_decode -- IN.ogv [OUT.yuv]`

#[path = "../tests/md5/mod.rs"]
mod md5;
#[path = "../tests/ogg/mod.rs"]
mod ogg;

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(input) = args.get(1) else {
        eprintln!("usage: theora_decode IN.ogv [OUT.yuv]");
        std::process::exit(2);
    };
    let data = std::fs::read(input).expect("read input");
    let pk = ogg::packets(&data);
    assert!(pk.len() >= 3, "no Theora headers");
    let mut d = specular_theora::Decoder::new(&pk[0], &pk[2]).expect("headers");
    let info = d.info().clone();
    let mut out = args
        .get(2)
        .map(|p| std::io::BufWriter::new(std::fs::File::create(p).expect("create output")));
    let mut busy = std::time::Duration::ZERO;
    let mut n = 0;
    for (i, f) in pk[3..].iter().enumerate() {
        let t = std::time::Instant::now();
        let r = d.decode(f);
        busy += t.elapsed();
        match r {
            Ok(p) => {
                let mut m = md5::Md5::new();
                for plane in &p.planes {
                    m.update(plane);
                    if let Some(o) = out.as_mut() {
                        o.write_all(plane).expect("write");
                    }
                }
                println!("{i} {} {}", if p.keyframe { "K" } else { "-" }, m.hex());
                n += 1;
            }
            Err(e) => println!("{i} error: {e}"),
        }
    }
    let secs = busy.as_secs_f64();
    eprintln!(
        "{}x{} {:?}: {n} frames in {secs:.3} s = {:.1} fps",
        info.pic_width,
        info.pic_height,
        info.chroma,
        n as f64 / secs
    );
}
