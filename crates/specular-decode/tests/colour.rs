//! Colour-managed CMYK decoding (src/colour.rs): JPEG (Adobe CMYK and YCCK), TIFF and PSD
//! go through specular-color's CMYK transform. The JPEG is compared with the RGB version of the
//! same picture in Pillow's test images.
//!
//! One test function: the colour policy is process-wide.

use specular_color::math::{bt709_to_pcs, delta_e2000, mul3, srgb_to_linear, xyz_to_lab};
use specular_color::{Policy, set_policy};
use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeRequest, Registry};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn corpus(rel: &str) -> Option<PathBuf> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/corpus/conformance")
        .join(rel);
    p.is_file().then_some(p)
}

fn jpeg_rgba(p: &Path) -> (u32, u32, Vec<u8>) {
    let data = std::fs::read(p).unwrap();
    let d = specular_jpeg::decode(&data, &Default::default()).unwrap();
    (d.width, d.height, specular_jpeg::planes_to_rgba(&d))
}

/// Full-size RGBA8 through the registry.
fn registry_rgba(reg: &Registry, p: &Path) -> Vec<u8> {
    let s = reg.open(Arc::new(Source::open(p).unwrap())).unwrap();
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!(r.layout, PixelLayout::Rgba, "{}", p.display());
    let pl = &r.planes[0];
    let mut out = Vec::with_capacity(r.width as usize * r.height as usize * 4);
    for y in 0..r.height as usize {
        out.extend_from_slice(&pl.data[y * pl.stride..y * pl.stride + r.width as usize * 4]);
    }
    out
}

fn lab(px: &[u8]) -> [f32; 3] {
    let m = bt709_to_pcs().to_f32();
    let lin = [0, 1, 2].map(|i| srgb_to_linear(px[i] as f32 / 255.0));
    xyz_to_lab(mul3(&m, lin))
}

/// Mean and 95th-percentile ΔE2000.
fn de(a: &[u8], b: &[u8]) -> (f32, f32) {
    let mut v: Vec<f32> = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .map(|(a, b)| delta_e2000(lab(a), lab(b)))
        .collect();
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    v.sort_by(f32::total_cmp);
    (mean, v[v.len() * 95 / 100])
}

#[test]
fn cmyk_is_colour_managed() {
    let reg = Registry::new(); // installs the CMYK hooks
    let (Some(cmyk), Some(rgb)) = (
        corpus("pillow/Tests/images/pil_sample_cmyk.jpg"),
        corpus("pillow/Tests/images/pil_sample_rgb.jpg"),
    ) else {
        eprintln!("conformance corpus missing; skipped");
        return;
    };
    let (w, h, reference) = jpeg_rgba(&rgb);
    let t0 = std::time::Instant::now();
    let (w2, h2, managed) = jpeg_rgba(&cmyk);
    let managed_ms = t0.elapsed().as_secs_f64() * 1e3;
    assert_eq!((w, h), (w2, h2));
    set_policy(Policy {
        enabled: false,
        ..Policy::default()
    });
    let t0 = std::time::Instant::now();
    let (_, _, naive) = jpeg_rgba(&cmyk);
    let naive_ms = t0.elapsed().as_secs_f64() * 1e3;
    let tiff_naive =
        corpus("pillow/Tests/images/tiff_strip_cmyk_jpeg.tif").map(|p| registry_rgba(&reg, &p));
    let psd_naive =
        corpus("zune-image/test-images/psd/cmyk_image.psd").map(|p| registry_rgba(&reg, &p));
    set_policy(Policy::default());
    let tiff =
        corpus("pillow/Tests/images/tiff_strip_cmyk_jpeg.tif").map(|p| registry_rgba(&reg, &p));
    let psd = corpus("zune-image/test-images/psd/cmyk_image.psd").map(|p| registry_rgba(&reg, &p));
    let ycck = corpus("jpeg-decoder/tests/reftest/images/ycck.jpg").map(|p| jpeg_rgba(&p).2);

    let (m_mean, m_p95) = de(&managed, &reference);
    let (n_mean, n_p95) = de(&naive, &reference);
    eprintln!(
        "pil_sample_cmyk.jpg {w}×{h} vs Pillow's RGB: managed ΔE2000 mean {m_mean:.2} p95 {m_p95:.2} \
         ({managed_ms:.1} ms incl. converter), naive mean {n_mean:.2} p95 {n_p95:.2} ({naive_ms:.1} ms)"
    );
    assert!(
        m_mean < n_mean,
        "managed CMYK is closer to the reference than naive"
    );
    assert!(m_mean < 4.0, "managed mean ΔE {m_mean}");

    for (name, a, b) in [("TIFF", tiff, tiff_naive), ("PSD", psd, psd_naive)] {
        if let (Some(a), Some(b)) = (a, b) {
            let (mean, _) = de(&a, &b);
            eprintln!("{name} CMYK: managed vs naive ΔE2000 mean {mean:.2}");
            assert!(mean > 0.5, "{name} goes through the CMYK transform");
        }
    }
    if let Some(y) = ycck {
        // Mostly not black or white: the YCCK → ink mapping is the right way round.
        let mean = y.chunks_exact(4).map(|p| p[1] as f32).sum::<f32>() / (y.len() / 4) as f32;
        eprintln!("ycck.jpg: mean G {mean:.0}");
        assert!((20.0..235.0).contains(&mean));
    }
}
