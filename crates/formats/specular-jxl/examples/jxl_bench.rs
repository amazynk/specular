//! `cargo run --release -p specular-jxl --example jxl_bench -- FILE [RUNS] [seq]`
//!
//! Times upstream jxl-oxide (render + 8-bit stream) against specular-jxl at 1/1, 1/2, 1/4, 1/8
//! and `decode_dc`, plus `djxl` when it is on `PATH`. `seq` also times single-threaded decodes;
//! `profile` only runs sequential full-size decodes of specular-jxl (for `perf record`).
use std::time::Instant;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn time(runs: usize, mut f: impl FnMut()) -> f64 {
    median(
        (0..runs)
            .map(|_| {
                let t0 = Instant::now();
                f();
                t0.elapsed().as_secs_f64() * 1e3
            })
            .collect(),
    )
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: jxl_bench FILE [RUNS] [seq|profile]");
    let runs: usize = args.next().map_or(5, |s| s.parse().expect("RUNS"));
    let mode = args.next().unwrap_or_default();
    let seq = mode == "seq";
    let data = std::fs::read(&path).expect("read FILE");

    let f = specular_jxl::JxlFile::open(&data[..]).expect("open");
    let i = f.info();
    println!(
        "{path}: {}×{} xyb {} alpha {} gray {}",
        i.width, i.height, i.xyb, i.has_alpha, i.gray
    );

    if mode == "profile" {
        let t = time(runs, || {
            std::hint::black_box(f.decode(1, false).unwrap());
        });
        println!("specular-jxl 1/1 sequential: {t:8.1} ms");
        return;
    }

    let up = time(runs, || {
        let img = jxl_oxide::JxlImage::builder().read(&data[..]).unwrap();
        let r = img.render_frame(0).unwrap();
        let mut s = r.stream();
        let mut buf = vec![0u8; (s.width() * s.height() * s.channels()) as usize];
        s.write_to_buffer(&mut buf);
        std::hint::black_box(&buf);
    });
    println!("upstream jxl-oxide 0.12.6 (render + u8 stream, parallel): {up:8.1} ms");

    let modes: &[bool] = if seq { &[true, false] } else { &[true] };
    for &par in modes {
        let tag = if par { "parallel" } else { "sequential" };
        for scale in [1, 2, 4, 8] {
            let t = time(runs, || {
                std::hint::black_box(f.decode(scale, par).unwrap());
            });
            println!("specular-jxl 1/{scale} {tag}: {t:8.1} ms");
        }
        let t = time(runs, || {
            std::hint::black_box(f.decode_dc(par).unwrap());
        });
        println!("specular-jxl decode_dc {tag}: {t:8.1} ms");
    }

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/laneb-jxl/djxl-out.ppm");
    let djxl = |_: ()| {
        std::process::Command::new("djxl")
            .arg(&path)
            .arg(&out)
            .arg("--quiet")
            .output()
            .is_ok_and(|o| o.status.success())
    };
    if djxl(()) {
        let t = time(runs, || {
            djxl(());
        });
        println!("djxl (libjxl, incl. process start and PPM write): {t:8.1} ms");
        let _ = std::fs::remove_file(&out);
    }
}
