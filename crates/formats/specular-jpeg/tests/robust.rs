//! Robustness: truncated / corrupted input must never panic, cancellation must work, and a
//! mismatched index must be rejected.

mod common;

use std::sync::atomic::AtomicBool;

use common::*;
use specular_jpeg::*;

const SCALES: [Scale; 4] = [Scale::Full, Scale::Half, Scale::Quarter, Scale::Eighth];

fn samples() -> Vec<(String, Vec<u8>)> {
    let src = source_ppm(301, 203, 99);
    let big = source_ppm(1400, 1100, 5);
    let mut v: Vec<(String, Vec<u8>)> = [
        ("r_420", &["-quality", "90", "-sample", "2x2"][..]),
        (
            "r_444_rst",
            &["-quality", "90", "-sample", "1x1", "-restart", "3B"],
        ),
        ("r_gray", &["-quality", "90", "-grayscale"]),
        (
            "r_420_prog",
            &["-quality", "90", "-sample", "2x2", "-progressive"],
        ),
        (
            "r_444_prog_rst",
            &[
                "-quality",
                "90",
                "-sample",
                "1x1",
                "-progressive",
                "-restart",
                "1",
            ],
        ),
    ]
    .iter()
    .map(|(n, a)| (n.to_string(), cjpeg(&src, n, a)))
    .collect();
    // Large enough for the speculative parallel path (several 128 KiB chunks).
    v.push((
        "r_big_444".into(),
        cjpeg(&big, "r_big_444_q98", &["-quality", "98", "-sample", "1x1"]),
    ));
    v
}

/// Decode with every scale and a couple of parallelism settings; must not panic.
fn try_all(data: &[u8]) {
    let _ = read_info(data);
    for scale in SCALES {
        for par in [Parallelism::Single, Parallelism::Threads(4)] {
            let o = DecodeOptions {
                scale,
                region: None,
                cancel: None,
                parallelism: par,
            };
            if let Ok(d) = decode(data, &o) {
                let _ = planes_to_rgba(&d);
            }
        }
    }
    let o = DecodeOptions {
        scale: Scale::Half,
        region: Some(Region {
            x: 10,
            y: 7,
            w: 40,
            h: 30,
        }),
        ..Default::default()
    };
    let _ = decode(data, &o);
    if let Ok(ix) = build_index(data) {
        let _ = decode_with_index(data, &ix, &o);
    }
    let _ = decode_and_index(data, &DecodeOptions::default());
}

#[test]
fn truncated_input_never_panics() {
    if !tools_available() {
        return;
    }
    for (name, jpg) in samples() {
        let n = jpg.len();
        let mut cuts: Vec<usize> = (0..64).chain((64..n).step_by((n / 97).max(1))).collect();
        cuts.extend([n - 2, n - 1]);
        for cut in cuts {
            let data = &jpg[..cut.min(n)];
            try_all(data);
            // Once the scan has started, truncation yields a partial image flagged `truncated`.
            if cut > n / 2 && cut < n - 2 {
                let d = decode(data, &DecodeOptions::default());
                if let Ok(d) = d {
                    assert!(
                        d.truncated,
                        "{name}: cut at {cut}/{n} not flagged as truncated"
                    );
                }
            }
        }
    }
}

#[test]
fn corrupted_input_never_panics() {
    if !tools_available() {
        return;
    }
    let mut rng = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for (_, jpg) in samples() {
        for round in 0..60 {
            let mut d = jpg.clone();
            let flips = 1 + (next() % 8) as usize;
            for _ in 0..flips {
                // Half the rounds hit the headers (first ~700 bytes), the rest anywhere.
                let lim = if round % 2 == 0 {
                    d.len().min(700)
                } else {
                    d.len()
                };
                let i = (next() as usize) % lim;
                d[i] = match next() % 4 {
                    0 => 0xFF,
                    1 => 0x00,
                    _ => next() as u8,
                };
            }
            try_all(&d);
        }
    }
}

#[test]
fn garbage_and_edge_inputs() {
    for data in [
        &b""[..],
        b"\xFF",
        b"\xFF\xD8",
        b"\xFF\xD8\xFF",
        b"\xFF\xD8\xFF\xD9",
        b"not a jpeg at all",
        &[0xFF; 300],
    ] {
        assert!(decode(data, &DecodeOptions::default()).is_err());
        assert!(read_info(data).is_err());
        try_all(data);
    }
    // A header claiming a huge image with no data must fail cleanly (not allocate wildly).
    let mut tiny = include_bytes!("fixtures/gray8.jpg").to_vec();
    let sof = tiny.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
    tiny[sof + 5..sof + 9].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
    try_all(&tiny);
}

#[test]
fn fractional_sampling_is_rejected() {
    // Fuzzer finding: Y 3x1, Cb 2x1 (max_h % h != 0) used to panic in planes_to_rgba. libjpeg
    // rejects such frames ("Fractional sampling not implemented yet"); so do we.
    if !tools_available() {
        return;
    }
    let mut jpg = samples().swap_remove(0).1; // r_420: Y 2x2, Cb 1x1, Cr 1x1
    let sof = jpg.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
    jpg[sof + 11] = 0x31; // Y: 3x1
    jpg[sof + 14] = 0x21; // Cb: 2x1
    jpg[sof + 17] = 0x11;
    assert!(read_info(&jpg).is_ok());
    assert!(matches!(
        decode(&jpg, &DecodeOptions::default()),
        Err(JpegError::Unsupported(_))
    ));
    try_all(&jpg);
}

#[test]
fn cancel_flag_is_honoured() {
    if !tools_available() {
        return;
    }
    let flag = AtomicBool::new(true);
    for (name, jpg) in samples() {
        for scale in SCALES {
            for par in [Parallelism::Single, Parallelism::Threads(4)] {
                let o = DecodeOptions {
                    scale,
                    region: None,
                    cancel: Some(&flag),
                    parallelism: par,
                };
                assert_eq!(
                    decode(&jpg, &o).err(),
                    Some(JpegError::Cancelled),
                    "{name} {scale:?} {par:?}"
                );
            }
        }
        let o = DecodeOptions {
            cancel: Some(&flag),
            ..Default::default()
        };
        assert_eq!(
            build_index_with(&jpg, &o).err(),
            Some(JpegError::Cancelled),
            "{name} index"
        );
    }
}

#[test]
fn mismatched_index_is_rejected() {
    if !tools_available() {
        return;
    }
    let s = samples();
    let ix = build_index(&s[0].1).unwrap();
    for (name, jpg) in &s[1..] {
        assert_eq!(
            decode_with_index(jpg, &ix, &DecodeOptions::default()).err(),
            Some(JpegError::IndexMismatch),
            "{name}"
        );
    }
}
