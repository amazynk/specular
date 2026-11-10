//! The OCIO display/view in the image shader against `specular_ocio`'s CPU processor: a
//! scene-linear ramp (greys over 16 stops and colours, half floats) drawn through the baked
//! table, read back and re-encoded, compared with the processor's display values. GPU
//! tests skip without an adapter.

use specular_color::math::linear_to_srgb;
use specular_core::{PixelLayout, Primaries};

use super::hdr_tests::{draw_with, gpu, wide_raster};
use super::*;

/// The ACES 1.0 SDR view and the plain sRGB view of a small config built from the
/// built-in transforms (the ACES configs' own definitions).
const CONFIG: &str = r#"ocio_profile_version: 2.1
roles:
  scene_linear: ACEScg
displays:
  sRGB - Display:
    - !<View> {name: ACES 1.0 - SDR Video, view_transform: ACES 1.0 - SDR Video, display_colorspace: sRGB - Display}
    - !<View> {name: Un-tone-mapped, view_transform: Un-tone-mapped, display_colorspace: sRGB - Display}
view_transforms:
  - !<ViewTransform>
    name: ACES 1.0 - SDR Video
    from_scene_reference: !<BuiltinTransform> {style: ACES-OUTPUT - ACES2065-1_to_CIE-XYZ-D65 - SDR-VIDEO_1.0}
  - !<ViewTransform>
    name: Un-tone-mapped
    from_scene_reference: !<BuiltinTransform> {style: UTILITY - ACES-AP0_to_CIE-XYZ-D65_BFD}
display_colorspaces:
  - !<ColorSpace>
    name: sRGB - Display
    from_display_reference: !<BuiltinTransform> {style: DISPLAY - CIE-XYZ-D65_to_sRGB}
colorspaces:
  - !<ColorSpace>
    name: ACES2065-1
  - !<ColorSpace>
    name: ACEScg
    to_scene_reference: !<BuiltinTransform> {style: ACEScg_to_ACES2065-1}
  - !<ColorSpace>
    name: Linear Rec.709 (sRGB)
    from_scene_reference: !<GroupTransform>
      children:
        - !<BuiltinTransform> {style: UTILITY - ACES-AP0_to_CIE-XYZ-D65_BFD}
        - !<MatrixTransform> {matrix: [3.2409699419, -1.5373831776, -0.4986107603, 0, -0.9692436363, 1.8759675015, 0.0415550574, 0, 0.0556300797, -0.2039769589, 1.0569715142, 0, 0, 0, 0, 1]}
"#;

#[test]
fn ocio_views_match_the_cpu_processor() {
    let Some(gpu) = gpu() else { return };
    let cfg = specular_ocio::Config::parse(CONFIG, std::path::Path::new(".")).unwrap();
    // The ramp: greys from 2^-10 to 2^4, then colours.
    let mut px: Vec<[f32; 3]> = (0..57)
        .map(|i| [(i as f32 * 0.25 - 10.0).exp2(); 3])
        .collect();
    for c in [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.8, 0.4, 0.1],
        [0.05, 0.3, 0.9],
        [2.0, 1.5, 0.2],
        [0.18, 0.18, 0.18],
        [6.0, 0.3, 3.0],
    ] {
        px.push(c);
    }
    let w = px.len() as u32;
    let r = wide_raster(w, 1, PixelLayout::Rgba16F(Primaries::Bt709), |x, _| {
        let p = px[x as usize];
        [
            hdr::f32_to_f16(p[0]),
            hdr::f32_to_f16(p[1]),
            hdr::f32_to_f16(p[2]),
            hdr::f32_to_f16(1.0),
        ]
    });
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let mut img = renderer.create_image(&gpu, 21, w, 1, None);
    renderer.upload_overview(&gpu, &mut img, &r);
    for (k, view) in ["ACES 1.0 - SDR Video", "Un-tone-mapped"]
        .iter()
        .enumerate()
    {
        let p = specular_ocio::Processor::display_view(
            &cfg,
            "Linear Rec.709 (sRGB)",
            "sRGB - Display",
            view,
        )
        .unwrap();
        let b = specular_ocio::bake(&p, 65, -12.0, 4.0);
        let lut = OcioLut {
            id: k as u64 + 1,
            size: b.size as u32,
            log2_lo: b.log2_lo,
            log2_hi: b.log2_hi,
        };
        renderer.upload_ocio(&gpu, lut.id, lut.size, &b.texels);
        let out = draw_with(
            &gpu,
            &mut renderer,
            &img,
            RenderOptions {
                ocio: Some(lut),
                ..Default::default()
            },
        );
        let (mut worst, mut worst_baked) = (0f32, 0f32);
        for (x, c) in px.iter().enumerate() {
            // The half float the GPU saw.
            let h = c.map(|v| hdr::f16_to_f32(hdr::f32_to_f16(v)));
            let want = p.apply(h.map(f64::from)).map(|v| v.clamp(0.0, 1.0) as f32);
            let baked = b.lookup(h);
            for i in 0..3 {
                let got = linear_to_srgb(out[x][i]);
                worst = worst.max((got - want[i]).abs());
                worst_baked = worst_baked.max((got - baked[i].clamp(0.0, 1.0)).abs());
            }
        }
        eprintln!(
            "ocio GPU {view}: max |Δ| {worst:.2e} against the CPU processor, {worst_baked:.2e} against the CPU bake lookup"
        );
        // Half-float targets keep about 11 bits; the table about 8 at saturated edges.
        assert!(worst < 1.5e-2, "{view}: {worst}");
        assert!(worst_baked < 3e-3, "{view}: {worst_baked}");
    }
}
