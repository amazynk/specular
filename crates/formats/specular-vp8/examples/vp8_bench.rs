//! Decode speed: `cargo run -p specular-vp8 --example vp8_bench [file.ivf] [threads]`.
//! Defaults to the 1080p clip from `tools/corpus/specular-vp8-fixtures.sh`, timed with 1
//! thread and with all threads.

use std::time::Instant;

fn frames(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut at = 32;
    while at + 12 <= data.len() {
        let n = u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as usize;
        at += 12;
        if at + n > data.len() {
            break;
        }
        out.push(&data[at..at + n]);
        at += n;
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).cloned().unwrap_or_else(|| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../tools/corpus/video2/specular-vp8/bench-1080p.ivf"
        )
        .to_string()
    });
    let data = std::fs::read(&path).expect("run tools/corpus/specular-vp8-fixtures.sh");
    let fr = frames(&data);
    let threads: Vec<usize> = match args.get(2) {
        Some(t) => vec![t.parse().expect("threads")],
        None => vec![1, 0],
    };
    for t in threads {
        let mut best = f64::MAX;
        let mut shown = 0;
        for _ in 0..3 {
            let mut d = specular_vp8::Decoder::new(t);
            shown = 0;
            let start = Instant::now();
            for f in &fr {
                if d.decode(f).expect("decode").is_some() {
                    shown += 1;
                }
            }
            best = best.min(start.elapsed().as_secs_f64());
        }
        println!(
            "{path}: threads {t}: {} frames ({shown} shown) in {:.3} s = {:.1} fps",
            fr.len(),
            best,
            fr.len() as f64 / best
        );
    }
}
