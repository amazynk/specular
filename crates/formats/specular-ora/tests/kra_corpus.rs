//! Krita layer pages against real files (not in git): every paint layer of every `.kra`
//! under `tools/corpus/large/kra` (or `$SPECULAR_KRA_CORPUS`) decodes cleanly, and a
//! single opaque normal layer at full opacity matches the file's merged image; the composite
//! of the layer tree is compared with the merged image (within 2 levels when nothing is left out
//! or approximated). Run with
//! `cargo test -p specular-ora --test kra_corpus -- --ignored --nocapture`.

use specular_ora::{Document, KraOutput};
use std::path::PathBuf;
use std::time::Instant;

fn files() -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(corpus()) else {
        eprintln!("no corpus at {}", corpus().display());
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "kra"))
        .collect();
    files.sort();
    files
}

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_KRA_CORPUS").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/large/kra"),
        PathBuf::from,
    )
}

#[test]
#[ignore = "needs the Krita corpus"]
fn corpus_layers_decode() {
    let mut layers = 0;
    for f in files() {
        let data = std::fs::read(&f).unwrap();
        let doc = Document::open(&data).unwrap();
        for l in &doc.kra_layers {
            let t = Instant::now();
            let img = doc.kra_layer_page(&data, l, &|| false).unwrap();
            eprintln!(
                "{} / {:?} {:?} at ({}, {}): {:?}, {:.1} ms{}",
                f.file_name().unwrap().to_string_lossy(),
                l.name,
                l.pixel,
                l.x,
                l.y,
                img.format,
                t.elapsed().as_secs_f64() * 1e3,
                if img.truncated { ", TRUNCATED" } else { "" }
            );
            assert!(!img.truncated);
            layers += 1;
        }
        // One layer and a merged image: they should agree.
        if let ([l], Some(m)) = (&doc.kra_layers[..], doc.merged()) {
            let png = doc.read(&data, m).unwrap();
            let merged = specular_png::decode(
                &png,
                &specular_png::DecodeOptions {
                    output: specular_png::Output::Rgba8,
                    ..Default::default()
                },
            )
            .unwrap();
            let img = doc.kra_layer_page(&data, l, &|| false).unwrap();
            if img.format == KraOutput::Rgba8 && l.opacity == 1.0 {
                // Krita may store a tile-aligned merged image larger than the canvas.
                let (w, h) = (img.width as usize, img.height as usize);
                let mw = merged.width as usize;
                let mut max = 0;
                let mut first = None;
                for y in 0..h.min(merged.height as usize) {
                    for x in 0..w.min(mw) {
                        let m = &merged.planes[0].data[(y * mw + x) * 4..][..4];
                        let p = &img.data[(y * w + x) * 4..][..4];
                        let d = m.iter().zip(p).map(|(&a, &b)| a.abs_diff(b)).max().unwrap();
                        if d > 1 && first.is_none() {
                            first = Some((x, y, m.to_vec(), p.to_vec()));
                        }
                        max = max.max(d);
                    }
                }
                eprintln!("  single layer vs merged image: max difference {max}");
                if let Some(f) = first {
                    eprintln!("  first at {f:?} (merged, layer)");
                }
                assert!(max <= 1);
            }
        }
    }
    eprintln!("{layers} layers");
}

#[test]
#[ignore = "needs the Krita corpus"]
fn corpus_composite_matches_merged() {
    for f in files() {
        let data = std::fs::read(&f).unwrap();
        let doc = Document::open(&data).unwrap();
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        if !doc.kra_can_composite() {
            eprintln!("{name}: nothing to composite");
            continue;
        }
        let t = Instant::now();
        let img = doc.kra_composite(&data, &|| false).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let notes = doc.kra_notes(&data, None, None);
        eprintln!(
            "{name}: {}×{} {:?} in {ms:.1} ms{}; notes {notes:?}",
            img.width,
            img.height,
            img.format,
            if img.truncated { ", TRUNCATED" } else { "" }
        );
        assert!(!img.truncated);
        let Some(m) = doc.merged() else { continue };
        if img.format == KraOutput::Rgba16F || img.format == KraOutput::GrayAlpha16 {
            eprintln!("  merged image not compared ({:?})", img.format);
            continue;
        }
        let wide = img.format == KraOutput::Rgba16;
        let png = doc.read(&data, m).unwrap();
        // Krita writes the merged image of a deep document in its colour space only when
        // that suits PNG; an 8-bit one has been converted (to sRGB).
        if wide && png.get(24) != Some(&16) {
            eprintln!("  merged image converted to 8-bit, not compared");
            continue;
        }
        let merged = specular_png::decode(
            &png,
            &specular_png::DecodeOptions {
                output: if wide {
                    specular_png::Output::Rgba16
                } else {
                    specular_png::Output::Rgba8
                },
                ..Default::default()
            },
        )
        .unwrap();
        // Both as straight 8-bit RGBA.
        let px = |d: &[u8], i: usize| -> [u8; 4] {
            if wide {
                std::array::from_fn(|c| {
                    let b = &d[(i * 4 + c) * 2..][..2];
                    (u16::from_ne_bytes([b[0], b[1]]) >> 8) as u8
                })
            } else {
                d[i * 4..][..4].try_into().unwrap()
            }
        };
        let (w, h) = (img.width as usize, img.height as usize);
        let mw = merged.width as usize;
        let (mut max, mut off, mut n) = (0u8, 0usize, 0usize);
        let mut first = None;
        for y in 0..h.min(merged.height as usize) {
            for x in 0..w.min(mw) {
                let a = px(&img.data, y * w + x);
                let b = px(&merged.planes[0].data, y * mw + x);
                // The colour of transparent pixels is not compared.
                let d = if a[3] == 0 && b[3] == 0 {
                    0
                } else {
                    a.iter().zip(&b).map(|(p, q)| p.abs_diff(*q)).max().unwrap()
                };
                if d > 2 && first.is_none() {
                    first = Some((x, y, a, b));
                }
                max = max.max(d);
                off += usize::from(d > 2);
                n += 1;
            }
        }
        eprintln!(
            "  composite vs merged image: max difference {max}, {:.3}% of pixels off by more than 2",
            off as f64 * 100.0 / n as f64
        );
        if let Some(f) = first {
            eprintln!("  first at {f:?} (composite, merged)");
        }
        if notes.is_empty() {
            assert!(max <= 2, "{name}");
        }
    }
}
