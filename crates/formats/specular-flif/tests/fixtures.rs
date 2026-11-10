//! Committed fixtures (tests/data, encoded by the reference encoder from the sources of
//! tools/corpus/flif-generate.py; see tools/corpus/fetch-editors-flif.sh). Their expected
//! pixels are hashes of the output, which the corpus oracle test compares sample by sample
//! with the reference decoder.

use specular_flif::{Error, Options, PixelFormat, decode, probe, probe_file, read_info};
use std::path::Path;

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

fn fnv(bytes: impl IntoIterator<Item = u8>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn hash_of(name: &str) -> (u32, u32, PixelFormat, usize, u64, bool) {
    let img = decode(&data(name), &Options::default()).unwrap();
    let h = fnv(img.frames.iter().flat_map(|f| f.data.iter().copied()));
    (
        img.width,
        img.height,
        img.format,
        img.frames.len(),
        h,
        img.truncated,
    )
}

const FIXTURES: &[(&str, u32, u32, PixelFormat, usize, u64, bool)] = &[
    (
        "anim-rgb-I.flif",
        96,
        64,
        PixelFormat::Rgba8,
        6,
        0x5f2974cc90917142,
        false,
    ),
    (
        "anim-rgba-L3N.flif",
        96,
        64,
        PixelFormat::Rgba8,
        6,
        0x8ca09557a8d167b9,
        false,
    ),
    (
        "grad-grey16-I.flif",
        181,
        97,
        PixelFormat::Gray16,
        1,
        0xc490cc0a45a47b2d,
        false,
    ),
    (
        "grad-rgb10-N.flif",
        160,
        120,
        PixelFormat::Rgba16,
        1,
        0x65dda175e1ebea7a,
        false,
    ),
    (
        "palette-rgb8-I-cut35.flif",
        210,
        150,
        PixelFormat::Rgba8,
        1,
        0x8414b2ba8062aac5,
        true,
    ),
    (
        "palette-rgba8-K.flif",
        210,
        150,
        PixelFormat::Rgba8,
        1,
        0xcd0f9c3651f7a9c9,
        false,
    ),
    (
        "tiny-rgba-5x4-I.flif",
        5,
        4,
        PixelFormat::Rgba8,
        1,
        0x7ebf94806ece1a10,
        false,
    ),
    (
        "cb-AN.flif",
        48,
        32,
        PixelFormat::Rgba8,
        1,
        0xd7f6e7e4efe611a5,
        false,
    ),
    (
        "cb-AI.flif",
        48,
        32,
        PixelFormat::Rgba8,
        1,
        0xd7f6e7e4efe611a5,
        false,
    ),
    (
        "meta.flif",
        16,
        8,
        PixelFormat::Rgba8,
        1,
        0x5b0836aed4064ce5,
        false,
    ),
];

#[test]
fn fixtures_decode_to_known_pixels() {
    let mut wrong = Vec::new();
    for &(name, w, h, fmt, frames, hash, truncated) in FIXTURES {
        let got = hash_of(name);
        if got != (w, h, fmt, frames, hash, truncated) {
            wrong.push(format!(
                "(\"{name}\", {}, {}, PixelFormat::{:?}, {}, {:#x}, {}),",
                got.0, got.1, got.2, got.3, got.4, got.5
            ));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

#[test]
fn info_and_metadata() {
    let i = read_info(&data("meta.flif")).unwrap();
    assert_eq!(
        (i.width, i.height, i.channels, i.bit_depth, i.frames),
        (16, 8, 3, 8, 1)
    );
    let icc = i.icc.expect("ICC profile");
    assert_eq!(&icc[36..40], b"acsp");
    let a = read_info(&data("anim-rgba-L3N.flif")).unwrap();
    assert_eq!(
        (a.channels, a.frames, a.interlaced, a.loop_count),
        (4, 6, false, 0)
    );
    let g = read_info(&data("grad-rgb10-N.flif")).unwrap();
    assert_eq!((g.bit_depth, g.significant_bits), (16, 10));
    let img = decode(&data("anim-rgb-I.flif"), &Options::default()).unwrap();
    assert!(img.frames.iter().all(|f| f.delay_ms == 100));
    assert!(probe(b"FLIFQ1"));
    assert!(probe(b"FLIF"));
    assert!(!probe(b"FLIX1"));
    assert!(!probe(b"FLIFZ"));
}

#[test]
fn first_frame_only() {
    let all = decode(&data("anim-rgba-L3N.flif"), &Options::default()).unwrap();
    let one = decode(
        &data("anim-rgba-L3N.flif"),
        &Options {
            first_frame_only: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(one.frames.len(), 1);
    assert_eq!(one.frames[0].data, all.frames[0].data);
}

/// A downscaled decode of an interlaced file is the full decode sampled every `scale`
/// pixels (invisible pixels aside).
#[test]
fn scaled_decode_is_subsampled_full_decode() {
    for name in [
        "grad-grey16-I.flif",
        "anim-rgb-I.flif",
        "cb-AI.flif",
        "tiny-rgba-5x4-I.flif",
    ] {
        let d = data(name);
        let full = decode(&d, &Options::default()).unwrap();
        let bpp = full.format.bytes_per_pixel();
        for scale in [2u32, 4, 8] {
            let s = decode(
                &d,
                &Options {
                    scale,
                    ..Options::default()
                },
            )
            .unwrap();
            // Tiny images stop at their last zoomlevel, a smaller factor.
            let eff = (0..=scale.trailing_zeros())
                .rev()
                .map(|k| 1u32 << k)
                .find(|&e| (full.width.div_ceil(e), full.height.div_ceil(e)) == (s.width, s.height))
                .unwrap_or_else(|| panic!("{name} 1/{scale}: {}x{}", s.width, s.height));
            let (sx, sy) = (eff, eff);
            for (ff, sf) in full.frames.iter().zip(&s.frames) {
                for y in 0..s.height {
                    for x in 0..s.width {
                        let a = &sf.data[((y * s.width + x) as usize) * bpp..][..bpp];
                        let b =
                            &ff.data[(((y * sy) * full.width + x * sx) as usize) * bpp..][..bpp];
                        assert_eq!(a, b, "{name} 1/{scale} at {x},{y}");
                    }
                }
            }
        }
    }
}

#[test]
fn cancel_and_memory_budget() {
    let d = data("grad-grey16-I.flif");
    let stop = || true;
    let e = decode(
        &d,
        &Options {
            cancel: Some(&stop),
            ..Options::default()
        },
    )
    .unwrap_err();
    assert_eq!(e, Error::Cancelled);
    let e =
        specular_core::budget::with_limit(1 << 12, || decode(&d, &Options::default())).unwrap_err();
    assert!(matches!(e, Error::NeedsMemory(_)), "{e:?}");
}

#[test]
fn damaged_files_never_panic() {
    let mut seed = 0x9e37_79b9u32;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let t = std::time::Instant::now();
    for &(name, ..) in FIXTURES {
        let good = data(name);
        let step = (good.len() / 97).max(1);
        for n in (0..good.len()).step_by(step) {
            let _ = decode(&good[..n], &Options::default());
            let _ = read_info(&good[..n]);
        }
        for _ in 0..60 {
            let mut b = good.clone();
            for _ in 0..1 + rnd() % 4 {
                let at = rnd() as usize % b.len();
                b[at] = rnd() as u8;
            }
            let _ = decode(&b, &Options::default());
        }
    }
    // Headers that ask for huge images are refused before allocating.
    let mut big = b"FLIF41".to_vec();
    big.extend_from_slice(&[0xff, 0xff, 0x7f, 0xff, 0xff, 0x7f, 0]);
    assert!(matches!(
        decode(&big, &Options::default()),
        Err(Error::TooLarge)
    ));
    assert!(t.elapsed().as_secs() < 120, "{:?}", t.elapsed());
}

/// A FLIF archive: `ar rc archive-tiny-rgba.flif readme.txt __image.flif` (GNU ar) with
/// tiny-rgba-5x4-I.flif as the image. The reference decoder (`flif -d`) decodes it to the
/// same PAM as the bare file; so must we.
#[test]
fn archive_wrapper() {
    let ar = data("archive-tiny-rgba.flif");
    let bare = data("tiny-rgba-5x4-I.flif");
    assert!(!probe(&ar));
    assert!(probe_file(&ar));
    let (a, b) = (
        decode(&ar, &Options::default()).unwrap(),
        decode(&bare, &Options::default()).unwrap(),
    );
    assert_eq!((a.width, a.height), (b.width, b.height));
    assert!(a.frames[0].data == b.frames[0].data);
    assert_eq!(read_info(&ar).unwrap().width, 5);
    // An archive without an `__image.flif` member, and one cut inside a member header.
    let mut other = ar.clone();
    let at = other
        .windows(13)
        .position(|w| w == b"__image.flif/")
        .unwrap();
    other[at] = b'x';
    assert!(matches!(
        decode(&other, &Options::default()),
        Err(Error::Malformed(_))
    ));
    assert!(!probe_file(&other));
    assert!(matches!(read_info(&ar[..30]), Err(Error::Malformed(_))));
}
