//! Decode speed of specular-tiff against the `tiff` crate on synthetic files, plus the two
//! inflate engines on the same Deflate data.
//!
//! `CARGO_TARGET_DIR=target/main cargo run --release -p specular-tiff --example bench-tiff -- [size]`

#[path = "../tests/common/mod.rs"]
mod common;

use common::{Comp, Spec, pattern8, write};
use specular_tiff::{DecodeOptions, Region, Tiff};
use std::io::Cursor;
use std::time::Instant;

fn best<F: FnMut()>(runs: usize, mut f: F) -> f64 {
    let mut b = f64::MAX;
    for _ in 0..runs {
        let t = Instant::now();
        f();
        b = b.min(t.elapsed().as_secs_f64() * 1e3);
    }
    b
}

fn main() {
    let n: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(4096);
    let raw = pattern8(n, n, 3, 1);
    println!(
        "{n}×{n} RGB8, best of 5, ms (threads: {})",
        rayon::current_num_threads()
    );
    println!(
        "{:<28} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "file", "tiff", "ours 1t", "ours mt", "region", "1/4"
    );
    for (name, comp, tile, pred) in [
        ("none strips", Comp::None, None, 1),
        ("lzw strips pred2", Comp::Lzw, None, 2),
        ("lzw tiles 256", Comp::Lzw, Some((256, 256)), 1),
        ("deflate strips pred2", Comp::Deflate, None, 2),
        ("deflate tiles 256", Comp::Deflate, Some((256, 256)), 1),
        ("packbits strips", Comp::PackBits, None, 1),
    ] {
        let mut s = Spec::new(n, n, 3, 8, 2, raw.clone());
        s.comp = comp;
        s.tile = tile;
        s.predictor = pred;
        s.rows_per_strip = 16;
        let f = write(&[s], false, true);
        let t_up = best(5, || {
            let mut d = tiff::decoder::Decoder::new(Cursor::new(&f[..]))
                .unwrap()
                .with_limits(tiff::decoder::Limits::unlimited());
            std::hint::black_box(d.read_image().unwrap());
        });
        let t = Tiff::open(&f).unwrap();
        let run = |o: DecodeOptions| {
            best(5, || {
                std::hint::black_box(t.decode(&f, 0, &o).unwrap());
            })
        };
        let single = run(DecodeOptions {
            parallel: false,
            ..Default::default()
        });
        let multi = run(DecodeOptions::default());
        let region = run(DecodeOptions {
            region: Some(Region {
                x: n / 3,
                y: n / 3,
                w: 512,
                h: 512,
            }),
            ..Default::default()
        });
        let quarter = run(DecodeOptions {
            scale: 4,
            ..Default::default()
        });
        println!("{name:<28} {t_up:>9.1} {single:>9.1} {multi:>9.1} {region:>9.1} {quarter:>9.1}");
    }

    // Inflate engines on 16-row strips of the pattern (zlib streams).
    let rb = n as usize * 3;
    let strips: Vec<Vec<u8>> = raw
        .chunks(rb * 16)
        .map(|c| miniz_oxide::deflate::compress_to_vec_zlib(c, 6))
        .collect();
    let mut out = vec![0u8; rb * 16];
    let z = best(5, || {
        for s in &strips {
            let (o, rc) =
                zlib_rs::decompress_slice(&mut out, s, zlib_rs::InflateConfig { window_bits: 15 });
            assert!(matches!(rc, zlib_rs::ReturnCode::Ok));
            std::hint::black_box(o);
        }
    });
    let m = best(5, || {
        for s in &strips {
            std::hint::black_box(miniz_oxide::inflate::decompress_slice_iter_to_slice(
                &mut out,
                std::iter::once(&s[..]),
                true,
                true,
            ))
            .unwrap();
        }
    });
    println!(
        "inflate {} MB in 16-row strips, 1 thread: zlib-rs {z:.1} ms, miniz_oxide {m:.1} ms",
        raw.len() >> 20
    );
}
