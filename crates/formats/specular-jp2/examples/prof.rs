//! Repeated decodes for profiling: `prof file [reps] [reduce] [threads]` (threads 1 = single).

use specular_jp2::{DecodeOptions, Decoder, Parallelism};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&args[1]).expect("read");
    let reps: usize = args.get(2).map_or(5, |s| s.parse().expect("reps"));
    let reduce: u8 = args.get(3).map_or(0, |s| s.parse().expect("reduce"));
    let threads: usize = args.get(4).map_or(1, |s| s.parse().expect("threads"));
    let dec = Decoder::new(&data).expect("header");
    let mut best = f64::MAX;
    for _ in 0..reps {
        let t = std::time::Instant::now();
        let img = dec
            .decode(&DecodeOptions {
                reduce,
                parallelism: if threads == 1 {
                    Parallelism::Single
                } else {
                    Parallelism::Threads(threads)
                },
                ..Default::default()
            })
            .expect("decode");
        std::hint::black_box(img);
        best = best.min(t.elapsed().as_secs_f64() * 1e3);
    }
    println!("best {best:.1} ms");
}
