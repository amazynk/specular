//! Colour management on the GPU: images tagged Display P3 and Adobe RGB render to the linear
//! BT.709 values of a reference CMS (moxcms, test-only), through the source transform
//! (curves + matrix) and through a wide-gamut storage space. GPU tests skip without an adapter.

use specular_color::math::{bt709_to_pcs, delta_e2000, mul3, xyz_to_lab};
use specular_color::{Display, Intent};
use specular_core::{PixelLayout, Raster, RasterPlane};
use specular_gfx::Gpu;

use super::hdr_tests::{draw, gpu};
use super::*;

fn rgb8(w: u32, px: &[[u8; 3]]) -> Raster {
    let planes = (0..3)
        .map(|c| RasterPlane {
            width: w,
            height: 1,
            stride: w as usize,
            data: px.iter().map(|p| p[c]).collect(),
            h_div: 1,
            v_div: 1,
        })
        .collect();
    Raster {
        width: w,
        height: 1,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgb,
        planes,
        truncated: false,
    }
}

/// Test colours: greys, primaries and secondaries at two levels, skin and sky tones, and a
/// grid through the cube.
fn colours() -> Vec<[u8; 3]> {
    let mut v = vec![
        [0, 0, 0],
        [255, 255, 255],
        [128, 128, 128],
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [255, 255, 0],
        [0, 255, 255],
        [255, 0, 255],
        [180, 40, 40],
        [40, 160, 60],
        [50, 70, 190],
        [224, 172, 140],
        [120, 170, 230],
    ];
    for r in [16u8, 96, 176, 240] {
        for g in [32u8, 128, 224] {
            for b in [8u8, 112, 200] {
                v.push([r, g, b]);
            }
        }
    }
    v
}

/// Reference: the profile's 8-bit values → `dst` (sRGB or Display P3, clipped to its gamut
/// as storage clips) → linear light with `dst`'s primaries; relative colorimetric, through
/// moxcms in floating point.
fn reference(
    profile: &moxcms::ColorProfile,
    dst: &moxcms::ColorProfile,
    px: &[[u8; 3]],
) -> Vec<[f32; 3]> {
    let t = profile
        .create_transform_f32(
            moxcms::Layout::Rgb,
            dst,
            moxcms::Layout::Rgb,
            moxcms::TransformOptions {
                rendering_intent: moxcms::RenderingIntent::RelativeColorimetric,
                ..Default::default()
            },
        )
        .unwrap();
    let src: Vec<f32> = px
        .iter()
        .flat_map(|p| p.map(|v| v as f32 / 255.0))
        .collect();
    let mut dst = vec![0f32; src.len()];
    t.transform(&src, &mut dst).unwrap();
    let lin = |v: f32| specular_color::math::srgb_to_linear(v.clamp(0.0, 1.0));
    dst.chunks_exact(3)
        .map(|c| [lin(c[0]), lin(c[1]), lin(c[2])])
        .collect()
}

fn lab(rgb: [f32; 3]) -> [f32; 3] {
    xyz_to_lab(mul3(&bt709_to_pcs().to_f32(), rgb))
}

/// (mean, max) ΔE2000 between rendered and reference linear BT.709, both clipped to `clip`.
fn compare(got: &[[f32; 4]], want: &[[f32; 3]], clip: (f32, f32)) -> (f32, f32) {
    let c = |v: f32| v.clamp(clip.0, clip.1);
    let d: Vec<f32> = got
        .iter()
        .zip(want)
        .map(|(g, w)| delta_e2000(lab([c(g[0]), c(g[1]), c(g[2])]), lab(w.map(c))))
        .collect();
    (
        d.iter().sum::<f32>() / d.len() as f32,
        d.iter().copied().fold(0.0, f32::max),
    )
}

fn render(
    gpu: &Gpu,
    renderer: &mut ImageRenderer,
    icc: &[u8],
    px: &[[u8; 3]],
) -> (Vec<[f32; 4]>, f64) {
    let t0 = std::time::Instant::now();
    let lut = crate::icc::build_lut(icc).expect("wide-gamut profile needs a transform");
    let lut_ms = t0.elapsed().as_secs_f64() * 1e3;
    assert!(
        matches!(lut, SourceLut::Curves { .. }),
        "matrix/TRC profile"
    );
    let w = px.len() as u32;
    let mut img = renderer.create_image(gpu, 3, w, 1, Some(&lut));
    renderer.upload_overview(gpu, &mut img, &rgb8(w, px));
    let out = draw(gpu, renderer, &img);
    renderer.release_image(3);
    (out, lut_ms)
}

#[test]
fn tagged_images_render_to_reference_values() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    renderer.set_display_peak(1.0);
    let px = colours();
    for (name, profile) in [
        ("Display P3", moxcms::ColorProfile::new_display_p3()),
        ("Adobe RGB", moxcms::ColorProfile::new_adobe_rgb()),
    ] {
        let icc = profile.encode().unwrap();
        let want = reference(&profile, &moxcms::ColorProfile::new_srgb(), &px);

        // sRGB storage: out-of-sRGB colours clip, as the 8-bit overview cannot hold them.
        renderer.set_storage(colour::IDENTITY, colour::IDENTITY);
        let (got, lut_ms) = render(&gpu, &mut renderer, &icc, &px);
        let (mean, max) = compare(&got, &want, (0.0, 1.0));
        eprintln!(
            "{name} → sRGB storage: ΔE2000 mean {mean:.3} max {max:.3} (source transform built in {lut_ms:.2} ms)"
        );
        assert!(mean < 0.4 && max < 1.5, "{name}: mean {mean} max {max}");

        // Display P3 storage (a P3 display): P3 colours survive storage and come back as
        // linear BT.709 outside 0..1.
        let p3 = Display::new(
            &moxcms::ColorProfile::new_display_p3().encode().unwrap(),
            Intent::Relative,
            true,
        );
        assert!(!p3.storage_is_bt709());
        renderer.set_storage(p3.to_storage, p3.from_storage);
        let (got, _) = render(&gpu, &mut renderer, &icc, &px);
        // Reference: clipped in Display P3 (the storage), then back to linear BT.709.
        let want_bt: Vec<[f32; 3]> =
            reference(&profile, &moxcms::ColorProfile::new_display_p3(), &px)
                .into_iter()
                .map(|c| mul3(&p3.from_storage, c))
                .collect();
        let got_bt = got;
        let (mean, max) = compare(&got_bt, &want_bt, (-1.0, 2.0));
        eprintln!("{name} → P3 storage: ΔE2000 mean {mean:.3} max {max:.3}");
        assert!(
            mean < 0.4 && max < 1.5,
            "{name} P3 storage: mean {mean} max {max}"
        );
        if name == "Display P3" {
            // Pure P3 red is outside sRGB: green goes negative instead of clipping.
            let red = got_bt[3];
            assert!(red[1] < -0.02 && red[0] > 1.0, "P3 red kept: {red:?}");
        }
    }
}
