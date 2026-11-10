//! VA-API JPEG against our software decoder (specular-jpeg): the IDCTs differ, so the planes are
//! compared by PSNR against the floor the runtime check uses ([`specular_hwdec::MIN_PSNR_DB`]).
//!
//! The point is the parameter layout: `VAIQMatrixBufferJPEGBaseline` quantiser tables (the
//! file's zig-zag order), the Huffman tables, table selectors per component, restart intervals
//! and partial MCUs. Our encoder's images carry the Annex K tables, which are nearly symmetric,
//! so each image is also decoded with its tables rewritten to a strongly asymmetric shape (and
//! a different one per table): a table taken in the wrong order or from the wrong slot then
//! costs far more than the floor. Corpus JPEGs are added when present. Skipped without a
//! hardware JPEG decoder (unless `SPECULAR_REQUIRE_GPU` is set).

use specular_core::{PixelLayout, Raster, RasterPlane};
use specular_jpeg::encode::{EncodeOptions, Samples, Subsampling, encode};
use std::path::{Path, PathBuf};

/// Raster index of each zig-zag position.
fn zigzag() -> [usize; 64] {
    let mut z = [0; 64];
    let (mut k, mut s) = (0, 0);
    while k < 64 {
        // Anti-diagonal s: odd ones run down-left, even ones up-right.
        for i in 0..=s {
            let (x, y) = if s % 2 == 1 { (s - i, i) } else { (i, s - i) };
            if x < 8 && y < 8 {
                z[k] = y * 8 + x;
                k += 1;
            }
        }
        s += 1;
    }
    z
}

/// Rewrites every 8-bit DQT table of `jpeg` by `f(table id, raster index, value)`; `raster`
/// writes the result in raster order instead of zig-zag (a deliberately wrong file).
fn patch_dqt(jpeg: &[u8], f: impl Fn(u8, usize, u8) -> u8, raster: bool) -> Vec<u8> {
    let zz = zigzag();
    let mut out = jpeg.to_vec();
    let mut i = 2;
    while i + 4 <= out.len() && out[i] == 0xff {
        let m = out[i + 1];
        let len = u16::from_be_bytes([out[i + 2], out[i + 3]]) as usize;
        if m == 0xda {
            break;
        }
        if m == 0xdb {
            let mut p = i + 4;
            while p < i + 2 + len {
                let (pq, tq) = (out[p] >> 4, out[p] & 15);
                assert_eq!(pq, 0, "16-bit table");
                let mut t = [0u8; 64];
                for (k, &r) in zz.iter().enumerate() {
                    t[r] = f(tq, r, out[p + 1 + k]);
                }
                for k in 0..64 {
                    out[p + 1 + k] = if raster { t[k] } else { t[zz[k]] };
                }
                p += 65;
            }
        }
        i += 2 + len;
    }
    out
}

/// Strongly asymmetric: table 0 triples the vertical-dominant frequencies, table 1 the
/// horizontal-dominant ones (raster index `r` = 8 × vertical + horizontal frequency).
fn asymmetric(tq: u8, r: usize, q: u8) -> u8 {
    let (u, v) = (r % 8, r / 8);
    let up = if tq == 0 { v > u } else { u > v };
    if up { q.saturating_mul(3) } else { q }
}

/// tools/corpus of this checkout, or of the main checkout from a worktree (…/.claude/worktrees/
/// <name>), whose corpus is not copied.
fn corpus() -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus");
    if here.join("conformance/libjpeg-turbo").is_dir() {
        return here;
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == ".claude"))
        .and_then(Path::parent)
        .map_or(here, |p| p.join("tools/corpus"))
}

fn software(data: &[u8]) -> Raster {
    let s = specular_jpeg::decode(data, &specular_jpeg::DecodeOptions::default()).unwrap();
    let max_h = s.planes.iter().map(|p| p.h_samp).max().unwrap_or(1);
    let max_v = s.planes.iter().map(|p| p.v_samp).max().unwrap_or(1);
    Raster {
        width: s.width,
        height: s.height,
        origin: (0, 0),
        scale: 1,
        layout: if s.planes.len() == 1 {
            PixelLayout::Gray
        } else {
            PixelLayout::YCbCr
        },
        planes: s
            .planes
            .into_iter()
            .map(|p| RasterPlane {
                width: p.width,
                height: p.height,
                stride: p.stride,
                data: p.data,
                h_div: max_h / p.h_samp,
                v_div: max_v / p.v_samp,
            })
            .collect(),
        truncated: false,
    }
}

/// A test card: gradients, a sharp-edged grid and diagonal texture, so every frequency of
/// both directions carries energy.
fn card(w: usize, h: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let grid = if (x / 13 + y / 7) % 2 == 0 { 60 } else { 0 };
            let r = (x * 255 / w) as u8 / 2 + grid;
            let g = (y * 255 / h) as u8 / 2 + ((x * 7 + y * 3) % 50) as u8;
            let b = (((x ^ y) & 31) * 4) as u8 + if x % 11 < 2 { 100 } else { 0 };
            v.extend_from_slice(&[r, g, b]);
        }
    }
    v
}

#[test]
fn hardware_matches_software() {
    if specular_hwdec::capabilities().is_none_or(|c| c.jpeg.is_none()) {
        assert!(
            std::env::var_os("SPECULAR_REQUIRE_GPU").is_none(),
            "SPECULAR_REQUIRE_GPU is set but there is no hardware JPEG decoder"
        );
        eprintln!("skipped: no hardware JPEG decoder");
        return;
    }
    let mut cases: Vec<(String, Vec<u8>)> = Vec::new();
    for (w, h) in [(640, 480), (637, 479)] {
        let rgb = card(w, h);
        let grey: Vec<u8> = rgb.chunks(3).map(|p| p[1]).collect();
        for (tag, samples, sub) in [
            ("444", Samples::Rgb(&rgb), Subsampling::S444),
            ("420", Samples::Rgb(&rgb), Subsampling::S420),
            ("grey", Samples::Gray(&grey), Subsampling::S444),
        ] {
            let o = EncodeOptions {
                quality: 75,
                subsampling: sub,
                ..Default::default()
            };
            let j = encode(w as u32, h as u32, samples, &o).unwrap();
            cases.push((
                format!("{w}x{h} {tag} asymmetric"),
                patch_dqt(&j, asymmetric, false),
            ));
            cases.push((format!("{w}x{h} {tag}"), j));
        }
    }
    let root = corpus();
    for f in [
        "conformance/libjpeg-turbo/testimages/testorig.jpg",
        "conformance/sdl_image/test/sample.jpg",
    ] {
        if let Ok(d) = std::fs::read(root.join(f)) {
            cases.push((format!("{f} asymmetric"), patch_dqt(&d, asymmetric, false)));
            cases.push((f.to_string(), d));
        }
    }
    // The user's photos, read in place (never copied); named by number only.
    if let Ok(rd) = std::fs::read_dir(root.join("real")) {
        let mut v: Vec<_> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg")
                })
            })
            .collect();
        v.sort();
        for (k, p) in v.iter().take(4).enumerate() {
            cases.push((format!("photo {k}"), std::fs::read(p).unwrap()));
        }
    }

    let (mut on_hw, mut bad) = (0, Vec::new());
    for (name, data) in &cases {
        let hw = match specular_hwdec::decode_jpeg_unchecked(data) {
            Ok(r) => r,
            Err(e) => {
                // The reason may quote a photo's size: kept to ourselves.
                let why = if name.starts_with("photo") {
                    String::new()
                } else {
                    format!(" ({e})")
                };
                eprintln!("{name}: not on the hardware{why}");
                continue;
            }
        };
        on_hw += 1;
        let p = specular_hwdec::raster_psnr(&hw, &software(data));
        eprintln!("{name}: {p:.1?} dB");
        if p.is_none_or(|p| p < specular_hwdec::MIN_PSNR_DB) {
            bad.push(format!("{name}: {p:?} dB"));
        }
    }
    assert!(bad.is_empty(), "hardware differs from software: {bad:#?}");
    assert!(on_hw >= 6, "only {on_hw} images went to the hardware");
}

/// The asymmetric tables make an order mistake visible: the same file with its tables
/// written in raster order decodes (in software) far below the floor.
#[test]
fn asymmetric_tables_expose_order_mistakes() {
    let (w, h) = (256, 192);
    let rgb = card(w, h);
    let o = EncodeOptions {
        quality: 75,
        subsampling: Subsampling::S420,
        ..Default::default()
    };
    let j = encode(w as u32, h as u32, Samples::Rgb(&rgb), &o).unwrap();
    let right = software(&patch_dqt(&j, asymmetric, false));
    let wrong = software(&patch_dqt(&j, asymmetric, true));
    let p = specular_hwdec::raster_psnr(&right, &wrong).unwrap();
    assert!(p < specular_hwdec::MIN_PSNR_DB - 10.0, "{p} dB");
    // And the rewritten tables do change the picture.
    let p = specular_hwdec::raster_psnr(&right, &software(&j)).unwrap();
    assert!(p < specular_hwdec::MIN_PSNR_DB - 10.0, "{p} dB");
}
