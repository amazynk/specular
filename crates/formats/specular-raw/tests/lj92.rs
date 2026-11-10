//! LJ92 encode → decode round trips over predictors, precisions, components and restarts.

mod common;

use common::{Lj92, lj92_encode, test_pattern};
use specular_raw::ljpeg;

fn roundtrip(p: &Lj92) {
    let data = test_pattern(
        p.width as usize,
        p.height as usize,
        p.components as usize,
        p.precision as u32,
        p.predictor as u32,
    );
    let enc = lj92_encode(p, &data);
    let (h, out, truncated) = ljpeg::decode(&enc, 1 << 24).expect("decode");
    assert!(!truncated);
    assert_eq!(
        (h.width, h.height, h.components, h.predictor),
        (p.width, p.height, p.components, p.predictor)
    );
    let pt = p.point_transform;
    let want: Vec<u16> = data.iter().map(|&v| (v >> pt) << pt).collect();
    if out != want {
        let i = out.iter().zip(&want).position(|(a, b)| a != b).unwrap();
        panic!(
            "mismatch at {i}: {} != {} ({:?})",
            out[i],
            want[i],
            (p.predictor, p.components, p.restart_interval)
        );
    }
}

#[test]
fn all_predictors_and_components() {
    for predictor in 1..=7u8 {
        for components in 1..=4u32 {
            for precision in [8u8, 12, 14, 16] {
                roundtrip(&Lj92 {
                    width: 37,
                    height: 11,
                    components,
                    precision,
                    predictor,
                    point_transform: 0,
                    restart_interval: 0,
                });
            }
        }
    }
}

#[test]
fn restart_intervals() {
    for ri in [1u16, 5, 37, 40, 100] {
        for predictor in [1u8, 4, 6] {
            roundtrip(&Lj92 {
                width: 37,
                height: 9,
                components: 2,
                precision: 14,
                predictor,
                point_transform: 0,
                restart_interval: ri,
            });
        }
    }
}

#[test]
fn point_transform() {
    for pt in [1u8, 2] {
        roundtrip(&Lj92 {
            width: 16,
            height: 8,
            components: 1,
            precision: 12,
            predictor: 1,
            point_transform: pt,
            restart_interval: 0,
        });
    }
}

#[test]
fn truncated_streams_never_panic() {
    let p = Lj92 {
        width: 32,
        height: 16,
        components: 2,
        precision: 14,
        predictor: 1,
        point_transform: 0,
        restart_interval: 0,
    };
    let enc = lj92_encode(&p, &test_pattern(32, 16, 2, 14, 3));
    for len in 0..enc.len() {
        let _ = ljpeg::decode(&enc[..len], 1 << 20);
    }
    // Truncated entropy data decodes with the truncated flag.
    let (_, _, t) = ljpeg::decode(&enc[..enc.len() * 2 / 3], 1 << 20).unwrap();
    assert!(t);
    let mut s = 1u32;
    for _ in 0..2000 {
        s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let mut m = enc.clone();
        let i = (s >> 8) as usize % m.len();
        m[i] ^= 1 << (s % 8);
        let _ = ljpeg::decode(&m, 1 << 20);
    }
    // Size limit.
    assert!(ljpeg::decode(&enc, 100).is_err());
}

/// Decode time of a 24 MP lossless-compressed DNG (LJ92 tiles, 14-bit with noise and a
/// clipped strip), split into the tiles and the develop statistics; median of 9. Run by hand:
/// `cargo test -p specular-raw --test lj92 sensor_decode_timing -- --ignored --nocapture`.
#[test]
#[ignore = "timing report"]
fn sensor_decode_timing() {
    use common::{DngCodec, DngSpec, write_dng};
    use specular_raw::{DecodeOptions, DevelopStats};
    use std::time::Instant;
    let (w, h) = (6000u32, 4000u32);
    let mut spec = DngSpec::new(w, h, DngCodec::Lj92Tiles(256));
    spec.bits = 14;
    spec.black = 512;
    spec.white = 16383;
    let mut seed = 0x9e37_79b9u32;
    let mut data = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let base =
                600.0 + 9000.0 * (0.5 + 0.5 * (fx * 7.0 + fy * 3.0).sin()) * (0.3 + 0.7 * fy);
            let noise = (seed % 64) as f32 - 32.0;
            // A clipped highlight (3 % of the frame): the pile-up search runs.
            let v = if fx > 0.97 { 16383.0 } else { base + noise };
            data.push(v.clamp(0.0, 16383.0) as u16);
        }
    }
    let file = write_dng(&spec, &data);
    let opts = DecodeOptions::default();
    let mut total = Vec::new();
    let mut tiles = Vec::new();
    let mut stats = Vec::new();
    for _ in 0..9 {
        let t0 = Instant::now();
        let f = specular_raw::parse(&file).expect("parse");
        let s = f.decode_sensor(&file, &opts).expect("sensor");
        total.push(t0.elapsed().as_secs_f64() * 1e3);
        assert_eq!(s.samples.len(), (w * h) as usize);
        let t1 = Instant::now();
        let raw = f.decode_raw_as_stored(&file, &opts).expect("raw");
        tiles.push(t1.elapsed().as_secs_f64() * 1e3);
        let t2 = Instant::now();
        let st = DevelopStats::of(&raw).expect("stats");
        stats.push(t2.elapsed().as_secs_f64() * 1e3);
        std::hint::black_box(st);
    }
    for v in [&mut total, &mut tiles, &mut stats] {
        v.sort_by(f64::total_cmp);
    }
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    eprintln!(
        "24 MP LJ92 DNG: decode_sensor p50 {:.1} ms (tiles {:.1}, statistics {:.1}); load {}",
        total[4],
        tiles[4],
        stats[4],
        load.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    );
}
