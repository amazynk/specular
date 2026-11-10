//! Compare against OpenJPEG's `opj_decompress` (skipped when it is not available) and the
//! pure-Rust `hayro-jpeg2000` crate.
//!
//! The fixtures in `tests/data` are produced by `tests/gen_fixtures.sh` with `opj_compress`.

mod common;

use common::*;
use specular_jp2::{DecodeOptions, Decoder, Depth, Pixels, Region};

/// Allowed max difference against OpenJPEG: reversible streams are exact; the irreversible
/// path differs only in float rounding.
fn tolerance(reversible: bool) -> u32 {
    if reversible { 0 } else { 1 }
}

#[test]
fn full_and_reduced_match_openjpeg() {
    let Some(bin) = opj_decompress() else {
        eprintln!("opj_decompress not found; skipping");
        return;
    };
    let mut checked = 0;
    for f in fixtures() {
        let n = name(&f);
        let data = std::fs::read(&f).unwrap();
        let dec = Decoder::new(&data).unwrap();
        let info = dec.info().clone();
        for reduce in 0..=info.levels.min(3) {
            let ours = decode16(&data, reduce, None);
            assert!(!ours.truncated, "{n} r{reduce}: truncated");
            let Some(r) = opj(
                &bin,
                &f,
                &info,
                &format!("r{reduce}"),
                &["-r".into(), reduce.to_string()],
            ) else {
                continue;
            };
            let (max, mean) = diff(&ours, &r, r.h);
            assert!(
                max <= tolerance(info.reversible),
                "{n} reduce {reduce}: max {max} mean {mean:.4}"
            );
            checked += 1;
        }
    }
    assert!(checked > 60, "only {checked} comparisons ran");
}

#[test]
fn regions_match_openjpeg() {
    let Some(bin) = opj_decompress() else {
        eprintln!("opj_decompress not found; skipping");
        return;
    };
    let regions = [
        (37u32, 21u32, 61u32, 47u32),
        (0, 0, 64, 64),
        (90, 70, 40, 30),
    ];
    for f in fixtures() {
        let n = name(&f);
        if n == "rgb_sub.jp2" {
            continue; // opj's -d is in reference-grid units; ours are output pixels.
        }
        let data = std::fs::read(&f).unwrap();
        let info = Decoder::new(&data).unwrap().info().clone();
        for reduce in 0..=info.levels.min(2) {
            let (fw, fh) = info.reduced_size(reduce);
            let fx0 = info.x0.div_ceil(1 << reduce);
            let fy0 = info.y0.div_ceil(1 << reduce);
            for &(x, y, w, h) in &regions {
                let (x, y) = (x >> reduce, y >> reduce);
                let (w, h) = ((w >> reduce).max(1), (h >> reduce).max(1));
                if x + w > fw || y + h > fh {
                    continue;
                }
                let region = Region { x, y, w, h };
                let ours = decode16(&data, reduce, Some(region));
                // Absolute full-resolution coordinates that reduce to exactly this window.
                let s = 1u32 << reduce;
                let ax0 = (fx0 + x) * s;
                let ay0 = (fy0 + y) * s;
                let ax1 = ((fx0 + x + w) * s).min(info.x0 + info.width);
                let ay1 = ((fy0 + y + h) * s).min(info.y0 + info.height);
                let Some(r) = opj(
                    &bin,
                    &f,
                    &info,
                    &format!("d{reduce}_{x}_{y}"),
                    &[
                        "-r".into(),
                        reduce.to_string(),
                        "-d".into(),
                        format!("{ax0},{ay0},{ax1},{ay1}"),
                    ],
                ) else {
                    continue;
                };
                let (max, mean) = diff(&ours, &r, r.h);
                assert!(
                    max <= tolerance(info.reversible),
                    "{n} reduce {reduce} region {region:?}: max {max} mean {mean:.4}"
                );
            }
        }
    }
}

#[test]
fn layers_match_openjpeg() {
    let Some(bin) = opj_decompress() else {
        eprintln!("opj_decompress not found; skipping");
        return;
    };
    for n in ["rgb_layers.jp2", "rgb_97_layers.j2k", "rgb_tileparts.j2k"] {
        let f = data_dir().join(n);
        let data = std::fs::read(&f).unwrap();
        let dec = Decoder::new(&data).unwrap();
        let info = dec.info().clone();
        for l in 1..info.layers {
            let ours = dec
                .decode(&DecodeOptions {
                    max_layers: Some(l),
                    depth: Depth::Sixteen,
                    ..Default::default()
                })
                .unwrap();
            let r = opj(
                &bin,
                &f,
                &info,
                &format!("l{l}"),
                &["-l".into(), l.to_string()],
            )
            .unwrap();
            let (max, mean) = diff(&ours, &r, r.h);
            assert!(
                max <= tolerance(info.reversible),
                "{n} layers {l}: max {max} mean {mean:.4}"
            );
        }
    }
}

/// Columns to compare against hayro: in tiles only 3 samples wide (x >= 200 with 100-wide tiles)
/// hayro deviates by up to 2 from both OpenJPEG and us on lossless streams.
fn hayro_cols(file: &str, w: usize) -> usize {
    if matches!(file, "rgb_plt_tlm.j2k" | "rgb_rpcl.j2k") {
        200
    } else {
        w
    }
}

/// hayro-jpeg2000 is always available. Lossless streams must match exactly; for 9/7 its
/// output deviates from OpenJPEG by up to ~17 levels itself, so only a loose bound applies.
#[test]
fn matches_hayro() {
    for f in fixtures() {
        let n = name(&f);
        let data = std::fs::read(&f).unwrap();
        let info = Decoder::new(&data).unwrap().info().clone();
        let ours = specular_jp2::decode(&data, &DecodeOptions::default()).unwrap();
        let Pixels::U8(px) = &ours.pixels else {
            unreachable!()
        };
        let img =
            hayro_jpeg2000::Image::new(&data, &hayro_jpeg2000::DecodeSettings::default()).unwrap();
        let h = img.decode().unwrap();
        assert_eq!(
            (img.width(), img.height()),
            (ours.width, ours.height),
            "{n}"
        );
        let hc = img.color_space().num_channels() as usize + usize::from(img.has_alpha());
        let oc = ours.format.channels();
        let rows = ours.height as usize;
        let (mut max, mut sum, mut cnt) = (0u32, 0u64, 0u64);
        let w = ours.width as usize;
        let cols = hayro_cols(&n, w);
        for i in (0..rows * w).filter(|i| i % w < cols) {
            for c in 0..hc {
                let d = (px[i * oc + our_channel(hc, c)] as u32).abs_diff(h[i * hc + c] as u32);
                max = max.max(d);
                sum += d as u64;
                cnt += 1;
            }
        }
        let mean = sum as f64 / cnt as f64;
        // Rate-limited 5/3 streams stop mid bit-plane, where the reconstruction point
        // (mid-interval rounding) is a decoder choice; hayro's differs from OpenJPEG's and ours.
        let truncated_53 = matches!(n.as_str(), "rgb_sop_eph.j2k" | "rgb_tileparts.j2k");
        if info.reversible && !truncated_53 {
            assert_eq!(max, 0, "{n}: mean {mean:.4}");
        } else {
            assert!(max <= 24 && mean < 3.0, "{n}: max {max} mean {mean:.4}");
        }
    }
}

/// 8-bit output is the 16-bit output's native value rescaled to 8 bits.
#[test]
fn eight_and_sixteen_bit_agree() {
    for f in fixtures() {
        let n = name(&f);
        let data = std::fs::read(&f).unwrap();
        let info = Decoder::new(&data).unwrap().info().clone();
        let a = specular_jp2::decode(&data, &DecodeOptions::default()).unwrap();
        let b = decode16(&data, 0, None);
        let (Pixels::U8(p8), Pixels::U16(p16)) = (&a.pixels, &b.pixels) else {
            unreachable!()
        };
        assert_eq!(p8.len(), p16.len(), "{n}");
        let vmax = (1u64 << info.components[0].precision.min(16)) - 1;
        for (i, (&x, &y)) in p8.iter().zip(p16).enumerate() {
            let v = (y as u64 * vmax + 32767) / 65535;
            let want = ((v * 255 + vmax / 2) / vmax) as u8;
            assert_eq!(x, want, "{n} sample {i}");
        }
    }
}
