//! Texture decode throughput: BCn vs upstream bcdec_rs on random blocks, then real 2048² ETC2,
//! ASTC, PVRTC, UASTC and ETC1S files (fetch-textures.sh bench_textures).
//! `cargo run --release -p specular-texture --example bench-texture`

use std::time::Instant;

use specular_texture::{DecodeOptions, Texture};

fn dds(w: u32, h: u32, dxgi: u32, data: &[u8]) -> Vec<u8> {
    let mut d = vec![0u8; 148];
    let put = |d: &mut Vec<u8>, p: usize, v: u32| d[p..p + 4].copy_from_slice(&v.to_le_bytes());
    d[..4].copy_from_slice(b"DDS ");
    put(&mut d, 4, 124);
    put(&mut d, 8, 0x1007);
    put(&mut d, 12, h);
    put(&mut d, 16, w);
    put(&mut d, 28, 1);
    put(&mut d, 76, 32);
    put(&mut d, 80, 4);
    d[84..88].copy_from_slice(b"DX10");
    put(&mut d, 128, dxgi);
    put(&mut d, 132, 3);
    put(&mut d, 140, 1);
    d.extend_from_slice(data);
    d
}

fn best<T>(n: usize, mut f: impl FnMut() -> T) -> f64 {
    (0..n)
        .map(|_| {
            let t = Instant::now();
            std::hint::black_box(f());
            t.elapsed().as_secs_f64() * 1e3
        })
        .fold(f64::INFINITY, f64::min)
}

fn main() {
    let (w, h) = (2048u32, 2048u32);
    let blocks = (w / 4 * (h / 4)) as usize;
    let mut seed = 0x1234_5678_9abc_def1u64;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    println!(
        "{w}x{h}, ms, best of 5, {} threads",
        rayon::current_num_threads()
    );
    println!(
        "{:<6} {:>10} {:>12} {:>12} {:>12}",
        "format", "bcdec_rs", "ours 1T", "ours MT", "ours 1/4"
    );
    type Up = fn(&[u8], &mut [u8], usize);
    let cases: [(&str, u32, usize, Up); 3] = [
        ("BC1", 71, 8, bcdec_rs::bc1),
        ("BC3", 77, 16, bcdec_rs::bc3),
        ("BC7", 98, 16, bcdec_rs::bc7),
    ];
    for (name, code, bb, up) in cases {
        let data: Vec<u8> = (0..blocks * bb / 8)
            .flat_map(|_| rnd().to_le_bytes())
            .collect();
        let file = dds(w, h, code, &data);
        let t = Texture::open(&file).unwrap();
        let pitch = w as usize * 4;
        let upstream = best(5, || {
            let mut out = vec![0u8; pitch * h as usize];
            for (i, b) in data.chunks_exact(bb).enumerate() {
                let (bx, by) = (i % (w as usize / 4), i / (w as usize / 4));
                up(b, &mut out[by * 4 * pitch + bx * 16..], pitch);
            }
            out
        });
        let one = best(5, || {
            t.decode(
                &file,
                &DecodeOptions {
                    parallel: false,
                    ..Default::default()
                },
            )
            .unwrap()
        });
        let multi = best(5, || t.decode(&file, &DecodeOptions::default()).unwrap());
        let quarter = best(5, || {
            t.decode(
                &file,
                &DecodeOptions {
                    scale: 4,
                    ..Default::default()
                },
            )
            .unwrap()
        });
        println!("{name:<6} {upstream:>10.1} {one:>12.1} {multi:>12.1} {quarter:>12.1}");
    }
    corpus_bench();
}

/// A KTX2 wrapping the blocks of an `.astc` file (astcenc output) of a 2D footprint.
fn astc_file_to_ktx2(a: &[u8]) -> Option<Vec<u8>> {
    if a.len() < 16 || a[..4] != [0x13, 0xAB, 0xA1, 0x5C] {
        return None;
    }
    let dim = |p: usize| u32::from(a[p]) | u32::from(a[p + 1]) << 8 | u32::from(a[p + 2]) << 16;
    let (bw, bh, w, h) = (a[4], a[5], dim(7), dim(10));
    const FOOTPRINTS: [(u8, u8); 14] = [
        (4, 4),
        (5, 4),
        (5, 5),
        (6, 5),
        (6, 6),
        (8, 5),
        (8, 6),
        (8, 8),
        (10, 5),
        (10, 6),
        (10, 8),
        (10, 10),
        (12, 10),
        (12, 12),
    ];
    let i = FOOTPRINTS.iter().position(|&f| f == (bw, bh))? as u32;
    let data = &a[16..];
    let mut d = vec![
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    for v in [157 + 2 * i, 1, w, h, 0, 0, 1, 1, 0, 0, 0, 0, 0] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&[0; 16]);
    for v in [104u64, data.len() as u64, data.len() as u64] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(data);
    Some(d)
}

/// Real 2048² textures from `tools/corpus/fetch-textures.sh bench_textures` (skipped when
/// absent): ms and Mpixel/s, one thread, all threads, and 1/4 scale.
fn corpus_bench() {
    let dir = std::env::var_os("SPECULAR_LARGE_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/large")
        })
        .join("texture/bench");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        println!("no {} (fetch-textures.sh bench_textures)", dir.display());
        return;
    };
    let mut files: Vec<_> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e == "ktx" || e == "ktx2" || e == "astc")
        })
        .collect();
    files.sort();
    println!(
        "\n{:<34} {:>9} {:>9} {:>9} {:>9}",
        "file (best of 5)", "1T ms", "MT ms", "1/4 ms", "1T Mpx/s"
    );
    for p in files {
        let raw = std::fs::read(&p).unwrap();
        let file = if p.extension().is_some_and(|e| e == "astc") {
            match astc_file_to_ktx2(&raw) {
                Some(f) => f,
                None => continue,
            }
        } else {
            raw
        };
        let Ok(t) = Texture::open(&file) else {
            continue;
        };
        let run = |scale: u32, parallel: bool| {
            best(5, || {
                t.decode(
                    &file,
                    &DecodeOptions {
                        scale,
                        parallel,
                        ..Default::default()
                    },
                )
                .unwrap()
            })
        };
        let (one, multi, quarter) = (run(1, false), run(1, true), run(4, true));
        let mpx = f64::from(t.width) * f64::from(t.height) / 1e3 / one;
        println!(
            "{:<34} {one:>9.1} {multi:>9.1} {quarter:>9.1} {mpx:>9.0}",
            format!("{} {}", t.format.name(), t.width)
        );
    }
}
