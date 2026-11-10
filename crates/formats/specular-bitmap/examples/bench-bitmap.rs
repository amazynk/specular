//! Time specular-bitmap on generated files, against the `gif` and `qoi` crates where available
//! (`image` is banned by deny.toml, so BMP/TGA/PNM have no upstream comparison here).
//!
//! `cargo run --release -p specular-bitmap --example bench-bitmap` (BENCH_RUNS=n, default 5).

#[path = "../tests/common/mod.rs"]
mod common;
use common::*;
use specular_bitmap::{
    Decode, Options, bmp::Bmp, gif::Animator, gif::Gif, pnm::Pnm, qoi::Qoi, tga::Tga,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn runs() -> usize {
    std::env::var("BENCH_RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
}

fn best(mut f: impl FnMut()) -> Duration {
    (0..runs())
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .min()
        .unwrap()
}

fn ms(d: Duration) -> String {
    format!("{:8.1} ms", d.as_secs_f64() * 1e3)
}

fn row(name: &str, data: &[u8], d: &dyn Decode, oracle: Option<(&str, &dyn Fn())>) {
    let no = &|| false;
    let full = best(|| {
        std::hint::black_box(d.decode(data, &Options::default(), no).unwrap());
    });
    let serial = best(|| {
        std::hint::black_box(
            d.decode(
                data,
                &Options {
                    parallel: false,
                    ..Options::default()
                },
                no,
            )
            .unwrap(),
        );
    });
    let s8 = best(|| {
        std::hint::black_box(d.decode(data, &Options::scaled(8), no).unwrap());
    });
    let o = oracle
        .map(|(n, f)| format!("{n} {}", ms(best(f))))
        .unwrap_or_default();
    println!(
        "{name:<26} {:>6.1} MB | ours 1:1 {} (1 thread {}) 1/8 {} | {o}",
        data.len() as f64 / 1e6,
        ms(full),
        ms(serial),
        ms(s8)
    );
}

fn main() {
    let (w, h) = (4000u32, 3000u32);
    println!("generated {w}x{h} images; best of {} runs", runs());
    let f = bmp(w, h, BmpSpec::new(24));
    row("BMP 24-bit", &f, &Bmp::parse(&f).unwrap(), None);
    let f = bmp(w, h, BmpSpec::new(8));
    row("BMP 8-bit palette", &f, &Bmp::parse(&f).unwrap(), None);
    let f = bmp(
        w,
        h,
        BmpSpec {
            comp: 1,
            ..BmpSpec::new(8)
        },
    );
    row("BMP RLE8", &f, &Bmp::parse(&f).unwrap(), None);
    let f = tga(w, h, 2, 32, 0x20);
    row("TGA 32-bit", &f, &Tga::parse(&f).unwrap(), None);
    let f = tga(w, h, 10, 24, 0);
    row("TGA RLE 24-bit", &f, &Tga::parse(&f).unwrap(), None);
    let f = pnm(w, h, 6, 255, 3);
    row("PPM P6", &f, &Pnm::parse(&f).unwrap(), None);
    let f = pnm(w, h, 7, 65535, 4);
    row("PAM 16-bit RGBA", &f, &Pnm::parse(&f).unwrap(), None);

    let px: Vec<u8> = (0..w * h)
        .flat_map(|i| {
            let (x, y) = (i % w, i / w);
            if (x / 6) % 3 == 0 {
                pat(x / 6, y)
            } else {
                pat(x, y / 4)
            }
        })
        .collect();
    let f = qoi::encode_to_vec(&px, w, h).unwrap();
    drop(px);
    row(
        "QOI RGBA",
        &f,
        &Qoi::parse(&f).unwrap(),
        Some(("qoi crate", &|| {
            std::hint::black_box(qoi::decode_to_vec(&f).unwrap());
        })),
    );

    let f = gif_file(2000, 2000, &[gif_frame(0, 2000, 2000)], None);
    // Parse per run: a `Gif` caches its first frame's indices.
    let no = &|| false;
    let ours = |o: Options| {
        ms(best(|| {
            std::hint::black_box(Gif::parse(&f).unwrap().decode(&f, &o, no).unwrap());
        }))
    };
    let gif_crate = best(|| {
        let mut o = gif::DecodeOptions::new();
        o.set_color_output(gif::ColorOutput::RGBA);
        let mut d = o.read_info(&f[..]).unwrap();
        std::hint::black_box(d.read_next_frame().unwrap().unwrap().buffer.len());
    });
    println!(
        "{:<26} {:>6.1} MB | ours 1:1 {} (1 thread {}) 1/8 {} | gif crate {}",
        "GIF still 2000x2000",
        f.len() as f64 / 1e6,
        ours(Options::default()),
        ours(Options {
            parallel: false,
            ..Options::default()
        }),
        ours(Options::scaled(8)),
        ms(gif_crate)
    );

    // Animation: 1000x1000, 200 full frames.
    let n = 200;
    let frames: Vec<GifFrame> = (0..n).map(|i| gif_frame(i, 1000, 1000)).collect();
    let t = Instant::now();
    let f = gif_file(1000, 1000, &frames, Some(0));
    println!(
        "animated GIF 1000x1000x{n}: {:.1} MB (encoded in {:.1}s)",
        f.len() as f64 / 1e6,
        t.elapsed().as_secs_f64()
    );
    let no = &|| false;
    let t = Instant::now();
    let g = Arc::new(Gif::parse(&f).unwrap());
    println!("  frame-count scan            {}", ms(t.elapsed()));
    let t = Instant::now();
    std::hint::black_box(g.decode(&f, &Options::default(), no).unwrap());
    println!("  first frame (still decode)  {}", ms(t.elapsed()));
    let fps = |d: Duration| n as f64 / d.as_secs_f64();
    let ours = best(|| {
        let mut a = Animator::new(g.clone());
        while let Some(r) = a.next_frame(&f, no) {
            std::hint::black_box(r.unwrap().0.len());
        }
    });
    let ours8 = best(|| {
        let mut a = Animator::new(g.clone());
        while let Some(r) = a.next_frame(&f, no) {
            let c = r.unwrap().0;
            std::hint::black_box(
                specular_bitmap::gif::scale_rgba(c, 1000, 1000, &Options::scaled(4), no).unwrap(),
            );
        }
    });
    let gif_crate = best(|| {
        let mut o = gif::DecodeOptions::new();
        o.set_color_output(gif::ColorOutput::RGBA);
        let mut d = o.read_info(&f[..]).unwrap();
        while let Some(fr) = d.read_next_frame().unwrap() {
            std::hint::black_box(fr.buffer.len());
        }
    });
    println!("  ours composited 1:1         {:7.1} frames/s", fps(ours));
    println!("  ours composited + 1/4 scale {:7.1} frames/s", fps(ours8));
    println!(
        "  gif crate (not composited)  {:7.1} frames/s",
        fps(gif_crate)
    );
}

fn gif_frame(seed: u32, w: u16, h: u16) -> GifFrame {
    GifFrame {
        left: 0,
        top: 0,
        w,
        h,
        dispose: gif::DisposalMethod::Keep,
        transparent: if seed % 2 == 1 { Some(5) } else { None },
        interlaced: false,
        local_palette: false,
        delay: 4,
        seed,
    }
}
