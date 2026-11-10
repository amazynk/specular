//! CMYK (a black extra channel) on every decode path, on libjxl's conformance case
//! `cmyk_layers` (fetched with `tools/corpus/fetch-conformance.sh src_jxl large_jxl_refs`;
//! `SPECULAR_CONFORMANCE_DIR` overrides the corpus root, as in `conformance.rs`, which checks
//! the inks against libjxl's reference). Here the 8-bit paths are checked against those inks:
//! full size exactly (the naive formula, or the hook's converter), reduced sizes as the
//! alpha-weighted box average of the full-size output.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_jxl::{CmykRowConverter, Decoded, Error, JxlFile, Layout, set_cmyk_hook};

fn case() -> Option<Vec<u8>> {
    let root = std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance")
        });
    std::fs::read(root.join("jxl-conformance/testcases/cmyk_layers/input.jxl")).ok()
}

/// The naive formula, as the crate (and `specular-jpeg`) applies it without a hook.
fn naive(ink: &[u8]) -> [u8; 3] {
    let k = 255 - u32::from(ink[3]);
    [0, 1, 2].map(|c| (((255 - u32::from(ink[c])) * k + 127) / 255) as u8)
}

/// A stand-in for a colour-managed converter: anything distinguishable from the formula.
fn test_hook(_icc: Option<&[u8]>) -> Option<CmykRowConverter> {
    Some(Arc::new(|ink: &[u8], rgba: &mut [u8]| {
        for (i, o) in ink.chunks_exact(4).zip(rgba.chunks_exact_mut(4)) {
            o.copy_from_slice(&[i[3], i[0], i[1], 255]);
        }
    }))
}

/// The alpha-weighted box average of RGBA `full` (`w` × `h`) at `1/s`.
fn reduce(full: &[u8], w: usize, h: usize, s: usize) -> Vec<u8> {
    let (ow, oh) = (w.div_ceil(s), h.div_ceil(s));
    let mut out = Vec::with_capacity(ow * oh * 4);
    for oy in 0..oh {
        for ox in 0..ow {
            let (mut c, mut a, mut n) = ([0f64; 3], 0f64, 0f64);
            for y in oy * s..((oy + 1) * s).min(h) {
                for x in ox * s..((ox + 1) * s).min(w) {
                    let p = &full[(y * w + x) * 4..(y * w + x) * 4 + 4];
                    let al = f64::from(p[3]) / 255.0;
                    for k in 0..3 {
                        c[k] += f64::from(p[k]) * al;
                    }
                    a += al;
                    n += 1.0;
                }
            }
            for v in c {
                out.push(if a > 0.0 { (v / a).round() as u8 } else { 0 });
            }
            out.push((a / n * 255.0).round() as u8);
        }
    }
    out
}

/// (mismatching samples, largest difference).
fn diff(a: &[u8], b: &[u8]) -> (usize, u8) {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).fold((0, 0), |(n, m), (&x, &y)| {
        let d = x.abs_diff(y);
        (n + usize::from(d > 0), m.max(d))
    })
}

fn rgba(d: &Decoded) -> Vec<u8> {
    match d.layout {
        Layout::Rgba => d.data.clone(),
        Layout::Rgb => d
            .data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        l => panic!("layout {l:?}"),
    }
}

#[test]
fn cmyk_every_path() {
    let Some(data) = case() else {
        eprintln!(
            "SKIPPED: cmyk_layers not found; fetch it with \
             tools/corpus/fetch-conformance.sh src_jxl large_jxl_refs"
        );
        return;
    };
    let file = JxlFile::open(&data[..]).unwrap();
    let info = file.info().clone();
    assert!(info.cmyk && info.has_alpha);
    assert!(info.output_icc.is_none(), "CMYK renders to sRGB");
    let (w, h) = (info.width as usize, info.height as usize);
    let inks = file.decode_cmyk(true).unwrap();
    let alpha = inks.alpha.clone().unwrap();
    let expect: Vec<u8> = inks
        .ink
        .chunks_exact(4)
        .zip(&alpha)
        .flat_map(|(i, &a)| {
            let [r, g, b] = naive(i);
            [r, g, b, a]
        })
        .collect();

    // Full size, both thread modes: exactly the formula over the inks.
    for parallel in [false, true] {
        let d = file.decode(1, parallel).unwrap();
        assert_eq!(
            (d.width, d.height, d.layout),
            (w as u32, h as u32, Layout::Rgba)
        );
        let (n, max) = diff(&d.data, &expect);
        eprintln!("decode 1/1 parallel={parallel}: {n} mismatches, max {max}");
        assert_eq!(n, 0);
    }
    // Reduced: the alpha-weighted box average, to rounding.
    for s in [2usize, 3, 8] {
        let d = file.decode(s as u32, true).unwrap();
        let (n, max) = diff(&d.data, &reduce(&expect, w, h, s));
        eprintln!("decode 1/{s}: {n} samples off, max {max}");
        assert!(max <= 1, "1/{s}: max {max}");
    }
    // The DC preview falls back to the 1/8 decode; frames render the same first frame.
    let d8 = file.decode(8, true).unwrap();
    let dc = file.decode_dc(true).unwrap();
    assert_eq!((dc.scale, &dc.data), (8, &d8.data));
    let frames: Vec<Decoded> = file.frames(1, true).unwrap().map(|f| f.unwrap()).collect();
    assert!(!frames.is_empty());
    assert_eq!(rgba(&frames[0]), expect);

    // The wide and sample paths refuse CMYK by name.
    for r in [file.decode_hdr(1, true), file.decode_linear(1, true)] {
        assert!(matches!(r, Err(Error::Unsupported(_))));
    }
    assert!(matches!(file.decode_samples(), Err(Error::Unsupported(_))));

    // A registered hook replaces the formula.
    set_cmyk_hook(Some(test_hook));
    let d = file.decode(1, true).unwrap();
    set_cmyk_hook(None);
    let hooked: Vec<u8> = inks
        .ink
        .chunks_exact(4)
        .zip(&alpha)
        .flat_map(|(i, &a)| [i[3], i[0], i[1], a])
        .collect();
    assert_eq!(diff(&d.data, &hooked).0, 0);
}

#[test]
fn decode_cmyk_refuses_rgb() {
    let data = std::fs::read(format!(
        "{}/tests/data/vardct.jxl",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let file = JxlFile::open(&data[..]).unwrap();
    assert!(!file.info().cmyk);
    assert!(matches!(
        file.decode_cmyk(false),
        Err(Error::Unsupported(_))
    ));
}
