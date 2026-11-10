//! CMYK / YCCK JPEGs (Adobe APP14) decode to RGBA matching djpeg's CMYK->RGB conversion.

mod common;

use common::*;
use jpeg_encoder::{ColorType, Encoder, SamplingFactor};
use specular_jpeg::*;

fn rgb_source(w: u32, h: u32) -> Vec<u8> {
    let p = parse_pnm(&std::fs::read(source_ppm(w, h, 21)).unwrap());
    assert_eq!(p.channels, 3);
    p.data
}

/// Naive RGB -> CMYK (ink amounts, 0 = no ink) as jpeg-encoder expects.
fn to_cmyk(rgb: &[u8]) -> Vec<u8> {
    rgb.chunks_exact(3)
        .flat_map(|p| {
            let k = 255 - *p.iter().max().unwrap();
            let f = |c: u8| if k == 255 { 0 } else { (255 - c - k) as u32 * 255 / (255 - k) as u32 } as u8;
            [f(p[0]), f(p[1]), f(p[2]), k]
        })
        .collect()
}

fn check(name: &str, jpg: &[u8]) {
    let info = read_info(jpg).unwrap();
    assert!(
        matches!(info.color, ColorModel::Cmyk | ColorModel::Ycck),
        "{name}: {:?}",
        info.color
    );
    for par in [Parallelism::Single, Parallelism::Threads(4)] {
        let d = decode(
            jpg,
            &DecodeOptions {
                parallelism: par,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(d.layout, PlanesLayout::Rgba);
        let ours = planes_to_rgba(&d);
        let refi = djpeg(jpg, &["-dct", "int", "-ppm"]);
        let (max, mean) = diff_rgba_pnm(&ours, d.width as usize, d.height as usize, &refi);
        assert!(max <= 2, "{name} {par:?}: max diff {max} (mean {mean:.4})");
        // Scaled decodes: dimensions and a loose colour check against djpeg -scale.
        for (scale, frac) in [
            (Scale::Half, "1/2"),
            (Scale::Quarter, "1/4"),
            (Scale::Eighth, "1/8"),
        ] {
            let d = decode(
                jpg,
                &DecodeOptions {
                    scale,
                    parallelism: par,
                    ..Default::default()
                },
            )
            .unwrap();
            let refs = djpeg(jpg, &["-dct", "int", "-scale", frac, "-ppm"]);
            let (max, mean) = diff_rgba_pnm(
                &planes_to_rgba(&d),
                d.width as usize,
                d.height as usize,
                &refs,
            );
            if scale == Scale::Eighth && name.contains("_22_") {
                // At 1/8 every component is DC-only here, while libjpeg uses a 2x2 IDCT for
                // subsampled components (see the crate-level docs): only a loose check. The
                // gap depends on the libjpeg build too (12.2 against libjpeg-turbo 3.2).
                assert!(mean < 14.0, "{name} {frac}: mean diff {mean}");
            } else {
                assert!(max <= 3, "{name} {frac}: max diff {max} (mean {mean:.3})");
            }
        }
    }
}

#[test]
fn cmyk_and_ycck_match_djpeg() {
    if !tools_available() {
        return;
    }
    for (w, h) in [(203u32, 117u32), (64, 48), (9, 5)] {
        let cmyk = to_cmyk(&rgb_source(w, h));
        for (ct, cname) in [(ColorType::Cmyk, "cmyk"), (ColorType::CmykAsYcck, "ycck")] {
            for (sf, sname) in [(SamplingFactor::F_1_1, "11"), (SamplingFactor::F_2_2, "22")] {
                for prog in [false, true] {
                    let mut jpg = Vec::new();
                    let mut e = Encoder::new(&mut jpg, 90);
                    e.set_sampling_factor(sf);
                    e.set_progressive(prog);
                    e.encode(&cmyk, w as u16, h as u16, ct).unwrap();
                    check(
                        &format!(
                            "{cname}_{sname}_{}_{w}x{h}",
                            if prog { "prog" } else { "base" }
                        ),
                        &jpg,
                    );
                }
            }
        }
    }
    // ImageMagick (libjpeg-turbo) CMYK output.
    let src = source_ppm(150, 99, 4);
    let out = tmp_dir().join(format!("magick_cmyk_{}.jpg", uniq()));
    let st = std::process::Command::new("magick")
        .arg(&src)
        .args(["-colorspace", "CMYK", "-quality", "92"])
        .arg(&out)
        .status()
        .unwrap();
    assert!(st.success());
    let jpg = std::fs::read(&out).unwrap();
    let _ = std::fs::remove_file(&out);
    check("magick_cmyk", &jpg);
}
