//! Malformed input: truncation at every length, corruption, absurd headers. Nothing may panic;
//! truncated files decode their available rows.

mod common;

use common::*;
use specular_png::{
    Animator, DecodeOptions, Output, Parallelism, decode, decode_adam7_preview, read_info,
};

fn all_ways(data: &[u8]) {
    let _ = read_info(data);
    for output in [Output::Auto8, Output::Rgba16] {
        for (scale, par) in [(1, Parallelism::Single), (3, Parallelism::Threads)] {
            let o = DecodeOptions {
                output,
                scale,
                parallelism: par,
                ..Default::default()
            };
            if let Ok(img) = decode(data, &o) {
                let _ = img.to_rgba8();
            }
            let _ = decode(
                data,
                &DecodeOptions {
                    verify_checksums: true,
                    ..o
                },
            );
        }
    }
    let _ = decode_adam7_preview(
        data,
        &DecodeOptions {
            scale: 8,
            ..Default::default()
        },
    );
    if let Ok(mut a) = Animator::new(data, Output::Rgba8, 2) {
        let mut n = 0;
        while let Some(f) = a.next_frame(None) {
            n += 1;
            if f.is_err() || n > 100 {
                break;
            }
        }
    }
}

fn samples() -> Vec<Vec<u8>> {
    let mut v = Vec::new();
    for (ct, depth, interlace) in [
        (6u8, 8u8, false),
        (2, 16, true),
        (3, 2, false),
        (0, 1, true),
    ] {
        let s = make(29, 23, ct, depth, Content::Photo, 3);
        let extra = Extra {
            palette: (ct == 3).then(|| palette(depth, 1)),
            idat_size: Some(100),
            ..Default::default()
        };
        v.push(encode(&s, interlace, Filters::Mixed, &extra, 2));
    }
    let frames: Vec<_> = (0..3)
        .map(|i| ApngFrame {
            s: make(10 + i, 8, 6, 8, Content::Noise, i as u64 + 1),
            x: i as u32,
            y: 1,
            delay: (1, 10),
            dispose: (i % 3) as u8,
            blend: (i % 2) as u8,
        })
        .collect();
    v.push(encode_apng(
        16,
        12,
        &frames,
        None,
        false,
        0,
        &Extra::default(),
    ));
    v.push(encode_idot(&make(40, 40, 2, 8, Content::Screen, 1), 3));
    v
}

#[test]
fn truncation() {
    for data in samples() {
        for n in 0..data.len() {
            all_ways(&data[..n]);
        }
    }
}

#[test]
fn truncated_rows_kept() {
    let s = make(64, 64, 2, 8, Content::Photo, 3);
    let data = encode(
        &s,
        false,
        Filters::Mixed,
        &Extra {
            level: Some(0),
            ..Default::default()
        },
        1,
    );
    let full = decode(&data, &Default::default()).unwrap();
    let img = decode(&data[..data.len() * 2 / 3], &Default::default()).unwrap();
    assert!(img.truncated);
    let full8 = full.to_rgba8();
    let part8 = img.to_rgba8();
    // The top rows are intact; the bottom is grey.
    assert_eq!(&part8[..64 * 4 * 20], &full8[..64 * 4 * 20]);
    assert_eq!(&part8[part8.len() - 4..], &[128, 128, 128, 255]);
    // Interlaced: a partial image of the whole frame.
    let data = encode(
        &s,
        true,
        Filters::Mixed,
        &Extra {
            level: Some(0),
            ..Default::default()
        },
        1,
    );
    let img = decode(&data[..data.len() / 2], &Default::default()).unwrap();
    assert!(img.truncated);
}

#[test]
fn corruption() {
    let mut r = Rng(12345);
    for data in samples() {
        for _ in 0..1500 {
            let mut d = data.clone();
            for _ in 0..1 + r.below(4) {
                let i = r.below(d.len() as u64) as usize;
                d[i] = r.next() as u8;
            }
            all_ways(&d);
        }
    }
}

#[test]
fn hostile_headers() {
    // Huge dimensions: must fail cleanly, not allocate terabytes.
    for (w, h) in [
        (0x7fff_ffff, 0x7fff_ffff),
        (1 << 20, 1 << 20),
        (0, 5),
        (1, 0x7fff_ffff),
    ] {
        let mut d = SIG.to_vec();
        chunk(&mut d, b"IHDR", &ihdr(w, h, 6, 16, false));
        chunk(&mut d, b"IDAT", &zlib(&[0u8; 100], 6));
        chunk(&mut d, b"IEND", &[]);
        all_ways(&d);
    }
    // Bad colour type / depth combinations.
    for (ct, depth) in [(2u8, 4u8), (3, 16), (7, 8), (0, 3)] {
        let mut d = SIG.to_vec();
        chunk(&mut d, b"IHDR", &ihdr(4, 4, ct, depth, false));
        chunk(&mut d, b"IDAT", &zlib(&[0u8; 100], 6));
        chunk(&mut d, b"IEND", &[]);
        assert!(decode(&d, &Default::default()).is_err());
    }
    // Indexed without PLTE, index out of range, unknown filter type.
    let mut d = SIG.to_vec();
    chunk(&mut d, b"IHDR", &ihdr(4, 4, 3, 8, false));
    chunk(
        &mut d,
        b"IDAT",
        &zlib(&[0, 200, 200, 200, 200, 9, 1, 2, 3, 4].repeat(2), 6),
    );
    chunk(&mut d, b"IEND", &[]);
    let img = decode(&d, &Default::default()).unwrap();
    assert!(img.truncated);
    all_ways(&d);
}

/// A header claiming 6000 × 6000 RGBA (144 MB of output) in a 200 KB file: refused under a
/// 64 MiB memory budget before the output is allocated, with what it needs and what is free.
#[test]
fn output_within_memory_budget() {
    let mut d = SIG.to_vec();
    chunk(&mut d, b"IHDR", &ihdr(6000, 6000, 6, 8, false));
    chunk(&mut d, b"IDAT", &zlib(&[0u8; 100], 6));
    // Enough IDAT bytes that the size is plausible.
    chunk(&mut d, b"IDAT", &[0u8; 200_000]);
    chunk(&mut d, b"IEND", &[]);
    let o = DecodeOptions {
        output: Output::Rgba8,
        parallelism: Parallelism::Single,
        ..Default::default()
    };
    let e = specular_core::budget::with_limit(64 << 20, || decode(&d, &o)).unwrap_err();
    let specular_png::PngError::NeedsMemory(n) = e else {
        panic!("{e:?}")
    };
    assert!(n.needed >= 144_000_000 && n.needed > n.free, "{n:?}");
    // A small image under the same budget decodes.
    let mut s = SIG.to_vec();
    chunk(&mut s, b"IHDR", &ihdr(4, 4, 6, 8, false));
    chunk(&mut s, b"IDAT", &zlib(&[0u8; 4 * 17], 6));
    chunk(&mut s, b"IEND", &[]);
    assert!(specular_core::budget::with_limit(64 << 20, || decode(&s, &o)).is_ok());
}
