//! Video YCbCr ([`PixelLayout::Yuv`]) converted on the GPU against the CPU reference,
//! `specular_core::yuv::to_rgb`: 8-bit BT.601 limited range with co-sited 4:2:0 chroma at an
//! odd size, 10-bit BT.709 full range, identity ("RGB" video), SMPTE 240M, FCC and YCgCo,
//! vertically co-sited chroma, and 10-bit BT.2020 PQ and HLG (the HDR path); 4:4:0 and 4:1:1
//! chroma and bottom siting (fidelity round 8). GPU tests skip without an adapter.

use specular_core::hdr::{self, HdrEncoding, Primaries, Transfer};

use specular_core::{PixelLayout, Raster, RasterPlane, YuvEncoding, YuvMatrix, yuv};

use super::hdr_tests::{draw, gpu};
use super::*;

/// A frame with smooth luma, chroma ramps and a hard chroma edge (siting matters there),
/// 4:2:0.
fn frame(w: u32, h: u32, e: YuvEncoding) -> Raster {
    frame_div(w, h, e, (2, 2))
}

/// [`frame`] with chroma subsampled `div` (across, down); the edge runs both ways.
fn frame_div(w: u32, h: u32, e: YuvEncoding, div: (u8, u8)) -> Raster {
    let wide = e.bits > 8;
    let max = (1u32 << e.bits) - 1;
    let plane = |pw: u32, ph: u32, (hd, vd): (u8, u8), f: &dyn Fn(u32, u32) -> u32| {
        let mut data = Vec::new();
        for y in 0..ph {
            for x in 0..pw {
                let v = f(x, y).min(max);
                if wide {
                    data.extend_from_slice(&(v as u16).to_ne_bytes());
                } else {
                    data.push(v as u8);
                }
            }
        }
        RasterPlane {
            width: pw,
            height: ph,
            stride: pw as usize * e.sample_bytes(),
            data,
            h_div: hd,
            v_div: vd,
        }
    };
    let (cw, ch) = (w.div_ceil(u32::from(div.0)), h.div_ceil(u32::from(div.1)));
    let y = plane(w, h, (1, 1), &|x, y| {
        (x * 7 + y * 3) * max / (w * 7 + h * 3)
    });
    let cb = plane(cw, ch, div, &|x, y| {
        let edge = (x < cw / 2) != (div == (1, 2) && y < ch / 2);
        if edge { max / 5 } else { max * 4 / 5 }
    });
    let cr = plane(cw, ch, div, &|x, y| (x + y) * max / (cw + ch));
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Yuv(e),
        planes: vec![y, cb, cr],
        truncated: false,
    }
}

/// Linear-light reference of each pixel from the CPU conversion.
fn reference(r: &Raster, e: YuvEncoding) -> Vec<[f32; 3]> {
    let rgb = yuv::to_rgb(r).expect("cpu conversion");
    let p = &rgb.planes[0];
    let mut out = Vec::new();
    for y in 0..rgb.height as usize {
        for x in 0..rgb.width as usize {
            let o = y * p.stride;
            out.push(match rgb.layout {
                PixelLayout::Rgba => {
                    let px = &p.data[o + 4 * x..o + 4 * x + 4];
                    std::array::from_fn(|c| hdr::srgb_to_linear(px[c] as f32 / 255.0))
                }
                _ => {
                    let px = &p.data[o + 8 * x..o + 8 * x + 8];
                    let code = |c: usize| u16::from_ne_bytes([px[2 * c], px[2 * c + 1]]);
                    hdr::decode16(e.colour, [code(0), code(1), code(2)])
                }
            });
        }
    }
    out
}

#[test]
fn gpu_matches_the_cpu_reference() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    renderer.set_display_peak(64.0);
    let cases = [
        (8, YuvMatrix::Bt601, false, true, HdrEncoding::SRGB),
        (10, YuvMatrix::Bt709, true, false, HdrEncoding::SRGB),
        // Fidelity round 7: the matrices beyond BT.601/709/2020 and vertical siting.
        (8, YuvMatrix::Identity, true, false, HdrEncoding::SRGB),
        (10, YuvMatrix::Identity, false, false, HdrEncoding::SRGB),
        (8, YuvMatrix::Smpte240, false, true, HdrEncoding::SRGB),
        (8, YuvMatrix::Fcc, false, true, HdrEncoding::SRGB),
        (10, YuvMatrix::YCgCo, true, false, HdrEncoding::SRGB),
        (
            10,
            YuvMatrix::Bt2020,
            false,
            true,
            HdrEncoding {
                transfer: Transfer::Pq,
                primaries: Primaries::Bt2020,
            },
        ),
        (
            10,
            YuvMatrix::Bt2020,
            false,
            true,
            HdrEncoding {
                transfer: Transfer::Hlg,
                primaries: Primaries::Bt2020,
            },
        ),
    ];
    for (i, (bits, matrix, full_range, cosited_x, colour)) in cases.into_iter().enumerate() {
        let e = YuvEncoding {
            matrix,
            full_range,
            bits,
            cosited_x,
            // Every other case with chroma rows on the even luma rows (AV1 colocated).
            cosited_y: i % 2 == 1,
            bottom_y: false,
            right_x: false,
            alpha: false,
            colour,
        };
        let (w, h) = (37, 9);
        let r = frame(w, h, e);
        let want = reference(&r, e);
        let mut img = renderer.create_image(&gpu, 40 + i as u64, w, h, None);
        renderer.upload_overview(&gpu, &mut img, &r);
        assert_eq!(img.hdr, colour.is_hdr(), "case {i}");
        let peak = img.content_peak;
        let got = draw(&gpu, &mut renderer, &img);
        let mut worst = 0.0f32;
        for (k, (g, w)) in got.iter().zip(&want).enumerate() {
            let m = w[0].max(w[1]).max(w[2]);
            let s = if img.hdr {
                hdr::tone_map_scale(m, peak, 64.0)
            } else {
                1.0
            };
            for c in 0..3 {
                let want = w[c] * s;
                let err = (g[c] - want).abs() / want.abs().max(0.05);
                worst = worst.max(err);
                assert!(
                    err <= 0.03,
                    "case {i} pixel {k} channel {c}: GPU {} vs CPU {want}",
                    g[c]
                );
            }
        }
        eprintln!("yuv case {i}: worst relative difference {worst:.4}");
    }
}

/// 4:4:0 and 4:1:1 chroma (VP9 profile 1, Motion JPEG, DV; they were widened by repeating
/// rows or columns before round 8) and bottom siting (H.273 chroma locations 4 and 5): the
/// shader interpolates at the same positions as the CPU twin.
#[test]
fn gpu_matches_the_cpu_reference_for_440_411_and_bottom_siting() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    renderer.set_display_peak(64.0);
    let cases = [
        // (divisors, bits, cosited_x, cosited_y, bottom_y)
        ((1, 2), 8, false, false, false),
        ((1, 2), 10, false, true, false),
        ((1, 2), 8, false, false, true),
        ((4, 1), 8, true, false, false),
        ((4, 1), 8, false, false, false),
        ((2, 2), 8, true, false, true),
        ((2, 2), 10, false, false, true),
    ];
    for (i, (div, bits, cosited_x, cosited_y, bottom_y)) in cases.into_iter().enumerate() {
        let e = YuvEncoding {
            matrix: YuvMatrix::Bt709,
            full_range: false,
            bits,
            cosited_x,
            cosited_y,
            bottom_y,
            right_x: false,
            alpha: false,
            colour: HdrEncoding::SRGB,
        };
        let (w, h) = (37, 11);
        let r = frame_div(w, h, e, div);
        let want = reference(&r, e);
        let mut img = renderer.create_image(&gpu, 80 + i as u64, w, h, None);
        renderer.upload_overview(&gpu, &mut img, &r);
        let got = draw(&gpu, &mut renderer, &img);
        let mut worst = 0.0f32;
        for (k, (g, w)) in got.iter().zip(&want).enumerate() {
            for c in 0..3 {
                let err = (g[c] - w[c]).abs() / w[c].abs().max(0.05);
                worst = worst.max(err);
                // 0.04: Apple GPUs filter with coarser fixed-point weights (0.0015 off in
                // the darkest pixel, 0.030 relative); a wrong siting is off by far more.
                assert!(
                    err <= 0.04,
                    "case {i} pixel {k} channel {c}: GPU {} vs CPU {}",
                    g[c],
                    w[c]
                );
            }
        }
        eprintln!("yuv 440/411/bottom case {i}: worst relative difference {worst:.4}");
    }
}

/// Still-image planes (HEIC): straight alpha in a fourth plane (8 and 10 bits), chroma on
/// the last luma column it covers (mirrored co-sited chroma) and an ICC source transform.
/// The GPU conversion of the planes draws like the CPU conversion's RGBA uploaded instead.
#[test]
fn alpha_right_siting_and_profile_draw_like_the_cpu_conversion() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let p3 = moxcms::ColorProfile::new_display_p3().encode().unwrap();
    let lut = crate::icc::build_lut(&p3).expect("wide-gamut profile needs a transform");
    // (divisors, bits, right_x, bottom_y, alpha, profile)
    let cases = [
        ((2, 2), 8, true, false, true, false),
        ((2, 2), 10, true, true, true, false),
        ((2, 1), 8, true, false, false, false),
        ((2, 1), 10, false, false, true, false),
        ((2, 2), 8, false, false, false, true),
        ((2, 2), 10, true, false, true, true),
    ];
    for (i, (div, bits, right_x, bottom_y, alpha, profile)) in cases.into_iter().enumerate() {
        let e = YuvEncoding {
            matrix: YuvMatrix::Bt601,
            full_range: i % 2 == 0,
            bits,
            cosited_x: false,
            cosited_y: false,
            bottom_y,
            right_x,
            alpha,
            colour: HdrEncoding::SRGB,
        };
        let (w, h) = (37, 11);
        let mut r = frame_div(w, h, e, div);
        if alpha {
            let max = (1u32 << bits) - 1;
            let mut data = Vec::new();
            for y in 0..h {
                for x in 0..w {
                    let a = ((x * 13 + y * 29) % 97) * max / 96;
                    if bits > 8 {
                        data.extend_from_slice(&(a as u16).to_ne_bytes());
                    } else {
                        data.push(a as u8);
                    }
                }
            }
            r.planes.push(RasterPlane {
                width: w,
                height: h,
                stride: w as usize * e.sample_bytes(),
                data,
                h_div: 1,
                v_div: 1,
            });
        }
        let mut rgb = yuv::to_rgb(&r).expect("cpu conversion");
        if profile && bits > 8 {
            // SDR planes go through a profile like 8-bit RGBA (clipped to reference white),
            // not like 16-bit RGBA (whose wide-gamut excursions stay in a half-float level).
            let p = &mut rgb.planes[0];
            p.data = p
                .data
                .chunks_exact(2)
                .map(|b| ((u32::from(u16::from_ne_bytes([b[0], b[1]])) + 128) / 257) as u8)
                .collect();
            p.stride /= 2;
            rgb.layout = PixelLayout::Rgba;
        }
        let lut = profile.then_some(&lut);
        let id = 120 + 2 * i as u64;
        let mut a = renderer.create_image(&gpu, id, w, h, lut);
        renderer.upload_overview(&gpu, &mut a, &r);
        let got = draw(&gpu, &mut renderer, &a);
        let mut b = renderer.create_image(&gpu, id + 1, w, h, lut);
        renderer.upload_overview(&gpu, &mut b, &rgb);
        let want = draw(&gpu, &mut renderer, &b);
        let mut worst = 0.0f32;
        for (k, (g, w)) in got.iter().zip(&want).enumerate() {
            for c in 0..4 {
                // In sRGB steps: the RGBA route rounds to 8 bits before a profile's transform,
                // which a wide-gamut matrix widens to about two steps on saturated edges.
                let enc = |v: f32| hdr::linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0;
                let err = if c == 3 {
                    (g[c] - w[c]).abs() * 255.0
                } else {
                    (enc(g[c]) - enc(w[c])).abs()
                };
                worst = worst.max(err);
                assert!(
                    err <= 2.5,
                    "case {i} pixel {k} channel {c}: planes {} vs RGBA {}",
                    g[c],
                    w[c]
                );
            }
        }
        // The profile is applied: a saturated pixel differs from the untagged draw.
        if profile {
            let mut c = renderer.create_image(&gpu, 200 + i as u64, w, h, None);
            renderer.upload_overview(&gpu, &mut c, &r);
            let plain = draw(&gpu, &mut renderer, &c);
            let moved = plain
                .iter()
                .zip(&got)
                .any(|(p, g)| (p[0] - g[0]).abs() > 0.02 || (p[1] - g[1]).abs() > 0.02);
            assert!(moved, "case {i}: the profile changed nothing");
        }
        eprintln!("yuv alpha/right/profile case {i}: worst difference {worst:.2} sRGB steps");
    }
}

/// AVIF stills as `specular-decode` hands them to the display: their planes (8/10-bit,
/// 4:2:0 with unknown, vertical and colocated siting, 4:2:2, 4:4:4, a grid, alpha, PQ) drawn
/// by the shader like their CPU conversion uploaded as RGBA instead.
#[test]
fn avif_planes_draw_like_the_cpu_conversion() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    renderer.set_display_peak(64.0);
    let registry = specular_decode::Registry::new();
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../formats/specular-avif/tests/data/"
    );
    let names = [
        "rgb8_420.avif",
        "rgb10_420.avif",
        "rgb8_444.avif",
        "alpha.avif",
        "grid.avif",
        "grid422.avif",
        "siting420-colocated.avif",
        "siting420-vertical.avif",
        "siting420-10bit-colocated.avif",
        "siting422-odd.avif",
        "pq10.avif",
    ];
    for (i, name) in names.into_iter().enumerate() {
        let bytes = std::fs::read(format!("{dir}{name}")).unwrap();
        let src = std::sync::Arc::new(specular_core::Source::from_vec(name, bytes));
        let s = registry.open(src).unwrap();
        let r = s
            .decode(
                &specular_decode::DecodeRequest::full(1),
                &specular_core::CancelToken::new(),
            )
            .unwrap();
        let PixelLayout::Yuv(e) = r.layout else {
            panic!("{name}: {:?}", r.layout)
        };
        let rgb = yuv::to_rgb(&r).expect("cpu conversion");
        let (w, h) = (r.width, r.height);
        let id = 300 + 2 * i as u64;
        let mut a = renderer.create_image(&gpu, id, w, h, None);
        renderer.upload_overview(&gpu, &mut a, &r);
        let got = draw(&gpu, &mut renderer, &a);
        let mut b = renderer.create_image(&gpu, id + 1, w, h, None);
        renderer.upload_overview(&gpu, &mut b, &rgb);
        let want = draw(&gpu, &mut renderer, &b);
        assert_eq!(a.hdr, e.colour.is_hdr(), "{name}");
        let mut worst = 0.0f32;
        for (k, (g, w)) in got.iter().zip(&want).enumerate() {
            for c in 0..4 {
                let err = if c == 3 {
                    (g[c] - w[c]).abs() * 255.0
                } else if a.hdr {
                    // Per cent, relative, in linear light.
                    (g[c] - w[c]).abs() / w[c].abs().max(0.05) * 100.0
                } else {
                    // In sRGB steps.
                    let enc = |v: f32| hdr::linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0;
                    (enc(g[c]) - enc(w[c])).abs()
                };
                worst = worst.max(err);
                assert!(
                    err <= 2.5,
                    "{name} pixel {k} channel {c}: planes {} vs RGBA {}",
                    g[c],
                    w[c]
                );
            }
        }
        eprintln!("avif {name}: worst difference {worst:.2}");
    }
}
