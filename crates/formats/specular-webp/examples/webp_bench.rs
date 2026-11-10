//! Decode timing: specular-webp at each scale vs upstream image-webp (RGB(A) output).
//! `cargo run --release -p specular-webp --example webp_bench -- FILE [RUNS]`
use std::io::Cursor;
use std::time::Instant;

fn median(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0])
}

fn time(runs: usize, mut f: impl FnMut()) -> (f64, f64) {
    median(
        (0..runs)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_secs_f64() * 1e3
            })
            .collect(),
    )
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("file");
    let runs: usize = args.next().map(|s| s.parse().unwrap()).unwrap_or(5);
    let data = std::fs::read(&path).unwrap();
    let only_ours = std::env::var_os("OURS_ONLY").is_some();
    if !only_ours {
        let (m, b) = time(runs, || {
            let mut d = image_webp::WebPDecoder::new(Cursor::new(&data)).unwrap();
            let mut buf = vec![0u8; d.output_buffer_size().unwrap()];
            d.read_image(&mut buf).unwrap();
            std::hint::black_box(&buf);
        });
        println!("upstream image-webp           median {m:7.1} ms  min {b:7.1}");
    }
    let img = specular_webp::WebpImage::parse(&data[..]).unwrap();
    println!("{:?}", img.info().coding);
    for parallel in [false, true] {
        if only_ours && std::env::var_os("SEQ").is_some() && parallel {
            continue;
        }
        for scale in [1, 2, 4, 8] {
            if only_ours && scale > 1 {
                continue;
            }
            let (m, b) = time(runs, || {
                std::hint::black_box(img.decode(scale, parallel).unwrap());
            });
            println!(
                "specular-webp 1/{scale} parallel={parallel:5}  median {m:7.1} ms  min {b:7.1}"
            );
        }
    }
}
