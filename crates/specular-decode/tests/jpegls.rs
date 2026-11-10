//! JPEG-LS through the registry: probed ahead of JPEG (both start with SOI), grey and colour
//! layouts, samples scaled from MAXVAL, reduced levels. Streams are specular-jpegls' vectors.

use std::sync::Arc;

use specular_core::{CancelToken, HdrEncoding, PixelLayout, Raster, Source};
use specular_decode::{DecodeRequest, Format, Registry};

fn vector(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../formats/specular-jpegls/tests/vectors/{name}.jls",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn u16s(r: &Raster) -> Vec<u16> {
    let p = &r.planes[0];
    let row = r.width as usize * r.layout.pixel_bytes();
    (0..r.height as usize)
        .flat_map(|y| p.data[y * p.stride..y * p.stride + row].chunks_exact(2))
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
        .collect()
}

#[test]
fn layouts_and_samples() {
    let c = CancelToken::new();
    let cases = [
        ("grey8", PixelLayout::Gray, 1),
        ("grey5", PixelLayout::Gray, 1),
        ("grey16", PixelLayout::Gray16, 1),
        ("rgb8_ilv2", PixelLayout::Rgba, 3),
        ("rgba8_ilv2", PixelLayout::Rgba, 4),
        ("rgb16_ilv1", PixelLayout::Rgba16(HdrEncoding::SRGB), 3),
    ];
    for (name, layout, n) in cases {
        let data = vector(name);
        let img = specular_jpegls::decode(&data, &Default::default()).unwrap();
        // A .jpg name must not send it to the JPEG decoder.
        for file in [format!("{name}.jls"), format!("{name}.jpg")] {
            let s = Registry::default()
                .open(Arc::new(Source::from_vec(file.clone(), data.clone())))
                .unwrap_or_else(|e| panic!("{file}: {e}"));
            assert_eq!(s.info().format, Format::JpegLs, "{file}");
            assert_eq!(s.info().bit_depth, img.precision, "{file}");
            assert_eq!(s.info().has_alpha, n == 4, "{file}");
            let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
            assert_eq!(r.layout, layout, "{file}");
            assert_eq!((r.width, r.height), (img.width, img.height), "{file}");
            let max = (1u32 << img.precision) - 1;
            let out_max = if img.precision > 8 { 65535 } else { 255 };
            let want: Vec<u32> = (0..img.width as usize * img.height as usize)
                .flat_map(|i| {
                    let mut px: Vec<u32> = img
                        .planes
                        .iter()
                        .map(|p| (p.data[i] as u32 * out_max + max / 2) / max)
                        .collect();
                    if n == 3 {
                        px.push(out_max);
                    }
                    px
                })
                .collect();
            let got: Vec<u32> = if img.precision > 8 {
                u16s(&r).into_iter().map(u32::from).collect()
            } else {
                let p = &r.planes[0];
                let row = r.width as usize * r.layout.pixel_bytes();
                (0..r.height as usize)
                    .flat_map(|y| &p.data[y * p.stride..y * p.stride + row])
                    .map(|&v| v as u32)
                    .collect()
            };
            assert_eq!(got, want, "{file}");
            let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
            assert_eq!(half.layout, layout, "{file} at 1:2");
            assert_eq!(half.width, img.width.div_ceil(2), "{file} at 1:2");
        }
    }
}

#[test]
fn custom_maxval_is_full_intensity_and_tables_are_refused() {
    let c = CancelToken::new();
    let data = vector("grey12_maxval");
    let img = specular_jpegls::decode(&data, &Default::default()).unwrap();
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("m.jls", data)))
        .unwrap();
    let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
    // MAXVAL 3000 maps to 65535.
    let want: Vec<u16> = img.planes[0]
        .data
        .iter()
        .map(|&v| ((v as u32 * 65535 + 1500) / 3000) as u16)
        .collect();
    assert_eq!(u16s(&r), want);
    let e = Registry::default()
        .open(Arc::new(Source::from_vec("t.jls", vector("pal6_table"))))
        .err()
        .unwrap();
    assert!(e.to_string().contains("mapping tables"), "{e}");
}
