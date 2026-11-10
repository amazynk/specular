//! Hierarchical JPEG (T.81 Annex J) against the ISO reference libjpeg: the fixtures in
//! `tests/fixtures/hier` were encoded by its `jpeg` tool, which also decoded the expected
//! images (see `tests/fixtures/make-hier.sh`).

use std::path::PathBuf;

use specular_jpeg::*;

fn fixture(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/hier")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// (width, height, channels, samples) of a binary PGM/PPM, 8 or 16 bits.
fn pnm(b: &[u8]) -> (usize, usize, usize, Vec<u16>) {
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while b[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !b[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&b[s..i]).to_string());
    }
    i += 1;
    let ch = if fields[0] == "P6" { 3 } else { 1 };
    let (w, h, max): (usize, usize, u32) = (
        fields[1].parse().unwrap(),
        fields[2].parse().unwrap(),
        fields[3].parse().unwrap(),
    );
    let n = w * h * ch;
    let data = if max > 255 {
        b[i..i + 2 * n]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect()
    } else {
        b[i..i + n].iter().map(|&v| v as u16).collect()
    };
    (w, h, ch, data)
}

/// Mismatching samples and the largest difference between `got` (RGB or grey, `ch` per
/// pixel) and the expected PNM samples.
fn compare(got: &[u8], want: &[u8]) -> (usize, u8) {
    assert_eq!(got.len(), want.len());
    let bad = got.iter().zip(want).filter(|(a, b)| a != b).count();
    let max = got
        .iter()
        .zip(want)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    (bad, max)
}

/// Our decode as RGB (or grey) bytes plus the expected ones in 8 bits.
fn decode_vs_oracle(name: &str) -> (Vec<u8>, Vec<u8>) {
    let jpg = fixture(&format!("{name}.jpg"));
    let ext = if name.starts_with("rgb") {
        "ppm"
    } else {
        "pgm"
    };
    let flat = name.ends_with("_flat");
    let (w, h, ch, want) = pnm(&fixture(&format!("{name}.{ext}")));
    let info = read_info(&jpg).unwrap();
    assert_eq!((info.width, info.height), (w as u32, h as u32), "{name}");
    assert_eq!(is_hierarchical(&jpg), !flat, "{name}");
    let d = decode(&jpg, &DecodeOptions::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!((d.width, d.height, d.origin), (w as u32, h as u32, (0, 0)));
    assert!(!d.truncated);
    let got: Vec<u8> = if ch == 1 {
        assert_eq!(d.layout, PlanesLayout::Gray);
        let p = &d.planes[0];
        (0..h)
            .flat_map(|y| p.data[y * p.stride..y * p.stride + w].to_vec())
            .collect()
    } else {
        planes_to_rgba(&d)
            .chunks_exact(4)
            .flat_map(|px| px[..3].to_vec())
            .collect()
    };
    let want: Vec<u8> = if info.precision == 12 {
        want.iter()
            .map(|&v| ((v as u32 * 255 + 2047) / 4095) as u8)
            .collect()
    } else {
        want.iter().map(|&v| v as u8).collect()
    };
    (got, want)
}

/// (fixture, largest sample difference allowed, mismatching samples allowed in per mille).
/// The reference tool's integer IDCT and ours (exact, see `src/hier.rs`) round a few samples
/// differently, and every level of the pyramid passes that on. The 4:2:0 colour file also
/// differs in the chroma upsampling and YCbCr conversion; its non-hierarchical twin
/// `rgb_420_flat` (same source, same settings) shows how much that alone accounts for.
const CASES: &[(&str, u8, usize)] = &[
    ("grey_y2", 1, 30),
    ("grey_y3_arith", 1, 30),
    ("grey_y2_prog", 1, 35),
    ("grey_y0_lossless", 1, 15),
    ("grey12_y2", 1, 25),
    ("rgb_y2_nocolour", 1, 25),
    ("rgb_y2_420", 3, 500),
    ("rgb_420_flat", 3, 500),
];

#[test]
fn hierarchical_matches_reference_libjpeg() {
    let mut fails = Vec::new();
    let mut counts = std::collections::HashMap::new();
    for &(name, max_ok, permille) in CASES {
        let (got, want) = decode_vs_oracle(name);
        let (bad, max) = compare(&got, &want);
        eprintln!("{name}: {bad} of {} samples differ, max {max}", got.len());
        counts.insert(name, bad);
        if max > max_ok || bad * 1000 > permille * got.len() {
            fails.push(format!("{name}: {bad} of {} differ, max {max}", got.len()));
        }
    }
    // The hierarchical 4:2:0 file is no further off than its flat twin, give or take 10%.
    assert!(
        counts["rgb_y2_420"] * 10 <= counts["rgb_420_flat"] * 11,
        "{counts:?}"
    );
    assert!(fails.is_empty(), "{fails:?}");
}

#[test]
fn hierarchical_scaled_and_region_decodes_agree_with_full() {
    let jpg = fixture("grey_y3_arith.jpg");
    let full = decode(&jpg, &DecodeOptions::default()).unwrap();
    let fp = &full.planes[0];
    // Half scale: 2x2 box averages of the full decode.
    let opts = DecodeOptions {
        scale: Scale::Half,
        ..Default::default()
    };
    let half = decode(&jpg, &opts).unwrap();
    assert_eq!((half.width, half.height), (66, 49));
    let hp = &half.planes[0];
    for y in 0..49usize {
        for x in 0..66usize {
            let (mut s, mut n) = (0u32, 0u32);
            for yy in 2 * y..(2 * y + 2).min(97) {
                for xx in 2 * x..(2 * x + 2).min(131) {
                    s += fp.data[yy * fp.stride + xx] as u32;
                    n += 1;
                }
            }
            assert_eq!(
                hp.data[y * hp.stride + x] as u32,
                (s + n / 2) / n,
                "({x}, {y})"
            );
        }
    }
    // A region is a crop of the full decode.
    let r = Region {
        x: 10,
        y: 20,
        w: 50,
        h: 30,
    };
    let opts = DecodeOptions {
        region: Some(r),
        ..Default::default()
    };
    let part = decode(&jpg, &opts).unwrap();
    assert_eq!((part.origin, part.width, part.height), ((10, 20), 50, 30));
    let pp = &part.planes[0];
    for y in 0..30usize {
        for x in 0..50usize {
            assert_eq!(
                pp.data[y * pp.stride + x],
                fp.data[(y + 20) * fp.stride + x + 10]
            );
        }
    }
    // An index is available (it only records the fingerprint) and gives the same pixels.
    let (d, ix) = decode_and_index(&jpg, &DecodeOptions::default()).unwrap();
    assert_eq!(d, full);
    assert_eq!(decode_with_index(&jpg, &ix, &opts).unwrap(), part);
    assert_eq!(build_index(&jpg).unwrap(), ix);
}

#[test]
fn hierarchical_is_refused_where_there_is_no_single_frame() {
    let jpg = fixture("grey_y2.jpg");
    assert!(matches!(
        read_coefficients(&jpg),
        Err(JpegError::Unsupported(_))
    ));
    // The stored values are the final components (see tests/wide.rs).
    assert_eq!(decode_wide(&jpg, None).unwrap().planes.len(), 1);
}

#[test]
fn hierarchical_corrupt_bytes_never_panic() {
    for &(name, _, _) in CASES {
        let jpg = fixture(&format!("{name}.jpg"));
        for i in 0..400usize {
            let mut bad = jpg.clone();
            let at = (i * 7919) % bad.len();
            bad[at] ^= 1 << (i % 8);
            let _ = decode(&bad, &DecodeOptions::default());
        }
    }
}

#[test]
fn hierarchical_truncated_is_an_error_not_a_panic() {
    let jpg = fixture("rgb_y2_420.jpg");
    for cut in [jpg.len() / 3, jpg.len() / 2, jpg.len() - 40] {
        let _ = decode(&jpg[..cut], &DecodeOptions::default());
    }
}

/// Timing of whole decodes: `SPECULAR_JPEG_BENCH=FILE,... cargo test --test hier -- --ignored`.
#[test]
#[ignore]
fn bench_files() {
    let Ok(list) = std::env::var("SPECULAR_JPEG_BENCH") else {
        return;
    };
    for f in list.split(',') {
        let jpg = std::fs::read(f).unwrap();
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            decode(&jpg, &DecodeOptions::default()).unwrap();
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
        }
        eprintln!("{f}: {best:.0} ms");
    }
}
